use std::ops::Range;

mod native_object_bound;
pub(in crate::render) use native_object_bound::NativeObjectEntryBounds;
#[cfg(all(test, feature = "serde"))]
pub(in crate::render) use native_object_bound::caller_capture_profile as native_object_capture_profile;

use crate::render::RenderTheme;
use crate::render::marker::{PreparedMarker, marker_center_y};
use crate::{
    BoundingBox, BulletType, LineSpacingType, ParagraphAlignment, ParagraphBullet,
    ParagraphLineSpacing, PredefinedTextStyle,
};

use super::native_line::{NativeLineBands, NativeLineMetrics};
use super::objects::{MeasuredObject, ObjectDiagnosticKind, ObjectMeasurementContext};
#[cfg(test)]
use super::wrap_paragraph;
use super::wrapping::ParagraphMeasurementWidth;
use super::{
    StyledText, TextRenderer, WrappedLine, explicit_line_height, paragraph_layout,
    paragraph_line_height, unmeasured_paragraph,
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
    #[cfg(test)]
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
        Self::for_edges(
            [first_margin_object, last_margin_object],
            previous,
            current,
            next,
        )
    }

    fn for_edges(
        margins: [bool; 2],
        previous: Option<ParagraphBullet>,
        current: Option<ParagraphBullet>,
        next: Option<ParagraphBullet>,
    ) -> Self {
        Self {
            before: !(spacing_bullet(previous) && spacing_bullet(current)) && !margins[0],
            after: !(spacing_bullet(current) && spacing_bullet(next)) && !margins[1],
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
    invalid_native_geometry: bool,
    native_bands: Option<NativeLineBands>,
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
    native: NativeLineMetrics,
}

impl LineMetrics {
    fn for_line(
        line: &WrappedLine,
        spacing: Option<ParagraphLineSpacing>,
        settings: super::TextSettings,
        height_limit: f32,
    ) -> Self {
        let mut native = NativeLineMetrics {
            max_font: line.font_size as f32,
            base_height: line.base_height() as f32,
            extra_pixels: 0.0,
            multiplier: if super::finite_native_geometry(line.font_size * 1.35).is_some() {
                1.35
            } else {
                1.0
            },
            height_limit,
            object_margin_line: line.has_block_margins(),
            has_object_metric: !line.objects.is_empty(),
        };
        if let Some(spacing) = spacing
            .filter(|spacing| explicit_line_height(line.font_size, *spacing, settings).is_some())
        {
            match spacing.kind {
                LineSpacingType::Pixels => {
                    native.extra_pixels = settings.pixels(spacing.value) as f32
                }
                LineSpacingType::Percent => native.multiplier = spacing.value,
                LineSpacingType::Other(_) => {}
            }
        }
        let height = native.height().0;
        let advance = if height.is_finite() {
            f64::from(height)
        } else if line.has_block_margins() {
            line.base_height()
        } else {
            line.base_height() + paragraph_line_height(line.font_size, spacing, settings)
                - line.font_size
        };
        Self { advance, native }
    }
}

pub(in crate::render) struct TextCursor {
    position: f64,
    pending_bottom: f64,
    enabled_before: f64,
    height_limit: f32,
}

impl TextCursor {
    pub fn new(position: f64) -> Self {
        Self {
            position,
            pending_bottom: 0.0,
            enabled_before: 0.0,
            height_limit: f32::INFINITY,
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
        let metrics = LineMetrics::for_line(line, spacing, settings, self.height_limit);
        let mut candidate = self.candidate(line, frame, frame.bbox.y_min + self.position);
        for _ in 0..frame.exclusions.len() {
            let Some(band) = frame.overlapping_band(line, candidate.top(), metrics.advance) else {
                break;
            };
            candidate = self.candidate(line, frame, band.bottom);
        }
        self.place_at(line, spacing, frame, settings, candidate)
    }

    fn place_at(
        &mut self,
        line: &WrappedLine,
        spacing: Option<ParagraphLineSpacing>,
        frame: &TextFrame<'_>,
        settings: super::TextSettings,
        candidate: LineCandidate,
    ) -> LinePlacement {
        let metrics = LineMetrics::for_line(line, spacing, settings, self.height_limit);
        let cursor = self.position + (candidate.raw_top - (frame.bbox.y_min + self.position));
        let bands = metrics
            .native
            .place(cursor as f32, candidate.margin_top as f32);
        let Some(bands) = bands else {
            let top = candidate.top();
            let epsilon = if metrics.native.has_object_metric {
                0.001
            } else {
                0.0
            };
            self.position = top - frame.bbox.y_min + metrics.advance + epsilon;
            self.pending_bottom = line.object_margins()[1];
            let baseline_offset = if metrics.native.object_margin_line {
                metrics.advance + epsilon
            } else {
                metrics.advance - 0.35 * line.font_size + epsilon
            };
            return LinePlacement {
                top: candidate.raw_top,
                background_top: top,
                baseline: top + baseline_offset,
                bottom: frame.bbox.y_min + self.position,
                post_cursor: frame.bbox.y_min + self.position,
                invalid_native_geometry: true,
                native_bands: None,
            };
        };
        self.position = f64::from(bands.bottom);
        self.pending_bottom = line.object_margins()[1];
        LinePlacement {
            top: candidate.raw_top,
            background_top: frame.bbox.y_min + f64::from(bands.top),
            baseline: frame.bbox.y_min + f64::from(bands.baseline),
            bottom: frame.bbox.y_min + self.position,
            post_cursor: frame.bbox.y_min + self.position,
            invalid_native_geometry: false,
            native_bands: Some(bands),
        }
    }
}

fn prepare_object(
    placement: &mut super::wrapping::PositionedObject,
    styled: &StyledText<'_>,
    candidate_top: f64,
    content_width: f64,
    theme: RenderTheme,
    renderer: &TextRenderer<'_>,
) {
    let Some(span) = styled.object_span(placement.object.span_index) else {
        return;
    };

    let object_renderer = renderer.for_object_source(placement.object.source.clone());
    if placement.object.context == ObjectMeasurementContext::Body
        && matches!(
            span.layout_constraint,
            crate::ObjectSpanLayoutConstraint::OverPages
                | crate::ObjectSpanLayoutConstraint::OverPagesOverlapPadding
        )
        && matches!(span.content.as_ref(), Some(crate::RichTextObjectContent::CodeBlock(code))
            if code.rotation_degrees.is_some_and(|rotation| rotation != 0.0))
    {
        let kind = super::ObjectDiagnosticKind::UnsupportedContent;
        object_renderer.report_object_issues(&[super::ObjectDiagnostic {
            anchor_utf16: span.text_index_utf16,
            kind,
        }]);
        placement.prepared = Some(Err(kind));
        return;
    }
    let prepared = match span.content.as_ref() {
        Some(crate::RichTextObjectContent::CodeBlock(code)) => crate::render::code::prepare_code(
            code,
            span.layout_constraint,
            candidate_top,
            theme,
            &object_renderer,
        )
        .map(|code| crate::render::embedded::PreparedObject::Code(Box::new(code))),
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
                    return;
                };
                prepared
                    .map(|table| crate::render::embedded::PreparedObject::Table(Box::new(table)))
            }
        }
        _ => return,
    };
    match &prepared {
        Ok(prepared) => {
            let height = prepared.height();
            if matches!(
                span.layout_constraint,
                crate::ObjectSpanLayoutConstraint::OverPages
                    | crate::ObjectSpanLayoutConstraint::OverPagesOverlapPadding
            ) && (height as f32 - placement.object.height as f32).abs() > 0.001_f32
            {
                placement.object.height = height;
            }
            if placement.object.context == super::ObjectMeasurementContext::Body
                && matches!(
                    span.layout_constraint,
                    crate::ObjectSpanLayoutConstraint::OverPages
                        | crate::ObjectSpanLayoutConstraint::OverPagesOverlapPadding
                )
            {
                match placement.object.callback_width_limit(
                    span,
                    content_width,
                    renderer.object_page_ownership,
                ) {
                    Ok(limit) => {
                        let width =
                            limit.map_or(prepared.width(), |limit| prepared.width().min(limit));
                        if super::finite_native_geometry(width).is_some() && width > 0.0 {
                            if (width as f32 - placement.object.width as f32).abs() > 0.001_f32 {
                                placement.object.width = width;
                                placement.object.advance = width;
                            }
                        } else {
                            object_renderer.report_object_issues(&[super::ObjectDiagnostic {
                                anchor_utf16: span.text_index_utf16,
                                kind: super::ObjectDiagnosticKind::InvalidBounds,
                            }]);
                        }
                    }
                    Err(kind) => object_renderer.report_object_issues(&[super::ObjectDiagnostic {
                        anchor_utf16: span.text_index_utf16,
                        kind,
                    }]),
                }
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
    pub(super) native_bands: Option<NativeLineBands>,
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

pub(in crate::render) struct NativePaintFrame {
    pub translation: [f64; 2],
}

#[derive(Default)]
pub(in crate::render) struct TextLayout {
    pub lines: Vec<TextLine>,
    content_height: f64,
    pub native_frame: Option<NativePaintFrame>,
    pub native_object_entry: Option<NativeObjectEntryBounds>,
}

impl TextLayout {
    pub fn height(&self) -> f64 {
        self.content_height
    }

    pub fn translate(&mut self, dx: f64, dy: f64) {
        if dx != 0.0 || dy != 0.0 {
            self.native_object_entry = None;
        }
        if let Some(frame) = &mut self.native_frame {
            frame.translation[0] += dx;
            frame.translation[1] += dy;
        }
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
        if matches!(gravity, Some(1 | 2)) {
            self.native_frame = None;
        }
        let available_height = (outer_height - content_height).max(0.0);
        let offset = match gravity {
            Some(1) => available_height / 2.0,
            Some(2) => available_height,
            _ => 0.0,
        };
        self.translate(0.0, offset);
    }
}

struct ParagraphLines<'a, 'text, 'fonts> {
    styled: &'a StyledText<'text>,
    theme: RenderTheme,
    predefined: Option<PredefinedTextStyle>,
    renderer: &'a TextRenderer<'fonts>,
    object_context: ObjectMeasurementContext,
    measured: Option<super::wrapping::ParagraphWrapper<'a, 'text, 'fonts>>,
    fallback: std::collections::VecDeque<WrappedLine>,
}

impl<'a, 'text, 'fonts> ParagraphLines<'a, 'text, 'fonts> {
    fn new(
        styled: &'a StyledText<'text>,
        source: Range<usize>,
        width: impl Into<ParagraphMeasurementWidth>,
        theme: RenderTheme,
        predefined: Option<PredefinedTextStyle>,
        renderer: &'a TextRenderer<'fonts>,
        object_context: ObjectMeasurementContext,
    ) -> Self {
        let mut lines = Self {
            styled,
            theme,
            predefined,
            renderer,
            object_context,
            measured: None,
            fallback: Default::default(),
        };
        if source.is_empty() {
            lines.fallback.push_back(WrappedLine::unmeasured(
                source.clone(),
                styled.font_size_at_caret(source.start),
            ));
        } else {
            match super::wrapping::ParagraphWrapper::new(
                styled,
                source.clone(),
                width,
                theme,
                predefined,
                renderer,
                object_context,
            ) {
                Ok(measured) => lines.measured = Some(measured),
                Err(_) => lines.recover(source),
            }
        }
        lines
    }

    fn measurement_width(&self) -> Option<f64> {
        self.measured
            .as_ref()
            .map(super::wrapping::ParagraphWrapper::measurement_width)
    }

    fn recover(&mut self, source: Range<usize>) {
        let style = self
            .styled
            .style_at(source.start, self.theme, self.predefined);
        self.renderer
            .measurement_failed(style.family.as_deref().unwrap_or("Roboto"));
        self.fallback = unmeasured_paragraph(
            self.styled,
            source,
            self.theme,
            self.predefined,
            self.renderer,
            self.object_context,
        )
        .into();
        self.measured = None;
    }

    fn object_edge_margins(&self) -> [bool; 2] {
        if let Some(measured) = &self.measured {
            measured.object_edge_margins()
        } else {
            let margins = |line: Option<&WrappedLine>, first: bool| {
                line.is_some_and(|line| {
                    line.objects.iter().any(|placement| {
                        (if first {
                            placement.object.source.start == line.source.start
                        } else {
                            placement.object.source.end == line.source.end
                        }) && placement.object.top_margin > 0.0
                            && placement.object.bottom_margin > 0.0
                    })
                })
            };
            [
                margins(self.fallback.front(), true),
                margins(self.fallback.back(), false),
            ]
        }
    }

    fn candidate(
        &mut self,
        width: f64,
        mut prepare: impl FnMut(&mut super::wrapping::PositionedObject),
    ) -> Option<WrappedLine> {
        if let Some(measured) = &mut self.measured {
            match measured.candidate(width, &mut prepare) {
                Ok(line) => return line,
                Err(_) => {
                    let source = measured.remaining_source();
                    self.recover(source);
                }
            }
        }
        let mut line = self.fallback.pop_front()?;
        for object in &mut line.objects {
            prepare(object);
        }
        Some(line)
    }

    fn restore(&mut self, line: WrappedLine) {
        if let Some(measured) = &mut self.measured {
            measured.restore(line);
        } else {
            self.fallback.push_front(line);
        }
    }

    fn commit(&mut self, source_end: usize) {
        if let Some(measured) = &mut self.measured {
            measured.commit(source_end);
        }
    }
}

#[cfg(test)]
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

#[derive(Clone, Copy)]
enum NativeCellMeasurementWidth {
    Fixed(i32),
    Automatic,
}

pub(in crate::render) struct NativeCellTextConstraints {
    pub width: i32,
    pub height_limit: f32,
}

#[derive(Clone, Copy)]
struct NativeCellMeasurement {
    width: NativeCellMeasurementWidth,
    height_limit: f32,
}

impl NativeCellMeasurementWidth {
    fn from_dimension(width: i32) -> Option<Self> {
        match width {
            0 => Some(Self::Automatic),
            1.. => Some(Self::Fixed(width)),
            _ => None,
        }
    }

    fn requested_width(self) -> f64 {
        match self {
            Self::Fixed(width) => f64::from(width),
            Self::Automatic => 0.0,
        }
    }
}

pub(in crate::render) fn layout_table_cell_text(
    styled: &StyledText<'_>,
    frame: TextFrame<'_>,
    constraints: NativeCellTextConstraints,
    theme: RenderTheme,
    renderer: &TextRenderer<'_>,
) -> Result<TextLayout, ObjectDiagnosticKind> {
    let width = NativeCellMeasurementWidth::from_dimension(constraints.width)
        .ok_or(ObjectDiagnosticKind::InvalidBounds)?;
    if constraints.height_limit.is_nan() {
        return Err(ObjectDiagnosticKind::InvalidBounds);
    }
    try_layout_text_with_context(
        styled,
        frame,
        theme,
        renderer,
        LayoutContext::Frame,
        None,
        Some(NativeCellMeasurement {
            width,
            height_limit: constraints.height_limit,
        }),
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
        styled: &StyledText<'_>,
    ) -> Option<f64> {
        if matches!(self, Self::Flow)
            && paragraph_number == 0
            && line_number == 0
            && let [object] = line.objects.as_slice()
            && !object.object.inline
        {
            saved_continuation_top(styled, &object.object)
        } else {
            None
        }
    }
}

fn saved_continuation_top(styled: &StyledText<'_>, object: &MeasuredObject) -> Option<f64> {
    let raw_top = match styled.object_span(object.span_index)?.content.as_ref() {
        Some(crate::RichTextObjectContent::Table(table)) => table.bbox.y_min,
        Some(crate::RichTextObjectContent::CodeBlock(code)) => code.bbox.y_min,
        _ => object.bounds.y_min,
    };
    (raw_top < 0.0).then_some(object.bounds.y_min)
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
    try_layout_text_with_context(
        styled,
        frame,
        theme,
        renderer,
        context,
        measurement_size,
        None,
    )
    .expect("constrained text layout does not require native automatic measurement")
}

fn try_layout_text_with_context(
    styled: &StyledText<'_>,
    frame: TextFrame<'_>,
    theme: RenderTheme,
    renderer: &TextRenderer<'_>,
    context: LayoutContext,
    measurement_size: Option<[i32; 2]>,
    native_cell_measurement: Option<NativeCellMeasurement>,
) -> Result<TextLayout, ObjectDiagnosticKind> {
    let scoped_renderer = renderer.local_measurement_scope();
    let renderer = &scoped_renderer;
    let text_box = styled.text_box;
    let settings = renderer.settings;
    renderer.report_object_issues(styled.object_issues());
    renderer.report_owned_geometry_issues(styled.geometry_issues());
    renderer.report_span_issues(styled);
    let margins = text_box
        .margins
        .unwrap_or([0.0; 4])
        .map(|margin| settings.pixels(margin));
    let [frame_width, outer_height] = measurement_size.map_or_else(
        || {
            [
                (frame.bbox.x_max - frame.bbox.x_min).ceil(),
                (frame.bbox.y_max - frame.bbox.y_min).ceil(),
            ]
        },
        |size| size.map(f64::from),
    );
    let outer_width = native_cell_measurement.map_or(frame_width, |measurement| {
        measurement.width.requested_width()
    });
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
    if let Some(measurement) = native_cell_measurement {
        cursor.height_limit = measurement.height_limit;
    }
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
        let mut marker_style =
            styled.style_at(paragraph.content.start, theme, layout.predefined_style);
        if paragraph.content.is_empty() {
            marker_style.font_size = styled.font_size_at_caret(paragraph.content.start);
        }
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
        let automatic = matches!(
            native_cell_measurement.map(|measurement| measurement.width),
            Some(NativeCellMeasurementWidth::Automatic)
        );
        if automatic
            && !styled.index.is_empty()
            && (paragraph.content.is_empty()
                || layout
                    .bullet
                    .is_some_and(|bullet| !matches!(bullet.kind, crate::BulletType::None))
                || layout.indent_level != 0)
        {
            return Err(ObjectDiagnosticKind::UnsupportedContent);
        }
        let measurement_width = if automatic {
            ParagraphMeasurementWidth::Automatic {
                insets: [margins[0] as f32, margins[2] as f32],
            }
        } else {
            ParagraphMeasurementWidth::Constrained(width)
        };
        let mut paragraph_lines = ParagraphLines::new(
            styled,
            paragraph.content.clone(),
            measurement_width,
            theme,
            layout.predefined_style,
            renderer,
            context.object_measurement(),
        );
        let width = if automatic && !paragraph.content.is_empty() {
            paragraph_lines
                .measurement_width()
                .ok_or(ObjectDiagnosticKind::UnsupportedContent)?
        } else {
            width
        };
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
        let spacing = ParagraphSpacing::for_edges(
            paragraph_lines.object_edge_margins(),
            previous,
            layout.bullet,
            next,
        );
        if spacing.before && layout.spacing_before_invalid {
            let style = styled.style_at(paragraph.content.start, theme, layout.predefined_style);
            renderer.invalid_geometry(style.family.as_deref().unwrap_or("Roboto"));
        }
        cursor.begin_paragraph(if spacing.before {
            layout.spacing_before
        } else {
            0.0
        });
        let mut line_number = 0;
        loop {
            let mut raw_top = frame.bbox.y_min + cursor.position;
            let mut settled = None;
            for attempt in 0..=frame.exclusions.len() {
                let line = paragraph_lines.candidate(width, |placement| {
                    let margin_top = frame.adjusted_top_margin(
                        raw_top,
                        cursor.enabled_before,
                        placement.object.top_margin,
                        cursor.pending_bottom,
                    );
                    let candidate = LineCandidate {
                        raw_top,
                        margin_top,
                    };
                    let style = styled.style_at(
                        placement.object.source.start,
                        theme,
                        layout.predefined_style,
                    );
                    let continuation = (matches!(context, LayoutContext::Flow)
                        && paragraph_number == 0
                        && line_number == 0
                        && !placement.object.inline
                        && placement.object.source.start == paragraph.content.start)
                        .then(|| saved_continuation_top(styled, &placement.object))
                        .flatten();
                    let object_top = continuation.unwrap_or_else(|| {
                        candidate.object_top(
                            &placement.object,
                            style.font_size,
                            layout.line_spacing,
                            settings,
                        )
                    });
                    prepare_object(
                        placement,
                        styled,
                        object_top,
                        content_width,
                        theme,
                        renderer,
                    );
                });
                let Some(line) = line else {
                    break;
                };
                let mut candidate = cursor.candidate(&line, &frame, raw_top);
                let metrics = LineMetrics::for_line(
                    &line,
                    layout.line_spacing,
                    settings,
                    cursor.height_limit,
                );
                if context
                    .continuation_top(paragraph_number, line_number, &line, styled)
                    .is_some()
                {
                    for _ in 0..frame.exclusions.len() {
                        let Some(band) =
                            frame.overlapping_band(&line, candidate.top(), metrics.advance)
                        else {
                            break;
                        };
                        candidate = cursor.candidate(&line, &frame, band.bottom);
                    }
                } else if let Some(band) =
                    frame.overlapping_band(&line, candidate.top(), metrics.advance)
                    && attempt < frame.exclusions.len()
                {
                    raw_top = band.bottom;
                    paragraph_lines.restore(line);
                    continue;
                }
                settled = Some((line, candidate));
                break;
            }
            let Some((mut line, settled_top)) = settled else {
                break;
            };
            paragraph_lines.commit(line.source.end);
            let continuation_top =
                context.continuation_top(paragraph_number, line_number, &line, styled);
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
            let mut line_x = x;
            let mut line_alignment = layout.alignment;
            if native_cell_measurement.is_some()
                && frame.bbox.x_min == 0.0
                && frame.exclusions.is_empty()
                && layout.indent_level == 0
                && marker_width == 0.0
            {
                match line.place_native_cell(x, width, layout.alignment) {
                    Ok(Some(origin)) => {
                        line_x = origin;
                        line_alignment = None;
                    }
                    Ok(None) => {}
                    Err(_) => {
                        let style =
                            styled.style_at(line.source.start, theme, layout.predefined_style);
                        renderer.invalid_geometry(style.family.as_deref().unwrap_or("Roboto"));
                    }
                }
            }
            renderer.report_line_geometry(&line, layout.line_spacing);
            let mut placement =
                cursor.place_at(&line, layout.line_spacing, &frame, settings, settled_top);
            if placement.invalid_native_geometry {
                let style = styled.style_at(line.source.start, theme, layout.predefined_style);
                renderer.invalid_geometry(style.family.as_deref().unwrap_or("Roboto"));
            }
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
                x: line_x,
                width,
                baseline: placement.baseline,
                top: placement.top,
                background_top: placement.background_top,
                bottom: placement.bottom,
                post_cursor: placement.post_cursor,
                alignment: line_alignment,
                predefined: layout.predefined_style,
                marker: positioned_marker,
                native_bands: placement.native_bands,
            });
            line_number += 1;
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
    let native_object_entry =
        native_object_bound::native_object_entry_bounds(styled, &frame, &lines, renderer, context);
    let mut layout = TextLayout {
        native_object_entry,
        native_frame: (native_cell_measurement.is_some()
            && frame.bbox.x_min == 0.0
            && frame.bbox.y_min == 0.0
            && frame.exclusions.is_empty()
            && paragraph_layouts.iter().all(|paragraph| {
                paragraph
                    .bullet
                    .is_none_or(|bullet| bullet.kind == BulletType::None && !bullet.checked)
            })
            && !lines_have_unsupported_native_metadata(&lines))
        .then_some(NativePaintFrame {
            translation: [0.0; 2],
        }),
        lines,
        content_height,
    };
    layout.apply_gravity(frame.gravity, outer_height, gravity_height);
    Ok(layout)
}

fn lines_have_unsupported_native_metadata(lines: &[TextLine]) -> bool {
    lines.iter().any(|line| {
        line.predefined.is_some()
            || line.marker.is_some()
            || !line.line.objects.is_empty()
            || line.native_bands.is_none()
            || (!line.line.source.is_empty() && line.line.native_placed.is_none())
    })
}

#[cfg(test)]
mod tests {
    mod measurement_width;

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
    fn layout_reports_unsupported_native_span_styles() {
        let mut content = text("AB");
        content.spans.push(crate::RichTextSpan {
            kind: crate::RichTextSpanType::Suggestion,
            start_utf16: 0,
            end_utf16: 1,
            interval_type: crate::SpanIntervalType::ClosedOpen,
            payload: Vec::new(),
        });
        let fonts = FontBook::default();
        let renderer = TextRenderer::new(TextSettings::default(), &fonts);
        let styled = StyledText::new(&content, TextContext::Placed, renderer.settings);
        layout_text(
            &styled,
            TextFrame {
                bbox: BoundingBox {
                    x_max: 100.0,
                    y_max: 100.0,
                    ..Default::default()
                },
                gravity: None,
                exclusions: &[],
            },
            RenderTheme::for_canvas(false),
            &renderer,
        );
        assert_eq!(
            renderer.diagnostics()[0].kind,
            super::super::TextDiagnosticKind::UnsupportedSuggestionStyle
        );
        assert_eq!(
            renderer.scoped_diagnostics()[0].owner,
            Some(super::super::SourceOwner::Text(0..1))
        );
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
                context: super::super::objects::ObjectMeasurementContext::Frame,
                source: 0..1,
                span_index: 0,
                bounds: BoundingBox {
                    x_min: 0.0,
                    y_min: 0.0,
                    x_max: 50.0,
                    y_max: 100.0,
                },
                height: 100.0,
                width: 50.0,
                advance: 50.0,
                inline,
                left_margin: 0.0,
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

    #[test]
    fn line_bounds_match_native_entry_and_emitted_run_captures() {
        use serde::Deserialize;
        use sha2::{Digest, Sha256};

        #[derive(Deserialize)]
        struct Capture {
            apk_sha256: String,
            text_library_sha256: String,
            base_library_sha256: String,
            memory_fills: Vec<u8>,
            cases: Vec<Case>,
        }

        #[derive(Deserialize)]
        struct Case {
            name: String,
            cursor: f32,
            margin: f32,
            font_size: f32,
            base_height: f32,
            pixels: f32,
            multiplier: f32,
            object_metric: bool,
            line_count: usize,
            post_cursors: Vec<f32>,
            entries: Vec<Entry>,
            #[serde(default)]
            runs: Vec<Run>,
            #[serde(default)]
            offset: [f32; 2],
            #[serde(default)]
            gravity: f32,
        }

        #[derive(Deserialize)]
        struct Entry {
            position: [f32; 2],
            layout_rect: [f32; 4],
        }

        #[derive(Deserialize)]
        struct Run {
            range_inclusive: Option<[usize; 2]>,
            layout_rect: [f32; 4],
            #[serde(default)]
            origin: [f32; 2],
        }

        for (bytes, digest, count) in [
            (
                include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../conformance/table-text-bounds.json"
                ))
                .as_slice(),
                "a3fd5eba3513c55bfbe3e3e14fe069592a384989700079ae0537ddbd1fbe1d8c",
                162,
            ),
            (
                include_bytes!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../conformance/table-text-runs.json"
                ))
                .as_slice(),
                "dae6b6929ce1349eefdcb9202bffd4ac9393c3b731511ccf1f493d38cdba0792",
                230,
            ),
        ] {
            assert_eq!(format!("{:x}", Sha256::digest(bytes)), digest);
            let capture: Capture = serde_json::from_slice(bytes).unwrap();
            assert_eq!(
                capture.apk_sha256,
                "daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667"
            );
            assert_eq!(
                capture.text_library_sha256,
                "5483711673a499743625eb3275e34b37a006919af346212b46b8d8857834308b"
            );
            assert_eq!(
                capture.base_library_sha256,
                "e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb"
            );
            assert_eq!(capture.memory_fills, [0, 165, 255]);
            assert_eq!(capture.cases.len(), count);
            if count == 230 {
                assert_eq!(
                    capture
                        .cases
                        .iter()
                        .map(|case| case.runs.len())
                        .sum::<usize>(),
                    528
                );
                assert!(
                    capture
                        .cases
                        .iter()
                        .flat_map(|case| &case.runs)
                        .all(|run| run.range_inclusive.is_some())
                );
            }
            let frame = TextFrame {
                bbox: BoundingBox::default(),
                gravity: None,
                exclusions: &[],
            };
            let mut rounding_negative_control = false;
            for case in capture.cases {
                if case.name == "object-leading-109" {
                    let folded = f64::from(case.post_cursors[0])
                        + f64::from(case.margin)
                        + f64::from(case.base_height)
                        + (f64::from(case.multiplier) - 1.0) * f64::from(case.font_size)
                        + 0.001;
                    assert_ne!((folded as f32).to_bits(), case.post_cursors[1].to_bits(),);
                    rounding_negative_control = true;
                }
                let font_size = f64::from(case.font_size);
                let mut line = if case.object_metric {
                    object_line(font_size, true, [0.0; 2])
                } else {
                    WrappedLine::unmeasured(0..3, font_size)
                };
                line.text_height = f64::from(case.base_height);
                let spacing = Some(ParagraphLineSpacing {
                    kind: if case.pixels == 0.0 {
                        crate::LineSpacingType::Percent
                    } else {
                        crate::LineSpacingType::Pixels
                    },
                    value: if case.pixels == 0.0 {
                        case.multiplier
                    } else {
                        case.pixels
                    },
                });
                let mut cursor = TextCursor::new(f64::from(case.cursor));
                assert_eq!(case.entries.len(), case.line_count * 3);
                assert_eq!(case.post_cursors.len(), case.line_count);
                let mut placements = Vec::new();
                for index in 0..case.line_count {
                    let placement = cursor.place_at(
                        &line,
                        spacing,
                        &frame,
                        TextSettings::default(),
                        LineCandidate {
                            raw_top: cursor.position(),
                            margin_top: f64::from(case.margin),
                        },
                    );
                    let entry = &case.entries[index * 3];
                    for (field, actual, expected) in [
                        ("top", placement.background_top, entry.layout_rect[1]),
                        ("bottom", placement.bottom, entry.layout_rect[3]),
                        ("baseline", placement.baseline, entry.position[1]),
                        ("cursor", placement.post_cursor, case.post_cursors[index]),
                    ] {
                        assert_eq!(
                            (actual as f32).to_bits(),
                            expected.to_bits(),
                            "{} line {index} {field}: {actual} != {expected}",
                            case.name
                        );
                    }
                    placements.push(placement);
                }
                let dy = case.gravity + case.offset[1];
                for run in case.runs {
                    let Some(range) = run.range_inclusive else {
                        continue;
                    };
                    let [first, last] = range.map(|index| index / 3);
                    let covered = &placements[first..=last];
                    let top = covered
                        .iter()
                        .map(|line| line.background_top)
                        .fold(f64::INFINITY, f64::min);
                    let bottom = covered
                        .iter()
                        .map(|line| line.bottom)
                        .fold(f64::NEG_INFINITY, f64::max);
                    for (field, actual, expected) in [
                        ("run top", top as f32 + dy, run.layout_rect[1]),
                        ("run bottom", bottom as f32 + dy, run.layout_rect[3]),
                        (
                            "run baseline",
                            placements[first].baseline as f32 + dy,
                            run.origin[1],
                        ),
                    ] {
                        assert_eq!(
                            actual.to_bits(),
                            expected.to_bits(),
                            "{} {} {field}: {actual} != {expected}",
                            case.name,
                            range[0]
                        );
                    }
                }
            }
            assert!(rounding_negative_control);
        }
    }

    #[test]
    fn rejected_native_line_bands_preserve_finite_recovery_and_geometry_status() {
        let mut line = WrappedLine::unmeasured(0..1, 1e38);
        line.text_height = 3e38;
        let mut cursor = TextCursor::new(0.0);
        let placement = cursor.place_at(
            &line,
            Some(ParagraphLineSpacing {
                kind: LineSpacingType::Pixels,
                value: 1e38,
            }),
            &TextFrame {
                bbox: BoundingBox::default(),
                gravity: None,
                exclusions: &[],
            },
            TextSettings::default(),
            LineCandidate {
                raw_top: 0.0,
                margin_top: 0.0,
            },
        );
        assert!(placement.invalid_native_geometry);
        assert!(placement.baseline.is_finite());
        assert!(placement.bottom.is_finite());
        assert!(placement.post_cursor.is_finite());
    }

    #[test]
    fn native_line_cursor_is_independent_of_a_large_world_origin() {
        let frame = TextFrame {
            bbox: BoundingBox {
                y_min: 1e30,
                y_max: 1e30,
                ..BoundingBox::default()
            },
            gravity: None,
            exclusions: &[],
        };
        let line = WrappedLine::unmeasured(0..1, 20.0);
        let mut cursor = TextCursor::new(0.0);
        for expected in [27.0, 54.0, 81.0] {
            let placement = cursor.place(&line, None, &frame, TextSettings::default());
            assert!(!placement.invalid_native_geometry);
            assert_eq!(cursor.position(), expected);
        }
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
                close(placed.lines[0].baseline, f64::from(100.001_f32));
                close(flow.lines[0].baseline, 80.0);
                close(flow.lines[0].top, f64::from(-20.001_f32));
                close(flow.lines[0].bottom, 83.5);
                close(flow.lines[0].post_cursor, 83.5);
                close(placed.lines[1].baseline, f64::from(113.501_f32));
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
            ("A\n\u{fffc}\nB", 2, 1, 45.0, f64::from(321.501_f32)),
            ("\u{fffc}\nB", 0, 0, 45.0, f64::from(260.751_f32)),
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
        close(initial.copy.y_min, 506.0);
        close(initial.copy.y_max, 578.0);
        close(initial.panel_bbox.y_max - initial.panel_bbox.y_min, 606.5);
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
        close(line.baseline, f64::from(1150.751_f32));
        close(line.line.objects[0].object.height, 620.75);
        let body = prepared.body_layout.as_ref().unwrap();
        close(body.lines[0].baseline, 707.0);
        close(body.lines[5].baseline, 1075.0);
        close(plan.lines[1].baseline, f64::from(1211.501_f32));
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
            (
                ObjectSpanLayoutConstraint::OverPages,
                false,
                7.0,
                f64::from(259.751_f32),
            ),
            (
                ObjectSpanLayoutConstraint::OverPages,
                false,
                7.25,
                f64::from(512.751_f32),
            ),
            (
                ObjectSpanLayoutConstraint::OverPages,
                true,
                7.0,
                f64::from(512.751_f32),
            ),
            (
                ObjectSpanLayoutConstraint::Normal,
                false,
                7.0,
                f64::from(512.751_f32),
            ),
        ] {
            let prepared = crate::render::code::prepare_code(
                &code,
                constraint,
                candidate,
                RenderTheme::for_canvas(false),
                &renderer,
            )
            .unwrap();
            assert_eq!(prepared.min_first_page_height, 192.75);
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
            f64::from(100.001_f32),
        );
        close(cursor.position(), f64::from(107.001_f32));
        close(cursor.height(), f64::from(107.001_f32));
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
            f64::from(130.001_f32),
        );
        close(cursor.position(), f64::from(130.001_f32));
        close(cursor.height(), f64::from(160.001_f32));
    }

    #[test]
    fn adjacent_block_margins_collapse_instead_of_accumulating() {
        let mut cursor = TextCursor::new(0.0);
        let line = object_line(20.0, false, [30.0; 2]);
        close(place_object(&mut cursor, &line), f64::from(130.001_f32));
        close(place_object(&mut cursor, &line), f64::from(260.002_f32));
        close(cursor.height(), f64::from(290.002_f32));
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
        close(placement.baseline, f64::from(1687.001_f32));
        close(placement.post_cursor, f64::from(1687.001_f32));
        close(cursor.height(), f64::from(1747.001_f32));
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
        close(first.baseline, f64::from(1717.001_f32));
        let second = cursor.place(&line, None, &frame, TextSettings::default());
        close(second.top, f64::from(1717.001_f32));
        close(second.baseline, f64::from(1847.002_f32));
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
        close(cursor.height_with_bottom(10.0), f64::from(160.001_f32));
        close(cursor.height_with_bottom(40.0), f64::from(170.001_f32));
        close(cursor.height_with_bottom(-10.0), f64::from(160.001_f32));
    }

    #[test]
    fn following_text_consumes_the_pending_object_margin_once() {
        let mut cursor = TextCursor::new(0.0);
        place_object(&mut cursor, &object_line(20.0, false, [30.0; 2]));
        let text = WrappedLine::unmeasured(1..2, 20.0);
        close(place_object(&mut cursor, &text), f64::from(180.001_f32));
        close(cursor.height(), f64::from(187.001_f32));
        close(place_object(&mut cursor, &text), f64::from(207.001_f32));
        close(cursor.height(), f64::from(214.001_f32));
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
        close(placement.bottom, f64::from(130.001_f32));

        let text = WrappedLine::unmeasured(1..2, 20.0);
        let mut placement = cursor.place(&text, None, &frame, TextSettings::default());
        close(placement.top, f64::from(130.001_f32));
        close(placement.background_top, f64::from(160.001_f32));
        close(placement.bottom, f64::from(187.001_f32));
        placement.translate(5.0);
        close(placement.top, f64::from(135.001_f32));
        close(placement.background_top, f64::from(165.001_f32));
        close(placement.bottom, f64::from(192.001_f32));
    }

    #[test]
    fn zero_text_font_metric_does_not_add_leading_to_object_height() {
        let mut cursor = TextCursor::new(0.0);
        close(
            place_object(&mut cursor, &object_line(0.0, true, [0.0; 2])),
            f64::from(100.001_f32),
        );
        close(cursor.height(), f64::from(100.001_f32));
    }
}
