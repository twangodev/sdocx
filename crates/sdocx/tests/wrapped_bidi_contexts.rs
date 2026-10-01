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
fn wrapped_even_override_and_isolate_keep_proven_positions_and_backgrounds() {
    for context in [Context::Placed, Context::Flow] {
        for replay in [false, true] {
            for (open, close) in [('\u{202d}', '\u{202c}'), ('\u{2066}', '\u{2069}')] {
                let source = format!("A{open}BC{close}Z");
                let page = render(context, &source, 14, replay);
                assert!(
                    page.text_diagnostics.is_empty(),
                    "{context:?} {source:?}: {:?}",
                    page.text_diagnostics
                );
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
                assert!((y(continuation) - y(first) - 27.0).abs() < 0.0001);
                let left = match context {
                    Context::Placed => 10.0,
                    Context::Flow => 48.0,
                };
                assert!(
                    (continuation
                        .attribute("x")
                        .unwrap()
                        .split_whitespace()
                        .next()
                        .unwrap()
                        .parse::<f64>()
                        .unwrap()
                        - left)
                        .abs()
                        < 0.0001
                );
                let background = xml
                    .descendants()
                    .find(|node| {
                        node.has_tag_name("rect")
                            && node.attribute("fill") == Some("#00ff00")
                            && (y(node) - (y(continuation) - 20.0)).abs() < 0.0001
                    })
                    .unwrap();
                assert!(
                    (background
                        .attribute("width")
                        .unwrap()
                        .parse::<f64>()
                        .unwrap()
                        - 1333.0 / 2048.0 * 20.0)
                        .abs()
                        < 0.0001
                );
                assert!(
                    (background
                        .attribute("height")
                        .unwrap()
                        .parse::<f64>()
                        .unwrap()
                        - 27.0)
                        .abs()
                        < 0.0001
                );
            }
        }
    }
}

#[test]
fn unclosed_override_stops_at_each_native_cr_and_lf_boundary() {
    for context in [Context::Placed, Context::Flow] {
        for replay in [false, true] {
            let page = render(context, "\u{202d}B\rC\nD\r\nE", 100, replay);
            assert!(
                page.text_diagnostics.is_empty(),
                "{context:?}: {:?}",
                page.text_diagnostics
            );
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
                4
            );
        }
    }
}

#[test]
fn unproven_continuation_keeps_the_guard_from_an_earlier_line() {
    for context in [Context::Placed, Context::Flow] {
        for replay in [false, true] {
            for (open, close) in [('\u{202d}', '\u{202c}'), ('\u{2066}', '\u{2069}')] {
                let source = format!("A{open}BB C中{close}Z");
                let page = render(context, &source, 30, replay);
                assert!(diagnostic(&page, TextDiagnosticKind::MissingGlyphs, '中'));
                assert!(
                    diagnostic(&page, TextDiagnosticKind::UnsupportedGlyphPositioning, 'C'),
                    "{context:?} {source:?}: {:?}",
                    page.text_diagnostics
                );
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
                let containing = |scalar| {
                    spans
                        .iter()
                        .find(|node| node.text().is_some_and(|text| text.contains(scalar)))
                        .unwrap()
                };
                let y = |node: &roxmltree::Node<'_, '_>| {
                    node.attribute("y").unwrap().parse::<f64>().unwrap()
                };
                assert_eq!(y(containing('C')), y(containing('中')));
                assert!(y(containing('C')) > y(containing(open)));
                assert!(!xml.descendants().any(|node| node.has_tag_name("rect")
                    && node.attribute("fill") == Some("#00ff00")
                    && (y(&node) - (y(containing('C')) - 20.0)).abs() < 0.0001));
            }
        }
    }
}

#[test]
fn fallback_after_cr_or_lf_does_not_inherit_the_previous_paragraph_override() {
    for context in [Context::Placed, Context::Flow] {
        for replay in [false, true] {
            for separator in ["\r", "\n", "\r\n"] {
                let page = render(context, &format!("\u{202d}B{separator}C中"), 100, replay);
                assert!(diagnostic(&page, TextDiagnosticKind::MissingGlyphs, '中'));
                assert!(
                    !page.text_diagnostics.iter().any(|issue| matches!(
                        issue.kind,
                        TextDiagnosticKind::UnsupportedGlyphPositioning
                            | TextDiagnosticKind::UnsupportedBackgroundPositioning
                    )),
                    "{context:?} {separator:?}: {:?}",
                    page.text_diagnostics
                );
                let xml = roxmltree::Document::parse(&page.svg).unwrap();
                let continuation = xml
                    .descendants()
                    .find(|node| {
                        node.has_tag_name("tspan")
                            && node.text().is_some_and(|text| text.contains('C'))
                    })
                    .unwrap();
                let baseline = continuation.attribute("y").unwrap().parse::<f64>().unwrap();
                assert!(xml.descendants().any(|node| {
                    node.has_tag_name("rect")
                        && node.attribute("fill") == Some("#00ff00")
                        && (node.attribute("y").unwrap().parse::<f64>().unwrap()
                            - (baseline - 20.0))
                            .abs()
                            < 0.0001
                }));
            }
        }
    }
}
