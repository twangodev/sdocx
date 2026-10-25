//! Test-only saved WaterColorBrush RT dab sampler.
//!
//! This follows the confirmed single-ObjectStroke Drawing route through native
//! SmPath sampling and RTV1 AddPoint records. It does not produce the GL tip
//! bitmap or production paint. A separate test reconstructs one captured paint
//! profile through generated SVG. Both stay within their fixture's context.

use super::path::{P, Quad};
use crate::Point;

#[cfg(feature = "render")]
mod paint;

const MAX_INPUT_POINTS: usize = 4096;
const MAX_OUTPUT_DABS: usize = 65_536;
const MIN_MOVEMENT: f32 = 1.0; // WaterColorBrushStrokeDrawableGL ctor, this+88.
const SAMPLE_STEP_FACTOR: f32 = f32::from_bits(0x3daa_9931); // Base rodata, 0.0833.
const PHASE_STEP: f32 = f32::from_bits(0x4019_07cd); // WaterColorBrush rodata.
const TAU: f32 = f32::from_bits(0x40c9_0fdb);
const NEG_TAU: f32 = f32::from_bits(0xc0c9_0fdb);

// This models the capture runner's x86_64 glibc 2.41 FMA sincosf boundary,
// independently recovered from libm.so.6 (full hash in the fixture; target RVA 0x838c0).
// It is not evidence for Samsung/Android libm. Explicit fused operations keep
// this host-oracle profile deterministic on every Rust test platform.
mod host_math {
    const TAU: f32 = f32::from_bits(0x40c9_0fdb);
    const PIO4: f32 = f32::from_bits(0x3f49_0fdb);
    const TINY: f32 = f32::from_bits(0x3980_0000); // 2^-12

    const HPI_INV: f64 = f64::from_bits(0x4164_5f30_6dc9_c883);
    const HPI: f64 = f64::from_bits(0x3ff9_21fb_5444_2d18);

    // Coefficients from the glibc sincosf FMA polynomial, represented by exact
    // binary64 words to avoid decimal parser differences.
    const C1: f64 = f64::from_bits(0xbfdf_ffff_fd0c_621c);
    const S1: f64 = f64::from_bits(0xbfc5_5554_5995_a603);
    const C2: f64 = f64::from_bits(0x3fa5_5553_e106_8f19);
    const S2: f64 = f64::from_bits(0x3f81_1076_0523_0bc4);
    const C3: f64 = f64::from_bits(0xbf56_c087_e89a_359d);
    const S3: f64 = f64::from_bits(0xbf29_94eb_3774_cf24);
    const C4: f64 = f64::from_bits(0x3ef9_9343_027b_f8c3);

    /// Return the host-profile sin/cos result for the finite closed interval [0, TAU].
    ///
    /// Values outside this interval are deliberately unsupported: the capture and
    /// captured WaterColor phase recurrence stay in this domain.
    pub fn sincosf_bounded(input: f32) -> Option<(f32, f32)> {
        if !input.is_finite() || input < 0.0 || input > TAU {
            return None;
        }

        if input < PIO4 {
            if input < TINY {
                return Some((input, 1.0));
            }
            let (sin, cos) = polynomial(input as f64, false);
            return Some((sin as f32, cos as f32));
        }

        // The host routine computes n = round(x * (2/pi) * 2^24) by adding
        // 2^23 and taking the high 8-bit quotient. All bounded inputs here are
        // nonnegative, so signed-conversion corner cases are outside the contract.
        let scaled = (input as f64) * HPI_INV;
        let n = ((scaled as i32).wrapping_add(0x0080_0000)) >> 24;
        let mut reduced = (-f64::from(n)).mul_add(HPI, f64::from(input));
        let quadrant = (n & 3) as usize;
        let sign = [1.0, -1.0, -1.0, 1.0][quadrant];
        reduced *= sign;
        let (mut sin, mut cos) = polynomial(reduced, n & 2 != 0);
        if n & 1 != 0 {
            core::mem::swap(&mut sin, &mut cos);
        }
        Some((sin as f32, cos as f32))
    }

    #[inline]
    fn polynomial(x: f64, negated_cosine: bool) -> (f64, f64) {
        let x2 = x * x;
        let x3 = x * x2;
        let x4 = x2 * x2;
        let x5 = x3 * x2;
        let x6 = x4 * x2;

        let (c0, c1, c2, c3, c4) = if negated_cosine {
            (-1.0, -C1, -C2, -C3, -C4)
        } else {
            (1.0, C1, C2, C3, C4)
        };
        let sin_tail = x2.mul_add(S3, S2);
        let cos_tail = x2.mul_add(c4, c3);
        let cos_head = x2.mul_add(c1, c0);
        let mut sin = x3.mul_add(S1, x);
        let mut cos = x4.mul_add(c2, cos_head);
        sin = x5.mul_add(sin_tail, sin);
        cos = x6.mul_add(cos_tail, cos);
        (sin, cos)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DrawableVersion {
    V1,
    V2,
}

impl DrawableVersion {
    fn from_observed_class(name: &str) -> Option<Self> {
        match name {
            "WaterColorBrushStrokeDrawableGLV1" => Some(Self::V1),
            "WaterColorBrushStrokeDrawableGLV2" => Some(Self::V2),
            _ => None,
        }
    }
}

struct SavedStrokeInput<'a> {
    points: &'a [Point],
    pressures: &'a [f64],
    timestamps: &'a [i64],
    pen_size: f32,
    color_argb: u32,
    color_type: u8,
    stroke_type: u8,
    curve_enabled: bool,
    tool_type: u8,
    canvas: [u32; 2],
}

#[derive(Clone, Copy)]
struct Dib {
    lanes: [f32; 6],
}

fn sample_saved_stroke(
    input: SavedStrokeInput<'_>,
    version: DrawableVersion,
) -> Result<Vec<[f32; 6]>, &'static str> {
    let n = input.points.len();
    if !(2..=MAX_INPUT_POINTS).contains(&n)
        || input.pressures.len() != n
        || input.timestamps.len() != n
        || input.tool_type != 2
        || input.stroke_type != 0
        || input.color_type != 0
        || !input.curve_enabled
        || !input.pen_size.is_finite()
        || input.pen_size <= 0.0
        || input.pen_size > 512.0
        || input.canvas.iter().any(|&d| d == 0 || d > 16_384)
    {
        return Err("input outside the captured WaterColorBrush style/context contract");
    }
    if input.points.iter().any(|p| {
        !p.x.is_finite()
            || !p.y.is_finite()
            || !(p.x as f32).is_finite()
            || !(p.y as f32).is_finite()
    }) || input
        .pressures
        .iter()
        .any(|p| !p.is_finite() || *p < 0.0 || *p > 16.0)
        || input.timestamps.iter().any(|t| i32::try_from(*t).is_err())
        || input.timestamps.windows(2).any(|w| w[1] <= w[0])
    {
        return Err("non-finite or out-of-range sample channel");
    }
    // The captured AddPoint geometry lanes do not consume ARGB color; the
    // native paint/shader path applies color after this stream is built.
    let _color_for_style_context = input.color_argb;

    let points: Vec<P> = input
        .points
        .iter()
        .map(|p| P {
            x: p.x as f32,
            y: p.y as f32,
        })
        .collect();
    let pressures: Vec<f32> = input
        .pressures
        .iter()
        .map(|p| (*p as f32).min(1.0))
        .collect();
    let times: Vec<i32> = input.timestamps.iter().map(|t| *t as i32).collect();
    let min_dimension = input.canvas[0].min(input.canvas[1]);
    let dimension_scale = 1440.0f32 / min_dimension as f32;

    // RedrawPen initializes from historical point zero, iterates that history
    // (the first drawLine is a no-op), then consumes the current point.
    let mut previous = points[0];
    let mut smooth = previous;
    let mut previous_time = times[0];
    let mut previous_width = (pressures[0] + 2.0) * input.pen_size;
    let mut previous_alpha = 0.0f32;
    let mut residual = 0.0f32;
    let mut phase = 0.0f32;
    let mut needs_initial_dab = true;
    let mut output = Vec::new();

    for i in 1..n {
        let target = points[i];
        let pressure = pressures[i];
        let target_width = (pressure + 2.0) * input.pen_size;
        let spacing = target_width * SAMPLE_STEP_FACTOR;

        // drawLine rejects sub-unit movement before updating its previous point,
        // timestamp, width, or alpha state. No terminal dab is forced on a short
        // ending segment; its unconsumed spacing remainder is retained.
        let dx = previous.x - target.x;
        let dy = previous.y - target.y;
        let distance_squared = dx.mul_add(dx, dy * dy);
        if distance_squared.sqrt() < MIN_MOVEMENT {
            continue;
        }

        let midpoint = P {
            x: (previous.x + target.x) * 0.5,
            y: (previous.y + target.y) * 0.5,
        };
        // SmPath moveTo(previous midpoint), quadTo(previous raw point, new
        // midpoint), then the repository's source-matched SmPath subdivision.
        let curve = Quad::new(smooth, previous, midpoint);
        let length = curve.length;
        if !length.is_finite() || !spacing.is_finite() || spacing <= 0.0 {
            return Err("invalid curve length or dab spacing");
        }

        let mut dt = times[i].wrapping_sub(previous_time);
        if version == DrawableVersion::V2 {
            dt = dt.wrapping_mul(1000);
        }
        if dt > 65_535 {
            dt = 65_535;
        }
        let target_alpha = (length / dt as f32) * dimension_scale;
        let repeat = (length - residual) / spacing;
        if !repeat.is_finite() || repeat > MAX_OUTPUT_DABS as f32 {
            return Err("dab output limit exceeded");
        }
        let count = (repeat as i32).max(0) + 1;
        let count_f = count as f32;
        let alpha_delta = (target_alpha - previous_alpha) / count_f;
        let width_delta = (target_width - previous_width) / count_f;
        let mut distance = residual;
        let mut alpha_state = previous_alpha;
        let mut width_state = previous_width;

        while distance <= length {
            if output.len() == MAX_OUTPUT_DABS {
                return Err("dab output limit exceeded");
            }
            let p = curve.at(distance);
            let (sin, cos) = host_math::sincosf_bounded(phase)
                .ok_or("phase outside the captured host math profile")?;
            output.push(Dib {
                lanes: [
                    p.x,
                    p.y,
                    sin,
                    cos,
                    width_state * 0.5,
                    alpha_lane(alpha_state),
                ],
            });
            needs_initial_dab = false;
            let next_phase = phase + PHASE_STEP;
            phase = if next_phase > TAU {
                next_phase + NEG_TAU
            } else {
                next_phase
            };
            alpha_state += alpha_delta;
            width_state += width_delta;
            distance += spacing;
        }
        residual = distance - length;
        previous = target;
        smooth = midpoint;
        previous_time = times[i];
        previous_width = target_width;
        previous_alpha = target_alpha;
    }

    // Native RedrawPen emits its initial point if no DrawLine call produced a dab.
    if needs_initial_dab {
        output.push(Dib {
            lanes: [
                points[0].x,
                points[0].y,
                0.0,
                1.0,
                previous_width * 0.5,
                alpha_lane(0.0),
            ],
        });
    }
    Ok(output.into_iter().map(|dab| dab.lanes).collect())
}

fn alpha_lane(alpha_state: f32) -> f32 {
    let factor = if alpha_state < 0.0 {
        0.15f32
    } else if alpha_state < 0.003f32 {
        ((0.003f32 - alpha_state) / 0.003f32).mul_add(0.85f32, 0.15f32)
    } else {
        0.15f32
    };
    factor * 0.04f32
}

#[cfg(test)]
mod tests {
    use super::{DrawableVersion, Point, SavedStrokeInput, sample_saved_stroke};
    use serde_json::Value;
    use sha2::{Digest, Sha256};

    const FIXTURE: &str = include_str!("../../../../conformance/watercolor-native.json");
    const NATIVE_STREAMS: &[u8] = include_bytes!("../../../../conformance/watercolor-native.bin");

    fn f32_array(value: &Value) -> Vec<f64> {
        value
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap())
            .collect()
    }

    fn digest(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    #[test]
    fn saved_stroke_v1_v2_match_native_addpoint_streams() {
        let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
        assert_eq!(fixture["schema"], "sdocx-watercolor-native-v1");
        assert_eq!(NATIVE_STREAMS.len(), 720);
        assert_eq!(
            digest(NATIVE_STREAMS),
            fixture["stream"]["sha256"].as_str().unwrap()
        );
        let cases = fixture["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 2);

        for case in cases {
            let observed_class = case["selected_drawable_class"].as_str().unwrap();
            let version = DrawableVersion::from_observed_class(observed_class).unwrap();
            let input = &case["input"];
            let point_rows = input["points_xy"].as_array().unwrap();
            let points: Vec<Point> = point_rows
                .iter()
                .map(|p| Point {
                    x: p[0].as_f64().unwrap(),
                    y: p[1].as_f64().unwrap(),
                })
                .collect();
            let pressures = f32_array(&input["pressure"]);
            let timestamps: Vec<i64> = input["timestamps"]
                .as_array()
                .unwrap()
                .iter()
                .map(|t| t.as_i64().unwrap())
                .collect();
            let canvas = input["canvas"].as_array().unwrap();
            let input = SavedStrokeInput {
                points: &points,
                pressures: &pressures,
                timestamps: &timestamps,
                pen_size: input["pen_size"].as_f64().unwrap() as f32,
                color_argb: u32::from_str_radix(
                    input["color_argb"]
                        .as_str()
                        .unwrap()
                        .trim_start_matches("0x"),
                    16,
                )
                .unwrap(),
                color_type: input["color_type"].as_u64().unwrap() as u8,
                stroke_type: input["stroke_type"].as_u64().unwrap() as u8,
                curve_enabled: input["curve_enabled"].as_bool().unwrap(),
                tool_type: input["tool_type"].as_u64().unwrap() as u8,
                canvas: [
                    canvas[0].as_u64().unwrap() as u32,
                    canvas[1].as_u64().unwrap() as u32,
                ],
            };
            let actual = sample_saved_stroke(input, version).unwrap();
            let expected_offset = case["stream_offset_bytes"].as_u64().unwrap() as usize;
            let expected_count = case["record_count"].as_u64().unwrap() as usize;
            assert_eq!(actual.len(), expected_count);
            for (i, row) in actual.iter().enumerate() {
                for (lane, value) in row.iter().enumerate() {
                    let offset = expected_offset + (i * 6 + lane) * 4;
                    let expected =
                        f32::from_le_bytes(NATIVE_STREAMS[offset..offset + 4].try_into().unwrap());
                    assert_eq!(
                        value.to_bits(),
                        expected.to_bits(),
                        "{observed_class} row {i} lane {lane}"
                    );
                }
            }
        }
    }
}
