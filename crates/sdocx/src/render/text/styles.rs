use std::{collections::BTreeSet, ops::Range};

use crate::{RichTextRun, RichTextSpan, RichTextSpanType};

use super::TextSettings;

#[derive(Clone, Copy)]
enum Property {
    FontSize,
    Foreground,
    Background,
    Family,
    Bold,
    Italic,
    Underline,
    Strikethrough,
    Hyperlink,
    ValidFontSize,
    RunBold,
    RunItalic,
}

const SPAN_PROPERTIES: usize = Property::ValidFontSize as usize;
const PROPERTIES: usize = Property::RunItalic as usize + 1;

impl Property {
    fn from_span(span: &RichTextSpan) -> Option<Self> {
        match span.kind {
            RichTextSpanType::FontSize => span.font_size_value().map(|_| Self::FontSize),
            RichTextSpanType::ForegroundColor => span.color_value().map(|_| Self::Foreground),
            RichTextSpanType::BackgroundColor => span.argb_value().map(|_| Self::Background),
            RichTextSpanType::FontName => span.decoded_font_name_value().map(|_| Self::Family),
            RichTextSpanType::Bold => span.boolean_value().map(|_| Self::Bold),
            RichTextSpanType::Italic => span.boolean_value().map(|_| Self::Italic),
            RichTextSpanType::Underline => span.boolean_value().map(|_| Self::Underline),
            RichTextSpanType::Strikethrough => span.boolean_value().map(|_| Self::Strikethrough),
            RichTextSpanType::Hyperlink => Some(Self::Hyperlink),
            _ => None,
        }
    }
}

struct Event {
    position: usize,
    property: Property,
    order: usize,
    entering: bool,
}

fn events_for(events: &mut Vec<Event>, range: &Range<usize>, property: Property, order: usize) {
    events.push(Event {
        position: range.start,
        property,
        order,
        entering: true,
    });
    events.push(Event {
        position: range.end,
        property,
        order,
        entering: false,
    });
}

#[derive(Clone, Copy)]
pub(super) struct StyleSelection<'a> {
    pub spans: [Option<&'a RichTextSpan>; SPAN_PROPERTIES],
    pub font_size: Option<f32>,
    pub invalid_font: bool,
    pub bold: bool,
    pub italic: bool,
}

pub(super) struct StyleIndex<'a> {
    positions: Vec<usize>,
    selections: Vec<StyleSelection<'a>>,
}

impl<'a> StyleIndex<'a> {
    pub fn new(
        spans: &[(Range<usize>, &'a RichTextSpan)],
        runs: &[RichTextRun],
        length: usize,
        settings: TextSettings,
    ) -> Self {
        let mut events = Vec::new();
        for (order, (range, span)) in spans.iter().enumerate() {
            let Some(property) = Property::from_span(span) else {
                continue;
            };
            events_for(&mut events, range, property, order);
            if matches!(property, Property::FontSize)
                && span
                    .font_size_value()
                    .is_some_and(|size| settings.checked_font_size(size).is_some())
            {
                events_for(&mut events, range, Property::ValidFontSize, order);
            }
        }
        for (order, run) in runs.iter().enumerate() {
            if run.start >= run.end || run.end > length {
                continue;
            }
            if run.bold {
                events_for(&mut events, &(run.start..run.end), Property::RunBold, order);
            }
            if run.italic {
                events_for(
                    &mut events,
                    &(run.start..run.end),
                    Property::RunItalic,
                    order,
                );
            }
        }
        events.sort_unstable_by_key(|event| event.position);
        let mut positions = vec![0, length];
        positions.extend(events.iter().map(|event| event.position));
        positions.sort_unstable();
        positions.dedup();
        let mut active: [BTreeSet<usize>; PROPERTIES] = std::array::from_fn(|_| BTreeSet::new());
        let mut selections = Vec::with_capacity(positions.len());
        let mut next_event = 0;
        for &position in &positions {
            while let Some(event) = events
                .get(next_event)
                .filter(|event| event.position == position)
            {
                let set = &mut active[event.property as usize];
                if event.entering {
                    set.insert(event.order);
                } else {
                    set.remove(&event.order);
                }
                next_event += 1;
            }
            let latest_font = active[Property::FontSize as usize].last();
            let valid_font = active[Property::ValidFontSize as usize].last();
            selections.push(StyleSelection {
                spans: std::array::from_fn(|property| {
                    active[property].last().map(|&order| spans[order].1)
                }),
                font_size: valid_font.and_then(|&order| spans[order].1.font_size_value()),
                invalid_font: latest_font > valid_font,
                bold: !active[Property::RunBold as usize].is_empty(),
                italic: !active[Property::RunItalic as usize].is_empty(),
            });
        }
        Self {
            positions,
            selections,
        }
    }

    pub fn at(&self, character: usize) -> StyleSelection<'a> {
        let position = self.positions.partition_point(|&start| start <= character) - 1;
        self.selections[position]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SpanIntervalType;

    fn span(kind: RichTextSpanType, payload: &[u8]) -> RichTextSpan {
        RichTextSpan {
            kind,
            start_utf16: 0,
            end_utf16: 1,
            interval_type: SpanIntervalType::ClosedOpen,
            payload: payload.to_vec(),
        }
    }

    #[test]
    fn event_sweep_matches_source_order_with_overlapping_properties() {
        let kinds = [
            RichTextSpanType::FontSize,
            RichTextSpanType::ForegroundColor,
            RichTextSpanType::BackgroundColor,
            RichTextSpanType::Bold,
            RichTextSpanType::Italic,
            RichTextSpanType::Underline,
            RichTextSpanType::Strikethrough,
            RichTextSpanType::Hyperlink,
        ];
        let mut seed = 93_u32;
        let mut owned = Vec::new();
        let mut ranges = Vec::new();
        for count in 0..512 {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let start = (seed % 31) as usize;
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let end = start + (seed as usize % (32 - start)) + 1;
            let payload = match count % 7 {
                0 => Vec::new(),
                1 => f32::NAN.to_le_bytes().to_vec(),
                2 => f32::INFINITY.to_le_bytes().to_vec(),
                3 => 1_u16.to_le_bytes().to_vec(),
                4 => 0_u16.to_le_bytes().to_vec(),
                _ => (count as f32 + 1.0).to_le_bytes().to_vec(),
            };
            owned.push(span(kinds[count % kinds.len()], &payload));
            ranges.push(start..end);
        }
        let spans = ranges.into_iter().zip(&owned).collect::<Vec<_>>();
        let runs = vec![
            RichTextRun {
                start: 4,
                end: 8,
                bold: true,
                italic: false,
            },
            RichTextRun {
                start: 6,
                end: 12,
                bold: true,
                italic: true,
            },
            RichTextRun {
                start: 0,
                end: 33,
                bold: true,
                italic: true,
            },
            RichTextRun {
                start: 20,
                end: 20,
                bold: true,
                italic: true,
            },
        ];
        let settings = TextSettings::default();
        let index = StyleIndex::new(&spans, &runs, 32, settings);
        for character in 0..=33 {
            let selected = index.at(character);
            for property in 0..SPAN_PROPERTIES {
                let expected = spans.iter().rev().find_map(|(range, span)| {
                    (range.contains(&character)
                        && Property::from_span(span).is_some_and(|kind| kind as usize == property))
                    .then_some(*span)
                });
                assert_eq!(
                    selected.spans[property], expected,
                    "character {character}, property {property}"
                );
            }
            let valid_font = spans.iter().rev().find_map(|(range, span)| {
                (range.contains(&character) && span.kind == RichTextSpanType::FontSize)
                    .then(|| span.font_size_value())
                    .flatten()
                    .filter(|size| settings.checked_font_size(*size).is_some())
            });
            assert_eq!(selected.font_size, valid_font);
            let last_font = selected.spans[Property::FontSize as usize];
            assert_eq!(
                selected.invalid_font,
                last_font.is_some_and(|span| settings
                    .checked_font_size(span.font_size_value().unwrap())
                    .is_none()),
            );
            assert_eq!(selected.bold, (4..12).contains(&character));
            assert_eq!(selected.italic, (6..12).contains(&character));
        }
    }

    #[test]
    fn boundary_snapshots_are_linear_in_span_and_run_count() {
        let owned = (0..10_000)
            .map(|_| span(RichTextSpanType::FontSize, &10_f32.to_le_bytes()))
            .collect::<Vec<_>>();
        let spans = owned
            .iter()
            .enumerate()
            .map(|(i, span)| (i..i + 1, span))
            .collect::<Vec<_>>();
        let index = StyleIndex::new(&spans, &[], 10_000, TextSettings::default());
        assert_eq!(index.positions.len(), 10_001);
        assert_eq!(index.selections.len(), index.positions.len());
        for character in 0..10_000 {
            assert_eq!(index.at(character).font_size, Some(10.0));
            assert!(!index.at(character).invalid_font);
        }
        assert_eq!(index.at(10_000).font_size, None);
    }
}
