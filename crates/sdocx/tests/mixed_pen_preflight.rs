#![cfg(feature = "render")]

use base64::Engine as _;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::Read as _;

#[test]
fn native_loader_constructor_and_factory_evidence_remains_consistent() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/mixed-pen-preflight-native.json")).unwrap();
    let originals = &fixture["originals"];
    let read = |name: &str| -> Value {
        let original = &originals[name];
        let data = base64::engine::general_purpose::STANDARD
            .decode(original["gzip_base64"].as_str().unwrap())
            .unwrap();
        let mut raw = Vec::new();
        flate2::read::GzDecoder::new(data.as_slice())
            .read_to_end(&mut raw)
            .unwrap();
        assert_eq!(format!("{:x}", Sha256::digest(&raw)), original["sha256"]);
        serde_json::from_slice(&raw).unwrap()
    };
    let relocation = read("mixed-pen-relocation-preflight/receipt.json");
    let libraries = relocation["libraries"].as_array().unwrap();
    assert_eq!(libraries.len(), 10);
    assert_eq!(
        libraries
            .iter()
            .map(|v| v["writes"].as_u64().unwrap())
            .sum::<u64>(),
        65_290
    );
    assert_eq!(
        libraries
            .iter()
            .map(|v| v["unresolved_relocations"].as_u64().unwrap())
            .sum::<u64>(),
        8_025
    );

    let mut preflight_hash = None;
    for (file, label, names) in [
        (
            "mixed-pen-factory-probe/forward.json",
            "factory",
            ["FountainPen", "Pencil2"],
        ),
        (
            "mixed-pen-factory-probe/reverse.json",
            "factory",
            ["Pencil2", "FountainPen"],
        ),
        (
            "mixed-pen-constructor-probe/receipt.json",
            "constructor",
            ["FountainPen", "Pencil2"],
        ),
        (
            "mixed-pen-constructor-probe/reverse-receipt.json",
            "constructor",
            ["Pencil2", "FountainPen"],
        ),
    ] {
        let capture = read(file);
        assert_eq!(capture["success"], true);
        let hash = capture["preflight_sha256"].as_str().unwrap();
        match &preflight_hash {
            None => preflight_hash = Some(hash.to_owned()),
            Some(expected) => assert_eq!(hash, expected),
        }
        let attempts = capture["attempts"].as_array().unwrap();
        assert_eq!(attempts.len(), 2);
        for (attempt, name) in attempts.iter().zip(names) {
            assert_eq!(attempt[label], name);
            assert_eq!(attempt["outcome"]["kind"], "returned");
            if label == "factory" {
                assert_ne!(attempt["outcome"]["result"], "0x0");
                assert_ne!(attempt["outcome"]["vtable"], "0x0");
            }
        }
    }
    let limited = read("mixed-pen-constructor-probe/reverse-limit-receipt.json");
    assert_eq!(limited["success"], false);
    assert_eq!(limited["instruction_limit"], 1);
    assert_eq!(limited["attempts"].as_array().unwrap().len(), 1);
    assert_eq!(
        limited["attempts"][0]["outcome"]["kind"],
        "instruction_limit"
    );
}

#[test]
fn native_factory_draw_diagnostics_keep_failure_and_sparse_frame() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/mixed-pen-preflight-native.json")).unwrap();
    let read = |name: &str| -> Value {
        let original = &fixture["originals"][format!("mixed-pen-pencil-factory-probe/{name}")];
        let data = base64::engine::general_purpose::STANDARD
            .decode(original["gzip_base64"].as_str().unwrap())
            .unwrap();
        let mut raw = Vec::new();
        flate2::read::GzDecoder::new(data.as_slice())
            .read_to_end(&mut raw)
            .unwrap();
        assert_eq!(format!("{:x}", Sha256::digest(&raw)), original["sha256"]);
        serde_json::from_slice(&raw).unwrap()
    };
    let failed = read("mesa-factory-draw-attempt.json");
    assert!(
        failed["failure"]
            .as_str()
            .unwrap()
            .contains("did not complete")
    );
    assert_eq!(failed["pixel_readback"], false);
    assert_eq!(failed["teardown_observed"], false);
    let drawing = read("mesa-factory-drawing-final-receipt.json");
    assert_eq!(drawing["exit_code"], 0);
    assert_eq!(drawing["first_gl_error"], 0);
    assert_eq!(drawing["second_gl_error"], 0);
    assert_eq!(drawing["second"]["drawing_result"], 1);
    assert_eq!(drawing["second"]["geometry_bytes"], 4592);
    assert_ne!(drawing["first_sha256"], drawing["second_sha256"]);
    let style = read("mesa-factory-style-control-receipt.json");
    assert_eq!(style["style"]["argb"], "0xff000000");
    let capture = &style["readback"];
    assert_eq!(capture["bytes"], 192 * 192 * 4);
    let pixels = capture["nonzero_pixel_rgba"].as_array().unwrap();
    assert_eq!(
        pixels.len(),
        capture["nonzero_pixels"].as_u64().unwrap() as usize
    );
    let mut rgba = vec![0_u8; 192 * 192 * 4];
    for pixel in pixels {
        let x = pixel[0].as_u64().unwrap() as usize;
        let y = pixel[1].as_u64().unwrap() as usize;
        assert!(x < 192 && y < 192);
        for c in 0..4 {
            rgba[(y * 192 + x) * 4 + c] = pixel[c + 2].as_u64().unwrap() as u8;
        }
    }
    assert_eq!(format!("{:x}", Sha256::digest(&rgba)), capture["sha256"]);
}
