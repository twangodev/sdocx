use super::super::{TableGrid, initialize_rows, tests::grid};
use super::*;
use crate::fonts::FontBook;
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
    measure_address: String,
    layout_address: String,
    measurement_inputs: String,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    cold_via_layout: bool,
    heights: Vec<f32>,
    widths: Vec<f32>,
    spans: Vec<[u32; 2]>,
    measured_heights: Vec<f32>,
    first_line_heights: Vec<f32>,
    last_line_bottoms: Vec<f32>,
    rectangles: Vec<[f32; 4]>,
    cold: State,
    warm: Vec<Warm>,
}

#[derive(Deserialize)]
struct Warm {
    rectangles: Vec<[f32; 4]>,
    minimum_height_bits: u32,
    state: State,
}

#[derive(Deserialize)]
struct State {
    selected_cells: Vec<usize>,
    frames: Vec<[f32; 4]>,
    pending_gaps: Vec<f32>,
    cell_splits: Vec<Option<Vec<[f32; 4]>>>,
    measured_bbox: [f32; 4],
    content_bbox: [f32; 4],
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
    [rect.x_min, rect.y_min, rect.x_max, rect.y_max].map(|v| (v as f32).to_bits())
}

fn assert_state(plan: &PreparedTable, selected: &[usize], state: &State, label: &str) {
    assert_eq!(selected, state.selected_cells, "{label}, cell selection");
    assert_eq!(
        plan.rows
            .iter()
            .flat_map(|r| &r.cells)
            .map(|c| bits(c.frame))
            .collect::<Vec<_>>(),
        state
            .frames
            .iter()
            .map(|r| r.map(f32::to_bits))
            .collect::<Vec<_>>(),
        "{label}, frames"
    );
    assert_eq!(
        plan.pending_gaps
            .iter()
            .map(|v| (*v as f32).to_bits())
            .collect::<Vec<_>>(),
        state
            .pending_gaps
            .iter()
            .map(|v| v.to_bits())
            .collect::<Vec<_>>(),
        "{label}, pending gaps"
    );
    assert_eq!(
        plan.rows
            .iter()
            .flat_map(|r| &r.cells)
            .map(|c| c.bands.as_ref().map(|b| b
                .rectangles
                .iter()
                .copied()
                .map(bits)
                .collect::<Vec<_>>()))
            .collect::<Vec<_>>(),
        state
            .cell_splits
            .iter()
            .map(|b| b
                .as_ref()
                .map(|r| r.iter().map(|r| r.map(f32::to_bits)).collect::<Vec<_>>()))
            .collect::<Vec<_>>(),
        "{label}, split caches"
    );
    assert_eq!(
        bits(plan.measured_bbox),
        state.measured_bbox.map(f32::to_bits),
        "{label}, measured bounds"
    );
    assert_eq!(
        bits(plan.content_bounds().unwrap()),
        state.content_bbox.map(f32::to_bits),
        "{label}, content bounds"
    );
    assert_eq!(
        bits(plan.content_bbox),
        state.content_bbox.map(f32::to_bits),
        "{label}, retained content bounds"
    );
}

#[test]
fn native_public_table_lifecycle_matches_composed_rust_phases() {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/table-lifecycle.json"
    ));
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "a71265b72c29a2acab94934d865c8a23587befa593051075d4b53546e6eceeb5"
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
    assert_eq!(capture.measure_address, "0xaa530");
    assert_eq!(capture.layout_address, "0xaa3d4");
    assert_eq!(
        capture.measurement_inputs,
        "fixed cached heights and line metrics; native public Measure/Layout drivers; text initialization/update/measurement, padding/font selection, diagnostics and final observers isolated"
    );
    assert_eq!(capture.cases.len(), 69);
    let fonts = FontBook::default();
    let renderer = TextRenderer::new(super::super::super::text::TextSettings::default(), &fonts);
    let mut slots = 0;
    let mut calls = 0;
    let mut snapshots = 0;
    let mut cold_layout_calls = 0;
    for case in capture.cases {
        cold_layout_calls += usize::from(case.cold_via_layout);
        let mut source = grid(&case.heights, &case.widths);
        for (cell, span) in source
            .rows
            .iter_mut()
            .flat_map(|r| &mut r.cells)
            .zip(&case.spans)
        {
            [cell.row_span, cell.column_span] = *span;
            cell.content.text = "x".into();
        }
        let mut plan = PreparedTable {
            callback_top: 0.0,
            content_bbox: BoundingBox::default(),
            topology: TableGrid::new(&source).unwrap(),
            rows: initialize_rows(&source, 0.5).unwrap(),
            measured_bbox: rect([0.0; 4]),
            pending_gaps: vec![0.0; source.rows.len()],
            min_first_page_height: 0.0,
            constraint: crate::ObjectSpanLayoutConstraint::OverPages,
            bands: BandList::new(case.rectangles.iter().copied().map(rect).collect()).unwrap(),
            half_border: 0.5,
        };
        let columns = case.widths.len();
        slots += case.spans.len();
        let mut selected = Vec::new();
        cold_from_row_using(&mut plan, 0, |plan, position| {
            let slot = position.row * columns + position.column;
            selected.push(slot);
            plan.rows[position.row].cells[position.column]
                .metrics
                .measured_height = f64::from(case.measured_heights[slot]);
            Ok(())
        })
        .unwrap();
        plan.update_geometry(&source, &renderer).unwrap();
        assert_state(
            &plan,
            &selected,
            &case.cold,
            &format!("{}, cold", case.name),
        );
        calls += selected.len();
        snapshots += case.spans.len();
        for (slot, cell) in plan.rows.iter_mut().flat_map(|r| &mut r.cells).enumerate() {
            cell.metrics.first_line_height = Some(f64::from(case.first_line_heights[slot]));
            cell.metrics.last_line_bottom = f64::from(case.last_line_bottoms[slot]);
        }
        for (index, warm) in case.warm.iter().enumerate() {
            selected.clear();
            plan.bands =
                BandList::new(warm.rectangles.iter().copied().map(rect).collect()).unwrap();
            warm_rows_using(&mut plan, &source, |plan, row| {
                layout_row_using(plan, row, &source, &renderer, |_, position| {
                    selected.push(position.row * columns + position.column);
                    Ok(())
                })
                .map(|_| ())
            })
            .unwrap();
            plan.update_geometry(&source, &renderer).unwrap();
            let label = format!("{}, warm {index}", case.name);
            assert_state(&plan, &selected, &warm.state, &label);
            assert_eq!(
                (plan.min_first_page_height as f32).to_bits(),
                warm.minimum_height_bits,
                "{label}, first-page minimum"
            );
            calls += selected.len();
            snapshots += case.spans.len();
        }
    }
    assert_eq!((slots, calls, snapshots), (555, 839, 1665));
    assert_eq!(cold_layout_calls, 32);
}
