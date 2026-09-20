#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use pdf_toolkit_core::{
    convert,
    model::{self, Comment, Group, Project},
    office,
    pdf::Engine,
    pipeline::{self, OutputReport, PdfOutputSettings},
    storage,
};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tauri::{Emitter, Manager};
#[derive(Default)]
struct Session {
    project: Project,
    sources: HashMap<String, Vec<u8>>,
    names: HashMap<String, String>,
    inputs: Vec<String>,
    undo: Vec<Project>,
    redo: Vec<Project>,
    pending: Vec<(String, Vec<u8>)>,
    output_token: Option<String>,
    output_reports: Vec<OutputReport>,
    comparisons: Vec<(Vec<u8>, Vec<u8>, usize)>,
    pending_import: Option<(String, Vec<u8>)>,
    dirty: bool,
    recovery_error: Option<String>,
}
struct State {
    session: Mutex<Session>,
    cancel: AtomicBool,
    busy: AtomicBool,
    dll: PathBuf,
    recovery: PathBuf,
}
fn uri(b: Vec<u8>) -> String {
    format!("data:image/png;base64,{}", STANDARD.encode(b))
}
fn field<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v[key].as_str().context(format!("{key} がありません"))
}
fn strings(v: &Value, key: &str) -> Vec<String> {
    v[key]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|s| s.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}
fn snapshot(s: &Session) -> Value {
    json!({"project":s.project,"names":s.names,"canUndo":!s.undo.is_empty(),"canRedo":!s.redo.is_empty(),"dirty":s.dirty,"recoveryError":s.recovery_error})
}
fn checkpoint(s: &mut Session, old: Project) {
    s.undo.push(old);
    if s.undo.len() > 60 {
        s.undo.remove(0);
    }
    s.redo.clear();
    s.dirty = true;
}
fn clear_output(s: &mut Session) {
    s.pending.clear();
    s.output_token = None;
    s.output_reports.clear();
    s.comparisons.clear();
}
fn authorize_output(s: &Session, request: &Value) -> Result<()> {
    anyhow::ensure!(
        s.output_token
            .as_deref()
            .is_some_and(|token| Some(token) == request["token"].as_str())
            && !s.pending.is_empty(),
        "出力結果が更新されました。もう一度確認してください"
    );
    for report in &s.output_reports {
        report.authorize_save(request["acknowledgeUnmet"].as_bool().unwrap_or(false))?;
    }
    Ok(())
}

#[cfg(test)]
mod output_tests {
    use super::*;
    #[test]
    fn stale_output_token_and_unacknowledged_target_are_rejected() {
        let mut session = Session {
            output_token: Some("current".into()),
            ..Default::default()
        };
        session.pending.push(("test.pdf".into(), vec![1]));
        session
            .output_reports
            .push(pdf_toolkit_core::pipeline::OutputReport {
                original_bytes: 10,
                final_bytes: 10,
                target_bytes: Some(1),
                met_target: Some(false),
                attempts: 1,
                setting: "test".into(),
                warnings: vec![],
                protected: false,
            });
        assert!(
            authorize_output(&session, &json!({"token":"old","acknowledgeUnmet":true})).is_err()
        );
        assert!(authorize_output(&session, &json!({"token":"current"})).is_err());
        assert!(authorize_output(
            &session,
            &json!({"token":"current","acknowledgeUnmet":true})
        )
        .is_ok());
        clear_output(&mut session);
        assert!(session.pending.is_empty());
        assert!(authorize_output(
            &session,
            &json!({"token":"current","acknowledgeUnmet":true})
        )
        .is_err());
    }
}
fn import_bytes(
    s: &mut Session,
    e: &Engine,
    bytes: &[u8],
    password: Option<&str>,
    name: &str,
    cancel: &AtomicBool,
) -> Result<()> {
    let source = model::id();
    let (data, mut pages) = e.import_checked(bytes, password, &source, |_, _| {
        anyhow::ensure!(!cancel.load(Ordering::Relaxed), "処理をキャンセルしました");
        Ok(())
    })?;
    for page in &mut pages {
        page.source_name = name.into();
    }
    s.sources.insert(source.clone(), data);
    s.names.insert(source, name.into());
    s.project.groups.push(Group {
        pdf_settings: PdfOutputSettings::default(),
        id: model::id(),
        name: std::path::Path::new(name)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into(),
        pages,
    });
    Ok(())
}
fn execute(state: &State, app: &tauri::AppHandle, v: Value) -> Result<Value> {
    let op = field(&v, "op")?;
    let mut s = state
        .session
        .lock()
        .map_err(|_| anyhow::anyhow!("作業データを取得できません"))?;
    let progress = |message: &str, percent: u32| {
        let _ = app.emit("job-progress", json!({"message":message,"percent":percent}));
    };
    let check = || -> Result<()> {
        anyhow::ensure!(
            !state.cancel.load(Ordering::Relaxed),
            "処理をキャンセルしました"
        );
        Ok(())
    };
    if op == "state" {
        return Ok(snapshot(&s));
    }
    if op == "detect" {
        return Ok(serde_json::from_str(&office::run(
            json!({"action":"detect"}),
        )?)?);
    }
    if op == "recovery_exists" {
        return Ok(json!(state.recovery.exists()));
    }
    if op == "discard_export" {
        clear_output(&mut s);
        return Ok(Value::Null);
    }
    let e = Engine::new(&state.dll)?;
    if op == "thumbnail" {
        let pid = field(&v, "id")?;
        let p = s
            .project
            .groups
            .iter()
            .flat_map(|g| &g.pages)
            .find(|p| p.id == pid)
            .context("ページがありません")?;
        let width = v["width"].as_i64().unwrap_or(250) as i32;
        return Ok(json!(uri(e.render(
            s.sources.get(&p.source).context("PDFがありません")?,
            p.index,
            width,
            true
        )?)));
    }
    if op == "comment_image" {
        let c: Comment = serde_json::from_value(v["comment"].clone())?;
        return Ok(json!(uri(pdf_toolkit_core::pdf::comment_png(&c)?)));
    }
    if op == "sheets" {
        return Ok(serde_json::from_str(&office::run_with_cancel(
            json!({"action":"sheets","input":field(&v,"path")?}),
            &state.cancel,
        )?)?);
    }
    if op == "prepare_office" {
        progress("OfficeでPDFに変換中…", 10);
        let path = field(&v, "path")?;
        let dir = std::env::temp_dir().join(format!("pdftk-{}.pdf", model::id()));
        let result = office::run_with_cancel(
            json!({"action":"convert","input":path,"output":dir,"sheets":strings(&v,"sheets")}),
            &state.cancel,
        );
        if let Err(err) = result {
            let _ = std::fs::remove_file(&dir);
            return Err(err);
        }
        let bytes = std::fs::read(&dir);
        let _ = std::fs::remove_file(&dir);
        let bytes = bytes?;
        check()?;
        let (_, pages) = e.import(&bytes, None, "preview")?;
        let preview = uri(e.render(&bytes, 0, 1000, false)?);
        s.pending_import = Some((path.to_owned(), bytes));
        return Ok(json!({"preview":preview,"pages":pages.len()}));
    }
    if op == "office_preview" {
        let (_, bytes) = s.pending_import.as_ref().context("変換結果がありません")?;
        return Ok(json!(uri(e.render(
            bytes,
            v["index"].as_u64().unwrap_or(0) as u16,
            1000,
            false
        )?)));
    }
    if op == "prepare_export" {
        clear_output(&mut s);
        let format = field(&v, "format")?;
        anyhow::ensure!(
            ["pdf", "docx", "xlsx"].contains(&format),
            "変換形式が不正です"
        );
        let gids = strings(&v, "groups");
        let groups: Vec<_> = s
            .project
            .groups
            .iter()
            .filter(|g| !g.pages.is_empty() && (gids.is_empty() || gids.contains(&g.id)))
            .cloned()
            .collect();
        anyhow::ensure!(!groups.is_empty(), "グループがありません");
        let mut pending = vec![];
        let mut previews = vec![];
        let mut reports = vec![];
        let mut comparisons = vec![];
        let mut used = std::collections::HashSet::new();
        for (i, g) in groups.iter().enumerate() {
            let settings = &g.pdf_settings;
            check()?;
            model::validate_name(&g.name)?;
            let filename = format!("{}.{format}", g.name.trim_end_matches(".pdf"));
            anyhow::ensure!(
                used.insert(filename.to_lowercase()),
                "出力ファイル名が重複しています"
            );
            progress(
                &format!("{} を変換中", g.name),
                (i * 100 / groups.len()) as u32,
            );
            let pdf = e.export_checked(&g.pages, &s.sources, |n, total| {
                check()?;
                progress(
                    &format!("{}：{} / {} ページ", g.name, n + 1, total),
                    (n * 100 / total) as u32,
                );
                Ok(())
            })?;
            let bytes = if format == "pdf" {
                let output =
                    pipeline::prepare_pdf(&pdf, settings, v["password"].as_str(), |n, total| {
                        check()?;
                        progress(
                            &format!("{}：圧縮候補 {} / {}", g.name, n, total),
                            (n * 100 / total) as u32,
                        );
                        Ok(())
                    })?;
                // Verify actual final file with PDFium, then preview the decrypted final PDF.
                e.verify_output(
                    &output.bytes,
                    if settings.protect {
                        v["password"].as_str()
                    } else {
                        None
                    },
                    g.pages.len(),
                    |n, total| {
                        check()?;
                        progress(
                            &format!("{}：最終PDFを検証中 {} / {}", g.name, n, total),
                            (n * 100 / total) as u32,
                        );
                        Ok(())
                    },
                )?;
                previews.push(uri(e.render(&output.preview, 0, 700, false)?));
                comparisons.push((pdf, output.preview, g.pages.len()));
                reports.push(output.report);
                output.bytes
            } else {
                let mut images = vec![];
                for (n, p) in g.pages.iter().enumerate() {
                    check()?;
                    let (w, h) = if p.rotation % 180 == 0 {
                        (p.width, p.height)
                    } else {
                        (p.height, p.width)
                    };
                    let dpi = if v["quality"] == "high" { 300. } else { 150. };
                    images.push((
                        e.render(&pdf, n as u16, (w * dpi / 72.) as i32, false)?,
                        w,
                        h,
                    ));
                }
                let bytes = convert::office_document(format, &images)?;
                previews.extend(
                    convert::preview_images(&bytes, format)?
                        .into_iter()
                        .map(uri),
                );
                bytes
            };
            pending.push((filename, bytes));
        }
        check()?;
        let names: Vec<_> = pending.iter().map(|(n, _)| n.clone()).collect();
        s.pending = pending;
        s.output_reports = reports;
        s.comparisons = comparisons;
        let token = model::id();
        s.output_token = Some(token.clone());
        return Ok(
            json!({"token":token,"names":names,"previews":previews,"reports":s.output_reports,"pageCounts":s.comparisons.iter().map(|(_,_,n)|*n).collect::<Vec<_>>()}),
        );
    }
    if op == "output_preview" {
        anyhow::ensure!(
            s.output_token
                .as_deref()
                .is_some_and(|t| Some(t) == v["token"].as_str()),
            "出力結果が更新されました"
        );
        let group = v["group"].as_u64().context("グループが不正です")? as usize;
        let index = v["index"].as_u64().context("ページが不正です")? as usize;
        let (before, after, count) = s.comparisons.get(group).context("比較データがありません")?;
        anyhow::ensure!(
            index < *count && index <= u16::MAX as usize,
            "ページが範囲外です"
        );
        let width = v["width"].as_i64().unwrap_or(700).clamp(200, 2000) as i32;
        let before = uri(e.render(before, index as u16, width, false)?);
        check()?;
        let after = uri(e.render(after, index as u16, width, false)?);
        return Ok(json!({"before":before,"after":after}));
    }
    if op == "export_conflicts" {
        let dir = PathBuf::from(field(&v, "path")?);
        return Ok(json!(s
            .pending
            .iter()
            .filter(|(n, _)| dir.join(n).exists())
            .map(|(n, _)| n.clone())
            .collect::<Vec<_>>()));
    }
    if op == "finish_export" {
        authorize_output(&s, &v)?;
        let dir = PathBuf::from(field(&v, "path")?);
        anyhow::ensure!(dir.is_dir(), "保存先フォルダーがありません");
        let overwrite = v["overwrite"].as_bool().unwrap_or(false);
        let mut written = vec![];
        for (n, b) in &s.pending {
            check()?;
            let path = dir.join(n);
            storage::write_file(&path, b, overwrite, &s.inputs).with_context(|| {
                format!(
                    "保存済み：{}。保存できなかったファイル：{}",
                    written
                        .iter()
                        .map(|p: &PathBuf| p.display().to_string())
                        .collect::<Vec<_>>()
                        .join("、"),
                    path.display()
                )
            })?;
            written.push(path);
        }
        clear_output(&mut s);
        return Ok(json!(written));
    }
    if op == "save_project" {
        let path = PathBuf::from(field(&v, "path")?);
        anyhow::ensure!(
            path.extension().is_some_and(|x| x == "pdftk"),
            "作業ファイルの拡張子は.pdftkです"
        );
        storage::save_checked(&path, &s.project, &s.sources, check)?;
        s.dirty = false;
        return Ok(snapshot(&s));
    }
    if op == "discard_recovery" {
        if state.recovery.exists() {
            std::fs::remove_file(&state.recovery)?;
        }
        return Ok(Value::Null);
    }
    let before = s.project.clone();
    clear_output(&mut s);
    match op {
        "set_output_settings" => {
            let settings: PdfOutputSettings = serde_json::from_value(v["settings"].clone())?;
            if settings.mode == pipeline::CompressionMode::Target {
                pdf_toolkit_core::output::TargetSize::from_mb(&settings.target_mb)?;
            }
            let ids = strings(&v, "groups");
            anyhow::ensure!(
                !ids.is_empty()
                    && ids
                        .iter()
                        .all(|id| s.project.groups.iter().any(|g| &g.id == id)),
                "出力対象がありません"
            );
            for group in &mut s.project.groups {
                if ids.contains(&group.id) {
                    group.pdf_settings = settings.clone();
                }
            }
        }
        "sample" => {
            let b = e.sample()?;
            import_bytes(&mut s, &e, &b, None, "サンプル.pdf", &state.cancel)?;
        }
        "import" => {
            let path = field(&v, "path")?;
            let b = std::fs::read(path)?;
            check()?;
            import_bytes(
                &mut s,
                &e,
                &b,
                v["password"].as_str(),
                std::path::Path::new(path)
                    .file_name()
                    .unwrap_or_default()
                    .to_str()
                    .unwrap_or("PDF"),
                &state.cancel,
            )?;
            s.inputs.push(path.into());
            if let Some(group) = s.project.groups.last_mut() {
                for page in &mut group.pages {
                    page.source_path = path.into();
                }
            }
        }
        "accept_office" => {
            let (path, b) = s.pending_import.take().context("変換結果がありません")?;
            import_bytes(
                &mut s,
                &e,
                &b,
                None,
                std::path::Path::new(&path)
                    .file_name()
                    .unwrap_or_default()
                    .to_str()
                    .unwrap_or("Office"),
                &state.cancel,
            )?;
            if let Some(group) = s.project.groups.last_mut() {
                for page in &mut group.pages {
                    page.source_path = path.clone();
                }
            }
            s.inputs.push(path);
        }
        "open_project" | "recover" => {
            let path = if op == "recover" {
                state.recovery.clone()
            } else {
                PathBuf::from(field(&v, "path")?)
            };
            let (p, sources) = storage::load(&path)?;
            s.project = p;
            s.sources = sources;
            s.names = s
                .project
                .groups
                .iter()
                .flat_map(|g| {
                    g.pages
                        .iter()
                        .map(|p| (p.source.clone(), p.source_name.clone()))
                })
                .collect();
            s.inputs = s
                .project
                .groups
                .iter()
                .flat_map(|g| g.pages.iter().map(|p| p.source_path.clone()))
                .filter(|s| !s.is_empty())
                .collect();
            s.pending.clear();
            s.pending_import = None;
            s.undo.clear();
            s.redo.clear();
            s.dirty = op == "recover";
            return Ok(snapshot(&s));
        }
        "rotate" => model::rotate(
            &mut s.project,
            &strings(&v, "ids"),
            v["delta"].as_i64().unwrap_or(90) as i32,
        )?,
        "delete" => {
            let ids = strings(&v, "ids");
            for g in &mut s.project.groups {
                g.pages.retain(|p| !ids.contains(&p.id));
            }
        }
        "transfer" => model::transfer(
            &mut s.project,
            &strings(&v, "ids"),
            field(&v, "target")?,
            v["at"].as_u64().unwrap_or(0) as usize,
            v["copy"].as_bool().unwrap_or(false),
        )?,
        "extract" => {
            let target = model::id();
            let name = format!("抽出 {}", s.project.groups.len() + 1);
            let ids = strings(&v, "ids");
            let pdf_settings = s
                .project
                .groups
                .iter()
                .find(|g| g.pages.iter().any(|p| ids.contains(&p.id)))
                .map(|g| g.pdf_settings.clone())
                .unwrap_or_default();
            s.project.groups.push(Group {
                pdf_settings,
                id: target.clone(),
                name,
                pages: vec![],
            });
            if let Err(err) = model::transfer(
                &mut s.project,
                &strings(&v, "ids"),
                &target,
                0,
                v["copy"].as_bool().unwrap_or(false),
            ) {
                s.project = before;
                return Err(err);
            }
        }
        "split" => {
            let gid = field(&v, "group")?;
            let g = s
                .project
                .groups
                .iter_mut()
                .find(|g| g.id == gid)
                .context("グループがありません")?;
            let at = v["at"].as_u64().unwrap_or(0) as usize;
            anyhow::ensure!(at > 0 && at < g.pages.len(), "分割位置が無効です");
            let pages = g.pages.split_off(at);
            let name = format!("{} 分割", g.name);
            let pdf_settings = g.pdf_settings.clone();
            s.project.groups.push(Group {
                pdf_settings,
                id: model::id(),
                name,
                pages,
            });
        }
        "merge" => {
            let source = field(&v, "source")?;
            let target = field(&v, "target")?;
            anyhow::ensure!(source != target, "同じグループです");
            let ids = s
                .project
                .groups
                .iter()
                .find(|g| g.id == source)
                .context("グループがありません")?
                .pages
                .iter()
                .map(|p| p.id.clone())
                .collect::<Vec<_>>();
            let at = s
                .project
                .groups
                .iter()
                .find(|g| g.id == target)
                .context("移動先がありません")?
                .pages
                .len();
            model::transfer(&mut s.project, &ids, target, at, false)?;
            s.project.groups.retain(|g| g.id != source);
        }
        "rename" => {
            let name = field(&v, "name")?;
            model::validate_name(name)?;
            s.project
                .groups
                .iter_mut()
                .find(|g| Some(g.id.as_str()) == v["id"].as_str())
                .context("グループがありません")?
                .name = name.into();
        }
        "remove_group" => {
            let gid = field(&v, "id")?;
            s.project.groups.retain(|g| g.id != gid);
        }
        "comment" => {
            let mut c: Comment = serde_json::from_value(v["comment"].clone())?;
            pdf_toolkit_core::pdf::comment_image(&c)?;
            let p = s
                .project
                .groups
                .iter_mut()
                .flat_map(|g| &mut g.pages)
                .find(|p| Some(p.id.as_str()) == v["page"].as_str())
                .context("ページがありません")?;
            anyhow::ensure!(
                c.x >= 0.
                    && c.y >= 0.
                    && c.x + c.width <= p.width + 0.1
                    && c.y + c.height <= p.height + 0.1,
                "コメントをページ内に配置してください"
            );
            if c.id.is_empty() {
                c.id = model::id();
            }
            p.comments.retain(|old| old.id != c.id);
            p.comments.push(c);
        }
        "delete_comment" => {
            let p = s
                .project
                .groups
                .iter_mut()
                .flat_map(|g| &mut g.pages)
                .find(|p| Some(p.id.as_str()) == v["page"].as_str())
                .context("ページがありません")?;
            p.comments
                .retain(|c| Some(c.id.as_str()) != v["id"].as_str());
        }
        "undo" => {
            if let Some(p) = s.undo.pop() {
                s.redo.push(before);
                s.project = p;
            }
            s.dirty = true;
            s.recovery_error = storage::save(&state.recovery, &s.project, &s.sources)
                .err()
                .map(|e| e.to_string());
            return Ok(snapshot(&s));
        }
        "redo" => {
            if let Some(p) = s.redo.pop() {
                s.undo.push(before);
                s.project = p;
            }
            s.dirty = true;
            s.recovery_error = storage::save(&state.recovery, &s.project, &s.sources)
                .err()
                .map(|e| e.to_string());
            return Ok(snapshot(&s));
        }
        _ => anyhow::bail!("未対応の操作です: {op}"),
    }
    checkpoint(&mut s, before);
    s.recovery_error = storage::save(&state.recovery, &s.project, &s.sources)
        .err()
        .map(|e| e.to_string());
    Ok(snapshot(&s))
}
#[tauri::command]
async fn dispatch(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<State>>,
    request: Value,
) -> std::result::Result<Value, String> {
    let state = state.inner().clone();
    if state.busy.swap(true, Ordering::SeqCst) {
        return Err("別の処理が実行中です".into());
    }
    state.cancel.store(false, Ordering::SeqCst);
    let inner = state.clone();
    let result = tauri::async_runtime::spawn_blocking(move || execute(&inner, &app, request)).await;
    state.busy.store(false, Ordering::SeqCst);
    result
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))
}
#[tauri::command]
fn cancel(state: tauri::State<'_, Arc<State>>) {
    state.cancel.store(true, Ordering::Relaxed);
}
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let resource = app.path().resource_dir()?;
            let dll = if resource.join("pdfium.dll").exists() {
                resource.join("pdfium.dll")
            } else {
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../vendor/pdfium/bin/pdfium.dll")
            };
            let data = app.path().app_local_data_dir()?;
            std::fs::create_dir_all(&data)?;
            app.manage(Arc::new(State {
                session: Mutex::new(Session::default()),
                cancel: AtomicBool::new(false),
                busy: AtomicBool::new(false),
                dll,
                recovery: data.join("recovery.pdftk"),
            }));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![dispatch, cancel])
        .run(tauri::generate_context!())
        .expect("アプリを起動できませんでした");
}
