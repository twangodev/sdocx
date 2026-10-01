use super::super::{TableGrid, initialize_rows, tests::grid};
use super::*;
use crate::ObjectSpanLayoutConstraint;
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
    update_split_address: String,
    cache_comparison_address: String,
    capture_boundary: String,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    heights: Vec<f32>,
    widths: Vec<f32>,
    spans: Vec<[u32; 2]>,
    frames: Vec<[f32; 4]>,
    updates: Vec<Update>,
}

#[derive(Deserialize)]
struct Update {
    row: usize,
    rectangles: Vec<[f32; 4]>,
    changed: bool,
    cell_splits: Vec<Option<Vec<[f32; 4]>>>,
}

fn rect([x_min, y_min, x_max, y_max]: [f32; 4]) -> BoundingBox {
    BoundingBox {
        x_min: f64::from(x_min),
        y_min: f64::from(y_min),
        x_max: f64::from(x_max),
        y_max: f64::from(y_max),
    }
}

fn rect_bits(rect: BoundingBox) -> [u32; 4] {
    [rect.x_min, rect.y_min, rect.x_max, rect.y_max].map(|value| (value as f32).to_bits())
}

#[test]
fn native_row_split_updates_preserve_owner_caches_and_stale_later_rectangles() {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/table-row-splits.json"
    ));
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "0e77850a6312268018e9e27ceeab0dc7f41654690de13e0171982b9fa6f2ff2e"
    );
    let capture: Capture = serde_json::from_slice(bytes).unwrap();
    assert_eq!(capture.apk_version, "4.4.45.37");
    assert_eq!(
        capture.apk_sha256,
        "daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667"
    );
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
    assert_eq!(capture.allocation_fills, [0, 165, 255]);
    assert_eq!(capture.update_split_address, "0xb24cc");
    assert_eq!(capture.cache_comparison_address, "0xb2e18");
    assert_eq!(
        capture.capture_boundary,
        "native frame and list selection with supplied bands; text initialization, single-thread mutex operations and diagnostic interfaces isolated"
    );
    assert_eq!(capture.cases.len(), 139);
    let mut updates = 0;
    let mut snapshots = 0;
    for case in capture.cases {
        let mut source = grid(&case.heights, &case.widths);
        for (cell, span) in source
            .rows
            .iter_mut()
            .flat_map(|row| &mut row.cells)
            .zip(case.spans)
        {
            [cell.row_span, cell.column_span] = span;
        }
        let rows = initialize_rows(&source, 0.5).unwrap();
        assert_eq!(
            rows.iter()
                .flat_map(|row| &row.cells)
                .map(|cell| rect_bits(cell.frame))
                .collect::<Vec<_>>(),
            case.frames
                .iter()
                .map(|frame| frame.map(f32::to_bits))
                .collect::<Vec<_>>(),
            "{}, initialization",
            case.name
        );
        let mut plan = PreparedTable {
            content_bbox: BoundingBox::default(),
            measured_bbox: rect([0.0; 4]),
            topology: TableGrid::new(&source).unwrap(),
            pending_gaps: vec![0.0; rows.len()],
            rows,
            min_first_page_height: 0.0,
            constraint: ObjectSpanLayoutConstraint::OverPages,
            bands: BandList::default(),
            half_border: 0.5,
        };
        for (index, update) in case.updates.into_iter().enumerate() {
            plan.bands = BandList::new(update.rectangles.into_iter().map(rect).collect()).unwrap();
            assert_eq!(
                update_split(&mut plan, update.row).unwrap(),
                update.changed,
                "{}, update {index}, changed",
                case.name
            );
            let actual: Vec<_> = plan
                .rows
                .iter()
                .flat_map(|row| &row.cells)
                .map(|cell| {
                    cell.bands.as_ref().map(|bands| {
                        bands
                            .rectangles
                            .iter()
                            .copied()
                            .map(rect_bits)
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            let expected: Vec<_> = update
                .cell_splits
                .into_iter()
                .map(|bands| {
                    bands.map(|bands| {
                        bands
                            .into_iter()
                            .map(|frame| frame.map(f32::to_bits))
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            assert_eq!(
                actual, expected,
                "{}, update {index}, cell caches",
                case.name
            );
            updates += 1;
            snapshots += plan.rows.iter().map(|row| row.cells.len()).sum::<usize>();
        }
    }
    assert_eq!(updates, 1068);
    assert_eq!(snapshots, 10828);
}
