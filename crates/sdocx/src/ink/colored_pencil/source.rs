use super::super::path::P;
use super::{PenData, StrokeInput, sample};
use crate::{InkRasterError, Stroke, StrokeStyle};

pub(crate) fn prepare_stroke(
    stroke: &Stroke,
    pen_data: [u32; 2],
) -> Result<(Vec<[f32; 5]>, u32), InkRasterError> {
    if pen_data.iter().any(|v| !(1..=16_384).contains(v)) {
        return Err(InkRasterError::InvalidContext);
    }
    let rendering = stroke
        .rendering
        .as_ref()
        .ok_or(InkRasterError::UnsupportedStroke("missing saved settings"))?;
    let style = &rendering.style;
    let props = rendering.properties;
    if rendering.pen_name.as_deref()
        != Some("com.samsung.android.sdk.pen.pen.preload.ColoredPencil")
        || rendering.advanced_settings.as_deref() != Some("4;")
        || rendering.tool_type_raw != 2
        || !props.compressed
        || !props.stylus_channels
        || props.replay_only
        || props.eraser
        || props.fixed_width
        || props.millisecond_timestamps
        || props.top_layer_pen
        || props.alpha_lock
        || props.binary_added
        || props.generated
        || props.fixed_opacity
        || props.rainbow_effect
        || props.straighten
        || props.reveal_mode
        || style.size_level.unwrap_or(0) != 0
        || style.stroke_type_raw.unwrap_or(0) != 0
        || style.color_type_raw.unwrap_or(0) != 0
        || style.line_type_raw.unwrap_or(0) != 0
    {
        return Err(InkRasterError::UnsupportedStroke(
            "normal ColoredPencil GLV4 stylus profile",
        ));
    }
    if rendering.metadata.as_ref().is_some_and(|metadata| {
        metadata.flip_enabled
            || metadata.rotation_degrees.is_some_and(|v| v != 0.)
            || metadata.property_mask.iter().enumerate().any(|(i, &byte)| {
                byte & !match i {
                    0 => 0x7f,
                    1 => 0x13,
                    _ => 0,
                } != 0
            })
            || metadata.field_mask.iter().any(|&byte| byte != 0)
            || !metadata.fixed_trailing_data.is_empty()
            || !metadata.flexible_trailing_data.is_empty()
    }) {
        return Err(InkRasterError::UnsupportedStroke(
            "object transform or extension",
        ));
    }
    let expected = StrokeStyle {
        legacy_pen_name_id: style.legacy_pen_name_id,
        advanced_pen_setting_id: style.advanced_pen_setting_id,
        pen_name_id: style.pen_name_id,
        color_argb: style.color_argb,
        pen_size: style.pen_size,
        size_level: style.size_level,
        stroke_type_raw: style.stroke_type_raw,
        color_type_raw: style.color_type_raw,
        line_type_raw: style.line_type_raw,
        ..StrokeStyle::default()
    };
    if *style != expected {
        return Err(InkRasterError::UnsupportedStroke("saved style extension"));
    }
    let color = style
        .color_argb
        .ok_or(InkRasterError::UnsupportedStroke("missing source ARGB"))?;
    let size = style
        .pen_size
        .ok_or(InkRasterError::UnsupportedStroke("missing source size"))?;
    if !size.is_finite() || stroke.pen_width.to_bits() != size.to_bits() {
        return Err(InkRasterError::InvalidStroke("source size"));
    }
    if stroke.color.is_some_and(|rgb| {
        [rgb.r, rgb.g, rgb.b] != [(color >> 16) as u8, (color >> 8) as u8, color as u8]
    }) {
        return Err(InkRasterError::UnsupportedStroke(
            "public RGB differs from source ARGB",
        ));
    }

    let count = stroke.points.len();
    if count > 4096 {
        return Err(InkRasterError::WorkLimit);
    }
    if count < 2
        || [
            stroke.pressures.len(),
            stroke.tilts.len(),
            stroke.orientations.len(),
            stroke.timestamps.len(),
        ]
        .iter()
        .any(|&n| n != count)
    {
        return Err(InkRasterError::InvalidStroke(
            "ColoredPencil source channels",
        ));
    }
    let finite = |value: f64| -> Result<f32, InkRasterError> {
        let f = value as f32;
        if value.is_finite() && f.is_finite() {
            Ok(f)
        } else {
            Err(InkRasterError::InvalidStroke(
                "nonfinite ColoredPencil channel",
            ))
        }
    };
    for &pressure in &stroke.pressures {
        finite(pressure)?;
    }
    if stroke.timestamps.iter().any(|&t| i32::try_from(t).is_err()) {
        return Err(InkRasterError::InvalidStroke(
            "ColoredPencil native timestamp range",
        ));
    }
    let input = StrokeInput {
        points: stroke
            .points
            .iter()
            .map(|p| {
                Ok(P {
                    x: finite(p.x)?,
                    y: finite(p.y)?,
                })
            })
            .collect::<Result<Vec<_>, InkRasterError>>()?,
        tilts: stroke
            .tilts
            .iter()
            .copied()
            .map(finite)
            .collect::<Result<Vec<_>, _>>()?,
        orientations: stroke
            .orientations
            .iter()
            .copied()
            .map(finite)
            .collect::<Result<Vec<_>, _>>()?,
        size,
        curve_enabled: true,
        tool_type: 2,
    };
    let sampled = sample(
        &input,
        PenData {
            size: size.clamp(0.4, 800.),
            width: pen_data[0] as i32,
            height: pen_data[1] as i32,
            spacing_extra: 0,
        },
    )
    .map_err(|message| {
        if message.contains("sample limit exceeded") {
            InkRasterError::WorkLimit
        } else {
            InkRasterError::InvalidStroke(message)
        }
    })?;
    Ok((sampled.records, color))
}
