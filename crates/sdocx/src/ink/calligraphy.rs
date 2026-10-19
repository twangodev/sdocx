//! Typed numeric core for the selected Montblanc calligraphy V1 path.
//!
//! Inputs distinguish saved history from drawable state retained by the
//! reused native object. Model admission is kept separate from numeric replay.

use super::path::{P, Quad};

const MAX_MODEL_POINTS: usize = 65_536;
const MAX_MODEL_ADD_POINTS: usize = 100_000;
const MAX_MODEL_TRIANGLES: usize = 1_000_000;

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
#[derive(Clone, Copy)]
pub(super) struct SavedObjectStrokeInput<'a> {
    pub(super) points: &'a [[f32; 2]],
    pub(super) pressures: &'a [f32],
    pub(super) timestamps: &'a [i32],
    pub(super) tilt: Option<&'a [f32]>,
    pub(super) orientation: Option<&'a [f32]>,
    pub(super) tool_type: i32,
    pub(super) pen_size: f32,
    /// Enabled native fixed-width style value, before RTV1's minimum of 1.
    pub(super) fixed_width: Option<f32>,
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

/// Per-stroke AddPoint members retained by the RTV1 drawable.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct AddPointState {
    pub(super) center: [f32; 2],
    pub(super) initial_edge: bool,
    pub(super) previous_edges: [[[f32; 2]; 2]; 3],
}

impl AddPointState {
    pub(super) const fn fresh() -> Self {
        Self {
            center: [0.0; 2],
            initial_edge: true,
            previous_edges: [[[0.0; 2]; 2]; 3],
        }
    }
}

/// Complete explicit state needed to adapt a saved stroke into RTV1 meshes.
#[derive(Clone, Copy, Debug)]
pub(super) struct CalligraphyState {
    pub(super) saved: SavedHistoryState,
    pub(super) add_point: AddPointState,
}

impl CalligraphyState {
    pub(super) const fn fresh() -> Self {
        let zero = P { x: 0.0, y: 0.0 };
        Self {
            saved: SavedHistoryState {
                nib: NibState::fresh(),
                accepted: zero,
                start: zero,
                control: zero,
                midpoint: zero,
            },
            add_point: AddPointState::fresh(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct AddPointFrame {
    pub(super) input: AddPointInput,
    pub(super) geometry: Option<AddPointGeometry>,
}

pub(super) struct SavedObjectStrokeMeshOutput {
    pub(super) stroke: SavedObjectStrokeOutput,
    pub(super) add_points: Vec<AddPointFrame>,
    pub(super) buffers: MeshBuffers,
    pub(super) state: CalligraphyState,
}

/// Per-Drawing state for the cached Calligraphy V1 drawable. Keep one session
/// for each native Drawing and exact pen identity; the caller owns that key.
/// An unsupported Calligraphy stroke poisons the session because silently
/// restarting from constructor defaults would lose retained native state.
pub(crate) struct CalligraphySession {
    state: Option<CalligraphyState>,
}

impl CalligraphySession {
    pub(crate) const fn new() -> Self {
        Self {
            state: Some(CalligraphyState::fresh()),
        }
    }

    pub(crate) fn reset(&mut self) {
        self.state = Some(CalligraphyState::fresh());
    }

    pub(crate) fn is_valid(&self) -> bool {
        self.state.is_some()
    }
}

/// A Calligraphy model stroke either does not belong to this renderer, cannot
/// be admitted without losing source or retained-state semantics, or produced
/// a native typed mesh.
pub(crate) enum CalligraphyAdmission {
    NotCalligraphy,
    Unsupported(CalligraphyUnsupported),
    Mesh(CalligraphyModelMesh),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CalligraphyUnsupported {
    MissingIdentity,
    InvalidSession,
    UnsupportedStrokeType,
    MissingStyle,
    InvalidChannels,
    EmptyGeometry,
    InvalidCoordinates,
    BudgetExceeded,
}

/// Output needed by the rendering adapter and replay/export sample mapping.
pub(crate) struct CalligraphyModelMesh {
    pub(crate) buffers: MeshBuffers,
    /// Exclusive triangle counts in [core, fringe A, fringe B] after each
    /// source-history sample. Keep native lane ordering in each replay step.
    pub(crate) sample_ends: Vec<[usize; 3]>,
    /// Lane-local triangle interval emitted by terminal `endPen`.
    pub(crate) terminal_start: [usize; 3],
    pub(crate) terminal_end: [usize; 3],
    pub(crate) rgb: [u8; 3],
}

const CALLIGRAPHY_PEN_NAME: &str =
    "com.samsung.android.sdk.pen.pen.preload.MontblancCalligraphyPen";

/// Admit one parsed stroke into this Drawing's Calligraphy drawable. On
/// unsupported matching input the retained state is discarded and stays
/// invalid until the owner creates a new Drawing session.
pub(crate) fn prepare_model_stroke(
    session: &mut CalligraphySession,
    stroke: &crate::Stroke,
) -> CalligraphyAdmission {
    let Some(rendering) = stroke.rendering.as_ref() else {
        session.state = None;
        return CalligraphyAdmission::Unsupported(CalligraphyUnsupported::MissingIdentity);
    };
    match rendering.pen_name.as_deref() {
        Some(name) if name != CALLIGRAPHY_PEN_NAME => return CalligraphyAdmission::NotCalligraphy,
        Some(_) => {}
        None => {
            session.state = None;
            return CalligraphyAdmission::Unsupported(CalligraphyUnsupported::MissingIdentity);
        }
    }
    let Some(mut state) = session.state else {
        return CalligraphyAdmission::Unsupported(CalligraphyUnsupported::InvalidSession);
    };

    // Drawing applies advanced settings, but selected V1 uses the fixed table
    // version and the saved geometry path does not read the parsed base value.
    if !matches!(rendering.style.stroke_type_raw, None | Some(0))
        || !matches!(rendering.style.color_type_raw, None | Some(0))
    {
        session.state = None;
        return CalligraphyAdmission::Unsupported(CalligraphyUnsupported::UnsupportedStrokeType);
    }

    let Some(pen_size) = rendering.style.pen_size else {
        session.state = None;
        return CalligraphyAdmission::Unsupported(CalligraphyUnsupported::MissingStyle);
    };
    let Some(argb) = rendering.style.color_argb else {
        session.state = None;
        return CalligraphyAdmission::Unsupported(CalligraphyUnsupported::MissingStyle);
    };
    if !pen_size.is_finite() {
        session.state = None;
        return CalligraphyAdmission::Unsupported(CalligraphyUnsupported::MissingStyle);
    }
    let fixed_width = if rendering.properties.fixed_width {
        let Some(width) = rendering.style.fixed_width else {
            session.state = None;
            return CalligraphyAdmission::Unsupported(CalligraphyUnsupported::MissingStyle);
        };
        if !width.is_finite() {
            session.state = None;
            return CalligraphyAdmission::Unsupported(CalligraphyUnsupported::MissingStyle);
        }
        Some(width)
    } else {
        None
    };

    if stroke.points.is_empty() {
        session.state = None;
        return CalligraphyAdmission::Unsupported(CalligraphyUnsupported::EmptyGeometry);
    }
    let count = stroke.points.len();
    if count > MAX_MODEL_POINTS {
        session.state = None;
        return CalligraphyAdmission::Unsupported(CalligraphyUnsupported::BudgetExceeded);
    }
    if stroke.pressures.len() != count || stroke.timestamps.len() != count {
        session.state = None;
        return CalligraphyAdmission::Unsupported(CalligraphyUnsupported::InvalidChannels);
    }
    let points = stroke
        .points
        .iter()
        .map(|point| [point.x as f32, point.y as f32])
        .collect::<Vec<_>>();
    let pressures = stroke
        .pressures
        .iter()
        .map(|&value| value as f32)
        .collect::<Vec<_>>();
    let timestamps = stroke
        .timestamps
        .iter()
        .map(|&value| i32::try_from(value))
        .collect::<Result<Vec<_>, _>>();
    let tilt = (!stroke.tilts.is_empty()).then(|| {
        stroke
            .tilts
            .iter()
            .map(|&value| value as f32)
            .collect::<Vec<_>>()
    });
    let orientation = (!stroke.orientations.is_empty()).then(|| {
        stroke
            .orientations
            .iter()
            .map(|&value| value as f32)
            .collect::<Vec<_>>()
    });
    let Some(timestamps) = timestamps.ok() else {
        session.state = None;
        return CalligraphyAdmission::Unsupported(CalligraphyUnsupported::InvalidChannels);
    };
    if (tilt.as_ref().is_some_and(|values| values.len() != count)
        || orientation
            .as_ref()
            .is_some_and(|values| values.len() != count))
        || pressures.iter().any(|value| !value.is_finite())
        || tilt
            .iter()
            .flatten()
            .chain(orientation.iter().flatten())
            .any(|value| !value.is_finite())
    {
        session.state = None;
        return CalligraphyAdmission::Unsupported(CalligraphyUnsupported::InvalidChannels);
    }
    if points.iter().flatten().any(|value| !value.is_finite()) {
        session.state = None;
        return CalligraphyAdmission::Unsupported(CalligraphyUnsupported::InvalidCoordinates);
    }

    let effective_tool_type = if rendering.tool_type_raw <= 4 {
        i32::from(rendering.tool_type_raw)
    } else {
        0
    };
    let input = SavedObjectStrokeInput {
        points: &points,
        pressures: &pressures,
        timestamps: &timestamps,
        tilt: tilt.as_deref(),
        orientation: orientation.as_deref(),
        tool_type: effective_tool_type,
        // The inherited native Pen::SetSize clamps before RedrawPen reads it.
        pen_size: pen_size.clamp(0.4, 800.0),
        fixed_width,
    };
    let Some(output) = saved_object_stroke_mesh_bounded(input, state, MAX_MODEL_ADD_POINTS) else {
        session.state = None;
        return CalligraphyAdmission::Unsupported(CalligraphyUnsupported::BudgetExceeded);
    };
    state = output.state;
    session.state = Some(state);
    let terminal_start_vector = output
        .stroke
        .history
        .as_ref()
        .map_or(0, |history| history.vectors.len());
    let history_sample_ends = output
        .stroke
        .history
        .as_ref()
        .map_or_else(Vec::new, |history| history.sample_ends.clone());
    let mut lane_triangle_counts = [0usize; 3];
    let mut lane_prefixes = Vec::with_capacity(output.add_points.len() + 1);
    lane_prefixes.push(lane_triangle_counts);
    for frame in &output.add_points {
        if let Some(geometry) = frame.geometry {
            for buffer in geometry.buffers {
                let lane = match buffer {
                    MeshBuffer::Core => 0,
                    MeshBuffer::FringeA => 1,
                    MeshBuffer::FringeB => 2,
                };
                lane_triangle_counts[lane] += 2;
            }
        }
        lane_prefixes.push(lane_triangle_counts);
    }
    let sample_ends = history_sample_ends
        .into_iter()
        .map(|vector_end| lane_prefixes[vector_end])
        .collect();
    let terminal_start = lane_prefixes[terminal_start_vector];
    let terminal_end = lane_prefixes[output.stroke.vectors.len()];
    CalligraphyAdmission::Mesh(CalligraphyModelMesh {
        buffers: output.buffers,
        sample_ends,
        terminal_start,
        terminal_end,
        rgb: [(argb >> 16) as u8, (argb >> 8) as u8, argb as u8],
    })
}

/// Adapt the saved object's source arrays to Base MotionEvent history plus its
/// separate terminal event. Callers must supply the drawable's retained state;
/// this function does not assume every saved record starts from a new drawable.
pub(super) fn saved_object_stroke_redraw(
    input: SavedObjectStrokeInput<'_>,
    retained: SavedHistoryState,
) -> Option<SavedObjectStrokeOutput> {
    let mut remaining_add_points = usize::MAX;
    saved_object_stroke_redraw_bounded(input, retained, &mut remaining_add_points)
}

fn saved_object_stroke_redraw_bounded(
    input: SavedObjectStrokeInput<'_>,
    retained: SavedHistoryState,
    remaining_add_points: &mut usize,
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
        || input.fixed_width.is_some_and(|width| !width.is_finite())
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
        Some(saved_history_vectors_bounded(
            &history_rows,
            prepared,
            remaining_add_points,
        )?)
    } else {
        None
    };
    let mut terminal = if let Some(history) = &history {
        Some(terminal_end_pen_vectors_bounded(
            history.state,
            P {
                x: input.points[count - 1][0],
                y: input.points[count - 1][1],
            },
            input.pen_size,
            remaining_add_points,
        )?)
    } else {
        None
    };
    if let (Some(terminal), Some(width)) = (&mut terminal, input.fixed_width) {
        terminal.point_extent = width.max(1.0);
    }
    let mut vectors = Vec::new();
    if let Some(history) = &history {
        vectors.extend(history.vectors.iter().copied());
    }
    if let Some(terminal) = &terminal {
        vectors.extend(terminal.vectors.iter().copied());
    }
    let extent = input
        .fixed_width
        .map_or((input.pen_size * 0.5).max(1.0), |width| width.max(1.0));
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
    state: SavedHistoryState,
    endpoint: P,
    pen_size: f32,
) -> Option<EndPenOutput> {
    let mut remaining_add_points = usize::MAX;
    terminal_end_pen_vectors_bounded(state, endpoint, pen_size, &mut remaining_add_points)
}

fn terminal_end_pen_vectors_bounded(
    mut state: SavedHistoryState,
    endpoint: P,
    pen_size: f32,
    remaining_add_points: &mut usize,
) -> Option<EndPenOutput> {
    if !pen_size.is_finite() || pen_size < 0.0 {
        return None;
    }
    if state.nib.initial_pending {
        if *remaining_add_points == 0 {
            return None;
        }
        *remaining_add_points -= 1;
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
    let emitted = if length > 0.0 { count as usize } else { 0 };
    if emitted > *remaining_add_points {
        return None;
    }
    *remaining_add_points -= emitted;
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
    state: SavedHistoryState,
) -> Option<SavedHistoryOutput> {
    let mut remaining_add_points = usize::MAX;
    saved_history_vectors_bounded(history, state, &mut remaining_add_points)
}

fn saved_history_vectors_bounded(
    history: &[[f32; 3]],
    mut state: SavedHistoryState,
    remaining_add_points: &mut usize,
) -> Option<SavedHistoryOutput> {
    if history.is_empty() {
        return None;
    }
    let mut output = SavedHistoryOutput {
        vectors: Vec::new(),
        sample_ends: Vec::with_capacity(history.len().saturating_sub(1)),
        state,
    };
    let mut redrew_line = false;
    for &[x, y, orientation] in history.iter().skip(1) {
        let current = P { x, y };
        let dx = (x - state.accepted.x).abs();
        let dy = (y - state.accepted.y).abs();
        if dx < 1.5 && dy < 1.5 {
            output.sample_ends.push(output.vectors.len());
            continue;
        }
        redrew_line = true;
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
                let emitted = if path.length > 0.0 {
                    count as usize + 1
                } else {
                    1
                };
                if emitted > *remaining_add_points {
                    return None;
                }
                *remaining_add_points -= emitted;
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
    // A history record rejected by the 1.5-pixel gate returns before the
    // midpoint epilogue, so +104 stays retained. An admitted redrawLine
    // epilogue copies its accepted position into midpoint (+104) and path
    // start (+88).
    if redrew_line {
        state.midpoint = state.start;
    }
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
pub(crate) struct MeshVertex {
    pub(crate) position: [f32; 2],
    pub(crate) coverage: [f32; 2],
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
pub(crate) struct MeshBuffers {
    pub(crate) core: Vec<MeshVertex>,
    pub(crate) fringe_a: Vec<MeshVertex>,
    pub(crate) fringe_b: Vec<MeshVertex>,
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

/// Adapt saved ObjectStroke channels and explicit retained drawable state to
/// the three Vector4 buffers produced by the native RTV1 AddPoint path.
pub(super) fn saved_object_stroke_mesh(
    input: SavedObjectStrokeInput<'_>,
    state: CalligraphyState,
) -> Option<SavedObjectStrokeMeshOutput> {
    saved_object_stroke_mesh_bounded(input, state, usize::MAX)
}

fn saved_object_stroke_mesh_bounded(
    input: SavedObjectStrokeInput<'_>,
    mut state: CalligraphyState,
    add_point_budget: usize,
) -> Option<SavedObjectStrokeMeshOutput> {
    if input.points.len() > MAX_MODEL_POINTS {
        return None;
    }
    let mut remaining_add_points = add_point_budget;
    if input.points.len() >= 2 {
        // RedrawPen starts each multi-record stroke at its first saved point,
        // rearms the initial edge, and retains RTV1's previous-edge cache.
        state.add_point.center = input.points[0];
        state.add_point.initial_edge = true;
    }
    let stroke = saved_object_stroke_redraw_bounded(input, state.saved, &mut remaining_add_points)?;
    let mut add_points = Vec::with_capacity(stroke.vectors.len());
    let mut buffers = MeshBuffers::default();
    let mut triangle_count = 0usize;
    for (vector, extent) in stroke
        .vectors
        .iter()
        .copied()
        .zip(stroke.extents.iter().copied())
    {
        let frame_input = AddPointInput {
            vector,
            extent,
            gate_center: state.add_point.center,
            initial_edge: state.add_point.initial_edge,
            previous_edges: state.add_point.previous_edges,
        };
        let geometry = add_point(frame_input);
        if let Some(geometry) = geometry {
            for (buffer, rect) in geometry
                .buffers
                .iter()
                .copied()
                .zip(geometry.rectangles.iter().copied())
            {
                triangle_count = triangle_count.checked_add(2)?;
                if triangle_count > MAX_MODEL_TRIANGLES {
                    return None;
                }
                buffers.append(buffer, rect);
            }
            state.add_point.previous_edges = geometry.next_edges;
        }
        state.add_point.center = [vector[0], vector[1]];
        state.add_point.initial_edge = false;
        add_points.push(AddPointFrame {
            input: frame_input,
            geometry,
        });
    }
    state.saved = stroke.final_state;
    Some(SavedObjectStrokeMeshOutput {
        stroke,
        add_points,
        buffers,
        state,
    })
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
    let output = saved_object_stroke_mesh(
        SavedObjectStrokeInput {
            points: &points,
            pressures: &pressures,
            timestamps: &timestamps,
            tilt: Some(&vec![0.0; points.len()]),
            orientation: Some(&orientations),
            tool_type: 2,
            pen_size: case["input"]["pen_size"].as_f64()? as f32,
            fixed_width: None,
        },
        CalligraphyState::fresh(),
    )?;
    let stroke = &output.stroke;
    let Some(last_vector) = stroke.vectors.last() else {
        return None;
    };
    if output.state.add_point.center.map(f32::to_bits)
        != [last_vector[0].to_bits(), last_vector[1].to_bits()]
        || output.state.saved.nib.angle_degrees.to_bits()
            != stroke.final_state.nib.angle_degrees.to_bits()
        || output.state.saved.nib.direction.map(f32::to_bits)
            != stroke.final_state.nib.direction.map(f32::to_bits)
    {
        return None;
    }

    let captured_add_points = case["saved_redraw"]["native_add_points"].as_array()?;
    if captured_add_points.len() != stroke.vectors.len() {
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
    for (index, (vector, extent)) in stroke.vectors.iter().zip(&stroke.extents).enumerate() {
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

    let native_builds = case["saved_redraw"]["native_build_rectangles"].as_array()?;
    let mut build_cursor = 0;
    for (index, frame) in output.add_points.iter().enumerate() {
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
        let actual_edges: Vec<u32> = frame
            .input
            .previous_edges
            .iter()
            .flat_map(|pair| pair.iter().flat_map(|point| point.iter().copied()))
            .map(f32::to_bits)
            .collect();
        if frame.input.gate_center.map(f32::to_bits) != expected_center
            || frame.input.initial_edge != (snapshot["initial_flag"].as_u64()? != 0)
            || actual_edges != expected_edges
        {
            return None;
        }
        let expected_build_count = captured_add["build_calls"].as_u64()? as usize;
        if frame.geometry.is_some() != (expected_build_count == 5) {
            return None;
        }
        if let Some(geometry) = frame.geometry {
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
            }
            build_cursor += 5;
        }
    }
    if build_cursor != native_builds.len() {
        return None;
    }
    let lanes = |vertices: &[MeshVertex]| {
        vertices
            .iter()
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
        lanes(&output.buffers.core),
        lanes(&output.buffers.fringe_a),
        lanes(&output.buffers.fringe_b),
    ])
}

impl NibState {
    /// GLV1 constructor defaults, as captured before the first RedrawPen.
    /// A cached drawable retains this state between records; RedrawPen rearms
    /// both lifecycle flags from each new source stroke. The pinned native
    /// constructor stores zero at GLV1+80 (0x1d7b0) and +272 (0x1d7e0).
    pub(super) const fn fresh() -> Self {
        Self {
            angle_degrees: 0.0,
            direction: [1.0, 0.0],
            initial_pending: false,
            movement_armed: false,
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
        AddPointInput, MeshBuffer, MeshBuffers, MeshRect, NibState, SavedHistoryState,
        SavedObjectStrokeInput, add_point, advance_angle, orientation_target,
        prepare_saved_history, repeat_count, saved_history_vectors, terminal_end_pen_vectors,
    };
    use crate::ink::path::P;

    fn fixture_f32s(source: &serde_json::Value, bits_key: &str, bytes_key: &str) -> Vec<f32> {
        if let Some(values) = source.get(bits_key).and_then(serde_json::Value::as_array) {
            return values
                .iter()
                .map(|value| f32::from_bits(bits(value.as_str().unwrap())))
                .collect();
        }
        let bytes = source[bytes_key].as_str().unwrap().as_bytes();
        bytes
            .chunks_exact(8)
            .map(|word| {
                let bytes = word
                    .chunks_exact(2)
                    .map(|byte| u8::from_str_radix(std::str::from_utf8(byte).unwrap(), 16).unwrap())
                    .collect::<Vec<_>>();
                f32::from_le_bytes(bytes.try_into().unwrap())
            })
            .collect()
    }

    fn fixture_stroke(source: &serde_json::Value) -> crate::Stroke {
        use crate::{
            BoundingBox, Color, Point, Stroke, StrokeProperties, StrokeRendering, StrokeStyle,
        };

        let coordinates = fixture_f32s(source, "points_f32_bits", "point_bytes_f32_le");
        let points = coordinates
            .chunks_exact(2)
            .map(|xy| Point {
                x: f64::from(xy[0]),
                y: f64::from(xy[1]),
            })
            .collect::<Vec<_>>();
        let pressures = fixture_f32s(source, "pressure_f32_bits", "pressure_bytes_f32_le")
            .into_iter()
            .map(f64::from)
            .collect();
        let tilts = fixture_f32s(source, "tilt_f32_bits", "tilt_bytes_f32_le")
            .into_iter()
            .map(f64::from)
            .collect();
        let orientations = fixture_f32s(source, "orientation_f32_bits", "orientation_bytes_f32_le")
            .into_iter()
            .map(f64::from)
            .collect();
        let timestamps = source["timestamps_i32"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_i64().unwrap())
            .collect();
        let argb = source["argb_u32"].as_u64().unwrap() as u32;
        let pen_size = f32::from_bits(bits(source["pen_size_f32_bits"].as_str().unwrap()));
        let curve_enabled = source["curve_enabled_setter_arg"].as_i64() != Some(0);
        Stroke {
            rendering: Some(StrokeRendering {
                metadata: None,
                pen_name: Some(
                    source
                        .get("pen_name")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or(super::CALLIGRAPHY_PEN_NAME)
                        .to_owned(),
                ),
                advanced_settings: None,
                tool_type_raw: source["tool_type"].as_u64().unwrap() as u16,
                properties: StrokeProperties {
                    compressed: curve_enabled,
                    replay_only: false,
                    stylus_channels: true,
                    eraser: false,
                    fixed_width: false,
                    millisecond_timestamps: false,
                    top_layer_pen: false,
                    alpha_lock: false,
                    binary_added: true,
                    generated: false,
                    fixed_opacity: false,
                    rainbow_effect: false,
                    straighten: false,
                    reveal_mode: false,
                },
                style: StrokeStyle {
                    color_argb: Some(argb),
                    pen_size: Some(pen_size),
                    stroke_type_raw: Some(source["stroke_type"].as_u64().unwrap() as u16),
                    color_type_raw: Some(0),
                    ..StrokeStyle::default()
                },
            }),
            bbox: BoundingBox::default(),
            points,
            pressures,
            timestamps,
            tilts,
            orientations,
            color: Some(Color {
                r: (argb >> 16) as u8,
                g: (argb >> 8) as u8,
                b: argb as u8,
            }),
            pen_width: pen_size,
        }
    }

    fn mesh_lane_bytes(vertices: &[super::MeshVertex]) -> Vec<u8> {
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
            .flat_map(f32::to_le_bytes)
            .collect()
    }

    fn fixture_hex_bytes(value: &str) -> Vec<u8> {
        value
            .as_bytes()
            .chunks_exact(2)
            .map(|byte| u8::from_str_radix(std::str::from_utf8(byte).unwrap(), 16).unwrap())
            .collect()
    }

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
        fixed_width: Option<f32>,
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
                fixed_width,
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
    fn object_drawing_fresh_streams_match_saved_mesh_producer_bitwise() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../conformance/calligraphy-saved-caller-native.json"
        ))
        .unwrap();
        assert_eq!(
            fixture["schema"],
            "sdocx-calligraphy-saved-caller-native-v1"
        );
        let case = &fixture["cases"]["three-point-curve-fresh-drawing"];
        let source = &case["input"];
        let f32s = |name: &str| -> Vec<f32> {
            source[name]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| f32::from_bits(bits(value.as_str().unwrap())))
                .collect()
        };
        let xy = f32s("points_f32_bits");
        assert_eq!(xy.len() % 2, 0);
        let points: Vec<[f32; 2]> = xy
            .chunks_exact(2)
            .map(|point| [point[0], point[1]])
            .collect();
        let pressures = f32s("pressure_f32_bits");
        let tilt = f32s("tilt_f32_bits");
        let orientation = f32s("orientation_f32_bits");
        let timestamps: Vec<i32> = source["timestamps_i32"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_i64().unwrap() as i32)
            .collect();
        let size = f32::from_bits(bits(source["pen_size_f32_bits"].as_str().unwrap()));
        assert_eq!(source["curve_enabled_setter_arg"].as_i64(), Some(1));
        let input = SavedObjectStrokeInput {
            points: &points,
            pressures: &pressures,
            timestamps: &timestamps,
            tilt: Some(&tilt),
            orientation: Some(&orientation),
            tool_type: source["tool_type"].as_i64().unwrap() as i32,
            pen_size: size,
            fixed_width: None,
        };
        let parse_words = |hex: &str| -> Vec<u32> {
            hex.as_bytes()
                .chunks_exact(8)
                .map(|word| bits(std::str::from_utf8(word).unwrap()))
                .collect()
        };
        let compare_capture = |label: &str,
                               capture: &serde_json::Value,
                               stream_input: SavedObjectStrokeInput<'_>,
                               state: super::CalligraphyState| {
            let actual = super::saved_object_stroke_mesh(stream_input, state)
                .expect("captured Model channels are accepted by saved mesh producer");
            let add_points = capture["add_points"].as_array().unwrap();
            let build_calls = capture["build_calls"].as_array().unwrap();
            assert_eq!(
                actual.add_points.len(),
                add_points.len(),
                "{label} AddPoint calls"
            );
            assert_eq!(
                build_calls.len(),
                add_points.len() * 5,
                "{label} buildMesh calls"
            );
            let mut build_cursor = 0;
            for (index, (frame, native)) in actual.add_points.iter().zip(add_points).enumerate() {
                assert_eq!(
                    frame.input.vector.map(f32::to_bits),
                    parse_words(native["vector_bytes"].as_str().unwrap()).as_slice(),
                    "{label} AddPoint {index} Vector4"
                );
                assert_eq!(
                    frame.input.extent.to_bits(),
                    bits(native["extent_bits"].as_str().unwrap()),
                    "{label} AddPoint {index} extent"
                );
                let state = &native["pre_state"];
                let center: [u32; 2] = state["center_bits"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|word| bits(word.as_str().unwrap()))
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap();
                assert_eq!(
                    frame.input.gate_center.map(f32::to_bits),
                    center,
                    "{label} AddPoint {index} center"
                );
                assert_eq!(
                    frame.input.initial_edge,
                    state["initial_flag"].as_u64().unwrap() != 0,
                    "{label} AddPoint {index} initial flag"
                );
                let expected_edges: Vec<u32> = state["edge_state_bits"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|word| bits(word.as_str().unwrap()))
                    .collect();
                let actual_edges: Vec<u32> = frame
                    .input
                    .previous_edges
                    .iter()
                    .flat_map(|pair| pair.iter().flat_map(|point| point.iter().copied()))
                    .map(f32::to_bits)
                    .collect();
                assert_eq!(
                    actual_edges, expected_edges,
                    "{label} AddPoint {index} edge cache"
                );
                let geometry = frame.geometry.expect("all captured AddPoint calls build");
                for (slot, rect) in geometry.rectangles.iter().enumerate() {
                    assert_eq!(
                        rect_bits(*rect).as_slice(),
                        parse_words(build_calls[build_cursor]["rect"].as_str().unwrap()).as_slice(),
                        "{label} AddPoint {index} buildMesh slot {slot}"
                    );
                    build_cursor += 1;
                }
            }
            assert_eq!(build_cursor, build_calls.len(), "{label} buildMesh count");
            let lanes = [
                &actual.buffers.core,
                // Native capture slots follow first-seen BuildMesh output
                // objects: core, FringeB, then FringeA.
                &actual.buffers.fringe_b,
                &actual.buffers.fringe_a,
            ];
            let native_buffers = capture["rtv1_mesh"]["buffers"].as_array().unwrap();
            for (slot, vertices) in lanes.iter().enumerate() {
                let native = native_buffers
                    .iter()
                    .find(|buffer| buffer["build_output_slot"].as_u64() == Some(slot as u64))
                    .unwrap();
                let actual_bytes: Vec<u8> = vertices
                    .iter()
                    .flat_map(|vertex| {
                        [
                            vertex.position[0],
                            vertex.position[1],
                            vertex.coverage[0],
                            vertex.coverage[1],
                        ]
                        .into_iter()
                        .flat_map(f32::to_le_bytes)
                    })
                    .collect();
                let expected_bytes: Vec<u8> = native["bytes_hex"]
                    .as_str()
                    .unwrap()
                    .as_bytes()
                    .chunks_exact(2)
                    .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                    .collect();
                assert_eq!(actual_bytes, expected_bytes, "{label} RTV1 buffer {slot}");
            }
            let last = stream_input.points.len() - 1;
            assert_eq!(
                actual.stroke.event_time_ms,
                i64::from(stream_input.timestamps[last] - stream_input.timestamps[0]),
                "{label} MotionEvent time"
            );
            assert_eq!(
                actual.stroke.event_pressure.to_bits(),
                stream_input.pressures[last].to_bits(),
                "{label} terminal pressure"
            );
            let (expected_tilt, expected_orientation) =
                match (stream_input.tilt, stream_input.orientation) {
                    (Some(tilt), Some(orientation)) => (tilt[last], orientation[last]),
                    _ => (0.0, 0.0),
                };
            assert_eq!(
                actual.stroke.event_tilt.to_bits(),
                expected_tilt.to_bits(),
                "{label} terminal tilt"
            );
            assert_eq!(
                actual.stroke.event_orientation.to_bits(),
                expected_orientation.to_bits(),
                "{label} terminal orientation"
            );
            assert_eq!(
                actual.stroke.tool_type, stream_input.tool_type,
                "{label} tool type"
            );
            actual
        };
        let first_capture = &case["native_capture"]["first_draw"];
        let first_output = compare_capture(
            "first_draw",
            first_capture,
            input,
            super::CalligraphyState::fresh(),
        );
        let fresh_output = compare_capture(
            "fresh_drawing",
            &case["native_capture"]["fresh_drawing"],
            input,
            super::CalligraphyState::fresh(),
        );
        assert_eq!(
            saved_history_state_bits(first_output.state.saved),
            saved_history_state_bits(fresh_output.state.saved),
            "independent new Drawings reach the same saved state"
        );

        let raw_state = |hex: &str| -> super::SavedHistoryState {
            let bytes: Vec<u8> = hex
                .as_bytes()
                .chunks_exact(2)
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect();
            let f32_at = |offset: usize| {
                f32::from_bits(u32::from_le_bytes(
                    bytes[offset..offset + 4].try_into().unwrap(),
                ))
            };
            super::SavedHistoryState {
                nib: super::NibState {
                    angle_degrees: f32_at(284),
                    direction: [f32_at(276), f32_at(280)],
                    initial_pending: bytes[80] != 0,
                    movement_armed: bytes[272] != 0,
                },
                accepted: P {
                    x: f32_at(116),
                    y: f32_at(120),
                },
                start: P {
                    x: f32_at(88),
                    y: f32_at(92),
                },
                control: P {
                    x: f32_at(96),
                    y: f32_at(100),
                },
                midpoint: P {
                    x: f32_at(104),
                    y: f32_at(108),
                },
            }
        };
        let reused = &case["reused_drawing_streams"];
        let same_stroke = &reused["same_stroke_repeat"];
        let same_entry = &same_stroke["redraw_sessions"][0];
        assert_eq!(
            saved_history_state_bits(first_output.state.saved),
            saved_history_state_bits(raw_state(
                reused["original_three_point_fresh_drawing"]["capture"]["redraw_sessions"]
                    .as_array()
                    .unwrap()
                    .last()
                    .unwrap()["glv1_state_after_hex"]
                    .as_str()
                    .unwrap()
            )),
            "first producer state matches full native GLV1 state after first drawing"
        );
        assert_eq!(
            saved_history_state_bits(first_output.state.saved),
            saved_history_state_bits(raw_state(
                same_entry["glv1_state_before_hex"].as_str().unwrap()
            )),
            "same-drawing repeat entry retains complete saved GLV1 state"
        );
        let repeated_output =
            compare_capture("same_stroke_repeat", same_stroke, input, first_output.state);
        assert_eq!(
            saved_history_state_bits(repeated_output.state.saved),
            saved_history_state_bits(raw_state(
                same_entry["glv1_state_after_hex"].as_str().unwrap()
            )),
            "repeated producer state matches full native GLV1 state after redraw"
        );

        let short = &reused["different_short_stroke"];
        let short_input = &short["input"];
        let decode_f32_bytes = |value: &serde_json::Value| -> Vec<f32> {
            let hex = value.as_str().unwrap();
            hex.as_bytes()
                .chunks_exact(8)
                .map(|word| f32::from_bits(bits(std::str::from_utf8(word).unwrap())))
                .collect()
        };
        let short_xy = decode_f32_bytes(&short_input["point_bytes_f32_le"]);
        let short_points: Vec<[f32; 2]> = short_xy
            .chunks_exact(2)
            .map(|point| [point[0], point[1]])
            .collect();
        let short_pressures = decode_f32_bytes(&short_input["pressure_bytes_f32_le"]);
        let short_tilt = decode_f32_bytes(&short_input["tilt_bytes_f32_le"]);
        let short_orientation = decode_f32_bytes(&short_input["orientation_bytes_f32_le"]);
        let short_timestamps: Vec<i32> = short_input["timestamps_i32"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_i64().unwrap() as i32)
            .collect();
        let short_stream_input = SavedObjectStrokeInput {
            points: &short_points,
            pressures: &short_pressures,
            timestamps: &short_timestamps,
            tilt: Some(&short_tilt),
            orientation: Some(&short_orientation),
            tool_type: short_input["tool_type"].as_i64().unwrap() as i32,
            pen_size: f32::from_bits(bits(short_input["pen_size_f32_bits"].as_str().unwrap())),
            fixed_width: None,
        };
        assert_eq!(
            short_points.len(),
            2,
            "captured short stream has two source points"
        );
        let short_entry = &short["same_drawing_capture"]["redraw_sessions"][0];
        assert_eq!(
            saved_history_state_bits(repeated_output.state.saved),
            saved_history_state_bits(raw_state(
                short_entry["glv1_state_before_hex"].as_str().unwrap()
            )),
            "different-short-stroke entry retains full prior GLV1 state"
        );
        let short_reused = compare_capture(
            "different_short_stroke_same_drawing",
            &short["same_drawing_capture"],
            short_stream_input,
            repeated_output.state,
        );
        let short_native_after = raw_state(short_entry["glv1_state_after_hex"].as_str().unwrap());
        let short_typed_after = short_reused.state.saved;
        assert_eq!(
            saved_history_state_bits(short_typed_after),
            saved_history_state_bits(short_native_after),
            "short stroke full saved state after admitted-history gating"
        );
        assert_eq!(
            short_reused.add_points.len(),
            1,
            "native short stream emits one AddPoint"
        );

        let fresh_capture = &short["fresh_drawing_capture"];
        let short_fresh = compare_capture(
            "different_short_stroke_fresh_drawing",
            fresh_capture,
            short_stream_input,
            super::CalligraphyState::fresh(),
        );
        assert_eq!(
            short_fresh.add_points.len(),
            1,
            "fresh short stream emits one AddPoint"
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
                if run_name == "pen_style" {
                    continue;
                }
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
            for (run_name, run) in case["native"].as_object().unwrap() {
                if run_name == "pen_style" {
                    continue;
                }
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
                        run_name
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
                                run_name
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
                                case["id"], run_name
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
        let native_fresh = prepare_saved_history(
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
        let actual = saved_history_vectors(&history, native_fresh).unwrap();
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
                if run_name == "pen_style" {
                    continue;
                }
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
            let effective_tool = native["saved_tool_type"].as_i64().unwrap() as i32;
            if let Some(tool) = case.get("tool_type") {
                let requested_tool = tool.as_i64().unwrap() as i32;
                let capture = &case["native_tool"];
                assert_eq!(
                    capture["requested"].as_i64().unwrap(),
                    i64::from(requested_tool),
                    "{id} requested tool"
                );
                assert_eq!(
                    capture["reset_to_2"]["setter_return"].as_i64().unwrap(),
                    1,
                    "{id} baseline setter"
                );
                assert_eq!(
                    capture["reset_to_2"]["getter"].as_i64().unwrap(),
                    2,
                    "{id} explicit baseline getter"
                );
                let accepted = (0..=4).contains(&requested_tool);
                assert_eq!(
                    capture["setter_return"].as_i64().unwrap(),
                    i64::from(accepted),
                    "{id} native setter status"
                );
                let captured_tool = capture["getter"].as_i64().unwrap() as i32;
                assert_eq!(captured_tool, effective_tool, "{id} effective model getter");
                assert_eq!(
                    effective_tool,
                    if accepted { requested_tool } else { 2 },
                    "{id} stored tool after setter"
                );
                assert!(
                    !capture["motion_event_getter_values"]["first"]
                        .as_array()
                        .unwrap()
                        .is_empty(),
                    "{id} native MotionEvent tool reads were recorded"
                );
                assert_eq!(
                    capture["motion_event_getter_values"]["first"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|entry| entry["return"].as_i64().unwrap() as i32)
                        .collect::<Vec<_>>(),
                    vec![
                        effective_tool;
                        capture["motion_event_getter_values"]["first"]
                            .as_array()
                            .unwrap()
                            .len()
                    ],
                    "{id} MotionEvent tool getter values"
                );
            } else if let Some(raw) = case.get("raw_tool_type") {
                let raw_tool = raw.as_i64().unwrap() as i32;
                let capture = &case["native_tool"];
                assert_eq!(capture["mode"], "direct_raw_field", "{id} raw tool mode");
                assert_eq!(
                    capture["reset_field_value"].as_i64().unwrap(),
                    2,
                    "{id} raw field baseline"
                );
                assert_eq!(
                    capture["reset_getter"].as_i64().unwrap(),
                    2,
                    "{id} raw getter baseline"
                );
                assert_eq!(
                    capture["raw_field_value"].as_i64().unwrap(),
                    i64::from(raw_tool),
                    "{id} raw field input"
                );
                assert_eq!(
                    capture["setter_invoked"], false,
                    "{id} bypasses Model setter"
                );
                assert!(
                    capture.get("setter_return").is_none(),
                    "{id} has no setter status"
                );
                let captured_tool = capture["getter"].as_i64().unwrap() as i32;
                let expected_tool = if (0..=4).contains(&raw_tool) {
                    raw_tool
                } else {
                    0
                };
                assert_eq!(
                    captured_tool, expected_tool,
                    "{id} native raw getter mapping"
                );
                assert_eq!(
                    captured_tool, effective_tool,
                    "{id} sampler uses effective getter"
                );
                let motion_reads = capture["motion_event_getter_values"]["first"]
                    .as_array()
                    .unwrap();
                assert!(
                    !motion_reads.is_empty(),
                    "{id} native MotionEvent reads recorded"
                );
                assert_eq!(
                    motion_reads
                        .iter()
                        .map(|entry| entry["return"].as_i64().unwrap() as i32)
                        .collect::<Vec<_>>(),
                    vec![effective_tool; motion_reads.len()],
                    "{id} MotionEvent receives raw getter result"
                );
            }
            let fixed_width = if let Some(style) = case.get("pen_style") {
                let captured = &case["native_style"];
                assert_eq!(captured["input"], *style, "{id} native style input");
                assert_eq!(captured["setter_returns"]["curve_enabled"], 1, "{id}");
                assert_eq!(captured["setter_returns"]["fixed_width_enabled"], 1, "{id}");
                assert_eq!(captured["setter_returns"]["fixed_width"], 1, "{id}");
                assert_eq!(
                    captured["readback"]["curve_enabled"].as_u64().unwrap(),
                    u64::from(style["curve_enabled"].as_bool().unwrap()),
                    "{id} native curve style readback"
                );
                assert_eq!(
                    captured["readback"]["fixed_width_enabled"]
                        .as_u64()
                        .unwrap(),
                    u64::from(style["fixed_width_enabled"].as_bool().unwrap()),
                    "{id} native fixed-width enable readback"
                );
                assert_eq!(
                    bits(captured["readback"]["fixed_width_bits"].as_str().unwrap()),
                    (style["fixed_width"].as_f64().unwrap() as f32).to_bits(),
                    "{id} native fixed-width value readback"
                );
                style["fixed_width_enabled"]
                    .as_bool()
                    .unwrap()
                    .then(|| style["fixed_width"].as_f64().unwrap() as f32)
            } else {
                None
            };
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
                effective_tool,
                fixed_width,
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
                    fixed_width,
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
        assert_eq!(ids.len(), 32);
        for id in [
            "fixed-width-two",
            "fixed-width-half",
            "fixed-width-zero",
            "fixed-width-negative",
            "fixed-width-disabled-value-two",
            "curve-enabled-fixed-width-disabled",
            "curve-pair-enabled",
            "curve-pair-disabled",
        ] {
            assert!(ids.iter().any(|captured| captured == id), "missing {id}");
        }
        for tool in 0..=4 {
            let id = format!("tool-type-{tool}");
            assert!(ids.iter().any(|captured| captured == &id), "missing {id}");
        }
        for tool in [5, 65535] {
            let id = format!("tool-type-{tool}");
            assert!(ids.iter().any(|captured| captured == &id), "missing {id}");
        }
        for tool in 0..=6 {
            let id = format!("raw-tool-type-{tool}");
            assert!(ids.iter().any(|captured| captured == &id), "missing {id}");
        }
        assert!(ids.iter().any(|captured| captured == "raw-tool-type-65535"));
    }

    #[test]
    fn object_list_batch_replays_chained_saved_strokes_and_native_meshes() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../conformance/calligraphy-saved-caller-native.json"
        ))
        .unwrap();
        let capture = &fixture["cases"]["two-stroke-object-list-batch"]["capture"];
        let sources = fixture["cases"]["two-stroke-object-list-batch"]["input"]["sources"]
            .as_array()
            .unwrap();
        let runs = capture["runs"].as_array().unwrap();
        assert_eq!(sources.len(), 2);
        assert_eq!(runs.len(), 2);

        let decode_words = |hex: &str| -> Vec<u32> {
            hex.as_bytes()
                .chunks_exact(8)
                .map(|word| {
                    let bytes = word
                        .chunks_exact(2)
                        .map(|pair| {
                            u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap()
                        })
                        .collect::<Vec<_>>();
                    u32::from_le_bytes(bytes.try_into().unwrap())
                })
                .collect()
        };
        let source_floats = |source: &serde_json::Value, bits_key: &str, bytes_key: &str| {
            if let Some(values) = source.get(bits_key).and_then(serde_json::Value::as_array) {
                values
                    .iter()
                    .map(|value| f32::from_bits(bits(value.as_str().unwrap())))
                    .collect::<Vec<_>>()
            } else {
                decode_words(source[bytes_key].as_str().unwrap())
                    .into_iter()
                    .map(f32::from_bits)
                    .collect()
            }
        };
        let source_points = |source: &serde_json::Value| {
            source_floats(source, "points_f32_bits", "point_bytes_f32_le")
                .chunks_exact(2)
                .map(|pair| [pair[0], pair[1]])
                .collect::<Vec<_>>()
        };
        let state_bits = |state: &serde_json::Value| {
            (
                state["center_bits"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| bits(value.as_str().unwrap()))
                    .collect::<Vec<_>>(),
                state["edge_state_bits"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| bits(value.as_str().unwrap()))
                    .collect::<Vec<_>>(),
                state["initial_flag"].as_u64().unwrap(),
            )
        };
        let captured_glv1_state = |hex: &str| {
            let bytes = hex
                .as_bytes()
                .chunks_exact(2)
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect::<Vec<_>>();
            let word =
                |offset: usize| u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
            super::SavedHistoryState {
                accepted: P {
                    x: f32::from_bits(word(116)),
                    y: f32::from_bits(word(120)),
                },
                start: P {
                    x: f32::from_bits(word(88)),
                    y: f32::from_bits(word(92)),
                },
                control: P {
                    x: f32::from_bits(word(96)),
                    y: f32::from_bits(word(100)),
                },
                midpoint: P {
                    x: f32::from_bits(word(104)),
                    y: f32::from_bits(word(108)),
                },
                nib: super::NibState {
                    angle_degrees: f32::from_bits(word(284)),
                    direction: [f32::from_bits(word(276)), f32::from_bits(word(280))],
                    initial_pending: bytes[80] != 0,
                    movement_armed: bytes[272] != 0,
                },
            }
        };

        // The saved wrapper's native constructor snapshot has both GLV1
        // lifecycle flags clear. RedrawPen rearms them from the first source
        // record before sampling.
        let mut state = super::CalligraphyState::fresh();
        let fresh_saved = saved_history_state_bits(state.saved);
        let native_fresh = saved_history_state_bits(captured_glv1_state(
            runs[0]["glv1_prestate_hex"].as_str().unwrap(),
        ));
        assert_eq!(fresh_saved, native_fresh, "native fresh GLV1 state");
        for index in 0..2 {
            let source = &sources[index];
            let run = &runs[index];
            assert_eq!(run["ordinal"].as_u64().unwrap() as usize, index);
            if index > 0 {
                assert_eq!(
                    state_bits(&runs[index - 1]["rtv1_state_after"]),
                    state_bits(&run["rtv1_state_before"]),
                    "native RTV1 state continuity before stroke {index}"
                );
                let previous_native = &runs[index - 1]["rtv1_state_after"];
                let actual_center = state.add_point.center.map(f32::to_bits).to_vec();
                let actual_edges = state
                    .add_point
                    .previous_edges
                    .iter()
                    .flat_map(|pair| pair.iter().flat_map(|point| point.iter().copied()))
                    .map(f32::to_bits)
                    .collect::<Vec<_>>();
                assert_eq!(actual_center, state_bits(previous_native).0);
                assert_eq!(actual_edges, state_bits(previous_native).1);
                assert!(!state.add_point.initial_edge);
            }
            if index > 0 {
                assert_eq!(
                    saved_history_state_bits(state.saved),
                    saved_history_state_bits(captured_glv1_state(
                        run["glv1_prestate_hex"].as_str().unwrap()
                    )),
                    "run {index} complete chained GLV1 input state"
                );
            }

            let points = source_points(source);
            let pressures = source_floats(source, "pressure_f32_bits", "pressure_bytes_f32_le");
            let orientation =
                source_floats(source, "orientation_f32_bits", "orientation_bytes_f32_le");
            let tilt = source_floats(source, "tilt_f32_bits", "tilt_bytes_f32_le");
            let timestamps = source["timestamps_i32"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_i64().unwrap() as i32)
                .collect::<Vec<_>>();
            let pen_size = f32::from_bits(bits(source["pen_size_f32_bits"].as_str().unwrap()));
            let output = super::saved_object_stroke_mesh(
                super::SavedObjectStrokeInput {
                    points: &points,
                    pressures: &pressures,
                    timestamps: &timestamps,
                    tilt: Some(&tilt),
                    orientation: Some(&orientation),
                    tool_type: source["tool_type"].as_u64().unwrap() as i32,
                    pen_size,
                    fixed_width: None,
                },
                state,
            )
            .expect("valid native ObjectStroke source arrays");

            let native_add_points = run["add_points"].as_array().unwrap();
            assert_eq!(
                output.add_points.len(),
                native_add_points.len(),
                "run {index} AddPoint count"
            );
            let native_builds = run["build_calls"].as_array().unwrap();
            let mut build_cursor = 0;
            let mut native_buffer_targets: [Option<&str>; 3] = [None; 3];
            for (frame, native) in output.add_points.iter().zip(native_add_points) {
                assert_eq!(
                    frame.input.vector.map(f32::to_bits).as_slice(),
                    decode_words(native["vector_bytes"].as_str().unwrap()),
                    "run {index} native AddPoint vector"
                );
                assert_eq!(
                    frame.input.extent.to_bits(),
                    bits(native["extent_bits"].as_str().unwrap()),
                    "run {index} native AddPoint extent"
                );
                let pre = &native["pre_state"];
                assert_eq!(
                    frame.input.gate_center.map(f32::to_bits).as_slice(),
                    state_bits(pre).0,
                    "run {index} AddPoint gate center"
                );
                assert_eq!(
                    frame.input.initial_edge,
                    pre["initial_flag"].as_u64().unwrap() != 0,
                    "run {index} AddPoint initial flag"
                );
                let edges = frame
                    .input
                    .previous_edges
                    .iter()
                    .flat_map(|pair| pair.iter().flat_map(|point| point.iter().copied()))
                    .map(f32::to_bits)
                    .collect::<Vec<_>>();
                assert_eq!(edges, state_bits(pre).1, "run {index} AddPoint edges");

                if let Some(geometry) = frame.geometry {
                    for (buffer, rect) in geometry.buffers.into_iter().zip(geometry.rectangles) {
                        let target = native_builds[build_cursor]["output"].as_str().unwrap();
                        let target_slot = match buffer {
                            super::MeshBuffer::Core => 0,
                            super::MeshBuffer::FringeA => 1,
                            super::MeshBuffer::FringeB => 2,
                        };
                        if let Some(previous) = native_buffer_targets[target_slot] {
                            assert_eq!(previous, target, "run {index} stable native output slot");
                        } else {
                            native_buffer_targets[target_slot] = Some(target);
                        }
                        assert_eq!(
                            rect_bits(rect).as_slice(),
                            decode_words(native_builds[build_cursor]["rect"].as_str().unwrap()),
                            "run {index} native build rectangle {build_cursor}"
                        );
                        build_cursor += 1;
                    }
                }
            }
            assert_eq!(
                build_cursor,
                native_builds.len(),
                "run {index} build call count"
            );

            for slot in 0..3 {
                let actual = match slot {
                    0 => mesh_bits(&output.buffers.core),
                    1 => mesh_bits(&output.buffers.fringe_a),
                    _ => mesh_bits(&output.buffers.fringe_b),
                };
                let target = native_buffer_targets[slot].expect("native output target captured");
                let native_buffer = run["rtv1_mesh"]["buffers"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|buffer| buffer["vector_object"].as_str() == Some(target))
                    .unwrap_or_else(|| panic!("run {index} native target {target} missing"));
                assert_eq!(
                    actual,
                    decode_words(native_buffer["bytes_hex"].as_str().unwrap()),
                    "run {index} native output buffer slot {slot}"
                );
                assert_eq!(
                    actual.len() / 4,
                    native_buffer["vertex_count"].as_u64().unwrap() as usize
                );
            }

            assert_eq!(
                output.state.add_point.center.map(f32::to_bits).as_slice(),
                state_bits(&run["rtv1_state_after"]).0,
                "run {index} retained center"
            );
            assert_eq!(
                output
                    .state
                    .add_point
                    .previous_edges
                    .iter()
                    .flat_map(|pair| pair.iter().flat_map(|point| point.iter().copied()))
                    .map(f32::to_bits)
                    .collect::<Vec<_>>(),
                state_bits(&run["rtv1_state_after"]).1,
                "run {index} retained edge cache"
            );
            assert!(!output.state.add_point.initial_edge);
            assert_eq!(
                saved_history_state_bits(output.state.saved),
                saved_history_state_bits(captured_glv1_state(
                    run["glv1_poststate_hex"].as_str().unwrap()
                )),
                "run {index} complete GLV1 output state"
            );
            state = output.state;
        }
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
                    fixed_width: None,
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
                    fixed_width: None,
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
                    fixed_width: None,
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
                    fixed_width: None,
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
                if run_name == "pen_style" {
                    continue;
                }
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
                let native_style_extent = case["native"]
                    .get("pen_style")
                    .filter(|style| style["input"]["fixed_width_enabled"] == true)
                    .map(|style| (style["input"]["fixed_width"].as_f64().unwrap() as f32).max(1.0))
                    .unwrap_or(actual.point_extent);
                for stamp in terminal {
                    assert_eq!(
                        native_style_extent.to_bits(),
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

    #[test]
    fn model_adapter_chains_native_object_list_buffers_and_source_boundaries() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../conformance/calligraphy-saved-caller-native.json"
        ))
        .unwrap();
        let batch = &fixture["cases"]["two-stroke-object-list-batch"];
        let sources = batch["input"]["sources"].as_array().unwrap();
        let native_runs = batch["capture"]["runs"].as_array().unwrap();
        assert_eq!(sources.len(), 2);
        assert_eq!(native_runs.len(), 2);

        let mut session = super::CalligraphySession::new();
        let first_stroke = fixture_stroke(&sources[0]);
        let first = match super::prepare_model_stroke(&mut session, &first_stroke) {
            super::CalligraphyAdmission::Mesh(mesh) => mesh,
            _ => panic!("captured Calligraphy source must be admitted"),
        };
        assert_eq!(first.rgb, [0x11, 0x22, 0x33]);
        assert_ne!((sources[0]["argb_u32"].as_u64().unwrap() >> 24), 0xff);
        assert!(
            first
                .terminal_start
                .iter()
                .zip(first.terminal_end)
                .all(|(start, end)| *start <= end)
        );
        assert_eq!(
            first.terminal_end,
            [
                first.buffers.core.len() / 3,
                first.buffers.fringe_a.len() / 3,
                first.buffers.fringe_b.len() / 3,
            ]
        );
        assert_eq!(first.sample_ends.len(), first_stroke.points.len() - 2);
        let mut previous = [0usize; 3];
        for end in &first.sample_ends {
            for lane in 0..3 {
                assert!(end[lane] >= previous[lane]);
                assert!(end[lane] <= first.terminal_start[lane]);
            }
            previous = *end;
        }
        for (lane, actual) in [
            mesh_lane_bytes(&first.buffers.core),
            mesh_lane_bytes(&first.buffers.fringe_a),
            mesh_lane_bytes(&first.buffers.fringe_b),
        ]
        .iter()
        .enumerate()
        {
            // The numeric producer's order is Core, FringeA, FringeB, while
            // native first-seen output slots are Core, FringeB, FringeA.
            let build_output_slot = [0, 2, 1][lane];
            let native_buffer = native_runs[0]["rtv1_mesh"]["buffers"]
                .as_array()
                .unwrap()
                .iter()
                .find(|buffer| buffer["build_output_slot"].as_u64() == Some(build_output_slot))
                .unwrap_or_else(|| panic!("native build output slot {build_output_slot} missing"));
            assert_eq!(
                actual,
                &fixture_hex_bytes(native_buffer["bytes_hex"].as_str().unwrap()),
                "first stroke native lane {lane}"
            );
        }

        // Both a native one-point no-op and a different resolved pen leave
        // this Calligraphy drawable's retained state available to the next
        // stroke in the same Drawing session.
        let mut one_point = first_stroke.clone();
        one_point.points.truncate(1);
        one_point.pressures.truncate(1);
        one_point.timestamps.truncate(1);
        one_point.tilts.truncate(1);
        one_point.orientations.truncate(1);
        let no_op = match super::prepare_model_stroke(&mut session, &one_point) {
            super::CalligraphyAdmission::Mesh(mesh) => mesh,
            _ => panic!("captured one-point Calligraphy source must be a no-op"),
        };
        assert!(no_op.buffers.core.is_empty());
        assert!(no_op.buffers.fringe_a.is_empty());
        assert!(no_op.buffers.fringe_b.is_empty());
        assert!(no_op.sample_ends.is_empty());
        assert_eq!(no_op.terminal_start, [0; 3]);
        assert_eq!(no_op.terminal_end, [0; 3]);
        let mut other_pen = first_stroke.clone();
        other_pen.rendering.as_mut().unwrap().pen_name =
            Some("com.samsung.android.sdk.pen.pen.preload.Marker2".into());
        assert!(matches!(
            super::prepare_model_stroke(&mut session, &other_pen),
            super::CalligraphyAdmission::NotCalligraphy
        ));

        let second_stroke = fixture_stroke(&sources[1]);
        let second = match super::prepare_model_stroke(&mut session, &second_stroke) {
            super::CalligraphyAdmission::Mesh(mesh) => mesh,
            _ => panic!("captured reused Calligraphy source must be admitted"),
        };
        for (lane, actual) in [
            mesh_lane_bytes(&second.buffers.core),
            mesh_lane_bytes(&second.buffers.fringe_a),
            mesh_lane_bytes(&second.buffers.fringe_b),
        ]
        .iter()
        .enumerate()
        {
            let build_output_slot = [0, 2, 1][lane];
            let native_buffer = native_runs[1]["rtv1_mesh"]["buffers"]
                .as_array()
                .unwrap()
                .iter()
                .find(|buffer| buffer["build_output_slot"].as_u64() == Some(build_output_slot))
                .unwrap_or_else(|| panic!("native build output slot {build_output_slot} missing"));
            assert_eq!(
                actual,
                &fixture_hex_bytes(native_buffer["bytes_hex"].as_str().unwrap()),
                "reused short stroke native lane {lane}"
            );
        }

        let mut poisoned = super::CalligraphySession::new();
        let mut unsupported = first_stroke;
        unsupported.rendering.as_mut().unwrap().style.pen_size = None;
        assert!(matches!(
            super::prepare_model_stroke(&mut poisoned, &unsupported),
            super::CalligraphyAdmission::Unsupported(super::CalligraphyUnsupported::MissingStyle)
        ));
        assert!(!poisoned.is_valid());
        assert!(matches!(
            super::prepare_model_stroke(&mut poisoned, &second_stroke),
            super::CalligraphyAdmission::Unsupported(super::CalligraphyUnsupported::InvalidSession)
        ));
        poisoned.reset();
        assert!(poisoned.is_valid());
        assert!(matches!(
            super::prepare_model_stroke(&mut poisoned, &second_stroke),
            super::CalligraphyAdmission::Mesh(_)
        ));

        let mut oversized = super::CalligraphySession::new();
        let mut huge_source = fixture_stroke(&sources[0]);
        huge_source.points[2].x = 100_000_000.0;
        assert!(matches!(
            super::prepare_model_stroke(&mut oversized, &huge_source),
            super::CalligraphyAdmission::Unsupported(super::CalligraphyUnsupported::BudgetExceeded)
        ));
        assert!(!oversized.is_valid());
    }
}
