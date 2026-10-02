use super::*;
use crate::fonts::FontBook;
use crate::render::text::{TextContext, TextSettings};
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
struct Capture {
    memory_fills: Vec<u8>,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    text_utf8: String,
    requested_font_size_bits: Option<u32>,
    supplied_margins_bits: Option<[u32; 4]>,
    native_flags: Vec<u8>,
    measure_widths: Vec<i32>,
    effective_widths: Vec<i32>,
    measured_entries: Vec<Entry>,
    placed_entries: Vec<Entry>,
}

#[derive(Deserialize)]
struct Entry {
    advance_bits: u32,
    font_height_bits: u32,
    font_size_bits: u32,
    kind: u32,
    position_bits: [u32; 2],
}

fn capture() -> Capture {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/table-text-cell-measurement.json"
    ));
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "890c2f236d6cb6711f0821520a491079fa3304d2b64e108a2d3dfcc787549416"
    );
    let capture: Capture = serde_json::from_slice(bytes).unwrap();
    assert_eq!(capture.memory_fills, [0, 165, 255]);
    assert_eq!(capture.cases.len(), 14);
    capture
}

#[test]
fn live_cell_measurement_widths_reuse_native_entries_and_block_selection() {
    let fonts = FontBook::default();
    let settings = TextSettings::resolved();
    let mut paragraphs = 0;
    let mut slots_checked = 0;
    let mut positions_checked = 0;
    for case in capture().cases {
        let Some(size) = case.requested_font_size_bits else {
            continue;
        };
        assert_eq!(size, 17.0_f32.to_bits());
        assert_eq!(case.native_flags, [0, 0, 1, 1]);
        let mut content = super::super::tests::text(&case.text_utf8);
        content.font_size = Some(f32::from_bits(size));
        let margins = case
            .supplied_margins_bits
            .unwrap_or([0; 4])
            .map(f32::from_bits);
        content.margins = Some(margins);
        let styled = StyledText::new(&content, TextContext::Flow, settings);
        let renderer = TextRenderer::new(settings, &fonts);
        for (ordinal, paragraph) in styled.index.display_paragraphs().enumerate() {
            if paragraph.content.is_empty() {
                continue;
            }
            let request = if case.measure_widths[0] == 0 {
                ParagraphMeasurementWidth::Automatic {
                    insets: [margins[0], margins[2]],
                }
            } else {
                ParagraphMeasurementWidth::Constrained(f64::from(case.measure_widths[0]))
            };
            let mut wrapper = ParagraphWrapper::new(
                &styled,
                paragraph.content.clone(),
                request,
                RenderTheme::for_canvas(false),
                None,
                &renderer,
                ObjectMeasurementContext::Frame,
            )
            .unwrap();
            let origin = styled.index.char_to_utf16(paragraph.content.start).unwrap() as usize;
            if matches!(
                case.name.as_str(),
                "supplementary-wrap" | "mixed-hebrew-direction1"
            ) || (case.name == "tab-newline" && ordinal == 0)
            {
                assert!(wrapper.native_slots.is_none(), "{}", case.name);
                continue;
            }
            let slots = wrapper
                .native_slots
                .as_ref()
                .unwrap_or_else(|| panic!("{} paragraph {ordinal} lacks native slots", case.name));
            for (local, slot) in slots.entries().iter().enumerate() {
                let captured = &case.measured_entries[origin + local];
                assert_eq!(
                    slot.advance.to_bits(),
                    captured.advance_bits,
                    "{} slot {local}",
                    case.name
                );
                assert_eq!(
                    slot.metrics.font_size.to_bits(),
                    captured.font_size_bits,
                    "{} slot {local} size",
                    case.name
                );
                assert_eq!(
                    slot.metrics.height.to_bits(),
                    captured.font_height_bits,
                    "{} slot {local} height",
                    case.name
                );
                let kind = match slot.kind {
                    NativeWrapKind::Space => 1,
                    NativeWrapKind::Tab => 2,
                    NativeWrapKind::Ordinary => 0,
                };
                assert_eq!(
                    kind,
                    if captured.kind == 3 { 0 } else { captured.kind },
                    "{} slot {local} kind",
                    case.name
                );
                slots_checked += 1;
            }
            let available = wrapper.measurement_width();
            assert_eq!(
                ((available as f32 + margins[0]) + margins[2]).to_bits(),
                (case.effective_widths[ordinal] as f32).to_bits(),
                "{} paragraph {ordinal}",
                case.name
            );
            let end = styled.index.char_to_utf16(paragraph.content.end).unwrap() as usize;
            let mut captured_line_tops = case.placed_entries[origin..end]
                .iter()
                .filter(|entry| entry.kind != 3)
                .map(|entry| entry.position_bits[1])
                .collect::<Vec<_>>();
            captured_line_tops.dedup();
            let mut line_number = 0;
            while let Some(mut line) = wrapper.candidate(available, |_| {}).unwrap() {
                let alignment = (ordinal == 0).then_some(crate::ParagraphAlignment::Center);
                let line_origin = line
                    .place_native_cell(&styled, f64::from(margins[0]), available, alignment)
                    .unwrap()
                    .unwrap();
                let mut captured_top = None;
                for (index, placement) in line.placements.iter().enumerate() {
                    let owner = styled
                        .index
                        .char_to_utf16(placement.cluster.source.start)
                        .unwrap() as usize;
                    let top = case.placed_entries[owner].position_bits[1];
                    let position = line.text_position(index, true).unwrap();
                    assert_eq!(
                        line_origin + position.x,
                        f64::from(f32::from_bits(case.placed_entries[owner].position_bits[0])),
                        "{} paragraph {ordinal} line {line_number} owner {owner} X",
                        case.name
                    );
                    assert_eq!(placement.x, position.x);
                    positions_checked += 1;
                    assert_eq!(
                        *captured_top.get_or_insert(top),
                        top,
                        "{} paragraph {ordinal} line {line_number} owner {owner} placement {index}",
                        case.name
                    );
                }
                assert_eq!(
                    captured_top,
                    captured_line_tops.get(line_number).copied(),
                    "{} paragraph {ordinal} line {line_number}",
                    case.name
                );
                wrapper.commit(line.source.end);
                line_number += 1;
            }
            assert_eq!(
                line_number,
                captured_line_tops.len(),
                "{} paragraph {ordinal}",
                case.name
            );
            paragraphs += 1;
        }
    }
    assert_eq!(paragraphs, 10);
    assert_eq!(slots_checked, 29);
    assert_eq!(positions_checked, 28);
}

#[test]
fn table_layout_retains_captured_combining_alignment_before_world_translation() {
    use super::super::super::layout::{
        NativeCellTextConstraints, TextFrame, layout_flow_text, layout_table_cell_text,
    };
    let capture = capture();
    let case = capture
        .cases
        .iter()
        .find(|case| case.name == "combining-auto")
        .unwrap();
    let fonts = FontBook::default();
    let settings = TextSettings::resolved();
    let mut content = super::super::tests::text(&case.text_utf8);
    content.font_size = Some(17.0);
    content.paragraphs.push(crate::RichTextParagraph {
        kind: crate::RichTextParagraphType::Alignment,
        start_paragraph: 0,
        end_paragraph: 1,
        payload: 2_u32.to_le_bytes().to_vec(),
    });
    let before = content.clone();
    let styled = StyledText::new(&content, TextContext::Flow, settings);
    let renderer = TextRenderer::new(settings, &fonts);
    let layout = layout_table_cell_text(
        &styled,
        TextFrame {
            bbox: crate::BoundingBox {
                x_max: 100.0,
                y_max: 100.0,
                ..Default::default()
            },
            gravity: None,
            exclusions: &[],
        },
        NativeCellTextConstraints {
            width: 0,
            height_limit: f32::MAX,
        },
        RenderTheme::for_canvas(false),
        &renderer,
    )
    .unwrap();
    assert_eq!(layout.lines.len(), 1);
    let line = &layout.lines[0];
    assert_eq!(
        line.x,
        f64::from(f32::from_bits(case.placed_entries[0].position_bits[0]))
    );
    assert_eq!(line.alignment, None);
    for (index, placement) in line.line.placements.iter().enumerate() {
        let owner = styled
            .index
            .char_to_utf16(placement.cluster.source.start)
            .unwrap() as usize;
        let expected = f64::from(f32::from_bits(case.placed_entries[owner].position_bits[0]));
        assert_eq!(line.x + placement.x, expected);
        assert_eq!(
            line.x + line.line.text_position(index, true).unwrap().x,
            expected
        );
    }
    let generic = layout_flow_text(
        &styled,
        TextFrame {
            bbox: crate::BoundingBox {
                x_max: 22.0,
                y_max: 100.0,
                ..Default::default()
            },
            gravity: None,
            exclusions: &[],
        },
        RenderTheme::for_canvas(false),
        &renderer,
    );
    let generic = &generic.lines[0];
    assert_eq!(generic.alignment, Some(crate::ParagraphAlignment::Center));
    assert_eq!(generic.line.placements[0].x, 0.0);
    assert_eq!(
        super::super::super::line_alignment_offset(
            generic.line.advance,
            generic.width,
            generic.alignment
        ),
        f64::from(f32::from_bits(1_042_871_744))
    );
    assert_eq!(content, before);
}

#[test]
fn constrained_zero_width_keeps_emergency_wrapping_separate_from_automatic_measurement() {
    let fonts = FontBook::default();
    let settings = TextSettings::resolved();
    let mut content = super::super::tests::text("AV");
    content.font_size = Some(17.0);
    let styled = StyledText::new(&content, TextContext::Flow, settings);
    let renderer = TextRenderer::new(settings, &fonts);
    let mut wrapper = ParagraphWrapper::new(
        &styled,
        0..2,
        ParagraphMeasurementWidth::Constrained(0.0),
        RenderTheme::for_canvas(false),
        None,
        &renderer,
        ObjectMeasurementContext::Frame,
    )
    .unwrap();
    assert_eq!(wrapper.measurement_width(), 0.0);
    assert_eq!(
        wrapper.candidate(0.0, |_| {}).unwrap().unwrap().source,
        0..1
    );
    wrapper.commit(1);
    assert_eq!(
        wrapper.candidate(0.0, |_| {}).unwrap().unwrap().source,
        1..2
    );
}
