use crate::{BoundingBox, ObjectSpanLayoutConstraint};

use super::{BorderPath, CellPosition, ObjectDiagnosticKind};

#[derive(Clone, Copy)]
pub(in crate::render) enum TableArtworkGeometry {
    Unprepared,
    Saved {
        source_origin: [f32; 2],
        paint_offset: [f32; 2],
    },
    Cached,
}

impl TableArtworkGeometry {
    pub fn compatibility_export(
        constraint: ObjectSpanLayoutConstraint,
        source_bounds: BoundingBox,
        offset: [f64; 2],
    ) -> Result<Self, ObjectDiagnosticKind> {
        match constraint {
            ObjectSpanLayoutConstraint::Normal => {
                let source_origin = [source_bounds.x_min as f32, source_bounds.y_min as f32];
                let paint_offset = offset.map(|coordinate| coordinate as f32);
                if !source_origin
                    .into_iter()
                    .chain(paint_offset)
                    .all(f32::is_finite)
                {
                    return Err(ObjectDiagnosticKind::InvalidBounds);
                }
                Ok(Self::Saved {
                    source_origin,
                    paint_offset,
                })
            }
            ObjectSpanLayoutConstraint::OverPages
            | ObjectSpanLayoutConstraint::OverPagesOverlapPadding => Ok(Self::Cached),
            _ => Err(ObjectDiagnosticKind::UnsupportedContent),
        }
    }

    #[cfg(test)]
    fn native_drawing(
        constraint: ObjectSpanLayoutConstraint,
        source_drawn_bounds: BoundingBox,
        scroll_x: f32,
    ) -> Result<Self, ObjectDiagnosticKind> {
        Self::compatibility_export(constraint, source_drawn_bounds, [f64::from(scroll_x), 0.0])
    }

    pub fn cached(self) -> bool {
        matches!(self, Self::Cached)
    }

    pub fn background(self, saved: BoundingBox, cached: BoundingBox) -> BoundingBox {
        match self {
            Self::Unprepared => saved,
            Self::Saved {
                source_origin,
                paint_offset,
            } => {
                let [left, top, right, bottom] = native_rect(saved);
                rect([
                    (left - source_origin[0]) + paint_offset[0],
                    (top - source_origin[1]) + paint_offset[1],
                    (right - source_origin[0]) + paint_offset[0],
                    (bottom - source_origin[1]) + paint_offset[1],
                ])
            }
            Self::Cached => cached,
        }
    }

    pub fn border(self, path: BorderPath, frame: BoundingBox) -> BorderPath {
        match self {
            Self::Unprepared => path,
            Self::Saved {
                source_origin,
                paint_offset,
            } => {
                let [x1, y1, x2, y2] = path.endpoints;
                let offset = [
                    source_origin[0] - paint_offset[0],
                    source_origin[1] - paint_offset[1],
                ];
                BorderPath {
                    endpoints: [
                        x1 - offset[0],
                        y1 - offset[1],
                        x2 - offset[0],
                        y2 - offset[1],
                    ],
                    ..path
                }
            }
            Self::Cached => path.on_frame(frame),
        }
    }
}

#[cfg(test)]
fn offset_frame(frame: BoundingBox, origin: [f64; 2]) -> Result<BoundingBox, ObjectDiagnosticKind> {
    let [left, top, right, bottom] = native_rect(frame);
    let [x, y] = origin.map(|coordinate| coordinate as f32);
    let coordinates = [left + x, top + y, right + x, bottom + y];
    if !coordinates.into_iter().all(f32::is_finite) {
        return Err(ObjectDiagnosticKind::InvalidBounds);
    }
    Ok(rect(coordinates))
}

pub(in crate::render) struct CellBackgroundGeometry {
    pub bounds: BoundingBox,
    pub radii: Option<[f32; 2]>,
    pub square_corners: [Option<BoundingBox>; 4],
}

impl CellBackgroundGeometry {
    pub fn new(
        bounds: BoundingBox,
        position: CellPosition,
        span: [u32; 2],
        shape: [usize; 2],
        radii: [f32; 2],
    ) -> Self {
        let [rx, ry] = radii;
        let first_row = position.row == 0;
        let first_column = position.column == 0;
        let last_row = position.row.checked_add(span[0] as usize) == Some(shape[0]);
        let last_column = position.column.checked_add(span[1] as usize) == Some(shape[1]);
        let corners = [
            first_row && first_column,
            first_row && last_column,
            last_row && last_column,
            last_row && first_column,
        ];
        if rx <= 0.0 || ry <= 0.0 || !corners.into_iter().any(|corner| corner) {
            return Self {
                bounds,
                radii: None,
                square_corners: [None; 4],
            };
        }
        let [left, top, right, bottom] = native_rect(bounds);
        let patches = [
            [left, top, left + rx, top + ry],
            [right - rx, top, right, top + ry],
            [right - rx, bottom - ry, right, bottom],
            [left, bottom - ry, left + rx, bottom],
        ];
        let square_corners =
            std::array::from_fn(|index| (!corners[index]).then(|| rect(patches[index])));
        Self {
            bounds,
            radii: Some(radii),
            square_corners,
        }
    }
}

fn native_rect(rect: BoundingBox) -> [f32; 4] {
    [rect.x_min, rect.y_min, rect.x_max, rect.y_max].map(|coordinate| coordinate as f32)
}

pub(in crate::render) fn finite_artwork_rect(rect: BoundingBox) -> bool {
    native_rect(rect).into_iter().all(f32::is_finite)
}

fn rect([x_min, y_min, x_max, y_max]: [f32; 4]) -> BoundingBox {
    BoundingBox {
        x_min: f64::from(x_min),
        y_min: f64::from(y_min),
        x_max: f64::from(x_max),
        y_max: f64::from(y_max),
    }
}

#[cfg(test)]
mod native_tests;

#[cfg(test)]
mod clone_origin_tests;
