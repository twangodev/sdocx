use crate::ink::host_rgba8::{over, quantize, sample_unorm8_half_rtz};
use base64::Engine;
use flate2::read::ZlibDecoder;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::Read;

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../../conformance/fountain-v14-real-source-rgba-native.json"
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
