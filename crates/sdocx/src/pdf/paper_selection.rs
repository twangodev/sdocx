//! Native-compatible stroke query admission prototype.
//!
//! This is test-only and deliberately not connected to PDF paper export. It covers only
//! ordinary saved strokes with retained source metadata and unrotated sample geometry.

use crate::{BoundingBox, Stroke};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StrokeAdmission {
    Admit,
    Reject,
    Unsupported(UnsupportedReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum UnsupportedReason {
    MissingSavedMetadata,
    MismatchedBoundingBox,
    RotatedStroke,
    InvalidGeometry,
    NegativePenSize,
}

/// Reproduces the verified ordinary `ObjectStroke::getSelectionRect` and partial-sample
/// admission path. Query bounds and all native geometry are interpreted as f32.
pub(super) fn admit_stroke(stroke: &Stroke, query: [f32; 4]) -> StrokeAdmission {
    let Some(rendering) = stroke.rendering.as_ref() else {
        return StrokeAdmission::Unsupported(UnsupportedReason::MissingSavedMetadata);
    };
    let Some(metadata) = rendering.metadata.as_deref() else {
        return StrokeAdmission::Unsupported(UnsupportedReason::MissingSavedMetadata);
    };
    let Some(source_bbox) = rect_from_bbox(metadata.bbox) else {
        return StrokeAdmission::Unsupported(UnsupportedReason::InvalidGeometry);
    };
    let Some(decoded_bbox) = rect_from_bbox(stroke.bbox) else {
        return StrokeAdmission::Unsupported(UnsupportedReason::InvalidGeometry);
    };
    if source_bbox.map(f32::to_bits) != decoded_bbox.map(f32::to_bits) {
        return StrokeAdmission::Unsupported(UnsupportedReason::MismatchedBoundingBox);
    }
    if metadata
        .rotation_degrees
        .is_some_and(|rotation| !rotation.is_finite() || rotation != 0.0)
    {
        return StrokeAdmission::Unsupported(UnsupportedReason::RotatedStroke);
    }

    let raw_size = rendering.style.pen_size.unwrap_or(0.0);
    if !raw_size.is_finite() || raw_size < 0.0 {
        return StrokeAdmission::Unsupported(UnsupportedReason::NegativePenSize);
    }
    let half_size = raw_size * 0.5;
    if !half_size.is_finite() {
        return StrokeAdmission::Unsupported(UnsupportedReason::InvalidGeometry);
    }

    let Some(selection) = native_selection_rect(source_bbox, half_size) else {
        return StrokeAdmission::Unsupported(UnsupportedReason::InvalidGeometry);
    };
    if !valid_rect(query) {
        return StrokeAdmission::Unsupported(UnsupportedReason::InvalidGeometry);
    }

    let points = stroke
        .points
        .iter()
        .map(|point| [point.x as f32, point.y as f32])
        .collect::<Vec<_>>();
    if points.iter().flatten().any(|value| !value.is_finite()) {
        return StrokeAdmission::Unsupported(UnsupportedReason::InvalidGeometry);
    }
    let Some(segment_rects) = points
        .windows(2)
        .map(|segment| expanded_segment(segment[0], segment[1], half_size))
        .collect::<Option<Vec<_>>>()
    else {
        return StrokeAdmission::Unsupported(UnsupportedReason::InvalidGeometry);
    };

    // IsIntersect admits a query that fully contains the selection rectangle, including
    // equality. Partial intersections use strict overlap plus sample-segment bounds.
    if contains(query, selection) {
        StrokeAdmission::Admit
    } else {
        let Some(overlap) = intersection(query, selection) else {
            return StrokeAdmission::Reject;
        };
        if segment_rects
            .iter()
            .any(|segment| strict_intersection(*segment, overlap))
        {
            StrokeAdmission::Admit
        } else {
            StrokeAdmission::Reject
        }
    }
}

fn rect_from_bbox(bbox: BoundingBox) -> Option<[f32; 4]> {
    let rect = [bbox.x_min, bbox.y_min, bbox.x_max, bbox.y_max].map(|value| value as f32);
    valid_rect(rect).then_some(rect)
}

fn native_selection_rect(bbox: [f32; 4], half_size: f32) -> Option<[f32; 4]> {
    let mut base = bbox;
    // Native GetRect gives a zero-width/height saved rectangle one unit of extent.
    if base[0] == base[2] {
        base[2] += 1.0;
    }
    if base[1] == base[3] {
        base[3] += 1.0;
    }
    let selection = [
        base[0] - half_size,
        base[1] - half_size,
        base[2] + half_size,
        base[3] + half_size,
    ];
    valid_rect(selection).then_some(selection)
}

fn valid_rect(rect: [f32; 4]) -> bool {
    rect.into_iter().all(f32::is_finite) && rect[0] <= rect[2] && rect[1] <= rect[3]
}

fn contains(outer: [f32; 4], inner: [f32; 4]) -> bool {
    outer[0] <= inner[0] && outer[1] <= inner[1] && outer[2] >= inner[2] && outer[3] >= inner[3]
}

fn intersection(left: [f32; 4], right: [f32; 4]) -> Option<[f32; 4]> {
    let overlap = [
        left[0].max(right[0]),
        left[1].max(right[1]),
        left[2].min(right[2]),
        left[3].min(right[3]),
    ];
    (overlap[0] < overlap[2] && overlap[1] < overlap[3]).then_some(overlap)
}

fn expanded_segment(start: [f32; 2], end: [f32; 2], half_size: f32) -> Option<[f32; 4]> {
    let segment = [
        start[0].min(end[0]) - half_size,
        start[1].min(end[1]) - half_size,
        start[0].max(end[0]) + half_size,
        start[1].max(end[1]) + half_size,
    ];
    valid_rect(segment).then_some(segment)
}

fn strict_intersection(left: [f32; 4], right: [f32; 4]) -> bool {
    // Native 0x2e6f14–0x2e6f54 calls overlap.IsIntersect(segment). The
    // overlap has positive area; a zero-height/width segment can lie inside it.
    left[0] < right[2] && right[0] < left[2] && left[1] < right[3] && right[1] < left[3]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ObjectMetadata, StrokeProperties, StrokeRendering, StrokeStyle};
    use serde_json::Value;

    const FIXTURE: &str = include_str!("../../../../conformance/pdf-stroke-selection-native.json");

    fn bits<const N: usize>(values: &Value) -> [f32; N] {
        values
            .as_array()
            .unwrap()
            .iter()
            .map(|value| u32::from_str_radix(value.as_str().unwrap(), 16).unwrap())
            .map(f32::from_bits)
            .collect::<Vec<_>>()
            .try_into()
            .unwrap()
    }

    fn metadata(bbox: BoundingBox, rotation_degrees: Option<f64>) -> ObjectMetadata {
        ObjectMetadata {
            format_version: 0,
            uuid: String::new(),
            modified_time_raw: 0,
            bbox,
            replay_timestamp_raw: 0,
            resize_mode_raw: 0,
            rotatable: true,
            selectable: true,
            movable: true,
            visible: true,
            replayable: true,
            out_of_canvas_enabled: false,
            template: false,
            flip_enabled: false,
            float_drawn_rect: false,
            locked: false,
            removable: true,
            rotation_degrees,
            property_mask: Vec::new(),
            field_mask: Vec::new(),
            fixed_trailing_data: Vec::new(),
            flexible_trailing_data: Vec::new(),
        }
    }

    fn stroke(bbox: BoundingBox, samples: &[[f64; 2]], pen_size: Option<f32>) -> Stroke {
        let saved = metadata(bbox, None);
        let properties = StrokeProperties {
            compressed: false,
            replay_only: false,
            stylus_channels: false,
            eraser: false,
            fixed_width: false,
            millisecond_timestamps: false,
            top_layer_pen: false,
            alpha_lock: false,
            binary_added: true,
            generated: false,
            fixed_opacity: false,
            rainbow_effect: false,
            straighten: false,
            reveal_mode: false,
        };
        Stroke {
            rendering: Some(StrokeRendering {
                metadata: Some(Box::new(saved)),
                pen_name: None,
                advanced_settings: None,
                tool_type_raw: 0,
                properties,
                style: StrokeStyle {
                    pen_size,
                    ..StrokeStyle::default()
                },
            }),
            bbox,
            points: samples
                .iter()
                .map(|p| crate::Point { x: p[0], y: p[1] })
                .collect(),
            pressures: Vec::new(),
            timestamps: Vec::new(),
            tilts: Vec::new(),
            orientations: Vec::new(),
            color: None,
            pen_width: 0.8,
        }
    }

    fn fixture_stroke(fixture: &Value) -> Stroke {
        let bbox_bits = bits::<4>(&fixture["bbox_f32_bits"]);
        let bbox = BoundingBox {
            x_min: f64::from(bbox_bits[0]),
            y_min: f64::from(bbox_bits[1]),
            x_max: f64::from(bbox_bits[2]),
            y_max: f64::from(bbox_bits[3]),
        };
        let samples = fixture["samples_f32_bits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                let point = bits::<2>(p);
                [f64::from(point[0]), f64::from(point[1])]
            })
            .collect::<Vec<_>>();
        let raw_size = f32::from_bits(
            u32::from_str_radix(fixture["raw_pen_size_f32_bits"].as_str().unwrap(), 16).unwrap(),
        );
        stroke(bbox, &samples, Some(raw_size))
    }

    #[test]
    fn native_fixture_covers_containment_partial_sample_and_strict_edges() {
        let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
        let source = fixture_stroke(&fixture);
        for case in fixture["queries"].as_array().unwrap() {
            let query = bits::<4>(&case["bits"]);
            let expected = case["admitted"].as_bool().unwrap();
            assert_eq!(
                admit_stroke(&source, query),
                if expected {
                    StrokeAdmission::Admit
                } else {
                    StrokeAdmission::Reject
                },
                "{}",
                case["name"].as_str().unwrap()
            );
        }
        let bbox = rect_from_bbox(source.bbox).unwrap();
        let half_size = source.rendering.as_ref().unwrap().style.pen_size.unwrap() * 0.5;
        assert_eq!(
            native_selection_rect(bbox, half_size),
            Some(bits::<4>(&fixture["selection_f32_bits"]))
        );
        assert_eq!(
            admit_stroke(&source, bits::<4>(&fixture["selection_f32_bits"])),
            StrokeAdmission::Admit
        );
    }

    #[test]
    fn native_one_point_control_and_unverified_inputs_fail_closed() {
        let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
        let one = &fixture["one_point"];
        let bbox_values = bits::<4>(&one["bbox_f32_bits"]);
        let bbox = BoundingBox {
            x_min: f64::from(bbox_values[0]),
            y_min: f64::from(bbox_values[1]),
            x_max: f64::from(bbox_values[2]),
            y_max: f64::from(bbox_values[3]),
        };
        let sample = one["samples_f32_bits"].as_array().unwrap()[0]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| {
                f64::from(f32::from_bits(
                    u32::from_str_radix(v.as_str().unwrap(), 16).unwrap(),
                ))
            })
            .collect::<Vec<_>>();
        let point = stroke(bbox, &[[sample[0], sample[1]]], Some(2.0));
        assert_eq!(
            admit_stroke(&point, bits::<4>(&one["contained_query_bits"])),
            StrokeAdmission::Admit
        );
        assert_eq!(
            admit_stroke(&point, bits::<4>(&one["partial_query_bits"])),
            StrokeAdmission::Reject
        );

        let zero_segment = &fixture["zero_size_segment"];
        let bbox = bits::<4>(&zero_segment["bbox_f32_bits"]);
        let bbox = BoundingBox {
            x_min: f64::from(bbox[0]),
            y_min: f64::from(bbox[1]),
            x_max: f64::from(bbox[2]),
            y_max: f64::from(bbox[3]),
        };
        let samples = zero_segment["samples_f32_bits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                let point = bits::<2>(p);
                [f64::from(point[0]), f64::from(point[1])]
            })
            .collect::<Vec<_>>();
        let horizontal = stroke(bbox, &samples, Some(0.0));
        assert_eq!(
            admit_stroke(&horizontal, bits::<4>(&zero_segment["query_f32_bits"])),
            if zero_segment["admitted"].as_bool().unwrap() {
                StrokeAdmission::Admit
            } else {
                StrokeAdmission::Reject
            }
        );

        for case in fixture["raw_size_controls"].as_array().unwrap() {
            let raw_size =
                f32::from_bits(u32::from_str_radix(case["input"].as_str().unwrap(), 16).unwrap());
            let getter =
                f32::from_bits(u32::from_str_radix(case["getter"].as_str().unwrap(), 16).unwrap());
            assert_eq!(raw_size.to_bits(), getter.to_bits());
            let expected = bits::<4>(&case["selection"]);
            assert_eq!(
                native_selection_rect([0.0, 0.0, 10.0, 0.0], raw_size * 0.5),
                Some(expected)
            );
        }
        let zero_extent = BoundingBox {
            x_min: 0.0,
            y_min: 0.0,
            x_max: 10.0,
            y_max: 0.0,
        };
        let missing_size = stroke(zero_extent, &[[0.0, 0.0], [10.0, 0.0]], None);
        assert_eq!(
            native_selection_rect(rect_from_bbox(zero_extent).unwrap(), 0.0),
            Some(bits::<4>(&fixture["raw_size_controls"][0]["selection"]))
        );
        assert_eq!(
            admit_stroke(
                &missing_size,
                bits::<4>(&fixture["raw_size_controls"][0]["selection"])
            ),
            StrokeAdmission::Admit
        );

        let mut rotated = fixture_stroke(&fixture);
        rotated
            .rendering
            .as_mut()
            .unwrap()
            .metadata
            .as_mut()
            .unwrap()
            .rotation_degrees = Some(5.0);
        assert_eq!(
            admit_stroke(&rotated, [-2.0, -2.0, 12.0, 3.0]),
            StrokeAdmission::Unsupported(UnsupportedReason::RotatedStroke)
        );
        let mut missing = fixture_stroke(&fixture);
        missing.rendering = None;
        assert_eq!(
            admit_stroke(&missing, [-2.0, -2.0, 12.0, 3.0]),
            StrokeAdmission::Unsupported(UnsupportedReason::MissingSavedMetadata)
        );
        let mut mismatch = fixture_stroke(&fixture);
        mismatch.bbox.x_max += 1.0;
        assert_eq!(
            admit_stroke(&mismatch, [-2.0, -2.0, 12.0, 3.0]),
            StrokeAdmission::Unsupported(UnsupportedReason::MismatchedBoundingBox)
        );
        let mut nonfinite = fixture_stroke(&fixture);
        nonfinite.points[0].x = f64::NAN;
        assert_eq!(
            admit_stroke(&nonfinite, [-2.0, -2.0, 12.0, 3.0]),
            StrokeAdmission::Unsupported(UnsupportedReason::InvalidGeometry)
        );

        let maximum = f64::from(f32::MAX);
        let overflow = stroke(
            BoundingBox {
                x_min: 0.0,
                y_min: 0.0,
                x_max: 0.0,
                y_max: 0.0,
            },
            &[[maximum, 0.0], [maximum, 0.0]],
            Some(f32::MAX),
        );
        assert_eq!(
            admit_stroke(&overflow, [-f32::MAX, -f32::MAX, f32::MAX, f32::MAX]),
            StrokeAdmission::Unsupported(UnsupportedReason::InvalidGeometry)
        );
    }
}
