use crate::{
    BoundingBox, ObjectSpanLayoutConstraint, RichTextTable, RichTextTableCell, TableAutoFit,
    TableBorder,
};

use super::RenderTheme;
use super::text::{
    ObjectDiagnosticKind, StyledText, TextContext, TextFrame, TextLayout, TextRenderer,
    finite_native_geometry,
};

pub(super) struct PreparedTable {
    pub measured_bbox: BoundingBox,
    pub rows: Vec<PreparedTableRow>,
    pub min_first_page_height: f64,
    constraint: ObjectSpanLayoutConstraint,
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
}

pub(super) struct PreparedTableRow {
    pub row_index: usize,
    pub cells: Vec<PreparedTableCell>,
}

pub(super) struct PreparedTableCell {
    pub column_index: usize,
    pub frame: BoundingBox,
    pub layout: TextLayout,
}

pub(super) fn prepare_table(
    table: &RichTextTable,
    constraint: ObjectSpanLayoutConstraint,
    candidate_top: f64,
    theme: RenderTheme,
    renderer: &TextRenderer<'_>,
) -> Option<Result<PreparedTable, ObjectDiagnosticKind>> {
    if !supports_cold_grid(table, constraint)
        || !renderer
            .object_exclusions(constraint, candidate_top, 0.0)
            .is_empty()
    {
        return None;
    }
    if !valid_grid_geometry(table, candidate_top) {
        return Some(Err(ObjectDiagnosticKind::InvalidBounds));
    }
    if table
        .column_widths
        .iter()
        .any(|width| width.fract() != 0.0 || f64::from(*width) > f64::from(i32::MAX))
    {
        return None;
    }
    Some(prepare_cold_grid(table, constraint, theme, renderer))
}

fn supports_cold_grid(table: &RichTextTable, constraint: ObjectSpanLayoutConstraint) -> bool {
    matches!(
        constraint,
        ObjectSpanLayoutConstraint::OverPages | ObjectSpanLayoutConstraint::OverPagesOverlapPadding
    ) && table
        .rotation_degrees
        .is_none_or(|rotation| rotation == 0.0)
        && !table.column_widths.is_empty()
        && !table.rows.is_empty()
        && !table.style.max_height_enabled
        && table
            .style
            .auto_fit
            .is_none_or(|fit| fit == TableAutoFit::None)
        && table.style.min_column_widths.is_none()
        && table.style.max_column_widths.is_none()
        && table.style.max_width.is_none()
        && table.style.vertical_cell_padding.is_none()
        && table.style.horizontal_cell_padding.is_none()
        && table.style.content_bbox.is_none()
        && table.rows.iter().enumerate().all(|(row_index, row)| {
            row.index as usize == row_index
                && row.min_height.is_none_or(|height| height == 0.0)
                && row.max_height.is_none_or(|height| height == f32::MAX)
                && row.cells.len() == table.column_widths.len()
                && row.cells.iter().enumerate().all(|(column_index, cell)| {
                    cell.column_index as usize == column_index
                        && cell.column_span == 1
                        && cell.row_span == 1
                        && cell.content.object_spans.is_empty()
                        && cell
                            .content
                            .rotation_degrees
                            .is_none_or(|rotation| rotation == 0.0)
                })
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
        && table
            .rows
            .iter()
            .all(|row| row.height.is_finite() && row.height >= 0.0)
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
    constraint: ObjectSpanLayoutConstraint,
    theme: RenderTheme,
    renderer: &TextRenderer<'_>,
) -> Result<PreparedTable, ObjectDiagnosticKind> {
    let half_border = drawable_half_border(table);
    let mut top = half_border;
    let mut rows = Vec::with_capacity(table.rows.len());
    for (row_index, row) in table.rows.iter().enumerate() {
        let mut height = f64::from(row.height);
        let mut left = half_border;
        let mut cells = Vec::with_capacity(row.cells.len());
        for (column_index, cell) in row.cells.iter().enumerate() {
            let right = left + f64::from(table.column_widths[column_index]);
            let frame = BoundingBox {
                x_min: left,
                y_min: top,
                x_max: right,
                y_max: top + f64::from(row.height),
            };
            if [left, right, top, frame.y_max]
                .into_iter()
                .any(|value| finite_native_geometry(value).is_none())
                || right <= left
            {
                return Err(ObjectDiagnosticKind::InvalidBounds);
            }
            let styled = StyledText::new(&cell.content, TextContext::Flow, renderer.settings);
            let cell_theme = theme.on_background(super::table_cell_background(cell, theme));
            let layout = super::text::layout_text(
                &styled,
                TextFrame {
                    bbox: frame,
                    gravity: Some(0),
                    exclusions: &[],
                },
                cell_theme,
                renderer,
            );
            if !valid_cell_layout(&layout) {
                return Err(ObjectDiagnosticKind::InvalidBounds);
            }
            height = height.max(native_measured_height(layout.height())?);
            cells.push(PreparedTableCell {
                column_index,
                frame,
                layout,
            });
            left = right;
        }
        top += height;
        if finite_native_geometry(top).is_none() {
            return Err(ObjectDiagnosticKind::InvalidBounds);
        }
        for cell in &mut cells {
            cell.frame.y_max = top;
        }
        rows.push(PreparedTableRow { row_index, cells });
    }
    let right = rows[0].cells.last().unwrap().frame.x_max;
    let content_bbox = BoundingBox {
        x_min: half_border,
        y_min: half_border,
        x_max: right,
        y_max: top,
    };
    let [left, upper, right_border, bottom] = table_border_widths(table);
    let measured_bbox = BoundingBox {
        x_min: 0.0,
        y_min: 0.0,
        x_max: content_bbox.x_max - content_bbox.x_min + (left + right_border) / 2.0,
        y_max: content_bbox.y_max - content_bbox.y_min + (upper + bottom) / 2.0,
    };
    if [measured_bbox.x_max, measured_bbox.y_max]
        .into_iter()
        .any(|value| finite_native_geometry(value).is_none())
        || measured_bbox.x_max <= 0.0
        || measured_bbox.y_max <= 0.0
    {
        return Err(ObjectDiagnosticKind::InvalidBounds);
    }
    let mut first_row_height = 0.0_f64;
    for cell in &rows[0].cells {
        let source = &table.rows[0].cells[cell.column_index].content;
        let height = if source.text.is_empty() {
            cell.layout.height()
        } else {
            let line = cell.layout.lines.first().unwrap();
            native_measured_height(line.bottom - line.top)?
                + renderer
                    .settings
                    .pixels(source.margins.unwrap_or([0.0; 4])[1])
        };
        first_row_height = first_row_height.max(native_measured_height(height)?);
    }
    if first_row_height == 0.0 {
        first_row_height = rows[0].cells[0].frame.y_max - rows[0].cells[0].frame.y_min;
    }
    let min_first_page_height =
        native_measured_height(first_row_height + content_bbox.y_min - measured_bbox.y_min)?;
    Ok(PreparedTable {
        measured_bbox,
        rows,
        min_first_page_height,
        constraint,
    })
}

#[derive(Clone, Copy)]
enum Edge {
    Left,
    Top,
    Right,
    Bottom,
}

impl Edge {
    fn width(self, border: Option<&TableBorder>) -> f64 {
        border.map_or(1.0, |border| {
            f64::from(match self {
                Self::Left => border.left.width,
                Self::Top => border.top.width,
                Self::Right => border.right.width,
                Self::Bottom => border.bottom.width,
            })
        })
    }
}

pub(super) fn table_drawn_bounds(table: &RichTextTable) -> BoundingBox {
    let [left, top, right, bottom] = table_border_widths(table);
    BoundingBox {
        x_min: table.bbox.x_min - left / 2.0,
        y_min: table.bbox.y_min - top / 2.0,
        x_max: table.bbox.x_max + right / 2.0,
        y_max: table.bbox.y_max + bottom / 2.0,
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
mod tests {
    use super::*;
    use crate::{RichTextBox, RichTextTableRow, TableEdgeStyle, TableRecordMetadata, TableStyle};

    fn grid(heights: &[f32], widths: &[f32]) -> RichTextTable {
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

    fn prepared(table: &RichTextTable) -> PreparedTable {
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
        assert_eq!(plan.rows[0].cells[0].frame.y_max, 140.5);
        assert_eq!(plan.rows[1].cells[0].frame.y_min, 140.5);
        assert_eq!(plan.rows[1].cells[0].frame.y_max, 240.5);
        assert_eq!(plan.measured_bbox.y_max, 241.0);
        assert_eq!(plan.min_first_page_height, 140.5);
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
        assert_eq!(plan.min_first_page_height, 55.0);
        table.style.border = Some(border([2.0, 4.0, 6.0, 8.0], 0));
        let transparent = prepared(&table);
        assert_eq!(transparent.rows[0].cells[0].frame.x_min, 0.0);
        assert_eq!(transparent.rows[0].cells[0].frame.y_min, 0.0);
        assert_eq!(transparent.measured_bbox, plan.measured_bbox);
    }

    #[test]
    fn child_split_bands_leave_the_stateful_table_path_explicitly_unsupported() {
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
            assert!(
                prepare_table(
                    &table,
                    constraint,
                    100.0,
                    RenderTheme::for_canvas(false),
                    &renderer
                )
                .is_none()
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
        assert!(prepare(&fractional, ObjectSpanLayoutConstraint::OverPages).is_none());
        let mut overridden = original.clone();
        overridden.style.vertical_cell_padding = Some(0.0);
        assert!(prepare(&overridden, ObjectSpanLayoutConstraint::OverPages).is_none());
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
        let plan = prepared(&table);
        assert_eq!(plan.rows[0].cells[0].layout.height(), 0.0);
        assert_eq!(plan.min_first_page_height, 100.5);
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
                vertical_cell_padding: None,
                horizontal_cell_padding: None,
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
