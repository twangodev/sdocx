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
    cold_rows_address: String,
    update_cell_address: String,
    measure_cell_address: String,
    measurement_inputs: String,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    heights: Vec<f32>,
    widths: Vec<f32>,
    spans: Vec<[u32; 2]>,
    minima: Vec<f32>,
    maxima: Vec<f32>,
    pending_gaps: Vec<f32>,
    initial_frames: Vec<[f32; 4]>,
    runs: Vec<Run>,
}

#[derive(Deserialize)]
struct Run {
    start: usize,
    rectangles: Vec<[f32; 4]>,
    measured_heights: Vec<f32>,
    calls: Vec<Call>,
    frames: Vec<[f32; 4]>,
    pending_gaps: Vec<f32>,
    cell_splits: Vec<Option<Vec<[f32; 4]>>>,
}

#[derive(Deserialize)]
struct Call {
    slot: usize,
    dimensions: [f32; 2],
    rectangles: Vec<[f32; 4]>,
}

fn rect([x_min, y_min, x_max, y_max]: [f32; 4]) -> BoundingBox {
    BoundingBox {
        x_min: f64::from(x_min),
        y_min: f64::from(y_min),
        x_max: f64::from(x_max),
        y_max: f64::from(y_max),
    }
}

fn bits(rect: BoundingBox) -> [u32; 4] {
    [rect.x_min, rect.y_min, rect.x_max, rect.y_max].map(|value| (value as f32).to_bits())
}

fn frame_bits(plan: &PreparedTable) -> Vec<[u32; 4]> {
    plan.rows
        .iter()
        .flat_map(|row| &row.cells)
        .map(|cell| bits(cell.frame))
        .collect()
}

impl Case {
    fn plan(&self) -> PreparedTable {
        let mut source = grid(&self.heights, &self.widths);
        for (cell, span) in source
            .rows
            .iter_mut()
            .flat_map(|row| &mut row.cells)
            .zip(&self.spans)
        {
            [cell.row_span, cell.column_span] = *span;
        }
        for (row, (minimum, maximum)) in source
            .rows
            .iter_mut()
            .zip(self.minima.iter().zip(&self.maxima))
        {
            row.min_height = Some(*minimum);
            row.max_height = Some(*maximum);
        }
        let plan = PreparedTable {
            topology: TableGrid::new(&source).unwrap(),
            rows: initialize_rows(&source, 0.5).unwrap(),
            measured_bbox: rect([0.0; 4]),
            pending_gaps: self.pending_gaps.iter().copied().map(f64::from).collect(),
            min_first_page_height: 0.0,
            constraint: ObjectSpanLayoutConstraint::OverPages,
            bands: BandList::default(),
            half_border: 0.5,
        };
        assert_eq!(
            frame_bits(&plan),
            self.initial_frames
                .iter()
                .map(|r| r.map(f32::to_bits))
                .collect::<Vec<_>>(),
            "{}",
            self.name
        );
        plan
    }
}

#[test]
fn native_cold_rows_measure_raw_cells_and_only_grow_frames() {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/table-cold-rows.json"
    ));
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "bbdb16741d4e80d36a583cb16d792400968f2c49bf3757083915c57546dd6202"
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
        capture.widget_library_sha256,
        "cfaaccbfd62763f0e514271cc372c0de7b6df41f0d2f991887b8b9584abd1ec9"
    );
    assert_eq!(capture.cold_rows_address, "0xaaf2c");
    assert_eq!(capture.update_cell_address, "0xae914");
    assert_eq!(capture.measure_cell_address, "0xaea60");
    assert_eq!(
        capture.measurement_inputs,
        "supplied cached measured heights; native cold driver, cell frame inputs, measurement differences and row updates; text update, measurement, padding assignment and font selection intercepted"
    );
    assert_eq!(capture.cases.len(), 144);
    let mut run_count = 0;
    let mut call_count = 0;
    let mut snapshots = 0;
    for case in capture.cases {
        let mut plan = case.plan();
        for (run_index, run) in case.runs.into_iter().enumerate() {
            run_count += 1;
            plan.bands = BandList::new(run.rectangles.iter().copied().map(rect).collect()).unwrap();
            let mut observed = Vec::new();
            cold_from_row_using(&mut plan, run.start, |plan, position| {
                let slot = position.row * case.widths.len() + position.column;
                let cell = &mut plan.rows[position.row].cells[position.column];
                observed.push(Call {
                    slot,
                    dimensions: [
                        native_sub(cell.frame.x_max, cell.frame.x_min)?,
                        native_sub(cell.frame.y_max, cell.frame.y_min)?,
                    ]
                    .map(|value| value as f32),
                    rectangles: cell
                        .bands
                        .as_ref()
                        .unwrap()
                        .rectangles
                        .iter()
                        .map(|r| [r.x_min, r.y_min, r.x_max, r.y_max].map(|v| v as f32))
                        .collect(),
                });
                cell.metrics.measured_height = f64::from(run.measured_heights[slot]);
                Ok(())
            })
            .unwrap();
            let label = format!("{}, run {run_index}", case.name);
            assert_eq!(observed.len(), run.calls.len(), "{label}");
            for (actual, expected) in observed.into_iter().zip(run.calls) {
                call_count += 1;
                assert_eq!(actual.slot, expected.slot, "{label}");
                assert_eq!(
                    actual.dimensions.map(f32::to_bits),
                    expected.dimensions.map(f32::to_bits),
                    "{label}, slot {}",
                    actual.slot
                );
                assert_eq!(
                    actual
                        .rectangles
                        .iter()
                        .map(|r| r.map(f32::to_bits))
                        .collect::<Vec<_>>(),
                    expected
                        .rectangles
                        .iter()
                        .map(|r| r.map(f32::to_bits))
                        .collect::<Vec<_>>(),
                    "{label}, slot {}",
                    actual.slot
                );
            }
            snapshots += run.frames.len();
            assert_eq!(
                frame_bits(&plan),
                run.frames
                    .iter()
                    .map(|r| r.map(f32::to_bits))
                    .collect::<Vec<_>>(),
                "{label}"
            );
            assert_eq!(
                plan.pending_gaps
                    .iter()
                    .map(|v| (*v as f32).to_bits())
                    .collect::<Vec<_>>(),
                run.pending_gaps
                    .iter()
                    .map(|v| v.to_bits())
                    .collect::<Vec<_>>(),
                "{label}, pending gaps"
            );
            assert_eq!(
                plan.rows
                    .iter()
                    .flat_map(|row| &row.cells)
                    .map(|cell| cell.bands.as_ref().map(|bands| bands
                        .rectangles
                        .iter()
                        .copied()
                        .map(bits)
                        .collect::<Vec<_>>()))
                    .collect::<Vec<_>>(),
                run.cell_splits
                    .iter()
                    .map(|bands| bands.as_ref().map(|bands| bands
                        .iter()
                        .map(|r| r.map(f32::to_bits))
                        .collect::<Vec<_>>()))
                    .collect::<Vec<_>>(),
                "{label}, split caches"
            );
        }
    }
    assert_eq!((run_count, call_count, snapshots), (147, 608, 1260));
}
