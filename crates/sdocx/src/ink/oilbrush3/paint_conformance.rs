//! Test-only source-input CPU model of two OilBrush3 host paint controls.
//!
//! The native-captured AddPoint/VBO streams are explicit geometry inputs; this test does not
//! regenerate the OilBrush3 sampler. Native readbacks are expected sparse outputs only.
//! P4/P3 varyings, R8 texture samples, and RGBA8 feedback are computed from
//! the captured AddPoint stream, draw uniforms, VBO and source texture planes.
//! P4 uses a bounded llvmpipe host model for RGBA8 framebuffer source-over; the
//! supplied shader LLVM IR does not include fixed-function blend lowering. This
//! profile is not production dispatch.

use flate2::read::ZlibDecoder;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, io::Read};

const PAGE: usize = 192;
const PIXELS: usize = PAGE * PAGE;
const V17_PAGE_WIDTH: usize = 1920;
const V17_PAGE_HEIGHT: usize = 1080;
const V17_PIXELS: usize = V17_PAGE_WIDTH * V17_PAGE_HEIGHT;
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
const NATIVE_PREFIX_BUNDLE: &[u8] = include_bytes!(
    "../../../../../conformance/oilbrush3-v23-native-paint/native-prefixes-sparse-rgba8-v1.bin"
);
const NATIVE_PREFIX_MAGIC: &[u8; 8] = b"SDOCXOB1";
const NATIVE_PREFIX_COUNT: usize = 15;
const NATIVE_PREFIX_HEADER_BYTES: usize = 14;

const ADDPOINT_SHA256: &str = "8862a946e50f1df2a4a74e940f58d3e03f839875e104d9cdeeba6efdd486870c";
const P3_VBO_SHA256: &str = "818bdaddffdbdd6e1cad09a1dac74a1da79ee482bf8b86ee601ccd93c0af3f39";
const STATE_SHA256: &str = "e8e6cdaf090ef76b70dd2824bcb7f7c979da910802794bc7183eb8ebbb383227";
const MASK_ZLIB_SHA256: &str = "88e2c40eda7057f95a35a2b80f4d1e95da5ca72267651e4f12e2b93dd3523302";
const MASK_R8_SHA256: &str = "fc277b78270fbee42ba6cc96ff4050f5a76c595e9ec841de36e3b7fbe1e368dc";
const DEPTH_ZLIB_SHA256: &str = "dcfb7a42804115addcc085a729abbd5ba6915ee262363063be1f8c741ed65da5";
const DEPTH_R8_SHA256: &str = "8188e033fffc6513cc997deb014033e7de1aacd8e27f5c6b464d491c7efaa27a";
const PROFILE_SHA256: &str = "560837b7cc189e849c19d332894a88868066dc1fe8478b7028a376f4f57387a8";
const NATIVE_PREFIX_BUNDLE_SHA256: &str =
    "20f504f2780d7d82feafe5ea5970e8a10a4919cd87ee6c2652c64d0229ab7028";
const V17_STATE: &[u8] = include_bytes!(
    "../../../../../conformance/oilbrush3-v17-native-paint/oilbrush3-v17-draw-state-f32le.bin"
);
const V17_PREFIX_BUNDLE: &[u8] = include_bytes!(
    "../../../../../conformance/oilbrush3-v17-native-paint/oilbrush3-v17-native-prefixes-sparse-rgba8-v2.bin"
);
const V17_STATE_SHA256: &str = "5962fca258a4158fe42c83ce46ab6f4559ceddf1c2e26b57a9294c6b1d59c18b";
const V17_PREFIX_BUNDLE_SHA256: &str =
    "92e5080f149a59e8569301d3d0577b1df7d13f1d0dd71dc63037342e6470ba05";
const V17_PREFIX_SHA256: [&str; 15] = [
    "a99168eb91498958bbdc6bfa502136595c35e1f58eb3eb759b58b1b121754660",
    "14dfa1bf824eb8fb3f1e38cfb1fd3c688fb320d9dcf1902f4bd1bff3ea4d43f5",
    "d506ab378408fd74f759e499a2f7465da759dc0e299199a2861a5af3b0fff09a",
    "224377c3189bb5956786984a1d5c0517c206e3d27e2f7b9464cc87e48313f492",
    "8cfdfabe9647747a67c209341548f4f4f30293a4e1299c9196f76499973787dc",
    "5f3910208918f6dcf2d360b72a986fa6344dbf9ebf53577c0f4e6d4cd460ac9c",
    "4a46b3fb0ccf5e67dfec9c46aa4455d9a986ea7ad92a46c8c0e43bde1c11844d",
    "66f9508fc3a9e439c97fdbe7b2cb8690b93a050235e85255eb787992682d1fcf",
    "db1e2ed1e08e71198362a0c622d3828fb143f33f8974b199ad3bae3bab97aa22",
    "91e10428dce2b8f97b0e58239330446804129bccc71207700d641cafcfd97fd6",
    "43de6795188eb427026165ef2ed6f84d7823298a9c014d206497b2fb2ad0f013",
    "cb263a7850a6bf05d202ec365ea7f78c7ce6d6374d54c9876f7fbf76fe905621",
    "fcb7f045933cf361f0b12500e4ffd4abe6820912a231024fa9ac660094c6f3df",
    "a54a69f9bfb1532f70a3a64a6962bf5481508c57fdba58eb345b9b8d36149e7a",
    "14f1911a67ba192934f2b3534f94925b7ec6b900d7046705fdfbdca16d8824ee",
];
const V17_EXPECTED_SUPPORT: [usize; 15] = [3, 6, 9, 9, 10, 13, 17, 23, 31, 37, 43, 54, 61, 67, 76];
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
fn quant(x: f32) -> u8 {
    // Mesa converts the shader's clamped F32 output before byte blending.
    // CVTPS2DQ follows MXCSR; this host profile assumes nearest-even.
    (x.clamp(0.0, 1.0) * 255.0).round_ties_even() as u8
}

// Captured P4 state uses FUNC_ADD with ONE / ONE_MINUS_SRC_ALPHA for color
// and alpha on RGBA8 attachments. Quantize the shader result first, then model
// the normalized-integer framebuffer blend from Mesa 25.0.7 lp_build_mul_norm.
// This common driver path has captured Crayon2 IR corroboration; OilBrush3's
// shader-only IR does not independently establish its active blend lowering.
fn rgba8_source_over(src: [u8; 4], dst: [u8; 4]) -> [u8; 4] {
    let inverse_alpha = 255 - u32::from(src[3]);
    std::array::from_fn(|channel| {
        let src = u32::from(src[channel]);
        let dst = u32::from(dst[channel]);
        let product = dst * inverse_alpha;
        let contribution = (product + (product >> 8) + 128) >> 8;
        (src + contribution).min(255) as u8
    })
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

// Mesa Gallivm's captured RGBA8 sampler profile, reconstructed from the active
// V17 NIR/LLVM in root-driver-ir-receipt.json. Coordinates are quantized to an
// 8-bit subtexel grid before byte-domain bilinear interpolation. All shader
// samplers are lowp: normalized byte values narrow to binary16 toward zero and
// widen to F32 before shader arithmetic. CVTPS2DQ follows MXCSR; this candidate
// assumes default RN-even, which was not directly recorded in the capture.
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

fn unpack_p4_triangle(stamp: usize, tri_indexes: [usize; 3]) -> P4Triangle {
    let state = read_p4_state(STATE, stamp);
    unpack_p4_triangle_for_page(
        stamp,
        tri_indexes,
        PageSize {
            width: PAGE,
            height: PAGE,
        },
        state,
    )
}

fn read_p4_state(bytes: &[u8], stamp: usize) -> P4State {
    let base = 64 + stamp * 72;
    P4State {
        projection: std::array::from_fn(|i| read_f32(bytes, i * 4)),
        model: std::array::from_fn(|i| read_f32(bytes, base + i * 4)),
        depth_scale: [read_f32(bytes, base + 64), read_f32(bytes, base + 68)],
    }
}

fn unpack_p4_triangle_for_page(
    stamp: usize,
    tri_indexes: [usize; 3],
    page: PageSize,
    state: P4State,
) -> P4Triangle {
    let model = state.model;
    let proj = state.projection;
    let depth_scale = state.depth_scale;
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

fn render_p4(
    canvas: &mut [Rgba],
    stamp: usize,
    brush: &[Rgba],
    mask: &[u8],
    depth: &[u8],
) -> usize {
    render_p4_for_page(
        canvas,
        PageSize {
            width: PAGE,
            height: PAGE,
        },
        stamp,
        read_p4_state(STATE, stamp),
        brush,
        mask,
        depth,
    )
}

fn render_p4_for_page(
    canvas: &mut [Rgba],
    page: PageSize,
    stamp: usize,
    state: P4State,
    brush: &[Rgba],
    mask: &[u8],
    depth: &[u8],
) -> usize {
    assert_eq!(canvas.len(), page.width * page.height);
    let mut touched = std::collections::BTreeSet::new();
    for ids in [[0, 1, 2], [3, 4, 5]] {
        let tri = unpack_p4_triangle_for_page(stamp, ids, page, state);
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

fn read_p3_state(index: usize) -> P3State {
    read_p3_state_from(STATE, index)
}

fn read_p3_state_from(bytes: &[u8], index: usize) -> P3State {
    let base = 64 + ADDPOINT_COUNT * 72 + index * 140;
    P3State {
        projection: std::array::from_fn(|i| read_f32(bytes, base + i * 4)),
        model: std::array::from_fn(|i| read_f32(bytes, base + 64 + i * 4)),
        depth_scale: [read_f32(bytes, base + 128), read_f32(bytes, base + 132)],
        ratio: read_f32(bytes, base + 136),
    }
}

fn render_p3(
    canvas: &[Rgba],
    params: P3State,
    mask: &[u8],
    depth: &[u8],
    brush: &[Rgba],
) -> Vec<Rgba> {
    render_p3_for_page(
        canvas,
        PageSize {
            width: PAGE,
            height: PAGE,
        },
        params,
        mask,
        depth,
        brush,
    )
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

fn read_u16_le(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().expect("two-byte u16"))
}

fn decode_native_prefix_bundle(bytes: &'static [u8]) -> Vec<&'static [u8]> {
    assert_eq!(sha256(bytes), NATIVE_PREFIX_BUNDLE_SHA256);
    assert!(bytes.len() >= NATIVE_PREFIX_HEADER_BYTES);
    assert_eq!(&bytes[..8], NATIVE_PREFIX_MAGIC);
    assert_eq!(read_u16_le(bytes, 8), PAGE as u16, "bundle width");
    assert_eq!(read_u16_le(bytes, 10), PAGE as u16, "bundle height");
    let count = usize::from(read_u16_le(bytes, 12));
    assert_eq!(count, NATIVE_PREFIX_COUNT, "bundle prefix count");

    let mut offset = NATIVE_PREFIX_HEADER_BYTES;
    let mut prefixes = Vec::with_capacity(count);
    for index in 0..count {
        let count_end = offset.checked_add(2).expect("entry count offset");
        assert!(
            count_end <= bytes.len(),
            "prefix {index} missing record count"
        );
        let records = usize::from(read_u16_le(bytes, offset));
        let payload_bytes = records.checked_mul(6).expect("sparse payload size");
        let payload_end = count_end.checked_add(payload_bytes).expect("payload end");
        assert!(
            payload_end <= bytes.len(),
            "prefix {index} truncated payload"
        );
        prefixes.push(&bytes[count_end..payload_end]);
        offset = payload_end;
    }
    assert_eq!(offset, bytes.len(), "trailing bytes after prefix bundle");
    prefixes
}

fn read_u32_le(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("four-byte u32"))
}

fn decode_v17_native_prefix_bundle(bytes: &'static [u8]) -> Vec<&'static [u8]> {
    assert_eq!(sha256(bytes), V17_PREFIX_BUNDLE_SHA256);
    assert!(bytes.len() >= 18);
    assert_eq!(&bytes[..8], b"SDOCXOV2");
    assert_eq!(read_u32_le(bytes, 8), V17_PAGE_WIDTH as u32);
    assert_eq!(read_u32_le(bytes, 12), V17_PAGE_HEIGHT as u32);
    assert_eq!(read_u16_le(bytes, 16) as usize, NATIVE_PREFIX_COUNT);

    let mut offset: usize = 18;
    let mut prefixes = Vec::with_capacity(NATIVE_PREFIX_COUNT);
    for index in 0..NATIVE_PREFIX_COUNT {
        let count_end = offset.checked_add(4).expect("V17 prefix count offset");
        assert!(count_end <= bytes.len(), "V17 prefix {index} missing count");
        let records = read_u32_le(bytes, offset) as usize;
        let payload_bytes = records.checked_mul(8).expect("V17 sparse payload size");
        let payload_end = count_end
            .checked_add(payload_bytes)
            .expect("V17 payload end");
        assert!(
            payload_end <= bytes.len(),
            "V17 prefix {index} truncated payload"
        );
        prefixes.push(&bytes[count_end..payload_end]);
        offset = payload_end;
    }
    assert_eq!(offset, bytes.len(), "V17 trailing bundle bytes");
    prefixes
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

fn decode_v17_expected(bytes: &[u8], expected_sha: &str) -> BTreeMap<usize, Rgba> {
    assert_eq!(sha256(bytes), expected_sha);
    assert_eq!(bytes.len() % 8, 0);
    let mut out = BTreeMap::new();
    for record in bytes.as_chunks::<8>().0 {
        let index = read_u32_le(record, 0) as usize;
        assert!(index < V17_PIXELS);
        let rgba = Rgba(record[4..8].try_into().expect("four-byte RGBA"));
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
    let native_prefixes = decode_native_prefix_bundle(NATIVE_PREFIX_BUNDLE);
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
        let expected = decode_expected(native_prefixes[stamp], PREFIX_SHA256[stamp]);
        stats.push(compare_frame(&canvas, &expected));
    }
    assert_eq!(feedback_unique, [1, 1, 1]);
    assert!(brush.iter().all(|p| p.0 == [68, 170, 238, 255]));
    stats
}

fn simulate_v17_public_drawing() -> Vec<DiffStats> {
    assert_eq!(ADDPOINTS.len(), ADDPOINT_COUNT * ADDPOINT_STRIDE);
    assert_eq!(sha256(ADDPOINTS), ADDPOINT_SHA256);
    assert_eq!(sha256(P3_VBO), P3_VBO_SHA256);
    assert_eq!(P3_VBO, &ADDPOINTS[..96]);
    assert_eq!(V17_STATE.len(), STATE_BYTES);
    assert_eq!(sha256(V17_STATE), V17_STATE_SHA256);
    let native_prefixes = decode_v17_native_prefix_bundle(V17_PREFIX_BUNDLE);
    let mask = decode_zlib(MASK_ZLIB, MASK_W * MASK_H, MASK_R8_SHA256);
    let depth = decode_zlib(DEPTH_ZLIB, DEPTH_W * DEPTH_H, DEPTH_R8_SHA256);
    let page = PageSize {
        width: V17_PAGE_WIDTH,
        height: V17_PAGE_HEIGHT,
    };
    let mut canvas = vec![Rgba::default(); page.width * page.height];
    // The captured Model color is 0x8044aaee. OilBrush3 initializes its feedback brush
    // as the source RGB with opaque alpha; the pre-feedback readback is not a model input.
    let color_argb = 0x8044_aaee_u32;
    let initial_brush_color = Rgba([
        ((color_argb >> 16) & 0xff) as u8,
        ((color_argb >> 8) & 0xff) as u8,
        (color_argb & 0xff) as u8,
        0xff,
    ]);
    let mut brush = vec![initial_brush_color; FEEDBACK * FEEDBACK];
    let mut p3_index = 0;
    let mut stats = Vec::with_capacity(ADDPOINT_COUNT);
    for stamp in 0..ADDPOINT_COUNT {
        if matches!(stamp, 3 | 7 | 11) {
            brush = render_p3_for_page(
                &canvas,
                page,
                read_p3_state_from(V17_STATE, p3_index),
                &mask,
                &depth,
                &brush,
            );
            p3_index += 1;
        }
        render_p4_for_page(
            &mut canvas,
            page,
            stamp,
            read_p4_state(V17_STATE, stamp),
            &brush,
            &mask,
            &depth,
        );
        let expected = decode_v17_expected(native_prefixes[stamp], V17_PREFIX_SHA256[stamp]);
        stats.push(compare_frame(&canvas, &expected));
    }
    assert_eq!(p3_index, 3);
    stats
}

// Test-only V25 SVG conformance builder. It uses the bounded captured P4/P3
// source inputs and never embeds a CPU or captured framebuffer as a page image.
#[cfg(feature = "pdf")]
fn svg_data_uri(width: u32, height: u32, rgba: &[u8]) -> String {
    use base64::Engine as _;
    let mut encoded = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut encoded, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("PNG header");
        writer.write_image_data(rgba).expect("PNG pixels");
    }
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(encoded)
    )
}

#[cfg(feature = "pdf")]
fn svg_texture_uri(width: usize, height: usize, pixels: &[Rgba]) -> String {
    assert_eq!(pixels.len(), width * height);
    // The model arrays use GL bottom-row-first order; PNG rows are top-down.
    let mut rgba = Vec::with_capacity(pixels.len() * 4);
    for y in (0..height).rev() {
        for p in &pixels[y * width..(y + 1) * width] {
            rgba.extend_from_slice(&p.0);
        }
    }
    svg_data_uri(width as u32, height as u32, &rgba)
}

#[cfg(feature = "pdf")]
fn svg_brush_clamped_uri(pixels: &[Rgba]) -> String {
    assert_eq!(pixels.len(), FEEDBACK * FEEDBACK);
    let mut padded = vec![Rgba::default(); 18 * 18];
    for y in 0usize..18 {
        for x in 0usize..18 {
            let sx = x.saturating_sub(1).min(FEEDBACK - 1);
            let sy = y.saturating_sub(1).min(FEEDBACK - 1);
            padded[y * 18 + x] = pixels[sy * FEEDBACK + sx];
        }
    }
    // GL_CLAMP_TO_EDGE extends the source texel at each border; this direct
    // padded carrier removes SVG image-edge coverage from the actual quad clip.
    svg_texture_uri(18, 18, &padded)
}

#[cfg(feature = "pdf")]
fn svg_r8_uri(width: usize, height: usize, values: &[u8]) -> String {
    assert_eq!(values.len(), width * height);
    let mut rgba = Vec::with_capacity(values.len() * 4);
    // Preserve the native source-upload row order for mask and depth inputs.
    for y in 0..height {
        for r in &values[y * width..(y + 1) * width] {
            // Match the captured source PNG carrier: R8 is replicated to RGB,
            // with opaque alpha. The shader-facing filter matrix later reads R.
            rgba.extend_from_slice(&[*r, *r, *r, 255]);
        }
    }
    svg_data_uri(width as u32, height as u32, &rgba)
}

#[cfg(feature = "pdf")]
fn svg_depth_repeat_uri(depth: &[u8]) -> String {
    assert_eq!(depth.len(), DEPTH_W * DEPTH_H);
    // Captured depth coordinates range from about -.013 to .091 on both axes.
    // A 3x3 source-texture tiling preserves GL_REPEAT without baking a page.
    let mut tiled = vec![0; DEPTH_W * 3 * DEPTH_H * 3];
    let width = DEPTH_W * 3;
    for ty in 0..3 {
        for tx in 0..3 {
            for y in 0..DEPTH_H {
                let dst = (ty * DEPTH_H + y) * width + tx * DEPTH_W;
                tiled[dst..dst + DEPTH_W].copy_from_slice(&depth[y * DEPTH_W..(y + 1) * DEPTH_W]);
            }
        }
    }
    // Source textures are encoded in their captured upload row order. The
    // shader V coordinate is used directly for these source R8 inputs.
    svg_r8_uri(width, DEPTH_H * 3, &tiled)
}

#[cfg(feature = "pdf")]
fn svg_doc_plane(p: Plane) -> [f64; 3] {
    let [origin, dx, dy] = p.0.map(f64::from);
    // Preserve the native Mesa F32 plane and change coordinate bases in F64.
    // Native plane coordinates are fragment centers in GL bottom-origin space;
    // SVG document coordinates are top-origin with x/y pixel centers at n+.5.
    [dx, -dy, origin - 0.5 * dx + (PAGE as f64 - 0.5) * dy]
}

#[cfg(feature = "pdf")]
fn svg_plane_image_map(
    u_plane: Plane,
    v_plane: Plane,
    scale_u: f64,
    scale_v: f64,
    offset_u: f64,
    offset_v: f64,
    flip_v: bool,
) -> [f64; 6] {
    // Source-image coordinates are an affine function of document coordinates:
    // (u_doc,v_doc) come from the captured global Mesa planes, not refitted UVs.
    let u = svg_doc_plane(u_plane);
    let v = svg_doc_plane(v_plane);
    let sy = if flip_v { -scale_v } else { scale_v };
    let so = if flip_v { scale_v + offset_v } else { offset_v };
    let sx = [scale_u * u[0], scale_u * u[1], scale_u * u[2] + offset_u];
    let sy = [sy * v[0], sy * v[1], sy * v[2] + so];
    let det = sx[0] * sy[1] - sx[1] * sy[0];
    assert!(
        det.is_finite() && det.abs() > 1e-12,
        "degenerate captured source plane"
    );
    // Invert source(x,y)=A*document(x,y)+b to obtain SVG's image transform.
    let a = sy[1] / det;
    let c = -sx[1] / det;
    let b = -sy[0] / det;
    let d = sx[0] / det;
    let e = -(a * sx[2] + c * sy[2]);
    let f = -(b * sx[2] + d * sy[2]);
    [a, b, c, d, e, f]
}

#[cfg(feature = "pdf")]
fn svg_matrix(m: [f64; 6]) -> String {
    format!(
        "matrix({:.17} {:.17} {:.17} {:.17} {:.17} {:.17})",
        m[0], m[1], m[2], m[3], m[4], m[5]
    )
}

#[cfg(feature = "pdf")]
fn append_p4_svg_defs(
    svg: &mut String,
    stamp: usize,
    brush_uri: &str,
    mask_uri: &str,
    depth_uri: &str,
) {
    use std::fmt::Write as _;
    let filter = format!("oil-p4-{stamp}");
    let brush_page = format!("oil-brush-page-{stamp}");
    let mask_page = format!("oil-mask-page-{stamp}");
    let depth_page = format!("oil-depth-page-{stamp}");
    for (kind, page, uri, dims, mode) in [
        ("brush", &brush_page, brush_uri, [18, 18], "pixelated"),
        ("mask", &mask_page, mask_uri, [18, 18], "pixelated"),
        (
            "depth",
            &depth_page,
            depth_uri,
            [DEPTH_W * 3, DEPTH_H * 3],
            "smooth",
        ),
    ] {
        let source_id = match kind {
            "brush" => format!("oil-brush-source-{stamp}"),
            "mask" => "oil-mask-source".to_owned(),
            _ => "oil-depth-source".to_owned(),
        };
        if kind == "brush" || (kind == "depth" && stamp == 0) {
            let _ = write!(
                svg,
                "<image id=\"{source_id}\" x=\"0\" y=\"0\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\" href=\"{}\" image-rendering=\"{}\" style=\"image-rendering:{}\"/>",
                dims[0],
                dims[1],
                uri,
                if mode == "pixelated" {
                    "optimizeSpeed"
                } else {
                    "optimizeQuality"
                },
                mode
            );
        }
        let _ = write!(svg, "<g id=\"{page}\">");
        for (tri_index, ids) in [[0, 1, 2], [3, 4, 5]].into_iter().enumerate() {
            let tri = unpack_p4_triangle(stamp, ids);
            let dest = tri
                .pos
                .map(|p| [f64::from(p[0]), PAGE as f64 - f64::from(p[1])]);
            let planes = match kind {
                // P4 vertex-shader event 110 assigns vBrushUV = aVertex.zw;
                // fragment-shader event 114 samples both brush and mask with
                // vBrushUV. The source is pinned in capture.json SHA-256
                // 8dbf4a98…; canvas UV is used for depth only on this route.
                "brush" => (
                    &tri.mask_u,
                    &tri.mask_v,
                    FEEDBACK as f64,
                    FEEDBACK as f64,
                    1.0,
                    1.0,
                    true,
                ),
                "mask" => (
                    &tri.mask_u,
                    &tri.mask_v,
                    MASK_W as f64,
                    MASK_H as f64,
                    1.0,
                    1.0,
                    false,
                ),
                _ => (
                    &tri.depth_u,
                    &tri.depth_v,
                    DEPTH_W as f64,
                    DEPTH_H as f64,
                    DEPTH_W as f64,
                    DEPTH_H as f64,
                    false,
                ),
            };
            let matrix = svg_matrix(svg_plane_image_map(
                *planes.0, *planes.1, planes.2, planes.3, planes.4, planes.5, planes.6,
            ));
            let clip = format!("oil-p4-clip-{stamp}-{tri_index}");
            let _ = write!(
                svg,
                "<clipPath id=\"{clip}\" clipPathUnits=\"userSpaceOnUse\" shape-rendering=\"crispEdges\"><polygon shape-rendering=\"crispEdges\" points=\""
            );
            for p in dest {
                let _ = write!(svg, "{:.17},{:.17} ", p[0], p[1]);
            }
            if kind == "mask" {
                // V25 replaces only the nested clamp-copy graph with this
                // direct image node, retaining the captured transform and
                // crisp-edged triangle clip.
                let _ = write!(
                    svg,
                    "\"/></clipPath><g clip-path=\"url(#{clip})\" shape-rendering=\"crispEdges\"><image x=\"0\" y=\"0\" width=\"18\" height=\"18\" preserveAspectRatio=\"none\" href=\"{}\" transform=\"{}\" image-rendering=\"optimizeSpeed\" style=\"image-rendering:pixelated\"/></g>",
                    uri, matrix
                );
            } else {
                let _ = write!(
                    svg,
                    "\"/></clipPath><g clip-path=\"url(#{clip})\"><use href=\"#{source_id}\" transform=\"{}\" image-rendering=\"{}\" style=\"image-rendering:{}\"/></g>",
                    matrix,
                    if mode == "pixelated" {
                        "optimizeSpeed"
                    } else {
                        "optimizeQuality"
                    },
                    mode
                );
            }
        }
        let _ = write!(svg, "</g>");
    }
    let _ = write!(
        svg,
        "<filter id=\"{filter}\" filterUnits=\"userSpaceOnUse\" primitiveUnits=\"userSpaceOnUse\" x=\"0\" y=\"0\" width=\"{PAGE}\" height=\"{PAGE}\" color-interpolation-filters=\"sRGB\"><feImage href=\"#{brush_page}\" x=\"0\" y=\"0\" width=\"{PAGE}\" height=\"{PAGE}\" preserveAspectRatio=\"none\" result=\"brush\" image-rendering=\"optimizeSpeed\" style=\"image-rendering:pixelated\"/><feImage href=\"#{mask_page}\" x=\"0\" y=\"0\" width=\"{PAGE}\" height=\"{PAGE}\" preserveAspectRatio=\"none\" result=\"mask\" image-rendering=\"optimizeSpeed\" style=\"image-rendering:pixelated\"/><feImage href=\"#{depth_page}\" x=\"0\" y=\"0\" width=\"{PAGE}\" height=\"{PAGE}\" preserveAspectRatio=\"none\" result=\"depth\" image-rendering=\"optimizeQuality\" style=\"image-rendering:smooth\"/><feColorMatrix in=\"mask\" type=\"matrix\" values=\"0 0 0 0 1 0 0 0 0 1 0 0 0 0 1 1 0 0 0 0\" result=\"maskRGBA\"/><feColorMatrix in=\"depth\" type=\"matrix\" values=\"0 0 0 0 1 0 0 0 0 1 0 0 0 0 1 1 0 0 0 0\" result=\"depthRGBA\"/><feComposite in=\"maskRGBA\" in2=\"depthRGBA\" operator=\"arithmetic\" k1=\"1\" k2=\"0\" k3=\"0\" k4=\"0\" result=\"maskDepth\"/><feComposite in=\"brush\" in2=\"maskDepth\" operator=\"in\" result=\"stamp\"/></filter>"
    );
}

#[cfg(feature = "pdf")]
fn oilbrush3_v25_svg(prefix: usize) -> String {
    // This is bounded to the captured 15-draw profile and five checked prefix
    // lengths. Texture20 is modeled as transparent after its null allocation;
    // the native capture has no clear of that target before the first P4 draw.
    // The 192x192 canvas below remains a vector/use graph; the separate CPU
    // canvas exists only to derive each source-input 16x16 P3 feedback texture.
    use std::fmt::Write as _;
    assert!([3, 7, 8, 11, 15].contains(&prefix));
    assert_eq!(sha256(ADDPOINTS), ADDPOINT_SHA256);
    assert_eq!(sha256(P3_VBO), P3_VBO_SHA256);
    assert_eq!(sha256(STATE), STATE_SHA256);
    let mask = decode_zlib(MASK_ZLIB, MASK_W * MASK_H, MASK_R8_SHA256);
    let depth = decode_zlib(DEPTH_ZLIB, DEPTH_W * DEPTH_H, DEPTH_R8_SHA256);
    let mut mask18 = vec![0u8; 18 * 18];
    for y in 0..18 {
        for x in 0..18 {
            mask18[y * 18 + x] =
                mask[y.saturating_sub(1).min(15) * MASK_W + x.saturating_sub(1).min(15)];
        }
    }
    let mask_uri = svg_r8_uri(18, 18, &mask18);
    let depth_uri = svg_depth_repeat_uri(&depth);
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{PAGE}\" height=\"{PAGE}\" viewBox=\"0 0 {PAGE} {PAGE}\"><defs><g id=\"oil-canvas-0\"/>"
    );
    let mut canvas = vec![Rgba::default(); PIXELS];
    let blue = Rgba([68, 170, 238, 255]);
    let mut brush = vec![blue; FEEDBACK * FEEDBACK];
    let mut p3 = 0;
    let mut current = 0usize;
    for stamp in 0..prefix {
        if matches!(stamp, 3 | 7 | 11) {
            brush = render_p3(&canvas, read_p3_state(p3), &mask, &depth, &brush);
            assert!(
                brush.iter().all(|p| p.0 == [68, 170, 238, 255]),
                "captured profile P3 source model must produce the checked 16x16 texture"
            );
            p3 += 1;
        }
        let brush_uri = svg_brush_clamped_uri(&brush);
        append_p4_svg_defs(&mut svg, stamp, &brush_uri, &mask_uri, &depth_uri);
        let next = stamp + 1;
        let _ = write!(
            svg,
            "<g id=\"oil-canvas-{next}\"><use href=\"#oil-canvas-{current}\"/><g filter=\"url(#oil-p4-{stamp})\"><rect x=\"0\" y=\"0\" width=\"{PAGE}\" height=\"{PAGE}\" fill=\"#000\" fill-opacity=\"0\"/></g></g>"
        );
        render_p4(&mut canvas, stamp, &brush, &mask, &depth);
        current = next;
    }
    let _ = write!(svg, "</defs><use href=\"#oil-canvas-{current}\"/></svg>");
    svg
}

#[cfg(feature = "pdf")]
fn compare_resvg_prefix(prefix: usize) -> DiffStats {
    let svg = oilbrush3_v25_svg(prefix);
    let tree = resvg::usvg::Tree::from_str(&svg, &Default::default()).expect("generated V25 SVG");
    let mut pix = resvg::tiny_skia::Pixmap::new(PAGE as u32, PAGE as u32).expect("page pixmap");
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pix.as_mut(),
    );
    let native = decode_native_prefix_bundle(NATIVE_PREFIX_BUNDLE);
    let sparse = decode_expected(native[prefix - 1], PREFIX_SHA256[prefix - 1]);
    let actual: Vec<Rgba> = pix
        .data()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| Rgba(*p))
        .collect();
    let mut expected_top = BTreeMap::new();
    for (index, pixel) in sparse {
        let x = index % PAGE;
        let y_bottom = index / PAGE;
        expected_top.insert((PAGE - 1 - y_bottom) * PAGE + x, pixel);
    }
    compare_frame(&actual, &expected_top)
}

#[cfg(all(test, feature = "pdf"))]
mod v25_svg_tests {
    use super::*;

    #[test]
    fn source_svg_matches_native_prefix_support_alpha_and_two_level_rgb_bound() {
        for prefix in [3, 7, 8, 11, 15] {
            let row = compare_resvg_prefix(prefix);
            assert_eq!(row.support, EXPECTED_SUPPORT[prefix - 1], "prefix {prefix}");
            assert_eq!(row.support_symdiff, 0, "prefix {prefix} support");
            assert_eq!(row.max[3], 0, "prefix {prefix} alpha");
            assert!(
                row.max[..3].iter().all(|v| *v <= 2),
                "prefix {prefix} RGB: {:?}",
                row.max
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oilbrush3_v17_public_drawing_source_model_matches_native_prefixes_exactly() {
        let rows = simulate_v17_public_drawing();
        assert_eq!(rows.len(), 15);
        for (i, row) in rows.iter().enumerate() {
            assert_eq!(
                row.support,
                V17_EXPECTED_SUPPORT[i],
                "V17 prefix {} support",
                i + 1
            );
            assert_eq!(
                row.support_symdiff,
                0,
                "V17 prefix {} support symmetric difference",
                i + 1
            );
            assert_eq!(row.diff_pixels, 0, "V17 prefix {} pixels", i + 1);
            assert_eq!(row.sums, [0; 4], "V17 prefix {} channel sums", i + 1);
            assert_eq!(row.max, [0; 4], "V17 prefix {} channel maxima", i + 1);
        }
    }

    #[test]
    fn oilbrush3_v23_source_model_matches_native_prefixes_exactly() {
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
            assert_eq!(row.diff_pixels, 0, "V23 prefix {} pixels", i + 1);
            assert_eq!(row.sums, [0; 4], "V23 prefix {} channel sums", i + 1);
            assert_eq!(row.max, [0; 4], "V23 prefix {} channel maxima", i + 1);
        }
    }
}
