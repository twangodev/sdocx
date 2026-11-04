//! Conditional CPU paint model for the captured Samsung legacy Pencil GLV1.
//!
//! This test-only model is separate from production dispatch. Its profile
//! is limited to the pinned V17 Mesa draw in `paint-conditional-profile.json`.
//! Mesa linked texture-coordinate arithmetic into the vertex shader on this host.
//! The source-side profile matches tested translated frames and injected wider
//! controls; a forced-triangle profile misses nine pixels in a width-7 control.
//! Callers must retain the guard.

use flate2::read::ZlibDecoder;
use sha2::{Digest, Sha256};
use std::io::Read;

const WIDTH: usize = 1920;
const HEIGHT: usize = 1080;
const ATLAS_WIDTH: usize = 144;
const ATLAS_HEIGHT: usize = 144;
const ATLAS_BYTES: usize = ATLAS_WIDTH * ATLAS_HEIGHT * 4;
const INSTANCE_COUNT: usize = 47;
const INSTANCE_STRIDE: usize = 16;
const INSTANCE_BYTES: usize = INSTANCE_COUNT * INSTANCE_STRIDE;
const SPARSE_RECORD_BYTES: usize = 8;
const EXPECTED_SPARSE_COUNT: usize = 35;
const NATIVE_INSTANCE_SHA256: &str =
    "64604038dcbf4e457a7bbe078e1467b802cffefa7f3309d19870c95628f4715d";
const NATIVE_SPARSE_SHA256: &str =
    "6ad8dcc464a01961f96d75ae4f0214e3deeefda55d6caeb3dbd480da82f0b580";
const NATIVE_ATLAS_SHA256: &str =
    "5149e5daea6e172fb1798904209904676bcb49da168993657868c0ef4858d440";

const INSTANCE_VBO: &[u8] = include_bytes!(
    "../../../../../conformance/old-pencil-color/alpha.vbo"
);
const NATIVE_SPARSE_RGBA8: &[u8] = include_bytes!(
    "../../../../../conformance/old-pencil-native-paint-v1/native-sparse-readback-u16xy-rgba8.bin"
);
const ATLAS: &[u8] = include_bytes!("../../render/old_pencil_atlas.bin");

// Actual attribute-0 GL_TRIANGLE_STRIP payload, V17 buffer object 2, 64 bytes.
const QUAD: [Corner; 4] = [
    Corner {
        x: -1.0,
        y: -1.0,
        frag_x: 0.0,
        frag_y: 1.0,
    },
    Corner {
        x: -1.0,
        y: 1.0,
        frag_x: 0.0,
        frag_y: 0.0,
    },
    Corner {
        x: 1.0,
        y: -1.0,
        frag_x: 1.0,
        frag_y: 1.0,
    },
    Corner {
        x: 1.0,
        y: 1.0,
        frag_x: 1.0,
        frag_y: 0.0,
    },
];

// Captured V17 glUniformMatrix4fv bytes, column major.
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

#[derive(Clone, Copy, Debug)]
struct Corner {
    x: f32,
    y: f32,
    frag_x: f32,
    frag_y: f32,
}

#[derive(Clone, Copy, Debug)]
struct Point {
    x: f32,
    y: f32,
    half_width: f32,
    texture_id: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VertexArithmetic {
    Separate32,
    Fma32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VaryingPlane {
    Triangle0,
    Triangle1,
    SourceSideSelection,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnormRound {
    NearestEven,
    NearestUp,
}

#[derive(Clone, Copy, Debug)]
pub struct Profile {
    pub vertex: VertexArithmetic,
    pub plane: VaryingPlane,
    pub unorm: UnormRound,
}

const fn f32_add(a: f32, b: f32) -> f32 {
    a + b
}

const fn f32_sub(a: f32, b: f32) -> f32 {
    a - b
}

const fn f32_mul(a: f32, b: f32) -> f32 {
    a * b
}

const fn f32_div(a: f32, b: f32) -> f32 {
    a / b
}

fn f32_fma(a: f32, b: f32, c: f32) -> f32 {
    a.mul_add(b, c)
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn decode_atlas() -> Result<Vec<u8>, &'static str> {
    if ATLAS.len() != ATLAS_BYTES || hash(ATLAS) != NATIVE_ATLAS_SHA256 {
        return Err("captured Pencil atlas dimensions or source hash changed");
    }
    Ok(ATLAS.to_vec())
}

fn read_f32_le(raw: &[u8]) -> f32 {
    f32::from_le_bytes(raw.try_into().expect("four-byte F32 lane"))
}

fn records_from_native_vbo() -> Result<[[f32; 4]; INSTANCE_COUNT], &'static str> {
    if INSTANCE_VBO.len() != INSTANCE_BYTES || hash(INSTANCE_VBO) != NATIVE_INSTANCE_SHA256 {
        return Err("actual V17 instance VBO length or hash changed");
    }
    let mut records = [[0.0; 4]; INSTANCE_COUNT];
    for (index, record) in records.iter_mut().enumerate() {
        let at = index * INSTANCE_STRIDE;
        *record = [
            read_f32_le(&INSTANCE_VBO[at..at + 4]),
            read_f32_le(&INSTANCE_VBO[at + 4..at + 8]),
            read_f32_le(&INSTANCE_VBO[at + 8..at + 12]),
            read_f32_le(&INSTANCE_VBO[at + 12..at + 16]),
        ];
        if !record.iter().copied().all(f32::is_finite)
            || record[2] <= 0.0
            || record[3].fract() != 0.0
            || !(0.0..9.0).contains(&record[3])
        {
            return Err("native instance VBO contains an unsupported Pencil record");
        }
    }
    Ok(records)
}

fn corner_position(corner: Corner, point: Point, arithmetic: VertexArithmetic) -> [f32; 4] {
    let (x, y) = match arithmetic {
        VertexArithmetic::Separate32 => (
            f32_add(point.x, f32_mul(corner.x, point.half_width)),
            f32_add(point.y, f32_mul(corner.y, point.half_width)),
        ),
        VertexArithmetic::Fma32 => (
            f32_fma(corner.x, point.half_width, point.x),
            f32_fma(corner.y, point.half_width, point.y),
        ),
    };
    [x, y, 0.0, 1.0]
}

fn matrix_vector(vector: [f32; 4], arithmetic: VertexArithmetic) -> [f32; 4] {
    matrix_vector_with_projection(vector, arithmetic, &PROJECTION)
}

fn matrix_vector_with_projection(
    vector: [f32; 4],
    arithmetic: VertexArithmetic,
    projection: &[f32; 16],
) -> [f32; 4] {
    let mut result = [0.0; 4];
    for row in 0..4 {
        let mut value = 0.0_f32;
        for column in 0..4 {
            let product = projection[column * 4 + row];
            value = match arithmetic {
                VertexArithmetic::Separate32 => f32_add(value, f32_mul(product, vector[column])),
                VertexArithmetic::Fma32 => f32_fma(product, vector[column], value),
            };
        }
        result[row] = value;
    }
    result
}

fn to_window(clip: [f32; 4]) -> [f32; 2] {
    let ndc_x = f32_div(clip[0], clip[3]);
    let ndc_y = f32_div(clip[1], clip[3]);
    [
        f32_add(0.0, f32_mul(f32_add(ndc_x, 1.0), (WIDTH as f32) / 2.0)),
        f32_add(0.0, f32_mul(f32_add(ndc_y, 1.0), (HEIGHT as f32) / 2.0)),
    ]
}

#[derive(Clone, Copy)]
struct Plane {
    origin: f32,
    dx: f32,
    dy: f32,
}

fn plane_coeff(vertices: [[f32; 2]; 3], values: [f32; 3]) -> Result<Plane, &'static str> {
    let [a, b, c] = vertices;
    let dx01 = a[0] - b[0];
    let dy01 = a[1] - b[1];
    let dx20 = c[0] - a[0];
    let dy20 = c[1] - a[1];
    let area = dx01 * dy20 - dy01 * dx20;
    if area == 0.0 || !area.is_finite() {
        return Err("captured Pencil strip triangle is degenerate");
    }
    let ooa = 1.0 / area;
    let da01 = values[0] - values[1];
    let da20 = values[2] - values[0];
    let dx = da01 * (dy20 * ooa) - da20 * (dy01 * ooa);
    let dy = da20 * (dx01 * ooa) - da01 * (dx20 * ooa);
    let origin = values[0] - (dx * (a[0] - 0.5) + dy * (a[1] - 0.5));
    Ok(Plane { origin, dx, dy })
}

fn eval_plane(plane: Plane, x: f32, y: f32) -> f32 {
    plane.dy.mul_add(y, plane.dx.mul_add(x, plane.origin))
}

fn coverage_bounds(window: &[[f32; 2]; 4]) -> [(f32, f32); 2] {
    let snap = |coordinate: f32| (coordinate * 256.0).round_ties_even() / 256.0;
    std::array::from_fn(|axis| {
        let low = window
            .iter()
            .map(|point| point[axis])
            .fold(f32::INFINITY, f32::min);
        let high = window
            .iter()
            .map(|point| point[axis])
            .fold(f32::NEG_INFINITY, f32::max);
        (snap(low), snap(high))
    })
}

fn covers_pixel(bounds: [(f32, f32); 2], x: f32, y: f32) -> bool {
    bounds[0].0 <= x && x < bounds[0].1 && bounds[1].0 <= y && y < bounds[1].1
}

fn nearest_texel(normalized: f32, size: usize) -> usize {
    (f32_mul(normalized, size as f32).floor() as isize).clamp(0, size as isize - 1) as usize
}

fn quantize_unorm8(value: f32, rounding: UnormRound) -> u8 {
    let scaled = (value as f64 * 255.0).clamp(0.0, 255.0);
    let floor = scaled.floor();
    let fraction = scaled - floor;
    let rounded = match rounding {
        UnormRound::NearestUp => (scaled + 0.5).floor(),
        UnormRound::NearestEven => {
            if fraction < 0.5 {
                floor
            } else if fraction > 0.5 || (floor as u32 & 1) == 1 {
                floor + 1.0
            } else {
                floor
            }
        }
    };
    rounded as u8
}

fn selected_triangle(vertices: &[[f32; 2]; 4], px: f32, py: f32) -> usize {
    // Source-side selection used by the captured offline models. The native
    // target had no pixel centers on the strip diagonal for this profile.
    let dx = f32_div(
        f32_sub(px, vertices[0][0]),
        f32_sub(vertices[2][0], vertices[0][0]),
    );
    let dy = f32_div(
        f32_sub(py, vertices[0][1]),
        f32_sub(vertices[1][1], vertices[0][1]),
    );
    if f32_add(dx, dy) > 1.0 { 1 } else { 0 }
}

fn triangle_indices(triangle: usize) -> [usize; 3] {
    if triangle == 0 { [0, 1, 2] } else { [2, 1, 3] }
}

/// Render a bounded list of VBO-equivalent `[x, y, half_width, tile_id]`
/// records through the captured Pencil material profile. Production-side
/// callers can pass records from `old_pencil::sample_saved_stroke` after the
/// source geometry profile guard succeeds.
pub fn render_records(
    records: &[[f32; 4]],
    profile: Profile,
    atlas: &[u8],
) -> Result<Vec<u8>, &'static str> {
    render_records_on(
        records,
        profile,
        atlas,
        vec![0; WIDTH * HEIGHT * 4],
        [0, 0, 0, 255],
    )
}

fn render_records_on(
    records: &[[f32; 4]],
    profile: Profile,
    atlas: &[u8],
    output: Vec<u8>,
    color: [u8; 4],
) -> Result<Vec<u8>, &'static str> {
    render_records_on_with_projection(records, profile, atlas, output, color, &PROJECTION)
}

fn render_records_on_with_projection(
    records: &[[f32; 4]],
    profile: Profile,
    atlas: &[u8],
    mut output: Vec<u8>,
    color: [u8; 4],
    projection: &[f32; 16],
) -> Result<Vec<u8>, &'static str> {
    if records.is_empty() || output.len() != WIDTH * HEIGHT * 4 {
        return Err("invalid captured Pencil frame");
    }
    if atlas.len() != ATLAS_BYTES || hash(atlas) != NATIVE_ATLAS_SHA256 {
        return Err("native Pencil atlas profile mismatch");
    }
    for record in records {
        let point = Point {
            x: record[0],
            y: record[1],
            half_width: record[2],
            texture_id: record[3],
        };
        if !record.iter().copied().all(f32::is_finite)
            || point.half_width <= 0.0
            || point.texture_id.fract() != 0.0
            || !(0.0..9.0).contains(&point.texture_id)
        {
            return Err("Pencil paint record is outside the captured VBO profile");
        }
        let clip = QUAD.map(|corner| {
            matrix_vector_with_projection(
                corner_position(corner, point, profile.vertex),
                profile.vertex,
                projection,
            )
        });
        let window = clip.map(to_window);
        let bounds = coverage_bounds(&window);
        let [(x0, x1), (y0, y1)] = bounds;
        let ix0 = ((x0 - 0.5).ceil() as isize).max(0) as usize;
        let ix1 = ((x1 - 0.5).floor() as isize).min(WIDTH as isize - 1);
        let iy0 = ((y0 - 0.5).ceil() as isize).max(0) as usize;
        let iy1 = ((y1 - 0.5).floor() as isize).min(HEIGHT as isize - 1);
        if ix1 < 0 || iy1 < 0 || ix0 > ix1 as usize || iy0 > iy1 as usize {
            continue;
        }
        let quotient = f32_div(point.texture_id, 3.0);
        let tile_y = quotient.floor();
        let tile_x = f32_sub(point.texture_id, f32_mul(3.0, tile_y));
        let offset_x = f32_mul(f32_div(1.0, 3.0), tile_x);
        let offset_y = f32_mul(f32_div(1.0, 3.0), tile_y);
        let mut planes = [(
            Plane {
                origin: 0.0,
                dx: 0.0,
                dy: 0.0,
            },
            Plane {
                origin: 0.0,
                dx: 0.0,
                dy: 0.0,
            },
        ); 2];
        for (triangle, output) in planes.iter_mut().enumerate() {
            let ids = triangle_indices(triangle);
            let vertices = ids.map(|i| window[i]);
            *output = (
                plane_coeff(vertices, ids.map(|i| QUAD[i].frag_x / 3.0 + offset_x))?,
                plane_coeff(
                    vertices,
                    ids.map(|i| (1.0 - QUAD[i].frag_y) / 3.0 + offset_y),
                )?,
            );
        }
        for y_gl in iy0..=iy1 as usize {
            let py = f32_add(y_gl as f32, 0.5);
            for x in ix0..=ix1 as usize {
                let px = f32_add(x as f32, 0.5);
                if !covers_pixel(bounds, px, py) {
                    continue;
                }
                let selected = selected_triangle(&window, px, py);
                let triangle = match profile.plane {
                    VaryingPlane::Triangle0 => 0,
                    VaryingPlane::Triangle1 => 1,
                    VaryingPlane::SourceSideSelection => selected,
                };
                let u = eval_plane(planes[triangle].0, x as f32, y_gl as f32);
                let v = eval_plane(planes[triangle].1, x as f32, y_gl as f32);
                let tex_x = nearest_texel(u, ATLAS_WIDTH);
                let tex_y = nearest_texel(v, ATLAS_HEIGHT);
                let tex_alpha = atlas[(tex_y * ATLAS_WIDTH + tex_x) * 4 + 3];
                if tex_alpha == 0 {
                    continue;
                }
                let source_alpha = f32_mul(
                    super::super::host_rgba8::sample_unorm8_half_rtz(tex_alpha),
                    f32_div(color[3] as f32, 255.0),
                );
                let y_top = HEIGHT - 1 - y_gl;
                let at = (y_top * WIDTH + x) * 4;
                let multiply = |a: u8, b: u8| {
                    let product = u32::from(a) * u32::from(b);
                    ((product + (product >> 8) + 128) >> 8) as u8
                };
                let alpha = quantize_unorm8(source_alpha, profile.unorm);
                let inverse = 255 - alpha;
                for channel in 0..4 {
                    let source = if channel == 3 {
                        alpha
                    } else {
                        multiply(color[channel], alpha)
                    };
                    output[at + channel] =
                        source.saturating_add(multiply(output[at + channel], inverse));
                }
            }
        }
    }
    Ok(output)
}

fn render_native_vbo(profile: Profile, atlas: &[u8]) -> Result<Vec<u8>, &'static str> {
    let records = records_from_native_vbo()?;
    render_records(&records, profile, atlas)
}

fn sparse_frame(rgba: &[u8]) -> Vec<[u8; SPARSE_RECORD_BYTES]> {
    let mut rows = Vec::new();
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let at = (y * WIDTH + x) * 4;
            if rgba[at..at + 4].iter().any(|&channel| channel != 0) {
                let mut record = [0_u8; SPARSE_RECORD_BYTES];
                record[0..2].copy_from_slice(&(x as u16).to_le_bytes());
                record[2..4].copy_from_slice(&(y as u16).to_le_bytes());
                record[4..8].copy_from_slice(&rgba[at..at + 4]);
                rows.push(record);
            }
        }
    }
    rows
}

fn expected_native_sparse() -> Result<Vec<[u8; SPARSE_RECORD_BYTES]>, &'static str> {
    if NATIVE_SPARSE_RGBA8.len() != EXPECTED_SPARSE_COUNT * SPARSE_RECORD_BYTES
        || hash(NATIVE_SPARSE_RGBA8) != NATIVE_SPARSE_SHA256
    {
        return Err("native sparse FBO fixture length or hash changed");
    }
    Ok(NATIVE_SPARSE_RGBA8
        .as_chunks::<SPARSE_RECORD_BYTES>()
        .0
        .to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colored_frames_match_native_per_stamp_composition() {
        type CaptureCase<'a> = (&'a [u8], &'a [u8], &'a [u8], [u8; 4]);
        let cases: [CaptureCase<'_>; 4] = [
            (
                include_bytes!("../../../../../conformance/old-pencil-color/red-pre.rgba.zlib"),
                include_bytes!("../../../../../conformance/old-pencil-color/red-post.rgba.zlib"),
                include_bytes!("../../../../../conformance/old-pencil-color/alpha.vbo"),
                [221, 68, 17, 255],
            ),
            (
                include_bytes!("../../../../../conformance/old-pencil-color/red-pre.rgba.zlib"),
                include_bytes!("../../../../../conformance/old-pencil-color/alpha-post.rgba.zlib"),
                include_bytes!("../../../../../conformance/old-pencil-color/alpha.vbo"),
                [221, 68, 17, 128],
            ),
            (
                include_bytes!(
                    "../../../../../conformance/old-pencil-color/backdrop-pre.rgba.zlib"
                ),
                include_bytes!(
                    "../../../../../conformance/old-pencil-color/backdrop-post.rgba.zlib"
                ),
                include_bytes!("../../../../../conformance/old-pencil-color/alpha.vbo"),
                [221, 68, 17, 128],
            ),
            (
                include_bytes!("../../../../../conformance/old-pencil-color/varied-pre.rgba.zlib"),
                include_bytes!("../../../../../conformance/old-pencil-color/varied-post.rgba.zlib"),
                include_bytes!("../../../../../conformance/old-pencil-color/varied.vbo"),
                [221, 68, 17, 128],
            ),
        ];
        let atlas = decode_atlas().unwrap();
        for (pre, expected, vbo, color) in cases {
            let decode = |bytes: &[u8]| {
                let mut out = Vec::new();
                ZlibDecoder::new(bytes).read_to_end(&mut out).unwrap();
                out
            };
            let records: Vec<[f32; 4]> = vbo
                .as_chunks::<16>()
                .0
                .iter()
                .map(|chunk| {
                    std::array::from_fn(|i| {
                        f32::from_le_bytes(chunk[i * 4..i * 4 + 4].try_into().unwrap())
                    })
                })
                .collect();
            for vertex in [VertexArithmetic::Separate32, VertexArithmetic::Fma32] {
                for plane in [
                    VaryingPlane::Triangle0,
                    VaryingPlane::Triangle1,
                    VaryingPlane::SourceSideSelection,
                ] {
                    for unorm in [UnormRound::NearestEven, UnormRound::NearestUp] {
                        let actual = render_records_on(
                            &records,
                            Profile {
                                vertex,
                                plane,
                                unorm,
                            },
                            &atlas,
                            decode(pre),
                            color,
                        )
                        .unwrap();
                        assert_eq!(actual, decode(expected));
                    }
                }
            }
        }
    }

    #[test]
    fn translated_native_frames_expose_pencil_paint_limits() {
        let decode = |bytes: &[u8]| {
            let mut out = Vec::new();
            ZlibDecoder::new(bytes).read_to_end(&mut out).unwrap();
            out
        };
        let cases: [(&[u8], &[u8]); 2] = [
            (
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/translation_only-post.rgba.zlib"
                ),
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/translation_only.vbo"
                ),
            ),
            (
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/offset125-post.rgba.zlib"
                ),
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/offset125.vbo"
                ),
            ),
        ];
        let pre = decode(include_bytes!(
            "../../../../../conformance/old-pencil-color/varied-pre.rgba.zlib"
        ));
        let atlas = decode_atlas().unwrap();
        for (expected, vbo) in cases {
            let expected = decode(expected);
            assert_eq!(expected.len(), WIDTH * HEIGHT * 4);
            let records: Vec<[f32; 4]> = vbo
                .as_chunks::<16>()
                .0
                .iter()
                .map(|v| {
                    std::array::from_fn(|i| {
                        f32::from_le_bytes(v[4 * i..4 * i + 4].try_into().unwrap())
                    })
                })
                .collect();
            assert_eq!(records.len(), 42);
            for vertex in [VertexArithmetic::Separate32, VertexArithmetic::Fma32] {
                for plane in [
                    VaryingPlane::Triangle0,
                    VaryingPlane::Triangle1,
                    VaryingPlane::SourceSideSelection,
                ] {
                    for unorm in [UnormRound::NearestEven, UnormRound::NearestUp] {
                        let actual = render_records_on(
                            &records,
                            Profile {
                                vertex,
                                plane,
                                unorm,
                            },
                            &atlas,
                            pre.clone(),
                            [221, 68, 17, 128],
                        )
                        .unwrap();
                        assert_eq!(actual.len(), WIDTH * HEIGHT * 4);
                        let diffs: Vec<_> = actual
                            .as_chunks::<4>()
                            .0
                            .iter()
                            .zip(expected.as_chunks::<4>().0.iter())
                            .enumerate()
                            .filter(|(_, (a, b))| a != b)
                            .collect();
                        assert!(diffs.is_empty(), "translated frame mismatch: {diffs:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn snapped_bounds_match_injected_host_coverage_masks() {
        let fixture = include_bytes!(
            "../../../../../conformance/old-pencil-counterexample/edge-controls.bin"
        );
        assert_eq!(fixture.len(), 28 * 66);
        for case in fixture.as_chunks::<66>().0 {
            let point = Point {
                x: read_f32_le(&case[0..4]),
                y: read_f32_le(&case[4..8]),
                half_width: read_f32_le(&case[8..12]),
                texture_id: 6.0,
            };
            let window = QUAD.map(|corner| {
                to_window(matrix_vector(
                    corner_position(corner, point, VertexArithmetic::Separate32),
                    VertexArithmetic::Separate32,
                ))
            });
            let bounds = coverage_bounds(&window);
            for y in 0..18 {
                for x in 0..24 {
                    let i = y * 24 + x;
                    let expected = case[12 + i / 8] & (1 << (i % 8)) != 0;
                    let px = (x + 156) as f32 + 0.5;
                    let py = (y + 98) as f32 + 0.5;
                    assert_eq!(
                        covers_pixel(bounds, px, py),
                        expected,
                        "point {point:?}, pixel {px} {py}"
                    );
                }
            }
        }
    }

    #[test]
    fn instrumented_varying_sweep_rejects_simple_interpolation_hypotheses() {
        let mut fixture = Vec::new();
        ZlibDecoder::new(
            include_bytes!(
                "../../../../../conformance/old-pencil-counterexample/varying-sweep-regions.zlib"
            )
            .as_slice(),
        )
        .read_to_end(&mut fixture)
        .unwrap();
        assert_eq!(fixture.len(), 40 * 24 * 18 * 4);
        assert_eq!(
            hash(&fixture),
            "ad5bed96058f660cbf3debbe620df4eaa1a2f0e1614207278ccd45f79820a261"
        );
        let mut scores = [0, 0, 0];
        for (i, pixels) in fixture.as_chunks::<{ 24 * 18 * 4 }>().0.iter().enumerate() {
            let cx = 168.0 + (i / 5) as f32 / 8.0;
            let width = [1.0, 3.0, 4.0, 5.0, 7.0][i % 5];
            let window = |x| f32_mul(f32_add(f32_sub(f32_mul(x, PROJECTION[0]), 1.0), 1.0), 960.0);
            let x0 = window(cx - width);
            let x1 = window(cx + width);
            let dx = f32_div(1.0, f32_sub(x1, x0));
            let constant = -f32_mul(x0, dx);
            for x in 156..180 {
                let offset = ((107 - 98) * 24 + x - 156) * 4;
                let observed = read_f32_le(&pixels[offset..offset + 4]);
                if !(0.0 < observed && observed < 1.0) {
                    continue;
                }
                let base = x / 2 * 2;
                let vertex = f32_mul(f32_sub(x as f32 + 0.5, x0), dx);
                let anchor = f32_add(
                    f32_add(f32_mul(base as f32 + 0.5, dx), constant),
                    f32_mul((x - base) as f32, dx),
                );
                scores[0] += 1;
                scores[1] += usize::from(observed == vertex);
                scores[2] += usize::from(observed == anchor);
            }
        }
        assert_eq!(scores, [316, 32, 52]);
    }

    #[test]
    fn source_side_profile_matches_injected_wide_controls() {
        let mut fixture = Vec::new();
        ZlibDecoder::new(
            include_bytes!(
                "../../../../../conformance/old-pencil-counterexample/wide-controls.zlib"
            )
            .as_slice(),
        )
        .read_to_end(&mut fixture)
        .unwrap();
        assert_eq!(fixture.len(), 12 * (12 + 24 * 18 * 4));
        let atlas = decode_atlas().unwrap();
        for rec in fixture.as_chunks::<{ 12 + 24 * 18 * 4 }>().0 {
            let record = [
                read_f32_le(&rec[0..4]),
                read_f32_le(&rec[4..8]),
                read_f32_le(&rec[8..12]),
                6.0,
            ];
            let actual = render_records_on(
                &[record],
                Profile {
                    vertex: VertexArithmetic::Separate32,
                    plane: VaryingPlane::SourceSideSelection,
                    unorm: UnormRound::NearestEven,
                },
                &atlas,
                vec![0; WIDTH * HEIGHT * 4],
                [221, 68, 17, 128],
            )
            .unwrap();
            for y in 0..18 {
                for x in 0..24 {
                    let native = 12 + (y * 24 + x) * 4;
                    let frame = ((HEIGHT - 1 - (98 + y)) * WIDTH + 156 + x) * 4;
                    assert_eq!(
                        &actual[frame..frame + 4],
                        &rec[native..native + 4],
                        "point {record:?}, GL pixel {}, {}",
                        156 + x,
                        98 + y
                    );
                }
            }
        }
    }

    #[test]
    fn saved_input_to_native_vbo_and_host_pixels_matches_pressure_controls() {
        use super::super::{PencilStyleProfile, SavedPoint, SavedStroke, sample_saved_stroke};
        let decode = |bytes: &[u8]| {
            let mut out = Vec::new();
            ZlibDecoder::new(bytes).read_to_end(&mut out).unwrap();
            out
        };
        let cases = [
            ([(241.375,208.625),(271.375,208.625),(301.375,218.625),(331.375,238.625)], [1.0,0.15,0.95,0.35], include_bytes!("../../../../../conformance/old-pencil-counterexample/long_pressure.vbo").as_slice(), include_bytes!("../../../../../conformance/old-pencil-counterexample/long_pressure-post.rgba.zlib").as_slice()),
            ([(433.75,307.375),(443.75,307.375),(453.75,307.375),(463.75,317.375)], [0.95,0.1,0.45,0.8], include_bytes!("../../../../../conformance/old-pencil-counterexample/translated_pressure.vbo").as_slice(), include_bytes!("../../../../../conformance/old-pencil-counterexample/translated_pressure-post.rgba.zlib").as_slice()),
        ];
        let pre = decode(include_bytes!(
            "../../../../../conformance/old-pencil-color/varied-pre.rgba.zlib"
        ));
        let atlas = decode_atlas().unwrap();
        for (points, pressures, native_vbo, native_post) in cases {
            let points = points.map(|(x, y)| SavedPoint { x, y });
            let records = sample_saved_stroke(
                &SavedStroke {
                    pen_name: "com.samsung.android.sdk.pen.pen.preload.Pencil",
                    tool_type: 2,
                    stroke_type: 0,
                    curve_enabled: true,
                    points: &points,
                    pressures: &pressures,
                    size: 2.0,
                },
                PencilStyleProfile {
                    size: 2.0,
                    point_width_override: None,
                },
            )
            .unwrap()
            .records;
            let packed: Vec<u8> = records
                .iter()
                .flatten()
                .flat_map(|lane| lane.to_le_bytes())
                .collect();
            assert_eq!(packed, native_vbo);
            let actual = render_records_on(
                &records,
                Profile {
                    vertex: VertexArithmetic::Separate32,
                    plane: VaryingPlane::SourceSideSelection,
                    unorm: UnormRound::NearestEven,
                },
                &atlas,
                pre.clone(),
                [221, 68, 17, 128],
            )
            .unwrap();
            assert_eq!(actual, decode(native_post));
        }
    }

    #[test]
    fn source_side_profile_matches_native_pressure_strokes() {
        let decode = |data: &[u8]| {
            let mut out = Vec::new();
            ZlibDecoder::new(data).read_to_end(&mut out).unwrap();
            out
        };
        let cases: [(&[u8], &[u8], usize); 2] = [
            (
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/long_pressure.vbo"
                ),
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/long_pressure-post.rgba.zlib"
                ),
                128,
            ),
            (
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/translated_pressure.vbo"
                ),
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/translated_pressure-post.rgba.zlib"
                ),
                46,
            ),
        ];
        let pre = decode(include_bytes!(
            "../../../../../conformance/old-pencil-color/varied-pre.rgba.zlib"
        ));
        let atlas = decode_atlas().unwrap();
        for (vbo, post, count) in cases {
            let records: Vec<[f32; 4]> = vbo
                .as_chunks::<16>()
                .0
                .iter()
                .map(|chunk| std::array::from_fn(|i| read_f32_le(&chunk[4 * i..4 * i + 4])))
                .collect();
            assert_eq!(records.len(), count);
            let actual = render_records_on(
                &records,
                Profile {
                    vertex: VertexArithmetic::Separate32,
                    plane: VaryingPlane::SourceSideSelection,
                    unorm: UnormRound::NearestEven,
                },
                &atlas,
                pre.clone(),
                [221, 68, 17, 128],
            )
            .unwrap();
            assert_eq!(actual, decode(post));
        }
    }

    #[test]
    fn size8_colors_match_native_source_vbo_and_pixels() {
        use super::super::{PencilStyleProfile, SavedPoint, SavedStroke, sample_saved_stroke};
        let points = [
            (241.375, 208.625),
            (271.375, 208.625),
            (301.375, 218.625),
            (331.375, 238.625),
        ]
        .map(|(x, y)| SavedPoint { x, y });
        let records = sample_saved_stroke(
            &SavedStroke {
                pen_name: "com.samsung.android.sdk.pen.pen.preload.Pencil",
                tool_type: 2,
                stroke_type: 0,
                curve_enabled: true,
                points: &points,
                pressures: &[1.0, 0.15, 0.95, 0.35],
                size: 8.0,
            },
            PencilStyleProfile {
                size: 8.0,
                point_width_override: None,
            },
        )
        .unwrap()
        .records;
        let packed: Vec<u8> = records
            .iter()
            .flatten()
            .flat_map(|lane| lane.to_le_bytes())
            .collect();
        assert_eq!(
            packed,
            include_bytes!("../../../../../conformance/old-pencil-counterexample/size8_color.vbo")
        );
        let decode = |bytes: &[u8]| {
            let mut out = Vec::new();
            ZlibDecoder::new(bytes).read_to_end(&mut out).unwrap();
            out
        };
        let pre = decode(include_bytes!(
            "../../../../../conformance/old-pencil-color/varied-pre.rgba.zlib"
        ));
        let atlas = decode_atlas().unwrap();
        let cases=[
            ([32,136,224,192],include_bytes!("../../../../../conformance/old-pencil-counterexample/size8_color-post.rgba.zlib").as_slice()),
            ([241,48,160,64],include_bytes!("../../../../../conformance/old-pencil-counterexample/size8_alpha64-post.rgba.zlib").as_slice()),
            ([16,221,112,127],include_bytes!("../../../../../conformance/old-pencil-counterexample/size8_alpha127-post.rgba.zlib").as_slice()),
            ([160,32,240,254],include_bytes!("../../../../../conformance/old-pencil-counterexample/size8_alpha254-post.rgba.zlib").as_slice()),
        ];
        for (color, native) in cases {
            let actual = render_records_on(
                &records,
                Profile {
                    vertex: VertexArithmetic::Separate32,
                    plane: VaryingPlane::SourceSideSelection,
                    unorm: UnormRound::NearestEven,
                },
                &atlas,
                pre.clone(),
                color,
            )
            .unwrap();
            assert_eq!(actual, decode(native), "color {color:?}");
        }
        let sample = crate::ink::host_rgba8::sample_unorm8_half_rtz(95);
        assert_eq!(
            quantize_unorm8(sample * (192.0 / 255.0), UnormRound::NearestEven),
            71
        );
        assert_eq!(
            quantize_unorm8((95.0 / 255.0) * (192.0 / 255.0), UnormRound::NearestEven),
            72
        );
    }

    #[test]
    fn host_projection_2x_full_frames() {
        let cases=[
            (include_bytes!("../../../../../conformance/old-pencil-counterexample/size8_color.vbo").as_slice(), include_bytes!("../../../../../conformance/old-pencil-counterexample/size8_color_scale2-native.rgba.zlib").as_slice(), [32,136,224,192]),
            (include_bytes!("../../../../../conformance/old-pencil-counterexample/long_pressure.vbo").as_slice(), include_bytes!("../../../../../conformance/old-pencil-counterexample/long_pressure_scale2-native.rgba.zlib").as_slice(), [221,68,17,128]),
        ];
        let decode = |bytes: &[u8]| {
            let mut out = Vec::new();
            ZlibDecoder::new(bytes).read_to_end(&mut out).unwrap();
            out
        };
        let pre = decode(include_bytes!(
            "../../../../../conformance/old-pencil-color/varied-pre.rgba.zlib"
        ));
        let atlas = decode_atlas().unwrap();
        let mut projection = PROJECTION;
        projection[0] *= 2.0;
        projection[5] *= 2.0;
        assert_eq!(projection[0].to_bits(), 0x3b08_8889);
        assert_eq!(projection[5].to_bits(), 0x3b72_b9d6);
        for (vbo, expected, color) in cases {
            let records: Vec<[f32; 4]> = vbo
                .as_chunks::<16>()
                .0
                .iter()
                .map(|chunk| std::array::from_fn(|i| read_f32_le(&chunk[i * 4..i * 4 + 4])))
                .collect();
            let actual = render_records_on_with_projection(
                &records,
                Profile {
                    vertex: VertexArithmetic::Separate32,
                    plane: VaryingPlane::SourceSideSelection,
                    unorm: UnormRound::NearestEven,
                },
                &atlas,
                pre.clone(),
                color,
                &projection,
            )
            .unwrap();
            assert_eq!(actual, decode(expected), "color {color:?}");
        }
    }

    #[test]
    fn serialized_synthetic_model_context_reconstructs_native_records_and_frames() {
        use super::super::{
            PencilStyleProfile, SavedPoint, SavedStroke, sample_saved_stroke,
            sample_saved_stroke_equal_threshold,
        };
        let mut payload = include_bytes!(
            "../../../../../conformance/old-pencil-counterexample/serialized-pencil-color.payload"
        )
        .to_vec();
        let native = include_bytes!(
            "../../../../../conformance/old-pencil-counterexample/serialized-pencil.vbo"
        );
        let decode = |data: &[u8]| {
            let mut out = Vec::new();
            ZlibDecoder::new(data).read_to_end(&mut out).unwrap();
            out
        };
        let pre = decode(include_bytes!(
            "../../../../../conformance/old-pencil-color/varied-pre.rgba.zlib"
        ));
        let atlas = decode_atlas().unwrap();
        let cases = [
            (0x8024_a0d8_u32, include_bytes!("../../../../../conformance/old-pencil-counterexample/serialized-pencil-color-post.rgba.zlib").as_slice()),
            (0x40f1_30a0_u32, include_bytes!("../../../../../conformance/old-pencil-counterexample/serialized-pencil-color-alt-post.rgba.zlib").as_slice()),
        ];
        for (argb, post) in cases {
            payload[2627..2631].copy_from_slice(&argb.to_le_bytes());
            let object = crate::StoredObject {
                object_type: crate::ObjectType::Stroke,
                declared_size: (payload.len() + 32) as u32,
                payload_offset: 0,
                payload_size: payload.len(),
                integrity_trailer: [0; 32],
                children: vec![],
            };
            let stroke = object
                .decode_stroke(&payload, &crate::ParseLimits::default())
                .unwrap();
            let rendering = stroke.rendering.as_ref().unwrap();
            assert!(rendering.metadata.is_some());
            assert!(rendering.pen_name.is_none());
            assert!(rendering.advanced_settings.is_none());
            assert!(
                rendering.properties.compressed
                    && rendering.properties.stylus_channels
                    && rendering.properties.millisecond_timestamps
            );
            assert_eq!(rendering.tool_type_raw, 2);
            assert_eq!(rendering.style.pen_name_id, Some(2));
            assert_eq!(rendering.style.stroke_type_raw, None);
            assert_eq!(rendering.style.color_argb, Some(argb));
            let mut channels = Vec::new();
            for point in &stroke.points {
                for value in [point.x, point.y] {
                    let f = value as f32;
                    assert_eq!(f64::from(f), value);
                    channels.extend_from_slice(&f.to_le_bytes());
                }
            }
            for &value in &stroke.pressures {
                let f = value as f32;
                assert_eq!(f64::from(f), value);
                channels.extend_from_slice(&f.to_le_bytes());
            }
            for &value in &stroke.timestamps {
                channels.extend_from_slice(&i32::try_from(value).unwrap().to_le_bytes());
            }
            for &value in stroke.tilts.iter().chain(&stroke.orientations) {
                let f = value as f32;
                assert_eq!(f64::from(f), value);
                channels.extend_from_slice(&f.to_le_bytes());
            }
            assert_eq!(
                channels,
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/serialized-pencil-channels.bin"
                )
            );
            assert_eq!(
                stroke.color,
                Some(crate::Color {
                    r: (argb >> 16) as u8,
                    g: (argb >> 8) as u8,
                    b: argb as u8
                })
            );
            let points: Vec<_> = stroke
                .points
                .iter()
                .map(|p| SavedPoint {
                    x: p.x as f32,
                    y: p.y as f32,
                })
                .collect();
            let pressures: Vec<_> = stroke.pressures.iter().map(|&p| p as f32).collect();
            let size = stroke.pen_width;
            // The fixture decodes an unresolved name ID and no stroke-type field.
            // These two values are supplied from the separately observed native
            // OnAttach name-table resolution and GetStrokeType result.
            let source = SavedStroke {
                pen_name: "com.samsung.android.sdk.pen.pen.preload.Pencil",
                tool_type: i32::from(rendering.tool_type_raw),
                stroke_type: 0,
                curve_enabled: rendering.properties.compressed,
                points: &points,
                pressures: &pressures,
                size,
            };
            let style = PencilStyleProfile {
                size,
                point_width_override: None,
            };
            assert_eq!(
                sample_saved_stroke(&source, style).unwrap().records.len(),
                202
            );
            let records = sample_saved_stroke_equal_threshold(&source, style)
                .unwrap()
                .records;
            let packed: Vec<u8> = records
                .iter()
                .flatten()
                .flat_map(|f| f.to_le_bytes())
                .collect();
            assert_eq!(packed, native);
            let frame = render_records_on(
                &records,
                Profile {
                    vertex: VertexArithmetic::Separate32,
                    plane: VaryingPlane::SourceSideSelection,
                    unorm: UnormRound::NearestEven,
                },
                &atlas,
                pre.clone(),
                [
                    (argb >> 16) as u8,
                    (argb >> 8) as u8,
                    argb as u8,
                    (argb >> 24) as u8,
                ],
            )
            .unwrap();
            assert_eq!(frame, decode(post));
        }
    }

    #[test]
    fn width7_injected_original_shader_remains_a_counterexample() {
        let mut expected = Vec::new();
        ZlibDecoder::new(
            include_bytes!("../../../../../conformance/old-pencil-counterexample/width7-rgba.zlib")
                .as_slice(),
        )
        .read_to_end(&mut expected)
        .unwrap();
        assert_eq!(expected.len(), 24 * 18 * 4);
        let actual = render_records_on(
            &[[168.125, 107.125, 7.0, 6.0]],
            Profile {
                vertex: VertexArithmetic::Separate32,
                plane: VaryingPlane::Triangle0,
                unorm: UnormRound::NearestEven,
            },
            &decode_atlas().unwrap(),
            vec![0; WIDTH * HEIGHT * 4],
            [221, 68, 17, 128],
        )
        .unwrap();
        let mut differences = Vec::new();
        for y in 0..18 {
            for x in 0..24 {
                let src = (y * 24 + x) * 4;
                let dst = ((HEIGHT - 1 - (98 + y)) * WIDTH + 156 + x) * 4;
                if actual[dst..dst + 4] != expected[src..src + 4] {
                    differences.push((156 + x, 98 + y));
                }
            }
        }
        assert_eq!(differences.len(), 9);
        assert!(differences.iter().all(|(x, _)| *x == 172));
    }

    #[cfg(feature = "render")]
    #[test]
    fn nearest_image_one_hot_boundary_control() {
        use base64::Engine as _;

        let mut observations = Vec::new();
        for translation in [0_u32, 1, 4] {
            for hot in [0_usize, 1] {
                let mut pixels = vec![0_u8; 48 * 4];
                pixels[hot * 4 + 3] = 255;
                let mut encoded = Vec::new();
                {
                    let mut encoder = png::Encoder::new(&mut encoded, 48, 1);
                    encoder.set_color(png::ColorType::Rgba);
                    encoder.set_depth(png::BitDepth::Eight);
                    encoder
                        .write_header()
                        .unwrap()
                        .write_image_data(&pixels)
                        .unwrap();
                }
                let uri = base64::engine::general_purpose::STANDARD.encode(encoded);
                let origin = 2.5_f32 + translation as f32;
                let svg = format!(
                    r#"<svg xmlns="http://www.w3.org/2000/svg" width="104" height="6"><image x="{origin}" y="1" width="96" height="4" preserveAspectRatio="none" image-rendering="optimizeSpeed" href="data:image/png;base64,{uri}"/></svg>"#
                );
                let tree = resvg::usvg::Tree::from_str(&svg, &Default::default()).unwrap();
                let mut pixmap = resvg::tiny_skia::Pixmap::new(104, 6).unwrap();
                resvg::render(
                    &tree,
                    resvg::tiny_skia::Transform::identity(),
                    &mut pixmap.as_mut(),
                );
                let mut differences = Vec::new();
                for x in 0..104_u32 {
                    let center = x as f32 + 0.5;
                    let expected = if center >= origin
                        && center < origin + 96.0
                        && nearest_texel((center - origin) / 96.0, 48) == hot
                    {
                        255
                    } else {
                        0
                    };
                    let actual = pixmap.data()[((2 * 104 + x) * 4 + 3) as usize];
                    if actual != expected {
                        differences.push((x, expected, actual));
                    }
                }
                observations.push(differences);
            }
        }
        // The nearest sampler owns the internal boundary correctly, but the
        // image's outer geometric edge is still antialiased by resvg.
        assert_eq!(
            observations,
            [
                vec![(2, 255, 128)],
                vec![],
                vec![(3, 255, 128)],
                vec![],
                vec![(6, 255, 128)],
                vec![],
            ]
        );
    }

    #[test]
    fn one_hot_texel_boundary_control_reports_svg_coverage() {
        // A 48-texel atlas tile, two output units per texel. Its origin
        // puts the boundary between texels 0 and 1 exactly on a pixel
        // center. Integer translations preserve that relation. The SVG
        // rectangle is the source-derived extent of the one nonzero texel;
        // it does not encode an empirically adjusted phase or edge rule.
        const OUTPUT_WIDTH: u32 = 104;
        const OUTPUT_HEIGHT: u32 = 6;
        const TILE_TEXELS: usize = 48;
        let mut observations = 0;
        for translation in [0_u32, 1, 4] {
            let origin = 2.5_f32 + translation as f32;
            for hot in [0_usize, 1] {
                let left = origin + hot as f32 * 2.0;
                let svg = format!(
                    r#"<svg xmlns="http://www.w3.org/2000/svg" width="{OUTPUT_WIDTH}" height="{OUTPUT_HEIGHT}"><rect x="{left}" y="1" width="2" height="4" fill="black"/></svg>"#
                );
                let tree = resvg::usvg::Tree::from_str(&svg, &Default::default()).unwrap();
                let mut pixmap =
                    resvg::tiny_skia::Pixmap::new(OUTPUT_WIDTH, OUTPUT_HEIGHT).unwrap();
                resvg::render(
                    &tree,
                    resvg::tiny_skia::Transform::identity(),
                    &mut pixmap.as_mut(),
                );

                let boundary_x = 4 + translation;
                let mut differences = Vec::new();
                for x in 0..OUTPUT_WIDTH {
                    let center = x as f32 + 0.5;
                    let source_alpha = if center >= origin
                        && center < origin + (TILE_TEXELS as f32 * 2.0)
                        && nearest_texel((center - origin) / 96.0, TILE_TEXELS) == hot
                    {
                        255
                    } else {
                        0
                    };
                    let svg_alpha = pixmap.data()[((2 * OUTPUT_WIDTH + x) * 4 + 3) as usize];
                    if source_alpha != svg_alpha {
                        differences.push((x, source_alpha, svg_alpha));
                    }
                }
                let boundary_texel =
                    nearest_texel((boundary_x as f32 + 0.5 - origin) / 96.0, TILE_TEXELS);
                assert_eq!(boundary_texel, 1, "source boundary ownership");
                let svg_boundary_alpha =
                    pixmap.data()[((2 * OUTPUT_WIDTH + boundary_x) * 4 + 3) as usize];
                assert_eq!(svg_boundary_alpha, 128);
                let left_pixel = 2 + translation + hot as u32 * 2;
                assert_eq!(
                    differences,
                    [(left_pixel, 255, 128), (left_pixel + 2, 0, 128)],
                    "SVG antialiasing splits exact one-hot texel edges"
                );
                observations += 1;
            }
        }
        assert_eq!(observations, 6);
    }

    #[test]
    fn captured_source_profile_pins_actual_vbo_atlas_and_sparse_target() {
        assert_eq!(INSTANCE_VBO.len(), 752);
        assert_eq!(hash(INSTANCE_VBO), NATIVE_INSTANCE_SHA256);
        assert_eq!(NATIVE_SPARSE_RGBA8.len(), 280);
        assert_eq!(hash(NATIVE_SPARSE_RGBA8), NATIVE_SPARSE_SHA256);
        assert_eq!(ATLAS.len(), ATLAS_BYTES);
        assert_eq!(hash(ATLAS), NATIVE_ATLAS_SHA256);
        let atlas = decode_atlas().unwrap();
        assert_eq!(atlas.len(), ATLAS_BYTES);
        let sparse = expected_native_sparse().unwrap();
        assert_eq!(sparse.len(), EXPECTED_SPARSE_COUNT);
        assert!(sparse.windows(2).all(|pair| {
            let left = (
                u16::from_le_bytes([pair[0][2], pair[0][3]]),
                u16::from_le_bytes([pair[0][0], pair[0][1]]),
            );
            let right = (
                u16::from_le_bytes([pair[1][2], pair[1][3]]),
                u16::from_le_bytes([pair[1][0], pair[1][1]]),
            );
            left < right
        }));
        assert!(
            sparse
                .iter()
                .all(|pixel| pixel[4..7] == [0, 0, 0] && pixel[7] != 0)
        );
    }

    #[test]
    fn all_twelve_source_f32_plane_nearest_source_over_profiles_match_full_frame() {
        let atlas = decode_atlas().unwrap();
        let expected = expected_native_sparse().unwrap();
        let mut compared = 0;
        for vertex in [VertexArithmetic::Separate32, VertexArithmetic::Fma32] {
            for plane in [
                VaryingPlane::Triangle0,
                VaryingPlane::Triangle1,
                VaryingPlane::SourceSideSelection,
            ] {
                for unorm in [UnormRound::NearestEven, UnormRound::NearestUp] {
                    let profile = Profile {
                        vertex,
                        plane,
                        unorm,
                    };
                    let frame = render_native_vbo(profile, &atlas)
                        .expect("captured V17 profile is bounded");
                    assert_eq!(frame.len(), WIDTH * HEIGHT * 4);
                    assert_eq!(
                        sparse_frame(&frame),
                        expected,
                        "profile {vertex:?}/{plane:?}/{unorm:?}"
                    );
                    compared += 1;
                }
            }
        }
        assert_eq!(compared, 12);
    }
}

#[cfg(test)]
mod tool1_boundary_tests {
    use super::super::{
        PencilStyleProfile, SavedPoint, SavedStroke, sample_synthetic_tool1_object_stroke,
    };
    use super::*;

    fn decode(data: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        ZlibDecoder::new(data).read_to_end(&mut bytes).unwrap();
        bytes
    }

    #[test]
    fn native_tool1_thresholds_and_unsupported_point() {
        type Case<'a> = (&'a str, f32, [f32; 4], &'a [u8], &'a [u8], usize);
        let cases: [Case<'_>; 3] = [
            (
                "low_exact",
                5.0,
                [100.0, 100.999_99, 101.0, 101.000_01],
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/low_exact.vbo"
                ),
                include_bytes!("../../../../../conformance/old-pencil-counterexample/low_exact.z"),
                0,
            ),
            (
                "high_exact",
                2.0,
                [100.0, 150.0, 150.0, 150.0],
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/high_exact.vbo"
                ),
                include_bytes!("../../../../../conformance/old-pencil-counterexample/high_exact.z"),
                1,
            ),
            (
                "nomove",
                2.0,
                [100.0, 100.1, 100.2, 100.3],
                include_bytes!("../../../../../conformance/old-pencil-counterexample/nomove.vbo"),
                include_bytes!("../../../../../conformance/old-pencil-counterexample/nomove.z"),
                0,
            ),
        ];
        let pre = decode(include_bytes!(
            "../../../../../conformance/old-pencil-color/varied-pre.rgba.zlib"
        ));
        let atlas = decode_atlas().unwrap();
        for (name, size, xs, native, post, expected_diff) in cases {
            let points: Vec<_> = xs.iter().map(|x| SavedPoint { x: *x, y: 100.0 }).collect();
            let pressures = [1.0, 0.15, 0.95, 0.35];
            let source = SavedStroke {
                pen_name: "com.samsung.android.sdk.pen.pen.preload.Pencil",
                tool_type: 1,
                stroke_type: 0,
                curve_enabled: true,
                points: &points,
                pressures: &pressures,
                size,
            };
            let sampled = sample_synthetic_tool1_object_stroke(
                &source,
                PencilStyleProfile {
                    size,
                    point_width_override: None,
                },
            );
            if name == "nomove" {
                assert!(sampled.is_err());
                assert_eq!(native.len(), 16);
                continue;
            }
            let records = sampled.unwrap().records;
            let packed: Vec<u8> = records
                .iter()
                .flatten()
                .flat_map(|f| f.to_le_bytes())
                .collect();
            assert_eq!(packed, native, "{name} VBO");
            let frame = render_records_on(
                &records,
                Profile {
                    vertex: VertexArithmetic::Separate32,
                    plane: VaryingPlane::SourceSideSelection,
                    unorm: UnormRound::NearestEven,
                },
                &atlas,
                pre.clone(),
                [221, 68, 17, 128],
            )
            .unwrap();
            let expected = decode(post);
            assert_eq!(frame.len(), WIDTH * HEIGHT * 4);
            assert_eq!(expected.len(), frame.len());
            let diff: Vec<_> = frame
                .as_chunks::<4>()
                .0
                .iter()
                .zip(expected.as_chunks::<4>().0.iter())
                .enumerate()
                .filter(|(_, (a, b))| a != b)
                .collect();
            assert_eq!(diff.len(), expected_diff, "{name} full-frame pixel count");
            if expected_diff == 1 {
                assert_eq!(
                    diff[0],
                    (979 * 1920 + 124, (&[159, 80, 67, 255], &[179, 76, 51, 255]))
                );
            }
        }
    }
}

#[cfg(test)]
mod serialized_size8_tests {
    use super::super::{
        PencilStyleProfile, SavedPoint, SavedStroke, sample_saved_stroke_equal_threshold,
    };
    use super::*;

    fn decode(bytes: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        ZlibDecoder::new(bytes).read_to_end(&mut out).unwrap();
        out
    }

    #[test]
    fn parsed_size8_strokes_match_native_vbo_and_host_frames() {
        type Case<'a> = (bool, &'a [u8], &'a [u8], &'a [u8]);
        let cases: [Case<'_>; 2] = [
            (
                false,
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/serialized8.vbo"
                ),
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/serialized8-post.rgba.zlib"
                ),
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/serialized-pencil-channels.bin"
                ),
            ),
            (
                true,
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/serialized8-reflect.vbo"
                ),
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/serialized8-reflect-1.rgba.zlib"
                ),
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/serialized8-reflect.channels"
                ),
            ),
        ];
        let pre = decode(include_bytes!(
            "../../../../../conformance/old-pencil-color/varied-pre.rgba.zlib"
        ));
        let atlas = decode_atlas().unwrap();
        for (reflected, native, post, native_channels) in cases {
            let mut payload=include_bytes!("../../../../../conformance/old-pencil-counterexample/serialized-pencil-color.payload").to_vec();
            for offset in [2631, 2639] {
                assert_eq!(&payload[offset..offset + 4], &9.1168_f32.to_le_bytes());
                payload[offset..offset + 4].copy_from_slice(&8.0_f32.to_le_bytes());
            }
            if reflected {
                for i in 0..203 {
                    payload[173 + i * 4 + 1] ^= 0x80;
                }
            }
            let object = crate::StoredObject {
                object_type: crate::ObjectType::Stroke,
                declared_size: (payload.len() + 32) as u32,
                payload_offset: 0,
                payload_size: payload.len(),
                integrity_trailer: [0; 32],
                children: vec![],
            };
            let stroke = object
                .decode_stroke(&payload, &crate::ParseLimits::default())
                .unwrap();
            let rendering = stroke.rendering.as_ref().unwrap();
            assert_eq!(stroke.pen_width, 8.0);
            assert_eq!(rendering.style.color_argb, Some(0x8024a0d8));
            assert_eq!(rendering.tool_type_raw, 2);
            assert!(rendering.properties.compressed);
            let mut channels = Vec::new();
            for p in &stroke.points {
                for v in [p.x, p.y] {
                    channels.extend_from_slice(&(v as f32).to_le_bytes());
                }
            }
            for v in &stroke.pressures {
                channels.extend_from_slice(&(*v as f32).to_le_bytes());
            }
            for v in &stroke.timestamps {
                channels.extend_from_slice(&(*v as i32).to_le_bytes());
            }
            for v in stroke.tilts.iter().chain(&stroke.orientations) {
                channels.extend_from_slice(&(*v as f32).to_le_bytes());
            }
            assert_eq!(channels, native_channels);
            let points: Vec<_> = stroke
                .points
                .iter()
                .map(|p| SavedPoint {
                    x: p.x as f32,
                    y: p.y as f32,
                })
                .collect();
            let pressures: Vec<_> = stroke.pressures.iter().map(|v| *v as f32).collect();
            // Name-table OnAttach and stroke-type are supplied by the separate native capture.
            let source = SavedStroke {
                pen_name: "com.samsung.android.sdk.pen.pen.preload.Pencil",
                tool_type: 2,
                stroke_type: 0,
                curve_enabled: true,
                points: &points,
                pressures: &pressures,
                size: 8.0,
            };
            let records = sample_saved_stroke_equal_threshold(
                &source,
                PencilStyleProfile {
                    size: 8.0,
                    point_width_override: None,
                },
            )
            .unwrap()
            .records;
            let packed: Vec<u8> = records
                .iter()
                .flatten()
                .flat_map(|v| v.to_le_bytes())
                .collect();
            assert_eq!(packed, native);
            let profile = Profile {
                vertex: VertexArithmetic::Separate32,
                plane: VaryingPlane::SourceSideSelection,
                unorm: UnormRound::NearestEven,
            };
            let actual =
                render_records_on(&records, profile, &atlas, pre.clone(), [36, 160, 216, 128])
                    .unwrap();
            assert_eq!(actual, decode(post));
            if reflected {
                let mut projection = PROJECTION;
                projection[0] *= 2.0;
                projection[5] *= 2.0;
                let actual = render_records_on_with_projection(
                    &records,
                    profile,
                    &atlas,
                    pre.clone(),
                    [36, 160, 216, 128],
                    &projection,
                )
                .unwrap();
                assert_eq!(
                    actual,
                    decode(include_bytes!(
                        "../../../../../conformance/old-pencil-counterexample/serialized8-reflect-2.rgba.zlib"
                    ))
                );
            }
        }
    }
}

#[cfg(test)]
mod serialized_size5_tests {
    use super::super::{
        PencilStyleProfile, SavedPoint, SavedStroke, sample_saved_stroke_equal_threshold,
    };
    use super::*;

    fn decode(bytes: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        ZlibDecoder::new(bytes).read_to_end(&mut out).unwrap();
        out
    }

    #[test]
    fn parsed_size5_strokes_match_native_vbo_and_host_frames() {
        type Case<'a> = (bool, &'a [u8], &'a [u8], &'a [u8]);
        let cases: [Case<'_>; 2] = [
            (
                false,
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/serialized5-plain.vbo"
                ),
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/serialized5-plain-1.z"
                ),
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/serialized-pencil-channels.bin"
                ),
            ),
            (
                true,
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/serialized5-reflect.vbo"
                ),
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/serialized5-reflect-1.z"
                ),
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/serialized8-reflect.channels"
                ),
            ),
        ];

        let atlas = decode_atlas().unwrap();
        for (reflected, native, post, native_channels) in cases {
            let pre = decode(if reflected {
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/serialized5-reflect-pre.z"
                )
            } else {
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/serialized5-plain-pre.z"
                )
            });
            let color = if reflected {
                [241, 48, 160, 64]
            } else {
                [36, 160, 216, 128]
            };
            let argb = if reflected {
                0x40f130a0_u32
            } else {
                0x8024a0d8_u32
            };
            let mut payload=include_bytes!("../../../../../conformance/old-pencil-counterexample/serialized-pencil-color.payload").to_vec();
            for offset in [2631, 2639] {
                assert_eq!(&payload[offset..offset + 4], &9.1168_f32.to_le_bytes());
                payload[offset..offset + 4].copy_from_slice(&5.0_f32.to_le_bytes());
            }
            payload[2627..2631].copy_from_slice(&argb.to_le_bytes());
            if reflected {
                for i in 0..203 {
                    payload[173 + i * 4 + 1] ^= 0x80;
                }
            }
            let object = crate::StoredObject {
                object_type: crate::ObjectType::Stroke,
                declared_size: (payload.len() + 32) as u32,
                payload_offset: 0,
                payload_size: payload.len(),
                integrity_trailer: [0; 32],
                children: vec![],
            };
            let stroke = object
                .decode_stroke(&payload, &crate::ParseLimits::default())
                .unwrap();
            let rendering = stroke.rendering.as_ref().unwrap();
            assert_eq!(stroke.pen_width, 5.0);
            assert_eq!(rendering.style.color_argb, Some(argb));
            assert_eq!(rendering.tool_type_raw, 2);
            assert!(rendering.properties.compressed);
            let mut channels = Vec::new();
            for p in &stroke.points {
                for v in [p.x, p.y] {
                    channels.extend_from_slice(&(v as f32).to_le_bytes());
                }
            }
            for v in &stroke.pressures {
                channels.extend_from_slice(&(*v as f32).to_le_bytes());
            }
            for v in &stroke.timestamps {
                channels.extend_from_slice(&(*v as i32).to_le_bytes());
            }
            for v in stroke.tilts.iter().chain(&stroke.orientations) {
                channels.extend_from_slice(&(*v as f32).to_le_bytes());
            }
            assert_eq!(channels, native_channels);
            let points: Vec<_> = stroke
                .points
                .iter()
                .map(|p| SavedPoint {
                    x: p.x as f32,
                    y: p.y as f32,
                })
                .collect();
            let pressures: Vec<_> = stroke.pressures.iter().map(|v| *v as f32).collect();
            // Name-table OnAttach and stroke-type are supplied by the separate native capture.
            let source = SavedStroke {
                pen_name: "com.samsung.android.sdk.pen.pen.preload.Pencil",
                tool_type: 2,
                stroke_type: 0,
                curve_enabled: true,
                points: &points,
                pressures: &pressures,
                size: 5.0,
            };
            let records = sample_saved_stroke_equal_threshold(
                &source,
                PencilStyleProfile {
                    size: 5.0,
                    point_width_override: None,
                },
            )
            .unwrap()
            .records;
            let packed: Vec<u8> = records
                .iter()
                .flatten()
                .flat_map(|v| v.to_le_bytes())
                .collect();
            assert_eq!(packed, native);
            let profile = Profile {
                vertex: VertexArithmetic::Separate32,
                plane: VaryingPlane::SourceSideSelection,
                unorm: UnormRound::NearestEven,
            };
            let actual = render_records_on(&records, profile, &atlas, pre.clone(), color).unwrap();
            assert_eq!(actual, decode(post));
            {
                let mut projection = PROJECTION;
                projection[0] *= 2.0;
                projection[5] *= 2.0;
                let actual = render_records_on_with_projection(
                    &records,
                    profile,
                    &atlas,
                    pre.clone(),
                    color,
                    &projection,
                )
                .unwrap();
                assert_eq!(
                    actual,
                    decode(if reflected {
                        include_bytes!(
                            "../../../../../conformance/old-pencil-counterexample/serialized5-reflect-2.z"
                        )
                    } else {
                        include_bytes!(
                            "../../../../../conformance/old-pencil-counterexample/serialized5-plain-2.z"
                        )
                    })
                );
            }
        }
    }
}

#[cfg(test)]
mod serialized_interval_tests {
    use super::super::{
        PencilStyleProfile, SavedPoint, SavedStroke, sample_saved_stroke_equal_threshold,
    };
    use super::*;
    fn decode(bytes: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        ZlibDecoder::new(bytes).read_to_end(&mut out).unwrap();
        out
    }
    #[test]
    fn serialized_zero_alpha_matches_native_prior_frame() {
        let mut payload = include_bytes!(
            "../../../../../conformance/old-pencil-counterexample/serialized-pencil-color.payload"
        )
        .to_vec();
        payload[2627..2631].copy_from_slice(&0x0024a0d8_u32.to_le_bytes());
        let object = crate::StoredObject {
            object_type: crate::ObjectType::Stroke,
            declared_size: (payload.len() + 32) as u32,
            payload_offset: 0,
            payload_size: payload.len(),
            integrity_trailer: [0; 32],
            children: vec![],
        };
        let stroke = object
            .decode_stroke(&payload, &crate::ParseLimits::default())
            .unwrap();
        assert_eq!(
            stroke.rendering.as_ref().unwrap().style.color_argb,
            Some(0x0024a0d8)
        );
        let points: Vec<_> = stroke
            .points
            .iter()
            .map(|p| SavedPoint {
                x: p.x as f32,
                y: p.y as f32,
            })
            .collect();
        let pressures: Vec<_> = stroke.pressures.iter().map(|v| *v as f32).collect();
        let size = stroke.pen_width;
        let source = SavedStroke {
            pen_name: "com.samsung.android.sdk.pen.pen.preload.Pencil",
            tool_type: 2,
            stroke_type: 0,
            curve_enabled: true,
            points: &points,
            pressures: &pressures,
            size,
        };
        let records = sample_saved_stroke_equal_threshold(
            &source,
            PencilStyleProfile {
                size,
                point_width_override: None,
            },
        )
        .unwrap()
        .records;
        let packed: Vec<u8> = records
            .iter()
            .flatten()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        assert_eq!(
            packed,
            include_bytes!(
                "../../../../../conformance/old-pencil-counterexample/serialized-pencil.vbo"
            )
        );
        let pre = decode(include_bytes!(
            "../../../../../conformance/old-pencil-color/varied-pre.rgba.zlib"
        ));
        let profile = Profile {
            vertex: VertexArithmetic::Separate32,
            plane: VaryingPlane::SourceSideSelection,
            unorm: UnormRound::NearestEven,
        };
        let actual = render_records_on(
            &records,
            profile,
            &decode_atlas().unwrap(),
            pre.clone(),
            [36, 160, 216, 0],
        )
        .unwrap();
        assert_eq!(actual, pre);
    }

    #[test]
    fn native_interval_source_and_host_frames() {
        type Case<'a> = (f32, bool, bool, &'a [u8], &'a [u8], &'a [u8]);
        let cases: [Case<'_>; 7] = [
            (
                3.0,
                false,
                false,
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/interval-3_plain.vbo"
                ),
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/interval-3_plain-1.z"
                ),
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/interval-3_plain-2.z"
                ),
            ),
            (
                3.0,
                true,
                false,
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/interval-3_reflect.vbo"
                ),
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/interval-3_reflect-1.z"
                ),
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/interval-3_reflect-2.z"
                ),
            ),
            (
                12.0,
                false,
                false,
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/interval-12_plain.vbo"
                ),
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/interval-12_plain-1.z"
                ),
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/interval-12_plain-2.z"
                ),
            ),
            (
                12.0,
                true,
                false,
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/interval-12_reflect.vbo"
                ),
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/interval-12_reflect-1.z"
                ),
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/interval-12_reflect-2.z"
                ),
            ),
            (
                4.25,
                false,
                false,
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/interval-4_25_plain.vbo"
                ),
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/interval-4_25_plain-1.z"
                ),
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/interval-4_25_plain-2.z"
                ),
            ),
            (
                7.25,
                false,
                true,
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/interval-7_25_flat.vbo"
                ),
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/interval-7_25_flat-1.z"
                ),
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/interval-7_25_flat-2.z"
                ),
            ),
            (
                4.25,
                true,
                true,
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/stretched.vbo"
                ),
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/stretched-1.z"
                ),
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/stretched-2.z"
                ),
            ),
        ];
        for (size, reflected, flat, native, post1, post2) in cases {
            let mut payload=include_bytes!("../../../../../conformance/old-pencil-counterexample/serialized-pencil-color.payload").to_vec();
            for off in [2631, 2639] {
                assert_eq!(&payload[off..off + 4], &9.1168_f32.to_le_bytes());
                payload[off..off + 4].copy_from_slice(&size.to_le_bytes());
            }
            if reflected {
                payload[2627..2631].copy_from_slice(&0x40f130a0_u32.to_le_bytes());
                for i in 0..203 {
                    let off = 173 + i * 4;
                    if flat {
                        let value = u16::from_le_bytes([payload[off], payload[off + 1]]);
                        payload[off..off + 2].copy_from_slice(
                            &((value & 0x8000) | ((value & 0x7fff) * 2)).to_le_bytes(),
                        );
                    } else {
                        payload[off + 1] ^= 0x80;
                    }
                }
            }
            if flat {
                payload[985..989].copy_from_slice(&0.5_f32.to_le_bytes());
                payload[989..1395].fill(0);
            }
            let obj = crate::StoredObject {
                object_type: crate::ObjectType::Stroke,
                declared_size: (payload.len() + 32) as u32,
                payload_offset: 0,
                payload_size: payload.len(),
                integrity_trailer: [0; 32],
                children: vec![],
            };
            let stroke = obj
                .decode_stroke(&payload, &crate::ParseLimits::default())
                .unwrap();
            let rendering = stroke.rendering.as_ref().unwrap();
            let argb = if reflected { 0x40f130a0 } else { 0x8024a0d8 };
            assert_eq!(rendering.style.color_argb, Some(argb));
            assert_eq!(stroke.pen_width, size);
            let mut channels = Vec::new();
            for p in &stroke.points {
                for v in [p.x, p.y] {
                    channels.extend_from_slice(&(v as f32).to_le_bytes());
                }
            }
            for v in &stroke.pressures {
                channels.extend_from_slice(&(*v as f32).to_le_bytes());
            }
            for v in &stroke.timestamps {
                channels.extend_from_slice(&(*v as i32).to_le_bytes());
            }
            for v in stroke.tilts.iter().chain(&stroke.orientations) {
                channels.extend_from_slice(&(*v as f32).to_le_bytes());
            }
            let expected_channels: &[u8] = if flat && reflected {
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/stretched.channels"
                )
            } else if flat {
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/interval-7_25_flat.channels"
                )
            } else if reflected {
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/serialized8-reflect.channels"
                )
            } else {
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/serialized-pencil-channels.bin"
                )
            };
            assert_eq!(channels, expected_channels);
            let points: Vec<_> = stroke
                .points
                .iter()
                .map(|p| SavedPoint {
                    x: p.x as f32,
                    y: p.y as f32,
                })
                .collect();
            let pressures: Vec<_> = stroke.pressures.iter().map(|v| *v as f32).collect();
            let source = SavedStroke {
                pen_name: "com.samsung.android.sdk.pen.pen.preload.Pencil",
                tool_type: 2,
                stroke_type: 0,
                curve_enabled: true,
                points: &points,
                pressures: &pressures,
                size,
            };
            let records = sample_saved_stroke_equal_threshold(
                &source,
                PencilStyleProfile {
                    size,
                    point_width_override: None,
                },
            )
            .unwrap()
            .records;
            let packed: Vec<u8> = records
                .iter()
                .flatten()
                .flat_map(|v| v.to_le_bytes())
                .collect();
            assert_eq!(packed, native);
            let pre = decode(if reflected {
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/serialized5-reflect-pre.z"
                )
            } else {
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/serialized5-plain-pre.z"
                )
            });
            let color = if reflected {
                [241, 48, 160, 64]
            } else {
                [36, 160, 216, 128]
            };
            let profile = Profile {
                vertex: VertexArithmetic::Separate32,
                plane: VaryingPlane::SourceSideSelection,
                unorm: UnormRound::NearestEven,
            };
            let atlas = decode_atlas().unwrap();
            let a = render_records_on(&records, profile, &atlas, pre.clone(), color).unwrap();
            assert_eq!(a, decode(post1));
            let mut projection = PROJECTION;
            projection[0] *= 2.0;
            projection[5] *= 2.0;
            let b = render_records_on_with_projection(
                &records,
                profile,
                &atlas,
                pre,
                color,
                &projection,
            )
            .unwrap();
            assert_eq!(b, decode(post2));
            if reflected && flat {
                let transparent = decode(include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/stretched-transparent-pre.z"
                ));
                let a = render_records_on(&records, profile, &atlas, transparent.clone(), color)
                    .unwrap();
                assert_eq!(
                    a,
                    decode(include_bytes!(
                        "../../../../../conformance/old-pencil-counterexample/stretched-transparent-1.z"
                    ))
                );
                let b = render_records_on_with_projection(
                    &records,
                    profile,
                    &atlas,
                    transparent,
                    color,
                    &projection,
                )
                .unwrap();
                assert_eq!(
                    b,
                    decode(include_bytes!(
                        "../../../../../conformance/old-pencil-counterexample/stretched-transparent-2.z"
                    ))
                );
            }
        }
    }
}

#[test]
fn synthetic_transparent_backdrops_match_full_native_frames() {
    use std::io::Read;
    let cases: [(&[u8], &[u8], f32); 4] = [
        (
            include_bytes!(
                "../../../../../conformance/old-pencil-counterexample/transparent-clear-pre.z"
            ),
            include_bytes!(
                "../../../../../conformance/old-pencil-counterexample/transparent-clear1-post.z"
            ),
            1.0,
        ),
        (
            include_bytes!(
                "../../../../../conformance/old-pencil-counterexample/transparent-clear-pre.z"
            ),
            include_bytes!(
                "../../../../../conformance/old-pencil-counterexample/transparent-clear2-post.z"
            ),
            2.0,
        ),
        (
            include_bytes!(
                "../../../../../conformance/old-pencil-counterexample/transparent-translucent1-pre.z"
            ),
            include_bytes!(
                "../../../../../conformance/old-pencil-counterexample/transparent-translucent1-post.z"
            ),
            1.0,
        ),
        (
            include_bytes!(
                "../../../../../conformance/old-pencil-counterexample/transparent-translucent2-pre.z"
            ),
            include_bytes!(
                "../../../../../conformance/old-pencil-counterexample/transparent-translucent2-post.z"
            ),
            2.0,
        ),
    ];
    let decode = |bytes: &[u8]| {
        let mut output = Vec::new();
        flate2::read::ZlibDecoder::new(bytes)
            .read_to_end(&mut output)
            .unwrap();
        output
    };
    let records: Vec<[f32; 4]> = include_bytes!(
        "../../../../../conformance/old-pencil-counterexample/serialized-pencil.vbo"
    )
    .as_chunks::<16>()
    .0
    .iter()
    .map(|chunk| std::array::from_fn(|i| read_f32_le(&chunk[i * 4..i * 4 + 4])))
    .collect();
    let atlas = decode_atlas().unwrap();
    for (case, (prior, expected, scale)) in cases.into_iter().enumerate() {
        let prior = decode(prior);
        let expected = decode(expected);
        assert_eq!(prior.len(), WIDTH * HEIGHT * 4);
        assert_eq!(expected.len(), WIDTH * HEIGHT * 4);
        if case < 2 {
            let (x, page_y, pixel) = if scale == 1.0 {
                (130, 225, [2, 7, 10, 12])
            } else {
                (264, 415, [3, 15, 21, 23])
            };
            let at = ((HEIGHT - 1 - page_y) * WIDTH + x) * 4;
            assert_eq!(&expected[at..at + 4], &pixel);
        }
        let mut projection = PROJECTION;
        projection[0] *= scale;
        projection[5] *= scale;
        let actual = render_records_on_with_projection(
            &records,
            Profile {
                vertex: VertexArithmetic::Separate32,
                plane: VaryingPlane::SourceSideSelection,
                unorm: UnormRound::NearestEven,
            },
            &atlas,
            prior,
            [36, 160, 216, 128],
            &projection,
        )
        .unwrap();
        assert_eq!(actual.len(), expected.len());
        assert_eq!(actual, expected, "scale {scale}");
    }
}

#[cfg(test)]
mod serialized_dot_tests {
    use super::super::{
        PencilStyleProfile, SavedPoint, SavedStroke, sample_saved_stroke_equal_threshold,
    };
    use super::*;
    fn decode(bytes: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        ZlibDecoder::new(bytes).read_to_end(&mut out).unwrap();
        out
    }
    #[test]
    fn serialized_dot_and_threshold_frames_match_native() {
        type Case<'a> = (
            &'a str,
            f32,
            u32,
            Option<f32>,
            u16,
            bool,
            &'a [u8],
            &'a [u8],
            &'a [u8],
        );
        let cases: [Case<'_>; 7] = [
            (
                "425",
                4.25,
                0x8024a0d8,
                None,
                0,
                false,
                include_bytes!("../../../../../conformance/old-pencil-counterexample/dot-425.vbo"),
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/dot-425.channels"
                ),
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/dot-425-post.z"
                ),
            ),
            (
                "8",
                8.0,
                0x40f130a0,
                Some(0.5),
                2,
                false,
                include_bytes!("../../../../../conformance/old-pencil-counterexample/dot-8.vbo"),
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/dot-8.channels"
                ),
                include_bytes!("../../../../../conformance/old-pencil-counterexample/dot-8-post.z"),
            ),
            (
                "3_full",
                3.0,
                0xff24a0d8,
                Some(1.0),
                2,
                false,
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/dot-3_full.vbo"
                ),
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/dot-3_full.channels"
                ),
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/dot-3_full-post.z"
                ),
            ),
            (
                "12_zero",
                12.0,
                0x0024a0d8,
                Some(0.0),
                2,
                false,
                include_bytes!("../../../../../conformance/old-pencil-counterexample/dot-8.vbo"),
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/dot-12_zero.channels"
                ),
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/serialized5-plain-pre.z"
                ),
            ),
            (
                "below",
                8.125,
                0x40f130a0,
                Some(0.5),
                51,
                true,
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/dot-threshold_below.vbo"
                ),
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/dot-threshold_below.channels"
                ),
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/dot-threshold_below-post.z"
                ),
            ),
            (
                "exact",
                8.125,
                0x40f130a0,
                Some(0.5),
                52,
                true,
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/dot-threshold_exact.vbo"
                ),
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/dot-threshold_exact.channels"
                ),
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/dot-threshold_exact-post.z"
                ),
            ),
            (
                "two",
                8.0,
                0x40f130a0,
                Some(0.5),
                2,
                false,
                include_bytes!("../../../../../conformance/old-pencil-counterexample/dot-8.vbo"),
                include_bytes!(
                    "../../../../../conformance/old-pencil-counterexample/dot-two.channels"
                ),
                include_bytes!("../../../../../conformance/old-pencil-counterexample/dot-8-post.z"),
            ),
        ];
        let pre = decode(include_bytes!(
            "../../../../../conformance/old-pencil-counterexample/serialized5-plain-pre.z"
        ));
        let atlas = decode_atlas().unwrap();
        for (name, size, argb, pressure, delta, origin100, native, expected_channels, post) in cases
        {
            let mut payload=include_bytes!("../../../../../conformance/old-pencil-counterexample/serialized-pencil-color.payload").to_vec();
            for off in [2631, 2639] {
                payload[off..off + 4].copy_from_slice(&size.to_le_bytes());
            }
            payload[2627..2631].copy_from_slice(&argb.to_le_bytes());
            payload[173..985].fill(0);
            payload[173..175].copy_from_slice(&delta.to_le_bytes());
            if origin100 {
                payload[157..165].copy_from_slice(&100.0_f64.to_le_bytes());
                payload[165..173].copy_from_slice(&100.0_f64.to_le_bytes());
            }
            if let Some(p) = pressure {
                payload[985..989].copy_from_slice(&p.to_le_bytes());
                payload[989..1395].fill(0);
            }
            if name == "two" {
                let frame = payload[137..].to_vec();
                let mut short = frame[..18].to_vec();
                short.extend_from_slice(&2u16.to_le_bytes());
                for part in [
                    &frame[20..40],
                    &frame[848..854],
                    &frame[1258..1264],
                    &frame[1668..1674],
                    &frame[2078..2084],
                    &frame[2488..2490],
                    &frame[2490..],
                ] {
                    short.extend_from_slice(part);
                }
                short[6..10].copy_from_slice(&66u32.to_le_bytes());
                let length = short.len() as u32;
                short[..4].copy_from_slice(&length.to_le_bytes());
                payload.truncate(137);
                payload.extend_from_slice(&short);
            }
            let object = crate::StoredObject {
                object_type: crate::ObjectType::Stroke,
                declared_size: (payload.len() + 32) as u32,
                payload_offset: 0,
                payload_size: payload.len(),
                integrity_trailer: [0; 32],
                children: vec![],
            };
            let stroke = object
                .decode_stroke(&payload, &crate::ParseLimits::default())
                .unwrap();
            if name == "two" {
                assert_eq!(stroke.points.len(), 2);
            }
            let rendering = stroke.rendering.as_ref().unwrap();
            assert_eq!(rendering.tool_type_raw, 2);
            assert_eq!(rendering.style.color_argb, Some(argb));
            assert_eq!(stroke.pen_width, size);
            let mut channels = Vec::new();
            for p in &stroke.points {
                for v in [p.x, p.y] {
                    channels.extend_from_slice(&(v as f32).to_le_bytes());
                }
            }
            for v in &stroke.pressures {
                channels.extend_from_slice(&(*v as f32).to_le_bytes());
            }
            for v in &stroke.timestamps {
                channels.extend_from_slice(&(*v as i32).to_le_bytes());
            }
            for v in stroke.tilts.iter().chain(&stroke.orientations) {
                channels.extend_from_slice(&(*v as f32).to_le_bytes());
            }
            assert_eq!(channels, expected_channels, "{name} channels");
            let points: Vec<_> = stroke
                .points
                .iter()
                .map(|p| SavedPoint {
                    x: p.x as f32,
                    y: p.y as f32,
                })
                .collect();
            let pressures: Vec<_> = stroke.pressures.iter().map(|v| *v as f32).collect();
            let source = SavedStroke {
                pen_name: "com.samsung.android.sdk.pen.pen.preload.Pencil",
                tool_type: 2,
                stroke_type: 0,
                curve_enabled: true,
                points: &points,
                pressures: &pressures,
                size,
            };
            let style = PencilStyleProfile {
                size,
                point_width_override: None,
            };
            let sample = sample_saved_stroke_equal_threshold(&source, style).unwrap();
            let packed: Vec<u8> = sample
                .records
                .iter()
                .flatten()
                .flat_map(|v| v.to_le_bytes())
                .collect();
            assert_eq!(packed, native, "{name} vbo");
            assert_eq!(sample.records.len(), if name == "exact" { 2 } else { 1 });
            let profile = Profile {
                vertex: VertexArithmetic::Separate32,
                plane: VaryingPlane::SourceSideSelection,
                unorm: UnormRound::NearestEven,
            };
            let color = [
                (argb >> 16) as u8,
                (argb >> 8) as u8,
                argb as u8,
                (argb >> 24) as u8,
            ];
            let actual =
                render_records_on(&sample.records, profile, &atlas, pre.clone(), color).unwrap();
            assert_eq!(actual, decode(post), "{name} pixels");
            if name == "8" {
                let mut projection = PROJECTION;
                projection[0] *= 2.0;
                projection[5] *= 2.0;
                let actual = render_records_on_with_projection(
                    &sample.records,
                    profile,
                    &atlas,
                    pre.clone(),
                    color,
                    &projection,
                )
                .unwrap();
                assert_eq!(
                    actual,
                    decode(include_bytes!(
                        "../../../../../conformance/old-pencil-counterexample/dot-8-scale2.z"
                    ))
                );
            }
        }
    }
}

#[cfg(test)]
mod rapid_bend_tests {
    use super::super::{
        PencilStyleProfile, SavedPoint, SavedStroke, sample_saved_stroke_equal_threshold,
    };
    use super::*;
    fn decode(bytes: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        ZlibDecoder::new(bytes).read_to_end(&mut out).unwrap();
        out
    }
    #[test]
    fn serialized_rapid_bend_matches_native_vbo_and_frames() {
        let mut payload = include_bytes!(
            "../../../../../conformance/old-pencil-counterexample/serialized-pencil-color.payload"
        )
        .to_vec();
        for off in [2631, 2639] {
            payload[off..off + 4].copy_from_slice(&12.0_f32.to_le_bytes());
        }
        payload[2627..2631].copy_from_slice(&0xc0ee8811_u32.to_le_bytes());
        for i in 0..203 {
            let off = 173 + 4 * i;
            let value = u16::from_le_bytes([payload[off], payload[off + 1]]);
            payload[off..off + 2]
                .copy_from_slice(&((value & 0x8000) | ((value & 0x7fff) * 2)).to_le_bytes());
            if i >= 100 {
                payload[off + 3] ^= 0x80;
            }
        }
        payload[985..989].copy_from_slice(&0.125_f32.to_le_bytes());
        for i in 0..203 {
            let value: u16 = if i % 2 == 0 { 3072 } else { 0x8c00 };
            payload[989 + 2 * i..991 + 2 * i].copy_from_slice(&value.to_le_bytes());
        }
        let object = crate::StoredObject {
            object_type: crate::ObjectType::Stroke,
            declared_size: (payload.len() + 32) as u32,
            payload_offset: 0,
            payload_size: payload.len(),
            integrity_trailer: [0; 32],
            children: vec![],
        };
        let stroke = object
            .decode_stroke(&payload, &crate::ParseLimits::default())
            .unwrap();
        let r = stroke.rendering.as_ref().unwrap();
        assert_eq!(r.style.color_argb, Some(0xc0ee8811));
        assert_eq!(r.tool_type_raw, 2);
        assert_eq!(stroke.pen_width, 12.0);
        let mut channels = Vec::new();
        for p in &stroke.points {
            for v in [p.x, p.y] {
                channels.extend_from_slice(&(v as f32).to_le_bytes());
            }
        }
        for v in &stroke.pressures {
            channels.extend_from_slice(&(*v as f32).to_le_bytes());
        }
        for v in &stroke.timestamps {
            channels.extend_from_slice(&(*v as i32).to_le_bytes());
        }
        for v in stroke.tilts.iter().chain(&stroke.orientations) {
            channels.extend_from_slice(&(*v as f32).to_le_bytes());
        }
        assert_eq!(
            channels,
            include_bytes!(
                "../../../../../conformance/old-pencil-counterexample/rapid-bend.channels"
            )
        );
        let points: Vec<_> = stroke
            .points
            .iter()
            .map(|p| SavedPoint {
                x: p.x as f32,
                y: p.y as f32,
            })
            .collect();
        let pressures: Vec<_> = stroke.pressures.iter().map(|v| *v as f32).collect();
        let source = SavedStroke {
            pen_name: "com.samsung.android.sdk.pen.pen.preload.Pencil",
            tool_type: 2,
            stroke_type: 0,
            curve_enabled: true,
            points: &points,
            pressures: &pressures,
            size: 12.0,
        };
        let records = sample_saved_stroke_equal_threshold(
            &source,
            PencilStyleProfile {
                size: 12.0,
                point_width_override: None,
            },
        )
        .unwrap()
        .records;
        let packed: Vec<u8> = records
            .iter()
            .flatten()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        assert_eq!(
            packed,
            include_bytes!("../../../../../conformance/old-pencil-counterexample/rapid-bend.vbo")
        );
        let pre = decode(include_bytes!(
            "../../../../../conformance/old-pencil-counterexample/serialized5-plain-pre.z"
        ));
        let atlas = decode_atlas().unwrap();
        let profile = Profile {
            vertex: VertexArithmetic::Separate32,
            plane: VaryingPlane::SourceSideSelection,
            unorm: UnormRound::NearestEven,
        };
        let color = [238, 136, 17, 192];
        let actual = render_records_on(&records, profile, &atlas, pre.clone(), color).unwrap();
        assert_eq!(
            actual,
            decode(include_bytes!(
                "../../../../../conformance/old-pencil-counterexample/rapid-bend-1.z"
            ))
        );
        let mut projection = PROJECTION;
        projection[0] *= 2.0;
        projection[5] *= 2.0;
        let actual =
            render_records_on_with_projection(&records, profile, &atlas, pre, color, &projection)
                .unwrap();
        assert_eq!(
            actual,
            decode(include_bytes!(
                "../../../../../conformance/old-pencil-counterexample/rapid-bend-2.z"
            ))
        );
    }
}

#[cfg(test)]
mod resolved_synthetic_tests {
    use super::super::{
        PencilStyleProfile, SavedPoint, SavedStroke, sample_saved_stroke_equal_threshold,
    };
    use super::*;
    fn decode(bytes: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        ZlibDecoder::new(bytes).read_to_end(&mut out).unwrap();
        out
    }
    #[test]
    fn archive_resolved_name_to_native_host_frames() {
        let doc = crate::parse_bytes(include_bytes!(
            "../../../../../conformance/old-pencil-counterexample/resolved-synthetic.sdocx"
        ))
        .unwrap();
        let strokes: Vec<_> = doc
            .pages
            .iter()
            .flat_map(|p| p.strokes())
            .filter(|s| {
                s.rendering.as_ref().is_some_and(|r| {
                    r.pen_name.as_deref() == Some("com.samsung.android.sdk.pen.pen.preload.Pencil")
                })
            })
            .collect();
        assert_eq!(strokes.len(), 1);
        let stroke = strokes[0];
        let rendering = stroke.rendering.as_ref().unwrap();
        assert_eq!(rendering.style.pen_name_id, Some(2));
        assert_eq!(rendering.style.color_argb, Some(0x8024a0d8));
        assert_eq!(rendering.tool_type_raw, 2);
        assert_eq!(stroke.points.len(), 204);
        let mut channels = Vec::new();
        for p in &stroke.points {
            for v in [p.x, p.y] {
                channels.extend_from_slice(&(v as f32).to_le_bytes());
            }
        }
        for v in &stroke.pressures {
            channels.extend_from_slice(&(*v as f32).to_le_bytes());
        }
        for v in &stroke.timestamps {
            channels.extend_from_slice(&(*v as i32).to_le_bytes());
        }
        for v in stroke.tilts.iter().chain(&stroke.orientations) {
            channels.extend_from_slice(&(*v as f32).to_le_bytes());
        }
        assert_eq!(
            channels,
            include_bytes!(
                "../../../../../conformance/old-pencil-counterexample/serialized-pencil-channels.bin"
            )
        );
        let points: Vec<_> = stroke
            .points
            .iter()
            .map(|p| SavedPoint {
                x: p.x as f32,
                y: p.y as f32,
            })
            .collect();
        let pressures: Vec<_> = stroke.pressures.iter().map(|v| *v as f32).collect();
        let size = stroke.pen_width;
        let source = SavedStroke {
            pen_name: rendering.pen_name.as_deref().unwrap(),
            tool_type: 2,
            stroke_type: 0,
            curve_enabled: true,
            points: &points,
            pressures: &pressures,
            size,
        };
        let records = sample_saved_stroke_equal_threshold(
            &source,
            PencilStyleProfile {
                size,
                point_width_override: None,
            },
        )
        .unwrap()
        .records;
        let packed: Vec<u8> = records
            .iter()
            .flatten()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        assert_eq!(
            packed,
            include_bytes!(
                "../../../../../conformance/old-pencil-counterexample/serialized-pencil.vbo"
            )
        );
        let pre = decode(include_bytes!(
            "../../../../../conformance/old-pencil-color/varied-pre.rgba.zlib"
        ));
        let atlas = decode_atlas().unwrap();
        let profile = Profile {
            vertex: VertexArithmetic::Separate32,
            plane: VaryingPlane::SourceSideSelection,
            unorm: UnormRound::NearestEven,
        };
        let actual =
            render_records_on(&records, profile, &atlas, pre.clone(), [36, 160, 216, 128]).unwrap();
        assert_eq!(
            actual,
            decode(include_bytes!(
                "../../../../../conformance/old-pencil-counterexample/serialized-pencil-color-post.rgba.zlib"
            ))
        );
        let mut projection = PROJECTION;
        projection[0] *= 2.0;
        projection[5] *= 2.0;
        let actual = render_records_on_with_projection(
            &records,
            profile,
            &atlas,
            pre,
            [36, 160, 216, 128],
            &projection,
        )
        .unwrap();
        assert_eq!(
            actual,
            decode(include_bytes!(
                "../../../../../conformance/old-pencil-counterexample/serialized_color_scale2-top.rgba.zlib"
            ))
        );
    }
}
