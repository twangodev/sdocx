use crate::{
    Document, ObjectSpanLayoutConstraint, ObjectSpanLayoutOption, ObjectType, ParagraphAlignment,
    RichTextBox, RichTextObjectContent, RichTextParagraphType, RichTextSpanType,
};

use super::PreparedBodyPlan;

/// Native-style result for one full-width body query band.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct BodyQueryObservation {
    pub visible: bool,
    pub line_index: Option<usize>,
    pub line_count: usize,
    /// Native rectangle order: left, top, right, bottom.
    pub line_rect: [f32; 4],
}

#[derive(Clone, Copy, Debug)]
struct BodyQueryLine {
    rect: [f32; 4],
    owns_visible_image: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct BodyQueryImageAnchor {
    source_index: usize,
    span_index: usize,
}

/// A narrow source-query view of the cached body layout used by the renderer.
/// It admits only LF-separated resolved image replacement objects.
pub(crate) struct ContinuousBodyQuerySnapshot {
    lines: Vec<BodyQueryLine>,
}

impl ContinuousBodyQuerySnapshot {
    pub(super) fn from_plan(plan: &PreparedBodyPlan, document: &Document) -> Option<Self> {
        let anchors = query_image_sources(&plan.text, document)?;
        if !plan.object_issues.is_empty() {
            return None;
        }

        let mut source_by_span = vec![None; plan.text.object_spans.len()];
        for anchor in anchors {
            *source_by_span.get_mut(anchor.span_index)? = Some(anchor.source_index);
        }
        let mut owner_counts = vec![0_u8; source_by_span.len()];
        let mut lines = Vec::with_capacity(plan.layout.lines.len());
        for (index, line) in plan.layout.lines.iter().enumerate() {
            let x = line.x
                + super::text::line_alignment_offset(line.line.advance, line.width, line.alignment);
            let first_empty_line = index == 0
                && line.line.source.is_empty()
                && line.line.objects.is_empty()
                && line.line.placements.is_empty();
            let left = if first_empty_line { 0.0 } else { x as f32 };
            let right = if first_empty_line {
                0.0
            } else {
                (x + line.line.advance) as f32
            };
            let rect = [left, line.top as f32, right, line.bottom as f32];
            if !valid_line_rect(rect) {
                return None;
            }
            let mut owns_visible_image = false;
            for object in &line.line.objects {
                let source_index = source_by_span
                    .get(object.object.span_index)
                    .copied()
                    .flatten()?;
                if object.object.source != (source_index..source_index + 1) {
                    return None;
                }
                let owner_count = owner_counts.get_mut(object.object.span_index)?;
                *owner_count = owner_count.checked_add(1)?;
                owns_visible_image = true;
            }
            lines.push(BodyQueryLine {
                rect,
                owns_visible_image,
            });
        }
        if lines.is_empty() || owner_counts.iter().any(|&count| count != 1) {
            return None;
        }
        Some(Self { lines })
    }

    /// Return the source line rectangle as `[left, top, right, bottom]`.
    pub(crate) fn line_rect(&self, line_index: usize) -> Option<[f32; 4]> {
        self.lines.get(line_index).map(|line| line.rect)
    }

    /// Match the native clamped binary search. At a shared line-top boundary,
    /// the preceding line owns the boundary.
    pub(crate) fn line_index(&self, y: f32) -> Option<usize> {
        if !y.is_finite() || self.lines.is_empty() {
            return None;
        }
        let preceding = self.lines.partition_point(|line| line.rect[3] < y);
        Some(preceding.min(self.lines.len() - 1))
    }

    /// Match native `GetLineRangeInRect` followed by `HasVisibleTextNParagraph`
    /// for the admitted LF-plus-image source class.
    pub(crate) fn query(&self, rect: [f32; 4]) -> BodyQueryObservation {
        let line_index = self.line_index(rect[1]);
        let line_rect = line_index
            .and_then(|index| self.line_rect(index))
            .unwrap_or([0.0; 4]);
        let visible = self.line_range(rect).is_some_and(|(first, last)| {
            self.lines[first..=last]
                .iter()
                .any(|line| line.owns_visible_image)
        });
        BodyQueryObservation {
            visible,
            line_index,
            line_count: self.lines.len(),
            line_rect,
        }
    }

    fn line_range(&self, query: [f32; 4]) -> Option<(usize, usize)> {
        if !valid_query_rect(query) {
            return None;
        }
        let first = self
            .lines
            .iter()
            .position(|line| rect_contains_or_intersects(query, line.rect))?;
        let last = self
            .lines
            .iter()
            .rposition(|line| rect_contains_or_intersects(query, line.rect))?;
        (first <= last).then_some((first, last))
    }
}

pub(super) fn query_image_sources(
    text: &RichTextBox,
    document: &Document,
) -> Option<Vec<BodyQueryImageAnchor>> {
    if text
        .text
        .chars()
        .any(|character| !matches!(character, '\n' | '\u{fffc}'))
        || !text.runs.is_empty()
        || text.text_area_type.is_some()
        || text.bbox != crate::BoundingBox::default()
        || text.rotation_degrees.is_some()
        || text.highlight_color.is_some()
        || text.underline
        || text
            .font_size
            .is_some_and(|size| !size.is_finite() || size <= 0.0)
        || text.margins.is_some_and(|margins| {
            margins
                .iter()
                .any(|value| !value.is_finite() || *value < 0.0)
        })
        || text.gravity.is_some_and(|gravity| gravity != 0)
    {
        return None;
    }

    let mut marker_offsets = Vec::new();
    let mut utf16_offset = 0_usize;
    for (source_index, character) in text.text.chars().enumerate() {
        if character == '\u{fffc}' {
            marker_offsets.push((source_index, utf16_offset));
        }
        utf16_offset = utf16_offset.checked_add(character.len_utf16())?;
    }
    if marker_offsets.is_empty()
        || marker_offsets.len() != text.object_spans.len()
        || marker_offsets
            .windows(2)
            .any(|pair| pair[1].0 == pair[0].0 + 1)
    {
        return None;
    }

    let utf16_len = text.text.encode_utf16().count();
    if text.text_sections.iter().any(|section| {
        section.start_utf16 < 0
            || section.length_utf16 < 0
            || section
                .start_utf16
                .checked_add(section.length_utf16)
                .and_then(|end| usize::try_from(end).ok())
                .is_none_or(|end| end > utf16_len)
    }) || text.spans.iter().any(|span| {
        span.start_utf16 > span.end_utf16
            || usize::try_from(span.end_utf16).is_err()
            || usize::try_from(span.end_utf16).is_ok_and(|end| end > utf16_len)
            || match span.kind {
                RichTextSpanType::FontSize => {
                    !matches!(span.payload.len(), 4 | 8)
                        || span
                            .font_size_value()
                            .is_none_or(|size| !size.is_finite() || size <= 0.0)
                }
                RichTextSpanType::ForegroundColor => !matches!(span.payload.len(), 4 | 8),
                _ => true,
            }
    }) || text
        .paragraphs
        .iter()
        .any(|paragraph| match paragraph.kind {
            RichTextParagraphType::Alignment => !matches!(
                paragraph.alignment(),
                Some(
                    ParagraphAlignment::Left
                        | ParagraphAlignment::Right
                        | ParagraphAlignment::Center
                        | ParagraphAlignment::Both
                )
            ),
            RichTextParagraphType::ParsingState => paragraph.is_parsed().is_none(),
            _ => true,
        })
    {
        return None;
    }

    let mut span_by_marker = vec![None; marker_offsets.len()];
    for (span_index, span) in text.object_spans.iter().enumerate() {
        let anchor_utf16 = usize::try_from(span.text_index_utf16).ok()?;
        let marker_index = marker_offsets
            .binary_search_by_key(&anchor_utf16, |&(_, marker_utf16)| marker_utf16)
            .ok()?;
        if span_by_marker[marker_index].is_some()
            || span.object_type != ObjectType::Image
            || span.layout_option != ObjectSpanLayoutOption::Block
            || span.layout_constraint != ObjectSpanLayoutConstraint::Normal
        {
            return None;
        }
        let Some(RichTextObjectContent::Image(image)) = span.content.as_ref() else {
            return None;
        };
        if image.rotation_degrees.is_some()
            || image.crop_rect.is_some()
            || image.original_bbox.is_some()
            || image.border_media_id.is_some()
            || image.original_media_id.is_some()
            || !valid_bbox(image.bbox)
        {
            return None;
        }
        let asset = document.metadata.media_assets.get(image.media_index?)?;
        if asset.data.is_empty()
            || !asset.mime_type.starts_with("image/")
            || image
                .media_id
                .is_some_and(|id| asset.archive_id.is_some_and(|archive_id| id != archive_id))
        {
            return None;
        }
        span_by_marker[marker_index] = Some(span_index);
    }

    marker_offsets
        .into_iter()
        .zip(span_by_marker)
        .map(|((source_index, _), span_index)| {
            Some(BodyQueryImageAnchor {
                source_index,
                span_index: span_index?,
            })
        })
        .collect()
}

fn valid_bbox(bbox: crate::BoundingBox) -> bool {
    [bbox.x_min, bbox.y_min, bbox.x_max, bbox.y_max]
        .iter()
        .all(|value| value.is_finite())
        && bbox.x_min < bbox.x_max
        && bbox.y_min < bbox.y_max
}

fn valid_line_rect(rect: [f32; 4]) -> bool {
    rect.iter().all(|value| value.is_finite()) && rect[0] <= rect[2] && rect[1] <= rect[3]
}

fn valid_query_rect(rect: [f32; 4]) -> bool {
    rect.iter().all(|value| value.is_finite()) && rect[0] <= rect[2] && rect[1] <= rect[3]
}

fn rect_contains_or_intersects(query: [f32; 4], line: [f32; 4]) -> bool {
    let contains =
        query[0] <= line[0] && query[1] <= line[1] && query[2] >= line[2] && query[3] >= line[3];
    let intersects = query[0] < query[2]
        && query[1] < query[3]
        && query[0] < line[2]
        && query[2] > line[0]
        && query[1] < line[3]
        && query[3] > line[1];
    contains || intersects
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(lines: &[([f32; 4], bool)]) -> ContinuousBodyQuerySnapshot {
        ContinuousBodyQuerySnapshot {
            lines: lines
                .iter()
                .map(|(rect, owns_visible_image)| BodyQueryLine {
                    rect: *rect,
                    owns_visible_image: *owns_visible_image,
                })
                .collect(),
        }
    }

    #[test]
    fn source_band_uses_native_boundaries_and_full_line_ownership() {
        let snapshot = snapshot(&[
            ([0.0, 30.0, 0.0, 78.6], false),
            ([48.0, 90.0, 48.0, 127.3], false),
            ([48.0, 5473.206, 567.4821, f32::from_bits(0x45c0b95a)], true),
            ([48.0, f32::from_bits(0x45c0b95a), 48.0, 6215.769], false),
        ]);

        assert_eq!(snapshot.line_index(78.6), Some(0));
        assert_eq!(
            snapshot.line_index(f32::from_bits(78.6_f32.to_bits() + 1)),
            Some(1)
        );
        assert_eq!(snapshot.line_index(80.0), Some(1));
        assert_eq!(snapshot.line_index(90.0), Some(1));
        assert_eq!(snapshot.line_index(-100.0), Some(0));
        assert_eq!(snapshot.line_index(100_000.0), Some(3));
        assert!(
            !snapshot
                .query([0.0, f32::from_bits(0x45a984d9), 1080.0, 5473.206])
                .visible
        );
        assert!(snapshot.query([0.0, 5473.206, 1080.0, 5474.206]).visible);
        assert!(snapshot.query([0.0, 6154.56, 1080.0, 6154.57]).visible);
        assert!(
            snapshot
                .query([
                    0.0,
                    f32::from_bits(0x45c0b15a),
                    1080.0,
                    f32::from_bits(0x45c0b95a)
                ])
                .visible
        );
        assert!(
            !snapshot
                .query([
                    0.0,
                    f32::from_bits(0x45c0b95a),
                    1080.0,
                    f32::from_bits(0x45c0c15a)
                ])
                .visible
        );
        assert!(!snapshot.query([0.0, 6065.0, 1080.0, 6065.0]).visible);
        assert!(!snapshot.query([600.0, 6065.0, 800.0, 6066.0]).visible);
    }

    #[test]
    fn contained_degenerate_line_is_selected_but_not_visible() {
        let snapshot = snapshot(&[
            ([0.0, 10.0, 0.0, 20.0], false),
            ([48.0, 20.0, 48.0, 30.0], false),
        ]);
        assert!(!snapshot.query([0.0, 0.0, 100.0, 40.0]).visible);
        assert!(!snapshot.query([0.0, 21.0, 49.0, 22.0]).visible);
        assert!(!snapshot.query([0.0, 20.0, 0.0, 21.0]).visible);
        assert!(!snapshot.query([0.0, 22.0, 100.0, 22.0]).visible);
    }
}
