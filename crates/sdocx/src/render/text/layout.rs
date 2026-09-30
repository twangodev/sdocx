use std::ops::Range;

use crate::render::RenderTheme;
use crate::{BoundingBox, ParagraphAlignment, PredefinedTextStyle};

use super::{
    StyledText, TextRenderer, WrappedLine, paragraph_layout, paragraph_line_height, wrap_paragraph,
};

#[derive(Clone, Copy)]
pub(in crate::render) struct VerticalExclusion {
    /// Absolute vertical coordinates in the same space as the frame's bounding box.
    pub top: f64,
    pub bottom: f64,
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
            vec![WrappedLine::unmeasured(
                source.clone(),
                styled.line_font_size(source, theme, predefined),
            )]
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
    let margins = text_box
        .margins
        .unwrap_or([0.0; 4])
        .map(|margin| settings.pixels(margin));
    let outer_width = (frame.bbox.x_max - frame.bbox.x_min).ceil();
    let outer_height = (frame.bbox.y_max - frame.bbox.y_min).ceil();
    let content_left = frame.bbox.x_min + margins[0];
    let content_right = frame.bbox.x_min + outer_width - margins[2];
    let paragraphs = styled.index.paragraphs().collect::<Vec<_>>();
    let mut lines = Vec::new();
    let mut cursor = margins[1];
    for (paragraph_number, paragraph) in paragraphs.iter().enumerate() {
        let ordinal = styled
            .index
            .paragraph_index(paragraph.content.start)
            .unwrap();
        let layout = paragraph_layout(text_box, ordinal, settings);
        let x = content_left + layout.left_indent(settings);
        let width = (content_right - x).max(0.0);
        cursor += layout.spacing_before;
        for line in measure_paragraph(
            styled,
            paragraph.content.clone(),
            width,
            theme,
            layout.predefined_style,
            renderer,
        ) {
            let advance = paragraph_line_height(line.font_size, layout.line_spacing, settings);
            let candidate_top = frame.bbox.y_min + cursor;
            let top = frame.line_top(candidate_top, advance);
            let baseline = top + line.font_size;
            cursor += top - candidate_top + advance;
            lines.push(TextLine {
                line,
                x,
                width,
                baseline,
                alignment: layout.alignment,
                predefined: layout.predefined_style,
            });
        }
        if paragraph_number + 1 < paragraphs.len() {
            cursor += layout.spacing_after;
        }
    }
    let content_height = if styled.index.is_empty() {
        styled.style_at(0, theme, None).font_size + margins[1] + margins[3]
    } else {
        cursor + margins[3].max(0.0)
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

        let mut centered = text("ABC");
        centered.gravity = Some(1);
        let plan = measure(&centered, &bands);
        assert_eq!(plan.height(), 33.5);
        assert_eq!(plan.lines[0].baseline, 163.25);
    }
}
