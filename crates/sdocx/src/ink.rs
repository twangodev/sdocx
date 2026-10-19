//! Shared, sample-addressable ink geometry for exports and interactive replay.
use crate::{BoundingBox, Stroke, stroke_paint};
use std::borrow::Cow;
#[cfg(test)]
mod airbrush;
#[cfg(test)]
mod brushpen;
pub(crate) mod calligraphy;

#[cfg(test)]
pub(crate) fn calligraphy_saved_native_mesh_for_test() -> Option<[Vec<[f32; 4]>; 3]> {
    calligraphy::saved_native_mesh_bridge()
}

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
pub(crate) mod marker3;
mod marker4;
// Oblique is admitted only for an explicit canonical page-vector context.
mod oblique;
mod oblique_v1;
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

/// Source-event boundaries for an ordered DefaultPen source-over disk stream.
/// Redraw boundaries cover input points 2..N-2; the terminal EndPen suffix is
/// rendered for the final input point. Empty ranges are retained.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct OrderedDiskReplay {
    pub redraw_ends: Vec<usize>,
    pub terminal_start: usize,
    pub terminal_part: usize,
}

/// Render preparation context whose coordinate contract is backed by the
/// Standard PDF ObjectPDFWriter mode-0 matrix witness.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InkRenderContext {
    CanonicalPage,
}

/// Prepare native Oblique V1/V4 geometry only for canonical page export. The
/// public context-free API remains approximate.
pub(crate) fn prepare_stroke_for_context<'a>(
    stroke: &'a Stroke,
    dark_mode: bool,
    context: InkRenderContext,
) -> PreparedStroke<'a> {
    prepare_stroke_inner(stroke, dark_mode, Some(context))
}

/// Reconstruct the typed Oblique V4 geometry for an explicit saved/runtime
/// input. The context-free public `prepare_stroke` API does not call this.
pub(crate) fn prepare_oblique_mesh(
    points: &[crate::Point],
    size: f32,
    curve_enabled: bool,
    tool_type: u32,
    inverse_scale: [f32; 2],
) -> Option<CoverageMesh> {
    // Match the numeric core's defensive MAX_INPUT_POINTS allocation budget.
    if points.len() > 65_536 {
        return None;
    }
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
    /// Ordered native DefaultPen disk instances rendered with source-over.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub ordered_disk_replay: Option<OrderedDiskReplay>,
    /// Native V14 stamp directions, including SmPath's small-vector behavior.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub dot_directions: Option<Vec<crate::Point>>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub rect_stamp: Option<RectStamp>,
    /// Native Oblique V1/V4 coverage mesh when prepared for canonical page
    /// export. Original source samples remain in `points`.
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
    prepare_stroke_inner(stroke, dark_mode, None)
}

fn prepare_stroke_inner(
    stroke: &Stroke,
    dark_mode: bool,
    context: Option<InkRenderContext>,
) -> PreparedStroke<'_> {
    let mut paint = stroke_paint(stroke, dark_mode);
    let defaultpen = context.and_then(|_| defaultpen::prepare(stroke));
    let oblique = context.and_then(|context| prepare_oblique_stroke(stroke, context));
    let mut fountain = fountain::prepare(stroke);
    if fountain.is_some()
        && stroke.color.is_none()
        && stroke
            .rendering
            .as_ref()
            .and_then(|rendering| rendering.style.color_argb)
            .is_none()
    {
        // Native Fountain ObjectStroke defaults an omitted ARGB color to
        // opaque black. Apply the existing canvas dark-mode adaptation.
        paint.color = if dark_mode { "#ffffff" } else { "#000000" }.into();
    }
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
    let (points, sample_ends, dot_radii, ordered_disk_replay, opacity) =
        if let Some(ink) = defaultpen.as_ref() {
            paint.segment_widths = None;
            (
                Cow::Owned(
                    ink.points
                        .iter()
                        .map(|point| crate::Point {
                            x: f64::from(point[0]),
                            y: f64::from(point[1]),
                        })
                        .collect(),
                ),
                None,
                Some(ink.points.iter().map(|point| f64::from(point[2])).collect()),
                Some(OrderedDiskReplay {
                    redraw_ends: ink.redraw_ends.clone(),
                    terminal_start: ink.terminal_start,
                    terminal_part: stroke.points.len(),
                }),
                1.0,
            )
        } else if let Some(ink) = oblique.as_ref() {
            paint.segment_widths = None;
            (
                Cow::Borrowed(stroke.points.as_slice()),
                None,
                None,
                None,
                ink.opacity,
            )
        } else if let Some((points, ends, radii, opacity)) = native {
            paint.segment_widths = None;
            (Cow::Owned(points), Some(ends), Some(radii), None, opacity)
        } else if let Some(ink) = rect {
            paint.segment_widths = None;
            (
                Cow::Owned(ink.points),
                Some(ink.sample_ends),
                None,
                None,
                ink.opacity,
            )
        } else {
            (
                Cow::Borrowed(stroke.points.as_slice()),
                None,
                None,
                None,
                1.,
            )
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
    let (coverage_mesh, oblique_bounds, oblique_color, has_oblique) = match oblique {
        Some(ink) => {
            let bounds = ink.mesh.bounds;
            (Some(ink.mesh), bounds, Some(ink.color), true)
        }
        None => (None, None, None, false),
    };
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
        support: if dot_radii.is_some() || has_oblique {
            InkSupport::Reconstructed
        } else {
            InkSupport::Approximate
        },
        points,
        sample_ends,
        dot_radii,
        ordered_disk_replay,
        dot_directions,
        rect_stamp,
        coverage_mesh,
        segment_widths: paint.segment_widths,
        width: paint.width,
        bounds: oblique_bounds.or(bounds),
        // DefaultPen has no native coverage mesh. Retain the public stroke
        // color/theme resolution used by the ordinary vector renderer.
        color: oblique_color.unwrap_or(paint.color),
        opacity,
        top_layer: stroke
            .rendering
            .as_ref()
            .is_some_and(|r| r.properties.top_layer_pen),
        profile: profile.map(|p| p.name),
    }
}

struct PreparedObliqueInk {
    mesh: CoverageMesh,
    opacity: f64,
    color: String,
}

fn prepare_oblique_stroke(
    stroke: &Stroke,
    context: InkRenderContext,
) -> Option<PreparedObliqueInk> {
    let rendering = stroke.rendering.as_ref()?;
    let name = rendering.pen_name.as_deref()?;
    if name.strip_prefix("com.samsung.android.sdk.pen.pen.preload.") != Some("ObliquePen") {
        return None;
    }
    let version = effective_oblique_version(rendering.advanced_settings.as_deref())?;
    if !matches!(version, 1 | 4)
        || effective_oblique_stroke_type(rendering.style.stroke_type_raw) != 0
        || !matches!(rendering.style.color_type_raw, None | Some(0))
        || rendering.properties.fixed_width
        || rendering.properties.replay_only
        || rendering.properties.eraser
        || rendering.properties.alpha_lock
        || rendering.properties.fixed_opacity
        || rendering.properties.rainbow_effect
        || rendering.properties.straighten
        || rendering.properties.reveal_mode
    {
        return None;
    }
    // ObjectStroke getters return size 0 and opaque black when these style
    // setters are omitted. Drawing passes size through Pen::SetSize, which
    // clamps it to 0.4; the color getter remains opaque black.
    let requested_size = rendering.style.pen_size.unwrap_or(0.0);
    if !requested_size.is_finite() || requested_size < 0.0 {
        return None;
    }
    let size = oblique_native_size(requested_size)?;
    let argb = rendering.style.color_argb.unwrap_or(0xff00_0000);
    let tool_type = u32::from(rendering.tool_type_raw);
    if version == 1
        && (tool_type > 3
            || rendering.properties.top_layer_pen
            || rendering.style.fixed_width.is_some())
    {
        return None;
    }
    let mesh = match (context, version) {
        // Standard ObjectPDFWriter mode 0 passes identity to DrawObjectList;
        // the SVG scene also keeps page coordinates in its unzoomed viewBox.
        (InkRenderContext::CanonicalPage, 4) => prepare_oblique_mesh(
            &stroke.points,
            size,
            rendering.properties.compressed,
            tool_type,
            [1.0, 1.0],
        )?,
        (InkRenderContext::CanonicalPage, 1) => prepare_oblique_v1_mesh(
            &stroke.points,
            size,
            rendering.properties.compressed,
            tool_type,
        )?,
        _ => return None,
    };
    let opacity = f64::from((argb >> 24) as u8) / 255.0;
    Some(PreparedObliqueInk {
        mesh,
        opacity,
        color: format!(
            "#{:02x}{:02x}{:02x}",
            (argb >> 16) & 0xff,
            (argb >> 8) & 0xff,
            argb & 0xff,
        ),
    })
}

fn prepare_oblique_v1_mesh(
    points: &[crate::Point],
    size: f32,
    curve_enabled: bool,
    tool_type: u32,
) -> Option<CoverageMesh> {
    if points.len() > 65_536
        || points
            .iter()
            .any(|point| !point.x.is_finite() || !point.y.is_finite())
    {
        return None;
    }
    let input_points = points
        .iter()
        .map(|point| path::P::from(*point))
        .collect::<Vec<_>>();
    if input_points
        .iter()
        .any(|point| !point.x.is_finite() || !point.y.is_finite())
    {
        return None;
    }
    let callbacks =
        oblique_v1::prepare_selected_v1_stream(&input_points, size, curve_enabled, tool_type)?;
    let runtime = oblique::ObliqueRuntime {
        k: oblique::DEFAULT_K,
        inverse_scale: [1.0, 1.0],
    };
    let native = oblique::prepare_addpoint_stream_with_boundaries(
        &callbacks.pairs,
        size as u32,
        runtime,
        &callbacks.redraw_line_ends,
        callbacks.terminal_start,
    )?;
    if native.triangles.is_empty() {
        return None;
    }
    let triangles = native
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
    let terminal_end = triangles.len();
    Some(CoverageMesh {
        triangles,
        sample_ends: native.sample_ends,
        terminal_start: native.terminal_start,
        terminal_end,
        bounds,
    })
}

/// ObjectStroke defaults missing StrokeType to zero and maps stored values >=3
/// to zero in GetStrokeType. Type zero is the only Drawing GL path that assigns
/// the ISPPenCanvas consumed by Oblique V4's inverse-scale lookup.
fn effective_oblique_stroke_type(raw: Option<u16>) -> u16 {
    let raw = raw.unwrap_or(0);
    if raw >= 3 { 0 } else { raw }
}

/// Model `Pen::SetSize` before V4's `FCVTZU` conversion. The setter clamps
/// finite values to this range; mesh traversal has separate work budgets.
fn oblique_native_size(size: f32) -> Option<f32> {
    size.is_finite().then_some(size.clamp(0.4, 800.0))
}

fn effective_oblique_version(setting: Option<&str>) -> Option<i32> {
    // Drawing applies the saved setting to the ObliquePen instance before
    // selecting its drawable. The plugin starts at V4; the inherited setter
    // resets null to V1, leaves an empty token stream unchanged, and parses
    // only the first nonempty semicolon token with C atoi semantics.
    // ObliquePen::GetStrokeDrawableGL clamps the signed version to 1..=4;
    // Pen::getVersion itself only reads the stored member.
    let Some(setting) = setting else {
        return Some(1);
    };
    let Some(token) = setting.split(';').find(|token| !token.is_empty()) else {
        return Some(4);
    };
    let parsed = native_atoi(token)?;
    if parsed == -1 {
        return Some(4);
    }
    Some(if parsed == 0 { 1 } else { parsed }.clamp(1, 4))
}

fn native_atoi(token: &str) -> Option<i32> {
    let bytes = token.as_bytes();
    let mut index = 0;
    while bytes
        .get(index)
        .is_some_and(|byte| matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c))
    {
        index += 1;
    }
    let number_start = index;
    if bytes
        .get(index)
        .is_some_and(|byte| matches!(byte, b'+' | b'-'))
    {
        index += 1;
    }
    let digits_start = index;
    while bytes.get(index).is_some_and(u8::is_ascii_digit) {
        index += 1;
    }
    if index == digits_start {
        return Some(0);
    }
    token[number_start..index].parse().ok()
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
            style: crate::StrokeStyle {
                color_argb: Some(0x8033_6699),
                color_type_raw: Some(0),
                pen_size: Some(5.0),
                stroke_type_raw: Some(0),
                ..crate::StrokeStyle::default()
            },
        });
        input
    }
    fn defaultpen_stroke() -> Stroke {
        let mut input = stroke(&[(0., 0.), (10., 0.), (20., 0.), (30., 10.)]);
        input.pen_width = 10.0;
        input.color = Some(crate::Color {
            r: 0x33,
            g: 0x66,
            b: 0x99,
        });
        input.rendering = Some(crate::StrokeRendering {
            metadata: None,
            pen_name: Some("com.samsung.android.sdk.pen.pen.preload.DefaultPen".into()),
            advanced_settings: None,
            tool_type_raw: 1,
            properties: crate::StrokeProperties {
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
            style: crate::StrokeStyle {
                color_argb: Some(0x0033_6699),
                color_type_raw: Some(0),
                pen_size: Some(10.0),
                stroke_type_raw: Some(0),
                ..crate::StrokeStyle::default()
            },
        });
        input
    }

    fn fountain_stroke(advanced_settings: &str) -> Stroke {
        let mut input = stroke(&[(0., 0.), (10., 0.), (20., 0.)]);
        input.pressures = vec![0.5; input.points.len()];
        input.timestamps = vec![0, 10, 20];
        input.pen_width = 2.5;
        input.rendering = Some(crate::StrokeRendering {
            metadata: None,
            pen_name: Some(
                "com.samsung.android.sdk.pen.pen.preload.FountainPen".into(),
            ),
            advanced_settings: Some(advanced_settings.into()),
            tool_type_raw: 2,
            properties: crate::StrokeProperties {
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
            style: crate::StrokeStyle {
                pen_size: (advanced_settings != "14;").then_some(2.5),
                initial_tolerance: (advanced_settings == "14;").then_some(0.),
                ..crate::StrokeStyle::default()
            },
        });
        input
    }

    #[test]
    fn defaultpen_redraw_returns_and_full_native_vectors_match_fixture() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../conformance/defaultpen-redraw-boundaries-native.json"
        ))
        .unwrap();
        let case = &fixture["cases"]["six_point_tool1_nonstationary"];
        let input = &case["input"];
        let f32_from_le_hex = |value: &str| {
            let bytes = (0..4)
                .map(|index| u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).unwrap())
                .collect::<Vec<_>>();
            f32::from_bits(u32::from_le_bytes(bytes.try_into().unwrap()))
        };
        let points = input["points_f32_bits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|point| crate::Point {
                x: f32_from_le_hex(point[0].as_str().unwrap()) as f64,
                y: f32_from_le_hex(point[1].as_str().unwrap()) as f64,
            })
            .collect::<Vec<_>>();
        let size = f32_from_le_hex(input["pen_size_f32_bits"].as_str().unwrap());
        let tool = input["tool_type"].as_u64().unwrap() as u32;
        let ink = super::defaultpen::saved_with_boundaries(&points, size, tool, 0).unwrap();

        let phase_rows = |phase: &str| {
            case["native_redraw_callback_boundaries"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|row| row["phase"] == phase)
                .collect::<Vec<_>>()
        };
        let draw_rows = phase_rows("draw_object_stroke");
        let line_deltas = draw_rows
            .iter()
            .filter(|row| row["method"].as_str().unwrap().ends_with("redrawLine"))
            .map(|row| row["callbacks_emitted"].as_u64().unwrap() as usize)
            .collect::<Vec<_>>();
        let expected_ends = line_deltas
            .iter()
            .scan(0usize, |total, count| {
                *total += count;
                Some(*total)
            })
            .collect::<Vec<_>>();
        assert_eq!(line_deltas, [0, 0, 7, 0]);
        assert_eq!(ink.redraw_ends, expected_ends);
        assert_eq!(ink.terminal_start, *expected_ends.last().unwrap());
        let terminal = draw_rows
            .iter()
            .find(|row| row["method"].as_str().unwrap().ends_with("endPen"))
            .unwrap();
        assert_eq!(
            ink.points.len() - ink.terminal_start,
            terminal["callbacks_emitted"].as_u64().unwrap() as usize
        );

        for phase in ["draw_object_stroke", "saved_redraw"] {
            let native = &case["phase_final_vectors"][phase];
            let expected = native["triples_f32_bits"]
                .as_array()
                .unwrap()
                .iter()
                .map(|triple| {
                    std::array::from_fn(|component| {
                        f32_from_le_hex(triple[component].as_str().unwrap()).to_bits()
                    })
                })
                .collect::<Vec<[u32; 3]>>();
            let actual = ink
                .points
                .iter()
                .map(|triple| triple.map(f32::to_bits))
                .collect::<Vec<_>>();
            assert_eq!(actual, expected, "full native {phase} triplets");
        }
        assert_eq!(ink.points.len(), 22);
    }

    #[test]
    fn defaultpen_missing_public_color_keeps_native_geometry_and_theme_paint() {
        let mut stroke = defaultpen_stroke();
        let colored_stroke = stroke.clone();
        let colored =
            prepare_stroke_for_context(&colored_stroke, false, InkRenderContext::CanonicalPage);
        stroke.color = None;
        stroke.rendering.as_mut().unwrap().style.color_argb = None;
        for dark_mode in [false, true] {
            let ordinary_paint = stroke_paint(&stroke, dark_mode);
            let prepared =
                prepare_stroke_for_context(&stroke, dark_mode, InkRenderContext::CanonicalPage);
            assert_eq!(prepared.support, InkSupport::Reconstructed);
            assert_eq!(prepared.points, colored.points);
            assert_eq!(prepared.dot_radii, colored.dot_radii);
            assert_eq!(prepared.ordered_disk_replay, colored.ordered_disk_replay);
            assert_eq!(prepared.color, ordinary_paint.color);
            assert_eq!(prepared.opacity, 1.0);
        }
    }

    #[test]
    fn admitted_fountain_missing_color_uses_native_default_and_theme_adaptation() {
        for version in ["14;", "18;0;100;"] {
            let mut input = fountain_stroke(version);
            assert!(input.color.is_none());
            assert!(input
                .rendering
                .as_ref()
                .unwrap()
                .style
                .color_argb
                .is_none());
            for (dark_mode, expected) in [(false, "#000000"), (true, "#ffffff")] {
                let prepared = prepare_stroke(&input, dark_mode);
                assert_eq!(prepared.support, InkSupport::Reconstructed, "{version}");
                assert_eq!(prepared.color, expected, "{version}, dark={dark_mode}");
                assert_eq!(prepared.opacity, 1.0, "{version}");
            }

            input.color = Some(crate::Color {
                r: 0x33,
                g: 0x66,
                b: 0x99,
            });
            assert_eq!(prepare_stroke(&input, false).color, "#336699", "{version}");
        }
    }

    #[test]
    fn fountain_color_fallback_does_not_change_style_argb_or_unsupported_fallback() {
        let mut explicit = fountain_stroke("18;0;100;");
        explicit.color = Some(crate::Color { r: 3, g: 3, b: 3 });
        explicit
            .rendering
            .as_mut()
            .unwrap()
            .style
            .color_argb = Some(0xff03_0303);
        let prepared = prepare_stroke(&explicit, false);
        assert_eq!(prepared.color, "#030303");
        assert_eq!(prepared.opacity, 1.0);

        let mut unsupported = fountain_stroke("17;0;100;");
        let expected = stroke_paint(&unsupported, false).color;
        let prepared = prepare_stroke(&unsupported, false);
        assert_eq!(prepared.support, InkSupport::Approximate);
        assert_eq!(prepared.color, expected);
        assert_eq!(prepared.color, "#1a1a1a");

        unsupported.rendering = None;
        assert_eq!(prepare_stroke(&unsupported, false).color, "#1a1a1a");
    }

    #[test]
    fn defaultpen_admission_matches_native_default_size_tools_and_clamps() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../conformance/defaultpen-admission-native.json"
        ))
        .unwrap();
        let float = |value: &serde_json::Value| {
            let hex = value.as_str().unwrap();
            f32::from_le_bytes(std::array::from_fn(|index| {
                u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).unwrap()
            }))
        };
        for (name, case) in fixture["cases"].as_object().unwrap() {
            let input = &case["input"];
            let mut stroke = defaultpen_stroke();
            stroke.points = input["points_f32_bits"]
                .as_array()
                .unwrap()
                .iter()
                .map(|point| crate::Point {
                    x: f64::from(float(&point[0])),
                    y: f64::from(float(&point[1])),
                })
                .collect();
            let rendering = stroke.rendering.as_mut().unwrap();
            rendering.tool_type_raw = input["tool_type"].as_u64().unwrap() as u16;
            rendering.style.pen_size = input["size_setter_called"]
                .as_bool()
                .unwrap()
                .then(|| float(&input["pen_size_f32_bits"]));
            let prepared =
                prepare_stroke_for_context(&stroke, false, InkRenderContext::CanonicalPage);
            assert!(prepared.ordered_disk_replay.is_some(), "{name}");
            let radii = prepared.dot_radii.as_ref().unwrap();
            let expected = case["streams"]["draw_object_stroke"]["values_f32_bits"]
                .as_array()
                .unwrap();
            assert_eq!(prepared.points.len() * 3, expected.len(), "{name}");
            for (index, ((point, radius), expected)) in prepared
                .points
                .iter()
                .zip(radii)
                .zip(expected.chunks_exact(3))
                .enumerate()
            {
                for (actual, expected) in [point.x as f32, point.y as f32, *radius as f32]
                    .into_iter()
                    .zip(expected)
                {
                    assert_eq!(
                        actual.to_bits(),
                        float(expected).to_bits(),
                        "{name}: {index}"
                    );
                }
            }
        }
        let mut unsupported = defaultpen_stroke();
        unsupported.rendering.as_mut().unwrap().tool_type_raw = 4;
        assert!(
            prepare_stroke_for_context(&unsupported, false, InkRenderContext::CanonicalPage)
                .ordered_disk_replay
                .is_none()
        );
    }

    #[test]
    fn defaultpen_canonical_profile_keeps_ordered_native_disks_and_ignores_alpha() {
        let input = defaultpen_stroke();
        assert!(prepare_stroke(&input, false).ordered_disk_replay.is_none());
        let prepared = prepare_stroke_for_context(&input, false, InkRenderContext::CanonicalPage);
        assert_eq!(prepared.color, "#336699");
        assert_eq!(prepared.opacity, 1.0);
        assert_eq!(
            prepared.ordered_disk_replay.as_ref().unwrap().redraw_ends,
            vec![0, 0]
        );
        assert_eq!(
            prepared
                .ordered_disk_replay
                .as_ref()
                .unwrap()
                .terminal_start,
            0
        );
        assert_eq!(
            prepared.ordered_disk_replay.as_ref().unwrap().terminal_part,
            4
        );
        assert_eq!(prepared.dot_radii.as_ref().unwrap().len(), 11);
        assert!(
            prepared
                .dot_radii
                .as_ref()
                .unwrap()
                .iter()
                .all(|radius| *radius == 5.5)
        );
        assert_eq!(prepared.support, InkSupport::Reconstructed);

        let mut unsupported = input.clone();
        unsupported
            .rendering
            .as_mut()
            .unwrap()
            .properties
            .top_layer_pen = true;
        assert!(
            prepare_stroke_for_context(&unsupported, false, InkRenderContext::CanonicalPage)
                .ordered_disk_replay
                .is_none()
        );
        let mut unsupported = input.clone();
        unsupported
            .rendering
            .as_mut()
            .unwrap()
            .properties
            .compressed = false;
        assert!(
            prepare_stroke_for_context(&unsupported, false, InkRenderContext::CanonicalPage)
                .ordered_disk_replay
                .is_some()
        );
        for setting in [None, Some(""), Some("4;"), Some("nonsense")] {
            for tool in 1..=3 {
                for curve in [false, true] {
                    let mut accepted = input.clone();
                    let rendering = accepted.rendering.as_mut().unwrap();
                    rendering.advanced_settings = setting.map(str::to_owned);
                    rendering.tool_type_raw = tool;
                    rendering.properties.compressed = curve;
                    assert!(
                        prepare_stroke_for_context(
                            &accepted,
                            false,
                            InkRenderContext::CanonicalPage
                        )
                        .ordered_disk_replay
                        .is_some(),
                        "DefaultPen source profile: setting={setting:?}, tool={tool}, curve={curve}"
                    );
                }
            }
        }
        for (source_size, expected_radius) in [(0.0, 1.5), (0.1, 1.5), (10.0, 5.5), (1000.0, 400.5)]
        {
            let mut source_sized = input.clone();
            source_sized.rendering.as_mut().unwrap().style.pen_size = Some(source_size);
            let prepared =
                prepare_stroke_for_context(&source_sized, false, InkRenderContext::CanonicalPage);
            assert!(prepared.ordered_disk_replay.is_some());
            assert!(
                prepared
                    .dot_radii
                    .as_ref()
                    .unwrap()
                    .iter()
                    .all(|radius| *radius == expected_radius)
            );
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
    #[test]
    fn oblique_canonical_admission_checks_supported_settings_and_context() {
        let input = oblique_stroke();
        let context_free = prepare_stroke(&input, false);
        assert!(context_free.coverage_mesh.is_none());
        assert_eq!(context_free.support, InkSupport::Approximate);
        let ink = prepare_stroke_for_context(&input, false, InkRenderContext::CanonicalPage);
        assert_eq!(ink.profile, Some("ObliquePen"));
        assert!(std::ptr::eq(ink.points.as_ptr(), input.points.as_ptr()));
        assert_eq!(ink.opacity, 128.0 / 255.0);
        assert_eq!(ink.color, "#336699");
        let mesh = ink.coverage_mesh.as_ref().expect("native Oblique mesh");
        assert!(!mesh.triangles.is_empty());
        assert_eq!(mesh.terminal_end, mesh.triangles.len());
        assert!(ink.dot_radii.is_none());
        let same_tool_context = prepare_oblique_mesh(&input.points, 5.0, false, 0, [1.0, 1.0])
            .expect("normalized tool context");
        assert_eq!(mesh, &same_tool_context);
        let mut fixed_width = input.clone();
        let style = &mut fixed_width.rendering.as_mut().unwrap().style;
        style.fixed_width = Some(24.0);
        fixed_width
            .rendering
            .as_mut()
            .unwrap()
            .properties
            .fixed_width = true;
        assert!(
            prepare_stroke_for_context(&fixed_width, false, InkRenderContext::CanonicalPage,)
                .coverage_mesh
                .is_none()
        );
        assert_eq!(
            ink.coverage_mesh,
            prepare_oblique_mesh(&input.points, 5.0, false, 0, [1.0, 1.0])
        );
        let mut missing_size = input.clone();
        missing_size.rendering.as_mut().unwrap().style.pen_size = None;
        let missing_size_ink =
            prepare_stroke_for_context(&missing_size, false, InkRenderContext::CanonicalPage);
        assert_eq!(
            missing_size_ink.coverage_mesh,
            prepare_oblique_mesh(&missing_size.points, 0.4, false, 0, [1.0, 1.0])
        );
        let mut missing_color = input.clone();
        missing_color.rendering.as_mut().unwrap().style.color_argb = None;
        let missing_color_ink =
            prepare_stroke_for_context(&missing_color, false, InkRenderContext::CanonicalPage);
        assert!(missing_color_ink.coverage_mesh.is_some());
        assert_eq!(missing_color_ink.color, "#000000");
        assert_eq!(missing_color_ink.opacity, 1.0);
        let mut absent_color_type = input.clone();
        absent_color_type
            .rendering
            .as_mut()
            .unwrap()
            .style
            .color_type_raw = None;
        assert!(
            prepare_stroke_for_context(&absent_color_type, false, InkRenderContext::CanonicalPage,)
                .coverage_mesh
                .is_some()
        );
        let mut negative_size = input.clone();
        negative_size.rendering.as_mut().unwrap().style.pen_size = Some(-0.1);
        assert!(
            prepare_stroke_for_context(&negative_size, false, InkRenderContext::CanonicalPage,)
                .coverage_mesh
                .is_none()
        );
        let mut nonstandard_color_type = input.clone();
        nonstandard_color_type
            .rendering
            .as_mut()
            .unwrap()
            .style
            .color_type_raw = Some(1);
        assert!(
            prepare_stroke_for_context(
                &nonstandard_color_type,
                false,
                InkRenderContext::CanonicalPage,
            )
            .coverage_mesh
            .is_none()
        );
        let mut absent_stroke_type = input.clone();
        absent_stroke_type
            .rendering
            .as_mut()
            .unwrap()
            .style
            .stroke_type_raw = None;
        assert!(
            prepare_stroke_for_context(
                &absent_stroke_type,
                false,
                InkRenderContext::CanonicalPage,
            )
                .coverage_mesh
                .is_some()
        );
        let mut high_size = input.clone();
        high_size.rendering.as_mut().unwrap().style.pen_size = Some(1_000.0);
        assert_eq!(
            prepare_stroke_for_context(&high_size, false, InkRenderContext::CanonicalPage,)
                .coverage_mesh,
            prepare_oblique_mesh(&high_size.points, 800.0, false, 0, [1.0, 1.0])
        );

        let mut other_version = input.clone();
        other_version.rendering.as_mut().unwrap().advanced_settings = Some("3;".into());
        assert!(
            prepare_stroke_for_context(&other_version, false, InkRenderContext::CanonicalPage,)
                .coverage_mesh
                .is_none()
        );
        let mut missing_version = input.clone();
        missing_version
            .rendering
            .as_mut()
            .unwrap()
            .advanced_settings = None;
        assert!(
            prepare_stroke_for_context(&missing_version, false, InkRenderContext::CanonicalPage,)
                .coverage_mesh
                .is_none()
        );
        let mut empty_version = input.clone();
        empty_version.rendering.as_mut().unwrap().advanced_settings = Some(String::new());
        assert!(
            prepare_stroke_for_context(&empty_version, false, InkRenderContext::CanonicalPage,)
                .coverage_mesh
                .is_some()
        );
        let mut clamped_version = input.clone();
        clamped_version
            .rendering
            .as_mut()
            .unwrap()
            .advanced_settings = Some("9;".into());
        assert!(
            prepare_stroke_for_context(&clamped_version, false, InkRenderContext::CanonicalPage,)
                .coverage_mesh
                .is_some()
        );
        let mut other_route = input.clone();
        other_route
            .rendering
            .as_mut()
            .unwrap()
            .style
            .stroke_type_raw = Some(1);
        assert!(
            prepare_stroke_for_context(&other_route, false, InkRenderContext::CanonicalPage,)
                .coverage_mesh
                .is_none()
        );
        let mut unassigned_gl_canvas = other_route.clone();
        unassigned_gl_canvas
            .rendering
            .as_mut()
            .unwrap()
            .style
            .stroke_type_raw = Some(2);
        assert!(
            prepare_stroke_for_context(
                &unassigned_gl_canvas,
                false,
                InkRenderContext::CanonicalPage,
            )
            .coverage_mesh
            .is_none()
        );
        let mut getter_normalized_type = other_route;
        getter_normalized_type
            .rendering
            .as_mut()
            .unwrap()
            .style
            .stroke_type_raw = Some(3);
        assert!(
            prepare_stroke_for_context(
                &getter_normalized_type,
                false,
                InkRenderContext::CanonicalPage,
            )
            .coverage_mesh
            .is_some()
        );

        let mut v1_tool = input.clone();
        {
            let rendering = v1_tool.rendering.as_mut().unwrap();
            rendering.advanced_settings = None;
            rendering.tool_type_raw = 1;
        }
        assert!(
            prepare_stroke_for_context(&v1_tool, false, InkRenderContext::CanonicalPage,)
                .coverage_mesh
                .is_some(),
            "V1 tool 1 uses the captured 50-unit short-path threshold"
        );
        {
            let rendering = v1_tool.rendering.as_mut().unwrap();
            rendering.tool_type_raw = 2;
            rendering.properties.compressed = false;
        }
        v1_tool.points = vec![
            crate::Point { x: 0.0, y: 0.0 },
            crate::Point { x: 0.5, y: 0.0 },
        ];
        assert!(
            prepare_stroke_for_context(&v1_tool, false, InkRenderContext::CanonicalPage,)
                .coverage_mesh
                .is_some(),
            "short V1 no-curve transitions have captured native support"
        );

        for effect in 0..6 {
            let mut unsupported_effect = input.clone();
            let properties = &mut unsupported_effect.rendering.as_mut().unwrap().properties;
            match effect {
                0 => properties.eraser = true,
                1 => properties.alpha_lock = true,
                2 => properties.fixed_opacity = true,
                3 => properties.rainbow_effect = true,
                4 => properties.straighten = true,
                _ => properties.reveal_mode = true,
            }
            assert!(
                prepare_stroke_for_context(
                    &unsupported_effect,
                    false,
                    InkRenderContext::CanonicalPage,
                )
                .coverage_mesh
                .is_none()
            );
        }
    }

    #[test]
    fn canonical_v1_adapter_matches_selected_native_full_vector() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../conformance/oblique-saved-caller-selected-native.json"
        ))
        .unwrap();
        let case = &fixture["cases"]["three_points_curve"];
        let input = &case["input"];
        let bits = |value: &serde_json::Value| {
            let hex = value.as_str().unwrap().trim_start_matches("0x");
            u32::from_le_bytes(std::array::from_fn(|index| {
                u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).unwrap()
            }))
        };
        let points = input["points_f32_bits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|pair| {
                let pair = pair.as_array().unwrap();
                crate::Point {
                    x: f64::from(f32::from_bits(bits(&pair[0]))),
                    y: f64::from(f32::from_bits(bits(&pair[1]))),
                }
            })
            .collect::<Vec<_>>();
        let size = f32::from_bits(bits(&input["pen_size_f32_bits"]));
        let stroke = Stroke {
            rendering: Some(crate::StrokeRendering {
                metadata: None,
                pen_name: input["pen_name"].as_str().map(str::to_owned),
                advanced_settings: None,
                tool_type_raw: input["tool_type"].as_u64().unwrap() as u16,
                properties: crate::StrokeProperties {
                    compressed: input["curve_enabled"].as_bool().unwrap(),
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
                style: crate::StrokeStyle {
                    color_argb: Some(0xff33_6699),
                    color_type_raw: Some(0),
                    pen_size: Some(size),
                    stroke_type_raw: Some(0),
                    ..crate::StrokeStyle::default()
                },
            }),
            bbox: BoundingBox::default(),
            points,
            pressures: Vec::new(),
            timestamps: Vec::new(),
            tilts: Vec::new(),
            orientations: Vec::new(),
            color: None,
            pen_width: size,
        };
        let prepared = prepare_stroke_for_context(&stroke, false, InkRenderContext::CanonicalPage);
        let mesh = prepared.coverage_mesh.expect("selected V1 canonical mesh");
        assert_eq!(mesh.sample_ends.len(), stroke.points.len() - 1);
        assert_eq!(mesh.terminal_end, mesh.triangles.len());
        let actual = mesh
            .triangles
            .iter()
            .flat_map(|triangle| triangle.vertices)
            .flat_map(|vertex| [vertex.x, vertex.y, vertex.coverage])
            .flat_map(f32::to_le_bytes)
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let expected = case["phase_final_vectors"]["draw_object_stroke"]["vector_bytes"]
            .as_str()
            .unwrap();
        assert_eq!(actual, expected, "full V1 native AddPoint vector");
    }

    #[test]
    fn canonical_v1_tool_adapters_match_native_mesh_and_keep_gates() {
        use sha2::Digest;

        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../conformance/oblique-v1-tool1-native.json"
        ))
        .unwrap();
        for case_name in [
            "tool1_curve_alternating_short_long",
            "tool3_curve_short_long",
            "tool3_linear_same_mesh",
        ] {
            let case = &fixture["cases"][case_name];
            let input = &case["input"];
            let bits = |value: &serde_json::Value| {
                let hex = value.as_str().unwrap().trim_start_matches("0x");
                u32::from_le_bytes(std::array::from_fn(|index| {
                    u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).unwrap()
                }))
            };
            let points = input["points_f32_bits"]
                .as_array()
                .unwrap()
                .iter()
                .map(|pair| {
                    let pair = pair.as_array().unwrap();
                    crate::Point {
                        x: f64::from(f32::from_bits(bits(&pair[0]))),
                        y: f64::from(f32::from_bits(bits(&pair[1]))),
                    }
                })
                .collect::<Vec<_>>();
            let size = f32::from_bits(bits(&input["pen_size_f32_bits"]));
            let mut stroke = oblique_stroke();
            stroke.points = points;
            stroke.pen_width = size;
            let rendering = stroke.rendering.as_mut().unwrap();
            rendering.advanced_settings = None;
            rendering.tool_type_raw = input["tool_type"].as_u64().unwrap() as u16;
            rendering.properties.compressed = input["curve_enabled"].as_bool().unwrap();
            rendering.style.pen_size = Some(size);

            let prepared =
                prepare_stroke_for_context(&stroke, false, InkRenderContext::CanonicalPage);
            assert!(prepare_stroke(&stroke, false).coverage_mesh.is_none());
            let mesh = prepared.coverage_mesh.expect("captured V1 tool mesh");
            assert_eq!(mesh.sample_ends.len(), stroke.points.len() - 1);
            assert_eq!(mesh.terminal_end, mesh.triangles.len());
            let actual: Vec<u8> = mesh
                .triangles
                .iter()
                .flat_map(|triangle| triangle.vertices)
                .flat_map(|vertex| [vertex.x, vertex.y, vertex.coverage])
                .flat_map(f32::to_le_bytes)
                .collect();
            let expected = &case["phase_final_vectors"]["draw_object_stroke"];
            assert_eq!(
                actual.len(),
                expected["raw_byte_count"].as_u64().unwrap() as usize
            );
            assert_eq!(
                format!("{:x}", sha2::Sha256::digest(&actual)),
                expected["sha256"].as_str().unwrap(),
                "complete native mesh: {case_name}"
            );

            let mut unsupported_tool = stroke.clone();
            unsupported_tool.rendering.as_mut().unwrap().tool_type_raw = 4;
            assert!(
                prepare_stroke_for_context(
                    &unsupported_tool,
                    false,
                    InkRenderContext::CanonicalPage
                )
                .coverage_mesh
                .is_none()
            );
            for gate in 0..3 {
                let mut unsupported = stroke.clone();
                let rendering = unsupported.rendering.as_mut().unwrap();
                match gate {
                    0 => rendering.properties.top_layer_pen = true,
                    1 => {
                        rendering.properties.fixed_width = true;
                        rendering.style.fixed_width = Some(size);
                    }
                    _ => rendering.properties.eraser = true,
                }
                assert!(
                    prepare_stroke_for_context(
                        &unsupported,
                        false,
                        InkRenderContext::CanonicalPage
                    )
                    .coverage_mesh
                    .is_none(),
                    "unsupported drawing gate {gate}: {case_name}"
                );
            }
        }
    }

    #[test]
    fn canonical_v1_duplicate_short_linear_input_matches_native_mesh() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../conformance/oblique-v1-edge-native.json"
        ))
        .unwrap();
        let case = &fixture["cases"]["edge_linear_duplicates_sub5"];
        let input = &case["input"];
        let bits = |value: &serde_json::Value| {
            let hex = value.as_str().unwrap().trim_start_matches("0x");
            u32::from_le_bytes(std::array::from_fn(|index| {
                u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).unwrap()
            }))
        };
        let points = input["points_f32_bits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|pair| {
                let pair = pair.as_array().unwrap();
                crate::Point {
                    x: f64::from(f32::from_bits(bits(&pair[0]))),
                    y: f64::from(f32::from_bits(bits(&pair[1]))),
                }
            })
            .collect::<Vec<_>>();
        let size = f32::from_bits(bits(&input["pen_size_f32_bits"]));
        let stroke = Stroke {
            rendering: Some(crate::StrokeRendering {
                metadata: None,
                pen_name: input["pen_name"].as_str().map(str::to_owned),
                advanced_settings: None,
                tool_type_raw: input["tool_type"].as_u64().unwrap() as u16,
                properties: crate::StrokeProperties {
                    compressed: false,
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
                style: crate::StrokeStyle {
                    color_argb: Some(0xff33_6699),
                    color_type_raw: Some(0),
                    pen_size: Some(size),
                    stroke_type_raw: Some(0),
                    ..crate::StrokeStyle::default()
                },
            }),
            bbox: BoundingBox::default(),
            points,
            pressures: Vec::new(),
            timestamps: Vec::new(),
            tilts: Vec::new(),
            orientations: Vec::new(),
            color: None,
            pen_width: size,
        };
        let prepared = prepare_stroke_for_context(&stroke, false, InkRenderContext::CanonicalPage);
        assert!(prepare_stroke(&stroke, false).coverage_mesh.is_none());
        let mesh = prepared
            .coverage_mesh
            .expect("captured short/duplicate V1 mesh");
        assert_eq!(mesh.sample_ends.len(), stroke.points.len() - 1);
        let native_lines: Vec<_> = case["transitions"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|event| {
                event["phase"] == "draw_object_stroke" && event["method"] == "redrawLineNoCurve"
            })
            .collect();
        assert_eq!(native_lines.len(), mesh.sample_ends.len());
        for (end, event) in mesh.sample_ends.iter().zip(native_lines) {
            assert_eq!(
                *end,
                event["vector_count_after_bytes"].as_u64().unwrap() as usize / 36
            );
        }
        let terminal = case["transitions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|event| event["phase"] == "draw_object_stroke" && event["method"] == "endPen")
            .unwrap();
        assert_eq!(
            mesh.terminal_start,
            terminal["vector_count_before_bytes"].as_u64().unwrap() as usize / 36
        );
        assert_eq!(mesh.terminal_end, mesh.triangles.len());
        let actual: Vec<u8> = mesh
            .triangles
            .iter()
            .flat_map(|triangle| triangle.vertices)
            .flat_map(|vertex| [vertex.x, vertex.y, vertex.coverage])
            .flat_map(f32::to_le_bytes)
            .collect();
        use sha2::{Digest, Sha256};
        assert_eq!(
            format!("{:x}", Sha256::digest(&actual)),
            case["phase_final_vectors"]["draw_object_stroke"]["sha256"]
                .as_str()
                .unwrap()
        );
    }

    #[test]
    fn canonical_oblique_native_missing_style_defaults_match_full_vectors() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../conformance/oblique_defaults_native.json"
        ))
        .unwrap();
        let bits = |value: &serde_json::Value| {
            let hex = value.as_str().unwrap().trim_start_matches("0x");
            u32::from_le_bytes(std::array::from_fn(|index| {
                u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).unwrap()
            }))
        };
        for (case_id, case) in fixture["cases"].as_object().unwrap() {
            let input = &case["input"];
            let points = input["points_f32_bits"]
                .as_array()
                .unwrap()
                .iter()
                .map(|pair| {
                    let pair = pair.as_array().unwrap();
                    crate::Point {
                        x: f64::from(f32::from_bits(bits(&pair[0]))),
                        y: f64::from(f32::from_bits(bits(&pair[1]))),
                    }
                })
                .collect::<Vec<_>>();
            let omitted = &input["style_setters_omitted"];
            let raw_size = f32::from_bits(bits(&input["pen_size_f32_bits"]));
            let raw_argb = u32::from_str_radix(
                input["color_argb_u32"]
                    .as_str()
                    .unwrap()
                    .trim_start_matches("0x"),
                16,
            )
            .unwrap();
            let size = (!omitted["pen_size"].as_bool().unwrap()).then_some(raw_size);
            let argb = (!omitted["color_argb"].as_bool().unwrap()).then_some(raw_argb);
            let stroke = Stroke {
                rendering: Some(crate::StrokeRendering {
                    metadata: None,
                    pen_name: input["pen_name"].as_str().map(str::to_owned),
                    advanced_settings: case["native_calls"]["route"]["advanced_setting"]
                        .as_str()
                        .map(str::to_owned),
                    tool_type_raw: input["tool_type"].as_u64().unwrap() as u16,
                    properties: crate::StrokeProperties {
                        compressed: input["curve_enabled"].as_bool().unwrap(),
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
                    style: crate::StrokeStyle {
                        color_argb: argb,
                        color_type_raw: None,
                        pen_size: size,
                        stroke_type_raw: Some(input["stroke_type"].as_u64().unwrap() as u16),
                        ..crate::StrokeStyle::default()
                    },
                }),
                bbox: BoundingBox::default(),
                points,
                pressures: Vec::new(),
                timestamps: Vec::new(),
                tilts: Vec::new(),
                orientations: Vec::new(),
                color: None,
                pen_width: raw_size,
            };
            let prepared =
                prepare_stroke_for_context(&stroke, false, InkRenderContext::CanonicalPage);
            assert!(
                prepare_stroke(&stroke, false).coverage_mesh.is_none(),
                "{case_id}: context-free API remains approximate"
            );
            let mesh = prepared
                .coverage_mesh
                .unwrap_or_else(|| panic!("{case_id}: canonical native defaults rejected"));
            let actual: Vec<u8> = mesh
                .triangles
                .iter()
                .flat_map(|triangle| triangle.vertices)
                .flat_map(|vertex| [vertex.x, vertex.y, vertex.coverage])
                .flat_map(f32::to_le_bytes)
                .collect();
            let expected = case["phase_final_vectors"]["draw_object_stroke"]["vector_bytes"]
                .as_str()
                .unwrap();
            assert_eq!(actual.len(), expected.len() / 2, "{case_id} bytes");
            for (index, byte) in actual.iter().enumerate() {
                assert_eq!(
                    *byte,
                    u8::from_str_radix(&expected[index * 2..index * 2 + 2], 16).unwrap(),
                    "{case_id} byte {index}"
                );
            }
            let probe = &case["native_calls"]["missing_style_defaults_probe"];
            assert_eq!(
                prepared.color,
                format!("#{:06x}", raw_argb & 0x00ff_ffff),
                "{case_id} native ARGB default"
            );
            let final_size = f32::from_bits(bits(&probe["Pen_GetSize_after_Drawing_f32_bits"]));
            assert_eq!(
                final_size,
                if omitted["pen_size"].as_bool().unwrap() {
                    0.4
                } else {
                    raw_size
                },
                "{case_id} native size default/clamp"
            );
            assert_eq!(prepared.opacity, f64::from((raw_argb >> 24) as u8) / 255.0);
        }
    }

    #[test]
    fn canonical_context_adapter_matches_native_advanced4_vectors() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../conformance/oblique-saved-caller-advanced4-native.json"
        ))
        .unwrap();
        let cases = fixture["cases"].as_object().unwrap();
        for (id, case) in cases {
            let source = &case["input"];
            let bits = |value: &serde_json::Value| {
                let hex = value.as_str().unwrap().trim_start_matches("0x");
                u32::from_le_bytes(std::array::from_fn(|index| {
                    u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).unwrap()
                }))
            };
            let points = source["points_f32_bits"]
                .as_array()
                .unwrap()
                .iter()
                .map(|pair| {
                    let pair = pair.as_array().unwrap();
                    crate::Point {
                        x: f64::from(f32::from_bits(bits(&pair[0]))),
                        y: f64::from(f32::from_bits(bits(&pair[1]))),
                    }
                })
                .collect::<Vec<_>>();
            let size = f32::from_bits(bits(&source["pen_size_f32_bits"]));
            // The saved-caller fixture captures V4 geometry state, not color
            // style. Supply ordinary type-0 ARGB style data only to exercise
            // this adapter's explicit native-style admission requirement.
            let stroke = Stroke {
                rendering: Some(crate::StrokeRendering {
                    metadata: None,
                    pen_name: source["pen_name"].as_str().map(str::to_owned),
                    advanced_settings: source["advanced_setting"].as_str().map(str::to_owned),
                    tool_type_raw: source["tool_type"].as_u64().unwrap() as u16,
                    properties: crate::StrokeProperties {
                        compressed: source["curve_enabled"].as_bool().unwrap(),
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
                    style: crate::StrokeStyle {
                        color_argb: Some(0xff33_6699),
                        color_type_raw: Some(0),
                        pen_size: Some(size),
                        stroke_type_raw: source["stroke_type"].as_u64().map(|v| v as u16),
                        ..crate::StrokeStyle::default()
                    },
                }),
                bbox: BoundingBox::default(),
                points,
                pressures: Vec::new(),
                timestamps: Vec::new(),
                tilts: Vec::new(),
                orientations: Vec::new(),
                color: None,
                pen_width: size,
            };
            let prepared =
                prepare_stroke_for_context(&stroke, false, InkRenderContext::CanonicalPage);
            let mesh = prepared
                .coverage_mesh
                .as_ref()
                .unwrap_or_else(|| panic!("{id}: canonical context rejected native V4 case"));
            let actual = mesh
                .triangles
                .iter()
                .flat_map(|triangle| triangle.vertices)
                .flat_map(|vertex| [vertex.x, vertex.y, vertex.coverage])
                .flat_map(f32::to_le_bytes)
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            let expected = case["phase_final_vectors"]["draw_object_stroke"]["vector_bytes"]
                .as_str()
                .unwrap();
            assert_eq!(
                actual, expected,
                "{id}: context adapter vs native callbacks"
            );
        }
    }

    #[test]
    fn oblique_size_applies_native_set_size_clamp_before_conversion() {
        assert_eq!(oblique_native_size(0.1), Some(0.4_f32));
        assert_eq!(oblique_native_size(0.4), Some(0.4_f32));
        assert_eq!(oblique_native_size(1_000.0), Some(800.0_f32));
        assert_eq!(oblique_native_size(800.0), Some(800.0_f32));
        assert_eq!(oblique_native_size(-0.1), Some(0.4_f32));
        assert_eq!(oblique_native_size(f32::NAN), None);
        assert_eq!(oblique_native_size(f32::INFINITY), None);
    }

    #[test]
    fn oblique_advanced_setting_matches_native_drawable_version_selection() {
        // Executed through Drawing with a synthetic ObjectStroke: absent,
        // empty, signed, malformed, and boundary-token cases all report the
        // selected GL drawable vtable before saved redraw.
        for (setting, expected) in [
            (None, 1),
            (Some(""), 4),
            (Some(";;;"), 4),
            (Some(";4;"), 4),
            (Some("0;"), 1),
            (Some("1;"), 1),
            (Some("2;"), 2),
            (Some("3;"), 3),
            (Some("4;"), 4),
            (Some("9;"), 4),
            (Some("-1;"), 4),
            (Some("-2;"), 1),
            (Some(" 4;"), 4),
            (Some("4 ;"), 4),
            (Some("+4;"), 4),
            (Some("   "), 1),
            (Some("oops"), 1),
            (Some("4;tail"), 4),
        ] {
            assert_eq!(
                effective_oblique_version(setting),
                Some(expected),
                "{setting:?}"
            );
        }
        assert_eq!(effective_oblique_version(Some("2147483648;")), None);
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
