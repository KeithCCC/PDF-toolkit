use crate::model::{id, Comment, Page};
use anyhow::{Context, Result};
use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use lopdf::{dictionary, Document, Object, Stream};
use pdfium_render::prelude::*;
use std::{collections::HashMap, io::Cursor, path::Path, sync::OnceLock};

pub struct Engine {
    pdfium: Pdfium,
}
impl Engine {
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        static INIT: OnceLock<Result<(), String>> = OnceLock::new();
        INIT.get_or_init(|| {
            Pdfium::bind_to_library(path)
                .map(|bindings| {
                    let _engine = Pdfium::new(bindings);
                })
                .map_err(|e| e.to_string())
        })
        .as_ref()
        .map_err(|e| anyhow::anyhow!(e.clone()))?;
        Ok(Self {
            pdfium: Pdfium::default(),
        })
    }
    pub fn import(
        &self,
        bytes: &[u8],
        password: Option<&str>,
        source: &str,
    ) -> Result<(Vec<u8>, Vec<Page>)> {
        self.import_checked(bytes, password, source, |_, _| Ok(()))
    }
    pub fn import_checked(
        &self,
        bytes: &[u8],
        password: Option<&str>,
        source: &str,
        check: impl Fn(usize, usize) -> Result<()>,
    ) -> Result<(Vec<u8>, Vec<Page>)> {
        let input = self
            .pdfium
            .load_pdf_from_byte_slice(bytes, password)
            .context("PDFを開けません。パスワードまたはファイルを確認してください")?;
        let editable = match input.permissions().can_assemble_document() {
            Ok(assemble) => assemble && input.permissions().can_add_or_modify_text_annotations()?,
            Err(PdfiumError::UnknownPdfSecurityHandlerRevision) => {
                // PDFium has authenticated the password. The wrapper currently only
                // recognizes revisions 2–4; AES-256 revisions 5–6 retain these bits.
                let encrypted = Document::load_mem(bytes)?;
                let encryption = resolve(&encrypted, encrypted.trailer.get(b"Encrypt")?)
                    .context("暗号化設定を読み取れません")?
                    .as_dict()?;
                let revision = encryption.get(b"R")?.as_i64()?;
                anyhow::ensure!(matches!(revision, 5 | 6), "未対応のPDF保護形式です");
                let permissions = encryption.get(b"P")?.as_i64()? as u32;
                permissions & 1024 != 0 && permissions & 32 != 0
            }
            Err(error) => return Err(error.into()),
        };
        anyhow::ensure!(editable, "このPDFの保護設定では編集できません");
        anyhow::ensure!(!input.pages().is_empty(), "PDFにページがありません");
        let mut clean = self.pdfium.create_new_pdf()?;
        for n in 0..input.pages().len() {
            check(n as usize, input.pages().len() as usize)?;
            clean.pages_mut().copy_page_from_document(&input, n, n)?;
        }
        let normalized = clean.save_to_bytes()?;
        let mut parsed = Document::load_mem(&normalized)?;
        let page_ids: Vec<_> = parsed.get_pages().values().copied().collect();
        let mut pages = vec![];
        for (i, page_id) in page_ids.iter().enumerate() {
            check(i, page_ids.len())?;
            let mut page = clean.pages().get(i as i32)?;
            let rotation = page.rotation()?.as_degrees() as i32;
            page.set_rotation(PdfPageRenderRotation::None);
            let mut comments = vec![];
            let anns = annotation_objects(&parsed, *page_id);
            let mut keep = vec![];
            for ann in anns {
                let dict = resolve(&parsed, &ann).and_then(|o| o.as_dict().ok());
                if let Some(raw) = dict
                    .and_then(|d| d.get(b"PDFToolkit").ok())
                    .and_then(|o| o.as_str().ok())
                {
                    if let Ok(mut c) = serde_json::from_slice::<Comment>(raw) {
                        c.id = id();
                        comments.push(c);
                        continue;
                    }
                }
                keep.push(ann);
            }
            parsed
                .get_object_mut(*page_id)?
                .as_dict_mut()?
                .set("Annots", keep);
            pages.push(Page {
                id: id(),
                source: source.into(),
                source_name: String::new(),
                source_path: String::new(),
                index: i as u16,
                width: page.width().value,
                height: page.height().value,
                rotation,
                comments,
            });
        }
        let mut output = vec![];
        parsed.save_to(&mut output)?;
        Ok((output, pages))
    }
    pub fn export(&self, pages: &[Page], sources: &HashMap<String, Vec<u8>>) -> Result<Vec<u8>> {
        self.export_checked(pages, sources, |_, _| Ok(()))
    }
    pub fn export_checked(
        &self,
        pages: &[Page],
        sources: &HashMap<String, Vec<u8>>,
        check: impl Fn(usize, usize) -> Result<()>,
    ) -> Result<Vec<u8>> {
        anyhow::ensure!(!pages.is_empty(), "空のグループは保存できません");
        anyhow::ensure!(
            pages.len() <= u16::MAX as usize,
            "ページ数が上限を超えています"
        );
        let mut out = self.pdfium.create_new_pdf()?;
        for (n, p) in pages.iter().enumerate() {
            check(n, pages.len())?;
            let bytes = sources
                .get(&p.source)
                .context("元PDFが作業データにありません")?;
            let source = self.pdfium.load_pdf_from_byte_slice(bytes, None)?;
            out.pages_mut()
                .copy_page_from_document(&source, p.index as i32, n as i32)?;
            out.pages().get(n as i32)?.set_rotation(match p.rotation {
                90 => PdfPageRenderRotation::Degrees90,
                180 => PdfPageRenderRotation::Degrees180,
                270 => PdfPageRenderRotation::Degrees270,
                _ => PdfPageRenderRotation::None,
            });
        }
        let mut doc = Document::load_mem(&out.save_to_bytes()?)?;
        let page_ids: Vec<_> = doc.get_pages().values().copied().collect();
        for (i, (p, page_id)) in pages.iter().zip(page_ids).enumerate() {
            check(i, pages.len())?;
            for c in &p.comments {
                add_comment(&mut doc, page_id, p, c)?;
            }
        }
        doc.compress();
        let mut output = vec![];
        doc.save_to(&mut output)?;
        Ok(output)
    }
    pub fn render(&self, bytes: &[u8], index: u16, width: i32, unrotated: bool) -> Result<Vec<u8>> {
        let document = self.pdfium.load_pdf_from_byte_slice(bytes, None)?;
        let mut page = document.pages().get(index as i32)?;
        if unrotated {
            page.set_rotation(PdfPageRenderRotation::None);
        }
        let img = page
            .render_with_config(
                &PdfRenderConfig::new()
                    .set_target_width(width.clamp(64, 5000))
                    .set_maximum_height(6000)
                    .render_annotations(true),
            )?
            .as_image()?;
        let mut out = Cursor::new(vec![]);
        img.write_to(&mut out, ImageFormat::Png)?;
        Ok(out.into_inner())
    }
    pub fn sample(&self) -> Result<Vec<u8>> {
        let mut doc = Document::with_version("1.7");
        let pages_id = doc.new_object_id();
        let font =
            doc.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
        let mut kids = vec![];
        for (i, (w, h)) in [(595, 842), (420, 595)].iter().enumerate() {
            let content=format!("0.9 0.95 1 rg 30 30 {} {} re f 0.1 0.3 0.6 rg BT /F1 28 Tf 50 {} Td (PDF Toolkit - Page {}) Tj ET",w-60,h-60,h-90,i+1);
            let stream = doc.add_object(Stream::new(dictionary! {}, content.into_bytes()));
            let p=doc.add_object(dictionary!{"Type"=>"Page","Parent"=>pages_id,"MediaBox"=>vec![0.into(),0.into(),(*w).into(),(*h).into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>stream});
            kids.push(p.into());
        }
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {"Type"=>"Pages","Kids"=>kids,"Count"=>2}),
        );
        let catalog = doc.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages_id});
        doc.trailer.set("Root", catalog);
        let mut b = vec![];
        doc.save_to(&mut b)?;
        Ok(b)
    }
}
fn resolve<'a>(doc: &'a Document, o: &'a Object) -> Option<&'a Object> {
    if let Ok(id) = o.as_reference() {
        doc.get_object(id).ok()
    } else {
        Some(o)
    }
}
fn annotation_objects(doc: &Document, page: lopdf::ObjectId) -> Vec<Object> {
    doc.get_object(page)
        .ok()
        .and_then(|o| o.as_dict().ok())
        .and_then(|d| d.get(b"Annots").ok())
        .and_then(|o| resolve(doc, o))
        .and_then(|o| o.as_array().ok())
        .cloned()
        .unwrap_or_default()
}
fn color(s: &str) -> Result<[u8; 3]> {
    anyhow::ensure!(
        s.len() == 7 && s.starts_with('#') && s.is_ascii(),
        "色が無効です"
    );
    Ok([
        u8::from_str_radix(&s[1..3], 16)?,
        u8::from_str_radix(&s[3..5], 16)?,
        u8::from_str_radix(&s[5..7], 16)?,
    ])
}
pub fn comment_image(c: &Comment) -> Result<RgbImage> {
    anyhow::ensure!(
        [c.x, c.y, c.width, c.height, c.font_size]
            .iter()
            .all(|v| v.is_finite())
            && c.width >= 10.
            && c.height >= 10.
            && c.width <= 3000.
            && c.height <= 3000.
            && c.font_size >= 6.
            && c.font_size <= 144.
            && c.text.len() <= 32000,
        "コメントのサイズまたは文字数が無効です"
    );
    let bg = color(&c.background)?;
    let fg = color(&c.color)?;
    let scale = 2.;
    let w = (c.width * scale).ceil() as u32;
    let h = (c.height * scale).ceil() as u32;
    let mut img = RgbImage::from_pixel(w, h, Rgb(bg));
    static FONT: OnceLock<Result<fontdue::Font, String>> = OnceLock::new();
    let font = FONT
        .get_or_init(|| {
            fontdue::Font::from_bytes(
                include_bytes!("../../../assets/fonts/NotoSansJP.ttf") as &[u8],
                fontdue::FontSettings::default(),
            )
            .map_err(str::to_owned)
        })
        .as_ref()
        .map_err(|e| anyhow::anyhow!(e.clone()))?;
    let mut layout = fontdue::layout::Layout::new(fontdue::layout::CoordinateSystem::PositiveYDown);
    layout.reset(&fontdue::layout::LayoutSettings {
        x: 8.,
        y: 8.,
        max_width: Some(w as f32 - 16.),
        line_height: 1.2,
        ..Default::default()
    });
    layout.append(
        &[font],
        &fontdue::layout::TextStyle::new(&c.text, c.font_size * scale, 0),
    );
    for g in layout.glyphs() {
        if g.parent.is_control() {
            continue;
        }
        let (m, bitmap) = font.rasterize_config(g.key);
        for y in 0..m.height {
            for x in 0..m.width {
                let px = g.x as i32 + x as i32;
                let py = g.y as i32 + y as i32;
                if px >= 0 && py >= 0 && px < w as i32 && py < h as i32 {
                    let a = bitmap[y * m.width + x] as u32;
                    img.put_pixel(
                        px as u32,
                        py as u32,
                        Rgb(std::array::from_fn(|i| {
                            ((fg[i] as u32 * a + bg[i] as u32 * (255 - a)) / 255) as u8
                        })),
                    );
                }
            }
        }
    }
    Ok(img)
}
pub fn comment_png(c: &Comment) -> Result<Vec<u8>> {
    let mut out = Cursor::new(vec![]);
    DynamicImage::ImageRgb8(comment_image(c)?).write_to(&mut out, ImageFormat::Png)?;
    Ok(out.into_inner())
}
fn add_comment(doc: &mut Document, page_id: lopdf::ObjectId, p: &Page, c: &Comment) -> Result<()> {
    let img = comment_image(c)?;
    let image_id=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Image","Width"=>img.width() as i64,"Height"=>img.height() as i64,"ColorSpace"=>"DeviceRGB","BitsPerComponent"=>8},img.into_raw()));
    let appearance=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),c.width.into(),c.height.into()],"Resources"=>dictionary!{"XObject"=>dictionary!{"Im0"=>image_id}}},format!("q {} 0 0 {} 0 0 cm /Im0 Do Q",c.width,c.height).into_bytes()));
    let dict = doc.get_object(page_id)?.as_dict()?;
    let crop = dict
        .get(b"CropBox")
        .or_else(|_| dict.get(b"MediaBox"))
        .ok()
        .and_then(|o| o.as_array().ok());
    let number = |o: &Object| o.as_float().unwrap_or(0.);
    let ox = crop.and_then(|a| a.first()).map(number).unwrap_or(0.);
    let oy = crop.and_then(|a| a.get(1)).map(number).unwrap_or(0.);
    let rect = vec![
        (ox + c.x).into(),
        (oy + p.height - c.y - c.height).into(),
        (ox + c.x + c.width).into(),
        (oy + p.height - c.y).into(),
    ];
    let mut unicode = vec![0xFE, 0xFF];
    for u in c.text.encode_utf16() {
        unicode.extend(u.to_be_bytes());
    }
    let ann=doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"FreeText","Rect"=>rect,"Contents"=>Object::String(unicode,lopdf::StringFormat::Hexadecimal),"F"=>4,"DA"=>Object::string_literal(format!("/Helv {} Tf 0 g",c.font_size)),"NM"=>Object::string_literal(c.id.clone()),"PDFToolkit"=>Object::string_literal(serde_json::to_vec(c)?),"AP"=>dictionary!{"N"=>appearance}});
    let mut anns = annotation_objects(doc, page_id);
    anns.push(ann.into());
    doc.get_object_mut(page_id)?
        .as_dict_mut()?
        .set("Annots", anns);
    Ok(())
}
