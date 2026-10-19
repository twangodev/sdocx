//! Bounded test-only model of the selected Oblique GLV1 saved sampler.
//!
//! This covers sixteen synthetic saved-caller witnesses with tool 0 or 2,
//! including curve-disabled, alternating terminal, and longer stress paths.
//! It does not establish other tool settings, arbitrary stroke histories, input
//! decoding, or renderer state.

use super::path::{P, Quad};

const MAX_SAMPLES: usize = 1_000_000;
const MAX_INPUT_POINTS: usize = 65_536;
const MIN_DELTA: f32 = 1.5;
const SHORT_PATH: f32 = 5.0;

fn repeat_count(size: f32, length: f32) -> Option<usize> {
    if !size.is_finite() || !length.is_finite() || size < 0.0 || length < 0.0 {
        return None;
    }
    let scaled = if size <= 5.0 {
        length * 0.5
    } else if size <= 10.0 {
        length / 3.0
    } else if size <= 15.0 {
        length * 0.25
    } else if size <= 20.0 {
        length / 5.0
    } else if size <= 30.0 {
        length * 0.125
    } else if size <= 40.0 {
        length / 7.0
    } else if size <= 55.0 {
        length / 9.0
    } else {
        length / 10.0
    };
    // Native converts the scaled value with FCVTZS into a signed W register.
    // Keep conversion overflow outside this bounded finite domain.
    if !scaled.is_finite() || scaled >= i32::MAX as f32 {
        return None;
    }
    let count = (scaled.trunc().max(2.0) as usize).checked_add(10)?;
    (count <= MAX_SAMPLES).then_some(count)
}

/// Return the `getPosTan` positions emitted by one native V1 redrawLine path.
/// The caller's endpoint `drawPoint` is separate and intentionally omitted.
fn quadratic_path_samples(
    start: P,
    control: P,
    end: P,
    size: f32,
    emission_budget: usize,
) -> Option<Vec<P>> {
    let path = Quad::new(start, control, end);
    if path.length == 0.0 || !path.length.is_finite() {
        return None;
    }
    let count = repeat_count(size, path.length)?;
    if count > emission_budget {
        return None;
    }
    let step = path.length / count as f32;
    let mut distance = 0.0_f32;
    let mut samples = Vec::with_capacity(count);
    for _ in 0..count {
        samples.push(path.at(distance));
        distance += step;
    }
    Some(samples)
}

fn quadratic_terminal_samples(
    start: P,
    control: P,
    end: P,
    size: f32,
    emission_budget: usize,
) -> Option<Vec<P>> {
    let path = Quad::new(start, control, end);
    if path.length == 0.0 || !path.length.is_finite() {
        return None;
    }
    let count = repeat_count(size, path.length)?;
    if count.checked_add(1)? > emission_budget {
        return None;
    }
    let step = path.length / count as f32;
    let mut distance = 0.0_f32;
    let mut samples = Vec::with_capacity(count + 1);
    for _ in 0..=count {
        samples.push(path.at(distance));
        distance += step;
    }
    Some(samples)
}

fn line_path_samples(start: P, end: P, size: f32, emission_budget: usize) -> Option<Vec<P>> {
    let length = end.sub(start).len();
    if length == 0.0 || !length.is_finite() {
        return None;
    }
    let count = repeat_count(size, length)?;
    if count > emission_budget {
        return None;
    }
    let step = length / count as f32;
    let scale = 32767.0_f32 * f32::from_bits(0x3800_00fd);
    let mut distance = 0.0_f32;
    let mut samples = Vec::with_capacity(count);
    for _ in 0..count {
        samples.push(start.lerp(end, distance * scale / length));
        distance += step;
    }
    Some(samples)
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
        let along = |a: P, b: P, t: f32| P {
            x: (b.x - a.x).mul_add(t, a.x),
            y: (b.y - a.y).mul_add(t, a.y),
        };
        let one = along(p[0], p[3], f32::from_bits(0x3eaa_aaab));
        let two = along(p[0], p[3], f32::from_bits(0x3f2a_aaab));
        let flat = (p[1].x - one.x).abs().max((p[1].y - one.y).abs()) <= 0.5
            && (p[2].x - two.x).abs().max((p[2].y - two.y).abs()) <= 0.5;
        if t1 - t0 >= 1024 && !flat {
            let a = p[0].mid(p[1]);
            let b = p[1].mid(p[2]);
            let c = p[2].mid(p[3]);
            let d = a.mid(b);
            let e = b.mid(c);
            let f = d.mid(e);
            let mid = (t0 + t1) / 2;
            self.split([p[0], a, d, f], t0, mid);
            self.split([f, e, c, p[3]], mid, t1);
        } else {
            let next = self.length + p[0].sub(p[3]).len();
            if next > self.length {
                self.length = next;
                self.segments.push((next, t1));
            }
        }
    }

    fn at(&self, distance: f32) -> Option<P> {
        let i = self.segments.partition_point(|&(end, _)| end < distance);
        let i = i.min(self.segments.len().checked_sub(1)?);
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
        Some(P {
            x: eval(self.p[0].x, self.p[1].x, self.p[2].x, self.p[3].x),
            y: eval(self.p[0].y, self.p[1].y, self.p[2].y, self.p[3].y),
        })
    }
}

fn cubic_terminal_samples(
    start: P,
    control: P,
    end: P,
    size: f32,
    emission_budget: usize,
) -> Option<Vec<P>> {
    // endPen's curve branch uses cubicTo(start, control, midpoint(control,end), end).
    let path = Cubic::new([start, control, control.mid(end), end]);
    if path.length == 0.0 || !path.length.is_finite() || path.segments.is_empty() {
        return None;
    }
    let count = repeat_count(size, path.length)?;
    if count.checked_add(1)? > emission_budget {
        return None;
    }
    let step = path.length / count as f32;
    let mut out = Vec::with_capacity(count + 1);
    let mut distance = 0.0_f32;
    for _ in 0..=count {
        out.push(path.at(distance)?);
        distance += step;
    }
    Some(out)
}

fn append_sampled_path(
    output: &mut Vec<(P, P)>,
    rt_previous: &mut P,
    samples: impl IntoIterator<Item = P>,
) -> Option<()> {
    for current in samples {
        if output.len() >= MAX_SAMPLES {
            return None;
        }
        output.push((*rt_previous, current));
        *rt_previous = current;
    }
    Some(())
}

fn append_direct_point(output: &mut Vec<(P, P)>, rt_previous: P, current: P) -> Option<()> {
    if output.len() >= MAX_SAMPLES {
        return None;
    }
    output.push((rt_previous, current));
    Some(())
}

struct CallbackStream {
    pairs: Vec<(P, P)>,
    redraw_line_ends: Vec<usize>,
    redraw_line_rt: Vec<P>,
    terminal_start: usize,
    terminal_end: usize,
    final_rt_previous: P,
}

/// Model the traced redrawLine/endPen state transitions for the narrow
/// selected V1 profile represented by the saved-caller and stress cases.
fn selected_v1_callback_stream(
    points: &[P],
    size: f32,
    curve_enabled: bool,
) -> Option<CallbackStream> {
    if points.len() < 2 || points.len() > MAX_INPUT_POINTS || !size.is_finite() || size <= 0.0 {
        return None;
    }
    let mut output = Vec::new();
    let mut redraw_line_ends = Vec::with_capacity(points.len() - 1);
    let mut redraw_line_rt = Vec::with_capacity(points.len() - 1);
    let mut rt_previous = *points.first()?;
    if !curve_enabled {
        for pair in points.windows(2) {
            let path_budget = MAX_SAMPLES.checked_sub(output.len())?.checked_sub(1)?;
            let samples = line_path_samples(pair[0], pair[1], size, path_budget)?;
            append_sampled_path(&mut output, &mut rt_previous, samples)?;
            // redrawLineNoCurve's direct endpoint draw is followed by an
            // explicit RT +344 update at 0x38f38–0x38f44.
            append_direct_point(&mut output, rt_previous, pair[1])?;
            rt_previous = pair[1];
            redraw_line_ends.push(output.len());
            redraw_line_rt.push(rt_previous);
        }
        let terminal_start = output.len();
        // The no-curve endPen line is zero length at the final source point;
        // getPosTan fails, then the direct endpoint draw updates RT +344.
        append_direct_point(&mut output, rt_previous, *points.last()?)?;
        let terminal_end = output.len();
        return (output.len() <= MAX_SAMPLES).then_some(CallbackStream {
            pairs: output,
            redraw_line_ends,
            redraw_line_rt,
            terminal_start,
            terminal_end,
            final_rt_previous: rt_previous,
        });
    }
    let mut previous = points[0];
    let mut start = points[0];
    let mut control = points[0];
    let mut needs_control = true;
    let mut alternate = true;
    let mut last_accepted = points[0];

    for &current in points.iter().skip(1) {
        let delta = P {
            x: (current.x - previous.x).abs(),
            y: (current.y - previous.y).abs(),
        };
        if delta.x < MIN_DELTA && delta.y < MIN_DELTA {
            // Native returns before updating +124 for this coordinate gate.
            redraw_line_ends.push(output.len());
            redraw_line_rt.push(rt_previous);
            continue;
        }
        let distance = current.sub(previous).len();
        if distance < SHORT_PATH {
            let was_alternate = alternate;
            alternate = !alternate;
            if was_alternate {
                // Native still updates +124 at the common return tail.
                last_accepted = current;
                previous = current;
                redraw_line_ends.push(output.len());
                redraw_line_rt.push(rt_previous);
                continue;
            }
        } else {
            alternate = true;
        }

        if needs_control {
            control = current;
            needs_control = false;
            previous = current;
            last_accepted = current;
            redraw_line_ends.push(output.len());
            redraw_line_rt.push(rt_previous);
            continue;
        }

        let endpoint = control.mid(current);
        let path_budget = MAX_SAMPLES.checked_sub(output.len())?.checked_sub(1)?;
        let path_samples = quadratic_path_samples(start, control, endpoint, size, path_budget)?;
        append_sampled_path(&mut output, &mut rt_previous, path_samples.iter().copied())?;
        // The saved V1 redraw loop passes the endpoint flag to each
        // redrawLine call. This direct endpoint AddPoint does not update RT
        // +344, which remains the last successful getPosTan sample.
        append_direct_point(&mut output, rt_previous, endpoint)?;
        // The next path starts at the last successful getPosTan position,
        // while the stored control advances to the raw current source point.
        if let Some(last) = path_samples.last() {
            start = *last;
        }
        control = current;
        previous = current;
        last_accepted = current;
        redraw_line_ends.push(output.len());
        redraw_line_rt.push(rt_previous);
    }

    let terminal_start = output.len();
    if needs_control {
        // endPen's one-point fallback calls drawPoint directly and leaves
        // RT +344 unchanged.
        append_direct_point(&mut output, rt_previous, last_accepted)?;
    } else if alternate {
        let terminal_budget = MAX_SAMPLES.checked_sub(output.len())?;
        append_sampled_path(
            &mut output,
            &mut rt_previous,
            cubic_terminal_samples(start, control, *points.last()?, size, terminal_budget)?,
        )?;
    } else {
        let terminal_budget = MAX_SAMPLES.checked_sub(output.len())?;
        append_sampled_path(
            &mut output,
            &mut rt_previous,
            quadratic_terminal_samples(start, control, *points.last()?, size, terminal_budget)?,
        )?;
    }
    (output.len() <= MAX_SAMPLES).then_some(CallbackStream {
        terminal_end: output.len(),
        pairs: output,
        redraw_line_ends,
        redraw_line_rt,
        terminal_start,
        final_rt_previous: rt_previous,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn bits(value: &Value) -> u32 {
        let hex = value.as_str().unwrap().trim_start_matches("0x");
        u32::from_le_bytes(std::array::from_fn(|i| {
            u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap()
        }))
    }

    fn word_bits(value: &Value) -> u32 {
        u32::from_str_radix(value.as_str().unwrap().trim_start_matches("0x"), 16).unwrap()
    }

    fn fixture_points(source: &Value) -> Vec<P> {
        source["points_f32_bits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|point| {
                let xy = point.as_array().unwrap();
                P {
                    x: f32::from_bits(bits(&xy[0])),
                    y: f32::from_bits(bits(&xy[1])),
                }
            })
            .collect()
    }

    fn oracle_cases() -> Vec<(String, Value)> {
        [
            include_str!("../../../../conformance/oblique-saved-caller-selected-native.json"),
            include_str!("../../../../conformance/oblique-v1-stress-native.json"),
        ]
        .into_iter()
        .flat_map(|json| {
            let fixture: Value = serde_json::from_str(json).unwrap();
            fixture["cases"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(name, case)| (name.clone(), case.clone()))
                .collect::<Vec<_>>()
        })
        .collect()
    }

    fn assert_vector_buffer(
        triangles: &[super::super::oblique::CoverageTriangle],
        case: &Value,
        phase: &str,
        case_name: &str,
    ) {
        let actual: Vec<u8> = triangles
            .iter()
            .flat_map(|triangle| {
                triangle.0.iter().flat_map(|vertex| {
                    [
                        vertex.position.x.to_bits().to_le_bytes(),
                        vertex.position.y.to_bits().to_le_bytes(),
                        vertex.coverage.to_bits().to_le_bytes(),
                    ]
                    .into_iter()
                    .flatten()
                })
            })
            .collect();
        let expected = &case["phase_final_vectors"][phase];
        let expected_len = expected["raw_byte_count"]
            .as_u64()
            .map(|count| count as usize)
            .or_else(|| expected["vector_bytes"].as_str().map(|hex| hex.len() / 2))
            .unwrap();
        assert_eq!(
            actual.len(),
            expected_len,
            "{case_name} {phase} full vector byte length"
        );
        if let Some(native_hex) = expected["vector_bytes"].as_str() {
            assert_eq!(
                native_hex.len(),
                actual.len() * 2,
                "{case_name} {phase} bytes"
            );
            for (index, value) in actual.iter().enumerate() {
                let offset = index * 2;
                let native = u8::from_str_radix(&native_hex[offset..offset + 2], 16).unwrap();
                assert_eq!(*value, native, "{case_name} {phase} byte {index}");
            }
        } else {
            use sha2::{Digest, Sha256};
            assert_eq!(
                format!("{:x}", Sha256::digest(&actual)),
                expected["sha256"].as_str().unwrap(),
                "{case_name} {phase} complete packed vector"
            );
        }
    }

    fn assert_native_transitions(stream: &CallbackStream, case: &Value, phase: &str, name: &str) {
        let Some(transitions) = case["transitions"].as_array() else {
            return;
        };
        let callbacks: Vec<_> = case["callback_add_point_calls"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|call| call["phase"] == phase)
            .collect();
        let vector_len_at = |callback_end: usize| -> u64 {
            if callback_end == 0 {
                0
            } else {
                callbacks[callback_end - 1]["vector_count_bytes"]
                    .as_u64()
                    .unwrap()
            }
        };
        let line_events: Vec<_> = transitions
            .iter()
            .filter(|event| {
                event["phase"] == phase
                    && matches!(
                        event["method"].as_str(),
                        Some("redrawLine" | "redrawLineNoCurve")
                    )
            })
            .collect();
        assert_eq!(
            line_events.len(),
            stream.redraw_line_ends.len(),
            "{name} {phase} redraw return count"
        );
        let mut previous_end = 0;
        for (index, (event, (&end, rt))) in line_events
            .iter()
            .zip(stream.redraw_line_ends.iter().zip(&stream.redraw_line_rt))
            .enumerate()
        {
            assert_eq!(
                event["callback_index_start"].as_u64().unwrap() as usize,
                previous_end,
                "{name} {phase} redraw {index} start"
            );
            assert_eq!(
                event["callback_index_end"].as_u64().unwrap() as usize,
                end,
                "{name} {phase} redraw {index} end"
            );
            assert_eq!(
                event["vector_count_before_bytes"].as_u64().unwrap(),
                vector_len_at(previous_end),
                "{name} {phase} redraw {index} vector before"
            );
            assert_eq!(
                event["vector_count_after_bytes"].as_u64().unwrap(),
                vector_len_at(end),
                "{name} {phase} redraw {index} vector after"
            );
            let bytes = event["rt_after_344_360"].as_str().unwrap();
            let native_x = bits(&Value::String(format!("0x{}", &bytes[0..8])));
            let native_y = bits(&Value::String(format!("0x{}", &bytes[8..16])));
            assert_eq!(
                rt.x.to_bits(),
                native_x,
                "{name} {phase} redraw {index} RT x"
            );
            assert_eq!(
                rt.y.to_bits(),
                native_y,
                "{name} {phase} redraw {index} RT y"
            );
            previous_end = end;
        }

        let end_pen = transitions
            .iter()
            .find(|event| event["phase"] == phase && event["method"] == "endPen")
            .unwrap_or_else(|| panic!("{name} {phase} missing endPen boundary"));
        assert_eq!(
            end_pen["callback_index_start"].as_u64().unwrap() as usize,
            stream.terminal_start,
            "{name} {phase} terminal start"
        );
        assert_eq!(
            end_pen["callback_index_end"].as_u64().unwrap() as usize,
            stream.terminal_end,
            "{name} {phase} terminal end"
        );
        assert_eq!(
            end_pen["vector_count_before_bytes"].as_u64().unwrap(),
            vector_len_at(stream.terminal_start),
            "{name} {phase} terminal vector before"
        );
        assert_eq!(
            end_pen["vector_count_after_bytes"].as_u64().unwrap(),
            vector_len_at(stream.terminal_end),
            "{name} {phase} terminal vector after"
        );
        for rt_bytes in [
            end_pen["rt_after_344_360"].as_str().unwrap(),
            case["phase_final_vectors"][phase]["rt_344_360"]
                .as_str()
                .unwrap(),
        ] {
            assert_eq!(
                bits(&Value::String(format!("0x{}", &rt_bytes[0..8]))),
                stream.final_rt_previous.x.to_bits(),
                "{name} {phase} terminal RT x"
            );
            assert_eq!(
                bits(&Value::String(format!("0x{}", &rt_bytes[8..16]))),
                stream.final_rt_previous.y.to_bits(),
                "{name} {phase} terminal RT y"
            );
        }
    }

    #[test]
    fn native_v1_callback_streams_match_selected_saved_caller_cases() {
        for (case_name, case) in oracle_cases() {
            let Some(native_callbacks) = case["callback_add_point_calls"].as_array() else {
                continue;
            };
            let source = &case["input"];
            assert_eq!(source["advanced_setting"], Value::Null, "{case_name}");
            let points = fixture_points(source);
            let size = f32::from_bits(bits(&source["pen_size_f32_bits"]));
            let stream = selected_v1_callback_stream(
                &points,
                size,
                source["curve_enabled"].as_bool().unwrap(),
            )
            .unwrap_or_else(|| panic!("unsupported saved V1 case {case_name}"));
            assert_eq!(
                stream.redraw_line_ends.len(),
                points.len() - 1,
                "{case_name}"
            );
            assert!(
                stream
                    .redraw_line_ends
                    .windows(2)
                    .all(|ends| ends[0] <= ends[1]),
                "{case_name} ordered redraw boundaries"
            );
            assert!(stream.terminal_start <= stream.terminal_end, "{case_name}");
            assert_eq!(stream.terminal_end, stream.pairs.len(), "{case_name}");
            assert!(
                stream.final_rt_previous.x.is_finite() && stream.final_rt_previous.y.is_finite()
            );
            for phase in ["draw_object_stroke", "saved_redraw"] {
                assert_native_transitions(&stream, &case, phase, &case_name);
                let native: Vec<_> = native_callbacks
                    .iter()
                    .filter(|call| call["phase"] == phase)
                    .collect();
                assert_eq!(
                    native.len(),
                    stream.pairs.len(),
                    "{case_name} {phase} count"
                );
                for (index, ((_, actual), expected)) in stream.pairs.iter().zip(native).enumerate()
                {
                    assert_eq!(
                        actual.x.to_bits(),
                        word_bits(&expected["xy_bits"][0]),
                        "{case_name} {phase} callback {index} x"
                    );
                    assert_eq!(
                        actual.y.to_bits(),
                        word_bits(&expected["xy_bits"][1]),
                        "{case_name} {phase} callback {index} y"
                    );
                }
            }
        }
    }

    #[test]
    fn native_v1_callback_streams_match_rtv1_geometry_buffers() {
        for (case_name, case) in oracle_cases() {
            let Some(native_callbacks) = case["callback_add_point_calls"].as_array() else {
                continue;
            };
            let source = &case["input"];
            let points = fixture_points(source);
            let stream = selected_v1_callback_stream(
                &points,
                f32::from_bits(bits(&source["pen_size_f32_bits"])),
                source["curve_enabled"].as_bool().unwrap(),
            )
            .unwrap();
            for phase in ["draw_object_stroke", "saved_redraw"] {
                assert_native_transitions(&stream, &case, phase, &case_name);
                let callbacks: Vec<_> = native_callbacks
                    .iter()
                    .filter(|call| call["phase"] == phase)
                    .collect();
                assert_eq!(callbacks.len(), stream.pairs.len(), "{case_name} {phase}");
                let first = callbacks.first().unwrap();
                let size = first["stored_size_u32"].as_u64().unwrap() as u32;
                let runtime = super::super::oblique::ObliqueRuntime {
                    k: f32::from_bits(bits(&first["k_bits"])),
                    inverse_scale: [
                        f32::from_bits(bits(&first["inverse_scale_bits"][0])),
                        f32::from_bits(bits(&first["inverse_scale_bits"][1])),
                    ],
                };

                for (index, ((modeled_previous, sample), callback)) in
                    stream.pairs.iter().zip(&callbacks).enumerate()
                {
                    let prior = &callback["previous_xy_bits"];
                    assert_eq!(
                        modeled_previous.x.to_bits(),
                        bits(&prior[0]),
                        "{case_name} {phase} callback {index} modeled previous x"
                    );
                    assert_eq!(
                        modeled_previous.y.to_bits(),
                        bits(&prior[1]),
                        "{case_name} {phase} callback {index} modeled previous y"
                    );
                    assert_eq!(
                        sample.x.to_bits(),
                        word_bits(&callback["xy_bits"][0]),
                        "{case_name} {phase} callback {index} current x"
                    );
                    assert_eq!(
                        sample.y.to_bits(),
                        word_bits(&callback["xy_bits"][1]),
                        "{case_name} {phase} callback {index} current y"
                    );
                    assert_eq!(
                        callback["stored_size_u32"].as_u64().unwrap() as u32,
                        size,
                        "{case_name} {phase} callback {index} stored size"
                    );
                    assert_eq!(
                        bits(&callback["k_bits"]),
                        runtime.k.to_bits(),
                        "{case_name} {phase} callback {index} k"
                    );
                    for axis in 0..2 {
                        assert_eq!(
                            bits(&callback["inverse_scale_bits"][axis]),
                            runtime.inverse_scale[axis].to_bits(),
                            "{case_name} {phase} callback {index} inverse scale {axis}"
                        );
                    }
                }

                let triangles =
                    super::super::oblique::prepare_addpoint_stream(&stream.pairs, size, runtime)
                        .unwrap();
                assert_vector_buffer(&triangles, &case, phase, &case_name);
            }
        }
    }

    #[test]
    fn rtv1_geometry_kernel_matches_captured_native_callback_arguments() {
        for (case_name, case) in oracle_cases() {
            let Some(native_callbacks) = case["callback_add_point_calls"].as_array() else {
                continue;
            };
            for phase in ["draw_object_stroke", "saved_redraw"] {
                let callbacks: Vec<_> = native_callbacks
                    .iter()
                    .filter(|call| call["phase"] == phase)
                    .collect();
                let first = callbacks.first().unwrap();
                let runtime = super::super::oblique::ObliqueRuntime {
                    k: f32::from_bits(bits(&first["k_bits"])),
                    inverse_scale: [
                        f32::from_bits(bits(&first["inverse_scale_bits"][0])),
                        f32::from_bits(bits(&first["inverse_scale_bits"][1])),
                    ],
                };
                let size = first["stored_size_u32"].as_u64().unwrap() as u32;
                let pairs: Vec<_> = callbacks
                    .iter()
                    .map(|call| {
                        let previous = &call["previous_xy_bits"];
                        let current = &call["xy_bits"];
                        (
                            P {
                                x: f32::from_bits(bits(&previous[0])),
                                y: f32::from_bits(bits(&previous[1])),
                            },
                            P {
                                x: f32::from_bits(word_bits(&current[0])),
                                y: f32::from_bits(word_bits(&current[1])),
                            },
                        )
                    })
                    .collect();
                let triangles =
                    super::super::oblique::prepare_addpoint_stream(&pairs, size, runtime).unwrap();
                assert_vector_buffer(&triangles, &case, phase, &case_name);
            }
        }
    }

    #[test]
    fn repeat_count_follows_v1_native_width_bands() {
        // Branch thresholds and divisors come from V1 getRepeat at 0x39ebc.
        // The lengths here make each side of every width band observable.
        for (size, expected) in [
            (5.0, 70),
            (5.001, 50),
            (10.0, 50),
            (10.001, 40),
            (15.0, 40),
            (15.001, 34),
            (20.0, 34),
            (20.001, 25),
            (30.0, 25),
            (30.001, 27),
            (40.0, 27),
            (40.001, 23),
            (55.0, 23),
            (55.001, 22),
        ] {
            assert_eq!(repeat_count(size, 120.0).unwrap(), expected, "size {size}");
        }
        assert_eq!(repeat_count(4.0, 12.704_784).unwrap(), 16);
        assert_eq!(repeat_count(1.0, 20.0).unwrap(), 20);
        assert_eq!(repeat_count(56.0, 100.0).unwrap(), 20);
        assert_eq!(repeat_count(f32::NAN, 1.0), None);
    }

    #[test]
    fn terminal_cubic_matches_shared_native_smpath_fixture() {
        let fixture: Value =
            serde_json::from_str(include_str!("../../../../conformance/smpath-cubic.json"))
                .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let controls = case["native"]["controls_f32"].as_array().unwrap();
            let point = |index: usize| P {
                x: controls[index * 2].as_f64().unwrap() as f32,
                y: controls[index * 2 + 1].as_f64().unwrap() as f32,
            };
            let path = Cubic::new([point(0), point(1), point(2), point(3)]);
            assert_eq!(
                path.length.to_bits(),
                word_bits(&case["native"]["length_bits"]),
                "{} cubic length",
                case["name"]
            );
            for sample in case["native"]["samples"].as_array().unwrap() {
                let position = path
                    .at(f32::from_bits(word_bits(&sample["distance_bits"])))
                    .unwrap();
                let expected = &sample["position_bits"];
                assert_eq!(
                    position.x.to_bits(),
                    word_bits(&expected[0]),
                    "{} cubic x at {}",
                    case["name"],
                    sample["fraction"]
                );
                assert_eq!(
                    position.y.to_bits(),
                    word_bits(&expected[1]),
                    "{} cubic y at {}",
                    case["name"],
                    sample["fraction"]
                );
            }
        }
    }
}
