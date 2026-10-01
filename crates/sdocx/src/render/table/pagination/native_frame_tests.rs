use super::super::{drawable_half_border, initialize_rows, tests::grid};
use super::*;
use crate::{ObjectSpanLayoutConstraint, TableBorder, TableEdgeStyle, TableRecordMetadata};
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
struct Capture {
    apk_version: String,
    apk_sha256: String,
    model_library_sha256: String,
    drawing_library_sha256: String,
    base_library_sha256: String,
    initializer_address: String,
    text_adapter: String,
    allocation_fills: Vec<u8>,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    origin: [f32; 2],
    heights: Vec<f32>,
    widths: Vec<f32>,
    spans: Vec<[u32; 2]>,
    native_defaults: bool,
    outer_border: Option<[Edge; 4]>,
    initial_frames: Vec<[f32; 4]>,
    pending_gaps: Vec<f32>,
    changes: Vec<Change>,
    frames: Vec<[f32; 4]>,
    cached_frames: Vec<[f32; 4]>,
    final_pending_gaps: Vec<f32>,
}

#[derive(Deserialize)]
struct Edge {
    color: u32,
    width: f32,
    start_radius: f32,
    end_radius: f32,
}

impl From<Edge> for TableEdgeStyle {
    fn from(edge: Edge) -> Self {
        Self {
            color: edge.color,
            width: edge.width,
            start_radius: edge.start_radius,
            end_radius: edge.end_radius,
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Change {
    OffsetRows { row: usize, amount: f32 },
    ExtendRow { row: usize, amount: f32 },
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
fn native_initialization_and_row_frame_updates_match() {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/table-cold-frames.json"
    ));
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "498bfb15e663cfeb648ed51a8c0ed2a2490b09fa47b578d1419e0d7eac0cdb84"
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
    assert_eq!(capture.initializer_address, "0xaa6b4");
    assert_eq!(capture.allocation_fills, [0, 165, 255]);
    assert_eq!(
        capture.text_adapter,
        "zeroed text-layout storage; SetObject and SetTextScale omitted; native shaping not executed"
    );
    assert_eq!(capture.cases.len(), 93);
    let mut cells = 0;
    for (index, case) in capture.cases.into_iter().enumerate() {
        let context = format!("{}, input {index}", case.name);
        let mut source = grid(&case.heights, &case.widths);
        source.bbox.x_min = f64::from(case.origin[0]);
        source.bbox.y_min = f64::from(case.origin[1]);
        if !case.native_defaults {
            source.style.border = case.outer_border.map(|edges| {
                let [left, top, right, bottom] = edges.map(Into::into);
                TableBorder {
                    left,
                    top,
                    right,
                    bottom,
                    metadata: TableRecordMetadata::default(),
                }
            });
        }
        for (cell, span) in source
            .rows
            .iter_mut()
            .flat_map(|row| &mut row.cells)
            .zip(case.spans)
        {
            [cell.row_span, cell.column_span] = span;
        }
        let half_border = drawable_half_border(&source);
        let rows = initialize_rows(&source, half_border).unwrap();
        assert_eq!(
            frame_bits(&rows),
            case.initial_frames
                .iter()
                .map(|frame| frame.map(f32::to_bits))
                .collect::<Vec<_>>(),
            "{context}, initialization"
        );
        let mut plan = PreparedTable {
            content_bbox: BoundingBox::default(),
            topology: super::super::TableGrid::new(&source).unwrap(),
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
            half_border,
        };
        for change in case.changes {
            match change {
                Change::OffsetRows { row, amount } => {
                    offset_from_row(&mut plan, row, f64::from(amount))
                }
                Change::ExtendRow { row, amount } => extend_row(&mut plan, row, f64::from(amount)),
            }
            .unwrap();
        }
        let actual = frame_bits(&plan.rows);
        for expected in [case.frames, case.cached_frames] {
            assert_eq!(
                actual,
                expected
                    .iter()
                    .map(|frame| frame.map(f32::to_bits))
                    .collect::<Vec<_>>(),
                "{context}, updated frames"
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
            "{context}, pending gaps"
        );
        cells += actual.len();
    }
    assert_eq!(cells, 1082);
}
