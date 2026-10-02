use std::ops::Range;

mod fixture_tests;

const MAX_UTF16_ENTRIES: usize = 250_000;
const MAX_GLYPHS: usize = 1_000_000;
const LATIN_SCRIPT: u32 = u32::from_be_bytes(*b"Latn");

#[derive(Debug, Clone, Copy)]
pub(super) struct NativeShapingPaint {
    pub size: f32,
    pub scale_x: f32,
    pub skew_x: f32,
    pub letter_spacing: f32,
    pub word_spacing: f32,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct NativeShapedGlyph {
    pub id: u32,
    pub cluster_utf16: u32,
    pub advance_x: i32,
    pub advance_y: i32,
    pub offset_x: i32,
    pub offset_y: i32,
    pub ink_bounds: [f32; 4],
}

#[derive(Debug)]
pub(super) struct NativeShapingChunk<'a> {
    pub source_utf16_length: u32,
    pub range_utf16: Range<u32>,
    pub direction: u32,
    pub script: u32,
    pub font_slot: u8,
    pub font_fakery: u32,
    pub glyphs: &'a [NativeShapedGlyph],
}

#[derive(Debug)]
pub(super) struct NativeGlyphPlacement {
    pub id: u32,
    pub owner_utf16: u32,
    pub full_position: [f32; 2],
    pub owner_position: [f32; 2],
    pub ink_bounds: [f32; 4],
}

#[derive(Debug)]
pub(super) struct NativeShapingGeometry {
    pub glyphs: Vec<NativeGlyphPlacement>,
    pub character_advances: Vec<f32>,
    pub total_advance: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NativeShapingUnavailable {
    InvalidRange,
    SourceRangeOutOfBounds,
    LimitExceeded,
    NonFinitePaint,
    UnsupportedPaint,
    UnsupportedChunks,
    UnsupportedDirection,
    UnsupportedScript,
    UnsupportedFontSlot,
    UnsupportedFontFakery,
    UnsupportedVerticalAdvance,
    NonFiniteInk,
    OwnerOutOfRange,
    NonMonotoneOwners,
    NonFiniteGeometry,
}

impl NativeShapingPaint {
    fn spacing(self) -> Result<(f64, f64), NativeShapingUnavailable> {
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
            return Err(NativeShapingUnavailable::NonFinitePaint);
        }
        if self.size <= 0.0 || self.scale_x <= 0.0 || self.word_spacing != 0.0 {
            return Err(NativeShapingUnavailable::UnsupportedPaint);
        }
        let spacing =
            (f64::from(self.size) * f64::from(self.letter_spacing)) * f64::from(self.scale_x);
        Ok((spacing, spacing * 0.5))
    }
}

pub(super) fn native_vector_advance(raw_advance: f32) -> Option<i32> {
    let scaled = raw_advance * 256.0;
    (scaled.is_finite()
        && f64::from(scaled) >= f64::from(i32::MIN)
        && f64::from(scaled) < f64::from(i32::MAX) + 1.0)
        .then_some(scaled as i32)
}

fn paint_units(coordinate: i32) -> f32 {
    coordinate as f32 * (1.0 / 256.0)
}

fn add_spacing(value: &mut f32, spacing: f64) {
    *value = (f64::from(*value) + spacing) as f32;
}

pub(super) fn native_shaping_geometry(
    paint: NativeShapingPaint,
    chunks: &[NativeShapingChunk<'_>],
) -> Result<NativeShapingGeometry, NativeShapingUnavailable> {
    let (spacing, half_spacing) = paint.spacing()?;
    let [chunk] = chunks else {
        return Err(NativeShapingUnavailable::UnsupportedChunks);
    };
    let Range { start, end } = chunk.range_utf16;
    let Some(length) = end.checked_sub(start) else {
        return Err(NativeShapingUnavailable::InvalidRange);
    };
    let length = length as usize;
    if length == 0 {
        return Err(NativeShapingUnavailable::InvalidRange);
    }
    if length > MAX_UTF16_ENTRIES || chunk.glyphs.len() > MAX_GLYPHS {
        return Err(NativeShapingUnavailable::LimitExceeded);
    }
    if end > chunk.source_utf16_length {
        return Err(NativeShapingUnavailable::SourceRangeOutOfBounds);
    }
    if !matches!(chunk.direction, 4 | 5) {
        return Err(NativeShapingUnavailable::UnsupportedDirection);
    }
    if chunk.script != LATIN_SCRIPT {
        return Err(NativeShapingUnavailable::UnsupportedScript);
    }
    if chunk.font_slot != 0 {
        return Err(NativeShapingUnavailable::UnsupportedFontSlot);
    }
    if chunk.font_fakery != 0 {
        return Err(NativeShapingUnavailable::UnsupportedFontFakery);
    }
    let mut character_advances = vec![0.0; length];
    let mut glyphs = Vec::with_capacity(chunk.glyphs.len());
    let mut cursor = 0.0;
    let mut cluster_origin = 0.0;
    let mut previous_owner = None;
    for glyph in chunk.glyphs {
        if glyph.advance_y != 0 {
            return Err(NativeShapingUnavailable::UnsupportedVerticalAdvance);
        }
        if !glyph.ink_bounds.into_iter().all(f32::is_finite) {
            return Err(NativeShapingUnavailable::NonFiniteInk);
        }
        let Some(owner) = glyph.cluster_utf16.checked_sub(start) else {
            return Err(NativeShapingUnavailable::OwnerOutOfRange);
        };
        let Some(owner_advance) = character_advances.get_mut(owner as usize) else {
            return Err(NativeShapingUnavailable::OwnerOutOfRange);
        };
        if let Some(previous) = previous_owner {
            if (chunk.direction == 4 && owner < previous)
                || (chunk.direction == 5 && owner > previous)
            {
                return Err(NativeShapingUnavailable::NonMonotoneOwners);
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
            return Err(NativeShapingUnavailable::NonFiniteGeometry);
        }
        glyphs.push(NativeGlyphPlacement {
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
            return Err(NativeShapingUnavailable::NonFiniteGeometry);
        }
    }
    Ok(NativeShapingGeometry {
        glyphs,
        character_advances,
        total_advance: cursor,
    })
}
