use skrifa::instance::{LocationRef, Size};
use skrifa::metrics::GlyphMetrics;
use skrifa::outline::pen::{ControlBoundsPen, OutlinePen};
use skrifa::outline::{
    DrawSettings, Engine, HintingInstance, HintingOptions, OutlineGlyphCollection,
    OutlineGlyphFormat, SmoothMode,
};
use skrifa::raw::{TableProvider, tables::glyf::Glyph};
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
    #[error("paint metrics require horizontal scale one and finite skew")]
    UnsupportedTransform,
    #[error("paint font data or face index is invalid")]
    InvalidFont,
    #[error("paint metrics support static glyf fonts without bitmap or VARC tables")]
    UnsupportedFont,
    #[error("skewed composite glyph {glyph_id} is not supported")]
    UnsupportedComposite { glyph_id: u32 },
    #[error("glyph {glyph_id} is unavailable")]
    InvalidGlyph { glyph_id: u32 },
    #[error("paint font scaler failed: {0}")]
    Scaler(#[from] skrifa::outline::DrawError),
    #[error("glyph metrics exceed the bounded paint metric domain")]
    MetricRange,
}

/// Measured backend values in native paint units, before HarfBuzz positioning.
pub struct PaintMetrics<'font> {
    font: FontRef<'font>,
    outlines: OutlineGlyphCollection<'font>,
    design_metrics: GlyphMetrics<'font>,
    skewed: bool,
    hinting: Option<HintingInstance>,
    backend_size: f32,
    normalization: f32,
    transform: FixedTransform,
    linear_scale: i64,
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
        if input.scale_x != 1.0 || !input.skew_x.is_finite() {
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
        let scaled_skew = input.size * input.skew_x;
        let squared_size = input.size * input.size;
        let normalized = input.mode == PaintMetricMode::Linear
            || squared_size > 4_194_304.0
            || scaled_skew.mul_add(scaled_skew, squared_size) > 4_194_304.0;
        let backend_size = if normalized { 64.0 } else { input.size };
        let normalization = if normalized { input.size / 64.0 } else { 1.0 };
        let hinting = if input.mode == PaintMetricMode::Normal && !normalized && input.skew_x == 0.0
        {
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
        let units_per_em = i64::from(
            font.metrics(Size::unscaled(), LocationRef::default())
                .units_per_em,
        );
        if units_per_em == 0 {
            return Err(PaintMetricError::InvalidFont);
        }
        let linear_scale =
            (((backend_size * 64.0) as i64) * 65536 + units_per_em / 2) / units_per_em;
        let design_metrics = font.glyph_metrics(Size::unscaled(), LocationRef::default());
        Ok(Self {
            font,
            outlines,
            skewed: input.skew_x != 0.0,
            design_metrics,
            hinting,
            backend_size,
            normalization,
            transform: FixedTransform::new(backend_size, input.skew_x)?,
            linear_scale,
        })
    }

    pub fn glyph(&self, glyph_id: u32) -> Result<PaintGlyphMetrics, PaintMetricError> {
        if glyph_id >= self.design_metrics.glyph_count() {
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
        let adjusted = if self.skewed {
            for [x, y] in self.simple_points(glyph_id)? {
                let [x, y] = self.transform.point(x, y)?;
                pen.move_to(x as f32 / 64.0, y as f32 / 64.0);
            }
            None
        } else {
            Some(outline.draw(settings, &mut pen)?)
        };
        let advance = if self.hinting.is_some() {
            adjusted
                .and_then(|metrics| metrics.advance_width)
                .ok_or(PaintMetricError::MetricRange)?
        } else {
            let units = self
                .design_metrics
                .advance_width(glyph_id)
                .ok_or(PaintMetricError::MetricRange)? as i64;
            let fixed = i32::try_from((units * self.linear_scale + 32) / 64)
                .map_err(|_| PaintMetricError::MetricRange)?;
            ((i64::from(fixed) * i64::from(self.transform.xx)) >> 16) as i32 as f32 / 65536.0
        } * self.normalization;
        let ink = self.ink_bounds(&pen)?;
        if !advance.is_finite() {
            return Err(PaintMetricError::MetricRange);
        }
        Ok(PaintGlyphMetrics { advance, ink })
    }

    fn simple_points(&self, glyph_id: GlyphId) -> Result<Vec<[i64; 2]>, PaintMetricError> {
        let loca = self
            .font
            .loca(None)
            .map_err(|_| PaintMetricError::InvalidFont)?;
        let glyf = self
            .font
            .glyf()
            .map_err(|_| PaintMetricError::InvalidFont)?;
        let glyph = loca
            .get_glyf(glyph_id, &glyf)
            .map_err(|_| PaintMetricError::InvalidFont)?;
        let Some(glyph) = glyph else {
            return Ok(Vec::new());
        };
        let Glyph::Simple(glyph) = glyph else {
            return Err(PaintMetricError::UnsupportedComposite {
                glyph_id: glyph_id.to_u32(),
            });
        };
        let bearing = self
            .design_metrics
            .left_side_bearing(glyph_id)
            .ok_or(PaintMetricError::MetricRange)? as i64;
        let shift = mul_fix(i64::from(glyph.x_min()) - bearing, self.linear_scale);
        let points = glyph
            .points()
            .map(|point| {
                [
                    mul_fix(i64::from(point.x), self.linear_scale) - shift,
                    mul_fix(i64::from(point.y), self.linear_scale),
                ]
            })
            .collect::<Vec<_>>();
        if points.len() != glyph.num_points() {
            return Err(PaintMetricError::InvalidFont);
        }
        Ok(points)
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

#[derive(Clone, Copy)]
struct FixedTransform {
    xx: i32,
    xy: i32,
}

impl FixedTransform {
    fn new(size: f32, skew: f32) -> Result<Self, PaintMetricError> {
        let (diagonal, off_diagonal) = if skew == 0.0 {
            (1.0, 0.0)
        } else {
            let reciprocal = 1.0 / size;
            (size * reciprocal, (size * skew) * reciprocal)
        };
        let fixed = |value: f32| {
            let scaled = value * 65536.0;
            if !scaled.is_finite()
                || f64::from(scaled) < f64::from(i32::MIN)
                || f64::from(scaled) >= f64::from(i32::MAX) + 1.0
            {
                Err(PaintMetricError::MetricRange)
            } else {
                Ok(scaled as i32)
            }
        };
        Ok(Self {
            xx: fixed(diagonal)?,
            xy: fixed(-off_diagonal)?,
        })
    }

    fn point(self, x: i64, y: i64) -> Result<[i64; 2], PaintMetricError> {
        if x.abs() > i64::from(i32::MAX) || y.abs() > i64::from(i32::MAX) {
            return Err(PaintMetricError::MetricRange);
        }
        let transformed = [
            mul_fix(x, i64::from(self.xx)) + mul_fix(y, i64::from(self.xy)),
            mul_fix(y, i64::from(self.xx)),
        ];
        if transformed
            .iter()
            .any(|value| value.abs() > i64::from(i32::MAX))
        {
            return Err(PaintMetricError::MetricRange);
        }
        Ok(transformed)
    }
}

fn mul_fix(value: i64, matrix: i64) -> i64 {
    let product = value * matrix;
    (product + (product >> 63) + 32768) >> 16
}

#[cfg(test)]
mod tests;
