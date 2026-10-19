//! Test-only DefaultPen saved-redraw sampler reconstructed from native RT AddPoint.
//!
//! The exercised contract is synthetic saved ObjectStroke replay. This module
//! does not define app admission, paint, or GPU rendering.
use super::path::{P, Quad};
use crate::Point;

fn repeat(length: f32, width: i32) -> usize {
    let scaled = if width <= 5 {
        length * 0.5
    } else if width <= 10 {
        length / 3.0
    } else if width <= 15 {
        length * 0.25
    } else if width <= 20 {
        length / 5.0
    } else if width <= 30 {
        length / 6.0
    } else if width <= 40 {
        length / 7.0
    } else if width <= 55 {
        length * 0.125
    } else {
        length / 10.0
    };
    (scaled.trunc().max(2.0) as usize) + 1
}

struct Cubic {
    points: [P; 4],
    segments: Vec<(f32, u16)>,
    length: f32,
}
impl Cubic {
    fn new(points: [P; 4]) -> Self {
        let mut path = Self {
            points,
            segments: Vec::new(),
            length: 0.0,
        };
        path.split(points, 0, 32767);
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
    fn at(&self, distance: f32) -> P {
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
        P {
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

const MAX_SAMPLES_PER_PATH: usize = 1_000_000;

fn count(length: f32, size: f32) -> Option<usize> {
    if !length.is_finite() || length < 0.0 {
        return None;
    }
    let count = repeat(length, size.trunc() as i32);
    (count <= MAX_SAMPLES_PER_PATH).then_some(count)
}
fn emit_cubic(out: &mut Vec<[f32; 3]>, points: [P; 4], size: f32) -> Option<()> {
    let path = Cubic::new(points);
    if path.length == 0.0 {
        return Some(()); // Native getPosTan fails for an empty path.
    }
    let count = count(path.length, size)?;
    let step = path.length / count as f32;
    let radius = (size * 0.5).max(1.0);
    let mut distance = 0.0_f32;
    for _ in 0..count {
        let p = path.at(distance);
        out.push([p.x, p.y, radius + 0.5]);
        distance += step;
    }
    Some(())
}

fn emit_quad(out: &mut Vec<[f32; 3]>, start: P, control: P, end: P, size: f32) -> Option<()> {
    let path = Quad::new(start, control, end);
    if path.length == 0.0 {
        return Some(()); // Native getPosTan fails for an empty path.
    }
    let count = count(path.length, size)?;
    let step = path.length / count as f32;
    let radius = (size * 0.5).max(1.0);
    let mut distance = 0.0_f32;
    for _ in 0..count {
        let p = path.at(distance);
        // DefaultPen RT stores the AddPoint radius with its native +0.5 bias.
        out.push([p.x, p.y, radius + 0.5]);
        distance += step;
    }
    Some(())
}

/// Reconstructs the observed saved-redraw profile for supported source context.
/// `source` is the MotionEvent source returned by the native Base event; the
/// harness's constructor produces source 0. Unsupported sizes/point counts are
/// not silently accepted by this bounded implementation.
fn saved(points: &[Point], size: f32, tool: u32, source: u32) -> Option<Vec<[f32; 3]>> {
    if points.is_empty() || !size.is_finite() || size < 0.0 || size >= i32::MAX as f32 {
        return None;
    }
    if !matches!(tool, 1..=3) {
        return None;
    }
    // Coordinates are bounded so float32 squared-distance calculations cannot
    // overflow: 8 * bound^2 is at most f32::MAX for any point pair.
    let coordinate_bound = (f64::from(f32::MAX) / 8.0).sqrt();
    if points.iter().any(|p| {
        !p.x.is_finite()
            || !p.y.is_finite()
            || p.x.abs() > coordinate_bound
            || p.y.abs() > coordinate_bound
            || !(p.x as f32).is_finite()
            || !(p.y as f32).is_finite()
    }) {
        return None;
    }
    if points.len() == 1 {
        return None; // The shipped ObjectStroke wrapper returns error 7 without AddPoint.
    }
    let input: Vec<P> = points.iter().copied().map(P::from).collect();
    let radius = (size * 0.5).max(1.0) + 0.5;
    if points.len() == 2 {
        return Some(vec![[input[0].x, input[0].y, radius]]);
    }
    // When every pre-terminal historical move is below the native 1.5-unit
    // axis gate, the wrapper leaves the start flag set and endPen emits one dot.
    let history_is_stationary = input[..input.len() - 1].windows(2).all(|pair| {
        let delta = pair[1].sub(pair[0]);
        delta.x.abs() < 1.5 && delta.y.abs() < 1.5
    });
    if history_is_stationary {
        return Some(vec![[input[0].x, input[0].y, radius]]);
    }
    let threshold = if tool == 1 || (tool == 2 && source == 0x1002) {
        50.0_f32
    } else {
        5.0_f32
    };
    let mut output = Vec::new();
    let mut alternate = true;
    let mut start = input[0];
    let mut control = input[1];
    let mut retained = input[1];
    let mut endpoint = input[1];
    let mut cubic_end = false;

    // RedrawPen visits all but the terminal event as history. The first
    // historical event seeds the drawable; later accepted events emit a
    // midpoint quadratic and advance its retained start/control state.
    for index in 2..input.len() - 1 {
        let previous = endpoint;
        let next = input[index];
        let distance = next.sub(previous).len();
        let accepted = if distance > threshold {
            alternate = true;
            true
        } else {
            let prior = alternate;
            alternate = !alternate;
            !prior
        };
        if accepted {
            let midpoint = retained.mid(next);
            emit_quad(&mut output, start, control, midpoint, size)?;
            start = midpoint;
            control = next;
            retained = next;
        }
        cubic_end = alternate;
        endpoint = next;
    }

    let terminal = *input.last()?;
    if cubic_end {
        let midpoint = control.mid(terminal);
        emit_cubic(&mut output, [start, control, midpoint, terminal], size)?;
    } else {
        emit_quad(&mut output, start, control, terminal, size)?;
    }
    Some(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn fixture() -> Value {
        serde_json::from_str(include_str!(
            "../../../../conformance/defaultpen-saved-native.json"
        ))
        .unwrap()
    }

    #[test]
    fn saved_quadratic_samples_match_native_addpoint_vectors_for_all_variants() {
        let fixture = fixture();
        for case in fixture["cases"].as_array().unwrap() {
            let input = &case["input"];
            let points = input["points"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| Point {
                    x: p[0].as_f64().unwrap(),
                    y: p[1].as_f64().unwrap(),
                })
                .collect::<Vec<_>>();
            let expected = case["native"]["triples"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| {
                    [
                        p[0].as_f64().unwrap() as f32,
                        p[1].as_f64().unwrap() as f32,
                        p[2].as_f64().unwrap() as f32,
                    ]
                })
                .collect::<Vec<_>>();
            let actual = saved(
                &points,
                input["size"].as_f64().unwrap() as f32,
                input["tool_type_raw"].as_u64().unwrap() as u32,
                0,
            );
            if expected.is_empty() {
                assert!(
                    actual.is_none(),
                    "{} should remain outside this sampler",
                    case["case"]
                );
                assert_eq!(case["native"]["error_codes"], serde_json::json!([7]));
                continue;
            }
            let actual = actual.unwrap();
            assert_eq!(actual.len(), expected.len(), "{} count", case["case"]);
            for (index, (actual, expected)) in actual.iter().zip(&expected).enumerate() {
                for component in 0..3 {
                    assert_eq!(
                        actual[component].to_bits(),
                        expected[component].to_bits(),
                        "{}[{index}][{component}]: {:?} != {:?}",
                        case["case"],
                        actual,
                        expected
                    );
                }
            }
        }
    }

    #[test]
    fn native_getrepeat_width_buckets_match_executed_helper() {
        let fixture = fixture();
        for row in fixture["repeat_matrix"].as_array().unwrap() {
            let width = row["width"].as_i64().unwrap() as i32;
            let lengths = row["lengths"].as_array().unwrap();
            let counts = row["counts"].as_array().unwrap();
            for (length, expected) in lengths.iter().zip(counts) {
                assert_eq!(
                    repeat(length.as_f64().unwrap() as f32, width),
                    expected.as_u64().unwrap() as usize,
                    "width={width}, length={length}"
                );
            }
        }
    }

    #[test]
    fn unsupported_context_is_not_mapped_to_an_approximation() {
        let points = [
            Point { x: 0.0, y: 0.0 },
            Point { x: 10.0, y: 0.0 },
            Point { x: 20.0, y: 0.0 },
            Point { x: 30.0, y: 10.0 },
        ];
        assert!(saved(&points, f32::NAN, 1, 0).is_none());
        assert!(saved(&points, -1.0, 1, 0).is_none());
        assert!(saved(&points, 10.0, 8, 0).is_none());
        assert!(saved(&points[..2], 10.0, 1, 0).is_some());
        let huge = [
            Point {
                x: f64::MAX,
                y: 0.0,
            },
            points[1],
            points[2],
        ];
        assert!(saved(&huge, 10.0, 1, 0).is_none());
        let nan = [
            Point {
                x: f64::NAN,
                y: 0.0,
            },
            points[1],
            points[2],
        ];
        assert!(saved(&nan, 10.0, 1, 0).is_none());
    }
}
