use std::collections::HashMap;

use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::{Direction, Feature, FontBook, ResolvedFace, ShapedRun, UnicodeBuffer};

#[derive(Deserialize)]
struct Capture {
    font_sha256: String,
    font_face_index: u32,
    memory_fills: [u8; 3],
    context_order: ContextOrder,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct ContextOrder {
    pre: String,
    post: String,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    text_utf8: String,
    range_utf16: [usize; 2],
    font_size_bits: u32,
    paint: PaintInput,
    layout_piece: LayoutPiece,
    hb_calls: Vec<HarfBuzzCall>,
}

#[derive(Deserialize)]
struct PaintInput {
    size_bits: u32,
    scale_x_bits: u32,
    skew_x_bits: u32,
    letter_spacing_bits: u32,
    word_spacing_bits: u32,
    packed_flags: u32,
}

#[derive(Deserialize)]
struct LayoutPiece {
    total_advance_bits: u32,
    glyph_ids: Vec<u32>,
    owners_utf16: Vec<u32>,
    font_indices: Vec<u32>,
}

#[derive(Deserialize)]
struct HarfBuzzCall {
    input: HarfBuzzInput,
    output: HarfBuzzOutput,
    nativefont_scale: [i32; 2],
    nativefont_ppem: [u32; 2],
}

#[derive(Clone, Deserialize)]
struct HarfBuzzInput {
    infos: Vec<UnicodeInfo>,
    direction: u32,
    script: u32,
    language: Option<String>,
    flags: u32,
    cluster_level: u32,
    content_type: u32,
    pre_context: Vec<u32>,
    post_context: Vec<u32>,
    features: Vec<FeatureInput>,
}

#[derive(Clone, Deserialize)]
struct UnicodeInfo {
    codepoint: u32,
    cluster: u32,
}

#[derive(Clone, Deserialize)]
struct FeatureInput {
    tag: u32,
    value: u32,
    start: u32,
    end: u32,
}

#[derive(Deserialize)]
struct HarfBuzzOutput {
    infos: Vec<GlyphInfo>,
    positions: Vec<[i32; 4]>,
    content_type: u32,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct GlyphInfo {
    glyph_id: u32,
    cluster: u32,
}

impl HarfBuzzInput {
    fn buffer(&self) -> UnicodeBuffer {
        let mut buffer = UnicodeBuffer::new();
        for info in &self.infos {
            buffer.add(char::from_u32(info.codepoint).unwrap(), info.cluster);
        }
        buffer.set_direction(match self.direction {
            4 => Direction::LeftToRight,
            5 => Direction::RightToLeft,
            other => panic!("unsupported captured horizontal direction {other}"),
        });
        buffer.set_script(
            rustybuzz::Script::from_iso15924_tag(tag(self.script))
                .expect("captured ISO 15924 script"),
        );
        if let Some(language) = &self.language {
            buffer.set_language(language.parse().expect("captured HarfBuzz language"));
        }
        buffer.set_flags(
            rustybuzz::BufferFlags::from_bits(self.flags)
                .expect("captured flags supported by rustybuzz"),
        );
        buffer.set_cluster_level(match self.cluster_level {
            0 => rustybuzz::BufferClusterLevel::MonotoneGraphemes,
            1 => rustybuzz::BufferClusterLevel::MonotoneCharacters,
            2 => rustybuzz::BufferClusterLevel::Characters,
            other => panic!("unsupported captured cluster level {other}"),
        });
        let preceding = self.pre_context.iter().rev().copied().collect::<Vec<_>>();
        buffer.set_pre_context(&context(&preceding));
        buffer.set_post_context(&context(&self.post_context));
        buffer
    }

    fn features(&self) -> Vec<Feature> {
        self.features
            .iter()
            .map(|feature| Feature {
                tag: tag(feature.tag),
                value: feature.value,
                start: feature.start,
                end: feature.end,
            })
            .collect()
    }
}

fn tag(value: u32) -> rustybuzz::ttf_parser::Tag {
    rustybuzz::ttf_parser::Tag::from_bytes(&value.to_be_bytes())
}

fn context(codepoints: &[u32]) -> String {
    codepoints
        .iter()
        .map(|&codepoint| char::from_u32(codepoint).expect("captured Unicode scalar"))
        .collect()
}

fn glyph_identities(run: &ShapedRun) -> Vec<GlyphInfo> {
    run.glyphs
        .iter()
        .map(|glyph| GlyphInfo {
            glyph_id: glyph.id,
            cluster: glyph.cluster,
        })
        .collect()
}

fn capture() -> Capture {
    pinned_capture(
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../conformance/table-text-shaping.json"
        )),
        "a4c58481cb1e36bb7de8783bea49a800b10b4227177684c48f5b5c356d7a425b",
    )
}

fn numeric_capture() -> Capture {
    pinned_capture(
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../conformance/table-text-shaping-numeric.json"
        )),
        "1e476f7fc8254b2a6c7316c6ff18b9c436f8ab57707d536daa07e11eca2af455",
    )
}

fn pinned_capture(bytes: &[u8], expected_sha256: &str) -> Capture {
    assert_eq!(format!("{:x}", Sha256::digest(bytes)), expected_sha256);
    serde_json::from_slice(bytes).unwrap()
}

fn captured_face(capture: &Capture) -> ResolvedFace {
    let face = FontBook::default().resolve("Roboto", false, false).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(face.bytes())),
        capture.font_sha256
    );
    assert_eq!(face.index, capture.font_face_index);
    assert_eq!(capture.memory_fills, [0, 0xa5, 0xff]);
    assert_eq!(capture.context_order.pre, "nearest_first");
    assert_eq!(capture.context_order.post, "source_order");
    face
}

#[test]
fn actual_native_harfbuzz_glyph_ids_and_utf16_ownership_match_rust_shaping() {
    compare_identities(capture(), [16, 16, 48, 49]);
}

#[test]
fn numeric_native_glyph_ids_and_utf16_ownership_match_across_script_chunks() {
    compare_identities(numeric_capture(), [17, 19, 182, 182]);
}

fn compare_identities(capture: Capture, counts: [usize; 4]) {
    let face = captured_face(&capture);
    assert_eq!(capture.cases.len(), counts[0]);
    assert_eq!(
        capture
            .cases
            .iter()
            .map(|case| case.hb_calls.len())
            .sum::<usize>(),
        counts[1]
    );
    assert_eq!(
        capture
            .cases
            .iter()
            .flat_map(|case| &case.hb_calls)
            .map(|call| call.output.infos.len())
            .sum::<usize>(),
        counts[2]
    );
    assert_eq!(
        capture
            .cases
            .iter()
            .map(|case| case.range_utf16[1] - case.range_utf16[0])
            .sum::<usize>(),
        counts[3]
    );
    for case in &capture.cases {
        assert!(case.range_utf16[0] < case.range_utf16[1], "{}", case.name);
        assert!(case.range_utf16[1] <= case.text_utf8.encode_utf16().count());
        assert_eq!(
            (f32::from_bits(case.font_size_bits) * 100.0).to_bits(),
            case.paint.size_bits,
            "{} paint size",
            case.name
        );
        let mut combined = Vec::new();
        assert_eq!(
            case.layout_piece.glyph_ids.len(),
            case.layout_piece.owners_utf16.len()
        );
        assert!(
            case.layout_piece
                .font_indices
                .iter()
                .all(|&index| index == 0)
        );
        for (index, call) in case.hb_calls.iter().enumerate() {
            assert_eq!(call.input.content_type, 1);
            assert_eq!(call.output.content_type, 2);
            assert_eq!(call.output.infos.len(), call.output.positions.len());
            assert!(call.nativefont_scale.into_iter().all(|scale| scale > 0));
            assert!(call.nativefont_ppem.into_iter().all(|ppem| ppem > 0));
            let shaped = face
                .shape(call.input.buffer(), &call.input.features())
                .unwrap();
            assert_eq!(
                glyph_identities(&shaped),
                call.output.infos,
                "{} HB call {index}",
                case.name
            );
            combined.extend(shaped.glyphs.iter().map(|glyph| {
                (
                    glyph.id,
                    glyph
                        .cluster
                        .checked_sub(case.range_utf16[0] as u32)
                        .expect("native range-relative source owner"),
                )
            }));
        }
        assert_eq!(
            combined,
            case.layout_piece
                .glyph_ids
                .iter()
                .copied()
                .zip(case.layout_piece.owners_utf16.iter().copied())
                .collect::<Vec<_>>(),
            "{} LayoutPiece glyph source ownership",
            case.name
        );
    }
}

#[test]
fn native_glyph_identity_comparison_detects_optional_ligatures() {
    let capture = capture();
    let face = captured_face(&capture);
    let case = capture
        .cases
        .iter()
        .find(|case| {
            case.text_utf8 == "office"
                && case.hb_calls.len() == 1
                && case.hb_calls[0].input.features.iter().any(|feature| {
                    feature.tag == u32::from_be_bytes(*b"liga") && feature.value == 0
                })
        })
        .expect("actual native optional ligature control");
    let call = &case.hb_calls[0];
    let mut enabled = call.input.features();
    for feature in &mut enabled {
        if [*b"liga", *b"clig"].contains(&feature.tag.to_bytes()) {
            feature.value = 1;
        }
    }
    let shaped = face.shape(call.input.buffer(), &enabled).unwrap();
    assert_ne!(glyph_identities(&shaped), call.output.infos);
}

#[test]
fn native_utf16_ownership_comparison_detects_utf8_cluster_offsets() {
    let capture = capture();
    let face = captured_face(&capture);
    let mut comparisons = 0;
    for case in &capture.cases {
        let mut utf16 = 0_u32;
        let offsets = case
            .text_utf8
            .char_indices()
            .map(|(byte, character)| {
                let pair = (utf16, byte as u32);
                utf16 += character.len_utf16() as u32;
                pair
            })
            .collect::<HashMap<_, _>>();
        for call in &case.hb_calls {
            let mut wrong = call.input.clone();
            let mut changed = false;
            for info in &mut wrong.infos {
                let byte = offsets[&info.cluster];
                changed |= byte != info.cluster;
                info.cluster = byte;
            }
            if changed {
                let shaped = face.shape(wrong.buffer(), &wrong.features()).unwrap();
                assert_eq!(
                    shaped
                        .glyphs
                        .iter()
                        .map(|glyph| glyph.id)
                        .collect::<Vec<_>>(),
                    call.output
                        .infos
                        .iter()
                        .map(|glyph| glyph.glyph_id)
                        .collect::<Vec<_>>(),
                    "{} unchanged glyphs with incorrect source units",
                    case.name
                );
                assert_ne!(
                    glyph_identities(&shaped),
                    call.output.infos,
                    "{}",
                    case.name
                );
                comparisons += 1;
            }
        }
    }
    assert!(
        comparisons > 0,
        "native multibyte ownership control required"
    );
}

#[test]
fn native_skia_metrics_remain_distinct_from_scaled_rust_font_units() {
    let capture = capture();
    let face = captured_face(&capture);
    let case = capture
        .cases
        .iter()
        .find(|case| {
            case.text_utf8 == "AV"
                && case.font_size_bits == 17_f32.to_bits()
                && case.paint.packed_flags == 0x20000
                && case.paint.scale_x_bits == 1_f32.to_bits()
                && case.paint.skew_x_bits == 0_f32.to_bits()
                && case.paint.letter_spacing_bits == 0_f32.to_bits()
                && case.paint.word_spacing_bits == 0_f32.to_bits()
                && case.hb_calls.len() == 1
        })
        .expect("actual native default-hinted AV control");
    let call = &case.hb_calls[0];
    let shaped = face
        .shape(call.input.buffer(), &call.input.features())
        .unwrap();
    let scaled_font_units = shaped.advance_x() as f64
        * f64::from(f32::from_bits(case.paint.size_bits))
        / f64::from(face.metrics.units_per_em);
    let native_advance = f32::from_bits(case.layout_piece.total_advance_bits);
    assert_ne!(f64::from(native_advance), scaled_font_units);
    assert_eq!(call.nativefont_scale, [435200, 435200]);
    assert_eq!(call.nativefont_ppem, [1700, 1700]);
}
