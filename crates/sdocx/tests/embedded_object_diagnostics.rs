#![cfg(feature = "render")]

use sdocx::{
    BoundingBox, Document, DocumentMetadata, ObjectDiagnostic, ObjectDiagnosticKind,
    ObjectSpanLayoutConstraint, ObjectSpanLayoutOption, ObjectType, Page, PageElement,
    RenderedPage, RichTextBox, RichTextCodeBlock, RichTextObjectContent, RichTextObjectSpan,
};

fn text(value: &str, object_spans: Vec<RichTextObjectSpan>) -> RichTextBox {
    RichTextBox {
        text_area_type: None,
        bbox: BoundingBox::default(),
        rotation_degrees: None,
        text: value.into(),
        color: None,
        highlight_color: None,
        underline: false,
        font_size: Some(15.0),
        runs: Vec::new(),
        spans: Vec::new(),
        paragraphs: Vec::new(),
        object_spans,
        text_sections: Vec::new(),
        margins: None,
        gravity: None,
    }
}

fn code(anchor: i32) -> RichTextObjectSpan {
    RichTextObjectSpan {
        object_type: ObjectType::CodeBlock,
        object_data: Vec::new(),
        content: Some(RichTextObjectContent::CodeBlock(Box::new(
            RichTextCodeBlock {
                bbox: BoundingBox {
                    x_min: 20.0,
                    y_min: 20.0,
                    x_max: 420.0,
                    y_max: 420.0,
                },
                rotation_degrees: None,
                title: None,
                body: Some(text("OBJECT CONTENT", Vec::new())),
            },
        ))),
        text_index_utf16: anchor,
        layout_option: ObjectSpanLayoutOption::Inline,
        layout_constraint: ObjectSpanLayoutConstraint::Normal,
    }
}

fn unsupported(anchor: i32) -> RichTextObjectSpan {
    RichTextObjectSpan {
        content: None,
        ..code(anchor)
    }
}

fn invalid_bounds(anchor: i32) -> RichTextObjectSpan {
    let mut span = code(anchor);
    let Some(RichTextObjectContent::CodeBlock(code)) = &mut span.content else {
        panic!()
    };
    code.bbox.x_max = code.bbox.x_min;
    span
}

fn document(value: &str, object_spans: Vec<RichTextObjectSpan>) -> Document {
    Document {
        pages: vec![Page {
            uuid: "object-diagnostics".into(),
            width: 1080,
            height: 1527,
            content_bbox: BoundingBox::default(),
            background_color: None,
            template: None,
            background: Default::default(),
            objects: vec![PageElement::TextBox(text(value, object_spans)).into()],
        }],
        metadata: DocumentMetadata {
            default_page_dimensions: Some((1080, 1527)),
            orientation: Some(0),
            ..Default::default()
        },
    }
}

fn modes(document: &Document) -> [RenderedPage; 2] {
    let layout = sdocx::layout_document(document);
    [
        sdocx::render_layout_page_svg(document, &layout, 0, &Default::default()).unwrap(),
        sdocx::render_layout_page_replay_svg(document, &layout, 0, &Default::default()).unwrap(),
    ]
}

fn selectable_text(svg: &str) -> String {
    let xml = roxmltree::Document::parse(svg).unwrap();
    xml.descendants()
        .filter(|node| node.has_tag_name("tspan"))
        .filter_map(|node| node.text())
        .collect()
}

fn assert_no_object_paint(svg: &str) {
    let xml = roxmltree::Document::parse(svg).unwrap();
    assert!(
        !xml.descendants()
            .any(|node| node.attribute("data-sdocx-object") == Some("code-block"))
    );
    assert!(!selectable_text(svg).contains("OBJECT CONTENT"));
}

#[test]
fn valid_solitary_object_is_painted_and_neighboring_paragraph_text_survives() {
    for page in modes(&document("\u{fffc}\nA😀e\u{301}", vec![code(0)])) {
        assert!(page.object_diagnostics.is_empty());
        assert_eq!(selectable_text(&page.svg), "OBJECT CONTENTA😀e\u{301}");
        let xml = roxmltree::Document::parse(&page.svg).unwrap();
        assert!(
            xml.descendants()
                .any(|node| node.attribute("data-sdocx-object") == Some("code-block"))
        );
    }
}

#[test]
fn malformed_and_nonreplacement_anchors_preserve_adjacent_unicode() {
    let source = "A😀\u{fffc}e\u{301} B";
    for (anchor, kind) in [
        (-1, ObjectDiagnosticKind::InvalidAnchor),
        (2, ObjectDiagnosticKind::InvalidAnchor),
        (8, ObjectDiagnosticKind::InvalidAnchor),
        (99, ObjectDiagnosticKind::InvalidAnchor),
        (0, ObjectDiagnosticKind::NonReplacementAnchor),
        (1, ObjectDiagnosticKind::NonReplacementAnchor),
        (4, ObjectDiagnosticKind::NonReplacementAnchor),
    ] {
        for page in modes(&document(source, vec![code(anchor)])) {
            assert_eq!(
                page.object_diagnostics,
                vec![ObjectDiagnostic {
                    anchor_utf16: anchor,
                    kind
                }]
            );
            assert_eq!(selectable_text(&page.svg), source);
            assert_no_object_paint(&page.svg);
        }
    }
}

#[test]
fn unsupported_content_and_invalid_geometry_preserve_source() {
    let source = "A😀\u{fffc}e\u{301} B";
    for (span, kind) in [
        (unsupported(3), ObjectDiagnosticKind::UnsupportedContent),
        (invalid_bounds(3), ObjectDiagnosticKind::InvalidBounds),
    ] {
        for page in modes(&document(source, vec![span])) {
            assert_eq!(
                page.object_diagnostics,
                vec![ObjectDiagnostic {
                    anchor_utf16: 3,
                    kind
                }]
            );
            assert_eq!(selectable_text(&page.svg), source);
            assert_no_object_paint(&page.svg);
        }
    }
}

#[test]
fn mixed_valid_objects_report_limits_without_discarding_neighbor_text() {
    let source = "A😀\u{fffc}e\u{301}\u{fffc}B";
    for page in modes(&document(source, vec![code(6), code(3)])) {
        assert_eq!(
            page.object_diagnostics,
            vec![
                ObjectDiagnostic {
                    anchor_utf16: 3,
                    kind: ObjectDiagnosticKind::MixedParagraphLayout
                },
                ObjectDiagnostic {
                    anchor_utf16: 6,
                    kind: ObjectDiagnosticKind::MixedParagraphLayout
                },
            ]
        );
        assert_eq!(selectable_text(&page.svg), source);
        assert_no_object_paint(&page.svg);
    }
}

#[test]
fn last_invalid_duplicate_does_not_resurrect_an_earlier_valid_object() {
    for (last, kind) in [
        (unsupported(0), ObjectDiagnosticKind::UnsupportedContent),
        (invalid_bounds(0), ObjectDiagnosticKind::InvalidBounds),
    ] {
        let doc = document("\u{fffc}\nA😀e\u{301}", vec![code(0), last]);
        for page in modes(&doc) {
            assert_eq!(
                page.object_diagnostics,
                vec![ObjectDiagnostic {
                    anchor_utf16: 0,
                    kind
                }]
            );
            assert_eq!(selectable_text(&page.svg), "\u{fffc}A😀e\u{301}");
            assert_no_object_paint(&page.svg);
        }
    }
}

#[cfg(feature = "serde")]
#[test]
fn rendered_page_serializes_typed_object_diagnostics_without_losing_source() {
    let page = modes(&document("A😀\u{fffc}B", vec![unsupported(3)]))
        .into_iter()
        .next()
        .unwrap();
    let json = serde_json::to_value(&page).unwrap();
    assert_eq!(
        json["object_diagnostics"],
        serde_json::json!([
            { "anchor_utf16": 3, "kind": "UnsupportedContent" }
        ])
    );
    let decoded: RenderedPage = serde_json::from_value(json).unwrap();
    assert_eq!(decoded, page);
    assert_eq!(selectable_text(&decoded.svg), "A😀\u{fffc}B");
}
