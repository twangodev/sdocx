#![cfg(all(feature = "render", feature = "serde"))]

use sdocx::{
    BoundingBox, Document, DocumentMetadata, ObjectSpanLayoutConstraint, ObjectSpanLayoutOption,
    ObjectType, Page, PageElement, PlacedImage, RichTextBox, RichTextObjectContent,
    RichTextObjectSpan, RichTextSection, RichTextSpan, RichTextSpanType, SpanIntervalType,
    TextDiagnosticKind,
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
        bbox: bounds(360.0, 800.0),
        rotation_degrees: None,
        text: source.into(),
        color: None,
        highlight_color: None,
        underline: false,
        font_size: Some(10.0),
        runs: Vec::new(),
        spans: Vec::new(),
        paragraphs: Vec::new(),
        object_spans: Vec::new(),
        text_sections: Vec::new(),
        margins: None,
        gravity: None,
    }
}

fn font_span(start: u32, end: u32, size: f32) -> RichTextSpan {
    RichTextSpan {
        kind: RichTextSpanType::FontSize,
        start_utf16: start,
        end_utf16: end,
        interval_type: SpanIntervalType::ClosedOpen,
        payload: size.to_le_bytes().to_vec(),
    }
}

fn page(index: usize, height: u32) -> Page {
    Page {
        uuid: format!("geometry-{index}"),
        width: 360,
        height,
        content_bbox: bounds(360.0, f64::from(height)),
        background_color: None,
        template: None,
        background: Default::default(),
        objects: Vec::new(),
    }
}

fn visible_source(svg: &str) -> String {
    roxmltree::Document::parse(svg)
        .unwrap()
        .descendants()
        .filter(|node| node.has_tag_name("tspan"))
        .filter_map(|node| node.text())
        .collect()
}

#[test]
fn capture_reports_invalid_font_only_on_its_visible_page() {
    let mut body = text("First\nSecond\n");
    body.bbox = BoundingBox::default();
    body.spans.push(font_span(0, 5, f32::NAN));
    body.text_sections = vec![
        RichTextSection {
            start_utf16: 0,
            length_utf16: 6,
        },
        RichTextSection {
            start_utf16: 5,
            length_utf16: 8,
        },
    ];
    let document = Document {
        pages: vec![page(0, 40), page(1, 40)],
        metadata: DocumentMetadata {
            note_text: Some(body),
            page_mode: Some(0),
            orientation: Some(0),
            default_page_dimensions: Some((360, 40)),
            flow_page_padding: Some((10, 0)),
            ..Default::default()
        },
    };
    let layout = sdocx::layout_document(&document);
    for replay in [false, true] {
        let render = |index| {
            if replay {
                sdocx::render_layout_page_replay_svg(&document, &layout, index, &Default::default())
            } else {
                sdocx::render_layout_page_svg(&document, &layout, index, &Default::default())
            }
            .unwrap()
        };
        let first = render(0);
        let second = render(1);
        assert_eq!(visible_source(&first.svg), "First");
        assert_eq!(visible_source(&second.svg), "Second");
        assert_eq!(first.text_diagnostics.len(), 1);
        assert_eq!(
            first.text_diagnostics[0].kind,
            TextDiagnosticKind::InvalidGeometry
        );
        assert!(
            second.text_diagnostics.is_empty(),
            "{:?}",
            second.text_diagnostics
        );
    }
}

fn inline_object(source: &str, anchor: i32, font_size: f32) -> RichTextBox {
    let mut content = text(source);
    content
        .spans
        .push(font_span(anchor as u32, anchor as u32 + 1, font_size));
    content.object_spans.push(RichTextObjectSpan {
        object_type: ObjectType::Image,
        object_data: Vec::new(),
        content: Some(RichTextObjectContent::Image(Box::new(
            serde_json::from_value::<PlacedImage>(serde_json::json!({
                "bbox": bounds(20.0, 100.0),
            }))
            .unwrap(),
        ))),
        text_index_utf16: anchor,
        layout_option: ObjectSpanLayoutOption::Inline,
        layout_constraint: ObjectSpanLayoutConstraint::Normal,
    });
    content
}

#[test]
fn inline_anchor_font_geometry_is_validated_where_its_leading_is_used() {
    for (source, anchor) in [("\u{fffc}", 0), ("A\n\u{fffc}", 2)] {
        for font_size in [20.0, f32::NAN] {
            let mut page = page(0, 800);
            page.objects
                .push(PageElement::TextBox(inline_object(source, anchor, font_size)).into());
            let document = Document {
                pages: vec![page],
                metadata: Default::default(),
            };
            let layout = sdocx::layout_document(&document);
            for replay in [false, true] {
                let rendered = if replay {
                    sdocx::render_layout_page_replay_svg(&document, &layout, 0, &Default::default())
                } else {
                    sdocx::render_layout_page_svg(&document, &layout, 0, &Default::default())
                }
                .unwrap();
                let geometry = rendered
                    .text_diagnostics
                    .iter()
                    .filter(|issue| issue.kind == TextDiagnosticKind::InvalidGeometry)
                    .count();
                assert_eq!(
                    geometry,
                    usize::from(font_size.is_nan()),
                    "{source:?}, replay={replay}"
                );
                assert_eq!(
                    visible_source(&rendered.svg),
                    if anchor == 0 { "" } else { "A" }
                );
            }
        }
    }
}
