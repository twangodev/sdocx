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

    fn little_endian_f32(hex: &str) -> f32 {
        let bytes: Vec<u8> = hex
            .as_bytes()
            .chunks_exact(2)
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
                .chunks_exact(2)
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect();
            let points: Vec<P> = bytes
                .chunks_exact(8)
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
                        .chunks_exact(2)
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
