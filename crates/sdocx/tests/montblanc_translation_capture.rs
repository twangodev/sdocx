use serde_json::Value;
use sha2::{Digest, Sha256};

fn native_crop(receipt: &Value) -> Vec<u8> {
    let stored = &receipt["crop_storage"];
    assert_eq!(stored["encoding"], "rgba8-palette-runs-v1");
    assert_eq!(stored["width"], 23);
    assert_eq!(stored["height"], 7);
    let values = stored["palette"].as_array().unwrap();
    assert!(!values.is_empty() && values.len() <= 161);
    let palette: Vec<[u8; 4]> = values
        .iter()
        .map(|value| {
            let rgba = value.as_array().unwrap();
            assert_eq!(rgba.len(), 4);
            std::array::from_fn(|i| u8::try_from(rgba[i].as_u64().unwrap()).unwrap())
        })
        .collect();
    let mut bytes = Vec::with_capacity(23 * 7 * 4);
    for run in stored["runs"].as_array().unwrap() {
        let run = run.as_array().unwrap();
        assert_eq!(run.len(), 2);
        let count = usize::try_from(run[0].as_u64().unwrap()).unwrap();
        assert!(count > 0 && count <= (23 * 7 * 4 - bytes.len()) / 4);
        let index = usize::try_from(run[1].as_u64().unwrap()).unwrap();
        let rgba = palette.get(index).expect("valid native palette index");
        for _ in 0..count {
            bytes.extend_from_slice(rgba);
        }
    }
    assert_eq!(bytes.len(), 23 * 7 * 4);
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        receipt["crop_rgba_sha256"].as_str().unwrap()
    );
    bytes
}

fn frame(crop: &[u8], left: usize, top: usize) -> Vec<u8> {
    assert_eq!(crop.len(), 23 * 7 * 4);
    let mut frame = vec![0; 256 * 256 * 4];
    for y in 0..7 {
        let offset = ((top + y) * 256 + left) * 4;
        frame[offset..offset + 23 * 4].copy_from_slice(&crop[y * 23 * 4..(y + 1) * 23 * 4]);
    }
    frame
}

#[test]
fn native_montblanc_integer_translation_moves_every_pixel() {
    let receipt: Value = serde_json::from_str(include_str!(
        "../../../conformance/montblanc-native-translation.json"
    ))
    .unwrap();
    let crop = native_crop(&receipt);
    let baseline = frame(&crop, 127, 126);
    let translated = frame(&crop, 143, 134);
    // Independently captured frame identities prevent this becoming only a
    // comparison between two placements of the same decoded crop.
    for (bytes, key) in [
        (&baseline, "baseline_rgba8_sha256"),
        (&translated, "translated_rgba8_sha256"),
    ] {
        assert_eq!(
            format!("{:x}", Sha256::digest(bytes)),
            receipt[key].as_str().unwrap()
        );
    }
    let mut shifted = vec![0; baseline.len()];
    for y in 0..248 {
        let source = y * 256 * 4;
        let dest = ((y + 8) * 256 + 16) * 4;
        shifted[dest..dest + 240 * 4].copy_from_slice(&baseline[source..source + 240 * 4]);
    }
    assert_eq!(shifted, translated);
}
