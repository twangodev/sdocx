use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::*;

#[derive(Deserialize)]
struct Capture {
    font_sha256: String,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    text_utf8: String,
    range_utf16: [u32; 2],
    font_size_bits: u32,
    paint: Paint,
    hb_calls: Vec<HarfBuzzCall>,
    layout_piece: LayoutPiece,
    callbacks: Callbacks,
    #[serde(default)]
    shear_operations: Vec<ShearOperation>,
}

#[derive(Deserialize)]
struct ShearOperation {
    offset_x_bits: u32,
    offset_y_bits: u32,
    skew_bits: u32,
    result_bits: u32,
}

#[derive(Deserialize)]
struct Paint {
    size_bits: u32,
    scale_x_bits: u32,
    skew_x_bits: u32,
    letter_spacing_bits: u32,
    word_spacing_bits: u32,
}

#[derive(Deserialize)]
struct HarfBuzzCall {
    input: Input,
    output: Output,
}

#[derive(Deserialize)]
struct Input {
    direction: u32,
    script: u32,
}

#[derive(Deserialize)]
struct Output {
    infos: Vec<GlyphInfo>,
    positions: Vec<[i32; 4]>,
}

#[derive(Deserialize)]
struct GlyphInfo {
    glyph_id: u32,
    cluster: u32,
}

#[derive(Deserialize)]
struct LayoutPiece {
    font_indices: Vec<u8>,
    glyph_ids: Vec<u32>,
    owners_utf16: Vec<u32>,
    full_positions_bits: Vec<[u32; 2]>,
    owner_positions_bits: Vec<[u32; 2]>,
    ink_bounds_bits: Vec<[u32; 4]>,
    character_advances_bits: Vec<u32>,
    total_advance_bits: u32,
    font_fakery_bits: Vec<u32>,
}

#[derive(Deserialize)]
struct Callbacks {
    scalar_quantized: Vec<i32>,
    vector_raw_bits: Vec<u32>,
    vector_quantized: Vec<i32>,
    raw_skia_bounds_bits: Vec<[u32; 4]>,
}

fn parse_capture(bytes: &[u8], hash: &str) -> Capture {
    assert_eq!(format!("{:x}", Sha256::digest(bytes)), hash);
    let capture: Capture = serde_json::from_slice(bytes).unwrap();
    assert_eq!(
        format!(
            "{:x}",
            Sha256::digest(include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/assets/fonts/Roboto-Regular.ttf"
            )))
        ),
        capture.font_sha256
    );
    capture
}

fn capture() -> Capture {
    parse_capture(
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../conformance/table-text-shaping.json"
        )),
        "a4c58481cb1e36bb7de8783bea49a800b10b4227177684c48f5b5c356d7a425b",
    )
}

fn numeric_capture() -> Capture {
    parse_capture(
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../conformance/table-text-shaping-numeric.json"
        )),
        "1e476f7fc8254b2a6c7316c6ff18b9c436f8ab57707d536daa07e11eca2af455",
    )
}

fn gpos_capture() -> Capture {
    parse_capture(
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../conformance/table-text-shaping-gpos.json"
        )),
        "9280953ac5de0b6b745baedcda73cf4d88a95a7b52cbfbc99188e0b66a71efb2",
    )
}

impl Case {
    fn paint(&self) -> NativeShapingPaint {
        NativeShapingPaint {
            size: f32::from_bits(self.paint.size_bits),
            scale_x: f32::from_bits(self.paint.scale_x_bits),
            skew_x: f32::from_bits(self.paint.skew_x_bits),
            letter_spacing: f32::from_bits(self.paint.letter_spacing_bits),
            word_spacing: f32::from_bits(self.paint.word_spacing_bits),
        }
    }

    fn glyphs(&self) -> Vec<NativeShapedGlyph> {
        let [call] = self.hb_calls.as_slice() else {
            panic!("{} must have one captured chunk", self.name)
        };
        assert_eq!(call.output.infos.len(), call.output.positions.len());
        assert_eq!(
            call.output.infos.len(),
            self.callbacks.raw_skia_bounds_bits.len()
        );
        call.output
            .infos
            .iter()
            .zip(&call.output.positions)
            .zip(&self.callbacks.raw_skia_bounds_bits)
            .map(
                |((info, &[advance_x, advance_y, offset_x, offset_y]), &bounds)| {
                    NativeShapedGlyph {
                        id: info.glyph_id,
                        cluster_utf16: info.cluster,
                        advance_x,
                        advance_y,
                        offset_x,
                        offset_y,
                        ink_bounds: bounds.map(f32::from_bits),
                    }
                },
            )
            .collect()
    }

    fn chunk<'a>(&self, glyphs: &'a [NativeShapedGlyph]) -> NativeShapingChunk<'a> {
        NativeShapingChunk {
            source_utf16_length: self.text_utf8.encode_utf16().count() as u32,
            range_utf16: self.range_utf16[0]..self.range_utf16[1],
            direction: self.hb_calls[0].input.direction,
            script: self.hb_calls[0].input.script,
            font_slot: 0,
            font_fakery: 0,
            glyphs,
        }
    }
}

#[test]
fn native_post_shaping_geometry_matches_all_captured_bits() {
    let capture = capture();
    assert_eq!(capture.cases.len(), 16);
    let mut glyph_count = 0;
    let mut character_count = 0;
    for case in &capture.cases {
        let (glyphs, characters) = verify_case(case);
        glyph_count += glyphs;
        character_count += characters;
    }
    assert_eq!((glyph_count, character_count), (48, 49));
}

fn verify_case(case: &Case) -> (usize, usize) {
    assert_eq!(
        (f32::from_bits(case.font_size_bits) * 100.0).to_bits(),
        case.paint.size_bits,
        "{} paint size",
        case.name
    );
    assert!(case.layout_piece.font_indices.iter().all(|&slot| slot == 0));
    assert!(
        case.layout_piece
            .font_fakery_bits
            .iter()
            .all(|&bits| bits == 0)
    );
    let inputs = case.glyphs();
    let actual = native_shaping_geometry(case.paint(), &[case.chunk(&inputs)]).unwrap();
    assert_eq!(
        actual
            .glyphs
            .iter()
            .map(|glyph| glyph.id)
            .collect::<Vec<_>>(),
        case.layout_piece.glyph_ids,
        "{} glyph identity",
        case.name
    );
    assert_eq!(
        actual
            .glyphs
            .iter()
            .map(|glyph| glyph.owner_utf16)
            .collect::<Vec<_>>(),
        case.layout_piece.owners_utf16,
        "{} owners",
        case.name
    );
    for (label, actual, expected) in [
        (
            "full positions",
            actual
                .glyphs
                .iter()
                .map(|glyph| glyph.full_position.map(f32::to_bits))
                .collect::<Vec<_>>(),
            &case.layout_piece.full_positions_bits,
        ),
        (
            "owner positions",
            actual
                .glyphs
                .iter()
                .map(|glyph| glyph.owner_position.map(f32::to_bits))
                .collect(),
            &case.layout_piece.owner_positions_bits,
        ),
    ] {
        assert_eq!(&actual, expected, "{} {label}", case.name);
    }
    assert_eq!(
        actual
            .glyphs
            .iter()
            .map(|glyph| glyph.ink_bounds.map(f32::to_bits))
            .collect::<Vec<_>>(),
        case.layout_piece.ink_bounds_bits,
        "{} ink bounds",
        case.name
    );
    assert_eq!(
        actual
            .character_advances
            .iter()
            .copied()
            .map(f32::to_bits)
            .collect::<Vec<_>>(),
        case.layout_piece.character_advances_bits,
        "{} character advances",
        case.name
    );
    assert_eq!(
        actual.total_advance.to_bits(),
        case.layout_piece.total_advance_bits,
        "{} total advance",
        case.name
    );
    (actual.glyphs.len(), actual.character_advances.len())
}

#[test]
fn numeric_capture_pins_large_coordinates_spacing_and_rejects_multiple_chunks() {
    let capture = numeric_capture();
    assert_eq!(capture.cases.len(), 17);
    let mut matched_cases = 0;
    let mut glyph_count = 0;
    let mut character_count = 0;
    for case in &capture.cases {
        if case.hb_calls.len() != 1 {
            assert_eq!(case.name, "mixed_scripts");
            assert_eq!(case.hb_calls.len(), 3);
            let empty = [];
            let chunks = [case.chunk(&empty), case.chunk(&empty), case.chunk(&empty)];
            assert_eq!(
                native_shaping_geometry(case.paint(), &chunks).unwrap_err(),
                NativeShapingUnavailable::UnsupportedChunks
            );
            continue;
        }
        let (glyphs, characters) = verify_case(case);
        glyph_count += glyphs;
        character_count += characters;
        matched_cases += 1;
    }
    assert_eq!(
        (matched_cases, glyph_count, character_count),
        (16, 176, 176)
    );
}

#[test]
fn numeric_capture_detects_integer_narrowing_and_step_accumulation() {
    let capture = numeric_capture();
    let large = capture
        .cases
        .iter()
        .find(|case| case.name == "large_to_2000")
        .unwrap();
    let advance = large.hb_calls[0].output.positions[0][0];
    assert_eq!(advance, 28_076_755);
    assert_eq!((advance as f32) as i32, 28_076_756);
    assert_ne!(f64::from(paint_units(advance)), f64::from(advance) / 256.0);

    let long = capture
        .cases
        .iter()
        .find(|case| case.name == "long_pen")
        .unwrap();
    let postponed_narrowing = long.hb_calls[0]
        .output
        .positions
        .iter()
        .map(|position| f64::from(paint_units(position[0])))
        .sum::<f64>() as f32;
    assert_ne!(
        postponed_narrowing.to_bits(),
        long.layout_piece.total_advance_bits
    );
}

#[test]
fn gpos_capture_preserves_post_shaping_geometry() {
    let capture = gpos_capture();
    assert_eq!(capture.cases.len(), 4);
    let mut glyph_count = 0;
    let mut character_count = 0;
    for case in &capture.cases {
        let (glyphs, characters) = verify_case(case);
        glyph_count += glyphs;
        character_count += characters;
    }
    assert_eq!((glyph_count, character_count), (12, 12));
}

#[test]
fn native_shear_capture_distinguishes_fused_and_separate_rounding() {
    let capture = gpos_capture();
    let mut count = 0;
    let mut separate_rounding_differences = 0;
    for case in capture.cases {
        let glyphs = case.glyphs();
        assert_eq!(glyphs.len(), case.shear_operations.len());
        for (glyph, operation) in glyphs.iter().zip(&case.shear_operations) {
            assert_eq!(
                paint_units(glyph.offset_x).to_bits(),
                operation.offset_x_bits,
                "{} native shear X input",
                case.name
            );
            assert_eq!(
                paint_units(glyph.offset_y).to_bits(),
                operation.offset_y_bits,
                "{} native shear Y input",
                case.name
            );
            assert_eq!(operation.skew_bits, case.paint.skew_x_bits);
            let offset_x = f32::from_bits(operation.offset_x_bits);
            let offset_y = f32::from_bits(operation.offset_y_bits);
            let skew = f32::from_bits(operation.skew_bits);
            let fused = (-offset_y).mul_add(skew, offset_x);
            assert_eq!(
                fused.to_bits(),
                operation.result_bits,
                "{} actual native FMSUB result",
                case.name
            );
            let separately_rounded = offset_x - offset_y * skew;
            separate_rounding_differences +=
                usize::from(separately_rounded.to_bits() != operation.result_bits);
            count += 1;
        }
    }
    assert_eq!(count, 12);
    assert_eq!(separate_rounding_differences, 1);
}

#[test]
fn vector_metric_conversion_matches_native_callback_capture() {
    let mut count = 0;
    for case in capture()
        .cases
        .into_iter()
        .chain(numeric_capture().cases)
        .chain(gpos_capture().cases)
    {
        assert!(case.callbacks.scalar_quantized.is_empty());
        assert_eq!(
            case.callbacks.vector_raw_bits.len(),
            case.callbacks.vector_quantized.len()
        );
        for (&raw, &expected) in case
            .callbacks
            .vector_raw_bits
            .iter()
            .zip(&case.callbacks.vector_quantized)
        {
            assert_eq!(
                native_vector_advance(f32::from_bits(raw)),
                Some(expected),
                "{} metric callback",
                case.name
            );
            count += 1;
        }
    }
    assert_eq!(count, 242);
}

#[test]
fn captured_skew_and_utf16_owner_controls_detect_wrong_geometry() {
    let capture = capture();
    let skew = capture
        .cases
        .iter()
        .find(|case| case.name == "combining_skew")
        .unwrap();
    let mut paint = skew.paint();
    paint.skew_x = 0.0;
    let glyphs = skew.glyphs();
    let changed = native_shaping_geometry(paint, &[skew.chunk(&glyphs)]).unwrap();
    assert_ne!(
        changed.glyphs[1].full_position.map(f32::to_bits),
        skew.layout_piece.full_positions_bits[1]
    );
    assert_ne!(
        changed.glyphs[1].ink_bounds.map(f32::to_bits),
        skew.layout_piece.ink_bounds_bits[1]
    );
    let supplementary = capture
        .cases
        .iter()
        .find(|case| case.name == "supplementary_combining")
        .unwrap();
    let mut glyphs = supplementary.glyphs();
    for glyph in &mut glyphs {
        if glyph.cluster_utf16 >= 3 {
            glyph.cluster_utf16 -= 1;
        }
    }
    let changed =
        native_shaping_geometry(supplementary.paint(), &[supplementary.chunk(&glyphs)]).unwrap();
    assert_ne!(
        changed
            .character_advances
            .iter()
            .copied()
            .map(f32::to_bits)
            .collect::<Vec<_>>(),
        supplementary.layout_piece.character_advances_bits
    );
    assert_eq!(
        changed.total_advance.to_bits(),
        supplementary.layout_piece.total_advance_bits
    );
}

#[test]
fn unsupported_profiles_and_invalid_geometry_are_explicit() {
    let capture = capture();
    let case = &capture.cases[0];
    let glyphs = case.glyphs();
    fn expect(
        paint: NativeShapingPaint,
        chunk: NativeShapingChunk<'_>,
        error: NativeShapingUnavailable,
    ) {
        assert_eq!(native_shaping_geometry(paint, &[chunk]).unwrap_err(), error);
    }
    let mut paint = case.paint();
    paint.word_spacing = 1.0;
    expect(
        paint,
        case.chunk(&glyphs),
        NativeShapingUnavailable::UnsupportedPaint,
    );
    paint = case.paint();
    paint.skew_x = f32::NAN;
    expect(
        paint,
        case.chunk(&glyphs),
        NativeShapingUnavailable::NonFinitePaint,
    );
    let mut chunk = case.chunk(&glyphs);
    chunk.script = u32::from_be_bytes(*b"Arab");
    expect(
        case.paint(),
        chunk,
        NativeShapingUnavailable::UnsupportedScript,
    );
    let mut chunk = case.chunk(&glyphs);
    chunk.direction = 6;
    expect(
        case.paint(),
        chunk,
        NativeShapingUnavailable::UnsupportedDirection,
    );
    let mut chunk = case.chunk(&glyphs);
    chunk.font_slot = 1;
    expect(
        case.paint(),
        chunk,
        NativeShapingUnavailable::UnsupportedFontSlot,
    );
    let mut chunk = case.chunk(&glyphs);
    chunk.font_fakery = 1;
    expect(
        case.paint(),
        chunk,
        NativeShapingUnavailable::UnsupportedFontFakery,
    );
    let mut chunk = case.chunk(&glyphs);
    chunk.source_utf16_length = 1;
    expect(
        case.paint(),
        chunk,
        NativeShapingUnavailable::SourceRangeOutOfBounds,
    );
    let mut chunk = case.chunk(&glyphs);
    chunk.range_utf16 = Range { start: 2, end: 1 };
    expect(case.paint(), chunk, NativeShapingUnavailable::InvalidRange);
    let mut chunk = case.chunk(&glyphs);
    chunk.range_utf16 = 0..MAX_UTF16_ENTRIES as u32 + 1;
    expect(case.paint(), chunk, NativeShapingUnavailable::LimitExceeded);
    let mut invalid = glyphs.clone();
    invalid[0].cluster_utf16 = case.range_utf16[1];
    expect(
        case.paint(),
        case.chunk(&invalid),
        NativeShapingUnavailable::OwnerOutOfRange,
    );
    invalid = glyphs.clone();
    invalid[0].advance_y = 1;
    expect(
        case.paint(),
        case.chunk(&invalid),
        NativeShapingUnavailable::UnsupportedVerticalAdvance,
    );
    invalid = glyphs.clone();
    invalid[0].ink_bounds[0] = f32::INFINITY;
    expect(
        case.paint(),
        case.chunk(&invalid),
        NativeShapingUnavailable::NonFiniteInk,
    );
    invalid = glyphs.clone();
    invalid.reverse();
    expect(
        case.paint(),
        case.chunk(&invalid),
        NativeShapingUnavailable::NonMonotoneOwners,
    );
    assert_eq!(
        native_shaping_geometry(case.paint(), &[case.chunk(&glyphs), case.chunk(&glyphs)])
            .unwrap_err(),
        NativeShapingUnavailable::UnsupportedChunks
    );
    for raw in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 8_388_608.0] {
        assert_eq!(native_vector_advance(raw), None);
    }
    assert_eq!(native_vector_advance(-8_388_608.0), Some(i32::MIN));
}
