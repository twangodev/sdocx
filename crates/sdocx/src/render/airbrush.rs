//! Bounded native AirBrush RTV1 texture paint for the explicit saved GLV2 profile.
//!
//! The captured 96x96 R8 field is one shared intrinsic RGBA image. Each saved
//! V2 stamp keeps its native affine; four bounded alpha masks reproduce the
//! source quad's point-sampled hard coverage from float32 projected corners.
use super::vector::{
    ColorValue, Definitions, Gradient, Group, Image, LinearGradient, Mask, Paint, Rectangle,
    ReplayPart, Scene, Stop, StrokeIndex, Styled, SvgMask, Transform, Use,
};
use crate::{Color, Stroke, ink::airbrush};
use std::io::Cursor;

use super::RenderTheme;

const PATTERN_R8: &[u8; 96 * 96] = include_bytes!("airbrush_assets/airbrush-pattern.r8");
const MAX_STAMPS: usize = 32_768;
const MAX_SOURCE_BYTES: usize = 4 * 1024 * 1024;
const MAX_ESTIMATED_MARKUP_BYTES: usize = 32 * 1024 * 1024;
// Four gradients, four masks, four nested groups, and one image use per stamp.
// The bound includes generous ID/coordinate attribute growth and is checked
// before any per-stamp SVG elements are allocated.
const STAMP_MARKUP_BUDGET: usize = 3_072;
const MAX_MASK_PIXELS: u64 = 64 * 1024 * 1024;
const MAX_PNG_BYTES: usize = 1024 * 1024;
#[cfg(test)]
const NATIVE_PATTERN_SHA256: &str =
    "2bcc637332cb6ca7da347f6bc0010a3312901fc19aa69dee31e4d8b4ced75172";

#[derive(Clone, Copy, Debug)]
struct HalfPlane {
    gradient_from: [f64; 2],
    gradient_to: [f64; 2],
    inclusive_edge: bool,
}

#[derive(Clone, Copy, Debug)]
struct QuadCoverage {
    edges: [HalfPlane; 4],
    bounds: [f64; 4], // x, y, width, height; one unit of mask-only padding
}

#[derive(Debug)]
pub(crate) struct Prepared {
    rows: Vec<[f32; 4]>,
    coverage: Vec<QuadCoverage>,
    sample_ends: Vec<usize>,
    terminal: std::ops::Range<usize>,
    png: Vec<u8>,
    png_key: String,
    image_id: String,
    image_x: f64,
    image_y: f64,
    image_side: f64,
}

pub(crate) fn prepare(stroke: &Stroke, theme: RenderTheme, viewport: [u32; 2]) -> Option<Prepared> {
    if viewport.contains(&0) {
        return None;
    }
    let source_bytes = stroke
        .points
        .len()
        .checked_mul(8)?
        .checked_add(stroke.pressures.len().checked_mul(4)?)?
        .checked_add(stroke.timestamps.len().checked_mul(4)?)?
        .checked_add(stroke.tilts.len().checked_mul(4)?)?
        .checked_add(stroke.orientations.len().checked_mul(4)?)?;
    if source_bytes > MAX_SOURCE_BYTES {
        return None;
    }
    let source = airbrush::saved_v2_from_stroke(stroke)?;
    if source.rows.is_empty() || source.rows.len() > MAX_STAMPS {
        return None;
    }
    if source.rows.iter().flatten().any(|lane| !lane.is_finite()) {
        return None;
    }
    let stamp_markup_bytes = source.rows.len().checked_mul(STAMP_MARKUP_BUDGET)?;
    if stamp_markup_bytes > MAX_ESTIMATED_MARKUP_BYTES {
        return None;
    }

    let native_argb = source.native_argb;
    let alpha = (native_argb >> 24) as u8;
    let native_rgb = [
        (native_argb >> 16) as u8,
        (native_argb >> 8) as u8,
        native_argb as u8,
    ];
    let source_color = stroke.color.unwrap_or(Color {
        r: native_rgb[0],
        g: native_rgb[1],
        b: native_rgb[2],
    });
    let resolved = theme.foreground_color(source_color);
    let rgb = [resolved.r, resolved.g, resolved.b];

    let half = source.rt_size as f32 * 0.5;
    let texel_px = (source.rt_size as f32) / 96.0;
    if !texel_px.is_finite() || texel_px <= 0.0 {
        return None;
    }
    let coverage = source
        .rows
        .iter()
        .map(|row| projected_quad_coverage(*row, half, viewport))
        .collect::<Option<Vec<_>>>()?;

    // Native CLAMP_TO_EDGE repeats the captured R8 boundary samples beyond
    // the quad. The padding is at least one output pixel at 1x plus one texel;
    // it changes texture storage only, not stamp geometry or phase.
    let pad_texels = (1.0 / texel_px).ceil() as u32 + 1;
    let pad = pad_texels as f32 * texel_px;
    let edge = 96_u32.checked_add(pad_texels.checked_mul(2)?)?;
    let rgba_bytes = usize::try_from(edge.checked_mul(edge)?.checked_mul(4)?).ok()?;
    if rgba_bytes > MAX_PNG_BYTES {
        return None;
    }

    let mask_pixels = coverage.iter().try_fold(0_u64, |total, quad| {
        let width = quad.bounds[2].ceil() as u64;
        let height = quad.bounds[3].ceil() as u64;
        total.checked_add(width.checked_mul(height)?.checked_mul(4)?)
    })?;
    if mask_pixels > MAX_MASK_PIXELS {
        return None;
    }

    let mut rgba = Vec::new();
    rgba.try_reserve_exact(rgba_bytes).ok()?;
    for y in 0..edge {
        // Preserve source row order in the PNG. The local-y reflection on each
        // Use below makes SVG nearest sampling follow native floor(v * 96),
        // including exact texel-row boundaries.
        let sy = y.saturating_sub(pad_texels).min(95) as usize;
        for x in 0..edge {
            let sx = x.saturating_sub(pad_texels).min(95) as usize;
            let r8 = PATTERN_R8[sy * 96 + sx];
            let pixel_alpha = native_texel_alpha(r8, alpha);
            rgba.extend_from_slice(&[rgb[0], rgb[1], rgb[2], pixel_alpha]);
        }
    }
    let png = encode_png_rgba(edge, edge, &rgba)?;
    if png.len() > MAX_PNG_BYTES {
        return None;
    }
    let png_base64_bytes = png.len().checked_add(2)?.checked_div(3)?.checked_mul(4)?;
    if stamp_markup_bytes.checked_add(png_base64_bytes)? > MAX_ESTIMATED_MARKUP_BYTES {
        return None;
    }

    let png_key = format!(
        "airbrush-v2-r8-v3-{:02x}{:02x}{:02x}-a{:02x}-size{}-pad{}",
        rgb[0], rgb[1], rgb[2], alpha, source.rt_size, pad_texels
    );
    let image_id = format!("sdocx-{png_key}");
    let image_side = f64::from(source.rt_size as f32 + 2.0 * pad);
    let image_x = -f64::from(half) - f64::from(pad);
    let image_y = image_x;
    Some(Prepared {
        rows: source.rows,
        coverage,
        sample_ends: source.sample_ends,
        terminal: source.terminal,
        png,
        png_key,
        image_id,
        image_x,
        image_y,
        image_side,
    })
}

pub(crate) fn render(scene: &mut Scene, prepared: &Prepared, replay_index: Option<usize>) -> bool {
    let mut stamp_masks = Vec::new();
    if stamp_masks.try_reserve_exact(prepared.rows.len()).is_err() {
        return false;
    }
    let image_key = prepared.png_key.clone();
    if !scene.has_shared_definition_key(&image_key) && scene.claim_shared_definition_key(image_key)
    {
        scene.push(
            Definitions::new().add(
                Image::embedded(&prepared.png, "image/png")
                    .id(&prepared.image_id)
                    .x(prepared.image_x)
                    .y(prepared.image_y)
                    .width(prepared.image_side)
                    .height(prepared.image_side)
                    .stretched()
                    .pixelated(),
            ),
        );
    }

    let mut definitions = Definitions::new();
    for coverage in &prepared.coverage {
        let masks: [super::vector::Definition<Mask>; 4] =
            std::array::from_fn(|_| scene.definition::<Mask>());
        for (edge_index, mask) in masks.iter().enumerate() {
            let edge = coverage.edges[edge_index];
            let gradient = scene.definition::<Gradient>();
            let (gradient_from, gradient_to, opacity_from, opacity_to) = if edge.inclusive_edge {
                (edge.gradient_to, edge.gradient_from, 0.0, 1.0)
            } else {
                (edge.gradient_from, edge.gradient_to, 1.0, 0.0)
            };
            definitions = definitions.add(
                LinearGradient::new(&gradient)
                    .x1(gradient_from[0])
                    .y1(gradient_from[1])
                    .x2(gradient_to[0])
                    .y2(gradient_to[1])
                    .add(Stop::new(0.5, Some(ColorValue::WHITE)).opacity(opacity_from))
                    .add(Stop::new(0.5, Some(ColorValue::WHITE)).opacity(opacity_to)),
            );
            let [x, y, width, height] = coverage.bounds;
            definitions = definitions.add(
                SvgMask::alpha(&mask.id_text(), x, y, width, height).add(
                    Rectangle::new()
                        .x(x)
                        .y(y)
                        .width(width)
                        .height(height)
                        .fill(Paint::Gradient(gradient)),
                ),
            );
        }
        stamp_masks.push(masks);
    }
    scene.push(definitions);

    let mut root = Group::new();
    if let Some(index) = replay_index {
        root = root.replay_stroke(StrokeIndex(index));
    }
    scene.scope(root, |scene| {
        let replay_parts = source_part_ends(
            &prepared.sample_ends,
            &prepared.terminal,
            prepared.rows.len(),
        );
        for (index, row) in prepared.rows.iter().enumerate() {
            let [x, y, sin, cos] = *row;
            let part = replay_index
                .and_then(|_| replay_parts.get(index).copied())
                .map(ReplayPart);
            let transform = Transform::matrix_unrounded([
                f64::from(cos),
                f64::from(sin),
                -f64::from(sin),
                f64::from(cos),
                f64::from(x),
                f64::from(y),
            ]);
            let flipped_image = Group::new()
                .transformed(Transform::matrix_unrounded([1.0, 0.0, 0.0, -1.0, 0.0, 0.0]))
                .add(Use::href(&prepared.image_id).replay_part(part));
            let mut stamp = Group::new().transformed(transform).add(flipped_image);
            for mask in stamp_masks[index].iter().rev() {
                stamp = Group::new().masked(mask).add(stamp);
            }
            scene.push(stamp);
        }
    });
    true
}

/// Recreate the native vertex transform and viewport projection using f32
/// arithmetic, then derive the four source-ordered quad half-planes in page
/// coordinates. Only mask regions receive the one-unit AABB expansion.
fn projected_quad_coverage(
    [x, y, sin, cos]: [f32; 4],
    half: f32,
    viewport: [u32; 2],
) -> Option<QuadCoverage> {
    let [width, height] = viewport;
    if width == 0 || height == 0 {
        return None;
    }
    let width_f = width as f32;
    let height_f = height as f32;
    let projection_x = 2.0_f32 / width_f;
    let projection_y = -2.0_f32 / height_f;
    let viewport_half_x = width_f * 0.5;
    let viewport_half_y = height_f * 0.5;
    let vertices = [[-half, -half], [-half, half], [half, half], [half, -half]];
    let mut points = [[0.0_f64; 2]; 4];
    for (index, [vx, vy]) in vertices.into_iter().enumerate() {
        // Keep the native matrix's separate multiply/add sequencing. Rust f32
        // operators round at each operation; do not combine into a fitted
        // affine or move the quad to pixel centers.
        let model_x = cos * vx + (-sin) * vy + x;
        let model_y = sin * vx + cos * vy + y;
        let clip_x = projection_x * model_x - 1.0;
        let clip_y = projection_y * model_y + 1.0;
        let window_x = (clip_x + 1.0) * viewport_half_x;
        let window_y = (clip_y + 1.0) * viewport_half_y;
        let page_y = height_f - window_y;
        if !window_x.is_finite() || !page_y.is_finite() {
            return None;
        }
        points[index] = [f64::from(window_x), f64::from(page_y)];
    }

    let center = [
        points.iter().map(|point| point[0]).sum::<f64>() * 0.25,
        points.iter().map(|point| point[1]).sum::<f64>() * 0.25,
    ];
    let min_x = points
        .iter()
        .map(|point| point[0])
        .fold(f64::INFINITY, f64::min);
    let max_x = points
        .iter()
        .map(|point| point[0])
        .fold(f64::NEG_INFINITY, f64::max);
    let min_y = points
        .iter()
        .map(|point| point[1])
        .fold(f64::INFINITY, f64::min);
    let max_y = points
        .iter()
        .map(|point| point[1])
        .fold(f64::NEG_INFINITY, f64::max);
    let signed_area = points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(4)
        .map(|(a, b)| a[0] * b[1] - b[0] * a[1])
        .sum::<f64>();
    if !signed_area.is_finite() || signed_area.abs() <= f64::EPSILON {
        return None;
    }

    let edges = std::array::from_fn(|index| {
        let a = points[index];
        let b = points[(index + 1) % 4];
        let dx = b[0] - a[0];
        let dy = b[1] - a[1];
        let length = dx.hypot(dy);
        let cross_to_center = dx * (center[1] - a[1]) - dy * (center[0] - a[0]);
        let (normal_x, normal_y) = if cross_to_center > 0.0 {
            (-dy / length, dx / length)
        } else {
            (dy / length, -dx / length)
        };
        let midpoint = [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5];
        HalfPlane {
            gradient_from: [midpoint[0] + normal_x * 0.5, midpoint[1] + normal_y * 0.5],
            gradient_to: [midpoint[0] - normal_x * 0.5, midpoint[1] - normal_y * 0.5],
            // Clockwise page-space quads include ties on left and bottom.
            inclusive_edge: dy > 0.0 || (dy == 0.0 && dx > 0.0),
        }
    });
    if edges.iter().any(|edge| {
        edge.gradient_from
            .into_iter()
            .chain(edge.gradient_to)
            .any(|coordinate| !coordinate.is_finite())
    }) {
        return None;
    }

    let bounds = [
        min_x - 1.0,
        min_y - 1.0,
        max_x - min_x + 2.0,
        max_y - min_y + 2.0,
    ];
    if bounds.iter().any(|value| !value.is_finite()) || bounds[2] <= 0.0 || bounds[3] <= 0.0 {
        return None;
    }
    Some(QuadCoverage { edges, bounds })
}

fn source_part_ends(
    history_ends: &[usize],
    terminal: &std::ops::Range<usize>,
    stamp_count: usize,
) -> Vec<usize> {
    let mut parts = Vec::with_capacity(stamp_count);
    let mut source_call = 0;
    for stamp in 0..stamp_count {
        while source_call < history_ends.len() && stamp >= history_ends[source_call] {
            source_call += 1;
        }
        let part = if stamp >= terminal.start && stamp < terminal.end {
            history_ends.len() + 2
        } else {
            source_call + 2
        };
        parts.push(part);
    }
    parts
}

/// Active host-Mesa FS: normalized R8 -> binary16 RTZ -> f32 alpha multiply,
/// then RGBA8 source-row conversion. All nonzero R8/255 values are normal in
/// binary16, so clearing 13 low mantissa bits implements that sampled value.
fn native_texel_alpha(r8: u8, native_alpha: u8) -> u8 {
    let normalized = f32::from(r8) / 255.0;
    let sampled = f32::from_bits(normalized.to_bits() & !0x1fff);
    let input_alpha = f32::from(native_alpha) / 255.0;
    ((sampled * input_alpha) * 255.0).round_ties_even() as u8
}

fn encode_png_rgba(width: u32, height: u32, pixels: &[u8]) -> Option<Vec<u8>> {
    let expected = usize::try_from(width.checked_mul(height)?.checked_mul(4)?).ok()?;
    if pixels.len() != expected {
        return None;
    }
    let mut encoded = Vec::new();
    {
        let mut encoder = png::Encoder::new(Cursor::new(&mut encoded), width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(pixels).ok()?;
        writer.finish().ok()?;
    }
    Some(encoded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_alpha_preserves_native_half_sampler_boundaries() {
        // These intrinsic texels straddle RGBA8 rounding after the captured
        // sampler's RTZ narrowing. Integer R8*A8/255 would incorrectly produce
        // 25 and 35, respectively, instead of the native shader's 24 and 34.
        assert!(PATTERN_R8.contains(&125));
        assert!(PATTERN_R8.contains(&176));
        for (texel, alpha) in [(0, 0), (4, 1), (125, 24), (129, 25), (176, 34), (255, 50)] {
            assert_eq!(native_texel_alpha(texel, 50), alpha);
        }
    }

    #[test]
    fn source_pattern_has_the_pinned_native_hash() {
        use sha2::{Digest, Sha256};
        assert_eq!(
            format!("{:x}", Sha256::digest(PATTERN_R8)),
            NATIVE_PATTERN_SHA256
        );
    }
}
