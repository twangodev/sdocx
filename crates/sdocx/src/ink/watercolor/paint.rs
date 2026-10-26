//! Saved WaterColor stroke -> native six-lane dabs -> generated SVG -> resvg frame.
//! RNE is a named, V13-captured host position profile; this is not universal GLSL behavior.
use super::{
    DrawableVersion, MathProfile, NEG_TAU, PHASE_STEP, SavedStrokeInput, TAU, host_math,
    sample_saved_stroke, sample_saved_stroke_with_math,
};
use crate::Point;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::Value;
use sha2::{Digest, Sha256};

const PAINT: &str = include_str!("../../../../../conformance/watercolor-paint-native.json");
const GEOMETRY: &str = include_str!("../../../../../conformance/watercolor-native.json");
const STREAM: &[u8] = include_bytes!("../../../../../conformance/watercolor-native.bin");
const PATTERN_PNG: &[u8] = include_bytes!("../../../../../conformance/watercolor-pattern.png");
const DENSE: &str = include_str!("../../../../../conformance/watercolor-dense-native.json");
const DENSE_SPARSE: &[u8] =
    include_bytes!("../../../../../conformance/watercolor-dense-sparse.bin");
const SIZE: usize = 256;
const DENSE_STREAM_SHA256: &str =
    "483159bd8fad294ca07d62f324ee0fa84e027d88bda1a78656e537356df21e8b";

// PenCommon SetPenData normalizes ARGB; WaterColor getColor applies this FMA
// before passing alpha to the shader. This is separate from mask blending.
fn input_color(argb: u32) -> [f32; 4] {
    let lane = |shift: u32| ((argb >> shift) & 0xff_u32) as f32 / 255.0;
    [
        lane(16),
        lane(8),
        lane(0),
        lane(24).mul_add(f32::from_bits(0x3f66_6666), f32::from_bits(0x3dcc_cccd)),
    ]
}

#[derive(Clone, Copy)]
struct Vertex {
    x: f64,
    y: f64,
    u: f64,
    v: f64,
}
fn sha256(b: &[u8]) -> String {
    format!("{:x}", Sha256::digest(b))
}
fn round_shift_even(v: u32, s: u32) -> u32 {
    if s == 0 {
        return v;
    }
    if s >= 32 {
        return 0;
    }
    let b = v >> s;
    let r = v & ((1 << s) - 1);
    let h = 1 << (s - 1);
    b + u32::from(r > h || (r == h && b & 1 != 0))
}
fn f32_to_f16_rne(x: f32) -> u16 {
    let b = x.to_bits();
    let sign = ((b >> 16) & 0x8000) as u16;
    let e = ((b >> 23) & 255) as i32;
    let f = b & 0x7fffff;
    if e == 255 {
        return sign | if f == 0 { 0x7c00 } else { 0x7e00 };
    }
    if e == 0 {
        return sign;
    }
    let e = e - 127;
    let sig = 0x800000 | f;
    if e > 15 {
        return sign | 0x7c00;
    }
    if e >= -14 {
        let mut r = round_shift_even(sig, 13);
        let mut he = e + 15;
        if r == 0x800 {
            r = 0x400;
            he += 1
        }
        if he >= 31 {
            return sign | 0x7c00;
        }
        return sign | ((he as u16) << 10) | (r as u16 & 0x3ff);
    }
    if e < -25 {
        return sign;
    }
    sign | round_shift_even(sig, (-e - 1) as u32) as u16
}
fn half_rne(x: f32) -> f32 {
    f16_to_f32(f32_to_f16_rne(x))
}

fn sample_unorm8_half_rtz(byte: u8) -> f32 {
    // Captured Mesa sampler: normalize in F32, narrow toward zero, then widen.
    let normalized = f32::from(byte) * f32::from_bits(0x3b80_8081);
    let mut bits = f32_to_f16_rne(normalized);
    if f16_to_f32(bits) > normalized {
        bits -= 1;
    }
    f16_to_f32(bits)
}
fn f16_to_f32(b: u16) -> f32 {
    let s = ((b & 0x8000) as u32) << 16;
    let e = ((b >> 10) & 31) as i32;
    let f = (b & 1023) as u32;
    let bits = match e {
        0 if f == 0 => s,
        0 => {
            let mut m = f;
            let mut q = -14;
            while m & 0x400 == 0 {
                m <<= 1;
                q -= 1
            }
            m &= 1023;
            s | (((q + 127) as u32) << 23) | (m << 13)
        }
        31 => s | 0x7f800000 | (f << 13),
        _ => s | (((e - 15 + 127) as u32) << 23) | (f << 13),
    };
    f32::from_bits(bits)
}
fn half_mul(a: f32, b: f32) -> f32 {
    half_rne(a * b)
}
fn half_add(a: f32, b: f32) -> f32 {
    half_rne(a + b)
}
fn vertices(r: [f32; 6]) -> [Vertex; 4] {
    let [x, y, lane2, lane3, radius, _] = r;
    let (x, y, lane2, lane3, radius) = (
        half_rne(x),
        half_rne(y),
        half_rne(lane2),
        half_rne(lane3),
        half_rne(radius),
    );
    let rect = [
        (-1.0f32, -1.0f32, 0.0f32, 1.0f32),
        (-1.0, 1.0, 0.0, 0.0),
        (1.0, -1.0, 1.0, 1.0),
        (1.0, 1.0, 1.0, 0.0),
    ];
    let mut o = [Vertex {
        x: 0.0,
        y: 0.0,
        u: 0.0,
        v: 0.0,
    }; 4];
    for (i, (rx, ry, u, v)) in rect.into_iter().enumerate() {
        let lx = half_mul(half_rne(rx), radius);
        let ly = half_mul(half_rne(ry), radius);
        // Captured program-3 attribute order: lane 3 multiplies local X/Y on the
        // transform.w path; lane 2 multiplies local X/Y on transform.z path.
        let xx = half_mul(lane3, lx);
        let xy = half_mul(lane2, lx);
        let yx = half_mul(lane2, ly);
        let yy = half_mul(lane3, ly);
        let mx = half_add(half_add(xx, x), half_rne(-yx));
        let my = half_add(half_add(xy, y), yy);
        let cx = half_mul(half_rne(1.0 / 128.0), mx);
        let cy = half_mul(half_rne(1.0 / 128.0), my);
        o[i] = Vertex {
            x: 128.0 + 128.0 * f64::from(cx),
            y: 128.0 - 128.0 * f64::from(cy),
            u: f64::from(u),
            v: f64::from(v),
        };
    }
    o
}

/// Build the screen-space quad from the captured draw MVP. The pinned Mesa NIR
/// lowers this mediump shader's model and projection arithmetic to f16, then
/// widens `gl_Position`; mirror its multiply/add ordering before viewport map.
/// The historical `vertices` helper above models the older centered profile.
fn vertices_with_projection(r: [f32; 6], matrix: [f32; 16]) -> [Vertex; 4] {
    let [x, y, lane2, lane3, radius, _] = r;
    let (x, y, lane2, lane3, radius) = (
        half_rne(x),
        half_rne(y),
        half_rne(lane2),
        half_rne(lane3),
        half_rne(radius),
    );
    let rect = [
        (-1.0f32, -1.0f32, 0.0f32, 1.0f32),
        (-1.0, 1.0, 0.0, 0.0),
        (1.0, -1.0, 1.0, 1.0),
        (1.0, 1.0, 1.0, 0.0),
    ];
    rect.map(|(rx, ry, u, v)| {
        let lx = half_mul(half_rne(rx), radius);
        let ly = half_mul(half_rne(ry), radius);
        let xx = half_mul(lane3, lx);
        let xy = half_mul(lane2, lx);
        let yx = half_mul(lane2, ly);
        let yy = half_mul(lane3, ly);
        let mx = half_add(half_add(xx, x), half_rne(-yx));
        let my = half_add(half_add(xy, y), yy);

        // Captured Mesa NIR source BLAKE3
        // 1fb6ed1d6f0d83fc69a07fd1fb1d803d8fb7a2055c5e79d19a9e71c7cf3f7c95
        // (debug stderr lines 1262-1357) shows f16 column products, translation
        // add, then other-axis add; `gl_Position` widens only afterward.
        let clip_x = half_add(
            half_add(half_mul(half_rne(matrix[0]), mx), half_rne(matrix[12])),
            half_mul(half_rne(matrix[4]), my),
        );
        let clip_y = half_add(
            half_add(half_mul(half_rne(matrix[1]), mx), half_rne(matrix[13])),
            half_mul(half_rne(matrix[5]), my),
        );
        let bottom_x = (clip_x + 1.0_f32) * 128.0_f32;
        let bottom_y = (clip_y + 1.0_f32) * 128.0_f32;
        Vertex {
            x: f64::from(bottom_x),
            y: f64::from(256.0_f32 - bottom_y),
            u: f64::from(u),
            v: f64::from(v),
        }
    })
}
fn mesa_linear_coeff(pos: [(f32, f32); 3], values: [f32; 3]) -> (f32, f32, f32) {
    let [(x0, y0), (x1, y1), (x2, y2)] = pos;
    let [a0, a1, a2] = values;
    let x0_center = x0 - 0.5;
    let y0_center = y0 - 0.5;
    let dx01 = x0 - x1;
    let dy01 = y0 - y1;
    let dx20 = x2 - x0;
    let dy20 = y2 - y0;
    let e = dx01 * dy20;
    let f = dy01 * dx20;
    let area = e - f;
    let ooa = 1.0 / area;
    let dy20_ooa = dy20 * ooa;
    let dy01_ooa = dy01 * ooa;
    let dx20_ooa = dx20 * ooa;
    let dx01_ooa = dx01 * ooa;
    let da01 = a0 - a1;
    let da20 = a2 - a0;
    let dadx_left = da01 * dy20_ooa;
    let dadx_right = da20 * dy01_ooa;
    let dadx = dadx_left - dadx_right;
    let dady_left = da20 * dx01_ooa;
    let dady_right = da01 * dx20_ooa;
    let dady = dady_left - dady_right;
    let origin_x = dadx * x0_center;
    let origin_y = dady * y0_center;
    let origin_offset = origin_x + origin_y;
    let origin = a0 - origin_offset;
    (origin, dadx, dady)
}

fn triangle_image_transform(v: [Vertex; 4], t: [usize; 3]) -> [f64; 6] {
    let pos: [(f32, f32); 3] = t.map(|i| (v[i].x as f32, 256.0_f32 - v[i].y as f32));
    let values_u = t.map(|i| v[i].u as f32);
    let values_v = t.map(|i| v[i].v as f32);
    let (u0, ux, uy) = mesa_linear_coeff(pos, values_u);
    let (v0, vx, vy) = mesa_linear_coeff(pos, values_v);
    // Convert the integer-index setup plane to physical screen pixel centers.
    let uc = (u0 - ux * 0.5) - uy * 0.5;
    let vc = (v0 - vx * 0.5) - vy * 0.5;
    let sx = f64::from(ux);
    let ky = f64::from(vx);
    let kx = f64::from(uy);
    let sy = f64::from(vy);
    let tx = f64::from(uc);
    let ty = f64::from(vc);
    let det = sx * sy - kx * ky;
    assert!(
        det.is_finite() && det != 0.0,
        "non-invertible Mesa UV plane"
    );
    let inverse_det = 1.0 / det;
    let screen_sx = (sy * inverse_det) as f32;
    let screen_ky = (-ky * inverse_det) as f32;
    let screen_kx = (-kx * inverse_det) as f32;
    let screen_sy = (sx * inverse_det) as f32;
    let screen_tx = ((kx * ty - sy * tx) * inverse_det) as f32;
    let screen_ty = ((ky * tx - sx * ty) * inverse_det) as f32;
    [
        f64::from(screen_sx),
        f64::from(-screen_ky),
        f64::from(screen_kx),
        f64::from(-screen_sy),
        f64::from(screen_tx),
        f64::from(256.0_f32 - screen_ty),
    ]
}
fn f(v: f64) -> String {
    format!("{v:.17}")
}
fn points(v: [Vertex; 4], t: [usize; 3]) -> String {
    t.map(|i| format!("{},{}", f(v[i].x), f(v[i].y))).join(" ")
}
fn matrix(m: [f64; 6]) -> String {
    format!("matrix({})", m.map(f).join(" "))
}
fn unorm(x: f32) -> u8 {
    (x.clamp(0.0, 1.0) * 255.0)
        .round_ties_even()
        .clamp(0.0, 255.0) as u8
}
fn mul_norm8(a: u8, b: u8) -> u8 {
    let mut x = u32::from(a) * u32::from(b);
    x += x >> 8;
    x += 128;
    (x >> 8).min(255) as u8
}
fn source_shader_rgba(g: u8, color: [f32; 4], mask_r: u8) -> ([u8; 4], [u8; 4]) {
    // Program-4 non-eraser NIR formula. Texture conversion is source-proven
    // RTZ. Fragment half-ALU RNE is a model validated by these frames; V13
    // observes vertex-side rounding only, not fragment worker MXCSR.
    let sample = sample_unorm8_half_rtz(g);
    let threshold = half_rne(0.7);
    let max_alpha = half_rne(0.8);
    let input = color.map(half_rne);
    let (rgb, alpha) = if sample < threshold {
        let ratio = half_rne(sample / threshold);
        let alpha = half_mul(half_mul(ratio, max_alpha), input[3]);
        ([input[0], input[1], input[2]], alpha)
    } else {
        assert_eq!(
            mask_r, 255,
            "high branch requires saturated-red mask evidence"
        );
        let mut delta = half_rne(sample + half_rne(-threshold));
        delta = half_mul(delta, half_rne(0.5));
        delta = half_mul(delta, sample_unorm8_half_rtz(mask_r));
        (
            [
                half_add(input[0], delta),
                half_add(input[1], delta),
                half_add(input[2], delta),
            ],
            half_mul(max_alpha, input[3]),
        )
    };
    let source = [unorm(rgb[0]), unorm(rgb[1]), unorm(rgb[2]), unorm(alpha)];
    let blended = [
        mul_norm8(source[0], source[3]),
        mul_norm8(source[1], source[3]),
        mul_norm8(source[2], source[3]),
        source[3],
    ];
    (source, blended)
}
fn resvg_mul_u8(u: u8, a: u8) -> u8 {
    let x = f32::from(u) / 255.0;
    let y = f32::from(a) / 255.0;
    (((((1.0f32 * x) * y) * 255.0) as u32).min(255)) as u8
}
fn encoded_unpremultiplied_rgb(g: u8, c: usize, color: [f32; 4], high_branch_admitted: bool) -> u8 {
    if sample_unorm8_half_rtz(g) >= half_rne(0.7) && !high_branch_admitted {
        return 0;
    }
    let (source, blended) = source_shader_rgba(g, color, 255);
    let a = source[3];
    let want = blended[c];
    (0..=255u16)
        .map(|u| u as u8)
        .filter(|&u| resvg_mul_u8(u, a) == want)
        .min_by_key(|&u| (u.abs_diff(source[c]), u))
        .expect("source-derived resvg U8 preimage")
}
fn table(channel: usize, color: [f32; 4], high_branch_admitted: bool) -> String {
    (0..=255u16)
        .map(|g| {
            let sample = sample_unorm8_half_rtz(g as u8);
            let high = sample >= half_rne(0.7);
            let value = if high && !high_branch_admitted {
                0
            } else if channel == 3 {
                source_shader_rgba(g as u8, color, 255).0[3]
            } else {
                encoded_unpremultiplied_rgb(g as u8, channel, color, high_branch_admitted)
            };
            let normalized = (f32::from(value) + 0.5) / 255.0;
            assert_eq!(
                (normalized * 255.0) as u8,
                value,
                "component-transfer midpoint must survive resvg U8 truncation"
            );
            format!("{normalized:.12}")
        })
        .collect::<Vec<_>>()
        .join(" ")
}
fn filter_program(color: [f32; 4], stamp_count: usize, high_branch_admitted: bool) -> String {
    assert!(stamp_count > 0 && stamp_count <= 480);
    let digits = if stamp_count >= 100 { 3 } else { 2 };
    let id = |prefix: &str, n: usize| format!("{prefix}-{n:0digits$}");
    let mut s = String::from(
        "<filter id=\"native-final-low-density\" x=\"0\" y=\"0\" width=\"256\" height=\"256\" filterUnits=\"userSpaceOnUse\" primitiveUnits=\"userSpaceOnUse\" color-interpolation-filters=\"sRGB\">",
    );
    s.push_str(&format!("<feImage href=\"#{}\" image-rendering=\"optimizeSpeed\" style=\"image-rendering:pixelated\" x=\"0\" y=\"0\" width=\"256\" height=\"256\" preserveAspectRatio=\"none\" result=\"{}\"/>", id("stamp",0),id("acc",0)));
    for i in 1..stamp_count {
        s.push_str(&format!("<feImage href=\"#{}\" image-rendering=\"optimizeSpeed\" style=\"image-rendering:pixelated\" x=\"0\" y=\"0\" width=\"256\" height=\"256\" preserveAspectRatio=\"none\" result=\"{}\"/><feComposite in=\"{}\" in2=\"{}\" operator=\"arithmetic\" k1=\"0\" k2=\"1\" k3=\"1\" k4=\"0\" result=\"{}\"/>", id("stamp",i),id("src",i),id("acc",i-1),id("src",i),id("acc",i)));
    }
    s.push_str(&format!("<feColorMatrix in=\"{}\" type=\"matrix\" values=\"0 1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 0 0 0 1\" result=\"mask-g-opaque\"/><feComponentTransfer in=\"mask-g-opaque\" result=\"source-color-u8\">",id("acc",stamp_count-1)));
    for (name, ch) in [("R", 0), ("G", 1), ("B", 2)] {
        s.push_str(&format!(
            "<feFunc{name} type=\"discrete\" tableValues=\"{}\"/>",
            table(ch, color, high_branch_admitted)
        ));
    }
    s.push_str("</feComponentTransfer><feComponentTransfer in=\"mask-g-opaque\" result=\"shader-alpha-unorm\">");
    for name in ["R", "G", "B"] {
        s.push_str(&format!(
            "<feFunc{name} type=\"discrete\" tableValues=\"{}\"/>",
            table(3, color, high_branch_admitted)
        ));
    }
    s.push_str(&format!("</feComponentTransfer><feColorMatrix in=\"shader-alpha-unorm\" type=\"matrix\" values=\"0 0 0 0 1 0 0 0 0 1 0 0 0 0 1 1 0 0 0 0\" result=\"shader-alpha-white\"/><feComposite in=\"source-color-u8\" in2=\"shader-alpha-white\" operator=\"arithmetic\" k1=\"1\" k2=\"0\" k3=\"0\" k4=\"0\" result=\"native-paint-premultiplied\"/><feColorMatrix in=\"{}\" type=\"matrix\" values=\"0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0 0 0 0\" result=\"mask-red-gate-source\"/><feComponentTransfer in=\"mask-red-gate-source\" result=\"mask-red-gate\"><feFuncA type=\"discrete\" tableValues=\"0 ",id("acc",stamp_count-1)));
    s.push_str(&vec!["1"; 255].join(" "));
    s.push_str("\"/></feComponentTransfer><feComposite in=\"native-paint-premultiplied\" in2=\"mask-red-gate\" operator=\"in\" result=\"native-final-paint\"/></filter>");
    s
}
fn atlas(alpha: f32) -> String {
    let mut image =
        resvg::tiny_skia::Pixmap::decode_png(PATTERN_PNG).expect("pinned original tip PNG");
    assert_eq!((image.width(), image.height()), (SIZE as u32, SIZE as u32));
    for px in image.data_mut().as_chunks_mut::<4>().0 {
        assert_eq!(px[3], 255, "atlas transform expects opaque source PNG");
        // The mask fragment multiplies its lowp sample and lane alpha in half
        // precision. F32 preapplication would incorrectly map G=250 to 2 for
        // alpha=0.006; the source-ordered half-RNE model maps it to 1.
        let green = half_mul(sample_unorm8_half_rtz(px[1]), half_rne(alpha));
        px[1] = (green * 255.0).round_ties_even().clamp(0.0, 255.0) as u8;
    }
    format!(
        "data:image/png;base64,{}",
        STANDARD.encode(image.encode_png().expect("PNG encode"))
    )
}
fn make_svg(rows: &[[f32; 6]], color: [f32; 4], high_branch_admitted: bool) -> String {
    make_svg_with_projection_translation(rows, color, high_branch_admitted, [0.0, 0.0])
}

fn make_svg_with_projection_translation(
    rows: &[[f32; 6]],
    color: [f32; 4],
    high_branch_admitted: bool,
    translation_ndc: [f32; 2],
) -> String {
    make_svg_inner(rows, color, high_branch_admitted, None, translation_ndc)
}

fn make_svg_with_projection_matrix(
    rows: &[[f32; 6]],
    color: [f32; 4],
    high_branch_admitted: bool,
    matrix: [f32; 16],
) -> String {
    make_svg_inner(rows, color, high_branch_admitted, Some(matrix), [0.0, 0.0])
}

fn make_svg_inner(
    rows: &[[f32; 6]],
    color: [f32; 4],
    high_branch_admitted: bool,
    projection: Option<[f32; 16]>,
    translation_ndc: [f32; 2],
) -> String {
    assert!(!rows.is_empty() && rows.len() <= 480);
    assert!(
        rows.len() <= 15 || high_branch_admitted,
        "dense native mask gate must be proven before high branch output"
    );
    let digits = if rows.len() >= 100 { 3 } else { 2 };
    let mut svg = String::from(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"256\" height=\"256\" viewBox=\"0 0 256 256\" color-interpolation=\"sRGB\"><defs>",
    );
    svg.push_str(&filter_program(color, rows.len(), high_branch_admitted));
    let mut atlases = std::collections::BTreeMap::<u32, String>::new();
    for (i, row) in rows.iter().copied().enumerate() {
        let abits = row[5].to_bits();
        assert!(
            abits == 0.04_f32.to_bits() || abits == 0.006_f32.to_bits(),
            "captured source lane alpha changed"
        );
        let uri = atlases
            .entry(abits)
            .or_insert_with(|| atlas(row[5]))
            .clone();
        let v = if let Some(matrix) = projection {
            vertices_with_projection(row, matrix)
        } else {
            let mut v = vertices(row);
            // `vertices` maps the historical centered projection. A legacy
            // NDC translation is kept for callers that use that profile.
            for vertex in &mut v {
                vertex.x += 128.0 * f64::from(translation_ndc[0]);
                vertex.y -= 128.0 * f64::from(translation_ndc[1]);
            }
            v
        };
        let ts = [[0usize, 1, 2], [2, 1, 3]];
        for (k, t) in ts.into_iter().enumerate() {
            let id = format!("native-triangle-{i:0digits$}-{k}");
            svg.push_str(&format!("<clipPath id=\"{id}\" clipPathUnits=\"userSpaceOnUse\"><polygon points=\"{}\" shape-rendering=\"crispEdges\"/></clipPath>",points(v,t)));
        }
        svg.push_str(&format!("<g id=\"stamp-{i:0digits$}\">"));
        for (k, t) in ts.into_iter().enumerate() {
            svg.push_str(&format!("<g clip-path=\"url(#native-triangle-{i:0digits$}-{k})\"><image x=\"0\" y=\"0\" width=\"1\" height=\"1\" preserveAspectRatio=\"none\" image-rendering=\"optimizeSpeed\" style=\"image-rendering:pixelated\" href=\"{uri}\" transform=\"{}\"/></g>",matrix(triangle_image_transform(v,t))));
        }
        svg.push_str("</g>");
    }
    svg.push_str("</defs><rect width=\"256\" height=\"256\" filter=\"url(#native-final-low-density)\"/></svg>");
    svg
}
fn inputs() -> (Vec<[f32; 6]>, Value, usize, u32) {
    let fixture: Value = serde_json::from_str(PAINT).unwrap();
    let geom: Value = serde_json::from_str(GEOMETRY).unwrap();
    let c = geom["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["case_id"] == fixture["source"]["geometry_case_id"])
        .unwrap();
    let x = &c["input"];
    let points: Vec<Point> = x["points_xy"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| Point {
            x: p[0].as_f64().unwrap(),
            y: p[1].as_f64().unwrap(),
        })
        .collect();
    let pressure: Vec<f64> = x["pressure"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect();
    let times: Vec<i64> = x["timestamps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_i64().unwrap())
        .collect();
    let canvas = &x["canvas"];
    let version =
        DrawableVersion::from_observed_class(c["selected_drawable_class"].as_str().unwrap())
            .unwrap();
    let argb = u32::from_str_radix(
        x["color_argb"].as_str().unwrap().trim_start_matches("0x"),
        16,
    )
    .unwrap();
    let rows = sample_saved_stroke(
        SavedStrokeInput {
            points: &points,
            pressures: &pressure,
            timestamps: &times,
            pen_size: x["pen_size"].as_f64().unwrap() as f32,
            color_argb: argb,
            color_type: x["color_type"].as_u64().unwrap() as u8,
            stroke_type: x["stroke_type"].as_u64().unwrap() as u8,
            curve_enabled: x["curve_enabled"].as_bool().unwrap(),
            tool_type: x["tool_type"].as_u64().unwrap() as u8,
            canvas: [
                canvas[0].as_u64().unwrap() as u32,
                canvas[1].as_u64().unwrap() as u32,
            ],
        },
        version,
    )
    .unwrap();
    let offset = c["stream_offset_bytes"].as_u64().unwrap() as usize;
    for (i, r) in rows.iter().enumerate() {
        for (j, v) in r.iter().enumerate() {
            let k = offset + (i * 6 + j) * 4;
            assert_eq!(
                v.to_bits(),
                f32::from_le_bytes(STREAM[k..k + 4].try_into().unwrap()).to_bits(),
                "native six-lane record {i}:{j}"
            );
        }
    }
    (rows, fixture, offset, argb)
}

fn dense_repeat32_rows(base: &[[f32; 6]]) -> Vec<[f32; 6]> {
    assert_eq!(
        base.len(),
        15,
        "dense profile requires the pinned natural 15-row source"
    );
    let mut rows = Vec::with_capacity(15 * 32);
    let mut phase = 0.0_f32;
    for _repeat in 0..32 {
        for source in base {
            let (sin, cos) = host_math::sincosf_bounded(phase)
                .expect("captured repeat-32 phase stays in the pinned host sincosf domain");
            let mut row = *source;
            row[2] = sin;
            row[3] = cos;
            rows.push(row);
            let next = phase + PHASE_STEP;
            phase = if next > TAU { next + NEG_TAU } else { next };
        }
    }
    assert_eq!(rows.len(), 480);
    for (i, row) in rows.iter().enumerate() {
        let source = &base[i % base.len()];
        for lane in [0, 1, 4, 5] {
            assert_eq!(
                row[lane].to_bits(),
                source[lane].to_bits(),
                "repeat source lane {i}:{lane}"
            );
        }
    }
    rows
}

fn serialize_stream(rows: &[[f32; 6]]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(rows.len() * 24);
    for row in rows {
        for lane in row {
            bytes.extend_from_slice(&lane.to_le_bytes());
        }
    }
    bytes
}

fn dense_sparse_bytes(offset: usize, len: usize, expected_sha: &str) -> &[u8] {
    let end = offset.checked_add(len).expect("sparse range overflow");
    let bytes = DENSE_SPARSE
        .get(offset..end)
        .expect("dense sparse oracle bounds");
    assert_eq!(sha256(bytes), expected_sha, "dense sparse segment hash");
    bytes
}

fn assert_high_branch_mask_gate(fixture: &Value) {
    let gate = &fixture["material"]["high_branch_mask_invariant"];
    let offset = gate["offset"].as_u64().unwrap() as usize;
    let len = gate["length_bytes"].as_u64().unwrap() as usize;
    let bytes = dense_sparse_bytes(offset, len, gate["sha256"].as_str().unwrap());
    assert_eq!(bytes.len() % 4, 0);
    assert_eq!(
        bytes.len() / 4,
        gate["entry_count"].as_u64().unwrap() as usize
    );
    let min_g = gate["threshold_g_u8_min"].as_u64().unwrap() as u8;
    let required_r = gate["all_red_u8"].as_u64().unwrap() as u8;
    let mut previous = None;
    for record in bytes.as_chunks::<4>().0 {
        let index = u16::from_le_bytes([record[0], record[1]]);
        let r = record[2];
        let g = record[3];
        assert!(
            previous.is_none_or(|p| p < index),
            "high branch records sorted and unique"
        );
        previous = Some(index);
        assert_eq!(r, required_r, "high branch requires native mask R=255");
        assert!(
            g >= min_g,
            "high branch fixture includes a below-threshold G"
        );
    }
    assert_eq!(
        bytes.len() / 4,
        42,
        "captured high-G native mask pixel count"
    );
}

fn assert_dense_final_frame(actual: &[u8], fixture: &Value) {
    assert_eq!(actual.len(), SIZE * SIZE * 4);
    let native = &fixture["material"]["native_final_sparse"];
    let offset = native["offset"].as_u64().unwrap() as usize;
    let len = native["length_bytes"].as_u64().unwrap() as usize;
    let sparse = dense_sparse_bytes(offset, len, native["sha256"].as_str().unwrap());
    assert_eq!(sparse.len() % 6, 0);
    assert_eq!(sparse.len() / 6, 226, "captured final image support count");
    let mut expected = vec![0_u8; SIZE * SIZE * 4];
    let mut previous = None;
    for record in sparse.as_chunks::<6>().0 {
        let index = u16::from_le_bytes([record[0], record[1]]) as usize;
        assert!(index < SIZE * SIZE);
        assert!(
            previous.is_none_or(|p| p < index),
            "final sparse records sorted and unique"
        );
        previous = Some(index);
        expected[index * 4..index * 4 + 4].copy_from_slice(&record[2..6]);
    }
    let differing_bytes = actual.iter().zip(&expected).filter(|(a, b)| a != b).count();
    assert_eq!(
        differing_bytes, 0,
        "dense frame must match at sparse and zero pixels"
    );
    assert_eq!(
        sha256(actual),
        fixture["material"]["native_final_frame_sha256"]
            .as_str()
            .unwrap()
    );
}

fn assert_dense_source_color(rows: &[[f32; 6]], fixture: &Value, color: [f32; 4], argb: u32) {
    assert_eq!(rows.len(), 480);
    assert_eq!(fixture["source"]["record_count"].as_u64(), Some(480));
    assert_eq!(fixture["source"]["repeat_count"].as_u64(), Some(32));
    assert_eq!(
        fixture["source"]["controlled_addpoint_calls"].as_u64(),
        Some(480)
    );
    assert_eq!(
        fixture["source"]["stream_sha256"].as_str(),
        Some(DENSE_STREAM_SHA256)
    );
    assert_eq!(
        DENSE_SPARSE.len(),
        fixture["material"]["sparse_container_bytes"]
            .as_u64()
            .unwrap() as usize
    );
    assert_eq!(
        sha256(DENSE_SPARSE),
        fixture["material"]["sparse_container_sha256"]
            .as_str()
            .unwrap()
    );
    assert_eq!(
        sha256(&serialize_stream(rows)),
        DENSE_STREAM_SHA256,
        "regenerated native stream"
    );
    let argb_text = fixture["source"]["argb"].as_str().unwrap();
    assert_eq!(
        argb,
        u32::from_str_radix(argb_text.trim_start_matches("0x"), 16).unwrap()
    );
    assert_eq!(
        sha256(PATTERN_PNG),
        fixture["material"]["pattern_png_sha256"]
    );
    let source = serde_json::from_str::<Value>(PAINT).unwrap();
    let expected_argb = u32::from_str_radix(
        source["input_context"]["color_argb"]
            .as_str()
            .unwrap()
            .trim_start_matches("0x"),
        16,
    )
    .unwrap();
    assert_eq!(
        argb, expected_argb,
        "dense profile derives color from existing baseline fixture"
    );
    let colors = source["material"]["input_color_uniform_f32"]
        .as_array()
        .unwrap();
    for lane in 0..4 {
        assert_eq!(
            color[lane].to_bits(),
            (colors[lane].as_f64().unwrap() as f32).to_bits()
        );
    }
    assert_eq!(
        fixture["material"]["threshold_f32"].as_f64().unwrap() as f32,
        0.7_f32
    );
    assert_eq!(
        fixture["material"]["max_alpha_f32"].as_f64().unwrap() as f32,
        0.8_f32
    );
}
#[test]
fn watercolor_getter_matches_all_native_alpha_controls() {
    let fixture: Value = serde_json::from_str(PAINT).unwrap();
    let control = &fixture["color_getter_native"];
    assert_eq!(control["count"], 256);
    let encoded = control["alpha_f32_le_hex"].as_str().unwrap();
    let bytes: Vec<u8> = encoded
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            let digit = |b: u8| match b {
                b'0'..=b'9' => b - b'0',
                b'a'..=b'f' => b - b'a' + 10,
                _ => panic!("invalid captured float encoding"),
            };
            digit(pair[0]) * 16 + digit(pair[1])
        })
        .collect();
    assert_eq!(bytes.len(), 256 * 4);
    assert_eq!(
        sha256(&bytes),
        control["alpha_f32_sha256"].as_str().unwrap()
    );
    let rgb: Vec<u32> = control["rgb_f32_bits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|lane| {
            u32::from_str_radix(lane.as_str().unwrap().trim_start_matches("0x"), 16).unwrap()
        })
        .collect();
    let source_rgb = u32::from_str_radix(
        control["source_rgb"]
            .as_str()
            .unwrap()
            .trim_start_matches("0x"),
        16,
    )
    .unwrap();
    for alpha in 0..=255_u32 {
        let actual = input_color((alpha << 24) | source_rgb);
        assert_eq!(
            actual[..3].iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            rgb
        );
        let index = alpha as usize * 4;
        let expected = u32::from_le_bytes(bytes[index..index + 4].try_into().unwrap());
        assert_eq!(actual[3].to_bits(), expected, "source alpha {alpha}");
    }
}

#[test]
fn watercolor_v1_generated_svg_matches_native_full_frame() {
    let (rows, fixture, offset, argb) = inputs();
    let source_color = input_color(argb);
    let geometry: Value = serde_json::from_str(GEOMETRY).unwrap();
    assert_eq!(
        sha256(GEOMETRY.as_bytes()),
        fixture["source"]["geometry_fixture_sha256"]
            .as_str()
            .unwrap()
    );
    assert_eq!(
        sha256(STREAM),
        geometry["stream"]["sha256"].as_str().unwrap()
    );
    assert_eq!(
        fixture["source"]["dso_sha256"].as_str(),
        geometry["provenance"]["native_libraries_sha256"]["libSPenWaterColorBrush.so"].as_str()
    );
    assert_eq!(
        sha256(PATTERN_PNG),
        fixture["material"]["pattern_png_sha256"].as_str().unwrap()
    );
    assert_eq!(
        fixture["position_rounding_profile"]["rounding_control"].as_u64(),
        Some(0)
    );
    let material = &fixture["material"];
    let pattern = resvg::tiny_skia::Pixmap::decode_png(PATTERN_PNG).unwrap();
    assert_eq!(
        sha256(pattern.data()),
        material["pattern_rgba8_sha256"].as_str().unwrap()
    );
    let colors = material["input_color_uniform_f32"].as_array().unwrap();
    for i in 0..4 {
        assert_eq!(
            source_color[i].to_bits(),
            (colors[i].as_f64().unwrap() as f32).to_bits()
        );
    }
    assert_eq!(material["threshold_f32"].as_f64().unwrap() as f32, 0.7f32);
    assert_eq!(material["max_alpha_f32"].as_f64().unwrap() as f32, 0.8f32);
    assert_eq!(
        rows.len(),
        fixture["source"]["instance_count"].as_u64().unwrap() as usize
    );
    let svg = make_svg(&rows, source_color, false);
    assert_eq!(svg.matches("<polygon points=").count(), 30);
    assert_eq!(svg.matches("<image ").count(), 30);
    assert!(svg.contains("feComposite in=\"acc-13\" in2=\"src-14\""));
    let tree =
        resvg::usvg::Tree::from_str(&svg, &Default::default()).expect("generated source SVG");
    let mut pix = resvg::tiny_skia::Pixmap::new(SIZE as u32, SIZE as u32).unwrap();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pix.as_mut(),
    );
    let actual = pix.data();
    assert_eq!(
        sha256(actual),
        fixture["native_frame"]["top_left_sha256"].as_str().unwrap()
    );
    let hex = fixture["native_frame"]["sparse_entries_hex"]
        .as_str()
        .unwrap();
    let sparse: Vec<u8> = hex
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| {
            let d = |b: u8| if b <= b'9' { b - b'0' } else { b - b'a' + 10 };
            d(p[0]) * 16 + d(p[1])
        })
        .collect();
    assert_eq!(sparse.len(), 206 * 6);
    let mut expected = vec![0u8; SIZE * SIZE * 4];
    let mut last = None;
    for e in sparse.as_chunks::<6>().0 {
        let i = u16::from_le_bytes([e[0], e[1]]) as usize;
        assert!(last.is_none_or(|p| p < i));
        last = Some(i);
        expected[i * 4..i * 4 + 4].copy_from_slice(&e[2..]);
    }
    assert_eq!(
        actual,
        expected.as_slice(),
        "full-frame RGBA8 differs; native source stream offset={offset}"
    );
}

#[test]
fn watercolor_repeat32_source_plane_svg_matches_native_dense_full_frame() {
    let (base_rows, base_fixture, _base_offset, argb) = inputs();
    let rows = dense_repeat32_rows(&base_rows);
    let fixture: Value = serde_json::from_str(DENSE).expect("dense WaterColor fixture JSON");
    let source_color = input_color(argb);
    let geometry: Value = serde_json::from_str(GEOMETRY).unwrap();
    assert_eq!(
        sha256(GEOMETRY.as_bytes()),
        base_fixture["source"]["geometry_fixture_sha256"]
            .as_str()
            .unwrap()
    );
    assert_eq!(
        sha256(STREAM),
        geometry["stream"]["sha256"].as_str().unwrap()
    );
    assert_dense_source_color(&rows, &fixture, source_color, argb);
    assert_high_branch_mask_gate(&fixture);

    let svg = make_svg(&rows, source_color, true);
    assert_eq!(svg.matches("<polygon points=").count(), 960);
    assert_eq!(svg.matches("<image ").count(), 960);
    assert!(svg.contains("feComposite in=\"acc-478\" in2=\"src-479\""));
    let tree = resvg::usvg::Tree::from_str(&svg, &Default::default())
        .expect("generated dense WaterColor SVG");
    let mut pix = resvg::tiny_skia::Pixmap::new(SIZE as u32, SIZE as u32).unwrap();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pix.as_mut(),
    );
    assert_dense_final_frame(pix.data(), &fixture);
}

#[test]
fn controlled_drawing_capture_v22_geometry_and_full_frame() {
    const CAPTURE: &str =
        include_str!("../../../../../conformance/watercolor-drawing-capture-v22.json");
    let fixture: Value = serde_json::from_str(CAPTURE).unwrap();
    assert_eq!(
        fixture["schema"],
        "sdocx-watercolor-controlled-drawing-capture-v22-v1"
    );
    assert_eq!(
        fixture["provenance"]["mesa_vertex_shader_ir"]["source_blake3"],
        "1fb6ed1d6f0d83fc69a07fd1fb1d803d8fb7a2055c5e79d19a9e71c7cf3f7c95"
    );
    assert_eq!(
        fixture["provenance"]["mesa_vertex_shader_ir"]["debug_stderr_sha256"],
        "67a820e7a577e5a38d45bce8f92d63b322fe40c67872685793003a5bb81ba029"
    );
    assert_eq!(
        fixture["provenance"]["case_id"],
        "watercolor-unset-tool2-v1"
    );
    assert!(
        fixture["scope"]
            .as_str()
            .unwrap()
            .contains("Not a deserialized/saved-note stroke")
    );
    assert_eq!(fixture["input"]["advanced_setting_mode"], "default-null");
    assert_eq!(fixture["input"]["target_dimensions"][0], 256);
    assert_eq!(fixture["input"]["target_dimensions"][1], 256);
    assert_eq!(fixture["native_geometry"]["record_count"], 8);
    assert_eq!(fixture["native_geometry"]["record_bytes"], 192);
    assert_eq!(
        fixture["native_geometry"]["sha256"],
        "9876831bb99e269a27b0d4c3b1b093dbae1ee3858460aa3219d33929dd1ff371"
    );
    assert_eq!(
        fixture["resource_path"]["texture_factory_source_symbol"]["source_rva"],
        "0x5ac68"
    );
    assert_eq!(
        fixture["resource_path"]["static_abi_x0_x5"]
            .as_array()
            .unwrap()
            .len(),
        6
    );
    assert_eq!(
        fixture["resource_path"]["inherited_create_pen_canvas_override_removed"],
        true
    );
    assert_eq!(
        fixture["resource_path"]["runtime_raw_private_canvas_this_plus_200_read"],
        false
    );
    assert_eq!(fixture["draws"][0]["fbo"], 2);
    assert_eq!(fixture["draws"][0]["attachment_texture"], 2);
    assert_eq!(fixture["draws"][0]["sampled_texture"], 3);
    assert_eq!(fixture["draws"][1]["fbo"], 1);
    assert_eq!(fixture["draws"][1]["attachment_texture"], 1);
    assert_eq!(fixture["draws"][1]["sampled_texture"], 2);
    assert_eq!(
        fixture["draws"][0]["sampler_parameters"]["min_filter"],
        9728
    );
    assert_eq!(fixture["draws"][0]["sampler_parameters"]["wrap_s"], 33071);

    let decode_hex = |s: &str| -> Vec<u8> {
        assert_eq!(s.len() % 2, 0);
        s.as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|p| {
                let d = |v: u8| match v {
                    b'0'..=b'9' => v - b'0',
                    b'a'..=b'f' => v - b'a' + 10,
                    b'A'..=b'F' => v - b'A' + 10,
                    _ => panic!("invalid hex"),
                };
                d(p[0]) * 16 + d(p[1])
            })
            .collect()
    };
    let floats32 = |s: &str| -> Vec<f32> {
        decode_hex(s)
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| f32::from_le_bytes(*b))
            .collect()
    };
    let arrays = &fixture["input"]["source_arrays"];
    let point_bytes = decode_hex(arrays["points"]["data_hex"].as_str().unwrap());
    let points: Vec<Point> = point_bytes
        .as_chunks::<8>()
        .0
        .iter()
        .map(|b| Point {
            x: f32::from_le_bytes(b[..4].try_into().unwrap()) as f64,
            y: f32::from_le_bytes(b[4..].try_into().unwrap()) as f64,
        })
        .collect();
    let pressures = floats32(arrays["pressures"]["data_hex"].as_str().unwrap())
        .into_iter()
        .map(f64::from)
        .collect::<Vec<_>>();
    let timestamps = decode_hex(arrays["timestamps"]["data_hex"].as_str().unwrap())
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| i32::from_le_bytes(*b) as i64)
        .collect::<Vec<_>>();
    let screen = fixture["input"]["screen_profile_dimensions"]
        .as_array()
        .unwrap();
    let argb = u32::from_str_radix(
        fixture["input"]["color_argb_u32"]
            .as_str()
            .unwrap()
            .trim_start_matches("0x"),
        16,
    )
    .unwrap();
    let size_bytes = decode_hex(fixture["input"]["size_f32_bits"].as_str().unwrap());
    let input = SavedStrokeInput {
        points: &points,
        pressures: &pressures,
        timestamps: &timestamps,
        pen_size: f32::from_bits(u32::from_le_bytes(size_bytes.try_into().unwrap())),
        color_argb: argb,
        color_type: fixture["input"]["color_type"].as_u64().unwrap() as u8,
        stroke_type: fixture["input"]["stroke_type"].as_u64().unwrap() as u8,
        curve_enabled: fixture["input"]["curve_enabled"].as_bool().unwrap(),
        tool_type: fixture["input"]["tool_type"].as_u64().unwrap() as u8,
        canvas: [
            screen[0].as_u64().unwrap() as u32,
            screen[1].as_u64().unwrap() as u32,
        ],
    };
    let rows = sample_saved_stroke_with_math(
        input,
        DrawableVersion::V1,
        MathProfile::CapturePythonDoubleSinCos,
    )
    .unwrap();
    let actual_stream = serialize_stream(&rows);
    assert_eq!(rows.len(), 8);
    assert_eq!(actual_stream.len(), 192);
    let native_stream = decode_hex(fixture["native_geometry"]["records_hex"].as_str().unwrap());
    if actual_stream != native_stream {
        let first_lane = actual_stream
            .as_chunks::<4>()
            .0
            .iter()
            .zip(native_stream.as_chunks::<4>().0.iter())
            .position(|(actual, expected)| actual != expected)
            .unwrap();
        let actual_bits = u32::from_le_bytes(
            actual_stream[first_lane * 4..first_lane * 4 + 4]
                .try_into()
                .unwrap(),
        );
        let native_bits = u32::from_le_bytes(
            native_stream[first_lane * 4..first_lane * 4 + 4]
                .try_into()
                .unwrap(),
        );
        panic!(
            "native geometry first differs at row {}, lane {}: sampler={actual_bits:#010x}, native={native_bits:#010x}",
            first_lane / 6,
            first_lane % 6
        );
    }
    assert_eq!(
        sha256(&actual_stream),
        fixture["native_geometry"]["sha256"].as_str().unwrap(),
        "geometry sampler must reproduce native Drawing rows before paint comparison"
    );

    let color = input_color(argb);
    let projection = fixture["draws"][0]["uniforms"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["call"] == "glUniformMatrix4fv" && row["location"] == 0)
        .expect("captured native draw projection uniform");
    let matrix = projection["values"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_f64().unwrap() as f32)
        .collect::<Vec<_>>();
    assert_eq!(matrix.len(), 16);
    assert_eq!(matrix[0].to_bits(), (1.0f32 / 128.0).to_bits());
    assert_eq!(matrix[5].to_bits(), (1.0f32 / 128.0).to_bits());
    assert_eq!(matrix[12].to_bits(), (-1.0f32).to_bits());
    assert_eq!(matrix[13].to_bits(), (-1.0f32).to_bits());
    let expected_matrix = [
        1.0f32 / 128.0,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0f32 / 128.0,
        0.0,
        0.0,
        0.0,
        0.0,
        f32::from_bits(0xb800_0100),
        0.0,
        -1.0,
        -1.0,
        0.0,
        1.0,
    ];
    assert_eq!(
        matrix
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>(),
        expected_matrix
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>()
    );
    let matrix: [f32; 16] = matrix.try_into().unwrap();
    let svg = make_svg_with_projection_matrix(&rows, color, false, matrix);
    let target = &fixture["input"]["target_dimensions"];
    assert_eq!(target[0].as_u64(), Some(256));
    assert_eq!(target[1].as_u64(), Some(256));
    let tree = resvg::usvg::Tree::from_str(&svg, &Default::default()).unwrap();
    let mut pix = resvg::tiny_skia::Pixmap::new(256, 256).unwrap();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pix.as_mut(),
    );

    // Sparse records are the captured native pixels; omitted pixels are zero.
    // Compare the full viewport after converting GL's lower-left rows to the
    // top-left Pixmap layout, without resizing the 256x256 target.
    let native = &fixture["draws"][1]["post"];
    assert_eq!(native["row_origin"], "GL lower-left");
    assert_eq!(
        fixture["draws"][1]["pre"]["nonzero_pixels"].as_u64(),
        Some(0)
    );
    assert_eq!(
        fixture["draws"][1]["pre"]["sha256"],
        "8a39d2abd3999ab73c34db2476849cddf303ce389b35826850f9a700589b4a90"
    );
    assert_eq!(
        sha256(PATTERN_PNG),
        fixture["pattern_upload"]["asset_png_sha256"]
            .as_str()
            .unwrap()
    );
    let pattern = resvg::tiny_skia::Pixmap::decode_png(PATTERN_PNG).unwrap();
    assert_eq!(
        sha256(pattern.data()),
        fixture["pattern_upload"]["native_uploaded_rgba_sha256"]
            .as_str()
            .unwrap()
    );
    let mut raw_bottom_up = vec![0u8; 256 * 256 * 4];
    let sparse = decode_hex(
        native["sparse_entries_u16_index_rgba_hex"]
            .as_str()
            .unwrap(),
    );
    assert_eq!(
        sparse.len(),
        native["sparse_entries"].as_u64().unwrap() as usize * 6
    );
    assert_eq!(native["nonzero_pixels"], native["sparse_entries"]);
    let mut last = None;
    for entry in sparse.as_chunks::<6>().0.iter() {
        let index = u16::from_le_bytes(entry[..2].try_into().unwrap()) as usize;
        assert!(index < 256 * 256);
        assert!(
            last.is_none_or(|prior| prior < index),
            "sparse indices are strictly ordered"
        );
        last = Some(index);
        raw_bottom_up[index * 4..index * 4 + 4].copy_from_slice(&entry[2..]);
    }
    assert_eq!(sha256(&raw_bottom_up), native["sha256"].as_str().unwrap());
    let row_bytes = 256 * 4;
    let mut native_top_left = vec![0u8; raw_bottom_up.len()];
    for y in 0..256 {
        let source_y = 255 - y;
        native_top_left[y * row_bytes..(y + 1) * row_bytes]
            .copy_from_slice(&raw_bottom_up[source_y * row_bytes..(source_y + 1) * row_bytes]);
    }
    let candidate_nonzero_pixels = pix
        .data()
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| pixel.iter().any(|&channel| channel != 0))
        .count();
    assert_eq!(
        candidate_nonzero_pixels,
        native["nonzero_pixels"].as_u64().unwrap() as usize,
        "candidate/native support count mismatch; candidate hash={} expected native raw hash={}",
        sha256(pix.data()),
        native["sha256"].as_str().unwrap()
    );
    let differing_bytes = pix
        .data()
        .iter()
        .zip(&native_top_left)
        .filter(|(candidate, expected)| candidate != expected)
        .count();
    let first_difference = pix
        .data()
        .iter()
        .zip(&native_top_left)
        .position(|(candidate, expected)| candidate != expected);
    let first_byte_difference = first_difference.map(|offset| {
        (
            offset / 4 % 256,
            offset / (256 * 4),
            offset % 4,
            pix.data()[offset],
            native_top_left[offset],
        )
    });
    assert_eq!(
        differing_bytes,
        0,
        "full-frame candidate/native mismatch: {differing_bytes} bytes, first difference (x,y,ch,candidate,native)={first_byte_difference:?}, candidate hash={} expected native raw hash={}",
        sha256(pix.data()),
        native["sha256"].as_str().unwrap()
    );
}
