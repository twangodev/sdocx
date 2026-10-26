//! Test-only source-input CPU model for the four captured Smudge GLES draws.
//!
//! Inputs are generated RGBA8 source pixels and the two captured six-vertex
//! quads. Native readbacks appear only as SHA assertions in tests. This is an
//! experimental llvmpipe profile, not portable GLES behavior. Sampler integer
//! weights/lerps follow the bounded Oil llvmpipe model and Smudge's active
//! NIR/LLVM. Program4 output narrowing follows its f2f16/fptrunc; program5
//! remains f32 through the captured store path. This covers two explicit host
//! controls with native-generated quads, not arbitrary stroke-to-quad generation
//! or production rendering. CVTPS2DQ nearest-even assumes uncaptured MXCSR state;
//! enabled dithering has no observed effect in these controls.

use sha2::{Digest, Sha256};

const WIDTH: usize = 256;
const HEIGHT: usize = 256;
const SCRATCH: usize = 128;

/// Column-major uProjectionMatrix captured at all four draws.
const PROJECTION: [f32; 16] = [
    0.0078125,
    0.0,
    0.0,
    0.0,
    0.0,
    0.0078125,
    0.0,
    0.0,
    0.0,
    0.0,
    f32::from_bits(0xb8000100),
    0.0,
    -1.0,
    -1.0,
    0.0,
    1.0,
];

/// Six vec4 attributes uploaded once for draws 0/1 (little-endian f32 values).
const QUAD_START: [[f32; 4]; 6] = [
    [
        f32::from_bits(0xbe564098),
        f32::from_bits(0xc04d640a),
        0.0,
        1.0,
    ],
    [
        f32::from_bits(0xbe564098),
        f32::from_bits(0xbf4a6fda),
        0.0,
        0.0,
    ],
    [
        f32::from_bits(0x400d640a),
        f32::from_bits(0xc04d640a),
        1.0,
        1.0,
    ],
    [
        f32::from_bits(0x400d640a),
        f32::from_bits(0xc04d640a),
        1.0,
        1.0,
    ],
    [
        f32::from_bits(0xbe564098),
        f32::from_bits(0xbf4a6fda),
        0.0,
        0.0,
    ],
    [
        f32::from_bits(0x400d640a),
        f32::from_bits(0xbf4a6fda),
        1.0,
        0.0,
    ],
];

/// Six vec4 attributes uploaded once for draws 2/3.
const QUAD_NEXT: [[f32; 4]; 6] = [
    [
        f32::from_bits(0x3fb9b7bb),
        f32::from_bits(0xbf4a1c89),
        0.0,
        1.0,
    ],
    [
        f32::from_bits(0x3fb9b7bb),
        f32::from_bits(0x3fd081e2),
        0.0,
        0.0,
    ],
    [
        f32::from_bits(0x4077a3f0),
        f32::from_bits(0xbf4a1c89),
        1.0,
        1.0,
    ],
    [
        f32::from_bits(0x4077a3f0),
        f32::from_bits(0xbf4a1c89),
        1.0,
        1.0,
    ],
    [
        f32::from_bits(0x3fb9b7bb),
        f32::from_bits(0x3fd081e2),
        0.0,
        0.0,
    ],
    [
        f32::from_bits(0x4077a3f0),
        f32::from_bits(0x3fd081e2),
        1.0,
        0.0,
    ],
];

const POINT_SIZE: f32 = f32::from_bits(0x3f9ac813);
const CANVAS_SIZE: f32 = 256.0;

#[derive(Clone, Debug)]
struct Frames {
    /// Destination writes from draws 1 and 3; all are sparse pixel patches.
    /// Source remains caller-owned; no full 256x256 destination clone is made.
    draw1_patch: Vec<PixelWrite>,
    /// Chronological overlay; draw 3 replaces draw 1 at duplicate offsets.
    destination_patch: Vec<PixelWrite>,
    /// Actual post-draw1 destination SHA, computed by streaming source+patch.
    destination_after_draw1_sha256: String,
    /// Actual final destination SHA, computed by streaming source+patch.
    destination_sha256: String,
    /// Scratch texture3 after draw0 (128x128 RGBA8).
    scratch_start: Vec<u8>,
    /// Scratch texture2 after draw2 (128x128 RGBA8).
    scratch_feedback: Vec<u8>,
}

#[derive(Debug, Eq, PartialEq)]
enum Error {
    InvalidSource,
    InvalidQuad,
    DuplicatePatch,
    DuplicateDrawWrite,
}

#[derive(Clone, Copy, Debug)]
struct PixelWrite {
    byte_offset: usize,
    rgba: [u8; 4],
}

/// Exact V22 `_feedback_rgba8(..., "high-frequency")` byte generator.
fn high_frequency_input(width: usize, height: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(width * height * 4);
    for y in 0..height {
        for x in 0..width {
            out.extend_from_slice(&[
                ((x * 37 + y * 13 + ((x ^ y) & 1) * 127) & 255) as u8,
                ((x * 11 + y * 29 + ((x / 2 + y / 2) & 1) * 83) & 255) as u8,
                ((x * 17 + y * 7 + ((x + y) & 1) * 191) & 255) as u8,
                255,
            ]);
        }
    }
    out
}

/// Run draw0 -> draw1 -> draw2 -> draw3 from a caller-owned RGBA8 source.
/// Two 128x128 scratch surfaces and temporary per-draw writes are
/// allocated. Draw1's captured quad is below the viewport, so its rasterized
/// coverage is empty; no output pixel exception is used.
fn render(source: &[u8]) -> Result<Frames, Error> {
    render_with_quads(source, &QUAD_START, &QUAD_NEXT)
}

/// Run the captured passes using native-captured geometry. Geometry is an
/// input to rasterization, never a list of prescribed output pixels.
fn render_with_quads(
    source: &[u8],
    first_quad: &[[f32; 4]; 6],
    next_quad: &[[f32; 4]; 6],
) -> Result<Frames, Error> {
    if source.len() != WIDTH * HEIGHT * 4 {
        return Err(Error::InvalidSource);
    }
    let mut scratch_start = vec![0; SCRATCH * SCRATCH * 4];
    let p0 = draw_program4(source, &[], source, SCRATCH, SCRATCH, first_quad, true)?;
    apply_writes(&mut scratch_start, &p0)?;
    drop(p0);
    let p1 = draw_program5(&scratch_start, SCRATCH, WIDTH, HEIGHT, first_quad)?;
    let draw1_sha = hash_overlaid(source, &p1)?;
    // Draw 2 samples the destination as it stood after draw 1. Keep that
    // destination caller-owned and resolve the small draw-1 patch per texel.
    let p2 = draw_program4(
        source,
        &p1,
        &scratch_start,
        SCRATCH,
        SCRATCH,
        next_quad,
        false,
    )?;
    let mut scratch_feedback = vec![0; SCRATCH * SCRATCH * 4];
    apply_writes(&mut scratch_feedback, &p2)?;
    drop(p2);
    let draw3_patch = draw_program5(&scratch_feedback, SCRATCH, WIDTH, HEIGHT, next_quad)?;
    let destination_patch = merge_writes(&p1, &draw3_patch)?;
    let final_sha = hash_overlaid(source, &destination_patch)?;
    Ok(Frames {
        scratch_start,
        scratch_feedback,
        draw1_patch: p1,
        destination_patch,
        destination_after_draw1_sha256: draw1_sha,
        destination_sha256: final_sha,
    })
}

/// Sorted sparse overlay with later writes winning at shared pixels.
fn merge_writes(earlier: &[PixelWrite], later: &[PixelWrite]) -> Result<Vec<PixelWrite>, Error> {
    let mut writes = earlier.to_vec();
    writes.extend_from_slice(later);
    writes.sort_by_key(|w| w.byte_offset);
    let mut merged: Vec<PixelWrite> = Vec::with_capacity(writes.len());
    for write in writes {
        if write.byte_offset % 4 != 0 {
            return Err(Error::DuplicatePatch);
        }
        if let Some(previous) = merged.last_mut()
            && previous.byte_offset == write.byte_offset
        {
            *previous = write;
            continue;
        }
        merged.push(write);
    }
    Ok(merged)
}

fn apply_writes(target: &mut [u8], writes: &[PixelWrite]) -> Result<(), Error> {
    for w in writes {
        if w.byte_offset + 4 > target.len() {
            return Err(Error::InvalidSource);
        }
        target[w.byte_offset..w.byte_offset + 4].copy_from_slice(&w.rgba);
    }
    Ok(())
}
fn hash_overlaid(source: &[u8], writes: &[PixelWrite]) -> Result<String, Error> {
    let mut sorted = writes.to_vec();
    sorted.sort_by_key(|w| w.byte_offset);
    let mut h = Sha256::new();
    let mut cursor = 0;
    for w in sorted {
        if w.byte_offset % 4 != 0 || w.byte_offset + 4 > source.len() || w.byte_offset < cursor {
            return Err(Error::DuplicatePatch);
        }
        h.update(&source[cursor..w.byte_offset]);
        h.update(w.rgba);
        cursor = w.byte_offset + 4;
    }
    h.update(&source[cursor..]);
    Ok(format!("{:x}", h.finalize()))
}

#[derive(Clone, Copy)]
struct Vertex {
    sx: f32,
    sy: f32,
    inv_w: f32,
    canvas_u: f32,
    canvas_v: f32,
    brush_u: f32,
    brush_v: f32,
}

/// Mesa 25 llvmpipe triangle-plane setup for one perspective-correct scalar.
/// Operation order follows `lp_state_setup.c::init_args/calc_coef4`: compute
/// 1/area from screen positions, normalize edge deltas, then form a0/dadx/dady
/// with separate F32 mul/sub/add operations. Half-pixel center is subtracted
/// from vertex 0 before a0 setup; FS evaluation uses integer pixel coordinates
/// and chained FMA (x first, then y), as in the captured LLVM.
fn mesa_plane(p0: Vertex, p1: Vertex, p2: Vertex, v0: f32, v1: f32, v2: f32) -> [f32; 3] {
    let dxy01 = [p0.sx - p1.sx, p0.sy - p1.sy];
    let dxy20 = [p2.sx - p0.sx, p2.sy - p0.sy];
    let e = dxy01[0] * dxy20[1];
    let f = dxy01[1] * dxy20[0];
    let ooa = 1.0 / (e - f);
    let dxy20_ooa = [dxy20[0] * ooa, dxy20[1] * ooa];
    let dxy01_ooa = [dxy01[0] * ooa, dxy01[1] * ooa];
    let da01 = v0 - v1;
    let da20 = v2 - v0;
    let dadx = da01 * dxy20_ooa[1] - da20 * dxy01_ooa[1];
    let dady = da20 * dxy01_ooa[0] - da01 * dxy20_ooa[0];
    let dadx_x0 = dadx * (p0.sx - 0.5);
    let dady_y0 = dady * (p0.sy - 0.5);
    let attr_v0 = dadx_x0 + dady_y0;
    let a0 = v0 - attr_v0;
    [a0, dadx, dady]
}

#[inline]
fn mesa_plane_at(plane: [f32; 3], x: f32, y: f32) -> f32 {
    plane[2].mul_add(y, plane[1].mul_add(x, plane[0]))
}

fn vertex(a: [f32; 4], program4: bool, target_w: usize, target_h: usize) -> Vertex {
    let x = a[0];
    let y = a[1];
    // GLES column-major mat4 * vec4(x,y,0,1), retaining f32 op order.
    let clip_x = (PROJECTION[0] * x + PROJECTION[4] * y).mul_add(1.0, PROJECTION[12]);
    let clip_y = (PROJECTION[1] * x + PROJECTION[5] * y).mul_add(1.0, PROJECTION[13]);
    let clip_w = (PROJECTION[3] * x + PROJECTION[7] * y).mul_add(1.0, PROJECTION[15]);
    let (gx, gy, gw) = if program4 {
        (a[2] * 2.0 - 1.0, a[3] * 2.0 - 1.0, 1.0)
    } else {
        (clip_x, clip_y, clip_w)
    };
    let ndc_x = gx / gw;
    let ndc_y = gy / gw;
    let sx = ndc_x.mul_add(target_w as f32 * 0.5, target_w as f32 * 0.5);
    let sy = ndc_y.mul_add(target_h as f32 * 0.5, target_h as f32 * 0.5);
    let canvas_u = (clip_x / clip_w + 1.0) * 0.5;
    let canvas_v = (clip_y / clip_w + 1.0) * 0.5;
    Vertex {
        sx,
        sy,
        inv_w: 1.0 / gw,
        canvas_u,
        canvas_v,
        brush_u: a[2],
        brush_v: a[3],
    }
}

fn draw_program4(
    canvas: &[u8],
    canvas_overlay: &[PixelWrite],
    brush: &[u8],
    tw: usize,
    th: usize,
    quad: &[[f32; 4]; 6],
    is_start: bool,
) -> Result<Vec<PixelWrite>, Error> {
    raster(
        canvas,
        [WIDTH, HEIGHT],
        [tw, th],
        quad,
        true,
        |uv, canvas_uv| {
            if is_start {
                return sample_linear_overlay(
                    canvas,
                    canvas_overlay,
                    WIDTH,
                    HEIGHT,
                    canvas_uv[0],
                    canvas_uv[1],
                );
            }
            let d = distance(uv[0] - 0.5, uv[1] - 0.5);
            let mut alpha = 0.0_f32;
            let (canvas, brush) = if d < 0.5 {
                alpha = (1.0 - (d - 0.2).max(0.0) / 0.3) * 0.4;
                let c = four_tap_overlay(
                    canvas,
                    canvas_overlay,
                    WIDTH,
                    HEIGHT,
                    canvas_uv[0],
                    canvas_uv[1],
                    0.5 / CANVAS_SIZE,
                );
                let b = four_tap(brush, SCRATCH, SCRATCH, uv[0], uv[1], 0.5 / POINT_SIZE);
                (c, b)
            } else {
                (
                    sample_linear_overlay(
                        canvas,
                        canvas_overlay,
                        WIDTH,
                        HEIGHT,
                        canvas_uv[0],
                        canvas_uv[1],
                    ),
                    sample_linear(brush, SCRATCH, SCRATCH, uv[0], uv[1]),
                )
            };
            mix(canvas, brush, alpha)
        },
    )
}

fn draw_program5(
    source: &[u8],
    sw: usize,
    tw: usize,
    th: usize,
    quad: &[[f32; 4]; 6],
) -> Result<Vec<PixelWrite>, Error> {
    raster(source, [sw, sw], [tw, th], quad, false, |uv, _| {
        let d = distance(uv[0] - 0.5, uv[1] - 0.5);
        let brush = sample_linear(source, sw, sw, uv[0], uv[1]);
        let mut alpha = 1.0_f32;
        if d < 0.5 {
            alpha = ((d - 0.5) / 0.05 + 1.0).max(0.0);
            let blur = four_tap(source, sw, sw, uv[0], uv[1], 0.5 / POINT_SIZE);
            return mix(blur, brush, alpha);
        }
        mix([0.0; 4], brush, alpha)
    })
}

/// Rasterize the two captured triangles at framebuffer pixel centers. The
/// callback returns fragment RGBA. Captured factors are ONE,ZERO with ADD,
/// so covered fragments overwrite destination. Only the feedback program's
/// final mix narrows to binary16 RNE; the paint program keeps F32 output.
/// Both then assume nearest-even UNORM8 conversion under the captured host
/// profile. Texture samples use the observed F16RTZ narrowing.
fn raster<F>(
    source: &[u8],
    source_size: [usize; 2],
    target_size: [usize; 2],
    quad: &[[f32; 4]; 6],
    program4: bool,
    mut shade: F,
) -> Result<Vec<PixelWrite>, Error>
where
    F: FnMut([f32; 2], [f32; 2]) -> [f32; 4],
{
    let [sw, sh] = source_size;
    let [tw, th] = target_size;
    if source.len() != sw * sh * 4 || tw == 0 || th == 0 {
        return Err(Error::InvalidSource);
    }
    let mut writes = Vec::new();
    let vs: Vec<Vertex> = quad.iter().map(|a| vertex(*a, program4, tw, th)).collect();
    if vs
        .iter()
        .any(|v| !v.sx.is_finite() || !v.sy.is_finite() || !v.inv_w.is_finite())
    {
        return Err(Error::InvalidQuad);
    }
    for tri in [[0usize, 1usize, 2usize], [3, 4, 5]] {
        let p0 = vs[tri[0]];
        let p1 = vs[tri[1]];
        let p2 = vs[tri[2]];
        let inv_w = mesa_plane(p0, p1, p2, p0.inv_w, p1.inv_w, p2.inv_w);
        let varying_plane = |get: fn(Vertex) -> f32| {
            mesa_plane(
                p0,
                p1,
                p2,
                get(p0) * p0.inv_w,
                get(p1) * p1.inv_w,
                get(p2) * p2.inv_w,
            )
        };
        let brush_u = varying_plane(|v| v.brush_u);
        let brush_v = varying_plane(|v| v.brush_v);
        let canvas_u = varying_plane(|v| v.canvas_u);
        let canvas_v = varying_plane(|v| v.canvas_v);
        let a = p0;
        let mut b = vs[tri[1]];
        let mut c = vs[tri[2]];
        let area = orient(a, b, c.sx, c.sy);
        if area == 0.0 {
            continue;
        }
        if area < 0.0 {
            std::mem::swap(&mut b, &mut c);
        }
        let min_x = a.sx.min(b.sx).min(c.sx).floor().max(0.0) as usize;
        let max_x = a.sx.max(b.sx).max(c.sx).ceil().min(tw as f32) as usize;
        let min_y = a.sy.min(b.sy).min(c.sy).floor().max(0.0) as usize;
        let max_y = a.sy.max(b.sy).max(c.sy).ceil().min(th as f32) as usize;
        for y in min_y..max_y {
            for x in min_x..max_x {
                let px = x as f32 + 0.5;
                let py = y as f32 + 0.5;
                let w0 = orient(b, c, px, py);
                let w1 = orient(c, a, px, py);
                let w2 = orient(a, b, px, py);
                if !edge_owned(w0, b, c) || !edge_owned(w1, c, a) || !edge_owned(w2, a, b) {
                    continue;
                }
                // Interpolator coordinates are integer pixel indices here:
                // the triangle-plane a0 was established relative to the
                // half-pixel center, so FS FMA evaluation adds the center.
                let pixel_x = x as f32;
                let pixel_y = y as f32;
                let denom = mesa_plane_at(inv_w, pixel_x, pixel_y);
                let reciprocal = 1.0 / denom;
                let uv = [
                    mesa_plane_at(brush_u, pixel_x, pixel_y) * reciprocal,
                    mesa_plane_at(brush_v, pixel_x, pixel_y) * reciprocal,
                ];
                let cv = [
                    mesa_plane_at(canvas_u, pixel_x, pixel_y) * reciprocal,
                    mesa_plane_at(canvas_v, pixel_x, pixel_y) * reciprocal,
                ];
                let rgba = shade(uv, cv);
                let off = (y * tw + x) * 4;
                let narrow = |v: f32| if program4 { binary16_rne(v) } else { v };
                writes.push(PixelWrite {
                    byte_offset: off,
                    rgba: [
                        unorm8(narrow(rgba[0])),
                        unorm8(narrow(rgba[1])),
                        unorm8(narrow(rgba[2])),
                        unorm8(narrow(rgba[3])),
                    ],
                });
            }
        }
    }
    writes.sort_by_key(|w| w.byte_offset);
    if writes
        .windows(2)
        .any(|pair| pair[0].byte_offset == pair[1].byte_offset)
    {
        return Err(Error::DuplicateDrawWrite);
    }
    Ok(writes)
}

#[inline]
fn orient(a: Vertex, b: Vertex, x: f32, y: f32) -> f32 {
    (b.sx - a.sx) * (y - a.sy) - (b.sy - a.sy) * (x - a.sx)
}
#[inline]
fn edge_owned(value: f32, a: Vertex, b: Vertex) -> bool {
    if value > 0.0 {
        return true;
    }
    if value < 0.0 {
        return false;
    }
    // Lower-left ownership: include bottom edges directed right and left edges directed down.
    let dx = b.sx - a.sx;
    let dy = b.sy - a.sy;
    dy < 0.0 || (dy == 0.0 && dx > 0.0)
}
#[inline]
fn distance(x: f32, y: f32) -> f32 {
    (x * x + y * y).sqrt()
}
#[inline]
fn mix(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    [
        a[0] * (1.0 - t) + b[0] * t,
        a[1] * (1.0 - t) + b[1] * t,
        a[2] * (1.0 - t) + b[2] * t,
        a[3] * (1.0 - t) + b[3] * t,
    ]
}
fn four_tap(src: &[u8], w: usize, h: usize, u: f32, v: f32, offset: f32) -> [f32; 4] {
    let a = sample_linear(src, w, h, u - offset, v - offset);
    let b = sample_linear(src, w, h, u - offset, v + offset);
    let c = sample_linear(src, w, h, u + offset, v - offset);
    let d = sample_linear(src, w, h, u + offset, v + offset);
    [
        (a[0] + b[0] + c[0] + d[0]) / 4.0,
        (a[1] + b[1] + c[1] + d[1]) / 4.0,
        (a[2] + b[2] + c[2] + d[2]) / 4.0,
        (a[3] + b[3] + c[3] + d[3]) / 4.0,
    ]
}

fn sample_linear_overlay(
    bytes: &[u8],
    overlay: &[PixelWrite],
    w: usize,
    h: usize,
    u: f32,
    v: f32,
) -> [f32; 4] {
    if overlay.is_empty() {
        return sample_linear(bytes, w, h, u, v);
    }
    let (x0, x1, fx) = sampler_axis(u, w);
    let (y0, y1, fy) = sampler_axis(v, h);
    let mut out = [0.0; 4];
    for (c, channel) in out.iter_mut().enumerate() {
        let p00 = overlay_byte(bytes, overlay, (y0 * w + x0) * 4 + c);
        let p10 = overlay_byte(bytes, overlay, (y0 * w + x1) * 4 + c);
        let p01 = overlay_byte(bytes, overlay, (y1 * w + x0) * 4 + c);
        let p11 = overlay_byte(bytes, overlay, (y1 * w + x1) * 4 + c);
        let filtered = lerp8(lerp8(p00, p10, fx), lerp8(p01, p11, fx), fy);
        *channel = binary16_rtz((filtered as f32) * f32::from_bits(0x3b80_8081));
    }
    out
}

#[inline]
fn overlay_byte(bytes: &[u8], overlay: &[PixelWrite], offset: usize) -> u8 {
    let pixel_offset = offset & !3;
    match overlay.binary_search_by_key(&pixel_offset, |w| w.byte_offset) {
        Ok(index) => overlay[index].rgba[offset - pixel_offset],
        Err(_) => bytes[offset],
    }
}

fn four_tap_overlay(
    src: &[u8],
    overlay: &[PixelWrite],
    w: usize,
    h: usize,
    u: f32,
    v: f32,
    offset: f32,
) -> [f32; 4] {
    let a = sample_linear_overlay(src, overlay, w, h, u - offset, v - offset);
    let b = sample_linear_overlay(src, overlay, w, h, u - offset, v + offset);
    let c = sample_linear_overlay(src, overlay, w, h, u + offset, v - offset);
    let d = sample_linear_overlay(src, overlay, w, h, u + offset, v + offset);
    [
        (a[0] + b[0] + c[0] + d[0]) / 4.0,
        (a[1] + b[1] + c[1] + d[1]) / 4.0,
        (a[2] + b[2] + c[2] + d[2]) / 4.0,
        (a[3] + b[3] + c[3] + d[3]) / 4.0,
    ]
}

/// Mesa's bounded llvmpipe RGBA8 linear sampler path. Quantize the normalized
/// coordinate to 8 fractional bits with ties-to-even; subtract the half-texel
/// offset, clamp the two texel indices, then do integer X and Y lerps. The
/// final byte converts using the captured 0x3b808081 reciprocal and the lowp
/// sampler's F16RTZ result conversion shown by active Smudge NIR/LLVM.
fn sample_linear(bytes: &[u8], w: usize, h: usize, u: f32, v: f32) -> [f32; 4] {
    let (x0, x1, fx) = sampler_axis(u, w);
    let (y0, y1, fy) = sampler_axis(v, h);
    let mut out = [0.0; 4];
    for c in 0..4 {
        let p00 = bytes[(y0 * w + x0) * 4 + c];
        let p10 = bytes[(y0 * w + x1) * 4 + c];
        let p01 = bytes[(y1 * w + x0) * 4 + c];
        let p11 = bytes[(y1 * w + x1) * 4 + c];
        let top = lerp8(p00, p10, fx);
        let bottom = lerp8(p01, p11, fx);
        let filtered = lerp8(top, bottom, fy);
        let normalized = (filtered as f32) * f32::from_bits(0x3b80_8081);
        out[c] = binary16_rtz(normalized);
    }
    out
}

/// q = RN-even(f32(uv * (N << 8))) - 128. Arithmetic right shift gives
/// i0; the low eight bits are the interpolation weight. Clamping both integer
/// coordinates implements GL_CLAMP_TO_EDGE without perturbing q's fraction.
fn sampler_axis(uv: f32, size: usize) -> (usize, usize, i32) {
    let scale = (size << 8) as f32;
    let q = (uv * scale).round_ties_even() as i32 - 128;
    let i0 = q >> 8;
    let frac = q & 255;
    let lo = i0.clamp(0, size as i32 - 1) as usize;
    let hi = (i0 + 1).clamp(0, size as i32 - 1) as usize;
    (lo, hi, frac)
}

#[inline]
fn lerp8(a: u8, b: u8, frac: i32) -> u8 {
    let delta = (frac * (i32::from(b) - i32::from(a)) + 128) >> 8;
    (i32::from(a) + delta).clamp(0, 255) as u8
}

/// NIR lowers active Smudge sampler results to binary16; LLVM uses
/// vcvtps2ph.256 immediate 3, i.e. round toward zero. Values are normalized
/// bytes in [0,1], so the positive normal-only conversion suffices.
fn binary16_rtz(x: f32) -> f32 {
    if x == 0.0 || x == 1.0 || !x.is_finite() {
        return x;
    }
    let bits = x.to_bits();
    let exp = ((bits >> 23) & 255) as i32;
    let mant = bits & 0x7f_ffff;
    if exp == 0 || x < 0.0 {
        return x;
    }
    let he = exp - 127 + 15;
    if he <= 0 || he >= 31 {
        return x;
    }
    let hb = ((he as u32) << 10) | (mant >> 13);
    let e = ((hb >> 10) & 31) as i32 - 15 + 127;
    let m = (hb & 1023) << 13;
    f32::from_bits(((e as u32) << 23) | m)
}

#[inline]
fn unorm8(x: f32) -> u8 {
    (x.clamp(0.0, 1.0) * 255.0).round_ties_even() as u8
}

fn binary16_rne(x: f32) -> f32 {
    if x == 0.0 || !x.is_finite() {
        return x;
    }
    let bits = x.to_bits();
    let sign = bits & 0x8000_0000;
    let exp = ((bits >> 23) & 255) as i32;
    let mant = bits & 0x7f_ffff;
    if exp == 0 {
        return x;
    }
    let he = exp - 127 + 15;
    if he <= 0 {
        return 0.0_f32.copysign(x);
    }
    if he >= 31 {
        return f32::INFINITY.copysign(x);
    }
    let mut hb = ((he as u32) << 10) | (mant >> 13);
    let rem = mant & 0x1fff;
    if rem > 0x1000 || (rem == 0x1000 && (hb & 1) != 0) {
        hb += 1;
    }
    if hb & 0x7c00 == 0x7c00 {
        return f32::INFINITY.copysign(x);
    }
    let e = ((hb >> 10) & 0x1f) as i32 - 15 + 127;
    let m = (hb & 0x03ff) << 13;
    f32::from_bits(sign | ((e as u32) << 23) | m)
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    // These are post-hoc native readback hashes only. No readback bytes are
    // loaded or fed into render(). If a stage differs, retain the failure.
    const INPUT_SHA: &str = "fff4fb36d37d47efdf75673ab9d8e844c406aebd19791a5ddc27b16a457ecc8a";
    const DRAW0_SHA: &str = "23bb17fcc69a3ea6cef1a602196adf5c79e0a413c85113f230e170866653f308";
    const DRAW1_SHA: &str = "fff4fb36d37d47efdf75673ab9d8e844c406aebd19791a5ddc27b16a457ecc8a";
    const DRAW2_SHA: &str = "859333655cac79883d8b8fa21c640e8bbb9a8f42072ec60f959576549f26a1cc";
    const DRAW3_SHA: &str = "2bbb8a791f65101cf83749e1b828af34165e8a97db0fcced6d26826fa8c5e250";
    const INTERIOR_FIRST: [[f32; 4]; 6] = [
        [
            f32::from_bits(0x427f29bf),
            f32::from_bits(0x427329bf),
            0.0,
            1.0,
        ],
        [
            f32::from_bits(0x427f29bf),
            f32::from_bits(0x427cd641),
            0.0,
            0.0,
        ],
        [
            f32::from_bits(0x42846b20),
            f32::from_bits(0x427329bf),
            1.0,
            1.0,
        ],
        [
            f32::from_bits(0x42846b20),
            f32::from_bits(0x427329bf),
            1.0,
            1.0,
        ],
        [
            f32::from_bits(0x427f29bf),
            f32::from_bits(0x427cd641),
            0.0,
            0.0,
        ],
        [
            f32::from_bits(0x42846b20),
            f32::from_bits(0x427cd641),
            1.0,
            0.0,
        ],
    ];
    const INTERIOR_NEXT: [[f32; 4]; 6] = [
        [
            f32::from_bits(0x4282e6df),
            f32::from_bits(0x427cd78d),
            0.0,
            1.0,
        ],
        [
            f32::from_bits(0x4282e6df),
            f32::from_bits(0x42834207),
            0.0,
            0.0,
        ],
        [
            f32::from_bits(0x4287bd1f),
            f32::from_bits(0x427cd78d),
            1.0,
            1.0,
        ],
        [
            f32::from_bits(0x4287bd1f),
            f32::from_bits(0x427cd78d),
            1.0,
            1.0,
        ],
        [
            f32::from_bits(0x4282e6df),
            f32::from_bits(0x42834207),
            0.0,
            0.0,
        ],
        [
            f32::from_bits(0x4287bd1f),
            f32::from_bits(0x42834207),
            1.0,
            0.0,
        ],
    ];

    #[test]
    fn captured_high_frequency_source_pipeline_stage_hashes() {
        let source = high_frequency_input(WIDTH, HEIGHT);
        assert_eq!(
            sha256(&source),
            INPUT_SHA,
            "generated source-input fixture drift"
        );
        for (quad, hash) in [
            (
                &QUAD_START,
                "e5825c9e20fc003bc25cbd35827400dbda716f3556828bf34f759b764edb7d22",
            ),
            (
                &QUAD_NEXT,
                "3bae4eda1ecc6926c1ba6edb124f8e21d28811ae6f675cb5549b73881630f665",
            ),
        ] {
            let raw: Vec<u8> = quad
                .iter()
                .flatten()
                .flat_map(|lane| lane.to_le_bytes())
                .collect();
            assert_eq!(sha256(&raw), hash, "native input quad bytes");
        }
        let got = render(&source).expect("captured source geometry");
        assert!(
            got.draw1_patch.is_empty(),
            "baseline geometry has no draw-1 coverage"
        );
        let changed = got
            .destination_patch
            .iter()
            .filter(|w| &source[w.byte_offset..w.byte_offset + 4] != w.rgba.as_slice())
            .count();
        assert_eq!(changed, 6, "native changed-pixel count");
        assert_eq!(
            sha256(&got.scratch_start),
            DRAW0_SHA,
            "draw0 source->128 scratch mismatch"
        );
        assert_eq!(
            got.destination_sha256, DRAW3_SHA,
            "draw3 final destination mismatch"
        );
        // Draw1 is geometrically below the 256x256 viewport. This checks the
        // computed post-draw frame, not an output-pixel exception.
        assert_eq!(
            got.destination_after_draw1_sha256, DRAW1_SHA,
            "draw1 destination mismatch"
        );
        assert_eq!(
            sha256(&got.scratch_feedback),
            DRAW2_SHA,
            "draw2 feedback scratch mismatch"
        );
    }

    #[test]
    fn translated_interior_control_stage_hashes_and_sparse_chronology() {
        let source = high_frequency_input(WIDTH, HEIGHT);
        for (quad, expected) in [
            (
                &INTERIOR_FIRST,
                "630039da471df0d1443e23cf628249d9a38ded480ec137a8d7557df4b397d128",
            ),
            (
                &INTERIOR_NEXT,
                "9f57182ebbb3561093fac07a6a5d1d17f5bdf82df1d772ba06681254aadd06bc",
            ),
        ] {
            let raw: Vec<u8> = quad
                .iter()
                .flatten()
                .flat_map(|lane| lane.to_le_bytes())
                .collect();
            assert_eq!(sha256(&raw), expected, "V30 native VBO geometry input");
        }
        let got = render_with_quads(&source, &INTERIOR_FIRST, &INTERIOR_NEXT)
            .expect("native-captured translated geometry");
        assert_eq!(
            sha256(&got.scratch_start),
            "7af292f05f8fe36e7f40db6aadf7c363c7abe2902a289712f5a7169229c445c1",
            "draw0"
        );
        assert_eq!(
            got.destination_after_draw1_sha256,
            "22fcba7b531c82e814451332b4f24df7bacf0ef5dd07e47852ff26b60a8b6935",
            "draw1"
        );
        assert_eq!(
            sha256(&got.scratch_feedback),
            "08fb1fcaa0ca8caf2c2900e13924d6fc49f53a7074a15375812cc7373423e5f4",
            "draw2"
        );
        assert_eq!(
            got.destination_sha256,
            "e85b8003770b441b036734f7a554f68c0ad3b95566707a3e499f7fdc7beaa63e",
            "draw3"
        );
        let changed_after_draw1 = got
            .draw1_patch
            .iter()
            .filter(|w| source[w.byte_offset..w.byte_offset + 4] != w.rgba)
            .count();
        let changed_after_draw3 = got
            .destination_patch
            .iter()
            .filter(|w| source[w.byte_offset..w.byte_offset + 4] != w.rgba)
            .count();
        assert_eq!(
            changed_after_draw1, 4,
            "computed interior draw-1 pixel changes"
        );
        assert_eq!(changed_after_draw3, 13, "native final changed-pixel count");
    }

    #[test]
    fn transparent_source_matches_native_four_stage_control() {
        let source = vec![0; WIDTH * HEIGHT * 4];
        let frame = render(&source).expect("captured transparent source geometry");
        let scratch_sha = "de2f256064a0af797747c2b97505dc0b9f3df0de4f489eac731c23ae9ca9cc31";
        let target_sha = "8a39d2abd3999ab73c34db2476849cddf303ce389b35826850f9a700589b4a90";
        assert_eq!(sha256(&frame.scratch_start), scratch_sha);
        assert_eq!(frame.destination_after_draw1_sha256, target_sha);
        assert_eq!(sha256(&frame.scratch_feedback), scratch_sha);
        assert_eq!(frame.destination_sha256, target_sha);
    }
}
