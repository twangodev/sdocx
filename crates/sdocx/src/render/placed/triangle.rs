use crate::{NativeShape, shape::NativePathCommand};

use super::native::{NativeRect, TemplatePath, clamp_horizontal};
use super::{ShapeTextFrameIssue, TextInsets};

pub(super) fn insets(
    shape: &NativeShape,
    rect: &NativeRect,
) -> Result<TextInsets, ShapeTextFrameIssue> {
    let path = TemplatePath::read(shape, rect)?;
    let [first, second, _] = vertices(path.commands())?;
    match shape.shape_type {
        2 => triangle(
            rect,
            first,
            shape.rotation_degrees,
            shape.vertical_flip,
            &shape.control_points,
        ),
        3 => right_triangle(
            rect,
            [first, second],
            shape.rotation_degrees,
            shape.horizontal_flip,
        ),
        _ => Err(ShapeTextFrameIssue::UnsupportedTemplate),
    }
}

fn vertices(commands: &[NativePathCommand<f32>]) -> Result<[[f32; 2]; 3], ShapeTextFrameIssue> {
    match commands {
        [
            NativePathCommand::Move(first),
            NativePathCommand::Line(second),
            NativePathCommand::Line(third),
            NativePathCommand::Close,
        ] => Ok([*first, *second, *third]),
        _ => Err(ShapeTextFrameIssue::UnsupportedTemplate),
    }
}

fn triangle(
    rect: &NativeRect,
    first: [f32; 2],
    angle: f32,
    vertical_flip: bool,
    controls: &[[f64; 2]],
) -> Result<TextInsets, ShapeTextFrameIssue> {
    let mut ratio = (first[0] - rect.left) / rect.width;
    let edge_y = if vertical_flip { rect.bottom } else { rect.top };
    let control_length = rect.width.mul_add(rect.width, 0.0).sqrt();
    for point in controls {
        let point = rect.restore_point(*point, angle)?;
        if control_length == 0.0 {
            continue;
        }
        let point = clamp_horizontal(point, [rect.left, edge_y], [rect.right, edge_y])?;
        ratio = (point[0] - rect.left) / rect.width;
    }
    let half_width = rect.width * 0.5;
    let half_height = rect.height * 0.5;
    let insets = TextInsets {
        left: half_width * ratio,
        top: if vertical_flip { 0.0 } else { half_height },
        right: half_width * (1.0 - ratio),
        bottom: if vertical_flip { half_height } else { 0.0 },
    };
    checked(insets)
}

fn right_triangle(
    rect: &NativeRect,
    mut points: [[f32; 2]; 2],
    angle: f32,
    horizontal_flip: bool,
) -> Result<TextInsets, ShapeTextFrameIssue> {
    let delta_x = points[1][0] - points[0][0];
    let delta_y = points[1][1] - points[0][1];
    let direction = ((delta_y.atan2(delta_x) * 180.0) as f64 / std::f64::consts::PI) as f32;
    let direction = if direction < 0.0 {
        direction + 360.0
    } else {
        direction
    };
    if !direction.is_finite() {
        return Err(ShapeTextFrameIssue::InvalidGeometry);
    }
    if (direction == 0.0 && !horizontal_flip) || (direction == 180.0 && horizontal_flip) {
        for point in &mut points {
            *point = rect.rotate_point(*point, -angle)?;
        }
    }
    let (left, right) = if points[0][0] < points[1][0] {
        (35.0, 165.0)
    } else {
        (165.0, 35.0)
    };
    let (top, bottom) = if points[0][1] < points[1][1] {
        (235.0, 35.0)
    } else {
        (35.0, 235.0)
    };
    checked(TextInsets {
        left: (rect.width * left) / 400.0,
        top: (rect.height * top) / 400.0,
        right: (rect.width * right) / 400.0,
        bottom: (rect.height * bottom) / 400.0,
    })
}

fn checked(insets: TextInsets) -> Result<TextInsets, ShapeTextFrameIssue> {
    if [insets.left, insets.top, insets.right, insets.bottom]
        .into_iter()
        .all(f32::is_finite)
    {
        Ok(insets)
    } else {
        Err(ShapeTextFrameIssue::InvalidGeometry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect() -> NativeRect {
        NativeRect {
            left: 0.0,
            top: 0.0,
            right: 400.0,
            bottom: 200.0,
            width: 400.0,
            height: 200.0,
            center: [200.0, 100.0],
        }
    }

    fn values(insets: TextInsets) -> [f32; 4] {
        [insets.left, insets.top, insets.right, insets.bottom]
    }

    #[test]
    fn triangle_retains_path_ratio_and_vertical_flip() {
        for (flip, expected) in [
            (false, [50.0, 100.0, 150.0, 0.0]),
            (true, [50.0, 0.0, 150.0, 100.0]),
        ] {
            assert_eq!(
                values(triangle(&rect(), [100.0, 0.0], 0.0, flip, &[]).unwrap()),
                expected
            );
        }
        assert_eq!(
            values(triangle(&rect(), [-100.0, 0.0], 0.0, false, &[]).unwrap()),
            [-50.0, 100.0, 250.0, 0.0],
        );
    }

    #[test]
    fn triangle_restores_controls_and_last_control_overrides_path() {
        let controls = [[300.0, 200.0], [300.0, 0.0]];
        assert_eq!(
            values(triangle(&rect(), [200.0, 0.0], 90.0, false, &controls).unwrap()),
            [50.0, 100.0, 150.0, 0.0],
        );
        for (point, expected) in [
            ([-100.0, 50.0], [0.0, 100.0, 200.0, 0.0]),
            ([500.0, 50.0], [200.0, 100.0, 0.0, 0.0]),
        ] {
            assert_eq!(
                values(triangle(&rect(), [200.0, 0.0], 0.0, false, &[point]).unwrap()),
                expected,
            );
        }
    }

    #[test]
    fn right_triangle_uses_vertex_order_and_separate_scaled_divisions() {
        for (points, expected) in [
            ([[0.0, 0.0], [400.0, 200.0]], [35.0, 117.5, 165.0, 17.5]),
            ([[400.0, 200.0], [0.0, 0.0]], [165.0, 17.5, 35.0, 117.5]),
        ] {
            assert_eq!(
                values(right_triangle(&rect(), points, 0.0, false).unwrap()),
                expected
            );
        }
        assert_eq!(
            values(
                right_triangle(&rect(), [[-f32::MAX, 0.0], [f32::MAX, 200.0]], 0.0, false).unwrap()
            ),
            [35.0, 117.5, 165.0, 17.5],
        );
    }

    #[test]
    fn right_triangle_horizontal_reload_uses_saved_flip_and_angle() {
        let points = [[0.0, 0.0], [400.0, 0.0]];
        assert_eq!(
            values(right_triangle(&rect(), points, 90.0, false).unwrap()),
            [165.0, 17.5, 35.0, 117.5],
        );
        assert_eq!(
            values(right_triangle(&rect(), points, 90.0, true).unwrap()),
            [35.0, 17.5, 165.0, 117.5],
        );
        let reversed = [[400.0, 0.0], [0.0, 0.0]];
        assert_eq!(
            values(right_triangle(&rect(), reversed, 90.0, true).unwrap()),
            [165.0, 117.5, 35.0, 17.5],
        );
        assert_eq!(
            values(right_triangle(&rect(), reversed, 90.0, false).unwrap()),
            [165.0, 17.5, 35.0, 117.5],
        );
    }

    #[test]
    fn underflowed_triangle_control_length_leaves_the_saved_apex_ratio() {
        let rect = NativeRect::new(crate::BoundingBox {
            x_min: 0.0,
            y_min: 0.0,
            x_max: 1e-23,
            y_max: 100.0,
        })
        .unwrap();
        let apex = f64::from(rect.width * 0.5);
        let expected = triangle(&rect, [apex as f32, 0.0], 0.0, false, &[]).unwrap();
        assert_eq!(
            values(expected),
            [rect.width * 0.25, 50.0, rect.width * 0.25, 0.0]
        );
        assert_eq!(
            triangle(&rect, [apex as f32, 0.0], 0.0, false, &[[apex, 0.0]]).unwrap(),
            expected,
        );
    }

    #[test]
    fn triangle_requires_the_native_vertex_schema() {
        assert!(
            vertices(&[
                NativePathCommand::Move([0.0, 0.0]),
                NativePathCommand::Close
            ])
            .is_err()
        );
    }
}
