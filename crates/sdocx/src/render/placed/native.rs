use crate::shape::{NativePathCommand, visit_path};
use crate::{BoundingBox, NativeShape};

use super::ShapeTextFrameIssue;

#[derive(Clone, Copy)]
pub(super) struct NativeRect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub width: f32,
    pub height: f32,
    pub center: [f32; 2],
}

impl NativeRect {
    pub fn new(bounds: BoundingBox) -> Result<Self, ShapeTextFrameIssue> {
        let left = bounds.x_min as f32;
        let top = bounds.y_min as f32;
        let right = bounds.x_max as f32;
        let bottom = bounds.y_max as f32;
        let width = right - left;
        let height = bottom - top;
        let center = [(left + right) * 0.5, (top + bottom) * 0.5];
        if ![
            left, top, right, bottom, width, height, center[0], center[1],
        ]
        .into_iter()
        .all(f32::is_finite)
            || width <= 0.0
            || height <= 0.0
        {
            return Err(ShapeTextFrameIssue::InvalidGeometry);
        }
        Ok(Self {
            left,
            top,
            right,
            bottom,
            width,
            height,
            center,
        })
    }

    pub fn bounds(self) -> BoundingBox {
        BoundingBox {
            x_min: f64::from(self.left),
            y_min: f64::from(self.top),
            x_max: f64::from(self.right),
            y_max: f64::from(self.bottom),
        }
    }

    pub fn restore_point(
        &self,
        point: [f64; 2],
        degrees: f32,
    ) -> Result<[f32; 2], ShapeTextFrameIssue> {
        self.rotate_point(point.map(|value| value as f32), -degrees)
    }

    pub fn rotate_point(
        &self,
        point: [f32; 2],
        degrees: f32,
    ) -> Result<[f32; 2], ShapeTextFrameIssue> {
        if !point.into_iter().chain([degrees]).all(f32::is_finite) {
            return Err(ShapeTextFrameIssue::InvalidGeometry);
        }
        if degrees == 0.0 {
            return Ok(point);
        }
        let radians = (f64::from(degrees) / 180.0) * std::f64::consts::PI;
        let (sin, cos) = radians.sin_cos();
        let dx = f64::from(point[0] - self.center[0]);
        let dy = f64::from(point[1] - self.center[1]);
        let result = [
            self.center[0] + dx.mul_add(cos, -(sin * dy)) as f32,
            self.center[1] + dx.mul_add(sin, cos * dy) as f32,
        ];
        result
            .into_iter()
            .all(f32::is_finite)
            .then_some(result)
            .ok_or(ShapeTextFrameIssue::InvalidGeometry)
    }
}

pub(super) fn clamp_horizontal(
    point: [f32; 2],
    start: [f32; 2],
    end: [f32; 2],
) -> Result<[f32; 2], ShapeTextFrameIssue> {
    if !point
        .into_iter()
        .chain(start)
        .chain(end)
        .all(f32::is_finite)
        || start[1] != end[1]
    {
        return Err(ShapeTextFrameIssue::InvalidGeometry);
    }
    let dx = start[0] - end[0];
    let dy = start[1] - end[1];
    let length = dx.mul_add(dx, dy * dy).sqrt();
    let divisor = if length == 0.0 { 1.0 } else { length };
    let term = (point[1] - start[1]) * (end[1] - start[1]);
    let projection = (point[0] - start[0]).mul_add(end[0] - start[0], term) / divisor;
    Ok(if projection <= 0.0 {
        start
    } else if projection >= divisor {
        end
    } else {
        [point[0], start[1]]
    })
}

pub(super) struct TemplatePath {
    commands: [NativePathCommand<f32>; 10],
    length: usize,
}

impl TemplatePath {
    pub fn read(shape: &NativeShape, rect: &NativeRect) -> Result<Self, ShapeTextFrameIssue> {
        if shape.path_data.is_empty() {
            return Err(ShapeTextFrameIssue::UnsupportedTemplate);
        }
        let mut raw = [NativePathCommand::Close; 10];
        let mut length = 0usize;
        let (consumed, supported) = visit_path(&shape.path_data, |command| {
            if let Some(slot) = raw.get_mut(length) {
                *slot = command;
            }
            length = length.saturating_add(1);
        })
        .map_err(|_| ShapeTextFrameIssue::InvalidGeometry)?;
        if !supported || consumed != shape.path_data.len() || length > raw.len() {
            return Err(ShapeTextFrameIssue::UnsupportedTemplate);
        }
        let mut commands = [NativePathCommand::Close; 10];
        let restore = |point| rect.restore_point(point, shape.rotation_degrees);
        for (slot, command) in commands.iter_mut().zip(raw).take(length) {
            *slot = match command {
                NativePathCommand::Move(point) => NativePathCommand::Move(restore(point)?),
                NativePathCommand::Line(point) => NativePathCommand::Line(restore(point)?),
                NativePathCommand::Quadratic([first, end]) => {
                    NativePathCommand::Quadratic([restore(first)?, restore(end)?])
                }
                NativePathCommand::Cubic([first, second, end]) => {
                    NativePathCommand::Cubic([restore(first)?, restore(second)?, restore(end)?])
                }
                NativePathCommand::Close => NativePathCommand::Close,
                NativePathCommand::Arc(_) | NativePathCommand::Oval(_) => {
                    return Err(ShapeTextFrameIssue::UnsupportedTemplate);
                }
            };
        }
        Ok(Self { commands, length })
    }

    pub fn commands(&self) -> &[NativePathCommand<f32>] {
        &self.commands[..self.length]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect() -> NativeRect {
        NativeRect::new(BoundingBox {
            x_min: 0.0,
            y_min: 0.0,
            x_max: 200.0,
            y_max: 100.0,
        })
        .unwrap()
    }

    #[test]
    fn inverse_rotation_recovers_world_points_about_the_original_center() {
        assert_eq!(
            rect().restore_point([100.0, 90.0], 90.0).unwrap(),
            [140.0, 50.0]
        );
        assert_eq!(
            rect().restore_point([60.0, 50.0], 180.0).unwrap(),
            [140.0, 50.0]
        );
        assert_eq!(
            rect().restore_point([100.0, 10.0], 270.0).unwrap(),
            [140.0, 50.0]
        );
    }

    #[test]
    fn noncardinal_rotation_preserves_native_narrowing_and_center_rounding() {
        for (bounds, point, angle, expected) in [
            (
                BoundingBox {
                    x_min: 1868.2598,
                    y_min: 33.549805,
                    x_max: 2305.0388,
                    y_max: 1003.1478,
                },
                [2_646.705_749_634_719, 116.720_810_392_523_43],
                102.409_91,
                [0x44c4_c180, 0x4266_c230],
            ),
            (
                BoundingBox {
                    x_min: -470.1499,
                    y_min: -4018.32,
                    x_max: 42.617096,
                    y_max: -3168.339,
                },
                [-69.716_888_304_734_37, -3_734.565_063_020_562_3],
                251.150_02,
                [0xc2fd_4b3e, 0xc555_35fb],
            ),
        ] {
            let rect = NativeRect::new(bounds).unwrap();
            assert_eq!(
                rect.restore_point(point, angle).unwrap().map(f32::to_bits),
                expected
            );
        }
    }

    #[test]
    fn zero_rotation_narrows_storage_without_subtracting_the_center() {
        let point = [f64::from(f32::MAX), f64::from(f32::MIN_POSITIVE)];
        assert_eq!(
            rect().restore_point(point, -0.0).unwrap(),
            [f32::MAX, f32::MIN_POSITIVE]
        );
        assert_eq!(
            rect().restore_point([f64::MAX, 0.0], 0.0),
            Err(ShapeTextFrameIssue::InvalidGeometry)
        );
    }

    #[test]
    fn horizontal_projection_clamps_in_both_directions_and_preserves_interior_x() {
        for (start, end) in [([20.0, 7.0], [100.0, 7.0]), ([100.0, 7.0], [20.0, 7.0])] {
            assert_eq!(
                clamp_horizontal([65.25, -50.0], start, end).unwrap(),
                [65.25, 7.0]
            );
            assert_eq!(
                clamp_horizontal([0.0, -50.0], start, end).unwrap(),
                [20.0, 7.0]
            );
            assert_eq!(
                clamp_horizontal([200.0, -50.0], start, end).unwrap(),
                [100.0, 7.0]
            );
        }
        assert_eq!(
            clamp_horizontal([65.0, 0.0], [20.0, 7.0], [20.0, 7.0]).unwrap(),
            [20.0, 7.0]
        );
    }

    #[test]
    fn finite_controls_keep_native_infinite_and_unordered_projection_branches() {
        assert_eq!(
            clamp_horizontal([f32::MAX, 0.0], [0.0, 0.0], [200.0, 0.0]).unwrap(),
            [200.0, 0.0],
        );
        assert_eq!(
            clamp_horizontal([-f32::MAX, 0.0], [0.0, 0.0], [200.0, 0.0]).unwrap(),
            [0.0, 0.0],
        );
        assert_eq!(
            clamp_horizontal([1e19, 0.0], [0.0, 0.0], [4e19, 0.0]).unwrap(),
            [1e19, 0.0],
        );
        assert_eq!(
            clamp_horizontal([65.25, f32::MAX], [0.0, -f32::MAX], [200.0, -f32::MAX]).unwrap(),
            [65.25, -f32::MAX],
        );
    }
}
