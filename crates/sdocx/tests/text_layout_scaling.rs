#![cfg(feature = "render")]

use std::time::Instant;

use sdocx::fonts::FontBook;
use sdocx::{
    BoundingBox, Document, DocumentMetadata, Page, PageElement, RenderOptions, RichTextBox,
    TextDiagnostic, TextDiagnosticKind, render_document_svg_with_fonts,
};

fn document(source: &str) -> Document {
    let text = RichTextBox {
        text_area_type: None,
        bbox: BoundingBox::default(),
        rotation_degrees: None,
        text: source.into(),
        color: None,
        highlight_color: None,
        underline: false,
        font_size: Some(17.0),
        runs: Vec::new(),
        spans: Vec::new(),
        paragraphs: Vec::new(),
        object_spans: Vec::new(),
        text_sections: Vec::new(),
        margins: None,
        gravity: None,
    };
    Document {
        pages: vec![Page {
            uuid: "text-layout-scaling".into(),
            width: 640,
            height: 100_000,
            content_bbox: BoundingBox::default(),
            background_color: None,
            template: None,
            background: Default::default(),
            objects: vec![PageElement::TextBox(text).into()],
        }],
        metadata: DocumentMetadata {
            default_page_dimensions: Some((360, 640)),
            orientation: Some(0),
            ..Default::default()
        },
    }
}

fn source(kind: &str, length: usize) -> String {
    match kind {
        "latin" => "A".repeat(length),
        "url" => {
            let prefix = "https://example.com/";
            format!("{prefix}{}", "A".repeat(length - prefix.len()))
        }
        "prose" => {
            let sentence = "The quick brown fox jumps over the lazy dog. ";
            let mut text = sentence.repeat(length.div_ceil(sentence.len()));
            text.truncate(length);
            text
        }
        _ => unreachable!(),
    }
}

#[test]
#[ignore = "manual release-mode timing probe for large paragraphs"]
fn large_paragraphs_preserve_source_and_report_layout_scaling() {
    let fonts = FontBook::default();
    let options = RenderOptions::default();
    use TextDiagnosticKind::{UnsupportedMeasurementBudget, UnsupportedMeasurementShaping};

    // Native positioning has a bounded numeric domain; large selections and
    // prose cache-word windows have separate budgets. Time both admitted native
    // measurement and these explicit fallback cases without conflating them.
    let cases = [
        ("latin", 100, None),
        ("latin", 1_000, None),
        ("latin", 3_000, None),
        ("latin", 5_000, Some(UnsupportedMeasurementShaping)),
        ("latin", 20_000, Some(UnsupportedMeasurementBudget)),
        ("latin", 50_000, Some(UnsupportedMeasurementBudget)),
        ("url", 100, None),
        ("url", 1_000, None),
        ("url", 3_000, None),
        ("url", 5_000, Some(UnsupportedMeasurementShaping)),
        ("url", 20_000, Some(UnsupportedMeasurementBudget)),
        ("url", 50_000, Some(UnsupportedMeasurementBudget)),
        ("prose", 100, None),
        ("prose", 1_000, None),
        ("prose", 3_000, Some(UnsupportedMeasurementBudget)),
        ("prose", 5_000, Some(UnsupportedMeasurementBudget)),
        ("prose", 20_000, Some(UnsupportedMeasurementBudget)),
        ("prose", 50_000, Some(UnsupportedMeasurementBudget)),
    ];
    for (kind, length, expected_diagnostic) in cases {
        let source = source(kind, length);
        let document = document(&source);
        let start = Instant::now();
        let pages = render_document_svg_with_fonts(&document, &options, &fonts);
        let elapsed = start.elapsed();
        assert_eq!(pages.len(), 1);
        let path = if expected_diagnostic.is_some() {
            "fallback"
        } else {
            "native"
        };
        let expected_diagnostics = expected_diagnostic
            .map(|kind| {
                let mut codepoints = source.chars().map(u32::from).collect::<Vec<_>>();
                codepoints.sort_unstable();
                codepoints.dedup();
                TextDiagnostic {
                    kind,
                    family: "Roboto".into(),
                    codepoints,
                }
            })
            .into_iter()
            .collect::<Vec<_>>();
        assert_eq!(
            pages[0].text_diagnostics, expected_diagnostics,
            "unexpected measurement path for {kind} chars={length}"
        );
        let xml = roxmltree::Document::parse(&pages[0].svg).unwrap();
        let lines = xml
            .descendants()
            .filter(|node| node.has_tag_name("text"))
            .map(|node| {
                node.descendants()
                    .filter(|child| child.is_text())
                    .filter_map(|child| child.text())
                    .collect::<String>()
            })
            .collect::<Vec<_>>();
        assert!(lines.len() > 1);
        let mut offset = 0;
        for line in &lines {
            assert!(!line.is_empty());
            let end = offset + line.len();
            assert_eq!(source.get(offset..end), Some(line.as_str()));
            offset = end;
        }
        assert_eq!(offset, source.len());
        assert_eq!(lines.concat(), source);
        println!(
            "kind={kind} chars={length} path={path} diagnostics={:?} render_ms={:.3} lines={} svg_bytes={}",
            pages[0].text_diagnostics,
            elapsed.as_secs_f64() * 1_000.0,
            lines.len(),
            pages[0].svg.len(),
        );
    }
}
