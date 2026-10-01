#![cfg(feature = "render")]

use sdocx::{
    BoundingBox, Document, DocumentMetadata, ObjectSpanLayoutConstraint, ObjectSpanLayoutOption,
    ObjectType, Page, PageElement, RichTextBox, RichTextCodeBlock, RichTextObjectContent,
    RichTextObjectSpan, RichTextSection, RichTextSpan, RichTextSpanType, SpanIntervalType,
};

fn bounds(width: f64, height: f64) -> BoundingBox {
    BoundingBox {
        x_min: 0.0,
        y_min: 0.0,
        x_max: width,
        y_max: height,
    }
}

fn text(source: &str) -> RichTextBox {
    RichTextBox {
        text_area_type: None,
        bbox: bounds(300.0, 200.0),
        rotation_degrees: None,
        text: source.into(),
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
    }
}

fn font_size(start: u32, end: u32, interval: u32, size: f32) -> RichTextSpan {
    RichTextSpan {
        kind: RichTextSpanType::FontSize,
        start_utf16: start,
        end_utf16: end,
        interval_type: SpanIntervalType::from(interval),
        payload: size.to_le_bytes().to_vec(),
    }
}

fn document(mut content: RichTextBox, flow: bool) -> Document {
    if flow {
        content.bbox = BoundingBox::default();
    }
    Document {
        pages: vec![Page {
            uuid: "terminal-paragraphs".into(),
            width: 360,
            height: 400,
            content_bbox: bounds(360.0, 400.0),
            background_color: None,
            template: None,
            background: Default::default(),
            objects: vec![PageElement::TextBox(content).into()],
        }],
        metadata: DocumentMetadata {
            default_page_dimensions: Some((360, 400)),
            orientation: Some(0),
            flow_page_padding: Some((0, 0)),
            ..Default::default()
        },
    }
}

fn code_document(body: RichTextBox) -> Document {
    let mut title = text("T");
    title.font_size = Some(1.0);
    let mut parent = text("\u{fffc}");
    parent.object_spans.push(RichTextObjectSpan {
        object_type: ObjectType::CodeBlock,
        object_data: Vec::new(),
        content: Some(RichTextObjectContent::CodeBlock(Box::new(
            RichTextCodeBlock {
                bbox: bounds(300.0, 200.0),
                rotation_degrees: None,
                title: Some(title),
                body: Some(body),
            },
        ))),
        text_index_utf16: 0,
        layout_option: ObjectSpanLayoutOption::Inline,
        layout_constraint: ObjectSpanLayoutConstraint::Normal,
    });
    document(parent, false)
}

fn render(doc: &Document, page: usize, replay: bool) -> sdocx::RenderedPage {
    let layout = sdocx::layout_document(doc);
    if replay {
        sdocx::render_layout_page_replay_svg(doc, &layout, page, &Default::default()).unwrap()
    } else {
        sdocx::render_layout_page_svg(doc, &layout, page, &Default::default()).unwrap()
    }
}

fn lines(svg: &str) -> Vec<(String, f64, f64)> {
    let xml = roxmltree::Document::parse(svg).unwrap();
    xml.descendants()
        .filter(|node| node.has_tag_name("text"))
        .map(|node| {
            let source = node
                .descendants()
                .filter(|node| node.has_tag_name("tspan"))
                .filter_map(|node| node.text())
                .collect();
            let first = node
                .descendants()
                .find(|node| node.has_tag_name("tspan"))
                .unwrap();
            let coordinate = |attribute| {
                first
                    .attribute(attribute)
                    .or_else(|| node.attribute(attribute))
                    .unwrap()
                    .split_whitespace()
                    .next()
                    .unwrap()
                    .parse::<f64>()
                    .unwrap()
            };
            let mut point = (coordinate("x"), coordinate("y"));
            for ancestor in first.ancestors() {
                if let Some(transform) = ancestor.attribute("transform") {
                    let transform: svgtypes::Transform = transform.parse().unwrap();
                    point = (
                        transform.a * point.0 + transform.c * point.1 + transform.e,
                        transform.b * point.0 + transform.d * point.1 + transform.f,
                    );
                }
            }
            let size: f64 = first.attribute("font-size").unwrap().parse().unwrap();
            (source, point.1, size)
        })
        .collect()
}

fn assert_lines(page: &sdocx::RenderedPage, expected: &[(&str, f64, f64)]) {
    let actual = lines(&page.svg);
    assert_eq!(actual.len(), expected.len(), "{actual:?}");
    for ((source, y, size), &(expected_source, expected_y, expected_size)) in
        actual.iter().zip(expected)
    {
        assert_eq!(source, expected_source);
        assert!((y - expected_y).abs() <= 1e-4, "{actual:?}");
        assert!((size - expected_size).abs() <= 1e-4, "{actual:?}");
    }
    assert!(page.object_diagnostics.is_empty());
}

fn assert_code_height(body: RichTextBox, height: f64, expected: &[(&str, f64, f64)], replay: bool) {
    let page = render(&code_document(body), 0, replay);
    let xml = roxmltree::Document::parse(&page.svg).unwrap();
    let panel = xml
        .descendants()
        .find(|node| node.has_tag_name("rect") && node.attribute("fill") == Some("#efefef"))
        .unwrap();
    assert_eq!(
        panel.attribute("height").unwrap().parse::<f64>().unwrap(),
        height
    );
    let mut child_lines = vec![("T", 13.001, 1.0)];
    child_lines.extend(
        expected
            .iter()
            .map(|&(source, y, size)| (source, y + 44.001, size)),
    );
    assert_lines(&page, &child_lines);
}

#[test]
fn native_terminal_and_crlf_paragraphs_add_measured_lines_without_empty_glyph_nodes() {
    // Native CR/LF paragraph producers process each UTF-16 delimiter separately.
    // Every ordinary F20 paragraph contributes 27px at default multiplier 1.35.
    for (source, height, expected) in [
        ("A\n", 118.0, vec![("A", 20.0, 20.0)]),
        ("\nA", 118.0, vec![("A", 47.0, 20.0)]),
        ("\n", 118.0, vec![]),
        ("\n\n", 145.0, vec![]),
        ("A\r\nB", 145.0, vec![("A", 20.0, 20.0), ("B", 74.0, 20.0)]),
    ] {
        for replay in [false, true] {
            for flow in [false, true] {
                let page = render(&document(text(source), flow), 0, replay);
                assert_lines(&page, &expected);
                assert!(page.text_diagnostics.is_empty());
            }
            // Native fixed code chrome 64 exposes otherwise invisible line heights.
            assert_code_height(text(source), height, &expected, replay);
        }
    }
}

#[test]
fn bottom_gravity_accounts_for_terminal_and_intermediate_empty_paragraphs() {
    for (source, expected) in [
        ("A\n", vec![("A", 166.0, 20.0)]),
        ("\nA", vec![("A", 193.0, 20.0)]),
        ("A\r\nB", vec![("A", 139.0, 20.0), ("B", 193.0, 20.0)]),
    ] {
        let mut content = text(source);
        content.gravity = Some(2);
        for replay in [false, true] {
            assert_lines(
                &render(&document(content.clone(), false), 0, replay),
                &expected,
            );
        }
    }
}

#[test]
fn fresh_entirely_empty_text_has_no_measured_lines_even_with_caret_font_spans() {
    // Native fresh Measure/DoLayout returns before storing height for length 0.
    // Gravity's separate caret size must not become a code body reservation.
    for interval in 0..4 {
        let mut content = text("");
        content.spans.push(font_size(0, 0, interval, 40.0));
        for replay in [false, true] {
            assert_code_height(content.clone(), 64.0, &[], replay);
            let page = render(&document(content.clone(), false), 0, replay);
            assert_lines(&page, &[]);
            assert!(page.text_diagnostics.is_empty());
        }
    }
}

#[test]
fn terminal_caret_uses_native_interval_endpoints_without_styling_glyphs_differently() {
    for (interval, caret_included, height, baseline) in [
        (0, false, 145.0, 159.0),
        (1, true, 172.0, 132.0),
        (2, false, 145.0, 159.0),
        (3, true, 172.0, 132.0),
    ] {
        let mut content = text("A\n");
        let span = font_size(0, 2, interval, 40.0);
        assert_eq!(span.contains_caret(2), caret_included);
        content.spans.push(span);
        for replay in [false, true] {
            assert_code_height(content.clone(), height, &[("A", 40.0, 40.0)], replay);
            content.gravity = Some(2);
            assert_lines(
                &render(&document(content.clone(), false), 0, replay),
                &[("A", baseline, 40.0)],
            );
            content.gravity = None;
        }
    }
}

#[test]
fn zero_length_terminal_font_spans_apply_only_including_native_interval_types() {
    for (interval, included, height) in [
        (0, true, 145.0),
        (1, true, 145.0),
        (2, false, 118.0),
        (3, true, 145.0),
    ] {
        let mut content = text("A\n");
        let span = font_size(2, 2, interval, 40.0);
        assert_eq!(span.contains_caret(2), included);
        content.spans.push(span);
        for replay in [false, true] {
            assert_code_height(content.clone(), height, &[("A", 20.0, 20.0)], replay);
        }
    }
}

#[test]
fn following_content_reads_glyph_style_instead_of_caret_or_previous_separator_style() {
    for (span, height, baseline) in [
        (font_size(0, 1, 0, 40.0), 145.0, 74.0),
        (font_size(1, 1, 1, 40.0), 118.0, 47.0),
    ] {
        let mut content = text("\nA");
        content.spans.push(span);
        for replay in [false, true] {
            assert_code_height(content.clone(), height, &[("A", baseline, 20.0)], replay);
            let page = render(&document(content.clone(), false), 0, replay);
            assert_lines(&page, &[("A", baseline, 20.0)]);
        }
    }
}

#[test]
fn terminal_caret_offset_is_utf16_after_a_supplementary_character() {
    for (interval, height) in [(0, 145.0), (1, 172.0), (2, 145.0), (3, 172.0)] {
        let mut content = text("😀\n");
        content.spans.push(font_size(0, 3, interval, 40.0));
        for replay in [false, true] {
            assert_code_height(content.clone(), height, &[("😀", 40.0, 40.0)], replay);
        }
    }
}

#[test]
fn capture_retains_blank_crlf_and_terminal_pages_without_resurrecting_visible_glyphs() {
    let mut body = text("A\r\nB\n");
    body.bbox = BoundingBox::default();
    body.text_sections = vec![
        RichTextSection {
            start_utf16: 0,
            length_utf16: 5
        };
        4
    ];
    let mut doc = document(text(""), false);
    doc.metadata.note_text = Some(body);
    doc.metadata.default_page_dimensions = Some((360, 50));
    doc.metadata.page_mode = Some(0);
    doc.metadata.flow_page_padding = Some((0, 10));
    doc.pages[0].height = 50;
    doc.pages[0].content_bbox = bounds(360.0, 50.0);
    doc.pages[0].objects.clear();
    for index in 1..4 {
        let mut page = doc.pages[0].clone();
        page.uuid = format!("terminal-capture-{index}");
        doc.pages.push(page);
    }
    for replay in [false, true] {
        for (index, expected) in [
            (3, vec![]),
            (2, vec![("B", 30.0, 20.0)]),
            (1, vec![]),
            (0, vec![("A", 30.0, 20.0)]),
        ] {
            let page = render(&doc, index, replay);
            assert_lines(&page, &expected);
            assert!(page.text_diagnostics.is_empty());
            assert_eq!(render(&doc, index, replay), page);
        }
    }
}

#[cfg(feature = "pdf")]
#[test]
fn terminal_paragraph_exports_remain_selectable_vector_text() {
    let mut content = text("A\r\nB\n");
    content.gravity = Some(2);
    for replay in [false, true] {
        let page = render(&document(content.clone(), false), 0, replay);
        assert_lines(&page, &[("A", 112.0, 20.0), ("B", 166.0, 20.0)]);
        let bytes = sdocx::render_svg_pages_pdf(&[page], &Default::default()).unwrap();
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        assert_eq!(
            pdf.extract_text(&[1]).unwrap().replace(['\n', ' '], ""),
            "AB"
        );
        assert!(
            !pdf.objects
                .values()
                .any(|object| object.as_stream().is_ok_and(|stream| stream
                    .dict
                    .get(b"Subtype")
                    .is_ok_and(|value| value.as_name().is_ok_and(|name| name == b"Image"))))
        );
        assert!(pdf.objects.values().any(|object| {
            object
                .as_dict()
                .is_ok_and(|dict| dict.has(b"FontFile2") || dict.has(b"FontFile3"))
        }));
    }
}
