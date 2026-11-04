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
fn nine_point_retained_native_capture_integrity() {
    use base64::Engine;
    use sha2::{Digest, Sha256};
    use std::io::Read;

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let directory = "tools/montblanc/nine-point";
    let manifest: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tools/montblanc/nine-point/manifest.json"
    ))
    .unwrap();
    let checked_bytes = |path: &str| {
        let bytes = std::fs::read(root.join(path)).unwrap();
        assert_eq!(
            format!("{:x}", Sha256::digest(&bytes)),
            manifest["sha256"][path],
            "{path}"
        );
        bytes
    };
    // Historical exporter pins remain in the manifest; only live native inputs
    // are required by this retained-capture check.
    for name in [
        "capture.py",
        "capture-expanded-1x.py",
        "capture-expanded-2x.py",
        "expanded-streams.json",
    ] {
        checked_bytes(&format!("{directory}/{name}"));
    }
    for path in [
        "conformance/montblanc-nine-point-native.json",
        "crates/sdocx/src/ink/montblanc_nine_streams.json",
    ] {
        checked_bytes(path);
    }
    let mut pairs = vec![("capture.json.gz".to_owned(), "frame.rgba.zlib".to_owned())];
    for scale in ["1x", "2x"] {
        for case in ["loop", "reverse"] {
            pairs.push((
                format!("{case}-{scale}.json.gz"),
                format!("{case}-{scale}.rgba.zlib"),
            ));
        }
    }
    for (capture_name, frame_name) in pairs {
        let capture_bytes = checked_bytes(&format!("{directory}/{capture_name}"));
        let capture: serde_json::Value =
            serde_json::from_reader(flate2::read::GzDecoder::new(capture_bytes.as_slice()))
                .unwrap();
        let compressed = checked_bytes(&format!("{directory}/{frame_name}"));
        let mut frame = Vec::new();
        flate2::read::ZlibDecoder::new(compressed.as_slice())
            .read_to_end(&mut frame)
            .unwrap();
        assert_eq!(frame.len(), 256 * 256 * 4, "{frame_name}");
        assert_eq!(
            format!("{:x}", Sha256::digest(&frame)),
            capture["post_readback"]["sha256"],
            "{frame_name}"
        );
        let embedded = base64::engine::general_purpose::STANDARD
            .decode(
                capture["post_readback"]["rgba8_zlib_base64"]
                    .as_str()
                    .unwrap(),
            )
            .unwrap();
        let mut original = Vec::new();
        flate2::read::ZlibDecoder::new(embedded.as_slice())
            .read_to_end(&mut original)
            .unwrap();
        assert_eq!(frame, original, "{frame_name}");
    }
}

#[test]
fn independent_redraw_window_retained_observations() {
    use sha2::{Digest, Sha256};

    let receipt: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tools/montblanc/redraw-window/independent-native.json"
    ))
    .unwrap();
    let hash = |bytes: &[u8]| format!("{:x}", Sha256::digest(bytes));
    // Match the compact, key-sorted JSON used by the historical receipt verifier.
    fn canonical(value: &serde_json::Value) -> String {
        match value {
            serde_json::Value::Object(map) => {
                let sorted: std::collections::BTreeMap<_, _> = map.iter().collect();
                format!(
                    "{{{}}}",
                    sorted
                        .into_iter()
                        .map(|(key, value)| {
                            format!(
                                "{}:{}",
                                serde_json::to_string(key).unwrap(),
                                canonical(value)
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(",")
                )
            }
            serde_json::Value::Array(values) => format!(
                "[{}]",
                values.iter().map(canonical).collect::<Vec<_>>().join(",")
            ),
            _ => serde_json::to_string(value).unwrap(),
        }
    }
    let hex = |text: &str, length: usize| {
        text.len() == length
            && text
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    };
    assert_eq!(
        receipt["capture_script_sha256"],
        hash(include_bytes!(
            "../../../../tools/montblanc/narrow-probe/capture.py"
        ))
    );
    let pins = serde_json::json!({
        "base": "e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb",
        "drawing": "788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd",
        "graphics": "aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621",
        "libcxx": "4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4",
        "marker3_adapter": "9b1eafd3dd07d1b491148d2635aad004c83ac401416f3c8a9c62b1e60099b7a8",
        "montblanc": "4c808cf7a1c3685e6c99e654bb8132e3ad95db4d28fef931f6936bed3d5b8350",
        "pencommon": "afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d",
        "renderer": "f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed",
        "runner": "a8647b96fa001dbe387de8c231ed4895d2f7b5ef619240835c17867366f8c0c8"
    });
    let gl = serde_json::json!({
        "libEGL.so.1": "f4b60a4618fbf760f395f64dfd53320446ba1011151a80e411742b3232b194e6",
        "libGLESv2.so.2": "a705c66de30ca4e883120c70acae0f1011d7c3f97605eef331e03493beae3812"
    });
    let expected = [
        (
            "zigzag10",
            10,
            2.4,
            32,
            14,
            9,
            "d43778afc8ffa96c68e232d0b6385bb4d8e749fc33bd33735ea062fc731532a9",
            "c34579f90b83dabf0039d24647a168d4a0fd5f9e6e44d1a541a4aab53b38c50e",
            "e2f5b8860e9cda2d47ae1c8329df45fbd98dd609b69891d727868b5bfb5f59da",
        ),
        (
            "vertical12",
            12,
            0.6,
            44,
            14,
            21,
            "25c513fc53428a1f61e82a7af6889df3de36e3d651b25cbba700fa261d1d07e7",
            "3ccb999b08a42af180979c47e2a1f56f23562442955d0dfee22d653041914286",
            "045c8c626ec7a2b55a79d057f30eb7331ac47a208126f6e3873bb2d5be600586",
        ),
        (
            "turn20",
            20,
            3.0,
            49,
            28,
            14,
            "9987fbfcc6dd333e48722446a784f3c9e240eb2da24be8c9479195d334cbcff4",
            "5d5e8d72b663f1c6c904015366cd2114fb8d592345f14d6a44f99651a62b814b",
            "bccdecdf8610a8377f52707c8e76a88ba69de34a8c8085b062278a0a89957146",
        ),
        (
            "loop32",
            32,
            1.2,
            118,
            15,
            95,
            "91b5074f05e64e36e5b3ddd51be5acbff748658f7e2409b1946c8cbc18faf919",
            "2569a0300862a36089ca4ef1e950d2b3505e3cf0b88f8a099843b7a308eaa03b",
            "88dd9438c22afdf428296f9a2974129843a1ea3d74026f70cd4c68b14d5aca60",
        ),
    ];
    let cases = receipt["cases"].as_object().unwrap();
    assert_eq!(cases.len(), expected.len());
    for (name, count, size, first, replay, offset, input_hash, stream_hash, frame_hash) in expected
    {
        let case = &cases[name];
        assert_eq!(
            hash(canonical(&case["input"]).as_bytes()),
            input_hash,
            "{name} input"
        );
        assert_eq!(case["input"]["points"].as_array().unwrap().len(), count);
        assert_eq!(case["input"]["pressure"].as_array().unwrap().len(), count);
        assert_eq!(case["input"]["pen_size"].as_f64().unwrap(), size);
        assert_eq!(case["input_pins"], pins);
        assert_eq!(case["host_gl_sha256"], gl);
        assert!(hex(case["frame_sha256"].as_str().unwrap(), 64));
        assert_eq!(case["frame_sha256"], frame_hash);
        assert_eq!(
            hash(canonical(&case["stream"]).as_bytes()),
            stream_hash,
            "{name} stream"
        );
        let stream = case["stream"].as_array().unwrap();
        assert_eq!(stream.len(), first + 1 + replay);
        for (index, row) in stream.iter().enumerate() {
            assert_eq!(row.as_array().unwrap().len(), 2);
            let kind = if index == 0 {
                "AddStartPoint"
            } else if index == first {
                "AddEndPoint"
            } else {
                "AddPoint"
            };
            assert_eq!(
                row[0],
                format!(
                    "SPen::MontblancFountainPenStrokeDrawableRTV1::{kind}(SPen::Vector3<float>, SPen::Vector2<float>, float)"
                )
            );
            let words = row[1].as_array().unwrap();
            assert_eq!(words.len(), 6);
            assert!(words.iter().all(|word| hex(word.as_str().unwrap(), 8)));
        }
        let matches: Vec<_> = (0..=first - replay)
            .filter(|&i| stream[i..i + replay] == stream[first + 1..])
            .collect();
        assert_eq!(matches, [offset], "{name} unique replay slice");
    }
}

#[test]
fn redraw_window_retained_trace_and_export_integrity() {
    use serde_json::Value;
    use sha2::{Digest, Sha256};

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let directory = "tools/montblanc/redraw-window";
    let read = |name: &str| -> Value {
        serde_json::from_slice(&std::fs::read(root.join(directory).join(name)).unwrap()).unwrap()
    };
    let file_hash = |name: &str| {
        format!(
            "{:x}",
            Sha256::digest(std::fs::read(root.join(name)).unwrap())
        )
    };
    fn unhex(value: &Value) -> Vec<u8> {
        value
            .as_str()
            .unwrap()
            .split_ascii_whitespace()
            .flat_map(|word| {
                assert_eq!(word.len() % 2, 0);
                assert!(word.bytes().all(|byte| byte.is_ascii_hexdigit()));
                (0..word.len())
                    .step_by(2)
                    .map(|i| u8::from_str_radix(&word[i..i + 2], 16).unwrap())
                    .collect::<Vec<_>>()
            })
            .collect()
    }
    fn check_words(stream: &[Value]) {
        for row in stream {
            assert_eq!(row.as_array().unwrap().len(), 2);
            let words = row[1].as_array().unwrap();
            assert_eq!(words.len(), 6);
            assert!(words.iter().all(|word| unhex(word).len() == 4));
        }
    }
    let symbol = |kind: &str| {
        format!(
            "SPen::MontblancFountainPenStrokeDrawableRTV1::{kind}(SPen::Vector3<float>, SPen::Vector2<float>, float)"
        )
    };
    let receipt = read("streams.json");
    assert_eq!(
        file_hash("tools/montblanc/narrow-probe/capture.py"),
        receipt["capture_source_sha256"]
    );
    let cases = receipt["cases"].as_object().unwrap();
    let expected: [(&str, usize, usize, &[usize]); 7] = [
        ("9", 45, 36, &[]),
        ("10-distinct", 49, 18, &[18]),
        ("11", 54, 27, &[18]),
        ("12", 58, 18, &[27]),
        ("13", 63, 27, &[27]),
        ("duplicate10", 45, 36, &[]),
        ("duplicate12", 45, 36, &[]),
    ];
    assert_eq!(cases.len(), expected.len());
    for (name, split, replay, expected_offsets) in expected {
        let case = &cases[name];
        let source = &case["input"];
        let points = source["points"].as_array().unwrap();
        let pressures = source["pressure"].as_array().unwrap();
        assert_eq!(points.len(), pressures.len());
        assert!(
            points
                .iter()
                .all(|point| point.as_array().unwrap().len() == 2)
        );
        assert!(
            pressures
                .iter()
                .all(|value| (0.0..=1.0).contains(&value.as_f64().unwrap()))
        );
        assert!(source["pen_size"].as_f64().unwrap() > 0.0);
        assert_eq!(case["first_count"].as_u64().unwrap() as usize, split);
        assert_eq!(case["replay_count"].as_u64().unwrap() as usize, replay);
        assert_eq!(
            case["full_word_matching_offsets"],
            serde_json::json!(expected_offsets)
        );
        let records = case["stream"].as_array().unwrap();
        assert_eq!(records.len(), split + replay);
        assert_eq!(records[0][0], symbol("AddStartPoint"));
        assert_eq!(records[split - 1][0], symbol("AddEndPoint"));
        for row in records[1..split - 1].iter().chain(&records[split..]) {
            assert_eq!(row[0], symbol("AddPoint"));
        }
        check_words(records);
        let offsets: Vec<_> = (0..split)
            .filter(|&i| {
                i + replay <= split
                    && records[i..i + replay]
                        .iter()
                        .zip(&records[split..])
                        .all(|(x, y)| {
                            x[1] == y[1] && x[0] == symbol("AddPoint") && y[0] == symbol("AddPoint")
                        })
            })
            .collect();
        assert_eq!(offsets, expected_offsets, "{name}");
        for digest in std::iter::once(&case["frame_sha256"])
            .chain(case["input_pins"].as_object().unwrap().values())
            .chain(case["host_gl_sha256"].as_object().unwrap().values())
        {
            assert_eq!(unhex(digest).len(), 32);
        }
    }
    let nine = &cases["9"];
    let stream = nine["stream"].as_array().unwrap();
    let split = nine["first_count"].as_u64().unwrap() as usize;
    let replay = nine["replay_count"].as_u64().unwrap() as usize;
    assert_eq!(
        stream[0][1].as_array().unwrap()[..5],
        stream[split][1].as_array().unwrap()[..5]
    );
    assert_ne!(stream[0][1][5], stream[split][1][5]);
    assert_eq!(stream[1..replay], stream[split + 1..]);
    for (name, copies) in [("duplicate10", 1), ("duplicate12", 3)] {
        let case = &cases[name];
        for channel in ["points", "pressure"] {
            let base = nine["input"][channel].as_array().unwrap();
            let expected: Vec<_> = base[..3]
                .iter()
                .cloned()
                .chain(std::iter::repeat_n(base[2].clone(), copies))
                .chain(base[3..].iter().cloned())
                .collect();
            assert_eq!(case["input"][channel], serde_json::json!(expected));
        }
        assert_eq!(case["input"]["pen_size"], nine["input"]["pen_size"]);
        assert_eq!(case["stream"], nine["stream"]);
    }

    let trace = read("event-traces.json");
    assert_eq!(
        file_hash(&format!("{directory}/trace.py")),
        trace["trace_script_sha256"]
    );
    assert_eq!(
        trace["capture_script_sha256"],
        receipt["capture_source_sha256"]
    );
    let traces = trace["cases"].as_object().unwrap();
    assert_eq!(traces.len(), expected.len() + 2);
    for name in expected
        .iter()
        .map(|row| row.0)
        .chain(["pressure-duplicate", "short-move"])
    {
        assert!(traces.contains_key(name));
    }
    let variants = [
        "duplicate10",
        "duplicate12",
        "pressure-duplicate",
        "short-move",
    ];
    for (name, case) in traces {
        if let Some(source) = cases.get(name) {
            assert_eq!(case["input"], source["input"]);
            assert_eq!(case["stream"], source["stream"]);
        }
        let calls = case["draw_calls"].as_array().unwrap();
        assert_eq!(calls.len(), 5);
        let base = if variants.contains(&name.as_str()) {
            &traces["9"]["input"]["points"]
        } else {
            &case["input"]["points"]
        }
        .as_array()
        .unwrap();
        let actual: Vec<[f64; 2]> = calls
            .iter()
            .map(|row| {
                std::array::from_fn(|i| {
                    f32::from_le_bytes(unhex(&row["s0_s5_f32_bits"][i]).try_into().unwrap()) as f64
                })
            })
            .collect();
        let expected: Vec<[f64; 2]> = base[base.len() - 6..base.len() - 1]
            .iter()
            .map(|point| [point[0].as_f64().unwrap(), point[1].as_f64().unwrap()])
            .collect();
        assert_eq!(actual, expected, "{name}");
        assert!(calls.iter().all(|row| row["lr"] == "0xb01dd90"));
        assert_eq!(case["end_state"]["lr"], "0xb01e7a0");
        assert_eq!(unhex(&case["end_state"]["motion_event_bytes"]).len(), 160);
        assert!(
            calls
                .iter()
                .all(|row| unhex(&row["receiver_bytes_80_144"]).len() == 64)
        );
    }
    for name in variants {
        assert_eq!(traces[name]["stream"], traces["9"]["stream"]);
        let words = |case: &Value| {
            case["draw_calls"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| row["s0_s5_f32_bits"].clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(words(&traces[name]), words(&traces["9"]));
    }
    let base = &traces["9"]["input"];
    for (name, delta) in [("pressure-duplicate", 0.0), ("short-move", 1.0)] {
        let points = base["points"].as_array().unwrap();
        let inserted = serde_json::json!([
            points[2][0].as_f64().unwrap() + delta,
            points[2][1].as_f64().unwrap()
        ]);
        let expected: Vec<_> = points[..3]
            .iter()
            .cloned()
            .chain([inserted])
            .chain(points[3..].iter().cloned())
            .collect();
        let actual = traces[name]["input"]["points"].as_array().unwrap();
        assert_eq!(actual.len(), expected.len());
        for (a, b) in actual.iter().zip(expected) {
            assert_eq!(a.as_array().unwrap().len(), 2);
            for i in 0..2 {
                assert_eq!(a[i].as_f64(), b[i].as_f64());
            }
        }
        let pressure = base["pressure"].as_array().unwrap();
        let expected: Vec<_> = pressure[..3]
            .iter()
            .cloned()
            .chain([serde_json::json!(0.9)])
            .chain(pressure[3..].iter().cloned())
            .collect();
        assert_eq!(
            traces[name]["input"]["pressure"],
            serde_json::json!(expected)
        );
        assert_eq!(traces[name]["input"]["pen_size"], base["pen_size"]);
    }

    let irregular = read("irregular-native.json");
    assert_eq!(irregular["input"], read("irregular-input.json"));
    assert_eq!(
        irregular["trace_source_sha256"],
        trace["trace_script_sha256"]
    );
    assert_eq!(
        irregular["candidate_patch_sha256"],
        file_hash(&format!("{directory}/irregular-candidate.patch"))
    );
    assert_eq!(irregular["stream"].as_array().unwrap().len(), 83);
    assert_eq!(irregular["candidate_result"]["callback_count"], 83);
    assert_eq!(
        irregular["candidate_result"]["methods_and_six_f32_words_equal"],
        true
    );
    assert_eq!(irregular["draw_calls"].as_array().unwrap().len(), 5);
    assert!(
        irregular["draw_calls"]
            .as_array()
            .unwrap()
            .iter()
            .all(|row| row["lr"] == "0xb01dd90")
    );
    check_words(irregular["stream"].as_array().unwrap());
    let variants = read("irregular-variants.json");
    assert_eq!(
        variants["candidate_patch_sha256"],
        irregular["candidate_patch_sha256"]
    );
    assert_eq!(
        variants["trace_source_sha256"],
        trace["trace_script_sha256"]
    );
    assert_eq!(
        variants["candidate_results"],
        serde_json::json!({"short14": 73, "long17": 97, "reverse17": 95})
    );
    let base = &irregular["input"];
    let points = base["points"].as_array().unwrap();
    let pressure = base["pressure"].as_array().unwrap();
    assert_eq!(
        variants["cases"]["short14"]["input"],
        serde_json::json!({"points": &points[..14], "pressure": &pressure[..14], "pen_size": base["pen_size"]})
    );
    let long_points: Vec<_> = points
        .iter()
        .cloned()
        .chain([serde_json::json!([97, 8])])
        .collect();
    let long_pressure: Vec<_> = pressure
        .iter()
        .cloned()
        .chain([serde_json::json!(0.48)])
        .collect();
    assert_eq!(
        variants["cases"]["long17"]["input"],
        serde_json::json!({"points": long_points, "pressure": long_pressure, "pen_size": base["pen_size"]})
    );
    assert_eq!(
        variants["cases"]["reverse17"]["input"],
        serde_json::json!({"points": long_points.iter().rev().collect::<Vec<_>>(), "pressure": long_pressure.iter().rev().collect::<Vec<_>>(), "pen_size": base["pen_size"]})
    );
    for (name, case) in variants["cases"].as_object().unwrap() {
        assert_eq!(
            case["stream"].as_array().unwrap().len() as u64,
            variants["candidate_results"][name].as_u64().unwrap()
        );
        assert_eq!(case["draw_calls"].as_array().unwrap().len(), 5);
        assert!(
            case["draw_calls"]
                .as_array()
                .unwrap()
                .iter()
                .all(|row| row["lr"] == "0xb01dd90")
        );
        check_words(case["stream"].as_array().unwrap());
    }

    // These assertions validate historical summaries, not fresh reader output.
    let exports = read("export-receipt.json");
    assert_eq!(exports["complete"], true);
    assert_eq!(exports["cases"].as_object().unwrap().len(), 8);
    for path in [
        "tools/montblanc/redraw-window/irregular-candidate.patch",
        "tools/montblanc/many-point-public/src/main.rs",
        "tools/montblanc/many-point-public/Cargo.toml",
        "tools/montblanc/many-point-public/Cargo.lock",
        "tools/montblanc/redraw-window/export-compare.py",
    ] {
        assert_eq!(exports["source_sha256"][path], file_hash(path));
    }
    let isolated = exports["isolated_source_sha256"].as_object().unwrap();
    assert_eq!(isolated.len(), 2);
    for mode in ["baseline", "candidate"] {
        let values = isolated[mode].as_object().unwrap();
        assert_eq!(values.len(), 3);
        for key in ["sampler_dispatch", "sampler", "generated_driver"] {
            assert_eq!(unhex(&values[key]).len(), 32);
        }
    }
    assert_eq!(
        isolated["baseline"]["generated_driver"],
        isolated["candidate"]["generated_driver"]
    );
    assert_ne!(
        isolated["baseline"]["sampler"],
        isolated["candidate"]["sampler"]
    );
    for case in exports["cases"].as_object().unwrap().values() {
        for reader in ["svg", "pdf", "pdfium"] {
            let baseline = case["metrics"]["baseline"][reader].as_array().unwrap();
            let candidate = case["metrics"]["candidate"][reader].as_array().unwrap();
            assert_eq!(baseline.len(), 5);
            assert_eq!(candidate.len(), 5);
            assert!(candidate[4].as_u64().unwrap() < baseline[4].as_u64().unwrap());
            assert_eq!(candidate[2], candidate[0]);
            assert!(candidate[1].as_u64().unwrap() > candidate[0].as_u64().unwrap());
        }
    }
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
