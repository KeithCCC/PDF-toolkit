use pdf_toolkit_core::{
    output::{choose_candidate, encrypt_pdf, Candidate, TargetSize},
    pdf::Engine,
};

fn engine() -> Engine {
    Engine::new(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/pdfium/bin/pdfium.dll"),
    )
    .unwrap()
}

#[test]
fn target_uses_decimal_megabytes_without_float_rounding() {
    assert_eq!(TargetSize::from_mb("10").unwrap().bytes(), 10_000_000);
    assert_eq!(TargetSize::from_mb(" 0.000001 ").unwrap().bytes(), 1);
    assert_eq!(TargetSize::from_mb("1.234567").unwrap().bytes(), 1_234_567);
}

#[test]
fn invalid_or_unrepresentable_targets_are_rejected() {
    for text in [
        "",
        "0",
        "-1",
        "NaN",
        "inf",
        "1e3",
        ".1",
        "1.",
        "0.0000001",
        "9999999999999999999999999",
        "1,5",
    ] {
        assert!(TargetSize::from_mb(text).is_err(), "accepted {text}");
    }
}

fn candidate(size: usize, quality_rank: u16) -> Candidate {
    Candidate {
        bytes: vec![42; size],
        quality_rank,
    }
}

#[test]
fn meeting_candidates_prefer_quality_and_keep_the_exact_bytes() {
    let mut high_quality = candidate(999, 90);
    high_quality.bytes[0] = 17;
    let result = choose_candidate(
        vec![candidate(400, 20), high_quality, candidate(1001, 100)],
        Some(TargetSize::from_mb("0.001").unwrap()),
    )
    .unwrap();
    assert_eq!(result.met_target, Some(true));
    assert_eq!(result.candidate.bytes.len(), 999);
    assert_eq!(result.candidate.bytes[0], 17);
}

#[test]
fn unattainable_target_returns_smallest_actual_candidate_and_marks_failure() {
    let result = choose_candidate(
        vec![
            candidate(1400, 80),
            candidate(1200, 70),
            candidate(1300, 60),
        ],
        Some(TargetSize::from_mb("0.001").unwrap()),
    )
    .unwrap();
    assert_eq!(result.met_target, Some(false));
    assert_eq!(result.candidate.bytes.len(), 1200);
}

#[test]
fn inclusive_boundary_and_missing_candidates_are_handled() {
    assert_eq!(
        choose_candidate(
            vec![candidate(1000, 100)],
            Some(TargetSize::from_mb("0.001").unwrap())
        )
        .unwrap()
        .met_target,
        Some(true)
    );
    assert!(choose_candidate(vec![], None).is_err());
    assert_eq!(
        choose_candidate(vec![candidate(1000, 100)], None)
            .unwrap()
            .met_target,
        None
    );
}

#[test]
fn aes256_requires_the_password_and_preserves_pages() {
    let engine = engine();
    let source = engine.sample().unwrap();
    let password = "日本語-password-確認";
    let encrypted = encrypt_pdf(&source, password).unwrap();
    assert!(engine.import(&encrypted, None, "s").is_err());
    assert!(engine.import(&encrypted, Some("wrong"), "s").is_err());
    assert_eq!(
        engine
            .import(&encrypted, Some(password), "s")
            .unwrap()
            .1
            .len(),
        2
    );
    let document = lopdf::Document::load_mem(&encrypted).unwrap();
    let encryption_id = document
        .trailer
        .get(b"Encrypt")
        .unwrap()
        .as_reference()
        .unwrap();
    let dict = document
        .get_object(encryption_id)
        .unwrap()
        .as_dict()
        .unwrap();
    assert_eq!(dict.get(b"V").unwrap().as_i64().unwrap(), 5);
    assert_eq!(dict.get(b"R").unwrap().as_i64().unwrap(), 6);
    assert_eq!(
        dict.get(b"CF")
            .unwrap()
            .as_dict()
            .unwrap()
            .get(b"StdCF")
            .unwrap()
            .as_dict()
            .unwrap()
            .get(b"CFM")
            .unwrap()
            .as_name()
            .unwrap(),
        b"AESV3"
    );
    let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../output/verification/additional-encrypted.pdf");
    std::fs::create_dir_all(output.parent().unwrap()).unwrap();
    std::fs::write(output, &encrypted).unwrap();
    assert_ne!(
        encrypted,
        encrypt_pdf(&source, password).unwrap(),
        "Fresh entropy for every export"
    );
}

#[test]
fn encryption_overhead_is_included_in_target_judgment() {
    let source = engine().sample().unwrap();
    let encrypted = encrypt_pdf(&source, "test-password").unwrap();
    assert!(encrypted.len() > source.len());
    let target = TargetSize::from_mb(&format!("0.{:06}", source.len())).unwrap();
    let selection = choose_candidate(
        vec![Candidate {
            bytes: encrypted,
            quality_rank: 100,
        }],
        Some(target),
    )
    .unwrap();
    assert_eq!(selection.met_target, Some(false));
}

#[test]
fn encryption_rejects_empty_and_overlong_passwords_without_echoing_secrets() {
    let source = engine().sample().unwrap();
    assert!(encrypt_pdf(&source, "").is_err());
    let secret = "secret".repeat(40);
    let error = encrypt_pdf(&source, &secret).unwrap_err().to_string();
    assert!(!error.contains(&secret));
    assert!(encrypt_pdf(b"not a PDF", "secret").is_err());
}

#[test]
fn encryption_rejects_passwords_that_normalize_to_empty() {
    let source = engine().sample().unwrap();
    assert!(encrypt_pdf(&source, "\u{00ad}").is_err());
}

#[test]
fn final_output_verification_checks_every_page_and_can_be_cancelled() {
    let e = engine();
    let source = e.sample().unwrap();
    let encrypted = encrypt_pdf(&source, "verify-test").unwrap();
    let checked = std::cell::Cell::new(0);
    assert!(e
        .verify_output(&encrypted, Some("verify-test"), 2, |n, _| {
            checked.set(checked.get() + 1);
            anyhow::ensure!(n < 1, "cancelled");
            Ok(())
        })
        .is_err());
    assert_eq!(checked.get(), 2);
    assert!(e
        .verify_output(&encrypted, Some("wrong"), 2, |_, _| Ok(()))
        .is_err());
    assert!(e
        .verify_output(&encrypted, Some("verify-test"), 3, |_, _| Ok(()))
        .is_err());
    e.verify_output(&encrypted, Some("verify-test"), 2, |_, _| Ok(()))
        .unwrap();
}

#[test]
fn concurrent_password_failures_do_not_break_other_pdf_operations() {
    let source = std::sync::Arc::new(engine().sample().unwrap());
    let encrypted = std::sync::Arc::new(encrypt_pdf(&source, "concurrency-test").unwrap());
    let threads: Vec<_> = (0..12)
        .map(|_| {
            let source = source.clone();
            let encrypted = encrypted.clone();
            std::thread::spawn(move || {
                let e = engine();
                for _ in 0..20 {
                    assert!(e.import(&encrypted, Some("wrong"), "wrong").is_err());
                    let (bytes, pages) = e.import(&source, None, "source").unwrap();
                    let result = e
                        .export(
                            &pages,
                            &std::collections::HashMap::from([("source".into(), bytes)]),
                        )
                        .unwrap();
                    assert!(!e.render(&result, 0, 64, false).unwrap().is_empty());
                }
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
}
