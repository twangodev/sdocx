#![cfg(feature = "render")]

use sdocx::{
    BoundingBox, Document, DocumentMetadata, Page, PageElement, RichTextBox, RichTextParagraph,
    RichTextParagraphType, RichTextSpan, RichTextSpanType,
};

fn document(kind: u32, number: u32, initial: u32, width: u32) -> Document {
    let content = RichTextBox {
        text_area_type: None,
        bbox: BoundingBox::default(),
        rotation_degrees: None,
        text: "item".into(),
        color: None,
        highlight_color: None,
        underline: false,
        font_size: Some(15.0),
        runs: Vec::new(),
        spans: Vec::new(),
        paragraphs: vec![RichTextParagraph {
            kind: RichTextParagraphType::Bullet,
            start_paragraph: 0,
            end_paragraph: 1,
            payload: [kind, number, 0, initial]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect(),
        }],
        object_spans: Vec::new(),
        text_sections: Vec::new(),
        margins: None,
        gravity: None,
    };
    Document {
        pages: vec![Page {
            uuid: "numbered-list-markers".into(),
            width: 1080,
            height: 1527,
            content_bbox: BoundingBox::default(),
            background_color: None,
            template: None,
            background: Default::default(),
            objects: vec![PageElement::TextBox(content).into()],
        }],
        metadata: DocumentMetadata {
            default_page_dimensions: Some((width, 1527)),
            orientation: Some(0),
            flow_page_padding: Some((10, 0)),
            ..Default::default()
        },
    }
}

fn content(doc: &mut Document) -> &mut RichTextBox {
    let sdocx::PageObjectContent::Element(PageElement::TextBox(content)) =
        &mut doc.pages[0].objects[0].content
    else {
        panic!()
    };
    content
}

fn modes(doc: &Document) -> [sdocx::RenderedPage; 2] {
    let layout = sdocx::layout_document(doc);
    [
        sdocx::render_layout_page_svg(doc, &layout, 0, &Default::default()).unwrap(),
        sdocx::render_layout_page_replay_svg(doc, &layout, 0, &Default::default()).unwrap(),
    ]
}

fn node_source(node: roxmltree::Node<'_, '_>) -> String {
    node.descendants()
        .filter(|node| node.has_tag_name("tspan"))
        .filter_map(|node| node.text())
        .collect()
}

fn source(svg: &str) -> String {
    node_source(roxmltree::Document::parse(svg).unwrap().root_element())
}

fn position(node: roxmltree::Node<'_, '_>) -> (f64, f64) {
    let value = |name| {
        node.attribute(name)
            .or_else(|| node.parent().unwrap().attribute(name))
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .parse::<f64>()
            .unwrap()
    };
    let mut point = (value("x"), value("y"));
    for node in node.ancestors() {
        if let Some(transform) = node.attribute("transform") {
            let transform: svgtypes::Transform = transform.parse().unwrap();
            point = (
                transform.a * point.0 + transform.c * point.1 + transform.e,
                transform.b * point.0 + transform.d * point.1 + transform.f,
            );
        }
    }
    point
}

fn assert_coordinate(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-8,
        "actual{actual}, expected{expected}"
    );
}

fn svg_decimal(value: f64) -> f64 {
    (value * 100_000.0).round() / 100_000.0
}

fn pinned_marker_advances(value: &str, size: f64) -> Vec<f64> {
    let face = rustybuzz::Face::from_slice(include_bytes!("../assets/fonts/Roboto-Regular.ttf"), 0)
        .unwrap();
    assert_eq!(face.units_per_em(), 2048);
    let mut buffer = rustybuzz::UnicodeBuffer::new();
    buffer.push_str(value);
    buffer.guess_segment_properties();
    let shaped = rustybuzz::shape(&face, &[], buffer);
    assert!(shaped.glyph_infos().iter().all(|glyph| glyph.glyph_id != 0));
    shaped
        .glyph_positions()
        .iter()
        .map(|glyph| f64::from(glyph.x_advance) * size / 2048.0)
        .collect()
}

#[test]
fn numbered_marker_strings_follow_native_start_rollover_and_conversion_rules() {
    for (kind, number, initial, marker) in [
        (4, 1, 1, "1."),
        (5, 1, 1, "1."),
        (4, 2, 5, "6."),
        (4, 0, 1, "."),
        (4, 0x8000_0000, 1, "."),
        (4, u32::MAX, 2, "."),
        (4, 2, u32::MAX, "."),
        (6, 27, 1, "aa."),
        (10, 27, 1, "AA."),
        (7, 3999, 1, "mmmcmxcix."),
        (7, 4000, 1, "mmmm."),
    ] {
        for page in modes(&document(kind, number, initial, 360)) {
            assert_eq!(source(&page.svg), format!("{marker}item"));
            let xml = roxmltree::Document::parse(&page.svg).unwrap();
            assert!(
                !xml.descendants()
                    .any(|node| node.has_tag_name("circle") || node.has_tag_name("ellipse"))
            );
            assert!(
                page.text_diagnostics.is_empty(),
                "{:?}",
                page.text_diagnostics
            );
            assert!(page.object_diagnostics.is_empty());
        }
    }
}

#[test]
fn numeric_reservations_and_retained_glyphs_use_pinned_default_face_metrics() {
    for (width, size, gap_scale, marker_y) in [
        (360, 15.0, 1.0, 14.625),
        (720, 30.0, 2.0, 29.25),
        (1080, 45.0, 3.0, 43.875),
    ] {
        for (kind, number, initial, marker, logical_gap) in [
            (4, 1, 1, "1.", 9.0),
            (5, 9, 1, "9.", 9.0),
            (4, 10, 1, "10.", 6.0),
            (4, 2, 9, "10.", 6.0),
            (6, 27, 1, "aa.", 6.0),
        ] {
            let advances = pinned_marker_advances(marker, size);
            let expected_body_x =
                svg_decimal(10.0 + advances.iter().sum::<f64>() + logical_gap * gap_scale);
            for page in modes(&document(kind, number, initial, width)) {
                let xml = roxmltree::Document::parse(&page.svg).unwrap();
                let marker_node = xml
                    .descendants()
                    .find(|node| node.has_tag_name("text") && node_source(*node) == marker)
                    .unwrap();
                let span = marker_node
                    .descendants()
                    .find(|node| node.has_tag_name("tspan"))
                    .unwrap();
                assert_coordinate(position(span).0, 10.0);
                assert_coordinate(position(span).1, marker_y);
                assert_eq!(
                    span.attribute("font-size")
                        .or_else(|| marker_node.attribute("font-size"))
                        .unwrap()
                        .parse::<f64>()
                        .unwrap(),
                    size
                );
                let xs: Vec<f64> = span
                    .attribute("x")
                    .unwrap()
                    .split_whitespace()
                    .map(|x| x.parse().unwrap())
                    .collect();
                assert_eq!(xs.len(), marker.chars().count());
                let mut pen = 0.0;
                for (x, advance) in xs.into_iter().zip(&advances) {
                    let local_origin = span
                        .attribute("x")
                        .unwrap()
                        .split_whitespace()
                        .next()
                        .unwrap()
                        .parse::<f64>()
                        .unwrap();
                    assert_coordinate(x - local_origin, svg_decimal(pen));
                    pen += advance;
                }
                let body = xml
                    .descendants()
                    .find(|node| node.has_tag_name("tspan") && node.text() == Some("item"))
                    .unwrap();
                assert_coordinate(position(body).0, expected_body_x);
                assert_coordinate(position(body).1, size);
            }
        }
    }
}

fn font_span(kind: RichTextSpanType, start: u32, end: u32, payload: Vec<u8>) -> RichTextSpan {
    RichTextSpan {
        kind,
        start_utf16: start,
        end_utf16: end,
        expand: true,
        payload,
    }
}

#[test]
fn markers_use_first_content_size_with_one_delta_and_ignore_parent_face_styles() {
    for (width, marker_size, body_size, marker_y) in [
        (360, 12.0, 22.0, 18.45),
        (720, 24.0, 44.0, 36.9),
        (1080, 36.0, 66.0, 55.35),
    ] {
        let mut doc = document(4, 1, 1, width);
        doc.metadata.body_font_size_delta = Some(2);
        let text = content(&mut doc);
        text.text = "AB".into();
        text.spans = vec![
            font_span(
                RichTextSpanType::FontSize,
                0,
                1,
                10_f32.to_le_bytes().to_vec(),
            ),
            font_span(
                RichTextSpanType::FontSize,
                1,
                2,
                20_f32.to_le_bytes().to_vec(),
            ),
            font_span(
                RichTextSpanType::FontName,
                0,
                2,
                [
                    vec![0; 8],
                    12_u16.to_le_bytes().to_vec(),
                    b"Roboto Mono\0".to_vec(),
                ]
                .concat(),
            ),
            font_span(RichTextSpanType::Bold, 0, 2, vec![1, 0]),
            font_span(RichTextSpanType::Italic, 0, 2, vec![1, 0]),
        ];
        let expected_body_x = svg_decimal(
            10.0 + pinned_marker_advances("1.", marker_size)
                .iter()
                .sum::<f64>()
                + 9.0 * f64::from(width / 360),
        );
        for page in modes(&doc) {
            let xml = roxmltree::Document::parse(&page.svg).unwrap();
            let marker = xml
                .descendants()
                .find(|node| node.has_tag_name("text") && node_source(*node) == "1.")
                .unwrap();
            let span = marker
                .descendants()
                .find(|node| node.has_tag_name("tspan"))
                .unwrap();
            assert_eq!(
                span.attribute("font-size")
                    .or_else(|| marker.attribute("font-size"))
                    .unwrap()
                    .parse::<f64>()
                    .unwrap(),
                marker_size
            );
            assert_coordinate(position(span).1, marker_y);
            let family = span
                .ancestors()
                .find_map(|node| node.attribute("font-family"))
                .unwrap();
            let families = svgtypes::parse_font_families(family).unwrap();
            assert_eq!(
                families.first(),
                Some(&svgtypes::FontFamily::Named("Roboto".into()))
            );
            assert!(!families.contains(&svgtypes::FontFamily::Named("Roboto Mono".into())));
            assert!(span.ancestors().all(|node| {
                node.attribute("font-style")
                    .is_none_or(|value| value == "normal")
                    && node
                        .attribute("font-weight")
                        .is_none_or(|value| matches!(value, "400" | "normal"))
            }));
            let first = xml
                .descendants()
                .find(|node| node.has_tag_name("tspan") && node.text() == Some("A"))
                .unwrap();
            assert_coordinate(position(first).0, expected_body_x);
            assert_coordinate(position(first).1, body_size);
            assert_eq!(source(&page.svg), "1.AB");
        }
    }
}

#[test]
fn oversized_roman_markers_report_measurement_failure_without_losing_body() {
    for page in modes(&document(7, i32::MAX as u32, 1, 360)) {
        assert_eq!(source(&page.svg), "item");
        assert_eq!(page.text_diagnostics.len(), 1);
        assert_eq!(
            page.text_diagnostics[0].kind,
            sdocx::TextDiagnosticKind::MeasurementFailure
        );
        assert!(page.object_diagnostics.is_empty());
    }
}

#[test]
fn marker_size_skips_the_retained_linefeed_and_uses_the_first_content_span() {
    let mut doc = document(4, 1, 1, 360);
    doc.metadata.body_font_size_delta = Some(2);
    let text = content(&mut doc);
    text.text = "\nAB".into();
    text.paragraphs[0].start_paragraph = 1;
    text.paragraphs[0].end_paragraph = 2;
    text.spans = vec![
        font_span(
            RichTextSpanType::FontSize,
            0,
            1,
            40_f32.to_le_bytes().to_vec(),
        ),
        font_span(
            RichTextSpanType::FontSize,
            1,
            2,
            10_f32.to_le_bytes().to_vec(),
        ),
        font_span(
            RichTextSpanType::FontSize,
            2,
            3,
            20_f32.to_le_bytes().to_vec(),
        ),
    ];
    for page in modes(&doc) {
        let xml = roxmltree::Document::parse(&page.svg).unwrap();
        let marker = xml
            .descendants()
            .find(|node| node.has_tag_name("text") && node_source(*node) == "1.")
            .unwrap();
        let span = marker
            .descendants()
            .find(|node| node.has_tag_name("tspan"))
            .unwrap();
        assert_eq!(
            span.attribute("font-size")
                .or_else(|| marker.attribute("font-size"))
                .unwrap()
                .parse::<f64>()
                .unwrap(),
            12.0
        );
        assert_eq!(source(&page.svg), "1.AB");
        assert!(page.text_diagnostics.is_empty());
    }
}

#[test]
fn fractional_marker_width_is_ceiled_for_its_child_and_reserved_before_body_wrapping() {
    let mut doc = document(4, 1, 1, 360);
    doc.pages[0].width = 82;
    content(&mut doc).text = "AAAA".into();
    content(&mut doc).font_size = Some(20.0);
    let advance = pinned_marker_advances("1.", 20.0).iter().sum::<f64>();
    assert_ne!(advance, advance.floor());
    let body_x = svg_decimal(10.0 + advance + 9.0);
    for page in modes(&doc) {
        let xml = roxmltree::Document::parse(&page.svg).unwrap();
        let lines: Vec<_> = xml
            .descendants()
            .filter(|node| node.has_tag_name("text"))
            .map(|node| node_source(node))
            .collect();
        assert_eq!(lines, ["1.", "AA", "AA"]);
        let body: Vec<_> = xml
            .descendants()
            .filter(|node| node.has_tag_name("tspan") && node.text() == Some("AA"))
            .collect();
        for (span, y) in body.into_iter().zip([20.0, 47.0]) {
            assert_coordinate(position(span).0, body_x);
            assert_coordinate(position(span).1, y);
        }
        assert!(page.text_diagnostics.is_empty());
    }
}

#[test]
fn numeric_children_center_on_mixed_object_lines_and_explicit_pixel_spacing() {
    for (pixels, body_y, marker_y) in [(0.0_f32, 100.001, 45.501), (4.0, 97.001, 67.45413)] {
        let mut doc = document(4, 1, 1, 360);
        let text = content(&mut doc);
        text.text = "A\u{fffc}B".into();
        text.font_size = Some(20.0);
        text.paragraphs.push(RichTextParagraph {
            kind: RichTextParagraphType::LineSpacing,
            start_paragraph: 0,
            end_paragraph: 1,
            payload: [0_u32.to_le_bytes().to_vec(), pixels.to_le_bytes().to_vec()].concat(),
        });
        text.object_spans.push(sdocx::RichTextObjectSpan {
            object_type: sdocx::ObjectType::CodeBlock,
            object_data: Vec::new(),
            content: Some(sdocx::RichTextObjectContent::CodeBlock(Box::new(
                sdocx::RichTextCodeBlock {
                    bbox: BoundingBox {
                        x_min: 0.0,
                        y_min: 0.0,
                        x_max: 120.0,
                        y_max: 100.0,
                    },
                    rotation_degrees: None,
                    title: None,
                    body: None,
                },
            ))),
            text_index_utf16: 1,
            layout_option: sdocx::ObjectSpanLayoutOption::Inline,
            layout_constraint: sdocx::ObjectSpanLayoutConstraint::Normal,
        });
        for page in modes(&doc) {
            let xml = roxmltree::Document::parse(&page.svg).unwrap();
            let marker = xml
                .descendants()
                .find(|node| node.has_tag_name("text") && node_source(*node) == "1.")
                .unwrap();
            assert_coordinate(
                position(
                    marker
                        .descendants()
                        .find(|node| node.has_tag_name("tspan"))
                        .unwrap(),
                )
                .1,
                marker_y,
            );
            for span in xml
                .descendants()
                .filter(|node| node.has_tag_name("tspan") && matches!(node.text(), Some("A" | "B")))
            {
                assert_coordinate(position(span).1, body_y);
            }
            assert_eq!(source(&page.svg), "1.AB");
            assert!(page.object_diagnostics.is_empty());
            assert!(
                xml.descendants()
                    .any(|node| node.attribute("data-sdocx-object") == Some("code-block"))
            );
        }
    }
}

#[cfg(feature = "pdf")]
#[test]
fn numbered_exports_keep_marker_and_body_selectable_without_raster_objects() {
    for (kind, number, marker) in [
        (4, 1, "1."),
        (5, 10, "10."),
        (6, 27, "aa."),
        (7, 3999, "mmmcmxcix."),
    ] {
        let doc = document(kind, number, 1, 1080);
        let page = modes(&doc).into_iter().next().unwrap();
        let bytes = sdocx::render_svg_pages_pdf(&[page], &Default::default()).unwrap();
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        assert_eq!(
            pdf.extract_text(&[1]).unwrap().replace(['\n', ' '], ""),
            format!("{marker}item")
        );
        assert!(
            !pdf.objects
                .values()
                .any(|object| object.as_stream().is_ok_and(|stream| stream
                    .dict
                    .get(b"Subtype")
                    .is_ok_and(|subtype| subtype.as_name().is_ok_and(|name| name == b"Image"))))
        );
    }
}
