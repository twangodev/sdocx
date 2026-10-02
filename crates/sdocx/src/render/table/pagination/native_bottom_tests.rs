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
    widget_library_sha256: String,
    text_library_sha256: String,
    intersect_split_address: String,
    row_offset_address: String,
    update_bottom_address: String,
    measurement_inputs: String,
    capture_boundary: String,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    heights: Vec<f32>,
    widths: Vec<f32>,
    spans: Vec<[u32; 2]>,
    rectangles: Vec<[f32; 4]>,
    last_line_bottoms: Vec<Option<f32>>,
    cache_rows: Vec<usize>,
    cell_splits: Vec<Option<Vec<[f32; 4]>>>,
    initial_frames: Vec<[f32; 4]>,
    pending_gaps: Vec<f32>,
    changes: Vec<Change>,
}

#[derive(Deserialize)]
struct Change {
    row: usize,
    selected_band_bits: [u32; 4],
    offset_bits: u32,
    frames: Vec<[f32; 4]>,
    pending_gaps: Vec<f32>,
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
    [rect.x_min, rect.y_min, rect.x_max, rect.y_max].map(|coordinate| (coordinate as f32).to_bits())
}

fn frame_bits(plan: &PreparedTable) -> Vec<[u32; 4]> {
    plan.rows
        .iter()
        .flat_map(|row| &row.cells)
        .map(|cell| rect_bits(cell.frame))
        .collect()
}

#[test]
fn native_row_bottom_compression_uses_owner_bands_and_last_lines() {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/table-row-bottom.json"
    ));
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "c2ea6ef80738b101c32d54e910275c3a7308916da286684f89ad4af1af9e3d50"
    );
    let capture: Capture = serde_json::from_slice(bytes).unwrap();
    assert_eq!(capture.apk_version, "4.4.45.37");
    assert_eq!(capture.allocation_fills, [0, 165, 255]);
    for (actual, expected) in [
        (
            capture.apk_sha256,
            "daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667",
        ),
        (
            capture.model_library_sha256,
            "4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a",
        ),
        (
            capture.drawing_library_sha256,
            "788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd",
        ),
        (
            capture.base_library_sha256,
            "e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb",
        ),
        (
            capture.widget_library_sha256,
            "cfaaccbfd62763f0e514271cc372c0de7b6df41f0d2f991887b8b9584abd1ec9",
        ),
        (
            capture.text_library_sha256,
            "5483711673a499743625eb3275e34b37a006919af346212b46b8d8857834308b",
        ),
    ] {
        assert_eq!(actual, expected);
    }
    assert_eq!(capture.intersect_split_address, "0xb304c");
    assert_eq!(capture.row_offset_address, "0xb294c");
    assert_eq!(capture.update_bottom_address, "0xb28ec");
    assert_eq!(
        capture.measurement_inputs,
        "supplied cached two-line text metrics or null layouts, not native text shaping"
    );
    assert_eq!(
        capture.capture_boundary,
        "native row compression, owner lookup, lists and text getters; text initialization, single-thread mutex operations and diagnostic interfaces isolated"
    );
    assert_eq!(capture.cases.len(), 148);
    let mut changes = 0;
    let mut cells = 0;
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
        let mut rows = initialize_rows(&source, 0.5).unwrap();
        for (cell, bottom) in rows
            .iter_mut()
            .flat_map(|row| &mut row.cells)
            .zip(case.last_line_bottoms)
        {
            cell.metrics.first_line_height = bottom.map(|_| 10.25);
            cell.metrics.last_line_bottom = f64::from(bottom.unwrap_or(0.0));
            cell.metrics.measured_height = 300.0;
        }
        let mut plan = PreparedTable {
            callback_top: 0.0,
            content_bbox: BoundingBox::default(),
            topology: TableGrid::new(&source).unwrap(),
            measured_bbox: rect([0.0; 4]),
            rows,
            pending_gaps: case.pending_gaps.into_iter().map(f64::from).collect(),
            min_first_page_height: 0.0,
            constraint: ObjectSpanLayoutConstraint::OverPages,
            bands: BandList::new(case.rectangles.into_iter().map(rect).collect()).unwrap(),
            half_border: 0.5,
        };
        assert_eq!(
            frame_bits(&plan),
            case.initial_frames
                .iter()
                .map(|frame| frame.map(f32::to_bits))
                .collect::<Vec<_>>(),
            "{}, initialization",
            case.name
        );
        for row in case.cache_rows {
            update_split(&mut plan, row).unwrap();
        }
        let actual_splits: Vec<_> = plan
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
        assert_eq!(
            actual_splits,
            case.cell_splits
                .iter()
                .map(|bands| bands.as_ref().map(|bands| bands
                    .iter()
                    .map(|rect| rect.map(f32::to_bits))
                    .collect::<Vec<_>>()))
                .collect::<Vec<_>>(),
            "{}, initial splits",
            case.name
        );
        cells += actual_splits.len();
        for (index, change) in case.changes.into_iter().enumerate() {
            assert_eq!(
                last_intersect_split(&plan, change.row)
                    .unwrap()
                    .map_or([0; 4], rect_bits),
                change.selected_band_bits,
                "{}, change {index}, band",
                case.name
            );
            assert_eq!(
                (row_bottom_offset(&plan, change.row).unwrap() as f32).to_bits(),
                change.offset_bits,
                "{}, change {index}, offset",
                case.name
            );
            compress_row(&mut plan, change.row).unwrap();
            assert_eq!(
                frame_bits(&plan),
                change
                    .frames
                    .iter()
                    .map(|frame| frame.map(f32::to_bits))
                    .collect::<Vec<_>>(),
                "{}, change {index}, frames",
                case.name
            );
            assert_eq!(
                plan.pending_gaps
                    .iter()
                    .map(|gap| (*gap as f32).to_bits())
                    .collect::<Vec<_>>(),
                change
                    .pending_gaps
                    .into_iter()
                    .map(f32::to_bits)
                    .collect::<Vec<_>>(),
                "{}, change {index}, pending gaps",
                case.name
            );
            changes += 1;
            snapshots += actual_splits.len();
        }
    }
    assert_eq!(cells, 1216);
    assert_eq!(changes, 425);
    assert_eq!(snapshots, 4368);
}
