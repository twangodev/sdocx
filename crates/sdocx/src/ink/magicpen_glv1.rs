//! Source model and bounded NoCurve page preparation for MagicPen GLV1.
//!
//! Curved sampling remains regression evidence. Production admission covers
//! ordinary type-2 NoCurve sources with explicit saved settings; GLV2, SDK
//! deserialization and native per-view paint coverage remain separate.

use super::path::{P, Quad};
use std::ops::Range;

use crate::{BoundingBox, Point, Stroke};

const MAX_SOURCE_POINTS: usize = 100_000;
const MAX_OUTPUT_RECORDS: usize = 65_536;

pub(crate) struct PreparedMagicPenV1 {
    pub(crate) records: Vec<[u32; 3]>,
    pub(crate) points: Vec<Point>,
    pub(crate) radii: Vec<f32>,
    pub(crate) sample_ends: Vec<usize>,
    pub(crate) terminal_range: Range<usize>,
    pub(crate) bounds: BoundingBox,
    pub(crate) argb: u32,
}

pub(crate) fn prepare_source_v1(stroke: &Stroke) -> Option<PreparedMagicPenV1> {
    let rendering = stroke.rendering.as_ref()?;
    if rendering.pen_name.as_deref() != Some("com.samsung.android.sdk.pen.pen.preload.MagicPen")
        || !matches!(rendering.advanced_settings.as_deref(), None | Some("1;"))
        || rendering.tool_type_raw != 2
        // Model ApplyBinary restores omitted style fields 16/25 to zero;
        // its writer omits these fields when their native values are zero.
        || rendering.style.stroke_type_raw.unwrap_or(0) != 0
        || rendering.style.color_type_raw.unwrap_or(0) != 0
        || rendering.properties.replay_only
        || rendering.properties.eraser
        || rendering.properties.fixed_width
        || rendering.properties.top_layer_pen
        || rendering.properties.alpha_lock
        || rendering.properties.fixed_opacity
        || rendering.properties.rainbow_effect
        || rendering.properties.straighten
        || rendering.properties.reveal_mode
        || stroke.points.len() < 2
        || stroke.points.len() > MAX_SOURCE_POINTS
        || stroke.points.len() != stroke.pressures.len()
    {
        return None;
    }
    // Drawing selects GLV1 for null settings; ObjectStroke's omitted color
    // is opaque black. Keep those defaults local to native preparation.
    let argb = rendering.style.color_argb.unwrap_or(0xff00_0000);
    let size = rendering.style.pen_size?;
    if !size.is_finite()
        || stroke.points.iter().any(|point| {
            !point.x.is_finite()
                || !point.y.is_finite()
                || !(point.x as f32).is_finite()
                || !(point.y as f32).is_finite()
        })
        || stroke.pressures.iter().any(|pressure| {
            !pressure.is_finite()
                || !(*pressure as f32).is_finite()
                || !(0.0..=1.0).contains(&(*pressure as f32))
        })
    {
        return None;
    }

    // Bound worst-case source-derived sample growth before invoking the native
    // numeric model, which otherwise follows native repeat counts directly.
    let mut source_length = 0.0_f64;
    for pair in stroke.points.windows(2) {
        let dx = pair[1].x as f32 as f64 - pair[0].x as f32 as f64;
        let dy = pair[1].y as f32 as f64 - pair[0].y as f32 as f64;
        source_length += dx.hypot(dy);
    }
    let upper_records = source_length * 4.0 + stroke.points.len() as f64 * 8.0;
    if !upper_records.is_finite() || upper_records > MAX_OUTPUT_RECORDS as f64 {
        return None;
    }

    let points: Vec<[f32; 2]> = stroke
        .points
        .iter()
        .map(|point| [point.x as f32, point.y as f32])
        .collect();
    let pressure: Vec<f32> = stroke
        .pressures
        .iter()
        .map(|pressure| *pressure as f32)
        .collect();
    let emission = if rendering.properties.compressed {
        // Curved sampling remains regression evidence; page paint admission
        // currently covers NoCurve sources only.
        return None;
    } else {
        let emission = source_glv1_no_curve_trace(&points, &pressure, size);
        (
            emission.records,
            emission.sample_ends,
            emission.terminal_range,
        )
    };
    let (records, sample_ends, terminal_range) = emission;
    if records.is_empty()
        || records.len() > MAX_OUTPUT_RECORDS
        || sample_ends.len() != points.len()
        || sample_ends.first() != Some(&0)
        || sample_ends.windows(2).any(|pair| pair[0] > pair[1])
        || terminal_range.start > terminal_range.end
        || terminal_range.end != records.len()
        || sample_ends
            .last()
            .is_none_or(|&end| end > terminal_range.start && end != terminal_range.end)
    {
        return None;
    }
    let mut prepared_points = Vec::with_capacity(records.len());
    let mut radii = Vec::with_capacity(records.len());
    let mut bounds: Option<BoundingBox> = None;
    for [x, y, radius] in &records {
        let (x, y, radius) = (
            f32::from_bits(*x),
            f32::from_bits(*y),
            f32::from_bits(*radius),
        );
        if !x.is_finite() || !y.is_finite() || !radius.is_finite() || radius < 0.0 {
            return None;
        }
        prepared_points.push(Point {
            x: f64::from(x),
            y: f64::from(y),
        });
        radii.push(radius);
        let next = BoundingBox {
            x_min: f64::from(x - radius),
            y_min: f64::from(y - radius),
            x_max: f64::from(x + radius),
            y_max: f64::from(y + radius),
        };
        if let Some(current) = &mut bounds {
            current.x_min = current.x_min.min(next.x_min);
            current.y_min = current.y_min.min(next.y_min);
            current.x_max = current.x_max.max(next.x_max);
            current.y_max = current.y_max.max(next.y_max);
        } else {
            bounds = Some(next);
        }
    }
    Some(PreparedMagicPenV1 {
        records,
        points: prepared_points,
        radii,
        sample_ends,
        terminal_range,
        bounds: bounds?,
        argb,
    })
}

#[derive(Clone, Copy)]
struct FPoint {
    x: f32,
    y: f32,
}

impl FPoint {
    fn mid(self, other: Self) -> Self {
        Self {
            x: (self.x + other.x) * 0.5,
            y: (self.y + other.y) * 0.5,
        }
    }

    fn sub(self, other: Self) -> Self {
        Self {
            x: self.x - other.x,
            y: self.y - other.y,
        }
    }

    fn len(self) -> f32 {
        self.x.mul_add(self.x, self.y * self.y).sqrt()
    }
}

struct Cubic {
    points: [FPoint; 4],
    segments: Vec<(f32, u16)>,
    length: f32,
}

impl Cubic {
    fn new(points: [FPoint; 4]) -> Self {
        let mut path = Self {
            points,
            segments: Vec::new(),
            length: 0.0,
        };
        path.split(points, 0, 32767);
        path
    }

    fn split(&mut self, p: [FPoint; 4], t0: u16, t1: u16) {
        let line = |a: FPoint, b: FPoint, t: f32| FPoint {
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
                self.segments.push((next, t1));
            }
        }
    }

    fn at(&self, distance: f32) -> FPoint {
        let i = self
            .segments
            .partition_point(|&(end, _)| end < distance)
            .min(self.segments.len() - 1);
        let (start, t0) = if i == 0 {
            (0.0, 0)
        } else {
            self.segments[i - 1]
        };
        let (end, t1) = self.segments[i];
        let scale = f32::from_bits(0x3800_00fd);
        let t0 = t0 as f32 * scale;
        let t1 = t1 as f32 * scale;
        let t = t0 + (distance - start) * (t1 - t0) / (end - start);
        let eval = |a: f32, b: f32, c: f32, d: f32| {
            if t == 0.0 {
                return a;
            }
            let x = (b - a).mul_add(t, a);
            let y = (c - b).mul_add(t, b);
            let z = (d - c).mul_add(t, c);
            let q = (y - x).mul_add(t, x);
            let r = (z - y).mul_add(t, y);
            (r - q).mul_add(t, q)
        };
        FPoint {
            x: eval(
                self.points[0].x,
                self.points[1].x,
                self.points[2].x,
                self.points[3].x,
            ),
            y: eval(
                self.points[0].y,
                self.points[1].y,
                self.points[2].y,
                self.points[3].y,
            ),
        }
    }
}

#[derive(Clone, Copy)]
enum SourceCurve {
    Quad([FPoint; 3]),
    Cubic([FPoint; 4]),
}

impl SourceCurve {
    fn length(self) -> f32 {
        match self {
            Self::Quad([a, b, c]) => quad(a, b, c).length,
            Self::Cubic(p) => Cubic::new(p).length,
        }
    }

    fn at(self, distance: f32) -> FPoint {
        match self {
            Self::Quad([a, b, c]) => {
                let p = quad(a, b, c).at(distance);
                FPoint { x: p.x, y: p.y }
            }
            Self::Cubic(p) => Cubic::new(p).at(distance),
        }
    }
}

fn quad(a: FPoint, b: FPoint, c: FPoint) -> Quad {
    Quad::new(
        P { x: a.x, y: a.y },
        P { x: b.x, y: b.y },
        P { x: c.x, y: c.y },
    )
}

fn speed_factor(distance: f32) -> f32 {
    if distance < 4.2 {
        1.0
    } else if distance < 9.5 {
        0.6
    } else if distance >= 35.0 {
        0.2
    } else {
        ((distance - 9.5) / -25.5).mul_add(0.4, 0.4) + 0.2
    }
}

fn width_target(old: f32, size: f32, pressure: f32, factor: f32) -> f32 {
    let adjustment = if factor < 0.35 {
        pressure
    } else if factor < 0.7 {
        factor.mul_add(0.5, pressure * 1.5) * 0.5
    } else {
        (factor + pressure) * 0.5
    };
    let candidate = size.mul_add(adjustment, old) * 0.5;
    let delta = candidate - old;
    let limited = if delta > 2.0 {
        old + 2.0
    } else if delta < -2.0 {
        old - 2.0
    } else if candidate < old && old < 2.0 {
        old - 0.5
    } else {
        candidate
    };
    limited.max(size / 10.0)
}

fn terminal_width_target(old: f32, size: f32, pressure: f32, factor: f32) -> f32 {
    let candidate = if factor < 0.35 {
        size.mul_add(pressure, old) * 0.5
    } else if factor < 0.7 {
        let adjustment = factor.mul_add(0.5, pressure * 1.5);
        size.mul_add(adjustment * 0.5, old) * 0.5
    } else {
        let adjustment = pressure + factor;
        size.mul_add(adjustment * 0.5, old) * 0.5
    };
    end_pen_width_target(old, candidate, size)
}

fn end_pen_width_target(old: f32, candidate: f32, size: f32) -> f32 {
    let delta = candidate - old;
    let target = if delta > 6.0 {
        old + 6.0
    } else if delta < -6.0 {
        old - 6.0
    } else if candidate < old && old < 6.0 {
        old - 0.5
    } else {
        candidate
    };
    target.max(size / 10.0)
}

fn repeat_index(length: f32, interval: f32) -> usize {
    let scaled = if interval <= 5.0 {
        length * 0.5
    } else if interval <= 10.0 {
        length / 3.0
    } else if interval <= 15.0 {
        length * 0.25
    } else if interval <= 20.0 {
        length / 5.0
    } else if interval <= 30.0 {
        length / 6.0
    } else if interval <= 40.0 {
        length / 7.0
    } else if interval <= 55.0 {
        length * 0.125
    } else {
        length / 10.0
    };
    (scaled as usize).max(2) + 1
}

fn admit_source_events(
    points: &[[f32; 2]],
    pressure: &[f32],
) -> (Vec<[f32; 2]>, Vec<f32>, Vec<usize>) {
    assert_eq!(points.len(), pressure.len());
    if points.is_empty() {
        return (Vec::new(), Vec::new(), Vec::new());
    }
    let mut admitted_points = vec![points[0]];
    let mut admitted_pressure = vec![pressure[0]];
    let mut source_indices = vec![0];
    for (source_index, (point, &point_pressure)) in points.iter().zip(pressure).enumerate().skip(1)
    {
        let previous = *admitted_points.last().unwrap();
        // Native source admission skips an event only when both axes are below 1.5.
        if (point[0] - previous[0]).abs() < 1.5 && (point[1] - previous[1]).abs() < 1.5 {
            continue;
        }
        admitted_points.push(*point);
        admitted_pressure.push(point_pressure);
        source_indices.push(source_index);
    }
    (admitted_points, admitted_pressure, source_indices)
}

fn source_point(p: [f32; 2]) -> FPoint {
    FPoint { x: p[0], y: p[1] }
}

// Curved replay metadata is consumed by native-oracle tests until admission.
#[cfg_attr(not(test), allow(dead_code))]
pub struct CurveEmission {
    pub records: Vec<[u32; 3]>,
    pub source_event_ranges: Vec<Range<usize>>,
    pub terminal_range: Range<usize>,
}

fn source_glv1_curve(points: &[[f32; 2]], pressure: &[f32], source_size: f32) -> Vec<[u32; 3]> {
    source_glv1_curve_trace(points, pressure, source_size).records
}

fn source_glv1_curve_trace(
    points: &[[f32; 2]],
    pressure: &[f32],
    source_size: f32,
) -> CurveEmission {
    assert!(points.len() >= 2 && points.len() == pressure.len());
    let raw_point_count = points.len();
    let raw_terminal = *points.last().unwrap();
    let raw_terminal_pressure = *pressure.last().unwrap();
    let (points, pressure, source_indices) = admit_source_events(points, pressure);
    if points.len() == 1 {
        // With no admitted move, native endPen emits a dot at the retained
        // initial point using the raw terminal pressure.
        let size = source_size.clamp(0.4, 800.0);
        let width = size * raw_terminal_pressure.mul_add(0.5, 0.5);
        return CurveEmission {
            records: vec![[
                points[0][0].to_bits(),
                points[0][1].to_bits(),
                (width * 0.5 + 0.5).to_bits(),
            ]],
            source_event_ranges: vec![0..0; raw_point_count],
            terminal_range: 0..1,
        };
    }
    if points.is_empty() {
        return CurveEmission {
            records: Vec::new(),
            source_event_ranges: vec![0..0; raw_point_count],
            terminal_range: 0..0,
        };
    }
    let raw_terminal_rejected = source_indices.last().copied() != Some(raw_point_count - 1);
    let size = source_size.clamp(0.4, 800.0);
    let p: Vec<_> = points.iter().copied().map(source_point).collect();
    let mut paths: Vec<(SourceCurve, f32, f32, bool, bool, Option<usize>)> = Vec::new();
    let mut width = size * pressure[1];
    let mut last_factor = 1.0_f32;
    if points.len() == 2 {
        if raw_terminal_rejected {
            // If the final raw event is rejected by redrawLine, native
            // endPen still closes the path at that raw endpoint and uses its
            // pressure while retaining the accepted point as control.
            let end = source_point(raw_terminal);
            let candidate = size.mul_add(raw_terminal_pressure.mul_add(0.25, 0.25), width * 0.5);
            let terminal = end_pen_width_target(width, candidate, size);
            paths.push((
                SourceCurve::Quad([p[0], p[1], end]),
                width,
                terminal,
                true,
                true,
                None,
            ));
        } else {
            paths.push((
                SourceCurve::Quad([p[0], p[1], p[1]]),
                width,
                width,
                true,
                true,
                None,
            ));
        }
    } else if points.len() == 3 {
        let distance = p[1].sub(p[2]).len();
        if distance < 1.5 {
            width = size * pressure[2];
            paths.push((
                SourceCurve::Quad([p[0], p[2], p[2]]),
                width,
                width,
                true,
                true,
                None,
            ));
        } else {
            let c2 = p[1].mid(p[2]);
            let candidate = size.mul_add(pressure[2].mul_add(0.25, 0.25), width * 0.5);
            let terminal = end_pen_width_target(width, candidate, size);
            paths.push((
                SourceCurve::Cubic([p[0], p[1], c2, p[2]]),
                width,
                terminal,
                true,
                true,
                None,
            ));
        }
    } else {
        let mut previous_end = p[0];
        let mut control = p[1];
        let mut i = 3;
        while i < p.len() {
            let factor = speed_factor(p[i - 1].sub(p[i]).len());
            let target = width_target(width, size, pressure[i], factor);
            let end = control.mid(p[i]);
            paths.push((
                SourceCurve::Quad([previous_end, control, end]),
                width,
                target,
                paths.is_empty(),
                false,
                Some(source_indices[i]),
            ));
            width = target;
            previous_end = end;
            control = p[i];
            last_factor = factor;
            i += 2;
        }
        let final_index = p.len() - 1;
        let terminal = terminal_width_target(width, size, raw_terminal_pressure, last_factor);
        let terminal_point = source_point(raw_terminal);
        if final_index % 2 == 0 {
            paths.push((
                SourceCurve::Cubic([
                    previous_end,
                    control,
                    control.mid(terminal_point),
                    terminal_point,
                ]),
                width,
                terminal,
                false,
                true,
                None,
            ));
        } else {
            paths.push((
                SourceCurve::Quad([previous_end, control, terminal_point]),
                width,
                terminal,
                false,
                true,
                None,
            ));
        }
    }

    let mut output = Vec::new();
    let mut source_event_ranges = vec![None; raw_point_count];
    let mut terminal_range = 0..0;
    for (path, from_width, to_width, _first, terminal, source_event_index) in paths {
        let path_start = output.len();
        let length = path.length();
        let repeat = repeat_index(length, from_width.min(to_width));
        let step = length / repeat as f32;
        let radius_step = (to_width * 0.5 - from_width * 0.5) / repeat as f32;
        let width_step = (to_width - from_width) / repeat as f32;
        let mut distance = 0.0_f32;
        let mut radius = from_width * 0.5;
        let mut current_width = from_width;
        for sample in 0..=repeat {
            let position = if sample == repeat
                && !raw_terminal_rejected
                && !matches!(path, SourceCurve::Cubic(_))
            {
                match path {
                    SourceCurve::Quad([_, _, end]) => end,
                    SourceCurve::Cubic([_, _, _, end]) => end,
                }
            } else if sample == repeat
                && terminal
                && (raw_terminal_rejected
                    || (points.len() > 3 && matches!(path, SourceCurve::Cubic(_))))
            {
                path.at(length)
            } else {
                path.at(distance)
            };
            // Native line callbacks are half-open and the end callback is
            // closed: each path owns its start, while only the final path
            // owns its endpoint. This also preserves the raw rejected final
            // endpoint supplied independently to endPen.
            if terminal || sample < repeat {
                let sample_width = if sample == repeat && !terminal {
                    to_width
                } else {
                    current_width
                };
                let stored = if points.len() == 3 {
                    radius + 0.5
                } else {
                    sample_width * 0.5 + 0.5
                };
                output.push([position.x.to_bits(), position.y.to_bits(), stored.to_bits()]);
            }
            distance += step;
            radius += radius_step;
            current_width += width_step;
        }
        let path_range = path_start..output.len();
        if terminal {
            terminal_range = path_range;
        } else if let Some(source_index) = source_event_index {
            source_event_ranges[source_index] = Some(path_range);
        }
    }
    let mut source_event_cursor = 0;
    let source_event_ranges = source_event_ranges
        .into_iter()
        .enumerate()
        .map(|(index, range)| {
            if index == 0 {
                return 0..0;
            }
            if let Some(range) = range {
                assert_eq!(range.start, source_event_cursor);
                source_event_cursor = range.end;
                range
            } else {
                source_event_cursor..source_event_cursor
            }
        })
        .collect();
    CurveEmission {
        records: output,
        source_event_ranges,
        terminal_range,
    }
}

struct NoCurveEmission {
    /// Complete XYZ float-bit records appended to RTV1.
    records: Vec<[u32; 3]>,
    /// Cumulative record counts after each raw source event; starts at zero.
    /// Rejected events repeat the previous count.
    sample_ends: Vec<usize>,
    /// Record range emitted by redrawEndPen_NoCurve.
    terminal_range: Range<usize>,
}

fn emit_no_curve_segment(
    output: &mut Vec<[u32; 3]>,
    start: [f32; 2],
    end: [f32; 2],
    from_width: f32,
    to_width: f32,
) {
    let a = source_point(start);
    let b = source_point(end);
    let delta = b.sub(a);
    let length = delta.len();
    let repeat = repeat_index(length, from_width.min(to_width));
    let distance_step = length / repeat as f32;
    let radius_step = (to_width * 0.5 - from_width * 0.5) / repeat as f32;
    let mut distance = 0.0_f32;
    let mut radius = from_width * 0.5;
    for _ in 0..repeat {
        let t_max = 32767.0_f32 * f32::from_bits(0x3800_00fd);
        let t = (distance * t_max) / length;
        let position = FPoint {
            x: delta.x.mul_add(t, a.x),
            y: delta.y.mul_add(t, a.y),
        };
        output.push([
            position.x.to_bits(),
            position.y.to_bits(),
            (radius + 0.5).to_bits(),
        ]);
        distance += distance_step;
        radius += radius_step;
    }
}

fn source_glv1_no_curve_trace(
    points: &[[f32; 2]],
    pressure: &[f32],
    source_size: f32,
) -> NoCurveEmission {
    assert!(points.len() >= 2 && points.len() == pressure.len());
    let size = source_size.clamp(0.4, 800.0);
    let mut output = Vec::new();
    let mut sample_ends = vec![0];
    let mut accepted = points[0];
    // The first visible width is determined by the first moved point.
    let mut width = 0.0_f32;
    let mut started = false;
    for edge in 1..points.len() {
        let candidate = points[edge];
        let dx = (candidate[0] - accepted[0]).abs();
        let dy = (candidate[1] - accepted[1]).abs();
        if dx >= 1.5 || dy >= 1.5 {
            let target = size * pressure[edge].min(1.0);
            if !started {
                width = target;
                started = true;
            }
            emit_no_curve_segment(&mut output, accepted, candidate, width, target);
            accepted = candidate;
            width = target;
        }
        sample_ends.push(output.len());
    }

    let terminal_start = output.len();
    let terminal = *points.last().unwrap();
    if !started {
        // Native endPen's pending-init branch emits one point at the original
        // start coordinate, using terminal pressure for size * pressure * 0.5.
        let radius = (size * pressure.last().unwrap().min(1.0)) * 0.5;
        output.push([
            points[0][0].to_bits(),
            points[0][1].to_bits(),
            (radius + 0.5).to_bits(),
        ]);
    } else if terminal != accepted {
        let target = size * pressure.last().unwrap().min(1.0);
        emit_no_curve_segment(&mut output, accepted, terminal, width, target);
    }
    let terminal_end = output.len();
    NoCurveEmission {
        records: output,
        sample_ends,
        terminal_range: terminal_start..terminal_end,
    }
}

fn source_glv1_no_curve(points: &[[f32; 2]], pressure: &[f32], source_size: f32) -> Vec<[u32; 3]> {
    source_glv1_no_curve_trace(points, pressure, source_size).records
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn f32_bits(word: &str) -> f32 {
        f32::from_bits(u32::from_str_radix(word, 16).unwrap())
    }

    fn fixture_bits(value: &Value) -> Vec<u32> {
        value
            .as_array()
            .unwrap()
            .iter()
            .map(|word| u32::from_str_radix(word.as_str().unwrap(), 16).unwrap())
            .collect()
    }

    fn le_hex_word(value: &Value) -> u32 {
        let text = value.as_str().unwrap();
        assert_eq!(text.len(), 8);
        let bytes: Vec<u8> = (0..8)
            .step_by(2)
            .map(|index| u8::from_str_radix(&text[index..index + 2], 16).unwrap())
            .collect();
        u32::from_le_bytes(bytes.try_into().unwrap())
    }

    fn le_hex_f32(value: &Value) -> f32 {
        f32::from_bits(le_hex_word(value))
    }

    fn range_words(value: &Value) -> Range<usize> {
        let range = value.as_array().unwrap();
        assert_eq!(range.len(), 2);
        range[0].as_u64().unwrap() as usize..range[1].as_u64().unwrap() as usize
    }

    #[test]
    fn source_model_matches_native_glv1_getters_and_complete_rtv1_vectors() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/magicpen-glv1-source-native.json"
        ))
        .unwrap();
        assert_eq!(fixture["schema"], "sdocx-magicpen-glv1-source-native-v1");
        let cases = fixture["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 35);
        let mut seen = std::collections::BTreeSet::new();
        for case in cases {
            let id = case["id"].as_str().unwrap();
            assert!(seen.insert(id), "duplicate case {id}");
            let input = &case["input"];
            assert_eq!(input["tool_type"], 2, "{id}");
            assert_eq!(input["stroke_type"], 0, "{id}");
            assert_eq!(input["advanced_setting"], "1;", "{id}");
            let point_words = fixture_bits(&input["points_f32_u32_hex"]);
            let pressure_words = fixture_bits(&input["pressure_f32_u32_hex"]);
            assert_eq!(point_words.len(), pressure_words.len() * 2, "{id}");
            let points: Vec<_> = point_words
                .chunks_exact(2)
                .map(|xy| [f32::from_bits(xy[0]), f32::from_bits(xy[1])])
                .collect();
            let pressure: Vec<_> = pressure_words.into_iter().map(f32::from_bits).collect();
            let size = f32_bits(input["size_f32_u32_hex"].as_str().unwrap());
            let predicted = if input["curve_enabled"].as_bool().unwrap() {
                source_glv1_curve(&points, &pressure, size)
            } else {
                source_glv1_no_curve(&points, &pressure, size)
            };
            let expected = fixture_bits(&case["native"]["xyz_f32_u32_hex"]);
            let actual: Vec<u32> = predicted.into_iter().flatten().collect();
            assert_eq!(actual, expected, "{id}");
            assert_eq!(
                case["native"]["record_count"].as_u64().unwrap() as usize * 3,
                expected.len(),
                "{id}"
            );
        }
    }

    #[test]
    fn curved_source_events_and_rejected_terminals_match_native_stream_and_ranges() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/magicpen-curved-source-events-native.json"
        ))
        .unwrap();
        assert_eq!(
            fixture["schema"],
            "sdocx-magicpen-glv1-curved-source-events-v1"
        );
        let cases = fixture["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 33);
        let case_ids: std::collections::BTreeSet<_> = cases
            .iter()
            .map(|case| case["id"].as_str().unwrap())
            .collect();
        assert!(case_ids.contains("curve_terminal_factor_02_pressure_zero"));
        assert!(case_ids.contains("curve_terminal_factor_02_pressure_one"));
        let mut seen = std::collections::BTreeSet::new();
        for case in cases {
            let id = case["id"].as_str().unwrap();
            assert!(seen.insert(id), "duplicate case {id}");
            let input = &case["input"];
            assert_eq!(input["curve_enabled"], true, "{id}");
            assert_eq!(input["tool_type"], 2, "{id}");
            assert_eq!(input["stroke_type"], 0, "{id}");
            assert_eq!(input["advanced_setting"], "1;", "{id}");
            let point_words = fixture_bits(&input["points_f32_u32_hex"]);
            let pressure_words = fixture_bits(&input["pressure_f32_u32_hex"]);
            let points: Vec<[f32; 2]> = point_words
                .chunks_exact(2)
                .map(|xy| [f32::from_bits(xy[0]), f32::from_bits(xy[1])])
                .collect();
            let pressure: Vec<f32> = pressure_words.iter().copied().map(f32::from_bits).collect();
            assert_eq!(points.len(), pressure.len(), "{id}");
            let size = f32::from_bits(
                fixture_bits(&Value::Array(vec![input["size_f32_u32_hex"].clone()]))[0],
            );
            let emission = source_glv1_curve_trace(&points, &pressure, size);

            let capture = &case["capture"];
            let getters = &capture["source_getters"];
            assert_eq!(
                getters["point_count"].as_u64().unwrap() as usize,
                points.len(),
                "{id}"
            );
            assert_eq!(
                fixture_bits(&getters["arrays"]["points_f32_bits"]),
                point_words,
                "{id}"
            );
            assert_eq!(
                fixture_bits(&getters["arrays"]["pressure_f32_bits"]),
                pressure_words,
                "{id}"
            );
            assert_eq!(
                getters["arrays"]["timestamps_i32"], input["timestamps_i32"],
                "{id}"
            );
            assert_eq!(
                fixture_bits(&getters["arrays"]["tilt_f32_bits"]),
                vec![0; points.len()],
                "{id}"
            );
            assert_eq!(
                fixture_bits(&getters["arrays"]["orientation_f32_bits"]),
                vec![0; points.len()],
                "{id}"
            );
            assert_eq!(getters["metadata"]["tool_type"], 2, "{id}");
            assert_eq!(getters["metadata"]["stroke_type"], 0, "{id}");
            assert_eq!(getters["metadata"]["curve_enabled"], true, "{id}");
            assert_eq!(getters["selected_drawable_class"], "GLV1", "{id}");

            let phase = &capture["redraw_phase"];
            assert_eq!(phase["phase_index"], 0, "{id}");
            assert!(
                phase["entry"].as_str().unwrap().contains("GLV1::RedrawPen"),
                "{id}"
            );
            let adds = &capture["native_addpoint"];
            let add_count = adds["record_count"].as_u64().unwrap() as usize;
            assert_eq!(range_words(&phase["addpoint_range"]), 0..add_count, "{id}");

            let trace = &adds["source_event_trace"];
            let lines = trace["curved_line_calls"].as_array().unwrap();
            let terminals = trace["curved_end_calls"].as_array().unwrap();
            assert_eq!(lines.len(), points.len() - 1, "{id}");
            assert_eq!(terminals.len(), 1, "{id}");
            assert_eq!(emission.source_event_ranges.len(), points.len(), "{id}");
            for (ordinal, event) in lines.iter().enumerate() {
                let source_index = ordinal + 1;
                assert_eq!(event["phase_index"], 0, "{id} event {source_index}");
                let args = fixture_bits(&event["args_f32_u32_hex"]);
                assert_eq!(
                    args,
                    vec![
                        point_words[source_index * 2],
                        point_words[source_index * 2 + 1],
                        pressure_words[source_index]
                    ],
                    "{id} event {source_index} args"
                );
                let native_range = range_words(&event["addpoint_range"]);
                assert_eq!(
                    native_range, emission.source_event_ranges[source_index],
                    "{id} event {source_index} range"
                );
                assert_eq!(
                    event["addpoint_count"].as_u64().unwrap() as usize,
                    native_range.end - native_range.start,
                    "{id} event {source_index} count"
                );
            }
            let terminal = &terminals[0];
            assert_eq!(terminal["phase_index"], 0, "{id} terminal");
            assert_eq!(
                fixture_bits(&terminal["args_f32_u32_hex"]),
                vec![
                    *point_words.get(point_words.len() - 2).unwrap(),
                    *point_words.last().unwrap(),
                    *pressure_words.last().unwrap()
                ],
                "{id} raw terminal args"
            );
            let terminal_range = range_words(&terminal["addpoint_range"]);
            assert_eq!(
                terminal_range, emission.terminal_range,
                "{id} terminal range"
            );
            assert_eq!(
                terminal["addpoint_count"].as_u64().unwrap() as usize,
                terminal_range.end - terminal_range.start,
                "{id} terminal count"
            );
            if id == "tool2_3_duplicate_terminal_same_xy_pressure" {
                let rejected_duplicate = lines.last().unwrap();
                assert_eq!(
                    rejected_duplicate["addpoint_count"].as_u64().unwrap(),
                    0,
                    "{id} duplicate line rejection"
                );
                assert_eq!(
                    rejected_duplicate["before"]["last_x_plus296_f32"]
                        .as_f64()
                        .unwrap() as f32,
                    points[1][0],
                    "{id} retained x"
                );
                assert_eq!(
                    rejected_duplicate["before"]["last_y_plus300_f32"]
                        .as_f64()
                        .unwrap() as f32,
                    points[1][1],
                    "{id} retained y"
                );
                assert_eq!(
                    terminal["args_f32_u32_hex"][2],
                    input["pressure_f32_u32_hex"].as_array().unwrap()[2],
                    "{id} terminal raw pressure"
                );
                assert_eq!(
                    emission.source_event_ranges[2].start, emission.source_event_ranges[2].end,
                    "{id} rejected source range"
                );
            }

            let expected = fixture_bits(&case["native"]["xyz_f32_u32_hex"]);
            let actual: Vec<u32> = emission.records.iter().flatten().copied().collect();
            assert_eq!(actual, expected, "{id} complete native stream");
            assert_eq!(
                case["native"]["record_count"].as_u64().unwrap() as usize,
                emission.records.len(),
                "{id} record count"
            );
        }
    }

    #[test]
    fn nocurve_source_events_match_native_line_terminal_ranges_and_complete_vectors() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/magicpen-nocurve-source-events-native.json"
        ))
        .unwrap();
        let baseline: Value = serde_json::from_str(include_str!(
            "../../../../conformance/magicpen-glv1-source-native.json"
        ))
        .unwrap();
        assert_eq!(
            fixture["schema"],
            "sdocx-magicpen-glv1-nocurve-source-events-native-v1"
        );
        let cases = fixture["cases"].as_array().unwrap();
        let baseline_cases = baseline["cases"].as_array().unwrap();
        assert_eq!(baseline_cases.len(), 35);
        assert_eq!(cases.len(), 53);
        assert_eq!(
            &cases[..35],
            baseline_cases.as_slice(),
            "baseline prefix changed"
        );

        let new_cases = &cases[35..];
        assert_eq!(new_cases.len(), 18);
        let mut seen = std::collections::BTreeSet::new();
        for case in new_cases {
            let id = case["id"].as_str().unwrap();
            assert!(seen.insert(id), "duplicate NoCurve case {id}");
            let input = &case["input"];
            assert_eq!(input["curve_enabled"], false, "{id}");
            assert_eq!(input["tool_type"], 2, "{id}");
            assert_eq!(input["stroke_type"], 0, "{id}");
            assert_eq!(input["advanced_setting"], "1;", "{id}");

            let point_words = fixture_bits(&input["points_f32_u32_hex"]);
            let pressure_words = fixture_bits(&input["pressure_f32_u32_hex"]);
            let points: Vec<[f32; 2]> = point_words
                .chunks_exact(2)
                .map(|xy| [f32::from_bits(xy[0]), f32::from_bits(xy[1])])
                .collect();
            let pressure: Vec<f32> = pressure_words.iter().copied().map(f32::from_bits).collect();
            let size = f32_bits(input["size_f32_u32_hex"].as_str().unwrap());

            let capture = &case["capture"];
            let getters = &capture["source_getters"];
            let source_arrays = &getters["arrays"];
            assert_eq!(
                getters["point_count"].as_u64().unwrap() as usize,
                points.len(),
                "{id}"
            );
            assert_eq!(
                fixture_bits(&source_arrays["points_f32_bits"]),
                point_words,
                "{id}"
            );
            assert_eq!(
                fixture_bits(&source_arrays["pressure_f32_bits"]),
                pressure_words,
                "{id}"
            );
            assert_eq!(
                source_arrays["timestamps_i32"], input["timestamps_i32"],
                "{id}"
            );
            assert_eq!(
                fixture_bits(&source_arrays["tilt_f32_bits"]),
                vec![0; points.len()],
                "{id}"
            );
            assert_eq!(
                fixture_bits(&source_arrays["orientation_f32_bits"]),
                vec![0; points.len()],
                "{id}"
            );
            assert_eq!(getters["metadata"]["tool_type"], 2, "{id}");
            assert_eq!(getters["metadata"]["stroke_type"], 0, "{id}");
            assert_eq!(getters["metadata"]["curve_enabled"], false, "{id}");
            assert_eq!(getters["selected_drawable_class"], "GLV1", "{id}");

            let redraw_phases = capture["redraw_phases"].as_array().unwrap();
            assert_eq!(redraw_phases.len(), 1, "{id}");
            let phase = &redraw_phases[0];
            assert_eq!(phase["phase_index"], 0, "{id}");
            assert!(
                phase["entry"].as_str().unwrap().contains("GLV1::RedrawPen"),
                "{id}"
            );
            let native_add = &capture["native_addpoint"];
            let add_count = native_add["record_count"].as_u64().unwrap() as usize;
            assert_eq!(range_words(&phase["addpoint_range"]), 0..add_count, "{id}");

            let trace = &native_add["source_event_trace"];
            let line_calls = trace["no_curve_line_calls"].as_array().unwrap();
            let terminal_calls = trace["no_curve_end_calls"].as_array().unwrap();
            assert_eq!(line_calls.len(), points.len() - 1, "{id}");
            assert_eq!(terminal_calls.len(), 1, "{id}");

            let emission = source_glv1_no_curve_trace(&points, &pressure, size);
            assert_eq!(emission.sample_ends.len(), points.len(), "{id}");
            assert_eq!(emission.sample_ends[0], 0, "{id}");
            let mut accepted = points[0];
            for (edge, event) in line_calls.iter().enumerate() {
                assert_eq!(event["phase_index"], 0, "{id} edge {edge}");
                let event_args = event["args_f32_u32_hex"].as_array().unwrap();
                let event_x = le_hex_f32(&event_args[0]);
                let event_y = le_hex_f32(&event_args[1]);
                let event_pressure = le_hex_f32(&event_args[2]);
                assert_eq!(
                    event_x.to_bits(),
                    points[edge + 1][0].to_bits(),
                    "{id} edge {edge} x"
                );
                assert_eq!(
                    event_y.to_bits(),
                    points[edge + 1][1].to_bits(),
                    "{id} edge {edge} y"
                );
                assert_eq!(
                    event_pressure.to_bits(),
                    pressure[edge + 1].min(1.0).to_bits(),
                    "{id} edge {edge} pressure"
                );
                assert_eq!(
                    event["before"]["last_x_plus296_f32"].as_f64().unwrap() as f32,
                    accepted[0],
                    "{id} edge {edge} prior x"
                );
                assert_eq!(
                    event["before"]["last_y_plus300_f32"].as_f64().unwrap() as f32,
                    accepted[1],
                    "{id} edge {edge} prior y"
                );
                let admitted =
                    (event_x - accepted[0]).abs() >= 1.5 || (event_y - accepted[1]).abs() >= 1.5;
                assert_eq!(event["threshold_admitted"], admitted, "{id} edge {edge}");
                if admitted {
                    accepted = points[edge + 1];
                }
                let native_range = range_words(&event["addpoint_range"]);
                assert_eq!(
                    native_range.end,
                    emission.sample_ends[edge + 1],
                    "{id} edge {edge} sample end"
                );
                assert_eq!(
                    native_range.start, emission.sample_ends[edge],
                    "{id} edge {edge} sample start"
                );
                assert_eq!(
                    event["addpoint_count"].as_u64().unwrap() as usize,
                    native_range.end - native_range.start,
                    "{id} edge {edge} count"
                );
            }

            let terminal = &terminal_calls[0];
            assert_eq!(terminal["phase_index"], 0, "{id} terminal");
            let terminal_args = terminal["args_f32_u32_hex"].as_array().unwrap();
            assert_eq!(
                le_hex_f32(&terminal_args[0]).to_bits(),
                points.last().unwrap()[0].to_bits(),
                "{id} terminal x"
            );
            assert_eq!(
                le_hex_f32(&terminal_args[1]).to_bits(),
                points.last().unwrap()[1].to_bits(),
                "{id} terminal y"
            );
            assert_eq!(
                le_hex_f32(&terminal_args[2]).to_bits(),
                pressure.last().unwrap().min(1.0).to_bits(),
                "{id} terminal pressure"
            );
            assert_eq!(
                terminal["before"]["last_x_plus296_f32"].as_f64().unwrap() as f32,
                accepted[0],
                "{id} terminal prior x"
            );
            assert_eq!(
                terminal["before"]["last_y_plus300_f32"].as_f64().unwrap() as f32,
                accepted[1],
                "{id} terminal prior y"
            );
            let native_terminal_range = range_words(&terminal["addpoint_range"]);
            assert_eq!(
                native_terminal_range, emission.terminal_range,
                "{id} terminal range"
            );
            assert_eq!(
                terminal["addpoint_count"].as_u64().unwrap() as usize,
                native_terminal_range.end - native_terminal_range.start,
                "{id} terminal count"
            );

            let expected = fixture_bits(&case["native"]["xyz_f32_u32_hex"]);
            let actual: Vec<u32> = emission.records.iter().flatten().copied().collect();
            assert_eq!(actual, expected, "{id} complete native output");
            assert_eq!(
                case["native"]["record_count"].as_u64().unwrap() as usize,
                emission.records.len(),
                "{id}"
            );
        }
    }
}

#[allow(dead_code)]
pub fn copied_model_curve(points: &[[f32; 2]], pressure: &[f32], size: f32) -> Vec<[u32; 3]> {
    source_glv1_curve(points, pressure, size)
}

#[allow(dead_code)]
pub fn copied_model_curve_trace(points: &[[f32; 2]], pressure: &[f32], size: f32) -> CurveEmission {
    source_glv1_curve_trace(points, pressure, size)
}

#[allow(dead_code)]
pub fn copied_model_no_curve(points: &[[f32; 2]], pressure: &[f32], size: f32) -> Vec<[u32; 3]> {
    source_glv1_no_curve(points, pressure, size)
}
