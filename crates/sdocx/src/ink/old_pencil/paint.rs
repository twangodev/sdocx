//! Conditional CPU paint model for the captured Samsung legacy Pencil GLV1.
//!
//! This test-only model is separate from production dispatch. Its profile
//! is limited to the pinned V17 Mesa draw in `paint-conditional-profile.json`. All 12 explicit
//! F32 vertex/plane/blend hypotheses matched the captured sparse FBO, but the
//! backend machine arithmetic IR is not observed; callers must keep the profile
//! guard and fall back outside this evidence.

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
    "../../../../../conformance/old-pencil-native-paint-v1/native-instance-vbo-v17.bin"
);
const NATIVE_SPARSE_RGBA8: &[u8] = include_bytes!(
    "../../../../../conformance/old-pencil-native-paint-v1/native-sparse-readback-u16xy-rgba8.bin"
);
const ATLAS_ZLIB: &[u8] = include_bytes!(
    "../../../../../conformance/old-pencil-native-paint-v1/pencil-atlas-rgba8-144x144.zlib"
);

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
    let decoder = ZlibDecoder::new(ATLAS_ZLIB);
    let mut atlas = Vec::with_capacity(ATLAS_BYTES);
    decoder
        .take((ATLAS_BYTES + 1) as u64)
        .read_to_end(&mut atlas)
        .map_err(|_| "captured Pencil atlas zlib fixture is invalid")?;
    if atlas.len() != ATLAS_BYTES || hash(&atlas) != NATIVE_ATLAS_SHA256 {
        return Err("captured Pencil atlas dimensions or source hash changed");
    }
    Ok(atlas)
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
    let mut result = [0.0; 4];
    for row in 0..4 {
        let mut value = 0.0_f32;
        for column in 0..4 {
            let product = PROJECTION[column * 4 + row];
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
    q0: f32,
    x0: f32,
    y0: f32,
    dx: f32,
    dy: f32,
}

fn plane_coeff(vertices: [[f32; 2]; 3], values: [f32; 3]) -> Result<Plane, &'static str> {
    let [p0, p1, p2] = vertices;
    let dx1 = f32_sub(p1[0], p0[0]);
    let dy1 = f32_sub(p1[1], p0[1]);
    let dx2 = f32_sub(p2[0], p0[0]);
    let dy2 = f32_sub(p2[1], p0[1]);
    let dq1 = f32_sub(values[1], values[0]);
    let dq2 = f32_sub(values[2], values[0]);
    let area = f32_sub(f32_mul(dx1, dy2), f32_mul(dy1, dx2));
    if area == 0.0 || !area.is_finite() {
        return Err("captured Pencil strip triangle is degenerate");
    }
    Ok(Plane {
        q0: values[0],
        x0: p0[0],
        y0: p0[1],
        dx: f32_div(f32_sub(f32_mul(dq1, dy2), f32_mul(dq2, dy1)), area),
        dy: f32_div(f32_sub(f32_mul(dx1, dq2), f32_mul(dx2, dq1)), area),
    })
}

fn eval_plane(plane: Plane, px: f32, py: f32) -> f32 {
    f32_add(
        plane.q0,
        f32_add(
            f32_mul(plane.dx, f32_sub(px, plane.x0)),
            f32_mul(plane.dy, f32_sub(py, plane.y0)),
        ),
    )
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
    if records.len() != INSTANCE_COUNT {
        return Err("captured Pencil paint profile requires exactly 47 instances");
    }
    if atlas.len() != ATLAS_BYTES || hash(atlas) != NATIVE_ATLAS_SHA256 {
        return Err("native Pencil atlas profile mismatch");
    }
    let mut output = vec![0_u8; WIDTH * HEIGHT * 4];
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
            matrix_vector(
                corner_position(corner, point, profile.vertex),
                profile.vertex,
            )
        });
        let window = clip.map(to_window);
        let x0 = window.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min);
        let x1 = window
            .iter()
            .map(|p| p[0])
            .fold(f32::NEG_INFINITY, f32::max);
        let y0 = window.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
        let y1 = window
            .iter()
            .map(|p| p[1])
            .fold(f32::NEG_INFINITY, f32::max);
        let ix0 = ((x0 - 0.5).ceil() as isize).max(0) as usize;
        let ix1 = ((x1 - 0.5).floor() as isize).min(WIDTH as isize - 1);
        let iy0 = ((y0 - 0.5).ceil() as isize).max(0) as usize;
        let iy1 = ((y1 - 0.5).floor() as isize).min(HEIGHT as isize - 1);
        if ix1 < 0 || iy1 < 0 || ix0 > ix1 as usize || iy0 > iy1 as usize {
            continue;
        }
        let bounds = [(x0, x1), (y0, y1)];
        let mut planes = [(
            Plane {
                q0: 0.0,
                x0: 0.0,
                y0: 0.0,
                dx: 0.0,
                dy: 0.0,
            },
            Plane {
                q0: 0.0,
                x0: 0.0,
                y0: 0.0,
                dx: 0.0,
                dy: 0.0,
            },
        ); 2];
        for (triangle, output) in planes.iter_mut().enumerate() {
            let indices = triangle_indices(triangle);
            let triangle_vertices = indices.map(|i| window[i]);
            let frag_x = indices.map(|i| QUAD[i].frag_x);
            let frag_y = indices.map(|i| QUAD[i].frag_y);
            *output = (
                plane_coeff(triangle_vertices, frag_x)?,
                plane_coeff(triangle_vertices, frag_y)?,
            );
        }
        let quotient = f32_div(point.texture_id, 3.0);
        let tile_y = quotient.floor();
        let tile_x = f32_sub(point.texture_id, f32_mul(3.0, tile_y));
        let offset_x = f32_mul(f32_div(1.0, 3.0), tile_x);
        let offset_y = f32_mul(f32_div(1.0, 3.0), tile_y);
        for y_gl in iy0..=iy1 as usize {
            let py = f32_add(y_gl as f32, 0.5);
            if !(bounds[1].0 <= py && py < bounds[1].1) {
                continue;
            }
            for x in ix0..=ix1 as usize {
                let px = f32_add(x as f32, 0.5);
                if !(bounds[0].0 <= px && px < bounds[0].1) {
                    continue;
                }
                let selected = selected_triangle(&window, px, py);
                let triangle = match profile.plane {
                    VaryingPlane::Triangle0 => 0,
                    VaryingPlane::Triangle1 => 1,
                    VaryingPlane::SourceSideSelection => selected,
                };
                let frag_x = eval_plane(planes[triangle].0, px, py);
                let frag_y = eval_plane(planes[triangle].1, px, py);
                let u = f32_add(f32_div(frag_x, 3.0), offset_x);
                let v = f32_add(f32_div(f32_sub(1.0, frag_y), 3.0), offset_y);
                let tex_x = nearest_texel(u, ATLAS_WIDTH);
                let tex_y = nearest_texel(v, ATLAS_HEIGHT);
                let tex_alpha = atlas[(tex_y * ATLAS_WIDTH + tex_x) * 4 + 3];
                if tex_alpha == 0 {
                    continue;
                }
                let source_alpha = f32_mul(f32_div(tex_alpha as f32, 255.0), 1.0);
                let y_top = HEIGHT - 1 - y_gl;
                let at = (y_top * WIDTH + x) * 4;
                let destination_alpha = f32_div(output[at + 3] as f32, 255.0);
                let one_minus_source = f32_sub(1.0, source_alpha);
                let output_alpha =
                    f32_add(source_alpha, f32_mul(destination_alpha, one_minus_source));
                // Input RGB is black and source/destination RGB remain zero.
                output[at + 3] = quantize_unorm8(output_alpha, profile.unorm);
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
        assert_eq!(
            hash(ATLAS_ZLIB),
            "72f21bd4ac7b84a27237a874fd137b50b6cd77c36d98764a3a7f8b11cb73bc6d"
        );
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
