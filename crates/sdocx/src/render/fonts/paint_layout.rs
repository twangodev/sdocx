use super::{PaintShapeDirection, PaintShapedRun};

use std::ops::Range;

#[cfg(test)]
mod fixture_tests;

const MAX_UTF16_ENTRIES: usize = 250_000;
const MAX_GLYPHS: usize = 1_000_000;
const LATIN_SCRIPT: u32 = u32::from_be_bytes(*b"Latn");

#[derive(Debug, Clone, Copy)]
pub(super) struct PaintLayoutPaint {
    pub size: f32,
    pub scale_x: f32,
    pub skew_x: f32,
    pub letter_spacing: f32,
    pub word_spacing: f32,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct PaintLayoutGlyphInput {
    pub id: u32,
    pub cluster_utf16: u32,
    pub advance_x: i32,
    pub advance_y: i32,
    pub offset_x: i32,
    pub offset_y: i32,
    pub ink_bounds: [f32; 4],
}

#[derive(Debug)]
pub(super) struct PaintLayoutChunk<'a> {
    pub source_utf16_length: u32,
    pub range_utf16: Range<u32>,
    pub direction: u32,
    pub script: u32,
    pub font_slot: u8,
    pub font_fakery: u32,
    pub glyphs: &'a [PaintLayoutGlyphInput],
}

#[derive(Debug, Clone, PartialEq)]
pub struct PaintGlyphPlacement {
    pub id: u32,
    /// UTF-16 offset relative to the requested source range.
    pub owner_utf16: u32,
    pub full_position: [f32; 2],
    pub owner_position: [f32; 2],
    /// Backend ink translated by shaped offsets, excluding the accumulated pen.
    pub ink_bounds: [f32; 4],
}

/// Full and owner-relative glyph positions, ink, and advances in native paint units.
#[derive(Debug, Clone, PartialEq)]
pub struct PaintLayout {
    pub glyphs: Vec<PaintGlyphPlacement>,
    pub character_advances: Vec<f32>,
    pub total_advance: f32,
}

#[derive(Debug, thiserror::Error, Clone, Copy, PartialEq, Eq)]
pub enum PaintLayoutError {
    #[error("paint layout requires a nonempty ordered UTF-16 range")]
    InvalidRange,
    #[error("paint layout range exceeds its source UTF-16 length")]
    SourceRangeOutOfBounds,
    #[error("paint layout exceeds the glyph or UTF-16 entry budget")]
    LimitExceeded,
    #[error("paint layout parameters must be finite")]
    NonFinitePaint,
    #[error("paint layout requires positive size and scale and zero word spacing")]
    UnsupportedPaint,
    #[error("paint layout supports one script chunk")]
    UnsupportedChunks,
    #[error("paint layout supports horizontal directions")]
    UnsupportedDirection,
    #[error("paint layout currently supports Latin script chunks")]
    UnsupportedScript,
    #[error("paint layout supports font slot zero")]
    UnsupportedFontSlot,
    #[error("paint layout does not support synthetic font fakery")]
    UnsupportedFontFakery,
    #[error("paint layout requires zero vertical advances")]
    UnsupportedVerticalAdvance,
    #[error("paint layout ink bounds must be finite")]
    NonFiniteInk,
    #[error("paint layout glyph owner lies outside the source range")]
    OwnerOutOfRange,
    #[error("paint layout owners do not follow the shaping direction")]
    NonMonotoneOwners,
    #[error("paint layout produced nonfinite geometry")]
    NonFiniteGeometry,
}

impl PaintShapedRun {
    /// Places a single Latin chunk in native paint units without re-shaping it.
    pub fn layout(
        &self,
        letter_spacing: f32,
        word_spacing: f32,
    ) -> Result<PaintLayout, PaintLayoutError> {
        validate_source_range(
            self.source_utf16_length,
            &self.source_range_utf16,
            self.glyphs.len(),
        )?;
        let glyphs: Vec<_> = self
            .glyphs
            .iter()
            .map(|glyph| PaintLayoutGlyphInput {
                id: glyph.id,
                cluster_utf16: glyph.owner_utf16,
                advance_x: glyph.x_advance,
                advance_y: glyph.y_advance,
                offset_x: glyph.x_offset,
                offset_y: glyph.y_offset,
                ink_bounds: [
                    glyph.backend.ink.left,
                    glyph.backend.ink.top,
                    glyph.backend.ink.right,
                    glyph.backend.ink.bottom,
                ],
            })
            .collect();
        let chunk = PaintLayoutChunk {
            source_utf16_length: self.source_utf16_length,
            range_utf16: self.source_range_utf16.clone(),
            direction: match self.direction {
                PaintShapeDirection::LeftToRight => 4,
                PaintShapeDirection::RightToLeft => 5,
            },
            script: u32::from_be_bytes(self.script),
            font_slot: 0,
            font_fakery: 0,
            glyphs: &glyphs,
        };
        paint_layout(
            PaintLayoutPaint {
                size: self.paint.size,
                scale_x: self.paint.scale_x,
                skew_x: self.paint.skew_x,
                letter_spacing,
                word_spacing,
            },
            &[chunk],
        )
    }
}

impl PaintLayoutPaint {
    fn spacing(self) -> Result<(f64, f64), PaintLayoutError> {
        if ![
            self.size,
            self.scale_x,
            self.skew_x,
            self.letter_spacing,
            self.word_spacing,
        ]
        .into_iter()
        .all(f32::is_finite)
        {
            return Err(PaintLayoutError::NonFinitePaint);
        }
        if self.size <= 0.0 || self.scale_x <= 0.0 || self.word_spacing != 0.0 {
            return Err(PaintLayoutError::UnsupportedPaint);
        }
        let spacing =
            (f64::from(self.size) * f64::from(self.letter_spacing)) * f64::from(self.scale_x);
        Ok((spacing, spacing * 0.5))
    }
}

fn validate_source_range(
    source_utf16_length: u32,
    range: &Range<u32>,
    glyph_count: usize,
) -> Result<usize, PaintLayoutError> {
    let length = range
        .end
        .checked_sub(range.start)
        .ok_or(PaintLayoutError::InvalidRange)? as usize;
    if length == 0 {
        return Err(PaintLayoutError::InvalidRange);
    }
    if length > MAX_UTF16_ENTRIES || glyph_count > MAX_GLYPHS {
        return Err(PaintLayoutError::LimitExceeded);
    }
    if range.end > source_utf16_length {
        return Err(PaintLayoutError::SourceRangeOutOfBounds);
    }
    Ok(length)
}

fn paint_units(coordinate: i32) -> f32 {
    coordinate as f32 * (1.0 / 256.0)
}

fn add_spacing(value: &mut f32, spacing: f64) {
    *value = (f64::from(*value) + spacing) as f32;
}

pub(super) fn paint_layout(
    paint: PaintLayoutPaint,
    chunks: &[PaintLayoutChunk<'_>],
) -> Result<PaintLayout, PaintLayoutError> {
    let (spacing, half_spacing) = paint.spacing()?;
    let [chunk] = chunks else {
        return Err(PaintLayoutError::UnsupportedChunks);
    };
    let start = chunk.range_utf16.start;
    let length = validate_source_range(
        chunk.source_utf16_length,
        &chunk.range_utf16,
        chunk.glyphs.len(),
    )?;
    if !matches!(chunk.direction, 4 | 5) {
        return Err(PaintLayoutError::UnsupportedDirection);
    }
    if chunk.script != LATIN_SCRIPT {
        return Err(PaintLayoutError::UnsupportedScript);
    }
    if chunk.font_slot != 0 {
        return Err(PaintLayoutError::UnsupportedFontSlot);
    }
    if chunk.font_fakery != 0 {
        return Err(PaintLayoutError::UnsupportedFontFakery);
    }
    let mut character_advances = vec![0.0; length];
    let mut glyphs = Vec::with_capacity(chunk.glyphs.len());
    let mut cursor = 0.0;
    let mut cluster_origin = 0.0;
    let mut previous_owner = None;
    for glyph in chunk.glyphs {
        if glyph.advance_y != 0 {
            return Err(PaintLayoutError::UnsupportedVerticalAdvance);
        }
        if !glyph.ink_bounds.into_iter().all(f32::is_finite) {
            return Err(PaintLayoutError::NonFiniteInk);
        }
        let Some(owner) = glyph.cluster_utf16.checked_sub(start) else {
            return Err(PaintLayoutError::OwnerOutOfRange);
        };
        let Some(owner_advance) = character_advances.get_mut(owner as usize) else {
            return Err(PaintLayoutError::OwnerOutOfRange);
        };
        if let Some(previous) = previous_owner {
            if (chunk.direction == 4 && owner < previous)
                || (chunk.direction == 5 && owner > previous)
            {
                return Err(PaintLayoutError::NonMonotoneOwners);
            }
            if owner != previous {
                add_spacing(owner_advance, half_spacing);
                add_spacing(&mut character_advances[previous as usize], half_spacing);
                cluster_origin = cursor;
                add_spacing(&mut cursor, spacing);
            }
        } else {
            add_spacing(owner_advance, half_spacing);
            add_spacing(&mut cursor, half_spacing);
            cluster_origin = cursor;
        }
        let offset_x = paint_units(glyph.offset_x);
        let offset_y = paint_units(glyph.offset_y);
        let sheared_x = (-offset_y).mul_add(paint.skew_x, offset_x);
        let y = 0.0 - offset_y;
        let full_position = [cursor + sheared_x, y];
        let owner_position = [(cursor - cluster_origin) + sheared_x, y];
        let [left, top, right, bottom] = glyph.ink_bounds;
        let ink_bounds = [
            left + sheared_x,
            top - offset_y,
            right + sheared_x,
            bottom - offset_y,
        ];
        let advance = paint_units(glyph.advance_x);
        character_advances[owner as usize] += advance;
        cursor += advance;
        if !full_position
            .into_iter()
            .chain(owner_position)
            .chain(ink_bounds)
            .chain([cursor, character_advances[owner as usize]])
            .all(f32::is_finite)
        {
            return Err(PaintLayoutError::NonFiniteGeometry);
        }
        glyphs.push(PaintGlyphPlacement {
            id: glyph.id,
            owner_utf16: owner,
            full_position,
            owner_position,
            ink_bounds,
        });
        previous_owner = Some(owner);
    }
    if let Some(owner) = previous_owner {
        add_spacing(&mut cursor, half_spacing);
        add_spacing(&mut character_advances[owner as usize], half_spacing);
        if ![cursor, character_advances[owner as usize]]
            .into_iter()
            .all(f32::is_finite)
        {
            return Err(PaintLayoutError::NonFiniteGeometry);
        }
    }
    Ok(PaintLayout {
        glyphs,
        character_advances,
        total_advance: cursor,
    })
}
