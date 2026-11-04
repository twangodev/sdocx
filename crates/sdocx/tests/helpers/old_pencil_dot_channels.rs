//! Exact native getter streams, retaining serialized-source auxiliary channels.
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, sync::OnceLock};

pub fn channels(name: &str) -> &'static [u8] {
    static CASES: OnceLock<BTreeMap<String, Vec<u8>>> = OnceLock::new();
    CASES
        .get_or_init(|| {
            let fixture: serde_json::Value = serde_json::from_str(include_str!(
                "../../../../conformance/old-pencil-counterexample/dot-channels.json"
            ))
            .unwrap();
            assert_eq!(fixture["schema"], "sdocx-native-dot-channels-v1");
            assert_eq!(
                fixture["channel_order"],
                serde_json::json!(["points_xy", "pressure", "timestamps", "tilt", "orientation"])
            );
            let cases = fixture["cases"].as_object().unwrap();
            assert_eq!(cases.len(), 8);
            cases
                .iter()
                .map(|(name, case)| {
                    let count = usize::try_from(case["count"].as_u64().unwrap()).unwrap();
                    assert!(matches!(count, 2 | 204));
                    let refs = case["streams"].as_array().unwrap();
                    assert_eq!(refs.len(), 5);
                    let mut bytes = Vec::with_capacity(count * 24);
                    for (channel, reference) in refs.iter().enumerate() {
                        let stream = &fixture["streams"][reference.as_str().unwrap()];
                        let kind =
                            ["f32_xy_bits", "f32_bits", "i32", "f32_bits", "f32_bits"][channel];
                        assert_eq!(stream["type"], kind);
                        let mut samples = 0;
                        for run in stream["runs"].as_array().unwrap() {
                            let run = run.as_array().unwrap();
                            assert_eq!(run.len(), 2);
                            let repeat = usize::try_from(run[0].as_u64().unwrap()).unwrap();
                            assert!(repeat > 0 && repeat <= count - samples);
                            samples += repeat;
                            let values = if channel == 0 {
                                let pair = run[1].as_array().unwrap();
                                assert_eq!(pair.len(), 2);
                                pair.clone()
                            } else {
                                vec![run[1].clone()]
                            };
                            let words: Vec<[u8; 4]> = values
                                .iter()
                                .map(|value| {
                                    if channel == 2 {
                                        i32::try_from(value.as_i64().unwrap())
                                            .unwrap()
                                            .to_le_bytes()
                                    } else {
                                        let bits = value.as_str().unwrap();
                                        assert_eq!(bits.len(), 8);
                                        u32::from_str_radix(bits, 16).unwrap().to_le_bytes()
                                    }
                                })
                                .collect();
                            for _ in 0..repeat {
                                for word in &words {
                                    bytes.extend_from_slice(word);
                                }
                            }
                        }
                        assert_eq!(samples, count);
                    }
                    assert_eq!(bytes.len(), count * 24);
                    let sha = format!("{:x}", Sha256::digest(&bytes));
                    assert_eq!(sha, case["sha256"].as_str().unwrap());
                    let label = name
                        .strip_prefix("dot-")
                        .unwrap()
                        .strip_suffix(".channels")
                        .unwrap();
                    // Original native channels_sha256 pins from the capture receipts.
                    let expected = match label {
                        "12-visible" => {
                            "949587d51f87dc662ab585ec0ebe1b6ff811ae986ffc83facb97cede5007db96"
                        }
                        "12_zero" => {
                            "949587d51f87dc662ab585ec0ebe1b6ff811ae986ffc83facb97cede5007db96"
                        }
                        "3_full" => {
                            "220c60e9bf18e27225c34ce635b5e65e33110cdb553c3b44ddcdb9dfe938de81"
                        }
                        "425" => "726f008d7e3b67dffe74f698b266ba49d3603d26fed39b9bb1c8829b430b6b10",
                        "8" => "7922f60678bd9474fd95e3360ca03d673152822af279da9a84cec116fac5f609",
                        "threshold_below" => {
                            "3ed16b0577348eec77c13e42f8c413d39e3516455e57763b821a1c0adf656b5a"
                        }
                        "threshold_exact" => {
                            "6f0de310a085c5a75838f555ae7f266506edf64a5b46531665dab334aa8a8ffc"
                        }
                        "two" => "a7b8e035f63df25567f53c939dada4ef69b8dfe53bbd378f5686dad930b34f3d",
                        _ => panic!("unknown native dot receipt"),
                    };
                    assert_eq!(sha, expected);
                    (name.clone(), bytes)
                })
                .collect()
        })
        .get(name)
        .expect("unknown native dot channels")
        .as_slice()
}

#[test]
fn all_dot_channels_match_native_receipt_hashes() {
    assert_eq!(channels("dot-two.channels").len(), 48);
}
