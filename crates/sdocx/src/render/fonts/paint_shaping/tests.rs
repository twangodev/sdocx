use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::*;
use crate::render::fonts::{FontBook, PaintInkBounds, PaintMetricMode, ResolvedFace};

#[derive(Deserialize)]
struct Capture {
    font_sha256: String,
    font_face_index: u32,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    text_utf8: String,
    paint: Paint,
    hb_calls: Vec<Call>,
    callbacks: Callbacks,
    layout_piece: ExpectedLayout,
}

#[derive(Deserialize)]
struct Paint {
    size_bits: u32,
    scale_x_bits: u32,
    skew_x_bits: u32,
    packed_flags: u32,
    letter_spacing_bits: u32,
    word_spacing_bits: u32,
}

#[derive(Deserialize)]
struct Call {
    nativefont_scale: [i32; 2],
    nativefont_ppem: [u32; 2],
    input: Input,
    output: Output,
}

#[derive(Deserialize)]
struct Input {
    infos: Vec<Info>,
    direction: u32,
    script: u32,
    language: Option<String>,
    flags: u32,
    cluster_level: u32,
    content_type: u32,
    pre_context: Vec<u32>,
    post_context: Vec<u32>,
    features: Vec<CapturedFeature>,
}

#[derive(Deserialize)]
struct Info {
    codepoint: u32,
    cluster: u32,
}

#[derive(Deserialize)]
struct CapturedFeature {
    tag: u32,
    value: u32,
    start: u32,
    end: u32,
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
struct ExpectedLayout {
    glyph_ids: Vec<u32>,
    owners_utf16: Vec<u32>,
    font_indices: Vec<u8>,
    font_fakery_bits: Vec<u32>,
    full_positions_bits: Vec<[u32; 2]>,
    owner_positions_bits: Vec<[u32; 2]>,
    ink_bounds_bits: Vec<[u32; 4]>,
    character_advances_bits: Vec<u32>,
    total_advance_bits: u32,
}

fn compare_layout(
    layout: &crate::render::fonts::PaintLayout,
    expected: &ExpectedLayout,
    name: &str,
) {
    assert_eq!(
        layout
            .glyphs
            .iter()
            .map(|glyph| glyph.id)
            .collect::<Vec<_>>(),
        expected.glyph_ids,
        "{name} layout IDs"
    );
    assert_eq!(
        layout
            .glyphs
            .iter()
            .map(|glyph| glyph.owner_utf16)
            .collect::<Vec<_>>(),
        expected.owners_utf16,
        "{name} layout owners"
    );
    assert_eq!(
        layout
            .glyphs
            .iter()
            .map(|glyph| glyph.full_position.map(f32::to_bits))
            .collect::<Vec<_>>(),
        expected.full_positions_bits,
        "{name} full positions"
    );
    assert_eq!(
        layout
            .glyphs
            .iter()
            .map(|glyph| glyph.owner_position.map(f32::to_bits))
            .collect::<Vec<_>>(),
        expected.owner_positions_bits,
        "{name} owner positions"
    );
    assert_eq!(
        layout
            .glyphs
            .iter()
            .map(|glyph| glyph.ink_bounds.map(f32::to_bits))
            .collect::<Vec<_>>(),
        expected.ink_bounds_bits,
        "{name} layout ink"
    );
    assert_eq!(
        layout
            .character_advances
            .iter()
            .copied()
            .map(f32::to_bits)
            .collect::<Vec<_>>(),
        expected.character_advances_bits,
        "{name} character advances"
    );
    assert_eq!(
        layout.total_advance.to_bits(),
        expected.total_advance_bits,
        "{name} total advance"
    );
}

#[derive(Deserialize)]
struct Callbacks {
    vector_raw_bits: Vec<u32>,
    raw_skia_bounds_bits: Vec<[u32; 4]>,
}

fn face() -> ResolvedFace {
    FontBook::default().resolve("Roboto", false, false).unwrap()
}

fn domain() -> PositioningDomain {
    let face = face();
    let font = FontRef::from_index(face.bytes(), face.index).unwrap();
    PositioningDomain::new(&font, paint(), PaintShapeScale::new(paint()).unwrap()).unwrap()
}

fn paint() -> PaintMetricInput {
    PaintMetricInput {
        size: 1700.0,
        scale_x: 1.0,
        skew_x: 0.0,
        mode: PaintMetricMode::Normal,
    }
}

fn simple_request<'a>(source: &'a str, infos: &'a [PaintSourceInfo]) -> PaintShapeRequest<'a> {
    PaintShapeRequest {
        source,
        infos,
        direction: PaintShapeDirection::LeftToRight,
        script: *b"Latn",
        language: None,
        flags: 0,
        cluster_level: PaintShapeClusterLevel::MonotoneGraphemes,
        pre_context: "",
        post_context: "",
        features: &[],
    }
}

fn infos(source: &str) -> Vec<PaintSourceInfo> {
    let mut owner_utf16 = 0;
    source
        .chars()
        .map(|character| {
            let info = PaintSourceInfo {
                character,
                owner_utf16,
            };
            owner_utf16 += character.len_utf16() as u32;
            info
        })
        .collect()
}

fn string(codepoints: impl IntoIterator<Item = u32>) -> String {
    codepoints
        .into_iter()
        .map(|codepoint| char::from_u32(codepoint).unwrap())
        .collect()
}

fn ink_bits(ink: PaintInkBounds) -> [u32; 4] {
    [ink.left, ink.top, ink.right, ink.bottom].map(f32::to_bits)
}

fn compare_capture(bytes: &[u8], hash: &str, expected: [usize; 3]) {
    assert_eq!(format!("{:x}", Sha256::digest(bytes)), hash);
    let capture: Capture = serde_json::from_slice(bytes).unwrap();
    let face = face();
    assert_eq!(
        format!("{:x}", Sha256::digest(face.bytes())),
        capture.font_sha256
    );
    assert_eq!(face.index, capture.font_face_index);
    let mut counts = [0; 3];
    for case in capture.cases {
        let input = PaintMetricInput {
            size: f32::from_bits(case.paint.size_bits),
            scale_x: f32::from_bits(case.paint.scale_x_bits),
            skew_x: f32::from_bits(case.paint.skew_x_bits),
            mode: match case.paint.packed_flags {
                0 => PaintMetricMode::Unhinted,
                0x20000 => PaintMetricMode::Normal,
                0x20040 => PaintMetricMode::Linear,
                other => panic!("unknown paint flags {other}"),
            },
        };
        let mut shaper = face.paint_shaper(input).unwrap();
        let mut callback_index = 0;
        let single_chunk = case.hb_calls.len() == 1;
        for call in case.hb_calls {
            assert_eq!(call.input.content_type, 1);
            assert_eq!([shaper.scale().x, shaper.scale().y], call.nativefont_scale);
            assert_eq!(shaper.scale().ppem, call.nativefont_ppem);
            let source_infos: Vec<_> = call
                .input
                .infos
                .iter()
                .map(|info| PaintSourceInfo {
                    character: char::from_u32(info.codepoint).unwrap(),
                    owner_utf16: info.cluster,
                })
                .collect();
            let features: Vec<_> = call
                .input
                .features
                .iter()
                .map(|feature| PaintShapeFeature {
                    tag: feature.tag.to_be_bytes(),
                    value: feature.value,
                    start: feature.start,
                    end: feature.end,
                })
                .collect();
            let pre = string(call.input.pre_context.iter().rev().copied());
            let post = string(call.input.post_context);
            let request = PaintShapeRequest {
                source: &case.text_utf8,
                infos: &source_infos,
                direction: match call.input.direction {
                    4 => PaintShapeDirection::LeftToRight,
                    5 => PaintShapeDirection::RightToLeft,
                    other => panic!("unverified native direction {other}"),
                },
                script: call.input.script.to_be_bytes(),
                language: call.input.language.as_deref(),
                flags: call.input.flags,
                cluster_level: match call.input.cluster_level {
                    0 => PaintShapeClusterLevel::MonotoneGraphemes,
                    1 => PaintShapeClusterLevel::MonotoneCharacters,
                    2 => PaintShapeClusterLevel::Characters,
                    other => panic!("unverified native cluster level {other}"),
                },
                pre_context: &pre,
                post_context: &post,
                features: &features,
            };
            let run = shaper
                .shape(request)
                .unwrap_or_else(|error| panic!("{}: {error}", case.name));
            if single_chunk {
                assert!(case.layout_piece.font_indices.iter().all(|&slot| slot == 0));
                assert!(
                    case.layout_piece
                        .font_fakery_bits
                        .iter()
                        .all(|&fakery| fakery == 0)
                );
                let layout = run
                    .layout(
                        f32::from_bits(case.paint.letter_spacing_bits),
                        f32::from_bits(case.paint.word_spacing_bits),
                    )
                    .unwrap();
                compare_layout(&layout, &case.layout_piece, &case.name);
                counts[2] += layout.glyphs.len();
            } else if run.script != *b"Latn" {
                assert_eq!(
                    run.layout(0.0, 0.0).unwrap_err(),
                    crate::render::fonts::PaintLayoutError::UnsupportedScript
                );
            }
            assert_eq!(run.glyphs.len(), call.output.infos.len());
            for ((glyph, info), position) in run
                .glyphs
                .iter()
                .zip(call.output.infos)
                .zip(call.output.positions)
            {
                assert_eq!(
                    glyph.id, info.glyph_id,
                    "{} glyph {callback_index}",
                    case.name
                );
                assert_eq!(
                    glyph.owner_utf16, info.cluster,
                    "{} owner {callback_index}",
                    case.name
                );
                assert_eq!(
                    [
                        glyph.x_advance,
                        glyph.y_advance,
                        glyph.x_offset,
                        glyph.y_offset
                    ],
                    position,
                    "{} position {callback_index}",
                    case.name
                );
                assert_eq!(
                    glyph.backend.advance.to_bits(),
                    case.callbacks.vector_raw_bits[callback_index],
                    "{} raw advance {callback_index}",
                    case.name
                );
                assert_eq!(
                    ink_bits(glyph.backend.ink),
                    case.callbacks.raw_skia_bounds_bits[callback_index],
                    "{} ink {callback_index}",
                    case.name
                );
                callback_index += 1;
                counts[1] += 1;
            }
            counts[0] += 1;
        }
        assert_eq!(callback_index, case.callbacks.vector_raw_bits.len());
    }
    assert_eq!(counts, expected);
}

#[test]
fn provider_derived_shaping_matches_native_profiles_context_and_owners() {
    compare_capture(
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../conformance/table-text-shaping.json"
        )),
        "a4c58481cb1e36bb7de8783bea49a800b10b4227177684c48f5b5c356d7a425b",
        [16, 48, 48],
    );
}

#[test]
fn provider_derived_high_scale_shaping_matches_native_gpos_and_script_chunks() {
    compare_capture(
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../conformance/table-text-shaping-numeric.json"
        )),
        "1e476f7fc8254b2a6c7316c6ff18b9c436f8ab57707d536daa07e11eca2af455",
        [19, 182, 176],
    );
    compare_capture(
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../conformance/table-text-shaping-gpos.json"
        )),
        "9280953ac5de0b6b745baedcda73cf4d88a95a7b52cbfbc99188e0b66a71efb2",
        [4, 12, 12],
    );
}

#[test]
fn provider_derived_skew_shaping_and_layout_match_native_transform_metrics() {
    compare_capture(
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../conformance/table-text-shaping-skia-metrics.json"
        )),
        "c8aa3d08839e9f7e7ddff6d9200616faa11adb67f8a7124f7fd91d5088c9eec0",
        [18, 108, 108],
    );
}

#[test]
fn scalar_and_vector_callback_rounding_are_distinct_and_cached() {
    let mut measure_count = 0;
    let mut measure = |_| {
        measure_count += 1;
        Ok(PaintGlyphMetrics {
            advance: 1.0 + 1.0 / 512.0,
            ink: PaintInkBounds::default(),
        })
    };
    let mut cache = HashMap::new();
    let mut callbacks = PaintCallbacks {
        measure: &mut measure,
        cache: &mut cache,
        error: None,
        advance_limit: u32::MAX,
        domain: &domain(),
    };
    let glyph = GlyphId::new(36);
    assert_eq!(callbacks.advance(glyph, true), 257);
    assert_eq!(callbacks.advance(glyph, false), 256);
    assert_eq!(callbacks.advance(glyph, true), 257);
    assert!(callbacks.error.is_none());
    assert_eq!(measure_count, 1);
}

#[test]
fn callback_errors_are_returned_after_shaping_without_silent_metric_fallback() {
    let face = face();
    let font = FontRef::from_index(face.bytes(), face.index).unwrap();
    let data = ShaperData::new(&font);
    let domain =
        PositioningDomain::new(&font, paint(), PaintShapeScale::new(paint()).unwrap()).unwrap();
    let source_infos = infos("AV");
    let request = simple_request("AV", &source_infos);
    let mut cache = HashMap::new();
    let error = shape_with_metrics(
        (&font, &data),
        paint(),
        request,
        &mut cache,
        &mut |_| Err(PaintMetricError::InvalidGlyph { glyph_id: 36 }.into()),
        IntegerScalingRounding::Floor,
        &domain,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        PaintShapeError::Metrics(PaintMetricError::InvalidGlyph { glyph_id: 36 })
    ));
    let error = shape_with_metrics(
        (&font, &data),
        paint(),
        request,
        &mut cache,
        &mut |_| {
            Ok(PaintGlyphMetrics {
                advance: f32::NAN,
                ink: PaintInkBounds::default(),
            })
        },
        IntegerScalingRounding::Floor,
        &domain,
    )
    .unwrap_err();
    assert!(matches!(error, PaintShapeError::AdvanceRange));
}

#[test]
fn utf16_source_validation_rejects_bytes_surrogate_interiors_and_missing_scalars() {
    let face = face();
    let mut shaper = face.paint_shaper(paint()).unwrap();
    let source = "A😀x";
    let valid = infos(source);
    assert_eq!(
        valid
            .iter()
            .map(|info| info.owner_utf16)
            .collect::<Vec<_>>(),
        [0, 1, 3]
    );
    assert!(shaper.shape(simple_request(source, &valid)).is_ok());
    for invalid in [
        vec![valid[0], valid[2]],
        vec![
            valid[0],
            PaintSourceInfo {
                owner_utf16: 2,
                ..valid[1]
            },
        ],
        vec![
            valid[0],
            valid[1],
            PaintSourceInfo {
                owner_utf16: 5,
                ..valid[2]
            },
        ],
        vec![PaintSourceInfo {
            character: 'B',
            ..valid[0]
        }],
    ] {
        assert!(matches!(
            shaper.shape(simple_request(source, &invalid)),
            Err(PaintShapeError::InvalidSource)
        ));
    }
}

#[test]
fn properties_budgets_and_scale_domain_are_checked() {
    let face = face();
    let mut shaper = face.paint_shaper(paint()).unwrap();
    let source_infos = infos("AV");
    let request = simple_request("AV", &source_infos);
    assert!(matches!(
        shaper.shape(PaintShapeRequest {
            flags: 0x100,
            ..request
        }),
        Err(PaintShapeError::InvalidProperties)
    ));
    assert!(matches!(
        shaper.shape(PaintShapeRequest {
            pre_context: "abcdef",
            ..request
        }),
        Err(PaintShapeError::InputBudget)
    ));
    assert!(matches!(
        shaper.shape(PaintShapeRequest {
            language: Some(""),
            ..request
        }),
        Err(PaintShapeError::InvalidProperties)
    ));
    let invalid_features = [PaintShapeFeature {
        tag: *b"kern",
        value: 1,
        start: 2,
        end: 1,
    }];
    assert!(matches!(
        shaper.shape(PaintShapeRequest {
            features: &invalid_features,
            ..request
        }),
        Err(PaintShapeError::InvalidProperties)
    ));
    let oversized = "A".repeat(MAX_SOURCE_UTF16 as usize + 1);
    assert!(matches!(
        shaper.shape(PaintShapeRequest {
            source: &oversized,
            ..request
        }),
        Err(PaintShapeError::InputBudget)
    ));
    for size in [f32::NAN, f32::INFINITY, 0.0, -1.0, 8_388_608.0] {
        assert!(matches!(
            PaintShapeScale::new(PaintMetricInput { size, ..paint() }),
            Err(PaintShapeError::InvalidScale)
        ));
    }
}

#[test]
fn repeated_shape_calls_reuse_each_paint_metric_without_changing_positions() {
    let face = face();
    let font = FontRef::from_index(face.bytes(), face.index).unwrap();
    let data = ShaperData::new(&font);
    let domain =
        PositioningDomain::new(&font, paint(), PaintShapeScale::new(paint()).unwrap()).unwrap();
    let metrics = face.paint_metrics(paint()).unwrap();
    let source_infos = infos("AVAV");
    let mut measured = Vec::new();
    let mut cache = HashMap::new();
    let mut measure = |glyph| {
        measured.push(glyph);
        metrics.glyph(glyph).map_err(Into::into)
    };
    let mut shape = || {
        shape_with_metrics(
            (&font, &data),
            paint(),
            simple_request("AVAV", &source_infos),
            &mut cache,
            &mut measure,
            IntegerScalingRounding::Floor,
            &domain,
        )
        .unwrap()
    };
    let first = shape();
    let second = shape();
    assert_eq!(first, second);
    assert_eq!(measured, [38, 59]);
}

#[test]
fn unverified_parent_extents_fail_explicitly_when_fallback_marks_need_them() {
    let face = face();
    let mut bytes = face.bytes().to_vec();
    let count = usize::from(u16::from_be_bytes(bytes[4..6].try_into().unwrap()));
    let record = bytes[12..12 + count * 16]
        .as_chunks_mut::<16>()
        .0
        .iter_mut()
        .find(|record| &record[..4] == b"GPOS")
        .unwrap();
    record[..4].copy_from_slice(b"ZZZZ");
    let mut shaper = PaintShaper::new(&bytes, face.index, paint()).unwrap();
    let source = "x\u{327}\u{301}";
    let source_infos = infos(source);
    assert!(matches!(
        shaper.shape(simple_request(source, &source_infos)),
        Err(PaintShapeError::UnsupportedCallback("glyph extents"))
    ));
}

#[test]
fn glyph_expansion_budget_returns_a_typed_error_and_preserves_shaper_reuse() {
    let bytes = harfrust::shaping_expansion_test_font(false);
    let mut shaper = PaintShaper::new(&bytes, 0, paint()).unwrap();
    let source = "A".repeat(300);
    let source_infos = infos(&source);
    assert!(matches!(
        shaper.shape(simple_request(&source, &source_infos)),
        Err(PaintShapeError::ShapingBudget)
    ));
    let source_infos = infos("B");
    let run = shaper.shape(simple_request("B", &source_infos)).unwrap();
    assert_eq!(run.glyphs().len(), 1);
    assert!(run.glyphs().iter().all(|glyph| glyph.owner_utf16 == 0));
    assert_eq!(run.source_utf16_length(), 1);
    assert_eq!(run.source_range_utf16(), 0..1);
    assert_eq!(run.script(), *b"Latn");
    assert_eq!(run.paint(), paint());
    assert_eq!(run.scale(), shaper.scale());
    assert_eq!(run.direction(), PaintShapeDirection::LeftToRight);
}

#[test]
fn scalar_callback_keeps_native_negative_conversion_order() {
    let mut cache = HashMap::new();
    let mut measure = |_| {
        Ok(PaintGlyphMetrics {
            advance: -1.0,
            ink: PaintInkBounds::default(),
        })
    };
    let mut callbacks = PaintCallbacks {
        measure: &mut measure,
        cache: &mut cache,
        error: None,
        advance_limit: u32::MAX,
        domain: &domain(),
    };
    let glyph = GlyphId::new(38);
    assert_eq!(callbacks.advance(glyph, true), -255);
    assert_eq!(callbacks.advance(glyph, false), -256);
}

#[test]
fn measured_callback_advances_cannot_escape_the_font_positioning_bound() {
    let face = face();
    let font = FontRef::from_index(face.bytes(), face.index).unwrap();
    let data = ShaperData::new(&font);
    let domain =
        PositioningDomain::new(&font, paint(), PaintShapeScale::new(paint()).unwrap()).unwrap();
    let source_infos = infos("AV");
    let advance = 5_000_000.0;
    let mut cache = HashMap::new();
    let error = shape_with_metrics(
        (&font, &data),
        paint(),
        simple_request("AV", &source_infos),
        &mut cache,
        &mut |_| {
            Ok(PaintGlyphMetrics {
                advance,
                ink: PaintInkBounds::default(),
            })
        },
        IntegerScalingRounding::Floor,
        &domain,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        PaintShapeError::UnsupportedPositioningDomain
    ));
}
