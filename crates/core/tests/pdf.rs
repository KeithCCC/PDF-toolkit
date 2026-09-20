use pdf_toolkit_core::{model::*, pdf::Engine};
use std::collections::HashMap;
fn engine() -> Engine {
    Engine::new(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/pdfium/bin/pdfium.dll"),
    )
    .unwrap()
}
#[test]
fn pdf_roundtrip_preserves_order_rotation_comments_and_source() {
    let engine = engine();
    let original = engine.sample().unwrap();
    let (source, mut pages) = engine.import(&original, None, "s").unwrap();
    assert_eq!(pages.len(), 2);
    let mut sources = HashMap::new();
    sources.insert("s".into(), source);
    pages.swap(0, 1);
    pages[0].rotation = 90;
    pages[0].comments.push(Comment {
        id: "comment".into(),
        text: "日本語のコメント\n確認済み".into(),
        x: 30.,
        y: 40.,
        width: 180.,
        height: 70.,
        font_size: 14.,
        color: "#202020".into(),
        background: "#fff6c4".into(),
    });
    let out = engine.export(&pages, &sources).unwrap();
    let (_, reopened) = engine.import(&out, None, "again").unwrap();
    assert_eq!(reopened.len(), 2);
    assert_eq!(reopened[0].rotation, 90);
    assert_eq!(reopened[0].comments[0].text, "日本語のコメント\n確認済み");
    assert_eq!(reopened[0].width, 420.);
    assert_eq!(reopened[1].width, 595.);
    let png = engine.render(&out, 0, 900, false).unwrap();
    assert!(png.len() > 1000);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../output/verification");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("comments.pdf"), out).unwrap();
    std::fs::write(root.join("comments.png"), png).unwrap();
    assert_eq!(
        engine.import(&original, None, "untouched").unwrap().1[0]
            .comments
            .len(),
        0
    );
}
#[test]
fn corrupt_pdf_returns_error() {
    assert!(engine().import(b"broken", None, "s").is_err());
}

#[test]
fn export_checks_cancellation_between_pages() {
    let e = engine();
    let sample = e.sample().unwrap();
    let (source, pages) = e.import(&sample, None, "s").unwrap();
    let sources = HashMap::from([("s".into(), source)]);
    let result = e.export_checked(&pages, &sources, |index, _| {
        if index >= 1 {
            anyhow::bail!("cancelled")
        }
        Ok(())
    });
    assert!(result.unwrap_err().to_string().contains("cancelled"));
}

#[test]
fn cropped_rotated_pages_keep_existing_annotations_and_new_comments() {
    let e = engine();
    let input = include_bytes!("fixtures/cropped-rotated.pdf");
    let (source, mut pages) = e.import(input, None, "s").unwrap();
    assert_eq!(
        (pages[0].width, pages[0].height, pages[0].rotation),
        (500., 700., 90)
    );
    pages[0].comments.push(Comment {
        id: "c".into(),
        text: "Crop comment".into(),
        x: 30.,
        y: 40.,
        width: 180.,
        height: 70.,
        font_size: 14.,
        color: "#202020".into(),
        background: "#fff6c4".into(),
    });
    let output = e
        .export(&pages, &HashMap::from([("s".into(), source)]))
        .unwrap();
    let doc = lopdf::Document::load_mem(&output).unwrap();
    let pid = *doc.get_pages().values().next().unwrap();
    let anns = doc
        .get_object(pid)
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"Annots")
        .unwrap()
        .as_array()
        .unwrap();
    assert!(anns.len() >= 2);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../output/verification");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("cropped-comments.pdf"), &output).unwrap();
    std::fs::write(
        root.join("cropped-comments.png"),
        e.render(&output, 0, 900, false).unwrap(),
    )
    .unwrap();
    let (_, reopened) = e.import(&output, None, "r").unwrap();
    assert_eq!(reopened[0].comments[0].text, "Crop comment");
}
#[test]
fn password_is_required_and_unlocked_source_can_be_reopened() {
    let e = engine();
    let input = include_bytes!("fixtures/password.pdf");
    assert!(e.import(input, None, "s").is_err());
    assert!(e.import(input, Some("wrong"), "s").is_err());
    let (source, pages) = e.import(input, Some("open-test"), "s").unwrap();
    assert_eq!(pages.len(), 1);
    assert!(e.import(&source, None, "r").is_ok());
}
