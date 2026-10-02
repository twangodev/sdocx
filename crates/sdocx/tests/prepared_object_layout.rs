#![cfg(feature = "render")]

use sdocx::{
    BoundingBox, Document, DocumentMetadata, ObjectSpanLayoutConstraint, ObjectSpanLayoutOption,
    ObjectType, Page, PageElement, RichTextBox, RichTextCodeBlock, RichTextObjectContent,
    RichTextObjectSpan,
};

#[derive(Clone, Copy, Debug)]
enum Context {
    Flow,
    Placed,
    CodeBody,
    #[cfg(feature = "serde")]
    Shape,
    #[cfg(feature = "serde")]
    Table,
}

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

fn object(
    content: RichTextObjectContent,
    kind: ObjectType,
    constraint: ObjectSpanLayoutConstraint,
) -> RichTextObjectSpan {
    RichTextObjectSpan {
        object_type: kind,
        object_data: Vec::new(),
        content: Some(content),
        text_index_utf16: 0,
        layout_option: ObjectSpanLayoutOption::Inline,
        layout_constraint: constraint,
    }
}

fn code_text(
    saved_top: f64,
    saved_height: f64,
    constraint: ObjectSpanLayoutConstraint,
) -> RichTextBox {
    let mut content = text("\u{fffc}\nC");
    content.object_spans.push(object(
        RichTextObjectContent::CodeBlock(Box::new(RichTextCodeBlock {
            bbox: bounds(0.0, saved_top, 340.0, saved_height),
            rotation_degrees: None,
            title: None,
            body: Some(text("A\nB")),
        })),
        ObjectType::CodeBlock,
        constraint,
    ));
    content
}

#[cfg(feature = "serde")]
fn shape(mut content: RichTextBox) -> PageElement {
    content.bbox = bounds(10.0, 0.0, 340.0, 700.0);
    PageElement::Shape(
        serde_json::from_value(serde_json::json!({
            "text_editable": true, "text_area_type": null, "shape_type": 0,
            "metadata": {
                "format_version": 1, "uuid": "prepared-shape", "modified_time_raw": 0,
                "bbox": content.bbox, "replay_timestamp_raw": 0, "resize_mode_raw": 0,
                "rotatable": true, "selectable": true, "movable": true, "visible": true,
                "replayable": true, "out_of_canvas_enabled": false, "template": false,
                "flip_enabled": false, "float_drawn_rect": false, "locked": false,
                "removable": true, "rotation_degrees": null, "property_mask": [], "field_mask": [],
                "fixed_trailing_data": [], "flexible_trailing_data": []
            },
            "geometry_bbox": content.bbox, "drawn_bbox": content.bbox, "rotation_degrees": 0.0,
            "control_points": [], "path_data": [], "style": sdocx::ShapeStyle::default(),
            "fill": "None", "pen_name_id": null, "pen_settings_id": null, "text": content,
        }))
        .unwrap(),
    )
}

#[cfg(feature = "serde")]
fn table(content: RichTextBox) -> PageElement {
    let style = serde_json::from_str(r#"{
        "heading_column_enabled": false, "heading_row_enabled": false, "max_height_enabled": false,
        "metadata": {"property_mask": [], "field_mask": [], "fixed_trailing_data": [], "flexible_trailing_data": []}
    }"#).unwrap();
    let frame = bounds(10.0, 0.0, 340.0, 700.0);
    let table = RichTextObjectContent::Table(Box::new(sdocx::RichTextTable {
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
                content,
            }],
        }],
    }));
    let mut flow = text("\u{fffc}");
    flow.object_spans.push(object(
        table,
        ObjectType::Table,
        ObjectSpanLayoutConstraint::Normal,
    ));
    PageElement::TextBox(flow)
}

fn document(
    context: Context,
    mut content: RichTextBox,
    page_height: u32,
    candidate_top: u32,
) -> Document {
    let element = match context {
        Context::Flow => {
            content.margins = Some([0.0, candidate_top as f32, 0.0, 0.0]);
            PageElement::TextBox(content)
        }
        Context::Placed => {
            content.bbox = bounds(10.0, 0.0, 340.0, 700.0);
            PageElement::TextBox(content)
        }
        Context::CodeBody => {
            let outer = RichTextObjectContent::CodeBlock(Box::new(RichTextCodeBlock {
                bbox: bounds(0.0, 0.0, 380.0, 700.0),
                rotation_degrees: None,
                title: None,
                body: Some(content),
            }));
            let mut flow = text("\u{fffc}");
            flow.object_spans.push(object(
                outer,
                ObjectType::CodeBlock,
                ObjectSpanLayoutConstraint::Normal,
            ));
            PageElement::TextBox(flow)
        }
        #[cfg(feature = "serde")]
        Context::Shape => shape(content),
        #[cfg(feature = "serde")]
        Context::Table => table(content),
    };
    Document {
        pages: vec![Page {
            uuid: "prepared-objects".into(),
            width: 360,
            height: page_height,
            content_bbox: bounds(0.0, 0.0, 360.0, f64::from(page_height)),
            background_color: None,
            template: None,
            background: Default::default(),
            objects: vec![element.into()],
        }],
        metadata: DocumentMetadata {
            default_page_dimensions: Some((360, 800)),
            orientation: Some(0),
            page_mode: Some(0),
            flow_page_padding: Some((10, candidate_top)),
            ..Default::default()
        },
    }
}

fn point(node: roxmltree::Node<'_, '_>) -> (f64, f64) {
    let coordinate = |attribute| {
        node.attribute(attribute)
            .or_else(|| node.parent().unwrap().attribute(attribute))
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .parse::<f64>()
            .unwrap()
    };
    let mut position = (coordinate("x"), coordinate("y"));
    for ancestor in node.ancestors() {
        if let Some(value) = ancestor.attribute("transform") {
            let transform: svgtypes::Transform = value.parse().unwrap();
            position = (
                transform.a * position.0 + transform.c * position.1 + transform.e,
                transform.b * position.0 + transform.d * position.1 + transform.f,
            );
        }
    }
    position
}

#[derive(Debug, PartialEq)]
struct Geometry {
    panel_position: (f64, f64),
    panel_height: f64,
    baselines: [f64; 3],
}

fn geometry(svg: &str) -> Geometry {
    let xml = roxmltree::Document::parse(svg).unwrap();
    let source = xml
        .descendants()
        .filter(|node| node.has_tag_name("tspan"))
        .filter_map(|node| node.text())
        .collect::<String>();
    assert_eq!(source, "ABC");
    let code = xml
        .descendants()
        .find(|node| {
            node.attribute("data-sdocx-object") == Some("code-block")
                && !node.descendants().any(|child| {
                    child != *node && child.attribute("data-sdocx-object") == Some("code-block")
                })
        })
        .unwrap();
    let panel = code
        .children()
        .find(|node| node.has_tag_name("rect"))
        .unwrap();
    let baseline = |value| {
        point(
            xml.descendants()
                .find(|node| node.has_tag_name("tspan") && node.text() == Some(value))
                .unwrap(),
        )
        .1
    };
    Geometry {
        panel_position: point(panel),
        panel_height: panel.attribute("height").unwrap().parse().unwrap(),
        baselines: [baseline("A"), baseline("B"), baseline("C")],
    }
}

fn modes(doc: &Document) -> [sdocx::RenderedPage; 2] {
    let layout = sdocx::layout_document(doc);
    [
        sdocx::render_layout_page_svg(doc, &layout, 0, &Default::default()).unwrap(),
        sdocx::render_layout_page_replay_svg(doc, &layout, 0, &Default::default()).unwrap(),
    ]
}

fn assert_baselines(actual: [f64; 3], expected: [f64; 3]) {
    for (actual, expected) in actual.into_iter().zip(expected) {
        assert!(
            (actual - expected).abs() < 1e-8,
            "actual {actual}, expected {expected}"
        );
    }
}

#[test]
fn cross_page_constraints_use_the_measured_child_height_in_every_text_context() {
    // Native Measure shifts chrome by the first split top, including negative tops.
    for context in [
        Context::Flow,
        Context::Placed,
        Context::CodeBody,
        #[cfg(feature = "serde")]
        Context::Shape,
        #[cfg(feature = "serde")]
        Context::Table,
    ] {
        for constraint in [
            ObjectSpanLayoutConstraint::OverPagesOverlapPadding,
            ObjectSpanLayoutConstraint::OverPages,
        ] {
            let (panel_height, expected) = match (context, constraint) {
                (Context::CodeBody, _) => (118.0, [108.00198, 135.00198, 189.00198]),
                #[cfg(feature = "serde")]
                (Context::Table, ObjectSpanLayoutConstraint::OverPagesOverlapPadding) => {
                    (117.5, [64.0, 91.0, 145.00101])
                }
                #[cfg(feature = "serde")]
                (Context::Table, ObjectSpanLayoutConstraint::OverPages) => {
                    (107.5, [54.0, 81.0, 135.00101])
                }
                (_, ObjectSpanLayoutConstraint::OverPagesOverlapPadding) => {
                    (118.0, [64.0, 91.0, 145.00101])
                }
                (_, ObjectSpanLayoutConstraint::OverPages) => (108.0, [54.0, 81.0, 135.00101]),
                _ => unreachable!(),
            };
            let mut previous = None;
            for saved_height in [50.0, 300.0] {
                let doc = document(context, code_text(0.0, saved_height, constraint), 800, 0);
                for page in modes(&doc) {
                    assert_eq!(
                        page.object_diagnostics,
                        match context {
                            Context::Flow => vec![sdocx::ObjectDiagnostic {
                                anchor_utf16: 0,
                                kind: sdocx::ObjectDiagnosticKind::UnsupportedWidthLimitContext,
                            }],
                            #[cfg(feature = "serde")]
                            Context::Table => vec![sdocx::ObjectDiagnostic {
                                anchor_utf16: 0,
                                kind: sdocx::ObjectDiagnosticKind::UnsupportedContent,
                            }],
                            _ => vec![],
                        }
                    );
                    let actual = geometry(&page.svg);
                    assert_eq!(actual.panel_height, panel_height, "{context:?}");
                    assert_baselines(actual.baselines, expected);
                    if let Some(previous) = &previous {
                        assert_eq!(&actual, previous, "{context:?}");
                    }
                    previous = Some(actual);
                }
            }
        }
    }
}

#[test]
fn placed_gravity_translates_the_retained_child_plan_with_its_parent_line() {
    for (gravity, panel_top, expected) in [
        (1, 274.0005, [338.0005, 365.0005, 419.0005]),
        (2, 547.99999, [611.99999, 638.99999, 693.0]),
    ] {
        for constraint in [
            ObjectSpanLayoutConstraint::OverPagesOverlapPadding,
            ObjectSpanLayoutConstraint::OverPages,
        ] {
            let (panel_top, expected) = match (gravity, constraint) {
                (1, ObjectSpanLayoutConstraint::OverPages) => {
                    (279.0005, [343.0005, 370.0005, 414.0005])
                }
                (2, ObjectSpanLayoutConstraint::OverPages) => {
                    (557.99999, [621.99999, 648.99999, 693.0])
                }
                _ => (panel_top, expected),
            };
            for saved_height in [50.0, 300.0] {
                let mut content = code_text(200.0, saved_height, constraint);
                content.gravity = Some(gravity);
                let doc = document(Context::Placed, content, 800, 0);
                for page in modes(&doc) {
                    let actual = geometry(&page.svg);
                    assert_eq!(actual.panel_height, 118.0);
                    assert_baselines(actual.baselines, expected);
                    assert!((actual.panel_position.1 - panel_top).abs() < 1e-8);
                }
            }
        }
    }
}

#[test]
fn child_page_exclusions_use_the_actual_candidate_top_instead_of_saved_y() {
    for (constraint, candidate_top, expected_height, expected_baselines) in [
        (
            ObjectSpanLayoutConstraint::OverPagesOverlapPadding,
            0,
            128.0,
            [64.0, 101.0, 155.00101],
        ),
        (
            ObjectSpanLayoutConstraint::OverPagesOverlapPadding,
            30,
            125.0,
            [101.0, 128.0, 182.00101],
        ),
        (
            ObjectSpanLayoutConstraint::OverPages,
            0,
            137.0,
            [54.0, 110.0, 164.00101],
        ),
        (
            ObjectSpanLayoutConstraint::OverPages,
            30,
            134.0,
            [110.0, 137.0, 191.00101],
        ),
    ] {
        let mut previous: Option<Geometry> = None;
        for saved_y in [0.0, 200.0] {
            for saved_height in [50.0, 300.0] {
                let doc = document(
                    Context::Flow,
                    code_text(saved_y, saved_height, constraint),
                    80,
                    candidate_top,
                );
                for page in modes(&doc) {
                    let actual = geometry(&page.svg);
                    assert_eq!(actual.panel_height, expected_height);
                    assert_eq!(actual.baselines, expected_baselines);
                    let panel_top = match (saved_y, candidate_top, constraint, saved_height) {
                        (0.0, 0, _, _) => 0.00101,
                        (0.0, 30, _, _) => 30.00101,
                        (200.0, 0, ObjectSpanLayoutConstraint::OverPages, 50.0) => 0.00098,
                        (200.0, 0, _, _) => 0.00101,
                        (200.0, 30, _, _) => 30.00101,
                        _ => unreachable!(),
                    };
                    assert_eq!(actual.panel_position.1, panel_top);
                    if let Some(previous) = &previous {
                        assert_eq!(actual.panel_position.0, previous.panel_position.0);
                        assert_eq!(actual.panel_height, previous.panel_height);
                        assert_eq!(actual.baselines[1..], previous.baselines[1..]);
                    }
                    previous = Some(actual);
                }
            }
        }
    }
}

#[test]
fn normal_constraint_retains_saved_reservation_while_painting_measured_code() {
    for (saved_height, expected) in [
        (50.0, [64.001, 91.001, 77.001]),
        (300.0, [64.00101, 91.00101, 327.00101]),
    ] {
        let doc = document(
            Context::Flow,
            code_text(0.0, saved_height, ObjectSpanLayoutConstraint::Normal),
            800,
            0,
        );
        for page in modes(&doc) {
            let actual = geometry(&page.svg);
            assert_eq!(actual.panel_height, 118.0);
            assert_eq!(actual.baselines, expected);
        }
    }
}

#[test]
fn invalid_derived_panels_preserve_the_anchor_and_neighbors_in_every_constraint() {
    for constraint in [
        ObjectSpanLayoutConstraint::Normal,
        ObjectSpanLayoutConstraint::OverPagesOverlapPadding,
        ObjectSpanLayoutConstraint::OverPages,
    ] {
        for oversized_font in [false, true] {
            let mut content = code_text(0.0, 50.0, constraint);
            content.text = "A\u{fffc}C".into();
            content.object_spans[0].text_index_utf16 = 1;
            let Some(RichTextObjectContent::CodeBlock(code)) = &mut content.object_spans[0].content
            else {
                panic!()
            };
            let body = code.body.as_mut().unwrap();
            if oversized_font {
                body.font_size = Some(f32::MAX);
            } else {
                body.margins = Some([0.0, -1000.0, 0.0, 0.0]);
            }
            let doc = document(Context::Flow, content, 800, 0);
            for page in modes(&doc) {
                assert_eq!(
                    page.object_diagnostics,
                    vec![sdocx::ObjectDiagnostic {
                        anchor_utf16: 1,
                        kind: sdocx::ObjectDiagnosticKind::InvalidBounds,
                    }]
                );
                let xml = roxmltree::Document::parse(&page.svg).unwrap();
                assert_eq!(
                    xml.descendants()
                        .filter(|node| node.has_tag_name("tspan"))
                        .filter_map(|node| node.text())
                        .collect::<String>(),
                    "A\u{fffc}C"
                );
                assert!(
                    !xml.descendants()
                        .any(|node| node.attribute("data-sdocx-object") == Some("code-block"))
                );
            }
        }
    }
}

#[test]
fn nested_child_height_feedback_reaches_the_following_outer_text() {
    for constraint in [
        ObjectSpanLayoutConstraint::OverPagesOverlapPadding,
        ObjectSpanLayoutConstraint::OverPages,
    ] {
        for inner_height in [50.0, 300.0] {
            for outer_height in [50.0, 300.0] {
                let mut content = text("\u{fffc}\nD");
                content.object_spans.push(object(
                    RichTextObjectContent::CodeBlock(Box::new(RichTextCodeBlock {
                        bbox: bounds(0.0, 200.0, 380.0, outer_height),
                        rotation_degrees: None,
                        title: None,
                        body: Some(code_text(200.0, inner_height, constraint)),
                    })),
                    ObjectType::CodeBlock,
                    constraint,
                ));
                for page in modes(&document(Context::Flow, content, 800, 0)) {
                    let xml = roxmltree::Document::parse(&page.svg).unwrap();
                    let spans: Vec<_> = xml
                        .descendants()
                        .filter(|node| node.has_tag_name("tspan"))
                        .collect();
                    assert_eq!(
                        spans
                            .iter()
                            .filter_map(|node| node.text())
                            .collect::<String>(),
                        "ABCD"
                    );
                    let (expected, outer_panel_height) = match constraint {
                        ObjectSpanLayoutConstraint::OverPagesOverlapPadding => {
                            ([108.001, 135.001, 189.00101, 243.00201], "216.00")
                        }
                        ObjectSpanLayoutConstraint::OverPages => {
                            ([98.001, 125.001, 179.00101, 233.00201], "206.00")
                        }
                        _ => unreachable!(),
                    };
                    for (span, expected) in spans.into_iter().zip(expected) {
                        assert!(
                            (point(span).1 - expected).abs() < 1e-8,
                            "actual {:?}, expected {expected}",
                            point(span)
                        );
                    }
                    let heights: Vec<_> = xml
                        .descendants()
                        .filter(|node| node.attribute("data-sdocx-object") == Some("code-block"))
                        .map(|node| {
                            node.children()
                                .find(|child| child.has_tag_name("rect"))
                                .unwrap()
                                .attribute("height")
                                .unwrap()
                        })
                        .collect();
                    assert_eq!(heights, [outer_panel_height, "118.00"]);
                    assert_eq!(
                        page.object_diagnostics,
                        [sdocx::ObjectDiagnostic {
                            anchor_utf16: 0,
                            kind: sdocx::ObjectDiagnosticKind::UnsupportedWidthLimitContext,
                        }]
                    );
                }
            }
        }
    }
}

#[test]
fn rejected_inline_code_keeps_source_order_on_one_placed_line() {
    for constraint in [
        ObjectSpanLayoutConstraint::Normal,
        ObjectSpanLayoutConstraint::OverPagesOverlapPadding,
        ObjectSpanLayoutConstraint::OverPages,
    ] {
        let mut content = code_text(0.0, 50.0, constraint);
        content.text = "A\u{fffc}C".into();
        content.object_spans[0].text_index_utf16 = 1;
        let Some(RichTextObjectContent::CodeBlock(code)) = &mut content.object_spans[0].content
        else {
            panic!()
        };
        code.bbox.x_max = 100.0;
        code.body.as_mut().unwrap().margins = Some([0.0, -1000.0, 0.0, 0.0]);
        for page in modes(&document(Context::Placed, content, 800, 0)) {
            let xml = roxmltree::Document::parse(&page.svg).unwrap();
            let lines: Vec<_> = xml
                .descendants()
                .filter(|node| node.has_tag_name("text"))
                .map(|line| {
                    line.descendants()
                        .filter(|node| node.has_tag_name("tspan"))
                        .filter_map(|node| node.text())
                        .collect::<String>()
                })
                .collect();
            assert_eq!(lines, ["A\u{fffc}C"]);
            assert_eq!(
                page.object_diagnostics,
                [sdocx::ObjectDiagnostic {
                    anchor_utf16: 1,
                    kind: sdocx::ObjectDiagnosticKind::InvalidBounds,
                }]
            );
            assert!(
                !xml.descendants()
                    .any(|node| node.attribute("data-sdocx-object") == Some("code-block"))
            );
        }
    }
}

#[cfg(feature = "pdf")]
#[test]
fn remeasured_code_and_following_text_stay_selectable_in_vector_pdf() {
    for constraint in [
        ObjectSpanLayoutConstraint::OverPagesOverlapPadding,
        ObjectSpanLayoutConstraint::OverPages,
    ] {
        let doc = document(Context::Flow, code_text(200.0, 50.0, constraint), 800, 0);
        let page = modes(&doc).into_iter().next().unwrap();
        let expected = match constraint {
            ObjectSpanLayoutConstraint::OverPagesOverlapPadding => [64.0, 91.0, 145.00101],
            ObjectSpanLayoutConstraint::OverPages => [54.0, 81.0, 135.00101],
            _ => unreachable!(),
        };
        assert_eq!(geometry(&page.svg).baselines, expected);
        let bytes = sdocx::render_svg_pages_pdf(&[page], &Default::default()).unwrap();
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        assert_eq!(
            pdf.extract_text(&[1]).unwrap().replace(['\n', ' '], ""),
            "ABC"
        );
        assert!(
            !pdf.objects
                .values()
                .any(|object| object.as_stream().is_ok_and(|stream| stream
                    .dict
                    .get(b"Subtype")
                    .is_ok_and(|value| value.as_name().is_ok_and(|name| name == b"Image"))))
        );
    }
}
