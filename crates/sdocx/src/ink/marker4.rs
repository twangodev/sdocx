//! Saved Marker4 V8: midpoint sampling and a rounded rectangular tip.
//! Traced from GLV8/RTV4 in Samsung Notes 4.4.45.37. The vector tip
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
    if r.pen_name.as_deref() != Some("com.samsung.android.sdk.pen.pen.preload.Marker4")
        || r.advanced_settings.as_deref() != Some("8;")
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
