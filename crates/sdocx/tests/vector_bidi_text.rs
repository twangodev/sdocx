#![cfg(feature = "render")]

use resvg::usvg;
use sdocx::fonts::FontBook;
use sdocx::{
    BoundingBox, Color, Document, DocumentMetadata, Page, PageElement, RenderedPage, RichTextBox,
    RichTextSpan, RichTextSpanType, SpanIntervalType, TextDiagnosticKind,
};

fn text(source: &str) -> RichTextBox {
    RichTextBox {
        text_area_type: None,
        bbox: BoundingBox {
            x_min: 10.0,
            y_min: 20.0,
            x_max: 510.0,
            y_max: 620.0,
        },
        rotation_degrees: None,
        text: source.into(),
        color: Some(Color { r: 0, g: 0, b: 0 }),
        highlight_color: None,
        underline: false,
        font_size: Some(45.0),
        runs: Vec::new(),
        spans: Vec::new(),
        paragraphs: Vec::new(),
        object_spans: Vec::new(),
        text_sections: Vec::new(),
        margins: None,
        gravity: None,
    }
}

fn span(kind: RichTextSpanType, start: u32, end: u32, payload: &[u8]) -> RichTextSpan {
    RichTextSpan {
        kind,
        start_utf16: start,
        end_utf16: end,
        interval_type: SpanIntervalType::ClosedOpen,
        payload: payload.into(),
    }
}

fn render(content: RichTextBox, width: u32, flow: bool, replay: bool) -> RenderedPage {
    render_with_fonts(content, width, flow, replay, &FontBook::default())
}

fn render_with_fonts(
    mut content: RichTextBox,
    width: u32,
    flow: bool,
    replay: bool,
    fonts: &FontBook,
) -> RenderedPage {
    content.bbox.x_max = content.bbox.x_min + f64::from(width);
    if flow {
        content.bbox = BoundingBox::default();
    }
    let document = Document {
        metadata: DocumentMetadata {
            default_page_dimensions: Some((360, 800)),
            orientation: Some(0),
            ..Default::default()
        },
        pages: vec![Page {
            uuid: "vector-bidi".into(),
            width: width + 96,
            height: 800,
            content_bbox: content.bbox,
            background_color: Some(Color {
                r: 255,
                g: 255,
                b: 255,
            }),
            template: None,
            background: Default::default(),
            objects: vec![PageElement::TextBox(content).into()],
        }],
    };
    let layout = sdocx::layout_document(&document);
    if replay {
        sdocx::render_layout_page_replay_svg_with_fonts(
            &document,
            &layout,
            0,
            &Default::default(),
            fonts,
        )
    } else {
        sdocx::render_layout_page_svg_with_fonts(&document, &layout, 0, &Default::default(), fonts)
    }
    .unwrap()
}

#[derive(Debug)]
struct Glyph {
    text: String,
    id: u16,
    x: f64,
    y: f64,
}

fn formatting(character: char) -> bool {
    matches!(character, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
}

fn collect_glyphs(group: &usvg::Group, output: &mut Vec<Glyph>) {
    for node in group.children() {
        match node {
            usvg::Node::Group(group) => collect_glyphs(group, output),
            usvg::Node::Text(text) => {
                for layout in text.layouted() {
                    for glyph in &layout.positioned_glyphs {
                        if glyph.text.chars().all(formatting) {
                            continue;
                        }
                        let transform = glyph.transform();
                        output.push(Glyph {
                            text: glyph.text.clone(),
                            id: glyph.id.0,
                            x: f64::from(transform.tx),
                            y: f64::from(transform.ty),
                        });
                    }
                }
            }
            _ => {}
        }
    }
}

fn glyphs(page: &RenderedPage) -> Vec<Glyph> {
    glyphs_with_fonts(page, &FontBook::default())
}

fn glyphs_with_fonts(page: &RenderedPage, fonts: &FontBook) -> Vec<Glyph> {
    let options = usvg::Options {
        fontdb: fonts.database(),
        ..Default::default()
    };
    let tree = usvg::Tree::from_str(&page.svg, &options).unwrap();
    let mut output = Vec::new();
    collect_glyphs(tree.root(), &mut output);
    output
}

fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 0.0001, "{actual} != {expected}");
}

fn assert_source(page: &RenderedPage, source: &str) {
    let xml = roxmltree::Document::parse(&page.svg).unwrap();
    assert_eq!(
        xml.descendants()
            .filter(|node| node.has_tag_name("tspan"))
            .filter_map(|node| node.text())
            .collect::<String>(),
        source
    );
}

#[test]
fn overrides_preserve_source_and_pinned_native_glyph_positions() {
    for (source, expected) in [
        (
            "A\u{202e}BC\u{202c}D",
            [
                ("A", 38, 0.0),
                ("B", 39, 58.64501953125),
                ("C", 40, 29.35546875),
                ("D", 41, 86.66015625),
            ],
        ),
        (
            "A\u{202e}AV\u{202c}D",
            [
                ("A", 38, 0.0),
                ("A", 38, 56.337890625),
                ("V", 59, 29.35546875),
                ("D", 41, 85.693359375),
            ],
        ),
        (
            "A\u{202d}BC\u{202c}D",
            [
                ("A", 38, 0.0),
                ("B", 39, 29.35546875),
                ("C", 40, 57.37060546875),
                ("D", 41, 86.66015625),
            ],
        ),
    ] {
        for flow in [false, true] {
            for replay in [false, true] {
                let page = render(text(source), 500, flow, replay);
                assert!(
                    page.text_diagnostics.is_empty(),
                    "{:?}",
                    page.text_diagnostics
                );
                assert_source(&page, source);
                let actual = glyphs(&page);
                assert_eq!(actual.len(), expected.len());
                let origin = if flow { 48.0 } else { 10.0 };
                for (glyph, (text, id, x)) in actual.iter().zip(expected) {
                    assert_eq!((glyph.text.as_str(), glyph.id), (text, id));
                    close(glyph.x, origin + x);
                    close(glyph.y, actual[0].y);
                }
            }
        }
    }
}

#[test]
fn wrapping_filters_paragraph_order_without_resetting_trailing_override_space() {
    let source = "A\u{202e}B C\u{202c}D";
    for flow in [false, true] {
        for replay in [false, true] {
            let page = render(text(source), 85, flow, replay);
            assert!(
                page.text_diagnostics.is_empty(),
                "{:?}",
                page.text_diagnostics
            );
            assert_source(&page, source);
            let actual = glyphs(&page);
            assert_eq!(
                actual
                    .iter()
                    .map(|glyph| glyph.text.as_str())
                    .collect::<Vec<_>>(),
                ["A", "B", " ", "C", "D"]
            );
            let origin = if flow { 48.0 } else { 10.0 };
            for (glyph, x) in
                actual
                    .iter()
                    .zip([0.0, 40.49560546875, 29.35546875, 0.0, 29.28955078125])
            {
                close(glyph.x, origin + x);
            }
            close(actual[1].y, actual[0].y);
            close(actual[2].y, actual[0].y);
            close(actual[4].y, actual[3].y);
            close(actual[3].y - actual[0].y, 60.75);
        }
    }
}

#[test]
fn rtl_style_boundaries_keep_foreground_sizes_and_native_positions() {
    let source = "A\u{202e}BC\u{202c}D";
    let mut content = text(source);
    content.spans = vec![
        span(
            RichTextSpanType::ForegroundColor,
            2,
            3,
            &0xffff0000_u32.to_le_bytes(),
        ),
        span(RichTextSpanType::FontSize, 2, 3, &60.0_f32.to_le_bytes()),
        span(
            RichTextSpanType::ForegroundColor,
            3,
            4,
            &0xff0000ff_u32.to_le_bytes(),
        ),
        span(RichTextSpanType::FontSize, 3, 4, &30.0_f32.to_le_bytes()),
    ];
    for flow in [false, true] {
        for replay in [false, true] {
            let page = render(content.clone(), 500, flow, replay);
            assert!(
                page.text_diagnostics.is_empty(),
                "{:?}",
                page.text_diagnostics
            );
            assert_source(&page, source);
            let actual = glyphs(&page);
            assert_eq!(actual.len(), 4);
            for (glyph, (id, x)) in actual.iter().zip([
                (38, 0.0),
                (39, 48.8818359375),
                (40, 29.35546875),
                (41, 86.2353515625),
            ]) {
                assert_eq!(glyph.id, id);
                close(glyph.x - actual[0].x, x);
                close(glyph.y, actual[0].y);
            }
            let xml = roxmltree::Document::parse(&page.svg).unwrap();
            for (character, color, size) in [("B", "#ff0000", 60.0), ("C", "#0000ff", 30.0)] {
                let node = xml
                    .descendants()
                    .find(|node| node.has_tag_name("tspan") && node.text() == Some(character))
                    .unwrap();
                assert_eq!(node.attribute("fill"), Some(color));
                close(node.attribute("font-size").unwrap().parse().unwrap(), size);
            }
        }
    }
}

#[test]
fn partial_underline_and_background_follow_disjoint_visual_intervals() {
    let source = "A\u{202e}BCD\u{202c}E";
    let mut content = text(source);
    content.spans = vec![
        span(RichTextSpanType::Underline, 0, 3, &[1, 0]),
        span(
            RichTextSpanType::BackgroundColor,
            0,
            3,
            &0xff00ff00_u32.to_le_bytes(),
        ),
    ];
    for flow in [false, true] {
        for replay in [false, true] {
            let page = render(content.clone(), 500, flow, replay);
            assert!(
                page.text_diagnostics.is_empty(),
                "{:?}",
                page.text_diagnostics
            );
            assert_source(&page, source);
            let actual = glyphs(&page);
            let xml = roxmltree::Document::parse(&page.svg).unwrap();
            for color in ["#000000", "#00ff00"] {
                let rects = xml
                    .descendants()
                    .filter(|node| {
                        node.has_tag_name("rect") && node.attribute("fill") == Some(color)
                    })
                    .collect::<Vec<_>>();
                assert_eq!(rects.len(), 2, "{color}");
                for (rect, (left, width)) in rects
                    .iter()
                    .zip([(0.0, 29.35546875), (88.154296875, 28.01513671875)])
                {
                    close(
                        rect.attribute("x").unwrap().parse::<f64>().unwrap(),
                        actual[0].x + left,
                    );
                    close(rect.attribute("width").unwrap().parse().unwrap(), width);
                    if color == "#000000" {
                        close(
                            rect.attribute("y").unwrap().parse::<f64>().unwrap(),
                            actual[0].y + 5.0,
                        );
                        close(rect.attribute("height").unwrap().parse().unwrap(), 2.5);
                    }
                }
            }
        }
    }
}

#[test]
fn mirrored_glyphs_and_rtl_combining_clusters_remain_explicit_fallbacks() {
    for source in ["A\u{202e}(BC)\u{202c}D", "A\u{202e}e\u{301}\u{202c}D"] {
        for flow in [false, true] {
            for replay in [false, true] {
                let page = render(text(source), 500, flow, replay);
                assert_source(&page, source);
                assert!(
                    page.text_diagnostics
                        .iter()
                        .any(|issue| issue.kind == TextDiagnosticKind::UnsupportedGlyphPositioning)
                );
                assert!(
                    !page
                        .text_diagnostics
                        .iter()
                        .any(|issue| issue.kind == TextDiagnosticKind::MissingGlyphs)
                );
            }
        }
    }
}

fn covered_test_font() -> FontBook {
    use sha2::{Digest, Sha256};
    use std::sync::Arc;

    let bytes = include_bytes!("assets/fonts/DejaVuSans.ttf");
    assert_eq!(bytes.len(), 759_720);
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "57f73e11f51999432bf7ab22ce55b6f945d5eca1bf824404cfa9ec2e3718c84e"
    );
    let mut database = sdocx::fonts::fontdb::Database::new();
    database.load_font_data(include_bytes!("../assets/fonts/Roboto-Regular.ttf").to_vec());
    database.load_font_data(bytes.to_vec());
    database.set_sans_serif_family("DejaVu Sans");
    assert_eq!(database.faces().count(), 2);
    FontBook::new(Arc::new(database))
}

fn covered_text(source: &str) -> RichTextBox {
    let mut content = text(source);
    let family = "DejaVu Sans";
    let payload = [
        vec![0; 8],
        (family.len() as u16 + 1).to_le_bytes().to_vec(),
        family.as_bytes().to_vec(),
        vec![0],
    ]
    .concat();
    content.spans.push(span(
        RichTextSpanType::FontName,
        0,
        source.encode_utf16().count() as u32,
        &payload,
    ));
    content
}

#[test]
fn covered_hebrew_scalars_and_numbers_keep_pinned_visual_positions() {
    let fonts = covered_test_font();
    let source = "אב 123";
    for flow in [false, true] {
        for replay in [false, true] {
            let page = render_with_fonts(covered_text(source), 500, flow, replay, &fonts);
            assert_source(&page, source);
            assert!(
                page.text_diagnostics.is_empty(),
                "{:?}",
                page.text_diagnostics
            );
            let actual = glyphs_with_fonts(&page, &fonts);
            let expected = [
                ("א", 1319, 126.2109375),
                ("ב", 1320, 100.1953125),
                (" ", 3, 85.89111328125),
                ("1", 20, 0.0),
                ("2", 21, 28.63037109375),
                ("3", 22, 57.2607421875),
            ];
            assert_eq!(actual.len(), expected.len());
            let origin = if flow { 48.0 } else { 10.0 };
            for (glyph, (text, id, x)) in actual.iter().zip(expected) {
                assert_eq!((glyph.text.as_str(), glyph.id), (text, id));
                close(glyph.x, origin + x);
                close(glyph.y, actual[0].y);
            }
        }
    }
}

#[test]
fn covered_arabic_clusters_and_ltr_override_keep_diagnosed_contextual_fallbacks() {
    let fonts = covered_test_font();
    for source in ["لا", "\u{202d}ال\u{202c}"] {
        for flow in [false, true] {
            for replay in [false, true] {
                let page = render_with_fonts(covered_text(source), 500, flow, replay, &fonts);
                assert_source(&page, source);
                assert!(
                    page.text_diagnostics
                        .iter()
                        .any(|issue| issue.kind == TextDiagnosticKind::UnsupportedGlyphPositioning)
                );
                assert!(
                    !page
                        .text_diagnostics
                        .iter()
                        .any(|issue| issue.kind == TextDiagnosticKind::MissingGlyphs)
                );
            }
        }
    }
}

#[test]
fn native_isolate_base_direction_moves_outer_numbers_but_not_strong_latin() {
    let fonts = covered_test_font();
    for (source, expected) in [
        (
            "\u{2067}א\u{2069}123",
            vec![
                ("א", 1319, 85.89111328125),
                ("1", 20, 0.0),
                ("2", 21, 28.63037109375),
                ("3", 22, 57.2607421875),
            ],
        ),
        (
            "\u{2068}א\u{2069}123",
            vec![
                ("א", 1319, 85.89111328125),
                ("1", 20, 0.0),
                ("2", 21, 28.63037109375),
                ("3", 22, 57.2607421875),
            ],
        ),
        (
            "\u{2066}א\u{2069}_123",
            vec![
                ("א", 1319, 108.39111328125),
                ("_", 66, 85.89111328125),
                ("1", 20, 0.0),
                ("2", 21, 28.63037109375),
                ("3", 22, 57.2607421875),
            ],
        ),
        (
            "\u{2067}א\u{2069}ABC",
            vec![
                ("א", 1319, 0.0),
                ("A", 36, 30.08056640625),
                ("B", 37, 60.8642578125),
                ("C", 38, 90.94482421875),
            ],
        ),
    ] {
        for flow in [false, true] {
            for replay in [false, true] {
                let page = render_with_fonts(covered_text(source), 500, flow, replay, &fonts);
                assert_source(&page, source);
                assert!(
                    page.text_diagnostics.is_empty(),
                    "{:?}",
                    page.text_diagnostics
                );
                let actual = glyphs_with_fonts(&page, &fonts);
                assert_eq!(actual.len(), expected.len());
                let origin = if flow { 48.0 } else { 10.0 };
                for (glyph, (text, id, x)) in actual.iter().zip(&expected) {
                    assert_eq!((glyph.text.as_str(), glyph.id), (*text, *id));
                    close(glyph.x, origin + x);
                    close(glyph.y, actual[0].y);
                }
            }
        }
    }
}

#[cfg(feature = "pdf")]
#[path = "support/pdf_geometry.rs"]
mod pdf_geometry;

#[cfg(feature = "pdf")]
mod pdf {
    use super::*;

    #[test]
    fn pdf_keeps_native_visible_positions_and_logical_control_metadata() {
        for flow in [false, true] {
            for replay in [false, true] {
                let source = "A\u{202e}BC\u{202c}D";
                let page = render(text(source), 500, flow, replay);
                assert_source(&page, source);
                let bytes = sdocx::render_svg_pages_pdf(&[page], &Default::default()).unwrap();
                let pdf = pdf_geometry::read(&bytes, 96.0);
                assert_eq!(
                    pdf.extracted_text
                        .chars()
                        .filter(|character| *character != '\n' && !formatting(*character))
                        .collect::<String>(),
                    "ABCD"
                );
                assert_eq!(pdf.image_resources, 0);
                assert!(pdf.images.is_empty());
                assert!(pdf.actual_text.iter().any(|text| text.contains('\u{202c}')));
                assert_eq!(pdf.source, source);
                let visible = pdf
                    .text
                    .iter()
                    .filter(|(text, _, _)| {
                        text.chars().any(|character| matches!(character, 'A'..='D'))
                    })
                    .collect::<Vec<_>>();
                assert_eq!(visible.len(), 4);
                let origin = if flow { 48.0 } else { 10.0 };
                for ((text, x, y), (expected, offset)) in visible.iter().zip([
                    ("A", 0.0),
                    ("B", 58.64501953125),
                    ("C", 29.35546875),
                    ("D", 86.66015625),
                ]) {
                    assert_eq!(text, expected);
                    close(*x, origin + offset);
                    close(*y, visible[0].2);
                }
            }
        }
    }
}
