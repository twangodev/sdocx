use super::*;

fn producer() -> Value {
    serde_json::from_str(include_str!(
        "../../../../conformance/colored-pencil-v4-background-scale-native.json"
    ))
    .unwrap()
}

#[test]
fn display_scale_matches_native_drawing_and_queued_setter_bits() {
    let fixture = producer();
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 8);
    for case in cases {
        let display =
            std::array::from_fn::<_, 2, _>(|i| case["display"][i].as_u64().unwrap() as u32);
        let context =
            InkRasterDepthContext::from_colored_pencil_display(31, 17, display[0], display[1])
                .unwrap();
        assert_eq!(
            context.background_scale.to_bits(),
            bits(&case["background_scale_f32_bits"]).to_bits()
        );
        assert_eq!(case["drawing_and_selected_redraw_geometry_unchanged"], true);
        for phase in ["draw_object_stroke", "saved_redraw"] {
            let rows: Vec<_> = case["events"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|row| row["phase"] == phase)
                .collect();
            assert_eq!(rows.len(), 5);
            assert_eq!(rows[0]["dimensions"], case["display"]);
            assert_eq!(rows[1]["minimum"], display[0].min(display[1]));
            assert_eq!(
                bits(&rows[2]["f32_bits"]).to_bits(),
                context.background_scale.to_bits()
            );
            assert_eq!(rows[3]["s0_f32_bits"], rows[4]["stored_f32_bits"]);
            assert_eq!(rows[3]["s0_f32_bits"], case["background_scale_f32_bits"]);
            assert!(rows[3]["queue_depth"].as_u64().unwrap() > 0);
        }
    }
    for display in [[0, 1080], [1920, 0], [16_385, 1080], [1920, 16_385]] {
        assert!(matches!(
            InkRasterDepthContext::from_colored_pencil_display(31, 17, display[0], display[1]),
            Err(InkRasterError::InvalidContext)
        ));
    }
    assert!(matches!(
        InkRasterDepthContext::from_colored_pencil_display(0, 17, 1920, 1080),
        Err(InkRasterError::InvalidDepthMap)
    ));
}

#[test]
fn derived_display_depth_context_matches_native_paint_and_svg() {
    let geometry: Value = serde_json::from_str(include_str!(
        "../../../../conformance/colored-pencil-v4-source-context-native.json"
    ))
    .unwrap();
    let receipt: Value = serde_json::from_str(include_str!(
        "../../../../conformance/colored-pencil-v4-display-depth-native.json"
    ))
    .unwrap();
    let producer = producer();
    assert_eq!(receipt["cases"].as_object().unwrap().len(), 96);
    let map = &receipt["depth_inputs"]["grid"];
    let raw = map["rgba_hex"].as_str().unwrap();
    let bytes: Vec<u8> = (0..raw.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&raw[i..i + 2], 16).unwrap())
        .collect();
    let pixels = bytes.as_chunks::<4>().0;
    let mut counterexample_found = false;
    for (name, case) in receipt["cases"].as_object().unwrap() {
        let source = geometry["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["id"] == case["source"])
            .unwrap();
        let scale = producer["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["id"] == case["source"])
            .unwrap();
        assert_eq!(case["stream_sha256"], scale["geometry_stream_sha256"]);
        let display =
            std::array::from_fn::<_, 2, _>(|i| source["screen"][i].as_u64().unwrap() as u32);
        let depth = InkRasterDepthContext::from_colored_pencil_display(
            map["width"].as_u64().unwrap() as u32,
            map["height"].as_u64().unwrap() as u32,
            display[0],
            display[1],
        )
        .unwrap();
        assert_eq!(
            depth.background_scale.to_bits(),
            bits(&scale["background_scale_f32_bits"]).to_bits()
        );
        assert_eq!(
            depth.background_scale.to_bits(),
            (case["background_scale"].as_f64().unwrap() as f32).to_bits()
        );
        let stroke = source_input(&source["input"], case["argb"].as_u64().unwrap() as u32);
        let profile = InkRasterProfile::ColoredPencilV4Host {
            pen_data_width: display[0],
            pen_data_height: display[1],
        };
        let ctx = context(case);
        let prior = std::array::from_fn(|i| case["background"][i].as_u64().unwrap() as u8);
        let mut canvas = InkRasterCanvas::new(ctx, prior).unwrap();
        canvas
            .paint_with_depth_map(&stroke, profile, depth, pixels)
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
        #[cfg(feature = "pdf")]
        if prior == [255; 4] {
            let page = canvas.rendered_page(0).unwrap();
            let tree =
                resvg::usvg::Tree::from_str(&page.svg, &resvg::usvg::Options::default()).unwrap();
            let mut image = resvg::tiny_skia::Pixmap::new(ctx.width, ctx.height).unwrap();
            resvg::render(
                &tree,
                resvg::tiny_skia::Transform::identity(),
                &mut image.as_mut(),
            );
            assert_eq!(image.data(), canvas.pixels().as_flattened());
        }

        if case["source"] == "screen_800x600"
            && case["origin"] == "bottom"
            && ctx.pixels_per_unit == 1
            && prior == [255; 4]
        {
            let mut wrong = InkRasterCanvas::new(ctx, prior).unwrap();
            wrong
                .paint_with_depth_map(
                    &stroke,
                    profile,
                    InkRasterDepthContext {
                        background_scale: 0.75,
                        ..depth
                    },
                    pixels,
                )
                .unwrap();
            let differences: Vec<_> = wrong
                .pixels()
                .as_flattened()
                .iter()
                .zip(expected.as_flattened())
                .map(|(a, b)| a.abs_diff(*b))
                .collect();
            let count = differences.iter().filter(|&&d| d != 0).count();
            assert!(
                count > 0,
                "fixed 0.75 must not replace the native display-scale producer"
            );
            counterexample_found = true;
        }
    }
    assert!(counterexample_found);
}
