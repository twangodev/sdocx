use super::{InkPen2RenderProfile, InkPen2V1OutputScale, RenderTheme, argb_rgb, inkpen2_vector};
use crate::{BoundingBox, Stroke};

const MAX_INKPEN2_SOURCE_SAMPLES: usize = 65_536;
const MAX_INKPEN2_SOURCE_BYTES: usize = 4 * 1024 * 1024;
const MAX_INKPEN2_OUTPUT_RECORDS: usize = 32_768;
const MAX_INKPEN2_ACTIVE_RGB_RECORDS: usize = 512;
const MAX_INKPEN2_ESTIMATED_MARKUP_BYTES: usize = 32 * 1024 * 1024;
const INKPEN2_MARKUP_BUDGET_PER_RECORD: usize = 1024;
const INKPEN2_MARKUP_FIXED_OVERHEAD_BYTES: usize = 4096;

/// Prepare the bounded V1 vector approximation for an explicitly selected output scale.
pub(super) fn prepare_inkpen2_v1(
    stroke: &Stroke,
    profile: Option<InkPen2RenderProfile>,
    theme: RenderTheme,
    scale: InkPen2V1OutputScale,
) -> Option<crate::ink::PreparedInkPen2V9> {
    prepare_inkpen2_internal(stroke, profile, theme, Some(scale), false)
}

/// Prepare V9 records for the explicitly configured fresh-draw host profile.
pub(super) fn prepare_inkpen2_v9(
    stroke: &Stroke,
    profile: Option<InkPen2RenderProfile>,
    theme: RenderTheme,
) -> Option<crate::ink::PreparedInkPen2V9> {
    prepare_inkpen2_internal(stroke, profile, theme, None, false)
}

fn inkpen2_v1_viewport_supported(
    viewport: (u32, u32),
    scale: InkPen2V1OutputScale,
    half_grid: bool,
) -> bool {
    matches!(viewport, (1920, 1080) | (1080, 1920))
        || (matches!(viewport, (1440, 2560) | (2560, 1440))
            && (matches!(
                scale,
                InkPen2V1OutputScale::One | InkPen2V1OutputScale::Double
            ) || half_grid && scale == InkPen2V1OutputScale::Half))
}

pub(super) fn prepare_inkpen2_internal(
    stroke: &Stroke,
    profile: Option<InkPen2RenderProfile>,
    theme: RenderTheme,
    v1_scale: Option<InkPen2V1OutputScale>,
    half_grid: bool,
) -> Option<crate::ink::PreparedInkPen2V9> {
    let profile = profile?;
    if profile.screen_width == 0
        || profile.screen_height == 0
        || profile.screen_width > i32::MAX as u32
        || profile.screen_height > i32::MAX as u32
    {
        return None;
    }
    let rendering = stroke.rendering.as_ref()?;
    let active_rainbow = rendering.properties.rainbow_effect;
    let n = stroke.points.len();
    let source_bytes = n.checked_mul(48);
    let supported_name = match rendering.pen_name.as_deref() {
        Some("com.samsung.android.sdk.pen.pen.preload.InkPen2") => true,
        // Paired native Drawing and host paint controls match for the literal
        // straight alias under normal-color V1 and V9 inputs.
        Some("com.samsung.android.sdk.pen.pen.preload.StraightInkPen2") => {
            !active_rainbow && rendering.style.color_type_raw.unwrap_or(0) == 0
        }
        _ => false,
    };
    let supported_setting = match rendering.advanced_settings.as_deref() {
        None | Some("1;") if v1_scale.is_some() => true,
        Some("9;") => true,
        // Native Drawing selects V9 for a non-null empty setting in both names.
        Some("") => !active_rainbow && rendering.style.color_type_raw.unwrap_or(0) == 0,
        _ => false,
    };
    let unsupported_v1 = v1_scale.is_some_and(|scale| {
        !inkpen2_v1_viewport_supported(
            (profile.screen_width, profile.screen_height),
            scale,
            half_grid,
        ) || profile.enhanced_antialias
            || !matches!(
                rendering.pen_name.as_deref(),
                Some("com.samsung.android.sdk.pen.pen.preload.InkPen2")
                    | Some("com.samsung.android.sdk.pen.pen.preload.StraightInkPen2")
            )
            || !matches!(rendering.advanced_settings.as_deref(), None | Some("1;"))
            // An unresolved table reference is not evidence of a native null setting.
            || (rendering.advanced_settings.is_none()
                && rendering.style.advanced_pen_setting_id.is_some())
            || active_rainbow
            || rendering.style.color_type_raw.unwrap_or(0) != 0
            || rendering.style.color_argb.is_none()
    });
    if unsupported_v1
        || !supported_name
        || !supported_setting
        || !(rendering.tool_type_raw == 2
            || (rendering.tool_type_raw == 3 && v1_scale.is_some()))
        || rendering.style.stroke_type_raw.unwrap_or(0) != 0
        || rendering.style.line_type_raw.unwrap_or(0) != 0
        // Type 1 retains source RGB with rainbow disabled, even with a
        // forwarded palette. Type 2 has no Patternable interface on InkPen2.
        || !matches!(rendering.style.color_type_raw.unwrap_or(0), 0..=2)
        || rendering.properties.replay_only
        || !rendering.properties.stylus_channels
        || rendering.properties.eraser
        || rendering.properties.fixed_width
        || rendering.properties.millisecond_timestamps
        || rendering.properties.top_layer_pen
        || rendering.properties.alpha_lock
        || !rendering.properties.binary_added
        || rendering.properties.generated
        || rendering.properties.fixed_opacity
        || rendering.properties.straighten
        || rendering.properties.reveal_mode
        || !(2..=MAX_INKPEN2_SOURCE_SAMPLES).contains(&n)
        || source_bytes.is_none_or(|bytes| bytes > MAX_INKPEN2_SOURCE_BYTES)
        || stroke.timestamps.len() != n
        || stroke.pressures.len() != n
        || stroke.tilts.len() != n
        || stroke.orientations.len() != n
        || stroke.timestamps.iter().any(|&t| i32::try_from(t).is_err())
        || stroke.points.iter().any(|p| {
            !p.x.is_finite()
                || !p.y.is_finite()
                || !(p.x as f32).is_finite()
                || !(p.y as f32).is_finite()
        })
        || stroke
            .pressures
            .iter()
            .any(|&v| !v.is_finite() || !(v as f32).is_finite())
        || stroke.tilts.iter().any(|&v| !v.is_finite())
        || stroke.orientations.iter().any(|&v| !v.is_finite())
    {
        return None;
    }
    let size = rendering.style.pen_size?;
    if !size.is_finite()
        || !stroke.pen_width.is_finite()
        || size.to_bits() != stroke.pen_width.to_bits()
    {
        return None;
    }
    let argb = rendering.style.color_argb?;
    if v1_scale.is_some()
        && (!(0.2..=12.0).contains(&size)
            || stroke.color.is_some_and(|color| color != argb_rgb(argb)))
    {
        return None;
    }
    let rainbow = if active_rainbow {
        if profile.enhanced_antialias
            || profile.screen_width.min(profile.screen_height) != 1080
            || rendering.style.color_type_raw != Some(1)
            || rendering.style.first_unparsed_field.is_some()
            || !rendering.style.trailing_data.is_empty()
        {
            return None;
        }
        let distance = rendering
            .style
            .rainbow_distance
            .filter(|distance| *distance > 0)?;
        let offset = rendering
            .style
            .rainbow_offset
            .filter(|offset| offset.is_finite() && *offset >= 0.0)?;
        let palette = rendering.style.gradient_colors_argb.as_deref()?;
        if !(2..=256).contains(&palette.len()) {
            return None;
        }
        Some(crate::ink::inkpen2::Rainbow {
            distance,
            offset,
            palette_argb: palette,
        })
    } else {
        None
    };
    // The observed V1 ObjectStroke redraw does not read the Model tolerance.
    // Native controls cover 0, 0.5 and 100; the interval between them is an
    // inference. Other versions require zero: the document factor is unknown.
    let tolerance = rendering.style.initial_tolerance.unwrap_or(0.0);
    if !tolerance.is_finite()
        || if v1_scale.is_some() {
            !(0.0..=100.0).contains(&tolerance)
        } else {
            tolerance != 0.0
        }
    {
        return None;
    }
    let points: Vec<_> = stroke
        .points
        .iter()
        .map(|p| crate::ink::inkpen2::source_point(p.x as f32, p.y as f32))
        .collect();
    let times: Vec<i32> = stroke.timestamps.iter().map(|&t| t as i32).collect();
    let pressures: Vec<f32> = stroke.pressures.iter().map(|&p| p as f32).collect();
    let tools = vec![i32::from(rendering.tool_type_raw); points.len()];
    let input = crate::ink::inkpen2::Input {
        points: &points,
        times: &times,
        tools: &tools,
        pressures: &pressures,
        rgb: [(argb >> 16) as u8, (argb >> 8) as u8, argb as u8],
        rainbow,
        size: if v1_scale.is_some() {
            size
        } else {
            size.clamp(0.4, 800.0)
        },
        curve_enabled: v1_scale.is_some() || rendering.properties.compressed,
        is_shape: false,
        runtime: crate::ink::inkpen2::Runtime {
            inverse_scale: [1.0, 1.0],
            spacing: 1.0,
            tolerance: if v1_scale.is_some() { 2.0 } else { tolerance },
            tool_threshold: 20.0,
            resolution_min: profile.screen_width.min(profile.screen_height) as i32,
            initial_alternation: false,
            initial_time: *times.first().unwrap_or(&0),
        },
    };
    let output_budget = if active_rainbow {
        MAX_INKPEN2_ACTIVE_RGB_RECORDS
    } else {
        MAX_INKPEN2_OUTPUT_RECORDS
    };
    let output = if let Some(scale) = v1_scale {
        let mut output = crate::ink::inkpen2::redraw_v1_bounded(input, output_budget)?;
        output.sample_ends.push(output.terminal.start);
        for [_, _, radius_x, radius_y, edge, coverage, ..] in &mut output.vertices {
            let radius = *radius_x;
            if !radius.is_finite() || radius <= 0.0 {
                return None;
            }
            *radius_y = radius;
            *edge = ((radius - 1.0 / scale.value()).max(0.0) / radius) / 2.0;
            *coverage = 1.0;
        }
        output
    } else {
        crate::ink::inkpen2::redraw_bounded(input, output_budget)?
    };
    let output_bytes = output.vertices.len().checked_mul(36);
    let markup_bytes = output
        .vertices
        .len()
        .checked_mul(INKPEN2_MARKUP_BUDGET_PER_RECORD)
        .and_then(|bytes| bytes.checked_add(INKPEN2_MARKUP_FIXED_OVERHEAD_BYTES));
    if output.vertices.len() > output_budget
        || output_bytes.is_none_or(|bytes| bytes > output_budget * 36)
        || markup_bytes.is_none_or(|bytes| bytes > MAX_INKPEN2_ESTIMATED_MARKUP_BYTES)
        || output.sample_ends.len() != n
        || output.sample_ends.first() != Some(&0)
        || output.sample_ends.windows(2).any(|pair| pair[0] > pair[1])
        || output.sample_ends.last() != Some(&output.terminal.start)
        || output.terminal.start > output.terminal.end
        || output.terminal.end != output.vertices.len()
    {
        return None;
    }
    if output.vertices.iter().any(|record| {
        let [_, _, radius_x, radius_y, _, coverage, ..] = record;
        record.iter().any(|value| !value.is_finite())
            || *radius_x <= 0.0
            || *radius_y <= 0.0
            || !(0.0..=1.0).contains(coverage)
    }) {
        return None;
    }
    if active_rainbow
        && (output
            .vertices
            .iter()
            .any(|[_, _, _, _, _, coverage, ..]| *coverage != 1.0)
            || !inkpen2_vector::valid_record_rgb(&output.vertices))
    {
        return None;
    }
    if output.vertices.is_empty() {
        return None;
    }
    let mut sample_ends = output.sample_ends;
    *sample_ends.last_mut()? = output.terminal.end;
    let mut bounds: Option<BoundingBox> = None;
    for [cx, cy, rx, ry, ..] in &output.vertices {
        let current = BoundingBox {
            x_min: f64::from(*cx) - f64::from(*rx),
            y_min: f64::from(*cy) - f64::from(*ry),
            x_max: f64::from(*cx) + f64::from(*rx),
            y_max: f64::from(*cy) + f64::from(*ry),
        };
        if let Some(old) = &mut bounds {
            old.x_min = old.x_min.min(current.x_min);
            old.y_min = old.y_min.min(current.y_min);
            old.x_max = old.x_max.max(current.x_max);
            old.y_max = old.y_max.max(current.y_max);
        } else {
            bounds = Some(current);
        }
    }
    let source_color = stroke.color.or(Some(argb_rgb(argb)));
    let color = if active_rainbow {
        // The native palette supplies each stamp's RGB. Keep the saved base
        // color intact for alpha/fallback semantics; never theme-map stops.
        argb_rgb(argb)
    } else {
        source_color
            .map(|color| theme.foreground_color(color))
            .unwrap_or_else(|| argb_rgb(argb))
    };
    Some(crate::ink::PreparedInkPen2V9 {
        per_stamp_rgb: active_rainbow,
        records: output.vertices,
        sample_ends,
        color,
        opacity: if v1_scale.is_some() {
            1.0
        } else {
            f64::from((argb >> 24) as u8 as f32 / 255.0)
        },
        enhanced_antialias: profile.enhanced_antialias,
        bounds: bounds?,
    })
}
