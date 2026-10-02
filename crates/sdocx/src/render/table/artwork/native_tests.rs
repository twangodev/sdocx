use super::*;
use crate::render::table::{TableBorderGeometry, TableGrid, tests::grid};
use crate::{TableBorder, TableEdgeStyle, TableRecordMetadata};
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
struct Capture {
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    constraint: u32,
    source_table_rect: [f32; 4],
    saved_cell_rects: Vec<[f32; 4]>,
    drawing_x: f32,
    display_rect: [f32; 4],
    prepared_frames: Vec<[f32; 4]>,
    pending_gaps: Vec<f32>,
    spans: Vec<[u32; 2]>,
    heights: Vec<f32>,
    widths: Vec<f32>,
    borders: Vec<[CapturedStyle; 4]>,
    outline_color: u32,
    outline: [f32; 3],
    canvas_scale: f32,
    selected_paths: Vec<CapturedPath>,
    commands: Vec<Command>,
}

#[derive(Clone, Copy, Deserialize)]
struct CapturedStyle {
    color: u32,
    width: f32,
    start_radius: f32,
    end_radius: f32,
}

impl From<CapturedStyle> for TableEdgeStyle {
    fn from(style: CapturedStyle) -> Self {
        Self {
            color: style.color,
            width: style.width,
            start_radius: style.start_radius,
            end_radius: style.end_radius,
        }
    }
}

#[derive(Deserialize)]
struct CapturedPath {
    position: [usize; 2],
    edge: u32,
    color: u32,
    width: f32,
    endpoint_bits: [u32; 4],
}

#[derive(Debug, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Command {
    Line {
        position: [usize; 2],
        endpoint_bits: [u32; 4],
        color: u32,
        width_bits: u32,
    },
    Rect {
        position: [usize; 2],
        bounds_bits: [u32; 4],
        radii: [f32; 2],
        color: u32,
    },
    RoundRect {
        position: [usize; 2],
        bounds_bits: [u32; 4],
        radii: [f32; 2],
        color: u32,
    },
}

fn rect_bits(bounds: BoundingBox) -> [u32; 4] {
    native_rect(bounds).map(f32::to_bits)
}

#[test]
fn complete_native_cell_artwork_matches_all_frozen_drawing_cases() {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/table-cell-drawing.json"
    ));
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "cee7c9dd9d06c5ce623c474183e058ada95af6de0d5a2c2834ea32cdc73a0b3d"
    );
    let capture: Capture = serde_json::from_slice(bytes).unwrap();
    assert_eq!(capture.cases.len(), 60);
    let mut total_paths = 0;
    let mut total_commands = 0;
    for case in capture.cases {
        let mut table = grid(&case.heights, &case.widths);
        table.bbox = rect(case.source_table_rect);
        table.style.content_bbox = Some(table.bbox);
        for (index, cell) in table
            .rows
            .iter_mut()
            .flat_map(|row| &mut row.cells)
            .enumerate()
        {
            cell.bbox = rect(case.saved_cell_rects[index]);
            [cell.row_span, cell.column_span] = case.spans[index];
            let [left, top, right, bottom] = case.borders[index].map(Into::into);
            cell.border = Some(TableBorder {
                left,
                top,
                right,
                bottom,
                metadata: TableRecordMetadata::default(),
            });
        }
        let grid = TableGrid::new(&table).unwrap();
        let borders = TableBorderGeometry::new(&table, &grid).unwrap();
        let mode = match case.constraint {
            0 => ObjectSpanLayoutConstraint::Normal,
            1 => ObjectSpanLayoutConstraint::OverPages,
            2 => ObjectSpanLayoutConstraint::OverPagesOverlapPadding,
            _ => panic!("uncaptured constraint"),
        };
        let artwork =
            TableArtworkGeometry::drawing(mode, table.bbox, [f64::from(case.drawing_x), 0.0])
                .unwrap();
        let mut commands = Vec::new();
        let mut paths = Vec::new();
        for &position in grid.visible_cells() {
            let index = position.row * case.widths.len() + position.column;
            let cached = offset_frame(
                rect(case.prepared_frames[index]),
                [f64::from(case.drawing_x), 0.0],
            )
            .unwrap();
            let frame =
                artwork.background(table.rows[position.row].cells[position.column].bbox, cached);
            let [left, top, right, bottom] = native_rect(frame);
            let [dl, dt, dr, db] = case.display_rect;
            if dl < dr && dt < db && !(left < dr && right > dl && top < db && bottom > dt) {
                continue;
            }
            let color = 0xff20_3040 + index as u32;
            let location = [position.row, position.column];
            let background = CellBackgroundGeometry::new(
                frame,
                position,
                case.spans[index],
                [case.heights.len(), case.widths.len()],
                [case.outline[1], case.outline[2]],
            );
            commands.push(match background.radii {
                Some(radii) => Command::RoundRect {
                    position: location,
                    bounds_bits: rect_bits(background.bounds),
                    radii,
                    color,
                },
                None => Command::Rect {
                    position: location,
                    bounds_bits: rect_bits(background.bounds),
                    radii: [0.0; 2],
                    color,
                },
            });
            for bounds in background.square_corners.into_iter().flatten() {
                commands.push(Command::Rect {
                    position: location,
                    bounds_bits: rect_bits(bounds),
                    radii: [0.0; 2],
                    color,
                });
            }
            for path in borders.cell_paths(position) {
                let Some(width) = path.paint_width() else {
                    continue;
                };
                let gap = case
                    .pending_gaps
                    .get(position.row + 1)
                    .is_some_and(|gap| gap.abs() > 0.001);
                if !path.selected(
                    position,
                    [case.heights.len(), case.widths.len()],
                    case.outline_color != 0 && case.outline[0] > 0.0,
                    gap,
                ) {
                    continue;
                }
                let drawn = artwork.border(path, frame);
                let selected = if artwork.cached() { drawn } else { path };
                paths.push((
                    location,
                    selected.edge as u32,
                    selected.style.color,
                    width,
                    selected.endpoints.map(f32::to_bits),
                ));
                let width = if width * case.canvas_scale < 1.0 {
                    1.0 / case.canvas_scale
                } else {
                    width
                };
                commands.push(Command::Line {
                    position: location,
                    endpoint_bits: drawn.endpoints.map(f32::to_bits),
                    color: drawn.style.color,
                    width_bits: width.to_bits(),
                });
            }
        }
        assert_eq!(
            paths.len(),
            case.selected_paths.len(),
            "{} paths",
            case.name
        );
        for (actual, expected) in paths.iter().zip(&case.selected_paths) {
            assert_eq!(
                *actual,
                (
                    expected.position,
                    expected.edge,
                    expected.color,
                    expected.width,
                    expected.endpoint_bits
                ),
                "{} selected path",
                case.name
            );
        }
        assert_eq!(commands, case.commands, "{} canvas commands", case.name);
        total_paths += paths.len();
        total_commands += commands.len();
    }
    assert_eq!(total_paths, 501);
    assert_eq!(total_commands, 738);
}
