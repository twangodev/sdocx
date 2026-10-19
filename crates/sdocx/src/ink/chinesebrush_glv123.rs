//! Bounded source-driven numeric model for ChineseBrush GLV1/2/3 calls.
//!
//! This module is test evidence only. It covers the pinned synthetic three- and
//! five-event Drawing fixtures, not saved-note parsing, renderer state, or pen
//! admission.

use super::path::{P, Quad};

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
    (scaled as usize).max(2) + 1
}

fn dab_size(pressure: f32, factor: f32, size: f32, prior: f32) -> f32 {
    if factor < 0.35 {
        (pressure * size).mul_add(1.0, prior) * 0.5
    } else if factor < 0.7 {
        let d = pressure * 1.5 + factor * 0.5;
        let d = d * 0.5;
        let d = d * size + prior;
        d * 0.5
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

fn factor(speed: f32) -> f32 {
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

fn event_speed(points: &[P], times: &[i32], event: usize, width: i32, height: i32) -> f32 {
    let dt = times[event] - times[event - 1];
    let dt = if times[event - 1] == 0 || dt <= 0 {
        50000.0
    } else {
        dt as f32
    };
    let scale = 1440.0 / width.min(height) as f32;
    (points[event].sub(points[event - 1]).len() * 10000.0 / dt) * scale
}

fn model_three(
    points: &[P],
    pressure: &[f32],
    times: &[i32],
    size: f32,
    version: i32,
    width: i32,
    height: i32,
    tool: i32,
    curve: bool,
) -> Vec<[f32; 5]> {
    assert_eq!(points.len(), 3);
    let gate = if version == 1 || version == 2 {
        4.0
    } else {
        width.min(height) as f32 / 360.0
    };
    let mut accepted = Vec::new();
    let mut previous = points[0];
    let mut previous_time = times[0];
    let mut speed = 0.0;
    for i in 1..points.len() {
        let delta = points[i].sub(previous);
        if delta.x.abs() < gate && delta.y.abs() < gate {
            continue;
        }
        if version != 1 {
            let elapsed = times[i] - previous_time;
            let elapsed = if previous_time == 0 || elapsed <= 0 {
                50000.0
            } else {
                elapsed as f32
            };
            speed = (delta.len() * 10000.0 / elapsed) * (1440.0 / width.min(height) as f32);
        }
        accepted.push(i);
        previous = points[i];
        previous_time = times[i];
    }

    if version == 1 {
        let p = effective_pressure(*pressure.last().unwrap(), tool);
        if accepted.is_empty() {
            return vec![[
                points[0].x,
                points[0].y,
                size * 0.25,
                0.0,
                (50.0 + p * 100.0) as i32 as f32,
            ]];
        }
        // GLV1's first admitted drawLine call initializes the drawable width
        // and scalar from that event. Later accepted events update the path
        // and terminal target, but do not replace this seed before repeat
        // selection (native width bands are selected from the seeded width).
        let seed_pressure = effective_pressure(pressure[accepted[0]], tool);
        let prior = size * seed_pressure;
        let index = f32_trunc(20.0 + seed_pressure * 100.0);
        let target = if curve {
            dab_size(p, 1.0, size, prior)
        } else {
            size * p
        };
        let final_width = target.max(size / 10.0);
        let path = if curve {
            let seed = accepted[0];
            let end = *accepted.last().unwrap();
            if accepted.len() == 1 {
                Path::Quad(points[0], points[end], points[end])
            } else {
                Path::Cubic([
                    points[0],
                    points[seed],
                    points[seed].mid(points[end]),
                    points[end],
                ])
            }
        } else {
            Path::Line(points[0], points[2])
        };
        let repeat = repeat_index(path.length(), prior.min(target) as i32);
        let start_index = if curve {
            index
        } else {
            f32_trunc(100.0 + p * 100.0)
        };
        let target_index = f32_trunc((if curve { 50.0 } else { 100.0 }) + p * 100.0);
        let mut out = Vec::new();
        let mut phase = 0;
        emit_path(
            &mut out,
            path,
            repeat,
            usize::from(curve),
            prior,
            final_width,
            repeat,
            start_index,
            target_index,
            repeat,
            &mut phase,
        );
        return out;
    }

    if !curve {
        return model_line(points, pressure, size, version, width, height, tool);
    }
    if accepted.is_empty() {
        let p = effective_pressure(*pressure.last().unwrap(), tool);
        let index = f32_trunc(50.0 + p * 100.0) as f32;
        return vec![[points[0].x, points[0].y, size * 0.25, 0.0, index]];
    }

    let seed = accepted[0];
    let seed_pressure = effective_pressure(pressure[seed], tool);
    let prior = size * seed_pressure;
    let index = f32_trunc(20.0 + seed_pressure * 100.0);
    let p = effective_pressure(*pressure.last().unwrap(), tool);
    let target = calculated_width(dab_size(p, factor(speed), size, prior), prior, size);
    let path = if seed == 1 {
        Path::Cubic([points[0], points[1], points[1].mid(points[2]), points[2]])
    } else {
        Path::Quad(points[0], points[2], points[2])
    };
    let repeat = repeat_index(path.length(), prior.min(target) as i32);
    let target_index = f32_trunc(50.0 + p * 100.0);
    let mut phase = 0;
    let mut out = Vec::new();
    emit_path(
        &mut out,
        path,
        repeat,
        1,
        prior,
        target,
        repeat,
        index,
        target_index,
        repeat,
        &mut phase,
    );
    out
}

fn effective_pressure(pressure: f32, tool: i32) -> f32 {
    if tool == 1 { 0.5 } else { pressure.min(1.0) }
}

fn packed_w(phase: f32, scalar: f32) -> f32 {
    (phase as f64).mul_add(1.0, scalar as f64 * 0.001) as f32
}

fn emit_path(
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
) {
    let length = path.length();
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
}

fn model_curve(
    points: &[P],
    pressure: &[f32],
    times: &[i32],
    size: f32,
    version: i32,
    width: i32,
    height: i32,
    tool: i32,
) -> Vec<[f32; 5]> {
    if points.len() == 3 {
        return model_three(
            points, pressure, times, size, version, width, height, tool, true,
        );
    }
    assert_eq!(points.len(), 5);
    let mut prior = size * effective_pressure(pressure[1], 2);
    let mut index = f32_trunc(20.0 + effective_pressure(pressure[1], 2) * 100.0);
    let mut phase = 0;
    let mut out = Vec::new();
    let middle = points[1].mid(points[3]);
    let factor1 = if version == 1 {
        let d = points[2].sub(points[3]).len();
        factor(d)
    } else {
        factor(event_speed(points, times, 3, width, height))
    };
    let target1 = calculated_width(
        dab_size(effective_pressure(pressure[3], 2), factor1, size, prior),
        prior,
        size,
    );
    let first = Path::Quad(points[0], points[1], middle);
    let repeat1 = repeat_index(first.length(), prior.min(target1) as i32);
    let index1 = f32_trunc(100.0 + effective_pressure(pressure[3], 2) * 100.0);
    emit_path(
        &mut out, first, repeat1, 0, prior, target1, repeat1, index, index1, repeat1, &mut phase,
    );
    prior = target1;
    index = index1;

    let factor2 = if version == 1 {
        factor(points[3].sub(points[4]).len())
    } else {
        factor(event_speed(points, times, 4, width, height))
    };
    let target2 = calculated_width(
        dab_size(effective_pressure(pressure[4], 2), factor2, size, prior),
        prior,
        size,
    );
    let cubic = Path::Cubic([middle, points[3], points[3].mid(points[4]), points[4]]);
    let repeat2 = repeat_index(cubic.length(), prior.min(target2) as i32);
    let index2 = f32_trunc(50.0 + effective_pressure(pressure[4], 2) * 100.0);
    emit_path(
        &mut out, cubic, repeat2, 1, prior, target2, repeat2, index, index2, repeat2, &mut phase,
    );
    out
}

fn model_line(
    points: &[P],
    pressure: &[f32],
    size: f32,
    version: i32,
    width: i32,
    height: i32,
    tool: i32,
) -> Vec<[f32; 5]> {
    let gate = if version == 1 || version == 2 {
        4.0
    } else {
        width.min(height) as f32 / 360.0
    };
    let mut accepted = Vec::new();
    let mut previous = points[0];
    for i in 1..points.len() {
        let d = points[i].sub(previous);
        if d.x.abs() < gate && d.y.abs() < gate {
            continue;
        }
        accepted.push(i);
        previous = points[i];
    }
    if accepted.is_empty() {
        let p = effective_pressure(*pressure.last().unwrap(), tool);
        return vec![[
            points[0].x,
            points[0].y,
            size * 0.25,
            0.0,
            (50.0 + p * 100.0) as i32 as f32,
        ]];
    }
    let seed = accepted[0];
    let scalar = f32_trunc(100.0 + effective_pressure(pressure[seed], tool) * 100.0) as f32;
    let scalar_index = scalar as i32;
    let mut width_state = size * effective_pressure(pressure[seed], tool);
    let mut start = points[0];
    let mut phase = 0;
    let mut out = Vec::new();
    for (event_index, &event) in accepted.iter().enumerate() {
        let target = if event_index == 0 {
            width_state
        } else {
            size * effective_pressure(pressure[event], tool)
        };
        let path = Path::Line(start, points[event]);
        let length = path.length();
        let repeat = repeat_index(length, width_state.min(target) as i32);
        emit_path(
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
        );
        width_state = target;
        start = points[event];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn words(hex: &str) -> Vec<u32> {
        hex.as_bytes()
            .chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect::<Vec<_>>()
            .chunks_exact(4)
            .map(|word| u32::from_le_bytes(word.try_into().unwrap()))
            .collect()
    }

    fn source_arrays(input: &Value) -> (Vec<P>, Vec<f32>, Vec<i32>) {
        let arrays = &input["source_arrays"];
        let point_words = words(arrays["points"]["data_hex"].as_str().unwrap());
        let points = point_words
            .chunks_exact(2)
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
            .chunks_exact(8)
            .map(|chunk| {
                let bytes: Vec<u8> = chunk
                    .chunks_exact(2)
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
            .chunks_exact(4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
            .collect()
    }

    fn hex_bytes(hex: &str) -> Vec<u8> {
        hex.as_bytes()
            .chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect()
    }

    #[test]
    fn source_model_matches_native_glv1_glv2_glv3_full_vectors() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/chinesebrush-source-native.json"
        ))
        .unwrap();
        assert_eq!(fixture["schema"], "chinesebrush-source-native-v1");
        assert_eq!(fixture["cases"].as_object().unwrap().len(), 44);
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
            let predicted = if input["curve_enabled"].as_bool().unwrap() {
                model_curve(
                    &points,
                    &pressure,
                    &times,
                    size,
                    version,
                    w,
                    h,
                    input["tool_type"].as_i64().unwrap() as i32,
                )
            } else {
                model_line(
                    &points,
                    &pressure,
                    size,
                    version,
                    w,
                    h,
                    input["tool_type"].as_i64().unwrap() as i32,
                )
            };
            let rows = case["producer"]["append_rows"].as_array().unwrap();
            assert_eq!(predicted.len(), rows.len(), "{name}");
            for (i, (point, row)) in predicted.iter().zip(rows).enumerate() {
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
    }
}
