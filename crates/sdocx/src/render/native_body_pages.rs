use super::text::{
    NativePageIndexUnavailable, NativePageLayout, NativePageLineRange, NativePageRecord,
    NativePageSections,
};
use crate::Document;

pub(super) struct NativeBodyPages {
    pages: Vec<NativePageLineRange>,
    #[allow(
        dead_code,
        reason = "Retains native UTF-16 page ownership with render line selection."
    )]
    sections: NativePageSections,
}

impl NativeBodyPages {
    pub fn from_measured(
        document: &Document,
        first_page: usize,
        last_page: usize,
        layout: &NativePageLayout,
    ) -> Result<Self, NativePageIndexUnavailable> {
        use NativePageIndexUnavailable::InvalidPages;
        let source = document
            .metadata
            .note_text
            .as_ref()
            .ok_or(NativePageIndexUnavailable::InvalidSource)?;
        if !source.text_sections.is_empty()
            || i32::try_from(source.text.encode_utf16().count()).ok()
                != Some(layout.text_length_utf16)
        {
            return Err(NativePageIndexUnavailable::OutsideCertificate);
        }
        if first_page != 0 || last_page >= document.pages.len() || last_page >= 250_000 {
            return Err(InvalidPages);
        }
        let mut cumulative_y = 0_i32;
        let records = document.pages[..=last_page]
            .iter()
            .map(|page| {
                let width = i32::try_from(page.width).map_err(|_| InvalidPages)?;
                let height = i32::try_from(page.height).map_err(|_| InvalidPages)?;
                if width <= 0 || height <= 0 {
                    return Err(InvalidPages);
                }
                let record = NativePageRecord {
                    cumulative_y,
                    local_bounds: [0, 0, width, height],
                };
                cumulative_y = cumulative_y.checked_add(height).ok_or(InvalidPages)?;
                Ok(record)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut sections = NativePageSections::default();
        layout.update_sections(Some(&records), 0, &mut sections)?;
        let pages = sections
            .lines
            .iter()
            .copied()
            .zip(&sections.text)
            .map(|(section, source)| {
                use NativePageIndexUnavailable::InvalidSource;
                if section.start == -1 {
                    if section.count != 0 || source.start != -1 || source.count != 0 {
                        return Err(InvalidSource);
                    }
                } else {
                    let end = source
                        .start
                        .checked_add(source.count)
                        .ok_or(InvalidSource)?;
                    if source.start < 0 || source.count < 0 || end > layout.text_length_utf16 {
                        return Err(InvalidSource);
                    }
                    if section.count > 0 && !layout.lines.is_empty() {
                        let start = usize::try_from(section.start).map_err(|_| InvalidSource)?;
                        let last = section
                            .start
                            .checked_add(section.count)
                            .and_then(|end| end.checked_sub(1))
                            .ok_or(InvalidSource)?;
                        let last = usize::try_from(last).map_err(|_| InvalidSource)?;
                        let first = layout
                            .lines
                            .get(start)
                            .ok_or(InvalidSource)?
                            .source_inclusive_utf16;
                        let last = layout
                            .lines
                            .get(last)
                            .ok_or(InvalidSource)?
                            .source_inclusive_utf16;
                        if first[0].max(0) != source.start || last[1].checked_add(1) != Some(end) {
                            return Err(InvalidSource);
                        }
                    }
                }
                let lines = if layout.text_length_utf16 == 0 && layout.lines.is_empty() {
                    NativePageLineRange::EMPTY
                } else {
                    NativePageLineRange::new(section, layout.lines.len())?
                };
                Ok(lines)
            })
            .collect::<Result<_, _>>()?;
        Ok(Self { pages, sections })
    }

    pub fn lines(&self, source_page: usize) -> Option<NativePageLineRange> {
        self.pages.get(source_page).copied()
    }
}

#[cfg(all(test, feature = "serde"))]
mod tests;
