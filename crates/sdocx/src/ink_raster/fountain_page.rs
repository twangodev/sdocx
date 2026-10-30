use super::{FountainV14Scene, InkRasterCanvas, InkRasterError};
use crate::{Page, Stroke};

impl InkRasterCanvas {
    /// Paint one explicitly selected saved-page stroke with the direct Fountain
    /// host profile. `stroke_index` indexes `Page::strokes()` in stored traversal
    /// order, which does not imply composition paint order. Object identity and
    /// revision are caller-defined cache keys, not native keys. Use distinct keys
    /// for live objects across pages; change the revision whenever the source
    /// changes or a key is retargeted. The caller owns the Drawing lifecycle,
    /// crop, paint color, backdrop and submission order. The direct host shader
    /// ignores the alpha component of `paint_argb`.
    /// Other page objects are not painted by this operation.
    ///
    /// Preparation commits scene state before paint; a paint failure leaves the
    /// canvas intact but does not roll back a newly prepared snapshot.
    #[allow(clippy::too_many_arguments)]
    pub fn paint_fountain_v14_page_stroke(
        &mut self,
        scene: &mut FountainV14Scene,
        page: &Page,
        stroke_index: usize,
        object: u64,
        revision: u64,
        source_origin: [i32; 2],
        paint_argb: u32,
    ) -> Result<(), InkRasterError> {
        let stroke = page
            .strokes()
            .nth(stroke_index)
            .ok_or(InkRasterError::InvalidStroke("saved page stroke index"))?;
        self.paint_fountain_v14_stroke(scene, stroke, object, revision, source_origin, paint_argb)
    }

    /// Paint a caller-selected stroke into the existing canvas. This accepts a
    /// stroke obtained while traversing mixed page objects, so the caller can
    /// interleave other painting in its chosen composition order. It does not
    /// select objects, determine their visibility, or paint non-stroke content.
    ///
    /// Object identity, revision, Drawing lifecycle, crop, color, backdrop,
    /// and submission order have the same caller-owned contract as
    /// [`Self::paint_fountain_v14_page_stroke`]. Preparation commits scene
    /// state before paint; a paint failure does not roll it back.
    pub fn paint_fountain_v14_stroke(
        &mut self,
        scene: &mut FountainV14Scene,
        stroke: &Stroke,
        object: u64,
        revision: u64,
        source_origin: [i32; 2],
        paint_argb: u32,
    ) -> Result<(), InkRasterError> {
        let prepared = scene.prepare(object, revision, stroke, self.context())?;
        self.paint_fountain_v14_prepared(prepared, source_origin, paint_argb)
    }
}
