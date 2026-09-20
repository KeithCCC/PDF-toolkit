use anyhow::{Context, Result};
use serde_json::Value;
use std::{
    io::Write,
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
fn hidden(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
}
pub fn run(config: Value) -> Result<String> {
    run_with_cancel(config, &AtomicBool::new(false))
}
pub fn run_with_cancel(mut config: Value, cancel: &AtomicBool) -> Result<String> {
    anyhow::ensure!(!cancel.load(Ordering::Relaxed), "処理をキャンセルしました");
    for key in ["input", "output"] {
        if let Some(path) = config[key].as_str() {
            config[key] = Value::String(if let Some(tail) = path.strip_prefix(r"\\?\UNC\") {
                format!(r"\\{tail}")
            } else {
                path.strip_prefix(r"\\?\").unwrap_or(path).to_owned()
            });
        }
    }
    let dir = tempfile::tempdir()?;
    let script = dir.path().join("office.ps1");
    let path = dir.path().join("config.json");
    let pid_file = dir.path().join("owned-process.json");
    config["pidFile"] = serde_json::json!(pid_file);
    let mut f = std::fs::File::create(&script)?;
    f.write_all(&[0xEF, 0xBB, 0xBF])?;
    f.write_all(include_bytes!("../../../scripts/office.ps1"))?;
    drop(f);
    std::fs::write(&path, serde_json::to_vec(&config)?)?;
    let stdout = dir.path().join("stdout.txt");
    let stderr = dir.path().join("stderr.txt");
    let mut cmd = Command::new("powershell.exe");
    hidden(&mut cmd);
    cmd.args([
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
    ])
    .arg(&script)
    .arg("-ConfigPath")
    .arg(&path)
    .stdout(Stdio::from(std::fs::File::create(&stdout)?))
    .stderr(Stdio::from(std::fs::File::create(&stderr)?));
    let mut child = cmd.spawn().context("Office連携プロセスを起動できません")?;
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if (cancel.load(Ordering::Relaxed)
            && (pid_file.exists() || start.elapsed() > Duration::from_secs(10)))
            || start.elapsed() > Duration::from_secs(180)
        {
            // Only terminate a process whose identity and creation time were registered by our helper.
            if pid_file.exists() {
                let stop = dir.path().join("stop.ps1");
                std::fs::write(&stop,br#"param([string]$Identity)
$x=Get-Content -LiteralPath $Identity -Raw | ConvertFrom-Json
$p=Get-Process -Id $x.id -ErrorAction SilentlyContinue
if($p -and $p.ProcessName -eq $x.name -and $p.StartTime.ToUniversalTime().Ticks -eq $x.ticks){Stop-Process -Id $p.Id -Force}
"#)?;
                let mut cleanup = Command::new("powershell.exe");
                hidden(&mut cleanup);
                let _ = cleanup
                    .args([
                        "-NoProfile",
                        "-NonInteractive",
                        "-ExecutionPolicy",
                        "Bypass",
                        "-File",
                    ])
                    .arg(stop)
                    .arg(&pid_file)
                    .output();
            }
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!(if cancel.load(Ordering::Relaxed) {
                "処理をキャンセルしました"
            } else {
                "Officeの処理が180秒以内に終了しませんでした"
            });
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let out = std::fs::read(stdout)?;
    let err = std::fs::read(stderr)?;
    anyhow::ensure!(
        status.success(),
        "Office変換に失敗しました: {}",
        String::from_utf8_lossy(&err)
    );
    anyhow::ensure!(!cancel.load(Ordering::Relaxed), "処理をキャンセルしました");
    Ok(String::from_utf8_lossy(&out)
        .trim()
        .trim_start_matches('\u{feff}')
        .to_string())
}
