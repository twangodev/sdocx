//! Saved WaterColor stroke -> native six-lane dabs -> generated SVG -> resvg frame.
//! RNE is a named, V13-captured host position profile; this is not universal GLSL behavior.
use super::{DrawableVersion, SavedStrokeInput, sample_saved_stroke};
use crate::Point;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::Value;
use sha2::{Digest, Sha256};

const PAINT: &str = include_str!("../../../../../conformance/watercolor-paint-native.json");
const GEOMETRY: &str = include_str!("../../../../../conformance/watercolor-native.json");
const STREAM: &[u8] = include_bytes!("../../../../../conformance/watercolor-native.bin");
const PATTERN_PNG: &[u8] = include_bytes!("../../../../../conformance/watercolor-pattern.png");
const SIZE: usize = 256;
const MAX_G: u8 = 13;

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
fn triangle_image_transform(v: [Vertex; 4], t: [usize; 3]) -> [f64; 6] {
    let [p, q, r] = t.map(|i| v[i]);
    let du1 = q.u - p.u;
    let dv1 = q.v - p.v;
    let du2 = r.u - p.u;
    let dv2 = r.v - p.v;
    let det = du1 * dv2 - du2 * dv1;
    assert_ne!(det, 0.0);
    let dx1 = q.x - p.x;
    let dy1 = q.y - p.y;
    let dx2 = r.x - p.x;
    let dy2 = r.y - p.y;
    let a = (dx1 * dv2 - dx2 * dv1) / det;
    let c = (-dx1 * du2 + dx2 * du1) / det;
    let b = (dy1 * dv2 - dy2 * dv1) / det;
    let d = (-dy1 * du2 + dy2 * du1) / det;
    [a, b, c, d, p.x - a * p.u - c * p.v, p.y - b * p.u - d * p.v]
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
    ((x.clamp(0.0, 1.0) * 255.0) + 0.5)
        .floor()
        .clamp(0.0, 255.0) as u8
}
fn mul_norm8(a: u8, b: u8) -> u8 {
    let mut x = u32::from(a) * u32::from(b);
    x += x >> 8;
    x += 128;
    (x >> 8).min(255) as u8
}
fn shader_alpha(g: u8, color: [f32; 4]) -> u8 {
    // Within the observed G=0..13 domain, this expression and the captured
    // half-sample/half-ALU RNE model produce the same A8. This is a bounded LUT,
    // not a general fragment-precision or high-density shader evaluator.
    let sample = (g as f32) / 255.0;
    if sample >= 0.7 {
        panic!("unmodeled high-density branch")
    }
    unorm(((sample / 0.7) * 0.8) * color[3])
}
fn resvg_mul_u8(u: u8, a: u8) -> u8 {
    let x = f32::from(u) / 255.0;
    let y = f32::from(a) / 255.0;
    (((((1.0f32 * x) * y) * 255.0) as u32).min(255)) as u8
}
fn encoded_unpremultiplied_rgb(g: u8, c: usize, color: [f32; 4]) -> u8 {
    let a = shader_alpha(g, color);
    let want = mul_norm8(unorm(color[c]), a);
    (0..=255u16)
        .map(|u| u as u8)
        .filter(|&u| resvg_mul_u8(u, a) == want)
        .min_by_key(|&u| (u.abs_diff(unorm(color[c])), u))
        .expect("source-derived resvg U8 preimage")
}
fn table(channel: usize, color: [f32; 4]) -> String {
    (0..=255u16)
        .map(|g| {
            let value = if g as u8 > MAX_G {
                0
            } else if channel == 3 {
                shader_alpha(g as u8, color)
            } else {
                encoded_unpremultiplied_rgb(g as u8, channel, color)
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
fn filter_program(color: [f32; 4]) -> String {
    let mut s = String::from(
        "<filter id=\"native-final-low-density\" x=\"0\" y=\"0\" width=\"256\" height=\"256\" filterUnits=\"userSpaceOnUse\" primitiveUnits=\"userSpaceOnUse\" color-interpolation-filters=\"sRGB\">",
    );
    s.push_str("<feImage href=\"#stamp-00\" image-rendering=\"optimizeSpeed\" style=\"image-rendering:pixelated\" x=\"0\" y=\"0\" width=\"256\" height=\"256\" preserveAspectRatio=\"none\" result=\"acc-00\"/>");
    for i in 1..15 {
        s.push_str(&format!("<feImage href=\"#stamp-{i:02}\" image-rendering=\"optimizeSpeed\" style=\"image-rendering:pixelated\" x=\"0\" y=\"0\" width=\"256\" height=\"256\" preserveAspectRatio=\"none\" result=\"src-{i:02}\"/><feComposite in=\"acc-{:02}\" in2=\"src-{i:02}\" operator=\"arithmetic\" k1=\"0\" k2=\"1\" k3=\"1\" k4=\"0\" result=\"acc-{i:02}\"/>",i-1));
    }
    s.push_str("<feColorMatrix in=\"acc-14\" type=\"matrix\" values=\"0 1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 0 0 0 1\" result=\"mask-g-opaque\"/><feComponentTransfer in=\"mask-g-opaque\" result=\"source-color-u8\">");
    for (name, ch) in [("R", 0), ("G", 1), ("B", 2)] {
        s.push_str(&format!(
            "<feFunc{name} type=\"discrete\" tableValues=\"{}\"/>",
            table(ch, color)
        ));
    }
    s.push_str("</feComponentTransfer><feComponentTransfer in=\"mask-g-opaque\" result=\"shader-alpha-unorm\">");
    for name in ["R", "G", "B"] {
        s.push_str(&format!(
            "<feFunc{name} type=\"discrete\" tableValues=\"{}\"/>",
            table(3, color)
        ));
    }
    s.push_str("</feComponentTransfer><feColorMatrix in=\"shader-alpha-unorm\" type=\"matrix\" values=\"0 0 0 0 1 0 0 0 0 1 0 0 0 0 1 1 0 0 0 0\" result=\"shader-alpha-white\"/><feComposite in=\"source-color-u8\" in2=\"shader-alpha-white\" operator=\"arithmetic\" k1=\"1\" k2=\"0\" k3=\"0\" k4=\"0\" result=\"native-paint-premultiplied\"/><feColorMatrix in=\"acc-14\" type=\"matrix\" values=\"0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0 0 0 0\" result=\"mask-red-gate-source\"/><feComponentTransfer in=\"mask-red-gate-source\" result=\"mask-red-gate\"><feFuncA type=\"discrete\" tableValues=\"0 ");
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
fn make_svg(rows: &[[f32; 6]], color: [f32; 4]) -> String {
    assert_eq!(rows.len(), 15);
    let mut svg = String::from(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"256\" height=\"256\" viewBox=\"0 0 256 256\" color-interpolation=\"sRGB\"><defs>",
    );
    svg.push_str(&filter_program(color));
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
        let v = vertices(row);
        let ts = [[0usize, 1, 2], [2, 1, 3]];
        for (k, t) in ts.into_iter().enumerate() {
            let id = format!("native-triangle-{i:02}-{k}");
            svg.push_str(&format!("<clipPath id=\"{id}\" clipPathUnits=\"userSpaceOnUse\"><polygon points=\"{}\" shape-rendering=\"crispEdges\"/></clipPath>",points(v,t)));
        }
        svg.push_str(&format!("<g id=\"stamp-{i:02}\">"));
        for (k, t) in ts.into_iter().enumerate() {
            svg.push_str(&format!("<g clip-path=\"url(#native-triangle-{i:02}-{k})\"><image x=\"0\" y=\"0\" width=\"1\" height=\"1\" preserveAspectRatio=\"none\" image-rendering=\"optimizeSpeed\" style=\"image-rendering:pixelated\" href=\"{uri}\" transform=\"{}\"/></g>",matrix(triangle_image_transform(v,t))));
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
    let svg = make_svg(&rows, source_color);
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
