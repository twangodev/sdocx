#![cfg(feature = "render")]

use std::sync::Arc;

use base64::Engine;
use sdocx::{
    BoundingBox, Document, Page, PageElement, RichTextBox, RichTextSpan, RichTextSpanType,
    SpanIntervalType, TextDiagnostic, TextDiagnosticKind,
    fonts::{FontBook, UnicodeBuffer, fontdb},
};
use sha2::{Digest, Sha256};

#[path = "helpers/font_collection.rs"]
mod fixtures;

const ROBOTO: &[u8] = include_bytes!("../assets/fonts/Roboto-Regular.ttf");
const DEJAVU: &[u8] = include_bytes!("assets/fonts/DejaVuSans.ttf");
const COLLECTION: &[u8] = include_bytes!("assets/fonts/Roboto-DejaVuSans.ttc");

fn document(family: &str) -> Document {
    let mut payload = vec![0; 8];
    payload.extend_from_slice(&u16::try_from(family.len() + 1).unwrap().to_le_bytes());
    payload.extend_from_slice(family.as_bytes());
    payload.push(0);
    let content = RichTextBox {
        text_area_type: None,
        bbox: BoundingBox {
            x_min: 10.0,
            y_min: 20.0,
            x_max: 310.0,
            y_max: 120.0,
        },
        rotation_degrees: None,
        text: "iiiWWW".into(),
        color: None,
        highlight_color: None,
        underline: false,
        font_size: Some(45.0),
        runs: Vec::new(),
        paragraphs: Vec::new(),
        object_spans: Vec::new(),
        text_sections: Vec::new(),
        margins: None,
        gravity: None,
        spans: vec![RichTextSpan {
            kind: RichTextSpanType::FontName,
            start_utf16: 0,
            end_utf16: 6,
            interval_type: SpanIntervalType::ClosedOpen,
            payload,
        }],
    };
    Document {
        metadata: Default::default(),
        pages: vec![Page {
            uuid: "collection".into(),
            width: 360,
            height: 160,
            content_bbox: Default::default(),
            background_color: None,
            template: None,
            background: Default::default(),
            objects: vec![PageElement::TextBox(content).into()],
        }],
    }
}

#[test]
fn shared_browser_collection_has_pinned_sources_and_reproducible_bytes() {
    for (bytes, expected) in [
        (
            ROBOTO,
            "56a45233d29f11b4dfb86d248e921939d115778f87325e7ae8cc108383d6664d",
        ),
        (
            DEJAVU,
            "57f73e11f51999432bf7ab22ce55b6f945d5eca1bf824404cfa9ec2e3718c84e",
        ),
        (
            COLLECTION,
            "05b90eb2586360bca28ff1852a12e0bda9933f344b0ae75b3465f22b73ed63ba",
        ),
    ] {
        assert_eq!(format!("{:x}", Sha256::digest(bytes)), expected);
    }
    assert_eq!(fixtures::collection(&[ROBOTO, DEJAVU]), COLLECTION);
}

#[test]
fn svg_and_replay_embed_the_selected_collection_face_with_identical_glyphs() {
    let mut database = fontdb::Database::new();
    database.load_font_data(COLLECTION.to_vec());
    database.set_sans_serif_family("Roboto");
    let fonts = FontBook::new(Arc::new(database));
    for (family, index) in [("Roboto", 0), ("DejaVu Sans", 1)] {
        let selected = fonts.resolve(family, false, false).unwrap();
        assert_eq!(selected.index, index);
        let source = document(family);
        let layout = sdocx::layout_document(&source);
        for page in [
            sdocx::render_layout_page_svg_with_fonts(
                &source,
                &layout,
                0,
                &Default::default(),
                &fonts,
            )
            .unwrap(),
            sdocx::render_layout_page_replay_svg_with_fonts(
                &source,
                &layout,
                0,
                &Default::default(),
                &fonts,
            )
            .unwrap(),
        ] {
            assert_eq!(
                page.text_diagnostics,
                vec![TextDiagnostic {
                    kind: TextDiagnosticKind::UnsupportedMeasurementFont,
                    family: family.into(),
                    codepoints: vec![u32::from('W'), u32::from('i')],
                }]
            );
            let xml = roxmltree::Document::parse(&page.svg).unwrap();
            let css = xml
                .descendants()
                .find(|node| node.has_tag_name("style"))
                .unwrap()
                .text()
                .unwrap();
            let encoded = css
                .split_once("base64,")
                .unwrap()
                .1
                .split('"')
                .next()
                .unwrap();
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .unwrap();
            assert!(!bytes.starts_with(b"ttcf"));
            let mut embedded_database = fontdb::Database::new();
            embedded_database.load_font_data(bytes);
            let embedded_fonts = FontBook::new(Arc::new(embedded_database));
            let embedded = embedded_fonts.resolve(family, false, false).unwrap();
            assert_eq!(embedded.index, 0);
            assert_eq!(embedded.metrics, selected.metrics);
            let buffer = || {
                let mut buffer = UnicodeBuffer::new();
                buffer.push_str("iiiWWW");
                buffer.guess_segment_properties();
                buffer
            };
            assert_eq!(
                selected.shape(buffer(), &[]).unwrap().glyphs,
                embedded.shape(buffer(), &[]).unwrap().glyphs
            );
            let span = xml
                .descendants()
                .find(|node| node.has_tag_name("tspan"))
                .unwrap();
            assert_eq!(span.text(), Some("iiiWWW"));
            let positions = svgtypes::NumberListParser::from(span.attribute("x").unwrap())
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            let advance = if index == 1 { 569.0 } else { 497.0 };
            assert_eq!(positions[0], 10.0);
            assert!(
                (positions[1] - 10.0 - advance * 45.0 / 2048.0).abs() < 0.001,
                "{family}: {positions:?}"
            );
        }
    }
}
