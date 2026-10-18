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
    let mut distance = spacing;
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
    particles
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

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
