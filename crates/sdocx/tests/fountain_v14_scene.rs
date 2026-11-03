#![cfg(all(feature = "render", feature = "serde"))]

use base64::Engine;
use flate2::read::ZlibDecoder;
use sdocx::{InkRasterCanvas, InkRasterContext, InkRasterOrigin, InkRasterProfile, Stroke};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::Read;

fn pixels(value: &Value) -> Vec<[u8; 4]> {
    let compressed = base64::engine::general_purpose::STANDARD
        .decode(value["rgba_zlib_base64"].as_str().unwrap())
        .unwrap();
    let mut bytes = Vec::new();
    ZlibDecoder::new(compressed.as_slice())
        .read_to_end(&mut bytes)
        .unwrap();
    assert_eq!(bytes.len(), 128 * 256 * 4);
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        value["sha256"].as_str().unwrap()
    );
    bytes
        .as_chunks::<512>()
        .0
        .iter()
        .rev()
        .flatten()
        .copied()
        .collect::<Vec<_>>()
        .as_chunks::<4>()
        .0
        .to_vec()
}

#[test]
#[ignore = "diagnostic: native drawable direction can depend on preceding strokes"]
fn saved_scene_exposes_short_stroke_counterexample() {
    let receipt: Value = serde_json::from_str(include_str!(
        "assets/fountain-v14/scene-native.json"
    ))
    .unwrap();
    let inputs: Value = serde_json::from_str(include_str!(
        "../../../conformance/fountain-v14-saved-scene-inputs.json"
    ))
    .unwrap();
    let sources = inputs["strokes"].as_array().unwrap();
    for origin in ["top"] {
        for scale in [1] {
            for prior in ["clear"] {
                let context = InkRasterContext {
                    width: 128,
                    height: 256,
                    pixels_per_unit: scale,
                    origin: if origin == "top" {
                        InkRasterOrigin::TopLeft
                    } else {
                        InkRasterOrigin::BottomLeft
                    },
                };
                let mut canvas =
                    InkRasterCanvas::from_pixels(context, pixels(&receipt["prior_inputs"][prior]))
                        .unwrap();
                for (step, source) in [3, 6, 7, 18, 19, 28, 29, 30].into_iter().enumerate() {
                    let case = &receipt["cases"]
                        [format!("scene_{origin}_{scale}_{prior}_{step}_{source}")];
                    let input = sources.iter().find(|v| v["index"] == source).unwrap();
                    let stroke: Stroke = serde_json::from_value(input["stroke"].clone()).unwrap();
                    canvas
                        .paint(
                            &stroke,
                            InkRasterProfile::FountainPenV14DirectHost {
                                source_origin: serde_json::from_value(
                                    case["source_origin"].clone(),
                                )
                                .unwrap(),
                                paint_argb: receipt["model"][source.to_string()]["color_argb"]
                                    .as_u64()
                                    .unwrap() as u32,
                            },
                        )
                        .unwrap();
                    let expected = pixels(&case["output"]);
                    let differs = canvas.pixels().iter().zip(&expected).any(|(a, b)| a != b);
                    assert_eq!(differs, step == 7, "{origin} {scale} {prior} step {step}");
                }
            }
        }
    }
}

#[test]
fn saved_scene_receipts_preserve_ordered_native_frames() {
    let receipt: Value = serde_json::from_str(include_str!(
        "assets/fountain-v14/scene-native.json"
    ))
    .unwrap();
    let sequence = [3, 6, 7, 18, 19, 28, 29, 30, 6];
    assert_eq!(receipt["cases"].as_object().unwrap().len(), 108);
    for origin in ["top", "bottom"] {
        for scale in [1, 2] {
            for prior in ["clear", "white", "checker"] {
                let mut before = pixels(&receipt["prior_inputs"][prior]);
                for (step, source) in sequence.into_iter().enumerate() {
                    let case = &receipt["cases"]
                        [format!("scene_{origin}_{scale}_{prior}_{step}_{source}")];
                    assert_eq!(case["prefix"], serde_json::json!(&sequence[..=step]));
                    let output = pixels(&case["output"]);
                    if step > 0 {
                        let preceding = &receipt["cases"][format!(
                            "scene_{origin}_{scale}_{prior}_{}_{}",
                            step - 1,
                            sequence[step - 1]
                        )];
                        assert_eq!(case["before_sha256"], preceding["output"]["sha256"]);
                    } else {
                        assert_eq!(
                            case["before_sha256"],
                            receipt["prior_inputs"][prior]["sha256"]
                        );
                    }
                    if case["native_draws"] == 0 {
                        assert_eq!(output, before);
                    }
                    before = output;
                }
            }
        }
    }
}
