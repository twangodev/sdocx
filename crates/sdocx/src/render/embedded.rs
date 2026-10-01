use super::{code::PreparedCode, table::PreparedTable};
use crate::BoundingBox;

use super::text::ObjectDiagnosticKind;

pub(super) fn cloned_raw_bounds(
    raw: BoundingBox,
    drawn: BoundingBox,
    target: BoundingBox,
) -> Result<BoundingBox, ObjectDiagnosticKind> {
    let coordinates = |rect: BoundingBox| {
        [rect.x_min, rect.y_min, rect.x_max, rect.y_max].map(|value| value as f32)
    };
    let raw = coordinates(raw);
    let drawn = coordinates(drawn);
    let target = coordinates(target);
    if !raw
        .into_iter()
        .chain(drawn)
        .chain(target)
        .all(f32::is_finite)
    {
        return Err(ObjectDiagnosticKind::InvalidBounds);
    }
    let dimensions = |rect: [f32; 4]| [rect[2] - rect[0], rect[3] - rect[1]];
    let drawn_size = dimensions(drawn);
    let target_size = dimensions(target);
    if drawn_size
        .into_iter()
        .chain(target_size)
        .any(|value| value <= 0.0 || !value.is_finite())
    {
        return Err(ObjectDiagnosticKind::InvalidBounds);
    }
    let same_size = drawn_size == target_size;
    let axis = |index: usize| {
        let translation = target[index] - drawn[index];
        if same_size {
            [raw[index] + translation, raw[index + 2] + translation]
        } else {
            let scale = target_size[index] / drawn_size[index];
            let translation = translation + drawn[index].mul_add(-scale, drawn[index]);
            [
                raw[index].mul_add(scale, 0.0) + translation,
                raw[index + 2].mul_add(scale, 0.0) + translation,
            ]
        }
    };
    let [left, right] = axis(0);
    let [top, bottom] = axis(1);
    if ![left, top, right, bottom].into_iter().all(f32::is_finite) || right <= left || bottom <= top
    {
        return Err(ObjectDiagnosticKind::InvalidBounds);
    }
    Ok(BoundingBox {
        x_min: f64::from(left),
        y_min: f64::from(top),
        x_max: f64::from(right),
        y_max: f64::from(bottom),
    })
}

pub(super) enum PreparedObject {
    Code(Box<PreparedCode>),
    Table(Box<PreparedTable>),
}

impl PreparedObject {
    pub fn width(&self) -> f64 {
        let bbox = match self {
            Self::Code(code) => code.panel_bbox,
            Self::Table(table) => table.measured_bbox,
        };
        f64::from(bbox.x_max as f32 - bbox.x_min as f32)
    }

    pub fn height(&self) -> f64 {
        let bbox = match self {
            Self::Code(code) => code.panel_bbox,
            Self::Table(table) => table.measured_bbox,
        };
        bbox.y_max - bbox.y_min
    }

    pub fn minimum_first_page_height(&self) -> Option<f64> {
        match self {
            Self::Code(code) => code.minimum_first_page_height(),
            Self::Table(table) => table.minimum_first_page_height(),
        }
    }
}
