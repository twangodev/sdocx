use super::*;
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
struct Capture {
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    merged: bool,
    supplied_null_outer_pointer: bool,
    supplied_null_default_pointer: bool,
    outer_border: Option<[CapturedEdge; 4]>,
    default_border: Option<[CapturedEdge; 4]>,
    physical_cells: Vec<CapturedCell>,
    native_drawn_widths: Option<[f32; 4]>,
    native_raw_rect: [f32; 4],
    native_drawn_rect: [f32; 4],
}

#[derive(Clone, Copy, Deserialize)]
struct CapturedEdge {
    color: u32,
    width: f32,
}

#[derive(Deserialize)]
struct CapturedCell {
    slot: usize,
    own_border: Option<[CapturedEdge; 4]>,
}

fn border(edges: [CapturedEdge; 4]) -> TableBorder {
    let [left, top, right, bottom] = edges.map(|edge| crate::TableEdgeStyle {
        color: edge.color,
        width: edge.width,
        start_radius: 0.0,
        end_radius: 0.0,
    });
    TableBorder {
        left,
        top,
        right,
        bottom,
        metadata: Default::default(),
    }
}

#[test]
fn native_drawn_bounds_use_physical_edges_without_color_gating_and_f32_expansion() {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/table-drawn-bounds.json"
    ));
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "f610b86467e0be9b1ddcd18d2265c08ac4e7810a600115061948c3fc38efebae"
    );
    let capture: Capture = serde_json::from_slice(bytes).unwrap();
    assert_eq!(capture.cases.len(), 13);
    let mut resolved_cases = 0;
    for case in capture.cases {
        if case.supplied_null_outer_pointer {
            assert_eq!(case.native_drawn_widths, None);
            assert_eq!(case.native_drawn_rect, case.native_raw_rect);
            continue;
        }
        if case.supplied_null_default_pointer {
            assert!(case.default_border.is_none());
            assert_eq!(case.native_drawn_widths, Some([0.0; 4]));
            assert_eq!(case.native_drawn_rect, case.native_raw_rect);
            continue;
        }
        let mut table = tests::grid(&[100.0; 3], &[80.0; 3]);
        let [left, top, right, bottom] = case.native_raw_rect.map(f64::from);
        table.bbox = BoundingBox {
            x_min: left,
            y_min: top,
            x_max: right,
            y_max: bottom,
        };
        table.style.border = case.outer_border.map(border);
        table.style.default_cell_border = case.default_border.map(border);
        for cell in case.physical_cells {
            table.rows[cell.slot / 3].cells[cell.slot % 3].border = cell.own_border.map(border);
        }
        if case.merged {
            table.rows[0].cells[0].row_span = 3;
            table.rows[0].cells[0].column_span = 3;
        }
        assert_eq!(
            table_border_widths(&table).map(|width| (width as f32).to_bits()),
            case.native_drawn_widths.unwrap().map(f32::to_bits),
            "{} physical widths",
            case.name
        );
        let drawn = table_drawn_bounds(&table);
        assert_eq!(
            [drawn.x_min, drawn.y_min, drawn.x_max, drawn.y_max]
                .map(|coordinate| (coordinate as f32).to_bits()),
            case.native_drawn_rect.map(f32::to_bits),
            "{} drawn bounds",
            case.name
        );
        if case.name == "native-constructor-defaults" {
            table.style.border = None;
            table.style.default_cell_border = None;
            assert_eq!(table_drawn_bounds(&table), drawn);
        }
        resolved_cases += 1;
    }
    assert_eq!(resolved_cases, 11);
}
