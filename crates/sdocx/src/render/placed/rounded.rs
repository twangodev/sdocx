use crate::NativeShape;
use crate::shape::NativePathCommand;

use super::native::{NativeRect, TemplatePath, clamp_horizontal};
use super::{ShapeTextFrameIssue, TextInsets};

pub(super) fn insets(
    shape: &NativeShape,
    rect: &NativeRect,
) -> Result<TextInsets, ShapeTextFrameIssue> {
    let path = TemplatePath::read(shape, rect)?;
    let mut corners = Corners::read(path.commands())?;
    for &point in &shape.control_points {
        let point = rect.restore_point(point, shape.rotation_degrees)?;
        corners.apply_control(point, rect, shape.horizontal_flip, shape.vertical_flip)?;
    }
    let margin = corners.margin(rect)?;
    Ok(TextInsets::symmetric(margin, margin))
}

struct Corners {
    first: [f32; 2],
    line3_x: f32,
    opposite_y: f32,
    line7_x: f32,
}

impl Corners {
    fn read(commands: &[NativePathCommand<f32>]) -> Result<Self, ShapeTextFrameIssue> {
        use NativePathCommand::{Close, Cubic, Line, Move};
        let [
            Move(first),
            Line(_),
            Cubic(_),
            Line(line3),
            Cubic(_),
            Line(opposite),
            Cubic(_),
            Line(line7),
            Cubic(_),
            Close,
        ] = commands
        else {
            return Err(ShapeTextFrameIssue::UnsupportedTemplate);
        };
        Ok(Self {
            first: *first,
            line3_x: line3[0],
            opposite_y: opposite[1],
            line7_x: line7[0],
        })
    }

    fn radius_at(&self, x: f32, rect: &NativeRect) -> Result<f32, ShapeTextFrameIssue> {
        let radius = if self.line7_x > self.line3_x {
            rect.right - x
        } else {
            x - rect.left
        };
        if radius.is_finite() {
            Ok(radius)
        } else {
            Err(ShapeTextFrameIssue::InvalidGeometry)
        }
    }

    fn apply_control(
        &mut self,
        point: [f32; 2],
        rect: &NativeRect,
        horizontal_flip: bool,
        vertical_flip: bool,
    ) -> Result<(), ShapeTextFrameIssue> {
        let mirrored = self.line7_x >= self.line3_x;
        let start_x = if mirrored { rect.right } else { rect.left };
        let half_height = if rect.height == 0.0 {
            0.0
        } else {
            rect.height * 0.5
        };
        let end_x = if rect.height >= rect.width {
            rect.center[0]
        } else if mirrored {
            rect.right - half_height
        } else {
            rect.left + half_height
        };
        let y = if self.first[1] < self.opposite_y {
            rect.top
        } else {
            rect.bottom
        };
        let point = clamp_horizontal(point, [start_x, y], [end_x, y])?;
        let radius = self.radius_at(point[0], rect)?;
        self.reconstruct(radius, rect, horizontal_flip, vertical_flip)
    }

    fn reconstruct(
        &mut self,
        radius: f32,
        rect: &NativeRect,
        horizontal_flip: bool,
        vertical_flip: bool,
    ) -> Result<(), ShapeTextFrameIssue> {
        let first_x = if horizontal_flip {
            rect.right - radius
        } else {
            rect.left + radius
        };
        if !first_x.is_finite() {
            return Err(ShapeTextFrameIssue::InvalidGeometry);
        }
        self.first = [first_x, if vertical_flip { rect.bottom } else { rect.top }];
        self.line3_x = if horizontal_flip {
            rect.left
        } else {
            rect.right
        };
        self.line7_x = if horizontal_flip {
            rect.right
        } else {
            rect.left
        };
        self.opposite_y = if vertical_flip { rect.top } else { rect.bottom };
        Ok(())
    }

    fn margin(&self, rect: &NativeRect) -> Result<f32, ShapeTextFrameIssue> {
        let radius = self.radius_at(self.first[0], rect)?;
        let margin = radius + ((radius * f32::from_bits(0xbfb5_04f3)) * 0.5);
        if margin.is_finite() {
            Ok(margin)
        } else {
            Err(ShapeTextFrameIssue::InvalidGeometry)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PATH: [NativePathCommand<f32>; 10] = [
        NativePathCommand::Move([10.0, 0.0]),
        NativePathCommand::Line([70.0, 0.0]),
        NativePathCommand::Cubic([[75.0, 0.0], [80.0, 5.0], [80.0, 10.0]]),
        NativePathCommand::Line([80.0, 30.0]),
        NativePathCommand::Cubic([[80.0, 35.0], [75.0, 40.0], [70.0, 40.0]]),
        NativePathCommand::Line([10.0, 40.0]),
        NativePathCommand::Cubic([[5.0, 40.0], [0.0, 35.0], [0.0, 30.0]]),
        NativePathCommand::Line([0.0, 10.0]),
        NativePathCommand::Cubic([[0.0, 5.0], [5.0, 0.0], [10.0, 0.0]]),
        NativePathCommand::Close,
    ];

    fn rect() -> NativeRect {
        NativeRect {
            left: 0.0,
            top: 0.0,
            right: 80.0,
            bottom: 40.0,
            width: 80.0,
            height: 40.0,
            center: [40.0, 20.0],
        }
    }

    #[test]
    fn canonical_saved_radius_uses_native_operation_order() {
        let corners = Corners::read(&PATH).unwrap();
        assert_eq!(
            f64::from(corners.margin(&rect()).unwrap()),
            2.9289321899414062
        );
        let corners = Corners {
            first: [6.666_666_5, 0.0],
            ..corners
        };
        assert_eq!(
            f64::from(corners.margin(&rect()).unwrap()),
            1.9526214599609375
        );
    }

    #[test]
    fn empty_and_noncanonical_paths_do_not_guess_a_default_radius() {
        assert!(matches!(
            Corners::read(&[]),
            Err(ShapeTextFrameIssue::UnsupportedTemplate)
        ));
        let mut path = PATH;
        path[2] = NativePathCommand::Quadratic([[80.0, 0.0], [80.0, 10.0]]);
        assert!(matches!(
            Corners::read(&path),
            Err(ShapeTextFrameIssue::UnsupportedTemplate)
        ));
        assert!(matches!(
            Corners::read(&PATH[..9]),
            Err(ShapeTextFrameIssue::UnsupportedTemplate)
        ));
    }

    #[test]
    fn controls_clamp_to_corner_range_and_replay_in_order() {
        let rect = rect();
        let mut corners = Corners::read(&PATH).unwrap();
        corners
            .apply_control([100.0, -100.0], &rect, false, false)
            .unwrap();
        assert_eq!(corners.first, [20.0, 0.0]);
        assert_eq!(
            f64::from(corners.margin(&rect).unwrap()),
            5.8578643798828125
        );
        corners
            .apply_control([-100.0, 100.0], &rect, false, false)
            .unwrap();
        assert_eq!(corners.margin(&rect).unwrap(), 0.0);
        corners
            .apply_control([5.0, 100.0], &rect, false, false)
            .unwrap();
        assert_eq!(
            f64::from(corners.margin(&rect).unwrap()),
            1.4644660949707031
        );
    }

    #[test]
    fn stored_flips_reconstruct_the_path_before_the_next_control() {
        let rect = rect();
        let mut corners = Corners::read(&PATH).unwrap();
        corners
            .apply_control([12.0, 0.0], &rect, true, true)
            .unwrap();
        assert_eq!(corners.first, [68.0, 40.0]);
        assert_eq!(corners.line3_x, 0.0);
        assert_eq!(corners.line7_x, 80.0);
        assert_eq!(corners.opposite_y, 0.0);
        corners
            .apply_control([60.0, -100.0], &rect, true, true)
            .unwrap();
        assert_eq!(corners.first, [60.0, 40.0]);
        assert_eq!(
            f64::from(corners.margin(&rect).unwrap()),
            5.8578643798828125
        );
    }

    #[test]
    fn equal_side_coordinates_use_different_clamp_and_radius_predicates() {
        let rect = rect();
        let mut corners = Corners::read(&PATH).unwrap();
        corners.line7_x = 80.0;
        corners
            .apply_control([70.0, 0.0], &rect, false, false)
            .unwrap();
        assert_eq!(corners.first, [70.0, 0.0]);
        assert_eq!(corners.radius_at(corners.first[0], &rect).unwrap(), 70.0);
    }

    #[test]
    fn radius_is_recovered_from_the_reconstructed_native_coordinate() {
        let rect = NativeRect::new(crate::BoundingBox {
            x_min: 393_669.656_25,
            y_min: 0.0,
            x_max: 2_000_000.0,
            y_max: 2_000_000.0,
        })
        .unwrap();
        let mut corners = Corners::read(&PATH).unwrap();
        corners
            .reconstruct(f32::from_bits(0x4904_9623), &rect, false, false)
            .unwrap();
        assert_eq!(f64::from(corners.first[0]), 936_743.875);
        assert_eq!(
            f64::from(corners.radius_at(corners.first[0], &rect).unwrap()),
            543_074.25
        );
    }

    #[test]
    fn overflowing_derived_radius_is_invalid_geometry() {
        let mut rect = rect();
        rect.left = -f32::MAX;
        let mut corners = Corners::read(&PATH).unwrap();
        corners.first[0] = f32::MAX;
        assert_eq!(
            corners.margin(&rect),
            Err(ShapeTextFrameIssue::InvalidGeometry)
        );
    }
}
