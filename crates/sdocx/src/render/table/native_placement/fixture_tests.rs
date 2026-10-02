use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::*;

#[derive(Deserialize)]
struct SetterCapture {
    cases: Vec<SetterCase>,
}

#[derive(Deserialize)]
struct SetterCase {
    name: String,
    method: String,
    cell_bbox: [f32; 4],
    content_bbox: [f32; 4],
    requested_bbox: [f32; 4],
    cell_bbox_after: [f32; 4],
    content_bbox_after: [f32; 4],
}

#[test]
fn cell_model_state_matches_all_native_setter_geometry_controls() {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/table-cell-model-bounds.json"
    ));
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "898739ecada7c4a2b66f71a4893e712fbd484eb715d96dfdd51cbc882a2210fd"
    );
    let capture: SetterCapture = serde_json::from_slice(bytes).unwrap();
    assert_eq!(capture.cases.len(), 60);
    let mut data_only = 0;
    for case in capture.cases {
        data_only += usize::from(case.method == "data-only");
        let state = NativeCellModelState::new(case.cell_bbox, case.content_bbox).unwrap();
        let updated = state.with_saved_rect(case.requested_bbox).unwrap();
        assert_eq!(
            updated.saved_rect().map(f32::to_bits),
            case.cell_bbox_after.map(f32::to_bits),
            "{} saved rectangle",
            case.name
        );
        assert_eq!(
            updated.content_rect().map(f32::to_bits),
            case.content_bbox_after.map(f32::to_bits),
            "{} content rectangle",
            case.name
        );
    }
    assert_eq!(data_only, 15);
}

#[test]
fn feedback_rejects_nonfinite_and_staged_overflow_without_changing_source() {
    let original = NativeCellModelState::new([0.0, 0.0, 10.0, 10.0], [1.0, 2.0, 3.0, 4.0]).unwrap();
    assert!(NativeCellModelState::new([f32::NAN; 4], [0.0; 4]).is_err());
    assert!(
        original
            .apply_feedback([f32::INFINITY, 0.0], [0.0; 4])
            .is_err()
    );
    assert!(original.apply_feedback([0.0; 2], [f32::NAN; 4]).is_err());
    assert!(
        original
            .apply_feedback([f32::MAX; 2], [f32::MAX; 4])
            .is_err()
    );
    assert_eq!(original.saved_rect(), [0.0, 0.0, 10.0, 10.0]);
    assert_eq!(original.content_rect(), [1.0, 2.0, 3.0, 4.0]);
}

fn observed_bits(value: &serde_json::Value) -> [u32; 4] {
    serde_json::from_value(value.clone()).unwrap()
}

fn assert_rect(actual: [f32; 4], expected: &serde_json::Value, label: &str) {
    assert_eq!(actual.map(f32::to_bits), observed_bits(expected), "{label}");
}

#[test]
fn actual_document_feedback_preserves_native_cell_source_and_writer_stages() {
    use crate::render::embedded::PreparedObject;
    use crate::render::table::{drawn_bounds_for_rect, table_drawn_bounds};

    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/table-bodytext-placement.json"
    ));
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "7a66e259030edb4e019258fc49f07a717d6386bba6db2d11945f1307c0fa594b"
    );
    let capture: serde_json::Value = serde_json::from_slice(bytes).unwrap();
    let cases = capture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 3);
    for case in cases {
        let (table, layout) = crate::render::text::native_object_capture_profile(case);
        let original = table.clone();
        let entry = layout.native_object_entry.unwrap();
        let Some(Ok(PreparedObject::Table(prepared))) = &layout.lines[0].line.objects[0].prepared
        else {
            panic!("expected actual prepared table");
        };
        let frames: Vec<_> = prepared
            .rows
            .iter()
            .flat_map(|row| &row.cells)
            .map(|cell| native_coordinates(cell.frame))
            .collect();
        let mut cells: Vec<_> = table
            .rows
            .iter()
            .flat_map(|row| &row.cells)
            .map(|cell| {
                NativeCellModelState::new(
                    native_coordinates(cell.bbox),
                    native_coordinates(cell.content.bbox),
                )
                .unwrap()
            })
            .collect();
        let mut model = NativeTableModelState::new(
            native_coordinates(table.bbox),
            native_coordinates(table.style.content_bbox.unwrap()),
        )
        .unwrap();
        let writer = &case["writer"];
        assert_eq!(
            model.content_rect(),
            native_coordinates(table.style.content_bbox.unwrap())
        );
        assert_rect(
            model.raw_rect(),
            &writer["table_raw_before_bits"],
            "initial table raw",
        );
        let source_stages = [
            "source_before",
            "source_after",
            "source_after_page_origin_placement",
        ];
        let writer_stages = [
            "writer_before",
            "writer_after",
            "writer_after_page_origin_placement",
        ];
        for stage in 0..3 {
            let mut placement = None;
            if stage > 0 {
                let origin_field = if stage == 1 {
                    "supplied_page_origin_bits"
                } else {
                    "supplied_moved_page_origin_bits"
                };
                let origin: [u32; 2] =
                    serde_json::from_value(writer[origin_field].clone()).unwrap();
                let current_drawn = drawn_bounds_for_rect(&table, native_bounds(model.raw_rect()));
                let feedback = NativeDocumentTablePlacement::from_entry(
                    entry,
                    origin.map(f32::from_bits),
                    native_bounds(model.raw_rect()),
                    current_drawn,
                )
                .unwrap();
                let parent_index = if stage == 1 { 0 } else { 2 };
                assert_rect(
                    native_coordinates(feedback.parent_rect()),
                    &writer["native_parent_rects_bits"][parent_index],
                    "native parent affine",
                );
                cells = cells
                    .iter()
                    .zip(&frames)
                    .map(|(&source, &frame)| feedback.cell_model(source, frame).unwrap())
                    .collect();
                model = model
                    .apply_document_placement(feedback)
                    .set_content_size(
                        [
                            prepared.content_bbox.x_max as f32 - prepared.content_bbox.x_min as f32,
                            prepared.content_bbox.y_max as f32 - prepared.content_bbox.y_min as f32,
                        ],
                        NativeTableContentFit {
                            mode: 3,
                            maximum_height: 0.0,
                            maximum_width: 0.0,
                            maximum_height_enabled: true,
                        },
                    )
                    .unwrap();
                if stage == 1 {
                    assert_rect(
                        model.raw_rect(),
                        &writer["table_raw_before_move_bits"],
                        "table content fit",
                    );
                    assert_rect(
                        native_coordinates(drawn_bounds_for_rect(
                            &table,
                            native_bounds(model.raw_rect()),
                        )),
                        &writer["table_drawn_before_move_bits"],
                        "fitted table drawn",
                    );
                }
                placement = Some(feedback);
            }
            let drawn = if stage == 0 {
                table_drawn_bounds(&table)
            } else {
                drawn_bounds_for_rect(&table, native_bounds(model.raw_rect()))
            };
            let window = NativeTableTextWriterWindow::from_layout(
                native_coordinates(drawn),
                native_coordinates(prepared.measured_bbox),
            )
            .unwrap();
            let observed_writer = &writer[writer_stages[stage]];
            assert_rect(
                window.rounded_measured_world(),
                &observed_writer["rounded_measured_world_bits"],
                "writer rounded world",
            );
            for (slot, (&cell, &frame)) in cells.iter().zip(&frames).enumerate() {
                assert_rect(
                    cell.saved_rect(),
                    &writer[source_stages[stage]][slot]["saved_cell_rect_bits"],
                    "saved source cell",
                );
                assert_rect(
                    cell.content_rect(),
                    &writer[source_stages[stage]][slot]["content_model_rect_bits"],
                    "content Model source",
                );
                assert_rect(
                    frame,
                    &writer[source_stages[stage]][slot]["drawing_frame_bits"],
                    "physical frame",
                );
                let caller = window.cell_origin(frame).unwrap();
                let context = if let Some(feedback) = placement {
                    feedback.cell_text(cell, frame, window, [0.0; 2]).unwrap()
                } else {
                    cell.saved_text(caller, [0.0; 2]).unwrap()
                };
                assert_eq!(
                    context.provenance(),
                    if stage == 0 {
                        NativeCellTextClipProvenance::SavedSource
                    } else {
                        NativeCellTextClipProvenance::ActualDocumentPlacement
                    }
                );
                let transport = context.transport().unwrap();
                assert_eq!(transport.output_translation(), [0.0; 2]);
                let _native_context = transport.native_context();
                for run in observed_writer["cells"][slot]["runs"].as_array().unwrap() {
                    let expected: [u32; 2] =
                        serde_json::from_value(run["caller_origin_bits"].clone()).unwrap();
                    assert_eq!(caller.0.map(f32::to_bits), expected, "writer caller origin");
                    assert_rect(
                        cell.content_rect(),
                        &run["actual_source_rect_bits"],
                        "writer source content",
                    );
                }
            }
        }
        assert_eq!(
            table, original,
            "document feedback must preserve parsed source"
        );
        assert_eq!(
            NativeCellTextPlacement::Unknown.provenance(),
            NativeCellTextClipProvenance::Unknown
        );
        assert!(NativeCellTextPlacement::Unknown.transport().is_none());
    }
}
