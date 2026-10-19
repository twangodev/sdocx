//! Typed numeric core for the selected Montblanc calligraphy V1 path.
//!
//! This module is intentionally not connected to production pen dispatch.
//! Inputs distinguish saved history from drawable state retained by the
//! reused native object.

use super::path::{P, Quad};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct NibState {
    pub(super) angle_degrees: f32,
    /// Retained drawable members +276/+280 in native order (sine, cosine).
    /// `drawPoint` writes these into Vector4 in reverse order.
    pub(super) direction: [f32; 2],
    pub(super) initial_pending: bool,
    pub(super) movement_armed: bool,
}

/// Saved-history state consumed by the historical RedrawPen loop. The final
/// MotionEvent and endPen path are intentionally supplied separately.
#[derive(Clone, Copy, Debug)]
pub(super) struct SavedHistoryState {
    pub(super) nib: NibState,
    pub(super) accepted: P,
    pub(super) start: P,
    pub(super) control: P,
    /// Retained drawable midpoint (+104/+108), updated by cubic endPen.
    pub(super) midpoint: P,
}

#[derive(Clone, Debug)]
pub(super) struct SavedHistoryOutput {
    /// Vector4(X, Y, cos(angle), sin(angle)) calls emitted by history redraw.
    pub(super) vectors: Vec<[f32; 4]>,
    /// Exclusive emitted-vector count after each supplied historical point.
    pub(super) sample_ends: Vec<usize>,
    pub(super) state: SavedHistoryState,
}

#[derive(Clone, Debug)]
pub(super) struct EndPenOutput {
    pub(super) vectors: Vec<[f32; 4]>,
    /// Absent when native endPen takes the initial-point branch and builds no path.
    pub(super) path_length: Option<f32>,
    pub(super) point_extent: f32,
    pub(super) state: SavedHistoryState,
}

/// Required saved ObjectStroke channels. Optional stylus arrays are kept as
/// supplied; Base MotionEvent pairs tilt and orientation, zeroing both unless
/// both arrays exist.
pub(super) struct SavedObjectStrokeInput<'a> {
    pub(super) points: &'a [[f32; 2]],
    pub(super) pressures: &'a [f32],
    pub(super) timestamps: &'a [i32],
    pub(super) tilt: Option<&'a [f32]>,
    pub(super) orientation: Option<&'a [f32]>,
    pub(super) tool_type: i32,
    pub(super) pen_size: f32,
}

/// Result of adapting one saved Model stroke through the MotionEvent history
/// and terminal event split used by the Calligraphy GLV1 wrapper.
pub(super) struct SavedObjectStrokeOutput {
    pub(super) history: Option<SavedHistoryOutput>,
    pub(super) terminal: Option<EndPenOutput>,
    /// Combined actual AddPoint input records, with the common point extent.
    pub(super) vectors: Vec<[f32; 4]>,
    pub(super) extents: Vec<f32>,
    pub(super) final_state: SavedHistoryState,
    pub(super) event_time_ms: i64,
    pub(super) event_pressure: f32,
    pub(super) event_tilt: f32,
    pub(super) event_orientation: f32,
    pub(super) tool_type: i32,
}

/// Adapt the saved object's source arrays to Base MotionEvent history plus its
/// separate terminal event. Callers must supply the drawable's retained state;
/// this function does not assume every saved record starts from a new drawable.
pub(super) fn saved_object_stroke_redraw(
    input: SavedObjectStrokeInput<'_>,
    retained: SavedHistoryState,
) -> Option<SavedObjectStrokeOutput> {
    let count = input.points.len();
    if count == 0
        || input.pressures.len() != count
        || input.timestamps.len() != count
        || input.tilt.is_some_and(|values| values.len() != count)
        || input
            .orientation
            .is_some_and(|values| values.len() != count)
        || !input.pen_size.is_finite()
        || input.pen_size < 0.0
    {
        return None;
    }
    let paired_stylus = input.tilt.is_some() && input.orientation.is_some();
    let event_tilt = if paired_stylus {
        input.tilt?.last().copied()?
    } else {
        0.0
    };
    let event_orientation = if paired_stylus {
        input.orientation?.last().copied()?
    } else {
        0.0
    };
    let event_time_ms = i64::from(*input.timestamps.last()?) - i64::from(input.timestamps[0]);
    let history = if count > 1 {
        let history_rows: Vec<[f32; 3]> = input.points[..count - 1]
            .iter()
            .enumerate()
            .map(|(index, point)| {
                let orientation = if paired_stylus {
                    input.orientation.expect("paired optional channel")[index]
                } else {
                    0.0
                };
                [point[0], point[1], orientation]
            })
            .collect();
        let prepared = prepare_saved_history(&history_rows, retained)?;
        Some(saved_history_vectors(&history_rows, prepared)?)
    } else {
        None
    };
    let terminal = if let Some(history) = &history {
        Some(terminal_end_pen_vectors(
            history.state,
            P {
                x: input.points[count - 1][0],
                y: input.points[count - 1][1],
            },
            input.pen_size,
        )?)
    } else {
        None
    };
    let mut vectors = Vec::new();
    if let Some(history) = &history {
        vectors.extend(history.vectors.iter().copied());
    }
    if let Some(terminal) = &terminal {
        vectors.extend(terminal.vectors.iter().copied());
    }
    let extent = (input.pen_size * 0.5).max(1.0);
    let extents = vec![extent; vectors.len()];
    let final_state = terminal
        .as_ref()
        .map_or(retained, |terminal| terminal.state);
    Some(SavedObjectStrokeOutput {
        history,
        terminal,
        vectors,
        extents,
        final_state,
        event_time_ms,
        event_pressure: *input.pressures.last()?,
        event_tilt,
        event_orientation,
        tool_type: input.tool_type,
    })
}

/// Apply RedrawPen's per-invocation initialization before its historical
/// sample loop. The drawable retains nib controls/orientation across reuse,
/// while accepted/start coordinates and the two lifecycle flags are reset
/// from the first saved record.
pub(super) fn prepare_saved_history(
    history: &[[f32; 3]],
    mut state: SavedHistoryState,
) -> Option<SavedHistoryState> {
    let first = history.first()?;
    state.accepted = P {
        x: first[0],
        y: first[1],
    };
    state.start = state.accepted;
    state.nib.initial_pending = true;
    state.nib.movement_armed = true;
    Some(state)
}

/// Reconstruct the separate terminal endPen sampling loop from retained
/// drawable state and the final event endpoint. Its repeat scale uses full
/// pen size; each emitted drawPoint uses half size and retains nib direction.
pub(super) fn terminal_end_pen_vectors(
    mut state: SavedHistoryState,
    endpoint: P,
    pen_size: f32,
) -> Option<EndPenOutput> {
    if !pen_size.is_finite() || pen_size < 0.0 {
        return None;
    }
    if state.nib.initial_pending {
        return Some(EndPenOutput {
            vectors: vec![[
                state.accepted.x,
                state.accepted.y,
                state.nib.direction[1],
                state.nib.direction[0],
            ]],
            path_length: None,
            point_extent: (pen_size * 0.5).max(1.0),
            state,
        });
    }
    let midpoint = if state.nib.movement_armed {
        // Native endPen replaces the retained midpoint with the terminal
        // control/end midpoint before cubic subdivision.
        let midpoint = state.control.mid(endpoint);
        state.midpoint = midpoint;
        Some(midpoint)
    } else {
        None
    };
    let path = if state.nib.movement_armed {
        let midpoint = midpoint?;
        TerminalPath::Cubic(TerminalCubic::new([
            state.start,
            state.control,
            midpoint,
            endpoint,
        ]))
    } else {
        TerminalPath::Quadratic(Quad::new(state.start, state.control, endpoint))
    };
    let length = path.length();
    if !length.is_finite() || length < 0.0 {
        return None;
    }
    let count = repeat_count(length, pen_size)?;
    let mut vectors = Vec::new();
    let mut distance = 0.0_f32;
    let step = length / count as f32;
    for _ in 0..count {
        // SmPath::getPosTan returns false for an empty subdivision list.
        if length == 0.0 {
            break;
        }
        let position = path.at(distance)?;
        vectors.push([
            position.x,
            position.y,
            state.nib.direction[1],
            state.nib.direction[0],
        ]);
        distance += step;
    }
    Some(EndPenOutput {
        vectors,
        path_length: Some(length),
        point_extent: (pen_size * 0.5).max(1.0),
        state,
    })
}

enum TerminalPath {
    Quadratic(Quad),
    Cubic(TerminalCubic),
}

impl TerminalPath {
    fn length(&self) -> f32 {
        match self {
            Self::Quadratic(path) => path.length,
            Self::Cubic(path) => path.length,
        }
    }

    fn at(&self, distance: f32) -> Option<P> {
        match self {
            Self::Quadratic(path) if path.length > 0.0 => Some(path.at(distance)),
            Self::Cubic(path) if !path.segments.is_empty() => Some(path.at(distance)),
            _ => None,
        }
    }
}

/// Base SmPath cubic subdivision used by GLV1 endPen.
struct TerminalCubic {
    points: [P; 4],
    segments: Vec<(f32, u16)>,
    length: f32,
}

impl TerminalCubic {
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
        let deviation_one = (p[1].x - one.x).abs().max((p[1].y - one.y).abs());
        let deviation_two = (p[2].x - two.x).abs().max((p[2].y - two.y).abs());
        if t1 - t0 >= 1024 && (deviation_one > 0.5 || deviation_two > 0.5) {
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
        let index = self
            .segments
            .partition_point(|&(end, _)| end < distance)
            .min(self.segments.len() - 1);
        let (start, t0) = if index == 0 {
            (0.0, 0)
        } else {
            self.segments[index - 1]
        };
        let (end, t1) = self.segments[index];
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

/// Reconstruct the finite history-to-AddPoint sampling stage. `history` starts
/// with the event's first historical record; the native loop skips that first
/// record and consumes the rest. The current event/endPen path is separate.
pub(super) fn saved_history_vectors(
    history: &[[f32; 3]],
    mut state: SavedHistoryState,
) -> Option<SavedHistoryOutput> {
    if history.is_empty() {
        return None;
    }
    let mut output = SavedHistoryOutput {
        vectors: Vec::new(),
        sample_ends: Vec::with_capacity(history.len().saturating_sub(1)),
        state,
    };
    for &[x, y, orientation] in history.iter().skip(1) {
        let current = P { x, y };
        let dx = (x - state.accepted.x).abs();
        let dy = (y - state.accepted.y).abs();
        if dx < 1.5 && dy < 1.5 {
            output.sample_ends.push(output.vectors.len());
            continue;
        }
        if !state.nib.movement_armed {
            state.nib.movement_armed = true;
        } else if state.nib.initial_pending {
            state.control = current;
            state.nib.angle_degrees = orientation_target(orientation);
            state.nib.initial_pending = false;
            state.nib.movement_armed = false;
        } else {
            let midpoint = state.control.mid(current);
            let path = Quad::new(state.start, state.control, midpoint);
            if path.length.is_finite() && path.length >= 0.0 {
                let target = orientation_target(orientation);
                let (next_angle, repeat_angle) = advance_angle(state.nib.angle_degrees, target);
                let count = repeat_count(path.length, repeat_angle)?;
                let distance_step = path.length / count as f32;
                let angle_step = (next_angle - state.nib.angle_degrees) / count as f32;
                let mut distance = 0.0_f32;
                let mut angle = state.nib.angle_degrees;
                let mut last_direction = state.nib.direction;
                if path.length == 0.0 {
                    // Inferred from RedrawLine control flow: sincos runs once,
                    // empty SmPath makes getPosTan fail, and midpoint
                    // drawPoint remains.
                    last_direction = direction_for_angle(angle);
                } else {
                    for _ in 0..count {
                        if distance > path.length {
                            break;
                        }
                        let position = path.at(distance);
                        last_direction = direction_for_angle(angle);
                        output.vectors.push([
                            position.x,
                            position.y,
                            last_direction[0],
                            last_direction[1],
                        ]);
                        state.nib.direction = [last_direction[1], last_direction[0]];
                        state.start = position;
                        distance += distance_step;
                        angle += angle_step;
                    }
                }
                output
                    .vectors
                    .push([midpoint.x, midpoint.y, last_direction[0], last_direction[1]]);
                state.nib.angle_degrees = next_angle;
                state.nib.direction = [last_direction[1], last_direction[0]];
            }
            state.control = current;
            state.nib.movement_armed = false;
        }
        state.accepted = current;
        output.sample_ends.push(output.vectors.len());
    }
    // Native redrawLine epilogue (0x1ec0c..0x1ec28) copies its accepted
    // position at +124 into both midpoint (+104) and path start (+88).
    state.midpoint = state.start;
    output.state = state;
    Some(output)
}

fn direction_for_angle(angle_degrees: f32) -> [f32; 2] {
    let radians = angle_degrees as f64 * std::f64::consts::PI / 180.0;
    [radians.cos() as f32, radians.sin() as f32]
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct MeshRect {
    /// Four ordered corners from the native AddPoint call.
    pub(super) corners: [[f32; 2]; 4],
}

/// Numeric inputs read by one RTV1 `AddPoint` call. These include its separate
/// near-zero gate center, initial-edge flag, and three cached edge pairs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct AddPointInput {
    pub(super) vector: [f32; 4],
    pub(super) extent: f32,
    pub(super) gate_center: [f32; 2],
    pub(super) initial_edge: bool,
    pub(super) previous_edges: [[[f32; 2]; 2]; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct AddPointGeometry {
    pub(super) buffers: [MeshBuffer; 5],
    pub(super) rectangles: [MeshRect; 5],
    pub(super) next_edges: [[[f32; 2]; 2]; 3],
}

/// Reconstruct RTV1::AddPoint's gate and its core/fringe Rect arguments.
/// Previous centers and the cached fringe edges are explicit runtime inputs.
pub(super) fn add_point(input: AddPointInput) -> Option<AddPointGeometry> {
    let [x, y, signed_x, signed_y] = input.vector;
    let extent = input.extent.max(1.0);
    if !input.initial_edge {
        let dx = x - input.gate_center[0];
        let dy = y - input.gate_center[1];
        let distance_squared = dx.mul_add(dx, dy * dy);
        if distance_squared.sqrt() < f32::from_bits(0x3400_0000) {
            return None;
        }
    }
    let current = [
        [(-signed_x).mul_add(extent, x), signed_y.mul_add(extent, y)],
        [signed_x.mul_add(extent, x), (-signed_y).mul_add(extent, y)],
    ];
    let previous_core = if input.initial_edge {
        current
    } else {
        input.previous_edges[0]
    };
    let (normal, tangent) = if input.initial_edge {
        let diagonal = f32::from_bits(0x4010_d067);
        ([diagonal, diagonal], [diagonal, diagonal])
    } else {
        let dx = x - input.gate_center[0];
        let dy = y - input.gate_center[1];
        let normal_length = dy.mul_add(dy, dx * dx).sqrt();
        let tangent_length = dx.mul_add(dx, dy * dy).sqrt();
        let normal_inverse = 1.0_f32 / normal_length;
        let tangent_inverse = 1.0_f32 / tangent_length;
        let scale = 3.2_f32;
        (
            [
                (-dy * normal_inverse) * scale,
                (dx * normal_inverse) * scale,
            ],
            [
                (-dx * tangent_inverse) * scale,
                (-dy * tangent_inverse) * scale,
            ],
        )
    };
    let upper = current[0];
    let lower = current[1];
    let upper_plus = [upper[0] + normal[0], upper[1] + normal[1]];
    let upper_minus = [upper[0] - normal[0], upper[1] - normal[1]];
    let lower_minus = [lower[0] - normal[0], lower[1] - normal[1]];
    let lower_plus = [lower[0] + normal[0], lower[1] + normal[1]];
    let upper_tangent_minus = [upper[0] - tangent[0], upper[1] - tangent[1]];
    let upper_tangent_plus = [upper[0] + tangent[0], upper[1] + tangent[1]];
    let lower_tangent_minus = [lower[0] - tangent[0], lower[1] - tangent[1]];
    let lower_tangent_plus = [lower[0] + tangent[0], lower[1] + tangent[1]];
    let previous_lower = if input.initial_edge {
        [lower_minus, lower_plus]
    } else {
        input.previous_edges[1]
    };
    let previous_upper = if input.initial_edge {
        [upper_plus, upper_minus]
    } else {
        input.previous_edges[2]
    };
    Some(AddPointGeometry {
        buffers: [
            MeshBuffer::Core,
            MeshBuffer::FringeB,
            MeshBuffer::FringeB,
            MeshBuffer::FringeA,
            MeshBuffer::FringeA,
        ],
        rectangles: [
            MeshRect {
                corners: [previous_core[0], previous_core[1], current[0], current[1]],
            },
            MeshRect {
                corners: [
                    previous_upper[0],
                    previous_upper[1],
                    upper_plus,
                    upper_minus,
                ],
            },
            MeshRect {
                corners: [
                    previous_lower[0],
                    previous_lower[1],
                    lower_minus,
                    lower_plus,
                ],
            },
            MeshRect {
                corners: [upper_minus, upper_plus, lower_minus, lower_plus],
            },
            MeshRect {
                corners: [
                    upper_tangent_minus,
                    upper_tangent_plus,
                    lower_tangent_minus,
                    lower_tangent_plus,
                ],
            },
        ],
        next_edges: [
            current,
            [lower_minus, lower_plus],
            [upper_plus, upper_minus],
        ],
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MeshBuffer {
    Core,
    FringeA,
    FringeB,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct MeshVertex {
    pub(super) position: [f32; 2],
    pub(super) coverage: [f32; 2],
}

/// Exact six-vertex sequence appended by one native RTV1::buildMesh call.
/// The constants are two-component shader coverage values, not alpha opacity.
pub(super) fn build_mesh(rect: MeshRect) -> [MeshVertex; 6] {
    let [a, b, c, d] = rect.corners;
    [
        MeshVertex {
            position: b,
            coverage: [0.0, 0.0],
        },
        MeshVertex {
            position: a,
            coverage: [0.0, 1.0],
        },
        MeshVertex {
            position: d,
            coverage: [1.0, 0.0],
        },
        MeshVertex {
            position: a,
            coverage: [0.0, 1.0],
        },
        MeshVertex {
            position: d,
            coverage: [1.0, 0.0],
        },
        MeshVertex {
            position: c,
            coverage: [1.0, 1.0],
        },
    ]
}

/// Three output buffers produced by the callback, in the order returned by
/// the native RTV1 object. Their different append counts are part of the
/// captured contract: core gets one Rect per AddPoint, each fringe gets two.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct MeshBuffers {
    pub(super) core: Vec<MeshVertex>,
    pub(super) fringe_a: Vec<MeshVertex>,
    pub(super) fringe_b: Vec<MeshVertex>,
}

impl MeshBuffers {
    pub(super) fn append(&mut self, buffer: MeshBuffer, rect: MeshRect) {
        let output = match buffer {
            MeshBuffer::Core => &mut self.core,
            MeshBuffer::FringeA => &mut self.fringe_a,
            MeshBuffer::FringeB => &mut self.fringe_b,
        };
        output.extend(build_mesh(rect));
    }
}

/// Test bridge from the captured saved coordinate/orientation history through
/// the sampler, AddPoint state, mesh builder, and RTV1 Vector4 lanes. Pressure,
/// timestamps, stylus channels, and model admission are outside this fixture.
#[cfg(test)]
pub(super) fn saved_native_mesh_bridge() -> Option<[Vec<[f32; 4]>; 3]> {
    let draw: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/calligraphy-draw-native.json"
    ))
    .ok()?;
    let case = draw["cases"]
        .as_array()?
        .iter()
        .find(|case| case["input"]["saved_wrapper_case"] == "three-samples")?;
    let history: Vec<[f32; 3]> = case["input"]["history_xy_orientation"]
        .as_array()?
        .iter()
        .map(|row| {
            row.as_array()?
                .iter()
                .map(|value| value.as_f64().map(|value| value as f32))
                .collect::<Option<Vec<_>>>()?
                .try_into()
                .ok()
        })
        .collect::<Option<Vec<_>>>()?;
    let points: Vec<[f32; 2]> = history.iter().map(|row| [row[0], row[1]]).collect();
    let orientations: Vec<f32> = history.iter().map(|row| row[2]).collect();
    let pressures = vec![0.5; points.len()];
    let timestamps: Vec<i32> = (0..points.len() as i32).collect();
    let zero = P { x: 0.0, y: 0.0 };
    let output = saved_object_stroke_redraw(
        SavedObjectStrokeInput {
            points: &points,
            pressures: &pressures,
            timestamps: &timestamps,
            tilt: Some(&vec![0.0; points.len()]),
            orientation: Some(&orientations),
            tool_type: 2,
            pen_size: case["input"]["pen_size"].as_f64()? as f32,
        },
        SavedHistoryState {
            nib: NibState::fresh(),
            accepted: zero,
            start: zero,
            control: zero,
            midpoint: zero,
        },
    )?;

    let captured_add_points = case["saved_redraw"]["native_add_points"].as_array()?;
    if captured_add_points.len() != output.vectors.len() {
        return None;
    }
    let bits = |hex: &str| -> Option<u32> {
        let bytes: Vec<u8> = hex
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok())
            .collect::<Option<Vec<_>>>()?;
        Some(u32::from_le_bytes(bytes.try_into().ok()?))
    };
    for (index, (vector, extent)) in output.vectors.iter().zip(&output.extents).enumerate() {
        let captured = &captured_add_points[index];
        let expected_vector: [u32; 4] = captured["vector_bits"]
            .as_array()?
            .iter()
            .map(|value| bits(value.as_str()?))
            .collect::<Option<Vec<_>>>()?
            .try_into()
            .ok()?;
        if vector.map(f32::to_bits) != expected_vector
            || extent.to_bits() != bits(captured["extent_bits"].as_str()?)?
        {
            return None;
        }
    }

    let mut center = [0.0_f32; 2];
    let mut initial_edge = true;
    let mut previous_edges = [[[0.0_f32; 2]; 2]; 3];
    let mut buffers = MeshBuffers::default();
    let native_builds = case["saved_redraw"]["native_build_rectangles"].as_array()?;
    let mut build_cursor = 0;
    for (index, (vector, extent)) in output.vectors.iter().zip(&output.extents).enumerate() {
        let captured_add = &captured_add_points[index];
        let snapshot = &captured_add["pre_state"];
        let expected_center: [u32; 2] = snapshot["center_bits"]
            .as_array()?
            .iter()
            .map(|value| bits(value.as_str()?))
            .collect::<Option<Vec<_>>>()?
            .try_into()
            .ok()?;
        let expected_edges: Vec<u32> = snapshot["edge_state_bits"]
            .as_array()?
            .iter()
            .map(|value| bits(value.as_str()?))
            .collect::<Option<Vec<_>>>()?;
        let actual_edges: Vec<u32> = previous_edges
            .iter()
            .flat_map(|pair| pair.iter().flat_map(|point| point.iter().copied()))
            .map(f32::to_bits)
            .collect();
        if center.map(f32::to_bits) != expected_center
            || initial_edge != (snapshot["initial_flag"].as_u64()? != 0)
            || actual_edges != expected_edges
        {
            return None;
        }
        let geometry = add_point(AddPointInput {
            vector: *vector,
            extent: *extent,
            gate_center: center,
            initial_edge,
            previous_edges,
        });
        let expected_build_count = captured_add["build_calls"].as_u64()? as usize;
        if geometry.is_some() != (expected_build_count == 5) {
            return None;
        }
        if let Some(geometry) = geometry {
            for slot in 0..5 {
                let rect = geometry.rectangles[slot];
                let native = &native_builds[build_cursor + slot];
                let expected_rect: Vec<u32> = native["rect_bits"]
                    .as_array()?
                    .iter()
                    .map(|value| bits(value.as_str()?))
                    .collect::<Option<Vec<_>>>()?;
                let [a, b, c, d] = rect.corners;
                let actual_rect =
                    [a[0], a[1], b[0], b[1], c[0], c[1], d[0], d[1]].map(f32::to_bits);
                let expected_buffer = match geometry.buffers[slot] {
                    MeshBuffer::Core => "mesh_200",
                    MeshBuffer::FringeA => "mesh_208",
                    MeshBuffer::FringeB => "mesh_216",
                };
                if actual_rect.as_slice() != expected_rect.as_slice()
                    || native["buffer"] != expected_buffer
                {
                    return None;
                }
                buffers.append(geometry.buffers[slot], rect);
            }
            build_cursor += 5;
            previous_edges = geometry.next_edges;
        }
        center = [vector[0], vector[1]];
        initial_edge = false;
    }
    if build_cursor != native_builds.len() {
        return None;
    }
    let lanes = |vertices: Vec<MeshVertex>| {
        vertices
            .into_iter()
            .map(|vertex| {
                [
                    vertex.position[0],
                    vertex.position[1],
                    vertex.coverage[0],
                    vertex.coverage[1],
                ]
            })
            .collect()
    };
    Some([
        lanes(buffers.core),
        lanes(buffers.fringe_a),
        lanes(buffers.fringe_b),
    ])
}

impl NibState {
    /// The native V1 constructor initializes these once. A cached drawable can
    /// retain this state between records; callers must not silently recreate it.
    pub(super) const fn fresh() -> Self {
        Self {
            angle_degrees: 0.0,
            direction: [1.0, 0.0],
            initial_pending: true,
            movement_armed: true,
        }
    }
}

/// Apply the finite orientation conversion and fold used by redrawLine.
/// Multiplication rounds as f32 before widening for the division by pi.
pub(super) fn orientation_target(orientation_radians: f32) -> f32 {
    let degrees = (orientation_radians * 180.0_f32) as f64 / std::f64::consts::PI;
    let angle = degrees as f32;
    if (0.0..=180.0).contains(&angle) {
        90.0 - angle
    } else if (-180.0..0.0).contains(&angle) {
        -90.0 - angle
    } else {
        0.0
    }
}

/// Advance the smoothed nib angle and return the separate angle value passed
/// to getRepeat. The native helper uses min(old, next) for that argument.
pub(super) fn advance_angle(old: f32, target: f32) -> (f32, f32) {
    let next = if (old - target).abs() > 1.0 {
        let step = if target > old { 1.0 } else { -1.0 };
        let lower = if old > 0.0 { 0.0 } else { -90.0 };
        (old + step).max(lower)
    } else if old < target {
        target
    } else {
        old
    };
    (next, old.min(next))
}

/// Native getRepeat's piecewise scale uses its second numeric argument. The
/// saved redrawLine caller passes min(old_angle, next_angle); drawPoint extent
/// is computed independently from half the pen size.
pub(super) fn repeat_count(path_length: f32, second_argument: f32) -> Option<u32> {
    if !path_length.is_finite() || !second_argument.is_finite() {
        return None;
    }
    let scaled = if second_argument <= 5.0 {
        path_length * 0.5
    } else if second_argument <= 10.0 {
        path_length / 3.0
    } else if second_argument <= 15.0 {
        path_length * 0.25
    } else if second_argument <= 20.0 {
        path_length / 5.0
    } else if second_argument <= 30.0 {
        path_length * 0.125
    } else if second_argument <= 40.0 {
        path_length / 7.0
    } else if second_argument <= 55.0 {
        path_length / 9.0
    } else {
        path_length / 10.0
    };
    if scaled < 0.0 || scaled >= i32::MAX as f32 {
        return None;
    }
    Some((scaled.trunc() as u32).max(2) + 10)
}

#[cfg(test)]
mod tests {
    use super::{
        AddPointInput, MeshBuffer, MeshBuffers, MeshRect, NibState, SavedHistoryState, add_point,
        advance_angle, orientation_target, prepare_saved_history, repeat_count,
        saved_history_vectors, terminal_end_pen_vectors,
    };
    use crate::ink::path::P;

    fn bits(hex: &str) -> u32 {
        u32::from_le_bytes(
            hex.as_bytes()
                .chunks_exact(2)
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
        )
    }

    fn mesh_bits(vertices: &[super::MeshVertex]) -> Vec<u32> {
        vertices
            .iter()
            .flat_map(|vertex| {
                [
                    vertex.position[0],
                    vertex.position[1],
                    vertex.coverage[0],
                    vertex.coverage[1],
                ]
            })
            .map(f32::to_bits)
            .collect()
    }

    fn rect_bits(rect: MeshRect) -> [u32; 8] {
        let [a, b, c, d] = rect.corners;
        [a[0], a[1], b[0], b[1], c[0], c[1], d[0], d[1]].map(f32::to_bits)
    }

    fn captured_history_state(value: &serde_json::Value) -> SavedHistoryState {
        let pair = |name: &str| -> [f32; 2] {
            value[name]
                .as_array()
                .unwrap()
                .iter()
                .map(|word| f32::from_bits(bits(word.as_str().unwrap())))
                .collect::<Vec<_>>()
                .try_into()
                .unwrap()
        };
        SavedHistoryState {
            nib: NibState {
                angle_degrees: f32::from_bits(bits(value["angle_bits"].as_str().unwrap())),
                direction: pair("direction_bits"),
                initial_pending: value["flags"]["initial"].as_u64().unwrap() != 0,
                movement_armed: value["flags"]["movement_armed"].as_u64().unwrap() != 0,
            },
            accepted: P {
                x: pair("accepted_bits")[0],
                y: pair("accepted_bits")[1],
            },
            start: P {
                x: pair("path_start_bits")[0],
                y: pair("path_start_bits")[1],
            },
            control: P {
                x: pair("control_bits")[0],
                y: pair("control_bits")[1],
            },
            midpoint: P {
                x: pair("path_mid_bits")[0],
                y: pair("path_mid_bits")[1],
            },
        }
    }

    fn saved_history_state_bits(
        state: SavedHistoryState,
    ) -> (
        [u32; 2],
        [u32; 2],
        [u32; 2],
        [u32; 2],
        u32,
        [u32; 2],
        bool,
        bool,
    ) {
        (
            [state.accepted.x.to_bits(), state.accepted.y.to_bits()],
            [state.start.x.to_bits(), state.start.y.to_bits()],
            [state.control.x.to_bits(), state.control.y.to_bits()],
            [state.midpoint.x.to_bits(), state.midpoint.y.to_bits()],
            state.nib.angle_degrees.to_bits(),
            state.nib.direction.map(f32::to_bits),
            state.nib.initial_pending,
            state.nib.movement_armed,
        )
    }

    fn assert_saved_state_matches(
        state: SavedHistoryState,
        expected: &serde_json::Value,
        label: &str,
    ) {
        assert_eq!(
            saved_history_state_bits(state),
            saved_history_state_bits(captured_history_state(expected)),
            "{label}"
        );
    }

    fn replay_saved_object(
        rows_value: &serde_json::Value,
        tilt_value: &serde_json::Value,
        orientation_value: &serde_json::Value,
        retained: SavedHistoryState,
        tool_type: i32,
    ) -> super::SavedObjectStrokeOutput {
        let rows = rows_value.as_array().unwrap();
        let mut points = Vec::with_capacity(rows.len());
        let mut pressures = Vec::with_capacity(rows.len());
        let mut timestamps = Vec::with_capacity(rows.len());
        for row in rows {
            let values = row.as_array().unwrap();
            points.push([
                values[0].as_f64().unwrap() as f32,
                values[1].as_f64().unwrap() as f32,
            ]);
            pressures.push(values[2].as_f64().unwrap() as f32);
            timestamps.push(values[3].as_i64().unwrap() as i32);
        }
        let optional_values = |value: &serde_json::Value| -> Option<Vec<f32>> {
            if value.is_null() {
                None
            } else {
                Some(
                    value
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|item| item.as_f64().unwrap() as f32)
                        .collect(),
                )
            }
        };
        let tilt = optional_values(tilt_value);
        let orientation = optional_values(orientation_value);
        super::saved_object_stroke_redraw(
            super::SavedObjectStrokeInput {
                points: &points,
                pressures: &pressures,
                timestamps: &timestamps,
                tilt: tilt.as_deref(),
                orientation: orientation.as_deref(),
                tool_type,
                // Captured GLV1 PenData settings size; this stays caller data,
                // never inferred from stroke pressure or coordinates.
                pen_size: 6.0,
            },
            retained,
        )
        .expect("valid captured saved ObjectStroke inputs")
    }

    #[test]
    fn native_saved_history_witness_locks_orientation_rounding_and_fresh_state() {
        // `calligraphy-saved-native.json`, produced by executing pinned APK
        // GLV1 RedrawPen through endPen. The 0.35 rad history target is exact.
        assert_eq!(orientation_target(0.35).to_bits(), 0x428b_e499);
        assert_eq!(orientation_target(0.0).to_bits(), 90.0_f32.to_bits());
        assert_eq!(NibState::fresh().angle_degrees.to_bits(), 0);
        assert_eq!(
            NibState::fresh().direction.map(f32::to_bits),
            [1.0_f32.to_bits(), 0]
        );
    }

    #[test]
    fn angle_step_and_repeat_use_the_native_second_argument() {
        let (next, repeat_angle) = advance_angle(69.94648, 49.892956);
        assert_eq!(next.to_bits(), 68.94648_f32.to_bits());
        assert_eq!(repeat_angle.to_bits(), next.to_bits());
        assert_eq!(repeat_count(12.0, 3.0), Some(16));
        assert_eq!(repeat_count(12.0, 6.0), Some(14));
        assert_eq!(repeat_count(12.0, 14.0), Some(13));
    }

    #[test]
    fn captured_build_mesh_rectangles_reproduce_all_native_buffers() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../conformance/calligraphy-saved-native.json"
        ))
        .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            for (run_name, run) in case["native"].as_object().unwrap() {
                let calls = run["build_calls"].as_array().unwrap();
                let mut mesh = MeshBuffers::default();
                for call in calls {
                    let corners: Vec<f32> = call["rect_bits"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|value| f32::from_bits(bits(value.as_str().unwrap())))
                        .collect();
                    let rect = MeshRect {
                        corners: [
                            [corners[0], corners[1]],
                            [corners[2], corners[3]],
                            [corners[4], corners[5]],
                            [corners[6], corners[7]],
                        ],
                    };
                    let buffer = match call["buffer"].as_str().unwrap() {
                        "mesh_200" => MeshBuffer::Core,
                        "mesh_208" => MeshBuffer::FringeA,
                        "mesh_216" => MeshBuffer::FringeB,
                        label => panic!("unknown native output buffer {label}"),
                    };
                    mesh.append(buffer, rect);
                }
                for (label, actual) in [
                    ("mesh_200", mesh_bits(&mesh.core)),
                    ("mesh_208", mesh_bits(&mesh.fringe_a)),
                    ("mesh_216", mesh_bits(&mesh.fringe_b)),
                ] {
                    let expected: Vec<u32> = run["meshes"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|item| item["buffer"] == label)
                        .unwrap()["vertex_f32_bits"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|value| bits(value.as_str().unwrap()))
                        .collect();
                    assert_eq!(actual, expected, "{} {run_name} {label}", case["id"]);
                }
            }
        }
    }

    #[test]
    fn captured_addpoints_reconstruct_core_edges_across_reused_drawable() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../conformance/calligraphy-saved-native.json"
        ))
        .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            for (_run_name, run) in case["native"].as_object().unwrap() {
                let stamps = run["add_points"].as_array().unwrap();
                let all_build_calls = run["build_calls"].as_array().unwrap();
                let mut build_cursor = 0;
                for (index, stamp) in stamps.iter().enumerate() {
                    let vector: [f32; 4] = stamp["vector_bits"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|value| f32::from_bits(bits(value.as_str().unwrap())))
                        .collect::<Vec<_>>()
                        .try_into()
                        .unwrap();
                    let extent = f32::from_bits(bits(stamp["extent_bits"].as_str().unwrap()));
                    let pre = &stamp["pre_state"];
                    let center: [f32; 2] = pre["center_bits"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|value| f32::from_bits(bits(value.as_str().unwrap())))
                        .collect::<Vec<_>>()
                        .try_into()
                        .unwrap();
                    let edge: Vec<f32> = pre["edge_state_bits"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|value| f32::from_bits(bits(value.as_str().unwrap())))
                        .collect();
                    let previous_edges: [[[f32; 2]; 2]; 3] = edge
                        .chunks_exact(4)
                        .map(|v| [[v[0], v[1]], [v[2], v[3]]])
                        .collect::<Vec<_>>()
                        .try_into()
                        .unwrap();
                    let input = AddPointInput {
                        vector,
                        extent,
                        gate_center: center,
                        initial_edge: pre["initial_flag"].as_u64().unwrap() != 0,
                        previous_edges,
                    };
                    let actual = add_point(input);
                    let expected_builds = stamp["build_calls"].as_u64().unwrap() as usize;
                    assert_eq!(
                        actual.is_some(),
                        expected_builds == 5,
                        "{} {} AddPoint {index} emission gate",
                        case["id"],
                        _run_name
                    );
                    if let Some(geometry) = actual {
                        assert_eq!(expected_builds, 5);
                        for slot in 0..5 {
                            let native_rect = &all_build_calls[build_cursor + slot];
                            let buffer = match geometry.buffers[slot] {
                                MeshBuffer::Core => "mesh_200",
                                MeshBuffer::FringeA => "mesh_208",
                                MeshBuffer::FringeB => "mesh_216",
                            };
                            assert_eq!(native_rect["buffer"], buffer);
                            let expected: [u32; 8] = native_rect["rect_bits"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .map(|value| bits(value.as_str().unwrap()))
                                .collect::<Vec<_>>()
                                .try_into()
                                .unwrap();
                            if case["id"] == "five_history_phase_boundaries"
                                && index > 0
                                && slot == 0
                            {
                                let cached_core: [u32; 4] = previous_edges[0]
                                    .iter()
                                    .flat_map(|point| point.iter().copied())
                                    .map(f32::to_bits)
                                    .collect::<Vec<_>>()
                                    .try_into()
                                    .unwrap();
                                assert_eq!(&expected[..4], cached_core);
                                let current_core = [
                                    (-vector[2]).mul_add(extent, vector[0]),
                                    vector[3].mul_add(extent, vector[1]),
                                    vector[2].mul_add(extent, vector[0]),
                                    (-vector[3]).mul_add(extent, vector[1]),
                                ]
                                .map(f32::to_bits);
                                assert_ne!(cached_core, current_core);
                            }
                            assert_eq!(
                                rect_bits(geometry.rectangles[slot]),
                                expected,
                                "{} {} AddPoint {index} rect {slot}",
                                case["id"],
                                _run_name
                            );
                        }
                        if let Some(next_stamp) = stamps.get(index + 1) {
                            let next_edges: Vec<u32> = next_stamp["pre_state"]["edge_state_bits"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .map(|value| bits(value.as_str().unwrap()))
                                .collect();
                            let actual_edges: Vec<u32> = geometry
                                .next_edges
                                .iter()
                                .flat_map(|pair| {
                                    pair.iter().flat_map(|point| point.iter().copied())
                                })
                                .map(f32::to_bits)
                                .collect();
                            assert_eq!(
                                actual_edges, next_edges,
                                "{} {} AddPoint {index} edge cache",
                                case["id"], _run_name
                            );
                        }
                    } else {
                        assert_eq!(expected_builds, 0);
                    }
                    build_cursor += expected_builds;
                }
                assert_eq!(build_cursor, all_build_calls.len());
            }
        }
    }

    #[test]
    fn direct_native_half_extent_is_clamped_by_addpoint() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../conformance/calligraphy-addpoint-direct-native.json"
        ))
        .unwrap();
        let native = &fixture["case"]["native"];
        let extent = f32::from_bits(bits(native["extent_bits"].as_str().unwrap()));
        assert_eq!(extent.to_bits(), 0.5_f32.to_bits());
        let vector: [f32; 4] = native["vector_bits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| f32::from_bits(bits(value.as_str().unwrap())))
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        let pre = &native["pre_state"];
        let center: [f32; 2] = pre["center_bits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| f32::from_bits(bits(value.as_str().unwrap())))
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        let edge: Vec<f32> = pre["edge_state_bits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| f32::from_bits(bits(value.as_str().unwrap())))
            .collect();
        let previous_edges: [[[f32; 2]; 2]; 3] = edge
            .chunks_exact(4)
            .map(|v| [[v[0], v[1]], [v[2], v[3]]])
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        let actual = add_point(AddPointInput {
            vector,
            extent,
            gate_center: center,
            initial_edge: pre["initial_flag"].as_u64().unwrap() != 0,
            previous_edges,
        })
        .unwrap();
        let rects = native["rectangles"].as_array().unwrap();
        assert_eq!(actual.rectangles.len(), rects.len());
        for (slot, (actual_rect, native_rect)) in actual.rectangles.iter().zip(rects).enumerate() {
            let expected: [u32; 8] = native_rect["rect_bits"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| bits(value.as_str().unwrap()))
                .collect::<Vec<_>>()
                .try_into()
                .unwrap();
            assert_eq!(
                rect_bits(*actual_rect),
                expected,
                "direct AddPoint Rect {slot}"
            );
            let buffer = match actual.buffers[slot] {
                MeshBuffer::Core => "mesh_200",
                MeshBuffer::FringeA => "mesh_208",
                MeshBuffer::FringeB => "mesh_216",
            };
            assert_eq!(native_rect["buffer"], buffer);
        }
        assert_eq!(
            actual.rectangles[0].corners[2][1].to_bits(),
            1.0_f32.to_bits()
        );
        assert_ne!(
            actual.rectangles[0].corners[2][1].to_bits(),
            0.5_f32.to_bits()
        );
    }

    #[test]
    fn five_history_native_samples_replay_from_saved_points_and_fresh_state() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../conformance/calligraphy-saved-native.json"
        ))
        .unwrap();
        let case = fixture["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|case| case["id"] == "five_history_phase_boundaries")
            .unwrap();
        let history: Vec<[f32; 3]> = case["history"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                row.as_array()
                    .unwrap()
                    .iter()
                    .map(|value| value.as_f64().unwrap() as f32)
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap()
            })
            .collect();
        let zero = P { x: 0.0, y: 0.0 };
        let actual = saved_history_vectors(
            &history,
            SavedHistoryState {
                nib: NibState::fresh(),
                accepted: zero,
                start: zero,
                control: zero,
                midpoint: zero,
            },
        )
        .unwrap();
        let native = &case["native"]["fresh"];
        let expected_ends: Vec<usize> = native["sample_ends"]
            .as_array()
            .unwrap()
            .iter()
            .map(|sample| sample["add_point_end"].as_u64().unwrap() as usize)
            .collect();
        assert_eq!(actual.sample_ends, expected_ends);
        let expected: Vec<[u32; 4]> = native["add_points"]
            .as_array()
            .unwrap()
            .iter()
            .take(*expected_ends.last().unwrap())
            .map(|stamp| {
                stamp["vector_bits"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| bits(value.as_str().unwrap()))
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap()
            })
            .collect();
        let actual_bits: Vec<[u32; 4]> = actual
            .vectors
            .iter()
            .map(|vector| vector.map(f32::to_bits))
            .collect();
        assert_eq!(actual_bits, expected);
    }

    #[test]
    fn captured_saved_history_replays_from_fresh_and_reused_drawable_state() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../conformance/calligraphy-saved-native.json"
        ))
        .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            for (run_name, run) in case["native"].as_object().unwrap() {
                let history_key = if run_name == "reused" {
                    "reuse_history"
                } else {
                    "history"
                };
                let history: Vec<[f32; 3]> = case[history_key]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|row| {
                        row.as_array()
                            .unwrap()
                            .iter()
                            .map(|value| value.as_f64().unwrap() as f32)
                            .collect::<Vec<_>>()
                            .try_into()
                            .unwrap()
                    })
                    .collect();
                let states = run["drawable_states"].as_array().unwrap();
                let entry = states
                    .iter()
                    .find(|state| state["phase"] == "redraw_entry")
                    .unwrap();
                let read_pair = |name: &str| -> [f32; 2] {
                    entry[name]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|value| f32::from_bits(bits(value.as_str().unwrap())))
                        .collect::<Vec<_>>()
                        .try_into()
                        .unwrap()
                };
                let accepted = read_pair("accepted_bits");
                let start = read_pair("path_start_bits");
                let control = read_pair("control_bits");
                let midpoint = read_pair("path_mid_bits");
                let flags = &entry["flags"];
                let redraw_state = SavedHistoryState {
                    nib: NibState {
                        angle_degrees: f32::from_bits(bits(entry["angle_bits"].as_str().unwrap())),
                        direction: read_pair("direction_bits"),
                        initial_pending: flags["initial"].as_u64().unwrap() != 0,
                        movement_armed: flags["movement_armed"].as_u64().unwrap() != 0,
                    },
                    accepted: P {
                        x: accepted[0],
                        y: accepted[1],
                    },
                    start: P {
                        x: start[0],
                        y: start[1],
                    },
                    control: P {
                        x: control[0],
                        y: control[1],
                    },
                    midpoint: P {
                        x: midpoint[0],
                        y: midpoint[1],
                    },
                };
                let prepared = prepare_saved_history(&history, redraw_state).unwrap();
                let read_state = |state: &serde_json::Value| SavedHistoryState {
                    nib: NibState {
                        angle_degrees: f32::from_bits(bits(state["angle_bits"].as_str().unwrap())),
                        direction: state["direction_bits"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|value| f32::from_bits(bits(value.as_str().unwrap())))
                            .collect::<Vec<_>>()
                            .try_into()
                            .unwrap(),
                        initial_pending: state["flags"]["initial"].as_u64().unwrap() != 0,
                        movement_armed: state["flags"]["movement_armed"].as_u64().unwrap() != 0,
                    },
                    accepted: P {
                        x: f32::from_bits(bits(state["accepted_bits"][0].as_str().unwrap())),
                        y: f32::from_bits(bits(state["accepted_bits"][1].as_str().unwrap())),
                    },
                    start: P {
                        x: f32::from_bits(bits(state["path_start_bits"][0].as_str().unwrap())),
                        y: f32::from_bits(bits(state["path_start_bits"][1].as_str().unwrap())),
                    },
                    control: P {
                        x: f32::from_bits(bits(state["control_bits"][0].as_str().unwrap())),
                        y: f32::from_bits(bits(state["control_bits"][1].as_str().unwrap())),
                    },
                    midpoint: P {
                        x: f32::from_bits(bits(state["path_mid_bits"][0].as_str().unwrap())),
                        y: f32::from_bits(bits(state["path_mid_bits"][1].as_str().unwrap())),
                    },
                };
                let state_bits = |state: SavedHistoryState| {
                    (
                        [state.accepted.x.to_bits(), state.accepted.y.to_bits()],
                        [state.start.x.to_bits(), state.start.y.to_bits()],
                        [state.control.x.to_bits(), state.control.y.to_bits()],
                        [state.midpoint.x.to_bits(), state.midpoint.y.to_bits()],
                        state.nib.angle_degrees.to_bits(),
                        state.nib.direction.map(f32::to_bits),
                        state.nib.initial_pending,
                        state.nib.movement_armed,
                    )
                };
                if let Some(loop_entry) = states
                    .iter()
                    .find(|state| state["phase"] == "history_loop_entry")
                {
                    assert_eq!(
                        state_bits(prepared),
                        state_bits(read_state(loop_entry)),
                        "{} {run_name} RedrawPen initialization",
                        case["id"]
                    );
                }
                let actual = saved_history_vectors(&history, prepared).unwrap();
                let expected_ends: Vec<usize> = run["sample_ends"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|sample| sample["add_point_end"].as_u64().unwrap() as usize)
                    .collect();
                assert_eq!(
                    actual.sample_ends, expected_ends,
                    "{} {run_name} saved sample boundaries",
                    case["id"]
                );
                let terminal_start = run["terminal_start"].as_u64().unwrap() as usize;
                let expected: Vec<[u32; 4]> = run["add_points"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .take(terminal_start)
                    .map(|stamp| {
                        stamp["vector_bits"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|value| bits(value.as_str().unwrap()))
                            .collect::<Vec<_>>()
                            .try_into()
                            .unwrap()
                    })
                    .collect();
                let actual_bits: Vec<[u32; 4]> = actual
                    .vectors
                    .iter()
                    .map(|vector| vector.map(f32::to_bits))
                    .collect();
                assert_eq!(
                    actual_bits, expected,
                    "{} {run_name} saved history AddPoint vectors",
                    case["id"]
                );

                // Link the emitted history stage to the state endPen actually
                // receives on this same RedrawPen invocation.
                let Some(end_state) = run["drawable_states"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|state| state["phase"] == "end_pen_entry")
                else {
                    panic!("{} {run_name} missing endPen snapshot", case["id"]);
                };
                let bits_pair = |state: &serde_json::Value, name: &str| -> [u32; 2] {
                    state[name]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|value| bits(value.as_str().unwrap()))
                        .collect::<Vec<_>>()
                        .try_into()
                        .unwrap()
                };
                let state_bits_pair = |p: P| [p.x.to_bits(), p.y.to_bits()];
                assert_eq!(
                    state_bits_pair(actual.state.accepted),
                    bits_pair(end_state, "accepted_bits"),
                    "{} {run_name} history accepted state",
                    case["id"]
                );
                assert_eq!(
                    state_bits_pair(actual.state.start),
                    bits_pair(end_state, "path_start_bits"),
                    "{} {run_name} history start state",
                    case["id"]
                );
                assert_eq!(
                    state_bits_pair(actual.state.control),
                    bits_pair(end_state, "control_bits"),
                    "{} {run_name} history control state",
                    case["id"]
                );
                assert_eq!(
                    actual.state.nib.angle_degrees.to_bits(),
                    bits(end_state["angle_bits"].as_str().unwrap()),
                    "{} {run_name} history angle state",
                    case["id"]
                );
                assert_eq!(
                    actual.state.nib.direction.map(f32::to_bits),
                    bits_pair(end_state, "direction_bits"),
                    "{} {run_name} history direction state",
                    case["id"]
                );
                assert_eq!(
                    actual.state.nib.initial_pending,
                    end_state["flags"]["initial"].as_u64().unwrap() != 0,
                    "{} {run_name} history initial flag",
                    case["id"]
                );
                assert_eq!(
                    actual.state.nib.movement_armed,
                    end_state["flags"]["movement_armed"].as_u64().unwrap() != 0,
                    "{} {run_name} history movement flag",
                    case["id"]
                );

                let endpoint = bits_pair(end_state, "event_end_bits").map(f32::from_bits);
                let pen_size = f32::from_bits(bits(end_state["size_bits"].as_str().unwrap()));
                let terminal = terminal_end_pen_vectors(
                    actual.state,
                    P {
                        x: endpoint[0],
                        y: endpoint[1],
                    },
                    pen_size,
                )
                .unwrap();
                let combined: Vec<[u32; 4]> = actual
                    .vectors
                    .iter()
                    .chain(terminal.vectors.iter())
                    .map(|vector| vector.map(f32::to_bits))
                    .collect();
                let native_combined: Vec<[u32; 4]> = run["add_points"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|stamp| {
                        stamp["vector_bits"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|value| bits(value.as_str().unwrap()))
                            .collect::<Vec<_>>()
                            .try_into()
                            .unwrap()
                    })
                    .collect();
                assert_eq!(
                    combined, native_combined,
                    "{} {run_name} linked history plus endPen AddPoint vectors",
                    case["id"]
                );
            }
        }
    }

    #[test]
    fn saved_object_wrapper_inputs_replay_actual_history_and_terminal_calls() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../conformance/calligraphy-saved-wrapper-native.json"
        ))
        .unwrap();
        assert_eq!(fixture["schema"], "sdocx-calligraphy-saved-wrapper-v1");

        let compare_run = |output: &super::SavedObjectStrokeOutput,
                           expected: &serde_json::Value,
                           states: &serde_json::Value,
                           label: &str| {
            let expected_vectors = expected.as_array().unwrap();
            let actual_vectors: Vec<[u32; 4]> = output
                .vectors
                .iter()
                .map(|vector| vector.map(f32::to_bits))
                .collect();
            let actual_extents: Vec<u32> =
                output.extents.iter().map(|value| value.to_bits()).collect();
            let expected_values: Vec<[u32; 4]> = expected_vectors
                .iter()
                .map(|entry| {
                    entry
                        .as_array()
                        .unwrap()
                        .iter()
                        .take(4)
                        .map(|word| bits(word.as_str().unwrap()))
                        .collect::<Vec<_>>()
                        .try_into()
                        .unwrap()
                })
                .collect();
            let expected_extents: Vec<u32> = expected_vectors
                .iter()
                .map(|entry| bits(entry[4].as_str().unwrap()))
                .collect();
            assert_eq!(
                actual_vectors, expected_values,
                "{label} full Vector4 records"
            );
            assert_eq!(actual_extents, expected_extents, "{label} AddPoint extents");
            let native_ends: Vec<usize> = states["sample_ends"]
                .as_array()
                .unwrap()
                .iter()
                .map(|sample| sample["add_point_end"].as_u64().unwrap() as usize)
                .collect();
            let actual_ends = output
                .history
                .as_ref()
                .map_or(&[][..], |history| history.sample_ends.as_slice());
            assert_eq!(
                actual_ends, native_ends,
                "{label} historical sample boundaries"
            );
            assert_eq!(
                saved_history_state_bits(output.final_state),
                saved_history_state_bits(captured_history_state(&states["state"])),
                "{label} retained state"
            );
        };

        let mut ids = Vec::new();
        for case in fixture["cases"].as_array().unwrap() {
            let id = case["id"].as_str().unwrap();
            ids.push(id.to_owned());
            let native = &case["native"];
            let entry = native["native_drawable_states"]
                .as_array()
                .unwrap()
                .iter()
                .find(|state| state["phase"] == "redraw_entry")
                .unwrap();
            let retained = captured_history_state(entry);
            let output = replay_saved_object(
                &case["rows"],
                &case["tilt"],
                &case["orientation"],
                retained,
                native["saved_tool_type"].as_i64().unwrap() as i32,
            );
            compare_run(&output, &native["native_add_point_vectors"], native, id);
            let rows = case["rows"].as_array().unwrap();
            assert_eq!(
                output.event_time_ms,
                rows.last().unwrap()[3].as_i64().unwrap() - rows[0][3].as_i64().unwrap(),
                "{id} Base GetEventTime"
            );
            assert_eq!(
                output.event_pressure.to_bits(),
                (rows.last().unwrap()[2].as_f64().unwrap() as f32).to_bits(),
                "{id} terminal pressure"
            );
            assert_eq!(
                output.tool_type,
                native["saved_tool_type"].as_i64().unwrap() as i32,
                "{id} saved tool"
            );
            let both_channels = !case["tilt"].is_null() && !case["orientation"].is_null();
            if both_channels {
                assert_eq!(
                    output.event_tilt.to_bits(),
                    (case["tilt"]
                        .as_array()
                        .unwrap()
                        .last()
                        .unwrap()
                        .as_f64()
                        .unwrap() as f32)
                        .to_bits()
                );
                assert_eq!(
                    output.event_orientation.to_bits(),
                    (case["orientation"]
                        .as_array()
                        .unwrap()
                        .last()
                        .unwrap()
                        .as_f64()
                        .unwrap() as f32)
                        .to_bits()
                );
            } else {
                assert_eq!(
                    output.event_tilt.to_bits(),
                    0.0_f32.to_bits(),
                    "{id} paired optional tilt default"
                );
                assert_eq!(
                    output.event_orientation.to_bits(),
                    0.0_f32.to_bits(),
                    "{id} paired optional orientation default"
                );
            }
            assert_eq!(
                output.history.is_some(),
                rows.len() > 1,
                "{id} MotionEvent history presence"
            );
            assert_eq!(
                output.terminal.is_some(),
                rows.len() > 1,
                "{id} terminal endPen presence"
            );
            if let Some(end_entry) = native["native_drawable_states"]
                .as_array()
                .unwrap()
                .iter()
                .find(|state| state["phase"] == "end_pen_entry")
            {
                assert_saved_state_matches(
                    output.history.as_ref().unwrap().state,
                    end_entry,
                    &format!("{id} pre-endPen retained state"),
                );
                assert_saved_state_matches(
                    output.terminal.as_ref().unwrap().state,
                    &native["state"],
                    &format!("{id} post-endPen retained state"),
                );
            }
            let calls = &native["actual_base_model_call_counts"];
            for getter in [
                "SPen::ObjectStroke::GetToolType() const",
                "SPen::ObjectStroke::GetPointCount() const",
                "SPen::ObjectStroke::GetPoint() const",
                "SPen::ObjectStroke::GetPressure() const",
                "SPen::ObjectStroke::GetTimeStamp() const",
                "SPen::ObjectStroke::GetTilt() const",
                "SPen::ObjectStroke::GetOrientation() const",
                "SPen::MotionEvent::GetHistorySize() const",
                "SPen::MotionEvent::MotionEvent(int, int, SPen::PointF const*, float const*, int, int const*, float const*, float const*, long long const*)",
            ] {
                assert!(
                    calls[getter].as_u64().unwrap_or(0) > 0,
                    "{id} native call missing: {getter}"
                );
            }

            if !case["repeat_rows"].is_null() {
                let repeat = &native["same_drawable_repeat"];
                let repeat_entry = repeat["drawable_states"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|state| state["phase"] == "redraw_entry")
                    .unwrap();
                assert_eq!(
                    saved_history_state_bits(captured_history_state(repeat_entry)),
                    saved_history_state_bits(captured_history_state(&native["state"])),
                    "{id} retained state entering second saved stroke"
                );
                let reused = replay_saved_object(
                    &case["repeat_rows"],
                    &case["repeat_tilt"],
                    &case["repeat_orientation"],
                    captured_history_state(repeat_entry),
                    native["saved_tool_type"].as_i64().unwrap() as i32,
                );
                assert_eq!(
                    reused.tool_type,
                    native["saved_tool_type"].as_i64().unwrap() as i32
                );
                compare_run(
                    &reused,
                    &repeat["add_vectors"],
                    repeat,
                    &format!("{id} repeated stroke"),
                );
                let repeat_end_entry = repeat["drawable_states"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|state| state["phase"] == "end_pen_entry")
                    .unwrap();
                assert_saved_state_matches(
                    reused.history.as_ref().unwrap().state,
                    repeat_end_entry,
                    &format!("{id} repeated pre-endPen state"),
                );
                assert_saved_state_matches(
                    reused.terminal.as_ref().unwrap().state,
                    &repeat["state"],
                    &format!("{id} repeated post-endPen state"),
                );
                let fresh_control = fixture["cases"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|other| other["id"] == "fresh-second-stroke-control")
                    .unwrap();
                let fresh_vectors = fresh_control["native"]["native_add_point_vectors"]
                    .as_array()
                    .unwrap();
                assert_eq!(
                    reused
                        .vectors
                        .iter()
                        .map(|v| v.map(f32::to_bits))
                        .collect::<Vec<_>>(),
                    fresh_vectors
                        .iter()
                        .map(|entry| entry
                            .as_array()
                            .unwrap()
                            .iter()
                            .take(4)
                            .map(|word| bits(word.as_str().unwrap()))
                            .collect::<Vec<_>>()
                            .try_into()
                            .unwrap())
                        .collect::<Vec<[u32; 4]>>(),
                    "{id} reused vector output vs fresh drawable control"
                );
                assert_eq!(
                    reused
                        .extents
                        .iter()
                        .map(|v| v.to_bits())
                        .collect::<Vec<_>>(),
                    fresh_vectors
                        .iter()
                        .map(|entry| bits(entry[4].as_str().unwrap()))
                        .collect::<Vec<_>>(),
                    "{id} reused extent output vs fresh drawable control"
                );
            }
        }
        assert_eq!(ids.len(), 9);
    }

    #[test]
    fn saved_object_adapter_requires_native_array_lengths_and_nonempty_time_zero() {
        let retained = SavedHistoryState {
            nib: NibState::fresh(),
            accepted: P { x: 0.0, y: 0.0 },
            start: P { x: 0.0, y: 0.0 },
            control: P { x: 0.0, y: 0.0 },
            midpoint: P { x: 0.0, y: 0.0 },
        };
        let point = [[1.0, 2.0]];
        let pressure = [0.5];
        let timestamp = [100];
        let mismatched_pressure = [];
        let mismatched_tilt = [0.1, 0.2];
        assert!(
            super::saved_object_stroke_redraw(
                super::SavedObjectStrokeInput {
                    points: &point,
                    pressures: &mismatched_pressure,
                    timestamps: &timestamp,
                    tilt: None,
                    orientation: None,
                    tool_type: 2,
                    pen_size: 6.0,
                },
                retained,
            )
            .is_none()
        );
        assert!(
            super::saved_object_stroke_redraw(
                super::SavedObjectStrokeInput {
                    points: &point,
                    pressures: &pressure,
                    timestamps: &[],
                    tilt: None,
                    orientation: None,
                    tool_type: 2,
                    pen_size: 6.0,
                },
                retained,
            )
            .is_none()
        );
        assert!(
            super::saved_object_stroke_redraw(
                super::SavedObjectStrokeInput {
                    points: &point,
                    pressures: &pressure,
                    timestamps: &timestamp,
                    tilt: Some(&mismatched_tilt),
                    orientation: None,
                    tool_type: 2,
                    pen_size: 6.0,
                },
                retained,
            )
            .is_none()
        );
        assert!(
            super::saved_object_stroke_redraw(
                super::SavedObjectStrokeInput {
                    points: &[],
                    pressures: &[],
                    timestamps: &[],
                    tilt: None,
                    orientation: None,
                    tool_type: 2,
                    pen_size: 6.0,
                },
                retained,
            )
            .is_none()
        );
    }

    #[test]
    fn native_terminal_endpen_samples_replay_from_saved_drawable_state() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../conformance/calligraphy-saved-native.json"
        ))
        .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            for (run_name, run) in case["native"].as_object().unwrap() {
                let state = run["drawable_states"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|state| state["phase"] == "end_pen_entry")
                    .unwrap();
                let read_pair = |name: &str| -> [f32; 2] {
                    state[name]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|value| f32::from_bits(bits(value.as_str().unwrap())))
                        .collect::<Vec<_>>()
                        .try_into()
                        .unwrap()
                };
                let direction = read_pair("direction_bits");
                let accepted = read_pair("accepted_bits");
                let start = read_pair("path_start_bits");
                let control = read_pair("control_bits");
                let flags = &state["flags"];
                let input = SavedHistoryState {
                    nib: NibState {
                        angle_degrees: f32::from_bits(bits(state["angle_bits"].as_str().unwrap())),
                        direction,
                        initial_pending: flags["initial"].as_u64().unwrap() != 0,
                        movement_armed: flags["movement_armed"].as_u64().unwrap() != 0,
                    },
                    accepted: P {
                        x: accepted[0],
                        y: accepted[1],
                    },
                    start: P {
                        x: start[0],
                        y: start[1],
                    },
                    control: P {
                        x: control[0],
                        y: control[1],
                    },
                    midpoint: P {
                        x: read_pair("path_mid_bits")[0],
                        y: read_pair("path_mid_bits")[1],
                    },
                };
                let endpoint_bits: [f32; 2] = state["event_end_bits"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| f32::from_bits(bits(value.as_str().unwrap())))
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap();
                let size = f32::from_bits(bits(state["size_bits"].as_str().unwrap()));
                let actual = terminal_end_pen_vectors(
                    input,
                    P {
                        x: endpoint_bits[0],
                        y: endpoint_bits[1],
                    },
                    size,
                )
                .unwrap();
                let terminal_start = run["terminal_start"].as_u64().unwrap() as usize;
                let terminal = &run["add_points"].as_array().unwrap()[terminal_start..];
                let expected: Vec<[u32; 4]> = terminal
                    .iter()
                    .map(|stamp| {
                        stamp["vector_bits"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|value| bits(value.as_str().unwrap()))
                            .collect::<Vec<_>>()
                            .try_into()
                            .unwrap()
                    })
                    .collect();
                let actual_bits: Vec<[u32; 4]> = actual
                    .vectors
                    .iter()
                    .map(|vector| vector.map(f32::to_bits))
                    .collect();
                for stamp in terminal {
                    assert_eq!(
                        actual.point_extent.to_bits(),
                        bits(stamp["extent_bits"].as_str().unwrap()),
                        "{} {run_name} endPen half-size extent",
                        case["id"]
                    );
                }
                let native_length = run["length_queries"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|query| query["caller_pc"] == "0x301deac");
                if let Some(native_length) = native_length {
                    assert_eq!(
                        actual.path_length.unwrap().to_bits(),
                        bits(native_length["length_bits"].as_str().unwrap()),
                        "{} {run_name} endPen path length",
                        case["id"]
                    );
                } else {
                    assert!(input.nib.initial_pending, "{} {run_name}", case["id"]);
                    assert_eq!(actual.path_length, None);
                }
                assert_eq!(
                    actual_bits, expected,
                    "{} {run_name} endPen AddPoint vectors",
                    case["id"]
                );
            }
        }
    }

    #[test]
    fn synthetic_zero_length_saved_step_matches_native_failed_query_fallback() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../conformance/calligraphy-zero-length-native.json"
        ))
        .unwrap();
        let native = &fixture["case"]["native"];
        assert_eq!(bits(native["redraw_line_length_bits"].as_str().unwrap()), 0);
        assert_eq!(native["get_pos_tan"]["distance_bits"], "00000000");
        assert_eq!(native["get_pos_tan"]["ok"], false);
        assert_eq!(native["get_pos_tan"]["add_points_after"], 1);
        let synthetic = &fixture["case"]["synthetic_state"];
        let pair = |name: &str| -> [f32; 2] {
            synthetic[name]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_f64().unwrap() as f32)
                .collect::<Vec<_>>()
                .try_into()
                .unwrap()
        };
        let actual = saved_history_vectors(
            &[[1.0, 0.0, 0.0], [0.0, 0.0, 0.0]],
            SavedHistoryState {
                nib: NibState {
                    angle_degrees: synthetic["angle_degrees"].as_f64().unwrap() as f32,
                    direction: [0.0, 1.0],
                    initial_pending: false,
                    movement_armed: true,
                },
                accepted: P {
                    x: pair("accepted")[0],
                    y: pair("accepted")[1],
                },
                start: P {
                    x: pair("start")[0],
                    y: pair("start")[1],
                },
                control: P {
                    x: pair("control")[0],
                    y: pair("control")[1],
                },
                midpoint: P { x: 0.0, y: 0.0 },
            },
        )
        .unwrap();
        let expected: [u32; 4] = native["add_points"][0]["vector_bits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| bits(value.as_str().unwrap()))
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        assert_eq!(actual.vectors.len(), 1);
        assert_eq!(actual.vectors[0].map(f32::to_bits), expected);
    }

    #[test]
    fn synthetic_zero_length_terminal_endpen_skips_failed_position_sample() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../conformance/calligraphy-zero-terminal-native.json"
        ))
        .unwrap();
        let case = &fixture["case"];
        let native = &case["native"];
        let length = native["length_queries"][0]["length_bits"].as_str().unwrap();
        assert_eq!(bits(length), 0);
        assert_eq!(native["position_queries"][0]["ok"], false);
        assert_eq!(native["position_queries"][0]["add_points_after"], 0);
        assert!(native["add_points"].as_array().unwrap().is_empty());

        let state = &case["synthetic_state"];
        let pair = |name: &str| -> [f32; 2] {
            state[name]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| f32::from_bits(bits(value.as_str().unwrap())))
                .collect::<Vec<_>>()
                .try_into()
                .unwrap()
        };
        let flags = &state["flags"];
        let actual = terminal_end_pen_vectors(
            SavedHistoryState {
                nib: NibState {
                    angle_degrees: f32::from_bits(bits(state["angle_bits"].as_str().unwrap())),
                    direction: pair("direction_bits"),
                    initial_pending: flags["initial"].as_u64().unwrap() != 0,
                    movement_armed: flags["movement_armed"].as_u64().unwrap() != 0,
                },
                accepted: P { x: 0.0, y: 0.0 },
                start: P {
                    x: pair("path_start_bits")[0],
                    y: pair("path_start_bits")[1],
                },
                control: P {
                    x: pair("control_bits")[0],
                    y: pair("control_bits")[1],
                },
                midpoint: P { x: 0.0, y: 0.0 },
            },
            P { x: 0.0, y: 0.0 },
            6.0,
        )
        .unwrap();
        assert_eq!(actual.path_length.unwrap().to_bits(), 0);
        assert!(actual.vectors.is_empty());
    }
}
