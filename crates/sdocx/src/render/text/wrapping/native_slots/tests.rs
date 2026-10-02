use super::super::super::{TextContext, TextDiagnosticKind, TextSettings};
use super::super::tests::{image, text};
use super::*;
use crate::fonts::FontBook;
use crate::{ObjectSpanLayoutOption, ParagraphAlignment};
use serde::Deserialize;

#[derive(Deserialize)]
struct NumericCapture {
    cases: Vec<NumericCase>,
}

#[derive(Deserialize)]
struct NumericCase {
    name: String,
    supplied_entries: Vec<SuppliedEntry>,
    supplied_size_bits: u32,
    budget_bits: u32,
    alignment: u32,
    reverse_visual_map: bool,
    selected_range_utf16_inclusive: [usize; 2],
    block_layout_rect_bits: [u32; 4],
    entries: Vec<PositionedEntry>,
    retained_line: RetainedLine,
}
#[derive(Deserialize)]
struct SuppliedEntry {
    advance_bits: u32,
    kind: u32,
    break_end_utf16: usize,
}
#[derive(Deserialize)]
struct PositionedEntry {
    position_bits: [u32; 2],
}
#[derive(Deserialize)]
struct RetainedLine {
    layout_rect_bits: [u32; 4],
}

fn numeric() -> NumericCapture {
    serde_json::from_str(include_str!(
        "../../../../../../../conformance/table-text-wrap-numeric.json"
    ))
    .unwrap()
}

fn entries(case: &NumericCase) -> Vec<NativeWrapEntry> {
    case.supplied_entries
        .iter()
        .map(|entry| NativeWrapEntry {
            advance: f32::from_bits(entry.advance_bits),
            kind: match entry.kind {
                0 => NativeWrapKind::Ordinary,
                1 => NativeWrapKind::Space,
                2 => NativeWrapKind::Tab,
                _ => panic!("unsupported supplied kind"),
            },
            break_end_utf16: Some(entry.break_end_utf16),
            metrics: NativeWrapMetrics {
                font_size: f32::from_bits(case.supplied_size_bits),
                height: f32::from_bits(case.supplied_size_bits),
            },
        })
        .collect()
}

#[test]
fn production_wrapper_replays_supplied_native_numeric_selection_and_placement() {
    let fonts = FontBook::default();
    let settings = TextSettings::resolved();
    let mut positions = 0;
    for case in numeric().cases {
        let source = if case.reverse_visual_map {
            "אבג".to_owned()
        } else {
            case.supplied_entries
                .iter()
                .enumerate()
                .map(|(index, entry)| match entry.kind {
                    1 => ' ',
                    2 => '\t',
                    _ => char::from_u32('A' as u32 + index as u32).unwrap(),
                })
                .collect()
        };
        let mut content = text(&source);
        content.font_size = Some(f32::from_bits(case.supplied_size_bits));
        let styled = StyledText::new(&content, TextContext::Flow, settings);
        let renderer = TextRenderer::new(settings, &fonts);
        let budget = f64::from(f32::from_bits(case.budget_bits));
        let mut wrapper = ParagraphWrapper::new(
            &styled,
            0..styled.index.len(),
            budget,
            RenderTheme::for_canvas(false),
            None,
            &renderer,
            ObjectMeasurementContext::Frame,
        )
        .unwrap();
        // This capture supplies numeric entries independently of shaping and glyph metrics.
        wrapper.native_slots = Some(NativeParagraphSlots {
            entries: entries(&case),
            origin_utf16: 0,
        });
        let mut line = wrapper
            .candidate(budget, |_| panic!("ordinary slots contain no objects"))
            .unwrap()
            .unwrap();
        assert_eq!(
            line.source,
            case.selected_range_utf16_inclusive[0]..case.selected_range_utf16_inclusive[1] + 1,
            "{} source",
            case.name
        );
        assert_eq!(
            (line.advance as f32).to_bits(),
            case.block_layout_rect_bits[2],
            "{} grouped width",
            case.name
        );
        if case.alignment == 3 {
            line.justify(&styled, budget).unwrap();
        }
        for (index, placement) in line.placements.iter().enumerate() {
            let logical = placement.cluster.source.start;
            assert_eq!(
                (line.text_position(index, true).unwrap().x as f32).to_bits(),
                case.entries[logical].position_bits[0],
                "{} slot {logical}",
                case.name
            );
            positions += 1;
        }
        let LineGeometry::Positioned { advance, .. } = line.geometry else {
            panic!("expected native slot positions")
        };
        assert_eq!(
            (advance as f32).to_bits(),
            case.retained_line.layout_rect_bits[2],
            "{} continuous extent",
            case.name
        );
    }
    assert_eq!(positions, 45);
}

#[test]
fn native_grouped_alignment_uses_block_width_instead_of_cursor_extent() {
    let fixture = numeric();
    let case = fixture
        .cases
        .iter()
        .find(|case| case.name == "grouped_f32")
        .unwrap();
    let content = text("ABC");
    let settings = TextSettings::resolved();
    let styled = StyledText::new(&content, TextContext::Flow, settings);
    let fonts = FontBook::default();
    let renderer = TextRenderer::new(settings, &fonts);
    let mut wrapper = ParagraphWrapper::new(
        &styled,
        0..3,
        16_777_220.0,
        RenderTheme::for_canvas(false),
        None,
        &renderer,
        ObjectMeasurementContext::Frame,
    )
    .unwrap();
    wrapper.native_slots = Some(NativeParagraphSlots {
        entries: entries(case),
        origin_utf16: 0,
    });
    let line = wrapper.candidate(16_777_220.0, |_| {}).unwrap().unwrap();
    let alignment_width = line.advance_for_paint(true).unwrap();
    assert_eq!(alignment_width, 16_777_218.0);
    for (alignment, expected) in [
        (ParagraphAlignment::Right, 2.0),
        (ParagraphAlignment::Center, 1.0),
    ] {
        assert_eq!(
            super::super::super::line_alignment_offset(
                alignment_width,
                16_777_220.0,
                Some(alignment)
            ),
            expected
        );
    }
    let LineGeometry::Positioned { advance, .. } = line.geometry else {
        panic!("missing positions")
    };
    assert_eq!(advance, 16_777_216.0);
}

#[test]
fn captured_native_slot_widths_keep_mark_zeros_after_supplementary_prefix() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../../../conformance/table-text-entry-geometry.json"
    ))
    .unwrap();
    let settings = TextSettings::resolved();
    let fonts = FontBook::default();
    for name in ["av_default", "fractional_size", "large_size", "marks"] {
        let case = fixture["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|case| case["name"] == name)
            .unwrap();
        let captured = case["entry_geometry"]["entry_widths_bits"]
            .as_array()
            .unwrap();
        for prefix in ["", "😀\n"] {
            let mut content = text(&format!("{prefix}{}", case["text_utf8"].as_str().unwrap()));
            content.font_size = Some(f32::from_bits(
                case["font_size_bits"].as_u64().unwrap() as u32
            ));
            let styled = StyledText::new(&content, TextContext::Flow, settings);
            let renderer = TextRenderer::new(settings, &fonts);
            let start = prefix.chars().count();
            let mut wrapper = ParagraphWrapper::new(
                &styled,
                start..styled.index.len(),
                f64::INFINITY,
                RenderTheme::for_canvas(false),
                None,
                &renderer,
                ObjectMeasurementContext::Frame,
            )
            .unwrap();
            let slots = wrapper
                .native_slots
                .as_ref()
                .expect("pinned default face uses native slots");
            assert_eq!(slots.origin_utf16, prefix.encode_utf16().count() as u32);
            assert_eq!(slots.entries.len(), captured.len());
            for (entry, expected) in slots.entries.iter().zip(captured) {
                assert_eq!(
                    entry.advance.to_bits(),
                    expected.as_u64().unwrap() as u32,
                    "{name} slot"
                );
            }
            let origin_utf16 = slots.origin_utf16;
            let line = wrapper.candidate(f64::INFINITY, |_| {}).unwrap().unwrap();
            assert!(line.native_slots.is_some());
            assert_eq!(line.source, start..styled.index.len());
            let mut cursor = 0.0_f32;
            let mut expected_positions = Vec::new();
            for slot in captured {
                expected_positions.push(cursor);
                cursor += f32::from_bits(slot.as_u64().unwrap() as u32);
            }
            for (index, placement) in line.placements.iter().enumerate() {
                let owner = styled
                    .index
                    .char_to_utf16(placement.cluster.source.start)
                    .unwrap()
                    - origin_utf16;
                assert_eq!(
                    line.text_position(index, true).unwrap().x,
                    f64::from(expected_positions[owner as usize]),
                    "{name} owner {owner}"
                );
            }
        }
    }
}

#[test]
fn default_native_exact_fit_uses_f32_budget_boundaries() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../../../conformance/table-text-entry-geometry.json"
    ))
    .unwrap();
    let case = fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == "av_default")
        .unwrap();
    let exact = case["entry_geometry"]["entry_widths_bits"]
        .as_array()
        .unwrap()
        .iter()
        .fold(0.0_f32, |sum, bits| {
            sum + f32::from_bits(bits.as_u64().unwrap() as u32)
        });
    let previous = f32::from_bits(exact.to_bits() - 1);
    let mut content = text("AV");
    content.font_size = Some(f32::from_bits(
        case["font_size_bits"].as_u64().unwrap() as u32
    ));
    let settings = TextSettings::resolved();
    let styled = StyledText::new(&content, TextContext::Flow, settings);
    let fonts = FontBook::default();
    let renderer = TextRenderer::new(settings, &fonts);
    let mut wrapper = ParagraphWrapper::new(
        &styled,
        0..2,
        f64::from(exact),
        RenderTheme::for_canvas(false),
        None,
        &renderer,
        ObjectMeasurementContext::Frame,
    )
    .unwrap();
    assert_eq!(
        wrapper
            .candidate(f64::from(exact), |_| {})
            .unwrap()
            .unwrap()
            .source,
        0..2
    );
    assert_eq!(
        wrapper
            .candidate(f64::from(previous), |_| {})
            .unwrap()
            .unwrap()
            .source,
        0..1
    );
    let rounds_back = f64::from(exact) - (f64::from(exact) - f64::from(previous)) / 4.0;
    assert!(rounds_back < f64::from(exact));
    assert_eq!(
        wrapper
            .candidate(rounds_back, |_| {})
            .unwrap()
            .unwrap()
            .source,
        0..2
    );
}

#[test]
fn supplied_zero_surrogate_slots_preserve_source_and_diagnose_owner_cuts() {
    let fixture = numeric();
    let case = fixture
        .cases
        .iter()
        .find(|case| case.name == "zero_continuations")
        .unwrap();
    let content = text("A😀B");
    let settings = TextSettings::resolved();
    let styled = StyledText::new(&content, TextContext::Flow, settings);
    let fonts = FontBook::default();
    let renderer = TextRenderer::new(settings, &fonts);
    let mut wrapper = ParagraphWrapper::new(
        &styled,
        0..3,
        10.0,
        RenderTheme::for_canvas(false),
        None,
        &renderer,
        ObjectMeasurementContext::Frame,
    )
    .unwrap();
    wrapper.native_slots = Some(NativeParagraphSlots {
        entries: entries(case),
        origin_utf16: 0,
    });
    let line = wrapper.candidate(10.0, |_| {}).unwrap().unwrap();
    assert_eq!(line.source, 0..2);
    assert!(!line.unsupported_native_wrapping());
    assert_eq!(line.native_slots.as_ref().unwrap().entries.len(), 3);
    wrapper.native_slots.as_mut().unwrap().entries[2].advance = 2.0;
    let safe = wrapper.candidate(10.0, |_| {}).unwrap().unwrap();
    assert_eq!(safe.source, 0..1);
    assert!(safe.unsupported_native_wrapping());
    assert!(
        renderer
            .diagnostics()
            .iter()
            .any(|issue| issue.kind == TextDiagnosticKind::UnsupportedNativeWrapping)
    );
}

#[test]
fn narrowed_native_no_block_uses_diagnosed_policy_without_exhausting_source() {
    let fixture = numeric();
    let case = fixture
        .cases
        .iter()
        .find(|case| case.name == "exact_budget")
        .unwrap();
    let content = text("ABC");
    let settings = TextSettings::resolved();
    let styled = StyledText::new(&content, TextContext::Flow, settings);
    let fonts = FontBook::default();
    let renderer = TextRenderer::new(settings, &fonts);
    let mut wrapper = ParagraphWrapper::new(
        &styled,
        0..3,
        30.0,
        RenderTheme::for_canvas(false),
        None,
        &renderer,
        ObjectMeasurementContext::Frame,
    )
    .unwrap();
    wrapper.native_slots = Some(NativeParagraphSlots {
        entries: entries(case),
        origin_utf16: 0,
    });
    assert_eq!(
        wrapper
            .native_slots
            .as_ref()
            .unwrap()
            .select(&styled, 0..3, 5.0, 30.0)
            .unwrap(),
        None
    );
    let mut retained = Vec::new();
    while let Some(line) = wrapper.candidate(5.0, |_| {}).unwrap() {
        assert!(line.unsupported_native_wrapping());
        retained.push(line.source.clone());
        wrapper.commit(line.source.end);
    }
    assert_eq!(retained, [0..1, 1..2, 2..3]);
    assert!(
        renderer
            .diagnostics()
            .iter()
            .any(|issue| issue.kind == TextDiagnosticKind::UnsupportedNativeWrapping)
    );
}

#[test]
fn default_native_glyph_entries_keep_mixed_object_policy_separate() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../../../conformance/table-text-entry-geometry.json"
    ))
    .unwrap();
    let case = fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == "av_default")
        .unwrap();
    let widths = case["entry_geometry"]["entry_widths_bits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| f64::from(f32::from_bits(value.as_u64().unwrap() as u32)))
        .collect::<Vec<_>>();
    let mut content = text("AV\u{fffc}AV");
    content.font_size = Some(f32::from_bits(
        case["font_size_bits"].as_u64().unwrap() as u32
    ));
    content
        .object_spans
        .push(image(2, 20.0, ObjectSpanLayoutOption::Inline));
    let settings = TextSettings::resolved();
    let styled = StyledText::new(&content, TextContext::Flow, settings);
    let fonts = FontBook::default();
    let renderer = TextRenderer::new(settings, &fonts);
    let mut wrapper = ParagraphWrapper::new(
        &styled,
        0..5,
        1000.0,
        RenderTheme::for_canvas(false),
        None,
        &renderer,
        ObjectMeasurementContext::Frame,
    )
    .unwrap();
    assert!(wrapper.native_slots.is_none());
    let line = wrapper.candidate(1000.0, |_| {}).unwrap().unwrap();
    assert_eq!(line.objects[0].x, widths[0] + widths[1]);
    assert_eq!(line.placements[2].x, widths[0] + widths[1] + 20.0);
    assert_eq!(
        line.advance,
        (widths[0] + widths[1]) + 20.0 + widths[0] + widths[1]
    );
    assert!(
        line.placements
            .iter()
            .all(|placement| placement.cluster.run.native_entries.is_some())
    );
    assert!(line.native_slots.is_none());
}
