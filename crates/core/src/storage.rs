use crate::model::Project;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    io::{Cursor, Read, Write},
    path::Path,
};
use zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};
#[derive(Serialize, Deserialize)]
struct Manifest {
    version: u32,
    project: Project,
}
pub fn save(path: &Path, project: &Project, sources: &HashMap<String, Vec<u8>>) -> Result<()> {
    save_checked(path, project, sources, || Ok(()))
}
pub fn save_checked(
    path: &Path,
    project: &Project,
    sources: &HashMap<String, Vec<u8>>,
    check: impl Fn() -> Result<()>,
) -> Result<()> {
    check()?;
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    zip.start_file("manifest.json", options)?;
    zip.write_all(&serde_json::to_vec(&Manifest {
        version: 1,
        project: project.clone(),
    })?)?;
    let used: HashSet<_> = project
        .groups
        .iter()
        .flat_map(|g| g.pages.iter().map(|p| &p.source))
        .collect();
    for source in used {
        anyhow::ensure!(
            source
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-'),
            "入力IDが不正です"
        );
        zip.start_file(format!("sources/{source}.pdf"), options)?;
        for chunk in sources
            .get(source)
            .context("元PDFがありません")?
            .chunks(1024 * 1024)
        {
            check()?;
            zip.write_all(chunk)?;
        }
    }
    let bytes = zip.finish()?.into_inner();
    check()?;
    write_file(path, &bytes, true, &[])
}
pub fn load(path: &Path) -> Result<(Project, HashMap<String, Vec<u8>>)> {
    let mut zip = ZipArchive::new(std::fs::File::open(path)?)?;
    let mut manifest = String::new();
    zip.by_name("manifest.json")?
        .take(16 * 1024 * 1024)
        .read_to_string(&mut manifest)?;
    let m: Manifest = serde_json::from_str(&manifest)?;
    anyhow::ensure!(
        m.version == 1,
        "この作業ファイルのバージョンには対応していません"
    );
    let mut sources = HashMap::new();
    let mut total = 0u64;
    for p in m.project.groups.iter().flat_map(|g| &g.pages) {
        anyhow::ensure!(
            p.source
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-'),
            "不正な入力ID"
        );
        anyhow::ensure!(
            p.width.is_finite()
                && p.height.is_finite()
                && p.width > 0.
                && p.height > 0.
                && [0, 90, 180, 270].contains(&p.rotation),
            "不正なページ情報"
        );
        if !sources.contains_key(&p.source) {
            let mut file = zip.by_name(&format!("sources/{}.pdf", p.source))?;
            total += file.size();
            anyhow::ensure!(
                total <= 1024 * 1024 * 1024,
                "作業ファイルの展開容量は1GBまでです"
            );
            let mut b = vec![];
            file.read_to_end(&mut b)?;
            sources.insert(p.source.clone(), b);
        }
    }
    Ok((m.project, sources))
}
pub fn write_file(path: &Path, bytes: &[u8], overwrite: bool, inputs: &[String]) -> Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let resolved = if path.exists() {
        path.canonicalize()?
    } else {
        parent
            .canonicalize()?
            .join(path.file_name().context("保存先が不正です")?)
    };
    for input in inputs {
        if let Ok(p) = Path::new(input).canonicalize() {
            anyhow::ensure!(
                p.to_string_lossy().to_lowercase() != resolved.to_string_lossy().to_lowercase(),
                "元の入力ファイルは上書きできません。別名で保存してください"
            );
        }
    }
    anyhow::ensure!(
        overwrite || !path.exists(),
        "同名ファイルがあります。上書きを確認してください"
    );
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    if overwrite {
        temp.persist(path)?;
    } else {
        temp.persist_noclobber(path)?;
    }
    Ok(())
}
