//! PDF-native transport for registered grayscale ink coverage fields.
//!
//! The SVG renderer emits an opaque, unique image marker at the exact paint
//! position. After Krilla serializes the page, this module replaces only those
//! marker Image XObjects with Forms that evaluate the native max-affine field.

use crate::{BoundingBox, CoverageMesh};
use krilla::{
    Data,
    geom::{Size, Transform},
    image::Image,
    surface::Surface,
};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};

/// Native coverage field retained alongside its unique in-stream marker.
pub(crate) struct PdfInkField<'a> {
    pub marker_id: u64,
    pub mesh: &'a CoverageMesh,
    pub color: [u8; 3],
    pub bounds: BoundingBox,
}

const MARKER_WIDTH: u32 = 8;
const MARKER_HEIGHT: u32 = 1;

fn marker_pixels(marker_id: u64) -> Vec<u8> {
    let mut payload = Vec::with_capacity(24);
    payload.extend_from_slice(b"SDOCXIF1");
    payload.extend_from_slice(&marker_id.to_le_bytes());
    payload.extend_from_slice(
        &marker_id
            .rotate_left(29)
            .wrapping_add(0x9e37_79b9_7f4a_7c15)
            .to_le_bytes(),
    );
    payload
}

fn marker_png(marker_id: u64) -> Result<Vec<u8>, String> {
    let pixels = marker_pixels(marker_id);
    let mut png = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png, MARKER_WIDTH, MARKER_HEIGHT);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|error| error.to_string())?;
        writer
            .write_image_data(&pixels)
            .map_err(|error| error.to_string())?;
    }
    Ok(png)
}

/// Emit a unique opaque marker image in local source coordinates. The caller
/// has already applied the registered SVG group's transform, clip and opacity.
pub(super) fn emit_marker(
    surface: &mut Surface<'_>,
    marker_id: u64,
    bounds: BoundingBox,
) -> Result<(), String> {
    let width = (bounds.x_max - bounds.x_min) as f32;
    let height = (bounds.y_max - bounds.y_min) as f32;
    if ![bounds.x_min, bounds.y_min, bounds.x_max, bounds.y_max]
        .into_iter()
        .all(f64::is_finite)
        || !width.is_finite()
        || !height.is_finite()
        || width <= 0.0
        || height <= 0.0
    {
        return Err("native ink field has invalid marker bounds".into());
    }
    let image = Image::from_png(Data::from(marker_png(marker_id)?), false)?;
    let size = Size::from_wh(width, height).ok_or("native ink marker size is invalid")?;
    surface.push_transform(&Transform::from_translate(
        bounds.x_min as f32,
        bounds.y_min as f32,
    ));
    surface.draw_image(image, size);
    surface.pop();
    Ok(())
}

#[derive(Clone, Copy)]
struct Triangle([[f64; 3]; 3]);

fn triangles(mesh: &CoverageMesh) -> Result<Vec<Triangle>, String> {
    mesh.triangles
        .iter()
        .map(|triangle| {
            let vertices = triangle.vertices.map(|vertex| {
                [
                    f64::from(vertex.x),
                    f64::from(vertex.y),
                    f64::from(vertex.coverage),
                ]
            });
            if vertices.iter().flatten().all(|value| value.is_finite()) {
                Ok(Triangle(vertices))
            } else {
                Err("native ink mesh contains non-finite vertices".into())
            }
        })
        .collect()
}

fn fmt(value: f64) -> String {
    let text = format!("{value:.17}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text.is_empty() || text == "-0" {
        "0".into()
    } else {
        text.into()
    }
}

fn plane(triangle: Triangle) -> (f64, f64, f64, f64) {
    let [[x0, y0, z0], [x1, y1, z1], [x2, y2, z2]] = triangle.0;
    let determinant = (x1 - x0) * (y2 - y0) - (x2 - x0) * (y1 - y0);
    let a = ((z1 - z0) * (y2 - y0) - (z2 - z0) * (y1 - y0)) / determinant;
    let b = ((x1 - x0) * (z2 - z0) - (x2 - x0) * (z1 - z0)) / determinant;
    (a, b, z0 - a * x0 - b * y0, determinant)
}

fn edge_code(p: [f64; 2], q: [f64; 2], orientation: i32) -> String {
    format!(
        " {} sub {} mul exch {} sub {} mul sub {} mul 0 ge",
        fmt(p[1]),
        fmt(q[0] - p[0]),
        fmt(p[0]),
        fmt(q[1] - p[1]),
        orientation
    )
}

fn function_code(triangles: &[Triangle], bounds: BoundingBox) -> Result<Vec<u8>, String> {
    let width = bounds.x_max - bounds.x_min;
    let height = bounds.y_max - bounds.y_min;
    let mut code = format!(
        "{{ 1 exch sub {} mul {} add exch {} mul {} add exch 0",
        fmt(height),
        fmt(bounds.y_min),
        fmt(width),
        fmt(bounds.x_min)
    );
    for triangle in triangles {
        let [[x0, y0, _], [x1, y1, _], [x2, y2, _]] = triangle.0;
        let (a, b, c, determinant) = plane(*triangle);
        if determinant == 0.0 {
            continue;
        }
        if ![a, b, c].into_iter().all(f64::is_finite) {
            return Err("native ink mesh produced an invalid affine plane".into());
        }
        for [x, y, z] in triangle.0 {
            let evaluated = a * x + b * y + c;
            let magnitude = (a * x).abs() + (b * y).abs() + c.abs() + z.abs() + 1.0;
            let tolerance = 128.0 * f64::EPSILON * magnitude;
            if (evaluated - z).abs() > tolerance {
                return Err("native ink affine plane failed its vertex check".into());
            }
        }
        let xy = [[x0, y0], [x1, y1], [x2, y2]];
        let orientation = if determinant > 0.0 { 1 } else { -1 };
        code.push_str(" 3 copy pop");
        code.push_str(&edge_code(xy[0], xy[1], orientation));
        for edge in 1..3 {
            code.push_str(" 4 copy pop pop");
            code.push_str(&edge_code(xy[edge], xy[(edge + 1) % 3], orientation));
            code.push_str(" and");
        }
        code.push_str(&format!(
            " {{ 3 copy pop {} mul exch {} mul add {} add 2 copy lt {{ exch pop }} {{ pop }} ifelse }} if",
            fmt(b), fmt(a), fmt(c)
        ));
    }
    code.push_str(" exch pop exch pop }");
    Ok(code.into_bytes())
}

fn tiles(
    triangles: &[Triangle],
    bounds: BoundingBox,
) -> Result<Vec<([f64; 4], Vec<Triangle>)>, String> {
    let width = bounds.x_max - bounds.x_min;
    let height = bounds.y_max - bounds.y_min;
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return Err("native ink field has invalid bounds".into());
    }
    const MAX_AXIS_TILES: usize = 64;
    const MAX_TILES: usize = MAX_AXIS_TILES * MAX_AXIS_TILES;
    const MAX_CANDIDATE_REFERENCES: usize = 1_000_000;
    let step = (width.max(height) / MAX_AXIS_TILES as f64).max(4.0);
    if !step.is_finite() || step <= 0.0 {
        return Err("native ink field bounds cannot be divided into finite tiles".into());
    }
    let mut result = Vec::new();
    let mut candidate_references = 0usize;
    // Indexed boundaries avoid an accumulated rounding remainder creating a
    // 65th tile on an axis that should have exactly 64 subdivisions.
    let columns = ((width / step).ceil() as usize).clamp(1, MAX_AXIS_TILES);
    let rows = ((height / step).ceil() as usize).clamp(1, MAX_AXIS_TILES);
    for column in 0..columns {
        let x = bounds.x_min + column as f64 * step;
        let x1 = if column + 1 == columns {
            bounds.x_max
        } else {
            bounds.x_min + (column + 1) as f64 * step
        };
        if !x1.is_finite() || x1 <= x {
            return Err("native ink x tiling did not advance".into());
        }
        for row in 0..rows {
            let y = bounds.y_min + row as f64 * step;
            let y1 = if row + 1 == rows {
                bounds.y_max
            } else {
                bounds.y_min + (row + 1) as f64 * step
            };
            if !y1.is_finite() || y1 <= y {
                return Err("native ink y tiling did not advance".into());
            }
            let candidates = triangles
                .iter()
                .copied()
                .filter(|triangle| {
                    plane(*triangle).3 != 0.0 && {
                        let xs = triangle.0.map(|vertex| vertex[0]);
                        let ys = triangle.0.map(|vertex| vertex[1]);
                        xs.iter().copied().fold(f64::NEG_INFINITY, f64::max) >= x
                            && xs.iter().copied().fold(f64::INFINITY, f64::min) <= x1
                            && ys.iter().copied().fold(f64::NEG_INFINITY, f64::max) >= y
                            && ys.iter().copied().fold(f64::INFINITY, f64::min) <= y1
                    }
                })
                .collect::<Vec<_>>();
            if !candidates.is_empty() {
                candidate_references = candidate_references
                    .checked_add(candidates.len())
                    .ok_or("native ink candidate count overflow")?;
                if candidate_references > MAX_CANDIDATE_REFERENCES {
                    return Err("native ink field exceeds candidate reference budget".into());
                }
                let u0 = (x - bounds.x_min) / width;
                let u1 = (x1 - bounds.x_min) / width;
                let v0 = 1.0 - (y1 - bounds.y_min) / height;
                let v1 = 1.0 - (y - bounds.y_min) / height;
                result.push(([u0, v0, u1, v1], candidates));
                if result.len() > MAX_TILES {
                    return Err("native ink field exceeds tile budget".into());
                }
            }
        }
    }
    if result.is_empty() {
        return Err("native ink field has no non-degenerate coverage triangles".into());
    }
    Ok(result)
}

fn field_form(document: &mut Document, field: &PdfInkField<'_>) -> Result<Object, String> {
    let triangles = triangles(field.mesh)?;
    const MAX_TRIANGLES: usize = 16_384;
    if triangles.len() > MAX_TRIANGLES {
        return Err("native ink field exceeds triangle budget".into());
    }
    if triangles.is_empty() || triangles.iter().all(|triangle| plane(*triangle).3 == 0.0) {
        return Err("native ink field has no non-degenerate coverage triangles".into());
    }
    for triangle in &triangles {
        for [x, y, _] in triangle.0 {
            if x < field.bounds.x_min
                || x > field.bounds.x_max
                || y < field.bounds.y_min
                || y > field.bounds.y_max
            {
                return Err("native ink bounds do not contain every mesh vertex".into());
            }
        }
    }
    let tile_data = tiles(&triangles, field.bounds)?;
    let mut shadings = Dictionary::new();
    let mut operators = Vec::with_capacity(tile_data.len());
    let mut generated_code_bytes = 0usize;
    for (index, (bbox, candidates)) in tile_data.iter().enumerate() {
        let code = function_code(candidates, field.bounds)?;
        generated_code_bytes = generated_code_bytes
            .checked_add(code.len())
            .ok_or("native ink calculator size overflow")?;
        if generated_code_bytes > 32 * 1024 * 1024 {
            return Err("native ink field exceeds calculator byte budget".into());
        }
        let function = document.add_object(Stream::new(
            Dictionary::from_iter([
                (b"FunctionType".to_vec(), Object::Integer(4)),
                (
                    b"Domain".to_vec(),
                    Object::Array(vec![0.into(), 1.into(), 0.into(), 1.into()]),
                ),
                (b"Range".to_vec(), Object::Array(vec![0.into(), 1.into()])),
            ]),
            code,
        ));
        let shading = document.add_object(Dictionary::from_iter([
            (b"ShadingType".to_vec(), Object::Integer(1)),
            (b"ColorSpace".to_vec(), Object::Name(b"DeviceGray".to_vec())),
            (
                b"Domain".to_vec(),
                Object::Array(vec![
                    bbox[0].into(),
                    bbox[2].into(),
                    bbox[1].into(),
                    bbox[3].into(),
                ]),
            ),
            (
                b"BBox".to_vec(),
                Object::Array(
                    bbox.iter()
                        .map(|value| Object::Real(*value as f32))
                        .collect(),
                ),
            ),
            (b"Function".to_vec(), Object::Reference(function)),
        ]));
        shadings.set(format!("S{index}"), Object::Reference(shading));
        operators.push(format!("/S{index} sh"));
    }

    let mut group_resources = Dictionary::new();
    group_resources.set("Shading", shadings);
    let mut group_dict = Dictionary::new();
    group_dict.set("Type", "XObject");
    group_dict.set("Subtype", "Form");
    group_dict.set("FormType", 1);
    group_dict.set("BBox", vec![0.into(), 0.into(), 1.into(), 1.into()]);
    group_dict.set("Resources", group_resources);
    group_dict.set(
        "Group",
        Dictionary::from_iter([
            (b"S".to_vec(), Object::Name(b"Transparency".to_vec())),
            (b"CS".to_vec(), Object::Name(b"DeviceGray".to_vec())),
            (b"I".to_vec(), Object::Boolean(true)),
            (b"K".to_vec(), Object::Boolean(false)),
        ]),
    );
    let mask = document.add_object(Stream::new(group_dict, operators.join("\n").into_bytes()));
    let state = document.add_object(Dictionary::from_iter([(
        b"SMask".to_vec(),
        Object::Dictionary(Dictionary::from_iter([
            (b"S".to_vec(), Object::Name(b"Luminosity".to_vec())),
            (b"G".to_vec(), Object::Reference(mask)),
            (b"BC".to_vec(), Object::Array(vec![0.into()])),
            (b"TR".to_vec(), Object::Name(b"Identity".to_vec())),
        ])),
    )]));
    let mut states = Dictionary::new();
    states.set("GS", Object::Reference(state));
    let mut resources = Dictionary::new();
    resources.set("ExtGState", states);
    let mut dict = Dictionary::new();
    dict.set("Type", "XObject");
    dict.set("Subtype", "Form");
    dict.set("FormType", 1);
    dict.set("BBox", vec![0.into(), 0.into(), 1.into(), 1.into()]);
    dict.set("Resources", resources);
    Ok(Object::Stream(Stream::new(
        dict,
        format!(
            "/GS gs {} {} {} rg 0 0 1 1 re f",
            field.color[0] as f64 / 255.0,
            field.color[1] as f64 / 255.0,
            field.color[2] as f64 / 255.0
        )
        .into_bytes(),
    )))
}

fn image_payload_matches(stream: &Stream, marker_id: u64) -> bool {
    stream
        .dict
        .get(b"Subtype")
        .ok()
        .and_then(|value| value.as_name().ok())
        == Some(b"Image")
        && stream
            .dict
            .get(b"Width")
            .ok()
            .and_then(|value| value.as_i64().ok())
            == Some(MARKER_WIDTH as i64)
        && stream
            .dict
            .get(b"Height")
            .ok()
            .and_then(|value| value.as_i64().ok())
            == Some(MARKER_HEIGHT as i64)
        && stream
            .dict
            .get(b"ColorSpace")
            .ok()
            .and_then(|value| value.as_name().ok())
            == Some(b"DeviceRGB")
        && stream
            .dict
            .get(b"BitsPerComponent")
            .ok()
            .and_then(|value| value.as_i64().ok())
            == Some(8)
        && !stream.dict.has(b"SMask")
        && stream.decompressed_content().ok().as_deref()
            == Some(marker_pixels(marker_id).as_slice())
}

/// Replace exact registered marker Image XObjects with native field Forms.
pub(crate) fn replace_markers(bytes: &[u8], fields: &[PdfInkField<'_>]) -> Result<Vec<u8>, String> {
    if fields.is_empty() {
        return Ok(bytes.to_vec());
    }
    let mut document = Document::load_mem(bytes).map_err(|error| error.to_string())?;
    let mut ids = std::collections::HashSet::new();
    for field in fields {
        if !ids.insert(field.marker_id) {
            return Err(format!(
                "duplicate native ink marker id {}",
                field.marker_id
            ));
        }
        let matches = document
            .objects
            .iter()
            .filter_map(|(&id, object)| {
                let Object::Stream(stream) = object else {
                    return None;
                };
                image_payload_matches(stream, field.marker_id).then_some(id)
            })
            .collect::<Vec<ObjectId>>();
        if matches.len() != 1 {
            return Err(format!(
                "native ink marker {} matched {} Image XObjects",
                field.marker_id,
                matches.len()
            ));
        }
        let form = field_form(&mut document, field)?;
        document.objects.insert(matches[0], form);
    }
    for object in document.objects.values_mut() {
        if let Object::Stream(stream) = object {
            if !stream.dict.has(b"Filter") {
                stream.compress().map_err(|error| error.to_string())?;
            }
        }
    }
    let mut output = Vec::new();
    document
        .save_to(&mut output)
        .map_err(|error| error.to_string())?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CoverageTriangle, CoverageVertex};

    fn mesh(vertices: [[f32; 3]; 3]) -> CoverageMesh {
        CoverageMesh {
            triangles: vec![CoverageTriangle {
                vertices: vertices.map(|[x, y, coverage]| CoverageVertex { x, y, coverage }),
            }],
            sample_ends: Vec::new(),
            terminal_start: 0,
            terminal_end: 1,
            bounds: None,
        }
    }

    #[test]
    fn tiles_advance_with_extreme_finite_bounds_and_stay_bounded() {
        let triangle = Triangle([[0.0, 0.0, 1.0], [1.0, 0.0, 1.0], [0.0, 1.0, 1.0]]);
        let bounds = BoundingBox {
            x_min: -3.0e38,
            y_min: -3.0e38,
            x_max: 3.0e38,
            y_max: 3.0e38,
        };
        let result = tiles(&[triangle], bounds).unwrap();
        assert!(result.len() <= 64 * 64);
        assert!(!result.is_empty());
        for (tile, _) in result {
            assert!(tile.into_iter().all(|value| (0.0..=1.0).contains(&value)));
            assert!(tile[0] < tile[2] && tile[1] < tile[3]);
        }
    }

    #[test]
    fn tile_budget_does_not_reject_rounding_remainders() {
        let lo = f64::from(1.0e-7_f32);
        let hi = f64::from(512.1_f32);
        let bounds = BoundingBox {
            x_min: lo,
            y_min: lo,
            x_max: hi,
            y_max: hi,
        };
        let triangle = Triangle([[lo, lo, 0.0], [hi, lo, 1.0], [lo, hi, 1.0]]);
        let result = tiles(&[triangle], bounds).unwrap();
        assert_eq!(result.len(), 64 * 64);
        assert_eq!(
            result.first().unwrap().0,
            [0.0, 63.0 / 64.0, 1.0 / 64.0, 1.0]
        );
        assert_eq!(result.last().unwrap().0[2], 1.0);
        assert_eq!(result.last().unwrap().0[1], 0.0);
        // Every boundary is shared exactly with the adjacent tile.
        for column in 0..64 {
            for row in 0..63 {
                let lower = result[column * 64 + row].0;
                let upper = result[column * 64 + row + 1].0;
                assert_eq!(lower[1], upper[3]);
            }
        }
    }

    #[test]
    fn rejects_mesh_without_any_non_degenerate_triangle() {
        let mesh = mesh([[0.0, 0.0, 0.0], [1.0, 1.0, 0.5], [2.0, 2.0, 1.0]]);
        let bounds = BoundingBox {
            x_min: 0.0,
            y_min: 0.0,
            x_max: 2.0,
            y_max: 2.0,
        };
        let field = PdfInkField {
            marker_id: 1,
            mesh: &mesh,
            color: [0, 0, 0],
            bounds,
        };
        assert!(
            field_form(&mut Document::with_version("1.7"), &field)
                .unwrap_err()
                .contains("no non-degenerate")
        );
    }

    #[test]
    fn validates_planes_at_large_document_coordinates() {
        let base = 1.0e30_f32;
        let delta = 1.0e24_f32;
        let mesh = mesh([
            [base, base, 0.0],
            [base + delta, base, 0.5],
            [base, base + delta, 1.0],
        ]);
        let bounds = BoundingBox {
            x_min: f64::from(base),
            y_min: f64::from(base),
            x_max: f64::from(base + delta),
            y_max: f64::from(base + delta),
        };
        let triangles = triangles(&mesh).unwrap();
        assert!(plane(triangles[0]).3 != 0.0);
        assert!(!function_code(&triangles, bounds).unwrap().is_empty());
    }
}
