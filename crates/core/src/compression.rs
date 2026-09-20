//! Conservative first-stage compression: content stays as PDF objects, not page bitmaps.
use anyhow::{Context, Result};
use image::{codecs::jpeg::JpegEncoder, ExtendedColorType, ImageFormat, ImageReader};
use lopdf::{Document, Object, ObjectId, Stream};
use std::{collections::HashSet, io::Cursor};

#[derive(Debug)]
pub struct CompressionResult {
    pub bytes: Vec<u8>,
    pub changed_images: usize,
    pub skipped_images: Vec<String>,
}

fn load(bytes: &[u8]) -> Result<Document> {
    let document = Document::load_mem(bytes).context("圧縮するPDFを読み込めません")?;
    anyhow::ensure!(
        !document.is_encrypted(),
        "圧縮前にPDFのパスワードを解除してください"
    );
    Ok(document)
}

fn save(mut document: Document) -> Result<Vec<u8>> {
    document.compress();
    let mut bytes = Vec::new();
    document.save_to(&mut bytes)?;
    Ok(bytes)
}

pub fn optimize_lossless(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut document = load(bytes)?;
    document.prune_objects();
    let candidate = save(document)?;
    // A no-op is preferable to a larger "compressed" PDF.
    Ok(if candidate.len() < bytes.len() {
        candidate
    } else {
        bytes.to_vec()
    })
}

// Walk references without cloning stream payloads. Cyclic PDF graphs are bounded by IDs.
fn references(document: &Document, roots: Vec<&Object>, stop_at_pages: bool) -> HashSet<ObjectId> {
    let mut stack = roots;
    let mut ids = HashSet::new();
    while let Some(object) = stack.pop() {
        match object {
            Object::Reference(id) => {
                if ids.insert(*id) {
                    if let Ok(next) = document.get_object(*id) {
                        stack.push(next);
                    }
                }
            }
            Object::Dictionary(dict) => {
                if !stop_at_pages
                    || !matches!(
                        dict.get(b"Type").and_then(Object::as_name).ok(),
                        Some(b"Page" | b"Pages")
                    )
                {
                    stack.extend(dict.iter().map(|(_, o)| o));
                }
            }
            Object::Stream(stream) => stack.extend(stream.dict.iter().map(|(_, o)| o)),
            Object::Array(array) => stack.extend(array),
            _ => {}
        }
    }
    ids
}

fn image_pixels(stream: &Stream) -> Result<(Vec<u8>, u32, u32)> {
    for key in [
        b"Mask".as_slice(),
        b"SMask",
        b"SMaskInData",
        b"Decode",
        b"DecodeParms",
    ] {
        anyhow::ensure!(
            !stream.dict.has(key),
            "{}付き画像は保持しました",
            String::from_utf8_lossy(key)
        );
    }
    anyhow::ensure!(
        !stream
            .dict
            .get(b"ImageMask")
            .and_then(Object::as_bool)
            .unwrap_or(false),
        "ImageMask画像は保持しました"
    );
    anyhow::ensure!(
        stream
            .dict
            .get(b"ColorSpace")
            .and_then(Object::as_name)
            .ok()
            == Some(b"DeviceRGB"),
        "DeviceRGB以外の画像は保持しました"
    );
    anyhow::ensure!(
        stream
            .dict
            .get(b"BitsPerComponent")
            .and_then(Object::as_i64)
            .ok()
            == Some(8),
        "8bit以外の画像は保持しました"
    );
    let width = u32::try_from(stream.dict.get(b"Width")?.as_i64()?)?;
    let height = u32::try_from(stream.dict.get(b"Height")?.as_i64()?)?;
    let count = (width as u64)
        .checked_mul(height as u64)
        .context("画像サイズが不正です")?;
    anyhow::ensure!(
        width > 0 && height > 0 && count <= 40_000_000,
        "画像サイズが対応範囲外です"
    );
    let expected = usize::try_from(count * 3)?;
    let filters = stream.filters().unwrap_or_default();
    let pixels = if filters == [b"DCTDecode".as_slice()] {
        let reader = ImageReader::with_format(Cursor::new(&stream.content), ImageFormat::Jpeg);
        anyhow::ensure!(
            reader.into_dimensions()? == (width, height),
            "画像の寸法情報が一致しません"
        );
        image::load_from_memory_with_format(&stream.content, ImageFormat::Jpeg)?
            .to_rgb8()
            .into_raw()
    } else if filters.is_empty() || filters == [b"FlateDecode".as_slice()] {
        stream.get_plain_content_with_limit(expected)?
    } else {
        anyhow::bail!("未対応の画像フィルターは保持しました")
    };
    anyhow::ensure!(pixels.len() == expected, "画像のデータ長が一致しません");
    Ok((pixels, width, height))
}

pub fn recompress_rgb_images(
    bytes: &[u8],
    quality: u8,
    check: impl Fn() -> Result<()>,
) -> Result<CompressionResult> {
    recompress_rgb_images_at_dpi(bytes, quality, None, check)
}

pub fn recompress_rgb_images_at_dpi(
    bytes: &[u8],
    quality: u8,
    dpi: Option<u16>,
    check: impl Fn() -> Result<()>,
) -> Result<CompressionResult> {
    anyhow::ensure!(
        dpi.is_none_or(|d| (36..=600).contains(&d)),
        "解像度は36～600dpiで指定してください"
    );
    anyhow::ensure!(
        (1..=100).contains(&quality),
        "JPEG品質は1～100で指定してください"
    );
    check()?;
    let mut document = load(bytes)?;
    let mut annotation_roots = Vec::new();
    let mut resource_roots = Vec::new();
    for page_id in document.get_pages().values() {
        let page = document.get_object(*page_id)?.as_dict()?;
        if let Ok(annotations) = page.get(b"Annots") {
            annotation_roots.push(annotations);
        }
        let mut dictionary = page;
        let mut ancestors = HashSet::new();
        loop {
            if let Ok(resources) = dictionary.get(b"Resources") {
                resource_roots.push(resources);
                break;
            }
            let Ok(parent) = dictionary.get(b"Parent").and_then(Object::as_reference) else {
                break;
            };
            anyhow::ensure!(ancestors.insert(parent), "ページの親参照が循環しています");
            dictionary = document.get_object(parent)?.as_dict()?;
        }
    }
    // Image masks and transparency groups also contain image resources. Their
    // samples define alpha, not ordinary photograph colors, so preserve them.
    let mut nested: Vec<_> = document.objects.values().collect();
    while let Some(object) = nested.pop() {
        if let Object::Array(items) = object {
            nested.extend(items);
        }
        let dict = match object {
            Object::Dictionary(dict) => Some(dict),
            Object::Stream(stream) => Some(&stream.dict),
            _ => None,
        };
        if let Some(dict) = dict {
            nested.extend(dict.iter().map(|(_, value)| value));
            for key in [b"SMask".as_slice(), b"Mask"] {
                if let Ok(mask) = dict.get(key) {
                    annotation_roots.push(mask);
                }
            }
        }
    }
    let protected = references(&document, annotation_roots, true);
    let resources = references(&document, resource_roots, false);
    let mut changed_images = 0;
    let mut skipped_images = Vec::new();
    let sizes = if dpi.is_some() {
        match crate::image_placement::image_sizes(&document) {
            Ok(sizes) => sizes,
            Err(error) => {
                skipped_images.push(format!("解像度を保持しました：{error}"));
                Default::default()
            }
        }
    } else {
        Default::default()
    };
    for (id, object) in document.objects.iter_mut() {
        check()?;
        if !resources.contains(id) {
            continue;
        }
        let Ok(stream) = object.as_stream_mut() else {
            continue;
        };
        if stream.dict.get(b"Subtype").and_then(Object::as_name).ok() != Some(b"Image") {
            continue;
        }
        if protected.contains(id) {
            skipped_images.push("注釈またはマスクから参照される画像は保持しました".into());
            continue;
        }
        let (mut pixels, mut width, mut height) = match image_pixels(stream) {
            Ok(value) => value,
            Err(error) => {
                skipped_images.push(error.to_string());
                continue;
            }
        };
        if let (Some(dpi), Some((w, h))) = (dpi, sizes.get(id)) {
            let target_w = (w * dpi as f64 / 72.).ceil().clamp(1., width as f64) as u32;
            let target_h = (h * dpi as f64 / 72.).ceil().clamp(1., height as f64) as u32;
            if (target_w, target_h) != (width, height) {
                let rgb = image::RgbImage::from_raw(width, height, pixels)
                    .context("画像サイズが不正です")?;
                pixels = image::imageops::resize(
                    &rgb,
                    target_w,
                    target_h,
                    image::imageops::FilterType::Lanczos3,
                )
                .into_raw();
                width = target_w;
                height = target_h;
            }
        }
        let mut jpeg = Vec::new();
        JpegEncoder::new_with_quality(&mut jpeg, quality).encode(
            &pixels,
            width,
            height,
            ExtendedColorType::Rgb8,
        )?;
        check()?;
        if jpeg.len() >= stream.content.len() {
            continue;
        }
        stream.set_content(jpeg);
        stream.dict.set("Width", i64::from(width));
        stream.dict.set("Height", i64::from(height));
        stream
            .dict
            .set("Filter", Object::Name(b"DCTDecode".to_vec()));
        changed_images += 1;
    }
    check()?;
    let candidate = save(document)?;
    check()?;
    let output = if candidate.len() < bytes.len() {
        candidate
    } else {
        changed_images = 0;
        bytes.to_vec()
    };
    Ok(CompressionResult {
        bytes: output,
        changed_images,
        skipped_images,
    })
}
