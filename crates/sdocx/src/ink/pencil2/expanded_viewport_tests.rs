use super::device_raster::{self, Context};
use super::mixed_rgb_tests::control_records;
use sha2::{Digest, Sha256};

fn context() -> Context {
    Context {
        scale: 2.,
        y_up: false,
        ..Context::default()
    }
}

fn bottom_hash(image: &[[u8; 4]]) -> String {
    let mut digest = Sha256::new();
    for row in image.as_chunks::<192>().0.iter().rev() {
        for pixel in row {
            digest.update(pixel);
        }
    }
    format!("{:x}", digest.finalize())
}

#[test]
fn expanded_viewport_control_compares_current_raster() {
    let receipt: serde_json::Value =
        serde_json::from_slice(include_bytes!("test_assets/expanded_viewport/native.json"))
            .unwrap();
    for name in ["mixed", "record_17", "record_20", "record_32", "record_35"] {
        let mut records = control_records("mixed");
        if name != "mixed" {
            let ordinal: usize = name[7..].parse().unwrap();
            for (i, row) in records.iter_mut().enumerate() {
                if i != ordinal - 1 {
                    row[4] = 0.;
                }
            }
        }
        let image = device_raster::render(&records, context()).unwrap();
        let frame = &receipt["results"][format!("{name}:1")];
        let mut native = vec![[0u8; 4]; 192 * 192];
        for pixel in frame["pixels"].as_array().unwrap() {
            let p = pixel.as_array().unwrap();
            let x = p[0].as_u64().unwrap() as usize;
            let y = p[1].as_u64().unwrap() as usize;
            native[(191 - y) * 192 + x] = std::array::from_fn(|i| p[i + 2].as_u64().unwrap() as u8);
        }
        assert_eq!(bottom_hash(&native), frame["sha256"].as_str().unwrap());
        assert_eq!(image, native, "{name}");
    }
}
