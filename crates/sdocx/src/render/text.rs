use std::ops::Range;

use crate::{
    Color, HyperlinkType, LineSpacingType, ParagraphAlignment, ParagraphBullet, ParagraphDirection,
    ParagraphLineSpacing, PredefinedTextStyle, RichTextBox, RichTextParagraphType, RichTextSpan,
    RichTextSpanType, text_index::TextIndex,
};

use super::RenderTheme;

mod background;
mod breaks;
mod layout;
mod measurement;
mod objects;
mod pagination;
mod paint;
mod resources;
mod wrapping;
pub(super) use background::render_line_backgrounds;
#[cfg(test)]
use layout::measure_paragraph;
pub(super) use layout::{
    PositionedMarker, TextFrame, TextLayout, TextLine, VerticalExclusion, layout_capture_text,
    layout_flow_text, layout_text, layout_text_with_size,
};
pub use objects::{ObjectDiagnostic, ObjectDiagnosticKind};
pub(super) use pagination::PageExclusions;
pub(super) use paint::{render_measured_line, text_ranges as body_text_ranges};
pub(super) use resources::{
    SourceObjectDiagnostic, SourceOwner, SourceTextDiagnostic, TextRenderer,
};
pub use resources::{TextDiagnostic, TextDiagnosticKind};
pub(super) use wrapping::{PositionedObject, WrappedLine, unmeasured_paragraph, wrap_paragraph};

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
    pub background: Option<TextBackground>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub link_target: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct TextBackground {
    pub color: Color,
    pub alpha: u8,
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
    pub(super) font_size_units: FontSizeUnits,
}

#[derive(Clone, Copy)]
pub(super) enum FontSizeUnits {
    Logical,
    Resolved,
}

impl Default for TextSettings {
    fn default() -> Self {
        Self {
            scale: 1.0,
            font_size_delta: 0.0,
            font_size_units: FontSizeUnits::Logical,
        }
    }
}

impl TextSettings {
    pub fn from_document(metadata: &crate::DocumentMetadata) -> Self {
        Self {
            scale: metadata.document_density(),
            font_size_delta: metadata
                .body_font_size_delta
                .filter(|delta| *delta != i32::MIN)
                .unwrap_or(0) as f32,
            ..Self::default()
        }
    }

    pub fn resolved() -> Self {
        Self {
            font_size_units: FontSizeUnits::Resolved,
            ..Self::default()
        }
    }

    pub fn font_size(self, size: f32) -> f64 {
        self.checked_font_size(size)
            .or_else(|| self.checked_font_size(DEFAULT_FONT_SIZE))
            .unwrap_or(f64::from(DEFAULT_FONT_SIZE))
    }

    pub fn pixels(self, value: f32) -> f64 {
        self.checked_pixels(value).unwrap_or(0.0)
    }

    fn checked_font_size(self, size: f32) -> Option<f64> {
        size.is_finite().then_some(())?;
        let pixels = match self.font_size_units {
            FontSizeUnits::Logical => (size + self.font_size_delta).max(1.0) * self.scale,
            FontSizeUnits::Resolved => {
                (size > 0.0).then_some(())?;
                size
            }
        };
        finite_native_geometry(f64::from(pixels))
    }

    fn checked_caret_default(self, size: f32) -> Option<f64> {
        size.is_finite().then_some(())?;
        let pixels = match self.font_size_units {
            FontSizeUnits::Logical => size.mul_add(self.scale, self.font_size_delta * self.scale),
            FontSizeUnits::Resolved => size,
        };
        (pixels > 0.0).then_some(())?;
        finite_native_geometry(f64::from(pixels))
    }

    fn checked_pixels(self, value: f32) -> Option<f64> {
        finite_native_geometry(f64::from(value * self.scale))
    }

    pub fn indent(self, level: u32) -> f64 {
        f64::from((level as i32 as f32 * (16.0 * self.scale)) as i32)
    }
}

#[derive(Default)]
pub(in crate::render) struct ParagraphLayout {
    pub alignment: Option<ParagraphAlignment>,
    pub indent_level: u32,
    pub indent_direction: Option<ParagraphDirection>,
    pub line_spacing: Option<ParagraphLineSpacing>,
    pub bullet: Option<ParagraphBullet>,
    pub spacing_before: f64,
    pub spacing_after: f64,
    pub spacing_before_invalid: bool,
    pub spacing_after_invalid: bool,
    pub predefined_style: Option<PredefinedTextStyle>,
}

impl ParagraphLayout {
    pub fn indent_insets(&self, settings: TextSettings) -> [f64; 2] {
        let indent = settings.indent(self.indent_level);
        match (self.indent_direction, self.alignment) {
            (Some(ParagraphDirection::RightToLeft), Some(ParagraphAlignment::Left))
            | (_, Some(ParagraphAlignment::Both)) => [0.0; 2],
            (Some(ParagraphDirection::RightToLeft), _) => [0.0, indent],
            (_, Some(ParagraphAlignment::Right)) => [0.0; 2],
            _ => [indent, 0.0],
        }
    }
}

pub(super) fn line_alignment_offset(
    advance: f64,
    width: f64,
    alignment: Option<ParagraphAlignment>,
) -> f64 {
    let remaining = (width - advance).max(0.0);
    match alignment {
        Some(ParagraphAlignment::Right) => remaining,
        Some(ParagraphAlignment::Center) => remaining / 2.0,
        _ => 0.0,
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
                    layout.indent_direction = Some(indent.direction);
                }
            }
            RichTextParagraphType::LineSpacing => layout.line_spacing = paragraph.line_spacing(),
            RichTextParagraphType::Bullet => layout.bullet = paragraph.bullet(),
            RichTextParagraphType::SpacingBefore => {
                let spacing = paragraph.spacing();
                layout.spacing_before_invalid = spacing.is_some_and(|spacing| {
                    (spacing > 0.0 || !spacing.is_finite())
                        && settings.checked_pixels(spacing).is_none()
                });
                layout.spacing_before = spacing
                    .filter(|spacing| spacing.is_finite() && *spacing > 0.0)
                    .map_or(0.0, |spacing| settings.pixels(spacing));
            }
            RichTextParagraphType::SpacingAfter => {
                let spacing = paragraph.spacing();
                layout.spacing_after_invalid = spacing.is_some_and(|spacing| {
                    (spacing > 0.0 || !spacing.is_finite())
                        && settings.checked_pixels(spacing).is_none()
                });
                layout.spacing_after = spacing
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
    spacing
        .and_then(|spacing| explicit_line_height(font_size, spacing, settings))
        .or_else(|| finite_native_geometry(font_size * 1.35))
        .unwrap_or(font_size)
}

pub(super) fn finite_native_geometry(value: f64) -> Option<f64> {
    (value.is_finite() && (value as f32).is_finite()).then_some(value)
}

pub(super) fn explicit_line_height(
    font_size: f64,
    spacing: ParagraphLineSpacing,
    settings: TextSettings,
) -> Option<f64> {
    match spacing {
        spacing
            if spacing.value.is_finite()
                && spacing.value > 0.0
                && spacing.kind == LineSpacingType::Percent =>
        {
            finite_native_geometry(font_size * f64::from(spacing.value))
        }
        spacing
            if spacing.value.is_finite()
                && spacing.value > 0.0
                && spacing.kind == LineSpacingType::Pixels =>
        {
            finite_native_geometry(font_size + settings.checked_pixels(spacing.value)?)
        }
        _ => None,
    }
}

pub(super) struct StyledText<'a> {
    pub index: TextIndex<'a>,
    text_box: &'a RichTextBox,
    objects: objects::TextObjectIndex<'a>,
    context: TextContext,
    settings: TextSettings,
    boundaries: Vec<usize>,
    foreground_boundaries: Vec<usize>,
    spans: Vec<(Range<usize>, &'a RichTextSpan)>,
    geometry_issues: Vec<TextDiagnostic>,
}

impl<'a> StyledText<'a> {
    pub fn context(&self) -> TextContext {
        self.context
    }
    pub fn object_span(&self, index: usize) -> Option<&crate::RichTextObjectSpan> {
        self.text_box.object_spans.get(index)
    }
    pub fn object_issues(&self) -> &[ObjectDiagnostic] {
        self.objects.issues()
    }
    pub fn geometry_issues(&self) -> &[TextDiagnostic] {
        &self.geometry_issues
    }
    pub fn new(text_box: &'a RichTextBox, context: TextContext, settings: TextSettings) -> Self {
        let index = TextIndex::new(&text_box.text);
        let objects = objects::TextObjectIndex::new(text_box, &index);
        let spans = text_box
            .spans
            .iter()
            .filter_map(|span| Some((span_range(&index, span)?, span)))
            .collect::<Vec<_>>();
        let mut boundaries = vec![0, index.len()];
        let mut foreground_boundaries = boundaries.clone();
        for (range, span) in &spans {
            boundaries.extend([range.start, range.end]);
            if span.kind != RichTextSpanType::BackgroundColor {
                foreground_boundaries.extend([range.start, range.end]);
            }
        }
        for run in &text_box.runs {
            if run.start < run.end && run.end <= index.len() {
                boundaries.extend([run.start, run.end]);
                foreground_boundaries.extend([run.start, run.end]);
            }
        }
        boundaries.sort_unstable();
        boundaries.dedup();
        foreground_boundaries.sort_unstable();
        foreground_boundaries.dedup();
        let mut styled = Self {
            index,
            text_box,
            objects,
            context,
            settings,
            boundaries,
            foreground_boundaries,
            spans,
            geometry_issues: Vec::new(),
        };
        styled.geometry_issues = styled.collect_geometry_issues();
        styled
    }

    pub fn style_at(
        &self,
        character: usize,
        theme: RenderTheme,
        predefined: Option<PredefinedTextStyle>,
    ) -> TextStyle {
        self.resolved_style_at(character, theme, predefined).0
    }

    pub fn font_size_at_caret(&self, character: usize) -> f64 {
        self.resolved_caret_font_size(character).0
    }

    pub fn caret_line_height(&self, character: usize) -> f64 {
        let font_size = self.font_size_at_caret(character) as f32;
        let spacing = self.index.paragraph_index(character).and_then(|ordinal| {
            paragraph_layout(self.text_box, ordinal, self.settings).line_spacing
        });
        let height = match spacing {
            Some(spacing) if spacing.value.is_finite() && spacing.value > 0.0 => match spacing.kind
            {
                LineSpacingType::Pixels => {
                    let pixels = self.settings.pixels(spacing.value) as f32;
                    if pixels != 0.0 {
                        font_size + pixels
                    } else {
                        font_size * 1.35_f32
                    }
                }
                LineSpacingType::Percent => font_size.mul_add(spacing.value - 1.0, font_size),
                _ => font_size * 1.35_f32,
            },
            _ => font_size * 1.35_f32,
        };
        f64::from(height)
    }

    fn resolved_caret_font_size(&self, character: usize) -> (f64, bool) {
        let default = self.text_box.font_size.unwrap_or(DEFAULT_FONT_SIZE);
        let resolved_default = self.settings.checked_caret_default(default);
        let Some(utf16) = self.index.char_to_utf16(character) else {
            return (
                resolved_default.unwrap_or_else(|| self.settings.font_size(default)),
                resolved_default.is_none(),
            );
        };
        let mut invalid_override = false;
        for span in self.text_box.spans.iter().rev() {
            if span.kind != RichTextSpanType::FontSize
                || span.start_utf16 > span.end_utf16
                || self.index.utf16_to_char(span.start_utf16).is_none()
                || self.index.utf16_to_char(span.end_utf16).is_none()
                || !span.contains_caret(utf16)
            {
                continue;
            }
            if let Some(value) = span.font_size_value() {
                if let Some(size) = self.settings.checked_font_size(value) {
                    return (size, invalid_override);
                }
                invalid_override = true;
            }
        }
        (
            resolved_default.unwrap_or_else(|| self.settings.font_size(default)),
            invalid_override || resolved_default.is_none(),
        )
    }

    fn resolved_style_at(
        &self,
        character: usize,
        theme: RenderTheme,
        predefined: Option<PredefinedTextStyle>,
    ) -> (TextStyle, bool) {
        let text_box = self.text_box;
        let mut size = match predefined {
            Some(PredefinedTextStyle::Heading1) => 21.0,
            Some(PredefinedTextStyle::Heading2) => 19.0,
            Some(PredefinedTextStyle::Heading3) => 17.0,
            _ => text_box.font_size.unwrap_or(DEFAULT_FONT_SIZE),
        };
        let mut invalid_override = false;
        for (range, span) in &self.spans {
            if span.kind == RichTextSpanType::FontSize
                && range.contains(&character)
                && let Some(value) = span.font_size_value()
            {
                if self.settings.checked_font_size(value).is_some() {
                    size = value;
                    invalid_override = false;
                } else {
                    invalid_override = true;
                }
            }
        }
        let invalid = invalid_override || self.settings.checked_font_size(size).is_none();
        let mut style = TextStyle {
            font_size: self.settings.font_size(size),
            family: None,
            color: theme.foreground(Some(text_box.color.unwrap_or(DEFAULT_FONT_COLOR))),
            source_color: text_box.color.unwrap_or(DEFAULT_FONT_COLOR),
            background: None,
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
                RichTextSpanType::BackgroundColor => {
                    if let Some(argb) = span.argb_value() {
                        style.background = Some(TextBackground {
                            color: theme.span_background_color(Color {
                                r: (argb >> 16) as u8,
                                g: (argb >> 8) as u8,
                                b: argb as u8,
                            }),
                            alpha: (argb >> 24) as u8,
                        });
                    }
                }
                RichTextSpanType::FontSize => {}
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
        (style, invalid)
    }

    pub fn segments(&self, range: Range<usize>) -> impl Iterator<Item = Range<usize>> + '_ {
        Self::segments_at(&self.boundaries, range)
    }

    pub fn foreground_segments(
        &self,
        range: Range<usize>,
    ) -> impl Iterator<Item = Range<usize>> + '_ {
        Self::segments_at(&self.foreground_boundaries, range)
    }

    fn segments_at(
        boundaries: &[usize],
        range: Range<usize>,
    ) -> impl Iterator<Item = Range<usize>> + '_ {
        let first = boundaries
            .partition_point(|boundary| *boundary <= range.start)
            .saturating_sub(1);
        let last = boundaries
            .partition_point(|boundary| *boundary < range.end)
            .saturating_add(1)
            .min(boundaries.len());
        boundaries[first.min(last)..last]
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

    fn collect_geometry_issues(&self) -> Vec<TextDiagnostic> {
        let mut issues = Vec::new();
        let mut record = |style: &TextStyle| {
            let family = style.family.as_deref().unwrap_or("Roboto");
            if !issues
                .iter()
                .any(|issue: &TextDiagnostic| issue.family == family)
            {
                issues.push(TextDiagnostic {
                    kind: TextDiagnosticKind::InvalidGeometry,
                    family: family.into(),
                    codepoints: Vec::new(),
                });
            }
        };
        let theme = RenderTheme::for_canvas(false);
        let invalid_margins = self.text_box.margins.is_some_and(|margins| {
            margins
                .iter()
                .any(|margin| self.settings.checked_pixels(*margin).is_none())
        });
        if self.index.is_empty() {
            let style = self.style_at(0, theme, None);
            let invalid = self.resolved_caret_font_size(0).1;
            if invalid || invalid_margins {
                record(&style);
            }
        }
        for paragraph in self.index.display_paragraphs() {
            let ordinal = self.index.paragraph_index(paragraph.content.start).unwrap();
            let layout = paragraph_layout(self.text_box, ordinal, self.settings);
            let mut ranges = self.segments(paragraph.content.clone()).collect::<Vec<_>>();
            if ranges.is_empty() {
                ranges.push(paragraph.content.clone());
            }
            for range in ranges {
                let (style, mut invalid_font) =
                    self.resolved_style_at(range.start, theme, layout.predefined_style);
                if range.is_empty() {
                    invalid_font = self.resolved_caret_font_size(range.start).1;
                }
                let objects = self.objects.in_range(range.clone());
                let object_only = !range.is_empty() && objects.len() == range.len();
                let inherits_separator = range.start == paragraph.content.start
                    && range.start > 0
                    && matches!(
                        self.index.slice(range.start - 1..range.start),
                        Some("\r" | "\n")
                    );
                let uses_font = !object_only || inherits_separator;
                if (uses_font && invalid_font) || invalid_margins {
                    record(&style);
                }
            }
        }
        issues
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

    fn text_box() -> RichTextBox {
        RichTextBox {
            text_area_type: None,
            bbox: Default::default(),
            rotation_degrees: None,
            text: "AB".into(),
            color: None,
            highlight_color: None,
            underline: false,
            font_size: None,
            runs: Vec::new(),
            spans: Vec::new(),
            paragraphs: Vec::new(),
            object_spans: Vec::new(),
            text_sections: Vec::new(),
            margins: None,
            gravity: None,
        }
    }

    fn font_span(size: f32, start: u32, end: u32) -> RichTextSpan {
        RichTextSpan {
            kind: RichTextSpanType::FontSize,
            start_utf16: start,
            end_utf16: end,
            interval_type: crate::SpanIntervalType::from(0),
            payload: size.to_le_bytes().to_vec(),
        }
    }

    fn paragraph(kind: RichTextParagraphType, payload: Vec<u8>) -> crate::RichTextParagraph {
        crate::RichTextParagraph {
            kind,
            start_paragraph: 0,
            end_paragraph: 1,
            payload,
        }
    }

    fn scaled_settings() -> TextSettings {
        TextSettings {
            scale: 3.0,
            font_size_delta: 0.0,
            ..Default::default()
        }
    }

    fn background_span(argb: u32, start: u32, end: u32) -> RichTextSpan {
        RichTextSpan {
            kind: RichTextSpanType::BackgroundColor,
            start_utf16: start,
            end_utf16: end,
            interval_type: crate::SpanIntervalType::from(0),
            payload: argb.to_le_bytes().to_vec(),
        }
    }

    #[test]
    fn caret_font_sizes_use_native_intervals_without_changing_glyph_styles() {
        let mut text = text_box();
        text.text = "A\n".into();
        text.font_size = Some(20.0);
        let theme = RenderTheme::for_canvas(false);
        for (raw, expected) in [(0, 20.0), (1, 40.0), (2, 20.0), (3, 40.0), (9, 40.0)] {
            let mut span = font_span(40.0, 0, 2);
            span.interval_type = crate::SpanIntervalType::from(raw);
            text.spans = vec![span];
            let styled = StyledText::new(&text, TextContext::Placed, TextSettings::default());
            assert_eq!(styled.font_size_at_caret(2), expected, "interval {raw}");
            assert_eq!(styled.style_at(0, theme, None).font_size, 40.0);
            assert_eq!(styled.style_at(1, theme, None).font_size, 40.0);
        }
    }

    #[test]
    fn caret_font_sizes_preserve_zero_length_priority_and_utf16_boundaries() {
        let mut text = text_box();
        text.text = "😀\n".into();
        text.font_size = Some(20.0);
        text.spans = vec![
            font_span(25.0, 0, 3),
            font_span(31.0, 2, 2),
            font_span(37.0, 1, 1),
            font_span(43.0, 4, 4),
            font_span(49.0, 3, 2),
        ];
        let styled = StyledText::new(&text, TextContext::Placed, TextSettings::default());
        assert_eq!(styled.font_size_at_caret(1), 31.0);
        assert_eq!(styled.font_size_at_caret(2), 20.0);
        assert_eq!(styled.font_size_at_caret(3), 20.0);
        assert_eq!(
            styled
                .style_at(1, RenderTheme::for_canvas(false), None)
                .font_size,
            25.0
        );
        text.spans.push(font_span(41.0, 2, 2));
        let styled = StyledText::new(&text, TextContext::Flow, TextSettings::default());
        assert_eq!(styled.font_size_at_caret(1), 41.0);
    }

    #[test]
    fn caret_font_sizes_apply_native_scaling_once_and_report_invalid_overrides() {
        let mut text = text_box();
        text.spans = vec![font_span(13.0, 2, 2)];
        let settings = TextSettings {
            scale: 2.5,
            font_size_delta: 1.0,
            ..Default::default()
        };
        let styled = StyledText::new(&text, TextContext::Flow, settings);
        assert_eq!(styled.font_size_at_caret(2), 35.0);
        assert_eq!(styled.font_size_at_caret(0), 45.0);
        let styled = StyledText::new(&text, TextContext::Placed, TextSettings::resolved());
        assert_eq!(styled.font_size_at_caret(2), 13.0);
        text.spans.push(font_span(f32::INFINITY, 2, 2));
        let styled = StyledText::new(&text, TextContext::Flow, settings);
        assert_eq!(styled.resolved_caret_font_size(2), (35.0, true));
    }

    #[test]
    fn default_caret_font_preserves_native_fma_without_changing_glyph_scaling() {
        let text = text_box();
        let settings = TextSettings {
            scale: 361.0 / 360.0,
            font_size_delta: 14.0,
            ..Default::default()
        };
        let styled = StyledText::new(&text, TextContext::Flow, settings);
        assert_eq!((styled.font_size_at_caret(0) as f32).to_bits(), 0x41f8_b05b);
        assert_eq!(
            (styled
                .style_at(0, RenderTheme::for_canvas(false), None)
                .font_size as f32)
                .to_bits(),
            0x41f8_b05c,
        );
    }

    #[test]
    fn ordered_background_spans_preserve_transparent_overrides_and_utf16_ranges() {
        let mut text = text_box();
        text.text = "A😀B".into();
        text.highlight_color = Some(Color {
            r: 255,
            g: 255,
            b: 0,
        });
        let mut malformed = background_span(0xffffffff, 1, 3);
        malformed.payload.truncate(3);
        text.spans = vec![
            background_span(0x80ffeedd, 1, 3),
            background_span(0x00123456, 1, 3),
            malformed,
            background_span(0xffffffff, 0, 8),
        ];
        let styled = StyledText::new(&text, TextContext::Placed, TextSettings::default());
        let light = RenderTheme::for_canvas(false);
        assert_eq!(styled.style_at(0, light, None).background, None);
        assert_eq!(styled.style_at(2, light, None).background, None);
        assert_eq!(
            styled.style_at(1, light, None).background,
            Some(TextBackground {
                color: Color {
                    r: 0x12,
                    g: 0x34,
                    b: 0x56
                },
                alpha: 0,
            })
        );
        assert_eq!(
            styled.style_at(0, light, None).color,
            styled.style_at(1, light, None).color
        );

        text.spans[1] = background_span(0xa0f4f4f4, 1, 3);
        let styled = StyledText::new(&text, TextContext::Placed, TextSettings::default());
        for (dark, value) in [(false, 244), (true, 11)] {
            assert_eq!(
                styled
                    .style_at(1, RenderTheme::for_canvas(dark), None)
                    .background,
                Some(TextBackground {
                    color: Color {
                        r: value,
                        g: value,
                        b: value
                    },
                    alpha: 160,
                })
            );
        }
    }

    #[test]
    fn background_boundaries_do_not_split_shared_shaping_runs() {
        let mut text = text_box();
        text.text = "AV".into();
        let settings = TextSettings::default();
        let theme = RenderTheme::for_canvas(false);
        let fonts = crate::fonts::FontBook::default();
        let renderer = TextRenderer::new(settings, &fonts);
        let measure = |text: &RichTextBox| {
            let styled = StyledText::new(text, TextContext::Placed, settings);
            measure_paragraph(&styled, 0..2, 1000.0, theme, None, &renderer)
        };
        let plain = measure(&text);
        text.spans.push(background_span(0x80ffeedd, 1, 2));
        let highlighted = measure(&text);
        assert_eq!(highlighted[0].advance, plain[0].advance);
        assert_eq!(highlighted[0].placements.len(), 2);
        assert!(std::sync::Arc::ptr_eq(
            &highlighted[0].placements[0].cluster.run,
            &highlighted[0].placements[1].cluster.run,
        ));
    }

    #[test]
    fn valid_geometry_preserves_native_scaling_and_line_height_precision() {
        let settings = TextSettings {
            scale: 2.625,
            font_size_delta: 1.125,
            ..Default::default()
        };
        let size = 17.3_f32;
        let expected_size = f64::from((size + settings.font_size_delta) * settings.scale);
        assert_eq!(settings.font_size(size), expected_size);
        assert_eq!(settings.pixels(7.3), f64::from(7.3_f32 * settings.scale));
        let spacing = ParagraphLineSpacing {
            kind: LineSpacingType::Percent,
            value: 1.17,
        };
        assert_eq!(
            paragraph_line_height(expected_size, Some(spacing), settings),
            expected_size * f64::from(spacing.value)
        );
        assert_eq!(
            paragraph_line_height(expected_size, None, settings),
            expected_size * 1.35
        );
    }

    #[test]
    fn overflowing_geometry_resolves_to_finite_defaults() {
        let settings = scaled_settings();
        assert_eq!(settings.font_size(f32::MAX), 51.0);
        assert_eq!(settings.pixels(f32::MAX), 0.0);
        for kind in [LineSpacingType::Pixels, LineSpacingType::Percent] {
            let spacing = ParagraphLineSpacing {
                kind,
                value: f32::MAX,
            };
            assert_eq!(
                paragraph_line_height(51.0, Some(spacing), settings),
                51.0 * 1.35
            );
        }
        let settings = TextSettings {
            scale: 1.0,
            font_size_delta: 0.0,
            ..Default::default()
        };
        let font_size = settings.font_size(f32::MAX);
        assert_eq!(paragraph_line_height(font_size, None, settings), font_size);
        let mut text = text_box();
        text.font_size = Some(f32::MAX);
        let styled = StyledText::new(&text, TextContext::Flow, settings);
        assert!(styled.geometry_issues().is_empty());
        let fonts = crate::fonts::FontBook::default();
        let renderer = TextRenderer::new(settings, &fonts);
        let lines = measure_paragraph(
            &styled,
            0..2,
            f64::MAX,
            RenderTheme::for_canvas(false),
            None,
            &renderer,
        );
        renderer.report_line_geometry(&lines[0], None);
        assert_eq!(
            renderer.diagnostics()[0].kind,
            TextDiagnosticKind::InvalidGeometry
        );
    }

    #[test]
    fn effective_font_overrides_determine_geometry_diagnostics() {
        let settings = scaled_settings();
        let theme = RenderTheme::for_canvas(false);
        let mut text = text_box();
        text.font_size = Some(f32::MAX);
        text.spans = vec![font_span(f32::MAX, 0, 2), font_span(12.0, 0, 2)];
        let styled = StyledText::new(&text, TextContext::Flow, settings);
        assert_eq!(styled.style_at(0, theme, None).font_size, 36.0);
        assert!(styled.geometry_issues().is_empty());
        text.spans.reverse();
        let styled = StyledText::new(&text, TextContext::Flow, settings);
        assert_eq!(styled.style_at(0, theme, None).font_size, 36.0);
        assert_eq!(styled.geometry_issues().len(), 1);
        text.spans = vec![
            font_span(12.0, 0, 2),
            font_span(f32::MAX, 3, 4),
            font_span(f32::MAX, 1, 1),
        ];
        let styled = StyledText::new(&text, TextContext::Flow, settings);
        assert!(styled.geometry_issues().is_empty());
        text.spans.clear();
        text.paragraphs = vec![paragraph(
            RichTextParagraphType::PredefinedStyle,
            vec![0; 8],
        )];
        let styled = StyledText::new(&text, TextContext::Flow, settings);
        assert!(styled.geometry_issues().is_empty());
        assert_eq!(
            styled
                .style_at(0, theme, Some(PredefinedTextStyle::Heading1))
                .font_size,
            63.0
        );
    }

    #[test]
    fn object_anchor_fonts_only_report_when_used_by_native_separator_metrics() {
        for (source, anchor, invalid) in [("\u{fffc}", 0, false), ("A\n\u{fffc}", 2, true)] {
            let mut text = text_box();
            text.text = source.into();
            text.spans = vec![font_span(f32::MAX, anchor as u32, anchor as u32 + 1)];
            text.object_spans = vec![crate::RichTextObjectSpan {
                object_type: crate::ObjectType::Image,
                object_data: Vec::new(),
                content: Some(crate::RichTextObjectContent::Image(Box::new(
                    crate::PlacedImage {
                        bbox: crate::BoundingBox {
                            x_min: 0.0,
                            y_min: 0.0,
                            x_max: 100.0,
                            y_max: 80.0,
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
            }];
            let styled = StyledText::new(&text, TextContext::Flow, scaled_settings());
            assert_eq!(!styled.geometry_issues().is_empty(), invalid);
            text.object_spans.clear();
            let styled = StyledText::new(&text, TextContext::Flow, scaled_settings());
            assert_eq!(styled.geometry_issues().len(), 1);
        }
    }

    #[test]
    fn invalid_gap_flags_follow_effective_paragraph_metadata() {
        let mut text = text_box();
        let settings = scaled_settings();
        for kind in [
            RichTextParagraphType::SpacingBefore,
            RichTextParagraphType::SpacingAfter,
        ] {
            text.paragraphs = vec![paragraph(kind, f32::MAX.to_le_bytes().to_vec())];
            let layout = paragraph_layout(&text, 0, settings);
            assert!(layout.spacing_before_invalid || layout.spacing_after_invalid);
            assert_eq!(layout.spacing_before + layout.spacing_after, 0.0);
            assert!(
                StyledText::new(&text, TextContext::Flow, settings)
                    .geometry_issues()
                    .is_empty()
            );
            text.paragraphs
                .push(paragraph(kind, 4.0_f32.to_le_bytes().to_vec()));
            let layout = paragraph_layout(&text, 0, settings);
            assert!(!layout.spacing_before_invalid && !layout.spacing_after_invalid);
            assert_eq!(layout.spacing_before + layout.spacing_after, 12.0);
            let inactive = paragraph_layout(&text, 1, settings);
            assert!(!inactive.spacing_before_invalid && !inactive.spacing_after_invalid);
            assert_eq!(inactive.spacing_before + inactive.spacing_after, 0.0);
        }
    }

    #[test]
    fn overridden_line_spacing_does_not_report_inactive_geometry() {
        let mut text = text_box();
        let settings = scaled_settings();
        for kind in [0_u32, 1] {
            let payload = |value: f32| {
                [
                    kind.to_le_bytes().as_slice(),
                    value.to_le_bytes().as_slice(),
                ]
                .concat()
            };
            text.paragraphs = vec![paragraph(
                RichTextParagraphType::LineSpacing,
                payload(f32::MAX),
            )];
            let fonts = crate::fonts::FontBook::default();
            let check = |text: &RichTextBox| {
                let styled = StyledText::new(text, TextContext::Flow, settings);
                assert!(styled.geometry_issues().is_empty());
                let renderer = TextRenderer::new(settings, &fonts);
                let lines = wrap_paragraph(
                    &styled,
                    0..2,
                    1000.0,
                    RenderTheme::for_canvas(false),
                    None,
                    &renderer,
                )
                .unwrap();
                let layout = paragraph_layout(text, 0, settings);
                for line in lines {
                    renderer.report_line_geometry(&line, layout.line_spacing);
                }
                renderer.diagnostics()
            };
            assert_eq!(check(&text).len(), 1);
            text.paragraphs
                .push(paragraph(RichTextParagraphType::LineSpacing, payload(1.5)));
            assert!(check(&text).is_empty());
        }
    }

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
                interval_type: crate::SpanIntervalType::from(0),
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
                ..Default::default()
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
