use crate::{Color, RichTextTableCell, TableStyle};

use super::super::RenderTheme;

#[derive(Clone, Copy)]
pub(in crate::render) struct CellFill {
    pub color: Color,
    pub opacity: f64,
}

impl CellFill {
    pub fn resolve(
        style: &TableStyle,
        row_index: u32,
        cell: &RichTextTableCell,
        theme: RenderTheme,
    ) -> Self {
        let argb = selected_argb(style, row_index, cell, true);
        Self {
            color: theme.span_color(super::super::argb_color(argb)),
            opacity: f64::from(argb >> 24) / 255.0,
        }
    }

    pub fn theme(self, theme: RenderTheme) -> RenderTheme {
        theme.on_surface(self.color, self.opacity)
    }
}

fn selected_argb(
    style: &TableStyle,
    row_index: u32,
    cell: &RichTextTableCell,
    honor_heading: bool,
) -> u32 {
    let heading = (style.heading_row_enabled && row_index == 0)
        || (style.heading_column_enabled && cell.column_index == 0);
    if cell.has_own_background_color && !(heading && honor_heading) {
        cell.background_color
    } else if heading {
        style.heading_background_color.unwrap_or(0)
    } else {
        style.default_cell_background_color.unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::table::tests::grid;
    use serde::Deserialize;
    use sha2::{Digest, Sha256};

    #[derive(Deserialize)]
    struct Capture {
        apk_version: String,
        apk_sha256: String,
        library_sha256: String,
        cell_background_address: String,
        table_background_address: String,
        cases: Vec<Case>,
    }

    #[derive(Deserialize)]
    struct Case {
        heading_row: bool,
        heading_column: bool,
        heading_color: u32,
        default_color: u32,
        spans: Vec<[u32; 2]>,
        cells: Vec<Cell>,
        without_heading_override: Vec<u32>,
        with_heading_override: Vec<u32>,
    }

    #[derive(Deserialize)]
    struct Cell {
        color: u32,
        owned: bool,
    }

    #[test]
    fn background_selection_matches_native_cell_and_table_getters() {
        let bytes = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../conformance/table-backgrounds.json"
        ));
        assert_eq!(
            format!("{:x}", Sha256::digest(bytes)),
            "46fe91b4a788c5276534b641b4a7224604a2410154724cd8ce56296809b78f44"
        );
        let capture: Capture = serde_json::from_slice(bytes).unwrap();
        assert_eq!(capture.apk_version, "4.4.45.37");
        assert_eq!(
            capture.apk_sha256,
            "daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667"
        );
        assert_eq!(
            capture.library_sha256,
            "4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a"
        );
        assert_eq!(capture.cell_background_address, "0x3c2384");
        assert_eq!(capture.table_background_address, "0x3cbf7c");
        assert_eq!(capture.cases.len(), 40);
        for (index, case) in capture.cases.into_iter().enumerate() {
            let mut table = grid(&[10.0; 3], &[20.0; 3]);
            table.style.heading_row_enabled = case.heading_row;
            table.style.heading_column_enabled = case.heading_column;
            table.style.heading_background_color = Some(case.heading_color);
            table.style.default_cell_background_color = Some(case.default_color);
            assert_eq!(case.cells.len(), 9);
            assert_eq!(case.spans.len(), 9);
            assert_eq!(case.with_heading_override.len(), 9);
            assert_eq!(case.without_heading_override.len(), 9);
            for (position, (cell, captured)) in table
                .rows
                .iter_mut()
                .flat_map(|row| &mut row.cells)
                .zip(case.cells)
                .enumerate()
            {
                [cell.row_span, cell.column_span] = case.spans[position];
                cell.background_color = captured.color;
                cell.has_own_background_color = captured.owned;
                for (honor_heading, expected) in [
                    (false, case.without_heading_override[position]),
                    (true, case.with_heading_override[position]),
                ] {
                    assert_eq!(
                        selected_argb(&table.style, (position / 3) as u32, cell, honor_heading),
                        expected,
                        "case {index}, cell {position}, heading override {honor_heading}"
                    );
                }
            }
        }
    }
}
