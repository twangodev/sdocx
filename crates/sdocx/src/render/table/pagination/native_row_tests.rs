use super::super::{CellPosition, TableGrid, initialize_rows, tests::grid};
use super::*;
use crate::ObjectSpanLayoutConstraint;
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
struct Capture {
    apk_version: String,
    apk_sha256: String,
    model_library_sha256: String,
    drawing_library_sha256: String,
    base_library_sha256: String,
    widget_library_sha256: String,
    update_rows_address: String,
    measured_height_address: String,
    measurement_inputs: String,
    allocation_fills: Vec<u8>,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    heights: Vec<f32>,
    widths: Vec<f32>,
    spans: Vec<[u32; 2]>,
    frame_owners: Vec<usize>,
    minima: Vec<f32>,
    maxima: Vec<f32>,
    measured_heights: Vec<Option<f32>>,
    start: usize,
    pending_gaps: Vec<f32>,
    initial_frames: Vec<[f32; 4]>,
    frames: Vec<[f32; 4]>,
    cached_frames: Vec<[f32; 4]>,
    final_pending_gaps: Vec<f32>,
}

fn frame_bits(rows: &[super::super::PreparedTableRow]) -> Vec<[u32; 4]> {
    rows.iter()
        .flat_map(|row| &row.cells)
        .map(|cell| {
            let frame = cell.frame;
            [frame.x_min, frame.y_min, frame.x_max, frame.y_max]
                .map(|coordinate| (coordinate as f32).to_bits())
        })
        .collect()
}

#[test]
fn native_warm_row_sizing_uses_owner_measurements() {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/table-warm-rows.json"
    ));
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "4a1b17063dadcf9d8d3ca819dbf92b6f75f689f87b3e1f38ab4369d3f215610e"
    );
    let capture: Capture = serde_json::from_slice(bytes).unwrap();
    assert_eq!(capture.apk_version, "4.4.45.37");
    assert_eq!(capture.allocation_fills, [0, 165, 255]);
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
    assert_eq!(
        capture.widget_library_sha256,
        "cfaaccbfd62763f0e514271cc372c0de7b6df41f0d2f991887b8b9584abd1ec9"
    );
    assert_eq!(capture.update_rows_address, "0xaecf4");
    assert_eq!(capture.measured_height_address, "0xd3b78");
    assert_eq!(
        capture.measurement_inputs,
        "supplied cached heights or null layouts, not native text shaping"
    );
    assert_eq!(capture.cases.len(), 142);
    let mut cells = 0;
    for case in capture.cases {
        let mut source = grid(&case.heights, &case.widths);
        let columns = case.widths.len();
        for (cell, span) in source
            .rows
            .iter_mut()
            .flat_map(|row| &mut row.cells)
            .zip(case.spans)
        {
            [cell.row_span, cell.column_span] = span;
        }
        for ((row, minimum), maximum) in source.rows.iter_mut().zip(case.minima).zip(case.maxima) {
            row.min_height = Some(minimum);
            row.max_height = Some(maximum);
        }
        let topology = TableGrid::new(&source).unwrap();
        let owners: Vec<_> = (0..source.rows.len() * columns)
            .map(|slot| {
                let owner = topology
                    .frame_owner(CellPosition {
                        row: slot / columns,
                        column: slot % columns,
                    })
                    .unwrap();
                owner.row * columns + owner.column
            })
            .collect();
        assert_eq!(owners, case.frame_owners, "{}, owners", case.name);
        let mut rows = initialize_rows(&source, 0.5).unwrap();
        assert_eq!(
            frame_bits(&rows),
            case.initial_frames
                .iter()
                .map(|frame| frame.map(f32::to_bits))
                .collect::<Vec<_>>(),
            "{}, initialization",
            case.name
        );
        for (cell, height) in rows
            .iter_mut()
            .flat_map(|row| &mut row.cells)
            .zip(case.measured_heights)
        {
            cell.metrics.measured_height = f64::from(height.unwrap_or(0.0));
        }
        let mut plan = PreparedTable {
            topology,
            measured_bbox: BoundingBox {
                x_min: 0.0,
                y_min: 0.0,
                x_max: 0.0,
                y_max: 0.0,
            },
            rows,
            pending_gaps: case.pending_gaps.into_iter().map(f64::from).collect(),
            min_first_page_height: 0.0,
            constraint: ObjectSpanLayoutConstraint::OverPages,
            bands: BandList::default(),
            half_border: 0.5,
        };
        update_positions(&mut plan, case.start, &source).unwrap();
        let actual = frame_bits(&plan.rows);
        for expected in [case.frames, case.cached_frames] {
            assert_eq!(
                actual,
                expected
                    .iter()
                    .map(|frame| frame.map(f32::to_bits))
                    .collect::<Vec<_>>(),
                "{}, updated frames",
                case.name
            );
        }
        assert_eq!(
            plan.pending_gaps
                .into_iter()
                .map(|gap| (gap as f32).to_bits())
                .collect::<Vec<_>>(),
            case.final_pending_gaps
                .into_iter()
                .map(f32::to_bits)
                .collect::<Vec<_>>(),
            "{}, pending gaps",
            case.name
        );
        cells += actual.len();
    }
    assert_eq!(cells, 1189);
}
