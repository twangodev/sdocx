//! Bounded test-only checks for selected Oblique GLV1 path sampling.
//!
//! This is deliberately not a saved-stroke adapter. The native V1 evidence
//! currently pins the first quadratic path of the three-point curve fixture;
//! short-stroke admission and the straight-path parameterization are kept out
//! until their state transitions have a source-level model.

use super::path::{P, Quad};

const MAX_SAMPLES: usize = 1_000_000;

fn repeat_count(size: f32, length: f32) -> Option<usize> {
    if !size.is_finite() || !length.is_finite() || size < 0.0 || length < 0.0 {
        return None;
    }
    let scaled = if size <= 5.0 {
        length * 0.5
    } else if size <= 10.0 {
        length / 3.0
    } else if size <= 15.0 {
        length * 0.25
    } else if size <= 20.0 {
        length / 5.0
    } else if size <= 30.0 {
        length * 0.125
    } else if size <= 40.0 {
        length / 7.0
    } else if size <= 55.0 {
        length / 9.0
    } else {
        length / 10.0
    };
    // Native converts the scaled value with FCVTZS into a signed W register.
    // Keep conversion overflow outside this bounded finite domain.
    if !scaled.is_finite() || scaled >= i32::MAX as f32 {
        return None;
    }
    let count = (scaled.trunc().max(2.0) as usize).checked_add(10)?;
    (count <= MAX_SAMPLES).then_some(count)
}

/// Return the `getPosTan` positions emitted by one native V1 redrawLine path.
/// The caller's endpoint `drawPoint` is separate and intentionally omitted.
fn quadratic_path_samples(start: P, control: P, end: P, size: f32) -> Option<Vec<P>> {
    let path = Quad::new(start, control, end);
    if path.length == 0.0 || !path.length.is_finite() {
        return None;
    }
    let count = repeat_count(size, path.length)?;
    let step = path.length / count as f32;
    let mut distance = 0.0_f32;
    let mut samples = Vec::with_capacity(count);
    for _ in 0..count {
        samples.push(path.at(distance));
        distance += step;
    }
    Some(samples)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn bits(value: &Value) -> u32 {
        let hex = value.as_str().unwrap().trim_start_matches("0x");
        u32::from_le_bytes(std::array::from_fn(|i| {
            u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap()
        }))
    }

    fn word_bits(value: &Value) -> u32 {
        u32::from_str_radix(value.as_str().unwrap().trim_start_matches("0x"), 16).unwrap()
    }

    fn fixture_points(source: &Value) -> Vec<P> {
        source["points_f32_bits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|point| {
                let xy = point.as_array().unwrap();
                P {
                    x: f32::from_bits(bits(&xy[0])),
                    y: f32::from_bits(bits(&xy[1])),
                }
            })
            .collect()
    }

    #[test]
    fn native_v1_first_quadratic_samples_match_selected_saved_caller() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/oblique-saved-caller-selected-native.json"
        ))
        .unwrap();
        let case = &fixture["cases"]["three_points_curve"];
        let source = &case["input"];
        let points = fixture_points(source);
        assert_eq!(points.len(), 3);
        assert_eq!(source["advanced_setting"], Value::Null);
        assert!(source["curve_enabled"].as_bool().unwrap());

        // The V1 call-level trace (Oblique DSO SHA-256
        // 9b76bdb709db9f56c5d35a1a615fe41c57ceef73c559c91efaf5bf436f90e0ee)
        // records moveTo(p0), quadTo(p1, midpoint(p1,p2)), length 12.7047844,
        // getRepeat(length, size=4) == 16, then one separate endpoint draw.
        let endpoint = points[1].mid(points[2]);
        let samples = quadratic_path_samples(
            points[0],
            points[1],
            endpoint,
            f32::from_bits(bits(&source["pen_size_f32_bits"])),
        )
        .unwrap();
        assert_eq!(samples.len(), 16);

        let native: Vec<_> = case["callback_add_point_calls"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|call| call["phase"] == "draw_object_stroke")
            .take(samples.len())
            .collect();
        assert_eq!(native.len(), samples.len());
        for (index, (actual, expected)) in samples.iter().zip(native).enumerate() {
            assert_eq!(
                actual.x.to_bits(),
                word_bits(&expected["xy_bits"][0]),
                "native first quadratic sample {index} x"
            );
            assert_eq!(
                actual.y.to_bits(),
                word_bits(&expected["xy_bits"][1]),
                "native first quadratic sample {index} y"
            );
        }
    }

    #[test]
    fn repeat_count_follows_v1_native_width_bands() {
        // These are the branch thresholds from V1 getRepeat at 0x39ebc.
        assert_eq!(repeat_count(4.0, 12.704_784).unwrap(), 16);
        assert_eq!(repeat_count(1.0, 20.0).unwrap(), 20);
        assert_eq!(repeat_count(56.0, 100.0).unwrap(), 20);
        assert_eq!(repeat_count(f32::NAN, 1.0), None);
    }
}
