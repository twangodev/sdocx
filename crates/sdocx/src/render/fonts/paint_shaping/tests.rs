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
    range_utf16: [u32; 2],
    #[serde(default)]
    entry_geometry: Option<ExpectedEntryGeometry>,
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
    #[serde(default)]
    feature_settings: String,
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
            .glyphs()
            .iter()
            .map(|glyph| glyph.id)
            .collect::<Vec<_>>(),
        expected.glyph_ids,
        "{name} layout IDs"
    );
    assert_eq!(
        layout
            .glyphs()
            .iter()
            .map(|glyph| glyph.owner_utf16)
            .collect::<Vec<_>>(),
        expected.owners_utf16,
        "{name} layout owners"
    );
    assert_eq!(
        layout
            .glyphs()
            .iter()
            .map(|glyph| glyph.full_position.map(f32::to_bits))
            .collect::<Vec<_>>(),
        expected.full_positions_bits,
        "{name} full positions"
    );
    assert_eq!(
        layout
            .glyphs()
            .iter()
            .map(|glyph| glyph.owner_position.map(f32::to_bits))
            .collect::<Vec<_>>(),
        expected.owner_positions_bits,
        "{name} owner positions"
    );
    assert_eq!(
        layout
            .glyphs()
            .iter()
            .map(|glyph| glyph.ink_bounds.map(f32::to_bits))
            .collect::<Vec<_>>(),
        expected.ink_bounds_bits,
        "{name} layout ink"
    );
    assert_eq!(
        layout
            .character_advances()
            .iter()
            .copied()
            .map(f32::to_bits)
            .collect::<Vec<_>>(),
        expected.character_advances_bits,
        "{name} character advances"
    );
    assert_eq!(
        layout.total_advance().to_bits(),
        expected.total_advance_bits,
        "{name} total advance"
    );
}

#[derive(Deserialize)]
struct Callbacks {
    vector_glyphs: Vec<Vec<u32>>,
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
    compare_capture_mode(bytes, hash, expected, false);
}

pub(super) fn compare_factory_capture(bytes: &[u8], hash: &str, expected: [usize; 3]) {
    compare_capture_mode(bytes, hash, expected, true);
}

fn compare_capture_mode(bytes: &[u8], hash: &str, expected: [usize; 3], factory: bool) {
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
        let itemization = factory.then(|| {
            crate::render::fonts::PaintItemization::new(
                &case.text_utf8,
                case.range_utf16[0]..case.range_utf16[1],
            )
            .unwrap()
        });
        let caller_features = match case.paint.feature_settings.as_str() {
            "" => Vec::new(),
            "liga=1,clig=1" => [*b"liga", *b"clig"]
                .map(|tag| PaintShapeFeature {
                    tag,
                    value: 1,
                    start: 0,
                    end: u32::MAX,
                })
                .to_vec(),
            other => panic!("unverified captured caller feature settings {other}"),
        };
        let factory_request = itemization.as_ref().map(|itemization| PaintTextRequest {
            itemization,
            direction: match case.hb_calls[0].input.direction {
                4 => PaintShapeDirection::LeftToRight,
                5 => PaintShapeDirection::RightToLeft,
                other => panic!("unverified native direction {other}"),
            },
            letter_spacing: f32::from_bits(case.paint.letter_spacing_bits),
            word_spacing: f32::from_bits(case.paint.word_spacing_bits),
            features: &caller_features,
        });
        if let Some(request) = &factory_request {
            assert_eq!(
                request.itemization.chunks().len(),
                case.hb_calls.len(),
                "{} chunk count",
                case.name
            );
        }
        let factory_piece = factory_request.map(|request| {
            shaper
                .shape_text(request)
                .unwrap_or_else(|error| panic!("{} factory: {error}", case.name))
        });
        let mut callback_index = 0;
        let callback_glyphs: Vec<_> = case
            .callbacks
            .vector_glyphs
            .iter()
            .flatten()
            .copied()
            .collect();
        assert_eq!(callback_glyphs.len(), case.callbacks.vector_raw_bits.len());
        let mut runs = Vec::new();
        for (call_index, call) in case.hb_calls.iter().enumerate() {
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
            let post = string(call.input.post_context.iter().copied());
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
            let run = if let Some(piece) = &factory_piece {
                let factory_request = factory_request.as_ref().unwrap();
                let chunk = &factory_request.itemization.chunks()[call_index];
                let features = factory_request.chunk_features(chunk).unwrap();
                let actual_request = factory_request.chunk_request(chunk, &features);
                assert_eq!(
                    actual_request.source, request.source,
                    "{} source",
                    case.name
                );
                assert_eq!(actual_request.infos, request.infos, "{} infos", case.name);
                assert_eq!(
                    actual_request.script, request.script,
                    "{} script",
                    case.name
                );
                assert_eq!(
                    actual_request.direction, request.direction,
                    "{} direction",
                    case.name
                );
                assert_eq!(
                    actual_request.language, request.language,
                    "{} language",
                    case.name
                );
                assert_eq!(actual_request.flags, request.flags, "{} flags", case.name);
                assert_eq!(
                    actual_request.cluster_level, request.cluster_level,
                    "{} cluster level",
                    case.name
                );
                assert_eq!(
                    actual_request.pre_context, request.pre_context,
                    "{} pre context",
                    case.name
                );
                assert_eq!(
                    actual_request.post_context, request.post_context,
                    "{} post context",
                    case.name
                );
                assert_eq!(
                    actual_request.features, request.features,
                    "{} features",
                    case.name
                );
                assert_eq!(piece.source(), case.text_utf8);
                assert_eq!(
                    piece.source_range_utf16(),
                    case.range_utf16[0]..case.range_utf16[1]
                );
                assert_eq!(piece.paint(), input);
                assert_eq!(piece.scale(), shaper.scale());
                assert_eq!(piece.direction(), request.direction);
                piece.shaped_runs()[call_index].clone()
            } else {
                shaper
                    .shape(request)
                    .unwrap_or_else(|error| panic!("{}: {error}", case.name))
            };
            assert_eq!(run.glyphs.len(), call.output.infos.len());
            for ((glyph, info), position) in run
                .glyphs
                .iter()
                .zip(&call.output.infos)
                .zip(&call.output.positions)
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
                    *position,
                    "{} position {callback_index}",
                    case.name
                );
                assert_eq!(
                    shaper
                        .metrics
                        .glyph(callback_glyphs[callback_index])
                        .unwrap()
                        .advance
                        .to_bits(),
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
            runs.push(run);
        }
        assert!(case.layout_piece.font_indices.iter().all(|&slot| slot == 0));
        assert!(
            case.layout_piece
                .font_fakery_bits
                .iter()
                .all(|&fakery| fakery == 0)
        );
        let letter_spacing = f32::from_bits(case.paint.letter_spacing_bits);
        let word_spacing = f32::from_bits(case.paint.word_spacing_bits);
        let layout = if let Some(piece) = &factory_piece {
            piece.layout().clone()
        } else {
            match runs.as_slice() {
                [run] => run.layout(letter_spacing, word_spacing),
                _ => crate::render::fonts::PaintLayout::from_runs(
                    &runs.iter().collect::<Vec<_>>(),
                    letter_spacing,
                    word_spacing,
                ),
            }
            .unwrap()
        };
        compare_layout(&layout, &case.layout_piece, &case.name);
        if let Some(expected_entries) = &case.entry_geometry {
            if let Some(piece) = &factory_piece {
                compare_actual_entry_geometry(piece.entry_geometry(), &case, expected_entries);
            } else {
                compare_entry_geometry(&layout, &case, expected_entries);
            }
        }
        counts[2] += layout.glyphs().len();
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
        [19, 182, 182],
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

#[test]
fn stitched_runs_require_semantically_identical_sources_fonts_and_paint() {
    use crate::render::fonts::{PaintLayout, PaintLayoutError};

    let face = face();
    let mut shaper = face.paint_shaper(paint()).unwrap();
    let source = String::from("AV");
    let first_infos = [PaintSourceInfo {
        character: 'A',
        owner_utf16: 0,
    }];
    let last_infos = [PaintSourceInfo {
        character: 'V',
        owner_utf16: 1,
    }];
    let first = shaper.shape(simple_request(&source, &first_infos)).unwrap();
    let same_text = String::from("AV");
    let last = shaper
        .shape(simple_request(&same_text, &last_infos))
        .unwrap();
    assert!(Arc::ptr_eq(&first.source, &last.source));
    let layout = PaintLayout::from_runs(&[&first, &last], 0.0, 0.0).unwrap();
    assert_eq!(
        PaintLayout::from_runs(&[&last, &first], 0.0, 0.0).unwrap_err(),
        PaintLayoutError::UnsupportedChunks
    );
    assert_eq!(layout.source(), "AV");
    assert_eq!(layout.source_range_utf16(), 0..2);

    let mut independent = face.paint_shaper(paint()).unwrap();
    let independent_last = independent
        .shape(simple_request(&same_text, &last_infos))
        .unwrap();
    assert!(!Arc::ptr_eq(&first.source, &independent_last.source));
    assert_eq!(
        PaintLayout::from_runs(&[&first, &independent_last], 0.0, 0.0).unwrap(),
        layout
    );

    let different_infos = [PaintSourceInfo {
        character: 'B',
        owner_utf16: 1,
    }];
    let different_source = shaper
        .shape(simple_request("AB", &different_infos))
        .unwrap();
    assert_eq!(
        PaintLayout::from_runs(&[&first, &different_source], 0.0, 0.0).unwrap_err(),
        PaintLayoutError::IncompatibleChunks
    );

    let mut font_bytes = face.bytes().to_vec();
    font_bytes.push(0);
    let mut different_font = PaintShaper::new(&font_bytes, face.index, paint()).unwrap();
    let font_last = different_font
        .shape(simple_request("AV", &last_infos))
        .unwrap();
    assert_eq!(
        PaintLayout::from_runs(&[&first, &font_last], 0.0, 0.0).unwrap_err(),
        PaintLayoutError::IncompatibleChunks
    );

    let mut different_paint = face
        .paint_shaper(PaintMetricInput {
            size: 1600.0,
            ..paint()
        })
        .unwrap();
    let paint_last = different_paint
        .shape(simple_request("AV", &last_infos))
        .unwrap();
    assert_eq!(
        PaintLayout::from_runs(&[&first, &paint_last], 0.0, 0.0).unwrap_err(),
        PaintLayoutError::IncompatibleChunks
    );
    assert_eq!(
        PaintLayout::from_runs(&[&first, &first], 0.0, 0.0).unwrap_err(),
        PaintLayoutError::UnsupportedChunks
    );
}

#[test]
fn provider_derived_multi_chunk_layout_matches_native_scripts_spacing_and_owners() {
    compare_capture(
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../conformance/table-text-shaping-mixed-scripts.json"
        )),
        "00d1634d29634fac146b42baa1e1a349bd66a2a74724ca7e7d5c539f116703ad",
        [380, 800, 800],
    );
}

#[derive(Deserialize)]
struct ExpectedEntryGeometry {
    source_utf16: Vec<u16>,
    glyphs: Vec<ExpectedEntryGlyph>,
    entry_widths_bits: Vec<u32>,
    entry_ink_bits: Vec<[u32; 4]>,
}

#[derive(Deserialize)]
struct ExpectedEntryGlyph {
    glyph_id: u32,
    owner_utf16: u32,
    entry_position_bits: [u32; 2],
    entry_ink_bits: [u32; 4],
}

fn compare_entry_geometry(
    layout: &crate::render::fonts::PaintLayout,
    case: &Case,
    expected: &ExpectedEntryGeometry,
) {
    let actual = layout.entry_geometry().unwrap();
    compare_actual_entry_geometry(&actual, case, expected);
}

fn compare_actual_entry_geometry(
    actual: &crate::render::fonts::PaintEntryGeometry,
    case: &Case,
    expected: &ExpectedEntryGeometry,
) {
    assert_eq!(
        actual.source(),
        case.text_utf8,
        "{} entry source",
        case.name
    );
    assert_eq!(
        actual.source_range_utf16(),
        case.range_utf16[0]..case.range_utf16[1],
        "{} entry range",
        case.name
    );
    assert_eq!(
        actual.source().encode_utf16().collect::<Vec<_>>(),
        expected.source_utf16,
        "{} entry source scalars",
        case.name
    );
    assert_eq!(actual.glyphs().len(), expected.glyphs.len());
    for (index, (glyph, expected_glyph)) in actual.glyphs().iter().zip(&expected.glyphs).enumerate()
    {
        assert_eq!(
            glyph.id(),
            expected_glyph.glyph_id,
            "{} entry glyph ID",
            case.name
        );
        assert_eq!(
            glyph.owner_utf16(),
            expected_glyph.owner_utf16,
            "{} absolute entry owner",
            case.name
        );
        assert_eq!(
            glyph.position().map(f32::to_bits),
            expected_glyph.entry_position_bits,
            "{} entry glyph position",
            case.name
        );
        assert_eq!(
            glyph.ink_bounds().map(f32::to_bits),
            expected_glyph.entry_ink_bits,
            "{} entry glyph ink",
            case.name
        );
        assert!(
            actual
                .entry_at_utf16(glyph.owner_utf16())
                .unwrap()
                .glyphs()
                .contains(&index)
        );
    }
    let mut widths = vec![0; expected.source_utf16.len()];
    let mut inks = vec![[0; 4]; expected.source_utf16.len()];
    let mut entry_count = 0;
    for owner in 0..expected.source_utf16.len() as u32 {
        if let Some(entry) = actual.entry_at_utf16(owner) {
            entry_count += 1;
            widths[owner as usize] = entry.advance().to_bits();
            inks[owner as usize] = entry.ink_bounds().map(f32::to_bits);
            for glyph in &actual.glyphs()[entry.glyphs()] {
                assert_eq!(
                    glyph.owner_utf16(),
                    owner,
                    "{} entry glyph range",
                    case.name
                );
            }
        }
    }
    assert_eq!(entry_count, actual.entries().len());
    assert_eq!(
        widths, expected.entry_widths_bits,
        "{} entry widths and zero padding",
        case.name
    );
    assert_eq!(
        inks, expected.entry_ink_bits,
        "{} owner ink unions and zero padding",
        case.name
    );
}

#[test]
fn provider_derived_logical_entries_match_native_append_and_owner_bounds() {
    compare_capture(
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../conformance/table-text-entry-geometry.json"
        )),
        "4615a8778c0a7b6301bd9efddbd591da2fd00278d4a3f0db06b04e4b1c923dc4",
        [8, 25, 25],
    );
}

fn entry_capture() -> Capture {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/table-text-entry-geometry.json"
    ));
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "4615a8778c0a7b6301bd9efddbd591da2fd00278d4a3f0db06b04e4b1c923dc4"
    );
    serde_json::from_slice(bytes).unwrap()
}

#[test]
fn captured_layout_geometry_converts_to_exact_native_logical_entries() {
    use crate::render::fonts::paint_layout::{
        PaintLayoutChunk, PaintLayoutGlyphInput, PaintLayoutPaint, paint_layout,
    };
    let capture = entry_capture();
    assert_eq!(capture.cases.len(), 8);
    let mut glyph_count = 0;
    for case in &capture.cases {
        let source: Arc<str> = Arc::from(case.text_utf8.as_str());
        let mut bounds = case.callbacks.raw_skia_bounds_bits.iter();
        let inputs: Vec<Vec<_>> = case
            .hb_calls
            .iter()
            .map(|call| {
                call.output
                    .infos
                    .iter()
                    .zip(&call.output.positions)
                    .map(|(info, &[advance_x, advance_y, offset_x, offset_y])| {
                        PaintLayoutGlyphInput {
                            id: info.glyph_id,
                            cluster_utf16: info.cluster,
                            advance_x,
                            advance_y,
                            offset_x,
                            offset_y,
                            ink_bounds: bounds.next().unwrap().map(f32::from_bits),
                        }
                    })
                    .collect()
            })
            .collect();
        assert!(bounds.next().is_none());
        let chunks: Vec<_> = case
            .hb_calls
            .iter()
            .zip(&inputs)
            .map(|(call, glyphs)| {
                let first = call.input.infos.first().unwrap();
                let last = call.input.infos.last().unwrap();
                PaintLayoutChunk {
                    source: Arc::clone(&source),
                    source_utf16_length: source.encode_utf16().count() as u32,
                    range_utf16: first.cluster
                        ..last.cluster + char::from_u32(last.codepoint).unwrap().len_utf16() as u32,
                    direction: call.input.direction,
                    script: call.input.script,
                    font_slot: 0,
                    font_fakery: 0,
                    glyphs,
                }
            })
            .collect();
        let layout = paint_layout(
            PaintLayoutPaint {
                size: f32::from_bits(case.paint.size_bits),
                scale_x: f32::from_bits(case.paint.scale_x_bits),
                skew_x: f32::from_bits(case.paint.skew_x_bits),
                letter_spacing: f32::from_bits(case.paint.letter_spacing_bits),
                word_spacing: f32::from_bits(case.paint.word_spacing_bits),
            },
            &chunks,
        )
        .unwrap();
        compare_layout(&layout, &case.layout_piece, &case.name);
        compare_entry_geometry(&layout, case, case.entry_geometry.as_ref().unwrap());
        glyph_count += layout.glyphs().len();
    }
    assert_eq!(glyph_count, 25);
}

#[test]
fn entry_capture_distinguishes_advance_division_and_owner_ink_translation() {
    let capture = entry_capture();
    let mut multiplication_mismatches = 0;
    let mut missing_translation_mismatches = 0;
    let mut repeated_translation_mismatches = 0;
    for case in &capture.cases {
        let expected = case.entry_geometry.as_ref().unwrap();
        for (relative_owner, &advance) in
            case.layout_piece.character_advances_bits.iter().enumerate()
        {
            let absolute_owner = case.range_utf16[0] as usize + relative_owner;
            let multiplied = f32::from_bits(advance) * 0.01_f32;
            multiplication_mismatches +=
                usize::from(multiplied.to_bits() != expected.entry_widths_bits[absolute_owner]);
        }
        for ((&ink, &position), glyph) in case
            .layout_piece
            .ink_bounds_bits
            .iter()
            .zip(&case.layout_piece.owner_positions_bits)
            .zip(&expected.glyphs)
        {
            let ink = ink.map(f32::from_bits);
            let [x, y] = position.map(f32::from_bits);
            let no_translation = ink.map(|coordinate| coordinate * 0.01_f32);
            let repeated = [
                (ink[0] + x) + x,
                (ink[1] + y) + y,
                (ink[2] + x) + x,
                (ink[3] + y) + y,
            ]
            .map(|coordinate| coordinate * 0.01_f32);
            missing_translation_mismatches +=
                usize::from(no_translation.map(f32::to_bits) != glyph.entry_ink_bits);
            repeated_translation_mismatches +=
                usize::from(repeated.map(f32::to_bits) != glyph.entry_ink_bits);
        }
    }
    assert_eq!(
        (
            multiplication_mismatches,
            missing_translation_mismatches,
            repeated_translation_mismatches,
        ),
        (4, 8, 8),
    );
}

#[test]
fn provider_derived_fractional_hinting_matches_native_raw_scaler_controls() {
    compare_capture(
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../conformance/table-text-shaping-entry-skia-metrics.json"
        )),
        "c7216fb3148f524a8e7cde5c0bb83a616c60ae81c3cedb7c7f2d266f1bd3800a",
        [11, 26, 26],
    );
}
