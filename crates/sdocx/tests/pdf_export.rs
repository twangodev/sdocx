#![cfg(feature = "pdf")]

use std::sync::Arc;

use sdocx::{PdfError, PdfOptions, RenderedPage, render_svg_pages_pdf};

fn page(width: u32, height: u32, content: &str) -> RenderedPage {
    RenderedPage {
        source_page_index: 0,
        width,
        height,
        svg: format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}">{content}</svg>"#
        ),
    }
}

fn no_fonts() -> PdfOptions {
    PdfOptions::new(Arc::new(sdocx::pdf::fontdb::Database::new()))
}

#[test]
fn pages_keep_order_dimensions_vectors_and_selectable_embedded_text() {
    let mut fonts = sdocx::pdf::fontdb::Database::new();
    fonts.load_system_fonts();
    let family = fonts.faces().next().expect("a system font").families[0]
        .0
        .clone();
    fonts.set_sans_serif_family(&family);
    let options = PdfOptions::new(Arc::new(fonts));
    let pages = [
        page(
            400,
            200,
            r#"<path d="M5 5 L100 50" stroke="red"/><text x="10" y="70" font-size="20" font-family="sans-serif">First page</text>"#,
        ),
        page(
            200,
            400,
            r#"<text x="10" y="70" font-size="20" font-family="sans-serif">Second page</text>"#,
        ),
    ];
    let bytes = render_svg_pages_pdf(&pages, &options).unwrap();
    let pdf = lopdf::Document::load_mem(&bytes).unwrap();
    let ids = pdf.get_pages();
    assert_eq!(ids.len(), 2);
    for ((_, id), expected) in ids.iter().zip([[300.0, 150.0], [150.0, 300.0]]) {
        let bounds = pdf
            .get_dictionary(*id)
            .unwrap()
            .get(b"MediaBox")
            .unwrap()
            .as_array()
            .unwrap();
        let dimensions: Vec<_> = bounds
            .iter()
            .map(|number| number.as_float().unwrap())
            .collect();
        assert_eq!(dimensions, [0.0, 0.0, expected[0], expected[1]]);
    }
    let first = pdf.extract_text(&[1]).unwrap().replace('\n', "");
    let second = pdf.extract_text(&[2]).unwrap().replace('\n', "");
    assert!(first.contains("First page"), "extracted: {first:?}");
    assert!(!first.contains("Second page"));
    assert!(second.contains("Second page"), "extracted: {second:?}");
    assert!(pdf.objects.values().any(|object| {
        object
            .as_dict()
            .is_ok_and(|dict| dict.has(b"FontFile2") || dict.has(b"FontFile3"))
    }));
    assert!(
        !pdf.objects
            .values()
            .any(|object| object.as_stream().is_ok_and(|stream| stream
                .dict
                .get(b"Subtype")
                .is_ok_and(|value| value.as_name().is_ok_and(|name| name == b"Image"))))
    );
    let content = pdf.get_page_content(ids[&1]).unwrap();
    let operations = lopdf::content::Content::decode(&content)
        .unwrap()
        .operations;
    assert!(
        operations.iter().any(|op| op.operator == "S"),
        "line stays a vector stroke"
    );
}

#[test]
fn scale_controls_physical_size_and_embedded_images_survive() {
    let mut options = no_fonts();
    options.dpi = 144.0;
    let image = r#"<image width="40" height="20" href="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC"/>"#;
    let bytes = render_svg_pages_pdf(&[page(200, 100, image)], &options).unwrap();
    let pdf = lopdf::Document::load_mem(&bytes).unwrap();
    let id = pdf.get_pages()[&1];
    let bounds = pdf
        .get_dictionary(id)
        .unwrap()
        .get(b"MediaBox")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(bounds[2].as_float().unwrap(), 100.0);
    assert_eq!(bounds[3].as_float().unwrap(), 50.0);
    assert!(
        pdf.objects
            .values()
            .any(|object| object.as_stream().is_ok_and(|stream| stream
                .dict
                .get(b"Subtype")
                .is_ok_and(|value| value.as_name().is_ok_and(|name| name == b"Image"))))
    );
}

#[test]
fn invalid_input_returns_errors_instead_of_a_partial_pdf() {
    let mut options = no_fonts();
    assert!(matches!(
        render_svg_pages_pdf(&[], &options),
        Err(PdfError::EmptyDocument)
    ));
    for dpi in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        options.dpi = dpi;
        assert!(matches!(
            render_svg_pages_pdf(&[page(100, 100, "")], &options),
            Err(PdfError::InvalidDpi)
        ));
    }
    options.dpi = 96.0;
    for (width, height) in [(0, 100), (100, 0), (u32::MAX, 100)] {
        assert!(matches!(
            render_svg_pages_pdf(&[page(width, height, "")], &options),
            Err(PdfError::InvalidPageSize { page_index: 0 })
        ));
    }
    let mut broken = page(100, 100, "");
    broken.svg = "invalid SVG".into();
    assert!(matches!(
        render_svg_pages_pdf(&[page(100, 100, ""), broken], &options),
        Err(PdfError::InvalidSvg { page_index: 1, .. })
    ));
    let mut mismatched = page(100, 100, "");
    mismatched.width = 200;
    assert!(matches!(
        render_svg_pages_pdf(&[mismatched], &options),
        Err(PdfError::InvalidPageSize { page_index: 0 })
    ));
}

#[test]
fn corrupt_png_returns_an_error_before_the_converter_can_panic() {
    let image = r#"<image width="40" height="20" href="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aV1cAAAAASUVORK5CYII="/>"#;
    assert!(matches!(
        render_svg_pages_pdf(&[page(200, 100, image)], &no_fonts()),
        Err(PdfError::InvalidImage { page_index: 0, .. })
    ));
}

#[test]
fn document_export_uses_visible_layout_and_color_options() {
    let page = sdocx::Page {
        uuid: "page".into(),
        width: 200,
        height: 100,
        content_bbox: Default::default(),
        background_color: None,
        template: None,
        strokes: vec![],
        elements: vec![],
    };
    let document = sdocx::Document {
        pages: vec![page.clone(), page],
        metadata: sdocx::DocumentMetadata {
            note_text: Some(sdocx::RichTextBox {
                text_area_type: None,
                bbox: Default::default(),
                rotation_degrees: None,
                text: "visible text".into(),
                color: None,
                highlight_color: None,
                underline: false,
                font_size: None,
                runs: vec![],
                spans: vec![],
                paragraphs: vec![],
                object_spans: vec![],
                text_sections: vec![],
                margins: None,
                gravity: None,
            }),
            ..Default::default()
        },
    };
    let mut render_options = sdocx::RenderOptions::default();
    render_options.color_mode = sdocx::RenderColorMode::Dark;
    let options = PdfOptions::default();
    let bytes = sdocx::render_document_pdf(&document, &render_options, &options).unwrap();
    let pdf = lopdf::Document::load_mem(&bytes).unwrap();
    assert_eq!(pdf.get_pages().len(), 1, "omit trailing storage page");
    let expected = render_svg_pages_pdf(
        &sdocx::render_document_svg(&document, &render_options),
        &options,
    )
    .unwrap();
    assert_eq!(bytes, expected, "same layout and color treatment as SVG");
}
