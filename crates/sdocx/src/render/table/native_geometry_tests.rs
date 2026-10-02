use super::*;
use crate::{TableEdgeStyle, fonts::FontBook};
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
    update_geometry_address: String,
    minimum_height_address: String,
    measurement_inputs: String,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    height_limit: Option<HeightLimit>,
    heights: Vec<f32>,
    widths: Vec<f32>,
    spans: Vec<[u32; 2]>,
    native_defaults: bool,
    outer_border: Option<[CapturedEdge; 4]>,
    default_border: Option<[CapturedEdge; 4]>,
    borders: Vec<Option<[CapturedEdge; 4]>>,
    frames: Vec<[f32; 4]>,
    metrics: Vec<Metrics>,
    drawn_widths: [f32; 4],
    content_bbox: [f32; 4],
    measured_bbox: [f32; 4],
    minimum_height_bits: Vec<[u32; 2]>,
}

#[derive(Deserialize)]
pub(super) struct HeightLimit {
    enabled: bool,
    maximum: f32,
}

impl HeightLimit {
    pub fn apply(&self, table: &mut RichTextTable) {
        table.style.max_height_enabled = self.enabled;
        table.style.max_height = Some(self.maximum);
    }
}

#[derive(Clone, Copy, Deserialize)]
struct CapturedEdge {
    color: u32,
    width: f32,
    start_radius: f32,
    end_radius: f32,
}

impl From<CapturedEdge> for TableEdgeStyle {
    fn from(edge: CapturedEdge) -> Self {
        Self {
            color: edge.color,
            width: edge.width,
            start_radius: edge.start_radius,
            end_radius: edge.end_radius,
        }
    }
}

fn border(edges: [CapturedEdge; 4]) -> TableBorder {
    let [left, top, right, bottom] = edges.map(TableEdgeStyle::from);
    TableBorder {
        left,
        top,
        right,
        bottom,
        metadata: crate::TableRecordMetadata::default(),
    }
}

#[derive(Deserialize)]
struct Metrics {
    has_text_layout: bool,
    has_text: bool,
    first_line_height: f32,
    top_margin: f32,
    measured_height: f32,
}

fn capture() -> Capture {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/table-measured-geometry.json"
    ));
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "6b210cfa59fa00971addb39da7d7576eeb3238214fa560d31a9dcdf7f4039ed4"
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
    assert_eq!(
        capture.text_library_sha256,
        "5483711673a499743625eb3275e34b37a006919af346212b46b8d8857834308b"
    );
    assert_eq!(capture.update_geometry_address, "0xab168");
    assert_eq!(capture.minimum_height_address, "0xac9f0");
    assert_eq!(
        capture.measurement_inputs,
        "supplied cached frames and text metrics, not native text shaping"
    );
    assert_eq!(capture.cases.len(), 154);
    capture
}

impl Case {
    fn plan(&self) -> (RichTextTable, PreparedTable) {
        let mut source = tests::grid(&self.heights, &self.widths);
        if let Some(limit) = &self.height_limit {
            limit.apply(&mut source);
        }
        source.style.border = if self.native_defaults {
            None
        } else {
            self.outer_border.map(border)
        };
        source.style.default_cell_border = self.default_border.map(border);
        for (slot, cell) in source
            .rows
            .iter_mut()
            .flat_map(|row| &mut row.cells)
            .enumerate()
        {
            [cell.row_span, cell.column_span] = self.spans[slot];
            cell.border = self.borders[slot].map(border);
            cell.content.text = if self.metrics[slot].has_text {
                "x".to_owned()
            } else {
                String::new()
            };
            cell.content.margins = Some([0.0, self.metrics[slot].top_margin, 0.0, 0.0]);
        }
        let half_border = drawable_half_border(&source);
        let mut rows = initialize_rows(&source, half_border).unwrap();
        for (slot, cell) in rows.iter_mut().flat_map(|row| &mut row.cells).enumerate() {
            let [x_min, y_min, x_max, y_max] = self.frames[slot].map(f64::from);
            cell.frame = BoundingBox {
                x_min,
                y_min,
                x_max,
                y_max,
            };
            cell.metrics.first_line_height = self.metrics[slot]
                .has_text_layout
                .then_some(f64::from(self.metrics[slot].first_line_height));
            cell.metrics.measured_height = f64::from(self.metrics[slot].measured_height);
        }
        let plan = PreparedTable {
            callback_top: 0.0,
            content_bbox: BoundingBox::default(),
            measured_bbox: BoundingBox {
                x_min: 0.0,
                y_min: 0.0,
                x_max: 0.0,
                y_max: 0.0,
            },
            topology: TableGrid::new(&source).unwrap(),
            pending_gaps: vec![0.0; rows.len()],
            rows,
            min_first_page_height: 0.0,
            constraint: ObjectSpanLayoutConstraint::OverPages,
            bands: BandList::default(),
            half_border,
        };
        (source, plan)
    }
}

fn rect_bits(rect: BoundingBox) -> [u32; 4] {
    [rect.x_min, rect.y_min, rect.x_max, rect.y_max].map(|value| (value as f32).to_bits())
}

#[test]
fn native_measured_geometry_uses_endpoint_owner_frames() {
    let fonts = FontBook::default();
    let renderer = TextRenderer::new(super::super::text::TextSettings::default(), &fonts);
    let mut slots = 0;
    for case in capture().cases {
        let (source, mut plan) = case.plan();
        assert_eq!(
            table_border_widths(&source).map(|value| (value as f32).to_bits()),
            case.drawn_widths.map(f32::to_bits),
            "{}, border widths",
            case.name
        );
        plan.update_geometry(&source, &renderer).unwrap();
        assert_eq!(
            rect_bits(plan.measured_bbox),
            case.measured_bbox.map(f32::to_bits),
            "{}, measured bounds",
            case.name
        );
        assert_eq!(
            rect_bits(plan.content_bbox),
            case.content_bbox.map(f32::to_bits),
            "{}, content bounds",
            case.name
        );
        assert_eq!(
            (plan.min_first_page_height as f32).to_bits(),
            case.minimum_height_bits[0][1],
            "{}, first-page offset",
            case.name
        );
        slots += case.frames.len();
    }
    assert_eq!(slots, 1281);
}

#[test]
fn native_first_page_minima_use_owner_metrics_and_first_row_fallback() {
    let fonts = FontBook::default();
    let renderer = TextRenderer::new(super::super::text::TextSettings::default(), &fonts);
    let mut queries = 0;
    for case in capture().cases {
        let (source, plan) = case.plan();
        for (row, minimum) in case.minimum_height_bits.iter().enumerate() {
            assert_eq!(
                (plan.first_line_minimum(row, &source, &renderer).unwrap() as f32).to_bits(),
                minimum[0],
                "{}, row {row}",
                case.name
            );
            assert_eq!(
                (plan.first_page_minimum(row, &source, &renderer).unwrap() as f32).to_bits(),
                minimum[1],
                "{}, row {row} with origin",
                case.name,
            );
            queries += 2;
        }
    }
    assert_eq!(queries, 914);
}
