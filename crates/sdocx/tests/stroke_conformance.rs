use std::{fs, path::PathBuf};

use sha2::{Digest, Sha256};

#[test]
#[ignore = "requires the original handwritten compatibility fixtures; see conformance/README.md"]
fn handwritten_documents_match_the_native_frame_audit() {
    let root = std::env::var_os("SDOCX_STROKE_CORPUS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../hf"));
    let manifest = include_str!("../../../conformance/strokes.tsv");
    for row in manifest
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
    {
        let columns = row.split('\t').collect::<Vec<_>>();
        assert_eq!(columns.len(), 5);
        let name = columns[0];
        let bytes = fs::read(root.join(name)).unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(
            format!("{:x}", Sha256::digest(&bytes)),
            columns[1],
            "{name}: fixture digest"
        );
        let parsed =
            sdocx::parse_bytes_detailed(&bytes).unwrap_or_else(|error| panic!("{name}: {error}"));
        assert!(
            parsed.report.diagnostics.is_empty(),
            "{name}: {:?}",
            parsed.report.diagnostics
        );
        assert_eq!(parsed.document.pages.len(), 1, "{name}: stored pages");
        let strokes = &parsed.document.pages[0].strokes;
        assert_eq!(
            strokes.len(),
            columns[3].parse::<usize>().unwrap(),
            "{name}: stroke count"
        );
        assert_eq!(
            strokes
                .iter()
                .map(|stroke| stroke.points.len())
                .sum::<usize>(),
            columns[4].parse::<usize>().unwrap(),
            "{name}: point count from independent APK audit"
        );
        for (index, stroke) in strokes.iter().enumerate() {
            let count = stroke.points.len();
            assert_eq!(
                stroke.pressures.len(),
                count,
                "{name}: stroke {index} pressures"
            );
            assert_eq!(
                stroke.timestamps.len(),
                count,
                "{name}: stroke {index} timestamps"
            );
            assert_eq!(stroke.tilts.len(), count, "{name}: stroke {index} tilts");
            assert_eq!(
                stroke.orientations.len(),
                count,
                "{name}: stroke {index} orientations"
            );
            assert!(stroke.pen_width.is_finite() && stroke.pen_width > 0.0);
            // The recorded base bbox independently constrains geometry. Allow
            // small quantization/float-rounding differences, not long stray
            // lines created by interpreting property masks as point counts.
            for point in &stroke.points {
                assert!(
                    point.x >= stroke.bbox.x_min - 2.0
                        && point.x <= stroke.bbox.x_max + 2.0
                        && point.y >= stroke.bbox.y_min - 2.0
                        && point.y <= stroke.bbox.y_max + 2.0,
                    "{name}: stroke {index} point {point:?} outside {:?}",
                    stroke.bbox
                );
            }
        }
    }
}
