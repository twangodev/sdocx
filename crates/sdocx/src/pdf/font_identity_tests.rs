use super::*;
use crate::{BoundingBox, Color, DocumentMetadata, Page, PageElement, RichTextBox};

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
        .render_layout_page_scenes(
            &document,
            &[&layout.pages[0]],
            &Default::default(),
            &fonts,
            &mut |_| {},
        )
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

fn svg_color_glyph_font(source: &[u8], glyph_id: u16, svg: &[u8], compress_svg: bool) -> Vec<u8> {
    use rustybuzz::ttf_parser::RawFace;
    use write_fonts::{FontBuilder, types::Tag};

    let face = RawFace::parse(source, 0).unwrap();
    let svg = if compress_svg {
        use std::io::Write;
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(svg).unwrap();
        encoder.finish().unwrap()
    } else {
        svg.to_vec()
    };
    let mut table = Vec::new();
    table.extend_from_slice(&0_u16.to_be_bytes());
    table.extend_from_slice(&10_u32.to_be_bytes());
    table.extend_from_slice(&0_u32.to_be_bytes());
    table.extend_from_slice(&1_u16.to_be_bytes());
    table.extend_from_slice(&glyph_id.to_be_bytes());
    table.extend_from_slice(&glyph_id.to_be_bytes());
    // The document offset is relative to the start of the SVG document list.
    table.extend_from_slice(&14_u32.to_be_bytes());
    table.extend_from_slice(&(svg.len() as u32).to_be_bytes());
    table.extend_from_slice(&svg);

    let mut builder = FontBuilder::new();
    for record in face.table_records {
        let raw_tag = record.tag.to_bytes();
        let data = if &raw_tag == b"SVG " {
            table.as_slice()
        } else {
            face.table(record.tag).unwrap()
        };
        builder.add_raw(Tag::new(&raw_tag), data);
    }
    if face
        .table(rustybuzz::ttf_parser::Tag::from_bytes(b"SVG "))
        .is_none()
    {
        builder.add_raw(Tag::new(b"SVG "), table.as_slice());
    }
    builder.build()
}

fn pdf_with_svg_font(font_bytes: Vec<u8>, text_color: Color) -> lopdf::Document {
    let mut database = fontdb::Database::new();
    database.load_font_data(font_bytes);
    database.set_sans_serif_family("Roboto");
    let fonts = FontBook::new(Arc::new(database));
    let bounds = BoundingBox {
        x_min: 10.0,
        y_min: 10.0,
        x_max: 90.0,
        y_max: 90.0,
    };
    let document = Document {
        metadata: DocumentMetadata {
            default_page_dimensions: Some((100, 100)),
            ..Default::default()
        },
        pages: vec![Page {
            uuid: "svg-color-glyph".into(),
            width: 100,
            height: 100,
            content_bbox: bounds,
            background_color: None,
            template: None,
            background: Default::default(),
            objects: vec![
                PageElement::TextBox(RichTextBox {
                    text_area_type: None,
                    bbox: bounds,
                    rotation_degrees: None,
                    text: "A".into(),
                    color: Some(text_color),
                    highlight_color: None,
                    underline: false,
                    font_size: Some(60.0),
                    runs: Vec::new(),
                    spans: Vec::new(),
                    paragraphs: Vec::new(),
                    object_spans: Vec::new(),
                    text_sections: Vec::new(),
                    margins: None,
                    gravity: None,
                })
                .into(),
            ],
        }],
    };
    let pdf_bytes = render_document_pdf(
        &document,
        &RenderOptions::default(),
        &PdfOptions::from_font_book(&fonts),
    )
    .unwrap();
    lopdf::Document::load_mem(&pdf_bytes).unwrap()
}

fn type3_glyph_forms(pdf: &lopdf::Document) -> Vec<Vec<u8>> {
    let mut glyph_forms = Vec::new();
    for object in pdf.objects.values() {
        let Ok(font) = object.as_dict() else { continue };
        if font
            .get(b"Subtype")
            .ok()
            .and_then(|value| value.as_name().ok())
            != Some(b"Type3")
        {
            continue;
        }
        let Ok(resources) = font.get(b"Resources") else {
            continue;
        };
        let Ok((_, resources)) = pdf.dereference(resources) else {
            continue;
        };
        let Ok(resources) = resources.as_dict() else {
            continue;
        };
        let Ok(xobjects) = resources.get(b"XObject") else {
            continue;
        };
        let Ok((_, xobjects)) = pdf.dereference(xobjects) else {
            continue;
        };
        let Ok(xobjects) = xobjects.as_dict() else {
            continue;
        };
        for xobject in xobjects.iter().map(|(_, object)| object) {
            let Ok((_, xobject)) = pdf.dereference(xobject) else {
                continue;
            };
            let Ok(stream) = xobject.as_stream() else {
                continue;
            };
            glyph_forms.push(
                stream
                    .decompressed_content()
                    .unwrap_or_else(|_| stream.content.clone()),
            );
        }
    }
    glyph_forms
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
            ink: Some(&scene.ink),
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

#[test]
fn svg_color_glyph_is_rendered_as_vector_paint_and_keeps_actual_text() {
    let source = include_bytes!("../../assets/fonts/Roboto-Regular.ttf");
    let glyph_id = rustybuzz::ttf_parser::Face::parse(source, 0)
        .unwrap()
        .glyph_index('A')
        .unwrap()
        .0;
    let unselected_glyph = glyph_id + 1;
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 2048 1024 2048"><g transform="translate(25 0)"><g id="glyph{glyph_id}" transform="translate(10 0)"><path fill="currentColor" d="M100 1248H400V2048H100Z"/><path fill="#ff0000" d="M100 848H400V1048H100Z"/></g></g><g id="glyph{unselected_glyph}"><path fill="#00ff00" d="M0 2048H2048V4096H0Z"/></g></svg>"##
    );
    let bytes = svg_color_glyph_font(source, glyph_id, svg.as_bytes(), true);
    let pdf = pdf_with_svg_font(bytes, Color { r: 0, g: 0, b: 255 });
    let page_id = pdf.get_pages()[&1];
    let content = lopdf::content::Content::decode(&pdf.get_page_content(page_id).unwrap()).unwrap();
    let actual_texts = content
        .operations
        .iter()
        .filter(|operation| operation.operator == "BDC")
        .filter_map(|operation| operation.operands.get(1))
        .filter_map(|properties| super::content_properties(&pdf, page_id, properties))
        .filter_map(|properties| {
            properties
                .get(b"ActualText")
                .ok()?
                .as_str()
                .ok()
                .map(<[u8]>::to_vec)
        })
        .collect::<Vec<_>>();
    assert!(
        actual_texts.iter().any(|text| text == b"A"),
        "{actual_texts:?}"
    );

    let glyph_forms = type3_glyph_forms(&pdf);
    assert!(
        glyph_forms.iter().any(|stream| {
            stream.windows(8).any(|window| window == b"0 0 1 rg")
                && stream.windows(8).any(|window| window == b"1 0 0 rg")
                && stream
                    .windows(b"1 0 0 -1 522 2048 cm".len())
                    .any(|window| window == b"1 0 0 -1 522 2048 cm")
                && !stream
                    .windows(b"1 0 0 1 25 0 cm".len())
                    .any(|window| window == b"1 0 0 1 25 0 cm")
                && stream
                    .windows(b"100 1248 m".len())
                    .any(|window| window == b"100 1248 m")
                && stream
                    .windows(b"400 2048 l".len())
                    .any(|window| window == b"400 2048 l")
                && stream.contains(&b'm')
                && stream.contains(&b'f')
        }),
        "expected blue currentColor and intrinsic red vector paths with the viewBox-to-em transform in the selected glyph resource"
    );
    assert!(
        glyph_forms
            .iter()
            .all(|stream| !stream.windows(8).any(|window| window == b"0 1 0 rg")),
        "the unselected glyph's green path must not be emitted"
    );
}

#[test]
fn svg_color_glyph_without_glyph_id_renders_the_document_root() {
    let source = include_bytes!("../../assets/fonts/Roboto-Regular.ttf");
    let glyph_id = rustybuzz::ttf_parser::Face::parse(source, 0)
        .unwrap()
        .glyph_index('A')
        .unwrap()
        .0;
    let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 2048 1024 2048"><path fill="currentColor" d="M100 1248H400V2048H100Z"/><path fill="#ff0000" d="M100 848H400V1048H100Z"/></svg>"##;
    let bytes = svg_color_glyph_font(source, glyph_id, svg, false);
    let pdf = pdf_with_svg_font(bytes, Color { r: 0, g: 0, b: 255 });
    let glyph_forms = type3_glyph_forms(&pdf);
    assert!(
        glyph_forms.iter().any(|stream| {
            stream.windows(8).any(|window| window == b"0 0 1 rg")
                && stream.windows(8).any(|window| window == b"1 0 0 rg")
                && stream
                    .windows(b"1 0 0 -1 512 2048 cm".len())
                    .any(|window| window == b"1 0 0 -1 512 2048 cm")
                && stream
                    .windows(b"100 1248 m".len())
                    .any(|window| window == b"100 1248 m")
                && stream.contains(&b'm')
                && stream.contains(&b'f')
        }),
        "root-level SVG art should be painted as a red glyph-local vector with the em-square transform"
    );
}
