//! Test-only CPU reconstruction of one captured legacy Marker GLES draw stream.
//!
//! The five VBOs, two cap textures, matrix, uniform values, and GL state are
//! pinned source inputs. The five native sparse frames are assertions only.
//! This module is not connected to production
//! rendering or pen dispatch.

use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

const WIDTH: usize = 1920;
const HEIGHT: usize = 1080;
const PIXELS: usize = WIDTH * HEIGHT;
const CAP_W: usize = 54;
const CAP_H: usize = 72;
const COLOR: [f32; 4] = [
    1.0,
    f32::from_bits(0x3f4b_cbcc), // captured 0.7960784435272217
    f32::from_bits(0x3e40_c0c1), // captured 0.1882352977991104
    f32::from_bits(0x3f00_8081), // captured 0.501960813999176
];
const PROJECTION: [f32; 16] = [
    f32::from_bits(0x3a88_8889),
    0.0,
    0.0,
    0.0,
    0.0,
    f32::from_bits(0x3af2_b9d6),
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

const VBO: &[u8] =
    include_bytes!("../../../../conformance/marker-v31-native-paint/native-vbo-f32.bin");
const CAPS: &[u8] =
    include_bytes!("../../../../conformance/marker-v31-native-paint/cap-textures-rgba8.bin");
const NATIVE_PREFIXES: [&[u8]; 5] = [
    include_bytes!(
        "../../../../conformance/marker-v31-native-paint/native-prefix-0.sparse-rgba8.bin"
    ),
    include_bytes!(
        "../../../../conformance/marker-v31-native-paint/native-prefix-1.sparse-rgba8.bin"
    ),
    include_bytes!(
        "../../../../conformance/marker-v31-native-paint/native-prefix-2.sparse-rgba8.bin"
    ),
    include_bytes!(
        "../../../../conformance/marker-v31-native-paint/native-prefix-3.sparse-rgba8.bin"
    ),
    include_bytes!(
        "../../../../conformance/marker-v31-native-paint/native-prefix-4.sparse-rgba8.bin"
    ),
];

const VBO_SHA256: &str = "84147403647db25d23be2837d1ef34bcd8248dac1ddba724edd3f35f1acc926d";
const DRAW_VBO_SHA256: [&str; 5] = [
    "fff5e92120ae2e203a8b73f01c2a1e9a2baa7b249ede2b85b43046d117e190b7",
    "463aa9697953c6fdec16962e85cb362458dfa112bab4a20bacee72cc891e278c",
    "6f8bdce23a861aa5b20ae4689828eacd12a6cd7de519a3d7c390a83c36d7847c",
    "2a1d13837e0a6b28fa44040e1b66573f72dfe85bd2951ee1025ff085ea77e0ce",
    "d471f0ca3d85e79a322d17b72abfc0869d32008773f6611d72531b77f963b933",
];
const CAPS_SHA256: &str = "6355aeca3989e08ed0943f5d91931636800b49afe06b3cdc48d4cc3712f0923f";
const PAINT_PROFILE: &[u8] =
    include_bytes!("../../../../conformance/marker-v31-native-paint/paint-profile.json");
const PAINT_PROFILE_SHA256: &str =
    "58d2361084227f10bc2d653d0791d8fa756fc1e13da1a1d3185d1c9e39d9a926";

const NATIVE_PREFIX_SHA256: [&str; 5] = [
    "1852cc029413372d862a25c2e76d93221f12c49dbcbb47cf300d13eb7c955cf9",
    "1bbc1753a52e8bcfe4c402b6b5e22f20722203429e78c23b85fa4736677fc8df",
    "6b99ce41826cf326ff570f36210388ba0cd3de4534832ebc4ee6f95747970571",
    "31ecf75c7ebcbb6f19b46bb603a4e7aa5b27f90e2b7738bddec04f2486585555",
    "d5e0c222b5b923bf5b35d19426691b9bc169d071aeb259c06ce0b0a0ff567442",
];
const DRAWS: [Draw; 5] = [
    Draw {
        program: 4,
        mode: 5,
        count: 48,
        lanes: 2,
        offset: 0,
        texture: None,
        revert: 0.0,
        z: 32763.0,
    },
    Draw {
        program: 6,
        mode: 4,
        count: 6,
        lanes: 4,
        offset: 384,
        texture: Some(0),
        revert: 1.0,
        z: 32764.0,
    },
    Draw {
        program: 5,
        mode: 5,
        count: 56,
        lanes: 3,
        offset: 480,
        texture: None,
        revert: 0.0,
        z: 32764.0,
    },
    Draw {
        program: 5,
        mode: 5,
        count: 56,
        lanes: 3,
        offset: 1152,
        texture: None,
        revert: 0.0,
        z: 32764.0,
    },
    Draw {
        program: 6,
        mode: 4,
        count: 6,
        lanes: 4,
        offset: 1824,
        texture: Some(1),
        revert: 0.0,
        z: 32763.0,
    },
];

#[derive(Clone, Copy)]
struct Draw {
    program: u8,
    mode: u8,
    count: usize,
    lanes: usize,
    offset: usize,
    texture: Option<usize>,
    revert: f32,
    z: f32,
}

#[derive(Clone, Copy)]
struct Vertex {
    x: f32,
    y: f32,
    attrs: [f32; 4],
}

#[derive(Clone, Copy, Debug)]
struct PrefixStats {
    draw: usize,
    model_support: usize,
    native_support: usize,
    support_symmetric_difference: usize,
    diff_pixels: usize,
    diff_channel_bytes: usize,
    abs_sums_rgba: [u64; 4],
    max_channel_error: u8,
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn read_f32(bytes: &[u8], offset: usize) -> f32 {
    f32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("four-byte lane"),
    )
}

fn source_vertex(draw: Draw, row: usize, vbo: &[u8]) -> Vertex {
    let stride = draw.lanes * 4;
    let offset = draw.offset + row * stride;
    let mut v = [0.0; 4];
    for (lane, out) in v.iter_mut().enumerate().take(draw.lanes) {
        *out = read_f32(vbo, offset + lane * 4);
    }
    let x = v[0];
    let y = v[1];
    // Varying layout is [x,y], [x,y,u,v], or [x,y,v] for the captured
    // solid, cap, and body programs respectively.
    let attrs = match draw.program {
        4 => [x, y, 0.0, 0.0],
        6 => [x, y, v[2], v[3]],
        5 => [x, y, v[2], 0.0],
        _ => unreachable!("pinned program id"),
    };
    let cx = PROJECTION[0] * x - 1.0;
    let cy = PROJECTION[5] * y - 1.0;
    let sx = (cx + 1.0) * (WIDTH as f32 / 2.0);
    let sy = (cy + 1.0) * (HEIGHT as f32 / 2.0);
    Vertex {
        x: sx,
        y: sy,
        attrs,
    }
}

fn orient(a: Vertex, b: Vertex, x: f64, y: f64) -> f64 {
    ((f64::from(b.x) - f64::from(a.x)) * (y - f64::from(a.y)))
        - ((f64::from(b.y) - f64::from(a.y)) * (x - f64::from(a.x)))
}

fn top_left(a: Vertex, b: Vertex) -> bool {
    let dx = f64::from(b.x) - f64::from(a.x);
    let dy = f64::from(b.y) - f64::from(a.y);
    dy > 0.0 || (dy == 0.0 && dx < 0.0)
}

fn triangle_vertices(draw: Draw, triangle: usize, vbo: &[u8]) -> [Vertex; 3] {
    let i = triangle;
    let indexes = match draw.mode {
        4 => [3 * i, 3 * i + 1, 3 * i + 2],            // GL_TRIANGLES
        5 if i.is_multiple_of(2) => [i, i + 1, i + 2], // GL_TRIANGLE_STRIP
        5 => [i + 1, i, i + 2],
        _ => unreachable!("pinned primitive mode"),
    };
    indexes.map(|row| source_vertex(draw, row, vbo))
}

fn sample(tri: [Vertex; 3], px: f64, py: f64) -> Option<[f32; 4]> {
    // One fragment sample at pixel center, with top-left half-open edge ownership.
    let mut t = tri;
    let mut area = orient(t[0], t[1], f64::from(t[2].x), f64::from(t[2].y));
    if area < 0.0 {
        t.swap(1, 2);
        area = -area;
    }
    if area <= 1.0e-12 {
        return None;
    }
    let es = [
        orient(t[0], t[1], px, py),
        orient(t[1], t[2], px, py),
        orient(t[2], t[0], px, py),
    ];
    let edges = [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])];
    for (e, (a, b)) in es.into_iter().zip(edges) {
        if e < 0.0 || (e == 0.0 && !top_left(a, b)) {
            return None;
        }
    }
    // Keep edge barycentrics in f64 like the independent Python model, then
    // round each varying operation explicitly to F32.
    let w = [es[1] / area, es[2] / area, es[0] / area];
    let mut out = [0.0_f32; 4];
    for (lane, output) in out.iter_mut().enumerate() {
        let a = (w[0] * f64::from(t[0].attrs[lane])) as f32;
        let b = (w[1] * f64::from(t[1].attrs[lane])) as f32;
        let c = (w[2] * f64::from(t[2].attrs[lane])) as f32;
        *output = (a + b) + c;
    }
    Some(out)
}

fn q_depth(z: f32) -> f64 {
    // Captured GL_DEPTH24_STENCIL8 + GL_LESS; model 24-bit normalized storage
    // with nearest quantization. Draws 0-3 write depth; draw 4 has depth off.
    let n = f64::from(0x00ff_ffff_u32);
    (f64::from(z.clamp(0.0, 1.0)) * n + 0.5).floor() / n
}

fn cap_alpha(texture: usize, revert: f32, u: f32, v: f32) -> u8 {
    let s = (revert - u).abs().clamp(0.0, 1.0);
    let t = (revert - v).abs().clamp(0.0, 1.0);
    let x = ((s * CAP_W as f32).floor() as usize).min(CAP_W - 1);
    let y = ((t * CAP_H as f32).floor() as usize).min(CAP_H - 1);
    let offset = texture * CAP_W * CAP_H * 4 + (y * CAP_W + x) * 4 + 3;
    CAPS[offset]
}

fn q8(value: f64) -> u8 {
    // Explicit UNORM8 nearest-up conversion used by the source-derived model.
    ((value + 0.5).floor() as i64).clamp(0, 255) as u8
}

fn blend_over(src_alpha: f32, dst: [u8; 4]) -> [u8; 4] {
    // Captured GL_FUNC_ADD, GL_ONE, GL_ONE_MINUS_SRC_ALPHA; fragment RGB is
    // premultiplied by the shader alpha before this source-over operation.
    let inv = 1.0 - src_alpha;
    let mut out = [0; 4];
    for (c, output) in out[..3].iter_mut().enumerate() {
        let src = COLOR[c] * src_alpha;
        let dest = ((f64::from(dst[c]) / 255.0) * f64::from(inv)) as f32;
        *output = q8(f64::from(src + dest) * 255.0);
    }
    let alpha = src_alpha + ((f64::from(dst[3]) / 255.0) * f64::from(inv)) as f32;
    out[3] = q8(f64::from(alpha) * 255.0);
    out
}

fn decode_expected(bytes: &[u8], expected_hash: &str) -> BTreeMap<usize, [u8; 4]> {
    assert_eq!(
        sha256(bytes),
        expected_hash,
        "native sparse fixture changed"
    );
    assert_eq!(bytes.len() % 8, 0);
    let mut result = BTreeMap::new();
    for record in bytes.as_chunks::<8>().0 {
        let idx = u32::from_le_bytes(record[..4].try_into().unwrap()) as usize;
        assert!(idx < PIXELS);
        let rgba = [record[4], record[5], record[6], record[7]];
        assert!(rgba.iter().any(|channel| *channel != 0));
        assert!(
            result.insert(idx, rgba).is_none(),
            "duplicate native pixel index"
        );
    }
    result
}

fn validate_prefix(draw: usize, model: &BTreeMap<usize, [u8; 4]>) -> PrefixStats {
    let expected = decode_expected(NATIVE_PREFIXES[draw], NATIVE_PREFIX_SHA256[draw]);
    let mut keys = std::collections::BTreeSet::new();
    keys.extend(model.keys().copied());
    keys.extend(expected.keys().copied());
    let mut stats = PrefixStats {
        draw,
        model_support: model.values().filter(|v| v.iter().any(|x| *x != 0)).count(),
        native_support: expected.len(),
        support_symmetric_difference: 0,
        diff_pixels: 0,
        diff_channel_bytes: 0,
        abs_sums_rgba: [0; 4],
        max_channel_error: 0,
    };
    for index in keys {
        let actual = model.get(&index).copied().unwrap_or([0; 4]);
        let native = expected.get(&index).copied().unwrap_or([0; 4]);
        let actual_covered = actual.iter().any(|v| *v != 0);
        let native_covered = expected.contains_key(&index);
        stats.support_symmetric_difference += (actual_covered != native_covered) as usize;
        let mut pixel_diff = false;
        for (c, (actual_channel, native_channel)) in actual.into_iter().zip(native).enumerate() {
            let delta = actual_channel.abs_diff(native_channel);
            stats.abs_sums_rgba[c] += u64::from(delta);
            stats.max_channel_error = stats.max_channel_error.max(delta);
            stats.diff_channel_bytes += (delta != 0) as usize;
            pixel_diff |= delta != 0;
        }
        stats.diff_pixels += pixel_diff as usize;
    }
    stats
}

fn render_and_compare() -> Vec<PrefixStats> {
    assert_eq!(sha256(VBO), VBO_SHA256, "source VBO fixture changed");
    assert_eq!(
        sha256(CAPS),
        CAPS_SHA256,
        "source cap texture fixture changed"
    );
    assert_eq!(CAPS.len(), 2 * CAP_W * CAP_H * 4);
    for (draw, expected) in DRAWS.into_iter().zip(DRAW_VBO_SHA256) {
        let bytes = &VBO[draw.offset..draw.offset + draw.count * draw.lanes * 4];
        assert_eq!(sha256(bytes), expected, "source draw VBO changed");
    }
    let mut color = vec![[0_u8; 4]; PIXELS];
    let mut depth = vec![1.0_f64; PIXELS]; // glClearDepthf(1.0), GL_DEPTH24_STENCIL8
    let mut results = Vec::with_capacity(DRAWS.len());

    for (draw_index, draw) in DRAWS.into_iter().enumerate() {
        let z_clip = PROJECTION[10] * -draw.z;
        let depth_value = q_depth((z_clip + 1.0) * 0.5);
        let depth_enabled = draw_index < 4; // captured GL_DEPTH_TEST toggles
        let triangle_count = match draw.mode {
            4 => draw.count / 3,
            5 => draw.count - 2,
            _ => unreachable!(),
        };
        for triangle_index in 0..triangle_count {
            let tri = triangle_vertices(draw, triangle_index, VBO);
            let signed_area = orient(tri[0], tri[1], f64::from(tri[2].x), f64::from(tri[2].y));
            if signed_area.abs() <= 1.0e-12 {
                continue;
            }
            let min_x = tri
                .iter()
                .map(|v| v.x)
                .fold(f32::INFINITY, f32::min)
                .floor()
                .max(0.0) as usize;
            let max_x = tri
                .iter()
                .map(|v| v.x)
                .fold(f32::NEG_INFINITY, f32::max)
                .ceil()
                .min(WIDTH as f32) as usize;
            let min_y = tri
                .iter()
                .map(|v| v.y)
                .fold(f32::INFINITY, f32::min)
                .floor()
                .max(0.0) as usize;
            let max_y = tri
                .iter()
                .map(|v| v.y)
                .fold(f32::NEG_INFINITY, f32::max)
                .ceil()
                .min(HEIGHT as f32) as usize;
            for y in min_y..max_y {
                for x in min_x..max_x {
                    let Some(varying) = sample(tri, x as f64 + 0.5, y as f64 + 0.5) else {
                        continue;
                    };
                    let index = y * WIDTH + x;
                    if depth_enabled && depth_value >= depth[index] {
                        continue;
                    }
                    if depth_enabled {
                        depth[index] = depth_value;
                    }
                    let alpha = match draw.program {
                        4 => COLOR[3],
                        5 => COLOR[3] - COLOR[3] * varying[2],
                        6 => {
                            if cap_alpha(draw.texture.unwrap(), draw.revert, varying[2], varying[3])
                                == 0
                            {
                                0.0
                            } else {
                                (COLOR[3] + varying[2].abs() * (CAP_W as f32 / 255.0))
                                    .clamp(0.0, 1.0)
                            }
                        }
                        _ => unreachable!(),
                    };
                    color[index] = blend_over(alpha, color[index]);
                }
            }
        }
        let sparse = color
            .iter()
            .enumerate()
            .filter(|(_, rgba)| rgba.iter().any(|x| *x != 0))
            .map(|(i, rgba)| (i, *rgba))
            .collect::<BTreeMap<_, _>>();
        results.push(validate_prefix(draw_index, &sparse));
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captured_marker_prefixes_match_source_derived_support_and_bounded_unorm_error() {
        assert_eq!(
            sha256(PAINT_PROFILE),
            PAINT_PROFILE_SHA256,
            "paint profile provenance changed"
        );
        let stats = render_and_compare();
        assert_eq!(stats.len(), 5);
        let expected_support = [247, 304, 337, 367, 423];
        let expected_diff_pixels = [0, 4, 5, 6, 8];
        let expected_diff_channel_bytes = [0, 7, 9, 10, 12];
        let expected_abs_sums_rgba = [
            [0, 0, 0, 0],
            [3, 1, 0, 3],
            [4, 1, 0, 4],
            [4, 2, 0, 4],
            [4, 3, 1, 4],
        ];
        for (draw, row) in stats.iter().enumerate() {
            assert_eq!(row.draw, draw);
            assert_eq!(row.native_support, expected_support[draw]);
            assert_eq!(row.model_support, expected_support[draw]);
            assert_eq!(row.support_symmetric_difference, 0);
            assert_eq!(row.diff_pixels, expected_diff_pixels[draw]);
            assert_eq!(row.diff_channel_bytes, expected_diff_channel_bytes[draw]);
            assert_eq!(row.abs_sums_rgba, expected_abs_sums_rgba[draw]);
            // GLES/Mesa and this CPU model may round a component at a UNORM8
            // boundary by one. No output color is copied or corrected.
            assert!(row.max_channel_error <= 1, "prefix {draw}: {row:?}");
        }
    }
}

#[test]
fn generated_mesh_paint_matches_native_source_controls() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/legacy-marker-source-geometry-controls.json"
    ))
    .unwrap();
    for (case, revert, native_bytes, expected_hash) in [
        (
            "straight",
            0.0,
            include_bytes!(
                "../../../../conformance/legacy-marker-source-straight-sparse-rgba8.bin"
            )
            .as_slice(),
            "7266a5a2d494b566f7e5643c013bfd1f30a5c417c348d03115f5863586d920fd",
        ),
        (
            "bend",
            1.0,
            include_bytes!("../../../../conformance/legacy-marker-source-bend-sparse-rgba8.bin")
                .as_slice(),
            "62f15c9c6302de5dcae527b1b463e5cc65c695a37c4dcd9cc521f6006407cc36",
        ),
    ] {
        let native = &fixture["cases"][case];
        let hex = native["input"]["source_arrays_hex"]["points"]
            .as_str()
            .unwrap();
        let bytes: Vec<u8> = hex
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
            .collect();
        let points: Vec<f32> = bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| f32::from_le_bytes(*b))
            .collect();
        let points: [[f32; 2]; 4] = points.as_chunks::<2>().0.try_into().unwrap();
        let size = f32::from_bits(
            u32::from_str_radix(native["input"]["size_f32_bits"].as_str().unwrap(), 16).unwrap(),
        );
        let mesh = super::legacy_marker_mesh::generate(points, size).unwrap();
        let mut vbo = Vec::new();
        let mut draws = DRAWS;
        let sources: [Vec<f32>; 5] = [
            mesh.body.iter().flatten().copied().collect(),
            mesh.caps[0].iter().flatten().copied().collect(),
            mesh.fringe[0].iter().flatten().copied().collect(),
            mesh.fringe[1].iter().flatten().copied().collect(),
            mesh.caps[1].iter().flatten().copied().collect(),
        ];
        draws[4].revert = revert;
        for (draw, source) in draws.iter_mut().zip(sources) {
            draw.offset = vbo.len();
            draw.count = source.len() / draw.lanes;
            for value in source {
                vbo.extend(value.to_le_bytes());
            }
        }
        let mut color = vec![[0_u8; 4]; PIXELS];
        let mut depth = vec![1.0_f64; PIXELS]; // glClearDepthf(1.0), GL_DEPTH24_STENCIL8

        for (draw_index, draw) in draws.into_iter().enumerate() {
            let z_clip = PROJECTION[10] * -draw.z;
            let depth_value = q_depth((z_clip + 1.0) * 0.5);
            let depth_enabled = draw_index < 4; // captured GL_DEPTH_TEST toggles
            let triangle_count = match draw.mode {
                4 => draw.count / 3,
                5 => draw.count - 2,
                _ => unreachable!(),
            };
            for triangle_index in 0..triangle_count {
                let tri = triangle_vertices(draw, triangle_index, &vbo);
                let signed_area = orient(tri[0], tri[1], f64::from(tri[2].x), f64::from(tri[2].y));
                if signed_area.abs() <= 1.0e-12 {
                    continue;
                }
                let min_x = tri
                    .iter()
                    .map(|v| v.x)
                    .fold(f32::INFINITY, f32::min)
                    .floor()
                    .max(0.0) as usize;
                let max_x = tri
                    .iter()
                    .map(|v| v.x)
                    .fold(f32::NEG_INFINITY, f32::max)
                    .ceil()
                    .min(WIDTH as f32) as usize;
                let min_y = tri
                    .iter()
                    .map(|v| v.y)
                    .fold(f32::INFINITY, f32::min)
                    .floor()
                    .max(0.0) as usize;
                let max_y = tri
                    .iter()
                    .map(|v| v.y)
                    .fold(f32::NEG_INFINITY, f32::max)
                    .ceil()
                    .min(HEIGHT as f32) as usize;
                for y in min_y..max_y {
                    for x in min_x..max_x {
                        let Some(varying) = sample(tri, x as f64 + 0.5, y as f64 + 0.5) else {
                            continue;
                        };
                        let index = y * WIDTH + x;
                        if depth_enabled && depth_value >= depth[index] {
                            continue;
                        }
                        if depth_enabled {
                            depth[index] = depth_value;
                        }
                        let alpha = match draw.program {
                            4 => COLOR[3],
                            5 => COLOR[3] - COLOR[3] * varying[2],
                            6 => {
                                if cap_alpha(
                                    draw.texture.unwrap(),
                                    draw.revert,
                                    varying[2],
                                    varying[3],
                                ) == 0
                                {
                                    0.0
                                } else {
                                    (COLOR[3] + varying[2].abs() * (CAP_W as f32 / 255.0))
                                        .clamp(0.0, 1.0)
                                }
                            }
                            _ => unreachable!(),
                        };
                        color[index] = blend_over(alpha, color[index]);
                    }
                }
            }
        }
        let expected = decode_expected(native_bytes, expected_hash);
        let mut full_frame = vec![0_u8; PIXELS * 4];
        for (&index, rgba) in &expected {
            full_frame[index * 4..index * 4 + 4].copy_from_slice(rgba);
        }
        assert_eq!(
            sha256(&full_frame),
            native["final_frame_sha256"].as_str().unwrap()
        );
        let mut diff = 0;
        let mut max = 0;
        for (i, pixel) in color.iter().enumerate() {
            let native = expected.get(&i).copied().unwrap_or([0; 4]);
            if *pixel != native {
                diff += 1;
            }
            for c in 0..4 {
                max = max.max(pixel[c].abs_diff(native[c]));
            }
        }
        eprintln!(
            "{case}: native support={}, different pixels={diff}, max channel error={max}",
            expected.len()
        );
        assert!(diff <= 12 && max <= 1, "{case}: {diff} pixels, max {max}");
    }
}
