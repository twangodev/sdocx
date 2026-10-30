#![cfg(all(feature = "render", feature = "serde"))]

use base64::Engine;
use flate2::read::ZlibDecoder;
use sdocx::{InkRasterCanvas, InkRasterContext, InkRasterOrigin, Stroke};
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
fn prepared_cache_reuse_preserves_direction_for_next_native_source() {
    use sdocx::{FountainV14Prepared, Point};
    use std::collections::HashMap;

    let inputs: Value = serde_json::from_str(include_str!(
        "assets/fountain-v14/fountain-v14-cache-lifecycle.json"
    ))
    .unwrap();
    let sources = inputs["strokes"].as_array().unwrap();
    let context = InkRasterContext {
        width: 128,
        height: 256,
        pixels_per_unit: 1,
        origin: InkRasterOrigin::TopLeft,
    };
    let captures = inputs["captures"].as_array().unwrap();
    assert_eq!(captures.len(), 3);
    for (receipt, sequence) in
        captures
            .iter()
            .zip([&[29, 3, 29, 30][..], &[29, 3, 30][..], &[29, 30, 30][..]])
    {
        let mut canvas =
            InkRasterCanvas::from_pixels(context, pixels(&receipt["prior_inputs"]["clear"]))
                .unwrap();
        let mut cache = HashMap::new();
        let mut direction = Point { x: 0., y: 1. };
        for (step, &id) in sequence.iter().enumerate() {
            let case = &receipt["cases"][format!("scene_top_1_clear_{step}_{id}")];
            if sequence == [29, 30, 30] && step == 1 {
                cache.clear();
                direction = Point { x: 0., y: 1. };
            }
            cache.entry(id).or_insert_with(|| {
                let input = sources.iter().find(|value| value["index"] == id).unwrap();
                let stroke: Stroke = serde_json::from_value(input["stroke"].clone()).unwrap();
                let snapshot = FountainV14Prepared::new(&stroke, context, direction).unwrap();
                direction = snapshot.outgoing_direction();
                snapshot
            });
            canvas
                .paint_fountain_v14_prepared(
                    &cache[&id],
                    serde_json::from_value(case["source_origin"].clone()).unwrap(),
                    receipt["model"][id.to_string()]["color_argb"]
                        .as_u64()
                        .unwrap() as u32,
                )
                .unwrap();
            assert!(
                canvas.pixels() == pixels(&case["output"]),
                "sequence {sequence:?} step {step}"
            );
        }
    }
}

#[test]
fn fresh_pen_canvas_retains_drawing_direction_and_reuses_geometry() {
    use sdocx::FountainV14Scene;

    let inputs: Value = serde_json::from_str(include_str!(
        "assets/fountain-v14/fountain-v14-cache-lifecycle.json"
    ))
    .unwrap();
    let receipt: Value = serde_json::from_str(include_str!(
        "assets/fountain-v14/fountain-v14-fresh-canvas-native.json"
    ))
    .unwrap();
    let fresh_drawing: Value = serde_json::from_str(include_str!(
        "assets/fountain-v14/fountain-v14-fresh-drawing-native.json"
    ))
    .unwrap();
    assert_eq!(
        receipt["source_provenance"],
        fresh_drawing["source_provenance"]
    );
    assert_eq!(receipt["model"], fresh_drawing["model"]);
    assert_ne!(
        receipt["streams"]["30_1_1"]["sha256"],
        fresh_drawing["streams"]["30_1_1"]["sha256"]
    );
    assert_eq!(
        receipt["streams"]["30_1_1"]["sha256"],
        receipt["streams"]["30_1_2"]["sha256"]
    );
    assert_eq!(
        receipt["cases"]["scene_top_1_clear_2_30"]["argument_count"],
        0
    );

    let cases = &receipt["cases"];
    let first = &cases["scene_top_1_clear_0_29"];
    let second = &cases["scene_top_1_clear_1_30"];
    let third = &cases["scene_top_1_clear_2_30"];
    assert_eq!(first["drawing_identity"], second["drawing_identity"]);
    assert_eq!(second["drawing_identity"], third["drawing_identity"]);
    assert_ne!(first["pen_canvas_identity"], second["pen_canvas_identity"]);
    assert_eq!(second["pen_canvas_identity"], third["pen_canvas_identity"]);
    assert_eq!(second["source_identity"], third["source_identity"]);
    assert_eq!(second["lifecycle_event"], "replace_pen_canvas");
    for (case, counts, arguments) in [
        (first, [1, 1, 1], 12),
        (second, [2, 1, 2], 1),
        (third, [3, 1, 2], 0),
    ] {
        assert_eq!(case["native_counts"]["clear_calls"], counts[0]);
        assert_eq!(case["native_counts"]["init_calls"], counts[1]);
        assert_eq!(case["native_counts"]["canvas_calls"], counts[2]);
        assert_eq!(case["argument_count"], arguments);
        assert_eq!(case["native_draws"], 1);
    }
    assert_eq!(
        first["before_sha256"],
        receipt["prior_inputs"]["clear"]["sha256"]
    );
    assert_eq!(second["before_sha256"], first["output"]["sha256"]);
    assert_eq!(third["before_sha256"], second["output"]["sha256"]);

    let context = InkRasterContext {
        width: 128,
        height: 256,
        pixels_per_unit: 1,
        origin: InkRasterOrigin::TopLeft,
    };
    let mut canvas =
        InkRasterCanvas::from_pixels(context, pixels(&receipt["prior_inputs"]["clear"])).unwrap();
    let mut scene = FountainV14Scene::default();
    for (step, id) in [29, 30, 30].into_iter().enumerate() {
        let input = inputs["strokes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["index"] == id)
            .unwrap();
        let stroke: Stroke = serde_json::from_value(input["stroke"].clone()).unwrap();
        let case = &receipt["cases"][format!("scene_top_1_clear_{step}_{id}")];
        let prepared = scene.prepare(id, 0, &stroke, context).unwrap();
        canvas
            .paint_fountain_v14_prepared(
                prepared,
                serde_json::from_value(case["source_origin"].clone()).unwrap(),
                receipt["model"][id.to_string()]["color_argb"]
                    .as_u64()
                    .unwrap() as u32,
            )
            .unwrap();
        assert_eq!(canvas.pixels(), pixels(&case["output"]), "step {step}");
    }
}
