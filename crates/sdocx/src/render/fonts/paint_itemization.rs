use std::ops::Range;
use std::sync::Arc;

use super::PaintSourceInfo;
use crate::render::harfrust;
use crate::text_index::TextIndex;

const MAX_SOURCE_BYTES: usize = 262_144;
const MAX_SOURCE_UTF16: usize = 65_536;
const MAX_SELECTED_SCALARS: usize = 16_384;
const CONTEXT_SCALARS: usize = 5;
const COMMON: [u8; 4] = *b"Zyyy";
const INHERITED: [u8; 4] = *b"Zinh";

/// A script chunk with absolute byte, scalar and UTF-16 coordinates in its full source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaintScriptChunk {
    script: [u8; 4],
    byte_range: Range<usize>,
    scalar_range: Range<usize>,
    source_range_utf16: Range<u32>,
    infos: Vec<PaintSourceInfo>,
    pre_context: String,
    post_context: String,
}

impl PaintScriptChunk {
    pub fn script(&self) -> [u8; 4] {
        self.script
    }
    pub fn byte_range(&self) -> Range<usize> {
        self.byte_range.clone()
    }
    pub fn scalar_range(&self) -> Range<usize> {
        self.scalar_range.clone()
    }
    pub fn source_range_utf16(&self) -> Range<u32> {
        self.source_range_utf16.clone()
    }
    pub fn infos(&self) -> &[PaintSourceInfo] {
        &self.infos
    }
    /// Preceding full-source scalars in source order, including outside the requested range.
    pub fn pre_context(&self) -> &str {
        &self.pre_context
    }
    /// Following full-source scalars in source order, including outside the requested range.
    pub fn post_context(&self) -> &str {
        &self.post_context
    }
}

/// Native-style script chunks with absolute source coordinates and bounded full-source context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaintItemization {
    source: Arc<str>,
    source_range_utf16: Range<u32>,
    chunks: Vec<PaintScriptChunk>,
}

#[derive(Debug, thiserror::Error, Clone, Copy, PartialEq, Eq)]
pub enum PaintItemizationError {
    #[error("script itemization exceeds its source or selected-scalar budget")]
    InputBudget,
    #[error("script itemization range must be ordered source UTF-16 scalar boundaries")]
    InvalidRange,
}

impl PaintItemization {
    pub fn new(source: &str, range_utf16: Range<u32>) -> Result<Self, PaintItemizationError> {
        if source.len() > MAX_SOURCE_BYTES {
            return Err(PaintItemizationError::InputBudget);
        }
        let source_utf16 = source.encode_utf16().take(MAX_SOURCE_UTF16 + 1).count();
        if source_utf16 > MAX_SOURCE_UTF16 {
            return Err(PaintItemizationError::InputBudget);
        }
        if range_utf16.start > range_utf16.end || range_utf16.end as usize > source_utf16 {
            return Err(PaintItemizationError::InvalidRange);
        }
        let selected_scalars = source
            .chars()
            .scan(0_u32, |owner, character| {
                let current = *owner;
                *owner += character.len_utf16() as u32;
                Some(current)
            })
            .filter(|owner| range_utf16.contains(owner))
            .take(MAX_SELECTED_SCALARS + 1)
            .count();
        if selected_scalars > MAX_SELECTED_SCALARS {
            return Err(PaintItemizationError::InputBudget);
        }
        let index = TextIndex::new(source);
        let start = index
            .utf16_to_char(range_utf16.start)
            .ok_or(PaintItemizationError::InvalidRange)?;
        let end = index
            .utf16_to_char(range_utf16.end)
            .ok_or(PaintItemizationError::InvalidRange)?;
        let selected = index.slice(start..end).unwrap();
        let mut owner_utf16 = range_utf16.start;
        let infos: Vec<_> = selected
            .chars()
            .map(|character| {
                let info = PaintSourceInfo {
                    character,
                    owner_utf16,
                };
                owner_utf16 += character.len_utf16() as u32;
                info
            })
            .collect();
        let chunks = script_ranges(&infos, |info, _| harfrust::script_for(info.character))
            .into_iter()
            .map(|(range, script)| {
                let scalar_range = start + range.start..start + range.end;
                let byte_range = index.char_to_byte(scalar_range.start).unwrap()
                    ..index.char_to_byte(scalar_range.end).unwrap();
                let source_range_utf16 = index.char_to_utf16(scalar_range.start).unwrap()
                    ..index.char_to_utf16(scalar_range.end).unwrap();
                PaintScriptChunk {
                    script,
                    byte_range,
                    pre_context: index
                        .slice(
                            scalar_range.start.saturating_sub(CONTEXT_SCALARS)..scalar_range.start,
                        )
                        .unwrap()
                        .to_owned(),
                    post_context: index
                        .slice(
                            scalar_range.end..(scalar_range.end + CONTEXT_SCALARS).min(index.len()),
                        )
                        .unwrap()
                        .to_owned(),
                    scalar_range,
                    source_range_utf16,
                    infos: infos[range].to_vec(),
                }
            })
            .collect();
        Ok(Self {
            source: Arc::from(source),
            source_range_utf16: range_utf16,
            chunks,
        })
    }

    pub fn source(&self) -> &str {
        &self.source
    }
    pub fn source_range_utf16(&self) -> Range<u32> {
        self.source_range_utf16.clone()
    }
    pub fn chunks(&self) -> &[PaintScriptChunk] {
        &self.chunks
    }
}

fn script_ranges(
    infos: &[PaintSourceInfo],
    mut script_for: impl FnMut(PaintSourceInfo, bool) -> [u8; 4],
) -> Vec<(Range<usize>, [u8; 4])> {
    let mut chunks = Vec::new();
    let mut start = 0;
    while start < infos.len() {
        let mut script = script_for(infos[start], true);
        let mut end = start + 1;
        while let Some(next) = infos.get(end) {
            let next_script = script_for(*next, false);
            if script == next_script {
                end += 1;
            } else if matches!(script, COMMON | INHERITED) {
                script = next_script;
                end += 1;
            } else if matches!(next_script, COMMON | INHERITED) {
                end += 1;
            } else {
                break;
            }
        }
        if script == INHERITED {
            script = COMMON;
        }
        chunks.push((start..end, script));
        start = end;
    }
    chunks
}

#[cfg(test)]
mod tests;
