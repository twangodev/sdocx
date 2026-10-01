use std::ops::Range;

use crate::render::RenderTheme;
use crate::render::marker::{PreparedMarker, marker_center_y};
use crate::{
    BoundingBox, BulletType, LineSpacingType, ParagraphAlignment, ParagraphBullet,
    ParagraphLineSpacing, PredefinedTextStyle,
};

use super::objects::{MeasuredObject, ObjectMeasurementContext};
use super::{
    StyledText, TextRenderer, WrappedLine, explicit_line_height, paragraph_layout,
    paragraph_line_height, unmeasured_paragraph, wrap_paragraph,
};

#[derive(Clone, Copy)]
pub(in crate::render) struct VerticalExclusion {
    /// Absolute vertical coordinates in the same space as the frame's bounding box.
    pub top: f64,
    pub bottom: f64,
    pub kind: ExclusionKind,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::render) enum ExclusionKind {
    Obstacle,
    PagePadding,
}

impl VerticalExclusion {
    pub fn obstacle(top: f64, bottom: f64) -> Self {
        Self {
            top,
            bottom,
            kind: ExclusionKind::Obstacle,
        }
    }

    pub fn page_padding(top: f64, bottom: f64) -> Self {
        Self {
            top,
            bottom,
            kind: ExclusionKind::PagePadding,
        }
    }
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
    fn overlapping_band(
        &self,
        line: &WrappedLine,
        top: f64,
        advance: f64,
    ) -> Option<&VerticalExclusion> {
        let minimum_first_page_height = line
            .objects
            .iter()
            .filter_map(|placement| {
                placement
                    .prepared
                    .as_ref()
                    .and_then(|prepared| prepared.as_ref().ok())
                    .and_then(|prepared| prepared.minimum_first_page_height())
                    .or(placement.object.minimum_first_page_height)
            })
            .reduce(f64::max);
        self.exclusions.iter().find(|band| {
            band.top.is_finite()
                && band.bottom.is_finite()
                && band.top < band.bottom
                && top < band.bottom - 0.0001
                && top + advance > band.top + 0.0001
                && !(band.kind == ExclusionKind::PagePadding
                    && minimum_first_page_height.is_some_and(|minimum| {
                        minimum <= f64::from(f32::EPSILON) || minimum.ceil() <= band.top - top
                    }))
        })
    }

    fn adjusted_top_margin(&self, top: f64, before: f64, current: f64, previous: f64) -> f64 {
        if current == 0.0 && previous == 0.0 && before == 0.0 {
            return 0.0;
        }
        let probe_bottom = top as f32 - before as f32;
        let probe_top = probe_bottom - 1.0;
        let band = self.exclusions.iter().find(|band| {
            let top = band.top as f32;
            let bottom = band.bottom as f32;
            top.is_finite()
                && bottom.is_finite()
                && top < bottom
                && probe_bottom - top > 0.0001_f32
                && bottom - probe_top > 0.0001_f32
        });
        if let Some(band) = band
            && band.kind == ExclusionKind::PagePadding
        {
            let half_height = (band.bottom as f32 - band.top as f32) * 0.5;
            if half_height != 0.0 {
                return f64::from((current as f32 - half_height).max(0.0));
            }
        }
        current.max(previous)
    }
}

#[derive(Clone, Copy)]
struct LineCandidate {
    raw_top: f64,
    margin_top: f64,
}

impl LineCandidate {
    fn top(self) -> f64 {
        self.raw_top + self.margin_top
    }

    fn object_top(
        self,
        object: &MeasuredObject,
        font_size: f64,
        spacing: Option<ParagraphLineSpacing>,
        settings: super::TextSettings,
    ) -> f64 {
        if object.inline {
            return self.raw_top;
        }
        if object.top_margin > 0.0 && object.bottom_margin > 0.0 {
            return self.top();
        }
        let font_size = font_size as f32;
        let leading = match spacing {
            Some(spacing)
                if spacing.value.is_finite()
                    && spacing.value > 0.0
                    && spacing.kind == LineSpacingType::Pixels =>
            {
                settings.pixels(spacing.value) as f32
            }
            Some(spacing)
                if spacing.value.is_finite()
                    && spacing.value > 0.0
                    && spacing.kind == LineSpacingType::Percent =>
            {
                (spacing.value - 1.0) * font_size
            }
            _ => (1.35_f32 - 1.0) * font_size,
        };
        f64::from((-0.35_f32).mul_add(font_size, self.raw_top as f32 + leading))
    }
}

pub(in crate::render) struct LinePlacement {
    pub top: f64,
    pub background_top: f64,
    pub baseline: f64,
    pub bottom: f64,
    pub post_cursor: f64,
}

impl LinePlacement {
    fn translate(&mut self, offset: f64) {
        self.top += offset;
        self.background_top += offset;
        self.baseline += offset;
        self.bottom += offset;
        self.post_cursor += offset;
    }
}

struct LineMetrics {
    advance: f64,
    baseline_offset: f64,
    epsilon: f64,
}

impl LineMetrics {
    fn for_line(
        line: &WrappedLine,
        spacing: Option<ParagraphLineSpacing>,
        settings: super::TextSettings,
    ) -> Self {
        let has_objects = !line.objects.is_empty();
        let block = line.has_block_margins();
        let base_height = line.base_height();
        let advance = if block {
            base_height
        } else {
            base_height + paragraph_line_height(line.font_size, spacing, settings) - line.font_size
        };
        let epsilon = if has_objects { 0.001 } else { 0.0 };
        let baseline_offset = if block {
            base_height + epsilon
        } else {
            advance - 0.35 * line.font_size + epsilon
        };
        Self {
            advance,
            baseline_offset,
            epsilon,
        }
    }
}

pub(in crate::render) struct TextCursor {
    position: f64,
    pending_bottom: f64,
    enabled_before: f64,
}

impl TextCursor {
    pub fn new(position: f64) -> Self {
        Self {
            position,
            pending_bottom: 0.0,
            enabled_before: 0.0,
        }
    }

    pub fn add_spacing(&mut self, spacing: f64) {
        self.position += spacing;
    }

    fn begin_paragraph(&mut self, before: f64) {
        self.enabled_before = before;
        self.add_spacing(before);
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

    fn candidate(&self, line: &WrappedLine, frame: &TextFrame<'_>, raw_top: f64) -> LineCandidate {
        LineCandidate {
            raw_top,
            margin_top: frame.adjusted_top_margin(
                raw_top,
                self.enabled_before,
                line.object_margins()[0],
                self.pending_bottom,
            ),
        }
    }

    #[cfg(test)]
    fn place(
        &mut self,
        line: &WrappedLine,
        spacing: Option<ParagraphLineSpacing>,
        frame: &TextFrame<'_>,
        settings: super::TextSettings,
    ) -> LinePlacement {
        let metrics = LineMetrics::for_line(line, spacing, settings);
        let mut candidate = self.candidate(line, frame, frame.bbox.y_min + self.position);
        for _ in 0..frame.exclusions.len() {
            let Some(band) = frame.overlapping_band(line, candidate.top(), metrics.advance) else {
                break;
            };
            candidate = self.candidate(line, frame, band.bottom);
        }
        self.place_at(line, spacing, frame, settings, candidate)
    }

    fn prepare_line(
        &self,
        line: &mut WrappedLine,
        styled: &StyledText<'_>,
        frame: &TextFrame<'_>,
        theme: RenderTheme,
        renderer: &TextRenderer<'_>,
        layout: &super::ParagraphLayout,
    ) -> LineCandidate {
        let mut candidate = self.candidate(line, frame, frame.bbox.y_min + self.position);
        for _ in 0..=frame.exclusions.len() {
            prepare_line_objects(
                line,
                styled,
                |object| {
                    let style =
                        styled.style_at(object.source.start, theme, layout.predefined_style);
                    candidate.object_top(
                        object,
                        style.font_size,
                        layout.line_spacing,
                        renderer.settings,
                    )
                },
                theme,
                renderer,
            );
            let metrics = LineMetrics::for_line(line, layout.line_spacing, renderer.settings);
            let Some(band) = frame.overlapping_band(line, candidate.top(), metrics.advance) else {
                return candidate;
            };
            candidate = self.candidate(line, frame, band.bottom);
        }
        candidate
    }

    fn place_at(
        &mut self,
        line: &WrappedLine,
        spacing: Option<ParagraphLineSpacing>,
        frame: &TextFrame<'_>,
        settings: super::TextSettings,
        candidate: LineCandidate,
    ) -> LinePlacement {
        let metrics = LineMetrics::for_line(line, spacing, settings);
        let top = candidate.top();
        self.position = top - frame.bbox.y_min + metrics.advance + metrics.epsilon;
        self.pending_bottom = line.object_margins()[1];
        LinePlacement {
            top: candidate.raw_top,
            background_top: top,
            baseline: top + metrics.baseline_offset,
            bottom: frame.bbox.y_min + self.position,
            post_cursor: frame.bbox.y_min + self.position,
        }
    }
}

fn prepare_line_objects(
    line: &mut WrappedLine,
    styled: &StyledText<'_>,
    candidate_top: impl Fn(&MeasuredObject) -> f64,
    theme: RenderTheme,
    renderer: &TextRenderer<'_>,
) {
    for placement in &mut line.objects {
        let Some(span) = styled.object_span(placement.object.span_index) else {
            continue;
        };
        let candidate_top = candidate_top(&placement.object);
        let object_renderer = renderer.for_object_source(placement.object.source.clone());
        let prepared = match span.content.as_ref() {
            Some(crate::RichTextObjectContent::CodeBlock(code)) => {
                crate::render::code::prepare_code(
                    code,
                    span.layout_constraint,
                    candidate_top,
                    theme,
                    &object_renderer,
                )
                .map(|code| crate::render::embedded::PreparedObject::Code(Box::new(code)))
            }
            Some(crate::RichTextObjectContent::Table(table)) => {
                if let Some(Ok(crate::render::embedded::PreparedObject::Table(mut retained))) =
                    placement.prepared.take()
                {
                    retained
                        .relayout(table, candidate_top, theme, &object_renderer)
                        .map(|()| crate::render::embedded::PreparedObject::Table(retained))
                } else {
                    let Some(prepared) = crate::render::table::prepare_table(
                        table,
                        span.layout_constraint,
                        candidate_top,
                        theme,
                        &object_renderer,
                    ) else {
                        continue;
                    };
                    prepared.map(|table| {
                        crate::render::embedded::PreparedObject::Table(Box::new(table))
                    })
                }
            }
            _ => continue,
        };
        match &prepared {
            Ok(prepared) => {
                let height = prepared.height();
                if matches!(
                    span.layout_constraint,
                    crate::ObjectSpanLayoutConstraint::OverPages
                        | crate::ObjectSpanLayoutConstraint::OverPagesOverlapPadding
                ) && (height - placement.object.height).abs() > 0.001
                {
                    placement.object.height = height;
                }
            }
            Err(kind) => object_renderer.report_object_issues(&[super::ObjectDiagnostic {
                anchor_utf16: span.text_index_utf16,
                kind: *kind,
            }]),
        }
        if matches!(
            prepared,
            Err(super::ObjectDiagnosticKind::UnsupportedContent)
        ) && matches!(
            span.content.as_ref(),
            Some(crate::RichTextObjectContent::Table(_))
        ) {
            placement.object.height = placement.object.bounds.y_max - placement.object.bounds.y_min;
            placement.prepared = None;
        } else {
            placement.prepared = Some(prepared);
        }
    }
}

pub(in crate::render) struct TextLine {
    pub line: WrappedLine,
    pub x: f64,
    pub width: f64,
    pub baseline: f64,
    pub top: f64,
    pub background_top: f64,
    pub bottom: f64,
    pub post_cursor: f64,
    pub alignment: Option<ParagraphAlignment>,
    pub predefined: Option<PredefinedTextStyle>,
    pub marker: Option<PositionedMarker>,
}

pub(in crate::render) struct PositionedMarker {
    pub marker: PreparedMarker,
    pub source: usize,
    pub x: f64,
    pub center_y: f64,
}

impl PositionedMarker {
    #[allow(clippy::too_many_arguments)]
    pub fn for_line(
        marker: PreparedMarker,
        source: usize,
        x: f64,
        line: &WrappedLine,
        spacing: Option<ParagraphLineSpacing>,
        baseline: f64,
        post_line_cursor: f64,
        renderer: &TextRenderer<'_>,
    ) -> Option<Self> {
        let settings = renderer.settings;
        let pixels = spacing
            .filter(|spacing| spacing.kind == LineSpacingType::Pixels)
            .and_then(|spacing| {
                explicit_line_height(line.font_size, spacing, settings)
                    .map(|_| settings.pixels(spacing.value))
            })
            .unwrap_or(0.0);
        let cap_ratio = (pixels != 0.0)
            .then(|| renderer.default_cap_height_ratio())
            .flatten();
        let Some(center_y) = marker_center_y(
            baseline,
            post_line_cursor,
            line.base_height(),
            pixels,
            cap_ratio,
        ) else {
            renderer.measurement_failed("sans-serif");
            return None;
        };
        Some(Self {
            marker,
            source,
            x,
            center_y,
        })
    }
}

#[derive(Default)]
pub(in crate::render) struct TextLayout {
    pub lines: Vec<TextLine>,
    content_height: f64,
}

impl TextLayout {
    pub fn height(&self) -> f64 {
        self.content_height
    }

    pub fn translate(&mut self, dx: f64, dy: f64) {
        for line in &mut self.lines {
            line.x += dx;
            line.baseline += dy;
            line.top += dy;
            line.background_top += dy;
            line.bottom += dy;
            line.post_cursor += dy;
            if let Some(marker) = &mut line.marker {
                marker.x += dx;
                marker.center_y += dy;
            }
        }
    }

    fn apply_gravity(&mut self, gravity: Option<u8>, outer_height: f64, content_height: f64) {
        let available_height = (outer_height - content_height).max(0.0);
        let offset = match gravity {
            Some(1) => available_height / 2.0,
            Some(2) => available_height,
            _ => 0.0,
        };
        self.translate(0.0, offset);
    }
}

pub(in crate::render) fn measure_paragraph(
    styled: &StyledText<'_>,
    source: Range<usize>,
    width: f64,
    theme: RenderTheme,
    predefined: Option<PredefinedTextStyle>,
    renderer: &TextRenderer<'_>,
    object_context: ObjectMeasurementContext,
) -> Vec<WrappedLine> {
    renderer.report_object_issues(styled.objects.issues());
    if source.is_empty() {
        return vec![WrappedLine::unmeasured(
            source.clone(),
            styled.font_size_at_caret(source.start),
        )];
    }
    wrap_paragraph(
        styled,
        source.clone(),
        width,
        theme,
        predefined,
        renderer,
        object_context,
    )
    .unwrap_or_else(|_| {
        let style = styled.style_at(source.start, theme, predefined);
        renderer.measurement_failed(style.family.as_deref().unwrap_or("Roboto"));
        unmeasured_paragraph(styled, source, theme, predefined, renderer, object_context)
    })
}

pub(in crate::render) fn layout_text(
    styled: &StyledText<'_>,
    frame: TextFrame<'_>,
    theme: RenderTheme,
    renderer: &TextRenderer<'_>,
) -> TextLayout {
    layout_text_with_context(styled, frame, theme, renderer, LayoutContext::Frame, None)
}

pub(in crate::render) fn layout_text_with_size(
    styled: &StyledText<'_>,
    frame: TextFrame<'_>,
    measurement_size: [i32; 2],
    theme: RenderTheme,
    renderer: &TextRenderer<'_>,
) -> TextLayout {
    layout_text_with_context(
        styled,
        frame,
        theme,
        renderer,
        LayoutContext::Frame,
        Some(measurement_size),
    )
}

pub(in crate::render) fn layout_flow_text(
    styled: &StyledText<'_>,
    frame: TextFrame<'_>,
    theme: RenderTheme,
    renderer: &TextRenderer<'_>,
) -> TextLayout {
    layout_text_with_context(styled, frame, theme, renderer, LayoutContext::Flow, None)
}

pub(in crate::render) fn layout_capture_text(
    styled: &StyledText<'_>,
    frame: TextFrame<'_>,
    theme: RenderTheme,
    renderer: &TextRenderer<'_>,
) -> TextLayout {
    layout_text_with_context(styled, frame, theme, renderer, LayoutContext::Capture, None)
}

#[derive(Clone, Copy)]
enum LayoutContext {
    Frame,
    Flow,
    Capture,
}

impl LayoutContext {
    fn object_measurement(self) -> ObjectMeasurementContext {
        match self {
            Self::Frame => ObjectMeasurementContext::Frame,
            Self::Flow | Self::Capture => ObjectMeasurementContext::Body,
        }
    }

    fn continuation_top(
        self,
        paragraph_number: usize,
        line_number: usize,
        line: &WrappedLine,
    ) -> Option<f64> {
        if matches!(self, Self::Flow)
            && paragraph_number == 0
            && line_number == 0
            && let [object] = line.objects.as_slice()
            && !object.object.inline
            && object.object.bounds.y_min < 0.0
        {
            Some(object.object.bounds.y_min)
        } else {
            None
        }
    }
}

fn empty_gravity_height(styled: &StyledText<'_>, margins: [f64; 4]) -> f64 {
    let height = styled.caret_line_height(0) as f32;
    f64::from((height + margins[1] as f32) + margins[3] as f32)
}

fn layout_text_with_context(
    styled: &StyledText<'_>,
    frame: TextFrame<'_>,
    theme: RenderTheme,
    renderer: &TextRenderer<'_>,
    context: LayoutContext,
    measurement_size: Option<[i32; 2]>,
) -> TextLayout {
    let text_box = styled.text_box;
    let settings = renderer.settings;
    renderer.report_object_issues(styled.object_issues());
    renderer.report_geometry_issues(styled.geometry_issues());
    let margins = text_box
        .margins
        .unwrap_or([0.0; 4])
        .map(|margin| settings.pixels(margin));
    let [outer_width, outer_height] = measurement_size.map_or_else(
        || {
            [
                (frame.bbox.x_max - frame.bbox.x_min).ceil(),
                (frame.bbox.y_max - frame.bbox.y_min).ceil(),
            ]
        },
        |size| size.map(f64::from),
    );
    let content_left = frame.bbox.x_min + margins[0];
    let content_width = outer_width - margins[0] - margins[2];
    let paragraphs = styled.index.display_paragraphs().collect::<Vec<_>>();
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
        let paragraph_renderer = renderer.for_source(paragraph.content.clone());
        let renderer = &paragraph_renderer;
        let marker_renderer = renderer.for_source(
            paragraph.content.start..(paragraph.content.start + 1).min(styled.index.len()),
        );
        let [left_indent, right_indent] = layout.indent_insets(settings);
        let marker_x = content_left + left_indent;
        let marker_style = styled.style_at(paragraph.content.start, theme, layout.predefined_style);
        let mut marker = layout.bullet.and_then(|bullet| {
            PreparedMarker::prepare(
                bullet,
                layout.indent_level,
                &marker_style,
                theme,
                &marker_renderer,
            )
        });
        let marker_width = marker.as_ref().map_or(0.0, PreparedMarker::reserved_width);
        let x = marker_x + marker_width;
        let width = (content_width - left_indent - right_indent - marker_width).max(0.0);
        let paragraph_lines = measure_paragraph(
            styled,
            paragraph.content.clone(),
            width,
            theme,
            layout.predefined_style,
            renderer,
            context.object_measurement(),
        );
        let previous = match context {
            LayoutContext::Flow | LayoutContext::Capture => styled
                .index
                .paragraph_index(paragraph.content.start)
                .and_then(|ordinal| ordinal.checked_sub(1))
                .and_then(|ordinal| paragraph_layout(text_box, ordinal, settings).bullet),
            LayoutContext::Frame => paragraph_number
                .checked_sub(1)
                .and_then(|previous| paragraph_layouts.get(previous))
                .and_then(|layout| layout.bullet),
        };
        let next = paragraph_layouts
            .get(paragraph_number + 1)
            .and_then(|layout| layout.bullet);
        let spacing = ParagraphSpacing::for_lines(&paragraph_lines, previous, layout.bullet, next);
        if spacing.before && layout.spacing_before_invalid {
            let style = styled.style_at(paragraph.content.start, theme, layout.predefined_style);
            renderer.invalid_geometry(style.family.as_deref().unwrap_or("Roboto"));
        }
        cursor.begin_paragraph(if spacing.before {
            layout.spacing_before
        } else {
            0.0
        });
        for (line_number, mut line) in paragraph_lines.into_iter().enumerate() {
            let continuation_top = context.continuation_top(paragraph_number, line_number, &line);
            let settled_top = if let Some(top) = continuation_top {
                prepare_line_objects(&mut line, styled, |_| top, theme, renderer);
                let metrics = LineMetrics::for_line(&line, layout.line_spacing, settings);
                let mut candidate =
                    cursor.candidate(&line, &frame, frame.bbox.y_min + cursor.position);
                for _ in 0..frame.exclusions.len() {
                    let Some(band) =
                        frame.overlapping_band(&line, candidate.top(), metrics.advance)
                    else {
                        break;
                    };
                    candidate = cursor.candidate(&line, &frame, band.bottom);
                }
                candidate
            } else {
                cursor.prepare_line(&mut line, styled, &frame, theme, renderer, layout)
            };
            if !line.objects.is_empty() && line.position_native(styled).is_err() {
                let style = styled.style_at(line.source.start, theme, layout.predefined_style);
                renderer.invalid_geometry(style.family.as_deref().unwrap_or("Roboto"));
            }
            if layout.alignment == Some(ParagraphAlignment::Both)
                && line.justify(styled, width).is_err()
            {
                let style = styled.style_at(line.source.start, theme, layout.predefined_style);
                renderer.invalid_geometry(style.family.as_deref().unwrap_or("Roboto"));
            }
            renderer.report_line_geometry(&line, layout.line_spacing);
            let mut placement =
                cursor.place_at(&line, layout.line_spacing, &frame, settings, settled_top);
            if let Some(top) = continuation_top
                && let [object] = line.objects.as_slice()
            {
                let correction = top + object.object.height - placement.baseline;
                cursor.add_spacing(correction);
                placement.translate(correction);
            }
            let positioned_marker = marker.take().and_then(|marker| {
                PositionedMarker::for_line(
                    marker,
                    paragraph.content.start,
                    marker_x + super::line_alignment_offset(line.advance, width, layout.alignment),
                    &line,
                    layout.line_spacing,
                    placement.baseline,
                    placement.post_cursor,
                    &marker_renderer,
                )
            });
            lines.push(TextLine {
                line,
                x,
                width,
                baseline: placement.baseline,
                top: placement.top,
                background_top: placement.background_top,
                bottom: placement.bottom,
                post_cursor: placement.post_cursor,
                alignment: layout.alignment,
                predefined: layout.predefined_style,
                marker: positioned_marker,
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
        0.0
    } else {
        cursor.height_with_bottom(margins[3])
    };
    let gravity_height = if styled.index.is_empty() {
        empty_gravity_height(styled, margins)
    } else {
        content_height
    };
    let mut layout = TextLayout {
        lines,
        content_height,
    };
    layout.apply_gravity(frame.gravity, outer_height, gravity_height);
    layout
}

#[cfg(test)]
mod tests {
    use super::super::objects::MeasuredObject;
    use super::super::wrapping::PositionedObject;
    use super::super::{TextContext, TextSettings};
    use super::*;
    use crate::fonts::FontBook;
    use crate::{
        ObjectSpanLayoutConstraint, RichTextBox, RichTextParagraph, RichTextParagraphType,
    };

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
        measure_with_context(text, exclusions, LayoutContext::Frame)
    }

    fn measure_with_context(
        text: &RichTextBox,
        exclusions: &[VerticalExclusion],
        context: LayoutContext,
    ) -> TextLayout {
        let settings = TextSettings {
            scale: 1.0,
            font_size_delta: 0.0,
            ..Default::default()
        };
        let fonts = FontBook::default();
        let renderer = TextRenderer::new(settings, &fonts);
        let styled = StyledText::new(text, TextContext::Placed, settings);
        layout_text_with_context(
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
            context,
            None,
        )
    }

    #[test]
    fn display_paragraphs_retain_each_native_separator_line() {
        for (value, sources) in [
            ("A\n", vec![0..1, 2..2]),
            ("\nA", vec![0..0, 1..2]),
            ("\n\n", vec![0..0, 1..1, 2..2]),
            ("A\r\nB", vec![0..1, 2..2, 3..4]),
        ] {
            let mut content = text(value);
            content.font_size = Some(20.0);
            for context in [
                LayoutContext::Frame,
                LayoutContext::Flow,
                LayoutContext::Capture,
            ] {
                let plan = measure_with_context(&content, &[], context);
                assert_eq!(
                    plan.lines
                        .iter()
                        .map(|line| line.line.source.clone())
                        .collect::<Vec<_>>(),
                    sources,
                    "{value:?}",
                );
                assert_eq!(plan.height(), sources.len() as f64 * 27.0, "{value:?}");
                for (index, line) in plan.lines.iter().enumerate() {
                    assert_eq!(line.baseline, 120.0 + index as f64 * 27.0, "{value:?}");
                }
            }
        }
    }

    #[test]
    fn separator_only_lines_use_caret_fonts_without_changing_content_prefix_fonts() {
        let mut content = text("A\n");
        content.font_size = Some(20.0);
        content.spans.push(crate::RichTextSpan {
            kind: crate::RichTextSpanType::FontSize,
            start_utf16: 2,
            end_utf16: 2,
            interval_type: crate::SpanIntervalType::ClosedOpen,
            payload: 60.0_f32.to_le_bytes().to_vec(),
        });
        let plan = measure(&content, &[]);
        assert_eq!(plan.lines[0].line.font_size, 20.0);
        assert_eq!(plan.lines[1].line.font_size, 60.0);
        assert_eq!(plan.lines[1].baseline, 187.0);
        assert_eq!(plan.height(), 108.0);

        content.text.push('B');
        let plan = measure(&content, &[]);
        assert_eq!(plan.lines[1].line.font_size, 20.0);
        assert_eq!(plan.lines[1].baseline, 147.0);
        assert_eq!(plan.height(), 54.0);
    }

    #[test]
    fn initial_empty_paragraph_uses_the_start_caret_font() {
        let mut content = text("\nA");
        content.font_size = Some(20.0);
        content.spans.push(crate::RichTextSpan {
            kind: crate::RichTextSpanType::FontSize,
            start_utf16: 0,
            end_utf16: 0,
            interval_type: crate::SpanIntervalType::ClosedOpen,
            payload: 40.0_f32.to_le_bytes().to_vec(),
        });
        let plan = measure(&content, &[]);
        assert_eq!(plan.lines[0].line.font_size, 40.0);
        assert_eq!(plan.lines[0].baseline, 140.0);
        assert_eq!(plan.lines[1].baseline, 174.0);
        assert_eq!(plan.height(), 81.0);
    }

    #[test]
    fn entirely_empty_text_has_no_measured_lines_or_height() {
        let mut content = text("");
        content.font_size = Some(20.0);
        content.margins = Some([2.0, 3.0, 5.0, 7.0]);
        for gravity in [None, Some(1), Some(2)] {
            content.gravity = gravity;
            let plan = measure(&content, &[]);
            assert!(plan.lines.is_empty());
            assert_eq!(plan.height(), 0.0);
        }
    }

    #[test]
    fn empty_gravity_height_uses_native_spacing_and_f32_addition_order() {
        let settings = TextSettings {
            scale: 1.0,
            font_size_delta: 0.0,
            ..Default::default()
        };
        let mut content = text("");
        content.font_size = Some(20.0);
        let height = |content: &RichTextBox, margins| {
            let styled = StyledText::new(content, TextContext::Placed, settings);
            empty_gravity_height(&styled, margins)
        };
        assert_eq!(height(&content, [0.0; 4]), 27.0);
        assert_eq!(height(&content, [0.0, 3.0, 0.0, 7.0]), 37.0);
        content.paragraphs.push(RichTextParagraph {
            kind: RichTextParagraphType::LineSpacing,
            start_paragraph: 0,
            end_paragraph: 1,
            payload: [1_u32.to_le_bytes(), 1.6_f32.to_le_bytes()].concat(),
        });
        assert_eq!(height(&content, [0.0; 4]), 32.0);
        content.paragraphs[0].payload = [0_u32.to_le_bytes(), 7.0_f32.to_le_bytes()].concat();
        assert_eq!(height(&content, [0.0; 4]), 27.0);

        content.font_size = Some(16_777_216.0);
        content.paragraphs[0].payload = [1_u32.to_le_bytes(), 1.0_f32.to_le_bytes()].concat();
        assert_eq!(height(&content, [0.0, 1.0, 0.0, 1.0]), 16_777_216.0);

        content.font_size = Some(20.0);
        content.paragraphs[0].payload =
            [0_u32.to_le_bytes(), f32::MIN_POSITIVE.to_le_bytes()].concat();
        let settings = TextSettings {
            scale: f32::MIN_POSITIVE,
            ..TextSettings::resolved()
        };
        let styled = StyledText::new(&content, TextContext::Flow, settings);
        assert_eq!(empty_gravity_height(&styled, [0.0; 4]), 27.0);
    }

    #[test]
    fn native_measurement_width_is_independent_of_a_large_world_origin() {
        let content = text("ABCD");
        let settings = TextSettings::default();
        let fonts = FontBook::default();
        let renderer = TextRenderer::new(settings, &fonts);
        let styled = StyledText::new(&content, TextContext::Placed, settings);
        let origin = 1e30;
        let width = i32::MAX;
        let collapsed_endpoint = origin + f64::from(width);
        assert_eq!(collapsed_endpoint, origin);
        let frame = |right| TextFrame {
            bbox: BoundingBox {
                x_min: origin,
                y_min: 0.0,
                x_max: right,
                y_max: 200.0,
            },
            gravity: None,
            exclusions: &[],
        };
        let theme = RenderTheme::for_canvas(false);
        let collapsed = layout_text(&styled, frame(collapsed_endpoint), theme, &renderer);
        assert_eq!(collapsed.lines.len(), 4);
        let native =
            layout_text_with_size(&styled, frame(2.0 * origin), [width, 200], theme, &renderer);
        assert_eq!(native.lines.len(), 1);
        assert_eq!(native.lines[0].line.source, 0..4);
        assert_eq!(native.lines[0].width, f64::from(width));
        assert_eq!(native.lines[0].x, origin);
    }

    #[test]
    fn native_measurement_height_controls_gravity_without_reconstructing_bounds() {
        let content = text("A");
        let settings = TextSettings::default();
        let fonts = FontBook::default();
        let renderer = TextRenderer::new(settings, &fonts);
        let styled = StyledText::new(&content, TextContext::Placed, settings);
        let native = layout_text_with_size(
            &styled,
            TextFrame {
                bbox: BoundingBox {
                    x_min: 0.0,
                    y_min: 0.0,
                    x_max: 100.0,
                    y_max: 1e30,
                },
                gravity: Some(2),
                exclusions: &[],
            },
            [100, 200],
            RenderTheme::for_canvas(false),
            &renderer,
        );
        assert_eq!(native.height(), 13.5);
        assert_eq!(native.lines[0].baseline, 196.5);
        assert_eq!(native.lines[0].bottom, 200.0);
    }

    #[test]
    fn touching_edges_and_sub_tolerance_overlap_do_not_move_lines() {
        for bands in [
            Vec::new(),
            vec![VerticalExclusion::obstacle(80.0, 100.0)],
            vec![VerticalExclusion::obstacle(113.5, 130.0)],
            vec![VerticalExclusion::obstacle(113.49995, 130.0)],
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
            let plan = measure(&text("ABC"), &[VerticalExclusion::obstacle(top, 130.0)]);
            assert_eq!(plan.lines[0].baseline, 140.0);
            assert_eq!(plan.height(), 43.5);
        }
    }

    #[test]
    fn unsorted_bands_are_rechecked_after_each_retry() {
        let plan = measure(
            &text("ABC\nDEF"),
            &[
                VerticalExclusion::obstacle(130.0, 145.0),
                VerticalExclusion::obstacle(105.0, 125.0),
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
        let plan = measure(&text("ABC"), &[VerticalExclusion::obstacle(90.0, 120.0)]);
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
        let blocked = measure(&content, &[VerticalExclusion::obstacle(110.0, 130.0)]);
        assert_eq!(blocked.lines[0].baseline, 140.0);
        assert_eq!(blocked.height(), 46.5);
    }

    #[test]
    fn exclusions_preserve_empty_height_and_measured_gravity() {
        let bands = [VerticalExclusion::obstacle(105.0, 120.0)];
        let mut empty = text("");
        empty.margins = Some([0.0, 2.0, 0.0, 3.0]);
        empty.gravity = Some(1);
        let plan = measure(&empty, &bands);
        assert!(plan.lines.is_empty());
        assert_eq!(plan.height(), 0.0);
        empty.margins = Some([0.0, 2.0, 0.0, -3.0]);
        assert_eq!(measure(&empty, &bands).height(), 0.0);

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
                left_margin: 0.0,
                right_margin: 0.0,
                top_margin: margins[0],
                bottom_margin: margins[1],
                minimum_first_page_height: None,
            },
            x: 0.0,
            visual_rank: 0,
            prepared: None,
        });
        line
    }

    fn place_object(cursor: &mut TextCursor, line: &WrappedLine) -> f64 {
        cursor
            .place(
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
                    ..Default::default()
                },
            )
            .baseline
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

    fn add_point_bullet(content: &mut RichTextBox) {
        content.paragraphs.push(RichTextParagraph {
            kind: RichTextParagraphType::Bullet,
            start_paragraph: 0,
            end_paragraph: u32::MAX,
            payload: [8_u32, 1, 0, 1]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect(),
        });
    }

    #[test]
    fn marker_reservation_precedes_wrapping_and_only_the_first_line_retains_it() {
        let mut content = text("ABC");
        add_point_bullet(&mut content);
        content.margins = Some([0.0, 0.0, 990.0, 0.0]);
        let plan = measure(&content, &[]);
        assert_eq!(plan.lines.len(), 3);
        assert_eq!(plan.height(), 40.5);
        for line in &plan.lines {
            assert_eq!(line.x, 36.0);
            assert_eq!(line.width, 0.0);
        }
        let marker = plan.lines[0].marker.as_ref().unwrap();
        assert_eq!(marker.x, 10.0);
        assert_eq!(marker.source, 0);
        assert_eq!(marker.center_y, 106.75);
        assert!(plan.lines[1..].iter().all(|line| line.marker.is_none()));
    }

    #[test]
    fn marker_centers_share_the_frame_origin_and_gravity_translation() {
        let mut content = text("ABC");
        add_point_bullet(&mut content);
        for (gravity, expected_center) in [(None, 106.75), (Some(1), 150.0), (Some(2), 193.25)] {
            content.gravity = gravity;
            let plan = measure(&content, &[]);
            let marker = plan.lines[0].marker.as_ref().unwrap();
            assert_eq!(marker.center_y, expected_center);
            assert_eq!(plan.lines[0].baseline - marker.center_y, 3.25);
            assert_eq!(plan.lines[0].top, expected_center - 6.75);
            assert_eq!(plan.lines[0].bottom, expected_center + 6.75);
            assert_eq!(plan.lines[0].post_cursor, plan.lines[0].bottom);
            assert_eq!(plan.height(), 13.5);
        }
    }

    #[test]
    fn later_paragraph_marker_uses_first_content_font_instead_of_line_maximum() {
        let mut content = text("A\nBC");
        add_point_bullet(&mut content);
        content.spans = [(2, 3, 2.0_f32), (3, 4, 20.0_f32)]
            .into_iter()
            .map(|(start, end, size)| crate::RichTextSpan {
                kind: crate::RichTextSpanType::FontSize,
                start_utf16: start,
                end_utf16: end,
                interval_type: crate::SpanIntervalType::from(0),
                payload: size.to_le_bytes().to_vec(),
            })
            .collect();
        let plan = measure(&content, &[]);
        let line = &plan.lines[1];
        assert_eq!(line.line.font_size, 20.0);
        let marker = line.marker.as_ref().unwrap();
        assert_eq!(marker.source, 2);
        let fonts = FontBook::default();
        let renderer = TextRenderer::new(
            TextSettings {
                scale: 1.0,
                font_size_delta: 0.0,
                ..Default::default()
            },
            &fonts,
        );
        let mut scene = crate::render::vector::Scene::new(crate::render::vector::Svg::new());
        marker
            .marker
            .paint(
                &mut scene,
                marker.x,
                marker.center_y,
                "#000000",
                RenderTheme::for_canvas(false),
                &renderer,
            )
            .unwrap();
        let svg = scene.finish();
        let document = roxmltree::Document::parse(&svg).unwrap();
        let radius = document
            .descendants()
            .find(|node| node.has_tag_name("circle"))
            .unwrap()
            .attribute("r")
            .unwrap()
            .parse::<f64>()
            .unwrap();
        assert_eq!(radius, 1.0);
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
    fn all_layout_contexts_keep_terminal_carriage_return_paragraphs() {
        for (source, sources) in [
            ("A\r", vec![0..1, 2..2]),
            ("A\r\r\n", vec![0..1, 2..2, 3..3, 4..4]),
        ] {
            let content = text(source);
            let placed = measure(&content, &[]);
            let flow = measure_with_context(&content, &[], LayoutContext::Flow);
            for plan in [placed, flow] {
                assert_eq!(
                    plan.lines
                        .iter()
                        .map(|line| line.line.source.clone())
                        .collect::<Vec<_>>(),
                    sources,
                );
                assert_eq!(plan.height(), sources.len() as f64 * 13.5);
            }
        }
    }

    #[test]
    fn flow_spacing_retains_the_immediate_native_crlf_predecessor() {
        let mut content = text("A\r\nB");
        for ordinal in [0, 2] {
            content.paragraphs.push(RichTextParagraph {
                kind: RichTextParagraphType::Bullet,
                start_paragraph: ordinal,
                end_paragraph: ordinal + 1,
                payload: [8_u32, 1, 0, 1]
                    .into_iter()
                    .flat_map(u32::to_le_bytes)
                    .collect(),
            });
        }
        content.paragraphs.push(RichTextParagraph {
            kind: RichTextParagraphType::SpacingBefore,
            start_paragraph: 2,
            end_paragraph: 3,
            payload: 5.0_f32.to_le_bytes().to_vec(),
        });
        let placed = measure(&content, &[]);
        let flow = measure_with_context(&content, &[], LayoutContext::Flow);
        close(placed.lines[0].baseline, 110.0);
        close(flow.lines[0].baseline, 110.0);
        close(placed.lines[1].baseline, 123.5);
        close(flow.lines[1].baseline, 123.5);
        close(placed.lines[2].baseline, 142.0);
        close(flow.lines[2].baseline, 142.0);
    }

    #[test]
    fn object_measurement_margins_follow_layout_ownership_instead_of_text_style_context() {
        let mut content = text("A\u{fffc}B");
        content.font_size = Some(45.0);
        content.object_spans.push(crate::RichTextObjectSpan {
            object_type: crate::ObjectType::Image,
            object_data: Vec::new(),
            content: Some(crate::RichTextObjectContent::Image(Box::new(
                crate::PlacedImage {
                    bbox: BoundingBox {
                        x_min: 500.0,
                        y_min: 700.0,
                        x_max: 540.0,
                        y_max: 760.0,
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
            text_index_utf16: 1,
            layout_option: crate::ObjectSpanLayoutOption::Inline,
            layout_constraint: ObjectSpanLayoutConstraint::Normal,
        });
        let settings = TextSettings {
            scale: 1.0,
            font_size_delta: 0.0,
            ..Default::default()
        };
        let fonts = FontBook::default();
        let renderer = TextRenderer::new(settings, &fonts);
        for style_context in [TextContext::Flow, TextContext::Placed] {
            let styled = StyledText::new(&content, style_context, settings);
            for (context, margin) in [
                (LayoutContext::Frame, 0.0),
                (LayoutContext::Flow, 4.0),
                (LayoutContext::Capture, 4.0),
            ] {
                let plan = layout_text_with_context(
                    &styled,
                    TextFrame {
                        bbox: BoundingBox {
                            x_min: 10.0,
                            y_min: 100.0,
                            x_max: 1010.0,
                            y_max: 1000.0,
                        },
                        gravity: None,
                        exclusions: &[],
                    },
                    RenderTheme::for_canvas(false),
                    &renderer,
                    context,
                    None,
                );
                assert_eq!(plan.lines.len(), 1);
                let line = &plan.lines[0].line;
                assert!(line.native_positioned);
                assert_eq!(line.objects[0].object.bounds.x_min, 500.0);
                assert_eq!(line.objects[0].x, 29.35546875 + margin);
                assert_eq!(line.placements[1].x, 69.35546875 + 2.0 * margin);
            }
        }
    }

    #[test]
    fn only_a_first_flow_block_object_uses_its_saved_negative_origin() {
        for (source, anchor, option, continues) in [
            ("\u{fffc}\nB", 0, crate::ObjectSpanLayoutOption::Block, true),
            (
                "\u{fffc}\nB",
                0,
                crate::ObjectSpanLayoutOption::Inline,
                false,
            ),
            (
                "A\n\u{fffc}\nB",
                2,
                crate::ObjectSpanLayoutOption::Block,
                false,
            ),
        ] {
            let mut content = text(source);
            content.object_spans.push(crate::RichTextObjectSpan {
                object_type: crate::ObjectType::Image,
                object_data: Vec::new(),
                content: Some(crate::RichTextObjectContent::Image(Box::new(
                    crate::PlacedImage {
                        bbox: BoundingBox {
                            x_min: 0.0,
                            y_min: -20.0,
                            x_max: 50.0,
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
                layout_option: option,
                layout_constraint: ObjectSpanLayoutConstraint::Normal,
            });
            let settings = TextSettings::default();
            let fonts = FontBook::default();
            let renderer = TextRenderer::new(settings, &fonts);
            let styled = StyledText::new(&content, TextContext::Placed, settings);
            let frame = || TextFrame {
                bbox: BoundingBox {
                    x_min: 0.0,
                    y_min: 0.0,
                    x_max: 1000.0,
                    y_max: 1000.0,
                },
                gravity: None,
                exclusions: &[],
            };
            let theme = RenderTheme::for_canvas(false);
            let placed = layout_text(&styled, frame(), theme, &renderer);
            let flow = layout_flow_text(&styled, frame(), theme, &renderer);
            let capture = layout_capture_text(&styled, frame(), theme, &renderer);
            assert_eq!(placed.lines.len(), flow.lines.len());
            assert_eq!(placed.lines.len(), capture.lines.len());
            for (placed, capture) in placed.lines.iter().zip(&capture.lines) {
                assert_eq!(placed.baseline, capture.baseline);
            }
            if continues {
                close(placed.lines[0].baseline, 100.001);
                close(flow.lines[0].baseline, 80.0);
                close(flow.lines[0].top, -20.001);
                close(flow.lines[0].bottom, 83.5);
                close(flow.lines[0].post_cursor, 83.5);
                close(placed.lines[1].baseline, 113.501);
                close(flow.lines[1].baseline, 93.5);
                close(flow.height(), 97.0);
            } else {
                for (placed, flow) in placed.lines.iter().zip(&flow.lines) {
                    assert_eq!(placed.baseline, flow.baseline);
                }
                assert_eq!(placed.height(), flow.height());
            }
        }
    }

    #[test]
    fn full_source_object_layout_preserves_inherited_separator_font_metrics() {
        for (source, anchor, object_line_index, expected_font, next_baseline) in [
            ("A\n\u{fffc}\nB", 2, 1, 45.0, 321.501),
            ("\u{fffc}\nB", 0, 0, 45.0, 260.751),
        ] {
            let mut content = text(source);
            content.font_size = Some(45.0);
            content.object_spans.push(crate::RichTextObjectSpan {
                object_type: crate::ObjectType::Image,
                object_data: Vec::new(),
                content: Some(crate::RichTextObjectContent::Image(Box::new(
                    crate::PlacedImage {
                        bbox: BoundingBox {
                            x_min: 0.0,
                            y_min: 0.0,
                            x_max: 50.0,
                            y_max: 100.0,
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
                layout_option: crate::ObjectSpanLayoutOption::Block,
                layout_constraint: ObjectSpanLayoutConstraint::OverPages,
            });
            let plan = measure(&content, &[]);
            let object_line = &plan.lines[object_line_index];
            assert_eq!(object_line.line.font_size, expected_font);
            assert_eq!(
                object_line.line.source,
                anchor as usize..anchor as usize + 1
            );
            close(plan.lines.last().unwrap().baseline, next_baseline);
        }
    }

    #[test]
    fn moving_a_parent_code_line_remeasures_child_splits_at_its_final_candidate() {
        let document = crate::Document {
            pages: (0..3)
                .map(|index| crate::Page {
                    uuid: format!("page-{index}"),
                    width: 1080,
                    height: 500,
                    content_bbox: BoundingBox::default(),
                    background_color: None,
                    template: None,
                    background: Default::default(),
                    objects: Vec::new(),
                })
                .collect(),
            metadata: crate::DocumentMetadata {
                page_mode: Some(0),
                default_page_dimensions: Some((1080, 500)),
                ..Default::default()
            },
        };
        let settings = TextSettings::from_document(&document.metadata);
        let fonts = FontBook::default();
        let pages = super::super::PageExclusions::for_document(&document, settings).unwrap();
        let bands = pages.line_bands();
        let renderer = TextRenderer::new(settings, &fonts).with_page_exclusions(Some(pages));
        let mut title = text("language");
        title.font_size = Some(15.0);
        let mut body = text("A\nB\nC\nD\nE\nF");
        body.font_size = Some(15.0);
        let code = crate::RichTextCodeBlock {
            bbox: BoundingBox {
                x_min: 0.0,
                y_min: 0.0,
                x_max: 600.0,
                y_max: 10.0,
            },
            rotation_degrees: None,
            title: Some(title),
            body: Some(body),
        };
        let theme = RenderTheme::for_canvas(false);
        let initial = crate::render::code::prepare_code(
            &code,
            ObjectSpanLayoutConstraint::OverPages,
            420.0,
            theme,
            &renderer,
        )
        .unwrap();
        close(initial.panel_bbox.y_max - initial.panel_bbox.y_min, 556.5);
        let mut content = text("\u{fffc}\nZ");
        content.font_size = Some(15.0);
        content.margins = Some([0.0, 140.0, 0.0, 0.0]);
        content.object_spans.push(crate::RichTextObjectSpan {
            object_type: crate::ObjectType::CodeBlock,
            object_data: Vec::new(),
            content: Some(crate::RichTextObjectContent::CodeBlock(Box::new(code))),
            text_index_utf16: 0,
            layout_option: crate::ObjectSpanLayoutOption::Block,
            layout_constraint: ObjectSpanLayoutConstraint::OverPages,
        });
        let styled = StyledText::new(&content, TextContext::Flow, settings);
        let plan = layout_capture_text(
            &styled,
            TextFrame {
                bbox: BoundingBox {
                    x_min: 0.0,
                    y_min: 0.0,
                    x_max: 1080.0,
                    y_max: 0.0,
                },
                gravity: None,
                exclusions: &bands,
            },
            theme,
            &renderer,
        );
        let line = &plan.lines[0];
        let prepared = line.line.objects[0]
            .prepared
            .as_ref()
            .unwrap()
            .as_ref()
            .unwrap();
        let crate::render::embedded::PreparedObject::Code(prepared) = prepared else {
            panic!("expected a prepared code block");
        };
        close(line.top, 530.0);
        close(prepared.panel_bbox.y_min, 530.0);
        close(prepared.panel_bbox.y_max, 1150.75);
        close(line.baseline, 1150.751);
        close(line.line.objects[0].object.height, 620.75);
        let body = prepared.body_layout.as_ref().unwrap();
        close(body.lines[0].baseline, 707.0);
        close(body.lines[5].baseline, 1075.0);
        close(plan.lines[1].baseline, 1211.501);
    }

    #[test]
    fn prepared_code_may_cross_only_padding_when_its_first_page_minimum_fits() {
        let fonts = FontBook::default();
        let settings = TextSettings {
            scale: 3.0,
            ..Default::default()
        };
        let renderer = TextRenderer::new(settings, &fonts);
        let mut child_text = text("A");
        child_text.font_size = Some(15.0);
        let code = crate::RichTextCodeBlock {
            bbox: BoundingBox {
                x_min: 0.0,
                y_min: 0.0,
                x_max: 600.0,
                y_max: 10.0,
            },
            rotation_degrees: None,
            title: Some(child_text.clone()),
            body: Some(child_text),
        };
        for (constraint, obstacle, candidate, expected_baseline) in [
            (ObjectSpanLayoutConstraint::OverPages, false, 18.0, 270.751),
            (ObjectSpanLayoutConstraint::OverPages, false, 18.25, 512.751),
            (ObjectSpanLayoutConstraint::OverPages, true, 18.0, 512.751),
            (ObjectSpanLayoutConstraint::Normal, false, 18.0, 512.751),
        ] {
            let prepared = crate::render::code::prepare_code(
                &code,
                constraint,
                candidate,
                RenderTheme::for_canvas(false),
                &renderer,
            )
            .unwrap();
            assert_eq!(prepared.min_first_page_height, 181.5);
            let mut line = object_line(0.0, false, [0.0; 2]);
            line.objects[0].object.height = prepared.panel_bbox.y_max - prepared.panel_bbox.y_min;
            line.objects[0].prepared = Some(Ok(crate::render::embedded::PreparedObject::Code(
                Box::new(prepared),
            )));
            let bands = [if obstacle {
                VerticalExclusion::obstacle(200.0, 260.0)
            } else {
                VerticalExclusion::page_padding(200.0, 260.0)
            }];
            let frame = TextFrame {
                bbox: BoundingBox::default(),
                gravity: None,
                exclusions: &bands,
            };
            let mut cursor = TextCursor::new(candidate);
            let placement = cursor.place(&line, None, &frame, settings);
            close(placement.baseline, expected_baseline);
            close(placement.bottom, placement.post_cursor);
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
    fn child_callbacks_use_the_object_anchor_font_and_native_layout_option() {
        let fonts = FontBook::default();
        let settings = TextSettings {
            scale: 1.0,
            ..Default::default()
        };
        let renderer = TextRenderer::new(settings, &fonts);
        let theme = RenderTheme::for_canvas(false);
        for (option, kind, spacing, object_font, expected) in [
            (
                crate::ObjectSpanLayoutOption::Block,
                LineSpacingType::Percent,
                1.6,
                20.0,
                105.0,
            ),
            (
                crate::ObjectSpanLayoutOption::Block,
                LineSpacingType::Pixels,
                20.0,
                20.0,
                113.0,
            ),
            (
                crate::ObjectSpanLayoutOption::Block,
                LineSpacingType::Percent,
                2.0,
                40.0,
                126.0,
            ),
            (
                crate::ObjectSpanLayoutOption::Block,
                LineSpacingType::Percent,
                2.0,
                10.0,
                106.5,
            ),
            (
                crate::ObjectSpanLayoutOption::Block,
                LineSpacingType::Pixels,
                9.0,
                40.0,
                95.0,
            ),
            (
                crate::ObjectSpanLayoutOption::Block,
                LineSpacingType::Pixels,
                9.0,
                10.0,
                105.5,
            ),
            (
                crate::ObjectSpanLayoutOption::Inline,
                LineSpacingType::Pixels,
                9.0,
                40.0,
                100.0,
            ),
            (
                crate::ObjectSpanLayoutOption::BlockWithSmallMargin,
                LineSpacingType::Pixels,
                9.0,
                40.0,
                110.0,
            ),
        ] {
            let mut content = text("\u{fffc}");
            content.spans.push(crate::RichTextSpan {
                kind: crate::RichTextSpanType::FontSize,
                start_utf16: 0,
                end_utf16: 1,
                interval_type: crate::SpanIntervalType::from(0),
                payload: (object_font as f32).to_le_bytes().to_vec(),
            });
            content.paragraphs.push(RichTextParagraph {
                kind: RichTextParagraphType::LineSpacing,
                start_paragraph: 0,
                end_paragraph: 1,
                payload: [
                    (if kind == LineSpacingType::Percent {
                        1_u32
                    } else {
                        0_u32
                    })
                    .to_le_bytes(),
                    (spacing as f32).to_le_bytes(),
                ]
                .concat(),
            });
            content.object_spans.push(crate::RichTextObjectSpan {
                object_type: crate::ObjectType::CodeBlock,
                object_data: Vec::new(),
                content: Some(crate::RichTextObjectContent::CodeBlock(Box::new(
                    crate::RichTextCodeBlock {
                        bbox: BoundingBox {
                            x_min: 0.0,
                            y_min: 0.0,
                            x_max: 500.0,
                            y_max: 10.0,
                        },
                        rotation_degrees: None,
                        title: Some(text("Title")),
                        body: Some(text("Body")),
                    },
                ))),
                text_index_utf16: 0,
                layout_option: option,
                layout_constraint: ObjectSpanLayoutConstraint::OverPages,
            });
            let styled = StyledText::new(&content, TextContext::Placed, settings);
            let plan = layout_text(
                &styled,
                TextFrame {
                    bbox: BoundingBox {
                        x_min: 0.0,
                        y_min: 100.0,
                        x_max: 500.0,
                        y_max: 1000.0,
                    },
                    gravity: None,
                    exclusions: &[],
                },
                theme,
                &renderer,
            );
            let prepared = plan.lines[0].line.objects[0]
                .prepared
                .as_ref()
                .unwrap()
                .as_ref()
                .unwrap();
            let crate::render::embedded::PreparedObject::Code(code) = prepared else {
                panic!("expected retained code");
            };
            close(code.panel_bbox.y_min, expected);
        }
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
    fn page_edge_margin_probe_uses_raw_y_and_enabled_paragraph_before() {
        let bands = [VerticalExclusion::page_padding(1497.0, 1557.0)];
        let frame = TextFrame {
            bbox: BoundingBox::default(),
            gravity: None,
            exclusions: &bands,
        };
        for (top, before, current, previous, expected) in [
            (1557.0, 0.0, 60.0, 80.0, 30.0),
            (1558.0, 0.0, 60.0, 80.0, 80.0),
            (1497.0, 0.0, 60.0, 80.0, 80.0),
            (1569.0, 12.0, 60.0, 80.0, 30.0),
            (1569.0, 0.0, 60.0, 80.0, 80.0),
            (1557.0, 0.0, 0.0, 80.0, 0.0),
        ] {
            close(
                frame.adjusted_top_margin(top, before, current, previous),
                expected,
            );
        }
        let bands = [VerticalExclusion::obstacle(1497.0, 1557.0)];
        let frame = TextFrame {
            exclusions: &bands,
            ..frame
        };
        close(frame.adjusted_top_margin(1557.0, 0.0, 60.0, 80.0), 80.0);
    }

    #[test]
    fn page_edge_probe_rounds_each_subtraction_in_native_f32() {
        let bands = [VerticalExclusion::page_padding(1497.0, 1557.0)];
        let frame = TextFrame {
            bbox: BoundingBox::default(),
            gravity: None,
            exclusions: &bands,
        };
        close(
            frame.adjusted_top_margin(1558.0 - 0.00004, 0.0, 60.0, 80.0),
            80.0,
        );
        close(
            frame.adjusted_top_margin(1558.0 - 0.00013, 0.0, 60.0, 80.0),
            30.0,
        );
    }

    #[test]
    fn collision_recomputes_margin_at_the_new_raw_cursor() {
        let bands = [VerticalExclusion::page_padding(1497.0, 1557.0)];
        let frame = TextFrame {
            bbox: BoundingBox::default(),
            gravity: None,
            exclusions: &bands,
        };
        let mut cursor = TextCursor::new(1480.0);
        let line = object_line(20.0, false, [60.0; 2]);
        let placement = cursor.place(&line, None, &frame, TextSettings::default());
        close(placement.top, 1557.0);
        close(placement.baseline, 1687.001);
        close(placement.post_cursor, 1687.001);
        close(cursor.height(), 1747.001);
    }

    #[test]
    fn enabled_before_remains_available_on_subsequent_wrapped_lines() {
        let bands = [VerticalExclusion::page_padding(1539.0, 1599.0)];
        let frame = TextFrame {
            bbox: BoundingBox::default(),
            gravity: None,
            exclusions: &bands,
        };
        let mut cursor = TextCursor::new(1407.0);
        cursor.begin_paragraph(150.0);
        let line = object_line(20.0, false, [60.0; 2]);
        let first = cursor.place(&line, None, &frame, TextSettings::default());
        close(first.top, 1557.0);
        close(first.baseline, 1717.001);
        let second = cursor.place(&line, None, &frame, TextSettings::default());
        close(second.top, 1717.001);
        close(second.baseline, 1847.002);
    }

    #[test]
    fn page_padding_can_suppress_a_previous_object_bottom_on_an_ordinary_line() {
        let bands = [VerticalExclusion::page_padding(1497.0, 1557.0)];
        let frame = TextFrame {
            bbox: BoundingBox::default(),
            gravity: None,
            exclusions: &bands,
        };
        let mut cursor = TextCursor::new(1557.0);
        cursor.pending_bottom = 80.0;
        let line = WrappedLine::unmeasured(0..1, 20.0);
        let placement = cursor.place(&line, None, &frame, TextSettings::default());
        close(placement.top, 1557.0);
        close(placement.baseline, 1577.0);
        close(cursor.height(), 1584.0);
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
    fn backgrounds_exclude_object_margins_and_translate_with_the_line() {
        let frame = TextFrame {
            bbox: BoundingBox::default(),
            gravity: None,
            exclusions: &[],
        };
        let mut cursor = TextCursor::new(0.0);
        let object = object_line(20.0, false, [30.0; 2]);
        let placement = cursor.place(&object, None, &frame, TextSettings::default());
        close(placement.top, 0.0);
        close(placement.background_top, 30.0);
        close(placement.bottom, 130.001);

        let text = WrappedLine::unmeasured(1..2, 20.0);
        let mut placement = cursor.place(&text, None, &frame, TextSettings::default());
        close(placement.top, 130.001);
        close(placement.background_top, 160.001);
        close(placement.bottom, 187.001);
        placement.translate(5.0);
        close(placement.top, 135.001);
        close(placement.background_top, 165.001);
        close(placement.bottom, 192.001);
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
