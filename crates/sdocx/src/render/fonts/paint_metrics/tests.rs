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
        if input.scale_x != 1.0 || input.skew_x != 0.0 {
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
        [15, 44, 1],
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
        [8, 146, 9],
    );
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
    for (scale_x, skew_x) in [(2.0, 0.0), (f32::NAN, 0.0), (1.0, -0.25), (1.0, f32::NAN)] {
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
