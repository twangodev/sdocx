//! Bounded source preparation for the selector-confirmed Pencil3 V3 path.

use super::pencil::{
    SavedStroke, V3PrivateRandom, saved_redraw_v3_particles_with_source_ends_bounded,
};
use crate::{BoundingBox, Color, Stroke};

const MAX_SOURCE_POINTS: usize = 65_536;
const MAX_OUTPUT_RECORDS: usize = 2_048;

pub(crate) struct PreparedPencil3V3 {
    /// RTV1 packed order: center x, center y, half-size, texture row, alpha.
    pub(crate) records: Vec<[f32; 5]>,
    /// Exclusive cumulative record end after each saved source point has
    /// been processed. The vector has one entry per source point; repeated
    /// ends represent points that emitted no particles, and the final end is
    /// the complete record count.
    pub(crate) sample_ends: Vec<usize>,
    pub(crate) bounds: BoundingBox,
    pub(crate) color: Color,
    /// Saved ARGB alpha converted to the renderer's 0..=1 range.
    pub(crate) opacity: f32,
}

// Shared source admission stops before screen-dependent sampling and output budgets.
// In particular, this keeps unsupported style/effect combinations on the
// existing generic path even when their stored alpha is zero.
fn saved_pencil3_v3_inputs(stroke: &Stroke) -> Option<(u32, f32)> {
    let rendering = stroke.rendering.as_ref()?;
    if rendering.pen_name.as_deref() != Some("com.samsung.android.sdk.pen.pen.preload.Pencil3")
        || !matches!(
            rendering.advanced_settings.as_deref(),
            Some("") | Some("4;")
        )
        || !matches!(rendering.tool_type_raw, 1..=3)
        || rendering.properties.replay_only
        || rendering.properties.eraser
        || rendering.properties.fixed_width
        || rendering.properties.top_layer_pen
        || rendering.properties.alpha_lock
        || rendering.properties.fixed_opacity
        || rendering.properties.rainbow_effect
        || rendering.properties.straighten
        || rendering.properties.reveal_mode
        || rendering.properties.millisecond_timestamps
        || !rendering.properties.binary_added
        || rendering.properties.generated
        || rendering.style.stroke_type_raw.unwrap_or(0) != 0
        || rendering.style.color_type_raw.unwrap_or(0) != 0
        || !matches!(rendering.style.line_type_raw, None | Some(0))
        || rendering.style.first_unparsed_field.is_some()
        || !rendering.style.trailing_data.is_empty()
        || rendering.style.field_4_raw.is_some()
        || rendering.style.legacy_partial_rectangle_data.is_some()
        || rendering.style.fixed_width.is_some()
        || rendering.style.size_level.is_some()
        || rendering.style.particle_density.is_some()
        || rendering.style.rendering_level.is_some()
        || rendering.style.original_width.is_some()
        || rendering
            .style
            .initial_tolerance
            .is_some_and(|value| value != 0.0)
        || rendering.style.dash_offset.is_some()
        || rendering.style.pen_repeat_distance.is_some()
        || rendering.style.particle_size.is_some()
        || rendering.style.pattern_index.is_some()
        || rendering.style.pattern_scale.is_some()
        || rendering.style.particle_level.is_some()
        || rendering.style.rainbow_distance.is_some()
        || rendering.style.rainbow_offset.is_some()
        || rendering.style.gradient_colors_argb.is_some()
        || rendering.style.color_argb.is_none()
        || stroke.points.len() < 2
        || stroke.points.len() > MAX_SOURCE_POINTS
        || stroke.points.len() != stroke.pressures.len()
        || stroke.points.len() != stroke.timestamps.len()
    {
        return None;
    }

    // ObjectStroke uses null optional-axis pointers when the saved stylus
    // channel flag is clear; present all-zero arrays remain distinct.
    let (tilts, orientations) = if rendering.properties.stylus_channels {
        if stroke.tilts.len() != stroke.points.len()
            || stroke.orientations.len() != stroke.points.len()
        {
            return None;
        }
        (Some(&stroke.tilts), Some(&stroke.orientations))
    } else {
        if !stroke.tilts.is_empty() || !stroke.orientations.is_empty() {
            return None;
        }
        (None, None)
    };

    let argb = rendering.style.color_argb?;
    // Model omission reads as zero; Drawing calls Pen::SetSize, whose native
    // implementation clamps to [0.4, 800] before Pencil3 constructs its
    // drawable. Stroke::pen_width is only a compatibility fallback when the
    // saved style omits size, so it must not replace that native zero.
    let raw_size = rendering.style.pen_size.unwrap_or(0.0);
    if !raw_size.is_finite()
        || raw_size < 0.0
        || rendering.style.pen_size.is_some()
            && (!stroke.pen_width.is_finite() || stroke.pen_width.to_bits() != raw_size.to_bits())
        || stroke.points.iter().any(|point| {
            !point.x.is_finite()
                || !point.y.is_finite()
                || !(point.x as f32).is_finite()
                || !(point.y as f32).is_finite()
        })
        || stroke
            .pressures
            .iter()
            .any(|value| !value.is_finite() || !(*value as f32).is_finite() || *value < 0.0)
        || stroke
            .timestamps
            .iter()
            .any(|value| i32::try_from(*value).is_err())
        || tilts.is_some_and(|values| {
            values
                .iter()
                .any(|value| !value.is_finite() || !(*value as f32).is_finite())
        })
        || orientations.is_some_and(|values| {
            values
                .iter()
                .any(|value| !value.is_finite() || !(*value as f32).is_finite())
        })
    {
        return None;
    }
    Some((argb, raw_size))
}

/// Whether a supported saved Pencil3 V3 stroke has completely transparent paint.
/// Partial alpha cannot be applied to generic whole-stroke geometry: native V3
/// composites its particles individually.
pub(crate) fn saved_pencil3_v3_transparent(stroke: &Stroke) -> bool {
    saved_pencil3_v3_inputs(stroke).is_some_and(|(argb, _)| argb >> 24 == 0)
}

/// Prepare an ordinary Pencil3 saved stroke for its V3 source path.
///
/// The caller supplies the native screen profile because it is part of the
/// drawing context. Captures at 1920x1080 and 1280x720 produced identical
/// source coordinates and packed records; dimensions are therefore validated
/// here but do not rescale document-space points.
pub(crate) fn prepare_pencil3_v3_source(
    stroke: &Stroke,
    screen_width: u32,
    screen_height: u32,
) -> Option<PreparedPencil3V3> {
    if screen_width == 0
        || screen_height == 0
        || screen_width > i32::MAX as u32
        || screen_height > i32::MAX as u32
    {
        return None;
    }
    let (argb, raw_size) = saved_pencil3_v3_inputs(stroke)?;
    let rendering = stroke.rendering.as_ref()?;
    let (tilts, orientations) = if rendering.properties.stylus_channels {
        (Some(&stroke.tilts), Some(&stroke.orientations))
    } else {
        (None, None)
    };
    let size = raw_size.clamp(0.4, 800.0);

    let source = SavedStroke {
        points: stroke
            .points
            .iter()
            .map(|point| [point.x as f32, point.y as f32])
            .collect(),
        pressures: stroke.pressures.iter().map(|value| *value as f32).collect(),
        timestamps: stroke
            .timestamps
            .iter()
            .map(|value| *value as i32)
            .collect(),
        tilts: tilts.map(|values| values.iter().map(|value| *value as f32).collect()),
        orientations: orientations.map(|values| values.iter().map(|value| *value as f32).collect()),
        tool_type: i32::from(rendering.tool_type_raw),
        size,
    };
    source.validate().ok()?;

    let mut rng = V3PrivateRandom::new();
    let (particles, sample_ends) =
        saved_redraw_v3_particles_with_source_ends_bounded(&source, &mut rng, MAX_OUTPUT_RECORDS)
            .ok()?;
    if particles.is_empty() {
        return None;
    }
    let mut bounds: Option<BoundingBox> = None;
    let mut records = Vec::with_capacity(particles.len());
    for particle in particles {
        let record = particle.attributes;
        let [x, y, half_size, _, alpha] = record;
        if !x.is_finite()
            || !y.is_finite()
            || !half_size.is_finite()
            || half_size < 0.0
            || !alpha.is_finite()
        {
            return None;
        }
        let next = BoundingBox {
            x_min: f64::from(x - half_size),
            y_min: f64::from(y - half_size),
            x_max: f64::from(x + half_size),
            y_max: f64::from(y + half_size),
        };
        if !next.x_min.is_finite()
            || !next.y_min.is_finite()
            || !next.x_max.is_finite()
            || !next.y_max.is_finite()
        {
            return None;
        }
        if let Some(current) = &mut bounds {
            current.x_min = current.x_min.min(next.x_min);
            current.y_min = current.y_min.min(next.y_min);
            current.x_max = current.x_max.max(next.x_max);
            current.y_max = current.y_max.max(next.y_max);
        } else {
            bounds = Some(next);
        }
        records.push(record);
    }
    Some(PreparedPencil3V3 {
        records,
        sample_ends,
        bounds: bounds?,
        color: Color {
            r: (argb >> 16) as u8,
            g: (argb >> 8) as u8,
            b: argb as u8,
        },
        opacity: ((argb >> 24) as u8) as f32 / 255.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Point, StrokeProperties, StrokeRendering, StrokeStyle};
    use serde_json::Value;
    use sha2::{Digest, Sha256};

    fn fixture_stroke(profile: &Value) -> Stroke {
        let points = profile["points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|point| Point {
                x: point[0].as_f64().unwrap(),
                y: point[1].as_f64().unwrap(),
            })
            .collect::<Vec<_>>();
        let values = |name: &str| {
            profile[name]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_f64().unwrap())
                .collect::<Vec<_>>()
        };
        // The capture supplies channels and tool type; saved properties, style,
        // and selector settings below are synthetic adapter inputs.
        let properties = StrokeProperties {
            compressed: true,
            replay_only: false,
            stylus_channels: true,
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
        };
        let rendering = StrokeRendering {
            metadata: None,
            pen_name: Some("com.samsung.android.sdk.pen.pen.preload.Pencil3".into()),
            advanced_settings: Some("4;".into()),
            tool_type_raw: profile["tool_type"].as_u64().unwrap() as u16,
            properties,
            style: StrokeStyle {
                color_argb: Some(0xff33_6699),
                pen_size: Some(profile["size"].as_f64().unwrap() as f32),
                ..StrokeStyle::default()
            },
        };
        let timestamps = profile["timestamps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_i64().unwrap())
            .collect();
        Stroke {
            rendering: Some(rendering),
            bbox: BoundingBox::default(),
            points,
            pressures: values("pressure"),
            timestamps,
            tilts: values("tilt"),
            orientations: values("orientation"),
            color: None,
            pen_width: profile["size"].as_f64().unwrap() as f32,
        }
    }

    fn little_word(value: &Value) -> u32 {
        let value = value.as_str().unwrap();
        u32::from_le_bytes(std::array::from_fn(|index| {
            u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).unwrap()
        }))
    }

    #[test]
    fn adapter_matches_complete_native_packed_streams_and_screen_contexts() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/pencil3-sampler-inputs-native.json"
        ))
        .unwrap();
        for (name, case) in fixture["cases"].as_object().unwrap() {
            let profile = &case["input_profile"];
            if profile["tool_type"] != 2 && profile["tool_type"] != 3 {
                continue;
            }
            let prepared = prepare_pencil3_v3_source(
                &fixture_stroke(profile),
                profile["width"].as_u64().unwrap() as u32,
                profile["height"].as_u64().unwrap() as u32,
            )
            .unwrap_or_else(|| panic!("{name} rejected"));
            let actual = prepared
                .records
                .iter()
                .flat_map(|record| record.iter().map(|value| value.to_bits()))
                .collect::<Vec<_>>();
            let native = case["routes"]["v3"]["phases"]["draw_object_stroke"]
                ["native_packed_stream_f32_bits_le"]
                .as_array()
                .unwrap()
                .iter()
                .map(little_word)
                .collect::<Vec<_>>();
            assert_eq!(actual, native, "{name} full packed stream");
            assert!(prepared.bounds.x_min.is_finite() && prepared.bounds.x_max.is_finite());
        }
    }

    #[test]
    fn curve_disabled_adapter_matches_native_controls() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/pencil3-sampler-inputs-native.json"
        ))
        .unwrap();
        let receipt: Value = serde_json::from_str(include_str!(
            "../../../../conformance/pencil3-no-curve-native.json"
        ))
        .unwrap();
        let tool1 = &fixture["cases"]["tool1_straight_4pt"];
        assert_eq!(tool1["input_profile"]["curve_enabled"], false);
        let mut stroke = fixture_stroke(&tool1["input_profile"]);
        stroke.rendering.as_mut().unwrap().properties.compressed = false;
        let prepared = prepare_pencil3_v3_source(&stroke, 1920, 1080).unwrap();
        let actual = prepared
            .records
            .iter()
            .flatten()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>();
        let native = tool1["routes"]["v3"]["phases"]["draw_object_stroke"]
            ["native_packed_stream_f32_bits_le"].as_array().unwrap()
            .iter().map(little_word).collect::<Vec<_>>();
        assert_eq!(
            actual, native,
            "tool1 curve-disabled complete native stream"
        );

        for name in ["baseline_tool2_curve", "tool3_curve_4pt"] {
            let case = &fixture["cases"][name];
            let mut stroke = fixture_stroke(&case["input_profile"]);
            stroke.rendering.as_mut().unwrap().properties.compressed = false;
            let prepared = prepare_pencil3_v3_source(&stroke, 1920, 1080).unwrap();
            let bytes = prepared
                .records
                .iter()
                .flatten()
                .flat_map(|value| value.to_le_bytes())
                .collect::<Vec<_>>();
            assert_eq!(
                format!("{:x}", Sha256::digest(&bytes)),
                receipt["cases"][name]["stream_sha256"].as_str().unwrap(),
                "{name}"
            );
        }
    }

    #[test]
    fn tool1_curve_adapter_matches_complete_native_stream() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/pencil3-v3-tool1-curve-native.json"
        ))
        .unwrap();
        let profile = &fixture["input_profile"];
        assert_eq!(profile["curve_enabled"], true);
        let prepared = prepare_pencil3_v3_source(&fixture_stroke(profile), 1920, 1080)
            .expect("native curve-enabled tool 1 profile");
        let actual = prepared
            .records
            .iter()
            .flat_map(|record| record.iter().map(|value| value.to_bits()))
            .collect::<Vec<_>>();
        let native = fixture["v3"]["native_packed_stream_f32_bits_le"]
            .as_array()
            .unwrap()
            .iter()
            .map(little_word)
            .collect::<Vec<_>>();
        assert_eq!(actual, native, "complete native packed stream");
    }

    #[test]
    fn source_event_ends_match_native_v3_boundaries_without_changing_streams() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/pencil3-sampler-inputs-native.json"
        ))
        .unwrap();
        let baseline_profile = &fixture["cases"]["baseline_tool2_curve"]["input_profile"];
        let baseline = prepare_pencil3_v3_source(&fixture_stroke(baseline_profile), 1920, 1080)
            .expect("baseline native profile");
        assert_eq!(baseline.sample_ends, [7, 13, 25, 48]);
        let baseline_bytes = baseline
            .records
            .iter()
            .flat_map(|record| record.iter().flat_map(|value| value.to_le_bytes()))
            .collect::<Vec<_>>();
        assert_eq!(
            format!("{:x}", Sha256::digest(&baseline_bytes)),
            "09c1d78531504e2537a0f4c572b7cd38d3b2fa8df10d063f11bac82a11e10a52",
            "baseline complete native packed stream"
        );

        let mut skipped = fixture_stroke(baseline_profile);
        skipped.points = [[0.0, 0.0], [12.0, 3.0], [12.02, 3.01], [31.0, 9.0]]
            .into_iter()
            .map(|[x, y]| Point { x, y })
            .collect();
        skipped.pressures = [0.2, 0.9, 0.35, 0.8].into_iter().collect();
        skipped.timestamps = [10, 20, 21, 40].into_iter().collect();
        skipped.tilts = [0.1, 0.72, 0.24, 0.61].into_iter().collect();
        skipped.orientations = [0.0, 0.3, 0.1, 0.7].into_iter().collect();
        let skipped =
            prepare_pencil3_v3_source(&skipped, 1920, 1080).expect("native tiny-segment profile");
        assert_eq!(skipped.sample_ends, [7, 14, 14, 47]);
        let skipped_bytes = skipped
            .records
            .iter()
            .flat_map(|record| record.iter().flat_map(|value| value.to_le_bytes()))
            .collect::<Vec<_>>();
        assert_eq!(
            format!("{:x}", Sha256::digest(&skipped_bytes)),
            "ad7e23bc2b7c3d559fb44b90feea02f770ad492fbe0c6e5ccd67ad5c0702292a",
            "tiny-segment complete native packed stream"
        );
    }

    #[test]
    fn adapter_rejects_other_selector_routes_and_incomplete_source_channels() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/pencil3-sampler-inputs-native.json"
        ))
        .unwrap();
        let profile = &fixture["cases"]["baseline_tool2_curve"]["input_profile"];
        let mut stroke = fixture_stroke(profile);
        for setting in [None, Some("0;"), Some("1;"), Some("2;"), Some("3;")] {
            stroke.rendering.as_mut().unwrap().advanced_settings = setting.map(str::to_owned);
            assert!(prepare_pencil3_v3_source(&stroke, 1920, 1080).is_none());
        }
        stroke.rendering.as_mut().unwrap().advanced_settings = Some("4;".into());
        for tool in [0, 4] {
            stroke.rendering.as_mut().unwrap().tool_type_raw = tool;
            assert!(prepare_pencil3_v3_source(&stroke, 1920, 1080).is_none());
        }
        stroke.rendering.as_mut().unwrap().tool_type_raw = 2;
        stroke.tilts.pop();
        assert!(prepare_pencil3_v3_source(&stroke, 1920, 1080).is_none());
        stroke.tilts.push(0.0);
        stroke.pressures[1] = f64::NAN;
        assert!(prepare_pencil3_v3_source(&stroke, 1920, 1080).is_none());
    }

    #[test]
    fn tool3_transparency_uses_the_same_source_admission() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/pencil3-sampler-inputs-native.json"
        ))
        .unwrap();
        let mut stroke = fixture_stroke(&fixture["cases"]["tool3_curve_4pt"]["input_profile"]);
        stroke.rendering.as_mut().unwrap().style.color_argb = Some(0x0033_6699);
        assert!(saved_pencil3_v3_transparent(&stroke));
        assert!(prepare_pencil3_v3_source(&stroke, 1920, 1080).is_some());
        stroke.rendering.as_mut().unwrap().properties.compressed = false;
        assert!(saved_pencil3_v3_transparent(&stroke));
        assert!(prepare_pencil3_v3_source(&stroke, 1920, 1080).is_some());
    }

    #[test]
    fn adapter_rejects_unverified_flags_styles_and_width_mismatches() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/pencil3-sampler-inputs-native.json"
        ))
        .unwrap();
        let profile = &fixture["cases"]["baseline_tool2_curve"]["input_profile"];
        let original = fixture_stroke(profile);

        let mut stroke = original.clone();
        stroke
            .rendering
            .as_mut()
            .unwrap()
            .properties
            .millisecond_timestamps = true;
        assert!(prepare_pencil3_v3_source(&stroke, 1920, 1080).is_none());

        let mut stroke = original.clone();
        stroke.rendering.as_mut().unwrap().properties.generated = true;
        assert!(prepare_pencil3_v3_source(&stroke, 1920, 1080).is_none());

        let mut stroke = original.clone();
        stroke.rendering.as_mut().unwrap().properties.binary_added = false;
        assert!(prepare_pencil3_v3_source(&stroke, 1920, 1080).is_none());

        let mut stroke = original.clone();
        stroke.rendering.as_mut().unwrap().style.line_type_raw = Some(1);
        assert!(prepare_pencil3_v3_source(&stroke, 1920, 1080).is_none());

        let mut stroke = original.clone();
        stroke.rendering.as_mut().unwrap().style.initial_tolerance = Some(0.5);
        assert!(prepare_pencil3_v3_source(&stroke, 1920, 1080).is_none());

        let mut stroke = original.clone();
        stroke.rendering.as_mut().unwrap().style.particle_size = Some(1.0);
        assert!(prepare_pencil3_v3_source(&stroke, 1920, 1080).is_none());

        let mut stroke = original.clone();
        stroke.rendering.as_mut().unwrap().style.color_argb = None;
        assert!(prepare_pencil3_v3_source(&stroke, 1920, 1080).is_none());

        let mut stroke = original;
        stroke.pen_width += 1.0;
        assert!(prepare_pencil3_v3_source(&stroke, 1920, 1080).is_none());
    }

    #[test]
    fn adapter_stops_at_the_exact_particle_budget_for_dense_sources() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/pencil3-sampler-inputs-native.json"
        ))
        .unwrap();
        let profile = &fixture["cases"]["baseline_tool2_curve"]["input_profile"];
        let mut stroke = fixture_stroke(profile);
        stroke.points = vec![Point { x: 0.0, y: 0.0 }, Point { x: 100.0, y: 0.0 }];
        stroke.pressures = vec![0.5; 2];
        stroke.timestamps = vec![0; 2];
        stroke.tilts = vec![0.0; 2];
        stroke.orientations = vec![0.0; 2];
        stroke.pen_width = 0.1;
        let rendering = stroke.rendering.as_mut().unwrap();
        rendering.style.pen_size = Some(0.1);
        assert!(prepare_pencil3_v3_source(&stroke, 1920, 1080).is_none());
    }

    #[test]
    fn native_model_size_defaults_and_pen_setsize_clamps_match_full_streams() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/pencil3-sampler-inputs-native.json"
        ))
        .unwrap();
        let profile = &fixture["cases"]["baseline_tool2_curve"]["input_profile"];
        let cases = [
            (
                None,
                1_048,
                "e597079876c3b10ab3def4cf395d8db944ba565d744c33cf5f682f09f04108a4",
            ),
            (
                Some(0.1),
                1_048,
                "e597079876c3b10ab3def4cf395d8db944ba565d744c33cf5f682f09f04108a4",
            ),
            (
                Some(0.0),
                1_048,
                "e597079876c3b10ab3def4cf395d8db944ba565d744c33cf5f682f09f04108a4",
            ),
            (
                Some(800.0),
                7,
                "9708ceaafaec1d2ebc85f8b2d2f5d3dcd232599f65c7f5cedbbeafb0077aff49",
            ),
            (
                Some(900.0),
                7,
                "9708ceaafaec1d2ebc85f8b2d2f5d3dcd232599f65c7f5cedbbeafb0077aff49",
            ),
            (
                Some(10.0),
                48,
                "09c1d78531504e2537a0f4c572b7cd38d3b2fa8df10d063f11bac82a11e10a52",
            ),
        ];
        for (raw_size, expected_count, expected_hash) in cases {
            let mut stroke = fixture_stroke(profile);
            let rendering = stroke.rendering.as_mut().unwrap();
            rendering.style.pen_size = raw_size;
            stroke.pen_width = raw_size.unwrap_or(0.8);
            let prepared = prepare_pencil3_v3_source(&stroke, 2560, 1440).unwrap();
            let bytes = prepared
                .records
                .iter()
                .flat_map(|record| record.iter().flat_map(|value| value.to_le_bytes()))
                .collect::<Vec<_>>();
            assert_eq!(
                prepared.records.len(),
                expected_count,
                "raw size {raw_size:?}"
            );
            assert_eq!(
                format!("{:x}", Sha256::digest(&bytes)),
                expected_hash,
                "raw size {raw_size:?} complete native stream"
            );
        }

        let mut zero_tolerance = fixture_stroke(profile);
        zero_tolerance
            .rendering
            .as_mut()
            .unwrap()
            .style
            .initial_tolerance = Some(0.0);
        assert!(prepare_pencil3_v3_source(&zero_tolerance, 1920, 1080).is_some());
    }
}
