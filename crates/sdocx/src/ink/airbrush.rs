//! Test-only numeric reconstruction of Samsung Notes AirBrush V1 saved redraw.
//!
//! This module is intentionally unregistered until the profile's saved
//! settings and rendering contract are admitted by the caller.
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

fn sample_quad(
    out: &mut Output,
    start: P,
    control: P,
    end: P,
    spacing: f32,
    include_end: bool,
    counter: &mut u32,
) -> Option<()> {
    if out.stamps.len() >= MAX_STAMPS
        || !spacing.is_finite()
        || spacing <= 0.0
        || ![start.x, start.y, control.x, control.y, end.x, end.y]
            .into_iter()
            .all(f32::is_finite)
    {
        return None;
    }
    let path = Quad::new(start, control, end);
    if path.length == 0.0 {
        if include_end {
            push_stamp(out, end, counter)?;
        }
        return Some(());
    }
    let repeats = (path.length / spacing).trunc();
    if !repeats.is_finite() || repeats < 0.0 || repeats as usize > MAX_STAMPS - out.stamps.len() {
        return None;
    }
    let count = repeats as usize;
    let step = if count == 0 {
        0.0
    } else {
        path.length / count as f32
    };
    let n = count + usize::from(include_end && count > 0);
    let mut distance = 0.0_f32;
    for i in 0..n {
        if i < count {
            push_stamp(out, path.at(distance), counter)?;
            distance += step;
        } else {
            push_stamp(out, end, counter)?;
        }
    }
    Some(())
}

fn push_stamp(out: &mut Output, p: P, counter: &mut u32) -> Option<()> {
    if out.stamps.len() >= MAX_STAMPS {
        return None;
    }
    let degrees = (*counter as f32) * 137.0;
    let radians = ((degrees as f64 * std::f64::consts::PI) / 180.0) as f32;
    // The conformance runner's explicit sincosf host boundary evaluates the
    // angle with Python's binary64 libm and rounds each result to f32.
    let sin = libm::sin(radians as f64) as f32;
    let cos = libm::cos(radians as f64) as f32;
    out.stamps.push(Stamp {
        values: [p.x, p.y, sin, cos],
    });
    *counter = counter.checked_add(1)?;
    Some(())
}

/// Run the observed V1 saved redraw lifecycle for explicit finite event data.
/// The native call emits one quadratic to each history midpoint, followed by
/// a terminal quadratic to the current event. The captured numeric profile
/// is independent of tool type at this sampling stage.
pub(super) fn saved_redraw(input: Input<'_>) -> Option<Output> {
    let Input {
        history,
        endpoint,
        size,
        runtime,
    } = input;
    if history.is_empty() || history.len() > MAX_POINTS || !size.is_finite() || size <= 0.0 {
        return None;
    }
    let spacing = ((size.trunc() * 2.0) / 20.0).max(f32::MIN_POSITIVE);
    let mut output = Output::default();
    let mut counter = runtime.counter_seed;
    let mut previous_raw = history[0];
    // RedrawPen seeds the GL drawable's active midpoint from history[0]; the
    // separately supplied runtime previous_position is consumed by its
    // movement/tolerance gate, not as this path's first moveTo coordinate.
    let mut previous_mid = history[0];
    for &current in history.iter().skip(1) {
        let mid = midpoint(previous_raw, current);
        sample_quad(
            &mut output,
            previous_mid,
            previous_raw,
            mid,
            spacing,
            false,
            &mut counter,
        )?;
        output.history_ends.push(output.stamps.len());
        previous_raw = current;
        previous_mid = mid;
    }
    let start = output.stamps.len();
    sample_quad(
        &mut output,
        previous_mid,
        previous_raw,
        endpoint,
        spacing,
        false,
        &mut counter,
    )?;
    output.terminal = start..output.stamps.len();
    Some(output)
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

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
}
