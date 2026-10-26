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

/// Context that is external to a public `Stroke` but observed for this one
/// synthetic Drawing profile. The selected GL version and source are never
/// inferred from a missing stroke field.
#[derive(Clone, Copy)]
struct PublicStrokeContext {
    selected_gl_version: u32,
    source: i32,
    screen_resolution: [u32; 2],
    model_matrix: [f32; 9],
}

/// Adapt only the pinned OilBrush3 Model setter profile into the sampler.
/// This remains test-only and does not turn the synthetic input into evidence
/// for general serialized-stroke admission.
fn sample_public_stroke<F>(
    stroke: &crate::Stroke,
    context: PublicStrokeContext,
    atan2f: F,
) -> Result<Vec<[f32; 28]>, &'static str>
where
    F: FnMut(f32, f32) -> Result<f32, &'static str>,
{
    const PEN_NAME: &str = "com.samsung.android.sdk.pen.pen.preload.OilBrush3";
    const ADVANCED_SETTING: &str = "";
    const SIZE_BITS: u32 = 0x4100_0000; // Model ObjectStroke SetPenSize(8.0).
    const ARGB: u32 = 0x8044_aaee;
    const IDENTITY: [f32; 9] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];

    if context.selected_gl_version != 1
        || context.source != 0
        || context.screen_resolution != [1920, 1080]
        || context.model_matrix != IDENTITY
    {
        return Err("Drawing context is outside the captured OilBrush3 GLV1 profile");
    }
    let rendering = stroke
        .rendering
        .as_ref()
        .ok_or("resolved OilBrush3 stroke metadata is required")?;
    let style = &rendering.style;
    let properties = rendering.properties;
    if rendering.pen_name.as_deref() != Some(PEN_NAME)
        || rendering.advanced_settings.as_deref() != Some(ADVANCED_SETTING)
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
    {
        return Err("stroke metadata is outside the captured OilBrush3 profile");
    }
    let size = style
        .pen_size
        .ok_or("explicit ObjectStroke pen size is required")?;
    let argb = style
        .color_argb
        .ok_or("explicit ObjectStroke ARGB is required")?;
    if size.to_bits() != SIZE_BITS || stroke.pen_width.to_bits() != SIZE_BITS || argb != ARGB {
        return Err("style values are outside the captured OilBrush3 profile");
    }
    let color = stroke
        .color
        .ok_or("RGB color derived from ARGB is required")?;
    if [color.r, color.g, color.b]
        != [
            ((ARGB >> 16) & 0xff) as u8,
            ((ARGB >> 8) & 0xff) as u8,
            (ARGB & 0xff) as u8,
        ]
    {
        return Err("Stroke RGB and retained ARGB disagree");
    }
    let timestamps = stroke
        .timestamps
        .iter()
        .map(|&value| i32::try_from(value).map_err(|_| "timestamp is outside i32 range"))
        .collect::<Result<Vec<_>, _>>()?;
    let pressures = stroke
        .pressures
        .iter()
        .map(|&value| value as f32)
        .collect::<Vec<_>>();
    let tilts = stroke
        .tilts
        .iter()
        .map(|&value| value as f32)
        .collect::<Vec<_>>();
    let orientations = stroke
        .orientations
        .iter()
        .map(|&value| value as f32)
        .collect::<Vec<_>>();
    let input = SavedStrokeInput {
        points: &stroke.points,
        pressures: &pressures,
        timestamps: &timestamps,
        tilts: &tilts,
        orientations: &orientations,
        size,
        screen_resolution: context.screen_resolution,
        tool_type: i32::from(rendering.tool_type_raw),
        source: context.source,
        curve_enabled: properties.compressed,
        color_argb: argb,
    };
    sample_saved_stroke(&input, atan2f)
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
    use super::{PublicStrokeContext, SavedStrokeInput, sample_public_stroke, sample_saved_stroke};
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

    fn public_stroke_from_fixture(fixture: &Value) -> crate::Stroke {
        let case = &fixture["cases"][0];
        let values = input_from_fixture(fixture, case);
        let input = &fixture["input"];
        let argb = u32::from_str_radix(
            input["color_argb"]
                .as_str()
                .unwrap()
                .trim_start_matches("0x"),
            16,
        )
        .unwrap();
        crate::Stroke {
            rendering: Some(crate::StrokeRendering {
                metadata: None,
                // Explicit Model setter inputs from the V10 controlled Drawing capture.
                pen_name: Some("com.samsung.android.sdk.pen.pen.preload.OilBrush3".to_owned()),
                advanced_settings: Some(String::new()),
                tool_type_raw: input["tool_type"].as_u64().unwrap() as u16,
                properties: crate::StrokeProperties {
                    compressed: input["curve_enabled"].as_bool().unwrap(),
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
                    color_argb: Some(argb),
                    pen_size: Some(values.size),
                    size_level: Some(0),
                    stroke_type_raw: Some(0),
                    color_type_raw: Some(0),
                    ..crate::StrokeStyle::default()
                },
            }),
            bbox: crate::BoundingBox::default(),
            points: values.points,
            pressures: values.pressures.into_iter().map(f64::from).collect(),
            timestamps: values.timestamps.into_iter().map(i64::from).collect(),
            tilts: values.tilts.into_iter().map(f64::from).collect(),
            orientations: values.orientations.into_iter().map(f64::from).collect(),
            color: Some(crate::Color {
                r: ((argb >> 16) & 0xff) as u8,
                g: ((argb >> 8) & 0xff) as u8,
                b: (argb & 0xff) as u8,
            }),
            pen_width: values.size,
        }
    }

    #[test]
    fn public_stroke_adapter_reconstructs_v10_drawing_call_stream() {
        let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
        let stroke = public_stroke_from_fixture(&fixture);
        let context = PublicStrokeContext {
            selected_gl_version: 1,
            source: 0,
            screen_resolution: [1920, 1080],
            model_matrix: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        };
        let oracle = fixture["atan2f_oracle"]["calls"].as_array().unwrap();
        assert_eq!(oracle.len(), 11);
        let mut math_call = 0;
        let records = sample_public_stroke(&stroke, context, |y, x| {
            let call = oracle.get(math_call).ok_or("unobserved atan2f call")?;
            if y.to_bits() != f32_bits(&call["s0_y_f32_bits"])
                || x.to_bits() != f32_bits(&call["s1_x_f32_bits"])
            {
                return Err("atan2f input differs from pinned native call sequence");
            }
            math_call += 1;
            Ok(f32::from_bits(f32_bits(&call["s0_result_f32_bits"])))
        })
        .unwrap();
        assert_eq!(math_call, 11);
        assert_eq!(records.len(), 15);
        assert_eq!(records.len() * 112, 1680);
        assert_eq!(
            digest(&records),
            "8862a946e50f1df2a4a74e940f58d3e03f839875e104d9cdeeba6efdd486870c"
        );

        let mut wrong_context = context;
        wrong_context.selected_gl_version = 2;
        assert!(sample_public_stroke(&stroke, wrong_context, |_, _| Ok(0.0)).is_err());
        let mut unresolved = stroke.clone();
        unresolved.rendering.as_mut().unwrap().pen_name = None;
        assert!(sample_public_stroke(&unresolved, context, |_, _| Ok(0.0)).is_err());

        let mut inconsistent = stroke.clone();
        inconsistent.pen_width = f32::from_bits(inconsistent.pen_width.to_bits() + 1);
        assert!(sample_public_stroke(&inconsistent, context, |_, _| Ok(0.0)).is_err());
        inconsistent = stroke.clone();
        inconsistent.color.as_mut().unwrap().r ^= 1;
        assert!(sample_public_stroke(&inconsistent, context, |_, _| Ok(0.0)).is_err());

        let mut incomplete = stroke.clone();
        incomplete.tilts.pop();
        assert!(sample_public_stroke(&incomplete, context, |_, _| Ok(0.0)).is_err());
        let mut out_of_range = stroke.clone();
        out_of_range.timestamps[0] = i64::from(i32::MAX) + 1;
        assert!(sample_public_stroke(&out_of_range, context, |_, _| Ok(0.0)).is_err());
        let mut unsupported = stroke.clone();
        unsupported
            .rendering
            .as_mut()
            .unwrap()
            .properties
            .rainbow_effect = true;
        assert!(sample_public_stroke(&unsupported, context, |_, _| Ok(0.0)).is_err());
    }
}

// Native-derived, source-input CPU paint conformance is test-only and bounded to its captured host profile.
#[cfg(test)]
#[path = "oilbrush3/paint_conformance.rs"]
mod paint_conformance;
