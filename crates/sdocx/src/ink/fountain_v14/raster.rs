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

pub(crate) fn prepare(
    stroke: &Stroke,
    context: InkRasterContext,
    source_origin: [i32; 2],
    paint_argb: u32,
    direct: bool,
    budget: usize,
) -> Result<Plan, InkRasterError> {
    prepare_with_dots(stroke, context, source_origin, paint_argb, direct, budget)
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

fn prepare_with_dots(
    stroke: &Stroke,
    context: InkRasterContext,
    source_origin: [i32; 2],
    paint_argb: u32,
    direct: bool,
    budget: usize,
) -> Result<Plan, InkRasterError> {
    let count = context.pixel_count()?;
    if source_origin.iter().any(|v| v.unsigned_abs() > 16_384) {
        return Err(InkRasterError::InvalidContext);
    }
    validate_source(stroke, context)?;
    let dots = super::super::fountain::prepare_v14_with_output_scale(
        stroke,
        context.pixels_per_unit as f32,
    )
    .ok_or(InkRasterError::UnsupportedStroke(
        "saved Fountain V14 source",
    ))?;
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
