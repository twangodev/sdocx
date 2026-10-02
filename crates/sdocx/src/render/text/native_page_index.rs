use super::{StyledText, TextLayout};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::render) struct NativePageLine {
    pub background: [f32; 4],
    pub top: f32,
    pub source_inclusive_utf16: [i32; 2],
}

#[derive(Debug)]
pub(in crate::render) struct NativePageLayout {
    pub text_length_utf16: i32,
    pub lines: Vec<NativePageLine>,
    pub first_empty: [f32; 4],
    pub default_cursor: [f32; 4],
    invalid_background: [f32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub(in crate::render) enum NativePageIndexUnavailable {
    #[error("native page indexing is outside the certified source or layout domain")]
    OutsideCertificate,
    #[error("native page indexing requires finite measured line geometry")]
    InvalidGeometry,
    #[error("native page indexing requires bounded UTF16 ownership")]
    InvalidSource,
    #[error("native page indexing requires a bounded page and rescan range")]
    InvalidPages,
    #[error("native page selection requires a bounded measured line range")]
    InvalidLines,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::render) struct NativePageRecord {
    pub cumulative_y: i32,
    pub local_bounds: [i32; 4],
}

impl NativePageRecord {
    fn bottom(self) -> f32 {
        self.cumulative_y
            .wrapping_add(self.local_bounds[3].wrapping_sub(self.local_bounds[1])) as f32
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::render) struct NativePageSection {
    pub start: i32,
    pub count: i32,
}

impl NativePageSection {
    pub const ABSENT: Self = Self {
        start: -1,
        count: 0,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::render) struct NativePageLineRange {
    start: usize,
    end: usize,
}

impl NativePageLineRange {
    pub const EMPTY: Self = Self { start: 0, end: 0 };

    pub fn new(
        section: NativePageSection,
        measured_lines: usize,
    ) -> Result<Self, NativePageIndexUnavailable> {
        use NativePageIndexUnavailable::InvalidLines;
        if section == NativePageSection::ABSENT {
            return Ok(Self::EMPTY);
        }
        let start = usize::try_from(section.start).map_err(|_| InvalidLines)?;
        let count = usize::try_from(section.count).map_err(|_| InvalidLines)?;
        let end = start.checked_add(count).ok_or(InvalidLines)?;
        if measured_lines > 250_000 || end > measured_lines {
            return Err(InvalidLines);
        }
        Ok(Self { start, end })
    }

    pub fn contains(self, line: usize) -> bool {
        self.start <= line && line < self.end
    }
}

#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub(in crate::render) struct NativePageSections {
    pub lines: Vec<NativePageSection>,
    pub text: Vec<NativePageSection>,
}

impl NativePageLayout {
    fn background(&self, line: i32) -> [f32; 4] {
        usize::try_from(line)
            .ok()
            .and_then(|line| self.lines.get(line))
            .map_or(self.invalid_background, |line| line.background)
    }

    pub fn is_down_line(&self, page: Option<NativePageRecord>, line: i32) -> bool {
        let Some(page) = page else {
            return false;
        };
        let background = if self.text_length_utf16 == 0 {
            self.default_cursor
        } else {
            let background = self.background(line);
            if line == 0 && background[1] <= 0.0 {
                self.first_empty
            } else {
                background
            }
        };
        let top = page.cumulative_y as f32;
        background[1] >= top || background[3] > top
    }

    pub fn is_up_line(&self, page: Option<NativePageRecord>, line: i32) -> bool {
        let Some(page) = page else {
            return false;
        };
        let top = if self.text_length_utf16 == 0 {
            self.default_cursor[1]
        } else {
            let Some(record) = usize::try_from(line)
                .ok()
                .and_then(|line| self.lines.get(line))
            else {
                return false;
            };
            if line == 0 && record.top <= 0.0 {
                self.first_empty[1]
            } else {
                record.top
            }
        };
        top < page.bottom()
    }

    pub fn update_sections(
        &self,
        pages: Option<&[NativePageRecord]>,
        first_page: usize,
        sections: &mut NativePageSections,
    ) -> Result<(), NativePageIndexUnavailable> {
        use NativePageIndexUnavailable as Error;
        let Some(pages) = pages else {
            return Ok(());
        };
        if pages.len() > 250_000
            || first_page > pages.len()
            || pages.iter().any(|page| page.cumulative_y < 0)
        {
            return Err(Error::InvalidPages);
        }
        if self.text_length_utf16 < 0
            || self.text_length_utf16 > 250_000
            || self.lines.len() > 250_000
            || self.lines.iter().enumerate().any(|(index, line)| {
                let [start, end] = line.source_inclusive_utf16;
                !(index == 0 && [start, end] == [-1; 2])
                    && (start < 0 || end < start || end >= self.text_length_utf16)
            })
            || self.lines.windows(2).any(|lines| {
                lines[0].source_inclusive_utf16[0] > lines[1].source_inclusive_utf16[0]
                    || lines[0].source_inclusive_utf16[1] > lines[1].source_inclusive_utf16[1]
            })
        {
            return Err(Error::InvalidSource);
        }
        if !self
            .first_empty
            .into_iter()
            .chain(self.default_cursor)
            .chain(self.invalid_background)
            .all(f32::is_finite)
            || self.lines.iter().any(|line| {
                !line
                    .background
                    .into_iter()
                    .chain([line.top])
                    .all(f32::is_finite)
            })
        {
            return Err(Error::InvalidGeometry);
        }
        if self.invalid_background != [0.0; 4] {
            return Err(Error::OutsideCertificate);
        }
        let line_count = if self.text_length_utf16 == 0 {
            1
        } else {
            self.lines.len() as i32
        };
        let mut cursor = if first_page > 0 {
            let previous = sections
                .lines
                .get(first_page - 1)
                .copied()
                .unwrap_or(NativePageSection::ABSENT);
            if previous.count < 0 || previous.start < -1 {
                return Err(Error::InvalidSource);
            }
            previous
                .start
                .checked_add(previous.count)
                .and_then(|end| end.checked_sub(1))
                .ok_or(Error::InvalidSource)?
                .max(0)
        } else {
            0
        };
        if cursor > line_count {
            return Err(Error::InvalidSource);
        }
        sections
            .lines
            .resize(pages.len(), NativePageSection::ABSENT);
        sections.text.resize(pages.len(), NativePageSection::ABSENT);
        let mut page_index = first_page;
        while page_index < pages.len() && line_count != 0 {
            let page = pages[page_index];
            while cursor < line_count && !self.is_down_line(Some(page), cursor) {
                cursor += 1;
            }
            let start = cursor;
            let found = start < line_count;
            let mut end = start;
            while end < line_count && self.is_up_line(Some(page), end) {
                end += 1;
            }
            let count = (end - start).max(0);
            sections.lines[page_index] = NativePageSection { start, count };
            let text_start = if found {
                if self.text_length_utf16 == 0 {
                    0
                } else {
                    self.lines[start as usize].source_inclusive_utf16[0].max(0)
                }
            } else {
                self.text_length_utf16
            };
            let text_count = if self.text_length_utf16 == 0 || count == 0 {
                0
            } else {
                self.lines[(end - 1) as usize].source_inclusive_utf16[1]
                    .wrapping_sub(text_start)
                    .wrapping_add(1)
            };
            sections.text[page_index] = NativePageSection {
                start: text_start,
                count: text_count,
            };
            cursor = if end > start || self.background(end - 1)[3] > page.cumulative_y as f32 {
                end - 1
            } else {
                end
            };
            page_index += 1;
            if start == line_count {
                break;
            }
        }
        for page in page_index..pages.len() {
            if sections.lines[page] == NativePageSection::ABSENT {
                break;
            }
            sections.lines[page] = NativePageSection::ABSENT;
            sections.text[page] = NativePageSection::ABSENT;
        }
        Ok(())
    }

    pub(super) fn checked_source_length(
        styled: &StyledText<'_>,
        width: i32,
    ) -> Result<i32, NativePageIndexUnavailable> {
        use NativePageIndexUnavailable as Error;
        let source = styled.text_box;
        if width <= 0
            || f64::from(width as f32) != f64::from(width)
            || !matches!(styled.context(), super::TextContext::Flow)
            || source.text.len() > 250_000
            || !source
                .text
                .bytes()
                .all(|byte| byte == b'\n' || (b' '..=b'~').contains(&byte))
            || !source.runs.is_empty()
            || !source.spans.is_empty()
            || !source.object_spans.is_empty()
            || !source.text_sections.is_empty()
            || source.highlight_color.is_some()
            || source.underline
            || source
                .font_size
                .is_some_and(|size| size != 17.0 && size != 50.0)
            || source.gravity.is_some_and(|gravity| gravity != 0)
            || source.rotation_degrees.is_some_and(|angle| angle != 0.0)
            || source
                .margins
                .is_some_and(|margins| !margins.into_iter().all(f32::is_finite))
            || source
                .paragraphs
                .iter()
                .any(|paragraph| paragraph.kind != crate::RichTextParagraphType::ParsingState)
            || styled.settings.scale != 1.0
            || styled.settings.font_size_delta != 0.0
        {
            return Err(Error::OutsideCertificate);
        }
        let text_length_utf16 = i32::try_from(
            styled
                .index
                .char_to_utf16(styled.index.len())
                .ok_or(Error::InvalidSource)?,
        )
        .map_err(|_| Error::InvalidSource)?;
        if text_length_utf16 > 250_000 {
            return Err(Error::OutsideCertificate);
        }
        Ok(text_length_utf16)
    }

    pub fn from_measured(
        styled: &StyledText<'_>,
        layout: &TextLayout,
        width: i32,
    ) -> Result<Self, NativePageIndexUnavailable> {
        use NativePageIndexUnavailable as Error;
        let text_length_utf16 = Self::checked_source_length(styled, width)?;
        if layout.lines.len() > 250_000
            || layout
                .native_frame
                .as_ref()
                .is_none_or(|frame| frame.translation != [0.0; 2])
        {
            return Err(Error::OutsideCertificate);
        }
        let source = styled.text_box;
        let paragraphs: Vec<_> = styled.index.display_paragraphs().collect();
        let margins = source.margins.unwrap_or([0.0; 4]);
        let content_width = f64::from(width) - f64::from(margins[0]) - f64::from(margins[2]);
        if content_width <= 0.0 || !content_width.is_finite() {
            return Err(Error::OutsideCertificate);
        }
        let mut paragraph_index = 0;
        let mut content_cursor = 0;
        let mut lines = Vec::with_capacity(layout.lines.len());
        for line in &layout.lines {
            let bands = line.native_bands.ok_or(Error::InvalidGeometry)?;
            if bands.overflow || line.marker.is_some() || line.predefined.is_some() {
                return Err(Error::OutsideCertificate);
            }
            if line.width != content_width || line.x != f64::from(margins[0]) {
                return Err(Error::OutsideCertificate);
            }
            for placement in &line.line.placements {
                let measured = &placement.cluster.run;
                if measured.variable
                    || measured.direction != crate::fonts::Direction::LeftToRight
                    || measured.synthesis != crate::fonts::FontSynthesis::default()
                {
                    return Err(Error::OutsideCertificate);
                }
                let native = measured
                    .native_entries
                    .as_ref()
                    .ok_or(Error::OutsideCertificate)?;
                let cluster = styled
                    .index
                    .source(placement.cluster.source.clone())
                    .ok_or(Error::InvalidSource)?;
                for owner in cluster.utf16().clone() {
                    let owner = owner
                        .checked_sub(native.source().utf16().start)
                        .ok_or(Error::InvalidSource)?;
                    if native.entry_facts_at_utf16(owner).is_none() {
                        return Err(Error::OutsideCertificate);
                    }
                }
            }
            let paragraph = paragraphs
                .get(paragraph_index)
                .ok_or(Error::InvalidSource)?;
            if line.line.source.start != content_cursor
                || line.line.source.start < paragraph.content.start
                || line.line.source.end < line.line.source.start
                || line.line.source.end > paragraph.content.end
            {
                return Err(Error::InvalidSource);
            }
            let source_inclusive_utf16 = if line.line.source.is_empty() && paragraph_index == 0 {
                [-1; 2]
            } else {
                let start = if line.line.source.start == paragraph.content.start {
                    paragraph.physical.start
                } else {
                    line.line.source.start
                };
                [
                    i32::try_from(
                        styled
                            .index
                            .char_to_utf16(start)
                            .ok_or(Error::InvalidSource)?,
                    )
                    .map_err(|_| Error::InvalidSource)?,
                    i32::try_from(
                        styled
                            .index
                            .char_to_utf16(line.line.source.end)
                            .ok_or(Error::InvalidSource)?,
                    )
                    .map_err(|_| Error::InvalidSource)?
                        - 1,
                ]
            };
            let x = line.x as f32;
            let right = if line.line.source.is_empty() {
                x
            } else {
                let placed = line
                    .line
                    .native_placed
                    .as_ref()
                    .ok_or(Error::OutsideCertificate)?;
                if placed.justified || placed.positions.is_empty() {
                    return Err(Error::OutsideCertificate);
                }
                let source = styled
                    .index
                    .source(line.line.source.clone())
                    .ok_or(Error::InvalidSource)?;
                if placed.positions.len() != (source.utf16().end - source.utf16().start) as usize
                    || placed.visual_to_logical.len() != placed.positions.len()
                    || placed
                        .visual_to_logical
                        .iter()
                        .enumerate()
                        .any(|(visual, &logical)| visual != logical)
                {
                    return Err(Error::OutsideCertificate);
                }
                placed.positions.last().ok_or(Error::InvalidGeometry)?.right
            };
            let background = [x, bands.top, right, bands.bottom];
            if !background.into_iter().all(f32::is_finite) || f64::from(x) != line.x {
                return Err(Error::InvalidGeometry);
            }
            lines.push(NativePageLine {
                background,
                top: bands.top,
                source_inclusive_utf16,
            });
            content_cursor = line.line.source.end;
            if content_cursor == paragraph.content.end {
                paragraph_index += 1;
                if let Some(next) = paragraphs.get(paragraph_index) {
                    content_cursor = next.content.start;
                }
            }
        }
        if paragraph_index != paragraphs.len() {
            return Err(Error::InvalidSource);
        }
        let caret_height = styled.caret_line_height(0) as f32;
        if !caret_height.is_finite() || caret_height <= 0.0 {
            return Err(Error::InvalidGeometry);
        }
        let first_paragraph_empty = paragraphs
            .first()
            .is_none_or(|paragraph| paragraph.content.is_empty());
        let first_empty = if first_paragraph_empty {
            if source.margins.is_some_and(|margins| margins != [0.0; 4]) {
                return Err(Error::OutsideCertificate);
            }
            [0.0, 0.0, width as f32, caret_height]
        } else {
            [0.0; 4]
        };
        let cursor_offset = (((first_empty[3] - first_empty[1]) - caret_height) * 0.5).trunc();
        let default_cursor = [
            0.0,
            first_empty[1] + cursor_offset,
            0.0,
            first_empty[1] + caret_height + cursor_offset,
        ];
        Ok(Self {
            text_length_utf16,
            lines,
            first_empty,
            default_cursor,
            invalid_background: [0.0; 4],
        })
    }
}

#[cfg(all(test, feature = "serde"))]
mod tests;
