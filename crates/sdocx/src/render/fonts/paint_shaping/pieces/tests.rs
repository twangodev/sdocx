use super::*;
use crate::render::fonts::{FontBook, PaintMetricError, PaintMetricMode};

#[test]
fn measured_piece_factory_matches_all_native_requests_positions_and_geometry() {
    use super::super::tests::compare_factory_capture;

    macro_rules! fixture {
        ($file:literal, $hash:literal, $counts:expr) => {
            compare_factory_capture(
                include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../conformance/",
                    $file
                )),
                $hash,
                $counts,
            );
        };
    }
    fixture!(
        "table-text-shaping.json",
        "a4c58481cb1e36bb7de8783bea49a800b10b4227177684c48f5b5c356d7a425b",
        [16, 48, 48]
    );
    fixture!(
        "table-text-shaping-numeric.json",
        "1e476f7fc8254b2a6c7316c6ff18b9c436f8ab57707d536daa07e11eca2af455",
        [19, 182, 182]
    );
    fixture!(
        "table-text-shaping-gpos.json",
        "9280953ac5de0b6b745baedcda73cf4d88a95a7b52cbfbc99188e0b66a71efb2",
        [4, 12, 12]
    );
    fixture!(
        "table-text-shaping-skia-metrics.json",
        "c8aa3d08839e9f7e7ddff6d9200616faa11adb67f8a7124f7fd91d5088c9eec0",
        [18, 108, 108]
    );
    fixture!(
        "table-text-shaping-mixed-scripts.json",
        "00d1634d29634fac146b42baa1e1a349bd66a2a74724ca7e7d5c539f116703ad",
        [380, 800, 800]
    );
    fixture!(
        "table-text-entry-geometry.json",
        "4615a8778c0a7b6301bd9efddbd591da2fd00278d4a3f0db06b04e4b1c923dc4",
        [8, 25, 25]
    );
    fixture!(
        "table-text-shaping-entry-skia-metrics.json",
        "c7216fb3148f524a8e7cde5c0bb83a616c60ae81c3cedb7c7f2d266f1bd3800a",
        [11, 26, 26]
    );
    fixture!(
        "table-text-shaping-itemization.json",
        "eddd4290eb75ab124b82c07814d4354debe5af8a167cb0535a83541238dff710",
        [102, 252, 252]
    );
}

#[test]
fn feature_profile_preserves_native_script_and_spacing_thresholds() {
    for script in [*b"Latn", *b"Grek", *b"Cyrl", *b"Zyyy"] {
        for sign in [-1.0, 1.0] {
            let below = 0.03_f32 * sign;
            let above = f32::from_bits(0.03_f32.to_bits() + 1) * sign;
            assert_eq!(
                PaintFeatureProfile::for_script(script, below).unwrap(),
                if script == *b"Latn" {
                    PaintFeatureProfile::DisableLigatures
                } else {
                    PaintFeatureProfile::Default
                }
            );
            assert_eq!(
                PaintFeatureProfile::for_script(script, above).unwrap(),
                PaintFeatureProfile::DisableLigatures
            );
        }
    }
    assert!(matches!(
        PaintFeatureProfile::for_script(*b"Latn", f32::NAN),
        Err(PaintPieceError::Layout(PaintLayoutError::NonFinitePaint))
    ));
}

fn paint() -> PaintMetricInput {
    PaintMetricInput {
        size: 1700.0,
        scale_x: 1.0,
        skew_x: 0.0,
        mode: PaintMetricMode::Normal,
    }
}

#[test]
fn factory_rejects_invalid_spacing_features_and_excessive_chunk_work_before_shaping() {
    let face = FontBook::default().resolve("Roboto", false, false).unwrap();
    let mut shaper = face.paint_shaper(paint()).unwrap();
    let itemization = PaintItemization::new("AV", 0..2).unwrap();
    let mut request = PaintTextRequest::new(&itemization, PaintShapeDirection::LeftToRight);
    request.word_spacing = 1.0;
    assert!(matches!(
        shaper.shape_text(request),
        Err(PaintPieceError::Layout(PaintLayoutError::UnsupportedPaint))
    ));
    request.word_spacing = 0.0;
    request.letter_spacing = f32::INFINITY;
    assert!(matches!(
        shaper.shape_text(request),
        Err(PaintPieceError::Layout(PaintLayoutError::NonFinitePaint))
    ));
    request.letter_spacing = 0.0;
    let features = [PaintShapeFeature {
        tag: *b"liga",
        value: 1,
        start: 0,
        end: u32::MAX,
    }; MAX_FEATURES - 1];
    request.features = &features;
    assert!(matches!(
        shaper.shape_text(request),
        Err(PaintPieceError::InputBudget)
    ));
    let invalid = [PaintShapeFeature {
        tag: *b"liga",
        value: 1,
        start: 2,
        end: 1,
    }];
    request.features = &invalid;
    assert!(matches!(
        shaper.shape_text(request),
        Err(PaintPieceError::Shape(PaintShapeError::InvalidProperties))
    ));
    let source = "AΑ".repeat(MAX_PIECE_CHUNKS / 2 + 1);
    let excessive =
        PaintItemization::new(&source, 0..source.encode_utf16().count() as u32).unwrap();
    assert!(matches!(
        shaper.shape_text(PaintTextRequest::new(
            &excessive,
            PaintShapeDirection::LeftToRight
        )),
        Err(PaintPieceError::InputBudget)
    ));
    assert!(shaper.cache.is_empty());
    assert!(shaper.last_source.is_none());
    let empty = PaintItemization::new("AV", 1..1).unwrap();
    assert!(matches!(
        shaper.shape_text(PaintTextRequest::new(
            &empty,
            PaintShapeDirection::LeftToRight
        )),
        Err(PaintPieceError::Layout(PaintLayoutError::InvalidRange))
    ));
    assert_eq!(
        shaper
            .shape_text(PaintTextRequest::new(
                &itemization,
                PaintShapeDirection::LeftToRight
            ))
            .unwrap()
            .shaped_runs()
            .len(),
        1
    );
}

#[test]
fn factory_preserves_explicit_unsupported_native_metrics_and_shaper_reuse() {
    let face = FontBook::default().resolve("Roboto", false, false).unwrap();
    let mut shaper = face
        .paint_shaper(PaintMetricInput {
            skew_x: -0.25,
            ..paint()
        })
        .unwrap();
    let composite = PaintItemization::new("Α", 0..1).unwrap();
    assert!(matches!(
        shaper.shape_text(PaintTextRequest::new(
            &composite,
            PaintShapeDirection::LeftToRight
        )),
        Err(PaintPieceError::Shape(PaintShapeError::Metrics(
            PaintMetricError::UnsupportedComposite { .. }
        )))
    ));
    let simple = PaintItemization::new("AV", 0..2).unwrap();
    let piece = shaper
        .shape_text(PaintTextRequest::new(
            &simple,
            PaintShapeDirection::LeftToRight,
        ))
        .unwrap();
    assert_eq!(piece.entry_geometry().entries().len(), 2);
    assert_eq!(piece.shaped_runs()[0].paint(), piece.paint());
}

#[test]
fn factory_keeps_unverified_scripts_as_typed_failures() {
    let face = FontBook::default().resolve("Roboto", false, false).unwrap();
    let mut shaper = face.paint_shaper(paint()).unwrap();
    let itemization = PaintItemization::new("ع", 0..1).unwrap();
    assert!(matches!(
        shaper.shape_text(PaintTextRequest::new(
            &itemization,
            PaintShapeDirection::RightToLeft
        )),
        Err(PaintPieceError::Shape(
            PaintShapeError::UnsupportedPositioningDomain
        ))
    ));
    assert!(shaper.cache.is_empty());
}
