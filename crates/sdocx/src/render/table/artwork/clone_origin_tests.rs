use super::*;
use crate::render::table::{TableBorderGeometry, TableGrid, tests::grid};
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
struct Capture {
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    spannable: bool,
    merged: bool,
    drawn_run: [f32; 4],
    local_raw_rect: [f32; 4],
    stages: Vec<Stage>,
    drawing: Drawing,
}

#[derive(Deserialize)]
struct Stage {
    stage: String,
    model_rect: [f32; 4],
    virtual_drawn_rect: [f32; 4],
    content_rect: [f32; 4],
    cells: Vec<Cell>,
}

#[derive(Deserialize)]
struct Cell {
    position: [usize; 2],
    saved_rect: [f32; 4],
    content_model_rect: [f32; 4],
    drawing_frame: Option<[f32; 4]>,
}

#[derive(Deserialize)]
struct Drawing {
    source_is_clone: bool,
    drawing_x: f32,
    commands: Vec<Command>,
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Command {
    Rect {
        position: [usize; 2],
        bounds_bits: [u32; 4],
        radii: [f32; 2],
        color: u32,
    },
    Line {
        position: [usize; 2],
        endpoint_bits: [u32; 4],
        color: u32,
        width_bits: u32,
    },
}

#[test]
fn actual_native_clone_uses_virtual_drawn_origin_and_retains_model_cells() {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/table-clone-origin.json"
    ));
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "78022f27f297e5b9accf5f58000f0da6c60b4e09fc809a2ffc5cf5cd885b8cc9"
    );
    let capture: Capture = serde_json::from_slice(bytes).unwrap();
    assert_eq!(capture.cases.len(), 11);
    for case in capture.cases {
        let source = &case.stages[0];
        let placed = case.stages.iter().find(|s| s.stage == "placed").unwrap();
        let cold = case
            .stages
            .iter()
            .find(|s| s.stage == "cold_frames")
            .unwrap();
        assert!(case.drawing.source_is_clone);
        let mut table = grid(&[100.0; 2], &[80.0; 2]);
        table.bbox = rect(source.model_rect);
        table.style.content_bbox = Some(rect(source.content_rect));
        for cell in &source.cells {
            let [row, column] = cell.position;
            table.rows[row].cells[column].bbox = rect(cell.saved_rect);
            table.rows[row].cells[column].content.bbox = rect(cell.content_model_rect);
        }
        if case.merged {
            table.rows[0].cells[0].row_span = 2;
            table.rows[0].cells[0].column_span = 2;
        }
        for stage in &case.stages {
            assert_eq!(stage.content_rect, source.content_rect, "{}", case.name);
            for (cell, saved) in stage.cells.iter().zip(&source.cells) {
                assert_eq!(cell.saved_rect, saved.saved_rect, "{}", case.name);
                assert_eq!(
                    cell.content_model_rect, saved.content_model_rect,
                    "{}",
                    case.name
                );
            }
            assert_eq!(
                native_rect(super::super::drawn_bounds_for_rect(
                    &table,
                    rect(stage.model_rect)
                ))
                .map(f32::to_bits),
                stage.virtual_drawn_rect.map(f32::to_bits),
                "{} {} virtual drawn rect",
                case.name,
                stage.stage
            );
        }
        let local_raw = crate::render::embedded::cloned_raw_bounds(
            table.bbox,
            rect(source.virtual_drawn_rect),
            rect(case.drawn_run),
        )
        .unwrap();
        assert_eq!(
            native_rect(local_raw).map(f32::to_bits),
            case.local_raw_rect.map(f32::to_bits),
            "{} native Widget affine",
            case.name
        );
        let constraint = if case.spannable {
            ObjectSpanLayoutConstraint::OverPages
        } else {
            ObjectSpanLayoutConstraint::Normal
        };
        let artwork = TableArtworkGeometry::native_drawing(
            constraint,
            rect(placed.virtual_drawn_rect),
            case.drawing.drawing_x,
        )
        .unwrap();
        let topology = TableGrid::new(&table).unwrap();
        let borders = TableBorderGeometry::new(&table, &topology).unwrap();
        let mut commands = Vec::new();
        for &position in topology.visible_cells() {
            let location = [position.row, position.column];
            let cell = cold.cells.iter().find(|c| c.position == location).unwrap();
            let cached = offset_frame(
                rect(cell.drawing_frame.unwrap()),
                [f64::from(case.drawing.drawing_x), 0.0],
            )
            .unwrap();
            let frame = artwork.background(rect(cell.saved_rect), cached);
            commands.push(Command::Rect {
                position: location,
                bounds_bits: native_rect(frame).map(f32::to_bits),
                radii: [0.0; 2],
                color: 0,
            });
            for path in borders.cell_paths(position) {
                if !path.selected(position, [2, 2], false, false) {
                    continue;
                }
                let path = artwork.border(path, frame);
                commands.push(Command::Line {
                    position: location,
                    endpoint_bits: path.endpoints.map(f32::to_bits),
                    color: path.style.color,
                    width_bits: path.paint_width().unwrap().to_bits(),
                });
            }
        }
        assert_eq!(commands, case.drawing.commands, "{} artwork", case.name);
    }
}
