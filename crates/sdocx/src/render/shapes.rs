use super::vector::{
    self, Data, Ellipse, Group, Line, LineCap, LineJoin, Paint, Path, Polygon, Rectangle, Scene,
    Styled, Transform, decimal,
};
use super::{
    GeometryDiagnosticKind, PaintDiagnostic, PaintDiagnosticKind, PaintRole, RenderTheme, arrows,
    gradient,
};
use crate::Color;

// Both shape and line writers serialize the drawing path in page coordinates,
// including rotation. Reject an unsupported path as a whole, never draw a prefix.
pub(super) fn native_svg_path(bytes: &[u8]) -> Result<Data, GeometryDiagnosticKind> {
    let mut commands = Vec::new();
    let parsed = crate::shape::native_path::visit_drawing_path(bytes, |command| {
        commands.push(command);
    });
    let (size, supported) = parsed.map_err(|_| GeometryDiagnosticKind::InvalidPath)?;
    if size != bytes.len() {
        return Err(GeometryDiagnosticKind::InvalidPath);
    }
    if !supported {
        return Err(GeometryDiagnosticKind::UnsupportedPath);
    }
    native_svg_commands(commands)
}

fn native_svg_commands(
    commands: impl IntoIterator<Item = crate::shape::NativePathCommand>,
) -> Result<Data, GeometryDiagnosticKind> {
    use crate::shape::NativePathCommand;

    fn coordinates<const N: usize>(values: [f64; N]) -> Option<[f32; N]> {
        let values = values.map(|value| value as f32);
        values
            .iter()
            .all(|value| value.is_finite())
            .then_some(values)
    }

    let mut prepared = Vec::new();
    let mut contour_start = None;
    let mut after_close = false;
    for command in commands {
        match command {
            NativePathCommand::Move(point) => {
                contour_start = Some(point);
                after_close = false;
                prepared.push(NativePathCommand::Move(point));
            }
            NativePathCommand::Close => {
                if contour_start.is_some() && !after_close {
                    prepared.push(NativePathCommand::Close);
                    after_close = true;
                }
            }
            draw @ (NativePathCommand::Line(_)
            | NativePathCommand::Quadratic(_)
            | NativePathCommand::Cubic(_)) => {
                if after_close {
                    prepared.push(NativePathCommand::Move(
                        contour_start.expect("closed contour"),
                    ));
                    after_close = false;
                } else if contour_start.is_none() {
                    contour_start = Some([0.0, 0.0]);
                    prepared.push(NativePathCommand::Move([0.0, 0.0]));
                }
                prepared.push(draw);
            }
            primitive @ (NativePathCommand::Arc(_) | NativePathCommand::Oval(_)) => {
                prepared.push(primitive);
            }
        }
    }
    let mut data = Data::new();
    for command in prepared {
        data = match command {
            NativePathCommand::Move(point) => {
                let [x, y] =
                    coordinates(point).ok_or(GeometryDiagnosticKind::UnrepresentablePath)?;
                data.move_to((x, y))
            }
            NativePathCommand::Line(point) => {
                let [x, y] =
                    coordinates(point).ok_or(GeometryDiagnosticKind::UnrepresentablePath)?;
                data.line_to((x, y))
            }
            NativePathCommand::Quadratic([[x1, y1], [x, y]]) => {
                let [x1, y1, x, y] = coordinates([x1, y1, x, y])
                    .ok_or(GeometryDiagnosticKind::UnrepresentablePath)?;
                data.quadratic_curve_to((x1, y1, x, y))
            }
            NativePathCommand::Cubic([[x1, y1], [x2, y2], [x, y]]) => {
                let [x1, y1, x2, y2, x, y] = coordinates([x1, y1, x2, y2, x, y])
                    .ok_or(GeometryDiagnosticKind::UnrepresentablePath)?;
                data.cubic_curve_to((x1, y1, x2, y2, x, y))
            }
            NativePathCommand::Close => data.close(),
            NativePathCommand::Arc(_) | NativePathCommand::Oval(_) => {
                return Err(GeometryDiagnosticKind::UnsupportedPath);
            }
        };
    }
    Ok(data)
}

// Native Drawing records a center-closed fill and open outline for captured
// unrotated Arc controls. Require the reconstructed outline to match the saved
// path exactly; unsupported styles and nonmatching geometry use the fallback.
fn matching_rotated_arc_sector(
    shape: &crate::NativeShape,
    commands: &[crate::shape::NativePathCommand],
    bounds: [f32; 4],
) -> Option<Data> {
    use crate::shape::NativePathCommand as C;
    if !shape.rotation_degrees.is_finite() {
        return None;
    }
    let [l, t, r, b] = bounds;
    let cx = (l + r) * 0.5;
    let cy = (t + b) * 0.5;
    let mut base = crate::shape::native_path::normalize_model_path(
        &shape.path_data,
        bounds.map(f64::from),
        0.,
        false,
        false,
        true,
    )
    .ok()??
    .commands;
    crate::shape::native_path::rotate_model_commands(&mut base, bounds, -shape.rotation_degrees);
    let angle = |p: [f64; 2]| {
        let x = p[0] as f32 - cx;
        let y = p[1] as f32 - cy;
        if !x.is_finite() || !y.is_finite() || (x == 0. && y == 0.) {
            None
        } else {
            Some(crate::shape::model_arc::angle_from_vector(x, y))
        }
    };
    let first = match base.first()? {
        C::Move(p) => *p,
        _ => return None,
    };
    let last = match base.last()? {
        C::Quadratic([_, p]) => *p,
        _ => return None,
    };
    if !base[1..].iter().all(|c| matches!(c, C::Quadratic(_))) {
        return None;
    }
    let mut angles = [angle(first)?, angle(last)?];
    let mut controls = shape
        .control_points
        .iter()
        .map(|p| C::Move(*p))
        .collect::<Vec<_>>();
    crate::shape::native_path::rotate_model_commands(
        &mut controls,
        bounds,
        -shape.rotation_degrees,
    );
    for (i, c) in controls.iter().enumerate() {
        let C::Move(p) = c else { return None };
        angles[i] = angle(*p)?;
        let mut sweep = angles[1] - angles[0];
        if sweep < 0. {
            sweep += 360.;
        }
        if sweep == 0. {
            return None;
        }
        let points = crate::shape::model_arc::expand([l, t, r, b, angles[0], sweep])?;
        if points.len() != base.len() * 2 - 1 {
            return None;
        }
        // The stricter margin is heuristic and does not bound Android math drift.
        let same = |a: f64, b: f32| {
            let a = a as f32;
            a.is_finite() && b.is_finite() && f64::from((a - b).abs()) < 0.000025
        };
        if !matches!(&base[0],C::Move(p) if same(p[0],points[0][0]) && same(p[1],points[0][1])) {
            return None;
        }
        for (c, p) in base[1..].iter().zip(points[1..].as_chunks::<2>().0.iter()) {
            if !matches!(c,C::Quadratic(q) if same(q[0][0],p[0][0])&&same(q[0][1],p[0][1])&&same(q[1][0],p[1][0])&&same(q[1][1],p[1][1]))
            {
                return None;
            }
        }
    }
    let mut fill = commands.to_vec();
    fill.push(C::Line([f64::from(cx), f64::from(cy)]));
    fill.push(C::Close);
    native_svg_commands(fill).ok()
}

fn matching_arc_sector(
    shape: &crate::NativeShape,
    commands: &[crate::shape::NativePathCommand],
    rotated_arc_sector_approximation: bool,
) -> Option<Data> {
    use crate::shape::NativePathCommand;
    if shape.shape_type != 20
        || shape.horizontal_flip
        || shape.vertical_flip
        || !matches!(shape.fill, crate::ShapePaint::Solid(_))
        || !matches!(shape.style.paint, crate::ShapePaint::Solid(_))
        || !shape.style.width.is_finite()
        || shape.style.width <= 0.0
        || shape.style.compound != 0
        || shape.style.dash_pattern().is_err()
        || shape.style.cap > 2
        || shape.style.join > 2
        || shape.style.begin_arrow != [0; 2]
        || shape.style.end_arrow != [0; 2]
    {
        return None;
    }
    let owner = shape.metadata.bbox;
    let drawn = shape.drawn_bbox;
    let owner = [owner.x_min, owner.y_min, owner.x_max, owner.y_max].map(|x| x as f32);
    let drawn = [drawn.x_min, drawn.y_min, drawn.x_max, drawn.y_max].map(|x| x as f32);
    if owner.iter().any(|x| !x.is_finite())
        || owner.map(f32::to_bits) != drawn.map(f32::to_bits)
        || shape
            .metadata
            .rotation_degrees
            .is_some_and(|angle| angle != 0.0)
    {
        return None;
    }
    let bounds = [
        shape.geometry_bbox.x_min,
        shape.geometry_bbox.y_min,
        shape.geometry_bbox.x_max,
        shape.geometry_bbox.y_max,
    ]
    .map(|x| x as f32);
    let [left, top, right, bottom] = bounds;
    if bounds.iter().any(|x| !x.is_finite()) || left >= right || top >= bottom {
        return None;
    }
    let cx = (left + right) * 0.5;
    let cy = (top + bottom) * 0.5;
    if !cx.is_finite() || !cy.is_finite() || shape.control_points.len() != 2 {
        return None;
    }
    if shape.rotation_degrees != 0.0 {
        if !rotated_arc_sector_approximation {
            return None;
        }
        return matching_rotated_arc_sector(shape, commands, bounds);
    }
    let mut angles = [0.0_f32; 2];
    for (angle, point) in angles.iter_mut().zip(&shape.control_points) {
        let x = point[0] as f32 - cx;
        let y = point[1] as f32 - cy;
        if !x.is_finite() || !y.is_finite() || (x == 0.0 && y == 0.0) {
            return None;
        }
        *angle = libm::atan2f(y, x).to_degrees().rem_euclid(360.0);
    }
    let sweep = (angles[1] - angles[0]).rem_euclid(360.0);
    if sweep == 0.0 {
        return None;
    }
    let points = crate::shape::model_arc::expand([left, top, right, bottom, angles[0], sweep])?;
    let mut expected = vec![NativePathCommand::Move(points.first()?.map(f64::from))];
    for pair in points[1..].as_chunks::<2>().0 {
        expected.push(NativePathCommand::Quadratic([
            pair[0].map(f64::from),
            pair[1].map(f64::from),
        ]));
    }
    if commands != expected {
        return None;
    }
    let mut fill = expected;
    fill.push(NativePathCommand::Line([f64::from(cx), f64::from(cy)]));
    fill.push(NativePathCommand::Close);
    native_svg_commands(fill).ok()
}

enum PreparedShapeGeometry {
    SavedPath {
        outline: Data,
        sector_fill: Option<Data>,
    },
    Ellipse {
        body: Ellipse,
        rotation: gradient::ElementRotation,
    },
    Rectangle {
        body: Rectangle,
        rotation: gradient::ElementRotation,
    },
    Polygon {
        body: Polygon,
        rotation: gradient::ElementRotation,
    },
}

impl PreparedShapeGeometry {
    fn paint_rotation(&self) -> Option<gradient::ElementRotation> {
        match self {
            Self::SavedPath { .. } => None,
            Self::Ellipse { rotation, .. }
            | Self::Rectangle { rotation, .. }
            | Self::Polygon { rotation, .. } => Some(*rotation),
        }
    }
}

fn prepare_shape_geometry(
    shape: &crate::NativeShape,
    rotated_arc_sector_approximation: bool,
) -> Result<PreparedShapeGeometry, GeometryDiagnosticKind> {
    if !shape.path_data.is_empty() {
        let (consumed, _) = crate::shape::visit_path(&shape.path_data, |_| {})
            .map_err(|_| GeometryDiagnosticKind::InvalidPath)?;
        if consumed != shape.path_data.len() {
            return Err(GeometryDiagnosticKind::InvalidPath);
        }
        let normalized = crate::shape::native_path::normalize_model_path(
            &shape.path_data,
            [
                shape.geometry_bbox.x_min,
                shape.geometry_bbox.y_min,
                shape.geometry_bbox.x_max,
                shape.geometry_bbox.y_max,
            ],
            shape.rotation_degrees,
            shape.horizontal_flip,
            shape.vertical_flip,
            true,
        )
        .map_err(|_| GeometryDiagnosticKind::InvalidPath)?;
        let Some(normalized) = normalized else {
            let mut narrowed_finite = true;
            crate::shape::visit_path(&shape.path_data, |command| {
                use crate::shape::NativePathCommand;
                let values: Vec<f64> = match command {
                    NativePathCommand::Move([x, y]) | NativePathCommand::Line([x, y]) => {
                        vec![x, y]
                    }
                    NativePathCommand::Quadratic([[x1, y1], [x, y]]) => vec![x1, y1, x, y],
                    NativePathCommand::Cubic([[x1, y1], [x2, y2], [x, y]]) => {
                        vec![x1, y1, x2, y2, x, y]
                    }
                    NativePathCommand::Arc(values) => values.to_vec(),
                    NativePathCommand::Oval(values) => values.to_vec(),
                    NativePathCommand::Close => Vec::new(),
                };
                narrowed_finite &= values.iter().all(|value| (*value as f32).is_finite());
            })
            .map_err(|_| GeometryDiagnosticKind::InvalidPath)?;
            return Err(if narrowed_finite {
                GeometryDiagnosticKind::UnsupportedPath
            } else {
                GeometryDiagnosticKind::UnrepresentablePath
            });
        };
        let sector_fill = matching_arc_sector(
            shape,
            &normalized.commands,
            rotated_arc_sector_approximation,
        );
        return Ok(PreparedShapeGeometry::SavedPath {
            outline: native_svg_commands(normalized.commands)?,
            sector_fill,
        });
    }

    let bbox = shape.geometry_bbox;
    let width = bbox.x_max - bbox.x_min;
    let height = bbox.y_max - bbox.y_min;
    let cx = bbox.x_min + width / 2.0;
    let cy = bbox.y_min + height / 2.0;
    if ![bbox.x_min, bbox.y_min, width, height, cx, cy]
        .iter()
        .all(|v| v.is_finite())
        || width <= 0.0
        || height <= 0.0
        || !shape.rotation_degrees.is_finite()
    {
        return Err(GeometryDiagnosticKind::InvalidGeometry);
    }
    let rotation = gradient::ElementRotation {
        angle: f64::from(shape.rotation_degrees),
        center: [cx, cy],
    };
    let points = match shape.shape_type {
        1 => {
            return Ok(PreparedShapeGeometry::Ellipse {
                body: Ellipse::new()
                    .cx(cx)
                    .cy(cy)
                    .rx(width / 2.0)
                    .ry(height / 2.0),
                rotation,
            });
        }
        2 => vec![
            (cx, bbox.y_min),
            (bbox.x_max, bbox.y_max),
            (bbox.x_min, bbox.y_max),
        ],
        3 => vec![
            (bbox.x_min, bbox.y_min),
            (bbox.x_max, bbox.y_max),
            (bbox.x_min, bbox.y_max),
        ],
        4 => {
            return Ok(PreparedShapeGeometry::Rectangle {
                body: Rectangle::new()
                    .x(bbox.x_min)
                    .y(bbox.y_min)
                    .width(width)
                    .height(height),
                rotation,
            });
        }
        8 => vec![
            (cx, bbox.y_min),
            (bbox.x_max, cy),
            (cx, bbox.y_max),
            (bbox.x_min, cy),
        ],
        _ => return Err(GeometryDiagnosticKind::UnsupportedShapeTemplate),
    };
    Ok(PreparedShapeGeometry::Polygon {
        body: Polygon::new().points_unrounded(&points),
        rotation,
    })
}

pub(super) fn render_shape(
    svg: &mut Scene,
    shape: &crate::NativeShape,
    theme: RenderTheme,
    source_offset: Option<usize>,
    rotated_arc_sector_approximation: bool,
) -> Result<(), GeometryDiagnosticKind> {
    // Validate all path/primitive geometry before paint definitions allocate IDs.
    let geometry = prepare_shape_geometry(shape, rotated_arc_sector_approximation)?;
    let rotation = geometry.paint_rotation();
    let (fill, opacity) =
        render_shape_paint(svg, shape, PaintRole::Fill, rotation, theme, source_offset);
    let outline = render_shape_paint(
        svg,
        shape,
        PaintRole::Outline,
        rotation,
        theme,
        source_offset,
    );
    let (body, separated_fill): (vector::Node, bool) = match geometry {
        PreparedShapeGeometry::SavedPath {
            outline,
            sector_fill,
        } => {
            let separated_fill = sector_fill.is_some();
            if let Some(fill_path) = sector_fill {
                svg.push(
                    Path::new()
                        .data(fill_path)
                        .fill(fill)
                        .fill_opacity(decimal(opacity, 4))
                        .stroke(Paint::None),
                );
            }
            (Path::new().data(outline).into(), separated_fill)
        }
        PreparedShapeGeometry::Ellipse { body, .. } => (body.into(), false),
        PreparedShapeGeometry::Rectangle { body, .. } => (body.into(), false),
        PreparedShapeGeometry::Polygon { body, .. } => (body.into(), false),
    };
    let mut style = shape_outline(
        svg,
        &shape.style,
        outline,
        source_offset,
        &shape.metadata.uuid,
    )
    .fill(if separated_fill { Paint::None } else { fill })
    .fill_opacity(decimal(opacity, 4));
    if let Some(rotation) = rotation {
        style = style.transformed(Transform::rotate_unrounded(
            rotation.angle,
            rotation.center[0],
            rotation.center[1],
        ));
    }
    svg.push(style.add(body));
    Ok(())
}

pub(super) fn render_line(
    svg: &mut Scene,
    line: &crate::NativeLine,
    theme: RenderTheme,
    source_offset: Option<usize>,
) -> Result<(), GeometryDiagnosticKind> {
    // Serialized endpoints already include the native rotation.
    if line.line_type > 2 {
        return Err(GeometryDiagnosticKind::UnsupportedLineType);
    }
    if line.begin.iter().chain(&line.end).any(|v| !v.is_finite()) {
        return Err(GeometryDiagnosticKind::InvalidGeometry);
    }
    let path = if !line.path_data.is_empty() {
        Some(native_svg_path(&line.path_data)?)
    } else if line.line_type == 0 {
        None
    } else {
        return Err(GeometryDiagnosticKind::MissingLinePath);
    };
    let outline = project_line_paint(svg, line, theme).unwrap_or_else(|kind| {
        svg.report_paint_issue(PaintDiagnostic {
            source_offset,
            object_uuid: line.metadata.uuid.clone(),
            role: PaintRole::Outline,
            kind,
        });
        (Paint::None, 0.0)
    });
    let style = shape_outline(
        svg,
        &line.style,
        outline,
        source_offset,
        &line.metadata.uuid,
    )
    .fill(Paint::None);
    let body = if let Some(path) = path {
        style.add(Path::new().data(path))
    } else {
        style.add(
            Line::new()
                .x1(line.begin[0])
                .y1(line.begin[1])
                .x2(line.end[0])
                .y2(line.end[1]),
        )
    };
    if !matches!(outline.0, Paint::None) && line.style.dash_pattern().is_ok() {
        match arrows::Plan::for_line(line) {
            Ok(Some(arrows)) => arrows.render(svg, outline.0, outline.1, body),
            Ok(None) => svg.push(body),
            Err(()) => {
                svg.report_paint_issue(PaintDiagnostic {
                    source_offset,
                    object_uuid: line.metadata.uuid.clone(),
                    role: PaintRole::Outline,
                    kind: PaintDiagnosticKind::UnsupportedArrowGeometry,
                });
                svg.push(body);
            }
        }
    } else {
        svg.push(body);
    }
    Ok(())
}

pub(super) fn shape_paint(paint: &crate::ShapePaint, theme: RenderTheme) -> (String, f64) {
    match paint {
        crate::ShapePaint::Solid(argb) => {
            let color = Color {
                r: (argb >> 16) as u8,
                g: (argb >> 8) as u8,
                b: *argb as u8,
            };
            let color = theme.foreground(Some(color));
            (color, f64::from((argb >> 24) as u8) / 255.0)
        }
        _ => ("none".into(), 0.0),
    }
}

fn render_shape_paint(
    svg: &mut Scene,
    shape: &crate::NativeShape,
    role: PaintRole,
    rotation: Option<gradient::ElementRotation>,
    theme: RenderTheme,
    source_offset: Option<usize>,
) -> (Paint, f64) {
    let (paint, source) = match role {
        PaintRole::Fill => (&shape.fill, shape.fill_source.as_ref()),
        PaintRole::Outline => (&shape.style.paint, shape.style.paint_source.as_ref()),
    };
    project_shape_paint(svg, paint, source, Some((shape, rotation)), role, theme).unwrap_or_else(
        |kind| {
            svg.report_paint_issue(PaintDiagnostic {
                source_offset,
                object_uuid: shape.metadata.uuid.clone(),
                role,
                kind,
            });
            (Paint::None, 0.0)
        },
    )
}

fn project_line_paint(
    svg: &mut Scene,
    line: &crate::NativeLine,
    theme: RenderTheme,
) -> Result<(Paint, f64), PaintDiagnosticKind> {
    match &line.style.paint {
        crate::ShapePaint::Gradient => {
            let Some(crate::ShapePaintSource::Color(source)) = line.style.paint_source.as_ref()
            else {
                return Err(PaintDiagnosticKind::MissingPaintSource);
            };
            Ok((
                gradient::Plan::for_line(line, source)?.paint(svg, theme),
                1.0,
            ))
        }
        paint => project_shape_paint(
            svg,
            paint,
            line.style.paint_source.as_ref(),
            None,
            PaintRole::Outline,
            theme,
        ),
    }
}

fn project_shape_paint(
    svg: &mut Scene,
    paint: &crate::ShapePaint,
    source: Option<&crate::ShapePaintSource>,
    frame: Option<(&crate::NativeShape, Option<gradient::ElementRotation>)>,
    role: PaintRole,
    theme: RenderTheme,
) -> Result<(Paint, f64), PaintDiagnosticKind> {
    match paint {
        crate::ShapePaint::None => Ok((Paint::None, 0.0)),
        crate::ShapePaint::Solid(_) => {
            let (color, opacity) = shape_paint(paint, theme);
            Ok((Paint::from_hex(&color).unwrap(), opacity))
        }
        crate::ShapePaint::Gradient => {
            let Some(crate::ShapePaintSource::Color(source)) = source else {
                return Err(PaintDiagnosticKind::MissingPaintSource);
            };
            let Some((shape, rotation)) = frame else {
                return Err(PaintDiagnosticKind::UnsupportedGradientFrame);
            };
            let plan = gradient::Plan::for_shape(shape, source, role, rotation)?;
            Ok((plan.paint(svg, theme), 1.0))
        }
        crate::ShapePaint::Unsupported { .. } => Err(PaintDiagnosticKind::UnsupportedPaint),
    }
}

fn shape_outline(
    svg: &mut Scene,
    style: &crate::ShapeStyle,
    (mut paint, opacity): (Paint, f64),
    source_offset: Option<usize>,
    object_uuid: &str,
) -> Group {
    let dash = if matches!(paint, Paint::None) {
        None
    } else {
        style.dash_pattern().unwrap_or_else(|()| {
            svg.report_paint_issue(PaintDiagnostic {
                source_offset,
                object_uuid: object_uuid.into(),
                role: PaintRole::Outline,
                kind: PaintDiagnosticKind::UnsupportedDashPattern,
            });
            paint = Paint::None;
            None
        })
    };
    let cap = match style.cap {
        1 => LineCap::Round,
        2 => LineCap::Square,
        _ => LineCap::Butt,
    };
    let join = match style.join {
        1 => LineJoin::Round,
        2 => LineJoin::Bevel,
        _ => LineJoin::Miter,
    };
    let width = if style.width.is_finite() {
        style.width.max(0.0)
    } else {
        0.0
    };
    let group = Group::new()
        .stroke(paint)
        .stroke_opacity(decimal(opacity, 4))
        .stroke_width(width)
        .line_cap(cap)
        .line_join(join);
    match dash {
        Some(dash) => group.stroke_dash(dash.intervals(), dash.phase),
        None => group,
    }
}
