use super::device_raster::{self, Context};
use sha2::{Digest, Sha256};
use std::io::Read;

#[test]
fn isolated_record_native_comparison() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/ink/pencil2/test_assets/isolated_record");
    let record_bytes = std::fs::read(dir.join("fragment-isolated-record.bin")).unwrap();
    let records = device_raster::records(&record_bytes);
    assert_eq!(records.len(), 1);
    for name in ["transparent", "opaque", "translucent"] {
        let receipt: serde_json::Value = serde_json::from_slice(
            &std::fs::read(dir.join(format!("{name}-receipt.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(
            format!("{:x}", Sha256::digest(&record_bytes)),
            receipt["record_sha256"].as_str().unwrap()
        );
        let read = |suffix: &str| {
            let bytes = std::fs::read(
                dir.join(format!("fragment-isolated-record-{name}{suffix}.rgba.zlib")),
            )
            .unwrap();
            let mut raw = Vec::new();
            flate2::read::ZlibDecoder::new(&bytes[..])
                .read_to_end(&mut raw)
                .unwrap();
            assert_eq!(raw.len(), 192 * 192 * 4);
            let expected_hash = if suffix.is_empty() {
                &receipt["cases"]["1"]["native"]["sha256"]
            } else {
                &receipt["prior_readback_sha256"]
            };
            assert_eq!(
                format!("{:x}", Sha256::digest(&raw)),
                expected_hash.as_str().unwrap()
            );
            raw.as_chunks::<{ 192 * 4 }>()
                .0
                .iter()
                .rev()
                .flat_map(|row| row.as_chunks::<4>().0.iter().copied())
                .collect::<Vec<_>>()
        };
        let prior = read("-prior");
        let native = read("");
        let rust = device_raster::render_with_backdrop(
            &records,
            Context {
                translation: [12., 12.],
                grain_offset: [132.41742, 138.9748],
                ..Context::default()
            },
            Some(&prior),
        )
        .unwrap();
        let diffs: Vec<_> = native
            .iter()
            .zip(&rust)
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, (a, b))| (i % 192, 191 - i / 192, *a, *b))
            .collect();
        assert_eq!(native[(191 - 150) * 192 + 129], [247, 186, 0, 255]);
        assert_eq!(rust[(191 - 150) * 192 + 129], [247, 186, 0, 255]);
        assert_eq!(diffs.len(), 0, "{name}: {diffs:?}");
    }
}
