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

#[test]
fn table_rect_placement_reanchors_content_and_reports_native_dimension_rescaling() {
    let source =
        NativeTableModelState::new([0.0, 0.0, 20.0, 20.0], [2.0, 3.0, 12.0, 13.0]).unwrap();
    let update = source
        .set_rect(
            [100.0, 200.0, 140.0, 240.0],
            NativeTableContentFit::default(),
        )
        .unwrap();
    assert_eq!(update.model.raw_rect(), [100.0, 200.0, 110.0, 210.0]);
    assert_eq!(update.model.content_rect(), [100.0, 200.0, 110.0, 210.0]);
    assert_eq!(
        update.resize,
        Some(NativeTableContentResize {
            column_scale: 1.0,
            row_scale: 1.0,
        })
    );
    let small = NativeTableModelState::new([0.0, 0.0, 0.1, 0.1], [0.0, 0.0, 0.1, 0.1]).unwrap();
    let update = small
        .set_rect(
            [65536.0, 65536.0, 65537.0, 65537.0],
            NativeTableContentFit::default(),
        )
        .unwrap();
    assert_eq!(
        update.model.content_rect().map(f32::to_bits),
        [0x4780_0000, 0x4780_0000, 0x4780_000d, 0x4780_000d]
    );
    assert_eq!(update.model.raw_rect(), update.model.content_rect());
    assert_eq!(update.resize.unwrap().column_scale, 1.015625);
    assert_eq!(update.resize.unwrap().row_scale, 1.015625);
    let resize = update.resize.unwrap();
    assert!(!resize.preserves_column(0.05));
    assert!(!resize.preserves_row(0.05, f32::MAX));
    assert!(resize.preserves_column(0.0));
    assert!(resize.preserves_row(0.05, 0.05));
    assert!(!resize.preserves_row(f32::MAX, f32::MAX));
    assert_eq!(source.content_rect(), [2.0, 3.0, 12.0, 13.0]);
    assert_eq!(small.content_rect(), [0.0, 0.0, 0.1, 0.1]);
}

#[test]
#[cfg(feature = "serde")]
fn changed_stored_row_height_withdraws_cached_cell_clip_provenance() {
    use crate::RichTextObjectContent;
    use crate::fonts::FontBook;
    use crate::render::embedded::PreparedObject;
    use crate::render::table::{
        NativeTableDocumentSource, prepare_table_clone_drawing_with_native_entry,
        table_drawn_bounds,
    };
    use crate::render::text::{StyledText, TextContext, TextFrame, TextRenderer, layout_flow_text};

    let capture: serde_json::Value = serde_json::from_slice(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/table-bodytext-placement.json"
    )))
    .unwrap();
    let case = capture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == "overpages-zero-origin")
        .unwrap();
    let mut source = crate::render::text::native_object_capture_source(case);
    let Some(RichTextObjectContent::Table(table)) = &mut source.object_spans[0].content else {
        panic!("expected source table")
    };
    table.bbox.y_max = f64::from(0.1_f32);
    table.style.content_bbox = Some(table.bbox);
    for (index, row) in table.rows.iter_mut().enumerate() {
        row.height = 0.05;
        for cell in &mut row.cells {
            cell.bbox.y_min = f64::from(0.05_f32 * index as f32);
            cell.bbox.y_max = f64::from(0.05_f32 * (index + 1) as f32);
            cell.content.bbox = cell.bbox;
        }
    }
    let original = source.clone();
    let fonts = FontBook::default();
    let renderer = TextRenderer::new(Default::default(), &fonts);
    let theme = crate::render::RenderTheme::for_canvas(false);
    let layout = layout_flow_text(
        &StyledText::new(&source, TextContext::Flow, renderer.settings),
        TextFrame {
            bbox: source.bbox,
            gravity: Some(0),
            exclusions: &[],
        },
        theme,
        &renderer,
    );
    let entry = layout.native_object_entry.unwrap();
    let Some(RichTextObjectContent::Table(table)) = &source.object_spans[0].content else {
        unreachable!()
    };
    let placement = NativeDocumentTablePlacement::from_entry(
        entry,
        [0.0; 2],
        table.bbox,
        table_drawn_bounds(table),
    )
    .unwrap();
    let update = NativeTableModelState::new(
        native_coordinates(table.bbox),
        native_coordinates(table.style.content_bbox.unwrap()),
    )
    .unwrap()
    .set_rect(
        native_coordinates(placement.parent_rect()),
        NativeTableContentFit::default(),
    )
    .unwrap();
    assert!(table.rows.iter().any(|row| {
        !update
            .resize
            .unwrap()
            .preserves_row(row.height, row.max_height.unwrap_or(f32::MAX))
    }));
    let Some(Ok(PreparedObject::Table(prepared))) = &layout.lines[0].line.objects[0].prepared
    else {
        panic!("expected actual prepared table")
    };
    let drawing = prepare_table_clone_drawing_with_native_entry(
        table,
        source.object_spans[0].layout_constraint,
        native_bounds(entry.text_bound()),
        theme,
        &renderer,
        Some(NativeTableDocumentSource::from_entry(entry, prepared)),
    )
    .unwrap()
    .unwrap();
    assert!(
        drawing
            .rows
            .iter()
            .flat_map(|row| &row.cells)
            .all(|cell| cell.native_text_placement().unwrap().provenance()
                == NativeCellTextClipProvenance::Unknown)
    );
    let mut scene = crate::render::Scene::new(crate::render::Svg::new());
    #[cfg(feature = "pdf")]
    scene.retain_text();
    crate::render::render_table(
        &mut scene,
        table,
        0,
        Some((&drawing).into()),
        0.0,
        &[],
        theme,
        &renderer,
        None,
    );
    #[cfg(feature = "pdf")]
    assert_eq!(scene.take_native_text().iter().count(), 6);
    let svg = scene.finish();
    let xml = roxmltree::Document::parse(&svg).unwrap();
    let groups = xml
        .descendants()
        .filter(|node| node.has_tag_name("text"))
        .collect::<Vec<_>>();
    assert_eq!(groups.len(), 6);
    for group in &groups {
        assert_eq!(
            group
                .ancestors()
                .filter_map(|node| node.attribute("clip-path"))
                .count(),
            1
        );
    }
    assert_eq!(
        groups
            .iter()
            .flat_map(|group| group.descendants())
            .filter(|node| node.is_text())
            .filter_map(|node| node.text())
            .collect::<String>(),
        "AVTolastToA\u{301}Blast"
    );
    assert_eq!(
        renderer.object_diagnostics(),
        [crate::render::ObjectDiagnostic {
            anchor_utf16: 0,
            kind: crate::render::ObjectDiagnosticKind::UnsupportedCellClipping,
        }]
    );
    assert!(!svg.contains("<image"));
    assert_eq!(source, original);
}

#[test]
fn table_rect_and_content_setters_apply_separate_inclusive_epsilon_gates() {
    let empty = NativeTableModelState::new([0.0; 4], [0.0; 4]).unwrap();
    let at_epsilon = empty
        .set_rect([0.01, 0.0, 0.01, 0.0], NativeTableContentFit::default())
        .unwrap();
    assert_eq!(at_epsilon.model, empty);
    assert!(at_epsilon.resize.is_none());
    let above = f32::from_bits(0.01_f32.to_bits() + 1);
    let outside = empty
        .set_rect([above, 0.0, above, 0.0], NativeTableContentFit::default())
        .unwrap();
    assert_eq!(outside.model.content_rect(), [above, 0.0, above, 0.0]);
    assert_eq!(
        outside.resize,
        Some(NativeTableContentResize {
            column_scale: 0.0,
            row_scale: 0.0,
        })
    );
    let original =
        NativeTableModelState::new([0.0, 0.0, 10.0, 10.0], [0.0, 0.0, 10.0, 10.0]).unwrap();
    let content_noop = original
        .set_rect([0.005, 0.0, 20.0, 10.0], NativeTableContentFit::default())
        .unwrap();
    assert_eq!(content_noop.model.raw_rect(), [0.005, 0.0, 10.005, 10.0]);
    assert_eq!(content_noop.model.content_rect(), original.content_rect());
    assert!(content_noop.resize.is_none());
    for requested in [[f32::NAN; 4], [2.0, 0.0, 1.0, 1.0]] {
        assert!(
            original
                .set_rect(requested, NativeTableContentFit::default())
                .is_err()
        );
    }
    let overflow =
        NativeTableModelState::new([0.0, 0.0, 1.0, 1.0], [-f32::MAX, 0.0, f32::MAX, 1.0]).unwrap();
    assert!(
        overflow
            .set_rect([1.0, 0.0, 2.0, 1.0], NativeTableContentFit::default())
            .is_err()
    );
    assert_eq!(original.raw_rect(), [0.0, 0.0, 10.0, 10.0]);
}

#[cfg(feature = "serde")]
fn observed_bits(value: &serde_json::Value) -> [u32; 4] {
    serde_json::from_value(value.clone()).unwrap()
}

#[test]
#[cfg(feature = "serde")]
fn ordinary_parent_model_transitions_match_immediate_native_content_and_dimensions() {
    use crate::render::embedded::PreparedObject;
    use crate::render::table::{drawn_bounds_for_rect, table_drawn_bounds};

    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/table-bodytext-one-page-placement.json"
    ));
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "cc0810421dceddffaa3454e8010ece57d7afddcce1f67dba1b662eeea4acd15a"
    );
    let capture: serde_json::Value = serde_json::from_slice(bytes).unwrap();
    let case = &capture["cases"][0];
    let writer = &case["writer"];
    assert_eq!(
        writer["native_page_measurement"]["height_without_last_page"],
        0
    );
    assert_eq!(
        writer["native_page_measurement"]["widget_layout_height_bits"],
        0
    );
    let (table, layout) = crate::render::text::native_object_capture_profile(case);
    let original = table.clone();
    let entry = layout.native_object_entry.unwrap();
    assert_rect(
        entry.text_bound(),
        &writer["document_bound_bits"],
        "document bound",
    );
    let Some(Ok(PreparedObject::Table(prepared))) = &layout.lines[0].line.objects[0].prepared
    else {
        panic!("actual source-derived preparation required")
    };
    let mut model = NativeTableModelState::new(
        native_coordinates(table.bbox),
        native_coordinates(table.style.content_bbox.unwrap()),
    )
    .unwrap();
    let mut columns = table.column_widths.clone();
    let mut rows = table.rows.iter().map(|row| row.height).collect::<Vec<_>>();
    let maximum_rows = table
        .rows
        .iter()
        .map(|row| row.max_height.unwrap_or(f32::MAX))
        .collect::<Vec<_>>();
    let transitions = writer["model_state_transitions"].as_array().unwrap();
    assert_eq!(transitions.len(), 6);
    let check =
        |index: usize, stage: &str, model: NativeTableModelState, columns: &[f32], rows: &[f32]| {
            let observed = &transitions[index];
            assert_eq!(observed["stage"], stage);
            assert_rect(model.raw_rect(), &observed["raw_rect_bits"], stage);
            assert_rect(model.content_rect(), &observed["content_rect_bits"], stage);
            for (values, field) in [
                (columns, "stored_column_width_bits"),
                (rows, "stored_row_height_bits"),
                (maximum_rows.as_slice(), "maximum_row_height_bits"),
            ] {
                assert_eq!(
                    values
                        .iter()
                        .map(|value| value.to_bits())
                        .collect::<Vec<_>>(),
                    serde_json::from_value::<Vec<u32>>(observed[field].clone()).unwrap(),
                    "{stage} {field}"
                );
            }
        };
    let mut offset = [0.0; 2];
    for index in [0, 4] {
        if index == 4 {
            offset = serde_json::from_value::<[u32; 2]>(
                writer["supplied_moved_page_origin_bits"].clone(),
            )
            .unwrap()
            .map(f32::from_bits);
        }
        let drawn = if index == 0 {
            table_drawn_bounds(&table)
        } else {
            drawn_bounds_for_rect(&table, native_bounds(model.raw_rect()))
        };
        let placement = NativeDocumentTablePlacement::from_entry(
            entry,
            offset,
            native_bounds(model.raw_rect()),
            drawn,
        )
        .unwrap();
        let request = native_coordinates(placement.parent_rect());
        assert_rect(
            request,
            &transitions[index]["input_bits"],
            "derived SetRect request",
        );
        check(index, "SetRect.entry", model, &columns, &rows);
        let update = model
            .set_rect(request, NativeTableContentFit::default())
            .unwrap();
        if let Some(resize) = update.resize {
            for column in &mut columns {
                *column *= resize.column_scale;
            }
            for (row, &maximum) in rows.iter_mut().zip(&maximum_rows) {
                let resized = resize.row_scale * *row;
                if resized <= maximum {
                    *row = resized;
                }
            }
        }
        model = update.model;
        check(index + 1, "SetRect.complete", model, &columns, &rows);
        if index == 0 {
            let content = native_coordinates(prepared.content_bbox);
            let size = [content[2] - content[0], content[3] - content[1]];
            assert_eq!(
                size.map(f32::to_bits),
                serde_json::from_value::<[u32; 2]>(transitions[2]["input_bits"].clone()).unwrap()
            );
            check(2, "SetContentSize.entry", model, &columns, &rows);
            model = model
                .set_content_size(size, NativeTableContentFit::default())
                .unwrap();
            check(3, "SetContentSize.complete", model, &columns, &rows);
        }
    }
    assert_eq!(table, original);
}

#[cfg(feature = "serde")]
fn assert_rect(actual: [f32; 4], expected: &serde_json::Value, label: &str) {
    assert_eq!(actual.map(f32::to_bits), observed_bits(expected), "{label}");
}

#[test]
#[cfg(feature = "serde")]
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
                    .unwrap()
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

#[test]
#[cfg(feature = "serde")]
fn near_equal_document_bounds_withdraw_cell_feedback_provenance() {
    use crate::fonts::FontBook;
    use crate::render::embedded::PreparedObject;
    use crate::render::table::{
        NativeTableDocumentSource, prepare_table_clone_drawing_with_native_entry,
        table_drawn_bounds,
    };
    use crate::render::text::{StyledText, TextContext, TextFrame, TextRenderer, layout_flow_text};
    use crate::{RichTextObjectContent, TableBorder, TableEdgeStyle};

    let capture: serde_json::Value = serde_json::from_slice(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/table-bodytext-placement.json"
    )))
    .unwrap();
    let case = capture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == "overpages-zero-origin")
        .unwrap();
    let mut source = crate::render::text::native_object_capture_source(case);
    let edge = TableEdgeStyle {
        color: 0xff000000,
        width: 0.0,
        start_radius: 0.0,
        end_radius: 0.0,
    };
    let border = TableBorder {
        left: edge,
        top: edge,
        right: edge,
        bottom: edge,
        metadata: Default::default(),
    };
    let Some(RichTextObjectContent::Table(table)) = &mut source.object_spans[0].content else {
        panic!("expected source table")
    };
    table.style.border = Some(border.clone());
    table.style.default_cell_border = Some(border);
    let fonts = FontBook::default();
    let renderer = TextRenderer::new(Default::default(), &fonts);
    let theme = crate::render::RenderTheme::for_canvas(false);
    let layout_source = |source: &crate::RichTextBox| {
        layout_flow_text(
            &StyledText::new(source, TextContext::Flow, renderer.settings),
            TextFrame {
                bbox: source.bbox,
                gravity: Some(0),
                exclusions: &[],
            },
            theme,
            &renderer,
        )
    };
    let measured = layout_source(&source).native_object_entry.unwrap().height;
    let Some(RichTextObjectContent::Table(table)) = &mut source.object_spans[0].content else {
        unreachable!()
    };
    table.bbox.y_max = f64::from(measured);
    table.style.content_bbox = Some(table.bbox);
    let original = source.clone();
    let layout = layout_source(&source);
    let entry = layout.native_object_entry.unwrap();
    let Some(RichTextObjectContent::Table(table)) = &source.object_spans[0].content else {
        unreachable!()
    };
    let parent = NativeDocumentTablePlacement::from_entry(
        entry,
        [0.0; 2],
        table.bbox,
        table_drawn_bounds(table),
    )
    .unwrap();
    let differences = std::array::from_fn::<_, 4, _>(|axis| {
        (native_coordinates(parent.parent_rect())[axis] - native_coordinates(table.bbox)[axis])
            .abs()
    });
    assert!(
        differences
            .into_iter()
            .all(|difference| difference <= 0.001_f32)
    );
    assert!(differences.into_iter().any(|difference| difference > 0.0));
    let Some(Ok(PreparedObject::Table(prepared))) = &layout.lines[0].line.objects[0].prepared
    else {
        panic!("expected actual prepared table")
    };
    let drawing = prepare_table_clone_drawing_with_native_entry(
        table,
        source.object_spans[0].layout_constraint,
        native_bounds(entry.text_bound()),
        theme,
        &renderer,
        Some(NativeTableDocumentSource::from_entry(entry, prepared)),
    )
    .unwrap()
    .unwrap();
    assert!(
        drawing
            .rows
            .iter()
            .flat_map(|row| &row.cells)
            .all(|cell| cell.native_text_placement().unwrap().provenance()
                == NativeCellTextClipProvenance::Unknown)
    );
    assert_eq!(source, original);
}
