use std::sync::Arc;

use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::*;

#[derive(Deserialize)]
struct Capture {
    apk_sha256: String,
    text_library_sha256: String,
    base_library_sha256: String,
    memory_fills: [u8; 3],
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    offset: [f32; 2],
    range_inclusive: [usize; 2],
    cached_input: CachedInput,
    runs: Vec<ExpectedRun>,
}

#[derive(Deserialize)]
struct CachedInput {
    source_utf16: Vec<u16>,
    logical_map: Vec<usize>,
    gravity: f32,
    paragraph_flag65: bool,
    entries: Vec<Entry>,
}

#[derive(Deserialize)]
struct Entry {
    advance: f32,
    position: [f32; 2],
    layout_rect: [f32; 4],
    ink_rect: [f32; 4],
    kind: u32,
    direction: i32,
    cache_flags: [u8; 2],
    drawable: bool,
    font: Option<Font>,
    span: Span,
    glyphs: Vec<(u32, f32, f32)>,
}

#[derive(Deserialize)]
struct Font {
    wrapper: u64,
    id: i32,
    bitmap: bool,
    language: String,
}

#[derive(Deserialize)]
struct Span {
    font_size: f32,
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
            font_size: self.font_size,
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
struct ExpectedRun {
    range_inclusive: [usize; 2],
    codewords: Vec<u32>,
    positions: Vec<f32>,
    origin: [f32; 2],
    layout_rect: [f32; 4],
    #[serde(alias = "glyph_rect")]
    ink_rect: [f32; 4],
    font_id: i64,
    font_size: f32,
    foreground: u32,
    style: Option<u8>,
    background: Option<u32>,
    span_bit_1: Option<u8>,
    first_codeword_high_bits: Option<u32>,
}

fn assert_float_bits<const N: usize>(actual: [f32; N], expected: [f32; N], context: &str) {
    assert_eq!(
        actual.map(f32::to_bits),
        expected.map(f32::to_bits),
        "{context}"
    );
}

fn check_case(case: &Case) -> (usize, usize) {
    let input = &case.cached_input;
    assert_eq!(input.source_utf16.len(), input.entries.len());
    assert_eq!(input.logical_map.len(), input.entries.len());
    let mut order = input.logical_map.clone();
    order.sort_unstable();
    assert_eq!(order, (0..input.entries.len()).collect::<Vec<_>>());
    let spans: Vec<_> = input
        .entries
        .iter()
        .map(|entry| entry.span.native())
        .collect();
    let mut payloads = Vec::new();
    let caches: Vec<Vec<_>> = input
        .entries
        .iter()
        .enumerate()
        .map(|(owner, entry)| {
            assert_eq!(
                entry.cache_flags,
                [0, 0],
                "{}: cache flag bounds",
                case.name
            );
            entry
                .glyphs
                .iter()
                .map(|&(codeword, x, y)| {
                    let payload = payloads.len();
                    payloads.push((codeword, owner));
                    NativeCachedGlyph {
                        payload,
                        offset: [x, y],
                    }
                })
                .collect()
        })
        .collect();
    let entries: Vec<_> = input
        .entries
        .iter()
        .zip(&spans)
        .zip(&caches)
        .map(|((entry, span), glyphs)| NativeRunEntry {
            kind: entry.kind,
            direction: entry.direction,
            advance: entry.advance,
            position: entry.position,
            layout: NativeRect(entry.layout_rect),
            ink: NativeRect(entry.ink_rect),
            cache: NativeGlyphCache {
                drawable: entry.drawable,
                glyphs,
            },
            span,
            font: entry
                .font
                .as_ref()
                .map_or(NativeFontState::Missing, |font| {
                    assert_ne!(font.wrapper, 0);
                    NativeFontState::Known {
                        source_id: font.id,
                        bitmap: font.bitmap,
                        language: &font.language,
                    }
                }),
            paragraph_override: input.paragraph_flag65,
        })
        .collect();
    let offset = NativeRunOffset {
        x: case.offset[0],
        y: case.offset[1],
        gravity: input.gravity,
    };
    let actual = native_runs(
        &entries,
        case.range_inclusive[0]..=case.range_inclusive[1],
        offset,
    )
    .unwrap_or_else(|error| panic!("{} {:?}: {error}", case.name, case.offset));
    assert_eq!(
        actual.len(),
        case.runs.len(),
        "{} {:?}",
        case.name,
        case.offset
    );
    for (index, (actual, expected)) in actual.iter().zip(&case.runs).enumerate() {
        let context = format!("{} {:?}: run {index}", case.name, case.offset);
        assert_eq!(
            [*actual.source.start(), *actual.source.end()],
            expected.range_inclusive,
            "{context}: source UTF16"
        );
        let codewords: Vec<_> = actual
            .glyphs
            .iter()
            .map(|glyph| payloads[glyph.payload].0)
            .collect();
        assert_eq!(codewords, expected.codewords, "{context}: cached codewords");
        assert_eq!(actual.glyphs.len(), expected.positions.len(), "{context}");
        for (glyph, expected_x) in actual.glyphs.iter().zip(&expected.positions) {
            assert_float_bits([glyph.x], [*expected_x], &context);
            assert_eq!(
                glyph.owner_utf16, payloads[glyph.payload].1,
                "{context}: payload owner"
            );
        }
        assert_float_bits(
            actual.origin,
            expected.origin,
            &format!("{context}: origin"),
        );
        assert_float_bits(
            actual.layout.0,
            expected.layout_rect,
            &format!("{context}: layout bounds"),
        );
        assert_float_bits(
            actual.ink.0,
            expected.ink_rect,
            &format!("{context}: ink bounds"),
        );
        let font_id = i32::try_from(expected.font_id)
            .unwrap_or_else(|_| u32::try_from(expected.font_id).unwrap() as i32);
        assert_eq!(actual.font_id, font_id, "{context}: font identity");
        assert_float_bits([actual.paint.font_size], [expected.font_size], &context);
        assert_eq!(
            actual.paint.foreground, expected.foreground,
            "{context}: foreground"
        );
        if let Some(style) = expected.style {
            assert_eq!(actual.paint.style_bits, style, "{context}: style bits");
        }
        if let Some(background) = expected.background {
            assert_eq!(
                actual.paint.background, background,
                "{context}: ordinary background"
            );
        }
        if let Some(object) = expected.span_bit_1 {
            assert_eq!(
                u8::from(actual.paint.object),
                object,
                "{context}: object flag"
            );
        }
        if let Some(high_bits) = expected.first_codeword_high_bits {
            assert_eq!(
                codewords.first().map_or(0, |word| word >> 8),
                high_bits,
                "{context}: first codeword metadata"
            );
        }
        assert_eq!(
            actual.kind,
            if expected.codewords.is_empty() {
                NativeEmittedKind::DefaultEmpty
            } else {
                NativeEmittedKind::Glyphs
            },
            "{context}: native empty record"
        );
    }
    (
        entries.len(),
        actual.iter().map(|run| run.glyphs.len()).sum(),
    )
}

#[test]
fn complete_cached_emission_matches_every_captured_native_run_field() {
    for (bytes, hash, expected_cases, expected_entries, expected_runs, expected_codewords) in [
        (
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../conformance/table-text-cached-runs.json"
            ))
            .as_slice(),
            "e0f60a1216fc515c800a368e60279fab9b209f3ee883588987fa2246a96665bb",
            230,
            1116,
            528,
            1180,
        ),
        (
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../conformance/table-text-cached-ownership.json"
            ))
            .as_slice(),
            "44b7fd9d1c4de0f803aca18782c281b6971d0ef4c5360afc4969f36dbfef63c0",
            38,
            124,
            40,
            94,
        ),
    ] {
        assert_eq!(format!("{:x}", Sha256::digest(bytes)), hash);
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
        assert_eq!(capture.memory_fills, [0, 165, 255]);
        assert_eq!(capture.cases.len(), expected_cases);
        let counts: Vec<_> = capture.cases.iter().map(check_case).collect();
        assert_eq!(
            counts.iter().map(|(entries, _)| entries).sum::<usize>(),
            expected_entries
        );
        assert_eq!(
            capture
                .cases
                .iter()
                .map(|case| case.runs.len())
                .sum::<usize>(),
            expected_runs
        );
        assert_eq!(
            counts.iter().map(|(_, glyphs)| glyphs).sum::<usize>(),
            expected_codewords
        );
    }
}
