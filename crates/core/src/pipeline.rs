//! Bounded output search. Every trial starts from the same edited PDF.
use crate::{
    compression::{optimize_lossless, recompress_rgb_images_at_dpi},
    output::{encrypt_pdf, TargetSize},
};
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompressionMode {
    #[default]
    Off,
    Quality,
    Balanced,
    Small,
    Target,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PdfOutputSettings {
    pub mode: CompressionMode,
    pub target_mb: String,
    pub protect: bool,
}
impl Default for PdfOutputSettings {
    fn default() -> Self {
        Self {
            mode: CompressionMode::Off,
            target_mb: "10".into(),
            protect: false,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct OutputReport {
    pub original_bytes: usize,
    pub final_bytes: usize,
    pub target_bytes: Option<u64>,
    pub met_target: Option<bool>,
    pub attempts: usize,
    pub setting: String,
    pub warnings: Vec<String>,
    pub protected: bool,
}
impl OutputReport {
    pub fn authorize_save(&self, acknowledge_unmet: bool) -> Result<()> {
        anyhow::ensure!(
            self.met_target != Some(false) || acknowledge_unmet,
            "目標未達のサイズを確認してから保存してください"
        );
        Ok(())
    }
}
pub struct PreparedPdf {
    pub bytes: Vec<u8>,
    /// Decrypted final file, in memory only, for preview. Never used for saving.
    pub preview: Vec<u8>,
    pub report: OutputReport,
}

pub fn prepare_pdf(
    original: &[u8],
    settings: &PdfOutputSettings,
    password: Option<&str>,
    mut progress: impl FnMut(usize, usize) -> Result<()>,
) -> Result<PreparedPdf> {
    let password = if settings.protect {
        Some(
            password
                .filter(|p| !p.is_empty())
                .ok_or_else(|| anyhow::anyhow!("開くパスワードを入力してください"))?,
        )
    } else {
        None
    };
    let target = if settings.mode == CompressionMode::Target {
        Some(TargetSize::from_mb(&settings.target_mb)?)
    } else {
        None
    };
    // Ordered by quality. Original and lossless have equal visual quality.
    let trials: Vec<Option<(u8, u16)>> = match settings.mode {
        CompressionMode::Off => vec![None],
        CompressionMode::Quality => vec![None, Some((0, 0)), Some((90, 150))],
        CompressionMode::Balanced => vec![None, Some((0, 0)), Some((75, 120))],
        CompressionMode::Small => vec![None, Some((0, 0)), Some((55, 72))],
        CompressionMode::Target => vec![
            None,
            Some((0, 0)),
            Some((90, 150)),
            Some((80, 150)),
            Some((75, 120)),
            Some((70, 96)),
            Some((60, 72)),
            Some((45, 72)),
        ],
    };
    let mut best: Option<(Vec<u8>, String, Vec<String>)> = None;
    let mut observed_warnings = Vec::new();
    let mut attempts = 0;
    for (i, trial) in trials.iter().enumerate() {
        progress(i, trials.len())?;
        let (plain, label, warnings) = match trial {
            None => (original.to_vec(), "画質を保持".into(), vec![]),
            Some((0, _)) => (optimize_lossless(original)?, "可逆最適化".into(), vec![]),
            Some((quality, dpi)) => {
                // RefCell adapts the existing cancellation callback to FnMut progress.
                let callback = std::cell::RefCell::new(&mut progress);
                let result = recompress_rgb_images_at_dpi(original, *quality, Some(*dpi), || {
                    callback.borrow_mut()(i, trials.len())
                })?;
                (
                    result.bytes,
                    format!("最大{dpi} dpi / JPEG品質{quality}"),
                    result.skipped_images,
                )
            }
        };
        let bytes = match password {
            Some(secret) => encrypt_pdf(&plain, secret)?,
            None => plain,
        };
        observed_warnings.extend(warnings.iter().cloned());
        attempts += 1;
        progress(i + 1, trials.len())?;
        let meets = target.is_some_and(|t| bytes.len() as u64 <= t.bytes());
        if meets || best.as_ref().is_none_or(|(b, _, _)| bytes.len() < b.len()) {
            best = Some((bytes, label, warnings));
        }
        // First meeting trial has highest quality; do not degrade it further.
        if meets {
            break;
        }
    }
    let (bytes, setting, mut warnings) = best.expect("at least one trial");
    warnings.extend(observed_warnings);
    warnings.sort();
    warnings.dedup();
    let preview = if let Some(secret) = password {
        // lopdf's passwordless loader only loads the encryption dictionary.
        // Supply authentication during loading so all page objects are read.
        let mut doc = lopdf::Document::load_mem_with_options(
            &bytes,
            lopdf::LoadOptions::with_password(secret),
        )
        .map_err(|_| anyhow::anyhow!("最終PDFの認証に失敗しました"))?;
        let mut plain = Vec::new();
        doc.save_to(&mut plain)?;
        plain
    } else {
        bytes.clone()
    };
    let report = OutputReport {
        original_bytes: original.len(),
        final_bytes: bytes.len(),
        target_bytes: target.map(TargetSize::bytes),
        met_target: target.map(|t| bytes.len() as u64 <= t.bytes()),
        attempts,
        setting,
        warnings,
        protected: settings.protect,
    };
    Ok(PreparedPdf {
        bytes,
        preview,
        report,
    })
}
