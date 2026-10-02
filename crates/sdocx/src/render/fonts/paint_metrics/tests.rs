use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::*;
use crate::fonts::{FontBook, UnicodeBuffer};

#[derive(Deserialize)]
struct Capture {
    font_sha256: String,
    font_face_index: u32,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    paint: Paint,
    callbacks: Callbacks,
    skia_metrics: Option<SkiaCapture>,
}

#[derive(Deserialize)]
struct Paint {
    size_bits: u32,
    scale_x_bits: u32,
    skew_x_bits: u32,
    packed_flags: u32,
}

#[derive(Deserialize)]
struct Callbacks {
    vector_glyphs: Vec<Vec<u32>>,
    vector_raw_bits: Vec<u32>,
    raw_skia_bounds_bits: Vec<[u32; 4]>,
}

fn input(size: f32, mode: PaintMetricMode) -> PaintMetricInput {
    PaintMetricInput {
        size,
        scale_x: 1.0,
        skew_x: 0.0,
        mode,
    }
}

fn captured_input(paint: &Paint) -> PaintMetricInput {
    let mode = match paint.packed_flags {
        0 => PaintMetricMode::Unhinted,
        0x20000 => PaintMetricMode::Normal,
        0x20040 => PaintMetricMode::Linear,
        other => panic!("uncaptured paint profile {other:#x}"),
    };
    PaintMetricInput {
        size: f32::from_bits(paint.size_bits),
        scale_x: f32::from_bits(paint.scale_x_bits),
        skew_x: f32::from_bits(paint.skew_x_bits),
        mode,
    }
}

fn ink_bits(ink: PaintInkBounds) -> [u32; 4] {
    [ink.left, ink.top, ink.right, ink.bottom].map(f32::to_bits)
}

fn compare_capture(bytes: &[u8], hash: &str, expected: [usize; 3]) {
    assert_eq!(format!("{:x}", Sha256::digest(bytes)), hash);
    let capture: Capture = serde_json::from_slice(bytes).unwrap();
    let face = FontBook::default().resolve("Roboto", false, false).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(face.bytes())),
        capture.font_sha256
    );
    assert_eq!(face.index, capture.font_face_index);
    let mut supported_cases = 0;
    let mut supported_glyphs = 0;
    let mut rejected_cases = 0;
    for case in capture.cases {
        let input = captured_input(&case.paint);
        if input.scale_x != 1.0 || !input.skew_x.is_finite() {
            assert!(matches!(
                face.paint_metrics(input),
                Err(PaintMetricError::UnsupportedTransform)
            ));
            rejected_cases += 1;
            continue;
        }
        let metrics = face.paint_metrics(input).unwrap();
        let callbacks = case.callbacks;
        let glyphs = callbacks
            .vector_glyphs
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        assert_eq!(glyphs.len(), callbacks.vector_raw_bits.len());
        assert_eq!(glyphs.len(), callbacks.raw_skia_bounds_bits.len());
        for ((glyph_id, advance), ink) in glyphs
            .into_iter()
            .zip(callbacks.vector_raw_bits)
            .zip(callbacks.raw_skia_bounds_bits)
        {
            let glyph = metrics.glyph(glyph_id).unwrap();
            assert_eq!(
                glyph.advance.to_bits(),
                advance,
                "{} glyph {glyph_id} advance",
                case.name
            );
            assert_eq!(
                ink_bits(glyph.ink),
                ink,
                "{} glyph {glyph_id} ink",
                case.name
            );
            supported_glyphs += 1;
        }
        supported_cases += 1;
    }
    assert_eq!(
        [supported_cases, supported_glyphs, rejected_cases],
        expected
    );
}

#[test]
fn native_paint_metric_profiles_match_all_captured_supported_advances_and_ink() {
    compare_capture(
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../conformance/table-text-shaping.json"
        )),
        "a4c58481cb1e36bb7de8783bea49a800b10b4227177684c48f5b5c356d7a425b",
        [16, 48, 0],
    );
}

#[test]
fn native_large_and_mixed_script_paint_metrics_match_captured_advances_and_ink() {
    compare_capture(
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../conformance/table-text-shaping-numeric.json"
        )),
        "1e476f7fc8254b2a6c7316c6ff18b9c436f8ab57707d536daa07e11eca2af455",
        [17, 182, 0],
    );
}

#[test]
fn native_entry_paints_preserve_fractional_hinted_ink() {
    compare_capture(
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../conformance/table-text-entry-geometry.json"
        )),
        "4615a8778c0a7b6301bd9efddbd591da2fd00278d4a3f0db06b04e4b1c923dc4",
        [8, 25, 0],
    );
    let face = FontBook::default().resolve("Roboto", false, false).unwrap();
    let metrics = face
        .paint_metrics(input(1712.5, PaintMetricMode::Normal))
        .unwrap();
    assert_eq!(metrics.glyph(84).unwrap().ink.right, 901.0);

    let font = FontRef::from_index(face.bytes(), face.index).unwrap();
    let outlines = font.outline_glyphs();
    let hinting = HintingInstance::new(
        &outlines,
        Size::new(1712.5),
        LocationRef::default(),
        HintingOptions {
            engine: Engine::AutoFallback,
            target: skrifa::outline::SmoothMode::Normal.into(),
        },
    )
    .unwrap();
    let mut smooth = ControlBoundsPen::new();
    outlines
        .get(GlyphId::new(84))
        .unwrap()
        .draw(DrawSettings::hinted(&hinting, false), &mut smooth)
        .unwrap();
    assert_eq!(smooth.bounding_box().unwrap().x_max.ceil(), 900.0);
}

#[test]
fn hinted_and_linear_advances_preserve_distinct_fixed_metric_domains() {
    let face = FontBook::default().resolve("Roboto", false, false).unwrap();
    let hinted = face
        .paint_metrics(input(1700.0, PaintMetricMode::Normal))
        .unwrap();
    let unhinted = face
        .paint_metrics(input(1700.0, PaintMetricMode::Unhinted))
        .unwrap();
    assert_eq!(hinted.glyph(59).unwrap().advance, 1082.0);
    assert_eq!(
        f64::from(unhinted.glyph(59).unwrap().advance),
        1081.591796875
    );
    let font = FontRef::from_index(face.bytes(), face.index).unwrap();
    let outline = font.outline_glyphs().get(GlyphId::new(59)).unwrap();
    let outline_advance = outline
        .draw(
            DrawSettings::unhinted(Size::new(1700.0), LocationRef::default()),
            &mut ControlBoundsPen::new(),
        )
        .unwrap()
        .advance_width
        .unwrap();
    assert_eq!(f64::from(outline_advance), 1081.59375);
    assert_ne!(outline_advance, unhinted.glyph(59).unwrap().advance);
    let raw = rustybuzz::Face::from_slice(face.bytes(), face.index).unwrap();
    let scaled_hmtx = f32::from(
        raw.glyph_hor_advance(rustybuzz::ttf_parser::GlyphId(59))
            .unwrap(),
    ) * 1700.0
        / f32::from(raw.as_ref().units_per_em());
    assert_ne!(scaled_hmtx, hinted.glyph(59).unwrap().advance);
}

#[test]
fn normalization_changes_backend_ink_before_rescaling() {
    let face = FontBook::default().resolve("Roboto", false, false).unwrap();
    let at = face
        .paint_metrics(input(2048.0, PaintMetricMode::Normal))
        .unwrap()
        .glyph(38)
        .unwrap();
    let above = face
        .paint_metrics(input(2049.0, PaintMetricMode::Normal))
        .unwrap()
        .glyph(38)
        .unwrap();
    assert_eq!(at.ink.left, 28.0);
    assert_eq!(above.ink.left, 0.0);
    assert_eq!(f64::from(above.ink.top), -1472.71875);
    let linear = face
        .paint_metrics(input(1700.0, PaintMetricMode::Linear))
        .unwrap()
        .glyph(38)
        .unwrap();
    let unhinted = face
        .paint_metrics(input(1700.0, PaintMetricMode::Unhinted))
        .unwrap()
        .glyph(38)
        .unwrap();
    assert_eq!(linear.advance, unhinted.advance);
    assert_eq!(linear.ink.left, 0.0);
    assert_eq!(unhinted.ink.left, 23.0);
}

#[test]
fn inkless_space_and_zero_advance_marks_remain_distinct() {
    let face = FontBook::default().resolve("Roboto", false, false).unwrap();
    let metrics = face
        .paint_metrics(input(1700.0, PaintMetricMode::Normal))
        .unwrap();
    let mut buffer = UnicodeBuffer::new();
    buffer.push_str(" ");
    let space = metrics
        .glyph(face.shape(buffer, &[]).unwrap().glyphs[0].id)
        .unwrap();
    assert!(space.advance > 0.0);
    assert_eq!(ink_bits(space.ink), [0; 4]);
    let mark = metrics.glyph(434).unwrap();
    assert_eq!(mark.advance, 0.0);
    assert!(mark.ink.right > mark.ink.left);
}

#[test]
fn paint_metric_input_and_glyph_domains_fail_explicitly() {
    let face = FontBook::default().resolve("Roboto", false, false).unwrap();
    for size in [
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        -1.0,
        0.0,
        0.5,
        8_388_608.0,
    ] {
        assert!(matches!(
            face.paint_metrics(input(size, PaintMetricMode::Normal)),
            Err(PaintMetricError::InvalidSize)
        ));
    }
    for (scale_x, skew_x) in [
        (2.0, 0.0),
        (f32::NAN, 0.0),
        (1.0, f32::INFINITY),
        (1.0, f32::NAN),
    ] {
        assert!(matches!(
            face.paint_metrics(PaintMetricInput {
                scale_x,
                skew_x,
                ..input(1700.0, PaintMetricMode::Normal)
            }),
            Err(PaintMetricError::UnsupportedTransform)
        ));
    }
    let metrics = face
        .paint_metrics(input(1700.0, PaintMetricMode::Normal))
        .unwrap();
    assert!(matches!(
        metrics.glyph(u32::MAX),
        Err(PaintMetricError::InvalidGlyph { glyph_id: u32::MAX })
    ));
    assert!(matches!(
        PaintMetrics::new(&[], 0, input(1700.0, PaintMetricMode::Normal)),
        Err(PaintMetricError::InvalidFont)
    ));
    assert!(matches!(
        PaintMetrics::new(face.bytes(), 1, input(1700.0, PaintMetricMode::Normal)),
        Err(PaintMetricError::InvalidFont)
    ));
}

#[test]
fn table_presence_gates_variable_bitmap_and_varc_fonts() {
    let source = FontBook::default().resolve("Roboto", false, false).unwrap();
    for tag in [b"fvar", b"CBDT", b"VARC"] {
        let mut bytes = source.bytes().to_vec();
        let count = usize::from(u16::from_be_bytes(bytes[4..6].try_into().unwrap()));
        let (records, _) = bytes[12..12 + count * 16].as_chunks_mut::<16>();
        let record = records
            .iter_mut()
            .find(|record| &record[..4] == b"gasp")
            .unwrap();
        record[..4].copy_from_slice(tag);
        records.sort_unstable_by_key(|record| <[u8; 4]>::try_from(&record[..4]).unwrap());
        assert!(matches!(
            PaintMetrics::new(&bytes, 0, input(1700.0, PaintMetricMode::Normal)),
            Err(PaintMetricError::UnsupportedFont)
        ));
    }
}

#[test]
fn backend_ink_coordinates_have_explicit_integer_bounds() {
    let face = FontBook::default().resolve("Roboto", false, false).unwrap();
    let metrics = face
        .paint_metrics(input(1700.0, PaintMetricMode::Normal))
        .unwrap();
    for coordinates in [[-32769.0, 0.0], [32768.0, 0.0], [0.0, f32::INFINITY]] {
        use skrifa::outline::pen::OutlinePen;
        let mut pen = ControlBoundsPen::new();
        pen.move_to(coordinates[0], coordinates[1]);
        assert!(matches!(
            metrics.ink_bounds(&pen),
            Err(PaintMetricError::MetricRange)
        ));
    }
}

const SKIA_CAPTURE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../conformance/table-text-shaping-skia-metrics.json"
));

#[derive(Deserialize)]
struct SkiaCapture {
    configs: Vec<ScalerConfig>,
    advances: Vec<CachedAdvance>,
    loads: Vec<GlyphLoad>,
}

#[derive(Deserialize)]
struct ScalerConfig {
    backend_size_bits: [u32; 2],
    residual_bits: [u32; 4],
    matrix: [i64; 4],
    record_flags: u16,
    mask_format: u8,
    load_flags: u32,
    linear: bool,
    ppem: [u16; 2],
    scale: [i64; 2],
}

#[derive(Deserialize)]
struct CachedAdvance {
    config: usize,
    glyph: u32,
    flags: u32,
    source_fixed: i32,
    before: Option<[i32; 2]>,
    after: [i32; 2],
}

#[derive(Deserialize)]
struct GlyphLoad {
    config: usize,
    glyph: u32,
    flags: u32,
    before_outline: CapturedOutline,
    after_outline: CapturedOutline,
    linear_fixed: i64,
    slot_advance: [i64; 2],
    before: Option<[i32; 2]>,
    after: [i32; 2],
    linear_store: bool,
}

#[derive(Deserialize)]
struct CapturedOutline {
    points: Vec<[i64; 2]>,
    tags: Vec<u8>,
    contours: Vec<i16>,
}

#[test]
fn native_gpos_profiles_preserve_raw_metrics_independently_of_positioning() {
    compare_capture(
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../conformance/table-text-shaping-gpos.json"
        )),
        "9280953ac5de0b6b745baedcda73cf4d88a95a7b52cbfbc99188e0b66a71efb2",
        [4, 12, 0],
    );
}

#[test]
fn native_skia_matrix_profiles_match_all_captured_advances_and_ink() {
    compare_capture(
        SKIA_CAPTURE,
        "c8aa3d08839e9f7e7ddff6d9200616faa11adb67f8a7124f7fd91d5088c9eec0",
        [18, 108, 0],
    );
}

#[test]
fn native_skia_original_points_and_fixed_advance_transitions_match_exactly() {
    let capture: Capture = serde_json::from_slice(SKIA_CAPTURE).unwrap();
    let face = FontBook::default().resolve("Roboto", false, false).unwrap();
    let mut point_count = 0;
    let mut advance_count = 0;
    let mut load_count = 0;
    for case in capture.cases {
        let input = captured_input(&case.paint);
        let metrics = face.paint_metrics(input).unwrap();
        let trace = case.skia_metrics.unwrap();
        for config in &trace.configs {
            assert_eq!(
                config.backend_size_bits,
                [metrics.backend_size.to_bits(); 2],
                "{} backend",
                case.name
            );
            let reciprocal = 1.0 / metrics.backend_size;
            let (diagonal, skew) = if input.skew_x == 0.0 {
                (1.0, 0.0)
            } else {
                (
                    metrics.backend_size * reciprocal,
                    (metrics.backend_size * input.skew_x) * reciprocal,
                )
            };
            assert_eq!(
                config.residual_bits,
                [diagonal.to_bits(), skew.to_bits(), 0, diagonal.to_bits()],
                "{} residual",
                case.name
            );
            assert_eq!(
                config.matrix,
                [
                    i64::from(metrics.transform.xx),
                    i64::from(metrics.transform.xy),
                    0,
                    i64::from(metrics.transform.xx)
                ],
                "{} matrix",
                case.name
            );
            assert_eq!(config.scale, [metrics.linear_scale; 2]);
            assert_eq!(config.ppem, [metrics.backend_size.round() as u16; 2]);
            assert_eq!(config.linear, metrics.hinting.is_none());
            assert_eq!(config.mask_format, 0);
            assert_eq!(
                config.load_flags,
                if config.linear { 0x10020a } else { 0x120208 }
            );
            assert_eq!(
                config.record_flags,
                if metrics.hinting.is_some() {
                    256
                } else if metrics.normalization != 1.0 {
                    16
                } else {
                    0
                }
            );
        }
        for advance in &trace.advances {
            let config = &trace.configs[advance.config];
            let units = metrics
                .design_metrics
                .advance_width(GlyphId::new(advance.glyph))
                .unwrap() as i64;
            let fixed = (units * metrics.linear_scale + 32) / 64;
            assert_eq!(i64::from(advance.source_fixed), fixed);
            assert_eq!(advance.flags, config.load_flags | 0x20000000);
            assert_eq!(advance.before, None);
            assert_eq!(
                advance.after,
                [((fixed * config.matrix[0]) >> 16) as i32, 0]
            );
            advance_count += 1;
        }
        for load in trace.loads {
            let config = &trace.configs[load.config];
            assert_eq!(load.flags, config.load_flags);
            assert_eq!(load.linear_store, config.linear);
            assert_eq!(load.before_outline.tags, load.after_outline.tags);
            assert_eq!(load.before_outline.contours, load.after_outline.contours);
            let expected_cache = if config.linear {
                [((load.linear_fixed * config.matrix[0]) >> 16) as i32, 0]
            } else {
                load.slot_advance.map(|advance| (advance << 10) as i32)
            };
            assert_eq!(load.after, expected_cache);
            if let Some(before) = load.before {
                assert_eq!(before, expected_cache);
            }
            if !config.linear {
                continue;
            }
            let glyph_id = GlyphId::new(load.glyph);
            let units = metrics.design_metrics.advance_width(glyph_id).unwrap() as i64;
            assert_eq!(load.linear_fixed, (units * metrics.linear_scale + 32) / 64);
            let points = metrics.simple_points(glyph_id).unwrap();
            assert_eq!(
                points, load.before_outline.points,
                "{} glyph {} original",
                case.name, load.glyph
            );
            let transformed = points
                .iter()
                .map(|[x, y]| metrics.transform.point(*x, *y).unwrap())
                .collect::<Vec<_>>();
            assert_eq!(
                transformed, load.after_outline.points,
                "{} glyph {} transformed",
                case.name, load.glyph
            );
            point_count += points.len();
            load_count += 1;
        }
    }
    assert_eq!([advance_count, load_count, point_count], [102, 102, 1105]);
}

#[test]
fn tiny_skew_disables_hinting_even_when_fixed_off_diagonal_is_zero() {
    let face = FontBook::default().resolve("Roboto", false, false).unwrap();
    let metrics = face
        .paint_metrics(PaintMetricInput {
            skew_x: -1e-7,
            ..input(1700.0, PaintMetricMode::Normal)
        })
        .unwrap();
    assert!(metrics.hinting.is_none());
    assert_eq!(metrics.transform.xy, 0);
    assert_eq!(metrics.transform.xx, 65535);
    assert_ne!(metrics.glyph(59).unwrap().advance, 1082.0);
}

#[test]
fn skewed_composites_fail_explicitly_and_unskewed_composites_keep_scaling() {
    let face = FontBook::default().resolve("Roboto", false, false).unwrap();
    let font = FontRef::from_index(face.bytes(), face.index).unwrap();
    let loca = font.loca(None).unwrap();
    let glyf = font.glyf().unwrap();
    let glyph_id = (0..font
        .glyph_metrics(Size::unscaled(), LocationRef::default())
        .glyph_count())
        .find(|glyph_id| {
            matches!(
                loca.get_glyf(GlyphId::new(*glyph_id), &glyf),
                Ok(Some(Glyph::Composite(_)))
            )
        })
        .unwrap();
    let metrics = face
        .paint_metrics(PaintMetricInput {
            skew_x: -0.25,
            ..input(1700.0, PaintMetricMode::Normal)
        })
        .unwrap();
    assert!(
        matches!(metrics.glyph(glyph_id), Err(PaintMetricError::UnsupportedComposite { glyph_id: rejected }) if rejected == glyph_id)
    );
    assert!(
        face.paint_metrics(input(1700.0, PaintMetricMode::Normal))
            .unwrap()
            .glyph(glyph_id)
            .is_ok()
    );
    assert!(matches!(
        face.paint_metrics(PaintMetricInput {
            skew_x: f32::MAX,
            ..input(1700.0, PaintMetricMode::Normal)
        }),
        Err(PaintMetricError::MetricRange)
    ));
}

#[test]
fn fractional_raw_fixed_advance_preserves_bits_lost_by_f32() {
    let capture: Capture = serde_json::from_slice(SKIA_CAPTURE).unwrap();
    let case = capture
        .cases
        .into_iter()
        .find(|case| case.name == "fractional_size")
        .unwrap();
    let advance = case
        .skia_metrics
        .unwrap()
        .advances
        .into_iter()
        .find(|advance| advance.glyph == 59)
        .unwrap();
    assert_eq!(advance.source_fixed, 71_397_885);
    assert_eq!(advance.source_fixed as f32 as i32, 71_397_888);
}

#[test]
fn fixed_outline_products_round_separately_before_addition() {
    let transform = FixedTransform {
        xx: 65536,
        xy: 32768,
    };
    assert_eq!(transform.point(0, -1).unwrap(), [-1, -1]);
    assert_eq!(transform.point(1, 1).unwrap(), [2, 1]);
    assert_eq!(mul_fix(-1, 32768), -1);
    assert_eq!(mul_fix(1, 32768), 1);
    let transform = FixedTransform {
        xx: 32768,
        xy: 32768,
    };
    assert_eq!(transform.point(1, 1).unwrap(), [2, 1]);
    assert_ne!(transform.point(1, 1).unwrap()[0], mul_fix(2, 32768));
}
