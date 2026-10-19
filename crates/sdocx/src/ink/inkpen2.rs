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
    pub tool_threshold: f32,
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
    /// Source RGB from ObjectStroke::GetColor, without the packed alpha byte.
    pub rgb: [u8; 3],
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
        let rgb = self.input.rgb.map(|channel| f32::from(channel) / 255.);
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

    fn hex_bytes(value: &str) -> Vec<u8> {
        assert!(value.len().is_multiple_of(2));
        value
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect()
    }

    fn floats_from_le_hex(value: &str) -> Vec<f32> {
        hex_bytes(value)
            .chunks_exact(4)
            .map(|bytes| f32::from_le_bytes(bytes.try_into().unwrap()))
            .collect()
    }

    fn floats_from_le_bytes(value: &[u8]) -> Vec<f32> {
        value
            .chunks_exact(4)
            .map(|bytes| f32::from_le_bytes(bytes.try_into().unwrap()))
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
                    .zip(native_floats.chunks_exact(9))
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
                    .filter(|event| event["phase"] == phase)
                    .last()
                    .unwrap();
                hex_bytes(event["after_bytes"].as_str().unwrap())
            };
            let colored = floats_from_le_bytes(&final_bytes(color));
            let black = floats_from_le_bytes(&final_bytes(nontrivial));
            assert_eq!(colored.len(), 25 * 9);
            assert_eq!(black.len(), colored.len());
            for (index, (colored, black)) in colored
                .chunks_exact(9)
                .zip(black.chunks_exact(9))
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
            .filter(|event| event["phase"] == "saved_redraw")
            .last()
            .unwrap()["after_bytes"]
            .as_str()
            .unwrap();
        let explicit_bytes = cases["route_control_explicit9_v9"]["callback_buffer_events"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|event| event["phase"] == "saved_redraw")
            .last()
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
                .filter(|event| event["phase"] == "saved_redraw")
                .last()
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
}
