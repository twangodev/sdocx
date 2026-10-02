#![cfg(feature = "render")]

#[path = "support/svg_fonts.rs"]
mod svg_fonts;

use std::sync::Arc;

use resvg::usvg;
use sdocx::fonts::{FontBook, fontdb};
use sdocx::{
    BoundingBox, Document, DocumentMetadata, Page, PageElement, RenderedPage, RichTextBox,
    RichTextSpan, RichTextSpanType, TextDiagnosticKind,
};

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
        color: None,
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

fn render(mut content: RichTextBox, flow: bool, fonts: &FontBook) -> RenderedPage {
    if flow {
        content.bbox = BoundingBox::default();
    }
    let document = Document {
        metadata: DocumentMetadata {
            default_page_dimensions: Some((360, 800)),
            ..Default::default()
        },
        pages: vec![Page {
            uuid: "fallback".into(),
            width: 596,
            height: 800,
            content_bbox: content.bbox,
            background_color: None,
            template: None,
            background: Default::default(),
            objects: vec![PageElement::TextBox(content).into()],
        }],
    };
    sdocx::render_document_svg_with_fonts(&document, &Default::default(), fonts)
        .pop()
        .unwrap()
}

#[derive(Debug, PartialEq)]
struct Glyph {
    text: String,
    id: u16,
    font: fontdb::ID,
    x: f32,
}

fn collect_glyphs(group: &usvg::Group, output: &mut Vec<Glyph>) {
    for node in group.children() {
        match node {
            usvg::Node::Group(group) => collect_glyphs(group, output),
            usvg::Node::Text(text) => {
                for span in text.layouted() {
                    for glyph in &span.positioned_glyphs {
                        output.push(Glyph {
                            text: glyph.text.clone(),
                            id: glyph.id.0,
                            font: glyph.font,
                            x: glyph.transform().tx,
                        });
                    }
                }
            }
            _ => {}
        }
    }
}

fn glyphs(page: &RenderedPage, fonts: &FontBook) -> Vec<Glyph> {
    let database = fonts.database();
    let options = usvg::Options {
        font_resolver: svg_fonts::font_resolver(&database),
        fontdb: database,
        ..Default::default()
    };
    let tree = usvg::Tree::from_str(&page.svg, &options).unwrap();
    let mut output = Vec::new();
    collect_glyphs(tree.root(), &mut output);
    output
}

fn source(page: &RenderedPage) -> String {
    let document = roxmltree::Document::parse(&page.svg).unwrap();
    document
        .descendants()
        .filter(|node| node.has_tag_name("tspan"))
        .map(|node| node.text().unwrap_or(""))
        .collect()
}

fn diagnostics(page: &RenderedPage, family: &str, expected: &[(TextDiagnosticKind, &[u32])]) {
    assert_eq!(
        page.text_diagnostics.len(),
        expected.len(),
        "{:?}",
        page.text_diagnostics
    );
    for (actual, (kind, codepoints)) in page.text_diagnostics.iter().zip(expected) {
        assert_eq!(&actual.kind, kind);
        assert_eq!(actual.family, family);
        assert_eq!(actual.codepoints, *codepoints);
    }
}

#[test]
fn a_missing_cluster_does_not_change_supported_latin_glyphs_or_positions() {
    let fonts = FontBook::default();
    for flow in [false, true] {
        let control = render(text("office"), flow, &fonts);
        let mixed = render(text("office \u{10ffff}"), flow, &fonts);
        let expected = glyphs(&control, &fonts);
        let actual = glyphs(&mixed, &fonts);
        assert_eq!(
            expected.iter().map(|glyph| glyph.id).collect::<Vec<_>>(),
            [84, 75, 75, 78, 72, 74]
        );
        assert_eq!(&actual[..6], expected.as_slice());
        assert_eq!(source(&mixed), "office \u{10ffff}");
        diagnostics(
            &mixed,
            "Roboto",
            &[
                (TextDiagnosticKind::UnsupportedMeasurementFont, &[0x10ffff]),
                (TextDiagnosticKind::MissingGlyphs, &[0x10ffff]),
            ],
        );
    }
}

#[test]
fn tab_preserves_source_and_reports_compatibility_measurement() {
    let fonts = FontBook::default();
    for flow in [false, true] {
        let tab = render(text("A\tB"), flow, &fonts);
        let spaces = render(text("A    B"), flow, &fonts);
        let actual = glyphs(&tab, &fonts);
        let expected = glyphs(&spaces, &fonts);
        assert_eq!(source(&tab), "A\tB");
        diagnostics(
            &tab,
            "Roboto",
            &[(TextDiagnosticKind::UnsupportedTabMeasurement, &[9])],
        );
        assert!(spaces.text_diagnostics.is_empty());
        let actual_last = actual.last().unwrap();
        let expected_last = expected.last().unwrap();
        assert_eq!(actual_last.id, expected_last.id);
        assert_eq!(actual_last.font, expected_last.font);
        assert_eq!(actual_last.text, expected_last.text);
        let origin = if flow { 48.0 } else { 10.0 };
        assert!((f64::from(actual_last.x) - origin - 73.916015625).abs() < 0.00001);
        let native_space = 285187.0_f32 / 256.0 / 100.0;
        let native_a_advance = 751500.0_f32 / 256.0 / 100.0;
        let native_b_position = (0..4).fold(native_a_advance, |x, _| x + native_space);
        assert!(
            (f64::from(expected_last.x) - origin - f64::from(native_b_position)).abs() < 0.00001
        );
    }
}

#[test]
fn a_covered_cluster_uses_its_retained_face_without_changing_its_neighbors() {
    let fonts = FontBook::default();
    let regular = fonts.resolve("Roboto", false, false).unwrap();
    let mono = fonts.resolve("Roboto Mono", false, false).unwrap();
    for flow in [false, true] {
        let page = render(text("A∕B"), flow, &fonts);
        let actual = glyphs(&page, &fonts);
        diagnostics(
            &page,
            "Roboto",
            &[(TextDiagnosticKind::UnsupportedMeasurementFont, &[0x2215])],
        );
        assert_eq!(source(&page), "A∕B");
        assert_eq!(
            actual.iter().map(|glyph| glyph.id).collect::<Vec<_>>(),
            [38, 137, 39]
        );
        assert_eq!(
            actual.iter().map(|glyph| glyph.font).collect::<Vec<_>>(),
            [regular.id, mono.id, regular.id]
        );
        let origin = if flow { 48.0 } else { 10.0 };
        assert!((f64::from(actual[1].x) - origin - 29.35546875).abs() < 0.00001);
        assert!((f64::from(actual[2].x) - origin - 56.35986328125).abs() < 0.00001);
    }
}

fn family_span(family: &str) -> RichTextSpan {
    RichTextSpan {
        kind: RichTextSpanType::FontName,
        start_utf16: 0,
        end_utf16: 3,
        interval_type: sdocx::SpanIntervalType::from(0),
        payload: [
            vec![0; 8],
            (family.len() as u16 + 1).to_le_bytes().to_vec(),
            family.as_bytes().to_vec(),
            vec![0],
        ]
        .concat(),
    }
}

#[test]
fn coverage_fallback_to_another_style_of_the_same_family_keeps_that_face() {
    let defaults = FontBook::default();
    let family = "Coverage Probe";
    let mut database = fontdb::Database::new();
    for face in [
        defaults.resolve("Roboto", false, false).unwrap(),
        defaults.resolve("Roboto Mono", true, false).unwrap(),
    ] {
        let original = defaults.database();
        let mut info = original.face(face.id).unwrap().clone();
        info.id = fontdb::ID::dummy();
        info.families.truncate(1);
        info.families[0].0 = family.into();
        database.push_face_info(info);
    }
    database.set_sans_serif_family(family);
    let fonts = FontBook::new(Arc::new(database));
    let regular = fonts.resolve(family, false, false).unwrap();
    let bold = fonts.resolve(family, true, false).unwrap();
    for flow in [false, true] {
        let mut content = text("A∕B");
        content.spans.push(family_span(family));
        let page = render(content, flow, &fonts);
        let actual = glyphs(&page, &fonts);
        diagnostics(
            &page,
            family,
            &[(
                TextDiagnosticKind::UnsupportedMeasurementFont,
                &[65, 66, 0x2215],
            )],
        );
        assert_eq!(
            actual.iter().map(|glyph| glyph.font).collect::<Vec<_>>(),
            [regular.id, bold.id, regular.id]
        );
        assert_eq!(
            actual.iter().map(|glyph| glyph.id).collect::<Vec<_>>(),
            [38, 137, 39]
        );
    }
}

#[cfg(feature = "pdf")]
#[test]
fn coverage_fallback_preserves_selectable_vector_text_in_pdf() {
    let fonts = FontBook::default();
    for flow in [false, true] {
        let source = "office ∕ AV";
        let page = render(text(source), flow, &fonts);
        let bytes = sdocx::render_svg_pages_pdf(&[page], &sdocx::PdfOptions::new(fonts.database()))
            .unwrap();
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        assert_eq!(pdf.extract_text(&[1]).unwrap().replace('\n', ""), source);
        assert!(!pdf.objects.values().any(|object| {
            object.as_stream().is_ok_and(|stream| {
                stream
                    .dict
                    .get(b"Subtype")
                    .is_ok_and(|value| value.as_name().is_ok_and(|name| name == b"Image"))
            })
        }));
    }
}
