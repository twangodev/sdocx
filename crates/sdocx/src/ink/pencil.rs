//! Numeric reconstruction of Pencil3 sampling and particle kernels.
//! V1 remains conformance-only; V3 has a bounded production saved-source path.

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct RectF {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Particle {
    /// Packed RTV1 order: center x, center y, half-size, random row, alpha.
    pub attributes: [f32; 5],
}

/// A Pencil3 V3 particle. V3 has the same packed lane count as V1, but it uses
/// a private 64-bit LCG rather than the process bionic RNG modeled below.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct V3Particle {
    pub attributes: [f32; 5],
}

/// Source-derived input shared by Pencil3 versions before their distinct
/// particle kernels consume random state or pack RTV records.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct SavedSample {
    pub(super) point: [f32; 2],
    pub(super) size: f32,
    pub(super) alpha: f32,
}

/// Per-Redraw private Pencil3 V3 random state. Native RedrawPen resets the
/// state at every draw, so this generator is deliberately independent from
/// `BionicRandom` and has an explicit reset operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct V3PrivateRandom {
    state: u64,
}

impl V3PrivateRandom {
    const INITIAL_STATE: u64 = 1;
    const MULTIPLIER: u64 = 0x343fd;
    const INCREMENT: u64 = 0x269ec3;

    pub(super) fn new() -> Self {
        Self {
            state: Self::INITIAL_STATE,
        }
    }

    pub(super) fn reset(&mut self) {
        self.state = Self::INITIAL_STATE;
    }

    fn advance(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(Self::MULTIPLIER)
            .wrapping_add(Self::INCREMENT);
        self.state
    }

    /// Build one V3 particle from actual drawPoint arguments. Native order is
    /// three LCG advances (x, y, texture row), then an RTV1 AddPoint record.
    pub(super) fn particle(&mut self, point: [f32; 2], size: f32, alpha: f32) -> V3Particle {
        let size1 = size + 1.0;
        let range = size1.mul_add(size1.min(5.0), 30.0);
        let divisor = range as u32;
        let centering = divisor as f32 / 40.0;
        let x_state = self.advance();
        let x_random = ((x_state >> 16) & 0x7fff_ffff) as u32;
        let y_state = self.advance();
        let y_random = ((y_state >> 16) & 0x7fff_ffff) as u32;
        let row_state = self.advance();
        let x = point[0] + (x_random % divisor) as f32 / 20.0 - centering;
        let y = point[1] + (y_random % divisor) as f32 / 20.0 - centering;
        let row = ((row_state >> 16) & 0xf) as f32;
        V3Particle {
            attributes: [x, y, size1 * 0.5, row, alpha],
        }
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug)]
pub(super) struct DrawPoint {
    pub point: [f32; 2],
    pub size: f32,
    pub alpha: f32,
    pub initial_bounds: RectF,
    /// Three C rand() results, in native call order.
    pub rand_values: [u32; 3],
}

#[cfg(test)]
#[derive(Clone, Copy, Debug)]
pub(super) struct DrawPointResult {
    pub particle: Particle,
    pub bounds: RectF,
    pub rand_calls: u8,
}

/// Saved ObjectStroke channels. `None` is kept distinct from a present array
/// of zeroes because the native adapter passes a null getter result only for
/// an absent channel.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct SavedStroke {
    pub points: Vec<[f32; 2]>,
    pub pressures: Vec<f32>,
    pub timestamps: Vec<i32>,
    pub tilts: Option<Vec<f32>>,
    pub orientations: Option<Vec<f32>>,
    pub tool_type: i32,
    pub size: f32,
}

impl SavedStroke {
    pub(super) fn validate(&self) -> Result<(), &'static str> {
        let n = self.points.len();
        if n == 0 || self.pressures.len() != n || self.timestamps.len() != n {
            return Err("saved point, pressure and timestamp channels must be aligned");
        }
        if self.tilts.as_ref().is_some_and(|v| v.len() != n)
            || self.orientations.as_ref().is_some_and(|v| v.len() != n)
        {
            return Err("present optional channel must align with saved points");
        }
        Ok(())
    }

    fn tilt_at(&self, index: usize) -> f32 {
        self.tilts.as_ref().map_or(0.0, |v| v[index])
    }

    #[cfg(test)]
    fn orientation_at(&self, index: usize) -> f32 {
        self.orientations.as_ref().map_or(0.0, |v| v[index])
    }
}

/// Execute the observed drawPoint and RTV1::AddPoint arithmetic with caller
/// supplied RNG outputs. Native drawPoint at Pencil3+0x108338 uses
/// `size1 = size + 1`, `range = 30 + size1 * min(size1, 5)`, then emits
/// `(point + rand % trunc(range) / 20 - trunc(range) / 40)` on each axis.
#[cfg(test)]
pub(super) fn draw_point(input: DrawPoint) -> DrawPointResult {
    let size1 = input.size + 1.0;
    let range = 30.0 + size1 * size1.min(5.0);
    let divisor = range as i32;
    let half = size1 * 0.5;
    let x = input.point[0] + (input.rand_values[0] % divisor as u32) as f32 / 20.0
        - (range as i32 as f32) / 40.0;
    let y = input.point[1] + (input.rand_values[1] % divisor as u32) as f32 / 20.0
        - (range as i32 as f32) / 40.0;
    let row = (input.rand_values[2] & 0xf) as f32;
    let rect = RectF {
        left: x - half,
        top: y - half,
        right: x + half,
        bottom: y + half,
    };
    let bounds = RectF {
        left: input.initial_bounds.left.min(rect.left),
        top: input.initial_bounds.top.min(rect.top),
        right: input.initial_bounds.right.max(rect.right),
        bottom: input.initial_bounds.bottom.max(rect.bottom),
    };
    DrawPointResult {
        particle: Particle {
            attributes: [x, y, half, row, input.alpha],
        },
        bounds,
        rand_calls: 3,
    }
}

/// A local AOSP bionic `random()` TYPE_3 state. It models the inspected AOSP
/// source profile; it does not assert that a particular Samsung OS release
/// executes this implementation.
#[cfg(test)]
#[derive(Clone, Debug)]
pub(super) struct BionicRandom {
    state: [u32; 31],
    front: usize,
    rear: usize,
}

#[cfg(test)]
impl BionicRandom {
    pub fn seeded(seed: u32) -> Self {
        let mut state = [0; 31];
        state[0] = seed;
        for i in 1..31 {
            let previous = state[i - 1] as i64;
            let next = 16807 * (previous % 127773) - 2836 * (previous / 127773);
            state[i] = if next <= 0 {
                (next + 0x7fff_ffff) as u32
            } else {
                next as u32
            };
        }
        let mut rng = Self {
            state,
            front: 3,
            rear: 0,
        };
        for _ in 0..310 {
            rng.next();
        }
        rng
    }

    pub fn next(&mut self) -> u32 {
        let sum = self.state[self.front].wrapping_add(self.state[self.rear]);
        self.state[self.front] = sum;
        let value = (sum >> 1) & 0x7fff_ffff;
        self.front = (self.front + 1) % 31;
        self.rear = (self.rear + 1) % 31;
        value
    }
}

#[cfg(test)]
fn seven_start_dabs(
    point: [f32; 2],
    size: f32,
    alpha: f32,
    rng: &mut BionicRandom,
) -> Vec<Particle> {
    (0..7)
        .map(|_| {
            draw_point(DrawPoint {
                point,
                size,
                alpha,
                initial_bounds: RectF {
                    left: 0.0,
                    top: 0.0,
                    right: 0.0,
                    bottom: 0.0,
                },
                rand_values: [rng.next(), rng.next(), rng.next()],
            })
            .particle
        })
        .collect()
}

/// Sample one saved quadratic path using native `drawLine` spacing. The
/// caller supplies the resolved per-segment drawPoint size and alpha because
/// the saved event adapter's pressure profile is a separate native contract.
#[cfg(test)]
fn line_particles(
    start: [f32; 2],
    control: [f32; 2],
    end: [f32; 2],
    point_size: f32,
    alpha: f32,
    spacing: f32,
    rng: &mut BionicRandom,
) -> Vec<Particle> {
    line_particles_with_phase(
        start, control, end, point_size, alpha, spacing, spacing, rng,
    )
    .0
}

/// Sample one native path while preserving `drawLine`'s distance remainder.
/// V1 carries `distance - path_length` from an accepted line into the next
/// accepted line or `endPen`; restarting each path at one full step loses a
/// boundary particle on curved multi-segment strokes.
// Keep recovered particle inputs and mutable RNG state visible at this seam.
#[allow(clippy::too_many_arguments)]
#[cfg(test)]
fn line_particles_with_phase(
    start: [f32; 2],
    control: [f32; 2],
    end: [f32; 2],
    point_size: f32,
    alpha: f32,
    spacing: f32,
    initial_distance: f32,
    rng: &mut BionicRandom,
) -> (Vec<Particle>, f32) {
    let mut particles = Vec::new();
    let phase = sample_line_with_phase(
        start,
        control,
        end,
        point_size,
        alpha,
        spacing,
        initial_distance,
        |sample| particles.push(v1_particle_from_sample(sample, rng)),
    );
    (particles, phase)
}

/// Run the shared native SmPath sampling loop without selecting a particle
/// RNG or output packing. This lets V1 and V3 share saved-source iteration
/// while keeping their Bionic and private-LCG particle kernels separate.
// The saved sampler uses explicit geometry, brush, RNG, and emission inputs.
#[allow(clippy::too_many_arguments)]
#[cfg(test)]
fn sample_line_with_phase(
    start: [f32; 2],
    control: [f32; 2],
    end: [f32; 2],
    point_size: f32,
    alpha: f32,
    spacing: f32,
    initial_distance: f32,
    mut emit: impl FnMut(SavedSample),
) -> f32 {
    try_sample_line_with_phase(
        start,
        control,
        end,
        point_size,
        alpha,
        spacing,
        initial_distance,
        |sample| {
            emit(sample);
            true
        },
    )
    .expect("finite conformance sampling path")
}

// A rejected emission stops before the next RNG advance or allocation.
#[allow(clippy::too_many_arguments)]
pub(super) fn try_sample_line_with_phase(
    start: [f32; 2],
    control: [f32; 2],
    end: [f32; 2],
    point_size: f32,
    alpha: f32,
    spacing: f32,
    initial_distance: f32,
    mut emit: impl FnMut(SavedSample) -> bool,
) -> Option<f32> {
    use super::path::{P, Quad};
    let path = Quad::new(
        P {
            x: start[0],
            y: start[1],
        },
        P {
            x: control[0],
            y: control[1],
        },
        P {
            x: end[0],
            y: end[1],
        },
    );
    if !path.length.is_finite() || !spacing.is_finite() || spacing <= 0.0 {
        return None;
    }
    let mut distance = initial_distance;
    while distance <= path.length {
        let point = path.at(distance);
        if !emit(SavedSample {
            point: [point.x, point.y],
            size: point_size,
            alpha,
        }) {
            return None;
        }
        let next = distance + spacing;
        if next <= distance {
            return None;
        }
        distance = next;
    }
    Some(distance - path.length)
}

#[cfg(test)]
fn v1_particle_from_sample(sample: SavedSample, rng: &mut BionicRandom) -> Particle {
    draw_point(DrawPoint {
        point: sample.point,
        size: sample.size,
        alpha: sample.alpha,
        initial_bounds: RectF {
            left: 0.0,
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
        },
        rand_values: [rng.next(), rng.next(), rng.next()],
    })
    .particle
}

/// Reconstruct the native V1 saved-event lifecycle for the bounded finite
/// profile used by the conformance fixture.  Values are derived from the
/// saved stroke; fixture output is never consulted by this implementation.
#[cfg(test)]
fn saved_redraw_particles(stroke: &SavedStroke, rng: &mut BionicRandom) -> Vec<Particle> {
    let mut particles = Vec::new();
    sample_saved_redraw(stroke, |sample| {
        particles.push(v1_particle_from_sample(sample, rng));
    });
    particles
}

/// Run the source-side saved-stroke sampler and hand each upstream point,
/// width, and opacity to a caller-owned particle kernel.
#[cfg(test)]
pub(super) fn sample_saved_redraw(stroke: &SavedStroke, mut emit: impl FnMut(SavedSample)) {
    assert!(try_sample_saved_redraw(stroke, |sample| {
        emit(sample);
        true
    }));
}

#[cfg(test)]
pub(super) fn try_sample_saved_redraw(
    stroke: &SavedStroke,
    mut emit: impl FnMut(SavedSample) -> bool,
) -> bool {
    try_sample_saved_redraw_with_source_index(stroke, |_, sample| emit(sample))
}

/// Run the saved-source sampler while preserving the index of the source
/// point whose event processing emits each sample. The ordinary sampler
/// delegates here and discards the index, so particle generation stays on
/// the same arithmetic and emission path.
pub(super) fn try_sample_saved_redraw_with_source_index(
    stroke: &SavedStroke,
    mut emit: impl FnMut(usize, SavedSample) -> bool,
) -> bool {
    if stroke.points.len() < 2 {
        return true;
    }
    let pressure_for_move = |pressure: f32| {
        if pressure.is_finite() {
            pressure.min(1.0)
        } else {
            0.5
        }
    };
    let initial_pressure = if matches!(stroke.tool_type, 1 | 3) {
        0.5
    } else {
        pressure_for_move(stroke.pressures[0])
    };
    let initial_alpha = initial_pressure.clamp(0.1, 0.6) / 1.5;
    let mut point_size = stroke.size.mul_add(initial_pressure, stroke.size) * 0.5;
    let step = stroke.size * 0.08;
    let mut distance_phase = step;
    for _ in 0..7 {
        if !emit(
            0,
            SavedSample {
                point: stroke.points[0],
                size: stroke.size,
                alpha: initial_alpha,
            },
        ) {
            return false;
        }
    }

    let mut raw = stroke.points[0];
    let mut midpoint = raw;
    let mut short_segment_flag = true;
    let initial_tilt = if stroke.tilt_at(0).is_finite() {
        stroke.tilt_at(0).min(0.79)
    } else {
        0.0
    };
    let mut tilt_ring = [initial_tilt; 50];
    let mut tilt_index = 0usize;
    for index in 1..stroke.points.len() - 1 {
        let current = stroke.points[index];
        let dx = current[0] - raw[0];
        let dy = current[1] - raw[1];
        let length = dx.mul_add(dx, dy * dy).sqrt();
        // drawLine first rejects movement below member[104] * member[108]
        // (member[104] is 0.01 * base-size and member[108] is 4), then applies
        // its 50 * member[108] short-movement window. At 0x10851c the
        // integer literal 0x42480000 is loaded; 0x108520 moves it to s2 as
        // 50.0f, then 0x108524 multiplies by member[108].
        if length < step * 0.5 {
            continue;
        }
        let short = length < 4.0 * 50.0;
        if matches!(stroke.tool_type, 1 | 3) && short {
            let was_set = short_segment_flag;
            short_segment_flag = !short_segment_flag;
            if was_set {
                continue;
            }
        } else if matches!(stroke.tool_type, 1 | 3) {
            short_segment_flag = true;
        }

        let pressure = if matches!(stroke.tool_type, 1 | 3) {
            0.5
        } else {
            pressure_for_move(stroke.pressures[index])
        };
        let alpha = pressure.clamp(0.1, 0.6) / 1.5;
        distance_phase = match try_sample_line_with_phase(
            midpoint,
            raw,
            [(raw[0] + current[0]) * 0.5, (raw[1] + current[1]) * 0.5],
            point_size,
            alpha,
            step,
            distance_phase,
            |sample| emit(index, sample),
        ) {
            Some(phase) => phase,
            None => return false,
        };
        midpoint = [(raw[0] + current[0]) * 0.5, (raw[1] + current[1]) * 0.5];
        raw = current;

        let source_tilt = stroke.tilt_at(index);
        let tilt = if source_tilt.is_finite() {
            source_tilt.min(0.79)
        } else {
            0.0
        };
        tilt_ring[tilt_index] = tilt;
        tilt_index = (tilt_index + 1) % tilt_ring.len();
        let average = tilt_ring.iter().copied().sum::<f32>() / 50.0;
        let tilt_scale = if average >= 0.55 {
            // Pinned rodata 0x12b50 is one ULP above the literal 0.48f32.
            (average - 0.55) / f32::from_bits(0x3ef5_c290)
        } else {
            0.0
        };
        // Matches getPointSize(base, adjustedTilt): base * (1 + tiltScale).
        let width_bias = (-2.0f32).mul_add(tilt_scale, 1.0);
        let width_modulation = stroke.size * 3.0 * tilt_scale;
        point_size = stroke.size.mul_add(width_bias, width_modulation);
    }

    let terminal_pressure = if matches!(stroke.tool_type, 1 | 3) {
        0.5
    } else if stroke.pressures.last().copied().unwrap_or(0.5).is_finite() {
        stroke.pressures.last().copied().unwrap_or(0.5).min(1.0)
    } else {
        0.5
    };
    let terminal_alpha = terminal_pressure.clamp(0.1, 0.6) / 1.5;
    let terminal = *stroke.points.last().unwrap();
    try_sample_line_with_phase(
        midpoint,
        raw,
        terminal,
        point_size,
        terminal_alpha,
        step,
        distance_phase,
        |sample| emit(stroke.points.len() - 1, sample),
    )
    .is_some()
}

/// Compose the shared saved-source sampler with V3's private particle kernel.
/// Channel validation happens before sampling so malformed arrays cannot
/// trigger partial output or an out-of-range getter read.
#[cfg(test)]
pub(super) fn saved_redraw_v3_particles(
    stroke: &SavedStroke,
    rng: &mut V3PrivateRandom,
) -> Result<Vec<V3Particle>, &'static str> {
    saved_redraw_v3_particles_bounded(stroke, rng, usize::MAX)
}

/// Bound generated particles before advancing random state or allocating output.
/// Failed attempts leave the caller's RNG untouched and expose no partial stream.
#[cfg(test)]
pub(super) fn saved_redraw_v3_particles_bounded(
    stroke: &SavedStroke,
    rng: &mut V3PrivateRandom,
    max_particles: usize,
) -> Result<Vec<V3Particle>, &'static str> {
    saved_redraw_v3_bounded_inner(stroke, rng, max_particles, false).map(|(particles, _)| particles)
}

/// Generate particles and retain one exclusive cumulative particle end per
/// source point. Ends are captured in the same bounded sampling pass as the
/// particles, so replay metadata cannot come from a second sampling run.
pub(super) fn saved_redraw_v3_particles_with_source_ends_bounded(
    stroke: &SavedStroke,
    rng: &mut V3PrivateRandom,
    max_particles: usize,
) -> Result<(Vec<V3Particle>, Vec<usize>), &'static str> {
    saved_redraw_v3_bounded_inner(stroke, rng, max_particles, true)
        .map(|(particles, source_ends)| (particles, source_ends.expect("requested source ends")))
}

fn saved_redraw_v3_bounded_inner(
    stroke: &SavedStroke,
    rng: &mut V3PrivateRandom,
    max_particles: usize,
    retain_source_ends: bool,
) -> Result<(Vec<V3Particle>, Option<Vec<usize>>), &'static str> {
    stroke.validate()?;
    let spacing = stroke.size * 0.08;
    if !stroke.size.is_finite() || stroke.size <= 0.0 || !spacing.is_finite() || spacing <= 0.0 {
        return Err("saved pen size must produce a finite positive spacing");
    }
    if stroke.points.iter().flatten().any(|v| !v.is_finite()) {
        return Err("saved coordinates must be finite");
    }
    // Native V3 resets on every redraw. Commit that state only on success;
    // a resource rejection is an adapter failure, not a native partial redraw.
    let mut next_rng = *rng;
    next_rng.reset();
    let mut particles = Vec::new();
    let mut source_ends = retain_source_ends.then(|| vec![0; stroke.points.len()]);
    let mut next_source_end = 0;
    if !try_sample_saved_redraw_with_source_index(stroke, |source_index, sample| {
        if particles.len() == max_particles {
            return false;
        }
        if let Some(ends) = &mut source_ends {
            ends[next_source_end..source_index].fill(particles.len());
            next_source_end = source_index;
        }
        particles.push(next_rng.particle(sample.point, sample.size, sample.alpha));
        true
    }) {
        return Err("saved particle budget or finite sampling path exceeded");
    }
    if let Some(ends) = &mut source_ends {
        ends[next_source_end..].fill(particles.len());
    }
    *rng = next_rng;
    Ok((particles, source_ends))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use sha2::{Digest, Sha256};

    fn close_bits(actual: f32, expected: &Value, context: &str) {
        let expected = expected.as_f64().unwrap() as f32;
        assert_eq!(actual.to_bits(), expected.to_bits(), "{context}");
    }

    fn prefixed_hex_u32(value: &Value) -> u32 {
        u32::from_str_radix(value.as_str().unwrap().trim_start_matches("0x"), 16).unwrap()
    }

    fn prefixed_hex_u64(value: &Value) -> u64 {
        u64::from_str_radix(value.as_str().unwrap().trim_start_matches("0x"), 16).unwrap()
    }

    fn little_endian_hex_word(value: &Value) -> u32 {
        let value = value.as_str().unwrap();
        assert_eq!(value.len(), 8);
        u32::from_le_bytes(std::array::from_fn(|index| {
            u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).unwrap()
        }))
    }

    fn little_endian_hex_pair(value: &Value) -> [f32; 2] {
        let value = value.as_str().unwrap();
        assert_eq!(value.len(), 16);
        std::array::from_fn(|index| {
            let start = index * 8;
            f32::from_bits(u32::from_le_bytes(std::array::from_fn(|byte| {
                u8::from_str_radix(&value[start + byte * 2..start + byte * 2 + 2], 16).unwrap()
            })))
        })
    }

    fn bounds_from_json(value: &Value) -> RectF {
        let v = value.as_array().unwrap();
        RectF {
            left: v[0].as_f64().unwrap() as f32,
            top: v[1].as_f64().unwrap() as f32,
            right: v[2].as_f64().unwrap() as f32,
            bottom: v[3].as_f64().unwrap() as f32,
        }
    }

    #[test]
    fn direct_particle_outputs_match_native_drawpoint_addpoint() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/pencil-v1-native.json"
        ))
        .unwrap();
        for (case, expected) in fixture["cases"]
            .as_array()
            .unwrap()
            .iter()
            .zip(fixture["native_outputs"].as_array().unwrap())
        {
            let point = case["point"].as_array().unwrap();
            let random = case["rand_values"].as_array().unwrap();
            let result = draw_point(DrawPoint {
                point: [
                    point[0].as_f64().unwrap() as f32,
                    point[1].as_f64().unwrap() as f32,
                ],
                size: case["size"].as_f64().unwrap() as f32,
                alpha: case["alpha"].as_f64().unwrap() as f32,
                initial_bounds: bounds_from_json(&case["initial_bounds"]),
                rand_values: [
                    random[0].as_u64().unwrap() as u32,
                    random[1].as_u64().unwrap() as u32,
                    random[2].as_u64().unwrap() as u32,
                ],
            });
            let label = case["name"].as_str().unwrap();
            for i in 0..5 {
                close_bits(
                    result.particle.attributes[i],
                    &expected["packed_attributes"][i],
                    &format!("{label} packed attribute {i}"),
                );
            }
            let bounds = [
                result.bounds.left,
                result.bounds.top,
                result.bounds.right,
                result.bounds.bottom,
            ];
            for (i, actual) in bounds.into_iter().enumerate() {
                close_bits(
                    actual,
                    &expected["bounds"][i],
                    &format!("{label} bound {i}"),
                );
            }
            assert_eq!(
                result.rand_calls,
                expected["rand_calls"].as_u64().unwrap() as u8
            );
            assert_eq!(expected["vector_length"].as_u64(), Some(5));
        }
    }

    #[test]
    fn bionic_type3_matches_android10_cts_vector_and_saved_prefix() {
        let mut rng = BionicRandom::seeded(0x0102_0304);
        assert_eq!(
            (0..4).map(|_| rng.next()).collect::<Vec<_>>(),
            [55_436_735, 1_399_865_117, 2_032_643_283, 571_329_216]
        );

        // Every non-singleton saved witness starts with seven native dots at
        // its first saved point. This pins the saved redraw seed/call prefix
        // without claiming the queued drawLine callback was executed here.
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/pencil-v1-native.json"
        ))
        .unwrap();
        for (case, output) in fixture["saved_cases"]
            .as_array()
            .unwrap()
            .iter()
            .zip(fixture["saved_native_outputs"].as_array().unwrap())
        {
            let stroke = &case["stroke"];
            let points = stroke["points"].as_array().unwrap();
            let output_points = output["packed_attributes"].as_array().unwrap();
            if points.len() < 2 {
                assert!(output_points.is_empty());
                continue;
            }
            let mut rng = BionicRandom::seeded(1);
            // Each initial dot consumes exactly three rand results. Recover
            // the native pressure alpha from the captured first particle;
            // this test focuses on coordinate/size/row arithmetic and the
            // seeded RNG sequence, while pressure-alpha callback remains a
            // separately captured native contract.
            let alpha = output_points[4].as_f64().unwrap() as f32;
            let first = &points[0];
            let mut prefix = Vec::new();
            for _ in 0..7 {
                let result = draw_point(DrawPoint {
                    point: [
                        first[0].as_f64().unwrap() as f32,
                        first[1].as_f64().unwrap() as f32,
                    ],
                    size: stroke["size"].as_f64().unwrap() as f32,
                    alpha,
                    initial_bounds: RectF {
                        left: 0.,
                        top: 0.,
                        right: 0.,
                        bottom: 0.,
                    },
                    rand_values: [rng.next(), rng.next(), rng.next()],
                });
                prefix.extend(result.particle.attributes);
            }
            for (i, actual) in prefix.iter().enumerate() {
                close_bits(
                    *actual,
                    &output_points[i],
                    &format!("{} saved prefix {i}", case["name"].as_str().unwrap()),
                );
            }
        }
    }

    #[test]
    fn two_point_saved_redraw_matches_native_path_samples_and_particle_buffer() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/pencil-v1-native.json"
        ))
        .unwrap();
        let case = &fixture["saved_cases"][1];
        let expected = &fixture["saved_native_outputs"][1];
        let stroke = saved_stroke_from_json(&case["stroke"]);
        assert_eq!(stroke.validate(), Ok(()));
        assert_eq!(stroke.points.len(), 2);
        let mut rng = BionicRandom::seeded(1);
        let alpha = expected["packed_attributes"][4].as_f64().unwrap() as f32;
        let mut particles = seven_start_dabs(stroke.points[0], stroke.size, alpha, &mut rng);
        // Native V1 stores a 0.08 * saved-size drawLine step and the observed
        // two-point path supplies a 0.75 * saved-size drawPoint width.
        let spacing = stroke.size * 0.08;
        let point_size = stroke.size * 0.75;
        particles.extend(line_particles(
            stroke.points[0],
            stroke.points[0],
            stroke.points[1],
            point_size,
            alpha,
            spacing,
            &mut rng,
        ));
        let actual: Vec<f32> = particles.iter().flat_map(|p| p.attributes).collect();
        let expected = expected["packed_attributes"].as_array().unwrap();
        assert_eq!(actual.len(), expected.len());
        for (i, value) in actual.into_iter().enumerate() {
            close_bits(
                value,
                &expected[i],
                &format!("saved two-point packed attribute {i}"),
            );
        }
        assert_eq!(particles.len(), 42);
        assert_eq!(expected.len() / 5, 42);
    }

    #[test]
    fn three_point_saved_redraw_matches_native_terminal_curve_and_buffer() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/pencil-v1-native.json"
        ))
        .unwrap();
        let case = &fixture["saved_cases"][2];
        let expected = &fixture["saved_native_outputs"][2];
        let stroke = saved_stroke_from_json(&case["stroke"]);
        assert_eq!(stroke.validate(), Ok(()));
        assert_eq!(stroke.points.len(), 3);

        // The first historical drawLine call is within the native short-move
        // debounce: it flips the object's flag and returns without changing
        // the control point. endPen therefore samples directly from p0 to p2.
        let mut rng = BionicRandom::seeded(1);
        let alpha = expected["packed_attributes"][4].as_f64().unwrap() as f32;
        let mut particles = seven_start_dabs(stroke.points[0], stroke.size, alpha, &mut rng);
        particles.extend(line_particles(
            stroke.points[0],
            stroke.points[0],
            stroke.points[2],
            stroke.size * 0.75,
            alpha,
            stroke.size * 0.08,
            &mut rng,
        ));

        let actual: Vec<f32> = particles.iter().flat_map(|p| p.attributes).collect();
        let expected = expected["packed_attributes"].as_array().unwrap();
        assert_eq!(actual.len(), expected.len());
        for (i, value) in actual.into_iter().enumerate() {
            close_bits(
                value,
                &expected[i],
                &format!("saved three-point packed attribute {i}"),
            );
        }
        assert_eq!(particles.len(), 76);
    }

    #[test]
    fn four_and_five_point_saved_redraw_match_debounced_and_terminal_curves() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/pencil-v1-native.json"
        ))
        .unwrap();
        for index in [3, 4] {
            let case = &fixture["saved_cases"][index];
            let expected = &fixture["saved_native_outputs"][index];
            let stroke = saved_stroke_from_json(&case["stroke"]);
            assert_eq!(stroke.validate(), Ok(()));
            assert_eq!(stroke.points.len(), index + 1);
            let mut rng = BionicRandom::seeded(1);
            let alpha = expected["packed_attributes"][4].as_f64().unwrap() as f32;
            let mut particles = seven_start_dabs(stroke.points[0], stroke.size, alpha, &mut rng);

            // The first short historical segment is skipped. The next one is
            // accepted, producing a path from the last midpoint through the
            // prior point to the new midpoint. endPen then joins that state
            // to the final current sample.
            let previous = stroke.points[0];
            let accepted = stroke.points[2];
            let midpoint = [
                (previous[0] + accepted[0]) * 0.5,
                (previous[1] + accepted[1]) * 0.5,
            ];
            let (internal, phase) = line_particles_with_phase(
                previous,
                previous,
                midpoint,
                stroke.size * 0.75,
                alpha,
                stroke.size * 0.08,
                stroke.size * 0.08,
                &mut rng,
            );
            let internal_count = internal.len();
            particles.extend(internal);
            let (terminal, _) = line_particles_with_phase(
                midpoint,
                accepted,
                *stroke.points.last().unwrap(),
                stroke.size,
                alpha,
                stroke.size * 0.08,
                phase,
                &mut rng,
            );
            let terminal_count = terminal.len();
            particles.extend(terminal);
            assert_eq!(
                (internal_count, terminal_count),
                if index == 3 { (34, 98) } else { (6, 21) },
                "{} sample partition",
                case["name"]
            );

            let actual: Vec<f32> = particles.iter().flat_map(|p| p.attributes).collect();
            let expected = expected["packed_attributes"].as_array().unwrap();
            assert_eq!(actual.len(), expected.len(), "{}", case["name"]);
            for (i, value) in actual.into_iter().enumerate() {
                close_bits(
                    value,
                    &expected[i],
                    &format!("{} packed attribute {i}", case["name"].as_str().unwrap()),
                );
            }
            assert_eq!(particles.len(), if index == 3 { 139 } else { 34 });
        }
    }

    #[test]
    fn saved_redraw_profile_emits_native_particles_for_all_bounded_cases() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/pencil-v1-native.json"
        ))
        .unwrap();
        for (case, expected) in fixture["saved_cases"]
            .as_array()
            .unwrap()
            .iter()
            .zip(fixture["saved_native_outputs"].as_array().unwrap())
        {
            let stroke = saved_stroke_from_json(&case["stroke"]);
            let mut rng = BionicRandom::seeded(1);
            let particles = saved_redraw_particles(&stroke, &mut rng);
            let actual: Vec<f32> = particles.iter().flat_map(|p| p.attributes).collect();
            let expected = expected["packed_attributes"].as_array().unwrap();
            assert_eq!(actual.len(), expected.len(), "{} length", case["name"]);
            for (index, value) in actual.into_iter().enumerate() {
                close_bits(
                    value,
                    &expected[index],
                    &format!(
                        "{} packed attribute {index}",
                        case["name"].as_str().unwrap()
                    ),
                );
            }
        }
    }

    #[test]
    fn saved_v1_drawing_caller_matches_full_native_particle_and_rng_streams() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/pencil3-saved-caller-native.json"
        ))
        .unwrap();
        let case = &fixture["cases"]["straight_curve_tool2_null"];
        assert_eq!(case["route"]["selected_drawable_vtable_id"], "v1");
        assert_eq!(case["input"]["tool_type"].as_i64(), Some(2));
        assert_eq!(case["profile"]["width"].as_u64(), Some(1920));
        assert_eq!(case["profile"]["height"].as_u64(), Some(1080));

        let bits = |value: &Value| {
            let value = value.as_str().unwrap();
            let bytes: [u8; 4] = std::array::from_fn(|index| {
                u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).unwrap()
            });
            u32::from_le_bytes(bytes)
        };
        let floats = |value: &Value| {
            value
                .as_array()
                .unwrap()
                .iter()
                .map(|value| f32::from_bits(bits(value)))
                .collect::<Vec<_>>()
        };
        let input = &case["input"];
        let points = input["points_f32_bits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|point| {
                [
                    f32::from_bits(bits(&point[0])),
                    f32::from_bits(bits(&point[1])),
                ]
            })
            .collect();
        let stroke = SavedStroke {
            points,
            pressures: floats(&input["pressure_f32_bits"]),
            timestamps: input["timestamps_i32"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_i64().unwrap() as i32)
                .collect(),
            tilts: Some(floats(&input["tilt_f32_bits"])),
            orientations: Some(floats(&input["orientation_f32_bits"])),
            tool_type: input["tool_type"].as_i64().unwrap() as i32,
            size: f32::from_bits(bits(&input["pen_size_f32_bits"])),
        };
        let mut rng = BionicRandom::seeded(1);
        let particles = saved_redraw_particles(&stroke, &mut rng);
        let expected = case["streams"]["draw_object_stroke"]["values_f32_bits"]
            .as_array()
            .unwrap();
        assert_eq!(particles.len(), 48);
        assert_eq!(expected.len(), particles.len() * 5);
        for (index, actual) in particles
            .iter()
            .flat_map(|particle| particle.attributes)
            .enumerate()
        {
            assert_eq!(
                actual.to_bits(),
                bits(&expected[index]),
                "native Drawing V1 attribute word {index}"
            );
        }

        let native_random = &case["rng"]["by_phase"]["draw_object_stroke"]["rand_values"];
        assert_eq!(native_random.as_array().unwrap().len(), particles.len() * 3);
        let mut rng = BionicRandom::seeded(1);
        for (index, expected) in native_random.as_array().unwrap().iter().enumerate() {
            assert_eq!(
                rng.next(),
                expected.as_u64().unwrap() as u32,
                "native rand {index}"
            );
        }
    }

    #[test]
    fn stationary_tool3_saved_redraw_matches_native_particle_buffer() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/pencil-v1-native.json"
        ))
        .unwrap();
        let case = &fixture["saved_cases"][7];
        let output = &fixture["saved_native_outputs"][7];
        let stroke = saved_stroke_from_json(&case["stroke"]);
        assert_eq!(stroke.tool_type, 3);
        let mut rng = BionicRandom::seeded(1);
        let particles = saved_redraw_particles(&stroke, &mut rng);
        let actual: Vec<f32> = particles
            .iter()
            .flat_map(|particle| particle.attributes)
            .collect();
        let expected = output["packed_attributes"].as_array().unwrap();
        assert_eq!(actual.len(), expected.len());
        assert_eq!(particles.len(), 7);
        for (index, value) in actual.into_iter().enumerate() {
            close_bits(value, &expected[index], &format!("tool3 start dab {index}"));
        }
    }

    #[test]
    fn line_debounce_200_unit_boundary_matches_native_oracle_witnesses() {
        for (distance, expected_count) in [(199.0, 628), (201.0, 635)] {
            let stroke = SavedStroke {
                points: vec![[0.0, 0.0], [distance, 0.0], [distance, 0.0]],
                pressures: vec![0.5; 3],
                timestamps: vec![0, 1, 2],
                tilts: Some(vec![0.0; 3]),
                orientations: None,
                tool_type: 1,
                size: 4.0,
            };
            let mut rng = BionicRandom::seeded(1);
            let particles = saved_redraw_particles(&stroke, &mut rng);
            assert_eq!(particles.len(), expected_count, "distance {distance}");
        }
    }

    #[test]
    fn tilt_ring_above_native_profile_threshold_changes_following_width() {
        let stroke = SavedStroke {
            points: vec![[0.0, 0.0], [250.0, 0.0], [500.0, 0.0], [600.0, 0.0]],
            pressures: vec![0.5; 4],
            timestamps: vec![0, 1, 2, 3],
            tilts: Some(vec![0.79; 4]),
            orientations: None,
            tool_type: 1,
            size: 4.0,
        };
        let mut rng = BionicRandom::seeded(1);
        let particles = saved_redraw_particles(&stroke, &mut rng);
        assert_eq!(particles.len(), 1881);
        assert_eq!(particles[0].attributes[2].to_bits(), 2.5f32.to_bits());
        assert_eq!(particles[7].attributes[2].to_bits(), 2.0f32.to_bits());
        assert_eq!(
            particles[397].attributes[2].to_bits(),
            f32::from_bits(0x4060_0009).to_bits()
        );
    }

    #[test]
    fn saved_redraw_threshold_witnesses_match_compact_native_hashes() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/pencil-v1-boundaries.json"
        ))
        .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let stroke = saved_stroke_from_json(&case["stroke"]);
            let mut rng = BionicRandom::seeded(1);
            let particles = saved_redraw_particles(&stroke, &mut rng);
            let expected = &case["expected"];
            assert_eq!(
                particles.len(),
                expected["particle_count"].as_u64().unwrap() as usize,
                "{} count",
                case["name"]
            );
            let packed: Vec<u8> = particles
                .iter()
                .flat_map(|particle| particle.attributes)
                .flat_map(f32::to_le_bytes)
                .collect();
            let digest = format!("{:x}", Sha256::digest(&packed));
            assert_eq!(
                digest,
                expected["packed_f32_le_sha256"].as_str().unwrap(),
                "{} packed native bytes",
                case["name"]
            );
            for (index, values) in expected["selected_particles"].as_object().unwrap() {
                let particle = &particles[index.parse::<usize>().unwrap()];
                for (attribute, expected_value) in
                    particle.attributes.iter().zip(values.as_array().unwrap())
                {
                    close_bits(
                        *attribute,
                        expected_value,
                        &format!("{} particle {index}", case["name"]),
                    );
                }
            }
        }
    }

    fn saved_stroke_from_json(value: &Value) -> SavedStroke {
        let floats = |name: &str| {
            value[name]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap() as f32)
                .collect::<Vec<_>>()
        };
        let optional_floats = |name: &str| {
            let values = floats(name);
            (!values.is_empty()).then_some(values)
        };
        SavedStroke {
            points: value["points"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| [p[0].as_f64().unwrap() as f32, p[1].as_f64().unwrap() as f32])
                .collect(),
            pressures: floats("pressures"),
            timestamps: value["timestamps"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_i64().unwrap() as i32)
                .collect(),
            tilts: optional_floats("tilts"),
            orientations: optional_floats("orientations"),
            tool_type: value["tool_type"].as_i64().unwrap() as i32,
            size: value["size"].as_f64().unwrap() as f32,
        }
    }

    fn saved_v3_stroke_from_json(value: &Value) -> SavedStroke {
        let points = value["points_f32_bits"].as_array().unwrap();
        let floats = |name: &str| {
            value[name]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| f32::from_bits(little_endian_hex_word(v)))
                .collect::<Vec<_>>()
        };
        SavedStroke {
            points: points
                .iter()
                .map(|pair| {
                    [
                        f32::from_bits(little_endian_hex_word(&pair[0])),
                        f32::from_bits(little_endian_hex_word(&pair[1])),
                    ]
                })
                .collect(),
            pressures: floats("pressure_f32_bits"),
            timestamps: value["timestamps_i32"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_i64().unwrap() as i32)
                .collect(),
            tilts: Some(floats("tilt_f32_bits")),
            orientations: Some(floats("orientation_f32_bits")),
            tool_type: value["tool_type"].as_i64().unwrap() as i32,
            size: f32::from_bits(little_endian_hex_word(&value["pen_size_f32_bits"])),
        }
    }

    #[test]
    fn saved_inputs_keep_optional_axes_absence_and_present_zeroes_distinct() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/pencil-v1-native.json"
        ))
        .unwrap();
        let cases = fixture["saved_cases"].as_array().unwrap();
        let present = saved_stroke_from_json(&cases[0]["stroke"]);
        let absent = saved_stroke_from_json(&cases[7]["stroke"]);
        assert_eq!(present.tilts, Some(vec![0.0]));
        assert_eq!(present.orientations, Some(vec![0.0]));
        assert_eq!(absent.tilts, None);
        assert_eq!(absent.orientations, None);
        assert_eq!(present.validate(), Ok(()));
        assert_eq!(absent.validate(), Ok(()));
        assert_eq!(present.tilt_at(0), absent.tilt_at(0));
        assert_eq!(present.orientation_at(0), absent.orientation_at(0));
        let mut invalid = absent;
        invalid.tilts = Some(vec![0.0]);
        assert_eq!(
            invalid.validate(),
            Err("present optional channel must align with saved points")
        );
    }

    #[test]
    fn v3_particle_size_controls_match_native_addpoint_arguments() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/pencil3-v3-particle-controls-native.json"
        ))
        .unwrap();
        assert_eq!(fixture["schema"], "pencil3-v3-particle-controls-native-v1");
        let cases = fixture["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 11);
        for case in cases {
            let size = case["size"].as_f64().unwrap() as f32;
            let point = std::array::from_fn(|i| case["point"][i].as_f64().unwrap() as f32);
            let alpha = case["alpha"].as_f64().unwrap() as f32;
            let mut rng = V3PrivateRandom::new();
            let particle = rng.particle(point, size, alpha).attributes;
            let actual = [
                particle[0],
                particle[1],
                particle[3],
                particle[2] * 2.0,
                particle[4],
            ];
            for (lane, value) in actual.iter().enumerate() {
                assert_eq!(
                    u64::from(value.to_bits()),
                    case["native_addpoint_u32"][lane].as_u64().unwrap(),
                    "size {size}, native AddPoint lane {lane}"
                );
            }
            assert_eq!(rng.state, prefixed_hex_u64(&case["native_final_state"]));
        }
    }

    #[test]
    fn v3_private_particle_kernel_matches_both_native_redraw_phases_and_resets() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/pencil3-v3-native.json"
        ))
        .unwrap();
        assert_eq!(fixture["schema"], "pencil3-v3-private-sampling-native-v1");
        assert_eq!(fixture["selected_route"]["drawable_vtable_id"], "v3");
        assert_eq!(fixture["native_libc_rng_callback_count"], 0);
        assert_eq!(
            fixture["v3_state_contract"]["constructor_initial_state_u64"],
            "0x0000000000000001"
        );
        assert_eq!(
            fixture["v3_state_contract"]["redraw_reset_constant_u64"],
            "0x0000000000000001"
        );
        assert_eq!(
            prefixed_hex_u64(&fixture["v3_state_contract"]["multiplier_u64"]),
            V3PrivateRandom::MULTIPLIER
        );
        assert_eq!(
            prefixed_hex_u64(&fixture["v3_state_contract"]["increment_u64"]),
            V3PrivateRandom::INCREMENT
        );
        assert_eq!(fixture["v3_state_contract"]["advances_per_particle"], 3);

        let mut phase_streams = Vec::new();
        for phase_name in ["draw_object_stroke", "saved_redraw"] {
            let phase = &fixture["phases"][phase_name];
            let records = phase["records"].as_array().unwrap();
            assert_eq!(records.len(), 48, "{phase_name} record count");
            assert_eq!(phase["particle_count"], 48);

            // Each native Redraw resets the same drawable-owned state even
            // though the preceding phase left the receiver at a nonseed value.
            assert_eq!(
                prefixed_hex_u64(&phase["redraw_entry_state_before_reset_u64"]),
                if phase_name == "draw_object_stroke" {
                    1
                } else {
                    prefixed_hex_u64(
                        &fixture["repeat_same_instance"]["pre_second_redraw_state_u64"],
                    )
                },
                "{phase_name} pre-reset state"
            );
            assert_eq!(
                prefixed_hex_u64(&phase["first_drawpoint_state_after_reset_u64"]),
                1
            );

            let mut rng = V3PrivateRandom::new();
            assert_eq!(rng.state, 1);
            let mut packed = Vec::with_capacity(48 * 5 * 4);
            for (record_index, record) in records.iter().enumerate() {
                let native_before = prefixed_hex_u64(&record["native_private_state_before_u64"]);
                assert_eq!(
                    rng.state, native_before,
                    "{phase_name} record {record_index} pre-state"
                );
                let observed_states = record["native_private_state_after_each_madd_u64"]
                    .as_array()
                    .unwrap();
                assert_eq!(observed_states.len(), 3);
                let mut traced_state = native_before;
                for (advance_index, observed) in observed_states.iter().enumerate() {
                    traced_state = traced_state
                        .wrapping_mul(V3PrivateRandom::MULTIPLIER)
                        .wrapping_add(V3PrivateRandom::INCREMENT);
                    assert_eq!(
                        prefixed_hex_u64(observed),
                        traced_state,
                        "{phase_name} record {record_index} MADD {advance_index}"
                    );
                }

                let draw_input = &record["native_drawpoint_input"];
                let particle = rng.particle(
                    little_endian_hex_pair(&draw_input["point_xy_f32_bits_le"]),
                    f32::from_bits(prefixed_hex_u32(&draw_input["size_s0_u32_hex"])),
                    f32::from_bits(prefixed_hex_u32(&draw_input["alpha_s1_u32_hex"])),
                );
                assert_eq!(
                    rng.state, traced_state,
                    "{phase_name} record {record_index} final state"
                );
                let native_addpoint = record["native_addpoint_s0_s4_u32_hex"].as_array().unwrap();
                assert_eq!(native_addpoint.len(), 5);
                let attributes = particle.attributes;
                assert_eq!(
                    attributes[0].to_bits(),
                    prefixed_hex_u32(&native_addpoint[0]),
                    "{phase_name} record {record_index} x"
                );
                assert_eq!(
                    attributes[1].to_bits(),
                    prefixed_hex_u32(&native_addpoint[1]),
                    "{phase_name} record {record_index} y"
                );
                // AddPoint's ABI passes texture row before full size; the
                // packed RTV record stores half-size before row.
                assert_eq!(
                    attributes[3].to_bits(),
                    prefixed_hex_u32(&native_addpoint[2]),
                    "{phase_name} record {record_index} row"
                );
                assert_eq!(
                    (attributes[2] * 2.0).to_bits(),
                    prefixed_hex_u32(&native_addpoint[3]),
                    "{phase_name} record {record_index} size"
                );
                assert_eq!(
                    attributes[4].to_bits(),
                    prefixed_hex_u32(&native_addpoint[4]),
                    "{phase_name} record {record_index} alpha"
                );
                packed.extend(attributes.into_iter().flat_map(f32::to_le_bytes));
            }

            assert_eq!(
                rng.state,
                prefixed_hex_u64(&phase["modeled_final_private_state_u64"]),
                "{phase_name} final private state"
            );
            let digest = format!("{:x}", Sha256::digest(&packed));
            assert_eq!(
                digest,
                phase["native_stream_sha256"].as_str().unwrap(),
                "{phase_name} packed stream hash"
            );
            if let Some(native_words) = phase["native_packed_stream_f32_bits_le"].as_array() {
                assert_eq!(native_words.len(), 48 * 5, "{phase_name} packed word count");
                let native_bytes = native_words
                    .iter()
                    .flat_map(|word| little_endian_hex_word(word).to_le_bytes())
                    .collect::<Vec<_>>();
                assert_eq!(packed, native_bytes, "{phase_name} all 240 packed words");
            } else {
                // The saved-redraw phase stores only per-callback captures;
                // reorder the native AddPoint ABI lanes into its packed
                // stream format and still compare the full 240-word buffer.
                let native_bytes = records
                    .iter()
                    .flat_map(|record| {
                        let args = record["native_addpoint_s0_s4_u32_hex"].as_array().unwrap();
                        let x = prefixed_hex_u32(&args[0]);
                        let y = prefixed_hex_u32(&args[1]);
                        let row = prefixed_hex_u32(&args[2]);
                        let full_size = f32::from_bits(prefixed_hex_u32(&args[3]));
                        let alpha = prefixed_hex_u32(&args[4]);
                        [x, y, (full_size * 0.5).to_bits(), row, alpha]
                    })
                    .flat_map(u32::to_le_bytes)
                    .collect::<Vec<_>>();
                assert_eq!(native_bytes.len(), 240 * 4);
                assert_eq!(packed, native_bytes, "{phase_name} all 240 packed words");
            }
            phase_streams.push(packed);
        }

        assert_eq!(
            phase_streams[0], phase_streams[1],
            "same native instance must reset V3 RNG for saved redraw"
        );
        assert_eq!(
            fixture["repeat_same_instance"]["same_native_stream_sha256"],
            true
        );
        assert_eq!(
            fixture["phases"]["saved_redraw"]["saved_stream_same_as_drawing"],
            true
        );

        // A single generator reused across calls must reproduce the native
        // lifecycle only after Redraw's explicit reset, not from carried state.
        let first_record = &fixture["phases"]["draw_object_stroke"]["records"][0];
        let input = &first_record["native_drawpoint_input"];
        let args = (
            little_endian_hex_pair(&input["point_xy_f32_bits_le"]),
            f32::from_bits(prefixed_hex_u32(&input["size_s0_u32_hex"])),
            f32::from_bits(prefixed_hex_u32(&input["alpha_s1_u32_hex"])),
        );
        let mut rng = V3PrivateRandom::new();
        let first = rng.particle(args.0, args.1, args.2);
        let carried = rng.particle(args.0, args.1, args.2);
        assert_ne!(
            first.attributes, carried.attributes,
            "private state should advance within one redraw"
        );
        rng.reset();
        let reset = rng.particle(args.0, args.1, args.2);
        assert_eq!(
            reset.attributes, first.attributes,
            "native redraw reset should reproduce the first particle"
        );
    }

    #[test]
    fn v3_saved_sampler_composes_source_channels_with_private_particle_kernel() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/pencil3-v3-native.json"
        ))
        .unwrap();
        let stroke = saved_v3_stroke_from_json(&fixture["input"]);
        assert_eq!(stroke.validate(), Ok(()));
        let mut rng = V3PrivateRandom::new();
        let particles = saved_redraw_v3_particles(&stroke, &mut rng).unwrap();
        assert_eq!(particles.len(), 48);

        let packed = particles
            .iter()
            .flat_map(|particle| particle.attributes)
            .flat_map(f32::to_le_bytes)
            .collect::<Vec<_>>();
        for phase_name in ["draw_object_stroke", "saved_redraw"] {
            let phase = &fixture["phases"][phase_name];
            let native = if let Some(words) = phase["native_packed_stream_f32_bits_le"].as_array() {
                words
                    .iter()
                    .flat_map(|word| little_endian_hex_word(word).to_le_bytes())
                    .collect::<Vec<_>>()
            } else {
                phase["records"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .flat_map(|record| {
                        let args = record["native_addpoint_s0_s4_u32_hex"].as_array().unwrap();
                        let x = prefixed_hex_u32(&args[0]);
                        let y = prefixed_hex_u32(&args[1]);
                        let row = prefixed_hex_u32(&args[2]);
                        let size = f32::from_bits(prefixed_hex_u32(&args[3]));
                        let alpha = prefixed_hex_u32(&args[4]);
                        [x, y, (size * 0.5).to_bits(), row, alpha]
                    })
                    .flat_map(u32::to_le_bytes)
                    .collect::<Vec<_>>()
            };
            assert_eq!(packed, native, "{phase_name} full native stream");
        }

        let replay = saved_redraw_v3_particles(&stroke, &mut rng).unwrap();
        assert_eq!(
            particles, replay,
            "a repeated native saved redraw resets the private generator"
        );

        let mut malformed = stroke.clone();
        malformed.pressures.pop();
        let mut untouched_rng = V3PrivateRandom { state: 0x1234 };
        assert_eq!(
            saved_redraw_v3_particles(&malformed, &mut untouched_rng),
            Err("saved point, pressure and timestamp channels must be aligned")
        );
        assert_eq!(untouched_rng.state, 0x1234);

        for invalid_size in [0.0, -1.0, f32::NAN, f32::INFINITY, f32::from_bits(1)] {
            let mut invalid_stroke = stroke.clone();
            invalid_stroke.size = invalid_size;
            assert_eq!(
                saved_redraw_v3_particles(&invalid_stroke, &mut untouched_rng),
                Err("saved pen size must produce a finite positive spacing"),
                "invalid size bits {:08x}",
                invalid_size.to_bits()
            );
            assert_eq!(untouched_rng.state, 0x1234);
        }
    }

    #[test]
    fn v3_saved_sampler_budget_preserves_native_stream_and_rejects_partial_output() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/pencil3-v3-native.json"
        ))
        .unwrap();
        let stroke = saved_v3_stroke_from_json(&fixture["input"]);
        let mut expected_rng = V3PrivateRandom::new();
        let expected = saved_redraw_v3_particles(&stroke, &mut expected_rng).unwrap();
        assert_eq!(expected.len(), 48); // complete native Drawing control
        for budget in [0, 1, 6, 7, 47] {
            let mut rng = V3PrivateRandom { state: 0x1234 };
            assert!(saved_redraw_v3_particles_bounded(&stroke, &mut rng, budget).is_err());
            assert_eq!(rng.state, 0x1234);
        }
        let mut rng = V3PrivateRandom::new();
        assert_eq!(
            saved_redraw_v3_particles_bounded(&stroke, &mut rng, 48).unwrap(),
            expected
        );
        assert_eq!(rng, expected_rng);

        // A tiny spacing over a long path must terminate at the budget,
        // rather than finish sampling an enormous discarded particle stream.
        let mut dense = stroke.clone();
        dense.size = 0.0001;
        dense.points[3] = [1_000_000., 1_000_000.];
        assert!(saved_redraw_v3_particles_bounded(&dense, &mut rng, 8).is_err());
        assert_eq!(rng, expected_rng);
        dense.points[0][0] = f32::NAN;
        assert_eq!(
            saved_redraw_v3_particles_bounded(&dense, &mut rng, 8),
            Err("saved coordinates must be finite")
        );
        assert_eq!(rng, expected_rng);
    }

    #[test]
    fn v3_saved_sampler_matches_all_native_source_profiles() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/pencil3-sampler-inputs-native.json"
        ))
        .unwrap();

        for (name, case) in fixture["cases"].as_object().unwrap() {
            let profile = &case["input_profile"];
            let stroke = SavedStroke {
                points: profile["points"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|point| {
                        [
                            point[0].as_f64().unwrap() as f32,
                            point[1].as_f64().unwrap() as f32,
                        ]
                    })
                    .collect(),
                pressures: profile["pressure"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| value.as_f64().unwrap() as f32)
                    .collect(),
                timestamps: profile["timestamps"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| value.as_i64().unwrap() as i32)
                    .collect(),
                tilts: Some(
                    profile["tilt"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|value| value.as_f64().unwrap() as f32)
                        .collect(),
                ),
                orientations: Some(
                    profile["orientation"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|value| value.as_f64().unwrap() as f32)
                        .collect(),
                ),
                tool_type: profile["tool_type"].as_i64().unwrap() as i32,
                size: profile["size"].as_f64().unwrap() as f32,
            };
            assert_eq!(stroke.validate(), Ok(()), "{name} source alignment");

            let mut samples = Vec::new();
            sample_saved_redraw(&stroke, |sample| samples.push(sample));
            let route = &case["routes"]["v3"];
            let draw_phase = &route["phases"]["draw_object_stroke"];
            let input_tuples = draw_phase["drawpoint_input_tuples_le_hex_point_then_s0_alpha_u32"]
                .as_array()
                .unwrap();
            assert_eq!(samples.len(), input_tuples.len(), "{name} drawPoint count");
            for (index, (sample, input)) in samples.iter().zip(input_tuples).enumerate() {
                let native_point = little_endian_hex_pair(&input[0]);
                assert_eq!(
                    sample.point.map(f32::to_bits),
                    native_point.map(f32::to_bits),
                    "{name} drawPoint {index} point"
                );
                assert_eq!(
                    sample.size.to_bits(),
                    u32::from_str_radix(input[1].as_str().unwrap(), 16).unwrap(),
                    "{name} drawPoint {index} size"
                );
                assert_eq!(
                    sample.alpha.to_bits(),
                    u32::from_str_radix(input[2].as_str().unwrap(), 16).unwrap(),
                    "{name} drawPoint {index} alpha"
                );
            }

            let mut rng = V3PrivateRandom::new();
            let particles = saved_redraw_v3_particles(&stroke, &mut rng)
                .unwrap_or_else(|error| panic!("{name}: {error}"));
            let packed = particles
                .iter()
                .flat_map(|particle| particle.attributes)
                .flat_map(f32::to_le_bytes)
                .collect::<Vec<_>>();
            let native = draw_phase["native_packed_stream_f32_bits_le"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|word| little_endian_hex_word(word).to_le_bytes())
                .collect::<Vec<_>>();
            assert_eq!(packed.len(), native.len(), "{name} packed bytes");
            assert_eq!(packed, native, "{name} complete native stream");
            let digest = format!("{:x}", Sha256::digest(&packed));
            assert_eq!(digest, draw_phase["native_packed_stream_sha256"]);

            let saved_phase = &route["phases"]["saved_redraw"];
            assert_eq!(
                saved_phase["drawpoint_inputs_identical_to_drawing_phase"],
                true
            );
            assert_eq!(
                saved_phase["native_packed_stream_same_bytes_as_drawing"],
                true
            );
            assert_eq!(saved_phase["native_packed_word_count"], packed.len() / 4);
            assert_eq!(saved_phase["native_packed_stream_sha256"], digest);
            assert_eq!(saved_phase["drawpoint_input_count"], samples.len());
            assert_eq!(
                saved_phase["drawpoint_input_sha256"],
                draw_phase["drawpoint_input_sha256"]
            );
        }
    }
}
