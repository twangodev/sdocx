//! Saved InkPen V4 callback geometry. This is intentionally an internal
//! preparer: callers supply the saved curve flag and PenCommon screen
//! resolution explicitly, keeping serialized properties separate from host state.
use super::path::{P, Quad};
use crate::{BoundingBox, Point, Stroke};

const PEN: &str = "com.samsung.android.sdk.pen.pen.preload.InkPen";
const MAX_SAVED_SAMPLES: usize = 100_000;
const MAX_GENERATED_DOTS: usize = 1_000_000;

/// Saved curve admission and host resolution, not inferred from page dimensions.
pub(super) struct RuntimeContext {
    pub curve_enabled: bool,
    pub screen_width: u32,
    pub screen_height: u32,
}

/// Geometry emitted by the V4 RTV1 callback. `radii` are the buffer radii
/// (drawPoint radius + 0.5); `pre_add_radii` retain the distinct drawPoint
/// values used for native bounds. `terminal` is the endPen suffix and is not
/// folded into saved-sample attribution.
pub(super) struct InkPenInk {
    pub points: Vec<Point>,
    pub pre_add_radii: Vec<f32>,
    pub radii: Vec<f32>,
    pub sample_ends: Vec<usize>,
    pub terminal: std::ops::Range<usize>,
    pub bounds: BoundingBox,
}

struct Bounds(Option<[f32; 4]>);
impl Bounds {
    fn new() -> Self {
        Self(None)
    }
    fn dot(&mut self, p: P, r: f32) {
        let v = [p.x - r, p.y - r, p.x + r, p.y + r];
        if let Some(b) = &mut self.0 {
            b[0] = b[0].min(v[0]);
            b[1] = b[1].min(v[1]);
            b[2] = b[2].max(v[2]);
            b[3] = b[3].max(v[3]);
        } else {
            self.0 = Some(v);
        }
    }
    fn include(&mut self, p: P) {
        if let Some(b) = &mut self.0 {
            b[0] = b[0].min(p.x);
            b[1] = b[1].min(p.y);
            b[2] = b[2].max(p.x);
            b[3] = b[3].max(p.y);
        }
    }
    fn rect(&mut self, ps: &[P], inset: f32) {
        for p in ps {
            self.include(P {
                x: p.x - inset,
                y: p.y - inset,
            });
            self.include(P {
                x: p.x + inset,
                y: p.y + inset,
            });
        }
    }
    fn grow(&mut self, amount: f32) {
        if let Some(b) = &mut self.0 {
            b[0] -= amount;
            b[1] -= amount;
            b[2] += amount;
            b[3] += amount;
        }
    }
    fn finish(self) -> BoundingBox {
        let b = self.0.unwrap_or([0.; 4]);
        BoundingBox {
            x_min: b[0] as f64,
            y_min: b[1] as f64,
            x_max: b[2] as f64,
            y_max: b[3] as f64,
        }
    }
}

/// Base SmPath's measured cubic used by endPen. It keeps leaf chords and
/// integer parameter endpoints, then evaluates the original cubic at the
/// distance-interpolated parameter, matching Base's 15-bit representation.
struct Cubic {
    p: [P; 4],
    segs: Vec<(f32, u16)>,
    length: f32,
}
impl Cubic {
    fn new(p: [P; 4]) -> Self {
        let mut c = Self {
            p,
            segs: Vec::new(),
            length: 0.,
        };
        c.split(p, 0, 32767);
        c
    }
    fn split(&mut self, p: [P; 4], t0: u16, t1: u16) {
        let line = |a: P, b: P, t: f32| P {
            x: (b.x - a.x).mul_add(t, a.x),
            y: (b.y - a.y).mul_add(t, a.y),
        };
        let one = line(p[0], p[3], f32::from_bits(0x3eaa_aaab));
        let two = line(p[0], p[3], f32::from_bits(0x3f2a_aaab));
        let dev1 = (p[1].x - one.x).abs().max((p[1].y - one.y).abs());
        let dev2 = (p[2].x - two.x).abs().max((p[2].y - two.y).abs());
        if t1 - t0 >= 1024 && (dev1 > 0.5 || dev2 > 0.5) {
            let a = p[0].mid(p[1]);
            let b = p[1].mid(p[2]);
            let c = p[2].mid(p[3]);
            let d = a.mid(b);
            let e = b.mid(c);
            let f = d.mid(e);
            let t = (t0 + t1) / 2;
            self.split([p[0], a, d, f], t0, t);
            self.split([f, e, c, p[3]], t, t1);
        } else {
            let next = self.length + p[0].sub(p[3]).len();
            if next > self.length {
                self.length = next;
                self.segs.push((next, t1));
            }
        }
    }
    fn at(&self, d: f32) -> P {
        let i = self
            .segs
            .partition_point(|&(end, _)| end < d)
            .min(self.segs.len() - 1);
        let (start, t0) = if i == 0 { (0., 0) } else { self.segs[i - 1] };
        let (end, t1) = self.segs[i];
        let scale = f32::from_bits(0x3800_00fd);
        let t0 = t0 as f32 * scale;
        let t1 = t1 as f32 * scale;
        let t = t0 + (d - start) * (t1 - t0) / (end - start);
        let eval = |a: f32, b: f32, c: f32, e: f32| {
            if t == 0. {
                return a;
            }
            let x = (b - a).mul_add(t, a);
            let y = (c - b).mul_add(t, b);
            let z = (e - c).mul_add(t, c);
            let q = (y - x).mul_add(t, x);
            let r = (z - y).mul_add(t, y);
            (r - q).mul_add(t, q)
        };
        P {
            x: eval(self.p[0].x, self.p[1].x, self.p[2].x, self.p[3].x),
            y: eval(self.p[0].y, self.p[1].y, self.p[2].y, self.p[3].y),
        }
    }
}

struct Pass {
    size: f32,
    tool: u16,
    screen_min: u32,
    last_tested: P,
    start: P,
    control: P,
    previous_time: i64,
    movement: f32,
    width: f32,
    fixed_width_radius: Option<f32>,
    alternate: bool,
    max_generated_dots: usize,
    points: Vec<Point>,
    pre_add_radii: Vec<f32>,
    radii: Vec<f32>,
    bounds: Bounds,
}

impl Pass {
    fn v1_factor(distance: f32) -> f32 {
        if distance < 4.2 {
            1.
        } else if distance < 9.5 {
            (((distance as f64 - 4.2_f64) / -5.3_f64).mul_add(0.4_f64, 0.4_f64) + 0.6_f64) as f32
        } else if distance < 35. {
            (((distance as f64 - 9.5_f64) / -25.5_f64).mul_add(0.5_f64, 0.5_f64) + 0.1_f64) as f32
        } else {
            0.1
        }
    }

    fn v1_candidate(&self, pressure: f32, distance: f32) -> f32 {
        let half = (Self::v1_factor(distance) + pressure) * 0.5;
        let candidate = half.mul_add(self.size, self.width) * 0.5;
        let change = (self.width - candidate).abs();
        if change > 1. {
            if candidate < self.width {
                self.width - 1.
            } else {
                self.width + 1.
            }
        } else if self.width < 1. && self.width > candidate {
            self.width - 0.5
        } else {
            candidate
        }
    }

    fn redraw_v1(
        &mut self,
        p: P,
        raw_pressure: f32,
        threshold: f32,
        initialize: &mut bool,
    ) -> Option<()> {
        if (p.x - self.last_tested.x).abs() < 1.5 && (p.y - self.last_tested.y).abs() < 1.5 {
            return Some(());
        }
        let distance = p.sub(self.last_tested).len();
        if !distance.is_finite() {
            return None;
        }
        if distance < threshold {
            let old = self.alternate;
            self.alternate = !self.alternate;
            if old {
                self.last_tested = p;
                return Some(());
            }
        } else {
            self.alternate = true;
        }

        let pressure = if self.tool == 1 || self.tool == 3 {
            0.5
        } else if raw_pressure > 1. {
            1.
        } else {
            raw_pressure
        };
        if *initialize {
            self.control = p;
            self.width = self.size * pressure;
            self.last_tested = p;
            *initialize = false;
            return Some(());
        }

        let target = self.v1_candidate(pressure, distance).max(self.size / 10.);
        let control = self.control;
        let mid = control.mid(p);
        let quad = Quad::new(self.start, control, mid);
        if !quad.length.is_finite() || !target.is_finite() {
            return None;
        }
        let n = Self::repeat(
            self.width.min(target).trunc(),
            quad.length,
            self.max_generated_dots.saturating_sub(self.points.len()),
        )?;
        if quad.length > 0. {
            let mut d = 0.;
            let step = quad.length / n as f32;
            let dw = (target - self.width) / n as f32;
            let mut width = self.width;
            for _ in 0..n {
                if d > quad.length {
                    break;
                }
                if !self.point(quad.at(d), width * 0.5) {
                    return None;
                }
                d += step;
                width += dw;
            }
        }
        self.width = target;
        self.start = mid;
        self.control = p;
        self.last_tested = p;
        Some(())
    }

    fn terminal_v1(&mut self, end: P, raw_pressure: f32, initialize: bool) -> Option<usize> {
        if initialize {
            let begin = self.points.len();
            if !self.point(end, self.size * 0.25) {
                return None;
            }
            self.width = self.size * 0.5;
            return Some(self.points.len() - begin);
        }
        let pressure = if self.tool == 1 || self.tool == 3 {
            0.5
        } else if raw_pressure > 1. {
            1.
        } else {
            raw_pressure
        };
        let distance = end.sub(self.last_tested).len();
        if !distance.is_finite() {
            return None;
        }
        let target = self.v1_candidate(pressure, distance).max(self.size / 10.);
        let control = self.control;
        let midpoint = control.mid(end);
        self.bounds.rect(&[self.start, control, midpoint, end], 1.);
        let begin = self.points.len();
        if self.alternate {
            let path = Cubic::new([self.start, control, midpoint, end]);
            path_emission(self, path.length, target, |d| path.at(d))?;
        } else {
            let path = Quad::new(self.start, control, end);
            path_emission(self, path.length, target, |d| path.at(d))?;
        }
        Some(self.points.len() - begin)
    }

    fn redraw_v1_no_curve(&mut self, p: P, raw_pressure: f32, initialize: &mut bool) -> Option<()> {
        let pressure = if self.tool == 1 {
            0.5
        } else if raw_pressure > 1. {
            1.
        } else {
            raw_pressure
        };
        let target = self.size * pressure;
        if *initialize {
            self.width = target;
            *initialize = false;
        }
        self.redraw_no_curve(p, target)
    }

    fn terminal_v1_no_curve(
        &mut self,
        end: P,
        raw_pressure: f32,
        initialize: bool,
    ) -> Option<usize> {
        let pressure = if self.tool == 1 {
            0.5
        } else if raw_pressure > 1. {
            1.
        } else {
            raw_pressure
        };
        let begin = self.points.len();
        if initialize {
            if !self.point(end, self.size * pressure * 0.5) {
                return None;
            }
        } else {
            self.terminal_no_curve(end, pressure)?;
        }
        Some(self.points.len() - begin)
    }
}

impl Pass {
    fn point(&mut self, p: P, radius: f32) -> bool {
        let radius = self.fixed_width_radius.unwrap_or(radius);
        let r = if radius < 1. { 1. } else { radius };
        if self.points.len() >= self.max_generated_dots
            || !p.x.is_finite()
            || !p.y.is_finite()
            || !r.is_finite()
            || !(r + 0.5).is_finite()
        {
            return false;
        }
        self.bounds.dot(p, r);
        self.points.push(Point {
            x: p.x as f64,
            y: p.y as f64,
        });
        self.pre_add_radii.push(r);
        self.radii.push(r + 0.5);
        true
    }

    fn repeat(width: f32, length: f32, remaining_budget: usize) -> Option<usize> {
        let w = width as i32;
        let scaled = if w < 3 {
            length
        } else if w == 3 {
            length / 1.5
        } else if w <= 5 {
            length * 0.5
        } else if w <= 10 {
            length / 3.
        } else if w <= 15 {
            length * 0.25
        } else if w <= 20 {
            length / 5.
        } else if w <= 30 {
            length / 6.
        } else if w <= 40 {
            length / 7.
        } else if w <= 55 {
            length * 0.125
        } else if w <= 80 {
            length / 9.
        } else {
            length / 10.
        };
        let count = ((scaled.trunc().max(2.) + 1.) * 1.5).trunc();
        if !scaled.is_finite()
            || !count.is_finite()
            || count < 0.
            || count > remaining_budget as f32
        {
            return None;
        }
        Some((count as usize).max(1))
    }

    fn movement(&self, p: P, time: i64) -> (f32, f32) {
        let delta = p.sub(self.last_tested);
        let distance = delta.len();
        let dt = time.wrapping_sub(self.previous_time).wrapping_mul(1000);
        let ms = dt as f32;
        let denominator = if dt < 1 || self.previous_time == 0 {
            50_000.
        } else {
            ms
        };
        let scale = if self.screen_min == 0 {
            1.
        } else {
            1440. / self.screen_min as f32
        };
        let movement = ((distance * 10_000.) / denominator) * scale;
        (distance, movement)
    }

    fn factor(v: f32) -> f32 {
        if v < 4.2 {
            1.
        } else if v < 9.5 {
            let a = v + -4.2;
            let b = a / -5.3;
            b.mul_add(0.4, 0.4) + 0.6
        } else if v < 35. {
            let a = v + -9.5;
            let b = a / -25.5;
            b.mul_add(0.5, 0.5) + 0.1
        } else {
            0.1
        }
    }

    fn candidate(&self, pressure: f32, old_movement: f32, movement: f32) -> f32 {
        let avg = (old_movement + movement) * 0.5;
        let factor = Self::factor(avg);
        let half = (factor + pressure) * 0.5;
        let candidate = half.mul_add(self.size, self.width) * 0.5;
        let change = (self.width - candidate).abs();
        if change > 1. {
            if candidate < self.width {
                self.width - 1.
            } else {
                self.width + 1.
            }
        } else if self.width < 1. && self.width > candidate {
            self.width - 0.5
        } else {
            candidate
        }
    }

    fn redraw(&mut self, p: P, pressure: f32, time: i64, threshold: f32) -> Option<()> {
        if (p.x - self.last_tested.x).abs() < 1.5 && (p.y - self.last_tested.y).abs() < 1.5 {
            return Some(());
        }
        let (distance, movement) = self.movement(p, time);
        if !distance.is_finite() || !movement.is_finite() {
            return None;
        }
        let old_movement = self.movement;
        self.previous_time = time;
        self.movement = movement;
        if distance < threshold {
            let old = self.alternate;
            self.alternate = !self.alternate;
            if old {
                self.last_tested = p;
                return Some(());
            }
        } else {
            self.alternate = true;
        }

        let target = self
            .candidate(pressure, old_movement, movement)
            .max(self.size / 10.);
        let control = self.control;
        let mid = control.mid(p);
        let quad = Quad::new(self.start, control, mid);
        if !quad.length.is_finite() || !target.is_finite() {
            return None;
        }
        let n = Self::repeat(
            self.width.min(target).trunc(),
            quad.length,
            self.max_generated_dots.saturating_sub(self.points.len()),
        )?;
        // Native reDrawLine emits no callback points for a zero-length
        // quadratic. Its bookkeeping still advances below, but sampling an
        // empty SmPath segment list would not be valid.
        if quad.length > 0. {
            let mut d = 0.;
            let step = quad.length / n as f32;
            let dw = (target - self.width) / n as f32;
            let mut w = self.width;
            for _ in 0..n {
                if d > quad.length {
                    break;
                }
                if !self.point(quad.at(d), w * 0.5) {
                    return None;
                }
                d += step;
                w += dw;
            }
        }
        self.width = target;
        self.start = mid;
        self.control = p;
        self.last_tested = p;
        Some(())
    }

    fn redraw_no_curve(&mut self, p: P, target: f32) -> Option<()> {
        if (p.x - self.last_tested.x).abs() < 1.5 && (p.y - self.last_tested.y).abs() < 1.5 {
            return Some(());
        }
        if !target.is_finite() {
            return None;
        }
        let delta = p.sub(self.last_tested);
        let length = delta.len();
        if !length.is_finite() {
            return None;
        }
        let n = Self::repeat(
            self.width.min(target).trunc(),
            length,
            self.max_generated_dots.saturating_sub(self.points.len()),
        )?;
        if length > 0. {
            let step = length / n as f32;
            let dw = (target - self.width) / n as f32;
            let mut d = 0.;
            let mut width = self.width;
            for _ in 0..n {
                if d > length {
                    break;
                }
                // SmPath stores the line endpoint's 15-bit parameter and
                // interpolates it with Base's packed-parameter scale.
                let t1 = 32767. * f32::from_bits(0x3800_00fd);
                let t = (d * t1) / length;
                if !self.point(self.last_tested.lerp(p, t), width * 0.5) {
                    return None;
                }
                d += step;
                width += dw;
            }
        }
        self.width = target;
        self.last_tested = p;
        Some(())
    }

    fn terminal_no_curve(&mut self, end: P, pressure: f32) -> Option<usize> {
        let target = pressure * self.size;
        let begin = self.points.len();
        let start = self.last_tested;
        self.bounds.rect(&[start, end], 1.);
        self.redraw_no_curve(end, target)?;
        Some(self.points.len() - begin)
    }

    fn terminal(&mut self, end: P, pressure: f32, event_time: i64) -> Option<usize> {
        let raw = pressure;
        let pressure = if self.tool == 1 || self.tool == 3 {
            0.5
        } else if raw > 1. {
            1.
        } else {
            raw
        };
        let (_, movement) = self.movement(end, event_time);
        if !movement.is_finite() {
            return None;
        }
        let target = self
            .candidate(pressure, self.movement, movement)
            .max(self.size / 10.);
        let rect_control = self.control;
        let midpoint = rect_control.mid(end);
        self.bounds
            .rect(&[self.start, rect_control, midpoint, end], 1.);
        let start = self.start;
        let curve = if self.alternate {
            let path = Cubic::new([start, rect_control, midpoint, end]);
            path_emission(self, path.length, target, |d| path.at(d))?
        } else {
            let path = Quad::new(start, rect_control, end);
            path_emission(self, path.length, target, |d| path.at(d))?
        };
        Some(curve)
    }
}

fn path_emission(
    pass: &mut Pass,
    length: f32,
    target: f32,
    at: impl Fn(f32) -> P,
) -> Option<usize> {
    if length <= 0. {
        return Some(0);
    }
    if !length.is_finite() || !target.is_finite() {
        return None;
    }
    let n = Pass::repeat(
        pass.width.min(target).trunc(),
        length,
        pass.max_generated_dots.saturating_sub(pass.points.len()),
    )?;
    let begin = pass.points.len();
    let mut d = 0.;
    let step = length / n as f32;
    let dw = (target - pass.width) / n as f32;
    let mut w = pass.width;
    for _ in 0..n {
        if d > length {
            break;
        }
        if !pass.point(at(d), w * 0.5) {
            return None;
        }
        d += step;
        w += dw;
    }
    Some(pass.points.len() - begin)
}

/// Prepare one saved V4 stroke for explicit host context. This function does
/// not select the Drawing caller's StrokeType branch or infer runtime state.
pub(super) fn prepare(s: &Stroke, context: RuntimeContext) -> Option<InkPenInk> {
    prepare_bounded(s, context, MAX_GENERATED_DOTS)
}

pub(super) fn prepare_bounded(
    s: &Stroke,
    context: RuntimeContext,
    max_generated_dots: usize,
) -> Option<InkPenInk> {
    let r = s.rendering.as_ref()?;
    // Model's missing size is zero; the native Pen setter supplies the clamp.
    let requested_size = r.style.pen_size.unwrap_or(0.);
    let screen_min_dimension = context.screen_width.min(context.screen_height);
    if context.screen_width > i32::MAX as u32
        || context.screen_height > i32::MAX as u32
        || r.pen_name.as_deref() != Some(PEN)
        || !matches!(
            r.advanced_settings.as_deref(),
            Some("") | Some("4;") | Some("18;0;100;")
        )
        || r.tool_type_raw > 4
        || r.properties.eraser
        || (r.properties.fixed_width
            && r.style
                .fixed_width
                .is_none_or(|width| !width.is_finite() || width < 0.))
        || r.properties.straighten
        || r.properties.rainbow_effect
        || s.points.is_empty()
        || s.points.len() > MAX_SAVED_SAMPLES
        || s.points.len() != s.pressures.len()
        || s.points.len() != s.timestamps.len()
        || (!s.tilts.is_empty() && s.tilts.len() != s.points.len())
        || (!s.orientations.is_empty() && s.orientations.len() != s.points.len())
        || !requested_size.is_finite()
        || requested_size < 0.
        || s.points.iter().any(|p| {
            !p.x.is_finite()
                || !p.y.is_finite()
                || !(p.x as f32).is_finite()
                || !(p.y as f32).is_finite()
        })
        || s.pressures
            .iter()
            .any(|v| !v.is_finite() || !(*v as f32).is_finite())
        || s.timestamps.iter().any(|v| i32::try_from(*v).is_err())
    {
        return None;
    }
    let size = requested_size.clamp(0.4, 800.);
    let first: P = s.points[0].into();
    let raw_pressure = s.pressures[0] as f32;
    let pressure = if r.tool_type_raw == 1 || (context.curve_enabled && r.tool_type_raw == 3) {
        0.5
    } else {
        raw_pressure
    };
    let width = pressure * size;
    let mut pass = Pass {
        size,
        tool: r.tool_type_raw,
        screen_min: screen_min_dimension,
        last_tested: first,
        start: first,
        control: first,
        previous_time: s.timestamps[0],
        movement: 0.,
        width,
        fixed_width_radius: r
            .properties
            .fixed_width
            .then(|| r.style.fixed_width.unwrap() * 0.5),
        alternate: false,
        max_generated_dots: max_generated_dots.min(MAX_GENERATED_DOTS),
        points: Vec::new(),
        pre_add_radii: Vec::new(),
        radii: Vec::new(),
        bounds: Bounds::new(),
    };
    let mut sample_ends = vec![0; s.points.len()];
    let (terminal_start, terminal_end);
    if context.curve_enabled {
        if !pass.point(first, width * 0.5) {
            return None;
        }
        sample_ends[0] = 1;
        for ((point, (raw, timestamp)), sample_end) in s
            .points
            .iter()
            .zip(s.pressures.iter().zip(&s.timestamps))
            .zip(sample_ends.iter_mut())
            .skip(1)
        {
            let raw = *raw as f32;
            let pressure = if r.tool_type_raw == 1 || r.tool_type_raw == 3 {
                0.5
            } else if raw > 1. {
                1.
            } else {
                raw
            };
            pass.redraw(
                (*point).into(),
                pressure,
                *timestamp,
                if r.tool_type_raw == 1 { 50. } else { 5. },
            )?;
            *sample_end = pass.points.len();
        }
        terminal_start = pass.points.len();
        if s.points.len() > 1 {
            let last = s.points.len() - 1;
            let event_time = s.timestamps[last].wrapping_sub(s.timestamps[0]);
            pass.terminal(s.points[last].into(), s.pressures[last] as f32, event_time)?;
            // The saved caller unions endPen's rectangle and applies RectF::grow(1).
            pass.bounds.grow(1.);
        }
        terminal_end = pass.points.len();
    } else {
        pass.bounds.dot(first, 0.);
        for ((point, raw), sample_end) in s
            .points
            .iter()
            .zip(&s.pressures)
            .zip(sample_ends.iter_mut())
            .skip(1)
        {
            let raw = *raw as f32;
            let pressure = if r.tool_type_raw == 1 {
                0.5
            } else if raw > 1. {
                1.
            } else {
                raw
            };
            pass.redraw_no_curve((*point).into(), pressure * size)?;
            *sample_end = pass.points.len();
        }
        terminal_start = pass.points.len();
        if s.points.len() > 1 {
            let last = s.points.len() - 1;
            let raw = s.pressures[last] as f32;
            let pressure = if r.tool_type_raw == 1 {
                0.5
            } else if raw > 1. {
                1.
            } else {
                raw
            };
            pass.terminal_no_curve(s.points[last].into(), pressure)?;
            pass.bounds.grow(1.);
        } else {
            // endPen_NoCurve still unions a one-pixel-inset point rectangle;
            // RedrawPen then grows the resulting rectangle by one more pixel.
            pass.bounds.rect(&[first], 1.);
            pass.bounds.grow(1.);
        }
        terminal_end = pass.points.len();
    }
    Some(InkPenInk {
        points: pass.points,
        pre_add_radii: pass.pre_add_radii,
        radii: pass.radii,
        sample_ends,
        terminal: terminal_start..terminal_end,
        bounds: pass.bounds.finish(),
    })
}

/// Test-only reconstruction of the saved InkPen V1 drawable. This remains a
/// separate policy from V4 because V1 movement is distance/threshold based and
/// does not consume timestamps or display resolution.
pub(super) fn prepare_v1(s: &Stroke, context: RuntimeContext) -> Option<InkPenInk> {
    prepare_v1_bounded(s, context, MAX_GENERATED_DOTS)
}

pub(super) fn prepare_v1_bounded(
    s: &Stroke,
    context: RuntimeContext,
    max_generated_dots: usize,
) -> Option<InkPenInk> {
    let r = s.rendering.as_ref()?;
    let requested_size = r.style.pen_size.unwrap_or(0.);
    if r.pen_name.as_deref() != Some(PEN)
        || r.tool_type_raw > 4
        || r.properties.eraser
        || r.properties.fixed_width
        || r.properties.straighten
        || r.properties.rainbow_effect
        || s.points.len() < 2
        || s.points.len() > MAX_SAVED_SAMPLES
        || s.points.len() != s.pressures.len()
        || s.points.len() != s.timestamps.len()
        || (!s.tilts.is_empty() && s.tilts.len() != s.points.len())
        || (!s.orientations.is_empty() && s.orientations.len() != s.points.len())
        || !requested_size.is_finite()
        || requested_size < 0.
        || s.points.iter().any(|p| {
            !p.x.is_finite()
                || !p.y.is_finite()
                || !(p.x as f32).is_finite()
                || !(p.y as f32).is_finite()
        })
        || s.pressures
            .iter()
            .any(|v| !v.is_finite() || !(*v as f32).is_finite())
        || s.timestamps.iter().any(|v| i32::try_from(*v).is_err())
    {
        return None;
    }

    let size = requested_size.clamp(0.4, 800.);
    let first: P = s.points[0].into();
    let mut pass = Pass {
        size,
        tool: r.tool_type_raw,
        screen_min: context.screen_width.min(context.screen_height),
        last_tested: first,
        start: first,
        control: first,
        previous_time: s.timestamps[0],
        movement: 0.,
        width: 0.,
        fixed_width_radius: None,
        alternate: false,
        max_generated_dots: max_generated_dots.min(MAX_GENERATED_DOTS),
        points: Vec::new(),
        pre_add_radii: Vec::new(),
        radii: Vec::new(),
        bounds: Bounds::new(),
    };
    let mut sample_ends = vec![0];
    let mut initialize = true;
    let threshold = if r.tool_type_raw == 1 { 50. } else { 5. };

    for index in 1..s.points.len() {
        let p: P = s.points[index].into();
        let raw_pressure = s.pressures[index] as f32;
        if context.curve_enabled {
            pass.redraw_v1(p, raw_pressure, threshold, &mut initialize)?;
        } else {
            pass.redraw_v1_no_curve(p, raw_pressure, &mut initialize)?;
        }
        sample_ends.push(pass.points.len());
    }

    let terminal_start = pass.points.len();
    let final_point: P = (*s.points.last()?).into();
    let terminal_count = if context.curve_enabled {
        pass.terminal_v1(final_point, *s.pressures.last()? as f32, initialize)?
    } else {
        pass.terminal_v1_no_curve(final_point, *s.pressures.last()? as f32, initialize)?
    };
    let terminal_end = terminal_start + terminal_count;
    if terminal_end != pass.points.len() {
        return None;
    }
    pass.bounds.grow(1.);
    Some(InkPenInk {
        points: pass.points,
        pre_add_radii: pass.pre_add_radii,
        radii: pass.radii,
        sample_ends,
        terminal: terminal_start..terminal_end,
        bounds: pass.bounds.finish(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{StrokeProperties, StrokeRendering, StrokeStyle};
    use serde_json::Value;

    fn stroke(defaults: &Value, case: &Value) -> Stroke {
        let mut data = defaults.clone();
        for (key, value) in case["stroke"].as_object().unwrap() {
            data[key] = value.clone();
        }
        let pen = data["rendering"].as_object().unwrap();
        let tool = case["tool_type_raw"]
            .as_u64()
            .unwrap_or_else(|| pen["tool_type_raw"].as_u64().unwrap()) as u16;
        let props = &pen["properties"];
        Stroke {
            rendering: Some(StrokeRendering {
                metadata: None,
                pen_name: Some(pen["pen_name"].as_str().unwrap().to_string()),
                advanced_settings: Some(pen["advanced_settings"].as_str().unwrap().to_string()),
                tool_type_raw: tool,
                properties: StrokeProperties {
                    compressed: false,
                    replay_only: false,
                    stylus_channels: props["stylus_channels"].as_bool().unwrap_or(false),
                    eraser: false,
                    fixed_width: props["fixed_width"].as_bool().unwrap_or(false),
                    millisecond_timestamps: false,
                    top_layer_pen: false,
                    alpha_lock: false,
                    binary_added: false,
                    generated: false,
                    fixed_opacity: false,
                    rainbow_effect: false,
                    straighten: false,
                    reveal_mode: false,
                },
                style: StrokeStyle {
                    pen_size: Some(data["pen_width"].as_f64().unwrap() as f32),
                    fixed_width: pen
                        .get("style")
                        .and_then(|style| style.get("fixed_width"))
                        .and_then(Value::as_f64)
                        .map(|width| width as f32),
                    ..StrokeStyle::default()
                },
            }),
            bbox: BoundingBox::default(),
            points: data["points"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| Point {
                    x: p["x"].as_f64().unwrap(),
                    y: p["y"].as_f64().unwrap(),
                })
                .collect(),
            pressures: data["pressures"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap())
                .collect(),
            timestamps: data["timestamps"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_i64().unwrap())
                .collect(),
            tilts: data["tilts"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap())
                .collect(),
            orientations: data["orientations"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap())
                .collect(),
            color: None,
            pen_width: data["pen_width"].as_f64().unwrap() as f32,
        }
    }

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() <= 2e-5, "{a} != {b}");
    }

    #[test]
    fn matches_all_hash_pinned_v4_callback_fixtures_and_sample_boundaries() {
        let fixture: Value =
            serde_json::from_str(include_str!("../../../../conformance/inkpen-v4.json")).unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let input = stroke(&fixture["stroke"], case);
            let native = &case["native"];
            let resolution = case["screen_resolution"]
                .as_array()
                .or_else(|| native["screen_resolution"].as_array())
                .cloned()
                .unwrap_or_else(|| vec![Value::from(0), Value::from(0)]);
            let context = RuntimeContext {
                curve_enabled: case["curve_enabled"].as_bool().unwrap_or(true),
                screen_width: resolution[0].as_u64().unwrap() as u32,
                screen_height: resolution[1].as_u64().unwrap() as u32,
            };
            let ink =
                prepare(&input, context).unwrap_or_else(|| panic!("{} not admitted", case["name"]));
            let triples = native["callback_triples"].as_array().unwrap();
            assert_eq!(
                ink.points.len(),
                triples.len(),
                "{} point count",
                case["name"]
            );
            assert_eq!(
                ink.radii.len(),
                triples.len(),
                "{} radii count",
                case["name"]
            );
            for (i, triple) in triples.iter().enumerate() {
                close(ink.points[i].x, triple[0].as_f64().unwrap());
                close(ink.points[i].y, triple[1].as_f64().unwrap());
                close(ink.radii[i] as f64, triple[2].as_f64().unwrap());
                close(
                    ink.pre_add_radii[i] as f64 + 0.5,
                    triple[2].as_f64().unwrap(),
                );
            }
            let expected_ends: Vec<usize> = native["sample_ends"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as usize / 3)
                .collect();
            assert_eq!(
                ink.sample_ends, expected_ends,
                "{} sample mapping",
                case["name"]
            );
            assert_eq!(
                ink.terminal.start,
                native["terminal_start"].as_u64().unwrap() as usize / 3
            );
            assert_eq!(
                ink.terminal.end,
                native["terminal_end"].as_u64().unwrap() as usize / 3
            );
            for (got, expected) in [
                ink.bounds.x_min,
                ink.bounds.y_min,
                ink.bounds.x_max,
                ink.bounds.y_max,
            ]
            .iter()
            .zip(native["bounds"].as_array().unwrap())
            {
                assert!(
                    (*got - expected.as_f64().unwrap()).abs() <= 2e-5,
                    "{} bounds: {} != {}",
                    case["name"],
                    got,
                    expected
                );
            }
        }
    }

    #[test]
    fn matches_native_size_and_fixed_width_setter_contexts() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/inkpen-setters-v4.json"
        ))
        .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let input = stroke(&fixture["stroke"], case);
            let native = &case["native"];
            let resolution = case["screen_resolution"].as_array().unwrap();
            let context = RuntimeContext {
                curve_enabled: case["curve_enabled"].as_bool().unwrap(),
                screen_width: resolution[0].as_u64().unwrap() as u32,
                screen_height: resolution[1].as_u64().unwrap() as u32,
            };
            let ink =
                prepare(&input, context).unwrap_or_else(|| panic!("{} not admitted", case["name"]));
            let triples = native["callback_triples"].as_array().unwrap();
            assert_eq!(ink.points.len(), triples.len(), "{} count", case["name"]);
            for (i, triple) in triples.iter().enumerate() {
                close(ink.points[i].x, triple[0].as_f64().unwrap());
                close(ink.points[i].y, triple[1].as_f64().unwrap());
                close(ink.radii[i] as f64, triple[2].as_f64().unwrap());
            }
            let expected_ends: Vec<usize> = native["sample_ends"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as usize / 3)
                .collect();
            assert_eq!(ink.sample_ends, expected_ends, "{} samples", case["name"]);
            assert_eq!(
                ink.terminal.start,
                native["terminal_start"].as_u64().unwrap() as usize / 3,
                "{} terminal start",
                case["name"]
            );
            assert_eq!(
                ink.terminal.end,
                native["terminal_end"].as_u64().unwrap() as usize / 3,
                "{} terminal end",
                case["name"]
            );
            for (got, expected) in [
                ink.bounds.x_min,
                ink.bounds.y_min,
                ink.bounds.x_max,
                ink.bounds.y_max,
            ]
            .iter()
            .zip(native["bounds"].as_array().unwrap())
            {
                assert!(
                    (*got - expected.as_f64().unwrap()).abs() <= 2e-5,
                    "{} bounds: {} != {}",
                    case["name"],
                    got,
                    expected
                );
            }
            let requested = native["requested_pen_size"].as_f64().unwrap() as f32;
            let native_size = native["setting_data_pen_size"].as_f64().unwrap() as f32;
            assert_eq!(
                input.pen_width.to_bits(),
                requested.to_bits(),
                "{} requested size",
                case["name"]
            );
            assert_eq!(
                input.pen_width.clamp(0.4, 800.).to_bits(),
                native_size.to_bits(),
                "{} native SetSize",
                case["name"]
            );
        }
    }

    fn bits(s: &str) -> u32 {
        u32::from_str_radix(s.strip_prefix("0x").unwrap(), 16).unwrap()
    }

    fn native_f32(s: &str) -> f32 {
        let bytes: [u8; 4] =
            std::array::from_fn(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap());
        f32::from_le_bytes(bytes)
    }

    fn native_caller_stroke(input: &Value) -> Stroke {
        let pen_size = input
            .get("pen_size_f32_bits")
            .or_else(|| input.get("size_f32_bits"))
            .and_then(Value::as_str)
            .map(native_f32)
            .expect("native caller case must include a size word");
        let properties = StrokeProperties {
            compressed: input["curve_enabled"].as_bool().unwrap(),
            replay_only: false,
            stylus_channels: false,
            eraser: false,
            fixed_width: false,
            millisecond_timestamps: false,
            top_layer_pen: false,
            alpha_lock: false,
            binary_added: false,
            generated: false,
            fixed_opacity: false,
            rainbow_effect: false,
            straighten: false,
            reveal_mode: false,
        };
        let floats = |key: &str| {
            input[key]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| native_f32(v.as_str().unwrap()))
                .collect::<Vec<_>>()
        };
        let xy = input["points_f32_bits"].as_array().unwrap();
        Stroke {
            rendering: Some(StrokeRendering {
                metadata: None,
                pen_name: Some(input["pen_name"].as_str().unwrap().to_string()),
                advanced_settings: input["advanced_getter_utf8"].as_str().map(str::to_string),
                tool_type_raw: input["tool_type"].as_u64().unwrap() as u16,
                properties,
                style: StrokeStyle {
                    pen_size: Some(pen_size),
                    stroke_type_raw: Some(input["stroke_type"].as_u64().unwrap() as u16),
                    ..StrokeStyle::default()
                },
            }),
            bbox: BoundingBox::default(),
            points: xy
                .iter()
                .map(|p| Point {
                    x: native_f32(p[0].as_str().unwrap()) as f64,
                    y: native_f32(p[1].as_str().unwrap()) as f64,
                })
                .collect(),
            pressures: floats("pressure_f32_bits")
                .into_iter()
                .map(f64::from)
                .collect(),
            timestamps: input["timestamps_i32"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_i64().unwrap())
                .collect(),
            tilts: floats("tilt_f32_bits").into_iter().map(f64::from).collect(),
            orientations: floats("orientation_f32_bits")
                .into_iter()
                .map(f64::from)
                .collect(),
            color: None,
            pen_width: pen_size,
        }
    }

    // Experimental paired output for the retained joined native capture. This
    // does not enable the candidate in production or assert pixel parity.
    #[cfg(feature = "pdf")]
    #[test]
    #[ignore = "writes experimental SVG/PDF to SDOCX_INKPEN_V4_EXPORT_DIR"]
    fn export_v4_joined_candidate_and_generic_fallback() {
        use sha2::{Digest, Sha256};

        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tmp/conformance-cleanup/native-capture-a241793/conformance/inkpen-joined-paint-native.json");
        let bytes = std::fs::read(root).expect("retained joined paint capture is required");
        assert_eq!(
            format!("{:x}", Sha256::digest(&bytes)),
            "48907aed06bbd5aedd70801ff51b0f3cffe881d4ce91dc1e30f98e73fd973ff6"
        );
        let fixture: Value = serde_json::from_slice(&bytes).unwrap();
        let case = &fixture["cases"]["inkpen_v4_black_1920x1080_to_128x64"];
        let caller = &case["source_caller"];
        let mut input = native_caller_stroke(&caller["input"]);
        input.rendering.as_mut().unwrap().style.color_argb = Some(0xff00_0000);
        let resolution = &caller["screen_resolution"][0];
        let ink = prepare(
            &input,
            RuntimeContext {
                curve_enabled: caller["input"]["curve_enabled"].as_bool().unwrap(),
                screen_width: resolution["width"].as_u64().unwrap() as u32,
                screen_height: resolution["height"].as_u64().unwrap() as u32,
            },
        )
        .expect("captured V4 input");
        let stream = caller["streams"]["draw_object_stroke"]["values_f32_bits"]
            .as_array()
            .unwrap();
        assert_eq!(stream.len(), ink.points.len() * 3);
        for (i, point) in ink.points.iter().enumerate() {
            assert_eq!(
                (point.x as f32).to_bits(),
                native_f32(stream[3 * i].as_str().unwrap()).to_bits()
            );
            assert_eq!(
                (point.y as f32).to_bits(),
                native_f32(stream[3 * i + 1].as_str().unwrap()).to_bits()
            );
            assert_eq!(
                ink.radii[i].to_bits(),
                native_f32(stream[3 * i + 2].as_str().unwrap()).to_bits()
            );
        }
        assert_eq!(
            caller["streams"]["draw_object_stroke"]["sha256"],
            "7b08daf7b07bd579c26d04009dfa4f13858858fc35508c24c66fb21ce4ed84e6"
        );
        assert_eq!(
            case["target"]["viewport"],
            serde_json::json!([0, 0, 128, 64])
        );
        let candidate = crate::render::inkpen_v1_vector_candidate_svg(
            &ink.points,
            &ink.radii,
            &ink.sample_ends,
            ink.terminal.clone(),
            "#000000",
            1.0,
            [128, 64],
            false,
        )
        .expect("bounded V4 contour candidate");
        let page = crate::Page {
            uuid: "inkpen-v4-capture".into(),
            width: 128,
            height: 64,
            content_bbox: BoundingBox::default(),
            background_color: None,
            template: None,
            background: Default::default(),
            objects: vec![input.into()],
        };
        let document = crate::Document {
            pages: vec![page],
            metadata: Default::default(),
        };
        let fallback = crate::render_document_svg(&document, &crate::RenderOptions::default())
            .remove(0)
            .svg;
        let output = std::path::PathBuf::from(
            std::env::var_os("SDOCX_INKPEN_V4_EXPORT_DIR")
                .expect("set SDOCX_INKPEN_V4_EXPORT_DIR to a fresh directory"),
        );
        std::fs::create_dir(&output).expect("output directory must not already exist");
        let write_pair = |prefix: &str, candidate: String, fallback: String| {
            for (kind, raw_svg) in [("candidate", candidate), ("generic", fallback)] {
                let name = format!("{prefix}{kind}");
                std::fs::write(output.join(format!("{name}-raw.svg")), &raw_svg).unwrap();
                let start = raw_svg.find('>').unwrap() + 1;
                let end = raw_svg.rfind("</svg>").unwrap();
                let mut body = &raw_svg[start..end];
                if kind == "generic" {
                    let background =
                        "<rect fill=\"#fcfcfc\" height=\"64\" width=\"128\" x=\"0\" y=\"0\"/>";
                    body = body
                        .trim_start()
                        .strip_prefix(background)
                        .expect("page background");
                }
                // The retained native framebuffer is bottom-up. Both exports use
                // the same reflection and transparent page for a controlled comparison.
                let svg = format!(
                    "{}<g transform=\"translate(0 64) scale(1 -1)\">{body}</g></svg>",
                    &raw_svg[..start],
                );
                let page = crate::RenderedPage::new(0, 128, 64, svg.clone());
                let pdf = crate::render_svg_pages_pdf(&[page], &crate::PdfOptions::default())
                    .expect("experimental SVG must lower to PDF");
                std::fs::write(output.join(format!("{name}.svg")), svg).unwrap();
                std::fs::write(output.join(format!("{name}.pdf")), pdf).unwrap();
            }
        };
        write_pair("", candidate.svg, fallback);

        let color_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../tmp/vector-parity-plan/inkpen-saved-rgb/native-inputs/inkpen-color-native.json",
        );
        let color_bytes = std::fs::read(color_path).expect("retained color capture is required");
        assert_eq!(
            format!("{:x}", Sha256::digest(&color_bytes)),
            "7a5abc7462969a8f1d99763c87f6f5d7fc5e6dd021250b246cf0dc09336ee20e"
        );
        let colors: Value = serde_json::from_slice(&color_bytes).unwrap();
        for argb in [0xffa1_b2c3_u32, 0x8040_2010, 0x0001_0203] {
            let id = format!("inkpen_v4_argb_{argb:08x}_1920x1080_to_128x64");
            let color_case = &colors["cases"][&id];
            assert_eq!(
                color_case["requested_argb_u32"].as_u64(),
                Some(u64::from(argb))
            );
            assert_eq!(color_case["target"]["viewport"], case["target"]["viewport"]);
            let color_caller = &color_case["source_caller"];
            assert_eq!(
                color_caller["screen_resolution"],
                caller["screen_resolution"]
            );
            assert_eq!(
                color_caller["streams"]["draw_object_stroke"]["values_f32_bits"],
                caller["streams"]["draw_object_stroke"]["values_f32_bits"]
            );
            let mut color_input = native_caller_stroke(&color_caller["input"]);
            color_input.rendering.as_mut().unwrap().style.color_argb = Some(argb);
            let color_ink = prepare(
                &color_input,
                RuntimeContext {
                    curve_enabled: color_caller["input"]["curve_enabled"].as_bool().unwrap(),
                    screen_width: resolution["width"].as_u64().unwrap() as u32,
                    screen_height: resolution["height"].as_u64().unwrap() as u32,
                },
            )
            .expect("captured V4 color input");
            assert_eq!(color_ink.points.len(), ink.points.len());
            for (index, point) in color_ink.points.iter().enumerate() {
                assert_eq!(
                    (point.x as f32).to_bits(),
                    native_f32(stream[3 * index].as_str().unwrap()).to_bits()
                );
                assert_eq!(
                    (point.y as f32).to_bits(),
                    native_f32(stream[3 * index + 1].as_str().unwrap()).to_bits()
                );
                assert_eq!(
                    color_ink.radii[index].to_bits(),
                    native_f32(stream[3 * index + 2].as_str().unwrap()).to_bits()
                );
            }
            let rgb = format!("#{:06x}", argb & 0x00ff_ffff);
            let opacity = f64::from((argb >> 24) as u8) / 255.0;
            let color_candidate = crate::render::inkpen_v1_vector_candidate_svg(
                &color_ink.points,
                &color_ink.radii,
                &color_ink.sample_ends,
                color_ink.terminal.clone(),
                &rgb,
                opacity,
                [128, 64],
                false,
            )
            .expect("bounded V4 color contour candidate");
            let color_page = crate::Page {
                uuid: id,
                width: 128,
                height: 64,
                content_bbox: BoundingBox::default(),
                background_color: None,
                template: None,
                background: Default::default(),
                objects: vec![color_input.into()],
            };
            let color_document = crate::Document {
                pages: vec![color_page],
                metadata: Default::default(),
            };
            let color_fallback =
                crate::render_document_svg(&color_document, &crate::RenderOptions::default())
                    .remove(0)
                    .svg;
            write_pair(
                &format!("argb-{argb:08x}-"),
                color_candidate.svg,
                color_fallback,
            );
        }
    }

    #[test]
    fn v4_output_scale_improves_svg_against_native_frames() {
        use flate2::read::ZlibDecoder;
        use sha2::{Digest, Sha256};
        use std::io::Read;

        let receipt: Value = serde_json::from_str(include_str!(
            "../../../../conformance/inkpen-v4-scale-native.json"
        ))
        .unwrap();
        let frames = include_bytes!("../../../../conformance/inkpen-v4-scale-native.bin");
        let hash = |bytes: &[u8]| format!("{:x}", Sha256::digest(bytes));
        assert_eq!(hash(frames), receipt["frames_sha256"]);
        let source: Value = serde_json::from_str(include_str!(
            "../../../../conformance/inkpen-saved-caller-native.json"
        ))
        .unwrap();
        let case = &source["cases"]["straight_curve_tool1_empty"];
        let mut stroke = native_caller_stroke(&case["input"]);
        for (indices, argb) in [([1, 2], 0xff00_0000), ([7, 8], 0x8040_2010)] {
            stroke.rendering.as_mut().unwrap().style.color_argb = Some(argb);
            let page = crate::Page {
                uuid: "inkpen-v4-scale-control".into(),
                width: 128,
                height: 64,
                content_bbox: BoundingBox::default(),
                background_color: None,
                template: None,
                background: Default::default(),
                objects: vec![stroke.clone().into()],
            };
            let document = crate::Document {
                pages: vec![page],
                metadata: Default::default(),
            };
            for index in indices {
                let frame = &receipt["cases"][index];
                assert_eq!(
                    frame["stream_sha256"],
                    case["streams"]["draw_object_stroke"]["sha256"]
                );
                let scale = frame["scale"].as_u64().unwrap() as u32;
                let offset = frame["zlib_offset"].as_u64().unwrap() as usize;
                let length = frame["zlib_length"].as_u64().unwrap() as usize;
                let mut native = Vec::new();
                ZlibDecoder::new(&frames[offset..offset + length])
                    .read_to_end(&mut native)
                    .unwrap();
                assert_eq!(hash(&native), frame["rgba_sha256"]);
                let width = 128 * scale;
                let height = 64 * scale;
                assert_eq!(native.len(), (width * height * 4) as usize);
                let render =
                    |output_scale| {
                        let options = crate::RenderOptions {
                            inkpen_profile: Some(crate::InkPenRenderProfile {
                                screen_width: 1920,
                                screen_height: 1080,
                            }),
                            inkpen_v4_output_scale: output_scale,
                            ..crate::RenderOptions::default()
                        };
                        let svg = crate::render_document_svg(&document, &options)
                            .remove(0)
                            .svg;
                        let start = svg.find('>').unwrap() + 1;
                        let end = svg.rfind("</svg>").unwrap();
                        let body = svg[start..end].trim_start().strip_prefix(
                        "<rect fill=\"#fcfcfc\" height=\"64\" width=\"128\" x=\"0\" y=\"0\"/>",
                    ).expect("page background");
                        let svg = format!(
                            "{}<g transform=\"translate(0 64) scale(1 -1)\">{body}</g></svg>",
                            &svg[..start]
                        );
                        let tree =
                            resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default())
                                .unwrap();
                        let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height).unwrap();
                        resvg::render(
                            &tree,
                            resvg::tiny_skia::Transform::from_scale(scale as f32, scale as f32),
                            &mut pixmap.as_mut(),
                        );
                        native
                            .chunks_exact((width * 4) as usize)
                            .rev()
                            .flat_map(|row| row.as_chunks::<4>().0)
                            .zip(pixmap.data().as_chunks::<4>().0)
                            .map(|(a, b)| u64::from(a[3].abs_diff(b[3])))
                            .sum::<u64>()
                    };
                let default_error = render(None);
                let scaled_error = render(Some(scale));
                assert!(
                    scaled_error < default_error,
                    "ARGB {argb:08x} scale {scale}: {scaled_error} vs {default_error}"
                );
            }
        }
    }

    #[test]
    fn v4_vector_candidate_improves_on_generic_against_host_frames() {
        use flate2::read::ZlibDecoder;
        use sha2::{Digest, Sha256};
        use std::io::Read;

        let receipt: Value = serde_json::from_str(include_str!(
            "../../../../conformance/inkpen-v4-vector-host.json"
        ))
        .unwrap();
        let frames = include_bytes!("../../../../conformance/inkpen-v4-vector-host.bin");
        let hash = |bytes: &[u8]| format!("{:x}", Sha256::digest(bytes));
        assert_eq!(
            frames.len(),
            receipt["native_frames"]["length"].as_u64().unwrap() as usize
        );
        assert_eq!(
            hash(frames),
            receipt["native_frames"]["sha256"].as_str().unwrap()
        );
        let source: Value = serde_json::from_str(include_str!(
            "../../../../conformance/inkpen-saved-caller-native.json"
        ))
        .unwrap();
        let case = &source["cases"]["straight_curve_tool1_empty"];
        let stream = case["streams"]["draw_object_stroke"]["values_f32_bits"]
            .as_array()
            .unwrap();
        assert_eq!(
            case["streams"]["draw_object_stroke"]["sha256"],
            receipt["native_stream_sha256"]
        );
        let mut input = native_caller_stroke(&case["input"]);
        let ink = prepare(
            &input,
            RuntimeContext {
                curve_enabled: true,
                screen_width: 1920,
                screen_height: 1080,
            },
        )
        .unwrap();
        assert_eq!(stream.len(), ink.points.len() * 3);
        for (i, point) in ink.points.iter().enumerate() {
            assert_eq!(
                (point.x as f32).to_bits(),
                native_f32(stream[3 * i].as_str().unwrap()).to_bits()
            );
            assert_eq!(
                (point.y as f32).to_bits(),
                native_f32(stream[3 * i + 1].as_str().unwrap()).to_bits()
            );
            assert_eq!(
                ink.radii[i].to_bits(),
                native_f32(stream[3 * i + 2].as_str().unwrap()).to_bits()
            );
        }
        let render = |raw: &str, generic: bool| {
            let start = raw.find('>').unwrap() + 1;
            let end = raw.rfind("</svg>").unwrap();
            let mut body = &raw[start..end];
            if generic {
                body = body
                    .trim_start()
                    .strip_prefix(
                        "<rect fill=\"#fcfcfc\" height=\"64\" width=\"128\" x=\"0\" y=\"0\"/>",
                    )
                    .expect("generic page background");
            }
            let svg = format!(
                "{}<g transform=\"translate(0 64) scale(1 -1)\">{body}</g></svg>",
                &raw[..start]
            );
            let tree = resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default()).unwrap();
            let mut pixmap = resvg::tiny_skia::Pixmap::new(128, 64).unwrap();
            resvg::render(
                &tree,
                resvg::tiny_skia::Transform::identity(),
                &mut pixmap.as_mut(),
            );
            pixmap.data().to_vec()
        };
        for (index, argb) in [0xff00_0000_u32, 0xffa1_b2c3, 0x8040_2010, 0x0001_0203]
            .into_iter()
            .enumerate()
        {
            let entry = &receipt["cases"][index];
            let frame = &entry["native_frame_zlib"];
            let offset = frame["offset"].as_u64().unwrap() as usize;
            let length = frame["length"].as_u64().unwrap() as usize;
            let compressed = &frames[offset..offset + length];
            assert_eq!(hash(compressed), frame["zlib_sha256"].as_str().unwrap());
            let mut native = Vec::new();
            ZlibDecoder::new(compressed)
                .read_to_end(&mut native)
                .unwrap();
            assert_eq!(native.len(), frame["raw_length"].as_u64().unwrap() as usize);
            assert_eq!(native.len(), 128 * 64 * 4);
            assert_eq!(hash(&native), entry["native_rgba_sha256"].as_str().unwrap());
            input.rendering.as_mut().unwrap().style.color_argb = Some(argb);
            let page = crate::Page {
                uuid: "inkpen-v4-capture".into(),
                width: 128,
                height: 64,
                content_bbox: BoundingBox::default(),
                background_color: None,
                template: None,
                background: Default::default(),
                objects: vec![input.clone().into()],
            };
            let document = crate::Document {
                pages: vec![page],
                metadata: Default::default(),
            };
            let generic = crate::render_document_svg(&document, &crate::RenderOptions::default())
                .remove(0)
                .svg;
            let options = crate::RenderOptions {
                inkpen_profile: Some(crate::InkPenRenderProfile {
                    screen_width: 1920,
                    screen_height: 1080,
                }),
                ..crate::RenderOptions::default()
            };
            let candidate = crate::render_document_svg(&document, &options)
                .remove(0)
                .svg;
            #[cfg(feature = "pdf")]
            {
                let bytes =
                    crate::render_document_pdf(&document, &options, &crate::PdfOptions::default())
                        .expect("profiled V4 page must export to PDF");
                let pdf = lopdf::Document::load_mem(&bytes).unwrap();
                assert!(
                    !pdf.objects
                        .values()
                        .any(|object| object.as_stream().is_ok_and(|stream| stream
                            .dict
                            .get(b"Subtype")
                            .is_ok_and(|value| value
                                .as_name()
                                .is_ok_and(|name| name == b"Image"))))
                );
            }
            let candidate_pixels = render(&candidate, true);
            let generic_pixels = render(&generic, true);
            let metrics = |pixels: &[u8]| {
                let mut intersection = 0usize;
                let mut union = 0usize;
                let mut alpha_error = 0u64;
                let mut rgba_error = 0u64;
                for (a, b) in native
                    .as_chunks::<{ 128 * 4 }>()
                    .0
                    .iter()
                    .rev()
                    .flat_map(|row| row.as_chunks::<4>().0.iter())
                    .zip(pixels.as_chunks::<4>().0.iter())
                {
                    intersection += usize::from(a[3] != 0 && b[3] != 0);
                    union += usize::from(a[3] != 0 || b[3] != 0);
                    alpha_error += u64::from(a[3].abs_diff(b[3]));
                    rgba_error += a
                        .iter()
                        .zip(b)
                        .map(|(x, y)| u64::from(x.abs_diff(*y)))
                        .sum::<u64>();
                }
                (intersection, union, alpha_error, rgba_error)
            };
            let c = metrics(&candidate_pixels);
            let g = metrics(&generic_pixels);
            if argb >> 24 == 0 {
                assert!(native.iter().all(|&v| v == 0));
                assert!(candidate_pixels.iter().all(|&v| v == 0));
                assert!(generic_pixels.iter().all(|&v| v == 0));
            } else {
                assert!(c.1 > 0 && g.1 > 0);
                assert!(
                    c.0 * g.1 > g.0 * c.1,
                    "support IoU for {argb:08x}: {c:?} vs {g:?}"
                );
                assert!(
                    c.2 * (g.1 as u64) < g.2 * (c.1 as u64),
                    "alpha error for {argb:08x}: {c:?} vs {g:?}"
                );
                assert!(c.3 < g.3, "RGBA error for {argb:08x}: {c:?} vs {g:?}");
            }
        }
    }

    #[test]
    fn matches_drawing_selected_v4_empty_setting_full_callback_bits() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/inkpen-saved-caller-native.json"
        ))
        .unwrap();
        let case = &fixture["cases"]["straight_curve_tool1_empty"];
        assert_eq!(case["input"]["advanced_setting_state"], "empty");
        assert_eq!(case["route"]["selected_drawable_vtable_id"], "v4");

        let input = native_caller_stroke(&case["input"]);
        let resolution = &case["screen_resolution"][0];
        let ink = prepare(
            &input,
            RuntimeContext {
                curve_enabled: case["input"]["curve_enabled"].as_bool().unwrap(),
                screen_width: resolution["width"].as_u64().unwrap() as u32,
                screen_height: resolution["height"].as_u64().unwrap() as u32,
            },
        )
        .expect("the actual empty-setting V4 input must be admitted by the test sampler");
        let stream = case["streams"]["draw_object_stroke"]["values_f32_bits"]
            .as_array()
            .unwrap();
        assert_eq!(stream.len(), ink.points.len() * 3);
        for (i, point) in ink.points.iter().enumerate() {
            assert_eq!(
                (point.x as f32).to_bits(),
                native_f32(stream[i * 3].as_str().unwrap()).to_bits(),
                "callback {i} x"
            );
            assert_eq!(
                (point.y as f32).to_bits(),
                native_f32(stream[i * 3 + 1].as_str().unwrap()).to_bits(),
                "callback {i} y"
            );
            assert_eq!(
                ink.radii[i].to_bits(),
                native_f32(stream[i * 3 + 2].as_str().unwrap()).to_bits(),
                "callback {i} radius"
            );
        }
        assert_eq!(
            case["streams"]["draw_object_stroke"]["sha256"],
            "7b08daf7b07bd579c26d04009dfa4f13858858fc35508c24c66fb21ce4ed84e6"
        );
        assert_eq!(
            case["streams"]["saved_redraw"]["same_bytes_as_drawing_phase"],
            true
        );

        let null_case = &fixture["cases"]["straight_curve_tool1_null"];
        assert_eq!(null_case["route"]["selected_drawable_vtable_id"], "v1");
        assert_eq!(null_case["input"]["advanced_setting_state"], "null");
        let null_setting = native_caller_stroke(&null_case["input"]);
        assert!(
            prepare(
                &null_setting,
                RuntimeContext {
                    curve_enabled: null_case["input"]["curve_enabled"].as_bool().unwrap(),
                    screen_width: null_case["screen_resolution"][0]["width"].as_u64().unwrap()
                        as u32,
                    screen_height: null_case["screen_resolution"][0]["height"]
                        .as_u64()
                        .unwrap() as u32,
                }
            )
            .is_none()
        );
    }

    #[test]
    fn matches_committed_drawing_selected_v1_null_setting_full_callback_bits() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/inkpen-saved-caller-native.json"
        ))
        .unwrap();
        let case = &fixture["cases"]["straight_curve_tool1_null"];
        assert_eq!(case["route"]["selected_drawable_vtable_id"], "v1");
        assert_eq!(case["input"]["advanced_setting_state"], "null");

        let input = native_caller_stroke(&case["input"]);
        let resolution = &case["screen_resolution"][0];
        let ink = prepare_v1(
            &input,
            RuntimeContext {
                curve_enabled: case["input"]["curve_enabled"].as_bool().unwrap(),
                screen_width: resolution["width"].as_u64().unwrap() as u32,
                screen_height: resolution["height"].as_u64().unwrap() as u32,
            },
        )
        .expect("the committed null-setting Drawing input must be handled by the explicit V1 core");
        let drawing = &case["streams"]["draw_object_stroke"];
        let stream = drawing["values_f32_bits"].as_array().unwrap();
        assert_eq!(stream.len() % 3, 0);
        assert_eq!(ink.points.len(), stream.len() / 3);
        assert_eq!(drawing["callback_count"], ink.points.len());
        assert_eq!(
            case["streams"]["saved_redraw"]["same_bytes_as_drawing_phase"],
            true
        );
        assert_eq!(case["streams"]["saved_redraw"]["sha256"], drawing["sha256"]);

        for (i, point) in ink.points.iter().enumerate() {
            assert_eq!(
                (point.x as f32).to_bits(),
                native_f32(stream[i * 3].as_str().unwrap()).to_bits(),
                "V1 callback {i} x"
            );
            assert_eq!(
                (point.y as f32).to_bits(),
                native_f32(stream[i * 3 + 1].as_str().unwrap()).to_bits(),
                "V1 callback {i} y"
            );
            assert_eq!(
                ink.radii[i].to_bits(),
                native_f32(stream[i * 3 + 2].as_str().unwrap()).to_bits(),
                "V1 callback {i} radius"
            );
        }
    }

    #[test]
    fn v1_vector_candidate_is_one_alpha_path_with_stamp_prefix_boundaries() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/inkpen-saved-caller-native.json"
        ))
        .unwrap();
        let case = &fixture["cases"]["straight_curve_tool1_null"];
        let mut input = native_caller_stroke(&case["input"]);
        input.rendering.as_mut().unwrap().style.color_argb = Some(0x8033_6699);
        let resolution = &case["screen_resolution"][0];
        let ink = prepare_v1(
            &input,
            RuntimeContext {
                curve_enabled: case["input"]["curve_enabled"].as_bool().unwrap(),
                screen_width: resolution["width"].as_u64().unwrap() as u32,
                screen_height: resolution["height"].as_u64().unwrap() as u32,
            },
        )
        .expect("native V1 full callback case");
        assert_eq!(ink.sample_ends.last().copied(), Some(ink.terminal.start));
        assert_eq!(ink.terminal.end, ink.points.len());

        let page_prepared = crate::prepare_stroke_for_page(&input, false);
        assert_eq!(
            page_prepared.sample_ends.as_deref(),
            Some(&[0, 0, 0, 25][..])
        );
        assert_eq!(page_prepared.points.len(), 25);
        assert_eq!(page_prepared.opacity, 128.0 / 255.0);
        assert_eq!(page_prepared.color, "#336699");
        assert_eq!(page_prepared.profile, Some("InkPen"));
        let context_free = crate::prepare_stroke(&input, false);
        assert!(context_free.sample_ends.is_none());
        assert!(context_free.dot_radii.is_none());

        let candidate = crate::render::inkpen_v1_vector_candidate_svg(
            &ink.points,
            &ink.radii,
            &ink.sample_ends,
            ink.terminal.clone(),
            "#336699",
            0.5,
            [128, 64],
            true,
        )
        .expect("bounded V1 vector candidate");
        let svg = &candidate.svg;
        assert_eq!(
            candidate.sample_ends.last().copied(),
            Some(ink.terminal.end)
        );
        assert_eq!(candidate.sample_ends.len(), ink.sample_ends.len());
        assert_eq!(
            candidate.source_event_path_ends.len(),
            candidate.sample_ends.len()
        );
        let xml = roxmltree::Document::parse(svg).unwrap();
        let path = xml
            .descendants()
            .find(|node| node.has_tag_name("path"))
            .expect("single union path");
        let paths = xml
            .descendants()
            .filter(|node| node.has_tag_name("path"))
            .collect::<Vec<_>>();
        assert_eq!(paths.len(), 1);
        assert_eq!(path.attribute("fill"), Some("#336699"));
        assert_eq!(path.attribute("fill-opacity"), None);
        assert_eq!(
            path.attribute("data-replay-lengths")
                .unwrap()
                .split(',')
                .count(),
            ink.points.len()
        );
        let data = path.attribute("d").unwrap();
        assert_eq!(data.matches('Z').count(), ink.points.len());
        assert_eq!(data.matches('L').count(), ink.points.len() * 95);
        let lengths = path
            .attribute("data-replay-lengths")
            .unwrap()
            .split(',')
            .map(|value| value.parse::<usize>().unwrap())
            .collect::<Vec<_>>();
        assert!(lengths.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(lengths.last().copied(), Some(data.len()));
        for (event_index, (&end, &path_end)) in candidate
            .sample_ends
            .iter()
            .zip(&candidate.source_event_path_ends)
            .enumerate()
        {
            let stamp_end = if end == 0 { 0 } else { lengths[end - 1] };
            assert_eq!(
                path_end, stamp_end,
                "source event {event_index} path cutoff"
            );
        }

        let opacity_groups = xml
            .descendants()
            .filter(|node| node.has_tag_name("g") && node.attribute("opacity").is_some())
            .collect::<Vec<_>>();
        assert_eq!(opacity_groups.len(), 1);
        assert_eq!(opacity_groups[0].attribute("opacity"), Some("0.5"));

        // ReplaySvg.show(count) cuts at the path's per-stamp byte offsets.
        // It can reveal empty prefixes and then the terminal suffix without
        // splitting any generated polygon.
        let show = |count: usize| {
            if count >= lengths.len() {
                data.to_string()
            } else if count == 0 {
                String::new()
            } else {
                data[..lengths[count - 1]].to_string()
            }
        };
        assert_eq!(show(0), "");
        for count in [
            candidate.sample_ends[0],
            candidate.sample_ends[1],
            ink.terminal.start,
        ] {
            let prefix = show(count);
            assert_eq!(prefix.matches('Z').count(), count);
        }
        assert_eq!(
            show(ink.terminal.end).matches('Z').count(),
            ink.points.len()
        );
        assert_eq!(show(ink.terminal.end), data);

        #[cfg(feature = "pdf")]
        {
            let page = crate::RenderedPage::new(0, 128, 64, svg.clone());
            let pdf = crate::render_svg_pages_pdf(&[page], &crate::PdfOptions::default())
                .expect("the vector-only V1 candidate must lower to PDF");
            let document = lopdf::Document::load_mem(&pdf).unwrap();
            assert!(document.objects.values().all(|object| {
                !matches!(
                    object,
                    lopdf::Object::Stream(stream)
                        if stream.dict.get(b"Subtype").ok()
                            == Some(&lopdf::Object::Name(b"Image".to_vec()))
                )
            }));
        }
    }

    #[test]
    fn v1_vector_candidate_rejects_unbounded_output() {
        let points = vec![
            Point {
                x: 1.0e30,
                y: 1.0e30,
            };
            2048
        ];
        let radii = vec![1.0e30_f32; points.len()];
        assert!(
            crate::render::inkpen_v1_vector_candidate_svg(
                &points,
                &radii,
                &[0],
                0..points.len(),
                "#000000",
                1.0,
                [128, 64],
                false,
            )
            .is_none()
        );
    }

    #[test]
    fn v1_vector_candidate_preserves_empty_event_cutoffs_and_folds_terminal_into_last() {
        let points = [
            Point { x: 1.0, y: 1.0 },
            Point { x: 2.0, y: 2.0 },
            Point { x: 3.0, y: 3.0 },
        ];
        let candidate = crate::render::inkpen_v1_vector_candidate_svg(
            &points,
            &[2.0; 3],
            &[0, 0, 1],
            1..3,
            "#000000",
            1.0,
            [16, 16],
            true,
        )
        .expect("bounded event sequence");
        assert_eq!(candidate.sample_ends, [0, 0, 3]);
        assert_eq!(candidate.source_event_path_ends[0], 0);
        assert_eq!(candidate.source_event_path_ends[1], 0);
        let document = roxmltree::Document::parse(&candidate.svg).unwrap();
        let path = document
            .descendants()
            .find(|node| node.has_tag_name("path"))
            .unwrap();
        let lengths = path
            .attribute("data-replay-lengths")
            .unwrap()
            .split(',')
            .map(|value| value.parse::<usize>().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(lengths.len(), 3);
        assert_eq!(
            candidate.source_event_path_ends[2],
            *lengths.last().unwrap()
        );
    }

    #[test]
    fn matches_native_v1_source_event_boundary_matrix() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/inkpen-v1-boundaries-native.json"
        ))
        .unwrap();
        assert_eq!(
            fixture["schema"],
            "sdocx-inkpen-v1-native-event-boundaries-v1"
        );
        let cases = fixture["cases"].as_object().unwrap();
        assert_eq!(cases.len(), 17);
        let mut tools = std::collections::BTreeSet::new();
        let mut no_curve_cases = 0;
        for (name, case) in cases {
            let input = native_caller_stroke(&case["input"]);
            let curve_enabled = case["input"]["curve_enabled"].as_bool().unwrap();
            tools.insert(case["input"]["tool_type"].as_u64().unwrap());
            no_curve_cases += usize::from(!curve_enabled);
            let ink = prepare_v1(
                &input,
                RuntimeContext {
                    curve_enabled,
                    screen_width: case["profile"]["width"].as_u64().unwrap() as u32,
                    screen_height: case["profile"]["height"].as_u64().unwrap() as u32,
                },
            )
            .unwrap();
            let packed = ink
                .points
                .iter()
                .zip(&ink.radii)
                .flat_map(|(point, radius)| [point.x as f32, point.y as f32, *radius])
                .flat_map(f32::to_le_bytes)
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            assert_eq!(packed, case["reuse"]["target_stream_bytes"], "{name}");
            for phase in ["draw_object_stroke", "saved_redraw"] {
                let ranges = &case["reuse"][if curve_enabled {
                    "native_redraw_sample_ranges"
                } else {
                    "native_no_curve_line_ranges"
                }];
                let events = ranges
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|event| event["phase"] == phase)
                    .filter(|event| !curve_enabled || event["method"] == "reDrawLine")
                    .collect::<Vec<_>>();
                let mut ends = vec![0];
                for event in events {
                    let end = event["after_record_count"].as_u64().unwrap() as usize;
                    assert_eq!(event["after_stream_bytes"], &packed[..end * 24]);
                    ends.push(end);
                }
                assert_eq!(ink.sample_ends, ends, "{name} {phase} source prefixes");
                let terminal_method = if curve_enabled {
                    "endPen"
                } else {
                    "endPen_NoCurve"
                };
                let terminal = case["reuse"]["native_redraw_sample_ranges"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|event| event["phase"] == phase && event["method"] == terminal_method)
                    .unwrap();
                let range = terminal["before_record_count"].as_u64().unwrap() as usize
                    ..terminal["after_record_count"].as_u64().unwrap() as usize;
                assert_eq!(ink.terminal, range, "{name} {phase} terminal");
                assert_eq!(terminal["after_stream_bytes"], packed, "{name} {phase}");
            }
        }
        assert_eq!(tools.into_iter().collect::<Vec<_>>(), [0, 1, 2, 3, 4]);
        assert_eq!(no_curve_cases, 1);
    }

    fn decode_hex_bytes(text: &str) -> Vec<u8> {
        text.as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect()
    }

    #[test]
    fn matches_native_v1_no_curve_tools_3_and_4_full_streams_and_event_ranges() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/inkpen-v1-nocurve-tool34-native.json"
        ))
        .unwrap();
        assert_eq!(
            fixture["schema"],
            "sdocx-inkpen-v1-nocurve-tool34-native-fixture-v1"
        );
        assert_eq!(fixture["cases"].as_object().unwrap().len(), 10);
        let defaults = serde_json::json!({
            "pen_width": 4.0,
            "rendering": {
                "pen_name": "com.samsung.android.sdk.pen.pen.preload.InkPen",
                "advanced_settings": "null",
                "tool_type_raw": 0,
                "properties": {"stylus_channels": false, "fixed_width": false},
                "style": {}
            },
            "points": [], "pressures": [], "timestamps": [], "tilts": [], "orientations": []
        });

        for (name, case) in fixture["cases"].as_object().unwrap() {
            let native = &case["native"];
            assert_eq!(case["curve_enabled"], false, "{name} curve input");
            assert!(
                matches!(case["tool_type_raw"].as_u64(), Some(3 | 4)),
                "{name} tool"
            );
            assert_eq!(
                native["input"]["curve_enabled"], false,
                "{name} native curve getter"
            );
            assert_eq!(
                native["input"]["tool_type"], case["tool_type_raw"],
                "{name} native tool getter"
            );

            let input = stroke(&defaults, case);
            let resolution = case["screen_resolution"].as_array().unwrap();
            let ink = prepare_v1(
                &input,
                RuntimeContext {
                    curve_enabled: false,
                    screen_width: resolution[0].as_u64().unwrap() as u32,
                    screen_height: resolution[1].as_u64().unwrap() as u32,
                },
            )
            .unwrap_or_else(|| panic!("{name}: V1 model rejected source case"));

            let raw = decode_hex_bytes(native["reuse"]["target_stream_bytes"].as_str().unwrap());
            let mut modeled = Vec::with_capacity(ink.points.len() * 12);
            for (index, point) in ink.points.iter().enumerate() {
                modeled.extend_from_slice(&(point.x as f32).to_bits().to_le_bytes());
                modeled.extend_from_slice(&(point.y as f32).to_bits().to_le_bytes());
                modeled.extend_from_slice(&ink.radii[index].to_bits().to_le_bytes());
            }
            assert_eq!(modeled, raw, "{name}: complete native XYZ/radius stream");

            for phase in native["event_boundary_summary"].as_array().unwrap() {
                let ends: Vec<usize> = phase["sample_ends"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_u64().unwrap() as usize)
                    .collect();
                assert_eq!(
                    ink.sample_ends, ends,
                    "{name}: native per-segment sample ends"
                );
                assert_eq!(
                    ink.terminal.start,
                    phase["terminal"][0].as_u64().unwrap() as usize,
                    "{name}: terminal range start"
                );
                assert_eq!(
                    ink.terminal.end,
                    phase["terminal"][1].as_u64().unwrap() as usize,
                    "{name}: terminal range end"
                );
            }

            let segments = native["reuse"]["native_no_curve_line_ranges"]
                .as_array()
                .unwrap();
            for phase_name in ["draw_object_stroke", "saved_redraw"] {
                let phase_events: Vec<_> = segments
                    .iter()
                    .filter(|event| event["phase"] == phase_name)
                    .collect();
                assert_eq!(
                    phase_events.len() + 1,
                    input.points.len(),
                    "{name}/{phase_name} segment count"
                );
                for (index, event) in phase_events.iter().enumerate() {
                    let end = event["after_record_count"].as_u64().unwrap() as usize;
                    assert_eq!(
                        end,
                        ink.sample_ends[index + 1],
                        "{name}/{phase_name} event boundary {index}"
                    );
                    assert_eq!(
                        decode_hex_bytes(event["after_stream_bytes"].as_str().unwrap()),
                        raw[..end * 12],
                        "{name}/{phase_name} native prefix {index}"
                    );
                }
            }
        }
    }

    #[test]
    fn inkpen2_null_v1_is_not_the_inkpen_v1_geometry() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/inkpen2-null-v1-geometry-native.json"
        ))
        .unwrap();
        let mut input = native_caller_stroke(&fixture["input"]);
        // Compare the existing InkPen model on the same channels, without
        // admitting the InkPen2 name to that model.
        input.rendering.as_mut().unwrap().pen_name = Some(PEN.to_string());
        let ink = prepare_v1(
            &input,
            RuntimeContext {
                curve_enabled: true,
                screen_width: fixture["profile"]["width"].as_u64().unwrap() as u32,
                screen_height: fixture["profile"]["height"].as_u64().unwrap() as u32,
            },
        )
        .unwrap();
        let native = decode_hex_bytes(fixture["stream_hex"].as_str().unwrap());
        assert_eq!(native.len(), 25 * 12);
        let native_first_radius = f32::from_le_bytes(native[8..12].try_into().unwrap());
        assert_eq!(native_first_radius.to_bits(), 0x4005_5556);
        assert_eq!(ink.radii[0], 1.5);
        assert_ne!(ink.radii[0].to_bits(), native_first_radius.to_bits());
    }

    #[test]
    fn curved_tool3_and_tool4_use_native_vectors_and_argb() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/inkpen-v1-boundaries-native.json"
        ))
        .unwrap();
        for tool in [3_u64, 4] {
            let case = &fixture["cases"][format!("tool{tool}_five_1920x1080")];
            let mut input = native_caller_stroke(&case["input"]);
            input.rendering.as_mut().unwrap().style.color_argb = Some(0xff12_3456);
            let prepared = crate::ink::prepare_inkpen_v1_for_page(&input)
                .expect("curved tool has native source and paint evidence");
            let packed = prepared
                .points
                .iter()
                .zip(&prepared.radii)
                .flat_map(|(point, radius)| [point.x as f32, point.y as f32, *radius])
                .flat_map(f32::to_le_bytes)
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            assert_eq!(packed, case["reuse"]["target_stream_bytes"], "tool {tool}");
            assert_eq!(prepared.argb, 0xff12_3456);
            assert_eq!(
                crate::prepare_stroke_for_page(&input, false).support,
                crate::InkSupport::Reconstructed
            );
        }
    }

    #[test]
    fn canonical_no_curve_tools_3_and_4_match_native_vectors_and_keep_fallback_gates() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/inkpen-v1-nocurve-tool34-native.json"
        ))
        .unwrap();
        assert_eq!(
            fixture["schema"],
            "sdocx-inkpen-v1-nocurve-tool34-native-fixture-v1"
        );

        for tool in [3_u64, 4] {
            let name = format!("tool{tool}_five_nonsymmetric");
            let source = &fixture["cases"][&name]["native"]["input"];
            assert_eq!(source["curve_enabled"], false);
            assert_eq!(source["tool_type"], tool);
            let mut input = native_caller_stroke(source);
            let argb = 0xff33_6699;
            input.rendering.as_mut().unwrap().style.color_argb = Some(argb);

            let prepared = crate::ink::prepare_inkpen_v1_for_page(&input)
                .unwrap_or_else(|| panic!("{name}: canonical page preparer rejected native V1"));
            assert_eq!(prepared.argb, argb, "{name}: saved ARGB");
            assert_eq!(
                crate::prepare_stroke_for_page(&input, false).support,
                crate::InkSupport::Reconstructed,
                "{name}: canonical support"
            );

            let expected = decode_hex_bytes(
                fixture["cases"][&name]["native"]["reuse"]["target_stream_bytes"]
                    .as_str()
                    .unwrap(),
            );
            let mut actual = Vec::with_capacity(prepared.points.len() * 12);
            for (point, radius) in prepared.points.iter().zip(&prepared.radii) {
                actual.extend_from_slice(&(point.x as f32).to_bits().to_le_bytes());
                actual.extend_from_slice(&(point.y as f32).to_bits().to_le_bytes());
                actual.extend_from_slice(&radius.to_bits().to_le_bytes());
            }
            assert_eq!(
                actual, expected,
                "{name}: canonical native XYZ/radius words"
            );

            let phase = &fixture["cases"][&name]["native"]["event_boundary_summary"][0];
            let sample_ends: Vec<usize> = phase["sample_ends"]
                .as_array()
                .unwrap()
                .iter()
                .map(|n| n.as_u64().unwrap() as usize)
                .collect();
            assert_eq!(
                prepared.sample_ends, sample_ends,
                "{name}: source event ends"
            );
            assert_eq!(
                prepared.terminal.start,
                phase["terminal"][0].as_u64().unwrap() as usize,
                "{name}: terminal start"
            );
            assert_eq!(
                prepared.terminal.end,
                phase["terminal"][1].as_u64().unwrap() as usize,
                "{name}: terminal end"
            );
        }

        let source = &fixture["cases"]["tool3_five_nonsymmetric"]["native"]["input"];
        let mut unsupported_tool = native_caller_stroke(source);
        let rendering = unsupported_tool.rendering.as_mut().unwrap();
        rendering.tool_type_raw = 5;
        rendering.style.color_argb = Some(0xff33_6699);
        assert!(crate::ink::prepare_inkpen_v1_for_page(&unsupported_tool).is_none());
        assert_eq!(
            crate::prepare_stroke_for_page(&unsupported_tool, false).support,
            crate::InkSupport::Approximate,
            "tool 5 retains generic fallback"
        );

        let mut advanced = native_caller_stroke(source);
        let rendering = advanced.rendering.as_mut().unwrap();
        rendering.advanced_settings = Some("4;".to_string());
        rendering.style.color_argb = Some(0xff33_6699);
        assert!(crate::ink::prepare_inkpen_v1_for_page(&advanced).is_none());
        assert_eq!(
            crate::prepare_stroke_for_page(&advanced, false).support,
            crate::InkSupport::Approximate,
            "advanced V4 retains generic fallback"
        );
    }

    #[test]
    fn matches_native_v1_reuse_source_event_boundaries() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/inkpen-v1-reuse-native.json"
        ))
        .unwrap();
        assert_eq!(fixture["schema"], "sdocx-inkpen-v1-reuse-native-v1");
        let cases = fixture["cases"].as_object().unwrap();
        assert_eq!(cases.len(), 4);
        for (name, case) in cases {
            let reuse = &case["reuse"];
            let mut first_input = case["input"].clone();
            for (channel, values) in reuse["first_input"].as_object().unwrap() {
                first_input[channel] = values.clone();
            }
            for (input, phase, stream_key) in [
                (&first_input, "draw_object_stroke", "first_stream_bytes"),
                (
                    &case["input"],
                    reuse["target_draw_phase"].as_str().unwrap(),
                    "target_stream_bytes",
                ),
            ] {
                let source = native_caller_stroke(input);
                let ink = prepare_v1(
                    &source,
                    RuntimeContext {
                        curve_enabled: input["curve_enabled"].as_bool().unwrap(),
                        screen_width: case["profile"]["width"].as_u64().unwrap() as u32,
                        screen_height: case["profile"]["height"].as_u64().unwrap() as u32,
                    },
                )
                .unwrap();
                let events = reuse["native_redraw_sample_ranges"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|event| event["phase"] == phase)
                    .collect::<Vec<_>>();
                let mut native_ends = vec![0];
                for event in &events {
                    if event["method"] == "reDrawLine" {
                        assert_eq!(event["source_history_index"], native_ends.len());
                        native_ends.push(event["after_record_count"].as_u64().unwrap() as usize);
                    }
                }
                assert_eq!(ink.sample_ends, native_ends, "{name} {phase} history");
                let terminal = events.last().unwrap();
                assert_eq!(terminal["method"], "endPen");
                let native_terminal = terminal["before_record_count"].as_u64().unwrap() as usize
                    ..terminal["after_record_count"].as_u64().unwrap() as usize;
                assert_eq!(ink.terminal, native_terminal, "{name} {phase} terminal");
                let packed = ink
                    .points
                    .iter()
                    .zip(&ink.radii)
                    .flat_map(|(point, radius)| [point.x as f32, point.y as f32, *radius])
                    .flat_map(f32::to_le_bytes)
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>();
                assert_eq!(
                    packed,
                    reuse[stream_key].as_str().unwrap(),
                    "{name} {phase}"
                );
                for event in events {
                    let end = event["after_count_bytes"].as_u64().unwrap() as usize * 2;
                    assert_eq!(
                        event["after_stream_bytes"].as_str().unwrap(),
                        &packed[..end],
                        "{name} {phase} native vector prefix"
                    );
                }
            }
        }
    }

    #[test]
    fn matches_native_v1_saved_matrix_and_rejects_preselection_singlepoint() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/inkpen-v1-saved-native.json"
        ))
        .unwrap();
        assert_eq!(fixture["schema"], "sdocx-inkpen-v1-saved-native-v1");
        let cases = fixture["cases"].as_object().unwrap();
        assert_eq!(cases.len(), 15);
        let mut emitted_cases = 0;
        let mut rejected_cases = 0;

        for (name, case) in cases {
            let input = native_caller_stroke(&case["input"]);
            let width = case["profile"]["width"].as_u64().unwrap() as u32;
            let height = case["profile"]["height"].as_u64().unwrap() as u32;
            let context = RuntimeContext {
                curve_enabled: case["input"]["curve_enabled"].as_bool().unwrap(),
                screen_width: width,
                screen_height: height,
            };

            if case["route"]["stream_emitted"] == false {
                rejected_cases += 1;
                assert_eq!(case["route"]["factory_created_pens"], 0, "{name}");
                assert!(prepare_v1(&input, context).is_none(), "{name}");
                continue;
            }

            emitted_cases += 1;
            assert_eq!(case["route"]["selected_drawable_vtable_id"], "v1", "{name}");
            let ink = prepare_v1(&input, context)
                .unwrap_or_else(|| panic!("{name} V1 input must be admitted"));
            let drawing = &case["streams"]["draw_object_stroke"];
            let saved = &case["streams"]["saved_redraw"];
            let stream = drawing["values_f32_bits"].as_array().unwrap();
            assert_eq!(stream.len() % 3, 0, "{name} native triples");
            assert_eq!(ink.points.len(), stream.len() / 3, "{name} total count");
            assert_eq!(drawing["callback_count"], ink.points.len(), "{name}");
            assert_eq!(saved["same_bytes_as_drawing_phase"], true, "{name}");
            assert_eq!(saved["sha256"], drawing["sha256"], "{name}");

            for (i, point) in ink.points.iter().enumerate() {
                assert_eq!(
                    (point.x as f32).to_bits(),
                    native_f32(stream[i * 3].as_str().unwrap()).to_bits(),
                    "{name} callback {i} x"
                );
                assert_eq!(
                    (point.y as f32).to_bits(),
                    native_f32(stream[i * 3 + 1].as_str().unwrap()).to_bits(),
                    "{name} callback {i} y"
                );
                assert_eq!(
                    ink.radii[i].to_bits(),
                    native_f32(stream[i * 3 + 2].as_str().unwrap()).to_bits(),
                    "{name} callback {i} radius"
                );
            }
        }
        assert_eq!(emitted_cases, 14);
        assert_eq!(rejected_cases, 1);
    }

    #[test]
    fn matches_all_native_v4_display_profile_callback_bits() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/inkpen-display-profiles-native.json"
        ))
        .unwrap();
        assert_eq!(fixture["schema"], "sdocx-inkpen-display-profiles-native-v1");
        let cases = fixture["cases"].as_object().unwrap();
        assert_eq!(cases.len(), 7);
        for (name, case) in cases {
            assert_eq!(case["route"]["selected_drawable_vtable_id"], "v4", "{name}");
            let settings = case["pen_setting_data"]["screen_resolution_i32_offsets_32_36"]
                .as_array()
                .unwrap();
            let width = settings[0].as_i64().unwrap() as u32;
            let height = settings[1].as_i64().unwrap() as u32;
            assert_eq!(
                case["profile_case"]["system_display_width_px"], width,
                "{name} width"
            );
            assert_eq!(
                case["profile_case"]["system_display_height_px"], height,
                "{name} height"
            );
            let input = native_caller_stroke(&case["input"]);
            let ink = prepare(
                &input,
                RuntimeContext {
                    curve_enabled: case["input"]["curve_enabled"].as_bool().unwrap(),
                    screen_width: width,
                    screen_height: height,
                },
            )
            .unwrap_or_else(|| panic!("{name} V4 caller input must be admitted"));
            let mut page_input = input.clone();
            page_input.rendering.as_mut().unwrap().style.color_argb = Some(0xff00_0000);
            let page_ink = crate::ink::prepare_inkpen_v4_for_page(&page_input, width, height);
            if width == 0 || height == 0 {
                assert!(page_ink.is_none(), "{name}: zero display is unsupported");
            } else {
                let page_ink = page_ink.expect("captured nonzero display profile");
                assert_eq!(page_ink.points, ink.points, "{name}: page points");
                assert_eq!(page_ink.radii, ink.radii, "{name}: page radii");
                assert_eq!(page_ink.sample_ends, ink.sample_ends, "{name}: page events");
                assert_eq!(page_ink.terminal, ink.terminal, "{name}: page terminal");
            }
            let drawing = &case["streams"]["draw_object_stroke"];
            let saved = &case["streams"]["saved_redraw"];
            let stream = drawing["values_f32_bits"].as_array().unwrap();
            assert_eq!(stream.len() % 3, 0, "{name} native triples");
            assert_eq!(ink.points.len(), stream.len() / 3, "{name} total count");
            assert_eq!(drawing["callback_count"], ink.points.len(), "{name}");
            assert_eq!(saved["same_bytes_as_drawing_phase"], true, "{name}");
            assert_eq!(saved["sha256"], drawing["sha256"], "{name}");

            for (i, point) in ink.points.iter().enumerate() {
                assert_eq!(
                    (point.x as f32).to_bits(),
                    native_f32(stream[i * 3].as_str().unwrap()).to_bits(),
                    "{name} callback {i} x"
                );
                assert_eq!(
                    (point.y as f32).to_bits(),
                    native_f32(stream[i * 3 + 1].as_str().unwrap()).to_bits(),
                    "{name} callback {i} y"
                );
                assert_eq!(
                    ink.radii[i].to_bits(),
                    native_f32(stream[i * 3 + 2].as_str().unwrap()).to_bits(),
                    "{name} callback {i} radius"
                );
            }
        }
    }

    #[test]
    fn matches_all_native_size_tool_admission_callback_bits() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/inkpen-admission-native.json"
        ))
        .unwrap();
        assert_eq!(fixture["schema"], "sdocx-inkpen-v4-size-tool-admission-v1");
        let cases = fixture["cases"].as_object().unwrap();
        assert_eq!(cases.len(), 9);

        for (name, case) in cases {
            assert_eq!(case["route"]["selected_drawable_vtable_id"], "v4", "{name}");
            let resolution = &case["screen_resolution"][0];
            let mut input = native_caller_stroke(&case["input"]);
            if name == "size_omitted" {
                input.rendering.as_mut().unwrap().style.pen_size = None;
                input.pen_width = 0.8;
            } else if name == "size_ten" {
                input.pen_width = 99.;
            }
            let ink = prepare(
                &input,
                RuntimeContext {
                    curve_enabled: case["input"]["curve_enabled"].as_bool().unwrap(),
                    screen_width: resolution["width"].as_u64().unwrap() as u32,
                    screen_height: resolution["height"].as_u64().unwrap() as u32,
                },
            )
            .unwrap_or_else(|| panic!("{name} V4 caller input must be admitted"));
            let drawing = &case["streams"]["draw_object_stroke"];
            let saved = &case["streams"]["saved_redraw"];
            let stream = drawing["values_f32_bits"].as_array().unwrap();
            assert_eq!(stream.len() % 3, 0, "{name} native triples");
            assert_eq!(ink.points.len(), stream.len() / 3, "{name} total count");
            assert_eq!(drawing["callback_count"], ink.points.len(), "{name}");
            assert_eq!(saved["same_bytes_as_drawing_phase"], true, "{name}");
            assert_eq!(saved["sha256"], drawing["sha256"], "{name}");

            for (i, point) in ink.points.iter().enumerate() {
                assert_eq!(
                    (point.x as f32).to_bits(),
                    native_f32(stream[i * 3].as_str().unwrap()).to_bits(),
                    "{name} callback {i} x"
                );
                assert_eq!(
                    (point.y as f32).to_bits(),
                    native_f32(stream[i * 3 + 1].as_str().unwrap()).to_bits(),
                    "{name} callback {i} y"
                );
                assert_eq!(
                    ink.radii[i].to_bits(),
                    native_f32(stream[i * 3 + 2].as_str().unwrap()).to_bits(),
                    "{name} callback {i} radius"
                );
            }
        }
    }

    #[test]
    fn cubic_sampler_matches_native_base_length_and_positions_bit_exactly() {
        let fixture: Value =
            serde_json::from_str(include_str!("../../../../conformance/smpath-cubic.json"))
                .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let control = case["native"]["controls_f32"].as_array().unwrap();
            let p = [
                P {
                    x: control[0].as_f64().unwrap() as f32,
                    y: control[1].as_f64().unwrap() as f32,
                },
                P {
                    x: control[2].as_f64().unwrap() as f32,
                    y: control[3].as_f64().unwrap() as f32,
                },
                P {
                    x: control[4].as_f64().unwrap() as f32,
                    y: control[5].as_f64().unwrap() as f32,
                },
                P {
                    x: control[6].as_f64().unwrap() as f32,
                    y: control[7].as_f64().unwrap() as f32,
                },
            ];
            let cubic = Cubic::new(p);
            assert_eq!(
                cubic.length.to_bits(),
                bits(case["native"]["length_bits"].as_str().unwrap()),
                "{} length",
                case["name"]
            );
            for sample in case["native"]["samples"].as_array().unwrap() {
                let distance = f32::from_bits(bits(sample["distance_bits"].as_str().unwrap()));
                let position = cubic.at(distance);
                let expected = sample["position_bits"].as_array().unwrap();
                assert_eq!(
                    position.x.to_bits(),
                    bits(expected[0].as_str().unwrap()),
                    "{} position x at {}",
                    case["name"],
                    sample["fraction"]
                );
                assert_eq!(
                    position.y.to_bits(),
                    bits(expected[1].as_str().unwrap()),
                    "{} position y at {}",
                    case["name"],
                    sample["fraction"]
                );
            }
        }
    }
    #[cfg(feature = "pdf")]
    #[test]
    fn fixed_width_v4_matches_native_drawing_and_improves_svg() {
        use base64::Engine;
        use std::io::Read;
        let mut cases: Vec<(f32, Value)> = [
            (
                3.0,
                include_str!("../../../../conformance/inkpen-v4-fixed-width-3-native.json"),
            ),
            (
                6.0,
                include_str!("../../../../conformance/inkpen-v4-fixed-width-native.json"),
            ),
            (
                12.0,
                include_str!("../../../../conformance/inkpen-v4-fixed-width-12-native.json"),
            ),
            (
                6.0,
                include_str!("../../../../conformance/inkpen-v4-fixed-width-geometry-native.json"),
            ),
        ]
        .into_iter()
        .map(|(width, data)| (width, serde_json::from_str(data).unwrap()))
        .collect();
        let matrix: Value = serde_json::from_str(include_str!(
            "../../../../conformance/inkpen-v4-fixed-width-matrix-native.json"
        ))
        .unwrap();
        cases.extend(
            matrix["cases"]
                .as_array()
                .unwrap()
                .iter()
                .map(|case| (case["width"].as_f64().unwrap() as f32, case.clone())),
        );
        for (width, fixture) in cases {
            let mut input = native_caller_stroke(&fixture["input"]);
            let r = input.rendering.as_mut().unwrap();
            r.properties.fixed_width = true;
            r.style.fixed_width = Some(width);
            r.style.color_argb = Some(0xff00_0000);
            assert!(crate::ink::prepare_inkpen_v4_for_page(&input, 1920, 1080).is_some());
            let mut unsupported = input.clone();
            unsupported.rendering.as_mut().unwrap().style.fixed_width = Some(-f32::MIN_POSITIVE);
            assert!(crate::ink::prepare_inkpen_v4_for_page(&unsupported, 1920, 1080).is_none());
            unsupported.rendering.as_mut().unwrap().style.fixed_width = Some(200.0_f32.next_up());
            assert!(crate::ink::prepare_inkpen_v4_for_page(&unsupported, 1920, 1080).is_none());
            let ink = prepare(
                &input,
                RuntimeContext {
                    curve_enabled: fixture["input"]["curve_enabled"].as_bool().unwrap(),
                    screen_width: 1920,
                    screen_height: 1080,
                },
            )
            .unwrap();
            let stream = fixture["stream"]["values_f32_bits"].as_array().unwrap();
            assert_eq!(stream.len(), ink.points.len() * 3);
            for (i, point) in ink.points.iter().enumerate() {
                assert_eq!(
                    (point.x as f32).to_bits(),
                    native_f32(stream[3 * i].as_str().unwrap()).to_bits()
                );
                assert_eq!(
                    (point.y as f32).to_bits(),
                    native_f32(stream[3 * i + 1].as_str().unwrap()).to_bits()
                );
                assert_eq!(
                    ink.radii[i].to_bits(),
                    native_f32(stream[3 * i + 2].as_str().unwrap()).to_bits()
                );
            }
            let packed = base64::engine::general_purpose::STANDARD
                .decode(fixture["rgba8_zlib_base64"].as_str().unwrap())
                .unwrap();
            let mut native = Vec::new();
            flate2::read::ZlibDecoder::new(&packed[..])
                .read_to_end(&mut native)
                .unwrap();
            let document = crate::Document {
                pages: vec![crate::Page {
                    uuid: "inkpen-fixed".into(),
                    width: 128,
                    height: 64,
                    content_bbox: BoundingBox::default(),
                    background_color: None,
                    template: None,
                    background: Default::default(),
                    objects: vec![input.into()],
                }],
                metadata: Default::default(),
            };
            let options = crate::RenderOptions {
                inkpen_profile: Some(crate::InkPenRenderProfile {
                    screen_width: 1920,
                    screen_height: 1080,
                }),
                ..Default::default()
            };
            let candidate = crate::render_document_svg(&document, &options)
                .remove(0)
                .svg;
            let generic = crate::render_document_svg(&document, &crate::RenderOptions::default())
                .remove(0)
                .svg;
            let raster = |raw: &str| {
                let start = raw.find('>').unwrap() + 1;
                let end = raw.rfind("</svg>").unwrap();
                let body = raw[start..end]
                    .trim_start()
                    .strip_prefix(
                        "<rect fill=\"#fcfcfc\" height=\"64\" width=\"128\" x=\"0\" y=\"0\"/>",
                    )
                    .unwrap();
                let svg = format!(
                    "{}<g transform=\"translate(0 64) scale(1 -1)\">{body}</g></svg>",
                    &raw[..start]
                );
                let tree = resvg::usvg::Tree::from_str(&svg, &Default::default()).unwrap();
                let mut pixmap = resvg::tiny_skia::Pixmap::new(128, 64).unwrap();
                resvg::render(
                    &tree,
                    resvg::tiny_skia::Transform::identity(),
                    &mut pixmap.as_mut(),
                );
                pixmap.data().to_vec()
            };
            let error = |pixels: &[u8]| {
                native
                    .as_chunks::<{ 128 * 4 }>()
                    .0
                    .iter()
                    .rev()
                    .flatten()
                    .zip(pixels)
                    .map(|(a, b)| u64::from(a.abs_diff(*b)))
                    .sum::<u64>()
            };
            let c = error(&raster(&candidate));
            let g = error(&raster(&generic));
            assert!(c < g, "candidate RGBA error {c}, generic {g}");
        }
    }
}
