use crate::RichTextTable;

const MAX_GRID_CELLS: usize = 65_536;
const MAX_SPAN_COVERAGE: usize = 1_048_576;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::render) struct CellPosition {
    pub row: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::render) enum TableGridError {
    Empty,
    CellLimit,
    SparseRow,
    InvalidOrigin,
    InvalidSpan,
    CoverageLimit,
}

#[derive(Debug)]
pub(in crate::render) struct TableGrid {
    columns: usize,
    frame_owners: Vec<CellPosition>,
    visible_cells: Vec<CellPosition>,
}

impl TableGrid {
    pub fn new(table: &RichTextTable) -> Result<Self, TableGridError> {
        let rows = table.rows.len();
        let columns = table.column_widths.len();
        if rows == 0 || columns == 0 {
            return Err(TableGridError::Empty);
        }
        let count = rows
            .checked_mul(columns)
            .filter(|&count| count <= MAX_GRID_CELLS)
            .ok_or(TableGridError::CellLimit)?;
        let mut coverage = 0_usize;
        for (row_index, row) in table.rows.iter().enumerate() {
            if row.index as usize != row_index {
                return Err(TableGridError::InvalidOrigin);
            }
            if row.cells.len() != columns {
                return Err(TableGridError::SparseRow);
            }
            for (column, cell) in row.cells.iter().enumerate() {
                if cell.column_index as usize != column {
                    return Err(TableGridError::InvalidOrigin);
                }
                let row_span = cell.row_span as usize;
                let column_span = cell.column_span as usize;
                if row_span == 0
                    || column_span == 0
                    || row_span > rows - row_index
                    || column_span > columns - column
                {
                    return Err(TableGridError::InvalidSpan);
                }
                coverage = row_span
                    .checked_mul(column_span)
                    .and_then(|area| coverage.checked_add(area))
                    .filter(|&coverage| coverage <= MAX_SPAN_COVERAGE)
                    .ok_or(TableGridError::CoverageLimit)?;
            }
        }

        let mut frame_owners: Vec<_> = (0..count)
            .map(|index| CellPosition {
                row: index / columns,
                column: index % columns,
            })
            .collect();
        let mut covered = vec![false; count];
        let mut visible_cells = Vec::new();
        for (row_index, row) in table.rows.iter().enumerate() {
            for (column, cell) in row.cells.iter().enumerate() {
                let index = row_index * columns + column;
                let owner = frame_owners[index];
                let visible = !covered[index];
                if visible {
                    visible_cells.push(CellPosition {
                        row: row_index,
                        column,
                    });
                }
                for covered_row in row_index..row_index + cell.row_span as usize {
                    let start = covered_row * columns + column;
                    let end = start + cell.column_span as usize;
                    frame_owners[start..end].fill(owner);
                    if visible {
                        covered[start..end].fill(true);
                    }
                }
            }
        }
        Ok(Self {
            columns,
            frame_owners,
            visible_cells,
        })
    }

    pub fn visible_cells(&self) -> &[CellPosition] {
        &self.visible_cells
    }

    pub fn frame_owner(&self, position: CellPosition) -> Option<CellPosition> {
        if position.column >= self.columns {
            return None;
        }
        position
            .row
            .checked_mul(self.columns)
            .and_then(|index| index.checked_add(position.column))
            .and_then(|index| self.frame_owners.get(index))
            .copied()
    }

    pub fn requires_merged_layout(&self) -> bool {
        self.frame_owners
            .iter()
            .enumerate()
            .any(|(index, owner)| owner.row * self.columns + owner.column != index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::table::tests::grid;
    use serde::Deserialize;
    use sha2::{Digest, Sha256};

    #[derive(Deserialize)]
    struct NativeCapture {
        apk_version: String,
        apk_sha256: String,
        library_sha256: String,
        frame_owner_address: String,
        visible_cells_address: String,
        allocation_fills: Vec<u8>,
        cases: Vec<NativeCase>,
    }

    #[derive(Deserialize)]
    struct NativeCase {
        name: String,
        rows: usize,
        columns: usize,
        spans: Vec<[u32; 2]>,
        frame_owners: Vec<usize>,
        visible: Vec<usize>,
    }

    #[test]
    fn native_frame_owners_and_visible_lists_match_independently() {
        let bytes = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../conformance/table-ownership.json"
        ));
        assert_eq!(
            format!("{:x}", Sha256::digest(bytes)),
            "46130e510ab3c44b2364509b423782c0f945721392f6dec10574fc0cf938c4a8"
        );
        let capture: NativeCapture = serde_json::from_slice(bytes).unwrap();
        assert_eq!(capture.apk_version, "4.4.45.37");
        assert_eq!(
            capture.apk_sha256,
            "daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667"
        );
        assert_eq!(
            capture.library_sha256,
            "4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a"
        );
        assert_eq!(capture.frame_owner_address, "0x3c75c0");
        assert_eq!(capture.visible_cells_address, "0x3c7784");
        assert_eq!(capture.allocation_fills, [0, 0xa5, 0xff]);
        assert_eq!(capture.cases.len(), 279);
        let mut checked_positions = 0;
        for case in capture.cases {
            let mut table = grid(&vec![100.0; case.rows], &vec![100.0; case.columns]);
            assert_eq!(case.spans.len(), case.rows * case.columns);
            for (cell, [row_span, column_span]) in table
                .rows
                .iter_mut()
                .flat_map(|row| &mut row.cells)
                .zip(case.spans)
            {
                cell.row_span = row_span;
                cell.column_span = column_span;
            }
            let grid = TableGrid::new(&table).unwrap();
            let flatten = |positions: &[CellPosition]| {
                positions
                    .iter()
                    .map(|position| position.row * case.columns + position.column)
                    .collect::<Vec<_>>()
            };
            assert_eq!(
                flatten(&grid.frame_owners),
                case.frame_owners,
                "{}",
                case.name
            );
            assert_eq!(flatten(grid.visible_cells()), case.visible, "{}", case.name);
            checked_positions += case.rows * case.columns;
        }
        assert_eq!(checked_positions, 2399);
    }

    #[test]
    fn covered_span_chains_do_not_hide_paint_visible_cells() {
        let mut table = grid(&[100.0], &[100.0; 4]);
        table.rows[0].cells[0].column_span = 2;
        table.rows[0].cells[1].column_span = 3;
        let grid = TableGrid::new(&table).unwrap();
        assert!(grid.requires_merged_layout());
        assert_eq!(grid.frame_owners, [CellPosition { row: 0, column: 0 }; 4]);
        assert_eq!(
            grid.visible_cells(),
            [
                CellPosition { row: 0, column: 0 },
                CellPosition { row: 0, column: 2 },
                CellPosition { row: 0, column: 3 },
            ]
        );
    }

    #[test]
    fn sparse_and_reordered_records_are_rejected_without_rewriting_source() {
        let mut sparse = grid(&[100.0; 2], &[100.0; 2]);
        sparse.rows[0].cells.remove(0);
        assert_eq!(
            TableGrid::new(&sparse).unwrap_err(),
            TableGridError::SparseRow
        );
        assert_eq!(sparse.rows[0].cells[0].column_index, 1);
        let mut reordered = grid(&[100.0; 2], &[100.0; 2]);
        reordered.rows[0].cells.swap(0, 1);
        assert_eq!(
            TableGrid::new(&reordered).unwrap_err(),
            TableGridError::InvalidOrigin
        );
        reordered.rows[0].cells.swap(0, 1);
        reordered.rows[1].index = 0;
        assert_eq!(
            TableGrid::new(&reordered).unwrap_err(),
            TableGridError::InvalidOrigin
        );
    }

    #[test]
    fn zero_out_of_grid_and_overflowing_spans_are_rejected() {
        for [row_span, column_span] in [[0, 1], [1, 0], [3, 1], [1, 3], [u32::MAX, u32::MAX]] {
            let mut table = grid(&[100.0; 2], &[100.0; 2]);
            table.rows[0].cells[0].row_span = row_span;
            table.rows[0].cells[0].column_span = column_span;
            assert_eq!(
                TableGrid::new(&table).unwrap_err(),
                TableGridError::InvalidSpan
            );
        }
    }

    #[test]
    fn dense_grid_and_overlapping_coverage_have_distinct_work_limits() {
        let mut oversized = grid(&[100.0], &[100.0]);
        oversized.column_widths.resize(MAX_GRID_CELLS + 1, 100.0);
        assert_eq!(
            TableGrid::new(&oversized).unwrap_err(),
            TableGridError::CellLimit
        );
        let mut overlapping = grid(&[100.0; 64], &[100.0; 64]);
        for (row_index, row) in overlapping.rows.iter_mut().enumerate() {
            for (column, cell) in row.cells.iter_mut().enumerate() {
                cell.row_span = (64 - row_index) as u32;
                cell.column_span = (64 - column) as u32;
            }
        }
        assert_eq!(
            TableGrid::new(&overlapping).unwrap_err(),
            TableGridError::CoverageLimit
        );
    }
}
