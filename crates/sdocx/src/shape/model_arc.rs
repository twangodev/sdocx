//! Test-only reconstruction of Model's outer arc helper (0x211814).
//!
//! Model treats its input angles as rays from the rectangle center. It
//! intersects those rays with the ellipse, converts the intersections back to
//! unit-ellipse parameters, then delegates curve construction to the separate
//! Model unit-arc builder.

const PI: f64 = f64::from_bits(0x4009_21fb_5444_2d18);
const PI_OVER_180: f64 = f64::from_bits(0x3f91_df46_a252_9d39);
const RAD_TO_DEG: f64 = 180.0;
const MAX_INPUT_ANGLE: f32 = 360.0 * 1024.0;
const MAX_NORMALIZATION_STEPS: usize = 2049;

fn radians(degrees: f32) -> f32 {
    ((f64::from(degrees) * PI) / 180.0) as f32
}

fn sincos_radians(degrees: f32) -> f32 {
    (f64::from(degrees) * PI_OVER_180) as f32
}

fn normalize_angle(mut angle: f32) -> Option<f32> {
    if !angle.is_finite() || angle.abs() > MAX_INPUT_ANGLE * 2.0 {
        return None;
    }
    let mut steps = 0;
    while angle < 0.0 {
        if steps == MAX_NORMALIZATION_STEPS {
            return None;
        }
        angle += 360.0;
        steps += 1;
    }
    while angle >= 360.0 {
        if steps == MAX_NORMALIZATION_STEPS {
            return None;
        }
        angle -= 360.0;
        steps += 1;
    }
    Some(angle)
}

fn angle_from_vector(x: f32, y: f32) -> f32 {
    let radians = y.atan2(x);
    let degrees = radians * RAD_TO_DEG as f32;
    let mut angle = (f64::from(degrees) / PI) as f32;
    if angle < 0.0 {
        angle += 360.0;
    }
    angle
}

fn unit_vector(angle: f32) -> Option<[f32; 2]> {
    let angle = normalize_angle(angle)?;
    Some(match angle {
        0.0 => [1.0, 0.0],
        90.0 => [0.0, 1.0],
        180.0 => [-1.0, 0.0],
        270.0 => [0.0, -1.0],
        _ => {
            let (sin, cos) = sincos_radians(angle).sin_cos();
            [cos, sin]
        }
    })
}

/// The native ellipse endpoint reaches `sincosf` with the unwrapped
/// start-plus-sweep angle. Near a full turn, normalizing first changes the
/// rounded result (for example, -359.9° versus 0.1000061°).
fn unit_vector_unwrapped(angle: f32) -> [f32; 2] {
    match angle.rem_euclid(360.0) {
        0.0 => return [1.0, 0.0],
        90.0 => return [0.0, 1.0],
        180.0 => return [-1.0, 0.0],
        270.0 => return [0.0, -1.0],
        _ => {}
    }
    let (sin, cos) = sincos_radians(angle).sin_cos();
    [cos, sin]
}

/// Return the unit ellipse coordinates where a ray intersects the ellipse.
/// The native implementation evaluates this with `tanf`, an FMA/sqrt
/// denominator, and explicit quadrant/sign branches.
fn ray_parameter(angle: f32, rect: [f32; 4], rx: f32, ry: f32) -> Option<([f32; 2], f32)> {
    let angle = normalize_angle(angle)?;
    match angle {
        0.0 => return Some(([1.0, 0.0], 0.0)),
        90.0 => return Some(([0.0, 1.0], 90.0)),
        180.0 => return Some(([-1.0, 0.0], 180.0)),
        270.0 => return Some(([0.0, -1.0], 270.0)),
        _ => {}
    }
    let tangent = radians(angle).tan();
    let rx2t2 = (rx * rx * tangent) * tangent;
    let denominator = ry.mul_add(ry, rx2t2).sqrt();
    let product = rx * ry;
    let abs_x_offset = product / denominator;
    let abs_y_offset = (product * tangent.abs()) / denominator;
    let (x_sign, y_sign) = match angle {
        a if a < 90.0 => (1.0, 1.0),
        a if a < 180.0 => (-1.0, 1.0),
        a if a < 270.0 => (-1.0, -1.0),
        _ => (1.0, -1.0),
    };
    let center_x = (rect[0] + rect[2]) * 0.5;
    let center_y = (rect[1] + rect[3]) * 0.5;
    let point_x = center_x + abs_x_offset * x_sign;
    let point_y = center_y + abs_y_offset * y_sign;
    let offset_x = point_x - center_x;
    let offset_y = point_y - center_y;
    let parameter = angle_from_vector(offset_x * (ry / rx), offset_y);
    let (sin, cos) = sincos_radians(parameter).sin_cos();
    Some(([cos, sin], parameter))
}

/// Expand Model's `[left, top, right, bottom, start, sweep]` tuple into a
/// start point followed by quadratic control/end pairs. `None` means the
/// rectangle is inverted or non-finite and this bounded reconstruction does
/// not claim a result for it.
pub(super) fn expand(input: [f32; 6]) -> Option<Vec<[f32; 2]>> {
    let [left, top, right, bottom, start, sweep] = input;
    if input.iter().any(|value| !value.is_finite()) || left > right || top > bottom {
        return None;
    }
    if start.abs() > MAX_INPUT_ANGLE || sweep.abs() > MAX_INPUT_ANGLE {
        return None;
    }
    let rx = (right - left) * 0.5;
    let ry = (bottom - top) * 0.5;
    if !rx.is_finite() || !ry.is_finite() {
        return None;
    }
    if rx == 0.0 && ry == 0.0 {
        return Some(vec![[left, top]]);
    }

    // Native caps a complete/multi-turn sweep to avoid producing a duplicate
    // closing vertex; this constant is the nearest native f32 to 359.9.
    let mut effective_sweep = sweep;
    let ellipse = rx != ry && rx != 0.0 && ry != 0.0;
    if ellipse && effective_sweep >= 359.9 {
        effective_sweep = f32::from_bits(0x43b3_f333);
    } else if ellipse && effective_sweep <= -359.9 {
        effective_sweep = f32::from_bits(0xc3b3_f333);
    }
    let raw_start = normalize_angle(start)?;
    let raw_end = normalize_angle(raw_start + effective_sweep)?;
    let full_turn = sweep.abs() >= 359.9;
    let (start_vector, end_vector, start_parameter, mut parameter_sweep) =
        if rx == ry || rx == 0.0 || ry == 0.0 {
            let start_vector = unit_vector(raw_start)?;
            let end_vector = if full_turn {
                unit_vector(raw_start + effective_sweep)?
            } else {
                unit_vector(raw_end)?
            };
            (start_vector, end_vector, raw_start, effective_sweep)
        } else {
            let (start_vector, start_parameter) =
                ray_parameter(raw_start, [left, top, right, bottom], rx, ry)?;
            let (end_vector, parameter_sweep) = if full_turn {
                let end_parameter = start_parameter + effective_sweep;
                (unit_vector_unwrapped(end_parameter), effective_sweep)
            } else {
                let (_, end_parameter) =
                    ray_parameter(raw_end, [left, top, right, bottom], rx, ry)?;
                let mut parameter_sweep = end_parameter - start_parameter;
                if effective_sweep >= 0.0 {
                    while parameter_sweep < 0.0 {
                        parameter_sweep += 360.0;
                    }
                } else {
                    while parameter_sweep > 0.0 {
                        parameter_sweep -= 360.0;
                    }
                }
                (
                    unit_vector_unwrapped(start_parameter + parameter_sweep),
                    parameter_sweep,
                )
            };
            (start_vector, end_vector, start_parameter, parameter_sweep)
        };

    // The raw angle parameter is also retained for the Model builder's
    // quadrant-specific collapse and signed-sweep decisions.
    if parameter_sweep == 0.0 && effective_sweep != 0.0 {
        parameter_sweep = effective_sweep;
    }
    super::model_quad_arc::expand(
        [left, top, right, bottom],
        start_vector,
        end_vector,
        start_parameter,
        parameter_sweep,
    )
}

#[cfg(test)]
mod tests {
    use super::expand;
    use serde_json::Value;

    fn bits(value: &Value) -> u32 {
        value.as_u64().expect("float bits") as u32
    }

    fn check(input: &[Value], expected: &[Value], label: &str) {
        let input: [f32; 6] = input
            .iter()
            .map(|value| f32::from_bits(bits(value)))
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        let expected: Vec<u32> = expected.iter().map(bits).collect();
        let actual = expand(input).unwrap_or_else(|| panic!("unsupported input: {label}"));
        assert_eq!(actual.len() * 2, expected.len(), "point count: {label}");
        let actual: Vec<u32> = actual.into_iter().flatten().map(f32::to_bits).collect();
        assert_eq!(actual, expected, "native helper bits: {label}");
    }

    #[test]
    fn direct_model_arc_helpers_match_numeric_vectors() {
        let fixture: Value =
            serde_json::from_str(include_str!("../../../../conformance/shape-paths.json")).unwrap();
        for case in fixture["capture"]["model_arc_helpers"].as_array().unwrap() {
            check(
                case["input_bits"].as_array().unwrap(),
                case["point_bits"].as_array().unwrap(),
                case["name"].as_str().unwrap(),
            );
        }
    }

    #[test]
    fn unsupported_extreme_angles_and_extent_overflow_are_bounded() {
        assert!(expand([0.0, 0.0, 10.0, 10.0, f32::MAX, 90.0]).is_none());
        assert!(expand([0.0, 0.0, 10.0, 10.0, 0.0, f32::MAX]).is_none());
        assert!(expand([-f32::MAX, 0.0, f32::MAX, 10.0, 0.0, 90.0]).is_none());
    }

    #[test]
    fn normalized_model_arc_helper_inputs_match_numeric_vectors() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/shape-normalization.json"
        ))
        .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            for helper in case["model_helpers"].as_array().unwrap() {
                check(
                    helper["input_bits"].as_array().unwrap(),
                    helper["point_bits"].as_array().unwrap(),
                    case["name"].as_str().unwrap(),
                );
            }
        }
    }
}
