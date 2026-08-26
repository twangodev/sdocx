use std::ops::Range;

use crate::types::{Document, Page, PageElement, RichTextBox};

/// A presentation-oriented view of a parsed document.
///
/// Physical `.page` records remain available on [`Document`]. This view omits
/// Samsung's trailing blank compatibility page and places document-level text
/// flow onto the remaining visible pages.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LayoutDocument {
    /// Pages intended for display or export.
    pub pages: Vec<LayoutPage>,
    /// Number of physical page records in the source document.
    pub stored_page_count: usize,
    /// Whether a trailing blank physical page was omitted from this view.
    pub omitted_trailing_blank_page: bool,
}

/// One visible page and the physical page record that backs it.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LayoutPage {
    /// Index into `Document::pages`.
    pub source_page_index: usize,
    /// Composite page with page-local content and its slice of flowing text.
    pub page: Page,
}

/// Build a visible-page view without changing the parsed storage model.
pub fn layout_document(document: &Document) -> LayoutDocument {
    let has_flowing_text = document
        .metadata
        .note_text
        .as_ref()
        .is_some_and(|text| !text.text.trim().is_empty());
    let omitted_trailing_blank_page = has_flowing_text
        && document.pages.len() > 1
        && document.pages.last().is_some_and(is_blank_storage_page);
    let visible_count = document
        .pages
        .len()
        .saturating_sub(usize::from(omitted_trailing_blank_page));

    let text_ranges = document
        .metadata
        .note_text
        .as_ref()
        .map_or_else(Vec::new, |text| {
            if text.text_sections.len() >= visible_count {
                text.text_sections
                    .iter()
                    .take(visible_count)
                    .map(|section| section_char_range(&text.text, *section))
                    .collect()
            } else {
                balanced_line_ranges(&text.text, visible_count)
                    .into_iter()
                    .map(Some)
                    .collect()
            }
        });
    let pages = document
        .pages
        .iter()
        .take(visible_count)
        .cloned()
        .enumerate()
        .map(|(source_page_index, mut page)| {
            if let (Some(note_text), Some(range)) = (
                document.metadata.note_text.as_ref(),
                text_ranges.get(source_page_index).and_then(Option::as_ref),
            ) && let Some(slice) = note_text.slice_chars(range.clone())
                && !slice.text.is_empty()
            {
                page.elements.push(PageElement::TextBox(slice));
            }
            LayoutPage {
                source_page_index,
                page,
            }
        })
        .collect();

    LayoutDocument {
        pages,
        stored_page_count: document.pages.len(),
        omitted_trailing_blank_page,
    }
}

fn section_char_range(text: &str, section: crate::types::RichTextSection) -> Option<Range<usize>> {
    let start_utf16 = u32::try_from(section.start_utf16).ok()?;
    let length_utf16 = u32::try_from(section.length_utf16).ok()?;
    let end_utf16 = start_utf16.checked_add(length_utf16)?;
    Some(utf16_to_char_index(text, start_utf16)?..utf16_to_char_index(text, end_utf16)?)
}

fn is_blank_storage_page(page: &Page) -> bool {
    page.strokes.is_empty() && page.elements.is_empty()
}

fn balanced_line_ranges(text: &str, page_count: usize) -> Vec<Range<usize>> {
    if page_count == 0 {
        return Vec::new();
    }

    let character_count = text.chars().count();
    let mut line_ends = text
        .char_indices()
        .scan(0_usize, |character_index, (_, character)| {
            *character_index += 1;
            Some((character == '\n').then_some(*character_index))
        })
        .flatten()
        .collect::<Vec<_>>();
    if line_ends.last().copied() != Some(character_count) {
        line_ends.push(character_count);
    }

    let mut ranges = Vec::with_capacity(page_count);
    let mut start = 0_usize;
    for page_index in 0..page_count {
        let end = if page_index + 1 == page_count {
            character_count
        } else {
            let target = character_count.saturating_mul(page_index + 1) / page_count;
            let remaining_pages = page_count - page_index - 1;
            let max_line_index = line_ends.len().saturating_sub(remaining_pages + 1);
            let candidate = line_ends.partition_point(|line_end| *line_end < target);
            line_ends[candidate.min(max_line_index)]
        };
        ranges.push(start..end);
        start = end;
    }
    ranges
}

impl RichTextBox {
    /// Return a character-indexed slice with intersecting style records rebased.
    pub fn slice_chars(&self, range: Range<usize>) -> Option<Self> {
        if range.start > range.end {
            return None;
        }
        let byte_offsets = char_byte_offsets(&self.text);
        let byte_start = *byte_offsets.get(range.start)?;
        let byte_end = *byte_offsets.get(range.end)?;
        let start_utf16 = char_to_utf16_index(&self.text, range.start)?;
        let end_utf16 = char_to_utf16_index(&self.text, range.end)?;

        let runs = self
            .runs
            .iter()
            .filter_map(|run| {
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
            .filter_map(|span| {
                let start = span.start_utf16.max(start_utf16);
                let end = span.end_utf16.min(end_utf16);
                (start < end).then(|| {
                    let mut span = span.clone();
                    span.start_utf16 = start - start_utf16;
                    span.end_utf16 = end - start_utf16;
                    span
                })
            })
            .collect();
        let paragraphs = self
            .paragraphs
            .iter()
            .filter_map(|paragraph| {
                let start = paragraph.start_utf16.max(start_utf16);
                let end = paragraph.end_utf16.min(end_utf16);
                (start < end).then(|| {
                    let mut paragraph = paragraph.clone();
                    paragraph.start_utf16 = start - start_utf16;
                    paragraph.end_utf16 = end - start_utf16;
                    paragraph
                })
            })
            .collect();

        let mut slice = self.clone();
        slice.text = self.text[byte_start..byte_end].to_string();
        slice.runs = runs;
        slice.spans = spans;
        slice.paragraphs = paragraphs;
        Some(slice)
    }
}

fn char_byte_offsets(text: &str) -> Vec<usize> {
    let mut offsets = text
        .char_indices()
        .map(|(offset, _)| offset)
        .collect::<Vec<_>>();
    offsets.push(text.len());
    offsets
}

fn char_to_utf16_index(text: &str, character_index: usize) -> Option<u32> {
    let units = text
        .chars()
        .take(character_index)
        .try_fold(0_usize, |total, character| {
            total.checked_add(character.len_utf16())
        })?;
    u32::try_from(units).ok()
}

fn utf16_to_char_index(text: &str, target: u32) -> Option<usize> {
    let target = usize::try_from(target).ok()?;
    let mut utf16_offset = 0_usize;
    for (char_index, character) in text.chars().enumerate() {
        if utf16_offset == target {
            return Some(char_index);
        }
        utf16_offset = utf16_offset.checked_add(character.len_utf16())?;
        if utf16_offset > target {
            return None;
        }
    }
    (utf16_offset == target).then_some(text.chars().count())
}

#[cfg(test)]
mod tests {
    use super::layout_document;
    use crate::{
        BoundingBox, Document, DocumentMetadata, Page, PageElement, RichTextBox, RichTextRun,
        RichTextSection, RichTextSpan, RichTextSpanType,
    };

    fn blank_page(index: usize) -> Page {
        Page {
            uuid: format!("page-{index}"),
            width: 1080,
            height: 1527,
            content_bbox: BoundingBox::default(),
            background_color: None,
            template: None,
            strokes: Vec::new(),
            elements: Vec::new(),
        }
    }

    #[test]
    fn separates_visible_flow_pages_from_trailing_storage_page() {
        let text = "one\ntwo 😀\nthree\nfour\nfive\n";
        let body = RichTextBox {
            bbox: BoundingBox::default(),
            rotation_degrees: None,
            text: text.into(),
            color: None,
            highlight_color: None,
            underline: false,
            font_size: None,
            runs: vec![RichTextRun {
                start: 4,
                end: 9,
                bold: true,
                italic: false,
            }],
            spans: vec![RichTextSpan {
                kind: RichTextSpanType::Bold,
                start_utf16: 4,
                end_utf16: 10,
                expand: false,
                payload: 1_u16.to_le_bytes().to_vec(),
            }],
            paragraphs: Vec::new(),
            text_sections: Vec::new(),
            margins: None,
            gravity: None,
        };
        let document = Document {
            pages: (0..6).map(blank_page).collect(),
            metadata: DocumentMetadata {
                note_text: Some(body),
                ..DocumentMetadata::default()
            },
        };

        let layout = layout_document(&document);

        assert_eq!(layout.stored_page_count, 6);
        assert!(layout.omitted_trailing_blank_page);
        assert_eq!(layout.pages.len(), 5);
        let reconstructed = layout
            .pages
            .iter()
            .filter_map(|page| page.page.elements.last())
            .map(|element| match element {
                PageElement::TextBox(text) => text.text.as_str(),
                _ => "",
            })
            .collect::<String>();
        assert_eq!(reconstructed, text);
    }

    #[test]
    fn uses_stored_text_sections_instead_of_balancing_content() {
        let text = "short\nthis page is intentionally much longer\nlast\n";
        let first_end = "short\n".encode_utf16().count() as i32;
        let second_end = "short\nthis page is intentionally much longer\n"
            .encode_utf16()
            .count() as i32;
        let mut body = RichTextBox {
            bbox: BoundingBox::default(),
            rotation_degrees: None,
            text: text.into(),
            color: None,
            highlight_color: None,
            underline: false,
            font_size: None,
            runs: Vec::new(),
            spans: Vec::new(),
            paragraphs: Vec::new(),
            text_sections: vec![
                RichTextSection {
                    start_utf16: 0,
                    length_utf16: first_end,
                },
                RichTextSection {
                    start_utf16: first_end,
                    length_utf16: second_end - first_end,
                },
                RichTextSection {
                    start_utf16: second_end,
                    length_utf16: text.encode_utf16().count() as i32 - second_end,
                },
            ],
            margins: None,
            gravity: None,
        };
        body.spans.push(RichTextSpan {
            kind: RichTextSpanType::Bold,
            start_utf16: first_end as u32,
            end_utf16: second_end as u32,
            expand: false,
            payload: 1_u16.to_le_bytes().to_vec(),
        });
        let document = Document {
            pages: (0..4).map(blank_page).collect(),
            metadata: DocumentMetadata {
                note_text: Some(body),
                ..DocumentMetadata::default()
            },
        };

        let layout = layout_document(&document);
        let page_text = layout
            .pages
            .iter()
            .map(|page| match &page.page.elements[0] {
                PageElement::TextBox(text) => text.text.as_str(),
                _ => unreachable!(),
            })
            .collect::<Vec<_>>();

        assert_eq!(
            page_text,
            vec![
                "short\n",
                "this page is intentionally much longer\n",
                "last\n"
            ]
        );
    }
}
