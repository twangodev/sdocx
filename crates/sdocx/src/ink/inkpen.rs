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
            MAX_GENERATED_DOTS.saturating_sub(self.points.len()),
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
        if !p.x.is_finite() || !p.y.is_finite() || !r.is_finite() || !(r + 0.5).is_finite() {
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
        let adjusted = if change > 1. {
            if candidate < self.width {
                self.width - 1.
            } else {
                self.width + 1.
            }
        } else if self.width < 1. && self.width > candidate {
            self.width - 0.5
        } else {
            candidate
        };
        adjusted
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
            MAX_GENERATED_DOTS.saturating_sub(self.points.len()),
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
            MAX_GENERATED_DOTS.saturating_sub(self.points.len()),
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
        MAX_GENERATED_DOTS.saturating_sub(pass.points.len()),
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
    let r = s.rendering.as_ref()?;
    // Model's missing size is zero; the native Pen setter supplies the clamp.
    let requested_size = r.style.pen_size.unwrap_or(0.);
    let screen_min_dimension = context.screen_width.min(context.screen_height);
    if context.screen_width > i32::MAX as u32
        || context.screen_height > i32::MAX as u32
        || r.pen_name.as_deref() != Some(PEN)
        || !matches!(r.advanced_settings.as_deref(), Some("") | Some("4;"))
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
        for i in 1..s.points.len() {
            let raw = s.pressures[i] as f32;
            let pressure = if r.tool_type_raw == 1 || r.tool_type_raw == 3 {
                0.5
            } else if raw > 1. {
                1.
            } else {
                raw
            };
            pass.redraw(
                s.points[i].into(),
                pressure,
                s.timestamps[i],
                if r.tool_type_raw == 1 { 50. } else { 5. },
            )?;
            sample_ends[i] = pass.points.len();
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
        for i in 1..s.points.len() {
            let raw = s.pressures[i] as f32;
            let pressure = if r.tool_type_raw == 1 {
                0.5
            } else if raw > 1. {
                1.
            } else {
                raw
            };
            pass.redraw_no_curve(s.points[i].into(), pressure * size)?;
            sample_ends[i] = pass.points.len();
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
fn prepare_v1(s: &Stroke, context: RuntimeContext) -> Option<InkPenInk> {
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
}
