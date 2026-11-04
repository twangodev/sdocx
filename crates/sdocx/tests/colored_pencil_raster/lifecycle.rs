use super::*;
use std::collections::BTreeMap;

#[test]
fn geometry_shell_depth_handoff_preserves_cached_gl_state() {
    let receipt: Value = serde_json::from_str(include_str!(
        "../assets/colored-pencil/colored-pencil-v4-depth-lifecycle-native.json"
    ))
    .unwrap();
    assert_eq!(receipt["cached_pen_reused"], true);
    assert_eq!(receipt["cases"].as_array().unwrap().len(), 10);
    let mut drawing: Option<&str> = None;
    let mut gl: Option<&str> = None;
    for case in receipt["cases"].as_array().unwrap() {
        let argument = case["argument"].as_str();
        if case["target"] == "drawing" {
            drawing = argument;
            if drawing.is_some() {
                gl = drawing.filter(|&name| name != "no_texture");
            }
        } else if case["target"] == "gl" {
            gl = argument;
        }
        assert_eq!(case["after_redraw"]["drawing"].as_str(), drawing);
        assert_eq!(case["after_redraw"]["gl"].as_str(), gl);
        assert_eq!(case["after_redraw"]["rt"], Value::Null);
        let callbacks: Vec<_> = case["events"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["kind"] == "callback_set")
            .collect();
        assert_eq!(callbacks.len(), 1);
        assert_eq!(callbacks[0]["argument"].as_str(), gl);
        if drawing.is_none() {
            assert!(
                case["events"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|e| e["kind"] != "drawing_handoff")
            );
        }
        for name in ["A", "B", "no_texture"] {
            let expected = 1 + usize::from(gl == Some(name));
            assert_eq!(
                case["after_redraw"]["references"][name].as_u64().unwrap() as usize,
                expected
            );
        }
    }
}

#[test]
fn full_native_drawing_sequences_match_public_canvas_and_svg() {
    let receipt: Value = serde_json::from_str(include_str!(
        "../assets/colored-pencil/colored-pencil-v4-drawing-lifecycle-native.json"
    ))
    .unwrap();
    check_drawing_sequences(&receipt, 96, 12);
}

#[test]
fn native_mixed_pencil2_prior_and_cached_drawing_match_public_canvas() {
    let receipt: Value = serde_json::from_str(include_str!(
        "../assets/colored-pencil/colored-pencil-v4-pencil2-drawing-native.json"
    ))
    .unwrap();
    let pencil_bytes =
        include_bytes!("../../../../conformance/pencil2-v14-page-context-native.json");
    assert_eq!(
        format!("{:x}", Sha256::digest(pencil_bytes)),
        receipt["capture"]["pencil_receipt_sha256"]
            .as_str()
            .unwrap()
    );
    let pencil: Value = serde_json::from_slice(pencil_bytes).unwrap();
    assert_eq!(receipt["prior_inputs"].as_object().unwrap().len(), 4);
    for prior in receipt["prior_inputs"].as_object().unwrap().values() {
        assert_eq!(prior["placement"], json!([0, 64]));
        let original = &pencil["results"][prior["pencil_case"].as_str().unwrap()];
        let mut pixels = vec![[255; 4]; 256 * 256];
        let mut source = vec![[255; 4]; 192 * 192];
        for pixel in original["pixels"].as_array().unwrap() {
            let x = pixel[0].as_u64().unwrap() as usize;
            let y = pixel[1].as_u64().unwrap() as usize;
            source[y * 192 + x] = std::array::from_fn(|i| pixel[i + 2].as_u64().unwrap() as u8);
        }
        assert_eq!(
            format!("{:x}", Sha256::digest(source.as_flattened())),
            original["sha256"].as_str().unwrap()
        );
        for y in 0..192 {
            pixels[(y + 64) * 256..(y + 64) * 256 + 192]
                .copy_from_slice(&source[y * 192..(y + 1) * 192]);
        }
        pixels = pixels
            .as_chunks::<256>()
            .0
            .iter()
            .rev()
            .flatten()
            .copied()
            .collect();
        assert_eq!(native(prior), pixels);
    }
    let cases = &receipt["cases"];
    for scale in [1, 2] {
        for step in ["0_drawing_A", "1_drawing_null", "2_gl_null"] {
            let a = &cases[format!("top_{scale}_pencil_mixed_{step}")];
            let b = &cases[format!("top_{scale}_pencil_mixed_reversed_{step}")];
            assert_ne!(a["frame_sha256"], b["frame_sha256"]);
        }
    }
    check_drawing_sequences(&receipt, 12, 4);
}

fn check_drawing_sequences(receipt: &Value, frames: usize, groups: usize) {
    let geometry: Value = serde_json::from_str(include_str!(
        "../../../../conformance/colored-pencil-v4-source-context-native.json"
    ))
    .unwrap();
    assert_eq!(receipt["cases"].as_object().unwrap().len(), frames);
    let mut sequences = BTreeMap::<&str, Vec<(&str, &Value)>>::new();
    for (name, case) in receipt["cases"].as_object().unwrap() {
        sequences
            .entry(case["sequence"].as_str().unwrap())
            .or_default()
            .push((name, case));
    }
    assert_eq!(sequences.len(), groups);
    let mut counterexample = None;
    for cases in sequences.values_mut() {
        cases.sort_by_key(|(_, case)| case["step"].as_u64().unwrap());
        let first = cases[0].1;
        let ctx = context(first);
        let prior = std::array::from_fn(|i| first["background"][i].as_u64().unwrap() as u8);
        let mut canvas = if let Some(id) = first["prior_id"].as_str() {
            InkRasterCanvas::from_pixels(ctx, native(&receipt["prior_inputs"][id])).unwrap()
        } else {
            InkRasterCanvas::new(ctx, prior).unwrap()
        };
        let mut drawing: Option<&str> = None;
        let mut gl: Option<&str> = None;
        let mut rt: Option<&str> = None;
        let mut previous: Option<Value> = None;
        for (step, (name, case)) in cases.iter().enumerate() {
            assert_eq!(case["step"].as_u64().unwrap() as usize, step);
            assert_eq!(case["native_draw_calls"], 1);
            assert_eq!(case["prior_verified"], true);
            let argument = case["argument"].as_str();
            match case["target"].as_str().unwrap() {
                "drawing" => drawing = argument,
                "gl" => gl = argument,
                "rt" => {
                    if argument.is_some() {
                        rt = argument;
                    }
                }
                "clear" => {}
                _ => panic!("unrecognized native action"),
            }
            if drawing.is_some() {
                gl = drawing;
            }
            if gl.is_some() {
                rt = gl;
            }
            assert_eq!(case["drawing_depth"].as_str(), drawing);
            assert_eq!(case["gl_depth"].as_str(), gl);
            assert_eq!(case["effective_depth"].as_str(), rt);
            let source = geometry["cases"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["id"] == case["source"])
                .unwrap();
            assert_eq!(case["stream_sha256"], source["stream_sha256"]);
            let stroke = source_input(&source["input"], case["argb"].as_u64().unwrap() as u32);
            if step == 2
                && matches!(
                    case["sequence"].as_str(),
                    Some("top_1_white" | "top_1_pencil_mixed")
                )
            {
                let mut fresh =
                    InkRasterCanvas::from_pixels(ctx, canvas.pixels().to_vec()).unwrap();
                fresh.paint(&stroke, profile()).unwrap();
                counterexample = Some(((*name).to_owned(), fresh.pixels().to_vec()));
            }
            let map = &receipt["depth_inputs"][rt.unwrap()];
            let raw = map["rgba_hex"].as_str().unwrap();
            let bytes: Vec<u8> = (0..raw.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&raw[i..i + 2], 16).unwrap())
                .collect();
            let depth = InkRasterDepthContext::from_colored_pencil_display(
                map["width"].as_u64().unwrap() as u32,
                map["height"].as_u64().unwrap() as u32,
                1920,
                1080,
            )
            .unwrap();
            assert_eq!(
                (case["background_scale"].as_f64().unwrap() as f32).to_bits(),
                depth.background_scale.to_bits()
            );
            canvas
                .paint_with_depth_map(&stroke, profile(), depth, bytes.as_chunks::<4>().0)
                .unwrap();
            let expected = native(case);
            let errors: Vec<_> = canvas
                .pixels()
                .iter()
                .zip(&expected)
                .enumerate()
                .filter(|(_, (a, b))| a != b)
                .take(2)
                .collect();
            assert!(errors.is_empty(), "{name}: {errors:?}");
            if (5..=6).contains(&step) {
                assert_eq!(&case["frame_sha256"], previous.as_ref().unwrap());
            }
            previous = Some(case["frame_sha256"].clone());
            #[cfg(feature = "pdf")]
            if prior == [255; 4] {
                let page = canvas.rendered_page(0).unwrap();
                let tree = resvg::usvg::Tree::from_str(&page.svg, &resvg::usvg::Options::default())
                    .unwrap();
                let mut image = resvg::tiny_skia::Pixmap::new(ctx.width, ctx.height).unwrap();
                resvg::render(
                    &tree,
                    resvg::tiny_skia::Transform::identity(),
                    &mut image.as_mut(),
                );
                assert_eq!(image.data(), canvas.pixels().as_flattened());
            }
        }
    }
    let (name, fresh) = counterexample.unwrap();
    let expected = native(&receipt["cases"][&name]);
    let differences: Vec<_> = fresh
        .as_flattened()
        .iter()
        .zip(expected.as_flattened())
        .map(|(a, b)| a.abs_diff(*b))
        .collect();
    let count = differences.iter().filter(|&&d| d != 0).count();
    assert!(
        count > 0,
        "a fresh null-depth submission must differ from native retained depth"
    );
}
