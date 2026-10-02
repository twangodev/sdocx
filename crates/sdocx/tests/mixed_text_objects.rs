#![cfg(all(feature = "render", feature = "serde"))]

use base64::Engine;
use sdocx::{
    BoundingBox, Color, Document, DocumentMetadata, MediaAsset, ObjectSpanLayoutConstraint,
    ObjectSpanLayoutOption, ObjectType, Page, PageElement, PlacedImage, RenderedPage, RichTextBox,
    RichTextCodeBlock, RichTextObjectContent, RichTextObjectSpan, RichTextSpan, RichTextSpanType,
};

fn bounds(left: f64, top: f64, width: f64, height: f64) -> BoundingBox {
    BoundingBox {
        x_min: left,
        y_min: top,
        x_max: left + width,
        y_max: top + height,
    }
}

fn text(value: &str) -> RichTextBox {
    RichTextBox {
        text_area_type: None,
        bbox: BoundingBox::default(),
        rotation_degrees: None,
        text: value.into(),
        color: Some(Color { r: 0, g: 0, b: 0 }),
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

fn image(
    anchor: i32,
    width: f64,
    height: f64,
    option: ObjectSpanLayoutOption,
) -> RichTextObjectSpan {
    RichTextObjectSpan {
        object_type: ObjectType::Image,
        object_data: Vec::new(),
        content: Some(RichTextObjectContent::Image(Box::new(
            serde_json::from_value::<PlacedImage>(
                serde_json::json!({ "bbox": bounds(0.0, 0.0, width, height),
                "rotation_degrees": null, "media_id": null, "media_index": 0,
                "crop_rect": null, "original_bbox": null, "border_media_id": null,
                "original_media_id": null }),
            )
            .unwrap(),
        ))),
        text_index_utf16: anchor,
        layout_option: option,
        layout_constraint: ObjectSpanLayoutConstraint::Normal,
    }
}

fn mixed() -> RichTextBox {
    let mut content = text("A\u{fffc}B");
    content
        .object_spans
        .push(image(1, 30.0, 100.0, ObjectSpanLayoutOption::Inline));
    content
}

fn document(element: PageElement) -> Document {
    Document {
        pages: vec![Page {
            uuid: "mixed-objects".into(), width: 360, height: 800,
            content_bbox: bounds(0.0, 0.0, 360.0, 800.0),
            background_color: Some(Color { r: 255, g: 255, b: 255 }),
            template: None, background: Default::default(), objects: vec![element.into()],
        }],
        metadata: DocumentMetadata {
            default_page_dimensions: Some((360, 800)), orientation: Some(0),
            flow_page_padding: Some((10, 0)), media_assets: vec![MediaAsset {
                name: "media/mixed.png".into(), archive_id: None, mime_type: "image/png".into(),
                data: base64::engine::general_purpose::STANDARD.decode(
                    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC"
                ).unwrap(),
            }], ..Default::default()
        },
    }
}

fn modes(doc: &Document) -> [RenderedPage; 2] {
    let layout = sdocx::layout_document(doc);
    [
        sdocx::render_layout_page_svg(doc, &layout, 0, &Default::default()).unwrap(),
        sdocx::render_layout_page_replay_svg(doc, &layout, 0, &Default::default()).unwrap(),
    ]
}

fn selectable(svg: &str) -> String {
    let xml = roxmltree::Document::parse(svg).unwrap();
    xml.descendants()
        .filter(|node| node.has_tag_name("tspan"))
        .filter_map(|node| node.text())
        .collect()
}

fn span<'a>(xml: &'a roxmltree::Document<'a>, value: &str) -> roxmltree::Node<'a, 'a> {
    xml.descendants()
        .find(|node| node.has_tag_name("tspan") && node.text() == Some(value))
        .unwrap_or_else(|| panic!("missing {value:?}"))
}

fn position(node: roxmltree::Node<'_, '_>, attribute: &str) -> f64 {
    node.attribute(attribute)
        .or_else(|| node.parent().unwrap().attribute(attribute))
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap()
}

fn image_position(node: roxmltree::Node<'_, '_>) -> (f64, f64) {
    let mut point = (position(node, "x"), position(node, "y"));
    for ancestor in node.ancestors() {
        if let Some(value) = ancestor.attribute("transform") {
            let transform: svgtypes::Transform = value.parse().unwrap();
            point = (
                transform.a * point.0 + transform.c * point.1 + transform.e,
                transform.b * point.0 + transform.d * point.1 + transform.f,
            );
        }
    }
    point
}

#[test]
fn inline_image_uses_pinned_neighbor_advances_and_native_mixed_baseline() {
    // Pinned Roboto20: A=1336/2048*20=13.046875, B=1276/2048*20=12.4609375.
    for page in modes(&document(PageElement::TextBox(mixed()))) {
        assert!(page.object_diagnostics.is_empty());
        assert_eq!(selectable(&page.svg), "AB");
        let xml = roxmltree::Document::parse(&page.svg).unwrap();
        assert_eq!(position(span(&xml, "A"), "x"), 10.0);
        assert_eq!(position(span(&xml, "B"), "x"), 61.04688);
        assert_eq!(position(span(&xml, "A"), "y"), 100.001);
        assert_eq!(position(span(&xml, "B"), "y"), 100.001);
        let image = xml
            .descendants()
            .find(|node| node.has_tag_name("image"))
            .unwrap();
        assert_eq!(image_position(image), (27.046875, 0.001));
        assert_eq!(image.attribute("width"), Some("30.00"));
        assert_eq!(image.attribute("height"), Some("100.00"));
    }
}

#[test]
fn wrapping_accounts_for_the_object_width_and_preserves_text_neighbors() {
    for (page_width, suffix_baseline) in [(84, 100.001), (83, 127.00101)] {
        let mut doc = document(PageElement::TextBox(mixed()));
        doc.pages[0].width = page_width;
        for page in modes(&doc) {
            assert_eq!(selectable(&page.svg), "AB");
            let xml = roxmltree::Document::parse(&page.svg).unwrap();
            assert_eq!(position(span(&xml, "A"), "y"), 100.001);
            assert_eq!(position(span(&xml, "B"), "y"), suffix_baseline);
            assert_eq!(
                xml.descendants()
                    .filter(|node| node.has_tag_name("image"))
                    .count(),
                1
            );
        }
    }
}

#[test]
fn block_objects_force_separate_lines_and_apply_native_symmetric_margin_units() {
    let mut output = Vec::new();
    for option in [
        ObjectSpanLayoutOption::BlockWithSmallMargin,
        ObjectSpanLayoutOption::BlockWithMediumMargin,
    ] {
        let mut content = mixed();
        content.object_spans[0].layout_option = option;
        let page = modes(&document(PageElement::TextBox(content)))
            .into_iter()
            .next()
            .unwrap();
        assert_eq!(selectable(&page.svg), "AB");
        let xml = roxmltree::Document::parse(&page.svg).unwrap();
        let image = xml
            .descendants()
            .find(|node| node.has_tag_name("image"))
            .unwrap();
        let (image_left, image_top) = image_position(image);
        let prefix_y = position(span(&xml, "A"), "y");
        let suffix_y = position(span(&xml, "B"), "y");
        assert!(prefix_y < image_top);
        assert!(image_top + 100.0 < suffix_y);
        assert_eq!(image_left, 10.0);
        assert_eq!(position(span(&xml, "B"), "x"), 10.0);
        output.push((image_top, suffix_y));
    }
    assert_eq!(output[1].0 - output[0].0, 10.0);
    assert_eq!(output[1].1 - output[0].1, 20.0);
}

#[test]
fn anchor_only_styles_do_not_inflate_text_or_draw_spurious_decorations() {
    let mut content = mixed();
    content.spans = vec![
        RichTextSpan {
            kind: RichTextSpanType::FontSize,
            start_utf16: 1,
            end_utf16: 2,
            interval_type: sdocx::SpanIntervalType::from(1),
            payload: 900.0_f32.to_le_bytes().to_vec(),
        },
        RichTextSpan {
            kind: RichTextSpanType::Underline,
            start_utf16: 1,
            end_utf16: 2,
            interval_type: sdocx::SpanIntervalType::from(1),
            payload: vec![1, 0],
        },
        RichTextSpan {
            kind: RichTextSpanType::Strikethrough,
            start_utf16: 1,
            end_utf16: 2,
            interval_type: sdocx::SpanIntervalType::from(1),
            payload: vec![1, 0],
        },
        RichTextSpan {
            kind: RichTextSpanType::ForegroundColor,
            start_utf16: 2,
            end_utf16: 3,
            interval_type: sdocx::SpanIntervalType::from(1),
            payload: vec![0, 0, 255, 255],
        },
    ];
    for page in modes(&document(PageElement::TextBox(content.clone()))) {
        assert_eq!(selectable(&page.svg), "AB");
        let xml = roxmltree::Document::parse(&page.svg).unwrap();
        for value in ["A", "B"] {
            let node = span(&xml, value);
            assert_eq!(node.attribute("font-size"), Some("20.00"));
            assert_eq!(position(node, "y"), 100.00104);
        }
        assert_eq!(span(&xml, "B").attribute("fill"), Some("#ff0000"));
        assert!(
            !xml.descendants()
                .any(|node| node.has_tag_name("rect") && node.attribute("fill") == Some("#000000"))
        );
    }
}

#[test]
fn unsupported_script_positioning_and_supplementary_prefixes_keep_objects_and_source() {
    for (source, anchor, expected) in [("Ωאב\u{fffc}β", 3, "Ωאבβ"), ("😀A\u{fffc}B", 3, "😀AB")]
    {
        let mut content = text(source);
        content
            .object_spans
            .push(image(anchor, 30.0, 100.0, ObjectSpanLayoutOption::Inline));
        for page in modes(&document(PageElement::TextBox(content.clone()))) {
            assert!(page.object_diagnostics.is_empty());
            assert_eq!(selectable(&page.svg), expected);
            let xml = roxmltree::Document::parse(&page.svg).unwrap();
            assert_eq!(
                xml.descendants()
                    .filter(|node| node.has_tag_name("image"))
                    .count(),
                1
            );
        }
    }
}

#[test]
fn last_stored_duplicate_selects_its_own_geometry_without_consuming_neighbor_text() {
    let mut content = mixed();
    content
        .object_spans
        .push(image(1, 60.0, 100.0, ObjectSpanLayoutOption::Inline));
    for page in modes(&document(PageElement::TextBox(content.clone()))) {
        assert_eq!(selectable(&page.svg), "AB");
        assert!(page.object_diagnostics.is_empty());
        let xml = roxmltree::Document::parse(&page.svg).unwrap();
        let images = xml
            .descendants()
            .filter(|node| node.has_tag_name("image"))
            .collect::<Vec<_>>();
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].attribute("width"), Some("60.00"));
        assert_eq!(position(span(&xml, "B"), "x"), 91.04688);
    }
}

#[test]
fn local_styles_remain_on_both_sides_of_the_object_and_end_at_the_next_line() {
    let mut content = mixed();
    content.text.push_str("\nC");
    content.spans = vec![
        RichTextSpan {
            kind: RichTextSpanType::FontSize,
            start_utf16: 0,
            end_utf16: 1,
            interval_type: sdocx::SpanIntervalType::from(1),
            payload: 18.0_f32.to_le_bytes().to_vec(),
        },
        RichTextSpan {
            kind: RichTextSpanType::FontSize,
            start_utf16: 2,
            end_utf16: 3,
            interval_type: sdocx::SpanIntervalType::from(1),
            payload: 22.0_f32.to_le_bytes().to_vec(),
        },
        RichTextSpan {
            kind: RichTextSpanType::ForegroundColor,
            start_utf16: 0,
            end_utf16: 1,
            interval_type: sdocx::SpanIntervalType::from(1),
            payload: vec![255, 0, 0, 255],
        },
        RichTextSpan {
            kind: RichTextSpanType::ForegroundColor,
            start_utf16: 2,
            end_utf16: 3,
            interval_type: sdocx::SpanIntervalType::from(1),
            payload: vec![0, 0, 255, 255],
        },
        RichTextSpan {
            kind: RichTextSpanType::Italic,
            start_utf16: 2,
            end_utf16: 3,
            interval_type: sdocx::SpanIntervalType::from(1),
            payload: vec![1, 0],
        },
    ];
    for page in modes(&document(PageElement::TextBox(content.clone()))) {
        assert_eq!(selectable(&page.svg), "ABC");
        let xml = roxmltree::Document::parse(&page.svg).unwrap();
        for (value, size, color) in [
            ("A", "18.00", "#0000ff"),
            ("B", "22.00", "#ff0000"),
            ("C", "20.00", "#000000"),
        ] {
            let node = span(&xml, value);
            assert_eq!(node.attribute("font-size"), Some(size));
            assert_eq!(node.attribute("fill"), Some(color));
        }
        assert_eq!(span(&xml, "B").attribute("font-style"), Some("italic"));
        assert_eq!(span(&xml, "C").attribute("font-style"), None);
        assert_eq!(position(span(&xml, "B"), "x"), 59.74219);
        assert_eq!(position(span(&xml, "C"), "y"), 127.70099);
    }
}

#[test]
fn only_valid_replacement_anchors_are_removed_from_selectable_text() {
    let mut content = mixed();
    content.text = "A\u{fffc}B\u{fffc} C".into();
    let mut unsupported = image(3, 30.0, 100.0, ObjectSpanLayoutOption::Inline);
    unsupported.content = None;
    content.object_spans.push(unsupported);
    for page in modes(&document(PageElement::TextBox(content.clone()))) {
        assert_eq!(selectable(&page.svg), "AB\u{fffc} C");
        assert_eq!(
            page.object_diagnostics,
            vec![sdocx::ObjectDiagnostic {
                anchor_utf16: 3,
                kind: sdocx::ObjectDiagnosticKind::UnsupportedContent,
            }]
        );
        let xml = roxmltree::Document::parse(&page.svg).unwrap();
        assert_eq!(
            xml.descendants()
                .filter(|node| node.has_tag_name("image"))
                .count(),
            1
        );
    }
}

fn embedded(content: RichTextObjectContent, kind: ObjectType) -> RichTextBox {
    let mut flow = text("\u{fffc}");
    flow.object_spans.push(RichTextObjectSpan {
        object_type: kind,
        object_data: Vec::new(),
        content: Some(content),
        text_index_utf16: 0,
        layout_option: ObjectSpanLayoutOption::Inline,
        layout_constraint: ObjectSpanLayoutConstraint::Normal,
    });
    flow
}

#[test]
fn placed_text_and_both_code_text_frames_keep_mixed_source_and_images() {
    let mut placed = mixed();
    placed.bbox = bounds(10.0, 0.0, 340.0, 700.0);
    let code_title = embedded(
        RichTextObjectContent::CodeBlock(Box::new(RichTextCodeBlock {
            bbox: bounds(0.0, 0.0, 360.0, 700.0),
            rotation_degrees: None,
            title: Some(mixed()),
            body: Some(text("body")),
        })),
        ObjectType::CodeBlock,
    );
    let code_body = embedded(
        RichTextObjectContent::CodeBlock(Box::new(RichTextCodeBlock {
            bbox: bounds(0.0, 0.0, 360.0, 700.0),
            rotation_degrees: None,
            title: Some(text("Title")),
            body: Some(mixed()),
        })),
        ObjectType::CodeBlock,
    );
    for (content, expected) in [
        (placed, "AB"),
        (code_title, "ABbody"),
        (code_body, "TitleAB"),
    ] {
        for page in modes(&document(PageElement::TextBox(content.clone()))) {
            assert_eq!(selectable(&page.svg), expected);
            assert!(page.object_diagnostics.is_empty());
            let xml = roxmltree::Document::parse(&page.svg).unwrap();
            assert_eq!(
                xml.descendants()
                    .filter(|node| node.has_tag_name("image"))
                    .count(),
                1
            );
            assert_eq!(span(&xml, "A").attribute("font-size"), Some("20.00"));
            assert_eq!(span(&xml, "B").attribute("font-size"), Some("20.00"));
        }
    }
}

#[cfg(feature = "serde")]
#[test]
fn shape_and_table_text_keep_mixed_source_and_images() {
    let mut placed = mixed();
    placed.bbox = bounds(10.0, 0.0, 340.0, 700.0);
    let shape = serde_json::from_value(serde_json::json!({
        "text_editable": true, "text_area_type": null, "shape_type": 0,
        "metadata": {
            "format_version": 1, "uuid": "mixed-shape", "modified_time_raw": 0,
            "bbox": placed.bbox, "replay_timestamp_raw": 0, "resize_mode_raw": 0,
            "rotatable": true, "selectable": true, "movable": true, "visible": true,
            "replayable": true, "out_of_canvas_enabled": false, "template": false,
            "flip_enabled": false, "float_drawn_rect": false, "locked": false,
            "removable": true, "rotation_degrees": null, "property_mask": [], "field_mask": [],
            "fixed_trailing_data": [], "flexible_trailing_data": []
        },
        "geometry_bbox": placed.bbox, "drawn_bbox": placed.bbox, "rotation_degrees": 0.0,
        "control_points": [], "path_data": [], "style": sdocx::ShapeStyle::default(),
        "fill": "None", "pen_name_id": null, "pen_settings_id": null, "text": placed,
    }))
    .unwrap();
    let style = serde_json::from_str(r#"{
        "heading_column_enabled": false, "heading_row_enabled": false, "max_height_enabled": false,
        "metadata": {"property_mask": [], "field_mask": [], "fixed_trailing_data": [], "flexible_trailing_data": []}
    }"#).unwrap();
    let frame = bounds(10.0, 0.0, 340.0, 700.0);
    let table = sdocx::RichTextTable {
        style,
        bbox: frame,
        rotation_degrees: None,
        column_widths: vec![340.0],
        rows: vec![sdocx::RichTextTableRow {
            max_height: None,
            min_height: None,
            metadata: Default::default(),
            index: 0,
            height: 700.0,
            cells: vec![sdocx::RichTextTableCell {
                border: None,
                metadata: Default::default(),
                column_index: 0,
                row_span: 1,
                column_span: 1,
                background_color: 0,
                has_own_background_color: false,
                bbox: frame,
                editable: false,
                content: mixed(),
            }],
        }],
    };
    let table = embedded(
        RichTextObjectContent::Table(Box::new(table)),
        ObjectType::Table,
    );
    for element in [PageElement::Shape(shape), PageElement::TextBox(table)] {
        for page in modes(&document(element)) {
            assert_eq!(selectable(&page.svg), "AB");
            assert!(page.object_diagnostics.is_empty());
            let xml = roxmltree::Document::parse(&page.svg).unwrap();
            assert_eq!(
                xml.descendants()
                    .filter(|node| node.has_tag_name("image"))
                    .count(),
                1
            );
        }
    }
}

#[cfg(feature = "pdf")]
#[test]
fn mixed_latin_text_remains_selectable_in_vector_pdf_with_one_inline_image() {
    let page = modes(&document(PageElement::TextBox(mixed())))
        .into_iter()
        .next()
        .unwrap();
    let bytes = sdocx::render_svg_pages_pdf(&[page], &Default::default()).unwrap();
    let pdf = lopdf::Document::load_mem(&bytes).unwrap();
    assert_eq!(
        pdf.extract_text(&[1]).unwrap().replace(['\n', ' '], ""),
        "AB"
    );
    assert!(pdf.objects.values().any(|object| {
        object
            .as_dict()
            .is_ok_and(|dict| dict.has(b"FontFile2") || dict.has(b"FontFile3"))
    }));
    assert_eq!(
        pdf.objects
            .values()
            .filter(|object| object.as_stream().is_ok_and(|stream| stream
                .dict
                .get(b"Subtype")
                .is_ok_and(|value| value.as_name().is_ok_and(|name| name == b"Image"))))
            .count(),
        1
    );
}
