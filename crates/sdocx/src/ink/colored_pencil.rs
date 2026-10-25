//! Test-only Colored Pencil GLV4 saved-stroke sampler.
//!
//! This reconstructs the native saved-stroke route only for the explicit
//! profiles in `conformance/colored-pencil-native.json`. The path measurement
//! uses the production SmPath `Quad`; the random sequence is the observed
//! glibc TYPE_3 host profile and does not claim Android bionic parity. Native
//! pre-jitter points and path outputs are test assertions, never sampler input.

use super::path::{P, Quad};

const MAX_INPUT_POINTS: usize = 4_096;
const MAX_OUTPUT_SAMPLES: usize = 65_536;

#[derive(Clone, Debug)]
struct StrokeInput {
    points: Vec<P>,
    tilts: Vec<f32>,
    orientations: Vec<f32>,
    size: f32,
    curve_enabled: bool,
    tool_type: i32,
}

#[derive(Clone, Copy, Debug)]
struct PenData {
    size: f32,
    width: i32,
    height: i32,
    spacing_extra: i32,
}

#[derive(Clone, Copy, Debug)]
struct PathSample {
    distance: f32,
    point: P,
    tangent: P,
}

#[derive(Clone, Copy, Debug)]
struct DabInput {
    point: P,
    size: f32,
    angle: f32,
}

#[derive(Debug)]
struct Dab {
    seed: u32,
    input: DabInput,
    accepted: bool,
    row: Option<f32>,
    center: Option<P>,
}

#[derive(Debug)]
struct Sampled {
    paths: Vec<PathSample>,
    dabs: Vec<Dab>,
    stream: Vec<u8>,
}

/// glibc 2.41 TYPE_3 `rand` sequence, including `srand(0)` normalization.
/// This is a source-level implementation of the captured host profile.
struct GlibcRand {
    state: [u32; 31],
    front: usize,
    rear: usize,
}

impl GlibcRand {
    fn seeded(seed: u32) -> Self {
        let seed = if seed == 0 { 1 } else { seed };
        let mut state = [0; 31];
        state[0] = seed;
        for i in 1..31 {
            // glibc stores each word as int32_t before the Park–Miller step.
            let previous = state[i - 1] as i32 as i64;
            let next = 16_807 * (previous % 127_773) - 2_836 * (previous / 127_773);
            state[i] = if next < 0 {
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

    fn next(&mut self) -> u32 {
        let sum = self.state[self.front].wrapping_add(self.state[self.rear]);
        self.state[self.front] = sum;
        let result = (sum >> 1) & 0x7fff_ffff;
        self.front = (self.front + 1) % 31;
        self.rear = (self.rear + 1) % 31;
        result
    }
}

fn spacing(size: f32, pen: PenData) -> f32 {
    let minimum = pen.width.min(pen.height);
    let density = if minimum == 0 {
        1.0
    } else {
        minimum as f32 / 360.0
    };
    let time = 30.0 / ((pen.spacing_extra as f32 + 45.0) * 1.3);
    let density_factor = 60.0 / density;
    let repeat = time * density_factor;
    (size * 0.08) * repeat
}

// drawLine explicitly selects zero when tilt <= its 0.65 threshold (AArch64
// fcsel at RVA 0x2ae38); it loads selected pen size into S4 afterward. The
// fixture checks both the exact 0.65 threshold and the higher 0.79 branch.
fn line_size(size: f32, tilt: f32) -> f32 {
    let adjustment = if tilt > 0.65 { tilt - 0.65 } else { 0.0 };
    let base = adjustment.mul_add(85.5, size);
    (size * 17.0).mul_add(adjustment, base)
}

fn rotate(candidate: P, center: P, angle: f32) -> P {
    let angle = angle as f64;
    // The exact native-stream tests check this portable pinned libm profile.
    // Do not substitute fitted ULP adjustments if another angle differs.
    let sin = libm::sin(angle);
    let cos = libm::cos(angle);
    let dx = (candidate.x - center.x) as f64;
    let dy = (candidate.y - center.y) as f64;
    let x_delta = dx.mul_add(cos, -(sin * dy)) as f32;
    let y_delta = dx.mul_add(sin, cos * dy) as f32;
    P {
        x: center.x + x_delta,
        y: center.y + y_delta,
    }
}

fn draw_point(input: DabInput, pen_size: f32, seed: u32, rng: &mut GlibcRand) -> Dab {
    let dab_size = input.size.max(2.0);
    let r0 = rng.next();
    let r1 = rng.next();
    let r2 = rng.next();
    let sign_bits = r0 & 0x8000_0001;
    let mut x_magnitude = r1 & 1;
    if (r1 as i32) < 0 {
        x_magnitude = x_magnitude.wrapping_neg();
    }
    let x_delta = if sign_bits == 1 {
        x_magnitude as f32
    } else {
        -(x_magnitude as f32)
    };
    let half = dab_size * 0.5;
    let y_divisor = (dab_size - half + 1.0) as u32;
    assert_ne!(y_divisor, 0);
    let candidate = P {
        x: input.point.x + x_delta,
        y: input.point.y + (r2 % y_divisor) as f32,
    };
    let dx = input.point.x - candidate.x;
    let dy = input.point.y - candidate.y;
    let radius = dx.mul_add(dx, dy * dy).sqrt();
    if radius > half {
        return Dab {
            seed,
            input,
            accepted: false,
            row: None,
            center: None,
        };
    }
    let row = (rng.next() % 3) as f32;
    let center = rotate(candidate, input.point, input.angle);
    let _record_half_size = pen_size.max(1.5) * 2.35 * 0.7 * 0.5;
    Dab {
        seed,
        input,
        accepted: true,
        row: Some(row),
        center: Some(center),
    }
}

fn next_seed(seed: u32) -> u32 {
    if (seed as i32) >= 10_000_000 {
        0
    } else {
        seed.wrapping_add(1)
    }
}

fn sample(input: &StrokeInput, pen: PenData) -> Result<Sampled, &'static str> {
    let n = input.points.len();
    if !(2..=MAX_INPUT_POINTS).contains(&n)
        || input.tilts.len() != n
        || input.orientations.len() != n
    {
        return Err("point, tilt, and orientation channels must align within the input limit");
    }
    if !input.curve_enabled || input.tool_type != 2 {
        return Err("sampler is scoped to curve-enabled tool-type-2 strokes");
    }
    if !input.size.is_finite() || !pen.size.is_finite() || pen.size <= 0.0 {
        return Err(
            "style and selected PenData sizes must be finite; selected size must be positive",
        );
    }
    if input
        .points
        .iter()
        .any(|point| !point.x.is_finite() || !point.y.is_finite())
        || input.tilts.iter().any(|value| !value.is_finite())
        || input.orientations.iter().any(|value| !value.is_finite())
    {
        return Err("point, tilt, and orientation channels must be finite");
    }
    if !(1..=65_535).contains(&pen.width)
        || !(1..=65_535).contains(&pen.height)
        || !(-44..=1_000_000).contains(&pen.spacing_extra)
    {
        return Err("PenData profile is outside the bounded captured range");
    }
    let mut seed = input.points[0].x.trunc() as i32 as u32;
    let mut paths = Vec::new();
    let mut dabs = Vec::new();
    let mut records = Vec::<[f32; 5]>::new();
    let mut emit = |point: P, dab_size: f32, angle: f32| -> Result<(), &'static str> {
        if dabs.len() >= MAX_OUTPUT_SAMPLES {
            return Err("Colored Pencil output sample limit exceeded");
        }
        let current_seed = seed;
        let mut rng = GlibcRand::seeded(current_seed);
        let dab = draw_point(
            DabInput {
                point,
                size: dab_size,
                angle,
            },
            pen.size,
            current_seed,
            &mut rng,
        );
        if dab.accepted {
            let center = dab.center.unwrap();
            let row = dab.row.unwrap();
            let half_size = pen.size.max(1.5) * 2.35 * 0.7 * 0.5;
            records.push([center.x, center.y, half_size, row, 0.3]);
        }
        dabs.push(dab);
        seed = next_seed(current_seed);
        Ok(())
    };

    emit(
        input.points[0],
        pen.size,
        *input.orientations.last().unwrap(),
    )?;
    let mut last_actual = input.points[0];
    let mut path_start = input.points[0];
    let width_scale = pen.size * 0.08;
    let mut carry = width_scale;
    let step = spacing(pen.size, pen);
    let mut terminal_size = pen.size;
    if !step.is_finite() || step <= 0.0 {
        return Err("Colored Pencil spacing step must be finite and positive");
    }
    for i in 1..(n - 1) {
        let current = input.points[i];
        let midpoint = last_actual.mid(current);
        let curve = Quad::new(path_start, last_actual, midpoint);
        if !curve.length.is_finite() {
            return Err("Colored Pencil curve length must be finite");
        }
        let dab_size = line_size(pen.size, input.tilts[i]);
        // endPen reads the drawable's latched drawPoint size; the V4 caller
        // leaves that value from its last drawLine size calculation.
        terminal_size = dab_size;
        let mut distance = carry;
        while distance <= curve.length {
            if paths.len() >= MAX_OUTPUT_SAMPLES {
                return Err("Colored Pencil path sample limit exceeded");
            }
            let point = curve.at(distance);
            let tangent = curve.normalized_tangent(distance);
            paths.push(PathSample {
                distance,
                point,
                tangent,
            });
            emit(point, dab_size, input.orientations[i])?;
            let next_distance = distance + step;
            if !next_distance.is_finite() || next_distance <= distance {
                return Err("Colored Pencil spacing step does not make finite progress");
            }
            distance = next_distance;
        }
        carry = distance - curve.length;
        last_actual = current;
        path_start = midpoint;
    }
    emit(
        input.points[n - 2],
        terminal_size,
        *input.orientations.last().unwrap(),
    )?;

    let mut stream = Vec::with_capacity(records.len() * 20);
    for record in records {
        for lane in record {
            stream.extend_from_slice(&lane.to_le_bytes());
        }
    }
    Ok(Sampled {
        paths,
        dabs,
        stream,
    })
}

#[cfg(test)]
mod tests {
    use super::{GlibcRand, P, PenData, StrokeInput, next_seed, sample};
    use serde_json::Value;
    use sha2::{Digest, Sha256};

    const FIXTURE: &str = include_str!("../../../../conformance/colored-pencil-native.json");
    const NATIVE: &[u8] = include_bytes!("../../../../conformance/colored-pencil-native.bin");

    fn hex_bytes(value: &str) -> Vec<u8> {
        assert_eq!(value.len() % 2, 0);
        (0..value.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&value[i..i + 2], 16).unwrap())
            .collect()
    }

    fn f32_bits(value: &Value) -> f32 {
        f32::from_le_bytes(hex_bytes(value.as_str().unwrap()).try_into().unwrap())
    }

    fn u32_bits_le(value: &Value) -> u32 {
        u32::from_le_bytes(hex_bytes(value.as_str().unwrap()).try_into().unwrap())
    }

    fn f32_pair_bits_le(value: &Value) -> [u32; 2] {
        let bytes = hex_bytes(value.as_str().unwrap());
        [
            u32::from_le_bytes(bytes[0..4].try_into().unwrap()),
            u32::from_le_bytes(bytes[4..8].try_into().unwrap()),
        ]
    }

    fn pair_bits(point: P) -> [u32; 2] {
        [point.x.to_bits(), point.y.to_bits()]
    }

    fn sha256(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    #[test]
    fn saved_glv4_geometry_matches_native_stream_and_path_samples() {
        let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
        assert_eq!(fixture["schema"], "sdocx-colored-pencil-native-v1");
        assert_eq!(
            fixture["provenance"]["random_profile"],
            "glibc-2.41-TYPE_3-host"
        );
        assert_eq!(fixture["provenance"]["android_rng_parity"], false);
        let cases = fixture["cases"].as_array().unwrap();
        let case_ids: Vec<&str> = cases
            .iter()
            .map(|case| case["id"].as_str().unwrap())
            .collect();
        assert_eq!(
            case_ids,
            vec![
                "saved-glv4-size-8",
                "saved-glv4-size-16",
                "negative_initial_x_minus2",
                "all_tilt_079",
                "all_tilt_065",
            ]
        );
        assert_eq!(
            NATIVE.len(),
            fixture["native_stream"]["length"].as_u64().unwrap() as usize
        );
        assert_eq!(
            sha256(NATIVE),
            fixture["native_stream"]["sha256"].as_str().unwrap()
        );

        for case in cases {
            assert_eq!(
                case["selected_drawable_class"],
                "ColoredPencilStrokeDrawableGLV4"
            );
            let input_json = &case["input"];
            let points: Vec<P> = input_json["points_f32_bits"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| P {
                    x: f32_bits(&row[0]),
                    y: f32_bits(&row[1]),
                })
                .collect();
            let input = StrokeInput {
                points,
                tilts: input_json["tilt_f32_bits"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(f32_bits)
                    .collect(),
                orientations: input_json["orientation_f32_bits"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(f32_bits)
                    .collect(),
                size: f32_bits(&input_json["size_f32_bits"]),
                curve_enabled: input_json["curve_enabled"].as_bool().unwrap(),
                tool_type: input_json["tool_type"].as_i64().unwrap() as i32,
            };
            let pen = PenData {
                size: f32_bits(&case["pen_data"]["size_f32_bits"]),
                width: case["pen_data"]["width"].as_i64().unwrap() as i32,
                height: case["pen_data"]["height"].as_i64().unwrap() as i32,
                spacing_extra: case["pen_data"]["spacing_extra"].as_i64().unwrap() as i32,
            };
            let result = sample(&input, pen).unwrap();

            let expected_paths = case["native_path_samples"].as_array().unwrap();
            assert_eq!(
                result.paths.len(),
                expected_paths.len(),
                "{} paths",
                case["id"]
            );
            for (actual, expected) in result.paths.iter().zip(expected_paths) {
                assert_eq!(
                    actual.distance.to_bits(),
                    u32_bits_le(&expected["distance_f32_bits"]),
                    "{} distance",
                    case["id"]
                );
                assert_eq!(
                    pair_bits(actual.point),
                    f32_pair_bits_le(&expected["point_f32_bits"]),
                    "{} point",
                    case["id"]
                );
                assert_eq!(
                    pair_bits(actual.tangent),
                    f32_pair_bits_le(&expected["tangent_f32_bits"]),
                    "{} tangent",
                    case["id"]
                );
            }

            let expected_dabs = case["native_pre_jitter_points_f32_bits"]
                .as_array()
                .unwrap();
            assert_eq!(
                result.dabs.len(),
                expected_dabs.len(),
                "{} dabs",
                case["id"]
            );
            for (actual, expected) in result.dabs.iter().zip(expected_dabs) {
                assert_eq!(
                    pair_bits(actual.input.point),
                    f32_pair_bits_le(expected),
                    "{} pre-jitter source point",
                    case["id"]
                );
            }
            let expected_sizes = case["native_drawpoint_size_f32_bits"].as_array().unwrap();
            assert_eq!(
                result.dabs.len(),
                expected_sizes.len(),
                "{} drawPoint size calls",
                case["id"]
            );
            for (actual, expected) in result.dabs.iter().zip(expected_sizes) {
                assert_eq!(
                    actual.input.size.to_bits(),
                    u32_bits_le(expected),
                    "{} native drawPoint S0 size",
                    case["id"]
                );
            }
            let expected_seeds = case["native_saved_rand_seeds_u32"].as_array().unwrap();
            assert_eq!(
                result.dabs.len(),
                expected_seeds.len(),
                "{} seed calls",
                case["id"]
            );
            for (actual, expected) in result.dabs.iter().zip(expected_seeds) {
                assert_eq!(
                    actual.seed,
                    expected.as_u64().unwrap() as u32,
                    "{} SetSrand input",
                    case["id"]
                );
            }

            let offset = case["native_stream"]["offset"].as_u64().unwrap() as usize;
            let length = case["native_stream"]["length"].as_u64().unwrap() as usize;
            let expected = &NATIVE[offset..offset + length];
            assert_eq!(
                result.stream, expected,
                "{} native five-lane stream",
                case["id"]
            );
            assert_eq!(
                sha256(&result.stream),
                case["native_stream"]["sha256"].as_str().unwrap()
            );
        }
    }

    #[test]
    fn glibc_type3_matches_pinned_host_cpu_controls() {
        let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
        let controls = fixture["glibc_cpu_controls"].as_object().unwrap();
        assert_eq!(controls["glibc_version"], "2.41");
        assert_eq!(
            controls["libc_sha256"],
            "9792e3cbb541c8f44c7acf5f14f4022ea62998ecc787d326bed4d8b6547dfd92"
        );
        assert_eq!(controls["android_parity"], false);

        for case in controls["cases"].as_array().unwrap() {
            let seed = case["seed_u32"].as_u64().unwrap() as u32;
            let expected = case["first_four_rand_u31"].as_array().unwrap();
            let mut rng = GlibcRand::seeded(seed);
            for (index, expected) in expected.iter().enumerate() {
                assert_eq!(
                    rng.next(),
                    expected.as_u64().unwrap() as u32,
                    "seed {seed}, output {index}"
                );
            }
        }
    }

    #[test]
    fn seed_counter_matches_signed_native_compare_and_wrap() {
        for (before, after) in [
            (0xffff_fffe, 0xffff_ffff),
            (0xffff_ffff, 0),
            (0x8000_0000, 0x8000_0001),
            (9_999_999, 10_000_000),
            (10_000_000, 0),
            (10_000_001, 0),
        ] {
            assert_eq!(next_seed(before), after, "counter {before:#010x}");
        }
    }
}
