//! Test-only reconstruction of the explicitly selected InkPen2 drawing V9
//! saved-redraw numeric path. It is deliberately not wired to production.
use super::path::{P, Quad};

const MAX_POINTS: usize = 65_536;
const MAX_EMISSIONS: usize = 1_000_000;
const MIN_RADIUS: f32 = f32::from_bits(0x3dcccccd);

/// Runtime values read by the saved redraw path, supplied explicitly so this
/// model does not guess saved settings or drawable state.
#[derive(Clone, Copy, Debug)]
pub(super) struct Runtime {
    pub inverse_scale: [f32; 2],
    pub spacing: f32,
    pub tolerance: f32,
    pub resolution_min: i32,
    pub initial_alternation: bool,
    pub initial_time: i32,
}

/// Saved channels. The numeric geometry path consumes XY, time and tool; the
/// arrays are required to have matching lengths because native terminal event
/// construction dereferences pressure even though it does not use its value.
#[derive(Clone, Copy, Debug)]
pub(super) struct Input<'a> {
    pub points: &'a [P],
    pub times: &'a [i32],
    pub tools: &'a [i32],
    pub pressures: &'a [f32],
    pub size: f32,
    pub curve_enabled: bool,
    pub is_shape: bool,
    pub runtime: Runtime,
}

#[derive(Clone, Debug)]
pub(super) struct Output {
    /// RTV3's nine floats per successful callback AddPoint.
    pub vertices: Vec<[f32; 9]>,
    /// Exclusive output count after each input sample, for attribution.
    pub sample_ends: Vec<usize>,
    pub terminal: std::ops::Range<usize>,
}

struct Core<'a> {
    input: Input<'a>,
    output: Output,
    history: [f32; 5],
    history_index: usize,
    previous_time: i64,
    control: P,
    last: P,
    residual: f32,
    radius: f32,
    no_emission: bool,
    alternation: bool,
    bounds: [f32; 4],
}

impl<'a> Core<'a> {
    fn add_point(&mut self, p: P, radius: f32) -> Option<()> {
        let [sx, sy] = self.input.runtime.inverse_scale;
        if !p.x.is_finite() || !p.y.is_finite() || !radius.is_finite() || sx <= 0. || sy <= 0. {
            return None;
        }
        let r = radius.max(MIN_RADIUS);
        let a = r / sx;
        let b = r / sy;
        let t = a.min(b);
        let k = if t < 0.5 { 0.5 / t } else { 1.0 };
        let coverage = 1.0 / k;
        let k = k.max(1.0);
        let ux = a.mul_add(k, 0.5);
        let uy = b.mul_add(k, 0.5);
        let m = ux.max(uy);
        let edge = ((m - 1.0) / m) * 0.5;
        let vertex = [p.x, p.y, sx * ux, sy * uy, edge, coverage, 0., 0., 0.];
        if vertex.iter().any(|v| !v.is_finite()) || self.output.vertices.len() >= MAX_EMISSIONS {
            return None;
        }
        self.bounds[0] = self.bounds[0].min(p.x - r);
        self.bounds[1] = self.bounds[1].min(p.y - r);
        self.bounds[2] = self.bounds[2].max(p.x + r);
        self.bounds[3] = self.bounds[3].max(p.y + r);
        self.output.vertices.push(vertex);
        self.no_emission = false;
        Some(())
    }

    fn movement(&mut self, path: &Path, time: i64) -> Option<()> {
        let length = path.length();
        let dt = time - self.previous_time;
        let candidate = if dt == 0 { 1000. } else { (dt as f32) * 1000. };
        let denominator = candidate.min(65535.);
        let factor = if self.input.runtime.resolution_min == 0 {
            1.
        } else {
            1440. / self.input.runtime.resolution_min as f32
        };
        let speed = ((length / denominator) * factor) * 1000.;
        if !speed.is_finite() {
            return None;
        }
        self.history[self.history_index] = speed;
        self.history_index = (self.history_index + 1) % self.history.len();
        let average = self.history.iter().copied().fold(0., |a, b| a + b) / 5.;
        let old = self.radius;
        let size = self.input.size;
        self.radius = if average > 5. {
            (size / 3.75).max(old * f32::from_bits(0x3f75c28f))
        } else if average > 3. {
            (size / 3.75).max(old * f32::from_bits(0x3f79999a))
        } else if average as f64 > 0.8 && average <= 3. {
            (size / 3.75).max(old * f32::from_bits(0x3f7d70a4))
        } else if (average as f64) < 0.2 {
            (size * 0.5).min(old * f32::from_bits(0x3f866666))
        } else if (average as f64) >= 0.2 && average < 0.5 {
            (size * 0.5).min(old * f32::from_bits(0x3f828f5c))
        } else {
            old
        };
        let spacing = self.input.runtime.spacing;
        let quotient = ((length - self.residual) / spacing).trunc();
        if !quotient.is_finite() || quotient < i32::MIN as f32 || quotient > i32::MAX as f32 {
            return None;
        }
        let count = (quotient as i32).checked_add(1)?;
        if count <= 0 {
            return None;
        }
        let increment = (self.radius - old) / count as f32;
        let mut distance = self.residual;
        let mut radius = old;
        let mut n = 0;
        while distance <= length {
            if let Some(p) = path.at(distance) {
                self.add_point(p, radius)?;
            }
            distance += spacing;
            radius += increment;
            n += 1;
            if n > MAX_EMISSIONS {
                return None;
            }
        }
        self.residual = distance - length;
        self.previous_time = time;
        Some(())
    }

    fn finish(mut self) -> Option<Output> {
        let n = self.input.points.len();
        for i in 1..n {
            let p = self.input.points[i];
            let dx = p.x - self.control.x;
            let dy = p.y - self.control.y;
            let distance = dx.mul_add(dx, dy * dy).sqrt();
            if distance < self.input.runtime.tolerance {
                self.output.sample_ends.push(self.output.vertices.len());
                continue;
            }
            let time = i64::from(self.input.times[i]);
            let tool = self.input.tools[i];
            if tool == 1 && distance < 20. {
                if self.alternation {
                    self.alternation = false;
                    self.output.sample_ends.push(self.output.vertices.len());
                    continue;
                }
                self.alternation = true;
            } else {
                self.alternation = true;
            }
            let path = if self.input.curve_enabled {
                let mid = self.control.mid(p);
                Path::Quad(Quad::new(self.last, self.control, mid))
            } else {
                Path::Line(self.control, p)
            };
            self.movement(&path, time)?;
            self.last = if self.input.curve_enabled {
                self.control.mid(p)
            } else {
                p
            };
            self.control = p;
            self.output.sample_ends.push(self.output.vertices.len());
        }
        let start = self.output.vertices.len();
        if self.no_emission {
            self.add_point(self.control, self.radius * 0.5)?;
        } else {
            let endpoint = *self.input.points.last()?;
            let terminal = if self.input.curve_enabled {
                Path::Quad(Quad::new(self.last, self.control, endpoint))
            } else {
                Path::Line(self.control, endpoint)
            };
            let mut d = self.residual;
            let mut n = 0;
            while d <= terminal.length() {
                if let Some(p) = terminal.at(d) {
                    self.add_point(p, self.radius)?;
                }
                d += self.input.runtime.spacing;
                n += 1;
                if n > MAX_EMISSIONS {
                    return None;
                }
            }
            self.add_point(*self.input.points.last()?, self.radius)?;
        }
        let end = self.output.vertices.len();
        self.output.terminal = start..end;
        Some(self.output)
    }
}

enum Path {
    Line(P, P),
    Quad(Quad),
}
impl Path {
    fn length(&self) -> f32 {
        match self {
            Self::Line(a, b) => b.sub(*a).len(),
            Self::Quad(q) => q.length,
        }
    }
    fn at(&self, d: f32) -> Option<P> {
        match self {
            Self::Line(a, b) => {
                let len = b.sub(*a).len();
                // SmPath stores a 15-bit terminal parameter and scales it
                // before dividing the requested distance by segment length.
                let t1 = 32_767. * f32::from_bits(0x3800_00fd);
                (len > 0.).then(|| a.lerp(*b, d * t1 / len))
            }
            Self::Quad(q) => (q.length > 0.).then(|| q.at(d)),
        }
    }
}

/// Reconstructs the native plain line-type-0 saved path for a fresh drawable
/// state. Saved samples are borrowed and never rewritten.
pub(super) fn redraw(input: Input<'_>) -> Option<Output> {
    let n = input.points.len();
    let rt = input.runtime;
    if input.is_shape
        || n == 0
        || n > MAX_POINTS
        || input.times.len() != n
        || input.tools.len() != n
        || input.pressures.len() != n
        || !input.size.is_finite()
        || input.size <= 0.
        || !rt.spacing.is_finite()
        || rt.spacing <= 0.
        || !rt.tolerance.is_finite()
        || rt.tolerance < 0.
        || rt.inverse_scale.iter().any(|v| !v.is_finite() || *v <= 0.)
        || rt.resolution_min < 0
        || input
            .points
            .iter()
            .any(|p| !p.x.is_finite() || !p.y.is_finite())
    {
        return None;
    }
    let first = input.points[0];
    let radius = input.size / 2.2_f32;
    let mut core = Core {
        input,
        output: Output {
            vertices: Vec::new(),
            sample_ends: Vec::with_capacity(n),
            terminal: 0..0,
        },
        history: [0.; 5],
        history_index: 0,
        previous_time: i64::from(input.runtime.initial_time),
        control: first,
        last: first,
        residual: input.runtime.spacing,
        radius,
        no_emission: true,
        alternation: input.runtime.initial_alternation,
        bounds: [
            f32::INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
        ],
    };
    core.output.sample_ends.push(0);
    core.finish()
}

#[cfg(test)]
mod tests {
    use super::{Input, P, Runtime, redraw};
    use serde_json::Value;

    fn number(value: &Value) -> f32 {
        value.as_f64().unwrap() as f32
    }

    #[test]
    fn saved_native_fixture_matches_every_rtv3_float_bit() {
        let fixture: Value =
            serde_json::from_str(include_str!("../../../../conformance/inkpen2-saved.json"))
                .unwrap();
        let cases = fixture["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 3, "native fixture case coverage changed");
        for case in cases {
            let points: Vec<P> = case["points"]
                .as_array()
                .unwrap()
                .iter()
                .map(|point| P {
                    x: number(&point[0]),
                    y: number(&point[1]),
                })
                .collect();
            let timestamps: Vec<i32> = case["timestamps"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_i64().unwrap() as i32)
                .collect();
            let tool = case["tool_type"].as_i64().unwrap() as i32;
            let tools = vec![tool; points.len()];
            let pressures: Vec<f32> = case["pressures"]
                .as_array()
                .unwrap()
                .iter()
                .map(number)
                .collect();
            let runtime_context = &case["native"]["runtime_context"];
            assert_eq!(
                runtime_context["requested_fixture_tolerance_applied"],
                false
            );
            assert_eq!(
                number(&case["tolerance"]).to_bits(),
                number(&runtime_context["requested_fixture_tolerance"]).to_bits()
            );
            let threshold_samples = runtime_context["drawable_threshold_at_drawline_entry"]
                .as_array()
                .unwrap();
            assert!(!threshold_samples.is_empty());
            let tolerance = number(&threshold_samples[0]);
            assert!(
                threshold_samples
                    .iter()
                    .all(|sample| number(sample).to_bits() == tolerance.to_bits())
            );

            let actual = redraw(Input {
                points: &points,
                times: &timestamps,
                tools: &tools,
                pressures: &pressures,
                size: number(&case["size"]),
                curve_enabled: case["curve_enabled"].as_bool().unwrap(),
                is_shape: case["is_shape"].as_bool().unwrap(),
                runtime: Runtime {
                    inverse_scale: [
                        number(&case["inverse_scale"][0]),
                        number(&case["inverse_scale"][1]),
                    ],
                    spacing: number(&case["spacing"]),
                    tolerance,
                    resolution_min: case["resolution"][0].as_i64().unwrap() as i32,
                    initial_alternation: case["initial_alternation"].as_bool().unwrap(),
                    initial_time: timestamps[0],
                },
            })
            .unwrap_or_else(|| panic!("{}: supported fixture was rejected", case["name"]));

            let expected = case["native"]["rtv3"].as_array().unwrap();
            assert_eq!(
                actual.vertices.len(),
                expected.len(),
                "{}: RTV3 record count",
                case["name"]
            );
            for (record_index, (actual, expected)) in
                actual.vertices.iter().zip(expected).enumerate()
            {
                let expected = expected.as_array().unwrap();
                assert_eq!(expected.len(), 9, "RTV3 record width");
                for (lane, expected) in expected.iter().enumerate() {
                    assert_eq!(
                        actual[lane].to_bits(),
                        number(expected).to_bits(),
                        "{}: record {record_index}, lane {lane}",
                        case["name"]
                    );
                }
            }
        }
    }

    #[test]
    fn rejects_unmodeled_shape_mode() {
        let point = [P { x: 0., y: 0. }];
        let time = [0];
        let tool = [2];
        let pressure = [0.5];
        assert!(
            redraw(Input {
                points: &point,
                times: &time,
                tools: &tool,
                pressures: &pressure,
                size: 4.,
                curve_enabled: false,
                is_shape: true,
                runtime: Runtime {
                    inverse_scale: [1., 1.],
                    spacing: 1.,
                    tolerance: 2.,
                    resolution_min: 1440,
                    initial_alternation: false,
                    initial_time: 0,
                },
            })
            .is_none()
        );
    }
}
