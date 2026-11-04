#![cfg(all(feature = "render", feature = "serde"))]

use sdocx::{
    BoundingBox, Color, InkRasterCanvas, InkRasterContext, InkRasterError, InkRasterOrigin,
    InkRasterProfile, Point, Stroke, StrokeRendering, StrokeStyle,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn stroke(version: u32, argb: u32) -> Stroke {
    let fixture: Value =
        serde_json::from_str(include_str!("../../../conformance/watercolor-native.json")).unwrap();
    let input = &fixture["cases"][0]["input"];
    let mut style = serde_json::to_value(StrokeStyle::default()).unwrap();
    style["color_argb"] = json!(argb);
    style["pen_size"] = json!(8.);
    let rendering:StrokeRendering=serde_json::from_value(json!({
        "metadata":null,"pen_name":"com.samsung.android.sdk.pen.pen.preload.WaterColorBrush","advanced_settings":if version==1{Value::Null}else{json!("")},"tool_type_raw":2,"style":style,
        "properties":{"compressed":true,"stylus_channels":true,"replay_only":false,"eraser":false,"fixed_width":false,"millisecond_timestamps":false,"top_layer_pen":false,"alpha_lock":false,"binary_added":false,"generated":false,"fixed_opacity":false,"rainbow_effect":false,"straighten":false,"reveal_mode":false}
    })).unwrap();
    Stroke {
        bbox: BoundingBox::default(),
        points: input["points_xy"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| Point {
                x: p[0].as_f64().unwrap(),
                y: p[1].as_f64().unwrap(),
            })
            .collect(),
        pressures: input["pressure"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p.as_f64().unwrap())
            .collect(),
        tilts: Vec::new(),
        orientations: Vec::new(),
        timestamps: input["timestamps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p.as_i64().unwrap())
            .collect(),
        pen_width: 8.,
        color: Some(Color {
            r: (argb >> 16) as u8,
            g: (argb >> 8) as u8,
            b: argb as u8,
        }),
        rendering: Some(rendering),
    }
}

fn profile() -> InkRasterProfile {
    InkRasterProfile::WaterColorHostRtv1 {
        screen_width: 1920,
        screen_height: 1080,
    }
}

fn expected(case: &Value) -> Vec<[u8; 4]> {
    let [width, height] = [
        case["frame"][0].as_u64().unwrap() as usize,
        case["frame"][1].as_u64().unwrap() as usize,
    ];
    let prior = std::array::from_fn(|c| case["background"][c].as_u64().unwrap() as u8);
    let mut pixels = vec![prior; width * height];
    let packed = case["final_sparse_hex"].as_str().unwrap().as_bytes();
    assert!(packed.len().is_multiple_of(16));
    let byte = |h: &[u8]| u8::from_str_radix(std::str::from_utf8(h).unwrap(), 16).unwrap();
    for record in packed.as_chunks::<16>().0 {
        let index =
            u32::from_le_bytes(std::array::from_fn(|i| byte(&record[i * 2..i * 2 + 2]))) as usize;
        pixels[index] = std::array::from_fn(|c| byte(&record[8 + c * 2..10 + c * 2]));
    }
    let raw: Vec<_> = pixels.iter().flatten().copied().collect();
    assert_eq!(
        format!("{:x}", Sha256::digest(&raw)),
        case["final_sha256"].as_str().unwrap()
    );
    pixels
        .chunks_exact(width)
        .rev()
        .flatten()
        .copied()
        .collect()
}

#[cfg(feature = "pdf")]
fn svg_pixels(page: &sdocx::RenderedPage) -> Vec<[u8; 4]> {
    let tree = resvg::usvg::Tree::from_str(&page.svg, &resvg::usvg::Options::default()).unwrap();
    let size = tree.size().to_int_size();
    let mut image = resvg::tiny_skia::Pixmap::new(size.width(), size.height()).unwrap();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut image.as_mut(),
    );
    image.data().as_chunks::<4>().0.to_vec()
}

#[test]
fn public_source_profiles_match_native_rgb_and_svg_at_both_scales() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../conformance/watercolor-public-source-native.json"
    ))
    .unwrap();
    assert_eq!(fixture["cases"].as_object().unwrap().len(), 8);
    for (name, case) in fixture["cases"].as_object().unwrap() {
        let scale = case["scale"].as_u64().unwrap() as u32;
        let context = InkRasterContext {
            width: 384 * scale,
            height: 320 * scale,
            pixels_per_unit: scale,
            origin: InkRasterOrigin::TopLeft,
        };
        let mut canvas = InkRasterCanvas::new(context, [255; 4]).unwrap();
        canvas
            .paint(
                &stroke(
                    case["version"].as_u64().unwrap() as u32,
                    case["argb"].as_u64().unwrap() as u32,
                ),
                profile(),
            )
            .unwrap();
        assert!(
            canvas.pixels() == expected(case),
            "{name}: native pixels differ"
        );
        let page = canvas.rendered_page(0).unwrap();
        #[cfg(feature = "pdf")]
        assert!(
            svg_pixels(&page) == canvas.pixels(),
            "{name}: intrinsic SVG pixels differ"
        );
    }
}

#[test]
fn invalid_source_context_and_exhausted_work_preserve_the_canvas() {
    let context = InkRasterContext {
        width: 384,
        height: 320,
        pixels_per_unit: 1,
        origin: InkRasterOrigin::TopLeft,
    };
    let mut canvas = InkRasterCanvas::new(context, [255; 4]).unwrap();
    let valid = stroke(1, 0x8044aaee);
    let mut invalid = valid.clone();
    invalid.rendering.as_mut().unwrap().advanced_settings = Some("1;".into());
    for _ in 0..300 {
        assert!(matches!(
            canvas.paint(&invalid, profile()),
            Err(InkRasterError::UnsupportedStroke(_))
        ));
    }
    assert!(matches!(
        canvas.paint(
            &valid,
            InkRasterProfile::WaterColorHostRtv1 {
                screen_width: 0,
                screen_height: 1080
            }
        ),
        Err(InkRasterError::InvalidContext)
    ));
    invalid = valid.clone();
    invalid.pressures[0] = f64::NAN;
    assert!(matches!(
        canvas.paint(&invalid, profile()),
        Err(InkRasterError::InvalidStroke(_))
    ));
    assert!(canvas.pixels().iter().all(|p| *p == [255; 4]));
    let mut closed = valid.clone();
    closed.points = vec![
        Point { x: 0., y: 0. },
        Point { x: 2., y: 0. },
        Point { x: 0., y: 0. },
    ];
    closed.pressures = vec![1.; 3];
    closed.timestamps = vec![0, 8, 16];
    assert!(matches!(
        canvas.paint(&closed, profile()),
        Err(InkRasterError::UnsupportedStroke(_))
    ));
    canvas.paint(&valid, profile()).unwrap();
    let mut reference = InkRasterCanvas::new(context, [255; 4]).unwrap();
    reference.paint(&valid, profile()).unwrap();
    assert_eq!(canvas.pixels(), reference.pixels());
    let mut invalid = valid.clone();
    invalid.points[3].x = 100_000.;
    let before = canvas.pixels().to_vec();
    assert!(matches!(
        canvas.paint(&invalid, profile()),
        Err(InkRasterError::WorkLimit)
    ));
    assert_eq!(canvas.pixels(), before);
    let mut heavy = InkRasterCanvas::new(
        InkRasterContext {
            width: 1024,
            height: 1024,
            ..context
        },
        [255; 4],
    )
    .unwrap();
    let mut succeeded = 0;
    loop {
        let before = heavy.pixels().to_vec();
        match heavy.paint(&valid, profile()) {
            Ok(()) => {
                succeeded += 1;
                assert!(succeeded < 40);
            }
            Err(InkRasterError::WorkLimit) => {
                assert_eq!(heavy.pixels(), before);
                break;
            }
            Err(error) => panic!("unexpected paint error: {error}"),
        }
    }
    assert!(succeeded > 0);
}

fn pencil_stroke() -> Stroke {
    let fixture: Value =
        serde_json::from_str(include_str!("../../../conformance/pencil2-v14-native.json")).unwrap();
    let input = &fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "channel_baseline")
        .unwrap()["input"];
    let mut source = stroke(1, 0x80336699);
    let size_bits = input["object_stroke_size_f32_bits"].as_str().unwrap();
    let size = f32::from_le_bytes(std::array::from_fn(|i| {
        u8::from_str_radix(&size_bits[i * 2..i * 2 + 2], 16).unwrap()
    }));
    source.pen_width = size;
    let rendering = source.rendering.as_mut().unwrap();
    rendering.pen_name = Some("com.samsung.android.sdk.pen.pen.preload.Pencil2".into());
    rendering.advanced_settings = Some("14;".into());
    rendering.style.pen_size = Some(size);
    source.points = input["points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| Point {
            x: p[0].as_f64().unwrap(),
            y: p[1].as_f64().unwrap(),
        })
        .collect();
    let channel = |key: &str| {
        input[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap())
            .collect()
    };
    source.pressures = channel("pressure");
    source.tilts = channel("tilt");
    source.orientations = channel("orientation");
    source.timestamps = input["timestamps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t.as_i64().unwrap())
        .collect();
    source
}

#[test]
fn watercolor_continuation_matches_native_pixel_handoff_and_pencil_precision_limit() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../conformance/watercolor-pencil2-continuation-native.json"
    ))
    .unwrap();
    let pencil: Value = serde_json::from_str(include_str!(
        "../../../conformance/pencil2-v14-page-context-native.json"
    ))
    .unwrap();
    for (key, bytes) in [
        (
            "pencil_receipt_sha256",
            include_bytes!("../../../conformance/pencil2-v14-page-context-native.json").as_slice(),
        ),
        (
            "script_sha256",
            include_bytes!("../../../tools/watercolor_host/mixed_capture.py").as_slice(),
        ),
        (
            "frame_helper_sha256",
            include_bytes!("../../../tools/watercolor_host/frame_capture.py").as_slice(),
        ),
    ] {
        assert_eq!(
            fixture["capture"][key].as_str().unwrap(),
            format!("{:x}", Sha256::digest(bytes))
        );
    }
    assert_eq!(fixture["cases"].as_object().unwrap().len(), 2);
    for scale in [1, 2] {
        let context = InkRasterContext {
            width: 192,
            height: 192,
            pixels_per_unit: scale,
            origin: InkRasterOrigin::TopLeft,
        };
        let mut case = fixture["cases"][scale.to_string()].clone();
        case["frame"] = json!([192, 192]);
        case["background"] = json!([255, 255, 255, 255]);
        let target = expected(&case);
        let p = &pencil["results"][format!("colored_top_scale{scale}_white")];
        let mut prior = vec![[255; 4]; 192 * 192];
        for pixel in p["pixels"].as_array().unwrap() {
            let x = pixel[0].as_u64().unwrap() as usize;
            let y = pixel[1].as_u64().unwrap() as usize;
            prior[(191 - y) * 192 + x] =
                std::array::from_fn(|c| pixel[c + 2].as_u64().unwrap() as u8);
        }
        let bottom_origin: Vec<u8> = prior
            .as_chunks::<192>()
            .0
            .iter()
            .rev()
            .flatten()
            .flatten()
            .copied()
            .collect();
        assert_eq!(
            format!("{:x}", Sha256::digest(&bottom_origin)),
            case["prior_sha256"].as_str().unwrap()
        );
        let water = stroke(1, 0x8044aaee);
        let mut seeded = InkRasterCanvas::from_pixels(context, prior).unwrap();
        seeded.paint(&water, profile()).unwrap();
        assert!(
            seeded.pixels() == target,
            "scale {scale}: native-prior WaterColor differs"
        );
        let mut joined = InkRasterCanvas::new(context, [255; 4]).unwrap();
        joined
            .paint(
                &pencil_stroke(),
                InkRasterProfile::Pencil2V14 {
                    inverse_scale: sdocx::InkRasterInverseScale::One,
                },
            )
            .unwrap();
        joined.paint(&water, profile()).unwrap();
        let differences: Vec<_> = joined
            .pixels()
            .iter()
            .zip(&target)
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, (a, b))| (i % 192, i / 192, *a, *b))
            .collect();
        let known = if scale == 1 {
            Vec::new()
        } else {
            vec![(11, 1, [241, 244, 249, 255], [240, 243, 249, 255])]
        };
        assert_eq!(differences, known, "mixed scale {scale}");
        #[cfg(feature = "pdf")]
        assert!(
            svg_pixels(&joined.rendered_page(0).unwrap()) == joined.pixels(),
            "scale {scale}: mixed SVG transport differs"
        );
    }
}

#[test]
fn translucent_source_profiles_match_native_canvas_and_svg() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../conformance/watercolor-translucent-source-native.json"
    ))
    .unwrap();
    assert_eq!(
        fixture["capture"]["public_source_fixture_sha256"]
            .as_str()
            .unwrap(),
        format!(
            "{:x}",
            Sha256::digest(include_bytes!(
                "../../../conformance/watercolor-native.json"
            ))
        )
    );
    assert_eq!(fixture["cases"].as_object().unwrap().len(), 8);
    for (name, case) in fixture["cases"].as_object().unwrap() {
        let scale = case["scale"].as_u64().unwrap() as u32;
        let context = InkRasterContext {
            width: 384 * scale,
            height: 320 * scale,
            pixels_per_unit: scale,
            origin: InkRasterOrigin::TopLeft,
        };
        let prior = std::array::from_fn(|c| case["background"][c].as_u64().unwrap() as u8);
        let mut canvas = InkRasterCanvas::new(context, prior).unwrap();
        canvas
            .paint(
                &stroke(
                    case["version"].as_u64().unwrap() as u32,
                    case["argb"].as_u64().unwrap() as u32,
                ),
                profile(),
            )
            .unwrap();
        assert_eq!(
            canvas.pixels(),
            expected(case),
            "{name}: native pixels differ"
        );
        #[cfg(feature = "pdf")]
        assert_eq!(
            svg_pixels(&canvas.rendered_page(0).unwrap()),
            canvas.pixels(),
            "{name}: SVG pixels differ"
        );
    }
}

#[test]
fn scale_change_preserves_prior_paint_and_matches_fresh_context() {
    for (stroke, profile) in [
        (stroke(1, 0x80336699), profile()),
        (
            pencil_stroke(),
            InkRasterProfile::Pencil2V14 {
                inverse_scale: sdocx::InkRasterInverseScale::One,
            },
        ),
    ] {
        for origin in [InkRasterOrigin::TopLeft, InkRasterOrigin::BottomLeft] {
            let mut canvas = InkRasterCanvas::new(
                InkRasterContext {
                    width: 256,
                    height: 256,
                    pixels_per_unit: 1,
                    origin,
                },
                [30, 20, 10, 128],
            )
            .unwrap();
            canvas.paint(&stroke, profile).unwrap();
            for scale in [2, 1] {
                let prior = canvas.pixels().to_vec();
                canvas.set_pixels_per_unit(scale).unwrap();
                assert_eq!(canvas.pixels(), prior);
                let mut fresh =
                    InkRasterCanvas::from_pixels(canvas.context(), prior.clone()).unwrap();
                canvas.paint(&stroke, profile).unwrap();
                fresh.paint(&stroke, profile).unwrap();
                assert_eq!(canvas.pixels(), fresh.pixels());
                assert_ne!(canvas.pixels(), prior);
            }
        }
    }
}
