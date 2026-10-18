use super::NativePathCommand;

const DEG_TO_RAD: f32 = f32::from_bits(0x3c8e_fa35);
const VECTOR_EPSILON: f32 = f32::from_bits(0x3980_0000);
const ARC_TAN_PI_OVER_8: f32 = f32::from_bits(0x3ed4_13cd);
const ARC_MID: f32 = f32::from_bits(0x3f35_04f7);
const NEAR_FULL_LOW: f32 = 359.0;
const NEAR_FULL_HIGH: f32 = 360.0;
const NEAR_FULL_STEP: f32 = 1.0 / 512.0;

const UNIT_ARC: [[f32; 2]; 17] = [
    [1.0, 0.0],
    [1.0, ARC_TAN_PI_OVER_8],
    [ARC_MID, ARC_MID],
    [ARC_TAN_PI_OVER_8, 1.0],
    [0.0, 1.0],
    [-ARC_TAN_PI_OVER_8, 1.0],
    [-ARC_MID, ARC_MID],
    [-1.0, ARC_TAN_PI_OVER_8],
    [-1.0, 0.0],
    [-1.0, -ARC_TAN_PI_OVER_8],
    [-ARC_MID, -ARC_MID],
    [-ARC_TAN_PI_OVER_8, -1.0],
    [0.0, -1.0],
    [ARC_TAN_PI_OVER_8, -1.0],
    [ARC_MID, -ARC_MID],
    [1.0, -ARC_TAN_PI_OVER_8],
    [1.0, 0.0],
];

/// Expand a direct Drawing Arc record into its native Move/Quadratic sequence.
/// Ordinary saved shapes first pass through the distinct Model normalizer;
/// this helper does not implement that upstream route.
///
/// The six stored f64s are narrowed to f32 before the native operations. This
/// follows Drawing's saved segment representation and Skia's quadratic arc
/// builder. Angles use the current platform's `f32::sin_cos`; Samsung's Android
/// `sincosf` may produce different last bits.
pub(crate) fn expand(values: [f64; 6]) -> Option<Vec<NativePathCommand>> {
    let [left, top, right, bottom, start, sweep] = values.map(|value| value as f32);
    if ![left, top, right, bottom, start, sweep]
        .into_iter()
        .all(f32::is_finite)
    {
        return None;
    }
    if right < left || bottom < top {
        return Some(Vec::new());
    }

    let rx = (right - left) * 0.5;
    let ry = (bottom - top) * 0.5;
    let cx = (right + left) * 0.5;
    let cy = (bottom + top) * 0.5;
    if ![rx, ry, cx, cy].into_iter().all(f32::is_finite) {
        return None;
    }
    if (sweep == 0.0 && (start == 0.0 || start == 360.0)) || (rx == 0.0 && ry == 0.0) {
        return Some(vec![NativePathCommand::Move([
            f64::from(right),
            f64::from(cy),
        ])]);
    }

    let start_radians = start * DEG_TO_RAD;
    let end_degrees = start + sweep;
    let mut end_radians = end_degrees * DEG_TO_RAD;
    let (start_sin, start_cos) = start_radians.sin_cos();
    let start_vector = [snap(start_cos), snap(start_sin)];
    let (end_sin, end_cos) = end_radians.sin_cos();
    let mut end_vector = [snap(end_cos), snap(end_sin)];

    if start_vector == end_vector && sweep.abs() > NEAR_FULL_LOW && sweep.abs() < NEAR_FULL_HIGH {
        let step = if sweep.is_sign_negative() {
            -NEAR_FULL_STEP
        } else {
            NEAR_FULL_STEP
        };
        for _ in 0..4096 {
            end_radians -= step;
            let (sin, cos) = end_radians.sin_cos();
            end_vector = [snap(cos), snap(sin)];
            if start_vector != end_vector {
                break;
            }
        }
        if start_vector == end_vector {
            return None;
        }
    }

    let direction_ccw = sweep <= 0.0;
    let unit_points = build_quad_arc(start_vector, end_vector, direction_ccw)?;
    let mut commands = Vec::with_capacity(unit_points.len().saturating_sub(1));
    let start_point = map_point(unit_points[0], start_vector, direction_ccw, rx, ry, cx, cy);
    commands.push(NativePathCommand::Move(start_point.map(f64::from)));
    for points in unit_points[1..].chunks_exact(2) {
        let control = map_point(points[0], start_vector, direction_ccw, rx, ry, cx, cy);
        let endpoint = map_point(points[1], start_vector, direction_ccw, rx, ry, cx, cy);
        commands.push(NativePathCommand::Quadratic([
            control.map(f64::from),
            endpoint.map(f64::from),
        ]));
    }
    if commands.iter().all(command_finite) {
        Some(commands)
    } else {
        None
    }
}

fn command_finite(command: &NativePathCommand) -> bool {
    match command {
        NativePathCommand::Move(point) | NativePathCommand::Line(point) => {
            point.iter().all(|value| (*value as f32).is_finite())
        }
        NativePathCommand::Quadratic(points) => points
            .iter()
            .flatten()
            .all(|value| (*value as f32).is_finite()),
        _ => false,
    }
}

fn snap(value: f32) -> f32 {
    if value.abs() <= VECTOR_EPSILON {
        0.0
    } else {
        value
    }
}

fn build_quad_arc(start: [f32; 2], stop: [f32; 2], ccw: bool) -> Option<Vec<[f32; 2]>> {
    // Match native FP fused multiply-add ordering for its dot/cross tests.
    let x = start[0].mul_add(stop[0], start[1] * stop[1]);
    let mut y = start[0].mul_add(stop[1], -(start[1] * stop[0]));
    let abs_x = x.abs();
    let abs_y = y.abs();
    if abs_y <= VECTOR_EPSILON && x > 0.0 && ((y >= 0.0 && !ccw) || (y <= 0.0 && ccw)) {
        return Some(vec![[1.0, 0.0]]);
    }
    if ccw {
        y = -y;
    }

    let octant = if y == 0.0 {
        4
    } else if x == 0.0 {
        if y > 0.0 { 2 } else { 6 }
    } else {
        let mut octant = 0;
        if y < 0.0 {
            octant += 4;
        }
        let same_sign = (x < 0.0) == (y < 0.0);
        if !same_sign {
            octant += 2;
        }
        if (abs_x < abs_y) == same_sign {
            octant += 1;
        }
        octant
    };

    let whole = octant * 2;
    let mut points = UNIT_ARC[..=whole].to_vec();
    let curve_start = UNIT_ARC[whole];
    let curve_control = UNIT_ARC[whole + 1];
    let curve_end = UNIT_ARC[whole + 2];
    if let Some(control) = truncate_last_curve(curve_start, curve_control, curve_end, [x, y]) {
        points.push(control);
        points.push([x, y]);
    }
    Some(points)
}

fn truncate_last_curve(
    start: [f32; 2],
    control: [f32; 2],
    end: [f32; 2],
    target: [f32; 2],
) -> Option<[f32; 2]> {
    let axis = if target[0].abs() < target[1].abs() {
        0
    } else {
        1
    };
    let a = start[axis];
    let b = control[axis];
    let c = end[axis];
    let target_axis = target[axis];
    let qa = b.mul_add(-2.0, a) + c;
    let qb = 2.0 * (b - a);
    let qc = a - target_axis;
    let mut roots = [0.0; 2];
    let count = unit_quad_roots(qa, qb, qc, &mut roots);
    let t = if count == 1 { roots[0] } else { 0.0 };
    if t > 0.0 {
        return Some([
            (control[0] - start[0]).mul_add(t, start[0]),
            (control[1] - start[1]).mul_add(t, start[1]),
        ]);
    }
    if (start[axis] < end[axis] && target_axis > control[axis])
        || (start[axis] > end[axis] && target_axis < control[axis])
    {
        return Some(control);
    }
    None
}

fn unit_quad_roots(a: f32, b: f32, c: f32, roots: &mut [f32; 2]) -> usize {
    fn valid_divide(numerator: f32, denominator: f32) -> Option<f32> {
        let (numerator, denominator) = if numerator < 0.0 {
            (-numerator, -denominator)
        } else {
            (numerator, denominator)
        };
        if denominator == 0.0 || numerator == 0.0 || numerator >= denominator {
            return None;
        }
        let ratio = numerator / denominator;
        (ratio.is_finite() && ratio > 0.0 && ratio < 1.0).then_some(ratio)
    }
    if a == 0.0 {
        if let Some(root) = valid_divide(-c, b) {
            roots[0] = root;
            return 1;
        }
        return 0;
    }
    // Native Skia rounds (-4 * A) before multiplying by C, then evaluates
    // B*B + product with a fused multiply-add (Skia ARM64 0x1b9c6c).
    let rounded_product = (-4.0 * a) * c;
    let mut discriminant = b.mul_add(b, rounded_product);
    if discriminant < 0.0 || discriminant.is_nan() {
        return 0;
    }
    discriminant = discriminant.sqrt();
    let q = if b < 0.0 {
        -((b - discriminant) / 2.0)
    } else {
        -((b + discriminant) / 2.0)
    };
    let mut count = 0;
    if let Some(root) = valid_divide(q, a) {
        roots[count] = root;
        count += 1;
    }
    if let Some(root) = valid_divide(c, q) {
        roots[count] = root;
        count += 1;
    }
    if count == 2 {
        if roots[0] > roots[1] {
            roots.swap(0, 1);
        } else if roots[0] == roots[1] {
            count -= 1;
        }
    }
    count
}

fn map_point(
    point: [f32; 2],
    start: [f32; 2],
    ccw: bool,
    rx: f32,
    ry: f32,
    cx: f32,
    cy: f32,
) -> [f32; 2] {
    let y = if ccw { -point[1] } else { point[1] };
    let x = start[0].mul_add(point[0], -(start[1] * y));
    let y = start[1].mul_add(point[0], start[0] * y);
    [x.mul_add(rx, cx), y.mul_add(ry, cy)]
}

#[cfg(test)]
mod tests {
    use super::expand;
    use crate::shape::NativePathCommand;
    use serde_json::Value;

    fn native_commands(commands: &[Value]) -> Vec<(u8, Vec<u32>)> {
        commands
            .iter()
            .filter_map(|command| {
                let verb = command["verb"].as_u64()? as u8;
                if verb == 6 {
                    return None;
                }
                let mut bits = command["point_bits"]
                    .as_array()?
                    .iter()
                    .map(|v| v.as_u64().unwrap() as u32)
                    .collect::<Vec<_>>();
                if verb == 2 {
                    bits.drain(..2);
                }
                Some((verb, bits))
            })
            .collect()
    }

    fn our_commands(commands: Vec<NativePathCommand>) -> Vec<(u8, Vec<u32>)> {
        commands
            .into_iter()
            .map(|command| match command {
                NativePathCommand::Move([x, y]) => {
                    (0, vec![(x as f32).to_bits(), (y as f32).to_bits()])
                }
                NativePathCommand::Quadratic([[cx, cy], [x, y]]) => (
                    2,
                    vec![
                        (cx as f32).to_bits(),
                        (cy as f32).to_bits(),
                        (x as f32).to_bits(),
                        (y as f32).to_bits(),
                    ],
                ),
                other => panic!("unexpected Drawing Arc expansion: {other:?}"),
            })
            .collect()
    }

    #[test]
    fn broad_native_drawing_arc_vectors_match_native_segmentation_within_two_ppm() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/shape-drawing-arcs.json"
        ))
        .unwrap();
        let mut compared = 0;
        let mut max_absolute_error = 0.0f32;
        let mut max_absolute_normal_error = 0.0f32;
        let mut max_relative_error = 0.0f32;
        for case in fixture["cases"].as_array().unwrap() {
            let seeded = case["seeded"] == true;
            let input: [f64; 6] = case["input_f64_bits"]
                .as_array()
                .unwrap()
                .iter()
                .map(|bits| f64::from_bits(bits.as_u64().unwrap()))
                .collect::<Vec<_>>()
                .try_into()
                .unwrap();
            let expected = native_commands(case["commands"].as_array().unwrap());
            if case["name"].as_str().unwrap().starts_with("inverted-") {
                assert!(
                    expand(input).is_some_and(|commands| commands.is_empty()),
                    "inverted input did not produce a no-op: {}",
                    case["name"]
                );
                assert!(
                    expected.is_empty(),
                    "native inverted case unexpectedly emitted commands"
                );
                continue;
            }
            let actual = expand(input).expect("finite ordered native Arc should expand");
            let mut expected = expected;
            if seeded {
                assert_eq!(expected.len() >= 3, true);
                expected.drain(..2);
            }
            let actual = our_commands(actual);
            assert_eq!(
                actual.len(),
                expected.len(),
                "native command count: {}",
                case["name"]
            );
            for (actual, expected) in actual.iter().zip(&expected) {
                assert_eq!(actual.0, expected.0, "native verb: {}", case["name"]);
                assert_eq!(actual.1.len(), expected.1.len());
                for (actual, expected) in actual.1.iter().zip(&expected.1) {
                    let actual = f32::from_bits(*actual);
                    let expected = f32::from_bits(*expected);
                    max_absolute_error = max_absolute_error.max((actual - expected).abs());
                    if expected.abs() < 1.0e6 {
                        max_absolute_normal_error =
                            max_absolute_normal_error.max((actual - expected).abs());
                    }
                    max_relative_error =
                        max_relative_error.max((actual - expected).abs() / expected.abs().max(1.0));
                    assert!(
                        (actual - expected).abs() <= 2.0e-6 * expected.abs().max(1.0),
                        "{} actual={actual:?} {:08x} native={expected:?} {:08x}",
                        case["name"],
                        actual.to_bits(),
                        expected.to_bits()
                    );
                }
            }
            compared += 1;
        }
        assert_eq!(
            compared, 31,
            "all finite ordered native arc cases must be compared"
        );
        eprintln!(
            "compared {compared} Drawing Arc cases; max abs error {max_absolute_error}, max normal-coordinate abs error {max_absolute_normal_error}, max relative error {max_relative_error}"
        );
    }

    #[test]
    fn unit_quad_roots_match_native_skia_helper_bits() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/shape-drawing-arcs.json"
        ))
        .unwrap();
        let probes = fixture["cases"][0]["native_quad_root_probes"]
            .as_array()
            .expect("fixture contains direct SkFindUnitQuadRoots probes");
        assert_eq!(probes.len(), 6);
        for probe in probes {
            let input = probe["input_bits"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| f32::from_bits(value.as_u64().unwrap() as u32))
                .collect::<Vec<_>>();
            let mut roots = [0.0; 2];
            let count = super::unit_quad_roots(input[0], input[1], input[2], &mut roots);
            let expected = probe["root_bits"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_u64().unwrap() as u32)
                .collect::<Vec<_>>();
            let actual = roots[..count]
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>();
            assert_eq!(
                count,
                probe["count"].as_u64().unwrap() as usize,
                "{}",
                probe["name"]
            );
            assert_eq!(actual, expected, "{}", probe["name"]);
        }
    }

    #[test]
    fn rejects_unrepresentable_values_and_preserves_inverted_rectangle_noop() {
        assert!(expand([0., 0., 1., 1., f64::MAX, 30.]).is_none());
        assert!(expand([0., 0., 1., 1., 0., f64::MAX]).is_none());
        assert!(expand([2., 0., 1., 1., 0., 90.]).unwrap().is_empty());
        let large = f64::from(f32::MAX);
        assert!(expand([large, large, large, large, 0., 0.]).is_none());
    }
}
