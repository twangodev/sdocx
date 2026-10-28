//! Bounded SimpleBrush saved-ObjectStroke geometry reconstruction.
//!
//! This is a numeric geometry oracle only. It does not model the shader,
//! color upload, blending, GPU pixels, or SVG/PDF rendering.

use super::path::{P, Quad};

#[derive(Clone, Copy)]
struct EdgePair([P; 2]);

fn midpoint(a: P, b: P) -> P {
    a.mid(b)
}

fn pressure_step(previous: f32, input: f32, tool_type: i32) -> f32 {
    let requested = if tool_type == 1 { 0.5 } else { input };
    requested.max(previous * 0.9).min(previous * 1.1).min(1.0)
}

fn half_width(size: f32, pressure: f32) -> f32 {
    size.mul_add(pressure, size) * 0.5
}

// Source CurveStyle::setValue (SimpleBrush DSO RVA 0x1c408) computes both
// endpoint offsets in float32. Preserve the fused multiply-add sites and the
// scalar multiply order from that source routine.
fn offset_edges(point: P, dx: f32, dy: f32, width: f32) -> [P; 2] {
    let dx_squared = dx * dx;
    let length_squared = (-dy).mul_add(-dy, dx_squared);
    let length = length_squared.sqrt();
    let determinant = (-point.y).mul_add(dx, point.x * dy);
    let negative_dot = (-dy).mul_add(point.y, (-point.x) * dx);
    let half_width = width * 0.5;

    let edge = |offset: f32| {
        let scaled_offset = offset.mul_add(length, determinant);
        let x_term = scaled_offset * dy;
        let y_term = scaled_offset * (-dx);
        P {
            x: (-dx).mul_add(negative_dot, x_term) / length_squared,
            y: (-dy).mul_add(negative_dot, y_term) / length_squared,
        }
    };

    [edge(-half_width), edge(half_width)]
}

// setValue stores the midpoint of its previous/current endpoint edge pairs.
fn set_value_edges(previous: P, current: P, old_width: f32, new_width: f32) -> EdgePair {
    let dx = current.x - previous.x;
    let dy = current.y - previous.y;
    let old_edges = offset_edges(previous, dx, dy, old_width);
    let new_edges = offset_edges(current, dx, dy, new_width);
    EdgePair([
        midpoint(old_edges[0], new_edges[0]),
        midpoint(old_edges[1], new_edges[1]),
    ])
}

// drawLine and endPen measure the stored cross-section as the next path's
// start width. The subtraction and FMA order matches the source helper.
fn edge_pair_width(pair: EdgePair) -> f32 {
    let dy = pair.0[0].y - pair.0[1].y;
    let dx = pair.0[0].x - pair.0[1].x;
    dx.mul_add(dx, dy * dy).sqrt()
}

fn get_repeat(length: f32, width: f32) -> Result<u32, &'static str> {
    let scaled = if width <= 5.0 {
        length * 0.5
    } else if width <= 10.0 {
        length / 3.0
    } else if width <= 15.0 {
        length * 0.25
    } else if width <= 20.0 {
        length / 5.0
    } else if width <= 30.0 {
        length / 6.0
    } else if width <= 40.0 {
        length / 7.0
    } else if width <= 55.0 {
        length * 0.125
    } else {
        length / 10.0
    };
    if !scaled.is_finite() || scaled < 0.0 || scaled >= u32::MAX as f32 {
        return Err("invalid SimpleBrush path length");
    }
    (scaled as u32)
        .max(2)
        .checked_add(1)
        .ok_or("SimpleBrush repeat overflow")
}

fn append_path(
    curve: &Quad,
    start_width: f32,
    end_width: f32,
    out: &mut Vec<[f32; 3]>,
    max_samples: usize,
) -> Result<(), &'static str> {
    if !curve.length.is_finite()
        || curve.length <= 0.0
        || !start_width.is_finite()
        || !end_width.is_finite()
        || start_width <= 0.0
        || end_width <= 0.0
    {
        return Err("invalid SimpleBrush path geometry");
    }
    let repeat = get_repeat(curve.length, start_width.min(end_width))?;
    let count = usize::try_from(repeat)
        .map_err(|_| "SimpleBrush repeat overflow")?
        .checked_add(1)
        .ok_or("SimpleBrush repeat overflow")?;
    let total = out
        .len()
        .checked_add(count)
        .ok_or("SimpleBrush sample overflow")?;
    if total > max_samples {
        return Err("SimpleBrush sample budget exceeded");
    }
    out.try_reserve(count)
        .map_err(|_| "SimpleBrush sample allocation failed")?;
    let distance_step = curve.length / repeat as f32;
    let width_step = (end_width - start_width) / repeat as f32;
    // Native drawPath and endPen add 1 before accumulating the width step.
    let (mut distance, mut width_plus_one) = (0.0_f32, start_width + 1.0);

    for _ in 0..=repeat {
        let point = curve.at(distance);
        let radius = width_plus_one * 0.5;
        // RT AddPoint stores radius + 0.5 in its third float lane.
        let radius_plus_half = radius + 0.5;
        if !point.x.is_finite()
            || !point.y.is_finite()
            || !radius_plus_half.is_finite()
            || radius_plus_half <= 0.0
        {
            return Err("invalid SimpleBrush sample geometry");
        }
        out.push([point.x, point.y, radius_plus_half]);
        distance += distance_step;
        width_plus_one += width_step;
    }
    Ok(())
}

/// Inputs must already be in the native geometry space. This routine does not
/// derive coordinates, curve flags, or tool types from serialized metadata.
pub(crate) struct NativeInput<'a> {
    pub(crate) points: &'a [[f32; 2]],
    pub(crate) pressures: &'a [f32],
    pub(crate) size: f32,
    pub(crate) tool_type: i32,
    pub(crate) curve_enabled: bool,
    pub(crate) max_samples: usize,
}

pub(crate) fn generate(input: NativeInput<'_>) -> Result<Vec<[f32; 3]>, &'static str> {
    let NativeInput {
        points,
        pressures,
        size,
        tool_type,
        curve_enabled,
        max_samples,
    } = input;
    if !curve_enabled || points.len() < 2 || points.len() != pressures.len() {
        return Err("unsupported SimpleBrush input shape");
    }
    // Each accepted input segment emits samples. Bound source work before
    // allocating or measuring curves, even if the caller supplies no budget.
    if points.len() > max_samples.saturating_add(1) {
        return Err("SimpleBrush sample budget exceeded");
    }
    if points.iter().flatten().any(|v| !v.is_finite())
        || pressures
            .iter()
            .any(|p| !p.is_finite() || !(0.0..=1.0).contains(p))
    {
        return Err("invalid SimpleBrush points or pressures");
    }
    if !size.is_finite() || size <= 0.0 {
        return Err("invalid pen size");
    }

    if points.len() > 2 && matches!(tool_type, 0 | 2 | 3) {
        let mut accepted_points = Vec::with_capacity(points.len());
        let mut accepted_pressures = Vec::with_capacity(points.len());
        accepted_points.push(points[0]);
        accepted_pressures.push(pressures[0]);
        let mut skipped = false;
        for index in 1..points.len() {
            let previous = *accepted_points.last().unwrap();
            let current = points[index];
            if (current[0] - previous[0]).abs() < 1.5 && (current[1] - previous[1]).abs() < 1.5 {
                if index == points.len() - 1 {
                    if accepted_points.len() == 1 {
                        if max_samples == 0 {
                            return Err("SimpleBrush sample budget exceeded");
                        }
                        let width = half_width(size, accepted_pressures[0]).max(1.0);
                        return Ok(vec![[points[0][0], points[0][1], width * 0.5 + 0.5]]);
                    }
                    return generate_with_terminal(
                        &accepted_points,
                        &accepted_pressures,
                        size,
                        tool_type,
                        max_samples,
                        Some((current, pressures[index])),
                    );
                }
                skipped = true;
            } else {
                accepted_points.push(current);
                accepted_pressures.push(pressures[index]);
            }
        }
        if skipped {
            return generate(NativeInput {
                points: &accepted_points,
                pressures: &accepted_pressures,
                size,
                tool_type,
                curve_enabled,
                max_samples,
            });
        }
    }

    generate_with_terminal(points, pressures, size, tool_type, max_samples, None)
}

fn generate_with_terminal(
    points: &[[f32; 2]],
    pressures: &[f32],
    size: f32,
    tool_type: i32,
    max_samples: usize,
    short_terminal: Option<([f32; 2], f32)>,
) -> Result<Vec<[f32; 3]>, &'static str> {
    let mut previous_pressure = if tool_type == 1 {
        0.5
    } else {
        pressures[0].min(1.0)
    };
    let mut previous_width = half_width(size, previous_pressure).max(1.0);
    if points.len() == 2
        && matches!(tool_type, 0 | 2 | 3)
        && (points[1][0] - points[0][0]).abs() < 1.5
        && (points[1][1] - points[0][1]).abs() < 1.5
    {
        if max_samples == 0 {
            return Err("SimpleBrush sample budget exceeded");
        }
        return Ok(vec![[
            points[0][0],
            points[0][1],
            previous_width * 0.5 + 0.5,
        ]]);
    }

    let mut edge_pair = None;
    let mut samples = Vec::new();

    for index in 1..points.len() {
        let previous = P {
            x: points[index - 1][0],
            y: points[index - 1][1],
        };
        let current = P {
            x: points[index][0],
            y: points[index][1],
        };
        if (current.x - previous.x).abs() < 1.5 && (current.y - previous.y).abs() < 1.5 {
            return Err("SimpleBrush drawLine movement is below the native gate");
        }

        let pressure = pressure_step(previous_pressure, pressures[index], tool_type);
        let next_width = half_width(size, pressure);
        let mean_width = (previous_width + next_width) * 0.5;
        let (start, control, end) = if index == 1 {
            (previous, previous, midpoint(previous, current))
        } else {
            let prior = P {
                x: points[index - 2][0],
                y: points[index - 2][1],
            };
            (
                midpoint(prior, previous),
                previous,
                midpoint(previous, current),
            )
        };
        let curve = Quad::new(start, control, end);
        let start_width = if index == 1 {
            mean_width
        } else {
            edge_pair
                .map(edge_pair_width)
                .ok_or("missing source-derived cross-section")?
        };
        append_path(&curve, start_width, mean_width, &mut samples, max_samples)?;

        edge_pair = Some(set_value_edges(
            previous,
            current,
            previous_width,
            next_width,
        ));
        previous_pressure = pressure;
        previous_width = next_width;
    }

    let last = points.len() - 1;
    let terminal_pressure = pressure_step(
        previous_pressure,
        short_terminal.map_or(pressures[last], |(_, pressure)| pressure),
        tool_type,
    );
    let terminal_width = half_width(size, terminal_pressure);
    let terminal_start_width = edge_pair
        .map(edge_pair_width)
        .ok_or("missing terminal source-derived cross-section")?;
    let a = P {
        x: points[last - 1][0],
        y: points[last - 1][1],
    };
    let b = P {
        x: points[last][0],
        y: points[last][1],
    };
    let end = short_terminal.map_or(b, |(point, _)| P {
        x: point[0],
        y: point[1],
    });
    let curve = Quad::new(midpoint(a, b), a, end);
    append_path(
        &curve,
        terminal_start_width,
        terminal_width,
        &mut samples,
        max_samples,
    )?;
    Ok(samples)
}

/// Geometry in explicitly supplied native coordinates, with the source color.
pub(crate) struct PreparedStroke {
    pub(crate) samples: Vec<[f32; 3]>,
    pub(crate) color: crate::Color,
}

/// Admit only the synthetic saved-stroke profile exercised by the geometry
/// capture. The caller must establish that coordinates are already native;
/// this function does not apply or infer a page/model transform. The captured
/// vectors establish geometry, not shader, color, or compositing parity.
pub(crate) fn prepare_stroke(
    stroke: &crate::Stroke,
    native_coordinates: bool,
    max_samples: usize,
) -> Result<PreparedStroke, &'static str> {
    if !native_coordinates {
        return Err("unsupported SimpleBrush coordinate context");
    }
    let rendering = stroke.rendering.as_ref().ok_or("missing stroke metadata")?;
    let p = rendering.properties;
    if rendering.pen_name.as_deref() != Some("com.samsung.android.sdk.pen.pen.preload.Brush")
        || !matches!(
            rendering.advanced_settings.as_deref(),
            None | Some("") | Some("1;")
        )
        || !matches!(rendering.tool_type_raw, 0 | 2 | 3)
        || !p.compressed
        || p.replay_only
        || p.stylus_channels
        || p.eraser
        || p.fixed_width
        || p.millisecond_timestamps
        || p.top_layer_pen
        || p.alpha_lock
        || p.generated
        || p.fixed_opacity
        || p.rainbow_effect
        || p.straighten
        || p.reveal_mode
        || !p.binary_added
        || !matches!(rendering.style.stroke_type_raw, None | Some(0))
        || !matches!(rendering.style.color_type_raw, None | Some(0))
    {
        return Err("unsupported SimpleBrush stroke metadata or channels");
    }
    let size = rendering.style.pen_size.ok_or("missing pen size")?;
    if !(3.0..=32.0).contains(&size) || stroke.pen_width.to_bits() != size.to_bits() {
        return Err("unsupported SimpleBrush size");
    }
    if !(2..=224).contains(&stroke.points.len()) || stroke.pressures.len() != stroke.points.len() {
        return Err("unsupported SimpleBrush sample shape");
    }
    let count = stroke.points.len();
    if !(stroke.timestamps.is_empty() || stroke.timestamps.len() == count)
        || !stroke.tilts.is_empty()
        || !stroke.orientations.is_empty()
    {
        return Err("unsupported SimpleBrush channels");
    }
    let exact_float = |v: f64| -> Result<f32, &'static str> {
        let f = v as f32;
        if !v.is_finite() || f64::from(f) != v {
            return Err("sample is not a finite exact f32");
        }
        Ok(f)
    };
    let points = stroke
        .points
        .iter()
        .map(|p| Ok([exact_float(p.x)?, exact_float(p.y)?]))
        .collect::<Result<Vec<_>, &'static str>>()?;
    let pressures = stroke
        .pressures
        .iter()
        .map(|&p| exact_float(p))
        .collect::<Result<Vec<_>, _>>()?;
    if rendering.tool_type_raw == 0
        && (!(4.0..=12.0).contains(&size)
            || !(2..=16).contains(&points.len())
            || (points.len() > 2
                && points.windows(2).last().is_some_and(|pair| {
                    (pair[1][0] - pair[0][0]).abs() < 1.5 && (pair[1][1] - pair[0][1]).abs() < 1.5
                })))
    {
        return Err("unsupported SimpleBrush tool 0 input");
    }
    let stored_argb = rendering.style.color_argb;
    let color = stroke
        .color
        .or_else(|| {
            stored_argb.map(|argb| crate::Color {
                r: (argb >> 16) as u8,
                g: (argb >> 8) as u8,
                b: argb as u8,
            })
        })
        .ok_or("missing SimpleBrush color")?;
    let samples = generate(NativeInput {
        points: &points,
        pressures: &pressures,
        size,
        tool_type: i32::from(rendering.tool_type_raw),
        curve_enabled: true,
        max_samples,
    })?;
    Ok(PreparedStroke { samples, color })
}

#[cfg(test)]
fn reconstruct(
    points: &[[f32; 2]],
    pressures: &[f32],
    size: f32,
    tool_type: i32,
    curve_enabled: bool,
) -> Result<Vec<[f32; 3]>, &'static str> {
    generate(NativeInput {
        points,
        pressures,
        size,
        tool_type,
        curve_enabled,
        max_samples: 1_000_000,
    })
}

#[cfg(all(test, feature = "serde"))]
mod tests {
    use super::{NativeInput, generate, reconstruct};
    use serde_json::Value;
    use sha2::{Digest, Sha256};

    const FIXTURE: &str = include_str!("../../../../conformance/simplebrush-native.json");
    const ORACLE: &[u8] = include_bytes!("../../../../conformance/simplebrush-native.bin");

    #[test]
    fn short_sequences_match_native_geometry() {
        for fixture in [
            include_str!("../../../../conformance/simplebrush-short-sequences.json"),
            include_str!("../../../../conformance/simplebrush-threshold-sequences.json"),
        ] {
            let cases: Value = serde_json::from_str(fixture).unwrap();
            let value =
                |v: &Value| f32::from_le_bytes(decode_hex(v.as_str().unwrap()).try_into().unwrap());
            for case in cases["cases"].as_array().unwrap() {
                let points: Vec<[f32; 2]> = case["points"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|p| [value(&p[0]), value(&p[1])])
                    .collect();
                let pressures: Vec<f32> = case["pressures"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(value)
                    .collect();
                let actual = reconstruct(&points, &pressures, 8.0, 2, true).unwrap();
                let expected = decode_hex(case["geometry_hex"].as_str().unwrap());
                let expected = expected
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|b| f32::from_le_bytes(*b));
                assert_eq!(actual.len() * 12, expected.len() * 4, "{}", case["case"]);
                for (a, b) in actual.iter().flatten().zip(expected) {
                    assert!((a - b).abs() < 0.0001, "{}: {a} != {b}", case["case"]);
                }
            }
        }
    }

    #[test]
    fn short_terminal_geometry_matches_native() {
        let cases: Value = serde_json::from_str(include_str!(
            "../../../../conformance/simplebrush-short-terminal-geometry.json"
        ))
        .unwrap();
        for case in cases.as_array().unwrap() {
            let points: Vec<[f32; 2]> = serde_json::from_value(case["points"].clone()).unwrap();
            let pressures: Vec<f32> = serde_json::from_value(case["pressures"].clone()).unwrap();
            let size = case["size"].as_f64().unwrap() as f32;
            let actual = reconstruct(&points, &pressures, size, 2, true).unwrap();
            let expected = decode_hex(case["geometry_hex"].as_str().unwrap());
            let expected: Vec<f32> = expected
                .as_chunks::<4>()
                .0
                .iter()
                .map(|v| f32::from_le_bytes(*v))
                .collect();
            assert_eq!(actual.len() * 3, expected.len(), "{}", case["case"]);
            for (a, b) in actual.iter().flatten().zip(expected) {
                assert!((a - b).abs() < 0.0001, "{}: {a} != {b}", case["case"]);
            }
        }
    }

    #[test]
    fn skipped_interior_segments_match_native_geometry() {
        let receipt: Value = serde_json::from_str(include_str!(
            "../../../../conformance/simplebrush-skipped-segments.json"
        ))
        .unwrap();
        let f32_bits = |value: &Value| {
            let bytes = decode_hex(value.as_str().unwrap());
            f32::from_le_bytes(bytes.try_into().unwrap())
        };
        for case in receipt["cases"].as_array().unwrap() {
            let points: Vec<[f32; 2]> = case["points_f32_bits"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| [f32_bits(&p[0]), f32_bits(&p[1])])
                .collect();
            let pressures: Vec<f32> = case["pressure_f32_bits"]
                .as_array()
                .unwrap()
                .iter()
                .map(f32_bits)
                .collect();
            let result = generate(NativeInput {
                points: &points,
                pressures: &pressures,
                size: 8.0,
                tool_type: 2,
                curve_enabled: true,
                max_samples: 1000,
            });
            {
                let bytes: Vec<u8> = result
                    .unwrap()
                    .iter()
                    .flatten()
                    .flat_map(|value| value.to_le_bytes())
                    .collect();
                assert_eq!(
                    bytes,
                    decode_hex(case["geometry_hex"].as_str().unwrap()),
                    "{}",
                    case["case"]
                );
            }
        }
    }

    #[test]
    fn bounded_generator_rejects_exhausted_and_invalid_inputs() {
        let points = [[0., 0.], [10., 0.], [20., 5.]];
        let pressures = [0.5, 0.25, 0.75];
        let input = |budget| NativeInput {
            points: &points,
            pressures: &pressures,
            size: 8.,
            tool_type: 2,
            curve_enabled: true,
            max_samples: budget,
        };
        assert!(generate(input(13)).is_err());
        assert_eq!(generate(input(14)).unwrap().len(), 14);
        let mut invalid = input(14);
        invalid.size = f32::NAN;
        assert!(generate(invalid).is_err());
        let invalid_points = [[0., 0.], [f32::INFINITY, 0.], [20., 5.]];
        let mut invalid = input(14);
        invalid.points = &invalid_points;
        assert!(generate(invalid).is_err());
        let terminal_points = [[0., 0.], [10., 0.], [20., 5.], [21., 6.]];
        let terminal_pressures = [0.5, 0.25, 0.75, 0.4];
        let terminal = |budget| NativeInput {
            points: &terminal_points,
            pressures: &terminal_pressures,
            size: 4.,
            tool_type: 2,
            curve_enabled: true,
            max_samples: budget,
        };
        assert!(generate(terminal(17)).is_err());
        assert_eq!(generate(terminal(18)).unwrap().len(), 18);
        let huge_points = [[0., 0.], [f32::MAX, 0.], [20., 5.]];
        let mut invalid = input(14);
        invalid.points = &huge_points;
        assert!(generate(invalid).is_err());
    }

    use crate::{BoundingBox, Point, Stroke, StrokeProperties, StrokeRendering, StrokeStyle};

    // Coordinates supplied here are already in the native geometry space. The
    // capture does not establish a general page-to-native transform.
    #[derive(Clone, Copy)]
    struct PublicStrokeContext {
        native_coordinates: bool,
        model_matrix: [f32; 9],
    }

    const CAPTURE_CONTEXT: PublicStrokeContext = PublicStrokeContext {
        native_coordinates: true,
        model_matrix: [1., 0., 0., 0., 1., 0., 0., 0., 1.],
    };

    fn sample_public_stroke(
        stroke: &Stroke,
        context: PublicStrokeContext,
    ) -> Result<Vec<[f32; 3]>, &'static str> {
        if !context.native_coordinates || context.model_matrix != CAPTURE_CONTEXT.model_matrix {
            return Err("unsupported SimpleBrush coordinate context");
        }
        let rendering = stroke.rendering.as_ref().ok_or("missing stroke metadata")?;
        let p = rendering.properties;
        if rendering.pen_name.as_deref() != Some("com.samsung.android.sdk.pen.pen.preload.Brush")
            || rendering.advanced_settings.as_deref() != Some("")
            || rendering.tool_type_raw != 2
            || !p.compressed
            || p.replay_only
            || p.stylus_channels
            || p.eraser
            || p.fixed_width
            || p.millisecond_timestamps
            || p.top_layer_pen
            || p.alpha_lock
            || p.generated
            || p.fixed_opacity
            || p.rainbow_effect
            || p.straighten
            || p.reveal_mode
            || !p.binary_added
            || !stroke.timestamps.is_empty()
            || !stroke.tilts.is_empty()
            || !stroke.orientations.is_empty()
        {
            return Err("unsupported SimpleBrush stroke metadata or channels");
        }
        let size = rendering.style.pen_size.ok_or("missing pen size")?;
        if !matches!(size.to_bits(), 0x4080_0000 | 0x4100_0000 | 0x4140_0000)
            || stroke.pen_width.to_bits() != size.to_bits()
        {
            return Err("unsupported SimpleBrush size");
        }
        let exact_float = |v: f64| -> Result<f32, &'static str> {
            let f = v as f32;
            if !v.is_finite() || f64::from(f) != v {
                return Err("sample is not a finite exact f32");
            }
            Ok(f)
        };
        let points = stroke
            .points
            .iter()
            .map(|p| Ok([exact_float(p.x)?, exact_float(p.y)?]))
            .collect::<Result<Vec<_>, &'static str>>()?;
        let pressures = stroke
            .pressures
            .iter()
            .map(|&p| exact_float(p))
            .collect::<Result<Vec<_>, _>>()?;
        if !matches!(points.len(), 3 | 4)
            || (size == 4.0 && points.len() != 3)
            || pressures.len() != points.len()
            || pressures.iter().any(|p| !(0.0..=1.0).contains(p))
        {
            return Err("unsupported SimpleBrush sample shape");
        }
        reconstruct(
            &points,
            &pressures,
            size,
            i32::from(rendering.tool_type_raw),
            p.compressed,
        )
    }

    fn fixture_stroke(input: &Value) -> Stroke {
        let size = float_from_bits(&input["pen_size_f32_bits"]);
        // The native fixture does not record serialization flags or advanced
        // settings. These synthetic public fields only exercise the adapter.
        Stroke {
            rendering: Some(StrokeRendering {
                metadata: None,
                pen_name: Some("com.samsung.android.sdk.pen.pen.preload.Brush".into()),
                advanced_settings: Some(String::new()),
                tool_type_raw: input["tool_type"].as_u64().unwrap() as u16,
                properties: StrokeProperties {
                    // Synthetic saved property bit 0, also the native curve flag.
                    compressed: true,
                    replay_only: false,
                    stylus_channels: false,
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
                    pen_size: Some(size),
                    ..StrokeStyle::default()
                },
            }),
            bbox: BoundingBox::default(),
            points: input["points_f32_bits"]
                .as_array()
                .unwrap()
                .iter()
                .map(|point| {
                    let xy = point.as_array().unwrap();
                    Point {
                        x: f64::from(float_from_bits(&xy[0])),
                        y: f64::from(float_from_bits(&xy[1])),
                    }
                })
                .collect(),
            pressures: input["pressure_f32_bits"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| f64::from(float_from_bits(v)))
                .collect(),
            timestamps: vec![],
            tilts: vec![],
            orientations: vec![],
            color: None,
            pen_width: size,
        }
    }

    #[test]
    fn single_point_native_controls_remain_outside_profile() {
        let controls: Value = serde_json::from_str(include_str!(
            "../../../../conformance/simplebrush-one-point-drawing.json"
        ))
        .unwrap();
        let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
        let mut stroke = fixture_stroke(&fixture["cases"][0]["input"]);
        stroke.rendering.as_mut().unwrap().advanced_settings = None;
        stroke.rendering.as_mut().unwrap().style.color_argb = Some(0x80402010);
        assert!(super::prepare_stroke(&stroke, true, 1000).is_ok());
        stroke.points.truncate(1);
        stroke.pressures.truncate(1);
        for control in controls["controls"].as_array().unwrap() {
            assert_eq!(control["drawing_return"], 0);
            assert_eq!(control["factory_created_pen_count"], 0);
            assert!(control["loader_calls"].as_array().unwrap().is_empty());
            let input = &control["input"];
            let size = input["size"].as_f64().unwrap() as f32;
            stroke.points[0] = crate::Point {
                x: input["points"][0][0].as_f64().unwrap(),
                y: input["points"][0][1].as_f64().unwrap(),
            };
            stroke.pressures[0] = input["pressure"][0].as_f64().unwrap();
            stroke.rendering.as_mut().unwrap().style.pen_size = Some(size);
            stroke.pen_width = size;
            assert!(super::prepare_stroke(&stroke, true, 1000).is_err());
        }
    }

    #[test]
    fn production_adapter_enforces_profile_and_color() {
        let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
        let mut stroke = fixture_stroke(&fixture["cases"][0]["input"]);
        stroke.rendering.as_mut().unwrap().advanced_settings = None;
        assert!(super::prepare_stroke(&stroke, true, 1000).is_err());
        stroke.rendering.as_mut().unwrap().style.color_argb = Some(0x80402010);
        let prepared = super::prepare_stroke(&stroke, true, 1000).unwrap();
        assert_eq!(
            prepared.color,
            crate::Color {
                r: 0x40,
                g: 0x20,
                b: 0x10
            }
        );
        assert_eq!(prepared.samples.len(), 14);
        assert!(super::prepare_stroke(&stroke, false, 1000).is_err());
        assert!(super::prepare_stroke(&stroke, true, 1).is_err());
        stroke.color = Some(crate::Color { r: 1, g: 2, b: 3 });
        stroke.rendering.as_mut().unwrap().style.color_argb = None;
        assert_eq!(
            super::prepare_stroke(&stroke, true, 1000).unwrap().color,
            stroke.color.unwrap()
        );
        stroke.rendering.as_mut().unwrap().advanced_settings = Some(String::new());
        assert!(super::prepare_stroke(&stroke, true, 1000).is_ok());
        stroke.rendering.as_mut().unwrap().advanced_settings = Some("1;".into());
        assert!(super::prepare_stroke(&stroke, true, 1000).is_ok());
        stroke.rendering.as_mut().unwrap().advanced_settings = Some("2;".into());
        assert!(super::prepare_stroke(&stroke, true, 1000).is_err());
        stroke.rendering.as_mut().unwrap().advanced_settings = None;
        stroke.rendering.as_mut().unwrap().style.pen_size = Some(4.0);
        stroke.pen_width = 4.0;
        assert!(super::prepare_stroke(&stroke, true, 1000).is_ok());
        stroke.points.push(crate::Point { x: 29.0, y: 11.0 });
        stroke.pressures.push(f64::from(0.4_f32));
        assert!(super::prepare_stroke(&stroke, true, 1000).is_ok());
    }

    #[test]
    fn tool0_admission_bounds_remain_conservative() {
        let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
        let mut stroke = fixture_stroke(&fixture["cases"][0]["input"]);
        let rendering = stroke.rendering.as_mut().unwrap();
        rendering.advanced_settings = None;
        rendering.tool_type_raw = 0;
        rendering.style.color_argb = Some(0x80402010);
        stroke.points = (0..16)
            .map(|i| crate::Point {
                x: f64::from(i * 8),
                y: 0.,
            })
            .collect();
        stroke.pressures = vec![0.5; 16];
        stroke.timestamps.clear();
        assert!(super::prepare_stroke(&stroke, true, 1000).is_ok());
        stroke.points.push(crate::Point { x: 128., y: 0. });
        stroke.pressures.push(0.5);
        assert!(super::prepare_stroke(&stroke, true, 1000).is_err());
        stroke.points.truncate(3);
        stroke.pressures.truncate(3);
        stroke.points[1].x = 0.5;
        assert!(super::prepare_stroke(&stroke, true, 1000).is_ok());
        stroke.points[2].x = 1.0;
        assert!(super::prepare_stroke(&stroke, true, 1000).is_err());
        stroke.points.truncate(2);
        stroke.pressures.truncate(2);
        assert!(super::prepare_stroke(&stroke, true, 1000).is_ok());
        stroke.rendering.as_mut().unwrap().style.pen_size = Some(13.0);
        stroke.pen_width = 13.0;
        assert!(super::prepare_stroke(&stroke, true, 1000).is_err());
    }

    #[test]
    fn production_size_range_is_bounded_and_finite() {
        let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
        let mut stroke = fixture_stroke(&fixture["cases"][0]["input"]);
        stroke.rendering.as_mut().unwrap().advanced_settings = None;
        stroke.rendering.as_mut().unwrap().style.color_argb = Some(0x80402010);
        for (size, accepted) in [
            (2.99, false),
            (3.0, true),
            (4.0, true),
            (5.5, true),
            (20.0, true),
            (24.0, true),
            (28.0, true),
            (32.0, true),
            (32.01, false),
            (f32::NAN, false),
            (f32::INFINITY, false),
        ] {
            stroke.rendering.as_mut().unwrap().style.pen_size = Some(size);
            stroke.pen_width = size;
            assert_eq!(
                super::prepare_stroke(&stroke, true, 1000).is_ok(),
                accepted,
                "size {size}"
            );
        }
    }

    #[test]
    fn public_stroke_adapter_rejects_unsupported_inputs() {
        let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
        let stroke = fixture_stroke(&fixture["cases"][0]["input"]);
        assert!(sample_public_stroke(&stroke, CAPTURE_CONTEXT).is_ok());
        let mut context = CAPTURE_CONTEXT;
        context.native_coordinates = false;
        assert!(sample_public_stroke(&stroke, context).is_err());
        context = CAPTURE_CONTEXT;
        context.model_matrix[0] = 2.;
        assert!(sample_public_stroke(&stroke, context).is_err());
        let mut changed = stroke.clone();
        changed.rendering.as_mut().unwrap().properties.compressed = false;
        assert!(sample_public_stroke(&changed, CAPTURE_CONTEXT).is_err());
        changed = stroke.clone();
        changed.rendering.as_mut().unwrap().pen_name = None;
        assert!(sample_public_stroke(&changed, CAPTURE_CONTEXT).is_err());
        changed = stroke.clone();
        changed.pressures.pop();
        assert!(sample_public_stroke(&changed, CAPTURE_CONTEXT).is_err());
        changed = stroke.clone();
        changed.points[0].x = f64::NAN;
        assert!(sample_public_stroke(&changed, CAPTURE_CONTEXT).is_err());
        changed = stroke.clone();
        changed.pen_width = 9.;
        assert!(sample_public_stroke(&changed, CAPTURE_CONTEXT).is_err());
        changed = stroke;
        changed.tilts.push(0.);
        assert!(sample_public_stroke(&changed, CAPTURE_CONTEXT).is_err());
    }

    fn decode_hex(text: &str) -> Vec<u8> {
        assert_eq!(text.len() % 2, 0);
        (0..text.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&text[index..index + 2], 16).unwrap())
            .collect()
    }

    fn float_from_bits(value: &Value) -> f32 {
        let bytes: [u8; 4] = decode_hex(value.as_str().unwrap()).try_into().unwrap();
        f32::from_le_bytes(bytes)
    }

    #[test]
    fn saved_object_stroke_geometry_matches_native_vectors() {
        let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
        let cases = fixture["cases"].as_array().unwrap();
        let oracle_hash = format!("{:x}", Sha256::digest(ORACLE));
        assert_eq!(
            fixture["provenance"]["binary_oracle"]["sha256"],
            oracle_hash
        );

        for case in cases {
            let input = &case["input"];
            assert_eq!(input["curve_enabled"].as_bool(), Some(true));
            let stroke = fixture_stroke(input);
            let samples = sample_public_stroke(&stroke, CAPTURE_CONTEXT)
                .unwrap_or_else(|error| panic!("{}: {error}", case["id"].as_str().unwrap()));

            let mut bytes = Vec::with_capacity(samples.len() * 12);
            for [x, y, radius_plus_half] in samples {
                bytes.extend_from_slice(&x.to_le_bytes());
                bytes.extend_from_slice(&y.to_le_bytes());
                bytes.extend_from_slice(&radius_plus_half.to_le_bytes());
            }

            let native = &case["native_oracle"];
            let offset = native["vector_offset_bytes"].as_u64().unwrap() as usize;
            let length = native["vector_length_bytes"].as_u64().unwrap() as usize;
            let expected = &ORACLE[offset..offset + length];
            assert_eq!(
                bytes,
                expected,
                "{} vector mismatch",
                case["id"].as_str().unwrap()
            );
            assert_eq!(
                format!("{:x}", Sha256::digest(&bytes)),
                native["vector_sha256"].as_str().unwrap(),
                "{} native vector SHA mismatch",
                case["id"].as_str().unwrap()
            );
        }
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn two_point_direction_and_distance_native_controls() {
        for probe in 201..=204 {
            v25_paint_control(8, false, false, false, probe);
        }
    }

    #[test]
    fn two_point_short_pressure_and_budget_match_native() {
        let receipt: Value = serde_json::from_str(include_str!(
            "../../../../conformance/simplebrush-two-short-pressure-geometry.json"
        ))
        .unwrap();
        for case in receipt["cases"].as_array().unwrap() {
            let points: Vec<[f32; 2]> = case["points"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| [p[0].as_f64().unwrap() as f32, p[1].as_f64().unwrap() as f32])
                .collect();
            let pressures: Vec<f32> = case["pressures"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| p.as_f64().unwrap() as f32)
                .collect();
            let input = || NativeInput {
                points: &points,
                pressures: &pressures,
                size: case["size"].as_f64().unwrap() as f32,
                tool_type: 2,
                curve_enabled: true,
                max_samples: 1,
            };
            let bytes: Vec<u8> = generate(input())
                .unwrap()
                .iter()
                .flatten()
                .flat_map(|v| v.to_le_bytes())
                .collect();
            assert_eq!(
                bytes,
                decode_hex(case["geometry_hex"].as_str().unwrap()),
                "{}",
                case["id"]
            );
            assert_eq!(
                format!("{:x}", Sha256::digest(&bytes)),
                case["geometry_sha256"].as_str().unwrap()
            );
            assert!(
                generate(NativeInput {
                    max_samples: 0,
                    ..input()
                })
                .is_err()
            );
        }
    }

    // This compares a source-derived 1px radial fringe with the actual generic
    // page renderer. These are bounded host-Mesa controls, not a parity claim.
    #[test]
    fn tool3_three_point_native_geometry_control() {
        use std::fmt::Write;
        let receipt: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../conformance/simplebrush-tool3-native.json"
        ))
        .unwrap();
        let samples = reconstruct(
            &[[0.0, 0.0], [10.0, 0.0], [20.0, 5.0]],
            &[0.75, 0.25, 1.0],
            8.0,
            3,
            true,
        )
        .unwrap();
        let mut actual = String::new();
        for value in samples.iter().flatten() {
            for byte in value.to_le_bytes() {
                write!(actual, "{byte:02x}").unwrap();
            }
        }
        assert_eq!(
            actual,
            receipt["controls"]["three"]["geometry_hex"]
                .as_str()
                .unwrap()
        );
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn tool3_two_point_native_paint_control() {
        for probe in [2, 201, 202, 203] {
            v25_paint_control_for_tool(8, false, false, false, probe, 3);
        }
        v25_paint_control_for_tool(8, false, false, false, 0, 3);
        v25_paint_control_for_tool(12, false, false, false, 7, 3);
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn tool0_two_point_native_paint_control() {
        v25_paint_control_for_tool(8, false, false, false, 2, 0);
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn tool0_short_and_six_point_native_paint_controls() {
        v25_paint_control_for_tool(8, false, false, false, 203, 0);
        v25_paint_control_for_tool(8, false, false, false, 6, 0);
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn tool3_long_native_paint_controls() {
        for probe in [16, 64, 224] {
            v25_paint_control_for_tool(8, false, false, false, probe, 3);
        }
        v25_paint_control_for_tool(20, false, false, false, 16, 3);
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn tool3_short_multi_point_native_paint_controls() {
        v25_paint_control_for_tool(8, false, false, false, 205, 3);
        v25_paint_control_for_tool(4, false, false, false, 207, 3);
    }

    #[cfg(feature = "pdf")]
    fn v25_paint_control(size: u32, four: bool, alt: bool, six: bool, probe: usize) {
        v25_paint_control_for_tool(size, four, alt, six, probe, 2);
    }

    #[cfg(feature = "pdf")]
    fn v25_paint_control_for_tool(
        size: u32,
        four: bool,
        alt: bool,
        six: bool,
        probe: usize,
        tool: u16,
    ) {
        use crate::{
            BoundingBox, Color, Document, Page, Point, Stroke, StrokeProperties, StrokeRendering,
            StrokeStyle,
        };
        use std::fmt::Write;

        // OpenGL bottom-up rows, cropped at x=0..crop_width, y=0..crop_height. Full-frame
        // See each receipt for the full-frame SHA-256;
        // all nonzero alpha lies in this crop. Source: V25 host-Mesa capture.
        let extra = matches!(probe, 216 | 217)
            || matches!(
                (size, probe),
                (10, 0) | (55, 7) | (20, 16) | (3 | 24, 7 | 16)
            );
        let native: &[u8] = match (four, size) {
            _ if tool == 0 && probe == 203 => {
                include_bytes!("../../../../conformance/simplebrush-tool0-short-crop.rgba")
            }
            _ if tool == 0 && probe == 6 => {
                include_bytes!("../../../../conformance/simplebrush-tool0-six-crop.rgba")
            }
            _ if probe == 216 => {
                include_bytes!("../../../../conformance/simplebrush-size32-crop.rgba")
            }
            _ if probe == 217 => {
                include_bytes!("../../../../conformance/simplebrush-size28-crop.rgba")
            }
            _ if probe == 215 => {
                include_bytes!("../../../../conformance/simplebrush-fractional-crop.rgba")
            }
            _ if probe == 214 => {
                include_bytes!("../../../../conformance/simplebrush-manyskip-crop.rgba")
            }
            _ if probe == 211 => {
                include_bytes!("../../../../conformance/simplebrush-threshold-below-crop.rgba")
            }
            _ if probe == 212 => {
                include_bytes!("../../../../conformance/simplebrush-threshold-equal-crop.rgba")
            }
            _ if probe == 213 => {
                include_bytes!("../../../../conformance/simplebrush-threshold-above-crop.rgba")
            }
            _ if probe == 210 => {
                include_bytes!("../../../../conformance/simplebrush-short-large-crop.rgba")
            }
            _ if probe == 209 => {
                include_bytes!("../../../../conformance/simplebrush-two-short-crop.rgba")
            }
            _ if probe == 208 => {
                include_bytes!("../../../../conformance/simplebrush-terminal-vertical-crop.rgba")
            }
            _ if probe == 207 => {
                include_bytes!("../../../../conformance/simplebrush-terminal-crop.rgba")
            }
            _ if probe == 205 => {
                include_bytes!("../../../../conformance/simplebrush-first-skip-crop.rgba")
            }
            _ if probe == 206 => {
                include_bytes!("../../../../conformance/simplebrush-repeat-skip-crop.rgba")
            }
            _ if probe == 201 => {
                include_bytes!("../../../../conformance/simplebrush-two-vertical-crop.rgba")
            }
            _ if probe == 202 => {
                include_bytes!("../../../../conformance/simplebrush-two-reversed-crop.rgba")
            }
            _ if probe == 203 => {
                include_bytes!("../../../../conformance/simplebrush-two-short-crop.rgba")
            }
            _ if probe == 204 => {
                include_bytes!("../../../../conformance/simplebrush-two-zero-crop.rgba")
            }

            (_, 4) if probe == 2 => {
                include_bytes!("../../../../conformance/simplebrush-two-4-crop.rgba")
            }
            (_, 8) if probe == 2 => {
                include_bytes!("../../../../conformance/simplebrush-two-8-crop.rgba")
            }
            (_, 16) if probe == 2 => {
                include_bytes!("../../../../conformance/simplebrush-two-16-crop.rgba")
            }
            (_, 3) if probe == 7 => {
                include_bytes!("../../../../conformance/simplebrush-edge-7-3-crop.rgba")
            }
            (_, 3) if probe == 16 => {
                include_bytes!("../../../../conformance/simplebrush-edge-16-3-crop.rgba")
            }
            (_, 24) if probe == 7 => {
                include_bytes!("../../../../conformance/simplebrush-edge-7-24-crop.rgba")
            }
            (_, 24) if probe == 16 => {
                include_bytes!("../../../../conformance/simplebrush-edge-16-24-crop.rgba")
            }
            (_, 10) => include_bytes!("../../../../conformance/simplebrush-size-3-10-crop.rgba"),
            (_, 55) => include_bytes!("../../../../conformance/simplebrush-size-7-55-crop.rgba"),
            (_, 20) => include_bytes!("../../../../conformance/simplebrush-size-16-20-crop.rgba"),
            _ if probe == 5 => include_bytes!("../../../../conformance/simplebrush-5-8-crop.rgba"),
            _ if probe == 7 => include_bytes!("../../../../conformance/simplebrush-7-12-crop.rgba"),
            _ if probe == 17 => {
                include_bytes!("../../../../conformance/simplebrush-17-8-crop.rgba")
            }
            _ if probe == 32 => {
                include_bytes!("../../../../conformance/simplebrush-32-8-crop.rgba")
            }
            _ if probe == 64 => {
                include_bytes!("../../../../conformance/simplebrush-64-8-crop.rgba")
            }
            _ if probe == 128 => {
                include_bytes!("../../../../conformance/simplebrush-128-8-crop.rgba")
            }
            _ if probe == 224 => {
                include_bytes!("../../../../conformance/simplebrush-224-8-crop.rgba")
            }
            _ if probe == 192 => {
                include_bytes!("../../../../conformance/simplebrush-192-8-crop.rgba")
            }
            _ if probe == 16 => {
                include_bytes!("../../../../conformance/simplebrush-16-8-crop.rgba")
            }
            (_, 4) if six => {
                include_bytes!("../../../../conformance/simplebrush-six-size4-crop.rgba")
            }
            (_, 16) if six => {
                include_bytes!("../../../../conformance/simplebrush-six-size16-crop.rgba")
            }
            (true, 16) if alt => include_bytes!(
                "../../../../conformance/simplebrush-paint-v25-four-size16-alt-crop.rgba"
            ),
            (true, 16) => include_bytes!(
                "../../../../conformance/simplebrush-paint-v25-four-size16-crop.rgba"
            ),
            (true, 4) if alt => include_bytes!(
                "../../../../conformance/simplebrush-paint-v25-four-size4-alt-crop.rgba"
            ),
            (false, 4) => {
                include_bytes!("../../../../conformance/simplebrush-paint-v25-size4-crop.rgba")
            }
            (false, 8) => {
                include_bytes!("../../../../conformance/simplebrush-paint-v25-crop.rgba")
            }
            (false, 12) => {
                include_bytes!("../../../../conformance/simplebrush-paint-v25-size12-crop.rgba")
            }
            (false, 16) => {
                include_bytes!("../../../../conformance/simplebrush-paint-v25-size16-crop.rgba")
            }
            (true, 4) => {
                include_bytes!("../../../../conformance/simplebrush-paint-v25-four-size4-crop.rgba")
            }
            (true, 8) => {
                include_bytes!("../../../../conformance/simplebrush-paint-v25-four-crop.rgba")
            }
            (true, 12) => include_bytes!(
                "../../../../conformance/simplebrush-paint-v25-four-size12-crop.rgba"
            ),
            _ => unreachable!(),
        };
        let receipt: Value = serde_json::from_str(match (four, size) {
            _ if tool == 0 && probe == 203 => {
                include_str!("../../../../conformance/simplebrush-tool0-short.json")
            }
            _ if tool == 0 && probe == 6 => {
                include_str!("../../../../conformance/simplebrush-tool0-six.json")
            }
            _ if probe == 216 => include_str!("../../../../conformance/simplebrush-size32.json"),
            _ if probe == 217 => include_str!("../../../../conformance/simplebrush-size28.json"),
            _ if probe == 215 => {
                include_str!("../../../../conformance/simplebrush-fractional.json")
            }
            _ if probe == 214 => include_str!("../../../../conformance/simplebrush-manyskip.json"),
            _ if probe == 211 => {
                include_str!("../../../../conformance/simplebrush-threshold-below.json")
            }
            _ if probe == 212 => {
                include_str!("../../../../conformance/simplebrush-threshold-equal.json")
            }
            _ if probe == 213 => {
                include_str!("../../../../conformance/simplebrush-threshold-above.json")
            }
            _ if probe == 210 => {
                include_str!("../../../../conformance/simplebrush-short-large.json")
            }
            _ if probe == 209 => include_str!("../../../../conformance/simplebrush-two-short.json"),
            _ if probe == 208 => {
                include_str!("../../../../conformance/simplebrush-terminal-vertical.json")
            }
            _ if probe == 207 => include_str!("../../../../conformance/simplebrush-terminal.json"),
            _ if probe == 205 => {
                include_str!("../../../../conformance/simplebrush-first-skip.json")
            }
            _ if probe == 206 => {
                include_str!("../../../../conformance/simplebrush-repeat-skip.json")
            }
            _ if probe == 201 => {
                include_str!("../../../../conformance/simplebrush-two-vertical.json")
            }
            _ if probe == 202 => {
                include_str!("../../../../conformance/simplebrush-two-reversed.json")
            }
            _ if probe == 203 => include_str!("../../../../conformance/simplebrush-two-short.json"),
            _ if probe == 204 => include_str!("../../../../conformance/simplebrush-two-zero.json"),

            (_, 4) if probe == 2 => include_str!("../../../../conformance/simplebrush-two-4.json"),
            (_, 8) if probe == 2 => include_str!("../../../../conformance/simplebrush-two-8.json"),
            (_, 16) if probe == 2 => {
                include_str!("../../../../conformance/simplebrush-two-16.json")
            }
            (_, 3) if probe == 7 => {
                include_str!("../../../../conformance/simplebrush-edge-7-3.json")
            }
            (_, 3) if probe == 16 => {
                include_str!("../../../../conformance/simplebrush-edge-16-3.json")
            }
            (_, 24) if probe == 7 => {
                include_str!("../../../../conformance/simplebrush-edge-7-24.json")
            }
            (_, 24) if probe == 16 => {
                include_str!("../../../../conformance/simplebrush-edge-16-24.json")
            }
            (_, 10) => include_str!("../../../../conformance/simplebrush-size-3-10.json"),
            (_, 55) => include_str!("../../../../conformance/simplebrush-size-7-55.json"),
            (_, 20) => include_str!("../../../../conformance/simplebrush-size-16-20.json"),
            _ if probe == 5 => include_str!("../../../../conformance/simplebrush-5-8.json"),
            _ if probe == 7 => include_str!("../../../../conformance/simplebrush-7-12.json"),
            _ if probe == 17 => include_str!("../../../../conformance/simplebrush-17-8.json"),
            _ if probe == 32 => include_str!("../../../../conformance/simplebrush-32-8.json"),
            _ if probe == 64 => include_str!("../../../../conformance/simplebrush-64-8.json"),
            _ if probe == 128 => include_str!("../../../../conformance/simplebrush-128-8.json"),
            _ if probe == 224 => include_str!("../../../../conformance/simplebrush-224-8.json"),
            _ if probe == 192 => include_str!("../../../../conformance/simplebrush-192-8.json"),
            _ if probe == 16 => include_str!("../../../../conformance/simplebrush-16-8.json"),
            (_, 4) if six => include_str!("../../../../conformance/simplebrush-six-size4.json"),
            (_, 16) if six => include_str!("../../../../conformance/simplebrush-six-size16.json"),
            (true, 16) if alt => {
                include_str!("../../../../conformance/simplebrush-paint-v25-four-size16-alt.json")
            }
            (true, 16) => {
                include_str!("../../../../conformance/simplebrush-paint-v25-four-size16.json")
            }
            (true, 4) if alt => {
                include_str!("../../../../conformance/simplebrush-paint-v25-four-size4-alt.json")
            }
            (false, 4) => include_str!("../../../../conformance/simplebrush-paint-v25-size4.json"),
            (false, 8) => include_str!("../../../../conformance/simplebrush-paint-v25.json"),
            (false, 12) => {
                include_str!("../../../../conformance/simplebrush-paint-v25-size12.json")
            }
            (false, 16) => {
                include_str!("../../../../conformance/simplebrush-paint-v25-size16.json")
            }
            (true, 4) => {
                include_str!("../../../../conformance/simplebrush-paint-v25-four-size4.json")
            }
            (true, 8) => {
                include_str!("../../../../conformance/simplebrush-paint-v25-four.json")
            }
            (true, 12) => {
                include_str!("../../../../conformance/simplebrush-paint-v25-four-size12.json")
            }
            _ => unreachable!(),
        })
        .unwrap();
        assert_eq!(
            format!("{:x}", Sha256::digest(native)),
            (if extra || six || probe != 0 {
                &receipt["crop"]["sha256"]
            } else {
                &receipt["paint_observation"]["crop"]["sha256"]
            })
            .as_str()
            .unwrap()
        );
        let probe_points: Vec<[f32; 2]> = (0..if (201..=215).contains(&probe) {
            2
        } else {
            probe
        })
            .map(|i| [(i * 8) as f32, (4 + (i % 3) * 4) as f32])
            .collect();
        let probe_pressures: Vec<f32> = (0..if (201..=215).contains(&probe) {
            2
        } else {
            probe
        })
            .map(|i| {
                if probe == 2 && size != 8 {
                    0.75
                } else {
                    [0.25, 0.5, 0.75, 0.375][i % 4]
                }
            })
            .collect();
        let points: &[[f32; 2]] = if matches!(probe, 216 | 217) {
            &[[0., 0.], [10., 0.], [20., 5.]]
        } else if probe == 215 {
            &[
                [0., 0.],
                [10., 0.],
                [10.5, 0.5],
                [11., 1.],
                [9.5, 0.3],
                [10.2, -0.3],
                [22., 7.],
                [22.3, 7.1],
                [22.6, 7.3],
                [22.1, 7.2],
                [32., 3.],
                [32.4, 3.2],
            ]
        } else if probe == 214 {
            &[
                [0., 0.],
                [10., 0.],
                [10.5, 0.5],
                [11., 1.],
                [9.5, 0.25],
                [10.25, -0.25],
                [22., 7.],
                [22.25, 7.25],
                [22.5, 7.25],
                [22.25, 7.5],
                [32., 3.],
                [32.5, 3.25],
            ]
        } else if probe == 211 {
            &[[4., 4.], [5.499, 4.], [5.6, 4.]]
        } else if probe == 212 {
            &[[4., 4.], [5.5, 4.], [5.6, 4.]]
        } else if probe == 213 {
            &[[4., 4.], [5.501, 4.], [5.6, 4.]]
        } else if probe == 210 {
            &[[4., 4.], [14., 4.], [14.2, 4.]]
        } else if probe == 209 {
            &[[4., 4.], [4.1, 4.], [4.2, 4.]]
        } else if probe == 208 {
            &[[0., 0.], [0., 10.], [5., 20.], [6., 21.]]
        } else if probe == 207 {
            &[[0., 0.], [10., 0.], [20., 5.], [21., 6.]]
        } else if probe == 205 {
            &[[0., 0.], [1., 1.], [10., 0.], [29., 11.]]
        } else if probe == 206 {
            &[[0., 0.], [10., 0.], [11., 1.], [10.5, 0.5], [29., 11.]]
        } else if probe == 201 {
            &[[4., 0.], [4., 8.]]
        } else if probe == 202 {
            &[[8., 8.], [0., 4.]]
        } else if probe == 203 {
            &[[4., 4.], [4.01, 4.]]
        } else if probe == 204 {
            &[[4., 4.], [4., 4.]]
        } else if probe != 0 {
            &probe_points
        } else if six {
            &[
                [0., 0.],
                [10., 0.],
                [20., 5.],
                [29., 11.],
                [37., 8.],
                [45., 15.],
            ]
        } else if alt {
            &[[0., 0.], [8., 8.], [17., 3.], [27., 8.]]
        } else if four {
            &[[0., 0.], [10., 0.], [20., 5.], [29., 11.]]
        } else {
            &[[0., 0.], [10., 0.], [20., 5.]]
        };
        let pressures: &[f32] = if matches!(probe, 216 | 217) {
            &[0.75, 0.25, 1.0]
        } else if matches!(probe, 214 | 215) {
            &[
                0.1, 0.9, 0.05, 0.95, 0.2, 0.8, 0.15, 1.0, 0.0, 0.85, 0.3, 0.9,
            ]
        } else if matches!(probe, 211..=213) {
            &[0.25, 0.75, 0.5]
        } else if matches!(probe, 209 | 210) {
            &[0.25, 0.5, 0.75]
        } else if probe == 208 {
            &[1., 1., 1., 1.]
        } else if matches!(probe, 205 | 207) {
            &[0.5, 0.25, 0.75, 0.4]
        } else if probe == 206 {
            &[0.5, 0.25, 0.75, 0.9, 0.4]
        } else if probe != 0 {
            &probe_pressures
        } else if six {
            &[0.5, 0.25, 0.75, 0.4, 0.6, 0.3]
        } else if four {
            &[0.5, 0.25, 0.75, 0.4]
        } else {
            &[0.5, 0.25, 0.75]
        };
        let expected_support = match (four, size) {
            _ if tool == 0 && probe == 6 => 337,
            _ if probe == 216 => 612,
            _ if probe == 217 => 522,
            _ if probe == 215 => 192,
            _ if probe == 214 => 194,
            _ if matches!(probe, 211..=213) => 48,
            _ if probe == 210 => 108,
            _ if probe == 209 => 32,
            _ if probe == 208 => 350,
            _ if probe == 207 => 86,
            _ if probe == 205 => 223,
            _ if probe == 206 => 215,
            _ if probe == 201 || probe == 202 => 80,
            _ if probe == 203 || probe == 204 => 32,
            (_, 4) if probe == 2 => 60,
            (_, 8) if probe == 2 => 85,
            (_, 16) if probe == 2 => 223,
            (_, 3) if probe == 7 => 202,
            (_, 3) if probe == 16 => 502,
            (_, 24) if probe == 7 => 958,
            (_, 24) if probe == 16 => 2266,
            (_, 10) => 167,
            (_, 55) => 312,
            (_, 20) => 2008,
            _ if probe == 5 => 273,
            _ if probe == 7 => 574,
            _ if probe == 16 => 985,
            _ if probe == 17 => 1036,
            _ if probe == 32 => 1993,
            _ if probe == 64 => 4045,
            _ if probe == 128 => 8113,
            _ if probe == 192 => 12192,
            _ if probe == 224 => 14233,
            (_, 4) if six => 211,
            (_, 16) if six => 623,
            (false, 4) => 80,
            (false, 8) => 137,
            (false, 12) => 185,
            (false, 16) => 249,
            (true, 16) if alt => 406,
            (true, 16) => 384,
            (true, 4) if alt => 151,
            (true, 4) => 128,
            (true, 8) => 214,
            (true, 12) => 295,
            _ => unreachable!(),
        };
        let size = if size == 55 { 5.5 } else { size as f32 };
        let (crop_width, crop_height) = if matches!(probe, 216 | 217) {
            (64, 32)
        } else if extra || probe != 0 {
            (
                if probe > 16 && !(201..=215).contains(&probe) {
                    probe * 8 + 16
                } else {
                    144
                },
                32,
            )
        } else if six {
            (64, 32)
        } else if four || size == 16.0 {
            (40, 20)
        } else {
            (32, 16)
        };
        let samples = reconstruct(points, pressures, size, 2, true).unwrap();
        assert_eq!(
            samples.len(),
            if extra || probe != 0 {
                receipt["geometry_count"].as_u64().unwrap() as usize
            } else if six && size == 4.0 {
                36
            } else if six {
                24
            } else if four && size == 16.0 {
                16
            } else if alt {
                22
            } else if four && size == 4.0 {
                24
            } else if four {
                18
            } else if size == 16.0 {
                12
            } else if size == 4.0 {
                17
            } else {
                14
            }
        );
        let geometry: Vec<u8> = samples
            .iter()
            .flatten()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        if extra || probe != 0 {
            let native_geometry: &[u8] = match (size, probe) {
                _ if tool == 0 && probe == 203 => {
                    include_bytes!("../../../../conformance/simplebrush-tool0-short-geometry.bin")
                }
                _ if tool == 0 && probe == 6 => {
                    include_bytes!("../../../../conformance/simplebrush-tool0-six-geometry.bin")
                }
                _ if probe == 216 => {
                    include_bytes!("../../../../conformance/simplebrush-size32-geometry.bin")
                }
                _ if probe == 217 => {
                    include_bytes!("../../../../conformance/simplebrush-size28-geometry.bin")
                }
                _ if probe == 215 => {
                    include_bytes!("../../../../conformance/simplebrush-fractional-geometry.bin")
                }
                _ if probe == 214 => {
                    include_bytes!("../../../../conformance/simplebrush-manyskip-geometry.bin")
                }
                _ if probe == 211 => include_bytes!(
                    "../../../../conformance/simplebrush-threshold-below-geometry.bin"
                ),
                _ if probe == 212 => include_bytes!(
                    "../../../../conformance/simplebrush-threshold-equal-geometry.bin"
                ),
                _ if probe == 213 => include_bytes!(
                    "../../../../conformance/simplebrush-threshold-above-geometry.bin"
                ),
                _ if probe == 210 => {
                    include_bytes!("../../../../conformance/simplebrush-short-large-geometry.bin")
                }
                _ if probe == 209 => {
                    include_bytes!("../../../../conformance/simplebrush-two-short-geometry.bin")
                }
                _ if probe == 208 => include_bytes!(
                    "../../../../conformance/simplebrush-terminal-vertical-geometry.bin"
                ),
                _ if probe == 207 => {
                    include_bytes!("../../../../conformance/simplebrush-terminal-geometry.bin")
                }
                _ if probe == 205 => {
                    include_bytes!("../../../../conformance/simplebrush-first-skip-geometry.bin")
                }
                _ if probe == 206 => {
                    include_bytes!("../../../../conformance/simplebrush-repeat-skip-geometry.bin")
                }
                _ if probe == 201 => {
                    include_bytes!("../../../../conformance/simplebrush-two-vertical-geometry.bin")
                }
                _ if probe == 202 => {
                    include_bytes!("../../../../conformance/simplebrush-two-reversed-geometry.bin")
                }
                _ if probe == 203 => {
                    include_bytes!("../../../../conformance/simplebrush-two-short-geometry.bin")
                }
                _ if probe == 204 => {
                    include_bytes!("../../../../conformance/simplebrush-two-zero-geometry.bin")
                }

                (4.0, 2) => {
                    include_bytes!("../../../../conformance/simplebrush-two-4-geometry.bin")
                }
                (8.0, 2) => {
                    include_bytes!("../../../../conformance/simplebrush-two-8-geometry.bin")
                }
                (16.0, 2) => {
                    include_bytes!("../../../../conformance/simplebrush-two-16-geometry.bin")
                }
                (3.0, 7) => {
                    include_bytes!("../../../../conformance/simplebrush-edge-7-3-geometry.bin")
                }
                (3.0, 16) => {
                    include_bytes!("../../../../conformance/simplebrush-edge-16-3-geometry.bin")
                }
                (24.0, 7) => {
                    include_bytes!("../../../../conformance/simplebrush-edge-7-24-geometry.bin")
                }
                (24.0, 16) => {
                    include_bytes!("../../../../conformance/simplebrush-edge-16-24-geometry.bin")
                }
                (10.0, 0) => {
                    include_bytes!("../../../../conformance/simplebrush-size-3-10-geometry.bin")
                }
                (5.5, 7) => {
                    include_bytes!("../../../../conformance/simplebrush-size-7-55-geometry.bin")
                }
                (20.0, 16) => {
                    include_bytes!("../../../../conformance/simplebrush-size-16-20-geometry.bin")
                }
                (_, 5) => include_bytes!("../../../../conformance/simplebrush-5-8-geometry.bin"),
                (_, 7) => include_bytes!("../../../../conformance/simplebrush-7-12-geometry.bin"),
                (_, 17) => include_bytes!("../../../../conformance/simplebrush-17-8-geometry.bin"),
                (_, 32) => include_bytes!("../../../../conformance/simplebrush-32-8-geometry.bin"),
                (_, 64) => include_bytes!("../../../../conformance/simplebrush-64-8-geometry.bin"),
                (_, 128) => {
                    include_bytes!("../../../../conformance/simplebrush-128-8-geometry.bin")
                }
                (_, 224) => {
                    include_bytes!("../../../../conformance/simplebrush-224-8-geometry.bin")
                }
                (_, 192) => {
                    include_bytes!("../../../../conformance/simplebrush-192-8-geometry.bin")
                }
                (_, 16) => include_bytes!("../../../../conformance/simplebrush-16-8-geometry.bin"),
                _ => unreachable!(),
            };
            assert_eq!(geometry, native_geometry);
        }
        assert_eq!(
            format!("{:x}", Sha256::digest(&geometry)),
            (if extra || six || probe != 0 {
                &receipt["geometry_sha256"]
            } else {
                &receipt["input"]["geometry_vector_sha256"]
            })
            .as_str()
            .unwrap()
        );
        let mut svg = String::from(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="1920" height="1080" viewBox="0 0 1920 1080"><defs>"##,
        );
        for (i, &[x, y, r]) in samples.iter().enumerate() {
            let y = 1080.0 - f64::from(y);
            let inner = (f64::from(r) - 1.0).max(0.0) / f64::from(r);
            write!(svg, r##"<radialGradient id="g{i}" gradientUnits="userSpaceOnUse" cx="{x}" cy="{y}" r="{r}"><stop offset="0" stop-color="#44aaee"/><stop offset="{inner}" stop-color="#44aaee"/><stop offset="1" stop-color="#44aaee" stop-opacity="0"/></radialGradient>"##).unwrap();
        }
        svg.push_str("</defs>");
        for (i, &[x, y, r]) in samples.iter().enumerate() {
            let y = 1080.0 - f64::from(y);
            write!(
                svg,
                r##"<circle cx="{x}" cy="{y}" r="{r}" fill="url(#g{i})"/>"##
            )
            .unwrap();
        }
        svg.push_str("</svg>");

        let stroke = Stroke {
            rendering: Some(StrokeRendering {
                metadata: None,
                pen_name: Some("com.samsung.android.sdk.pen.pen.preload.Brush".into()),
                advanced_settings: None,
                tool_type_raw: tool,
                properties: StrokeProperties {
                    compressed: true,
                    replay_only: false,
                    stylus_channels: false,
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
                    color_argb: Some(0x8044_aaee),
                    pen_size: Some(size),
                    ..StrokeStyle::default()
                },
            }),
            bbox: BoundingBox::default(),
            points: points
                .iter()
                .map(|p| Point {
                    x: f64::from(p[0]),
                    // Probe 215 evaluates a rounded candidate; the production guard
                    // still rejects the unrounded flipped coordinate.
                    y: if probe == 215 {
                        f64::from((1080. - f64::from(p[1])) as f32)
                    } else {
                        1080. - f64::from(p[1])
                    },
                })
                .collect(),
            pressures: pressures.iter().copied().map(f64::from).collect(),
            timestamps: vec![],
            tilts: vec![],
            orientations: vec![],
            color: Some(Color {
                r: 0x44,
                g: 0xaa,
                b: 0xee,
            }),
            pen_width: size,
        };
        assert!(
            super::prepare_stroke(&stroke, true, 100000).is_ok(),
            "tool {tool} probe {probe}: {:?}",
            super::prepare_stroke(&stroke, true, 100000).err()
        );
        // The native fragment shader ignores input alpha in this capture. The
        // generic Brush route also uses whole-stroke opacity 1, despite the
        // saved 0x80 alpha; compare the actual routes, not an assumed opacity.
        assert_eq!(crate::prepare_stroke(&stroke, false).opacity, 1.0);
        let mut paired_stroke = stroke.clone();
        paired_stroke.rendering.as_mut().unwrap().tool_type_raw = 2;
        let document = Document {
            pages: vec![Page {
                uuid: "simplebrush-v25".into(),
                width: 1920,
                height: 1080,
                content_bbox: BoundingBox::default(),
                background_color: None,
                template: None,
                background: Default::default(),
                objects: vec![stroke.into()],
            }],
            metadata: Default::default(),
        };
        if matches!(tool, 0 | 3) {
            let mut paired_document = document.clone();
            paired_document.pages[0].objects = vec![paired_stroke.into()];
            let options = crate::RenderOptions {
                simplebrush_profile: Some(crate::SimpleBrushRenderProfile::HostV25),
                ..Default::default()
            };
            let pdf_options =
                crate::PdfOptions::from_font_book(&crate::render::fonts::FontBook::default());
            let actual = crate::render_document_pdf(&document, &options, &pdf_options).unwrap();
            let paired =
                crate::render_document_pdf(&paired_document, &options, &pdf_options).unwrap();
            assert_eq!(actual, paired);
        }
        let layout = crate::layout_document(&document);
        let request =
            crate::render::PageSceneRequest::sliced(&layout.pages[0], 0, 1080, false).unwrap();
        let mut cache = crate::render::DocumentTextCache::default();
        let fallback = cache
            .render_layout_page_scene_requests(
                &document,
                &[request],
                &crate::RenderOptions::default(),
                &crate::render::fonts::FontBook::default(),
                &mut |_| {},
            )
            .remove(0)
            .page
            .svg;
        let request =
            crate::render::PageSceneRequest::sliced(&layout.pages[0], 0, 1080, false).unwrap();
        let profiled = cache
            .render_layout_page_scene_requests(
                &document,
                &[request],
                &crate::RenderOptions {
                    simplebrush_profile: Some(crate::SimpleBrushRenderProfile::HostV25),
                    ..Default::default()
                },
                &crate::render::fonts::FontBook::default(),
                &mut |_| {},
            )
            .remove(0)
            .page
            .svg;
        if let Some(directory) = std::env::var_os("SDOCX_SIMPLEBRUSH_EXPORT_DIR") {
            let directory = std::path::Path::new(&directory);
            std::fs::create_dir_all(directory).unwrap();
            for (name, source) in [
                ("candidate", &svg),
                ("fallback", &fallback),
                ("profiled", &profiled),
            ] {
                let page = crate::RenderedPage::new(0, 1920, 1080, source.clone());
                let pdf = crate::render_svg_pages_pdf(&[page], &Default::default()).unwrap();
                use std::io::Write as _;
                let path = directory.join(format!("{name}-{size}-{four}-{alt}-{six}-{probe}.pdf"));
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(path)
                    .unwrap();
                file.write_all(&pdf).unwrap();
            }
        }
        let raster = |source: &str| {
            let tree =
                resvg::usvg::Tree::from_str(source, &resvg::usvg::Options::default()).unwrap();
            let mut pixmap = resvg::tiny_skia::Pixmap::new(1920, 1080).unwrap();
            resvg::render(
                &tree,
                resvg::tiny_skia::Transform::identity(),
                &mut pixmap.as_mut(),
            );
            pixmap
        };
        let candidate = raster(&svg);
        let profiled = raster(&profiled);
        let fallback = raster(&fallback);
        // Both use the same transparent, bottom-up native reference. Count
        // support intersection/union and absolute alpha error across the crop.
        let metrics = |image: &resvg::tiny_skia::Pixmap| {
            let mut intersection = 0usize;
            let mut union = 0usize;
            let mut alpha_error = 0u64;
            let mut rgba_error = 0u64;
            let mut support = 0usize;
            for y in 0..crop_height {
                for x in 0..crop_width {
                    let n = native[(y * crop_width + x) * 4 + 3];
                    let a = image.data()[((1079 - y) * 1920 + x) * 4 + 3];
                    intersection += usize::from(n != 0 && a != 0);
                    union += usize::from(n != 0 || a != 0);
                    support += usize::from(a != 0);
                    alpha_error += u64::from(n.abs_diff(a));
                    let native_pixel = &native[(y * crop_width + x) * 4..][..4];
                    let output_pixel = &image.data()[((1079 - y) * 1920 + x) * 4..][..4];
                    rgba_error += native_pixel
                        .iter()
                        .zip(output_pixel)
                        .map(|(native, actual)| u64::from(native.abs_diff(*actual)))
                        .sum::<u64>();
                }
            }
            (intersection, union, support, alpha_error, rgba_error)
        };
        let candidate_metrics = metrics(&candidate);
        let profiled_metrics = metrics(&profiled);
        let fallback_metrics = metrics(&fallback);
        for (name, image, inside_support) in [
            ("candidate", &candidate, candidate_metrics.2),
            ("profiled", &profiled, profiled_metrics.2),
            ("fallback", &fallback, fallback_metrics.2),
        ] {
            let full_support = image
                .data()
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|p| p[3] != 0)
                .count();
            assert_eq!(
                full_support, inside_support,
                "{name} extends outside the crop"
            );
        }
        eprintln!(
            "v25 candidate (intersection, union, support, alpha error, RGBA error): {candidate_metrics:?}; profiled: {profiled_metrics:?}; fallback: {fallback_metrics:?}"
        );
        assert_eq!(
            native
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|p| p[3] != 0)
                .count(),
            expected_support
        );
        if !extra && probe <= 16 && probe != 2 {
            assert_eq!(
                candidate_metrics.2,
                expected_support
                    + usize::from((four && size == 12.0) || probe == 7) * 2
                    + usize::from(probe == 16)
            );
        }
        assert!(
            candidate_metrics.0 * fallback_metrics.1 > fallback_metrics.0 * candidate_metrics.1
        );
        assert!(candidate_metrics.3 < fallback_metrics.3);
        assert!(candidate_metrics.4 < fallback_metrics.4);
        assert!(profiled_metrics.0 * fallback_metrics.1 > fallback_metrics.0 * profiled_metrics.1);
        assert!(profiled_metrics.3 < fallback_metrics.3);
        assert!(profiled_metrics.4 < fallback_metrics.4);
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn larger_size_public_paint_controls() {
        v25_paint_control(32, false, false, false, 216);
        v25_paint_control(28, false, false, false, 217);
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn skipped_segment_public_paint_controls() {
        for probe in [209, 210, 211, 212, 213, 214, 215] {
            v25_paint_control(8, false, false, false, probe);
        }
        for probe in [205, 206] {
            v25_paint_control(8, false, false, false, probe);
        }
        v25_paint_control(4, false, false, false, 207);
        v25_paint_control(16, false, false, false, 208);
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn two_point_public_paint_controls() {
        for size in [4, 8, 16] {
            v25_paint_control(size, false, false, false, 2);
        }
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn size_edge_public_paint_controls() {
        for size in [3, 24] {
            for probe in [7, 16] {
                v25_paint_control(size, false, false, false, probe);
            }
        }
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn intermediate_size_public_paint_controls() {
        v25_paint_control(10, false, false, false, 0);
        v25_paint_control(55, false, false, false, 7);
        v25_paint_control(20, false, false, false, 16);
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn six_point_public_paint_controls() {
        v25_paint_control(4, false, false, true, 0);
        v25_paint_control(16, false, false, true, 0);
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn longer_public_paint_controls() {
        v25_paint_control(8, false, false, false, 5);
        v25_paint_control(12, false, false, false, 7);
        v25_paint_control(8, false, false, false, 16);
        v25_paint_control(8, false, false, false, 17);
        v25_paint_control(8, false, false, false, 32);
        v25_paint_control(8, false, false, false, 64);
        v25_paint_control(8, false, false, false, 128);
        v25_paint_control(8, false, false, false, 192);
        v25_paint_control(8, false, false, false, 224);
    }

    #[test]
    fn size4_candidate_geometry_matches_native() {
        let receipt: Value = serde_json::from_str(include_str!(
            "../../../../conformance/simplebrush-paint-v25-size4.json"
        ))
        .unwrap();
        let samples = reconstruct(
            &[[0., 0.], [10., 0.], [20., 5.]],
            &[0.5, 0.25, 0.75],
            4.,
            2,
            true,
        )
        .unwrap();
        assert_eq!(samples.len(), 17);
        let geometry: Vec<u8> = samples
            .iter()
            .flatten()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        assert_eq!(
            format!("{:x}", Sha256::digest(&geometry)),
            receipt["input"]["geometry_vector_sha256"].as_str().unwrap()
        );
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn v25_size4_paint_control() {
        v25_paint_control(4, false, false, false, 0);
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn v25_size16_paint_control() {
        v25_paint_control(16, false, false, false, 0);
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn v25_candidate_and_generic_fallback_against_host_paint() {
        v25_paint_control(8, false, false, false, 0);
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn v25_size12_candidate_and_generic_fallback_against_host_paint() {
        v25_paint_control(12, false, false, false, 0);
    }
    #[cfg(feature = "pdf")]
    #[test]
    fn v25_four_point_paint_control() {
        v25_paint_control(4, true, false, false, 0);
        v25_paint_control(4, true, true, false, 0);
        v25_paint_control(8, true, false, false, 0);
        v25_paint_control(12, true, false, false, 0);
        v25_paint_control(16, true, false, false, 0);
        v25_paint_control(16, true, true, false, 0);
    }
}

#[cfg(test)]
mod six_point_native_probe {
    use super::{NativeInput, generate};

    #[test]
    fn six_point_native_geometry() {
        let points = [
            [0., 0.],
            [10., 0.],
            [20., 5.],
            [29., 11.],
            [37., 8.],
            [45., 15.],
        ];
        let pressures = [0.5, 0.25, 0.75, 0.4, 0.6, 0.3];
        for (size, expected) in [
            (
                4.,
                &include_bytes!("../../../../conformance/simplebrush-six-size4-geometry.bin")[..],
            ),
            (
                16.,
                &include_bytes!("../../../../conformance/simplebrush-six-size16-geometry.bin")[..],
            ),
        ] {
            let samples = generate(NativeInput {
                points: &points,
                pressures: &pressures,
                size,
                tool_type: 2,
                curve_enabled: true,
                max_samples: 10000,
            })
            .unwrap();
            let bytes: Vec<u8> = samples
                .into_iter()
                .flatten()
                .flat_map(f32::to_le_bytes)
                .collect();
            assert_eq!(bytes, expected, "size {size}");
        }
    }
}
