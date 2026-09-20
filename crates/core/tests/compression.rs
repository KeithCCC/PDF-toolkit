use lopdf::{dictionary, Document, Object, Stream};
use pdf_toolkit_core::{
    compression::{optimize_lossless, recompress_rgb_images},
    model::Comment,
    pdf::Engine,
};
use std::collections::HashMap;

fn engine() -> Engine {
    Engine::new(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/pdfium/bin/pdfium.dll"),
    )
    .unwrap()
}

fn photo_document(masked: bool) -> Vec<u8> {
    let e = engine();
    let sample = e.sample().unwrap();
    let (source, mut pages) = e.import(&sample, None, "s").unwrap();
    pages[0].comments.push(Comment {
        id: "note".into(),
        text: "圧縮してもコメントは編集可能".into(),
        x: 30.,
        y: 120.,
        width: 220.,
        height: 70.,
        font_size: 14.,
        color: "#202020".into(),
        background: "#fff6c4".into(),
    });
    let annotated = e
        .export(&pages, &HashMap::from([("s".into(), source)]))
        .unwrap();
    let mut doc = Document::load_mem(&annotated).unwrap();
    let mut pixels = Vec::with_capacity(1024 * 768 * 3);
    let mut seed = 123456789u32;
    for y in 0..768 {
        for x in 0..1024 {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            pixels.extend([
                ((x + y) % 256) as u8,
                ((x / 4) % 256) as u8,
                (seed >> 24) as u8,
            ]);
        }
    }
    let mut image = Stream::new(
        dictionary! {"Type"=>"XObject","Subtype"=>"Image","Width"=>1024,"Height"=>768,"ColorSpace"=>"DeviceRGB","BitsPerComponent"=>8},
        pixels,
    );
    if masked {
        image.dict.set(
            "Mask",
            vec![0.into(), 0.into(), 0.into(), 0.into(), 0.into(), 0.into()],
        );
    }
    let image_id = doc.add_object(image);
    let content = doc.add_object(Stream::new(
        dictionary! {},
        b"q 400 0 0 300 30 300 cm /Photo Do Q".to_vec(),
    ));
    let page_id = *doc.get_pages().values().next().unwrap();
    let mut resources = doc
        .get_object(page_id)
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"Resources")
        .unwrap()
        .as_dict()
        .unwrap()
        .clone();
    resources.set("XObject", dictionary! {"Photo"=>image_id});
    let page = doc.get_object_mut(page_id).unwrap().as_dict_mut().unwrap();
    let previous = page.get(b"Contents").unwrap().clone();
    page.set("Contents", vec![previous, Object::Reference(content)]);
    page.set("Resources", resources);
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    bytes
}

#[test]
fn lossless_optimization_keeps_text_pages_and_annotations() {
    let bytes = photo_document(false);
    let result = optimize_lossless(&bytes).unwrap();
    let before = Document::load_mem(&bytes).unwrap();
    let after = Document::load_mem(&result).unwrap();
    assert_eq!(
        before.extract_text(&[1, 2]).unwrap(),
        after.extract_text(&[1, 2]).unwrap()
    );
    assert_eq!(
        engine().import(&result, None, "s").unwrap().1[0]
            .comments
            .len(),
        1
    );
    assert!(result.len() < bytes.len());
}

#[test]
fn jpeg_recompression_reduces_photo_without_flattening_text_or_annotations() {
    let bytes = photo_document(false);
    let result = recompress_rgb_images(&bytes, 70, || Ok(())).unwrap();
    assert_eq!(
        result.changed_images, 1,
        "annotation appearance must not be recompressed"
    );
    assert!(result.bytes.len() < bytes.len() / 2);
    assert_eq!(
        Document::load_mem(&result.bytes)
            .unwrap()
            .extract_text(&[1, 2])
            .unwrap(),
        Document::load_mem(&bytes)
            .unwrap()
            .extract_text(&[1, 2])
            .unwrap()
    );
    let e = engine();
    let (_, pages) = e.import(&result.bytes, None, "s").unwrap();
    assert_eq!(pages.len(), 2);
    assert_eq!(pages[0].comments[0].text, "圧縮してもコメントは編集可能");
    assert!(!e.render(&result.bytes, 0, 800, false).unwrap().is_empty());
}

#[test]
fn masked_images_are_preserved_with_an_explicit_skip_reason() {
    let bytes = photo_document(true);
    let result = recompress_rgb_images(&bytes, 50, || Ok(())).unwrap();
    assert_eq!(result.changed_images, 0);
    assert!(result
        .skipped_images
        .iter()
        .any(|reason| reason.contains("Mask")));
    let doc = Document::load_mem(&result.bytes).unwrap();
    assert!(doc
        .objects
        .values()
        .filter_map(|o| o.as_stream().ok())
        .any(|s| s.dict.has(b"Mask")));
}

#[test]
fn recompression_can_be_cancelled_without_modifying_the_input() {
    let bytes = photo_document(false);
    let before = bytes.clone();
    let error = recompress_rgb_images(&bytes, 70, || anyhow::bail!("cancelled")).unwrap_err();
    assert!(error.to_string().contains("cancelled"));
    assert_eq!(bytes, before);
    assert!(recompress_rgb_images(&bytes, 0, || Ok(())).is_err());
}

#[test]
fn final_encrypted_compressed_candidate_meets_the_requested_size() {
    use pdf_toolkit_core::output::{choose_candidate, encrypt_pdf, Candidate, TargetSize};
    let source = photo_document(false);
    assert!(source.len() > 1_000_000);
    let compressed = recompress_rgb_images(&source, 75, || Ok(())).unwrap();
    let protected = encrypt_pdf(&compressed.bytes, "test-output-password").unwrap();
    let result = choose_candidate(
        vec![Candidate {
            bytes: protected,
            quality_rank: 75,
        }],
        Some(TargetSize::from_mb("1").unwrap()),
    )
    .unwrap();
    assert_eq!(result.met_target, Some(true));
    assert!(result.candidate.bytes.len() <= 1_000_000);
    let e = engine();
    let (_, pages) = e
        .import(&result.candidate.bytes, Some("test-output-password"), "s")
        .unwrap();
    assert_eq!(pages.len(), 2);
    assert_eq!(pages[0].comments.len(), 1);
    let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../output/verification");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("additional-original.pdf"), &source).unwrap();
    std::fs::write(
        folder.join("additional-compressed-encrypted.pdf"),
        &result.candidate.bytes,
    )
    .unwrap();
    println!(
        "original={} final={} target=1000000",
        source.len(),
        result.candidate.bytes.len()
    );
}

fn photo_dimensions(bytes: &[u8]) -> (i64, i64) {
    let doc = Document::load_mem(bytes).unwrap();
    let page = *doc.get_pages().values().next().unwrap();
    let resources = doc
        .get_object(page)
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"Resources")
        .unwrap()
        .as_dict()
        .unwrap();
    let image_id = resources
        .get(b"XObject")
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"Photo")
        .unwrap()
        .as_reference()
        .unwrap();
    let image = doc.get_object(image_id).unwrap().as_stream().unwrap();
    (
        image.dict.get(b"Width").unwrap().as_i64().unwrap(),
        image.dict.get(b"Height").unwrap().as_i64().unwrap(),
    )
}

#[test]
fn dpi_resize_respects_placement_and_never_upscales() {
    use pdf_toolkit_core::compression::recompress_rgb_images_at_dpi;
    let source = photo_document(false);
    let resized = recompress_rgb_images_at_dpi(&source, 75, Some(72), || Ok(())).unwrap();
    assert_eq!(photo_dimensions(&resized.bytes), (400, 300));
    let high = recompress_rgb_images_at_dpi(&source, 75, Some(600), || Ok(())).unwrap();
    assert_eq!(photo_dimensions(&high.bytes), (1024, 768));
    assert_eq!(
        engine().import(&resized.bytes, None, "s").unwrap().1[0]
            .comments
            .len(),
        1
    );
}

#[test]
fn dpi_resize_uses_largest_shared_placement_and_user_unit() {
    use pdf_toolkit_core::compression::recompress_rgb_images_at_dpi;
    let source = photo_document(false);
    let mut doc = Document::load_mem(&source).unwrap();
    let pages: Vec<_> = doc.get_pages().values().copied().collect();
    let resources = doc
        .get_object(pages[0])
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"Resources")
        .unwrap()
        .clone();
    let content = doc.add_object(Stream::new(
        dictionary! {},
        b"q 300 0 0 200 0 0 cm /Photo Do Q".to_vec(),
    ));
    let page = doc.get_object_mut(pages[1]).unwrap().as_dict_mut().unwrap();
    page.set("Resources", resources);
    page.set("Contents", content);
    page.set("UserUnit", 2);
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    let result = recompress_rgb_images_at_dpi(&bytes, 75, Some(72), || Ok(())).unwrap();
    assert_eq!(photo_dimensions(&result.bytes), (600, 400));
}

#[test]
fn nested_form_matrix_is_used_for_dpi() {
    use pdf_toolkit_core::compression::recompress_rgb_images_at_dpi;
    let mut doc = Document::load_mem(&photo_document(false)).unwrap();
    let page_id = *doc.get_pages().values().next().unwrap();
    let resources = doc
        .get_object(page_id)
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"Resources")
        .unwrap()
        .clone();
    let form_id = doc.add_object(Stream::new(dictionary! {"Type"=>"XObject", "Subtype"=>"Form", "BBox"=>vec![0.into(),0.into(),400.into(),300.into()], "Matrix"=>vec![0.5.into(),0.into(),0.into(),0.5.into(),0.into(),0.into()], "Resources"=>resources}, b"q 400 0 0 300 0 0 cm /Photo Do Q".to_vec()));
    let content = doc.add_object(Stream::new(
        dictionary! {},
        b"q 0 2 -2 0 600 0 cm /Form Do Q".to_vec(),
    ));
    let page = doc.get_object_mut(page_id).unwrap().as_dict_mut().unwrap();
    page.set("Contents", content);
    page.get_mut(b"Resources")
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .get_mut(b"XObject")
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set("Form", form_id);
    let mut source = Vec::new();
    doc.save_to(&mut source).unwrap();
    let result = recompress_rgb_images_at_dpi(&source, 75, Some(72), || Ok(())).unwrap();
    assert_eq!(photo_dimensions(&result.bytes), (400, 300));
}

#[test]
fn compression_pipeline_measures_final_encrypted_bytes_and_bounds_attempts() {
    use pdf_toolkit_core::pipeline::{prepare_pdf, CompressionMode, PdfOutputSettings};
    let source = photo_document(false);
    let settings = PdfOutputSettings {
        mode: CompressionMode::Target,
        target_mb: "0.1".into(),
        protect: true,
    };
    let mut attempts = Vec::new();
    let output = prepare_pdf(&source, &settings, Some("test-only-password"), |n, _| {
        attempts.push(n);
        Ok(())
    })
    .unwrap();
    assert!(output.report.attempts <= 8);
    assert_eq!(output.report.final_bytes, output.bytes.len());
    std::fs::write(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../output/verification/decrypted-preview.pdf"),
        &output.preview,
    )
    .unwrap();
    assert!(!engine()
        .render(&output.preview, 0, 700, false)
        .unwrap()
        .is_empty());
    assert_eq!(
        output.report.met_target,
        Some(output.bytes.len() <= 100_000)
    );
    assert!(engine().import(&output.bytes, None, "s").is_err());
    assert_eq!(
        engine()
            .import(&output.bytes, Some("test-only-password"), "s")
            .unwrap()
            .1
            .len(),
        2
    );
    assert!(prepare_pdf(&source, &settings, None, |_, _| Ok(())).is_err());
    assert!(
        prepare_pdf(&source, &settings, Some("test"), |_, _| anyhow::bail!(
            "cancelled"
        ))
        .is_err()
    );
}

#[test]
fn compression_off_preserves_bytes_and_unmet_target_requires_acknowledgment() {
    use pdf_toolkit_core::pipeline::{prepare_pdf, CompressionMode, PdfOutputSettings};
    let source = engine().sample().unwrap();
    let off = prepare_pdf(&source, &PdfOutputSettings::default(), None, |_, _| Ok(())).unwrap();
    assert_eq!(off.bytes, source);
    let settings = PdfOutputSettings {
        mode: CompressionMode::Target,
        target_mb: "0.000001".into(),
        protect: false,
    };
    let output = prepare_pdf(&source, &settings, None, |_, _| Ok(())).unwrap();
    assert_eq!(output.report.met_target, Some(false));
    assert!(output.report.authorize_save(false).is_err());
    assert!(output.report.authorize_save(true).is_ok());
    assert_eq!(
        Document::load_mem(&output.bytes)
            .unwrap()
            .extract_text(&[1, 2])
            .unwrap(),
        Document::load_mem(&source)
            .unwrap()
            .extract_text(&[1, 2])
            .unwrap()
    );
}

#[test]
fn skipped_image_reason_survives_selection_of_lossless_candidate() {
    use pdf_toolkit_core::pipeline::{prepare_pdf, CompressionMode, PdfOutputSettings};
    let output = prepare_pdf(
        &photo_document(true),
        &PdfOutputSettings {
            mode: CompressionMode::Small,
            ..Default::default()
        },
        None,
        |_, _| Ok(()),
    )
    .unwrap();
    assert!(output.report.warnings.iter().any(|w| w.contains("Mask")));
}

#[test]
fn annotation_page_backreference_does_not_disable_photo_compression() {
    let mut doc = Document::load_mem(&photo_document(false)).unwrap();
    let page = *doc.get_pages().values().next().unwrap();
    let annotation = doc
        .get_object(page)
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"Annots")
        .unwrap()
        .as_array()
        .unwrap()[0]
        .as_reference()
        .unwrap();
    doc.get_object_mut(annotation)
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set("P", page);
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    let result = recompress_rgb_images(&bytes, 70, || Ok(())).unwrap();
    assert_eq!(result.changed_images, 1);
}

#[test]
fn soft_mask_group_images_are_not_recompressed() {
    for inline in [false, true] {
        let mut doc = Document::load_mem(&photo_document(false)).unwrap();
        let page = *doc.get_pages().values().next().unwrap();
        let resources = doc
            .get_object(page)
            .unwrap()
            .as_dict()
            .unwrap()
            .get(b"Resources")
            .unwrap()
            .clone();
        let form=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),1.into(),1.into()],"Resources"=>resources,"Group"=>dictionary!{"S"=>"Transparency","CS"=>"DeviceRGB"}},b"/Photo Do".to_vec()));
        let state =
            dictionary! {"Type"=>"ExtGState","SMask"=>dictionary!{"S"=>"Luminosity","G"=>form}};
        let gs = if inline {
            Object::Dictionary(state)
        } else {
            Object::Reference(doc.add_object(state))
        };
        doc.get_object_mut(page)
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .get_mut(b"Resources")
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set("ExtGState", dictionary! {"MaskState"=>gs});
        let mut bytes = Vec::new();
        doc.save_to(&mut bytes).unwrap();
        let result = recompress_rgb_images(&bytes, 70, || Ok(())).unwrap();
        assert_eq!(
            result.changed_images, 0,
            "RGB image used in soft mask must remain exact"
        );
    }
}

#[test]
fn special_color_space_and_decode_images_keep_their_samples() {
    for cmyk in [true, false] {
        let mut doc = Document::load_mem(&photo_document(false)).unwrap();
        let image_id = *doc
            .objects
            .iter()
            .find(|(_, o)| {
                o.as_stream()
                    .is_ok_and(|s| s.dict.get(b"Width").and_then(Object::as_i64).ok() == Some(1024))
            })
            .unwrap()
            .0;
        let image = doc
            .get_object_mut(image_id)
            .unwrap()
            .as_stream_mut()
            .unwrap();
        if cmyk {
            image
                .dict
                .set("ColorSpace", Object::Name(b"DeviceCMYK".to_vec()));
            image.set_content(vec![127; 1024 * 768 * 4]);
        } else {
            image.dict.set(
                "Decode",
                vec![1.into(), 0.into(), 1.into(), 0.into(), 1.into(), 0.into()],
            );
        }
        let expected = image.content.clone();
        let mut input = Vec::new();
        doc.save_to(&mut input).unwrap();
        let result = recompress_rgb_images(&input, 50, || Ok(())).unwrap();
        assert_eq!(result.changed_images, 0);
        assert!(!result.skipped_images.is_empty());
        let after = Document::load_mem(&result.bytes).unwrap();
        assert_eq!(
            after
                .get_object(image_id)
                .unwrap()
                .as_stream()
                .unwrap()
                .get_plain_content()
                .unwrap(),
            expected
        );
        assert!(!engine()
            .render(&result.bytes, 0, 300, false)
            .unwrap()
            .is_empty());
    }
}

#[test]
#[ignore = "100-page performance fixture; run explicitly and measure process peak memory"]
fn hundred_page_scan_benchmark() {
    use pdf_toolkit_core::pipeline::{prepare_pdf, CompressionMode, PdfOutputSettings};
    let source = photo_document(false);
    let initial = recompress_rgb_images(&source, 95, || Ok(())).unwrap();
    let template = Document::load_mem(&initial.bytes).unwrap();
    let page_id = *template.get_pages().values().next().unwrap();
    let image_id = template
        .get_object(page_id)
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"Resources")
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"XObject")
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"Photo")
        .unwrap()
        .as_reference()
        .unwrap();
    let image = template.get_object(image_id).unwrap().clone();
    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();
    let font =
        doc.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let mut kids = Vec::new();
    for n in 0..100 {
        // Distinct image objects model 100 scanned pages; the synthetic scene repeats.
        let photo = doc.add_object(image.clone());
        let content = doc.add_object(Stream::new(
            dictionary! {},
            format!(
                "q 500 0 0 700 40 70 cm /Photo Do Q BT /F1 12 Tf 40 800 Td (Scan page {}) Tj ET",
                n + 1
            )
            .into_bytes(),
        ));
        let page=doc.add_object(dictionary!{"Type"=>"Page","Parent"=>pages_id,"MediaBox"=>vec![0.into(),0.into(),595.into(),842.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font},"XObject"=>dictionary!{"Photo"=>photo}},"Contents"=>content});
        kids.push(page.into());
    }
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {"Type"=>"Pages","Kids"=>kids,"Count"=>100}),
    );
    let root = doc.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages_id});
    doc.trailer.set("Root", root);
    let mut input = Vec::new();
    doc.save_to(&mut input).unwrap();
    drop(doc);
    let start = std::time::Instant::now();
    let output = prepare_pdf(
        &input,
        &PdfOutputSettings {
            mode: CompressionMode::Balanced,
            ..Default::default()
        },
        None,
        |_, _| Ok(()),
    )
    .unwrap();
    assert!(output.bytes.len() < input.len());
    let parsed = Document::load_mem(&output.bytes).unwrap();
    assert_eq!(parsed.get_pages().len(), 100);
    assert!(parsed
        .extract_text(&[100])
        .unwrap()
        .contains("Scan page 100"));
    let elapsed = start.elapsed().as_secs_f64();
    let result = serde_json::json!({"pages":100,"source_bytes":input.len(),"final_bytes":output.bytes.len(),"seconds":elapsed,"attempts":output.report.attempts,"synthetic":true});
    std::fs::write(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../output/verification/100-page-performance.json"),
        serde_json::to_vec_pretty(&result).unwrap(),
    )
    .unwrap();
    println!("{result}");
}
