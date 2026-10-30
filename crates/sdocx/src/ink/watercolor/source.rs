use super::{DrawableVersion, MAX_INPUT_POINTS, SavedStrokeInput, sample_saved_stroke};
use crate::{InkRasterError, Stroke, StrokeStyle};

pub(crate) fn prepare_stroke(
    stroke: &Stroke,
    screen: [u32; 2],
) -> Result<(Vec<[f32; 6]>, u32), InkRasterError> {
    if screen.iter().any(|&n| n == 0 || n > 16384) {
        return Err(InkRasterError::InvalidContext);
    }
    let rendering = stroke
        .rendering
        .as_ref()
        .ok_or(InkRasterError::UnsupportedStroke("missing saved settings"))?;
    let style = &rendering.style;
    let props = rendering.properties;
    if rendering.pen_name.as_deref()
        != Some("com.samsung.android.sdk.pen.pen.preload.WaterColorBrush")
        || !matches!(rendering.advanced_settings.as_deref(), None | Some(""))
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
            "normal WaterColor GLV1/GLV2 stylus profile",
        ));
    }
    if rendering.metadata.as_ref().is_some_and(|metadata| {
        metadata.flip_enabled
            || metadata.rotation_degrees.is_some_and(|v| v != 0.)
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
    if count > MAX_INPUT_POINTS {
        return Err(InkRasterError::WorkLimit);
    }
    if count < 2
        || stroke.pressures.len() != count
        || stroke.timestamps.len() != count
        || [&stroke.tilts, &stroke.orientations]
            .iter()
            .any(|v| (!v.is_empty() && v.len() != count) || v.iter().any(|n| !n.is_finite()))
    {
        return Err(InkRasterError::InvalidStroke("WaterColor source channels"));
    }
    let version = if rendering.advanced_settings.is_none() {
        DrawableVersion::V1
    } else {
        DrawableVersion::V2
    };
    let input = SavedStrokeInput {
        points: &stroke.points,
        pressures: &stroke.pressures,
        timestamps: &stroke.timestamps,
        pen_size: size,
        color_argb: color,
        color_type: 0,
        stroke_type: 0,
        curve_enabled: true,
        tool_type: 2,
        canvas: screen,
    };
    let rows = sample_saved_stroke(input, version).map_err(|message| {
        if message == "dab output limit exceeded" {
            InkRasterError::WorkLimit
        } else if message == "empty measured WaterColor curve" {
            InkRasterError::UnsupportedStroke(message)
        } else {
            InkRasterError::InvalidStroke(message)
        }
    })?;
    Ok((rows, color))
}
