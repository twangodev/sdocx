use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::{FontBook, ResolvedFace};

const CAPTURE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../conformance/table-text-shaping-gpos.json"
));
const CAPTURE_SHA256: &str = "9280953ac5de0b6b745baedcda73cf4d88a95a7b52cbfbc99188e0b66a71efb2";

#[derive(Deserialize)]
struct Capture {
    font_sha256: String,
    font_face_index: u32,
    memory_fills: [u8; 3],
    repeat_zero_fill: bool,
    gpos_hook_addresses: [u64; 4],
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    hb_calls: Vec<ShapeCall>,
    callbacks: Callbacks,
    gpos_apply_values: Vec<ValueTrace>,
}

#[derive(Deserialize)]
struct ShapeCall {
    nativefont_scale: [i32; 2],
    output: ShapeOutput,
}

#[derive(Deserialize)]
struct ShapeOutput {
    infos: Vec<GlyphInfo>,
    positions: Vec<[i32; 4]>,
}

#[derive(Deserialize)]
struct GlyphInfo {
    glyph_id: u32,
    cluster: u32,
}

#[derive(Deserialize)]
struct Callbacks {
    vector_glyphs: Vec<Vec<u32>>,
    vector_quantized: Vec<i32>,
}

#[derive(Deserialize)]
struct ValueTrace {
    source_font_offset: usize,
    source_bytes: [u8; 2],
    value_i16: i16,
    multiplier_i64: i64,
    product_i64: i64,
    adjustment_i32: i32,
    value_format: u16,
    destination: String,
    glyph_index: usize,
    glyph_id: u32,
    cluster_utf16: u32,
    position_before: [i32; 4],
    position_after: [i32; 4],
}

fn capture() -> (Capture, ResolvedFace) {
    assert_eq!(format!("{:x}", Sha256::digest(CAPTURE)), CAPTURE_SHA256);
    let capture: Capture = serde_json::from_slice(CAPTURE).unwrap();
    let face = FontBook::default().resolve("Roboto", false, false).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(face.bytes())),
        capture.font_sha256
    );
    assert_eq!(face.index, capture.font_face_index);
    assert_eq!(capture.memory_fills, [0, 0xa5, 0xff]);
    assert!(capture.repeat_zero_fill);
    assert_eq!(
        capture.gpos_hook_addresses,
        [0xd013c, 0xd014c, 0xd0150, 0xd015c]
    );
    assert_eq!(capture.cases.len(), 4);
    (capture, face)
}

fn gpos_range(font: &[u8]) -> std::ops::Range<usize> {
    let count = usize::from(u16::from_be_bytes(font[4..6].try_into().unwrap()));
    let record = font[12..12 + count * 16]
        .as_chunks::<16>()
        .0
        .iter()
        .find(|record| &record[..4] == b"GPOS")
        .unwrap();
    let start = u32::from_be_bytes(record[8..12].try_into().unwrap()) as usize;
    let length = u32::from_be_bytes(record[12..16].try_into().unwrap()) as usize;
    assert!(start + length <= font.len());
    start..start + length
}

#[test]
fn native_pair_adjustment_operands_are_actual_pinned_gpos_values() {
    let (capture, face) = capture();
    let font = face.bytes();
    let table = gpos_range(font);
    assert_eq!(table.start, 0x71bf0);
    let expected = [
        ("av_default", 0x79464, -87),
        ("to_default", 0x79760, -99),
        ("large_default", 0x79464, -87),
        ("pair_combining_fma", 0x79464, -87),
    ];
    for (case, (name, offset, value)) in capture.cases.iter().zip(expected) {
        assert_eq!(case.name, name);
        assert_eq!(case.hb_calls.len(), 1);
        assert_eq!(case.gpos_apply_values.len(), 1);
        let trace = &case.gpos_apply_values[0];
        assert_eq!(trace.source_font_offset, offset);
        assert!(table.contains(&offset) && table.contains(&(offset + 1)));
        assert_eq!(font[offset..offset + 2], trace.source_bytes);
        assert_eq!(i16::from_be_bytes(trace.source_bytes), value);
        assert_eq!(trace.value_i16, value);
        assert_eq!(trace.value_format, 4);
        assert_eq!(trace.destination, "x_advance");
        let call = &case.hb_calls[0];
        let info = &call.output.infos[trace.glyph_index];
        assert_eq!(trace.glyph_id, info.glyph_id);
        assert_eq!(trace.cluster_utf16, info.cluster);
        assert_eq!(
            trace.position_after,
            call.output.positions[trace.glyph_index]
        );
        let callback_glyphs: Vec<_> = case
            .callbacks
            .vector_glyphs
            .iter()
            .flatten()
            .copied()
            .collect();
        assert_eq!(callback_glyphs[trace.glyph_index], trace.glyph_id);
        assert_eq!(
            case.callbacks.vector_quantized[trace.glyph_index],
            trace.position_before[0]
        );
    }
}

#[test]
fn native_signed_pair_scaling_floors_each_adjustment_before_adding() {
    let (capture, face) = capture();
    let mut round_to_nearest_differences = 0;
    for case in &capture.cases {
        let scale = case.hb_calls[0].nativefont_scale[0];
        for trace in &case.gpos_apply_values {
            let multiplier = (i64::from(scale) << 16) / i64::from(face.metrics.units_per_em);
            assert_eq!(trace.multiplier_i64, multiplier);
            let product = i64::from(trace.value_i16) * multiplier;
            assert_eq!(trace.product_i64, product);
            assert_eq!(trace.adjustment_i32, (product >> 16) as i32);
            assert!(product < 0 && trace.adjustment_i32 < 0);
            assert_ne!(product % 65536, 0);
            let mut expected = trace.position_before;
            expected[0] = expected[0].wrapping_add(trace.adjustment_i32);
            assert_eq!(trace.position_after, expected);
            let modern_rounded = ((product + 32768) >> 16) as i32;
            if modern_rounded != trace.adjustment_i32 {
                round_to_nearest_differences += 1;
            }
        }
    }
    assert_eq!(round_to_nearest_differences, 4);
    let av = &capture.cases[0].gpos_apply_values[0];
    assert_eq!(av.position_before[0], 283904);
    assert_eq!(av.adjustment_i32, -18488);
    assert_eq!(av.position_after[0], 265416);
}
