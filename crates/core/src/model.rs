use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Comment {
    pub id: String,
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub font_size: f32,
    pub color: String,
    pub background: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Page {
    pub id: String,
    pub source: String,
    #[serde(default)]
    pub source_name: String,
    #[serde(default)]
    pub source_path: String,
    pub index: u16,
    pub width: f32,
    pub height: f32,
    pub rotation: i32,
    pub comments: Vec<Comment>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub pages: Vec<Page>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Project {
    pub groups: Vec<Group>,
}

pub fn id() -> String {
    uuid::Uuid::new_v4().to_string()
}
pub fn transfer(
    project: &mut Project,
    ids: &[String],
    target: &str,
    at: usize,
    copy: bool,
) -> anyhow::Result<()> {
    let dest = project
        .groups
        .iter()
        .position(|g| g.id == target)
        .ok_or_else(|| anyhow::anyhow!("移動先がありません"))?;
    anyhow::ensure!(at <= project.groups[dest].pages.len(), "挿入位置が無効です");
    let mut selected: Vec<Page> = project
        .groups
        .iter()
        .flat_map(|g| &g.pages)
        .filter(|p| ids.contains(&p.id))
        .cloned()
        .collect();
    anyhow::ensure!(!selected.is_empty(), "ページを選択してください");
    let before = if copy {
        0
    } else {
        project.groups[dest].pages[..at]
            .iter()
            .filter(|p| ids.contains(&p.id))
            .count()
    };
    if copy {
        for p in &mut selected {
            p.id = id();
            for c in &mut p.comments {
                c.id = id();
            }
        }
    } else {
        for g in &mut project.groups {
            g.pages.retain(|p| !ids.contains(&p.id));
        }
    }
    project.groups[dest]
        .pages
        .splice(at - before..at - before, selected);
    Ok(())
}
pub fn rotate(project: &mut Project, ids: &[String], delta: i32) -> anyhow::Result<()> {
    anyhow::ensure!(delta == 90 || delta == -90, "回転は90度単位です");
    for p in project
        .groups
        .iter_mut()
        .flat_map(|g| &mut g.pages)
        .filter(|p| ids.contains(&p.id))
    {
        p.rotation = (p.rotation + delta).rem_euclid(360);
    }
    Ok(())
}
pub fn validate_name(name: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        !name.trim().is_empty()
            && name.len() <= 180
            && !name.ends_with(['.', ' '])
            && !name
                .chars()
                .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c)),
        "ファイル名に使用できない文字があります"
    );
    let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    anyhow::ensure!(
        ![
            "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
            "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9"
        ]
        .contains(&stem.as_str()),
        "Windowsの予約名は使えません"
    );
    Ok(())
}
