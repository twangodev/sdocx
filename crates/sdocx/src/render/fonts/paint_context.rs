use std::ops::Range;
use std::sync::Arc;

use crate::text_index::TextIndex;

const MAX_SOURCE_BYTES: usize = 262_144;
const MAX_SOURCE_UTF16: usize = 65_536;
const MAX_SELECTED_SCALARS: usize = 16_384;
const MAX_WINDOWS: usize = 128;

/// A native cache-word source view and its selected interval in paragraph coordinates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaintContextWindow {
    paragraph: Arc<str>,
    context_bytes: Range<usize>,
    context_scalars: Range<usize>,
    context_utf16: Range<u32>,
    selected_bytes: Range<usize>,
    selected_scalars: Range<usize>,
    selected_utf16: Range<u32>,
}

impl PaintContextWindow {
    pub fn source(&self) -> &str {
        &self.paragraph[self.context_bytes.clone()]
    }
    pub fn context_byte_range(&self) -> Range<usize> {
        self.context_bytes.clone()
    }
    pub fn context_scalar_range(&self) -> Range<usize> {
        self.context_scalars.clone()
    }
    pub fn context_range_utf16(&self) -> Range<u32> {
        self.context_utf16.clone()
    }
    pub fn selected_byte_range(&self) -> Range<usize> {
        self.selected_bytes.clone()
    }
    pub fn selected_scalar_range(&self) -> Range<usize> {
        self.selected_scalars.clone()
    }
    pub fn selected_range_utf16(&self) -> Range<u32> {
        self.selected_utf16.clone()
    }
    pub fn selected_range_in_window_utf16(&self) -> Range<u32> {
        self.selected_utf16.start - self.context_utf16.start
            ..self.selected_utf16.end - self.context_utf16.start
    }
}

/// Source-ordered cache-word windows for normal whole-paragraph native measurement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaintContextWindows {
    source: Arc<str>,
    selected_utf16: Range<u32>,
    windows: Vec<PaintContextWindow>,
}

#[derive(Debug, thiserror::Error, Clone, Copy, PartialEq, Eq)]
pub enum PaintContextError {
    #[error("paint context source, selected scalars or window count exceeds its budget")]
    InputBudget,
    #[error("paint context range must be ordered source UTF-16 scalar boundaries")]
    InvalidRange,
}

impl PaintContextWindows {
    pub fn new(source: &str, selected_utf16: Range<u32>) -> Result<Self, PaintContextError> {
        if source.len() > MAX_SOURCE_BYTES {
            return Err(PaintContextError::InputBudget);
        }
        let source_utf16 = source.encode_utf16().take(MAX_SOURCE_UTF16 + 1).count();
        if source_utf16 > MAX_SOURCE_UTF16 {
            return Err(PaintContextError::InputBudget);
        }
        if selected_utf16.start > selected_utf16.end || selected_utf16.end as usize > source_utf16 {
            return Err(PaintContextError::InvalidRange);
        }
        let selected_scalars = source
            .chars()
            .scan(0_u32, |owner, character| {
                let current = *owner;
                *owner += character.len_utf16() as u32;
                Some(current)
            })
            .filter(|owner| selected_utf16.contains(owner))
            .take(MAX_SELECTED_SCALARS + 1)
            .count();
        if selected_scalars > MAX_SELECTED_SCALARS {
            return Err(PaintContextError::InputBudget);
        }
        let index = TextIndex::new(source);
        for endpoint in [selected_utf16.start, selected_utf16.end] {
            index
                .utf16_to_char(endpoint)
                .ok_or(PaintContextError::InvalidRange)?;
        }
        let units: Vec<_> = source.encode_utf16().collect();
        let mut ranges = Vec::new();
        let mut position = selected_utf16.start as usize;
        while position < selected_utf16.end as usize {
            if ranges.len() == MAX_WINDOWS {
                return Err(PaintContextError::InputBudget);
            }
            let context = previous_boundary(&units, position + 1)..next_boundary(&units, position);
            let selected = position..context.end.min(selected_utf16.end as usize);
            position = selected.end;
            ranges.push((context, selected));
        }
        let source: Arc<str> = Arc::from(source);
        let mut windows = Vec::with_capacity(ranges.len());
        for (context, selected) in ranges {
            let context_utf16 = context.start as u32..context.end as u32;
            let selected_utf16 = selected.start as u32..selected.end as u32;
            let context_scalars = scalar_range(&index, &context_utf16)?;
            let selected_scalars = scalar_range(&index, &selected_utf16)?;
            windows.push(PaintContextWindow {
                paragraph: Arc::clone(&source),
                context_bytes: byte_range(&index, &context_scalars),
                selected_bytes: byte_range(&index, &selected_scalars),
                context_scalars,
                context_utf16,
                selected_scalars,
                selected_utf16,
            });
        }
        Ok(Self {
            source,
            selected_utf16,
            windows,
        })
    }

    pub fn source(&self) -> &str {
        &self.source
    }
    pub fn selected_range_utf16(&self) -> Range<u32> {
        self.selected_utf16.clone()
    }
    pub fn windows(&self) -> &[PaintContextWindow] {
        &self.windows
    }
}

fn scalar_range(index: &TextIndex, range: &Range<u32>) -> Result<Range<usize>, PaintContextError> {
    Ok(index
        .utf16_to_char(range.start)
        .ok_or(PaintContextError::InvalidRange)?
        ..index
            .utf16_to_char(range.end)
            .ok_or(PaintContextError::InvalidRange)?)
}

fn byte_range(index: &TextIndex, range: &Range<usize>) -> Range<usize> {
    index.char_to_byte(range.start).unwrap()..index.char_to_byte(range.end).unwrap()
}

fn separator(unit: u16) -> bool {
    matches!(unit, 0x20 | 0x2000..=0x200a | 0x200e..=0x200f | 0x202a..=0x202e | 0x2066..=0x2069 | 0x3000)
}

fn cjk(unit: u16) -> bool {
    matches!(unit, 0x3400..=0x9fff)
}

fn previous_boundary(units: &[u16], offset: usize) -> usize {
    let Some(mut position) = offset.min(units.len()).checked_sub(1) else {
        return 0;
    };
    if separator(units[position]) || cjk(units[position]) {
        return position;
    }
    while position > 0 {
        position -= 1;
        if separator(units[position]) {
            return position + 1;
        }
        if cjk(units[position]) {
            return position;
        }
    }
    0
}

fn next_boundary(units: &[u16], offset: usize) -> usize {
    let Some(&unit) = units.get(offset) else {
        return units.len();
    };
    if separator(unit) {
        return offset + 1;
    }
    units
        .iter()
        .enumerate()
        .skip(offset + 1)
        .find(|&(_, &unit)| separator(unit) || cjk(unit))
        .map_or(units.len(), |(position, _)| position)
}

#[cfg(test)]
mod tests;
