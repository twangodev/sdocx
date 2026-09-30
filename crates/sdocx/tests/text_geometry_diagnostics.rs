#![cfg(feature = "render")]

use sdocx::{
    BoundingBox, Document, DocumentMetadata, ObjectSpanLayoutConstraint, ObjectSpanLayoutOption,
    ObjectType, Page, PageElement, RenderedPage, RichTextBox, RichTextCodeBlock,
    RichTextObjectContent, RichTextObjectSpan, RichTextParagraph, RichTextParagraphType,
    RichTextSpan, RichTextSpanType,
};

#[derive(Clone, Copy, Debug)]
enum Context {
    Flow,
    Placed,
    CodeTitle,
    CodeBody,
    #[cfg(feature = "serde")]
    Shape,
    #[cfg(feature = "serde")]
    Table,
}

const CONTEXTS: &[Context] = &[
    Context::Flow,
    Context::Placed,
    Context::CodeTitle,
    Context::CodeBody,
    #[cfg(feature = "serde")]
    Context::Shape,
    #[cfg(feature = "serde")]
    Context::Table,
];

#[derive(Clone, Copy, Debug)]
enum Metric {
    Font,
    LocalFont,
    PixelSpacing,
    PercentSpacing,
    Before,
    After,
    Margins,
}

const METRICS: &[Metric] = &[
    Metric::Font,
    Metric::LocalFont,
    Metric::PixelSpacing,
    Metric::PercentSpacing,
    Metric::Before,
    Metric::After,
    Metric::Margins,
];

fn bounds() -> BoundingBox {
    BoundingBox {
        x_min: 20.0,
        y_min: 20.0,
        x_max: 520.0,
        y_max: 620.0,
    }
}

fn text() -> RichTextBox {
    RichTextBox {
        text_area_type: None,
        bbox: BoundingBox::default(),
        rotation_degrees: None,
        text: "A\nB".into(),
        color: None,
        highlight_color: None,
        underline: false,
        font_size: Some(15.0),
        runs: Vec::new(),
        spans: Vec::new(),
        paragraphs: Vec::new(),
        object_spans: Vec::new(),
        text_sections: Vec::new(),
        margins: None,
        gravity: None,
    }
}

fn malformed(metric: Metric, value: f32) -> RichTextBox {
    let mut content = text();
    match metric {
        Metric::Font => content.font_size = Some(value),
        Metric::LocalFont => content.spans.push(RichTextSpan {
            kind: RichTextSpanType::FontSize,
            start_utf16: 2,
            end_utf16: 3,
            expand: true,
            payload: value.to_le_bytes().to_vec(),
        }),
        Metric::PixelSpacing | Metric::PercentSpacing => {
            content.paragraphs.push(RichTextParagraph {
                kind: RichTextParagraphType::LineSpacing,
                start_paragraph: 0,
                end_paragraph: 2,
                payload: [
                    u32::from(matches!(metric, Metric::PercentSpacing))
                        .to_le_bytes()
                        .to_vec(),
                    value.to_le_bytes().to_vec(),
                ]
                .concat(),
            })
        }
        Metric::Before | Metric::After => content.paragraphs.push(RichTextParagraph {
            kind: if matches!(metric, Metric::Before) {
                RichTextParagraphType::SpacingBefore
            } else {
                RichTextParagraphType::SpacingAfter
            },
            start_paragraph: 0,
            end_paragraph: 2,
            payload: value.to_le_bytes().to_vec(),
        }),
        Metric::Margins => content.margins = Some([value; 4]),
    }
    content
}

fn embedded(content: RichTextObjectContent, kind: ObjectType) -> PageElement {
    let mut flow = text();
    flow.text = "\u{fffc}".into();
    flow.object_spans.push(RichTextObjectSpan {
        object_type: kind,
        object_data: Vec::new(),
        content: Some(content),
        text_index_utf16: 0,
        layout_option: ObjectSpanLayoutOption::Inline,
        layout_constraint: ObjectSpanLayoutConstraint::Normal,
    });
    PageElement::TextBox(flow)
}

#[cfg(feature = "serde")]
fn shape(mut content: RichTextBox) -> PageElement {
    content.bbox = bounds();
    PageElement::Shape(
        serde_json::from_value(serde_json::json!({
            "text_editable": true, "text_area_type": null, "shape_type": 4,
            "metadata": {
                "format_version": 1, "uuid": "geometry-shape", "modified_time_raw": 0,
                "bbox": bounds(), "replay_timestamp_raw": 0, "resize_mode_raw": 0,
                "rotatable": true, "selectable": true, "movable": true, "visible": true,
                "replayable": true, "out_of_canvas_enabled": false, "template": false,
                "flip_enabled": false, "float_drawn_rect": false, "locked": false,
                "removable": true, "rotation_degrees": null, "property_mask": [], "field_mask": [],
                "fixed_trailing_data": [], "flexible_trailing_data": []
            },
            "geometry_bbox": bounds(), "drawn_bbox": bounds(), "rotation_degrees": 0.0,
            "control_points": [], "path_data": [], "style": sdocx::ShapeStyle::default(),
            "fill": "None", "pen_name_id": null, "pen_settings_id": null, "text": null
        }))
        .map(|mut shape: sdocx::NativeShape| {
            shape.text = Some(Box::new(content));
            shape
        })
        .unwrap(),
    )
}

#[cfg(feature = "serde")]
fn table(content: RichTextBox) -> PageElement {
    let style = serde_json::from_str(r#"{
        "heading_column_enabled": false, "heading_row_enabled": false, "max_height_enabled": false,
        "metadata": {"property_mask": [], "field_mask": [], "fixed_trailing_data": [], "flexible_trailing_data": []}
    }"#).unwrap();
    embedded(
        RichTextObjectContent::Table(Box::new(sdocx::RichTextTable {
            style,
            bbox: bounds(),
            rotation_degrees: None,
            column_widths: vec![500.0],
            rows: vec![sdocx::RichTextTableRow {
                max_height: None,
                min_height: None,
                metadata: Default::default(),
                index: 0,
                height: 600.0,
                cells: vec![sdocx::RichTextTableCell {
                    border: None,
                    metadata: Default::default(),
                    column_index: 0,
                    row_span: 1,
                    column_span: 1,
                    background_color: 0,
                    has_own_background_color: false,
                    bbox: bounds(),
                    editable: false,
                    content,
                }],
            }],
        })),
        ObjectType::Table,
    )
}

fn document(context: Context, mut content: RichTextBox) -> Document {
    let element = match context {
        Context::Flow => PageElement::TextBox(content),
        Context::Placed => {
            content.bbox = bounds();
            PageElement::TextBox(content)
        }
        Context::CodeTitle | Context::CodeBody => {
            let (title, body) = if matches!(context, Context::CodeTitle) {
                (Some(content), None)
            } else {
                (None, Some(content))
            };
            embedded(
                RichTextObjectContent::CodeBlock(Box::new(RichTextCodeBlock {
                    bbox: bounds(),
                    rotation_degrees: None,
                    title,
                    body,
                })),
                ObjectType::CodeBlock,
            )
        }
        #[cfg(feature = "serde")]
        Context::Shape => shape(content),
        #[cfg(feature = "serde")]
        Context::Table => table(content),
    };
    Document {
        pages: vec![Page {
            uuid: "text-geometry".into(),
            width: 1080,
            height: 1527,
            content_bbox: bounds(),
            background_color: None,
            template: None,
            background: Default::default(),
            objects: vec![element.into()],
        }],
        metadata: DocumentMetadata {
            default_page_dimensions: Some((1080, 1527)),
            orientation: Some(0),
            flow_page_padding: Some((20, 20)),
            ..Default::default()
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

fn assert_finite_and_complete(page: &RenderedPage, case: &str) {
    let xml = roxmltree::Document::parse(&page.svg).unwrap();
    let source = xml
        .descendants()
        .filter(|node| node.has_tag_name("tspan"))
        .filter_map(|node| node.text())
        .collect::<String>();
    assert_eq!(source, "AB", "{case}");
    for node in xml.descendants().filter(|node| node.is_element()) {
        for attribute in [
            "x",
            "y",
            "dx",
            "dy",
            "width",
            "height",
            "font-size",
            "stroke-width",
            "cx",
            "cy",
            "rx",
            "ry",
            "x1",
            "x2",
            "y1",
            "y2",
        ] {
            if let Some(value) = node.attribute(attribute) {
                for number in svgtypes::NumberListParser::from(value) {
                    assert!(
                        number.unwrap().is_finite(),
                        "{case}: nonfinite {attribute}: {value}"
                    );
                }
            }
        }
        if let Some(value) = node.attribute("transform") {
            let transform: svgtypes::Transform = value.parse().unwrap();
            assert!(
                [
                    transform.a,
                    transform.b,
                    transform.c,
                    transform.d,
                    transform.e,
                    transform.f
                ]
                .into_iter()
                .all(f64::is_finite)
            );
        }
    }
}

#[test]
fn invalid_and_overflowing_text_metrics_keep_source_and_report_geometry_failure() {
    for &context in CONTEXTS {
        for &metric in METRICS {
            for value in [f32::MAX, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                for page in modes(&document(context, malformed(metric, value))) {
                    assert_finite_and_complete(&page, &format!("{context:?} {metric:?} {value}"));
                    assert!(
                        page.text_diagnostics
                            .iter()
                            .any(|diagnostic| diagnostic.kind
                                == sdocx::TextDiagnosticKind::InvalidGeometry),
                        "{context:?} {metric:?} {value}"
                    );
                    assert!(
                        !page.text_diagnostics.iter().any(|diagnostic| matches!(
                            diagnostic.kind,
                            sdocx::TextDiagnosticKind::MeasurementFailure
                                | sdocx::TextDiagnosticKind::UnsupportedGlyphPositioning
                        )),
                        "{context:?} {metric:?} {value}: misleading font failure"
                    );
                }
            }
        }
    }
}

#[test]
fn ordinary_text_geometry_does_not_report_errors_in_any_context() {
    for &context in CONTEXTS {
        for page in modes(&document(context, text())) {
            assert_finite_and_complete(&page, &format!("ordinary {context:?}"));
            assert!(page.text_diagnostics.is_empty(), "{context:?}");
        }
    }
}

#[test]
fn overwritten_and_unused_invalid_font_values_do_not_report_geometry_failure() {
    for &context in CONTEXTS {
        for value in [f32::MAX, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut overridden = text();
            overridden.spans = vec![
                RichTextSpan {
                    kind: RichTextSpanType::FontSize,
                    start_utf16: 0,
                    end_utf16: 3,
                    expand: true,
                    payload: value.to_le_bytes().to_vec(),
                },
                RichTextSpan {
                    kind: RichTextSpanType::FontSize,
                    start_utf16: 0,
                    end_utf16: 3,
                    expand: true,
                    payload: 10.0_f32.to_le_bytes().to_vec(),
                },
            ];
            let mut unused = text();
            unused.spans.push(RichTextSpan {
                kind: RichTextSpanType::FontSize,
                start_utf16: 9,
                end_utf16: 10,
                expand: true,
                payload: value.to_le_bytes().to_vec(),
            });
            let mut overridden_summary = text();
            overridden_summary.font_size = Some(value);
            overridden_summary.spans.push(RichTextSpan {
                kind: RichTextSpanType::FontSize,
                start_utf16: 0,
                end_utf16: 3,
                expand: true,
                payload: 10.0_f32.to_le_bytes().to_vec(),
            });
            for (content, expected_size) in [
                (overridden, "30.00"),
                (unused, "45.00"),
                (overridden_summary, "30.00"),
            ] {
                for page in modes(&document(context, content)) {
                    assert_finite_and_complete(&page, &format!("inactive {context:?} {value}"));
                    assert!(page.text_diagnostics.is_empty(), "{context:?} {value}");
                    let xml = roxmltree::Document::parse(&page.svg).unwrap();
                    for span in xml.descendants().filter(|node| node.has_tag_name("tspan")) {
                        assert_eq!(span.attribute("font-size"), Some(expected_size));
                    }
                }
            }
        }
    }
}

#[test]
fn invalid_active_font_span_keeps_the_previous_valid_size() {
    for &context in CONTEXTS {
        for value in [f32::MAX, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            for page in modes(&document(context, malformed(Metric::LocalFont, value))) {
                assert_finite_and_complete(&page, &format!("active {context:?} {value}"));
                let xml = roxmltree::Document::parse(&page.svg).unwrap();
                for span in xml.descendants().filter(|node| node.has_tag_name("tspan")) {
                    assert_eq!(span.attribute("font-size"), Some("45.00"));
                }
            }
        }
    }
}

#[cfg(feature = "pdf")]
#[test]
fn recovered_text_geometry_remains_selectable_and_vector_in_pdf() {
    for &context in CONTEXTS {
        for metric in [Metric::Font, Metric::PixelSpacing, Metric::Margins] {
            let page = modes(&document(context, malformed(metric, f32::INFINITY)))
                .into_iter()
                .next()
                .unwrap();
            assert_finite_and_complete(&page, &format!("{context:?} {metric:?} infinity PDF"));
            let bytes = sdocx::render_svg_pages_pdf(&[page], &Default::default()).unwrap();
            let pdf = lopdf::Document::load_mem(&bytes).unwrap();
            assert_eq!(
                pdf.extract_text(&[1]).unwrap().replace(['\n', ' '], ""),
                "AB"
            );
            assert!(
                !pdf.objects
                    .values()
                    .any(|object| object
                        .as_stream()
                        .is_ok_and(|stream| stream.dict.get(b"Subtype").is_ok_and(|value| value
                            .as_name()
                            .is_ok_and(|name| name == b"Image"))))
            );
        }
    }
}
