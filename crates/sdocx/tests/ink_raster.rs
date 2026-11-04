#![cfg(all(feature = "render", feature = "serde"))]

use sdocx::{
    BoundingBox, Color, InkRasterCanvas, InkRasterContext, InkRasterError, InkRasterInverseScale,
    InkRasterOrigin, InkRasterProfile, Point, Stroke, StrokeRendering, StrokeStyle,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn stroke(case_id: &str, argb: u32) -> Stroke {
    let fixture: Value =
        serde_json::from_str(include_str!("../../../conformance/pencil2-v14-native.json")).unwrap();
    let input = &fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["id"] == case_id)
        .unwrap()["input"];
    let hex = input["object_stroke_size_f32_bits"].as_str().unwrap();
    let size = f32::from_le_bytes(std::array::from_fn(|i| {
        u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap()
    }));
    let mut style = serde_json::to_value(StrokeStyle::default()).unwrap();
    for (key, value) in [
        ("color_argb", json!(argb)),
        ("pen_size", json!(size)),
        ("size_level", json!(0)),
        ("stroke_type_raw", json!(0)),
        ("color_type_raw", json!(0)),
    ] {
        style[key] = value;
    }
    let rendering:StrokeRendering=serde_json::from_value(json!({
        "metadata":null,"pen_name":"com.samsung.android.sdk.pen.pen.preload.Pencil2","advanced_settings":"14;","tool_type_raw":2,
        "style":style,"properties":{"compressed":true,"stylus_channels":true,"replay_only":false,"eraser":false,"fixed_width":false,"millisecond_timestamps":false,"top_layer_pen":false,"alpha_lock":false,"binary_added":false,"generated":false,"fixed_opacity":false,"rainbow_effect":false,"straighten":false,"reveal_mode":false}
    })).unwrap();
    let channel = |name: &str| {
        input[name]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap())
            .collect()
    };
    Stroke {
        bbox: BoundingBox::default(),
        points: input["points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| Point {
                x: p[0].as_f64().unwrap(),
                y: p[1].as_f64().unwrap(),
            })
            .collect(),
        pressures: channel("pressure"),
        tilts: channel("tilt"),
        orientations: channel("orientation"),
        timestamps: input["timestamps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_i64().unwrap())
            .collect(),
        rendering: Some(rendering),
        color: Some(Color {
            r: (argb >> 16) as u8,
            g: (argb >> 8) as u8,
            b: argb as u8,
        }),
        pen_width: size,
    }
}

fn context(origin: InkRasterOrigin, scale: u32) -> InkRasterContext {
    InkRasterContext {
        width: 192,
        height: 192,
        pixels_per_unit: scale,
        origin,
    }
}

fn profile(inverse_scale: InkRasterInverseScale) -> InkRasterProfile {
    InkRasterProfile::Pencil2V14 { inverse_scale }
}

#[test]
fn pencil2_translation_bounds_and_zero_preserve_canvas_contract() {
    let stroke = stroke("pressure_ramp", 0xff000000);
    for origin in [InkRasterOrigin::TopLeft, InkRasterOrigin::BottomLeft] {
        for scale in [1, 2] {
            let prior: Vec<_> = (0..192 * 192)
                .map(|i| {
                    let v = (i % 256) as u8;
                    [v, v / 2, 0, 255]
                })
                .collect();
            let mut ordinary =
                InkRasterCanvas::from_pixels(context(origin, scale), prior.clone()).unwrap();
            let mut translated =
                InkRasterCanvas::from_pixels(context(origin, scale), prior.clone()).unwrap();
            ordinary
                .paint(&stroke, profile(InkRasterInverseScale::One))
                .unwrap();
            for invalid in [
                [f32::INFINITY, 0.],
                [0., f32::NAN],
                [16_385., 0.],
                [0., -16_385.],
            ] {
                assert!(matches!(
                    translated.paint_pencil2_translated(
                        &stroke,
                        InkRasterInverseScale::One,
                        invalid
                    ),
                    Err(InkRasterError::InvalidContext)
                ));
                assert_eq!(translated.pixels(), prior);
            }
            translated
                .paint_pencil2_translated(&stroke, InkRasterInverseScale::One, [0., 0.])
                .unwrap();
            assert_eq!(translated.pixels(), ordinary.pixels());
        }
    }
}

#[test]
fn unsupported_inputs_and_work_exhaustion_preserve_the_canvas() {
    let mut canvas =
        InkRasterCanvas::new(context(InkRasterOrigin::TopLeft, 1), [64, 32, 96, 128]).unwrap();
    let original = canvas.pixels().to_vec();
    let mut source = stroke("channel_baseline", 0x80336699);
    source.pressures.push(0.5);
    assert!(matches!(
        canvas.paint(&source, profile(InkRasterInverseScale::One)),
        Err(InkRasterError::InvalidStroke(_))
    ));
    assert_eq!(canvas.pixels(), original);
    source.pressures.pop();
    source.rendering.as_mut().unwrap().pen_name =
        Some("com.samsung.android.sdk.pen.pen.preload.WaterColorBrush".into());
    assert!(matches!(
        canvas.paint(&source, profile(InkRasterInverseScale::One)),
        Err(InkRasterError::UnsupportedStroke(_))
    ));
    assert_eq!(canvas.pixels(), original);
    source.rendering.as_mut().unwrap().pen_name =
        Some("com.samsung.android.sdk.pen.pen.preload.Pencil2".into());
    source.points.last_mut().unwrap().x = 100_000.;
    assert!(matches!(
        canvas.paint(&source, profile(InkRasterInverseScale::One)),
        Err(InkRasterError::WorkLimit)
    ));
    assert_eq!(canvas.pixels(), original);
    source = stroke("channel_baseline", 0x80336699);
    canvas
        .paint(&source, profile(InkRasterInverseScale::One))
        .unwrap();
    assert_ne!(canvas.pixels(), original);
}

#[test]
fn canvas_limits_cover_prior_pixels_and_cumulative_work() {
    assert!(matches!(
        InkRasterCanvas::new(
            InkRasterContext {
                width: u32::MAX,
                height: u32::MAX,
                ..context(InkRasterOrigin::TopLeft, 1)
            },
            [0; 4]
        ),
        Err(InkRasterError::InvalidContext)
    ));
    assert!(matches!(
        InkRasterCanvas::new(
            InkRasterContext {
                width: 8192,
                height: 1025,
                ..context(InkRasterOrigin::TopLeft, 1)
            },
            [0; 4]
        ),
        Err(InkRasterError::InvalidContext)
    ));
    assert!(matches!(
        InkRasterCanvas::new(context(InkRasterOrigin::TopLeft, 1), [255, 0, 0, 0]),
        Err(InkRasterError::InvalidBackdrop)
    ));
    assert!(matches!(
        InkRasterCanvas::from_pixels(context(InkRasterOrigin::TopLeft, 1), vec![[0; 4]; 10]),
        Err(InkRasterError::InvalidBackdrop)
    ));
    let mut canvas = InkRasterCanvas::new(
        InkRasterContext {
            width: 4096,
            height: 1024,
            ..context(InkRasterOrigin::TopLeft, 1)
        },
        [255; 4],
    )
    .unwrap();
    let source = stroke("channel_baseline", 0x80336699);
    let mut submitted = 0;
    loop {
        match canvas.paint(&source, profile(InkRasterInverseScale::One)) {
            Ok(()) => submitted += 1,
            Err(InkRasterError::WorkLimit) => break,
            Err(error) => panic!("unexpected paint failure: {error}"),
        }
        assert!(submitted <= 256);
    }
    assert!(submitted > 0 && submitted < 256);
    let before = canvas.pixels().to_vec();
    assert!(matches!(
        canvas.paint(&source, profile(InkRasterInverseScale::One)),
        Err(InkRasterError::WorkLimit)
    ));
    assert_eq!(canvas.pixels(), before);
}

#[test]
fn public_raster_page_round_trips_through_svg_preview() {
    let mut canvas = InkRasterCanvas::new(context(InkRasterOrigin::TopLeft, 1), [255; 4]).unwrap();
    canvas
        .paint(
            &stroke("channel_baseline", 0x80336699),
            profile(InkRasterInverseScale::One),
        )
        .unwrap();
    let page = canvas.rendered_page(7).unwrap();
    assert_eq!(page.source_page_index, 7);
    let tree = resvg::usvg::Tree::from_str(&page.svg, &Default::default()).unwrap();
    let mut pixels = resvg::tiny_skia::Pixmap::new(192, 192).unwrap();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixels.as_mut(),
    );
    assert_eq!(pixels.data().as_chunks::<4>().0, canvas.pixels());
    #[cfg(feature = "pdf")]
    {
        let mut options = sdocx::PdfOptions::default();
        options.dpi = 72.;
        let bytes = sdocx::render_svg_pages_pdf(&[page], &options).unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
    }
}

fn native(id: &str) -> (Vec<[u8; 4]>, [u8; 4]) {
    let mut receipt: Value = serde_json::from_str(include_str!(
        "../../../conformance/pencil2-v14-page-context-native.json"
    ))
    .unwrap();
    let old: Value = serde_json::from_str(include_str!(
        "../../../conformance/pencil2-v14-mixed-rgb-native.json"
    ))
    .unwrap();
    if receipt["results"][id].is_null() {
        let suffix = id.strip_prefix("colored_bottom_scale1").unwrap();
        receipt["results"][id] = old["results"][format!("colored{suffix}")].clone();
    }
    let frame = &receipt["results"][id];
    let background = std::array::from_fn(|i| frame["background"][i].as_u64().unwrap() as u8);
    let mut image = vec![background; 192 * 192];
    for row in frame["pixels"].as_array().unwrap() {
        let x = row[0].as_u64().unwrap() as usize;
        let y = row[1].as_u64().unwrap() as usize;
        image[(191 - y) * 192 + x] = std::array::from_fn(|i| row[i + 2].as_u64().unwrap() as u8);
    }
    let bytes: Vec<_> = image
        .as_chunks::<192>()
        .0
        .iter()
        .rev()
        .flatten()
        .flatten()
        .copied()
        .collect();
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        frame["sha256"].as_str().unwrap()
    );
    (image, background)
}

fn errors(image: &[[u8; 4]], expected: &[[u8; 4]]) -> (usize, u64, u8) {
    assert_eq!(image.len(), expected.len());
    image.iter().flatten().zip(expected.iter().flatten()).fold(
        (0, 0, 0),
        |(count, sum, maximum), (&a, &b)| {
            let error = a.abs_diff(b);
            (
                count + usize::from(error != 0),
                sum + u64::from(error),
                maximum.max(error),
            )
        },
    )
}

#[test]
fn public_raster_canvas_joins_saved_source_to_native_page_pixels() {
    for (case, name, inverse) in [
        ("channel_baseline", "colored", InkRasterInverseScale::One),
        (
            "v5_exact_combined",
            "inverse_half",
            InkRasterInverseScale::Half,
        ),
    ] {
        let stroke = stroke(
            case,
            if name == "colored" {
                0x80336699
            } else {
                0xff000000
            },
        );
        for origin in [InkRasterOrigin::TopLeft, InkRasterOrigin::BottomLeft] {
            for scale in [1, 2] {
                for suffix in ["", "_background", "_white", "_paper"] {
                    let axis = if origin == InkRasterOrigin::TopLeft {
                        "top"
                    } else {
                        "bottom"
                    };
                    let id = format!("{name}_{axis}_scale{scale}{suffix}");
                    let (expected, background) = native(&id);
                    let mut canvas =
                        InkRasterCanvas::new(context(origin, scale), background).unwrap();
                    canvas.paint(&stroke, profile(inverse)).unwrap();
                    let error = errors(canvas.pixels(), &expected);
                    let expected = match (name, origin, scale, suffix) {
                        ("colored", InkRasterOrigin::TopLeft, 2, "_background" | "_paper") => {
                            (1, 1, 1)
                        }
                        ("colored", InkRasterOrigin::TopLeft, 2, _) => (2, 2, 1),
                        _ => (0, 0, 0),
                    };
                    assert_eq!(error, expected, "{id}");
                }
            }
        }
    }
}

#[test]
fn pencil2_transparent_page_exports_native_frame() {
    let (expected, background) = native("colored_top_scale1");
    let mut canvas =
        InkRasterCanvas::new(context(InkRasterOrigin::TopLeft, 1), background).unwrap();
    canvas
        .paint(
            &stroke("channel_baseline", 0x80336699),
            profile(InkRasterInverseScale::One),
        )
        .unwrap();
    assert_eq!(canvas.pixels(), expected);
    let page = canvas.rendered_page(0).unwrap();
    let tree = resvg::usvg::Tree::from_str(&page.svg, &Default::default()).unwrap();
    let mut pixels = resvg::tiny_skia::Pixmap::new(192, 192).unwrap();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixels.as_mut(),
    );
    assert_eq!(pixels.data().as_chunks::<4>().0, expected);
}
