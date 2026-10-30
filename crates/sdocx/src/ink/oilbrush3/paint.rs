//! Bounded source-driven OilBrush3 host paint.

use crate::ink::host_rgba8::{over as host_over, quantize as quant};

const FEEDBACK: usize = 16;
const MASK_W: usize = 16;
const MASK_H: usize = 16;
const DEPTH_W: usize = 297;
const DEPTH_H: usize = 372;
const ADDPOINT_STRIDE: usize = 112;
const P3_VBO: &[u8] = include_bytes!("p3-vbo-6x4-f32le.bin");

#[derive(Clone, Copy, Default, PartialEq, Eq)]
struct Rgba([u8; 4]);

#[derive(Clone, Copy, Debug)]
struct Plane([f32; 3]);

#[derive(Clone, Copy, Debug)]
struct P4Vertex {
    x: f32,
    y: f32,
    mask_uv: [f32; 2],
    depth_uv: [f32; 2],
}

#[derive(Clone, Copy, Debug)]
struct P4Triangle {
    pos: [[f32; 2]; 3],
    mask_u: Plane,
    mask_v: Plane,
    depth_u: Plane,
    depth_v: Plane,
}

#[derive(Clone, Copy, Debug)]
struct P3State {
    projection: [f32; 16],
    model: [f32; 16],
    depth_scale: [f32; 2],
    ratio: f32,
}

#[derive(Clone, Copy, Debug)]
struct PageSize {
    width: usize,
    height: usize,
}

#[derive(Clone, Copy, Debug)]
struct P4State {
    projection: [f32; 16],
    model: [f32; 16],
    depth_scale: [f32; 2],
}

fn read_f32(bytes: &[u8], offset: usize) -> f32 {
    f32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("four-byte F32"))
}

fn fadd(a: f32, b: f32) -> f32 {
    a + b
}
fn fmul(a: f32, b: f32) -> f32 {
    a * b
}
fn fdiv(a: f32, b: f32) -> f32 {
    a / b
}
fn ffma(a: f32, b: f32, c: f32) -> f32 {
    ((f64::from(a) * f64::from(b)) + f64::from(c)) as f32
}
// Captured P4 state uses FUNC_ADD with ONE / ONE_MINUS_SRC_ALPHA for color
// and alpha on RGBA8 attachments. Quantize the shader result first, then model
// the normalized-integer framebuffer blend from Mesa 25.0.7 lp_build_mul_norm.
// This common driver path has captured Crayon2 IR corroboration; OilBrush3's
// shader-only IR does not independently establish its active blend lowering.
fn rgba8_source_over(src: [u8; 4], dst: [u8; 4]) -> [u8; 4] {
    host_over(dst, src)
}

fn mat_vec(m: &[f32; 16], v: [f32; 4], translation_first: bool) -> [f32; 4] {
    let mut out = [0.0; 4];
    for row in 0..4 {
        let c0 = fmul(m[row], v[0]);
        let c1 = fmul(m[4 + row], v[1]);
        let c2 = fmul(m[8 + row], v[2]);
        let c3 = fmul(m[12 + row], v[3]);
        out[row] = if translation_first {
            fadd(fadd(fadd(c0, c3), c1), c2)
        } else {
            fadd(fadd(fadd(c0, c1), c2), c3)
        };
    }
    out
}

fn mesa_plane(pos: [[f32; 2]; 3], values: [f32; 3]) -> Plane {
    let [[x0, y0], [x1, y1], [x2, y2]] = pos;
    let [a0, a1, a2] = values;
    let x0c = (f64::from(x0) - 0.5) as f32;
    let y0c = (f64::from(y0) - 0.5) as f32;
    let dx01 = (f64::from(x0) - f64::from(x1)) as f32;
    let dy01 = (f64::from(y0) - f64::from(y1)) as f32;
    let dx20 = (f64::from(x2) - f64::from(x0)) as f32;
    let dy20 = (f64::from(y2) - f64::from(y0)) as f32;
    let e = fmul(dx01, dy20);
    let f = fmul(dy01, dx20);
    let area = (f64::from(e) - f64::from(f)) as f32;
    let ooa = fdiv(1.0, area);
    let dy20_ooa = fmul(dy20, ooa);
    let dy01_ooa = fmul(dy01, ooa);
    let dx20_ooa = fmul(dx20, ooa);
    let dx01_ooa = fmul(dx01, ooa);
    let da01 = (f64::from(a0) - f64::from(a1)) as f32;
    let da20 = (f64::from(a2) - f64::from(a0)) as f32;
    let dx_l = fmul(da01, dy20_ooa);
    let dx_r = fmul(da20, dy01_ooa);
    let dy_l = fmul(da20, dx01_ooa);
    let dy_r = fmul(da01, dx20_ooa);
    let dadx = (f64::from(dx_l) - f64::from(dx_r)) as f32;
    let dady = (f64::from(dy_l) - f64::from(dy_r)) as f32;
    let origin = (f64::from(a0) - f64::from(fadd(fmul(dadx, x0c), fmul(dady, y0c)))) as f32;
    Plane([origin, dadx, dady])
}

fn eval_plane(p: Plane, x: usize, y: usize) -> f32 {
    ffma(p.0[1], x as f32, ffma(p.0[2], y as f32, p.0[0]))
}

fn inside_triangle(p: [f64; 2], tri: [[f32; 2]; 3]) -> Option<[f64; 3]> {
    let [[ax, ay], [bx, by], [cx, cy]] = tri;
    let (ax, ay, bx, by, cx, cy) = (
        f64::from(ax),
        f64::from(ay),
        f64::from(bx),
        f64::from(by),
        f64::from(cx),
        f64::from(cy),
    );
    let den = (by - cy) * (ax - cx) + (cx - bx) * (ay - cy);
    if den.abs() < 1.0e-12 {
        return None;
    }
    let a = ((by - cy) * (p[0] - cx) + (cx - bx) * (p[1] - cy)) / den;
    let b = ((cy - ay) * (p[0] - cx) + (ax - cx) * (p[1] - cy)) / den;
    let c = 1.0 - a - b;
    if a.min(b).min(c) < -1.0e-7 {
        None
    } else {
        Some([a, b, c])
    }
}

const UNORM8_TO_F32: f32 = f32::from_bits(0x3b80_8081);

fn unorm8_half_rtz(value: u8) -> f32 {
    let normalized = f32::from(value) * UNORM8_TO_F32;
    if normalized == 0.0 {
        return 0.0;
    }
    let bits = normalized.to_bits();
    let exponent = ((bits >> 23) & 0xff) as i32 - 127;
    debug_assert!((-14..=0).contains(&exponent), "UNORM8 sample is normal F16");
    let half = (((exponent + 15) as u16) << 10) | ((bits >> 13) as u16 & 0x03ff);
    f16_to_f32(half)
}

fn f16_to_f32(bits: u16) -> f32 {
    let sign = (u32::from(bits & 0x8000)) << 16;
    let exponent = i32::from((bits >> 10) & 0x1f);
    let fraction = u32::from(bits & 0x03ff);
    let result = match exponent {
        0 if fraction == 0 => sign,
        0 => {
            let mut significand = fraction;
            let mut unbiased = -14;
            while significand & 0x0400 == 0 {
                significand <<= 1;
                unbiased -= 1;
            }
            significand &= 0x03ff;
            sign | (((unbiased + 127) as u32) << 23) | (significand << 13)
        }
        0x1f => sign | 0x7f80_0000 | (fraction << 13),
        _ => sign | (((exponent - 15 + 127) as u32) << 23) | (fraction << 13),
    };
    f32::from_bits(result)
}

fn nearest_index(coord: f32, size: usize, repeat: bool) -> usize {
    let coord = if repeat { coord.rem_euclid(1.0) } else { coord };
    ((coord * size as f32).floor() as isize).clamp(0, size as isize - 1) as usize
}

fn sample_nearest_r8_lp(tex: &[u8], w: usize, h: usize, u: f32, v: f32, repeat: bool) -> f32 {
    let x = nearest_index(u, w, repeat);
    let y = nearest_index(v, h, repeat);
    unorm8_half_rtz(tex[y * w + x])
}

fn sample_nearest_rgba_lp(tex: &[Rgba], w: usize, h: usize, u: f32, v: f32) -> [f32; 4] {
    let x = nearest_index(u, w, false);
    let y = nearest_index(v, h, false);
    tex[y * w + x].0.map(unorm8_half_rtz)
}

fn cvtps2dq_default(value: f32) -> i32 {
    // The captured Gallivm lowering is CVTPS2DQ, whose rounding follows MXCSR.
    value.round_ties_even() as i32
}

fn sampler_axis(coord: f32, size: usize) -> (isize, i32) {
    let scale = (size << 8) as f32;
    let fixed = cvtps2dq_default(coord * scale) - 128;
    ((fixed >> 8) as isize, fixed & 0xff)
}

fn sampler_index(index: isize, size: usize, repeat: bool) -> usize {
    if repeat {
        index.rem_euclid(size as isize) as usize
    } else {
        index.clamp(0, size as isize - 1) as usize
    }
}

// Gallivm emits PMULHRSW(frac, delta << 7), then adds the byte base and
// packs unsigned; integer form is base + ((frac*delta + 128) >> 8).
fn sampler_lerp_u8(a: u8, b: u8, fraction: i32) -> u8 {
    let delta = i32::from(b) - i32::from(a);
    (i32::from(a) + ((fraction * delta + 128) >> 8)).clamp(0, 255) as u8
}

fn sample_linear_r8_lp(tex: &[u8], w: usize, h: usize, u: f32, v: f32, repeat: bool) -> f32 {
    let (x0, fx) = sampler_axis(u, w);
    let (y0, fy) = sampler_axis(v, h);
    let at =
        |x: isize, y: isize| tex[sampler_index(y, h, repeat) * w + sampler_index(x, w, repeat)];
    let top = sampler_lerp_u8(at(x0, y0), at(x0 + 1, y0), fx);
    let bottom = sampler_lerp_u8(at(x0, y0 + 1), at(x0 + 1, y0 + 1), fx);
    unorm8_half_rtz(sampler_lerp_u8(top, bottom, fy))
}

fn sample_linear_rgba_lp(tex: &[Rgba], w: usize, h: usize, u: f32, v: f32) -> [f32; 4] {
    let (x0, fx) = sampler_axis(u, w);
    let (y0, fy) = sampler_axis(v, h);
    std::array::from_fn(|channel| {
        let at = |x: isize, y: isize| {
            tex[sampler_index(y, h, false) * w + sampler_index(x, w, false)].0[channel]
        };
        let top = sampler_lerp_u8(at(x0, y0), at(x0 + 1, y0), fx);
        let bottom = sampler_lerp_u8(at(x0, y0 + 1), at(x0 + 1, y0 + 1), fx);
        unorm8_half_rtz(sampler_lerp_u8(top, bottom, fy))
    })
}

fn model_from_addpoint(record: &[u8]) -> [f32; 16] {
    let x = read_f32(record, 96);
    let y = read_f32(record, 100);
    let coefficient = read_f32(record, 104);
    let angle = read_f32(record, 108);
    let (sin, cos) = if angle >= 5.0 {
        (0.0, 1.0)
    } else {
        (2.0 * angle).sin_cos()
    };
    let diagonal = coefficient * cos;
    let off_diagonal = -coefficient * sin;
    [
        1.0 - diagonal,
        off_diagonal,
        0.0,
        0.0,
        off_diagonal,
        1.0 + diagonal,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
        x,
        y,
        0.0,
        1.0,
    ]
}

fn unpack_p4_triangle_with_records(
    stamp: usize,
    tri_indexes: [usize; 3],
    page: PageSize,
    state: P4State,
    records: &[u8],
) -> P4Triangle {
    let model = state.model;
    let proj = state.projection;
    let depth_scale = state.depth_scale;
    let record = &records[stamp * ADDPOINT_STRIDE..(stamp + 1) * ADDPOINT_STRIDE];
    let draw_params = [
        read_f32(record, 96),
        read_f32(record, 100),
        read_f32(record, 104),
        read_f32(record, 108),
    ];
    assert_eq!(model[12].to_bits(), draw_params[0].to_bits());
    assert_eq!(model[13].to_bits(), draw_params[1].to_bits());
    let mut vertices = [P4Vertex {
        x: 0.0,
        y: 0.0,
        mask_uv: [0.0; 2],
        depth_uv: [0.0; 2],
    }; 6];
    for (row, vertex) in vertices.iter_mut().enumerate() {
        let o = stamp * ADDPOINT_STRIDE + row * 16;
        let x = read_f32(records, o);
        let y = read_f32(records, o + 4);
        let u = read_f32(records, o + 8);
        let v = read_f32(records, o + 12);
        let world = mat_vec(&model, [x, y, 0.0, 1.0], true);
        let clip = mat_vec(&proj, world, false);
        let uv = [
            fmul(fadd(fdiv(clip[0], clip[3]), 1.0), 0.5),
            fmul(fadd(fdiv(clip[1], clip[3]), 1.0), 0.5),
        ];
        let inv_w = fdiv(1.0, clip[3]);
        let half_width = (page.width as f64 * 0.5) as f32;
        let half_height = (page.height as f64 * 0.5) as f32;
        let wx = fadd(fmul(fmul(clip[0], inv_w), half_width), half_width);
        let wy = fadd(fmul(fmul(clip[1], inv_w), half_height), half_height);
        *vertex = P4Vertex {
            x: wx,
            y: wy,
            mask_uv: [u, v],
            depth_uv: [fmul(uv[0], depth_scale[0]), fmul(uv[1], depth_scale[1])],
        };
    }
    let vs = tri_indexes.map(|i| vertices[i]);
    let pos = vs.map(|v| [v.x, v.y]);
    let mask_u = mesa_plane(pos, vs.map(|v| v.mask_uv[0]));
    let mask_v = mesa_plane(pos, vs.map(|v| v.mask_uv[1]));
    let depth_u = mesa_plane(pos, vs.map(|v| v.depth_uv[0]));
    let depth_v = mesa_plane(pos, vs.map(|v| v.depth_uv[1]));
    P4Triangle {
        pos,
        mask_u,
        mask_v,
        depth_u,
        depth_v,
    }
}

struct P4Textures<'a> {
    brush: &'a [Rgba],
    mask: &'a [u8],
    depth: &'a [u8],
}

fn render_p4_with_records(
    canvas: &mut [Rgba],
    page: PageSize,
    stamp: usize,
    state: P4State,
    textures: P4Textures<'_>,
    records: &[u8],
) -> usize {
    let P4Textures { brush, mask, depth } = textures;
    assert_eq!(canvas.len(), page.width * page.height);
    let mut touched = std::collections::BTreeSet::new();
    for ids in [[0, 1, 2], [3, 4, 5]] {
        let tri = unpack_p4_triangle_with_records(stamp, ids, page, state, records);
        let min_x = ((tri
            .pos
            .iter()
            .map(|p| f64::from(p[0]))
            .fold(f64::INFINITY, f64::min)
            - 0.5)
            .floor() as isize)
            .max(0);
        let max_x = ((tri
            .pos
            .iter()
            .map(|p| f64::from(p[0]))
            .fold(f64::NEG_INFINITY, f64::max)
            - 0.5)
            .ceil() as isize)
            .min(page.width as isize - 1);
        let min_y = ((tri
            .pos
            .iter()
            .map(|p| f64::from(p[1]))
            .fold(f64::INFINITY, f64::min)
            - 0.5)
            .floor() as isize)
            .max(0);
        let max_y = ((tri
            .pos
            .iter()
            .map(|p| f64::from(p[1]))
            .fold(f64::NEG_INFINITY, f64::max)
            - 0.5)
            .ceil() as isize)
            .min(page.height as isize - 1);
        if min_x > max_x || min_y > max_y {
            continue;
        }
        for y in min_y as usize..=max_y as usize {
            for x in min_x as usize..=max_x as usize {
                let index = y * page.width + x;
                if touched.contains(&index) {
                    continue;
                }
                if inside_triangle([x as f64 + 0.5, y as f64 + 0.5], tri.pos).is_none() {
                    continue;
                }
                touched.insert(index);
                let mu = eval_plane(tri.mask_u, x, y);
                let mv = eval_plane(tri.mask_v, x, y);
                let du = eval_plane(tri.depth_u, x, y);
                let dv = eval_plane(tri.depth_v, x, y);
                let src = sample_nearest_rgba_lp(brush, FEEDBACK, FEEDBACK, mu, mv);
                let mask_r = sample_nearest_r8_lp(mask, MASK_W, MASK_H, mu, mv, false);
                let depth_r = sample_linear_r8_lp(depth, DEPTH_W, DEPTH_H, du, dv, true);
                // P4 NIR multiplies sampled brush by depth first, then by mask.
                let alpha = fmul(fmul(src[3], depth_r), mask_r);
                let rgb: [f32; 3] = std::array::from_fn(|c| fmul(fmul(src[c], depth_r), mask_r));
                // This host converts the fragment source to RGBA8 before
                // the fixed-function normalized-integer blend with its RGBA8 dst.
                let src = std::array::from_fn(|channel| {
                    quant(if channel == 3 { alpha } else { rgb[channel] })
                });
                canvas[index] = Rgba(rgba8_source_over(src, canvas[index].0));
            }
        }
    }
    touched.len()
}

fn render_p3_for_page(
    canvas: &[Rgba],
    page: PageSize,
    params: P3State,
    mask: &[u8],
    depth: &[u8],
    brush: &[Rgba],
) -> Vec<Rgba> {
    assert_eq!(canvas.len(), page.width * page.height);
    let mut out = vec![Rgba::default(); FEEDBACK * FEEDBACK];
    let mut vertices = [[0.0_f64; 4]; 6];
    for (row, v) in vertices.iter_mut().enumerate() {
        let o = row * 16;
        let x = read_f32(P3_VBO, o);
        let y = read_f32(P3_VBO, o + 4);
        let u = read_f32(P3_VBO, o + 8);
        let vv = read_f32(P3_VBO, o + 12);
        let world = mat_vec(&params.model, [x, y, 0.0, 1.0], true);
        let clip = mat_vec(&params.projection, world, false);
        let canvas_uv = [
            fmul(fadd(fdiv(clip[0], clip[3]), 1.0), 0.5),
            fmul(fadd(fdiv(clip[1], clip[3]), 1.0), 0.5),
        ];
        *v = [
            f64::from(fmul(u, FEEDBACK as f32)),
            f64::from(fmul(vv, FEEDBACK as f32)),
            f64::from(canvas_uv[0]),
            f64::from(canvas_uv[1]),
        ];
    }
    for ids in [[0, 1, 2], [3, 4, 5]] {
        let tri = ids.map(|i| vertices[i]);
        let pos = tri.map(|v| [v[0] as f32, v[1] as f32]);
        for y in 0..FEEDBACK {
            for x in 0..FEEDBACK {
                let index = y * FEEDBACK + x;
                if out[index] != Rgba::default() {
                    continue;
                }
                let Some(weights) = inside_triangle([x as f64 + 0.5, y as f64 + 0.5], pos) else {
                    continue;
                };
                let lerp = |lane: usize| -> f64 {
                    (weights[0] * tri[0][lane] + weights[1] * tri[1][lane])
                        + weights[2] * tri[2][lane]
                };
                let (cu, cv) = (lerp(2) as f32, lerp(3) as f32);
                // Brush/mask coordinates are the untransformed attribute ZW lanes.
                let bu = (weights[0] * f64::from(read_f32(P3_VBO, ids[0] * 16 + 8))
                    + weights[1] * f64::from(read_f32(P3_VBO, ids[1] * 16 + 8))
                    + weights[2] * f64::from(read_f32(P3_VBO, ids[2] * 16 + 8)))
                    as f32;
                let bv = (weights[0] * f64::from(read_f32(P3_VBO, ids[0] * 16 + 12))
                    + weights[1] * f64::from(read_f32(P3_VBO, ids[1] * 16 + 12))
                    + weights[2] * f64::from(read_f32(P3_VBO, ids[2] * 16 + 12)))
                    as f32;
                let cr = sample_linear_rgba_lp(canvas, page.width, page.height, cu, cv);
                let mu = sample_nearest_r8_lp(mask, MASK_W, MASK_H, bu, bv, false);
                let du = fmul(cu, params.depth_scale[0]);
                let dv = fmul(cv, params.depth_scale[1]);
                let dr = sample_linear_r8_lp(depth, DEPTH_W, DEPTH_H, du, dv, true);
                let b = fmul(fmul(mu, params.ratio), dr);
                let ca: [f32; 4] = cr.map(|c| fmul(c, b));
                let br = sample_nearest_rgba_lp(brush, FEEDBACK, FEEDBACK, bu, bv);
                let alpha_inv = 1.0 - ca[3];
                let result: [u8; 4] =
                    std::array::from_fn(|c| quant(ca[c] + fmul(br[c], alpha_inv)));
                out[index] = Rgba(result);
            }
        }
    }
    out
}

fn feedback_ratio_for_rendering_level(level: i32) -> f32 {
    let (threshold, factor) = match level {
        1 => (5, 0.095_f32),
        2 => (10, 0.085_f32),
        _ => (3, 0.1_f32),
    };
    factor * (threshold + 1) as f32
}

fn source_draw_state(page: PageSize, screen: [u32; 2], record: &[u8]) -> P4State {
    let mut projection = [0.0; 16];
    projection[0] = 2.0 / page.width as f32;
    projection[5] = 2.0 / page.height as f32;
    projection[10] = -2.0 / 65534.0;
    projection[12] = -1.0;
    projection[13] = -1.0;
    projection[15] = 1.0;
    let background_scale = screen[0].min(screen[1]) as f32 / 1440.0;
    P4State {
        projection,
        model: model_from_addpoint(record),
        depth_scale: [
            (page.width as f32 / DEPTH_W as f32) / background_scale,
            (page.height as f32 / DEPTH_H as f32) / background_scale,
        ],
    }
}

pub(crate) fn paint_source(
    stroke: &crate::Stroke,
    mask: &[u8],
    depth: &[u8],
) -> Result<Vec<[u8; 4]>, &'static str> {
    use sha2::{Digest, Sha256};
    if mask.len() != MASK_W * MASK_H
        || depth.len() != DEPTH_W * DEPTH_H
        || format!("{:x}", Sha256::digest(mask))
            != "fc277b78270fbee42ba6cc96ff4050f5a76c595e9ec841de36e3b7fbe1e368dc"
        || format!("{:x}", Sha256::digest(depth))
            != "8188e033fffc6513cc997deb014033e7de1aacd8e27f5c6b464d491c7efaa27a"
    {
        return Err("OilBrush3 source textures do not match the captured profile");
    }
    let context = super::PublicStrokeContext {
        selected_gl_version: 1,
        source: 0,
        screen_resolution: [1920, 1080],
        model_matrix: [1., 0., 0., 0., 1., 0., 0., 0., 1.],
    };
    let records = super::sample_public_stroke(stroke, context, |y, x| Ok(y.atan2(x)))?;
    if records.len() > 128 {
        return Err("OilBrush3 source profile exceeds the dab limit");
    }
    let bytes: Vec<u8> = records
        .iter()
        .flatten()
        .flat_map(|x| x.to_le_bytes())
        .collect();
    let page = PageSize {
        width: 1920,
        height: 1080,
    };
    let mut pixels = vec![Rgba::default(); page.width * page.height];
    let mut brush = vec![Rgba([68, 170, 238, 255]); FEEDBACK * FEEDBACK];
    for stamp in 0..records.len() {
        let record = &bytes[stamp * ADDPOINT_STRIDE..(stamp + 1) * ADDPOINT_STRIDE];
        let state = source_draw_state(page, context.screen_resolution, record);
        if stamp % 4 == 3 {
            brush = render_p3_for_page(
                &pixels,
                page,
                P3State {
                    projection: state.projection,
                    model: state.model,
                    depth_scale: state.depth_scale,
                    ratio: feedback_ratio_for_rendering_level(0),
                },
                mask,
                depth,
                &brush,
            );
        }
        render_p4_with_records(
            &mut pixels,
            page,
            stamp,
            state,
            P4Textures {
                brush: &brush,
                mask,
                depth,
            },
            &bytes,
        );
    }
    Ok(pixels
        .chunks_exact(page.width)
        .rev()
        .flat_map(|row| row.iter().map(|p| p.0))
        .collect())
}
