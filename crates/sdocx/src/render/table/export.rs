use crate::{BoundingBox, DocumentMetadata, Page};

#[derive(Clone, Copy)]
pub(in crate::render) struct TableExportPage {
    body_bounds: [f32; 4],
    origin: [f64; 2],
}

impl TableExportPage {
    fn new(size: [i32; 2], margins: [f32; 2]) -> Self {
        Self {
            body_bounds: [0.0, margins[0], size[0] as f32, size[1] as f32 - margins[1]],
            origin: [0.0; 2],
        }
    }

    pub fn for_page(page: &Page, metadata: &DocumentMetadata) -> Option<Self> {
        let margins = if metadata.page_mode == Some(0) {
            let stored = metadata
                .note_text
                .as_ref()
                .and_then(|text| text.margins)
                .unwrap_or([0.0; 4]);
            [stored[1], stored[3]].map(|margin| margin * metadata.document_density())
        } else {
            [0.0; 2]
        };
        margins
            .into_iter()
            .all(f32::is_finite)
            .then(|| Self::new([page.width as i32, page.height as i32], margins))
    }

    pub fn translated(mut self, dx: f64, dy: f64) -> Self {
        self.origin[0] += dx;
        self.origin[1] += dy;
        self
    }

    pub fn artwork_bounds(self, measured: BoundingBox) -> BoundingBox {
        let local = [
            (measured.x_min - self.origin[0]) as f32,
            (measured.y_min - self.origin[1]) as f32,
            (measured.x_max - self.origin[0]) as f32,
            (measured.y_max - self.origin[1]) as f32,
        ];
        let mut bounds = self.body_bounds;
        if bounds[0] < local[2]
            && local[0] < bounds[2]
            && bounds[1] < local[3]
            && local[1] < bounds[3]
        {
            bounds = [
                bounds[0].max(local[0]),
                bounds[1].max(local[1]),
                bounds[2].min(local[2]),
                bounds[3].min(local[3]),
            ];
        }
        let bounds = expanded_bounds(bounds);
        BoundingBox {
            x_min: bounds.x_min + self.origin[0],
            y_min: bounds.y_min + self.origin[1],
            x_max: bounds.x_max + self.origin[0],
            y_max: bounds.y_max + self.origin[1],
        }
    }
}

pub(in crate::render) fn artwork_bounds(
    measured: BoundingBox,
    page: Option<TableExportPage>,
) -> BoundingBox {
    page.map_or_else(
        || {
            expanded_bounds(
                [
                    measured.x_min,
                    measured.y_min,
                    measured.x_max,
                    measured.y_max,
                ]
                .map(|value| value as f32),
            )
        },
        |page| page.artwork_bounds(measured),
    )
}

fn expanded_bounds([left, top, right, bottom]: [f32; 4]) -> BoundingBox {
    let [x_min, y_min, x_max, y_max] = [
        (left - 1.0).floor(),
        (top - 1.0).floor(),
        (right + 1.0).ceil(),
        (bottom + 1.0).ceil(),
    ]
    .map(f64::from);
    BoundingBox {
        x_min,
        y_min,
        x_max,
        y_max,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use sha2::{Digest, Sha256};

    #[derive(Deserialize)]
    struct Capture {
        apk_version: String,
        apk_sha256: String,
        allocation_fills: Vec<u8>,
        model_library_sha256: String,
        drawing_library_sha256: String,
        base_library_sha256: String,
        composer_library_sha256: String,
        constructor_address: String,
        crop_start: String,
        crop_end: String,
        measurement_inputs: String,
        cases: Vec<Case>,
    }

    #[derive(Deserialize)]
    struct Case {
        name: String,
        page_size: [i32; 2],
        scaled_margins: [f32; 2],
        measured_bbox: [f32; 4],
        background_crop: [f32; 4],
        default_focus: [f32; 4],
        default_clip_counts: [usize; 3],
    }

    fn bounds([left, top, right, bottom]: [f32; 4]) -> BoundingBox {
        BoundingBox {
            x_min: f64::from(left),
            y_min: f64::from(top),
            x_max: f64::from(right),
            y_max: f64::from(bottom),
        }
    }

    #[test]
    fn native_export_crops_match_every_coordinate_bit() {
        let bytes = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../conformance/table-export-clipping.json"
        ));
        assert_eq!(
            format!("{:x}", Sha256::digest(bytes)),
            "872b937dedb795d3b995249c4ab26ae421d3f6f0eaec909ccf396038cf7d38b8"
        );
        let capture: Capture = serde_json::from_slice(bytes).unwrap();
        assert_eq!(capture.apk_version, "4.4.45.37");
        assert_eq!(
            capture.apk_sha256,
            "daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667"
        );
        assert_eq!(capture.allocation_fills, [0, 165, 255]);
        assert_eq!(
            capture.model_library_sha256,
            "4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a"
        );
        assert_eq!(
            capture.drawing_library_sha256,
            "788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd"
        );
        assert_eq!(
            capture.base_library_sha256,
            "e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb"
        );
        assert_eq!(
            capture.composer_library_sha256,
            "52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f"
        );
        assert_eq!(capture.constructor_address, "0xa5634");
        assert_eq!(capture.crop_start, "0x37e534");
        assert_eq!(capture.crop_end, "0x37e56c");
        assert!(
            capture
                .measurement_inputs
                .starts_with("Supplied world measured bounds")
        );
        assert_eq!(capture.cases.len(), 78);
        for case in capture.cases {
            let actual = TableExportPage::new(case.page_size, case.scaled_margins)
                .artwork_bounds(bounds(case.measured_bbox));
            assert_eq!(
                [actual.x_min, actual.y_min, actual.x_max, actual.y_max]
                    .map(|value| (value as f32).to_bits()),
                case.background_crop.map(f32::to_bits),
                "{}",
                case.name,
            );
            assert_eq!(
                case.default_focus.map(f32::to_bits),
                [0; 4],
                "{}",
                case.name
            );
            assert_eq!(case.default_clip_counts, [0; 3], "{}", case.name);
        }
    }

    #[test]
    fn translated_page_crops_stay_in_the_active_paint_coordinates() {
        let page = TableExportPage::new([100, 200], [10.0, 20.0]).translated(-5.25, 200.0);
        assert_eq!(
            page.artwork_bounds(bounds([-20.25, 170.0, 120.0, 421.0])),
            bounds([-6.25, 209.0, 95.75, 381.0]),
        );
    }

    #[test]
    fn artwork_visibility_includes_border_ink_beyond_the_cell_frame() {
        use super::super::{CellFill, CellPosition, TableBorderGeometry, TableGrid, tests::grid};
        use crate::render::{RenderTheme, TableCellPaint, viewport::Viewport};

        let table = grid(&[100.0], &[100.0]);
        let grid = TableGrid::new(&table).unwrap();
        let borders = TableBorderGeometry::new(&table, &grid).unwrap();
        let cell = &table.rows[0].cells[0];
        let mut paint = TableCellPaint {
            cell,
            fill: CellFill::resolve(&table.style, 0, cell, RenderTheme::for_canvas(false)),
            frame: bounds([0.0, -100.0, 100.0, -0.25]),
            layout: None,
            position: CellPosition { row: 0, column: 0 },
            gap: false,
        };
        let viewport = Viewport::new(bounds([0.0, 0.0, 100.0, 100.0]));
        assert!(!viewport.intersects(paint.frame));
        assert!(viewport.intersects(paint.artwork_bounds(Some(&borders), 0.0, true)));
        paint.frame.y_max = -0.5;
        assert!(!viewport.intersects(paint.artwork_bounds(Some(&borders), 0.0, true)));
    }
}
