//! Scalable vector contours for MagicPen GLV1 stamps.
//!
//! The polygon is a scalable approximation of the native mask shader's
//! half-coverage contour. It is not pixel-exact after arbitrary SVG/PDF
//! transforms because the native shader recomputes finite 2x2 `fwidth`.

const CONTOUR_VERTICES: usize = 192;
const MAX_RECORDS: usize = 65_536;
const MAX_PATH_BYTES: usize = 8 * 1024 * 1024;

pub(super) fn source_contour_path(records: &[[u32; 3]]) -> Option<String> {
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
            // This is the native shader's smoothAlpha == 0.5 contour for an
            // identity 1 px/source-unit transform. A fixed SVG contour then
            // scales with the document, unlike native per-view `fwidth`.
            let contour_radius = (radius - 0.5 * (cos.abs() + sin.abs())).max(0.0);
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

    fn float_records(hex: &str) -> Vec<[u32; 3]> {
        let bytes = hex.as_bytes();
        assert_eq!(bytes.len() % 24, 0);
        bytes
            .chunks_exact(24)
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
        let path = source_contour_path(&records).unwrap();
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
            .chunks_exact(3)
            .map(|xyz| {
                xyz.iter()
                    .map(|word| u32::from_str_radix(word.as_str().unwrap(), 16).unwrap())
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap()
            })
            .collect::<Vec<[u32; 3]>>();
        assert_eq!(source_contour_path(&native), Some(path));

        assert!(source_contour_path(&[]).is_none());
        assert!(source_contour_path(&[[0, 0, f32::NAN.to_bits()]]).is_none());
        assert!(source_contour_path(&[[0, 0, (-1.0_f32).to_bits()]]).is_none());
    }
}
