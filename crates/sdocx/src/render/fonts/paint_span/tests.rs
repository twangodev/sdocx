use super::*;
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
struct Capture {
    cases: Vec<Case>,
}
#[derive(Deserialize)]
struct Case {
    entry_geometry: Entry,
}
#[derive(Deserialize)]
struct Entry {
    paint_profiles: Vec<Profile>,
}
#[derive(Deserialize)]
struct Profile {
    source_style: u8,
    source_size_bits: u32,
    paint_size_bits: u32,
    scale_x_bits: u32,
    skew_x_bits: u32,
    packed_minikin_flags: u32,
    underline: bool,
    fake_bold: bool,
}

#[derive(Deserialize)]
struct FullHelperCapture {
    cases: Vec<FullHelperProfile>,
}
#[derive(Deserialize)]
struct FullHelperProfile {
    source_size_bits: u32,
    source_flags: u8,
    paint_size_bits: u32,
    scale_x_bits: u32,
    skew_x_bits: u32,
    packed_minikin_flags: u32,
    typeface_style: u32,
    minikin_weight: u16,
    minikin_italic: bool,
}

#[test]
fn full_native_span_helper_preserves_size_thresholds_and_source_style_bits() {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/table-text-span-paint.json"
    ));
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "1d0479915ad0d602da2245b5a3767c7cd308229e0232b9fc544949989c790fdb"
    );
    let capture: FullHelperCapture = serde_json::from_slice(bytes).unwrap();
    assert_eq!(capture.cases.len(), 93);
    let mut low_metric_sizes = 0;
    for profile in capture.cases {
        let actual = PaintSpanProfile::new(
            f32::from_bits(profile.source_size_bits),
            profile.source_flags,
        )
        .unwrap();
        assert_eq!(actual.size().to_bits(), profile.paint_size_bits);
        assert_eq!(actual.scale_x().to_bits(), profile.scale_x_bits);
        assert_eq!(actual.skew_x().to_bits(), profile.skew_x_bits);
        assert_eq!(actual.packed_flags(), profile.packed_minikin_flags);
        assert_eq!(
            profile.typeface_style & 0xffff,
            u32::from(profile.minikin_weight)
        );
        assert_eq!(
            profile.typeface_style & 0x10000 != 0,
            profile.minikin_italic
        );
        if actual.fake_bold() {
            assert_eq!(
                actual.metric_input(),
                Err(PaintSpanError::UnsupportedFakeBold)
            );
        } else if actual.size() < 1.0 {
            assert_eq!(
                actual.metric_input(),
                Err(PaintSpanError::UnsupportedPaintSize)
            );
            low_metric_sizes += 1;
        } else {
            let input = actual.metric_input().unwrap();
            assert_eq!(input.size.to_bits(), profile.paint_size_bits);
            assert_eq!(input.scale_x.to_bits(), profile.scale_x_bits);
            assert_eq!(input.skew_x.to_bits(), profile.skew_x_bits);
        }
    }
    assert_eq!(low_metric_sizes, 12);
}

#[test]
fn source_span_profiles_match_executed_native_setters() {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/table-text-entry-geometry.json"
    ));
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "4615a8778c0a7b6301bd9efddbd591da2fd00278d4a3f0db06b04e4b1c923dc4"
    );
    let capture: Capture = serde_json::from_slice(bytes).unwrap();
    let mut count = 0;
    for profile in capture
        .cases
        .iter()
        .flat_map(|case| &case.entry_geometry.paint_profiles)
    {
        let actual = PaintSpanProfile::new(
            f32::from_bits(profile.source_size_bits),
            profile.source_style,
        )
        .unwrap();
        assert_eq!(actual.size().to_bits(), profile.paint_size_bits);
        assert_eq!(actual.scale_x().to_bits(), profile.scale_x_bits);
        assert_eq!(actual.skew_x().to_bits(), profile.skew_x_bits);
        assert_eq!(actual.packed_flags(), profile.packed_minikin_flags);
        assert_eq!(actual.underline(), profile.underline);
        assert_eq!(actual.fake_bold(), profile.fake_bold);
        if profile.fake_bold {
            assert_eq!(
                actual.metric_input(),
                Err(PaintSpanError::UnsupportedFakeBold)
            );
        } else {
            let input = actual.metric_input().unwrap();
            assert_eq!(input.size.to_bits(), profile.paint_size_bits);
            assert_eq!(input.scale_x.to_bits(), profile.scale_x_bits);
            assert_eq!(input.skew_x.to_bits(), profile.skew_x_bits);
            assert_eq!(input.mode, PaintMetricMode::Normal);
        }
        count += 1;
    }
    assert_eq!(count, 64);
}

#[test]
fn source_size_rounding_precedes_native_metric_validation() {
    for source_size in [0.0, -1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(
            PaintSpanProfile::new(source_size, 0),
            Err(PaintSpanError::InvalidSourceSize)
        );
    }
    assert_eq!(
        PaintSpanProfile::new(f32::MAX, 0),
        Err(PaintSpanError::NonFinitePaintSize)
    );
    for source_size in [0.000001, 0.005, 100_000.0] {
        let actual = PaintSpanProfile::new(source_size, 0).unwrap();
        assert_eq!(
            actual.metric_input(),
            Err(PaintSpanError::UnsupportedPaintSize)
        );
    }
    assert_eq!(PaintSpanProfile::new(0.01, 0).unwrap().size(), 1.0);
}
