//! Saved Marker4 V7/V8: midpoint sampling and a rounded rectangular tip.
//! Traced from GLV7/RTV3 and GLV8/RTV4 in Samsung Notes 4.4.45.37. The vector tip
//! approximates the native 200px mask's filtering, not its exact GPU coverage.
use super::{
    RectStamp,
    path::{P, Quad},
};
use crate::{Point, Stroke};

pub(super) struct Ink {
    pub points: Vec<Point>,
    pub sample_ends: Vec<usize>,
    pub stamp: RectStamp,
    pub opacity: f64,
}

pub(super) fn prepare(s: &Stroke) -> Option<Ink> {
    let r = s.rendering.as_ref()?;
    // Drawing reads the saved Model style size, then Pen::SetSize clamps it.
    // The decoder's public width may carry a separate presentation default.
    let requested_size = r.style.pen_size.unwrap_or(0.0);
    if !requested_size.is_finite() || requested_size < 0.0 {
        return None;
    }
    let size = requested_size.clamp(0.4, 800.0);
    let v7 = r.advanced_settings.as_deref() == Some("7;");
    if v7
        && (r.tool_type_raw != 2
            || r.properties.straighten
            || r.properties.fixed_width
            || s.pressures.len() != s.points.len()
            || s.timestamps.len() != s.points.len())
    {
        return None;
    }
    if !matches!(
        r.pen_name.as_deref(),
        Some(
            "com.samsung.android.sdk.pen.pen.preload.Marker4"
                | "com.samsung.android.sdk.pen.pen.preload.StraightHighlighter"
        )
    ) || !matches!(r.advanced_settings.as_deref(), Some("7;" | "8;"))
        || r.properties.eraser
        || r.properties.rainbow_effect
        || !matches!(r.tool_type_raw, 0 | 2)
        || s.points.is_empty()
        || s.points.len() > 100_000
        || s.points
            .iter()
            .any(|p| !p.x.is_finite() || !p.y.is_finite() || p.x.abs() > 1e7 || p.y.abs() > 1e7)
        || s.points
            .windows(2)
            .map(|p| (p[1].x - p[0].x).hypot(p[1].y - p[0].y))
            .sum::<f64>()
            > 400_000.
    {
        return None;
    }
    let mut previous = P::from(s.points[0]);
    let mut midpoint = previous;
    let mut residual = 1.;
    let mut alternate = true;
    let mut angle = None;
    let mut points = Vec::new();
    let mut sample_ends = vec![0; s.points.len()];
    for (i, &point) in s.points.iter().enumerate().skip(1) {
        let p = P::from(point);
        let last = i + 1 == s.points.len();
        let distance = p.sub(previous).len();
        if !last {
            if distance < 2. {
                sample_ends[i] = points.len();
                continue;
            }
            if r.tool_type_raw == 0 && distance < 20. {
                alternate = !alternate;
                if !alternate {
                    sample_ends[i] = points.len();
                    continue;
                }
            } else {
                alternate = true;
            }
        }
        let end = if last { p } else { previous.mid(p) };
        let q = Quad::new(midpoint, previous, end);
        let mut d = residual;
        while q.length > 0. && d <= q.length {
            let at = q.at(d);
            // Tool 0 locks the angle from the first sampled tangent. Native
            // repeats that lookup ten times before advancing the distance.
            if r.tool_type_raw == 0 && angle.is_none() {
                let direction = q.tangent(d);
                angle = Some(direction.y.atan2(direction.x) as f64);
            }
            points.push(Point {
                x: at.x as f64,
                y: at.y as f64,
            });
            d += 1.;
        }
        residual = d - q.length;
        midpoint = end;
        if last {
            // endPen emits the endpoint when samples were drawn, otherwise
            // the previous input provides the single-dot fallback.
            points.push(if points.is_empty() {
                Point {
                    x: previous.x as f64,
                    y: previous.y as f64,
                }
            } else {
                point
            });
        }
        previous = p;
        sample_ends[i] = points.len();
    }
    if s.points.len() == 1 {
        points.push(s.points[0]);
        sample_ends[0] = 1;
    }
    if v7 {
        let diameter = f64::from((size * 0.5).max(1.) + 0.5) * 2.;
        for p in &mut points {
            p.x = f64::from(p.x as f32);
            p.y = f64::from(p.y as f32);
        }
        return Some(Ink {
            points,
            sample_ends,
            stamp: RectStamp {
                width: 0.77 * diameter * 0.99,
                height: diameter * 0.99,
                angle: 0.,
            },
            opacity: r.style.color_argb.map_or(1., |c| f64::from(c >> 24) / 255.),
        });
    }
    let size = f64::from(size.trunc());
    // RTV4's static quad is centered on each saved AddPoint position; its
    // captured local bounds are x=±0.77*(size/2+0.5), y=±(size/2+0.5).
    // Its 200px mask covers [1,199] with elliptical corner radii 50.
    let angle = angle.unwrap_or(0.);
    Some(Ink {
        points,
        sample_ends,
        stamp: RectStamp {
            width: 0.77 * (size + 1.) * 0.99,
            height: size * 0.99,
            angle,
        },
        opacity: r.style.color_argb.map_or(1., |c| f64::from(c >> 24) / 255.),
    })
}

#[cfg(all(test, feature = "serde"))]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Reference {
        stroke: Stroke,
        cases: Vec<Case>,
    }

    #[derive(Deserialize)]
    struct Case {
        name: String,
        size: f32,
        samples: Vec<[f64; 4]>,
        dots: Vec<[f64; 4]>,
        sample_ends: Vec<usize>,
        tip_bounds: [f64; 4],
    }

    fn reference() -> Reference {
        serde_json::from_str(include_str!("../../../../conformance/marker4-v7.json")).unwrap()
    }

    #[test]
    fn v7_geometry_matches_native_saved_redraw() {
        let reference = reference();
        for case in reference.cases {
            let mut stroke = reference.stroke.clone();
            stroke.pen_width = case.size;
            stroke.rendering.as_mut().unwrap().style.pen_size = Some(case.size);
            for [x, y, pressure, tilt] in case.samples {
                stroke.points.push(Point { x, y });
                stroke.pressures.push(pressure);
                stroke.tilts.push(tilt);
                stroke.timestamps.push(stroke.timestamps.len() as i64 * 8);
            }
            let actual = prepare(&stroke).unwrap();
            assert_eq!(actual.sample_ends, case.sample_ends, "{}", case.name);
            assert_eq!(actual.points.len(), case.dots.len(), "{}", case.name);
            for (point, [x, y, _, angle]) in actual.points.iter().zip(&case.dots) {
                assert!((point.x - x).abs() < 1e-4, "{} x", case.name);
                assert!((point.y - y).abs() < 1e-4, "{} y", case.name);
                assert_eq!(actual.stamp.angle, *angle, "{} angle", case.name);
            }
            let [left, top, right, bottom] = case.tip_bounds;
            assert!((actual.stamp.width - (right - left) * 0.99).abs() < 1e-4);
            assert!((actual.stamp.height - (bottom - top) * 0.99).abs() < 1e-4);
        }
    }

    #[test]
    fn production_geometry_matches_drawing_size_and_alias_matrix() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../conformance/marker4-saved-caller-native.json"
        ))
        .unwrap();
        let source_float = |v: &serde_json::Value| {
            f32::from_bits(
                u32::from_str_radix(v.as_str().unwrap(), 16)
                    .unwrap()
                    .swap_bytes(),
            )
        };
        let register_float = |v: &serde_json::Value| {
            f32::from_bits(
                u32::from_str_radix(v.as_str().unwrap().trim_start_matches("0x"), 16).unwrap(),
            )
        };
        let cases = fixture["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 14);
        for case in cases {
            let input = &case["input"];
            let mut stroke = reference().stroke;
            stroke.pen_width = 99.;
            stroke.points = input["points_f32_bits"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| Point {
                    x: f64::from(source_float(&p[0])),
                    y: f64::from(source_float(&p[1])),
                })
                .collect();
            stroke.pressures = input["pressure_f32_bits"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| f64::from(source_float(v)))
                .collect();
            stroke.tilts = input["tilt_f32_bits"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| f64::from(source_float(v)))
                .collect();
            stroke.orientations = input["orientation_f32_bits"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| f64::from(source_float(v)))
                .collect();
            stroke.timestamps = input["timestamps_i32"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_i64().unwrap())
                .collect();
            let rendering = stroke.rendering.as_mut().unwrap();
            rendering.pen_name = Some(input["pen_name"].as_str().unwrap().into());
            rendering.advanced_settings = Some(input["advanced_setting"].as_str().unwrap().into());
            rendering.tool_type_raw = input["tool_type"].as_u64().unwrap() as u16;
            rendering.style.pen_size = input["size_setter_called"]
                .as_bool()
                .unwrap()
                .then(|| input["requested_size"].as_f64().unwrap() as f32);
            let is_v7 = input["advanced_setting"] == "7;";
            let actual = prepare(&stroke).unwrap();
            let native = case["streams"]["saved_RedrawPen_addpoint_registers"]
                .as_array()
                .unwrap();
            assert_eq!(actual.points.len(), native.len(), "{}", case["case_id"]);
            for (point, record) in actual.points.iter().zip(native) {
                let registers = &record["float_registers"];
                let angle = if is_v7 {
                    register_float(&registers[3])
                } else {
                    register_float(&registers[2])
                } as f64;
                let x = f64::from(register_float(&registers[0]));
                let y = f64::from(register_float(&registers[1]));
                assert!(
                    (point.x - x).abs() < 1e-4 && (point.y - y).abs() < 1e-4,
                    "{} point {point:?} vs ({x},{y})",
                    case["case_id"]
                );
                assert!((actual.stamp.angle - angle).abs() < 1e-6);
                if is_v7 {
                    let diameter = (f64::from(register_float(&registers[2])) + 0.5) * 2.;
                    assert!((actual.stamp.height - diameter * 0.99).abs() < 1e-5);
                }
            }
        }
    }

    #[test]
    fn v8_rotated_stamp_centers_remain_at_saved_addpoint_positions() {
        let mut stroke = reference().stroke;
        stroke.points = vec![Point { x: 20., y: 20. }, Point { x: 20., y: 40. }];
        stroke.pressures = vec![0.5; 2];
        stroke.timestamps = vec![0, 10];
        let rendering = stroke.rendering.as_mut().unwrap();
        rendering.advanced_settings = Some("8;".into());
        rendering.tool_type_raw = 0;
        rendering.style.pen_size = Some(36.21);

        let actual = prepare(&stroke).unwrap();
        assert!((actual.stamp.angle - std::f64::consts::FRAC_PI_2).abs() < 1e-6);
        // The sampled quadratic controls the first Y; a rotated local-center
        // shift would incorrectly move every point's X away from 20.
        assert!(actual.points.first().unwrap().y > 20.);
        assert_eq!(actual.points.last(), Some(&Point { x: 20., y: 40. }));
        assert!(actual.points.iter().all(|point| point.x == 20.));
    }

    #[test]
    fn saved_style_size_and_alias_match_native_caller_controls() {
        let mut stroke = reference().stroke;
        stroke.points = vec![Point { x: 20., y: 20. }, Point { x: 40., y: 20. }];
        stroke.pressures = vec![0.5; 2];
        stroke.timestamps = vec![0, 10];
        stroke.pen_width = 99.; // Presentation width must not replace saved size.
        for (settings, controls) in [
            (
                "7;",
                [
                    (None, 2.97),
                    (Some(0.), 2.97),
                    (Some(0.1), 2.97),
                    (Some(2.), 2.97),
                    (Some(10.), 10.89),
                ],
            ),
            (
                "8;",
                [
                    (None, 0.),
                    (Some(0.), 0.),
                    (Some(0.1), 0.),
                    (Some(2.), 1.98),
                    (Some(10.), 9.9),
                ],
            ),
        ] {
            stroke.rendering.as_mut().unwrap().advanced_settings = Some(settings.into());
            for (size, height) in controls {
                let rendering = stroke.rendering.as_mut().unwrap();
                rendering.style.pen_size = size;
                rendering.pen_name = Some("com.samsung.android.sdk.pen.pen.preload.Marker4".into());
                let native = prepare(&stroke).unwrap();
                assert!(
                    (native.stamp.height - height).abs() < 1e-5,
                    "{settings} {size:?}"
                );
                stroke.rendering.as_mut().unwrap().pen_name =
                    Some("com.samsung.android.sdk.pen.pen.preload.StraightHighlighter".into());
                let alias = prepare(&stroke).unwrap();
                assert_eq!(alias.points, native.points);
                assert_eq!(alias.sample_ends, native.sample_ends);
                assert_eq!(alias.stamp.width, native.stamp.width);
                assert_eq!(alias.stamp.height, native.stamp.height);
                assert_eq!(alias.stamp.angle, native.stamp.angle);
                assert_eq!(alias.opacity, native.opacity);
            }
        }
    }

    #[test]
    fn v7_unverified_settings_and_missing_channels_use_fallback() {
        let mut stroke = reference().stroke;
        stroke.points = vec![Point { x: 20., y: 20. }];
        stroke.pressures = vec![0.5];
        stroke.timestamps = vec![0];
        assert!(prepare(&stroke).is_some());
        for tool in [0, 1, 3] {
            stroke.rendering.as_mut().unwrap().tool_type_raw = tool;
            assert!(prepare(&stroke).is_none());
        }
        stroke.rendering.as_mut().unwrap().tool_type_raw = 2;
        stroke.rendering.as_mut().unwrap().properties.straighten = true;
        assert!(prepare(&stroke).is_none());
        stroke.rendering.as_mut().unwrap().properties.straighten = false;
        stroke.pressures.clear();
        assert!(prepare(&stroke).is_none());
    }

    #[test]
    fn v8_finger_sampler_matches_native_v7_finger_geometry() {
        let controls: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../conformance/marker4-v7-finger-native.json"
        ))
        .unwrap();
        for case in controls["cases"].as_array().unwrap() {
            let mut stroke = reference().stroke;
            let rendering = stroke.rendering.as_mut().unwrap();
            rendering.advanced_settings = Some("8;".into());
            rendering.tool_type_raw = 0;
            rendering.style.pen_size = Some(case["size"].as_f64().unwrap() as f32);
            for sample in case["samples"].as_array().unwrap() {
                stroke.points.push(Point {
                    x: sample[0].as_f64().unwrap(),
                    y: sample[1].as_f64().unwrap(),
                });
            }
            let actual = prepare(&stroke).unwrap();
            let dots = case["dots"].as_array().unwrap();
            let radius = (case["size"].as_f64().unwrap() as f32 * 0.5).max(1.0);
            let half_height = f64::from(radius + 0.5);
            let bounds = case["tip_bounds"].as_array().unwrap();
            for (value, expected) in bounds.iter().zip([
                -0.77 * half_height,
                -half_height,
                0.77 * half_height,
                half_height,
            ]) {
                assert!(
                    (value.as_f64().unwrap() - expected).abs() < 1e-4,
                    "{} tip",
                    case["name"]
                );
            }
            let ends: Vec<usize> = case["sample_ends"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as usize)
                .collect();
            assert_eq!(actual.sample_ends, ends, "{}", case["name"]);
            assert_eq!(actual.points.len(), dots.len(), "{}", case["name"]);
            for (point, dot) in actual.points.iter().zip(dots) {
                assert!(
                    (dot[2].as_f64().unwrap() - f64::from(radius)).abs() < 1e-4,
                    "{} radius",
                    case["name"]
                );
                assert!(
                    (point.x - dot[0].as_f64().unwrap()).abs() < 1e-4,
                    "{} x",
                    case["name"]
                );
                assert!(
                    (point.y - dot[1].as_f64().unwrap()).abs() < 1e-4,
                    "{} y",
                    case["name"]
                );
                assert!(
                    (actual.stamp.angle - dot[3].as_f64().unwrap()).abs() < 1e-4,
                    "{} angle",
                    case["name"]
                );
            }
        }
    }
}
