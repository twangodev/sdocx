use std::sync::Arc;

use crate::fonts::{Direction, FontSynthesis, RegisteredFontSource, ResolvedFace};
use crate::render::RenderTheme;
use crate::text_index::TextSource;

use super::layout::TextLayout;
use super::measurement::{MeasuredGlyph, MeasuredRun};
use super::native_runs::{
    MAX_CACHED_GLYPHS, NativeCachedGlyph, NativeEmittedKind, NativeFontState, NativeGlyphCache,
    NativeRect, NativeRunEntry, NativeRunError, NativeRunOffset, native_runs,
};
use super::{NativeDrawSpan, StyledText};

#[derive(Debug, Clone)]
pub(in crate::render) struct NativePaintGlyph {
    pub glyph_id: u32,
    pub origin: [f32; 2],
    #[cfg_attr(not(feature = "pdf"), allow(dead_code))]
    pub advance: [f64; 2],
    pub source: TextSource,
}

#[derive(Debug, Clone)]
pub(in crate::render) struct NativePaintRun {
    pub source: TextSource,
    pub face: ResolvedFace,
    pub font_size: f32,
    pub foreground: u32,
    pub style_bits: u8,
    #[cfg_attr(not(test), allow(dead_code))]
    pub background: u32,
    pub glyphs: Vec<NativePaintGlyph>,
    pub origin: [f32; 2],
    pub layout: NativeRect,
    pub ink: NativeRect,
    synthesis: FontSynthesis,
    #[cfg_attr(not(feature = "pdf"), allow(dead_code))]
    variable: bool,
}

impl NativePaintRun {
    pub fn project(&self, source: TextSource, glyphs: Vec<NativePaintGlyph>) -> Self {
        Self {
            source,
            face: self.face.clone(),
            font_size: self.font_size,
            foreground: self.foreground,
            style_bits: self.style_bits,
            background: self.background,
            glyphs,
            origin: self.origin,
            layout: self.layout,
            ink: self.ink,
            synthesis: self.synthesis,
            variable: self.variable,
        }
    }

    pub fn synthesis(&self) -> FontSynthesis {
        self.synthesis
    }

    #[cfg(feature = "pdf")]
    pub fn variable(&self) -> bool {
        self.variable
    }
}

#[derive(Debug, Clone)]
pub(in crate::render) struct NativePaintParagraph {
    pub source: TextSource,
    pub inverse_logical_map_utf16: Vec<usize>,
}

#[derive(Debug, Clone)]
pub(in crate::render) struct NativePaintPlan {
    pub source: Arc<str>,
    pub runs: Vec<NativePaintRun>,
    pub translation: [f64; 2],
    pub paragraphs: Vec<NativePaintParagraph>,
}

impl NativePaintPlan {
    fn validate_source(&self) -> Result<(), NativePaintPlanUnavailable> {
        let mut end = 0;
        for paragraph in &self.paragraphs {
            if paragraph.source.utf16().start != end {
                return Err(NativePaintPlanUnavailable::InvalidSource);
            }
            end = paragraph.source.utf16().end;
            let length = (end - paragraph.source.utf16().start) as usize;
            if paragraph.inverse_logical_map_utf16.len() != length {
                return Err(NativePaintPlanUnavailable::InvalidSource);
            }
            let mut visited = vec![false; length];
            for &logical in &paragraph.inverse_logical_map_utf16 {
                let used = visited
                    .get_mut(logical)
                    .ok_or(NativePaintPlanUnavailable::InvalidSource)?;
                if std::mem::replace(used, true) {
                    return Err(NativePaintPlanUnavailable::InvalidSource);
                }
            }
        }
        let index = crate::text_index::TextIndex::new(&self.source);
        if index.char_to_utf16(index.len()) != Some(end) {
            return Err(NativePaintPlanUnavailable::InvalidSource);
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub(in crate::render) enum NativePaintPlanUnavailable {
    #[error("text is outside the certified native paint profile: {0}")]
    OutsideCertificate(&'static str),
    #[error("native paint source ownership is invalid")]
    InvalidSource,
    #[error("native paint entry placement is unavailable or invalid")]
    InvalidGeometry,
    #[error("native paint emission failed: {0}")]
    Emission(#[from] NativeRunError),
}

struct PreparedEntry<'a> {
    kind: u32,
    advance: f32,
    position: [f32; 2],
    layout: NativeRect,
    ink: NativeRect,
    drawable: bool,
    font: Option<&'a RegisteredFontSource>,
    span: NativeDrawSpan,
    glyphs: Vec<NativeCachedGlyph>,
}

struct Payload<'a> {
    glyph: &'a MeasuredGlyph,
    run: &'a MeasuredRun,
}

pub(in crate::render) fn native_paint_plan(
    styled: &StyledText<'_>,
    layout: &TextLayout,
    theme: RenderTheme,
) -> Result<NativePaintPlan, NativePaintPlanUnavailable> {
    use NativePaintPlanUnavailable as Error;
    let frame = layout
        .native_frame
        .as_ref()
        .ok_or(Error::OutsideCertificate("layout"))?;
    let source = styled
        .index
        .slice(0..styled.index.len())
        .ok_or(Error::InvalidSource)?;
    if source.is_empty() {
        return Err(Error::OutsideCertificate("empty source"));
    }
    if source.contains(['\r', '\u{2028}', '\u{2029}']) {
        return Err(Error::OutsideCertificate("paragraph separator"));
    }
    if !frame.translation.into_iter().all(f64::is_finite) {
        return Err(Error::InvalidGeometry);
    }
    if styled
        .index
        .char_to_utf16(styled.index.len())
        .is_none_or(|length| length > 250_000)
    {
        return Err(Error::OutsideCertificate("entry budget"));
    }
    let units: Vec<_> = source.encode_utf16().collect();
    let mut prepared: Vec<Option<PreparedEntry<'_>>> = (0..units.len()).map(|_| None).collect();
    let mut payloads = Vec::new();
    let mut paragraphs = styled
        .index
        .display_paragraphs()
        .map(|paragraph| {
            let source = styled
                .index
                .source(paragraph.physical)
                .ok_or(Error::InvalidSource)?;
            let length = (source.utf16().end - source.utf16().start) as usize;
            Ok(NativePaintParagraph {
                source,
                inverse_logical_map_utf16: (0..length).collect(),
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;

    let mut paragraph_index = 0;
    for line in &layout.lines {
        let bands = line.native_bands.ok_or(Error::InvalidGeometry)?;
        if bands.overflow {
            return Err(Error::OutsideCertificate("height limit"));
        }
        let line_source = styled
            .index
            .source(line.line.source.clone())
            .ok_or(Error::InvalidSource)?;
        if line.line.source.is_empty() {
            continue;
        }
        let placed = line
            .line
            .native_placed
            .as_ref()
            .ok_or(Error::InvalidGeometry)?;
        if placed.justified {
            return Err(Error::OutsideCertificate("justification"));
        }
        if placed.positions.len() != (line_source.utf16().end - line_source.utf16().start) as usize
        {
            return Err(Error::InvalidSource);
        }
        while paragraphs
            .get(paragraph_index)
            .is_some_and(|paragraph| paragraph.source.utf16().end <= line_source.utf16().start)
        {
            paragraph_index += 1;
        }
        let paragraph = paragraphs
            .get_mut(paragraph_index)
            .ok_or(Error::InvalidSource)?;
        if paragraph.source.utf16().start > line_source.utf16().start
            || line_source.utf16().end > paragraph.source.utf16().end
        {
            return Err(Error::InvalidSource);
        }
        let map_start = (line_source.utf16().start - paragraph.source.utf16().start) as usize;
        for (visual, &logical) in placed.visual_to_logical.iter().enumerate() {
            *paragraph
                .inverse_logical_map_utf16
                .get_mut(map_start + visual)
                .ok_or(Error::InvalidSource)? = map_start + logical;
        }
        for cluster in &line.line.placements {
            let measured = &cluster.cluster.run;
            let native = measured
                .native_entries
                .as_ref()
                .ok_or(Error::OutsideCertificate("measurement"))?;
            if measured.variable
                || measured.direction != Direction::LeftToRight
                || measured.synthesis != FontSynthesis::default()
            {
                return Err(Error::OutsideCertificate("font or direction"));
            }
            let cluster_source = styled
                .index
                .source(cluster.cluster.source.clone())
                .ok_or(Error::InvalidSource)?;
            let base = native.source().utf16().start;
            for slot in cluster_source.utf16().clone() {
                let owner = slot.checked_sub(base).ok_or(Error::InvalidSource)?;
                let entry = native
                    .geometry()
                    .entry_at_utf16(owner)
                    .ok_or(Error::InvalidSource)?;
                let facts = native
                    .entry_facts_at_utf16(owner)
                    .ok_or(Error::OutsideCertificate("entry facts"))?;
                let logical = (slot - line_source.utf16().start) as usize;
                let position = *placed.positions.get(logical).ok_or(Error::InvalidSource)?;
                let scalar = styled
                    .index
                    .utf16_to_char(slot)
                    .ok_or(Error::OutsideCertificate("surrogate continuation"))?;
                let span = styled
                    .resolved_style_at(scalar, theme, line.predefined)
                    .native_draw
                    .map_err(|_| Error::OutsideCertificate("span"))?;
                if span.foreground >> 24 != 255 || span.flags & 2 != 0 {
                    return Err(Error::OutsideCertificate("paint"));
                }
                let glyph_indices = entry.glyphs();
                if payloads
                    .len()
                    .checked_add(glyph_indices.len())
                    .is_none_or(|count| count > MAX_CACHED_GLYPHS)
                {
                    return Err(NativeRunError::BudgetExceeded.into());
                }
                let mut glyphs = Vec::with_capacity(glyph_indices.len());
                for glyph_index in glyph_indices {
                    let cache = native
                        .geometry()
                        .glyphs()
                        .get(glyph_index)
                        .ok_or(Error::InvalidSource)?;
                    let glyph = measured
                        .glyphs
                        .get(glyph_index)
                        .ok_or(Error::InvalidSource)?;
                    if cache.id() != glyph.id {
                        return Err(Error::InvalidSource);
                    }
                    glyphs.push(NativeCachedGlyph {
                        payload: payloads.len(),
                        offset: cache.position(),
                    });
                    payloads.push(Payload {
                        glyph,
                        run: measured,
                    });
                }
                let ink = entry.ink_bounds();
                let ink = NativeRect([
                    ink[0] + position.left,
                    ink[1] + bands.baseline,
                    ink[2] + position.left,
                    ink[3] + bands.baseline,
                ]);
                let current = PreparedEntry {
                    kind: facts.kind as u32,
                    advance: entry.advance(),
                    position: [position.left, bands.baseline],
                    layout: NativeRect([position.left, bands.top, position.right, bands.bottom]),
                    ink,
                    drawable: facts.drawable,
                    font: facts.drawable.then(|| native.source_instance()).flatten(),
                    span,
                    glyphs,
                };
                let target = prepared
                    .get_mut(slot as usize)
                    .ok_or(Error::InvalidSource)?;
                if target.replace(current).is_some() {
                    return Err(Error::InvalidSource);
                }
            }
        }
    }
    let first_lines = layout
        .lines
        .iter()
        .map(|line| (line.line.source.start, line))
        .rev()
        .collect::<std::collections::BTreeMap<_, _>>();
    for paragraph in styled.index.display_paragraphs() {
        if paragraph.physical.start == paragraph.content.start {
            continue;
        }
        let slot = styled
            .index
            .char_to_utf16(paragraph.physical.start)
            .ok_or(Error::InvalidSource)? as usize;
        if units.get(slot) != Some(&10) {
            return Err(Error::OutsideCertificate("paragraph separator"));
        }
        let line = first_lines
            .get(&paragraph.content.start)
            .ok_or(Error::InvalidSource)?;
        let bands = line.native_bands.ok_or(Error::InvalidGeometry)?;
        let left = if let Some(placed) = &line.line.native_placed {
            placed.positions.first().ok_or(Error::InvalidGeometry)?.left
        } else {
            0.0
        };
        let span = styled
            .resolved_style_at(paragraph.physical.start, theme, line.predefined)
            .native_draw
            .map_err(|_| Error::OutsideCertificate("span"))?;
        prepared[slot] = Some(PreparedEntry {
            kind: 4,
            advance: 0.0,
            position: [left, bands.baseline],
            layout: NativeRect([left, bands.top, left, bands.bottom]),
            ink: NativeRect([left, bands.baseline, left, bands.baseline]),
            drawable: false,
            font: None,
            span,
            glyphs: Vec::new(),
        });
    }
    let prepared = prepared
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .ok_or(Error::InvalidSource)?;
    let entries: Vec<_> = prepared
        .iter()
        .map(|entry| NativeRunEntry {
            kind: entry.kind,
            direction: 0,
            advance: entry.advance,
            position: entry.position,
            layout: entry.layout,
            ink: entry.ink,
            cache: NativeGlyphCache {
                drawable: entry.drawable,
                glyphs: &entry.glyphs,
            },
            span: &entry.span,
            font: entry
                .font
                .map_or(NativeFontState::Missing, |source| NativeFontState::Known {
                    source,
                    bitmap: source.bitmap(),
                    language: source.language(),
                }),
            paragraph_override: false,
        })
        .collect();
    let emitted = if entries.is_empty() {
        Vec::new()
    } else {
        native_runs(&entries, 0..=entries.len() - 1, NativeRunOffset::default())?
    };
    let mut runs = Vec::new();
    for emitted in emitted {
        if emitted.kind == NativeEmittedKind::DefaultEmpty {
            return Err(Error::OutsideCertificate("empty owner"));
        }
        let start = u32::try_from(*emitted.source.start())
            .ok()
            .and_then(|slot| styled.index.utf16_to_char(slot))
            .ok_or(Error::InvalidSource)?;
        let end = u32::try_from(*emitted.source.end() + 1)
            .ok()
            .and_then(|slot| styled.index.utf16_to_char(slot))
            .ok_or(Error::InvalidSource)?;
        let run_source = styled
            .index
            .source(start..end)
            .ok_or(Error::InvalidSource)?;
        let first = payloads
            .get(emitted.glyphs.first().ok_or(Error::InvalidSource)?.payload)
            .ok_or(Error::InvalidSource)?;
        let mut glyphs = Vec::new();
        for glyph in &emitted.glyphs {
            let payload = payloads.get(glyph.payload).ok_or(Error::InvalidSource)?;
            payload
                .glyph
                .source
                .relative_to(&run_source)
                .ok_or(Error::InvalidSource)?;
            glyphs.push(NativePaintGlyph {
                glyph_id: payload.glyph.id,
                origin: [glyph.x, emitted.origin[1]],
                advance: payload.glyph.transport_advance,
                source: payload.glyph.source.clone(),
            });
        }
        runs.push(NativePaintRun {
            source: run_source,
            face: first.run.face.clone(),
            font_size: emitted.paint.font_size,
            foreground: emitted.paint.foreground,
            style_bits: emitted.paint.style_bits,
            background: emitted.paint.background,
            glyphs,
            origin: emitted.origin,
            layout: emitted.layout,
            ink: emitted.ink,
            synthesis: first.run.synthesis,
            variable: first.run.variable,
        });
    }
    let plan = NativePaintPlan {
        source: source.into(),
        runs,
        translation: frame.translation,
        paragraphs,
    };
    plan.validate_source()?;
    Ok(plan)
}

#[cfg(all(test, feature = "serde"))]
mod fixture_tests;

#[cfg(all(test, feature = "serde"))]
mod source_fixture_tests;

#[cfg(all(test, feature = "serde"))]
mod test_support;
