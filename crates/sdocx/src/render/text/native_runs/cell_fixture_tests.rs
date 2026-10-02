use std::sync::Arc;

use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::*;

#[derive(Deserialize)]
struct Capture {
    memory_fills: [u8; 3],
    repeat_zero_fill: bool,
    text_library_sha256: String,
    base_library_sha256: String,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    source_utf16: Vec<u16>,
    placed_entries: Vec<Entry>,
    emission_input: EmissionInput,
    emitted_runs: Emission,
}

#[derive(Deserialize)]
struct EmissionInput {
    offset_bits: [u32; 2],
    gravity_bits: u32,
    range_inclusive: [usize; 2],
    layout_paragraphs: Vec<LayoutParagraph>,
}

#[derive(Deserialize)]
struct LayoutParagraph {
    source_start_utf16: usize,
    source_length_utf16: usize,
    inverse_logical_map_utf16: Vec<usize>,
}

#[derive(Deserialize)]
struct Entry {
    utf16_slot: usize,
    advance_bits: u32,
    position_bits: [u32; 2],
    layout_rect_bits: [u32; 4],
    ink_rect_bits: [u32; 4],
    kind: u32,
    raw_direction: i32,
    drawable: bool,
    glyph_records: Vec<[u32; 3]>,
    font: Option<Font>,
    span: Span,
    paragraph_override: bool,
}

#[derive(Deserialize)]
struct Font {
    source_id: i32,
    bitmap: bool,
    language: String,
}

#[derive(Deserialize)]
struct Span {
    font_size_bits: u32,
    foreground: u32,
    background: u32,
    composing_background: u32,
    style_bits: u8,
    family: Option<String>,
    underline: u32,
    correction_foreground: u32,
    flags: u8,
    correction_foreground_enabled: bool,
}

impl Span {
    fn native(&self) -> NativeDrawSpan {
        NativeDrawSpan {
            font_size: f32::from_bits(self.font_size_bits),
            foreground: self.foreground,
            background: self.background,
            composing_background: self.composing_background,
            style_bits: self.style_bits,
            family: self.family.as_deref().map(Arc::from),
            underline: self.underline,
            correction_foreground: self.correction_foreground,
            flags: self.flags,
            correction_foreground_enabled: self.correction_foreground_enabled,
        }
    }
}

#[derive(Deserialize)]
struct Emission {
    accepted: bool,
    runs: Vec<ExpectedRun>,
}

#[derive(Deserialize)]
struct ExpectedRun {
    range_inclusive: [usize; 2],
    codewords: Vec<u32>,
    position_bits: Vec<u32>,
    origin_bits: [u32; 2],
    ink_rect_bits: [u32; 4],
    layout_rect_bits: [u32; 4],
    is_object: bool,
    source_id: u32,
    font_size_bits: u32,
    foreground: u32,
    style_byte: u8,
    background: u32,
}

fn check_run(
    actual: &NativeEmittedRun<i32>,
    expected: &ExpectedRun,
    payloads: &[(u32, usize)],
    name: &str,
) {
    assert_eq!(
        [*actual.source.start(), *actual.source.end()],
        expected.range_inclusive,
        "{name}: source range"
    );
    assert_eq!(
        actual
            .glyphs
            .iter()
            .map(|glyph| payloads[glyph.payload].0)
            .collect::<Vec<_>>(),
        expected.codewords,
        "{name}: codewords"
    );
    for glyph in &actual.glyphs {
        assert_eq!(
            glyph.owner_utf16, payloads[glyph.payload].1,
            "{name}: input owner"
        );
    }
    assert_eq!(
        actual
            .glyphs
            .iter()
            .map(|glyph| glyph.x.to_bits())
            .collect::<Vec<_>>(),
        expected.position_bits,
        "{name}: glyph positions"
    );
    assert_eq!(
        actual.origin.map(f32::to_bits),
        expected.origin_bits,
        "{name}: origin"
    );
    assert_eq!(
        actual.ink.0.map(f32::to_bits),
        expected.ink_rect_bits,
        "{name}: ink rectangle"
    );
    assert_eq!(
        actual.layout.0.map(f32::to_bits),
        expected.layout_rect_bits,
        "{name}: layout rectangle"
    );
    assert_eq!(
        actual
            .font_source
            .map_or(u32::MAX, |source| u32::from_le_bytes(source.to_le_bytes())),
        expected.source_id,
        "{name}: font source"
    );
    assert_eq!(
        actual.paint.font_size.to_bits(),
        expected.font_size_bits,
        "{name}: font size"
    );
    assert_eq!(
        actual.paint.foreground, expected.foreground,
        "{name}: foreground"
    );
    assert_eq!(
        actual.paint.style_bits, expected.style_byte,
        "{name}: style"
    );
    assert_eq!(
        actual.paint.background, expected.background,
        "{name}: background"
    );
    assert_eq!(
        actual.paint.object, expected.is_object,
        "{name}: object flag"
    );
}

fn check_case(case: &Case) {
    assert_eq!(
        case.placed_entries.len(),
        case.source_utf16.len(),
        "{}: dense input",
        case.name
    );
    for paragraph in &case.emission_input.layout_paragraphs {
        assert_eq!(
            paragraph.inverse_logical_map_utf16.len(),
            paragraph.source_length_utf16,
            "{}: paragraph map length",
            case.name
        );
        assert!(
            paragraph.source_start_utf16 + paragraph.source_length_utf16 <= case.source_utf16.len(),
            "{}: paragraph source bounds",
            case.name
        );
    }
    let spans: Vec<_> = case
        .placed_entries
        .iter()
        .map(|entry| entry.span.native())
        .collect();
    let mut payloads = Vec::new();
    let glyphs: Vec<Vec<_>> = case
        .placed_entries
        .iter()
        .enumerate()
        .map(|(slot, entry)| {
            assert_eq!(entry.utf16_slot, slot, "{}: native slot", case.name);
            entry
                .glyph_records
                .iter()
                .map(|record| {
                    let payload = payloads.len();
                    payloads.push((record[0], slot));
                    NativeCachedGlyph {
                        payload,
                        offset: [f32::from_bits(record[1]), f32::from_bits(record[2])],
                    }
                })
                .collect()
        })
        .collect();
    let entries: Vec<_> = case
        .placed_entries
        .iter()
        .zip(&spans)
        .zip(&glyphs)
        .map(|((entry, span), glyphs)| NativeRunEntry {
            kind: entry.kind,
            direction: entry.raw_direction,
            advance: f32::from_bits(entry.advance_bits),
            position: entry.position_bits.map(f32::from_bits),
            layout: NativeRect(entry.layout_rect_bits.map(f32::from_bits)),
            ink: NativeRect(entry.ink_rect_bits.map(f32::from_bits)),
            cache: NativeGlyphCache {
                drawable: entry.drawable,
                glyphs,
            },
            span,
            font: entry
                .font
                .as_ref()
                .map_or(NativeFontState::Missing, |font| NativeFontState::Known {
                    source: font.source_id,
                    bitmap: font.bitmap,
                    language: &font.language,
                }),
            paragraph_override: entry.paragraph_override,
        })
        .collect();
    let [x, y] = case.emission_input.offset_bits.map(f32::from_bits);
    let [start, end] = case.emission_input.range_inclusive;
    let result = native_runs(
        &entries,
        start..=end,
        NativeRunOffset {
            x,
            y,
            gravity: f32::from_bits(case.emission_input.gravity_bits),
        },
    );
    if entries.is_empty() {
        assert!(
            !case.emitted_runs.accepted,
            "{}: native empty admission",
            case.name
        );
        assert!(
            case.emitted_runs.runs.is_empty(),
            "{}: native empty output",
            case.name
        );
        assert_eq!(
            result,
            Err(NativeRunError::InvalidRange),
            "{}: bounded Rust empty admission",
            case.name
        );
        return;
    }
    assert!(
        case.emitted_runs.accepted,
        "{}: native admission",
        case.name
    );
    let actual = result.unwrap_or_else(|error| panic!("{}: {error}", case.name));
    assert_eq!(
        actual.len(),
        case.emitted_runs.runs.len(),
        "{}: run count",
        case.name
    );
    for (actual, expected) in actual.iter().zip(&case.emitted_runs.runs) {
        check_run(actual, expected, &payloads, &case.name);
    }
}

#[test]
fn actual_native_cell_placed_entries_match_cached_run_emission() {
    let bytes = include_bytes!("../../../../../../conformance/table-text-cell-emission.json");
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "0098cfcd93274b210892fd0653677fd4cef3a35ebc4f0419b4b661857cbfa4ce"
    );
    let capture: Capture = serde_json::from_slice(bytes).unwrap();
    assert_eq!(capture.memory_fills, [0, 165, 255]);
    assert!(capture.repeat_zero_fill);
    assert_eq!(
        capture.text_library_sha256,
        "5483711673a499743625eb3275e34b37a006919af346212b46b8d8857834308b"
    );
    assert_eq!(
        capture.base_library_sha256,
        "e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb"
    );
    assert_eq!(capture.cases.len(), 17);
    assert_eq!(
        capture
            .cases
            .iter()
            .map(|case| case.placed_entries.len())
            .sum::<usize>(),
        78
    );
    assert_eq!(
        capture
            .cases
            .iter()
            .flat_map(|case| &case.placed_entries)
            .map(|entry| entry.glyph_records.len())
            .sum::<usize>(),
        67
    );
    assert_eq!(
        capture
            .cases
            .iter()
            .map(|case| case.emitted_runs.runs.len())
            .sum::<usize>(),
        27
    );
    assert_eq!(
        capture
            .cases
            .iter()
            .flat_map(|case| &case.emitted_runs.runs)
            .map(|run| run.codewords.len())
            .sum::<usize>(),
        67
    );
    for case in &capture.cases {
        check_case(case);
    }
}
