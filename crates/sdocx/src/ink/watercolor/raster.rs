//! Bounded WaterColor RTV1 host paint with a fresh mask and redraw enabled.

use super::shader::{self, half_mul, half_rne};
use crate::InkRasterError;
use crate::ink::host_raster::{
    ClippedTriangle as Triangle, clip_triangle, triangle_contains as inside,
};
use crate::ink::host_rgba8::{over, quantize, sample_unorm8_half_rtz};
use smallvec::SmallVec;

const PATTERN: &[u8] = include_bytes!("assets/pattern.rg8");
const MAX_RECORDS: usize = 16_384;
const MAX_MASK_WORK: usize = 16 * 1024 * 1024;

struct Stamp {
    triangles: SmallVec<[Triangle; 2]>,
    bounds: [usize; 4],
    alpha: f32,
}

pub(crate) struct Plan {
    frame: [usize; 2],
    stamps: Vec<Stamp>,
    color: [f32; 4],
    work: usize,
}

pub(crate) fn prepare(
    rows: &[[f32; 6]],
    frame: [u32; 2],
    projection: [f32; 16],
    argb: u32,
    budget: usize,
) -> Result<Plan, InkRasterError> {
    let [width, height] = frame.map(|n| n as usize);
    let count = width
        .checked_mul(height)
        .ok_or(InkRasterError::InvalidContext)?;
    if width == 0
        || height == 0
        || width > 8192
        || height > 8192
        || count > 8 * 1024 * 1024
        || projection.iter().any(|v| !v.is_finite())
    {
        return Err(InkRasterError::InvalidContext);
    }
    if rows.len() > MAX_RECORDS {
        return Err(InkRasterError::WorkLimit);
    }
    let mut work = count
        .checked_mul(2)
        .and_then(|v| v.checked_add(rows.len() * 64))
        .ok_or(InkRasterError::WorkLimit)?;
    if work > budget {
        return Err(InkRasterError::WorkLimit);
    }
    let mut mask_work = 0usize;
    let mut stamps = Vec::new();
    stamps
        .try_reserve_exact(rows.len())
        .map_err(|_| InkRasterError::Allocation)?;
    for &row in rows {
        if row.iter().any(|v| !v.is_finite())
            || row[2].abs() > 1.
            || row[3].abs() > 1.
            || row[4] <= 0.
            || row[4] > 768.
            || !(0.0..=1.).contains(&row[5])
        {
            return Err(InkRasterError::InvalidStroke("WaterColor derived stamp"));
        }
        let vertices = shader::vertices_with_projection(row, projection);
        if vertices
            .iter()
            .any(|v| v.position.iter().any(|p| !p.is_finite()))
        {
            return Err(InkRasterError::InvalidStroke(
                "WaterColor half-precision position",
            ));
        }
        let mut triangles: SmallVec<[Triangle; 2]> = SmallVec::new();
        for indices in [[0, 1, 2], [2, 1, 3]] {
            let clipped = clip_triangle(indices.map(|i| vertices[i]), [width, height])?;
            triangles
                .try_reserve(clipped.len())
                .map_err(|_| InkRasterError::Allocation)?;
            triangles.extend_from_slice(&clipped);
        }
        let screen = vertices.map(|v| {
            [
                v.position[0].mul_add(width as f32 * 0.5, width as f32 * 0.5),
                height as f32 - v.position[1].mul_add(height as f32 * 0.5, height as f32 * 0.5),
            ]
        });
        let bound =
            |v: f32, extent: usize| (f64::from(v) - 0.5).ceil().clamp(0., extent as f64) as usize;
        let bounds = [
            bound(
                screen.iter().map(|v| v[0]).fold(f32::INFINITY, f32::min),
                width,
            ),
            bound(
                screen
                    .iter()
                    .map(|v| v[0])
                    .fold(f32::NEG_INFINITY, f32::max),
                width,
            ),
            bound(
                screen.iter().map(|v| v[1]).fold(f32::INFINITY, f32::min),
                height,
            ),
            bound(
                screen
                    .iter()
                    .map(|v| v[1])
                    .fold(f32::NEG_INFINITY, f32::max),
                height,
            ),
        ];
        let area = (bounds[1] - bounds[0]) * (bounds[3] - bounds[2]);
        let samples = area
            .checked_mul(triangles.len())
            .ok_or(InkRasterError::WorkLimit)?;
        mask_work = mask_work
            .checked_add(samples)
            .ok_or(InkRasterError::WorkLimit)?;
        work = work.checked_add(samples).ok_or(InkRasterError::WorkLimit)?;
        if mask_work > MAX_MASK_WORK || work > budget {
            return Err(InkRasterError::WorkLimit);
        }
        stamps.push(Stamp {
            triangles,
            bounds,
            alpha: half_rne(row[5]),
        });
    }
    Ok(Plan {
        frame: [width, height],
        stamps,
        color: shader::input_color(argb),
        work,
    })
}

impl Plan {
    pub(crate) fn work(&self) -> usize {
        self.work
    }

    fn empty_mask(&self) -> Result<Vec<[u8; 4]>, InkRasterError> {
        let mut mask = Vec::new();
        mask.try_reserve_exact(self.frame[0] * self.frame[1])
            .map_err(|_| InkRasterError::Allocation)?;
        mask.resize(self.frame[0] * self.frame[1], [0; 4]);
        Ok(mask)
    }

    fn stamp(&self, stamp: &Stamp, mask: &mut [[u8; 4]]) {
        if stamp.triangles.is_empty() {
            return;
        }
        let [width, height] = self.frame;
        let [xa, xb, ya, yb] = stamp.bounds;
        for y in ya..yb {
            for x in xa..xb {
                let pixel = (x as i64 * 256 + 128, (height - 1 - y) as i64 * 256 + 128);
                let Some(triangle) = stamp.triangles.iter().find(|t| inside(t.coverage, pixel))
                else {
                    continue;
                };
                let uv = triangle.uv.map(|(a, dx, dy)| {
                    dy.mul_add((height - 1 - y) as f32, dx.mul_add(x as f32, a))
                });
                let index = |f: f32| (f * 256.).floor().clamp(0., 255.) as usize;
                let offset = (index(uv[1]) * 256 + index(uv[0])) * 2;
                let source = [
                    PATTERN[offset],
                    quantize(half_mul(
                        sample_unorm8_half_rtz(PATTERN[offset + 1]),
                        stamp.alpha,
                    )),
                    0,
                    255,
                ];
                for (destination, source) in mask[y * width + x].iter_mut().zip(source) {
                    *destination = destination.saturating_add(source);
                }
            }
        }
    }

    pub(crate) fn render(&self, prior: &[[u8; 4]]) -> Result<Vec<[u8; 4]>, InkRasterError> {
        if prior.len() != self.frame[0] * self.frame[1] {
            return Err(InkRasterError::InvalidBackdrop);
        }
        let mut pixels = self.empty_mask()?;
        for stamp in &self.stamps {
            self.stamp(stamp, &mut pixels);
        }
        for (mask, &prior) in pixels.iter_mut().zip(prior) {
            let source = if mask[0] == 0 {
                [0; 4]
            } else {
                shader::source_shader_rgba(mask[1], self.color, mask[0]).1
            };
            *mask = over(prior, source);
        }
        Ok(pixels)
    }

    #[cfg(all(test, feature = "serde"))]
    pub(crate) fn mask_with_trace(&self, index: usize) -> (Vec<[u8; 4]>, Vec<[u8; 4]>) {
        let mut mask = self.empty_mask().unwrap();
        let mut trace = Vec::new();
        for stamp in &self.stamps {
            self.stamp(stamp, &mut mask);
            trace.push(mask[index]);
        }
        (mask, trace)
    }
}
