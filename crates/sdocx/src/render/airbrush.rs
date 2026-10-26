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
// Six gradients/masks and two independently transformed triangle images per stamp.
// The bound includes generous ID/coordinate attribute growth and is checked
// before any per-stamp SVG elements are allocated.
const STAMP_MARKUP_BUDGET: usize = 6_144;
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
struct TriangleCoverage {
    edges: [HalfPlane; 3],
    bounds: [f64; 4],
}

#[derive(Debug)]
pub(crate) struct Prepared {
    rows: Vec<[f32; 4]>,
    coverage: Vec<[TriangleCoverage; 2]>,
    atlas_to_page: Vec<[[f64; 6]; 2]>,
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
        .map(|row| projected_triangle_coverage(*row, half, viewport))
        .collect::<Option<Vec<_>>>()?;

    // Native CLAMP_TO_EDGE repeats the captured R8 boundary samples beyond
    // the quad. The padding is at least one output pixel at 1x plus one texel;
    // it changes texture storage only, not stamp geometry or phase.
    let pad_texels = (1.0 / texel_px).ceil() as u32 + 1;
    let edge = 96_u32.checked_add(pad_texels.checked_mul(2)?)?;
    let rgba_bytes = usize::try_from(edge.checked_mul(edge)?.checked_mul(4)?).ok()?;
    if rgba_bytes > MAX_PNG_BYTES {
        return None;
    }

    let mask_pixels = coverage.iter().flatten().try_fold(0_u64, |total, tri| {
        let width = tri.bounds[2].ceil() as u64;
        let height = tri.bounds[3].ceil() as u64;
        total.checked_add(width.checked_mul(height)?.checked_mul(3)?)
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
    let image_side = f64::from(edge);
    let atlas_to_page = source
        .rows
        .iter()
        .map(|row| native_uv_source_to_page(*row, half, viewport, pad_texels))
        .collect::<Option<Vec<_>>>()?;
    Some(Prepared {
        rows: source.rows,
        coverage,
        sample_ends: source.sample_ends,
        terminal: source.terminal,
        png,
        png_key,
        image_id,
        image_x: 0.0,
        image_y: 0.0,
        image_side,
        atlas_to_page,
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
        let masks: [[super::vector::Definition<Mask>; 3]; 2] =
            std::array::from_fn(|_| std::array::from_fn(|_| scene.definition::<Mask>()));
        for tri in 0..2 {
            for (edge_index, mask) in masks[tri].iter().enumerate() {
                let edge = coverage[tri].edges[edge_index];
                let gradient = scene.definition::<Gradient>();
                let (gradient_from, gradient_to, opacity_from, opacity_to) = if edge.inclusive_edge
                {
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
                let [x, y, width, height] = coverage[tri].bounds;
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
        for (index, masks) in stamp_masks.iter().enumerate() {
            let part = replay_index
                .and_then(|_| replay_parts.get(index).copied())
                .map(ReplayPart);
            let mut stamp = Group::new();
            for (triangle_masks, transform) in masks.iter().zip(&prepared.atlas_to_page[index]) {
                let mut image = Group::new()
                    .transformed(Transform::matrix_unrounded(*transform))
                    .add(Use::href(&prepared.image_id).replay_part(part));
                for mask in triangle_masks.iter().rev() {
                    image = Group::new().masked(mask).add(image);
                }
                stamp = Group::new().add(stamp).add(image);
            }
            scene.push(stamp);
        }
    });
    true
}

#[derive(Clone, Copy)]
struct NativeQuad {
    window: [[f32; 2]; 4],
    page: [[f64; 2]; 4],
}
#[derive(Clone, Copy)]
struct NativeTriangle {
    indices: [usize; 3],
    window: [[f32; 2]; 3],
    page: [[f64; 2]; 3],
}
#[derive(Clone, Copy)]
struct UvPlane {
    origin: f32,
    dx: f32,
    dy: f32,
}

fn native_projected_quad(row: [f32; 4], half: f32, viewport: [u32; 2]) -> Option<NativeQuad> {
    let [width, height] = viewport;
    if width == 0 || height == 0 {
        return None;
    }
    let w = width as f32;
    let h = height as f32;
    let [x, y, sin, cos] = row;
    let corners = [[-half, -half], [-half, half], [half, -half], [half, half]];
    let mut window = [[0.0_f32; 2]; 4];
    let mut page = [[0.0_f64; 2]; 4];
    for (i, [vx, vy]) in corners.into_iter().enumerate() {
        let mx = (cos * vx + x) + (-sin * vy);
        let my = (sin * vx + y) + (cos * vy);
        let clipx = (2.0 / w) * mx - 1.0;
        let clipy = (-2.0 / h) * my + 1.0;
        window[i][0] = (clipx * 1.0) * (w * 0.5) + w * 0.5;
        window[i][1] = (clipy * 1.0) * (h * 0.5) + h * 0.5;
        if window[i].iter().any(|coordinate| !coordinate.is_finite()) {
            return None;
        }
        page[i] = [f64::from(window[i][0]), f64::from(h - window[i][1])];
    }
    Some(NativeQuad { window, page })
}

fn orient_strip_triangles(quad: NativeQuad) -> Option<[NativeTriangle; 2]> {
    let mut out = Vec::with_capacity(2);
    for mut ids in [[0usize, 1, 2], [2, 1, 3]] {
        let fixed = |x: f32| i128::from(((x - 0.5) * 256.0).round_ties_even() as i64);
        let [i, j, k] = ids;
        let dx01 = fixed(quad.window[i][0]) - fixed(quad.window[j][0]);
        let dy01 = fixed(quad.window[i][1]) - fixed(quad.window[j][1]);
        let dx20 = fixed(quad.window[k][0]) - fixed(quad.window[i][0]);
        let dy20 = fixed(quad.window[k][1]) - fixed(quad.window[i][1]);
        let area = dx01
            .checked_mul(dy20)?
            .checked_sub(dx20.checked_mul(dy01)?)?;
        if area == 0 {
            return None;
        }
        if area < 0 {
            ids = [j, i, k];
        }
        out.push(NativeTriangle {
            indices: ids,
            window: ids.map(|n| quad.window[n]),
            page: ids.map(|n| quad.page[n]),
        });
    }
    out.try_into().ok()
}

// Captured Mesa setup_variant_0: keep separate binary32 operations and its
// half-pixel origin. Replacing this with an ideal affine loses nearest-cell ties.
fn mesa_linear_plane(p: [[f32; 2]; 3], a: [f32; 3]) -> Option<UvPlane> {
    let [p0, p1, p2] = p;
    let [a0, a1, a2] = a;
    let dx01 = p0[0] - p1[0];
    let dy01 = p0[1] - p1[1];
    let dx20 = p2[0] - p0[0];
    let dy20 = p2[1] - p0[1];
    let area = dx01 * dy20 - dy01 * dx20;
    if area == 0.0 || !area.is_finite() {
        return None;
    }
    let ooa = 1.0 / area;
    let dy20ooa = dy20 * ooa;
    let dy01ooa = dy01 * ooa;
    let dx20ooa = dx20 * ooa;
    let dx01ooa = dx01 * ooa;
    let da01 = a0 - a1;
    let da20 = a2 - a0;
    let dx = da01 * dy20ooa - da20 * dy01ooa;
    let dy = da20 * dx01ooa - da01 * dx20ooa;
    let centered = dx * (p0[0] - 0.5) + dy * (p0[1] - 0.5);
    let origin = a0 - centered;
    (origin.is_finite() && dx.is_finite() && dy.is_finite()).then_some(UvPlane { origin, dx, dy })
}

fn source_pixel_to_page(u: UvPlane, v: UvPlane, pad: u32, height: u32) -> Option<[f64; 6]> {
    let h = height as f32;
    let coeff = |p: UvPlane| {
        let sx = f64::from(96.0 * p.dx);
        let sy = f64::from(-96.0 * p.dy);
        let offset = p.origin - p.dx * 0.5 + p.dy * (h - 0.5);
        let c = f64::from(pad as f32 + 96.0 * offset);
        (sx, sy, c)
    };
    let (a, c, e) = coeff(u);
    let (b, d, f) = coeff(v);
    let det = a * d - b * c;
    if !det.is_finite() || det == 0.0 {
        return None;
    }
    let m = [
        d / det,
        -b / det,
        -c / det,
        a / det,
        (c * f - d * e) / det,
        (b * e - a * f) / det,
    ];
    m.iter().all(|x| x.is_finite()).then_some(m)
}

fn native_uv_source_to_page(
    row: [f32; 4],
    half: f32,
    viewport: [u32; 2],
    pad: u32,
) -> Option<[[f64; 6]; 2]> {
    let quad = native_projected_quad(row, half, viewport)?;
    let triangles = orient_strip_triangles(quad)?;
    let uv = [[0.0_f32, 1.0], [0.0, 0.0], [1.0, 1.0], [1.0, 0.0]];
    let mut matrices = Vec::with_capacity(2);
    for tri in triangles {
        let u = mesa_linear_plane(tri.window, tri.indices.map(|i| uv[i][0]))?;
        let v = mesa_linear_plane(tri.window, tri.indices.map(|i| uv[i][1]))?;
        matrices.push(source_pixel_to_page(u, v, pad, viewport[1])?);
    }
    matrices.try_into().ok()
}

fn triangle_halfplanes(points: [[f64; 2]; 3]) -> Option<TriangleCoverage> {
    let center = [
        points.iter().map(|p| p[0]).sum::<f64>() / 3.0,
        points.iter().map(|p| p[1]).sum::<f64>() / 3.0,
    ];
    let edges = std::array::from_fn(|i| {
        let a = points[i];
        let b = points[(i + 1) % 3];
        let dx = b[0] - a[0];
        let dy = b[1] - a[1];
        let len = dx.hypot(dy);
        let cross = dx * (center[1] - a[1]) - dy * (center[0] - a[0]);
        let (nx, ny) = if cross > 0.0 {
            (-dy / len, dx / len)
        } else {
            (dy / len, -dx / len)
        };
        let mid = [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5];
        HalfPlane {
            gradient_from: [mid[0] + nx * 0.5, mid[1] + ny * 0.5],
            gradient_to: [mid[0] - nx * 0.5, mid[1] - ny * 0.5],
            // Inclusion is geometric left/bottom; winding reverses the
            // directed-edge predicate for the two page-space triangles.
            inclusive_edge: if cross > 0.0 {
                dy < 0.0 || (dy == 0.0 && dx < 0.0)
            } else {
                dy > 0.0 || (dy == 0.0 && dx > 0.0)
            },
        }
    });
    let minx = points.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min);
    let maxx = points
        .iter()
        .map(|p| p[0])
        .fold(f64::NEG_INFINITY, f64::max);
    let miny = points.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min);
    let maxy = points
        .iter()
        .map(|p| p[1])
        .fold(f64::NEG_INFINITY, f64::max);
    let bounds = [minx - 1.0, miny - 1.0, maxx - minx + 2.0, maxy - miny + 2.0];
    if bounds.iter().any(|v| !v.is_finite()) {
        return None;
    }
    Some(TriangleCoverage { edges, bounds })
}

fn projected_triangle_coverage(
    row: [f32; 4],
    half: f32,
    viewport: [u32; 2],
) -> Option<[TriangleCoverage; 2]> {
    let quad = native_projected_quad(row, half, viewport)?;
    let triangles = orient_strip_triangles(quad)?;
    let mut out = Vec::with_capacity(2);
    for tri in triangles {
        out.push(triangle_halfplanes(tri.page)?);
    }
    out.try_into().ok()
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
