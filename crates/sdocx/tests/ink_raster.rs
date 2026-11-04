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
    #[cfg(feature = "pdf")]
    {
        let mut options = sdocx::PdfOptions::default();
        options.dpi = 72.;
        sdocx::render_svg_pages_pdf(&[page], &options).unwrap();
        canvas.to_pdf(&options).unwrap();
    }
}

fn native_frame_from_rows(prior: &[[u8; 4]], receipt: &Value, hash_key: &str) -> Vec<u8> {
    let mut expected: Vec<u8> = prior.iter().flatten().copied().collect();
    let mut delta = Vec::new();
    let mut previous = None;
    for row in receipt["delta_rows"].as_array().unwrap() {
        let row = row.as_array().unwrap();
        assert_eq!(row.len(), 5);
        let index = u32::try_from(row[0].as_u64().unwrap()).unwrap();
        assert!((index as usize) < prior.len());
        assert!(previous.is_none_or(|last| index > last));
        previous = Some(index);
        delta.extend_from_slice(&index.to_le_bytes());
        for (i, channel) in row[1..].iter().enumerate() {
            let channel = u8::try_from(channel.as_u64().unwrap()).unwrap();
            delta.push(channel);
            expected[index as usize * 4 + i] = channel;
        }
    }
    assert_eq!(
        format!("{:x}", Sha256::digest(&delta)),
        receipt["historical_delta_sha256"]
    );
    assert_eq!(expected.len(), 192 * 192 * 4);
    assert_eq!(
        format!("{:x}", Sha256::digest(&expected)),
        receipt[hash_key]
    );
    expected
}

#[test]
fn pencil2_translation_matrix_and_nonuniform_prior_match_host() {
    let prior: Vec<[u8; 4]> = (0..192 * 192)
        .map(|i| {
            let (x, y) = (i % 192, i / 192);
            [
                (x * 3 + y) as u8,
                (x + y * 5) as u8,
                (x * 7 + y * 3) as u8,
                255,
            ]
        })
        .collect();
    let receipt: Value = serde_json::from_str(include_str!(
        "assets/pencil2-translation/translation-matrix-receipt.json"
    ))
    .unwrap();
    for (name, origin, scale, dy) in [
        ("bottom1", InkRasterOrigin::BottomLeft, 1, 9.),
        ("bottom2", InkRasterOrigin::BottomLeft, 2, 9.),
        ("top2", InkRasterOrigin::TopLeft, 2, -7.),
    ] {
        let expected = native_frame_from_rows(&prior, &receipt["results"][name], "native_sha256");
        let mut canvas =
            InkRasterCanvas::from_pixels(context(origin, scale), prior.clone()).unwrap();
        canvas
            .paint_pencil2_translated(
                &stroke("pressure_ramp", 0xff000000),
                InkRasterInverseScale::One,
                [6., dy],
            )
            .unwrap();
        let actual: Vec<u8> = canvas.pixels().iter().flatten().copied().collect();
        assert_eq!(actual.len(), expected.len(), "native frame length {name}");
        let errors: Vec<_> = actual
            .iter()
            .zip(&expected)
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .collect();
        assert!(
            errors.is_empty(),
            "{name}: {} differing channels, first {:?}",
            errors.len(),
            errors.first()
        );
        let page = canvas.rendered_page(0).unwrap();
        let tree = resvg::usvg::Tree::from_str(&page.svg, &Default::default()).unwrap();
        let mut image = resvg::tiny_skia::Pixmap::new(192, 192).unwrap();
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::identity(),
            &mut image.as_mut(),
        );
        assert_eq!(image.data(), expected, "SVG {name}");
        #[cfg(feature = "pdf")]
        {
            let mut options = sdocx::PdfOptions::default();
            options.dpi = 72.;
            sdocx::render_svg_pages_pdf(&[page], &options).unwrap();
            canvas.to_pdf(&options).unwrap();
        }
    }
}

#[test]
fn pencil2_colored_tilted_top2_translucent_overlap_matches_host() {
    let prior: Vec<[u8; 4]> = (0..192 * 192)
        .map(|i| {
            let (x, y) = (i % 192, i / 192);
            [
                ((x * 3 + y) as u8) / 2,
                ((x + y * 5) as u8) / 2,
                ((x * 7 + y * 3) as u8) / 2,
                128,
            ]
        })
        .collect();
    let mut expected: Vec<u8> = prior.iter().flatten().copied().collect();
    let receipt: Value = serde_json::from_str(include_str!(
        "assets/pencil2-overlap/colored-top2-translucent-receipt.json"
    ))
    .unwrap();
    let mut delta = Vec::new();
    let rows = receipt["delta_rows"].as_array().unwrap();
    assert_eq!(
        rows.len(),
        receipt["native_changed_pixels"].as_u64().unwrap() as usize
    );
    let mut previous = None;
    for row in rows {
        let row = row.as_array().unwrap();
        assert_eq!(row.len(), 5);
        let index = u32::try_from(row[0].as_u64().unwrap()).unwrap();
        assert!((index as usize) < prior.len());
        assert!(previous.is_none_or(|last| index > last));
        previous = Some(index);
        delta.extend_from_slice(&index.to_le_bytes());
        for (i, channel) in row[1..].iter().enumerate() {
            let channel = u8::try_from(channel.as_u64().unwrap()).unwrap();
            delta.push(channel);
            expected[index as usize * 4 + i] = channel;
        }
    }
    assert_eq!(
        format!("{:x}", Sha256::digest(&delta)),
        receipt["delta_sha256"]
    );
    assert_eq!(
        format!("{:x}", Sha256::digest(&expected)),
        receipt["native_rgba_sha256"].as_str().unwrap()
    );
    let mut canvas =
        InkRasterCanvas::from_pixels(context(InkRasterOrigin::TopLeft, 2), prior).unwrap();
    for (case, translation, argb, tilt) in [
        ("pressure_ramp", [6., -7.], 0x80336699, 0.4),
        ("channel_baseline", [9., -9.], 0xffdd4411, 0.8),
    ] {
        let mut input = stroke(case, argb);
        input.tilts.fill(tilt);
        canvas
            .paint_pencil2_translated(&input, InkRasterInverseScale::One, translation)
            .unwrap();
    }
    let actual: Vec<u8> = canvas.pixels().iter().flatten().copied().collect();
    assert_eq!(actual.len(), expected.len());
    let differences: Vec<_> = actual
        .iter()
        .zip(&expected)
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .collect();
    assert!(
        differences.is_empty(),
        "{} differences: {:?}",
        differences.len(),
        differences.first()
    );
    let page = canvas.rendered_page(0).unwrap();
    let tree = resvg::usvg::Tree::from_str(&page.svg, &Default::default()).unwrap();
    let mut image = resvg::tiny_skia::Pixmap::new(192, 192).unwrap();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut image.as_mut(),
    );
    assert_eq!(image.data(), expected);
    #[cfg(feature = "pdf")]
    {
        let mut options = sdocx::PdfOptions::default();
        options.dpi = 72.;
        sdocx::render_svg_pages_pdf(&[page], &options).unwrap();
        canvas.to_pdf(&options).unwrap();
    }
}

#[test]
fn pencil2_translated_overlapping_strokes_match_host_and_svg() {
    let prior: Vec<[u8; 4]> = (0..192 * 192)
        .map(|i| {
            let (x, y) = (i % 192, i / 192);
            [
                (x * 3 + y) as u8,
                (x + y * 5) as u8,
                (x * 7 + y * 3) as u8,
                255,
            ]
        })
        .collect();
    let receipt: Value = serde_json::from_str(include_str!(
        "assets/pencil2-overlap-black/overlap-receipt.json"
    ))
    .unwrap();
    let expected = native_frame_from_rows(&prior, &receipt, "native_rgba_sha256");
    let mut canvas =
        InkRasterCanvas::from_pixels(context(InkRasterOrigin::BottomLeft, 1), prior).unwrap();
    for (case, translation) in [("pressure_ramp", [6., 9.]), ("channel_baseline", [9., 11.])] {
        canvas
            .paint_pencil2_translated(
                &stroke(case, 0xff000000),
                InkRasterInverseScale::One,
                translation,
            )
            .unwrap();
    }
    assert_eq!(
        canvas
            .pixels()
            .iter()
            .flatten()
            .copied()
            .collect::<Vec<_>>(),
        expected
    );
    let page = canvas.rendered_page(0).unwrap();
    let tree = resvg::usvg::Tree::from_str(&page.svg, &Default::default()).unwrap();
    let mut image = resvg::tiny_skia::Pixmap::new(192, 192).unwrap();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut image.as_mut(),
    );
    assert_eq!(image.data(), expected);
    #[cfg(feature = "pdf")]
    {
        let mut options = sdocx::PdfOptions::default();
        options.dpi = 72.;
        sdocx::render_svg_pages_pdf(&[page], &options).unwrap();
        canvas.to_pdf(&options).unwrap();
    }
}

#[test]
fn changed_input_native_grain_offset_and_canvas_match() {
    use std::io::Read;
    // Saved-derived coordinates with synthetic Pencil2 metadata, not a saved Pencil2 note.
    let data: Value = serde_json::from_str(include_str!(
        "../../../conformance/fountain-v14-saved-scene-inputs.json"
    ))
    .unwrap();
    let receipt: Value = serde_json::from_str(include_str!(
        "../../../tools/pencil2-control/native-receipt.json"
    ))
    .unwrap();
    let mut stroke: Stroke = serde_json::from_value(data["strokes"][1]["stroke"].clone()).unwrap();
    for point in &mut stroke.points {
        point.x += 3.25;
        point.y -= 2.5;
    }
    for pressure in &mut stroke.pressures {
        *pressure *= 0.75;
    }
    stroke.pen_width = f32::from_bits(0x4111de6a) as _;
    stroke.rendering.as_mut().unwrap().style.color_argb = Some(4294425088);
    stroke.rendering.as_mut().unwrap().pen_name =
        Some("com.samsung.android.sdk.pen.pen.preload.Pencil2".into());
    {
        let source_rendering = stroke.rendering.as_ref().unwrap();
        let mut style = serde_json::to_value(sdocx::StrokeStyle::default()).unwrap();
        style["pen_size"] = serde_json::json!(stroke.pen_width);
        style["color_argb"] = serde_json::json!(source_rendering.style.color_argb);
        style["size_level"] = serde_json::json!(0);
        style["stroke_type_raw"] = serde_json::json!(0);
        style["color_type_raw"] = serde_json::json!(0);
        let props = serde_json::json!({"compressed":true,"stylus_channels":true,"replay_only":false,"eraser":false,"fixed_width":false,"millisecond_timestamps":false,"top_layer_pen":false,"alpha_lock":false,"binary_added":false,"generated":false,"fixed_opacity":false,"rainbow_effect":false,"straighten":false,"reveal_mode":false});
        stroke.rendering = Some(serde_json::from_value(serde_json::json!({"metadata":null,"pen_name":"com.samsung.android.sdk.pen.pen.preload.Pencil2","advanced_settings":"14;","tool_type_raw":2,"properties":props,"style":style})).unwrap());
    }

    let decode = |packed: &[u8], key: &str| {
        let mut raw = Vec::new();
        flate2::read::ZlibDecoder::new(packed)
            .read_to_end(&mut raw)
            .unwrap();
        assert_eq!(raw.len(), 192 * 192 * 4);
        assert_eq!(
            format!("{:x}", Sha256::digest(&raw)),
            receipt[key].as_str().unwrap()
        );
        raw.as_chunks::<{ 192 * 4 }>()
            .0
            .iter()
            .rev()
            .flat_map(|row| row.as_chunks::<4>().0.iter().copied())
            .collect::<Vec<_>>()
    };
    let prior = decode(
        include_bytes!("../../../tools/pencil2-control/first.rgba.zlib"),
        "first_sha256",
    );
    let expected = decode(
        include_bytes!("../../../tools/pencil2-control/second.rgba.zlib"),
        "second_sha256",
    );
    let mut canvas =
        InkRasterCanvas::from_pixels(context(InkRasterOrigin::BottomLeft, 1), prior).unwrap();
    let projection = [
        0x3c2aaaab, 0, 0, 0, 0, 0x3c2aaaab, 0, 0, 0, 0, 0xb8000100, 0, 0xbf800000, 0xbf800000,
        0x80000000, 0x3f800000,
    ]
    .map(f32::from_bits);
    let mut matrix = [0.; 16];
    for i in 0..4 {
        matrix[5 * i] = 1.;
    }
    matrix[12] = 12.;
    matrix[13] = 12.;
    let mut grain = sdocx::Pencil2V14HostGrainState::default();
    grain.set_start_point([stroke.points[0].x as f32, stroke.points[0].y as f32]);
    let offset = grain.draw(projection, matrix, [192, 192]).unwrap();

    let uniform = receipt["uniforms"]
        .as_array()
        .unwrap()
        .iter()
        .rfind(|call| {
            call[0].as_str() == Some("glUniform2fv")
                && call[1].as_u64() == Some(4)
                && call[2].as_u64() == Some(1)
        })
        .unwrap();
    let values = uniform[3].as_array().unwrap();
    assert_eq!(values.len(), 2);
    assert_eq!(
        offset.map(f32::to_bits),
        std::array::from_fn(|i| (values[i].as_f64().unwrap() as f32).to_bits())
    );
    canvas
        .paint_pencil2_with_grain_offset(&stroke, InkRasterInverseScale::One, [12., 12.], offset)
        .unwrap();
    assert_eq!(canvas.pixels(), expected);
    let png = resvg::tiny_skia::Pixmap::decode_png(&canvas.to_png().unwrap()).unwrap();
    assert_eq!(png.data().as_chunks::<4>().0, expected);
    let page = canvas.rendered_page(0).unwrap();
    let tree = resvg::usvg::Tree::from_str(&page.svg, &Default::default()).unwrap();
    let mut pixels = resvg::tiny_skia::Pixmap::new(192, 192).unwrap();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixels.as_mut(),
    );
    assert_eq!(pixels.data().as_chunks::<4>().0, expected);
    #[cfg(feature = "pdf")]
    {
        let mut options = sdocx::PdfOptions::default();
        options.dpi = 72.;
        canvas.to_pdf(&options).unwrap();
    }
}

#[test]
fn pencil2_colored_tilted_overlapping_strokes_match_host_and_svg() {
    let prior: Vec<[u8; 4]> = (0..192 * 192)
        .map(|i| {
            let (x, y) = (i % 192, i / 192);
            [
                (x * 3 + y) as u8,
                (x + y * 5) as u8,
                (x * 7 + y * 3) as u8,
                255,
            ]
        })
        .collect();
    let receipt: Value = serde_json::from_str(include_str!(
        "assets/pencil2-overlap/bottom1-opaque-receipt.json"
    ))
    .unwrap();
    let mut delta = Vec::new();
    for row in receipt["delta_rows"].as_array().unwrap() {
        let row = row.as_array().unwrap();
        assert_eq!(row.len(), 5);
        let index = u32::try_from(row[0].as_u64().unwrap()).unwrap();
        delta.extend_from_slice(&index.to_le_bytes());
        for channel in &row[1..] {
            delta.push(u8::try_from(channel.as_u64().unwrap()).unwrap());
        }
    }
    assert_eq!(
        format!("{:x}", Sha256::digest(&delta)),
        receipt["delta_sha256"]
    );
    let (records, remainder) = delta.as_chunks::<8>();
    assert!(remainder.is_empty());
    assert_eq!(
        records.len(),
        receipt["changed_pixels"].as_u64().unwrap() as usize
    );
    let mut expected: Vec<u8> = prior.iter().flatten().copied().collect();
    let mut previous = None;
    for record in records {
        let index = u32::from_le_bytes(record[..4].try_into().unwrap()) as usize;
        assert!(index < prior.len());
        assert!(previous.is_none_or(|last| index > last));
        previous = Some(index);
        expected[index * 4..index * 4 + 4].copy_from_slice(&record[4..]);
    }
    assert_eq!(expected.len(), 192 * 192 * 4);
    assert_eq!(
        expected.len(),
        receipt["frame_bytes"].as_u64().unwrap() as usize
    );
    assert_eq!(
        format!("{:x}", Sha256::digest(&expected)),
        receipt["native_rgba_sha256"]
    );
    let mut canvas =
        InkRasterCanvas::from_pixels(context(InkRasterOrigin::BottomLeft, 1), prior).unwrap();
    for (case, translation, argb, tilt) in [
        ("pressure_ramp", [6., 9.], 0x80336699, 0.4),
        ("channel_baseline", [9., 11.], 0xffdd4411, 0.8),
    ] {
        let mut input = stroke(case, argb);
        input.tilts.fill(tilt);
        canvas
            .paint_pencil2_translated(&input, InkRasterInverseScale::One, translation)
            .unwrap();
    }
    assert_eq!(
        canvas
            .pixels()
            .iter()
            .flatten()
            .copied()
            .collect::<Vec<_>>(),
        expected
    );
    let page = canvas.rendered_page(0).unwrap();
    let tree = resvg::usvg::Tree::from_str(&page.svg, &Default::default()).unwrap();
    let mut image = resvg::tiny_skia::Pixmap::new(192, 192).unwrap();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut image.as_mut(),
    );
    assert_eq!(image.data(), expected);
    #[cfg(feature = "pdf")]
    {
        let mut options = sdocx::PdfOptions::default();
        options.dpi = 72.;
        sdocx::render_svg_pages_pdf(&[page], &options).unwrap();
        canvas.to_pdf(&options).unwrap();
    }
}
