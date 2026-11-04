//! Test-only reconstruction checked against captured host-Mesa frames.
use super::device_raster::{self, Context};

const NATIVE_STREAMS: &[u8] = include_bytes!("../../../../../conformance/pencil2-v14-native.bin");

#[test]
fn packaged_raster_textures_match_native_uploads() {
    use sha2::{Digest, Sha256};
    assert_eq!(
        format!("{:x}", Sha256::digest(include_bytes!("assets/grain.r8"))),
        "dbe4f006debde4b765c1e5ee6966798dd8b14cdc939b149c9e22770a2c0f97f9"
    );
    assert_eq!(
        format!("{:x}", Sha256::digest(include_bytes!("assets/point.r8"))),
        "475d71a47445657c58c944fb14a4746a248b57d33aff7bff864f30dc72d9ad6f"
    );
}

fn render(stream: &[u8], scale: f32, translation_x: f32) -> Vec<u8> {
    device_raster::render(
        &device_raster::records(stream),
        Context {
            scale,
            translation: [translation_x, 0.],
            ..Context::default()
        },
    )
    .unwrap()
    .into_iter()
    .map(|rgba| rgba[3])
    .collect()
}

#[test]
fn device_profile_matches_bounded_host_controls() {
    type DeviceCase<'a> = (f32, f32, &'a [u8], &'a [(usize, usize, u8, u8)]);
    let cases: [DeviceCase<'_>; 3] = [
        (
            1.,
            0.,
            include_bytes!("../../../../../conformance/pencil2-v14-device-profile/baseline.alpha"),
            &[],
        ),
        (
            1.,
            6.,
            include_bytes!(
                "../../../../../conformance/pencil2-v14-device-profile/translate6.alpha"
            ),
            &[],
        ),
        (
            2.,
            0.,
            include_bytes!("../../../../../conformance/pencil2-v14-device-profile/scale2.alpha"),
            &[],
        ),
    ];
    assert_eq!(NATIVE_STREAMS[9408..14112].len(), 84 * 56);
    for (scale, tx, native, expected_differences) in cases {
        assert_eq!(native.len(), 62 * 22);
        let image = render(&NATIVE_STREAMS[9408..14112], scale, tx);
        let mut differences = Vec::new();
        for y in 170..192 {
            for x in 0..62 {
                let error = image[y * 192 + x].abs_diff(native[(y - 170) * 62 + x]);
                if error != 0 {
                    differences.push((x, y, image[y * 192 + x], native[(y - 170) * 62 + x]));
                }
            }
        }
        assert_eq!(differences, expected_differences);
        assert!(
            image
                .iter()
                .enumerate()
                .all(|(i, &a)| a == 0 || (i % 192 < 62 && i / 192 >= 170))
        );
    }
}

#[test]
fn source_alpha_rounding_hypothesis_matches_size2_color_controls() {
    let color = include_bytes!("../../../../../conformance/pencil2-v14-color-80336699-native.bin");
    type ColorCase<'a> = (&'a [u8], &'a [u8], usize, u32, u8);
    let cases: [ColorCase<'_>; 2] = [
        (
            &NATIVE_STREAMS[..4704],
            include_bytes!(
                "../../../../../conformance/pencil2-v14-device-profile/size2-black.alpha"
            ),
            0,
            0,
            0,
        ),
        (
            &color[..4704],
            include_bytes!(
                "../../../../../conformance/pencil2-v14-device-profile/size2-color.alpha"
            ),
            0,
            0,
            0,
        ),
    ];
    for (stream, native, expected_count, expected_sum, expected_max) in cases {
        let image = render(stream, 1., 0.);
        let errors: Vec<_> = (170..192)
            .flat_map(|y| {
                (0..62).map({
                    let image = &image;
                    move |x| image[y * 192 + x].abs_diff(native[(y - 170) * 62 + x])
                })
            })
            .collect();
        assert_eq!(
            (
                errors.iter().filter(|&&e| e != 0).count(),
                errors.iter().map(|&e| u32::from(e)).sum::<u32>(),
                *errors.iter().max().unwrap()
            ),
            (expected_count, expected_sum, expected_max)
        );
        assert!(
            image
                .iter()
                .enumerate()
                .all(|(i, &a)| a == 0 || (i % 192 < 62 && i / 192 >= 170))
        );
    }
}

#[test]
fn full_rgba_frames_match_native_transform_receipts() {
    use sha2::{Digest, Sha256};
    let receipt: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../conformance/pencil2-v14-device-profile/receipt.json"
    ))
    .unwrap();
    for case in receipt["cases"].as_array().unwrap() {
        let rgba = device_raster::render(
            &device_raster::records(&NATIVE_STREAMS[9408..14112]),
            Context {
                scale: case["scale"].as_f64().unwrap() as f32,
                translation: [case["translation_x"].as_f64().unwrap() as f32, 0.],
                ..Context::default()
            },
        )
        .unwrap();
        let bottom_origin: Vec<u8> = rgba
            .as_chunks::<192>()
            .0
            .iter()
            .rev()
            .flatten()
            .flatten()
            .copied()
            .collect();
        assert_eq!(
            format!("{:x}", Sha256::digest(&bottom_origin)),
            case["full_rgba_sha256"].as_str().unwrap(),
            "{}",
            case["name"]
        );
    }
}
