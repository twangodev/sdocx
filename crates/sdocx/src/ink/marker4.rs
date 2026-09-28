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
    let v7 = r.advanced_settings.as_deref() == Some("7;");
    if v7
        && (r.tool_type_raw != 2
            || r.properties.straighten
            || r.properties.fixed_width
            || !(0.4..=800.).contains(&s.pen_width)
            || s.pressures.len() != s.points.len()
            || s.timestamps.len() != s.points.len())
    {
        return None;
    }
    if r.pen_name.as_deref() != Some("com.samsung.android.sdk.pen.pen.preload.Marker4")
        || !matches!(r.advanced_settings.as_deref(), Some("7;" | "8;"))
        || r.properties.eraser
        || r.properties.rainbow_effect
        || !matches!(r.tool_type_raw, 0 | 2)
        || s.points.is_empty()
        || s.points.len() > 100_000
        || !s.pen_width.is_finite()
        || s.pen_width <= 0.
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
        let diameter = f64::from((s.pen_width * 0.5).max(1.) + 0.5) * 2.;
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
    let size = f64::from(s.pen_width.clamp(0.4, 800.).trunc());
    // RTV4 quad: x = ±0.77*(size/2+0.5), y = 0.5 ± size/2.
    // Its 200px mask covers [1,199] with elliptical corner radii 50.
    let angle = angle.unwrap_or(0.);
    let (sin, cos) = angle.sin_cos();
    for p in &mut points {
        p.x -= sin * 0.5;
        p.y += cos * 0.5;
    }
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
}
