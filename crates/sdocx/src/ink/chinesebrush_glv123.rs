//! Bounded source-driven numeric model for ChineseBrush GLV1/2/3 calls.
//!
//! Test evidence for pinned synthetic two- to eight-event Drawing fixtures.
//! Saved-note parsing, renderer state and production admission remain separate.

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

// Keep the native sampler's explicit path, repeat, width, and index state.
#[allow(clippy::too_many_arguments)]
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
    for (i, point) in points.iter().enumerate().skip(1) {
        let d = point.sub(previous);
        if d.x.abs() < gate && d.y.abs() < gate {
            continue;
        }
        accepted.push(i);
        previous = *point;
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
) -> Vec<[f32; 5]> {
    let points: Vec<P> = xy.iter().map(|p| P { x: p[0], y: p[1] }).collect();
    assert_eq!(points.len(), pressure.len());
    assert_eq!(points.len(), times.len());
    if !curve {
        return model_line(&points, pressure, size, version, width, height, tool);
    }
    let gate = if version <= 2 {
        4.0
    } else {
        width.min(height) as f32 / 360.0
    };
    let mut accepted = Vec::new();
    let mut previous = 0usize;
    let mut speeds = vec![0.0_f32; points.len()];
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
        return vec![[
            points[0].x,
            points[0].y,
            size * 0.25,
            0.0,
            (50.0 + p * 100.0) as i32 as f32,
        ]];
    }
    let seed = accepted[0];
    let seed_pressure = effective_pressure(pressure[seed], tool);
    let mut prior = size * seed_pressure;
    let mut index = f32_trunc(20.0 + seed_pressure * 100.0);
    let mut phase = 0;
    let mut out = Vec::new();
    let mut start = points[0];
    let mut j = 0;
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
        emit_path(
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
        );
        start = endpoint;
        prior = target;
        index = target_index;
        j += 2;
    }
    let m = accepted.len();
    let (path, extra) = if m % 2 == 0 {
        let a = accepted[m - 2];
        let b = accepted[m - 1];
        (
            Path::Cubic([start, points[a], points[a].mid(points[b]), points[b]]),
            1,
        )
    } else {
        let a = accepted[m - 1];
        (Path::Quad(start, points[a], points[a]), 1)
    };
    let event = accepted[m - 1];
    let p = effective_pressure(pressure[event], tool);
    let terminal_speed = if version == 1 {
        if m <= 2 {
            1.0
        } else {
            points[event].sub(points[accepted[m - 2]]).len()
        }
    } else {
        speeds[event]
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
    emit_path(
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
    );
    out
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
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

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
}
