use std::ops::Range;

use crate::render::RenderTheme;
use crate::render::vector::{Paint, Rectangle, Scene, Styled, color_hex, decimal};
use crate::render::viewport::Viewport;
use crate::{BoundingBox, RichTextSpanType};
use rustybuzz::Direction;
use unicode_script::Script;

use super::{StyledText, TextBackground, TextLine};

pub(in crate::render) struct TextBackgroundIssue {
    pub source: Range<usize>,
}

struct BackgroundRectangle {
    source: Range<usize>,
    bounds: BoundingBox,
    background: TextBackground,
}

pub(in crate::render) fn render_line_backgrounds(
    svg: &mut Scene,
    styled: &StyledText<'_>,
    line: &TextLine,
    theme: RenderTheme,
    viewport: Option<Viewport>,
) -> Vec<TextBackgroundIssue> {
    let (rectangles, issues) = line_backgrounds(styled, line, theme, viewport);
    for rectangle in rectangles {
        let bounds = rectangle.bounds;
        svg.push(
            Rectangle::new()
                .x(decimal(bounds.x_min, 5))
                .y(decimal(bounds.y_min, 5))
                .width(decimal(bounds.x_max - bounds.x_min, 5))
                .height(decimal(bounds.y_max - bounds.y_min, 5))
                .fill(Paint::from_hex(&color_hex(&rectangle.background.color)))
                .fill_opacity(decimal(f64::from(rectangle.background.alpha) / 255.0, 6)),
        );
    }
    issues
}

fn line_backgrounds(
    styled: &StyledText<'_>,
    line: &TextLine,
    theme: RenderTheme,
    viewport: Option<Viewport>,
) -> (Vec<BackgroundRectangle>, Vec<TextBackgroundIssue>) {
    let mut rectangles: Vec<BackgroundRectangle> = Vec::new();
    let mut issues = Vec::new();
    if !styled.spans.iter().any(|(range, span)| {
        range.start < line.line.source.end
            && range.end > line.line.source.start
            && span.kind == RichTextSpanType::BackgroundColor
            && span.argb_value().is_some_and(|argb| argb >> 24 != 0)
    }) {
        return (rectangles, issues);
    }
    let ranges = super::paint::text_ranges(&line.line);
    let background_at = |source| {
        styled
            .style_at(source, theme, line.predefined)
            .background
            .filter(|background| background.alpha != 0)
    };
    let paragraph = styled
        .index
        .paragraph_index(line.line.source.start)
        .and_then(|ordinal| styled.index.native_paragraphs().nth(ordinal as usize));
    let contexts = paragraph
        .and_then(|paragraph| {
            styled
                .index
                .slice(paragraph.content.clone())
                .map(|text| super::paint::bidi_contexts(text, paragraph.content.start))
        })
        .unwrap_or_default();
    let reordered = line
        .line
        .placements
        .iter()
        .any(|placement| placement.cluster.run.direction == Direction::RightToLeft)
        || contexts
            .iter()
            .any(|range| range.start < line.line.source.end && range.end > line.line.source.start);
    if line.line.placements.is_empty() || reordered {
        if viewport.is_none_or(|viewport| viewport.background_visible(line)) {
            for range in ranges {
                for segment in styled.segments(range) {
                    if background_at(segment.start).is_some() {
                        issues.push(TextBackgroundIssue { source: segment });
                    }
                }
            }
        }
        return (rectangles, issues);
    }
    let origin =
        line.x + super::line_alignment_offset(line.line.advance, line.width, line.alignment);
    for placement in &line.line.placements {
        let source = &placement.cluster.source;
        if source.is_empty()
            || !ranges
                .iter()
                .any(|range| range.start <= source.start && source.end <= range.end)
        {
            continue;
        }
        let left = origin + placement.x;
        let right = left + placement.cluster.advance + placement.extra_advance;
        let bounds = BoundingBox {
            x_min: left,
            y_min: line.background_top,
            x_max: right,
            y_max: line.bottom,
        };
        if ![
            left,
            right,
            bounds.y_min,
            bounds.y_max,
            right - left,
            bounds.y_max - bounds.y_min,
        ]
        .into_iter()
        .all(f64::is_finite)
            || right <= left
            || bounds.y_max <= bounds.y_min
            || viewport.is_some_and(|viewport| !viewport.intersects(bounds))
        {
            continue;
        }
        let background = background_at(source.start);
        if styled
            .segments(source.clone())
            .any(|segment| background_at(segment.start) != background)
        {
            issues.push(TextBackgroundIssue {
                source: source.clone(),
            });
            continue;
        }
        let Some(background) = background else {
            continue;
        };
        let run = &placement.cluster.run;
        if run.variable
            || !matches!(
                run.script,
                Script::Latin | Script::Common | Script::Inherited
            )
        {
            issues.push(TextBackgroundIssue {
                source: source.clone(),
            });
            continue;
        }
        if let Some(previous) = rectangles.last_mut()
            && previous.background == background
            && previous.source.end == source.start
            && previous.bounds.x_max == bounds.x_min
            && previous.bounds.y_min == bounds.y_min
            && previous.bounds.y_max == bounds.y_max
        {
            previous.source.end = source.end;
            previous.bounds.x_max = bounds.x_max;
        } else {
            rectangles.push(BackgroundRectangle {
                source: source.clone(),
                bounds,
                background,
            });
        }
    }
    (rectangles, issues)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::fonts::FontBook;
    use crate::render::text::{TextContext, TextFrame, TextLayout, TextRenderer, TextSettings};
    use crate::render::vector::Svg;
    use crate::{RichTextBox, RichTextSpan};

    fn span(kind: RichTextSpanType, argb: u32, start: u32, end: u32) -> RichTextSpan {
        RichTextSpan {
            kind,
            start_utf16: start,
            end_utf16: end,
            interval_type: crate::SpanIntervalType::from(0),
            payload: argb.to_le_bytes().to_vec(),
        }
    }

    fn text(source: &str, spans: Vec<RichTextSpan>) -> RichTextBox {
        RichTextBox {
            text_area_type: None,
            bbox: Default::default(),
            rotation_degrees: None,
            text: source.into(),
            color: None,
            highlight_color: None,
            underline: false,
            font_size: Some(45.0),
            runs: Vec::new(),
            spans,
            paragraphs: Vec::new(),
            object_spans: Vec::new(),
            text_sections: Vec::new(),
            margins: None,
            gravity: None,
        }
    }

    fn measure(text: &RichTextBox, width: f64) -> TextLayout {
        let fonts = FontBook::default();
        let renderer = TextRenderer::new(TextSettings::default(), &fonts);
        let styled = StyledText::new(text, TextContext::Placed, renderer.settings);
        super::super::layout_text(
            &styled,
            TextFrame {
                bbox: BoundingBox {
                    x_min: 0.0,
                    y_min: 0.0,
                    x_max: width,
                    y_max: 200.0,
                },
                gravity: Some(0),
                exclusions: &[],
            },
            RenderTheme::for_canvas(false),
            &renderer,
        )
    }

    fn close(actual: f64, expected: f64) {
        assert!((actual - expected).abs() < 1e-8, "{actual} != {expected}");
    }

    #[test]
    fn background_uses_retained_kerning_and_merges_across_foreground_changes() {
        let theme = RenderTheme::for_canvas(false);
        let mut text = text(
            "AV",
            vec![span(RichTextSpanType::BackgroundColor, 0x80ffff00, 0, 2)],
        );
        let styled = StyledText::new(&text, TextContext::Placed, TextSettings::default());
        let layout = measure(&text, 200.0);
        let line = &layout.lines[0];
        let (rectangles, issues) = line_backgrounds(&styled, line, theme, None);
        assert!(issues.is_empty());
        assert_eq!(rectangles.len(), 1);
        assert_eq!(rectangles[0].source, 0..2);
        close(rectangles[0].bounds.x_max, 56.07421875);
        close(rectangles[0].bounds.y_max, 60.75);
        let mut scene = Scene::new(Svg::new());
        assert!(render_line_backgrounds(&mut scene, &styled, line, theme, None).is_empty());
        let svg = scene.finish();
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let rect = xml
            .descendants()
            .find(|node| node.has_tag_name("rect"))
            .unwrap();
        assert_eq!(rect.attribute("fill"), Some("#ffff00"));
        assert_eq!(rect.attribute("fill-opacity"), Some("0.501961"));
        text.spans
            .push(span(RichTextSpanType::ForegroundColor, 0xff00ff00, 1, 2));
        let styled = StyledText::new(&text, TextContext::Placed, TextSettings::default());
        let layout = measure(&text, 200.0);
        let (rectangles, issues) = line_backgrounds(&styled, &layout.lines[0], theme, None);
        assert!(issues.is_empty());
        assert_eq!(rectangles.len(), 1);
        close(rectangles[0].bounds.x_max, 57.98583984375);
    }

    #[test]
    fn transparent_last_override_leaves_an_unpainted_source_gap() {
        let text = text(
            "ABC",
            vec![
                span(RichTextSpanType::BackgroundColor, 0x80ffff00, 0, 3),
                span(RichTextSpanType::BackgroundColor, 0x00123456, 1, 2),
            ],
        );
        let styled = StyledText::new(&text, TextContext::Placed, TextSettings::default());
        let layout = measure(&text, 200.0);
        let (rectangles, issues) = line_backgrounds(
            &styled,
            &layout.lines[0],
            RenderTheme::for_canvas(false),
            None,
        );
        assert!(issues.is_empty());
        assert_eq!(rectangles.len(), 2);
        assert_eq!(rectangles[0].source, 0..1);
        assert_eq!(rectangles[1].source, 2..3);
        close(rectangles[0].bounds.x_max, 29.35546875);
        close(rectangles[1].bounds.x_min, 57.37060546875);
        close(rectangles[1].bounds.x_max, 86.66015625);
    }

    #[test]
    fn combining_cluster_requires_one_effective_background() {
        for (start, expected_rectangles, expected_issues) in [(0, 1, 0), (1, 0, 1)] {
            let text = text(
                "e\u{301}",
                vec![span(
                    RichTextSpanType::BackgroundColor,
                    0xffff0000,
                    start,
                    2,
                )],
            );
            let styled = StyledText::new(&text, TextContext::Placed, TextSettings::default());
            let layout = measure(&text, 200.0);
            assert_eq!(layout.lines[0].line.placements.len(), 1);
            let (rectangles, issues) = line_backgrounds(
                &styled,
                &layout.lines[0],
                RenderTheme::for_canvas(false),
                None,
            );
            assert_eq!(rectangles.len(), expected_rectangles);
            assert_eq!(issues.len(), expected_issues);
            if let Some(rectangle) = rectangles.first() {
                close(rectangle.bounds.x_max, 23.84033203125);
            }
            if let Some(issue) = issues.first() {
                assert_eq!(issue.source, 0..2);
            }
        }
    }

    #[test]
    fn background_visibility_is_independent_of_glyph_ink_and_raw_margin_top() {
        let text = text(
            "A",
            vec![span(RichTextSpanType::BackgroundColor, 0xffffff00, 0, 1)],
        );
        let styled = StyledText::new(&text, TextContext::Placed, TextSettings::default());
        let mut layout = measure(&text, 200.0);
        let line = &mut layout.lines[0];
        line.background_top = 30.0;
        let viewport = Viewport::new(BoundingBox {
            x_min: 0.0,
            y_min: 50.0,
            x_max: 100.0,
            y_max: 60.0,
        });
        let theme = RenderTheme::for_canvas(false);
        assert!(!viewport.text_visible(&styled, line, theme));
        let (rectangles, issues) = line_backgrounds(&styled, line, theme, Some(viewport));
        assert!(issues.is_empty());
        assert_eq!(rectangles.len(), 1);
        assert_eq!(rectangles[0].bounds.y_min, 30.0);
        line.line.placements.clear();
        let margin = Viewport::new(BoundingBox {
            x_min: 0.0,
            y_min: 1.0,
            x_max: 100.0,
            y_max: 20.0,
        });
        assert!(margin.line_visible(line));
        assert!(!margin.background_visible(line));
        let (rectangles, issues) = line_backgrounds(&styled, line, theme, Some(margin));
        assert!(rectangles.is_empty());
        assert!(issues.is_empty());
        let (rectangles, issues) = line_backgrounds(&styled, line, theme, Some(viewport));
        assert!(rectangles.is_empty());
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].source, 0..1);
    }

    #[test]
    fn overlapping_translucent_blocks_are_not_unioned() {
        let text = text(
            "AA",
            vec![span(RichTextSpanType::BackgroundColor, 0x80ffffff, 0, 2)],
        );
        let styled = StyledText::new(&text, TextContext::Placed, TextSettings::default());
        let mut layout = measure(&text, 200.0);
        layout.lines[0].line.placements[1].x -= 1.0;
        let (rectangles, issues) = line_backgrounds(
            &styled,
            &layout.lines[0],
            RenderTheme::for_canvas(false),
            None,
        );
        assert!(issues.is_empty());
        assert_eq!(rectangles.len(), 2);
        assert!(rectangles[1].bounds.x_min < rectangles[0].bounds.x_max);
    }

    #[test]
    fn valid_object_source_is_excluded_from_background_rectangles() {
        let text = text(
            "A\u{fffc}C",
            vec![span(RichTextSpanType::BackgroundColor, 0xffffffff, 0, 3)],
        );
        let styled = StyledText::new(&text, TextContext::Placed, TextSettings::default());
        let mut layout = measure(&text, 200.0);
        layout.lines[0]
            .line
            .objects
            .push(super::super::wrapping::PositionedObject {
                x: 29.35546875,
                prepared: None,
                object: super::super::objects::MeasuredObject {
                    source: 1..2,
                    span_index: 0,
                    bounds: BoundingBox {
                        x_min: 0.0,
                        y_min: 0.0,
                        x_max: 20.0,
                        y_max: 20.0,
                    },
                    height: 20.0,
                    inline: true,
                    top_margin: 0.0,
                    bottom_margin: 0.0,
                    minimum_first_page_height: None,
                },
            });
        let (rectangles, issues) = line_backgrounds(
            &styled,
            &layout.lines[0],
            RenderTheme::for_canvas(false),
            None,
        );
        assert!(issues.is_empty());
        assert_eq!(rectangles.len(), 2);
        assert_eq!(rectangles[0].source, 0..1);
        assert_eq!(rectangles[1].source, 2..3);
    }

    #[test]
    fn bidi_reordering_and_complex_scripts_report_unsupported_background_positions() {
        for (source, start, end) in [
            ("\u{5d0}AB", 1, 3),
            ("\u{202e}AB\u{202c}", 1, 3),
            ("λ", 0, 1),
        ] {
            let text = text(
                source,
                vec![span(
                    RichTextSpanType::BackgroundColor,
                    0xffff0000,
                    start,
                    end,
                )],
            );
            let styled = StyledText::new(&text, TextContext::Placed, TextSettings::default());
            let layout = measure(&text, 200.0);
            let (rectangles, issues) = line_backgrounds(
                &styled,
                &layout.lines[0],
                RenderTheme::for_canvas(false),
                None,
            );
            assert!(rectangles.is_empty());
            assert!(!issues.is_empty());
        }
        let text = text(
            "\u{202a}AAAA\u{202c}",
            vec![span(RichTextSpanType::BackgroundColor, 0xffff0000, 0, 6)],
        );
        let styled = StyledText::new(&text, TextContext::Placed, TextSettings::default());
        let layout = measure(&text, 35.0);
        let line = layout
            .lines
            .iter()
            .find(|line| styled.index.slice(line.line.source.clone()) == Some("A"))
            .unwrap();
        assert!(
            line.line
                .placements
                .iter()
                .all(|placement| placement.cluster.run.direction == Direction::LeftToRight)
        );
        let (rectangles, issues) =
            line_backgrounds(&styled, line, RenderTheme::for_canvas(false), None);
        assert!(rectangles.is_empty());
        assert!(!issues.is_empty());
    }

    #[test]
    fn invalid_or_zero_area_retained_geometry_never_emits_a_rectangle() {
        let text = text(
            "A",
            vec![span(RichTextSpanType::BackgroundColor, 0xffffffff, 0, 1)],
        );
        let styled = StyledText::new(&text, TextContext::Placed, TextSettings::default());
        let mut layout = measure(&text, 200.0);
        for advance in [0.0, -1.0, f64::INFINITY, f64::NAN] {
            layout.lines[0].line.placements[0].cluster.advance = advance;
            let (rectangles, issues) = line_backgrounds(
                &styled,
                &layout.lines[0],
                RenderTheme::for_canvas(false),
                None,
            );
            assert!(rectangles.is_empty());
            assert!(issues.is_empty());
        }
    }
}
