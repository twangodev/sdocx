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
pub(super) struct GlibcRand {
    state: [u32; 31],
    front: usize,
    rear: usize,
}

impl GlibcRand {
    pub(super) fn seeded(seed: u32) -> Self {
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

    pub(super) fn next(&mut self) -> u32 {
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

    // The selected PenData and host RNG are external to the public Stroke.
    // Metadata and unused channels below are synthetic adapter inputs, not
    // evidence of fields in a serialized note or a production admission rule.
    #[derive(Clone, Copy)]
    struct PublicStrokeContext {
        gl_version: u32,
        host_glibc_type3: bool,
        pen: PenData,
    }

    fn sample_public_stroke(
        stroke: &crate::Stroke,
        context: PublicStrokeContext,
    ) -> Result<super::Sampled, &'static str> {
        let rendering = stroke
            .rendering
            .as_ref()
            .ok_or("missing rendering metadata")?;
        let style = &rendering.style;
        let properties = rendering.properties;
        let size = style.pen_size.ok_or("missing pen size")?;
        if context.gl_version != 4
            || !context.host_glibc_type3
            || context.pen.width != 1920
            || context.pen.height != 1080
            || context.pen.spacing_extra != 0
            || context.pen.size.to_bits() != size.to_bits()
            || ![8.0_f32.to_bits(), 16.0_f32.to_bits()].contains(&size.to_bits())
            || stroke.pen_width.to_bits() != size.to_bits()
            || rendering.pen_name.as_deref()
                != Some("com.samsung.android.sdk.pen.pen.preload.ColoredPencil")
            || rendering.advanced_settings.as_deref() != Some("4;")
            || rendering.tool_type_raw != 2
            || !properties.compressed
            || !properties.stylus_channels
            || properties.replay_only
            || properties.eraser
            || properties.fixed_width
            || properties.millisecond_timestamps
            || properties.top_layer_pen
            || properties.alpha_lock
            || properties.binary_added
            || properties.generated
            || properties.fixed_opacity
            || properties.rainbow_effect
            || properties.straighten
            || properties.reveal_mode
            || style.size_level != Some(0)
            || style.stroke_type_raw != Some(0)
            || style.color_type_raw != Some(0)
            || style.color_argb != Some(0x8044_aaee)
            || stroke.color
                != Some(crate::Color {
                    r: 0x44,
                    g: 0xaa,
                    b: 0xee,
                })
        {
            return Err("stroke or context outside captured ColoredPencil GLV4 profile");
        }
        let n = stroke.points.len();
        if stroke.pressures.len() != n
            || stroke.timestamps.len() != n
            || stroke.tilts.len() != n
            || stroke.orientations.len() != n
        {
            return Err("public stroke channels must align");
        }
        fn exact(value: f64) -> Result<f32, &'static str> {
            let narrowed = value as f32;
            if !value.is_finite() || f64::from(narrowed) != value {
                return Err("channel is not exactly representable as finite f32");
            }
            Ok(narrowed)
        }
        let points = stroke
            .points
            .iter()
            .map(|p| {
                Ok(P {
                    x: exact(p.x)?,
                    y: exact(p.y)?,
                })
            })
            .collect::<Result<Vec<_>, &'static str>>()?;
        let tilts = stroke
            .tilts
            .iter()
            .copied()
            .map(exact)
            .collect::<Result<Vec<_>, _>>()?;
        let orientations = stroke
            .orientations
            .iter()
            .copied()
            .map(exact)
            .collect::<Result<Vec<_>, _>>()?;
        for &pressure in &stroke.pressures {
            exact(pressure)?;
        }
        for &timestamp in &stroke.timestamps {
            i32::try_from(timestamp).map_err(|_| "timestamp outside i32 range")?;
        }
        sample(
            &StrokeInput {
                points,
                tilts,
                orientations,
                size,
                curve_enabled: properties.compressed,
                tool_type: i32::from(rendering.tool_type_raw),
            },
            context.pen,
        )
    }

    fn public_stroke(input: &StrokeInput) -> crate::Stroke {
        crate::Stroke {
            rendering: Some(crate::StrokeRendering {
                metadata: None,
                pen_name: Some("com.samsung.android.sdk.pen.pen.preload.ColoredPencil".into()),
                advanced_settings: Some("4;".into()),
                tool_type_raw: 2,
                properties: crate::StrokeProperties {
                    compressed: true,
                    replay_only: false,
                    stylus_channels: true,
                    eraser: false,
                    fixed_width: false,
                    millisecond_timestamps: false,
                    top_layer_pen: false,
                    alpha_lock: false,
                    binary_added: false,
                    generated: false,
                    fixed_opacity: false,
                    rainbow_effect: false,
                    straighten: false,
                    reveal_mode: false,
                },
                style: crate::StrokeStyle {
                    pen_size: Some(input.size),
                    color_argb: Some(0x8044_aaee),
                    size_level: Some(0),
                    stroke_type_raw: Some(0),
                    color_type_raw: Some(0),
                    ..Default::default()
                },
            }),
            bbox: crate::BoundingBox::default(),
            points: input
                .points
                .iter()
                .map(|p| crate::Point {
                    x: f64::from(p.x),
                    y: f64::from(p.y),
                })
                .collect(),
            pressures: vec![0.5; input.points.len()],
            timestamps: vec![0; input.points.len()],
            tilts: input.tilts.iter().copied().map(f64::from).collect(),
            orientations: input.orientations.iter().copied().map(f64::from).collect(),
            color: Some(crate::Color {
                r: 0x44,
                g: 0xaa,
                b: 0xee,
            }),
            pen_width: input.size,
        }
    }

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
            let stroke = public_stroke(&input);
            let context = PublicStrokeContext {
                gl_version: 4,
                host_glibc_type3: true,
                pen,
            };
            let result = sample_public_stroke(&stroke, context).unwrap();
            let mut wrong_context = context;
            wrong_context.host_glibc_type3 = false;
            assert!(sample_public_stroke(&stroke, wrong_context).is_err());
            let mut lossy = stroke.clone();
            lossy.points[0].x = 0.1;
            assert!(sample_public_stroke(&lossy, context).is_err());
            let mut incomplete = stroke.clone();
            incomplete.tilts.pop();
            assert!(sample_public_stroke(&incomplete, context).is_err());

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

    #[cfg(all(feature = "render", feature = "serde"))]
    mod paint_conformance {
        use super::{PenData, StrokeInput, f32_bits, sha256};
        use base64::Engine as _;
        use serde_json::Value;
        use std::fmt::Write as _;

        const PAINT_FIXTURE: &str =
            include_str!("../../../../conformance/colored-pencil-paint-native.json");
        const POINT_R8: &[u8] = include_bytes!("../../../../conformance/colored-pencil-point.r8");
        const SPARSE: &[u8] =
            include_bytes!("../../../../conformance/colored-pencil-paint-sparse.bin");

        fn half_rtz_positive(value: f32) -> f32 {
            if value == 0.0 {
                return 0.0;
            }
            let bits = value.to_bits();
            let exponent = ((bits >> 23) & 0xff) as i32 - 127;
            assert!((-14..=15).contains(&exponent));
            let half = (((exponent + 15) as u32) << 10) | ((bits & 0x7f_ffff) >> 13);
            let exponent = (half >> 10) & 0x1f;
            let mantissa = half & 0x3ff;
            f32::from_bits(((exponent + 112) << 23) | (mantissa << 13))
        }

        fn rgba_atlas(point_r8: &[u8], argb: u32, instance_alpha: f32) -> Vec<u8> {
            assert_eq!(point_r8.len(), 10 * 30);
            let rgb = [
                ((argb >> 16) & 0xff) as u8,
                ((argb >> 8) & 0xff) as u8,
                (argb & 0xff) as u8,
            ];
            let uniform_alpha = ((argb >> 24) & 0xff) as f32 / 255.0;
            let mut bytes = Vec::with_capacity(14 * 34 * 4);
            for py in 0_usize..34 {
                let source_y = py.saturating_sub(2).min(29);
                for px in 0_usize..14 {
                    let source_x = px.saturating_sub(2).min(9);
                    let point = f32::from(point_r8[source_y * 10 + source_x]) / 255.0;
                    // The captured lowp sampler narrows normalized RGBA8 to FP16 with RTZ,
                    // then widens its red lane before the two highp F32 multiplies.
                    let sampled = half_rtz_positive(point);
                    let color_point = uniform_alpha * sampled;
                    let alpha = color_point * instance_alpha;
                    // Mesa's captured source-byte path multiplies by 255 and uses the
                    // host's nearest-even mode. The opaque control covers exact ties.
                    let source_alpha = (alpha * 255.0).round_ties_even().clamp(0.0, 255.0) as u8;
                    bytes.extend_from_slice(&[rgb[0], rgb[1], rgb[2], source_alpha]);
                }
            }
            bytes
        }

        fn png_data_uri(rgba: &[u8]) -> String {
            assert_eq!(rgba.len(), 14 * 34 * 4);
            let mut encoded = Vec::new();
            {
                let mut encoder = png::Encoder::new(&mut encoded, 14, 34);
                encoder.set_color(png::ColorType::Rgba);
                encoder.set_depth(png::BitDepth::Eight);
                let mut writer = encoder.write_header().unwrap();
                writer.write_image_data(rgba).unwrap();
            }
            format!(
                "data:image/png;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(encoded)
            )
        }

        fn source_svg(stream: &[u8], point_r8: &[u8], argb: u32) -> String {
            assert_eq!(stream.len(), 4 * 20, "native four-record AddPoint stream");
            source_svg_control(stream, point_r8, argb, true, 0.0)
        }

        fn source_svg_control(
            stream: &[u8],
            point_r8: &[u8],
            argb: u32,
            clip: bool,
            dx: f32,
        ) -> String {
            assert!(!stream.is_empty() && stream.len().is_multiple_of(20));
            let translation = if dx == 0.0 {
                String::new()
            } else {
                format!(" translate({dx} 0)")
            };
            let mut svg = format!(
                "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"256\" height=\"256\" viewBox=\"0 0 256 256\"><g transform=\"translate(0 256) scale(1 -1){translation}\" color-interpolation=\"sRGB\" style=\"isolation:isolate\">",
            );
            for (index, record) in stream.as_chunks::<20>().0.iter().enumerate() {
                let lane = |offset: usize| {
                    f32::from_le_bytes(record[offset..offset + 4].try_into().unwrap())
                };
                let center_x = f64::from(lane(0));
                let center_y = f64::from(lane(4));
                let half_size = f64::from(lane(8));
                let atlas_row = lane(12) as usize;
                let instance_alpha = lane(16);
                assert!(atlas_row < 3);
                let left = center_x - half_size;
                let bottom = center_y - half_size;
                let width = half_size * 2.0;
                let texel = width / 10.0;
                let image_x = left - 2.0 * texel;
                let image_width = 14.0 * texel;
                let image_height = 34.0 * texel;
                let image_high = bottom + width + (2.0 + 10.0 * atlas_row as f64) * texel;
                let image_y = image_high - image_height;
                let reflection = image_y + image_high;
                let rect = |value: f64| value.to_string();
                let uri = png_data_uri(&rgba_atlas(point_r8, argb, instance_alpha));
                if clip {
                    write!(svg, "<defs><clipPath id=\"cp-clip-{index}\" clipPathUnits=\"userSpaceOnUse\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" shape-rendering=\"crispEdges\"/></clipPath></defs><g clip-path=\"url(#cp-clip-{index})\">", rect(left), rect(bottom), rect(width), rect(width)).unwrap();
                }
                write!(svg, "<image x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\" image-rendering=\"optimizeSpeed\" style=\"image-rendering:pixelated\" href=\"{}\" transform=\"translate(0 {}) scale(1 -1)\"/>", rect(image_x), rect(image_y), rect(image_width), rect(image_height), uri, rect(reflection)).unwrap();
                if clip {
                    svg.push_str("</g>");
                }
            }
            svg.push_str("</g></svg>");
            svg
        }

        fn sampled_native_stream(fixture: &Value) -> Vec<u8> {
            let geometry_fixture: Value = serde_json::from_str(super::FIXTURE).unwrap();
            let case = &geometry_fixture["cases"][0];
            let source = &case["input"];
            let input = StrokeInput {
                points: source["points_f32_bits"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|pair| super::super::P {
                        x: f32_bits(&pair[0]),
                        y: f32_bits(&pair[1]),
                    })
                    .collect(),
                tilts: source["tilt_f32_bits"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(f32_bits)
                    .collect(),
                orientations: source["orientation_f32_bits"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(f32_bits)
                    .collect(),
                size: f32_bits(&source["size_f32_bits"]),
                curve_enabled: source["curve_enabled"].as_bool().unwrap(),
                tool_type: source["tool_type"].as_i64().unwrap() as i32,
            };
            let pen = PenData {
                size: f32_bits(&case["pen_data"]["size_f32_bits"]),
                width: case["pen_data"]["width"].as_i64().unwrap() as i32,
                height: case["pen_data"]["height"].as_i64().unwrap() as i32,
                spacing_extra: case["pen_data"]["spacing_extra"].as_i64().unwrap() as i32,
            };
            let sampled = super::sample_public_stroke(
                &super::public_stroke(&input),
                super::PublicStrokeContext {
                    gl_version: 4,
                    host_glibc_type3: true,
                    pen,
                },
            )
            .unwrap();
            let stream_meta = &case["native_stream"];
            let geometry_offset = stream_meta["offset"].as_u64().unwrap() as usize;
            let geometry_len = stream_meta["length"].as_u64().unwrap() as usize;
            assert_eq!(
                sampled.stream.as_slice(),
                &super::NATIVE[geometry_offset..geometry_offset + geometry_len]
            );
            assert_eq!(
                sha256(&sampled.stream),
                fixture["provenance"]["geometry_stream_sha256"]
                    .as_str()
                    .unwrap()
            );
            assert_eq!(sampled.stream.len(), 80);

            sampled.stream
        }

        // Experimental reader fixtures only; this does not admit production rendering.
        #[cfg(feature = "pdf")]
        #[test]
        #[ignore = "writes experimental SVG/PDF artifacts to SDOCX_COLORED_PENCIL_EXPORT_DIR"]
        fn export_colored_pencil_native_controls() {
            let output = std::path::PathBuf::from(
                std::env::var_os("SDOCX_COLORED_PENCIL_EXPORT_DIR")
                    .expect("set SDOCX_COLORED_PENCIL_EXPORT_DIR to a fresh directory"),
            );
            std::fs::create_dir(&output).expect("output directory must not already exist");
            let fixture: Value = serde_json::from_str(PAINT_FIXTURE).unwrap();
            let stream = sampled_native_stream(&fixture);
            for color_case in fixture["cases"].as_array().unwrap() {
                let color = color_case["argb"]
                    .as_str()
                    .unwrap()
                    .trim_start_matches("0x");
                let argb = u32::from_str_radix(color, 16).unwrap();
                let svg = source_svg(&stream, POINT_R8, argb);
                let page = crate::RenderedPage::new(0, 256, 256, svg.clone());
                let pdf = crate::render_svg_pages_pdf(&[page], &crate::PdfOptions::default())
                    .expect("experimental SVG must lower to PDF");
                std::fs::write(output.join(format!("{color}.svg")), svg).unwrap();
                std::fs::write(output.join(format!("{color}.pdf")), pdf).unwrap();
                for (phase, dx) in [("original", 0.0), ("fractional-dx0.375", 0.375)] {
                    for clip in [true, false] {
                        let svg = source_svg_control(&stream[..20], POINT_R8, argb, clip, dx);
                        let page = crate::RenderedPage::new(0, 256, 256, svg.clone());
                        let pdf =
                            crate::render_svg_pages_pdf(&[page], &crate::PdfOptions::default())
                                .unwrap();
                        let name = format!(
                            "{color}-first-{phase}-{}",
                            if clip { "clip" } else { "unclipped" }
                        );
                        std::fs::write(output.join(format!("{name}.svg")), svg).unwrap();
                        std::fs::write(output.join(format!("{name}.pdf")), pdf).unwrap();
                    }
                }
            }
            std::fs::write(output.join("provenance.json"), PAINT_FIXTURE).unwrap();
        }

        #[test]
        fn colored_pencil_public_argb_controls_match_native_rgba8_frames() {
            let fixture: Value = serde_json::from_str(PAINT_FIXTURE).unwrap();
            assert_eq!(
                fixture["schema"],
                "sdocx-colored-pencil-v4-paint-native-sparse-v1"
            );
            assert_eq!(
                POINT_R8.len(),
                fixture["binary"]["point_atlas"]["length"].as_u64().unwrap() as usize
            );
            assert_eq!(
                sha256(POINT_R8),
                fixture["binary"]["point_atlas"]["sha256"].as_str().unwrap()
            );
            let expanded_rgba: Vec<u8> = POINT_R8
                .iter()
                .flat_map(|value| [*value, 0, 0, 0])
                .collect();
            assert_eq!(
                sha256(&expanded_rgba),
                fixture["binary"]["point_atlas"]["native_RGBA8_expanded_sha256"]
                    .as_str()
                    .unwrap()
            );
            assert_eq!(
                sha256(SPARSE),
                fixture["binary"]["sha256"].as_str().unwrap()
            );

            let stream = sampled_native_stream(&fixture);

            for color_case in fixture["cases"].as_array().unwrap() {
                assert_eq!(
                    color_case["geometry_stream_sha256"].as_str().unwrap(),
                    sha256(&stream)
                );
                let argb = u32::from_str_radix(
                    color_case["argb"]
                        .as_str()
                        .unwrap()
                        .trim_start_matches("0x"),
                    16,
                )
                .unwrap();
                let svg = source_svg(&stream, POINT_R8, argb);
                let tree =
                    resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default()).unwrap();
                let mut pixmap = resvg::tiny_skia::Pixmap::new(256, 256).unwrap();
                resvg::render(
                    &tree,
                    resvg::tiny_skia::Transform::identity(),
                    &mut pixmap.as_mut(),
                );
                let sparse_meta = &color_case["sparse_pixels"];
                let offset = sparse_meta["offset"].as_u64().unwrap() as usize;
                let length = sparse_meta["length"].as_u64().unwrap() as usize;
                let sparse = &SPARSE[offset..offset + length];
                assert_eq!(
                    sparse.len(),
                    sparse_meta["count"].as_u64().unwrap() as usize * 8
                );
                assert_eq!(sha256(sparse), sparse_meta["sha256"].as_str().unwrap());
                let mut expected = vec![0u8; 256 * 256 * 4];
                for pixel in sparse.as_chunks::<8>().0 {
                    let x = u16::from_le_bytes(pixel[0..2].try_into().unwrap()) as usize;
                    let y = u16::from_le_bytes(pixel[2..4].try_into().unwrap()) as usize;
                    assert!(x < 256 && y < 256);
                    let dest = (y * 256 + x) * 4;
                    assert_eq!(
                        &expected[dest..dest + 4],
                        &[0; 4],
                        "duplicate native pixel coordinate"
                    );
                    expected[dest..dest + 4].copy_from_slice(&pixel[4..8]);
                }
                assert_eq!(
                    sha256(pixmap.data()),
                    color_case["native_rgba8_top_down_sha256"].as_str().unwrap(),
                    "ARGB {argb:08x}"
                );
                assert_eq!(
                    pixmap.data(),
                    expected,
                    "full native 256x256 RGBA8 frame, ARGB {argb:08x}"
                );
            }
        }
    }
}
