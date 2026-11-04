//! Decode retained native records without invoking the renderer under test.
use std::{collections::BTreeMap, sync::OnceLock};

use serde_json::Value;
use sha2::{Digest, Sha256};

pub fn bytes(name: &str) -> &'static [u8] {
    // These standalone captures are byte-identical to checked native bundle slices.
    // Preserve their historical names without storing duplicate binary fixtures.
    let alias = match name {
        "conformance/legacy-marker-source-straight-sparse-rgba8.bin" => Some((0, 3040)),
        "conformance/legacy-marker-source-bend-sparse-rgba8.bin" => Some((3040, 2680)),
        "conformance/legacy-marker-alpha40-sparse-rgba8.bin" => Some((5720, 3040)),
        _ => None,
    };
    if let Some((offset, length)) = alias {
        return &bytes("conformance/legacy-marker-empty-setting-sparse-rgba8.bin")
            [offset..offset + length];
    }
    static CASES: OnceLock<BTreeMap<String, Vec<u8>>> = OnceLock::new();
    CASES
        .get_or_init(|| {
            let fixture: Value = serde_json::from_str(include_str!(
                "../../../../conformance/legacy-marker-sparse-runs.json"
            ))
            .unwrap();
            let empty: Value = serde_json::from_str(include_str!(
                "../../../../conformance/legacy-marker-empty-setting-host-pairs.json"
            ))
            .unwrap();
            let two: Value = serde_json::from_str(include_str!(
                "../../../../conformance/legacy-marker-two-point-controls.json"
            ))
            .unwrap();
            let widths: Value = serde_json::from_str(include_str!(
                "../../../../conformance/legacy-marker-width-controls.json"
            ))
            .unwrap();
            let vector: Value = serde_json::from_str(include_str!(
                "../../../../conformance/legacy-marker-vector-public-comparison.json"
            ))
            .unwrap();
            let alpha01: Value = serde_json::from_str(include_str!(
                "../../../../conformance/legacy-marker-alpha01-control.json"
            ))
            .unwrap();
            let alphac8: Value = serde_json::from_str(include_str!(
                "../../../../conformance/legacy-marker-alphac8-control.json"
            ))
            .unwrap();
            let long: Value = serde_json::from_str(include_str!(
                "../../../../conformance/legacy-marker-long-native-controls.json"
            ))
            .unwrap();
            assert_eq!(fixture["schema"], "sdocx-native-sparse-rgba-runs-v1");
            let cases = fixture["cases"].as_object().unwrap();
            assert_eq!(cases.len(), 16);
            let mut frame_cases = 0;
            let mut total_slices = 0;
            let decoded = cases
                .iter()
                .map(|(name, case)| {
                    let length = usize::try_from(case["byte_length"].as_u64().unwrap()).unwrap();
                    assert!(length <= 23_456 && length % 8 == 0);
                    let values = case["palette"].as_array().unwrap();
                    assert!(!values.is_empty() && values.len() <= 203);
                    let palette: Vec<[u8; 4]> = values
                        .iter()
                        .map(|value| {
                            let hex = value.as_str().unwrap();
                            assert_eq!(hex.len(), 8);
                            assert!(hex.bytes().all(|b| b.is_ascii_hexdigit()));
                            std::array::from_fn(|i| {
                                u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap()
                            })
                        })
                        .collect();
                    let mut raw = Vec::with_capacity(length);
                    assert_ne!(case.get("runs").is_some(), case.get("row_runs").is_some());
                    if let Some(groups) = case.get("row_runs") {
                        assert_eq!(case["frame_dimensions"], serde_json::json!([1920, 1080]));
                        let mut previous_y_end = 0;
                        for group in groups.as_array().unwrap() {
                            let group = group.as_array().unwrap();
                            assert_eq!(group.len(), 3);
                            let first_y = u32::try_from(group[0].as_u64().unwrap()).unwrap();
                            let repeats = u32::try_from(group[1].as_u64().unwrap()).unwrap();
                            let end_y = first_y.checked_add(repeats).unwrap();
                            assert!(repeats > 0 && first_y >= previous_y_end && end_y <= 1080);
                            previous_y_end = end_y;
                            let runs = group[2].as_array().unwrap();
                            assert!(!runs.is_empty() && runs.len() <= 1920);
                            for y in first_y..end_y {
                                let mut previous_x_end = 0;
                                for run in runs {
                                    let run = run.as_array().unwrap();
                                    assert_eq!(run.len(), 3);
                                    let first_x = u32::try_from(run[0].as_u64().unwrap()).unwrap();
                                    let count = u32::try_from(run[1].as_u64().unwrap()).unwrap();
                                    let end_x = first_x.checked_add(count).unwrap();
                                    assert!(
                                        count > 0 && first_x >= previous_x_end && end_x <= 1920
                                    );
                                    assert!(count as usize <= (length - raw.len()) / 8);
                                    previous_x_end = end_x;
                                    let index = usize::try_from(run[2].as_u64().unwrap()).unwrap();
                                    let pixel = palette.get(index).expect("invalid palette index");
                                    for x in first_x..end_x {
                                        raw.extend_from_slice(&(y * 1920 + x).to_le_bytes());
                                        raw.extend_from_slice(pixel);
                                    }
                                }
                            }
                        }
                    } else {
                        for run in case["runs"].as_array().unwrap() {
                            let run = run.as_array().unwrap();
                            assert_eq!(run.len(), 3);
                            let first = u32::try_from(run[0].as_u64().unwrap()).unwrap();
                            let count = u32::try_from(run[1].as_u64().unwrap()).unwrap();
                            assert!(count > 0 && count as usize <= (length - raw.len()) / 8);
                            let end = first.checked_add(count).unwrap();
                            assert!(end <= 1920 * 1080);
                            let index = usize::try_from(run[2].as_u64().unwrap()).unwrap();
                            let pixel = palette.get(index).expect("invalid palette index");
                            for index in first..end {
                                raw.extend_from_slice(&index.to_le_bytes());
                                raw.extend_from_slice(pixel);
                            }
                        }
                    }
                    assert_eq!(raw.len(), length);
                    assert_eq!(
                        format!("{:x}", Sha256::digest(&raw)),
                        case["sha256"].as_str().unwrap()
                    );
                    if case.get("row_runs").is_some() {
                        let control = case["control"].as_str().unwrap();
                        assert_eq!(
                            name,
                            &format!("conformance/legacy-marker-{control}-sparse-rgba8.bin")
                        );
                        let (sparse_sha, frame_sha, nonzero) = match control {
                            "size2" | "size3" | "size6" | "size24" | "size48" => {
                                let receipt = &widths["controls"][control];
                                assert_eq!(receipt["sparse_path"].as_str().unwrap(), name);
                                (
                                    &receipt["sparse_sha256"],
                                    &receipt["frame_sha256"],
                                    receipt["nonzero_channel_bytes"].as_u64(),
                                )
                            }
                            "alpha00" => (
                                vector
                                    .pointer("/zero_alpha_cap_fix/fixture_sha256")
                                    .unwrap(),
                                vector
                                    .pointer("/zero_alpha_cap_fix/cases/0/native_frame_sha256")
                                    .unwrap(),
                                None,
                            ),
                            "alpha01" => (
                                alpha01.pointer("/sparse_sha256").unwrap(),
                                alpha01.pointer("/final_frame_sha256").unwrap(),
                                None,
                            ),
                            "alphac8" => (
                                alphac8.pointer("/sparse_sha256").unwrap(),
                                alphac8.pointer("/final_frame_sha256").unwrap(),
                                None,
                            ),
                            "two-alpha20" => (
                                vector
                                    .pointer("/two_drawing_low_alpha_second/fixture_sha256")
                                    .unwrap(),
                                vector
                                    .pointer("/two_drawing_low_alpha_second/native_frame_sha256")
                                    .unwrap(),
                                None,
                            ),
                            "two-distinct" => (
                                vector
                                    .pointer("/two_distinct_drawing_submissions/fixture_sha256")
                                    .unwrap(),
                                vector
                                    .pointer(
                                        "/two_distinct_drawing_submissions/native_frame_sha256",
                                    )
                                    .unwrap(),
                                None,
                            ),
                            "two-drawing" => (
                                vector
                                    .pointer("/two_identical_drawing_submissions/fixture_sha256")
                                    .unwrap(),
                                vector
                                    .pointer(
                                        "/two_identical_drawing_submissions/native_frame_sha256",
                                    )
                                    .unwrap(),
                                None,
                            ),
                            "self-overlap" => (
                                vector.pointer("/self_overlap_fixture/sha256").unwrap(),
                                vector.pointer("/cases/2/native_frame_sha256").unwrap(),
                                None,
                            ),
                            "long-turns" => (
                                long.pointer("/cases/1/sparse_sha256").unwrap(),
                                long.pointer("/cases/1/frame_sha256").unwrap(),
                                None,
                            ),
                            _ => panic!("unknown native single-frame control"),
                        };
                        assert_eq!(&case["sha256"], sparse_sha);
                        assert_eq!(&case["frame_sha256"], frame_sha);
                        let mut frame = vec![0_u8; 1920 * 1080 * 4];
                        for record in raw.as_chunks::<8>().0 {
                            let index =
                                u32::from_le_bytes(record[..4].try_into().unwrap()) as usize;
                            frame[index * 4..index * 4 + 4].copy_from_slice(&record[4..]);
                        }
                        assert_eq!(
                            format!("{:x}", Sha256::digest(&frame)),
                            frame_sha.as_str().unwrap()
                        );
                        if let Some(nonzero) = nonzero {
                            assert_eq!(frame.iter().filter(|&&v| v != 0).count() as u64, nonzero);
                        }
                        frame_cases += 1;
                        return (name.clone(), raw);
                    }
                    let rows: Vec<&Value> = match name.as_str() {
                        "conformance/legacy-marker-empty-setting-sparse-rgba8.bin" => {
                            empty["pairs"].as_array().unwrap().iter().collect()
                        }
                        "conformance/legacy-marker-empty-omitted-sparse-rgba8.bin" => {
                            empty["omitted_color_pairs"]["cases"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .collect()
                        }
                        "conformance/legacy-marker-two-point-sparse-rgba8.bin" => two["cases"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .chain(std::iter::once(&two["scale2_counterexample"]))
                            .collect(),
                        _ => panic!("unknown native sparse bundle"),
                    };
                    let slices = case["slices"].as_array().unwrap();
                    assert_eq!(slices.len(), rows.len());
                    let mut offset: usize = 0;
                    for (slice, receipt) in slices.iter().zip(rows) {
                        assert_eq!(slice["byte_offset"], receipt["sparse_offset"]);
                        assert_eq!(slice["byte_length"], receipt["sparse_bytes"]);
                        assert_eq!(slice["sha256"], receipt["sparse_sha256"]);
                        assert_eq!(slice["byte_offset"].as_u64().unwrap(), offset as u64);
                        let size = usize::try_from(slice["byte_length"].as_u64().unwrap()).unwrap();
                        assert_eq!(size % 8, 0);
                        let end = offset.checked_add(size).unwrap();
                        let segment = raw.get(offset..end).unwrap();
                        // Ordering restarts at each original frame slice, not at bundle boundaries.
                        let mut previous = None;
                        for record in segment.as_chunks::<8>().0 {
                            let index = u32::from_le_bytes(record[..4].try_into().unwrap());
                            assert!(index < 1920 * 1080);
                            assert!(previous.is_none_or(|value| index > value));
                            previous = Some(index);
                        }
                        assert_eq!(
                            format!("{:x}", Sha256::digest(segment)),
                            receipt["sparse_sha256"].as_str().unwrap()
                        );
                        offset = end;
                        total_slices += 1;
                    }
                    assert_eq!(offset, raw.len());
                    (name.clone(), raw)
                })
                .collect();
            assert_eq!(total_slices, 12);
            assert_eq!(frame_cases, 13);
            decoded
        })
        .get(name)
        .expect("unknown native sparse bundle")
        .as_slice()
}

#[test]
fn all_native_sparse_bundles_preserve_receipt_slices() {
    // The loader validates every bundle, including the historical two-point case.
    assert_eq!(
        bytes("conformance/legacy-marker-two-point-sparse-rgba8.bin").len(),
        18_264
    );
}
