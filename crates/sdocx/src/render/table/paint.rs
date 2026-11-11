use crate::render::table;
use crate::render::{
    Clip, ClipPath, Definitions, Group, Line, ObjectDiagnostic, ObjectDiagnosticKind, ObjectKind,
    Paint, RenderTheme, Scene, Styled, StyledText, TextContext, TextRenderer, argb_rgb, color_hex,
    decimal, paint_text_layout_with_target, rectangle, render_text_frame, text, viewport::Viewport,
};
use crate::{BoundingBox, MediaAsset};

pub(in crate::render) struct TablePaint<'a> {
    measured_bbox: BoundingBox,
    content_bbox: BoundingBox,
    rows: &'a [table::PreparedTableRow],
    pending_gaps: &'a [f64],
    artwork: table::TableArtworkGeometry,
}

impl<'a> From<&'a table::PreparedTable> for TablePaint<'a> {
    fn from(table: &'a table::PreparedTable) -> Self {
        Self {
            measured_bbox: table.measured_bbox,
            content_bbox: table.content_bbox,
            rows: &table.rows,
            pending_gaps: &table.pending_gaps,
            artwork: table::TableArtworkGeometry::Cached,
        }
    }
}

impl<'a> From<&'a table::PreparedTableDrawing> for TablePaint<'a> {
    fn from(table: &'a table::PreparedTableDrawing) -> Self {
        Self {
            measured_bbox: table.measured_bbox,
            content_bbox: table.content_bbox,
            rows: &table.rows,
            pending_gaps: &table.pending_gaps,
            artwork: table.artwork,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::render) fn render_table(
    svg: &mut Scene,
    table: &crate::RichTextTable,
    anchor_utf16: i32,
    prepared: Option<TablePaint<'_>>,
    offset_y: f64,
    media_assets: &[MediaAsset],
    theme: RenderTheme,
    renderer: &TextRenderer<'_>,
    viewport: Option<Viewport>,
) {
    let table_bbox = prepared
        .as_ref()
        .map_or(table.bbox, |table| table.measured_bbox);
    let artwork = prepared
        .as_ref()
        .map_or(table::TableArtworkGeometry::Unprepared, |prepared| {
            prepared.artwork
        });
    let grid = table::TableGrid::new(table);
    if prepared.is_none()
        && (grid
            .as_ref()
            .map_or(true, |grid| grid.requires_merged_layout())
            || !table::supports_prepared_content(table))
    {
        renderer.report_object_issues(&[ObjectDiagnostic {
            anchor_utf16,
            kind: ObjectDiagnosticKind::UnsupportedContent,
        }]);
    }
    let borders =
        grid.as_ref()
            .ok()
            .and_then(|grid| match table::TableBorderGeometry::new(table, grid) {
                Ok(borders) => Some(borders),
                Err(kind) => {
                    renderer.report_object_issues(&[ObjectDiagnostic { anchor_utf16, kind }]);
                    None
                }
            });
    let outline = borders
        .as_ref()
        .map(|borders| borders.outline())
        .unwrap_or_default();
    let content_bbox = prepared
        .as_ref()
        .map_or(table.style.content_bbox.unwrap_or(table.bbox), |prepared| {
            prepared.content_bbox
        });
    let mut cells = Vec::new();
    let mut add_cell = |row: usize, column: usize| {
        let cell = &table.rows[row].cells[column];
        let measured = if let Some(prepared) = &prepared {
            let Some(measured) = prepared
                .rows
                .get(row)
                .filter(|prepared| prepared.row_index == row)
                .and_then(|row| {
                    row.cells
                        .get(column)
                        .filter(|cell| cell.column_index == column)
                })
            else {
                return;
            };
            Some(measured)
        } else {
            None
        };
        let frame = measured.map_or(cell.bbox, |cell| cell.frame);
        let artwork_frame = artwork.background(cell.bbox, frame);
        if !artwork.cached()
            && (!table::finite_artwork_rect(cell.bbox)
                || !table::finite_artwork_rect(artwork_frame))
        {
            renderer.report_object_issues(&[ObjectDiagnostic {
                anchor_utf16,
                kind: ObjectDiagnosticKind::InvalidBounds,
            }]);
            return;
        }
        cells.push(TableCellPaint {
            cell,
            fill: table::CellFill::resolve(&table.style, table.rows[row].index, cell, theme),
            frame,
            artwork_frame,
            layout: measured.map(|cell| &cell.layout),
            native_text_placement: measured.map_or(
                Ok(table::native_placement::NativeCellTextPlacement::Unknown),
                |cell| cell.native_text_placement(),
            ),
            position: table::CellPosition { row, column },
            gap: prepared.as_ref().is_some_and(|prepared| {
                prepared
                    .pending_gaps
                    .get(row + 1)
                    .is_some_and(|gap| (*gap as f32).abs() > 0.001_f32)
            }),
        });
    };
    match &grid {
        Ok(grid) => {
            for position in grid.visible_cells() {
                add_cell(position.row, position.column);
            }
        }
        Err(_) => {
            for (row_index, row) in table.rows.iter().enumerate() {
                for column in 0..row.cells.len() {
                    add_cell(row_index, column);
                }
            }
        }
    }
    svg.scope(Group::new().object(ObjectKind::Table), |svg| {
        let clip = svg.definition::<Clip>();
        let artwork_clip = svg.definition::<Clip>();
        let measured_bounds = BoundingBox {
            y_min: table_bbox.y_min + offset_y,
            y_max: table_bbox.y_max + offset_y,
            ..table_bbox
        };
        let artwork_bounds = if prepared.is_some() {
            table::artwork_bounds(measured_bounds, renderer.table_export_page)
        } else {
            measured_bounds
        };
        svg.push(
            Definitions::new()
                .add(ClipPath::new(&clip).add(rectangle(measured_bounds, 0.0, 2)))
                .add(ClipPath::new(&artwork_clip).add(rectangle(artwork_bounds, 0.0, 5))),
        );
        svg.scope(Group::new().clipped(&artwork_clip), |svg| {
            let shape = [table.rows.len(), table.column_widths.len()];
            for paint in &cells {
                let border_paths = borders
                    .as_ref()
                    .map_or_else(Vec::new, |borders| borders.cell_paths(paint.position));
                if viewport.is_some_and(|viewport| {
                    !viewport.intersects(paint.artwork_bounds(
                        &border_paths,
                        offset_y,
                        prepared.is_some(),
                        artwork,
                    ))
                }) {
                    continue;
                }
                paint_table_cell_background(svg, paint, shape, [outline.rx, outline.ry], offset_y);
                for path in border_paths {
                    if path.selected(paint.position, shape, outline.active(), paint.gap) {
                        let path = artwork.border(path, paint.artwork_frame);
                        if !path.has_finite_endpoints() {
                            renderer.report_object_issues(&[ObjectDiagnostic {
                                anchor_utf16,
                                kind: ObjectDiagnosticKind::InvalidBounds,
                            }]);
                            continue;
                        }
                        paint_table_border(svg, path, offset_y, theme);
                    }
                }
            }
            if prepared.is_some() {
                paint_table_outline(svg, borders.as_ref(), content_bbox, offset_y, theme);
            }
        });
        if prepared.is_none() {
            paint_table_outline(svg, borders.as_ref(), content_bbox, offset_y, theme);
        }
        for paint in &cells {
            let native_clip = paint
                .native_text_placement
                .is_ok_and(|placement| placement.transport().is_some());
            if !native_clip
                && viewport.is_some_and(|viewport| {
                    !viewport.intersects(BoundingBox {
                        y_min: paint.frame.y_min + offset_y,
                        y_max: paint.frame.y_max + offset_y,
                        ..paint.frame
                    })
                })
            {
                continue;
            }
            let cell_theme = paint.fill.theme(theme);
            if let Some(layout) = paint.layout {
                let styled =
                    StyledText::new(&paint.cell.content, TextContext::Flow, renderer.settings);
                let mut target = text::NativePaintTarget::new(svg.retains_text(), viewport)
                    .with_cell(anchor_utf16, paint.native_text_placement);
                target.conservative_clip = Some(clip);
                paint_text_layout_with_target(
                    svg,
                    &styled,
                    layout,
                    media_assets,
                    cell_theme,
                    renderer,
                    target,
                );
            } else {
                let frame = text::TextFrame {
                    bbox: BoundingBox {
                        y_min: paint.frame.y_min + offset_y,
                        y_max: paint.frame.y_max + offset_y,
                        ..paint.frame
                    },
                    gravity: Some(0),
                    exclusions: &[],
                };
                svg.scope(Group::new().clipped(&clip), |svg| {
                    render_text_frame(
                        svg,
                        &paint.cell.content,
                        frame,
                        media_assets,
                        cell_theme,
                        renderer,
                        viewport,
                    );
                });
            }
        }
    });
}

struct TableCellPaint<'a> {
    cell: &'a crate::RichTextTableCell,
    fill: table::CellFill,
    frame: BoundingBox,
    artwork_frame: BoundingBox,
    layout: Option<&'a text::TextLayout>,
    native_text_placement: Result<
        table::native_placement::NativeCellTextPlacement,
        text::native_cell_clip::NativeCellClipError,
    >,
    position: table::CellPosition,
    gap: bool,
}

impl TableCellPaint<'_> {
    fn artwork_bounds(
        &self,
        border_paths: &[table::BorderPath],
        offset_y: f64,
        prepared: bool,
        artwork: table::TableArtworkGeometry,
    ) -> BoundingBox {
        let guard = if prepared {
            border_paths
                .iter()
                .filter_map(|path| path.paint_width())
                .fold(0.0_f32, f32::max)
                / 2.0
        } else {
            0.0
        };
        let guard = f64::from(guard);
        let mut bounds = BoundingBox {
            x_min: self.artwork_frame.x_min - guard,
            y_min: self.artwork_frame.y_min + offset_y - guard,
            x_max: self.artwork_frame.x_max + guard,
            y_max: self.artwork_frame.y_max + offset_y + guard,
        };
        if matches!(artwork, table::TableArtworkGeometry::Saved { .. }) {
            for &path in border_paths {
                let Some(width) = path.paint_width() else {
                    continue;
                };
                let path = artwork.border(path, self.artwork_frame);
                if !path.has_finite_endpoints() {
                    continue;
                }
                let [x1, y1, x2, y2] = path.endpoints.map(f64::from);
                let guard = f64::from(width) / 2.0;
                bounds.x_min = bounds.x_min.min(x1.min(x2) - guard);
                bounds.y_min = bounds.y_min.min(y1.min(y2) + offset_y - guard);
                bounds.x_max = bounds.x_max.max(x1.max(x2) + guard);
                bounds.y_max = bounds.y_max.max(y1.max(y2) + offset_y + guard);
            }
        }
        bounds
    }
}

fn paint_table_outline(
    svg: &mut Scene,
    borders: Option<&table::TableBorderGeometry>,
    bounds: BoundingBox,
    offset_y: f64,
    theme: RenderTheme,
) {
    let Some(borders) = borders else { return };
    let outline = borders.outline();
    if !outline.active() {
        return;
    }
    if outline.rounded() {
        svg.push(
            rectangle(bounds, offset_y, 5)
                .rx(decimal(f64::from(outline.rx), 5))
                .ry(decimal(f64::from(outline.ry), 5))
                .fill(Paint::None)
                .stroke(Paint::from_hex(&color_hex(
                    &theme.foreground_color(argb_rgb(outline.color)),
                )))
                .stroke_opacity(decimal(f64::from(outline.color >> 24) / 255.0, 6))
                .stroke_width(decimal(f64::from(outline.width), 5)),
        );
    } else {
        for path in borders.outer_paths(Some(bounds)) {
            paint_table_border(svg, path, offset_y, theme);
        }
    }
}

fn paint_table_cell_background(
    svg: &mut Scene,
    paint: &TableCellPaint<'_>,
    shape: [usize; 2],
    radii: [f32; 2],
    offset_y: f64,
) {
    let fill = Paint::from_hex(&color_hex(&paint.fill.color));
    let rectangle = |bounds, offset_y, places| {
        rectangle(bounds, offset_y, places).fill_opacity(decimal(paint.fill.opacity, 6))
    };
    let geometry = table::CellBackgroundGeometry::new(
        paint.artwork_frame,
        paint.position,
        [paint.cell.row_span, paint.cell.column_span],
        shape,
        radii,
    );
    let background = rectangle(geometry.bounds, offset_y, 5).fill(fill);
    if let Some([rx, ry]) = geometry.radii {
        svg.push(
            background
                .rx(decimal(f64::from(rx), 5))
                .ry(decimal(f64::from(ry), 5)),
        );
    } else {
        svg.push(background);
    }
    for bounds in geometry.square_corners.into_iter().flatten() {
        svg.push(rectangle(bounds, offset_y, 5).fill(fill));
    }
}

pub(super) fn paint_table_border(
    svg: &mut Scene,
    path: table::BorderPath,
    offset_y: f64,
    theme: RenderTheme,
) {
    let Some(width) = path.paint_width() else {
        return;
    };
    let [x1, y1, x2, y2] = path.endpoints.map(f64::from);
    svg.push(
        Line::new()
            .x1(x1)
            .y1(y1 + offset_y)
            .x2(x2)
            .y2(y2 + offset_y)
            .stroke(Paint::from_hex(&color_hex(
                &theme.foreground_color(argb_rgb(path.style.color)),
            )))
            .stroke_opacity(decimal(f64::from(path.style.color >> 24) / 255.0, 6))
            .stroke_width(width),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    fn bounds([left, top, right, bottom]: [f32; 4]) -> BoundingBox {
        BoundingBox {
            x_min: f64::from(left),
            y_min: f64::from(top),
            x_max: f64::from(right),
            y_max: f64::from(bottom),
        }
    }
    #[test]
    fn artwork_visibility_includes_border_ink_beyond_the_cell_frame() {
        use super::table::{CellFill, CellPosition, TableBorderGeometry, TableGrid, tests::grid};

        let table = grid(&[100.0], &[100.0]);
        let grid = TableGrid::new(&table).unwrap();
        let borders = TableBorderGeometry::new(&table, &grid).unwrap();
        let cell = &table.rows[0].cells[0];
        let mut paint = TableCellPaint {
            cell,
            fill: CellFill::resolve(&table.style, 0, cell, RenderTheme::for_canvas(false)),
            frame: bounds([0.0, -100.0, 100.0, -0.25]),
            artwork_frame: bounds([0.0, -100.0, 100.0, -0.25]),
            layout: None,
            position: CellPosition { row: 0, column: 0 },
            gap: false,
            native_text_placement: Ok(table::native_placement::NativeCellTextPlacement::Unknown),
        };
        let viewport = Viewport::new(bounds([0.0, 0.0, 100.0, 100.0]));
        assert!(!viewport.intersects(paint.frame));
        let border_paths = borders.cell_paths(paint.position);
        assert!(viewport.intersects(paint.artwork_bounds(
            &border_paths,
            0.0,
            true,
            table::TableArtworkGeometry::Cached
        )));
        paint.artwork_frame.y_max = -0.5;
        assert!(!viewport.intersects(paint.artwork_bounds(
            &border_paths,
            0.0,
            true,
            table::TableArtworkGeometry::Cached
        )));
    }
}
