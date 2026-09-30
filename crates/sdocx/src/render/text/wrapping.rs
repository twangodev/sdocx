use std::ops::Range;

use crate::PredefinedTextStyle;
use crate::render::RenderTheme;

use super::breaks::{BreakCandidate, BreakKind, ParagraphBreaks, break_candidates};
use super::measurement::{MeasuredCluster, MeasurementError, ParagraphMeasurer};
use super::objects::MeasuredObject;
use super::{StyledText, TextRenderer};

pub(in crate::render) struct WrappedLine {
    pub source: Range<usize>,
    pub font_size: f64,
    pub text_height: f64,
    pub advance: f64,
    pub placements: Vec<PositionedCluster>,
    pub objects: Vec<PositionedObject>,
}

pub(in crate::render) struct PositionedCluster {
    pub cluster: MeasuredCluster,
    pub x: f64,
    pub extra_advance: f64,
}

pub(in crate::render) struct PositionedObject {
    pub object: MeasuredObject,
    pub x: f64,
    pub prepared:
        Option<Result<crate::render::embedded::PreparedObject, super::ObjectDiagnosticKind>>,
}

enum MeasuredItem {
    TextCluster(MeasuredCluster),
    Object {
        object: MeasuredObject,
        font_size: f64,
    },
}

struct ParagraphItems {
    items: Vec<MeasuredItem>,
    font_size: f64,
    advance: f64,
}

impl MeasuredItem {
    fn source(&self) -> &Range<usize> {
        match self {
            Self::TextCluster(cluster) => &cluster.source,
            Self::Object { object, .. } => &object.source,
        }
    }

    fn advance(&self) -> f64 {
        match self {
            Self::TextCluster(cluster) => cluster.advance,
            Self::Object { object, .. } => object.bounds.x_max - object.bounds.x_min,
        }
    }
}

impl WrappedLine {
    pub fn unmeasured(source: Range<usize>, font_size: f64) -> Self {
        Self {
            source,
            font_size,
            text_height: font_size,
            advance: 0.0,
            placements: Vec::new(),
            objects: Vec::new(),
        }
    }

    pub fn object_height(&self) -> f64 {
        self.objects.iter().fold(0.0_f64, |height, positioned| {
            height.max(positioned.object.height)
        })
    }

    pub fn base_height(&self) -> f64 {
        self.text_height.max(self.object_height())
    }

    pub fn justify(&mut self, styled: &StyledText<'_>, width: f64) -> Result<(), MeasurementError> {
        #[derive(Clone, Copy)]
        enum Entry {
            Text(usize),
            Object(usize),
        }
        let geometry = |value| {
            super::finite_native_geometry(value)
                .map(|value| value as f32)
                .ok_or(MeasurementError::InvalidCluster)
        };
        let width = geometry(width)?;
        let advance = geometry(self.advance)?;
        let mut entries = Vec::with_capacity(self.placements.len() + self.objects.len());
        let mut weights = 0_u32;
        for (index, placement) in self.placements.iter().enumerate() {
            let text = styled
                .index
                .slice(placement.cluster.source.clone())
                .ok_or(MeasurementError::InvalidRange)?;
            let weight = text
                .chars()
                .try_fold(0_u32, |weight, scalar| {
                    weight.checked_add(match scalar {
                        ' ' => 1,
                        '\t' => 4,
                        _ => 0,
                    })
                })
                .ok_or(MeasurementError::InvalidCluster)?;
            weights = weights
                .checked_add(weight)
                .ok_or(MeasurementError::InvalidCluster)?;
            entries.push((
                geometry(placement.x)?,
                placement.cluster.source.start,
                weight,
                Entry::Text(index),
            ));
        }
        for (index, placement) in self.objects.iter().enumerate() {
            entries.push((
                geometry(placement.x)?,
                placement.object.source.start,
                0,
                Entry::Object(index),
            ));
        }
        if weights == 0 {
            return Ok(());
        }
        let extra = geometry(f64::from((width - advance) / weights as f32))?;
        entries.sort_by(|left, right| left.0.total_cmp(&right.0).then(left.1.cmp(&right.1)));
        let mut shift = 0.0_f32;
        let mut positions = Vec::with_capacity(entries.len());
        for (x, _, weight, entry) in entries {
            let x = geometry(f64::from(x + shift))?;
            let extra_advance = geometry(f64::from(extra * weight as f32))?;
            shift = geometry(f64::from(shift + extra_advance))?;
            positions.push((entry, f64::from(x), f64::from(extra_advance)));
        }
        let advance = geometry(f64::from(advance + shift))?;
        for (entry, x, extra_advance) in positions {
            match entry {
                Entry::Text(index) => {
                    self.placements[index].x = x;
                    self.placements[index].extra_advance = extra_advance;
                }
                Entry::Object(index) => self.objects[index].x = x,
            }
        }
        self.advance = f64::from(advance);
        Ok(())
    }

    pub fn has_block_margins(&self) -> bool {
        let [top, bottom] = self.object_margins();
        self.placements.is_empty()
            && self.objects.iter().any(|object| !object.object.inline)
            && top > 0.0
            && bottom > 0.0
    }

    pub fn object_margins(&self) -> [f64; 2] {
        self.objects
            .iter()
            .fold([0.0_f64; 2], |margins, positioned| {
                [
                    margins[0].max(positioned.object.top_margin),
                    margins[1].max(positioned.object.bottom_margin),
                ]
            })
    }
}

fn measured_items(
    styled: &StyledText<'_>,
    range: Range<usize>,
    measurer: &ParagraphMeasurer<'_, '_, '_>,
    renderer: &TextRenderer<'_>,
    breaks: &mut ParagraphBreaks,
) -> Result<ParagraphItems, MeasurementError> {
    let paragraph_objects = styled.objects.in_range(range.clone());
    let mut items = Vec::new();
    let mut start = range.start;
    let mut font_size = 0.0_f64;
    let mut advance = 0.0;
    for object in paragraph_objects {
        if start < object.source.start {
            let measured = measurer.measure_line(start..object.source.start)?;
            font_size = font_size.max(measured.font_size);
            advance += measured.advance;
            items.extend(measured.clusters.into_iter().map(MeasuredItem::TextCluster));
        }
        let object = object.measured(renderer.settings);
        let object_font_size = measurer.font_size(object.source.clone())?;
        font_size = font_size.max(object_font_size);
        let kind = if object.inline {
            BreakKind::Allowed
        } else {
            BreakKind::Mandatory
        };
        for end in [object.source.start, object.source.end] {
            let end = end - range.start;
            if end != 0 {
                breaks.candidates.push(BreakCandidate { end, kind });
                breaks.emergency.push(end);
            }
        }
        start = object.source.end;
        advance += object.bounds.x_max - object.bounds.x_min;
        items.push(MeasuredItem::Object {
            object,
            font_size: object_font_size,
        });
    }
    if start < range.end {
        let measured = measurer.measure_line(start..range.end)?;
        font_size = font_size.max(measured.font_size);
        advance += measured.advance;
        items.extend(measured.clusters.into_iter().map(MeasuredItem::TextCluster));
    }
    if !paragraph_objects.is_empty() {
        breaks.candidates.sort_unstable_by_key(|candidate| {
            (candidate.end, candidate.kind != BreakKind::Mandatory)
        });
        breaks.candidates.dedup_by_key(|candidate| candidate.end);
        breaks.emergency.sort_unstable();
        breaks.emergency.dedup();
    }
    Ok(ParagraphItems {
        items,
        font_size,
        advance,
    })
}

fn paragraph_prefix_font_size(
    styled: &StyledText<'_>,
    source: &Range<usize>,
    theme: RenderTheme,
    predefined: Option<PredefinedTextStyle>,
) -> f64 {
    if source.start > 0
        && matches!(
            styled.index.slice(source.start - 1..source.start),
            Some("\r" | "\n")
        )
    {
        styled.style_at(source.start, theme, predefined).font_size
    } else {
        0.0
    }
}

fn finite_advance(advance: f64) -> Result<f64, MeasurementError> {
    if advance.is_finite() {
        Ok(advance)
    } else {
        Err(MeasurementError::InvalidCluster)
    }
}

pub(in crate::render) fn unmeasured_paragraph(
    styled: &StyledText<'_>,
    source: Range<usize>,
    theme: RenderTheme,
    predefined: Option<PredefinedTextStyle>,
    renderer: &TextRenderer<'_>,
) -> Vec<WrappedLine> {
    let objects = styled.objects.in_range(source.clone());
    if objects.is_empty() {
        return vec![WrappedLine::unmeasured(
            source.clone(),
            styled.line_font_size(source, theme, predefined),
        )];
    }
    let mut lines = Vec::with_capacity(2 * objects.len() + 1);
    let mut start = source.start;
    for object in objects {
        if start < object.source.start {
            let text = start..object.source.start;
            lines.push(WrappedLine::unmeasured(
                text.clone(),
                styled.line_font_size(text, theme, predefined),
            ));
        }
        let measured = object.measured(renderer.settings);
        let mut line = WrappedLine::unmeasured(measured.source.clone(), 0.0);
        line.font_size = styled
            .style_at(measured.source.start, theme, predefined)
            .font_size;
        line.advance = measured.bounds.x_max - measured.bounds.x_min;
        line.objects.push(PositionedObject {
            object: measured,
            x: 0.0,
            prepared: None,
        });
        lines.push(line);
        renderer.object_layout_unsupported(object.span.text_index_utf16);
        start = object.source.end;
    }
    if start < source.end {
        let text = start..source.end;
        lines.push(WrappedLine::unmeasured(
            text.clone(),
            styled.line_font_size(text, theme, predefined),
        ));
    }
    if let Some(line) = lines.first_mut() {
        let prefix_font_size = paragraph_prefix_font_size(styled, &source, theme, predefined);
        line.font_size = line.font_size.max(prefix_font_size);
    }
    lines
}

pub(in crate::render) fn wrap_paragraph(
    styled: &StyledText<'_>,
    range: Range<usize>,
    max_width: f64,
    theme: RenderTheme,
    predefined: Option<PredefinedTextStyle>,
    renderer: &TextRenderer<'_>,
) -> Result<Vec<WrappedLine>, MeasurementError> {
    let text = styled
        .index
        .slice(range.clone())
        .ok_or(MeasurementError::InvalidRange)?;
    let mut breaks = break_candidates(text);
    let measurer = ParagraphMeasurer::new(styled, range.clone(), theme, predefined, renderer)?;
    let measured = measured_items(styled, range.clone(), &measurer, renderer, &mut breaks)?;
    let first_font_size = paragraph_prefix_font_size(styled, &range, theme, predefined);
    let items = measured.items;
    let mut advances = vec![0.0; range.len() + 1];
    let mut cluster_ends = vec![false; range.len() + 1];
    cluster_ends[0] = true;
    let mut source_end = range.start;
    let mut advance = 0.0;
    for item in &items {
        let source = item.source();
        if source.start != source_end || source.end <= source_end || source.end > range.end {
            return Err(MeasurementError::InvalidCluster);
        }
        advance = finite_advance(advance + item.advance())?;
        let end = source.end - range.start;
        advances[end] = advance;
        cluster_ends[end] = true;
        source_end = source.end;
    }
    if source_end != range.end {
        return Err(MeasurementError::InvalidCluster);
    }
    advances[range.len()] = finite_advance(measured.advance)?;
    if breaks
        .candidates
        .iter()
        .any(|candidate| candidate.kind == BreakKind::Mandatory && !cluster_ends[candidate.end])
    {
        return Err(MeasurementError::InvalidCluster);
    }
    breaks
        .candidates
        .retain(|candidate| cluster_ends[candidate.end]);
    breaks.emergency.retain(|&end| cluster_ends[end]);
    let width = if max_width.is_nan() {
        0.0
    } else {
        max_width.max(0.0)
    };
    let mut lines = Vec::new();
    let mut start = 0;
    let mut candidate_index = 0;
    let mut item_index = 0;
    while start < range.len() {
        let mut selected = None;
        let mut overflow_end = None;
        while breaks
            .candidates
            .get(candidate_index)
            .is_some_and(|candidate| candidate.end <= start)
        {
            candidate_index += 1;
        }
        let mut probe = candidate_index;
        for candidate in &breaks.candidates[candidate_index..] {
            if finite_advance(advances[candidate.end] - advances[start])? > width {
                overflow_end = Some(candidate.end);
                break;
            }
            selected = Some(candidate.end);
            probe += 1;
            if candidate.kind == BreakKind::Mandatory {
                break;
            }
        }
        candidate_index = probe;
        if selected.is_none() {
            let overflow_end = overflow_end.ok_or(MeasurementError::InvalidRange)?;
            let first = breaks.emergency.partition_point(|&end| end <= start);
            let last = breaks.emergency.partition_point(|&end| end <= overflow_end);
            let ends = &breaks.emergency[first..last];
            for &end in ends {
                if finite_advance(advances[end] - advances[start])? > width {
                    break;
                }
                selected = Some(end);
            }
            selected = selected.or_else(|| ends.first().copied());
        }
        let end = selected.ok_or(MeasurementError::InvalidRange)?;
        let line_advance = finite_advance(advances[end] - advances[start])?;
        let source = range.start + start..range.start + end;
        let mut placements = Vec::new();
        let mut objects = Vec::new();
        let mut font_size = 0.0_f64;
        let mut text_height = 0.0_f64;
        let mut x = 0.0;
        while let Some(item) = items.get(item_index)
            && item.source().end <= source.end
        {
            match item {
                MeasuredItem::TextCluster(cluster) => {
                    font_size = font_size.max(cluster.run.style.font_size);
                    text_height = text_height.max(cluster.run.style.font_size);
                    placements.push(PositionedCluster {
                        cluster: cluster.clone(),
                        x,
                        extra_advance: 0.0,
                    });
                }
                MeasuredItem::Object {
                    object,
                    font_size: object_font_size,
                } => {
                    font_size = font_size.max(*object_font_size);
                    objects.push(PositionedObject {
                        object: object.clone(),
                        x,
                        prepared: None,
                    });
                }
            }
            x = finite_advance(x + item.advance())?;
            item_index += 1;
        }
        if source == range {
            font_size = measured.font_size;
        }
        if lines.is_empty() {
            font_size = font_size.max(first_font_size);
        }
        lines.push(WrappedLine {
            source,
            font_size,
            text_height,
            advance: line_advance,
            placements,
            objects,
        });
        start = end;
    }
    Ok(lines)
}
#[cfg(test)]
mod tests {
    use super::super::{TextContext, TextSettings};
    use super::*;
    use crate::fonts::{FontBook, fontdb};
    use crate::{
        BoundingBox, ObjectSpanLayoutConstraint, ObjectSpanLayoutOption, ObjectType, PlacedImage,
        RichTextBox, RichTextObjectContent, RichTextObjectSpan, RichTextSpan, RichTextSpanType,
    };
    use std::sync::Arc;

    fn text(value: &str) -> RichTextBox {
        RichTextBox {
            text_area_type: None,
            bbox: BoundingBox::default(),
            rotation_degrees: None,
            text: value.into(),
            color: None,
            highlight_color: None,
            underline: false,
            font_size: Some(45.0),
            runs: Vec::new(),
            spans: Vec::new(),
            paragraphs: Vec::new(),
            object_spans: Vec::new(),
            text_sections: Vec::new(),
            margins: None,
            gravity: None,
        }
    }

    fn span(kind: RichTextSpanType, start: u32, end: u32, payload: &[u8]) -> RichTextSpan {
        RichTextSpan {
            kind,
            start_utf16: start,
            end_utf16: end,
            interval_type: crate::SpanIntervalType::from(0),
            payload: payload.into(),
        }
    }

    fn image(anchor: i32, width: f64, option: ObjectSpanLayoutOption) -> RichTextObjectSpan {
        RichTextObjectSpan {
            object_type: ObjectType::Image,
            object_data: Vec::new(),
            content: Some(RichTextObjectContent::Image(Box::new(PlacedImage {
                bbox: BoundingBox {
                    x_min: -10.0,
                    y_min: -20.0,
                    x_max: -10.0 + width,
                    y_max: 40.0,
                },
                rotation_degrees: None,
                media_id: None,
                media_index: None,
                crop_rect: None,
                original_bbox: None,
                border_media_id: None,
                original_media_id: None,
            }))),
            text_index_utf16: anchor,
            layout_option: option,
            layout_constraint: ObjectSpanLayoutConstraint::Normal,
        }
    }

    fn wrap_with_fonts(
        content: &RichTextBox,
        range: Range<usize>,
        width: f64,
        fonts: &FontBook,
    ) -> Result<Vec<WrappedLine>, MeasurementError> {
        let settings = TextSettings {
            scale: 1.0,
            font_size_delta: 0.0,
            ..Default::default()
        };
        let styled = StyledText::new(content, TextContext::Placed, settings);
        let renderer = TextRenderer::new(settings, fonts);
        wrap_paragraph(
            &styled,
            range,
            width,
            RenderTheme::for_canvas(false),
            None,
            &renderer,
        )
    }

    fn wrap(content: &RichTextBox, width: f64) -> Vec<WrappedLine> {
        wrap_with_fonts(
            content,
            0..content.text.chars().count(),
            width,
            &FontBook::default(),
        )
        .unwrap()
    }

    #[test]
    fn justification_compresses_negative_residuals_without_clamping() {
        let content = text("A\t B");
        let settings = TextSettings {
            scale: 1.0,
            ..Default::default()
        };
        let styled = StyledText::new(&content, TextContext::Placed, settings);
        let mut lines = wrap(&content, 1000.0);
        lines[0].justify(&styled, 50.0).unwrap();
        assert_eq!(lines[0].advance, 50.0);
        assert_eq!(lines[0].placements[1].extra_advance, -50.45703125);
        assert_eq!(lines[0].placements[2].extra_advance, -12.6142578125);
        assert_eq!(lines[0].placements[3].x, 21.98486328125);
    }

    #[test]
    fn justification_moves_inline_objects_with_the_surrounding_text() {
        let mut content = text("A \u{fffc} B");
        content.object_spans = vec![image(2, 40.0, ObjectSpanLayoutOption::Inline)];
        let settings = TextSettings {
            scale: 1.0,
            ..Default::default()
        };
        let styled = StyledText::new(&content, TextContext::Placed, settings);
        let mut lines = wrap(&content, 1000.0);
        lines[0].justify(&styled, 200.0).unwrap();
        assert_eq!(lines[0].advance, 200.0);
        assert_eq!(lines[0].objects[0].x, 80.670166015625);
        assert_eq!(lines[0].placements.last().unwrap().x, 171.98486328125);
        assert_eq!(lines[0].objects[0].object.source, 2..3);
        assert_eq!(lines[0].placements.last().unwrap().cluster.source, 4..5);
    }

    #[test]
    fn invalid_justification_geometry_does_not_mutate_retained_positions() {
        let content = text("A B");
        let settings = TextSettings {
            scale: 1.0,
            ..Default::default()
        };
        let styled = StyledText::new(&content, TextContext::Placed, settings);
        let mut lines = wrap(&content, 1000.0);
        let advance = lines[0].advance;
        let positions: Vec<_> = lines[0]
            .placements
            .iter()
            .map(|placement| placement.x)
            .collect();
        for width in [f64::NAN, f64::INFINITY, f64::MAX] {
            assert!(lines[0].justify(&styled, width).is_err());
            assert_eq!(lines[0].advance, advance);
            assert_eq!(
                lines[0]
                    .placements
                    .iter()
                    .map(|placement| placement.x)
                    .collect::<Vec<_>>(),
                positions
            );
            assert!(
                lines[0]
                    .placements
                    .iter()
                    .all(|placement| placement.extra_advance == 0.0)
            );
        }
    }

    fn ranges(lines: &[WrappedLine]) -> Vec<Range<usize>> {
        lines.iter().map(|line| line.source.clone()).collect()
    }

    fn assert_single_line(lines: &[WrappedLine], expected: Range<usize>) {
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].source, expected);
    }

    #[test]
    fn inline_object_uses_actual_width_and_positions_surrounding_text() {
        let mut content = text("A\u{fffc}A");
        content.object_spans = vec![image(1, 40.0, ObjectSpanLayoutOption::Inline)];
        content.spans = vec![span(
            RichTextSpanType::FontSize,
            1,
            2,
            &200.0_f32.to_le_bytes(),
        )];
        let letter_width = wrap(&text("A"), 1000.0)[0].advance;
        let lines = wrap(&content, 2.0 * letter_width + 40.0);
        assert_single_line(&lines, 0..3);
        let line = &lines[0];
        assert_eq!(line.advance, 2.0 * letter_width + 40.0);
        assert_eq!(line.font_size, 200.0);
        assert_eq!(line.text_height, 45.0);
        assert_eq!(line.base_height(), 60.0);
        assert_eq!(line.object_height(), 60.0);
        assert_eq!(line.object_margins(), [0.0, 0.0]);
        assert_eq!(line.placements.len(), 2);
        assert_eq!(line.placements[0].cluster.source, 0..1);
        assert_eq!(line.placements[1].cluster.source, 2..3);
        assert_eq!(line.placements[1].x, letter_width + 40.0);
        assert_eq!(line.objects.len(), 1);
        assert_eq!(line.objects[0].object.source, 1..2);
        assert_eq!(line.objects[0].object.span_index, 0);
        assert_eq!(line.objects[0].x, letter_width);
        assert_eq!(
            ranges(&wrap(&content, line.advance - 0.000001)),
            [0..2, 2..3]
        );
    }

    #[test]
    fn block_objects_force_own_lines_with_actual_alignment_width() {
        for (option, margin) in [
            (ObjectSpanLayoutOption::Block, 0.0),
            (ObjectSpanLayoutOption::BlockWithSmallMargin, 10.0),
            (ObjectSpanLayoutOption::BlockWithMediumMargin, 20.0),
            (ObjectSpanLayoutOption::Other(9), 0.0),
        ] {
            let mut content = text("A\u{fffc}B");
            content.object_spans = vec![image(1, 20.0, option)];
            let lines = wrap(&content, 1000.0);
            assert_eq!(ranges(&lines), [0..1, 1..2, 2..3]);
            assert!(lines[0].objects.is_empty());
            assert!(lines[2].objects.is_empty());
            assert_eq!(lines[1].advance, 20.0);
            assert_eq!(lines[1].font_size, 45.0);
            assert_eq!(lines[1].text_height, 0.0);
            assert!(lines[1].placements.is_empty());
            assert_eq!(lines[1].objects[0].x, 0.0);
            assert_eq!(lines[1].object_margins(), [margin, margin]);
        }
    }

    #[test]
    fn object_only_paragraph_uses_source_font_metric_without_resolving_a_face() {
        let mut content = text("\u{fffc}");
        content.font_size = Some(500.0);
        content.object_spans = vec![image(0, 200.0, ObjectSpanLayoutOption::Inline)];
        let fonts = FontBook::new(Arc::new(fontdb::Database::new()));
        let lines = wrap_with_fonts(&content, 0..1, 0.0, &fonts).unwrap();
        assert_single_line(&lines, 0..1);
        assert_eq!(lines[0].font_size, 500.0);
        assert_eq!(lines[0].text_height, 0.0);
        assert_eq!(lines[0].base_height(), 60.0);
        assert_eq!(lines[0].advance, 200.0);
        assert_eq!(lines[0].object_height(), 60.0);
        assert!(lines[0].placements.is_empty());
    }

    #[test]
    fn preceding_native_separator_seeds_only_the_first_object_line_font_size() {
        for separator in ["\r", "\n"] {
            let mut content = text(&format!("A{separator}\u{fffc}\u{fffc}"));
            content.object_spans = vec![
                image(2, 200.0, ObjectSpanLayoutOption::Inline),
                image(3, 300.0, ObjectSpanLayoutOption::Inline),
            ];
            content.spans = vec![span(
                RichTextSpanType::FontSize,
                2,
                3,
                &90.0_f32.to_le_bytes(),
            )];
            let fonts = FontBook::new(Arc::new(fontdb::Database::new()));
            let lines = wrap_with_fonts(&content, 2..4, 0.0, &fonts).unwrap();
            assert_eq!(ranges(&lines), [2..3, 3..4]);
            assert_eq!(lines[0].font_size, 90.0);
            assert_eq!(lines[1].font_size, 45.0);
            assert!(lines.iter().all(|line| line.text_height == 0.0));
            assert!(lines.iter().all(|line| line.base_height() == 60.0));
            assert!(lines.iter().all(|line| line.placements.is_empty()));
        }
        let mut content = text("A\u{fffc}");
        content.object_spans = vec![image(1, 200.0, ObjectSpanLayoutOption::Inline)];
        let lines = wrap_with_fonts(&content, 1..2, 0.0, &FontBook::default()).unwrap();
        assert_eq!(lines[0].font_size, 45.0);
    }

    #[test]
    fn a_leading_separator_retains_font_metric_but_its_height_is_replaced_by_content() {
        let mut content = text("\n\u{fffc}");
        let mut object = image(1, 20.0, ObjectSpanLayoutOption::Inline);
        let Some(RichTextObjectContent::Image(image)) = &mut object.content else {
            unreachable!()
        };
        image.bbox.y_max = image.bbox.y_min + 20.0;
        content.object_spans.push(object);
        content.spans.push(span(
            RichTextSpanType::FontSize,
            1,
            2,
            &900.0_f32.to_le_bytes(),
        ));
        let lines = wrap_with_fonts(&content, 1..2, 100.0, &FontBook::default()).unwrap();
        assert_eq!(lines[0].font_size, 900.0);
        assert_eq!(lines[0].text_height, 0.0);
        assert_eq!(lines[0].base_height(), 20.0);
    }

    #[test]
    fn oversized_inline_objects_progress_without_losing_neighboring_source() {
        let mut content = text("A\u{fffc}\u{fffc}B");
        content.object_spans = vec![
            image(1, 200.0, ObjectSpanLayoutOption::Inline),
            image(2, 300.0, ObjectSpanLayoutOption::Inline),
        ];
        let lines = wrap(&content, 0.0);
        assert_eq!(ranges(&lines), [0..1, 1..2, 2..3, 3..4]);
        assert_eq!(lines[1].objects[0].object.span_index, 0);
        assert_eq!(lines[2].objects[0].object.span_index, 1);
        assert_eq!(lines[1].advance, 200.0);
        assert_eq!(lines[2].advance, 300.0);
    }

    #[test]
    fn objects_preserve_full_paragraph_direction_context() {
        let mut content = text("ب\u{fffc}.");
        content.object_spans = vec![image(1, 20.0, ObjectSpanLayoutOption::Inline)];
        let lines = wrap(&content, f64::INFINITY);
        assert_single_line(&lines, 0..3);
        assert_eq!(lines[0].placements.len(), 2);
        let punctuation = &lines[0].placements[1].cluster;
        assert_eq!(punctuation.source, 2..3);
        assert_eq!(
            punctuation.run.direction,
            crate::fonts::Direction::RightToLeft
        );
    }

    #[test]
    fn mixed_source_uses_utf16_anchors_and_grapheme_emergency_boundaries() {
        let mut content = text("😀\u{fffc}e\u{301}");
        content.object_spans = vec![image(2, 200.0, ObjectSpanLayoutOption::Inline)];
        let lines = wrap(&content, 0.0);
        assert_eq!(ranges(&lines), [0..1, 1..2, 2..4]);
        assert_eq!(lines[1].objects[0].object.source, 1..2);
        assert_eq!(lines[2].placements[0].cluster.source, 2..4);
    }

    #[test]
    fn text_on_each_side_of_objects_retains_cached_cross_line_kerning() {
        let mut content = text("AVA\u{fffc}AVA");
        content.object_spans = vec![image(3, 20.0, ObjectSpanLayoutOption::Block)];
        let lines = wrap(&content, 55.0);
        assert_eq!(ranges(&lines), [0..2, 2..3, 3..4, 4..6, 6..7]);
        for (first, second) in [(&lines[0], &lines[1]), (&lines[3], &lines[4])] {
            assert_eq!(first.advance, 54.42626953125);
            assert_eq!(second.advance, 29.35546875);
            assert!(Arc::ptr_eq(
                &first.placements[0].cluster.run,
                &second.placements[0].cluster.run,
            ));
        }
    }

    #[test]
    fn unavailable_font_fallback_retains_objects_and_source_without_width_estimates() {
        let mut content = text("A\u{fffc}V\u{fffc}B");
        content.object_spans = vec![
            image(1, 40.0, ObjectSpanLayoutOption::Inline),
            image(3, 50.0, ObjectSpanLayoutOption::BlockWithSmallMargin),
        ];
        let settings = TextSettings {
            scale: 1.0,
            font_size_delta: 0.0,
            ..Default::default()
        };
        let styled = StyledText::new(&content, TextContext::Placed, settings);
        let fonts = FontBook::new(Arc::new(fontdb::Database::new()));
        let renderer = TextRenderer::new(settings, &fonts);
        let lines = unmeasured_paragraph(
            &styled,
            0..5,
            RenderTheme::for_canvas(false),
            None,
            &renderer,
        );
        assert_eq!(ranges(&lines), [0..1, 1..2, 2..3, 3..4, 4..5]);
        assert_eq!(lines[1].advance, 40.0);
        assert_eq!(lines[3].advance, 50.0);
        assert_eq!(lines[3].object_margins(), [10.0, 10.0]);
        assert!(lines.iter().all(|line| line.placements.is_empty()));
        for line in [&lines[0], &lines[2], &lines[4]] {
            assert_eq!(line.advance, 0.0);
            assert_eq!(line.font_size, 45.0);
            assert!(line.objects.is_empty());
        }
        assert_eq!(lines[1].objects[0].object.source, 1..2);
        assert_eq!(lines[3].objects[0].object.source, 3..4);
        assert!(renderer.diagnostics().is_empty());
        let issues = renderer.object_diagnostics();
        assert_eq!(issues.len(), 2);
        assert_eq!(issues[0].anchor_utf16, 1);
        assert_eq!(issues[1].anchor_utf16, 3);
        assert!(issues.iter().all(|issue| {
            issue.kind == super::super::objects::ObjectDiagnosticKind::MixedParagraphLayout
        }));
    }

    #[test]
    fn advance_arithmetic_overflow_returns_error_without_clamping_finite_values() {
        let maximum = f64::MAX;
        let summed = maximum + maximum;
        let difference = maximum - -maximum;
        for overflow in [summed, difference, summed - difference] {
            assert!(matches!(
                finite_advance(overflow),
                Err(MeasurementError::InvalidCluster)
            ));
        }
        for advance in [-maximum, -45.0, 0.0, 45.0, maximum] {
            assert_eq!(finite_advance(advance).unwrap(), advance);
        }
    }

    #[test]
    fn wrapped_placements_keep_shared_paragraph_glyphs_and_kerning_positions() {
        let lines = wrap(&text("AVA"), 55.0);
        assert_eq!(ranges(&lines), [0..2, 2..3]);
        assert_eq!(lines[0].advance, 54.42626953125);
        assert_eq!(lines[1].advance, 29.35546875);
        assert_eq!(lines[0].placements.len(), 2);
        assert_eq!(lines[1].placements.len(), 1);
        assert_eq!(lines[0].placements[0].x, 0.0);
        assert_eq!(lines[0].placements[1].x, 27.44384765625);
        assert_eq!(lines[1].placements[0].x, 0.0);
        let run = &lines[0].placements[0].cluster.run;
        assert!(Arc::ptr_eq(run, &lines[0].placements[1].cluster.run));
        assert!(Arc::ptr_eq(run, &lines[1].placements[0].cluster.run));
        assert_eq!(run.glyphs[1].raw.x_advance, 1228);
        for (placement, character) in lines
            .iter()
            .flat_map(|line| &line.placements)
            .zip("AVA".chars())
        {
            let offset = placement
                .cluster
                .paint_offset(&character.to_string())
                .unwrap()
                .unwrap();
            assert_eq!(offset.x, 0.0);
            assert_eq!(offset.y, 0.0);
        }
    }

    #[test]
    fn pinned_abc_fits_exact_advance_but_wraps_below_it() {
        let content = text("ABC");
        let exact = wrap(&content, 86.66015625);
        assert_single_line(&exact, 0..3);
        assert_eq!(exact[0].font_size, 45.0);
        assert_eq!(
            ranges(&wrap(&content, 86.66015625 - 0.00000001)),
            [0..2, 2..3]
        );
    }

    #[test]
    fn paragraph_kerning_is_retained_across_emergency_line_breaks() {
        let face = rustybuzz::Face::from_slice(
            include_bytes!("../../../assets/fonts/Roboto-Regular.ttf"),
            0,
        )
        .unwrap();
        let upstream_advances = |value: &str| {
            let mut buffer = rustybuzz::UnicodeBuffer::new();
            buffer.push_str(value);
            buffer.guess_segment_properties();
            rustybuzz::shape(&face, &[], buffer)
                .glyph_positions()
                .iter()
                .map(|position| position.x_advance)
                .collect::<Vec<_>>()
        };
        let paragraph_advances = upstream_advances("AVA");
        let isolated_advances = upstream_advances("AV");
        assert_eq!(paragraph_advances, [1249, 1228, 1336]);
        assert_eq!(isolated_advances, [1249, 1303]);
        assert_eq!(
            f64::from(paragraph_advances[..2].iter().sum::<i32>()) / 2048.0 * 45.0,
            54.42626953125
        );
        assert_eq!(
            f64::from(isolated_advances.iter().sum::<i32>()) / 2048.0 * 45.0,
            56.07421875
        );
        assert_eq!(ranges(&wrap(&text("AVA"), 55.0)), [0..2, 2..3]);
        assert_eq!(ranges(&wrap(&text("AVAV"), 56.0)), [0..2, 2..3, 3..4]);
    }

    #[test]
    fn line_maximum_font_size_excludes_neighboring_style_ranges() {
        let mut content = text("AAA");
        content.spans = vec![span(
            RichTextSpanType::FontSize,
            1,
            2,
            &90.0_f32.to_le_bytes(),
        )];
        let lines = wrap(&content, 60.0);
        assert_eq!(ranges(&lines), [0..1, 1..2, 2..3]);
        assert_eq!(
            lines.iter().map(|line| line.font_size).collect::<Vec<_>>(),
            [45.0, 90.0, 45.0]
        );
    }

    #[test]
    fn arabic_combining_graphemes_preserve_contiguous_logical_source_ranges() {
        let content = text("Aب\u{64e}ت\u{64e}Z");
        let lines = wrap(&content, 0.0);
        assert_eq!(ranges(&lines), [0..1, 1..3, 3..5, 5..6]);
    }

    #[test]
    fn long_unbroken_token_retains_every_scalar_in_expected_line_ranges() {
        let content = text(&"A".repeat(4096));
        let lines = wrap(&content, 60.0);
        assert_eq!(lines.len(), 2048);
        for (index, line) in lines.iter().enumerate() {
            assert_eq!(line.source, index * 2..index * 2 + 2);
            assert_eq!(line.font_size, 45.0);
        }
    }

    #[test]
    fn local_font_size_and_selected_styles_change_breaks() {
        let content = text("ABC");
        assert_single_line(&wrap(&content, 87.0), 0..3);
        let mut larger = content.clone();
        larger.spans = vec![span(
            RichTextSpanType::FontSize,
            1,
            2,
            &90.0_f32.to_le_bytes(),
        )];
        let lines = wrap(&larger, 87.0);
        assert_eq!(ranges(&lines), [0..2, 2..3]);
        assert_eq!(
            lines.iter().map(|line| line.font_size).collect::<Vec<_>>(),
            [90.0, 45.0]
        );
        let mut bold = content.clone();
        bold.spans = vec![span(RichTextSpanType::Bold, 0, 3, &[1, 0])];
        assert_eq!(ranges(&wrap(&bold, 87.0)), [0..2, 2..3]);
        let mut italic = content.clone();
        italic.spans = vec![span(RichTextSpanType::Italic, 0, 3, &[1, 0])];
        assert_eq!(ranges(&wrap(&content, 85.0)), [0..2, 2..3]);
        assert_single_line(&wrap(&italic, 85.0), 0..3);
    }

    #[test]
    fn wrapped_ranges_preserve_all_leading_repeated_and_trailing_spaces() {
        let content = text(" A  B   C  ");
        let lines = wrap(&content, 50.0);
        assert!(lines.len() > 1);
        assert_eq!(lines.first().unwrap().source.start, 0);
        assert_eq!(lines.last().unwrap().source.end, 11);
        for pair in lines.windows(2) {
            assert_eq!(pair[0].source.end, pair[1].source.start);
        }
        let retained = lines
            .iter()
            .map(|line| {
                content
                    .text
                    .chars()
                    .skip(line.source.start)
                    .take(line.source.len())
                    .collect::<String>()
            })
            .collect::<String>();
        assert_eq!(retained, " A  B   C  ");
    }

    #[test]
    fn emergency_breaks_keep_combining_and_emoji_clusters_with_scalar_offsets() {
        let content = text("Ae\u{301}👩‍👩‍👧‍👦Z");
        let lines = wrap_with_fonts(&content, 1..10, 0.0, &FontBook::default()).unwrap();
        assert_eq!(ranges(&lines), [1..3, 3..10]);
    }

    #[test]
    fn nonbreaking_space_is_only_broken_when_the_whole_group_overflows() {
        let content = text("A\u{a0}B");
        assert_single_line(&wrap(&content, 80.0), 0..3);
        assert_eq!(ranges(&wrap(&content, 60.0)), [0..2, 2..3]);
    }

    #[test]
    fn crlf_mandatory_break_is_retained_as_one_source_boundary() {
        let content = text("A\r\nB");
        assert_eq!(ranges(&wrap(&content, f64::INFINITY)), [0..3, 3..4]);
    }

    #[test]
    fn empty_text_and_oversized_graphemes_terminate_without_dropping_source() {
        assert!(wrap(&text(""), 0.0).is_empty());
        assert_single_line(&wrap(&text("e\u{301}"), 0.0), 0..2);
        assert_single_line(&wrap(&text("👩‍👩‍👧‍👦"), 0.0), 0..7);
        assert_eq!(ranges(&wrap(&text("ABC"), f64::NAN)), [0..1, 1..2, 2..3]);
        assert_eq!(ranges(&wrap(&text("ABC"), -1.0)), [0..1, 1..2, 2..3]);
    }

    #[test]
    fn caller_monospace_font_changes_breaks_from_default_roboto() {
        let mut database = fontdb::Database::new();
        database.load_font_data(
            include_bytes!("../../../assets/fonts/RobotoMono-Regular.ttf").to_vec(),
        );
        database.set_sans_serif_family("Roboto Mono");
        let fonts = FontBook::new(Arc::new(database));
        let content = text("ABC");
        assert_eq!(ranges(&wrap(&content, 83.0)), [0..2, 2..3]);
        let caller = wrap_with_fonts(&content, 0..3, 83.0, &fonts).unwrap();
        assert_single_line(&caller, 0..3);
    }

    #[test]
    fn unavailable_fonts_return_typed_error_instead_of_estimated_widths() {
        let fonts = FontBook::new(Arc::new(fontdb::Database::new()));
        let content = text("ABC");
        let result = wrap_with_fonts(&content, 0..3, 1000.0, &fonts);
        assert!(
            matches!(result, Err(MeasurementError::UnavailableFace(family)) if family == "Roboto")
        );
    }
}
