//! Text slicing only: character ranges select content, UTF-16 offsets select
//! native style and object records, and native paragraph ordinals select styles.
//! Page capture and object placement remain the layout owner's responsibility.

use std::ops::Range;

use crate::RichTextBox;
use crate::text_index::TextIndex;

impl RichTextBox {
    /// Return a logical character-indexed slice with intersecting style records rebased.
    /// Font carets at either boundary retain their original interval policy.
    pub fn slice_chars(&self, range: Range<usize>) -> Option<Self> {
        self.slice_indexed(&TextIndex::new(&self.text), range)
    }

    pub(super) fn slice_indexed(&self, index: &TextIndex<'_>, range: Range<usize>) -> Option<Self> {
        let text = index.slice(range.clone())?;
        let start_utf16 = index.char_to_utf16(range.start)?;
        let end_utf16 = index.char_to_utf16(range.end)?;
        let paragraph_range = paragraph_range_for_chars(index, range.clone())?;

        let runs = self
            .runs
            .iter()
            .filter_map(|run| {
                if run.start >= run.end || run.end > index.len() {
                    return None;
                }
                let start = run.start.max(range.start);
                let end = run.end.min(range.end);
                (start < end).then(|| crate::types::RichTextRun {
                    start: start - range.start,
                    end: end - range.start,
                    bold: run.bold,
                    italic: run.italic,
                })
            })
            .collect();
        let spans = self
            .spans
            .iter()
            .filter_map(|span| slice_span(span, index, start_utf16..end_utf16))
            .collect();
        let paragraphs = self
            .paragraphs
            .iter()
            .filter_map(|paragraph| {
                let start = paragraph.start_paragraph.max(paragraph_range.start);
                let end = paragraph.end_paragraph.min(paragraph_range.end);
                (start < end).then(|| {
                    let mut paragraph = paragraph.clone();
                    paragraph.start_paragraph = start - paragraph_range.start;
                    paragraph.end_paragraph = end - paragraph_range.start;
                    paragraph
                })
            })
            .collect();
        let object_spans = self
            .object_spans
            .iter()
            .filter_map(|object_span| {
                let index = u32::try_from(object_span.text_index_utf16).ok()?;
                (index >= start_utf16 && index < end_utf16).then(|| {
                    let mut object_span = object_span.clone();
                    object_span.text_index_utf16 =
                        i32::try_from(index - start_utf16).unwrap_or(i32::MAX);
                    object_span
                })
            })
            .collect();

        let text = text.to_string();
        let text_sections = vec![crate::types::RichTextSection {
            start_utf16: 0,
            length_utf16: i32::try_from(end_utf16.checked_sub(start_utf16)?).ok()?,
        }];
        Some(Self {
            text_area_type: self.text_area_type,
            bbox: self.bbox,
            rotation_degrees: self.rotation_degrees,
            text,
            color: self.color,
            highlight_color: self.highlight_color,
            underline: self.underline,
            font_size: self.font_size,
            runs,
            spans,
            paragraphs,
            object_spans,
            text_sections,
            margins: self.margins,
            gravity: self.gravity,
        })
    }
}

fn slice_span(
    span: &crate::RichTextSpan,
    index: &TextIndex<'_>,
    range: Range<u32>,
) -> Option<crate::RichTextSpan> {
    if span.start_utf16 > span.end_utf16
        || index.utf16_to_char(span.start_utf16).is_none()
        || index.utf16_to_char(span.end_utf16).is_none()
    {
        return None;
    }
    let start = span.start_utf16.max(range.start);
    let end = span.end_utf16.min(range.end);
    let font_caret = span.start_utf16 == span.end_utf16
        && span.kind == crate::RichTextSpanType::FontSize
        && range.start <= span.start_utf16
        && span.start_utf16 <= range.end;
    (start < end || font_caret).then(|| {
        let mut span = span.clone();
        span.start_utf16 = start - range.start;
        span.end_utf16 = end - range.start;
        span
    })
}

fn paragraph_range_for_chars(index: &TextIndex<'_>, range: Range<usize>) -> Option<Range<u32>> {
    let first = index.paragraph_index(range.start)?;
    let count = index
        .native_paragraphs()
        .skip(usize::try_from(first).ok()?)
        .take_while(|paragraph| paragraph.physical.start <= range.end)
        .count();
    Some(first..first.checked_add(u32::try_from(count).ok()?)?)
}
