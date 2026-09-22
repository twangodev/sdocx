//! Shared, sample-addressable ink geometry for exports and interactive replay.
use crate::{BoundingBox, Stroke, stroke_paint};
use std::borrow::Cow;
mod fountain;
mod marker2;
mod marker4;
mod path;

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
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub rect_stamp: Option<RectStamp>,
    pub segment_widths: Option<Vec<f64>>,
    pub width: f64,
    pub bounds: Option<BoundingBox>,
    pub color: String,
    pub opacity: f64,
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
    let native = fountain::prepare(stroke)
        .map(|ink| (ink.points, ink.sample_ends, ink.radii, 1.))
        .or_else(|| {
            marker2::prepare(stroke)
                .map(|ink| (ink.points, ink.sample_ends, ink.radii, ink.opacity))
        });
    let rect = marker4::prepare(stroke);
    let rect_stamp = rect.as_ref().map(|ink| ink.stamp);
    let (points, sample_ends, dot_radii, opacity) =
        if let Some((points, ends, radii, opacity)) = native {
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
        support: if dot_radii.is_some() {
            InkSupport::Reconstructed
        } else {
            InkSupport::Approximate
        },
        points,
        sample_ends,
        dot_radii,
        rect_stamp,
        segment_widths: paint.segment_widths,
        width: paint.width,
        bounds,
        color: paint.color,
        opacity,
        profile: profile.map(|p| p.name),
    }
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
