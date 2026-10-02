use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::*;

const CAPTURE: &str = include_str!("../../../../../../conformance/table-text-object-feedback.json");

#[derive(Deserialize)]
struct Capture {
    cases: Vec<Case>,
}
#[derive(Deserialize)]
struct Case {
    name: String,
    route: String,
    supplied_advances_bits: Vec<u32>,
    supplied_kinds: Vec<u32>,
    supplied_break_ends_utf16: Vec<usize>,
    object_utf16: usize,
    source_start_utf16: usize,
    budget_bits: u32,
    paragraph_full_width_bits: u32,
    entry_before: ObjectEntry,
    stages: Vec<Stage>,
}
#[derive(Deserialize)]
struct ObjectEntry {
    advance_bits: u32,
    over_pages: bool,
}
#[derive(Deserialize)]
struct Stage {
    stage: String,
    selection: Option<Selection>,
    entry_after: ObjectEntry,
    callback_calls: usize,
    events: Vec<Event>,
}
#[derive(Deserialize)]
struct Selection {
    range_utf16_inclusive: [i32; 2],
    layout_bits: [u32; 4],
    flags: [u8; 3],
}
#[derive(Deserialize)]
struct Event {
    stage: String,
    relative_utf16: Option<usize>,
    candidate_register_bits: Option<u32>,
    source_utf16_anchor: Option<usize>,
}

#[test]
fn captured_object_admission_retains_old_candidate_and_reloads_prepared_width() {
    assert_eq!(
        format!("{:x}", Sha256::digest(CAPTURE.as_bytes())),
        "3c0e26208b66a67c7b90d7941b5bebf3633af73a4eb53dfc50a643b818d15d19"
    );
    let capture: Capture = serde_json::from_str(CAPTURE).unwrap();
    assert_eq!(capture.cases.len(), 34);
    let mut checked = 0;
    for case in capture.cases.into_iter().filter(|case| {
        case.route == "GetBlockInfo"
            && case.entry_before.over_pages
            && case
                .supplied_kinds
                .iter()
                .all(|kind| matches!(kind, 0 | 1 | 2 | 5))
    }) {
        let mut entries = case
            .supplied_advances_bits
            .iter()
            .enumerate()
            .map(|(index, &bits)| NativeWrapEntry {
                advance: f32::from_bits(bits),
                kind: match case.supplied_kinds[index] {
                    1 => NativeWrapKind::Space,
                    2 => NativeWrapKind::Tab,
                    _ => NativeWrapKind::Ordinary,
                },
                break_end_utf16: Some(case.supplied_break_ends_utf16[index]),
                metrics: NativeWrapMetrics {
                    font_size: 17.25,
                    height: 17.25,
                },
            })
            .collect::<Vec<_>>();
        let mut objects = vec![false; entries.len()];
        objects[case.object_utf16] = true;
        for stage in case.stages {
            let mut callbacks = Vec::new();
            let mut candidates = Vec::new();
            let block = select_with_feedback(
                &entries,
                0..entries.len(),
                NativeWrapWidths {
                    available: f32::from_bits(case.budget_bits),
                    full: f32::from_bits(case.paragraph_full_width_bits),
                },
                Some(&objects),
                |slot, _, _| {
                    callbacks.push(case.source_start_utf16 + slot);
                    Ok(f32::from_bits(stage.entry_after.advance_bits))
                },
                |operation| {
                    if let NativeWrapOperation::Candidate {
                        entry_utf16,
                        candidate,
                        ..
                    } = operation
                    {
                        candidates.push((entry_utf16, candidate.to_bits()));
                    }
                },
            )
            .unwrap();
            let selection = stage.selection.as_ref().unwrap();
            if let Some(block) = &block {
                assert_eq!(
                    block.encountered_object_metric,
                    selection.flags[1] != 0,
                    "{} {} encountered object metric",
                    case.name,
                    stage.stage
                );
            }
            assert_eq!(
                block
                    .as_ref()
                    .map(|block| [
                        *block.range_utf16_inclusive.start() as i32,
                        *block.range_utf16_inclusive.end() as i32
                    ])
                    .unwrap_or([0, -1]),
                selection.range_utf16_inclusive,
                "{} {} range",
                case.name,
                stage.stage
            );
            assert_eq!(
                block.map_or(0.0, |block| block.width).to_bits(),
                selection.layout_bits[2],
                "{} {} grouped width",
                case.name,
                stage.stage
            );
            assert_eq!(
                callbacks.len(),
                stage.callback_calls,
                "{} {} callbacks",
                case.name,
                stage.stage
            );
            let actual_anchors = stage
                .events
                .iter()
                .filter_map(|event| event.source_utf16_anchor)
                .collect::<Vec<_>>();
            assert_eq!(
                callbacks, actual_anchors,
                "{} {} source anchors",
                case.name, stage.stage
            );
            let actual_candidates = stage
                .events
                .iter()
                .filter(|event| event.stage == "Text+0x6ae1c")
                .map(|event| {
                    (
                        event.relative_utf16.unwrap(),
                        event.candidate_register_bits.unwrap(),
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(
                candidates, actual_candidates,
                "{} {} saved candidates",
                case.name, stage.stage
            );
            entries[case.object_utf16].advance = f32::from_bits(stage.entry_after.advance_bits);
            checked += 1;
        }
    }
    assert_eq!(checked, 27);
}

#[test]
fn object_inputs_are_validated_before_any_preparation() {
    let entry = NativeWrapEntry {
        advance: 1.0,
        kind: NativeWrapKind::Ordinary,
        break_end_utf16: Some(2),
        metrics: NativeWrapMetrics::default(),
    };
    let entries = [
        entry,
        NativeWrapEntry {
            advance: f32::NAN,
            ..entry
        },
    ];
    let result = select_native_block_with_objects(
        &entries,
        0..2,
        NativeWrapWidths {
            available: 10.0,
            full: 10.0,
        },
        &[true, false],
        |_, _| panic!("invalid subsequent input must not partially prepare earlier objects"),
    );
    assert_eq!(result, Err(NativeWrapError::InvalidAdvance));
}
