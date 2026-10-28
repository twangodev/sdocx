use super::tests::end_alpha;
use super::*;
use crate::ink::brushpen::Cubic;

fn four_point(input: Input<'_>) -> Result<Vec<RtRecord>, Error> {
    if input.points.len() != 4 {
        return Err(Error::PointCount);
    }
    if input.pressure.len() != 4 {
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
    let last_move_distance = p(2).sub(p(1)).len();
    if last_move_distance >= 15.0 {
        return Err(Error::InvalidInput);
    }
    let curve = Cubic::new([p(0), p(1), p(1).mid(p(3)), p(3)]);
    if !curve.length.is_finite() || curve.length <= 0.0 {
        return Err(Error::InvalidInput);
    }
    let (_, _, target_width, repeat_width) = width_state(
        input.points[1],
        input.points[3],
        input.pen_size,
        input.pressure[3],
        input.pen_size,
    );
    let count = repeat_count(curve.length, repeat_width)?;
    let prior = (170 + ((input.pressure[1] * 0.5) * 170.0).trunc() as i32) / 2;
    let alpha = end_alpha(prior, prior, Some(last_move_distance), count as u32);
    let width_step = (target_width - input.pen_size) / count as f32;
    let mut width = input.pen_size;
    let mut distance = 0.0_f32;
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let position = curve.at(distance);
        let tangent = curve.normalized_tangent(distance);
        let call = if i == 0 {
            RtCall::Start
        } else if i + 1 == count {
            RtCall::End
        } else {
            RtCall::Point
        };
        let opacity = if call == RtCall::Point {
            (i as f32).mul_add(alpha.step, alpha.start as f32).trunc() / 255.0
        } else {
            1.0
        };
        out.push(RtRecord {
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
    Ok(out)
}

#[test]
fn four_point_matches_complete_native_streams() {
    let extended: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/montblanc-extended-native-streams.json"
    ))
    .unwrap();
    let curved: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/montblanc-curved-four-native.json"
    ))
    .unwrap();
    for (name, input, stream) in [
        (
            "four_points",
            &extended["cases"]["four_points"]["input"],
            &extended["cases"]["four_points"]["result"]["stream"],
        ),
        (
            "four_second_y_10",
            &extended["cases"]["four_second_y_10"]["input"],
            &extended["cases"]["four_second_y_10"]["result"]["stream"],
        ),
        ("curved", &curved["input"], &curved["stream"]["stream"]),
    ] {
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
        let actual = four_point(Input {
            points: &points,
            pressure: &pressure,
            pen_size: number(&input["pen_size"]),
        })
        .unwrap();
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
fn unsupported_four_point_state_is_rejected() {
    let points = [[0.0, 0.0], [10.0, 0.0], [25.0, 0.0], [30.0, 0.0]];
    let pressure = [0.3, 0.6, 1.0, 0.7];
    assert!(matches!(
        four_point(Input {
            points: &points,
            pressure: &pressure,
            pen_size: 2.0
        }),
        Err(Error::InvalidInput)
    ));
    assert!(matches!(
        four_point(Input {
            points: &points,
            pressure: &pressure,
            pen_size: 3.0
        }),
        Err(Error::UnsupportedWidthBranch)
    ));
}
