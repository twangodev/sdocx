use crate::{BoundingBox, RichTextTable, RichTextTableCell, TableBorder};

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
    BoundingBox {
        x_min: table.bbox.x_min - left / 2.0,
        y_min: table.bbox.y_min - top / 2.0,
        x_max: table.bbox.x_max + right / 2.0,
        y_max: table.bbox.y_max + bottom / 2.0,
    }
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
