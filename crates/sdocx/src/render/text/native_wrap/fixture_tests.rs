use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::*;

const CAPTURE: &str = include_str!("../../../../../../conformance/table-text-wrap-numeric.json");

#[derive(Deserialize)]
struct Capture {
    apk_sha256: String,
    text_library_sha256: String,
    base_library_sha256: String,
    memory_fills: [u8; 3],
    repeat_zero_fill: bool,
    get_block_info: String,
    set_layout: String,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    supplied_entries: Vec<Entry>,
    requested_range_utf16: [usize; 2],
    supplied_available_rect_bits: [u32; 4],
    supplied_size_bits: u32,
    supplied_layout_options: u32,
    visual_to_logical_utf16: Vec<usize>,
    budget_bits: u32,
    alignment: u32,
    selected_range_utf16_inclusive: [usize; 2],
    block_layout_rect_bits: [u32; 4],
    block_metric_bits: [u32; 4],
    space_count: u32,
    candidate_operations: Vec<Candidate>,
    commit_operations: Vec<Commit>,
    entries: Vec<PositionedEntry>,
    retained_line: RetainedLine,
}

#[derive(Deserialize)]
struct Entry {
    advance_bits: u32,
    kind: u32,
    break_end_utf16: usize,
    object_type: u32,
}

#[derive(Deserialize)]
struct Candidate {
    entry_utf16: usize,
    committed_bits: u32,
    pending_bits: u32,
    advance_bits: u32,
    base_bits: u32,
    candidate_bits: u32,
    available_bits: u32,
}

#[derive(Deserialize)]
struct Commit {
    entry_utf16: usize,
    reason: String,
    committed_before_bits: u32,
    pending_bits: u32,
    committed_after_bits: u32,
}

#[derive(Deserialize)]
struct PositionedEntry {
    position_bits: [u32; 2],
    layout_rect_bits: [u32; 4],
}

#[derive(Deserialize)]
struct RetainedLine {
    layout_rect_bits: [u32; 4],
}

fn capture() -> Capture {
    assert_eq!(
        format!("{:x}", Sha256::digest(CAPTURE.as_bytes())),
        "b3380614708e4a16712f2bdfa9d9a80ed3610b733f4ff274d50d09da962e09b8"
    );
    let capture: Capture = serde_json::from_str(CAPTURE).unwrap();
    assert_eq!(
        capture.apk_sha256,
        "daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667"
    );
    assert_eq!(
        capture.text_library_sha256,
        "5483711673a499743625eb3275e34b37a006919af346212b46b8d8857834308b"
    );
    assert_eq!(
        capture.base_library_sha256,
        "e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb"
    );
    assert_eq!(capture.memory_fills, [0, 0xa5, 0xff]);
    assert!(capture.repeat_zero_fill);
    assert_eq!(capture.get_block_info, "0x6ab9c");
    assert_eq!(capture.set_layout, "0x6b4a4");
    assert_eq!(capture.cases.len(), 17);
    capture
}

fn kind(value: u32) -> NativeWrapKind {
    match value {
        0 => NativeWrapKind::Ordinary,
        1 => NativeWrapKind::Space,
        2 => NativeWrapKind::Tab,
        _ => panic!("unsupported capture entry kind {value}"),
    }
}

fn entries(case: &Case) -> Vec<NativeWrapEntry> {
    case.supplied_entries
        .iter()
        .map(|entry| {
            assert_eq!(entry.object_type, 0);
            NativeWrapEntry {
                advance: f32::from_bits(entry.advance_bits),
                kind: kind(entry.kind),
                break_end_utf16: Some(entry.break_end_utf16),
                metrics: NativeWrapMetrics {
                    font_size: f32::from_bits(case.supplied_size_bits),
                    height: f32::from_bits(case.supplied_size_bits),
                },
            }
        })
        .collect()
}

#[test]
fn native_wrap_replays_actual_candidates_commits_and_selected_blocks() {
    let capture = capture();
    let mut candidate_count = 0;
    let mut commit_count = 0;
    let mut supplied_count = 0;
    let mut selected_count = 0;
    for case in &capture.cases {
        let entries = entries(case);
        let mut operations = Vec::new();
        let [start, end] = case.requested_range_utf16;
        assert_eq!(
            case.supplied_available_rect_bits,
            [0, 0, case.budget_bits, 100_f32.to_bits()]
        );
        let block = select_with_operations(
            &entries,
            start..end,
            NativeWrapWidths {
                available: f32::from_bits(case.budget_bits),
                full: f32::from_bits(case.budget_bits),
            },
            |operation| operations.push(operation),
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            [
                *block.range_utf16_inclusive.start(),
                *block.range_utf16_inclusive.end()
            ],
            case.selected_range_utf16_inclusive,
            "{} range",
            case.name
        );
        assert_eq!(
            block.width.to_bits(),
            case.block_layout_rect_bits[2],
            "{} width",
            case.name
        );
        assert_eq!(
            [
                block.metrics.font_size.to_bits(),
                block.metrics.height.to_bits(),
                0,
                0
            ],
            case.block_metric_bits,
            "{} metrics",
            case.name
        );
        assert_eq!(block.space_weight, case.space_count, "{} spaces", case.name);
        let mut candidates = case.candidate_operations.iter();
        let mut commits = case.commit_operations.iter();
        for operation in operations {
            match operation {
                NativeWrapOperation::Candidate {
                    entry_utf16,
                    committed,
                    pending,
                    advance,
                    base,
                    candidate,
                    available,
                } => {
                    let expected = candidates.next().unwrap();
                    assert_eq!(
                        entry_utf16, expected.entry_utf16,
                        "{} candidate source",
                        case.name
                    );
                    assert_eq!(
                        [
                            committed.to_bits(),
                            pending.to_bits(),
                            advance.to_bits(),
                            base.to_bits(),
                            candidate.to_bits(),
                            available.to_bits()
                        ],
                        [
                            expected.committed_bits,
                            expected.pending_bits,
                            expected.advance_bits,
                            expected.base_bits,
                            expected.candidate_bits,
                            expected.available_bits
                        ],
                        "{} candidate {entry_utf16}",
                        case.name
                    );
                    candidate_count += 1;
                }
                NativeWrapOperation::Commit {
                    entry_utf16,
                    reason,
                    committed_before,
                    pending,
                    committed_after,
                } => {
                    let expected = commits.next().unwrap();
                    assert_eq!(
                        entry_utf16, expected.entry_utf16,
                        "{} commit source",
                        case.name
                    );
                    assert_eq!(
                        reason,
                        match expected.reason.as_str() {
                            "break_end" => NativeCommitReason::BreakEnd,
                            "space" => NativeCommitReason::Space,
                            "tab" => NativeCommitReason::Tab,
                            _ => panic!("unknown native commit"),
                        }
                    );
                    assert_eq!(
                        [
                            committed_before.to_bits(),
                            pending.to_bits(),
                            committed_after.to_bits()
                        ],
                        [
                            expected.committed_before_bits,
                            expected.pending_bits,
                            expected.committed_after_bits
                        ],
                        "{} commit {entry_utf16}",
                        case.name
                    );
                    commit_count += 1;
                }
            }
        }
        assert!(
            candidates.next().is_none() && commits.next().is_none(),
            "{} missing operations",
            case.name
        );
        supplied_count += entries.len();
        selected_count += block.range_utf16_inclusive.count();
    }
    assert_eq!(
        (
            supplied_count,
            candidate_count,
            commit_count,
            selected_count
        ),
        (59, 55, 16, 45)
    );
}

#[test]
fn native_entry_cursor_replays_actual_set_layout_in_visual_slot_order() {
    let capture = capture();
    for case in &capture.cases {
        let entries = entries(case);
        let [start, end] = case.requested_range_utf16;
        let available = f32::from_bits(case.budget_bits);
        let block = select_native_block(
            &entries,
            start..end,
            NativeWrapWidths {
                available,
                full: available,
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(case.supplied_layout_options, 0);
        let share = if case.alignment == 3 {
            block.justification_share(available).unwrap()
        } else {
            0.0
        };
        let mut cursor =
            NativeEntryCursor::new(f32::from_bits(case.block_layout_rect_bits[0])).unwrap();
        for visual in block.range_utf16_inclusive.clone() {
            let logical = case.visual_to_logical_utf16[visual];
            let entry = entries[logical];
            let placed = cursor
                .advance_justified(entry.advance, entry.kind, share)
                .unwrap();
            let expected = &case.entries[logical];
            assert_eq!(
                placed.left.to_bits(),
                expected.position_bits[0],
                "{} position slot {logical}",
                case.name
            );
            assert_eq!(
                [placed.left.to_bits(), placed.right.to_bits()],
                [expected.layout_rect_bits[0], expected.layout_rect_bits[2]],
                "{} rectangle slot {logical}",
                case.name
            );
        }
        assert_eq!(
            cursor.x.to_bits(),
            case.retained_line.layout_rect_bits[2],
            "{} final X",
            case.name
        );
    }
}

#[test]
fn captured_grouping_and_ulp_controls_distinguish_numeric_stages() {
    let capture = capture();
    let case = |name| capture.cases.iter().find(|case| case.name == name).unwrap();
    let grouped = case("grouped_f32");
    let continuous = case("continuous_f32");
    assert_eq!(grouped.block_layout_rect_bits[2], 16_777_218_f32.to_bits());
    assert_eq!(
        continuous.block_layout_rect_bits[2],
        16_777_216_f32.to_bits()
    );
    assert_eq!(
        grouped.retained_line.layout_rect_bits[2],
        continuous.retained_line.layout_rect_bits[2]
    );
    assert_eq!(case("below_budget").selected_range_utf16_inclusive, [0, 1]);
    assert_eq!(case("exact_budget").selected_range_utf16_inclusive, [0, 2]);
    assert_eq!(case("above_budget").selected_range_utf16_inclusive, [0, 2]);
    assert_eq!(
        case("break_at_index_zero").selected_range_utf16_inclusive,
        [0, 1]
    );
    assert_eq!(
        case("zero_continuations").selected_range_utf16_inclusive,
        [0, 2]
    );
    assert_eq!(
        case("oversized_first_slot").selected_range_utf16_inclusive,
        [0, 0]
    );
    assert_eq!(case("space_commit").space_count, 1);
    assert_eq!(case("tab_commit").space_count, 4);
}

#[test]
fn native_wrap_rejects_invalid_source_and_numeric_domains_without_mutating_cursor() {
    let entry = NativeWrapEntry {
        advance: 1.0,
        kind: NativeWrapKind::Ordinary,
        break_end_utf16: None,
        metrics: NativeWrapMetrics {
            font_size: 1.0,
            height: 1.0,
        },
    };
    let widths = NativeWrapWidths {
        available: 10.0,
        full: 10.0,
    };
    assert_eq!(
        select_native_block(&[entry], 0..0, widths),
        Err(NativeWrapError::InvalidRange)
    );
    assert_eq!(
        select_native_block(&[entry], 0..2, widths),
        Err(NativeWrapError::InvalidRange)
    );
    assert_eq!(
        select_native_block(
            &[NativeWrapEntry {
                advance: f32::NAN,
                ..entry
            }],
            0..1,
            widths
        ),
        Err(NativeWrapError::InvalidAdvance)
    );
    assert_eq!(
        select_native_block(
            &[NativeWrapEntry {
                break_end_utf16: Some(0),
                ..entry
            }],
            0..1,
            widths
        ),
        Err(NativeWrapError::InvalidBreakEnd)
    );
    assert_eq!(
        select_native_block(
            &[entry],
            0..1,
            NativeWrapWidths {
                available: f32::INFINITY,
                ..widths
            }
        ),
        Err(NativeWrapError::InvalidGeometry)
    );
    let mut cursor = NativeEntryCursor::new(f32::MAX).unwrap();
    assert_eq!(
        cursor.advance(f32::MAX),
        Err(NativeWrapError::NumericOverflow)
    );
    assert_eq!(cursor.x.to_bits(), f32::MAX.to_bits());
}
