//! Source-derived Samsung Notes AirBrush saved redraw model and conformance tests.
//!
//! Production access is restricted to the explicit saved GLV2 adapter below;
//! its sampler and paint contract remain opt-in at the renderer.
use super::path::{P, Quad};

const MAX_POINTS: usize = 65_536;
const MAX_STAMPS: usize = 1_000_000;

/// Apply the captured AirBrush drawable color conversion to a raw ARGB value.
/// Keep the two f32 operations separate: the native code multiplies the alpha
/// byte by 50.0f, divides by 255.0f, truncates toward zero, then clamps the
/// output alpha to [3, 50]. The source RGB bytes pass through unchanged.
pub(super) fn saved_get_color(argb: u32) -> u32 {
    let source_alpha = (argb >> 24) as u8;
    let scaled_alpha = (source_alpha as f32 * 50.0_f32) / 255.0_f32;
    let truncated_alpha = scaled_alpha as i32;
    let output_alpha = truncated_alpha.clamp(3, 50) as u32;
    (argb & 0x00ff_ffff) | (output_alpha << 24)
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Runtime {
    /// drawPoint's native counter seed, captured from drawable state.
    pub counter_seed: u32,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Input<'a> {
    /// Explicit MotionEvent history, including index zero.
    pub history: &'a [P],
    pub endpoint: P,
    pub size: f32,
    pub tool_type: i32,
    pub source: i32,
    pub runtime: Runtime,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Stamp {
    /// RTV1 AddPoint record: x, y, sin(angle), cos(angle).
    pub values: [f32; 4],
}

#[derive(Clone, Debug, Default)]
pub(super) struct Output {
    pub stamps: Vec<Stamp>,
    /// Exclusive stamp count after each historical event's drawLine call.
    pub history_ends: Vec<usize>,
    /// Terminal endPen stamp range.
    pub terminal: std::ops::Range<usize>,
}

fn midpoint(a: P, b: P) -> P {
    a.mid(b)
}

/// Reconstruct the captured V1 saved redraw using the shared movement and
/// sampling state. V1 and V2 differ in PenData writeback; this output models
/// only their observed AddPoint stream, not those side effects.
pub(super) fn saved_redraw(input: Input<'_>) -> Option<Output> {
    let result = saved_redraw_v2(V2Input {
        history: input.history,
        endpoint: input.endpoint,
        initial_position: *input.history.first()?,
        size: input.size,
        tolerance: input.size / 20.0,
        tool_type: input.tool_type,
        source: input.source,
        counter_seed: input.runtime.counter_seed,
    })?;
    Some(Output {
        stamps: result.stamps,
        history_ends: result.history_ends,
        terminal: result.terminal,
    })
}

/// Direct V2 saved-redraw input used only by this module's conformance tests.
#[derive(Clone, Copy, Debug)]
struct V2Input<'a> {
    history: &'a [P],
    endpoint: P,
    initial_position: P,
    size: f32,
    tolerance: f32,
    tool_type: i32,
    source: i32,
    counter_seed: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct V2StateSnapshot {
    counter: u32,
    short_moves: u32,
    tolerance_bits: u32,
    flags: [u8; 5],
}

#[derive(Clone, Copy, Debug, Default)]
struct V2State {
    previous: P,
    midpoint: P,
    counter: u32,
    short_moves: u32,
    tolerance: f32,
    first_point: bool,       // drawable +248
    after_first_short: bool, // drawable +249
    alternate: bool,         // drawable +250
    source_mode: bool,       // drawable +251
    jitter: bool,            // drawable +252
}

impl V2State {
    fn snapshot(self) -> V2StateSnapshot {
        V2StateSnapshot {
            counter: self.counter,
            short_moves: self.short_moves,
            tolerance_bits: self.tolerance.to_bits(),
            flags: [
                self.first_point as u8,
                self.after_first_short as u8,
                self.alternate as u8,
                self.source_mode as u8,
                self.jitter as u8,
            ],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum V2CallKind {
    DrawLine,
    EndPen,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct V2CallState {
    kind: V2CallKind,
    entry: V2StateSnapshot,
    returned: V2StateSnapshot,
}

#[derive(Clone, Debug, Default)]
struct V2Output {
    stamps: Vec<Stamp>,
    history_ends: Vec<usize>,
    terminal: std::ops::Range<usize>,
    calls: Vec<V2CallState>,
    redraw_before: V2StateSnapshot,
    redraw_after: V2StateSnapshot,
    final_state: V2StateSnapshot,
}

const V2_SOURCE_STYLUS: i32 = 0x1002;
const V2_SHORT_MOVE_LIMIT: u32 = 3;
const V2_ALTERNATE_DISTANCE: f32 = 50.0;

fn v2_spacing(size: f32) -> Option<f32> {
    if !size.is_finite() || size <= 0.0 {
        return None;
    }
    // Native code truncates size to int before doubling and dividing by 20.
    let truncated = size.trunc() as i32 as f32;
    let spacing = (truncated + truncated) / 20.0;
    (spacing.is_finite() && spacing > 0.0).then_some(spacing)
}

fn v2_distance(a: P, b: P) -> f32 {
    a.sub(b).len()
}

fn v2_offset(point: &mut P, counter: u32, tolerance: f32) {
    match counter & 7 {
        0 => point.y -= tolerance,
        1 => point.y += tolerance,
        2 => point.x += tolerance,
        3 => point.x -= tolerance,
        4 => {
            point.x -= tolerance;
            point.y -= tolerance;
        }
        5 => {
            point.x += tolerance;
            point.y -= tolerance;
        }
        6 => {
            point.x -= tolerance;
            point.y += tolerance;
        }
        7 => {
            point.x += tolerance;
            point.y += tolerance;
        }
        _ => unreachable!(),
    }
}

fn v2_push_stamp(output: &mut V2Output, state: &mut V2State, mut point: P) -> Option<()> {
    if output.stamps.len() >= MAX_STAMPS {
        return None;
    }
    if state.jitter && v2_distance(point, state.previous) < state.tolerance {
        v2_offset(&mut point, state.counter, state.tolerance);
    }
    let degrees = (state.counter as f32) * 137.0_f32;
    let radians = ((degrees as f64 * std::f64::consts::PI) / 180.0) as f32;
    // Native conformance captures route sincosf through a host binary64 libm
    // callback, then round the sine and cosine lanes to float32.
    let sin = libm::sin(radians as f64) as f32;
    let cos = libm::cos(radians as f64) as f32;
    output.stamps.push(Stamp {
        values: [point.x, point.y, sin, cos],
    });
    state.counter = state.counter.wrapping_add(1);
    Some(())
}

fn v2_emit_path(
    output: &mut V2Output,
    state: &mut V2State,
    start: P,
    control: P,
    end: P,
    spacing: f32,
) -> Option<()> {
    if output.stamps.len() >= MAX_STAMPS
        || !spacing.is_finite()
        || spacing <= 0.0
        || ![start.x, start.y, control.x, control.y, end.x, end.y]
            .into_iter()
            .all(f32::is_finite)
    {
        return None;
    }
    let path = Quad::new(start, control, end);
    if !path.length.is_finite() || path.length <= 0.0 {
        // SmPath::getPosTan fails on an empty path, although the caller still
        // completes the drawable state update after this helper returns.
        return Some(());
    }
    let raw_repeats = (path.length / spacing).floor();
    if !raw_repeats.is_finite() || raw_repeats > (MAX_STAMPS - output.stamps.len()) as f32 {
        return None;
    }
    let repeats = (raw_repeats as usize).max(1);
    let distance_step = path.length / repeats as f32;
    let mut distance = 0.0_f32;
    for _ in 0..repeats {
        v2_push_stamp(output, state, path.at(distance))?;
        distance += distance_step;
    }
    Some(())
}

/// Execute the source-derived GLV2 movement gates for one historical point.
/// A `false` return represents a native early return before SmPath/AddPoint.
fn v2_draw_line(
    output: &mut V2Output,
    state: &mut V2State,
    current: P,
    spacing: f32,
) -> Option<bool> {
    let movement = v2_distance(current, state.previous);
    if !state.source_mode && movement < V2_ALTERNATE_DISTANCE {
        let was_alternate = state.alternate;
        state.alternate = !state.alternate;
        if was_alternate {
            return Some(false);
        }
    } else if movement >= V2_ALTERNATE_DISTANCE {
        state.alternate = true;
    }

    if movement < state.tolerance {
        state.short_moves = state.short_moves.wrapping_add(1);
        if state.short_moves != V2_SHORT_MOVE_LIMIT {
            return Some(false);
        }
        state.short_moves = 0;
        state.after_first_short = true;
        state.jitter = true;
    } else {
        state.jitter = false;
        state.short_moves = 0;
        if state.source_mode && !state.after_first_short {
            state.previous = current;
            state.after_first_short = true;
            return Some(false);
        }
    }

    let start = state.midpoint;
    let next_midpoint = midpoint(state.previous, current);
    // Native drawLine moves to the prior midpoint, curves through the old
    // previous point, and ends at the midpoint between previous and current.
    // endPen alone uses the current event endpoint as its quadratic end.
    v2_emit_path(output, state, start, state.previous, next_midpoint, spacing)?;
    state.midpoint = next_midpoint;
    state.previous = current;
    state.first_point = false;
    state.after_first_short = false;
    Some(true)
}

fn v2_end_pen(output: &mut V2Output, state: &mut V2State, endpoint: P, spacing: f32) -> Option<()> {
    if state.first_point {
        return v2_push_stamp(output, state, state.previous);
    }
    v2_emit_path(
        output,
        state,
        state.midpoint,
        state.previous,
        endpoint,
        spacing,
    )
}

fn saved_redraw_v2(input: V2Input<'_>) -> Option<V2Output> {
    let V2Input {
        history,
        endpoint,
        initial_position,
        size,
        tolerance,
        tool_type,
        source,
        counter_seed,
    } = input;
    let spacing = v2_spacing(size)?;
    if history.is_empty()
        || history.len() > MAX_POINTS
        || ![
            endpoint.x,
            endpoint.y,
            initial_position.x,
            initial_position.y,
        ]
        .into_iter()
        .all(f32::is_finite)
        || history
            .iter()
            .any(|point| !point.x.is_finite() || !point.y.is_finite())
    {
        return None;
    }

    if !tolerance.is_finite() || tolerance <= 0.0 {
        return None;
    }
    let redraw_before = V2StateSnapshot {
        counter: counter_seed,
        short_moves: 0,
        tolerance_bits: tolerance.to_bits(),
        flags: [1, 1, 0, 0, 0],
    };
    let source_mode =
        !(tool_type == 2 && source == V2_SOURCE_STYLUS) && tool_type != 1 && tool_type != 3;
    let mut state = V2State {
        // RedrawPen replaces the constructor/runtime position with history[0]
        // before its historical drawLine loop.
        previous: history[0],
        midpoint: history[0],
        counter: 0, // RedrawPen resets +232 regardless of the pre-call seed.
        short_moves: 0,
        // RedrawPen overwrites +244 with saved size / 20 before processing.
        tolerance: size / 20.0_f32,
        first_point: true,
        after_first_short: true,
        alternate: false,
        source_mode,
        jitter: false,
    };
    let mut output = V2Output {
        redraw_before,
        ..V2Output::default()
    };

    for &current in history.iter().skip(1) {
        let entry = state.snapshot();
        v2_draw_line(&mut output, &mut state, current, spacing)?;
        output.calls.push(V2CallState {
            kind: V2CallKind::DrawLine,
            entry,
            returned: state.snapshot(),
        });
        output.history_ends.push(output.stamps.len());
    }

    output.terminal.start = output.stamps.len();
    let entry = state.snapshot();
    v2_end_pen(&mut output, &mut state, endpoint, spacing)?;
    output.terminal.end = output.stamps.len();
    output.calls.push(V2CallState {
        kind: V2CallKind::EndPen,
        entry,
        returned: state.snapshot(),
    });
    output.redraw_after = state.snapshot();
    output.final_state = state.snapshot();
    Some(output)
}

/// Minimal saved GLV2 output consumed by the production texture renderer.
#[derive(Clone, Debug)]
pub(crate) struct SavedV2Output {
    pub(crate) rows: Vec<[f32; 4]>,
    pub(crate) sample_ends: Vec<usize>,
    pub(crate) terminal: std::ops::Range<usize>,
    pub(crate) rt_size: u32,
    pub(crate) native_argb: u32,
}

/// Reconstruct only the captured empty or `2;` saved ObjectStroke GLV2 paths.
/// Saved playback derives MotionEvent source zero in the native wrapper; each
/// redraw initializes fresh V2 state and resets its stamp counter.
pub(crate) fn saved_v2_from_stroke(stroke: &crate::Stroke) -> Option<SavedV2Output> {
    saved_from_stroke(stroke, false)
}

pub(crate) fn saved_from_stroke(stroke: &crate::Stroke, v1: bool) -> Option<SavedV2Output> {
    let rendering = stroke.rendering.as_ref()?;
    if !(if v1 {
        matches!(rendering.advanced_settings.as_deref(), None | Some("1;"))
    } else {
        matches!(
            rendering.advanced_settings.as_deref(),
            Some("") | Some("2;")
        )
    }) || !rendering.properties.compressed
        || !(rendering.tool_type_raw == 2
            || (!v1
                && (matches!(rendering.tool_type_raw, 1 | 3)
                    || (rendering.tool_type_raw == 0
                        && rendering.advanced_settings.as_deref() == Some("")))))
    {
        return None;
    }
    let source_argb = super::saved_airbrush_argb_with_extra_tools(stroke, !v1)?;
    let raw_size = rendering.style.pen_size?;
    if !raw_size.is_finite() {
        return None;
    }
    let size = raw_size.clamp(0.4, 800.0);
    let rt_size = size as u32;
    if rt_size == 0 {
        return None;
    }
    let count = stroke.points.len();
    if !(2..=MAX_POINTS).contains(&count)
        || stroke.pressures.len() != count
        || stroke.timestamps.len() != count
        || stroke.tilts.len() != count
        || stroke.orientations.len() != count
    {
        return None;
    }
    if stroke
        .pressures
        .iter()
        .chain(&stroke.tilts)
        .chain(&stroke.orientations)
        .any(|value| !(*value as f32).is_finite())
        || stroke
            .timestamps
            .iter()
            .any(|value| i32::try_from(*value).is_err())
    {
        return None;
    }
    let points: Vec<P> = stroke
        .points
        .iter()
        .map(|point| P {
            x: point.x as f32,
            y: point.y as f32,
        })
        .collect();
    if points
        .iter()
        .any(|point| !point.x.is_finite() || !point.y.is_finite())
    {
        return None;
    }
    let output = if v1 {
        saved_redraw(Input {
            history: &points[..count - 1],
            endpoint: *points.last()?,
            size,
            tool_type: i32::from(rendering.tool_type_raw),
            source: 0,
            runtime: Runtime { counter_seed: 0 },
        })?
    } else {
        let output = saved_redraw_v2(V2Input {
            history: &points[..count - 1],
            endpoint: *points.last()?,
            initial_position: points[0],
            size,
            tolerance: size / 20.0,
            tool_type: i32::from(rendering.tool_type_raw),
            source: 0,
            counter_seed: 0,
        })?;
        Output {
            stamps: output.stamps,
            history_ends: output.history_ends,
            terminal: output.terminal,
        }
    };
    if output.stamps.len() > MAX_STAMPS
        || output.history_ends.len() != count.saturating_sub(2)
        || output.history_ends.windows(2).any(|ends| ends[0] > ends[1])
        || output
            .history_ends
            .last()
            .is_some_and(|end| *end > output.stamps.len())
        || output.terminal.start > output.terminal.end
        || output.terminal.end != output.stamps.len()
    {
        return None;
    }
    let rows = output
        .stamps
        .into_iter()
        .map(|stamp| stamp.values)
        .collect();
    Some(SavedV2Output {
        rows,
        sample_ends: output.history_ends,
        terminal: output.terminal,
        rt_size,
        native_argb: saved_get_color(source_argb),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn v1_replay_boundaries_match_native_calls() {
        // Native MotionEvent redraw captures, including empty drawLine calls:
        // conformance/airbrush-v1-replay-boundaries-native.json.
        let curve = [(0., 0.), (8., 5.), (16., 0.), (24., -5.)].map(|(x, y)| P { x, y });
        let short = std::array::from_fn::<_, 8, _>(|i| P {
            x: i as f32 * 0.0625,
            y: 0.,
        });
        for (history, endpoint, size, source, ends, terminal) in [
            (
                &curve[..],
                P { x: 32., y: 0. },
                10.,
                0,
                vec![4, 4, 20],
                20..32,
            ),
            (
                &short[..],
                P { x: 0.5, y: 0. },
                14.,
                0,
                vec![0, 0, 1, 1, 1, 2, 2],
                2..3,
            ),
            (
                &short[..],
                P { x: 0.5, y: 0. },
                14.,
                4098,
                vec![0, 0, 0, 0, 1, 1, 1],
                1..2,
            ),
        ] {
            let output = saved_redraw(Input {
                history,
                endpoint,
                size,
                tool_type: 2,
                source,
                runtime: Runtime { counter_seed: 0 },
            })
            .unwrap();
            assert_eq!(output.history_ends, ends);
            assert_eq!(output.terminal, terminal);
        }
    }

    fn point(value: &Value) -> P {
        let v = value.as_array().unwrap();
        P {
            x: v[0].as_f64().unwrap() as f32,
            y: v[1].as_f64().unwrap() as f32,
        }
    }

    fn bits(value: &str) -> f32 {
        f32::from_bits(u32::from_str_radix(value, 16).unwrap())
    }

    #[test]
    fn saved_v1_matches_every_native_addpoint_record() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/airbrush-saved-native.json"
        ))
        .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let input = &case["input"];
            let runtime = &case["runtime_context"];
            let history: Vec<P> = input["history"]
                .as_array()
                .unwrap()
                .iter()
                .map(point)
                .collect();
            let output = saved_redraw(Input {
                history: &history,
                endpoint: point(&input["endpoint"]),
                size: input["size"].as_f64().unwrap() as f32,
                tool_type: if input["id"].as_str().unwrap().starts_with("tool2") {
                    2
                } else {
                    1
                },
                source: 0,
                runtime: Runtime {
                    counter_seed: runtime["counter_seed"].as_u64().unwrap() as u32,
                },
            })
            .unwrap();
            let native = case["native"]["stamps"].as_array().unwrap();
            assert_eq!(output.stamps.len(), native.len(), "{} count", input["id"]);
            for (index, (actual, expected)) in output.stamps.iter().zip(native).enumerate() {
                for component in 0..4 {
                    let expected = bits(expected[component].as_str().unwrap());
                    assert_eq!(
                        actual.values[component].to_bits(),
                        expected.to_bits(),
                        "{} stamp {index} component {component}",
                        input["id"]
                    );
                }
            }
        }
    }

    fn read_u16_le(bytes: &[u8], offset: &mut usize) -> u16 {
        let value = u16::from_le_bytes(bytes[*offset..*offset + 2].try_into().unwrap());
        *offset += 2;
        value
    }

    fn read_u32_le(bytes: &[u8], offset: &mut usize) -> u32 {
        let value = u32::from_le_bytes(bytes[*offset..*offset + 4].try_into().unwrap());
        *offset += 4;
        value
    }

    fn read_v2_state(bytes: &[u8], offset: &mut usize) -> V2StateSnapshot {
        let counter = read_u32_le(bytes, offset);
        let short_moves = read_u32_le(bytes, offset);
        let tolerance_bits = read_u32_le(bytes, offset);
        let flags = bytes[*offset..*offset + 5].try_into().unwrap();
        *offset += 5;
        V2StateSnapshot {
            counter,
            short_moves,
            tolerance_bits,
            flags,
        }
    }

    fn f32_le_pair(value: &Value) -> P {
        let pair = value.as_array().unwrap();
        P {
            x: little_endian_f32(pair[0].as_str().unwrap()),
            y: little_endian_f32(pair[1].as_str().unwrap()),
        }
    }

    fn boundary_history(metadata: &Value, history_id: &str) -> Vec<P> {
        let spec = &metadata["histories"][history_id];
        match spec["encoding"].as_str().unwrap() {
            "points-f32-le" => spec["points"]
                .as_array()
                .unwrap()
                .iter()
                .map(f32_le_pair)
                .collect(),
            "linear-f32-le" => {
                let mut point = f32_le_pair(&spec["start_f32_le"]);
                let step = f32_le_pair(&spec["step_f32_le"]);
                let mut points = vec![point];
                let transitions = spec["transitions"].as_u64().unwrap() as usize;
                for _ in 0..transitions {
                    point = P {
                        x: point.x + step.x,
                        y: point.y + step.y,
                    };
                    points.push(point);
                }
                points
            }
            other => panic!("unknown AirBrush history encoding {other}"),
        }
    }

    #[test]
    fn saved_v2_full_state_matches_compact_native_bundle() {
        const BUNDLE_JSON: &str =
            include_str!("../../../../conformance/airbrush-boundary-native.json");
        const BUNDLE_BIN: &[u8] =
            include_bytes!("../../../../conformance/airbrush-boundary-native.bin");
        let metadata: Value = serde_json::from_str(BUNDLE_JSON).unwrap();
        assert_eq!(metadata["schema"], "sdocx.airbrush-boundary-native.v1");
        use sha2::{Digest, Sha256};
        assert_eq!(
            format!("{:x}", Sha256::digest(BUNDLE_BIN)),
            metadata["binary_sha256"].as_str().unwrap(),
            "compact AirBrush bundle digest"
        );
        assert_eq!(&BUNDLE_BIN[..4], b"ABN1");
        assert_eq!(u16::from_le_bytes(BUNDLE_BIN[4..6].try_into().unwrap()), 1);
        let case_count = u16::from_le_bytes(BUNDLE_BIN[6..8].try_into().unwrap()) as usize;
        let cases = metadata["cases"].as_array().unwrap();
        assert_eq!(cases.len(), case_count);

        let mut offset = 8;
        for (case_index, case) in cases.iter().enumerate() {
            let native_index = read_u16_le(BUNDLE_BIN, &mut offset) as usize;
            let stamp_count = read_u16_le(BUNDLE_BIN, &mut offset) as usize;
            let call_count = read_u16_le(BUNDLE_BIN, &mut offset) as usize;
            let state_present = read_u16_le(BUNDLE_BIN, &mut offset) != 0;
            assert_eq!(native_index, case_index, "{} bundle index", case["id"]);
            assert_eq!(state_present, case["state_capture"].as_bool().unwrap());

            let native_redraw_states = if state_present {
                Some((
                    read_v2_state(BUNDLE_BIN, &mut offset),
                    read_v2_state(BUNDLE_BIN, &mut offset),
                    read_v2_state(BUNDLE_BIN, &mut offset),
                ))
            } else {
                None
            };
            let expected_rows: Vec<[u32; 4]> = (0..stamp_count)
                .map(|_| std::array::from_fn(|_| read_u32_le(BUNDLE_BIN, &mut offset)))
                .collect();
            let expected_calls: Vec<V2CallState> = (0..call_count)
                .map(|_| {
                    let kind = match BUNDLE_BIN[offset] {
                        0 => V2CallKind::DrawLine,
                        1 => V2CallKind::EndPen,
                        other => panic!("unknown native AirBrush call kind {other}"),
                    };
                    offset += 1;
                    V2CallState {
                        kind,
                        entry: read_v2_state(BUNDLE_BIN, &mut offset),
                        returned: read_v2_state(BUNDLE_BIN, &mut offset),
                    }
                })
                .collect();

            let history = boundary_history(&metadata, case["history"].as_str().unwrap());
            let input = V2Input {
                history: &history,
                endpoint: f32_le_pair(&case["endpoint_f32_le"]),
                initial_position: f32_le_pair(&case["initial_f32_le"]),
                size: little_endian_f32(case["size_f32_le"].as_str().unwrap()),
                tolerance: little_endian_f32(case["tolerance_f32_le"].as_str().unwrap()),
                tool_type: case["tool_type"].as_i64().unwrap() as i32,
                source: case["source"].as_i64().unwrap() as i32,
                counter_seed: case["counter_seed"].as_u64().unwrap() as u32,
            };
            let output = saved_redraw_v2(input).unwrap();
            let actual_rows: Vec<[u32; 4]> = output
                .stamps
                .iter()
                .map(|stamp| stamp.values.map(f32::to_bits))
                .collect();
            assert_eq!(actual_rows, expected_rows, "{} AddPoint stream", case["id"]);

            if let Some((before, after, final_state)) = native_redraw_states {
                assert_eq!(output.redraw_before, before, "{} pre-state", case["id"]);
                assert_eq!(output.redraw_after, after, "{} post-state", case["id"]);
                assert_eq!(
                    output.final_state, final_state,
                    "{} final state",
                    case["id"]
                );
                assert_eq!(
                    output.calls, expected_calls,
                    "{} per-call states",
                    case["id"]
                );
            }
        }
        assert_eq!(
            offset,
            BUNDLE_BIN.len(),
            "compact AirBrush bundle has trailing bytes"
        );
    }

    fn little_endian_f32(hex: &str) -> f32 {
        let bytes: Vec<u8> = hex
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        f32::from_le_bytes(bytes.try_into().unwrap())
    }

    #[test]
    fn v1_two_point_stationary_drawing_matches_native_stamp() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/airbrush-v1-stationary-two-native.json"
        ))
        .unwrap();
        assert_eq!(fixture["result"]["selected_drawable_profile_version"], 1);
        let point = P {
            x: 128.453_13,
            y: 128.515_63,
        };
        let output = saved_redraw(Input {
            history: &[point],
            endpoint: point,
            size: fixture["input"]["size"].as_f64().unwrap() as f32,
            tool_type: 2,
            source: 0,
            runtime: Runtime { counter_seed: 0 },
        })
        .unwrap();
        let actual: String = output
            .stamps
            .iter()
            .flat_map(|stamp| stamp.values)
            .flat_map(f32::to_le_bytes)
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(actual, fixture["result"]["buffer_hex"].as_str().unwrap());
    }

    #[test]
    fn v1_boundary_streams_match_native() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/airbrush-v1-v2-boundary-native.json"
        ))
        .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let points: Vec<P> = case["points_f32_le"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| P {
                    x: little_endian_f32(p[0].as_str().unwrap()),
                    y: little_endian_f32(p[1].as_str().unwrap()),
                })
                .collect();
            let output = saved_redraw(Input {
                history: &points[..7],
                endpoint: points[7],
                size: 14.0,
                tool_type: case["tool_type"].as_i64().unwrap() as i32,
                source: case["source"].as_i64().unwrap() as i32,
                runtime: Runtime { counter_seed: 0 },
            })
            .unwrap();
            let actual: Vec<_> = output
                .stamps
                .iter()
                .map(|s| s.values.map(f32::to_bits))
                .collect();
            let expected: Vec<[u32; 4]> = case["v1_stamps_bits"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| {
                    std::array::from_fn(|i| {
                        u32::from_str_radix(row[i].as_str().unwrap(), 16).unwrap()
                    })
                })
                .collect();
            assert_eq!(actual, expected, "native V1 boundary: {case}");
            assert_eq!(case["v2_matches_v1"], true);
        }
    }

    #[test]
    fn v2_tool_motion_streams_match_native_drawing() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/airbrush-tool-motion-native.json"
        ))
        .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let bytes = fixture["source_arrays"]["points"]["data_hex"]
                .as_str()
                .unwrap();
            let points: Vec<P> = bytes
                .as_bytes()
                .as_chunks::<16>()
                .0
                .iter()
                .map(|pair| {
                    let pair = std::str::from_utf8(pair).unwrap();
                    P {
                        x: little_endian_f32(&pair[..8]),
                        y: little_endian_f32(&pair[8..]),
                    }
                })
                .collect();
            let size = case["model_state"]["size"].as_f64().unwrap() as f32;
            let output = saved_redraw_v2(V2Input {
                history: &points[..points.len() - 1],
                endpoint: *points.last().unwrap(),
                initial_position: points[0],
                size,
                tolerance: size / 20.0,
                tool_type: case["tool_type"].as_i64().unwrap() as i32,
                source: 0,
                counter_seed: 0,
            })
            .unwrap();
            let actual: String = output
                .stamps
                .iter()
                .flat_map(|stamp| stamp.values)
                .flat_map(f32::to_le_bytes)
                .map(|byte| format!("{byte:02x}"))
                .collect();
            assert_eq!(
                actual,
                case["stream_hex"].as_str().unwrap(),
                "tool {}",
                case["tool_type"]
            );
        }
    }

    #[test]
    fn saved_v1_stationary_stream_matches_native_drawing_matrix() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/airbrush-v1-stationary-matrix-native.json"
        ))
        .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let point_hex = case["points_f32_le"].as_str().unwrap();
            let points: Vec<P> = point_hex
                .as_bytes()
                .as_chunks::<16>()
                .0
                .iter()
                .map(|point| P {
                    x: little_endian_f32(std::str::from_utf8(&point[..8]).unwrap()),
                    y: little_endian_f32(std::str::from_utf8(&point[8..]).unwrap()),
                })
                .collect();
            let size = case["size"].as_f64().unwrap() as f32;
            let output = saved_redraw(Input {
                history: &points[..points.len() - 1],
                endpoint: *points.last().unwrap(),
                size,
                tool_type: 2,
                source: 0,
                runtime: Runtime { counter_seed: 0 },
            })
            .unwrap();
            let expected = case["rows_f32_le"].as_str().unwrap();
            let actual: String = output
                .stamps
                .iter()
                .flat_map(|stamp| stamp.values)
                .flat_map(f32::to_le_bytes)
                .map(|byte| format!("{byte:02x}"))
                .collect();
            assert_eq!(
                actual,
                expected,
                "native stationary count {} size {size}",
                points.len()
            );
        }
    }

    #[test]
    fn short_saved_v1_native_stream_matches_shared_sampler() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/airbrush-short-v1-v2-native.json"
        ))
        .unwrap();
        let cases = fixture["cases"].as_array().unwrap();
        let first = &cases[0];
        assert_eq!(first["profile"], "v1");
        assert_eq!(first["selected_drawable_profile_version"], 1);
        assert_eq!(cases[1]["selected_drawable_profile_version"], 2);
        assert_eq!(
            first["producer"]["buffer_hex"],
            cases[1]["producer"]["buffer_hex"]
        );
        let point_hex = first["input"]["points_f32_le"].as_str().unwrap();
        let points: Vec<P> = point_hex
            .as_bytes()
            .as_chunks::<16>()
            .0
            .iter()
            .map(|point| P {
                x: little_endian_f32(std::str::from_utf8(&point[..8]).unwrap()),
                y: little_endian_f32(std::str::from_utf8(&point[8..]).unwrap()),
            })
            .collect();
        let size = little_endian_f32(first["input"]["size_f32_le"].as_str().unwrap());
        let history = &points[..points.len() - 1];
        let endpoint = *points.last().unwrap();
        let expected_hex = first["producer"]["buffer_hex"].as_str().unwrap();
        let expected: Vec<[u32; 4]> = expected_hex
            .as_bytes()
            .as_chunks::<32>()
            .0
            .iter()
            .map(|row| {
                std::array::from_fn(|i| {
                    little_endian_f32(std::str::from_utf8(&row[i * 8..i * 8 + 8]).unwrap())
                        .to_bits()
                })
            })
            .collect();
        let v2 = saved_redraw_v2(V2Input {
            history,
            endpoint,
            initial_position: points[0],
            size,
            tolerance: size / 20.0,
            tool_type: 2,
            source: 0,
            counter_seed: 0,
        })
        .unwrap();
        let v2_rows: Vec<_> = v2
            .stamps
            .iter()
            .map(|s| s.values.map(f32::to_bits))
            .collect();
        assert_eq!(v2_rows, expected, "V2 model versus paired native captures");
        let legacy = saved_redraw(Input {
            history,
            endpoint,
            size,
            tool_type: 2,
            source: 0,
            runtime: Runtime { counter_seed: 0 },
        })
        .unwrap();
        assert_eq!(
            legacy
                .stamps
                .iter()
                .map(|s| s.values.map(f32::to_bits))
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(expected.len(), 2, "native V1 emits short-move stamps");
    }

    #[test]
    fn saved_caller_records_match_from_model_points_and_drawable_size() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/airbrush-saved-caller-native.json"
        ))
        .unwrap();
        for case in fixture["cases"].as_object().unwrap().values() {
            let input = &case["input"];
            match input["advanced_setting_mode"].as_str().unwrap() {
                "unset" => {
                    assert_eq!(
                        input["advanced_setting_mutation"],
                        "SetAdvancedPenSetting was omitted; native getter returned null"
                    );
                    assert!(input["native_model_getter_snapshot"]["advanced_setting"].is_null());
                    assert_eq!(
                        case["native_selection"]["selected_drawable_profile_version"],
                        1
                    );
                }
                "explicit-empty" => {
                    assert_eq!(
                        input["advanced_setting_mutation"],
                        "SetAdvancedPenSetting was called with empty String"
                    );
                    assert_eq!(
                        input["native_model_getter_snapshot"]["advanced_setting"],
                        ""
                    );
                    assert_eq!(
                        case["native_selection"]["selected_drawable_profile_version"],
                        2
                    );
                }
                other => panic!("unexpected setting mode {other}"),
            }
            let encoded = input["source_arrays"]["points"]["data_hex"]
                .as_str()
                .unwrap();
            let bytes: Vec<u8> = encoded
                .as_bytes()
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect();
            let points: Vec<P> = bytes
                .as_chunks::<8>()
                .0
                .iter()
                .map(|point| P {
                    x: f32::from_le_bytes(point[..4].try_into().unwrap()),
                    y: f32::from_le_bytes(point[4..].try_into().unwrap()),
                })
                .collect();
            let output = saved_redraw(Input {
                history: &points[..points.len() - 1],
                endpoint: *points.last().unwrap(),
                size: little_endian_f32(input["size_f32_bits"].as_str().unwrap()),
                tool_type: 2,
                source: 0,
                runtime: Runtime { counter_seed: 0 },
            })
            .unwrap();
            assert_eq!(
                little_endian_f32(input["size_f32_bits"].as_str().unwrap()).to_bits(),
                little_endian_f32(case["native_selection"]["size_f32_bits"].as_str().unwrap())
                    .to_bits(),
                "{} Model versus selected drawable size",
                case["case_id"]
            );
            let expected: Vec<[u32; 4]> = case["producer"]["append_rows"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| {
                    let bytes: Vec<u8> = row["delta_bytes"]
                        .as_str()
                        .unwrap()
                        .as_bytes()
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|pair| {
                            u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap()
                        })
                        .collect();
                    std::array::from_fn(|i| {
                        u32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap())
                    })
                })
                .collect();
            let actual: Vec<[u32; 4]> = output
                .stamps
                .iter()
                .map(|stamp| stamp.values.map(f32::to_bits))
                .collect();
            assert_eq!(actual.len(), expected.len(), "{} count", case["case_id"]);
            assert_eq!(actual, expected, "{}", case["case_id"]);
            let actual_buffer: Vec<u8> = output
                .stamps
                .iter()
                .flat_map(|stamp| stamp.values.into_iter().flat_map(f32::to_le_bytes))
                .collect();
            let actual_hex: String = actual_buffer
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            assert_eq!(actual_hex, case["producer"]["buffer_hex"]);
        }
    }

    #[test]
    fn saved_native_color_matches_all_getter_inputs_and_drawing_controls() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/airbrush-color-native.json"
        ))
        .unwrap();
        let source_rgb = u32::from_str_radix(
            fixture["source_rgb_u24"]
                .as_str()
                .unwrap()
                .trim_start_matches("0x"),
            16,
        )
        .unwrap();

        let sweep = fixture["selected_receiver_alpha_sweep"].as_array().unwrap();
        assert_eq!(sweep.len(), 256);
        for (source_alpha, row) in sweep.iter().enumerate() {
            assert_eq!(
                row["source_alpha_u8"].as_u64().unwrap(),
                source_alpha as u64
            );
            let source_argb = ((source_alpha as u32) << 24) | source_rgb;
            let expected = u32::from_str_radix(
                row["returned_argb_u32"]
                    .as_str()
                    .unwrap()
                    .trim_start_matches("0x"),
                16,
            )
            .unwrap();
            assert_eq!(
                saved_get_color(source_argb),
                expected,
                "source alpha {source_alpha}"
            );
            assert_eq!(saved_get_color(source_argb) & 0x00ff_ffff, source_rgb);
        }

        for control in fixture["full_drawing_controls"].as_array().unwrap() {
            let source = u32::from_str_radix(
                control["source_argb_u32"]
                    .as_str()
                    .unwrap()
                    .trim_start_matches("0x"),
                16,
            )
            .unwrap();
            let expected = u32::from_str_radix(
                control["rt_setpendata_argb_u32"]
                    .as_str()
                    .unwrap()
                    .trim_start_matches("0x"),
                16,
            )
            .unwrap();
            assert_eq!(saved_get_color(source), expected, "source {source:#010x}");
            assert_eq!(source & 0x00ff_ffff, source_rgb);
            assert_eq!(
                control["direct_native_getter_argb_u32"],
                control["rt_setpendata_argb_u32"]
            );
        }
    }
    #[cfg(feature = "render")]
    fn png_data_url_rgba(width: u32, height: u32, pixels: &[u8]) -> String {
        use base64::Engine as _;
        let mut encoded = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut encoded, width, height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(pixels).unwrap();
        }
        format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(encoded)
        )
    }

    #[cfg(feature = "render")]
    fn native_prealpha_png(pattern: &[u8], size: u32, argb: u32) -> (String, f32) {
        assert_eq!(pattern.len(), 96 * 96);
        let native_argb = saved_get_color(argb);
        let rgb = [
            ((native_argb >> 16) & 0xff) as u8,
            ((native_argb >> 8) & 0xff) as u8,
            (native_argb & 0xff) as u8,
        ];
        let style_alpha = (native_argb >> 24) as u8;
        let half = size as f32 / 2.0;
        let extent = half * 2.0;
        let texel_px = extent / 96.0;
        // Nearest image filtering still antialiases the image's outer bounds.
        // Extend real CLAMP_TO_EDGE samples past a whole output pixel; retain
        // the native quad separately as a crisp clip. This is a 1x oracle.
        let pad = (1.0 / texel_px).ceil() as u32 + 1;
        let pad_px = pad as f32 * texel_px;
        let edge = 96 + 2 * pad;
        let mut rgba = Vec::with_capacity((edge * edge * 4) as usize);
        for y in 0..edge {
            let source_y = y.saturating_sub(pad).min(95) as usize;
            for x in 0..edge {
                let source_x = x.saturating_sub(pad).min(95) as usize;
                let r8 = pattern[source_y * 96 + source_x] as u16;
                let alpha = ((r8 * style_alpha as u16 + 127) / 255) as u8;
                rgba.extend_from_slice(&[rgb[0], rgb[1], rgb[2], alpha]);
            }
        }
        (png_data_url_rgba(edge, edge, &rgba), pad_px)
    }

    #[cfg(feature = "render")]
    fn native_texture_svg(stamps: &[Stamp], size: u32, data_url: &str, pad_px: f32) -> String {
        use std::fmt::Write as _;
        let half = size as f32 / 2.0;
        let extent = half * 2.0;
        let image_extent = extent + 2.0 * pad_px;
        let mut svg = String::from(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"256\" height=\"256\" viewBox=\"0 0 256 256\">",
        );
        write!(svg, "<defs><clipPath id=\"native-quad-clip\" clipPathUnits=\"userSpaceOnUse\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" shape-rendering=\"crispEdges\"/></clipPath><image id=\"padded-airbrush-pattern\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\" image-rendering=\"pixelated\" style=\"image-rendering: pixelated\" href=\"{}\"/></defs>",
            -half, -half, extent, extent, -half - pad_px, -half - pad_px,
            image_extent, image_extent, data_url).unwrap();
        for (index, stamp) in stamps.iter().enumerate() {
            let [x, y, sin, cos] = stamp.values;
            write!(svg, "<g data-instance=\"{}\" transform=\"matrix({},{},{},{},{},{})\" clip-path=\"url(#native-quad-clip)\"><use href=\"#padded-airbrush-pattern\"/></g>",
                index, cos, -sin, sin, cos, x, 256.0_f32 - y).unwrap();
        }
        svg.push_str("</svg>");
        svg
    }

    #[cfg(feature = "render")]
    fn native_texture_svg_with_draw_matrix(
        stamps: &[Stamp],
        size: u32,
        data_url: &str,
        pad_px: f32,
        draw_matrix: [f32; 16],
        background: [u8; 4],
    ) -> String {
        let mut svg = native_texture_svg(stamps, size, data_url, pad_px);
        if background[3] != 0 {
            let at = svg.find("<defs>").unwrap();
            svg.insert_str(
                at,
                &format!(
                    "<rect width=\"256\" height=\"256\" fill=\"#{:02x}{:02x}{:02x}\" fill-opacity=\"{:.17}\"/>",
                    background[0], background[1], background[2], background[3] as f64 / 255.0
                ),
            );
        }
        let [a, b, c, d, e, f] = native_draw_matrix_svg_conjugate(draw_matrix);
        let first = svg.find("<g data-instance=\"0\"").unwrap();
        // Insert the wrapper before the first stamp; close it before the root SVG.
        let wrapper =
            format!("<g transform=\"matrix({a:.17},{b:.17},{c:.17},{d:.17},{e:.17},{f:.17})\">");
        svg.insert_str(first, &wrapper);
        let end = svg.rfind("</svg>").unwrap();
        svg.insert_str(end, "</g>");
        svg
    }

    #[cfg(feature = "render")]
    fn native_draw_matrix_svg_conjugate(matrix: [f32; 16]) -> [f64; 6] {
        // T * M * T^-1 for the GL bottom-left and SVG top-left coordinate systems.
        let (a, b, c, d, tx, ty) = (
            matrix[0] as f64,
            matrix[1] as f64,
            matrix[4] as f64,
            matrix[5] as f64,
            matrix[12] as f64,
            matrix[13] as f64,
        );
        [a, -b, -c, d, c * 256.0 + tx, 256.0 * (1.0 - d) - ty]
    }

    #[cfg(feature = "render")]
    fn native_matrix_from_bits(value: &Value) -> [f32; 16] {
        let words = value.as_array().unwrap();
        assert_eq!(words.len(), 16);
        let mut matrix = [0.0_f32; 16];
        for (index, word) in words.iter().enumerate() {
            matrix[index] = f32::from_bits(
                u32::from_str_radix(word.as_str().unwrap().trim_start_matches("0x"), 16).unwrap(),
            );
        }
        matrix
    }

    #[cfg(feature = "render")]
    #[test]
    fn saved_v2_matrix_controls_match_compact_native_frames() {
        use sha2::{Digest, Sha256};

        const JSON: &str = include_str!("../../../../conformance/airbrush-matrix-native.json");
        const BIN: &[u8] = include_bytes!("../../../../conformance/airbrush-matrix-native.bin");
        const PAINT: &str = include_str!("../../../../conformance/airbrush-paint-native.json");
        const PATTERN: &[u8] = include_bytes!("../../../../conformance/airbrush-pattern.r8");
        const JSON_SHA256: &str =
            "72b10ef6f67b16e023530b49227e092830f59e11593a27681e142f54fc3ff605";
        const BIN_SHA256: &str = "99c61ff47e32bb27b9df1834fa8844234946e362ab3c353af5843823836f5a73";

        assert_eq!(
            format!("{:x}", Sha256::digest(JSON.as_bytes())),
            JSON_SHA256
        );
        assert_eq!(format!("{:x}", Sha256::digest(BIN)), BIN_SHA256);
        let meta: Value = serde_json::from_str(JSON).unwrap();
        assert_eq!(meta["schema"], "sdocx.airbrush-matrix-paint-oracles.v1");
        let profile = &meta["shared_native_provenance"]["native_wrapper_profile"];
        assert_eq!(
            profile["pen_name"],
            "com.samsung.android.sdk.pen.pen.preload.AirBrushPen"
        );
        assert_eq!(profile["advanced_setting_mode"], "explicit-empty");
        assert_eq!(profile["selected_drawable_version"], 2);
        assert_eq!(profile["tool_type_raw"], 2);
        assert_eq!(profile["motion_event_source"], 0);
        assert_eq!(&BIN[..4], b"ABM1");
        assert_eq!(u16::from_le_bytes(BIN[4..6].try_into().unwrap()), 1);
        let cases = meta["controls"].as_array().unwrap();
        assert_eq!(cases.len(), 6);
        assert_eq!(
            u16::from_le_bytes(BIN[6..8].try_into().unwrap()) as usize,
            cases.len()
        );
        assert_eq!(meta["open_case"]["id"], "translate-32-48-transparent");
        assert_eq!(meta["open_case"]["exact_rgba"], false);
        assert_eq!(meta["open_case"]["differing_pixels"], 3);
        assert_eq!(meta["open_case"]["differing_bytes"], 11);

        let paint: Value = serde_json::from_str(PAINT).unwrap();
        let identity = paint["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|case| case["id"] == "source-size-3p144-rt3")
            .unwrap();
        assert_eq!(
            identity["native_capture_sha256"],
            meta["identity_control_reuse"]["existing_paint_fixture_capture_sha256"]
        );
        assert_eq!(
            identity["native_readback_top_down_sha256"],
            meta["identity_control_reuse"]["frame_top_down_sha256"]
        );
        let source = &identity["source"];
        assert_eq!(
            source["source_size_f32"].as_f64().unwrap() as f32,
            3.144_f32
        );
        assert_eq!(source["native_rt_size_u32"], 3);
        assert_eq!(source["tool_type_raw"], 2);
        assert_eq!(source["motion_event_source"], 0);
        assert_eq!(source["counter_seed"], 0);
        let points: Vec<P> = source["points_f32"]
            .as_array()
            .unwrap()
            .iter()
            .map(point)
            .collect();
        let size = source["source_size_f32"].as_f64().unwrap() as f32;
        let output = saved_redraw_v2(V2Input {
            history: &points[..points.len() - 1],
            endpoint: *points.last().unwrap(),
            initial_position: P { x: 0.0, y: 0.0 },
            size,
            tolerance: size / 20.0,
            tool_type: 2,
            source: 0,
            counter_seed: 0,
        })
        .expect("saved V2 model accepts captured finite points");
        let rows: Vec<u8> = output
            .stamps
            .iter()
            .flat_map(|stamp| stamp.values.into_iter().flat_map(f32::to_le_bytes))
            .collect();
        let rows_sha = format!("{:x}", Sha256::digest(&rows));
        assert_eq!(rows_sha, identity["native_instance_sha256"]);
        assert_eq!(
            rows_sha,
            meta["shared_native_provenance"]["instance_stream_sha256"]
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(PATTERN)),
            meta["shared_native_provenance"]["pattern_r8"]["sha256"]
        );

        let base_projection =
            native_matrix_from_bits(&meta["shared_native_provenance"]["projection_base_f32_bits"]);
        let mut offset = 8;
        for (index, case) in cases.iter().enumerate() {
            let id = case["id"].as_str().unwrap();
            assert_eq!(usize::from(BIN[offset]), index, "{id} binary case index");
            offset += 1;
            let count = usize::from(read_u16_le(BIN, &mut offset));
            assert_eq!(
                count,
                case["changed_from_background_pixels"].as_u64().unwrap() as usize
            );
            assert_eq!(case["native_rgba_exact"], true);
            assert_eq!(
                case["source_size_f32_bits"],
                meta["shared_native_provenance"]["Drawing_to_RT_source_size_f32_bits"]
            );
            assert_eq!(
                case["rt_size_u32"],
                meta["shared_native_provenance"]["RT_size_u32"]
            );
            assert_eq!(case["instance_bytes_sha256"], rows_sha);
            assert_eq!(
                case["quad_vbo_sha256"],
                meta["shared_native_provenance"]["quad_upload_sha256"]
            );
            assert_eq!(
                case["texture_r8_sha256"],
                meta["shared_native_provenance"]["pattern_r8"]["sha256"]
            );
            assert_eq!(
                case["pattern_rgba8_upload_sha256"],
                meta["shared_native_provenance"]["pattern_r8"]["uploaded_rgba8_sha256"]
            );
            assert_eq!(
                case["vertex_shader_sha256"],
                meta["shared_native_provenance"]["shader_hashes"]["vertex"]
            );
            assert_eq!(
                case["fragment_shader_sha256"],
                meta["shared_native_provenance"]["shader_hashes"]["fragment"]
            );
            assert!(case["native_capture"]["sha256"].as_str().unwrap().len() == 64);
            assert_eq!(
                case["draw_matrix_source"],
                "explicit argument to actual native AirBrushPenStrokeDrawableRTV1::Draw(ISPSubBitmap*, Matrix4<float>&); no shader or vertex/instance buffer patch"
            );
            assert!(
                case["background_source"]
                    .as_str()
                    .unwrap()
                    .starts_with("host-cleared actual target framebuffer")
            );
            assert_eq!(case["effective_sampling_parameters"]["min_filter"], 9728);
            assert_eq!(case["effective_sampling_parameters"]["mag_filter"], 9728);
            assert_eq!(case["effective_sampling_parameters"]["wrap_s"], 33071);
            assert_eq!(case["effective_sampling_parameters"]["wrap_t"], 33071);
            assert_eq!(case["blend_contract"]["enabled"], true);
            assert_eq!(
                case["blend_contract"]["equation_separate"],
                serde_json::json!([32774, 32774])
            );
            assert_eq!(
                case["blend_contract"]["factors_separate"],
                serde_json::json!([770, 771, 1, 771])
            );

            let matrix = native_matrix_from_bits(&case["native_matrix_f32_bits"]);
            // Verify the captured native projection uniform equals its projection * DrawMatrix input.
            let mut projected = [0.0_f32; 16];
            for col in 0..4 {
                for row in 0..4 {
                    let mut value = 0.0_f64;
                    for k in 0..4 {
                        value += base_projection[k * 4 + row] as f64 * matrix[col * 4 + k] as f64;
                    }
                    projected[col * 4 + row] = value as f32;
                }
            }
            for (actual, expected) in projected.iter().zip(
                case["native_projection_uniform_f32_bits"]
                    .as_array()
                    .unwrap(),
            ) {
                assert_eq!(
                    actual.to_bits(),
                    u32::from_str_radix(expected.as_str().unwrap().trim_start_matches("0x"), 16)
                        .unwrap(),
                    "{id} projection uniform"
                );
            }
            let svg_matrix = native_draw_matrix_svg_conjugate(matrix);
            for (actual, expected) in svg_matrix
                .iter()
                .zip(case["svg_matrix_conjugate_f64"].as_array().unwrap())
            {
                assert_eq!(
                    *actual,
                    expected.as_f64().unwrap(),
                    "{id} GL-to-SVG transform"
                );
            }

            let bg: [u8; 4] = case["background_rgba8"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u8)
                .collect::<Vec<_>>()
                .try_into()
                .unwrap();
            let mut expected = bg.repeat(256 * 256);
            let mut previous = None;
            for _ in 0..count {
                let x = usize::from(read_u16_le(BIN, &mut offset));
                let y = usize::from(read_u16_le(BIN, &mut offset));
                assert!(x < 256 && y < 256);
                let pixel = y * 256 + x;
                assert!(previous.is_none_or(|last| last < pixel));
                previous = Some(pixel);
                let rgba = &BIN[offset..offset + 4];
                offset += 4;
                assert_ne!(rgba, bg.as_slice());
                expected[pixel * 4..pixel * 4 + 4].copy_from_slice(rgba);
            }
            assert_eq!(
                format!("{:x}", Sha256::digest(&expected)),
                case["native_frame_sha256"],
                "{id} full native frame hash"
            );
            assert_eq!(
                case["native_frame_sha256"],
                case["native_rgba_exact_resvg_sha256"]
            );

            let saved_argb = u32::from_str_radix(
                case["source_argb_u32"]
                    .as_str()
                    .unwrap()
                    .trim_start_matches("0x"),
                16,
            )
            .unwrap();
            let native_argb = u32::from_str_radix(
                case["native_argb_u32"]
                    .as_str()
                    .unwrap()
                    .trim_start_matches("0x"),
                16,
            )
            .unwrap();
            assert_eq!(
                saved_get_color(saved_argb),
                native_argb,
                "{id} native color getter"
            );
            let actual_color = [
                ((native_argb >> 16) & 255) as f32 / 255.0,
                ((native_argb >> 8) & 255) as f32 / 255.0,
                (native_argb & 255) as f32 / 255.0,
                ((native_argb >> 24) & 255) as f32 / 255.0,
            ];
            for (actual, expected) in actual_color
                .iter()
                .zip(case["shader_input_color_f32_bits"].as_array().unwrap())
            {
                assert_eq!(
                    actual.to_bits(),
                    u32::from_str_radix(expected.as_str().unwrap().trim_start_matches("0x"), 16)
                        .unwrap(),
                    "{id} inputColor uniform"
                );
            }

            let (data_url, pad_px) = native_prealpha_png(PATTERN, 3, saved_argb);
            let svg = native_texture_svg_with_draw_matrix(
                &output.stamps,
                3,
                &data_url,
                pad_px,
                matrix,
                bg,
            );
            let tree = resvg::usvg::Tree::from_str(&svg, &Default::default()).unwrap();
            let mut pixmap = resvg::tiny_skia::Pixmap::new(256, 256).unwrap();
            resvg::render(
                &tree,
                resvg::tiny_skia::Transform::identity(),
                &mut pixmap.as_mut(),
            );
            assert_eq!(pixmap.data(), expected, "{id} full RGBA");
        }
        assert_eq!(offset, BIN.len(), "matrix bundle has no trailing bytes");
    }

    #[cfg(feature = "render")]
    #[test]
    fn saved_v2_intrinsic_texture_paint_matches_compact_native_frames() {
        use sha2::{Digest, Sha256};

        const JSON: &str = include_str!("../../../../conformance/airbrush-paint-native.json");
        const BIN: &[u8] = include_bytes!("../../../../conformance/airbrush-paint-native.bin");
        const PATTERN: &[u8] = include_bytes!("../../../../conformance/airbrush-pattern.r8");

        let meta: Value = serde_json::from_str(JSON).unwrap();
        assert_eq!(meta["schema"], "sdocx.airbrush-saved-paint-oracles.v1");
        assert_eq!(format!("{:x}", Sha256::digest(BIN)), meta["binary_sha256"]);
        assert_eq!(
            format!("{:x}", Sha256::digest(PATTERN)),
            meta["pattern"]["sha256"]
        );
        assert_eq!(PATTERN.len(), 96 * 96);
        assert_eq!(&BIN[..4], b"ABP1");
        assert_eq!(u16::from_le_bytes(BIN[4..6].try_into().unwrap()), 1);
        let cases = meta["cases"].as_array().unwrap();
        assert_eq!(
            u16::from_le_bytes(BIN[6..8].try_into().unwrap()) as usize,
            cases.len()
        );

        let mut offset = 8;
        for (case_index, case) in cases.iter().enumerate() {
            let index = usize::from(BIN[offset]);
            offset += 1;
            let vector_bytes = usize::from(read_u16_le(BIN, &mut offset));
            let pixel_count = usize::from(read_u16_le(BIN, &mut offset));
            assert_eq!(index, case_index);
            assert_eq!(vector_bytes % 16, 0);
            let native_rows = &BIN[offset..offset + vector_bytes];
            offset += vector_bytes;
            assert_eq!(
                format!("{:x}", Sha256::digest(native_rows)),
                case["native_instance_sha256"]
            );

            let source = &case["source"];
            let points: Vec<P> = source["points_f32"]
                .as_array()
                .unwrap()
                .iter()
                .map(point)
                .collect();
            let history = &points[..points.len() - 1];
            let size = source["source_size_f32"].as_f64().unwrap() as f32;
            assert_eq!(source["motion_event_source"], 0);
            assert_eq!(source["counter_seed"], 0);
            let output = saved_redraw_v2(V2Input {
                history,
                endpoint: *points.last().unwrap(),
                initial_position: P { x: 0.0, y: 0.0 },
                size,
                tolerance: size / 20.0,
                tool_type: source["tool_type_raw"].as_i64().unwrap() as i32,
                source: source["motion_event_source"].as_i64().unwrap() as i32,
                counter_seed: source["counter_seed"].as_u64().unwrap() as u32,
            })
            .expect("saved V2 model accepts the captured finite inputs");
            let actual_rows: Vec<u8> = output
                .stamps
                .iter()
                .flat_map(|stamp| stamp.values.into_iter().flat_map(f32::to_le_bytes))
                .collect();
            assert_eq!(
                actual_rows, native_rows,
                "{} native AddPoint stream",
                case["id"]
            );
            assert_eq!(output.stamps.len(), vector_bytes / 16);

            let mut expected = vec![0_u8; 256 * 256 * 4];
            for _ in 0..pixel_count {
                let x = usize::from(read_u16_le(BIN, &mut offset));
                let y = usize::from(read_u16_le(BIN, &mut offset));
                let rgba = &BIN[offset..offset + 4];
                offset += 4;
                let dst = (y * 256 + x) * 4;
                expected[dst..dst + 4].copy_from_slice(rgba);
            }
            assert_eq!(
                format!("{:x}", Sha256::digest(&expected)),
                case["native_readback_top_down_sha256"]
            );

            let argb = u32::from_str_radix(
                source["saved_argb"]
                    .as_str()
                    .unwrap()
                    .trim_start_matches("0x"),
                16,
            )
            .unwrap();
            let rt_size = source["native_rt_size_u32"].as_u64().unwrap() as u32;
            assert_eq!(rt_size, size.trunc() as u32);
            let (data_url, pad_px) = native_prealpha_png(PATTERN, rt_size, argb);
            let svg = native_texture_svg(&output.stamps, rt_size, &data_url, pad_px);
            let tree = resvg::usvg::Tree::from_str(&svg, &Default::default()).unwrap();
            let mut pixmap = resvg::tiny_skia::Pixmap::new(256, 256).unwrap();
            resvg::render(
                &tree,
                resvg::tiny_skia::Transform::identity(),
                &mut pixmap.as_mut(),
            );
            assert_eq!(pixmap.data(), expected, "{} full RGBA", case["id"]);
        }
        assert_eq!(offset, BIN.len(), "compact paint bundle trailing bytes");
    }
}
