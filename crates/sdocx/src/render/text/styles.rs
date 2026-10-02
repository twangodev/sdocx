use std::{collections::BTreeSet, ops::Range, sync::Arc};

use crate::{RichTextHyperlink, RichTextRun, RichTextSpan, RichTextSpanType};

use super::{NativeIdentityUnavailable, TextSettings, TextSpanProducer};

#[derive(Clone, Copy)]
enum Property {
    FontSize,
    ValidFontSize,
    Foreground,
    Background,
    ObjectBackground,
    ComposingBackground,
    Family,
    Bold,
    Italic,
    Underline,
    Strikethrough,
    Hyperlink,
    Suggestion,
    UnderlineColor,
    Unavailable,
    ObjectUnavailable,
    RunBold,
    RunItalic,
    Object,
}

const SPAN_PROPERTIES: usize = Property::RunBold as usize;
const PROPERTIES: usize = Property::Object as usize + 1;

#[derive(Debug, PartialEq)]
pub(super) struct SelectedHyperlink<'a> {
    pub span: &'a RichTextSpan,
    pub value: Option<RichTextHyperlink>,
}

#[derive(Clone, Copy)]
struct FontSize {
    value: f32,
    valid: bool,
}

#[derive(Clone, Copy)]
struct BackgroundPatch {
    argb: u32,
    includes_objects: bool,
}

#[derive(Default)]
struct SpanPatch<'a> {
    font_size: Option<FontSize>,
    foreground: Option<u32>,
    background: Option<BackgroundPatch>,
    composing_background: Option<u32>,
    family: Option<Arc<str>>,
    bold: Option<bool>,
    italic: Option<bool>,
    underline: Option<bool>,
    strikethrough: Option<bool>,
    hyperlink: Option<Arc<SelectedHyperlink<'a>>>,
    suggestion: bool,
    underline_color: Option<u32>,
    unavailable: Option<NativeIdentityUnavailable>,
}

impl<'a> SpanPatch<'a> {
    fn decode(span: &'a RichTextSpan, settings: TextSettings) -> Self {
        let mut patch = Self::default();
        match span.kind {
            RichTextSpanType::FontSize => {
                patch.font_size = span.font_size_value().map(|value| FontSize {
                    value,
                    valid: settings.checked_font_size(value).is_some(),
                });
            }
            RichTextSpanType::ForegroundColor => patch.foreground = span.argb_value(),
            RichTextSpanType::BackgroundColor => {
                patch.background = span.argb_value().map(|argb| BackgroundPatch {
                    argb,
                    includes_objects: false,
                });
            }
            RichTextSpanType::ComposingBackgroundColor => {
                patch.composing_background = span.composing_background_value();
            }
            RichTextSpanType::FontName => {
                patch.family = span
                    .decoded_font_name_value()
                    .map(|name| Arc::from(name.as_ref()));
            }
            RichTextSpanType::Bold => patch.bold = span.boolean_value(),
            RichTextSpanType::Italic => patch.italic = span.boolean_value(),
            RichTextSpanType::Underline => patch.underline = span.boolean_value(),
            RichTextSpanType::Strikethrough => patch.strikethrough = span.boolean_value(),
            RichTextSpanType::Hyperlink => {
                patch.hyperlink = Some(Arc::new(SelectedHyperlink {
                    span,
                    value: span.hyperlink_value(),
                }));
            }
            RichTextSpanType::Composing if span.composition_value().is_some() => {
                patch.underline = Some(true);
            }
            RichTextSpanType::ComposingTag => match span.composition_value() {
                Some(true) => {
                    patch.background = Some(BackgroundPatch {
                        argb: 0x1925_2525,
                        includes_objects: true,
                    });
                }
                Some(false) => {
                    patch.bold = Some(true);
                    patch.italic = Some(true);
                    patch.underline = Some(true);
                }
                None => {}
            },
            RichTextSpanType::Suggestion => {
                if let Some(suggestion) = span.suggestion_value() {
                    patch.suggestion = true;
                    patch.underline_color = Some(suggestion.underline_argb);
                }
            }
            RichTextSpanType::SpellCorrection => {
                patch.unavailable = Some(NativeIdentityUnavailable::UnsupportedCorrection);
            }
            _ => {}
        }
        let decoded = match span.kind {
            RichTextSpanType::FontSize => patch.font_size.is_some(),
            RichTextSpanType::ForegroundColor => patch.foreground.is_some(),
            RichTextSpanType::BackgroundColor => patch.background.is_some(),
            RichTextSpanType::ComposingBackgroundColor => patch.composing_background.is_some(),
            RichTextSpanType::FontName => patch.family.is_some(),
            RichTextSpanType::Bold => patch.bold.is_some(),
            RichTextSpanType::Italic => patch.italic.is_some(),
            RichTextSpanType::Underline | RichTextSpanType::Composing => patch.underline.is_some(),
            RichTextSpanType::Strikethrough => patch.strikethrough.is_some(),
            RichTextSpanType::Hyperlink => patch
                .hyperlink
                .as_ref()
                .is_some_and(|link| link.value.is_some()),
            RichTextSpanType::ComposingTag => patch.background.is_some() || patch.bold.is_some(),
            RichTextSpanType::Suggestion => patch.suggestion,
            _ => true,
        };
        if !decoded {
            patch.unavailable = Some(NativeIdentityUnavailable::MalformedSpan(span.kind));
        }
        patch
    }

    fn unavailable_in(&self, widget_object: bool) -> Option<NativeIdentityUnavailable> {
        self.unavailable.filter(|reason| {
            !widget_object
                || !matches!(
                    reason,
                    NativeIdentityUnavailable::MalformedSpan(
                        RichTextSpanType::BackgroundColor
                            | RichTextSpanType::ComposingBackgroundColor
                    )
                )
        })
    }

    fn properties(&self) -> impl Iterator<Item = Property> {
        [
            (Property::FontSize, self.font_size.is_some()),
            (
                Property::ValidFontSize,
                self.font_size.is_some_and(|size| size.valid),
            ),
            (Property::Foreground, self.foreground.is_some()),
            (Property::Background, self.background.is_some()),
            (
                Property::ObjectBackground,
                self.background.is_some_and(|patch| patch.includes_objects),
            ),
            (
                Property::ComposingBackground,
                self.composing_background.is_some(),
            ),
            (Property::Family, self.family.is_some()),
            (Property::Bold, self.bold.is_some()),
            (Property::Italic, self.italic.is_some()),
            (Property::Underline, self.underline.is_some()),
            (Property::Strikethrough, self.strikethrough.is_some()),
            (Property::Hyperlink, self.hyperlink.is_some()),
            (Property::Suggestion, self.suggestion),
            (Property::UnderlineColor, self.underline_color.is_some()),
            (Property::Unavailable, self.unavailable.is_some()),
            (
                Property::ObjectUnavailable,
                self.unavailable_in(true).is_some(),
            ),
        ]
        .into_iter()
        .filter_map(|(property, written)| written.then_some(property))
    }

    fn apply(&self, selection: &mut StyleSelection<'a>, widget_object: bool) {
        if let Some(size) = self.font_size {
            selection.invalid_font = !size.valid;
            if size.valid {
                selection.font_size = Some(size.value);
            }
        }
        selection.foreground = self.foreground.or(selection.foreground);
        selection.background = self
            .background
            .filter(|patch| !widget_object || patch.includes_objects)
            .map(|patch| patch.argb)
            .or(selection.background);
        if !widget_object {
            selection.composing_background =
                self.composing_background.or(selection.composing_background);
        }
        selection.family = self.family.clone().or_else(|| selection.family.take());
        selection.bold = self.bold.unwrap_or(selection.bold);
        selection.italic = self.italic.unwrap_or(selection.italic);
        selection.underline = self.underline.or(selection.underline);
        selection.strikethrough = self.strikethrough.or(selection.strikethrough);
        selection.hyperlink = self
            .hyperlink
            .clone()
            .or_else(|| selection.hyperlink.take());
        selection.suggestion |= self.suggestion;
        selection.underline_color = self.underline_color.or(selection.underline_color);
        selection.unavailable = self.unavailable_in(widget_object).or(selection.unavailable);
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

#[derive(Debug, Default, Clone, PartialEq)]
pub(super) struct StyleSelection<'a> {
    pub font_size: Option<f32>,
    pub invalid_font: bool,
    pub bold: bool,
    pub italic: bool,
    pub foreground: Option<u32>,
    pub background: Option<u32>,
    pub composing_background: Option<u32>,
    pub family: Option<Arc<str>>,
    pub underline: Option<bool>,
    pub strikethrough: Option<bool>,
    pub hyperlink: Option<Arc<SelectedHyperlink<'a>>>,
    pub suggestion: bool,
    pub underline_color: Option<u32>,
    pub widget_object: bool,
    pub unavailable: Option<NativeIdentityUnavailable>,
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
        Self::with_objects(spans, runs, length, settings, [], TextSpanProducer::Drawing)
    }

    pub fn with_objects(
        spans: &[(Range<usize>, &'a RichTextSpan)],
        runs: &[RichTextRun],
        length: usize,
        settings: TextSettings,
        objects: impl IntoIterator<Item = Range<usize>>,
        producer: TextSpanProducer,
    ) -> Self {
        let patches = spans
            .iter()
            .map(|(_, span)| SpanPatch::decode(span, settings))
            .collect::<Vec<_>>();
        let mut events = Vec::new();
        for (order, ((range, _), patch)) in spans.iter().zip(&patches).enumerate() {
            for property in patch.properties() {
                events_for(&mut events, range, property, order);
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
        if producer == TextSpanProducer::Widget {
            for (order, range) in objects.into_iter().enumerate() {
                events_for(&mut events, &range, Property::Object, order);
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
            let widget_object = !active[Property::Object as usize].is_empty();
            let mut selection = StyleSelection {
                bold: !active[Property::RunBold as usize].is_empty(),
                italic: !active[Property::RunItalic as usize].is_empty(),
                widget_object,
                ..Default::default()
            };
            let mut writers: [Option<usize>; SPAN_PROPERTIES] = std::array::from_fn(|property| {
                if (widget_object
                    && (property == Property::Background as usize
                        || property == Property::ComposingBackground as usize
                        || property == Property::Unavailable as usize))
                    || (!widget_object
                        && (property == Property::ObjectBackground as usize
                            || property == Property::ObjectUnavailable as usize))
                {
                    None
                } else {
                    active[property].last().copied()
                }
            });
            writers.sort_unstable();
            let mut previous = None;
            for writer in writers.into_iter().flatten() {
                if previous != Some(writer) {
                    patches[writer].apply(&mut selection, widget_object);
                    previous = Some(writer);
                }
            }
            selections.push(selection);
        }
        Self {
            positions,
            selections,
        }
    }

    pub fn at(&self, character: usize) -> &StyleSelection<'a> {
        let position = self.positions.partition_point(|&start| start <= character) - 1;
        &self.selections[position]
    }
}

#[cfg(test)]
pub(super) mod tests {
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
            RichTextSpanType::ComposingBackgroundColor,
            RichTextSpanType::Composing,
            RichTextSpanType::ComposingTag,
            RichTextSpanType::Suggestion,
        ];
        let mut seed = 93_u32;
        let mut owned = Vec::new();
        let mut ranges = Vec::new();
        for count in 0..512 {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let start = (seed % 31) as usize;
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let end = start + (seed as usize % (32 - start)) + 1;
            let mut payload = match count % 7 {
                0 => Vec::new(),
                1 => f32::NAN.to_le_bytes().to_vec(),
                2 => f32::INFINITY.to_le_bytes().to_vec(),
                3 => 1_u16.to_le_bytes().to_vec(),
                4 => 0_u16.to_le_bytes().to_vec(),
                _ => (count as f32 + 1.0).to_le_bytes().to_vec(),
            };
            let kind = kinds[count % kinds.len()];
            if count % 7 != 0 {
                match kind {
                    RichTextSpanType::ComposingBackgroundColor
                    | RichTextSpanType::Composing
                    | RichTextSpanType::ComposingTag => payload.resize(8, 0),
                    RichTextSpanType::Suggestion | RichTextSpanType::Hyperlink => {
                        payload.resize(12, 0);
                    }
                    _ => {}
                }
            }
            owned.push(span(kind, &payload));
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
        let objects = [1..3, 6..11, 10..12, 16..20];
        for producer in [TextSpanProducer::Drawing, TextSpanProducer::Widget] {
            let index =
                StyleIndex::with_objects(&spans, &runs, 32, settings, objects.clone(), producer);
            for character in 0..=33 {
                let mut expected = StyleSelection {
                    bold: (4..12).contains(&character),
                    italic: (6..12).contains(&character),
                    ..Default::default()
                };
                let widget_object = producer == TextSpanProducer::Widget
                    && objects.iter().any(|range| range.contains(&character));
                expected.widget_object = widget_object;
                for (range, span) in &spans {
                    if range.contains(&character) {
                        SpanPatch::decode(span, settings).apply(&mut expected, widget_object);
                    }
                }
                assert_eq!(index.at(character), &expected, "character {character}");
            }
        }
    }

    #[test]
    fn widget_objects_keep_eligible_background_writers_in_source_order() {
        let owned = [
            span(RichTextSpanType::ComposingTag, &[1, 0, 0, 0, 0, 0, 0, 0]),
            span(
                RichTextSpanType::BackgroundColor,
                &0xffff_0000_u32.to_le_bytes(),
            ),
            span(
                RichTextSpanType::ComposingBackgroundColor,
                &[0xff00_00ff_u32.to_le_bytes(), [0; 4]].concat(),
            ),
            span(RichTextSpanType::BackgroundColor, &[0; 4]),
            span(RichTextSpanType::ComposingTag, &[0; 8]),
            span(RichTextSpanType::Underline, &[0; 2]),
        ];
        let spans = [0..5, 0..5, 0..5, 2..4, 1..4, 2..3]
            .into_iter()
            .zip(&owned)
            .collect::<Vec<_>>();
        for producer in [TextSpanProducer::Drawing, TextSpanProducer::Widget] {
            let index = StyleIndex::with_objects(
                &spans,
                &[],
                5,
                TextSettings::default(),
                [1..2, 3..4],
                producer,
            );
            for character in 0..5 {
                let widget_object =
                    producer == TextSpanProducer::Widget && matches!(character, 1 | 3);
                assert_eq!(
                    index.at(character).background,
                    Some(if widget_object {
                        0x1925_2525
                    } else if (2..4).contains(&character) {
                        0
                    } else {
                        0xffff_0000
                    })
                );
                assert_eq!(
                    index.at(character).composing_background,
                    (!widget_object).then_some(0xff00_00ff)
                );
                assert_eq!(index.at(character).bold, (1..4).contains(&character));
                assert_eq!(index.at(character).italic, (1..4).contains(&character));
                assert_eq!(
                    index.at(character).underline,
                    if character == 2 {
                        Some(false)
                    } else if (1..4).contains(&character) {
                        Some(true)
                    } else {
                        None
                    }
                );
            }
            assert_eq!(index.at(5), &StyleSelection::default());
        }
    }

    #[test]
    fn widget_object_guards_match_captured_span_conversion() {
        const FIXTURE: &str =
            include_str!("../../../../../conformance/table-text-span-identity.json");
        let capture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
        let case = capture["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|case| case["name"] == "object-skips-background")
            .unwrap();
        for side in ["left", "right"] {
            let spans = case[format!("{side}_inputs")]
                .as_array()
                .unwrap()
                .iter()
                .filter(|input| input["kind"] != "object")
                .map(|input| captured_attribute(input).unwrap())
                .collect::<Vec<_>>();
            let indexed = spans.iter().map(|span| (0..1, span)).collect::<Vec<_>>();
            let index = StyleIndex::with_objects(
                &indexed,
                &[],
                1,
                TextSettings::default(),
                std::iter::once(0..1),
                TextSpanProducer::Widget,
            );
            let expected = &case[side];
            assert_eq!(
                index.at(0).background.unwrap_or(0),
                u32::try_from(expected["background"].as_u64().unwrap()).unwrap()
            );
            assert_eq!(
                index.at(0).composing_background.unwrap_or(0),
                u32::try_from(expected["composing_background"].as_u64().unwrap()).unwrap()
            );
        }
    }

    #[test]
    fn boundary_snapshots_are_linear_in_span_and_run_count() {
        let owned = (0..50_000)
            .map(|_| span(RichTextSpanType::FontSize, &10_f32.to_le_bytes()))
            .collect::<Vec<_>>();
        let spans = owned
            .iter()
            .enumerate()
            .map(|(i, span)| (i..i + 1, span))
            .collect::<Vec<_>>();
        let index = StyleIndex::new(&spans, &[], 50_000, TextSettings::default());
        assert_eq!(index.positions.len(), 50_001);
        assert_eq!(index.selections.len(), index.positions.len());
        for character in 0..50_000 {
            assert_eq!(index.at(character).font_size, Some(10.0));
            assert!(!index.at(character).invalid_font);
        }
        assert_eq!(index.at(50_000).font_size, None);
    }

    #[test]
    fn nested_spans_restore_previous_writers_at_fifty_thousand_depth() {
        let count = 50_000;
        let owned = (0..count)
            .map(|order| {
                span(
                    RichTextSpanType::FontSize,
                    &(10.0 + (order % 50) as f32).to_le_bytes(),
                )
            })
            .collect::<Vec<_>>();
        let spans = owned
            .iter()
            .enumerate()
            .map(|(order, span)| (order..2 * count - order, span))
            .collect::<Vec<_>>();
        let index = StyleIndex::new(&spans, &[], 2 * count, TextSettings::default());
        assert_eq!(index.positions.len(), 2 * count);
        assert_eq!(index.selections.len(), index.positions.len());
        for character in 0..2 * count {
            let writer = character.min(2 * count - character - 1);
            assert_eq!(
                index.at(character).font_size,
                Some(10.0 + (writer % 50) as f32),
                "character {character}"
            );
        }
        assert_eq!(index.at(2 * count), &StyleSelection::default());
    }

    pub(in crate::render::text) fn captured_attribute(
        value: &serde_json::Value,
    ) -> Option<RichTextSpan> {
        let word = |key: &str| u32::try_from(value[key].as_u64().unwrap()).unwrap();
        let (kind, payload) = match value["kind"].as_str().unwrap() {
            "foreground" => (
                RichTextSpanType::ForegroundColor,
                word("color").to_le_bytes().to_vec(),
            ),
            "background" => (
                RichTextSpanType::BackgroundColor,
                word("color").to_le_bytes().to_vec(),
            ),
            "composing_background" => (
                RichTextSpanType::ComposingBackgroundColor,
                [word("color").to_le_bytes(), [0; 4]].concat(),
            ),
            "font_size" => (
                RichTextSpanType::FontSize,
                (value["size"].as_f64().unwrap() as f32)
                    .to_le_bytes()
                    .to_vec(),
            ),
            "font_name" => {
                let name = value["name"].as_str()?;
                let mut payload = vec![0; 8];
                payload.extend_from_slice(&u16::try_from(name.len() + 1).unwrap().to_le_bytes());
                payload.extend_from_slice(name.as_bytes());
                payload.push(0);
                (RichTextSpanType::FontName, payload)
            }
            "toggle" => (
                RichTextSpanType::from(word("type")),
                u16::from(value["enabled"].as_bool().unwrap())
                    .to_le_bytes()
                    .to_vec(),
            ),
            "hyperlink" => (
                RichTextSpanType::Hyperlink,
                [word("type").to_le_bytes(), [0; 4], [0; 4]].concat(),
            ),
            "composing_tag" => (
                RichTextSpanType::ComposingTag,
                vec![
                    u8::from(value["background_enabled"].as_bool().unwrap()),
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                ],
            ),
            "composing" => (RichTextSpanType::Composing, vec![0; 8]),
            "suggestion" => (
                RichTextSpanType::Suggestion,
                [[0; 4], word("underline").to_le_bytes(), [0; 4]].concat(),
            ),
            "ignored" => (RichTextSpanType::from(word("type")), Vec::new()),
            _ => return None,
        };
        Some(span(kind, &payload))
    }

    #[test]
    fn typed_patches_match_captured_native_span_fields() {
        use sha2::Digest;

        const FIXTURE: &str =
            include_str!("../../../../../conformance/table-text-span-identity.json");
        assert_eq!(
            format!("{:x}", sha2::Sha256::digest(FIXTURE.as_bytes())),
            "42f53231b1f764cd66325609a13995e84964cb4d8b3978922b1b77fb289710f7"
        );
        let capture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
        let mut checked = 0;
        for case in capture["cases"].as_array().unwrap() {
            if case["theme_xor"].as_u64() != Some(0)
                || case["object_defaults"].as_bool() != Some(true)
            {
                continue;
            }
            let sides = ["left", "right"].map(|side| {
                case[format!("{side}_inputs")]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(captured_attribute)
                    .collect::<Option<Vec<_>>>()
            });
            let [Some(left), Some(right)] = sides else {
                continue;
            };
            let settings = TextSettings {
                font_size_delta: case["font_size_delta"].as_f64().unwrap() as f32,
                scale: case["scale"].as_f64().unwrap() as f32,
                ..Default::default()
            };
            for (side, spans) in [("left", left), ("right", right)] {
                let indexed = spans.iter().map(|span| (0..1, span)).collect::<Vec<_>>();
                let index = StyleIndex::new(&indexed, &[], 1, settings);
                let selection = index.at(0);
                let expected = &case[side];
                let field = |name: &str| u32::try_from(expected[name].as_u64().unwrap()).unwrap();
                let font_size = selection
                    .font_size
                    .map(|size| settings.font_size(size))
                    .unwrap_or(f64::from(
                        (17.0_f32 + settings.font_size_delta) * settings.scale,
                    ));
                assert_eq!(
                    font_size,
                    expected["font_size"].as_f64().unwrap(),
                    "{} {side}",
                    case["name"]
                );
                assert_eq!(
                    selection.foreground.unwrap_or(0xff26_2626),
                    field("foreground")
                );
                assert_eq!(selection.background.unwrap_or(0), field("background"));
                assert_eq!(
                    selection.composing_background.unwrap_or(0),
                    field("composing_background")
                );
                assert_eq!(selection.family.as_deref(), expected["font_name"].as_str());
                let style = u32::from(selection.bold)
                    | (u32::from(selection.italic) << 1)
                    | (u32::from(selection.underline.unwrap_or(false)) << 2)
                    | (u32::from(selection.strikethrough.unwrap_or(false)) << 3)
                    | (u32::from(selection.suggestion) << 4);
                assert_eq!(style, field("style"), "{} {side}", case["name"]);
                assert_eq!(
                    selection.underline_color.unwrap_or(0xff00_0000),
                    field("underline")
                );
                let hypertext = selection
                    .hyperlink
                    .as_ref()
                    .and_then(|link| link.value.as_ref())
                    .is_some_and(|value| value.kind.is_hypertext());
                assert_eq!(u32::from(hypertext), field("flags"));
                assert_eq!(index.at(1), &StyleSelection::default());
            }
            checked += 1;
        }
        assert_eq!(checked, 54);
    }
}
