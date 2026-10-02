use std::sync::Arc;

use crate::{Color, RichTextSpanType};

use super::{RenderTheme, TextMeasureStyle};

#[derive(Debug, Clone, PartialEq)]
pub(in crate::render) struct NativeDrawSpan {
    pub font_size: f32,
    pub foreground: u32,
    pub background: u32,
    pub composing_background: u32,
    pub style_bits: u8,
    pub family: Option<Arc<str>>,
    pub underline: u32,
    pub correction_foreground: u32,
    pub flags: u8,
    pub correction_foreground_enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::render) enum NativeIdentityUnavailable {
    UnsupportedCorrection,
    MalformedSpan(RichTextSpanType),
    InvalidFontMetrics,
}

impl NativeDrawSpan {
    pub(super) fn measurement_style(&self) -> TextMeasureStyle {
        TextMeasureStyle {
            font_size: self.font_size,
            foreground: self.foreground,
            family: self.family.clone(),
            style_bits: self.style_bits & 0xc3,
        }
    }
}

pub(super) fn mapped_argb(argb: u32, theme: RenderTheme) -> u32 {
    let mapped = theme.span_color(Color {
        r: (argb >> 16) as u8,
        g: (argb >> 8) as u8,
        b: argb as u8,
    });
    argb & 0xff00_0000 | u32::from(mapped.r) << 16 | u32::from(mapped.g) << 8 | u32::from(mapped.b)
}

#[cfg(test)]
mod tests {
    use super::super::{StyledText, TextContext, TextSettings, TextSpanProducer};
    use super::*;
    use crate::{
        BoundingBox, ObjectSpanLayoutConstraint, ObjectSpanLayoutOption, ObjectType, PlacedImage,
        RichTextObjectContent, RichTextObjectSpan, RichTextSpan, SpanIntervalType,
    };

    fn object_span() -> RichTextObjectSpan {
        RichTextObjectSpan {
            object_type: ObjectType::Image,
            object_data: Vec::new(),
            content: Some(RichTextObjectContent::Image(Box::new(PlacedImage {
                bbox: BoundingBox {
                    x_min: 0.0,
                    y_min: 0.0,
                    x_max: 30.0,
                    y_max: 40.0,
                },
                rotation_degrees: None,
                media_id: None,
                media_index: None,
                crop_rect: None,
                original_bbox: None,
                border_media_id: None,
                original_media_id: None,
            }))),
            text_index_utf16: 0,
            layout_option: ObjectSpanLayoutOption::Inline,
            layout_constraint: ObjectSpanLayoutConstraint::Normal,
        }
    }

    fn raw_span(kind: RichTextSpanType, payload: Vec<u8>) -> RichTextSpan {
        RichTextSpan {
            kind,
            start_utf16: 0,
            end_utf16: 1,
            interval_type: SpanIntervalType::ClosedOpen,
            payload,
        }
    }

    fn assert_native_fields(actual: &NativeDrawSpan, expected: &serde_json::Value) {
        let word = |key| u32::try_from(expected[key].as_u64().unwrap()).unwrap();
        assert_eq!(
            actual.font_size,
            expected["font_size"].as_f64().unwrap() as f32
        );
        assert_eq!(actual.foreground, word("foreground"));
        assert_eq!(actual.background, word("background"));
        assert_eq!(actual.composing_background, word("composing_background"));
        assert_eq!(u32::from(actual.style_bits), word("style"));
        assert_eq!(actual.family.as_deref(), expected["font_name"].as_str());
        assert_eq!(actual.underline, word("underline"));
        assert_eq!(actual.correction_foreground, word("correction_foreground"));
        assert_eq!(u32::from(actual.flags), word("flags"));
        assert_eq!(
            actual.correction_foreground_enabled,
            expected["correction_enabled"].as_bool().unwrap()
        );
    }

    #[test]
    fn native_draw_projection_checks_all_captured_span_pairs_with_explicit_limits() {
        use sha2::Digest;

        const FIXTURE: &str =
            include_str!("../../../../../conformance/table-text-span-identity.json");
        assert_eq!(
            format!("{:x}", sha2::Sha256::digest(FIXTURE.as_bytes())),
            "42f53231b1f764cd66325609a13995e84964cb4d8b3978922b1b77fb289710f7"
        );
        let capture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
        let mut compared = 0;
        let mut unavailable = 0;
        let mut constructors = 0;
        for case in capture["cases"].as_array().unwrap() {
            if case["object_defaults"] == false {
                let constructor = NativeDrawSpan {
                    font_size: 17.0,
                    foreground: 0xff00_0000,
                    background: 0,
                    composing_background: 0,
                    style_bits: 0,
                    family: None,
                    underline: 0xff00_0000,
                    correction_foreground: 0xff00_0000,
                    flags: 0,
                    correction_foreground_enabled: false,
                };
                for side in ["left", "right"] {
                    assert_native_fields(&constructor, &case[side]);
                }
                let content = super::super::tests::text_box();
                let object_default =
                    StyledText::new(&content, TextContext::Flow, TextSettings::default())
                        .resolved_style_at(0, RenderTheme::for_canvas(false), None)
                        .native_draw
                        .unwrap();
                assert_ne!(constructor.foreground, object_default.foreground);
                constructors += 1;
                continue;
            }
            let settings = TextSettings {
                font_size_delta: case["font_size_delta"].as_f64().unwrap() as f32,
                scale: case["scale"].as_f64().unwrap() as f32,
                ..Default::default()
            };
            let theme = RenderTheme::for_canvas(case["theme_xor"].as_u64().unwrap() != 0);
            let identities = ["left", "right"].map(|side| {
                let mut content = super::super::tests::text_box();
                content.text = "A".into();
                let mut is_object = false;
                for input in case[format!("{side}_inputs")].as_array().unwrap() {
                    match input["kind"].as_str().unwrap() {
                        "object" => is_object = input["enabled"].as_bool().unwrap(),
                        "inline" | "over_pages" => {}
                        "spell_correction" => content
                            .spans
                            .push(raw_span(RichTextSpanType::SpellCorrection, Vec::new())),
                        "font_name" if input["name"].is_null() => content
                            .spans
                            .push(raw_span(RichTextSpanType::FontName, Vec::new())),
                        _ => content
                            .spans
                            .push(super::super::styles::tests::captured_attribute(input).unwrap()),
                    }
                }
                if is_object {
                    content.text = "\u{fffc}".into();
                    content.object_spans.push(object_span());
                }
                StyledText::new(&content, TextContext::Flow, settings)
                    .resolved_style_at(0, theme, None)
                    .native_draw
            });
            for (side, identity) in ["left", "right"].into_iter().zip(&identities) {
                match identity {
                    Ok(identity) => assert_native_fields(identity, &case[side]),
                    Err(reason) => match case["name"].as_str().unwrap() {
                        "negative-clamp" => {
                            assert_eq!(*reason, NativeIdentityUnavailable::InvalidFontMetrics)
                        }
                        "font-null-and-default" | "font-null-overwrite" => assert_eq!(
                            *reason,
                            NativeIdentityUnavailable::MalformedSpan(RichTextSpanType::FontName)
                        ),
                        name if name.starts_with("correction")
                            || name == "supplied-theme-conversion" =>
                        {
                            assert_eq!(*reason, NativeIdentityUnavailable::UnsupportedCorrection)
                        }
                        name => panic!("unexpected unavailable identity in {name}: {reason:?}"),
                    },
                }
            }
            if let [Ok(left), Ok(right)] = &identities {
                let equal = !case["different"].as_bool().unwrap();
                assert_eq!(left == right, equal, "{} forward", case["name"]);
                assert_eq!(right == left, equal, "{} reverse", case["name"]);
                compared += 1;
            } else {
                unavailable += 1;
            }
        }
        assert_eq!((compared, unavailable, constructors), (59, 10, 1));
    }

    #[test]
    fn fractional_unclamped_native_defaults_keep_recovery_but_have_no_exact_identity() {
        let mut content = super::super::tests::text_box();
        let settings = TextSettings {
            font_size_delta: -16.75,
            ..Default::default()
        };
        let theme = RenderTheme::for_canvas(false);
        let styled = StyledText::new(&content, TextContext::Flow, settings);
        let recovered = styled.resolved_style_at(0, theme, None);
        assert_eq!(recovered.paint.font_size, 1.0);
        assert_eq!(recovered.measurement.font_size, 1.0);
        assert_eq!(
            recovered.native_draw,
            Err(NativeIdentityUnavailable::InvalidFontMetrics)
        );
        content.spans.push(raw_span(
            RichTextSpanType::FontSize,
            17.0_f32.to_le_bytes().to_vec(),
        ));
        let explicit = StyledText::new(&content, TextContext::Flow, settings)
            .resolved_style_at(0, theme, None);
        assert_eq!(explicit.native_draw.unwrap().font_size, 1.0);
    }

    #[test]
    fn unavailable_source_ranges_and_widget_object_guards_restore_identity() {
        let mut content = super::super::tests::text_box();
        content.text = "\u{fffc}A".into();
        content.object_spans.push(object_span());
        content.spans.push(raw_span(
            RichTextSpanType::ComposingBackgroundColor,
            Vec::new(),
        ));
        for producer in [TextSpanProducer::Widget, TextSpanProducer::Drawing] {
            let styled = StyledText::with_span_producer(
                &content,
                TextContext::Flow,
                TextSettings::default(),
                producer,
            );
            let first = styled
                .resolved_style_at(0, RenderTheme::for_canvas(false), None)
                .native_draw;
            assert_eq!(first.is_ok(), producer == TextSpanProducer::Widget);
            assert!(
                styled
                    .resolved_style_at(1, RenderTheme::for_canvas(false), None)
                    .native_draw
                    .is_ok()
            );
        }
        content.spans[0] = raw_span(RichTextSpanType::SpellCorrection, Vec::new());
        let styled = StyledText::new(&content, TextContext::Flow, TextSettings::default());
        assert_eq!(
            styled
                .resolved_style_at(0, RenderTheme::for_canvas(false), None)
                .native_draw,
            Err(NativeIdentityUnavailable::UnsupportedCorrection)
        );
        assert!(
            styled
                .resolved_style_at(1, RenderTheme::for_canvas(false), None)
                .native_draw
                .is_ok()
        );
        content.spans[0].end_utf16 = 0;
        let styled = StyledText::new(&content, TextContext::Flow, TextSettings::default());
        assert!(
            styled
                .resolved_style_at(0, RenderTheme::for_canvas(false), None)
                .native_draw
                .is_ok()
        );
    }
}
