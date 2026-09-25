//! Software counterpart of the saved RTV4/RTV5 coverage shaders.
//!
//! Positions are snapped to the target rasterizer's subpixel grid before
//! interpolating the native quad UVs. Coverage is quantized once per stamp,
//! combined with MAX, then colored when the stroke is composited.
use super::PreparedStroke;

/// Pixel grid and page-to-pixel mapping for fountain ink.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InkViewport {
    pub width: u32,
    pub height: u32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub offset_x: f32,
    pub offset_y: f32,
    /// GLES SUBPIXEL_BITS. The validated SwiftShader reference reports four.
    pub subpixel_bits: u8,
}

/// A cropped coverage mask positioned on the viewport's unchanged pixel grid.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct InkMask {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub alpha: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum InkRasterError {
    #[error("invalid ink viewport or stamp attributes")]
    InvalidInput,
    #[error("ink raster exceeds its allocation limit")]
    ResourceLimit,
}

struct Stamp {
    quad: [[f64; 2]; 4],
    inner: f32,
    alpha: f32,
    bounds: [u32; 4],
}

fn bounds(quad: &[[f64; 2]; 4], width: u32, height: u32) -> [u32; 4] {
    let mut b = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for &[x, y] in quad {
        b[0] = b[0].min(x);
        b[1] = b[1].min(y);
        b[2] = b[2].max(x);
        b[3] = b[3].max(y);
    }
    [
        (b[0].floor() as u32).min(width),
        (b[1].floor() as u32).min(height),
        (b[2].ceil() as u32).min(width),
        (b[3].ceil() as u32).min(height),
    ]
}

/// Reconstruct native saved fountain coverage. Other pen profiles return None.
/// The mask excludes the stroke color/opacity, which must be applied once.
pub fn rasterize_fountain(
    paint: &PreparedStroke<'_>,
    viewport: InkViewport,
) -> Result<Option<InkMask>, InkRasterError> {
    let Some(version @ (4 | 5)) = paint.fountain_shader else {
        return Ok(None);
    };
    let Some(radii) = paint.dot_radii.as_ref() else {
        return Err(InkRasterError::InvalidInput);
    };
    rasterize_fountain_geometry(
        &paint.points,
        radii,
        paint.dot_directions.as_deref(),
        version,
        viewport,
    )
}

/// Rasterize already prepared fountain geometry, including a replay prefix.
pub fn rasterize_fountain_geometry(
    points: &[crate::Point],
    radii: &[f64],
    directions: Option<&[crate::Point]>,
    version: u8,
    viewport: InkViewport,
) -> Result<Option<InkMask>, InkRasterError> {
    if !matches!(version, 4 | 5)
        || viewport.width == 0
        || viewport.height == 0
        || radii.len() != points.len()
        || (version == 4 && directions.is_none_or(|d| d.len() != radii.len()))
        || !viewport.scale_x.is_finite()
        || viewport.scale_x <= 0.
        || !viewport.scale_y.is_finite()
        || viewport.scale_y <= 0.
        || !viewport.offset_x.is_finite()
        || !viewport.offset_y.is_finite()
        || viewport.subpixel_bits > 16
    {
        return Err(InkRasterError::InvalidInput);
    }
    // Bound both stamp storage and the eventual RGBA8 output to 64 MiB each.
    const MAX_BYTES: usize = 64 * 1024 * 1024;
    if points.len() > MAX_BYTES / std::mem::size_of::<Stamp>() {
        return Err(InkRasterError::ResourceLimit);
    }
    let sx = 1. / viewport.scale_x;
    let sy = 1. / viewport.scale_y;
    let grid = (1_u32 << viewport.subpixel_bits) as f64;
    let snap = |value: f32, size: u32, flip: bool| {
        let factor = if flip { -2. } else { 2. } / size as f32;
        let clip = value * factor + if flip { 1. } else { -1. };
        let half = size as f32 * 0.5;
        let window = clip * half + half;
        let snapped = (window as f64 * grid).round_ties_even() / grid;
        if flip { size as f64 - snapped } else { snapped }
    };
    let mut stamps = Vec::with_capacity(radii.len());
    let mut all = [viewport.width, viewport.height, 0, 0];
    for (i, p) in points.iter().enumerate() {
        let radius = radii[i] as f32;
        let rx = radius / sx;
        let ry = radius / sy;
        let factor = if rx.min(ry) < 0.5 {
            0.5 / rx.min(ry)
        } else {
            1.
        };
        let ex = rx * factor.max(1.) + 0.5;
        let ey = ry * factor.max(1.) + 0.5;
        let inner = ((ex.max(ey) - 1.) / ex.max(ey)) * 0.5;
        let (dx, dy) = if version == 4 {
            let d = directions.unwrap()[i];
            (d.x as f32, d.y as f32)
        } else {
            (1., 0.)
        };
        let x = (p.x * viewport.scale_x as f64 + viewport.offset_x as f64) as f32;
        let y = (p.y * viewport.scale_y as f64 + viewport.offset_y as f64) as f32;
        if radius <= 0.
            || ![x, y, ex, ey, dx, dy, inner, factor]
                .iter()
                .all(|v| v.is_finite())
        {
            return Err(InkRasterError::InvalidInput);
        }
        // Native RTV4 triangle strip: UV (0,1), (0,0), (1,1), (1,0).
        let quad = [(1., -1.), (-1., 1.), (1., 1.), (-1., -1.)].map(|(a, b)| {
            [
                snap(x + (a * (dx + dy * b)) * ex, viewport.width, false),
                snap(y + (a * (dy - dx * b)) * ey, viewport.height, true),
            ]
        });
        let b = bounds(&quad, viewport.width, viewport.height);
        if b[0] >= b[2] || b[1] >= b[3] {
            continue;
        }
        all[0] = all[0].min(b[0]);
        all[1] = all[1].min(b[1]);
        all[2] = all[2].max(b[2]);
        all[3] = all[3].max(b[3]);
        stamps.push(Stamp {
            quad,
            inner,
            alpha: 1. / factor,
            bounds: b,
        });
    }
    if stamps.is_empty() {
        return Ok(None);
    }
    let width = all[2] - all[0];
    let height = all[3] - all[1];
    let len = (width as usize)
        .checked_mul(height as usize)
        .filter(|&n| n <= MAX_BYTES / 4)
        .ok_or(InkRasterError::ResourceLimit)?;
    let mut mask = InkMask {
        x: all[0],
        y: all[1],
        width,
        height,
        alpha: vec![0; len],
    };
    for stamp in stamps {
        for y in stamp.bounds[1]..stamp.bounds[3] {
            for x in stamp.bounds[0]..stamp.bounds[2] {
                let Some([u, v]) = interpolate(&stamp.quad, x as f64 + 0.5, y as f64 + 0.5) else {
                    continue;
                };
                let u = 0.5 - u as f32;
                let v = 0.5 - v as f32;
                let distance = (u * u + v * v).sqrt();
                let mut alpha = stamp.alpha;
                if distance > stamp.inner {
                    alpha *= 1. - (distance - stamp.inner) / (0.5 - stamp.inner);
                }
                if version == 4 {
                    alpha *= (1. - (0.93 / 0.25) * (0.25 - (0.5 - v.abs()))).clamp(0., 1.);
                }
                let alpha = (alpha.clamp(0., 1.) * 255.).round_ties_even() as u8;
                let index = (y - mask.y) as usize * width as usize + (x - mask.x) as usize;
                mask.alpha[index] = mask.alpha[index].max(alpha);
            }
        }
    }
    Ok(Some(mask))
}

fn interpolate(quad: &[[f64; 2]; 4], x: f64, y: f64) -> Option<[f64; 2]> {
    const UV: [[f64; 2]; 4] = [[0., 1.], [0., 0.], [1., 1.], [1., 0.]];
    for [i, j, k] in [[0, 1, 2], [2, 1, 3]] {
        let [ax, ay] = quad[i];
        let [bx, by] = quad[j];
        let [cx, cy] = quad[k];
        let denominator = (by - cy) * (ax - cx) + (cx - bx) * (ay - cy);
        if denominator == 0. {
            continue;
        }
        let a = ((by - cy) * (x - cx) + (cx - bx) * (y - cy)) / denominator;
        let b = ((cy - ay) * (x - cx) + (ax - cx) * (y - cy)) / denominator;
        let c = 1. - a - b;
        if a.min(b).min(c) >= -1e-9 {
            return Some([
                a * UV[i][0] + b * UV[j][0] + c * UV[k][0],
                a * UV[i][1] + b * UV[j][1] + c * UV[k][1],
            ]);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_viewports_and_oversized_rgba_output() {
        let mut viewport = InkViewport {
            width: 8192,
            height: 8192,
            scale_x: 1.,
            scale_y: 1.,
            offset_x: 0.,
            offset_y: 0.,
            subpixel_bits: 4,
        };
        let points = [crate::Point { x: 4096., y: 4096. }];
        assert_eq!(
            rasterize_fountain_geometry(&points, &[8192.], None, 5, viewport).unwrap_err(),
            InkRasterError::ResourceLimit
        );
        viewport.width = 0;
        assert_eq!(
            rasterize_fountain_geometry(&points, &[1.], None, 5, viewport).unwrap_err(),
            InkRasterError::InvalidInput
        );
    }

    #[test]
    fn overlapping_stamps_use_maximum_coverage() {
        let viewport = InkViewport {
            width: 16,
            height: 16,
            scale_x: 1.,
            scale_y: 1.,
            offset_x: 0.,
            offset_y: 0.,
            subpixel_bits: 4,
        };
        let point = crate::Point { x: 8., y: 8. };
        let single = rasterize_fountain_geometry(&[point], &[2.], None, 5, viewport)
            .unwrap()
            .unwrap();
        let repeated = rasterize_fountain_geometry(&[point, point], &[2., 2.], None, 5, viewport)
            .unwrap()
            .unwrap();
        assert!(single.alpha.iter().any(|&alpha| alpha > 0));
        assert_eq!(single.alpha, repeated.alpha);
    }
}
