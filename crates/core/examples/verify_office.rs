use pdf_toolkit_core::{convert, office, pdf::Engine};
use serde_json::json;
fn main() -> anyhow::Result<()> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let dir = root.join("output/verification");
    std::fs::create_dir_all(&dir)?;
    let e = Engine::new(root.join("vendor/pdfium/bin/pdfium.dll"))?;
    let sample = e.sample()?;
    std::fs::write(dir.join("sample.pdf"), &sample)?;
    let images = vec![
        (e.render(&sample, 0, 1240, false)?, 595., 842.),
        (e.render(&sample, 1, 875, false)?, 420., 595.),
    ];
    println!(
        "Office detection: {}",
        office::run(json!({"action":"detect"}))?
    );
    for format in ["docx", "xlsx"] {
        let bytes = convert::office_document(format, &images)?;
        let file = dir.join(format!("sample.{format}"));
        std::fs::write(&file, bytes)?;
        if format == "xlsx" {
            println!(
                "Sheets: {}",
                office::run(json!({"action":"sheets","input":file}))?
            );
        }
        let pdf = dir.join(format!("{format}-roundtrip.pdf"));
        office::run(json!({"action":"convert","input":file,"output":pdf,"sheets":[]}))?;
        let bytes = std::fs::read(pdf)?;
        let (_, pages) = e.import(&bytes, None, "s")?;
        println!("{format} roundtrip pages: {}", pages.len());
        anyhow::ensure!(pages.len() == 2, "expected 2 pages, got {}", pages.len());
        for n in 0..2 {
            let pixels = image::load_from_memory(&e.render(&bytes, n, 200, false)?)?.to_rgb8();
            anyhow::ensure!(
                pixels.pixels().any(|p| p[0] < 100 && p[2] > 120),
                "{format} page {n} lost visible content"
            );
            std::fs::write(
                dir.join(format!("{format}-page-{n}.png")),
                e.render(&bytes, n, 850, false)?,
            )?;
        }
    }
    let selected_pdf = dir.join("selected-sheet.pdf");
    office::run(
        json!({"action":"convert","input":dir.join("sample.xlsx"),"output":selected_pdf,"sheets":["Page 2"]}),
    )?;
    let (_, pages) = e.import(&std::fs::read(selected_pdf)?, None, "selected")?;
    anyhow::ensure!(pages.len() == 1, "selected sheet must produce one page");
    let cancelled = std::sync::atomic::AtomicBool::new(false);
    let result = std::thread::scope(|scope| {
        scope.spawn(|| {
            std::thread::sleep(std::time::Duration::from_millis(750));
            cancelled.store(true, std::sync::atomic::Ordering::Relaxed);
        });
        office::run_with_cancel(
            json!({"action":"convert","input":dir.join("sample.docx"),"output":dir.join("cancelled.pdf"),"sheets":[]}),
            &cancelled,
        )
    });
    anyhow::ensure!(result.unwrap_err().to_string().contains("キャンセル"));
    println!("Selected Excel sheet and Office cancellation: verified");
    Ok(())
}
