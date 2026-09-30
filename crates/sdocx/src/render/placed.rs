use crate::{BoundingBox, NativeShape, RichTextBox};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct TextRotation {
    pub degrees: f64,
    pub center: [f64; 2],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct PlacedTextFrame {
    pub bounds: BoundingBox,
    pub background_bounds: BoundingBox,
    pub rotation: Option<TextRotation>,
    measured_size: Option<[i32; 2]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ShapeTextFrameIssue {
    UnsupportedTemplate,
    InvalidGeometry,
}

impl PlacedTextFrame {
    pub fn from_text_box(text: &RichTextBox) -> Self {
        let bounds = text.bbox;
        Self {
            bounds,
            background_bounds: bounds,
            measured_size: None,
            rotation: text.rotation_degrees.map(|degrees| TextRotation {
                degrees,
                center: [
                    bounds.x_min + (bounds.x_max - bounds.x_min) / 2.0,
                    bounds.y_min + (bounds.y_max - bounds.y_min) / 2.0,
                ],
            }),
        }
    }

    pub fn measurement_size(&self) -> Option<[i32; 2]> {
        self.measured_size
    }

    pub fn for_shape(shape: &NativeShape) -> Result<Self, ShapeTextFrameIssue> {
        Self::from_shape_geometry(
            shape.shape_type,
            shape.geometry_bbox,
            shape.rotation_degrees,
        )
    }

    fn from_shape_geometry(
        template: u32,
        geometry: BoundingBox,
        rotation_degrees: f32,
    ) -> Result<Self, ShapeTextFrameIssue> {
        if !matches!(template, 1 | 4 | 8) {
            return Err(ShapeTextFrameIssue::UnsupportedTemplate);
        }
        let [left, top, right, bottom] = [
            geometry.x_min as f32,
            geometry.y_min as f32,
            geometry.x_max as f32,
            geometry.y_max as f32,
        ];
        let width = right - left;
        let height = bottom - top;
        let background_bounds = BoundingBox {
            x_min: f64::from(left),
            y_min: f64::from(top),
            x_max: f64::from(right),
            y_max: f64::from(bottom),
        };
        let center = [(left + right) * 0.5, (top + bottom) * 0.5];
        if ![
            left,
            top,
            right,
            bottom,
            width,
            height,
            center[0],
            center[1],
            rotation_degrees,
        ]
        .into_iter()
        .all(f32::is_finite)
            || width <= 0.0
            || height <= 0.0
        {
            return Err(ShapeTextFrameIssue::InvalidGeometry);
        }
        let [horizontal, vertical] = match template {
            1 => [(width * 3.0) / 20.0, (height * 3.0) / 20.0],
            4 => [0.0, 0.0],
            8 => [
                width * f32::from_bits(0x3e85_1eb8),
                height * f32::from_bits(0x3e85_1eb8),
            ],
            _ => unreachable!(),
        };
        let [left, top, right, bottom] = [
            left + horizontal,
            top + vertical,
            right - horizontal,
            bottom - vertical,
        ];
        let width = right - left;
        let height = bottom - top;
        if ![left, top, right, bottom, width, height]
            .into_iter()
            .all(f32::is_finite)
            || width <= 0.0
            || height <= 0.0
        {
            return Err(ShapeTextFrameIssue::InvalidGeometry);
        }
        let bounds = BoundingBox {
            x_min: f64::from(left),
            y_min: f64::from(top),
            x_max: f64::from(right),
            y_max: f64::from(bottom),
        };
        Ok(Self {
            bounds,
            background_bounds,
            measured_size: Some([width.ceil() as i32, height.ceil() as i32]),
            rotation: Some(TextRotation {
                degrees: f64::from(rotation_degrees),
                center: center.map(f64::from),
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds(left: f64, top: f64, right: f64, bottom: f64) -> BoundingBox {
        BoundingBox {
            x_min: left,
            y_min: top,
            x_max: right,
            y_max: bottom,
        }
    }

    #[test]
    fn native_templates_inset_the_unrotated_frame() {
        for (template, expected, size) in [
            (1, bounds(30.0, 15.0, 170.0, 85.0), [140, 70]),
            (4, bounds(0.0, 0.0, 200.0, 100.0), [200, 100]),
            (8, bounds(52.0, 26.0, 148.0, 74.0), [96, 48]),
        ] {
            let frame = PlacedTextFrame::from_shape_geometry(
                template,
                bounds(0.0, 0.0, 200.0, 100.0),
                30.0,
            )
            .unwrap();
            assert_eq!(frame.bounds, expected);
            assert_eq!(frame.measurement_size(), Some(size));
            assert_eq!(
                frame.rotation,
                Some(TextRotation {
                    degrees: 30.0,
                    center: [100.0, 50.0]
                })
            );
        }
    }

    #[test]
    fn native_endpoint_subtraction_precedes_ceiled_layout_dimensions() {
        let frame =
            PlacedTextFrame::from_shape_geometry(4, bounds(1.0, 0.25, 201.000001, 100.250001), 0.0)
                .unwrap();
        assert_eq!(frame.bounds, bounds(1.0, 0.25, 201.0, 100.25));
        assert_eq!(frame.measurement_size(), Some([200, 100]));
        let frame =
            PlacedTextFrame::from_shape_geometry(4, bounds(1.0, 0.25, 201.00001, 100.25001), 0.0)
                .unwrap();
        assert_eq!(
            frame.bounds,
            bounds(1.0, 0.25, 201.000_015_258_789_06, 100.250_007_629_394_53)
        );
        assert_eq!(frame.measurement_size(), Some([201, 101]));
    }

    #[test]
    fn native_measurement_dimensions_ceil_to_saturating_signed_integers() {
        let frame = PlacedTextFrame::from_shape_geometry(
            4,
            bounds(0.0, 0.0, 30_000_000_000.0, 30_000_000_000.0),
            0.0,
        )
        .unwrap();
        assert_eq!(frame.measurement_size(), Some([i32::MAX, i32::MAX]));
        let tiny =
            PlacedTextFrame::from_shape_geometry(4, bounds(0.0, 0.0, 0.000001, 0.0000001), 0.0)
                .unwrap();
        assert_eq!(tiny.measurement_size(), Some([1, 1]));
    }

    #[test]
    fn enormous_origins_keep_native_measurement_size_independent_of_endpoints() {
        let frame =
            PlacedTextFrame::from_shape_geometry(4, bounds(1e30, 0.0, 1.000001e30, 100.0), 0.0)
                .unwrap();
        assert_eq!(frame.measurement_size(), Some([i32::MAX, 100]));
        assert!(frame.bounds.x_max > frame.bounds.x_min);
        assert_eq!(frame.bounds.x_min + f64::from(i32::MAX), frame.bounds.x_min);
    }

    #[test]
    fn rotation_pivot_uses_original_native_endpoint_sum_then_half() {
        let frame = PlacedTextFrame::from_shape_geometry(
            4,
            bounds(16_777_216.0, 0.0, 16_777_218.0, 100.0),
            45.0,
        )
        .unwrap();
        assert_eq!(frame.rotation.unwrap().center, [16_777_216.0, 50.0]);
        let ellipse =
            PlacedTextFrame::from_shape_geometry(1, bounds(20.0, -10.0, 220.0, 90.0), -90.0)
                .unwrap();
        assert_eq!(ellipse.rotation.unwrap().center, [120.0, 40.0]);
    }

    #[test]
    fn unsupported_templates_are_explicit() {
        for template in [0, 2, 3, 5, 9, u32::MAX] {
            assert_eq!(
                PlacedTextFrame::from_shape_geometry(template, bounds(0.0, 0.0, 200.0, 100.0), 0.0),
                Err(ShapeTextFrameIssue::UnsupportedTemplate)
            );
        }
    }

    #[test]
    fn invalid_native_geometry_and_intermediate_overflow_are_rejected() {
        for (template, geometry, rotation) in [
            (4, bounds(0.0, 0.0, 0.0, 100.0), 0.0),
            (4, bounds(0.0, 0.0, 200.0, -1.0), 0.0),
            (4, bounds(16_777_216.0, 0.0, 16_777_217.0, 100.0), 0.0),
            (4, bounds(0.0, 0.0, 200.0, 100.0), f32::INFINITY),
            (4, bounds(0.0, 0.0, f64::INFINITY, 100.0), 0.0),
            (
                4,
                bounds(-f64::from(f32::MAX), 0.0, f64::from(f32::MAX), 100.0),
                0.0,
            ),
            (
                4,
                bounds(f64::from(f32::MAX / 2.0), 0.0, f64::from(f32::MAX), 100.0),
                0.0,
            ),
            (1, bounds(0.0, 0.0, f64::from(f32::MAX), 100.0), 0.0),
        ] {
            assert_eq!(
                PlacedTextFrame::from_shape_geometry(template, geometry, rotation),
                Err(ShapeTextFrameIssue::InvalidGeometry)
            );
        }
    }

    #[test]
    fn ordinary_text_keeps_its_original_f64_frame_and_pivot() {
        let text = RichTextBox {
            text_area_type: None,
            bbox: bounds(1.00000001, 2.25, 201.000001, 102.25),
            rotation_degrees: Some(30.0),
            text: "A".into(),
            color: None,
            highlight_color: None,
            underline: false,
            font_size: None,
            runs: Vec::new(),
            spans: Vec::new(),
            paragraphs: Vec::new(),
            object_spans: Vec::new(),
            text_sections: Vec::new(),
            margins: None,
            gravity: None,
        };
        let frame = PlacedTextFrame::from_text_box(&text);
        assert_eq!(frame.bounds, text.bbox);
        assert_eq!(frame.measurement_size(), None);
        assert_eq!(
            frame.rotation.unwrap().center,
            [101.000_000_504_999_99, 52.25]
        );
    }
}
