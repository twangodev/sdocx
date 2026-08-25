use std::path::PathBuf;

fn sample_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("samples")
        .join(name)
}

#[test]
fn sample_stroke_channels_align_with_points() {
    for name in ["cs61bl_su22.sdocx", "handwritten.sdocx", "quiz.sdocx"] {
        let document = sdocx::parse(sample_path(name)).unwrap();
        let mut stroke_count = 0;

        for stroke in document.pages.iter().flat_map(|page| &page.strokes) {
            stroke_count += 1;
            assert_eq!(stroke.pressures.len(), stroke.points.len(), "{name}");
            assert_eq!(stroke.timestamps.len(), stroke.points.len(), "{name}");
            assert!(stroke.pressures.iter().all(|value| value.is_finite()));

            if !stroke.tilts.is_empty() || !stroke.orientations.is_empty() {
                assert_eq!(stroke.tilts.len(), stroke.points.len(), "{name}");
                assert_eq!(stroke.orientations.len(), stroke.points.len(), "{name}");
                assert!(stroke.tilts.iter().all(|value| value.is_finite()));
                assert!(stroke.orientations.iter().all(|value| value.is_finite()));
            }
        }

        assert!(stroke_count > 0, "{name} should contain strokes");
    }
}
