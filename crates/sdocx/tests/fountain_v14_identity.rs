#![cfg(all(feature = "render", feature = "serde"))]

use base64::Engine;
use flate2::read::ZlibDecoder;
use sdocx::{
    FountainV14Prepared, InkRasterCanvas, InkRasterContext, InkRasterOrigin, Point, Stroke,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::Read;

#[test]
fn caller_managed_snapshots_replay_observed_identity_and_scale_sequence() {
    let receipt: Value = serde_json::from_str(include_str!(
        "assets/fountain-v14/cache-invalidation-native.json"
    ))
    .unwrap();
    assert_eq!(receipt["model"]["29"], receipt["model"]["129"]);
    let source = include_str!("assets/fountain-v14/saved-source-29.json");
    let first: Stroke = serde_json::from_str(source).unwrap();
    let second: Stroke = serde_json::from_str(source).unwrap();
    let color = receipt["model"]["29"]["color_argb"].as_u64().unwrap() as u32;
    let mut direction = Point { x: 0., y: 1. };
    let mut snapshot = None;
    let mut pixels = vec![[0; 4]; 128 * 256];
    for (step, (id, scale, arguments, draws, regenerate)) in [
        (29, 1, 12, 1, true),
        (129, 1, 12, 1, true),
        (29, 2, 23, 0, true),
        (29, 2, 0, 0, false),
        (29, 1, 12, 1, true),
    ]
    .into_iter()
    .enumerate()
    {
        let case = &receipt["cases"][format!("scene_top_{scale}_clear_{step}_{id}")];
        assert_eq!(case["argument_count"], arguments);
        assert_eq!(case["native_draws"], draws);
        let context = InkRasterContext {
            width: 128,
            height: 256,
            pixels_per_unit: scale,
            origin: InkRasterOrigin::TopLeft,
        };
        if regenerate {
            let stroke = if id == 129 { &second } else { &first };
            let prepared = FountainV14Prepared::new(stroke, context, direction).unwrap();
            direction = prepared.outgoing_direction();
            snapshot = Some(prepared);
        }
        if draws != 0 {
            let mut canvas = InkRasterCanvas::from_pixels(context, pixels).unwrap();
            canvas
                .paint_fountain_v14_prepared(snapshot.as_ref().unwrap(), [80, 72], color)
                .unwrap();
            pixels = canvas.pixels().to_vec();
        }
        let compressed = base64::engine::general_purpose::STANDARD
            .decode(case["output"]["rgba_zlib_base64"].as_str().unwrap())
            .unwrap();
        let mut bytes = Vec::new();
        ZlibDecoder::new(compressed.as_slice())
            .read_to_end(&mut bytes)
            .unwrap();
        assert_eq!(
            format!("{:x}", Sha256::digest(&bytes)),
            case["output"]["sha256"].as_str().unwrap()
        );
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
        assert_eq!(pixels, expected, "step {step}");
    }
}
