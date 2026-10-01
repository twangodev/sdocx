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
    layout_row_address: String,
    first_line_address: String,
    capture_boundary: String,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Metrics {
    has_text_layout: bool,
    has_text: bool,
    first_line_height: f32,
    top_margin: f32,
    measured_height: f32,
}

#[derive(Deserialize)]
struct Update {
    row: usize,
    rectangles: Vec<[f32; 4]>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Kind {
    FirstLine,
    LayoutRow,
    WarmRow,
}

#[derive(Deserialize)]
struct Action {
    kind: Kind,
    row: usize,
    rectangles: Vec<[f32; 4]>,
    changed: Option<bool>,
    relayout_cells: Vec<usize>,
    frames: Vec<[f32; 4]>,
    pending_gaps: Vec<f32>,
    cell_splits: Vec<Option<Vec<[f32; 4]>>>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    heights: Vec<f32>,
    widths: Vec<f32>,
    spans: Vec<[u32; 2]>,
    metrics: Vec<Metrics>,
    last_line_bottoms: Vec<f32>,
    minima: Vec<f32>,
    cache_updates: Vec<Update>,
    pending_gaps: Vec<f32>,
    initial_frames: Vec<[f32; 4]>,
    initial_splits: Vec<Option<Vec<[f32; 4]>>>,
    actions: Vec<Action>,
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

fn assert_state(
    plan: &PreparedTable,
    frames: &[[f32; 4]],
    gaps: &[f32],
    splits: &[Option<Vec<[f32; 4]>>],
    label: &str,
) {
    assert_eq!(
        plan.rows
            .iter()
            .flat_map(|row| &row.cells)
            .map(|cell| rect_bits(cell.frame))
            .collect::<Vec<_>>(),
        frames
            .iter()
            .map(|frame| frame.map(f32::to_bits))
            .collect::<Vec<_>>(),
        "{label}, frames"
    );
    assert_eq!(
        plan.pending_gaps
            .iter()
            .map(|gap| (*gap as f32).to_bits())
            .collect::<Vec<_>>(),
        gaps.iter().map(|gap| gap.to_bits()).collect::<Vec<_>>(),
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
                .map(rect_bits)
                .collect::<Vec<_>>()))
            .collect::<Vec<_>>(),
        splits
            .iter()
            .map(|bands| bands.as_ref().map(|bands| bands
                .iter()
                .map(|rectangle| rectangle.map(f32::to_bits))
                .collect::<Vec<_>>()))
            .collect::<Vec<_>>(),
        "{label}, split caches"
    );
}

impl Case {
    fn plan(&self) -> (RichTextTable, PreparedTable) {
        let mut source = grid(&self.heights, &self.widths);
        for (slot, cell) in source
            .rows
            .iter_mut()
            .flat_map(|row| &mut row.cells)
            .enumerate()
        {
            [cell.row_span, cell.column_span] = self.spans[slot];
            cell.content.text = if self.metrics[slot].has_text {
                "x".into()
            } else {
                String::new()
            };
            cell.content.margins = Some([0.0, self.metrics[slot].top_margin, 0.0, 0.0]);
        }
        for (row, minimum) in source.rows.iter_mut().zip(&self.minima) {
            row.min_height = Some(*minimum);
        }
        let mut rows = initialize_rows(&source, 0.5).unwrap();
        for (slot, cell) in rows.iter_mut().flat_map(|row| &mut row.cells).enumerate() {
            let metrics = &self.metrics[slot];
            cell.metrics.first_line_height = metrics
                .has_text_layout
                .then_some(f64::from(metrics.first_line_height));
            cell.metrics.measured_height = f64::from(metrics.measured_height);
            cell.metrics.last_line_bottom = if metrics.has_text_layout {
                f64::from(self.last_line_bottoms[slot])
            } else {
                0.0
            };
        }
        let mut plan = PreparedTable {
            topology: TableGrid::new(&source).unwrap(),
            rows,
            measured_bbox: rect([0.0; 4]),
            pending_gaps: self.pending_gaps.iter().copied().map(f64::from).collect(),
            min_first_page_height: 0.0,
            constraint: ObjectSpanLayoutConstraint::OverPages,
            bands: BandList::default(),
            half_border: 0.5,
        };
        for update in &self.cache_updates {
            plan.bands =
                BandList::new(update.rectangles.iter().copied().map(rect).collect()).unwrap();
            update_split(&mut plan, update.row).unwrap();
        }
        (source, plan)
    }
}

#[test]
fn native_warm_row_control_matches_movement_flags_and_selected_cells() {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/table-warm-control.json"
    ));
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "b78403ccecffe594cecf18eea48ba60c4c56808905049230589a9c9ab42c30ab"
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
    assert_eq!(capture.layout_row_address, "0xb0420");
    assert_eq!(capture.first_line_address, "0xb2360");
    assert_eq!(
        capture.capture_boundary,
        "native warm-row decisions with fixed supplied two-line caches; layoutCell recorded without shaping; text initialization, single-thread mutex operations and diagnostic interfaces isolated"
    );
    assert_eq!(capture.cases.len(), 148);
    let fonts = crate::fonts::FontBook::default();
    let renderer = TextRenderer::new(super::super::super::text::TextSettings::default(), &fonts);
    let mut slots = 0;
    let mut actions = 0;
    let mut snapshots = 0;
    let mut calls = 0;
    let mut changed_without_layout = 0;
    for case in capture.cases {
        let (source, mut plan) = case.plan();
        let columns = case.widths.len();
        slots += case.spans.len();
        assert_state(
            &plan,
            &case.initial_frames,
            &case.pending_gaps,
            &case.initial_splits,
            &case.name,
        );
        for (index, action) in case.actions.iter().enumerate() {
            let label = format!("{}, action {index}", case.name);
            plan.bands =
                BandList::new(action.rectangles.iter().copied().map(rect).collect()).unwrap();
            let mut relayout = Vec::new();
            let changed = match action.kind {
                Kind::FirstLine => {
                    adjust_first_line(&mut plan, action.row, &source, &renderer).unwrap();
                    None
                }
                Kind::LayoutRow | Kind::WarmRow => {
                    let changed = layout_row_using(
                        &mut plan,
                        action.row,
                        &source,
                        &renderer,
                        |_, position| {
                            relayout.push(position.row * columns + position.column);
                            Ok(())
                        },
                    )
                    .unwrap();
                    if matches!(action.kind, Kind::WarmRow) {
                        update_positions(&mut plan, action.row, &source).unwrap();
                        compress_row(&mut plan, action.row).unwrap();
                    }
                    Some(changed)
                }
            };
            assert_eq!(changed, action.changed, "{label}, changed flag");
            assert_eq!(relayout, action.relayout_cells, "{label}, selected cells");
            assert_state(
                &plan,
                &action.frames,
                &action.pending_gaps,
                &action.cell_splits,
                &label,
            );
            changed_without_layout += usize::from(changed == Some(true) && relayout.is_empty());
            calls += relayout.len();
            actions += 1;
            snapshots += case.spans.len();
        }
    }
    assert_eq!(slots, 1207);
    assert_eq!(actions, 1089);
    assert_eq!(snapshots, 10892);
    assert_eq!(calls, 418);
    assert_eq!(changed_without_layout, 124);
}
