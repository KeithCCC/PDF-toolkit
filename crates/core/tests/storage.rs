use pdf_toolkit_core::{model::*, storage};
use std::collections::HashMap;
#[test]
fn project_contains_input_bytes_and_roundtrips() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("work.pdftk");
    let p = Project {
        groups: vec![Group {
            id: "g".into(),
            name: "文書".into(),
            pages: vec![Page {
                id: "p".into(),
                source: "s".into(),
                source_name: "original.pdf".into(),
                source_path: String::new(),
                index: 0,
                width: 10.,
                height: 20.,
                rotation: 90,
                comments: vec![],
            }],
        }],
    };
    let s = HashMap::from([("s".into(), b"stored source".to_vec())]);
    storage::save(&path, &p, &s).unwrap();
    let (loaded, bytes) = storage::load(&path).unwrap();
    assert_eq!(loaded, p);
    assert_eq!(bytes["s"], b"stored source");
}
#[test]
fn writes_protect_existing_files_and_input_paths() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("input.pdf");
    std::fs::write(&path, b"original").unwrap();
    assert!(storage::write_file(&path, b"changed", false, &[]).is_err());
    assert!(
        storage::write_file(&path, b"changed", true, &[path.to_string_lossy().into()]).is_err()
    );
    assert_eq!(std::fs::read(&path).unwrap(), b"original");
    let output = dir.path().join("output.pdf");
    storage::write_file(&output, b"exported", false, &[]).unwrap();
    assert_eq!(std::fs::read(output).unwrap(), b"exported");
}

#[test]
fn cancelled_save_preserves_previous_project_and_corrupt_input_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("work.pdftk");
    storage::save(&path, &Project::default(), &HashMap::new()).unwrap();
    let original = std::fs::read(&path).unwrap();
    let count = std::cell::Cell::new(0);
    assert!(
        storage::save_checked(&path, &Project::default(), &HashMap::new(), || {
            count.set(count.get() + 1);
            anyhow::ensure!(count.get() < 2, "cancelled");
            Ok(())
        })
        .is_err()
    );
    assert_eq!(std::fs::read(&path).unwrap(), original);
    std::fs::write(&path, b"broken archive").unwrap();
    assert!(storage::load(&path).is_err());
}
