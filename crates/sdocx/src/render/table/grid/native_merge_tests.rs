use super::super::{initialize_rows, tests::grid};
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
    merge_address: String,
    validity_address: String,
    capture_boundary: String,
    raw_cell_identity_preserved: bool,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    heights: Vec<f32>,
    widths: Vec<f32>,
    initial_spans: Vec<[u32; 2]>,
    actions: Vec<Action>,
    frames: Vec<[f32; 4]>,
}

#[derive(Deserialize)]
struct Action {
    range: [i32; 4],
    valid: bool,
    spans: Vec<[u32; 2]>,
    dirty_flags: Vec<[u8; 2]>,
    frame_owners: Vec<usize>,
    visible: Vec<usize>,
}

#[test]
fn native_merge_generated_states_preserve_raw_frames_and_paint_visibility() {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/table-merge-cells.json"
    ));
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "96eda7bc2741c68e0eb18469193217678aad7922e78c2b4acda4d1879214ac33"
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
    assert_eq!(capture.merge_address, "0x3d6038");
    assert_eq!(capture.validity_address, "0x3ca320");
    assert_eq!(
        capture.capture_boundary,
        "native merge mutation and ownership with no attached document or history; diagnostics and cold text initialization isolated"
    );
    assert!(capture.raw_cell_identity_preserved);
    assert_eq!(capture.cases.len(), 134);
    let mut actions = 0;
    let mut owners = 0;
    for case in capture.cases {
        let mut source = grid(&case.heights, &case.widths);
        let columns = case.widths.len();
        let count = case.heights.len() * columns;
        let mut previous = case.initial_spans;
        for action in case.actions {
            actions += 1;
            assert_eq!(action.spans.len(), count);
            assert_eq!(action.dirty_flags.len(), count);
            assert!(action.dirty_flags.iter().flatten().all(|flag| *flag <= 1));
            if !action.valid {
                assert_eq!(action.spans, previous, "{}, {:?}", case.name, action.range);
            }
            for (cell, span) in source
                .rows
                .iter_mut()
                .flat_map(|row| &mut row.cells)
                .zip(&action.spans)
            {
                [cell.row_span, cell.column_span] = *span;
            }
            let grid = TableGrid::new(&source).unwrap();
            let flatten = |position: CellPosition| position.row * columns + position.column;
            assert_eq!(
                grid.frame_owners
                    .iter()
                    .copied()
                    .map(flatten)
                    .collect::<Vec<_>>(),
                action.frame_owners,
                "{}, {:?}",
                case.name,
                action.range
            );
            assert_eq!(
                grid.visible_cells()
                    .iter()
                    .copied()
                    .map(flatten)
                    .collect::<Vec<_>>(),
                action.visible,
                "{}, {:?}",
                case.name,
                action.range
            );
            owners += count;
            previous = action.spans;
        }
        let frames = initialize_rows(&source, 0.5).unwrap();
        assert_eq!(
            frames
                .iter()
                .flat_map(|row| &row.cells)
                .map(|cell| {
                    let r = cell.frame;
                    [r.x_min, r.y_min, r.x_max, r.y_max].map(|v| (v as f32).to_bits())
                })
                .collect::<Vec<_>>(),
            case.frames
                .iter()
                .map(|r| r.map(f32::to_bits))
                .collect::<Vec<_>>(),
            "{}",
            case.name
        );
    }
    assert_eq!((actions, owners), (1046, 9240));
}
