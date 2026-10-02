#![cfg(feature = "render")]

use sdocx::{
    BoundingBox, Document, DocumentMetadata, ObjectSpanLayoutConstraint, ObjectSpanLayoutOption,
    ObjectType, Page, RenderedPage, RichTextBox, RichTextCodeBlock, RichTextObjectContent,
    RichTextObjectSpan, RichTextSection,
};

const A_ADVANCE: f64 = (652.0_f32 / 100.0) as f64;
const CODE_BODY: &str = "AAAAAAAAAAAAAAAAAAAAAA";

fn bounds(width: f64, height: f64) -> BoundingBox {
    BoundingBox {
        x_min: 0.0,
        y_min: 0.0,
        x_max: width,
        y_max: height,
    }
}

fn text(source: &str, font_size: f32) -> RichTextBox {
    RichTextBox {
        text_area_type: None,
        bbox: BoundingBox::default(),
        rotation_degrees: None,
        text: source.into(),
        color: None,
        highlight_color: None,
        underline: false,
        font_size: Some(font_size),
        runs: vec![],
        spans: vec![],
        paragraphs: vec![],
        object_spans: vec![],
        text_sections: vec![],
        margins: None,
        gravity: None,
    }
}

fn metadata(maximum_width: f32) -> Vec<u8> {
    let mut fixed = 5500_u32.to_le_bytes().to_vec();
    fixed.extend(0_u16.to_le_bytes());
    fixed.extend(0_i64.to_le_bytes());
    fixed.extend(
        [0.0_f64, 0.0, 300.0, 100.0]
            .into_iter()
            .flat_map(f64::to_le_bytes),
    );
    fixed.extend([0; 5]);
    let offset = 15 + fixed.len();
    let mut data = ((offset + 8) as u32).to_le_bytes().to_vec();
    data.extend(0_i16.to_le_bytes());
    data.extend((offset as u32).to_le_bytes());
    data.extend([1, 8, 2, 0, 1]);
    data.extend(fixed);
    data.extend(maximum_width.to_le_bytes());
    data.extend(500.0_f32.to_le_bytes());
    data
}

fn document(constraint: ObjectSpanLayoutConstraint, maximum_width: f32) -> Document {
    let mut body = text("Z\nA\u{fffc}B\nQ\nR", 10.0);
    body.text_sections = vec![
        RichTextSection {
            start_utf16: 0,
            length_utf16: 2,
        },
        RichTextSection {
            start_utf16: 2,
            length_utf16: 5,
        },
    ];
    body.object_spans.push(RichTextObjectSpan {
        object_type: ObjectType::CodeBlock,
        object_data: metadata(maximum_width),
        content: Some(RichTextObjectContent::CodeBlock(Box::new(
            RichTextCodeBlock {
                bbox: bounds(300.0, 100.0),
                rotation_degrees: None,
                title: Some(text("T", 1.0)),
                body: Some(text(CODE_BODY, 10.0)),
            },
        ))),
        text_index_utf16: 3,
        layout_option: ObjectSpanLayoutOption::Inline,
        layout_constraint: constraint,
    });
    Document {
        pages: (0..2)
            .map(|index| Page {
                uuid: format!("code-width-{index}"),
                width: 500,
                height: 500,
                content_bbox: bounds(500.0, 500.0),
                background_color: None,
                template: None,
                background: Default::default(),
                objects: vec![],
            })
            .collect(),
        metadata: DocumentMetadata {
            note_text: Some(body),
            page_mode: Some(0),
            default_page_dimensions: Some((360, 500)),
            orientation: Some(0),
            flow_page_padding: Some((0, 0)),
            ..Default::default()
        },
    }
}

fn render(document: &Document, replay: bool) -> RenderedPage {
    let layout = sdocx::layout_document(document);
    let window = layout.pages[1]
        .body_text_slice()
        .unwrap()
        .capture_window
        .as_ref()
        .unwrap();
    assert_eq!(window.first_page_index, 1);
    assert_eq!(window.saved_source_range, 2..7);
    let page = if replay {
        sdocx::render_layout_page_replay_svg(document, &layout, 1, &Default::default())
    } else {
        sdocx::render_layout_page_svg(document, &layout, 1, &Default::default())
    }
    .unwrap();
    assert!(
        page.text_diagnostics.is_empty(),
        "{:?}",
        page.text_diagnostics
    );
    assert!(
        page.object_diagnostics.is_empty(),
        "{:?}",
        page.object_diagnostics
    );
    page
}

fn point(node: roxmltree::Node<'_, '_>) -> (f64, f64) {
    let coordinate = |key| {
        node.ancestors()
            .find_map(|ancestor| ancestor.attribute(key))
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .parse::<f64>()
            .unwrap()
    };
    let mut point = (coordinate("x"), coordinate("y"));
    for ancestor in node.ancestors() {
        if let Some(transform) = ancestor.attribute("transform") {
            let transform: svgtypes::Transform = transform.parse().unwrap();
            point = (
                transform.a * point.0 + transform.c * point.1 + transform.e,
                transform.b * point.0 + transform.d * point.1 + transform.f,
            );
        }
    }
    point
}

fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 0.0001, "{actual} != {expected}");
}

fn assert_geometry(page: &RenderedPage, panel_width: f64, normal: bool) {
    let xml = roxmltree::Document::parse(&page.svg).unwrap();
    assert!(!xml.descendants().any(|node| node.has_tag_name("image")));
    let glyph = |source| {
        point(
            xml.descendants()
                .find(|node| node.has_tag_name("tspan") && node.text() == Some(source))
                .unwrap(),
        )
    };
    let a = glyph("A");
    let b = glyph("B");
    close(a.0, 0.0);
    close(
        b.0,
        A_ADVANCE
            + if normal || panel_width == 300.0 {
                308.0
            } else {
                panel_width
            },
    );
    close(a.1, if normal { 110.001 } else { 87.501 });
    close(b.1, a.1);
    close(glyph("Q").1, if normal { 123.501 } else { 101.001 });
    let panel = xml
        .descendants()
        .find(|node| node.has_tag_name("rect") && node.attribute("fill") == Some("#efefef"))
        .unwrap();
    assert!((point(panel).0 - (A_ADVANCE + 4.0)).abs() < 0.005);
    close(
        panel.attribute("width").unwrap().parse().unwrap(),
        panel_width,
    );
    close(
        panel.attribute("height").unwrap().parse().unwrap(),
        if panel_width == 150.0 { 91.0 } else { 77.5 },
    );
    let body_lines: Vec<_> = xml
        .descendants()
        .filter(|node| {
            node.has_tag_name("tspan")
                && node.text().is_some_and(|source| {
                    source.len() > 1 && source.bytes().all(|byte| byte == b'A')
                })
        })
        .collect();
    let expected = if panel_width == 150.0 {
        vec!["AAAAAAAAAAAAAAAAAA", "AAAA"]
    } else {
        vec![CODE_BODY]
    };
    assert_eq!(
        body_lines
            .iter()
            .map(|node| node.text().unwrap())
            .collect::<Vec<_>>(),
        expected
    );
    for (index, line) in body_lines.iter().enumerate() {
        close(point(*line).1, 64.001 + index as f64 * 13.5);
    }
}

#[test]
fn detached_code_cap_updates_inline_advance_and_drawing_without_replacing_parent_height() {
    const {
        assert!(18.0 * A_ADVANCE <= 150.0 - 32.0);
        assert!(19.0 * A_ADVANCE > 150.0 - 32.0);
    }
    let face = sdocx::fonts::FontBook::default()
        .resolve("Roboto", false, false)
        .unwrap();
    let mut buffer = sdocx::fonts::UnicodeBuffer::new();
    buffer.push_str("A");
    buffer.set_direction(sdocx::fonts::Direction::LeftToRight);
    let shaped = face.shape(buffer, &[]).unwrap();
    assert_eq!(shaped.advance_x(), 1336);
    assert_eq!(shaped.metrics.units_per_em, 2048);
    for replay in [false, true] {
        assert_geometry(
            &render(&document(ObjectSpanLayoutConstraint::Normal, 150.0), replay),
            300.0,
            true,
        );
        for constraint in [
            ObjectSpanLayoutConstraint::OverPagesOverlapPadding,
            ObjectSpanLayoutConstraint::OverPages,
        ] {
            for maximum_width in [150.0, 200.0, 300.0] {
                assert_geometry(
                    &render(&document(constraint, maximum_width), replay),
                    f64::from(maximum_width),
                    false,
                );
            }
        }
    }
}

#[cfg(feature = "pdf")]
#[path = "support/pdf_geometry.rs"]
mod pdf_geometry;

#[cfg(feature = "pdf")]
#[test]
fn detached_capped_code_exports_selectable_vector_pdf() {
    for constraint in [
        ObjectSpanLayoutConstraint::OverPagesOverlapPadding,
        ObjectSpanLayoutConstraint::OverPages,
    ] {
        let page = render(&document(constraint, 150.0), false);
        let bytes = sdocx::render_svg_pages_pdf(&[page], &Default::default()).unwrap();
        let pdf = pdf_geometry::read(&bytes, 96.0);
        assert_eq!(pdf.image_resources, 0);
        assert!(pdf.images.is_empty());
        assert_eq!(pdf.source.matches(CODE_BODY).count(), 1);
        assert_eq!(
            pdf.extracted_text
                .chars()
                .filter(|&character| character == 'A')
                .count(),
            23
        );
        let b = pdf
            .text
            .iter()
            .find(|(source, _, _)| source == "B")
            .unwrap();
        assert!((b.1 - (A_ADVANCE + 150.0)).abs() < 0.0002);
        assert!((b.2 - 87.501).abs() < 0.0002);
    }
}
