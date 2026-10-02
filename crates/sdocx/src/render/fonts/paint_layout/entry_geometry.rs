use super::{MAX_GLYPHS, MAX_UTF16_ENTRIES, PaintLayout};
use crate::text_index::TextIndex;
use std::ops::Range;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub struct PaintEntryGlyph {
    id: u32,
    owner_utf16: u32,
    position: [f32; 2],
    ink_bounds: [f32; 4],
}

impl PaintEntryGlyph {
    pub fn id(&self) -> u32 {
        self.id
    }
    /// Absolute UTF-16 offset in the complete source.
    pub fn owner_utf16(&self) -> u32 {
        self.owner_utf16
    }
    pub fn position(&self) -> [f32; 2] {
        self.position
    }
    pub fn ink_bounds(&self) -> [f32; 4] {
        self.ink_bounds
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PaintLogicalEntry {
    advance: f32,
    ink_bounds: [f32; 4],
    glyphs: Range<usize>,
}

impl PaintLogicalEntry {
    pub fn advance(&self) -> f32 {
        self.advance
    }
    pub fn ink_bounds(&self) -> [f32; 4] {
        self.ink_bounds
    }
    pub fn glyphs(&self) -> Range<usize> {
        self.glyphs.clone()
    }
}

/// Owner-relative geometry in document units, with one entry per requested UTF-16 slot.
#[derive(Debug, Clone, PartialEq)]
pub struct PaintEntryGeometry {
    source: Arc<str>,
    source_range_utf16: Range<u32>,
    entries: Vec<PaintLogicalEntry>,
    glyphs: Vec<PaintEntryGlyph>,
}

impl PaintEntryGeometry {
    pub fn source(&self) -> &str {
        &self.source
    }
    pub fn source_range_utf16(&self) -> Range<u32> {
        self.source_range_utf16.clone()
    }
    pub fn entries(&self) -> &[PaintLogicalEntry] {
        &self.entries
    }
    pub fn glyphs(&self) -> &[PaintEntryGlyph] {
        &self.glyphs
    }
    pub fn entry_at_utf16(&self, owner: u32) -> Option<&PaintLogicalEntry> {
        let offset = owner.checked_sub(self.source_range_utf16.start)?;
        self.entries.get(offset as usize)
    }
}

#[derive(Debug, thiserror::Error, Clone, Copy, PartialEq, Eq)]
pub enum PaintEntryError {
    #[error("logical entry range must use source UTF-16 scalar boundaries")]
    InvalidSourceRange,
    #[error("logical entry geometry exceeds the glyph or UTF-16 budget")]
    LimitExceeded,
    #[error("logical entry advances must match the requested UTF-16 range")]
    InvalidAdvanceCount,
    #[error("logical entry glyph owner must be a source scalar in the requested range")]
    InvalidOwner,
    #[error("logical entry glyphs must be contiguous within each owner")]
    NonContiguousOwner,
    #[error("logical entry conversion requires finite input and output geometry")]
    NonFiniteGeometry,
}

impl PaintLayout {
    pub fn entry_geometry(&self) -> Result<PaintEntryGeometry, PaintEntryError> {
        let length = self
            .source_range_utf16
            .end
            .checked_sub(self.source_range_utf16.start)
            .filter(|&length| length != 0)
            .ok_or(PaintEntryError::InvalidSourceRange)? as usize;
        if length > MAX_UTF16_ENTRIES || self.glyphs.len() > MAX_GLYPHS {
            return Err(PaintEntryError::LimitExceeded);
        }
        if self.character_advances.len() != length {
            return Err(PaintEntryError::InvalidAdvanceCount);
        }
        if self.source.len() > MAX_UTF16_ENTRIES * 4 {
            return Err(PaintEntryError::LimitExceeded);
        }
        let index = TextIndex::new(&self.source);
        if index.utf16_to_char(self.source_range_utf16.start).is_none()
            || index.utf16_to_char(self.source_range_utf16.end).is_none()
        {
            return Err(PaintEntryError::InvalidSourceRange);
        }
        let mut entries = Vec::with_capacity(length);
        for &advance in &self.character_advances {
            let logical_advance = advance / 100.0_f32;
            if !advance.is_finite() || !logical_advance.is_finite() {
                return Err(PaintEntryError::NonFiniteGeometry);
            }
            entries.push(PaintLogicalEntry {
                advance: logical_advance,
                ink_bounds: [0.0; 4],
                glyphs: 0..0,
            });
        }
        let mut glyphs = Vec::with_capacity(self.glyphs.len());
        for glyph in &self.glyphs {
            let owner = self
                .source_range_utf16
                .start
                .checked_add(glyph.owner_utf16)
                .filter(|owner| self.source_range_utf16.contains(owner))
                .filter(|&owner| index.utf16_to_char(owner).is_some())
                .ok_or(PaintEntryError::InvalidOwner)?;
            let entry = entries
                .get_mut(glyph.owner_utf16 as usize)
                .ok_or(PaintEntryError::InvalidOwner)?;
            if !entry.glyphs.is_empty() && entry.glyphs.end != glyphs.len() {
                return Err(PaintEntryError::NonContiguousOwner);
            }
            if !glyph
                .owner_position
                .into_iter()
                .chain(glyph.ink_bounds)
                .all(f32::is_finite)
            {
                return Err(PaintEntryError::NonFiniteGeometry);
            }
            let [x, y] = glyph.owner_position;
            let position = [x / 100.0_f32, y / 100.0_f32];
            let [left, top, right, bottom] = glyph.ink_bounds;
            let translated = [left + x, top + y, right + x, bottom + y];
            let ink_bounds = translated.map(|coordinate| coordinate * 0.01_f32);
            if !position.into_iter().chain(ink_bounds).all(f32::is_finite) {
                return Err(PaintEntryError::NonFiniteGeometry);
            }
            if entry.glyphs.is_empty() {
                entry.glyphs.start = glyphs.len();
            }
            entry.glyphs.end = glyphs.len() + 1;
            union_native_ink(&mut entry.ink_bounds, ink_bounds);
            glyphs.push(PaintEntryGlyph {
                id: glyph.id,
                owner_utf16: owner,
                position,
                ink_bounds,
            });
        }
        Ok(PaintEntryGeometry {
            source: Arc::clone(&self.source),
            source_range_utf16: self.source_range_utf16.clone(),
            entries,
            glyphs,
        })
    }
}

fn union_native_ink(current: &mut [f32; 4], incoming: [f32; 4]) {
    if incoming[0] >= incoming[2] || incoming[1] >= incoming[3] {
        return;
    }
    if current[0] >= current[2] || current[1] >= current[3] {
        *current = incoming;
        return;
    }
    for axis in 0..2 {
        if incoming[axis] < current[axis] {
            current[axis] = incoming[axis];
        }
        if incoming[axis + 2] > current[axis + 2] {
            current[axis + 2] = incoming[axis + 2];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::fonts::PaintGlyphPlacement;

    fn layout() -> PaintLayout {
        PaintLayout {
            source: Arc::from("xA😀e\u{301}y"),
            source_range_utf16: 1..6,
            character_advances: vec![100.0, 200.0, 0.0, 300.0, 0.0],
            total_advance: 600.0,
            glyphs: vec![PaintGlyphPlacement {
                id: 38,
                owner_utf16: 0,
                full_position: [0.0; 2],
                owner_position: [0.0; 2],
                ink_bounds: [0.0, -100.0, 100.0, 0.0],
            }],
        }
    }

    #[test]
    fn partial_source_entries_preserve_absolute_owners_and_empty_continuations() {
        let result = layout().entry_geometry().unwrap();
        assert_eq!(result.source(), "xA😀e\u{301}y");
        assert_eq!(result.source_range_utf16(), 1..6);
        assert!(result.entry_at_utf16(0).is_none());
        assert!(result.entry_at_utf16(6).is_none());
        assert_eq!(result.glyphs()[0].owner_utf16(), 1);
        assert_eq!(result.entry_at_utf16(1).unwrap().glyphs(), 0..1);
        let continuation = result.entry_at_utf16(3).unwrap();
        assert_eq!(continuation.advance(), 0.0);
        assert!(continuation.glyphs().is_empty());
        assert_eq!(continuation.ink_bounds(), [0.0; 4]);
    }

    #[test]
    fn rejects_surrogate_interiors_and_invalid_geometry_before_indexing() {
        let mut input = layout();
        input.source_range_utf16.start = 3;
        input.character_advances.drain(..2);
        assert_eq!(
            input.entry_geometry(),
            Err(PaintEntryError::InvalidSourceRange)
        );
        let mut input = layout();
        input.glyphs[0].owner_utf16 = 2;
        assert_eq!(input.entry_geometry(), Err(PaintEntryError::InvalidOwner));
        input.glyphs[0].owner_utf16 = u32::MAX;
        assert_eq!(input.entry_geometry(), Err(PaintEntryError::InvalidOwner));
        let mut input = layout();
        input.glyphs[0].owner_position[0] = f32::INFINITY;
        assert_eq!(
            input.entry_geometry(),
            Err(PaintEntryError::NonFiniteGeometry)
        );
        let mut input = layout();
        input.glyphs[0].ink_bounds = [f32::MAX; 4];
        input.glyphs[0].owner_position = [f32::MAX; 2];
        assert_eq!(
            input.entry_geometry(),
            Err(PaintEntryError::NonFiniteGeometry)
        );
        let mut input = layout();
        input.character_advances.clear();
        assert_eq!(
            input.entry_geometry(),
            Err(PaintEntryError::InvalidAdvanceCount)
        );
        let mut input = layout();
        input.source = Arc::from("a".repeat(MAX_UTF16_ENTRIES * 4 + 1));
        assert_eq!(input.entry_geometry(), Err(PaintEntryError::LimitExceeded));
    }

    #[test]
    fn owner_glyph_ranges_must_be_contiguous() {
        let mut input = layout();
        let mut next = input.glyphs[0].clone();
        next.owner_utf16 = 1;
        input.glyphs.push(next);
        input.glyphs.push(input.glyphs[0].clone());
        assert_eq!(
            input.entry_geometry(),
            Err(PaintEntryError::NonContiguousOwner)
        );
    }

    #[test]
    fn native_union_ignores_empty_rectangles_and_preserves_signed_zero_ties() {
        let mut ink = [0.0; 4];
        union_native_ink(&mut ink, [-0.0, -1.0, 2.0, 1.0]);
        union_native_ink(&mut ink, [0.0, -1.0, 2.0, 1.0]);
        assert_eq!(ink[0].to_bits(), (-0.0_f32).to_bits());
        union_native_ink(&mut ink, [-10.0, 0.0, 10.0, 0.0]);
        assert_eq!(ink, [-0.0, -1.0, 2.0, 1.0]);
        union_native_ink(&mut ink, [-2.0, -3.0, 4.0, 5.0]);
        assert_eq!(ink, [-2.0, -3.0, 4.0, 5.0]);
    }
}
