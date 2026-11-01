use super::cubic::Cubic;
use super::*;
enum Curve {
    Q(Quad),
    C(Cubic),
}
impl Curve {
    fn length(&self) -> f32 {
        match self {
            Self::Q(q) => q.length,
            Self::C(c) => c.length,
        }
    }
    fn at(&self, d: f32) -> P {
        match self {
            Self::Q(q) => q.at(d),
            Self::C(c) => c.at(d),
        }
    }
    fn normalized_tangent(&self, d: f32) -> P {
        match self {
            Self::Q(q) => q.normalized_tangent(d),
            Self::C(c) => c.normalized_tangent(d),
        }
    }
}

pub(super) fn many_point(input: Input<'_>) -> Result<Vec<RtRecord>, Error> {
    let n = input.points.len();
    if !(6..=24).contains(&n) {
        return Err(Error::PointCount);
    }
    if input.pressure.len() != n {
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
    if input.pen_size > 2.5 {
        return Err(Error::UnsupportedWidthBranch);
    }
    // Bound intermediate coordinate arithmetic before constructing adaptive curves.
    if input.points.iter().flatten().any(|v| v.abs() > 1_000_000.0) {
        return Err(Error::InvalidInput);
    }
    let p = |i: usize| P {
        x: input.points[i][0],
        y: input.points[i][1],
    };
    let mut accepted = vec![0];
    for i in 1..n - 1 {
        let prev = p(*accepted.last().unwrap());
        let next = p(i);
        let dx = (next.x - prev.x).abs();
        let dy = (next.y - prev.y).abs();
        if !dx.is_finite() || !dy.is_finite() {
            return Err(Error::InvalidInput);
        }
        if dx >= 5.0 || dy >= 5.0 {
            accepted.push(i);
        }
    }
    if accepted.len() < 4 {
        return Err(Error::InvalidInput);
    }
    let mut alpha = (170 + ((input.pressure[accepted[1]] * 0.5) * 170.0).trunc() as i32) / 2;
    let selected_size = input.pen_size.max(0.4);
    let mut width = selected_size;
    let mut segments = Vec::new();
    let mut begin = p(0);
    let mut control = p(accepted[1]);
    for pair in (3..accepted.len()).step_by(2) {
        let prior = accepted[pair - 1];
        let next = accepted[pair];
        let distance = p(next).sub(p(prior)).len();
        if !distance.is_finite() || distance < 4.2 {
            return Err(Error::InvalidInput);
        }
        let finish = control.mid(p(next));
        let curve = Quad::new(begin, control, finish);
        let (_, _, target_width, repeat_width) = width_state(
            input.points[prior],
            input.points[next],
            selected_size,
            input.pressure[next],
            width,
        );
        if !curve.length.is_finite()
            || curve.length <= 0.0
            || !target_width.is_finite()
            || !repeat_width.is_finite()
        {
            return Err(Error::InvalidInput);
        }
        let count = repeat_count(curve.length, repeat_width)?;
        let (_, _, target_alpha) = redraw_alpha_candidate(alpha, input.pressure[next], distance);
        segments.push((
            Curve::Q(curve),
            count,
            width,
            target_width,
            alpha,
            (target_alpha - alpha) as f32 / count as f32,
            true,
        ));
        begin = finish;
        control = p(next);
        width = target_width;
        alpha = target_alpha;
    }
    let last = *accepted.last().unwrap();
    let distance = p(last).sub(p(accepted[accepted.len() - 2])).len();
    if !distance.is_finite() {
        return Err(Error::InvalidInput);
    }
    let curve = if accepted.len() % 2 == 1 {
        Curve::C(Cubic::new([
            begin,
            control,
            control.mid(p(n - 1)),
            p(n - 1),
        ]))
    } else {
        Curve::Q(Quad::new(begin, control, p(n - 1)))
    };
    let (_, _, target_width, repeat_width) = width_state(
        input.points[last],
        input.points[n - 1],
        selected_size,
        input.pressure[n - 1],
        width,
    );
    if !curve.length().is_finite()
        || curve.length() <= 0.0
        || !target_width.is_finite()
        || !repeat_width.is_finite()
    {
        return Err(Error::InvalidInput);
    }
    let count = repeat_count(curve.length(), repeat_width)?;
    // endPen compares the two latest accepted move positions against 15.
    // At or above that boundary it interpolates alpha and skips replay.
    let use_end_fade = distance >= 15.0;
    let end_distance = p(n - 1).sub(control).len();
    let (factor, _, _) = redraw_alpha_candidate(alpha, input.pressure[n - 1], end_distance);
    let seed = (input.pressure[n - 1] * 0.5).mul_add(170.0, 170.0).trunc();
    let candidate = (-(1.0 - factor)).mul_add(0.5, 1.0) * seed;
    let candidate = candidate.trunc() as i32;
    let end_target = if (candidate - alpha).abs() >= 11 {
        alpha + if candidate > alpha { 10 } else { -10 }
    } else {
        candidate
    }
    .clamp(60, 255);
    let end = end_alpha(alpha, end_target, Some(distance), count as u32);
    segments.push((
        curve,
        count,
        width,
        target_width,
        end.start,
        end.step,
        false,
    ));
    let total = record_budget(
        segments
            .iter()
            .map(|(_, count, _, _, _, _, redraw)| (*count, *redraw)),
    )?;
    let mut records = Vec::with_capacity(total);
    // This seven-point history window matches the captured redraw controls.
    for replay in [false, true] {
        for (seg_index, (q, count, initial_width, target_width, alpha_start, alpha_step, redraw)) in
            segments.iter().enumerate()
        {
            if replay && use_end_fade {
                break;
            }
            if replay && seg_index < accepted.len().saturating_sub(7) / 2 {
                continue;
            }
            if replay && !redraw {
                break;
            }
            if !q.length().is_finite() || q.length() <= 0.0 {
                return Err(Error::InvalidInput);
            }
            let width_step = (target_width - initial_width) / *count as f32;
            let mut width = *initial_width;
            let mut distance = 0.0_f32;
            for index in 0..*count {
                let position = q.at(distance);
                let mut tangent = q.normalized_tangent(distance);
                let call = if !replay && seg_index == 0 && index == 0 {
                    RtCall::Start
                } else if !replay && !redraw && index + 1 == *count {
                    if use_end_fade
                        && ((*count as f32)
                            .mul_add(*alpha_step, *alpha_start as f32)
                            .trunc() as i32
                            != alpha)
                    {
                        tangent.x = -tangent.x;
                        tangent.y = -tangent.y;
                        RtCall::Start
                    } else {
                        RtCall::End
                    }
                } else {
                    RtCall::Point
                };
                let opacity = if call == RtCall::Point {
                    ((index + usize::from(*redraw || use_end_fade)) as f32)
                        .mul_add(*alpha_step, *alpha_start as f32)
                        .trunc()
                        / 255.0
                } else {
                    1.0
                };
                let lanes = [
                    position.x,
                    position.y,
                    (width * 0.5).max(1.0),
                    tangent.x,
                    tangent.y,
                    opacity,
                ];
                if lanes.iter().any(|x| !x.is_finite()) {
                    return Err(Error::InvalidInput);
                }
                records.push(RtRecord { call, lanes });
                width += width_step;
                distance += q.length() / *count as f32;
            }
        }
    }
    Ok(records)
}

fn record_budget(mut counts: impl Iterator<Item = (usize, bool)>) -> Result<usize, Error> {
    counts
        .try_fold(0_usize, |total, (count, redraw)| {
            count
                .checked_mul(if redraw { 2 } else { 1 })
                .and_then(|n| total.checked_add(n))
        })
        .filter(|&total| total <= MAX_RECORDS)
        .ok_or(Error::RepeatCount)
}

#[test]
fn record_budget_rejects_aggregate_overflow() {
    assert!(record_budget([(120, false), (120, false)].into_iter()).is_ok());
    assert!(record_budget([(200, false), (60, false)].into_iter()).is_err());
    assert!(record_budget([(130, true)].into_iter()).is_err());
}

#[test]
fn many_point_matches_native_streams() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/montblanc-many-point-full-streams.json"
    ))
    .unwrap();
    let independent: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/montblanc-many-point-independent-streams.json"
    ))
    .unwrap();
    let eight: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/montblanc-eight-probe.json"
    ))
    .unwrap();
    let narrow: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/montblanc-narrow-probe.json"
    ))
    .unwrap();
    let narrow_independent: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/montblanc-narrow-independent-streams.json"
    ))
    .unwrap();
    let narrow_diverse: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tools/montblanc/narrow-probe/diverse-v8-receipt.json"
    ))
    .unwrap();
    let nine_narrow: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tools/montblanc/nine-narrow/streams.json"
    ))
    .unwrap();
    for (name, case) in fixture["cases"]
        .as_object()
        .unwrap()
        .iter()
        .chain(independent["cases"].as_object().unwrap().iter())
        .chain(eight["cases"].as_object().unwrap().iter())
        .chain(narrow["cases"].as_object().unwrap().iter())
        .chain(narrow_independent["cases"].as_object().unwrap().iter())
        .chain(narrow_diverse["cases"].as_object().unwrap().iter())
        .chain(nine_narrow["cases"].as_object().unwrap().iter())
    {
        let number = |v: &serde_json::Value| v.as_f64().unwrap() as f32;
        let points: Vec<[f32; 2]> = case["input"]["points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| [number(&p[0]), number(&p[1])])
            .collect();
        let pressure: Vec<f32> = case["input"]["pressure"]
            .as_array()
            .unwrap()
            .iter()
            .map(number)
            .collect();
        let actual = many_point(Input {
            points: &points,
            pressure: &pressure,
            pen_size: number(&case["input"]["pen_size"]),
        })
        .unwrap();
        let expected = case["stream"].as_array().unwrap();
        assert_eq!(actual.len(), expected.len(), "{name}");
        for (i, (a, e)) in actual.iter().zip(expected).enumerate() {
            let symbol = e[0].as_str().unwrap();
            let prefix = "SPen::MontblancFountainPenStrokeDrawableRTV1::";
            let suffix = "(SPen::Vector3<float>, SPen::Vector2<float>, float)";
            let method = symbol
                .strip_prefix(prefix)
                .and_then(|s| s.strip_suffix(suffix))
                .unwrap_or_else(|| panic!("unexpected native symbol {symbol}"));
            let call = match method {
                "AddStartPoint" => RtCall::Start,
                "AddEndPoint" => RtCall::End,
                "AddPoint" => RtCall::Point,
                _ => panic!("unexpected native symbol {symbol}"),
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
fn many_point_rejects_nonfinite_and_extreme_coordinates() {
    let pressure = [0.5; 6];
    for bad in [f32::NAN, f32::INFINITY, f32::MAX, 1_000_001.0] {
        let points = [
            [0.0, 0.0],
            [6.0, 0.0],
            [12.0, 0.0],
            [18.0, 0.0],
            [24.0, 0.0],
            [bad, 0.0],
        ];
        assert!(
            many_point(Input {
                points: &points,
                pressure: &pressure,
                pen_size: 2.0
            })
            .is_err()
        );
    }
}

#[test]
fn many_point_rejects_excessive_records() {
    let points = [
        [0., 0.],
        [7., 3.],
        [14., -2.],
        [21., 4.],
        [28., -1.],
        [10000., 3.],
    ];
    assert!(
        many_point(Input {
            points: &points,
            pressure: &[0.5; 6],
            pen_size: 0.5
        })
        .is_err()
    );
}

#[test]
fn nine_point_investigative_native_stream() {
    let cases: serde_json::Value =
        serde_json::from_str(include_str!("montblanc_nine_streams.json")).unwrap();
    for case in cases.as_object().unwrap().values() {
        let number = |v: &serde_json::Value| v.as_f64().unwrap() as f32;
        let points: Vec<[f32; 2]> = case["input"]["points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| [number(&p[0]), number(&p[1])])
            .collect();
        let pressure: Vec<f32> = case["input"]["pressure"]
            .as_array()
            .unwrap()
            .iter()
            .map(number)
            .collect();
        let actual = many_point(Input {
            points: &points,
            pressure: &pressure,
            pen_size: number(&case["input"]["pen_size"]),
        })
        .unwrap();
        let expected = case["result"]["stream"].as_array().unwrap();
        assert_eq!(actual.len(), expected.len());
        for (i, (actual, expected)) in actual.iter().zip(expected).enumerate() {
            let symbol = expected[0].as_str().unwrap();
            let call = if symbol.contains("::AddStartPoint(") {
                RtCall::Start
            } else if symbol.contains("::AddEndPoint(") {
                RtCall::End
            } else if symbol.contains("::AddPoint(") {
                RtCall::Point
            } else {
                panic!("unexpected symbol {symbol}")
            };
            assert_eq!(actual.call, call, "record {i}");
            for (lane, word) in expected[1].as_array().unwrap().iter().enumerate() {
                let s = word.as_str().unwrap();
                let bits = u32::from_le_bytes(std::array::from_fn(|j| {
                    u8::from_str_radix(&s[j * 2..j * 2 + 2], 16).unwrap()
                }));
                assert_eq!(actual.lanes[lane].to_bits(), bits, "record {i} lane {lane}");
            }
        }
    }
}

#[test]
fn long_independent_native_stream() {
    let receipt: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/montblanc-long-independent-streams.json"
    ))
    .unwrap();
    let paint: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tools/montblanc/long-paint/streams.json"
    ))
    .unwrap();
    let holdout: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tools/montblanc/long-holdout/streams.json"
    ))
    .unwrap();
    let double: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tools/montblanc/long-2x/streams.json"
    ))
    .unwrap();
    let sizes: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tools/montblanc/long-sizes/streams.json"
    ))
    .unwrap();
    let double_sizes: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tools/montblanc/long-2x-sizes/streams.json"
    ))
    .unwrap();
    let fresh: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tools/montblanc/long-fresh/streams.json"
    ))
    .unwrap();
    let long_sizes_v191: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tools/montblanc/long-size-v191/streams.json"
    ))
    .unwrap();
    let independent_2x: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tools/montblanc/long-2x-independent/streams.json"
    ))
    .unwrap();
    let longer: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tools/montblanc/longer-probe/streams.json"
    ))
    .unwrap();
    let double_long_sizes: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tools/montblanc/long-2x-size-v192/streams.json"
    ))
    .unwrap();
    let long_unit_sizes: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tools/montblanc/long-unit-size-v205/streams.json"
    ))
    .unwrap();
    let double_long_unit_sizes: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tools/montblanc/long-2x-v217/streams.json"
    ))
    .unwrap();
    for case in receipt["cases"]
        .as_object()
        .unwrap()
        .values()
        .chain(paint.as_object().unwrap().values())
        .chain(holdout.as_object().unwrap().values())
        .chain(double.as_object().unwrap().values())
        .chain(sizes.as_object().unwrap().values())
        .chain(fresh.as_object().unwrap().values())
        .chain(long_sizes_v191.as_object().unwrap().values())
        .chain(double_sizes.as_object().unwrap().values())
        .chain(independent_2x.as_object().unwrap().values())
        .chain(double_long_sizes.as_object().unwrap().values())
        .chain(longer.as_object().unwrap().values())
        .chain(long_unit_sizes.as_object().unwrap().values())
        .chain(double_long_unit_sizes.as_object().unwrap().values())
    {
        let number = |v: &serde_json::Value| v.as_f64().unwrap() as f32;
        let points: Vec<[f32; 2]> = case["input"]["points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| [number(&p[0]), number(&p[1])])
            .collect();
        let pressure: Vec<f32> = case["input"]["pressure"]
            .as_array()
            .unwrap()
            .iter()
            .map(number)
            .collect();
        let actual = many_point(Input {
            points: &points,
            pressure: &pressure,
            pen_size: number(&case["input"]["pen_size"]),
        })
        .unwrap();
        let expected = case["stream"].as_array().unwrap();
        assert_eq!(actual.len(), expected.len());
        for (i, (actual, expected)) in actual.iter().zip(expected).enumerate() {
            let symbol = expected[0].as_str().unwrap();
            let call = if symbol.contains("::AddStartPoint(") {
                RtCall::Start
            } else if symbol.contains("::AddEndPoint(") {
                RtCall::End
            } else if symbol.contains("::AddPoint(") {
                RtCall::Point
            } else {
                panic!("unexpected symbol {symbol}")
            };
            assert_eq!(actual.call, call, "record {i}");
            for (lane, word) in expected[1].as_array().unwrap().iter().enumerate() {
                let s = word.as_str().unwrap();
                let bits = u32::from_le_bytes(std::array::from_fn(|j| {
                    u8::from_str_radix(&s[j * 2..j * 2 + 2], 16).unwrap()
                }));
                assert_eq!(actual.lanes[lane].to_bits(), bits, "record {i} lane {lane}");
            }
        }
    }
}

#[test]
fn note_derived_wide_streams_match_native() {
    for fixture in [
        include_str!("../../../../tools/montblanc/note-derived-v302/note3-6/native.json"),
        include_str!("../../../../tools/montblanc/note-short-v310/note6-stream.json"),
        include_str!("../../../../tools/montblanc/note-short-v310/note7-stream.json"),
        include_str!("../../../../tools/montblanc/note-short-v310/wide-stream.json"),
        include_str!("../../../../tools/montblanc/note-short-v310/wider-stream.json"),
        include_str!("../../../../tools/montblanc/note-short-v310/thin-stream.json"),
        include_str!("../../../../tools/montblanc/note-short-v310/pressure-stream.json"),
        include_str!("../../../../tools/montblanc/note-short-v310/threshold-below-stream.json"),
        include_str!("../../../../tools/montblanc/note-short-v310/threshold-equal-stream.json"),
        include_str!("../../../../tools/montblanc/note-short-v310/threshold-above-stream.json"),
        include_str!("../../../../tools/montblanc/note-short-v310/long-stream.json"),
        include_str!("../../../../tools/montblanc/note-derived-v302/note3/native.json"),
        include_str!("../../../../tools/montblanc/note-derived-v302/note3-12/native.json"),
        include_str!("../../../../tools/montblanc/note-derived-v302/note3-9/native.json"),
        include_str!("../../../../tools/montblanc/note-derived-v302/note6/native.json"),
    ] {
        let data: serde_json::Value = serde_json::from_str(fixture).unwrap();
        let num = |v: &serde_json::Value| v.as_f64().unwrap() as f32;
        let points: Vec<[f32; 2]> = data["input"]["points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| [num(&p[0]), num(&p[1])])
            .collect();
        let pressure: Vec<f32> = data["input"]["pressure"]
            .as_array()
            .unwrap()
            .iter()
            .map(num)
            .collect();
        let actual = many_point(Input {
            points: &points,
            pressure: &pressure,
            pen_size: num(&data["input"]["pen_size"]),
        })
        .unwrap();
        let expected = data["stream"].as_array().unwrap();
        assert_eq!(actual.len(), expected.len());
        for (i, (a, e)) in actual.iter().zip(expected).enumerate() {
            let method = e[0].as_str().unwrap();
            let call = match method
                .strip_prefix("SPen::MontblancFountainPenStrokeDrawableRTV1::")
                .and_then(|s| s.strip_suffix("(SPen::Vector3<float>, SPen::Vector2<float>, float)"))
                .unwrap()
            {
                "AddStartPoint" => RtCall::Start,
                "AddEndPoint" => RtCall::End,
                "AddPoint" => RtCall::Point,
                _ => panic!("unexpected method {method}"),
            };
            assert_eq!(a.call, call, "call {i}");
            for (lane, word) in e[1].as_array().unwrap().iter().enumerate() {
                let word = word.as_str().unwrap();
                let bits = u32::from_le_bytes(std::array::from_fn(|j| {
                    u8::from_str_radix(&word[j * 2..j * 2 + 2], 16).unwrap()
                }));
                assert_eq!(a.lanes[lane].to_bits(), bits, "record {i} lane {lane}");
            }
        }
    }
}
