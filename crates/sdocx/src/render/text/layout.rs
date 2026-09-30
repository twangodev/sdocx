use std::ops::Range;

use crate::render::RenderTheme;
use crate::{BoundingBox, ParagraphAlignment, PredefinedTextStyle};

use super::{
    StyledText, TextRenderer, WrappedLine, paragraph_layout, paragraph_line_height, wrap_paragraph,
};

pub(in crate::render) struct TextFrame {
    pub bbox: BoundingBox,
    pub gravity: Option<u8>,
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
        },
        theme,
        renderer,
    )
}

pub(in crate::render) fn layout_text(
    styled: &StyledText<'_>,
    frame: TextFrame,
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
            let baseline = frame.bbox.y_min + cursor + line.font_size;
            cursor += paragraph_line_height(line.font_size, layout.line_spacing, settings);
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
