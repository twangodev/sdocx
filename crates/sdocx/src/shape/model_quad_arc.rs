//! Model's native unit-arc builder and distinct rectangle mapping (0x211418).
//! Kept test-only until the outer Model arc and saved-load routes are verified.

const PI: f64 = f64::from_bits(0x4009_21fb_5444_2d18);

/// Match Model's scalar ABI, including its two stack-passed angle arguments.
pub(super) fn expand(
    rect: [f32; 4],
    start: [f32; 2],
    stop: [f32; 2],
    angle: f32,
    sweep: f32,
) -> Option<Vec<[f32; 2]>> {
    if rect
        .into_iter()
        .chain(start)
        .chain(stop)
        .chain([angle, sweep])
        .any(|v| !v.is_finite())
    {
        return None;
    }
    let x = start[0].mul_add(stop[0], start[1] * stop[1]);
    let cross = start[0].mul_add(stop[1], -(start[1] * stop[0]));
    let negative = sweep < 0.;
    // Model's small-cross collapse predicate differs from Drawing/Skia's.
    // Only the subsequent table/octant/root arithmetic is shared.
    let collapsed = x > 0.
        && cross.abs() <= f32::from_bits(0x3980_0000)
        && ((negative && cross >= 0.) || (!negative && cross <= 0.));
    let mut points = if collapsed {
        vec![[1., 0.]]
    } else {
        super::native_arc::unit_quad_points(x, if negative { -cross } else { cross })?
    };
    let rotation = angle + if sweep < 0. { sweep } else { -0. };
    let [left, top, right, bottom] = rect;
    let rx = (right - left) * 0.5;
    let ry = (bottom - top) * 0.5;
    let radians = (f64::from(rotation) / 180.) * PI;
    let (sin, cos) = radians.sin_cos();
    for point in &mut points {
        // Base::PointF::GetRotatedPoint first rotates in f64 and narrows,
        // then Model maps (rotated + 1) with separate f32 add/mul/add.
        let rotated = if rotation == 0. {
            *point
        } else {
            let x = f64::from(point[0]);
            let y = f64::from(point[1]);
            [
                0. + x.mul_add(cos, -(sin * y)) as f32,
                0. + x.mul_add(sin, cos * y) as f32,
            ]
        };
        *point = [left + rx * (rotated[0] + 1.), top + ry * (rotated[1] + 1.)];
        if !point.iter().all(|v| v.is_finite()) {
            return None;
        }
    }
    if sweep < 0. {
        points.reverse();
    }
    Some(points)
}

#[cfg(test)]
mod tests {
    use super::expand;

    #[test]
    fn direct_model_unit_arc_matches_native_point_bits() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../../conformance/model-quad-arcs.json"))
                .unwrap();
        let cases = fixture["cases"].as_array().unwrap();
        for case in cases {
            let input: Vec<f32> = case["input_bits"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| f32::from_bits(value.as_u64().unwrap() as u32))
                .collect();
            assert_eq!(input.len(), 10);
            let actual = expand(
                input[..4].try_into().unwrap(),
                input[4..6].try_into().unwrap(),
                input[6..8].try_into().unwrap(),
                input[8],
                input[9],
            )
            .unwrap();
            assert_eq!(
                actual.len() as u64,
                case["point_count"].as_u64().unwrap(),
                "{}",
                case["name"]
            );
            let actual: Vec<u32> = actual.into_iter().flatten().map(f32::to_bits).collect();
            let expected: Vec<u32> = case["point_bits"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_u64().unwrap() as u32)
                .collect();
            assert_eq!(actual, expected, "{}", case["name"]);
        }
        assert_eq!(cases.len(), 16);
    }
}
