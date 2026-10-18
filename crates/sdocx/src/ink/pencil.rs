//! Test-only numeric reconstruction of Pencil3 V1's particle core.
//!
//! This module is intentionally not registered for production use. The public
//! functions require explicit native RNG values or an explicit bionic RNG
//! context so they cannot silently depend on process-global libc state.

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct RectF {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Particle {
    /// Packed RTV1 order: center x, center y, half-size, random row, alpha.
    pub attributes: [f32; 5],
}

#[derive(Clone, Copy, Debug)]
pub(super) struct DrawPoint {
    pub point: [f32; 2],
    pub size: f32,
    pub alpha: f32,
    pub initial_bounds: RectF,
    /// Three C rand() results, in native call order.
    pub rand_values: [u32; 3],
}

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
    fn validate(&self) -> Result<(), &'static str> {
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

    fn orientation_at(&self, index: usize) -> f32 {
        self.orientations.as_ref().map_or(0.0, |v| v[index])
    }
}

/// Execute the observed drawPoint and RTV1::AddPoint arithmetic with caller
/// supplied RNG outputs. Native drawPoint at Pencil3+0x108338 uses
/// `size1 = size + 1`, `range = 30 + size1 * min(size1, 5)`, then emits
/// `(point + rand % trunc(range) / 20 - trunc(range) / 40)` on each axis.
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
#[derive(Clone, Debug)]
pub(super) struct BionicRandom {
    state: [u32; 31],
    front: usize,
    rear: usize,
}

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
    use super::path::{Quad, P};
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
    let mut particles = Vec::new();
    let mut distance = initial_distance;
    while distance <= path.length {
        let point = path.at(distance);
        particles.push(
            draw_point(DrawPoint {
                point: [point.x, point.y],
                size: point_size,
                alpha,
                initial_bounds: RectF {
                    left: 0.0,
                    top: 0.0,
                    right: 0.0,
                    bottom: 0.0,
                },
                rand_values: [rng.next(), rng.next(), rng.next()],
            })
            .particle,
        );
        distance += spacing;
    }
    (particles, distance - path.length)
}

/// Reconstruct the native V1 saved-event lifecycle for the bounded finite
/// profile used by the conformance fixture.  Values are derived from the
/// saved stroke; fixture output is never consulted by this implementation.
fn saved_redraw_particles(stroke: &SavedStroke, rng: &mut BionicRandom) -> Vec<Particle> {
    if stroke.points.len() < 2 {
        return Vec::new();
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
    let mut particles = seven_start_dabs(stroke.points[0], stroke.size, initial_alpha, rng);

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
        let (line, phase) = line_particles_with_phase(
            midpoint,
            raw,
            [(raw[0] + current[0]) * 0.5, (raw[1] + current[1]) * 0.5],
            point_size,
            alpha,
            step,
            distance_phase,
            rng,
        );
        particles.extend(line);
        distance_phase = phase;
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
        let tilt_scale = if average > 0.0 && average >= 0.55 {
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
    let terminal_alpha = terminal_pressure / 1.5;
    let terminal = *stroke.points.last().unwrap();
    let (line, _) = line_particles_with_phase(
        midpoint,
        raw,
        terminal,
        point_size,
        terminal_alpha,
        step,
        distance_phase,
        rng,
    );
    particles.extend(line);
    particles
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
            3.500002145767212f32.to_bits()
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
}
