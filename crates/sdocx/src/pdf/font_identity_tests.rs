use super::*;
use crate::{BoundingBox, DocumentMetadata, Page, PageElement, RichTextBox};

fn scene() -> crate::render::RenderedScene {
    let bounds = BoundingBox {
        x_min: 10.0,
        y_min: 10.0,
        x_max: 90.0,
        y_max: 90.0,
    };
    let content = RichTextBox {
        text_area_type: None,
        bbox: bounds,
        rotation_degrees: None,
        text: "W".into(),
        color: None,
        highlight_color: None,
        underline: false,
        font_size: Some(20.0),
        runs: Vec::new(),
        spans: Vec::new(),
        paragraphs: Vec::new(),
        object_spans: Vec::new(),
        text_sections: Vec::new(),
        margins: None,
        gravity: None,
    };
    let document = Document {
        metadata: DocumentMetadata {
            default_page_dimensions: Some((100, 100)),
            ..Default::default()
        },
        pages: vec![Page {
            uuid: "font-identity".into(),
            width: 100,
            height: 100,
            content_bbox: bounds,
            background_color: None,
            template: None,
            background: Default::default(),
            objects: vec![PageElement::TextBox(content).into()],
        }],
    };
    let layout = crate::layout_document(&document);
    let fonts = FontBook::default();
    let scene = DocumentTextCache::default()
        .render_layout_page_scenes(&document, &[&layout.pages[0]], &Default::default(), &fonts)
        .remove(0);
    assert!(scene.text_error.is_none());
    assert_eq!(scene.text.len(), 1);
    scene
}

fn different_fonts() -> PdfOptions {
    let mut database = fontdb::Database::new();
    database.load_font_data(include_bytes!("../../assets/fonts/RobotoMono-Regular.ttf").to_vec());
    database.set_sans_serif_family("Roboto Mono");
    database.set_serif_family("Roboto Mono");
    PdfOptions::new(Arc::new(database))
}

#[test]
fn raw_svg_rejects_a_missing_selected_physical_font() {
    let scene = scene();
    assert!(matches!(
        render_svg_pages_pdf(&[scene.page], &different_fonts()),
        Err(PdfError::UnsupportedText { page_index: 0, .. })
    ));
}

#[test]
fn retained_text_keeps_its_font_when_svg_parsing_uses_a_different_database() {
    let scene = scene();
    let bytes = render_pages_pdf(
        [PdfPage {
            page: &scene.page,
            text: Some(&scene.text),
            text_error: None,
        }],
        &different_fonts(),
    )
    .unwrap();
    let pdf = lopdf::Document::load_mem(&bytes).unwrap();
    assert!(pdf.extract_text(&[1]).unwrap().contains('W'));
    let regular = FontBook::default().resolve("Roboto", false, false).unwrap();
    let regular = rustybuzz::ttf_parser::Face::parse(regular.bytes(), regular.index).unwrap();
    let expected = regular
        .glyph_bounding_box(regular.glyph_index('W').unwrap())
        .unwrap();
    let mut bounds = Vec::new();
    for object in pdf.objects.values() {
        let Ok(dictionary) = object.as_dict() else {
            continue;
        };
        let Ok(font) = dictionary.get(b"FontFile2") else {
            continue;
        };
        let stream = pdf.dereference(font).unwrap().1.as_stream().unwrap();
        let bytes = stream
            .decompressed_content()
            .unwrap_or_else(|_| stream.content.clone());
        let face = rustybuzz::ttf_parser::Face::parse(&bytes, 0).unwrap();
        bounds.extend(
            (0..face.number_of_glyphs())
                .filter_map(|id| face.glyph_bounding_box(rustybuzz::ttf_parser::GlyphId(id))),
        );
    }
    assert!(bounds.contains(&expected), "{bounds:?}");
}
