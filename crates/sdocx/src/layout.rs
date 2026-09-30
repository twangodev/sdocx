use std::ops::Range;
use std::sync::Arc;

use crate::PageObjectContent;
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
    /// The inspection slice inserted from document-level body text.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub body_text: Option<BodyTextSlice>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BodyTextSlice {
    /// Character range in `DocumentMetadata::note_text`.
    pub source_range: Range<usize>,
    /// Position of the synthetic TextBox in `LayoutPage::page.objects`.
    pub object_index: usize,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub capture_window: Option<BodyTextCaptureWindow>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub reflow: Option<BodyTextReflow>,
}

/// Full-source measurement context when usable saved page sections are absent.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BodyTextReflow {
    pub source_range: Range<usize>,
    pub first_page_index: usize,
    pub requested_page_index: usize,
    #[cfg_attr(feature = "serde", serde(skip))]
    source_snapshot: Option<Arc<RichTextBox>>,
}

/// Source context measured together for a native page capture.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BodyTextCaptureWindow {
    /// Saved section extent before native partial-text copying.
    pub saved_source_range: Range<usize>,
    /// Character range measured after native leading-LF removal.
    pub source_range: Range<usize>,
    /// Native output mapping offset, which may skip a leading LF after measurement.
    pub reported_source_start: usize,
    /// First physical page included in the measurement.
    pub first_page_index: usize,
    /// Physical page requested for painting.
    pub requested_page_index: usize,
}

fn native_capture_window(
    document: &Document,
    index: &TextIndex<'_>,
    requested_page_index: usize,
) -> Option<BodyTextCaptureWindow> {
    if document.metadata.page_mode != Some(0) {
        return None;
    }
    let body = document.metadata.note_text.as_ref()?;
    let ranges = body
        .text_sections
        .get(..=requested_page_index)?
        .iter()
        .map(|section| section_char_range(index, *section))
        .collect::<Option<Vec<_>>>()?;
    if ranges.windows(2).any(|pair| pair[0].start > pair[1].start) {
        return None;
    }
    let mut first_page_index = requested_page_index;
    let requested_start = ranges[requested_page_index].start;
    if requested_page_index > 0
        && requested_start > 0
        && index.slice(requested_start..requested_start.checked_add(1)?)? != "\n"
    {
        let paragraph_index = index.paragraph_index(requested_start)?;
        if body.paragraphs.iter().any(|paragraph| {
            paragraph.kind == crate::RichTextParagraphType::Bullet
                && paragraph.start_paragraph <= paragraph_index
                && paragraph.end_paragraph > paragraph_index
        }) {
            let paragraph_start = index
                .native_paragraphs()
                .nth(usize::try_from(paragraph_index).ok()?)?
                .physical
                .start;
            while first_page_index > 0 && ranges[first_page_index].start > paragraph_start {
                first_page_index -= 1;
            }
        }
    }
    while first_page_index > 0 && ranges[first_page_index - 1].end > ranges[first_page_index].start
    {
        first_page_index -= 1;
    }
    let first_nonempty =
        (first_page_index..=requested_page_index).find(|&page| !ranges[page].is_empty())?;
    let last_nonempty = (first_nonempty..=requested_page_index)
        .rev()
        .find(|&page| !ranges[page].is_empty())?;
    let saved_source_range = ranges[first_nonempty].start..ranges[last_nonempty].end;
    index.slice(saved_source_range.clone())?;
    let mut source_range = saved_source_range.clone();
    let keep_initial_lf =
        source_range.start == 0 && ranges[..first_nonempty].iter().all(Range::is_empty);
    if !keep_initial_lf
        && index.slice(source_range.start..source_range.start.checked_add(1)?) == Some("\n")
    {
        source_range.start += 1;
    }
    let mut reported_source_start = if requested_page_index == 0 {
        0
    } else {
        ranges[first_page_index].start
    };
    let reported_utf16 = index.char_to_utf16(reported_source_start)?;
    let full_utf16 = index.char_to_utf16(index.len())?;
    if reported_utf16 >= 1
        && reported_utf16.checked_add(1)? < full_utf16
        && index.slice(reported_source_start..reported_source_start.checked_add(1)?) == Some("\n")
    {
        reported_source_start += 1;
    }
    Some(BodyTextCaptureWindow {
        saved_source_range,
        source_range,
        reported_source_start,
        first_page_index,
        requested_page_index,
    })
}

impl LayoutPage {
    /// Body metadata is active only while its inspection TextBox remains present.
    /// Callers editing object positions must update `body_text.object_index`.
    pub fn body_text_slice(&self) -> Option<&BodyTextSlice> {
        let slice = self.body_text.as_ref()?;
        matches!(
            self.page.objects.get(slice.object_index)?.content,
            PageObjectContent::Element(PageElement::TextBox(_))
        )
        .then_some(slice)
    }

    /// Rebuild validated capture text from the authoritative document body.
    pub fn body_text_capture(&self, document: &Document) -> Option<RichTextBox> {
        let metadata = self.body_text_slice()?;
        if metadata.reflow.is_some() {
            return None;
        }
        let window = metadata.capture_window.as_ref()?;
        if window.requested_page_index != self.source_page_index
            || window.first_page_index > window.requested_page_index
            || window.requested_page_index >= document.pages.len()
        {
            return None;
        }
        let body = document.metadata.note_text.as_ref()?;
        let index = TextIndex::new(&body.text);
        if native_capture_window(document, &index, self.source_page_index).as_ref() != Some(window)
        {
            return None;
        }
        let ranges = self.validated_inspection_ranges(document, &index)?;
        let page_heights = document
            .pages
            .iter()
            .map(|page| f64::from(page.height))
            .collect::<Vec<_>>();
        let mut capture = body.slice_indexed(&index, window.source_range.clone())?;
        translate_continuing_objects(
            &mut capture,
            &index,
            &window.source_range,
            window.first_page_index,
            &ranges,
            &page_heights,
        );
        Some(capture)
    }

    /// Retrieve the original body for measured pagination, never its inspection slice.
    pub fn body_text_reflow(&self, document: &Document) -> Option<RichTextBox> {
        let metadata = self.body_text_slice()?;
        let reflow = metadata.reflow.as_ref()?;
        let body = document.metadata.note_text.as_ref()?;
        let index = TextIndex::new(&body.text);
        let visible_count = document.pages.len() - usize::from(omits_trailing_blank_page(document));
        if metadata.capture_window.is_some()
            || reflow.first_page_index != 0
            || reflow.requested_page_index != self.source_page_index
            || self.source_page_index >= visible_count
            || reflow.source_range != (0..index.len())
            || reflow.source_snapshot.as_deref() != Some(body)
            || saved_inspection_ranges(body, &index, visible_count).is_some()
        {
            return None;
        }
        self.validated_inspection_ranges(document, &index)?;
        Some(body.clone())
    }

    fn validated_inspection_ranges(
        &self,
        document: &Document,
        index: &TextIndex<'_>,
    ) -> Option<Vec<Option<Range<usize>>>> {
        let metadata = self.body_text_slice()?;
        let object = self.page.objects.get(metadata.object_index)?;
        let PageObjectContent::Element(PageElement::TextBox(inspection)) = &object.content else {
            return None;
        };
        if object.render_layer != crate::ObjectRenderLayer::Base
            || object.source_offset.is_some()
            || !(inspection.bbox.x_max <= inspection.bbox.x_min
                || inspection.bbox.y_max <= inspection.bbox.y_min)
        {
            return None;
        }
        let body = document.metadata.note_text.as_ref()?;
        let visible_count = document.pages.len() - usize::from(omits_trailing_blank_page(document));
        let ranges = inspection_text_ranges(body, index, visible_count);
        let mut expected_range = ranges.get(self.source_page_index)?.clone()?;
        trim_inspection_lf(index, &mut expected_range, self.source_page_index);
        if expected_range != metadata.source_range {
            return None;
        }
        let page_heights = document
            .pages
            .iter()
            .map(|page| f64::from(page.height))
            .collect::<Vec<_>>();
        let mut expected = body.slice_indexed(index, expected_range.clone())?;
        translate_continuing_objects(
            &mut expected,
            index,
            &expected_range,
            self.source_page_index,
            &ranges,
            &page_heights,
        );
        if &expected != inspection {
            return None;
        }
        Some(ranges)
    }
}

/// Build a visible-page view without changing the parsed storage model.
pub fn layout_document(document: &Document) -> LayoutDocument {
    let omitted_trailing_blank_page = omits_trailing_blank_page(document);
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
            inspection_text_ranges(text, index, visible_count)
        });
    let page_heights = document
        .pages
        .iter()
        .take(visible_count)
        .map(|page| f64::from(page.height))
        .collect::<Vec<_>>();
    let reflow_source = document
        .metadata
        .note_text
        .as_ref()
        .zip(text_index.as_ref())
        .filter(|(body, index)| saved_inspection_ranges(body, index, visible_count).is_none())
        .map(|(body, _)| Arc::new(body.clone()));
    let pages = document
        .pages
        .iter()
        .take(visible_count)
        .cloned()
        .enumerate()
        .map(|(source_page_index, mut page)| {
            let mut body_text = None;
            if let (Some(note_text), Some(index), Some(stored_range)) = (
                document.metadata.note_text.as_ref(),
                text_index.as_ref(),
                text_ranges.get(source_page_index).and_then(Option::as_ref),
            ) {
                let mut range = stored_range.clone();
                trim_inspection_lf(index, &mut range, source_page_index);
                if let Some(mut slice) = note_text.slice_indexed(index, range.clone())
                    && (!slice.text.is_empty() || reflow_source.is_some())
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
                    body_text = Some(BodyTextSlice {
                        source_range: range,
                        object_index: 0,
                        capture_window: reflow_source
                            .is_none()
                            .then(|| native_capture_window(document, index, source_page_index))
                            .flatten(),
                        reflow: reflow_source.as_ref().map(|source| BodyTextReflow {
                            source_range: 0..index.len(),
                            first_page_index: 0,
                            requested_page_index: source_page_index,
                            source_snapshot: Some(Arc::clone(source)),
                        }),
                    });
                }
            }
            LayoutPage {
                source_page_index,
                page,
                body_text,
            }
        })
        .collect();

    LayoutDocument {
        pages,
        stored_page_count: document.pages.len(),
        omitted_trailing_blank_page,
    }
}

fn omits_trailing_blank_page(document: &Document) -> bool {
    // Native list-mode export excludes its final compatibility record, even
    // when the body text is empty. Require a complete flow canvas and matching
    // background so incomplete/ambiguous documents retain their final page.
    has_list_compatibility_page(document)
        && document.pages.len() > 1
        && document.pages.last().is_some_and(is_blank_storage_page)
}

fn inspection_text_ranges(
    body: &RichTextBox,
    index: &TextIndex<'_>,
    page_count: usize,
) -> Vec<Option<Range<usize>>> {
    saved_inspection_ranges(body, index, page_count).unwrap_or_else(|| {
        balanced_line_ranges(&body.text, page_count)
            .into_iter()
            .map(Some)
            .collect()
    })
}

fn saved_inspection_ranges(
    body: &RichTextBox,
    index: &TextIndex<'_>,
    page_count: usize,
) -> Option<Vec<Option<Range<usize>>>> {
    let ranges = body
        .text_sections
        .get(..page_count)?
        .iter()
        .map(|section| section_char_range(index, *section))
        .collect::<Option<Vec<_>>>()?;
    if (!body.text.is_empty() && ranges.iter().all(Range::is_empty))
        || ranges.windows(2).any(|pair| pair[0].start > pair[1].start)
    {
        return None;
    }
    Some(ranges.into_iter().map(Some).collect())
}

fn trim_inspection_lf(index: &TextIndex<'_>, range: &mut Range<usize>, page: usize) {
    if page > 0 && index.slice(range.start..range.start.saturating_add(1)) == Some("\n") {
        range.start = range.start.saturating_add(1).min(range.end);
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
    use super::{layout_document, native_capture_window, section_char_range};
    use crate::text_index::TextIndex;
    use crate::{
        BoundingBox, Document, DocumentMetadata, Page, PageElement, PageObjectContent, RichTextBox,
        RichTextObjectContent, RichTextParagraph, RichTextParagraphType, RichTextRun,
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
            background: Default::default(),
            objects: Vec::new(),
        }
    }

    fn capture_document(text: &str, sections: &[(i32, i32)]) -> Document {
        Document {
            pages: (0..sections.len().max(1)).map(blank_page).collect(),
            metadata: DocumentMetadata {
                page_mode: Some(0),
                note_text: Some(RichTextBox {
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
                    text_sections: sections
                        .iter()
                        .map(|&(start_utf16, length_utf16)| RichTextSection {
                            start_utf16,
                            length_utf16,
                        })
                        .collect(),
                    margins: None,
                    gravity: None,
                }),
                ..Default::default()
            },
        }
    }

    fn capture(document: &Document, page: usize) -> Option<super::BodyTextCaptureWindow> {
        let index = TextIndex::new(&document.metadata.note_text.as_ref()?.text);
        native_capture_window(document, &index, page)
    }

    #[test]
    fn capture_groups_strict_overlaps_without_joining_equal_boundaries() {
        let overlap = capture_document("abcdefghij", &[(0, 5), (4, 4), (7, 3)]);
        let window = capture(&overlap, 2).unwrap();
        assert_eq!(window.first_page_index, 0);
        assert_eq!(window.requested_page_index, 2);
        assert_eq!(window.source_range, 0..10);

        let touching = capture_document("abcdefghij", &[(0, 4), (4, 3), (7, 3)]);
        let window = capture(&touching, 2).unwrap();
        assert_eq!(window.first_page_index, 2);
        assert_eq!(window.source_range, 7..10);
    }

    #[test]
    fn capture_uses_requested_end_not_the_largest_overlapping_end() {
        let document = capture_document("abcdefghij", &[(0, 10), (4, 2)]);
        let window = capture(&document, 1).unwrap();
        assert_eq!(window.first_page_index, 0);
        assert_eq!(window.source_range, 0..6);
        let unsorted = capture_document("abcdefghij", &[(4, 4), (2, 4)]);
        assert!(capture(&unsorted, 1).is_none());
    }

    #[test]
    fn capture_maps_utf16_boundaries_and_rejects_surrogate_interiors() {
        let document = capture_document("A😀BC\nD", &[(0, 4), (3, 4)]);
        let window = capture(&document, 1).unwrap();
        assert_eq!(window.source_range, 0..6);
        let layout = layout_document(&document);
        assert_eq!(
            layout.pages[1].body_text_slice().unwrap().source_range,
            2..6
        );

        for sections in [[(0, 4), (2, 5)], [(0, 2), (3, 4)], [(0, 4), (3, 5)]] {
            assert!(capture(&capture_document("A😀BC\nD", &sections), 1).is_none());
        }
    }

    #[test]
    fn capture_keeps_measurement_lf_removal_separate_from_output_mapping() {
        let document = capture_document("a\nbc", &[(0, 1), (1, 3)]);
        let window = capture(&document, 1).unwrap();
        assert_eq!(window.saved_source_range, 1..4);
        assert_eq!(window.source_range, 2..4);
        assert_eq!(window.reported_source_start, 2);

        let final_lf = capture_document("a\n", &[(0, 1), (1, 1)]);
        let window = capture(&final_lf, 1).unwrap();
        assert_eq!(window.saved_source_range, 1..2);
        assert_eq!(window.source_range, 2..2);
        assert_eq!(window.reported_source_start, 1);

        let grouped = capture_document("a\nb\nc", &[(0, 3), (2, 3)]);
        assert_eq!(capture(&grouped, 1).unwrap().source_range, 0..5);

        let leading_lf = capture_document("\naB", &[(0, 0), (0, 3)]);
        let window = capture(&leading_lf, 1).unwrap();
        assert_eq!(window.source_range, 0..3);
        assert_eq!(window.reported_source_start, 0);
        let previous_nonempty = capture_document("\naB", &[(0, 1), (0, 0), (0, 3)]);
        let window = capture(&previous_nonempty, 2).unwrap();
        assert_eq!(window.source_range, 1..3);
        assert_eq!(window.saved_source_range, 0..3);
    }

    #[test]
    fn capture_backs_up_bullet_paragraphs_without_requiring_a_bullet_payload() {
        let mut document = capture_document("abcde\nfghij", &[(0, 2), (2, 3), (5, 6)]);
        assert_eq!(capture(&document, 1).unwrap().first_page_index, 1);
        document
            .metadata
            .note_text
            .as_mut()
            .unwrap()
            .paragraphs
            .push(RichTextParagraph {
                kind: RichTextParagraphType::Bullet,
                start_paragraph: 0,
                end_paragraph: 1,
                payload: Vec::new(),
            });
        let window = capture(&document, 1).unwrap();
        assert_eq!(window.first_page_index, 0);
        assert_eq!(window.source_range, 0..5);
        assert_eq!(capture(&document, 2).unwrap().first_page_index, 2);
    }

    #[test]
    fn body_metadata_requires_its_inspection_object_and_keeps_native_objects() {
        let mut document = capture_document("body", &[(0, 4)]);
        document.pages[0].objects.push(
            PageElement::Image {
                bbox: BoundingBox::default(),
                media_index: 9,
            }
            .into(),
        );
        let mut layout = layout_document(&document);
        let page = &mut layout.pages[0];
        assert_eq!(page.body_text_slice().unwrap().object_index, 0);
        assert!(matches!(
            page.page.objects[1].content,
            crate::PageObjectContent::Element(PageElement::Image { media_index: 9, .. })
        ));
        page.page.clear_strokes();
        assert!(page.body_text_slice().is_some());
        page.page.objects.clear();
        assert!(page.body_text_slice().is_none());
    }

    #[test]
    fn body_capture_rejects_replaced_reordered_or_cleared_inspection_content() {
        let mut document = capture_document("body", &[(0, 4)]);
        let mut native = document.metadata.note_text.as_ref().unwrap().clone();
        native.text = "native".into();
        document.pages[0]
            .objects
            .push(PageElement::TextBox(native).into());
        let page = layout_document(&document).pages.remove(0);
        assert!(page.body_text_capture(&document).is_some());

        let mut reordered = page.clone();
        reordered.page.objects.swap(0, 1);
        assert!(reordered.body_text_capture(&document).is_none());
        let mut cleared = page.clone();
        cleared.page.objects.clear();
        assert!(cleared.body_text_capture(&document).is_none());
        let mut changed = page.clone();
        let PageObjectContent::Element(PageElement::TextBox(body)) =
            &mut changed.page.objects[0].content
        else {
            unreachable!()
        };
        body.font_size = Some(37.0);
        assert!(changed.body_text_capture(&document).is_none());
        let mut stored = page.clone();
        stored.page.objects[0].source_offset = Some(42);
        assert!(stored.body_text_capture(&document).is_none());
        let mut top = page.clone();
        top.page.objects[0].render_layer = crate::ObjectRenderLayer::Top;
        assert!(top.body_text_capture(&document).is_none());
    }

    #[test]
    fn body_capture_rejects_source_and_window_changes() {
        let mut document = capture_document("a😀bc", &[(0, 5)]);
        let page = layout_document(&document).pages.remove(0);
        let mut bad_range = page.clone();
        bad_range.body_text.as_mut().unwrap().source_range = 0..usize::MAX;
        assert!(bad_range.body_text_capture(&document).is_none());
        let mut bad_window = page.clone();
        bad_window
            .body_text
            .as_mut()
            .unwrap()
            .capture_window
            .as_mut()
            .unwrap()
            .first_page_index = 1;
        assert!(bad_window.body_text_capture(&document).is_none());
        let mut wrong_page = page.clone();
        wrong_page.source_page_index = 1;
        assert!(wrong_page.body_text_capture(&document).is_none());
        document.metadata.note_text.as_mut().unwrap().font_size = Some(23.0);
        assert!(page.body_text_capture(&document).is_none());
        document.metadata.note_text.as_mut().unwrap().font_size = None;
        document.metadata.note_text.as_mut().unwrap().text = "z😀bc".into();
        assert!(page.body_text_capture(&document).is_none());
    }

    fn add_capture_object(document: &mut Document) {
        document
            .metadata
            .note_text
            .as_mut()
            .unwrap()
            .object_spans
            .push(crate::RichTextObjectSpan {
                object_type: crate::ObjectType::CodeBlock,
                object_data: Vec::new(),
                content: Some(RichTextObjectContent::CodeBlock(Box::new(
                    crate::RichTextCodeBlock {
                        bbox: BoundingBox {
                            x_min: 10.0,
                            y_min: 205.0,
                            x_max: 100.0,
                            y_max: 235.0,
                        },
                        rotation_degrees: None,
                        title: None,
                        body: None,
                    },
                ))),
                text_index_utf16: 2,
                layout_option: crate::ObjectSpanLayoutOption::Block,
                layout_constraint: crate::ObjectSpanLayoutConstraint::OverPages,
            });
    }

    fn code_top(text: &RichTextBox) -> f64 {
        let Some(RichTextObjectContent::CodeBlock(code)) = text.object_spans[0].content.as_ref()
        else {
            unreachable!()
        };
        code.bbox.y_min
    }

    #[test]
    fn capture_rebuilds_group_objects_from_source_and_preserves_debugger_clones() {
        let mut document = capture_document("ab\u{fffc}cd", &[(0, 3), (2, 3)]);
        add_capture_object(&mut document);
        document.pages[1].objects.push(
            crate::Stroke {
                rendering: None,
                bbox: BoundingBox::default(),
                points: Vec::new(),
                pressures: Vec::new(),
                timestamps: Vec::new(),
                tilts: Vec::new(),
                orientations: Vec::new(),
                color: None,
                pen_width: 1.0,
            }
            .into(),
        );
        let layout = layout_document(&document);
        let mut preview = layout.pages[1].clone();
        let PageObjectContent::Element(PageElement::TextBox(inspection)) =
            &preview.page.objects[0].content
        else {
            unreachable!()
        };
        assert_eq!(code_top(inspection), 205.0 - 1527.0);
        assert_eq!(preview.page.strokes().count(), 1);
        preview.page.clear_strokes();
        assert_eq!(preview.page.strokes().count(), 0);
        let capture = preview.body_text_capture(&document).unwrap();
        assert_eq!(capture.text, "ab\u{fffc}cd");
        assert_eq!(capture.object_spans[0].text_index_utf16, 2);
        assert_eq!(code_top(&capture), 205.0);
        let PageObjectContent::Element(PageElement::TextBox(inspection)) =
            &mut preview.page.objects[0].content
        else {
            unreachable!()
        };
        let Some(RichTextObjectContent::CodeBlock(code)) =
            inspection.object_spans[0].content.as_mut()
        else {
            unreachable!()
        };
        code.bbox.y_min += 1.0;
        assert!(preview.body_text_capture(&document).is_none());
    }

    #[test]
    fn capture_translates_only_objects_anchored_before_its_first_page_once() {
        let mut document = capture_document("ab\u{fffc}cd", &[(0, 5), (2, 0), (2, 3)]);
        add_capture_object(&mut document);
        let page = layout_document(&document).pages.remove(2);
        let capture = page.body_text_capture(&document).unwrap();
        assert_eq!(capture.object_spans[0].text_index_utf16, 0);
        assert_eq!(code_top(&capture), 205.0 - 3054.0);
        assert_eq!(
            code_top(document.metadata.note_text.as_ref().unwrap()),
            205.0
        );
        assert_eq!(
            code_top(&page.body_text_capture(&document).unwrap()),
            205.0 - 3054.0
        );
    }

    #[test]
    fn capture_does_not_promote_balanced_inspection_ranges_to_native_sections() {
        let document = capture_document("body", &[]);
        let layout = layout_document(&document);
        let body = layout.pages[0].body_text_slice().unwrap();
        assert_eq!(body.source_range, 0..4);
        assert!(body.capture_window.is_none());
        for mode in [None, Some(1), Some(99)] {
            let mut document = capture_document("body", &[(0, 4)]);
            document.metadata.page_mode = mode;
            assert!(capture(&document, 0).is_none());
        }
    }

    fn reflow_document() -> Document {
        let mut document = capture_document(&"a".repeat(300), &[]);
        document.pages = (0..2).map(blank_page).collect();
        document.pages[1].objects.push(
            PageElement::Image {
                bbox: BoundingBox::default(),
                media_index: 9,
            }
            .into(),
        );
        document
    }

    #[test]
    fn reflow_returns_full_source_for_nonempty_and_empty_inspection_pages() {
        let document = reflow_document();
        let layout = layout_document(&document);
        for (page_index, page) in layout.pages.iter().enumerate() {
            let metadata = page.body_text_slice().unwrap();
            let reflow = metadata.reflow.as_ref().unwrap();
            assert_eq!(reflow.source_range, 0..300);
            assert_eq!(reflow.first_page_index, 0);
            assert_eq!(reflow.requested_page_index, page_index);
            assert!(metadata.capture_window.is_none());
            assert!(page.body_text_capture(&document).is_none());
            assert_eq!(
                page.body_text_reflow(&document).as_ref(),
                document.metadata.note_text.as_ref()
            );
        }
        assert_eq!(
            layout.pages[0].body_text_slice().unwrap().source_range,
            0..300
        );
        assert_eq!(
            layout.pages[1].body_text_slice().unwrap().source_range,
            300..300
        );
        let first = layout.pages[0]
            .body_text
            .as_ref()
            .unwrap()
            .reflow
            .as_ref()
            .unwrap();
        let second = layout.pages[1]
            .body_text
            .as_ref()
            .unwrap()
            .reflow
            .as_ref()
            .unwrap();
        assert!(std::sync::Arc::ptr_eq(
            first.source_snapshot.as_ref().unwrap(),
            second.source_snapshot.as_ref().unwrap()
        ));
    }

    #[test]
    fn unknown_mode_keeps_blank_physical_pages_for_unsectioned_reflow() {
        let mut document = capture_document(&"a".repeat(300), &[]);
        document.metadata.page_mode = None;
        document.pages = (0..2).map(blank_page).collect();
        let layout = layout_document(&document);
        assert_eq!(layout.pages.len(), 2);
        assert!(!layout.omitted_trailing_blank_page);
        assert_eq!(layout.pages[1].source_page_index, 1);
        assert!(
            layout.pages[1]
                .body_text_slice()
                .unwrap()
                .source_range
                .is_empty()
        );
        for page in &layout.pages {
            assert_eq!(page.body_text_reflow(&document).unwrap().text.len(), 300);
        }
    }

    #[test]
    fn reflow_rejects_removed_reordered_and_edited_empty_inspection_nodes() {
        let document = reflow_document();
        let page = layout_document(&document).pages.remove(1);
        let mut changed = page.clone();
        changed.page.objects.clear();
        assert!(changed.body_text_reflow(&document).is_none());
        let mut changed = page.clone();
        changed.page.objects.swap(0, 1);
        assert!(changed.body_text_reflow(&document).is_none());
        let mut changed = page.clone();
        let PageObjectContent::Element(PageElement::TextBox(inspection)) =
            &mut changed.page.objects[0].content
        else {
            unreachable!()
        };
        inspection.font_size = Some(39.0);
        assert!(changed.body_text_reflow(&document).is_none());
        let mut changed = page.clone();
        changed.page.objects[0].source_offset = Some(1);
        assert!(changed.body_text_reflow(&document).is_none());
        let mut changed = page.clone();
        changed.page.objects[0].render_layer = crate::ObjectRenderLayer::Top;
        assert!(changed.body_text_reflow(&document).is_none());
        let mut preview = page.clone();
        preview.page.clear_strokes();
        assert!(preview.body_text_reflow(&document).is_some());
    }

    #[test]
    fn reflow_rejects_changes_outside_the_empty_inspection_slice() {
        let mut document = reflow_document();
        let page = layout_document(&document).pages.remove(1);
        document
            .metadata
            .note_text
            .as_mut()
            .unwrap()
            .runs
            .push(RichTextRun {
                start: 0,
                end: 1,
                bold: true,
                italic: false,
            });
        assert!(page.body_text_reflow(&document).is_none());
        document.metadata.note_text.as_mut().unwrap().runs.clear();
        document
            .metadata
            .note_text
            .as_mut()
            .unwrap()
            .text
            .replace_range(..1, "z");
        assert!(page.body_text_reflow(&document).is_none());
    }

    #[test]
    fn reflow_rejects_stale_bounds_and_native_section_replacement() {
        let mut document = reflow_document();
        let page = layout_document(&document).pages.remove(1);
        let mut changed = page.clone();
        changed
            .body_text
            .as_mut()
            .unwrap()
            .reflow
            .as_mut()
            .unwrap()
            .source_range = 0..usize::MAX;
        assert!(changed.body_text_reflow(&document).is_none());
        let mut changed = page.clone();
        changed
            .body_text
            .as_mut()
            .unwrap()
            .reflow
            .as_mut()
            .unwrap()
            .requested_page_index = 0;
        assert!(changed.body_text_reflow(&document).is_none());
        let mut changed = page.clone();
        changed.body_text.as_mut().unwrap().source_range = 0..0;
        assert!(changed.body_text_reflow(&document).is_none());
        document.metadata.note_text.as_mut().unwrap().text_sections = vec![
            RichTextSection {
                start_utf16: 0,
                length_utf16: 150,
            },
            RichTextSection {
                start_utf16: 150,
                length_utf16: 150,
            },
        ];
        assert!(page.body_text_reflow(&document).is_none());
    }

    #[test]
    fn reflow_handles_invalid_saved_sections_without_slicing_surrogate_pairs() {
        for sections in [
            vec![(0, 2), (2, 2)],
            vec![(3, 1), (0, 3)],
            vec![(0, 0), (0, 0)],
        ] {
            let mut document = capture_document("A😀B", &sections);
            document.pages[1].objects.push(
                PageElement::Image {
                    bbox: BoundingBox::default(),
                    media_index: 9,
                }
                .into(),
            );
            let layout = layout_document(&document);
            for page in &layout.pages {
                assert_eq!(page.body_text_reflow(&document).unwrap().text, "A😀B");
                assert_eq!(
                    page.body_text_slice()
                        .unwrap()
                        .reflow
                        .as_ref()
                        .unwrap()
                        .source_range,
                    0..3
                );
                assert!(page.body_text_capture(&document).is_none());
            }
        }
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serialized_reflow_metadata_requires_rebuilt_source_identity() {
        let document = reflow_document();
        let layout = layout_document(&document);
        let serialized = serde_json::to_value(&layout).unwrap();
        assert!(
            serialized["pages"][0]["body_text"]["reflow"]
                .get("source_snapshot")
                .is_none()
        );
        let decoded: super::LayoutDocument = serde_json::from_value(serialized).unwrap();
        for page in &decoded.pages {
            assert!(page.body_text_reflow(&document).is_none());
        }
    }

    #[cfg(feature = "serde")]
    #[test]
    fn layout_pages_deserialize_without_new_body_metadata() {
        let document = capture_document("body", &[(0, 4)]);
        let mut serialized = serde_json::to_value(layout_document(&document)).unwrap();
        serialized["pages"][0]
            .as_object_mut()
            .unwrap()
            .remove("body_text");
        let layout: super::LayoutDocument = serde_json::from_value(serialized).unwrap();
        assert!(layout.pages[0].body_text_slice().is_none());
        assert_eq!(layout.pages[0].page.elements().count(), 1);
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
    fn keeps_ambiguous_storage_pages_with_unsectioned_flow() {
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
        assert!(!layout.omitted_trailing_blank_page);
        assert_eq!(layout.pages.len(), 6);
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
                page_mode: Some(0),
                flow_dimensions: Some((1080, 4 * 1527)),
                flow_page_padding: Some((0, 0)),
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
                page_mode: Some(0),
                flow_dimensions: Some((1080, 3 * 1527)),
                flow_page_padding: Some((0, 0)),
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
