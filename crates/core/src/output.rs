//! Final output candidates. Passwords deliberately are not part of persistent settings.
use anyhow::{Context, Result};
use lopdf::{
    encryption::crypt_filters::{Aes256CryptFilter, CryptFilter},
    Document, EncryptionState, EncryptionVersion, Permissions,
};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TargetSize(u64);

impl TargetSize {
    /// Decimal MB, parsed without floating point or rounding at the size boundary.
    pub fn from_mb(value: &str) -> Result<Self> {
        let parts: Vec<_> = value.trim().split('.').collect();
        anyhow::ensure!((1..=2).contains(&parts.len()), "目標サイズの書式が不正です");
        anyhow::ensure!(
            parts
                .iter()
                .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit())),
            "目標サイズには正の数値を入力してください"
        );
        let integer: u64 = parts[0].parse().context("目標サイズが大きすぎます")?;
        let fraction = if parts.len() == 2 {
            anyhow::ensure!(parts[1].len() <= 6, "目標サイズは小数点以下6桁までです");
            parts[1].parse::<u64>()? * 10u64.pow(6 - parts[1].len() as u32)
        } else {
            0
        };
        let bytes = integer
            .checked_mul(1_000_000)
            .and_then(|n| n.checked_add(fraction))
            .context("目標サイズが大きすぎます")?;
        anyhow::ensure!(bytes > 0, "目標サイズは0より大きい値にしてください");
        Ok(Self(bytes))
    }

    pub fn bytes(self) -> u64 {
        self.0
    }
}

pub struct Candidate {
    /// Exactly the bytes to persist, including encryption and all decoration overhead.
    pub bytes: Vec<u8>,
    /// Higher is better. A planner assigns this before evaluating file size.
    pub quality_rank: u16,
}

pub struct Selection {
    pub candidate: Candidate,
    /// None when no size target was requested, false is an explicitly unmet target.
    pub met_target: Option<bool>,
}

pub fn choose_candidate(
    mut candidates: Vec<Candidate>,
    target: Option<TargetSize>,
) -> Result<Selection> {
    anyhow::ensure!(!candidates.is_empty(), "有効な出力候補がありません");
    let meets =
        |candidate: &Candidate| target.is_none_or(|t| candidate.bytes.len() as u64 <= t.bytes());
    let best = candidates
        .iter()
        .enumerate()
        .filter(|(_, c)| meets(c))
        .max_by(|(_, a), (_, b)| {
            a.quality_rank
                .cmp(&b.quality_rank)
                .then_with(|| b.bytes.len().cmp(&a.bytes.len()))
        })
        .map(|(i, _)| i);
    let index = best.unwrap_or_else(|| {
        candidates
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                a.bytes
                    .len()
                    .cmp(&b.bytes.len())
                    .then_with(|| b.quality_rank.cmp(&a.quality_rank))
            })
            .map(|(i, _)| i)
            .expect("nonempty candidates")
    });
    Ok(Selection {
        candidate: candidates.swap_remove(index),
        met_target: target.map(|_| best.is_some()),
    })
}

/// AES-256, Standard Security Handler revision 6. No password is serialized or logged.
/// The caller owns the plaintext input and the short-lived password.
pub fn encrypt_pdf(bytes: &[u8], password: &str) -> Result<Vec<u8>> {
    anyhow::ensure!(
        !password.is_empty() && password.len() <= 127 && !password.chars().any(char::is_control),
        "パスワードは制御文字を除く1～127 UTF-8 bytesで入力してください"
    );
    let mut document = Document::load_mem(bytes).context("暗号化するPDFを読み込めません")?;
    anyhow::ensure!(
        !document.is_encrypted(),
        "先に入力PDFのパスワードを解除してください"
    );
    let mut file_key = [0u8; 32];
    let mut owner_random = [0u8; 32];
    getrandom::fill(&mut file_key).map_err(|_| anyhow::anyhow!("暗号鍵の乱数を生成できません"))?;
    getrandom::fill(&mut owner_random)
        .map_err(|_| anyhow::anyhow!("所有者鍵の乱数を生成できません"))?;
    let owner: String = owner_random.iter().map(|b| format!("{b:02x}")).collect();
    let filter: Arc<dyn CryptFilter> = Arc::new(Aes256CryptFilter);
    let state = EncryptionState::try_from(EncryptionVersion::V5 {
        encrypt_metadata: true,
        crypt_filters: BTreeMap::from([(b"StdCF".to_vec(), filter)]),
        file_encryption_key: &file_key,
        stream_filter: b"StdCF".to_vec(),
        string_filter: b"StdCF".to_vec(),
        owner_password: &owner,
        user_password: password,
        permissions: Permissions::all(),
    })
    .map_err(|_| anyhow::anyhow!("PDFの暗号化設定を作成できません"))?;
    document.version = "2.0".into();
    document.encrypt(&state).context("PDFを暗号化できません")?;
    // R6 normalizes Unicode passwords. Nonempty input can normalize to empty.
    anyhow::ensure!(
        document.authenticate_password("").is_err(),
        "空として扱われるパスワードは使用できません"
    );
    let mut output = Vec::new();
    document.save_to(&mut output)?;
    Ok(output)
}
