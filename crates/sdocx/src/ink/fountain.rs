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
}
struct Pass<'a> {
    size: f32,
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
            let width = self.history.width(time, self.size * 0.5);
            self.dot(self.previous, width * 0.5);
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
    if r.pen_name.as_deref() != Some("com.samsung.android.sdk.pen.pen.preload.FountainPen")
        || !matches!(r.advanced_settings.as_deref(), Some("14;" | "18;0;100;"))
        || !input_supported
        || !fixed_width_supported
        || r.properties.eraser
        || r.properties.straighten
        || r.properties.rainbow_effect
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
    if v14 {
        return Some(super::fountain_v14::prepare(s, size, tolerance));
    }
    let stylus = r.tool_type_raw == 2;
    let pressure_at = |i| if stylus { s.pressures[i] as f32 } else { 0.5 };
    let first = P::from(s.points[0]);
    let mut history = History::default();
    let mut ratios = [0.; 3];
    let mut ratio_count = 0;
    let mut output = None;
    for _ in 0..2 {
        let width = history.width(s.timestamps[0], (size * 0.5) * pressure_at(0).min(1.));
        let mut pass = Pass {
            size,
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
            dots: Dots {
                directions: None,
                points: vec![],
                radii: vec![],
                sample_ends: vec![0; s.points.len()],
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
            pass.dots.sample_ends[i] = pass.dots.points.len();
        }
        let last = s.points.len() - 1;
        // MotionEvent::GetEventTime returns the latest event time relative to
        // the MotionEvent constructor's base timestamp. ObjectStroke channel
        // timestamps remain absolute in the width-history entries.
        let terminal_event_time = s.timestamps[last] - s.timestamps[0];
        pass.end(s.points[last].into(), terminal_event_time);
        pass.dots.sample_ends[last] = pass.dots.points.len();
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
                    .chunks_exact(2)
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
                assert_eq!(
                    raw_size.max(0.4).min(800.).to_bits(),
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
                .chunks_exact(2)
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
}
