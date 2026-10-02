use super::*;
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
struct Capture {
    separator_ranges_utf16: Vec<[u32; 2]>,
    helper_unit_ranges_utf16: Vec<[u32; 4]>,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    source: String,
    helper_queries: Vec<[usize; 3]>,
    selections: Vec<Selection>,
}

#[derive(Deserialize)]
struct Selection {
    range_utf16: [u32; 2],
    rtl: bool,
    windows: Vec<Window>,
}

#[derive(Deserialize)]
struct Window {
    context_utf16: [u32; 2],
    selected_utf16: [u32; 2],
    caller_view: CallerView,
}

#[derive(Deserialize)]
struct CallerView {
    source_origin_utf16: u32,
    source_length_utf16: u32,
    selected_range_in_view_utf16: [u32; 2],
}

#[test]
fn actual_native_helpers_and_directional_windows_match() {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/table-text-context-windows.json"
    ));
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "07d5f3c44ae387975f8070fe24dbe90049395a5ad8f1ff4e80f4780c36417d5d"
    );
    let capture: Capture = serde_json::from_slice(bytes).unwrap();
    for unit in 0..=u16::MAX {
        assert_eq!(
            separator(unit),
            capture
                .separator_ranges_utf16
                .iter()
                .any(|&[start, end]| (start..end).contains(&u32::from(unit))),
            "separator U+{unit:04X}"
        );
        let &[_, _, previous, next] = capture
            .helper_unit_ranges_utf16
            .iter()
            .find(|&&[start, end, _, _]| (start..end).contains(&u32::from(unit)))
            .unwrap();
        let stencil = [65, unit, 66];
        assert_eq!(
            previous_boundary(&stencil, 3),
            previous as usize,
            "previous stencil U+{unit:04X}"
        );
        assert_eq!(
            next_boundary(&stencil, 0),
            next as usize,
            "next stencil U+{unit:04X}"
        );
    }
    for case in capture.cases {
        let units: Vec<_> = case.source.encode_utf16().collect();
        for [offset, previous, next] in case.helper_queries {
            assert_eq!(
                previous_boundary(&units, offset),
                previous,
                "{} previous {offset}",
                case.name
            );
            assert_eq!(
                next_boundary(&units, offset),
                next,
                "{} next {offset}",
                case.name
            );
        }
        let index = TextIndex::new(&case.source);
        for selection in case.selections {
            let [start, end] = selection.range_utf16;
            let actual = PaintContextWindows::new(&case.source, start..end).unwrap();
            let mut expected = selection.windows;
            if selection.rtl {
                expected.reverse();
            }
            assert_eq!(
                actual.windows().len(),
                expected.len(),
                "{} {start}..{end} rtl{}",
                case.name,
                selection.rtl
            );
            for (actual, expected) in actual.windows().iter().zip(expected) {
                let [context_start, context_end] = expected.context_utf16;
                let [selected_start, selected_end] = expected.selected_utf16;
                assert_eq!(
                    actual.context_range_utf16(),
                    context_start..context_end,
                    "{} {start}..{end} rtl{}",
                    case.name,
                    selection.rtl
                );
                assert_eq!(actual.selected_range_utf16(), selected_start..selected_end);
                let context_scalars = index.utf16_to_char(context_start).unwrap()
                    ..index.utf16_to_char(context_end).unwrap();
                let selected_scalars = index.utf16_to_char(selected_start).unwrap()
                    ..index.utf16_to_char(selected_end).unwrap();
                assert_eq!(actual.context_scalar_range(), context_scalars.clone());
                assert_eq!(actual.selected_scalar_range(), selected_scalars.clone());
                assert_eq!(
                    actual.context_byte_range(),
                    byte_range(&index, &context_scalars)
                );
                assert_eq!(
                    actual.selected_byte_range(),
                    byte_range(&index, &selected_scalars)
                );
                assert_eq!(actual.source(), index.slice(context_scalars).unwrap());
                assert_eq!(
                    actual.selected_range_in_window_utf16(),
                    selected_start - context_start..selected_end - context_start
                );
                let view = expected.caller_view;
                assert_eq!(actual.context_range_utf16().start, view.source_origin_utf16);
                assert_eq!(
                    actual.source().encode_utf16().count() as u32,
                    view.source_length_utf16
                );
                assert_eq!(
                    actual.selected_range_in_window_utf16(),
                    view.selected_range_in_view_utf16[0]..view.selected_range_in_view_utf16[1]
                );
            }
        }
    }
}

#[test]
fn ranges_are_checked_before_retaining_source() {
    for (start, end) in [(2, 1), (0, 5), (2, 3), (1, 2)] {
        assert_eq!(
            PaintContextWindows::new("A😀B", start..end),
            Err(PaintContextError::InvalidRange)
        );
    }
    let empty = PaintContextWindows::new("A😀B", 3..3).unwrap();
    assert_eq!(empty.source(), "A😀B");
    assert!(empty.windows().is_empty());
    assert!(
        PaintContextWindows::new("", 0..0)
            .unwrap()
            .windows()
            .is_empty()
    );
}

#[test]
fn source_selection_and_window_budgets_are_bounded() {
    assert_eq!(
        PaintContextWindows::new(&"A".repeat(MAX_SOURCE_BYTES + 1), 0..1),
        Err(PaintContextError::InputBudget)
    );
    assert_eq!(
        PaintContextWindows::new(&"😀".repeat(MAX_SOURCE_UTF16 / 2 + 1), 0..2),
        Err(PaintContextError::InputBudget)
    );
    assert_eq!(
        PaintContextWindows::new(
            &"A".repeat(MAX_SELECTED_SCALARS + 1),
            0..(MAX_SELECTED_SCALARS + 1) as u32
        ),
        Err(PaintContextError::InputBudget)
    );
    assert_eq!(
        PaintContextWindows::new(&" ".repeat(MAX_WINDOWS + 1), 0..(MAX_WINDOWS + 1) as u32),
        Err(PaintContextError::InputBudget)
    );
    assert_eq!(
        PaintContextWindows::new(&" ".repeat(MAX_WINDOWS), 0..MAX_WINDOWS as u32)
            .unwrap()
            .windows()
            .len(),
        MAX_WINDOWS
    );
}
