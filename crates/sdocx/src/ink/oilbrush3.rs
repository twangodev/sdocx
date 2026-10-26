//! Test-only saved-stroke mesh sampler for OilBrush3 RTV1.
//!
//! This models the cold Drawing `ObjectStroke` route through
//! `OilBrush3StrokeDrawableGLV1::RedrawPen(ObjectStroke const*)` and the
//! `OilBrush3StrokeDrawableRTV1::AddPoint` float-vector append. It stops at the
//! native 28-float quad record; texture generation and final GL composition are
//! separate work and are not represented here. RNG uses the captured glibc
//! profile; atan2f is an explicit captured input/output boundary, so this does
//! not reconstruct the platform math library or establish Android parity.

use super::{
    colored_pencil::GlibcRand,
    path::{P, Quad},
};
use crate::Point;

const MAX_INPUT_POINTS: usize = 4096;
const MAX_OUTPUT_DABS: usize = 65_536;
const OIL_SIZE_SCALE: f32 = f32::from_bits(0x3fa6_6666); // OilBrush3::SetSize rodata: 1.3.
const NEG_ONE_AND_HALF: f32 = -1.5;
const CONTINUITY_DOT_SCALE: f32 = f32::from_bits(0x3e99_999a); // 0.3.
const CONTINUITY_KEEP: f32 = f32::from_bits(0x3f73_3333); // 0.95.
const CONTINUITY_BLEND: f32 = f32::from_bits(0x3d4c_cccd); // 0.05.

/// Bounded input slice for the observed Model `ObjectStroke` route.
///
/// Pressure, timestamps, tilt, orientation, curve flag, and color are retained
/// to make the source-level unused channels explicit. The captured
/// MotionEvent-to-GL path reads only X/Y, tool type, and source for this mesh.
struct SavedStrokeInput<'a> {
    points: &'a [Point],
    pressures: &'a [f32],
    timestamps: &'a [i32],
    tilts: &'a [f32],
    orientations: &'a [f32],
    size: f32,
    screen_resolution: [u32; 2],
    tool_type: i32,
    source: i32,
    curve_enabled: bool,
    color_argb: u32,
}

#[derive(Default)]
struct AddPointState {
    previous_x: f32,
    previous_y: f32,
    previous_unit_x: f32,
    previous_unit_y: f32,
    coefficient: f32,
}

impl AddPointState {
    fn append(&mut self, point: P, half_width: f32, angle: f32, output: &mut Vec<[f32; 28]>) {
        let mut record = [0.0; 28];
        let neg_width = -half_width;
        record[..24].copy_from_slice(&[
            neg_width, neg_width, 0.0, 1.0, neg_width, half_width, 0.0, 0.0, half_width, neg_width,
            1.0, 1.0, half_width, neg_width, 1.0, 1.0, neg_width, half_width, 0.0, 0.0, half_width,
            half_width, 1.0, 0.0,
        ]);

        let (coefficient, unit_x, unit_y) = if angle >= 5.0 {
            // AddPoint's angle>=5 sentinel branch zeros its direction state.
            (0.0, 0.0, 0.0)
        } else {
            let dy = point.y - self.previous_y;
            let dx = point.x - self.previous_x;
            let squared = dx.mul_add(dx, dy * dy);
            let inverse_length = 1.0 / squared.sqrt();
            let unit_y = dy * inverse_length;
            let unit_x = dx * inverse_length;
            let dot = self
                .previous_unit_x
                .mul_add(unit_x, self.previous_unit_y * unit_y)
                .abs();
            let target = dot * CONTINUITY_DOT_SCALE;
            let blended = target.mul_add(CONTINUITY_BLEND, self.coefficient * CONTINUITY_KEEP);
            let coefficient = if self.coefficient > target {
                target
            } else {
                blended
            };
            (coefficient, unit_x, unit_y)
        };

        // Native AddPoint appends source order x, y, continuity coefficient,
        // angle after the six local quad vertices.
        record[24] = point.x;
        record[25] = point.y;
        record[26] = coefficient;
        record[27] = angle;
        output.push(record);

        self.previous_x = point.x;
        self.previous_y = point.y;
        self.previous_unit_x = unit_x;
        self.previous_unit_y = unit_y;
        self.coefficient = coefficient;
    }
}

/// Generate the source-shaped AddPoint record stream for the bounded saved
/// stroke profile captured in `oilbrush3-native.json`.
fn sample_saved_stroke<F>(
    input: &SavedStrokeInput<'_>,
    mut atan2f: F,
) -> Result<Vec<[f32; 28]>, &'static str>
where
    F: FnMut(f32, f32) -> Result<f32, &'static str>,
{
    let count = input.points.len();
    if !(2..=MAX_INPUT_POINTS).contains(&count)
        || input.pressures.len() != count
        || input.timestamps.len() != count
        || input.tilts.len() != count
        || input.orientations.len() != count
        || !input.size.is_finite()
        || input.size <= 0.0
        || input
            .screen_resolution
            .iter()
            .any(|dimension| *dimension == 0 || *dimension > i32::MAX as u32)
        || input.tool_type != 2
        || input.source != 0
        || input
            .points
            .iter()
            .any(|p| !p.x.is_finite() || !p.y.is_finite())
        || input
            .pressures
            .iter()
            .chain(input.tilts)
            .chain(input.orientations)
            .any(|v| !v.is_finite())
    {
        return Err("input is outside the captured OilBrush3 saved-stroke profile");
    }

    // These arrays are present in the actual MotionEvent constructor, but
    // RedrawPen/movePen consume only GetX/GetY, GetToolType, and GetSource.
    let _unused_saved_channels = (
        input.pressures,
        input.timestamps,
        input.tilts,
        input.orientations,
        input.curve_enabled,
        input.color_argb,
    );

    let points: Vec<P> = input
        .points
        .iter()
        .map(|p| P {
            x: p.x as f32,
            y: p.y as f32,
        })
        .collect();
    if points.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) {
        return Err("point is outside finite f32 input range");
    }
    let min_dimension = input.screen_resolution[0].min(input.screen_resolution[1]) as f32;
    let min_move = min_dimension / 1440.0;
    // setScreenResolution stores 3*(min/1440) at drawable+88. For the
    // observed source=0 saved route, the long-segment/tool-type branch is
    // false; the +100 threshold does not change this sampling loop.
    let spacing = min_move * 3.0;
    if !spacing.is_finite() || spacing <= 0.0 {
        return Err("invalid screen-resolution-derived OilBrush3 dab spacing");
    }

    let half_width = (input.size / OIL_SIZE_SCALE) * 0.5;
    let mut output = Vec::with_capacity(32);
    if !half_width.is_finite() || half_width <= 0.0 {
        return Err("invalid OilBrush3 half-width");
    }

    let mut state = AddPointState::default();
    let first = points[0];
    state.append(first, half_width, 5.0, &mut output);

    // startPen seeds three fixed-radius dabs after srand(0). Reuse the
    // source-level glibc TYPE_3 implementation shared with Colored Pencil;
    // Android/bionic RNG parity remains unverified.
    let mut rng = GlibcRand::seeded(0);
    for _ in 0..3 {
        let x = first.x + ((rng.next() & 3) as f32 + NEG_ONE_AND_HALF);
        let y = first.y + ((rng.next() & 3) as f32 + NEG_ONE_AND_HALF);
        state.append(P { x, y }, half_width, 5.0, &mut output);
    }

    let mut previous = first;
    let mut previous_midpoint = first;
    // RedrawPen initializes the next path distance to one spacing interval.
    let mut next_distance = spacing;

    for &target in &points[1..] {
        let dy = target.y - previous.y;
        let dx = target.x - previous.x;
        let movement_squared = dx.mul_add(dx, dy * dy);
        let movement = movement_squared.sqrt();
        if movement < min_move {
            // drawLine returns before changing the stored previous point,
            // midpoint, or distance remainder on this short-move branch.
            continue;
        }

        let midpoint = P {
            x: (previous.x + target.x) * 0.5,
            y: (previous.y + target.y) * 0.5,
        };
        // Native drawLine: moveTo(previous midpoint), quadTo(previous raw
        // point, midpoint(previous raw,current raw)), then reset/getLength.
        let path = Quad::new(previous_midpoint, previous, midpoint);
        if !path.length.is_finite() {
            return Err("non-finite OilBrush3 SmPath length");
        }

        while next_distance <= path.length {
            if output.len() == MAX_OUTPUT_DABS {
                return Err("OilBrush3 AddPoint output limit exceeded");
            }
            let position = path.at(next_distance);
            let tangent = path.normalized_tangent(next_distance);
            // drawLine calls host atan2f(tangent_y, tangent_x) at RVA 0x41518.
            let angle = atan2f(tangent.y, tangent.x)?;
            state.append(position, half_width, angle, &mut output);
            next_distance += spacing;
        }

        next_distance -= path.length;
        previous = target;
        previous_midpoint = midpoint;
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::{SavedStrokeInput, sample_saved_stroke};
    use crate::Point;
    use serde_json::Value;
    use sha2::{Digest, Sha256};

    const FIXTURE: &str = include_str!("../../../../conformance/oilbrush3-native.json");

    struct TestInput {
        points: Vec<Point>,
        pressures: Vec<f32>,
        timestamps: Vec<i32>,
        tilts: Vec<f32>,
        orientations: Vec<f32>,
        size: f32,
        screen_resolution: [u32; 2],
    }

    fn digest(records: &[[f32; 28]]) -> String {
        let mut bytes = Vec::with_capacity(records.len() * 112);
        for record in records {
            for lane in record {
                bytes.extend_from_slice(&lane.to_le_bytes());
            }
        }
        format!("{:x}", Sha256::digest(bytes))
    }

    fn f32_bits(value: &Value) -> u32 {
        u32::from_str_radix(value.as_str().unwrap().trim_start_matches("0x"), 16).unwrap()
    }

    fn input_from_fixture(fixture: &Value, case: &Value) -> TestInput {
        let source = &fixture["input"];
        let points = source["points_xy"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| Point {
                x: p[0].as_f64().unwrap(),
                y: p[1].as_f64().unwrap(),
            })
            .collect();
        let f32s = |key: &str| {
            source[key]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_f64().unwrap() as f32)
                .collect::<Vec<_>>()
        };
        let pressures = f32s("pressures");
        let mut pressures = pressures;
        let pressure_scale = case["pressure_scale"].as_f64().unwrap() as f32;
        for pressure in &mut pressures {
            *pressure *= pressure_scale;
        }
        let mut orientations = f32s("orientations");
        let orientation_offset = case["orientation_offset"].as_f64().unwrap() as f32;
        for orientation in &mut orientations {
            *orientation += orientation_offset;
        }
        let timestamps = source["timestamps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| i32::try_from(value.as_i64().unwrap()).unwrap())
            .collect();
        let size = case["size"].as_f64().unwrap() as f32;
        let screen_resolution = [
            source["screen_resolution"][0].as_u64().unwrap() as u32,
            source["screen_resolution"][1].as_u64().unwrap() as u32,
        ];
        TestInput {
            points,
            pressures,
            timestamps,
            tilts: f32s("tilts"),
            orientations,
            size,
            screen_resolution,
        }
    }

    #[test]
    fn saved_stroke_mesh_matches_native_with_captured_host_math() {
        let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
        assert_eq!(fixture["schema"], "sdocx-oilbrush3-native-v1");
        assert_eq!(
            fixture["scope"]["selected_drawable"],
            "OilBrush3StrokeDrawableGLV1"
        );
        let cases = fixture["cases"].as_array().unwrap();
        let oracle = fixture["atan2f_oracle"]["calls"].as_array().unwrap();
        assert_eq!(cases.len(), 5);
        assert_eq!(oracle.len(), 11);

        for case in cases {
            let values = input_from_fixture(&fixture, case);
            let input = SavedStrokeInput {
                points: &values.points,
                pressures: &values.pressures,
                timestamps: &values.timestamps,
                tilts: &values.tilts,
                orientations: &values.orientations,
                size: values.size,
                screen_resolution: values.screen_resolution,
                tool_type: fixture["input"]["tool_type"].as_i64().unwrap() as i32,
                source: fixture["input"]["source"].as_i64().unwrap() as i32,
                curve_enabled: fixture["input"]["curve_enabled"].as_bool().unwrap(),
                color_argb: u32::from_str_radix(
                    fixture["input"]["color_argb"]
                        .as_str()
                        .unwrap()
                        .trim_start_matches("0x"),
                    16,
                )
                .unwrap(),
            };
            let mut math_call = 0;
            let records = sample_saved_stroke(&input, |y, x| {
                let call = oracle.get(math_call).ok_or("unobserved atan2f call")?;
                if y.to_bits() != f32_bits(&call["s0_y_f32_bits"])
                    || x.to_bits() != f32_bits(&call["s1_x_f32_bits"])
                {
                    return Err("atan2f input differs from native call sequence");
                }
                math_call += 1;
                Ok(f32::from_bits(f32_bits(&call["s0_result_f32_bits"])))
            })
            .unwrap();
            assert_eq!(math_call, 11, "{} atan2f calls", case["name"]);
            assert_eq!(
                records.len(),
                case["addpoint_calls"].as_u64().unwrap() as usize
            );
            assert_eq!(
                records.len() * 112,
                case["stream_bytes"].as_u64().unwrap() as usize
            );
            assert_eq!(
                digest(&records),
                case["stream_sha256"].as_str().unwrap(),
                "{}",
                case["name"]
            );
        }
    }
}

// Native-derived, source-input CPU paint conformance is test-only and bounded to its captured host profile.
#[cfg(test)]
#[path = "oilbrush3/paint_conformance.rs"]
mod paint_conformance;
