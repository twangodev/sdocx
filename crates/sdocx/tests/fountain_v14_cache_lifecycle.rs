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
    for (receipt, sequence) in captures
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
