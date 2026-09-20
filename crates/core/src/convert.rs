use anyhow::Result;
use std::io::{Cursor, Read, Write};
use zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};
const REL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const OFFICE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
fn entry(zip: &mut ZipWriter<Cursor<Vec<u8>>>, name: &str, bytes: &[u8]) -> Result<()> {
    zip.start_file(
        name,
        SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated),
    )?;
    zip.write_all(bytes)?;
    Ok(())
}
fn relationships(items: &[(String, String, String)]) -> String {
    format!(
        "<Relationships xmlns=\"{REL}\">{}</Relationships>",
        items
            .iter()
            .map(|(id, typ, target)| format!(
                "<Relationship Id=\"{id}\" Type=\"{OFFICE}/{typ}\" Target=\"{target}\"/>"
            ))
            .collect::<String>()
    )
}
pub fn office_document(format: &str, images: &[(Vec<u8>, f32, f32)]) -> Result<Vec<u8>> {
    anyhow::ensure!(
        !images.is_empty() && ["docx", "xlsx"].contains(&format),
        "変換形式またはページ数が無効です"
    );
    let mut zip = ZipWriter::new(Cursor::new(vec![]));
    let mut types=String::from("<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Default Extension=\"png\" ContentType=\"image/png\"/>");
    if format == "docx" {
        entry(
            &mut zip,
            "_rels/.rels",
            relationships(&[(
                "rId1".into(),
                "officeDocument".into(),
                "word/document.xml".into(),
            )])
            .as_bytes(),
        )?;
        types.push_str("<Override PartName=\"/word/document.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/>");
        let mut xml=String::from("<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" xmlns:wp=\"http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:pic=\"http://schemas.openxmlformats.org/drawingml/2006/picture\"><w:body>");
        let mut rels = vec![];
        for (idx, (image, w, h)) in images.iter().enumerate() {
            let n = idx + 1;
            let cx = (w * 12700.) as i64;
            let cy = (h * 12700.) as i64;
            entry(&mut zip, &format!("word/media/page{n}.png"), image)?;
            rels.push((
                format!("rId{n}"),
                "image".into(),
                format!("media/page{n}.png"),
            ));
            xml.push_str(&format!("<w:p><w:pPr><w:spacing w:before=\"0\" w:after=\"0\"/><w:rPr><w:sz w:val=\"2\"/></w:rPr></w:pPr><w:r><w:drawing><wp:anchor distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\" simplePos=\"0\" relativeHeight=\"0\" behindDoc=\"0\" locked=\"0\" layoutInCell=\"1\" allowOverlap=\"1\"><wp:simplePos x=\"0\" y=\"0\"/><wp:positionH relativeFrom=\"page\"><wp:posOffset>0</wp:posOffset></wp:positionH><wp:positionV relativeFrom=\"page\"><wp:posOffset>0</wp:posOffset></wp:positionV><wp:extent cx=\"{cx}\" cy=\"{cy}\"/><wp:wrapNone/><wp:docPr id=\"{n}\" name=\"Page {n}\"/><a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/picture\"><pic:pic><pic:nvPicPr><pic:cNvPr id=\"{n}\" name=\"Page {n}\"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed=\"rId{n}\"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:anchor></w:drawing></w:r></w:p>"));
            let section=format!("<w:sectPr><w:type w:val=\"nextPage\"/><w:pgSz w:w=\"{}\" w:h=\"{}\"/><w:pgMar w:top=\"0\" w:right=\"0\" w:bottom=\"0\" w:left=\"0\" w:header=\"0\" w:footer=\"0\" w:gutter=\"0\"/></w:sectPr>",(w*20.) as i64,(h*20.) as i64);
            if n == images.len() {
                xml.push_str(&section);
            } else {
                xml.push_str(&format!("<w:p><w:pPr><w:spacing w:before=\"0\" w:after=\"0\" w:line=\"1\" w:lineRule=\"exact\"/>{section}</w:pPr></w:p>"));
            }
        }
        xml.push_str("</w:body></w:document>");
        entry(&mut zip, "word/document.xml", xml.as_bytes())?;
        entry(
            &mut zip,
            "word/_rels/document.xml.rels",
            relationships(&rels).as_bytes(),
        )?;
    } else {
        entry(
            &mut zip,
            "_rels/.rels",
            relationships(&[(
                "rId1".into(),
                "officeDocument".into(),
                "xl/workbook.xml".into(),
            )])
            .as_bytes(),
        )?;
        types.push_str("<Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/>");
        let mut book=format!("<workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:r=\"{OFFICE}\"><sheets>");
        let mut rels = vec![];
        for (idx, (image, w, h)) in images.iter().enumerate() {
            let n = idx + 1;
            let cx = (w * 12700.) as i64;
            let cy = (h * 12700.) as i64;
            entry(&mut zip, &format!("xl/media/page{n}.png"), image)?;
            book.push_str(&format!(
                "<sheet name=\"Page {n}\" sheetId=\"{n}\" r:id=\"rId{n}\"/>"
            ));
            rels.push((
                format!("rId{n}"),
                "worksheet".into(),
                format!("worksheets/sheet{n}.xml"),
            ));
            types.push_str(&format!("<Override PartName=\"/xl/worksheets/sheet{n}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/><Override PartName=\"/xl/drawings/drawing{n}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.drawing+xml\"/>"));
            let sheet=format!("<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:r=\"{OFFICE}\"><sheetPr><pageSetUpPr fitToPage=\"1\"/></sheetPr><sheetData/><pageMargins left=\"0\" right=\"0\" top=\"0\" bottom=\"0\" header=\"0\" footer=\"0\"/><pageSetup paperSize=\"9\" orientation=\"{}\" fitToWidth=\"1\" fitToHeight=\"1\"/><drawing r:id=\"rId1\"/></worksheet>",if w>h{"landscape"}else{"portrait"});
            entry(
                &mut zip,
                &format!("xl/worksheets/sheet{n}.xml"),
                sheet.as_bytes(),
            )?;
            entry(
                &mut zip,
                &format!("xl/worksheets/_rels/sheet{n}.xml.rels"),
                relationships(&[(
                    "rId1".into(),
                    "drawing".into(),
                    format!("../drawings/drawing{n}.xml"),
                )])
                .as_bytes(),
            )?;
            let drawing=format!("<xdr:wsDr xmlns:xdr=\"http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:r=\"{OFFICE}\"><xdr:oneCellAnchor><xdr:from><xdr:col>0</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>0</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:from><xdr:ext cx=\"{cx}\" cy=\"{cy}\"/><xdr:pic><xdr:nvPicPr><xdr:cNvPr id=\"{n}\" name=\"Page {n}\"/><xdr:cNvPicPr/></xdr:nvPicPr><xdr:blipFill><a:blip r:embed=\"rId1\"/><a:stretch><a:fillRect/></a:stretch></xdr:blipFill><xdr:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></xdr:spPr></xdr:pic><xdr:clientData/></xdr:oneCellAnchor></xdr:wsDr>");
            entry(
                &mut zip,
                &format!("xl/drawings/drawing{n}.xml"),
                drawing.as_bytes(),
            )?;
            entry(
                &mut zip,
                &format!("xl/drawings/_rels/drawing{n}.xml.rels"),
                relationships(&[(
                    "rId1".into(),
                    "image".into(),
                    format!("../media/page{n}.png"),
                )])
                .as_bytes(),
            )?;
        }
        book.push_str("</sheets></workbook>");
        entry(&mut zip, "xl/workbook.xml", book.as_bytes())?;
        entry(
            &mut zip,
            "xl/_rels/workbook.xml.rels",
            relationships(&rels).as_bytes(),
        )?;
    }
    types.push_str("</Types>");
    entry(&mut zip, "[Content_Types].xml", types.as_bytes())?;
    Ok(zip.finish()?.into_inner())
}

fn zip_bytes(zip: &mut ZipArchive<Cursor<&[u8]>>, name: &str) -> Result<Vec<u8>> {
    let mut b = vec![];
    zip.by_name(name)?.read_to_end(&mut b)?;
    Ok(b)
}
fn attr(node: roxmltree::Node<'_, '_>, name: &str) -> Result<f32> {
    Ok(node
        .attributes()
        .find(|a| a.name() == name)
        .ok_or_else(|| anyhow::anyhow!("Missing {name}"))?
        .value()
        .parse()?)
}
fn descendant<'a, 'b>(
    node: roxmltree::Node<'a, 'b>,
    name: &str,
) -> Result<roxmltree::Node<'a, 'b>> {
    node.descendants()
        .find(|n| n.tag_name().name() == name)
        .ok_or_else(|| anyhow::anyhow!("Missing {name}"))
}
fn compose_page(image: &[u8], page: (f32, f32), rect: (f32, f32, f32, f32)) -> Result<Vec<u8>> {
    let scale = 800. / page.0;
    let height = (page.1 * scale).round() as u32;
    anyhow::ensure!(height > 0 && height < 12000, "ページサイズが不正です");
    let mut canvas = image::RgbImage::from_pixel(800, height, image::Rgb([255, 255, 255]));
    let img = image::load_from_memory(image)?.to_rgb8();
    let resized = image::imageops::resize(
        &img,
        (rect.2 * scale).round().max(1.) as u32,
        (rect.3 * scale).round().max(1.) as u32,
        image::imageops::FilterType::Triangle,
    );
    image::imageops::overlay(
        &mut canvas,
        &resized,
        (rect.0 * scale).round() as i64,
        (rect.1 * scale).round() as i64,
    );
    let mut out = Cursor::new(vec![]);
    image::DynamicImage::ImageRgb8(canvas).write_to(&mut out, image::ImageFormat::Png)?;
    Ok(out.into_inner())
}
/// Render the restricted OOXML layout produced by this application, including actual
/// section sizes, drawing relationships, anchors, and worksheet fit-to-page settings.
/// This is not a general-purpose Office document viewer.
pub fn preview_images(bytes: &[u8], format: &str) -> Result<Vec<Vec<u8>>> {
    let mut zip = ZipArchive::new(Cursor::new(bytes))?;
    let mut result = vec![];
    if format == "docx" {
        let xml = String::from_utf8(zip_bytes(&mut zip, "word/document.xml")?)?;
        let doc = roxmltree::Document::parse(&xml)?;
        let relxml = String::from_utf8(zip_bytes(&mut zip, "word/_rels/document.xml.rels")?)?;
        let rels = roxmltree::Document::parse(&relxml)?;
        let body = descendant(doc.root(), "body")?;
        let mut pending = None;
        for child in body.children().filter(|n| n.is_element()) {
            if let Ok(anchor) = descendant(child, "anchor") {
                let extent = descendant(anchor, "extent")?;
                let blip = descendant(anchor, "blip")?;
                let rid = blip
                    .attributes()
                    .find(|a| a.name() == "embed")
                    .ok_or_else(|| anyhow::anyhow!("image reference missing"))?
                    .value();
                let rel = rels
                    .descendants()
                    .find(|n| n.attribute("Id") == Some(rid))
                    .ok_or_else(|| anyhow::anyhow!("image relationship missing"))?;
                let target = rel
                    .attribute("Target")
                    .ok_or_else(|| anyhow::anyhow!("image target missing"))?;
                let x: f32 = descendant(descendant(anchor, "positionH")?, "posOffset")?
                    .text()
                    .unwrap_or("0")
                    .parse()?;
                let y: f32 = descendant(descendant(anchor, "positionV")?, "posOffset")?
                    .text()
                    .unwrap_or("0")
                    .parse()?;
                anyhow::ensure!(
                    pending.is_none(),
                    "1ページに複数の画像がある文書には未対応です"
                );
                pending = Some((
                    zip_bytes(&mut zip, &format!("word/{target}"))?,
                    (
                        x / 12700.,
                        y / 12700.,
                        attr(extent, "cx")? / 12700.,
                        attr(extent, "cy")? / 12700.,
                    ),
                ));
            }
            if let Ok(section) = descendant(child, "sectPr") {
                let size = descendant(section, "pgSz")?;
                let (image, rect) = pending
                    .take()
                    .ok_or_else(|| anyhow::anyhow!("画像のないページです"))?;
                result.push(compose_page(
                    &image,
                    (attr(size, "w")? / 20., attr(size, "h")? / 20.),
                    rect,
                )?);
            }
        }
        anyhow::ensure!(pending.is_none(), "セクション情報がありません");
    } else if format == "xlsx" {
        for n in 1.. {
            let sheet = match zip_bytes(&mut zip, &format!("xl/worksheets/sheet{n}.xml")) {
                Ok(b) => String::from_utf8(b)?,
                Err(_) => break,
            };
            let sheetdoc = roxmltree::Document::parse(&sheet)?;
            let setup = descendant(sheetdoc.root(), "pageSetup")?;
            anyhow::ensure!(
                setup.attribute("fitToWidth") == Some("1")
                    && setup.attribute("fitToHeight") == Some("1"),
                "印刷設定は1ページに収まる必要があります"
            );
            let drawing =
                String::from_utf8(zip_bytes(&mut zip, &format!("xl/drawings/drawing{n}.xml"))?)?;
            let drawdoc = roxmltree::Document::parse(&drawing)?;
            let anchor = descendant(drawdoc.root(), "oneCellAnchor")?;
            let extent = descendant(anchor, "ext")?;
            let width = attr(extent, "cx")? / 12700.;
            let height = attr(extent, "cy")? / 12700.;
            let page = if setup.attribute("orientation") == Some("landscape") {
                (841.89, 595.276)
            } else {
                (595.276, 841.89)
            };
            let factor = (page.0 / width).min(page.1 / height);
            let relxml = String::from_utf8(zip_bytes(
                &mut zip,
                &format!("xl/drawings/_rels/drawing{n}.xml.rels"),
            )?)?;
            let rels = roxmltree::Document::parse(&relxml)?;
            let blip = descendant(anchor, "blip")?;
            let rid = blip
                .attributes()
                .find(|a| a.name() == "embed")
                .ok_or_else(|| anyhow::anyhow!("画像参照がありません"))?
                .value();
            let target = rels
                .descendants()
                .find(|n| n.attribute("Id") == Some(rid))
                .and_then(|n| n.attribute("Target"))
                .ok_or_else(|| anyhow::anyhow!("画像の関連付けがありません"))?;
            let path = format!(
                "xl/{}",
                target
                    .strip_prefix("../")
                    .ok_or_else(|| anyhow::anyhow!("画像パスが不正です"))?
            );
            result.push(compose_page(
                &zip_bytes(&mut zip, &path)?,
                page,
                (0., 0., width * factor, height * factor),
            )?);
        }
    } else {
        anyhow::bail!("変換形式が不正です");
    }
    anyhow::ensure!(!result.is_empty(), "変換結果にページがありません");
    Ok(result)
}
