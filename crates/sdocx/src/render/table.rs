use crate::{
    BoundingBox, ObjectSpanLayoutConstraint, RichTextTable, RichTextTableCell, TableBorder,
};

use super::RenderTheme;
use super::text::{
    ObjectDiagnosticKind, StyledText, TextContext, TextFrame, TextLayout, TextRenderer,
    VerticalExclusion, finite_native_geometry,
};

mod borders;
mod export;
mod fills;
mod grid;
mod pagination;

#[cfg(test)]
mod native_geometry_tests;

pub(super) use borders::{BorderPath, TableBorderGeometry};
pub(super) use export::{TableExportPage, artwork_bounds};
pub(super) use fills::CellFill;
pub(super) use grid::{CellPosition, TableGrid};

use borders::Edge;

use pagination::BandList;

pub(super) struct PreparedTable {
    pub measured_bbox: BoundingBox,
    pub content_bbox: BoundingBox,
    pub rows: Vec<PreparedTableRow>,
    pub min_first_page_height: f64,
    constraint: ObjectSpanLayoutConstraint,
    bands: BandList,
    pub pending_gaps: Vec<f64>,
    half_border: f64,
    topology: TableGrid,
}

pub(super) struct PreparedTableDrawing {
    pub measured_bbox: BoundingBox,
    pub content_bbox: BoundingBox,
    pub rows: Vec<PreparedTableRow>,
    pub pending_gaps: Vec<f64>,
}

impl PreparedTable {
    pub fn minimum_first_page_height(&self) -> Option<f64> {
        matches!(
            self.constraint,
            ObjectSpanLayoutConstraint::OverPages
                | ObjectSpanLayoutConstraint::OverPagesOverlapPadding
        )
        .then_some(self.min_first_page_height)
    }

    pub fn relayout(
        &mut self,
        table: &RichTextTable,
        candidate_top: f64,
        theme: RenderTheme,
        renderer: &TextRenderer<'_>,
    ) -> Result<(), ObjectDiagnosticKind> {
        self.bands = table_bands(self.constraint, candidate_top, renderer)?;
        pagination::warm(self, table, theme, renderer)?;
        self.update_geometry(table, renderer)
    }
}

pub(super) struct PreparedTableRow {
    pub row_index: usize,
    pub cells: Vec<PreparedTableCell>,
}

pub(super) struct PreparedTableCell {
    pub column_index: usize,
    pub frame: BoundingBox,
    pub layout: TextLayout,
    bands: Option<BandList>,
    metrics: CellMetrics,
}

#[derive(Default)]
struct CellMetrics {
    measured_height: f64,
    first_line_height: Option<f64>,
    last_line_bottom: f64,
}

pub(super) fn prepare_table(
    table: &RichTextTable,
    constraint: ObjectSpanLayoutConstraint,
    candidate_top: f64,
    theme: RenderTheme,
    renderer: &TextRenderer<'_>,
) -> Option<Result<PreparedTable, ObjectDiagnosticKind>> {
    if !matches!(
        constraint,
        ObjectSpanLayoutConstraint::OverPages | ObjectSpanLayoutConstraint::OverPagesOverlapPadding
    ) {
        return None;
    }
    let topology = preparation_grid(table)?;
    if !valid_grid_geometry(table, candidate_top) {
        return Some(Err(ObjectDiagnosticKind::InvalidBounds));
    }
    if table
        .column_widths
        .iter()
        .any(|width| f64::from(*width) > f64::from(i32::MAX))
    {
        return None;
    }
    Some(prepare_cold_grid(
        table,
        topology,
        constraint,
        candidate_top,
        theme,
        renderer,
    ))
}

pub(super) fn prepare_table_drawing(
    table: &RichTextTable,
    constraint: ObjectSpanLayoutConstraint,
    drawing_origin: [f64; 2],
    theme: RenderTheme,
    renderer: &TextRenderer<'_>,
) -> Option<Result<PreparedTableDrawing, ObjectDiagnosticKind>> {
    if !matches!(
        constraint,
        ObjectSpanLayoutConstraint::Normal
            | ObjectSpanLayoutConstraint::OverPages
            | ObjectSpanLayoutConstraint::OverPagesOverlapPadding
    ) || table
        .column_widths
        .iter()
        .any(|width| f64::from(*width) > f64::from(i32::MAX))
    {
        return None;
    }
    let topology = preparation_grid(table)?;
    if !valid_grid_geometry(table, drawing_origin[1])
        || finite_native_geometry(drawing_origin[0]).is_none()
    {
        return Some(Err(ObjectDiagnosticKind::InvalidBounds));
    }
    Some((|| {
        let mut prepared = prepare_cold_grid(
            table,
            topology,
            constraint,
            drawing_origin[1],
            theme,
            renderer,
        )?;
        prepared.relayout(table, drawing_origin[1], theme, renderer)?;
        let measured_bbox = offset_rounded_rect(prepared.measured_bbox, drawing_origin)?;
        let origin = [measured_bbox.x_min, measured_bbox.y_min];
        let content_bbox = offset_rounded_rect(prepared.content_bbox, origin)?;
        for row in &mut prepared.rows {
            for cell in &mut row.cells {
                let frame = offset_rounded_rect(cell.frame, origin)?;
                cell.layout.translate(-cell.frame.x_min, -cell.frame.y_min);
                translate_cell_drawing(&mut cell.layout, [frame.x_min, frame.y_min])?;
                if !valid_cell_layout(&cell.layout) {
                    return Err(ObjectDiagnosticKind::InvalidBounds);
                }
                cell.frame = frame;
            }
        }
        Ok(PreparedTableDrawing {
            measured_bbox,
            content_bbox,
            rows: prepared.rows,
            pending_gaps: prepared.pending_gaps,
        })
    })())
}

pub(super) fn prepare_table_clone_drawing(
    table: &RichTextTable,
    constraint: ObjectSpanLayoutConstraint,
    target: BoundingBox,
    theme: RenderTheme,
    renderer: &TextRenderer<'_>,
) -> Option<Result<PreparedTableDrawing, ObjectDiagnosticKind>> {
    let bbox =
        match super::embedded::cloned_raw_bounds(table.bbox, table_drawn_bounds(table), target) {
            Ok(bbox) => bbox,
            Err(kind) => return Some(Err(kind)),
        };
    let drawn = drawn_bounds_for_rect(table, bbox);
    prepare_table_drawing(
        table,
        constraint,
        [drawn.x_min, drawn.y_min],
        theme,
        renderer,
    )
}

fn translate_cell_drawing(
    layout: &mut TextLayout,
    origin: [f64; 2],
) -> Result<(), ObjectDiagnosticKind> {
    for line in &mut layout.lines {
        if !line.line.placements.is_empty() {
            line.x +=
                super::text::line_alignment_offset(line.line.advance, line.width, line.alignment);
            line.alignment = None;
        }
        line.x = native_add(line.x, origin[0])?;
        line.baseline = native_add(line.baseline, origin[1])?;
        line.top = native_add(line.top, origin[1])?;
        line.background_top = native_add(line.background_top, origin[1])?;
        line.bottom = native_add(line.bottom, origin[1])?;
        line.post_cursor = native_add(line.post_cursor, origin[1])?;
        if let Some(marker) = &mut line.marker {
            marker.x = native_add(marker.x, origin[0])?;
            marker.center_y = native_add(marker.center_y, origin[1])?;
        }
    }
    Ok(())
}

fn offset_rounded_rect(
    rect: BoundingBox,
    origin: [f64; 2],
) -> Result<BoundingBox, ObjectDiagnosticKind> {
    let result = BoundingBox {
        x_min: native_add(rect.x_min, origin[0])?.floor(),
        y_min: native_add(rect.y_min, origin[1])?.floor(),
        x_max: native_add(rect.x_max, origin[0])?.ceil(),
        y_max: native_add(rect.y_max, origin[1])?.ceil(),
    };
    if result.x_max <= result.x_min || result.y_max <= result.y_min {
        return Err(ObjectDiagnosticKind::InvalidBounds);
    }
    Ok(result)
}

fn preparation_grid(table: &RichTextTable) -> Option<TableGrid> {
    let topology = TableGrid::new(table).ok()?;
    supports_prepared_content(table).then_some(topology)
}

pub(super) fn supports_prepared_content(table: &RichTextTable) -> bool {
    table
        .rotation_degrees
        .is_none_or(|rotation| rotation == 0.0)
        && table.rows.iter().flat_map(|row| &row.cells).all(|cell| {
            cell.content.object_spans.is_empty()
                && cell
                    .content
                    .rotation_degrees
                    .is_none_or(|rotation| rotation == 0.0)
        })
}

fn valid_grid_geometry(table: &RichTextTable, candidate_top: f64) -> bool {
    let bbox = table.bbox;
    finite_native_geometry(candidate_top).is_some()
        && [
            bbox.x_min,
            bbox.y_min,
            bbox.x_max,
            bbox.y_max,
            bbox.x_max - bbox.x_min,
            bbox.y_max - bbox.y_min,
        ]
        .into_iter()
        .all(|value| finite_native_geometry(value).is_some())
        && bbox.x_max > bbox.x_min
        && bbox.y_max > bbox.y_min
        && table
            .column_widths
            .iter()
            .all(|width| width.is_finite() && *width > 0.0)
        && table.rows.iter().all(|row| {
            row.height.is_finite()
                && row.height >= 0.0
                && row
                    .min_height
                    .is_none_or(|height| height.is_finite() && height >= 0.0)
        })
        && table
            .style
            .border
            .iter()
            .chain(table.style.default_cell_border.iter())
            .chain(
                table
                    .rows
                    .iter()
                    .flat_map(|row| &row.cells)
                    .filter_map(|cell| cell.border.as_ref()),
            )
            .all(|border| {
                border_edges(border)
                    .into_iter()
                    .all(|edge| edge.width.is_finite() && edge.width >= 0.0)
            })
}

fn border_edges(border: &TableBorder) -> [&crate::TableEdgeStyle; 4] {
    [&border.left, &border.top, &border.right, &border.bottom]
}

fn drawable_half_border(table: &RichTextTable) -> f64 {
    table.style.border.as_ref().map_or(0.5, |border| {
        border_edges(border)
            .into_iter()
            .filter(|edge| edge.color != 0)
            .map(|edge| f64::from(edge.width))
            .fold(0.0, f64::max)
            / 2.0
    })
}

fn native_measured_height(height: f64) -> Result<f64, ObjectDiagnosticKind> {
    finite_native_geometry(height)
        .map(|height| f64::from(height as f32))
        .ok_or(ObjectDiagnosticKind::InvalidBounds)
}

fn valid_cell_layout(layout: &TextLayout) -> bool {
    layout.lines.iter().all(|line| {
        line.width >= 0.0
            && line.line.font_size >= 0.0
            && [
                line.x,
                line.width,
                line.baseline,
                line.top,
                line.background_top,
                line.bottom,
                line.post_cursor,
                line.line.font_size,
                line.line.advance,
            ]
            .into_iter()
            .all(|value| finite_native_geometry(value).is_some())
    })
}

fn prepare_cold_grid(
    table: &RichTextTable,
    topology: TableGrid,
    constraint: ObjectSpanLayoutConstraint,
    candidate_top: f64,
    theme: RenderTheme,
    renderer: &TextRenderer<'_>,
) -> Result<PreparedTable, ObjectDiagnosticKind> {
    let half_border = drawable_half_border(table);
    let rows = initialize_rows(table, half_border)?;
    let mut prepared = PreparedTable {
        content_bbox: BoundingBox::default(),
        measured_bbox: BoundingBox {
            x_min: 0.0,
            y_min: 0.0,
            x_max: 0.0,
            y_max: 0.0,
        },
        pending_gaps: vec![0.0; rows.len()],
        rows,
        min_first_page_height: 0.0,
        constraint,
        half_border,
        topology,
        bands: table_bands(constraint, candidate_top, renderer)?,
    };
    pagination::cold(&mut prepared, table, theme, renderer)?;
    prepared.update_geometry(table, renderer)?;
    Ok(prepared)
}

fn initialize_rows(
    table: &RichTextTable,
    half_border: f64,
) -> Result<Vec<PreparedTableRow>, ObjectDiagnosticKind> {
    let mut top = half_border;
    let mut rows = Vec::with_capacity(table.rows.len());
    for (row_index, row) in table.rows.iter().enumerate() {
        let bottom = native_add(top, f64::from(row.height))?;
        let mut left = half_border;
        let mut cells = Vec::with_capacity(row.cells.len());
        for column_index in 0..row.cells.len() {
            let right = native_add(left, f64::from(table.column_widths[column_index]))?;
            let frame = BoundingBox {
                x_min: left,
                y_min: top,
                x_max: right,
                y_max: bottom,
            };
            if [left, right, top, frame.y_max]
                .into_iter()
                .any(|value| finite_native_geometry(value).is_none())
                || right <= left
            {
                return Err(ObjectDiagnosticKind::InvalidBounds);
            }
            cells.push(PreparedTableCell {
                column_index,
                frame,
                layout: TextLayout::default(),
                bands: None,
                metrics: CellMetrics::default(),
            });
            left = right;
        }
        top = bottom;
        rows.push(PreparedTableRow { row_index, cells });
    }
    Ok(rows)
}

fn native_add(left: f64, right: f64) -> Result<f64, ObjectDiagnosticKind> {
    native_measured_height(f64::from(
        native_measured_height(left)? as f32 + native_measured_height(right)? as f32,
    ))
}

fn native_sub(left: f64, right: f64) -> Result<f64, ObjectDiagnosticKind> {
    native_measured_height(f64::from(
        native_measured_height(left)? as f32 - native_measured_height(right)? as f32,
    ))
}

fn table_bands(
    constraint: ObjectSpanLayoutConstraint,
    candidate_top: f64,
    renderer: &TextRenderer<'_>,
) -> Result<BandList, ObjectDiagnosticKind> {
    let rectangles = renderer.table_split_rects(constraint, candidate_top);
    BandList::new(rectangles)
}

fn cell_text_bounds(frame: BoundingBox) -> Result<BoundingBox, ObjectDiagnosticKind> {
    let width = native_sub(frame.x_max, frame.x_min)? as f32 as i32;
    let height = native_sub(frame.y_max, frame.y_min)? as f32 as i32;
    Ok(BoundingBox {
        x_min: 0.0,
        y_min: 0.0,
        x_max: f64::from(width),
        y_max: f64::from(height),
    })
}

impl PreparedTable {
    fn layout_cell(
        &mut self,
        row_index: usize,
        column_index: usize,
        table: &RichTextTable,
        theme: RenderTheme,
        renderer: &TextRenderer<'_>,
    ) -> Result<(), ObjectDiagnosticKind> {
        let cell = &mut self.rows[row_index].cells[column_index];
        let bands = cell
            .bands
            .as_ref()
            .ok_or(ObjectDiagnosticKind::UnsupportedContent)?;
        let source = &table.rows[row_index].cells[column_index];
        let bounds = cell_text_bounds(cell.frame)?;
        let width = bounds.x_max as i32;
        let full_width = |rect: &&BoundingBox| rect.x_min <= 0.0 && rect.x_max >= f64::from(width);
        let exclusions = bands
            .rectangles
            .iter()
            .filter(full_width)
            .map(|rect| VerticalExclusion::obstacle(rect.y_min, rect.y_max))
            .collect::<Vec<_>>();
        let styled = StyledText::new(&source.content, TextContext::Flow, renderer.settings);
        let theme = CellFill::resolve(&table.style, table.rows[row_index].index, source, theme)
            .theme(theme);
        let mut layout = super::text::layout_text(
            &styled,
            TextFrame {
                bbox: bounds,
                gravity: Some(0),
                exclusions: &exclusions,
            },
            theme,
            renderer,
        );
        if !valid_cell_layout(&layout) {
            return Err(ObjectDiagnosticKind::InvalidBounds);
        }
        if bands
            .rectangles
            .iter()
            .filter(|rect| !full_width(rect))
            .any(|rect| {
                layout
                    .lines
                    .iter()
                    .any(|line| line.bottom > rect.y_min && line.top < rect.y_max)
            })
        {
            return Err(ObjectDiagnosticKind::UnsupportedContent);
        }
        let first_line_height = Some(
            layout
                .lines
                .first()
                .map(|line| native_sub(line.bottom, line.top))
                .transpose()?
                .unwrap_or(-1.0),
        );
        let last_line_bottom = if source.content.text.is_empty() {
            0.0
        } else {
            layout
                .lines
                .last()
                .map(|line| native_measured_height(line.bottom))
                .transpose()?
                .unwrap_or(0.0)
        };
        let measured_height = if source.content.text.is_empty() {
            empty_cell_height(&styled, source, width, bands, theme, renderer)?
        } else {
            native_add(
                last_line_bottom,
                renderer
                    .settings
                    .pixels(source.content.margins.unwrap_or([0.0; 4])[3]),
            )?
        };
        layout.translate(cell.frame.x_min, cell.frame.y_min);
        cell.layout = layout;
        cell.metrics = CellMetrics {
            measured_height,
            first_line_height,
            last_line_bottom,
        };
        Ok(())
    }

    fn row_height(&self, row_index: usize) -> Result<f64, ObjectDiagnosticKind> {
        let frame = self.rows[row_index].cells[0].frame;
        native_sub(frame.y_max, frame.y_min)
    }

    fn frame_cell(
        &self,
        position: CellPosition,
    ) -> Result<(CellPosition, &PreparedTableCell), ObjectDiagnosticKind> {
        let owner = self
            .topology
            .frame_owner(position)
            .ok_or(ObjectDiagnosticKind::UnsupportedContent)?;
        let cell = self
            .rows
            .get(owner.row)
            .and_then(|row| row.cells.get(owner.column))
            .ok_or(ObjectDiagnosticKind::UnsupportedContent)?;
        Ok((owner, cell))
    }

    fn content_bounds(&self) -> Result<BoundingBox, ObjectDiagnosticKind> {
        let (_, first) = self.frame_cell(CellPosition { row: 0, column: 0 })?;
        let row = self
            .rows
            .len()
            .checked_sub(1)
            .ok_or(ObjectDiagnosticKind::UnsupportedContent)?;
        let column = self.rows[row]
            .cells
            .len()
            .checked_sub(1)
            .ok_or(ObjectDiagnosticKind::UnsupportedContent)?;
        let (_, last) = self.frame_cell(CellPosition { row, column })?;
        Ok(BoundingBox {
            x_min: first.frame.x_min.min(last.frame.x_min),
            y_min: first.frame.y_min.min(last.frame.y_min),
            x_max: first.frame.x_max.max(last.frame.x_max),
            y_max: first.frame.y_max.max(last.frame.y_max),
        })
    }

    fn first_line_minimum(
        &self,
        row_index: usize,
        table: &RichTextTable,
        renderer: &TextRenderer<'_>,
    ) -> Result<f64, ObjectDiagnosticKind> {
        let mut height = 0.0_f64;
        for column in 0..self.rows[row_index].cells.len() {
            let (owner, cell) = self.frame_cell(CellPosition {
                row: row_index,
                column,
            })?;
            let source = &table.rows[owner.row].cells[owner.column].content;
            let cell_height = match cell.metrics.first_line_height {
                Some(first_line_height) if !source.text.is_empty() => native_add(
                    first_line_height,
                    renderer
                        .settings
                        .pixels(source.margins.unwrap_or([0.0; 4])[1]),
                )?,
                _ => cell.metrics.measured_height,
            };
            height = height.max(cell_height);
        }
        if height == 0.0 {
            self.row_height(0)
        } else {
            Ok(height)
        }
    }

    fn update_geometry(
        &mut self,
        table: &RichTextTable,
        renderer: &TextRenderer<'_>,
    ) -> Result<(), ObjectDiagnosticKind> {
        for row in &self.rows {
            for cell in &row.cells {
                let frame = cell.frame;
                if [frame.x_min, frame.y_min, frame.x_max, frame.y_max]
                    .into_iter()
                    .any(|value| finite_native_geometry(value).is_none())
                    || frame.x_max <= frame.x_min
                    || frame.y_max < frame.y_min
                    || !valid_cell_layout(&cell.layout)
                {
                    return Err(ObjectDiagnosticKind::InvalidBounds);
                }
            }
        }
        let content = self.content_bounds()?;
        self.content_bbox = content;
        let [left, upper, right, bottom] = table_border_widths(table);
        let expanded_left = native_sub(content.x_min, left / 2.0)?;
        let expanded_top = native_sub(content.y_min, upper / 2.0)?;
        self.measured_bbox = BoundingBox {
            x_min: 0.0,
            y_min: 0.0,
            x_max: native_sub(native_add(content.x_max, right / 2.0)?, expanded_left)?,
            y_max: native_sub(native_add(content.y_max, bottom / 2.0)?, expanded_top)?,
        };
        if self.measured_bbox.x_max <= 0.0 || self.measured_bbox.y_max <= 0.0 {
            return Err(ObjectDiagnosticKind::InvalidBounds);
        }
        self.min_first_page_height = self.first_page_minimum(0, table, renderer)?;
        Ok(())
    }

    fn first_page_minimum(
        &self,
        row_index: usize,
        table: &RichTextTable,
        renderer: &TextRenderer<'_>,
    ) -> Result<f64, ObjectDiagnosticKind> {
        native_add(
            self.first_line_minimum(row_index, table, renderer)?,
            self.content_bounds()?.y_min,
        )
    }
}

fn empty_cell_height(
    styled: &StyledText<'_>,
    source: &RichTextTableCell,
    width: i32,
    bands: &BandList,
    theme: RenderTheme,
    renderer: &TextRenderer<'_>,
) -> Result<f64, ObjectDiagnosticKind> {
    let margins = source
        .content
        .margins
        .unwrap_or([0.0; 4])
        .map(|margin| renderer.settings.pixels(margin) as f32);
    let paragraph = super::text::paragraph_layout(&source.content, 0, renderer.settings);
    if paragraph.spacing_before_invalid {
        return Err(ObjectDiagnosticKind::InvalidBounds);
    }
    let [left_indent, right_indent] = paragraph.indent_insets(renderer.settings);
    let mut style = styled.style_at(0, theme, paragraph.predefined_style);
    style.font_size = styled.font_size_at_caret(0);
    let marker_width = paragraph
        .bullet
        .and_then(|bullet| {
            super::marker::PreparedMarker::prepare(
                bullet,
                paragraph.indent_level,
                &style,
                theme,
                renderer,
            )
        })
        .map_or(0.0, |marker| marker.reserved_width()) as f32;
    let cursor_height = styled.caret_line_height(0) as f32;
    let mut sorted_bands = bands.rectangles.iter().collect::<Vec<_>>();
    sorted_bands.sort_by(|left, right| left.y_min.total_cmp(&right.y_min));
    let maximum_height = sorted_bands.get(1).map_or(f32::MAX, |second| {
        second.y_min as f32 - sorted_bands[0].y_max as f32
    });
    let line_height = if cursor_height > maximum_height {
        styled.font_size_at_caret(0) as f32
    } else {
        cursor_height
    };
    let original_left = margins[0] + left_indent as f32;
    let original_right = (width as f32 - margins[2]) - right_indent as f32;
    let mut left = original_left;
    let mut right = original_right;
    let mut top = margins[1] + paragraph.spacing_before as f32;
    let mut bottom = top + line_height;
    if [left, right, top, bottom, cursor_height, marker_width]
        .into_iter()
        .any(|value| !value.is_finite())
    {
        return Err(ObjectDiagnosticKind::InvalidBounds);
    }
    for _ in 0..sorted_bands.len() {
        let Some(band) = sorted_bands.iter().find(|band| {
            band.x_max > band.x_min
                && band.y_max > band.y_min
                && right - band.x_min as f32 > 0.0001_f32
                && band.x_max as f32 - left > 0.0001_f32
                && bottom - band.y_min as f32 > 0.0001_f32
                && band.y_max as f32 - top > 0.0001_f32
        }) else {
            break;
        };
        if band.x_min as f32 > left || (band.x_max as f32) < right {
            let minimum_width = marker_width + 5.0;
            if band.x_min as f32 - left >= minimum_width {
                right = band.x_min as f32;
                continue;
            }
            if right - band.x_max as f32 >= minimum_width {
                left = band.x_max as f32;
                continue;
            }
        }
        left = original_left;
        right = original_right;
        top = band.y_max as f32;
        bottom = top + line_height;
        if !bottom.is_finite() {
            return Err(ObjectDiagnosticKind::InvalidBounds);
        }
    }
    let centering = (bottom - top - cursor_height) * 0.5;
    if !centering.is_finite()
        || centering >= 2_147_483_648.0_f32
        || centering < -2_147_483_648.0_f32
    {
        return Err(ObjectDiagnosticKind::InvalidBounds);
    }
    let cursor_bottom = (top + cursor_height) + (centering as i32) as f32;
    native_measured_height(f64::from(cursor_bottom + margins[3]))
}

pub(super) fn table_drawn_bounds(table: &RichTextTable) -> BoundingBox {
    drawn_bounds_for_rect(table, table.bbox)
}

fn drawn_bounds_for_rect(table: &RichTextTable, rect: BoundingBox) -> BoundingBox {
    let [left, top, right, bottom] = table_border_widths(table);
    BoundingBox {
        x_min: rect.x_min - left / 2.0,
        y_min: rect.y_min - top / 2.0,
        x_max: rect.x_max + right / 2.0,
        y_max: rect.y_max + bottom / 2.0,
    }
}

fn table_border_widths(table: &RichTextTable) -> [f64; 4] {
    let mut left = Edge::Left.width(table.style.border.as_ref());
    let mut top = Edge::Top.width(table.style.border.as_ref());
    let mut right = Edge::Right.width(table.style.border.as_ref());
    let mut bottom = Edge::Bottom.width(table.style.border.as_ref());
    if let Some(last_column) = table.column_widths.len().checked_sub(1) {
        for row in &table.rows {
            for (edge, column, width) in [
                (Edge::Left, 0, &mut left),
                (Edge::Right, last_column, &mut right),
            ] {
                if let Some(cell) = row.cells.get(column) {
                    include_cell_edge(table, cell, edge, width);
                }
            }
        }
        for (edge, row, width) in [
            (Edge::Top, table.rows.first(), &mut top),
            (Edge::Bottom, table.rows.last(), &mut bottom),
        ] {
            if let Some(row) = row {
                for cell in row.cells.iter().take(table.column_widths.len()) {
                    include_cell_edge(table, cell, edge, width);
                }
            }
        }
    }
    [left, top, right, bottom]
}

fn include_cell_edge(table: &RichTextTable, cell: &RichTextTableCell, edge: Edge, width: &mut f64) {
    let cell_width = edge.width(
        cell.border
            .as_ref()
            .or(table.style.default_cell_border.as_ref()),
    );
    if cell_width > *width {
        *width = cell_width;
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::{RichTextBox, RichTextTableRow, TableEdgeStyle, TableRecordMetadata, TableStyle};

    pub(in crate::render) fn grid(heights: &[f32], widths: &[f32]) -> RichTextTable {
        let mut table = table();
        table.column_widths = widths.to_vec();
        table.rows = heights
            .iter()
            .enumerate()
            .map(|(row_index, &height)| {
                let mut row = row(widths
                    .iter()
                    .enumerate()
                    .map(|(column_index, _)| {
                        let mut cell = cell(None);
                        cell.column_index = column_index as u32;
                        cell
                    })
                    .collect());
                row.index = row_index as u32;
                row.height = height;
                row
            })
            .collect();
        table
    }

    pub(super) fn prepared(table: &RichTextTable) -> PreparedTable {
        let fonts = crate::fonts::FontBook::default();
        let renderer = TextRenderer::new(
            super::super::text::TextSettings {
                scale: 3.0,
                ..Default::default()
            },
            &fonts,
        );
        prepare_table(
            table,
            ObjectSpanLayoutConstraint::OverPages,
            999.0,
            RenderTheme::for_canvas(false),
            &renderer,
        )
        .unwrap()
        .unwrap()
    }

    fn close(actual: f64, expected: f64) {
        assert!((actual - expected).abs() < 1e-5, "{actual} != {expected}");
    }

    #[test]
    fn cloned_outer_frame_changes_drawn_origin_without_fitting_columns() {
        let mut table = grid(&[100.0], &[200.0]);
        table.bbox = BoundingBox {
            x_min: 100.0,
            y_min: 200.0,
            x_max: 300.0,
            y_max: 300.0,
        };
        table.style.border = Some(border([2.0; 4], 0xff000000));
        let fonts = crate::fonts::FontBook::default();
        let renderer = TextRenderer::new(super::super::text::TextSettings::default(), &fonts);
        let drawing = prepare_table_clone_drawing(
            &table,
            ObjectSpanLayoutConstraint::Normal,
            BoundingBox {
                x_min: 10.0,
                y_min: 20.0,
                x_max: 111.0,
                y_max: 122.0,
            },
            RenderTheme::for_canvas(false),
            &renderer,
        )
        .unwrap()
        .unwrap();
        assert_eq!(drawing.measured_bbox.x_min, 9.0);
        assert_eq!(drawing.measured_bbox.x_max, 212.0);
        let cell = drawing.rows[0].cells[0].frame;
        assert_eq!(cell.x_min, 10.0);
        assert_eq!(cell.x_max, 210.0);
        assert_eq!(table.column_widths, [200.0]);
    }

    #[test]
    fn partial_width_bands_require_an_actual_text_line_intersection() {
        let mut table = grid(&[100.0], &[60.0]);
        let content = &mut table.rows[0].cells[0].content;
        content.text = "T".into();
        content.font_size = Some(10.0);
        content.margins = None;
        let fonts = crate::fonts::FontBook::default();
        let renderer = TextRenderer::new(super::super::text::TextSettings::default(), &fonts);
        let theme = RenderTheme::for_canvas(false);
        let mut prepared = prepare_table(
            &table,
            ObjectSpanLayoutConstraint::OverPages,
            0.0,
            theme,
            &renderer,
        )
        .unwrap()
        .unwrap();
        for (top, expected) in [
            (0.0, Err(ObjectDiagnosticKind::UnsupportedContent)),
            (100.0, Ok(())),
        ] {
            prepared.rows[0].cells[0].bands = Some(
                BandList::new(vec![BoundingBox {
                    x_min: 0.0,
                    y_min: top,
                    x_max: 50.0,
                    y_max: top + 20.0,
                }])
                .unwrap(),
            );
            assert_eq!(
                prepared.layout_cell(0, 0, &table, theme, &renderer),
                expected
            );
        }
        assert_eq!(prepared.rows[0].cells[0].layout.lines.len(), 1);
    }

    #[test]
    fn cold_grid_reconstructs_native_frames_and_retains_every_cell_layout() {
        let mut table = grid(&[108.0; 2], &[492.0; 2]);
        for row in &mut table.rows {
            for cell in &mut row.cells {
                cell.bbox = BoundingBox {
                    x_min: 9000.0,
                    y_min: -100.0,
                    x_max: 9001.0,
                    y_max: -99.0,
                };
                cell.content.text = "A".into();
                cell.content.font_size = Some(15.0);
                cell.content.margins = Some([0.0, 4.0, 0.0, 4.0]);
                cell.content.gravity = Some(2);
                cell.editable = true;
                cell.content.paragraphs = [
                    (crate::RichTextParagraphType::LineSpacing, 1.6_f32),
                    (crate::RichTextParagraphType::SpacingBefore, 4.0_f32),
                ]
                .into_iter()
                .map(|(kind, value)| crate::RichTextParagraph {
                    kind,
                    start_paragraph: 0,
                    end_paragraph: 1,
                    payload: if kind == crate::RichTextParagraphType::LineSpacing {
                        [1_u32.to_le_bytes(), value.to_le_bytes()].concat()
                    } else {
                        value.to_le_bytes().to_vec()
                    },
                })
                .collect();
            }
        }
        let plan = prepared(&table);
        assert_eq!(
            plan.measured_bbox,
            BoundingBox {
                x_min: 0.0,
                y_min: 0.0,
                x_max: 985.0,
                y_max: 217.0
            }
        );
        assert_eq!(
            plan.rows[0].cells[0].frame,
            BoundingBox {
                x_min: 0.5,
                y_min: 0.5,
                x_max: 492.5,
                y_max: 108.5,
            }
        );
        assert_eq!(plan.rows[0].cells[1].frame.x_min, 492.5);
        assert_eq!(plan.rows[1].cells[0].frame.y_min, 108.5);
        for (row_index, row) in plan.rows.iter().enumerate() {
            assert_eq!(row.row_index, row_index);
            for (column_index, cell) in row.cells.iter().enumerate() {
                assert_eq!(cell.column_index, column_index);
                assert_eq!(cell.layout.lines.len(), 1);
                assert_eq!(cell.layout.lines[0].line.source, 0..1);
                close(
                    cell.layout.lines[0].baseline,
                    80.75 + 108.0 * row_index as f64,
                );
            }
        }
        close(plan.min_first_page_height, 84.5);
        close(plan.minimum_first_page_height().unwrap(), 84.5);
    }

    #[test]
    fn captured_passive_table_metadata_uses_stored_grid_for_both_phases() {
        let mut table = grid(&[108.0; 2], &[492.0; 2]);
        table.style.content_bbox = Some(table.bbox);
        table.style.min_column_widths = Some(vec![98.4; 2]);
        table.style.max_column_widths = Some(vec![984.0; 2]);
        table.style.max_width = Some(984.0);
        table.style.auto_fit = Some(crate::TableAutoFit::None);
        for row in &mut table.rows {
            for cell in &mut row.cells {
                cell.content.text = "A".into();
                cell.content.font_size = Some(15.0);
                cell.content.margins = Some([8.0, 4.0, 8.0, 4.0]);
                cell.content.paragraphs = [
                    (crate::RichTextParagraphType::LineSpacing, 1.6_f32),
                    (crate::RichTextParagraphType::SpacingBefore, 4.0),
                    (crate::RichTextParagraphType::SpacingAfter, 4.0),
                ]
                .into_iter()
                .map(|(kind, value)| crate::RichTextParagraph {
                    kind,
                    start_paragraph: 0,
                    end_paragraph: 1,
                    payload: if kind == crate::RichTextParagraphType::LineSpacing {
                        [1_u32.to_le_bytes(), value.to_le_bytes()].concat()
                    } else {
                        value.to_le_bytes().to_vec()
                    },
                })
                .collect();
            }
        }
        let callback = prepared(&table);
        assert_eq!(callback.measured_bbox.x_max, 985.0);
        assert_eq!(callback.measured_bbox.y_max, 217.0);
        let fonts = crate::fonts::FontBook::default();
        let renderer = TextRenderer::new(
            super::super::text::TextSettings {
                scale: 3.0,
                ..Default::default()
            },
            &fonts,
        );
        let drawing = prepare_table_drawing(
            &table,
            ObjectSpanLayoutConstraint::OverPages,
            [48.0, 948.7509765625],
            RenderTheme::for_canvas(false),
            &renderer,
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            drawing.measured_bbox,
            BoundingBox {
                x_min: 48.0,
                y_min: 948.0,
                x_max: 1033.0,
                y_max: 1166.0
            }
        );
        assert_eq!(
            drawing.rows[0].cells[0].frame,
            BoundingBox {
                x_min: 48.0,
                y_min: 948.0,
                x_max: 541.0,
                y_max: 1057.0
            }
        );
        close(drawing.rows[0].cells[0].layout.lines[0].baseline, 1028.25);
        close(drawing.rows[0].cells[0].layout.lines[0].x, 72.0);
        assert_eq!(callback.rows[0].cells[0].frame.y_min, 0.5);
    }

    #[test]
    fn fresh_drawing_warms_without_changing_callback_height() {
        let mut table = grid(&[100.0], &[200.0]);
        table.rows[0].cells[0].content.text = "A".into();
        table.rows[0].cells[0].content.font_size = Some(10.0);
        let fonts = crate::fonts::FontBook::default();
        let renderer = TextRenderer::new(super::super::text::TextSettings::default(), &fonts);
        let theme = RenderTheme::for_canvas(false);
        let callback = prepare_table(
            &table,
            ObjectSpanLayoutConstraint::OverPages,
            0.0,
            theme,
            &renderer,
        )
        .unwrap()
        .unwrap();
        assert_eq!(callback.measured_bbox.y_max, 101.0);
        for constraint in [
            ObjectSpanLayoutConstraint::Normal,
            ObjectSpanLayoutConstraint::OverPages,
            ObjectSpanLayoutConstraint::OverPagesOverlapPadding,
        ] {
            let drawing = prepare_table_drawing(&table, constraint, [0.0, 0.0], theme, &renderer)
                .unwrap()
                .unwrap();
            assert_eq!(drawing.measured_bbox.y_max, 15.0);
            assert_eq!(drawing.rows[0].cells[0].frame.y_max, 14.0);
            assert_eq!(drawing.rows[0].cells[0].layout.lines[0].line.source, 0..1);
            close(drawing.rows[0].cells[0].layout.lines[0].baseline, 10.0);
        }
        assert_eq!(callback.measured_bbox.y_max, 101.0);
    }

    #[test]
    fn drawing_offsets_f32_rectangles_before_rounding_cells_and_glyph_origins() {
        let mut table = grid(&[100.0], &[200.0]);
        table.rows[0].cells[0].content.text = "A".into();
        table.rows[0].cells[0].content.font_size = Some(10.0);
        let fonts = crate::fonts::FontBook::default();
        let renderer = TextRenderer::new(super::super::text::TextSettings::default(), &fonts);
        let theme = RenderTheme::for_canvas(false);
        for (origin, bbox, frame, baseline) in [
            (
                [10.25, -7.75],
                BoundingBox {
                    x_min: 10.0,
                    y_min: -8.0,
                    x_max: 212.0,
                    y_max: 7.0,
                },
                BoundingBox {
                    x_min: 10.0,
                    y_min: -8.0,
                    x_max: 211.0,
                    y_max: 6.0,
                },
                2.0,
            ),
            (
                [-10.25, -20.75],
                BoundingBox {
                    x_min: -11.0,
                    y_min: -21.0,
                    x_max: 191.0,
                    y_max: -6.0,
                },
                BoundingBox {
                    x_min: -11.0,
                    y_min: -21.0,
                    x_max: 190.0,
                    y_max: -7.0,
                },
                -11.0,
            ),
            (
                [16_777_216.5, 0.0],
                BoundingBox {
                    x_min: 16_777_216.0,
                    y_min: 0.0,
                    x_max: 16_777_416.0,
                    y_max: 15.0,
                },
                BoundingBox {
                    x_min: 16_777_216.0,
                    y_min: 0.0,
                    x_max: 16_777_416.0,
                    y_max: 14.0,
                },
                10.0,
            ),
        ] {
            let drawing = prepare_table_drawing(
                &table,
                ObjectSpanLayoutConstraint::Normal,
                origin,
                theme,
                &renderer,
            )
            .unwrap()
            .unwrap();
            assert_eq!(drawing.measured_bbox, bbox);
            assert_eq!(drawing.rows[0].cells[0].frame, frame);
            close(drawing.rows[0].cells[0].layout.lines[0].baseline, baseline);
            assert_eq!(drawing.rows[0].cells[0].layout.lines[0].x, frame.x_min);
        }
        assert!(matches!(
            prepare_table_drawing(
                &table,
                ObjectSpanLayoutConstraint::Normal,
                [f64::from(f32::MAX), 0.0],
                theme,
                &renderer
            ),
            Some(Err(ObjectDiagnosticKind::InvalidBounds))
        ));
    }

    #[test]
    fn drawing_adds_local_text_positions_to_world_origin_with_native_f32_precision() {
        let mut table = grid(&[100.0], &[200.0]);
        let content = &mut table.rows[0].cells[0].content;
        content.text = "A".into();
        content.font_size = Some(10.0);
        content.margins = Some([3.0, 3.0, 0.0, 0.0]);
        let fonts = crate::fonts::FontBook::default();
        let renderer = TextRenderer::new(super::super::text::TextSettings::default(), &fonts);
        let theme = RenderTheme::for_canvas(false);
        let callback = prepare_table(
            &table,
            ObjectSpanLayoutConstraint::OverPages,
            0.0,
            theme,
            &renderer,
        )
        .unwrap()
        .unwrap();
        let drawing = prepare_table_drawing(
            &table,
            ObjectSpanLayoutConstraint::Normal,
            [16_777_216.0; 2],
            theme,
            &renderer,
        )
        .unwrap()
        .unwrap();
        let line = &drawing.rows[0].cells[0].layout.lines[0];
        assert_eq!(line.x, 16_777_220.0);
        assert_eq!(line.baseline, 16_777_228.0);
        assert_eq!(line.top, 16_777_220.0);
        assert_eq!(line.background_top, 16_777_220.0);
        assert_eq!(line.bottom, 16_777_232.0);
        assert_eq!(line.post_cursor, 16_777_232.0);
        assert_eq!(line.width, callback.rows[0].cells[0].layout.lines[0].width);
        assert_eq!(line.line.source, 0..1);
        assert_eq!(callback.rows[0].cells[0].layout.lines[0].x, 3.5);
    }

    #[test]
    fn drawing_resolves_measured_alignment_before_native_world_addition() {
        for (alignment, raw, advance, expected_x) in [
            (crate::ParagraphAlignment::Center, 2_u32, 18.0, 16_777_308.0),
            (crate::ParagraphAlignment::Right, 1_u32, 17.0, 16_777_400.0),
        ] {
            let mut table = grid(&[100.0], &[200.0]);
            let content = &mut table.rows[0].cells[0].content;
            content.text = "A".into();
            content.font_size = Some(10.0);
            content.paragraphs.push(crate::RichTextParagraph {
                kind: crate::RichTextParagraphType::Alignment,
                start_paragraph: 0,
                end_paragraph: 1,
                payload: raw.to_le_bytes().to_vec(),
            });
            let callback = prepared(&table);
            let mut local = prepared(&table);
            let layout = &mut local.rows[0].cells[0].layout;
            let line = &mut layout.lines[0];
            assert!(!line.line.placements.is_empty());
            line.x = 0.0;
            line.width = 200.0;
            line.line.advance = advance;
            translate_cell_drawing(layout, [16_777_216.0, 0.0]).unwrap();
            let line = &layout.lines[0];
            assert_eq!(line.x, expected_x);
            assert_eq!(line.alignment, None);
            assert_eq!(line.width, 200.0);
            assert_eq!(line.line.advance, advance);
            assert_eq!(
                callback.rows[0].cells[0].layout.lines[0].alignment,
                Some(alignment)
            );
        }
    }

    #[test]
    fn drawing_translates_background_top_independently_with_native_precision() {
        let mut table = grid(&[100.0], &[200.0]);
        table.rows[0].cells[0].content.text = "A".into();
        let mut local = prepared(&table);
        let layout = &mut local.rows[0].cells[0].layout;
        layout.lines[0].top = 0.5;
        layout.lines[0].background_top = 1.5;
        translate_cell_drawing(layout, [0.0, 16_777_216.0]).unwrap();
        assert_eq!(layout.lines[0].top, 16_777_216.0);
        assert_eq!(layout.lines[0].background_top, 16_777_218.0);
    }

    #[test]
    fn drawing_preserves_alignment_when_no_retained_glyph_positions_exist() {
        let mut table = grid(&[100.0], &[200.0]);
        table.rows[0].cells[0].content.text = "A".into();
        let mut local = prepared(&table);
        let layout = &mut local.rows[0].cells[0].layout;
        let line = &mut layout.lines[0];
        line.line.placements.clear();
        line.x = 3.0;
        line.width = 200.0;
        line.line.advance = 18.0;
        line.alignment = Some(crate::ParagraphAlignment::Center);
        translate_cell_drawing(layout, [16_777_216.0, 0.0]).unwrap();
        assert_eq!(layout.lines[0].x, 16_777_220.0);
        assert_eq!(
            layout.lines[0].alignment,
            Some(crate::ParagraphAlignment::Center)
        );
    }

    #[test]
    fn cold_rows_grow_to_measured_children_and_never_shrink_stored_rows() {
        let mut table = grid(&[100.0; 2], &[200.0; 2]);
        for (row, fonts) in table
            .rows
            .iter_mut()
            .zip([[140.0_f32, 120.0], [30.0, 40.0]])
        {
            for (cell, size) in row.cells.iter_mut().zip(fonts) {
                cell.content.font_size = Some(size / 3.0);
            }
        }
        let plan = prepared(&table);
        assert_eq!(plan.rows[0].cells[0].frame.y_max, 189.5);
        assert_eq!(plan.rows[1].cells[0].frame.y_min, 189.5);
        assert_eq!(plan.rows[1].cells[0].frame.y_max, 289.5);
        assert_eq!(plan.measured_bbox.y_max, 290.0);
        assert_eq!(plan.min_first_page_height, 189.5);
    }

    #[test]
    fn cold_growth_preserves_native_positive_deltas_below_one_thousandth() {
        for saved_height in [45.0_f32 - 0.0005, 45.0_f32 - 0.002] {
            let mut table = grid(&[saved_height, 100.0], &[200.0]);
            table.rows[0].cells[0].content.font_size = Some(15.0);
            table.rows[0].cells[0]
                .content
                .paragraphs
                .push(crate::RichTextParagraph {
                    kind: crate::RichTextParagraphType::LineSpacing,
                    start_paragraph: 0,
                    end_paragraph: 1,
                    payload: [1_u32.to_le_bytes(), 1.0_f32.to_le_bytes()].concat(),
                });
            let plan = prepared(&table);
            assert_eq!(plan.rows[0].cells[0].metrics.measured_height, 45.0);
            assert_eq!(plan.rows[0].cells[0].frame.y_max, 45.5);
            assert_eq!(plan.rows[1].cells[0].frame.y_min, 45.5);
        }
    }

    #[test]
    fn warm_empty_band_callback_shrinks_frames_and_keeps_selectable_source() {
        let mut table = grid(&[100.0], &[200.5]);
        let content = &mut table.rows[0].cells[0].content;
        content.text = "A".into();
        content.font_size = Some(10.0);
        content.margins = Some([0.0, 0.0, 0.0, 2.0]);
        table.style.auto_fit = Some(crate::TableAutoFit::Both);
        table.style.min_column_width = Some(500.0);
        table.style.min_row_height = Some(500.0);
        let fonts = crate::fonts::FontBook::default();
        let renderer = TextRenderer::new(super::super::text::TextSettings::default(), &fonts);
        let theme = RenderTheme::for_canvas(false);
        let mut plan = prepare_table(
            &table,
            ObjectSpanLayoutConstraint::OverPages,
            0.0,
            theme,
            &renderer,
        )
        .unwrap()
        .unwrap();
        assert_eq!(plan.row_height(0).unwrap(), 100.0);
        close(plan.rows[0].cells[0].layout.lines[0].baseline, 10.5);
        plan.relayout(&table, 0.0, theme, &renderer).unwrap();
        assert_eq!(plan.row_height(0).unwrap(), 15.5);
        assert_eq!(plan.measured_bbox.y_max, 16.5);
        assert_eq!(plan.rows[0].cells[0].frame.x_max, 201.0);
        assert_eq!(plan.rows[0].cells[0].layout.lines[0].line.source, 0..1);
        assert_eq!(plan.min_first_page_height, 14.0);
    }

    #[test]
    fn cell_metrics_distinguish_empty_cursor_height_from_last_output_line() {
        let mut table = grid(&[100.0], &[200.0]);
        table.rows[0].cells[0].content.font_size = Some(10.0);
        table.rows[0].cells[0].content.margins = Some([0.0, 2.0, 0.0, 3.0]);
        let fonts = crate::fonts::FontBook::default();
        let renderer = TextRenderer::new(super::super::text::TextSettings::default(), &fonts);
        let theme = RenderTheme::for_canvas(false);
        let mut plan = prepare_table(
            &table,
            ObjectSpanLayoutConstraint::OverPages,
            0.0,
            theme,
            &renderer,
        )
        .unwrap()
        .unwrap();
        assert_eq!(plan.rows[0].cells[0].metrics.measured_height, 18.5);
        assert_eq!(plan.rows[0].cells[0].layout.height(), 0.0);
        assert_eq!(plan.rows[0].cells[0].metrics.last_line_bottom, 0.0);
        plan.relayout(&table, 0.0, theme, &renderer).unwrap();
        assert_eq!(plan.row_height(0).unwrap(), 18.5);
        assert_eq!(plan.min_first_page_height, 19.0);
    }

    #[test]
    fn drawable_outer_border_offset_is_distinct_from_drawn_boundary_cell_expansion() {
        let mut table = grid(&[108.0], &[100.0]);
        table.style.border = Some(border([2.0, 4.0, 6.0, 8.0], 0xff000000));
        table.rows[0].cells[0].border = Some(border([50.0; 4], 0));
        let plan = prepared(&table);
        assert_eq!(
            plan.rows[0].cells[0].frame,
            BoundingBox {
                x_min: 4.0,
                y_min: 4.0,
                x_max: 104.0,
                y_max: 112.0,
            }
        );
        assert_eq!(
            plan.measured_bbox,
            BoundingBox {
                x_min: 0.0,
                y_min: 0.0,
                x_max: 150.0,
                y_max: 158.0
            }
        );
        close(plan.min_first_page_height, 72.85);
        table.style.border = Some(border([2.0, 4.0, 6.0, 8.0], 0));
        let transparent = prepared(&table);
        assert_eq!(transparent.rows[0].cells[0].frame.x_min, 0.0);
        assert_eq!(transparent.rows[0].cells[0].frame.y_min, 0.0);
        assert_eq!(transparent.measured_bbox, plan.measured_bbox);
    }

    #[test]
    fn child_split_bands_are_retained_even_when_beyond_the_first_row() {
        let document = crate::Document {
            pages: vec![crate::Page {
                uuid: "page".into(),
                width: 1080,
                height: 500,
                content_bbox: BoundingBox::default(),
                background_color: None,
                template: None,
                background: Default::default(),
                objects: Vec::new(),
            }],
            metadata: crate::DocumentMetadata {
                page_mode: Some(0),
                default_page_dimensions: Some((1080, 500)),
                ..Default::default()
            },
        };
        let settings = super::super::text::TextSettings::from_document(&document.metadata);
        let fonts = crate::fonts::FontBook::default();
        let pages = super::super::text::PageExclusions::for_document(&document, settings);
        let renderer = TextRenderer::new(settings, &fonts).with_page_exclusions(pages);
        let table = grid(&[100.0], &[200.0]);
        for constraint in [
            ObjectSpanLayoutConstraint::OverPages,
            ObjectSpanLayoutConstraint::OverPagesOverlapPadding,
        ] {
            let plan = prepare_table(
                &table,
                constraint,
                100.0,
                RenderTheme::for_canvas(false),
                &renderer,
            )
            .unwrap()
            .unwrap();
            assert!(
                !plan.rows[0].cells[0]
                    .bands
                    .as_ref()
                    .unwrap()
                    .rectangles
                    .is_empty()
            );
            assert!(
                prepare_table(
                    &table,
                    constraint,
                    1000.0,
                    RenderTheme::for_canvas(false),
                    &renderer
                )
                .unwrap()
                .is_ok()
            );
        }
    }

    #[test]
    fn saved_height_limits_do_not_cap_callback_or_drawing_layout() {
        let fonts = crate::fonts::FontBook::default();
        let renderer = TextRenderer::new(super::super::text::TextSettings::default(), &fonts);
        let theme = RenderTheme::for_canvas(false);
        let mut table = grid(&[10.0, 10.0], &[200.0]);
        for row in &mut table.rows {
            row.cells[0].content.text = "First\nSecond\nThird".into();
            row.cells[0].content.font_size = Some(15.0);
        }
        for constraint in [
            ObjectSpanLayoutConstraint::Normal,
            ObjectSpanLayoutConstraint::OverPages,
            ObjectSpanLayoutConstraint::OverPagesOverlapPadding,
        ] {
            let baseline =
                prepare_table_drawing(&table, constraint, [10.0, 20.0], theme, &renderer)
                    .unwrap()
                    .unwrap();
            for maximum in [0.0, 1.0, 10.0] {
                let mut limited = table.clone();
                for row in &mut limited.rows {
                    row.max_height = Some(maximum);
                }
                limited.style.max_height_enabled = true;
                limited.style.max_height = Some(maximum);
                let drawing =
                    prepare_table_drawing(&limited, constraint, [10.0, 20.0], theme, &renderer)
                        .unwrap()
                        .unwrap();
                assert_eq!(drawing.measured_bbox, baseline.measured_bbox);
                for (actual, expected) in drawing.rows.iter().zip(&baseline.rows) {
                    let actual = &actual.cells[0];
                    let expected = &expected.cells[0];
                    assert_eq!(actual.frame, expected.frame);
                    assert!(actual.frame.y_max - actual.frame.y_min > f64::from(maximum));
                    assert_eq!(actual.layout.lines.len(), 3);
                    for (actual, expected) in actual.layout.lines.iter().zip(&expected.layout.lines)
                    {
                        assert_eq!(actual.baseline, expected.baseline);
                        assert_eq!(actual.line.source, expected.line.source);
                    }
                }
                if constraint != ObjectSpanLayoutConstraint::Normal {
                    let mut callback = prepare_table(&limited, constraint, 20.0, theme, &renderer)
                        .unwrap()
                        .unwrap();
                    callback.relayout(&limited, 20.0, theme, &renderer).unwrap();
                    assert!(callback.measured_bbox.y_max > f64::from(maximum));
                    assert_eq!(callback.rows[0].cells[0].layout.lines.len(), 3);
                }
            }
        }
    }

    #[test]
    fn unsupported_layout_shapes_remain_distinct_from_invalid_grid_geometry() {
        let fonts = crate::fonts::FontBook::default();
        let renderer = TextRenderer::new(super::super::text::TextSettings::default(), &fonts);
        let prepare = |table: &RichTextTable, constraint| {
            prepare_table(
                table,
                constraint,
                0.0,
                RenderTheme::for_canvas(false),
                &renderer,
            )
        };
        let original = grid(&[100.0], &[200.0]);
        assert!(prepare(&original, ObjectSpanLayoutConstraint::Normal).is_none());
        let mut merged = original.clone();
        merged.rows[0].cells[0].column_span = 2;
        assert!(prepare(&merged, ObjectSpanLayoutConstraint::OverPages).is_none());
        let mut fractional = original.clone();
        fractional.column_widths[0] = 200.5;
        let fractional = prepare(&fractional, ObjectSpanLayoutConstraint::OverPages)
            .unwrap()
            .unwrap();
        assert_eq!(fractional.rows[0].cells[0].frame.x_max, 201.0);
        let mut invalid_width = original.clone();
        invalid_width.column_widths[0] = f32::NAN;
        let mut invalid_height = original;
        invalid_height.rows[0].height = -1.0;
        for invalid in [invalid_width, invalid_height] {
            assert!(matches!(
                prepare(&invalid, ObjectSpanLayoutConstraint::OverPages),
                Some(Err(ObjectDiagnosticKind::InvalidBounds))
            ));
        }
        let overflowing = grid(&[f32::MAX; 2], &[200.0]);
        assert!(matches!(
            prepare(&overflowing, ObjectSpanLayoutConstraint::OverPages),
            Some(Err(ObjectDiagnosticKind::InvalidBounds))
        ));
    }

    #[test]
    fn finite_final_height_does_not_hide_unrepresentable_intermediate_cell_lines() {
        let mut table = grid(&[100.0], &[200.0]);
        let content = &mut table.rows[0].cells[0].content;
        content.text = "A\nB\nC".into();
        content.font_size = Some(10.0);
        content.paragraphs = [
            (crate::RichTextParagraphType::SpacingBefore, 0, f32::MAX),
            (crate::RichTextParagraphType::SpacingAfter, 0, f32::MAX),
            (crate::RichTextParagraphType::SpacingAfter, 1, -f32::MAX),
            (crate::RichTextParagraphType::SpacingBefore, 2, -f32::MAX),
        ]
        .into_iter()
        .map(|(kind, ordinal, value)| crate::RichTextParagraph {
            kind,
            start_paragraph: ordinal,
            end_paragraph: ordinal + 1,
            payload: value.to_le_bytes().to_vec(),
        })
        .collect();
        let fonts = crate::fonts::FontBook::default();
        let renderer = TextRenderer::new(super::super::text::TextSettings::default(), &fonts);
        assert!(matches!(
            prepare_table(
                &table,
                ObjectSpanLayoutConstraint::OverPages,
                0.0,
                RenderTheme::for_canvas(false),
                &renderer
            ),
            Some(Err(ObjectDiagnosticKind::InvalidBounds))
        ));
    }

    #[test]
    fn zero_derived_height_is_rejected_instead_of_being_silently_culled() {
        let mut table = grid(&[0.0], &[200.0]);
        table.style.border = Some(border([0.0; 4], 0));
        table.style.default_cell_border = Some(border([0.0; 4], 0));
        let content = &mut table.rows[0].cells[0].content;
        content.font_size = Some(1.0);
        content.margins = Some([0.0, -0.5, 0.0, -0.5]);
        content.paragraphs.push(crate::RichTextParagraph {
            kind: crate::RichTextParagraphType::LineSpacing,
            start_paragraph: 0,
            end_paragraph: 1,
            payload: [1_u32.to_le_bytes(), 1.0_f32.to_le_bytes()].concat(),
        });
        let fonts = crate::fonts::FontBook::default();
        let renderer = TextRenderer::new(super::super::text::TextSettings::default(), &fonts);
        assert!(matches!(
            prepare_table(
                &table,
                ObjectSpanLayoutConstraint::OverPages,
                0.0,
                RenderTheme::for_canvas(false),
                &renderer
            ),
            Some(Err(ObjectDiagnosticKind::InvalidBounds))
        ));
    }

    #[test]
    fn zero_empty_child_measurement_uses_first_frame_height_for_the_minimum() {
        let mut table = grid(&[100.0], &[200.0]);
        let content = &mut table.rows[0].cells[0].content;
        content.font_size = Some(1.0);
        content.margins = Some([0.0, -0.5, 0.0, -0.5]);
        content.paragraphs.push(crate::RichTextParagraph {
            kind: crate::RichTextParagraphType::LineSpacing,
            start_paragraph: 0,
            end_paragraph: 1,
            payload: [1_u32.to_le_bytes(), 1.0_f32.to_le_bytes()].concat(),
        });
        let plan = prepared(&table);
        assert_eq!(plan.rows[0].cells[0].layout.height(), 0.0);
        assert_eq!(plan.min_first_page_height, 100.5);
    }

    fn empty_cursor_height(
        font_size: f32,
        margins: [f32; 4],
        paragraphs: Vec<crate::RichTextParagraph>,
        rectangles: &[[f64; 4]],
    ) -> Result<f64, ObjectDiagnosticKind> {
        let mut table = grid(&[100.0], &[200.0]);
        let source = &mut table.rows[0].cells[0];
        source.content.font_size = Some(font_size);
        source.content.margins = Some(margins);
        source.content.paragraphs = paragraphs;
        let fonts = crate::fonts::FontBook::default();
        let renderer = TextRenderer::new(super::super::text::TextSettings::default(), &fonts);
        let styled = StyledText::new(&source.content, TextContext::Flow, renderer.settings);
        let bands = BandList::new(
            rectangles
                .iter()
                .map(|rect| BoundingBox {
                    x_min: rect[0],
                    y_min: rect[1],
                    x_max: rect[2],
                    y_max: rect[3],
                })
                .collect(),
        )?;
        empty_cell_height(
            &styled,
            source,
            200,
            &bands,
            RenderTheme::for_canvas(false),
            &renderer,
        )
    }

    #[test]
    fn empty_cursor_measurement_includes_spacing_before_and_both_margins() {
        for (kind, value, expected) in [(1_u32, 1.6_f32, 26.0), (0, 7.0, 27.0)] {
            let paragraphs = vec![
                crate::RichTextParagraph {
                    kind: crate::RichTextParagraphType::LineSpacing,
                    start_paragraph: 0,
                    end_paragraph: 1,
                    payload: [kind.to_le_bytes(), value.to_le_bytes()].concat(),
                },
                crate::RichTextParagraph {
                    kind: crate::RichTextParagraphType::SpacingBefore,
                    start_paragraph: 0,
                    end_paragraph: 1,
                    payload: 5.0_f32.to_le_bytes().to_vec(),
                },
            ];
            assert_eq!(
                empty_cursor_height(10.0, [0.0, 2.0, 0.0, 3.0], paragraphs, &[]).unwrap(),
                expected
            );
        }
    }

    #[test]
    fn empty_cursor_bands_use_first_gap_clamping_and_integer_centering() {
        assert_eq!(
            empty_cursor_height(20.0, [0.0; 4], vec![], &[[0.0, 10.0, 200.0, 15.0]]).unwrap(),
            42.0
        );
        assert_eq!(
            empty_cursor_height(
                20.0,
                [0.0; 4],
                vec![],
                &[[0.0, 10.0, 200.0, 15.0], [0.0, 25.0, 200.0, 30.0]],
            )
            .unwrap(),
            54.0
        );
        assert_eq!(
            empty_cursor_height(
                20.0,
                [0.0; 4],
                vec![],
                &[[0.0, 10.0, 200.0, 15.0], [0.0, 55.0, 200.0, 60.0]],
            )
            .unwrap(),
            42.0
        );
    }

    #[test]
    fn empty_cursor_band_intersection_uses_native_tolerance() {
        for (band_top, expected) in [(13.5, 13.5), (13.49995, 13.5), (13.4998, 33.5)] {
            assert_eq!(
                empty_cursor_height(10.0, [0.0; 4], vec![], &[[0.0, band_top, 200.0, 20.0]],)
                    .unwrap(),
                expected
            );
        }
    }

    #[test]
    fn empty_cursor_can_use_horizontal_room_outside_a_padding_band() {
        assert_eq!(
            empty_cursor_height(
                10.0,
                [-10.0, 0.0, -10.0, 0.0],
                vec![],
                &[[0.0, 0.0, 200.0, 20.0]],
            )
            .unwrap(),
            13.5
        );
        let checkbox = crate::RichTextParagraph {
            kind: crate::RichTextParagraphType::Bullet,
            start_paragraph: 0,
            end_paragraph: 1,
            payload: [2_u32, 0, 0, 1]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect(),
        };
        assert_eq!(
            empty_cursor_height(
                10.0,
                [-10.0, 0.0, -10.0, 0.0],
                vec![checkbox],
                &[[0.0, 0.0, 200.0, 20.0]],
            )
            .unwrap(),
            33.5
        );
    }

    #[test]
    fn empty_cursor_rejects_unrepresentable_integer_centering() {
        assert!(matches!(
            empty_cursor_height(
                1e20,
                [0.0; 4],
                vec![],
                &[[0.0, 10.0, 200.0, 15.0], [0.0, 25.0, 200.0, 30.0]],
            ),
            Err(ObjectDiagnosticKind::InvalidBounds)
        ));
    }

    fn border(widths: [f32; 4], color: u32) -> TableBorder {
        let edge = |width| TableEdgeStyle {
            color,
            width,
            start_radius: 0.0,
            end_radius: 0.0,
        };
        TableBorder {
            left: edge(widths[0]),
            top: edge(widths[1]),
            right: edge(widths[2]),
            bottom: edge(widths[3]),
            metadata: TableRecordMetadata::default(),
        }
    }

    fn table() -> RichTextTable {
        RichTextTable {
            style: TableStyle {
                heading_column_enabled: false,
                heading_row_enabled: false,
                max_height_enabled: false,
                min_column_width: None,
                min_row_height: None,
                content_bbox: None,
                border: None,
                auto_fit: None,
                min_column_widths: None,
                max_column_widths: None,
                max_height: None,
                max_width: None,
                default_cell_border: None,
                heading_background_color: None,
                default_cell_background_color: None,
                metadata: TableRecordMetadata::default(),
            },
            bbox: BoundingBox {
                x_min: 48.5,
                y_min: 949.2509765625,
                x_max: 1032.5,
                y_max: 1165.2509765625,
            },
            rotation_degrees: None,
            column_widths: Vec::new(),
            rows: Vec::new(),
        }
    }

    fn cell(border: Option<TableBorder>) -> RichTextTableCell {
        RichTextTableCell {
            border,
            metadata: TableRecordMetadata::default(),
            column_index: 0,
            row_span: 1,
            column_span: 1,
            background_color: 0,
            has_own_background_color: false,
            bbox: BoundingBox::default(),
            editable: false,
            content: RichTextBox {
                text_area_type: None,
                bbox: BoundingBox::default(),
                rotation_degrees: None,
                text: String::new(),
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
            },
        }
    }

    fn row(cells: Vec<RichTextTableCell>) -> RichTextTableRow {
        RichTextTableRow {
            max_height: None,
            min_height: None,
            metadata: TableRecordMetadata::default(),
            index: 0,
            height: 100.0,
            cells,
        }
    }

    #[test]
    fn native_unit_defaults_expand_captured_raw_dimensions_by_one() {
        let table = table();
        let drawn = table_drawn_bounds(&table);
        assert_eq!(drawn.x_max - drawn.x_min, 985.0);
        assert_eq!(drawn.y_max - drawn.y_min, 217.0);
        assert_eq!(drawn.x_min, 48.0);
        assert_eq!(drawn.y_min, 948.7509765625);
    }

    #[test]
    fn each_outer_width_contributes_its_own_half_without_color_or_alpha_gating() {
        let mut table = table();
        table.style.border = Some(border([2.0, 4.0, 6.0, 8.0], 0));
        assert_eq!(
            table_drawn_bounds(&table),
            BoundingBox {
                x_min: 47.5,
                y_min: 947.2509765625,
                x_max: 1035.5,
                y_max: 1169.2509765625,
            }
        );
        table.style.border = Some(border([0.0; 4], 0xff000000));
        assert_eq!(table_drawn_bounds(&table), table.bbox);
    }

    #[test]
    fn only_boundary_cell_edges_expand_the_corresponding_side() {
        let mut table = table();
        table.style.border = Some(border([1.0; 4], 0xff000000));
        table.column_widths = vec![10.0; 3];
        table.rows = vec![
            row(vec![
                cell(None),
                cell(Some(border([90.0, 4.0, 90.0, 90.0], 0))),
                cell(None),
            ]),
            row(vec![
                cell(Some(border([2.0, 90.0, 90.0, 90.0], 0))),
                cell(Some(border([1000.0; 4], 0))),
                cell(Some(border([90.0, 90.0, 6.0, 90.0], 0))),
            ]),
            row(vec![
                cell(None),
                cell(Some(border([90.0, 90.0, 90.0, 8.0], 0))),
                cell(None),
            ]),
        ];
        assert_eq!(
            table_drawn_bounds(&table),
            BoundingBox {
                x_min: 47.5,
                y_min: 947.2509765625,
                x_max: 1035.5,
                y_max: 1169.2509765625,
            }
        );
    }

    #[test]
    fn explicit_cell_border_overrides_default_and_missing_cells_do_not_inherit() {
        let mut table = table();
        table.style.border = Some(border([0.0; 4], 0));
        table.style.default_cell_border = Some(border([10.0; 4], 0));
        table.column_widths = vec![10.0];
        table.rows = vec![row(vec![cell(Some(border([2.0; 4], 0)))])];
        let drawn = table_drawn_bounds(&table);
        assert_eq!(drawn.x_max - drawn.x_min, 986.0);
        assert_eq!(drawn.y_max - drawn.y_min, 218.0);
        table.rows[0].cells[0].border = None;
        assert_eq!(
            table_drawn_bounds(&table).x_max - table_drawn_bounds(&table).x_min,
            994.0
        );
        table.rows[0].cells.clear();
        assert_eq!(table_drawn_bounds(&table), table.bbox);
    }

    #[test]
    fn saved_and_prepared_paint_use_visible_cells_in_covered_span_chains() {
        let mut source = grid(&[100.0], &[90.0; 4]);
        source.rows[0].cells[0].column_span = 2;
        source.rows[0].cells[1].column_span = 3;
        for (slot, cell) in source.rows[0].cells.iter_mut().enumerate() {
            cell.content.text = ["A", "B", "C", "D"][slot].into();
            cell.content.font_size = Some(8.0);
            cell.has_own_background_color = true;
            cell.background_color = [0xffa11234, 0xffa25678, 0xffa39abc, 0xffa4def0][slot];
        }
        let fonts = crate::fonts::FontBook::default();
        let renderer = TextRenderer::new(
            super::super::text::TextSettings {
                scale: 1.0,
                ..Default::default()
            },
            &fonts,
        );
        let theme = RenderTheme::for_canvas(false);
        let drawing = prepare_table_drawing(
            &source,
            ObjectSpanLayoutConstraint::Normal,
            [0.0, 0.0],
            theme,
            &renderer,
        )
        .unwrap()
        .unwrap();
        source.bbox = drawing.measured_bbox;
        for (source, measured) in source.rows[0].cells.iter_mut().zip(&drawing.rows[0].cells) {
            source.bbox = measured.frame;
        }
        for prepared in [None, Some(super::super::TablePaint::from(&drawing))] {
            let mut scene = super::super::Scene::new(super::super::Svg::new());
            #[cfg(feature = "pdf")]
            scene.retain_text();
            super::super::render_table(
                &mut scene,
                &source,
                0,
                prepared,
                0.0,
                &[],
                theme,
                &renderer,
                None,
            );
            #[cfg(feature = "pdf")]
            {
                assert!(scene.take_native_text_error().is_none());
                let text = scene.take_native_text();
                assert_eq!(
                    text.iter()
                        .map(|(_, block)| block.source.as_ref())
                        .collect::<Vec<_>>(),
                    ["A", "C", "D"]
                );
            }
            let svg = scene.finish();
            for text in [">A<", ">C<", ">D<"] {
                assert!(svg.contains(text), "missing {text}: {svg}");
            }
            assert!(!svg.contains(">B<"), "covered text: {svg}");
            for color in ["#a11234", "#a39abc", "#a4def0"] {
                assert!(svg.contains(color), "missing {color}: {svg}");
            }
            assert!(!svg.contains("#a25678"), "covered background: {svg}");
        }
    }

    #[test]
    fn merged_preparation_keeps_raw_cold_metrics_and_uses_warm_frame_owners() {
        let mut source = grid(&[100.0; 3], &[200.0, 100.0]);
        source.rows[0].cells[0].row_span = 3;
        source.rows[0].cells[0].column_span = 2;
        for cell in source.rows.iter_mut().flat_map(|row| &mut row.cells) {
            cell.content.text = "A".into();
            cell.content.font_size = Some(10.0);
        }
        source.rows[0].cells[1].content.text = ["B"; 12].join("\n");
        let fonts = crate::fonts::FontBook::default();
        let renderer = TextRenderer::new(
            super::super::text::TextSettings {
                scale: 1.0,
                ..Default::default()
            },
            &fonts,
        );
        let theme = RenderTheme::for_canvas(false);
        for constraint in [
            ObjectSpanLayoutConstraint::OverPages,
            ObjectSpanLayoutConstraint::OverPagesOverlapPadding,
        ] {
            let mut plan = prepare_table(&source, constraint, 0.0, theme, &renderer)
                .unwrap()
                .unwrap();
            assert_eq!(plan.rows[0].cells[1].layout.lines.len(), 12);
            assert_eq!(plan.rows[0].cells[1].metrics.measured_height, 162.0);
            assert_eq!(plan.row_height(0).unwrap(), 162.0);
            assert_eq!(plan.measured_bbox.y_max, 163.0);
            assert_eq!(plan.content_bbox, plan.rows[0].cells[0].frame);
            plan.relayout(&source, 0.0, theme, &renderer).unwrap();
            for row in 0..3 {
                assert_eq!(plan.row_height(row).unwrap(), 13.5);
            }
            assert_eq!(plan.rows[0].cells[1].metrics.measured_height, 162.0);
            assert_eq!(plan.measured_bbox.y_max, 14.5);
            assert_eq!(plan.content_bbox, plan.rows[0].cells[0].frame);
        }
    }

    #[test]
    fn merged_outline_uses_owner_content_bounds_after_drawing_rounding() {
        let mut source = grid(&[100.0], &[90.0; 4]);
        source.rows[0].cells[0].column_span = 2;
        source.rows[0].cells[1].column_span = 3;
        let mut outer = border([1.0; 4], 0xff112233);
        for edge in [
            &mut outer.left,
            &mut outer.top,
            &mut outer.right,
            &mut outer.bottom,
        ] {
            edge.start_radius = 3.0;
            edge.end_radius = 3.0;
        }
        source.style.border = Some(outer);
        for cell in &mut source.rows[0].cells {
            cell.content.text = "A".into();
            cell.content.font_size = Some(8.0);
        }
        let fonts = crate::fonts::FontBook::default();
        let renderer = TextRenderer::new(
            super::super::text::TextSettings {
                scale: 1.0,
                ..Default::default()
            },
            &fonts,
        );
        let theme = RenderTheme::for_canvas(false);
        let drawing = prepare_table_drawing(
            &source,
            ObjectSpanLayoutConstraint::Normal,
            [0.0, 0.0],
            theme,
            &renderer,
        )
        .unwrap()
        .unwrap();
        assert_eq!(drawing.content_bbox, drawing.rows[0].cells[0].frame);
        assert_eq!(drawing.content_bbox.x_max, 91.0);
        assert_eq!(drawing.rows[0].cells[3].frame.x_max, 361.0);
        let mut scene = super::super::Scene::new(super::super::Svg::new());
        super::super::render_table(
            &mut scene,
            &source,
            0,
            Some((&drawing).into()),
            0.0,
            &[],
            theme,
            &renderer,
            None,
        );
        let svg = scene.finish();
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let outline = xml
            .descendants()
            .find(|node| node.has_tag_name("rect") && node.attribute("stroke") == Some("#112233"))
            .unwrap();
        assert_eq!(outline.attribute("x").unwrap().parse::<f64>().unwrap(), 0.0);
        assert_eq!(
            outline.attribute("width").unwrap().parse::<f64>().unwrap(),
            91.0
        );
    }

    #[test]
    fn native_grid_lookup_uses_stored_positions_without_expanding_span_metadata() {
        let mut table = table();
        table.style.border = Some(border([0.0; 4], 0));
        table.column_widths = vec![10.0; 2];
        let mut first = cell(Some(border([2.0; 4], 0)));
        first.column_index = 900;
        first.column_span = 2;
        first.row_span = 5;
        table.rows = vec![row(vec![first])];
        let drawn = table_drawn_bounds(&table);
        assert_eq!(drawn.x_min, table.bbox.x_min - 1.0);
        assert_eq!(drawn.x_max, table.bbox.x_max);
        assert_eq!(drawn.y_min, table.bbox.y_min - 1.0);
        assert_eq!(drawn.y_max, table.bbox.y_max + 1.0);
        table.column_widths.clear();
        assert_eq!(table_drawn_bounds(&table), table.bbox);
    }
}
