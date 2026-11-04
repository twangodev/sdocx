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
            assert_eq!(fixture["schema"], "sdocx-native-sparse-rgba-runs-v1");
            let cases = fixture["cases"].as_object().unwrap();
            assert_eq!(cases.len(), 3);
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
                    assert_eq!(raw.len(), length);
                    assert_eq!(
                        format!("{:x}", Sha256::digest(&raw)),
                        case["sha256"].as_str().unwrap()
                    );
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
