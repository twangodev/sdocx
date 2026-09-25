//! Saved FountainPen V14/V16 geometry, traced from Samsung Notes 4.4.45.37.
//! Arithmetic stays float32 where the native implementation uses float32.
use super::path::{P, Quad};
use crate::{Point, Stroke};

// FountainPen V16 getInverseScale, canonical canvas inverse scale = 1.
const REPEAT: f32 = 0.82;

struct Tolerance {
    large: f32,
    small: f32,
    last: P,
    previous: P,
    farthest: P,
    distance: f32,
}
impl Tolerance {
    fn new(p: P, t: f32) -> Self {
        Self {
            large: t,
            small: t / 5.,
            last: p,
            previous: p,
            farthest: p,
            distance: 0.,
        }
    }
    fn skip(&self, p: P) -> bool {
        let delta = p.sub(self.last);
        let d = delta.len();
        if d > self.large {
            return false;
        }
        if self.distance > 0. && d > self.small {
            let previous = self.last.sub(self.previous);
            if d * (previous.len() * 0.865) > delta.dot(previous) {
                return false;
            }
        }
        if self.distance > self.small {
            let delta = p.sub(self.farthest);
            if delta.len() > self.small {
                return delta.dot(self.farthest.sub(self.last)) >= 0.;
            }
        }
        true
    }
    fn drop(&mut self, p: P) {
        let d = self.last.sub(p).len();
        if d > self.distance {
            self.distance = d;
            self.farthest = p;
        }
    }
    fn accept(&mut self, p: P) {
        self.previous = self.last;
        self.last = p;
        self.distance = 0.;
    }
}
#[derive(Clone, Copy)]
struct Width {
    time: i64,
    raw: f32,
    value: f32,
}
#[derive(Default)]
struct History {
    widths: Vec<Width>,
    replay: bool,
}
impl History {
    fn width(&mut self, time: i64, raw: f32) -> f32 {
        if !self.replay {
            if self.widths.last().is_none_or(|w| w.time < time) {
                self.widths.push(Width {
                    time,
                    raw,
                    value: raw,
                });
            }
            return raw;
        }
        let Some(last) = self.widths.last() else {
            return raw;
        };
        if time > last.time {
            return raw;
        }
        let i = self.widths.partition_point(|w| w.time < time);
        if i == 0 || self.widths[i].time == time {
            return self.widths[i].value;
        }
        let a = self.widths[i - 1];
        let b = self.widths[i];
        a.value + (b.value - a.value) * (time - a.time) as f32 / (b.time - a.time) as f32
    }
    fn smooth(&mut self) {
        for i in 1..self.widths.len() {
            let end = (i + i.min(3)).min(self.widths.len());
            let mut sum = 0.;
            for w in &self.widths[i..end] {
                sum += w.raw;
            }
            let mut w = sum / (end - i) as f32;
            w = (w + w) * 0.5;
            if i > 9 {
                let previous = self.widths[i - 1].value;
                w = (w - previous).mul_add(0.15, previous);
            }
            self.widths[i].value = w;
        }
        self.replay = true;
    }
}

pub(super) struct Dots {
    pub points: Vec<Point>,
    pub radii: Vec<f64>,
    pub sample_ends: Vec<usize>,
    pub directions: Option<Vec<Point>>,
}
struct Pass<'a> {
    size: f32,
    previous: P,
    midpoint: P,
    width: f32,
    residual: f32,
    alternate: bool,
    first: bool,
    tolerance: Tolerance,
    ratios: &'a mut [f32; 3],
    ratio_count: &'a mut usize,
    history: &'a mut History,
    dots: Dots,
}
impl Pass<'_> {
    fn dot(&mut self, p: P, r: f32) {
        self.first = false;
        if !self.history.replay {
            return;
        }
        self.dots.points.push(Point {
            x: p.x as f64,
            y: p.y as f64,
        });
        self.dots.radii.push(r.max(0.1) as f64);
    }
    fn line(&mut self, p: P, pressure: f32, tilt: f32, time: i64) {
        if self.tolerance.skip(p) {
            self.tolerance.drop(p);
            return;
        }
        let delta = p.sub(self.previous);
        let distance = delta.len();
        if distance < 5. {
            self.alternate = !self.alternate;
            if !self.alternate {
                self.tolerance.drop(p);
                return;
            }
        } else {
            self.alternate = true;
        }
        if distance == 0. {
            self.tolerance.drop(p);
            return;
        }
        let mid = self.previous.mid(p);
        let q = Quad::new(self.midpoint, self.previous, mid);
        self.ratios[*self.ratio_count % 3] = delta.y / distance;
        if *self.ratio_count == 0 {
            self.ratios.fill(delta.y / distance);
        }
        *self.ratio_count += 1;
        let ratio = ((self.ratios[0] + self.ratios[1]) + self.ratios[2]) / 3.;
        let pressure = pressure.min(1.);
        let pressure_term = (pressure + pressure) * 0.5;
        let raw = self.size / 3. + (tilt.mul_add(0.5, pressure_term) * self.size) * 0.5;
        let target = (((ratio * raw) as f64).mul_add(0.8, raw as f64) * 0.5) as f32;
        let step = self.size / if ratio > 0. { 4. } else { 2. };
        let limited = if (self.width - target).abs() > step {
            if self.width > target {
                self.width - step
            } else {
                self.width + step
            }
        } else {
            target
        };
        let width = self.history.width(
            time,
            limited
                .max((pressure * 0.7) * self.size)
                .max(self.size / 3.),
        );
        if q.length == 0. || q.length < self.residual {
            self.tolerance.drop(p);
            return;
        }
        let count = ((q.length - self.residual) / REPEAT) as usize + 1;
        let increment = (width - self.width) / count as f32;
        let mut w = self.width;
        let mut d = self.residual;
        while d <= q.length {
            w += increment;
            self.dot(q.at(d), w * 0.5);
            d += REPEAT;
        }
        self.residual = d - q.length;
        self.previous = p;
        self.midpoint = mid;
        self.width = width;
        self.tolerance.accept(p);
    }
    fn end(&mut self, p: P, time: i64) {
        if self.first {
            let w = self.history.width(time, self.size * 0.5);
            self.dot(self.previous, w * 0.5);
        } else {
            let q = Quad::new(self.midpoint, self.previous, p);
            let mut d = self.residual;
            if q.length > 0. {
                while d <= q.length {
                    self.dot(q.at(d), self.width * 0.5);
                    d += REPEAT;
                }
            }
            self.dot(p, self.width * 0.5);
        }
    }
}

pub(super) fn prepare(s: &Stroke) -> Option<Dots> {
    let r = s.rendering.as_ref()?;
    // Legacy saved pressure quantization can produce small negative values.
    // V14 handles these through its native width and stamp-radius floors.
    let minimum_pressure = if r.advanced_settings.as_deref() == Some("14;") {
        -1.
    } else {
        0.
    };
    // Only the verified saved-stylus profile. Other versions/settings retain
    // their explicit approximation instead of silently borrowing this model.
    if r.pen_name.as_deref() != Some("com.samsung.android.sdk.pen.pen.preload.FountainPen")
        || !matches!(r.advanced_settings.as_deref(), Some("14;" | "18;0;100;"))
        || r.tool_type_raw != 2
        || r.properties.fixed_width
        || r.properties.eraser
        || r.properties.straighten
        || r.properties.rainbow_effect
        || r.style.color_argb.is_some_and(|argb| argb >> 24 != 255)
        || s.points.is_empty()
        || s.pressures.len() != s.points.len()
        || s.timestamps.len() != s.points.len()
        || s.timestamps
            .iter()
            .any(|&time| i32::try_from(time).is_err())
        || (!s.tilts.is_empty() && s.tilts.len() != s.points.len())
        || !s.pen_width.is_finite()
        || s.pen_width <= 0.
        || s.pen_width > 1024.
        || s.points
            .iter()
            .any(|p| !p.x.is_finite() || !p.y.is_finite() || p.x.abs() > 1e7 || p.y.abs() > 1e7)
        || s.pressures
            .iter()
            .any(|&v| v < minimum_pressure || !(v as f32).is_finite())
        || s.tilts.iter().any(|&v| !(v as f32).is_finite())
    {
        return None;
    }
    let tolerance = r.style.initial_tolerance?;
    if !(0. ..=1000.).contains(&tolerance) {
        return None;
    }
    // Bound expansion before allocating: native fixed-distance stamping can
    // otherwise turn a tiny hostile input into billions of prepared points.
    let distance: f64 = s
        .points
        .windows(2)
        .map(|p| (p[1].x - p[0].x).hypot(p[1].y - p[0].y))
        .sum();
    if distance > 400_000. || s.points.len() > 100_000 {
        return None;
    }
    if r.advanced_settings.as_deref() == Some("14;") {
        return Some(super::fountain_v14::prepare(s, tolerance));
    }
    let first = P::from(s.points[0]);
    let mut history = History::default();
    let mut ratios = [0.; 3];
    let mut ratio_count = 0;
    let mut output = None;
    for _ in 0..2 {
        let width = history.width(
            s.timestamps[0],
            (s.pen_width * 0.5) * (s.pressures[0] as f32).min(1.),
        );
        let mut pass = Pass {
            size: s.pen_width,
            previous: first,
            midpoint: first,
            width,
            residual: 0.,
            alternate: false,
            first: true,
            tolerance: Tolerance::new(first, tolerance),
            ratios: &mut ratios,
            ratio_count: &mut ratio_count,
            history: &mut history,
            dots: Dots {
                directions: None,
                points: vec![],
                radii: vec![],
                sample_ends: vec![0; s.points.len()],
            },
        };
        for i in 1..s.points.len().saturating_sub(1) {
            let tilt = s.tilts.get(i).copied().unwrap_or(0.) as f32;
            let degrees = ((tilt * 180.) as f64 / std::f64::consts::PI) as f32;
            let tilt = (degrees.min(75.) - 15.).max(0.) / 60. * 3.;
            pass.line(
                s.points[i].into(),
                s.pressures[i] as f32,
                tilt,
                s.timestamps[i],
            );
            pass.dots.sample_ends[i] = pass.dots.points.len();
        }
        let last = s.points.len() - 1;
        pass.end(s.points[last].into(), s.timestamps[last]);
        pass.dots.sample_ends[last] = pass.dots.points.len();
        output = Some(pass.dots);
        if !history.replay {
            history.smooth();
        }
    }
    output
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
        tolerance: f32,
        samples: Vec<(f64, f64, f64, f64, i64)>,
        dots: Vec<Vec<f64>>,
        sample_ends: Vec<usize>,
    }
    fn reference() -> Reference {
        serde_json::from_str(include_str!("../../../../conformance/fountain-v16.json")).unwrap()
    }
    #[test]
    fn generated_geometry_matches_native_saved_stroke_helpers() {
        for reference in [
            reference(),
            serde_json::from_str(include_str!("../../../../conformance/fountain-v14.json"))
                .unwrap(),
        ] {
            for case in reference.cases {
                let mut stroke = reference.stroke.clone();
                stroke.pen_width = case.size;
                stroke.rendering.as_mut().unwrap().style.initial_tolerance = Some(case.tolerance);
                for (x, y, pressure, tilt, time) in case.samples {
                    stroke.points.push(Point { x, y });
                    stroke.pressures.push(pressure);
                    stroke.tilts.push(tilt);
                    stroke.timestamps.push(time);
                }
                let actual = prepare(&stroke).unwrap();
                assert_eq!(
                    actual.sample_ends, case.sample_ends,
                    "{} mapping",
                    case.name
                );
                assert_eq!(actual.points.len(), case.dots.len(), "{} count", case.name);
                for (index, ((point, radius), expected)) in actual
                    .points
                    .iter()
                    .zip(actual.radii)
                    .zip(case.dots)
                    .enumerate()
                {
                    let mut values = vec![point.x, point.y, radius];
                    if let Some(directions) = &actual.directions {
                        values.extend([directions[index].x, directions[index].y]);
                    }
                    assert_eq!(values.len(), expected.len(), "{} attributes", case.name);
                    for (a, b) in values.into_iter().zip(expected) {
                        assert!(
                            (a - b).abs() < 0.0001,
                            "{} dot {index}: {a} vs {b}",
                            case.name
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn unsupported_settings_and_unbounded_inputs_use_the_fallback() {
        let mut s = reference().stroke;
        s.points = vec![Point { x: 0., y: 0. }, Point { x: 10., y: 10. }];
        s.pressures = vec![0.5; 2];
        s.timestamps = vec![0, 8];
        assert!(prepare(&s).is_some());
        s.rendering.as_mut().unwrap().advanced_settings = Some("17;0;100;".into());
        assert!(prepare(&s).is_none());
        s.rendering.as_mut().unwrap().advanced_settings = Some("18;0;100;".into());
        s.points[1].x = 1_000_000.;
        assert!(prepare(&s).is_none());
        s.points[1].x = f64::NAN;
        assert!(prepare(&s).is_none());
        s.points[1].x = 10.;
        s.timestamps.clear();
        assert!(prepare(&s).is_none());
    }
}
