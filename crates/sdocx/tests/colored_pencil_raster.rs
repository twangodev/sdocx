#![cfg(all(feature = "render", feature = "serde"))]

use base64::Engine;
use flate2::read::ZlibDecoder;
use sdocx::{
    BoundingBox, Color, InkRasterCanvas, InkRasterContext, InkRasterDepthContext, InkRasterError,
    InkRasterOrigin, InkRasterProfile, Point, Stroke, StrokeRendering, StrokeStyle,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::io::Read;

#[path = "colored_pencil_raster/display.rs"]
mod display;

fn bits(value: &Value) -> f32 {
    let raw = value.as_str().unwrap();
    f32::from_le_bytes(std::array::from_fn(|i| {
        u8::from_str_radix(&raw[i * 2..i * 2 + 2], 16).unwrap()
    }))
}

fn source(id: &str, argb: u32) -> Stroke {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../conformance/colored-pencil-native.json"
    ))
    .unwrap();
    let input = &fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == id)
        .unwrap()["input"];
    source_input(input, argb)
}

fn source_input(input: &Value, argb: u32) -> Stroke {
    let size = bits(
        input
            .get("size_f32_bits")
            .unwrap_or(&input["pen_size_f32_bits"]),
    );
    let mut style = serde_json::to_value(StrokeStyle::default()).unwrap();
    style["color_argb"] = json!(argb);
    style["pen_size"] = json!(size);
    let rendering:StrokeRendering=serde_json::from_value(json!({
        "metadata":null,"pen_name":"com.samsung.android.sdk.pen.pen.preload.ColoredPencil","advanced_settings":"4;","tool_type_raw":2,"style":style,
        "properties":{"compressed":true,"stylus_channels":true,"replay_only":false,"eraser":false,"fixed_width":false,"millisecond_timestamps":false,"top_layer_pen":false,"alpha_lock":false,"binary_added":false,"generated":false,"fixed_opacity":false,"rainbow_effect":false,"straighten":false,"reveal_mode":false}
    })).unwrap();
    let points: Vec<_> = input["points_f32_bits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| Point {
            x: f64::from(bits(&p[0])),
            y: f64::from(bits(&p[1])),
        })
        .collect();
    let count = points.len();
    Stroke {
        bbox: BoundingBox::default(),
        points,
        pressures: input.get("pressure_f32_bits").map_or_else(
            || vec![0.5; count],
            |values| {
                values
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| f64::from(bits(v)))
                    .collect()
            },
        ),
        timestamps: input.get("timestamps_i32").map_or_else(
            || vec![0; count],
            |values| {
                values
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_i64().unwrap())
                    .collect()
            },
        ),
        tilts: input["tilt_f32_bits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| f64::from(bits(v)))
            .collect(),
        orientations: input["orientation_f32_bits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| f64::from(bits(v)))
            .collect(),
        pen_width: size,
        color: Some(Color {
            r: (argb >> 16) as u8,
            g: (argb >> 8) as u8,
            b: argb as u8,
        }),
        rendering: Some(rendering),
    }
}

fn profile() -> InkRasterProfile {
    InkRasterProfile::ColoredPencilV4Host {
        pen_data_width: 1920,
        pen_data_height: 1080,
    }
}
fn context(case: &Value) -> InkRasterContext {
    InkRasterContext {
        width: case["frame"][0].as_u64().unwrap() as u32,
        height: case["frame"][1].as_u64().unwrap() as u32,
        pixels_per_unit: case["scale"].as_u64().unwrap() as u32,
        origin: if case["origin"] == "top" {
            InkRasterOrigin::TopLeft
        } else {
            InkRasterOrigin::BottomLeft
        },
    }
}
fn native(case: &Value) -> Vec<[u8; 4]> {
    let width = case["frame"][0].as_u64().unwrap() as usize;
    let height = case["frame"][1].as_u64().unwrap() as usize;
    let background = std::array::from_fn(|i| case["background"][i].as_u64().unwrap() as u8);
    let mut image = vec![background; width * height];
    let full = case.get("rgba_zlib_base64");
    let compressed = base64::engine::general_purpose::STANDARD
        .decode(
            full.unwrap_or(&case["sparse_zlib_base64"])
                .as_str()
                .unwrap(),
        )
        .unwrap();
    let mut sparse = Vec::new();
    ZlibDecoder::new(compressed.as_slice())
        .take((width * height * 8 + 1) as u64)
        .read_to_end(&mut sparse)
        .unwrap();
    if full.is_some() {
        assert_eq!(sparse.len(), width * height * 4);
        image.copy_from_slice(sparse.as_chunks::<4>().0);
    } else {
        assert_eq!(
            sparse.len(),
            case["changed_pixels"].as_u64().unwrap() as usize * 8
        );
        for p in sparse.as_chunks::<8>().0 {
            let i = u32::from_le_bytes(p[..4].try_into().unwrap()) as usize;
            image[i] = p[4..8].try_into().unwrap();
        }
    }
    assert_eq!(
        format!(
            "{:x}",
            Sha256::digest(image.iter().flatten().copied().collect::<Vec<_>>())
        ),
        case["frame_sha256"].as_str().unwrap()
    );
    image.chunks_exact(width).rev().flatten().copied().collect()
}

fn artifacts(name: &str, canvas: &InkRasterCanvas, stroke: &Stroke) {
    let Some(path) = std::env::var_os("SDOCX_COLORED_PENCIL_ARTIFACTS") else {
        return;
    };
    let path = std::path::PathBuf::from(path);
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(
        path.join(format!("{name}.stroke.json")),
        serde_json::to_vec(stroke).unwrap(),
    )
    .unwrap();
    std::fs::write(
        path.join(format!("{name}.rgba")),
        canvas
            .pixels()
            .iter()
            .flatten()
            .copied()
            .collect::<Vec<_>>(),
    )
    .unwrap();
    std::fs::write(path.join(format!("{name}.png")), canvas.to_png().unwrap()).unwrap();
    let page = canvas.rendered_page(0).unwrap();
    std::fs::write(path.join(format!("{name}.svg")), &page.svg).unwrap();
    #[cfg(feature = "pdf")]
    {
        let mut options = sdocx::PdfOptions::default();
        options.dpi = 72.;
        std::fs::write(
            path.join(format!("{name}.svg.pdf")),
            sdocx::render_svg_pages_pdf(&[page], &options).unwrap(),
        )
        .unwrap();
        std::fs::write(
            path.join(format!("{name}.direct.pdf")),
            canvas.to_pdf(&options).unwrap(),
        )
        .unwrap();
    }
}

#[test]
fn public_source_to_pixels_matches_all_native_backdrops_and_projection_controls() {
    let receipt: Value = serde_json::from_str(include_str!(
        "../../../conformance/colored-pencil-v4-canvas-native.json"
    ))
    .unwrap();
    assert_eq!(receipt["cases"].as_object().unwrap().len(), 100);
    for (name, case) in receipt["cases"].as_object().unwrap() {
        let ctx = context(case);
        let prior = std::array::from_fn(|i| case["background"][i].as_u64().unwrap() as u8);
        let stroke = source(
            case["source"].as_str().unwrap(),
            case["argb"].as_u64().unwrap() as u32,
        );
        let mut canvas =
            InkRasterCanvas::from_pixels(ctx, vec![prior; ctx.pixel_count().unwrap()]).unwrap();
        canvas.paint(&stroke, profile()).unwrap();
        let expected = native(case);
        let errors: Vec<_> = canvas
            .pixels()
            .iter()
            .zip(&expected)
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .take(3)
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
            assert!(
                image.data().as_chunks::<4>().0 == expected,
                "SVG native mismatch: {name}"
            );
        }
        artifacts(name, &canvas, &stroke);
    }
}

#[test]
fn source_sizes_display_contexts_and_displacement_gates_match_native_pixels() {
    let geometry: Value = serde_json::from_str(include_str!(
        "../../../conformance/colored-pencil-v4-source-context-native.json"
    ))
    .unwrap();
    let receipt: Value = serde_json::from_str(include_str!(
        "../../../conformance/colored-pencil-v4-context-paint-native.json"
    ))
    .unwrap();
    assert_eq!(receipt["cases"].as_object().unwrap().len(), 300);
    let mut failures = Vec::new();
    for (name, case) in receipt["cases"].as_object().unwrap() {
        assert_eq!(case["prior_verified"], true);
        let source = geometry["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["id"] == case["source"])
            .unwrap();
        assert_eq!(source["stream_sha256"], case["stream_sha256"]);
        let stroke = source_input(&source["input"], case["argb"].as_u64().unwrap() as u32);
        let profile = InkRasterProfile::ColoredPencilV4Host {
            pen_data_width: source["screen"][0].as_u64().unwrap() as u32,
            pen_data_height: source["screen"][1].as_u64().unwrap() as u32,
        };
        let ctx = context(case);
        let prior = std::array::from_fn(|i| case["background"][i].as_u64().unwrap() as u8);
        let mut canvas = InkRasterCanvas::new(ctx, prior).unwrap();
        canvas.paint(&stroke, profile).unwrap();
        let expected = native(case);
        let errors: Vec<_> = canvas
            .pixels()
            .iter()
            .zip(&expected)
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, (a, b))| (i % ctx.width as usize, i / ctx.width as usize, *a, *b))
            .collect();
        if !errors.is_empty() {
            failures.push((
                name,
                errors.len(),
                errors.into_iter().take(2).collect::<Vec<_>>(),
            ));
        }
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
        artifacts(&format!("context_{name}"), &canvas, &stroke);
    }
    assert!(failures.is_empty(), "{failures:?}");
}

#[test]
fn explicit_depth_bitmap_matches_native_pixels_and_svg() {
    let geometry: Value = serde_json::from_str(include_str!(
        "../../../conformance/colored-pencil-v4-source-context-native.json"
    ))
    .unwrap();
    let receipt: Value = serde_json::from_str(include_str!(
        "../../../conformance/colored-pencil-v4-depth-native.json"
    ))
    .unwrap();
    assert_eq!(receipt["cases"].as_object().unwrap().len(), 360);
    for (name, case) in receipt["cases"].as_object().unwrap() {
        let source = geometry["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == case["source"])
            .unwrap();
        let stroke = source_input(&source["input"], case["argb"].as_u64().unwrap() as u32);
        let map = &receipt["depth_inputs"][case["depth"].as_str().unwrap()];
        let raw = map["rgba_hex"].as_str().unwrap();
        let bytes: Vec<u8> = (0..raw.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&raw[i..i + 2], 16).unwrap())
            .collect();
        let depth = InkRasterDepthContext {
            width: map["width"].as_u64().unwrap() as u32,
            height: map["height"].as_u64().unwrap() as u32,
            background_scale: case["background_scale"].as_f64().unwrap() as f32,
        };
        let ctx = context(case);
        let prior = std::array::from_fn(|i| case["background"][i].as_u64().unwrap() as u8);
        let mut canvas = InkRasterCanvas::new(ctx, prior).unwrap();
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
        artifacts(&format!("depth_{name}"), &canvas, &stroke);
    }
}

#[test]
fn rejected_depth_contexts_storage_and_profiles_are_atomic() {
    let context = InkRasterContext {
        width: 256,
        height: 256,
        pixels_per_unit: 1,
        origin: InkRasterOrigin::TopLeft,
    };
    let stroke = source("saved-glv4-size-8", 0x8044aaee);
    let mut canvas = InkRasterCanvas::new(context, [255; 4]).unwrap();
    let valid = InkRasterDepthContext {
        width: 1,
        height: 1,
        background_scale: 0.75,
    };
    for invalid in [
        InkRasterDepthContext { width: 0, ..valid },
        InkRasterDepthContext { height: 0, ..valid },
        InkRasterDepthContext {
            width: 8193,
            ..valid
        },
        InkRasterDepthContext {
            background_scale: 0.0001,
            ..valid
        },
        InkRasterDepthContext {
            background_scale: 12.,
            ..valid
        },
        InkRasterDepthContext {
            background_scale: f32::NAN,
            ..valid
        },
        InkRasterDepthContext {
            background_scale: f32::INFINITY,
            ..valid
        },
    ] {
        assert!(matches!(
            canvas.paint_with_depth_map(&stroke, profile(), invalid, &[[255; 4]]),
            Err(InkRasterError::InvalidDepthMap)
        ));
    }
    assert!(matches!(
        canvas.paint_with_depth_map(
            &stroke,
            profile(),
            InkRasterDepthContext {
                width: 8192,
                height: 8192,
                ..valid
            },
            &[]
        ),
        Err(InkRasterError::WorkLimit)
    ));
    assert!(matches!(
        canvas.paint_with_depth_map(
            &stroke,
            sdocx::InkRasterProfile::Pencil2V14 {
                inverse_scale: sdocx::InkRasterInverseScale::One
            },
            valid,
            &[[255; 4]]
        ),
        Err(InkRasterError::UnsupportedStroke(_))
    ));
    for _ in 0..300 {
        assert!(matches!(
            canvas.paint_with_depth_map(&stroke, profile(), valid, &[]),
            Err(InkRasterError::InvalidDepthMap)
        ));
    }
    assert!(canvas.pixels().iter().all(|&p| p == [255; 4]));
    canvas
        .paint_with_depth_map(&stroke, profile(), valid, &[[255, 0, 0, 0]])
        .unwrap();
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../conformance/colored-pencil-v4-depth-native.json"
    ))
    .unwrap();
    assert!(canvas.pixels() == native(&fixture["cases"]["size_8_white_paper0.75_top_1_white"]));
}

#[test]
fn clamped_sizes_and_equivalent_display_contexts_match_native_alias_frames() {
    let geometry: Value = serde_json::from_str(include_str!(
        "../../../conformance/colored-pencil-v4-source-context-native.json"
    ))
    .unwrap();
    let receipt: Value = serde_json::from_str(include_str!(
        "../../../conformance/colored-pencil-v4-context-paint-native.json"
    ))
    .unwrap();
    assert_eq!(receipt["aliases"].as_object().unwrap().len(), 6);
    for (alias, canonical) in receipt["aliases"].as_object().unwrap() {
        let source = geometry["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == *alias)
            .unwrap();
        let case = &receipt["cases"][format!("{}_top_1_white", canonical.as_str().unwrap())];
        assert_eq!(source["stream_sha256"], case["stream_sha256"]);
        let stroke = source_input(&source["input"], case["argb"].as_u64().unwrap() as u32);
        let mut canvas = InkRasterCanvas::new(context(case), [255; 4]).unwrap();
        canvas
            .paint(
                &stroke,
                InkRasterProfile::ColoredPencilV4Host {
                    pen_data_width: source["screen"][0].as_u64().unwrap() as u32,
                    pen_data_height: source["screen"][1].as_u64().unwrap() as u32,
                },
            )
            .unwrap();
        let expected = native(case);
        assert!(canvas.pixels() == expected, "{alias}");
    }
}

#[test]
fn unsupported_profiles_invalid_channels_and_resource_failures_are_atomic() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../conformance/colored-pencil-v4-canvas-native.json"
    ))
    .unwrap();
    let case = &fixture["cases"]["8044aaee_saved-glv4-size-8_top_1_white"];
    let ctx = context(case);
    let prior = vec![[255; 4]; ctx.pixel_count().unwrap()];
    let stroke = source("saved-glv4-size-8", 0x8044aaee);
    let mut canvas = InkRasterCanvas::from_pixels(ctx, prior.clone()).unwrap();
    let mut rejected = stroke.clone();
    rejected.rendering.as_mut().unwrap().advanced_settings = None;
    for _ in 0..300 {
        assert!(matches!(
            canvas.paint(&rejected, profile()),
            Err(InkRasterError::UnsupportedStroke(_))
        ));
    }
    assert!(matches!(
        canvas.paint(
            &stroke,
            InkRasterProfile::ColoredPencilV4Host {
                pen_data_width: 0,
                pen_data_height: 1920
            }
        ),
        Err(InkRasterError::InvalidContext)
    ));
    assert!(matches!(
        canvas.paint(
            &stroke,
            InkRasterProfile::ColoredPencilV4Host {
                pen_data_width: 16_385,
                pen_data_height: 1080
            }
        ),
        Err(InkRasterError::InvalidContext)
    ));
    rejected = stroke.clone();
    rejected.pressures[0] = f64::NAN;
    assert!(matches!(
        canvas.paint(&rejected, profile()),
        Err(InkRasterError::InvalidStroke(_))
    ));
    rejected = stroke.clone();
    rejected.tilts.pop();
    assert!(matches!(
        canvas.paint(&rejected, profile()),
        Err(InkRasterError::InvalidStroke(_))
    ));
    rejected = stroke.clone();
    rejected.timestamps[0] = i64::MAX;
    assert!(matches!(
        canvas.paint(&rejected, profile()),
        Err(InkRasterError::InvalidStroke(_))
    ));
    rejected = stroke.clone();
    rejected.tilts[1] = f64::from(f32::MAX);
    assert!(matches!(
        canvas.paint(&rejected, profile()),
        Err(InkRasterError::InvalidStroke(_))
    ));
    rejected = stroke.clone();
    rejected.points.resize(4097, Point { x: 0., y: 0. });
    assert!(matches!(
        canvas.paint(&rejected, profile()),
        Err(InkRasterError::WorkLimit)
    ));
    rejected = stroke.clone();
    rejected.pen_width = 4.;
    assert!(matches!(
        canvas.paint(&rejected, profile()),
        Err(InkRasterError::InvalidStroke(_))
    ));
    assert_eq!(canvas.pixels(), prior);
    canvas.paint(&stroke, profile()).unwrap();
    assert_eq!(canvas.pixels(), native(case));
}

#[test]
fn raw_transform_and_unknown_metadata_masks_are_rejected_atomically() {
    let stroke = source("saved-glv4-size-8", 0x8044aaee);
    let context = InkRasterContext {
        width: 256,
        height: 256,
        pixels_per_unit: 1,
        origin: InkRasterOrigin::TopLeft,
    };
    let mut canvas = InkRasterCanvas::new(context, [255; 4]).unwrap();
    for (property_mask, field_mask) in [
        (vec![0x80], vec![]),
        (vec![0, 4], vec![]),
        (vec![0, 0, 1], vec![]),
        (vec![], vec![1]),
        (vec![], vec![2]),
        (vec![], vec![0, 1]),
    ] {
        let mut rejected = stroke.clone();
        rejected.rendering.as_mut().unwrap().metadata = Some(
            serde_json::from_value(json!({
                "format_version": 0, "uuid": "", "modified_time_raw": 0,
                "bbox": {"x_min": 0., "y_min": 0., "x_max": 0., "y_max": 0.},
                "replay_timestamp_raw": 0, "resize_mode_raw": 0,
                "rotatable": false, "selectable": false, "movable": false,
                "visible": true, "replayable": false, "out_of_canvas_enabled": false,
                "template": false, "flip_enabled": false, "float_drawn_rect": false,
                "locked": false, "removable": true, "rotation_degrees": null,
                "property_mask": property_mask, "field_mask": field_mask,
                "fixed_trailing_data": [], "flexible_trailing_data": []
            }))
            .unwrap(),
        );
        assert!(matches!(
            canvas.paint(&rejected, profile()),
            Err(InkRasterError::UnsupportedStroke(_))
        ));
        assert!(canvas.pixels().iter().all(|p| *p == [255; 4]));
    }
    let mut control = InkRasterCanvas::new(context, [255; 4]).unwrap();
    control.paint(&stroke, profile()).unwrap();
    canvas.paint(&stroke, profile()).unwrap();
    assert_eq!(canvas.pixels(), control.pixels());
}

#[test]
fn exhausted_submission_budget_preserves_pixels() {
    let stroke = source("saved-glv4-size-8", 0x8044aaee);
    let context = InkRasterContext {
        width: 1,
        height: 1,
        pixels_per_unit: 1,
        origin: InkRasterOrigin::TopLeft,
    };
    let mut canvas = InkRasterCanvas::new(context, [255; 4]).unwrap();
    for _ in 0..256 {
        canvas.paint(&stroke, profile()).unwrap();
    }
    let before = canvas.pixels().to_vec();
    assert!(matches!(
        canvas.paint(&stroke, profile()),
        Err(InkRasterError::WorkLimit)
    ));
    assert_eq!(canvas.pixels(), before);
}

#[test]
fn scale_change_preserves_prior_paint_and_matches_fresh_context() {
    for (stroke, profile) in [(source("saved-glv4-size-8", 0x8044aaee), profile())] {
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
