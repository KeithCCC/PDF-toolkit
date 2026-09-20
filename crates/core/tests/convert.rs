use pdf_toolkit_core::convert::*;
use std::io::{Cursor, Read};
#[test]
fn office_documents_contain_separate_page_images_and_print_settings() {
    let mut encoded = Cursor::new(vec![]);
    image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
        20,
        30,
        image::Rgb([40, 80, 180]),
    ))
    .write_to(&mut encoded, image::ImageFormat::Png)
    .unwrap();
    let png = encoded.into_inner();
    let images = vec![(png.clone(), 595., 842.), (png.clone(), 420., 595.)];
    for format in ["docx", "xlsx"] {
        let bytes = office_document(format, &images).unwrap();
        let preview = preview_images(&bytes, format).unwrap();
        assert_eq!(preview.len(), 2);
        let rendered = image::load_from_memory(&preview[0]).unwrap();
        assert!(
            rendered.width() > 500,
            "preview must use page layout rather than the source image dimensions"
        );
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut xml = String::new();
        if format == "docx" {
            zip.by_name("word/document.xml")
                .unwrap()
                .read_to_string(&mut xml)
                .unwrap();
            assert!(xml.contains("w:pgSz"));
            assert!(xml.contains("rId2"));
        } else {
            zip.by_name("xl/worksheets/sheet2.xml")
                .unwrap()
                .read_to_string(&mut xml)
                .unwrap();
            assert!(xml.contains("fitToWidth=\"1\""));
            assert!(xml.contains("fitToHeight=\"1\""));
        }
    }
}
