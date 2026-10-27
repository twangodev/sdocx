//! Bounded numeric reconstruction of explicitly selected InkPen2 V9 saved redraw.
//! Native conformance evidence and test-only probes remain in the cfg(test) section.
use super::path::{P, Quad};

pub(crate) fn source_point(x: f32, y: f32) -> P {
    P { x, y }
}

const MAX_POINTS: usize = 65_536;
#[cfg(test)]
const DIAGNOSTIC_MAX_EMISSIONS: usize = 1_000_000;
const MIN_RADIUS: f32 = f32::from_bits(0x3dcccccd);

/// Runtime values read by the saved redraw path, supplied explicitly so this
/// model does not guess saved settings or drawable state.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Runtime {
    pub inverse_scale: [f32; 2],
    pub spacing: f32,
    pub tolerance: f32,
    pub tool_threshold: f32,
    pub resolution_min: i32,
    pub initial_alternation: bool,
    pub initial_time: i32,
}

/// Saved channels. The numeric geometry path consumes XY, time and tool; the
/// arrays are required to have matching lengths because native terminal event
/// construction dereferences pressure even though it does not use its value.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Input<'a> {
    pub points: &'a [P],
    pub times: &'a [i32],
    pub tools: &'a [i32],
    pub pressures: &'a [f32],
    /// Source RGB from ObjectStroke::GetColor, without the packed alpha byte.
    pub rgb: [u8; 3],
    /// Explicit saved rainbow settings when native V9's rainbow mode is selected.
    /// Public dispatch currently leaves this unset until paint transport is ready.
    pub rainbow: Option<Rainbow<'a>>,
    pub size: f32,
    pub curve_enabled: bool,
    pub is_shape: bool,
    pub runtime: Runtime,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Rainbow<'a> {
    pub distance: i32,
    pub offset: f32,
    pub palette_argb: &'a [u32],
}

impl Rainbow<'_> {
    fn color_at(&self, distance: f32, fallback: [u8; 3], offset: f32) -> Option<[f32; 3]> {
        if self.palette_argb.is_empty() {
            return Some(fallback.map(|channel| f32::from(channel) / 255.0));
        }
        // Native uses this signed integer as a floating period. Zero produces
        // non-finite values; negative periods are retained when their actual
        // converted palette index remains within the source vector.
        if self.distance == 0 || !distance.is_finite() || !offset.is_finite() {
            return None;
        }
        let period = self.distance as f32;
        let sum = offset + distance;
        if !sum.is_finite() {
            return None;
        }
        let quotient = sum / period;
        if !quotient.is_finite() {
            return None;
        }
        // Native FCVTZS produces a signed 32-bit integer (saturating at the
        // finite extremes), then SCVTF converts that integer back to f32.
        let quotient = (quotient as i32) as f32;
        let wrapped = (-quotient).mul_add(period, sum);
        let phase = wrapped / period;
        if !phase.is_finite() {
            return None;
        }

        let count = self.palette_argb.len();
        let step = (1.0f64 / count as f64) as f32;
        let raw_index = phase / step;
        if !raw_index.is_finite() {
            return None;
        }
        // calculateRainbowColor uses FCVTZS followed by a signed `index < n`
        // conditional select. Positive overflow clamps to the last palette
        // entry; it does not wrap to zero.
        // This is another FCVTZS. Small negative phases can truncate to index
        // zero and extrapolate; reject only converted negative indices, which
        // would read before the palette.
        let index = raw_index as i32;
        if index < 0 {
            return None;
        }
        let index = if index as usize >= count {
            count - 1
        } else {
            index as usize
        };
        let next = if index + 1 == count { 0 } else { index + 1 };
        let fraction = (-(index as f32)).mul_add(step, phase) / step;
        let first = self.palette_argb[index];
        let second = self.palette_argb[next];
        let channel = |shift: u32| {
            let a = ((first >> shift) & 0xff) as f32;
            let b = ((second >> shift) & 0xff) as f32;
            (b - a).mul_add(fraction, a) / 255.0
        };
        let rgb = [channel(16), channel(8), channel(0)];
        rgb.iter().all(|value| value.is_finite()).then_some(rgb)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Output {
    /// RTV3's nine floats per successful callback AddPoint.
    pub vertices: Vec<[f32; 9]>,
    /// Exclusive output count after each input sample, for attribution.
    pub sample_ends: Vec<usize>,
    pub terminal: std::ops::Range<usize>,
    /// Native V9 drawable byte +100 after this redraw; callers may carry it
    /// into the next stroke on the same drawable.
    pub final_alternation: bool,
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
    max_emissions: usize,
    bounds: [f32; 4],
    rainbow_offset: f32,
    current_rgb: [f32; 3],
    v1_callback: bool,
}

impl<'a> Core<'a> {
    fn add_point(&mut self, p: P, radius: f32, rgb: [f32; 3]) -> Option<()> {
        // RTV1 clamps the drawable radius before AddPoint adds 0.75. This
        // branch is restricted to native geometry controls, not paint.
        if self.v1_callback {
            if self.output.vertices.len() >= self.max_emissions {
                return None;
            }
            self.output
                .vertices
                .push([p.x, p.y, radius.max(1.) + 0.75, 0., 0., 0., 0., 0., 0.]);
            self.no_emission = false;
            return Some(());
        }
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
        let vertex = [
            p.x,
            p.y,
            sx * ux,
            sy * uy,
            edge,
            coverage,
            rgb[0],
            rgb[1],
            rgb[2],
        ];
        if vertex.iter().any(|v| !v.is_finite()) || self.output.vertices.len() >= self.max_emissions
        {
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
        // Native commits the speed/radius and time state even when residual
        // spacing leaves no stamp on this segment. Avoid rejecting that
        // zero-emission transition; its radius increment is unused.
        let increment = if count > 0 {
            (self.radius - old) / count as f32
        } else {
            0.0
        };
        let mut distance = self.residual;
        let mut radius = old;
        let mut n = 0;
        while distance <= length {
            if let Some(p) = path.at(distance) {
                let rgb = if let Some(rainbow) = self.input.rainbow {
                    rainbow.color_at(distance, self.input.rgb, self.rainbow_offset)?
                } else {
                    self.input.rgb.map(|channel| f32::from(channel) / 255.0)
                };
                self.current_rgb = rgb;
                self.add_point(p, radius, rgb)?;
            }
            distance += spacing;
            radius += increment;
            n += 1;
            if n > self.max_emissions {
                return None;
            }
        }
        if let Some(rainbow) = self.input.rainbow
            && !rainbow.palette_argb.is_empty()
        {
            self.rainbow_offset += length;
            if !self.rainbow_offset.is_finite() {
                return None;
            }
        }
        self.residual = distance - length;
        self.previous_time = time;
        Some(())
    }

    fn finish(mut self) -> Option<Output> {
        let n = self.input.points.len();
        let movement_end = if self.v1_callback {
            n.saturating_sub(1)
        } else {
            n
        };
        for i in 1..movement_end {
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
            if tool == 1 && distance < self.input.runtime.tool_threshold {
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
        // V1 endPen uses the last sample as the quadratic endpoint and
        // tapers to 0.6 times the last movement radius.
        if self.v1_callback {
            let endpoint = *self.input.points.last()?;
            let terminal = Quad::new(self.last, self.control, endpoint);
            let mut d = self.residual;
            let count = ((terminal.length - d) / self.input.runtime.spacing) as i32 + 1;
            let target = ((self.radius as f64) * 0.6) as f32;
            let increment = (target - self.radius) / count as f32;
            let mut radius = self.radius;
            while d <= terminal.length {
                self.add_point(terminal.at(d), radius, self.current_rgb)?;
                radius += increment;
                d += self.input.runtime.spacing;
            }
            self.output.terminal = start..self.output.vertices.len();
            return Some(self.output);
        }
        if self.no_emission {
            let rgb = if let Some(rainbow) = self.input.rainbow {
                if rainbow.palette_argb.is_empty() {
                    rainbow.color_at(0.0, self.input.rgb, 0.0)?
                } else {
                    let period = rainbow.distance as f32;
                    if period == 0.0 || !period.is_finite() || !self.rainbow_offset.is_finite() {
                        return None;
                    }
                    let wrapped = self.rainbow_offset % period;
                    rainbow.color_at(wrapped, self.input.rgb, 0.0)?
                }
            } else {
                self.input.rgb.map(|channel| f32::from(channel) / 255.0)
            };
            self.current_rgb = rgb;
            self.add_point(self.control, self.radius * 0.5, rgb)?;
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
                    self.add_point(p, self.radius, self.current_rgb)?;
                }
                d += self.input.runtime.spacing;
                n += 1;
                if n > self.max_emissions {
                    return None;
                }
            }
            self.add_point(*self.input.points.last()?, self.radius, self.current_rgb)?;
        }
        let end = self.output.vertices.len();
        self.output.terminal = start..end;
        self.output.final_alternation = self.alternation;
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
#[cfg(test)]
pub(crate) fn redraw(input: Input<'_>) -> Option<Output> {
    redraw_bounded(input, DIAGNOSTIC_MAX_EMISSIONS)
}

/// Runs one producer invocation with a caller-selected allocation ceiling.
/// This is used by public dispatch to reject excessive vector expansion while
/// leaving the diagnostic fixture path at its broader historical bound.
pub(crate) fn redraw_bounded(input: Input<'_>, max_emissions: usize) -> Option<Output> {
    redraw_with_mode(input, max_emissions, false)
}

pub(crate) fn redraw_v1_bounded(input: Input<'_>, max_emissions: usize) -> Option<Output> {
    redraw_with_mode(input, max_emissions, true)
}

fn redraw_with_mode(input: Input<'_>, max_emissions: usize, v1: bool) -> Option<Output> {
    let n = input.points.len();
    let rt = input.runtime;
    if input.is_shape
        || n == 0
        || n > MAX_POINTS
        || max_emissions == 0
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
            final_alternation: input.runtime.initial_alternation,
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
        max_emissions,
        bounds: [
            f32::INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
        ],
        rainbow_offset: input.rainbow.map_or(0.0, |rainbow| rainbow.offset),
        current_rgb: input.rgb.map(|channel| f32::from(channel) / 255.0),
        v1_callback: v1,
    };
    core.output.sample_ends.push(0);
    if v1 {
        core.add_point(first, input.size / 3., [0.; 3])?;
    }
    core.finish()
}

#[cfg(test)]
mod tests {
    use super::{Input, P, Rainbow, Runtime, redraw};
    use serde_json::Value;

    fn hex_bytes(value: &str) -> Vec<u8> {
        assert!(value.len().is_multiple_of(2));
        value
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect()
    }

    fn floats_from_le_hex(value: &str) -> Vec<f32> {
        hex_bytes(value)
            .as_chunks::<4>()
            .0
            .iter()
            .map(|bytes| f32::from_le_bytes(*bytes))
            .collect()
    }

    fn floats_from_le_bytes(value: &[u8]) -> Vec<f32> {
        value
            .as_chunks::<4>()
            .0
            .iter()
            .map(|bytes| f32::from_le_bytes(*bytes))
            .collect()
    }

    fn f32_from_le_hex(value: &Value) -> f32 {
        let bytes = hex_bytes(value.as_str().unwrap());
        f32::from_le_bytes(bytes.try_into().unwrap())
    }

    fn redraw_from_native_caller(case: &Value, phase: &str) -> super::Output {
        let input = &case["input"];
        let points: Vec<P> = input["points_f32_bits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|point| {
                let values = point
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(f32_from_le_hex)
                    .collect::<Vec<_>>();
                P {
                    x: values[0],
                    y: values[1],
                }
            })
            .collect();
        let times: Vec<i32> = input["timestamps_i32"]
            .as_array()
            .unwrap()
            .iter()
            .map(|time| time.as_i64().unwrap() as i32)
            .collect();
        let tool = input["tool_type"].as_i64().unwrap() as i32;
        let tools = vec![tool; points.len()];
        let pressures: Vec<f32> = input["pressure_f32_bits"]
            .as_array()
            .unwrap()
            .iter()
            .map(f32_from_le_hex)
            .collect();
        let native_calls = &case["native_calls"];
        let rgb = if let Some(packed) = native_calls["source_ObjectStroke_GetColor_u32"].as_u64() {
            let argb = packed as u32;
            assert_eq!(input["color_argb_u32"].as_u64(), Some(packed));
            let setter_calls = native_calls["paint_color_calls"].as_array().unwrap();
            assert!(setter_calls.iter().any(|call| {
                call["method"] == "SPen::Pen::SetColor(int)"
                    && call["phase"] == "draw_object_stroke"
                    && call["argb_u32"] == format!("0x{argb:08x}")
            }));
            assert!(setter_calls.iter().any(|call| {
                call["method"] == "SPen::PenDrawableRT::SetPenData(float, int)"
                    && call["phase"] == phase
                    && call["argb_u32"] == format!("0x{argb:08x}")
                    && u32::from_str_radix(
                        call["size_f32_bits"]
                            .as_str()
                            .unwrap()
                            .trim_start_matches("0x"),
                        16,
                    )
                    .unwrap()
                        == u32::from_le_bytes(
                            hex_bytes(input["pen_size_f32_bits"].as_str().unwrap())
                                .try_into()
                                .unwrap(),
                        )
            }));
            [
                ((argb >> 16) & 0xff) as u8,
                ((argb >> 8) & 0xff) as u8,
                (argb & 0xff) as u8,
            ]
        } else {
            [0; 3]
        };
        let entry = native_calls["selected_drawable_redraw_entry_state"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["phase"] == phase)
            .expect("V9 native redraw entry state is required");
        let drawline_entry = native_calls["drawline_threshold_entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["phase"] == phase)
            .expect("V9 drawLine threshold entry is required");
        assert_eq!(entry["entry_pc"], "0x504e1d0");
        assert_eq!(
            entry["receiver_address"],
            native_calls["selected_drawable_state"]["address"]
        );
        let resolution = &native_calls["pen_screen_resolution"][0];
        let snapshots = native_calls["ObjectStroke_getter_snapshots"]
            .as_array()
            .unwrap();
        let before_redraw = snapshots
            .iter()
            .find(|snapshot| snapshot["label"] == "before_saved_RedrawPen")
            .unwrap();
        let inverse_scale = native_calls["drawable_getInverseScale_f32_bits"]
            .as_array()
            .unwrap()
            .iter()
            .map(f32_from_le_hex)
            .collect::<Vec<_>>();
        redraw(Input {
            points: &points,
            times: &times,
            tools: &tools,
            pressures: &pressures,
            rgb,
            rainbow: None,
            size: f32_from_le_hex(&native_calls["drawable_getSize_f32_bits"]),
            curve_enabled: before_redraw["curve_enabled"].as_i64().unwrap() != 0,
            is_shape: before_redraw["shape"].as_i64().unwrap() != 0,
            runtime: Runtime {
                inverse_scale: [inverse_scale[0], inverse_scale[1]],
                spacing: f32_from_le_hex(&entry["spacing_plus_88_f32_bits"]),
                // RedrawPen entry fields are captured as provenance, but the
                // native body seeds its sampling residual from spacing and
                // DrawLine's tolerance is read after the body has normalized it.
                tolerance: f32_from_le_hex(&drawline_entry["plus44_tolerance_f32_bits"]),
                tool_threshold: f32_from_le_hex(&drawline_entry["plus96_tool_threshold_f32_bits"]),
                resolution_min: (resolution["width"].as_i64().unwrap() as i32)
                    .min(resolution["height"].as_i64().unwrap() as i32),
                initial_alternation: entry["alternation_plus_100_u8"].as_u64().unwrap() != 0,
                initial_time: times[0],
            },
        })
        .expect("captured V9 caller input/runtime must be supported by the test model")
    }

    fn number(value: &Value) -> f32 {
        value.as_f64().unwrap() as f32
    }

    // A bounded synthetic profile, not evidence for arbitrary strokes or paint.
    #[test]
    fn null_v1_screen_profiles_match_candidate_geometry() {
        let receipt: Value = serde_json::from_str(include_str!(
            "../../../../conformance/inkpen2-v1-screen-profiles-native.json"
        ))
        .unwrap();
        let points = [
            P { x: 0., y: 0. },
            P { x: 8., y: 0. },
            P { x: 16., y: 3. },
            P { x: 24., y: 3. },
        ];
        for (profile, native) in receipt["profiles"].as_object().unwrap() {
            let (width, height) = profile.split_once('x').unwrap();
            let resolution_min = width.parse::<i32>().unwrap().min(height.parse().unwrap());
            let output = super::redraw_v1_bounded(
                Input {
                    points: &points,
                    times: &[0, 10, 20, 30],
                    pressures: &[0.2, 0.4, 0.6, 0.8],
                    tools: &[2; 4],
                    rgb: [0; 3],
                    rainbow: None,
                    size: 4.,
                    curve_enabled: true,
                    is_shape: false,
                    runtime: Runtime {
                        inverse_scale: [1.; 2],
                        spacing: 1.,
                        tolerance: 2.,
                        tool_threshold: 20.,
                        resolution_min,
                        initial_alternation: false,
                        initial_time: 0,
                    },
                },
                10000,
            )
            .unwrap();
            let actual: Vec<u8> = output
                .vertices
                .iter()
                .flat_map(|v| v[..3].iter().flat_map(|f| f.to_le_bytes()))
                .collect();
            assert_eq!(
                actual,
                hex_bytes(native["stream_hex"].as_str().unwrap()),
                "{profile}"
            );
        }
    }

    #[test]
    fn null_v1_screen_shapes_match_candidate_geometry() {
        let shapes: Value = serde_json::from_str(include_str!(
            "../../../../conformance/inkpen2-null-v1-shapes-paint-native.json"
        ))
        .unwrap();
        let screens: Value = serde_json::from_str(include_str!(
            "../../../../conformance/inkpen2-v1-screen-paint-native.json"
        ))
        .unwrap();
        for (name, case) in shapes["cases"].as_object().unwrap() {
            let input = &case["input"];
            let points: Vec<P> = input["points"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| P {
                    x: number(&p[0]),
                    y: number(&p[1]),
                })
                .collect();
            for (screen, native) in screens["cases"][name].as_object().unwrap() {
                let (width, height) = screen.split_once('x').unwrap();
                let resolution_min = width.parse::<i32>().unwrap().min(height.parse().unwrap());
                let output = super::redraw_v1_bounded(
                    Input {
                        points: &points,
                        times: &[0, 10, 20, 30],
                        pressures: &[0.2, 0.4, 0.6, 0.8],
                        tools: &[2; 4],
                        rgb: [0; 3],
                        rainbow: None,
                        size: number(&input["size"]),
                        curve_enabled: true,
                        is_shape: false,
                        runtime: Runtime {
                            inverse_scale: [1.; 2],
                            spacing: 1.,
                            tolerance: 2.,
                            tool_threshold: 20.,
                            resolution_min,
                            initial_alternation: false,
                            initial_time: 0,
                        },
                    },
                    10000,
                )
                .unwrap();
                let actual: Vec<u8> = output
                    .vertices
                    .iter()
                    .flat_map(|v| v[..3].iter().flat_map(|f| f.to_le_bytes()))
                    .collect();
                assert_eq!(
                    actual,
                    hex_bytes(native["geometry_hex"].as_str().unwrap()),
                    "{name} {screen}"
                );
            }
        }
    }

    #[test]
    fn null_v1_controls_match_candidate_geometry() {
        let receipt: Value = serde_json::from_str(include_str!(
            "../../../../conformance/inkpen2-null-v1-controls-native.json"
        ))
        .unwrap();
        let boundaries: Value = serde_json::from_str(include_str!(
            "../../../../conformance/inkpen2-v1-saved-callback-boundaries-native.json"
        ))
        .unwrap();
        assert_eq!(receipt["cases"].as_object().unwrap().len(), 14);
        // Drawing did not create a pen for the one-point setter input, so
        // that observation does not validate direct V1 RedrawPen geometry.
        assert_eq!(
            receipt["route_rejections"]["single"]["native_result"]["factory_count"],
            0
        );
        for (name, case) in receipt["cases"].as_object().unwrap() {
            let i = &case["input"];
            let points: Vec<P> = i["points_f32_bits"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| P {
                    x: f32_from_le_hex(&p[0]),
                    y: f32_from_le_hex(&p[1]),
                })
                .collect();
            let times: Vec<i32> = i["timestamps_i32"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_i64().unwrap() as i32)
                .collect();
            let pressure: Vec<f32> = i["pressure_f32_bits"]
                .as_array()
                .unwrap()
                .iter()
                .map(f32_from_le_hex)
                .collect();
            let tools = vec![i["tool_type"].as_i64().unwrap() as i32; points.len()];
            let out = super::redraw_with_mode(
                Input {
                    points: &points,
                    times: &times,
                    pressures: &pressure,
                    tools: &tools,
                    rgb: [0; 3],
                    rainbow: None,
                    size: f32_from_le_hex(&i["pen_size_f32_bits"]),
                    // Both native curve flag values follow this curve in the captured profile.
                    curve_enabled: true,
                    is_shape: false,
                    runtime: Runtime {
                        inverse_scale: [1.; 2],
                        spacing: 1.,
                        tolerance: 2.,
                        tool_threshold: 20.,
                        resolution_min: 1080,
                        initial_alternation: false,
                        initial_time: times[0],
                    },
                },
                10000,
                true,
            )
            .unwrap();
            let actual: Vec<u8> = out
                .vertices
                .iter()
                .flat_map(|v| v[..3].iter().flat_map(|f| f.to_le_bytes()))
                .collect();
            let expected = hex_bytes(case["stream_hex"].as_str().unwrap());
            assert_eq!(actual, expected, "{name}");
            if let Some(native) = boundaries["cases"].get(name) {
                let calls = native["calls"].as_array().unwrap();
                assert_eq!(calls.len(), points.len() - 1, "{name}");
                assert_eq!(calls[0]["count"], 1, "{name}: initial stamp");
                for (index, call) in calls.iter().enumerate() {
                    let expected_method = if index + 1 == calls.len() {
                        "endPen"
                    } else {
                        "drawLine"
                    };
                    assert_eq!(call["method"], expected_method, "{name}");
                    if index > 0 {
                        assert_eq!(
                            out.sample_ends[index],
                            call["count"].as_u64().unwrap() as usize,
                            "{name}: movement boundary {index}"
                        );
                    }
                }
                assert_eq!(
                    out.terminal.start,
                    calls.last().unwrap()["count"].as_u64().unwrap() as usize,
                    "{name}: terminal boundary"
                );
                assert_eq!(
                    out.vertices.len(),
                    native["total"].as_u64().unwrap() as usize,
                    "{name}: final count"
                );
            }
        }
    }

    #[test]
    fn null_v1_extra_controls_match_candidate_geometry() {
        let receipt: Value = serde_json::from_str(include_str!(
            "../../../../conformance/inkpen2-v1-extra-controls-native.json"
        ))
        .unwrap();
        for (name, case) in receipt["cases"].as_object().unwrap() {
            let i = &case["input"];
            let points: Vec<P> = i["points_f32_bits"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| P {
                    x: f32_from_le_hex(&p[0]),
                    y: f32_from_le_hex(&p[1]),
                })
                .collect();
            let times: Vec<i32> = i["timestamps_i32"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_i64().unwrap() as i32)
                .collect();
            let pressure: Vec<f32> = i["pressure_f32_bits"]
                .as_array()
                .unwrap()
                .iter()
                .map(f32_from_le_hex)
                .collect();
            let tools = vec![i["tool_type"].as_i64().unwrap() as i32; points.len()];
            let out = super::redraw_with_mode(
                Input {
                    points: &points,
                    times: &times,
                    pressures: &pressure,
                    tools: &tools,
                    rgb: [0; 3],
                    rainbow: None,
                    size: f32_from_le_hex(&i["pen_size_f32_bits"]),
                    // Both native curve flag values follow this curve in the captured profile.
                    curve_enabled: true,
                    is_shape: false,
                    runtime: Runtime {
                        inverse_scale: [1.; 2],
                        spacing: 1.,
                        tolerance: 2.,
                        tool_threshold: 20.,
                        resolution_min: 1080,
                        initial_alternation: false,
                        initial_time: times[0],
                    },
                },
                10000,
                true,
            )
            .unwrap();
            let actual: Vec<u8> = out
                .vertices
                .iter()
                .flat_map(|v| v[..3].iter().flat_map(|f| f.to_le_bytes()))
                .collect();
            let expected = hex_bytes(case["stream_hex"].as_str().unwrap());
            assert_eq!(actual, expected, "{name}");
        }
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
                rgb: [0; 3],
                rainbow: None,
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
                    tool_threshold: 20.0,
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
    fn actual_drawing_routes_and_v9_callback_stream_match_numeric_capture() {
        let caller: Value = serde_json::from_str(include_str!(
            "../../../../conformance/inkpen2-saved-caller-native.json"
        ))
        .unwrap();
        let cases = caller["cases"].as_object().unwrap();
        assert_eq!(
            cases.len(),
            5,
            "native ObjectDrawing route coverage changed"
        );

        let null_v1 = &cases["route_control_null_v1"];
        assert_eq!(
            null_v1["native_calls"]["route"]["drawing_selected_drawable_version"],
            "1"
        );
        assert_eq!(
            null_v1["native_calls"]["selected_drawable_primary_vtable_id"],
            "InkPen2StrokeDrawableGLV1"
        );

        let empty = &cases["route_control_empty_v9"];
        let explicit = &cases["route_control_explicit9_v9"];
        let nontrivial = &cases["line0_curve_short_history_explicit9_v9"];
        let color = &cases["line0_curve_short_history_color_explicit9_v9"];
        let input = &nontrivial["input"];
        let numeric: Value =
            serde_json::from_str(include_str!("../../../../conformance/inkpen2-saved.json"))
                .unwrap();
        let numeric_case = &numeric["cases"][0];
        let caller_points = input["points_f32_bits"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|point| {
                point
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| floats_from_le_hex(value.as_str().unwrap())[0])
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let numeric_points = numeric_case["points"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|point| {
                point
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| value.as_f64().unwrap() as f32)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            caller_points
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>(),
            numeric_points
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>(),
            "actual ObjectStroke caller XY channels match the numeric oracle source"
        );
        assert_eq!(
            input["timestamps_i32"], numeric_case["timestamps"],
            "actual ObjectStroke caller time channels"
        );
        assert_eq!(input["tool_type"], 2);
        assert_eq!(color["input"]["color_argb_u32"], 0xff336699_u32);
        assert_eq!(
            color["native_calls"]["source_ObjectStroke_GetColor_u32"],
            0xff336699_u32
        );
        assert_eq!(
            color["native_calls"]["color_contract_validation"]["draw_object_stroke"]["constant_rgb_f32_bits"],
            serde_json::json!(["cdcc4c3e", "cdcccc3e", "9a99193f"])
        );
        assert_eq!(
            color["native_calls"]["color_contract_validation"]["saved_redraw"]["geometry_first_six_match_numeric_fixture"],
            true
        );
        let caller_pressures = input["pressure_f32_bits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| floats_from_le_hex(value.as_str().unwrap())[0])
            .collect::<Vec<_>>();
        let numeric_pressures = numeric_case["pressures"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_f64().unwrap() as f32)
            .collect::<Vec<_>>();
        assert_eq!(
            caller_pressures
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>(),
            numeric_pressures
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>(),
            "actual ObjectStroke caller pressure channels"
        );
        assert_eq!(
            input["timestamps_i32"].as_array().unwrap().len(),
            caller_points.len() / 2
        );
        assert_eq!(caller_pressures.len(), caller_points.len() / 2);
        assert_eq!(
            input["tilt_f32_bits"].as_array().unwrap().len(),
            caller_points.len() / 2
        );
        assert_eq!(
            input["orientation_f32_bits"].as_array().unwrap().len(),
            caller_points.len() / 2
        );
        assert_eq!(
            nontrivial["native_calls"]["drawable_getInverseScale_f32_bits"],
            serde_json::json!(["0000803f", "0000803f"])
        );
        assert_eq!(
            nontrivial["native_calls"]["drawable_getSize_f32_bits"],
            input["pen_size_f32_bits"]
        );
        assert_eq!(
            nontrivial["native_calls"]["pen_screen_resolution"][0]["width"],
            1440
        );
        assert_eq!(
            nontrivial["native_calls"]["pen_screen_resolution"][0]["height"],
            2560
        );
        let redraw_entries = nontrivial["native_calls"]["selected_drawable_redraw_entry_state"]
            .as_array()
            .unwrap();
        let drawing_entry = redraw_entries
            .iter()
            .find(|entry| entry["phase"] == "draw_object_stroke")
            .unwrap();
        let replay_entry = redraw_entries
            .iter()
            .find(|entry| entry["phase"] == "saved_redraw")
            .unwrap();
        assert_eq!(drawing_entry["tolerance_plus_44_f32_bits"], "00000040");
        assert_eq!(drawing_entry["residual_plus_92_f32_bits"], "00000000");
        assert_eq!(drawing_entry["alternation_plus_100_u8"], 0);
        assert_eq!(replay_entry["tolerance_plus_44_f32_bits"], "00000000");
        assert_eq!(replay_entry["residual_plus_92_f32_bits"], "409f383f");
        assert_eq!(replay_entry["alternation_plus_100_u8"], 1);
        for case in [empty, explicit, nontrivial, color] {
            assert_eq!(
                case["native_calls"]["route"]["drawing_selected_drawable_version"],
                "9"
            );
            assert_eq!(
                case["native_calls"]["selected_drawable_primary_vtable_id"],
                "InkPen2StrokeDrawableGLV9"
            );
            assert_eq!(
                case["native_calls"]["selected_drawable_object_redraw_return"],
                1
            );

            let phases: &[&str] = if case["case_id"] == nontrivial["case_id"]
                || case["case_id"] == color["case_id"]
            {
                &["draw_object_stroke", "saved_redraw"]
            } else {
                &["saved_redraw"]
            };
            for phase in phases {
                let method_entries = case["native_method_entries"].as_array().unwrap();
                let phase_entries = method_entries
                    .iter()
                    .filter(|entry| entry["phase"] == *phase)
                    .collect::<Vec<_>>();
                let end_pen = phase_entries
                    .iter()
                    .rposition(|entry| {
                        entry["signatures"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .any(|signature| signature.as_str().unwrap().contains("::endPen("))
                    })
                    .expect("selected V9 saved redraw must enter endPen");
                let terminal_callbacks = phase_entries[end_pen + 1..]
                    .iter()
                    .filter(|entry| {
                        entry["signatures"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .any(|signature| signature.as_str().unwrap().contains("::drawPoint("))
                    })
                    .count();

                let events = case["callback_buffer_events"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|event| event["phase"] == *phase)
                    .collect::<Vec<_>>();
                assert!(
                    !events.is_empty(),
                    "{}: V9 callback stream is empty",
                    case["case_id"]
                );
                let mut previous = Vec::new();
                for (index, event) in events.iter().enumerate() {
                    let before = event["before_bytes"].as_str().unwrap();
                    let after = event["after_bytes"].as_str().unwrap();
                    let delta = event["delta_bytes"].as_str().unwrap();
                    assert_eq!(
                        hex_bytes(before),
                        previous,
                        "{}: callback {index} before",
                        case["case_id"]
                    );
                    assert_eq!(
                        hex_bytes(delta).len(),
                        36,
                        "{}: callback {index} RTV3 record",
                        case["case_id"]
                    );
                    assert_eq!(
                        hex_bytes(after),
                        [previous.as_slice(), hex_bytes(delta).as_slice()].concat()
                    );
                    assert_eq!(
                        event["before_count_bytes"].as_u64().unwrap() as usize,
                        index * 36
                    );
                    assert_eq!(
                        event["after_count_bytes"].as_u64().unwrap() as usize,
                        (index + 1) * 36
                    );
                    previous = hex_bytes(after);
                }
                assert_eq!(previous.len(), events.len() * 36);
                assert!(terminal_callbacks > 0);
                let terminal_start = events.len() - terminal_callbacks;
                assert_eq!(
                    events[terminal_start]["before_count_bytes"]
                        .as_u64()
                        .unwrap() as usize,
                    terminal_start * 36,
                    "{}: callback stream must expose the endPen terminal boundary",
                    case["case_id"]
                );
                let model = redraw_from_native_caller(case, phase);
                assert_eq!(
                    model.vertices.len(),
                    events.len(),
                    "{} {phase}: modeled callback count",
                    case["case_id"]
                );
                assert_eq!(model.terminal.start, terminal_start);
                assert_eq!(model.terminal.end, events.len());
                assert_eq!(model.sample_ends.last(), Some(&terminal_start));
                let native_floats = floats_from_le_bytes(&previous);
                for (record_index, (actual, expected)) in model
                    .vertices
                    .iter()
                    .zip(native_floats.as_chunks::<9>().0)
                    .enumerate()
                {
                    for lane in 0..9 {
                        assert_eq!(
                            actual[lane].to_bits(),
                            expected[lane].to_bits(),
                            "{}: native caller record {record_index}, lane {lane}",
                            case["case_id"]
                        );
                    }
                }
            }
        }

        for phase in ["draw_object_stroke", "saved_redraw"] {
            let final_bytes = |case: &Value| {
                let event = case["callback_buffer_events"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .rfind(|event| event["phase"] == phase)
                    .unwrap();
                hex_bytes(event["after_bytes"].as_str().unwrap())
            };
            let colored = floats_from_le_bytes(&final_bytes(color));
            let black = floats_from_le_bytes(&final_bytes(nontrivial));
            assert_eq!(colored.len(), 25 * 9);
            assert_eq!(black.len(), colored.len());
            for (index, (colored, black)) in colored
                .as_chunks::<9>()
                .0
                .iter()
                .zip(black.as_chunks::<9>().0)
                .enumerate()
            {
                assert_eq!(
                    colored[..6]
                        .iter()
                        .map(|value| value.to_bits())
                        .collect::<Vec<_>>(),
                    black[..6]
                        .iter()
                        .map(|value| value.to_bits())
                        .collect::<Vec<_>>(),
                    "{phase}: colored record {index} geometry matches black native stream"
                );
                assert_eq!(colored[6].to_bits(), (51f32 / 255.).to_bits());
                assert_eq!(colored[7].to_bits(), (102f32 / 255.).to_bits());
                assert_eq!(colored[8].to_bits(), (153f32 / 255.).to_bits());
            }
        }

        let empty_bytes = cases["route_control_empty_v9"]["callback_buffer_events"]
            .as_array()
            .unwrap()
            .iter()
            .rfind(|event| event["phase"] == "saved_redraw")
            .unwrap()["after_bytes"]
            .as_str()
            .unwrap();
        let explicit_bytes = cases["route_control_explicit9_v9"]["callback_buffer_events"]
            .as_array()
            .unwrap()
            .iter()
            .rfind(|event| event["phase"] == "saved_redraw")
            .unwrap()["after_bytes"]
            .as_str()
            .unwrap();
        assert_eq!(
            empty_bytes, explicit_bytes,
            "empty and explicit V9 controls share the same saved input and output"
        );

        let captured =
            caller["cases"]["line0_curve_short_history_explicit9_v9"]["callback_buffer_events"]
                .as_array()
                .unwrap()
                .iter()
                .rfind(|event| event["phase"] == "saved_redraw")
                .unwrap();
        let actual = floats_from_le_hex(captured["after_bytes"].as_str().unwrap());
        let expected = numeric_case["native"]["rtv3"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|record| {
                record
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|lane| lane.as_f64().unwrap() as f32)
            })
            .collect::<Vec<_>>();
        assert_eq!(actual.len(), expected.len());
        for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
            assert_eq!(
                actual.to_bits(),
                expected.to_bits(),
                "caller RTV3 lane {index}"
            );
        }
        assert_eq!(actual.len(), 25 * 9);
        assert_eq!(captured["after_count_bytes"].as_u64().unwrap(), 900);
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
                rgb: [0; 3],
                rainbow: None,
                size: 4.,
                curve_enabled: false,
                is_shape: true,
                runtime: Runtime {
                    inverse_scale: [1., 1.],
                    spacing: 1.,
                    tolerance: 2.,
                    tool_threshold: 20.,
                    resolution_min: 1440,
                    initial_alternation: false,
                    initial_time: 0,
                },
            })
            .is_none()
        );
    }

    fn redraw_from_reuse_case(
        case: &Value,
        stroke_name: &str,
        phase: &str,
        initial_alternation: bool,
    ) -> super::Output {
        let stroke = &case["input"][stroke_name];
        let points: Vec<P> = if let Some(points) = stroke["points_f32_bits"].as_array() {
            points
                .iter()
                .map(|point| P {
                    x: f32_from_le_hex(&point[0]),
                    y: f32_from_le_hex(&point[1]),
                })
                .collect()
        } else {
            stroke["points"]
                .as_array()
                .unwrap()
                .iter()
                .map(|point| P {
                    x: number(&point[0]),
                    y: number(&point[1]),
                })
                .collect()
        };
        let times: Vec<i32> = stroke["timestamps_i32"]
            .as_array()
            .or_else(|| stroke["timestamps"].as_array())
            .unwrap()
            .iter()
            .map(|time| time.as_i64().unwrap() as i32)
            .collect();
        let tools = vec![stroke["tool_type"].as_i64().unwrap() as i32; points.len()];
        let pressures: Vec<f32> = if let Some(pressures) = stroke["pressure_f32_bits"].as_array() {
            pressures.iter().map(f32_from_le_hex).collect()
        } else {
            stroke["pressure"]
                .as_array()
                .unwrap()
                .iter()
                .map(number)
                .collect()
        };
        let native = &case["native"];
        let state = native["drawable_states"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["phase"] == phase)
            .unwrap();
        assert_eq!(
            state["entry_alternation_u8"].as_u64().unwrap() != 0,
            initial_alternation,
            "{phase}: caller-provided persistent field"
        );
        let drawline = native["drawline_runtime_entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["phase"] == phase)
            .unwrap();
        let resolution = &native["screen_resolution_calls"][0];
        let spacing = f32::from_le_bytes(
            hex_bytes(state["entry_fields_f32_bits"]["88"].as_str().unwrap())
                .try_into()
                .unwrap(),
        );
        let runtime = Runtime {
            inverse_scale: [1., 1.],
            spacing,
            tolerance: f32_from_le_hex(&drawline["plus44_tolerance_f32_bits"]),
            tool_threshold: f32_from_le_hex(&drawline["plus96_tool_threshold_f32_bits"]),
            resolution_min: (resolution["width"].as_i64().unwrap() as i32)
                .min(resolution["height"].as_i64().unwrap() as i32),
            initial_alternation,
            initial_time: times[0],
        };
        let actual = redraw(Input {
            points: &points,
            times: &times,
            tools: &tools,
            pressures: &pressures,
            rgb: [0; 3],
            rainbow: None,
            size: f32_from_le_hex(&case["input"]["first_stroke"]["size_f32_bits"]),
            curve_enabled: true,
            is_shape: false,
            runtime,
        })
        .expect("captured native InkPen2 sequence input must be supported");
        let stream = &native["streams"][phase];
        assert_eq!(
            actual.vertices.len(),
            stream["record_count"].as_u64().unwrap() as usize
        );
        let actual_bytes: Vec<u8> = actual
            .vertices
            .iter()
            .flat_map(|vertex| vertex.iter().flat_map(|value| value.to_le_bytes()))
            .collect();
        assert_eq!(
            actual_bytes,
            hex_bytes(stream["full_bytes_hex"].as_str().unwrap()),
            "{phase}"
        );
        assert_eq!(
            actual.final_alternation,
            state["return_alternation_u8"].as_u64().unwrap() != 0,
            "{phase}: retained native byte after redraw"
        );
        actual
    }

    fn output_bits(output: &super::Output) -> Vec<u32> {
        output
            .vertices
            .iter()
            .flat_map(|record| record.iter().map(|value| value.to_bits()))
            .collect()
    }

    #[test]
    fn saved_caller_reuse_carries_native_alternation_between_strokes() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/inkpen2-reuse-native.json"
        ))
        .unwrap();
        let cases = &fixture["cases"];
        let short = &cases["short_then_tool1_tool2"];
        let mut persistent = false;
        let _short_first =
            redraw_from_reuse_case(short, "first_stroke", "draw_object_stroke", persistent);
        persistent = _short_first.final_alternation;
        let short_tool1 =
            redraw_from_reuse_case(short, "baseline_tool1", "second_tool1_baseline", persistent);
        persistent = short_tool1.final_alternation;
        let short_tool2 =
            redraw_from_reuse_case(short, "baseline_tool2", "third_tool2_baseline", persistent);

        // Drawing retains the drawable across separately constructed source
        // objects too. Source-object identity is not a reset boundary.
        let distinct = &cases["distinct_objects_short_tool1_tool2"];
        persistent = false;
        for (input, phase, expected) in [
            ("first_stroke", "draw_object_stroke", &_short_first),
            ("baseline_tool1", "second_tool1_baseline", &short_tool1),
            ("baseline_tool2", "third_tool2_baseline", &short_tool2),
        ] {
            let actual = redraw_from_reuse_case(distinct, input, phase, persistent);
            assert_eq!(output_bits(&actual), output_bits(expected), "{phase}");
            persistent = actual.final_alternation;
        }

        let long = &cases["long_then_tool1_tool2"];
        persistent = false;
        let _long_first =
            redraw_from_reuse_case(long, "first_stroke", "draw_object_stroke", persistent);
        persistent = _long_first.final_alternation;
        let long_tool1 =
            redraw_from_reuse_case(long, "baseline_tool1", "second_tool1_baseline", persistent);
        persistent = long_tool1.final_alternation;
        let long_tool2 =
            redraw_from_reuse_case(long, "baseline_tool2", "third_tool2_baseline", persistent);

        let fresh_tool1 = redraw_from_reuse_case(
            &cases["fresh_tool1"],
            "first_stroke",
            "draw_object_stroke",
            false,
        );
        let fresh_tool2 = redraw_from_reuse_case(
            &cases["fresh_tool2"],
            "first_stroke",
            "draw_object_stroke",
            false,
        );
        assert_ne!(output_bits(&short_tool1), output_bits(&fresh_tool1));
        assert_eq!(output_bits(&long_tool1), output_bits(&fresh_tool1));
        assert_eq!(output_bits(&short_tool2), output_bits(&fresh_tool2));
        assert_eq!(output_bits(&long_tool2), output_bits(&fresh_tool2));
    }

    #[test]
    fn repeated_coincident_sample_commits_state_and_matches_native_solid_stream() {
        // Captured from the pinned InkPen2 V9 saved Drawing caller with a fresh
        // drawable: points (0,0),(0,0),(8,0), times 0,1,2, size 4, curve on,
        // tool 2, inverse scale 1, spacing 1, DrawLine tolerance 0, 1920x1080.
        // Native capture SHA-256: 30bf493d257960f1f898d887ff81e107050179f1f2539b9254dcd4d3bea742cb.
        // InkPen2 DSO SHA-256: 61806b1a7b11c89d0b9c8fa620ac7407d21eda9ac7555c280dbeb38076501c7c.
        // A zero-length first segment emits no stamp but still updates the
        // smoothing state used to size the following segment.
        let points = [
            P { x: 0.0, y: 0.0 },
            P { x: 0.0, y: 0.0 },
            P { x: 8.0, y: 0.0 },
        ];
        let times = [0, 1, 2];
        let tools = [2, 2, 2];
        let pressures = [0.5, 0.5, 0.5];
        let actual = redraw(Input {
            points: &points,
            times: &times,
            tools: &tools,
            pressures: &pressures,
            rgb: [0, 0, 0],
            rainbow: None,
            size: 4.0,
            curve_enabled: true,
            is_shape: false,
            runtime: Runtime {
                inverse_scale: [1.0, 1.0],
                spacing: 1.0,
                tolerance: 0.0,
                tool_threshold: 20.0,
                resolution_min: 1080,
                initial_alternation: false,
                initial_time: 0,
            },
        })
        .expect("native zero-length transition is supported");
        let expected: [[u32; 9]; 9] = [
            [
                0x3f7ffbf4, 0x00000000, 0x401a2e8b, 0x401a2e8b, 0x3e95bc60, 0x3f800000, 0, 0, 0,
            ],
            [
                0x3fe38c67, 0x00000000, 0x4019e059, 0x4019e059, 0x3e958660, 0x3f800000, 0, 0, 0,
            ],
            [
                0x4031c686, 0x00000000, 0x40199227, 0x40199227, 0x3e955029, 0x3f800000, 0, 0, 0,
            ],
            [
                0x407ffff4, 0x00000000, 0x401943f5, 0x401943f5, 0x3e9519bb, 0x3f800000, 0, 0, 0,
            ],
            [
                0x40a71c2a, 0x00000000, 0x4018f5c2, 0x4018f5c2, 0x3e94e314, 0x3f800000, 0, 0, 0,
            ],
            [
                0x40c71bfe, 0x00000000, 0x4018f5c2, 0x4018f5c2, 0x3e94e314, 0x3f800000, 0, 0, 0,
            ],
            [
                0x40dfff7f, 0x00000000, 0x4018f5c2, 0x4018f5c2, 0x3e94e314, 0x3f800000, 0, 0, 0,
            ],
            [
                0x41000000, 0x00000000, 0x4018f5c2, 0x4018f5c2, 0x3e94e314, 0x3f800000, 0, 0, 0,
            ],
            [
                0x41000000, 0x00000000, 0x4018f5c2, 0x4018f5c2, 0x3e94e314, 0x3f800000, 0, 0, 0,
            ],
        ];
        assert_eq!(actual.vertices.len(), expected.len());
        for (record, bits) in actual.vertices.iter().zip(expected) {
            assert_eq!(record.map(f32::to_bits), bits);
        }
    }

    fn rainbow_test_input<'a>(
        points: &'a [P],
        times: &'a [i32],
        tools: &'a [i32],
        pressures: &'a [f32],
        rgb: [u8; 3],
        rainbow: Option<Rainbow<'a>>,
    ) -> Input<'a> {
        Input {
            points,
            times,
            tools,
            pressures,
            rgb,
            rainbow,
            size: 4.0,
            curve_enabled: true,
            is_shape: false,
            runtime: Runtime {
                inverse_scale: [1.0, 1.0],
                spacing: 1.0,
                tolerance: 0.0,
                tool_threshold: 20.0,
                resolution_min: 1080,
                initial_alternation: false,
                initial_time: times[0],
            },
        }
    }

    #[test]
    fn rainbow_two_color_saved_stream_matches_native_bits() {
        // Native InkPen2 V9 saved Drawing caller, exact pins inherited from
        // the rainbow probe: InkPen2 61806b1a..., Model 4fbcf6d4...,
        // Drawing 788bf413..., Base e10da011.... Input is the captured
        // line0_curve_short_history case, D=5, offset=2.5, red/blue palette.
        // Native JSON capture SHA-256:
        // 64876f8246cda28cab6d6eb3592db3e9df02ff1477366b069b437f126c5f3a00.
        // Its plain-color companion geometry is independently checked by the
        // durable native saved-caller numeric fixture test above.
        let points = [
            P { x: 0.0, y: 0.0 },
            P { x: 8.0, y: 0.0 },
            P { x: 16.0, y: 3.0 },
            P { x: 24.0, y: 3.0 },
        ];
        let times = [0, 10, 20, 30];
        let tools = [2; 4];
        let pressures = [0.2, 0.4, 0.6, 0.8];
        let palette = [0xffff_0000, 0xff00_00ff];
        let plain = redraw(rainbow_test_input(
            &points, &times, &tools, &pressures, [0; 3], None,
        ))
        .unwrap();
        let actual = redraw(rainbow_test_input(
            &points,
            &times,
            &tools,
            &pressures,
            [0; 3],
            Some(Rainbow {
                distance: 5,
                offset: 2.5,
                palette_argb: &palette,
            }),
        ))
        .unwrap();
        assert_eq!(actual.vertices.len(), 25);
        let cycle = [
            [0x3ecc_cccc, 0x0000_0000, 0x3f19_999a],
            [0x3f4c_cccc, 0x0000_0000, 0x3e4c_ccd0],
            [0x3f4c_cccd, 0x0000_0000, 0x3e4c_cccd],
            [0x3ecc_cccc, 0x0000_0000, 0x3f19_999a],
            [0x0000_0000, 0x0000_0000, 0x3f80_0000],
        ];
        for (index, ((actual, plain), expected)) in actual
            .vertices
            .iter()
            .zip(&plain.vertices)
            .zip((0..25).map(|i| if i < 20 { cycle[i % 5] } else { cycle[4] }))
            .enumerate()
        {
            assert_eq!(
                actual[..6]
                    .iter()
                    .map(|value| value.to_bits())
                    .collect::<Vec<_>>(),
                plain[..6]
                    .iter()
                    .map(|value| value.to_bits())
                    .collect::<Vec<_>>(),
                "rainbow must preserve native geometry at record {index}"
            );
            assert_eq!(
                [
                    actual[6].to_bits(),
                    actual[7].to_bits(),
                    actual[8].to_bits(),
                ],
                expected,
                "record {index}"
            );
        }
    }

    #[test]
    fn rainbow_no_emission_terminal_uses_native_endpen_phase() {
        // Native V9 short-stroke branch (+376=1): (0,0)->(.5,0), size 4,
        // times 0/1, curve on, period 5, initial offset 2.5. The zero-stamp
        // movement advances Data+72 to 2.75; endPen computes fmodf(2.75,5)/5
        // = .55 before CalculateRainbowColor. Capture: InkPen2
        // 61806b1a7b11c89d0b9c8fa620ac7407d21eda9ac7555c280dbeb38076501c7c.
        let points = [P { x: 0.0, y: 0.0 }, P { x: 0.5, y: 0.0 }];
        let times = [0, 1];
        let tools = [2; 2];
        let pressures = [0.5; 2];
        let palette = [0xffff_0000, 0xff00_00ff];
        let actual = redraw(rainbow_test_input(
            &points,
            &times,
            &tools,
            &pressures,
            [0; 3],
            Some(Rainbow {
                distance: 5,
                offset: 2.5,
                palette_argb: &palette,
            }),
        ))
        .unwrap();
        assert_eq!(actual.vertices.len(), 1);
        assert_eq!(
            actual.vertices[0].map(f32::to_bits),
            [
                0x3f00_0000,
                0x0000_0000,
                0x3fba_2e8b,
                0x3fba_2e8b,
                0x3e1f_ffff,
                0x3f80_0000,
                0x3dcc_ccd0,
                0x0000_0000,
                0x3f66_6666,
            ]
        );
        assert_eq!(actual.terminal, 0..1);
    }

    #[test]
    fn rainbow_no_emission_negative_period_matches_native_fmod_phase() {
        // Native short-stroke saved caller: (0,0)->(.5,0), D=-5, offset=-1.
        // At endPen, offset is -.75; fmodf(-.75,-5)/-5=.15. The converted
        // palette index is zero, so this is in-bounds negative-period behavior.
        // Capture SHA-256 93ecb480d2d62e30b6575fccb6379322a4f0b6fbcb5fbcccc28bc18c8ef679a8.
        let points = [P { x: 0.0, y: 0.0 }, P { x: 0.5, y: 0.0 }];
        let times = [0, 1];
        let tools = [2; 2];
        let pressures = [0.5; 2];
        let palette = [0xffff_0000, 0xff00_00ff];
        let actual = redraw(rainbow_test_input(
            &points,
            &times,
            &tools,
            &pressures,
            [0; 3],
            Some(Rainbow {
                distance: -5,
                offset: -1.0,
                palette_argb: &palette,
            }),
        ))
        .unwrap();
        assert_eq!(
            actual.vertices[0].map(f32::to_bits),
            [
                0x3f00_0000,
                0,
                0x3fba_2e8b,
                0x3fba_2e8b,
                0x3e1f_ffff,
                0x3f80_0000,
                0x3f33_3333,
                0,
                0x3e99_999a,
            ]
        );
    }

    #[test]
    fn rainbow_native_palette_cardinality_and_empty_fallback() {
        let two = [0xffff_0000, 0xff00_00ff];
        let two_alpha = [0x00ff_0000, 0x7f00_00ff];
        let one = [0xff00_ff00];
        let three = [0xffff_0000, 0xff00_ff00, 0xff00_00ff];
        let one_color = Rainbow {
            distance: 5,
            offset: 2.5,
            palette_argb: &one,
        };
        let rgb = one_color.color_at(1.0, [0; 3], 0.0).unwrap();
        assert_eq!(rgb.map(f32::to_bits), [0, 0x3f80_0000, 0]);
        let three_color = Rainbow {
            distance: 5,
            offset: 4.0,
            palette_argb: &three,
        };
        let actual = three_color
            .color_at(1.0, [0; 3], three_color.offset)
            .unwrap();
        assert_eq!(actual.map(f32::to_bits), [0x3f80_0000, 0, 0]);
        let positive_phase = three_color
            .color_at(2.0, [0; 3], three_color.offset)
            .unwrap();
        assert_eq!(
            positive_phase.map(f32::to_bits),
            [0x3ecc_ccce, 0x3f19_9999, 0]
        );
        let regular = Rainbow {
            distance: 5,
            offset: 2.5,
            palette_argb: &two,
        }
        .color_at(1.0, [0; 3], 2.5)
        .unwrap();
        let alpha_changed = Rainbow {
            distance: 5,
            offset: 2.5,
            palette_argb: &two_alpha,
        }
        .color_at(1.0, [0; 3], 2.5)
        .unwrap();
        assert_eq!(regular.map(f32::to_bits), alpha_changed.map(f32::to_bits));
        let empty = Rainbow {
            distance: 0,
            offset: f32::NAN,
            palette_argb: &[],
        }
        .color_at(0.0, [51, 102, 153], f32::NAN)
        .unwrap();
        assert_eq!(
            empty.map(f32::to_bits),
            [
                (51.0f32 / 255.0).to_bits(),
                (102.0f32 / 255.0).to_bits(),
                (153.0f32 / 255.0).to_bits(),
            ]
        );
        let negative_period = Rainbow {
            distance: -5,
            offset: -1.0,
            palette_argb: &two,
        }
        .color_at(0.0, [0; 3], -1.0)
        .unwrap();
        assert_eq!(
            negative_period.map(f32::to_bits),
            [0x3f19_999a, 0, 0x3ecc_cccd]
        );
        // Native FCVTZS saturates before converting its index and upper
        // clamping; this real-caller capture used offset 1e20, D=5.
        let saturated = Rainbow {
            distance: 5,
            offset: 1.0e20,
            palette_argb: &two,
        }
        .color_at(1.0, [0; 3], 1.0e20)
        .unwrap();
        assert_eq!(saturated.map(f32::to_bits), [0x600a_c723, 0, 0xe00a_c723]);
        let negative_phase = Rainbow {
            distance: 5,
            offset: -0.25,
            palette_argb: &two,
        }
        .color_at(0.0, [0; 3], -0.25)
        .unwrap();
        assert_eq!(
            negative_phase.map(f32::to_bits),
            [0x3f8c_cccd, 0, 0xbdcc_cccd]
        );
        assert!(
            Rainbow {
                distance: 5,
                offset: -3.0,
                palette_argb: &two,
            }
            .color_at(0.0, [0; 3], -3.0)
            .is_none()
        );
    }

    #[test]
    fn rainbow_native_saved_caller_matrix_matches_full_vbo_hashes() {
        // Native matrix capture SHA-256: 8366510f0ca3e73c1fcd9970cfa79061b926b9eec8d05b9852be619d1ef68cf3.
        // Digests cover complete native Vector9<f32> little-endian buffers.
        // Runtime/getter values are captured per case; the 1920x1080 profile
        // is an explicit synthetic host context.
        struct Case {
            name: &'static str,
            points: &'static [[u32; 2]],
            times: &'static [i32],
            size: u32,
            curve: bool,
            distance: i32,
            offset: u32,
            palette: &'static [u32],
            records: usize,
            sha256: &'static str,
        }
        const RB: &[u32] = &[0xffff_0000, 0xff00_00ff];
        const RGB: &[u32] = &[0xffff_0000, 0xff00_ff00, 0xff00_00ff];
        const SEVEN: &[u32] = &[
            0xffff_0000,
            0xff00_ff00,
            0xff00_00ff,
            0xffff_ff00,
            0xffff_00ff,
            0xff00_ffff,
            0xffff_ffff,
        ];
        const CASES: &[Case] = &[
            Case {
                name: "short_negative_period",
                points: &[[0, 0], [0x3f00_0000, 0]],
                times: &[0, 1],
                size: 0x4080_0000,
                curve: true,
                distance: -5,
                offset: 0xbf80_0000,
                palette: RB,
                records: 1,
                sha256: "cf6beeeb5db11fd4cd1bc6fddef4c0bbac71b219cc117a0c8f4cd2691e4a9ab9",
            },
            Case {
                name: "repeat_zero_bend",
                points: &[[0, 0], [0, 0], [0x4040_0000, 0]],
                times: &[0, 1, 2],
                size: 0x4080_0000,
                curve: true,
                distance: 5,
                offset: 0x4020_0000,
                palette: RB,
                records: 4,
                sha256: "0daaed87ea54b096d0b9db520bebf789d78073e195dfad87a89ba1a129be5ef8",
            },
            Case {
                name: "subspacing_point1_n1",
                points: &[[0, 0], [0x3dcccccd, 0], [0x3f00_0000, 0]],
                times: &[0, 1, 2],
                size: 0x3ecccccd,
                curve: true,
                distance: 4,
                offset: 0x3e80_0000,
                palette: &[0xff00_ff00],
                records: 1,
                sha256: "bc3339b2df5afe8cf1212227c060f4682efafddf76da4ac3ad0ee43396612ed9",
            },
            Case {
                name: "subspacing_point5_n3_line",
                points: &[[0, 0], [0x3f00_0000, 0], [0x3f80_0000, 0x3f80_0000]],
                times: &[0, 0, 1],
                size: 0x3f00_0000,
                curve: false,
                distance: 1,
                offset: 0,
                palette: RGB,
                records: 2,
                sha256: "211d4175cd5c52f510fb97bb2df11b4d87d16863bded5dea400536fc0d9eaf22",
            },
            Case {
                name: "sharp_curve_n7",
                points: &[
                    [0, 0],
                    [0x3f00_0000, 0],
                    [0x3f00_0000, 0x3f00_0000],
                    [0, 0x3f00_0000],
                    [0x3e80_0000, 0x3e80_0000],
                    [0x3f80_0000, 0x3f80_0000],
                ],
                times: &[0, 0, 1, 1, 2, 1000],
                size: 0x4080_0000,
                curve: true,
                distance: 5,
                offset: 0x409f_f7cf,
                palette: SEVEN,
                records: 3,
                sha256: "dcabcc277fdf4130ded08eef5d49316ac885ebca1b2105309d0651b9e6182540",
            },
            Case {
                name: "sharp_line_n3",
                points: &[
                    [0, 0],
                    [0x3f00_0000, 0],
                    [0x3f00_0000, 0x3f00_0000],
                    [0, 0x3f00_0000],
                    [0x3e80_0000, 0x3e80_0000],
                    [0x3f80_0000, 0x3f80_0000],
                ],
                times: &[0, 1, 2, 3, 4, 5],
                size: 0x4080_0000,
                curve: false,
                distance: 4,
                offset: 0x3e80_0000,
                palette: RGB,
                records: 3,
                sha256: "7115c93058c5fafc6e501b99670b743151ad0e3b884da03c2c63727821862b5a",
            },
            Case {
                name: "zero_time_curve",
                points: &[[0, 0], [0x3f80_0000, 0], [0x3f80_0000, 0x3f80_0000]],
                times: &[0, 0, 0],
                size: 0x4080_0000,
                curve: true,
                distance: 5,
                offset: 0x3e80_0000,
                palette: RB,
                records: 2,
                sha256: "033c7c94f54f37f6325ffc21b654fc4acbf7022e99c09d0b329d5af1ea3bd346",
            },
            Case {
                name: "large_time_n7",
                points: &[
                    [0, 0],
                    [0x4100_0000, 0],
                    [0x4180_0000, 0x4040_0000],
                    [0x41c0_0000, 0x4040_0000],
                ],
                times: &[0, 1_000_000, 2_000_000, 3_000_000],
                size: 0x4080_0000,
                curve: false,
                distance: 5,
                offset: 0x4020_0000,
                palette: SEVEN,
                records: 25,
                sha256: "558017abf6afb0d8d41dd2362188656564e06c984543c60036ae4ef444eb025c",
            },
            Case {
                name: "repeated_bend_d1",
                points: &[
                    [0, 0],
                    [0x3f00_0000, 0],
                    [0x3f00_0000, 0],
                    [0x3f00_0000, 0x3f00_0000],
                    [0, 0x3f00_0000],
                    [0x3f80_0000, 0x3f80_0000],
                ],
                times: &[0, 1, 2, 3, 4, 5],
                size: 0x4080_0000,
                curve: true,
                distance: 1,
                offset: 0x3dcccccd,
                palette: RB,
                records: 3,
                sha256: "0e4089b295954014608d0ea528ec6bfe9acd0f4e39049c98823f94ce97f2e533",
            },
            Case {
                name: "period_boundary_n3",
                points: &[
                    [0, 0],
                    [0x4100_0000, 0],
                    [0x4180_0000, 0x4040_0000],
                    [0x41c0_0000, 0x4040_0000],
                ],
                times: &[0, 10, 20, 30],
                size: 0x4080_0000,
                curve: true,
                distance: 5,
                offset: 0x40a0_0000,
                palette: RGB,
                records: 25,
                sha256: "ade65740ce645e5368de44cf982d03329d759bcaffe3272457dfc3a4c9aac47c",
            },
            Case {
                name: "small_negative_phase",
                points: &[[0, 0], [0x3f00_0000, 0]],
                times: &[0, 1],
                size: 0x4080_0000,
                curve: true,
                distance: 5,
                offset: 0xbe80_0000,
                palette: RB,
                records: 1,
                sha256: "f7c01ed96ce0eaeb15d8ea8378c85d477c262a3c88b808e8456664929145023f",
            },
            Case {
                name: "empty_palette_d0",
                points: &[[0, 0], [0x4100_0000, 0], [0x4180_0000, 0x4040_0000]],
                times: &[0, 1, 2],
                size: 0x4080_0000,
                curve: false,
                distance: 0,
                offset: 0x3e80_0000,
                palette: &[],
                records: 17,
                sha256: "bc25e1a3170d73f1d17c07674f4af721a56de1b083c2ea852179b515c033c06e",
            },
        ];
        fn digest(bytes: &[u8]) -> String {
            use sha2::{Digest, Sha256};
            Sha256::digest(bytes)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect()
        }
        let mut total = 0;
        for case in CASES {
            assert_eq!(
                case.points.len(),
                case.times.len(),
                "{} source lengths",
                case.name
            );
            let points: Vec<P> = case
                .points
                .iter()
                .map(|[x, y]| P {
                    x: f32::from_bits(*x),
                    y: f32::from_bits(*y),
                })
                .collect();
            let tools = vec![2; points.len()];
            let pressures = vec![0.5; points.len()];
            let output = redraw(Input {
                points: &points,
                times: case.times,
                tools: &tools,
                pressures: &pressures,
                rgb: [0; 3],
                rainbow: Some(Rainbow {
                    distance: case.distance,
                    offset: f32::from_bits(case.offset),
                    palette_argb: case.palette,
                }),
                size: f32::from_bits(case.size),
                curve_enabled: case.curve,
                is_shape: false,
                runtime: Runtime {
                    inverse_scale: [1.0, 1.0],
                    spacing: 1.0,
                    tolerance: 0.0,
                    tool_threshold: 20.0,
                    resolution_min: 1080,
                    initial_alternation: false,
                    initial_time: case.times[0],
                },
            })
            .unwrap_or_else(|| panic!("{} native input rejected", case.name));
            assert_eq!(
                output.vertices.len(),
                case.records,
                "{} record count",
                case.name
            );
            let bytes: Vec<u8> = output
                .vertices
                .iter()
                .flat_map(|record| record.iter().flat_map(|value| value.to_le_bytes()))
                .collect();
            assert_eq!(
                digest(&bytes),
                case.sha256,
                "{} full Vector9<f32> stream",
                case.name
            );
            total += output.vertices.len();
        }
        assert_eq!(CASES.len(), 12);
        assert_eq!(total, 87);
    }
}
