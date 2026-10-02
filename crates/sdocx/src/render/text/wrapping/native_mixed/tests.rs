use super::super::super::{TextContext, TextSettings};
use super::super::tests::{image, text};
use super::*;
use crate::ObjectSpanLayoutOption;
use crate::fonts::FontBook;

#[test]
fn long_mixed_paragraph_retains_only_each_selected_line_and_later_source_offsets() {
    for repeats in [32, 128] {
        let prefix = "😀\n";
        let paragraph = "AV\u{fffc}x ".repeat(repeats);
        let mut content = text(&format!("{prefix}{paragraph}"));
        content.font_size = Some(17.0);
        for index in 0..repeats {
            content.object_spans.push(image(
                (prefix.encode_utf16().count() + index * 5 + 2) as i32,
                4.0,
                ObjectSpanLayoutOption::Inline,
            ));
        }
        let settings = TextSettings::resolved();
        let styled = StyledText::new(&content, TextContext::Flow, settings);
        let fonts = FontBook::default();
        let renderer = TextRenderer::new(settings, &fonts);
        let start = prefix.chars().count();
        let mut wrapper = ParagraphWrapper::new(
            &styled,
            start..styled.index.len(),
            35.0,
            RenderTheme::for_canvas(false),
            None,
            &renderer,
            ObjectMeasurementContext::Frame,
        )
        .unwrap();
        let mut next = start;
        let mut retained_entries = 0;
        let mut retained_capacity = 0;
        let mut lines = 0;
        while let Some(mut line) = wrapper.candidate(35.0, |_| {}).unwrap() {
            assert_eq!(line.source.start, next);
            let mixed = line.native_mixed.as_ref().unwrap();
            let source = styled.index.source(line.source.clone()).unwrap();
            let units = (source.utf16().end - source.utf16().start) as usize;
            assert_eq!(mixed.slots.origin_utf16, source.utf16().start);
            assert_eq!(mixed.slots.entries.len(), units);
            assert_eq!(mixed.slots.objects.len(), units);
            assert_eq!(mixed.slots.block_objects.len(), units);
            assert_eq!(mixed.visual_scalars.len(), line.source.len());
            retained_entries += units;
            retained_capacity += mixed.slots.entries.capacity();
            let logical = line.source.clone();
            let origin = line
                .place_native_cell(&styled, 2.0, 35.0, Some(crate::ParagraphAlignment::Center))
                .unwrap()
                .unwrap();
            assert!(origin >= 2.0);
            assert_eq!(line.source, logical);
            for index in 0..line.objects.len() {
                let position = line.object_position(index, true).unwrap();
                assert!(origin + position.x >= origin);
                assert_eq!(line.objects[index].object.source.len(), 1);
                assert_eq!(
                    styled
                        .index
                        .slice(line.objects[index].object.source.clone()),
                    Some("\u{fffc}")
                );
            }
            next = line.source.end;
            wrapper.commit(next);
            lines += 1;
        }
        assert!(lines >= repeats);
        assert_eq!(next, styled.index.len());
        assert_eq!(retained_entries, paragraph.encode_utf16().count());
        assert!(retained_capacity <= 2 * retained_entries);
    }
}

#[test]
fn supplied_feedback_capture_drives_sdk_objects_with_exact_source_translation() {
    let capture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../../../conformance/table-text-object-feedback.json"
    ))
    .unwrap();
    let fonts = FontBook::default();
    let settings = TextSettings::resolved();
    let mut checked = 0;
    for case in capture["cases"].as_array().unwrap().iter().filter(|case| {
        case["route"] == "GetBlockInfo"
            && case["entry_before"]["over_pages"] == true
            && case["supplied_kinds"]
                .as_array()
                .unwrap()
                .iter()
                .all(|kind| matches!(kind.as_u64(), Some(0 | 1 | 2 | 5)))
    }) {
        let object = case["object_utf16"].as_u64().unwrap() as usize;
        let advances = case["supplied_advances_bits"].as_array().unwrap();
        let local = advances
            .iter()
            .enumerate()
            .map(|(index, _)| {
                if index == object {
                    '\u{fffc}'
                } else {
                    char::from_u32('A' as u32 + index as u32).unwrap()
                }
            })
            .collect::<String>();
        for prefix in ["", "😀\n"] {
            let start = prefix.chars().count();
            let mut content = text(&format!("{prefix}{local}"));
            content.font_size = Some(17.0);
            let anchor = prefix.encode_utf16().count() + object;
            content
                .object_spans
                .push(image(anchor as i32, 1.0, ObjectSpanLayoutOption::Inline));
            let styled = StyledText::new(&content, TextContext::Flow, settings);
            let renderer = TextRenderer::new(settings, &fonts);
            let end = styled.index.len();
            let full = f64::from(f32::from_bits(
                case["paragraph_full_width_bits"].as_u64().unwrap() as u32,
            ));
            let budget = f64::from(f32::from_bits(case["budget_bits"].as_u64().unwrap() as u32));
            let mut wrapper = ParagraphWrapper::new(
                &styled,
                start..end,
                full,
                RenderTheme::for_canvas(false),
                None,
                &renderer,
                ObjectMeasurementContext::Frame,
            )
            .unwrap();
            let mut slots =
                NativeMixedSlots::new(&styled, start..end, &wrapper.items, &wrapper.allowed)
                    .unwrap()
                    .unwrap();
            assert_eq!(slots.entries.len(), advances.len());
            for (index, entry) in slots.entries.iter_mut().enumerate() {
                entry.advance = f32::from_bits(advances[index].as_u64().unwrap() as u32);
                entry.break_end_utf16 =
                    Some(case["supplied_break_ends_utf16"][index].as_u64().unwrap() as usize);
            }
            let MeasuredItem::Object { placement, .. } = &mut wrapper.items[object] else {
                panic!("expected independent SDK object anchor")
            };
            placement.object.advance = f64::from(slots.entries[object].advance);
            for stage in case["stages"].as_array().unwrap() {
                let mut calls = Vec::new();
                let new =
                    f32::from_bits(stage["entry_after"]["advance_bits"].as_u64().unwrap() as u32);
                let selected = slots
                    .select(
                        &styled,
                        start..end,
                        [budget, full],
                        &mut wrapper.items,
                        &mut |placement| {
                            calls.push(
                                styled
                                    .index
                                    .char_to_utf16(placement.object.source.start)
                                    .unwrap() as usize,
                            );
                            placement.object.advance = f64::from(new);
                        },
                    )
                    .unwrap();
                let captured_end = stage["selection"]["range_utf16_inclusive"][1]
                    .as_i64()
                    .unwrap();
                assert_eq!(
                    selected.as_ref().map(|selection| selection.end),
                    (captured_end >= 0).then(|| start + captured_end as usize + 1),
                    "{} {} {prefix:?}",
                    case["name"],
                    stage["stage"]
                );
                if let Some(selection) = &selected {
                    assert_eq!(
                        selection.encountered_object_metric,
                        stage["selection"]["flags"][1] == 1,
                        "{} {} {prefix:?}",
                        case["name"],
                        stage["stage"]
                    );
                }
                assert_eq!(
                    calls.len(),
                    stage["callback_calls"].as_u64().unwrap() as usize
                );
                assert!(calls.iter().all(|&call| call == anchor));
                slots.entries[object].advance = new;
                if let Some(selected) = selected {
                    assert_eq!(
                        (slots.block_width(&styled, start..selected.end).unwrap() as f32).to_bits(),
                        stage["selection"]["layout_bits"][2].as_u64().unwrap() as u32
                    );
                }
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 54);
}

#[test]
fn production_mixed_slots_retain_combining_zeros_and_sdk_callback_state() {
    let mut content = text("x\u{327}\u{301}\u{fffc}A");
    content.font_size = Some(17.0);
    content
        .object_spans
        .push(image(3, 3.25, ObjectSpanLayoutOption::Inline));
    let settings = TextSettings::resolved();
    let styled = StyledText::new(&content, TextContext::Flow, settings);
    let fonts = FontBook::default();
    let renderer = TextRenderer::new(settings, &fonts);
    let mut wrapper = ParagraphWrapper::new(
        &styled,
        0..5,
        100.0,
        RenderTheme::for_canvas(false),
        None,
        &renderer,
        ObjectMeasurementContext::Frame,
    )
    .unwrap();
    let slots = NativeMixedSlots::new(&styled, 0..5, &wrapper.items, &wrapper.allowed)
        .unwrap()
        .unwrap();
    assert_eq!(slots.entries.len(), 5);
    assert_eq!(slots.entries[1].advance.to_bits(), 0);
    assert_eq!(slots.entries[2].advance.to_bits(), 0);
    let mut calls = 0;
    let line = wrapper
        .candidate(100.0, |placement| {
            calls += 1;
            placement.object.advance = 12.5;
        })
        .unwrap()
        .unwrap();
    assert_eq!(calls, 1);
    assert_eq!(line.source, 0..5);
    assert!(line.native_mixed.is_some());
    assert_eq!(line.objects[0].x, f64::from(slots.entries[0].advance));
    assert_eq!(
        line.placements[1].x,
        f64::from(slots.entries[0].advance + 12.5)
    );
    wrapper.restore(line);
    assert_eq!(wrapper.items[1].advance(), 12.5);
}

#[test]
fn narrowed_object_no_block_preserves_source_without_preparing_it() {
    let mut content = text("\u{fffc}");
    content.font_size = Some(17.0);
    content
        .object_spans
        .push(image(0, 40.0, ObjectSpanLayoutOption::Inline));
    let settings = TextSettings::resolved();
    let styled = StyledText::new(&content, TextContext::Flow, settings);
    let fonts = FontBook::default();
    let renderer = TextRenderer::new(settings, &fonts);
    let mut wrapper = ParagraphWrapper::new(
        &styled,
        0..1,
        100.0,
        RenderTheme::for_canvas(false),
        None,
        &renderer,
        ObjectMeasurementContext::Frame,
    )
    .unwrap();
    let line = wrapper
        .candidate(10.0, |_| {
            panic!("a narrowed available region suppresses first-object preparation")
        })
        .unwrap()
        .unwrap();
    assert_eq!(line.source, 0..1);
    assert_eq!(line.advance, 40.0);
    assert!(line.unsupported_native_wrapping());
    wrapper.commit(line.source.end);
    assert!(wrapper.candidate(10.0, |_| {}).unwrap().is_none());
}

#[test]
fn two_sdk_objects_keep_warm_mutations_and_separate_grouped_width_from_cursor() {
    let settings = TextSettings::resolved();
    let fonts = FontBook::default();
    for prefix in ["", "😀\n"] {
        let start = prefix.chars().count();
        let origin = prefix.encode_utf16().count();
        let mut content = text(&format!("{prefix}A\u{fffc}BC\u{fffc}"));
        content.font_size = Some(17.0);
        content.object_spans = vec![
            image((origin + 1) as i32, 1.0, ObjectSpanLayoutOption::Inline),
            image((origin + 4) as i32, 1.0, ObjectSpanLayoutOption::Inline),
        ];
        let styled = StyledText::new(&content, TextContext::Flow, settings);
        let renderer = TextRenderer::new(settings, &fonts);
        let base = 16_777_216.0_f32;
        let budget = f64::from(base + 6.0);
        let end = start + 5;
        let mut wrapper = ParagraphWrapper::new(
            &styled,
            start..end,
            budget,
            RenderTheme::for_canvas(false),
            None,
            &renderer,
            ObjectMeasurementContext::Frame,
        )
        .unwrap();
        for item in &mut wrapper.items {
            if let MeasuredItem::Object { placement, .. } = item {
                placement.object.advance = 0.0;
            }
        }
        let supplied_slots = |items: &[MeasuredItem]| {
            let mut slots = NativeMixedSlots::new(&styled, start..end, items, &wrapper.allowed)
                .unwrap()
                .unwrap();
            // Numeric entry inputs and placement are SDK policy, not captured native object output.
            for (slot, advance) in [(0, base), (2, 1.0), (3, 1.0)] {
                slots.entries[slot].advance = advance;
            }
            for (entry, end) in slots.entries.iter_mut().zip([1, 4, 4, 4, 5]) {
                entry.break_end_utf16 = Some(end);
            }
            slots
        };
        let slots = supplied_slots(&wrapper.items);
        let mut callbacks = Vec::new();
        let selected = slots
            .select(
                &styled,
                start..end,
                [budget, budget],
                &mut wrapper.items,
                &mut |placement| {
                    callbacks.push((
                        placement.object.span_index,
                        styled
                            .index
                            .char_to_utf16(placement.object.source.start)
                            .unwrap() as usize,
                        placement.x,
                    ));
                    placement.object.advance = if placement.object.span_index == 0 {
                        4.0
                    } else {
                        1.0
                    };
                },
            )
            .unwrap();
        assert_eq!(selected.map(|selection| selection.end), Some(end));
        assert_eq!(
            callbacks,
            [
                (0, origin + 1, f64::from(base)),
                (1, origin + 4, f64::from(base + 4.0))
            ]
        );
        let prepared = supplied_slots(&wrapper.items);
        let grouped = prepared.block_width(&styled, start..end).unwrap();
        assert_eq!(grouped, f64::from(base + 8.0));
        assert!(grouped > budget);

        let mut line = WrappedLine::unmeasured(start..end, 17.0);
        for item in &wrapper.items {
            match item {
                MeasuredItem::TextCluster(cluster) => {
                    line.visual_order
                        .push(LineEntry::Text(line.placements.len()));
                    line.placements.push(PositionedCluster {
                        cluster: cluster.clone(),
                        x: 0.0,
                        extra_advance: 0.0,
                        visual_rank: line.visual_order.len() - 1,
                    });
                }
                MeasuredItem::Object { placement, .. } => {
                    line.visual_order
                        .push(LineEntry::Object(line.objects.len()));
                    line.objects.push(PositionedObject {
                        object: placement.object.clone(),
                        x: 0.0,
                        visual_rank: line.visual_order.len() - 1,
                        prepared: None,
                    });
                }
            }
        }
        let geometry = prepared
            .positions(&styled, &line, &[0, 1, 2, 3, 4], &line.visual_order)
            .unwrap();
        let LineGeometry::Positioned {
            advance,
            ref objects,
            ..
        } = geometry
        else {
            panic!("expected physical slot positions")
        };
        assert_eq!(advance, f64::from(base + 4.0));
        assert_eq!(objects[1].x, f64::from(base + 4.0));
        assert_ne!(grouped, advance);
        line.advance = grouped;
        line.geometry = geometry;
        line.native_mixed = Some(
            NativeMixedLine::new(prepared, &styled, line.source.clone(), vec![0, 1, 2, 3, 4])
                .unwrap(),
        );
        assert_eq!(line.advance_for_paint(true), Some(grouped));

        let warm = supplied_slots(&wrapper.items);
        let mut warm_callbacks = Vec::new();
        assert_eq!(
            warm.select(
                &styled,
                start..end,
                [budget, budget],
                &mut wrapper.items,
                &mut |placement| {
                    warm_callbacks.push(placement.object.span_index);
                }
            )
            .unwrap()
            .map(|selection| selection.end),
            Some(start + 4)
        );
        assert_eq!(warm_callbacks, [0]);
        assert_eq!(
            supplied_slots(&wrapper.items)
                .block_width(&styled, start..start + 4)
                .unwrap(),
            f64::from(base + 6.0)
        );
        assert_eq!(wrapper.items[4].advance(), 1.0);
    }
}
