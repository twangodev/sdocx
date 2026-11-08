//! Saved Marker4 V7/V8: midpoint sampling and a rounded rectangular tip.
//! Traced from GLV7/RTV3 and GLV8/RTV4 in Samsung Notes 4.4.45.37. The vector tip
//! approximates the native 200px mask's filtering, not its exact GPU coverage.
use super::{
    RectStamp,
    path::{P, Quad},
};
use crate::{Point, Stroke};

pub(crate) struct Ink {
    pub points: Vec<Point>,
    pub sample_ends: Vec<usize>,
    pub stamp: RectStamp,
    pub opacity: f64,
}

/// Experimental host-controlled V7 finger geometry, not document dispatch.
pub(crate) fn prepare_v7_finger(s: &Stroke) -> Option<Ink> {
    let rendering = s.rendering.as_ref()?;
    if rendering.advanced_settings.as_deref() != Some("7;")
        || rendering.tool_type_raw != 0
        || rendering.properties.straighten
        || rendering.properties.fixed_width
        || s.points.is_empty()
        || s.points.len() > 100_000
        || s.pressures.len() != s.points.len()
        || s.timestamps.len() != s.points.len()
        || s.pressures.iter().any(|p| !p.is_finite())
        || (!s.tilts.is_empty() && s.tilts.len() != s.points.len())
        || (!s.orientations.is_empty() && s.orientations.len() != s.points.len())
        || s.tilts
            .iter()
            .chain(&s.orientations)
            .any(|v| !v.is_finite())
    {
        return None;
    }
    let size = rendering.style.pen_size?;
    if !size.is_finite() || size < 0. {
        return None;
    }
    let mut proxy = s.clone();
    proxy.rendering.as_mut()?.advanced_settings = Some("8;".into());
    let mut ink = prepare(&proxy)?;
    let radius = (size.clamp(0.4, 800.) * 0.5).max(1.);
    ink.stamp.width = f64::from(radius + 0.5) * 1.54;
    ink.stamp.height = f64::from(radius + 0.5) * 2.;
    Some(ink)
}

pub(crate) fn prepare_v7_finger_bounded(
    s: &Stroke,
    remaining: &std::cell::Cell<usize>,
) -> Option<Ink> {
    let rendering = s.rendering.as_ref()?;
    let argb = rendering.style.color_argb?;
    let color = s.color?;
    if color.r != (argb >> 16) as u8 || color.g != (argb >> 8) as u8 || color.b != argb as u8 {
        return None;
    }
    if !matches!(
        rendering.pen_name.as_deref(),
        Some(
            "com.samsung.android.sdk.pen.pen.preload.Marker4"
                | "com.samsung.android.sdk.pen.pen.preload.StraightHighlighter"
        )
    ) || !rendering
        .style
        .pen_size
        .is_some_and(|size| (4.0..=35.0).contains(&size))
        || rendering.properties.replay_only
        || rendering.properties.eraser
        || rendering.properties.alpha_lock
        || rendering.properties.fixed_opacity
        || rendering.properties.rainbow_effect
        || rendering.properties.straighten
        || rendering.properties.fixed_width
        || rendering.properties.reveal_mode
        || rendering.properties.generated
        || rendering.style.stroke_type_raw.is_some_and(|v| v != 0)
        || rendering.style.color_type_raw.is_some_and(|v| v != 0)
        || rendering.style.line_type_raw.is_some_and(|v| v != 0)
        || rendering.style.gradient_colors_argb.is_some()
        || s.points.len() > 1024
        || s.points
            .windows(2)
            .map(|p| (p[1].x - p[0].x).hypot(p[1].y - p[0].y))
            .sum::<f64>()
            > 4000.
    {
        return None;
    }
    let ink = prepare_v7_finger(s)?;
    let left = remaining.get().checked_sub(ink.points.len())?;
    remaining.set(left);
    Some(ink)
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
    fn v7_finger_alias_sources_match_native_drawing_and_bridge() {
        use serde_json::Value;
        use sha2::{Digest, Sha256};
        use std::collections::BTreeSet;

        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/marker4-v7-finger-paint-host.json"
        ))
        .unwrap();
        let originals = &fixture["alias_controls_original"];
        let drawing: Value =
            serde_json::from_str(originals["drawing_receipt_json"].as_str().unwrap()).unwrap();
        let bridge: Value =
            serde_json::from_str(originals["bridged_receipt_json"].as_str().unwrap()).unwrap();
        let source_float = |value: &Value| {
            f32::from_bits(
                u32::from_str_radix(value.as_str().unwrap(), 16)
                    .unwrap()
                    .swap_bytes(),
            )
        };
        let register_float = |value: &Value| {
            f32::from_bits(
                u32::from_str_radix(value.as_str().unwrap().trim_start_matches("0x"), 16).unwrap(),
            )
        };
        let cases = drawing["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 3);
        let mut sizes = BTreeSet::new();
        for case in cases {
            let size = case["size"].as_u64().unwrap();
            assert!(sizes.insert(size));
            let inputs = case["inputs"].as_array().unwrap();
            let routes = case["routes"].as_array().unwrap();
            let registers = case["registers"].as_array().unwrap();
            assert_eq!((inputs.len(), routes.len(), registers.len()), (2, 2, 33));
            let mut names = BTreeSet::new();
            for (input, route) in inputs.iter().zip(routes) {
                let name = input["pen_name"].as_str().unwrap();
                assert!(names.insert(name));
                assert_eq!(input["advanced_setting"], "7;");
                assert_eq!(input["tool_type"], 0);
                assert_eq!(input["requested_size"].as_f64().unwrap(), size as f64);
                assert_eq!(route["selected_drawable_version"], "v7");
                let mut stroke = reference().stroke;
                stroke.points = input["points_f32_bits"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|point| Point {
                        x: f64::from(source_float(&point[0])),
                        y: f64::from(source_float(&point[1])),
                    })
                    .collect();
                stroke.pressures = input["pressure_f32_bits"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| f64::from(source_float(value)))
                    .collect();
                stroke.timestamps = input["timestamps_i32"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| value.as_i64().unwrap())
                    .collect();
                let rendering = stroke.rendering.as_mut().unwrap();
                rendering.pen_name = Some(name.to_owned());
                rendering.advanced_settings = Some("7;".into());
                rendering.tool_type_raw = 0;
                rendering.style.pen_size = Some(size as f32);
                let ink = prepare_v7_finger(&stroke).expect("native finger control is admitted");
                assert_eq!(ink.points.len(), registers.len());
                for (point, record) in ink.points.iter().zip(registers) {
                    let values = &record["float_registers"];
                    let x = f64::from(register_float(&values[0]));
                    let y = f64::from(register_float(&values[1]));
                    let radius = f64::from(register_float(&values[2]));
                    let angle = f64::from(register_float(&values[3]));
                    assert!((point.x - x).abs() < 1e-4 && (point.y - y).abs() < 1e-4);
                    assert!((ink.stamp.width - 1.54 * (radius + 0.5)).abs() < 1e-4);
                    assert!((ink.stamp.height - 2.0 * (radius + 0.5)).abs() < 1e-4);
                    assert!((ink.stamp.angle - angle).abs() < 1e-4);
                }
            }
            assert_eq!(names.len(), 2);
        }
        assert_eq!(sizes, BTreeSet::from([4, 10, 35]));

        let inputs = cases.iter().find(|case| case["size"] == 10).unwrap()["capture_sha256"]
            .as_array()
            .unwrap();
        let frames = bridge["frames"].as_array().unwrap();
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0]["rgba_sha256"], frames[1]["rgba_sha256"]);
        assert_eq!(frames[0]["producer_sha256"], frames[1]["producer_sha256"]);
        for frame in frames {
            assert!(inputs.contains(&frame["source_json_sha256"]));
            assert_eq!(frame["source_dot_count"], 33);
            assert_eq!(frame["producer_bytes"], 3168);
            assert_eq!(frame["gl_error"], 0);
            let uploads = frame["pre_draw_native_vbo_uploads"].as_array().unwrap();
            assert_eq!(uploads.len(), 1);
            let hex = uploads[0]["dataHex"].as_str().unwrap().as_bytes();
            let (pairs, remainder) = hex.as_chunks::<2>();
            assert!(remainder.is_empty());
            let bytes: Vec<_> = pairs
                .iter()
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect();
            assert_eq!(bytes.len(), 3168);
            assert_eq!(
                format!("{:x}", Sha256::digest(&bytes)),
                frame["producer_sha256"]
            );
            assert_eq!(uploads[0]["dataSha256"], frame["producer_sha256"]);
        }
    }

    #[test]
    fn v7_finger_budget_is_atomic_and_bounded() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tools/marker4_v7/public-finger/wide-extra/geometry.json"
        ))
        .unwrap();
        let case = &fixture["cases"][0];
        let mut stroke = reference().stroke;
        let rendering = stroke.rendering.as_mut().unwrap();
        rendering.tool_type_raw = 0;
        rendering.style.pen_size = Some(4.01);
        for sample in case["samples"].as_array().unwrap() {
            stroke.points.push(Point {
                x: sample[0].as_f64().unwrap(),
                y: sample[1].as_f64().unwrap(),
            });
            stroke.pressures.push(sample[2].as_f64().unwrap());
            stroke.tilts.push(sample[3].as_f64().unwrap());
            stroke.timestamps.push(stroke.timestamps.len() as i64 * 8);
        }
        stroke.color = Some(crate::Color {
            r: 0x33,
            g: 0x96,
            b: 0xff,
        });
        let count = prepare_v7_finger(&stroke).unwrap().points.len();
        assert!(count > 0);
        let remaining = std::cell::Cell::new(count - 1);
        assert!(prepare_v7_finger_bounded(&stroke, &remaining).is_none());
        assert_eq!(remaining.get(), count - 1);
        remaining.set(count);
        assert!(prepare_v7_finger_bounded(&stroke, &remaining).is_some());
        assert_eq!(remaining.get(), 0);
        let mut alias = stroke.clone();
        alias.rendering.as_mut().unwrap().pen_name =
            Some("com.samsung.android.sdk.pen.pen.preload.StraightHighlighter".into());
        remaining.set(count);
        let alias_ink = prepare_v7_finger_bounded(&alias, &remaining).unwrap();
        let original_ink = prepare_v7_finger(&stroke).unwrap();
        assert_eq!(alias_ink.points, original_ink.points);
        assert_eq!(alias_ink.sample_ends, original_ink.sample_ends);
        assert_eq!(remaining.get(), 0);
        remaining.set(0);
        assert!(prepare_v7_finger_bounded(&stroke, &remaining).is_none());
        assert_eq!(remaining.get(), 0);
        let mut too_many = stroke.clone();
        too_many.points.resize(1025, Point { x: 1., y: 1. });
        remaining.set(usize::MAX);
        assert!(prepare_v7_finger_bounded(&too_many, &remaining).is_none());
        assert_eq!(remaining.get(), usize::MAX);
    }

    #[test]
    fn v7_finger_adapter_matches_native_saved_geometry() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../conformance/marker4-v7-finger-native.json"
        ))
        .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let mut stroke = reference().stroke;
            let rendering = stroke.rendering.as_mut().unwrap();
            rendering.tool_type_raw = 0;
            rendering.advanced_settings = Some("7;".into());
            rendering.style.pen_size = Some(case["size"].as_f64().unwrap() as f32);
            for sample in case["samples"].as_array().unwrap() {
                stroke.points.push(Point {
                    x: sample[0].as_f64().unwrap(),
                    y: sample[1].as_f64().unwrap(),
                });
                stroke.pressures.push(sample[2].as_f64().unwrap());
                stroke.tilts.push(sample[3].as_f64().unwrap());
                stroke.timestamps.push(stroke.timestamps.len() as i64 * 8);
            }
            let actual = prepare_v7_finger(&stroke).unwrap();
            let dots = case["dots"].as_array().unwrap();
            let name = case["name"].as_str().unwrap();
            let ends: Vec<_> = case["sample_ends"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as usize)
                .collect();
            assert_eq!(actual.sample_ends, ends, "{name}");
            assert_eq!(actual.points.len(), dots.len(), "{name}");
            let bounds = case["tip_bounds"].as_array().unwrap();
            assert!(
                (actual.stamp.width - (bounds[2].as_f64().unwrap() - bounds[0].as_f64().unwrap()))
                    .abs()
                    < 1e-4,
                "{name} width"
            );
            assert!(
                (actual.stamp.height - (bounds[3].as_f64().unwrap() - bounds[1].as_f64().unwrap()))
                    .abs()
                    < 1e-4,
                "{name} height"
            );
            for (point, dot) in actual.points.iter().zip(dots) {
                assert!(
                    (point.x - dot[0].as_f64().unwrap()).abs() < 1e-4,
                    "{name} x"
                );
                assert!(
                    (point.y - dot[1].as_f64().unwrap()).abs() < 1e-4,
                    "{name} y"
                );
                assert!(
                    (actual.stamp.angle - dot[3].as_f64().unwrap()).abs() < 1e-4,
                    "{name} angle"
                );
                let radius = dot[2].as_f64().unwrap();
                assert!(
                    (actual.stamp.width - 1.54 * (radius + 0.5)).abs() < 1e-4,
                    "{name} radius"
                );
            }
            assert!(prepare(&stroke).is_none(), "{name} stays on fallback");
        }
    }

    #[test]
    fn v7_finger_adapter_matches_wide_native_saved_geometry() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../conformance/marker4-v7-finger-wide-native.json"
        ))
        .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let mut stroke = reference().stroke;
            let rendering = stroke.rendering.as_mut().unwrap();
            rendering.tool_type_raw = 0;
            rendering.advanced_settings = Some("7;".into());
            rendering.style.pen_size = Some(case["size"].as_f64().unwrap() as f32);
            for sample in case["samples"].as_array().unwrap() {
                stroke.points.push(Point {
                    x: sample[0].as_f64().unwrap(),
                    y: sample[1].as_f64().unwrap(),
                });
                stroke.pressures.push(sample[2].as_f64().unwrap());
                stroke.tilts.push(sample[3].as_f64().unwrap());
                stroke.timestamps.push(stroke.timestamps.len() as i64 * 8);
            }
            let actual = prepare_v7_finger(&stroke).unwrap();
            let dots = case["dots"].as_array().unwrap();
            let name = case["name"].as_str().unwrap();
            let ends: Vec<_> = case["sample_ends"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as usize)
                .collect();
            assert_eq!(actual.sample_ends, ends, "{name}");
            assert_eq!(actual.points.len(), dots.len(), "{name}");
            let bounds = case["tip_bounds"].as_array().unwrap();
            assert!(
                (actual.stamp.width - (bounds[2].as_f64().unwrap() - bounds[0].as_f64().unwrap()))
                    .abs()
                    < 1e-4,
                "{name} width"
            );
            assert!(
                (actual.stamp.height - (bounds[3].as_f64().unwrap() - bounds[1].as_f64().unwrap()))
                    .abs()
                    < 1e-4,
                "{name} height"
            );
            for (point, dot) in actual.points.iter().zip(dots) {
                assert!(
                    (point.x - dot[0].as_f64().unwrap()).abs() < 1e-4,
                    "{name} x"
                );
                assert!(
                    (point.y - dot[1].as_f64().unwrap()).abs() < 1e-4,
                    "{name} y"
                );
                assert!(
                    (actual.stamp.angle - dot[3].as_f64().unwrap()).abs() < 1e-4,
                    "{name} angle"
                );
                let radius = dot[2].as_f64().unwrap();
                assert!(
                    (actual.stamp.width - 1.54 * (radius + 0.5)).abs() < 1e-4,
                    "{name} radius"
                );
            }
            assert!(prepare(&stroke).is_none(), "{name} stays on fallback");
        }
    }

    #[test]
    fn v7_finger_adapter_rejects_unsupported_inputs() {
        let mut valid = reference().stroke;
        valid.rendering.as_mut().unwrap().tool_type_raw = 0;
        valid.rendering.as_mut().unwrap().style.pen_size = Some(9.5);
        valid.points.push(Point { x: 20., y: 20. });
        valid.pressures.push(0.5);
        valid.timestamps.push(0);
        assert!(prepare_v7_finger(&valid).is_some());
        macro_rules! rejected {
            ($change:expr) => {{
                let mut stroke = valid.clone();
                ($change)(&mut stroke);
                assert!(
                    prepare_v7_finger(&stroke).is_none(),
                    "{}",
                    stringify!($change)
                );
            }};
        }
        rejected!(|s: &mut Stroke| s.rendering.as_mut().unwrap().tool_type_raw = 2);
        rejected!(
            |s: &mut Stroke| s.rendering.as_mut().unwrap().advanced_settings = Some("8;".into())
        );
        rejected!(|s: &mut Stroke| s.rendering.as_mut().unwrap().pen_name = Some("other".into()));
        rejected!(|s: &mut Stroke| s.rendering.as_mut().unwrap().properties.straighten = true);
        rejected!(|s: &mut Stroke| s.rendering.as_mut().unwrap().properties.fixed_width = true);
        rejected!(|s: &mut Stroke| s.rendering.as_mut().unwrap().properties.eraser = true);
        rejected!(|s: &mut Stroke| s.rendering.as_mut().unwrap().properties.rainbow_effect = true);
        rejected!(|s: &mut Stroke| s.pressures.clear());
        rejected!(|s: &mut Stroke| s.timestamps.clear());
        rejected!(|s: &mut Stroke| s.pressures[0] = f64::NAN);
        rejected!(|s: &mut Stroke| s.tilts.push(f64::NAN));
        rejected!(|s: &mut Stroke| s.orientations.extend([0., 0.]));
        rejected!(|s: &mut Stroke| s.rendering.as_mut().unwrap().style.pen_size = None);
        rejected!(|s: &mut Stroke| s.rendering.as_mut().unwrap().style.pen_size = Some(-1.));
        rejected!(|s: &mut Stroke| s.rendering.as_mut().unwrap().style.pen_size = Some(f32::NAN));
        rejected!(|s: &mut Stroke| s.points.clear());
        rejected!(|s: &mut Stroke| s.points[0].x = f64::INFINITY);
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
            "../../../../tools/marker4_v7/public-finger/wide-extra/geometry.json"
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
