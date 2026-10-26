//! Test-only source-input CPU model of one OilBrush3 V23 paint replay.
//!
//! The native-captured AddPoint/VBO streams are explicit geometry inputs; this test does not
//! regenerate the OilBrush3 sampler. Native readbacks are expected sparse outputs only.
//! P4/P3 varyings,
//! R8 texture samples, RGBA8 feedback, and all blend values are computed from
//! the captured AddPoint stream, draw uniforms, VBO and source texture planes.
//! This bounded Mesa/llvmpipe profile is not production dispatch.

use flate2::read::ZlibDecoder;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, io::Read};

const PAGE: usize = 192;
const PIXELS: usize = PAGE * PAGE;
const FEEDBACK: usize = 16;
const MASK_W: usize = 16;
const MASK_H: usize = 16;
const DEPTH_W: usize = 297;
const DEPTH_H: usize = 372;
const ADDPOINT_STRIDE: usize = 112;
const ADDPOINT_COUNT: usize = 15;
const STATE_BYTES: usize = 1564;
const ADDPOINTS: &[u8] = include_bytes!(
    "../../../../../conformance/oilbrush3-v23-native-paint/oilbrush3-addpoint-records-15x28-f32le.bin"
);
const P3_VBO: &[u8] = include_bytes!(
    "../../../../../conformance/oilbrush3-v23-native-paint/oilbrush3-p3-vbo-6x4-f32le.bin"
);
const STATE: &[u8] = include_bytes!(
    "../../../../../conformance/oilbrush3-v23-native-paint/oilbrush3-captured-draw-state-f32le.bin"
);
const MASK_ZLIB: &[u8] =
    include_bytes!("../../../../../conformance/oilbrush3-v23-native-paint/oilbrush3-mask-r8.zlib");
const DEPTH_ZLIB: &[u8] =
    include_bytes!("../../../../../conformance/oilbrush3-v23-native-paint/oilbrush3-depth-r8.zlib");
const PROFILE: &[u8] = include_bytes!(
    "../../../../../conformance/oilbrush3-v23-native-paint/oilbrush3-paint-profile.json"
);
const NATIVE_PREFIX: [&[u8]; 15] = [
    include_bytes!(
        "../../../../../conformance/oilbrush3-v23-native-paint/native-after-p4-01.sparse-rgba8.bin"
    ),
    include_bytes!(
        "../../../../../conformance/oilbrush3-v23-native-paint/native-after-p4-02.sparse-rgba8.bin"
    ),
    include_bytes!(
        "../../../../../conformance/oilbrush3-v23-native-paint/native-after-p4-03.sparse-rgba8.bin"
    ),
    include_bytes!(
        "../../../../../conformance/oilbrush3-v23-native-paint/native-after-p4-04.sparse-rgba8.bin"
    ),
    include_bytes!(
        "../../../../../conformance/oilbrush3-v23-native-paint/native-after-p4-05.sparse-rgba8.bin"
    ),
    include_bytes!(
        "../../../../../conformance/oilbrush3-v23-native-paint/native-after-p4-06.sparse-rgba8.bin"
    ),
    include_bytes!(
        "../../../../../conformance/oilbrush3-v23-native-paint/native-after-p4-07.sparse-rgba8.bin"
    ),
    include_bytes!(
        "../../../../../conformance/oilbrush3-v23-native-paint/native-after-p4-08.sparse-rgba8.bin"
    ),
    include_bytes!(
        "../../../../../conformance/oilbrush3-v23-native-paint/native-after-p4-09.sparse-rgba8.bin"
    ),
    include_bytes!(
        "../../../../../conformance/oilbrush3-v23-native-paint/native-after-p4-10.sparse-rgba8.bin"
    ),
    include_bytes!(
        "../../../../../conformance/oilbrush3-v23-native-paint/native-after-p4-11.sparse-rgba8.bin"
    ),
    include_bytes!(
        "../../../../../conformance/oilbrush3-v23-native-paint/native-after-p4-12.sparse-rgba8.bin"
    ),
    include_bytes!(
        "../../../../../conformance/oilbrush3-v23-native-paint/native-after-p4-13.sparse-rgba8.bin"
    ),
    include_bytes!(
        "../../../../../conformance/oilbrush3-v23-native-paint/native-after-p4-14.sparse-rgba8.bin"
    ),
    include_bytes!(
        "../../../../../conformance/oilbrush3-v23-native-paint/native-after-p4-15.sparse-rgba8.bin"
    ),
];

const ADDPOINT_SHA256: &str = "8862a946e50f1df2a4a74e940f58d3e03f839875e104d9cdeeba6efdd486870c";
const P3_VBO_SHA256: &str = "818bdaddffdbdd6e1cad09a1dac74a1da79ee482bf8b86ee601ccd93c0af3f39";
const STATE_SHA256: &str = "e8e6cdaf090ef76b70dd2824bcb7f7c979da910802794bc7183eb8ebbb383227";
const MASK_ZLIB_SHA256: &str = "88e2c40eda7057f95a35a2b80f4d1e95da5ca72267651e4f12e2b93dd3523302";
const MASK_R8_SHA256: &str = "fc277b78270fbee42ba6cc96ff4050f5a76c595e9ec841de36e3b7fbe1e368dc";
const DEPTH_ZLIB_SHA256: &str = "dcfb7a42804115addcc085a729abbd5ba6915ee262363063be1f8c741ed65da5";
const DEPTH_R8_SHA256: &str = "8188e033fffc6513cc997deb014033e7de1aacd8e27f5c6b464d491c7efaa27a";
const PROFILE_SHA256: &str = "a7f8ec44958fd64a789b1c202827e4e8e94b7c88dcacf24d071a588a6052ab43";
const PREFIX_SHA256: [&str; 15] = [
    "b9749d0227ccf3087119b940eba436258a33d86a5713bc33740c173af4fe5a4c",
    "8b8aeba311fb068177fb647e50e83c9178e6bc33d363d2978aa8684711f2ac7e",
    "1aee423e984a9384e2c741f1b026da86d8f26a028e90db6074e124511a7f19a0",
    "f34b6cc30f3754c30371e8e7283bc90f02795e875917fdba5aef6042dbe8229f",
    "814a13898983b5cf723e676bfb0c87364ffd0f50bb75357734c0191da4816e03",
    "611729bc3b7d719222138818de227688c70a460ca6cb7b92aad4fb6cda7b02fb",
    "e1ca0470a16be38a1fe55df43a27e2ea2e93144be36629cd1bbc887baf6460d0",
    "62e8cd23edfa96a5d79b6fff2764bfe3eb39293cde2d3278105dddb58e4247aa",
    "c34d3d52e9feb19e17a0492aea157d3451afd40de98b4f580816d635a7d995ae",
    "9e87fbe2c7814eaea6083f9a65d5aa0333487117b9cb64b7da5ab22c90ef6f9f",
    "b5fabe9e06b7cdbdc5430a836c6bb5caf83bdf1a161c18b5f297c2b2d30244ad",
    "1847b2e5a916e9c8ed1782558117877596b740b87918d051f1de5437b784d837",
    "99706a4fa83a7da45284c4b4fac08a73eebacb4a4ae947d2f30bd1b7f24a3e78",
    "59655086e5af2afd8f70816a7ce8d699ebb6534d31cf4be2b1c56afa8dd741a7",
    "c8a5b1153dc13458d161696e957fe2f7be0f766a1467a2150f22e5b48061aa6b",
];
const EXPECTED_SUPPORT: [usize; 15] = [3, 7, 9, 9, 10, 13, 17, 23, 31, 37, 43, 54, 61, 67, 76];
const EXPECTED_DIFF_PIXELS: [usize; 15] = [1, 4, 4, 4, 3, 5, 6, 7, 10, 13, 13, 13, 14, 14, 16];
const EXPECTED_ABS_SUMS: [[u64; 4]; 15] = [
    [1, 0, 0, 0],
    [2, 1, 1, 0],
    [2, 2, 1, 0],
    [2, 2, 1, 0],
    [2, 1, 0, 0],
    [3, 1, 1, 0],
    [3, 1, 2, 0],
    [3, 1, 3, 0],
    [3, 1, 6, 0],
    [4, 1, 8, 0],
    [4, 1, 8, 0],
    [4, 1, 8, 0],
    [5, 1, 9, 0],
    [5, 1, 9, 0],
    [6, 1, 11, 0],
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
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
struct DiffStats {
    support: usize,
    support_symdiff: usize,
    diff_pixels: usize,
    sums: [u64; 4],
    max: [u8; 4],
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
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
fn quant(x: f64) -> u8 {
    x.mul_add(255.0, 0.0).round_ties_even().clamp(0.0, 255.0) as u8
}

fn decode_zlib(bytes: &[u8], expected_len: usize, expected_sha: &str) -> Vec<u8> {
    let decoder = ZlibDecoder::new(bytes);
    let mut raw = Vec::with_capacity(expected_len);
    decoder
        .take((expected_len + 1) as u64)
        .read_to_end(&mut raw)
        .expect("pinned source R8 zlib decodes");
    assert_eq!(raw.len(), expected_len);
    assert_eq!(sha256(&raw), expected_sha);
    raw
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

fn sample_nearest_r8(tex: &[u8], w: usize, h: usize, u: f64, v: f64, repeat: bool) -> u8 {
    fn coord(t: f64, n: usize, repeat: bool) -> usize {
        let i = if repeat {
            ((t.rem_euclid(1.0)) * n as f64).floor() as isize
        } else {
            (t * n as f64).floor() as isize
        };
        i.clamp(0, n as isize - 1) as usize
    }
    tex[coord(v, h, repeat) * w + coord(u, w, repeat)]
}

fn sample_nearest_rgba(tex: &[Rgba], w: usize, h: usize, u: f64, v: f64) -> Rgba {
    let x = ((u * w as f64).floor() as isize).clamp(0, w as isize - 1) as usize;
    let y = ((v * h as f64).floor() as isize).clamp(0, h as isize - 1) as usize;
    tex[y * w + x]
}

fn sample_linear_r8(tex: &[u8], w: usize, h: usize, u: f64, v: f64, repeat: bool) -> f64 {
    fn index(i: isize, n: usize, repeat: bool) -> usize {
        if repeat {
            i.rem_euclid(n as isize) as usize
        } else {
            i.clamp(0, n as isize - 1) as usize
        }
    }
    let x = u * w as f64 - 0.5;
    let y = v * h as f64 - 0.5;
    let x0 = x.floor() as isize;
    let y0 = y.floor() as isize;
    let fx = x - x0 as f64;
    let fy = y - y0 as f64;
    let ch = |ix: isize, iy: isize| {
        f64::from(tex[index(iy, h, repeat) * w + index(ix, w, repeat)]) / 255.0
    };
    ch(x0, y0) * (1.0 - fx) * (1.0 - fy)
        + ch(x0 + 1, y0) * fx * (1.0 - fy)
        + ch(x0, y0 + 1) * (1.0 - fx) * fy
        + ch(x0 + 1, y0 + 1) * fx * fy
}

fn sample_linear_rgba(tex: &[Rgba], w: usize, h: usize, u: f64, v: f64) -> [f64; 4] {
    let x = u * w as f64 - 0.5;
    let y = v * h as f64 - 0.5;
    let x0 = x.floor() as isize;
    let y0 = y.floor() as isize;
    let fx = x - x0 as f64;
    let fy = y - y0 as f64;
    let clamp = |n: isize, lim: usize| n.clamp(0, lim as isize - 1) as usize;
    let ch = |ix: isize, iy: isize, c: usize| {
        f64::from(tex[clamp(iy, h) * w + clamp(ix, w)].0[c]) / 255.0
    };
    std::array::from_fn(|c| {
        ch(x0, y0, c) * (1.0 - fx) * (1.0 - fy)
            + ch(x0 + 1, y0, c) * fx * (1.0 - fy)
            + ch(x0, y0 + 1, c) * (1.0 - fx) * fy
            + ch(x0 + 1, y0 + 1, c) * fx * fy
    })
}

fn unpack_p4_triangle(stamp: usize, tri_indexes: [usize; 3]) -> P4Triangle {
    let proj = std::array::from_fn(|i| read_f32(STATE, i * 4));
    let base = 64 + stamp * 72;
    let model: [f32; 16] = std::array::from_fn(|i| read_f32(STATE, base + i * 4));
    let depth_scale = [read_f32(STATE, base + 64), read_f32(STATE, base + 68)];
    let record = &ADDPOINTS[stamp * ADDPOINT_STRIDE..(stamp + 1) * ADDPOINT_STRIDE];
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
        let x = read_f32(ADDPOINTS, o);
        let y = read_f32(ADDPOINTS, o + 4);
        let u = read_f32(ADDPOINTS, o + 8);
        let v = read_f32(ADDPOINTS, o + 12);
        let world = mat_vec(&model, [x, y, 0.0, 1.0], true);
        let clip = mat_vec(&proj, world, false);
        let uv = [
            fmul(fadd(fdiv(clip[0], clip[3]), 1.0), 0.5),
            fmul(fadd(fdiv(clip[1], clip[3]), 1.0), 0.5),
        ];
        let inv_w = fdiv(1.0, clip[3]);
        let wx = fadd(fmul(fmul(clip[0], inv_w), 96.0), 96.0);
        let wy = fadd(fmul(fmul(clip[1], inv_w), 96.0), 96.0);
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

fn render_p4(
    canvas: &mut [Rgba],
    stamp: usize,
    brush: &[Rgba],
    mask: &[u8],
    depth: &[u8],
) -> usize {
    let mut touched = std::collections::BTreeSet::new();
    for ids in [[0, 1, 2], [3, 4, 5]] {
        let tri = unpack_p4_triangle(stamp, ids);
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
            .min(PAGE as isize - 1);
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
            .min(PAGE as isize - 1);
        if min_x > max_x || min_y > max_y {
            continue;
        }
        for y in min_y as usize..=max_y as usize {
            for x in min_x as usize..=max_x as usize {
                let index = y * PAGE + x;
                if touched.contains(&index) {
                    continue;
                }
                if inside_triangle([x as f64 + 0.5, y as f64 + 0.5], tri.pos).is_none() {
                    continue;
                }
                touched.insert(index);
                let mu = f64::from(eval_plane(tri.mask_u, x, y));
                let mv = f64::from(eval_plane(tri.mask_v, x, y));
                let du = f64::from(eval_plane(tri.depth_u, x, y));
                let dv = f64::from(eval_plane(tri.depth_v, x, y));
                let src = sample_nearest_rgba(brush, FEEDBACK, FEEDBACK, mu, mv).0;
                let mask_r =
                    f64::from(sample_nearest_r8(mask, MASK_W, MASK_H, mu, mv, false)) / 255.0;
                let depth_r = sample_linear_r8(depth, DEPTH_W, DEPTH_H, du, dv, true);
                let alpha = fmul(
                    fmul((f64::from(src[3]) / 255.0) as f32, mask_r as f32),
                    depth_r as f32,
                );
                let rgb: [f32; 3] = std::array::from_fn(|c| {
                    fmul(
                        fmul((f64::from(src[c]) / 255.0) as f32, mask_r as f32),
                        depth_r as f32,
                    )
                });
                let dst = canvas[index].0;
                let inv = 1.0 - f64::from(alpha);
                let out = [
                    f64::from(rgb[0]) + (f64::from(dst[0]) / 255.0) * inv,
                    f64::from(rgb[1]) + (f64::from(dst[1]) / 255.0) * inv,
                    f64::from(rgb[2]) + (f64::from(dst[2]) / 255.0) * inv,
                    f64::from(alpha) + (f64::from(dst[3]) / 255.0) * inv,
                ];
                canvas[index] = Rgba(out.map(quant));
            }
        }
    }
    touched.len()
}

fn read_p3_state(index: usize) -> P3State {
    let base = 64 + ADDPOINT_COUNT * 72 + index * 140;
    P3State {
        projection: std::array::from_fn(|i| read_f32(STATE, base + i * 4)),
        model: std::array::from_fn(|i| read_f32(STATE, base + 64 + i * 4)),
        depth_scale: [read_f32(STATE, base + 128), read_f32(STATE, base + 132)],
        ratio: read_f32(STATE, base + 136),
    }
}

fn render_p3(
    canvas: &[Rgba],
    params: P3State,
    mask: &[u8],
    depth: &[u8],
    brush: &[Rgba],
) -> Vec<Rgba> {
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
                let (cu, cv) = (lerp(2), lerp(3));
                // Brush coordinates are the untransformed attribute ZW lanes.
                let bu = weights[0] * f64::from(read_f32(P3_VBO, ids[0] * 16 + 8))
                    + weights[1] * f64::from(read_f32(P3_VBO, ids[1] * 16 + 8))
                    + weights[2] * f64::from(read_f32(P3_VBO, ids[2] * 16 + 8));
                let bv = weights[0] * f64::from(read_f32(P3_VBO, ids[0] * 16 + 12))
                    + weights[1] * f64::from(read_f32(P3_VBO, ids[1] * 16 + 12))
                    + weights[2] * f64::from(read_f32(P3_VBO, ids[2] * 16 + 12));
                let cr = sample_linear_rgba(canvas, PAGE, PAGE, cu, cv);
                let mu = f64::from(sample_nearest_r8(mask, MASK_W, MASK_H, bu, bv, false)) / 255.0;
                let du = cu * f64::from(params.depth_scale[0]);
                let dv = cv * f64::from(params.depth_scale[1]);
                let dr = sample_linear_r8(depth, DEPTH_W, DEPTH_H, du, dv, true);
                let b = fmul(fmul(mu as f32, params.ratio), dr as f32);
                let ca: [f32; 4] = cr.map(|c| fmul(c as f32, b));
                let br = sample_nearest_rgba(brush, FEEDBACK, FEEDBACK, bu, bv).0;
                let alpha_inv = 1.0 - f64::from(ca[3]);
                let result = [
                    f64::from(ca[0]) + (f64::from(br[0]) / 255.0) * alpha_inv,
                    f64::from(ca[1]) + (f64::from(br[1]) / 255.0) * alpha_inv,
                    f64::from(ca[2]) + (f64::from(br[2]) / 255.0) * alpha_inv,
                    f64::from(ca[3]) + (f64::from(br[3]) / 255.0) * alpha_inv,
                ];
                out[index] = Rgba(result.map(quant));
            }
        }
    }
    out
}

fn decode_expected(bytes: &[u8], expected_sha: &str) -> BTreeMap<usize, Rgba> {
    assert_eq!(sha256(bytes), expected_sha);
    assert_eq!(bytes.len() % 6, 0);
    let mut out = BTreeMap::new();
    for r in bytes.as_chunks::<6>().0 {
        let index = usize::from(u16::from_le_bytes([r[0], r[1]]));
        assert!(index < PIXELS);
        let rgba = Rgba([r[2], r[3], r[4], r[5]]);
        assert!(rgba.0.iter().any(|c| *c != 0));
        assert!(out.insert(index, rgba).is_none());
    }
    out
}

fn compare_frame(model: &[Rgba], native: &BTreeMap<usize, Rgba>) -> DiffStats {
    let mut stats = DiffStats {
        support: 0,
        support_symdiff: 0,
        diff_pixels: 0,
        sums: [0; 4],
        max: [0; 4],
    };
    for (index, actual) in model.iter().enumerate() {
        let expected = native.get(&index).copied().unwrap_or_default();
        let actual_alpha = actual.0[3] != 0;
        let expected_alpha = expected.0[3] != 0;
        stats.support += usize::from(actual_alpha);
        stats.support_symdiff += usize::from(actual_alpha != expected_alpha);
        let mut pixel_different = false;
        for (c, (a, b)) in actual.0.into_iter().zip(expected.0).enumerate() {
            let d = a.abs_diff(b);
            stats.sums[c] += u64::from(d);
            stats.max[c] = stats.max[c].max(d);
            pixel_different |= d != 0;
        }
        stats.diff_pixels += usize::from(pixel_different);
    }
    stats
}

fn simulate() -> Vec<DiffStats> {
    assert_eq!(ADDPOINTS.len(), ADDPOINT_COUNT * ADDPOINT_STRIDE);
    assert_eq!(sha256(ADDPOINTS), ADDPOINT_SHA256);
    assert_eq!(sha256(P3_VBO), P3_VBO_SHA256);
    assert_eq!(P3_VBO, &ADDPOINTS[..96]);
    assert_eq!(STATE.len(), STATE_BYTES);
    assert_eq!(sha256(STATE), STATE_SHA256);
    assert_eq!(sha256(MASK_ZLIB), MASK_ZLIB_SHA256);
    assert_eq!(sha256(DEPTH_ZLIB), DEPTH_ZLIB_SHA256);
    assert_eq!(sha256(PROFILE), PROFILE_SHA256);
    let mask = decode_zlib(MASK_ZLIB, MASK_W * MASK_H, MASK_R8_SHA256);
    let depth = decode_zlib(DEPTH_ZLIB, DEPTH_W * DEPTH_H, DEPTH_R8_SHA256);
    let mut canvas = vec![Rgba::default(); PIXELS]; // conditional transparent texture20 initialization profile
    let blue = Rgba([68, 170, 238, 255]); // native source-cleared texture8 input
    let mut brush = vec![blue; FEEDBACK * FEEDBACK];
    let mut feedback_unique = Vec::new();
    let mut stats = Vec::with_capacity(ADDPOINT_COUNT);
    let mut p3_index = 0;
    for stamp in 0..ADDPOINT_COUNT {
        if matches!(stamp, 3 | 7 | 11) {
            let feedback = render_p3(&canvas, read_p3_state(p3_index), &mask, &depth, &brush);
            let unique = feedback
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>();
            feedback_unique.push(unique.len());
            brush = feedback;
            p3_index += 1;
        }
        render_p4(&mut canvas, stamp, &brush, &mask, &depth);
        let expected = decode_expected(NATIVE_PREFIX[stamp], PREFIX_SHA256[stamp]);
        stats.push(compare_frame(&canvas, &expected));
    }
    assert_eq!(feedback_unique, [1, 1, 1]);
    assert!(brush.iter().all(|p| p.0 == [68, 170, 238, 255]));
    stats
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oilbrush3_v23_source_model_matches_all_native_prefix_support_and_bounded_rgb_error() {
        let rows = simulate();
        assert_eq!(rows.len(), 15);
        for (i, row) in rows.iter().enumerate() {
            assert_eq!(row.support, EXPECTED_SUPPORT[i], "prefix {} support", i + 1);
            assert_eq!(
                row.support_symdiff,
                0,
                "prefix {} support symmetric difference",
                i + 1
            );
            assert!(
                row.max[..3].iter().all(|v| *v <= 1),
                "prefix {} RGB max: {:?}",
                i + 1,
                row.max
            );
            assert_eq!(row.max[3], 0, "prefix {} alpha max", i + 1);
            assert_eq!(
                row.diff_pixels,
                EXPECTED_DIFF_PIXELS[i],
                "prefix {} diff pixels",
                i + 1
            );
            assert_eq!(row.sums, EXPECTED_ABS_SUMS[i], "prefix {} abs sums", i + 1);
        }
        assert_eq!(rows[14].diff_pixels, 16);
        assert_eq!(rows[14].sums, [6, 1, 11, 0]);
        assert_eq!(rows[14].max, [1, 1, 1, 0]);
    }
}
