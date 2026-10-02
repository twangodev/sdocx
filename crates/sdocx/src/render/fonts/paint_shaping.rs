use std::collections::{HashMap, HashSet};

use crate::render::harfrust;

use harfrust::font::{AdvanceWidthBatch, BuiltinFontFuncs, FontFuncs};
use harfrust::{FontRef, GlyphId, IntegerScalingRounding, ShapeOptions, ShaperData};

use super::{PaintGlyphMetrics, PaintMetricError, PaintMetricInput, PaintMetrics};

mod device_tables;
mod positioning_domain;

use positioning_domain::PositioningDomain;

const MAX_SOURCE_BYTES: usize = 262_144;
const MAX_SOURCE_UTF16: u32 = 65_536;
const MAX_INPUT_SCALARS: usize = 16_384;
const MAX_OUTPUT_GLYPHS: usize = 65_536;
const MAX_SHAPING_OPERATIONS: i32 = 2_000_000;
const MAX_FEATURES: usize = 256;
const MAX_CONTEXT_SCALARS: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaintShapeDirection {
    LeftToRight,
    RightToLeft,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaintShapeClusterLevel {
    MonotoneGraphemes,
    MonotoneCharacters,
    Characters,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaintSourceInfo {
    pub character: char,
    pub owner_utf16: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaintShapeFeature {
    pub tag: [u8; 4],
    pub value: u32,
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct PaintShapeRequest<'a> {
    pub source: &'a str,
    pub infos: &'a [PaintSourceInfo],
    pub direction: PaintShapeDirection,
    pub script: [u8; 4],
    pub language: Option<&'a str>,
    pub flags: u32,
    pub cluster_level: PaintShapeClusterLevel,
    /// At most five preceding Unicode scalars, in source order.
    pub pre_context: &'a str,
    /// At most five following Unicode scalars, in source order.
    pub post_context: &'a str,
    pub features: &'a [PaintShapeFeature],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaintShapeScale {
    pub x: i32,
    pub y: i32,
    /// Native ppem conversion, recorded independently of shaping scale.
    pub ppem: [u32; 2],
}

impl PaintShapeScale {
    pub fn new(input: PaintMetricInput) -> Result<Self, PaintShapeError> {
        let horizontal = f64::from(input.size) * f64::from(input.scale_x);
        let x = quantized_integer(f64::from((horizontal as f32) * 256.0))?;
        let y = quantized_integer(f64::from(input.size * 256.0))?;
        if x <= 0 || y <= 0 || horizontal < 0.0 || !horizontal.is_finite() {
            return Err(PaintShapeError::InvalidScale);
        }
        Ok(Self {
            x,
            y,
            ppem: [horizontal as u32, input.size as u32],
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PaintShapedGlyph {
    pub id: u32,
    pub owner_utf16: u32,
    pub x_advance: i32,
    pub y_advance: i32,
    pub x_offset: i32,
    pub y_offset: i32,
    pub unsafe_to_break: bool,
    /// Backend advance and ink before HarfBuzz positioning, in paint units.
    pub backend: PaintGlyphMetrics,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PaintShapedRun {
    pub(super) source_utf16_length: u32,
    pub(super) source_range_utf16: std::ops::Range<u32>,
    pub(super) script: [u8; 4],
    pub(super) paint: PaintMetricInput,
    pub(super) scale: PaintShapeScale,
    pub(super) direction: PaintShapeDirection,
    /// Positions and advances use signed 24.8 paint units.
    pub(super) glyphs: Vec<PaintShapedGlyph>,
}

impl PaintShapedRun {
    pub fn source_utf16_length(&self) -> u32 {
        self.source_utf16_length
    }

    pub fn source_range_utf16(&self) -> std::ops::Range<u32> {
        self.source_range_utf16.clone()
    }

    pub fn script(&self) -> [u8; 4] {
        self.script
    }

    pub fn paint(&self) -> PaintMetricInput {
        self.paint
    }

    pub fn scale(&self) -> PaintShapeScale {
        self.scale
    }

    pub fn direction(&self) -> PaintShapeDirection {
        self.direction
    }

    pub fn glyphs(&self) -> &[PaintShapedGlyph] {
        &self.glyphs
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PaintShapeError {
    #[error(transparent)]
    Metrics(#[from] PaintMetricError),
    #[error("paint shaping scale exceeds the positive signed 24.8 domain")]
    InvalidScale,
    #[error("paint shaping input exceeds the bounded source, feature, or context budget")]
    InputBudget,
    #[error("source infos must identify contiguous Unicode scalars at their UTF-16 starts")]
    InvalidSource,
    #[error("paint shaping properties or feature ranges are invalid")]
    InvalidProperties,
    #[error("paint shaping does not support device positioning or hinted contour anchors")]
    UnsupportedPositioningTables,
    #[error("paint shaping callback {0} has no verified native implementation")]
    UnsupportedCallback(&'static str),
    #[error("paint shaping output exceeds the glyph budget or has invalid UTF-16 ownership")]
    InvalidOutput,
    #[error("paint shaping font positioning exceeds the bounded numeric domain")]
    UnsupportedPositioningDomain,
    #[error("paint shaping exceeded its glyph expansion or tracked operation budget")]
    ShapingBudget,
    #[error("paint advance exceeds the signed callback coordinate domain")]
    AdvanceRange,
}

/// A reusable font/paint instance; raw glyph metrics are cached across shape calls.
///
/// This backend shapes explicit script chunks. It does not select fonts, itemize
/// text, apply layout-piece spacing, or implement ppem-dependent device tables.
pub struct PaintShaper<'font> {
    font: FontRef<'font>,
    data: ShaperData,
    metrics: PaintMetrics<'font>,
    paint: PaintMetricInput,
    scale: PaintShapeScale,
    cache: HashMap<u32, PaintGlyphMetrics>,
    domain: PositioningDomain,
}

impl<'font> PaintShaper<'font> {
    pub(super) fn new(
        bytes: &'font [u8],
        index: u32,
        input: PaintMetricInput,
    ) -> Result<Self, PaintShapeError> {
        let metrics = PaintMetrics::new(bytes, index, input)?;
        let font = FontRef::from_index(bytes, index).map_err(|_| PaintMetricError::InvalidFont)?;
        if device_tables::requires_device_positioning(&font)
            .map_err(|_| PaintMetricError::InvalidFont)?
        {
            return Err(PaintShapeError::UnsupportedPositioningTables);
        }
        let scale = PaintShapeScale::new(input)?;
        let domain = PositioningDomain::new(&font, input, scale)?;
        let data = ShaperData::new(&font);
        Ok(Self {
            font,
            data,
            metrics,
            paint: input,
            scale,
            cache: HashMap::new(),
            domain,
        })
    }

    pub fn scale(&self) -> PaintShapeScale {
        self.scale
    }

    pub fn shape(
        &mut self,
        request: PaintShapeRequest<'_>,
    ) -> Result<PaintShapedRun, PaintShapeError> {
        shape_with_metrics(
            (&self.font, &self.data),
            self.paint,
            request,
            &mut self.cache,
            &mut |glyph| self.metrics.glyph(glyph).map_err(Into::into),
            IntegerScalingRounding::Floor,
            &self.domain,
        )
    }
}

fn shape_with_metrics(
    engine: (&FontRef<'_>, &ShaperData),
    paint: PaintMetricInput,
    request: PaintShapeRequest<'_>,
    cache: &mut HashMap<u32, PaintGlyphMetrics>,
    measure: &mut dyn FnMut(u32) -> Result<PaintGlyphMetrics, PaintShapeError>,
    rounding: IntegerScalingRounding,
    domain: &PositioningDomain,
) -> Result<PaintShapedRun, PaintShapeError> {
    let (font, data) = engine;
    let scale = PaintShapeScale::new(paint)?;
    let mut buffer = request.buffer()?;
    buffer.set_shaping_limits(domain.glyph_budget(&request)?, MAX_SHAPING_OPERATIONS);
    let features: Vec<_> = request
        .features
        .iter()
        .map(|feature| harfrust::Feature {
            tag: harfrust::Tag::new(&feature.tag),
            value: feature.value,
            start: feature.start,
            end: feature.end,
        })
        .collect();
    let mut callbacks = PaintCallbacks {
        measure,
        cache,
        error: None,
        advance_limit: domain.advance_limit(),
        domain,
    };
    let shaped = data.shaper(font).build().shape(
        buffer,
        ShapeOptions::new()
            .scale_separate(Some((scale.x, scale.y)))
            .integer_scaling_rounding(rounding)
            .features(&features)
            .font_funcs(Some(&mut callbacks)),
    );
    if let Some(error) = callbacks.error.take() {
        return Err(error);
    }
    if !shaped.shaping_succeeded() {
        return Err(PaintShapeError::ShapingBudget);
    }
    if shaped.len() > MAX_OUTPUT_GLYPHS {
        return Err(PaintShapeError::InvalidOutput);
    }
    let owners: HashSet<_> = request.infos.iter().map(|info| info.owner_utf16).collect();
    let mut glyphs = Vec::with_capacity(shaped.len());
    for (info, position) in shaped.glyph_infos().iter().zip(shaped.glyph_positions()) {
        if !owners.contains(&info.cluster) {
            return Err(PaintShapeError::InvalidOutput);
        }
        let backend = callbacks.glyph(info.glyph_id)?;
        glyphs.push(PaintShapedGlyph {
            id: info.glyph_id,
            owner_utf16: info.cluster,
            x_advance: position.x_advance,
            y_advance: position.y_advance,
            x_offset: position.x_offset,
            y_offset: position.y_offset,
            unsafe_to_break: info.unsafe_to_break(),
            backend,
        });
    }
    let source_range_utf16 = match (request.infos.first(), request.infos.last()) {
        (Some(first), Some(last)) => {
            first.owner_utf16..last.owner_utf16 + last.character.len_utf16() as u32
        }
        _ => 0..0,
    };
    Ok(PaintShapedRun {
        source_utf16_length: request.source.encode_utf16().count() as u32,
        source_range_utf16,
        script: request.script,
        paint,
        scale,
        direction: request.direction,
        glyphs,
    })
}

impl PaintShapeRequest<'_> {
    fn buffer(self) -> Result<harfrust::UnicodeBuffer, PaintShapeError> {
        if self.source.len() > MAX_SOURCE_BYTES
            || self.infos.len() > MAX_INPUT_SCALARS
            || self.features.len() > MAX_FEATURES
            || self
                .pre_context
                .chars()
                .take(MAX_CONTEXT_SCALARS + 1)
                .count()
                > MAX_CONTEXT_SCALARS
            || self
                .post_context
                .chars()
                .take(MAX_CONTEXT_SCALARS + 1)
                .count()
                > MAX_CONTEXT_SCALARS
        {
            return Err(PaintShapeError::InputBudget);
        }
        let mut source = HashMap::new();
        let mut owner = 0_u32;
        for character in self.source.chars() {
            source.insert(owner, character);
            owner += character.len_utf16() as u32;
            if owner > MAX_SOURCE_UTF16 {
                return Err(PaintShapeError::InputBudget);
            }
        }
        let mut next = self.infos.first().map(|info| info.owner_utf16);
        for info in self.infos {
            if next != Some(info.owner_utf16)
                || source.get(&info.owner_utf16) != Some(&info.character)
            {
                return Err(PaintShapeError::InvalidSource);
            }
            next = Some(info.owner_utf16 + info.character.len_utf16() as u32);
        }
        if self
            .features
            .iter()
            .any(|feature| feature.start > feature.end)
        {
            return Err(PaintShapeError::InvalidProperties);
        }
        if !self.script.iter().all(u8::is_ascii_alphabetic) {
            return Err(PaintShapeError::InvalidProperties);
        }
        let script = harfrust::Script::from_iso15924_tag(harfrust::Tag::new(&self.script))
            .ok_or(PaintShapeError::InvalidProperties)?;
        let flags = harfrust::BufferFlags::from_bits(self.flags)
            .ok_or(PaintShapeError::InvalidProperties)?;
        let mut buffer = harfrust::UnicodeBuffer::new();
        if !buffer.reserve(self.infos.len()) {
            return Err(PaintShapeError::InputBudget);
        }
        for info in self.infos {
            buffer.add(info.character, info.owner_utf16);
        }
        buffer.set_direction(match self.direction {
            PaintShapeDirection::LeftToRight => harfrust::Direction::LeftToRight,
            PaintShapeDirection::RightToLeft => harfrust::Direction::RightToLeft,
        });
        buffer.set_script(script);
        if let Some(language) = self.language {
            if language.len() > 64
                || language.is_empty()
                || !language
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
            {
                return Err(PaintShapeError::InvalidProperties);
            }
            buffer.set_language(
                language
                    .parse()
                    .map_err(|_| PaintShapeError::InvalidProperties)?,
            );
        }
        buffer.set_flags(flags);
        buffer.set_cluster_level(match self.cluster_level {
            PaintShapeClusterLevel::MonotoneGraphemes => {
                harfrust::BufferClusterLevel::MonotoneGraphemes
            }
            PaintShapeClusterLevel::MonotoneCharacters => {
                harfrust::BufferClusterLevel::MonotoneCharacters
            }
            PaintShapeClusterLevel::Characters => harfrust::BufferClusterLevel::Characters,
        });
        buffer.set_pre_context(self.pre_context);
        buffer.set_post_context(self.post_context);
        Ok(buffer)
    }
}

struct PaintCallbacks<'a> {
    measure: &'a mut dyn FnMut(u32) -> Result<PaintGlyphMetrics, PaintShapeError>,
    cache: &'a mut HashMap<u32, PaintGlyphMetrics>,
    error: Option<PaintShapeError>,
    advance_limit: u32,
    domain: &'a PositioningDomain,
}

impl PaintCallbacks<'_> {
    fn glyph(&mut self, glyph: u32) -> Result<PaintGlyphMetrics, PaintShapeError> {
        if let Some(metrics) = self.cache.get(&glyph) {
            return Ok(*metrics);
        }
        if self.cache.len() >= MAX_OUTPUT_GLYPHS {
            return Err(PaintShapeError::InvalidOutput);
        }
        let metrics = (self.measure)(glyph)?;
        self.cache.insert(glyph, metrics);
        Ok(metrics)
    }

    fn advance(&mut self, glyph: GlyphId, scalar: bool) -> i32 {
        if self.error.is_some() {
            return 0;
        }
        let result = self.glyph(glyph.to_u32()).and_then(|metrics| {
            let advance = paint_advance(metrics.advance, scalar)?;
            if advance.unsigned_abs() > self.advance_limit {
                return Err(PaintShapeError::UnsupportedPositioningDomain);
            }
            Ok(advance)
        });
        match result {
            Ok(advance) => advance,
            Err(error) => {
                self.error = Some(error);
                0
            }
        }
    }

    fn unsupported(&mut self, callback: &'static str) {
        if self.error.is_none() {
            self.error = Some(PaintShapeError::UnsupportedCallback(callback));
        }
    }
}

impl FontFuncs for PaintCallbacks<'_> {
    fn validate_positioning(
        &mut self,
        infos: &[harfrust::GlyphInfo],
        positions: &[harfrust::GlyphPosition],
    ) -> bool {
        if self.error.is_some() {
            return false;
        }
        match self.domain.validate_positioning(infos, positions) {
            Ok(()) => true,
            Err(error) => {
                self.error = Some(error);
                false
            }
        }
    }

    fn advance_width(&mut self, _: &BuiltinFontFuncs, glyph: GlyphId) -> i32 {
        self.advance(glyph, true)
    }

    fn populate_advance_widths(&mut self, _: &BuiltinFontFuncs, batch: AdvanceWidthBatch<'_>) {
        for (glyph, advance) in batch {
            *advance = self.advance(glyph, false);
        }
    }

    fn advance_height(&mut self, _: &BuiltinFontFuncs, _: GlyphId) -> i32 {
        self.unsupported("vertical advance");
        0
    }

    fn vertical_origin(&mut self, _: &BuiltinFontFuncs, _: GlyphId) -> (i32, i32) {
        self.unsupported("vertical origin");
        (0, 0)
    }

    fn extents(&mut self, _: &BuiltinFontFuncs, _: GlyphId) -> Option<harfrust::GlyphExtents> {
        self.unsupported("glyph extents");
        None
    }
}

pub(super) fn paint_advance(raw_advance: f32, scalar: bool) -> Result<i32, PaintShapeError> {
    let scaled = raw_advance * 256.0;
    quantized_integer(f64::from(scaled) + if scalar { 0.5 } else { 0.0 })
        .map_err(|_| PaintShapeError::AdvanceRange)
}

fn quantized_integer(value: f64) -> Result<i32, PaintShapeError> {
    if !value.is_finite()
        || value.trunc() < f64::from(i32::MIN)
        || value.trunc() > f64::from(i32::MAX)
    {
        return Err(PaintShapeError::InvalidScale);
    }
    Ok(value as i32)
}

#[cfg(test)]
mod tests;
