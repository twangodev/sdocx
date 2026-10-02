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
    range_inclusive: Option<[usize; 2]>,
    cached_input: Option<CachedInput>,
    kernel_case: Option<bool>,
    terminal: Option<String>,
    native_terminal_pc: Option<u64>,
    unicorn_error: Option<u32>,
    output_vector: Option<[u64; 3]>,
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
    positions: Option<Vec<f32>>,
    position_bits: Option<Vec<u32>>,
    origin: [f32; 2],
    layout_rect: [f32; 4],
    #[serde(alias = "glyph_rect")]
    ink_rect: [f32; 4],
    font_id: i64,
    font_size: f32,
    foreground: u32,
    style: Option<u8>,
    #[serde(alias = "ordinary_background")]
    background: Option<u32>,
    #[serde(alias = "span_bit1")]
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

fn checked_capture(bytes: &[u8], hash: &str) -> Capture {
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
    capture
}

struct CachedEmission {
    entry_count: usize,
    payloads: Vec<(u32, usize)>,
    result: Result<Vec<NativeEmittedRun>, NativeRunError>,
}

fn emit_case(case: &Case) -> CachedEmission {
    let input = case
        .cached_input
        .as_ref()
        .expect("native cached input snapshot");
    let range = case.range_inclusive.expect("native requested source range");
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
                        source: font.id,
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
    CachedEmission {
        entry_count: entries.len(),
        payloads,
        result: native_runs(&entries, range[0]..=range[1], offset),
    }
}

fn check_case(case: &Case) -> (usize, usize) {
    let CachedEmission {
        entry_count,
        payloads,
        result,
    } = emit_case(case);
    let actual = result.unwrap_or_else(|error| panic!("{} {:?}: {error}", case.name, case.offset));
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
        let position_bits: Vec<_> = match (&expected.positions, &expected.position_bits) {
            (Some(positions), None) => positions.iter().map(|value| value.to_bits()).collect(),
            (None, Some(bits)) => bits.clone(),
            _ => panic!("{context}: exactly one captured glyph position representation required"),
        };
        assert_eq!(actual.glyphs.len(), position_bits.len(), "{context}");
        for (glyph, expected_x) in actual.glyphs.iter().zip(position_bits) {
            assert_eq!(glyph.x.to_bits(), expected_x, "{context}: glyph X");
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
        assert_eq!(
            actual.font_source.unwrap_or(-1),
            font_id,
            "{context}: font identity"
        );
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
    (entry_count, actual.iter().map(|run| run.glyphs.len()).sum())
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
        let capture = checked_capture(bytes, hash);
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

fn cached_object_capture() -> Capture {
    checked_capture(
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../conformance/table-text-cached-object-runs.json"
        )),
        "de3b95b24e236c14ef8af01af89c1e58478599a260aa8c09f7f2e3db8b084b45",
    )
}

#[test]
fn cached_object_emission_matches_every_successful_native_record() {
    let capture = cached_object_capture();
    assert_eq!(capture.cases.len(), 82);
    assert!(capture.cases.iter().all(|case| case.kernel_case.is_some()));
    let cases: Vec<_> = capture
        .cases
        .iter()
        .filter(|case| case.kernel_case == Some(true))
        .collect();
    assert_eq!(cases.len(), 56);
    let mut entry_count = 0;
    let mut glyph_count = 0;
    let mut undrawable_count = 0;
    let mut null_font_count = 0;
    for case in cases {
        assert_eq!(
            case.terminal.as_deref(),
            Some("producer-window-retained-record-published")
        );
        assert_eq!(case.unicorn_error, Some(0));
        assert_eq!(case.native_terminal_pc, Some(0));
        let [begin, end, capacity] = case.output_vector.unwrap();
        assert_ne!(begin, 0);
        assert_eq!(end - begin, 8);
        assert_eq!(capacity, end);
        assert_eq!(case.runs.len(), 1);
        let input = case.cached_input.as_ref().unwrap();
        assert_eq!(input.source_utf16, [0xfffc]);
        assert_eq!(input.entries.len(), 1);
        let entry = &input.entries[0];
        assert_eq!(entry.kind, 5);
        assert_eq!(entry.direction, 0);
        assert_eq!(entry.span.flags, 2);
        assert!(!entry.glyphs.is_empty());
        undrawable_count += usize::from(!entry.drawable);
        null_font_count += usize::from(entry.font.is_none());
        if let Some(font) = &entry.font {
            assert_eq!(font.id, 7);
        }
        let expected = &case.runs[0];
        assert!(expected.positions.is_none());
        assert!(expected.position_bits.is_some());
        assert!(expected.style.is_some());
        assert!(expected.background.is_some());
        assert_eq!(expected.span_bit_1, Some(1));
        let counts = check_case(case);
        entry_count += counts.0;
        glyph_count += counts.1;
    }
    assert_eq!(entry_count, 56);
    assert_eq!(glyph_count, 80);
    assert_eq!(undrawable_count, 1);
    assert_eq!(null_font_count, 1);
}

#[test]
fn glyphless_object_controls_are_separate_from_published_native_records() {
    let capture = cached_object_capture();
    let controls: Vec<_> = capture
        .cases
        .iter()
        .filter(|case| case.kernel_case == Some(false))
        .collect();
    assert_eq!(controls.len(), 26);
    let mut rejected_objects = 0;
    let mut empty_defaults = 0;
    for case in controls {
        assert_eq!(
            case.terminal.as_deref(),
            Some("isolated-helper-missing-upstream-glyph")
        );
        assert_eq!(case.unicorn_error, Some(6));
        assert_eq!(case.native_terminal_pc, Some(0x68144));
        assert_eq!(case.output_vector, Some([0; 3]));
        assert!(case.runs.is_empty());
        let input = case.cached_input.as_ref().unwrap();
        assert_eq!(input.entries.len(), 1);
        let entry = &input.entries[0];
        assert!(entry.glyphs.is_empty());
        assert!(!entry.drawable);
        assert!(entry.font.is_none());
        let emitted = emit_case(case);
        assert!(emitted.payloads.is_empty());
        match entry.kind {
            5 => {
                assert_eq!(
                    emitted.result,
                    Err(NativeRunError::MissingGlyph(0)),
                    "{}",
                    case.name
                );
                rejected_objects += 1;
            }
            3 => {
                let runs = emitted.result.unwrap();
                assert_eq!(runs.len(), 1);
                assert_eq!(runs[0].kind, NativeEmittedKind::DefaultEmpty);
                assert!(runs[0].glyphs.is_empty());
                assert_eq!(runs[0].font_source, None);
                assert_float_bits(runs[0].origin, [0.0; 2], &case.name);
                empty_defaults += 1;
            }
            kind => panic!("{}: unexpected empty control kind {kind}", case.name),
        }
    }
    assert_eq!(rejected_objects, 25);
    assert_eq!(empty_defaults, 1);
}
