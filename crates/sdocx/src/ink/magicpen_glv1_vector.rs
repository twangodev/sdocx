//! Scalable vector contours for MagicPen GLV1 stamps.
//!
//! The polygon is a source-derived approximation of native radial coverage:
//! at canonical 1 px/source-unit scale, its area on each ray equals the
//! integral of the native linear radial field. It is not pointwise exact and
//! does not follow the native shader's scale-dependent finite 2x2 `fwidth`.

const CONTOUR_VERTICES: usize = 192;
const MAX_RECORDS: usize = 65_536;
const MAX_PATH_BYTES: usize = 8 * 1024 * 1024;

pub(super) fn source_contour_path(records: &[[u32; 3]], scale: u32) -> Option<String> {
    if !matches!(scale, 1 | 2 | 4) {
        return None;
    }
    if records.is_empty() || records.len() > MAX_RECORDS {
        return None;
    }

    let mut path = String::new();
    for &[x_bits, y_bits, radius_bits] in records {
        let x = f32::from_bits(x_bits) as f64;
        let y = f32::from_bits(y_bits) as f64;
        let radius = f32::from_bits(radius_bits) as f64;
        if !x.is_finite() || !y.is_finite() || !radius.is_finite() || radius < 0.0 {
            return None;
        }

        for vertex in 0..CONTOUR_VERTICES {
            let angle = 2.0 * std::f64::consts::PI * vertex as f64 / CONTOUR_VERTICES as f64;
            let (sin, cos) = angle.sin_cos();
            // Preserve the radial integral of the canonical linear coverage
            // ramp. This retains small stamps whose half-coverage contour
            // has zero area, without fitting a captured pixel boundary.
            let edge = (cos.abs() + sin.abs()) / f64::from(scale);
            let rho2 = if radius >= edge {
                radius * radius - radius * edge + edge * edge / 3.0
            } else {
                radius * radius * radius / (3.0 * edge)
            };
            let contour_radius = rho2.max(0.0).sqrt();
            let px = x + contour_radius * cos;
            let py = y + contour_radius * sin;
            if !px.is_finite() || !py.is_finite() {
                return None;
            }
            if vertex == 0 {
                use std::fmt::Write as _;
                write!(&mut path, "M{px:.8},{py:.8}").ok()?;
            } else {
                use std::fmt::Write as _;
                write!(&mut path, " L{px:.8},{py:.8}").ok()?;
            }
            if path.len() > MAX_PATH_BYTES {
                return None;
            }
        }
        path.push_str(" Z");
        if path.len() > MAX_PATH_BYTES {
            return None;
        }
    }
    Some(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn native_edge_translation_preserves_overlapping_pixels() {
        use base64::Engine;
        use sha2::{Digest, Sha256};
        use std::io::Read;

        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/magicpen-phase-export-host.json"
        ))
        .unwrap();
        let controls = &fixture["edge_translation_controls"];
        let originals = controls["native"].as_array().unwrap();
        let translations = controls["translated"].as_array().unwrap();
        assert_eq!(originals.len(), 6);
        assert_eq!(translations.len(), 6);

        fn find<'a>(cases: &'a [Value], name: &str, scale: usize) -> &'a Value {
            let mut found = cases
                .iter()
                .filter(|case| case["case"] == name && case["scale"] == scale);
            let case = found.next().expect("missing native edge control");
            assert!(found.next().is_none(), "duplicate native edge control");
            case
        }

        let decode = |case: &Value, data_key: &str, hash_key: &str, size: usize| {
            let compressed = base64::engine::general_purpose::STANDARD
                .decode(case[data_key].as_str().unwrap())
                .unwrap();
            let mut bytes = Vec::new();
            flate2::read::ZlibDecoder::new(compressed.as_slice())
                .take((size + 1) as u64)
                .read_to_end(&mut bytes)
                .unwrap();
            assert_eq!(bytes.len(), size);
            assert_eq!(format!("{:x}", Sha256::digest(&bytes)), case[hash_key]);
            bytes
        };

        for (name, shift) in [("left_edge", 8_isize), ("right_edge", -8)] {
            for scale in [1_usize, 2, 4] {
                let original = find(originals, name, scale);
                let translated = find(translations, name, scale);
                assert_eq!(translated["shift"], shift);
                assert_eq!(translated["overlap_rgba_error"], 0);
                assert_eq!(translated["overlap_changed_pixels"], 0);
                let width = 32 * scale;
                let size = width * width * 4;
                let before = decode(
                    original,
                    "transparent_rgba_zlib",
                    "transparent_sha256",
                    size,
                );
                let after = decode(
                    translated,
                    "translated_rgba_zlib",
                    "translated_sha256",
                    size,
                );
                let _opaque = decode(original, "opaque_rgba_zlib", "opaque_sha256", size);
                for y in 0..width {
                    for x in 0..width {
                        let translated_x = x as isize + shift * scale as isize;
                        if (0..width as isize).contains(&translated_x) {
                            let left = (y * width + x) * 4;
                            let right = (y * width + translated_x as usize) * 4;
                            assert_eq!(
                                &before[left..left + 4],
                                &after[right..right + 4],
                                "{name} at {scale}x, pixel ({x}, {y})"
                            );
                        }
                    }
                }
            }
        }
    }

    fn float_records(hex: &str) -> Vec<[u32; 3]> {
        let bytes = hex.as_bytes();
        assert_eq!(bytes.len() % 24, 0);
        bytes
            .as_chunks::<24>()
            .0
            .iter()
            .map(|record| {
                std::array::from_fn(|index| {
                    let start = index * 8;
                    u32::from_str_radix(std::str::from_utf8(&record[start..start + 8]).unwrap(), 16)
                        .unwrap()
                        .swap_bytes()
                })
            })
            .collect()
    }

    #[test]
    fn diagnostic_contour_uses_frozen_native_v1_buffer_and_stays_bounded() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/magicpen-paint-controls-native.json"
        ))
        .unwrap();
        let control = fixture["controls"]
            .as_array()
            .unwrap()
            .iter()
            .find(|control| control["id"] == "identity-v1-transparent")
            .unwrap();
        let paint = &control["paint"];
        let records = float_records(paint["native_vector_hex"].as_str().unwrap());
        assert_eq!(records.len(), 8);
        assert_eq!(paint["native_vector_bytes"], 96);
        assert_eq!(
            paint["native_vector_sha256"].as_str(),
            Some("48fab498fb7b870d2fb0ebcf3d725efbcf1352324592286aa9082df635b1377d")
        );
        let path = source_contour_path(&records, 1).unwrap();
        assert_eq!(path.matches('M').count(), records.len());
        assert_eq!(path.matches('Z').count(), records.len());
        assert!(path.len() <= MAX_PATH_BYTES);

        let source: Value = serde_json::from_str(include_str!(
            "../../../../conformance/magicpen-glv1-source-native.json"
        ))
        .unwrap();
        let baseline = source["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|case| case["id"] == "baseline")
            .unwrap();
        let native = baseline["native"]["xyz_f32_u32_hex"]
            .as_array()
            .unwrap()
            .as_chunks::<3>()
            .0
            .iter()
            .map(|xyz| {
                xyz.iter()
                    .map(|word| u32::from_str_radix(word.as_str().unwrap(), 16).unwrap())
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap()
            })
            .collect::<Vec<[u32; 3]>>();
        assert_eq!(source_contour_path(&native, 1), Some(path));

        assert!(source_contour_path(&[], 1).is_none());
        assert!(source_contour_path(&[[0, 0, f32::NAN.to_bits()]], 1).is_none());
        assert!(source_contour_path(&[[0, 0, (-1.0_f32).to_bits()]], 1).is_none());
    }
}
