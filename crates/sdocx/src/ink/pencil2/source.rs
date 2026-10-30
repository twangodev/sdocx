use super::{
    AlphaState, SampledRecord, SamplerError, SavedInput, sample, sample_with_inverse_scale,
};
use crate::{Stroke, StrokeStyle};

pub(crate) fn plan_stroke(stroke: &Stroke, inverse_scale: f32) -> Result<usize, SamplerError> {
    let rendering = stroke
        .rendering
        .as_ref()
        .ok_or(SamplerError::Unsupported("missing saved settings"))?;
    let style = &rendering.style;
    let props = rendering.properties;
    if rendering.pen_name.as_deref() != Some("com.samsung.android.sdk.pen.pen.preload.Pencil2")
        || !matches!(rendering.advanced_settings.as_deref(), Some("" | "14;"))
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
        return Err(SamplerError::Unsupported(
            "fresh V14 source-0 stylus profile",
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
        return Err(SamplerError::Unsupported("object transform or extension"));
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
        return Err(SamplerError::Unsupported("saved style extension"));
    }
    let color = style
        .color_argb
        .ok_or(SamplerError::Unsupported("missing source ARGB"))?;
    let size = style
        .pen_size
        .ok_or(SamplerError::Unsupported("missing source size"))?;
    if !size.is_finite() || stroke.pen_width.to_bits() != size.to_bits() {
        return Err(SamplerError::Invalid("source size"));
    }
    if stroke.color.is_some_and(|rgb| {
        [rgb.r, rgb.g, rgb.b] != [(color >> 16) as u8, (color >> 8) as u8, color as u8]
    }) {
        return Err(SamplerError::Unsupported(
            "public RGB differs from source ARGB",
        ));
    }
    if stroke.points.len() > super::MAX_SOURCE_POINTS {
        return Err(SamplerError::WorkLimit);
    }
    let count = stroke.points.len();
    if count < 2
        || [
            stroke.pressures.len(),
            stroke.tilts.len(),
            stroke.orientations.len(),
            stroke.timestamps.len(),
        ]
        .iter()
        .any(|&length| length != count)
    {
        return Err(SamplerError::Invalid("source channel counts"));
    }
    if stroke.pressures.iter().any(|&pressure| pressure < 0.) {
        return Err(SamplerError::Invalid("negative source pressure"));
    }
    let finite = |v: f64| -> Result<f32, SamplerError> {
        let f = v as f32;
        if v.is_finite() && f.is_finite() {
            Ok(f)
        } else {
            Err(SamplerError::Invalid("nonfinite source channel"))
        }
    };
    if stroke
        .timestamps
        .iter()
        .any(|&value| i32::try_from(value).is_err())
    {
        return Err(SamplerError::Invalid("native timestamp range"));
    }
    // Validate conversions before planning; planning does not allocate source vectors.
    for point in &stroke.points {
        finite(point.x)?;
        finite(point.y)?;
    }
    for values in [&stroke.pressures, &stroke.tilts, &stroke.orientations] {
        for &value in values {
            finite(value)?;
        }
    }
    // Match the sampler's validation before checking its record budget.
    if inverse_scale != 1.0 && inverse_scale != 0.5 {
        return Err(SamplerError::Unsupported("uncaptured inverse scale"));
    }
    if size <= 0.0 {
        return Err(SamplerError::Invalid("nonfinite or nonpositive geometry"));
    }
    super::plan_records(count, |i| super::P {
        x: stroke.points[i].x as f32,
        y: stroke.points[i].y as f32,
    })
}

pub(crate) fn prepare_stroke(
    stroke: &Stroke,
    inverse_scale: f32,
) -> Result<Vec<SampledRecord>, SamplerError> {
    let planned = plan_stroke(stroke, inverse_scale)?;
    let style = &stroke.rendering.as_ref().unwrap().style;
    let color = style.color_argb.unwrap();
    let size = style.pen_size.unwrap();
    let channels = |values: &[f64]| values.iter().map(|&value| value as f32).collect();
    let alpha = AlphaState {
        alpha_byte: (color >> 24) as u8,
        square_pressure_flag: 0,
        base: size.clamp(0.4, 800.),
        size_level: 1,
    };
    let input = SavedInput {
        advanced_version: 14,
        tool_type: 2,
        source: 0,
        stroke_type: 0,
        curve_enabled: true,
        color_argb: color,
        size,
        effective_size: alpha.base,
        points: stroke
            .points
            .iter()
            .map(|p| super::P {
                x: p.x as f32,
                y: p.y as f32,
            })
            .collect(),
        pressure: channels(&stroke.pressures),
        tilt: channels(&stroke.tilts),
        orientation: channels(&stroke.orientations),
        timestamps: stroke.timestamps.clone(),
    };
    let records = if inverse_scale == 1. {
        sample(&input, alpha)?
    } else {
        sample_with_inverse_scale(&input, alpha, inverse_scale)?
    };
    debug_assert_eq!(planned, records.len());
    Ok(records)
}

#[cfg(all(test, feature = "serde"))]
mod serialized_fresh_native_test {
    use super::prepare_stroke;
    use crate::Stroke;
    use sha2::{Digest, Sha256};
    #[test]
    fn remapped_saved_metadata_is_not_admitted_as_fresh_source() {
        let data: serde_json::Value =
            serde_json::from_slice(include_bytes!("assets/terminal-source.json")).unwrap();
        let mut stroke: Stroke =
            serde_json::from_value(data["strokes"][0]["stroke"].clone()).unwrap();
        let rendering = stroke.rendering.as_mut().unwrap();
        rendering.pen_name = Some("com.samsung.android.sdk.pen.pen.preload.Pencil2".into());
        assert_eq!(rendering.style.initial_tolerance, Some(0.47816235));
        assert!(rendering.properties.generated);
        assert!(rendering.properties.millisecond_timestamps);
        assert!(prepare_stroke(&stroke, 1.).is_err());
    }

    #[test]
    fn fresh_zero_tolerance_geometry_matches_native() {
        let mut stroke: Stroke =
            serde_json::from_slice(include_bytes!("assets/fresh-source.json")).unwrap();
        stroke.rendering.as_mut().unwrap().pen_name =
            Some("com.samsung.android.sdk.pen.pen.preload.Pencil2".into());
        {
            let source_rendering = stroke.rendering.as_ref().unwrap();
            let mut style = serde_json::to_value(crate::StrokeStyle::default()).unwrap();
            style["pen_size"] = serde_json::json!(stroke.pen_width);
            style["color_argb"] = serde_json::json!(source_rendering.style.color_argb);
            style["size_level"] = serde_json::json!(0);
            style["stroke_type_raw"] = serde_json::json!(0);
            style["color_type_raw"] = serde_json::json!(0);
            let props = serde_json::json!({"compressed":true,"stylus_channels":true,"replay_only":false,"eraser":false,"fixed_width":false,"millisecond_timestamps":false,"top_layer_pen":false,"alpha_lock":false,"binary_added":false,"generated":false,"fixed_opacity":false,"rainbow_effect":false,"straighten":false,"reveal_mode":false});
            stroke.rendering = Some(serde_json::from_value(serde_json::json!({"metadata":null,"pen_name":"com.samsung.android.sdk.pen.pen.preload.Pencil2","advanced_settings":"14;","tool_type_raw":2,"properties":props,"style":style})).unwrap());
        }
        let mut over_budget = stroke.clone();
        let last = over_budget.points.len() - 1;
        over_budget.points[last].x = 1_000_000.;
        assert!(matches!(
            prepare_stroke(&over_budget, 1.),
            Err(super::SamplerError::WorkLimit)
        ));
        assert!(matches!(
            prepare_stroke(&over_budget, 0.75),
            Err(super::SamplerError::Unsupported("uncaptured inverse scale"))
        ));
        let mut bad_size = over_budget.clone();
        bad_size.pen_width = 0.;
        bad_size.rendering.as_mut().unwrap().style.pen_size = Some(0.);
        assert!(matches!(
            prepare_stroke(&bad_size, 1.),
            Err(super::SamplerError::Invalid(
                "nonfinite or nonpositive geometry"
            ))
        ));
        assert!(matches!(
            prepare_stroke(&bad_size, 0.75),
            Err(super::SamplerError::Unsupported("uncaptured inverse scale"))
        ));
        let mut bad_channel = over_budget.clone();
        bad_channel.pressures[0] = f64::NAN;
        assert!(matches!(
            prepare_stroke(&bad_channel, 0.75),
            Err(super::SamplerError::Invalid("nonfinite source channel"))
        ));
        let mut bad_count = stroke.clone();
        bad_count.pressures.pop();
        let mut missing_settings = stroke.clone();
        missing_settings.rendering = None;
        assert!(matches!(
            crate::ink::pencil2::plan_stroke(&bad_count, 1.0),
            Err(super::SamplerError::Invalid("source channel counts"))
        ));
        assert!(matches!(
            crate::ink::pencil2::plan_stroke(&missing_settings, 1.0),
            Err(super::SamplerError::Unsupported("missing saved settings"))
        ));
        for (input, scale) in [
            (&over_budget, 1.0),
            (&over_budget, 0.75),
            (&bad_size, 1.0),
            (&bad_channel, 0.75),
            (&bad_count, 1.0),
            (&missing_settings, 1.0),
        ] {
            let plan = crate::ink::pencil2::plan_stroke(input, scale).unwrap_err();
            let prepare = prepare_stroke(input, scale).unwrap_err();
            assert_eq!(format!("{plan:?}"), format!("{prepare:?}"));
        }
        let records = prepare_stroke(&stroke, 1.).unwrap();
        assert_eq!(
            crate::ink::pencil2::plan_stroke(&stroke, 1.).unwrap(),
            records.len()
        );
        assert_eq!(
            crate::ink::pencil2::plan_stroke(&stroke, 0.5).unwrap(),
            prepare_stroke(&stroke, 0.5).unwrap().len()
        );
        let bytes: Vec<u8> = records
            .iter()
            .flat_map(|r| r.lanes.iter().flat_map(|f| f.to_le_bytes()))
            .collect();
        assert_eq!(records.len(), 208);
        assert_eq!(
            format!("{:x}", Sha256::digest(&bytes)),
            "de3cf9e4c6a832385e68daf4e99bd0b2c9174a2e576f9567e6c5f5f6d2d286d0"
        );

        // ObjectMetadata decodes property bits 0..=9 and 12, and field bit 0.
        // Raw flip and rotation bits are rejected even when decoded values disagree.
        // Other retained mask bits have no established Pencil2 drawing behavior.
        let original: Stroke =
            serde_json::from_slice(include_bytes!("assets/fresh-source.json")).unwrap();
        let mut metadata = original.rendering.unwrap().metadata.unwrap();
        metadata.flip_enabled = false;
        metadata.rotation_degrees = None;
        metadata.fixed_trailing_data.clear();
        metadata.flexible_trailing_data.clear();
        metadata.property_mask = vec![0x7f, 0x13];
        metadata.field_mask = vec![0];
        let mut guarded = stroke.clone();
        guarded.rendering.as_mut().unwrap().metadata = Some(metadata);
        assert!(prepare_stroke(&guarded, 1.).is_ok());
        for (property, field) in [
            (vec![0x80], vec![]),
            (vec![0, 4], vec![]),
            (vec![0, 0, 1], vec![]),
            (vec![], vec![1]),
            (vec![], vec![2]),
            (vec![], vec![0, 1]),
        ] {
            let metadata = guarded
                .rendering
                .as_mut()
                .unwrap()
                .metadata
                .as_mut()
                .unwrap();
            metadata.property_mask = property;
            metadata.field_mask = field;
            assert!(prepare_stroke(&guarded, 1.).is_err());
        }

        let mut tolerance = stroke.clone();
        tolerance
            .rendering
            .as_mut()
            .unwrap()
            .style
            .initial_tolerance = Some(0.47816235);
        assert!(prepare_stroke(&tolerance, 1.).is_err());
        let mut generated = stroke.clone();
        generated.rendering.as_mut().unwrap().properties.generated = true;
        assert!(prepare_stroke(&generated, 1.).is_err());
        let mut milliseconds = stroke;
        milliseconds
            .rendering
            .as_mut()
            .unwrap()
            .properties
            .millisecond_timestamps = true;
        assert!(prepare_stroke(&milliseconds, 1.).is_err());
    }
}
