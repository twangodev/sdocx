use std::ops::Range;

use crate::{
    Color, HyperlinkType, PredefinedTextStyle, RichTextBox, RichTextSpan, RichTextSpanType,
    text_index::TextIndex,
};

use super::RenderTheme;

mod breaks;
mod measurement;
mod resources;
mod wrapping;
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
        self.boundaries.windows(2).filter_map(move |pair| {
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
