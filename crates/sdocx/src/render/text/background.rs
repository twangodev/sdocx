use std::ops::Range;

use crate::BoundingBox;
use crate::render::RenderTheme;
use crate::render::vector::{Paint, Rectangle, Scene, Styled, color_hex, decimal};
use crate::render::viewport::Viewport;
use rustybuzz::Direction;

use super::{SourceOwner, StyledText, TextBackground, TextLine};

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
) -> Vec<SourceOwner> {
    let (rectangles, issues) =
        line_backgrounds_for_paint(styled, line, theme, viewport, svg.retains_text());
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

#[cfg(test)]
fn line_backgrounds(
    styled: &StyledText<'_>,
    line: &TextLine,
    theme: RenderTheme,
    viewport: Option<Viewport>,
) -> (Vec<BackgroundRectangle>, Vec<SourceOwner>) {
    line_backgrounds_for_paint(styled, line, theme, viewport, false)
}

fn line_backgrounds_for_paint(
    styled: &StyledText<'_>,
    line: &TextLine,
    theme: RenderTheme,
    viewport: Option<Viewport>,
    canonical: bool,
) -> (Vec<BackgroundRectangle>, Vec<SourceOwner>) {
    let mut rectangles: Vec<BackgroundRectangle> = Vec::new();
    let mut issues = Vec::new();
    let background_at = |source| {
        styled
            .background_at(source, theme, canonical)
            .filter(|background| background.alpha != 0)
    };
    if !styled
        .segments(line.line.source.clone())
        .any(|segment| background_at(segment.start).is_some())
    {
        return (rectangles, issues);
    }
    let ranges = super::paint::text_ranges(&line.line);
    let visual_positioned =
        line.line.native_positioned || (canonical && line.line.advance_for_paint(true).is_some());
    let contexts = if visual_positioned {
        &[][..]
    } else {
        super::paint::paragraph_bidi_contexts(styled, line.line.source.start)
    };
    let reordered = !visual_positioned
        && (line
            .line
            .placements
            .iter()
            .any(|placement| placement.cluster.run.direction == Direction::RightToLeft)
            || contexts
                .get(contexts.partition_point(|range| range.end <= line.line.source.start))
                .is_some_and(|range| range.start < line.line.source.end));
    let advance = line
        .line
        .advance_for_paint(canonical)
        .unwrap_or(line.line.advance);
    let origin = line.x + super::line_alignment_offset(advance, line.width, line.alignment);
    for (index, placement) in line.line.objects.iter().enumerate() {
        let source = &placement.object.source;
        if canonical && placement.object.context == super::objects::ObjectMeasurementContext::Body {
            continue;
        }
        if styled.object_span(placement.object.span_index).is_none() {
            continue;
        }
        let Some(background) = background_at(source.start) else {
            continue;
        };
        let Some(bounds) =
            object_background_bounds(line, index, origin, canonical).filter(|_| !reordered)
        else {
            if viewport.is_none_or(|viewport| viewport.background_visible(line)) {
                issues.push(SourceOwner::Object(source.clone()));
            }
            continue;
        };
        if background_bounds_visible(bounds, viewport) {
            rectangles.push(BackgroundRectangle {
                source: source.clone(),
                bounds,
                background,
            });
        }
    }
    if line.line.placements.is_empty() || reordered {
        if viewport.is_none_or(|viewport| viewport.background_visible(line)) {
            for range in ranges {
                for segment in styled.segments(range) {
                    if background_at(segment.start).is_some() {
                        issues.push(SourceOwner::Text(segment));
                    }
                }
            }
        }
        return (rectangles, issues);
    }
    let placements = line
        .line
        .placements
        .iter()
        .enumerate()
        .filter(|_| !visual_positioned)
        .chain(
            line.line
                .visual_order
                .iter()
                .filter(|_| visual_positioned)
                .filter_map(|entry| match entry {
                    super::wrapping::LineEntry::Text(index) => line
                        .line
                        .placements
                        .get(*index)
                        .map(|placement| (*index, placement)),
                    super::wrapping::LineEntry::Object(_) => None,
                }),
        );
    for (index, placement) in placements {
        let Some(position) = line.line.text_position(index, canonical) else {
            continue;
        };
        let source = &placement.cluster.source;
        if source.is_empty()
            || !ranges
                .iter()
                .any(|range| range.start <= source.start && source.end <= range.end)
        {
            continue;
        }
        let left = origin + position.x;
        let right = left + placement.cluster.advance + position.extra_advance;
        let bounds = BoundingBox {
            x_min: left,
            y_min: line.background_top,
            x_max: right,
            y_max: line.bottom,
        };
        if !background_bounds_visible(bounds, viewport) {
            continue;
        }
        let background = background_at(source.start);
        if styled
            .segments(source.clone())
            .any(|segment| background_at(segment.start) != background)
        {
            issues.push(SourceOwner::Text(source.clone()));
            continue;
        }
        let Some(background) = background else {
            continue;
        };
        let run = &placement.cluster.run;
        if run.variable {
            issues.push(SourceOwner::Text(source.clone()));
            continue;
        }
        if let Some(previous) = rectangles.last_mut()
            && previous.background == background
            && (previous.source.end == source.start || source.end == previous.source.start)
            && previous.bounds.x_max == bounds.x_min
            && previous.bounds.y_min == bounds.y_min
            && previous.bounds.y_max == bounds.y_max
        {
            previous.source =
                previous.source.start.min(source.start)..previous.source.end.max(source.end);
            previous.bounds.x_max = bounds.x_max;
        } else {
            rectangles.push(BackgroundRectangle {
                source: source.clone(),
                bounds,
                background,
            });
        }
    }
    rectangles.sort_by_key(|rectangle| rectangle.source.start);
    (rectangles, issues)
}

fn object_background_bounds(
    line: &TextLine,
    index: usize,
    origin: f64,
    canonical: bool,
) -> Option<BoundingBox> {
    let placement = line.line.objects.get(index)?;
    let position = line.line.object_position(index, canonical)?;
    let left = origin + position.x - placement.object.left_margin;
    let width = if placement.object.inline {
        placement.object.advance()
    } else {
        placement.object.width()
    };
    Some(BoundingBox {
        x_min: left,
        y_min: line.background_top,
        x_max: left + width,
        y_max: line.bottom,
    })
}

fn background_bounds_visible(bounds: BoundingBox, viewport: Option<Viewport>) -> bool {
    [
        bounds.x_min,
        bounds.x_max,
        bounds.y_min,
        bounds.y_max,
        bounds.x_max - bounds.x_min,
        bounds.y_max - bounds.y_min,
    ]
    .into_iter()
    .all(f64::is_finite)
        && bounds.x_max > bounds.x_min
        && bounds.y_max > bounds.y_min
        && viewport.is_none_or(|viewport| viewport.intersects(bounds))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::fonts::FontBook;
    use crate::render::text::{TextContext, TextFrame, TextLayout, TextRenderer, TextSettings};
    use crate::render::vector::Svg;
    use crate::{RichTextBox, RichTextSpan, RichTextSpanType};

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

    fn image_text(source: &str, anchor: i32, argb: u32) -> RichTextBox {
        let mut content = text(
            source,
            vec![span(
                RichTextSpanType::BackgroundColor,
                argb,
                anchor as u32,
                anchor as u32 + 1,
            )],
        );
        content.object_spans.push(crate::RichTextObjectSpan {
            object_type: crate::ObjectType::Image,
            object_data: Vec::new(),
            content: Some(crate::RichTextObjectContent::Image(Box::new(
                crate::PlacedImage {
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
                },
            ))),
            text_index_utf16: anchor,
            layout_option: crate::ObjectSpanLayoutOption::Inline,
            layout_constraint: crate::ObjectSpanLayoutConstraint::Normal,
        });
        content
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
                assert_eq!(issue, &SourceOwner::Text(0..2));
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
        assert_eq!(issues[0], SourceOwner::Text(0..1));
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
                visual_rank: 1,
                prepared: None,
                object: super::super::objects::MeasuredObject {
                    context: super::super::objects::ObjectMeasurementContext::Frame,
                    source: 1..2,
                    span_index: 0,
                    bounds: BoundingBox {
                        x_min: 0.0,
                        y_min: 0.0,
                        x_max: 20.0,
                        y_max: 20.0,
                    },
                    height: 20.0,
                    width: 20.0,
                    advance: 20.0,
                    inline: true,
                    left_margin: 0.0,
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
    fn bidi_reordering_reports_unsupported_background_positions() {
        for (source, start, end) in [("\u{5d0}AB", 1, 3), ("\u{202e}(AB)\u{202c}", 1, 3)] {
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
    }

    #[test]
    fn safe_wrapped_embeddings_use_retained_background_bounds() {
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
        assert!(line.line.native_positioned);
        assert_eq!(rectangles.len(), 1);
        assert!(issues.is_empty());
    }

    #[test]
    fn covered_greek_and_cyrillic_use_retained_native_advances() {
        for (source, expected_advance) in [("λΩ", 54.84375), ("Жя", 65.54443359375)] {
            let text = text(
                source,
                vec![span(RichTextSpanType::BackgroundColor, 0x80ffff00, 0, 2)],
            );
            let styled = StyledText::new(&text, TextContext::Placed, TextSettings::default());
            let layout = measure(&text, 200.0);
            let line = &layout.lines[0];
            assert!(line.line.placements.iter().all(|placement| {
                placement.cluster.run.direction == Direction::LeftToRight
                    && placement.cluster.run.glyphs[placement.cluster.glyphs.clone()]
                        .iter()
                        .all(|glyph| glyph.id != 0)
            }));
            let (rectangles, issues) =
                line_backgrounds(&styled, line, RenderTheme::for_canvas(false), None);
            assert!(issues.is_empty());
            assert_eq!(rectangles.len(), 1);
            assert_eq!(rectangles[0].source, 0..2);
            close(rectangles[0].bounds.x_max, expected_advance);
        }
        let text = text(
            "λλ",
            vec![span(RichTextSpanType::BackgroundColor, 0xffff0000, 1, 2)],
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
        assert_eq!(rectangles.len(), 1);
        close(rectangles[0].bounds.x_min, 25.2685546875);
        close(rectangles[0].bounds.x_max, 50.185546875);
    }

    #[test]
    fn uncovered_ltr_background_keeps_advance_and_foreground_missing_glyph_diagnosis() {
        let text = text(
            "中",
            vec![span(RichTextSpanType::BackgroundColor, 0xffffffff, 0, 1)],
        );
        let fonts = FontBook::default();
        let renderer = TextRenderer::new(TextSettings::default(), &fonts);
        let styled = StyledText::new(&text, TextContext::Placed, renderer.settings);
        let layout = super::super::layout_text(
            &styled,
            TextFrame {
                bbox: BoundingBox {
                    x_min: 0.0,
                    y_min: 0.0,
                    x_max: 200.0,
                    y_max: 200.0,
                },
                gravity: Some(0),
                exclusions: &[],
            },
            RenderTheme::for_canvas(false),
            &renderer,
        );
        let line = &layout.lines[0];
        let mut scene = Scene::new(Svg::new());
        let issues = render_line_backgrounds(
            &mut scene,
            &styled,
            line,
            RenderTheme::for_canvas(false),
            None,
        );
        assert!(issues.is_empty());
        super::super::render_measured_line(
            &mut scene,
            &styled,
            &line.line,
            line.x,
            line.width,
            line.baseline,
            line.alignment,
            RenderTheme::for_canvas(false),
            line.predefined,
            &renderer,
        );
        assert!(
            renderer
                .diagnostics()
                .iter()
                .any(|issue| issue.kind == super::super::TextDiagnosticKind::MissingGlyphs)
        );
        let svg = scene.finish();
        let xml = roxmltree::Document::parse(&svg).unwrap();
        assert_eq!(
            xml.descendants()
                .filter(|node| node.has_tag_name("rect"))
                .count(),
            1
        );
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

    #[test]
    fn object_background_projection_matches_native_placed_entry_capture() {
        use sha2::{Digest, Sha256};

        let bytes = include_bytes!("../../../../../conformance/table-text-object-background.json");
        assert_eq!(
            format!("{:x}", Sha256::digest(bytes)),
            "adc0d81004f5b372cf3f6cddc6c34aade8d9728339bc25056a5494cf71b0e076"
        );
        let fixture: serde_json::Value = serde_json::from_slice(bytes).unwrap();
        let number = |case: &serde_json::Value, field: &str, index: usize| {
            case[field][index].as_f64().unwrap()
        };
        let mut compared = 0;
        for case in fixture["cases"].as_array().unwrap() {
            if case["object"] != true {
                continue;
            }
            let Some(command) = case["commands"].as_array().unwrap().first() else {
                continue;
            };
            let inline = case["inline"].as_bool().unwrap();
            let raw_width = f32::from_bits(case["width_bits"].as_u64().unwrap() as u32);
            let width = f64::from(if raw_width.is_nan() { 17.0 } else { raw_width });
            let left_margin = if inline {
                number(case, "context_margins", 0)
            } else {
                0.0
            };
            let mut wrapped = super::super::WrappedLine::unmeasured(0..1, 17.0);
            wrapped.objects.push(super::super::PositionedObject {
                object: super::super::objects::MeasuredObject {
                    source: 0..1,
                    span_index: 0,
                    context: super::super::objects::ObjectMeasurementContext::Frame,
                    bounds: BoundingBox::default(),
                    width,
                    advance: number(case, "measured_advance_height", 0),
                    height: number(case, "measured_advance_height", 1),
                    inline,
                    left_margin,
                    top_margin: 0.0,
                    bottom_margin: 0.0,
                    minimum_first_page_height: None,
                },
                x: number(case, "position", 0) + left_margin,
                visual_rank: 0,
                prepared: None,
            });
            let offset_y = number(case, "offset", 1);
            let line = TextLine {
                line: wrapped,
                x: 0.0,
                width: 100.0,
                baseline: 0.0,
                top: 0.0,
                background_top: case["initial_cursor"].as_f64().unwrap()
                    + case["line_top_margin"].as_f64().unwrap()
                    + offset_y,
                bottom: case["post_cursor"].as_f64().unwrap() + offset_y,
                post_cursor: 0.0,
                alignment: None,
                predefined: None,
                marker: None,
            };
            let actual =
                object_background_bounds(&line, 0, number(case, "offset", 0), false).unwrap();
            let actual = [actual.x_min, actual.y_min, actual.x_max, actual.y_max];
            for (axis, actual) in actual.into_iter().enumerate() {
                let expected = command["rect"][axis].as_f64().unwrap();
                let tolerance = 2.0 * f64::from(f32::EPSILON) * expected.abs().max(1.0);
                assert!(
                    (actual - expected).abs() <= tolerance,
                    "{} axis {axis}: {actual} != {expected}",
                    case["name"]
                );
            }
            compared += 1;
        }
        assert_eq!(compared, 36);
    }

    #[test]
    fn unavailable_object_background_geometry_keeps_its_source_owner() {
        let content = image_text("\u{202e}\u{fffc}\u{202c}", 1, 0xff12_3456);
        let settings = TextSettings::default();
        let styled = StyledText::new(&content, TextContext::Placed, settings);
        let mut wrapped = super::super::WrappedLine::unmeasured(0..3, 17.0);
        wrapped.geometry = super::super::wrapping::LineGeometry::Unavailable(
            super::super::bidi::BidiError::MixedEmbeddingLevels,
        );
        wrapped.objects.push(super::super::PositionedObject {
            object: styled.objects.in_range(0..3)[0].measured(
                settings,
                super::super::objects::ObjectMeasurementContext::Frame,
            ),
            x: 0.0,
            visual_rank: 0,
            prepared: None,
        });
        let line = TextLine {
            line: wrapped,
            x: 0.0,
            width: 100.0,
            baseline: 22.0,
            top: 0.0,
            background_top: 5.0,
            bottom: 45.0,
            post_cursor: 45.0,
            alignment: None,
            predefined: None,
            marker: None,
        };
        let theme = RenderTheme::for_canvas(false);
        let offscreen = Viewport::new(BoundingBox {
            x_min: 0.0,
            y_min: 80.0,
            x_max: 100.0,
            y_max: 100.0,
        });
        for canonical in [false, true] {
            let (rectangles, issues) =
                line_backgrounds_for_paint(&styled, &line, theme, None, canonical);
            assert!(rectangles.is_empty());
            assert_eq!(issues, [SourceOwner::Object(1..2)]);
            let (rectangles, issues) =
                line_backgrounds_for_paint(&styled, &line, theme, Some(offscreen), canonical);
            assert!(rectangles.is_empty());
            assert!(issues.is_empty());
        }
    }

    #[test]
    fn retained_object_backgrounds_follow_captured_legacy_export_callers() {
        use sha2::{Digest, Sha256};

        let bytes =
            include_bytes!("../../../../../conformance/table-text-object-export-policy.json");
        assert_eq!(
            format!("{:x}", Sha256::digest(bytes)),
            "63bca0a1f43a8a828c5127c4899d5ac63837e59d78ced56da34cedaf0520134c"
        );
        let fixture: serde_json::Value = serde_json::from_slice(bytes).unwrap();
        let theme = RenderTheme::for_canvas(false);
        let mut compared = 0;
        for case in fixture["cases"].as_array().unwrap() {
            let context = match case["window"].as_str().unwrap() {
                "body-writer-background" => super::super::objects::ObjectMeasurementContext::Body,
                "table-writer-background"
                | "code-title-background"
                | "code-body-background"
                | "placed-background" => super::super::objects::ObjectMeasurementContext::Frame,
                _ => continue,
            };
            if case["present"] != true || case["object_flag"] == 0 {
                continue;
            }
            let argb = u32::try_from(case["background_argb"].as_u64().unwrap()).unwrap();
            let content = image_text("\u{fffc}", 0, argb);
            let styled = StyledText::new(&content, TextContext::Placed, TextSettings::default());
            let mut layout = measure(&content, 200.0);
            let line = &mut layout.lines[0];
            line.line.objects[0].object.context = context;
            let (rectangles, issues) = line_backgrounds_for_paint(&styled, line, theme, None, true);
            let requested = case["calls"]
                .as_array()
                .unwrap()
                .iter()
                .any(|call| call == "background");
            assert_eq!(rectangles.len(), usize::from(requested && argb >> 24 != 0));
            assert!(issues.is_empty(), "{}", case["window"]);
            for rectangle in rectangles {
                assert_eq!(rectangle.source, 0..1);
                assert_eq!(rectangle.background.alpha, (argb >> 24) as u8);
                assert_eq!(rectangle.bounds.x_max - rectangle.bounds.x_min, 30.0);
            }
            compared += 1;
        }
        assert_eq!(compared, 40);
    }
}
