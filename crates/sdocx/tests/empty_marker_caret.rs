#![cfg(feature = "render")]

use sdocx::{
    BoundingBox, Document, DocumentMetadata, Page, PageElement, RenderedPage, RichTextBox,
    RichTextParagraph, RichTextParagraphType, RichTextSpan, RichTextSpanType, SpanIntervalType,
};

#[cfg(feature = "pdf")]
#[path = "support/pdf_geometry.rs"]
mod pdf_geometry;

fn document(source: &str, start: u32, end: u32, interval: u32, right_aligned: bool) -> Document {
    let ordinal = source
        .chars()
        .filter(|character| matches!(character, '\r' | '\n'))
        .count() as u32;
    let mut paragraphs = vec![RichTextParagraph {
        kind: RichTextParagraphType::Bullet,
        start_paragraph: ordinal,
        end_paragraph: ordinal + 1,
        payload: [4_u32, 1, 0, 1]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect(),
    }];
    if right_aligned {
        paragraphs.push(RichTextParagraph {
            kind: RichTextParagraphType::Alignment,
            start_paragraph: ordinal,
            end_paragraph: ordinal + 1,
            payload: 1_u32.to_le_bytes().into(),
        });
    }
    let bounds = BoundingBox {
        x_min: 20.0,
        y_min: 20.0,
        x_max: 300.0,
        y_max: 350.0,
    };
    let content = RichTextBox {
        text_area_type: None,
        bbox: bounds,
        rotation_degrees: None,
        text: source.into(),
        color: None,
        highlight_color: None,
        underline: false,
        font_size: Some(17.0),
        runs: Vec::new(),
        spans: vec![RichTextSpan {
            kind: RichTextSpanType::FontSize,
            start_utf16: start,
            end_utf16: end,
            interval_type: SpanIntervalType::from(interval),
            payload: 100.0_f32.to_le_bytes().to_vec(),
        }],
        paragraphs,
        object_spans: Vec::new(),
        text_sections: Vec::new(),
        margins: None,
        gravity: None,
    };
    Document {
        metadata: DocumentMetadata {
            default_page_dimensions: Some((360, 500)),
            orientation: Some(0),
            ..Default::default()
        },
        pages: vec![Page {
            uuid: "empty-marker-caret".into(),
            width: 360,
            height: 500,
            content_bbox: bounds,
            background_color: None,
            template: None,
            background: Default::default(),
            objects: vec![PageElement::TextBox(content).into()],
        }],
    }
}

fn modes(document: &Document) -> [RenderedPage; 2] {
    let layout = sdocx::layout_document(document);
    [
        sdocx::render_layout_page_svg(document, &layout, 0, &Default::default()).unwrap(),
        sdocx::render_layout_page_replay_svg(document, &layout, 0, &Default::default()).unwrap(),
    ]
}

fn marker(page: &RenderedPage) -> (f64, Vec<f64>, (f64, f64)) {
    assert!(
        page.text_diagnostics.is_empty(),
        "{:?}",
        page.text_diagnostics
    );
    assert!(page.object_diagnostics.is_empty());
    let xml = roxmltree::Document::parse(&page.svg).unwrap();
    let node = xml
        .descendants()
        .find(|node| node.has_tag_name("tspan") && node.text() == Some("1."))
        .unwrap();
    let size = node.attribute("font-size").unwrap().parse().unwrap();
    let xs: Vec<f64> = node
        .attribute("x")
        .unwrap()
        .split_whitespace()
        .map(|x| x.parse().unwrap())
        .collect();
    let mut position = (xs[0], node.attribute("y").unwrap().parse::<f64>().unwrap());
    for ancestor in node.ancestors() {
        if let Some(transform) = ancestor.attribute("transform") {
            let transform: svgtypes::Transform = transform.parse().unwrap();
            position = (
                transform.a * position.0 + transform.c * position.1 + transform.e,
                transform.b * position.0 + transform.d * position.1 + transform.f,
            );
        }
    }
    (size, xs, position)
}

fn advances(size: f64) -> Vec<f64> {
    let face = rustybuzz::Face::from_slice(include_bytes!("../assets/fonts/Roboto-Regular.ttf"), 0)
        .unwrap();
    assert_eq!(face.units_per_em(), 2048);
    let mut buffer = rustybuzz::UnicodeBuffer::new();
    buffer.push_str("1.");
    buffer.guess_segment_properties();
    rustybuzz::shape(&face, &[], buffer)
        .glyph_positions()
        .iter()
        .map(|position| f64::from(position.x_advance) * size / 2048.0)
        .collect()
}

fn close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() < tolerance,
        "actual {actual}, expected {expected}"
    );
}

#[test]
fn terminal_empty_numeric_markers_follow_caret_endpoint_policies() {
    for (start, end, interval, expected_size) in [
        (2, 2, 0, 100.0),
        (2, 2, 1, 100.0),
        (2, 2, 2, 17.0),
        (2, 2, 3, 100.0),
        (2, 2, 99, 100.0),
        (1, 2, 0, 17.0),
        (1, 2, 1, 100.0),
        (1, 2, 3, 100.0),
    ] {
        for page in modes(&document("X\n", start, end, interval, false)) {
            let (size, xs, position) = marker(&page);
            assert_eq!(
                size, expected_size,
                "span [{start},{end}] raw interval {interval}"
            );
            close(xs[1] - xs[0], advances(size)[0], 0.00001);
            assert_eq!(position.0, 20.0);
            let xml = roxmltree::Document::parse(&page.svg).unwrap();
            let first = xml
                .descendants()
                .find(|node| node.text() == Some("X") && node.has_tag_name("tspan"))
                .unwrap();
            assert_eq!(first.attribute("font-size"), Some("17.00"));
        }
    }
}

#[test]
fn empty_numeric_marker_reservation_uses_the_caret_font_advance_and_native_gap() {
    for (source, caret) in [("X\n", 2), ("Ж\n", 2), ("X\r\n", 3)] {
        for interval in [0, 2] {
            let expected_size = if interval == 0 { 100.0 } else { 17.0 };
            for page in modes(&document(source, caret, caret, interval, true)) {
                let (size, _, position) = marker(&page);
                assert_eq!(size, expected_size);
                let reserved_width = advances(size).iter().sum::<f64>() + 9.0;
                close(position.0, 300.0 - reserved_width, 0.00001);
            }
        }
    }
}

#[test]
fn zero_length_caret_font_does_not_override_a_nonempty_numeric_paragraph() {
    for page in modes(&document("X\nA", 2, 2, 0, false)) {
        assert_eq!(marker(&page).0, 17.0);
        let xml = roxmltree::Document::parse(&page.svg).unwrap();
        let body = xml
            .descendants()
            .find(|node| node.has_tag_name("tspan") && node.text() == Some("A"))
            .unwrap();
        assert_eq!(body.attribute("font-size"), Some("17.00"));
    }
}

#[cfg(feature = "pdf")]
#[test]
fn caret_sized_empty_numeric_markers_remain_selectable_vector_pdf_text() {
    for page in modes(&document("X\n", 2, 2, 0, false)) {
        let (_, xs, expected) = marker(&page);
        let bytes = sdocx::render_svg_pages_pdf(&[page], &Default::default()).unwrap();
        let geometry = pdf_geometry::read(&bytes, 96.0);
        assert_eq!(geometry.source, "X1.");
        assert_eq!(geometry.extracted_text.replace(['\n', ' '], ""), "X1.");
        assert_eq!(geometry.image_resources, 0);
        let digit = geometry
            .text
            .iter()
            .find(|(source, _, _)| source == "1")
            .unwrap();
        let dot = geometry
            .text
            .iter()
            .find(|(source, _, _)| source == ".")
            .unwrap();
        close(digit.1, expected.0, 0.0001);
        close(digit.2, expected.1, 0.0001);
        close(dot.1 - digit.1, xs[1] - xs[0], 0.0001);
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        let content =
            lopdf::content::Content::decode(&pdf.get_page_content(pdf.get_pages()[&1]).unwrap())
                .unwrap();
        let sizes: Vec<f32> = content
            .operations
            .iter()
            .filter(|operation| operation.operator == "Tf")
            .map(|operation| operation.operands[1].as_float().unwrap())
            .collect();
        assert!(sizes.contains(&17.0));
        assert!(sizes.contains(&100.0));
    }
}
