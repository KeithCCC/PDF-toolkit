use pdf_toolkit_core::model::*;
fn project() -> Project {
    Project {
        groups: vec![
            Group {
                id: "a".into(),
                name: "one".into(),
                pages: (0..3)
                    .map(|n| Page {
                        id: n.to_string(),
                        source: "source".into(),
                        source_name: "original.pdf".into(),
                        source_path: String::new(),
                        index: n,
                        width: 600.,
                        height: 800.,
                        rotation: 0,
                        comments: vec![],
                    })
                    .collect(),
            },
            Group {
                id: "b".into(),
                name: "two".into(),
                pages: vec![],
            },
        ],
    }
}
#[test]
fn moving_to_end_keeps_relative_order() {
    let mut p = project();
    transfer(&mut p, &["0".into(), "1".into()], "a", 3, false).unwrap();
    assert_eq!(
        p.groups[0]
            .pages
            .iter()
            .map(|p| p.index)
            .collect::<Vec<_>>(),
        vec![2, 0, 1]
    );
}
#[test]
fn copying_has_independent_page_and_comment_identity() {
    let mut p = project();
    p.groups[0].pages[0].comments.push(Comment {
        id: "c".into(),
        text: "メモ".into(),
        x: 10.,
        y: 20.,
        width: 100.,
        height: 50.,
        font_size: 14.,
        color: "#000000".into(),
        background: "#fff6c4".into(),
    });
    transfer(&mut p, &["0".into()], "b", 0, true).unwrap();
    assert_eq!(p.groups[0].pages.len(), 3);
    assert_ne!(p.groups[1].pages[0].id, "0");
    assert_ne!(p.groups[1].pages[0].comments[0].id, "c");
    p.groups[1].pages[0].comments[0].text = "変更".into();
    assert_eq!(p.groups[0].pages[0].comments[0].text, "メモ");
}
#[test]
fn invalid_destination_is_transactional() {
    let mut p = project();
    let original = p.clone();
    assert!(transfer(&mut p, &["0".into()], "missing", 0, false).is_err());
    assert_eq!(p, original);
}
#[test]
fn rotation_normalizes_negative_quarter_turns() {
    let mut p = project();
    rotate(&mut p, &["0".into()], -90).unwrap();
    assert_eq!(p.groups[0].pages[0].rotation, 270);
    assert_eq!(p.groups[0].pages[1].rotation, 0);
}
#[test]
fn file_names_reject_paths_reserved_names_and_trailing_dots() {
    assert!(validate_name("請求書 2026").is_ok());
    for name in ["../escape", "CON", "nul.pdf", "report.", "a/b", "a\\b", ""] {
        assert!(validate_name(name).is_err(), "{name}");
    }
}
