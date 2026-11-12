use super::marker::PointMarkerTarget;
use super::pencil3::Pencil3PaperSampler;

/// Color treatment to use while rendering a document.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum RenderColorMode {
    /// Infer the color treatment from the page or document background.
    #[default]
    Auto,
    /// Use light-canvas defaults.
    Light,
    /// Use dark-canvas defaults.
    Dark,
}

/// Host display dimensions for the opt-in saved InkPen V4 reconstruction.
/// This profile assumes fresh drawable state and an identity page transform.
/// Its one-source-unit contour approximates native coverage; device and
/// arbitrary output-scale parity are not established.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InkPenRenderProfile {
    pub screen_width: u32,
    pub screen_height: u32,
}

/// Explicit host policy for fresh-draw InkPen2 V9 vector reconstruction.
///
/// Selecting this profile opts into the captured InkPen2 V9 paint route with
/// identity page transform and fresh drawable state. Screen dimensions provide
/// the native resolution input. `enhanced_antialias` selects the captured
/// MAX-coverage paint path; this is an explicit host choice, not a claim
/// about Samsung Notes' device policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InkPen2RenderProfile {
    pub screen_width: u32,
    pub screen_height: u32,
    /// Select the captured enhanced antialiasing paint route.
    #[cfg_attr(feature = "serde", serde(default))]
    pub enhanced_antialias: bool,
}

/// Explicit output scale for the experimental InkPen2 V1 vector approximation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum InkPen2V1OutputScale {
    Half,
    One,
    Double,
}

impl InkPen2V1OutputScale {
    pub(super) fn value(self) -> f32 {
        match self {
            Self::Half => 0.5,
            Self::One => 1.0,
            Self::Double => 2.0,
        }
    }
}

/// Caller assertion that the whole page is rasterized at 0.5 pixels per page
/// unit, with zero translation and a top-left output origin. The grid dimensions
/// must be exactly half the page dimensions. This host approximation is not
/// scalable and is supported only for unsliced, untransformed strokes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InkPen2HalfGridViewport {
    pub width: u32,
    pub height: u32,
}

/// Explicit opt-in to bounded saved AirBrush texture routes.
///
/// Both profiles require normal saved style flags. `SavedV1` requires tool 2 and
/// accepts null or `1;` advanced settings; admitted empty streams emit no paint.
/// `SavedV2` accepts tools 0, 1, 2 and 3 with empty or `2;` settings.
/// Inputs beyond captured synthetic host controls are extrapolated.
/// V1 replay groups follow synthetic
/// redraw call boundaries; native progressive display is unverified.
/// One synthetic V1 host-Mesa frame matches the public SVG; device and PDF
/// parity and arbitrary output transforms remain unverified. Stamp angles use
/// pinned Rust libm, so Android sincosf parity is not claimed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AirBrushRenderProfile {
    /// Empty or `2;` saved setting for tools 0, 1, 2 and 3.
    SavedV2,
    /// Null saved setting selecting native AirBrush GLV1.
    SavedV1,
}

/// Opt into the experimental SimpleBrush vector approximation based on one
/// synthetic host-Mesa V25 capture. Saved-note, device, output-scale and PDF
/// parity are not established; unsupported strokes retain generic paint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SimpleBrushRenderProfile {
    /// Canonical page-unit approximation of the V25 capture.
    HostV25,
}

/// Explicit canonical canvas policy for fresh Pencil3 V3 reconstruction.
///
/// Screen dimensions determine native paper scale; canvas dimensions and the
/// supplied-paper sampler are caller inputs, not inferred Samsung app defaults.
/// The canvas must match the exported page. This route retains intrinsic brush
/// and paper textures with vector stamp geometry and does not select replay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Pencil3RenderProfile {
    pub screen_width: u32,
    pub screen_height: u32,
    pub canvas_width: u32,
    pub canvas_height: u32,
    pub paper_sampler: Pencil3PaperSampler,
}

/// Explicit source-device dimensions used to reconstruct ChineseBrush V1–V3 dabs.
/// This opts into approximate vector paint with intrinsic atlas alpha. It uses
/// hard-circle coverage unless `RenderOptions::chinesebrush_fwidth_output_profile`
/// supplies an explicit output scale. Dimensions are never inferred from the
/// page or consumer viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChineseBrushRenderProfile {
    pub screen_width: u32,
    pub screen_height: u32,
}

/// Explicit uniform output scale for ChineseBrush screen-space coverage.
/// The caller must use a uniform scale plus translation from page units to
/// output pixels. This is never inferred from source-device or page dimensions.
/// Equality uses the exact `f32` bit pattern; nonfinite and nonpositive values
/// are invalid for rendering and trigger generic whole-stroke fallback. Thus
/// equal numeric values with different bits (including signed zero or NaNs)
/// remain distinct option values.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChineseBrushFwidthOutputProfile {
    pub pixels_per_page_unit: f32,
}
impl PartialEq for ChineseBrushFwidthOutputProfile {
    fn eq(&self, other: &Self) -> bool {
        self.pixels_per_page_unit.to_bits() == other.pixels_per_page_unit.to_bits()
    }
}
impl Eq for ChineseBrushFwidthOutputProfile {}

/// Experimental host-selected reconstruction of native Fountain GLV17 rainbow paint.
/// It accepts bounded AS18 stylus variable-width inputs with an explicit palette
/// and phase. The pinned standard native factory selects GLV16 for AS18; these
/// class-level GLV17 controls do not establish that saved-style selection.
/// Unsupported metadata or invalid values use generic paint.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FountainV17RainbowRenderProfile {
    pub palette_argb: Vec<u32>,
    pub period: i32,
    pub initial_offset: f32,
}
impl PartialEq for FountainV17RainbowRenderProfile {
    fn eq(&self, other: &Self) -> bool {
        self.palette_argb == other.palette_argb
            && self.period == other.period
            && self.initial_offset.to_bits() == other.initial_offset.to_bits()
    }
}
impl Eq for FountainV17RainbowRenderProfile {}

/// Explicit uniform output scale for the Fountain V17 output-profile route.
/// The caller must apply this same scale, plus translation, from page units to
/// output pixels. The renderer never infers a scale from page or viewport size.
/// This option affects only strokes admitted by an explicit
/// [`FountainV17RainbowRenderProfile`]; other Fountain paths ignore it.
/// Nonfinite, nonpositive, or unrepresentable derived values use generic
/// whole-stroke fallback before SVG/PDF markers are registered.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FountainV17OutputScaleProfile {
    /// Uniform output pixels per page unit.
    pub pixels_per_page_unit: f32,
}
impl PartialEq for FountainV17OutputScaleProfile {
    fn eq(&self, other: &Self) -> bool {
        self.pixels_per_page_unit.to_bits() == other.pixels_per_page_unit.to_bits()
    }
}
impl Eq for FountainV17OutputScaleProfile {}

/// Explicit uniform output scale for saved Fountain V16, including its
/// supported AS18 rainbow-flag-ignored route.
/// The caller applies this same scale, plus translation, from page units to
/// output pixels; the renderer never infers it from the viewport. `None` keeps
/// the canonical unit-scale sampling and paint. Invalid scales preserve the
/// existing canonical V16 output through whole-stroke fallback.
/// An explicit V17 rainbow profile retains its existing priority and behavior.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FountainV16OutputScaleProfile {
    /// Uniform output pixels per page unit.
    pub pixels_per_page_unit: f32,
}
impl PartialEq for FountainV16OutputScaleProfile {
    fn eq(&self, other: &Self) -> bool {
        self.pixels_per_page_unit.to_bits() == other.pixels_per_page_unit.to_bits()
    }
}
impl Eq for FountainV16OutputScaleProfile {}

/// Explicit uniform output scale for V14 Fountain source sampling and edge coverage.
///
/// The caller must apply this same scale, plus translation, from page units to
/// output pixels. The renderer does not infer scale from the page or viewport.
/// `None`, nonfinite, nonpositive, or unrepresentable scales preserve the
/// existing canonical V14 vector paint. The scaled route is bounded and falls
/// back to that same canonical paint when its derived native geometry exceeds
/// its supported limits.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FountainV14OutputScaleProfile {
    /// Uniform output pixels per page unit.
    pub pixels_per_page_unit: f32,
}
impl PartialEq for FountainV14OutputScaleProfile {
    fn eq(&self, other: &Self) -> bool {
        self.pixels_per_page_unit.to_bits() == other.pixels_per_page_unit.to_bits()
    }
}
impl Eq for FountainV14OutputScaleProfile {}

/// Experimental Marker centerline approximation with explicit native PenData resolution.
///
/// Use only for fresh saved-stroke drawing in identity page coordinates. The
/// round polyline approximates the captured centerline; native textured caps,
/// depth, fringe and replay are not reconstructed. Unsupported inputs fall back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LegacyMarkerCenterlineProfile {
    /// Width supplied to native Pen::SetScreenResolution.
    pub screen_width: u32,
    /// Height supplied to native Pen::SetScreenResolution.
    pub screen_height: u32,
}

/// Experimental Montblanc approximation. The caller asserts a fresh drawable
/// and the stated output pixels per page unit. The model extrapolates from synthetic
/// host controls; saved-note and device parity are unverified. Unsupported
/// inputs, replay, and sliced output retain generic paint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum MontblancRenderProfile {
    /// Approximate captured host paint under the declared unit-scale context.
    HostUnitScale,
}

/// Assert a known uniform output scale for experimental DefaultPen edge paint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DefaultPenOutputScale {
    Half,
    One,
    Two,
    Four,
    Eight,
}

/// Assert the uniform output scale used for the experimental Marker3 V5 fringe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Marker3OutputScale {
    Half,
    One,
    Two,
    Four,
    Eight,
}

/// Explicit fixed-grid Crayon2 host context for a page-sized, unit-pixel viewport.
/// Textures, phase tables, projection, and effective color must belong to the
/// same captured host context. RGB must match the effective themed stroke color;
/// the caller asserts the effective opacity. This profile is resolution-specific.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Crayon2DocumentGridProfile {
    pub phase_x: Vec<u32>,
    pub phase_y: Vec<u32>,
    pub point_atlas: Vec<u8>,
    pub background_texture: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub clip_scale: [f32; 2],
    pub clip_translation: [f32; 2],
    pub background_scale: [f32; 2],
    pub rgba: [f32; 4],
    /// Total sample visits permitted across the page, capped at 100,000.
    pub max_samples: u64,
}
impl PartialEq for Crayon2DocumentGridProfile {
    fn eq(&self, other: &Self) -> bool {
        self.phase_x == other.phase_x
            && self.phase_y == other.phase_y
            && self.point_atlas == other.point_atlas
            && self.background_texture == other.background_texture
            && self.width == other.width
            && self.height == other.height
            && self.max_samples == other.max_samples
            && self
                .clip_scale
                .iter()
                .chain(&self.clip_translation)
                .chain(&self.background_scale)
                .chain(&self.rgba)
                .map(|x| x.to_bits())
                .eq(other
                    .clip_scale
                    .iter()
                    .chain(&other.clip_translation)
                    .chain(&other.background_scale)
                    .chain(&other.rgba)
                    .map(|x| x.to_bits()))
    }
}
impl Eq for Crayon2DocumentGridProfile {}

/// Options controlling SVG rendering.

#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct RenderOptions {
    /// Opt into the explicit Crayon2 host grid for a unit-pixel page viewport.
    #[cfg_attr(feature = "serde", serde(default))]
    pub crayon2_grid_profile: Option<Crayon2DocumentGridProfile>,
    /// Caller asserts Pencil identity, normal stylus context, and 1920 by 1080 unit-scale output. Experimental SVG only.
    #[cfg_attr(feature = "serde", serde(default))]
    pub old_pencil_svg_profile: bool,
    /// Opt into experimental scalable legacy Pencil PDF Forms, asserting the
    /// same host source and 1920×1080 canvas context as the SVG profile.
    /// Synthetic capture comparisons remain approximate and reader-dependent.
    #[cfg(feature = "pdf")]
    #[cfg_attr(feature = "serde", serde(default))]
    pub old_pencil_pdf_profile: bool,
    /// Opt into saved InkPen V4 reconstruction with explicit display dimensions.
    #[cfg_attr(feature = "serde", serde(default))]
    pub inkpen_profile: Option<InkPenRenderProfile>,
    /// Opt into scale-aware V4 coverage at a tested uniform output scale (1 through 8).
    #[cfg_attr(feature = "serde", serde(default))]
    pub inkpen_v4_output_scale: Option<u32>,
    /// Opt into scale-aware V1 coverage at a tested uniform output scale (1 through 8).
    #[cfg_attr(feature = "serde", serde(default))]
    pub inkpen_v1_output_scale: Option<u32>,
    /// Explicit InkPen2 V9 paint profile. `None` preserves fallback behavior;
    /// `Some` opts into fresh state and identity page transform; the profile
    /// explicitly selects its enhanced-antialiasing paint route.
    #[cfg_attr(feature = "serde", serde(default))]
    pub inkpen2_profile: Option<InkPen2RenderProfile>,
    /// Opt into the bounded V1 approximation at a known output scale.
    #[cfg_attr(feature = "serde", serde(default))]
    pub inkpen2_v1_output_scale: Option<InkPen2V1OutputScale>,
    /// Opt into the experimental V1 host pixel grid at the asserted viewport.
    #[cfg_attr(feature = "serde", serde(default))]
    pub inkpen2_half_grid_viewport: Option<InkPen2HalfGridViewport>,
    #[cfg(test)]
    #[cfg_attr(feature = "serde", serde(skip))]
    pub(crate) inkpen2_half_grid_test_budget: Option<[usize; 3]>,
    /// Opt into bounded, approximate ChineseBrush V1–V3 vector paint.
    #[cfg_attr(feature = "serde", serde(default))]
    pub chinesebrush_profile: Option<ChineseBrushRenderProfile>,
    /// Opt into a bounded approximation of the native screen-space fringe.
    /// Invalid explicit values fall back to generic rendering for the whole stroke.
    #[cfg_attr(feature = "serde", serde(default))]
    pub chinesebrush_fwidth_output_profile: Option<ChineseBrushFwidthOutputProfile>,
    /// Experimental host palette and phase for the direct Fountain GLV17 route.
    /// `None` retains the existing Fountain paint path.
    #[cfg_attr(feature = "serde", serde(default))]
    pub fountain_v17_rainbow_profile: Option<FountainV17RainbowRenderProfile>,
    /// Optional output-scale-aware coverage for the explicit Fountain V17
    /// rainbow route. `None` preserves canonical page-unit geometry.
    #[cfg_attr(feature = "serde", serde(default))]
    pub fountain_v17_output_scale_profile: Option<FountainV17OutputScaleProfile>,
    /// Optional scale-aware sampling and coverage for non-rainbow saved Fountain V16.
    /// `None` preserves the canonical unit-scale path.
    #[cfg_attr(feature = "serde", serde(default))]
    pub fountain_v16_output_scale_profile: Option<FountainV16OutputScaleProfile>,
    /// Optional scale-aware source sampling and edge coverage for saved Fountain V14.
    /// Invalid or absent profiles preserve the existing V14 vector output.
    #[cfg_attr(feature = "serde", serde(default))]
    pub fountain_v14_output_scale_profile: Option<FountainV14OutputScaleProfile>,
    /// Experimental DefaultPen edge approximation at a known uniform output scale.
    /// The caller must assert the actual output pixels per page unit. Replay and
    /// sliced scenes retain the canonical paint.
    #[cfg_attr(feature = "serde", serde(default))]
    pub defaultpen_output_scale: Option<DefaultPenOutputScale>,
    /// Experimental Marker3 V5 fringe for a caller-asserted uniform output scale.
    /// Replay and sliced scenes preserve the canonical route.
    #[cfg_attr(feature = "serde", serde(default))]
    pub marker3_output_scale: Option<Marker3OutputScale>,
    /// Experimental Marker2 fringe for an axis-aligned 2x output grid.
    /// Viewer zoom or other transforms change its coverage.
    #[cfg_attr(feature = "serde", serde(default))]
    pub marker2_output_scale: Option<u32>,
    /// Experimental MagicPen contour for a caller-asserted uniform output
    /// scale in pixels per page unit (1, 2, or 4), with an axis-aligned
    /// output grid and no rotation. Other values, replay, and sliced scenes
    /// use the canonical contour. Viewer zoom changes coverage.
    #[cfg_attr(feature = "serde", serde(default))]
    pub magicpen_output_scale: Option<u32>,
    /// Opt in to host-derived minimum-dot cells at 1x. The caller asserts
    /// the page origin maps to the output pixel origin with even 2x2 quad
    /// phase, without translation, rotation, or later viewer scaling.
    /// Requires `magicpen_output_scale: Some(1)`.
    #[cfg_attr(feature = "serde", serde(default))]
    pub magicpen_host_minimum_dot_grid: bool,
    /// Explicit fresh Pencil3 V3 canvas and intrinsic texture policy.
    #[cfg_attr(feature = "serde", serde(default))]
    pub pencil3_profile: Option<Pencil3RenderProfile>,
    /// Explicit opt-in to a bounded saved AirBrush intrinsic-texture route.
    #[cfg_attr(feature = "serde", serde(default))]
    pub airbrush_profile: Option<AirBrushRenderProfile>,
    /// Experimental SimpleBrush host-capture approximation; replay uses generic paint.
    #[cfg_attr(feature = "serde", serde(default))]
    pub simplebrush_profile: Option<SimpleBrushRenderProfile>,
    /// Assert two output pixels per page unit for the experimental SimpleBrush route.
    /// Combining this assertion with `simplebrush_profile` falls back to generic paint.
    #[cfg_attr(feature = "serde", serde(default))]
    pub simplebrush_two_pixels_per_page_unit: bool,
    /// Opt into experimental rotated Arc sectors; numeric agreement with Android is unverified.
    #[cfg_attr(feature = "serde", serde(default))]
    pub rotated_arc_sector_approximation: bool,
    /// Opt into approximate fresh-state Montblanc paint at unit output scale.
    #[cfg_attr(feature = "serde", serde(default))]
    pub montblanc_profile: Option<MontblancRenderProfile>,
    /// Assert a fresh drawable and two output pixels per page unit for Montblanc.
    /// For 10–11 points, sizes 0.5–2 are admitted; for 12–20 points, only size 1.5 is admitted.
    /// Combining this with a Montblanc profile, or using unsupported inputs,
    /// falls back to generic paint.
    #[cfg_attr(feature = "serde", serde(default))]
    pub montblanc_two_pixels_per_page_unit: bool,
    /// Opt into the experimental saved Marker centerline approximation.
    #[cfg_attr(feature = "serde", serde(default))]
    pub legacy_marker_centerline_profile: Option<LegacyMarkerCenterlineProfile>,
    /// Opt into experimental fresh Marker4 V7 finger paint at an asserted axis-aligned
    /// output scale of 1, 2, or 4 pixels per page unit. The scale selects the same approximation;
    /// it does not transform the output. Stroke color must match saved ARGB RGB.
    /// Unsupported inputs use generic paint.
    #[cfg_attr(feature = "serde", serde(default))]
    pub marker4_v7_finger_output_scale: Option<u32>,
    /// Opt into experimental bounded Marker vector paint with explicit native screen dimensions.
    #[cfg(feature = "pdf")]
    #[cfg_attr(feature = "serde", serde(default))]
    pub legacy_marker_vector_profile: Option<LegacyMarkerCenterlineProfile>,
    /// Color treatment for page backgrounds, ink, and compatibility text.
    pub color_mode: RenderColorMode,
    /// Native display profile used to size point-list artwork.
    #[cfg_attr(feature = "serde", serde(default))]
    pub point_marker_target: PointMarkerTarget,
}
