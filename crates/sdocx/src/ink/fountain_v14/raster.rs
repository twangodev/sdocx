//! Explicit native direct or fresh enhanced-AA Fountain RTV4 host paint.
use crate::ink::host_half::{half_add, half_mul, half_rne as h};
use crate::ink::host_raster::{
    ClipVertex, ClippedTriangle as Triangle, clip_triangle, triangle_contains as inside,
};
use crate::ink::host_rgba8::{over, quantize, sample_unorm8_half_rtz};
use crate::{InkRasterContext, InkRasterError, InkRasterOrigin, Stroke};
use smallvec::SmallVec;

struct Stamp {
    triangles: SmallVec<[Triangle; 2]>,
    bounds: [usize; 4],
    alpha: f32,
    inner: f32,
    width_alpha: f32,
}

pub(crate) struct Plan {
    stamps: Vec<Stamp>,
    frame: [usize; 2],
    color: [f32; 4],
    direct: bool,
    work: usize,
}

/// Caller-owned snapshot of Fountain V14 source geometry at one output scale.
///
/// Recreate this snapshot when the source or output scale changes, including
/// source color, size, or object rectangle mutations. A changed paint origin can reuse it. The caller
/// supplies the incoming direction and decides when to reuse the snapshot. Update
/// the caller's direction from `outgoing_direction` only when generating a new
/// snapshot; painting a cached snapshot does not advance it. A new native Drawing
/// has its own direction and cache lifecycle; the caller must reset both. This
/// does not infer
/// native object identity or mutation invalidation. Scales
/// 1 and 2, at most 4096 source samples and 8192 generated stamps, and the
/// canvas work budget are host bounds. These are not native limits.
pub struct FountainV14Prepared {
    dots: super::super::fountain::Dots,
    scale: u32,
    outgoing_direction: crate::Point,
}

/// Caller-managed snapshots and direction for one observed Drawing lifecycle.
///
/// `object` identifies a caller-owned object; `revision` must change whenever
/// its source, color, size or rectangle changes. Scale changes regenerate the
/// snapshot automatically. A cache hit does not advance direction. Reset this
/// state for a new Drawing. These caller-defined keys are not native cache keys.
/// At most 256 distinct objects are retained; this is a host resource bound.
pub struct FountainV14Scene {
    direction: crate::Point,
    entries: Vec<SceneEntry>,
}

struct SceneEntry {
    object: u64,
    revision: u64,
    prepared: FountainV14Prepared,
}

impl Default for FountainV14Scene {
    fn default() -> Self {
        Self {
            direction: crate::Point { x: 0., y: 1. },
            entries: Vec::new(),
        }
    }
}

impl FountainV14Scene {
    /// Start a new Drawing lifecycle, clearing snapshots and direction.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Reuse or generate a snapshot. Failed generation leaves the scene intact.
    /// Successful generation commits the snapshot and direction immediately;
    /// a later paint failure does not roll either back.
    pub fn prepare(
        &mut self,
        object: u64,
        revision: u64,
        stroke: &Stroke,
        context: InkRasterContext,
    ) -> Result<&FountainV14Prepared, InkRasterError> {
        context.pixel_count()?;
        let index = self.entries.iter().position(|entry| entry.object == object);
        if let Some(i) = index {
            if self.entries[i].revision == revision
                && self.entries[i].prepared.scale == context.pixels_per_unit
            {
                return Ok(&self.entries[i].prepared);
            }
        } else if self.entries.len() >= 256 {
            return Err(InkRasterError::WorkLimit);
        }
        if index.is_none() {
            self.entries
                .try_reserve(1)
                .map_err(|_| InkRasterError::Allocation)?;
        }
        let prepared = FountainV14Prepared::new(stroke, context, self.direction)?;
        let direction = prepared.outgoing_direction();
        let entry = SceneEntry {
            object,
            revision,
            prepared,
        };
        let i = match index {
            Some(i) => {
                self.entries[i] = entry;
                i
            }
            None => {
                self.entries.push(entry);
                self.entries.len() - 1
            }
        };
        self.direction = direction;
        Ok(&self.entries[i].prepared)
    }
}

impl FountainV14Prepared {
    /// Generate a bounded snapshot. A single-stamp source uses `incoming_direction`.
    pub fn new(
        stroke: &Stroke,
        context: InkRasterContext,
        incoming_direction: crate::Point,
    ) -> Result<Self, InkRasterError> {
        if !incoming_direction.x.is_finite()
            || !incoming_direction.y.is_finite()
            || incoming_direction.x.abs() > 1.
            || incoming_direction.y.abs() > 1.
        {
            return Err(InkRasterError::InvalidStroke(
                "Fountain V14 incoming direction",
            ));
        }
        if [
            stroke.points.len(),
            stroke.pressures.len(),
            stroke.timestamps.len(),
            stroke.tilts.len(),
            stroke.orientations.len(),
        ]
        .into_iter()
        .any(|len| len > 4096)
        {
            return Err(InkRasterError::WorkLimit);
        }
        validate_source(stroke, context)?;
        let mut dots = super::super::fountain::prepare_v14_with_output_scale(
            stroke,
            context.pixels_per_unit as f32,
        )
        .ok_or(InkRasterError::UnsupportedStroke(
            "saved Fountain V14 source",
        ))?;
        let directions = dots
            .directions
            .as_mut()
            .ok_or(InkRasterError::UnsupportedStroke("Fountain V14 directions"))?;
        if dots.points.len() == 1 {
            directions[0] = incoming_direction;
        }
        let outgoing_direction = *directions.last().ok_or(InkRasterError::UnsupportedStroke(
            "empty Fountain V14 source",
        ))?;
        if dots.points.len() > 8192 {
            return Err(InkRasterError::WorkLimit);
        }
        Ok(Self {
            dots,
            scale: context.pixels_per_unit,
            outgoing_direction,
        })
    }

    /// Direction produced by generating this snapshot, for a subsequent source.
    pub fn outgoing_direction(&self) -> crate::Point {
        self.outgoing_direction
    }
}

pub(crate) fn prepare_snapshot(
    snapshot: &FountainV14Prepared,
    context: InkRasterContext,
    source_origin: [i32; 2],
    paint_argb: u32,
    budget: usize,
) -> Result<Plan, InkRasterError> {
    if context.pixels_per_unit != snapshot.scale {
        return Err(InkRasterError::InvalidContext);
    }
    let frame = RasterFrame::new(context, source_origin)?;
    // Snapshot construction admitted this immutable geometry at the same scale.
    prepare_dots(&snapshot.dots, frame, paint_argb, true, budget)
}

pub(crate) fn prepare(
    stroke: &Stroke,
    context: InkRasterContext,
    source_origin: [i32; 2],
    paint_argb: u32,
    direct: bool,
    budget: usize,
) -> Result<Plan, InkRasterError> {
    let frame = RasterFrame::new(context, source_origin)?;
    validate_source(stroke, context)?;
    let dots = super::super::fountain::prepare_v14_with_output_scale(
        stroke,
        context.pixels_per_unit as f32,
    )
    .ok_or(InkRasterError::UnsupportedStroke(
        "saved Fountain V14 source",
    ))?;
    prepare_dots(&dots, frame, paint_argb, direct, budget)
}

fn validate_source(stroke: &Stroke, context: InkRasterContext) -> Result<(), InkRasterError> {
    context.pixel_count()?;
    let settings = stroke
        .rendering
        .as_ref()
        .ok_or(InkRasterError::UnsupportedStroke(
            "missing Fountain settings",
        ))?;
    let p = settings.properties;
    let style = &settings.style;
    let expected_style = crate::StrokeStyle {
        legacy_pen_name_id: style.legacy_pen_name_id,
        advanced_pen_setting_id: style.advanced_pen_setting_id,
        pen_name_id: style.pen_name_id,
        color_argb: style.color_argb,
        pen_size: style.pen_size,
        fixed_width: style.fixed_width,
        initial_tolerance: style.initial_tolerance,
        size_level: style.size_level,
        particle_density: style.particle_density,
        stroke_type_raw: style.stroke_type_raw,
        color_type_raw: style.color_type_raw,
        line_type_raw: style.line_type_raw,
        ..crate::StrokeStyle::default()
    };
    if *style != expected_style
        || style.size_level.unwrap_or(0) != 0
        || style.particle_density.unwrap_or(1) != 1
    {
        return Err(InkRasterError::UnsupportedStroke(
            "Fountain V14 style extension",
        ));
    }
    if settings.advanced_settings.as_deref() != Some("14;")
        || settings.tool_type_raw != 2
        || !p.compressed
        || !p.stylus_channels
        || p.fixed_width
        || p.fixed_opacity
        || p.alpha_lock
        || p.replay_only
        || p.reveal_mode
        || p.top_layer_pen
        || p.rainbow_effect
        || settings.style.stroke_type_raw.unwrap_or(0) != 0
        || settings.style.color_type_raw.unwrap_or(0) != 0
        || settings.style.line_type_raw.unwrap_or(0) != 0
        || settings
            .metadata
            .as_ref()
            .is_some_and(|m| m.rotation_degrees.is_some_and(|v| v != 0.))
    {
        return Err(InkRasterError::UnsupportedStroke(
            "normal saved Fountain V14 stylus profile",
        ));
    }
    if stroke.points.len() > 4096
        || stroke
            .points
            .windows(2)
            .map(|p| (p[1].x - p[0].x).hypot(p[1].y - p[0].y))
            .sum::<f64>()
            > 8190. * (0.82 / context.pixels_per_unit as f64)
    {
        return Err(InkRasterError::WorkLimit);
    }
    Ok(())
}

/// Validated per-paint destination and crop, independent of source admission.
struct RasterFrame {
    context: InkRasterContext,
    source_origin: [i32; 2],
    pixel_count: usize,
}

impl RasterFrame {
    fn new(context: InkRasterContext, source_origin: [i32; 2]) -> Result<Self, InkRasterError> {
        let pixel_count = context.pixel_count()?;
        if source_origin.iter().any(|v| v.unsigned_abs() > 16_384) {
            return Err(InkRasterError::InvalidContext);
        }
        Ok(Self {
            context,
            source_origin,
            pixel_count,
        })
    }
}

fn prepare_dots(
    dots: &super::super::fountain::Dots,
    frame: RasterFrame,
    paint_argb: u32,
    direct: bool,
    budget: usize,
) -> Result<Plan, InkRasterError> {
    let RasterFrame {
        context,
        source_origin,
        pixel_count: count,
    } = frame;
    if dots.points.len() > 8192 {
        return Err(InkRasterError::WorkLimit);
    }
    let directions = dots
        .directions
        .as_ref()
        .ok_or(InkRasterError::UnsupportedStroke("Fountain V14 directions"))?;
    let [width, height] = [context.width as usize, context.height as usize];
    let scale = context.pixels_per_unit as f32;
    let sy = if context.origin == InkRasterOrigin::TopLeft {
        -scale
    } else {
        scale
    };
    let px = 2. / width as f32;
    let py = 2. / height as f32;
    let projection = [
        scale * px,
        sy * py,
        (-source_origin[0] as f32 * scale) * px - 1.,
        ((if sy < 0. { height as f32 } else { 0. }) - source_origin[1] as f32 * sy) * py - 1.,
    ];
    let corners = [
        ([0., 1.], [1., -1.]),
        ([0., 0.], [-1., 1.]),
        ([1., 1.], [1., 1.]),
        ([1., 0.], [-1., -1.]),
    ];
    let mut stamps = Vec::new();
    stamps
        .try_reserve_exact(dots.points.len())
        .map_err(|_| InkRasterError::Allocation)?;
    let mut work = count * 2 + dots.points.len() * 64;
    for ((&point, &radius), &direction) in dots.points.iter().zip(&dots.radii).zip(directions) {
        let row = super::addpoint_record(point, radius, direction, scale)
            .ok_or(InkRasterError::InvalidStroke("Fountain V14 derived record"))?;
        let clip = corners.map(|(_, [cx, cy])| {
            let x = row[0] + (cx * (row[4] + row[5] * cy)) * row[2];
            let y = row[1] + (cx * (row[5] - row[4] * cy)) * row[3];
            [
                x * projection[0] + projection[2],
                y * projection[1] + projection[3],
            ]
        });
        if clip.iter().flatten().any(|v| !v.is_finite()) {
            return Err(InkRasterError::InvalidStroke("Fountain V14 clip position"));
        }
        let positions = clip.map(|[x, y]| {
            (
                x.mul_add(width as f32 * 0.5, width as f32 * 0.5),
                y.mul_add(height as f32 * 0.5, height as f32 * 0.5),
            )
        });
        let bound =
            |value: f32, extent: usize| (value - 0.5).ceil().clamp(0., extent as f32) as usize;
        let bounds = [
            bound(
                positions.iter().map(|v| v.0).fold(f32::INFINITY, f32::min),
                width,
            ),
            bound(
                positions
                    .iter()
                    .map(|v| v.0)
                    .fold(f32::NEG_INFINITY, f32::max),
                width,
            ),
            bound(
                positions.iter().map(|v| v.1).fold(f32::INFINITY, f32::min),
                height,
            ),
            bound(
                positions
                    .iter()
                    .map(|v| v.1)
                    .fold(f32::NEG_INFINITY, f32::max),
                height,
            ),
        ];
        let vertices = std::array::from_fn::<_, 4, _>(|i| ClipVertex {
            position: clip[i],
            uv: corners[i].0,
        });
        let mut triangles: SmallVec<[Triangle; 2]> = SmallVec::new();
        for indices in [[0, 1, 2], [2, 1, 3]] {
            let clipped = clip_triangle(indices.map(|i| vertices[i]), [width, height])?;
            triangles
                .try_reserve(clipped.len())
                .map_err(|_| InkRasterError::Allocation)?;
            triangles.extend_from_slice(&clipped);
        }
        work += (bounds[1] - bounds[0]) * (bounds[3] - bounds[2]) * triangles.len();
        if work > budget {
            return Err(InkRasterError::WorkLimit);
        }
        stamps.push(Stamp {
            triangles,
            bounds,
            alpha: h(row[6]),
            inner: h(row[7]),
            width_alpha: h(row[8]),
        });
    }
    Ok(Plan {
        stamps,
        frame: [width, height],
        color: [16, 8, 0, 24].map(|s| ((paint_argb >> s) as u8) as f32 / 255.),
        direct,
        work,
    })
}

impl Plan {
    pub(crate) fn work(&self) -> usize {
        self.work
    }

    pub(crate) fn render(&self, prior: &[[u8; 4]]) -> Result<Vec<[u8; 4]>, InkRasterError> {
        let [width, height] = self.frame;
        if prior.len() != width * height {
            return Err(InkRasterError::InvalidBackdrop);
        }
        let mut mask = Vec::new();
        let mut output = Vec::new();
        if self.direct {
            output
                .try_reserve_exact(prior.len())
                .map_err(|_| InkRasterError::Allocation)?;
            output.extend_from_slice(prior);
        } else {
            mask.try_reserve_exact(prior.len())
                .map_err(|_| InkRasterError::Allocation)?;
            mask.resize(prior.len(), 0u8);
        }
        for stamp in &self.stamps {
            if stamp.triangles.is_empty() {
                continue;
            }
            let [xa, xb, ya, yb] = stamp.bounds;
            for y in ya..yb {
                for x in xa..xb {
                    let fixed = (x as i64 * 256 + 128, y as i64 * 256 + 128);
                    let Some(triangle) = stamp.triangles.iter().find(|t| inside(t.coverage, fixed))
                    else {
                        continue;
                    };
                    let [u, v] = triangle
                        .uv
                        .map(|(a, dx, dy)| h(dy.mul_add(y as f32, dx.mul_add(x as f32, a))));
                    let dx = half_add(0.5, -u);
                    let dy = half_add(0.5, -v);
                    let gradient =
                        half_add(1., -half_mul(h(3.72), half_add(dy.abs(), -0.25))).clamp(0., 1.);
                    let distance = h(half_add(half_mul(dx, dx), half_mul(dy, dy)).sqrt());
                    let mut alpha = stamp.width_alpha;
                    if distance > stamp.inner {
                        alpha = half_mul(
                            alpha,
                            half_add(
                                1.,
                                -h(half_add(distance, -stamp.inner) / half_add(0.5, -stamp.inner)),
                            ),
                        );
                    }
                    let alpha = half_mul(half_mul(stamp.alpha, gradient), alpha);
                    let index = (height - 1 - y) * width + x;
                    if self.direct {
                        let source = [
                            quantize(half_mul(h(self.color[0]), alpha)),
                            quantize(half_mul(h(self.color[1]), alpha)),
                            quantize(half_mul(h(self.color[2]), alpha)),
                            quantize(alpha),
                        ];
                        output[index] = over(output[index], source);
                    } else {
                        mask[index] = mask[index].max(quantize(alpha));
                    }
                }
            }
        }
        if self.direct {
            return Ok(output);
        }
        let mut pixels = Vec::new();
        pixels
            .try_reserve_exact(prior.len())
            .map_err(|_| InkRasterError::Allocation)?;
        for (mask, &prior) in mask.iter().zip(prior) {
            let alpha = sample_unorm8_half_rtz(*mask) * self.color[3];
            let source = [
                self.color[0] * alpha,
                self.color[1] * alpha,
                self.color[2] * alpha,
                alpha,
            ]
            .map(quantize);
            pixels.push(over(prior, source));
        }
        Ok(pixels)
    }
}

#[cfg(all(test, feature = "serde"))]
mod tests {
    use super::*;

    fn source() -> Stroke {
        serde_json::from_str(include_str!(
            "../../../tests/assets/fountain-v14/saved-source-29.json"
        ))
        .unwrap()
    }

    fn context() -> InkRasterContext {
        InkRasterContext {
            width: 8,
            height: 8,
            pixels_per_unit: 1,
            origin: InkRasterOrigin::TopLeft,
        }
    }

    #[test]
    fn frame_and_crop_rejections_precede_source_and_work_rejections() {
        let mut stroke = source();
        stroke.rendering = None;
        let mut invalid_frame = context();
        invalid_frame.width = 0;
        assert!(matches!(
            prepare(&stroke, invalid_frame, [0; 2], 0xff000000, true, 0),
            Err(InkRasterError::InvalidContext)
        ));
        assert!(matches!(
            prepare(&stroke, context(), [i32::MIN, 0], 0xff000000, true, 0),
            Err(InkRasterError::InvalidContext)
        ));
        assert!(matches!(
            prepare(&stroke, context(), [0; 2], 0xff000000, true, 0),
            Err(InkRasterError::UnsupportedStroke(
                "missing Fountain settings"
            ))
        ));
    }

    #[test]
    fn snapshots_validate_each_paint_frame_and_crop_before_work() {
        let snapshot =
            FountainV14Prepared::new(&source(), context(), crate::Point { x: 0., y: 1. }).unwrap();
        let mut invalid_frame = context();
        invalid_frame.height = 0;
        assert!(matches!(
            prepare_snapshot(&snapshot, invalid_frame, [0; 2], 0xff000000, 0),
            Err(InkRasterError::InvalidContext)
        ));
        assert!(matches!(
            prepare_snapshot(&snapshot, context(), [i32::MIN, 0], 0xff000000, 0),
            Err(InkRasterError::InvalidContext)
        ));
        assert!(matches!(
            prepare_snapshot(&snapshot, context(), [0; 2], 0xff000000, 0),
            Err(InkRasterError::WorkLimit)
        ));
    }
}
