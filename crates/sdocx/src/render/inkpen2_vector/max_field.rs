//! Vector MAX coverage with isolated filter inputs for each native stamp.
//!
//! Packing preserves each center's fractional phase, so offsets are integral
//! in source coordinates. Explicit color-matrix crops are required because
//! resvg 0.47 does not implement `feOffset` subregion clipping.

use super::super::vector::{
    Circle, ColorMatrix, ColorValue, Definition, Definitions, Filter, FilterBlend, FilterInput,
    FilterOffset, FilterPrimitive, FilterRedByteMap, FilterResult, Gradient, Group, Mask, Paint,
    RadialGradient, Rectangle, ReplayPart, Scene, Stop, Styled, SvgFilter, SvgMask, Transform,
};
use crate::PreparedInkPen2V9;

const PADDING: f64 = 3.0;
const MAX_RECORDS: usize = 512;
const MAX_EXTENT: f64 = 16_384.0;
const MAX_AREA: f64 = 16.0 * 1024.0 * 1024.0;
const IDENTITY: [f64; 20] = [
    1., 0., 0., 0., 0., 0., 1., 0., 0., 0., 0., 0., 1., 0., 0., 0., 0., 0., 1., 0.,
];
const RED_TO_GRAY: [f64; 20] = [
    1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 0., 0., 0., 1., 0.,
];

fn exact_coverage_byte(coverage: f32) -> Option<u8> {
    let byte = (coverage * 255.0).round() as u8;
    (f32::from(byte) / 255.0 == coverage).then_some(byte)
}

#[derive(Clone, Copy)]
struct Region {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

struct Cell {
    center: [f64; 2],
    offset: [f64; 2],
    crop: Region,
    backplate: Region,
}

pub(super) struct MaxAtlas {
    cells: Vec<Cell>,
    filter: Region,
    output: Region,
}

impl MaxAtlas {
    pub(super) fn new(ink: &PreparedInkPen2V9) -> Result<Self, String> {
        if ink.records.is_empty() || ink.records.len() > MAX_RECORDS {
            return Err("InkPen2 MAX atlas record count exceeds its staged range".into());
        }
        let bounds = ink.bounds;
        if ![bounds.x_min, bounds.y_min, bounds.x_max, bounds.y_max]
            .into_iter()
            .all(f64::is_finite)
            || bounds.x_max <= bounds.x_min
            || bounds.y_max <= bounds.y_min
        {
            return Err("InkPen2 MAX atlas bounds are invalid".into());
        }
        let mut max_radius = [0.0_f64; 2];
        for &[cx, cy, rx, ry, edge, coverage, ..] in &ink.records {
            if ![cx, cy, rx, ry, edge, coverage]
                .into_iter()
                .all(f32::is_finite)
                || rx <= 0.0
                || ry <= 0.0
                || !(0.0..0.5).contains(&edge)
                || !(0.0..=1.0).contains(&coverage)
                || f64::from(cx) - f64::from(rx) < bounds.x_min
                || f64::from(cx) + f64::from(rx) > bounds.x_max
                || f64::from(cy) - f64::from(ry) < bounds.y_min
                || f64::from(cy) + f64::from(ry) > bounds.y_max
            {
                return Err("InkPen2 MAX atlas contains an invalid ellipse".into());
            }
            max_radius[0] = max_radius[0].max(f64::from(rx));
            max_radius[1] = max_radius[1].max(f64::from(ry));
        }

        let columns = (ink.records.len() as f64).sqrt().ceil() as usize;
        let rows = ink.records.len().div_ceil(columns);
        let pitch = max_radius.map(|r| (2.0 * r).ceil() + 2.0 * PADDING + 2.0);
        let origin = [bounds.x_min.floor(), bounds.y_min.floor()];
        let center_in_cell = max_radius.map(|r| r.ceil() + PADDING);
        let output = Region {
            x: bounds.x_min.floor() - PADDING,
            y: bounds.y_min.floor() - PADDING,
            width: bounds.x_max.ceil() - bounds.x_min.floor() + 2.0 * PADDING,
            height: bounds.y_max.ceil() - bounds.y_min.floor() + 2.0 * PADDING,
        };
        let filter_right = (origin[0] + columns as f64 * pitch[0]).max(output.x + output.width);
        let filter_bottom = (origin[1] + rows as f64 * pitch[1]).max(output.y + output.height);
        let filter = Region {
            x: origin[0].min(output.x),
            y: origin[1].min(output.y),
            width: filter_right - origin[0].min(output.x),
            height: filter_bottom - origin[1].min(output.y),
        };
        if ![filter.x, filter.y, filter.width, filter.height]
            .into_iter()
            .all(f64::is_finite)
            || filter.width > MAX_EXTENT
            || filter.height > MAX_EXTENT
            || filter.width * filter.height > MAX_AREA
        {
            return Err("InkPen2 MAX atlas exceeds its staged extent budget".into());
        }

        let cells = ink
            .records
            .iter()
            .enumerate()
            .map(|(index, &[cx, cy, rx, ry, ..])| {
                let source = [f64::from(cx), f64::from(cy)];
                let tile = [index % columns, index / columns];
                let center = std::array::from_fn(|axis| {
                    origin[axis]
                        + tile[axis] as f64 * pitch[axis]
                        + center_in_cell[axis]
                        + (source[axis] - source[axis].floor())
                });
                let offset = [source[0] - center[0], source[1] - center[1]];
                let x = (source[0] - f64::from(rx)).floor() - PADDING;
                let y = (source[1] - f64::from(ry)).floor() - PADDING;
                let crop = Region {
                    x,
                    y,
                    width: (source[0] + f64::from(rx)).ceil() + PADDING - x,
                    height: (source[1] + f64::from(ry)).ceil() + PADDING - y,
                };
                Cell {
                    center,
                    offset,
                    crop,
                    backplate: Region {
                        x: x - offset[0],
                        y: y - offset[1],
                        ..crop
                    },
                }
            })
            .collect();
        Ok(Self {
            cells,
            filter,
            output,
        })
    }

    pub(super) fn emit(
        &self,
        svg: &mut Scene,
        ink: &PreparedInkPen2V9,
        mask: &Definition<Mask>,
        replay: bool,
    ) {
        let filter_id = svg.definition::<Filter>();
        let region = self.filter;
        let mut filter =
            SvgFilter::new(&filter_id).region(region.x, region.y, region.width, region.height);
        let mut maximum = None;
        for (cell, record) in self.cells.iter().zip(&ink.records) {
            let raw = svg.definition::<FilterResult>();
            let field = svg.definition::<FilterResult>();
            let region = cell.crop;
            filter = filter.add(
                FilterOffset::new(cell.offset[0], cell.offset[1])
                    .input(FilterInput::SourceGraphic)
                    .result(&raw)
                    .region(region.x, region.y, region.width, region.height),
            );
            let (input, matrix) = if exact_coverage_byte(record[5]).is_some() {
                (FilterInput::Result(raw), IDENTITY)
            } else {
                // resvg rounds gradient stops to bytes but truncates color-
                // matrix output. Map the unit radial R8 field to native
                // rounded coverage explicitly, then copy red to gray. Exact
                // byte peaks (including the common coverage=1) need no map.
                let table = std::array::from_fn(|value| {
                    (f64::from(record[5]) * value as f64).round() as u8
                });
                let mapped = svg.definition::<FilterResult>();
                filter = filter.add(
                    FilterRedByteMap::new(FilterInput::Result(raw), &table)
                        .result(&mapped)
                        .region(region.x, region.y, region.width, region.height),
                );
                (FilterInput::Result(mapped), RED_TO_GRAY)
            };
            filter = filter.add(
                ColorMatrix::matrix(matrix)
                    .input(input)
                    .result(&field)
                    .region(region.x, region.y, region.width, region.height),
            );
            maximum = Some(if let Some(previous) = maximum {
                let result = svg.definition::<FilterResult>();
                let region = self.output;
                filter = filter.add(
                    FilterBlend::lighten(FilterInput::Result(previous), FilterInput::Result(field))
                        .result(&result)
                        .region(region.x, region.y, region.width, region.height),
                );
                result
            } else {
                field
            });
        }
        svg.push(filter);
        let (x, y, width, height) = super::field_region(ink);
        svg.scope(
            SvgMask::luminance(mask)
                .x(x)
                .y(y)
                .width(width)
                .height(height),
            |svg| {
                svg.scope(Group::new().filtered(&filter_id), |svg| {
                    for (index, (cell, &[_, _, rx, ry, edge, coverage, ..])) in
                        self.cells.iter().zip(&ink.records).enumerate()
                    {
                        let gradient = svg.definition::<Gradient>();
                        // An exactly representable byte peak preserves native
                        // float coverage through resvg's stop conversion.
                        // Other peaks use a unit ramp and the R8 map above.
                        let peak = exact_coverage_byte(coverage).unwrap_or(255);
                        let color = ColorValue::Rgb(crate::Color {
                            r: peak,
                            g: peak,
                            b: peak,
                        });
                        let field = RadialGradient::new(&gradient)
                            .cx(0)
                            .cy(0)
                            .r(1)
                            .add(Stop::new(0, color))
                            .add(Stop::new(f64::from(edge) * 2.0, color))
                            .add(Stop::new(1, ColorValue::BLACK));
                        let back = cell.backplate;
                        svg.scope(
                            Group::new().replay_part(replay.then_some(ReplayPart(index + 1))),
                            |svg| {
                                svg.push(
                                    Rectangle::new()
                                        .x(back.x)
                                        .y(back.y)
                                        .width(back.width)
                                        .height(back.height)
                                        .fill(Paint::BLACK),
                                );
                                svg.push(
                                    Group::new()
                                        .transformed(Transform::matrix_unrounded([
                                            f64::from(rx),
                                            0.,
                                            0.,
                                            f64::from(ry),
                                            cell.center[0],
                                            cell.center[1],
                                        ]))
                                        .add(Definitions::new().add(field))
                                        .add(
                                            Circle::new()
                                                .cx(0)
                                                .cy(0)
                                                .r(1)
                                                .fill(Paint::Gradient(gradient)),
                                        ),
                                );
                            },
                        );
                    }
                });
            },
        );
    }
}

#[cfg(test)]
#[path = "max_field_tests.rs"]
mod tests;
