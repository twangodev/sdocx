// Bounded comparison against synthetic native host Mesa captures; not device parity.
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
fn nominal_period_translation_control_compares_current_raster() {
    let receipt: serde_json::Value =
        serde_json::from_slice(include_bytes!("test_assets/periodic_edge/native.json")).unwrap();
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
        for dy in [0, -150] {
            let mut ctx = context();
            ctx.translation[1] = dy as f32;
            let image = device_raster::render(&records, ctx).unwrap();
            let frame = &receipt["results"][format!("{name}:{dy}")];
            let mut native = vec![[0u8; 4]; 192 * 192];
            for pixel in frame["pixels"].as_array().unwrap() {
                let p = pixel.as_array().unwrap();
                let x = p[0].as_u64().unwrap() as usize;
                let y = p[1].as_u64().unwrap() as usize;
                native[(191 - y) * 192 + x] =
                    std::array::from_fn(|i| p[i + 2].as_u64().unwrap() as u8);
            }
            assert_eq!(bottom_hash(&native), frame["sha256"].as_str().unwrap());
            let bytes: Vec<_> = records
                .iter()
                .flatten()
                .flat_map(|v| v.to_le_bytes())
                .collect();
            assert_eq!(
                format!("{:x}", Sha256::digest(bytes)),
                receipt["inputs"][name].as_str().unwrap()
            );
            let mut count = 0;
            let mut sum = 0;
            let mut max = 0;
            for (a, b) in image.iter().zip(&native) {
                for (&x, &y) in a.iter().zip(b) {
                    let d = x.abs_diff(y);
                    if d > 0 {
                        count += 1;
                        sum += usize::from(d);
                        max = max.max(d);
                    }
                }
            }
            let expected = match (name, dy) {
                (_, -150) => (0, 0, 0),
                ("mixed", 0) => (7, 10, 3),
                ("record_17", 0) => (3, 7, 3),
                ("record_20", 0) | ("record_35", 0) => (1, 1, 1),
                ("record_32", 0) => (2, 2, 1),
                _ => unreachable!(),
            };
            assert!(
                count <= expected.0 && sum <= expected.1 && max <= expected.2,
                "{name} {dy}: actual {count}/{sum}/{max}, maximum {expected:?}"
            );
        }
    }
}
