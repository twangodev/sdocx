use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::*;

#[derive(Deserialize)]
struct Capture {
    apk_version: String,
    clip_start: String,
    measurement_inputs: String,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    source_bbox: [f32; 4],
    run_bbox: [f32; 4],
    cell_origin: [f32; 2],
    selected: bool,
    world_clip: Option<[f32; 4]>,
}

#[test]
fn conditional_cell_clip_matches_all_supplied_native_writer_controls() {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/table-text-clipping.json"
    ));
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "6e72c668c96255d87561db715cbec349371ae02be499ead7d675440165343850"
    );
    let capture: Capture = serde_json::from_slice(bytes).unwrap();
    assert_eq!(capture.apk_version, "4.4.45.37");
    assert_eq!(capture.clip_start, "0x37f6ac");
    assert!(capture.measurement_inputs.contains("Supplied"));
    assert_eq!(capture.cases.len(), 132);
    let mut selected = 0;
    for case in capture.cases {
        let context = NativeCellTextClipContext::new(case.source_bbox, case.cell_origin).unwrap();
        match context.decide(case.run_bbox).unwrap() {
            NativeCellRunClip::Unclipped => {
                assert!(!case.selected, "{}", case.name);
                assert!(case.world_clip.is_none(), "{}", case.name);
            }
            NativeCellRunClip::Clipped { world_rect, .. } => {
                assert!(case.selected, "{}", case.name);
                assert_eq!(
                    world_rect.map(f32::to_bits),
                    case.world_clip.unwrap().map(f32::to_bits),
                    "{}",
                    case.name,
                );
                selected += 1;
            }
        }
    }
    assert_eq!(selected, 105);
}

#[derive(Deserialize)]
struct ProducerCapture {
    memory_fills: Vec<u8>,
    repeat_zero_fill: bool,
    capture_boundary: String,
    cases: Vec<ProducerCase>,
}

#[derive(Deserialize)]
struct ProducerCase {
    name: String,
    writer: ProducerWriter,
}

#[derive(Deserialize)]
struct ProducerWriter {
    cells: Vec<ProducerCell>,
}

#[derive(Deserialize)]
struct ProducerCell {
    runs: Vec<ProducerRun>,
}

#[derive(Deserialize)]
struct ProducerRun {
    actual_source_rect_bits: [u32; 4],
    caller_origin_bits: [u32; 2],
    local_rect_bits: [u32; 4],
    source_is_selected_cell: bool,
    is_object: bool,
    clip_selected: bool,
    intersect_succeeded: Option<bool>,
    world_clip_bits: Option<[u32; 4]>,
}

#[test]
fn conditional_cell_clip_matches_actual_cell_source_and_cached_run_producers() {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/table-live-text-clipping.json"
    ));
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "3106348e8271cc6ca15437c0daa792c8eca86097f5505ef3c5cf55d17d054503"
    );
    let capture: ProducerCapture = serde_json::from_slice(bytes).unwrap();
    assert_eq!(capture.memory_fills, [0, 85, 165, 255]);
    assert!(capture.repeat_zero_fill);
    assert!(
        capture
            .capture_boundary
            .contains("complete writeTextContent and writeTextBlock")
    );
    assert!(
        capture
            .capture_boundary
            .contains("Bodytext callback activation")
    );
    assert_eq!(capture.cases.len(), 11);
    let mut observed_runs = 0;
    let mut selected_clips = 0;
    let mut successful_intersections = 0;
    for case in capture.cases {
        for cell in case.writer.cells {
            for run in cell.runs {
                assert!(run.source_is_selected_cell, "{}", case.name);
                assert!(!run.is_object, "{}", case.name);
                let context = NativeCellTextClipContext::new(
                    run.actual_source_rect_bits.map(f32::from_bits),
                    run.caller_origin_bits.map(f32::from_bits),
                )
                .unwrap();
                match context
                    .decide(run.local_rect_bits.map(f32::from_bits))
                    .unwrap()
                {
                    NativeCellRunClip::Unclipped => {
                        assert!(!run.clip_selected, "{}", case.name);
                        assert_eq!(run.world_clip_bits, None, "{}", case.name);
                        assert_eq!(run.intersect_succeeded, None, "{}", case.name);
                    }
                    NativeCellRunClip::Clipped {
                        world_rect,
                        intersects_source,
                    } => {
                        assert!(run.clip_selected, "{}", case.name);
                        assert_eq!(
                            Some(world_rect.map(f32::to_bits)),
                            run.world_clip_bits,
                            "{}",
                            case.name
                        );
                        assert_eq!(
                            Some(intersects_source),
                            run.intersect_succeeded,
                            "{}",
                            case.name
                        );
                        selected_clips += 1;
                        successful_intersections += usize::from(intersects_source);
                    }
                }
                observed_runs += 1;
            }
        }
    }
    assert_eq!(
        (observed_runs, selected_clips, successful_intersections),
        (47, 8, 3)
    );
}
