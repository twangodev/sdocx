use crate::Stroke;
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
                        if let Some(path) = std::env::var_os("SDOCX_FOUNTAIN_SCENE_ARTIFACTS") {
                            let path = std::path::PathBuf::from(path);
                            std::fs::create_dir_all(&path).unwrap();
                            let name = format!("scene_{origin}_{scale}_{prior}_{step}_{id}");
                            let mut options = crate::PdfOptions::default();
                            options.dpi = 72.;
                            std::fs::write(
                                path.join(format!("{name}.rgba")),
                                canvas.pixels().as_flattened(),
                            )
                            .unwrap();
                            std::fs::write(
                                path.join(format!("{name}.svg.pdf")),
                                crate::render_svg_pages_pdf(&[page], &options).unwrap(),
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
