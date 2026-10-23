//! Bounded SVG projection of Drawing's shared fill/outline color-gradient math.
//! Source pins and execution limits: docs/reverse-engineering/shape-fill-findings.md.

use super::vector::{
    ColorValue, Definitions, Gradient, LinearGradient, Paint, RadialGradient, Scene, Stop,
    Transform,
};
use super::{PaintDiagnosticKind, PaintRole, RenderTheme};
use crate::{BoundingBox, Color, ColorPaintSource, NativeLine, NativeShape};

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
    /// Native captures passed the HF02 line's `reference_bbox` to Drawing and
    /// verified geometry at Skia's factories. For type 0 and type 1, rotatable
    /// flag 1 with zero effect angle and zero object rotation matches the
    /// nonrotatable control's helper/factory geometry and RGBA frame (linear
    /// V22 result `d30162b9379963344b37a58b18df6628dfdb0625737dd2d409d5c3f0f305ffd7`,
    /// radial V2 result
    /// `a715e5e069e797908213b0133fe4654e9472ab7dada40529770d40809d91910d`).
    /// These are synthetic-paint frame witnesses, not proof of saved archive
    /// paint or device pixels. The native source contract rotates linear
    /// endpoints when the outline rotatable flag is set; paired captures
    /// verify the route at 0/30 degrees. Source math defines the finite f32
    /// rotation path; radial metadata rotations remain on the fallback path.
    pub fn for_line(
        line: &NativeLine,
        source: &ColorPaintSource,
    ) -> Result<Self, PaintDiagnosticKind> {
        if source.outline_color_type != Some(1) || !source.trailing_data.is_empty() {
            return Err(PaintDiagnosticKind::UnsupportedPaint);
        }
        if !matches!(source.gradient_type, 0 | 1) || source.property_flags > 1 {
            return Err(PaintDiagnosticKind::UnsupportedGradientFrame);
        }
        let rect = bounds(line.reference_bbox);
        let width = rect[2] - rect[0];
        let height = rect[3] - rect[1];
        let object_rotation = line
            .metadata
            .rotation_degrees
            .map(|angle| angle as f32)
            .unwrap_or(0.0);
        if !object_rotation.is_finite() {
            return Err(PaintDiagnosticKind::UnrepresentableGradient);
        }
        let rotated_linear = object_rotation != 0.0 && source.gradient_type == 0;
        if object_rotation != 0.0 && (!rotated_linear || !source.gradient_rotatable()) {
            return Err(PaintDiagnosticKind::UnsupportedGradientFrame);
        }
        // Conservative eligibility limits: the native frame witness covers a
        // type-0, simple stroke; parser-marked routing/settings and compound,
        // dashed or arrowed variants have no paired gradient evidence.
        if line.line_type != 0
            || line.routing != 0
            || line.raw_setting != 0
            || line.style.compound != 0
            || line.style.dash != 0
            || line.style.cap > 2
            || line.style.join > 2
            || line.style.begin_arrow != [0; 2]
            || line.style.end_arrow != [0; 2]
            || !line.style.width.is_finite()
            || line.style.width <= 0.0
            || rect.iter().any(|value| !value.is_finite())
            || !width.is_finite()
            || !height.is_finite()
            || width <= 0.0
            || height <= 0.0
        {
            return Err(PaintDiagnosticKind::UnsupportedGradientFrame);
        }
        let geometry = if source.gradient_type == 0 {
            let mut points = linear_points(rect, source.linear_angle);
            if rotated_linear {
                let center = [rect[0] + width * 0.5, rect[1] + height * 0.5];
                points = points.map(|point| rotate_point(point, center, object_rotation, true));
            }
            if points.iter().flatten().any(|value| !value.is_finite()) || points[0] == points[1] {
                return Err(PaintDiagnosticKind::UnrepresentableGradient);
            }
            Geometry::Linear(points)
        } else {
            let (center, radius) = radial_points(
                rect,
                source.position.map(|value| value.value()),
                0.0,
                source.gradient_rotatable(),
            );
            if center.iter().any(|value| !value.is_finite()) || !radius.is_finite() || radius <= 0.0
            {
                return Err(PaintDiagnosticKind::UnrepresentableGradient);
            }
            Geometry::Radial { center, radius }
        };
        let stops = if source.gradient_type == 1 {
            project_radial_line_stops(source)?
        } else {
            project_linear_line_stops(source)?
        };
        Ok(Self {
            geometry,
            stops,
            element_rotation: None,
        })
    }

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
            || element_rotation.is_some_and(|rotation| {
                ![rotation.angle, rotation.center[0], rotation.center[1]]
                    .iter()
                    .all(|value| value.is_finite())
            })
        {
            return Err(PaintDiagnosticKind::UnrepresentableGradient);
        }
        let stops = match role {
            PaintRole::Fill => project_stops(source)?,
            PaintRole::Outline => project_outline_stops(source)?,
        };
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
    let mut descending = false;
    for stop in saved {
        let offset = stop.position.value();
        if !offset.is_finite() || !(0.0..=1.0).contains(&offset) {
            return Err(PaintDiagnosticKind::InvalidGradient);
        }
        descending |= offset < previous;
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
    if descending {
        // The pinned Skia notes establish source order and skipping of
        // non-increasing quantized spans. A final positive [0, 1] span is the
        // only descending shape projected here; partial overlap depends on
        // discrete cache-index/interpolation details, so keep rejecting it.
        // This is not a claim of native cache or pixel equality.
        let last_positive = stops
            .windows(2)
            .rposition(|pair| pair[1].offset > pair[0].offset);
        let Some(index) = last_positive else {
            return Err(PaintDiagnosticKind::InvalidGradient);
        };
        let pair = &stops[index..=index + 1];
        if pair[0].offset == 0.0 && pair[1].offset == 1.0 {
            return Ok(vec![
                ProjectedStop {
                    offset: pair[0].offset,
                    argb: pair[0].argb,
                },
                ProjectedStop {
                    offset: pair[1].offset,
                    argb: pair[1].argb,
                },
            ]);
        }
        return Err(PaintDiagnosticKind::InvalidGradient);
    }
    Ok(stops)
}

/// Drawing's type-6 line-color copy path stores outline stops in descending
/// position order before its ten-stop consumer runs. Type-7 shape outlines use
/// that same copy path; fills have different semantics and remain source-order.
fn project_outline_stops(
    source: &ColorPaintSource,
) -> Result<Vec<ProjectedStop>, PaintDiagnosticKind> {
    let stops = normalize_outline_stops(source)?;
    project_normalized_outline_stops(source, stops)
}

/// Project a bounded descending two-stop linear line gradient. The native
/// cache endpoint-extends the first/last stops, skips the nonpositive middle
/// span, then lets the final constant-color span overwrite from the low
/// position's Q16.16 cache bucket. Stop alpha changes cached colors, not the
/// index/span arithmetic.
fn project_linear_line_stops(
    source: &ColorPaintSource,
) -> Result<Vec<ProjectedStop>, PaintDiagnosticKind> {
    let stops = normalize_outline_stops(source)?;
    if let [high, low] = stops.as_slice()
        && high.position.value() > low.position.value()
    {
        let cache_index = gradient_cache_index(low.position.value());
        if cache_index == 0 {
            return Ok(vec![
                ProjectedStop {
                    offset: 0.0,
                    argb: low.argb,
                },
                ProjectedStop {
                    offset: 1.0,
                    argb: low.argb,
                },
            ]);
        }
        let boundary = f64::from(cache_index) / 256.0;
        return Ok(vec![
            ProjectedStop {
                offset: 0.0,
                argb: high.argb,
            },
            ProjectedStop {
                offset: boundary,
                argb: high.argb,
            },
            ProjectedStop {
                offset: boundary,
                argb: low.argb,
            },
            ProjectedStop {
                offset: 1.0,
                argb: low.argb,
            },
        ]);
    }
    project_normalized_outline_stops(source, stops)
}

fn project_normalized_outline_stops(
    source: &ColorPaintSource,
    stops: Vec<crate::GradientStopSource>,
) -> Result<Vec<ProjectedStop>, PaintDiagnosticKind> {
    let mut normalized = source.clone();
    normalized.stops = stops;
    project_stops(&normalized)
}

/// Project the recovered two-stop type-8 radial line cache. Skia keeps the
/// descending stop order, endpoint-extends the first color to zero and the
/// last color to one, then writes increasing cache spans in order. For cache
/// indices 1..255, the later span overwrites at its quantized index. The V41
/// and V42 actual 256x4 N32 cache reads show that index 0 yields the
/// low-position stop throughout the table for an opaque stop and for a
/// nonprimary partial-alpha stop. Native V25-V26 and V34-V38 controls cover
/// nonzero indices with opaque, partial-alpha, zero-alpha and nonprimary RGB.
/// These controls do not prove arbitrary source or device pixel parity. Keep
/// fills, shape outlines, linear paint and gradients with more than two
/// consumed stops on their existing paths.
fn project_radial_line_stops(
    source: &ColorPaintSource,
) -> Result<Vec<ProjectedStop>, PaintDiagnosticKind> {
    let stops = normalize_outline_stops(source)?;
    if stops.len() != 2 {
        return project_normalized_outline_stops(source, stops);
    }

    let [high, low] = stops.as_slice() else {
        unreachable!("the stop count was checked")
    };
    let high_position = high.position.value();
    let low_position = low.position.value();
    if high_position <= low_position || high.argb == low.argb {
        return project_normalized_outline_stops(source, stops);
    }

    let cache_index = gradient_cache_index(low_position);
    if cache_index == 0 {
        return Ok(vec![
            ProjectedStop {
                offset: 0.0,
                argb: low.argb,
            },
            ProjectedStop {
                offset: 1.0,
                argb: low.argb,
            },
        ]);
    }
    if !(1..256).contains(&cache_index) {
        return project_normalized_outline_stops(source, stops);
    }
    let boundary = f64::from(cache_index) / 256.0;
    Ok(vec![
        ProjectedStop {
            offset: 0.0,
            argb: high.argb,
        },
        ProjectedStop {
            offset: boundary,
            argb: high.argb,
        },
        ProjectedStop {
            offset: boundary,
            argb: low.argb,
        },
        ProjectedStop {
            offset: 1.0,
            argb: low.argb,
        },
    ])
}

/// Return the 256-entry Skia gradient-cache index for a finite position in
/// `[0, 1]`. Skia first converts to Q16.16, then removes the integer component
/// and shifts by eight. The subtraction makes the endpoint `1.0` map to the
/// last cache entry (`255`) instead of an out-of-range index (`256`).
fn gradient_cache_index(position: f32) -> u32 {
    let fixed = (f64::from(position) * 65536.0).trunc() as u32;
    ((fixed - (fixed >> 16)) >> 8).min(255)
}

fn normalize_outline_stops(
    source: &ColorPaintSource,
) -> Result<Vec<crate::GradientStopSource>, PaintDiagnosticKind> {
    if source.stops.len() > usize::from(u8::MAX) {
        return Err(PaintDiagnosticKind::InvalidGradient);
    }
    if source.stops.is_empty() {
        return Err(PaintDiagnosticKind::InvalidGradient);
    }
    if source.stops.len() == 1 {
        // The native singleton factory duplicates its color and ignores its
        // position; retain the existing behavior for this proven special case.
        return Ok(source.stops.clone());
    }

    for stop in &source.stops {
        let position = stop.position.value();
        if !position.is_finite() || !(0.0..=1.0).contains(&position) {
            return Err(PaintDiagnosticKind::InvalidGradient);
        }
    }

    let mut order = (0..source.stops.len()).collect::<Vec<_>>();
    order.sort_by(|left, right| {
        source.stops[*right]
            .position
            .value()
            .partial_cmp(&source.stops[*left].position.value())
            .expect("all line stop positions were validated as finite")
    });

    // Equal-position ordering is not pinned. If differently colored tied
    // stops could enter the native ten-stop prefix, fail closed rather than
    // invent a tie-break. Identical colors and ties wholly beyond the prefix
    // retain their original multiplicity/order.
    let consumed = order.len().min(10);
    let mut start = 0;
    while start < consumed {
        let position = source.stops[order[start]].position.value();
        let mut end = start + 1;
        while end < order.len() && source.stops[order[end]].position.value() == position {
            end += 1;
        }
        if end > start + 1 {
            let color = source.stops[order[start]].argb;
            if order[start..end]
                .iter()
                .any(|index| source.stops[*index].argb != color)
            {
                return Err(PaintDiagnosticKind::InvalidGradient);
            }
        }
        start = end;
    }

    Ok(order
        .into_iter()
        .map(|index| source.stops[index].clone())
        .collect())
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
pub(super) mod tests;
