//! Vector projection of the recovered Oblique V4 coverage field.
//!
//! The identity inverse scale used by the producer is a canonical coordinate
//! profile. SVG zoom and PDF raster scale can change the coverage seen at a
//! device pixel; this adapter does not claim device-grid parity.
use super::vector::{
    Blend, ColorMatrix, ColorValue, Data, Definitions, Filter, Gradient, Group, LinearGradient,
    Paint, Path, Rectangle, ReplayPart, Scene, Stop, Styled, SvgFilter,
};
use crate::{CoverageMesh, CoverageTriangle, PreparedStroke};

pub(super) fn render(svg: &mut Scene, paint: &PreparedStroke<'_>, replay: bool) -> bool {
    if paint.profile != Some("ObliquePen") {
        return false;
    }
    let Some(mesh) = paint.coverage_mesh.as_ref() else {
        return false;
    };
    let Some(bounds) = mesh.bounds else {
        return false;
    };
    let Some(color) = parse_rgb(&paint.color) else {
        return false;
    };
    if mesh.triangles.is_empty()
        || mesh.sample_ends.windows(2).any(|w| w[0] > w[1])
        || mesh.terminal_start > mesh.terminal_end
        || mesh.terminal_end > mesh.triangles.len()
        || mesh
            .sample_ends
            .last()
            .is_some_and(|end| *end > mesh.triangles.len())
    {
        return false;
    }

    let x = bounds.x_min - 1.0;
    let y = bounds.y_min - 1.0;
    let width = bounds.x_max - bounds.x_min + 2.0;
    let height = bounds.y_max - bounds.y_min + 2.0;
    if ![x, y, width, height].into_iter().all(f64::is_finite) || width <= 0.0 || height <= 0.0 {
        return false;
    }

    let filter = svg.definition::<Filter>();
    let matrix = ColorMatrix::matrix([
        0.,
        0.,
        0.,
        0.,
        f64::from(color.0) / 255.,
        0.,
        0.,
        0.,
        0.,
        f64::from(color.1) / 255.,
        0.,
        0.,
        0.,
        0.,
        f64::from(color.2) / 255.,
        1.,
        0.,
        0.,
        0.,
        0.,
    ]);
    let mut definitions = Definitions::new().add(
        SvgFilter::new(&filter)
            .region(x, y, width, height)
            .add(matrix),
    );

    let mut fields = Vec::with_capacity(mesh.triangles.len());
    for (index, triangle) in mesh.triangles.iter().enumerate() {
        if !valid_triangle(triangle) {
            return false;
        }
        if zero_area(triangle) {
            fields.push((index, Field::Empty));
            continue;
        }
        if let Some((gradient, field)) = gradient_for(svg, triangle) {
            definitions = definitions.add(gradient);
            fields.push((index, field));
        } else if triangle.vertices.iter().all(|v| v.coverage == 0.0) {
            fields.push((index, Field::Empty));
        } else if triangle.vertices.iter().all(|v| v.coverage == 1.0) {
            fields.push((index, Field::White));
        } else {
            // The structural PDF lowerer recognizes constant white or an
            // exact black-to-white user-space ramp; it does not approximate
            // a constant intermediate field.
            return false;
        }
    }

    #[cfg(feature = "pdf")]
    if svg.retains_text() && paint.opacity == 0.0 {
        // A fully transparent field has no PDF-visible contribution; omit it
        // before the usvg tree can prune its carrier ID.
        return true;
    }

    #[cfg(feature = "pdf")]
    let native_ink_id = if svg.retains_text() {
        match svg
            .native_ink()
            .register(&mesh, [color.0, color.1, color.2], paint.opacity)
        {
            Ok(id) => Some(id),
            Err(_) => return false,
        }
    } else {
        None
    };

    svg.scope(definitions, |_| {});
    let mut outer_group = Group::new().filtered(&filter).opacity(paint.opacity);
    #[cfg(feature = "pdf")]
    if let Some(id) = native_ink_id {
        outer_group = outer_group.native_ink_id(id);
    }
    svg.scope(outer_group, |svg| {
        svg.scope(Group::new().isolated(), |svg| {
            // Keep the black field first: it is the opaque destination for
            // max/lighten coverage, not a translucent accumulation.
            svg.push(
                Rectangle::new()
                    .x(x)
                    .y(y)
                    .width(width)
                    .height(height)
                    .fill(Paint::BLACK),
            );
            for (index, field) in &fields {
                if matches!(field, Field::Empty) {
                    continue;
                }
                let Some(triangle) = mesh.triangles.get(*index) else {
                    continue;
                };
                let data = triangle_path(triangle);
                let sample = replay_sample(mesh, *index, paint.points.len());
                let group = Group::new()
                    .blend(Blend::Lighten)
                    .isolated()
                    .replay_part(replay.then_some(ReplayPart(sample + 1)));
                let (black, gray) = match field {
                    Field::White => (
                        Path::new()
                            .fill(Paint::BLACK)
                            .crisp_edges()
                            .data(data.clone()),
                        Path::new()
                            .fill(Paint::from_hex("#ffffff"))
                            .crisp_edges()
                            .data(data),
                    ),
                    Field::Gradient(id) => (
                        Path::new()
                            .fill(Paint::BLACK)
                            .crisp_edges()
                            .data(data.clone()),
                        Path::new()
                            .fill(Paint::Gradient(*id))
                            .crisp_edges()
                            .data(data),
                    ),
                    Field::Empty => unreachable!(),
                };
                svg.scope(group, |svg| {
                    svg.push(black);
                    svg.push(gray);
                });
            }
        });
    });
    true
}

#[derive(Clone, Copy)]
enum Field {
    Empty,
    White,
    Gradient(super::vector::Definition<Gradient>),
}

fn gradient_for(svg: &mut Scene, triangle: &CoverageTriangle) -> Option<(LinearGradient, Field)> {
    let [a, b, c] = triangle.vertices;
    let vertices = [a, b, c];
    if vertices.iter().any(|v| {
        !v.x.is_finite()
            || !v.y.is_finite()
            || !v.coverage.is_finite()
            || !(0.0..=1.0).contains(&v.coverage)
    }) {
        return None;
    }
    let dx1 = f64::from(b.x) - f64::from(a.x);
    let dy1 = f64::from(b.y) - f64::from(a.y);
    let dx2 = f64::from(c.x) - f64::from(a.x);
    let dy2 = f64::from(c.y) - f64::from(a.y);
    let dc1 = f64::from(b.coverage) - f64::from(a.coverage);
    let dc2 = f64::from(c.coverage) - f64::from(a.coverage);
    let determinant = dx1 * dy2 - dy1 * dx2;
    if determinant == 0.0 || !determinant.is_finite() {
        return None;
    }
    let gx = (dc1 * dy2 - dy1 * dc2) / determinant;
    let gy = (dx1 * dc2 - dc1 * dx2) / determinant;
    let norm2 = gx * gx + gy * gy;
    if norm2 == 0.0 || !norm2.is_finite() {
        return None;
    }
    let origin_x = f64::from(a.x) - gx * f64::from(a.coverage) / norm2;
    let origin_y = f64::from(a.y) - gy * f64::from(a.coverage) / norm2;
    let id = svg.definition::<Gradient>();
    let gradient = LinearGradient::new(&id)
        .x1(origin_x)
        .y1(origin_y)
        .x2(origin_x + gx / norm2)
        .y2(origin_y + gy / norm2)
        .add(Stop::new(0, ColorValue::BLACK))
        .add(Stop::new(1, ColorValue::WHITE));
    Some((gradient, Field::Gradient(id)))
}

fn valid_triangle(triangle: &CoverageTriangle) -> bool {
    triangle.vertices.iter().all(|v| {
        v.x.is_finite()
            && v.y.is_finite()
            && v.coverage.is_finite()
            && (0.0..=1.0).contains(&v.coverage)
    })
}

fn zero_area(triangle: &CoverageTriangle) -> bool {
    let [a, b, c] = triangle.vertices;
    let area = (f64::from(b.x) - f64::from(a.x)) * (f64::from(c.y) - f64::from(a.y))
        - (f64::from(b.y) - f64::from(a.y)) * (f64::from(c.x) - f64::from(a.x));
    area == 0.0
}

fn triangle_path(triangle: &CoverageTriangle) -> Data {
    let [a, b, c] = triangle.vertices;
    Data::new()
        .move_to((a.x, a.y))
        .line_to((b.x, b.y))
        .line_to((c.x, c.y))
        .close()
}

/// Native sample boundaries describe the saved interior calls; the distinct
/// terminal suffix is intentionally attached to the last saved sample.
fn replay_sample(mesh: &CoverageMesh, triangle: usize, original_count: usize) -> usize {
    if original_count == 0 {
        return 0;
    }
    if triangle >= mesh.terminal_start {
        return original_count - 1;
    }
    let source_boundary = mesh.sample_ends.partition_point(|end| *end <= triangle);
    source_boundary.saturating_add(1).min(original_count - 1)
}

fn parse_rgb(text: &str) -> Option<(u8, u8, u8)> {
    let hex = text.strip_prefix('#')?;
    if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let value = u32::from_str_radix(hex, 16).ok()?;
    Some(((value >> 16) as u8, (value >> 8) as u8, value as u8))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CoverageTriangle, CoverageVertex, Point};
    use std::borrow::Cow;

    #[test]
    fn native_mesh_serializes_as_the_max_lowerer_triangle_graph() {
        let points = [
            Point { x: 8.0, y: 12.0 },
            Point { x: 23.0, y: 15.0 },
            Point { x: 39.0, y: 10.0 },
        ];
        let mesh = crate::ink::prepare_oblique_mesh(&points, 5.0, true, 2, [1.0, 1.0])
            .expect("bounded native geometry");
        assert_eq!(mesh.sample_ends.len(), points.len() - 1);
        assert!(mesh.terminal_start <= mesh.terminal_end);
        if mesh.terminal_start < mesh.terminal_end {
            assert_eq!(
                replay_sample(&mesh, mesh.terminal_start, points.len()),
                points.len() - 1
            );
        }
        let paint = PreparedStroke {
            points: Cow::Borrowed(&points),
            sample_ends: None,
            dot_radii: None,
            dot_directions: None,
            rect_stamp: None,
            coverage_mesh: Some(mesh.clone()),
            segment_widths: None,
            width: 5.0,
            bounds: mesh.bounds,
            color: "#336699".into(),
            opacity: 0.5,
            top_layer: false,
            profile: Some("ObliquePen"),
            support: crate::InkSupport::Reconstructed,
        };
        let mut svg = Scene::new(super::super::vector::Svg::new());
        assert!(render(&mut svg, &paint, true));
        let output = svg.finish();
        let document = roxmltree::Document::parse(&output).unwrap();
        let outer = document
            .descendants()
            .find(|node| node.has_tag_name("g") && node.attribute("filter").is_some())
            .unwrap();
        assert_eq!(outer.attribute("opacity"), Some("0.5"));
        let source = outer
            .children()
            .find(|node| node.has_tag_name("g"))
            .unwrap();
        let mut children = source.children().filter(|node| node.is_element());
        assert_eq!(children.next().unwrap().tag_name().name(), "rect");
        let groups = children.collect::<Vec<_>>();
        assert!(groups.len() > 1);
        assert!(groups.iter().all(|group| {
            group
                .attribute("style")
                .is_some_and(|s| s.contains("mix-blend-mode:lighten"))
                && group
                    .attribute("style")
                    .is_some_and(|s| s.contains("isolation:isolate"))
                && group
                    .attribute("data-replay-part")
                    .and_then(|n| n.parse::<usize>().ok())
                    .is_some_and(|index| (1..=points.len()).contains(&index))
        }));
        for group in &groups {
            let paths = group
                .children()
                .filter(|node| node.has_tag_name("path"))
                .collect::<Vec<_>>();
            assert_eq!(paths.len(), 2);
            assert_eq!(paths[0].attribute("d"), paths[1].attribute("d"));
            assert!(
                paths
                    .iter()
                    .all(|path| path.attribute("shape-rendering") == Some("crispEdges"))
            );
        }
        let background = source
            .children()
            .find(|node| node.has_tag_name("rect"))
            .unwrap();
        assert_eq!(background.attribute("fill"), Some("#000000"));
        assert!(
            document
                .descendants()
                .filter(|node| node.has_tag_name("linearGradient"))
                .all(|gradient| {
                    gradient.attribute("gradientUnits") == Some("userSpaceOnUse")
                        && gradient
                            .children()
                            .filter(|node| node.has_tag_name("stop"))
                            .count()
                            == 2
                })
        );
    }

    #[test]
    fn translated_triangle_gradient_uses_widened_f32_endpoint_differences() {
        let triangle = CoverageTriangle {
            vertices: [
                CoverageVertex {
                    x: 100_000.0,
                    y: 100_000.0,
                    coverage: 0.0,
                },
                CoverageVertex {
                    x: 100_008.0,
                    y: 100_000.0,
                    coverage: 1.0,
                },
                CoverageVertex {
                    x: 100_000.0,
                    y: 100_016.0,
                    coverage: 0.0,
                },
            ],
        };
        let mut svg = Scene::new(super::super::vector::Svg::new());
        let (gradient, _) = gradient_for(&mut svg, &triangle).unwrap();
        svg.scope(Definitions::new().add(gradient), |_| {});
        let output = svg.finish();
        let document = roxmltree::Document::parse(&output).unwrap();
        let gradient = document
            .descendants()
            .find(|node| node.has_tag_name("linearGradient"))
            .unwrap();
        let x1: f64 = gradient.attribute("x1").unwrap().parse().unwrap();
        let x2: f64 = gradient.attribute("x2").unwrap().parse().unwrap();
        assert!((x2 - x1 - 8.0).abs() < 1e-9);
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn transparent_retained_field_is_omitted_before_carrier_pruning() {
        let points = [Point { x: 8.0, y: 12.0 }, Point { x: 23.0, y: 15.0 }];
        let mesh = crate::ink::prepare_oblique_mesh(&points, 5.0, true, 2, [1.0, 1.0])
            .expect("bounded native geometry");
        let paint = PreparedStroke {
            points: Cow::Borrowed(&points),
            sample_ends: None,
            dot_radii: None,
            dot_directions: None,
            rect_stamp: None,
            coverage_mesh: Some(mesh.clone()),
            segment_widths: None,
            width: 5.0,
            bounds: mesh.bounds,
            color: "#336699".into(),
            opacity: 0.0,
            top_layer: false,
            profile: Some("ObliquePen"),
            support: crate::InkSupport::Reconstructed,
        };
        let mut svg = Scene::new(super::super::vector::Svg::new());
        svg.retain_text();
        assert!(render(&mut svg, &paint, false));
        assert!(svg.take_native_ink().is_empty());
        assert!(!svg.finish().contains("filter"));
    }
}
