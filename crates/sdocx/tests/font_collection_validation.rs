#![cfg(feature = "render")]

use std::sync::Arc;

use rustybuzz::ttf_parser::{RawFace, Tag};
use sdocx::fonts::{FontBook, fontdb};
use sdocx::{
    BoundingBox, Document, Page, PageElement, RichTextBox, TextDiagnostic, TextDiagnosticKind,
};

const FONT: &[u8] = include_bytes!("../assets/fonts/Roboto-Regular.ttf");

fn collection(extra_tags: &[u32]) -> Vec<u8> {
    let face = RawFace::parse(FONT, 0).unwrap();
    let mut tables: Vec<_> = face
        .table_records
        .into_iter()
        .map(|record| (record.tag.0, face.table(record.tag).unwrap()))
        .chain(extra_tags.iter().map(|tag| (*tag, &[][..])))
        .collect();
    tables.sort_by_key(|(tag, _)| *tag);
    let directory_end = 28 + tables.len() * 16;
    let mut bytes = vec![0; directory_end];
    bytes[..4].copy_from_slice(b"ttcf");
    bytes[4..8].copy_from_slice(&0x0001_0000_u32.to_be_bytes());
    bytes[8..12].copy_from_slice(&1_u32.to_be_bytes());
    bytes[12..16].copy_from_slice(&16_u32.to_be_bytes());
    bytes[16..20].copy_from_slice(&FONT[..4]);
    bytes[20..22].copy_from_slice(&(tables.len() as u16).to_be_bytes());
    for (index, (tag, table)) in tables.iter().enumerate() {
        let record = 28 + index * 16;
        bytes[record..record + 4].copy_from_slice(&tag.to_be_bytes());
        let offset = bytes.len() as u32;
        bytes[record + 8..record + 12].copy_from_slice(&offset.to_be_bytes());
        bytes[record + 12..record + 16].copy_from_slice(&(table.len() as u32).to_be_bytes());
        bytes.extend_from_slice(table);
        bytes.resize(bytes.len().next_multiple_of(4), 0);
    }
    bytes
}

fn record_offset(bytes: &[u8], tag: &[u8; 4]) -> usize {
    let count = u16::from_be_bytes(bytes[20..22].try_into().unwrap());
    (0..usize::from(count))
        .map(|index| 28 + index * 16)
        .find(|offset| &bytes[*offset..*offset + 4] == tag)
        .unwrap()
}

fn render(bytes: Vec<u8>) -> sdocx::RenderedPage {
    let mut database = fontdb::Database::new();
    database.load_font_data(bytes);
    database.set_sans_serif_family("Roboto");
    let fonts = FontBook::new(Arc::new(database));
    assert_eq!(fonts.resolve("Roboto", false, false).unwrap().index, 0);
    let bounds = BoundingBox {
        x_min: 10.0,
        y_min: 10.0,
        x_max: 290.0,
        y_max: 190.0,
    };
    let text = RichTextBox {
        text_area_type: None,
        bbox: bounds,
        rotation_degrees: None,
        text: "Font validation".into(),
        color: None,
        highlight_color: None,
        underline: false,
        font_size: Some(20.0),
        runs: Vec::new(),
        spans: Vec::new(),
        paragraphs: Vec::new(),
        object_spans: Vec::new(),
        text_sections: Vec::new(),
        margins: None,
        gravity: None,
    };
    let document = Document {
        metadata: Default::default(),
        pages: vec![Page {
            uuid: "font-validation".into(),
            width: 300,
            height: 200,
            content_bbox: bounds,
            background_color: None,
            template: None,
            background: Default::default(),
            objects: vec![PageElement::TextBox(text).into()],
        }],
    };
    sdocx::render_document_svg_with_fonts(&document, &Default::default(), &fonts)
        .into_iter()
        .next()
        .unwrap()
}

fn assert_rejected(bytes: Vec<u8>) {
    let page = render(bytes);
    assert!(page.text_diagnostics.iter().any(|issue| {
        issue.kind == TextDiagnosticKind::UnusableFontData && issue.family == "Roboto"
    }));
    assert!(!page.svg.contains("@font-face"));
}

#[test]
fn valid_collection_is_embedded_with_native_measurement_configuration_diagnostic() {
    let page = render(collection(&[]));
    assert_eq!(
        page.text_diagnostics,
        vec![TextDiagnostic {
            kind: TextDiagnosticKind::UnsupportedMeasurementFont,
            family: "Roboto".into(),
            codepoints: " Fadilnotv".chars().map(u32::from).collect(),
        }]
    );
    assert!(page.svg.contains("@font-face"));
}

#[test]
fn malformed_optional_table_is_diagnosed_instead_of_dropped() {
    let mut bytes = collection(&[]);
    let offset = record_offset(&bytes, b"GSUB");
    bytes[offset + 8..offset + 12].copy_from_slice(&u32::MAX.to_be_bytes());
    assert_rejected(bytes);
}

#[test]
fn duplicate_table_tags_are_diagnosed() {
    let duplicate = Tag::from_bytes(b"xtra").0;
    assert_rejected(collection(&[duplicate, duplicate]));
}

#[test]
fn oversized_table_directory_is_diagnosed_without_panicking() {
    let count = usize::from(RawFace::parse(FONT, 0).unwrap().table_records.len());
    let tags: Vec<_> = (0..4096 - count)
        .map(|index| 0x7a00_0000 + index as u32)
        .collect();
    assert_rejected(collection(&tags));
}

#[test]
fn overlapping_nonempty_tables_are_diagnosed() {
    let mut bytes = collection(&[]);
    let source = record_offset(&bytes, b"GSUB");
    let target = record_offset(&bytes, b"GPOS");
    let offset_and_length: [u8; 8] = bytes[source + 8..source + 16].try_into().unwrap();
    bytes[target + 8..target + 16].copy_from_slice(&offset_and_length);
    assert_rejected(bytes);
}
