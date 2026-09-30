use std::ops::Range;

use crate::render::RenderTheme;
use crate::{
    BoundingBox, BulletType, ParagraphAlignment, ParagraphBullet, ParagraphLineSpacing,
    PredefinedTextStyle,
};

use super::{
    StyledText, TextRenderer, WrappedLine, paragraph_layout, paragraph_line_height,
    unmeasured_paragraph, wrap_paragraph,
};

#[derive(Clone, Copy)]
pub(in crate::render) struct VerticalExclusion {
    /// Absolute vertical coordinates in the same space as the frame's bounding box.
    pub top: f64,
    pub bottom: f64,
}

pub(in crate::render) struct ParagraphSpacing {
    pub before: bool,
    pub after: bool,
}

impl ParagraphSpacing {
    pub fn for_lines(
        lines: &[WrappedLine],
        previous: Option<ParagraphBullet>,
        current: Option<ParagraphBullet>,
        next: Option<ParagraphBullet>,
    ) -> Self {
        let first_margin_object = lines.first().is_some_and(|line| {
            line.objects.iter().any(|placement| {
                placement.object.source.start == line.source.start
                    && placement.object.top_margin > 0.0
                    && placement.object.bottom_margin > 0.0
            })
        });
        let last_margin_object = lines.last().is_some_and(|line| {
            line.objects.iter().any(|placement| {
                placement.object.source.end == line.source.end
                    && placement.object.top_margin > 0.0
                    && placement.object.bottom_margin > 0.0
            })
        });
        Self {
            before: !(spacing_bullet(previous) && spacing_bullet(current)) && !first_margin_object,
            after: !(spacing_bullet(current) && spacing_bullet(next)) && !last_margin_object,
        }
    }
}

fn spacing_bullet(bullet: Option<ParagraphBullet>) -> bool {
    matches!(
        bullet.map(|bullet| bullet.kind),
        Some(
            BulletType::Arrow
                | BulletType::Checker
                | BulletType::Diamond
                | BulletType::Digit
                | BulletType::CircledDigit
                | BulletType::Alphabet
                | BulletType::RomanNumeral
                | BulletType::SolidCircle
                | BulletType::WhiteCircle
                | BulletType::UppercaseAlphabet
                | BulletType::BlackSquare
                | BulletType::WhiteSquare
        )
    )
}

pub(in crate::render) struct TextFrame<'a> {
    pub bbox: BoundingBox,
    pub gravity: Option<u8>,
    pub exclusions: &'a [VerticalExclusion],
}

impl TextFrame<'_> {
    fn line_top(&self, mut top: f64, advance: f64) -> f64 {
        while let Some(band) = self.exclusions.iter().find(|band| {
            band.top.is_finite()
                && band.bottom.is_finite()
                && band.top < band.bottom
                && top < band.bottom - 0.0001
                && top + advance > band.top + 0.0001
        }) {
            top = band.bottom;
        }
        top
    }
}

pub(in crate::render) struct TextCursor {
    position: f64,
    pending_bottom: f64,
}

impl TextCursor {
    pub fn new(position: f64) -> Self {
        Self {
            position,
            pending_bottom: 0.0,
        }
    }

    pub fn add_spacing(&mut self, spacing: f64) {
        self.position += spacing;
    }

    pub fn position(&self) -> f64 {
        self.position
    }

    pub fn height(&self) -> f64 {
        self.position() + self.pending_bottom
    }

    pub fn height_with_bottom(&self, bottom: f64) -> f64 {
        let bottom = bottom.max(0.0);
        if bottom <= self.pending_bottom {
            self.height()
        } else {
            self.position() + bottom
        }
    }

    pub fn candidate_top(&self, line: &WrappedLine, frame: &TextFrame<'_>) -> f64 {
        frame.bbox.y_min + self.position + self.pending_bottom.max(line.object_margins()[0])
    }

    pub fn place(
        &mut self,
        line: &WrappedLine,
        spacing: Option<ParagraphLineSpacing>,
        frame: &TextFrame<'_>,
        settings: super::TextSettings,
    ) -> f64 {
        let [top_margin, bottom_margin] = line.object_margins();
        self.position += self.pending_bottom.max(top_margin);
        self.pending_bottom = bottom_margin;
        let has_objects = !line.objects.is_empty();
        let block = line.has_block_margins();
        let base_height = line.font_size.max(line.object_height());
        let advance = if block {
            base_height
        } else if has_objects {
            base_height + paragraph_line_height(line.font_size, spacing, settings) - line.font_size
        } else {
            paragraph_line_height(line.font_size, spacing, settings)
        };
        let candidate_top = frame.bbox.y_min + self.position;
        let top = frame.line_top(candidate_top, advance);
        let offset = if block {
            base_height + 0.001
        } else if has_objects {
            advance - 0.35 * line.font_size + 0.001
        } else {
            advance - 0.35 * line.font_size
        };
        let epsilon = if has_objects { 0.001 } else { 0.0 };
        self.position += top - candidate_top + advance + epsilon;
        top + offset
    }
}

pub(in crate::render) fn prepare_line_objects(
    line: &mut WrappedLine,
    styled: &StyledText<'_>,
    candidate_top: f64,
    theme: RenderTheme,
    renderer: &TextRenderer<'_>,
) {
    for placement in &mut line.objects {
        let Some(span) = styled.object_span(placement.object.span_index) else {
            continue;
        };
        let Some(crate::RichTextObjectContent::CodeBlock(code)) = span.content.as_ref() else {
            continue;
        };
        let prepared = crate::render::code::prepare_code(
            code,
            span.layout_constraint,
            candidate_top,
            theme,
            renderer,
        );
        match &prepared {
            Ok(prepared) => {
                let height = prepared.panel_bbox.y_max - prepared.panel_bbox.y_min;
                if matches!(
                    span.layout_constraint,
                    crate::ObjectSpanLayoutConstraint::OverPages
                        | crate::ObjectSpanLayoutConstraint::OverPagesOverlapPadding
                ) && (height - placement.object.height).abs() > 0.001
                {
                    placement.object.height = height;
                }
            }
            Err(kind) => renderer.report_object_issues(&[super::ObjectDiagnostic {
                anchor_utf16: span.text_index_utf16,
                kind: *kind,
            }]),
        }
        placement.prepared_code = Some(prepared.map(Box::new));
    }
}

pub(in crate::render) struct TextLine {
    pub line: WrappedLine,
    pub x: f64,
    pub width: f64,
    pub baseline: f64,
    pub alignment: Option<ParagraphAlignment>,
    pub predefined: Option<PredefinedTextStyle>,
}

pub(in crate::render) struct TextLayout {
    pub lines: Vec<TextLine>,
    content_height: f64,
}

impl TextLayout {
    pub fn height(&self) -> f64 {
        self.content_height
    }

    fn apply_gravity(&mut self, gravity: Option<u8>, outer_height: f64) {
        let available_height = (outer_height - self.height()).max(0.0);
        let offset = match gravity {
            Some(1) => available_height / 2.0,
            Some(2) => available_height,
            _ => 0.0,
        };
        for line in &mut self.lines {
            line.baseline += offset;
        }
    }
}

pub(in crate::render) fn measure_paragraph(
    styled: &StyledText<'_>,
    source: Range<usize>,
    width: f64,
    theme: RenderTheme,
    predefined: Option<PredefinedTextStyle>,
    renderer: &TextRenderer<'_>,
) -> Vec<WrappedLine> {
    renderer.report_object_issues(styled.objects.issues());
    if source.is_empty() {
        return vec![WrappedLine::unmeasured(
            source.clone(),
            styled.line_font_size(source, theme, predefined),
        )];
    }
    wrap_paragraph(styled, source.clone(), width, theme, predefined, renderer).unwrap_or_else(
        |_| {
            let style = styled.style_at(source.start, theme, predefined);
            renderer.measurement_failed(style.family.as_deref().unwrap_or("Roboto"));
            unmeasured_paragraph(styled, source, theme, predefined, renderer)
        },
    )
}

pub(in crate::render) fn layout_placed_text(
    styled: &StyledText<'_>,
    theme: RenderTheme,
    renderer: &TextRenderer<'_>,
) -> TextLayout {
    layout_text(
        styled,
        TextFrame {
            bbox: styled.text_box.bbox,
            gravity: styled.text_box.gravity,
            exclusions: &[],
        },
        theme,
        renderer,
    )
}

pub(in crate::render) fn layout_text(
    styled: &StyledText<'_>,
    frame: TextFrame<'_>,
    theme: RenderTheme,
    renderer: &TextRenderer<'_>,
) -> TextLayout {
    let text_box = styled.text_box;
    let settings = renderer.settings;
    renderer.report_geometry_issues(styled.geometry_issues());
    let margins = text_box
        .margins
        .unwrap_or([0.0; 4])
        .map(|margin| settings.pixels(margin));
    let outer_width = (frame.bbox.x_max - frame.bbox.x_min).ceil();
    let outer_height = (frame.bbox.y_max - frame.bbox.y_min).ceil();
    let content_left = frame.bbox.x_min + margins[0];
    let content_right = frame.bbox.x_min + outer_width - margins[2];
    let paragraphs = styled.index.paragraphs().collect::<Vec<_>>();
    let paragraph_layouts = paragraphs
        .iter()
        .map(|paragraph| {
            let ordinal = styled
                .index
                .paragraph_index(paragraph.content.start)
                .unwrap();
            paragraph_layout(text_box, ordinal, settings)
        })
        .collect::<Vec<_>>();
    let mut lines = Vec::new();
    let mut cursor = TextCursor::new(margins[1]);
    for (paragraph_number, (paragraph, layout)) in
        paragraphs.iter().zip(&paragraph_layouts).enumerate()
    {
        let x = content_left + layout.left_indent(settings);
        let width = (content_right - x).max(0.0);
        let paragraph_lines = measure_paragraph(
            styled,
            paragraph.content.clone(),
            width,
            theme,
            layout.predefined_style,
            renderer,
        );
        let previous = paragraph_number
            .checked_sub(1)
            .and_then(|previous| paragraph_layouts.get(previous))
            .and_then(|layout| layout.bullet);
        let next = paragraph_layouts
            .get(paragraph_number + 1)
            .and_then(|layout| layout.bullet);
        let spacing = ParagraphSpacing::for_lines(&paragraph_lines, previous, layout.bullet, next);
        if spacing.before {
            if layout.spacing_before_invalid {
                let style =
                    styled.style_at(paragraph.content.start, theme, layout.predefined_style);
                renderer.invalid_geometry(style.family.as_deref().unwrap_or("Roboto"));
            }
            cursor.add_spacing(layout.spacing_before);
        }
        for mut line in paragraph_lines {
            let candidate_top = cursor.candidate_top(&line, &frame);
            prepare_line_objects(&mut line, styled, candidate_top, theme, renderer);
            renderer.report_line_geometry(&line, layout.line_spacing);
            let baseline = cursor.place(&line, layout.line_spacing, &frame, settings);
            lines.push(TextLine {
                line,
                x,
                width,
                baseline,
                alignment: layout.alignment,
                predefined: layout.predefined_style,
            });
        }
        if spacing.after && paragraph_number + 1 < paragraphs.len() {
            if layout.spacing_after_invalid {
                let style =
                    styled.style_at(paragraph.content.start, theme, layout.predefined_style);
                renderer.invalid_geometry(style.family.as_deref().unwrap_or("Roboto"));
            }
            cursor.add_spacing(layout.spacing_after);
        }
    }
    let content_height = if styled.index.is_empty() {
        styled.style_at(0, theme, None).font_size + margins[1] + margins[3]
    } else {
        cursor.height_with_bottom(margins[3])
    };
    let mut layout = TextLayout {
        lines,
        content_height,
    };
    layout.apply_gravity(frame.gravity, outer_height);
    layout
}

#[cfg(test)]
mod tests {
    use super::super::objects::MeasuredObject;
    use super::super::wrapping::PositionedObject;
    use super::super::{TextContext, TextSettings};
    use super::*;
    use crate::fonts::FontBook;
    use crate::{RichTextBox, RichTextParagraph, RichTextParagraphType};

    fn text(value: &str) -> RichTextBox {
        RichTextBox {
            text_area_type: None,
            bbox: BoundingBox::default(),
            rotation_degrees: None,
            text: value.into(),
            color: None,
            highlight_color: None,
            underline: false,
            font_size: Some(10.0),
            runs: Vec::new(),
            spans: Vec::new(),
            paragraphs: Vec::new(),
            object_spans: Vec::new(),
            text_sections: Vec::new(),
            margins: None,
            gravity: None,
        }
    }

    fn measure(text: &RichTextBox, exclusions: &[VerticalExclusion]) -> TextLayout {
        let settings = TextSettings {
            scale: 1.0,
            font_size_delta: 0.0,
        };
        let fonts = FontBook::default();
        let renderer = TextRenderer::new(settings, &fonts);
        let styled = StyledText::new(text, TextContext::Placed, settings);
        layout_text(
            &styled,
            TextFrame {
                bbox: BoundingBox {
                    x_min: 10.0,
                    y_min: 100.0,
                    x_max: 1010.0,
                    y_max: 200.0,
                },
                gravity: text.gravity,
                exclusions,
            },
            RenderTheme::for_canvas(false),
            &renderer,
        )
    }

    #[test]
    fn touching_edges_and_sub_tolerance_overlap_do_not_move_lines() {
        for bands in [
            Vec::new(),
            vec![VerticalExclusion {
                top: 80.0,
                bottom: 100.0,
            }],
            vec![VerticalExclusion {
                top: 113.5,
                bottom: 130.0,
            }],
            vec![VerticalExclusion {
                top: 113.49995,
                bottom: 130.0,
            }],
        ] {
            let plan = measure(&text("ABC"), &bands);
            assert_eq!(plan.lines.len(), 1);
            assert_eq!(plan.lines[0].baseline, 110.0);
            assert_eq!(plan.height(), 13.5);
        }
    }

    #[test]
    fn full_line_advance_crossing_a_band_retries_at_its_bottom() {
        for top in [112.0, 113.499] {
            let plan = measure(&text("ABC"), &[VerticalExclusion { top, bottom: 130.0 }]);
            assert_eq!(plan.lines[0].baseline, 140.0);
            assert_eq!(plan.height(), 43.5);
        }
    }

    #[test]
    fn unsorted_bands_are_rechecked_after_each_retry() {
        let plan = measure(
            &text("ABC\nDEF"),
            &[
                VerticalExclusion {
                    top: 130.0,
                    bottom: 145.0,
                },
                VerticalExclusion {
                    top: 105.0,
                    bottom: 125.0,
                },
            ],
        );
        assert_eq!(
            plan.lines
                .iter()
                .map(|line| line.baseline)
                .collect::<Vec<_>>(),
            [155.0, 168.5]
        );
        assert_eq!(plan.height(), 72.0);
        assert_eq!(plan.lines[0].line.source, 0..3);
        assert_eq!(plan.lines[1].line.source, 4..7);
    }

    #[test]
    fn starting_inside_a_band_skips_only_the_remaining_height() {
        let plan = measure(
            &text("ABC"),
            &[VerticalExclusion {
                top: 90.0,
                bottom: 120.0,
            }],
        );
        assert_eq!(plan.lines[0].baseline, 130.0);
        assert_eq!(plan.height(), 33.5);
    }

    #[test]
    fn spacing_before_is_applied_once_and_skipped_height_counts_toward_layout() {
        let mut content = text("ABC");
        content.margins = Some([0.0, 2.0, 0.0, 3.0]);
        content.paragraphs.push(RichTextParagraph {
            kind: RichTextParagraphType::SpacingBefore,
            start_paragraph: 0,
            end_paragraph: 1,
            payload: 4.0_f32.to_le_bytes().to_vec(),
        });
        let clear = measure(&content, &[]);
        assert_eq!(clear.lines[0].baseline, 116.0);
        assert_eq!(clear.height(), 22.5);
        let blocked = measure(
            &content,
            &[VerticalExclusion {
                top: 110.0,
                bottom: 130.0,
            }],
        );
        assert_eq!(blocked.lines[0].baseline, 140.0);
        assert_eq!(blocked.height(), 46.5);
    }

    #[test]
    fn exclusions_preserve_empty_height_and_measured_gravity() {
        let bands = [VerticalExclusion {
            top: 105.0,
            bottom: 120.0,
        }];
        let mut empty = text("");
        empty.margins = Some([0.0, 2.0, 0.0, 3.0]);
        empty.gravity = Some(1);
        let plan = measure(&empty, &bands);
        assert!(plan.lines.is_empty());
        assert_eq!(plan.height(), 15.0);
        empty.margins = Some([0.0, 2.0, 0.0, -3.0]);
        assert_eq!(measure(&empty, &bands).height(), 9.0);

        let mut centered = text("ABC");
        centered.gravity = Some(1);
        let plan = measure(&centered, &bands);
        assert_eq!(plan.height(), 33.5);
        assert_eq!(plan.lines[0].baseline, 163.25);
    }

    #[test]
    fn native_percent_spacing_baseline_uses_the_full_line_advance() {
        let mut content = text("ABC");
        content.font_size = Some(45.0);
        content.paragraphs.push(RichTextParagraph {
            kind: RichTextParagraphType::LineSpacing,
            start_paragraph: 0,
            end_paragraph: 1,
            payload: [1_u32.to_le_bytes(), 1.6_f32.to_le_bytes()].concat(),
        });
        let native = measure(&content, &[]);
        assert!((native.lines[0].baseline - 156.25).abs() < 0.00001);
        assert!((native.height() - 72.0).abs() < 0.00001);
    }

    #[test]
    fn native_pixel_spacing_baseline_preserves_the_extra_leading() {
        let mut content = text("ABC");
        content.font_size = Some(45.0);
        content.paragraphs.push(RichTextParagraph {
            kind: RichTextParagraphType::LineSpacing,
            start_paragraph: 0,
            end_paragraph: 1,
            payload: [0_u32.to_le_bytes(), 20.0_f32.to_le_bytes()].concat(),
        });
        let native = measure(&content, &[]);
        assert_eq!(native.lines[0].baseline, 149.25);
        assert_eq!(native.height(), 65.0);
    }

    fn object_line(font_size: f64, inline: bool, margins: [f64; 2]) -> WrappedLine {
        let mut line = WrappedLine::unmeasured(0..1, font_size);
        line.objects.push(PositionedObject {
            object: MeasuredObject {
                source: 0..1,
                span_index: 0,
                bounds: BoundingBox {
                    x_min: 0.0,
                    y_min: 0.0,
                    x_max: 50.0,
                    y_max: 100.0,
                },
                height: 100.0,
                inline,
                top_margin: margins[0],
                bottom_margin: margins[1],
            },
            x: 0.0,
            prepared_code: None,
        });
        line
    }

    fn place_object(cursor: &mut TextCursor, line: &WrappedLine) -> f64 {
        cursor.place(
            line,
            None,
            &TextFrame {
                bbox: BoundingBox::default(),
                gravity: None,
                exclusions: &[],
            },
            TextSettings {
                scale: 1.0,
                font_size_delta: 0.0,
            },
        )
    }

    fn close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 0.0000001,
            "{actual} != {expected}"
        );
    }

    fn bullet(kind: BulletType) -> ParagraphBullet {
        ParagraphBullet {
            kind,
            number: 1,
            checked: false,
            initial_number: 1,
        }
    }

    #[test]
    fn paragraph_spacing_follows_the_native_rich_bullet_conversion() {
        let native_types = [
            (0, 0),
            (1, 5),
            (2, 1),
            (3, 5),
            (4, 2),
            (5, 2),
            (6, 3),
            (7, 4),
            (8, 5),
            (9, 6),
            (10, 7),
            (11, 8),
            (12, 9),
            (13, 0),
            (u32::MAX, 0),
        ];
        for (raw, native) in native_types {
            for (neighbor_raw, neighbor_native) in native_types {
                let current = Some(bullet(BulletType::from(raw)));
                let neighbor = Some(bullet(BulletType::from(neighbor_raw)));
                let spacing = ParagraphSpacing::for_lines(&[], neighbor, current, neighbor);
                let enabled = native == 0 || neighbor_native == 0;
                assert_eq!(
                    (spacing.before, spacing.after),
                    (enabled, enabled),
                    "raw bullets {raw}, {neighbor_raw}"
                );
            }
        }
        let spacing =
            ParagraphSpacing::for_lines(&[], None, Some(bullet(BulletType::RomanNumeral)), None);
        assert!(spacing.before && spacing.after);
    }

    #[test]
    fn paragraph_object_spacing_checks_each_content_edge_independently() {
        let mut line = object_line(20.0, false, [30.0; 2]);
        line.source = 0..3;
        for (source, expected) in [
            (0..1, (false, true)),
            (1..2, (true, true)),
            (2..3, (true, false)),
        ] {
            line.objects[0].object.source = source;
            let spacing =
                ParagraphSpacing::for_lines(std::slice::from_ref(&line), None, None, None);
            assert_eq!((spacing.before, spacing.after), expected);
        }
        line.objects[0].object.bottom_margin = 0.0;
        let spacing = ParagraphSpacing::for_lines(std::slice::from_ref(&line), None, None, None);
        assert!(spacing.before && spacing.after);
        let mut middle = object_line(20.0, false, [30.0; 2]);
        middle.source = 1..2;
        middle.objects[0].object.source = 1..2;
        let lines = [
            WrappedLine::unmeasured(0..1, 10.0),
            middle,
            WrappedLine::unmeasured(2..3, 10.0),
        ];
        let spacing = ParagraphSpacing::for_lines(&lines, None, None, None);
        assert!(spacing.before && spacing.after);
    }

    #[test]
    fn paragraph_spacing_suppression_changes_only_the_adjacent_gaps() {
        for (raw, second_baseline) in [
            (7_u32, 127.5),
            (10, 127.5),
            (11, 127.5),
            (12, 127.5),
            (0, 136.5),
            (13, 136.5),
        ] {
            let mut content = text("A\nB");
            content.paragraphs = vec![
                RichTextParagraph {
                    kind: RichTextParagraphType::Bullet,
                    start_paragraph: 0,
                    end_paragraph: 2,
                    payload: [raw, 1, 0, 1]
                        .into_iter()
                        .flat_map(u32::to_le_bytes)
                        .collect(),
                },
                RichTextParagraph {
                    kind: RichTextParagraphType::SpacingBefore,
                    start_paragraph: 0,
                    end_paragraph: 2,
                    payload: 4.0_f32.to_le_bytes().to_vec(),
                },
                RichTextParagraph {
                    kind: RichTextParagraphType::SpacingAfter,
                    start_paragraph: 0,
                    end_paragraph: 2,
                    payload: 5.0_f32.to_le_bytes().to_vec(),
                },
            ];
            let plan = measure(&content, &[]);
            assert_eq!(plan.lines[0].baseline, 114.0);
            assert_eq!(plan.lines[1].baseline, second_baseline, "raw bullet {raw}");
        }
    }

    #[test]
    fn inline_object_height_keeps_text_leading_and_native_epsilon() {
        let mut cursor = TextCursor::new(0.0);
        close(
            place_object(&mut cursor, &object_line(20.0, true, [0.0; 2])),
            100.001,
        );
        close(cursor.position(), 107.001);
        close(cursor.height(), 107.001);
    }

    #[test]
    fn block_object_margins_defer_the_bottom_and_omit_extra_leading() {
        let mut cursor = TextCursor::new(0.0);
        close(
            place_object(&mut cursor, &object_line(20.0, false, [30.0; 2])),
            130.001,
        );
        close(cursor.position(), 130.001);
        close(cursor.height(), 160.001);
    }

    #[test]
    fn adjacent_block_margins_collapse_instead_of_accumulating() {
        let mut cursor = TextCursor::new(0.0);
        let line = object_line(20.0, false, [30.0; 2]);
        close(place_object(&mut cursor, &line), 130.001);
        close(place_object(&mut cursor, &line), 260.002);
        close(cursor.height(), 290.002);
    }

    #[test]
    fn final_text_box_bottom_collapses_with_the_pending_object_margin() {
        let mut cursor = TextCursor::new(0.0);
        place_object(&mut cursor, &object_line(20.0, false, [30.0; 2]));
        close(cursor.height_with_bottom(10.0), 160.001);
        close(cursor.height_with_bottom(40.0), 170.001);
        close(cursor.height_with_bottom(-10.0), 160.001);
    }

    #[test]
    fn following_text_consumes_the_pending_object_margin_once() {
        let mut cursor = TextCursor::new(0.0);
        place_object(&mut cursor, &object_line(20.0, false, [30.0; 2]));
        let text = WrappedLine::unmeasured(1..2, 20.0);
        close(place_object(&mut cursor, &text), 180.001);
        close(cursor.height(), 187.001);
        close(place_object(&mut cursor, &text), 207.001);
        close(cursor.height(), 214.001);
    }

    #[test]
    fn zero_text_font_metric_does_not_add_leading_to_object_height() {
        let mut cursor = TextCursor::new(0.0);
        close(
            place_object(&mut cursor, &object_line(0.0, true, [0.0; 2])),
            100.001,
        );
        close(cursor.height(), 100.001);
    }
}
