use std::ops::Range;

use super::{
    MAX_FEATURES, MAX_INPUT_SCALARS, MAX_OUTPUT_GLYPHS, PaintShapeClusterLevel,
    PaintShapeDirection, PaintShapeError, PaintShapeFeature, PaintShapeRequest, PaintShapeScale,
    PaintShapedRun, PaintShaper,
};
use crate::render::fonts::{
    PaintEntryError, PaintEntryGeometry, PaintItemization, PaintLayout, PaintLayoutError,
    PaintMetricInput, PaintScriptChunk,
};

const LIGATURE_SPACING_THRESHOLD: f64 = 0.03;
const MAX_PIECE_CHUNKS: usize = 128;
const DISABLED_LIGATURES: [PaintShapeFeature; 2] = [
    PaintShapeFeature {
        tag: *b"liga",
        value: 0,
        start: 0,
        end: u32::MAX,
    },
    PaintShapeFeature {
        tag: *b"clig",
        value: 0,
        start: 0,
        end: u32::MAX,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaintFeatureProfile {
    Default,
    DisableLigatures,
}

impl PaintFeatureProfile {
    pub fn for_script(script: [u8; 4], letter_spacing: f32) -> Result<Self, PaintPieceError> {
        if !letter_spacing.is_finite() {
            return Err(PaintLayoutError::NonFinitePaint.into());
        }
        Ok(
            if script == *b"Latn" || f64::from(letter_spacing.abs()) > LIGATURE_SPACING_THRESHOLD {
                Self::DisableLigatures
            } else {
                Self::Default
            },
        )
    }

    pub fn features(self) -> &'static [PaintShapeFeature] {
        match self {
            Self::Default => &[],
            Self::DisableLigatures => &DISABLED_LIGATURES,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PaintTextRequest<'a> {
    pub itemization: &'a PaintItemization,
    pub direction: PaintShapeDirection,
    pub letter_spacing: f32,
    pub word_spacing: f32,
    pub features: &'a [PaintShapeFeature],
}

impl<'a> PaintTextRequest<'a> {
    pub fn new(itemization: &'a PaintItemization, direction: PaintShapeDirection) -> Self {
        Self {
            itemization,
            direction,
            letter_spacing: 0.0,
            word_spacing: 0.0,
            features: &[],
        }
    }

    pub(super) fn chunk_features(
        &self,
        chunk: &PaintScriptChunk,
    ) -> Result<Vec<PaintShapeFeature>, PaintPieceError> {
        let profile = PaintFeatureProfile::for_script(chunk.script(), self.letter_spacing)?;
        Ok(self
            .features
            .iter()
            .chain(profile.features())
            .copied()
            .collect())
    }

    pub(super) fn chunk_request(
        &self,
        chunk: &'a PaintScriptChunk,
        features: &'a [PaintShapeFeature],
    ) -> PaintShapeRequest<'a> {
        PaintShapeRequest {
            source: self.itemization.source(),
            infos: chunk.infos(),
            direction: self.direction,
            script: chunk.script(),
            language: None,
            flags: 0,
            cluster_level: PaintShapeClusterLevel::MonotoneGraphemes,
            pre_context: chunk.pre_context(),
            post_context: chunk.post_context(),
            features,
        }
    }

    fn validate(&self) -> Result<(), PaintPieceError> {
        if self.features.len() > MAX_FEATURES - DISABLED_LIGATURES.len() {
            return Err(PaintPieceError::InputBudget);
        }
        if self
            .features
            .iter()
            .any(|feature| feature.start > feature.end)
        {
            return Err(PaintShapeError::InvalidProperties.into());
        }
        if !self.letter_spacing.is_finite() || !self.word_spacing.is_finite() {
            return Err(PaintLayoutError::NonFinitePaint.into());
        }
        if self.word_spacing != 0.0 {
            return Err(PaintLayoutError::UnsupportedPaint.into());
        }
        if self.itemization.chunks().is_empty() {
            return Err(PaintLayoutError::InvalidRange.into());
        }
        let scalar_count = self
            .itemization
            .chunks()
            .iter()
            .try_fold(0_usize, |count, chunk| {
                count.checked_add(chunk.infos().len())
            })
            .ok_or(PaintPieceError::InputBudget)?;
        if scalar_count > MAX_INPUT_SCALARS
            || self.itemization.chunks().len() > MAX_PIECE_CHUNKS
            || scalar_count.saturating_mul(2) > MAX_OUTPUT_GLYPHS
        {
            return Err(PaintPieceError::InputBudget);
        }
        Ok(())
    }
}

/// One font and paint authority for a contiguous itemized source range.
#[derive(Debug, Clone, PartialEq)]
pub struct PaintMeasuredPiece {
    shaped_runs: Vec<PaintShapedRun>,
    layout: PaintLayout,
    entry_geometry: PaintEntryGeometry,
}

impl PaintMeasuredPiece {
    pub fn source(&self) -> &str {
        self.layout.source()
    }
    pub fn source_range_utf16(&self) -> Range<u32> {
        self.layout.source_range_utf16()
    }
    pub fn paint(&self) -> PaintMetricInput {
        self.shaped_runs[0].paint()
    }
    pub fn scale(&self) -> PaintShapeScale {
        self.shaped_runs[0].scale()
    }
    pub fn direction(&self) -> PaintShapeDirection {
        self.shaped_runs[0].direction()
    }
    pub fn shaped_runs(&self) -> &[PaintShapedRun] {
        &self.shaped_runs
    }
    pub fn layout(&self) -> &PaintLayout {
        &self.layout
    }
    pub fn entry_geometry(&self) -> &PaintEntryGeometry {
        &self.entry_geometry
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PaintPieceError {
    #[error("paint piece exceeds its aggregate scalar, chunk, or glyph budget")]
    InputBudget,
    #[error(transparent)]
    Shape(#[from] PaintShapeError),
    #[error(transparent)]
    Layout(#[from] PaintLayoutError),
    #[error(transparent)]
    Entry(#[from] PaintEntryError),
}

impl PaintShaper<'_> {
    pub fn shape_text(
        &mut self,
        request: PaintTextRequest<'_>,
    ) -> Result<PaintMeasuredPiece, PaintPieceError> {
        request.validate()?;
        let mut shaped_runs = Vec::with_capacity(request.itemization.chunks().len());
        let mut glyph_count = 0_usize;
        for chunk in request.itemization.chunks() {
            let features = request.chunk_features(chunk)?;
            let run = self.shape(request.chunk_request(chunk, &features))?;
            glyph_count = glyph_count
                .checked_add(run.glyphs().len())
                .filter(|&count| count <= MAX_OUTPUT_GLYPHS)
                .ok_or(PaintPieceError::InputBudget)?;
            shaped_runs.push(run);
        }
        let runs: Vec<_> = shaped_runs.iter().collect();
        let layout = PaintLayout::from_runs(&runs, request.letter_spacing, request.word_spacing)?;
        let entry_geometry = layout.entry_geometry()?;
        Ok(PaintMeasuredPiece {
            shaped_runs,
            layout,
            entry_geometry,
        })
    }
}

#[cfg(test)]
mod tests;
