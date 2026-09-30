use std::ops::Range;

use crate::text_index::TextIndex;
use crate::types::{Document, Page, PageElement, RichTextBox, RichTextObjectContent};

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
    // Native list-mode export excludes its final compatibility record, even
    // when the body text is empty. Require a complete flow canvas and matching
    // background so incomplete/ambiguous documents retain their final page.
    let list_compatibility_page = has_list_compatibility_page(document);
    let legacy_text_compatibility = document.metadata.page_mode.is_none() && has_flowing_text;
    let omitted_trailing_blank_page = (list_compatibility_page || legacy_text_compatibility)
        && document.pages.len() > 1
        && document.pages.last().is_some_and(is_blank_storage_page);
    let visible_count = document
        .pages
        .len()
        .saturating_sub(usize::from(omitted_trailing_blank_page));

    let text_index = document
        .metadata
        .note_text
        .as_ref()
        .map(|text| TextIndex::new(&text.text));
    let text_ranges = document
        .metadata
        .note_text
        .as_ref()
        .zip(text_index.as_ref())
        .map_or_else(Vec::new, |(text, index)| {
            if text.text_sections.len() >= visible_count {
                text.text_sections
                    .iter()
                    .take(visible_count)
                    .map(|section| section_char_range(index, *section))
                    .collect()
            } else {
                balanced_line_ranges(&text.text, visible_count)
                    .into_iter()
                    .map(Some)
                    .collect()
            }
        });
    let page_heights = document
        .pages
        .iter()
        .take(visible_count)
        .map(|page| f64::from(page.height))
        .collect::<Vec<_>>();
    let pages = document
        .pages
        .iter()
        .take(visible_count)
        .cloned()
        .enumerate()
        .map(|(source_page_index, mut page)| {
            if let (Some(note_text), Some(index), Some(stored_range)) = (
                document.metadata.note_text.as_ref(),
                text_index.as_ref(),
                text_ranges.get(source_page_index).and_then(Option::as_ref),
            ) {
                let mut range = stored_range.clone();
                // The SDK's continuation sections overlap the preceding page by
                // its terminating newline. It is a page-break marker, not a
                // blank paragraph on the new page.
                if source_page_index > 0
                    && index.slice(range.start..range.start.saturating_add(1)) == Some("\n")
                {
                    range.start = range.start.saturating_add(1).min(range.end);
                }
                if let Some(mut slice) = note_text.slice_indexed(index, range.clone())
                    && !slice.text.is_empty()
                {
                    translate_continuing_objects(
                        &mut slice,
                        index,
                        &range,
                        source_page_index,
                        &text_ranges,
                        &page_heights,
                    );
                    page.objects.insert(0, PageElement::TextBox(slice).into());
                }
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

fn translate_continuing_objects(
    slice: &mut RichTextBox,
    source: &TextIndex<'_>,
    source_range: &Range<usize>,
    page_index: usize,
    page_ranges: &[Option<Range<usize>>],
    page_heights: &[f64],
) {
    let Some(section_start_utf16) = source.char_to_utf16(source_range.start) else {
        return;
    };
    for object_span in &mut slice.object_spans {
        let Ok(local_index) = u32::try_from(object_span.text_index_utf16) else {
            continue;
        };
        let Some(absolute_utf16) = section_start_utf16.checked_add(local_index) else {
            continue;
        };
        let Some(absolute_character) = source.utf16_to_char(absolute_utf16) else {
            continue;
        };
        let Some(anchor_page) = page_ranges.iter().position(|range| {
            range
                .as_ref()
                .is_some_and(|range| range.contains(&absolute_character))
        }) else {
            continue;
        };
        if anchor_page >= page_index {
            continue;
        }
        let delta_y = -page_heights[anchor_page..page_index].iter().sum::<f64>();
        translate_object_content_y(object_span.content.as_mut(), delta_y);
    }
}

fn translate_object_content_y(content: Option<&mut RichTextObjectContent>, delta_y: f64) {
    match content {
        Some(RichTextObjectContent::Image(image)) => {
            translate_bbox_y(&mut image.bbox, delta_y);
            if let Some(original) = &mut image.original_bbox {
                translate_bbox_y(original, delta_y);
            }
        }
        Some(RichTextObjectContent::Table(table)) => {
            translate_bbox_y(&mut table.bbox, delta_y);
            for row in &mut table.rows {
                for cell in &mut row.cells {
                    translate_bbox_y(&mut cell.bbox, delta_y);
                    translate_bbox_y(&mut cell.content.bbox, delta_y);
                }
            }
        }
        Some(RichTextObjectContent::CodeBlock(code)) => {
            translate_bbox_y(&mut code.bbox, delta_y);
            // Code title/body boxes are local to the object. Only the outer
            // document-flow box advances between pages.
        }
        None => {}
    }
}

fn translate_bbox_y(bbox: &mut crate::types::BoundingBox, delta_y: f64) {
    bbox.y_min += delta_y;
    bbox.y_max += delta_y;
}

fn section_char_range(
    index: &TextIndex<'_>,
    section: crate::types::RichTextSection,
) -> Option<Range<usize>> {
    let start_utf16 = u32::try_from(section.start_utf16).ok()?;
    let length_utf16 = u32::try_from(section.length_utf16).ok()?;
    let end_utf16 = start_utf16.checked_add(length_utf16)?;
    Some(index.utf16_to_char(start_utf16)?..index.utf16_to_char(end_utf16)?)
}

fn has_list_compatibility_page(document: &Document) -> bool {
    let metadata = &document.metadata;
    if metadata.page_mode != Some(0) || document.pages.len() < 2 {
        return false;
    }
    let Some((flow_width, flow_height)) = metadata.flow_dimensions else {
        return false;
    };
    let Some((_, padding)) = metadata.flow_page_padding else {
        return false;
    };
    let total_height = document
        .pages
        .iter()
        .map(|page| u64::from(page.height))
        .sum::<u64>()
        + (document.pages.len() as u64 - 1) * u64::from(padding);
    let last = &document.pages[document.pages.len() - 1];
    let previous = &document.pages[document.pages.len() - 2];
    total_height == u64::from(flow_height)
        && flow_width == last.width
        && last.width == previous.width
        && last.height == previous.height
        && last.background == previous.background
        && last.template == previous.template
        && last.background_color == previous.background_color
}

fn is_blank_storage_page(page: &Page) -> bool {
    page.strokes().next().is_none() && page.elements().next().is_none()
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
        self.slice_indexed(&TextIndex::new(&self.text), range)
    }

    fn slice_indexed(&self, index: &TextIndex<'_>, range: Range<usize>) -> Option<Self> {
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
            .filter_map(|span| {
                if span.start_utf16 >= span.end_utf16
                    || index.utf16_to_char(span.start_utf16).is_none()
                    || index.utf16_to_char(span.end_utf16).is_none()
                {
                    return None;
                }
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

        let mut slice = self.clone();
        slice.text = text.to_string();
        slice.runs = runs;
        slice.spans = spans;
        slice.paragraphs = paragraphs;
        slice.object_spans = object_spans;
        slice.text_sections = vec![crate::types::RichTextSection {
            start_utf16: 0,
            length_utf16: i32::try_from(end_utf16.checked_sub(start_utf16)?).ok()?,
        }];
        Some(slice)
    }
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

#[cfg(test)]
mod tests {
    use super::{layout_document, section_char_range};
    use crate::text_index::TextIndex;
    use crate::{
        BoundingBox, Document, DocumentMetadata, Page, PageElement, RichTextBox, RichTextParagraph,
        RichTextParagraphType, RichTextRun, RichTextSection, RichTextSpan, RichTextSpanType,
    };

    fn blank_page(index: usize) -> Page {
        Page {
            uuid: format!("page-{index}"),
            width: 1080,
            height: 1527,
            content_bbox: BoundingBox::default(),
            background_color: None,
            template: None,
            background: Default::default(),
            objects: Vec::new(),
        }
    }

    #[test]
    fn list_mode_omits_only_the_trailing_compatibility_record() {
        let template = crate::PageTemplate {
            id: 7,
            source: crate::PageTemplateSource::BuiltIn,
        };
        let mut document = Document {
            pages: (0..3).map(blank_page).collect(),
            metadata: DocumentMetadata {
                page_mode: Some(0),
                flow_dimensions: Some((1080, 3 * 1527 + 2 * 41)),
                flow_page_padding: Some((0, 41)),
                ..Default::default()
            },
        };
        for page in &mut document.pages {
            page.template = Some(template);
        }
        // Empty body text does not prevent list-mode compatibility handling;
        // the two intentionally blank/template-only pages remain visible.
        let layout = layout_document(&document);
        assert_eq!(layout.pages.len(), 2);
        assert_eq!(document.pages.len(), 3);
        assert!(layout.omitted_trailing_blank_page);
        assert_eq!(layout.pages[1].source_page_index, 1);
        assert_eq!(layout.pages[1].page.template, Some(template));
        for mode in [None, Some(1), Some(99)] {
            document.metadata.page_mode = mode;
            assert_eq!(layout_document(&document).pages.len(), 3);
        }
        document.metadata.page_mode = Some(0);
        document.pages[2].template = None;
        assert_eq!(layout_document(&document).pages.len(), 3);
        document.pages[2].template = Some(template);
        document.metadata.flow_dimensions = Some((1080, 1527));
        assert_eq!(layout_document(&document).pages.len(), 3);
        document.pages.truncate(1);
        assert_eq!(layout_document(&document).pages.len(), 1);
    }

    #[test]
    fn list_mode_keeps_a_final_page_with_decoded_content() {
        let mut document = Document {
            pages: (0..2).map(blank_page).collect(),
            metadata: DocumentMetadata {
                page_mode: Some(0),
                flow_dimensions: Some((1080, 3054)),
                flow_page_padding: Some((0, 0)),
                ..Default::default()
            },
        };
        document.pages[1].objects.push(
            PageElement::Image {
                bbox: BoundingBox::default(),
                media_index: 0,
            }
            .into(),
        );
        assert_eq!(layout_document(&document).pages.len(), 2);
    }

    #[test]
    fn separates_visible_flow_pages_from_trailing_storage_page() {
        let text = "one\ntwo 😀\nthree\nfour\nfive\n";
        let body = RichTextBox {
            text_area_type: None,
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
            object_spans: Vec::new(),
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
            .filter_map(|page| page.page.elements().last())
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
            text_area_type: None,
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
            object_spans: Vec::new(),
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
            .map(|page| match page.page.elements().next().unwrap() {
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

    #[test]
    fn continuation_sections_preserve_native_text_margins() {
        let text = "first\n\nsecond\n";
        let first_end = "first\n".encode_utf16().count() as i32;
        let overlapping_start = first_end - 1;
        let body = RichTextBox {
            text_area_type: None,
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
            object_spans: Vec::new(),
            text_sections: vec![
                RichTextSection {
                    start_utf16: 0,
                    length_utf16: first_end,
                },
                RichTextSection {
                    start_utf16: overlapping_start,
                    length_utf16: text.encode_utf16().count() as i32 - overlapping_start,
                },
            ],
            margins: Some([16.0, 10.0, 16.0, 10.0]),
            gravity: None,
        };
        let document = Document {
            pages: (0..3).map(blank_page).collect(),
            metadata: DocumentMetadata {
                note_text: Some(body),
                ..DocumentMetadata::default()
            },
        };

        let layout = layout_document(&document);
        let PageElement::TextBox(first) = layout.pages[0].page.elements().next().unwrap() else {
            panic!("first page text")
        };
        let PageElement::TextBox(second) = layout.pages[1].page.elements().next().unwrap() else {
            panic!("second page text")
        };

        assert_eq!(first.text, "first\n");
        assert_eq!(second.text, "\nsecond\n");
        assert_eq!(first.margins.unwrap()[1], 10.0);
        assert_eq!(second.margins, first.margins);
    }

    #[test]
    fn slices_paragraph_ordinals_and_text_sections() {
        let body = RichTextBox {
            text_area_type: None,
            bbox: BoundingBox::default(),
            rotation_degrees: None,
            text: "alpha\nbeta\ngamma".into(),
            color: None,
            highlight_color: None,
            underline: false,
            font_size: None,
            runs: Vec::new(),
            spans: Vec::new(),
            paragraphs: vec![RichTextParagraph {
                kind: RichTextParagraphType::Alignment,
                start_paragraph: 1,
                end_paragraph: 3,
                payload: 2_u32.to_le_bytes().to_vec(),
            }],
            object_spans: Vec::new(),
            text_sections: Vec::new(),
            margins: None,
            gravity: None,
        };

        let slice = body.slice_chars(6..11).unwrap();

        assert_eq!(slice.text, "beta\n");
        assert_eq!(slice.paragraphs[0].start_paragraph, 0);
        assert_eq!(slice.paragraphs[0].end_paragraph, 2);
        assert_eq!(slice.text_sections[0].start_utf16, 0);
        assert_eq!(slice.text_sections[0].length_utf16, 5);
    }
    fn crlf_body() -> RichTextBox {
        RichTextBox {
            text_area_type: None,
            bbox: BoundingBox::default(),
            rotation_degrees: None,
            text: "one\r\n😀two\r\nlast".into(),
            color: None,
            highlight_color: None,
            underline: false,
            font_size: None,
            runs: vec![RichTextRun {
                start: 5,
                end: 9,
                bold: true,
                italic: false,
            }],
            spans: vec![
                RichTextSpan {
                    kind: RichTextSpanType::Bold,
                    start_utf16: 5,
                    end_utf16: 10,
                    expand: false,
                    payload: 1_u16.to_le_bytes().to_vec(),
                },
                RichTextSpan {
                    kind: RichTextSpanType::Italic,
                    start_utf16: 9,
                    end_utf16: 13,
                    expand: false,
                    payload: 1_u16.to_le_bytes().to_vec(),
                },
            ],
            paragraphs: vec![
                RichTextParagraph {
                    kind: RichTextParagraphType::Alignment,
                    start_paragraph: 0,
                    end_paragraph: 1,
                    payload: 0_u32.to_le_bytes().to_vec(),
                },
                RichTextParagraph {
                    kind: RichTextParagraphType::Alignment,
                    start_paragraph: 1,
                    end_paragraph: 2,
                    payload: 1_u32.to_le_bytes().to_vec(),
                },
                RichTextParagraph {
                    kind: RichTextParagraphType::Alignment,
                    start_paragraph: 2,
                    end_paragraph: 5,
                    payload: 2_u32.to_le_bytes().to_vec(),
                },
            ],
            object_spans: Vec::new(),
            text_sections: Vec::new(),
            margins: None,
            gravity: None,
        }
    }

    #[test]
    fn slices_inside_crlf_paragraphs_rebase_unicode_styles_once() {
        let mut body = crlf_body();
        body.runs.push(RichTextRun {
            start: 0,
            end: 16,
            bold: false,
            italic: true,
        });
        body.spans.push(RichTextSpan {
            kind: RichTextSpanType::FontSize,
            start_utf16: 6,
            end_utf16: 14,
            expand: false,
            payload: 72_f32.to_le_bytes().to_vec(),
        });
        let emoji = body.slice_chars(5..9).unwrap();
        assert_eq!(emoji.text, "😀two");
        assert_eq!(body.runs.len(), 2);
        assert_eq!(body.runs[1].end, 16);
        assert_eq!(emoji.runs.len(), 1);
        assert!(emoji.runs[0].bold);
        assert!(!emoji.runs[0].italic);
        assert_eq!(emoji.paragraphs.len(), 1);
        assert_eq!(emoji.paragraphs[0].payload, 2_u32.to_le_bytes());
        assert_eq!(
            (
                emoji.paragraphs[0].start_paragraph,
                emoji.paragraphs[0].end_paragraph
            ),
            (0, 1)
        );
        let slice = body.slice_chars(7..11).unwrap();
        assert_eq!(body.spans.len(), 3);
        assert_eq!(slice.text, "wo\r\n");
        assert_eq!(slice.runs.len(), 1);
        assert_eq!((slice.runs[0].start, slice.runs[0].end), (0, 2));
        assert_eq!(
            slice
                .spans
                .iter()
                .map(|span| (span.start_utf16, span.end_utf16))
                .collect::<Vec<_>>(),
            [(0, 2), (1, 4)]
        );
        assert_eq!(slice.paragraphs.len(), 1);
        assert_eq!(
            (
                slice.paragraphs[0].start_paragraph,
                slice.paragraphs[0].end_paragraph
            ),
            (0, 3)
        );
        assert_eq!(slice.paragraphs[0].payload, 2_u32.to_le_bytes());
        assert_eq!(slice.text_sections[0].start_utf16, 0);
        assert_eq!(slice.text_sections[0].length_utf16, 4);
        let last = body.slice_chars(11..15).unwrap();
        assert_eq!(last.text, "last");
        assert_eq!(
            (
                last.paragraphs[0].start_paragraph,
                last.paragraphs[0].end_paragraph
            ),
            (0, 1)
        );
        assert_eq!(last.spans[0].kind, RichTextSpanType::Italic);
        assert_eq!((last.spans[0].start_utf16, last.spans[0].end_utf16), (0, 1));
    }

    #[test]
    fn slices_accept_full_and_empty_boundaries_and_reject_invalid_ranges() {
        let body = crlf_body();
        let full = body.slice_chars(0..15).unwrap();
        assert_eq!(full.text, "one\r\n😀two\r\nlast");
        assert_eq!(full.text_sections[0].length_utf16, 16);
        assert_eq!(
            (full.spans[0].start_utf16, full.spans[0].end_utf16),
            (5, 10)
        );
        assert_eq!(
            (
                full.paragraphs[2].start_paragraph,
                full.paragraphs[2].end_paragraph
            ),
            (2, 5)
        );
        for range in [0..0, 7..7, 15..15] {
            let empty = body.slice_chars(range).unwrap();
            assert_eq!(empty.text, "");
            assert!(empty.spans.is_empty());
            assert!(empty.runs.is_empty());
            assert_eq!(empty.paragraphs.len(), 1);
            assert_eq!(
                (
                    empty.paragraphs[0].start_paragraph,
                    empty.paragraphs[0].end_paragraph
                ),
                (0, 1)
            );
            assert_eq!(empty.text_sections[0].length_utf16, 0);
        }
        assert!(
            body.slice_chars(std::ops::Range { start: 8, end: 7 })
                .is_none()
        );
        assert!(body.slice_chars(0..16).is_none());
        assert!(body.slice_chars(16..16).is_none());
        let index = TextIndex::new(&body.text);
        for (section, expected) in [
            (
                RichTextSection {
                    start_utf16: 0,
                    length_utf16: 16,
                },
                Some(0..15),
            ),
            (
                RichTextSection {
                    start_utf16: 5,
                    length_utf16: 2,
                },
                Some(5..6),
            ),
            (
                RichTextSection {
                    start_utf16: 16,
                    length_utf16: 0,
                },
                Some(15..15),
            ),
            (
                RichTextSection {
                    start_utf16: 6,
                    length_utf16: 1,
                },
                None,
            ),
            (
                RichTextSection {
                    start_utf16: 5,
                    length_utf16: 1,
                },
                None,
            ),
            (
                RichTextSection {
                    start_utf16: 17,
                    length_utf16: 0,
                },
                None,
            ),
            (
                RichTextSection {
                    start_utf16: -1,
                    length_utf16: 1,
                },
                None,
            ),
            (
                RichTextSection {
                    start_utf16: 0,
                    length_utf16: -1,
                },
                None,
            ),
        ] {
            assert_eq!(section_char_range(&index, section), expected);
        }
    }
    #[test]
    fn empty_slices_keep_the_containing_native_paragraph_style() {
        let mut body = crlf_body();
        body.text = "ab\r\ncd\n".into();
        body.spans.clear();
        body.runs.clear();
        body.paragraphs = (0..4)
            .map(|ordinal| RichTextParagraph {
                kind: RichTextParagraphType::Alignment,
                start_paragraph: ordinal,
                end_paragraph: ordinal + 1,
                payload: ordinal.to_le_bytes().to_vec(),
            })
            .collect();
        for (offset, ordinal) in [(0, 0_u32), (1, 0), (3, 1), (5, 2), (7, 3)] {
            let slice = body.slice_chars(offset..offset).unwrap();
            assert_eq!(slice.text, "");
            assert_eq!(slice.paragraphs.len(), 1);
            assert_eq!(slice.paragraphs[0].payload, ordinal.to_le_bytes());
            assert_eq!(
                (
                    slice.paragraphs[0].start_paragraph,
                    slice.paragraphs[0].end_paragraph
                ),
                (0, 1)
            );
        }
    }
}
