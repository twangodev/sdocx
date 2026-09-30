use crate::NativeShape;
use crate::shape::NativePathCommand;

use super::native::{NativeRect, TemplatePath, clamp_horizontal};
use super::{ShapeTextFrameIssue, TextInsets};

pub(super) fn insets(
    shape: &NativeShape,
    rect: &NativeRect,
) -> Result<TextInsets, ShapeTextFrameIssue> {
    let path = TemplatePath::read(shape, rect)?;
    match shape.shape_type {
        11 => pentagon_insets(vertices(path.commands())?, rect),
        6 => {
            let mut points = vertices(path.commands())?;
            for &control in &shape.control_points {
                let control = rect.restore_point(control, shape.rotation_degrees)?;
                points = apply_hexagon_control(
                    points,
                    control,
                    rect,
                    shape.horizontal_flip,
                    shape.vertical_flip,
                )?;
            }
            hexagon_insets(points, rect)
        }
        _ => Err(ShapeTextFrameIssue::UnsupportedTemplate),
    }
}

fn vertices<const N: usize>(
    commands: &[NativePathCommand<f32>],
) -> Result<[[f32; 2]; N], ShapeTextFrameIssue> {
    if commands.len() != N + 1 || commands.last() != Some(&NativePathCommand::Close) {
        return Err(ShapeTextFrameIssue::UnsupportedTemplate);
    }
    let mut points = [[0.0; 2]; N];
    for (index, (point, command)) in points.iter_mut().zip(commands).enumerate() {
        *point = match command {
            NativePathCommand::Move(point) if index == 0 => *point,
            NativePathCommand::Line(point) if index != 0 => *point,
            _ => return Err(ShapeTextFrameIssue::UnsupportedTemplate),
        };
        if !point.iter().all(|coordinate| coordinate.is_finite()) {
            return Err(ShapeTextFrameIssue::InvalidGeometry);
        }
    }
    Ok(points)
}

fn pentagon_insets(
    points: [[f32; 2]; 5],
    rect: &NativeRect,
) -> Result<TextInsets, ShapeTextFrameIssue> {
    let horizontal = if points[4][0] > points[1][0] {
        rect.right - points[3][0]
    } else {
        points[3][0] - rect.left
    };
    let vertical = (rect.height * 0.5) * 0.5;
    let (top, bottom) = if points[0][1] < points[2][1] {
        (vertical, 0.0)
    } else {
        (0.0, vertical)
    };
    validated(TextInsets {
        left: horizontal,
        top,
        right: horizontal,
        bottom,
    })
}

fn apply_hexagon_control(
    points: [[f32; 2]; 6],
    control: [f32; 2],
    rect: &NativeRect,
    horizontal_flip: bool,
    vertical_flip: bool,
) -> Result<[[f32; 2]; 6], ShapeTextFrameIssue> {
    let from_right = points[5][0] > points[2][0];
    let y = if points[0][1] > points[4][1] {
        rect.bottom
    } else {
        rect.top
    };
    let edge = if from_right { rect.right } else { rect.left };
    let control = clamp_horizontal(control, [edge, y], [rect.center[0], y])?;
    let half_width = rect.width * 0.5;
    let distance = if from_right {
        rect.right - control[0]
    } else {
        control[0] - rect.left
    };
    let control_x = if half_width < distance {
        rect.center[0]
    } else {
        control[0]
    };
    let distance = if from_right {
        rect.right - control_x
    } else {
        control_x - rect.left
    };
    let (first_x, second_x, side_x, other_side_x) = if horizontal_flip {
        (
            rect.right - distance,
            rect.left + distance,
            rect.left,
            rect.right,
        )
    } else {
        (
            rect.left + distance,
            rect.right - distance,
            rect.right,
            rect.left,
        )
    };
    let half_height = rect.height * 0.5;
    let (first_y, second_y, other_middle_y) = if vertical_flip {
        (rect.bottom, rect.top, rect.bottom - half_height)
    } else {
        (rect.top, rect.bottom, rect.top + half_height)
    };
    let points = [
        [first_x, first_y],
        [second_x, first_y],
        [side_x, rect.top + half_height],
        [second_x, second_y],
        [first_x, second_y],
        [other_side_x, other_middle_y],
    ];
    if !points
        .iter()
        .flatten()
        .all(|coordinate| coordinate.is_finite())
    {
        return Err(ShapeTextFrameIssue::InvalidGeometry);
    }
    Ok(points)
}

fn hexagon_insets(
    points: [[f32; 2]; 6],
    rect: &NativeRect,
) -> Result<TextInsets, ShapeTextFrameIssue> {
    let distance = if points[5][0] >= points[2][0] {
        rect.right - points[0][0]
    } else {
        points[0][0] - rect.left
    };
    let half_height = rect.height * 0.5;
    let half_width = rect.width * 0.5;
    let vertical_adjustment = (((half_height * 80.0) / 200.0) * distance) / half_width;
    let horizontal_adjustment = (((half_width * 80.0) / 200.0) * distance) / half_width;
    let horizontal = ((half_width * 35.0) / 200.0) + horizontal_adjustment;
    let vertical = ((half_height * 35.0) / 200.0) + vertical_adjustment;
    validated(TextInsets {
        left: horizontal,
        top: vertical,
        right: horizontal,
        bottom: vertical,
    })
}

fn validated(insets: TextInsets) -> Result<TextInsets, ShapeTextFrameIssue> {
    [insets.left, insets.top, insets.right, insets.bottom]
        .into_iter()
        .all(f32::is_finite)
        .then_some(insets)
        .ok_or(ShapeTextFrameIssue::InvalidGeometry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BoundingBox;

    fn rect() -> NativeRect {
        NativeRect::new(BoundingBox {
            x_min: 0.0,
            y_min: 0.0,
            x_max: 200.0,
            y_max: 100.0,
        })
        .unwrap()
    }

    fn hexagon() -> [[f32; 2]; 6] {
        [
            [30.0, 0.0],
            [170.0, 0.0],
            [200.0, 50.0],
            [170.0, 100.0],
            [30.0, 100.0],
            [0.0, 50.0],
        ]
    }

    fn assert_insets(actual: TextInsets, expected: [f32; 4]) {
        assert_eq!(
            [actual.left, actual.top, actual.right, actual.bottom],
            expected
        );
    }

    #[test]
    fn pentagon_reserves_the_vertex_cut_and_only_the_apex_side() {
        let points = [
            [100.0, 0.0],
            [200.0, 37.5],
            [162.5, 100.0],
            [37.5, 100.0],
            [0.0, 37.5],
        ];
        assert_insets(
            pentagon_insets(points, &rect()).unwrap(),
            [37.5, 25.0, 37.5, 0.0],
        );
        let flipped = points.map(|[x, y]| [200.0 - x, 100.0 - y]);
        assert_insets(
            pentagon_insets(flipped, &rect()).unwrap(),
            [37.5, 0.0, 37.5, 25.0],
        );
    }

    #[test]
    fn hexagon_uses_the_saved_side_cut_and_native_arithmetic_chain() {
        assert_insets(
            hexagon_insets(hexagon(), &rect()).unwrap(),
            [29.5, 14.75, 29.5, 14.75],
        );
        let flipped = hexagon().map(|[x, y]| [200.0 - x, 100.0 - y]);
        assert_insets(
            hexagon_insets(flipped, &rect()).unwrap(),
            [29.5, 14.75, 29.5, 14.75],
        );
    }

    #[test]
    fn finite_signed_polygon_margins_follow_retained_vertices() {
        let pentagon = [
            [100.0, 0.0],
            [200.0, 37.5],
            [162.5, 100.0],
            [-5.0, 100.0],
            [0.0, 37.5],
        ];
        assert_insets(
            pentagon_insets(pentagon, &rect()).unwrap(),
            [-5.0, 25.0, -5.0, 0.0],
        );
        let mut hexagon = hexagon();
        hexagon[0][0] = -50.0;
        assert_insets(
            hexagon_insets(hexagon, &rect()).unwrap(),
            [-2.5, -1.25, -2.5, -1.25],
        );
    }

    #[test]
    fn controls_rebuild_orientation_before_the_next_saved_control() {
        let first = apply_hexagon_control(hexagon(), [40.0, -900.0], &rect(), true, true).unwrap();
        assert_eq!(
            first,
            [
                [160.0, 100.0],
                [40.0, 100.0],
                [0.0, 50.0],
                [40.0, 0.0],
                [160.0, 0.0],
                [200.0, 50.0]
            ]
        );
        let second = apply_hexagon_control(first, [150.0, 900.0], &rect(), true, true).unwrap();
        assert_insets(
            hexagon_insets(second, &rect()).unwrap(),
            [37.5, 18.75, 37.5, 18.75],
        );
    }

    #[test]
    fn controls_clamp_to_the_horizontal_half_width_segment() {
        for (control, expected) in [
            ([-10.0, 80.0], [17.5, 8.75, 17.5, 8.75]),
            ([300.0, -80.0], [57.5, 28.75, 57.5, 28.75]),
        ] {
            let points = apply_hexagon_control(hexagon(), control, &rect(), false, false).unwrap();
            assert_insets(hexagon_insets(points, &rect()).unwrap(), expected);
        }
        assert!(matches!(
            apply_hexagon_control(hexagon(), [f32::INFINITY, 0.0], &rect(), false, false),
            Err(ShapeTextFrameIssue::InvalidGeometry)
        ));
    }

    #[test]
    fn equal_side_coordinates_keep_distinct_native_margin_and_control_predicates() {
        let mut points = hexagon();
        points[5][0] = points[2][0];
        assert_insets(
            hexagon_insets(points, &rect()).unwrap(),
            [85.5, 42.75, 85.5, 42.75],
        );
        let points = apply_hexagon_control(points, [40.0, 0.0], &rect(), false, false).unwrap();
        assert_insets(
            hexagon_insets(points, &rect()).unwrap(),
            [33.5, 16.75, 33.5, 16.75],
        );
    }

    #[test]
    fn native_center_clamping_keeps_endpoint_rounding_instead_of_substituting_half_width() {
        let rect = NativeRect::new(BoundingBox {
            x_min: 16_777_216.0,
            y_min: 0.0,
            x_max: 16_777_222.0,
            y_max: 100.0,
        })
        .unwrap();
        let points = hexagon().map(|[x, y]| [rect.left + x * 0.03, y]);
        let points =
            apply_hexagon_control(points, [16_777_220.0, 0.0], &rect, false, false).unwrap();
        assert_eq!(points[0][0], 16_777_220.0);
        assert_eq!(points[1][0], 16_777_218.0);
        assert_eq!(hexagon_insets(points, &rect).unwrap().left, 2.125);
    }

    #[test]
    fn native_margin_intermediate_overflow_is_reported() {
        let rect = NativeRect::new(BoundingBox {
            x_min: 0.0,
            y_min: 0.0,
            x_max: 1e37,
            y_max: 100.0,
        })
        .unwrap();
        let points = hexagon().map(|[x, y]| [x * 5e34, y]);
        assert!(matches!(
            hexagon_insets(points, &rect),
            Err(ShapeTextFrameIssue::InvalidGeometry)
        ));
    }

    #[test]
    fn polygon_command_contract_rejects_missing_close_curves_and_nonfinite_points() {
        let mut commands = vec![NativePathCommand::Move([100.0, 0.0])];
        commands.extend((0..4).map(|_| NativePathCommand::Line([0.0, 0.0])));
        assert!(matches!(
            vertices::<5>(&commands),
            Err(ShapeTextFrameIssue::UnsupportedTemplate)
        ));
        commands.push(NativePathCommand::Close);
        assert!(vertices::<5>(&commands).is_ok());
        commands[1] = NativePathCommand::Quadratic([[0.0, 0.0]; 2]);
        assert!(matches!(
            vertices::<5>(&commands),
            Err(ShapeTextFrameIssue::UnsupportedTemplate)
        ));
        commands[1] = NativePathCommand::Line([f32::NAN, 0.0]);
        assert!(matches!(
            vertices::<5>(&commands),
            Err(ShapeTextFrameIssue::InvalidGeometry)
        ));
    }
}
