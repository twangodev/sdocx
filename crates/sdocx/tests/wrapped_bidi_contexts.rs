#![cfg(feature = "render")]

use sdocx::{
    BoundingBox, Color, Document, DocumentMetadata, Page, PageElement, RenderedPage, RichTextBox,
    RichTextSpan, RichTextSpanType, TextDiagnosticKind,
};

#[derive(Clone, Copy, Debug)]
enum Context {
    Placed,
    Flow,
}

fn render(context: Context, source: &str, width: u32, replay: bool) -> RenderedPage {
    let mut text = RichTextBox {
        text_area_type: None,
        bbox: BoundingBox {
            x_min: 10.0,
            y_min: 20.0,
            x_max: 10.0 + f64::from(width),
            y_max: 320.0,
        },
        rotation_degrees: None,
        text: source.into(),
        color: Some(Color { r: 0, g: 0, b: 0 }),
        highlight_color: None,
        underline: false,
        font_size: Some(20.0),
        runs: Vec::new(),
        spans: vec![RichTextSpan {
            kind: RichTextSpanType::BackgroundColor,
            start_utf16: 0,
            end_utf16: source.encode_utf16().count() as u32,
            interval_type: sdocx::SpanIntervalType::ClosedOpen,
            payload: 0xff00ff00_u32.to_le_bytes().to_vec(),
        }],
        paragraphs: Vec::new(),
        object_spans: Vec::new(),
        text_sections: Vec::new(),
        margins: None,
        gravity: None,
    };
    if matches!(context, Context::Flow) {
        text.bbox = BoundingBox::default();
    }
    let document = Document {
        metadata: DocumentMetadata {
            default_page_dimensions: Some((360, 400)),
            orientation: Some(0),
            ..Default::default()
        },
        pages: vec![Page {
            uuid: "wrapped-bidi-contexts".into(),
            width: width + 96,
            height: 400,
            content_bbox: text.bbox,
            background_color: Some(Color {
                r: 255,
                g: 255,
                b: 255,
            }),
            template: None,
            background: Default::default(),
            objects: vec![PageElement::TextBox(text).into()],
        }],
    };
    let layout = sdocx::layout_document(&document);
    if replay {
        sdocx::render_layout_page_replay_svg(&document, &layout, 0, &Default::default())
    } else {
        sdocx::render_layout_page_svg(&document, &layout, 0, &Default::default())
    }
    .unwrap()
}

fn diagnostic(page: &RenderedPage, kind: TextDiagnosticKind, scalar: char) -> bool {
    page.text_diagnostics
        .iter()
        .any(|issue| issue.kind == kind && issue.codepoints.contains(&u32::from(scalar)))
}

#[test]
fn wrapped_even_override_and_isolate_keep_their_paragraph_guard() {
    for context in [Context::Placed, Context::Flow] {
        for replay in [false, true] {
            for (open, close) in [('\u{202d}', '\u{202c}'), ('\u{2066}', '\u{2069}')] {
                let source = format!("A{open}BC{close}Z");
                let page = render(context, &source, 14, replay);
                assert!(
                    diagnostic(&page, TextDiagnosticKind::UnsupportedGlyphPositioning, 'C'),
                    "{context:?} {source:?}: {:?}",
                    page.text_diagnostics
                );
                assert!(page.text_diagnostics.iter().any(
                    |issue| issue.kind == TextDiagnosticKind::UnsupportedBackgroundPositioning
                ));
                for scalar in ['A', 'Z'] {
                    assert!(!diagnostic(
                        &page,
                        TextDiagnosticKind::UnsupportedGlyphPositioning,
                        scalar
                    ));
                }
                assert!(!page.text_diagnostics.iter().any(|issue| matches!(
                    issue.kind,
                    TextDiagnosticKind::MissingGlyphs | TextDiagnosticKind::MeasurementFailure
                )));
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
                    source
                );
                let first = spans
                    .iter()
                    .find(|node| node.text().is_some_and(|text| text.contains('B')))
                    .unwrap();
                let continuation = spans
                    .iter()
                    .find(|node| node.text().is_some_and(|text| text.contains('C')))
                    .unwrap();
                let y = |node: &roxmltree::Node<'_, '_>| {
                    node.attribute("y").unwrap().parse::<f64>().unwrap()
                };
                assert!(y(continuation) > y(first));
                assert!(!xml.descendants().any(|node| node.has_tag_name("rect")
                    && node.attribute("fill") == Some("#00ff00")
                    && (y(&node) - (y(continuation) - 20.0)).abs() < 0.0001));
            }
        }
    }
}

#[test]
fn unclosed_override_stops_at_each_native_cr_and_lf_boundary() {
    for context in [Context::Placed, Context::Flow] {
        for replay in [false, true] {
            let page = render(context, "\u{202d}B\rC\nD\r\nE", 100, replay);
            assert!(diagnostic(
                &page,
                TextDiagnosticKind::UnsupportedGlyphPositioning,
                'B'
            ));
            assert!(page.text_diagnostics.iter().any(|issue| issue.kind == TextDiagnosticKind::UnsupportedBackgroundPositioning));
            for scalar in ['C', 'D', 'E'] {
                assert!(
                    !diagnostic(
                        &page,
                        TextDiagnosticKind::UnsupportedGlyphPositioning,
                        scalar
                    ),
                    "{context:?}: {:?}",
                    page.text_diagnostics
                );
            }
            let xml = roxmltree::Document::parse(&page.svg).unwrap();
            assert_eq!(
                xml.descendants()
                    .filter(|node| node.has_tag_name("tspan"))
                    .filter_map(|node| node.text())
                    .collect::<String>(),
                "\u{202d}BCDE"
            );
            assert_eq!(
                xml.descendants()
                    .filter(|node| node.has_tag_name("rect")
                        && node.attribute("fill") == Some("#00ff00"))
                    .count(),
                3
            );
        }
    }
}
