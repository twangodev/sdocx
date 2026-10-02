use skrifa::instance::{LocationRef, Size};
use skrifa::metrics::GlyphMetrics;
use skrifa::outline::pen::ControlBoundsPen;
use skrifa::outline::{
    DrawSettings, Engine, HintingInstance, HintingOptions, OutlineGlyphCollection,
    OutlineGlyphFormat, SmoothMode,
};
use skrifa::{FontRef, GlyphId, MetadataProvider, Tag};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaintMetricMode {
    Unhinted,
    Normal,
    Linear,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaintMetricInput {
    pub size: f32,
    pub scale_x: f32,
    pub skew_x: f32,
    pub mode: PaintMetricMode,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PaintInkBounds {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaintGlyphMetrics {
    pub advance: f32,
    pub ink: PaintInkBounds,
}

#[derive(Debug, thiserror::Error)]
pub enum PaintMetricError {
    #[error("paint size must be finite and in [1, 8388608)")]
    InvalidSize,
    #[error("paint metrics require horizontal scale one and skew zero")]
    UnsupportedTransform,
    #[error("paint font data or face index is invalid")]
    InvalidFont,
    #[error("paint metrics support static glyf fonts without bitmap or VARC tables")]
    UnsupportedFont,
    #[error("glyph {glyph_id} is unavailable")]
    InvalidGlyph { glyph_id: u32 },
    #[error("paint font scaler failed: {0}")]
    Scaler(#[from] skrifa::outline::DrawError),
    #[error("glyph metrics exceed the bounded paint metric domain")]
    MetricRange,
}

/// Measured backend values in native paint units, before HarfBuzz positioning.
pub struct PaintMetrics<'font> {
    outlines: OutlineGlyphCollection<'font>,
    linear_metrics: GlyphMetrics<'font>,
    hinting: Option<HintingInstance>,
    backend_size: f32,
    normalization: f32,
}

impl<'font> PaintMetrics<'font> {
    pub(super) fn new(
        bytes: &'font [u8],
        index: u32,
        input: PaintMetricInput,
    ) -> Result<Self, PaintMetricError> {
        if !input.size.is_finite() || !(1.0..8_388_608.0).contains(&input.size) {
            return Err(PaintMetricError::InvalidSize);
        }
        if input.scale_x != 1.0 || input.skew_x != 0.0 {
            return Err(PaintMetricError::UnsupportedTransform);
        }
        let font = FontRef::from_index(bytes, index).map_err(|_| PaintMetricError::InvalidFont)?;
        let outlines = font.outline_glyphs();
        if outlines.format() != Some(OutlineGlyphFormat::Glyf)
            || [b"fvar", b"CBDT", b"VARC"]
                .into_iter()
                .any(|tag| font.table_data(Tag::new(tag)).is_some())
        {
            return Err(PaintMetricError::UnsupportedFont);
        }
        let normalized = input.mode == PaintMetricMode::Linear || input.size > 2048.0;
        let backend_size = if normalized { 64.0 } else { input.size };
        let normalization = if normalized { input.size / 64.0 } else { 1.0 };
        let hinting = if input.mode == PaintMetricMode::Normal && !normalized {
            Some(HintingInstance::new(
                &outlines,
                Size::new(backend_size),
                LocationRef::default(),
                HintingOptions {
                    engine: Engine::AutoFallback,
                    target: SmoothMode::Normal.into(),
                },
            )?)
        } else {
            None
        };
        Ok(Self {
            outlines,
            linear_metrics: font.glyph_metrics(Size::new(backend_size), LocationRef::default()),
            hinting,
            backend_size,
            normalization,
        })
    }

    pub fn glyph(&self, glyph_id: u32) -> Result<PaintGlyphMetrics, PaintMetricError> {
        if glyph_id >= self.linear_metrics.glyph_count() {
            return Err(PaintMetricError::InvalidGlyph { glyph_id });
        }
        let glyph_id = GlyphId::new(glyph_id);
        let outline = self
            .outlines
            .get(glyph_id)
            .ok_or(PaintMetricError::InvalidGlyph {
                glyph_id: glyph_id.to_u32(),
            })?;
        let settings = match &self.hinting {
            Some(instance) => DrawSettings::hinted(instance, false),
            None => DrawSettings::unhinted(Size::new(self.backend_size), LocationRef::default()),
        };
        let mut pen = ControlBoundsPen::new();
        let adjusted = outline.draw(settings, &mut pen)?;
        let advance = if self.hinting.is_some() {
            adjusted.advance_width
        } else {
            self.linear_metrics.advance_width(glyph_id)
        }
        .ok_or(PaintMetricError::MetricRange)?
            * self.normalization;
        let ink = self.ink_bounds(&pen)?;
        if !advance.is_finite() {
            return Err(PaintMetricError::MetricRange);
        }
        Ok(PaintGlyphMetrics { advance, ink })
    }

    fn ink_bounds(&self, pen: &ControlBoundsPen) -> Result<PaintInkBounds, PaintMetricError> {
        let Some(bounds) = pen.bounding_box() else {
            return Ok(PaintInkBounds::default());
        };
        let left = bounds.x_min.floor();
        let top = (-bounds.y_max).floor();
        let right = bounds.x_max.ceil();
        let bottom = (-bounds.y_min).ceil();
        if [left, top, right, bottom]
            .into_iter()
            .any(|value| !value.is_finite())
            || left < f32::from(i16::MIN)
            || left > f32::from(i16::MAX)
            || top < f32::from(i16::MIN)
            || top > f32::from(i16::MAX)
            || !(0.0..=f32::from(u16::MAX)).contains(&(right - left))
            || !(0.0..=f32::from(u16::MAX)).contains(&(bottom - top))
        {
            return Err(PaintMetricError::MetricRange);
        }
        Ok(PaintInkBounds {
            left: (left as i32) as f32 * self.normalization,
            top: (top as i32) as f32 * self.normalization,
            right: (right as i32) as f32 * self.normalization,
            bottom: (bottom as i32) as f32 * self.normalization,
        })
    }
}

#[cfg(test)]
mod tests;
