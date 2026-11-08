use super::device_raster::{self, Context};
use super::mixed_rgb_tests::{control_records, page_context_receipt};
use base64::Engine;
use flate2::read::ZlibDecoder;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::Read;

#[derive(Deserialize)]
struct PrefixFrame {
    prefix_rgba: Vec<Vec<[u8; 4]>>,
    full_sha256: String,
}

#[derive(Deserialize)]
struct PrefixReceipt {
    pixels_bottom_origin: Vec<[usize; 2]>,
    input_sha256: BTreeMap<String, String>,
    results: BTreeMap<String, PrefixFrame>,
}

fn original() -> PrefixReceipt {
    serde_json::from_str(include_str!(
        "../../../../../conformance/pencil2-v14-page-prefix-native.json"
    ))
    .unwrap()
}

fn context() -> Context {
    Context {
        scale: 2.,
        y_up: false,
        ..Context::default()
    }
}

fn bottom_hash(image: &[[u8; 4]]) -> String {
    let mut digest = Sha256::new();
    for row in image.as_chunks::<192>().0.iter().rev() {
        for pixel in row {
            digest.update(pixel);
        }
    }
    format!("{:x}", digest.finalize())
}

#[test]
fn page_prefixes_isolate_existing_residuals_to_individual_stamps() {
    let receipt = original();
    assert_eq!(receipt.results.len(), 3);
    assert_eq!(
        receipt.pixels_bottom_origin,
        [[11, 190], [23, 190], [9, 187], [21, 187]]
    );
    let page = page_context_receipt();
    for (name, frame) in &receipt.results {
        let records = control_records(name);
        let bytes: Vec<_> = records
            .iter()
            .flatten()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        assert_eq!(
            format!("{:x}", Sha256::digest(bytes)),
            receipt.input_sha256[name]
        );
        assert_eq!(frame.prefix_rgba.len(), 84);
        let page_pixels = page.results[&format!("{name}_top_scale2")].image();
        assert_eq!(bottom_hash(&page_pixels), frame.full_sha256);
        let mut first_difference = [None; 4];
        let mut different_pixels = 0;
        for (i, native) in frame.prefix_rgba.iter().enumerate() {
            assert_eq!(native.len(), 4);
            let mut prefix = records.clone();
            for row in &mut prefix[i + 1..] {
                row[4] = 0.;
            }
            let image = device_raster::render(&prefix, context()).unwrap();
            for (p, &[x, y]) in receipt.pixels_bottom_origin.iter().enumerate() {
                let index = (191 - y) * 192 + x;
                if image[index] != native[p] {
                    first_difference[p].get_or_insert(i + 1);
                    different_pixels += 1;
                }
                if i == 83 {
                    assert_eq!(native[p], page_pixels[index]);
                }
            }
        }
        let expected = match name.as_str() {
            "colored" => ([Some(17), None, None, None], 68),
            "mixed" => ([Some(17), Some(32), Some(20), Some(35)], 236),
            "mixed_reversed" => ([Some(68), Some(53), Some(65), Some(50)], 104),
            _ => unreachable!(),
        };
        assert!(different_pixels <= expected.1, "{name}: {different_pixels}");
        for (actual, baseline) in first_difference.into_iter().zip(expected.0) {
            match (actual, baseline) {
                (Some(actual), Some(baseline)) => assert!(actual >= baseline, "{name}"),
                (Some(_), None) => panic!("{name}: new prefix residual"),
                (None, _) => {}
            }
        }
    }
}

#[test]
fn transform_feedback_is_rejected_as_an_original_interpolation_oracle() {
    let receipt: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../conformance/pencil2-v14-page-vertex-native.json"
    ))
    .unwrap();
    assert_eq!(receipt["valid_as_original_pipeline_oracle"], false);
    let reference = original();
    for (name, original) in reference.results {
        let observed: PrefixFrame =
            serde_json::from_value(receipt["prefix_results"][&name].clone()).unwrap();
        assert_eq!(observed.prefix_rgba[..83], original.prefix_rgba[..83]);
        assert_ne!(observed.prefix_rgba[83], original.prefix_rgba[83]);
        assert_ne!(observed.full_sha256, original.full_sha256);
        assert_eq!(
            receipt["comparison"][&name]["changed_prefix_ordinals"],
            serde_json::json!([84])
        );
        let records = control_records(&name);
        let image = device_raster::render(&records, context()).unwrap();
        assert_eq!(bottom_hash(&image), observed.full_sha256);

        let compressed = base64::engine::general_purpose::STANDARD
            .decode(
                receipt["results"][&name]["f32_zlib_base64"]
                    .as_str()
                    .unwrap(),
            )
            .unwrap();
        let mut bytes = Vec::new();
        ZlibDecoder::new(compressed.as_slice())
            .take(84 * 6 * 16 + 1)
            .read_to_end(&mut bytes)
            .unwrap();
        assert_eq!(bytes.len(), 84 * 6 * 16);
        assert_eq!(
            format!("{:x}", Sha256::digest(&bytes)),
            receipt["results"][&name]["sha256"].as_str().unwrap()
        );
        for (r, vertices) in records.iter().zip(bytes.as_chunks::<96>().0) {
            let positions = [(0, 0), (0, 1), (1, 0), (1, 1)].map(|(i, j)| {
                let x = r[0] + [-1., 1.][i] * r[2];
                let y = r[1] + [-1., 1.][j] * r[3];
                [x * (4. / 192.) - 1., 1. + y * (-4. / 192.), 0., 1.]
            });
            for (&index, captured) in [0, 1, 2, 2, 1, 3].iter().zip(vertices.as_chunks::<16>().0) {
                let actual: [u32; 4] = std::array::from_fn(|i| {
                    u32::from_le_bytes(captured[i * 4..i * 4 + 4].try_into().unwrap())
                });
                assert_eq!(actual, positions[index].map(f32::to_bits), "{name}");
            }
        }
    }
}

#[test]
fn disabling_linear_rasterizer_preserves_the_native_prefix_controls() {
    let receipt: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../conformance/pencil2-v14-page-no-linear-native.json"
    ))
    .unwrap();
    assert_eq!(
        receipt["capture"]["environment"]["LP_PERF"],
        "no_rast_linear"
    );
    for (name, original) in original().results {
        assert_eq!(receipt["results"][&name]["all_84_prefixes_unchanged"], true);
        assert_eq!(
            receipt["results"][&name]["full_sha256"],
            original.full_sha256
        );
    }
}

#[test]
fn nominal_period_native_overlap_is_not_identical() {
    let receipt: serde_json::Value =
        serde_json::from_slice(include_bytes!("test_assets/periodic_edge/native.json")).unwrap();
    for (name, expected) in [
        ("mixed", 4),
        ("record_17", 1),
        ("record_20", 0),
        ("record_32", 1),
        ("record_35", 0),
    ] {
        let mut original = BTreeMap::new();
        let mut translated = BTreeMap::new();
        for (dy, target) in [(0, &mut original), (-150, &mut translated)] {
            for row in receipt["results"][format!("{name}:{dy}")]["pixels"]
                .as_array()
                .unwrap()
            {
                let p = row.as_array().unwrap();
                let x = p[0].as_i64().unwrap();
                let y = p[1].as_i64().unwrap() - dy;
                if (150..192).contains(&y) {
                    target.insert(
                        (x, y),
                        std::array::from_fn::<_, 4, _>(|i| p[i + 2].as_u64().unwrap() as u8),
                    );
                }
            }
        }
        let differences = original
            .keys()
            .chain(translated.keys())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .filter(|key| original.get(*key) != translated.get(*key))
            .count();
        assert_eq!(differences, expected, "{name}");
    }
}

#[test]
fn viewport_origin_controls_preserve_native_capture_and_baseline() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../conformance/pencil2-v14-expanded-viewport-sweep-native.json"
    ))
    .unwrap();
    let captures = fixture["viewport_controls_original"].as_object().unwrap();
    let cases = [
        (
            "viewport_1_5_receipt.json",
            "19f7959ac925cd2d919bf73410edf87299e066b9ea0cd251f13cc1153ddd14e2",
            true,
        ),
        (
            "viewport_origin_1_receipt.json",
            "b2f6c15da3a46d91b47ab37658873a242c1a06bd19f6b86a7ba97d36eea401c4",
            false,
        ),
        (
            "viewport_origin_2_receipt.json",
            "14c0793588ae96297306c3b24dc543a3d7027b467e437ed7fff19643b1e75d29",
            false,
        ),
        (
            "viewport_origin_4_receipt.json",
            "6f6a3397eb78ec9aabd864a8dee50b253d0b1f80e2bd53986619254e6180a038",
            true,
        ),
        (
            "viewport_origin_8_receipt.json",
            "6349bdea87b63e5e274b47422a9f18bacaa931f92f97e244faa4c1d8172c698f",
            true,
        ),
        (
            "viewport_origin_16_receipt.json",
            "d6bf8de19d1432e91e805ca7cc6a4b6ceec9d543e50e6237b4f95808e6e34103",
            true,
        ),
    ];
    assert_eq!(captures.len(), cases.len());
    for (name, expected_hash, changed) in cases {
        let compressed = base64::engine::general_purpose::STANDARD
            .decode(captures[name].as_str().unwrap())
            .unwrap();
        let mut original = Vec::new();
        flate2::read::GzDecoder::new(compressed.as_slice())
            .read_to_end(&mut original)
            .unwrap();
        assert_eq!(
            format!("{:x}", Sha256::digest(&original)),
            expected_hash,
            "{name}"
        );
        let receipt: serde_json::Value = serde_json::from_slice(&original).unwrap();
        assert_eq!(receipt["frame"], serde_json::json!([192, 192]));
        assert_eq!(
            !receipt["comparison"].as_object().unwrap().is_empty(),
            changed,
            "{name}"
        );
        for case in ["mixed", "record_17", "record_20", "record_32", "record_35"] {
            let input = format!("0_0_{case}");
            assert_eq!(
                receipt["inputs"][&input], fixture["inputs"][case],
                "{name} {case}"
            );
            assert_eq!(
                receipt["results"][format!("{input}:0")]["sha256"],
                fixture["results"][format!("{case}:0")]["sha256"],
                "{name} {case}"
            );
            if name == "viewport_1_5_receipt.json" {
                assert_eq!(
                    receipt["results"][format!("{input}:1")]["sha256"],
                    fixture["results"][format!("{case}:1")]["sha256"],
                    "{name} {case}"
                );
            }
        }
    }
}

#[test]
fn expanded_viewport_record_sweep_changes_only_edge_controls() {
    let edge: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../conformance/pencil2-v14-expanded-viewport-sweep-native.json"
    ))
    .unwrap();
    let interior: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../conformance/pencil2-v14-expanded-viewport-interior-sweep-native.json"
    ))
    .unwrap();
    fn hash<'a>(receipt: &'a serde_json::Value, name: &str, variant: u8) -> &'a str {
        let hash = receipt["results"][format!("{name}:{variant}")]["sha256"]
            .as_str()
            .expect("required frame hash");
        assert_eq!(hash.len(), 64);
        assert!(hash.bytes().all(|byte| byte.is_ascii_hexdigit()));
        hash
    }
    let records = control_records("mixed");
    assert_eq!(records.len(), 84);
    for receipt in [&edge, &interior] {
        assert_eq!(receipt["results"].as_object().unwrap().len(), 170);
        assert_eq!(receipt["inputs"].as_object().unwrap().len(), 85);
        for name in
            std::iter::once("mixed".to_owned()).chain((1..=84).map(|i| format!("record_{i}")))
        {
            assert!(receipt["inputs"].as_object().unwrap().contains_key(&name));
            for variant in 0..=1 {
                hash(receipt, &name, variant);
            }
        }
    }
    assert_ne!(hash(&edge, "mixed", 0), hash(&edge, "mixed", 1));
    for (i, record) in records.iter().enumerate() {
        let [x, y, rx, ry] = [record[0], record[1], record[2], record[3]];
        assert!(32. + 2. * (x - rx) > 0. && 32. + 2. * (x + rx) < 192.);
        assert!(160. - 2. * (y + ry) > 0. && 160. - 2. * (y - ry) < 192.);
        let name = format!("record_{}", i + 1);
        let same = |receipt: &serde_json::Value| hash(receipt, &name, 0) == hash(receipt, &name, 1);
        assert_eq!(!same(&edge), [17, 20, 32, 35].contains(&(i + 1)));
        assert!(same(&interior), "{name}");
    }
    assert_eq!(hash(&interior, "mixed", 0), hash(&interior, "mixed", 1));
}
