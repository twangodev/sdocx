#![cfg(all(feature = "render", feature = "serde"))]

use base64::Engine;
use flate2::read::ZlibDecoder;
use sdocx::{
    InkRasterCanvas, InkRasterContext, InkRasterError, InkRasterOrigin, InkRasterProfile, Stroke,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::Read;

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../conformance/fountain-v14-scaled-source-rgba-native.json"
    ))
    .unwrap()
}

fn pixels(value: &Value) -> Vec<[u8; 4]> {
    let compressed = base64::engine::general_purpose::STANDARD
        .decode(value["rgba_zlib_base64"].as_str().unwrap())
        .unwrap();
    let mut bytes = Vec::new();
    ZlibDecoder::new(compressed.as_slice())
        .take(64 * 64 * 4 + 1)
        .read_to_end(&mut bytes)
        .unwrap();
    assert_eq!(bytes.len(), 64 * 64 * 4);
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        value["sha256"].as_str().unwrap()
    );
    bytes
        .as_chunks::<256>()
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

fn context(case: &Value) -> InkRasterContext {
    InkRasterContext {
        width: 64,
        height: 64,
        pixels_per_unit: case["scale"].as_u64().unwrap() as u32,
        origin: if case["origin"] == "top" {
            InkRasterOrigin::TopLeft
        } else {
            InkRasterOrigin::BottomLeft
        },
    }
}

fn profile(case: &Value) -> InkRasterProfile {
    if case["paint_mode"] == "direct" {
        return InkRasterProfile::FountainPenV14DirectHost {
            source_origin: serde_json::from_value(case["source_origin"].clone()).unwrap(),
            paint_argb: case["argb"].as_u64().unwrap() as u32,
        };
    }
    InkRasterProfile::FountainPenV14Host {
        source_origin: serde_json::from_value(case["source_origin"].clone()).unwrap(),
        paint_argb: case["argb"].as_u64().unwrap() as u32,
    }
}

#[test]
fn persisted_native_drawing_direct_paint_matches_public_canvas() {
    let receipt: Value = serde_json::from_str(include_str!(
        "../../../conformance/fountain-v14-drawing-paint-native.json"
    ))
    .unwrap();
    for case in receipt["cases"].as_object().unwrap().values() {
        assert_eq!(case["aa"], false);
        let outside = case["view"] == "outside";
        assert_eq!(
            case["draw_mode"],
            if outside {
                Value::Null
            } else {
                serde_json::json!([false, true])
            }
        );
        assert_eq!(case["native_counts"]["init_calls"], 1);
        assert_eq!(case["native_counts"]["canvas_calls"], 1);
        assert_eq!(
            case["gl_draw_names"],
            if outside {
                serde_json::json!([])
            } else {
                serde_json::json!(["glDrawArraysInstanced"])
            }
        );
        assert_eq!(case["paint_mode"], "direct");
        assert_eq!(case["apply_consumed"], 2119);
    }
    check_native_cases(&receipt, 108, 36);
    let enhanced = fixture();
    let direct = pixels(&receipt["cases"]["drawing_center_top_1_ff000000_white"]["output"]);
    let mask = pixels(&enhanced["cases"]["top_1_ff000000_white"]["output"]);
    let errors: Vec<_> = direct
        .as_flattened()
        .iter()
        .zip(mask.as_flattened())
        .map(|(a, b)| a.abs_diff(*b))
        .collect();
    assert_eq!(errors.iter().filter(|&&e| e != 0).count(), 96);
    assert_eq!(*errors.iter().max().unwrap(), 67);
    for case in receipt["cases"].as_object().unwrap().values() {
        let output = pixels(&case["output"]);
        let prior = pixels(&receipt["prior_inputs"][case["prior"].as_str().unwrap()]);
        if case["view"] == "outside" {
            assert_eq!(output, prior);
        } else if case["argb"].as_u64().unwrap() >> 24 == 0 {
            assert_ne!(
                output, prior,
                "native direct shader ignores saved color alpha"
            );
        }
    }
}

#[test]
fn public_vector_scale_option_serializes_the_native_resampled_centers() {
    let receipt = fixture();
    let stroke: Stroke = serde_json::from_value(receipt["source_stroke"].clone()).unwrap();
    let document = sdocx::Document {
        pages: vec![sdocx::Page {
            uuid: "fountain-native-source".into(),
            width: 1024,
            height: 1536,
            content_bbox: stroke.bbox,
            background_color: None,
            template: None,
            background: Default::default(),
            objects: vec![stroke.into()],
        }],
        metadata: Default::default(),
    };
    let drawing: Value = serde_json::from_str(include_str!(
        "../../../conformance/fountain-v14-drawing-scale-native.json"
    ))
    .unwrap();
    for scale in [1, 2] {
        let mut options = sdocx::RenderOptions::default();
        options.fountain_v14_output_scale_profile = Some(sdocx::FountainV14OutputScaleProfile {
            pixels_per_page_unit: scale as f32,
        });
        let page = sdocx::render_document_svg(&document, &options).remove(0);
        let xml = roxmltree::Document::parse(&page.svg).unwrap();
        let centers: Vec<_> = xml
            .descendants()
            .filter_map(|node| node.attribute("transform"))
            .filter_map(|text| {
                text.strip_prefix("matrix(")
                    .and_then(|v| v.strip_suffix(')'))
            })
            .filter_map(|text| {
                let values: Vec<f64> = text.split(',').map(|v| v.parse().unwrap()).collect();
                let [_, _, _, _, x, y] = values.as_slice() else {
                    return None;
                };
                ((*x > 900.) && (*y > 1400.)).then_some([*x, *y])
            })
            .collect();
        let native = drawing["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == format!("top_{scale}_crop"))
            .unwrap();
        let expected: Vec<[f64; 2]> = native["addpoint_arguments"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                std::array::from_fn(|i| format!("{:.4}", row[i].as_f64().unwrap()).parse().unwrap())
            })
            .collect();
        assert_eq!(centers, expected, "scale {scale}");
    }
}

#[test]
fn unchanged_saved_stroke_matches_native_masks_rgb_and_public_exports() {
    let receipt = fixture();
    check_native_cases(&receipt, 36, 12);
    let canonical: Value = serde_json::from_str(include_str!(
        "../../../conformance/fountain-v14-real-source-rgba-native.json"
    ))
    .unwrap();
    let name = "top_2_8044aaee_white";
    let old = pixels(&canonical["cases"][name]["output"]);
    let expected = pixels(&receipt["cases"][name]["output"]);
    let differences: Vec<_> = old
        .as_flattened()
        .iter()
        .zip(expected.as_flattened())
        .map(|(a, b)| a.abs_diff(*b))
        .collect();
    assert_eq!(differences.iter().filter(|&&e| e != 0).count(), 228);
    assert_eq!(*differences.iter().max().unwrap(), 87);
    if let Some(directory) = std::env::var_os("SDOCX_FOUNTAIN_V14_ARTIFACTS") {
        let metrics = serde_json::json!({"case":name,"canonical_24_stamp_channel_errors":228,"absolute_sum":differences.iter().map(|&v|u64::from(v)).sum::<u64>(),"maximum":87,"native_scaled_49_stamp_channel_errors":0});
        std::fs::write(
            std::path::PathBuf::from(directory).join("source-sampling-counterexample.json"),
            serde_json::to_vec(&metrics).unwrap(),
        )
        .unwrap();
    }
}

#[test]
fn viewport_edges_corners_and_outside_match_every_native_frame() {
    let receipt: Value = serde_json::from_str(include_str!(
        "../../../conformance/fountain-v14-clipping-native.json"
    ))
    .unwrap();
    check_native_cases(&receipt, 216, 72);
    for (name, case) in receipt["cases"].as_object().unwrap() {
        let mask = pixels(&case["mask"]);
        if case["view"] == "outside" {
            assert!(mask.iter().all(|p| *p == [0; 4]), "{name}");
            assert_eq!(
                pixels(&case["output"]),
                pixels(&receipt["prior_inputs"][case["prior"].as_str().unwrap()])
            );
        } else {
            assert!(mask.iter().any(|p| p[3] != 0), "{name}");
            assert!(
                mask.iter().enumerate().any(|(i, p)| p[3] != 0
                    && (i % 64 == 0 || i % 64 == 63 || i / 64 == 0 || i / 64 == 63)),
                "{name}: capture does not exercise a visible boundary"
            );
        }
    }
}

fn check_native_cases(receipt: &Value, frames: usize, expected_opaque: usize) {
    let stroke: Stroke = serde_json::from_value(receipt["source_stroke"].clone()).unwrap();
    assert_eq!(stroke.points.len(), 161);
    assert_eq!(receipt["cases"].as_object().unwrap().len(), frames);
    let mut opaque = 0;
    for (name, case) in receipt["cases"].as_object().unwrap() {
        let prior = pixels(&receipt["prior_inputs"][case["prior"].as_str().unwrap()]);
        let expected = pixels(&case["output"]);
        let mut canvas = InkRasterCanvas::from_pixels(context(case), prior.clone()).unwrap();
        canvas.paint(&stroke, profile(case)).unwrap();
        let errors: Vec<_> = canvas
            .pixels()
            .iter()
            .zip(&expected)
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .take(4)
            .collect();
        assert!(errors.is_empty(), "{name}: {errors:?}");
        if case["argb"].as_u64().unwrap() >> 24 == 0 && case["paint_mode"] != "direct" {
            assert_eq!(canvas.pixels(), prior);
        }
        let artifacts =
            std::env::var_os("SDOCX_FOUNTAIN_V14_ARTIFACTS").map(std::path::PathBuf::from);
        if let Some(path) = &artifacts {
            std::fs::create_dir_all(path).unwrap();
            std::fs::write(
                path.join(format!("{name}.rgba")),
                canvas.pixels().as_flattened(),
            )
            .unwrap();
            std::fs::write(
                path.join(format!("{name}.stroke.json")),
                serde_json::to_vec(&stroke).unwrap(),
            )
            .unwrap();
        }
        if expected.iter().any(|p| p[3] != 255) {
            continue;
        }
        opaque += 1;
        let page = canvas.rendered_page(0).unwrap();
        let tree =
            resvg::usvg::Tree::from_str(&page.svg, &resvg::usvg::Options::default()).unwrap();
        let mut image = resvg::tiny_skia::Pixmap::new(64, 64).unwrap();
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::identity(),
            &mut image.as_mut(),
        );
        assert_eq!(image.data(), expected.as_flattened(), "{name}");
        if let Some(path) = artifacts {
            std::fs::write(path.join(format!("{name}.svg")), &page.svg).unwrap();
            std::fs::write(path.join(format!("{name}.png")), canvas.to_png().unwrap()).unwrap();
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
    }
    assert_eq!(opaque, expected_opaque);
}

#[test]
fn unsupported_saved_settings_contexts_and_failed_submissions_are_atomic() {
    let receipt = fixture();
    let stroke: Stroke = serde_json::from_value(receipt["source_stroke"].clone()).unwrap();
    let case = &receipt["cases"]["top_1_ff000000_white"];
    let mut canvas = InkRasterCanvas::new(context(case), [255; 4]).unwrap();
    let prior = canvas.pixels().to_vec();
    assert!(matches!(
        canvas.paint(
            &stroke,
            InkRasterProfile::FountainPenV14Host {
                source_origin: [i32::MIN, 0],
                paint_argb: 0xff000000,
            }
        ),
        Err(InkRasterError::InvalidContext)
    ));
    assert_eq!(canvas.pixels(), prior);
    let mut unsupported = stroke.clone();
    unsupported
        .rendering
        .as_mut()
        .unwrap()
        .properties
        .fixed_opacity = true;
    assert!(matches!(
        canvas.paint(&unsupported, profile(case)),
        Err(InkRasterError::UnsupportedStroke(_))
    ));
    assert_eq!(canvas.pixels(), prior);
    let mut unsupported = stroke.clone();
    unsupported.rendering.as_mut().unwrap().style.dash_offset = Some(1.);
    assert!(matches!(
        canvas.paint(&unsupported, profile(case)),
        Err(InkRasterError::UnsupportedStroke(_))
    ));
    assert_eq!(canvas.pixels(), prior);
    for _ in 0..256 {
        canvas.paint(&stroke, profile(case)).unwrap();
    }
    let painted = canvas.pixels().to_vec();
    assert!(matches!(
        canvas.paint(&stroke, profile(case)),
        Err(InkRasterError::WorkLimit)
    ));
    assert_eq!(canvas.pixels(), painted);
}
