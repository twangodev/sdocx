//! Bounded SVG projection of Drawing's shared fill/outline color-gradient math.
//! Source pins and execution limits: docs/reverse-engineering/shape-fill-findings.md.

use super::vector::{
    ColorValue, Definitions, Gradient, LinearGradient, Paint, RadialGradient, Scene, Stop,
    Transform,
};
use super::{PaintDiagnosticKind, PaintRole, RenderTheme};
use crate::{BoundingBox, Color, ColorPaintSource, NativeShape};

/// The exact f64 rotation applied to procedural geometry by the SVG renderer.
#[derive(Clone, Copy, Debug)]
pub(super) struct ElementRotation {
    pub angle: f64,
    pub center: [f64; 2],
}

impl ElementRotation {
    fn inverse(self) -> Transform {
        Transform::rotate_unrounded(-self.angle, self.center[0], self.center[1])
    }
}

#[derive(Debug)]
enum Geometry {
    Linear([[f32; 2]; 2]),
    Radial { center: [f32; 2], radius: f32 },
}

#[derive(Debug, PartialEq)]
struct ProjectedStop {
    offset: f64,
    argb: u32,
}

/// All admission and numeric validation precede SVG definition allocation.
#[derive(Debug)]
pub(super) struct Plan {
    geometry: Geometry,
    stops: Vec<ProjectedStop>,
    element_rotation: Option<ElementRotation>,
}

impl Plan {
    pub fn for_shape(
        shape: &NativeShape,
        source: &ColorPaintSource,
        role: PaintRole,
        element_rotation: Option<ElementRotation>,
    ) -> Result<Self, PaintDiagnosticKind> {
        let active = match role {
            PaintRole::Fill => {
                source.outline_color_type.is_none() && source.property_flags & 1 != 0
            }
            PaintRole::Outline => source.outline_color_type == Some(1),
        };
        // Fill consumes bits 0/1; outline consumes the whole byte as a bool.
        if !active || !source.trailing_data.is_empty() || !matches!(source.gradient_type, 0 | 1) {
            return Err(PaintDiagnosticKind::UnsupportedPaint);
        }
        let metadata = bounds(shape.metadata.bbox);
        let drawn = bounds(shape.drawn_bbox);
        if metadata != drawn
            || metadata.iter().any(|v| !v.is_finite())
            || shape
                .metadata
                .rotation_degrees
                .is_some_and(|angle| angle != 0.0)
        {
            return Err(PaintDiagnosticKind::UnsupportedGradientFrame);
        }
        let rect = bounds(shape.geometry_bbox);
        let width = rect[2] - rect[0];
        let height = rect[3] - rect[1];
        let center = [(rect[0] + rect[2]) * 0.5, (rect[1] + rect[3]) * 0.5];
        if rect.iter().any(|v| !v.is_finite())
            || !width.is_finite()
            || !height.is_finite()
            || width <= 0.0
            || height <= 0.0
            || !shape.rotation_degrees.is_finite()
            || element_rotation.is_some_and(|rotation| rotation.inverse().text().is_none())
        {
            return Err(PaintDiagnosticKind::UnrepresentableGradient);
        }
        let stops = project_stops(source)?;
        let rotate = |point| {
            rotate_point(
                point,
                center,
                shape.rotation_degrees,
                source.gradient_rotatable(),
            )
        };
        let geometry = if source.gradient_type == 0 {
            let points = linear_points(rect, source.linear_angle).map(rotate);
            if points.iter().flatten().any(|v| !v.is_finite()) || points[0] == points[1] {
                return Err(PaintDiagnosticKind::UnrepresentableGradient);
            }
            Geometry::Linear(points)
        } else {
            let (position, radius) = radial_points(
                rect,
                source.position.map(|value| value.value()),
                shape.rotation_degrees,
                source.gradient_rotatable(),
            );
            if position.iter().any(|v| !v.is_finite()) || !radius.is_finite() || radius <= 0.0 {
                return Err(PaintDiagnosticKind::UnrepresentableGradient);
            }
            Geometry::Radial {
                center: position,
                radius,
            }
        };
        Ok(Self {
            geometry,
            stops,
            element_rotation,
        })
    }

    /// The caller uses opacity 1; saved alpha belongs only to each stop.
    pub fn paint(self, scene: &mut Scene, theme: RenderTheme) -> Paint {
        let id = scene.definition::<Gradient>();
        let stops = self.stops.into_iter().map(|stop| {
            let color = theme.foreground_color(Color {
                r: (stop.argb >> 16) as u8,
                g: (stop.argb >> 8) as u8,
                b: stop.argb as u8,
            });
            Stop::new(stop.offset, ColorValue::Rgb(color))
                .opacity(f64::from((stop.argb >> 24) as u8) / 255.0)
        });
        let definition = match self.geometry {
            Geometry::Linear([begin, end]) => {
                let mut gradient = LinearGradient::new(&id)
                    .x1(begin[0])
                    .y1(begin[1])
                    .x2(end[0])
                    .y2(end[1]);
                if let Some(rotation) = self.element_rotation {
                    gradient = gradient.transformed(rotation.inverse());
                }
                for stop in stops {
                    gradient = gradient.add(stop);
                }
                Definitions::new().add(gradient)
            }
            Geometry::Radial { center, radius } => {
                let mut gradient = RadialGradient::new(&id)
                    .cx(center[0])
                    .cy(center[1])
                    .r(radius);
                if let Some(rotation) = self.element_rotation {
                    gradient = gradient.transformed(rotation.inverse());
                }
                for stop in stops {
                    gradient = gradient.add(stop);
                }
                Definitions::new().add(gradient)
            }
        };
        scene.push(definition);
        Paint::Gradient(id)
    }
}

fn bounds(bbox: BoundingBox) -> [f32; 4] {
    [
        bbox.x_min as f32,
        bbox.y_min as f32,
        bbox.x_max as f32,
        bbox.y_max as f32,
    ]
}

fn project_stops(source: &ColorPaintSource) -> Result<Vec<ProjectedStop>, PaintDiagnosticKind> {
    let saved = &source.stops[..source.stops.len().min(10)];
    let Some(first) = saved.first() else {
        return Err(PaintDiagnosticKind::InvalidGradient);
    };
    if saved.len() == 1 {
        return Ok(vec![
            ProjectedStop {
                offset: 0.0,
                argb: first.argb,
            },
            ProjectedStop {
                offset: 1.0,
                argb: first.argb,
            },
        ]);
    }
    let mut previous = 0.0;
    for stop in saved {
        let offset = stop.position.value();
        if !offset.is_finite() || !(0.0..=1.0).contains(&offset) || offset < previous {
            return Err(PaintDiagnosticKind::InvalidGradient);
        }
        previous = offset;
    }
    let mut stops = Vec::with_capacity(saved.len() + 2);
    if first.position.value() != 0.0 {
        stops.push(ProjectedStop {
            offset: 0.0,
            argb: first.argb,
        });
    }
    for stop in saved {
        // FCVTZS #16 scales directly, without a rounded intermediate f32 product.
        let fixed = (f64::from(stop.position.value()) * 65536.0).trunc() as u32;
        stops.push(ProjectedStop {
            offset: f64::from(fixed) / 65536.0,
            argb: stop.argb,
        });
    }
    let last = saved.last().unwrap();
    if last.position.value() != 1.0 {
        stops.push(ProjectedStop {
            offset: 1.0,
            argb: last.argb,
        });
    }
    Ok(stops)
}

const ROTATION_GATE: f32 = f32::from_bits(0x34000000);

fn linear_points([left, top, right, bottom]: [f32; 4], angle: u16) -> [[f32; 2]; 2] {
    let half_width = (right - left) * 0.5;
    let half_height = (bottom - top) * 0.5;
    let cy = top + half_height;
    let diagonal = ((bottom - top) / (right - left)).atan() * 180.0 / f32::from_bits(0x40490fdb);
    let comparison = if angle > 179 { angle - 180 } else { angle };
    let radians = f32::from(angle % 180) * f32::from_bits(0x3c8efa35);
    let tangent = f64::from(radians).tan();
    let mut points = if diagonal > f32::from(comparison) {
        [
            [
                left,
                (-tangent).mul_add(f64::from(half_width), f64::from(cy)) as f32,
            ],
            [
                right,
                tangent.mul_add(f64::from(half_width), f64::from(cy)) as f32,
            ],
        ]
    } else if 180.0 - diagonal > f32::from(comparison) {
        let cx = left + half_width;
        let cotangent = 1.0 / tangent;
        [
            [
                (-cotangent).mul_add(f64::from(half_height), f64::from(cx)) as f32,
                top,
            ],
            [
                cotangent.mul_add(f64::from(half_height), f64::from(cx)) as f32,
                bottom,
            ],
        ]
    } else {
        [
            [
                right,
                tangent.mul_add(f64::from(half_width), f64::from(cy)) as f32,
            ],
            [
                left,
                (-tangent).mul_add(f64::from(half_width), f64::from(cy)) as f32,
            ],
        ]
    };
    if angle >= 180 {
        points.swap(0, 1);
    }
    points
}

fn radial_points(
    rect: [f32; 4],
    [px, py]: [f32; 2],
    rotation: f32,
    rotatable: bool,
) -> ([f32; 2], f32) {
    let width = rect[2] - rect[0];
    let height = rect[3] - rect[1];
    let mut position = [width.mul_add(px, rect[0]), height.mul_add(py, rect[1])];
    let mut radius = height.mul_add(height, width * width).sqrt();
    if (px - 0.5).abs() < ROTATION_GATE && (py - 0.5).abs() < ROTATION_GATE {
        radius *= 0.5;
    } else {
        let center = [(rect[0] + rect[2]) * 0.5, (rect[1] + rect[3]) * 0.5];
        position = rotate_point(position, center, rotation, rotatable);
    }
    (position, radius)
}

fn rotate_point(point: [f32; 2], center: [f32; 2], mut rotation: f32, rotatable: bool) -> [f32; 2] {
    if rotation < 0.0 {
        rotation += 360.0;
    }
    if !rotatable || rotation <= ROTATION_GATE {
        return point;
    }
    let radians = (f64::from(rotation) / 180.0) * f64::from_bits(0x400921fb54442d18);
    let (sin, cos) = radians.sin_cos();
    let dx = f64::from(point[0] - center[0]);
    let dy = f64::from(point[1] - center[1]);
    [
        (dx.mul_add(cos, -(sin * dy)) as f32) + center[0],
        (dx.mul_add(sin, cos * dy) as f32) + center[1],
    ]
}

#[cfg(test)]
mod tests;
