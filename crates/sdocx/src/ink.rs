//! Shared, sample-addressable ink geometry for exports and interactive replay.
use crate::{BoundingBox, Stroke, stroke_paint};
use std::borrow::Cow;
#[cfg(test)]
mod airbrush;
#[cfg(test)]
mod brushpen;
#[cfg(test)]
mod calligraphy;
#[cfg(test)]
mod defaultpen;
mod fountain;
mod fountain_v14;
// Native numeric regression only until caller context and RTV1 paint transport
// are integrated; production InkPen rendering remains explicitly approximate.
#[cfg(test)]
mod inkpen;
#[cfg(test)]
mod inkpen2;
mod marker2;
#[cfg(test)]
mod marker3;
mod marker4;
#[cfg(test)]
mod oblique;
mod path;
#[cfg(test)]
mod pencil;

/// A profile describes evidence, not just whether a pen name is recognized.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct PenProfile {
    pub name: &'static str,
    pub library: &'static str,
    pub bundled: bool,
}

/// Native registry from Samsung Notes 4.4.45.37, including missing libraries.
pub const PEN_PROFILES: &[PenProfile] = &[
    pen("DefaultPen", "DefaultPen", true),
    pen("AirBrushPen", "AirBrushPen", true),
    pen("Beautify", "Beautify", true),
    pen("Brush", "SimpleBrush", true),
    pen("BrushPen", "BrushPen", true),
    pen("ChineseBrush", "ChineseBrush", true),
    pen("Crayon", "Crayon", false),
    pen("Crayon2", "Crayon2", true),
    pen("Eraser", "Eraser", true),
    pen("FadedPen", "FadedPen", false),
    pen("FountainPen", "FountainPen", true),
    pen("InkPen", "InkPen", true),
    pen("InkPen2", "InkPen2", true),
    pen("MagicPen", "MagicPen", true),
    pen("Marker", "Marker", true),
    pen("Marker2", "Marker2", true),
    pen("Marker3", "Marker3", true),
    pen("Marker4", "Marker4", true),
    pen("MontblancCalligraphyPen", "MontblancCalligraphyPen", true),
    pen("MontblancFountainPen", "MontblancFountainPen", true),
    pen("MosaicPen", "MosaicPen", false),
    pen("ObliquePen", "ObliquePen", true),
    pen("OilBrush3", "OilBrush3", true),
    pen("Pencil", "Pencil", true),
    pen("Pencil2", "Pencil2", true),
    pen("Pencil3", "Pencil3", true),
    pen("SelectPen", "SelectPen", true),
    pen("SelectPen2", "SelectPen2", true),
    pen("Smudge", "Smudge", true),
    pen("ColoredPencil", "ColoredPencil", true),
    pen("WaterColorBrush", "WaterColorBrush", true),
    pen("StraightHighlighter", "Marker4", true),
    pen("StraightMarker", "Marker3", true),
    pen("LaserPen", "LaserPen", true),
    pen("GlowPen", "GlowPen", false),
    pen("PatternImagePen", "PatternImagePen", false),
    pen("StraightInkPen2", "InkPen2", true),
    pen("StraightGlowPen", "GlowPen", false),
    pen("BlurPen", "BlurPen", false),
    pen("StraightMosaicPen", "MosaicPen", false),
    pen("StraightBlurPen", "BlurPen", false),
    pen("MosaicPen2", "MosaicPen2", false),
    pen("StraightMosaicPen2", "MosaicPen2", false),
    pen("TapePen", "TapePen", true),
];

const fn pen(name: &'static str, library: &'static str, bundled: bool) -> PenProfile {
    PenProfile {
        name,
        library,
        bundled,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize),
    serde(rename_all = "snake_case")
)]
#[non_exhaustive]
pub enum InkSupport {
    /// Preserved legacy pressure approximation; no native parity claim.
    Approximate,
    /// Native geometry reconstructed and checked against the APK; backend
    /// antialiasing still differs from Samsung's GPU shader.
    Reconstructed,
}

/// Rounded rectangular stamp shared by SVG and Canvas. Angle is in radians.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct RectStamp {
    pub width: f64,
    pub height: f64,
    pub angle: f64,
}

/// One vertex in a native triangle coverage mesh. Coverage is a linear field
/// value in `[0, 1]`, retained separately from the source stroke samples.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct CoverageVertex {
    pub x: f32,
    pub y: f32,
    pub coverage: f32,
}

/// A triangle in the prepared vector coverage field.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct CoverageTriangle {
    pub vertices: [CoverageVertex; 3],
}

/// Prepared triangle coverage and its relationship to saved input samples.
/// `sample_ends` contains exclusive triangle counts after each saved redraw
/// call, corresponding to source samples 1..N. The terminal output remains a
/// distinct interval and render adapters attribute it to the final sample for
/// stepwise replay.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct CoverageMesh {
    pub triangles: Vec<CoverageTriangle>,
    pub sample_ends: Vec<usize>,
    pub terminal_start: usize,
    pub terminal_end: usize,
    pub bounds: Option<BoundingBox>,
}

/// Prepare Oblique's captured saved-redraw mesh for an explicit native input
/// context. The saved curve mode and runtime inverse scale are separate
/// inputs because they come from Drawing and Canvas state, not point samples.
#[cfg(test)]
pub(crate) fn prepare_oblique_mesh(
    points: &[crate::Point],
    size: f32,
    curve_enabled: bool,
    tool_type: u32,
    inverse_scale: [f32; 2],
) -> Option<CoverageMesh> {
    let input_points = points
        .iter()
        .map(|p| path::P {
            x: p.x as f32,
            y: p.y as f32,
        })
        .collect::<Vec<_>>();
    let prepared = oblique::prepare(oblique::ObliqueInput::new(
        &input_points,
        size,
        curve_enabled,
        if tool_type <= 4 { tool_type } else { 0 },
        oblique::ObliqueRuntime {
            k: oblique::DEFAULT_K,
            inverse_scale,
        },
    ))?;
    let triangles = prepared
        .triangles
        .into_iter()
        .map(|triangle| CoverageTriangle {
            vertices: triangle.0.map(|vertex| CoverageVertex {
                x: vertex.position.x,
                y: vertex.position.y,
                coverage: vertex.coverage,
            }),
        })
        .collect::<Vec<_>>();
    let mut bounds: Option<BoundingBox> = None;
    for vertex in triangles.iter().flat_map(|triangle| triangle.vertices) {
        let next = BoundingBox {
            x_min: f64::from(vertex.x),
            y_min: f64::from(vertex.y),
            x_max: f64::from(vertex.x),
            y_max: f64::from(vertex.y),
        };
        if let Some(old) = &mut bounds {
            old.x_min = old.x_min.min(next.x_min);
            old.y_min = old.y_min.min(next.y_min);
            old.x_max = old.x_max.max(next.x_max);
            old.y_max = old.y_max.max(next.y_max);
        } else {
            bounds = Some(next);
        }
    }
    Some(CoverageMesh {
        triangles,
        sample_ends: prepared.sample_ends,
        terminal_start: prepared.terminal.start,
        terminal_end: prepared.terminal.end,
        bounds,
    })
}

/// Geometry and paint shared by SVG and Canvas adapters. The compatibility
/// profile references original samples; it never changes the stored channels.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct PreparedStroke<'a> {
    /// Original positions. Replay already carries this array in `stroke.points`.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "original_samples"))]
    pub points: Cow<'a, [crate::Point]>,
    /// Exclusive prepared-point counts at each original sample, when resampled.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub sample_ends: Option<Vec<usize>>,
    /// Native circular stamps; one radius per prepared point.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub dot_radii: Option<Vec<f64>>,
    /// Native V14 stamp directions, including SmPath's small-vector behavior.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub dot_directions: Option<Vec<crate::Point>>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub rect_stamp: Option<RectStamp>,
    /// Native Oblique V4 coverage mesh when prepared with an explicit runtime
    /// contract. Original source samples remain in `points`.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub coverage_mesh: Option<CoverageMesh>,
    pub segment_widths: Option<Vec<f64>>,
    pub width: f64,
    pub bounds: Option<BoundingBox>,
    pub color: String,
    pub opacity: f64,
    /// Composite this stroke in the highlighter batch after ordinary ink.
    pub top_layer: bool,
    pub profile: Option<&'static str>,
    pub support: InkSupport,
}

#[cfg(feature = "serde")]
#[allow(clippy::ptr_arg)] // Serde predicate must distinguish borrowed from generated storage.
fn original_samples(points: &Cow<'_, [crate::Point]>) -> bool {
    matches!(points, Cow::Borrowed(_))
}

/// Prepare a verified native geometry profile when its saved inputs match.
/// Other profiles retain the explicit pressure approximation.
pub fn prepare_stroke(stroke: &Stroke, dark_mode: bool) -> PreparedStroke<'_> {
    let mut paint = stroke_paint(stroke, dark_mode);
    #[cfg(test)]
    let oblique = prepare_oblique_stroke(stroke);
    #[cfg(not(test))]
    let oblique: Option<(CoverageMesh, f64)> = None;
    let mut fountain = fountain::prepare(stroke);
    let dot_directions = fountain.as_mut().and_then(|ink| ink.directions.take());
    let native = fountain
        .map(|ink| {
            let opacity = stroke
                .rendering
                .as_ref()
                .and_then(|r| r.style.color_argb)
                .map_or(1., |argb| ((argb >> 24) as f32 / 255.) as f64);
            (ink.points, ink.sample_ends, ink.radii, opacity)
        })
        .or_else(|| {
            marker2::prepare(stroke)
                .map(|ink| (ink.points, ink.sample_ends, ink.radii, ink.opacity))
        });
    let rect = marker4::prepare(stroke);
    let rect_stamp = rect.as_ref().map(|ink| ink.stamp);
    let (points, sample_ends, dot_radii, opacity) = if let Some((_, opacity)) = oblique.as_ref() {
        paint.segment_widths = None;
        (
            Cow::Borrowed(stroke.points.as_slice()),
            None,
            None,
            *opacity,
        )
    } else if let Some((points, ends, radii, opacity)) = native {
        paint.segment_widths = None;
        (Cow::Owned(points), Some(ends), Some(radii), opacity)
    } else if let Some(ink) = rect {
        paint.segment_widths = None;
        (
            Cow::Owned(ink.points),
            Some(ink.sample_ends),
            None,
            ink.opacity,
        )
    } else {
        (Cow::Borrowed(stroke.points.as_slice()), None, None, 1.)
    };
    let profile = stroke
        .rendering
        .as_ref()
        .and_then(|r| r.pen_name.as_deref())
        .and_then(|name| {
            name.strip_prefix("com.samsung.android.sdk.pen.pen.preload.")
                .or_else(|| (name == "Eraser").then_some("Eraser"))
        })
        .and_then(|name| PEN_PROFILES.iter().find(|p| p.name == name));
    let coverage_mesh = oblique.as_ref().map(|(mesh, _)| mesh.clone());
    let mut bounds: Option<BoundingBox> = None;
    for (index, point) in points.iter().enumerate() {
        // A vertex belongs to its incoming and outgoing segment. Include both
        // widths so rapidly changing pressure cannot clip the larger cap.
        let radius = dot_radii.as_ref().map_or_else(
            || {
                paint.segment_widths.as_ref().map_or(paint.width, |widths| {
                    widths
                        .get(index.saturating_sub(1))
                        .copied()
                        .unwrap_or(paint.width)
                        .max(widths.get(index).copied().unwrap_or(0.0))
                }) / 2.0
            },
            |radii| radii[index],
        );
        if !point.x.is_finite() || !point.y.is_finite() {
            continue;
        }
        let (rx, ry) = rect_stamp.map_or((radius, radius), |s| {
            let (sin, cos) = s.angle.sin_cos();
            (
                (cos.abs() * s.width + sin.abs() * s.height) / 2.,
                (sin.abs() * s.width + cos.abs() * s.height) / 2.,
            )
        });
        let next = BoundingBox {
            x_min: point.x - rx,
            y_min: point.y - ry,
            x_max: point.x + rx,
            y_max: point.y + ry,
        };
        if let Some(old) = &mut bounds {
            old.x_min = old.x_min.min(next.x_min);
            old.y_min = old.y_min.min(next.y_min);
            old.x_max = old.x_max.max(next.x_max);
            old.y_max = old.y_max.max(next.y_max);
        } else {
            bounds = Some(next);
        }
    }
    PreparedStroke {
        support: if dot_radii.is_some() || oblique.is_some() {
            InkSupport::Reconstructed
        } else {
            InkSupport::Approximate
        },
        points,
        sample_ends,
        dot_radii,
        dot_directions,
        rect_stamp,
        coverage_mesh,
        segment_widths: paint.segment_widths,
        width: paint.width,
        bounds: oblique
            .as_ref()
            .and_then(|(mesh, _)| mesh.bounds)
            .or(bounds),
        color: paint.color,
        opacity,
        top_layer: stroke
            .rendering
            .as_ref()
            .is_some_and(|r| r.properties.top_layer_pen),
        profile: profile.map(|p| p.name),
    }
}

#[cfg(test)]
fn prepare_oblique_stroke(stroke: &Stroke) -> Option<(CoverageMesh, f64)> {
    let rendering = stroke.rendering.as_ref()?;
    let name = rendering.pen_name.as_deref()?;
    if name.strip_prefix("com.samsung.android.sdk.pen.pen.preload.") != Some("ObliquePen") {
        return None;
    }
    if effective_oblique_version(rendering.advanced_settings.as_deref()) != 4
        || rendering.style.stroke_type_raw == Some(1)
    {
        return None;
    }
    let size = (stroke.pen_width as f32).clamp(0.4, 800.0);
    let tool_type = u32::from(rendering.tool_type_raw);
    let mesh = prepare_oblique_mesh(
        &stroke.points,
        size,
        rendering.properties.compressed,
        tool_type,
        [1.0, 1.0],
    )?;
    let opacity = rendering
        .style
        .color_argb
        .map_or(1.0, |argb| f64::from((argb >> 24) as u8) / 255.0);
    Some((mesh, opacity))
}

#[cfg(test)]
fn effective_oblique_version(setting: Option<&str>) -> i32 {
    let Some(setting) = setting else { return 4 };
    let token = setting.split(';').next().unwrap_or("").trim_start();
    if token.is_empty() {
        return 4;
    }
    let bytes = token.as_bytes();
    let digits_start = usize::from(bytes.first().is_some_and(|b| *b == b'+' || *b == b'-'));
    let digits_end = token[digits_start..]
        .bytes()
        .take_while(u8::is_ascii_digit)
        .count()
        + digits_start;
    let parsed = if digits_end > digits_start {
        token[..digits_end].parse::<i32>().unwrap_or(0)
    } else {
        0
    };
    let current = if parsed == -1 {
        4
    } else if parsed == 0 {
        1
    } else {
        parsed
    };
    current.clamp(1, 4)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Point;
    fn stroke(points: &[(f64, f64)]) -> Stroke {
        Stroke {
            rendering: None,
            bbox: BoundingBox::default(),
            points: points.iter().map(|&(x, y)| Point { x, y }).collect(),
            pressures: vec![],
            timestamps: vec![],
            tilts: vec![],
            orientations: vec![],
            color: None,
            pen_width: 2.5,
        }
    }
    fn oblique_stroke() -> Stroke {
        let mut input = stroke(&[(10., 12.), (22., 15.), (38., 9.)]);
        input.pen_width = 5.0;
        input.rendering = Some(crate::StrokeRendering {
            metadata: None,
            pen_name: Some("com.samsung.android.sdk.pen.pen.preload.ObliquePen".into()),
            advanced_settings: Some("4;".into()),
            tool_type_raw: 8,
            properties: crate::StrokeProperties {
                compressed: false,
                replay_only: false,
                stylus_channels: false,
                eraser: false,
                fixed_width: true,
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
            style: crate::StrokeStyle {
                color_argb: Some(0x8033_6699),
                fixed_width: Some(24.0),
                stroke_type_raw: Some(2),
                ..crate::StrokeStyle::default()
            },
        });
        input
    }
    #[test]
    fn shared_geometry_borrows_samples_and_bounds_both_sides_of_width_changes() {
        let mut input = stroke(&[(0., 0.), (5., 0.), (5., 4.)]);
        input.pressures = vec![0.2, 0.8, 1.0];
        let ink = prepare_stroke(&input, false);
        assert!(std::ptr::eq(ink.points.as_ptr(), input.points.as_ptr()));
        let widths = ink.segment_widths.as_ref().unwrap();
        let bounds = ink.bounds.unwrap();
        assert_eq!(bounds.x_min, -widths[0] / 2.0);
        assert_eq!(bounds.x_max, 5.0 + widths[1] / 2.0);
        assert_eq!(bounds.y_max, 4.0 + widths[1] / 2.0);
    }
    #[test]
    fn empty_and_single_sample_strokes_have_explicit_bounds() {
        assert!(prepare_stroke(&stroke(&[]), false).bounds.is_none());
        assert_eq!(
            prepare_stroke(&stroke(&[(2., 3.)]), false)
                .bounds
                .unwrap()
                .x_min,
            1.5
        );
    }
    #[test]
    fn oblique_admission_uses_saved_curve_bit_and_keeps_fixed_width_out_of_mesh() {
        let input = oblique_stroke();
        let ink = prepare_stroke(&input, false);
        assert_eq!(ink.profile, Some("ObliquePen"));
        assert!(std::ptr::eq(ink.points.as_ptr(), input.points.as_ptr()));
        assert_eq!(ink.opacity, 128.0 / 255.0);
        let mesh = ink.coverage_mesh.as_ref().expect("native Oblique mesh");
        assert!(!mesh.triangles.is_empty());
        assert_eq!(mesh.terminal_end, mesh.triangles.len());
        assert!(ink.dot_radii.is_none());
        let same_tool_context = prepare_oblique_mesh(&input.points, 5.0, false, 0, [1.0, 1.0])
            .expect("normalized tool context");
        assert_eq!(mesh, &same_tool_context);
        let mut no_fixed_width = input.clone();
        let style = &mut no_fixed_width.rendering.as_mut().unwrap().style;
        style.fixed_width = None;
        no_fixed_width
            .rendering
            .as_mut()
            .unwrap()
            .properties
            .fixed_width = false;
        assert_eq!(
            ink.coverage_mesh,
            prepare_stroke(&no_fixed_width, false).coverage_mesh
        );
        let mut absent_stroke_type = input.clone();
        absent_stroke_type
            .rendering
            .as_mut()
            .unwrap()
            .style
            .stroke_type_raw = None;
        assert!(prepare_stroke(&absent_stroke_type, false).coverage_mesh.is_some());
        let mut high_size = input.clone();
        high_size.pen_width = 1_000.0;
        assert_eq!(
            prepare_stroke(&high_size, false).coverage_mesh,
            prepare_oblique_mesh(&high_size.points, 800.0, false, 0, [1.0, 1.0])
        );

        let mut other_version = input.clone();
        other_version.rendering.as_mut().unwrap().advanced_settings = Some("3;".into());
        assert!(
            prepare_stroke(&other_version, false)
                .coverage_mesh
                .is_none()
        );
        let mut clamped_version = input.clone();
        clamped_version.rendering.as_mut().unwrap().advanced_settings = Some("9;".into());
        assert!(prepare_stroke(&clamped_version, false).coverage_mesh.is_some());
        let mut other_route = input;
        other_route
            .rendering
            .as_mut()
            .unwrap()
            .style
            .stroke_type_raw = Some(1);
        assert!(prepare_stroke(&other_route, false).coverage_mesh.is_none());
    }

    #[test]
    fn oblique_advanced_setting_uses_effective_native_version() {
        assert_eq!(effective_oblique_version(None), 4);
        assert_eq!(effective_oblique_version(Some("")), 4);
        assert_eq!(effective_oblique_version(Some("-1;")), 4);
        assert_eq!(effective_oblique_version(Some("0;")), 1);
        assert_eq!(effective_oblique_version(Some("2;")), 2);
        assert_eq!(effective_oblique_version(Some("9;")), 4);
    }
    #[cfg(feature = "serde")]
    #[test]
    fn wire_geometry_omits_borrowed_samples_but_retains_generated_points_and_mapping() {
        let input = stroke(&[(0., 0.), (1., 1.)]);
        let mut ink = prepare_stroke(&input, false);
        let original = serde_json::to_value(&ink).unwrap();
        assert!(original.get("points").is_none());
        assert!(original.get("sample_ends").is_none());
        ink.points = Cow::Owned(vec![
            Point { x: 0., y: 0. },
            Point { x: 0.5, y: 0.25 },
            Point { x: 1., y: 1. },
        ]);
        ink.sample_ends = Some(vec![1, 3]);
        let generated = serde_json::to_value(&ink).unwrap();
        assert_eq!(generated["points"].as_array().unwrap().len(), 3);
        assert_eq!(generated["sample_ends"], serde_json::json!([1, 3]));
        assert_eq!(input.points.len(), 2);
    }

    #[test]
    fn registry_distinguishes_aliases_and_missing_plugins() {
        assert_eq!(PEN_PROFILES.len(), 44);
        let libraries: std::collections::BTreeSet<_> = PEN_PROFILES
            .iter()
            .filter(|p| p.bundled)
            .map(|p| p.library)
            .collect();
        assert_eq!(libraries.len(), 30);
        assert!(
            PEN_PROFILES
                .iter()
                .any(|p| p.name == "StraightHighlighter" && p.library == "Marker4")
        );
        assert!(
            PEN_PROFILES
                .iter()
                .any(|p| p.name == "Crayon" && !p.bundled)
        );
    }
}
