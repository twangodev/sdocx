//! Lossless decoding of retained native pixels, independent of the paint model.
use std::{collections::BTreeMap, sync::OnceLock};

use sha2::{Digest, Sha256};

fn rgba(value: &serde_json::Value) -> [u8; 4] {
    let hex = value.as_str().unwrap().as_bytes();
    assert_eq!(hex.len(), 8);
    let nibble = |byte| match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        _ => panic!("invalid native RGBA hex"),
    };
    std::array::from_fn(|i| nibble(hex[i * 2]) * 16 + nibble(hex[i * 2 + 1]))
}

pub(crate) fn pixels(name: &str) -> &'static [u8] {
    static CASES: OnceLock<BTreeMap<String, Vec<u8>>> = OnceLock::new();
    CASES
        .get_or_init(|| {
            let fixture: serde_json::Value = serde_json::from_str(include_str!(
                "../../../../conformance/simplebrush-native-crop-runs.json"
            ))
            .unwrap();
            assert!(matches!(
                fixture["schema"].as_str(),
                Some("sdocx-native-rgba-runs-v1" | "sdocx-native-rgba-runs-v2")
            ));
            let cases = fixture["cases"].as_object().unwrap();
            assert_eq!(
                cases.len(),
                if fixture["schema"] == "sdocx-native-rgba-runs-v1" {
                    31
                } else {
                    65
                }
            );
            cases
                .iter()
                .map(|(name, case)| {
                    let length = usize::try_from(case["byte_length"].as_u64().unwrap()).unwrap();
                    assert!(length <= 231_424 && length % 4 == 0);
                    let palette = case.get("palette").map(|value| {
                        let values = value.as_array().unwrap();
                        assert!(!values.is_empty() && values.len() <= 358);
                        values.iter().map(rgba).collect::<Vec<_>>()
                    });
                    let mut pixels = Vec::with_capacity(length);
                    for run in case["runs"].as_array().unwrap() {
                        let run = run.as_array().unwrap();
                        assert_eq!(run.len(), 2);
                        let count = usize::try_from(run[0].as_u64().unwrap()).unwrap();
                        assert!(count > 0 && count <= (length - pixels.len()) / 4);
                        let pixel = match &palette {
                            Some(palette) => {
                                let index = usize::try_from(run[1].as_u64().unwrap()).unwrap();
                                *palette
                                    .get(index)
                                    .expect("native palette index out of range")
                            }
                            None => rgba(&run[1]),
                        };
                        for _ in 0..count {
                            pixels.extend_from_slice(&pixel);
                        }
                    }
                    assert_eq!(pixels.len(), length, "{name}");
                    assert_eq!(
                        format!("{:x}", Sha256::digest(&pixels)),
                        case["sha256"].as_str().unwrap(),
                        "{name}"
                    );
                    (name.clone(), pixels)
                })
                .collect()
        })
        .get(name)
        .expect("unknown native crop")
        .as_slice()
}

#[test]
fn receipt_only_scale_crops_preserve_native_hashes() {
    let scale: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/simplebrush-scale-2x-host.json"
    ))
    .unwrap();
    let points: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/simplebrush-scale-2x-points.json"
    ))
    .unwrap();
    for (filename, cases) in [
        ("simplebrush-2x-scale2.rgba", &scale["capture"]["cases"]),
        ("simplebrush-scale-2x-points-4.rgba", &points["cases"]),
        ("simplebrush-scale-2x-points-7.rgba", &points["cases"]),
    ] {
        let receipt = cases
            .as_array()
            .unwrap()
            .iter()
            .find(|case| case["crop"]["file"].as_str() == Some(filename))
            .unwrap();
        // Loading a case also validates every retained case's byte length/hash.
        let crop = pixels(&format!("conformance/{filename}"));
        assert_eq!(
            format!("{:x}", Sha256::digest(crop)),
            receipt["crop"]["sha256"].as_str().unwrap()
        );
        let bounds = receipt["alpha_bounds"].as_array().unwrap();
        assert_eq!(bounds.len(), 4);
        let [x0, y0, x1, y1]: [usize; 4] =
            std::array::from_fn(|i| usize::try_from(bounds[i].as_u64().unwrap()).unwrap());
        assert!(x0 < x1 && x1 <= 256 && y0 < y1 && y1 <= 256);
        let row_bytes = (x1 - x0) * 4;
        assert_eq!(crop.len(), row_bytes * (y1 - y0));
        // Preserve native readback row order on the transparent 256x256 target.
        // This checks retained evidence, without invoking a renderer.
        let mut frame = vec![0_u8; 256 * 256 * 4];
        for (row, pixels) in crop.chunks_exact(row_bytes).enumerate() {
            let offset = ((y0 + row) * 256 + x0) * 4;
            frame[offset..offset + row_bytes].copy_from_slice(pixels);
        }
        assert_eq!(
            format!("{:x}", Sha256::digest(&frame)),
            receipt["frame_sha256"].as_str().unwrap()
        );
    }
}
