use super::tests::{end_alpha, redraw_alpha_candidate};
use super::*;
use crate::ink::brushpen::Cubic;

fn five_point(input: Input<'_>) -> Result<Vec<RtRecord>, Error> {
    if input.points.len() != 5 {
        return Err(Error::PointCount);
    }
    if input.pressure.len() != 5 {
        return Err(Error::PressureCount);
    }
    if input
        .points
        .iter()
        .flatten()
        .chain(input.pressure.iter())
        .chain([&input.pen_size])
        .any(|v| !v.is_finite())
        || input.pressure.iter().any(|v| !(0.0..=1.0).contains(v))
        || input.pen_size <= 0.0
    {
        return Err(Error::InvalidInput);
    }
    if input.pen_size > 2.0 {
        return Err(Error::UnsupportedWidthBranch);
    }
    let p = |i: usize| P {
        x: input.points[i][0],
        y: input.points[i][1],
    };
    if (1..5).any(|i| !(p(i).x - p(0).x).is_finite() || !(p(i).y - p(0).y).is_finite()) {
        return Err(Error::InvalidInput);
    }
    let admitted = |a: P, b: P| (a.x - b.x).abs() >= 5.0 || (a.y - b.y).abs() >= 5.0;
    let mut accepted = Vec::new();
    let mut previous = 0;
    for index in 1..4 {
        if admitted(p(previous), p(index)) {
            accepted.push(index);
            previous = index;
        }
    }
    if accepted.len() == 2 {
        let first = accepted[0];
        let second = accepted[1];
        let distance = p(second).sub(p(first)).len();
        if !distance.is_finite() || distance >= 15.0 {
            return Err(Error::InvalidInput);
        }
        let curve = Cubic::new([p(0), p(first), p(first).mid(p(4)), p(4)]);
        if !curve.length.is_finite() || curve.length <= 0.0 {
            return Err(Error::InvalidInput);
        }
        let (_, _, target_width, repeat_width) = width_state(
            input.points[first],
            input.points[4],
            input.pen_size,
            input.pressure[4],
            input.pen_size,
        );
        let count = repeat_count(curve.length, repeat_width)?;
        let prior = (170 + ((input.pressure[first] * 0.5) * 170.0).trunc() as i32) / 2;
        let alpha = end_alpha(prior, prior, Some(distance), count as u32);
        let mut width = input.pen_size;
        let width_step = (target_width - width) / count as f32;
        let mut distance = 0.0_f32;
        let mut records = Vec::with_capacity(count);
        for index in 0..count {
            let position = curve.at(distance);
            let tangent = curve.normalized_tangent(distance);
            let call = if index == 0 {
                RtCall::Start
            } else if index + 1 == count {
                RtCall::End
            } else {
                RtCall::Point
            };
            let opacity = if call == RtCall::Point {
                (index as f32)
                    .mul_add(alpha.step, alpha.start as f32)
                    .trunc()
                    / 255.0
            } else {
                1.0
            };
            if ![position.x, position.y, tangent.x, tangent.y, width, opacity]
                .iter()
                .all(|v| v.is_finite())
            {
                return Err(Error::InvalidInput);
            }
            records.push(RtRecord {
                call,
                lanes: [
                    position.x,
                    position.y,
                    (width * 0.5).max(1.0),
                    tangent.x,
                    tangent.y,
                    opacity,
                ],
            });
            width += width_step;
            distance += curve.length / count as f32;
        }
        return Ok(records);
    }
    if accepted.len() != 3 {
        return Err(Error::InvalidInput);
    }
    let mid = p(1).mid(p(3));
    let curves = [Quad::new(p(0), p(1), mid), Quad::new(mid, p(3), p(4))];
    if curves
        .iter()
        .any(|q| !q.length.is_finite() || q.length <= 0.0)
    {
        return Err(Error::InvalidInput);
    }
    let distance = p(3).sub(p(2)).len();
    if !distance.is_finite() || distance >= 15.0 {
        return Err(Error::InvalidInput);
    }
    let start_alpha = (170 + ((input.pressure[1] * 0.5) * 170.0).trunc() as i32) / 2;
    let (_, _, redraw_target) = redraw_alpha_candidate(start_alpha, input.pressure[3], distance);
    let (_, _, redraw_width, repeat_width) = width_state(
        input.points[2],
        input.points[3],
        input.pen_size,
        input.pressure[3],
        input.pen_size,
    );
    let counts = [
        repeat_count(curves[0].length, repeat_width)?,
        repeat_count(curves[1].length, redraw_width)?,
    ];
    let end = end_alpha(
        redraw_target,
        redraw_target,
        Some(distance),
        counts[1] as u32,
    );
    let (_, _, end_width, _) = width_state(
        input.points[3],
        input.points[4],
        input.pen_size,
        input.pressure[4],
        redraw_width,
    );
    let total = counts[0]
        .checked_mul(2)
        .and_then(|n| n.checked_add(counts[1]))
        .filter(|&n| n <= 3 * MAX_RECORDS)
        .ok_or(Error::RepeatCount)?;
    let mut records = Vec::with_capacity(total);
    for batch in 0..3 {
        let segment = usize::from(batch == 1);
        let count = counts[segment];
        let q = &curves[segment];
        let initial_width = if segment == 0 {
            input.pen_size
        } else {
            redraw_width
        };
        let target_width = if segment == 0 {
            redraw_width
        } else {
            end_width
        };
        let width_step = (target_width - initial_width) / count as f32;
        let alpha_step = if segment == 0 {
            (redraw_target - start_alpha) as f32 / count as f32
        } else {
            end.step
        };
        let alpha_start = if segment == 0 { start_alpha } else { end.start };
        let mut width = initial_width;
        let mut distance = 0.0_f32;
        for index in 0..count {
            let position = q.at(distance);
            let tangent = q.normalized_tangent(distance);
            let call = if batch == 0 && index == 0 {
                RtCall::Start
            } else if batch == 1 && index + 1 == count {
                RtCall::End
            } else {
                RtCall::Point
            };
            let alpha = if call == RtCall::Point {
                ((index + usize::from(segment == 0)) as f32)
                    .mul_add(alpha_step, alpha_start as f32)
                    .trunc()
                    / 255.0
            } else {
                1.0
            };
            if ![position.x, position.y, tangent.x, tangent.y, width, alpha]
                .iter()
                .all(|v| v.is_finite())
            {
                return Err(Error::InvalidInput);
            }
            records.push(RtRecord {
                call,
                lanes: [
                    position.x,
                    position.y,
                    (width * 0.5).max(1.0),
                    tangent.x,
                    tangent.y,
                    alpha,
                ],
            });
            width += width_step;
            distance += q.length / count as f32;
        }
    }
    Ok(records)
}

#[test]
fn five_point_matches_complete_native_streams() {
    let extended: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/montblanc-extended-native-streams.json"
    ))
    .unwrap();
    let midpoint: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/montblanc-midpoint-native.json"
    ))
    .unwrap();
    let shapes: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/montblanc-five-point-shapes-native.json"
    ))
    .unwrap();
    let admission: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/montblanc-five-point-admission-trace.json"
    ))
    .unwrap();
    let admission_cases = admission["cases"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(name, case)| (name.as_str(), &case["input"], &case["result"]["stream"]));
    for (name, input, stream) in [
        (
            "uneven",
            &shapes["cases"]["uneven"]["input"],
            &shapes["cases"]["uneven"]["result"]["stream"],
        ),
        (
            "bent",
            &shapes["cases"]["bent"]["input"],
            &shapes["cases"]["bent"]["result"]["stream"],
        ),
        (
            "reverse",
            &shapes["cases"]["reverse"]["input"],
            &shapes["cases"]["reverse"]["result"]["stream"],
        ),
        (
            "five_points",
            &extended["cases"]["five_points"]["input"],
            &extended["cases"]["five_points"]["result"]["stream"],
        ),
        (
            "midpoint",
            &midpoint["input"],
            &midpoint["stream"]["stream"],
        ),
    ]
    .into_iter()
    .chain(admission_cases)
    {
        let number = |x: &serde_json::Value| x.as_f64().unwrap() as f32;
        let points: Vec<[f32; 2]> = input["points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| [number(&p[0]), number(&p[1])])
            .collect();
        let pressure: Vec<f32> = input["pressure"]
            .as_array()
            .unwrap()
            .iter()
            .map(number)
            .collect();
        let result = five_point(Input {
            points: &points,
            pressure: &pressure,
            pen_size: number(&input["pen_size"]),
        });
        let actual = match result {
            Ok(records) => records,
            Err(Error::InvalidInput)
                if {
                    let mut last = points[0];
                    let mut accepted = 0;
                    for point in &points[1..4] {
                        if (point[0] - last[0]).abs() >= 5.0 || (point[1] - last[1]).abs() >= 5.0 {
                            accepted += 1;
                            last = *point;
                        }
                    }
                    accepted < 2
                } =>
            {
                continue;
            }
            Err(error) => panic!("{name}: {error}"),
        };
        let expected = stream.as_array().unwrap();
        assert_eq!(actual.len(), expected.len(), "{name}");
        for (i, (a, e)) in actual.iter().zip(expected).enumerate() {
            let symbol = e[0].as_str().unwrap();
            let call = if symbol.contains("AddStartPoint") {
                RtCall::Start
            } else if symbol.contains("AddEndPoint") {
                RtCall::End
            } else if symbol.contains("::AddPoint(") {
                RtCall::Point
            } else {
                panic!("unexpected native call {symbol}");
            };
            assert_eq!(a.call, call, "{name} {i}");
            let words = e[1].as_array().unwrap();
            assert_eq!(words.len(), 6, "{name} {i}");
            for (lane, word) in words.iter().enumerate() {
                let s = word.as_str().unwrap();
                let bits = u32::from_le_bytes(std::array::from_fn(|j| {
                    u8::from_str_radix(&s[j * 2..j * 2 + 2], 16).unwrap()
                }));
                assert_eq!(a.lanes[lane].to_bits(), bits, "{name} {i} lane {lane}");
            }
        }
    }
}

#[test]
fn five_point_pressure_sweep_matches_native_stream_digests() {
    let receipt: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/montblanc-five-point-pressure-sweep.json"
    ))
    .unwrap();
    for (name, case) in receipt["cases"].as_object().unwrap() {
        let input = &case["input"];
        let number = |x: &serde_json::Value| x.as_f64().unwrap() as f32;
        let points: Vec<[f32; 2]> = input["points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| [number(&p[0]), number(&p[1])])
            .collect();
        let pressure: Vec<f32> = input["pressure"]
            .as_array()
            .unwrap()
            .iter()
            .map(number)
            .collect();
        let actual = five_point(Input {
            points: &points,
            pressure: &pressure,
            pen_size: number(&input["pen_size"]),
        })
        .unwrap();
        let stream: Vec<_> = actual.iter().map(|r| {
            let method = match r.call { RtCall::Start => "AddStartPoint", RtCall::Point => "AddPoint", RtCall::End => "AddEndPoint" };
            let symbol = format!("SPen::MontblancFountainPenStrokeDrawableRTV1::{method}(SPen::Vector3<float>, SPen::Vector2<float>, float)");
            let lanes: Vec<_> = r.lanes.iter().map(|x| x.to_le_bytes().iter().map(|b| format!("{b:02x}")).collect::<String>()).collect();
            serde_json::json!([symbol, lanes])
        }).collect();
        assert_eq!(
            actual.len(),
            case["count"].as_u64().unwrap() as usize,
            "{name}"
        );
        let bytes = serde_json::to_vec(&stream).unwrap();
        assert_eq!(
            format!("{:x}", Sha256::digest(bytes)),
            case["stream_sha256"].as_str().unwrap(),
            "{name}"
        );
    }
}

#[test]
fn rejected_admission_and_nonfinite_intermediates_are_guarded() {
    let pressure = [0.3, 0.6, 1.0, 0.7, 0.5];
    for points in [
        [
            [0.0, 0.0],
            [4.0, 4.0],
            [-4.0, -4.0],
            [10.0, 0.0],
            [20.0, 6.0],
        ],
        [
            [f32::MAX, 0.0],
            [-f32::MAX, 0.0],
            [0.0, 0.0],
            [10.0, 0.0],
            [20.0, 0.0],
        ],
    ] {
        assert!(
            five_point(Input {
                points: &points,
                pressure: &pressure,
                pen_size: 2.0
            })
            .is_err()
        );
    }
}
