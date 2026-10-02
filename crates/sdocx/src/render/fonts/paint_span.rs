use super::{PaintMetricInput, PaintMetricMode};

/// Scalar paint parameters from the native span measurement helper.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaintSpanProfile {
    size: f32,
    underline: bool,
    fake_bold: bool,
    skew_x: f32,
}

#[derive(Debug, thiserror::Error, Clone, Copy, PartialEq, Eq)]
pub enum PaintSpanError {
    #[error("span font size must be finite and positive")]
    InvalidSourceSize,
    #[error("span font size produced nonfinite native paint size")]
    NonFinitePaintSize,
    #[error("span paint size is outside the supported metric domain")]
    UnsupportedPaintSize,
    #[error("span measurement requires unsupported synthetic bold metrics")]
    UnsupportedFakeBold,
}

impl PaintSpanProfile {
    pub fn new(font_size: f32, source_style_bits: u8) -> Result<Self, PaintSpanError> {
        if !font_size.is_finite() || font_size <= 0.0 {
            return Err(PaintSpanError::InvalidSourceSize);
        }
        let size = font_size * 100.0_f32;
        if !size.is_finite() {
            return Err(PaintSpanError::NonFinitePaintSize);
        }
        Ok(Self {
            size,
            underline: source_style_bits & 1 != 0,
            fake_bold: source_style_bits & 2 != 0,
            skew_x: if source_style_bits & 4 != 0 {
                -0.25
            } else {
                0.0
            },
        })
    }

    pub fn size(&self) -> f32 {
        self.size
    }
    pub fn scale_x(&self) -> f32 {
        1.0
    }
    pub fn skew_x(&self) -> f32 {
        self.skew_x
    }
    pub fn underline(&self) -> bool {
        self.underline
    }
    pub fn fake_bold(&self) -> bool {
        self.fake_bold
    }
    pub fn packed_flags(&self) -> u32 {
        0x20000 | if self.fake_bold { 0x20 } else { 0 }
    }

    pub fn metric_input(&self) -> Result<PaintMetricInput, PaintSpanError> {
        if self.fake_bold {
            return Err(PaintSpanError::UnsupportedFakeBold);
        }
        if !super::paint_metrics::SUPPORTED_SIZE.contains(&self.size) {
            return Err(PaintSpanError::UnsupportedPaintSize);
        }
        Ok(PaintMetricInput {
            size: self.size,
            scale_x: self.scale_x(),
            skew_x: self.skew_x,
            mode: PaintMetricMode::Normal,
        })
    }
}

#[cfg(test)]
mod tests;
