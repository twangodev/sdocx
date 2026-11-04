use super::device_raster::{self, Context};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

const NATIVE: &[u8] = include_bytes!("../../../../../conformance/pencil2-v14-native.bin");
const COLORED: &[u8] =
    include_bytes!("../../../../../conformance/pencil2-v14-color-80336699-native.bin");
const SECOND: &[u8] =
    include_bytes!("../../../../../conformance/pencil2-v14-color-ffdd4411-native.bin");

#[derive(Deserialize)]
pub(crate) struct Frame {
    sha256: String,
    background: [u8; 4],
    pixels: Vec<[u32; 6]>,
}

impl Frame {
    pub(crate) fn image(&self) -> Vec<[u8; 4]> {
        let mut image = vec![self.background; 192 * 192];
        for &[x, y, r, g, b, a] in &self.pixels {
            assert!(x < 192 && y < 192);
            image[((191 - y) * 192 + x) as usize] = [r, g, b, a].map(|v| u8::try_from(v).unwrap());
        }
        let bytes: Vec<_> = image
            .as_chunks::<192>()
            .0
            .iter()
            .rev()
            .flatten()
            .flatten()
            .copied()
            .collect();
        assert_eq!(format!("{:x}", Sha256::digest(bytes)), self.sha256);
        image
    }
}

#[derive(Deserialize)]
pub(crate) struct Receipt {
    pub results: BTreeMap<String, Frame>,
}

pub(crate) fn receipt() -> Receipt {
    serde_json::from_str(include_str!(
        "../../../../../conformance/pencil2-v14-mixed-rgb-native.json"
    ))
    .unwrap()
}

pub(crate) fn page_context_receipt() -> Receipt {
    serde_json::from_str(include_str!(
        "../../../../../conformance/pencil2-v14-page-context-native.json"
    ))
    .unwrap()
}

pub(crate) fn control_records(name: &str) -> Vec<[f32; 14]> {
    if name == "inverse_half" {
        return device_raster::records(include_bytes!(
            "../../../../../conformance/pencil2-v14-inverse-half-native.bin"
        ));
    }
    let colored = device_raster::records(&COLORED[..4704]);
    if name == "colored" {
        return colored;
    }
    let sources = [
        colored,
        device_raster::records(SECOND),
        device_raster::records(&NATIVE[14112..18816]),
        device_raster::records(&NATIVE[18816..23520]),
        device_raster::records(&NATIVE[23520..28224]),
    ];
    let mut mixed: Vec<_> = (0..84).map(|i| sources[i % 5][i]).collect();
    if name == "mixed_reversed" {
        mixed.reverse();
    } else {
        assert_eq!(name, "mixed");
    }
    mixed
}

pub(crate) fn error(image: &[[u8; 4]], native: &[[u8; 4]]) -> (usize, u64, u8) {
    assert_eq!(image.len(), native.len());
    let values: Vec<_> = image
        .iter()
        .flatten()
        .zip(native.iter().flatten())
        .map(|(&a, &b)| a.abs_diff(b))
        .collect();
    (
        values.iter().filter(|&&v| v != 0).count(),
        values.iter().map(|&v| u64::from(v)).sum(),
        *values.iter().max().unwrap(),
    )
}

#[test]
fn host_byte_blending_matches_varying_native_records_on_all_backgrounds() {
    let receipt = receipt();
    assert_eq!(receipt.results.len(), 264);
    assert_eq!(
        format!("{:x}", Sha256::digest(SECOND)),
        "5a1a5c3a88b3b20d67043d76be1410878cb2093f4029a67635be2be50df29483"
    );
    for name in ["colored", "mixed", "mixed_reversed"] {
        for suffix in ["", "_background", "_white", "_paper"] {
            let target = &receipt.results[&format!("{name}{suffix}")];
            let mut image = vec![target.background; 192 * 192];
            let mut rounded_ratio = image.clone();
            for i in 0..84 {
                let (source, index) = if name == "mixed_reversed" {
                    ("mixed", 83 - i)
                } else {
                    (name, i)
                };
                let single = receipt.results[&format!("{source}_single_{index}")].image();
                for ((d, ratio), s) in image.iter_mut().zip(&mut rounded_ratio).zip(single) {
                    device_raster::over(d, s);
                    for (channel, &source) in ratio.iter_mut().zip(&s) {
                        *channel = (u32::from(source)
                            + (u32::from(*channel) * (255 - u32::from(s[3])) + 127) / 255)
                            .min(255) as u8;
                    }
                }
            }
            let expected = match (name, suffix) {
                ("mixed", "_white") => (6, 6, 1),
                ("mixed_reversed", "_white") => (4, 4, 1),
                _ => (0, 0, 0),
            };
            let native = target.image();
            assert_eq!(
                error(&rounded_ratio, &native),
                expected,
                "rounded ratio: {name}{suffix}"
            );
            assert_eq!(
                error(&image, &native),
                (0, 0, 0),
                "host bytes: {name}{suffix}"
            );
        }
    }
    let forward = receipt.results["mixed"].image();
    let reversed = receipt.results["mixed_reversed"].image();
    assert_ne!(error(&forward, &reversed), (0, 0, 0));
}

#[test]
fn isolated_white_stamps_match_the_same_host_blend_profile() {
    let receipt = receipt();
    for i in 0..84 {
        let single = receipt.results[&format!("mixed_single_{i}")].image();
        let white = receipt.results[&format!("mixed_white_single_{i}")].image();
        let modeled: Vec<_> = single
            .into_iter()
            .map(|source| {
                let mut destination = [255; 4];
                device_raster::over(&mut destination, source);
                destination
            })
            .collect();
        assert_eq!(error(&modeled, &white), (0, 0, 0), "record {i}");
    }
}

#[test]
fn fixed_resolution_rgba_model_against_mixed_native_frames() {
    let receipt = receipt();
    for (name, expected) in [
        ("colored", (0, 0, 0)),
        ("mixed", (0, 0, 0)),
        ("mixed_reversed", (0, 0, 0)),
    ] {
        let image = device_raster::render(&control_records(name), Context::default()).unwrap();
        let errors = error(&image, &receipt.results[name].image());
        assert_eq!(errors, expected, "{name}");
    }
}

#[test]
fn scalar_stamp_compositing_uses_the_existing_backdrop() {
    let receipt = receipt();
    for name in ["colored", "mixed", "mixed_reversed"] {
        let records = control_records(name);
        for suffix in ["", "_background", "_white", "_paper"] {
            let native = &receipt.results[&format!("{name}{suffix}")];
            let backdrop = vec![native.background; 192 * 192];
            let image =
                device_raster::render_with_backdrop(&records, Context::default(), Some(&backdrop))
                    .unwrap();
            let errors = error(&image, &native.image());
            assert_eq!(errors, (0, 0, 0), "{name}{suffix}");
        }
    }
}

#[test]
fn existing_canvas_preserves_stamp_order_across_submission_boundaries() {
    let records = control_records("mixed");
    let backdrop = vec![[64, 32, 96, 128]; 192 * 192];
    let full =
        device_raster::render_with_backdrop(&records, Context::default(), Some(&backdrop)).unwrap();
    let mut submitted = backdrop;
    for chunk in records.chunks(13) {
        submitted =
            device_raster::render_with_backdrop(chunk, Context::default(), Some(&submitted))
                .unwrap();
    }
    assert_eq!(submitted, full);
    assert_eq!(
        error(&submitted, &receipt().results["mixed_background"].image()),
        (0, 0, 0)
    );
}
