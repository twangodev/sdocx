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
