#![cfg(feature = "render")]

use resvg::usvg;
use sdocx::fonts::FontBook;
use sdocx::{
    BoundingBox, Document, DocumentMetadata, Page, PageElement, RenderedPage, RichTextBox,
    RichTextParagraph, RichTextParagraphType, TextDiagnostic, TextDiagnosticKind,
};

fn text(value: &str, width: f64) -> RichTextBox {
    RichTextBox {
        text_area_type: None,
        bbox: BoundingBox {
            x_min: 10.0,
            y_min: 20.0,
            x_max: 10.0 + width,
            y_max: 620.0,
        },
        rotation_degrees: None,
        text: value.into(),
        color: None,
        highlight_color: None,
        underline: false,
        font_size: Some(45.0),
        runs: Vec::new(),
        spans: Vec::new(),
        paragraphs: vec![RichTextParagraph {
            kind: RichTextParagraphType::Alignment,
            start_paragraph: 0,
            end_paragraph: 1,
            payload: 3_u32.to_le_bytes().to_vec(),
        }],
        object_spans: Vec::new(),
        text_sections: Vec::new(),
        margins: None,
        gravity: None,
    }
}

fn render(mut content: RichTextBox, width: u32, flow: bool) -> RenderedPage {
    if flow {
        content.bbox = BoundingBox::default();
    }
    let document = Document {
        metadata: DocumentMetadata {
            default_page_dimensions: Some((360, 800)),
            ..Default::default()
        },
        pages: vec![Page {
            uuid: "justify".into(),
            width: width + 96,
            height: 800,
            content_bbox: content.bbox,
            background_color: None,
            template: None,
            background: Default::default(),
            objects: vec![PageElement::TextBox(content).into()],
        }],
    };
    sdocx::render_document_svg(&document, &Default::default())
        .pop()
        .unwrap()
}

#[derive(Debug)]
struct Glyph {
    text: String,
    id: u16,
    x: f64,
    y: f64,
}

fn collect_glyphs(group: &usvg::Group, output: &mut Vec<Glyph>) {
    for node in group.children() {
        match node {
            usvg::Node::Group(group) => collect_glyphs(group, output),
            usvg::Node::Text(text) => {
                for span in text.layouted() {
                    for glyph in &span.positioned_glyphs {
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
    let options = usvg::Options {
        fontdb: FontBook::default().database(),
        ..Default::default()
    };
    let tree = usvg::Tree::from_str(&page.svg, &options).unwrap();
    let mut glyphs = Vec::new();
    collect_glyphs(tree.root(), &mut glyphs);
    glyphs
}

fn source(page: &RenderedPage) -> String {
    let xml = roxmltree::Document::parse(&page.svg).unwrap();
    xml.descendants()
        .filter(|node| node.has_tag_name("tspan"))
        .filter_map(|node| node.text())
        .collect()
}

fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 0.0001, "{actual} != {expected}");
}

#[test]
fn the_final_paragraph_line_uses_the_full_width_without_changing_glyphs() {
    for flow in [false, true] {
        let page = render(text("A B", 200.0), 200, flow);
        assert!(page.text_diagnostics.is_empty());
        assert_eq!(source(&page), "A B");
        let glyphs = glyphs(&page);
        assert_eq!(
            glyphs.iter().map(|glyph| glyph.id).collect::<Vec<_>>(),
            [38, 5, 39]
        );
        let origin = if flow { 48.0 } else { 10.0 };
        close(glyphs[0].x, origin);
        close(glyphs[1].x, origin + 29.35546875);
        close(glyphs[2].x, origin + 171.98486328125);
    }
}

#[test]
fn leading_and_trailing_spaces_each_receive_one_share() {
    for flow in [false, true] {
        let page = render(text(" A B ", 200.0), 200, flow);
        assert_eq!(source(&page), " A B ");
        assert!(page.text_diagnostics.is_empty());
        let glyphs = glyphs(&page);
        let origin = if flow { 48.0 } else { 10.0 };
        close(glyphs[1].x, origin + 47.54313278198242);
        close(glyphs[3].x, origin + 124.44173431396484);
        close(glyphs[4].x, origin + 152.45687866210938);
    }
}

#[test]
fn tab_receives_four_shares_while_a_space_receives_one() {
    for flow in [false, true] {
        let page = render(text("A\t B", 200.0), 200, flow);
        assert_eq!(source(&page), "A\t B");
        assert_eq!(
            page.text_diagnostics,
            [TextDiagnostic {
                kind: TextDiagnosticKind::UnsupportedTabMeasurement,
                family: "Roboto".into(),
                codepoints: vec![u32::from('\t')],
            }],
        );
        let glyphs = glyphs(&page);
        let origin = if flow { 48.0 } else { 10.0 };
        close(glyphs[1].x, origin + 29.35546875);
        close(glyphs[2].x, origin + 143.458984375);
        close(glyphs[3].x, origin + 171.98486328125);
    }
}

#[test]
fn nbsp_and_other_unicode_spaces_do_not_create_justification_shares() {
    for source_text in ["A\u{a0}B", "A\u{2002}B"] {
        for flow in [false, true] {
            let justified = render(text(source_text, 200.0), 200, flow);
            let mut ordinary = text(source_text, 200.0);
            ordinary.paragraphs.clear();
            let ordinary = render(ordinary, 200, flow);
            assert_eq!(source(&justified), source_text);
            let actual = glyphs(&justified);
            let expected = glyphs(&ordinary);
            assert_eq!(actual.len(), expected.len());
            for (actual, expected) in actual.iter().zip(expected) {
                assert_eq!(actual.id, expected.id);
                close(actual.x, expected.x);
            }
        }
    }
}

#[test]
fn each_wrapped_line_including_the_final_line_is_justified() {
    for flow in [false, true] {
        let page = render(text("A B A B", 100.0), 100, flow);
        assert_eq!(source(&page), "A B A B");
        assert!(page.text_diagnostics.is_empty());
        let letters: Vec<_> = glyphs(&page)
            .into_iter()
            .filter(|glyph| glyph.text == "A" || glyph.text == "B")
            .collect();
        assert_eq!(letters.len(), 4);
        assert!(letters[0].y == letters[1].y && letters[2].y == letters[3].y);
        assert!(letters[2].y > letters[0].y);
        let origin = if flow { 48.0 } else { 10.0 };
        close(letters[3].x, origin + 71.98486328125);
    }
}

#[test]
fn missing_and_reordered_clusters_preserve_source_and_report_their_limits() {
    for source_text in ["A \u{10ffff} B", "A\u{202e}(A V)\u{202c}Z"] {
        for flow in [false, true] {
            let page = render(text(source_text, 300.0), 300, flow);
            assert_eq!(source(&page), source_text);
            assert!(page.text_diagnostics.iter().any(|diagnostic| matches!(
                diagnostic.kind,
                TextDiagnosticKind::MissingGlyphs | TextDiagnosticKind::UnsupportedGlyphPositioning
            )));
            if source_text.ends_with('B') {
                let glyphs = glyphs(&page);
                let last = glyphs.last().unwrap();
                assert_eq!(last.id, 39);
                close(last.x, if flow { 48.0 } else { 10.0 } + 271.98486328125);
            }
        }
    }
}

#[test]
fn covered_greek_justifies_with_retained_glyph_positions() {
    for flow in [false, true] {
        let page = render(text("A Ω B", 300.0), 300, flow);
        assert_eq!(source(&page), "A Ω B");
        assert!(page.text_diagnostics.is_empty());
        let glyphs = glyphs(&page);
        assert_eq!(glyphs[2].id, 569);
        assert_eq!(glyphs.last().unwrap().id, 39);
        close(
            glyphs.last().unwrap().x,
            if flow { 48.0 } else { 10.0 } + 271.98486328125,
        );
    }
}

#[cfg(feature = "pdf")]
#[test]
fn justified_text_remains_selectable_vector_text_in_pdf() {
    for flow in [false, true] {
        let page = render(text("A B", 200.0), 200, flow);
        let fonts = FontBook::default();
        let bytes = sdocx::render_svg_pages_pdf(&[page], &sdocx::PdfOptions::new(fonts.database()))
            .unwrap();
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        assert_eq!(pdf.extract_text(&[1]).unwrap().replace('\n', ""), "A B");
        assert!(
            !pdf.objects
                .values()
                .any(|object| object.as_stream().is_ok_and(|stream| {
                    stream
                        .dict
                        .get(b"Subtype")
                        .is_ok_and(|value| value.as_name().is_ok_and(|name| name == b"Image"))
                }))
        );
    }
}
