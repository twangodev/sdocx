//! Lossless decoding of retained native pixels, independent of the paint model.
use std::{collections::BTreeMap, sync::OnceLock};

use sha2::{Digest, Sha256};

pub(crate) fn pixels(name: &str) -> &'static [u8] {
    static CASES: OnceLock<BTreeMap<String, Vec<u8>>> = OnceLock::new();
    CASES
        .get_or_init(|| {
            let fixture: serde_json::Value = serde_json::from_str(include_str!(
                "../../../../conformance/simplebrush-native-crop-runs.json"
            ))
            .unwrap();
            assert_eq!(fixture["schema"], "sdocx-native-rgba-runs-v1");
            let cases = fixture["cases"].as_object().unwrap();
            assert_eq!(cases.len(), 31);
            cases
                .iter()
                .map(|(name, case)| {
                    let length = usize::try_from(case["byte_length"].as_u64().unwrap()).unwrap();
                    assert!(length <= 18_432 && length % 4 == 0);
                    let mut pixels = Vec::with_capacity(length);
                    for run in case["runs"].as_array().unwrap() {
                        let run = run.as_array().unwrap();
                        assert_eq!(run.len(), 2);
                        let count = usize::try_from(run[0].as_u64().unwrap()).unwrap();
                        assert!(count > 0 && count <= (length - pixels.len()) / 4);
                        let hex = run[1].as_str().unwrap().as_bytes();
                        assert_eq!(hex.len(), 8);
                        let nibble = |byte| match byte {
                            b'0'..=b'9' => byte - b'0',
                            b'a'..=b'f' => byte - b'a' + 10,
                            _ => panic!("invalid native RGBA hex"),
                        };
                        let rgba: [u8; 4] = std::array::from_fn(|i| {
                            nibble(hex[i * 2]) * 16 + nibble(hex[i * 2 + 1])
                        });
                        for _ in 0..count {
                            pixels.extend_from_slice(&rgba);
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
