//! Geometry for ordinary image source-rectangle crops.
//!
//! Native image drawing sends the crop rectangle as a bitmap source rectangle,
//! then stretches that selected rectangle over the image's existing bounds.
//! This helper computes the equivalent full-source placement for a viewport
//! clipped to those bounds. It does not decode or modify bitmap pixels.

use crate::BoundingBox;

/// Return the full bitmap placement that maps an in-bounds source crop onto
/// `displayed_bbox`.
///
/// `None` means the encoded image dimensions could not be read, the crop is
/// empty/inverted/outside the source, or the displayed geometry is invalid.
pub(crate) fn crop_only_placement(
    source_bytes: &[u8],
    crop_rect: [i32; 4],
    displayed_bbox: BoundingBox,
) -> Option<BoundingBox> {
    let source = imagesize::blob_size(source_bytes).ok()?;
    source_crop_placement(source.width, source.height, crop_rect, displayed_bbox)
}

fn source_crop_placement(
    source_width: usize,
    source_height: usize,
    [left, top, right, bottom]: [i32; 4],
    displayed: BoundingBox,
) -> Option<BoundingBox> {
    if source_width == 0
        || source_height == 0
        || left < 0
        || top < 0
        || right <= left
        || bottom <= top
        || right as usize > source_width
        || bottom as usize > source_height
        || !valid_bbox(displayed)
    {
        return None;
    }

    let crop_width = f64::from(right - left);
    let crop_height = f64::from(bottom - top);
    let scale_x = (displayed.x_max - displayed.x_min) / crop_width;
    let scale_y = (displayed.y_max - displayed.y_min) / crop_height;
    let source_width = source_width as f64;
    let source_height = source_height as f64;
    let x_min = displayed.x_min - f64::from(left) * scale_x;
    let y_min = displayed.y_min - f64::from(top) * scale_y;
    let placed = BoundingBox {
        x_min,
        y_min,
        x_max: x_min + source_width * scale_x,
        y_max: y_min + source_height * scale_y,
    };
    valid_bbox(placed).then_some(placed)
}

fn valid_bbox(bbox: BoundingBox) -> bool {
    bbox.x_min.is_finite()
        && bbox.y_min.is_finite()
        && bbox.x_max.is_finite()
        && bbox.y_max.is_finite()
        && bbox.x_max > bbox.x_min
        && bbox.y_max > bbox.y_min
}

#[cfg(test)]
mod tests {
    use super::*;

    // 4×3 RGBA PNG with red/green top quadrants and blue/yellow bottom
    // quadrants. This asymmetric source makes the source-to-destination map
    // independently checkable without relying on production render helpers.
    const QUADRANTS: &[u8] = include_bytes!("../../tests/assets/crop-quadrants.png");

    #[test]
    fn maps_colored_source_crop_over_unchanged_display_bounds() {
        assert_eq!(imagesize::blob_size(QUADRANTS).unwrap().width, 4);
        assert_eq!(imagesize::blob_size(QUADRANTS).unwrap().height, 3);

        let displayed = BoundingBox {
            x_min: 10.0,
            y_min: 20.0,
            x_max: 50.0,
            y_max: 50.0,
        };
        let original = crop_only_placement(QUADRANTS, [1, 1, 3, 2], displayed).unwrap();

        // Source edge (1,1) maps to displayed top-left (10,20), and (3,2)
        // maps to bottom-right (50,50). Red and green source pixel centers
        // consequently land at (20,35) and (40,35), respectively.
        assert_eq!(
            original,
            BoundingBox {
                x_min: -10.0,
                y_min: -10.0,
                x_max: 70.0,
                y_max: 80.0,
            }
        );
        let map_x =
            |source_x: f64| original.x_min + source_x * (displayed.x_max - displayed.x_min) / 2.0;
        let map_y = |source_y: f64| original.y_min + source_y * (displayed.y_max - displayed.y_min);
        assert_eq!((map_x(1.0), map_y(1.0)), (10.0, 20.0));
        assert_eq!((map_x(1.5), map_y(1.5)), (20.0, 35.0));
        assert_eq!((map_x(2.5), map_y(1.5)), (40.0, 35.0));
        assert_eq!((map_x(3.0), map_y(2.0)), (50.0, 50.0));
        assert_eq!(imagesize::blob_size(QUADRANTS).unwrap().width, 4);
    }

    #[test]
    fn rejects_empty_inverted_negative_and_out_of_bounds_crops() {
        let bbox = BoundingBox {
            x_min: 0.0,
            y_min: 0.0,
            x_max: 10.0,
            y_max: 10.0,
        };
        for crop in [
            [1, 1, 1, 2],
            [1, 2, 3, 2],
            [2, 1, 1, 2],
            [1, 2, 3, 1],
            [-1, 0, 2, 2],
            [0, -1, 2, 2],
            [0, 0, 5, 2],
            [0, 0, 2, 4],
        ] {
            assert_eq!(
                crop_only_placement(QUADRANTS, crop, bbox),
                None,
                "crop {crop:?}"
            );
        }
    }

    #[test]
    fn rejects_invalid_destination_and_unreadable_image_dimensions() {
        for bbox in [
            BoundingBox {
                x_min: 0.0,
                y_min: 0.0,
                x_max: 0.0,
                y_max: 1.0,
            },
            BoundingBox {
                x_min: 1.0,
                y_min: 0.0,
                x_max: 0.0,
                y_max: 1.0,
            },
            BoundingBox {
                x_min: f64::NAN,
                y_min: 0.0,
                x_max: 1.0,
                y_max: 1.0,
            },
            BoundingBox {
                x_min: 0.0,
                y_min: 0.0,
                x_max: f64::INFINITY,
                y_max: 1.0,
            },
        ] {
            assert_eq!(crop_only_placement(QUADRANTS, [0, 0, 1, 1], bbox), None);
        }
        assert_eq!(
            crop_only_placement(
                b"not an image",
                [0, 0, 1, 1],
                BoundingBox {
                    x_min: 0.0,
                    y_min: 0.0,
                    x_max: 1.0,
                    y_max: 1.0,
                }
            ),
            None
        );
    }
}
