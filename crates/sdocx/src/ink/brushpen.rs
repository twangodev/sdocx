//! Test-only BrushPen GLV10 -> RTV1 instance packing.
//!
//! The saved-native oracles execute the pinned GLV10 redraw path and record
//! the arguments at each RTV1 `AddPoint` call. This leaf replays captured
//! MotionEvent and ObjectStroke source channels through path construction,
//! radius and opacity state, and stamp sampling, then models the 11-float
//! records written by those calls. Constructor profiles and event shapes are
//! narrow; saved device samples remain outside this module. The paint child
//! joins this sampler to one manually configured native caller profile and a
//! conditional host-Mesa frame; it is not a production dispatch route.

#[cfg(feature = "serde")]
mod paint;

use super::path::{P, Quad};

const GL_VERSION: u32 = 10;
const MAX_SAMPLES: usize = 1_000_000;
const TEXTURE_OPACITY_SCALE: f64 = 0.001;
const GLV10_PRESSURE_SCALE: f32 = 1.666_666_6;
const GLV10_PRESSURE_MIN: f32 = 0.01;
const GLV10_MOTION_DISTANCE_SCALE: f32 = 10_000.0;
const GLV10_MOTION_SCALAR_DELTA_MAX: f32 = 5.0;
const GLV10_FIRST_TIME_FALLBACK_SCALE: f32 = 1.23;
const GLV10_FIRST_TIME_FALLBACK_NUMERATOR: f32 = 50_000.0;
const GLV10_RADIUS_BIAS: f64 = 0.34;
const GLV10_RADIUS_SCALE: f64 = 0.769_999_980_926_513_7;
const GLV10_RADIUS_STATE_SMOOTHING: f32 = 0.6;
const GLV10_RADIUS_STATE_PRIOR_WEIGHT: f32 = f32::from_bits(0x3ecc_cccc);
const GLV10_OPACITY_TARGET_SMOOTHING: f32 = 0.6;
const GLV10_OPACITY_PRIOR_WEIGHT: f32 = f32::from_bits(0x3ecc_cccc);
const GLV10_RADIUS_STATE_FLOOR: f32 = 2.0;
const GLV10_END_PEN_ALPHA_DISTANCE_SCALE: f32 = 60.0;
const GLV10_PATH_PARAMETER_SCALE: f32 = f32::from_bits(0x3800_00fd);

/// Settings traced from this fixture's fresh BrushPen constructor. The saved
/// cases do not call the size setter, so these defaults are deliberately kept
/// as an explicit profile instead of treating GLV10 as sufficient admission.
#[derive(Clone, Copy, PartialEq)]
struct Glv10RuntimeProfile {
    runtime_size: f32,
    pressure_scale: f32,
    pressure_min: f32,
    radius_bias: f64,
    radius_scale: f64,
    radius_state_smoothing: f32,
    radius_state_prior_weight: f32,
    radius_state_floor: f32,
    initial_motion_width: f32,
    first_draw_width: f32,
    steady_draw_width: f32,
    fixed_width_enabled: bool,
    fixed_width: f32,
    constructor_pen_angle: f32,
    constructor_pen_tilt: f32,
    neutral_tilt_min: f32,
    constructor_opacity: f32,
    motion_distance_threshold: f32,
    /// Native per-pen limits updated by the Drawing screen-resolution setter.
    /// GLV10 scales drawLine speed by 1440 / min(limit0, limit1).
    motion_settings_limits: [i32; 2],
}

const SAVED_GLV10_CONSTRUCTOR_PROFILE: Glv10RuntimeProfile = Glv10RuntimeProfile {
    runtime_size: 3.144,
    pressure_scale: GLV10_PRESSURE_SCALE,
    pressure_min: GLV10_PRESSURE_MIN,
    radius_bias: GLV10_RADIUS_BIAS,
    radius_scale: GLV10_RADIUS_SCALE,
    radius_state_smoothing: GLV10_RADIUS_STATE_SMOOTHING,
    radius_state_prior_weight: GLV10_RADIUS_STATE_PRIOR_WEIGHT,
    radius_state_floor: GLV10_RADIUS_STATE_FLOOR,
    initial_motion_width: 0.77,
    first_draw_width: 0.88,
    steady_draw_width: 1.0,
    fixed_width_enabled: false,
    fixed_width: 0.0,
    constructor_pen_angle: -0.05,
    constructor_pen_tilt: 0.68,
    neutral_tilt_min: 0.0001,
    constructor_opacity: 255.0,
    motion_distance_threshold: 2.6,
    motion_settings_limits: [1440, 2560],
};

/// SmPath cubic representation selected by endPen's retained path flag.
/// Subdivision and parameter interpolation mirror the pinned native cubic
/// helper and the existing GLV1 cubic model.
pub(super) struct Cubic {
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

    fn parameter(&self, distance: f32) -> f32 {
        let index = self
            .segments
            .partition_point(|&(end, _)| end < distance)
            .min(self.segments.len() - 1);
        let (start, t0) = if index == 0 {
            (0.0, 0)
        } else {
            self.segments[index - 1]
        };
        let (end, t1) = self.segments[index];
        let scale = f32::from_bits(0x3800_00fd);
        let t0 = t0 as f32 * scale;
        let t1 = t1 as f32 * scale;
        t0 + (distance - start) * (t1 - t0) / (end - start)
    }

    fn at(&self, distance: f32) -> P {
        let t = self.parameter(distance);
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
        P {
            x: eval(self.p[0].x, self.p[1].x, self.p[2].x, self.p[3].x),
            y: eval(self.p[0].y, self.p[1].y, self.p[2].y, self.p[3].y),
        }
    }

    fn normalized_tangent(&self, distance: f32) -> P {
        let t = self.parameter(distance);
        // Native cubic getPosTan uses its expanded derivative polynomial,
        // without the common factor three.
        let component = |a: f32, b: f32, c: f32, d: f32| {
            let first = b - a;
            let linear = ((-2.0_f32).mul_add(b, c) + a) * 2.0;
            let quadratic = 3.0_f32.mul_add(b - c, d) - a;
            let curvature = quadratic.mul_add(t, linear);
            curvature.mul_add(t, first)
        };
        let mut tangent = P {
            x: component(self.p[0].x, self.p[1].x, self.p[2].x, self.p[3].x),
            y: component(self.p[0].y, self.p[1].y, self.p[2].y, self.p[3].y),
        };
        let squared = tangent.x.mul_add(tangent.x, tangent.y * tangent.y);
        if squared > 2.0_f32.powi(-24) {
            let inverse = 1.0_f32 / squared.sqrt();
            tangent.x *= inverse;
            tangent.y *= inverse;
        }
        tangent
    }
}

#[derive(Clone, Copy)]
pub(super) enum EndPath<'a> {
    Line { from: P, to: P },
    Quad(&'a Quad),
    Cubic(&'a Cubic),
}

impl EndPath<'_> {
    fn length(&self) -> f32 {
        match self {
            Self::Line { from, to } => to.sub(*from).len(),
            Self::Quad(path) => path.length,
            Self::Cubic(path) => path.length,
        }
    }

    fn at(&self, distance: f32) -> P {
        match self {
            Self::Line { from, to } => {
                let length = to.sub(*from).len();
                if length == 0.0 {
                    *to
                } else {
                    // SmPath stores line endpoints as 15-bit path parameters.
                    // getPosTan obtains t through helper_distanceToSegment's
                    // separate f32 multiply/divide/add sequence, then uses a
                    // fused vector FMLA for the point itself.
                    let t1 = 32767.0_f32 * GLV10_PATH_PARAMETER_SCALE;
                    let scaled = distance * t1;
                    let divided = scaled / length;
                    let t = 0.0_f32 + divided;
                    P {
                        x: (to.x - from.x).mul_add(t, from.x),
                        y: (to.y - from.y).mul_add(t, from.y),
                    }
                }
            }
            Self::Quad(path) => path.at(distance),
            Self::Cubic(path) => path.at(distance),
        }
    }

    fn normalized_tangent(&self, distance: f32) -> P {
        match self {
            Self::Line { from, to } => {
                let delta = to.sub(*from);
                let length = delta.len();
                if length == 0.0 {
                    P::default()
                } else {
                    P {
                        x: delta.x / length,
                        y: delta.y / length,
                    }
                }
            }
            Self::Quad(path) => path.normalized_tangent(distance),
            Self::Cubic(path) => path.normalized_tangent(distance),
        }
    }
}

enum DrawLinePath {
    Line { from: P, to: P },
    Quad(Quad),
}

impl DrawLinePath {
    fn as_end_path(&self) -> EndPath<'_> {
        match self {
            Self::Line { from, to } => EndPath::Line {
                from: *from,
                to: *to,
            },
            Self::Quad(path) => EndPath::Quad(path),
        }
    }

    fn length(&self) -> f32 {
        self.as_end_path().length()
    }
}

/// Build one smoothed path for RedrawPen's historical-event loop. The first
/// event has the initial point duplicated as its quadratic control; later
/// events continue from the previous pair's midpoint, through its latest
/// source point, to the next midpoint.
fn motion_history_path(points: &[P], event_index: usize) -> Option<Quad> {
    if points.len() < 3 || event_index == 0 || event_index >= points.len() - 1 {
        return None;
    }
    if event_index == 1 {
        Some(Quad::new(points[0], points[0], points[0].mid(points[1])))
    } else {
        Some(Quad::new(
            points[event_index - 2].mid(points[event_index - 1]),
            points[event_index - 1],
            points[event_index - 1].mid(points[event_index]),
        ))
    }
}

struct MotionHistoryTransition {
    event_index: usize,
    previous: P,
    current: P,
    elapsed_ms: f32,
    prior_speed: f32,
    smoothed_speed: f32,
    path: Quad,
}

struct AdmittedMotionTransition {
    event_index: usize,
    accepted_current: P,
    speed: f32,
    smoothed_speed: f32,
    path: DrawLinePath,
}

struct AdmittedMotionReplay {
    transitions: Vec<AdmittedMotionTransition>,
    terminal_flag226: u8,
    fallback_time_counter: f32,
}

/// Replay the historical-event loop in GLV10::redraw. It starts after the
/// initial point and consumes indices 1 through len-2; the final current
/// MotionEvent is reserved for endPen.
fn motion_history_transitions(
    profile: Glv10RuntimeProfile,
    points: &[P],
    times: &[i64],
) -> Option<Vec<MotionHistoryTransition>> {
    if points.len() < 3 || times.len() != points.len() {
        return None;
    }
    let mut prior_speed = 0.0;
    let mut transitions = Vec::with_capacity(points.len() - 2);
    for event_index in 1..points.len() - 1 {
        let previous = points[event_index - 1];
        let current = points[event_index];
        let elapsed_ms = (times[event_index] - times[event_index - 1]) as f32;
        let (speed, smoothed_speed) =
            motion_line_scalar(profile, previous, current, prior_speed, elapsed_ms)?;
        transitions.push(MotionHistoryTransition {
            event_index,
            previous,
            current,
            elapsed_ms,
            prior_speed,
            smoothed_speed,
            path: motion_history_path(points, event_index)?,
        });
        prior_speed = speed;
    }
    Some(transitions)
}

/// Apply drawLine's movement gate to source history transitions. The native
/// first comparison at 0x5c374..380 rejects only a short move after the first
/// accepted path has established +225/+227 state. The second comparison at
/// 0x5c398..3e0 has its own +226 toggle. Accepted paths update the stored
/// endpoint. A gate rejection returns before the captured point/time/speed
/// state writes, so the next candidate is measured from the last admission.
fn admitted_motion_transitions(
    profile: Glv10RuntimeProfile,
    points: &[P],
    times: &[i64],
    tool_type: i32,
    curve_enabled: bool,
) -> Option<AdmittedMotionReplay> {
    if points.len() < 3 || times.len() != points.len() {
        return None;
    }
    let mut accepted_previous = *points.first()?;
    let mut accepted_current = accepted_previous;
    let mut accepted_index = 0;
    let mut prior_speed = 0.0_f32;
    let mut has_accepted_path = false;
    let (mut flag225, mut flag226, mut flag227) = (1_u8, 0_u8, 0_u8);
    let mut admitted = Vec::with_capacity(points.len() - 2);
    let threshold336 = if matches!(tool_type, 1 | 3) {
        50.0
    } else {
        10.4
    };
    let mut fallback_time_counter = 1.0_f32;
    for event_index in 1..points.len() - 1 {
        let current = points[event_index];
        let delta = current.sub(accepted_current);
        let distance = delta.len();
        let first_path = flag225 != 0;
        // AArch64 FCMP/CCMP chain: unordered does not trigger this rejection.
        let first_gate_reject =
            distance < profile.motion_distance_threshold && flag225 == 0 && flag227 != 0;
        if first_gate_reject {
            continue;
        }

        // The captured fresh constructor derives +336 from the same scaled
        // size profile as +44. Keep a distinct field because the native paths
        // compare these offsets independently.
        if distance < threshold336 {
            let old = flag226;
            flag226 ^= 1;
            if old != 0 {
                continue;
            }
        } else {
            flag226 = 1;
        }

        let path = if !curve_enabled {
            DrawLinePath::Line {
                from: accepted_current,
                to: current,
            }
        } else if has_accepted_path {
            DrawLinePath::Quad(Quad::new(
                accepted_previous.mid(accepted_current),
                accepted_current,
                accepted_current.mid(current),
            ))
        } else {
            DrawLinePath::Quad(Quad::new(
                accepted_current,
                accepted_current,
                accepted_current.mid(current),
            ))
        };
        let previous_time = times[accepted_index];
        let current_time = times[event_index];
        let elapsed_us = if previous_time != 0 && current_time > previous_time {
            (current_time - previous_time) as f32 * 1000.0_f32
        } else {
            fallback_time_counter *= GLV10_FIRST_TIME_FALLBACK_SCALE;
            (GLV10_FIRST_TIME_FALLBACK_NUMERATOR / fallback_time_counter).trunc()
        };
        let (speed, smoothed_speed) =
            motion_line_scalar_us(profile, accepted_current, current, prior_speed, elapsed_us)?;
        admitted.push(AdmittedMotionTransition {
            event_index,
            accepted_current,
            speed,
            smoothed_speed,
            path,
        });
        accepted_previous = accepted_current;
        accepted_current = current;
        accepted_index = event_index;
        prior_speed = speed;
        has_accepted_path = true;
        // The first admitted drawPattern clears the toggle field after its
        // initial seed; later accepted lines retain the gate's toggle result.
        flag225 = 0;
        if first_path {
            flag226 = 0;
        }
        flag227 = 1;
    }
    Some(AdmittedMotionReplay {
        transitions: admitted,
        terminal_flag226: flag226,
        fallback_time_counter,
    })
}

/// RedrawPen's MotionEvent pressure choice before it calls GLV10::drawLine.
/// The native stylus/eraser tool types (1 and 3) keep the initialized 0.5
/// pressure; other tool types read the historical pressure and cap only its
/// upper bound at 1. NaN also retains 0.5 through the native unordered branch.
fn motion_pressure(tool_type: i32, historical_pressure: f32) -> f32 {
    if matches!(tool_type, 1 | 3) || historical_pressure.is_nan() {
        0.5
    } else if historical_pressure > 1.0 {
        1.0
    } else {
        historical_pressure
    }
}

/// Select and rotate the tilt state used by the captured GLV10 profiles. Tool
/// types 1 and 3 retain the drawable's initialized angle/magnitude; the other
/// profile uses its MotionEvent channels, with its observed neutral magnitude
/// of 1e-4. The signed x and positive y match the native `sincosf`/multiply
/// sequence before `drawPattern`; wider channel-transfer cases remain open.
fn motion_tilt(
    profile: Glv10RuntimeProfile,
    tool_type: i32,
    orientation: f32,
    tilt: f32,
) -> Option<[f32; 2]> {
    let (angle, magnitude) = if matches!(tool_type, 1 | 3) {
        (profile.constructor_pen_angle, profile.constructor_pen_tilt)
    } else {
        (orientation, tilt.max(profile.neutral_tilt_min))
    };
    if !angle.is_finite() || !magnitude.is_finite() {
        return None;
    }
    let (sin, cos) = angle.sin_cos();
    Some([-sin * magnitude, cos * magnitude])
}

/// Native drawLine's distance scalar. The DSO computes
/// `hypot(delta) * 10000 / (delta_time_ms * 1000) * .5`, clamps it to five units from the
/// prior scalar, stores that clamped step, and averages old/new for the
/// makeDrawPoint exponent. The first segment starts with prior scalar zero.
fn motion_line_scalar(
    profile: Glv10RuntimeProfile,
    previous: P,
    current: P,
    previous_scalar: f32,
    delta_time_ms: f32,
) -> Option<(f32, f32)> {
    if ![
        previous.x,
        previous.y,
        current.x,
        current.y,
        previous_scalar,
        delta_time_ms,
    ]
    .into_iter()
    .all(f32::is_finite)
        || delta_time_ms <= 0.0
    {
        return None;
    }
    motion_line_scalar_us(
        profile,
        previous,
        current,
        previous_scalar,
        delta_time_ms * 1000.0_f32,
    )
}

fn motion_line_scalar_us(
    profile: Glv10RuntimeProfile,
    previous: P,
    current: P,
    previous_scalar: f32,
    delta_time_us: f32,
) -> Option<(f32, f32)> {
    if ![
        previous.x,
        previous.y,
        current.x,
        current.y,
        previous_scalar,
        delta_time_us,
    ]
    .into_iter()
    .all(f32::is_finite)
        || delta_time_us <= 0.0
    {
        return None;
    }
    let dx = current.x - previous.x;
    let dy = current.y - previous.y;
    let distance = dx.mul_add(dx, dy * dy).sqrt();
    // GLV10 drawLine order at ELF 0x5c434..0x5c468: distance * 10000,
    // divide by elapsed microseconds, multiply by 0.5, then multiply by
    // 1440 / min(the two native settings limits). The Drawing caller updates
    // these limits from screen resolution; a fresh constructor retains 1440,
    // 2560. Keep the division and final multiply separate for f32 parity.
    let rate = (distance * GLV10_MOTION_DISTANCE_SCALE) / delta_time_us;
    let half_rate = rate * 0.5;
    let minimum_limit = profile.motion_settings_limits[0].min(profile.motion_settings_limits[1]);
    let normalization = if minimum_limit == 0 {
        1.0
    } else {
        1440.0_f32 / minimum_limit as f32
    };
    let mut next = half_rate * normalization;
    let change = next - previous_scalar;
    if change > GLV10_MOTION_SCALAR_DELTA_MAX {
        next = previous_scalar + GLV10_MOTION_SCALAR_DELTA_MAX;
    } else if change < -GLV10_MOTION_SCALAR_DELTA_MAX {
        next = previous_scalar - GLV10_MOTION_SCALAR_DELTA_MAX;
    }
    Some((next, (previous_scalar + next) * 0.5))
}

/// Source-side endpoint state selected by RedrawPen's history loop and
/// endPen's current-event read. For these captured profiles, drawLine has
/// consumed events through `points[len - 2]`; endPen compares that stored
/// point with the current event at `points[len - 1]`.
#[derive(Clone, Copy)]
struct MotionEndState {
    previous: P,
    current: P,
    elapsed_ms: f32,
    prior_speed: f32,
    terminal_scalar: f32,
}

/// EndPen starts from the last admitted drawLine endpoint/time, computes its
/// terminal scalar without drawLine's five-unit clamp, averages with the
/// stored drawLine speed, and multiplies by three for pen tools. Fixture times
/// are milliseconds; native converts their difference to microseconds.
fn motion_end_state(
    profile: Glv10RuntimeProfile,
    points: &[P],
    times: &[i64],
    tool_type: i32,
    curve_enabled: bool,
) -> Option<MotionEndState> {
    if !matches!(tool_type, 1 | 3) || points.len() < 3 || times.len() != points.len() {
        return None;
    }
    let history = admitted_motion_transitions(profile, points, times, tool_type, curve_enabled)?;
    let accepted = history.transitions.last()?;
    let prior = accepted.speed;
    let last = points.len() - 1;
    let previous = accepted.event_index;
    let elapsed = (times[last] - times[previous]) as f32;
    let delta_us = if times[previous] != 0 && times[last] > times[previous] {
        elapsed * 1000.0_f32
    } else {
        let counter = history.fallback_time_counter * GLV10_FIRST_TIME_FALLBACK_SCALE;
        (GLV10_FIRST_TIME_FALLBACK_NUMERATOR / counter).trunc()
    };
    if !delta_us.is_finite() || delta_us <= 0.0 {
        return None;
    }
    let delta = points[last].sub(points[previous]);
    let distance = delta.len();
    let raw_terminal_speed = (distance * GLV10_MOTION_DISTANCE_SCALE) / delta_us * 0.5_f32;
    let smoothed = (prior + raw_terminal_speed) * 0.5_f32;
    Some(MotionEndState {
        previous: points[previous],
        current: points[last],
        elapsed_ms: elapsed,
        prior_speed: prior,
        terminal_scalar: smoothed * 3.0,
    })
}

/// Recover the first GLV10 pattern spacing from a traced drawLine path and the
/// drawable's captured style constants. Native computes its style field as
/// `(2/(3*(1+expf((half_length-50)/30))) + .34) *
/// clamp(pressure*1.6666666, .01, 1) * .77`; makeDrawPoint seeds its spacing
/// radius with half that field.
fn motion_radius_seed(
    profile: Glv10RuntimeProfile,
    half_length_scalar: f32,
    pressure: f32,
) -> Option<f32> {
    if !pressure.is_finite()
        || !half_length_scalar.is_finite()
        || !profile.runtime_size.is_finite()
        || profile.runtime_size <= 0.0
    {
        return None;
    }
    let exponent = (half_length_scalar - 50.0) / 30.0;
    let exp = exponent.exp();
    let logistic = 2.0 / ((exp + 1.0) * 3.0);
    let pressure_scale = (pressure * profile.pressure_scale)
        .max(profile.pressure_min)
        .min(1.0);
    let style_field = (f64::from(logistic) + profile.radius_bias)
        * f64::from(pressure_scale)
        * profile.radius_scale;
    Some((style_field as f32) * 0.5)
}

/// GLV10::makeDrawPoint's persistent drawable+256 radius update. The native
/// code computes a logistic scalar in double, scales it by the per-event
/// interpolation `u` and drawable width factor, then blends 60% of that
/// size-scaled candidate with 40% of the previous radius and floors at 2.
fn update_motion_radius_state(
    profile: Glv10RuntimeProfile,
    prior_radius: f32,
    interpolation: f32,
    speed: f32,
    width_factor: f32,
) -> Option<f32> {
    if ![
        prior_radius,
        interpolation,
        speed,
        width_factor,
        profile.runtime_size,
        profile.radius_state_smoothing,
        profile.radius_state_prior_weight,
        profile.radius_state_floor,
    ]
    .into_iter()
    .all(f32::is_finite)
        || prior_radius < 0.0
        || interpolation < 0.0
        || width_factor < 0.0
        || profile.runtime_size <= 0.0
    {
        return None;
    }
    // Match the native instruction widths/order: expf and the logistic are
    // float32, then the bias/interpolation/width multiply is float64, and the
    // size scale, prior contribution and final FMA return to float32.
    let exponent = (speed - 50.0_f32) / 30.0_f32;
    let exp = exponent.exp();
    let denominator = (exp + 1.0_f32) * 3.0_f32;
    let logistic = 2.0_f32 / denominator;
    let q = ((f64::from(logistic) + profile.radius_bias)
        * f64::from(interpolation)
        * f64::from(width_factor)) as f32;
    let scaled = profile.runtime_size * q;
    let candidate = blend_motion_radius(prior_radius, scaled, profile);
    Some(if candidate < profile.radius_state_floor {
        profile.radius_state_floor
    } else {
        candidate
    })
}

/// Blend the input radius with the independent native `.4` prior weight.
fn blend_motion_radius(
    prior_radius: f32,
    scaled_candidate: f32,
    profile: Glv10RuntimeProfile,
) -> f32 {
    let prior_contribution = prior_radius * profile.radius_state_prior_weight;
    scaled_candidate.mul_add(profile.radius_state_smoothing, prior_contribution)
}

/// GLV10 makeDrawPoint's persistent integer opacity target. This follows the
/// native float32 exp/logistic, f32 quotient and clamp, then the exact
/// multiply-add and integer truncations observed in the pinned routine.
fn motion_opacity_target(
    speed: f32,
    interpolation: f32,
    width_factor: f32,
    prior_opacity: i32,
) -> Option<i32> {
    if ![speed, interpolation, width_factor]
        .into_iter()
        .all(f32::is_finite)
        || interpolation < 0.0
        || width_factor <= 0.0
    {
        return None;
    }
    let exp = ((speed - 50.0_f32) / 30.0_f32).exp();
    let logistic = 2.0_f32 / ((exp + 1.0_f32) * 3.0_f32);
    let q = ((f64::from(logistic) + GLV10_RADIUS_BIAS)
        * f64::from(interpolation)
        * f64::from(width_factor)) as f32;
    if !q.is_finite() || q <= 0.0 {
        return None;
    }
    let ratio = (speed / q).clamp(15.0, 80.0);
    let opacity_scale = 1.0_f32 - ((ratio - 15.0_f32) / 65.0_f32);
    let target = opacity_scale.mul_add(190.0_f32, 65.0_f32).trunc() as i32;
    let prior = prior_opacity as f32 * GLV10_OPACITY_PRIOR_WEIGHT;
    Some(
        (target as f32)
            .mul_add(GLV10_OPACITY_TARGET_SMOOTHING, prior)
            .trunc() as i32,
    )
}

/// Replay the persistent radius updates observed at each makeDrawPoint
/// epilogue. The initial prior radius is twice the constructor-profile seed;
/// each historical event then supplies pressure-scaled interpolation and the
/// smoothed speed state. Width factors reflect this fresh constructor's
/// initial drawLine state and subsequent transition.
fn motion_history_radius_states(
    profile: Glv10RuntimeProfile,
    points: &[P],
    times: &[i64],
    pressures: &[f32],
    tool_type: i32,
    curve_enabled: bool,
) -> Option<Vec<f32>> {
    if pressures.len() != points.len() {
        return None;
    }
    let history = admitted_motion_transitions(profile, points, times, tool_type, curve_enabled)?;
    let first = history.transitions.first()?;
    let initial_pressure = motion_pressure(tool_type, pressures[first.event_index]);
    let mut radius = motion_radius_seed(profile, first.smoothed_speed, initial_pressure)? * 2.0;
    let mut previous_width = profile.initial_motion_width;
    let mut states = Vec::with_capacity(history.transitions.len());
    for transition in history.transitions {
        let pressure = motion_pressure(tool_type, pressures[transition.event_index]);
        let interpolation = (pressure * profile.pressure_scale)
            .max(profile.pressure_min)
            .min(1.0);
        // drawLine stores .77 at 0x5c4e0, then compares the prior width to
        // that constructor constant and selects .88 or 1.0 at 0x5c63c.
        let width = if previous_width.to_bits() == profile.initial_motion_width.to_bits() {
            profile.first_draw_width
        } else {
            profile.steady_draw_width
        };
        radius = update_motion_radius_state(
            profile,
            radius,
            interpolation,
            transition.smoothed_speed,
            width,
        )?;
        previous_width = width;
        states.push(radius);
    }
    Some(states)
}

fn motion_history_radius_state(
    profile: Glv10RuntimeProfile,
    points: &[P],
    times: &[i64],
    pressures: &[f32],
    tool_type: i32,
    curve_enabled: bool,
) -> Option<f32> {
    motion_history_radius_states(profile, points, times, pressures, tool_type, curve_enabled)?
        .last()
        .copied()
}

/// Native makeEndPenPoint's target before its 0.5 multiplier. The runtime
/// style size and pressure are explicit because the saved fixture does not
/// exercise BrushPen's size setter.
fn motion_end_radius_target(
    profile: Glv10RuntimeProfile,
    half_length_scalar: f32,
    pressure: f32,
) -> Option<f32> {
    if ![half_length_scalar, profile.runtime_size, pressure]
        .into_iter()
        .all(f32::is_finite)
        || profile.runtime_size <= 0.0
        || pressure < 0.0
    {
        return None;
    }
    let exponent = ((half_length_scalar - 50.0) / 30.0).exp();
    let logistic = 2.0 / ((exponent + 1.0) * 3.0);
    let sized =
        (f64::from(logistic) + profile.radius_bias) * f64::from(profile.runtime_size * pressure);
    Some((sized as f32) * 0.5)
}

/// One explicit GLV10 sample as passed to `BrushPenStrokeDrawableRTV1::AddPoint`.
///
/// `vector` is `(x, y, radius, pattern_index)`. The remaining fields correspond
/// to the scalar opacity, tilt, direction, and trailing three-float argument
/// observed at the call boundary.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct AddPointInput {
    pub(super) vector: [f32; 4],
    pub(super) opacity: f32,
    pub(super) tilt: [f32; 2],
    pub(super) direction: [f32; 2],
    pub(super) auxiliary: [f32; 3],
}

/// RTV1 instance-buffer item: eleven float32 values, matching its 44-byte
/// per-instance vertex attribute stride.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Instance {
    pub(super) values: [f32; 11],
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Output {
    pub(super) instances: Vec<Instance>,
}

/// Explicit context for the one manually configured Drawing caller profile
/// joined to the retained 26x44 native buffer. This is test-only evidence,
/// not a general saved-stroke admission rule.
#[derive(Clone, Copy)]
struct SavedBrushPenNativeContext {
    gl_version: u32,
    screen_resolution: [i32; 2],
    constructor_profile: Glv10RuntimeProfile,
    fixed_width: f32,
    fixed_width_enabled: bool,
    fixed_opacity_enabled: bool,
}

struct SavedBrushPenSamples {
    output: Output,
    argb: u32,
}

/// Bridge an explicitly supplied public `Stroke` through the saved-caller
/// sampler profile. The fixture behind this bridge was assembled by manual
/// native ObjectStroke setters; it is not a parsed Drawing/document stroke.
/// Unknown pen identity, settings, flags, colors, sizes, or context fail closed.
fn sample_saved_native_stroke(
    stroke: &crate::Stroke,
    context: SavedBrushPenNativeContext,
) -> Option<SavedBrushPenSamples> {
    sample_native_stroke_with_size(stroke, context, 0x4049_374c, 0xff6c_dd8f)
}

fn sample_native_stroke_with_size(
    stroke: &crate::Stroke,
    context: SavedBrushPenNativeContext,
    expected_size_bits: u32,
    expected_argb: u32,
) -> Option<SavedBrushPenSamples> {
    const NATIVE_PEN_NAME: &str = "com.samsung.android.sdk.pen.pen.preload.BrushPen";

    if context.gl_version != GL_VERSION
        || !matches!(context.screen_resolution, [1920, 1080] | [1080, 1920])
        || context.constructor_profile != SAVED_GLV10_CONSTRUCTOR_PROFILE
        || context.fixed_width.to_bits() != 0.0_f32.to_bits()
        || context.fixed_width_enabled
        || context.fixed_opacity_enabled
    {
        return None;
    }

    let rendering = stroke.rendering.as_ref()?;
    let style = &rendering.style;
    let properties = rendering.properties;
    if rendering.pen_name.as_deref() != Some(NATIVE_PEN_NAME)
        || !matches!(rendering.advanced_settings.as_deref(), Some("" | "10;"))
        || rendering.tool_type_raw != 2
        || !properties.compressed
        || !properties.stylus_channels
        || properties.replay_only
        || properties.eraser
        || properties.fixed_width
        || properties.millisecond_timestamps
        || properties.top_layer_pen
        || properties.alpha_lock
        || properties.binary_added
        || properties.generated
        || properties.fixed_opacity
        || properties.rainbow_effect
        || properties.straighten
        || properties.reveal_mode
        || style.fixed_width.is_some()
        || style.size_level != Some(30)
        || style.stroke_type_raw != Some(0)
        || style.color_argb != Some(expected_argb)
    {
        return None;
    }
    let size = style.pen_size?;
    if size.to_bits() != expected_size_bits || stroke.pen_width.to_bits() != expected_size_bits {
        return None;
    }
    let argb = style.color_argb?;
    let color = stroke.color?;
    if [color.r, color.g, color.b]
        != [
            ((argb >> 16) & 0xff) as u8,
            ((argb >> 8) & 0xff) as u8,
            (argb & 0xff) as u8,
        ]
    {
        return None;
    }

    let points = stroke
        .points
        .iter()
        .map(|point| P {
            x: point.x as f32,
            y: point.y as f32,
        })
        .collect::<Vec<_>>();
    let pressures = stroke
        .pressures
        .iter()
        .map(|&value| value as f32)
        .collect::<Vec<_>>();
    let orientations = stroke
        .orientations
        .iter()
        .map(|&value| value as f32)
        .collect::<Vec<_>>();
    let tilts = stroke
        .tilts
        .iter()
        .map(|&value| value as f32)
        .collect::<Vec<_>>();

    let mut profile = context.constructor_profile;
    profile.motion_settings_limits = context.screen_resolution;
    let samples = sample_object_stroke_wrapper(
        context.gl_version,
        profile,
        size,
        context.fixed_width,
        context.fixed_width_enabled,
        properties.compressed,
        context.fixed_opacity_enabled,
        &points,
        &stroke.timestamps,
        &pressures,
        &orientations,
        &tilts,
        i32::from(rendering.tool_type_raw),
    )?;
    Some(SavedBrushPenSamples {
        output: sampled_instances(context.gl_version, &samples)?,
        argb,
    })
}

/// Native `makeDrawPoint` state after `drawLine` has built an SmPath.
///
/// The path, radius, opacity, and fixed-width endpoints are explicit native
/// state inputs assembled from the selected source profile before sampling.
#[derive(Clone, Copy)]
pub(super) struct DrawPointInput<'a> {
    pub(super) path: EndPath<'a>,
    pub(super) half_length: f32,
    pub(super) radius_start: f32,
    pub(super) radius_target: f32,
    pub(super) fixed_radius: Option<f32>,
    pub(super) opacity_start: i32,
    pub(super) opacity_target: i32,
    pub(super) tilt: [f32; 2],
    /// GLV10 drawPattern's tilt vector, scaled by max(current radius, 1) and
    /// applied after SmPath sampling.
    pub(super) point_offset: [f32; 2],
    /// Native drawable counter at makeDrawPoint entry. It persists across
    /// adjacent drawLine calls rather than restarting for each path.
    pub(super) pattern_start: usize,
}

/// Pack the numeric result of one native AddPoint call.
///
/// The output order is position/radius, opacity-encoded pattern index, tilt,
/// direction, then the three auxiliary values. The native shader consumes the
/// first eight values; the final three are retained in the instance buffer.
fn instance(input: AddPointInput) -> Option<Instance> {
    let [x, y, radius, pattern] = input.vector;
    if ![x, y, radius, pattern, input.opacity]
        .into_iter()
        .chain(input.tilt)
        .chain(input.direction)
        .chain(input.auxiliary)
        .all(f32::is_finite)
        || radius < 0.0
        || input.opacity < 0.0
    {
        return None;
    }

    // AddPoint widens the opacity and pattern index to double for this
    // multiply-add, then converts the packed texture/opacity value back to f32.
    let texture = (f64::from(pattern) + f64::from(input.opacity) * TEXTURE_OPACITY_SCALE) as f32;
    Some(Instance {
        values: [
            x,
            y,
            radius,
            texture,
            input.tilt[0],
            input.tilt[1],
            input.direction[0],
            input.direction[1],
            input.auxiliary[0],
            input.auxiliary[1],
            input.auxiliary[2],
        ],
    })
}

/// Convert explicit sampled GLV10 calls to native RTV1 instance records.
///
/// The GL version is an admission condition, not inferred from the tool label:
/// the fixture records the fresh BrushPen constructor selecting GLV10 and its
/// RTV1 drawable. Inputs beyond this captured GLV10 contract are rejected.
pub(super) fn sampled_instances(gl_version: u32, samples: &[AddPointInput]) -> Option<Output> {
    if gl_version != GL_VERSION || samples.len() > MAX_SAMPLES {
        return None;
    }
    let mut output = Output {
        instances: Vec::with_capacity(samples.len()),
    };
    for &sample in samples {
        output.instances.push(instance(sample)?);
    }
    Some(output)
}

/// Sample the native GLV10 `makeDrawPoint` path after its `drawLine` state
/// setup. The native loop stamps at distance zero, advances by one quarter of
/// the current radius, and linearly moves that radius toward its target over
/// the remaining distance to half the SmPath length.
pub(super) fn sample_draw_point(
    gl_version: u32,
    input: DrawPointInput<'_>,
) -> Option<Vec<AddPointInput>> {
    if gl_version != GL_VERSION
        || !input.half_length.is_finite()
        || input.half_length <= 0.0
        || !input.radius_start.is_finite()
        || input.radius_start <= 0.0
        || !input.radius_target.is_finite()
        || input.radius_target <= 0.0
        || input
            .fixed_radius
            .is_some_and(|radius| !radius.is_finite() || radius <= 0.0)
        || !input.tilt.into_iter().all(f32::is_finite)
        || !input.point_offset.into_iter().all(f32::is_finite)
    {
        return None;
    }

    let mut samples = Vec::new();
    let mut distance = 0.0_f32;
    let mut radius = input.radius_start;
    let mut opacity = input.opacity_start;
    loop {
        if samples.len() == MAX_SAMPLES {
            return None;
        }
        let point = input.path.at(distance);
        let tangent = input.path.normalized_tangent(distance);
        let offset_scale = radius.max(1.0);
        samples.push(AddPointInput {
            vector: [
                input.point_offset[0].mul_add(offset_scale, point.x),
                input.point_offset[1].mul_add(offset_scale, point.y),
                input.fixed_radius.unwrap_or(offset_scale),
                // drawPattern's multiply-high reduction wraps its counter at
                // seven before passing the texture index to AddPoint.
                ((input.pattern_start + samples.len()) % 7) as f32,
            ],
            opacity: opacity as f32,
            tilt: input.tilt,
            direction: [tangent.x, tangent.y],
            auxiliary: [0.0; 3],
        });

        // AArch64 native evaluation: step = radius * .25; next = distance +
        // step; radius += ((target-radius)/(half_length-distance))*step.
        let step = radius * 0.25;
        let next_distance = distance + step;
        if next_distance + step >= input.half_length || next_distance <= distance {
            break;
        }
        let slope = (input.radius_target - radius) / (input.half_length - distance);
        radius = slope.mul_add(step, radius);
        let opacity_rate = (input.opacity_target - opacity) as f32 / (input.half_length - distance);
        opacity = opacity_rate.mul_add(step, opacity as f32).trunc() as i32;
        distance = next_distance;
    }
    Some(samples)
}

/// `makeEndPenPoint` has a distinct radius/alpha state machine from the
/// ordinary drawLine sampler. It uses a quarter-radius step, interpolates the
/// unscaled radius toward a style-derived target, then applies the native
/// distance alpha multiplier before the next drawPattern call.
fn sample_end_pen_point(
    gl_version: u32,
    path: EndPath<'_>,
    radius_start: f32,
    radius_target: f32,
    tilt: [f32; 2],
    pattern_start: usize,
) -> Option<Vec<AddPointInput>> {
    let length = path.length();
    if gl_version != GL_VERSION
        || !length.is_finite()
        || length <= 0.0
        || !radius_start.is_finite()
        || radius_start <= 0.0
        || !radius_target.is_finite()
        || radius_target <= 0.0
        || !tilt.into_iter().all(f32::is_finite)
    {
        return None;
    }

    let mut output = Vec::new();
    let mut distance = 0.0_f32;
    let mut radius = radius_start;
    let mut alpha = 1.0_f32;
    loop {
        if output.len() >= 200 {
            return None;
        }
        let point = path.at(distance);
        let tangent = path.normalized_tangent(distance);
        let effective_radius = radius * alpha;
        let stamp_radius = effective_radius.max(1.0);
        output.push(AddPointInput {
            vector: [
                tilt[0].mul_add(stamp_radius, point.x),
                tilt[1].mul_add(stamp_radius, point.y),
                stamp_radius,
                ((pattern_start + output.len()) % 7) as f32,
            ],
            opacity: 255.0,
            tilt,
            direction: [tangent.x, tangent.y],
            auxiliary: [0.0; 3],
        });

        let step = radius * 0.25;
        let next_distance = distance + step;
        if next_distance <= distance {
            break;
        }
        let slope = (radius_target - radius) / (length - distance);
        let next_radius = slope.mul_add(step, radius);
        if next_distance + next_radius * 0.25 >= length {
            break;
        }

        // Native endPen scales each non-initial makeEnd stamp by this
        // quadratic distance alpha. The denominator is SmPath length * 60.
        let alpha_numerator = (next_distance - length) * next_distance;
        alpha = 1.0 - alpha_numerator / (length * GLV10_END_PEN_ALPHA_DISTANCE_SCALE);
        radius = next_radius;
        distance = next_distance;
    }
    Some(output)
}

/// endPen chooses a cubic or quadratic from persistent drawable flag +226,
/// independently of tool type. The last admitted drawLine transition supplies
/// moveTo/control state; the terminal MotionEvent supplies the endpoint.
// Keep the recovered native event channels and path state explicit here.
#[allow(clippy::too_many_arguments)]
fn sample_motion_end_segment(
    gl_version: u32,
    points: &[P],
    last_draw_previous: P,
    last_draw_current: P,
    cubic: bool,
    tool_type: i32,
    radius_start: f32,
    radius_target: f32,
    tilt: [f32; 2],
    pattern_start: usize,
) -> Option<Vec<AddPointInput>> {
    if points.len() < 3 {
        return None;
    }
    let n = points.len();
    let terminal = points[n - 1];
    if cubic {
        let start = last_draw_previous.mid(last_draw_current);
        let next_midpoint = last_draw_current.mid(terminal);
        let path = Cubic::new([start, last_draw_current, next_midpoint, terminal]);
        sample_end_pen_point(
            gl_version,
            EndPath::Cubic(&path),
            radius_start,
            radius_target,
            tilt,
            pattern_start,
        )
    } else if matches!(tool_type, 1 | 3) {
        let start = last_draw_previous.mid(last_draw_current);
        let path = Quad::new(start, last_draw_current, terminal);
        sample_end_pen_point(
            gl_version,
            EndPath::Quad(&path),
            radius_start,
            radius_target,
            tilt,
            pattern_start,
        )
    } else {
        None
    }
}

/// Reconstruct every GLV10 drawPattern stamp generated by one MotionEvent
/// sequence for the captured fresh-constructor profile. Source event arrays
/// drive the history loop, persistent radius, and optional terminal endPen;
/// the native constructor profile supplies the captured default style state.
// This mirrors the native redraw inputs rather than hiding source channels in
// an aggregate whose field order would be less obvious at this boundary.
#[allow(clippy::too_many_arguments)]
fn sample_motion_events(
    gl_version: u32,
    profile: Glv10RuntimeProfile,
    points: &[P],
    times: &[i64],
    pressures: &[f32],
    orientations: &[f32],
    tilts: &[f32],
    tool_type: i32,
    curve_enabled: bool,
    // The pinned BrushPen morphable vtable resolves slots 8/9 to native
    // no-op implementations (setter returns 1, getter returns 0). The saved
    // wrapper oracle checks those virtual targets and compares a true
    // ObjectStroke flag against its false baseline byte-for-byte; keep the
    // source input explicit without inventing an opacity transition here.
    _fixed_opacity_enabled: bool,
) -> Option<Vec<AddPointInput>> {
    if gl_version != GL_VERSION
        || points.len() != times.len()
        || points.len() != pressures.len()
        || points.len() != orientations.len()
        || points.len() != tilts.len()
    {
        return None;
    }
    let replay = admitted_motion_transitions(profile, points, times, tool_type, curve_enabled)?;
    let history = &replay.transitions;
    let radius_states =
        motion_history_radius_states(profile, points, times, pressures, tool_type, curve_enabled)?;
    if history.len() != radius_states.len() {
        return None;
    }

    let mut samples = Vec::new();
    let mut prior_opacity = profile.constructor_opacity as i32;
    let mut previous_width = profile.initial_motion_width;
    for (index, transition) in history.iter().enumerate() {
        let pressure = motion_pressure(tool_type, pressures[transition.event_index]);
        let interpolation = (pressure * profile.pressure_scale)
            .max(profile.pressure_min)
            .min(1.0);
        let width = if previous_width.to_bits() == profile.initial_motion_width.to_bits() {
            profile.first_draw_width
        } else {
            profile.steady_draw_width
        };
        let opacity_target = if profile.fixed_width_enabled {
            255
        } else {
            motion_opacity_target(
                transition.smoothed_speed,
                interpolation,
                width,
                prior_opacity,
            )?
        };
        let radius_start = if index == 0 {
            motion_radius_seed(profile, transition.smoothed_speed, pressure)?
        } else {
            radius_states[index - 1] * 0.5
        };
        let radius_target = radius_states[index] * 0.5;
        let tilt = motion_tilt(
            profile,
            tool_type,
            orientations[transition.event_index],
            tilts[transition.event_index],
        )?;
        let segment = sample_draw_point(
            gl_version,
            DrawPointInput {
                path: transition.path.as_end_path(),
                half_length: transition.path.length(),
                radius_start,
                radius_target,
                fixed_radius: profile
                    .fixed_width_enabled
                    .then_some(profile.fixed_width * 0.5),
                opacity_start: prior_opacity,
                opacity_target,
                tilt,
                point_offset: if profile.fixed_width_enabled {
                    [0.0; 2]
                } else {
                    tilt
                },
                pattern_start: samples.len(),
            },
        )?;
        samples.extend(segment);
        prior_opacity = opacity_target;
        previous_width = width;
    }

    if curve_enabled && matches!(tool_type, 1 | 3) {
        let end = motion_end_state(profile, points, times, tool_type, curve_enabled)?;
        let terminal_pressure = motion_pressure(tool_type, pressures[pressures.len() - 1]);
        let target = motion_end_radius_target(profile, end.terminal_scalar, terminal_pressure)?;
        let radius_start = radius_states.last().copied()? * 0.5;
        let tilt = motion_tilt(
            profile,
            tool_type,
            orientations[orientations.len() - 1],
            tilts[tilts.len() - 1],
        )?;
        let terminal = sample_motion_end_segment(
            gl_version,
            points,
            history.last()?.accepted_current,
            points[history.last()?.event_index],
            replay.terminal_flag226 != 0,
            tool_type,
            radius_start,
            target,
            tilt,
            samples.len(),
        )?;
        samples.extend(terminal);
    }
    Some(samples)
}

/// Replay the ObjectStroke wrapper's source iteration. Its redraw loop calls
/// drawLine for every point after the initial row, then passes the last row to
/// endPen; in the captured tool-2 cases that call emits no stamps. A duplicate final
/// row therefore closes the history window without adding a synthetic line.
// Preserve the source wrapper's full constructor and MotionEvent contract.
#[allow(clippy::too_many_arguments)]
fn sample_object_stroke_wrapper(
    gl_version: u32,
    constructor_profile: Glv10RuntimeProfile,
    saved_size: f32,
    fixed_width: f32,
    fixed_width_enabled: bool,
    curve_enabled: bool,
    fixed_opacity_enabled: bool,
    points: &[P],
    times: &[i64],
    pressures: &[f32],
    orientations: &[f32],
    tilts: &[f32],
    tool_type: i32,
) -> Option<Vec<AddPointInput>> {
    if points.len() < 2
        || points.len() != times.len()
        || points.len() != pressures.len()
        || points.len() != orientations.len()
        || points.len() != tilts.len()
        || !saved_size.is_finite()
        || saved_size <= 0.0
        || !fixed_width.is_finite()
        || fixed_width < 0.0
    {
        return None;
    }
    let mut profile = constructor_profile;
    profile.runtime_size = saved_size;
    profile.fixed_width_enabled = fixed_width_enabled;
    profile.fixed_width = fixed_width;
    let mut points = points.to_vec();
    let mut times = times.to_vec();
    let mut pressures = pressures.to_vec();
    let mut orientations = orientations.to_vec();
    let mut tilts = tilts.to_vec();
    points.push(*points.last()?);
    times.push(*times.last()?);
    pressures.push(*pressures.last()?);
    orientations.push(*orientations.last()?);
    tilts.push(*tilts.last()?);
    sample_motion_events(
        gl_version,
        profile,
        &points,
        &times,
        &pressures,
        &orientations,
        &tilts,
        tool_type,
        curve_enabled,
        fixed_opacity_enabled,
    )
}

/// Reconstruct one explicitly parameterized first GLV10 MotionEvent segment.
/// Source-driven multi-event replay is provided by `sample_motion_events`.
// These are the exact fields passed through the recovered first-segment path.
#[allow(clippy::too_many_arguments)]
pub(super) fn sample_first_motion_segment(
    gl_version: u32,
    previous: P,
    current: P,
    radius_start: f32,
    radius_target: f32,
    opacity: f32,
    tilt: [f32; 2],
    point_offset: [f32; 2],
) -> Option<Vec<AddPointInput>> {
    if gl_version != GL_VERSION {
        return None;
    }
    let path = Quad::new(previous, previous, previous.mid(current));
    sample_draw_point(
        gl_version,
        DrawPointInput {
            path: EndPath::Quad(&path),
            half_length: path.length,
            radius_start,
            radius_target,
            fixed_radius: None,
            opacity_start: opacity.trunc() as i32,
            opacity_target: opacity.trunc() as i32,
            tilt,
            point_offset,
            pattern_start: 0,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn f32_bits(value: &Value) -> f32 {
        let hex = value.as_str().unwrap();
        let mut bytes = [0_u8; 4];
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).unwrap();
        }
        f32::from_bits(u32::from_le_bytes(bytes))
    }

    fn bytes_from_hex(hex: &str) -> Vec<u8> {
        assert_eq!(hex.len() % 2, 0);
        hex.as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| {
                let text = std::str::from_utf8(pair).unwrap();
                u8::from_str_radix(text, 16).unwrap()
            })
            .collect()
    }

    fn f32s_from_hex(hex: &str) -> Vec<f32> {
        bytes_from_hex(hex)
            .as_chunks::<4>()
            .0
            .iter()
            .map(|bytes| f32::from_le_bytes(*bytes))
            .collect()
    }

    fn recorded_call(call: &Value) -> AddPointInput {
        let vector = call["vector4_bits_le"]
            .as_array()
            .unwrap()
            .iter()
            .map(f32_bits)
            .collect::<Vec<_>>();
        let registers = call["s0_s7_bits"]
            .as_array()
            .unwrap()
            .iter()
            .map(f32_bits)
            .collect::<Vec<_>>();
        AddPointInput {
            vector: [vector[0], vector[1], vector[2], vector[3]],
            opacity: registers[0],
            tilt: [registers[1], registers[2]],
            direction: [registers[3], registers[4]],
            // The remaining native call values are retained as explicit zero
            // synthetic inputs in these four profiles. Their host meaning is
            // unresolved; do not infer them from event channels.
            auxiliary: [0.0; 3],
        }
    }

    #[test]
    fn glv10_samples_pack_every_captured_rtv1_instance_word() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/brushpen-saved-native.json"
        ))
        .unwrap();
        assert!(
            fixture["library_sha256"]["BrushPen"]
                .as_str()
                .unwrap()
                .eq("acf59300dbdf53942680761a6cfe57e5164e0e6c45076d30fc56b9efa04ea0ce")
        );

        for case in fixture["synthetic_cases"].as_array().unwrap() {
            let admission = &case["admission"];
            assert_eq!(
                admission["actual_getVersion"].as_u64(),
                Some(GL_VERSION.into())
            );
            assert!(admission["selected_GLVx_vtable_symbol_plus_16"].is_string());
            assert!(admission["constructed_RTV1_vtable_symbol_plus_16"].is_string());
            assert_eq!(
                admission["constructor_setting_parser_inputs"][0]["text"].as_str(),
                Some("10")
            );

            let calls = case["callback_arguments"].as_array().unwrap();
            let buffers = case["buffer_records"].as_array().unwrap();
            assert_eq!(calls.len(), buffers.len(), "{} calls", case["label"]);
            let input = calls.iter().map(recorded_call).collect::<Vec<_>>();
            let output = sampled_instances(GL_VERSION, &input).unwrap();
            assert_eq!(
                output.instances.len(),
                calls.len(),
                "{} count",
                case["label"]
            );

            for (index, (actual, buffer)) in output.instances.iter().zip(buffers).enumerate() {
                assert_eq!(buffer["append_index"].as_u64(), Some(index as u64));
                assert_eq!(
                    buffer["count_f32"].as_u64(),
                    Some(((index + 1) * 11) as u64)
                );
                let values = buffer["instance_bits_le"].as_array().unwrap();
                assert_eq!(values.len(), 11, "{} record", case["label"]);
                assert_eq!(values.len(), 11);
                for (word, value) in values.iter().enumerate() {
                    let expected = f32_bits(value);
                    assert_eq!(
                        actual.values[word].to_bits(),
                        expected.to_bits(),
                        "{} sample {index} word {word}",
                        case["label"]
                    );
                }
            }
        }
        assert!(sampled_instances(9, &[]).is_none());
        assert!(
            sampled_instances(
                GL_VERSION,
                &[AddPointInput {
                    vector: [f32::NAN, 0.0, 1.0, 0.0],
                    opacity: 255.0,
                    tilt: [0.0; 2],
                    direction: [0.0; 2],
                    auxiliary: [0.0; 3],
                }]
            )
            .is_none()
        );
    }

    #[test]
    fn glv10_motion_segments_match_native_saved_records() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/brushpen-saved-native.json"
        ))
        .unwrap();
        let profile = SAVED_GLV10_CONSTRUCTOR_PROFILE;
        let expected_counts = [16, 15, 16];

        for (case_index, case) in fixture["synthetic_cases"]
            .as_array()
            .unwrap()
            .iter()
            .take(3)
            .enumerate()
        {
            let source = &case["synthetic_input"];
            assert_eq!(
                case["admission"]["actual_getVersion"].as_u64(),
                Some(GL_VERSION.into())
            );
            assert_eq!(
                case["admission"]["constructor_setting_parser_inputs"][0]["text"].as_str(),
                Some("10")
            );
            let points = source["points"].as_array().unwrap();
            let point = |index: usize| P {
                x: points[index][0].as_f64().unwrap() as f32,
                y: points[index][1].as_f64().unwrap() as f32,
            };
            let source_points = (0..points.len()).map(point).collect::<Vec<_>>();
            let previous = point(0);
            let current = point(1);
            let tool_type = source["tool_type"].as_i64().unwrap() as i32;
            let orientation = source["orientations"][1].as_f64().unwrap() as f32;
            let source_tilt = source["tilts"][1].as_f64().unwrap() as f32;
            let tilt = motion_tilt(profile, tool_type, orientation, source_tilt).unwrap();
            let pressure =
                motion_pressure(tool_type, source["pressures"][1].as_f64().unwrap() as f32);
            let times = source["times"]
                .as_array()
                .unwrap()
                .iter()
                .map(|time| time.as_i64().unwrap())
                .collect::<Vec<_>>();
            let history = motion_history_transitions(profile, &source_points, &times).unwrap();
            assert_eq!(history.len(), source_points.len() - 2);
            assert_eq!(history[0].event_index, 1);
            assert_eq!(history[0].previous.x.to_bits(), previous.x.to_bits());
            assert_eq!(history[0].current.y.to_bits(), current.y.to_bits());
            assert_eq!(
                history[0].elapsed_ms.to_bits(),
                ((times[1] - times[0]) as f32).to_bits()
            );
            assert_eq!(history[0].prior_speed.to_bits(), 0.0_f32.to_bits());
            let line_scalar = history[0].smoothed_speed;
            let radius_start = motion_radius_seed(profile, line_scalar, pressure).unwrap();
            let expected_radius: f32 = [0.287_913_08, 0.288_141_28, 0.288_455_9][case_index];
            assert_eq!(radius_start.to_bits(), expected_radius.to_bits());
            let samples = sample_first_motion_segment(
                GL_VERSION,
                previous,
                current,
                radius_start,
                1.0,
                255.0,
                tilt,
                tilt,
            )
            .unwrap();
            let calls = case["callback_arguments"].as_array().unwrap();
            assert_eq!(
                samples.len(),
                expected_counts[case_index],
                "case {case_index} first segment"
            );

            for (index, (actual, call)) in samples.iter().zip(calls).enumerate() {
                let expected = recorded_call(call);
                assert_eq!(
                    actual.vector.map(f32::to_bits),
                    expected.vector.map(f32::to_bits),
                    "case {case_index} sample {index} position, size, and pattern counter"
                );
                assert_eq!(actual.opacity.to_bits(), expected.opacity.to_bits());
                assert_eq!(
                    actual.tilt.map(f32::to_bits),
                    expected.tilt.map(f32::to_bits)
                );
                assert_eq!(
                    actual.direction.map(f32::to_bits),
                    expected.direction.map(f32::to_bits),
                    "case {case_index} sample {index} direction"
                );
            }
            let output = sampled_instances(GL_VERSION, &samples).unwrap();
            for (index, (actual, buffer)) in output
                .instances
                .iter()
                .zip(case["buffer_records"].as_array().unwrap())
                .enumerate()
            {
                let expected = buffer["instance_bits_le"].as_array().unwrap();
                assert_eq!(expected.len(), 11);
                for (word, value) in expected.iter().enumerate() {
                    assert_eq!(
                        actual.values[word].to_bits(),
                        f32_bits(value).to_bits(),
                        "case {case_index} sample {index} instance word {word}: actual={} expected={}",
                        actual.values[word],
                        f32_bits(&expected[word]),
                    );
                }
            }

            if case_index == 1 {
                let next_scalar = history[1].smoothed_speed;
                assert_eq!(next_scalar.to_bits(), 1.507_801_3_f32.to_bits());
                let path = &history[1].path;
                let radius_states = motion_history_radius_states(
                    profile,
                    &source_points,
                    &times,
                    &source["pressures"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|pressure| pressure.as_f64().unwrap() as f32)
                        .collect::<Vec<_>>(),
                    tool_type,
                    true,
                )
                .unwrap();
                // drawLine uses the just-updated drawable radius field, halved
                // by its makeDrawPoint setup, as the next segment's target.
                let target = radius_states[1] * 0.5;
                let second = sample_draw_point(
                    GL_VERSION,
                    DrawPointInput {
                        path: EndPath::Quad(path),
                        half_length: path.length,
                        radius_start: radius_states[0] * 0.5,
                        radius_target: target,
                        fixed_radius: None,
                        opacity_start: 255,
                        opacity_target: 255,
                        tilt,
                        point_offset: tilt,
                        pattern_start: samples.len(),
                    },
                )
                .unwrap();
                let calls = case["callback_arguments"].as_array().unwrap();
                assert_eq!(second.len(), 16);
                for (index, (actual, call)) in second.iter().zip(&calls[15..31]).enumerate() {
                    let expected = recorded_call(call);
                    assert_eq!(
                        actual.vector.map(f32::to_bits),
                        expected.vector.map(f32::to_bits),
                        "case 1 second segment sample {index}"
                    );
                    assert_eq!(actual.opacity.to_bits(), expected.opacity.to_bits());
                    assert_eq!(
                        actual.tilt.map(f32::to_bits),
                        expected.tilt.map(f32::to_bits)
                    );
                    assert_eq!(
                        actual.direction.map(f32::to_bits),
                        expected.direction.map(f32::to_bits),
                        "case 1 second segment direction {index}"
                    );
                }
                let packed = sampled_instances(GL_VERSION, &second).unwrap();
                let buffers = case["buffer_records"].as_array().unwrap();
                for (index, (actual, buffer)) in
                    packed.instances.iter().zip(&buffers[15..31]).enumerate()
                {
                    for (word, expected) in buffer["instance_bits_le"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .enumerate()
                    {
                        assert_eq!(
                            actual.values[word].to_bits(),
                            f32_bits(expected).to_bits(),
                            "case 1 second segment sample {index} record word {word}"
                        );
                    }
                }
            }
        }
        assert!(
            sample_first_motion_segment(
                9,
                P { x: 0.0, y: 0.0 },
                P { x: 4.0, y: 2.0 },
                0.287_913_08,
                1.0,
                255.0,
                [-0.0, 0.0001],
                [0.0, 0.0001],
            )
            .is_none()
        );
    }

    #[test]
    fn glv10_end_pen_sampling_matches_terminal_native_records() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/brushpen-saved-native.json"
        ))
        .unwrap();
        let profile = SAVED_GLV10_CONSTRUCTOR_PROFILE;
        // The expected scalar/path constants are independent native checks;
        // persistent radius fields are now replayed from constructor/profile
        // state and each source MotionEvent transition below.
        let captured = [
            (1usize, 31usize, 1.247_835_5_f32, 3.692_831_8_f32),
            (2usize, 16usize, 1.118_034_f32, 2.586_188_3_f32),
        ];
        let tilt = motion_tilt(profile, 1, 0.0, 0.0).unwrap();
        for (case_index, first, expected_prior_speed, expected_terminal_scalar) in captured {
            let case = &fixture["synthetic_cases"][case_index];
            assert_eq!(
                case["admission"]["actual_getVersion"].as_u64(),
                Some(GL_VERSION.into())
            );
            assert_eq!(
                case["admission"]["constructor_setting_parser_inputs"][0]["text"].as_str(),
                Some("10")
            );
            let source = &case["synthetic_input"];
            let points = source["points"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| P {
                    x: p[0].as_f64().unwrap() as f32,
                    y: p[1].as_f64().unwrap() as f32,
                })
                .collect::<Vec<_>>();
            let tool_type = source["tool_type"].as_i64().unwrap() as i32;
            let times = source["times"]
                .as_array()
                .unwrap()
                .iter()
                .map(|time| time.as_i64().unwrap())
                .collect::<Vec<_>>();
            let end_state = motion_end_state(profile, &points, &times, tool_type, true).unwrap();
            assert_eq!(
                end_state.previous.x.to_bits(),
                points[points.len() - 2].x.to_bits()
            );
            assert_eq!(
                end_state.previous.y.to_bits(),
                points[points.len() - 2].y.to_bits()
            );
            assert_eq!(
                end_state.current.x.to_bits(),
                points[points.len() - 1].x.to_bits()
            );
            assert_eq!(
                end_state.current.y.to_bits(),
                points[points.len() - 1].y.to_bits()
            );
            assert_eq!(
                end_state.elapsed_ms.to_bits(),
                ((times[times.len() - 1] - times[times.len() - 2]) as f32).to_bits()
            );
            assert_eq!(
                end_state.prior_speed.to_bits(),
                expected_prior_speed.to_bits(),
                "case {case_index} drawLine state before endPen"
            );
            let terminal_scalar = end_state.terminal_scalar;
            assert_eq!(
                terminal_scalar.to_bits(),
                expected_terminal_scalar.to_bits(),
                "case {case_index} endPen scalar from event pairs"
            );
            let pressures = source["pressures"]
                .as_array()
                .unwrap()
                .iter()
                .map(|pressure| pressure.as_f64().unwrap() as f32)
                .collect::<Vec<_>>();
            let radius_field =
                motion_history_radius_state(profile, &points, &times, &pressures, tool_type, true)
                    .unwrap();
            let expected_radius_field = if case_index == 1 {
                f32::from_bits(0x400d_5d65)
            } else {
                2.0_f32
            };
            assert_eq!(
                radius_field.to_bits(),
                expected_radius_field.to_bits(),
                "case {case_index} source-derived radius field"
            );
            let radius_start = radius_field * 0.5;
            if tool_type == 1 {
                let n = points.len();
                let path = Cubic::new([
                    points[n - 3].mid(points[n - 2]),
                    points[n - 2],
                    points[n - 2].mid(points[n - 1]),
                    points[n - 1],
                ]);
                assert_eq!(path.length.to_bits(), 7.031_569_5_f32.to_bits());
            } else {
                let n = points.len();
                let path = Quad::new(
                    points[n - 3].mid(points[n - 2]),
                    points[n - 2],
                    points[n - 1],
                );
                assert_eq!(path.length.to_bits(), 6.403_124_3_f32.to_bits());
            }
            let pressure = motion_pressure(
                tool_type,
                source["pressures"][points.len() - 1].as_f64().unwrap() as f32,
            );
            let target = motion_end_radius_target(profile, terminal_scalar, pressure).unwrap();
            let samples = sample_motion_end_segment(
                GL_VERSION,
                &points,
                points[points.len() - 3],
                points[points.len() - 2],
                tool_type == 1,
                tool_type,
                radius_start,
                target,
                tilt,
                first,
            )
            .unwrap();
            let buffers = case["buffer_records"].as_array().unwrap();
            assert_eq!(
                samples.len(),
                buffers.len() - first,
                "case {case_index} terminal count"
            );
            let packed = sampled_instances(GL_VERSION, &samples).unwrap();
            for (offset, (sample, record)) in samples.iter().zip(&buffers[first..]).enumerate() {
                let expected = record["instance_bits_le"].as_array().unwrap();
                let expected_call = recorded_call(&case["callback_arguments"][first + offset]);
                assert_eq!(
                    sample.vector.map(f32::to_bits),
                    expected_call.vector.map(f32::to_bits),
                    "case {case_index} terminal sample {offset} vector"
                );
                assert_eq!(
                    sample.direction.map(f32::to_bits),
                    expected_call.direction.map(f32::to_bits),
                    "case {case_index} terminal sample {offset} tangent"
                );
                assert_eq!(expected.len(), 11);
                for (word, value) in expected.iter().enumerate() {
                    assert_eq!(
                        packed.instances[offset].values[word].to_bits(),
                        f32_bits(value).to_bits(),
                        "case {case_index} terminal sample {offset} word {word}"
                    );
                }
            }
        }
    }

    #[test]
    fn glv10_make_draw_point_radius_state_replays_all_saved_motion_events() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/brushpen-saved-native.json"
        ))
        .unwrap();
        let profile = SAVED_GLV10_CONSTRUCTOR_PROFILE;
        // Directly observed at GLV10::makeDrawPoint's +256 store. These are
        // persistent drawable fields after each source-history drawLine path.
        let expected = [2.0_f32, f32::from_bits(0x400d_5d65), 2.0_f32];
        for (case_index, case) in fixture["synthetic_cases"]
            .as_array()
            .unwrap()
            .iter()
            .take(3)
            .enumerate()
        {
            let source = &case["synthetic_input"];
            let points = source["points"]
                .as_array()
                .unwrap()
                .iter()
                .map(|point| P {
                    x: point[0].as_f64().unwrap() as f32,
                    y: point[1].as_f64().unwrap() as f32,
                })
                .collect::<Vec<_>>();
            let times = source["times"]
                .as_array()
                .unwrap()
                .iter()
                .map(|time| time.as_i64().unwrap())
                .collect::<Vec<_>>();
            let pressures = source["pressures"]
                .as_array()
                .unwrap()
                .iter()
                .map(|pressure| pressure.as_f64().unwrap() as f32)
                .collect::<Vec<_>>();
            let tool_type = source["tool_type"].as_i64().unwrap() as i32;
            let radius =
                motion_history_radius_state(profile, &points, &times, &pressures, tool_type, true)
                    .unwrap();
            assert_eq!(
                radius.to_bits(),
                expected[case_index].to_bits(),
                "case {case_index} source-derived persistent radius"
            );
        }
    }

    #[test]
    fn make_draw_point_uses_literal_prior_weight_in_fused_blend() {
        let profile = SAVED_GLV10_CONSTRUCTOR_PROFILE;
        let prior = 2.04_f32;
        let scaled = profile.runtime_size * 0.75_f32;
        let actual = blend_motion_radius(prior, scaled, profile);
        assert_eq!(prior.to_bits(), 0x4002_8f5c);
        assert_eq!(scaled.to_bits(), 0x4016_e979);
        assert_eq!(actual.to_bits(), 0x400e_c56d);

        let literal_decimal_weight = 0.4_f32;
        let wrong_prior = prior * literal_decimal_weight;
        let wrong = scaled.mul_add(profile.radius_state_smoothing, wrong_prior);
        assert_eq!(profile.radius_state_prior_weight.to_bits(), 0x3ecc_cccc);
        assert_eq!(literal_decimal_weight.to_bits(), 0x3ecc_cccd);
        assert_eq!(wrong.to_bits(), 0x400e_c56e);
        assert_ne!(actual.to_bits(), wrong.to_bits());
    }

    #[test]
    fn saved_constructor_profile_matches_pinned_glv10_numeric_constants() {
        let profile = SAVED_GLV10_CONSTRUCTOR_PROFILE;
        assert_eq!(profile.runtime_size.to_bits(), 0x4049_374c);
        assert_eq!(profile.pressure_scale.to_bits(), 0x3fd5_5555);
        assert_eq!(profile.pressure_min.to_bits(), 0x3c23_d70a);
        assert_eq!(profile.radius_bias.to_bits(), 0x3fd5_c28f_5c28_f5c3);
        assert_eq!(profile.radius_scale.to_bits(), 0x3fe8_a3d7_0000_0000);
        assert_eq!(profile.radius_state_smoothing.to_bits(), 0x3f19_999a);
        assert_eq!(profile.radius_state_prior_weight.to_bits(), 0x3ecc_cccc);
        assert_eq!(profile.initial_motion_width.to_bits(), 0x3f45_1eb8);
        assert_eq!(profile.first_draw_width.to_bits(), 0x3f61_47ae);
        assert_eq!(profile.steady_draw_width.to_bits(), 0x3f80_0000);
        assert_eq!(profile.motion_distance_threshold.to_bits(), 0x4026_6666);
        assert_eq!(profile.motion_settings_limits, [1440, 2560]);
    }

    #[test]
    fn glv10_source_motion_events_reconstruct_all_captured_instance_records() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/brushpen-saved-native.json"
        ))
        .unwrap();
        let profile = SAVED_GLV10_CONSTRUCTOR_PROFILE;
        let mut mismatches = Vec::new();
        for case in fixture["synthetic_cases"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|case| {
                case["label"]
                    .as_str()
                    .is_some_and(|label| label.starts_with("synthetic-motion-"))
            })
        {
            let source = &case["synthetic_input"];
            let points = source["points"]
                .as_array()
                .unwrap()
                .iter()
                .map(|point| P {
                    x: point[0].as_f64().unwrap() as f32,
                    y: point[1].as_f64().unwrap() as f32,
                })
                .collect::<Vec<_>>();
            let times = source["times"]
                .as_array()
                .unwrap()
                .iter()
                .map(|time| time.as_i64().unwrap())
                .collect::<Vec<_>>();
            let channels = |field: &str| {
                source[field]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| value.as_f64().unwrap() as f32)
                    .collect::<Vec<_>>()
            };
            let tool_type = source["tool_type"].as_i64().unwrap() as i32;
            let samples = sample_motion_events(
                GL_VERSION,
                profile,
                &points,
                &times,
                &channels("pressures"),
                &channels("orientations"),
                &channels("tilts"),
                tool_type,
                true,
                false,
            )
            .unwrap_or_else(|| {
                panic!(
                    "source sampler returned None for {}",
                    case["label"].as_str().unwrap()
                )
            });
            let records = case["buffer_records"].as_array().unwrap();
            if samples.len() != records.len() {
                mismatches.push(format!(
                    "{} count {} != {}",
                    case["label"].as_str().unwrap(),
                    samples.len(),
                    records.len()
                ));
            }
            let packed = sampled_instances(GL_VERSION, &samples).unwrap();
            let mut first_difference = None;
            for (index, (actual, record)) in packed.instances.iter().zip(records).enumerate() {
                let expected = record["instance_bits_le"].as_array().unwrap();
                assert_eq!(expected.len(), 11);
                for (word, value) in expected.iter().enumerate() {
                    if actual.values[word].to_bits() != f32_bits(value).to_bits()
                        && first_difference.is_none()
                    {
                        first_difference =
                            Some((index, word, actual.values[word], f32_bits(value)));
                    }
                }
            }
            if let Some(mismatch) = first_difference {
                mismatches.push(format!("{}: {mismatch:?}", case["label"].as_str().unwrap()));
            }
        }
        assert!(
            mismatches.is_empty(),
            "source parity mismatches: {mismatches:?}"
        );
    }

    #[test]
    fn glv10_drawing_caller_screen_limits_reconstruct_full_native_buffer() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/brushpen-saved-caller-native.json"
        ))
        .unwrap();
        assert_eq!(fixture["schema"], "brushpen-saved-caller-native-v1");
        assert_eq!(fixture["profile"]["width"], 1920);
        assert_eq!(fixture["profile"]["height"], 1080);

        let source = &fixture["input"]["source_arrays"];
        let points_flat = f32s_from_hex(source["points"]["data_hex"].as_str().unwrap());
        assert_eq!(points_flat.len() % 2, 0);
        let points = points_flat
            .as_chunks::<2>()
            .0
            .iter()
            .map(|xy| P { x: xy[0], y: xy[1] })
            .collect::<Vec<_>>();
        let times_bytes = bytes_from_hex(source["timestamps"]["data_hex"].as_str().unwrap());
        let times = times_bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|bytes| i32::from_le_bytes(*bytes) as i64)
            .collect::<Vec<_>>();
        let pressures = f32s_from_hex(source["pressures"]["data_hex"].as_str().unwrap());
        let orientations = f32s_from_hex(source["orientations"]["data_hex"].as_str().unwrap());
        let tilts = f32s_from_hex(source["tilts"]["data_hex"].as_str().unwrap());

        let mut profile = SAVED_GLV10_CONSTRUCTOR_PROFILE;
        profile.runtime_size = f32_bits(&fixture["input"]["size_f32_bits"]);
        profile.motion_settings_limits = [
            fixture["profile"]["width"].as_i64().unwrap() as i32,
            fixture["profile"]["height"].as_i64().unwrap() as i32,
        ];
        assert_eq!(
            SAVED_GLV10_CONSTRUCTOR_PROFILE.motion_settings_limits,
            [1440, 2560]
        );
        assert_eq!(profile.motion_settings_limits, [1920, 1080]);

        let samples = sample_object_stroke_wrapper(
            GL_VERSION,
            profile,
            profile.runtime_size,
            0.0,
            false,
            fixture["input"]["curve_enabled"].as_bool().unwrap(),
            false,
            &points,
            &times,
            &pressures,
            &orientations,
            &tilts,
            fixture["input"]["tool_type"].as_i64().unwrap() as i32,
        )
        .expect("captured Drawing source should be admitted by the bounded GLV10 profile");
        let packed = sampled_instances(GL_VERSION, &samples).unwrap();
        let actual_bytes = packed
            .instances
            .iter()
            .flat_map(|instance| instance.values.iter())
            .flat_map(|value| value.to_le_bytes())
            .collect::<Vec<_>>();
        let expected_bytes = bytes_from_hex(fixture["producer"]["buffer_hex"].as_str().unwrap());
        assert_eq!(actual_bytes.len(), 1144);
        assert_eq!(samples.len(), 26);
        assert_eq!(actual_bytes, expected_bytes);
        assert_eq!(
            fixture["producer"]["buffer_sha256"].as_str().unwrap(),
            "57405d81981cd9deb40190fc185572dc9d5885ff5a92cf885e1ef53d652cee92"
        );
    }

    #[test]
    fn glv10_object_stroke_source_reconstructs_all_saved_wrapper_profiles() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/brushpen-saved-wrapper-native.json"
        ))
        .unwrap();
        assert_eq!(fixture["schema"], "sdocx-brushpen-saved-wrapper-native-v1");
        let mut checked = 0;
        for case in fixture["cases"].as_array().unwrap() {
            let input = &case["input"];
            let channels = |field: &str| {
                input[field]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| value.as_f64().unwrap() as f32)
                    .collect::<Vec<_>>()
            };
            let points = input["points"]
                .as_array()
                .unwrap()
                .iter()
                .map(|point| P {
                    x: point[0].as_f64().unwrap() as f32,
                    y: point[1].as_f64().unwrap() as f32,
                })
                .collect::<Vec<_>>();
            let times = input["times"]
                .as_array()
                .unwrap()
                .iter()
                .map(|time| time.as_i64().unwrap())
                .collect::<Vec<_>>();
            let size = case["style_setters"]["native_GetSize_f32"]
                .as_f64()
                .unwrap() as f32;
            let fixed_width_size = case["style_setters"]["native_GetFixedWidth_f32"]
                .as_f64()
                .unwrap() as f32;
            let fixed_width = case["style_setters"]["native_fixed_width_enabled_input"]
                .as_bool()
                .unwrap();
            let curve_enabled = input["curve_enabled"].as_bool().unwrap_or(true);
            let fixed_opacity_enabled = input["fixed_opacity_enabled"].as_bool().unwrap_or(false);
            assert_eq!(case["native_admission"]["actual_getVersion"], GL_VERSION);
            assert_eq!(
                case["object_setters_and_getters"]["native_saved_size_f32"]
                    .as_f64()
                    .unwrap() as f32,
                size
            );
            assert_eq!(
                case["object_setters_and_getters"]["native_saved_tool_type"],
                input["tool_type"]
            );
            assert_eq!(
                fixed_width_size.to_bits(),
                (case["style_setters"]["fixed_width_argument"]
                    .as_f64()
                    .unwrap() as f32)
                    .to_bits()
            );
            assert_eq!(
                case["style_setters"]["native_fixed_width_enabled_byte_offset_89"],
                u8::from(fixed_width)
            );
            assert_eq!(
                case["object_setters_and_getters"]["native_curve_enabled"],
                curve_enabled
            );
            assert_eq!(
                case["object_setters_and_getters"]["native_fixed_opacity_enabled"],
                fixed_opacity_enabled
            );
            if matches!(input["tool_type"].as_i64(), Some(1 | 3)) {
                let terminal = &case["endPen_terminal_record"];
                if curve_enabled {
                    assert_eq!(terminal["selected_path_method"], "cubicTo");
                    assert_eq!(terminal["emitted_addpoint_count"], 9);
                } else {
                    assert_eq!(terminal["selected_path_method"], "lineTo");
                    assert_eq!(terminal["path_length_f32"], 0.0);
                    assert_eq!(terminal["emitted_addpoint_count"], 0);
                }
            }
            if fixed_opacity_enabled {
                let contract = &case["native_fixed_opacity_contract"];
                assert_eq!(contract["GetMorphable_result_is_pen_plus_96"], true);
                assert_eq!(contract["setter_symbol_target_match"], true);
                assert_eq!(contract["getter_symbol_target_match"], true);
                assert_eq!(contract["vtable_slot_8_brush_elf_offset"], "0x566e0");
                assert_eq!(contract["vtable_slot_9_brush_elf_offset"], "0x566e8");
                let native_calls = contract["native_calls"].as_array().unwrap();
                assert_eq!(native_calls.len(), 2);
                for (call, input_value) in native_calls.iter().zip([false, true]) {
                    assert_eq!(call["setter_input"], input_value);
                    assert_eq!(call["setter_return_w0_u32"], 1);
                    assert_eq!(call["getter_return_w0_u32"], 0);
                    assert_eq!(call["pen_256_bytes_unchanged"], true);
                    assert_eq!(call["settings_96_bytes_unchanged"], true);
                }
            }
            let object_getters =
                &case["object_setters_and_getters"]["native_channel_getter_import_calls"];
            for getter in [
                "SPen::ObjectStroke::GetOrientation() const",
                "SPen::ObjectStroke::GetPoint() const",
                "SPen::ObjectStroke::GetPointCount() const",
                "SPen::ObjectStroke::GetPressure() const",
                "SPen::ObjectStroke::GetTilt() const",
                "SPen::ObjectStroke::GetTimeStamp() const",
            ] {
                assert_eq!(object_getters[getter], 1, "{getter}");
            }
            let final_index = points.len() - 1;
            let motion_ctor = &case["native_MotionEvent_constructor_calls"][0];
            assert_eq!(
                motion_ctor["arguments_decoded"]["down_time_ms_i64"],
                times[0]
            );
            let coords = &motion_ctor["pointer_coords_decoded_from_Base_getter_offsets"];
            assert_eq!(coords["event_time_i64_ms"], times[final_index]);
            assert_eq!(
                coords["x_f64"].as_f64().unwrap() as f32,
                points[final_index].x
            );
            assert_eq!(
                coords["y_f64"].as_f64().unwrap() as f32,
                points[final_index].y
            );
            assert_eq!(
                coords["pressure_f32"].as_f64().unwrap() as f32,
                channels("pressures")[final_index]
            );
            let getter_returns = case["native_MotionEvent_getter_returns"]
                .as_array()
                .unwrap();
            let find_getter = |suffix: &str| {
                getter_returns
                    .iter()
                    .find(|getter| getter["name"].as_str().unwrap().ends_with(suffix))
                    .unwrap()
            };
            assert_eq!(
                find_getter("GetEventTime() const")["return_x0"],
                times[final_index] - times[0]
            );
            assert_eq!(
                find_getter("GetX(int) const")["return_d0_f64"]
                    .as_f64()
                    .unwrap() as f32,
                points[final_index].x
            );
            assert_eq!(
                find_getter("GetY(int) const")["return_d0_f64"]
                    .as_f64()
                    .unwrap() as f32,
                points[final_index].y
            );
            assert_eq!(
                find_getter("GetToolType(int) const")["return_x0"],
                input["tool_type"]
            );
            if input["tool_type"] == 2 {
                assert_eq!(
                    find_getter("GetPressure(int) const")["return_s0_f32"]
                        .as_f64()
                        .unwrap() as f32,
                    channels("pressures")[final_index]
                );
            }
            let samples = sample_object_stroke_wrapper(
                GL_VERSION,
                SAVED_GLV10_CONSTRUCTOR_PROFILE,
                size,
                fixed_width_size,
                fixed_width,
                curve_enabled,
                fixed_opacity_enabled,
                &points,
                &times,
                &channels("pressures"),
                &channels("orientations"),
                &channels("tilts"),
                input["tool_type"].as_i64().unwrap() as i32,
            )
            .unwrap();
            let records = case["native_AddPoint_records"].as_array().unwrap();
            assert_eq!(samples.len(), records.len(), "{} count", case["label"]);
            if fixed_opacity_enabled {
                let baseline = fixture["cases"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|candidate| {
                        candidate["label"]
                            == "tool-2-curve-1-fixed-opacity-0-size-3.144-fixed-width-0"
                    })
                    .unwrap();
                assert_eq!(
                    case["native_output_buffer_f32_le_hex"],
                    baseline["native_output_buffer_f32_le_hex"]
                );
                assert_eq!(
                    case["native_AddPoint_records"],
                    baseline["native_AddPoint_records"]
                );
                assert_eq!(
                    case["native_fixed_opacity_buffer_comparison"]["native_buffer_bytes_equal"],
                    true
                );
                assert_eq!(
                    case["native_fixed_opacity_buffer_comparison"]["native_AddPoint_records_equal"],
                    true
                );
            }
            let draw_calls = case["native_drawPattern_calls"].as_array().unwrap();
            let style_steps = case["native_drawPattern_style_transition_steps"]
                .as_array()
                .unwrap();
            assert_eq!(
                draw_calls.len(),
                records.len(),
                "{} DrawPattern calls",
                case["label"]
            );
            assert_eq!(
                style_steps.len(),
                records.len(),
                "{} style steps",
                case["label"]
            );
            let packed = sampled_instances(GL_VERSION, &samples).unwrap();
            for (index, ((actual, expected), (draw_call, style_step))) in packed
                .instances
                .iter()
                .zip(records)
                .zip(draw_calls.iter().zip(style_steps))
                .enumerate()
            {
                let expected = expected["instance_bits_le"].as_array().unwrap();
                let working_radius = style_step["working_radius_s14_f32"].as_f64().unwrap() as f32;
                let derived_radius = if fixed_width {
                    fixed_width_size * 0.5_f32
                } else {
                    (draw_call["radius_argument_s0_f32"].as_f64().unwrap() as f32).max(1.0)
                };
                let input_opacity = draw_call["input_opacity_u32"].as_u64().unwrap() as u32;
                let derived_opacity = if fixed_width { 255 } else { input_opacity };
                assert_eq!(
                    actual.values[2].to_bits(),
                    working_radius.to_bits(),
                    "{} DrawPattern S14 at call {index}",
                    case["label"]
                );
                assert_eq!(
                    working_radius.to_bits(),
                    derived_radius.to_bits(),
                    "{} S14 from fixed-width/radius input at call {index}",
                    case["label"]
                );
                assert_eq!(
                    style_step["working_opacity_w20_u32"].as_u64().unwrap() as u32,
                    derived_opacity,
                    "{} DrawPattern W20 opacity at call {index}",
                    case["label"]
                );
                assert_eq!(
                    samples[index].opacity.to_bits(),
                    (derived_opacity as f32).to_bits(),
                    "{} sampled opacity at call {index}",
                    case["label"]
                );
                assert_eq!(expected.len(), 11);
                for (word, value) in expected.iter().enumerate() {
                    assert_eq!(
                        actual.values[word].to_bits(),
                        f32_bits(value).to_bits(),
                        "{} instance {index} word {word}",
                        case["label"]
                    );
                }
            }
            checked += 1;
        }
        assert_eq!(checked, fixture["cases"].as_array().unwrap().len());
    }

    #[test]
    fn redraw_selects_tool_pressure_before_draw_line() {
        assert_eq!(motion_pressure(1, 0.45), 0.5);
        assert_eq!(motion_pressure(3, 0.65), 0.5);
        assert_eq!(motion_pressure(2, 0.5), 0.5);
        assert_eq!(motion_pressure(2, 1.5), 1.0);
        assert_eq!(motion_pressure(2, f32::NAN), 0.5);
        assert_eq!(motion_pressure(2, -0.25), -0.25);
    }

    #[test]
    fn redraw_selects_orientated_tilt_or_pen_defaults() {
        let profile = SAVED_GLV10_CONSTRUCTOR_PROFILE;
        let default_pen = motion_tilt(profile, 1, 0.15, 0.25).unwrap();
        let default_eraser = motion_tilt(profile, 3, -0.1, 0.35).unwrap();
        assert_eq!(
            default_pen.map(f32::to_bits),
            default_eraser.map(f32::to_bits)
        );
        let event_tilt = motion_tilt(profile, 2, 0.0, 0.0).unwrap();
        assert_eq!(
            event_tilt.map(f32::to_bits),
            [-0.0_f32, 0.0001_f32].map(f32::to_bits)
        );
        assert_eq!(
            default_pen.map(f32::to_bits),
            [0.033_985_835_f32, 0.679_150_16_f32].map(f32::to_bits)
        );
    }
}
