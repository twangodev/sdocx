#![cfg(feature = "render")]

use resvg::usvg;
use sdocx::fonts::FontBook;
use sdocx::{
    BoundingBox, Color, Document, DocumentMetadata, Page, PageElement, RenderedPage, RichTextBox,
    RichTextParagraph, RichTextParagraphType, RichTextRun, RichTextSpan, RichTextSpanType,
};

#[derive(Clone, Copy, Debug)]
enum Context {
    Placed,
    Flow,
}

const CONTEXTS: &[Context] = &[Context::Placed, Context::Flow];
const SIZE: f64 = 45.0;
const UNITS: f64 = 2048.0;

fn text(value: &str) -> RichTextBox {
    RichTextBox {
        text_area_type: None,
        bbox: BoundingBox {
            x_min: 10.0,
            y_min: 20.0,
            x_max: 510.0,
            y_max: 620.0,
        },
        rotation_degrees: None,
        text: value.into(),
        color: Some(Color { r: 0, g: 0, b: 0 }),
        highlight_color: None,
        underline: false,
        font_size: Some(SIZE as f32),
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
        expand: false,
        payload: payload.into(),
    }
}

fn document(context: Context, mut content: RichTextBox, width: u32) -> Document {
    if matches!(context, Context::Flow) {
        content.bbox = BoundingBox::default();
    } else {
        content.bbox.x_max = content.bbox.x_min + f64::from(width);
    }
    Document {
        metadata: DocumentMetadata {
            default_page_dimensions: Some((360, 800)),
            orientation: Some(0),
            ..Default::default()
        },
        pages: vec![Page {
            uuid: "positions".into(),
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
    }
}

fn render(context: Context, content: RichTextBox, width: u32) -> RenderedPage {
    sdocx::render_page_svg(&document(context, content, width), 0, &Default::default()).unwrap()
}

fn origin(context: Context) -> (f64, f64) {
    match context {
        Context::Placed => (10.0, 65.0),
        Context::Flow => (48.0, 45.0),
    }
}

#[derive(Debug)]
struct Glyph {
    id: u16,
    text: String,
    x: f64,
    y: f64,
}

fn collect_glyphs(group: &usvg::Group, glyphs: &mut Vec<Glyph>) {
    for node in group.children() {
        match node {
            usvg::Node::Group(group) => collect_glyphs(group, glyphs),
            usvg::Node::Text(text) => {
                for span in text.layouted() {
                    for glyph in &span.positioned_glyphs {
                        let transform = glyph.transform();
                        glyphs.push(Glyph {
                            id: glyph.id.0,
                            text: glyph.text.clone(),
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

fn glyphs_with_fonts(page: &RenderedPage, fonts: &FontBook) -> Vec<Glyph> {
    let options = usvg::Options {
        fontdb: fonts.database(),
        ..Default::default()
    };
    let tree = usvg::Tree::from_str(&page.svg, &options).unwrap();
    let mut glyphs = Vec::new();
    collect_glyphs(tree.root(), &mut glyphs);
    glyphs
}

fn glyphs(page: &RenderedPage) -> Vec<Glyph> {
    glyphs_with_fonts(page, &FontBook::default())
}

fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 0.0001, "{actual} != {expected}");
}

fn svg_text(svg: &str) -> String {
    let xml = roxmltree::Document::parse(svg).unwrap();
    xml.descendants()
        .filter(|node| {
            node.is_text()
                && node
                    .parent()
                    .is_some_and(|parent| parent.has_tag_name("tspan"))
        })
        .filter_map(|node| node.text())
        .collect()
}

#[test]
fn native_latin_ligature_policy_reaches_the_svg_pdf_shaper() {
    for &context in CONTEXTS {
        let page = render(context, text("office"), 500);
        assert!(
            page.text_diagnostics.is_empty(),
            "{context:?}: {:?}",
            page.text_diagnostics
        );
        let actual = glyphs(&page);
        assert_eq!(
            actual.iter().map(|glyph| glyph.id).collect::<Vec<_>>(),
            [84, 75, 75, 78, 72, 74]
        );
        assert_eq!(
            actual
                .iter()
                .map(|glyph| glyph.text.as_str())
                .collect::<Vec<_>>(),
            ["o", "f", "f", "i", "c", "e"]
        );
        assert_eq!(svg_text(&page.svg), "office");
        let xml = roxmltree::Document::parse(&page.svg).unwrap();
        assert_eq!(
            xml.descendants()
                .filter(|node| node.has_tag_name("tspan"))
                .count(),
            1
        );
    }
}

#[test]
fn positioned_latin_glyphs_keep_paragraph_kerning() {
    for &context in CONTEXTS {
        let page = render(context, text("AV"), 500);
        let actual = glyphs(&page);
        assert_eq!(
            actual.iter().map(|glyph| glyph.id).collect::<Vec<_>>(),
            [38, 59]
        );
        let (left, baseline) = origin(context);
        close(actual[0].x, left);
        close(actual[1].x, left + 1249.0 / UNITS * SIZE);
        close(actual[0].y, baseline);
        close(actual[1].y, baseline);
    }
}

#[test]
fn explicit_spacing_reaches_positioned_glyphs_in_placed_and_flow_text() {
    for &context in CONTEXTS {
        for (kind, spacing, first, second) in [
            (0_u32, 4.0_f32, 33.25, 82.25),
            (1, 1.5, 51.75, 119.25),
            (1, 0.5, 6.75, 29.25),
        ] {
            let mut content = text("AV\nAV");
            content.paragraphs.push(RichTextParagraph {
                kind: RichTextParagraphType::LineSpacing,
                start_paragraph: 0,
                end_paragraph: 2,
                payload: [kind.to_le_bytes(), spacing.to_le_bytes()].concat(),
            });
            let page = render(context, content, 500);
            assert!(page.text_diagnostics.is_empty());
            let actual = glyphs(&page);
            assert_eq!(actual.len(), 4);
            let (left, _) = origin(context);
            let top = match context {
                Context::Placed => 20.0,
                Context::Flow => 0.0,
            };
            for (line, baseline) in actual.as_chunks::<2>().0.iter().zip([first, second]) {
                close(line[0].x, left);
                close(line[1].x, left + 1249.0 / UNITS * SIZE);
                close(line[0].y, top + baseline);
                close(line[1].y, top + baseline);
            }
            assert_eq!(svg_text(&page.svg), "AVAV");
        }
    }
}

#[test]
fn narrow_boxes_wrap_oversized_clusters_without_inflating_available_width() {
    for &context in CONTEXTS {
        for alignment in [0_u32, 1, 2] {
            let mut content = text("ii");
            content.paragraphs.push(RichTextParagraph {
                kind: RichTextParagraphType::Alignment,
                start_paragraph: 0,
                end_paragraph: 1,
                payload: alignment.to_le_bytes().to_vec(),
            });
            let page = render(context, content, 1);
            let actual = glyphs(&page);
            assert_eq!(actual.len(), 2);
            let (left, baseline) = origin(context);
            for (index, glyph) in actual.iter().enumerate() {
                close(glyph.x, left);
                close(glyph.y, baseline + index as f64 * SIZE * 1.35);
            }
            assert_eq!(svg_text(&page.svg), "ii");
        }
    }
}

#[test]
fn combining_mark_keeps_native_glyph_offsets_inside_its_cluster() {
    for &context in CONTEXTS {
        let page = render(context, text("x\u{301}z"), 500);
        assert!(page.text_diagnostics.is_empty());
        let actual = glyphs(&page);
        assert_eq!(
            actual.iter().map(|glyph| glyph.id).collect::<Vec<_>>(),
            [93, 434, 95]
        );
        let (left, baseline) = origin(context);
        close(actual[0].x, left);
        close(actual[1].x, left + 1072.0 / UNITS * SIZE);
        close(actual[1].y, baseline + 10.0 / UNITS * SIZE);
        close(actual[2].x, left + 1015.0 / UNITS * SIZE);
        assert_eq!(svg_text(&page.svg), "x\u{301}z");
    }
}

#[test]
fn combining_cluster_keeps_its_composed_glyph_and_original_unicode() {
    for &context in CONTEXTS {
        let page = render(context, text("e\u{301}x"), 500);
        assert!(page.text_diagnostics.is_empty());
        let actual = glyphs(&page);
        assert_eq!(
            actual.iter().map(|glyph| glyph.id).collect::<Vec<_>>(),
            [2317, 93]
        );
        assert_eq!(actual[0].text, "e\u{301}");
        assert_eq!(svg_text(&page.svg), "e\u{301}x");
        let (left, _) = origin(context);
        close(actual[1].x, left + 1085.0 / UNITS * SIZE);
    }
}

#[test]
fn resolved_color_transitions_define_native_shaping_boundaries() {
    for &context in CONTEXTS {
        let mut content = text("AV");
        content.runs.push(RichTextRun {
            start: 0,
            end: 1,
            bold: false,
            italic: false,
        });
        content.spans.push(span(
            RichTextSpanType::ForegroundColor,
            0,
            1,
            &[0, 0, 0, 255],
        ));
        let same = glyphs(&render(context, content.clone(), 500));
        let (left, _) = origin(context);
        close(same[1].x, left + 1249.0 / UNITS * SIZE);
        content.spans.push(span(
            RichTextSpanType::ForegroundColor,
            1,
            2,
            &[0, 0, 255, 255],
        ));
        let changed = glyphs(&render(context, content, 500));
        close(changed[1].x, left + 1336.0 / UNITS * SIZE);
    }
}

#[test]
fn emergency_wrap_reuses_the_retained_paragraph_glyph_positions() {
    for &context in CONTEXTS {
        let page = render(context, text("AVA"), 55);
        let actual = glyphs(&page);
        assert_eq!(
            actual
                .iter()
                .map(|glyph| glyph.text.as_str())
                .collect::<Vec<_>>(),
            ["A", "V", "A"]
        );
        let (left, baseline) = origin(context);
        close(actual[0].x, left);
        close(actual[1].x, left + 1249.0 / UNITS * SIZE);
        close(actual[1].y, baseline);
        close(actual[2].x, left);
        close(actual[2].y, baseline + SIZE * f64::from(1.35_f32));
        assert_eq!(svg_text(&page.svg), "AVA");
    }
}

#[test]
fn alignment_moves_the_complete_line_instead_of_anchoring_each_glyph() {
    for &context in CONTEXTS {
        for (alignment, fraction) in [(1_u32, 1.0), (2_u32, 0.5)] {
            let mut content = text("AV");
            content.paragraphs.push(RichTextParagraph {
                kind: RichTextParagraphType::Alignment,
                start_paragraph: 0,
                end_paragraph: 1,
                payload: alignment.to_le_bytes().to_vec(),
            });
            let actual = glyphs(&render(context, content, 500));
            let (left, _) = origin(context);
            let aligned = left + (500.0 - 2552.0 / UNITS * SIZE) * fraction;
            close(actual[0].x, aligned);
            close(actual[1].x, aligned + 1249.0 / UNITS * SIZE);
        }
    }
}

#[test]
fn decorations_cover_retained_advances_continuously_across_paint_spans() {
    for &context in CONTEXTS {
        let mut content = text("AV");
        content.underline = true;
        content
            .spans
            .push(span(RichTextSpanType::Strikethrough, 0, 2, &[1, 0]));
        content.spans.push(span(
            RichTextSpanType::ForegroundColor,
            0,
            1,
            &[0, 0, 0, 255],
        ));
        let page = render(context, content, 500);
        let xml = roxmltree::Document::parse(&page.svg).unwrap();
        let rectangles = xml
            .descendants()
            .filter(|node| node.has_tag_name("rect") && node.attribute("fill") == Some("#000000"))
            .collect::<Vec<_>>();
        assert_eq!(rectangles.len(), 4);
        let (left, baseline) = origin(context);
        let number = |node: roxmltree::Node<'_, '_>, name| {
            node.attribute(name).unwrap().parse::<f64>().unwrap()
        };
        for (offset, pair) in [
            (SIZE / 9.0, [rectangles[0], rectangles[2]]),
            (-2.0 * SIZE / 7.0, [rectangles[1], rectangles[3]]),
        ] {
            close(number(pair[0], "x"), left);
            close(
                number(pair[0], "x") + number(pair[0], "width"),
                number(pair[1], "x"),
            );
            close(
                number(pair[1], "x") + number(pair[1], "width"),
                left + 2552.0 / UNITS * SIZE,
            );
            for rectangle in pair {
                close(number(rectangle, "y"), baseline + offset);
                close(number(rectangle, "height"), SIZE / 18.0);
            }
        }
        assert_eq!(svg_text(&page.svg), "AV");
        assert!(
            xml.descendants()
                .filter(|node| node.has_tag_name("tspan"))
                .all(|node| node.attribute("text-decoration").is_none())
        );
    }
}

#[test]
fn unsupported_rtl_clusters_keep_logical_source_and_report_the_fallback() {
    for &context in CONTEXTS {
        let source = "A\u{202e}AV\u{202c}Z";
        let page = render(context, text(source), 500);
        assert_eq!(svg_text(&page.svg), source);
        assert!(
            page.text_diagnostics
                .iter()
                .any(|diagnostic| diagnostic.kind
                    == sdocx::TextDiagnosticKind::UnsupportedGlyphPositioning),
            "{context:?}: {:?}",
            page.text_diagnostics
        );
        let xml = roxmltree::Document::parse(&page.svg).unwrap();
        assert!(
            xml.descendants()
                .filter(|node| node.has_tag_name("tspan"))
                .all(|node| node.attribute("x").is_none())
        );
    }
}

#[test]
fn missing_arabic_glyphs_explain_the_fallback_without_duplicate_diagnostics() {
    for &context in CONTEXTS {
        let source = "Aب\u{64e}ت\u{64e}Z";
        let page = render(context, text(source), 500);
        assert_eq!(svg_text(&page.svg), source);
        assert!(
            page.text_diagnostics
                .iter()
                .any(|diagnostic| diagnostic.kind == sdocx::TextDiagnosticKind::MissingGlyphs)
        );
        assert!(
            !page
                .text_diagnostics
                .iter()
                .any(|diagnostic| diagnostic.kind
                    == sdocx::TextDiagnosticKind::UnsupportedGlyphPositioning)
        );
        let xml = roxmltree::Document::parse(&page.svg).unwrap();
        assert!(
            xml.descendants()
                .filter(|node| node.has_tag_name("tspan"))
                .all(|node| node.attribute("x").is_none())
        );
    }
}

#[test]
fn caller_font_database_controls_both_svg_positions_and_export_shaping() {
    use sdocx::fonts::fontdb;
    use std::sync::Arc;

    let bytes = include_bytes!("../assets/fonts/RobotoMono-Regular.ttf");
    let face = rustybuzz::ttf_parser::Face::parse(bytes, 0).unwrap();
    let advance = f64::from(
        face.glyph_hor_advance(face.glyph_index('A').unwrap())
            .unwrap(),
    ) / f64::from(face.units_per_em())
        * SIZE;
    let mut database = fontdb::Database::new();
    database.load_font_data(bytes.to_vec());
    database.set_sans_serif_family("Roboto Mono");
    let fonts = FontBook::new(Arc::new(database));
    for &context in CONTEXTS {
        let document = document(context, text("ABC"), 500);
        let page =
            sdocx::render_document_svg_with_fonts(&document, &Default::default(), &fonts).remove(0);
        let actual = glyphs_with_fonts(&page, &fonts);
        let (left, _) = origin(context);
        for (index, glyph) in actual.iter().enumerate() {
            close(glyph.x, left + advance * index as f64);
        }
        assert_eq!(
            actual
                .iter()
                .map(|glyph| glyph.text.as_str())
                .collect::<Vec<_>>(),
            ["A", "B", "C"]
        );
        assert_eq!(svg_text(&page.svg), "ABC");
        #[cfg(feature = "pdf")]
        {
            let bytes =
                sdocx::render_svg_pages_pdf(&[page], &sdocx::PdfOptions::new(fonts.database()))
                    .unwrap();
            let pdf = lopdf::Document::load_mem(&bytes).unwrap();
            assert_eq!(pdf.extract_text(&[1]).unwrap().replace('\n', ""), "ABC");
        }
    }
}

#[cfg(feature = "pdf")]
#[test]
fn positioned_latin_and_combining_text_remain_selectable_in_pdf() {
    for &context in CONTEXTS {
        let source = "office AV e\u{301}";
        let page = render(context, text(source), 500);
        let fonts = FontBook::default();
        let bytes = sdocx::render_svg_pages_pdf(&[page], &sdocx::PdfOptions::new(fonts.database()))
            .unwrap();
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        assert_eq!(pdf.extract_text(&[1]).unwrap().replace('\n', ""), source);
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
