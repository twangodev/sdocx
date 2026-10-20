//! Bounded source-driven numeric model for ChineseBrush GLV1/2/3 calls.
//!
//! Test evidence for pinned synthetic source-event Drawing fixtures.
//! Saved-note parsing, renderer state and production admission remain separate.

use super::path::{P, Quad};

/// Hard work limits for callers that admit saved source into this sampler.
pub const MAX_SOURCE_EVENTS: usize = 65_536;
pub const MAX_OUTPUT_RECORDS: usize = 1_000_000;
// Native pressure has only an upper clamp. Bound negative values so width
// interpolation and scalar-index deltas remain finite and within i32 range at
// size <= 800, while preserving the captured negative-pressure cases.
const MIN_SOURCE_PRESSURE: f32 = -1_000_000.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleError {
    InvalidSource,
    UnsupportedProfile,
    OutputBudgetExceeded,
    AllocationFailed,
    DegeneratePath,
}

/// Bounded records plus cumulative exclusive record ends for raw source events.
///
/// `sample_ends[i]` is the number of records attributable through source event
/// `i`. A row batch becomes visible at the event index proven by the native
/// saved-redraw trace; this is source metadata, not a live-event replay policy.
#[derive(Debug, Clone, PartialEq)]
pub struct BoundedSample {
    pub records: Vec<[f32; 5]>,
    pub sample_ends: Vec<usize>,
}

struct Cubic {
    p: [P; 4],
    segments: Vec<(f32, u16)>,
    length: f32,
}

impl Cubic {
    fn new(p: [P; 4]) -> Self {
        let mut path = Self {
            p,
            segments: Vec::new(),
            length: 0.0,
        };
        path.split(p, 0, 32767);
        path
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
                self.segments.push((next, t1));
            }
        }
    }

    fn parameter(&self, d: f32) -> f32 {
        // getPosTan clamps the requested path distance to [0, getLength()]
        // before its segment lookup (Base 0xbd408–0xbd41c). The endPen loop
        // can advance one ULP beyond the path length after its final sample.
        let d = d.clamp(0.0, self.length);
        let i = self
            .segments
            .partition_point(|&(end, _)| end < d)
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
        t0 + (d - start) * (t1 - t0) / (end - start)
    }

    fn at(&self, d: f32) -> P {
        let t = self.parameter(d);
        let line = |a: f32, b: f32, t| (b - a).mul_add(t, a);
        let x0 = line(self.p[0].x, self.p[1].x, t);
        let x1 = line(self.p[1].x, self.p[2].x, t);
        let x2 = line(self.p[2].x, self.p[3].x, t);
        let y0 = line(self.p[0].y, self.p[1].y, t);
        let y1 = line(self.p[1].y, self.p[2].y, t);
        let y2 = line(self.p[2].y, self.p[3].y, t);
        let x = line(line(x0, x1, t), line(x1, x2, t), t);
        let y = line(line(y0, y1, t), line(y1, y2, t), t);
        P { x, y }
    }
}

#[derive(Clone, Copy)]
enum Path {
    Line(P, P),
    Quad(P, P, P),
    Cubic([P; 4]),
}

impl Path {
    fn length(self) -> f32 {
        match self {
            Self::Line(a, b) => b.sub(a).len(),
            Self::Quad(a, b, c) => Quad::new(a, b, c).length,
            Self::Cubic(p) => Cubic::new(p).length,
        }
    }

    fn at(self, d: f32) -> P {
        match self {
            Self::Line(a, b) => {
                let len = b.sub(a).len();
                let parameter_scale = 32767.0 * f32::from_bits(0x3800_00fd);
                let t = (d * parameter_scale) / len;
                P {
                    x: (b.x - a.x).mul_add(t, a.x),
                    y: (b.y - a.y).mul_add(t, a.y),
                }
            }
            Self::Quad(a, b, c) => Quad::new(a, b, c).at(d),
            Self::Cubic(p) => Cubic::new(p).at(d),
        }
    }
}

fn f32_trunc(x: f32) -> i32 {
    x as i32
}

fn repeat_index(length: f32, width: i32) -> usize {
    let width = width as f32;
    let scaled = if width <= 5.0 {
        length * 0.5
    } else if width <= 10.0 {
        length / 3.0
    } else if width <= 15.0 {
        length * 0.25
    } else if width <= 20.0 {
        length / 5.0
    } else if width <= 30.0 {
        length / 6.0
    } else if width <= 40.0 {
        length / 7.0
    } else if width <= 55.0 {
        length / 8.0
    } else {
        length / 10.0
    };
    (scaled as usize).max(2).saturating_add(1)
}

fn dab_size(pressure: f32, factor: f32, size: f32, prior: f32) -> f32 {
    // The native code widens factor to f64 before comparing against double
    // constants in its read-only data, so compare after widening as well.
    let factor64 = f64::from(factor);
    if factor64 < 0.35 {
        pressure.mul_add(size, prior) * 0.5
    } else if factor64 < 0.7 {
        // The native middle branch promotes all operands to f64, uses a
        // fused multiply-add for factor*0.5 + pressure*1.5, then converts
        // only the final result back to f32.
        let d = f64::from(pressure) * 1.5;
        let d = f64::from(factor).mul_add(0.5, d);
        let d = d * 0.5;
        let d = d.mul_add(f64::from(size), f64::from(prior));
        (d * 0.5) as f32
    } else {
        let d = (pressure + factor) * 0.5;
        size.mul_add(d, prior) * 0.5
    }
}

fn calculated_width(target: f32, prior: f32, size: f32) -> f32 {
    // GLV2 getCalculatedWidth compares |target-prior| with 10. For a large
    // change it advances by at most ten units; for a smaller upward change it
    // accepts the target, while a downward change below width ten slews by
    // half a unit. Every path applies the size/10 floor.
    let floor = size / 10.0;
    let difference = (prior - target).abs();
    if difference > 10.0 {
        let direction = if target > prior { 1.0 } else { -1.0 };
        10.0_f32.mul_add(direction, prior).max(floor)
    } else if target >= prior || prior >= 10.0 {
        target.max(floor)
    } else {
        (prior - 0.5).max(floor)
    }
}

fn factor_f32(speed: f32) -> f32 {
    if speed < 4.2 {
        1.0
    } else if speed < 9.5 {
        0.6
    } else if speed < 35.0 {
        let t = (speed - 9.5) / -25.5;
        t.mul_add(0.4, 0.4) + 0.2
    } else {
        0.2
    }
}

fn factor_f64(speed: f32) -> f32 {
    if speed < 4.2 {
        1.0
    } else if speed < 9.5 {
        0.6
    } else if speed < 35.0 {
        let t = (f64::from(speed) - 9.5) / -25.5;
        (t.mul_add(0.4, 0.4) + 0.2) as f32
    } else {
        0.2
    }
}

// Native drawLine and endPen compile the same interpolation with different
// scalar precision by selected GL version.
fn line_factor(speed: f32, version: i32) -> f32 {
    if version == 1 {
        factor_f32(speed)
    } else {
        factor_f64(speed)
    }
}

fn terminal_factor(speed: f32, version: i32) -> f32 {
    if version == 1 {
        factor_f64(speed)
    } else {
        factor_f32(speed)
    }
}

fn effective_pressure(pressure: f32, tool: i32) -> f32 {
    if tool == 1 { 0.5 } else { pressure.min(1.0) }
}

pub fn packed_w(phase: f32, scalar: f32) -> f32 {
    (phase as f64).mul_add(1.0, scalar as f64 * 0.001) as f32
}

#[allow(clippy::too_many_arguments)]
fn emit_path_limited(
    out: &mut Vec<[f32; 5]>,
    path: Path,
    repeat: usize,
    extra: usize,
    mut width: f32,
    width_target: f32,
    width_steps: usize,
    mut index: i32,
    index_target: i32,
    index_steps: usize,
    phase: &mut usize,
    record_limit: usize,
) -> Result<(), SampleError> {
    let count = repeat
        .checked_add(extra)
        .ok_or(SampleError::OutputBudgetExceeded)?;
    let new_len = out
        .len()
        .checked_add(count)
        .ok_or(SampleError::OutputBudgetExceeded)?;
    if new_len > record_limit {
        return Err(SampleError::OutputBudgetExceeded);
    }
    let length = path.length();
    if !length.is_finite() || length <= 0.0 {
        // Flattening a closed, subpixel curve can leave no parameter segments.
        // Reject it rather than inventing a native position or indexing an
        // empty segment table.
        return Err(SampleError::DegeneratePath);
    }
    out.try_reserve_exact(count)
        .map_err(|_| SampleError::AllocationFailed)?;
    let step = length / repeat as f32;
    let width_step = (width_target - width) / width_steps as f32;
    let index_step = (index_target - index) / index_steps as i32;
    let mut distance = 0.0_f32;
    for _ in 0..repeat + extra {
        let p = path.at(distance);
        out.push([p.x, p.y, width * 0.5, (*phase % 7) as f32, index as f32]);
        *phase += 1;
        distance += step;
        width += width_step;
        index += index_step;
    }
    Ok(())
}

fn single_record(record: [f32; 5]) -> Result<Vec<[f32; 5]>, SampleError> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(1)
        .map_err(|_| SampleError::AllocationFailed)?;
    output.push(record);
    Ok(output)
}

#[allow(clippy::too_many_arguments)] // Explicit native source/context plus output budget.
fn model_line(
    points: &[P],
    pressure: &[f32],
    size: f32,
    version: i32,
    width: i32,
    height: i32,
    tool: i32,
    record_limit: usize,
    mut sample_ends: Option<&mut [usize]>,
) -> Result<Vec<[f32; 5]>, SampleError> {
    let gate = if version == 1 || version == 2 {
        4.0
    } else {
        width.min(height) as f32 / 360.0
    };
    let mut accepted = Vec::new();
    accepted
        .try_reserve_exact(points.len())
        .map_err(|_| SampleError::AllocationFailed)?;
    let mut previous = points[0];
    for (i, point) in points.iter().enumerate().skip(1) {
        let d = point.sub(previous);
        if d.x.abs() < gate && d.y.abs() < gate {
            continue;
        }
        accepted.push(i);
        previous = *point;
    }
    if accepted.is_empty() {
        if record_limit == 0 {
            return Err(SampleError::OutputBudgetExceeded);
        }
        if let Some(ends) = sample_ends.as_deref_mut() {
            ends.fill(0);
            *ends.last_mut().expect("validated nonempty source") = 1;
        }
        return single_record([
            points.last().unwrap().x,
            points.last().unwrap().y,
            size * effective_pressure(*pressure.last().unwrap(), tool) * 0.5,
            0.0,
            0.0,
        ]);
    }
    let terminal = points.len() - 1;
    let accepted_last = *accepted.last().unwrap();
    if accepted_last != terminal
        && (points[accepted_last].x != points[terminal].x
            || points[accepted_last].y != points[terminal].y)
    {
        // endPen consumes the raw final source event after the gated move loop.
        // A duplicate endpoint has no terminal segment and would form a
        // degenerate Path::Line in the numeric sampler.
        accepted.push(terminal);
    }
    let seed = accepted[0];
    let scalar = f32_trunc(100.0 + effective_pressure(pressure[seed], tool) * 100.0) as f32;
    let scalar_index = scalar as i32;
    let mut width_state = size * effective_pressure(pressure[seed], tool);
    let mut start = points[0];
    let mut phase = 0;
    let mut out = Vec::new();
    let mut source_cursor = 1;
    for (event_index, &event) in accepted.iter().enumerate() {
        let target = if event_index == 0 {
            width_state
        } else {
            size * effective_pressure(pressure[event], tool)
        };
        let path = Path::Line(start, points[event]);
        let length = path.length();
        let repeat = repeat_index(length, width_state.min(target) as i32);
        emit_path_limited(
            &mut out,
            path,
            repeat,
            0,
            width_state,
            target,
            repeat,
            scalar_index,
            scalar_index,
            repeat,
            &mut phase,
            record_limit,
        )?;
        if let Some(ends) = sample_ends.as_deref_mut() {
            let end = out.len();
            let before = end - repeat;
            ends[source_cursor..event].fill(before);
            ends[event] = end;
            source_cursor = event + 1;
        }
        width_state = target;
        start = points[event];
    }
    if let Some(ends) = sample_ends {
        ends[source_cursor..].fill(out.len());
    }
    Ok(out)
}

// This boundary mirrors the source arrays and explicit native profile.
#[allow(clippy::too_many_arguments)]
pub fn model_event_loop(
    xy: &[[f32; 2]],
    pressure: &[f32],
    times: &[i32],
    size: f32,
    version: i32,
    width: i32,
    height: i32,
    tool: i32,
    curve: bool,
    record_limit: usize,
) -> Result<Vec<[f32; 5]>, SampleError> {
    model_event_loop_inner(
        xy,
        pressure,
        times,
        size,
        version,
        width,
        height,
        tool,
        curve,
        record_limit,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn model_event_loop_inner(
    xy: &[[f32; 2]],
    pressure: &[f32],
    times: &[i32],
    size: f32,
    version: i32,
    width: i32,
    height: i32,
    tool: i32,
    curve: bool,
    record_limit: usize,
    mut sample_ends: Option<&mut [usize]>,
) -> Result<Vec<[f32; 5]>, SampleError> {
    let mut points = Vec::new();
    points
        .try_reserve_exact(xy.len())
        .map_err(|_| SampleError::AllocationFailed)?;
    points.extend(xy.iter().map(|p| P { x: p[0], y: p[1] }));
    assert_eq!(points.len(), pressure.len());
    assert_eq!(points.len(), times.len());
    if !curve {
        return model_line(
            &points,
            pressure,
            size,
            version,
            width,
            height,
            tool,
            record_limit,
            sample_ends,
        );
    }
    let gate = if version <= 2 {
        4.0
    } else {
        width.min(height) as f32 / 360.0
    };
    let mut accepted = Vec::new();
    accepted
        .try_reserve_exact(points.len())
        .map_err(|_| SampleError::AllocationFailed)?;
    let mut previous = 0usize;
    let mut speeds = Vec::new();
    speeds
        .try_reserve_exact(points.len())
        .map_err(|_| SampleError::AllocationFailed)?;
    speeds.resize(points.len(), 0.0_f32);
    for i in 1..points.len() {
        let delta = points[i].sub(points[previous]);
        if delta.x.abs() < gate && delta.y.abs() < gate {
            continue;
        }
        if version == 1 {
            speeds[i] = 1.0;
        } else {
            let dt = times[i] - times[previous];
            let dt = if times[previous] == 0 || dt <= 0 {
                50000.0
            } else {
                dt as f32
            };
            speeds[i] = (delta.len() * 10000.0 / dt) * (1440.0 / width.min(height) as f32);
        }
        accepted.push(i);
        previous = i;
    }
    if accepted.is_empty() {
        let p = effective_pressure(*pressure.last().unwrap(), tool);
        if record_limit == 0 {
            return Err(SampleError::OutputBudgetExceeded);
        }
        if let Some(ends) = sample_ends.as_deref_mut() {
            ends.fill(0);
            *ends.last_mut().expect("validated nonempty source") = 1;
        }
        return single_record([
            points[0].x,
            points[0].y,
            size * 0.25,
            0.0,
            (50.0 + p * 100.0) as i32 as f32,
        ]);
    }
    let seed = accepted[0];
    let seed_pressure = effective_pressure(pressure[seed], tool);
    let mut prior = size * seed_pressure;
    let mut index = f32_trunc(20.0 + seed_pressure * 100.0);
    let mut phase = 0;
    let mut out = Vec::new();
    let mut start = points[0];
    let mut j = 0;
    let mut source_cursor = 1;
    while j + 2 < accepted.len() {
        let control = accepted[j];
        let lookahead = accepted[j + 2];
        let endpoint = points[control].mid(points[lookahead]);
        let path = Path::Quad(start, points[control], endpoint);
        let p = effective_pressure(pressure[lookahead], tool);
        let line_speed = if version == 1 {
            points[lookahead].sub(points[accepted[j + 1]]).len()
        } else {
            (speeds[accepted[j + 1]] + speeds[lookahead]) * 0.5
        };
        let f = line_factor(line_speed, version);
        let target = calculated_width(dab_size(p, f, size, prior), prior, size);
        let repeat = repeat_index(path.length(), prior.min(target) as i32);
        let target_index = f32_trunc(100.0 + p * 100.0);
        let before = out.len();
        emit_path_limited(
            &mut out,
            path,
            repeat,
            0,
            prior,
            target,
            repeat,
            index,
            target_index,
            repeat,
            &mut phase,
            record_limit,
        )?;
        if let Some(ends) = sample_ends.as_deref_mut() {
            ends[source_cursor..lookahead].fill(before);
            ends[lookahead] = out.len();
            source_cursor = lookahead + 1;
        }
        start = endpoint;
        prior = target;
        index = target_index;
        j += 2;
    }
    let m = accepted.len();
    let accepted_last = accepted[m - 1];
    let raw_last = points.len() - 1;
    // Native endPen rereads the raw MotionEvent even when drawLine rejected it.
    let use_raw_terminal = raw_last != accepted_last;
    let terminal_point = if use_raw_terminal {
        raw_last
    } else {
        accepted_last
    };
    let (path, extra) = if m % 2 == 0 {
        let a = accepted[m - 2];
        let b = terminal_point;
        (
            Path::Cubic([start, points[a], points[a].mid(points[b]), points[b]]),
            1,
        )
    } else {
        (
            Path::Quad(start, points[accepted_last], points[terminal_point]),
            1,
        )
    };
    let event = terminal_point;
    let p = effective_pressure(pressure[event], tool);
    let terminal_speed = if version == 1 {
        if m <= 2 {
            1.0
        } else if use_raw_terminal {
            points[accepted_last].sub(points[accepted[m - 2]]).len()
        } else {
            let previous = accepted[m - 2];
            points[event].sub(points[previous]).len()
        }
    } else {
        // endPen reads the speed stored by the last accepted drawLine. A raw
        // terminal MotionEvent contributes geometry and pressure, but does
        // not update that state because it did not pass the move gate.
        speeds[accepted_last]
    };
    let f = if version == 1 && m <= 2 {
        1.0
    } else {
        terminal_factor(terminal_speed, version)
    };
    let target = if version == 1 && m <= 2 {
        dab_size(p, f, size, prior).max(size / 10.0)
    } else {
        calculated_width(dab_size(p, f, size, prior), prior, size)
    };
    let repeat = repeat_index(path.length(), prior.min(target) as i32);
    let target_index = f32_trunc(50.0 + p * 100.0);
    let before = out.len();
    emit_path_limited(
        &mut out,
        path,
        repeat,
        extra,
        prior,
        target,
        repeat,
        index,
        target_index,
        repeat,
        &mut phase,
        record_limit,
    )?;
    if let Some(ends) = sample_ends {
        if source_cursor < event {
            ends[source_cursor..event].fill(before);
        }
        ends[event] = out.len();
        ends[event + 1..].fill(out.len());
    }
    Ok(out)
}

// Keep the compatibility wrapper's source and runtime parameters explicit.
#[allow(clippy::too_many_arguments)]
pub fn run_model(
    xy: &[[f32; 2]],
    pressure: &[f32],
    times: &[i32],
    size: f32,
    version: i32,
    width: i32,
    height: i32,
    tool: i32,
    curve: bool,
) -> Vec<[f32; 5]> {
    // Native selected Pen::SetSize clamps the requested size before RedrawPen.
    model_event_loop(
        xy,
        pressure,
        times,
        size.clamp(0.4, 800.0),
        version,
        width,
        height,
        tool,
        curve,
        usize::MAX,
    )
    .expect("compatibility model sampling failed")
}

/// Produce exact GLV1/2/3 records while enforcing a cumulative output budget.
/// The budget is checked for each whole path before reserving or emitting it.
/// Source-event boundaries are intentionally not returned: current native
/// fixtures capture append order but do not attribute rows to source events.
#[allow(clippy::too_many_arguments)]
pub fn run_model_bounded(
    xy: &[[f32; 2]],
    pressure: &[f32],
    times: &[i32],
    size: f32,
    version: i32,
    width: i32,
    height: i32,
    tool: i32,
    curve: bool,
    record_limit: usize,
) -> Result<Vec<[f32; 5]>, SampleError> {
    let (safe_size, limit) = validate_bounded_source(
        xy,
        pressure,
        times,
        size,
        version,
        width,
        height,
        tool,
        record_limit,
    )?;
    model_event_loop_inner(
        xy, pressure, times, safe_size, version, width, height, tool, curve, limit, None,
    )
}

/// Produce bounded native saved-redraw records with one cumulative exclusive
/// record end for every raw source event. Curved batches are tagged at the
/// accepted lookahead event that emitted them; terminal rows are tagged at the
/// raw final event. NoCurve batches are tagged from observed native source-array
/// reads, including rejected raw-terminal and all-rejected dot paths.
#[allow(clippy::too_many_arguments)]
pub fn run_model_bounded_with_sample_ends(
    xy: &[[f32; 2]],
    pressure: &[f32],
    times: &[i32],
    size: f32,
    version: i32,
    width: i32,
    height: i32,
    tool: i32,
    curve: bool,
    record_limit: usize,
) -> Result<BoundedSample, SampleError> {
    let (safe_size, limit) = validate_bounded_source(
        xy,
        pressure,
        times,
        size,
        version,
        width,
        height,
        tool,
        record_limit,
    )?;
    let mut sample_ends = Vec::new();
    sample_ends
        .try_reserve_exact(xy.len())
        .map_err(|_| SampleError::AllocationFailed)?;
    sample_ends.resize(xy.len(), 0);
    let records = model_event_loop_inner(
        xy,
        pressure,
        times,
        safe_size,
        version,
        width,
        height,
        tool,
        curve,
        limit,
        Some(&mut sample_ends),
    )?;
    Ok(BoundedSample {
        records,
        sample_ends,
    })
}

#[allow(clippy::too_many_arguments)]
fn validate_bounded_source(
    xy: &[[f32; 2]],
    pressure: &[f32],
    times: &[i32],
    size: f32,
    version: i32,
    width: i32,
    height: i32,
    tool: i32,
    record_limit: usize,
) -> Result<(f32, usize), SampleError> {
    let len = xy.len();
    if len == 0 || len > MAX_SOURCE_EVENTS || pressure.len() != len || times.len() != len {
        return Err(SampleError::InvalidSource);
    }
    if !(1..=3).contains(&version) || width <= 0 || height <= 0 || !(0..=3).contains(&tool) {
        return Err(SampleError::UnsupportedProfile);
    }
    if !size.is_finite()
        || xy.iter().flatten().any(|v| !v.is_finite())
        || xy.iter().flatten().any(|v| v.abs() > 1.0e18)
        // Native traces include negative pressure values; keep them, while
        // bounding their magnitude so size/pressure width arithmetic stays finite.
        || pressure
            .iter()
            .any(|v| !v.is_finite() || *v < MIN_SOURCE_PRESSURE)
        || (i64::from(*times.iter().max().expect("nonempty timestamps"))
            - i64::from(*times.iter().min().expect("nonempty timestamps")))
            > i64::from(i32::MAX)
    {
        return Err(SampleError::InvalidSource);
    }
    let limit = record_limit.min(MAX_OUTPUT_RECORDS);
    let safe_size = size.clamp(0.4, 800.0);
    Ok((safe_size, limit))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use sha2::{Digest, Sha256};

    fn words(hex: &str) -> Vec<u32> {
        hex.as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect::<Vec<_>>()
            .as_chunks::<4>()
            .0
            .iter()
            .map(|word| u32::from_le_bytes(*word))
            .collect()
    }

    fn source_arrays(input: &Value) -> (Vec<P>, Vec<f32>, Vec<i32>) {
        let arrays = &input["source_arrays"];
        let point_words = words(arrays["points"]["data_hex"].as_str().unwrap());
        let points = point_words
            .as_chunks::<2>()
            .0
            .iter()
            .map(|xy| P {
                x: f32::from_bits(xy[0]),
                y: f32::from_bits(xy[1]),
            })
            .collect();
        let pressure = words(arrays["pressures"]["data_hex"].as_str().unwrap())
            .into_iter()
            .map(f32::from_bits)
            .collect();
        let times = arrays["timestamps"]["data_hex"]
            .as_str()
            .unwrap()
            .as_bytes()
            .as_chunks::<8>()
            .0
            .iter()
            .map(|chunk| {
                let bytes: Vec<u8> = chunk
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                    .collect();
                i32::from_le_bytes(bytes.try_into().unwrap())
            })
            .collect();
        (points, pressure, times)
    }

    fn expected_words(row: &Value, field: &str) -> Vec<u32> {
        let bytes = hex_bytes(row[field].as_str().unwrap());
        bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| u32::from_le_bytes(*b))
            .collect()
    }

    fn hex_bytes(hex: &str) -> Vec<u8> {
        hex.as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect()
    }

    fn digest_hex(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    fn stress_source(count: usize, shifted: bool) -> (Vec<[f32; 2]>, Vec<f32>, Vec<i32>) {
        let mut points64: Vec<[f64; 2]> = Vec::with_capacity(count);
        for i in 0..count {
            if i > 0 && i % 17 == 0 {
                points64.push(points64[i - 1]);
            } else if i > 0 && i % 19 == 0 {
                let [x, y] = points64[i - 1];
                points64.push([x + 0.05_f64, y - 0.03_f64]);
            } else {
                points64.push([
                    i as f64 * 1.35_f64,
                    (i % 9) as f64 * 1.1_f64 + (i / 9) as f64 * 0.4_f64,
                ]);
            }
        }
        let points: Vec<[f32; 2]> = points64
            .into_iter()
            .map(|[x, y]| {
                if shifted {
                    [(x + 120.0_f64) as f32, (y + 80.0_f64) as f32]
                } else {
                    [x as f32, y as f32]
                }
            })
            .collect();
        let pressure_values = [0.12_f64, 0.35, 0.62, 0.88, 0.48, 0.73];
        let pressure = (0..count)
            .map(|i| pressure_values[i % pressure_values.len()] as f32)
            .collect();
        let mut ticks = 200_i32;
        let times = (0..count)
            .map(|i| {
                let current = ticks;
                ticks += [11, 13, 17, 23, 29][i % 5] + if i % 23 == 22 { 19 } else { 0 };
                current
            })
            .collect();
        (points, pressure, times)
    }

    fn source_input_sha256(xy: &[[f32; 2]], pressure: &[f32], times: &[i32]) -> String {
        let mut bytes = Vec::with_capacity(xy.len() * 12);
        for [x, y] in xy {
            bytes.extend_from_slice(&x.to_le_bytes());
            bytes.extend_from_slice(&y.to_le_bytes());
        }
        for value in pressure {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        for value in times {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        digest_hex(&bytes)
    }

    fn check_native_trace(
        name: &str,
        xy: &[[f32; 2]],
        pressure: &[f32],
        times: &[i32],
        profile: (f32, i32),
        expected: (usize, &str, &str),
    ) {
        let (size, version) = profile;
        check_native_trace_profile(
            name,
            xy,
            pressure,
            times,
            (size, version, 1920, 1080, 2, true),
            expected,
        );
    }

    fn check_native_trace_profile(
        name: &str,
        xy: &[[f32; 2]],
        pressure: &[f32],
        times: &[i32],
        profile: (f32, i32, i32, i32, i32, bool),
        expected: (usize, &str, &str),
    ) -> BoundedSample {
        let (size, version, width, height, tool, curve) = profile;
        let (count, input_sha, output_sha) = expected;
        assert_eq!(
            source_input_sha256(xy, pressure, times),
            input_sha,
            "{name} source bytes"
        );
        let sample = |limit| {
            run_model_bounded(
                xy, pressure, times, size, version, width, height, tool, curve, limit,
            )
        };
        let output = sample(count).unwrap_or_else(|error| panic!("{name} exact budget: {error:?}"));
        assert_eq!(output.len(), count, "{name} record count");
        let with_ends = run_model_bounded_with_sample_ends(
            xy, pressure, times, size, version, width, height, tool, curve, count,
        )
        .unwrap_or_else(|error| panic!("{name} exact metadata budget: {error:?}"));
        assert_eq!(with_ends.records.len(), count, "{name} metadata records");
        assert_eq!(
            with_ends.sample_ends.len(),
            xy.len(),
            "{name} source-event ends"
        );
        assert!(
            with_ends
                .sample_ends
                .windows(2)
                .all(|pair| pair[0] <= pair[1]),
            "{name} monotone source-event ends"
        );
        assert_eq!(with_ends.sample_ends.last().copied(), Some(count));
        for (i, (with_end, bounded)) in with_ends.records.iter().zip(&output).enumerate() {
            assert_eq!(
                with_end.map(f32::to_bits),
                bounded.map(f32::to_bits),
                "{name} metadata record {i}"
            );
        }
        let packed: Vec<u8> = output
            .iter()
            .flat_map(|r| {
                [r[0], r[1], r[2], packed_w(r[3], r[4])]
                    .into_iter()
                    .flat_map(f32::to_le_bytes)
            })
            .collect();
        assert_eq!(
            digest_hex(&packed),
            output_sha,
            "{name} complete packed stream"
        );
        assert_eq!(
            sample(count - 1),
            Err(SampleError::OutputBudgetExceeded),
            "{name} one-below budget"
        );
        assert_eq!(
            run_model_bounded_with_sample_ends(
                xy,
                pressure,
                times,
                size,
                version,
                width,
                height,
                tool,
                curve,
                count - 1,
            ),
            Err(SampleError::OutputBudgetExceeded),
            "{name} metadata one-below budget"
        );
        with_ends
    }

    #[allow(clippy::too_many_arguments)]
    fn check_native_trace_cut(
        name: &str,
        xy: &[[f32; 2]],
        pressure: &[f32],
        times: &[i32],
        profile: (f32, i32, i32, i32, i32, bool),
        expected: (usize, &str, &str),
        expected_ends: &[usize],
    ) {
        let sample = check_native_trace_profile(name, xy, pressure, times, profile, expected);
        assert_eq!(
            sample.sample_ends, expected_ends,
            "{name} native source cuts"
        );
    }

    fn check_fixture(fixture: &Value) -> usize {
        let mut record_count = 0;
        for (name, case) in fixture["cases"].as_object().unwrap() {
            let input = &case["input"];
            let (points, pressure, times) = source_arrays(input);
            let size = f32::from_bits(u32::from_le_bytes(
                hex_bytes(input["size_f32_bits"].as_str().unwrap())
                    .try_into()
                    .unwrap(),
            ));
            let version = case["native_selection"]["version"].as_i64().unwrap() as i32;
            let profile = &case["profile"];
            let w = profile["width"].as_i64().unwrap() as i32;
            let h = profile["height"].as_i64().unwrap() as i32;
            let xy: Vec<[f32; 2]> = points.iter().map(|p| [p.x, p.y]).collect();
            let predicted = run_model(
                &xy,
                &pressure,
                &times,
                size,
                version,
                w,
                h,
                input["tool_type"].as_i64().unwrap() as i32,
                input["curve_enabled"].as_bool().unwrap(),
            );
            let rows = case["producer"]["append_rows"].as_array().unwrap();
            record_count += rows.len();
            assert_eq!(predicted.len(), rows.len(), "{name}");
            let bounded = run_model_bounded(
                &xy,
                &pressure,
                &times,
                size,
                version,
                w,
                h,
                input["tool_type"].as_i64().unwrap() as i32,
                input["curve_enabled"].as_bool().unwrap(),
                rows.len(),
            )
            .unwrap_or_else(|error| panic!("{name} bounded sampler: {error:?}"));
            assert_eq!(bounded.len(), rows.len(), "{name} bounded row count");
            let bounded_with_ends = run_model_bounded_with_sample_ends(
                &xy,
                &pressure,
                &times,
                size,
                version,
                w,
                h,
                input["tool_type"].as_i64().unwrap() as i32,
                input["curve_enabled"].as_bool().unwrap(),
                rows.len(),
            )
            .unwrap_or_else(|error| panic!("{name} bounded sampler with ends: {error:?}"));
            assert_eq!(
                bounded_with_ends.records.len(),
                rows.len(),
                "{name} records"
            );
            assert_eq!(
                bounded_with_ends.sample_ends.len(),
                xy.len(),
                "{name} endpoints"
            );
            assert!(
                bounded_with_ends
                    .sample_ends
                    .windows(2)
                    .all(|pair| pair[0] <= pair[1]),
                "{name} endpoint monotonicity"
            );
            assert_eq!(
                bounded_with_ends.sample_ends.last().copied(),
                Some(rows.len()),
                "{name} final endpoint"
            );
            for (i, (bounded_row, compatibility_row)) in bounded.iter().zip(&predicted).enumerate()
            {
                assert_eq!(
                    bounded_row.map(f32::to_bits),
                    compatibility_row.map(f32::to_bits),
                    "{name} bounded record {i}"
                );
                assert_eq!(
                    bounded_with_ends.records[i].map(f32::to_bits),
                    bounded_row.map(f32::to_bits),
                    "{name} metadata route record {i}"
                );
            }
            let joined: Vec<u8> = rows
                .iter()
                .flat_map(|row| hex_bytes(row["delta_bytes"].as_str().unwrap()))
                .collect();
            assert_eq!(joined.len(), rows.len() * 16, "{name} buffer length");
            assert_eq!(
                joined,
                hex_bytes(case["producer"]["buffer_hex"].as_str().unwrap()),
                "{name} complete buffer"
            );
            for (i, (point, row)) in predicted.iter().zip(rows).enumerate() {
                assert_eq!(
                    expected_words(row, "scalar_s0_f32_bits"),
                    vec![point[4].to_bits()],
                    "{name} scalar {i}"
                );
                let actual_arg = expected_words(row, "vector4_arg_bytes");
                let actual_delta = expected_words(row, "delta_bytes");
                let vector_expected: Vec<u32> = point[..4].iter().map(|v| v.to_bits()).collect();
                let delta_expected = vec![
                    point[0].to_bits(),
                    point[1].to_bits(),
                    point[2].to_bits(),
                    packed_w(point[3], point[4]).to_bits(),
                ];
                assert_eq!(vector_expected, actual_arg, "{name} vector arg {i}");
                assert_eq!(delta_expected, actual_delta, "{name} packed delta {i}");
            }
        }
        record_count
    }

    #[test]
    fn short_terminal_cases_match_native_for_each_version_and_time_origin() {
        let a_xy = [[0.0, 0.0], [10.0, 0.0], [10.25, 0.25]];
        let a_pressure = [0.2, 0.4, 0.9];
        let a_times = [200, 220, 240];
        let b_xy = [[0.0, 0.0], [10.0, 0.0], [20.0, 3.0], [20.25, 3.25]];
        let b_pressure = [0.2, 0.4, 0.6, 0.9];
        let b_times = [200, 220, 240, 260];
        let c_xy = [
            [0.0, 0.0],
            [10.0, 0.0],
            [20.0, 3.0],
            [30.0, 6.0],
            [30.25, 6.25],
        ];
        let c_pressure = [0.2, 0.4, 0.6, 0.8, 0.9];
        let c_times = [200, 220, 240, 260, 280];
        let d_xy = [[0.0, 0.0], [0.25, 0.25], [10.0, 0.0], [10.25, 0.25]];
        let d_pressure = [0.2, 0.4, 0.6, 0.9];
        let d_times = [200, 210, 220, 240];
        let size = 3.144_f64 as f32;

        let cases = [
            (
                "short-a-v1",
                &a_xy[..],
                &a_pressure[..],
                &a_times[..],
                1,
                7,
                "9799392c62e10d1e4b8fca0da845172c14cfdd63b27d50af052def7a39269938",
                "4d13cd8e00b98b4f05b5f4ffa7fc0717df96dadbae6314124212c30b90be85d6",
            ),
            (
                "short-a-v2",
                &a_xy[..],
                &a_pressure[..],
                &a_times[..],
                2,
                7,
                "9799392c62e10d1e4b8fca0da845172c14cfdd63b27d50af052def7a39269938",
                "a2d05bf6aa9e389a0b8b48ad40794faf116099e371ae57de95f9bfa89aeaf879",
            ),
            (
                "short-a-v3",
                &a_xy[..],
                &a_pressure[..],
                &a_times[..],
                3,
                7,
                "9799392c62e10d1e4b8fca0da845172c14cfdd63b27d50af052def7a39269938",
                "a2d05bf6aa9e389a0b8b48ad40794faf116099e371ae57de95f9bfa89aeaf879",
            ),
            (
                "short-b-v1",
                &b_xy[..],
                &b_pressure[..],
                &b_times[..],
                1,
                12,
                "de426aa6df6fa24567556bc214a2e3e23abf540a4f7facb95a3421ca9c3a5251",
                "31ba4c95237d1a818a745f9d02770211ee000c9617524f3ed21a69aea278f51b",
            ),
            (
                "short-b-v2",
                &b_xy[..],
                &b_pressure[..],
                &b_times[..],
                2,
                12,
                "de426aa6df6fa24567556bc214a2e3e23abf540a4f7facb95a3421ca9c3a5251",
                "c9bbfff98641c6391e421c92ee9174990cd873d24d8f150b2fc25c2fb0a62810",
            ),
            (
                "short-b-v3",
                &b_xy[..],
                &b_pressure[..],
                &b_times[..],
                3,
                12,
                "de426aa6df6fa24567556bc214a2e3e23abf540a4f7facb95a3421ca9c3a5251",
                "c9bbfff98641c6391e421c92ee9174990cd873d24d8f150b2fc25c2fb0a62810",
            ),
            (
                "short-c-v1",
                &c_xy[..],
                &c_pressure[..],
                &c_times[..],
                1,
                18,
                "e70902ab1c885fd4ad157d86dfc53aa9a4e77a4d754b3b538b2932453f53e204",
                "7673e6230ee6b3550c3eb39ca9b6f94190a270988debc1b350b9f9368367ccca",
            ),
            (
                "short-c-v2",
                &c_xy[..],
                &c_pressure[..],
                &c_times[..],
                2,
                18,
                "e70902ab1c885fd4ad157d86dfc53aa9a4e77a4d754b3b538b2932453f53e204",
                "8b97a362b74df8cfdd873d94207c5fb86c8f96e6eb4573f83a2aa331edd4660c",
            ),
            (
                "short-c-v3",
                &c_xy[..],
                &c_pressure[..],
                &c_times[..],
                3,
                18,
                "e70902ab1c885fd4ad157d86dfc53aa9a4e77a4d754b3b538b2932453f53e204",
                "8b97a362b74df8cfdd873d94207c5fb86c8f96e6eb4573f83a2aa331edd4660c",
            ),
            (
                "short-d-v3",
                &d_xy[..],
                &d_pressure[..],
                &d_times[..],
                3,
                7,
                "2d1d38f1a6ac372728d1445dedb35014341e90e250fe34cd0099e6d6c7b378d9",
                "f4f5230ac89595bcfc7cfeb9ca52f27e61a0a153576f71e0e9bf799a4f2d62ac",
            ),
        ];

        for (name, xy, pressure, times, version, expected_count, input_sha, output_sha) in cases {
            check_native_trace(
                name,
                xy,
                pressure,
                times,
                (size, version),
                (expected_count, input_sha, output_sha),
            );
        }

        for (name, xy, pressure, times, count, input_sha, output_sha) in [
            (
                "short-a-zero-v3",
                &a_xy[..],
                &a_pressure[..],
                &[0, 20, 40][..],
                7,
                "d1126de495f7b8308717dae69893e0160df12ba0bec66c54195bb3b2f64b6309",
                "4d13cd8e00b98b4f05b5f4ffa7fc0717df96dadbae6314124212c30b90be85d6",
            ),
            (
                "short-b-zero-v3",
                &b_xy[..],
                &b_pressure[..],
                &[0, 20, 40, 60][..],
                12,
                "1de312d0f14562e5738486034e32c24d62d6e12a24b23b1add0c4100b30a2d25",
                "c9bbfff98641c6391e421c92ee9174990cd873d24d8f150b2fc25c2fb0a62810",
            ),
            (
                "short-d-zero-v3",
                &d_xy[..],
                &d_pressure[..],
                &[0, 10, 20, 40][..],
                7,
                "0ca7699e1ca9d2aa8aa0a839a42cbc57e6f61915091668a014a92f2ecf987e50",
                "79211558fff0bbcea87cb53fe67cf8937abf1d166a4576bae3f03d15b3554155",
            ),
        ] {
            check_native_trace(
                name,
                xy,
                pressure,
                times,
                (size, 3),
                (count, input_sha, output_sha),
            );
        }
    }

    #[test]
    fn source_streams_match_native_paint_controls_at_small_and_large_sizes() {
        // Audited native Drawing -> queued paint captures share these source
        // bytes at 1x/2x/4x. Check the complete VBO before comparing paint;
        // matching producer bytes does not establish raster or export parity.
        let xy = [
            [40.0, 40.0],
            [46.0, 43.0],
            [49.0, 52.0],
            [57.0, 53.0],
            [63.0, 55.0],
        ];
        let pressure = [0.3, 0.55, 0.75, 0.35, 0.95];
        let times = [300, 311, 324, 338, 355];
        let input_sha = "0029677ef22dc5a238bfdcc87895fdf2b3a86123487c9cc63a9995507bcfe13e";
        for (name, size, curve, count, output_sha) in [
            (
                "paint-size24",
                24.0,
                true,
                11,
                "a87baacfcd4321e69430c93f9f472ff134f9326cf943d13626873ac7d50bdf3d",
            ),
            (
                "paint-size3.144",
                3.144,
                true,
                16,
                "d612fdcbd17c9094617111d0dd82c79919ad511ef2305d5089358970c8f82018",
            ),
            (
                "paint-size0.4",
                0.4,
                true,
                16,
                "6f3494b41b3e1dd0fa50a042f5f4a11e2d7e786741884b1c7146c0de2a59f05e",
            ),
            (
                "paint-size1",
                1.0,
                true,
                16,
                "f2e1564ab18e09f8408679fcf1f147f5709f6f9d1df3c4b5a0195a7a8a5e53a2",
            ),
            (
                "paint-no-curve",
                3.144,
                false,
                18,
                "38c112b760466a6f1d9f2a906a6e468145208727ca9daf2118f2fec0120e1403",
            ),
        ] {
            check_native_trace_profile(
                name,
                &xy,
                &pressure,
                &times,
                (size, 3, 1920, 1080, 2, curve),
                (count, input_sha, output_sha),
            );
        }
    }

    #[test]
    fn captured_saved_redraw_sample_ends_match_native_source_read_cuts() {
        let proto_xy = [
            [0.0, 0.0],
            [6.0, 3.0],
            [9.0, 12.0],
            [17.0, 13.0],
            [23.0, 15.0],
            [31.0, 18.0],
            [36.0, 28.0],
            [45.0, 34.0],
        ];
        let proto_pressure = [0.3, 0.55, 0.75, 0.35, 0.95, 0.5, 0.7, 0.4];
        let proto_times = [300, 311, 324, 338, 355, 376, 390, 407];
        let proto_ends = [0, 0, 0, 8, 8, 16, 16, 32];
        for (version, output_sha) in [
            (
                1,
                "1a85978c2052ff9c202e56762cca545f9506a57f0b78c9b44d9f8153eb2f488b",
            ),
            (
                2,
                "4e7b93d798e599093cabf861cb4bb2df95a76b73d0765b7e75f94a5109388770",
            ),
            (
                3,
                "4e7b93d798e599093cabf861cb4bb2df95a76b73d0765b7e75f94a5109388770",
            ),
        ] {
            check_native_trace_cut(
                &format!("saved-proto8-v{version}"),
                &proto_xy,
                &proto_pressure,
                &proto_times,
                (3.144_f64 as f32, version, 1920, 1080, 2, true),
                (
                    32,
                    "650c0661aca26161bf38c7f926d8a6817bf91acf9f4e9727c5e9be4665d5673d",
                    output_sha,
                ),
                &proto_ends,
            );
        }

        let (mono_xy, mono_pressure, mono_times) = stress_source(32, false);
        let mut mono_ends = [0; 32];
        mono_ends[9..15].fill(5);
        mono_ends[15..21].fill(10);
        mono_ends[21..27].fill(15);
        mono_ends[27..31].fill(20);
        mono_ends[31] = 27;
        let mono_input_sha = "5e5dd289f19c8bfc465a2c5f23ec2b7532afb9e1cb6b1b5d30234f96ddfa42f5";
        for (version, output_sha) in [
            (
                1,
                "2e374d6db19b25ccb27f33516bfd623bcfb0db9122f65d76d9f4cae5637190b2",
            ),
            (
                2,
                "ec9b78aaf737d17294c344a5b8a93f927e6d759422d2745c7dfddb65fd8661d3",
            ),
            (
                3,
                "ec9b78aaf737d17294c344a5b8a93f927e6d759422d2745c7dfddb65fd8661d3",
            ),
        ] {
            check_native_trace_cut(
                &format!("saved-mono32-v{version}"),
                &mono_xy,
                &mono_pressure,
                &mono_times,
                (3.144_f64 as f32, version, 1920, 1080, 2, true),
                (27, mono_input_sha, output_sha),
                &mono_ends,
            );
        }

        let control_xy = [
            [40.0, 40.0],
            [46.0, 43.0],
            [49.0, 52.0],
            [57.0, 53.0],
            [63.0, 55.0],
        ];
        let control_pressure = [0.3, 0.55, 0.75, 0.35, 0.95];
        let control_times = [300, 311, 324, 338, 355];
        for version in 1..=3 {
            check_native_trace_cut(
                &format!("saved-no-curve-five-v{version}"),
                &control_xy,
                &control_pressure,
                &control_times,
                (3.144_f64 as f32, version, 1920, 1080, 2, false),
                (
                    18,
                    "0029677ef22dc5a238bfdcc87895fdf2b3a86123487c9cc63a9995507bcfe13e",
                    "38c112b760466a6f1d9f2a906a6e468145208727ca9daf2118f2fec0120e1403",
                ),
                &[0, 4, 9, 14, 18],
            );
        }

        let rejected_xy = [[0.0, 0.0], [8.0, 0.0], [16.0, 0.0], [17.0, 0.0]];
        let rejected_pressure = [0.3, 0.5, 0.8, 0.95];
        let rejected_times = [200, 220, 240, 260];
        let duplicate_xy = [[0.0, 0.0], [8.0, 0.0], [8.0, 0.0]];
        let duplicate_pressure = [0.3, 0.5, 0.9];
        let duplicate_times = [200, 220, 240];
        let dot_xy = [[0.0, 0.0], [1.0, 1.0], [2.0, 2.0]];
        let dot_pressure = [0.3, 0.5, 0.8];
        let dot_times = [200, 220, 240];
        for version in 1..=3 {
            check_native_trace_cut(
                &format!("saved-no-curve-rejected-v{version}"),
                &rejected_xy,
                &rejected_pressure,
                &rejected_times,
                (3.144_f64 as f32, version, 1920, 1080, 2, false),
                (
                    13,
                    "d27cfa3a6ca81d6314f2d7afd76344f7c4df20ca1c52df64c127afa0fa7e7aad",
                    "b6aec4e98ed3f6970395ea9baca5f74ce848fc4b28069919b02fee0bfeeb03dd",
                ),
                &[0, 5, 10, 13],
            );
            check_native_trace_cut(
                &format!("saved-no-curve-duplicate-v{version}"),
                &duplicate_xy,
                &duplicate_pressure,
                &duplicate_times,
                (3.144_f64 as f32, version, 1920, 1080, 2, false),
                (
                    5,
                    "267f07456725cc3dc28bb98a64d1a67fd327371714cf3314b14ea69de956e926",
                    "a1b055b6c7b26c4164bb6b1ced962d87a9265234cf5d9d2ac30820d9b0285389",
                ),
                &[0, 5, 5],
            );
            check_native_trace_cut(
                &format!("saved-no-curve-dot-v{version}"),
                &dot_xy,
                &dot_pressure,
                &dot_times,
                (3.144_f64 as f32, version, 1920, 1080, 2, false),
                (
                    1,
                    "bba82f61262449bdabe3c0508c0ebaf4f0b3bc59737d332b1ac78b44e5f7d550",
                    "a172561328d8da58127c45466d41afe5ca644f3912a78cfb40e95d6dfc7d413f",
                ),
                &[0, 0, 1],
            );
        }
    }

    #[test]
    fn no_curve_source_loop_matches_native_terminal_and_gate_matrix() {
        let size = 3.144_f64 as f32;
        let accepted4_xy = [[0.0, 0.0], [8.0, 0.0], [16.0, 0.0], [24.0, 0.0]];
        let accepted4_pressure = [0.3, 0.5, 0.8, 0.2];
        let accepted4_times = [200, 220, 240, 260];
        let rejected_final_xy = [[0.0, 0.0], [8.0, 0.0], [16.0, 0.0], [17.0, 0.0]];
        let rejected_final_pressure = [0.3, 0.5, 0.8, 0.95];
        let rejected_final_times = [200, 220, 240, 260];
        let all_rejected_xy = [[0.0, 0.0], [1.0, 1.0], [2.0, 2.0]];
        let all_rejected_pressure = [0.3, 0.5, 0.8];
        let all_rejected_times = [200, 220, 240];
        let all_rejected_low_pressure = [0.2, 0.4, 0.3];
        let two_event_xy = [[0.0, 0.0], [1.0, 0.0]];
        let two_event_pressure = [0.3, 0.8];
        let two_event_times = [200, 220];
        let negative_pressure_xy = [[0.0, 0.0], [8.0, 0.0], [16.0, 0.0]];
        let negative_pressure = [0.3, -0.2, -1.0];
        let three_times = [200, 220, 240];
        let zero_pressure = [0.3, 0.0, 0.0];
        let edge_components_xy = [
            [0.0, 0.0],
            [2.99, 0.0],
            [3.0, 0.0],
            [3.01, 0.0],
            [3.99, 0.0],
            [4.0, 0.0],
            [4.01, 0.0],
        ];
        let edge_components_pressure = [0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9];
        let edge_components_times = [200, 210, 220, 230, 240, 250, 260];
        let edge_diagonal_xy = [[0.0, 0.0], [3.9, 3.9], [7.9, 7.9]];
        let edge_diagonal_pressure = [0.3, 0.4, 0.8];
        let edge_diagonal_times = [200, 210, 220];
        let screen_xy = [[0.0, 0.0], [3.5, 0.0], [8.0, 0.0]];
        let screen_pressure = [0.3, 0.5, 0.8];
        let identical_xy = [[0.0, 0.0], [0.0, 0.0], [0.0, 0.0]];
        let identical_pressure = [0.2, 0.4, 0.9];
        let identical_times = [200, 220, 240];
        let duplicate_terminal_xy = [[0.0, 0.0], [8.0, 0.0], [8.0, 0.0]];
        let duplicate_terminal_pressure = [0.3, 0.5, 0.9];
        let duplicate_terminal_times = [200, 220, 240];
        let out_back_xy = [[0.0, 0.0], [8.0, 0.0], [0.0, 0.0]];
        let out_back_pressure = [0.3, 0.5, 0.9];
        let out_back_times = [200, 220, 240];

        macro_rules! check_versions {
            ($name:literal, $size:expr, $xy:expr, $pressure:expr, $times:expr, $tool:expr,
             $width:expr, $height:expr, $count:expr, $input:literal,
             $v1:literal, $v2:literal, $v3:literal) => {{
                for (version, output_sha) in [(1, $v1), (2, $v2), (3, $v3)] {
                    let case_name = format!("{}-v{}", $name, version);
                    check_native_trace_profile(
                        &case_name,
                        $xy,
                        $pressure,
                        $times,
                        ($size, version, $width, $height, $tool, false),
                        ($count, $input, output_sha),
                    );
                }
            }};
        }

        check_versions!(
            "nocurve-accepted4",
            size,
            &accepted4_xy,
            &accepted4_pressure,
            &accepted4_times,
            2,
            1920,
            1080,
            15,
            "011f5250f98499e958fdf7505898e441ca0800ce983f0a4a60b621f2e2373c12",
            "f8e38c47c2bf9153d2c8d4586276da8630907892d6992d7d0634fa697847ac94",
            "f8e38c47c2bf9153d2c8d4586276da8630907892d6992d7d0634fa697847ac94",
            "f8e38c47c2bf9153d2c8d4586276da8630907892d6992d7d0634fa697847ac94"
        );
        check_versions!(
            "nocurve-rejected-final",
            size,
            &rejected_final_xy,
            &rejected_final_pressure,
            &rejected_final_times,
            2,
            1920,
            1080,
            13,
            "d27cfa3a6ca81d6314f2d7afd76344f7c4df20ca1c52df64c127afa0fa7e7aad",
            "b6aec4e98ed3f6970395ea9baca5f74ce848fc4b28069919b02fee0bfeeb03dd",
            "b6aec4e98ed3f6970395ea9baca5f74ce848fc4b28069919b02fee0bfeeb03dd",
            "b6aec4e98ed3f6970395ea9baca5f74ce848fc4b28069919b02fee0bfeeb03dd"
        );
        check_versions!(
            "nocurve-all-rejected",
            size,
            &all_rejected_xy,
            &all_rejected_pressure,
            &all_rejected_times,
            2,
            1920,
            1080,
            1,
            "bba82f61262449bdabe3c0508c0ebaf4f0b3bc59737d332b1ac78b44e5f7d550",
            "a172561328d8da58127c45466d41afe5ca644f3912a78cfb40e95d6dfc7d413f",
            "a172561328d8da58127c45466d41afe5ca644f3912a78cfb40e95d6dfc7d413f",
            "a172561328d8da58127c45466d41afe5ca644f3912a78cfb40e95d6dfc7d413f"
        );
        check_versions!(
            "nocurve-all-rejected-tool1",
            size,
            &all_rejected_xy,
            &all_rejected_pressure,
            &all_rejected_times,
            1,
            1920,
            1080,
            1,
            "bba82f61262449bdabe3c0508c0ebaf4f0b3bc59737d332b1ac78b44e5f7d550",
            "4f8ae40389b66aef784b674706acb22f1ee3ffe02179c7286d79256ba8399025",
            "4f8ae40389b66aef784b674706acb22f1ee3ffe02179c7286d79256ba8399025",
            "4f8ae40389b66aef784b674706acb22f1ee3ffe02179c7286d79256ba8399025"
        );
        check_versions!(
            "nocurve-all-rejected-size",
            8.333333,
            &all_rejected_xy,
            &all_rejected_low_pressure,
            &all_rejected_times,
            2,
            1920,
            1080,
            1,
            "4705583333c28ed50039fa43454317c7cf2952e0881325415f0e0d9b1ec73744",
            "afa0f047eb72733a46aae3c717e68cf2a26f538930f05488165b7cf6bc7db803",
            "afa0f047eb72733a46aae3c717e68cf2a26f538930f05488165b7cf6bc7db803",
            "afa0f047eb72733a46aae3c717e68cf2a26f538930f05488165b7cf6bc7db803"
        );
        check_versions!(
            "nocurve-two-event-reject",
            size,
            &two_event_xy,
            &two_event_pressure,
            &two_event_times,
            2,
            1920,
            1080,
            1,
            "b13cb702eb94fd20a64eff2ba72ac0f9623c9de02805655a6e89567adffcb559",
            "c7ab1c10a8f718a8facf9507f2f11716ac4121527aad001ae1d2acb6b53003b5",
            "c7ab1c10a8f718a8facf9507f2f11716ac4121527aad001ae1d2acb6b53003b5",
            "c7ab1c10a8f718a8facf9507f2f11716ac4121527aad001ae1d2acb6b53003b5"
        );
        check_versions!(
            "nocurve-negative-pressure",
            size,
            &negative_pressure_xy,
            &negative_pressure,
            &three_times,
            2,
            1920,
            1080,
            10,
            "1c4f56a16560dfa8e0ba5067ba2c5ee1172c79458a62eb454f8822e3313d8754",
            "e5d8987fd489d1007aa494b8b335fceb7eea5d433d9c69befab4bc82cbe64dee",
            "e5d8987fd489d1007aa494b8b335fceb7eea5d433d9c69befab4bc82cbe64dee",
            "e5d8987fd489d1007aa494b8b335fceb7eea5d433d9c69befab4bc82cbe64dee"
        );
        check_versions!(
            "nocurve-zero-pressure",
            size,
            &negative_pressure_xy,
            &zero_pressure,
            &three_times,
            2,
            1920,
            1080,
            10,
            "42064788667d5ea530bcc78e08f98bb63762cbb3585622c3f9b98cff30f558e5",
            "351cefbf739174a7d8575a0b51bfe69d6f7b43102f3cb1130a097617830048a4",
            "351cefbf739174a7d8575a0b51bfe69d6f7b43102f3cb1130a097617830048a4",
            "351cefbf739174a7d8575a0b51bfe69d6f7b43102f3cb1130a097617830048a4"
        );
        check_versions!(
            "nocurve-edge-components",
            size,
            &edge_components_xy,
            &edge_components_pressure,
            &edge_components_times,
            2,
            1920,
            1080,
            6,
            "57f9d390bf1bfe6d7c4dae739394df7a54a318b206ee6b043941b2d0b85f1c4f",
            "3ff06733643a0888653e476a2a0abbc9f0540fc04bad72cf405fdfd0e4d7739a",
            "3ff06733643a0888653e476a2a0abbc9f0540fc04bad72cf405fdfd0e4d7739a",
            "737856a23c061f86d19b0ffed447ab5c7bcd5e73032321f288a22db3330b34e2"
        );
        check_versions!(
            "nocurve-edge-diagonal",
            size,
            &edge_diagonal_xy,
            &edge_diagonal_pressure,
            &edge_diagonal_times,
            2,
            1920,
            1080,
            6,
            "764308e79a54769b4d182901d11bd98ed9ee7d57c1e48751af439204b5d61381",
            "ccf88ba115be5a7bc40a4295064c5f88805bce92f1da7f35d41abe97f8a10079",
            "ccf88ba115be5a7bc40a4295064c5f88805bce92f1da7f35d41abe97f8a10079",
            "9d3396c454905b535270e3e0caae35026957e4c9a468ef84a0e9c6baa54d6c42"
        );
        check_versions!(
            "nocurve-screen-gate",
            size,
            &screen_xy,
            &screen_pressure,
            &three_times,
            2,
            1440,
            2560,
            5,
            "f57c11dabd30e3f4d936102d5cbdb21427b1bab43ed9498340b4d4dcc3e2b5ae",
            "f21a46aa5c0c41dce850cf46f2c2057881f28df9b9e2b3fa884e1ef285ca605f",
            "f21a46aa5c0c41dce850cf46f2c2057881f28df9b9e2b3fa884e1ef285ca605f",
            "f21a46aa5c0c41dce850cf46f2c2057881f28df9b9e2b3fa884e1ef285ca605f"
        );
        check_versions!(
            "nocurve-all-identical",
            size,
            &identical_xy,
            &identical_pressure,
            &identical_times,
            2,
            1920,
            1080,
            1,
            "b1bfb2a50318e47aebb2170201e61b62fab6b7057d4438dbf248b1ea0e1c7c54",
            "6c6ea73194e27d1bac22f0083503a7d74c8d1dc55204ccc377debaca0078dd23",
            "6c6ea73194e27d1bac22f0083503a7d74c8d1dc55204ccc377debaca0078dd23",
            "6c6ea73194e27d1bac22f0083503a7d74c8d1dc55204ccc377debaca0078dd23"
        );
        check_versions!(
            "nocurve-duplicate-terminal",
            size,
            &duplicate_terminal_xy,
            &duplicate_terminal_pressure,
            &duplicate_terminal_times,
            2,
            1920,
            1080,
            5,
            "267f07456725cc3dc28bb98a64d1a67fd327371714cf3314b14ea69de956e926",
            "a1b055b6c7b26c4164bb6b1ced962d87a9265234cf5d9d2ac30820d9b0285389",
            "a1b055b6c7b26c4164bb6b1ced962d87a9265234cf5d9d2ac30820d9b0285389",
            "a1b055b6c7b26c4164bb6b1ced962d87a9265234cf5d9d2ac30820d9b0285389"
        );
        check_versions!(
            "nocurve-out-back",
            size,
            &out_back_xy,
            &out_back_pressure,
            &out_back_times,
            2,
            1920,
            1080,
            10,
            "6839137f900fe936c680c5dde95b1cece80f74ff1987b45185ece4c6b5439864",
            "ea0207cdce92d877531f3c489954b334359510138c146c09fb2cfaef1f2e4453",
            "ea0207cdce92d877531f3c489954b334359510138c146c09fb2cfaef1f2e4453",
            "ea0207cdce92d877531f3c489954b334359510138c146c09fb2cfaef1f2e4453"
        );
        check_versions!(
            "nocurve-tool1",
            size,
            &rejected_final_xy,
            &rejected_final_pressure,
            &rejected_final_times,
            1,
            1920,
            1080,
            13,
            "d27cfa3a6ca81d6314f2d7afd76344f7c4df20ca1c52df64c127afa0fa7e7aad",
            "1ea82381a48266228f43f1edc1942e8d4e49ec3988788af9dd84110defd7e08a",
            "1ea82381a48266228f43f1edc1942e8d4e49ec3988788af9dd84110defd7e08a",
            "1ea82381a48266228f43f1edc1942e8d4e49ec3988788af9dd84110defd7e08a"
        );

        let dot = run_model_bounded(
            &all_rejected_xy,
            &all_rejected_pressure,
            &all_rejected_times,
            size,
            3,
            1920,
            1080,
            2,
            false,
            1,
        )
        .unwrap();
        assert_eq!(
            dot[0].map(f32::to_bits),
            [0x4000_0000, 0x4000_0000, 0x3fa0_f90a, 0, 0],
            "native all-rejected dot uses last point, half-pressure width and zero scalar"
        );
        let dot_tool1 = run_model_bounded(
            &all_rejected_xy,
            &all_rejected_pressure,
            &all_rejected_times,
            size,
            3,
            1920,
            1080,
            1,
            false,
            1,
        )
        .unwrap();
        assert_eq!(
            dot_tool1[0].map(f32::to_bits),
            [0x4000_0000, 0x4000_0000, 0x3f49_374c, 0, 0],
            "native dot uses the tool-1 effective pressure"
        );
        let dot_large_size = run_model_bounded(
            &all_rejected_xy,
            &all_rejected_low_pressure,
            &all_rejected_times,
            8.333333,
            3,
            1920,
            1080,
            2,
            false,
            1,
        )
        .unwrap();
        assert_eq!(
            dot_large_size[0].map(f32::to_bits),
            [0x4000_0000, 0x4000_0000, 0x3fa0_0000, 0, 0],
            "native dot width scales with size and last pressure"
        );
        let terminal = run_model_bounded(
            &rejected_final_xy,
            &rejected_final_pressure,
            &rejected_final_times,
            size,
            3,
            1920,
            1080,
            2,
            false,
            13,
        )
        .unwrap();
        assert_eq!(
            terminal.last().unwrap().map(f32::to_bits),
            [0x4185_5555, 0, 0x3fb5_182a, 0x40a0_0000, 0x4316_0000],
            "native terminal line samples toward the raw endpoint using its pressure target"
        );
    }

    #[test]
    fn source_model_matches_native_glv1_glv2_glv3_full_vectors() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/chinesebrush-source-native.json"
        ))
        .unwrap();
        assert_eq!(fixture["schema"], "chinesebrush-source-native-v1");
        assert_eq!(fixture["cases"].as_object().unwrap().len(), 52);
        check_fixture(&fixture);
    }

    #[test]
    fn source_model_matches_native_adversarial_precision_and_speed_cases() {
        let baseline: Value = serde_json::from_str(include_str!(
            "../../../../conformance/chinesebrush-source-native.json"
        ))
        .unwrap();
        let extension: Value = serde_json::from_str(include_str!(
            "../../../../conformance/chinesebrush-source-extension-native.json"
        ))
        .unwrap();
        assert_eq!(
            extension["schema"],
            "chinesebrush-source-extension-native-v1"
        );
        assert_eq!(extension["cases"].as_object().unwrap().len(), 24);
        assert_eq!(check_fixture(&baseline) + check_fixture(&extension), 953);
    }

    fn bounded(
        xy: &[[f32; 2]],
        pressure: &[f32],
        times: &[i32],
        curve: bool,
        record_limit: usize,
    ) -> Result<Vec<[f32; 5]>, SampleError> {
        run_model_bounded(
            xy,
            pressure,
            times,
            20.0,
            2,
            1000,
            1000,
            0,
            curve,
            record_limit,
        )
    }

    #[test]
    fn bounded_sampler_accepts_exact_output_boundary_and_rejects_one_below() {
        let xy = [[0.0, 0.0]];
        let pressure = [0.5];
        let times = [1];

        let exact = bounded(&xy, &pressure, &times, false, 1).unwrap();
        assert_eq!(exact.len(), 1);
        assert_eq!(
            bounded(&xy, &pressure, &times, false, 0),
            Err(SampleError::OutputBudgetExceeded)
        );
    }

    #[test]
    fn bounded_sampler_rejects_huge_paths_without_partial_records() {
        let xy = [[0.0, 0.0], [1.0e18, 0.0]];
        let pressure = [0.5, 0.5];
        let times = [1, 2];

        assert_eq!(
            bounded(&xy, &pressure, &times, true, MAX_OUTPUT_RECORDS),
            Err(SampleError::OutputBudgetExceeded)
        );
    }

    #[test]
    fn bounded_sampler_rejects_empty_parameterization_without_panicking() {
        // At this small display gate the movements are accepted, but the
        // closed terminal cubic flattens to a zero chord and no segments.
        assert_eq!(
            run_model_bounded(
                &[[0.0, 0.0], [0.1, 0.0], [0.0, 0.0]],
                &[0.5; 3],
                &[1, 2, 3],
                20.0,
                3,
                1,
                1,
                2,
                true,
                100,
            ),
            Err(SampleError::DegeneratePath)
        );
    }

    #[test]
    fn bounded_sampler_rejects_invalid_arrays_and_timestamp_overflow_span() {
        let xy = [[0.0, 0.0], [10.0, 0.0]];
        let pressure = [0.5, 0.5];
        let times = [1, 2];
        assert_eq!(
            bounded(&xy, &pressure[..1], &times, true, 100),
            Err(SampleError::InvalidSource)
        );

        let non_finite = [[0.0, 0.0], [f32::NAN, 0.0]];
        assert_eq!(
            bounded(&non_finite, &pressure, &times, true, 100),
            Err(SampleError::InvalidSource)
        );

        let wide_span = [i32::MIN, i32::MAX];
        assert_eq!(
            bounded(&xy, &pressure, &wide_span, true, 100),
            Err(SampleError::InvalidSource)
        );
    }

    #[test]
    fn bounded_sampler_preserves_native_negative_pressure_edge_case() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/chinesebrush-source-extension-native.json"
        ))
        .unwrap();
        let case = &fixture["cases"]["adv-pressure-edges-v3"];
        let (_, pressure, _) = source_arrays(&case["input"]);
        assert!(pressure.iter().any(|value| *value < 0.0));
        check_fixture(&serde_json::json!({"cases": {"negative-pressure": case}}));
    }

    #[test]
    fn bounded_sampler_matches_long_native_glv123_traces() {
        let cases = [
            (
                "stressmono-32-v3-size3_144",
                32,
                3.144_f64 as f32,
                false,
                27,
                "5e5dd289f19c8bfc465a2c5f23ec2b7532afb9e1cb6b1b5d30234f96ddfa42f5",
                "ec9b78aaf737d17294c344a5b8a93f927e6d759422d2745c7dfddb65fd8661d3",
            ),
            (
                "stressmono-32-v3-size24",
                32,
                24.0_f32,
                false,
                16,
                "5e5dd289f19c8bfc465a2c5f23ec2b7532afb9e1cb6b1b5d30234f96ddfa42f5",
                "ce792982cf154160dac05e43c59889180c165b41b8e0595a9cf6e7abac858f72",
            ),
            (
                "stressmono-64-v3-size3_144",
                64,
                3.144_f64 as f32,
                false,
                54,
                "f01d96b338d891520398af32a951fc89cf37fc611130f69aeab6ada43c640c2b",
                "de7a84d07eb3ecffcf87e69b2eaf66b0c7b77229203ef50f1a08b465d786ed1b",
            ),
            (
                "stressmono-64-v3-size24",
                64,
                24.0_f32,
                false,
                34,
                "f01d96b338d891520398af32a951fc89cf37fc611130f69aeab6ada43c640c2b",
                "03166eb65313f11e96731c86d99d1b4248e3683eca3a3d51e779d1ee00ec71e2",
            ),
            (
                "stressmono-128-v3-size3_144",
                128,
                3.144_f64 as f32,
                false,
                106,
                "13c5416ed965a8d36f1628c9d99014961c51f527aa475cb7df498fecaedf8686",
                "cc25fbd761e0690c4296c0dde4272fa12a2f129a29a49b39bc9f06eab98b6d9f",
            ),
            (
                "stressmono-128-v3-size24",
                128,
                24.0_f32,
                false,
                64,
                "13c5416ed965a8d36f1628c9d99014961c51f527aa475cb7df498fecaedf8686",
                "dfc3df66c71d80ca8df00ad5accd5def1e477430fe6f505a4bb178deb00db543",
            ),
            (
                "stressmono-32-v3-size24-shifted",
                32,
                24.0_f32,
                true,
                16,
                "da1a0004d2d34e990b0404d5a88de92011dfbbeb02399cbb7d4321a176fe0708",
                "e8f53f206f07201f8092419e60b750ce861cdf2ad1f83cb7bd8ad5feec95296e",
            ),
        ];

        for (name, count, size, shifted, expected_count, input_sha, output_sha) in cases {
            let (xy, pressure, times) = stress_source(count, shifted);
            check_native_trace(
                name,
                &xy,
                &pressure,
                &times,
                (size, 3),
                (expected_count, input_sha, output_sha),
            );
        }

        let source_sha = "5e5dd289f19c8bfc465a2c5f23ec2b7532afb9e1cb6b1b5d30234f96ddfa42f5";
        let v1_v2_cases = [
            (
                "terminal-v1-32",
                1,
                27,
                "2e374d6db19b25ccb27f33516bfd623bcfb0db9122f65d76d9f4cae5637190b2",
            ),
            (
                "terminal-v2-32",
                2,
                27,
                "ec9b78aaf737d17294c344a5b8a93f927e6d759422d2745c7dfddb65fd8661d3",
            ),
        ];
        for (name, version, expected_count, output_sha) in v1_v2_cases {
            let (xy, pressure, times) = stress_source(32, false);
            check_native_trace(
                name,
                &xy,
                &pressure,
                &times,
                (3.144_f64 as f32, version),
                (expected_count, source_sha, output_sha),
            );
        }
    }
}

#[cfg(test)]
mod synthetic_archive_regression {
    use super::*;
    use sha2::{Digest, Sha256};

    // Synthetic authored note table (IDs 0/1 resolve ChineseBrush + `3;`)
    // wraps the native serialized ObjectStroke payload from
    // conformance/object-stroke-channels-native.json, SHA-256
    // 794c365ed0999439c289a45713ba2a33a23d19e58324acfb8e3956564bf1c37.
    // The sampler profile is explicitly synthetic: 1920 x 1080.
    #[test]
    fn synthetic_sdocx_resolves_and_matches_native_chinesebrush_records() {
        let archive = include_bytes!("../../tests/assets/chinesebrush/synthetic_v3.sdocx");
        let oracle = include_bytes!("../../tests/assets/chinesebrush/native_packed.bin");
        let document = crate::parse_bytes(archive).expect("synthetic ChineseBrush archive");
        assert_eq!(document.pages.len(), 1);
        let strokes: Vec<_> = document.pages[0].strokes().collect();
        assert_eq!(strokes.len(), 1);
        let stroke = strokes[0];
        let rendering = stroke.rendering.as_ref().expect("saved rendering metadata");
        assert_eq!(
            rendering.pen_name.as_deref(),
            Some("com.samsung.android.sdk.pen.pen.preload.ChineseBrush")
        );
        assert_eq!(rendering.advanced_settings.as_deref(), Some("3;"));
        assert_eq!(rendering.tool_type_raw, 2);
        assert!(rendering.properties.compressed);
        assert!(rendering.properties.stylus_channels);
        assert!(rendering.properties.millisecond_timestamps);
        assert_eq!(rendering.style.stroke_type_raw.unwrap_or(0), 0);
        assert!(!rendering.properties.eraser && !rendering.properties.fixed_width);
        assert!(!rendering.properties.fixed_opacity && !rendering.properties.rainbow_effect);
        assert!(!rendering.properties.alpha_lock && !rendering.properties.straighten);
        assert!(!rendering.properties.reveal_mode && !rendering.properties.replay_only);

        let version = match rendering.advanced_settings.as_deref() {
            Some("1;") => 1,
            Some("2;") => 2,
            Some("3;") => 3,
            _ => panic!("unsupported ChineseBrush version setting"),
        };
        let size = rendering.style.pen_size.expect("native saved pen size");
        assert_eq!(size.to_bits(), 0x4111_de6a);
        let points: Vec<[f32; 2]> = stroke
            .points
            .iter()
            .map(|point| [point.x as f32, point.y as f32])
            .collect();
        let pressures: Vec<f32> = stroke.pressures.iter().map(|value| *value as f32).collect();
        let timestamps: Vec<i32> = stroke
            .timestamps
            .iter()
            .map(|value| i32::try_from(*value).expect("native timestamp fits i32"))
            .collect();
        let tilts: Vec<f32> = stroke.tilts.iter().map(|value| *value as f32).collect();
        let orientations: Vec<f32> = stroke
            .orientations
            .iter()
            .map(|value| *value as f32)
            .collect();
        assert_eq!(points.len(), 204);
        assert_eq!(pressures.len(), 204);
        assert_eq!(timestamps.len(), 204);
        assert_eq!(tilts.len(), 204);
        assert_eq!(orientations.len(), 204);

        let digest = |bytes: &[u8]| format!("{:x}", Sha256::digest(bytes));
        let point_bytes: Vec<u8> = points
            .iter()
            .flat_map(|xy| xy.iter().flat_map(|v| v.to_le_bytes()))
            .collect();
        let pressure_bytes: Vec<u8> = pressures.iter().flat_map(|v| v.to_le_bytes()).collect();
        let timestamp_bytes: Vec<u8> = timestamps.iter().flat_map(|v| v.to_le_bytes()).collect();
        let tilt_bytes: Vec<u8> = tilts.iter().flat_map(|v| v.to_le_bytes()).collect();
        let orientation_bytes: Vec<u8> =
            orientations.iter().flat_map(|v| v.to_le_bytes()).collect();
        assert_eq!(
            digest(&point_bytes),
            "45bce4b3399b64a0eaa121214fda0ed6aa4dbb8e8db888b1fc0d1f1a768fa5f4"
        );
        assert_eq!(
            digest(&pressure_bytes),
            "4039d46b4f7c86035b24b07d2bfe7bb11aa853009bdd18e2a7872ef5a9ad37f9"
        );
        assert_eq!(
            digest(&timestamp_bytes),
            "c3a666ad17fb6230076b651dd27ebe61d321f97c9a337a9f4a769ca98401829a"
        );
        assert_eq!(
            digest(&tilt_bytes),
            "e56bb1cbfb7892cb63900be7f13027444ce6bf72801106d3ac805647315a18c6"
        );
        assert_eq!(
            digest(&orientation_bytes),
            "751141dbf512853aa7662510b8b5f4b9cbd53213b91eead31529113a0248ba0b"
        );

        let rows = run_model_bounded(
            &points,
            &pressures,
            &timestamps,
            size,
            version,
            1920,
            1080,
            i32::from(rendering.tool_type_raw),
            rendering.properties.compressed,
            MAX_OUTPUT_RECORDS,
        )
        .expect("bounded sampler accepts parsed synthetic source");
        let packed: Vec<u8> = rows
            .iter()
            .flat_map(|row| {
                [row[0], row[1], row[2], packed_w(row[3], row[4])]
                    .into_iter()
                    .flat_map(f32::to_le_bytes)
            })
            .collect();
        assert_eq!(rows.len(), 33);
        assert_eq!(packed.as_slice(), oracle);
    }
}
