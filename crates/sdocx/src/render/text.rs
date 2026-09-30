use std::ops::Range;

use crate::{
    Color, HyperlinkType, LineSpacingType, ParagraphAlignment, ParagraphBullet,
    ParagraphLineSpacing, PredefinedTextStyle, RichTextBox, RichTextParagraphType, RichTextSpan,
    RichTextSpanType, text_index::TextIndex,
};

use super::RenderTheme;

mod breaks;
mod measurement;
mod paint;
mod resources;
mod wrapping;
pub(super) use paint::render_measured_line;
pub(super) use resources::TextRenderer;
pub use resources::{TextDiagnostic, TextDiagnosticKind};
pub(super) use wrapping::{WrappedLine, wrap_paragraph};

pub(super) const DEFAULT_FONT_SIZE: f32 = 17.0;
const DEFAULT_FONT_COLOR: Color = Color {
    r: 38,
    g: 38,
    b: 38,
};

#[derive(Clone)]
pub(super) struct TextStyle {
    pub font_size: f64,
    pub family: Option<String>,
    pub color: String,
    pub source_color: Color,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub link_target: Option<String>,
}

#[derive(Clone, Copy)]
pub(super) enum TextContext {
    Flow,
    Placed,
}

#[derive(Clone, Copy)]
pub(super) struct TextSettings {
    pub scale: f32,
    pub font_size_delta: f32,
}

impl TextSettings {
    pub fn from_document(metadata: &crate::DocumentMetadata) -> Self {
        Self {
            scale: metadata.document_density(),
            font_size_delta: metadata
                .body_font_size_delta
                .filter(|delta| *delta != i32::MIN)
                .unwrap_or(0) as f32,
        }
    }

    pub fn font_size(self, size: f32) -> f64 {
        let size = if size.is_finite() {
            size
        } else {
            DEFAULT_FONT_SIZE
        };
        f64::from((size + self.font_size_delta).max(1.0) * self.scale)
    }

    pub fn pixels(self, value: f32) -> f64 {
        f64::from(value * self.scale)
    }

    pub fn indent(self, level: u32) -> f64 {
        f64::from((level as i32 as f32 * (16.0 * self.scale)) as i32)
    }
}

#[derive(Default)]
pub(in crate::render) struct ParagraphLayout {
    pub alignment: Option<ParagraphAlignment>,
    pub indent_level: u32,
    pub line_spacing: Option<ParagraphLineSpacing>,
    pub bullet: Option<ParagraphBullet>,
    pub spacing_before: f64,
    pub spacing_after: f64,
    pub predefined_style: Option<PredefinedTextStyle>,
}

impl ParagraphLayout {
    pub fn left_indent(&self, settings: TextSettings) -> f64 {
        if matches!(
            self.alignment,
            Some(ParagraphAlignment::Right | ParagraphAlignment::Both)
        ) {
            0.0
        } else {
            settings.indent(self.indent_level)
        }
    }
}

pub(in crate::render) fn paragraph_layout(
    text_box: &RichTextBox,
    paragraph_index: u32,
    settings: TextSettings,
) -> ParagraphLayout {
    let mut layout = ParagraphLayout::default();
    for paragraph in text_box.paragraphs.iter().filter(|paragraph| {
        paragraph.start_paragraph <= paragraph_index && paragraph.end_paragraph > paragraph_index
    }) {
        match paragraph.kind {
            RichTextParagraphType::Alignment => layout.alignment = paragraph.alignment(),
            RichTextParagraphType::IndentLevel => {
                if let Some(indent) = paragraph.indent() {
                    layout.indent_level = indent.level;
                }
            }
            RichTextParagraphType::LineSpacing => layout.line_spacing = paragraph.line_spacing(),
            RichTextParagraphType::Bullet => layout.bullet = paragraph.bullet(),
            RichTextParagraphType::SpacingBefore => {
                layout.spacing_before = paragraph
                    .spacing()
                    .filter(|spacing| spacing.is_finite() && *spacing > 0.0)
                    .map_or(0.0, |spacing| settings.pixels(spacing));
            }
            RichTextParagraphType::SpacingAfter => {
                layout.spacing_after = paragraph
                    .spacing()
                    .filter(|spacing| spacing.is_finite() && *spacing > 0.0)
                    .map_or(0.0, |spacing| settings.pixels(spacing));
            }
            RichTextParagraphType::PredefinedStyle => {
                layout.predefined_style = paragraph.predefined_style().map(|style| style.style)
            }
            _ => {}
        }
    }
    layout
}

pub(in crate::render) fn paragraph_line_height(
    font_size: f64,
    spacing: Option<ParagraphLineSpacing>,
    settings: TextSettings,
) -> f64 {
    match spacing {
        Some(spacing)
            if spacing.value.is_finite()
                && spacing.value > 0.0
                && spacing.kind == LineSpacingType::Percent =>
        {
            font_size * f64::from(spacing.value)
        }
        Some(spacing)
            if spacing.value.is_finite()
                && spacing.value > 0.0
                && spacing.kind == LineSpacingType::Pixels =>
        {
            font_size + settings.pixels(spacing.value)
        }
        _ => font_size * 1.35,
    }
}

pub(super) struct StyledText<'a> {
    pub index: TextIndex<'a>,
    text_box: &'a RichTextBox,
    context: TextContext,
    settings: TextSettings,
    boundaries: Vec<usize>,
    spans: Vec<(Range<usize>, &'a RichTextSpan)>,
}

impl<'a> StyledText<'a> {
    pub fn context(&self) -> TextContext {
        self.context
    }
    pub fn new(text_box: &'a RichTextBox, context: TextContext, settings: TextSettings) -> Self {
        let index = TextIndex::new(&text_box.text);
        let spans = text_box
            .spans
            .iter()
            .filter_map(|span| Some((span_range(&index, span)?, span)))
            .collect::<Vec<_>>();
        let mut boundaries = vec![0, index.len()];
        for (range, _) in &spans {
            boundaries.extend([range.start, range.end]);
        }
        for run in &text_box.runs {
            if run.start < run.end && run.end <= index.len() {
                boundaries.extend([run.start, run.end]);
            }
        }
        boundaries.sort_unstable();
        boundaries.dedup();
        Self {
            index,
            text_box,
            context,
            settings,
            boundaries,
            spans,
        }
    }

    pub fn style_at(
        &self,
        character: usize,
        theme: RenderTheme,
        predefined: Option<PredefinedTextStyle>,
    ) -> TextStyle {
        let text_box = self.text_box;
        let mut font_size = self
            .settings
            .font_size(text_box.font_size.unwrap_or(DEFAULT_FONT_SIZE));
        if let Some(style) = predefined {
            font_size = match style {
                PredefinedTextStyle::Heading1 => self.settings.font_size(21.0),
                PredefinedTextStyle::Heading2 => self.settings.font_size(19.0),
                PredefinedTextStyle::Heading3 => self.settings.font_size(17.0),
                _ => font_size,
            };
        }
        let mut style = TextStyle {
            font_size,
            family: None,
            color: theme.foreground(Some(text_box.color.unwrap_or(DEFAULT_FONT_COLOR))),
            source_color: text_box.color.unwrap_or(DEFAULT_FONT_COLOR),
            bold: false,
            italic: false,
            underline: text_box.underline,
            strikethrough: false,
            link_target: None,
        };
        for run in &text_box.runs {
            if run.start <= character && character < run.end && run.end <= self.index.len() {
                style.bold |= run.bold;
                style.italic |= run.italic;
            }
        }
        let mut is_hyperlink = false;
        for (range, span) in &self.spans {
            if !range.contains(&character) {
                continue;
            }
            match span.kind {
                RichTextSpanType::ForegroundColor => {
                    if let Some(color) = span.color_value() {
                        style.source_color = color;
                        style.color = theme.foreground(Some(color));
                    }
                }
                RichTextSpanType::FontSize => {
                    if let Some(size) = span.font_size_value().filter(|size| size.is_finite()) {
                        style.font_size = self.settings.font_size(size);
                    }
                }
                RichTextSpanType::FontName => {
                    if let Some(name) = span.font_name_value() {
                        style.family = (!name.is_empty()).then(|| name.to_owned());
                    }
                }
                RichTextSpanType::Bold => {
                    if let Some(value) = span.boolean_value() {
                        style.bold = value;
                    }
                }
                RichTextSpanType::Italic => {
                    if let Some(value) = span.boolean_value() {
                        style.italic = value;
                    }
                }
                RichTextSpanType::Underline => {
                    if let Some(value) = span.boolean_value() {
                        style.underline = value;
                    }
                }
                RichTextSpanType::Strikethrough => {
                    if let Some(value) = span.boolean_value() {
                        style.strikethrough = value;
                    }
                }
                RichTextSpanType::Hyperlink => {
                    is_hyperlink = true;
                    style.link_target = hyperlink_target(&self.index, span);
                }
                _ => {}
            }
        }
        if is_hyperlink {
            style.source_color = Color {
                r: 0,
                g: 84,
                b: 255,
            };
            style.color = theme.foreground(Some(style.source_color));
            style.underline = true;
        }
        if matches!(
            predefined,
            Some(
                PredefinedTextStyle::Heading1
                    | PredefinedTextStyle::Heading2
                    | PredefinedTextStyle::Heading3
            )
        ) {
            style.bold = false;
        }
        style
    }

    pub fn segments(&self, range: Range<usize>) -> impl Iterator<Item = Range<usize>> + '_ {
        let first = self
            .boundaries
            .partition_point(|boundary| *boundary <= range.start)
            .saturating_sub(1);
        let last = self
            .boundaries
            .partition_point(|boundary| *boundary < range.end)
            .saturating_add(1)
            .min(self.boundaries.len());
        self.boundaries[first.min(last)..last]
            .windows(2)
            .filter_map(move |pair| {
                let start = pair[0].max(range.start);
                let end = pair[1].min(range.end);
                (start < end).then_some(start..end)
            })
    }

    pub fn line_font_size(
        &self,
        range: Range<usize>,
        theme: RenderTheme,
        predefined: Option<PredefinedTextStyle>,
    ) -> f64 {
        self.segments(range.clone())
            .map(|segment| self.style_at(segment.start, theme, predefined).font_size)
            .reduce(f64::max)
            .unwrap_or_else(|| self.style_at(range.start, theme, predefined).font_size)
    }
}

fn span_range(index: &TextIndex<'_>, span: &RichTextSpan) -> Option<Range<usize>> {
    let start = index.utf16_to_char(span.start_utf16)?;
    let end = index.utf16_to_char(span.end_utf16)?;
    (start < end).then_some(start..end)
}

fn hyperlink_target(index: &TextIndex<'_>, span: &RichTextSpan) -> Option<String> {
    let hyperlink = span.hyperlink_value()?;
    if let Some(target) = hyperlink.custom_data.filter(|target| !target.is_empty()) {
        return sanitize_hyperlink_target(target);
    }
    let visible_text = index.slice(span_range(index, span)?)?;
    let target = match hyperlink.kind {
        HyperlinkType::Email => format!("mailto:{visible_text}"),
        HyperlinkType::Telephone => format!("tel:{visible_text}"),
        HyperlinkType::Url => visible_text.to_owned(),
        _ => return None,
    };
    sanitize_hyperlink_target(target)
}

pub(super) fn sanitize_hyperlink_target(target: String) -> Option<String> {
    let target = target.trim();
    if target.is_empty() || target.chars().any(char::is_control) {
        return None;
    }
    if target.starts_with('#') || target.starts_with('?') {
        return Some(target.to_string());
    }
    if target.starts_with("//") || target.starts_with('\\') {
        return None;
    }
    if target.starts_with('/') || target.starts_with("./") || target.starts_with("../") {
        return Some(target.to_string());
    }
    if let Some(colon_index) = target.find(':') {
        let path_delimiter = target
            .char_indices()
            .find_map(|(index, character)| matches!(character, '/' | '?' | '#').then_some(index));
        if path_delimiter.is_none_or(|index| colon_index < index) {
            let scheme = &target[..colon_index];
            if !["http", "https", "mailto", "tel"]
                .iter()
                .any(|allowed| scheme.eq_ignore_ascii_case(allowed))
            {
                return None;
            }
        }
    }
    Some(target.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_segments_clip_unicode_ranges_and_handle_empty_and_reversed_queries() {
        let text_box = RichTextBox {
            text_area_type: None,
            bbox: Default::default(),
            rotation_degrees: None,
            text: "ab😀cdef".into(),
            color: None,
            highlight_color: None,
            underline: false,
            font_size: None,
            runs: vec![crate::RichTextRun {
                start: 1,
                end: 6,
                bold: true,
                italic: false,
            }],
            spans: vec![RichTextSpan {
                kind: RichTextSpanType::ForegroundColor,
                start_utf16: 4,
                end_utf16: 6,
                expand: false,
                payload: vec![0, 0, 255, 255],
            }],
            paragraphs: Vec::new(),
            object_spans: Vec::new(),
            text_sections: Vec::new(),
            margins: None,
            gravity: None,
        };
        let styled = StyledText::new(
            &text_box,
            TextContext::Flow,
            TextSettings {
                scale: 1.0,
                font_size_delta: 0.0,
            },
        );
        assert_eq!(
            styled.segments(2..6).collect::<Vec<_>>(),
            [2..3, 3..5, 5..6]
        );
        assert!(styled.segments(3..5).eq(std::iter::once(3..5)));
        assert!(styled.segments(6..7).eq(std::iter::once(6..7)));
        assert_eq!(
            styled.segments(4..usize::MAX).collect::<Vec<_>>(),
            [4..5, 5..6, 6..7]
        );
        for range in [0..0, 2..2, 7..7, 8..usize::MAX, Range { start: 6, end: 2 }] {
            assert_eq!(styled.segments(range).count(), 0);
        }
        let names = styled
            .segments(2..6)
            .map(|range| styled.index.slice(range).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(names, ["😀", "cd", "e"]);
    }
}
