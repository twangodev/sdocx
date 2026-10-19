//! Bounded saved-source model for MagicPen GLV1.
//!
//! This is test-only evidence for type-2, stroke-type-0 synthetic ObjectStroke
//! inputs routed by native Drawing to GLV1. It does not model GLV2, saved-note
//! parsing, canvas context, or RTV1 painting.

use super::path::{P, Quad};

#[derive(Clone, Copy)]
struct FPoint {
    x: f32,
    y: f32,
}

impl FPoint {
    fn mid(self, other: Self) -> Self {
        Self {
            x: (self.x + other.x) * 0.5,
            y: (self.y + other.y) * 0.5,
        }
    }

    fn sub(self, other: Self) -> Self {
        Self {
            x: self.x - other.x,
            y: self.y - other.y,
        }
    }

    fn len(self) -> f32 {
        self.x.mul_add(self.x, self.y * self.y).sqrt()
    }
}

struct Cubic {
    points: [FPoint; 4],
    segments: Vec<(f32, u16)>,
    length: f32,
}

impl Cubic {
    fn new(points: [FPoint; 4]) -> Self {
        let mut path = Self {
            points,
            segments: Vec::new(),
            length: 0.0,
        };
        path.split(points, 0, 32767);
        path
    }

    fn split(&mut self, p: [FPoint; 4], t0: u16, t1: u16) {
        let line = |a: FPoint, b: FPoint, t: f32| FPoint {
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

    fn at(&self, distance: f32) -> FPoint {
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
        FPoint {
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

#[derive(Clone, Copy)]
enum SourceCurve {
    Quad([FPoint; 3]),
    Cubic([FPoint; 4]),
}

impl SourceCurve {
    fn length(self) -> f32 {
        match self {
            Self::Quad([a, b, c]) => quad(a, b, c).length,
            Self::Cubic(p) => Cubic::new(p).length,
        }
    }

    fn at(self, distance: f32) -> FPoint {
        match self {
            Self::Quad([a, b, c]) => {
                let p = quad(a, b, c).at(distance);
                FPoint { x: p.x, y: p.y }
            }
            Self::Cubic(p) => Cubic::new(p).at(distance),
        }
    }
}

fn quad(a: FPoint, b: FPoint, c: FPoint) -> Quad {
    Quad::new(
        P { x: a.x, y: a.y },
        P { x: b.x, y: b.y },
        P { x: c.x, y: c.y },
    )
}

fn speed_factor(distance: f32) -> f32 {
    if distance < 1.5 {
        1.0
    } else if distance < 9.5 {
        0.6
    } else if distance >= 35.0 {
        0.2
    } else {
        ((distance - 9.5) / -25.5).mul_add(0.4, 0.4) + 0.2
    }
}

fn width_target(old: f32, size: f32, pressure: f32, factor: f32) -> f32 {
    let adjustment = if factor < 0.35 {
        pressure
    } else if factor < 0.7 {
        factor.mul_add(0.5, pressure * 1.5) * 0.5
    } else {
        (factor + pressure) * 0.5
    };
    let candidate = size.mul_add(adjustment, old) * 0.5;
    let delta = candidate - old;
    if delta > 2.0 {
        old + 2.0
    } else if delta < -2.0 {
        old - 2.0
    } else {
        candidate
    }
}

fn terminal_width_target(old: f32, size: f32, pressure: f32, factor: f32) -> f32 {
    let adjustment = if factor < 0.35 {
        pressure * 1.5 + factor * 0.5
    } else if factor < 0.7 {
        factor.mul_add(0.5, pressure * 1.5)
    } else {
        pressure + factor
    };
    let candidate = size.mul_add(adjustment * 0.5, old) * 0.5;
    let delta = candidate - old;
    if delta > 6.0 {
        old + 6.0
    } else if delta < -6.0 {
        old - 6.0
    } else if delta < 0.0 && old > 1.0 && old < 10.0 && -delta < 0.5 {
        old - 0.5
    } else {
        candidate
    }
}

fn repeat_index(length: f32, interval: f32) -> usize {
    let scaled = if interval <= 5.0 {
        length * 0.5
    } else if interval <= 10.0 {
        length / 3.0
    } else if interval <= 15.0 {
        length * 0.25
    } else if interval <= 20.0 {
        length / 5.0
    } else if interval <= 30.0 {
        length / 6.0
    } else if interval <= 40.0 {
        length / 7.0
    } else if interval <= 60.0 {
        length / 10.0
    } else {
        length * 0.125
    };
    (scaled as usize).max(2) + 1
}

fn source_point(p: [f32; 2]) -> FPoint {
    FPoint { x: p[0], y: p[1] }
}

fn source_glv1_curve(points: &[[f32; 2]], pressure: &[f32], source_size: f32) -> Vec<[u32; 3]> {
    assert!(points.len() >= 2 && points.len() == pressure.len());
    let size = source_size.max(0.4);
    let p: Vec<_> = points.iter().copied().map(source_point).collect();
    let mut paths: Vec<(SourceCurve, f32, f32, bool, bool)> = Vec::new();
    let mut width = size * pressure[1];
    let mut last_factor = 1.0_f32;
    if points.len() == 2 {
        paths.push((
            SourceCurve::Quad([p[0], p[1], p[1]]),
            width,
            width,
            true,
            true,
        ));
    } else if points.len() == 3 {
        let distance = p[1].sub(p[2]).len();
        if distance < 1.5 {
            width = size * pressure[2];
            paths.push((
                SourceCurve::Quad([p[0], p[2], p[2]]),
                width,
                width,
                true,
                true,
            ));
        } else {
            let c2 = p[1].mid(p[2]);
            let terminal = size.mul_add(pressure[2].mul_add(0.25, 0.25), width * 0.5);
            paths.push((
                SourceCurve::Cubic([p[0], p[1], c2, p[2]]),
                width,
                terminal,
                true,
                true,
            ));
        }
    } else {
        let mut previous_end = p[0];
        let mut control = p[1];
        let mut i = 3;
        while i < p.len() {
            let factor = speed_factor(p[i - 1].sub(p[i]).len());
            let target = width_target(width, size, pressure[i], factor);
            let end = control.mid(p[i]);
            paths.push((
                SourceCurve::Quad([previous_end, control, end]),
                width,
                target,
                paths.is_empty(),
                false,
            ));
            width = target;
            previous_end = end;
            control = p[i];
            last_factor = factor;
            i += 2;
        }
        let final_index = p.len() - 1;
        let terminal = terminal_width_target(width, size, pressure[final_index], last_factor);
        if final_index % 2 == 0 {
            paths.push((
                SourceCurve::Cubic([
                    previous_end,
                    control,
                    control.mid(p[final_index]),
                    p[final_index],
                ]),
                width,
                terminal,
                false,
                true,
            ));
        } else {
            paths.push((
                SourceCurve::Quad([previous_end, control, p[final_index]]),
                width,
                terminal,
                false,
                true,
            ));
        }
    }

    let mut output = Vec::new();
    for (path, from_width, to_width, first, terminal) in paths {
        let length = path.length();
        let repeat = repeat_index(length, from_width.min(to_width));
        let step = length / repeat as f32;
        let radius_step = (to_width * 0.5 - from_width * 0.5) / repeat as f32;
        let width_step = (to_width - from_width) / repeat as f32;
        let mut distance = 0.0_f32;
        let mut radius = from_width * 0.5;
        let mut current_width = from_width;
        for sample in 0..=repeat {
            let position = if sample == repeat && !matches!(path, SourceCurve::Cubic(_)) {
                match path {
                    SourceCurve::Quad([_, _, end]) => end,
                    SourceCurve::Cubic([_, _, _, end]) => end,
                }
            } else if sample == repeat
                && terminal
                && points.len() > 3
                && matches!(path, SourceCurve::Cubic(_))
            {
                path.at(length)
            } else {
                path.at(distance)
            };
            if first || sample != 0 {
                let sample_width = if sample == repeat && !terminal {
                    to_width
                } else {
                    current_width
                };
                let stored = if points.len() == 3 {
                    radius + 0.5
                } else {
                    sample_width * 0.5 + 0.5
                };
                output.push([position.x.to_bits(), position.y.to_bits(), stored.to_bits()]);
            }
            distance += step;
            radius += radius_step;
            current_width += width_step;
        }
    }
    output
}

fn source_glv1_no_curve(points: &[[f32; 2]], pressure: &[f32], source_size: f32) -> Vec<[u32; 3]> {
    assert!(points.len() >= 2 && points.len() == pressure.len());
    let size = source_size.max(0.4);
    let mut output = Vec::new();
    let mut width = size * pressure[1];
    for edge in 1..points.len() {
        let a = source_point(points[edge - 1]);
        let b = source_point(points[edge]);
        let delta = b.sub(a);
        let length = delta.len();
        let target = size * pressure[edge];
        let repeat = repeat_index(length, width.min(target));
        let distance_step = length / repeat as f32;
        let radius_step = (target * 0.5 - width * 0.5) / repeat as f32;
        let first = edge == 1;
        let samples = if first { repeat + 1 } else { repeat };
        let mut distance = 0.0_f32;
        let mut radius = width * 0.5;
        for sample in 0..samples {
            let terminal = first && sample == repeat;
            let t_max = 32767.0_f32 * f32::from_bits(0x3800_00fd);
            let t = (distance * t_max) / length;
            let position = if terminal {
                b
            } else {
                FPoint {
                    x: delta.x.mul_add(t, a.x),
                    y: delta.y.mul_add(t, a.y),
                }
            };
            if first || sample != 0 {
                output.push([
                    position.x.to_bits(),
                    position.y.to_bits(),
                    (radius + 0.5).to_bits(),
                ]);
            }
            distance += distance_step;
            radius += radius_step;
        }
        width = target;
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn f32_bits(word: &str) -> f32 {
        f32::from_bits(u32::from_str_radix(word, 16).unwrap())
    }

    fn fixture_bits(value: &Value) -> Vec<u32> {
        value
            .as_array()
            .unwrap()
            .iter()
            .map(|word| u32::from_str_radix(word.as_str().unwrap(), 16).unwrap())
            .collect()
    }

    #[test]
    fn source_model_matches_native_glv1_getters_and_complete_rtv1_vectors() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/magicpen-glv1-source-native.json"
        ))
        .unwrap();
        assert_eq!(fixture["schema"], "sdocx-magicpen-glv1-source-native-v1");
        let cases = fixture["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 26);
        let mut seen = std::collections::BTreeSet::new();
        for case in cases {
            let id = case["id"].as_str().unwrap();
            assert!(seen.insert(id), "duplicate case {id}");
            let input = &case["input"];
            assert_eq!(input["tool_type"], 2, "{id}");
            assert_eq!(input["stroke_type"], 0, "{id}");
            assert_eq!(input["advanced_setting"], "1;", "{id}");
            let point_words = fixture_bits(&input["points_f32_u32_hex"]);
            let pressure_words = fixture_bits(&input["pressure_f32_u32_hex"]);
            assert_eq!(point_words.len(), pressure_words.len() * 2, "{id}");
            let points: Vec<_> = point_words
                .chunks_exact(2)
                .map(|xy| [f32::from_bits(xy[0]), f32::from_bits(xy[1])])
                .collect();
            let pressure: Vec<_> = pressure_words.into_iter().map(f32::from_bits).collect();
            let size = f32_bits(input["size_f32_u32_hex"].as_str().unwrap());
            let predicted = if input["curve_enabled"].as_bool().unwrap() {
                source_glv1_curve(&points, &pressure, size)
            } else {
                source_glv1_no_curve(&points, &pressure, size)
            };
            let expected = fixture_bits(&case["native"]["xyz_f32_u32_hex"]);
            let actual: Vec<u32> = predicted.into_iter().flatten().collect();
            assert_eq!(actual, expected, "{id}");
            assert_eq!(
                case["native"]["record_count"].as_u64().unwrap() as usize * 3,
                expected.len(),
                "{id}"
            );
        }
    }
}
