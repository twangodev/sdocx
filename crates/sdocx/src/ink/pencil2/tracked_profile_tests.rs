// Bounded native host-profile controls; these do not establish general saved-page parity.
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
    schema: String,
    frame: [u32; 2],
    inputs: BTreeMap<String, String>,
    pub results: BTreeMap<String, Frame>,
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
fn repeated_native_record_retains_rgb_independent_of_final_alpha() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../conformance/pencil2-v14-rgb-recurrence-native.json"
    ))
    .unwrap();
    let results = fixture["results"].as_array().unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(fixture["repetitions"], 84);
    for result in results {
        let outcomes = result["outcomes"].as_array().unwrap();
        assert_eq!(outcomes.len(), 3);
        for outcome in outcomes {
            let source = std::array::from_fn(|i| outcome["single"][i].as_u64().unwrap() as u8);
            let native: [u8; 4] =
                std::array::from_fn(|i| outcome["native_84"][i].as_u64().unwrap() as u8);
            let mut destination = [0; 4];
            for _ in 0..84 {
                device_raster::over(&mut destination, source);
            }
            assert_eq!(destination, native);
        }
    }
    let mut destination = [0; 4];
    for _ in 0..84 {
        device_raster::over(&mut destination, [2, 4, 6, 10]);
    }
    assert_eq!(destination, [39, 90, 141, 243]);
    assert_ne!(&destination[..3], &[49, 97, 146]);
}

#[test]
fn page_coordinate_matrices_match_bounded_native_controls() {
    let receipt = page_context_receipt();
    assert_eq!(receipt.schema, "sdocx-pencil2-v14-page-context-native-v1");
    assert_eq!(receipt.frame, [192, 192]);
    assert_eq!(receipt.results.len(), 52);
    for name in ["colored", "mixed", "mixed_reversed", "inverse_half"] {
        let bytes: Vec<_> = control_records(name)
            .iter()
            .flatten()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        assert_eq!(
            format!("{:x}", Sha256::digest(bytes)),
            receipt.inputs[name],
            "{name}"
        );
    }
    for (id, target) in &receipt.results {
        let (name, rest) = id
            .split_once("_top_scale")
            .or_else(|| id.split_once("_bottom_scale"))
            .unwrap();
        let scale = rest.as_bytes()[0] - b'0';
        let context = Context {
            scale: f32::from(scale),
            y_up: id.contains("_bottom_scale"),
            ..Context::default()
        };
        let backdrop = vec![target.background; 192 * 192];
        let image =
            device_raster::render_with_backdrop(&control_records(name), context, Some(&backdrop))
                .unwrap();
        let errors = error(&image, &target.image());
        if id.contains("_top_scale2") && name != "inverse_half" {
            assert!(errors.2 <= 3 && errors.1 <= 12, "{id}: {errors:?}");
        } else {
            assert_eq!(errors, (0, 0, 0), "{id}");
        }
    }
}

#[test]
fn raster_preflight_rejects_invalid_channels_dimensions_and_work() {
    let records = control_records("mixed");
    for frame in [
        [0, 192],
        [192, 0],
        [u32::MAX, u32::MAX],
        [8193, 1],
        [4096, 4096],
    ] {
        assert!(
            device_raster::render(
                &records,
                Context {
                    frame,
                    ..Context::default()
                }
            )
            .is_none()
        );
    }
    for scale in [f32::NAN, f32::INFINITY, 0., -1.] {
        assert!(
            device_raster::render(
                &records,
                Context {
                    scale,
                    ..Context::default()
                }
            )
            .is_none()
        );
    }
    assert!(
        device_raster::render(
            &records,
            Context {
                translation: [f32::NAN, 0.],
                ..Context::default()
            }
        )
        .is_none()
    );
    for lane in 0..14 {
        let mut invalid = records.clone();
        invalid[83][lane] = f32::NAN;
        assert!(device_raster::render(&invalid, Context::default()).is_none());
    }
    let mut large = records[40];
    large[0] = 96.;
    large[1] = 96.;
    large[2] = 100.;
    large[3] = 100.;
    assert!(device_raster::render(&vec![large; 456], Context::default()).is_none());
    assert!(device_raster::render(&vec![records[40]; 16_385], Context::default()).is_none());
    assert!(
        device_raster::render_with_backdrop(&records, Context::default(), Some(&[[255; 4]; 10]))
            .is_none()
    );
    let invisible = Context {
        translation: [-1000., -1000.],
        ..Context::default()
    };
    assert!(
        device_raster::render(&records, invisible)
            .unwrap()
            .iter()
            .all(|p| *p == [0; 4])
    );
}
