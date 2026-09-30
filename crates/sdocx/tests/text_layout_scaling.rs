#![cfg(feature = "render")]

use std::time::Instant;

use sdocx::fonts::FontBook;
use sdocx::{
    BoundingBox, Document, DocumentMetadata, Page, PageElement, RenderOptions, RichTextBox,
    render_document_svg_with_fonts,
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
    for kind in ["latin", "url", "prose"] {
        for length in [5_000, 20_000, 50_000] {
            let source = source(kind, length);
            let document = document(&source);
            let start = Instant::now();
            let pages = render_document_svg_with_fonts(&document, &options, &fonts);
            let elapsed = start.elapsed();
            assert_eq!(pages.len(), 1);
            assert!(pages[0].text_diagnostics.is_empty());
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
                "kind={kind} chars={length} render_ms={:.3} lines={} svg_bytes={}",
                elapsed.as_secs_f64() * 1_000.0,
                lines.len(),
                pages[0].svg.len(),
            );
        }
    }
}
