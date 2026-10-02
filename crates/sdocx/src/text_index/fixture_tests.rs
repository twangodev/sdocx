use std::collections::BTreeSet;

use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::TextIndex;

#[derive(Deserialize)]
struct Capture {
    apk_sha256: String,
    text_library_sha256: String,
    base_library_sha256: String,
    memory_fills: [u8; 3],
    entry_constructor: String,
    producer_window: [String; 2],
    range_convention: String,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    document_utf16: Vec<u16>,
    source_vector_start_utf16: usize,
    source_vector_utf16: Vec<u16>,
    requested_range_utf16: [u32; 2],
    supplied_shaped_glyphs: Vec<SuppliedGlyph>,
    produced_entries: Vec<Entry>,
}

#[derive(Deserialize)]
struct SuppliedGlyph {
    relative_owner_utf16: u32,
    glyph_id: u32,
}

#[derive(Deserialize)]
struct Entry {
    absolute_utf16: u32,
    kind: u32,
    advance: f32,
    font_size: f32,
    ink_rect: [f32; 4],
    cache_drawable: bool,
    glyphs: Vec<Glyph>,
}

#[derive(Deserialize)]
struct Glyph {
    glyph_id: u32,
}

fn checked_capture() -> Capture {
    let bytes = include_bytes!("../../../../conformance/table-text-owner-bases.json");
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "504bbe262a6bde8f9db3ecd1f5162039f6ed644cc9d85fe7c766e2e941a390ed"
    );
    let capture: Capture = serde_json::from_slice(bytes).unwrap();
    assert_eq!(
        capture.apk_sha256,
        "daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667"
    );
    assert_eq!(
        capture.text_library_sha256,
        "5483711673a499743625eb3275e34b37a006919af346212b46b8d8857834308b"
    );
    assert_eq!(
        capture.base_library_sha256,
        "e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb"
    );
    assert_eq!(capture.memory_fills, [0, 0xa5, 0xff]);
    assert_eq!(capture.entry_constructor, "0x65920");
    assert_eq!(capture.producer_window, ["0x77324", "0x77894"]);
    assert_eq!(capture.range_convention, "half-open UTF-16");
    assert_eq!(capture.cases.len(), 9);
    capture
}

#[test]
fn supplied_relative_utf16_owners_project_to_actual_native_cache_owners() {
    let capture = checked_capture();
    let mut checked_glyphs = 0;
    let mut owned_slots = 0;
    let mut scalar_unit_negative_controls = 0;
    for case in &capture.cases {
        let document = String::from_utf16(&case.document_utf16).unwrap();
        let index = TextIndex::new(&document);
        assert_eq!(
            case.source_vector_utf16,
            case.document_utf16[case.source_vector_start_utf16..],
            "{} source-vector base",
            case.name
        );
        let [start, end] = case.requested_range_utf16;
        let characters = index.utf16_to_char(start).unwrap()..index.utf16_to_char(end).unwrap();
        let requested = index.source(characters.clone()).unwrap();
        let local = TextIndex::new(index.slice(characters).unwrap());
        let mut glyph_ids = BTreeSet::new();
        let mut projected_owners = BTreeSet::new();
        for supplied in &case.supplied_shaped_glyphs {
            assert!(glyph_ids.insert(supplied.glyph_id));
            let local_character = local.utf16_to_char(supplied.relative_owner_utf16).unwrap();
            assert!(local_character < local.len());
            let global_character = requested.characters().start + local_character;
            let source = index
                .source(global_character..global_character + 1)
                .unwrap();
            let relative = source.relative_to(&requested).unwrap();
            assert_eq!(
                relative.utf16().start,
                supplied.relative_owner_utf16,
                "{} relative owner {}",
                case.name,
                supplied.glyph_id
            );
            let native = case
                .produced_entries
                .iter()
                .filter(|entry| {
                    entry
                        .glyphs
                        .iter()
                        .any(|glyph| glyph.glyph_id == supplied.glyph_id)
                })
                .collect::<Vec<_>>();
            assert_eq!(native.len(), 1, "{} native glyph owner", case.name);
            assert_eq!(
                source.utf16().start,
                native[0].absolute_utf16,
                "{} glyph {}",
                case.name,
                supplied.glyph_id
            );
            assert_ne!(source.utf16().start, supplied.relative_owner_utf16);
            assert_ne!(
                native[0].absolute_utf16,
                u32::try_from(case.source_vector_start_utf16).unwrap()
                    + supplied.relative_owner_utf16,
                "{} source-vector base is not the requested owner base",
                case.name
            );
            if u32::try_from(global_character).unwrap() != native[0].absolute_utf16 {
                scalar_unit_negative_controls += 1;
            }
            projected_owners.insert(source.utf16().start);
            checked_glyphs += 1;
        }
        assert_eq!(
            case.produced_entries
                .iter()
                .map(|entry| entry.glyphs.len())
                .sum::<usize>(),
            case.supplied_shaped_glyphs.len(),
            "{} retained glyph count",
            case.name
        );
        owned_slots += projected_owners.len();
    }
    assert_eq!(checked_glyphs, 21);
    assert_eq!(owned_slots, 18);
    assert_eq!(scalar_unit_negative_controls, 19);
}

#[test]
fn captured_continuations_and_source_base_space_keep_bounded_producer_metadata() {
    let capture = checked_capture();
    let mut entries = 0;
    let mut unowned = 0;
    let mut surrogate_continuations = 0;
    let mut spaces = 0;
    for case in &capture.cases {
        let document = String::from_utf16(&case.document_utf16).unwrap();
        let index = TextIndex::new(&document);
        assert_eq!(case.produced_entries.len(), case.document_utf16.len());
        for (offset, entry) in case.produced_entries.iter().enumerate() {
            assert_eq!(usize::try_from(entry.absolute_utf16).unwrap(), offset);
            if entry.glyphs.is_empty() {
                assert_eq!(entry.kind, 3, "{} unowned constructor kind", case.name);
                assert!(
                    !entry.cache_drawable,
                    "{} supplied unowned cache flag",
                    case.name
                );
                assert_eq!(entry.advance.to_bits(), 0);
                assert_eq!(entry.font_size.to_bits(), 0);
                assert_eq!(entry.ink_rect.map(f32::to_bits), [0; 4]);
                unowned += 1;
            } else {
                assert!(
                    entry.cache_drawable,
                    "{} supplied owner cache flag",
                    case.name
                );
                if case.name == "space-source-base" && entry.absolute_utf16 == 4 {
                    assert_eq!(case.source_vector_start_utf16, 3);
                    assert_eq!(case.source_vector_utf16[1], u16::from(b' '));
                    assert_eq!(entry.kind, 1);
                    spaces += 1;
                } else {
                    assert_eq!(entry.kind, 0);
                }
            }
            if index.utf16_to_char(entry.absolute_utf16).is_none() {
                assert!(entry.glyphs.is_empty());
                surrogate_continuations += 1;
            }
            entries += 1;
        }
    }
    assert_eq!(entries, 70);
    assert_eq!(unowned, 52);
    assert_eq!(surrogate_continuations, 9);
    assert_eq!(spaces, 1);
    assert_eq!(entries * capture.memory_fills.len(), 210);
    assert_eq!(capture.cases.len() * capture.memory_fills.len(), 27);
}
