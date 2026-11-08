//! Scalable vector contours for MagicPen GLV1 stamps.
//!
//! The polygon is a source-derived approximation of native radial coverage:
//! at canonical 1 px/source-unit scale, its area on each ray equals the
//! integral of the native linear radial field. It is not pointwise exact and
//! does not follow the native shader's scale-dependent finite 2x2 `fwidth`.

const CONTOUR_VERTICES: usize = 192;
const MAX_RECORDS: usize = 65_536;
const MAX_PATH_BYTES: usize = 8 * 1024 * 1024;

pub(super) fn source_contour_path(records: &[[u32; 3]], scale: u32) -> Option<String> {
    if !matches!(scale, 1 | 2 | 4) {
        return None;
    }
    if records.is_empty() || records.len() > MAX_RECORDS {
        return None;
    }

    let mut path = String::new();
    for &[x_bits, y_bits, radius_bits] in records {
        let x = f32::from_bits(x_bits) as f64;
        let y = f32::from_bits(y_bits) as f64;
        let radius = f32::from_bits(radius_bits) as f64;
        if !x.is_finite() || !y.is_finite() || !radius.is_finite() || radius < 0.0 {
            return None;
        }

        for vertex in 0..CONTOUR_VERTICES {
            let angle = 2.0 * std::f64::consts::PI * vertex as f64 / CONTOUR_VERTICES as f64;
            let (sin, cos) = angle.sin_cos();
            // Preserve the radial integral of the canonical linear coverage
            // ramp. This retains small stamps whose half-coverage contour
            // has zero area, without fitting a captured pixel boundary.
            let edge = (cos.abs() + sin.abs()) / f64::from(scale);
            let rho2 = if radius >= edge {
                radius * radius - radius * edge + edge * edge / 3.0
            } else {
                radius * radius * radius / (3.0 * edge)
            };
            let contour_radius = rho2.max(0.0).sqrt();
            let px = x + contour_radius * cos;
            let py = y + contour_radius * sin;
            if !px.is_finite() || !py.is_finite() {
                return None;
            }
            if vertex == 0 {
                use std::fmt::Write as _;
                write!(&mut path, "M{px:.8},{py:.8}").ok()?;
            } else {
                use std::fmt::Write as _;
                write!(&mut path, " L{px:.8},{py:.8}").ok()?;
            }
            if path.len() > MAX_PATH_BYTES {
                return None;
            }
        }
        path.push_str(" Z");
        if path.len() > MAX_PATH_BYTES {
            return None;
        }
    }
    Some(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use serde_json::Value;
    use sha2::{Digest, Sha256};
    use std::io::Read;

    fn decode_native_frame(case: &Value, data_key: &str, hash_key: &str, size: usize) -> Vec<u8> {
        decode_frame_data(&case[data_key], &case[hash_key], size)
    }

    fn decode_frame_data(data: &Value, hash: &Value, size: usize) -> Vec<u8> {
        let compressed = base64::engine::general_purpose::STANDARD
            .decode(data.as_str().unwrap())
            .unwrap();
        let mut bytes = Vec::new();
        flate2::read::ZlibDecoder::new(compressed.as_slice())
            .take((size + 1) as u64)
            .read_to_end(&mut bytes)
            .unwrap();
        assert_eq!(bytes.len(), size);
        assert_eq!(format!("{:x}", Sha256::digest(&bytes)), *hash);
        bytes
    }

    #[test]
    fn native_edge_translation_preserves_overlapping_pixels() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/magicpen-phase-export-host.json"
        ))
        .unwrap();
        let controls = &fixture["edge_translation_controls"];
        let originals = controls["native"].as_array().unwrap();
        let translations = controls["translated"].as_array().unwrap();
        assert_eq!(originals.len(), 6);
        assert_eq!(translations.len(), 6);

        fn find<'a>(cases: &'a [Value], name: &str, scale: usize) -> &'a Value {
            let mut found = cases
                .iter()
                .filter(|case| case["case"] == name && case["scale"] == scale);
            let case = found.next().expect("missing native edge control");
            assert!(found.next().is_none(), "duplicate native edge control");
            case
        }

        for (name, shift) in [("left_edge", 8_isize), ("right_edge", -8)] {
            for scale in [1_usize, 2, 4] {
                let original = find(originals, name, scale);
                let translated = find(translations, name, scale);
                assert_eq!(translated["shift"], shift);
                assert_eq!(translated["overlap_rgba_error"], 0);
                assert_eq!(translated["overlap_changed_pixels"], 0);
                let width = 32 * scale;
                let size = width * width * 4;
                let before = decode_native_frame(
                    original,
                    "transparent_rgba_zlib",
                    "transparent_sha256",
                    size,
                );
                let after = decode_native_frame(
                    translated,
                    "translated_rgba_zlib",
                    "translated_sha256",
                    size,
                );
                let _opaque =
                    decode_native_frame(original, "opaque_rgba_zlib", "opaque_sha256", size);
                for y in 0..width {
                    for x in 0..width {
                        let translated_x = x as isize + shift * scale as isize;
                        if (0..width as isize).contains(&translated_x) {
                            let left = (y * width + x) * 4;
                            let right = (y * width + translated_x as usize) * 4;
                            assert_eq!(
                                &before[left..left + 4],
                                &after[right..right + 4],
                                "{name} at {scale}x, pixel ({x}, {y})"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn native_cell_frames_match_opaque_background_composition() {
        use std::collections::BTreeSet;

        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/magicpen-phase-export-host.json"
        ))
        .unwrap();
        for group in ["standard", "variable"] {
            let cases = fixture["cell_composition_controls"][group]
                .as_array()
                .unwrap();
            assert_eq!(cases.len(), 24);
            let mut names = BTreeSet::new();
            let mut pairs = BTreeSet::new();
            for case in cases {
                let name = case["case"].as_str().unwrap();
                let scale = case["scale"].as_u64().unwrap() as usize;
                assert!([1, 2, 4].contains(&scale));
                names.insert(name);
                assert!(pairs.insert((name, scale)), "duplicate {group} control");
                assert_eq!(case["backed_rgb_error"], 0);
                let size = (32 * scale).pow(2) * 4;
                let transparent =
                    decode_native_frame(case, "transparent_rgba_zlib", "transparent_sha256", size);
                let opaque = decode_native_frame(case, "opaque_rgba_zlib", "opaque_sha256", size);
                for (source, actual) in transparent
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .zip(opaque.as_chunks::<4>().0.iter())
                {
                    assert_eq!(actual[3], 255);
                    for (channel, background) in [64_u16, 128, 192].into_iter().enumerate() {
                        let alpha = u16::from(source[3]);
                        let expected = (u16::from(source[channel])
                            + (background * (255 - alpha) + 127) / 255)
                            .min(255) as u8;
                        assert_eq!(actual[channel], expected, "{group}/{name}/{scale}x");
                    }
                }
            }
            assert_eq!(names.len(), 8);
        }
    }

    #[test]
    fn native_minimum_dot_color_and_alpha_controls() {
        use std::collections::BTreeSet;

        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/magicpen-phase-export-host.json"
        ))
        .unwrap();
        let cases = fixture["single_dot_color_controls"]["cases"]
            .as_array()
            .unwrap();
        assert_eq!(cases.len(), 15);
        let mut seen = BTreeSet::new();
        for (position, opaque_alpha, half_alpha) in
            [("p825", 20_u8, 10_u8), ("p850", 64, 32), ("p875", 43, 22)]
        {
            for (color, expected_alpha) in [
                ("ff336699", opaque_alpha),
                ("80336699", half_alpha),
                ("01336699", 0),
                ("80ff0000", half_alpha),
                ("4000ff00", (u16::from(opaque_alpha) + 2) as u8 / 4),
            ] {
                let id = format!("{position}-{color}");
                let mut matches = cases.iter().filter(|case| case["input"]["id"] == id);
                let case = matches.next().expect("missing native color control");
                assert!(matches.next().is_none(), "duplicate native color control");
                assert!(seen.insert(id));
                let readback = &case["readback"];
                assert_eq!(readback["width"], 32);
                assert_eq!(readback["height"], 32);
                assert_eq!(readback["bytes"], 32 * 32 * 4);
                let frame =
                    decode_native_frame(readback, "rgba8_zlib_base64", "sha256", 32 * 32 * 4);
                let pixels = frame.as_chunks::<4>().0;
                let active: Vec<_> = pixels.iter().filter(|pixel| pixel[3] != 0).collect();
                assert_eq!(active.len(), usize::from(expected_alpha != 0));
                assert_eq!(readback["changed_from_background_pixels"], active.len());
                if let Some(pixel) = active.first() {
                    assert_eq!(pixel[3], expected_alpha);
                    assert!(pixel[..3].iter().all(|channel| *channel <= pixel[3]));
                }
            }
        }
        assert_eq!(seen.len(), cases.len());
    }

    #[test]
    fn native_sequential_stroke_frames_preserve_seed_handoff() {
        use std::collections::BTreeSet;

        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/magicpen-phase-export-host.json"
        ))
        .unwrap();
        let cases = fixture["two_stroke_controls"].as_array().unwrap();
        let reentrant = fixture["reentrant_controls"].as_array().unwrap();
        assert_eq!(cases.len(), 6);
        assert_eq!(reentrant.len(), 6);
        let mut seen = BTreeSet::new();
        for case in cases {
            let background = case["background"].as_str().unwrap();
            let scale = case["scale"].as_u64().unwrap() as usize;
            assert!(["solid", "pattern"].contains(&background));
            assert!([1, 2, 4].contains(&scale));
            assert!(seen.insert((background, scale)));
            let stages = case["stages"].as_array().unwrap();
            assert_eq!(stages.len(), 2);
            assert_eq!(stages[0]["case"], "red");
            assert_eq!(stages[1]["case"], "blue");
            assert_eq!(stages[1]["seed_sha256"], stages[0]["readback_sha256"]);
            let mut comparisons = reentrant
                .iter()
                .filter(|capture| capture["background"] == background && capture["scale"] == scale);
            let joined = comparisons.next().expect("missing reentrant control");
            assert!(comparisons.next().is_none(), "duplicate reentrant control");
            assert_eq!(joined["sha256"], stages[1]["readback_sha256"]);
            assert_eq!(joined["difference_from_two_process_rgb"], 0);
            let first_hash = stages[0]["readback_sha256"].as_str().unwrap();
            assert!(
                joined["trace"]
                    .as_str()
                    .unwrap()
                    .contains(&format!("FIRST_FRAME {first_hash} "))
            );
            let size = (32 * scale).pow(2) * 4;
            let frame = decode_frame_data(&case["rgba_zlib"], &stages[1]["readback_sha256"], size);
            assert!(frame.as_chunks::<4>().0.iter().all(|pixel| pixel[3] == 255));
        }
    }

    #[test]
    fn native_object_list_controls_retain_order_and_frame_handoff() {
        use std::collections::BTreeSet;

        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/magicpen-phase-export-host.json"
        ))
        .unwrap();
        let captures = &fixture["object_list_capture_original"];
        let baseline = fixture["two_stroke_controls"].as_array().unwrap();
        let mut seen = BTreeSet::new();
        for key in ["solid_template", "background_template"] {
            let text = captures[key].as_str().unwrap();
            for block in text.split("LIST_METADATA ").skip(1) {
                let mut lines = block.lines();
                let metadata: Value = serde_json::from_str(lines.next().unwrap()).unwrap();
                let background = metadata["background"].as_str().unwrap();
                let scale = metadata["scale"].as_u64().unwrap() as usize;
                assert!(["solid", "pattern", "transparent"].contains(&background));
                assert!([1, 2, 4].contains(&scale));
                assert!(seen.insert((background.to_owned(), scale)));
                assert_eq!(metadata["cases"], serde_json::json!(["red", "blue"]));
                let lines: Vec<_> = lines.collect();
                let line = |prefix: &str| {
                    let mut found = lines.iter().filter_map(|line| line.strip_prefix(prefix));
                    let value = found.next().expect("missing object-list trace line");
                    assert!(found.next().is_none(), "duplicate object-list trace line");
                    value
                };
                assert_eq!(line("LIST_RESULT "), "0x1");
                assert_eq!(line("exit "), "77"); // Intentional probe termination.
                let fields: Vec<_> = line("LIST_PIXELS ").split(' ').collect();
                assert_eq!(fields.len(), 4);
                assert_eq!(fields[0], "2");
                assert_eq!(fields[2], "0");
                let frame_data = if background == "transparent" {
                    Value::String(fields[3].to_owned())
                } else {
                    let mut matches = baseline
                        .iter()
                        .filter(|case| case["background"] == background && case["scale"] == scale);
                    let prior = matches.next().expect("missing sequential frame");
                    assert!(matches.next().is_none(), "duplicate sequential frame");
                    assert_eq!(prior["stages"][1]["readback_sha256"], fields[1]);
                    assert_eq!(fields[3], format!("__SDOCX_FRAME_{background}_{scale}__"));
                    prior["rgba_zlib"].clone()
                };
                let size = (32 * scale).pow(2) * 4;
                let frame = decode_frame_data(&frame_data, &Value::from(fields[1]), size);
                if background == "transparent" {
                    assert_eq!(&frame[..4], &[0, 0, 0, 0]);
                }
                let gl: Vec<Value> = serde_json::from_str(line("LIST_GL ")).unwrap();
                let statuses: Vec<_> = gl
                    .iter()
                    .filter(|call| call["call"] == "glCheckFramebufferStatus")
                    .collect();
                assert!(!statuses.is_empty());
                assert!(statuses.iter().all(|call| call["status"] == 36053));
                let draws: Vec<Value> = serde_json::from_str(line("LIST_DRAWS ")).unwrap();
                let draw_calls: Vec<_> = draws
                    .iter()
                    .map(|draw| draw["call"].as_str().unwrap())
                    .collect();
                assert_eq!(
                    draw_calls,
                    [
                        "glDrawArraysInstanced",
                        "glDrawArrays",
                        "glDrawArraysInstanced",
                        "glDrawArrays"
                    ]
                );
                let events: Vec<Value> = serde_json::from_str(line("LIST_EVENTS ")).unwrap();
                let dispatch: Vec<_> = events
                    .iter()
                    .filter_map(|event| {
                        let name = event["name"].as_str()?;
                        if name.contains("::drawObjectStroke(") {
                            Some("stroke")
                        } else if name.contains("::drawObject(") {
                            Some("object")
                        } else {
                            None
                        }
                    })
                    .collect();
                assert_eq!(dispatch, ["object", "stroke", "object", "stroke"]);
                let objects: Vec<_> = events
                    .iter()
                    .filter(|event| event["name"] == "GetData return")
                    .map(|event| event["value"].as_str().unwrap())
                    .collect();
                assert_eq!(objects.len(), 2);
                assert_ne!(objects[0], objects[1]);
            }
        }
        assert_eq!(seen.len(), 9);
    }

    fn float_records(hex: &str) -> Vec<[u32; 3]> {
        let bytes = hex.as_bytes();
        assert_eq!(bytes.len() % 24, 0);
        bytes
            .as_chunks::<24>()
            .0
            .iter()
            .map(|record| {
                std::array::from_fn(|index| {
                    let start = index * 8;
                    u32::from_str_radix(std::str::from_utf8(&record[start..start + 8]).unwrap(), 16)
                        .unwrap()
                        .swap_bytes()
                })
            })
            .collect()
    }

    #[test]
    fn diagnostic_contour_uses_frozen_native_v1_buffer_and_stays_bounded() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/magicpen-paint-controls-native.json"
        ))
        .unwrap();
        let control = fixture["controls"]
            .as_array()
            .unwrap()
            .iter()
            .find(|control| control["id"] == "identity-v1-transparent")
            .unwrap();
        let paint = &control["paint"];
        let records = float_records(paint["native_vector_hex"].as_str().unwrap());
        assert_eq!(records.len(), 8);
        assert_eq!(paint["native_vector_bytes"], 96);
        assert_eq!(
            paint["native_vector_sha256"].as_str(),
            Some("48fab498fb7b870d2fb0ebcf3d725efbcf1352324592286aa9082df635b1377d")
        );
        let path = source_contour_path(&records, 1).unwrap();
        assert_eq!(path.matches('M').count(), records.len());
        assert_eq!(path.matches('Z').count(), records.len());
        assert!(path.len() <= MAX_PATH_BYTES);

        let source: Value = serde_json::from_str(include_str!(
            "../../../../conformance/magicpen-glv1-source-native.json"
        ))
        .unwrap();
        let baseline = source["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|case| case["id"] == "baseline")
            .unwrap();
        let native = baseline["native"]["xyz_f32_u32_hex"]
            .as_array()
            .unwrap()
            .as_chunks::<3>()
            .0
            .iter()
            .map(|xyz| {
                xyz.iter()
                    .map(|word| u32::from_str_radix(word.as_str().unwrap(), 16).unwrap())
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap()
            })
            .collect::<Vec<[u32; 3]>>();
        assert_eq!(source_contour_path(&native, 1), Some(path));

        assert!(source_contour_path(&[], 1).is_none());
        assert!(source_contour_path(&[[0, 0, f32::NAN.to_bits()]], 1).is_none());
        assert!(source_contour_path(&[[0, 0, (-1.0_f32).to_bits()]], 1).is_none());
    }
}
