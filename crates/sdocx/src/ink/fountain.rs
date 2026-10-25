//! Saved FountainPen V14/V16 geometry, traced from Samsung Notes 4.4.45.37.
//! Arithmetic stays float32 where the native implementation uses float32.
use super::path::{P, Quad};
use crate::{Point, Stroke};

// FountainPen V16 getInverseScale, canonical canvas inverse scale = 1.
const REPEAT: f32 = 0.82;

/// Native GLV16's uniform inverse canvas scale: length of the X basis, then
/// reciprocal. Keep the float32 multiply/add/sqrt/divide sequence explicit.
pub(super) fn inverse_scale(output_scale: f32) -> Option<f32> {
    if !output_scale.is_finite() || output_scale <= 0.0 {
        return None;
    }
    let x_squared = output_scale * output_scale;
    let squared_length = (x_squared + 0.0_f32) + 0.0_f32;
    let length = squared_length.sqrt();
    let inverse = 1.0_f32 / length;
    (inverse.is_finite() && inverse > 0.0).then_some(inverse)
}

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
    fn smooth(&mut self, stylus: bool) {
        let window = if stylus { 3 } else { 1 };
        let smoothing_start = if stylus { 9 } else { 3 };
        for i in 1..self.widths.len() {
            let end = (i + i.min(window)).min(self.widths.len());
            let mut sum = 0.;
            for w in &self.widths[i..end] {
                sum += w.raw;
            }
            let mut w = sum / (end - i) as f32;
            w = (w + w) * 0.5;
            if i > smoothing_start {
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
    pub rgb: Option<Vec<[f32; 3]>>,
    pub rainbow_final_offset: Option<f32>,
    #[cfg(test)]
    pub rainbow_path_offsets: Vec<f32>,
}

#[derive(Clone, Copy)]
pub(crate) struct V17Rainbow<'a> {
    pub palette_argb: &'a [u32],
    pub period: i32,
    pub offset: f32,
}

struct RainbowState<'a> {
    palette_argb: &'a [u32],
    period: f32,
    offset: f32,
    current_rgb: Option<[f32; 3]>,
    invalid: bool,
    #[cfg(test)]
    path_offsets: Vec<f32>,
}

impl RainbowState<'_> {
    fn color_at(&self, distance: f32) -> Option<[f32; 3]> {
        let sum = distance + self.offset;
        if !sum.is_finite() || sum < 0. || !self.period.is_finite() || self.period <= 0. {
            return None;
        }
        let phase = (sum % self.period) / self.period;
        if !phase.is_finite() || phase < 0. {
            return None;
        }
        // Native computes the division in double precision, stores as float,
        // then uses FMA for the per-sector remainder.
        let step = (1.0_f64 / self.palette_argb.len() as f64) as f32;
        let scaled = phase / step;
        if !scaled.is_finite() || scaled < 0. {
            return None;
        }
        let index = (scaled as usize).min(self.palette_argb.len() - 1);
        let next = (index + 1) % self.palette_argb.len();
        let fraction = (-(index as f32)).mul_add(step, phase) / step;
        if !fraction.is_finite() {
            return None;
        }
        let channel = |shift: u32| {
            let a = ((self.palette_argb[index] >> shift) & 0xff_u32) as f32;
            let b = ((self.palette_argb[next] >> shift) & 0xff_u32) as f32;
            (b - a).mul_add(fraction, a) / 255.0
        };
        let rgb = [channel(16), channel(8), channel(0)];
        rgb.iter()
            .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
            .then_some(rgb)
    }

    fn advance(&mut self, length: f32) {
        self.offset += length;
        if !self.offset.is_finite() {
            self.invalid = true;
        }
        #[cfg(test)]
        self.path_offsets.push(self.offset);
    }
}
struct Pass<'a> {
    size: f32,
    repeat: f32,
    max_stamps: Option<usize>,
    attempted_stamps: usize,
    stamp_budget_exceeded: bool,
    short_move_threshold: f32,
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
    rainbow: Option<RainbowState<'a>>,
    rainbow_rgb: Vec<[f32; 3]>,
    rainbow_stamp_budget: usize,
    store_rainbow_rgb: bool,
    dots: Dots,
}
impl Pass<'_> {
    fn dot(&mut self, p: P, r: f32, local_distance: Option<f32>) {
        self.first = false;
        if let Some(limit) = self.max_stamps {
            if self.attempted_stamps >= limit {
                self.stamp_budget_exceeded = true;
                return;
            }
            self.attempted_stamps += 1;
        }
        if let Some(rainbow) = &mut self.rainbow {
            if let Some(distance) = local_distance {
                match rainbow.color_at(distance) {
                    Some(rgb) => rainbow.current_rgb = Some(rgb),
                    None => rainbow.invalid = true,
                }
            }
            if self.store_rainbow_rgb {
                if self.dots.points.len() >= self.rainbow_stamp_budget
                    || self.rainbow_rgb.len() >= self.rainbow_stamp_budget
                {
                    rainbow.invalid = true;
                    return;
                } else if let Some(rgb) = rainbow.current_rgb {
                    self.rainbow_rgb.push(rgb);
                } else {
                    rainbow.invalid = true;
                    self.rainbow_rgb.push([0.0; 3]);
                }
            }
        }
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
        if self.stamp_budget_exceeded {
            return;
        }
        if self.tolerance.skip(p) {
            self.tolerance.drop(p);
            return;
        }
        let delta = p.sub(self.previous);
        let distance = delta.len();
        if distance < self.short_move_threshold {
            self.alternate = !self.alternate;
            if !self.alternate {
                self.tolerance.drop(p);
                return;
            }
        } else {
            self.alternate = true;
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
        if self.max_stamps.is_some()
            && !self.sampling_budget_allows((q.length - self.residual) / self.repeat)
        {
            return;
        }
        let count = ((q.length - self.residual) / self.repeat) as usize + 1;
        let increment = (width - self.width) / count as f32;
        let mut w = self.width;
        let mut d = self.residual;
        while d <= q.length {
            w += increment;
            self.dot(q.at(d), w * 0.5, Some(d));
            if self.stamp_budget_exceeded {
                return;
            }
            d += self.repeat;
        }
        if let Some(rainbow) = &mut self.rainbow {
            rainbow.advance(q.length);
        }
        self.residual = d - q.length;
        self.previous = p;
        self.midpoint = mid;
        self.width = width;
        self.tolerance.accept(p);
    }
    fn end(&mut self, p: P, time: i64) {
        if self.stamp_budget_exceeded {
            return;
        }
        if self.first {
            let width = self.history.width(time, self.size * 0.5);
            // The native V17 single-point path still evaluates rainbow at
            // local distance zero before appending its one stamp. Ordinary
            // V14/V16 passes keep their original no-color path.
            self.dot(
                self.previous,
                width * 0.5,
                self.rainbow.as_ref().map(|_| 0.0),
            );
        } else {
            let q = Quad::new(self.midpoint, self.previous, p);
            let mut d = self.residual;
            if q.length > 0. {
                if self.max_stamps.is_some()
                    && d <= q.length
                    && !self.sampling_budget_allows((q.length - d) / self.repeat)
                {
                    return;
                }
                while d <= q.length {
                    self.dot(q.at(d), self.width * 0.5, Some(d));
                    if self.stamp_budget_exceeded {
                        return;
                    }
                    d += self.repeat;
                }
            }
            if let Some(rainbow) = &mut self.rainbow {
                rainbow.advance(q.length);
            }
            self.dot(p, self.width * 0.5, None);
        }
    }

    fn sampling_budget_allows(&mut self, quotient: f32) -> bool {
        let Some(limit) = self.max_stamps else {
            return true;
        };
        let remaining = limit.saturating_sub(self.attempted_stamps);
        if !quotient.is_finite() || quotient >= remaining as f32 {
            self.stamp_budget_exceeded = true;
            return false;
        }
        true
    }
}

pub(super) fn prepare(s: &Stroke) -> Option<Dots> {
    prepare_with_rainbow(s, None)
}

pub(super) fn prepare_with_output_scale(s: &Stroke, output_scale: f32) -> Option<Dots> {
    prepare_with_rainbow_mode(s, None, false, Some(output_scale))
}

pub(super) fn prepare_v16_rainbow_ignored(s: &Stroke) -> Option<Dots> {
    prepare_v16_rainbow_ignored_with_output_scale(s, None)
}

pub(super) fn prepare_v16_rainbow_ignored_with_output_scale(
    s: &Stroke,
    output_scale: Option<f32>,
) -> Option<Dots> {
    let rendering = s.rendering.as_ref()?;
    if rendering.advanced_settings.as_deref() != Some("18;0;100;")
        || rendering.tool_type_raw != 2
        || rendering.properties.fixed_width
        || !rendering.properties.stylus_channels
        || !rendering.properties.rainbow_effect
    {
        return None;
    }
    prepare_with_rainbow_mode(s, None, true, output_scale)
}

pub(crate) fn prepare_v17_rainbow(s: &Stroke, profile: V17Rainbow<'_>) -> Option<Dots> {
    prepare_with_rainbow(s, Some(profile))
}

fn prepare_with_rainbow(s: &Stroke, profile: Option<V17Rainbow<'_>>) -> Option<Dots> {
    prepare_with_rainbow_mode(s, profile, false, None)
}

fn prepare_with_rainbow_mode(
    s: &Stroke,
    profile: Option<V17Rainbow<'_>>,
    ignore_glv16_rainbow_flag: bool,
    v16_output_scale: Option<f32>,
) -> Option<Dots> {
    let r = s.rendering.as_ref()?;
    let v14 = r.advanced_settings.as_deref() == Some("14;");
    // Both saved drawable versions consume ObjectStroke's saved style size.
    // A missing native size reads as zero, then Pen::SetSize clamps it to 0.4.
    let saved_size = r.style.pen_size.unwrap_or(0.);
    if !saved_size.is_finite() || saved_size < 0. {
        return None;
    }
    let size = saved_size.clamp(0.4, 800.);
    let input_supported = matches!(r.tool_type_raw, 1..=3);
    let fixed_width_supported = !r.properties.fixed_width
        || r.style
            .fixed_width
            .is_some_and(|w| (0. ..=1024.).contains(&w));
    // Legacy saved pressure quantization can produce small negative values.
    // Both profiles handle these through native width and stamp-radius floors.
    let minimum_pressure = -1.;
    let rainbow = if let Some(profile) = profile {
        if r.advanced_settings.as_deref() != Some("18;0;100;")
            || r.tool_type_raw != 2
            || !r.properties.rainbow_effect
            || r.properties.fixed_width
            || r.properties.fixed_opacity
            || r.properties.alpha_lock
            || r.properties.replay_only
            || r.properties.reveal_mode
            || r.properties.top_layer_pen
            || r.properties.generated
            || !r.properties.binary_added
            || !r.properties.stylus_channels
            || profile.period <= 0
            || !profile.offset.is_finite()
            || profile.offset < 0.0
            || !(2..=256).contains(&profile.palette_argb.len())
        {
            return None;
        }
        Some(profile)
    } else {
        None
    };
    if r.pen_name.as_deref() != Some("com.samsung.android.sdk.pen.pen.preload.FountainPen")
        || (!matches!(r.advanced_settings.as_deref(), Some("14;" | "18;0;100;")))
        || (rainbow.is_some() && v14)
        || !input_supported
        || !fixed_width_supported
        || r.properties.eraser
        || r.properties.straighten
        || (r.properties.rainbow_effect && rainbow.is_none() && !ignore_glv16_rainbow_flag)
        || (rainbow.is_some()
            && (r.properties.fixed_opacity
                || r.properties.alpha_lock
                || r.properties.replay_only
                || r.properties.reveal_mode
                || r.properties.top_layer_pen
                || !r.properties.stylus_channels
                || r.style.stroke_type_raw.is_some_and(|value| value != 0)
                || r.style.line_type_raw.is_some_and(|value| value != 0)))
        || s.points.is_empty()
        || s.pressures.len() != s.points.len()
        || s.timestamps.len() != s.points.len()
        || s.timestamps
            .iter()
            .any(|&time| i32::try_from(time).is_err())
        || (!s.tilts.is_empty() && s.tilts.len() != s.points.len())
        || !size.is_finite()
        || size <= 0.
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
    // The native Model getter returns zero when this V14/V16 style value is
    // absent, and RedrawPen copies that value into drawable+44.
    let tolerance = r.style.initial_tolerance.unwrap_or(0.);
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
    if rainbow.is_some() && (distance > f64::from(REPEAT) * 32_000.0 || s.points.len() > 32_768) {
        return None;
    }
    if v14 {
        return Some(super::fountain_v14::prepare(s, size, tolerance));
    }
    let repeat = match v16_output_scale {
        Some(scale) => {
            let inverse_scale = inverse_scale(scale)?;
            if scale == 1.0 {
                REPEAT
            } else {
                REPEAT * inverse_scale
            }
        }
        None => REPEAT,
    };
    if !repeat.is_finite() || repeat <= 0.0 {
        return None;
    }
    // Scaled sampling can increase the source-generated stamp count. Bound
    // this opt-in route before it can expand a modest stroke into unbounded
    // SVG work; the unscaled compatibility path keeps its historical budget.
    let max_stamps: Option<usize> = v16_output_scale.map(|_| 32_000);
    let stylus = r.tool_type_raw == 2;
    let pressure_at = |i| if stylus { s.pressures[i] as f32 } else { 0.5 };
    let first = P::from(s.points[0]);
    let mut history = History::default();
    let mut ratios = [0.; 3];
    let mut ratio_count = 0;
    let mut output = None;
    let mut attempted_scaled_stamps = 0;
    for _ in 0..2 {
        let store_rainbow_rgb = history.replay;
        let width = history.width(s.timestamps[0], (size * 0.5) * pressure_at(0).min(1.));
        let mut pass = Pass {
            size,
            repeat,
            max_stamps: max_stamps.map(|limit| limit.saturating_sub(attempted_scaled_stamps)),
            attempted_stamps: 0,
            stamp_budget_exceeded: false,
            short_move_threshold: if r.tool_type_raw == 1 { 50. } else { 5. },
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
            rainbow: rainbow.map(|profile| RainbowState {
                palette_argb: profile.palette_argb,
                period: profile.period as f32,
                offset: profile.offset,
                current_rgb: None,
                invalid: false,
                #[cfg(test)]
                path_offsets: Vec::new(),
            }),
            rainbow_rgb: if store_rainbow_rgb {
                Vec::with_capacity(32_000)
            } else {
                Vec::new()
            },
            rainbow_stamp_budget: 32_000,
            store_rainbow_rgb,
            dots: Dots {
                directions: None,
                points: vec![],
                radii: vec![],
                sample_ends: vec![0; s.points.len()],
                rgb: None,
                rainbow_final_offset: None,
                #[cfg(test)]
                rainbow_path_offsets: Vec::new(),
            },
        };
        for i in 1..s.points.len().saturating_sub(1) {
            let tilt = if stylus {
                s.tilts.get(i).copied().unwrap_or(0.) as f32
            } else {
                0.8
            };
            let degrees = ((tilt * 180.) as f64 / std::f64::consts::PI) as f32;
            let tilt = (degrees.min(75.) - 15.).max(0.) / 60. * 3.;
            pass.line(s.points[i].into(), pressure_at(i), tilt, s.timestamps[i]);
            if pass.stamp_budget_exceeded {
                return None;
            }
            pass.dots.sample_ends[i] = pass.dots.points.len();
        }
        let last = s.points.len() - 1;
        // MotionEvent::GetEventTime returns the latest event time relative to
        // the MotionEvent constructor's base timestamp. ObjectStroke channel
        // timestamps remain absolute in the width-history entries.
        let terminal_event_time = s.timestamps[last] - s.timestamps[0];
        pass.end(s.points[last].into(), terminal_event_time);
        pass.dots.sample_ends[last] = pass.dots.points.len();
        if pass.stamp_budget_exceeded {
            return None;
        }
        attempted_scaled_stamps += pass.attempted_stamps;
        if let Some(rainbow) = pass.rainbow {
            if rainbow.invalid {
                return None;
            }
            if store_rainbow_rgb {
                pass.dots.rgb = Some(pass.rainbow_rgb);
                pass.dots.rainbow_final_offset = Some(rainbow.offset);
                #[cfg(test)]
                {
                    pass.dots.rainbow_path_offsets = rainbow.path_offsets;
                }
            }
        }
        output = Some(pass.dots);
        if !history.replay {
            history.smooth(stylus);
        }
    }
    let mut output = output?;
    if r.properties.fixed_width {
        let radius = (r.style.fixed_width.unwrap() * 0.5).max(0.1);
        output.radii.fill(radius as f64);
    }
    Some(output)
}

#[cfg(all(test, feature = "serde"))]
mod tests {
    use super::*;
    use serde::Deserialize;
    use serde_json::Value;
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
        #[serde(default)]
        fixed_width: Option<f32>,
        #[serde(default)]
        tool_type_raw: Option<u16>,
        samples: Vec<(f64, f64, f64, f64, i64)>,
        dots: Vec<Vec<f64>>,
        sample_ends: Vec<usize>,
    }
    fn reference() -> Reference {
        serde_json::from_str(include_str!("../../../../conformance/fountain-v16.json")).unwrap()
    }
    fn little_endian_u32(value: &Value) -> u32 {
        let value = value.as_str().unwrap();
        assert_eq!(value.len(), 8);
        u32::from_le_bytes(std::array::from_fn(|index| {
            u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).unwrap()
        }))
    }

    fn little_endian_f32(value: &Value) -> f32 {
        f32::from_bits(little_endian_u32(value))
    }

    fn source_array_f32(value: &Value) -> f32 {
        f32::from_bits(u32::from_str_radix(value.as_str().unwrap(), 16).unwrap())
    }

    #[test]
    fn generated_geometry_matches_native_saved_stroke_helpers() {
        for reference in [
            reference(),
            serde_json::from_str(include_str!("../../../../conformance/fountain-v14.json"))
                .unwrap(),
        ] {
            let v14 = reference
                .stroke
                .rendering
                .as_ref()
                .unwrap()
                .advanced_settings
                .as_deref()
                == Some("14;");
            for case in reference.cases {
                let mut stroke = reference.stroke.clone();
                stroke.pen_width = case.size;
                let style = &mut stroke.rendering.as_mut().unwrap().style;
                style.initial_tolerance = Some(case.tolerance);
                // The normalized helper adapter maps this case's size to the
                // native saved style size for both drawable versions.
                style.pen_size = Some(case.size);
                if let Some(tool) = case.tool_type_raw {
                    stroke.rendering.as_mut().unwrap().tool_type_raw = tool;
                }
                if let Some(width) = case.fixed_width {
                    let rendering = stroke.rendering.as_mut().unwrap();
                    rendering.properties.fixed_width = true;
                    rendering.style.fixed_width = Some(width);
                }
                for (x, y, pressure, tilt, time) in case.samples {
                    stroke.points.push(Point { x, y });
                    stroke.pressures.push(pressure);
                    stroke.tilts.push(tilt);
                    stroke.timestamps.push(time);
                }
                let actual = if v14 {
                    // This fixture captures the lower-level V14 sampler helper,
                    // before Drawing applies the saved Pen::SetSize clamp.
                    super::super::fountain_v14::prepare(&stroke, case.size, case.tolerance)
                } else {
                    prepare(&stroke).unwrap_or_else(|| {
                        panic!("{} unexpectedly used fallback geometry", case.name)
                    })
                };
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
    fn v16_scaled_centers_and_radii_match_native_redraw() {
        use sha2::{Digest, Sha256};

        let bytes = include_bytes!("../../tests/fixtures/fountain-v16-scale-native.json");
        assert_eq!(
            format!("{:x}", Sha256::digest(bytes)),
            "8f5d82ff5f6dc8beadae6a22be7b57839d271bb7006b8cc8a8a0a06433297b4e"
        );
        let fixture: Value = serde_json::from_slice(bytes).unwrap();
        assert_eq!(
            fixture["capture_sha256"],
            "c4def42b77ae1b7f480507253cf0a696575e52d28dc751295436b289b1ad3866"
        );
        let stroke: Stroke = serde_json::from_value(fixture["stroke"].clone()).unwrap();
        assert_eq!(stroke.points.len(), 204);
        for case in fixture["cases"].as_array().unwrap() {
            let q = case["q"].as_f64().unwrap() as f32;
            let actual = prepare_with_output_scale(&stroke, q).unwrap();
            let expected = case["xy_radius_f32_bits"].as_array().unwrap();
            assert_eq!(actual.points.len(), expected.len(), "native count at q={q}");
            for ((point, radius), expected) in actual.points.iter().zip(&actual.radii).zip(expected)
            {
                let bits: [u32; 3] = std::array::from_fn(|lane| {
                    u32::from_str_radix(expected[lane].as_str().unwrap(), 16).unwrap()
                });
                assert_eq!(
                    [
                        (point.x as f32).to_bits(),
                        (point.y as f32).to_bits(),
                        (*radius as f32).to_bits()
                    ],
                    bits,
                    "native center/radius at q={q}"
                );
            }
            assert_eq!(actual.sample_ends.last().copied(), Some(expected.len()));
            assert!(actual.sample_ends.windows(2).all(|pair| pair[0] <= pair[1]));
            if q == 1.0 {
                let canonical = prepare(&stroke).unwrap();
                assert_eq!(actual.points, canonical.points);
                assert_eq!(actual.radii, canonical.radii);
                assert_eq!(actual.sample_ends, canonical.sample_ends);
            }
        }
    }

    #[test]
    fn v16_explicit_scale_changes_sampling_and_keeps_unit_scale_exact() {
        let reference = reference();
        let case = reference
            .cases
            .iter()
            .find(|case| case.name == "right angle")
            .expect("native-backed V16 right-angle case");
        let mut stroke = reference.stroke.clone();
        stroke.pen_width = case.size;
        let style = &mut stroke.rendering.as_mut().unwrap().style;
        style.initial_tolerance = Some(case.tolerance);
        style.pen_size = Some(case.size);
        for &(x, y, pressure, tilt, time) in &case.samples {
            stroke.points.push(Point { x, y });
            stroke.pressures.push(pressure);
            stroke.tilts.push(tilt);
            stroke.timestamps.push(time);
        }

        let canonical = prepare(&stroke).expect("canonical native-backed V16 geometry");
        let unit = prepare_with_output_scale(&stroke, 1.0).expect("explicit unit profile");
        assert_eq!(canonical.sample_ends, unit.sample_ends);
        assert_eq!(canonical.points, unit.points);
        assert_eq!(canonical.radii, unit.radii);
        assert_eq!(canonical.points.len(), case.dots.len());

        let down = prepare_with_output_scale(&stroke, 0.5).expect("half-scale V16 geometry");
        let up = prepare_with_output_scale(&stroke, 2.0).expect("double-scale V16 geometry");
        assert!(down.points.len() < unit.points.len());
        assert!(up.points.len() > unit.points.len());
        for dots in [&down, &unit, &up] {
            assert_eq!(dots.sample_ends.len(), stroke.points.len());
            assert_eq!(dots.sample_ends.last().copied(), Some(dots.points.len()));
            assert!(dots.sample_ends.windows(2).all(|pair| pair[0] <= pair[1]));
        }
    }

    #[test]
    fn v16_scaled_sampling_rejects_unrepresentable_or_over_budget_scales() {
        let reference = reference();
        let case = reference
            .cases
            .iter()
            .find(|case| case.name == "right angle")
            .expect("native-backed V16 right-angle case");
        let mut stroke = reference.stroke;
        stroke.pen_width = case.size;
        let style = &mut stroke.rendering.as_mut().unwrap().style;
        style.initial_tolerance = Some(case.tolerance);
        style.pen_size = Some(case.size);
        for &(x, y, pressure, tilt, time) in &case.samples {
            stroke.points.push(Point { x, y });
            stroke.pressures.push(pressure);
            stroke.tilts.push(tilt);
            stroke.timestamps.push(time);
        }
        assert!(prepare_with_output_scale(&stroke, 1.0e30).is_none());
        assert!(prepare_with_output_scale(&stroke, 1.0e-30).is_none());
        assert!(prepare_with_output_scale(&stroke, 20_000.0).is_none());
    }

    #[test]
    fn persisted_v14_decoding_and_preparation_match_native_drawing() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/fountain-persisted-drawing-native.json"
        ))
        .unwrap();
        let saved: Value = serde_json::from_str(include_str!(
            "../../../../conformance/fountain-saved-color-native.json"
        ))
        .unwrap();
        let frame = saved["real_serialized_frames"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["id"] == "real_v14_handwritten_flat1445")
            .unwrap();
        let hex = format!(
            "{}{}",
            fixture["provenance"]["object_payload"]["prefix_hex"]
                .as_str()
                .unwrap(),
            frame["frame_hex"].as_str().unwrap()
        );
        let payload: Vec<u8> = hex
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        let mut stroke =
            crate::decode::decode_stroke(&payload, &crate::ParseLimits::default()).unwrap();
        let native = &fixture["cases"][0];
        // Resource resolution is separate from frame decoding. Native OnAttach
        // resolved these names from the retained original StringID table.
        let rendering = stroke.rendering.as_mut().unwrap();
        rendering.pen_name = Some(native["native_pen_name"].as_str().unwrap().into());
        rendering.advanced_settings =
            Some(native["native_advanced_setting"].as_str().unwrap().into());
        assert_eq!(
            rendering.style.pen_size.unwrap().to_bits(),
            little_endian_u32(&native["native_state_after_apply"]["size_bits"])
        );
        let arrays = &native["source_getter_arrays"];
        let expected_points = arrays["points_f32_bits"].as_array().unwrap();
        assert_eq!(stroke.points.len() * 2, expected_points.len());
        assert_eq!(
            stroke.timestamps,
            arrays["timestamps_i32"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_i64().unwrap())
                .collect::<Vec<_>>()
        );
        for (point, xy) in stroke.points.iter().zip(expected_points.as_chunks::<2>().0) {
            assert_eq!(
                (point.x as f32).to_bits(),
                source_array_f32(&xy[0]).to_bits()
            );
            assert_eq!(
                (point.y as f32).to_bits(),
                source_array_f32(&xy[1]).to_bits()
            );
        }
        for (values, field) in [
            (&stroke.pressures, "pressure_f32_bits"),
            (&stroke.tilts, "tilt_f32_bits"),
            (&stroke.orientations, "orientation_f32_bits"),
        ] {
            let expected = arrays[field].as_array().unwrap();
            assert_eq!(values.len(), expected.len());
            for (actual, expected) in values.iter().zip(expected) {
                assert_eq!(
                    (*actual as f32).to_bits(),
                    source_array_f32(expected).to_bits()
                );
            }
        }
        let dots = prepare(&stroke).unwrap();
        let callbacks = native["drawing_addpoint_records"].as_array().unwrap();
        assert_eq!(dots.points.len(), callbacks.len());
        assert_eq!(dots.sample_ends.len(), stroke.points.len());
        assert_eq!(dots.sample_ends.last(), Some(&callbacks.len()));
        let directions = dots.directions.as_ref().unwrap();
        for (index, callback) in callbacks.iter().enumerate() {
            let words = callback["float_words_s0_s5"].as_array().unwrap();
            for (actual, channel) in [
                (dots.points[index].x, 0),
                (dots.points[index].y, 1),
                (directions[index].x, 3),
                (directions[index].y, 4),
            ] {
                assert_eq!(
                    (actual as f32).to_bits(),
                    little_endian_u32(&words[channel])
                );
            }
            assert_eq!(
                (dots.radii[index] as f32).to_bits(),
                little_endian_u32(&words[2]),
                "persisted radius {index}"
            );
            assert_eq!(little_endian_f32(&words[5]), 1.0);
        }
    }

    #[test]
    fn as18_glv16_ignores_forwarded_rainbow_flag_and_matches_native_addpoint_arguments() {
        let drawing: Value = serde_json::from_str(include_str!(
            "../../../../conformance/fountain-persisted-drawing-native.json"
        ))
        .unwrap();
        let saved: Value = serde_json::from_str(include_str!(
            "../../../../conformance/fountain-saved-color-native.json"
        ))
        .unwrap();
        let frame = saved["real_serialized_frames"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["id"] == "real_v14_handwritten_flat1445")
            .unwrap();
        let payload_hex = format!(
            "{}{}",
            drawing["provenance"]["object_payload"]["prefix_hex"]
                .as_str()
                .unwrap(),
            frame["frame_hex"].as_str().unwrap()
        );
        let payload: Vec<u8> = payload_hex
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        let mut stroke =
            crate::decode::decode_stroke(&payload, &crate::ParseLimits::default()).unwrap();
        let native = &drawing["cases"][0];
        let rendering = stroke.rendering.as_mut().unwrap();
        rendering.pen_name = Some(native["native_pen_name"].as_str().unwrap().into());
        rendering.advanced_settings = Some("18;0;100;".into());
        rendering.tool_type_raw = 2;
        rendering.properties.fixed_width = false;
        rendering.properties.stylus_channels = true;
        rendering.properties.rainbow_effect = false;
        rendering.style.color_type_raw = Some(1);
        rendering.style.rainbow_distance = Some(37);
        rendering.style.rainbow_offset = Some(2.75);
        rendering.style.gradient_colors_argb = Some(vec![0xffff_0000, 0xff00_ff00, 0xff00_00ff]);

        let plain = prepare(&stroke).expect("AS18 GLV16 plain control");
        stroke.rendering.as_mut().unwrap().properties.rainbow_effect = true;
        let ignored =
            prepare_v16_rainbow_ignored(&stroke).expect("bounded AS18 GLV16 rainbow-flag control");
        assert_eq!(ignored.points, plain.points);
        assert_eq!(ignored.radii, plain.radii);
        assert_eq!(ignored.sample_ends, plain.sample_ends);
        assert!(ignored.rgb.is_none());

        let unit = prepare_v16_rainbow_ignored_with_output_scale(&stroke, Some(1.0))
            .expect("unit-scale rainbow-flag-ignored route");
        assert_eq!(unit.points, ignored.points);
        assert_eq!(unit.radii, ignored.radii);
        assert_eq!(unit.sample_ends, ignored.sample_ends);
        assert!(unit.rgb.is_none());
        let down = prepare_v16_rainbow_ignored_with_output_scale(&stroke, Some(0.5))
            .expect("downscaled rainbow-flag-ignored route");
        let up = prepare_v16_rainbow_ignored_with_output_scale(&stroke, Some(2.0))
            .expect("upscaled rainbow-flag-ignored route");
        assert!(down.points.len() < unit.points.len());
        assert!(up.points.len() > unit.points.len());
        for scaled in [&down, &unit, &up] {
            assert!(scaled.rgb.is_none());
            assert_eq!(
                scaled.sample_ends.last().copied(),
                Some(scaled.points.len())
            );
            assert!(scaled.sample_ends.windows(2).all(|pair| pair[0] <= pair[1]));
        }
        let oversized =
            crate::ink::prepare_stroke_for_context_with_fountain_v16_rainbow_ignored_output_scale(
                &stroke,
                crate::ink::InkRenderContext::CanonicalPage,
                20_000.0,
            );
        let canonical = crate::ink::prepare_stroke_for_context_with_fountain_v16_rainbow_ignored(
            &stroke,
            crate::ink::InkRenderContext::CanonicalPage,
        );
        assert_eq!(oversized.points, canonical.points);
        assert_eq!(
            oversized.dot_radii.as_deref().unwrap(),
            ignored.radii.as_slice()
        );
        assert_eq!(oversized.sample_ends, canonical.sample_ends);
        assert_eq!(oversized.fountain_output_scale, None);

        let golden: Value = serde_json::from_str(include_str!(
            "../../../../conformance/fountain-as18-glv16-rainbow-ignored-native.json"
        ))
        .unwrap();
        assert_eq!(
            golden["native_addpoint_s0_s3_f32_bits"]
                .as_array()
                .unwrap()
                .len(),
            24
        );
        assert_eq!(ignored.points.len(), 24);
        assert_eq!(ignored.radii.len(), 24);
        for (index, (point, radius)) in ignored.points.iter().zip(&ignored.radii).enumerate() {
            let expected = golden["native_addpoint_s0_s3_f32_bits"][index]
                .as_array()
                .unwrap();
            for (actual, word_index) in [(point.x, 0), (point.y, 1), (*radius, 2)] {
                assert_eq!(
                    (actual as f32).to_bits(),
                    little_endian_u32(&expected[word_index]),
                    "native GLV16 AddPoint argument {index}:{word_index}"
                );
            }
            assert_eq!(little_endian_f32(&expected[3]), 1.0);
        }
    }

    #[test]
    fn v16_native_size_and_tolerance_defaults_match_all_native_argument_records() {
        for encoded in [
            include_str!("../../../../conformance/fountain-v16-defaults-native.json"),
            include_str!("../../../../conformance/fountain-v16-event-time-native.json"),
        ] {
            let fixture: Value = serde_json::from_str(encoded).unwrap();
            // In the captured no-line size-10 case, ObjectStroke times are
            // 100/110/120 while MotionEvent::GetEventTime returns 20, relative to
            // its constructor base of 100. The terminal history lookup therefore
            // precedes the first history entry and returns its initial width.
            for case in fixture["cases"].as_array().unwrap() {
                let name = case["case"].as_str().unwrap();
                let mut stroke = reference().stroke;
                let rendering = stroke.rendering.as_mut().unwrap();
                rendering.advanced_settings =
                    Some(case["advanced_setting"].as_str().unwrap().into());
                rendering.tool_type_raw = fixture["source_input"]["tool_type"]
                    .as_u64()
                    .or_else(|| fixture["tool_type"].as_u64())
                    .unwrap() as u16;
                rendering.style.pen_size = if case["style_pen_size_setter"] == "omit" {
                    None
                } else {
                    Some(little_endian_f32(
                        &case["source_model_get_pen_size_f32_bits"],
                    ))
                };
                rendering.style.initial_tolerance =
                    if case["style_initial_tolerance_setter"] == "omit" {
                        None
                    } else {
                        Some(little_endian_f32(
                            &case["model_get_initial_tolerance_f32_bits"],
                        ))
                    };

                let source = &case["source_arrays"];
                let point_words = source["points_f32_bits"].as_array().unwrap();
                stroke.points = point_words
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|pair| Point {
                        x: f64::from(source_array_f32(&pair[0])),
                        y: f64::from(source_array_f32(&pair[1])),
                    })
                    .collect();
                stroke.pressures = source["pressure_f32_bits"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| f64::from(source_array_f32(value)))
                    .collect();
                stroke.tilts = source["tilt_f32_bits"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| f64::from(source_array_f32(value)))
                    .collect();
                stroke.timestamps = source["timestamps_i32"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| value.as_i64().unwrap())
                    .collect();

                // Deliberately disagree with the native saved style. V16 geometry
                // must follow style.pen_size, not the display/public width field.
                stroke.pen_width = 99.;
                let dots = prepare(&stroke).unwrap_or_else(|| panic!("{name} rejected"));
                let native = case["v16_rtv5_addpoint_host_boundary"].as_array().unwrap();
                assert_eq!(dots.points.len(), native.len(), "{name} record count");
                for (index, ((point, radius), callback)) in
                    dots.points.iter().zip(&dots.radii).zip(native).enumerate()
                {
                    let words = callback["float_words_s0_s3"].as_array().unwrap();
                    assert_eq!(
                        (point.x as f32).to_bits(),
                        little_endian_u32(&words[0]),
                        "{name} record {index} x"
                    );
                    assert_eq!(
                        (point.y as f32).to_bits(),
                        little_endian_u32(&words[1]),
                        "{name} record {index} y"
                    );
                    assert_eq!(
                        (*radius as f32).to_bits(),
                        little_endian_u32(&words[2]),
                        "{name} record {index} vector z"
                    );
                    assert_eq!(little_endian_u32(&words[3]), 1.0f32.to_bits());
                }
                let model_tolerance = stroke
                    .rendering
                    .as_ref()
                    .unwrap()
                    .style
                    .initial_tolerance
                    .unwrap_or(0.);
                assert_eq!(
                    model_tolerance.to_bits(),
                    little_endian_f32(&case["model_get_initial_tolerance_f32_bits"]).to_bits(),
                    "{name} Model tolerance getter"
                );
                let raw_size = stroke
                    .rendering
                    .as_ref()
                    .unwrap()
                    .style
                    .pen_size
                    .unwrap_or(0.);
                // Preserve the captured max-then-min NaN behavior.
                #[allow(clippy::manual_clamp)]
                let clamped_size = raw_size.max(0.4).min(800.);
                assert_eq!(
                    clamped_size.to_bits(),
                    little_endian_f32(&case["native_pen_getsize_f32_bits"]).to_bits(),
                    "{name} native Pen GetSize"
                );
            }
        }
    }

    #[test]
    fn v14_saved_size_and_missing_tolerance_match_native_drawing_records() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/fountain-v14-defaults-native.json"
        ))
        .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let name = case["case"].as_str().unwrap();
            let mut stroke = reference().stroke;
            let rendering = stroke.rendering.as_mut().unwrap();
            rendering.advanced_settings = Some("14;".into());
            rendering.pen_name = Some("com.samsung.android.sdk.pen.pen.preload.FountainPen".into());
            rendering.tool_type_raw = fixture["source_input"]["tool_type"].as_u64().unwrap() as u16;
            rendering.style.pen_size = if case["style_saved_size_setter"] == "omit" {
                None
            } else {
                Some(little_endian_f32(
                    &case["source_model_get_pen_size_f32_bits"],
                ))
            };
            rendering.style.initial_tolerance = if case["style_initial_tolerance_setter"] == "omit"
            {
                None
            } else {
                Some(little_endian_f32(
                    &case["model_get_initial_tolerance_f32_bits"],
                ))
            };
            stroke.pen_width =
                little_endian_f32(&case["adapter_public_pen_width_metadata_f32_bits"]);

            let source = &case["source_arrays"];
            let point_words = source["points_f32_bits"].as_array().unwrap();
            stroke.points = point_words
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| Point {
                    x: f64::from(source_array_f32(&pair[0])),
                    y: f64::from(source_array_f32(&pair[1])),
                })
                .collect();
            stroke.pressures = source["pressure_f32_bits"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| f64::from(source_array_f32(value)))
                .collect();
            stroke.tilts = source["tilt_f32_bits"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| f64::from(source_array_f32(value)))
                .collect();
            stroke.timestamps = source["timestamps_i32"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_i64().unwrap())
                .collect();

            let dots = prepare(&stroke).unwrap_or_else(|| panic!("{name} rejected"));
            let native = case["rtv_addpoint_entries"].as_array().unwrap();
            assert_eq!(dots.points.len(), native.len(), "{name} RTV4 count");
            let directions = dots.directions.as_ref().unwrap();
            for (index, ((point, radius), callback)) in
                dots.points.iter().zip(&dots.radii).zip(native).enumerate()
            {
                let words = callback["float_words_s0_s5"].as_array().unwrap();
                assert_eq!(
                    (point.x as f32).to_bits(),
                    little_endian_u32(&words[0]),
                    "{name} record {index} x"
                );
                assert_eq!(
                    (point.y as f32).to_bits(),
                    little_endian_u32(&words[1]),
                    "{name} record {index} y"
                );
                assert_eq!(
                    (*radius as f32).to_bits(),
                    little_endian_u32(&words[2]),
                    "{name} record {index} radius"
                );
                assert_eq!(
                    (directions[index].x as f32).to_bits(),
                    little_endian_u32(&words[3]),
                    "{name} record {index} direction x"
                );
                assert_eq!(
                    (directions[index].y as f32).to_bits(),
                    little_endian_u32(&words[4]),
                    "{name} record {index} direction y"
                );
                assert_eq!(
                    little_endian_u32(&words[5]),
                    1.0f32.to_bits(),
                    "{name} alpha"
                );
            }
            let effective_size = stroke
                .rendering
                .as_ref()
                .unwrap()
                .style
                .pen_size
                .unwrap_or(0.)
                .clamp(0.4, 800.);
            assert_eq!(
                effective_size.to_bits(),
                little_endian_f32(&case["native_pen_getsize_f32_bits"]).to_bits(),
                "{name} native Pen GetSize"
            );
            assert_eq!(
                stroke
                    .rendering
                    .as_ref()
                    .unwrap()
                    .style
                    .initial_tolerance
                    .unwrap_or(0.)
                    .to_bits(),
                little_endian_f32(&case["model_get_initial_tolerance_f32_bits"]).to_bits(),
                "{name} native tolerance default/setter"
            );
        }
    }

    #[test]
    fn v14_rejects_negative_or_nonfinite_style_size_before_clamp() {
        let mut stroke = reference().stroke;
        stroke.rendering.as_mut().unwrap().advanced_settings = Some("14;".into());
        stroke.pen_width = 99.;
        stroke.points = vec![Point { x: 0., y: 0. }];
        stroke.pressures = vec![0.5];
        stroke.timestamps = vec![0];
        stroke.rendering.as_mut().unwrap().style.initial_tolerance = None;
        for size in [-0.1, f32::NAN, f32::INFINITY] {
            stroke.rendering.as_mut().unwrap().style.pen_size = Some(size);
            assert!(prepare(&stroke).is_none(), "{size:?}");
        }
        stroke.rendering.as_mut().unwrap().style.pen_size = None;
        assert_eq!(prepare(&stroke).unwrap().radii, vec![f64::from(0.1f32)]);
        stroke.rendering.as_mut().unwrap().style.pen_size = Some(900.);
        assert_eq!(prepare(&stroke).unwrap().radii, vec![200.]);
    }

    #[test]
    fn v16_rejects_negative_or_nonfinite_style_size_before_native_clamp() {
        let mut stroke = reference().stroke;
        stroke.points = vec![Point { x: 0., y: 0. }];
        stroke.pressures = vec![0.5];
        stroke.timestamps = vec![0];
        stroke.pen_width = 99.;
        stroke.rendering.as_mut().unwrap().style.initial_tolerance = None;
        for size in [-0.1, f32::NAN, f32::INFINITY] {
            stroke.rendering.as_mut().unwrap().style.pen_size = Some(size);
            assert!(prepare(&stroke).is_none(), "{size:?}");
        }
        stroke.rendering.as_mut().unwrap().style.pen_size = Some(900.);
        assert_eq!(prepare(&stroke).unwrap().radii, vec![100.]);
    }
    #[test]
    fn unsupported_settings_and_unbounded_inputs_use_the_fallback() {
        let mut s = reference().stroke;
        s.points = vec![Point { x: 0., y: 0. }, Point { x: 10., y: 10. }];
        s.pressures = vec![0.5; 2];
        s.timestamps = vec![0, 8];
        assert!(prepare(&s).is_some());
        for tool in [0, 4] {
            s.rendering.as_mut().unwrap().tool_type_raw = tool;
            assert!(prepare(&s).is_none());
        }
        s.rendering.as_mut().unwrap().tool_type_raw = 2;
        s.rendering.as_mut().unwrap().advanced_settings = Some("17;0;100;".into());
        assert!(prepare(&s).is_none());
        s.rendering.as_mut().unwrap().advanced_settings = Some("18;0;100;".into());
        s.points[1].x = 1_000_000.;
        assert!(prepare(&s).is_none());
        s.points[1].x = f64::NAN;
        assert!(prepare(&s).is_none());
        s.points[1].x = 10.;
        s.pressures[0] = -1.01;
        assert!(prepare(&s).is_none());
        s.pressures[0] = 0.5;
        s.timestamps.clear();
        assert!(prepare(&s).is_none());
    }

    #[test]
    fn fixed_width_requires_a_verified_profile_and_saved_width() {
        let mut s = reference().stroke;
        s.points = vec![Point { x: 20., y: 30. }];
        s.pressures = vec![0.5];
        s.timestamps = vec![0];
        let r = s.rendering.as_mut().unwrap();
        r.properties.fixed_width = true;
        r.style.fixed_width = Some(3.);
        assert_eq!(prepare(&s).unwrap().radii, vec![1.5]);
        s.rendering.as_mut().unwrap().advanced_settings = Some("14;".into());
        assert_eq!(prepare(&s).unwrap().radii, vec![1.5]);
        for width in [
            None,
            Some(-1.),
            Some(f32::NAN),
            Some(f32::INFINITY),
            Some(1025.),
        ] {
            s.rendering.as_mut().unwrap().style.fixed_width = width;
            assert!(prepare(&s).is_none(), "{width:?}");
        }
    }

    #[test]
    fn v17_rainbow_right_angle_matches_native_rgb_words_and_terminal_reuse() {
        // V6 receipt SHA-256 0bc7651bfc571eac372d061e728986e2416d39c8d1ce8397a795743bc7721198;
        // archive 05e3d22aa91fc3c1a975f46c84890622abd63719; Fountain DSO
        // SHA-256 1fed4e071caffcb88a4df4ca345b6df5e3374852c9ef18044793fc2e340ac6ec.
        const NATIVE_RGB: [[u32; 3]; 48] = [
            [0x3f46c6c7, 0x3e008081, 0x3e28a8a9],
            [0x3ddc6182, 0x3e008081, 0x3f5564c1],
            [0x3e53b4fb, 0x3f3fde95, 0x3e8244d8],
            [0x3f1e37d1, 0x3eed9169, 0x3d808081],
            [0x3f0c4edf, 0x3e008081, 0x3ec94424],
            [0x3dd73da1, 0x3e9e37ce, 0x3f31e51a],
            [0x3e8ef55b, 0x3f678b63, 0x3d808081],
            [0x3f4b319b, 0x3e714321, 0x3d808081],
            [0x3ea3ade0, 0x3e008081, 0x3f1f1a01],
            [0x3e21975b, 0x3f0993d3, 0x3eeeda5c],
            [0x3ee8e8e3, 0x3f2d137e, 0x3d808081],
            [0x3f42999e, 0x3e008081, 0x3e395d4c],
            [0x3dbaf858, 0x3e008081, 0x3f5991e6],
            [0x3e578fe0, 0x3f440bb8, 0x3e73d525],
            [0x3f216e40, 0x3ee53715, 0x3d808081],
            [0x3f0821ac, 0x3e008081, 0x3ed19e8b],
            [0x3ddef384, 0x3ea6922e, 0x3f2db7ea],
            [0x3e956239, 0x3f635e39, 0x3d808081],
            [0x3f4e6804, 0x3e608e97, 0x3d808081],
            [0x3e9b53aa, 0x3e008081, 0x3f23471c],
            [0x3e257239, 0x3f0dc0ee, 0x3ee68026],
            [0x3eef55ae, 0x3f28e660, 0x3d808081],
            [0x3f3e6c8f, 0x3e008081, 0x3e4a1189],
            [0x3d998f15, 0x3e008081, 0x3f5dbf0e],
            [0x3e5b6ac9, 0x3f4838df, 0x3e632087],
            [0x3f24a4a4, 0x3edcdcdf, 0x3d808081],
            [0x3f03f49d, 0x3e008081, 0x3ed9f8a9],
            [0x3de6a929, 0x3eaeec4c, 0x3f298adb],
            [0x3e9bceee, 0x3f5f312a, 0x3d808081],
            [0x3f519e5e, 0x3e4fda5a, 0x3d808081],
            [0x3e92f955, 0x3e008081, 0x3f277446],
            [0x3e294d25, 0x3f11ee18, 0x3ede25d2],
            [0x3ef5c289, 0x3f24b939, 0x3d808081],
            [0x3f3a3f65, 0x3e008081, 0x3e5ac62e],
            [0x3d826ddc, 0x3e04ad72, 0x3f5fd5a5],
            [0x3e5f45b5, 0x3f4c6609, 0x3e526bdf],
            [0x3f27db13, 0x3ed4828b, 0x3d808081],
            [0x3eff8ee8, 0x3e008081, 0x3ee252fa],
            [0x3dee5efe, 0x3eb7469e, 0x3f255db2],
            [0x3ea23bcc, 0x3f5b0400, 0x3d808081],
            [0x3f54d4cc, 0x3e3f25bc, 0x3d808081],
            [0x3e8a9f37, 0x3e008081, 0x3f2ba155],
            [0x3e2d27f8, 0x3f161b27, 0x3ed5cbb4],
            [0x3efc2f3d, 0x3f208c2a, 0x3d808081],
            [0x3f361256, 0x3e008081, 0x3e6b7a6a],
            [0x3d8a2382, 0x3e1561af, 0x3f5ba896],
            [0x3e63206e, 0x3f5092fd, 0x3e41b80e],
            [0x3e63206e, 0x3f5092fd, 0x3e41b80e],
        ];
        let reference = reference();
        let mut stroke = reference.stroke;
        let case = reference
            .cases
            .into_iter()
            .find(|case| case.name == "right angle")
            .unwrap();
        stroke.pen_width = case.size;
        let rendering = stroke.rendering.as_mut().unwrap();
        rendering.style.pen_size = Some(case.size);
        rendering.style.initial_tolerance = Some(case.tolerance);
        rendering.properties.rainbow_effect = false;
        for (x, y, pressure, tilt, time) in case.samples {
            stroke.points.push(Point { x, y });
            stroke.pressures.push(pressure);
            stroke.tilts.push(tilt);
            stroke.timestamps.push(time);
        }
        let plain = prepare(&stroke).expect("fixture's ordinary V16 sampler");
        let mut profile_stroke = stroke.clone();
        profile_stroke
            .rendering
            .as_mut()
            .unwrap()
            .properties
            .rainbow_effect = true;
        let colored = prepare_v17_rainbow(
            &profile_stroke,
            V17Rainbow {
                palette_argb: &[0x40e0_2010, 0x8010_20e0, 0x7f40_f010],
                period: 3,
                offset: 0.125,
            },
        )
        .expect("explicit V17 host profile");
        assert_eq!(colored.sample_ends, [0, 7, 19, 28, 48]);
        assert_eq!(colored.points, plain.points);
        assert_eq!(colored.radii, plain.radii);
        let rgb = colored.rgb.expect("native per-stamp RGB");
        assert_eq!(rgb.len(), 48);
        assert_eq!(
            rgb.iter()
                .map(|channels| channels.map(f32::to_bits))
                .collect::<Vec<_>>(),
            NATIVE_RGB
        );
        assert_eq!(
            colored
                .rainbow_path_offsets
                .iter()
                .map(|offset| offset.to_bits())
                .collect::<Vec<_>>(),
            [0x40a4_0000, 0x4172_0000, 0x41b8_3edc, 0x4218_1f6e]
        );
        assert_eq!(
            colored.rainbow_final_offset.map(f32::to_bits),
            Some(0x4218_1f6e)
        );
        assert_eq!(
            rgb[46], rgb[47],
            "terminal endpoint retains previous selected RGB"
        );
    }

    #[test]
    fn v17_rainbow_single_tap_and_duplicate_zero_length_select_at_zero() {
        // Tiny native receipt SHA-256 72ce4cc1c35e335db4075c17ecff2eb72888da5c8410fa0a88db574a1a519d9e.
        let mut stroke = reference().stroke;
        stroke.points = vec![Point { x: 20., y: 30. }];
        stroke.pressures = vec![0.5];
        stroke.tilts = vec![0.];
        stroke.timestamps = vec![0];
        let rendering = stroke.rendering.as_mut().unwrap();
        rendering.properties.rainbow_effect = true;
        let profile = V17Rainbow {
            palette_argb: &[0x40e0_2010, 0x8010_20e0, 0x7f40_f010],
            period: 3,
            offset: 0.125,
        };

        let mut plain_stroke = stroke.clone();
        plain_stroke
            .rendering
            .as_mut()
            .unwrap()
            .properties
            .rainbow_effect = false;
        let plain = prepare(&plain_stroke).expect("ordinary single-point pass");
        let colored = prepare_v17_rainbow(&stroke, profile).expect("native V17 tap control");
        assert_eq!(colored.sample_ends, [1]);
        assert_eq!(colored.points, plain.points);
        assert_eq!(colored.radii, plain.radii);
        assert_eq!(
            colored.rgb.unwrap()[0].map(f32::to_bits),
            [0x3f46_c6c7, 0x3e00_8081, 0x3e28_a8a9]
        );

        // Duplicate samples do not enter drawLine; the native tap path selects
        // at zero and keeps the one terminal stamp in both modes.
        let mut duplicate = stroke;
        duplicate.points.push(Point { x: 20., y: 30. });
        duplicate.pressures.push(0.5);
        duplicate.tilts.push(0.);
        duplicate.timestamps.push(1);
        let mut ordinary_duplicate = duplicate.clone();
        ordinary_duplicate
            .rendering
            .as_mut()
            .unwrap()
            .properties
            .rainbow_effect = false;
        let ordinary = prepare(&ordinary_duplicate).expect("default zero-length terminal stamp");
        assert_eq!(ordinary.points.len(), 1);
        let colored = prepare_v17_rainbow(&duplicate, profile).expect("native zero-length tap");
        assert_eq!(colored.points.len(), 1);
        assert_eq!(
            colored.rgb.unwrap()[0].map(f32::to_bits),
            [0x3f46_c6c7, 0x3e00_8081, 0x3e28_a8a9,]
        );
    }

    #[test]
    fn v17_rainbow_rejects_invalid_or_unbounded_profiles_before_sampling() {
        let mut stroke = reference().stroke;
        stroke.points = vec![Point { x: 0., y: 0. }, Point { x: 8., y: 0. }];
        stroke.pressures = vec![0.5; 2];
        stroke.tilts = vec![0.; 2];
        stroke.timestamps = vec![0, 1];
        let rendering = stroke.rendering.as_mut().unwrap();
        rendering.properties.rainbow_effect = true;
        let palette = [0x40e0_2010, 0x8010_20e0, 0x7f40_f010];
        for (palette, period, offset) in [
            (&[][..], 3, 0.125),
            (&palette[..], 0, 0.125),
            (&palette[..], -3, 0.125),
            (&palette[..], 3, f32::NAN),
            (&palette[..], 3, -0.125),
        ] {
            assert!(
                prepare_v17_rainbow(
                    &stroke,
                    V17Rainbow {
                        palette_argb: palette,
                        period,
                        offset,
                    },
                )
                .is_none()
            );
        }
    }

    #[test]
    fn v17_rainbow_five_color_sector_uses_native_fma_remainder() {
        let palette = [0, 0, 0, 0, 0x00ff_0000];
        let state = RainbowState {
            palette_argb: &palette,
            period: 1.0,
            offset: f32::from_bits(0x3f37_9b1d),
            current_rgb: None,
            invalid: false,
            #[cfg(test)]
            path_offsets: Vec::new(),
        };

        // With native step=(1.0f64 / 5.0) as f32 and FMA remainder,
        // the selected red channel is 0x3f160790. The previous rounded
        // phase - index*step expression produced 0x3f16078f.
        assert_eq!(
            state.color_at(0.0).unwrap().map(f32::to_bits),
            [0x3f16_0790, 0, 0]
        );
    }

    #[test]
    fn v17_rainbow_skipped_samples_do_not_advance_or_reject_the_profile() {
        let reference = reference();
        let case = reference
            .cases
            .into_iter()
            .find(|case| case.name == "right angle")
            .unwrap();
        let mut stroke = reference.stroke;
        stroke.pen_width = case.size;
        let rendering = stroke.rendering.as_mut().unwrap();
        rendering.style.pen_size = Some(case.size);
        rendering.style.initial_tolerance = Some(1000.0);
        rendering.properties.rainbow_effect = true;
        for (x, y, pressure, tilt, time) in case.samples {
            stroke.points.push(Point { x, y });
            stroke.pressures.push(pressure);
            stroke.tilts.push(tilt);
            stroke.timestamps.push(time);
        }

        let prepared = prepare_v17_rainbow(
            &stroke,
            V17Rainbow {
                palette_argb: &[0x40e0_2010, 0x8010_20e0, 0x7f40_f010],
                period: 3,
                offset: 0.125,
            },
        )
        .expect("native skipped-line path remains supported");
        assert_eq!(prepared.points.len(), 1);
        assert_eq!(prepared.rainbow_final_offset, Some(0.125));
        assert!(prepared.rgb.is_some());
    }
}
