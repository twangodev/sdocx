//! Test-only numeric reconstruction of Samsung Notes AirBrush V1 saved redraw.
//!
//! This module is intentionally unregistered until the profile's saved
//! settings and rendering contract are admitted by the caller.
use super::path::{P, Quad};

const MAX_POINTS: usize = 65_536;
const MAX_STAMPS: usize = 1_000_000;

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
    let sin = (radians as f64).sin() as f32;
    let cos = (radians as f64).cos() as f32;
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
}
