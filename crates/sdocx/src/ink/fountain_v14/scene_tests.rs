use super::addpoint_record;
use crate::Stroke;
use crate::ink::host_rgba8::{over, quantize, sample_unorm8_half_rtz};
use base64::Engine;
use flate2::read::ZlibDecoder;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::Read;

#[cfg(feature = "serde")]
#[test]
fn saved_scene_candidate_replays_original_native_frames() {
    use crate::{FountainV14Scene, InkRasterCanvas, InkRasterContext, InkRasterOrigin};
    use std::collections::HashMap;

    let receipt: Value = serde_json::from_str(include_str!(
        "../../../tests/assets/fountain-v14/scene-native.json"
    ))
    .unwrap();
    let inputs: Value = serde_json::from_str(include_str!(
        "../../../tests/assets/fountain-v14/scene-sources.json"
    ))
    .unwrap();
    let sources: HashMap<u64, Stroke> = inputs["strokes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            (
                row["index"].as_u64().unwrap(),
                serde_json::from_value(row["stroke"].clone()).unwrap(),
            )
        })
        .collect();
    let mut ids: Vec<_> = sources.keys().copied().collect();
    ids.sort_unstable();
    let page = crate::Page {
        uuid: String::new(),
        width: 128,
        height: 256,
        content_bbox: Default::default(),
        background_color: None,
        template: None,
        background: Default::default(),
        objects: {
            use crate::{ObjectRenderLayer, PageElement, PageObject, PageObjectContent};
            let object = |id: u64, render_layer| PageObject {
                render_layer,
                source_offset: Some(id as usize),
                content: PageObjectContent::Stroke(sources[&id].clone()),
            };
            vec![
                object(30, ObjectRenderLayer::Masking),
                object(29, ObjectRenderLayer::Top),
                PageObject {
                    render_layer: ObjectRenderLayer::Base,
                    source_offset: None,
                    content: PageObjectContent::Element(PageElement::Image {
                        bbox: Default::default(),
                        media_index: 0,
                    }),
                },
                PageObject {
                    render_layer: ObjectRenderLayer::Base,
                    source_offset: None,
                    content: PageObjectContent::Container(
                        ids[..6]
                            .iter()
                            .map(|id| object(*id, ObjectRenderLayer::Base))
                            .collect(),
                    ),
                },
                object(3, ObjectRenderLayer::Other(9)),
            ]
        },
    };
    let composed = page.composed_objects_with_context();
    let composed_strokes: Vec<_> = composed
        .iter()
        .filter_map(|(object, allowed)| {
            object
                .content
                .as_stroke()
                .map(|stroke| (object.source_offset.unwrap() as u64, stroke, *allowed))
        })
        .collect();
    assert_eq!(
        composed_strokes
            .iter()
            .map(|(id, _, allowed)| (*id, *allowed))
            .collect::<Vec<_>>(),
        ids.iter().map(|id| (*id, true)).collect::<Vec<_>>()
    );
    assert!(
        composed
            .iter()
            .any(|(object, _)| matches!(object.content, crate::PageObjectContent::Element(_)))
    );
    let decode = |value: &Value| -> Vec<[u8; 4]> {
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
    };
    let mut scene = FountainV14Scene::default();
    for origin in ["top", "bottom"] {
        for scale in [1, 2] {
            for prior in ["clear", "white", "checker"] {
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
                    InkRasterCanvas::from_pixels(context, decode(&receipt["prior_inputs"][prior]))
                        .unwrap();
                for (step, id) in [3, 6, 7, 18, 19, 28, 29, 30, 6].into_iter().enumerate() {
                    let case =
                        &receipt["cases"][format!("scene_{origin}_{scale}_{prior}_{step}_{id}")];
                    canvas
                        .paint_fountain_v14_stroke(
                            &mut scene,
                            composed_strokes
                                .iter()
                                .find(|(candidate, _, _)| *candidate == id)
                                .unwrap()
                                .1,
                            id,
                            0,
                            serde_json::from_value(case["source_origin"].clone()).unwrap(),
                            receipt["model"][id.to_string()]["color_argb"]
                                .as_u64()
                                .unwrap() as u32,
                        )
                        .unwrap();
                    let expected = decode(&case["output"]);
                    assert_eq!(
                        canvas.pixels(),
                        expected,
                        "{origin} {scale} {prior} step {step} source {id}"
                    );
                    {
                        let page = canvas.rendered_page(0).unwrap();
                        let tree = resvg::usvg::Tree::from_str(
                            &page.svg,
                            &resvg::usvg::Options::default(),
                        )
                        .unwrap();
                        let mut image = resvg::tiny_skia::Pixmap::new(128, 256).unwrap();
                        resvg::render(
                            &tree,
                            resvg::tiny_skia::Transform::identity(),
                            &mut image.as_mut(),
                        );
                        assert_eq!(
                            image.data(),
                            expected.as_flattened(),
                            "SVG {origin} {scale} {prior} step {step} source {id}"
                        );
                        #[cfg(feature = "pdf")]
                        {
                            let mut options = crate::PdfOptions::default();
                            options.dpi = 72.;
                            crate::render_svg_pages_pdf(&[page], &options).unwrap();
                            canvas.to_pdf(&options).unwrap();
                        }
                    }
                }
            }
        }
    }
}

#[cfg(feature = "serde")]
#[test]
fn scene_cache_reuses_invalidates_and_resets_snapshots() {
    use crate::{FountainV14Scene, InkRasterContext, InkRasterOrigin};

    let inputs: Value = serde_json::from_str(include_str!(
        "../../../tests/assets/fountain-v14/scene-sources.json"
    ))
    .unwrap();
    let stroke: Stroke = serde_json::from_value(
        inputs["strokes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["index"] == 29)
            .unwrap()["stroke"]
            .clone(),
    )
    .unwrap();
    let context = InkRasterContext {
        width: 128,
        height: 256,
        pixels_per_unit: 1,
        origin: InkRasterOrigin::TopLeft,
    };
    let mut scene = FountainV14Scene::default();
    let first = scene.prepare(29, 0, &stroke, context).unwrap() as *const _;
    assert_eq!(
        first,
        scene.prepare(29, 0, &stroke, context).unwrap() as *const _
    );
    let mut invalid = stroke.clone();
    invalid.points.clear();
    assert!(scene.prepare(29, 1, &invalid, context).is_err());
    assert_eq!(
        first,
        scene.prepare(29, 0, &stroke, context).unwrap() as *const _
    );
    assert!(scene.prepare(29, 1, &stroke, context).is_ok());
    scene.reset();
    let fresh = scene
        .prepare(29, 0, &stroke, context)
        .unwrap()
        .outgoing_direction();
    let independent =
        crate::FountainV14Prepared::new(&stroke, context, crate::Point { x: 0., y: 1. }).unwrap();
    assert_eq!(fresh, independent.outgoing_direction());
}

#[cfg(feature = "serde")]
#[test]
fn scene_cache_capacity_and_failed_generation_preserve_direction() {
    use crate::{FountainV14Scene, InkRasterContext, InkRasterOrigin};

    let inputs: Value = serde_json::from_str(include_str!(
        "../../../tests/assets/fountain-v14/scene-sources.json"
    ))
    .unwrap();
    let source = |id: u64| -> Stroke {
        serde_json::from_value(
            inputs["strokes"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["index"] == id)
                .unwrap()["stroke"]
                .clone(),
        )
        .unwrap()
    };
    let stroke = source(29);
    let short = source(30);
    let context = InkRasterContext {
        width: 128,
        height: 256,
        pixels_per_unit: 1,
        origin: InkRasterOrigin::TopLeft,
    };
    let mut scene = FountainV14Scene::default();
    let mut control = FountainV14Scene::default();
    scene.prepare(29, 0, &stroke, context).unwrap();
    control.prepare(29, 0, &stroke, context).unwrap();
    let mut invalid = stroke.clone();
    invalid.points.clear();
    assert!(scene.prepare(29, 1, &invalid, context).is_err());
    assert_eq!(
        scene
            .prepare(30, 0, &short, context)
            .unwrap()
            .outgoing_direction(),
        control
            .prepare(30, 0, &short, context)
            .unwrap()
            .outgoing_direction(),
    );

    let mut full = FountainV14Scene::default();
    for id in 0..256 {
        full.prepare(id, 0, &stroke, context).unwrap();
    }
    assert!(matches!(
        full.prepare(256, 0, &stroke, context),
        Err(crate::InkRasterError::WorkLimit)
    ));
    assert!(full.prepare(0, 1, &stroke, context).is_ok());
    full.reset();
    assert!(full.prepare(256, 0, &stroke, context).is_ok());
}

#[cfg(feature = "serde")]
#[test]
fn prepared_snapshot_owns_source_and_rejects_other_scale_atomically() {
    use crate::{FountainV14Prepared, InkRasterCanvas, InkRasterContext, InkRasterOrigin};

    let inputs: Value = serde_json::from_str(include_str!(
        "../../../tests/assets/fountain-v14/scene-sources.json"
    ))
    .unwrap();
    let mut stroke: Stroke =
        serde_json::from_value(inputs["strokes"][0]["stroke"].clone()).unwrap();
    let context = InkRasterContext {
        width: 128,
        height: 256,
        pixels_per_unit: 1,
        origin: InkRasterOrigin::TopLeft,
    };
    let single: Stroke = serde_json::from_value(
        inputs["strokes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["index"] == 30)
            .unwrap()["stroke"]
            .clone(),
    )
    .unwrap();
    let direction = crate::Point { x: 1., y: 0. };
    let single_snapshot = FountainV14Prepared::new(&single, context, direction).unwrap();
    assert_eq!(single_snapshot.outgoing_direction(), direction);
    let snapshot =
        FountainV14Prepared::new(&stroke, context, crate::Point { x: 0., y: 1. }).unwrap();
    assert!(
        FountainV14Prepared::new(&stroke, context, crate::Point { x: f64::NAN, y: 0. }).is_err()
    );
    stroke.points.clear();
    assert!(FountainV14Prepared::new(&stroke, context, crate::Point { x: 0., y: 1. }).is_err());
    let mut canvas = InkRasterCanvas::new(context, [0; 4]).unwrap();
    canvas
        .paint_fountain_v14_prepared(&snapshot, [80, 72], 0xff000000)
        .unwrap();
    let other = InkRasterContext {
        pixels_per_unit: 2,
        ..context
    };
    let mut mismatched = InkRasterCanvas::new(other, [0; 4]).unwrap();
    let before = mismatched.pixels().to_vec();
    assert!(
        mismatched
            .paint_fountain_v14_prepared(&snapshot, [80, 72], 0xff000000)
            .is_err()
    );
    assert_eq!(mismatched.pixels(), before);

    let tiny = InkRasterContext {
        width: 1,
        height: 1,
        ..context
    };
    let mut bounded = InkRasterCanvas::new(tiny, [0; 4]).unwrap();
    for _ in 0..256 {
        bounded
            .paint_fountain_v14_prepared(&snapshot, [0, 0], 0xff000000)
            .unwrap();
    }
    let before = bounded.pixels().to_vec();
    assert!(
        bounded
            .paint_fountain_v14_prepared(&snapshot, [0, 0], 0xff000000)
            .is_err()
    );
    assert_eq!(bounded.pixels(), before);
}

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../../../conformance/fountain-v14-real-source-rgba-native.json"
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
    bytes.as_chunks::<4>().0.to_vec()
}

fn composite(mask: &[[u8; 4]], prior: &[[u8; 4]], argb: u32, narrow: bool) -> Vec<[u8; 4]> {
    let color = [16, 8, 0, 24].map(|shift| ((argb >> shift) as u8) as f32 / 255.);
    mask.iter()
        .zip(prior)
        .map(|(mask, prior)| {
            assert_eq!(mask, &[mask[3]; 4]);
            let sample = if narrow {
                sample_unorm8_half_rtz(mask[3])
            } else {
                mask[3] as f32 / 255.
            };
            let alpha = sample * color[3];
            let source =
                [color[0] * alpha, color[1] * alpha, color[2] * alpha, alpha].map(quantize);
            over(*prior, source)
        })
        .collect()
}

#[test]
fn current_saved_source_and_shared_addpoint_match_native_records() {
    let receipt = fixture();
    let stroke: Stroke = serde_json::from_value(receipt["source_stroke"].clone()).unwrap();
    assert_eq!(stroke.points.len(), 161);
    assert_eq!(
        stroke
            .rendering
            .as_ref()
            .unwrap()
            .advanced_settings
            .as_deref(),
        Some("14;")
    );
    let dots = super::super::fountain::prepare(&stroke).unwrap();
    assert_eq!(dots.points.len(), 24);
    assert_eq!(
        dots.sample_ends,
        serde_json::from_value::<Vec<usize>>(receipt["source_sample_ends"].clone()).unwrap()
    );
    let directions = dots.directions.unwrap();
    for scale in [1, 2] {
        let stream = &receipt["native_streams"][format!("scale_{scale}")];
        let records: Vec<[f32; 9]> = serde_json::from_value(stream["records"].clone()).unwrap();
        let actual: Vec<_> = dots
            .points
            .iter()
            .zip(&dots.radii)
            .zip(&directions)
            .map(|((&point, &radius), &direction)| {
                addpoint_record(point, radius, direction, scale as f32).unwrap()
            })
            .collect();
        let bytes: Vec<_> = actual
            .iter()
            .flatten()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        assert_eq!(
            format!("{:x}", Sha256::digest(&bytes)),
            stream["sha256"].as_str().unwrap()
        );
        for (a, b) in actual.iter().zip(records) {
            assert_eq!(a.map(f32::to_bits), b.map(f32::to_bits));
        }
    }
}

#[test]
fn persisted_native_drawing_matrices_drive_repeat_and_scaled_source_sampling() {
    let receipt: Value = serde_json::from_str(include_str!(
        "../../../../../conformance/fountain-v14-drawing-scale-native.json"
    ))
    .unwrap();
    let source = fixture();
    let stroke: Stroke = serde_json::from_value(source["source_stroke"].clone()).unwrap();
    assert_eq!(receipt["cases"].as_array().unwrap().len(), 10);
    for case in receipt["cases"].as_array().unwrap() {
        assert_eq!(case["point_count"], 161);
        assert_eq!(case["gl_inverse_scale"], case["rt_inverse_scale"]);
        let inverse: [f32; 2] = serde_json::from_value(case["gl_inverse_scale"].clone()).unwrap();
        let matrix: [f32; 9] = serde_json::from_value(case["matrix"].clone()).unwrap();
        let expected_inverse = [
            1. / matrix[0]
                .mul_add(
                    matrix[0],
                    matrix[1].mul_add(matrix[1], matrix[2] * matrix[2]),
                )
                .sqrt(),
            1. / matrix[3]
                .mul_add(
                    matrix[3],
                    matrix[4].mul_add(matrix[4], matrix[5] * matrix[5]),
                )
                .sqrt(),
        ];
        assert_eq!(
            inverse.map(f32::to_bits),
            expected_inverse.map(f32::to_bits)
        );
        let scale = 1. / inverse[0].min(inverse[1]);
        let repeat = 0.82f32 * inverse[0].min(inverse[1]);
        assert_eq!(
            repeat.to_bits(),
            (case["repeat_distance"].as_f64().unwrap() as f32).to_bits()
        );
        let dots = super::super::fountain::prepare_v14_with_output_scale(&stroke, scale).unwrap();
        assert_eq!(
            dots.points.len(),
            case["addpoint_count"].as_u64().unwrap() as usize
        );
        let mut bytes = Vec::new();
        for ((p, r), d) in dots
            .points
            .iter()
            .zip(&dots.radii)
            .zip(dots.directions.as_ref().unwrap())
        {
            for v in [
                p.x as f32, p.y as f32, *r as f32, d.x as f32, d.y as f32, 1.,
            ] {
                bytes.extend_from_slice(&v.to_le_bytes());
            }
        }
        assert_eq!(
            format!("{:x}", Sha256::digest(&bytes)),
            case["addpoint_sha256"].as_str().unwrap(),
            "{}",
            case["id"]
        );
        let mut prepared = crate::prepare_stroke(&stroke, false);
        assert!(crate::ink::apply_fountain_v14_output_scale(
            &mut prepared,
            &stroke,
            scale
        ));
        assert_eq!(prepared.points.as_ref(), dots.points.as_slice());
        assert_eq!(prepared.dot_radii.as_ref().unwrap(), &dots.radii);
        assert_eq!(prepared.dot_directions, dots.directions);
    }
    for scale in [0., f32::NAN, f32::INFINITY, 1e20] {
        assert!(super::super::fountain::prepare_v14_with_output_scale(&stroke, scale).is_none());
    }
}

#[test]
fn native_mask_composite_reuses_host_half_sampling_and_byte_blending() {
    let receipt = fixture();
    assert_eq!(receipt["cases"].as_object().unwrap().len(), 36);
    let mut plain_differences = 0;
    for (name, case) in receipt["cases"].as_object().unwrap() {
        assert_eq!(case["prior_verified"], true);
        assert_eq!(case["native_draw_calls"].as_array().unwrap().len(), 2);
        assert_eq!(
            case["stream_sha256"],
            receipt["native_streams"][case["stream"].as_str().unwrap()]["sha256"]
        );
        let mask = pixels(&case["mask"]);
        let prior = pixels(&receipt["prior_inputs"][case["prior"].as_str().unwrap()]);
        let expected = pixels(&case["output"]);
        let argb = case["argb"].as_u64().unwrap() as u32;
        let actual = composite(&mask, &prior, argb, true);
        assert_eq!(actual, expected, "{name}");
        plain_differences += composite(&mask, &prior, argb, false)
            .as_flattened()
            .iter()
            .zip(expected.as_flattened())
            .filter(|(a, b)| a != b)
            .count();
        if argb >> 24 == 0 {
            assert_eq!(actual, prior, "{name}");
        }
    }
    assert_eq!(plain_differences, 18);
}

#[test]
fn saved_color_mutation_recreates_geometry_and_replays_native_frames() {
    use crate::{FountainV14Prepared, InkRasterCanvas, InkRasterContext, InkRasterOrigin};

    let receipt: Value = serde_json::from_str(include_str!(
        "../../../tests/assets/fountain-v14/fountain-v14-color-mutation-native.json"
    ))
    .unwrap();
    let same_value: Value = serde_json::from_str(include_str!(
        "../../../tests/assets/fountain-v14/fountain-v14-color-same-value-native.json"
    ))
    .unwrap();
    for step in 0..5 {
        let key = format!("scene_top_1_clear_{step}_29");
        assert_eq!(
            same_value["cases"][&key]["argument_count"],
            receipt["cases"][&key]["argument_count"]
        );
        assert_eq!(
            same_value["cases"][&key]["output"]["sha256"],
            receipt["cases"][&key]["output"]["sha256"]
        );
    }
    let inputs: Value = serde_json::from_str(include_str!(
        "../../../tests/assets/fountain-v14/scene-sources.json"
    ))
    .unwrap();
    let source = inputs["strokes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["index"] == 29)
        .unwrap();
    let stroke: Stroke = serde_json::from_value(source["stroke"].clone()).unwrap();
    let original = receipt["model"]["29"]["color_argb"].as_u64().unwrap() as u32;
    let context = InkRasterContext {
        width: 128,
        height: 256,
        pixels_per_unit: 1,
        origin: InkRasterOrigin::TopLeft,
    };
    let mut canvas = InkRasterCanvas::new(context, [0; 4]).unwrap();
    let mut direction = crate::Point { x: 0., y: 1. };
    let mut snapshot = None;
    for step in 0..5 {
        let case = &receipt["cases"][format!("scene_top_1_clear_{step}_29")];
        let regenerates = matches!(step, 0 | 1 | 3);
        assert_eq!(case["argument_count"].as_u64().unwrap() > 0, regenerates);
        if regenerates {
            let prepared = FountainV14Prepared::new(&stroke, context, direction).unwrap();
            direction = prepared.outgoing_direction();
            snapshot = Some(prepared);
        }
        canvas
            .paint_fountain_v14_prepared(
                snapshot.as_ref().unwrap(),
                [80, 72],
                if matches!(step, 1 | 2) {
                    0xffd42818
                } else {
                    original
                },
            )
            .unwrap();
        let compressed = base64::engine::general_purpose::STANDARD
            .decode(case["output"]["rgba_zlib_base64"].as_str().unwrap())
            .unwrap();
        let mut bytes = Vec::new();
        ZlibDecoder::new(compressed.as_slice())
            .read_to_end(&mut bytes)
            .unwrap();
        let expected: Vec<[u8; 4]> = bytes
            .as_chunks::<512>()
            .0
            .iter()
            .rev()
            .flatten()
            .copied()
            .collect::<Vec<_>>()
            .as_chunks::<4>()
            .0
            .to_vec();
        assert_eq!(canvas.pixels(), expected, "step {step}");
    }
}

#[test]
fn saved_size_and_translation_mutation_replays_native_frames() {
    use crate::{FountainV14Prepared, InkRasterCanvas, InkRasterContext, InkRasterOrigin};

    let receipt: Value = serde_json::from_str(include_str!(
        "../../../tests/assets/fountain-v14/fountain-v14-geometry-mutation-native.json"
    ))
    .unwrap();
    let inputs: Value = serde_json::from_str(include_str!(
        "../../../tests/assets/fountain-v14/scene-sources.json"
    ))
    .unwrap();
    let source = inputs["strokes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["index"] == 29)
        .unwrap();
    let mut stroke: Stroke = serde_json::from_value(source["stroke"].clone()).unwrap();
    let original_size = stroke.pen_width;
    let original = receipt["model"]["29"]["color_argb"].as_u64().unwrap() as u32;
    let context = InkRasterContext {
        width: 128,
        height: 256,
        pixels_per_unit: 1,
        origin: InkRasterOrigin::TopLeft,
    };
    let mut canvas = InkRasterCanvas::new(context, [0; 4]).unwrap();
    let mut direction = crate::Point { x: 0., y: 1. };
    let mut snapshot = None;
    for step in 0..5 {
        let case = &receipt["cases"][format!("scene_top_1_clear_{step}_29")];
        let regenerates = matches!(step, 0 | 1 | 3);
        assert_eq!(case["argument_count"].as_u64().unwrap() > 0, regenerates);
        if regenerates {
            let size = if step == 1 { 8.0 } else { original_size };
            stroke.pen_width = size;
            stroke.rendering.as_mut().unwrap().style.pen_size = Some(size);
            let prepared = FountainV14Prepared::new(&stroke, context, direction).unwrap();
            direction = prepared.outgoing_direction();
            snapshot = Some(prepared);
        }
        canvas
            .paint_fountain_v14_prepared(
                snapshot.as_ref().unwrap(),
                if step == 4 { [72, 72] } else { [80, 72] },
                original,
            )
            .unwrap();
        let compressed = base64::engine::general_purpose::STANDARD
            .decode(case["output"]["rgba_zlib_base64"].as_str().unwrap())
            .unwrap();
        let mut bytes = Vec::new();
        ZlibDecoder::new(compressed.as_slice())
            .read_to_end(&mut bytes)
            .unwrap();
        let expected: Vec<[u8; 4]> = bytes
            .as_chunks::<512>()
            .0
            .iter()
            .rev()
            .flatten()
            .copied()
            .collect::<Vec<_>>()
            .as_chunks::<4>()
            .0
            .to_vec();
        assert_eq!(canvas.pixels(), expected, "step {step}");
    }
}

#[test]
fn saved_object_rect_translation_replays_native_frames() {
    use crate::{FountainV14Prepared, InkRasterCanvas, InkRasterContext, InkRasterOrigin};

    let receipt: Value = serde_json::from_str(include_str!(
        "../../../tests/assets/fountain-v14/fountain-v14-object-rect-native.json"
    ))
    .unwrap();
    let stroke: Stroke = serde_json::from_str(include_str!(
        "../../../tests/assets/fountain-v14/saved-source-29.json"
    ))
    .unwrap();
    let original = receipt["model"]["29"]["color_argb"].as_u64().unwrap() as u32;
    let context = InkRasterContext {
        width: 128,
        height: 256,
        pixels_per_unit: 1,
        origin: InkRasterOrigin::TopLeft,
    };
    let mut canvas = InkRasterCanvas::new(context, [0; 4]).unwrap();
    let mut direction = crate::Point { x: 0., y: 1. };
    let mut snapshot = None;
    for step in 0..5 {
        let case = &receipt["cases"][format!("scene_top_1_clear_{step}_29")];
        let regenerates = matches!(step, 0 | 1 | 3);
        assert_eq!(case["argument_count"].as_u64().unwrap() > 0, regenerates);
        if regenerates {
            let prepared = FountainV14Prepared::new(&stroke, context, direction).unwrap();
            direction = prepared.outgoing_direction();
            snapshot = Some(prepared);
        }
        canvas
            .paint_fountain_v14_prepared(snapshot.as_ref().unwrap(), [80, 72], original)
            .unwrap();
        let compressed = base64::engine::general_purpose::STANDARD
            .decode(case["output"]["rgba_zlib_base64"].as_str().unwrap())
            .unwrap();
        let mut bytes = Vec::new();
        ZlibDecoder::new(compressed.as_slice())
            .read_to_end(&mut bytes)
            .unwrap();
        let expected: Vec<[u8; 4]> = bytes
            .as_chunks::<512>()
            .0
            .iter()
            .rev()
            .flatten()
            .copied()
            .collect::<Vec<_>>()
            .as_chunks::<4>()
            .0
            .to_vec();
        assert_eq!(canvas.pixels(), expected, "step {step}");
    }
}

#[test]
fn saved_canvas_scale_mutation_replays_native_frames() {
    use crate::{FountainV14Prepared, InkRasterCanvas, InkRasterContext, InkRasterOrigin};

    let receipt: Value = serde_json::from_str(include_str!(
        "../../../tests/assets/fountain-v14/fountain-v14-scale-mutation-native.json"
    ))
    .unwrap();
    let stroke: Stroke = serde_json::from_str(include_str!(
        "../../../tests/assets/fountain-v14/saved-source-29.json"
    ))
    .unwrap();
    let color = receipt["model"]["29"]["color_argb"].as_u64().unwrap() as u32;
    let context = InkRasterContext {
        width: 128,
        height: 256,
        pixels_per_unit: 1,
        origin: InkRasterOrigin::TopLeft,
    };
    let mut canvas = InkRasterCanvas::new(context, [0; 4]).unwrap();
    let mut direction = crate::Point { x: 0., y: 1. };
    let mut snapshot = None;
    for (step, scale) in [1, 2, 2, 1, 1].into_iter().enumerate() {
        let case = &receipt["cases"][format!("scene_top_scale_clear_{step}_29")];
        let regenerates = matches!(step, 0 | 1 | 3);
        assert_eq!(case["argument_count"].as_u64().unwrap() > 0, regenerates);
        canvas.set_pixels_per_unit(scale).unwrap();
        if regenerates {
            let prepared = FountainV14Prepared::new(&stroke, canvas.context(), direction).unwrap();
            direction = prepared.outgoing_direction();
            snapshot = Some(prepared);
        }
        canvas
            .paint_fountain_v14_prepared(
                snapshot.as_ref().unwrap(),
                if scale == 1 { [80, 72] } else { [104, 168] },
                color,
            )
            .unwrap();
        let compressed = base64::engine::general_purpose::STANDARD
            .decode(case["output"]["rgba_zlib_base64"].as_str().unwrap())
            .unwrap();
        let mut bytes = Vec::new();
        ZlibDecoder::new(compressed.as_slice())
            .read_to_end(&mut bytes)
            .unwrap();
        let expected: Vec<[u8; 4]> = bytes
            .as_chunks::<512>()
            .0
            .iter()
            .rev()
            .flatten()
            .copied()
            .collect::<Vec<_>>()
            .as_chunks::<4>()
            .0
            .to_vec();
        assert_eq!(canvas.pixels(), expected, "step {step}");
        let page = canvas.rendered_page(0).unwrap();
        let tree =
            resvg::usvg::Tree::from_str(&page.svg, &resvg::usvg::Options::default()).unwrap();
        let mut image = resvg::tiny_skia::Pixmap::new(128, 256).unwrap();
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::identity(),
            &mut image.as_mut(),
        );
        assert_eq!(image.data(), expected.as_flattened(), "SVG step {step}");
        #[cfg(feature = "pdf")]
        {
            let mut options = crate::PdfOptions::default();
            options.dpi = 72.;
            crate::render_svg_pages_pdf(&[page], &options).unwrap();
            canvas.to_pdf(&options).unwrap();
        }
    }
    let before = canvas.pixels().to_vec();
    assert!(canvas.set_pixels_per_unit(3).is_err());
    assert_eq!(canvas.context().pixels_per_unit, 1);
    assert_eq!(canvas.pixels(), before);
}
