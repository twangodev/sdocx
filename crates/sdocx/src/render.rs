//! Presentation-oriented Svg rendering for parsed Samsung Notes documents.

use crate::progress::WorkProgress;
use crate::{Progress, ProgressStage};

use crate::{
    BoundingBox, Color, Document, LayoutDocument, MediaAsset, Page, PageElement,
    ParagraphAlignment, PlacedImage, PredefinedTextStyle, RichTextBox, RichTextObjectContent,
    RichTextObjectSpan, Stroke, layout_document,
};
use crate::{PageObject, PageObjectContent, composition::RenderPass};
use std::borrow::Cow;
use std::cell::Cell;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;
use vector::{
    Anchor, Blend, Circle, Clip, ClipPath, Data, Definitions, Ellipse, FontFamily, Group, Image,
    Line, LineCap, LineJoin, ObjectKind, PageTemplate, Paint, Path, Polygon, Rectangle, ReplayPart,
    ReplayPath, Scene, StrokeIndex, Styled, Svg, TSpan, Text, TextAnchor, TextDecoration,
    Transform, ViewBox, color_hex, coordinate, decimal,
};

mod airbrush;
mod arrows;
mod calligraphy;
mod code;
mod embedded;
pub mod fonts;
mod fountain;
mod geometry;
mod gradient;
mod image_crop;
mod inkpen2_vector;
mod simplebrush;
#[cfg(test)]
mod simplebrush_tests;
pub use geometry::{GeometryDiagnostic, GeometryDiagnosticKind};
mod pencil3;
pub use pencil3::Pencil3PaperSampler;
mod paint_diagnostic;
pub use paint_diagnostic::{PaintDiagnostic, PaintDiagnosticKind, PaintRole};
#[allow(clippy::all, dead_code, unused_imports, unexpected_cfgs)]
mod harfrust;
#[cfg(all(test, feature = "serde"))]
mod inkpen2;
mod marker;
#[cfg(all(test, feature = "serde"))]
mod marker3;
#[allow(dead_code)]
mod oblique;
pub use marker::PointMarkerTarget;
#[cfg(feature = "pdf")]
mod body_query;
mod native_body_pages;
#[cfg(feature = "pdf")]
pub(crate) use body_query::{BodyQueryObservation, ContinuousBodyQuerySnapshot};
#[cfg(feature = "pdf")]
mod native_ink;
mod pdf_slice;
mod placed;
mod table;
mod text;
#[cfg(feature = "pdf")]
pub(crate) use native_ink::{NativeInkPayload, NativeInkRegistry};
#[cfg(feature = "pdf")]
pub(crate) use text::native::{NativePdfPainter, NativeTextRegistry};
mod theme;
mod viewport;
#[cfg(test)]
use text::sanitize_hyperlink_target;
pub use text::{ObjectDiagnostic, ObjectDiagnosticKind, TextDiagnostic, TextDiagnosticKind};
use text::{StyledText, TextContext, TextRenderer, TextSettings, TextStyle, render_measured_line};
pub use theme::RenderTheme;
#[cfg(test)]
use theme::is_dark_background;
use viewport::Viewport;
mod chinesebrush;
#[cfg(test)]
mod chinesebrush_native_fringe_test;
#[cfg(all(test, feature = "serde"))]
mod native_table_cache_tests;
#[cfg(all(test, feature = "serde"))]
mod native_table_clip_tests;
mod vector;

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

/// Explicit opt-in to bounded saved AirBrush texture routes.
///
/// Both profiles require tool type 2 and normal saved style flags. `SavedV1`
/// accepts a null advanced setting; empty stamp streams use generic rendering.
/// `SavedV2` accepts empty or `2;` settings. V1 replay groups follow synthetic
/// redraw call boundaries; native progressive display is unverified.
/// One synthetic V1 host-Mesa frame matches the public SVG; device and PDF
/// parity and arbitrary output transforms remain unverified. Stamp angles use
/// pinned Rust libm, so Android sincosf parity is not claimed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AirBrushRenderProfile {
    /// Empty or `2;` saved setting selecting native AirBrush GLV2.
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

/// Explicit uniform output scale for V14 Fountain edge coverage.
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

/// Options controlling Svg rendering.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct RenderOptions {
    /// Opt into saved InkPen V4 reconstruction with explicit display dimensions.
    #[cfg_attr(feature = "serde", serde(default))]
    pub inkpen_profile: Option<InkPenRenderProfile>,
    /// Explicit InkPen2 V9 paint profile. `None` preserves fallback behavior;
    /// `Some` opts into fresh state and identity page transform; the profile
    /// explicitly selects its enhanced-antialiasing paint route.
    #[cfg_attr(feature = "serde", serde(default))]
    pub inkpen2_profile: Option<InkPen2RenderProfile>,
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
    /// Optional scale-aware edge coverage for the saved Fountain V14 path.
    /// Invalid or absent profiles preserve the existing V14 vector output.
    #[cfg_attr(feature = "serde", serde(default))]
    pub fountain_v14_output_scale_profile: Option<FountainV14OutputScaleProfile>,
    /// Explicit fresh Pencil3 V3 canvas and intrinsic texture policy.
    #[cfg_attr(feature = "serde", serde(default))]
    pub pencil3_profile: Option<Pencil3RenderProfile>,
    /// Explicit opt-in to a bounded saved AirBrush intrinsic-texture route.
    #[cfg_attr(feature = "serde", serde(default))]
    pub airbrush_profile: Option<AirBrushRenderProfile>,
    /// Experimental SimpleBrush host-capture approximation; replay uses generic paint.
    #[cfg_attr(feature = "serde", serde(default))]
    pub simplebrush_profile: Option<SimpleBrushRenderProfile>,
    /// Color treatment for page backgrounds, ink, and compatibility text.
    pub color_mode: RenderColorMode,
    /// Native display profile used to size point-list artwork.
    #[cfg_attr(feature = "serde", serde(default))]
    pub point_marker_target: PointMarkerTarget,
}

/// A page background retained in the document but omitted from SVG rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "kind", rename_all = "snake_case"))]
#[non_exhaustive]
pub enum BackgroundDiagnostic {
    /// Imported PDF paper is not yet painted by the SVG preview renderer.
    PdfPaperOmitted { record_count: usize },
}

/// One visible page rendered as a standalone Svg document.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct RenderedPage {
    /// Index of the backing page in the parsed document.
    pub source_page_index: usize,
    /// Page width in Samsung Notes coordinates.
    pub width: u32,
    /// Page height in Samsung Notes coordinates.
    pub height: u32,
    /// Standalone Svg markup.
    pub svg: String,
    /// Font resolution and glyph coverage limits encountered while rendering.
    #[cfg_attr(feature = "serde", serde(default))]
    pub text_diagnostics: Vec<TextDiagnostic>,
    /// Embedded-object anchors and composition that could not be laid out.
    #[cfg_attr(feature = "serde", serde(default))]
    pub object_diagnostics: Vec<ObjectDiagnostic>,
    /// Shape and line geometry not emitted during an admitted render attempt.
    #[cfg_attr(feature = "serde", serde(default))]
    pub geometry_diagnostics: Vec<GeometryDiagnostic>,
    /// Shape and line fill or outline components omitted during rendering.
    #[cfg_attr(feature = "serde", serde(default))]
    pub paint_diagnostics: Vec<PaintDiagnostic>,
    /// Retained page backgrounds that the SVG renderer cannot draw.
    #[cfg_attr(feature = "serde", serde(default))]
    pub background_diagnostics: Vec<BackgroundDiagnostic>,
}

impl RenderedPage {
    /// Construct a page with no render diagnostics.
    pub fn new(source_page_index: usize, width: u32, height: u32, svg: impl Into<String>) -> Self {
        Self {
            source_page_index,
            width,
            height,
            svg: svg.into(),
            text_diagnostics: Vec::new(),
            object_diagnostics: Vec::new(),
            geometry_diagnostics: Vec::new(),
            paint_diagnostics: Vec::new(),
            background_diagnostics: Vec::new(),
        }
    }
}

/// Reuses body-text measurement across pages without retaining rendered SVG.
/// Source, fonts, options and page geometry changes automatically invalidate plans.
#[derive(Default)]
pub struct DocumentTextCache {
    context: Option<BodyPreparationContext>,
    plans: Vec<(BodyPlanKey, Rc<PreparedBodyPlan>)>,
}

#[derive(PartialEq, Eq)]
struct BodyPreparationMetrics {
    scale: u32,
    font_size_delta: u32,
    default_page_dimensions: Option<(u32, u32)>,
    orientation: Option<i32>,
    page_mode: Option<u16>,
    padding: Option<(u32, u32)>,
    pages: Vec<(u32, u32)>,
    native_object_page_obstacles: Option<text::NativeObjectPageObstacles>,
    options: RenderOptions,
}

struct BodyPreparationContext {
    source: Option<RichTextBox>,
    metrics: BodyPreparationMetrics,
    fonts: Arc<fonts::fontdb::Database>,
    native_names: Option<fonts::NativeFontNameConfig>,
    native_registry: Option<fonts::RegisteredFontSource>,
}

#[derive(PartialEq, Eq)]
struct BodyPlanKey {
    source_range: Range<usize>,
    first_page_index: usize,
    last_page_index: usize,
    detached: bool,
    theme: RenderTheme,
}

const BODY_PLAN_CACHE_CAPACITY: usize = 2;

impl DocumentTextCache {
    /// Release retained body plans and their font references.
    pub fn clear(&mut self) {
        self.context = None;
        self.plans.clear();
    }

    /// Snapshot the bounded continuous-body line query over the same cached
    /// plan used by page rendering. Callers must pass their normalized scene
    /// document and page, if they render a normalized scene.
    #[cfg(feature = "pdf")]
    pub(crate) fn continuous_body_query_snapshot(
        &mut self,
        document: &Document,
        layout_page: &crate::LayoutPage,
        options: &RenderOptions,
        fonts: &fonts::FontBook,
    ) -> Option<ContinuousBodyQuerySnapshot> {
        let source_flow_width = document.metadata.flow_dimensions.map(|(width, _)| width);
        if document.metadata.page_mode != Some(1)
            || source_flow_width.is_none_or(|width| width == 0 || width != layout_page.page.width)
            || layout_page.single_page_continuous_body(document).is_none()
            || !document
                .metadata
                .note_text
                .as_ref()
                .is_some_and(|text| body_query::query_image_sources(text, document).is_some())
        {
            return None;
        }
        self.bind(document, options, fonts);
        let theme = RenderTheme::resolve(&layout_page.page, &document.metadata, options.color_mode);
        let renderer = page_text_renderer(document, layout_page, options, fonts);
        let body = PreparedBodyText::new(document, layout_page, theme, &renderer, self, None)?;
        ContinuousBodyQuerySnapshot::from_plan(&body.plan, document)
    }

    /// Render a visible page, sharing compatible body measurements with prior calls.
    pub fn render_layout_page_svg(
        &mut self,
        document: &Document,
        layout: &LayoutDocument,
        page_index: usize,
        options: &RenderOptions,
        fonts: &fonts::FontBook,
    ) -> Option<RenderedPage> {
        let page = layout.pages.get(page_index)?;
        Some(self.render_page(document, page, options, false, fonts))
    }

    /// Render the same vector scene while observing preparation and object composition.
    #[allow(clippy::too_many_arguments)]
    pub fn render_layout_page_svg_with_progress(
        &mut self,
        document: &Document,
        layout: &LayoutDocument,
        page_index: usize,
        options: &RenderOptions,
        fonts: &fonts::FontBook,
        observer: &mut dyn FnMut(Progress),
    ) -> Option<RenderedPage> {
        let page = layout.pages.get(page_index)?;
        observer(Progress::pending(ProgressStage::Preparing));
        self.bind(document, options, fonts);
        let (mut rendered, scene) =
            self.prepare_page(document, page, options, false, fonts, false, None, observer);
        observer(Progress::pending(ProgressStage::Finalizing));
        rendered.svg = scene.finish();
        Some(rendered)
    }

    /// Render sample-addressable replay using the same retained body measurements.
    pub fn render_layout_page_replay_svg(
        &mut self,
        document: &Document,
        layout: &LayoutDocument,
        page_index: usize,
        options: &RenderOptions,
        fonts: &fonts::FontBook,
    ) -> Option<RenderedPage> {
        let page = layout.pages.get(page_index)?;
        Some(self.render_page(document, page, options, true, fonts))
    }

    fn bind(&mut self, document: &Document, options: &RenderOptions, fonts: &fonts::FontBook) {
        let settings = TextSettings::from_document(&document.metadata);
        let metrics = BodyPreparationMetrics {
            scale: settings.scale.to_bits(),
            font_size_delta: settings.font_size_delta.to_bits(),
            default_page_dimensions: document.metadata.default_page_dimensions,
            orientation: document.metadata.orientation,
            page_mode: document.metadata.page_mode,
            padding: document.metadata.flow_page_padding,
            pages: document
                .pages
                .iter()
                .map(|page| (page.width, page.height))
                .collect(),
            native_object_page_obstacles: text::NativeObjectPageObstacles::from_document(document),
            options: options.clone(),
        };
        let database = fonts.database();
        let compatible = self.context.as_ref().is_some_and(|context| {
            context.metrics == metrics
                && Arc::ptr_eq(&context.fonts, &database)
                && context.native_names.as_ref() == fonts.native_name_config()
                && context.native_registry == fonts.native_registry_identity()
                && match (&context.source, &document.metadata.note_text) {
                    (Some(previous), Some(current)) => {
                        crate::layout::same_text_source(previous, current)
                    }
                    (None, None) => true,
                    _ => false,
                }
        });
        if !compatible {
            self.plans.clear();
            self.context = Some(BodyPreparationContext {
                source: document.metadata.note_text.clone(),
                metrics,
                fonts: database,
                native_names: fonts.native_name_config().cloned(),
                native_registry: fonts.native_registry_identity(),
            });
        }
    }
}

/// Render every visible page in presentation order.
pub fn render_document_svg(document: &Document, options: &RenderOptions) -> Vec<RenderedPage> {
    render_document_svg_with_fonts(document, options, &fonts::FontBook::default())
}

/// Render every visible page using a caller-controlled font database.
pub fn render_document_svg_with_fonts(
    document: &Document,
    options: &RenderOptions,
    fonts: &fonts::FontBook,
) -> Vec<RenderedPage> {
    let layout = layout_document(document);
    let mut cache = DocumentTextCache::default();
    cache.bind(document, options, fonts);
    layout
        .pages
        .iter()
        .map(|layout_page| cache.render_bound_page(document, layout_page, options, false, fonts))
        .collect()
}

/// Render one visible page by presentation index.
pub fn render_page_svg(
    document: &Document,
    page_index: usize,
    options: &RenderOptions,
) -> Option<RenderedPage> {
    let layout = layout_document(document);
    render_layout_page_svg(document, &layout, page_index, options)
}

/// Render one visible page from a precomputed presentation layout.
pub fn render_layout_page_svg(
    document: &Document,
    layout: &LayoutDocument,
    page_index: usize,
    options: &RenderOptions,
) -> Option<RenderedPage> {
    render_layout_page_svg_with_fonts(
        document,
        layout,
        page_index,
        options,
        &fonts::FontBook::default(),
    )
}

/// Render one prepared visible page with the supplied fonts.
pub fn render_layout_page_svg_with_fonts(
    document: &Document,
    layout: &LayoutDocument,
    page_index: usize,
    options: &RenderOptions,
    fonts: &fonts::FontBook,
) -> Option<RenderedPage> {
    layout
        .pages
        .get(page_index)
        .map(|layout_page| render_layout_page(document, layout_page, options, false, fonts))
}

/// Render the same page with stroke and path boundaries for sample-addressable replay.
/// `data-replay-stroke` indexes the page's strokes; part numbers and path lengths
/// index prepared points. Use `PreparedStroke::sample_ends` to resolve saved samples.
pub fn render_layout_page_replay_svg(
    document: &Document,
    layout: &LayoutDocument,
    page_index: usize,
    options: &RenderOptions,
) -> Option<RenderedPage> {
    render_layout_page_replay_svg_with_fonts(
        document,
        layout,
        page_index,
        options,
        &fonts::FontBook::default(),
    )
}

/// Render sample-addressable replay with the same font resources as export.
pub fn render_layout_page_replay_svg_with_fonts(
    document: &Document,
    layout: &LayoutDocument,
    page_index: usize,
    options: &RenderOptions,
    fonts: &fonts::FontBook,
) -> Option<RenderedPage> {
    layout
        .pages
        .get(page_index)
        .map(|page| render_layout_page(document, page, options, true, fonts))
}

fn render_layout_page(
    document: &Document,
    layout_page: &crate::LayoutPage,
    options: &RenderOptions,
    replay: bool,
    fonts: &fonts::FontBook,
) -> RenderedPage {
    DocumentTextCache::default().render_page(document, layout_page, options, replay, fonts)
}

#[cfg(feature = "pdf")]
pub(crate) struct RenderedScene {
    pub page: RenderedPage,
    pub text: text::native::NativeTextRegistry,
    pub ink: native_ink::NativeInkRegistry,
    pub text_error: Option<String>,
}

#[cfg(feature = "pdf")]
pub(crate) use pdf_slice::PageSceneRequest;
pub(crate) use pdf_slice::PageSlice;

impl DocumentTextCache {
    fn render_page(
        &mut self,
        document: &Document,
        layout_page: &crate::LayoutPage,
        options: &RenderOptions,
        replay: bool,
        fonts: &fonts::FontBook,
    ) -> RenderedPage {
        self.bind(document, options, fonts);
        self.render_bound_page(document, layout_page, options, replay, fonts)
    }

    fn render_bound_page(
        &mut self,
        document: &Document,
        layout_page: &crate::LayoutPage,
        options: &RenderOptions,
        replay: bool,
        fonts: &fonts::FontBook,
    ) -> RenderedPage {
        let (page, scene) = self.prepare_page(
            document,
            layout_page,
            options,
            replay,
            fonts,
            false,
            None,
            &mut |_| {},
        );
        RenderedPage {
            svg: scene.finish(),
            ..page
        }
    }

    #[cfg(feature = "pdf")]
    pub(crate) fn render_layout_page_scenes(
        &mut self,
        document: &Document,
        pages: &[&crate::LayoutPage],
        options: &RenderOptions,
        fonts: &fonts::FontBook,
        observer: &mut dyn FnMut(Progress),
    ) -> Vec<RenderedScene> {
        let requests = pages
            .iter()
            .map(|layout_page| PageSceneRequest::full_page(layout_page))
            .collect::<Vec<_>>();
        self.render_layout_page_scene_requests(document, &requests, options, fonts, observer)
    }

    #[cfg(feature = "pdf")]
    pub(crate) fn render_layout_page_scene_requests(
        &mut self,
        document: &Document,
        requests: &[PageSceneRequest<'_>],
        options: &RenderOptions,
        fonts: &fonts::FontBook,
        observer: &mut dyn FnMut(Progress),
    ) -> Vec<RenderedScene> {
        observer(Progress::pending(ProgressStage::Preparing));
        self.bind(document, options, fonts);
        requests
            .iter()
            .enumerate()
            .map(|(index, request)| {
                observer(Progress::pending(ProgressStage::Preparing));
                let layout_page = request.layout_page();
                let slice = request.slice();
                let (mut page, mut scene) = self.prepare_page(
                    document,
                    layout_page,
                    options,
                    false,
                    fonts,
                    true,
                    slice,
                    observer,
                );
                let text = scene.take_native_text();
                let ink = scene.take_native_ink();
                let text_error = scene.take_native_text_error();
                observer(Progress::pending(ProgressStage::Finalizing));
                page.svg = scene.finish();
                observer(Progress::new(
                    ProgressStage::Pages,
                    index + 1,
                    Some(requests.len()),
                ));
                RenderedScene {
                    page,
                    text,
                    ink,
                    text_error,
                }
            })
            .collect()
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare_page(
        &mut self,
        document: &Document,
        layout_page: &crate::LayoutPage,
        options: &RenderOptions,
        replay: bool,
        fonts: &fonts::FontBook,
        retain_text: bool,
        slice: Option<PageSlice>,
        observer: &mut dyn FnMut(Progress),
    ) -> (RenderedPage, Scene) {
        let page = &layout_page.page;
        let theme = RenderTheme::resolve(page, &document.metadata, options.color_mode);
        let text_renderer = page_text_renderer(document, layout_page, options, fonts);
        observer(Progress::pending(ProgressStage::Preparing));
        let body_text =
            PreparedBodyText::new(document, layout_page, theme, &text_renderer, self, slice);
        let mut svg = render_page_contents_svg(
            page,
            &document.metadata,
            &document.metadata.media_assets,
            theme,
            replay,
            &text_renderer,
            body_text.as_ref(),
            retain_text,
            options.inkpen_profile,
            options.inkpen2_profile,
            options.chinesebrush_profile,
            options.chinesebrush_fwidth_output_profile,
            options.fountain_v17_rainbow_profile.as_ref(),
            options.fountain_v17_output_scale_profile,
            options.fountain_v16_output_scale_profile,
            options.fountain_v14_output_scale_profile,
            options.pencil3_profile,
            options.airbrush_profile,
            options.simplebrush_profile,
            slice,
            observer,
        );
        (
            RenderedPage {
                source_page_index: layout_page.source_page_index,
                width: page.width,
                height: slice.map_or(page.height, PageSlice::height),
                svg: String::new(),
                text_diagnostics: text_renderer.diagnostics(),
                object_diagnostics: text_renderer.object_diagnostics(),
                geometry_diagnostics: svg.take_geometry_diagnostics(),
                paint_diagnostics: svg.take_paint_diagnostics(),
                background_diagnostics: page
                    .background
                    .pdf_paper
                    .as_ref()
                    .filter(|records| !records.is_empty())
                    .map(|records| {
                        vec![BackgroundDiagnostic::PdfPaperOmitted {
                            record_count: records.len(),
                        }]
                    })
                    .unwrap_or_default(),
            },
            svg,
        )
    }
}

struct PreparedBodyText {
    plan: Rc<PreparedBodyPlan>,
    source_page_index: usize,
    object_index: usize,
    page_top: f64,
    viewport: Viewport,
}

struct PreparedBodyPlan {
    text: RichTextBox,
    layout: text::TextLayout,
    text_issues: Vec<text::SourceTextDiagnostic>,
    object_issues: Vec<text::SourceObjectDiagnostic>,
    exclusions: Option<text::PageExclusions>,
    native_pages: Result<native_body_pages::NativeBodyPages, text::NativePageIndexUnavailable>,
}

impl PreparedBodyText {
    fn new(
        document: &Document,
        page: &crate::LayoutPage,
        theme: RenderTheme,
        renderer: &TextRenderer<'_>,
        cache: &mut DocumentTextCache,
        slice: Option<PageSlice>,
    ) -> Option<Self> {
        let body = page.body_text_slice()?;
        let full_body = page.single_page_continuous_body(document);
        let (text, source_range, first_page_index, last_page_index, detached) =
            if let Some(text) = full_body {
                let source_range = 0..text.text.chars().count();
                (text, source_range, 0, 0, false)
            } else if let Some(window) = body.capture_window.as_ref() {
                (
                    page.body_text_capture(document)?,
                    window.source_range.clone(),
                    window.first_page_index,
                    window.requested_page_index,
                    window.first_page_index != 0
                        || document.metadata.note_text.as_ref().is_some_and(|source| {
                            window.saved_source_range != (0..source.text.chars().count())
                        }),
                )
            } else {
                (
                    page.body_text_reflow(document)?,
                    body.reflow.as_ref()?.source_range.clone(),
                    body.reflow.as_ref()?.first_page_index,
                    document.pages.len().checked_sub(1)?,
                    false,
                )
            };
        let key = BodyPlanKey {
            source_range,
            first_page_index,
            last_page_index,
            detached,
            theme,
        };
        let plan = if let Some((_, plan)) = cache.plans.iter().find(|(stored, _)| *stored == key) {
            Rc::clone(plan)
        } else {
            let plan = Rc::new(PreparedBodyPlan::new(document, text, &key, renderer)?);
            if cache.plans.len() == BODY_PLAN_CACHE_CAPACITY {
                cache.plans.remove(0);
            }
            cache.plans.push((key, Rc::clone(&plan)));
            plan
        };
        let page_top = document.pages[first_page_index..page.source_page_index]
            .iter()
            .map(|page| f64::from(page.height))
            .sum::<f64>();
        // Keep native overshooting tail geometry in the root SVG/canvas, but
        // select body text only from this physical note page. Otherwise a
        // fallback plan without certified native line ownership can admit
        // next-page glyphs into this page's retained-text registry before the
        // physical-page clip hides their paint.
        let slice_y = slice.map_or(0, PageSlice::y);
        let text_end = slice.map_or(page.page.height, |slice| {
            slice
                .y()
                .saturating_add(slice.height())
                .min(page.page.height)
        });
        Some(Self {
            plan,
            source_page_index: page.source_page_index,
            object_index: body.object_index,
            page_top,
            viewport: Viewport::new(BoundingBox {
                x_min: 0.0,
                x_max: f64::from(page.page.width),
                y_min: page_top + f64::from(slice_y),
                y_max: page_top + f64::from(text_end),
            }),
        })
    }

    fn paint(&self, svg: &mut Scene, context: &CompositionContext<'_>) {
        let plan = &self.plan;
        let renderer = context
            .text_renderer
            .translated_paint(0.0, self.page_top)
            .with_page_exclusions(plan.exclusions.clone());
        let styled = StyledText::new(&plan.text, TextContext::Flow, renderer.settings);
        let native_lines = plan
            .native_pages
            .as_ref()
            .ok()
            .and_then(|pages| pages.lines(self.source_page_index));
        let sources = VisibleTextSources::new(
            &styled,
            &plan.layout,
            self.viewport,
            context.theme,
            native_lines,
        );
        renderer.report_owned_object_issues(
            &plan
                .object_issues
                .iter()
                .filter(|issue| sources.contains(issue.owner.as_ref(), false))
                .cloned()
                .collect::<Vec<_>>(),
        );
        let clip = svg.definition::<Clip>();
        svg.push(
            Definitions::new().add(
                ClipPath::new(&clip).add(
                    Rectangle::new()
                        .x(0)
                        .y(0)
                        .width(context.page.width)
                        .height(context.page.height),
                ),
            ),
        );
        svg.scope(Group::new().flow().clipped(&clip), |svg| {
            svg.scope(
                Group::new().transformed(Transform::translate(0.0, -self.page_top, 5)),
                |svg| {
                    let mut target =
                        text::NativePaintTarget::new(svg.retains_text(), Some(self.viewport));
                    if let Some(lines) = native_lines {
                        target = target.with_native_lines(lines);
                    }
                    paint_text_layout_with_target(
                        svg,
                        &styled,
                        &plan.layout,
                        context.media_assets,
                        context.theme,
                        &renderer,
                        target,
                    );
                },
            );
        });
        renderer.report_text_issues(
            &plan
                .text_issues
                .iter()
                .filter_map(|issue| {
                    is_preparation_issue(&issue.diagnostic)
                        .then(|| sources.project_issue(&styled, issue))
                        .flatten()
                })
                .collect::<Vec<_>>(),
        );
    }
}

impl PreparedBodyPlan {
    fn new(
        document: &Document,
        text: RichTextBox,
        key: &BodyPlanKey,
        renderer: &TextRenderer<'_>,
    ) -> Option<Self> {
        let frame_width = document.pages[key.first_page_index..=key.last_page_index]
            .iter()
            .map(|page| page.width)
            .max()
            .map(f64::from)?;
        let exclusions = text::PageExclusions::for_range(
            document,
            key.first_page_index..=key.last_page_index,
            renderer.settings,
        );
        let planner = TextRenderer::new(renderer.settings, renderer.fonts)
            .with_point_marker_target(renderer.point_marker_target)
            .with_native_object_page_obstacles(text::NativeObjectPageObstacles::from_document(
                document,
            ))
            .with_object_page_ownership(if key.detached {
                text::ObjectPageOwnership::Detached
            } else {
                text::ObjectPageOwnership::Unknown
            })
            .with_page_exclusions(exclusions.clone());
        let bands = exclusions
            .as_ref()
            .map_or_else(Vec::new, text::PageExclusions::line_bands);
        let padding = document
            .metadata
            .flow_page_padding
            .map_or(FLOW_HORIZONTAL_PADDING, |(horizontal, _)| {
                f64::from(horizontal)
            });
        let styled = StyledText::new(&text, TextContext::Flow, renderer.settings);
        let frame = text::TextFrame {
            bbox: BoundingBox {
                x_min: padding,
                y_min: 0.0,
                x_max: frame_width - padding,
                y_max: 0.0,
            },
            gravity: None,
            exclusions: &bands,
        };
        let complete_source = !key.detached
            && matches!(document.metadata.page_mode, None | Some(0))
            && document.metadata.note_text.as_ref().is_some_and(|source| {
                source.text_sections.is_empty() && crate::layout::same_text_source(source, &text)
            });
        let (layout, native_page_layout) = if complete_source {
            text::layout_native_page_text(&styled, frame, key.theme, &planner)
        } else {
            let mut layout = text::layout_capture_text(&styled, frame, key.theme, &planner);
            layout.native_object_entry = None;
            (
                layout,
                Err(text::NativePageIndexUnavailable::OutsideCertificate),
            )
        };
        let native_pages = native_page_layout.and_then(|page_layout| {
            native_body_pages::NativeBodyPages::from_measured(
                document,
                key.first_page_index,
                key.last_page_index,
                &page_layout,
            )
        });
        Some(Self {
            text,
            layout,
            text_issues: planner.scoped_diagnostics(),
            object_issues: planner.scoped_object_diagnostics(),
            exclusions,
            native_pages,
        })
    }
}

fn page_text_renderer<'a>(
    document: &Document,
    layout_page: &crate::LayoutPage,
    options: &RenderOptions,
    fonts: &'a fonts::FontBook,
) -> TextRenderer<'a> {
    let settings = TextSettings::from_document(&document.metadata);
    TextRenderer::new(settings, fonts)
        .with_point_marker_target(options.point_marker_target)
        .with_native_object_page_obstacles(text::NativeObjectPageObstacles::from_document(document))
        .with_table_export_page(table::TableExportPage::for_page(
            &layout_page.page,
            &document.metadata,
        ))
        .with_page_exclusions(text::PageExclusions::for_page(
            document,
            layout_page.source_page_index,
            settings,
        ))
}

struct VisibleTextSources {
    text: Vec<Range<usize>>,
    layout: Vec<Range<usize>>,
    objects: Vec<Range<usize>>,
}

impl VisibleTextSources {
    fn new(
        styled: &StyledText<'_>,
        layout: &text::TextLayout,
        viewport: Viewport,
        theme: RenderTheme,
        native_lines: Option<text::NativePageLineRange>,
    ) -> Self {
        let mut sources = Self {
            text: Vec::new(),
            layout: Vec::new(),
            objects: Vec::new(),
        };
        for (index, line) in layout.lines.iter().enumerate() {
            if native_lines.is_some_and(|lines| !lines.contains(index)) {
                continue;
            }
            if viewport.line_visible(line) {
                sources.layout.push(line.line.source.clone());
                if viewport.text_visible(styled, line, theme) {
                    sources.text.push(line.line.source.clone());
                }
            }
            if let Some(marker) = &line.marker
                && marker
                    .marker
                    .bounds(marker.x, marker.center_y)
                    .is_some_and(|bounds| viewport.intersects(bounds))
            {
                sources
                    .text
                    .push(marker.source..marker.source.saturating_add(1));
            }
            let left = aligned_line_left(&line.line, line.x, line.width, line.alignment);
            for object in &line.line.objects {
                if viewport.intersects(object_paint_bounds(object, left, line.baseline)) {
                    sources.objects.push(object.object.source.clone());
                }
            }
        }
        sources
    }

    fn contains(&self, owner: Option<&text::SourceOwner>, geometry: bool) -> bool {
        let Some(owner) = owner else {
            return true;
        };
        let (source, visible) = match owner {
            text::SourceOwner::Text(source) => {
                (source, if geometry { &self.layout } else { &self.text })
            }
            text::SourceOwner::Object(source) => (source, &self.objects),
        };
        visible.iter().any(|visible| {
            if source.is_empty() {
                visible.start <= source.start && source.start <= visible.end
            } else {
                source.start < visible.end && visible.start < source.end
            }
        })
    }

    fn project_issue(
        &self,
        styled: &StyledText<'_>,
        issue: &text::SourceTextDiagnostic,
    ) -> Option<text::SourceTextDiagnostic> {
        let geometry = issue.diagnostic.kind == TextDiagnosticKind::InvalidGeometry;
        if !self.contains(issue.owner.as_ref(), geometry) {
            return None;
        }
        let mut issue = issue.clone();
        if let Some(text::SourceOwner::Text(source)) = &issue.owner
            && !issue.diagnostic.codepoints.is_empty()
        {
            let ranges = if geometry { &self.layout } else { &self.text };
            let visible = ranges
                .iter()
                .filter_map(|range| {
                    let start = range.start.max(source.start);
                    let end = range.end.min(source.end);
                    (start < end)
                        .then(|| styled.index.slice(start..end))
                        .flatten()
                })
                .flat_map(str::chars)
                .map(u32::from)
                .collect::<std::collections::BTreeSet<_>>();
            issue
                .diagnostic
                .codepoints
                .retain(|codepoint| visible.contains(codepoint));
            if issue.diagnostic.codepoints.is_empty() {
                return None;
            }
        }
        Some(issue)
    }
}

// Default ink for uncolored strokes, by canvas: light on dark, dark on light.
const DEFAULT_INK_DARK_MODE: &str = "#ffffff";
const DEFAULT_INK_LIGHT_MODE: &str = "#1a1a1a";
// Pressure channel on v4.4.x files can be present but all-zero; treat as absent.
const PRESSURE_PRESENT_EPSILON: f64 = 0.01;

#[allow(clippy::too_many_arguments)]
fn render_page_contents_svg(
    page: &Page,
    metadata: &crate::DocumentMetadata,
    media_assets: &[MediaAsset],
    theme: RenderTheme,
    replay: bool,
    text_renderer: &TextRenderer<'_>,
    body_text: Option<&PreparedBodyText>,
    retain_text: bool,
    inkpen_profile: Option<InkPenRenderProfile>,
    inkpen2_profile: Option<InkPen2RenderProfile>,
    chinesebrush_profile: Option<ChineseBrushRenderProfile>,
    chinesebrush_fwidth_output_profile: Option<ChineseBrushFwidthOutputProfile>,
    fountain_v17_rainbow_profile: Option<&FountainV17RainbowRenderProfile>,
    fountain_v17_output_scale_profile: Option<FountainV17OutputScaleProfile>,
    fountain_v16_output_scale_profile: Option<FountainV16OutputScaleProfile>,
    fountain_v14_output_scale_profile: Option<FountainV14OutputScaleProfile>,
    pencil3_profile: Option<Pencil3RenderProfile>,
    airbrush_profile: Option<AirBrushRenderProfile>,
    simplebrush_profile: Option<SimpleBrushRenderProfile>,
    slice: Option<PageSlice>,
    observer: &mut dyn FnMut(Progress),
) -> Scene {
    let bg = color_hex(&theme.background());
    let vb_x = 0.0;
    let vb_y = f64::from(slice.map_or(0, PageSlice::y));
    let vb_w = page.width as f64;
    let vb_h = f64::from(slice.map_or(page.height, PageSlice::height));
    let svg_w = page.width;
    let svg_h = slice.map_or(page.height, PageSlice::height);

    let mut svg = Scene::new(
        Svg::new()
            .view_box(ViewBox::new(vb_x, vb_y, vb_w, vb_h, 1))
            .width(svg_w)
            .height(svg_h),
    );
    #[cfg(feature = "pdf")]
    if retain_text {
        svg.retain_text();
    }
    #[cfg(not(feature = "pdf"))]
    let _ = retain_text;
    let include_canvas = slice.is_none_or(PageSlice::include_canvas);
    if include_canvas {
        svg.push(
            Rectangle::new()
                .x(vb_x)
                .y(vb_y)
                .width(vb_w)
                .height(vb_h)
                .fill(Paint::from_hex(&bg)),
        );
    }

    if include_canvas
        && let Ok(Some(pattern)) = crate::page_background::template_pattern(page, metadata)
    {
        match pattern {
            crate::page_background::TemplatePattern::Dots(dots) => {
                render_dot_background(&mut svg, page, dots, theme)
            }
            crate::page_background::TemplatePattern::Lines(lines) => {
                render_line_background(&mut svg, page, lines, theme)
            }
        }
    }

    let composition = CompositionContext {
        page,
        media_assets,
        flow_page_padding: metadata.flow_page_padding,
        text_renderer,
        theme,
        replay,
        body_text,
        inkpen_profile,
        inkpen2_profile,
        chinesebrush_profile,
        chinesebrush_fwidth_output_profile,
        fountain_v17_rainbow_profile,
        fountain_v17_output_scale_profile,
        fountain_v16_output_scale_profile,
        fountain_v14_output_scale_profile,
        pencil3_profile,
        airbrush_profile,
        simplebrush_profile,
        simplebrush_remaining_stamps: Cell::new(32_768),
        slice,
    };
    let total = page.composed_objects().count();
    let mut progress = WorkProgress::new(observer, ProgressStage::Rendering, total);
    render_pass(&mut svg, &composition, RenderPass::Base, &mut progress);
    if page
        .objects
        .iter()
        .any(|object| object.render_pass() == Some(RenderPass::Top))
    {
        let blend = if theme.is_dark() {
            Blend::Lighten
        } else {
            Blend::Darken
        };
        svg.scope(Group::new().blend(blend), |svg| {
            render_pass(svg, &composition, RenderPass::Top, &mut progress);
        });
    }
    render_pass(&mut svg, &composition, RenderPass::Masking, &mut progress);
    text_renderer.embed_fonts(&mut svg);
    svg
}

struct CompositionContext<'a> {
    page: &'a Page,
    media_assets: &'a [MediaAsset],
    flow_page_padding: Option<(u32, u32)>,
    text_renderer: &'a TextRenderer<'a>,
    theme: RenderTheme,
    replay: bool,
    body_text: Option<&'a PreparedBodyText>,
    inkpen_profile: Option<InkPenRenderProfile>,
    inkpen2_profile: Option<InkPen2RenderProfile>,
    chinesebrush_profile: Option<ChineseBrushRenderProfile>,
    chinesebrush_fwidth_output_profile: Option<ChineseBrushFwidthOutputProfile>,
    fountain_v17_rainbow_profile: Option<&'a FountainV17RainbowRenderProfile>,
    fountain_v17_output_scale_profile: Option<FountainV17OutputScaleProfile>,
    fountain_v16_output_scale_profile: Option<FountainV16OutputScaleProfile>,
    fountain_v14_output_scale_profile: Option<FountainV14OutputScaleProfile>,
    pencil3_profile: Option<Pencil3RenderProfile>,
    airbrush_profile: Option<AirBrushRenderProfile>,
    simplebrush_profile: Option<SimpleBrushRenderProfile>,
    // Shared across all passes in this composition; each scene gets its own budget.
    simplebrush_remaining_stamps: Cell<usize>,
    slice: Option<PageSlice>,
}

fn render_pass(
    svg: &mut Scene,
    context: &CompositionContext<'_>,
    pass: RenderPass,
    progress: &mut WorkProgress<'_>,
) {
    let mut calligraphy = crate::ink::calligraphy::CalligraphySession::new();
    let mut stroke_index = 0;
    for (object_index, object) in context.page.objects.iter().enumerate() {
        if let Some(selected) = object.render_pass() {
            if selected == pass {
                if pass == RenderPass::Base && object.standard_batch_boundary().is_some() {
                    calligraphy.reset();
                }
                if let Some(body) = context.body_text
                    && body.object_index == object_index
                {
                    body.paint(svg, context);
                    progress.advance(1);
                } else {
                    render_object(
                        svg,
                        context,
                        object,
                        &mut stroke_index,
                        &mut calligraphy,
                        true,
                        progress,
                    );
                }
            } else {
                stroke_index += object.stroke_count();
            }
        }
    }
}

fn render_object(
    svg: &mut Scene,
    context: &CompositionContext<'_>,
    object: &PageObject,
    stroke_index: &mut usize,
    calligraphy_session: &mut crate::ink::calligraphy::CalligraphySession,
    native_allowed: bool,
    progress: &mut WorkProgress<'_>,
) {
    let native_allowed =
        native_allowed && !matches!(object.content, PageObjectContent::Descendant(_));
    match object.content.leaf() {
        PageObjectContent::Stroke(stroke) => {
            render_stroke(
                svg,
                stroke,
                context.theme,
                context.replay.then_some(*stroke_index),
                calligraphy_session,
                native_allowed,
                [context.page.width, context.page.height],
                context.inkpen_profile,
                context.inkpen2_profile,
                context.chinesebrush_profile,
                context.chinesebrush_fwidth_output_profile,
                context.fountain_v17_rainbow_profile,
                context.fountain_v17_output_scale_profile,
                context.fountain_v16_output_scale_profile,
                context.fountain_v14_output_scale_profile,
                context.pencil3_profile,
                context.airbrush_profile,
                context.simplebrush_profile,
                &context.simplebrush_remaining_stamps,
                context.slice,
            );
            *stroke_index += 1;
        }
        PageObjectContent::Element(element) => {
            render_element(svg, element, object.source_offset, context);
        }
        PageObjectContent::Container(children) => {
            for child in children {
                render_object(
                    svg,
                    context,
                    child,
                    stroke_index,
                    calligraphy_session,
                    native_allowed,
                    progress,
                );
            }
        }
        PageObjectContent::Descendant(_) => unreachable!("leaf unwraps descendant content"),
    }
    progress.advance(1);
}

fn render_line_background(
    svg: &mut Scene,
    page: &Page,
    lines: crate::page_background::LinePattern,
    theme: RenderTheme,
) {
    let color = if theme.is_dark() {
        "#fafafa"
    } else {
        "#010102"
    };
    let opacity = if theme.is_dark() { 0.3 } else { 0.2 };
    let mut data = Data::new();
    for row in 0..lines.rows {
        let y = lines.first_y + f64::from(row) * lines.pitch_y;
        data = data
            .move_to((0, coordinate(y, 6)))
            .horizontal_line_to(page.width);
    }
    svg.push(
        Path::new()
            .template(PageTemplate::Lines)
            .data(data)
            .fill(Paint::None)
            .stroke(Paint::from_hex(color))
            .stroke_opacity(opacity)
            .stroke_width(decimal(lines.width, 6)),
    );
}

fn render_dot_background(
    svg: &mut Scene,
    page: &Page,
    dots: crate::page_background::DotPattern,
    theme: RenderTheme,
) {
    let crate::page_background::DotPattern {
        pitch_x,
        pitch_y,
        top,
        radius_x,
        radius_y,
        rows,
    } = dots;
    let color = if theme.is_dark() {
        "#fafafa"
    } else {
        "#010102"
    };
    let scale_y = radius_y / radius_x;
    // Samsung draws round, zero-length dashes. Explicit row subpaths avoid
    // fractional Svg pattern-tile rounding in raster exporters. Dash phase
    // restarts at x=0 for every row; the outer Svg clips the page boundaries.
    let mut data = Data::new();
    for row in 0..rows {
        let y = (top + radius_y + f64::from(row) * pitch_y) / scale_y;
        data = data
            .move_to((0, coordinate(y, 6)))
            .horizontal_line_to(page.width);
    }
    svg.push(
        Path::new()
            .template(PageTemplate::Dots)
            .data(data)
            .fill(Paint::None)
            .stroke(Paint::from_hex(color))
            .stroke_opacity(0.2)
            .stroke_width(decimal(2.0 * radius_x, 6))
            .line_cap(LineCap::Round)
            .dotted(decimal(pitch_x, 6))
            .transformed(Transform::scale(1., scale_y, 9)),
    );
}

fn render_element(
    svg: &mut Scene,
    element: &PageElement,
    source_offset: Option<usize>,
    context: &CompositionContext<'_>,
) {
    let CompositionContext {
        page,
        media_assets,
        flow_page_padding,
        text_renderer,
        theme,
        ..
    } = *context;
    match element {
        PageElement::Image { bbox, media_index } => {
            render_image(svg, *bbox, Some(*media_index), None, media_assets);
        }
        PageElement::PlacedImage(image) => render_placed_image(svg, image, media_assets),
        // Native fill-effect transparency is defined only for type-3 images.
        PageElement::FormulaImage(image) => render_placed_image_body(svg, image, media_assets),
        PageElement::TextBox(text_box) => render_text_box(
            svg,
            text_box,
            page,
            media_assets,
            flow_page_padding,
            theme,
            text_renderer,
        ),
        PageElement::Shape(shape) => {
            if let Err(kind) = render_shape(svg, shape, theme, source_offset) {
                svg.report_geometry_issue(GeometryDiagnostic {
                    source_offset,
                    object_uuid: shape.metadata.uuid.clone(),
                    kind,
                });
            }
            if let Some(text) = &shape.text {
                let theme = match shape.fill {
                    crate::ShapePaint::Solid(argb) => theme.on_surface(
                        theme.foreground_color(argb_color(argb)),
                        f64::from((argb >> 24) as u8) / 255.0,
                    ),
                    _ => theme,
                };
                let frame = placed::PlacedTextFrame::for_shape(shape).unwrap_or_else(|issue| {
                    if !text.text.is_empty() {
                        let kind = match issue {
                            placed::ShapeTextFrameIssue::UnsupportedTemplate => {
                                TextDiagnosticKind::UnsupportedTextFrame
                            }
                            placed::ShapeTextFrameIssue::InvalidGeometry => {
                                TextDiagnosticKind::InvalidGeometry
                            }
                        };
                        text_renderer.report_geometry_issues(&[TextDiagnostic {
                            kind,
                            family: String::new(),
                            codepoints: Vec::new(),
                        }]);
                    }
                    placed::PlacedTextFrame::from_text_box(text)
                });
                render_placed_text(svg, text, frame, media_assets, theme, text_renderer);
            }
        }
        PageElement::Line(line) => {
            if let Err(kind) = render_line(svg, line, theme, source_offset) {
                svg.report_geometry_issue(GeometryDiagnostic {
                    source_offset,
                    object_uuid: line.metadata.uuid.clone(),
                    kind,
                });
            }
        }
    }
}

// Both shape and line writers serialize the drawing path in page coordinates,
// including rotation. Reject an unsupported path as a whole, never draw a prefix.
fn native_svg_path(bytes: &[u8]) -> Result<Data, GeometryDiagnosticKind> {
    let mut commands = Vec::new();
    let parsed = crate::shape::native_path::visit_drawing_path(bytes, |command| {
        commands.push(command);
    });
    let (size, supported) = parsed.map_err(|_| GeometryDiagnosticKind::InvalidPath)?;
    if size != bytes.len() {
        return Err(GeometryDiagnosticKind::InvalidPath);
    }
    if !supported {
        return Err(GeometryDiagnosticKind::UnsupportedPath);
    }
    native_svg_commands(commands)
}

fn native_svg_commands(
    commands: impl IntoIterator<Item = crate::shape::NativePathCommand>,
) -> Result<Data, GeometryDiagnosticKind> {
    use crate::shape::NativePathCommand;

    fn coordinates<const N: usize>(values: [f64; N]) -> Option<[f32; N]> {
        let values = values.map(|value| value as f32);
        values
            .iter()
            .all(|value| value.is_finite())
            .then_some(values)
    }

    let mut prepared = Vec::new();
    let mut contour_start = None;
    let mut after_close = false;
    for command in commands {
        match command {
            NativePathCommand::Move(point) => {
                contour_start = Some(point);
                after_close = false;
                prepared.push(NativePathCommand::Move(point));
            }
            NativePathCommand::Close => {
                if contour_start.is_some() && !after_close {
                    prepared.push(NativePathCommand::Close);
                    after_close = true;
                }
            }
            draw @ (NativePathCommand::Line(_)
            | NativePathCommand::Quadratic(_)
            | NativePathCommand::Cubic(_)) => {
                if after_close {
                    prepared.push(NativePathCommand::Move(
                        contour_start.expect("closed contour"),
                    ));
                    after_close = false;
                } else if contour_start.is_none() {
                    contour_start = Some([0.0, 0.0]);
                    prepared.push(NativePathCommand::Move([0.0, 0.0]));
                }
                prepared.push(draw);
            }
            primitive @ (NativePathCommand::Arc(_) | NativePathCommand::Oval(_)) => {
                prepared.push(primitive);
            }
        }
    }
    let mut data = Data::new();
    for command in prepared {
        data = match command {
            NativePathCommand::Move(point) => {
                let [x, y] =
                    coordinates(point).ok_or(GeometryDiagnosticKind::UnrepresentablePath)?;
                data.move_to((x, y))
            }
            NativePathCommand::Line(point) => {
                let [x, y] =
                    coordinates(point).ok_or(GeometryDiagnosticKind::UnrepresentablePath)?;
                data.line_to((x, y))
            }
            NativePathCommand::Quadratic([[x1, y1], [x, y]]) => {
                let [x1, y1, x, y] = coordinates([x1, y1, x, y])
                    .ok_or(GeometryDiagnosticKind::UnrepresentablePath)?;
                data.quadratic_curve_to((x1, y1, x, y))
            }
            NativePathCommand::Cubic([[x1, y1], [x2, y2], [x, y]]) => {
                let [x1, y1, x2, y2, x, y] = coordinates([x1, y1, x2, y2, x, y])
                    .ok_or(GeometryDiagnosticKind::UnrepresentablePath)?;
                data.cubic_curve_to((x1, y1, x2, y2, x, y))
            }
            NativePathCommand::Close => data.close(),
            NativePathCommand::Arc(_) | NativePathCommand::Oval(_) => {
                return Err(GeometryDiagnosticKind::UnsupportedPath);
            }
        };
    }
    Ok(data)
}

fn render_shape(
    svg: &mut Scene,
    shape: &crate::NativeShape,
    theme: RenderTheme,
    source_offset: Option<usize>,
) -> Result<(), GeometryDiagnosticKind> {
    // Validate all path/primitive geometry before paint definitions allocate IDs.
    let path = if shape.path_data.is_empty() {
        None
    } else {
        let (consumed, _) = crate::shape::visit_path(&shape.path_data, |_| {})
            .map_err(|_| GeometryDiagnosticKind::InvalidPath)?;
        if consumed != shape.path_data.len() {
            return Err(GeometryDiagnosticKind::InvalidPath);
        }
        let normalized = crate::shape::native_path::normalize_model_path(
            &shape.path_data,
            [
                shape.geometry_bbox.x_min,
                shape.geometry_bbox.y_min,
                shape.geometry_bbox.x_max,
                shape.geometry_bbox.y_max,
            ],
            shape.rotation_degrees,
            shape.horizontal_flip,
            shape.vertical_flip,
            true,
        )
        .map_err(|_| GeometryDiagnosticKind::InvalidPath)?;
        let Some(normalized) = normalized else {
            let mut narrowed_finite = true;
            crate::shape::visit_path(&shape.path_data, |command| {
                use crate::shape::NativePathCommand;
                let values: Vec<f64> = match command {
                    NativePathCommand::Move([x, y]) | NativePathCommand::Line([x, y]) => {
                        vec![x, y]
                    }
                    NativePathCommand::Quadratic([[x1, y1], [x, y]]) => vec![x1, y1, x, y],
                    NativePathCommand::Cubic([[x1, y1], [x2, y2], [x, y]]) => {
                        vec![x1, y1, x2, y2, x, y]
                    }
                    NativePathCommand::Arc(values) => values.to_vec(),
                    NativePathCommand::Oval(values) => values.to_vec(),
                    NativePathCommand::Close => Vec::new(),
                };
                narrowed_finite &= values.iter().all(|value| (*value as f32).is_finite());
            })
            .map_err(|_| GeometryDiagnosticKind::InvalidPath)?;
            return Err(if narrowed_finite {
                GeometryDiagnosticKind::UnsupportedPath
            } else {
                GeometryDiagnosticKind::UnrepresentablePath
            });
        };
        Some(native_svg_commands(normalized.commands)?)
    };
    let bbox = shape.geometry_bbox;
    let width = bbox.x_max - bbox.x_min;
    let height = bbox.y_max - bbox.y_min;
    let cx = bbox.x_min + width / 2.0;
    let cy = bbox.y_min + height / 2.0;
    let rotation = if path.is_none() {
        if ![bbox.x_min, bbox.y_min, width, height, cx, cy]
            .iter()
            .all(|v| v.is_finite())
            || width <= 0.0
            || height <= 0.0
            || !shape.rotation_degrees.is_finite()
        {
            return Err(GeometryDiagnosticKind::InvalidGeometry);
        }
        if !matches!(shape.shape_type, 1..=4 | 8) {
            return Err(GeometryDiagnosticKind::UnsupportedShapeTemplate);
        }
        Some(gradient::ElementRotation {
            angle: f64::from(shape.rotation_degrees),
            center: [cx, cy],
        })
    } else {
        None
    };
    let (fill, opacity) =
        render_shape_paint(svg, shape, PaintRole::Fill, rotation, theme, source_offset);
    let outline = render_shape_paint(
        svg,
        shape,
        PaintRole::Outline,
        rotation,
        theme,
        source_offset,
    );
    let mut style = shape_outline(
        svg,
        &shape.style,
        outline,
        source_offset,
        &shape.metadata.uuid,
    )
    .fill(fill)
    .fill_opacity(decimal(opacity, 4));
    if let Some(path) = path {
        svg.push(style.add(Path::new().data(path)));
        return Ok(());
    }
    if let Some(rotation) = rotation {
        style = style.transformed(Transform::rotate_unrounded(
            rotation.angle,
            rotation.center[0],
            rotation.center[1],
        ));
    }
    let points = match shape.shape_type {
        1 => {
            svg.push(
                style.add(
                    Ellipse::new()
                        .cx(cx)
                        .cy(cy)
                        .rx(width / 2.0)
                        .ry(height / 2.0),
                ),
            );
            return Ok(());
        }
        2 => vec![
            (cx, bbox.y_min),
            (bbox.x_max, bbox.y_max),
            (bbox.x_min, bbox.y_max),
        ],
        3 => vec![
            (bbox.x_min, bbox.y_min),
            (bbox.x_max, bbox.y_max),
            (bbox.x_min, bbox.y_max),
        ],
        4 => {
            svg.push(
                style.add(
                    Rectangle::new()
                        .x(bbox.x_min)
                        .y(bbox.y_min)
                        .width(width)
                        .height(height),
                ),
            );
            return Ok(());
        }
        8 => vec![
            (cx, bbox.y_min),
            (bbox.x_max, cy),
            (cx, bbox.y_max),
            (bbox.x_min, cy),
        ],
        _ => return Err(GeometryDiagnosticKind::UnsupportedShapeTemplate),
    };
    svg.push(style.add(Polygon::new().points_unrounded(&points)));
    Ok(())
}

fn render_line(
    svg: &mut Scene,
    line: &crate::NativeLine,
    theme: RenderTheme,
    source_offset: Option<usize>,
) -> Result<(), GeometryDiagnosticKind> {
    // Serialized endpoints already include the native rotation.
    if line.line_type > 2 {
        return Err(GeometryDiagnosticKind::UnsupportedLineType);
    }
    if line.begin.iter().chain(&line.end).any(|v| !v.is_finite()) {
        return Err(GeometryDiagnosticKind::InvalidGeometry);
    }
    let path = if !line.path_data.is_empty() {
        Some(native_svg_path(&line.path_data)?)
    } else if line.line_type == 0 {
        None
    } else {
        return Err(GeometryDiagnosticKind::MissingLinePath);
    };
    let outline = project_line_paint(svg, line, theme).unwrap_or_else(|kind| {
        svg.report_paint_issue(PaintDiagnostic {
            source_offset,
            object_uuid: line.metadata.uuid.clone(),
            role: PaintRole::Outline,
            kind,
        });
        (Paint::None, 0.0)
    });
    let style = shape_outline(
        svg,
        &line.style,
        outline,
        source_offset,
        &line.metadata.uuid,
    )
    .fill(Paint::None);
    let body = if let Some(path) = path {
        style.add(Path::new().data(path))
    } else {
        style.add(
            Line::new()
                .x1(line.begin[0])
                .y1(line.begin[1])
                .x2(line.end[0])
                .y2(line.end[1]),
        )
    };
    if !matches!(outline.0, Paint::None) && line.style.dash_pattern().is_ok() {
        match arrows::Plan::for_line(line) {
            Ok(Some(arrows)) => arrows.render(svg, outline.0, outline.1, body),
            Ok(None) => svg.push(body),
            Err(()) => {
                svg.report_paint_issue(PaintDiagnostic {
                    source_offset,
                    object_uuid: line.metadata.uuid.clone(),
                    role: PaintRole::Outline,
                    kind: PaintDiagnosticKind::UnsupportedArrowGeometry,
                });
                svg.push(body);
            }
        }
    } else {
        svg.push(body);
    }
    Ok(())
}

fn rectangle(bbox: BoundingBox, offset_y: f64, precision: usize) -> Rectangle {
    Rectangle::new()
        .x(decimal(bbox.x_min, precision))
        .y(decimal(bbox.y_min + offset_y, precision))
        .width(decimal(bbox.x_max - bbox.x_min, precision))
        .height(decimal(bbox.y_max - bbox.y_min, precision))
}

fn shape_paint(paint: &crate::ShapePaint, theme: RenderTheme) -> (String, f64) {
    match paint {
        crate::ShapePaint::Solid(argb) => {
            let color = Color {
                r: (argb >> 16) as u8,
                g: (argb >> 8) as u8,
                b: *argb as u8,
            };
            let color = theme.foreground(Some(color));
            (color, f64::from((argb >> 24) as u8) / 255.0)
        }
        _ => ("none".into(), 0.0),
    }
}

fn render_shape_paint(
    svg: &mut Scene,
    shape: &crate::NativeShape,
    role: PaintRole,
    rotation: Option<gradient::ElementRotation>,
    theme: RenderTheme,
    source_offset: Option<usize>,
) -> (Paint, f64) {
    let (paint, source) = match role {
        PaintRole::Fill => (&shape.fill, shape.fill_source.as_ref()),
        PaintRole::Outline => (&shape.style.paint, shape.style.paint_source.as_ref()),
    };
    project_shape_paint(svg, paint, source, Some((shape, rotation)), role, theme).unwrap_or_else(
        |kind| {
            svg.report_paint_issue(PaintDiagnostic {
                source_offset,
                object_uuid: shape.metadata.uuid.clone(),
                role,
                kind,
            });
            (Paint::None, 0.0)
        },
    )
}

fn project_line_paint(
    svg: &mut Scene,
    line: &crate::NativeLine,
    theme: RenderTheme,
) -> Result<(Paint, f64), PaintDiagnosticKind> {
    match &line.style.paint {
        crate::ShapePaint::Gradient => {
            let Some(crate::ShapePaintSource::Color(source)) = line.style.paint_source.as_ref()
            else {
                return Err(PaintDiagnosticKind::MissingPaintSource);
            };
            Ok((
                gradient::Plan::for_line(line, source)?.paint(svg, theme),
                1.0,
            ))
        }
        paint => project_shape_paint(
            svg,
            paint,
            line.style.paint_source.as_ref(),
            None,
            PaintRole::Outline,
            theme,
        ),
    }
}

fn project_shape_paint(
    svg: &mut Scene,
    paint: &crate::ShapePaint,
    source: Option<&crate::ShapePaintSource>,
    frame: Option<(&crate::NativeShape, Option<gradient::ElementRotation>)>,
    role: PaintRole,
    theme: RenderTheme,
) -> Result<(Paint, f64), PaintDiagnosticKind> {
    match paint {
        crate::ShapePaint::None => Ok((Paint::None, 0.0)),
        crate::ShapePaint::Solid(_) => {
            let (color, opacity) = shape_paint(paint, theme);
            Ok((Paint::from_hex(&color).unwrap(), opacity))
        }
        crate::ShapePaint::Gradient => {
            let Some(crate::ShapePaintSource::Color(source)) = source else {
                return Err(PaintDiagnosticKind::MissingPaintSource);
            };
            let Some((shape, rotation)) = frame else {
                return Err(PaintDiagnosticKind::UnsupportedGradientFrame);
            };
            let plan = gradient::Plan::for_shape(shape, source, role, rotation)?;
            Ok((plan.paint(svg, theme), 1.0))
        }
        crate::ShapePaint::Unsupported { .. } => Err(PaintDiagnosticKind::UnsupportedPaint),
    }
}

fn shape_outline(
    svg: &mut Scene,
    style: &crate::ShapeStyle,
    (mut paint, opacity): (Paint, f64),
    source_offset: Option<usize>,
    object_uuid: &str,
) -> Group {
    let dash = if matches!(paint, Paint::None) {
        None
    } else {
        style.dash_pattern().unwrap_or_else(|()| {
            svg.report_paint_issue(PaintDiagnostic {
                source_offset,
                object_uuid: object_uuid.into(),
                role: PaintRole::Outline,
                kind: PaintDiagnosticKind::UnsupportedDashPattern,
            });
            paint = Paint::None;
            None
        })
    };
    let cap = match style.cap {
        1 => LineCap::Round,
        2 => LineCap::Square,
        _ => LineCap::Butt,
    };
    let join = match style.join {
        1 => LineJoin::Round,
        2 => LineJoin::Bevel,
        _ => LineJoin::Miter,
    };
    let width = if style.width.is_finite() {
        style.width.max(0.0)
    } else {
        0.0
    };
    let group = Group::new()
        .stroke(paint)
        .stroke_opacity(decimal(opacity, 4))
        .stroke_width(width)
        .line_cap(cap)
        .line_join(join);
    match dash {
        Some(dash) => group.stroke_dash(dash.intervals(), dash.phase),
        None => group,
    }
}

fn render_image(
    svg: &mut Scene,
    bbox: BoundingBox,
    media_index: Option<usize>,
    rotation: Option<f64>,
    media_assets: &[MediaAsset],
) {
    let Some(asset) = media_index.and_then(|index| media_assets.get(index)) else {
        return;
    };
    render_image_data(svg, bbox, rotation, &asset.data, &asset.mime_type);
}

fn render_image_data(
    svg: &mut Scene,
    bbox: BoundingBox,
    rotation: Option<f64>,
    data: &[u8],
    mime_type: &str,
) {
    let width = bbox.x_max - bbox.x_min;
    let height = bbox.y_max - bbox.y_min;
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return;
    }
    let mut image = Image::embedded(data, mime_type)
        .x(bbox.x_min)
        .y(bbox.y_min)
        .width(width)
        .height(height)
        .stretched();
    if let Some(angle) = rotation.filter(|angle| angle.is_finite()) {
        image = image.transformed(Transform::rotate_unrounded(
            angle,
            bbox.x_min + width / 2.,
            bbox.y_min + height / 2.,
        ));
    }
    svg.push(image);
}

fn image_drawn_bbox(image: &PlacedImage) -> BoundingBox {
    let bbox = image.bbox;
    let (sin, cos) = image.rotation_degrees.unwrap_or(0.0).to_radians().sin_cos();
    let width = bbox.x_max - bbox.x_min;
    let height = bbox.y_max - bbox.y_min;
    let drawn_width = width * cos.abs() + height * sin.abs();
    let drawn_height = width * sin.abs() + height * cos.abs();
    let cx = (bbox.x_min + bbox.x_max) / 2.0;
    let cy = (bbox.y_min + bbox.y_max) / 2.0;
    BoundingBox {
        x_min: cx - drawn_width / 2.0,
        y_min: cy - drawn_height / 2.0,
        x_max: cx + drawn_width / 2.0,
        y_max: cy + drawn_height / 2.0,
    }
}

fn render_placed_image(svg: &mut Scene, image: &PlacedImage, media_assets: &[MediaAsset]) {
    let alpha = placed_image_alpha(image.fill_effect_transparency);
    if alpha == 0 {
        return;
    }
    if alpha < 255 {
        svg.scope(Group::new().opacity(f64::from(alpha) / 255.0), |svg| {
            render_placed_image_content(svg, image, media_assets);
        });
    } else {
        render_placed_image_content(svg, image, media_assets);
    }
}

// The native Drawing path computes 255 - transparency * 255 as one f32 fused
// operation, converts toward zero, then clamps to a byte. Nonpositive values
// bypass the setter and retain the opaque paint alpha.
fn placed_image_alpha(transparency: f32) -> u8 {
    if transparency.is_nan() || transparency <= 0.0 {
        return 255;
    }
    let scaled = (-transparency).mul_add(255.0, 255.0);
    (scaled as i32).clamp(0, 255) as u8
}

fn render_placed_image_content(svg: &mut Scene, image: &PlacedImage, media_assets: &[MediaAsset]) {
    let simple_original_crop = image.original_bbox.is_none()
        || (image.border_media_id.is_none() && image.original_media_id.is_none());
    if simple_original_crop
        && image
            .rotation_degrees
            .is_none_or(|rotation| rotation.is_finite() && rotation == 0.0)
        && let (Some(crop), Some(asset)) = (
            image.crop_rect,
            image.media_index.and_then(|index| media_assets.get(index)),
        )
        && asset.mime_type == "image/png"
        && let Some(cropped_png) = image_crop::crop_rgb8_or_rgba8_png(&asset.data, crop)
    {
        // Keep this an SVG image at the displayed bounds. Only exact source
        // pixels are copied, so PDF export retains its image XObject.
        render_image_data(svg, image.bbox, None, &cropped_png, "image/png");
        return;
    }

    render_placed_image_body(svg, image, media_assets);
}

fn render_placed_image_body(svg: &mut Scene, image: &PlacedImage, media_assets: &[MediaAsset]) {
    let original = image.crop_rect.and_then(|crop| {
        image.original_bbox.or_else(|| {
            let asset = media_assets.get(image.media_index?)?;
            image_crop::crop_only_placement(&asset.data, crop, image.bbox)
        })
    });
    if let Some(original) = original {
        let bbox = image.bbox;
        let width = bbox.x_max - bbox.x_min;
        let height = bbox.y_max - bbox.y_min;
        if width <= 0.0 || height <= 0.0 || !width.is_finite() || !height.is_finite() {
            return;
        }
        let angle = image.rotation_degrees.unwrap_or(0.0);
        let cx = (bbox.x_min + bbox.x_max) / 2.0;
        let cy = (bbox.y_min + bbox.y_max) / 2.0;
        svg.scope(
            Group::new().transformed(Transform::rotate_unrounded(angle, cx, cy)),
            |svg| {
                svg.scope(
                    Svg::new()
                        .x(bbox.x_min)
                        .y(bbox.y_min)
                        .width(width)
                        .height(height)
                        .view_box(ViewBox::unrounded(bbox.x_min, bbox.y_min, width, height))
                        .clipped_viewport(),
                    |svg| {
                        render_image(svg, original, image.media_index, None, media_assets);
                    },
                );
            },
        );
    } else {
        render_image(
            svg,
            image.bbox,
            image.media_index,
            image.rotation_degrees,
            media_assets,
        );
    }
}

fn render_text_box(
    svg: &mut Scene,
    text_box: &RichTextBox,
    page: &Page,
    media_assets: &[MediaAsset],
    flow_page_padding: Option<(u32, u32)>,
    theme: RenderTheme,
    renderer: &TextRenderer<'_>,
) {
    let is_note_body =
        text_box.bbox.x_max <= text_box.bbox.x_min || text_box.bbox.y_max <= text_box.bbox.y_min;
    if is_note_body {
        render_flow_text_box(
            svg,
            text_box,
            page,
            media_assets,
            flow_page_padding,
            theme,
            renderer,
        );
        return;
    }
    render_placed_text(
        svg,
        text_box,
        placed::PlacedTextFrame::from_text_box(text_box),
        media_assets,
        theme,
        renderer,
    );
}

fn render_placed_text(
    svg: &mut Scene,
    text_box: &RichTextBox,
    frame: placed::PlacedTextFrame,
    media_assets: &[MediaAsset],
    theme: RenderTheme,
    renderer: &TextRenderer<'_>,
) {
    let has_background_spans = text_box
        .spans
        .iter()
        .any(|span| span.kind == crate::RichTextSpanType::BackgroundColor);
    let legacy_highlight = text_box.highlight_color.filter(|_| !has_background_spans);
    let theme = legacy_highlight.map_or(theme, |color| theme.on_background(color));
    let styled = StyledText::new(text_box, TextContext::Placed, renderer.settings);
    let text_frame = text::TextFrame {
        bbox: frame.bounds,
        gravity: text_box.gravity,
        exclusions: &[],
    };
    let layout = match frame.measurement_size() {
        Some(size) => text::layout_text_with_size(&styled, text_frame, size, theme, renderer),
        None => text::layout_text(&styled, text_frame, theme, renderer),
    };
    let mut group = Group::new();
    if let Some(rotation) = frame.rotation {
        group = group.transformed(Transform::rotate(
            rotation.degrees,
            rotation.center[0],
            rotation.center[1],
            2,
        ));
    }
    svg.scope(group, |svg| {
        if let Some(highlight) = legacy_highlight.as_ref() {
            svg.push(rectangle(text_box.bbox, 0., 2).fill(Paint::from_hex(&color_hex(highlight))));
        }
        if has_background_spans {
            let clip = svg.definition::<Clip>();
            svg.push(Definitions::new().add(ClipPath::new(&clip).add(rectangle(
                frame.background_bounds,
                0.0,
                5,
            ))));
            svg.scope(Group::new().clipped(&clip), |svg| {
                paint_text_backgrounds(
                    svg,
                    &styled,
                    &layout,
                    theme,
                    renderer,
                    text::NativePaintTarget::new(
                        svg.retains_text(),
                        Some(Viewport::new(frame.background_bounds)),
                    ),
                );
            });
            let target = text::NativePaintTarget::new(svg.retains_text(), None);
            paint_text_foreground(svg, &styled, &layout, media_assets, theme, renderer, target);
        } else {
            paint_text_layout(svg, &styled, &layout, media_assets, theme, renderer);
        }
    });
}

fn paint_text_layout(
    svg: &mut Scene,
    styled: &StyledText<'_>,
    layout: &text::TextLayout,
    media_assets: &[MediaAsset],
    theme: RenderTheme,
    renderer: &TextRenderer<'_>,
) {
    paint_text_layout_in_viewport(svg, styled, layout, media_assets, theme, renderer, None);
}

fn paint_text_layout_in_viewport(
    svg: &mut Scene,
    styled: &StyledText<'_>,
    layout: &text::TextLayout,
    media_assets: &[MediaAsset],
    theme: RenderTheme,
    renderer: &TextRenderer<'_>,
    viewport: Option<Viewport>,
) {
    let target = text::NativePaintTarget::new(svg.retains_text(), viewport);
    paint_text_layout_with_target(svg, styled, layout, media_assets, theme, renderer, target);
}

fn paint_text_layout_with_target(
    svg: &mut Scene,
    styled: &StyledText<'_>,
    layout: &text::TextLayout,
    media_assets: &[MediaAsset],
    theme: RenderTheme,
    renderer: &TextRenderer<'_>,
    target: text::NativePaintTarget,
) {
    target.paint_conservative(svg, |svg| {
        paint_text_backgrounds(svg, styled, layout, theme, renderer, target);
    });
    paint_text_foreground(svg, styled, layout, media_assets, theme, renderer, target);
}

fn paint_text_backgrounds(
    svg: &mut Scene,
    styled: &StyledText<'_>,
    layout: &text::TextLayout,
    theme: RenderTheme,
    renderer: &TextRenderer<'_>,
    target: text::NativePaintTarget,
) {
    for (index, line) in layout.lines.iter().enumerate() {
        if !target.includes_line(index) {
            continue;
        }
        for issue in text::render_line_backgrounds(svg, styled, line, theme, target.viewport) {
            let scoped = match issue {
                text::SourceOwner::Text(source) => renderer.for_source(source),
                text::SourceOwner::Object(source) => renderer.for_object_source(source),
            };
            scoped.report_geometry_issues(&[TextDiagnostic {
                kind: TextDiagnosticKind::UnsupportedBackgroundPositioning,
                family: String::new(),
                codepoints: Vec::new(),
            }]);
        }
    }
}

fn paint_text_foreground(
    svg: &mut Scene,
    styled: &StyledText<'_>,
    layout: &text::TextLayout,
    media_assets: &[MediaAsset],
    theme: RenderTheme,
    renderer: &TextRenderer<'_>,
    target: text::NativePaintTarget,
) {
    let viewport = target.viewport;
    let visible_lines = layout
        .lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            target.includes_line(index)
                && viewport.is_none_or(|viewport| viewport.text_visible(styled, line, theme))
        })
        .collect::<Vec<_>>();
    let mut native_paint =
        text::NativePaintDispatcher::new(styled, layout, theme, renderer, &visible_lines, target);
    svg.text_source(|svg| {
        for (index, line) in layout.lines.iter().enumerate() {
            if !target.includes_line(index) {
                continue;
            }
            if let Some(marker) = &line.marker
                && viewport.is_none_or(|viewport| {
                    marker
                        .marker
                        .bounds(marker.x, marker.center_y)
                        .is_some_and(|bounds| viewport.intersects(bounds))
                })
            {
                let style = styled.style_at(marker.source, theme, line.predefined);
                target.paint_conservative(svg, |svg| {
                    svg.text_marker(marker.source, |svg| {
                        paint_positioned_marker(svg, marker, &style, theme, renderer);
                    });
                });
            }
            if visible_lines[index] {
                renderer.report_visible_line_issues(styled, &line.line);
                if let Some(native) = &mut native_paint {
                    native.paint_line(svg, styled, index, line, theme, renderer);
                } else {
                    target.paint_conservative(svg, |svg| {
                        render_measured_line(
                            svg,
                            styled,
                            &line.line,
                            line.x,
                            line.width,
                            line.baseline,
                            line.alignment,
                            theme,
                            line.predefined,
                            renderer,
                        );
                    });
                }
            }
            target.paint_conservative(svg, |svg| {
                paint_line_objects(
                    svg,
                    styled,
                    &line.line,
                    layout.native_object_entry,
                    line.x,
                    line.width,
                    line.baseline,
                    line.alignment,
                    line.predefined,
                    media_assets,
                    theme,
                    renderer,
                    viewport,
                );
            });
        }
    });
}

#[allow(clippy::too_many_arguments)]
fn paint_line_objects(
    svg: &mut Scene,
    styled: &StyledText<'_>,
    line: &text::WrappedLine,
    native_entry: Option<text::NativeObjectEntryBounds>,
    left: f64,
    available_width: f64,
    baseline: f64,
    alignment: Option<ParagraphAlignment>,
    predefined: Option<PredefinedTextStyle>,
    media_assets: &[MediaAsset],
    theme: RenderTheme,
    renderer: &TextRenderer<'_>,
    viewport: Option<Viewport>,
) {
    let canonical = svg.retains_text();
    let advance = line.advance_for_paint(canonical).unwrap_or(line.advance);
    let left = left + text::line_alignment_offset(advance, available_width, alignment);
    for (index, placement) in line.objects.iter().enumerate() {
        let Some(position) = line.object_position(index, canonical) else {
            continue;
        };
        let placement_x = position.x;
        let object = &placement.object;
        let Some(span) = styled.object_span(object.span_index) else {
            continue;
        };
        if matches!(placement.prepared, Some(Err(_))) {
            continue;
        }
        svg.inline_text_object(object.source.start, |svg| {
            let height = object.height;
            let drawing_renderer = renderer
                .for_object_source(object.source.clone())
                .planning_scope();
            let body_clone = object.context == text::ObjectMeasurementContext::Body
                && match &span.content {
                    Some(RichTextObjectContent::CodeBlock(code)) => {
                        code.rotation_degrees.is_none_or(|rotation| rotation == 0.0)
                    }
                    Some(RichTextObjectContent::Table(table)) => table
                        .rotation_degrees
                        .is_none_or(|rotation| rotation == 0.0),
                    _ => false,
                };
            let target = BoundingBox {
                x_min: left + placement_x,
                x_max: left + placement_x + object.width(),
                y_min: baseline - height,
                y_max: baseline,
            };
            let drawing_code = match (&placement.prepared, &span.content) {
                (
                    Some(Ok(embedded::PreparedObject::Code(_))),
                    Some(RichTextObjectContent::CodeBlock(code)),
                ) => Some(if body_clone {
                    code::prepare_code_drawing(
                        code,
                        span.layout_constraint,
                        target,
                        theme,
                        &drawing_renderer,
                    )
                } else {
                    code::prepare_code(
                        code,
                        span.layout_constraint,
                        baseline - height,
                        theme,
                        &drawing_renderer,
                    )
                }),
                _ => None,
            };
            let drawing_table = match &span.content {
                Some(RichTextObjectContent::Table(table)) => {
                    if body_clone {
                        let native_source = native_entry
                            .filter(|_| {
                                renderer
                                    .table_export_page
                                    .is_none_or(table::TableExportPage::has_zero_origin)
                            })
                            .and_then(|entry| {
                                let Some(Ok(embedded::PreparedObject::Table(prepared))) =
                                    &placement.prepared
                                else {
                                    return None;
                                };
                                Some(table::NativeTableDocumentSource::from_entry(
                                    entry, prepared,
                                ))
                            });
                        table::prepare_table_clone_drawing_with_native_entry(
                            table,
                            span.layout_constraint,
                            target,
                            theme,
                            &drawing_renderer,
                            native_source,
                        )
                    } else {
                        table::prepare_table_drawing(
                            table,
                            span.layout_constraint,
                            [left + placement_x, baseline - height],
                            theme,
                            &drawing_renderer,
                        )
                    }
                }
                _ => None,
            };
            let mut paint_bounds =
                object_paint_bounds(placement, left + placement_x - placement.x, baseline);
            if let Some(Ok(prepared)) = &drawing_code {
                if body_clone {
                    paint_bounds = prepared.panel_bbox;
                } else {
                    paint_bounds.y_max =
                        paint_bounds.y_min + prepared.panel_bbox.y_max - prepared.panel_bbox.y_min;
                }
            }
            if let Some(Ok(prepared)) = &drawing_table {
                paint_bounds = prepared.measured_bbox;
            }
            let native_table = drawing_table.as_ref().is_some_and(|prepared| {
                prepared
                    .as_ref()
                    .is_ok_and(|table| has_native_table_clipping(&table.rows))
            });
            if !native_table && viewport.is_some_and(|viewport| !viewport.intersects(paint_bounds))
            {
                return;
            }
            if body_clone
                && let (Some(Ok(prepared)), Some(RichTextObjectContent::CodeBlock(code))) =
                    (&drawing_code, &span.content)
            {
                render_prepared_code(svg, code, prepared, media_assets, theme, renderer, viewport);
                report_drawing_layout_issues(renderer, &drawing_renderer);
                return;
            }
            if let (Some(Ok(prepared)), Some(RichTextObjectContent::Table(table))) =
                (&drawing_table, &span.content)
            {
                render_table(
                    svg,
                    table,
                    span.text_index_utf16,
                    Some(prepared.into()),
                    0.0,
                    media_assets,
                    theme,
                    renderer,
                    viewport,
                );
                report_drawing_layout_issues(renderer, &drawing_renderer);
                return;
            }
            report_drawing_layout_issues(renderer, &drawing_renderer);
            for kind in [
                drawing_code
                    .as_ref()
                    .and_then(|result| result.as_ref().err()),
                drawing_table
                    .as_ref()
                    .and_then(|result| result.as_ref().err()),
            ]
            .into_iter()
            .flatten()
            {
                renderer.report_object_issues(&[ObjectDiagnostic {
                    anchor_utf16: span.text_index_utf16,
                    kind: *kind,
                }]);
            }
            if body_clone
                && (matches!(drawing_code, Some(Err(_))) || matches!(drawing_table, Some(Err(_))))
            {
                render_flow_line(
                    svg,
                    styled,
                    object.source.clone(),
                    target.x_min,
                    target.x_max,
                    baseline,
                    None,
                    theme,
                    predefined,
                    renderer,
                );
                return;
            }
            let offset = (
                left + placement_x - object.bounds.x_min,
                baseline - height - object.bounds.y_min,
            );
            svg.scope(
                Group::new().transformed(Transform::translate(offset.0, 0.0, 5)),
                |svg| {
                    if let (
                        Some(Ok(embedded::PreparedObject::Code(prepared))),
                        Some(RichTextObjectContent::CodeBlock(code)),
                    ) = (&placement.prepared, &span.content)
                    {
                        let prepared = drawing_code
                            .as_ref()
                            .and_then(|result| result.as_ref().ok())
                            .unwrap_or(prepared);
                        let shift = baseline - height - prepared.panel_bbox.y_min;
                        svg.scope(
                            Group::new().transformed(Transform::translate(0.0, shift, 5)),
                            |svg| {
                                render_prepared_code(
                                    svg,
                                    code,
                                    prepared,
                                    media_assets,
                                    theme,
                                    renderer,
                                    viewport.map(|viewport| viewport.translated(-offset.0, -shift)),
                                );
                            },
                        );
                    } else if let (
                        Some(Ok(embedded::PreparedObject::Table(prepared))),
                        Some(RichTextObjectContent::Table(table)),
                    ) = (&placement.prepared, &span.content)
                    {
                        let shift = (object.bounds.x_min, baseline - height);
                        svg.scope(
                            Group::new().transformed(Transform::translate(shift.0, shift.1, 5)),
                            |svg| {
                                render_table(
                                    svg,
                                    table,
                                    span.text_index_utf16,
                                    Some(prepared.as_ref().into()),
                                    0.0,
                                    media_assets,
                                    theme,
                                    &renderer.translated_paint(-offset.0 - shift.0, -shift.1),
                                    viewport.map(|viewport| {
                                        viewport.translated(-offset.0 - shift.0, -shift.1)
                                    }),
                                );
                            },
                        );
                    } else if render_embedded_object(
                        svg,
                        span,
                        offset.1,
                        media_assets,
                        theme,
                        &renderer
                            .for_object_source(object.source.clone())
                            .translated_paint(-offset.0, 0.0),
                        viewport.map(|viewport| viewport.translated(-offset.0, 0.0)),
                    )
                    .is_none()
                    {
                        render_flow_line(
                            svg,
                            styled,
                            object.source.clone(),
                            object.bounds.x_min,
                            object.bounds.x_min,
                            baseline,
                            None,
                            theme,
                            predefined,
                            renderer,
                        );
                    }
                },
            );
        });
    }
}

fn aligned_line_left(
    line: &text::WrappedLine,
    left: f64,
    width: f64,
    alignment: Option<ParagraphAlignment>,
) -> f64 {
    left + text::line_alignment_offset(line.advance, width, alignment)
}

fn object_paint_bounds(object: &text::PositionedObject, left: f64, baseline: f64) -> BoundingBox {
    let (x_min, width, height) = match &object.prepared {
        Some(Ok(embedded::PreparedObject::Code(prepared))) => (
            left + object.x + prepared.panel_bbox.x_min - object.object.bounds.x_min,
            prepared.panel_bbox.x_max - prepared.panel_bbox.x_min,
            prepared.panel_bbox.y_max - prepared.panel_bbox.y_min,
        ),
        Some(Ok(embedded::PreparedObject::Table(prepared))) => (
            left + object.x,
            prepared.measured_bbox.x_max - prepared.measured_bbox.x_min,
            prepared.measured_bbox.y_max - prepared.measured_bbox.y_min,
        ),
        _ => (
            left + object.x,
            object.object.bounds.x_max - object.object.bounds.x_min,
            object.object.height,
        ),
    };
    let y_min = baseline - object.object.height;
    BoundingBox {
        x_min,
        x_max: x_min + width,
        y_min,
        y_max: y_min + height,
    }
}

fn render_text_frame(
    svg: &mut Scene,
    text_box: &RichTextBox,
    frame: text::TextFrame<'_>,
    media_assets: &[MediaAsset],
    theme: RenderTheme,
    renderer: &TextRenderer<'_>,
    viewport: Option<Viewport>,
) -> f64 {
    let styled = StyledText::new(text_box, TextContext::Flow, renderer.settings);
    let planner = renderer.planning_scope();
    let layout = text::layout_text(&styled, frame, theme, &planner);
    let sources =
        viewport.map(|viewport| VisibleTextSources::new(&styled, &layout, viewport, theme, None));
    paint_text_layout_in_viewport(
        svg,
        &styled,
        &layout,
        media_assets,
        theme,
        renderer,
        viewport,
    );
    renderer.report_text_issues(
        &planner
            .scoped_diagnostics()
            .into_iter()
            .filter_map(|issue| {
                is_preparation_issue(&issue.diagnostic)
                    .then(|| {
                        sources.as_ref().map_or_else(
                            || Some(issue.clone()),
                            |sources| sources.project_issue(&styled, &issue),
                        )
                    })
                    .flatten()
            })
            .collect::<Vec<_>>(),
    );
    renderer.report_owned_object_issues(
        &planner
            .scoped_object_diagnostics()
            .into_iter()
            .filter(|issue| {
                sources
                    .as_ref()
                    .is_none_or(|sources| sources.contains(issue.owner.as_ref(), false))
            })
            .collect::<Vec<_>>(),
    );
    layout.height()
}

const FLOW_HORIZONTAL_PADDING: f64 = 48.0;

fn is_preparation_issue(issue: &TextDiagnostic) -> bool {
    matches!(
        issue.kind,
        TextDiagnosticKind::MeasurementFailure
            | TextDiagnosticKind::InvalidGeometry
            | TextDiagnosticKind::UnsupportedCompositionStyle
            | TextDiagnosticKind::UnsupportedSuggestionStyle
            | TextDiagnosticKind::UnsupportedCorrectionStyle
    )
}

fn render_flow_text_box(
    svg: &mut Scene,
    text_box: &RichTextBox,
    page: &Page,
    media_assets: &[MediaAsset],
    flow_page_padding: Option<(u32, u32)>,
    theme: RenderTheme,
    renderer: &TextRenderer<'_>,
) {
    let horizontal_padding = flow_page_padding
        .map(|(horizontal, _)| f64::from(horizontal))
        .unwrap_or(FLOW_HORIZONTAL_PADDING);
    let styled = StyledText::new(text_box, TextContext::Flow, renderer.settings);
    let layout = text::layout_flow_text(
        &styled,
        text::TextFrame {
            bbox: BoundingBox {
                x_min: horizontal_padding,
                y_min: 0.0,
                x_max: f64::from(page.width) - horizontal_padding,
                y_max: 0.0,
            },
            gravity: None,
            exclusions: &[],
        },
        theme,
        renderer,
    );
    svg.scope(Group::new().flow(), |svg| {
        paint_text_layout(svg, &styled, &layout, media_assets, theme, renderer);
    });
}

fn paint_positioned_marker(
    svg: &mut Scene,
    marker: &text::PositionedMarker,
    style: &TextStyle,
    theme: RenderTheme,
    renderer: &TextRenderer<'_>,
) {
    if marker
        .marker
        .paint(
            svg,
            marker.x,
            marker.center_y,
            &style.color,
            theme,
            renderer,
        )
        .is_none()
    {
        renderer.invalid_geometry("sans-serif");
    }
}

#[allow(clippy::too_many_arguments)]
fn render_flow_line(
    svg: &mut Scene,
    styled: &StyledText<'_>,
    range: Range<usize>,
    left: f64,
    right: f64,
    baseline: f64,
    alignment: Option<ParagraphAlignment>,
    theme: RenderTheme,
    predefined_style: Option<PredefinedTextStyle>,
    renderer: &TextRenderer<'_>,
) {
    if range.is_empty() {
        return;
    }
    #[cfg(feature = "pdf")]
    if svg.retains_text() {
        svg.reject_native_text("text fallback has no retained glyph plan".into());
        return;
    }
    let (x, anchor) = match alignment {
        Some(ParagraphAlignment::Center) => ((left + right) / 2.0, TextAnchor::Middle),
        Some(ParagraphAlignment::Right) => (right, TextAnchor::End),
        _ => (left, TextAnchor::Start),
    };
    svg.scope(
        Text::new("")
            .x(decimal(x, 2))
            .y(decimal(baseline, 2))
            .anchor(anchor)
            .family(FontFamily::Roboto)
            .preserve_space(),
        |svg| {
            for segment in styled.foreground_segments(range.clone()) {
                let style = styled.style_at(segment.start, theme, predefined_style);
                write_styled_tspan(
                    svg,
                    styled.index.slice(segment).unwrap(),
                    &style,
                    styled.context(),
                    renderer,
                );
            }
        },
    );
}

fn write_styled_tspan(
    svg: &mut Scene,
    text: &str,
    style: &TextStyle,
    context: TextContext,
    renderer: &TextRenderer<'_>,
) {
    let style = renderer.output_style(text, style, context);
    let span = styled_tspan(text, &style, context);
    push_text_span(svg, span, &style);
}

fn styled_tspan(text: &str, style: &TextStyle, context: TextContext) -> TSpan {
    let mut span = TSpan::new(text)
        .fill(Paint::from_hex(&style.color))
        .font_size(decimal(style.font_size, 2));
    if let Some(family) = style.family.as_deref() {
        span = span.family(FontFamily::Named(family));
    }
    let decoration = match (style.underline, style.strikethrough) {
        (true, true) => Some(TextDecoration::Both),
        (true, false) => Some(TextDecoration::Underline),
        (false, true) => Some(TextDecoration::StrikeThrough),
        (false, false) => None,
    };
    if let Some(decoration) = decoration {
        span = span.decoration(decoration);
    }
    if style.bold {
        span = match context {
            TextContext::Placed => span.bold(),
            TextContext::Flow => span
                .stroke(Paint::from_hex(style.color.as_str()))
                .stroke_width(0.45)
                .stroke_under_fill(),
        };
    }
    if style.italic {
        span = span.italic();
    }
    span
}

fn push_text_span(svg: &mut Scene, span: TSpan, style: &TextStyle) {
    if let Some(target) = &style.link_target {
        svg.scope(Anchor::new(target), |svg| svg.push(span));
    } else {
        svg.push(span);
    }
}

fn render_embedded_object(
    svg: &mut Scene,
    object: &RichTextObjectSpan,
    offset_y: f64,
    media_assets: &[MediaAsset],
    theme: RenderTheme,
    renderer: &TextRenderer<'_>,
    viewport: Option<Viewport>,
) -> Option<f64> {
    match object.content.as_ref() {
        Some(RichTextObjectContent::Image(image)) => {
            let drawn = image_drawn_bbox(image);
            if ![drawn.x_min, drawn.y_min, drawn.x_max, drawn.y_max]
                .iter()
                .all(|value| value.is_finite())
                || drawn.x_max <= drawn.x_min
                || drawn.y_max <= drawn.y_min
            {
                return None;
            }
            svg.scope(
                Group::new()
                    .object(ObjectKind::Image)
                    .transformed(Transform::translate(0., offset_y, 4)),
                |svg| {
                    render_placed_image(svg, image, media_assets);
                },
            );
            Some(drawn.y_max + offset_y)
        }
        Some(RichTextObjectContent::Table(table)) => {
            render_table(
                svg,
                table,
                object.text_index_utf16,
                None,
                offset_y,
                media_assets,
                theme,
                renderer,
                viewport,
            );
            Some(table.bbox.y_max + offset_y)
        }
        Some(RichTextObjectContent::CodeBlock(code)) => {
            let prepared = match code::prepare_code(
                code,
                object.layout_constraint,
                code.bbox.y_min + offset_y,
                theme,
                renderer,
            ) {
                Ok(prepared) => prepared,
                Err(kind) => {
                    renderer.report_object_issues(&[text::ObjectDiagnostic {
                        anchor_utf16: object.text_index_utf16,
                        kind,
                    }]);
                    return None;
                }
            };
            render_prepared_code(
                svg,
                code,
                &prepared,
                media_assets,
                theme,
                renderer,
                viewport,
            );
            Some(prepared.panel_bbox.y_max)
        }
        None => None,
    }
}

fn report_drawing_layout_issues(renderer: &TextRenderer<'_>, drawing: &TextRenderer<'_>) {
    renderer.report_text_issues(
        &drawing
            .scoped_diagnostics()
            .into_iter()
            .filter(|issue| is_preparation_issue(&issue.diagnostic))
            .collect::<Vec<_>>(),
    );
    renderer.report_owned_object_issues(&drawing.scoped_object_diagnostics());
}

struct TablePaint<'a> {
    measured_bbox: BoundingBox,
    content_bbox: BoundingBox,
    rows: &'a [table::PreparedTableRow],
    pending_gaps: &'a [f64],
    artwork: table::TableArtworkGeometry,
}

fn has_native_table_clipping(rows: &[table::PreparedTableRow]) -> bool {
    rows.iter().flat_map(|row| &row.cells).any(|cell| {
        cell.native_text_placement()
            .is_ok_and(|placement| placement.transport().is_some())
    })
}

impl<'a> From<&'a table::PreparedTable> for TablePaint<'a> {
    fn from(table: &'a table::PreparedTable) -> Self {
        Self {
            measured_bbox: table.measured_bbox,
            content_bbox: table.content_bbox,
            rows: &table.rows,
            pending_gaps: &table.pending_gaps,
            artwork: table::TableArtworkGeometry::Cached,
        }
    }
}

impl<'a> From<&'a table::PreparedTableDrawing> for TablePaint<'a> {
    fn from(table: &'a table::PreparedTableDrawing) -> Self {
        Self {
            measured_bbox: table.measured_bbox,
            content_bbox: table.content_bbox,
            rows: &table.rows,
            pending_gaps: &table.pending_gaps,
            artwork: table.artwork,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_table(
    svg: &mut Scene,
    table: &crate::RichTextTable,
    anchor_utf16: i32,
    prepared: Option<TablePaint<'_>>,
    offset_y: f64,
    media_assets: &[MediaAsset],
    theme: RenderTheme,
    renderer: &TextRenderer<'_>,
    viewport: Option<Viewport>,
) {
    let table_bbox = prepared
        .as_ref()
        .map_or(table.bbox, |table| table.measured_bbox);
    let artwork = prepared
        .as_ref()
        .map_or(table::TableArtworkGeometry::Unprepared, |prepared| {
            prepared.artwork
        });
    let grid = table::TableGrid::new(table);
    if prepared.is_none()
        && (grid
            .as_ref()
            .map_or(true, |grid| grid.requires_merged_layout())
            || !table::supports_prepared_content(table))
    {
        renderer.report_object_issues(&[ObjectDiagnostic {
            anchor_utf16,
            kind: ObjectDiagnosticKind::UnsupportedContent,
        }]);
    }
    let borders =
        grid.as_ref()
            .ok()
            .and_then(|grid| match table::TableBorderGeometry::new(table, grid) {
                Ok(borders) => Some(borders),
                Err(kind) => {
                    renderer.report_object_issues(&[ObjectDiagnostic { anchor_utf16, kind }]);
                    None
                }
            });
    let outline = borders
        .as_ref()
        .map(|borders| borders.outline())
        .unwrap_or_default();
    let content_bbox = prepared
        .as_ref()
        .map_or(table.style.content_bbox.unwrap_or(table.bbox), |prepared| {
            prepared.content_bbox
        });
    let mut cells = Vec::new();
    let mut add_cell = |row: usize, column: usize| {
        let cell = &table.rows[row].cells[column];
        let measured = if let Some(prepared) = &prepared {
            let Some(measured) = prepared
                .rows
                .get(row)
                .filter(|prepared| prepared.row_index == row)
                .and_then(|row| {
                    row.cells
                        .get(column)
                        .filter(|cell| cell.column_index == column)
                })
            else {
                return;
            };
            Some(measured)
        } else {
            None
        };
        let frame = measured.map_or(cell.bbox, |cell| cell.frame);
        let artwork_frame = artwork.background(cell.bbox, frame);
        if !artwork.cached()
            && (!table::finite_artwork_rect(cell.bbox)
                || !table::finite_artwork_rect(artwork_frame))
        {
            renderer.report_object_issues(&[ObjectDiagnostic {
                anchor_utf16,
                kind: ObjectDiagnosticKind::InvalidBounds,
            }]);
            return;
        }
        cells.push(TableCellPaint {
            cell,
            fill: table::CellFill::resolve(&table.style, table.rows[row].index, cell, theme),
            frame,
            artwork_frame,
            layout: measured.map(|cell| &cell.layout),
            native_text_placement: measured.map_or(
                Ok(table::native_placement::NativeCellTextPlacement::Unknown),
                |cell| cell.native_text_placement(),
            ),
            position: table::CellPosition { row, column },
            gap: prepared.as_ref().is_some_and(|prepared| {
                prepared
                    .pending_gaps
                    .get(row + 1)
                    .is_some_and(|gap| (*gap as f32).abs() > 0.001_f32)
            }),
        });
    };
    match &grid {
        Ok(grid) => {
            for position in grid.visible_cells() {
                add_cell(position.row, position.column);
            }
        }
        Err(_) => {
            for (row_index, row) in table.rows.iter().enumerate() {
                for column in 0..row.cells.len() {
                    add_cell(row_index, column);
                }
            }
        }
    }
    svg.scope(Group::new().object(ObjectKind::Table), |svg| {
        let clip = svg.definition::<Clip>();
        let artwork_clip = svg.definition::<Clip>();
        let measured_bounds = BoundingBox {
            y_min: table_bbox.y_min + offset_y,
            y_max: table_bbox.y_max + offset_y,
            ..table_bbox
        };
        let artwork_bounds = if prepared.is_some() {
            table::artwork_bounds(measured_bounds, renderer.table_export_page)
        } else {
            measured_bounds
        };
        svg.push(
            Definitions::new()
                .add(ClipPath::new(&clip).add(rectangle(measured_bounds, 0.0, 2)))
                .add(ClipPath::new(&artwork_clip).add(rectangle(artwork_bounds, 0.0, 5))),
        );
        svg.scope(Group::new().clipped(&artwork_clip), |svg| {
            let shape = [table.rows.len(), table.column_widths.len()];
            for paint in &cells {
                if viewport.is_some_and(|viewport| {
                    !viewport.intersects(paint.artwork_bounds(
                        borders.as_ref(),
                        offset_y,
                        prepared.is_some(),
                        artwork,
                    ))
                }) {
                    continue;
                }
                paint_table_cell_background(svg, paint, shape, [outline.rx, outline.ry], offset_y);
                if let Some(borders) = &borders {
                    for path in borders.cell_paths(paint.position) {
                        if path.selected(paint.position, shape, outline.active(), paint.gap) {
                            let path = artwork.border(path, paint.artwork_frame);
                            if !path.has_finite_endpoints() {
                                renderer.report_object_issues(&[ObjectDiagnostic {
                                    anchor_utf16,
                                    kind: ObjectDiagnosticKind::InvalidBounds,
                                }]);
                                continue;
                            }
                            paint_table_border(svg, path, offset_y, theme);
                        }
                    }
                }
            }
            if prepared.is_some() {
                paint_table_outline(svg, borders.as_ref(), content_bbox, offset_y, theme);
            }
        });
        if prepared.is_none() {
            paint_table_outline(svg, borders.as_ref(), content_bbox, offset_y, theme);
        }
        for paint in &cells {
            let native_clip = paint
                .native_text_placement
                .is_ok_and(|placement| placement.transport().is_some());
            if !native_clip
                && viewport.is_some_and(|viewport| {
                    !viewport.intersects(BoundingBox {
                        y_min: paint.frame.y_min + offset_y,
                        y_max: paint.frame.y_max + offset_y,
                        ..paint.frame
                    })
                })
            {
                continue;
            }
            let cell_theme = paint.fill.theme(theme);
            if let Some(layout) = paint.layout {
                let styled =
                    StyledText::new(&paint.cell.content, TextContext::Flow, renderer.settings);
                let mut target = text::NativePaintTarget::new(svg.retains_text(), viewport)
                    .with_cell(anchor_utf16, paint.native_text_placement);
                target.conservative_clip = Some(clip);
                paint_text_layout_with_target(
                    svg,
                    &styled,
                    layout,
                    media_assets,
                    cell_theme,
                    renderer,
                    target,
                );
            } else {
                let frame = text::TextFrame {
                    bbox: BoundingBox {
                        y_min: paint.frame.y_min + offset_y,
                        y_max: paint.frame.y_max + offset_y,
                        ..paint.frame
                    },
                    gravity: Some(0),
                    exclusions: &[],
                };
                svg.scope(Group::new().clipped(&clip), |svg| {
                    render_text_frame(
                        svg,
                        &paint.cell.content,
                        frame,
                        media_assets,
                        cell_theme,
                        renderer,
                        viewport,
                    );
                });
            }
        }
    });
}

struct TableCellPaint<'a> {
    cell: &'a crate::RichTextTableCell,
    fill: table::CellFill,
    frame: BoundingBox,
    artwork_frame: BoundingBox,
    layout: Option<&'a text::TextLayout>,
    native_text_placement: Result<
        table::native_placement::NativeCellTextPlacement,
        text::native_cell_clip::NativeCellClipError,
    >,
    position: table::CellPosition,
    gap: bool,
}

impl TableCellPaint<'_> {
    fn artwork_bounds(
        &self,
        borders: Option<&table::TableBorderGeometry>,
        offset_y: f64,
        prepared: bool,
        artwork: table::TableArtworkGeometry,
    ) -> BoundingBox {
        let guard = if prepared {
            borders.map_or(0.0, |borders| {
                borders
                    .cell_paths(self.position)
                    .into_iter()
                    .filter_map(|path| path.paint_width())
                    .fold(0.0_f32, f32::max)
                    / 2.0
            })
        } else {
            0.0
        };
        let guard = f64::from(guard);
        let mut bounds = BoundingBox {
            x_min: self.artwork_frame.x_min - guard,
            y_min: self.artwork_frame.y_min + offset_y - guard,
            x_max: self.artwork_frame.x_max + guard,
            y_max: self.artwork_frame.y_max + offset_y + guard,
        };
        if matches!(artwork, table::TableArtworkGeometry::Saved { .. })
            && let Some(borders) = borders
        {
            for path in borders.cell_paths(self.position) {
                let Some(width) = path.paint_width() else {
                    continue;
                };
                let path = artwork.border(path, self.artwork_frame);
                if !path.has_finite_endpoints() {
                    continue;
                }
                let [x1, y1, x2, y2] = path.endpoints.map(f64::from);
                let guard = f64::from(width) / 2.0;
                bounds.x_min = bounds.x_min.min(x1.min(x2) - guard);
                bounds.y_min = bounds.y_min.min(y1.min(y2) + offset_y - guard);
                bounds.x_max = bounds.x_max.max(x1.max(x2) + guard);
                bounds.y_max = bounds.y_max.max(y1.max(y2) + offset_y + guard);
            }
        }
        bounds
    }
}

fn paint_table_outline(
    svg: &mut Scene,
    borders: Option<&table::TableBorderGeometry>,
    bounds: BoundingBox,
    offset_y: f64,
    theme: RenderTheme,
) {
    let Some(borders) = borders else { return };
    let outline = borders.outline();
    if !outline.active() {
        return;
    }
    if outline.rounded() {
        svg.push(
            rectangle(bounds, offset_y, 5)
                .rx(decimal(f64::from(outline.rx), 5))
                .ry(decimal(f64::from(outline.ry), 5))
                .fill(Paint::None)
                .stroke(Paint::from_hex(&color_hex(
                    &theme.foreground_color(argb_color(outline.color)),
                )))
                .stroke_opacity(decimal(f64::from(outline.color >> 24) / 255.0, 6))
                .stroke_width(decimal(f64::from(outline.width), 5)),
        );
    } else {
        for path in borders.outer_paths(Some(bounds)) {
            paint_table_border(svg, path, offset_y, theme);
        }
    }
}

fn paint_table_cell_background(
    svg: &mut Scene,
    paint: &TableCellPaint<'_>,
    shape: [usize; 2],
    radii: [f32; 2],
    offset_y: f64,
) {
    let fill = Paint::from_hex(&color_hex(&paint.fill.color));
    let rectangle = |bounds, offset_y, places| {
        rectangle(bounds, offset_y, places).fill_opacity(decimal(paint.fill.opacity, 6))
    };
    let geometry = table::CellBackgroundGeometry::new(
        paint.artwork_frame,
        paint.position,
        [paint.cell.row_span, paint.cell.column_span],
        shape,
        radii,
    );
    let background = rectangle(geometry.bounds, offset_y, 5).fill(fill);
    if let Some([rx, ry]) = geometry.radii {
        svg.push(
            background
                .rx(decimal(f64::from(rx), 5))
                .ry(decimal(f64::from(ry), 5)),
        );
    } else {
        svg.push(background);
    }
    for bounds in geometry.square_corners.into_iter().flatten() {
        svg.push(rectangle(bounds, offset_y, 5).fill(fill));
    }
}

fn paint_table_border(svg: &mut Scene, path: table::BorderPath, offset_y: f64, theme: RenderTheme) {
    let Some(width) = path.paint_width() else {
        return;
    };
    let [x1, y1, x2, y2] = path.endpoints.map(f64::from);
    svg.push(
        Line::new()
            .x1(x1)
            .y1(y1 + offset_y)
            .x2(x2)
            .y2(y2 + offset_y)
            .stroke(Paint::from_hex(&color_hex(
                &theme.foreground_color(argb_color(path.style.color)),
            )))
            .stroke_opacity(decimal(f64::from(path.style.color >> 24) / 255.0, 6))
            .stroke_width(width),
    );
}

fn render_prepared_code(
    svg: &mut Scene,
    code: &crate::RichTextCodeBlock,
    prepared: &code::PreparedCode,
    media_assets: &[MediaAsset],
    theme: RenderTheme,
    renderer: &TextRenderer<'_>,
    viewport: Option<Viewport>,
) {
    let settings = renderer.settings;
    let background = argb_color(if theme.is_dark() { 0x333333 } else { 0xefefef });
    let theme = theme.on_background(background);
    let fill = color_hex(&background);
    let stroke = if theme.is_dark() {
        "#5f5f5f"
    } else {
        "#dddddd"
    };
    let copy_size = settings.pixels(24.0);
    let radius = settings.pixels(12.0);
    let copy = prepared.copy;
    let panel_bbox = prepared.panel_bbox;
    svg.scope(Group::new().object(ObjectKind::CodeBlock), |svg| {
        svg.push(
            rectangle(panel_bbox, 0.0, 2)
                .y(decimal(panel_bbox.y_min, 5))
                .rx(decimal(radius, 2))
                .fill(Paint::from_hex(&fill))
                .stroke(Paint::from_hex(stroke))
                .stroke_width(1),
        );
        if let (Some(title), Some(layout)) = (&code.title, &prepared.title_layout) {
            let styled = StyledText::with_span_producer(
                title,
                TextContext::Flow,
                settings,
                text::TextSpanProducer::Drawing,
            );
            paint_text_layout_in_viewport(
                svg,
                &styled,
                layout,
                media_assets,
                theme,
                renderer,
                viewport,
            );
        }
        let icon_stroke = if theme.is_dark() {
            "#b7b7b7"
        } else {
            "#8b8b8b"
        };
        let icon = Data::new()
            .move_to((31, 25))
            .vertical_line_by(-4)
            .quadratic_curve_by((0., -8., 8., -8.))
            .horizontal_line_by(17)
            .quadratic_curve_by((8., 0., 8., 8.))
            .vertical_line_by(29);
        let icon_scale = copy_size / 72.0;
        svg.push(
            Group::new()
                .transformed(Transform::matrix(
                    [icon_scale, 0.0, 0.0, icon_scale, copy.x_min, copy.y_min],
                    6,
                    2,
                ))
                .fill(Paint::None)
                .stroke(Paint::from_hex(icon_stroke))
                .stroke_width(6)
                .line_join(LineJoin::Round)
                .add(Path::new().data(icon))
                .add(Rectangle::new().x(15).y(23).width(31).height(38).rx(5)),
        );
        if let (Some(body), Some(layout)) = (&code.body, &prepared.body_layout) {
            let styled = StyledText::with_span_producer(
                body,
                TextContext::Flow,
                settings,
                text::TextSpanProducer::Drawing,
            );
            paint_text_layout_in_viewport(
                svg,
                &styled,
                layout,
                media_assets,
                theme,
                renderer,
                viewport,
            );
        }
    });
}

fn argb_color(argb: u32) -> Color {
    Color {
        r: (argb >> 16) as u8,
        g: (argb >> 8) as u8,
        b: argb as u8,
    }
}

fn magicpen_v1_vector_path(ink: &crate::ink::PreparedMagicPenV1, replay: bool) -> Option<Path> {
    const MAX_PATH_BYTES: usize = 8 * 1024 * 1024;
    if ink.records.is_empty()
        || ink.records.len() > 65_536
        || ink.sample_ends.is_empty()
        || ink.sample_ends.len() > 100_000
        || ink.sample_ends.first() != Some(&0)
        || ink.sample_ends.windows(2).any(|pair| pair[0] > pair[1])
        || ink.sample_ends.iter().any(|&end| end > ink.records.len())
        || ink.terminal_range.start > ink.terminal_range.end
        || ink.terminal_range.end != ink.records.len()
        || ink
            .sample_ends
            .last()
            .is_none_or(|&end| end > ink.terminal_range.start && end != ink.terminal_range.end)
    {
        return None;
    }
    let mut path = ReplayPath::default();
    // ReplaySvg resolves saved samples through PreparedStroke::sample_ends,
    // then indexes these prefixes by emitted stamp count, not source event.
    for record in &ink.records {
        let data = crate::ink::magicpen_source_contour_path(std::slice::from_ref(record))?;
        if !path.push_serialized(&data, replay, MAX_PATH_BYTES) {
            return None;
        }
    }
    Some(path.finish())
}

fn magicpen_source_color(stroke: &Stroke, argb: u32) -> Option<Color> {
    stroke.color.or(Some(argb_color(argb)))
}

#[allow(clippy::too_many_arguments)] // Carries the independent per-render pen profiles.
fn render_stroke(
    svg: &mut Scene,
    stroke: &Stroke,
    theme: RenderTheme,
    replay_index: Option<usize>,
    calligraphy_session: &mut crate::ink::calligraphy::CalligraphySession,
    native_allowed: bool,
    viewport: [u32; 2],
    inkpen_profile: Option<InkPenRenderProfile>,
    inkpen2_profile: Option<InkPen2RenderProfile>,
    chinesebrush_profile: Option<ChineseBrushRenderProfile>,
    chinesebrush_fwidth_output_profile: Option<ChineseBrushFwidthOutputProfile>,
    fountain_v17_rainbow_profile: Option<&FountainV17RainbowRenderProfile>,
    fountain_v17_output_scale_profile: Option<FountainV17OutputScaleProfile>,
    fountain_v16_output_scale_profile: Option<FountainV16OutputScaleProfile>,
    fountain_v14_output_scale_profile: Option<FountainV14OutputScaleProfile>,
    pencil3_profile: Option<Pencil3RenderProfile>,
    airbrush_profile: Option<AirBrushRenderProfile>,
    simplebrush_profile: Option<SimpleBrushRenderProfile>,
    simplebrush_remaining_stamps: &Cell<usize>,
    slice: Option<PageSlice>,
) {
    // Saved ObjectDrawing admission requires at least two source points before
    // pen-name dispatch. Keep the context-free prepare_stroke compatibility
    // path unchanged; this renderer only paints persisted page objects.
    if stroke.points.len() <= 1 {
        return;
    }
    if native_allowed
        && stroke.rendering.as_ref().is_some_and(|rendering| {
            rendering.pen_name.as_deref()
                == Some("com.samsung.android.sdk.pen.pen.preload.FountainPen")
                && rendering.properties.rainbow_effect
        })
        && let Some(profile) = fountain_v17_rainbow_profile
        && fountain_v17_output_scale_profile
            .is_none_or(|profile| valid_fountain_v17_output_scale(profile.pixels_per_page_unit))
    {
        let paint = crate::ink::prepare_stroke_for_context_with_fountain_v17_rainbow(
            stroke,
            crate::ink::InkRenderContext::CanonicalPage,
            &profile.palette_argb,
            profile.period,
            profile.initial_offset,
            fountain_v17_output_scale_profile.map(|profile| profile.pixels_per_page_unit),
        );
        if paint.fountain_v17_rgb.is_some() {
            if let Some(index) = replay_index {
                svg.scope(Group::new().replay_stroke(StrokeIndex(index)), |svg| {
                    render_prepared_stroke_with_fallback(svg, &paint, true, slice, || {
                        Some(prepare_context_stroke_with_theme(stroke, theme))
                    });
                });
            } else {
                render_prepared_stroke_with_fallback(svg, &paint, false, slice, || {
                    Some(prepare_context_stroke_with_theme(stroke, theme))
                });
            }
            return;
        }
    }
    if native_allowed
        && fountain_v17_rainbow_profile.is_none()
        && stroke.rendering.as_ref().is_some_and(|rendering| {
            rendering.pen_name.as_deref()
                == Some("com.samsung.android.sdk.pen.pen.preload.FountainPen")
                && rendering.advanced_settings.as_deref() == Some("18;0;100;")
                && rendering.tool_type_raw == 2
                && !rendering.properties.fixed_width
                && rendering.properties.stylus_channels
                && rendering.properties.rainbow_effect
        })
    {
        let output_scale = fountain_v16_output_scale_profile
            .map(|profile| profile.pixels_per_page_unit)
            .filter(|scale| valid_fountain_v16_output_scale(*scale));
        let paint = prepare_fountain_v16_rainbow_ignored_with_theme(stroke, theme, output_scale);
        if paint.support == crate::ink::InkSupport::Reconstructed
            && paint.dot_radii.is_some()
            && paint.fountain_v17_rgb.is_none()
        {
            if let Some(index) = replay_index {
                svg.scope(Group::new().replay_stroke(StrokeIndex(index)), |svg| {
                    render_prepared_stroke_with_fallback(svg, &paint, true, slice, || {
                        Some(prepare_context_stroke_with_theme(stroke, theme))
                    });
                });
            } else {
                render_prepared_stroke_with_fallback(svg, &paint, false, slice, || {
                    Some(prepare_context_stroke_with_theme(stroke, theme))
                });
            }
            return;
        }
    }
    if native_allowed
        && let Some(profile) = chinesebrush_profile
        && (chinesebrush_fwidth_output_profile.is_none_or(|output| {
            output.pixels_per_page_unit.is_finite() && output.pixels_per_page_unit > 0.0
        }))
        && let Some(ink) =
            chinesebrush::prepare(stroke, profile, chinesebrush_fwidth_output_profile)
    {
        chinesebrush::render(svg, &ink, replay_index);
        return;
    }
    if native_allowed
        && let Some(profile) = pencil3_profile
        && let Some(ink) = prepare_pencil3_v3(stroke, profile, theme, viewport)
    {
        let context = pencil3_paint_context(profile);
        let page_y_range = slice.map(|slice| {
            let top = f64::from(slice.y());
            (top, top + f64::from(slice.height()))
        });
        if pencil3::render_slice(svg, &ink, context, replay_index, page_y_range) {
            return;
        }
    }
    if native_allowed
        && airbrush_profile == Some(AirBrushRenderProfile::SavedV1)
        && let Some(paint) = airbrush::prepare_source(stroke, theme, viewport, true)
        && airbrush::render(svg, &paint, replay_index)
    {
        return;
    }
    if native_allowed
        && airbrush_profile == Some(AirBrushRenderProfile::SavedV2)
        && let Some(paint) = airbrush::prepare(stroke, theme, viewport)
        && airbrush::render(svg, &paint, replay_index)
    {
        return;
    }
    // The experimental renderer is deliberately disabled for replay until it
    // carries correct per-source-sample and terminal part boundaries.
    if native_allowed
        && replay_index.is_none()
        && simplebrush_profile == Some(SimpleBrushRenderProfile::HostV25)
        && viewport.iter().all(|dimension| *dimension > 0)
        && let Ok(paint) = crate::ink::simplebrush::prepare_stroke(
            stroke,
            true,
            simplebrush_remaining_stamps.get(),
        )
        && simplebrush::render(
            svg,
            &paint.samples,
            &theme.foreground(Some(paint.color)),
            slice.map(|slice| {
                let top = f64::from(slice.y());
                (top, top + f64::from(slice.height()))
            }),
        )
    {
        simplebrush_remaining_stamps.set(simplebrush_remaining_stamps.get() - paint.samples.len());
        return;
    }
    if native_allowed && let Some(ink) = prepare_inkpen2_v9(stroke, inkpen2_profile, theme) {
        render_inkpen2_v9(svg, &ink, replay_index);
        return;
    }
    if native_allowed
        && viewport.iter().all(|dimension| *dimension > 0)
        && let Some(ink) = crate::ink::prepare_magicpen_v1_source(stroke)
        && let Some(path) = magicpen_v1_vector_path(&ink, replay_index.is_some())
    {
        let color = theme.foreground(magicpen_source_color(stroke, ink.argb));
        let opacity = f64::from((ink.argb >> 24) as u8) / 255.0;
        let draw = |svg: &mut Scene| {
            svg.push(path.fill(Paint::from_hex(&color)));
        };
        let draw = |svg: &mut Scene| {
            if opacity == 1.0 {
                draw(svg);
            } else {
                svg.scope(Group::new().opacity(opacity), draw);
            }
        };
        if let Some(index) = replay_index {
            svg.scope(Group::new().replay_stroke(StrokeIndex(index)), draw);
        } else {
            draw(svg);
        }
        return;
    }
    if native_allowed
        && viewport.iter().all(|dimension| *dimension > 0)
        && let Some(ink) = prepare_page_inkpen(stroke, inkpen_profile)
        && let Some(vector) = inkpen_v1_vector_path(
            &ink.points,
            &ink.radii,
            &ink.sample_ends,
            ink.terminal.clone(),
            replay_index.is_some(),
        )
    {
        let color = theme.foreground(stroke.color.or(Some(argb_rgb(ink.argb))));
        let opacity = f64::from((ink.argb >> 24) as u8) / 255.0;
        let draw = |svg: &mut Scene| {
            svg.scope(Group::new().opacity(opacity), |svg| {
                svg.push(vector.path.fill(Paint::from_hex(&color)));
            });
        };
        if let Some(index) = replay_index {
            svg.scope(Group::new().replay_stroke(StrokeIndex(index)), draw);
        } else {
            draw(svg);
        }
        return;
    }
    if native_allowed {
        match crate::ink::calligraphy::prepare_model_stroke(calligraphy_session, stroke) {
            crate::ink::calligraphy::CalligraphyAdmission::Mesh(mesh) => {
                let mut outcome = None;
                if let Some(index) = replay_index {
                    svg.scope(Group::new().replay_stroke(StrokeIndex(index)), |svg| {
                        outcome = Some(calligraphy::render_model(svg, &mesh, true));
                    });
                } else {
                    outcome = Some(calligraphy::render_model(svg, &mesh, false));
                }
                if outcome == Some(Ok(())) {
                    return;
                }
            }
            crate::ink::calligraphy::CalligraphyAdmission::NotCalligraphy
            | crate::ink::calligraphy::CalligraphyAdmission::Unsupported(_) => {}
        }
    }
    let v16_output_scale = fountain_v16_output_scale_profile
        .map(|profile| profile.pixels_per_page_unit)
        .filter(|scale| valid_fountain_v16_output_scale(*scale));
    let mut paint = if native_allowed {
        if let Some(scale) = v16_output_scale {
            prepare_context_stroke_with_fountain_v16_scale(stroke, theme, scale)
        } else {
            prepare_context_stroke_with_theme(stroke, theme)
        }
    } else {
        crate::prepare_stroke(stroke, false)
    };
    if native_allowed
        && paint.profile == Some("FountainPen")
        && paint.dot_directions.is_some()
        && let Some(profile) = fountain_v14_output_scale_profile
        && valid_fountain_v14_output_scale(profile.pixels_per_page_unit)
    {
        paint.fountain_output_scale = Some(profile.pixels_per_page_unit);
    }
    if !native_allowed && paint.coverage_mesh.is_none() {
        let native_fountain_default_color = paint.profile == Some("FountainPen")
            && paint.support == crate::ink::InkSupport::Reconstructed
            && stroke.color.is_none()
            && stroke
                .rendering
                .as_ref()
                .and_then(|rendering| rendering.style.color_argb)
                .is_none();
        let color = if native_fountain_default_color {
            Some(Color { r: 0, g: 0, b: 0 })
        } else {
            stroke_source_color(stroke)
        };
        paint.color = theme.foreground(color);
    }
    let slice = if native_allowed { slice } else { None };
    if let Some(index) = replay_index {
        svg.scope(Group::new().replay_stroke(StrokeIndex(index)), |svg| {
            render_prepared_stroke(svg, &paint, true, slice);
        });
    } else {
        render_prepared_stroke(svg, &paint, false, slice);
    }
}

fn prepare_context_stroke_with_theme<'a>(
    stroke: &'a Stroke,
    theme: RenderTheme,
) -> crate::PreparedStroke<'a> {
    let mut paint = crate::ink::prepare_stroke_for_context(
        stroke,
        false,
        crate::ink::InkRenderContext::CanonicalPage,
    );
    apply_context_stroke_theme(stroke, theme, &mut paint);
    paint
}

fn prepare_fountain_v16_rainbow_ignored_with_theme<'a>(
    stroke: &'a Stroke,
    theme: RenderTheme,
    output_scale: Option<f32>,
) -> crate::PreparedStroke<'a> {
    let mut paint = if let Some(output_scale) = output_scale {
        crate::ink::prepare_stroke_for_context_with_fountain_v16_rainbow_ignored_output_scale(
            stroke,
            crate::ink::InkRenderContext::CanonicalPage,
            output_scale,
        )
    } else {
        crate::ink::prepare_stroke_for_context_with_fountain_v16_rainbow_ignored(
            stroke,
            crate::ink::InkRenderContext::CanonicalPage,
        )
    };
    apply_context_stroke_theme(stroke, theme, &mut paint);
    paint
}

fn prepare_context_stroke_with_fountain_v16_scale<'a>(
    stroke: &'a Stroke,
    theme: RenderTheme,
    output_scale: f32,
) -> crate::PreparedStroke<'a> {
    let mut paint = crate::ink::prepare_stroke_for_context_with_fountain_v16_output_scale(
        stroke,
        crate::ink::InkRenderContext::CanonicalPage,
        output_scale,
    );
    apply_context_stroke_theme(stroke, theme, &mut paint);
    paint
}

fn apply_context_stroke_theme(
    stroke: &Stroke,
    theme: RenderTheme,
    paint: &mut crate::PreparedStroke<'_>,
) {
    if paint.coverage_mesh.is_none() {
        let native_fountain_default_color = paint.profile == Some("FountainPen")
            && paint.support == crate::ink::InkSupport::Reconstructed
            && stroke.color.is_none()
            && stroke
                .rendering
                .as_ref()
                .and_then(|rendering| rendering.style.color_argb)
                .is_none();
        let color = if native_fountain_default_color {
            Some(Color { r: 0, g: 0, b: 0 })
        } else {
            stroke_source_color(stroke)
        };
        paint.color = theme.foreground(color);
    }
}

fn valid_fountain_v17_output_scale(scale: f32) -> bool {
    if !scale.is_finite() || scale <= 0.0 {
        return false;
    }
    let inverse = 1.0_f32 / scale;
    inverse.is_finite() && inverse > 0.0
}

fn valid_fountain_v16_output_scale(scale: f32) -> bool {
    crate::ink::fountain_v16_inverse_scale(scale).is_some()
}

fn valid_fountain_v14_output_scale(scale: f32) -> bool {
    if !scale.is_finite() || scale <= 0.0 || scale > 1_000_000.0 {
        return false;
    }
    let inverse = 1.0_f32 / scale;
    inverse.is_finite() && inverse > 0.0 && inverse <= 1_000_000.0
}

fn prepare_page_inkpen(
    stroke: &Stroke,
    profile: Option<InkPenRenderProfile>,
) -> Option<crate::ink::PreparedInkPenV1> {
    profile
        .and_then(|profile| {
            crate::ink::prepare_inkpen_v4_for_page(
                stroke,
                profile.screen_width,
                profile.screen_height,
            )
        })
        .or_else(|| crate::ink::prepare_inkpen_v1_for_page(stroke))
}

/// Prepare replay geometry in the same one-source-unit-per-SVG-pixel context
/// used by canonical page exports. The context-free `prepare_stroke` keeps
/// its compatibility behavior.
pub fn prepare_stroke_for_page(stroke: &Stroke, dark_mode: bool) -> crate::PreparedStroke<'_> {
    if stroke.points.len() <= 1 {
        return empty_saved_stroke_paint(stroke, RenderTheme::for_canvas(dark_mode));
    }
    let mut prepared = crate::ink::prepare_stroke_for_context(
        stroke,
        dark_mode,
        crate::ink::InkRenderContext::CanonicalPage,
    );
    if let Some(ink) = crate::ink::prepare_magicpen_v1_source(stroke)
        && magicpen_v1_vector_path(&ink, false).is_some()
    {
        let color =
            RenderTheme::for_canvas(dark_mode).foreground(magicpen_source_color(stroke, ink.argb));
        let mut sample_ends = ink.sample_ends;
        if let Some(last) = sample_ends.last_mut() {
            *last = ink.points.len();
        }
        prepared.points = Cow::Owned(ink.points);
        prepared.sample_ends = Some(sample_ends);
        prepared.dot_radii = Some(ink.radii.into_iter().map(f64::from).collect());
        prepared.ordered_disk_replay = None;
        prepared.dot_directions = None;
        prepared.rect_stamp = None;
        prepared.coverage_mesh = None;
        prepared.segment_widths = None;
        prepared.bounds = Some(ink.bounds);
        prepared.color = color;
        prepared.opacity = f64::from((ink.argb >> 24) as u8) / 255.0;
        prepared.top_layer = false;
        prepared.profile = Some("MagicPen");
        prepared.support = crate::InkSupport::Reconstructed;
    }
    if let Some(ink) = crate::ink::prepare_inkpen_v1_for_page(stroke)
        && let Some(vector) = inkpen_v1_vector_path(
            &ink.points,
            &ink.radii,
            &ink.sample_ends,
            ink.terminal,
            true,
        )
    {
        prepared.points = Cow::Owned(ink.points);
        prepared.sample_ends = Some(vector.sample_ends);
        prepared.dot_radii = Some(ink.radii.into_iter().map(f64::from).collect());
        prepared.ordered_disk_replay = None;
        prepared.dot_directions = None;
        prepared.rect_stamp = None;
        prepared.coverage_mesh = None;
        prepared.segment_widths = None;
        prepared.bounds = Some(ink.bounds);
        prepared.color = RenderTheme::for_canvas(dark_mode).foreground(stroke_source_color(stroke));
        prepared.opacity = f64::from((ink.argb >> 24) as u8) / 255.0;
        prepared.top_layer = false;
        prepared.profile = Some("InkPen");
        prepared.support = crate::InkSupport::Reconstructed;
    }
    prepared
}

fn empty_saved_stroke_paint(stroke: &Stroke, theme: RenderTheme) -> crate::PreparedStroke<'_> {
    crate::PreparedStroke {
        // Owned empty storage is intentional: serde must emit `points: []`
        // instead of omitting the field and allowing replay to reuse source
        // singleton geometry.
        points: Cow::Owned(Vec::new()),
        sample_ends: Some(vec![0; stroke.points.len()]),
        dot_radii: None,
        ordered_disk_replay: None,
        inkpen2_v9: None,
        fountain_v17_rgb: None,
        fountain_output_scale: None,
        dot_directions: None,
        rect_stamp: None,
        coverage_mesh: None,
        segment_widths: None,
        width: normalized_stroke_width(stroke.pen_width),
        bounds: None,
        color: theme.foreground(stroke_source_color(stroke)),
        opacity: 1.0,
        top_layer: false,
        profile: None,
        support: crate::InkSupport::Approximate,
    }
}

struct InkPenV1VectorPath {
    path: Path,
    sample_ends: Vec<usize>,
    #[cfg(test)]
    source_event_path_ends: Vec<usize>,
}

fn inkpen_v1_vector_path(
    points: &[crate::Point],
    radii: &[f32],
    sample_ends: &[usize],
    terminal: Range<usize>,
    replay: bool,
) -> Option<InkPenV1VectorPath> {
    const VERTICES: usize = 96;
    const MAX_STAMPS: usize = 65_536;
    const MAX_PATH_BYTES: usize = 8 * 1024 * 1024;

    if points.is_empty()
        || points.len() != radii.len()
        || points.len() > MAX_STAMPS
        || sample_ends.is_empty()
        || sample_ends.windows(2).any(|pair| pair[0] > pair[1])
        || sample_ends.iter().any(|&end| end > terminal.start)
        || sample_ends.last().copied() != Some(terminal.start)
        || terminal.start > terminal.end
        || terminal.end != points.len()
        || points.iter().any(|point| {
            !point.x.is_finite()
                || !point.y.is_finite()
                || !(point.x as f32).is_finite()
                || !(point.y as f32).is_finite()
        })
        || radii
            .iter()
            .any(|radius| !radius.is_finite() || *radius < 0.0)
    {
        return None;
    }

    // The saved V1 prefix table ends before the terminal path suffix. Page
    // replay folds that suffix into the final source event.
    let mut effective_ends = sample_ends.to_vec();
    if let Some(last) = effective_ends.last_mut() {
        *last = terminal.end;
    }

    let mut path = ReplayPath::default();
    let contour_vertices = (0..VERTICES)
        .map(|vertex| {
            let angle = 2.0 * std::f64::consts::PI * vertex as f64 / VERTICES as f64;
            let cos = angle.cos();
            let sin = angle.sin();
            (cos, sin, 0.5 * (cos.abs() + sin.abs()))
        })
        .collect::<Vec<_>>();
    #[cfg(test)]
    let mut source_event_path_ends = Vec::with_capacity(effective_ends.len());
    let mut previous_end = 0;
    for &event_end in &effective_ends {
        if event_end < previous_end || event_end > points.len() {
            return None;
        }
        for (point, radius) in points[previous_end..event_end]
            .iter()
            .zip(&radii[previous_end..event_end])
        {
            let radius = f64::from(*radius);
            let mut data = String::with_capacity(VERTICES * 48 + 8);
            for (vertex, &(cos, sin, coverage_offset)) in contour_vertices.iter().enumerate() {
                // Approximate the native shader's half-coverage contour with
                // a continuous-derivative boundary at one source unit per SVG
                // pixel. Native finite 2x2 `fwidth` coverage differs near edges.
                let contour = (radius - coverage_offset).max(0.0);
                let x = point.x + contour * cos;
                let y = point.y + contour * sin;
                if !x.is_finite() || !y.is_finite() {
                    return None;
                }
                if vertex == 0 {
                    data.push_str(&format!("M{x:.7},{y:.7}"));
                } else {
                    data.push_str(&format!(" L{x:.7},{y:.7}"));
                }
            }
            data.push_str(" Z");
            if !path.push_serialized(&data, replay, MAX_PATH_BYTES) {
                return None;
            }
        }
        #[cfg(test)]
        source_event_path_ends.push(path.serialized_len());
        previous_end = event_end;
    }
    if previous_end != points.len() {
        return None;
    }

    Some(InkPenV1VectorPath {
        path: path.finish(),
        sample_ends: effective_ends,
        #[cfg(test)]
        source_event_path_ends,
    })
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn inkpen_v1_vector_candidate_svg(
    points: &[crate::Point],
    radii: &[f32],
    sample_ends: &[usize],
    terminal: Range<usize>,
    color: &str,
    opacity: f64,
    viewport: [u32; 2],
    replay: bool,
) -> Option<InkPenV1VectorCandidate> {
    if !opacity.is_finite()
        || !(0.0..=1.0).contains(&opacity)
        || viewport.contains(&0)
        || !matches!(color.len(), 7)
        || !color.starts_with('#')
        || !color[1..].bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return None;
    }
    let vector = inkpen_v1_vector_path(points, radii, sample_ends, terminal, replay)?;
    let root = Svg::new()
        .width(viewport[0])
        .height(viewport[1])
        .view_box(ViewBox::new(
            0.0,
            0.0,
            f64::from(viewport[0]),
            f64::from(viewport[1]),
            1,
        ));
    let mut scene = Scene::new(root);
    scene.scope(Group::new().opacity(opacity), |scene| {
        scene.push(vector.path.fill(Paint::from_hex(color)));
    });
    Some(InkPenV1VectorCandidate {
        svg: scene.finish(),
        sample_ends: vector.sample_ends,
        source_event_path_ends: vector.source_event_path_ends,
    })
}

#[cfg(test)]
pub(crate) struct InkPenV1VectorCandidate {
    pub(crate) svg: String,
    pub(crate) sample_ends: Vec<usize>,
    pub(crate) source_event_path_ends: Vec<usize>,
}

const MAX_INKPEN2_SOURCE_SAMPLES: usize = 65_536;
const MAX_INKPEN2_SOURCE_BYTES: usize = 4 * 1024 * 1024;
const MAX_INKPEN2_OUTPUT_RECORDS: usize = 32_768;
const MAX_INKPEN2_ACTIVE_RGB_RECORDS: usize = 512;
const MAX_INKPEN2_ESTIMATED_MARKUP_BYTES: usize = 32 * 1024 * 1024;
const INKPEN2_MARKUP_BUDGET_PER_RECORD: usize = 1024;
const INKPEN2_MARKUP_FIXED_OVERHEAD_BYTES: usize = 4096;

// Test-only admission for paired native experiments. Thread-local scope prevents
// unrelated tests from observing it; the guard restores even during unwinding.
#[cfg(test)]
thread_local! {
    static INKPEN2_EXPERIMENTAL_AA_RAINBOW: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
struct InkPen2ExperimentalAaGuard(bool);

#[cfg(test)]
impl InkPen2ExperimentalAaGuard {
    fn enter() -> Self {
        Self(INKPEN2_EXPERIMENTAL_AA_RAINBOW.with(|flag| flag.replace(true)))
    }
}

#[cfg(test)]
impl Drop for InkPen2ExperimentalAaGuard {
    fn drop(&mut self) {
        INKPEN2_EXPERIMENTAL_AA_RAINBOW.with(|flag| flag.set(self.0));
    }
}

#[cfg(test)]
thread_local! {
    static INKPEN2_POSITIVE_Y_CHART: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
struct InkPen2PositiveYChartGuard(bool);

#[cfg(test)]
impl InkPen2PositiveYChartGuard {
    fn enter() -> Self {
        Self(INKPEN2_POSITIVE_Y_CHART.with(|flag| flag.replace(true)))
    }
}

#[cfg(test)]
impl Drop for InkPen2PositiveYChartGuard {
    fn drop(&mut self) {
        INKPEN2_POSITIVE_Y_CHART.with(|flag| flag.set(self.0));
    }
}

fn inkpen2_reject_active_rainbow_aa(enabled: bool) -> bool {
    #[cfg(test)]
    if INKPEN2_EXPERIMENTAL_AA_RAINBOW.with(|flag| flag.get()) {
        return false;
    }
    enabled
}

/// Prepare exact V9 records for the explicitly configured fresh-draw host profile.
fn prepare_inkpen2_v9(
    stroke: &Stroke,
    profile: Option<InkPen2RenderProfile>,
    theme: RenderTheme,
) -> Option<crate::ink::PreparedInkPen2V9> {
    let profile = profile?;
    if profile.screen_width == 0
        || profile.screen_height == 0
        || profile.screen_width > i32::MAX as u32
        || profile.screen_height > i32::MAX as u32
    {
        return None;
    }
    let rendering = stroke.rendering.as_ref()?;
    let active_rainbow = rendering.properties.rainbow_effect;
    let n = stroke.points.len();
    let source_bytes = n.checked_mul(48);
    if rendering.pen_name.as_deref() != Some("com.samsung.android.sdk.pen.pen.preload.InkPen2")
        || rendering.advanced_settings.as_deref() != Some("9;")
        || rendering.tool_type_raw != 2
        || rendering.style.stroke_type_raw.unwrap_or(0) != 0
        || rendering.style.line_type_raw.unwrap_or(0) != 0
        // Type 1 retains source RGB with rainbow disabled, even with a
        // forwarded palette. Type 2 has no Patternable interface on InkPen2.
        || !matches!(rendering.style.color_type_raw.unwrap_or(0), 0..=2)
        || rendering.properties.replay_only
        || !rendering.properties.stylus_channels
        || rendering.properties.eraser
        || rendering.properties.fixed_width
        || rendering.properties.millisecond_timestamps
        || rendering.properties.top_layer_pen
        || rendering.properties.alpha_lock
        || !rendering.properties.binary_added
        || rendering.properties.generated
        || rendering.properties.fixed_opacity
        || rendering.properties.straighten
        || rendering.properties.reveal_mode
        || !(2..=MAX_INKPEN2_SOURCE_SAMPLES).contains(&n)
        || source_bytes.is_none_or(|bytes| bytes > MAX_INKPEN2_SOURCE_BYTES)
        || stroke.timestamps.len() != n
        || stroke.pressures.len() != n
        || stroke.tilts.len() != n
        || stroke.orientations.len() != n
        || stroke.timestamps.iter().any(|&t| i32::try_from(t).is_err())
        || stroke.points.iter().any(|p| {
            !p.x.is_finite()
                || !p.y.is_finite()
                || !(p.x as f32).is_finite()
                || !(p.y as f32).is_finite()
        })
        || stroke
            .pressures
            .iter()
            .any(|&v| !v.is_finite() || !(v as f32).is_finite())
        || stroke.tilts.iter().any(|&v| !v.is_finite())
        || stroke.orientations.iter().any(|&v| !v.is_finite())
    {
        return None;
    }
    let size = rendering.style.pen_size?;
    if !size.is_finite()
        || !stroke.pen_width.is_finite()
        || size.to_bits() != stroke.pen_width.to_bits()
    {
        return None;
    }
    let argb = rendering.style.color_argb?;
    let rainbow = if active_rainbow {
        if inkpen2_reject_active_rainbow_aa(profile.enhanced_antialias)
            || profile.screen_width.min(profile.screen_height) != 1080
            || rendering.style.color_type_raw != Some(1)
            || rendering.style.first_unparsed_field.is_some()
            || !rendering.style.trailing_data.is_empty()
        {
            return None;
        }
        let distance = rendering
            .style
            .rainbow_distance
            .filter(|distance| *distance > 0)?;
        let offset = rendering
            .style
            .rainbow_offset
            .filter(|offset| offset.is_finite() && *offset >= 0.0)?;
        let palette = rendering.style.gradient_colors_argb.as_deref()?;
        if !(2..=256).contains(&palette.len()) {
            return None;
        }
        Some(crate::ink::inkpen2::Rainbow {
            distance,
            offset,
            palette_argb: palette,
        })
    } else {
        None
    };
    // Binary field 13 is multiplied by a native document conversion factor.
    // Until that factor is derived for this input route, only absent/zero is safe.
    let tolerance = rendering.style.initial_tolerance.unwrap_or(0.0);
    if !tolerance.is_finite() || tolerance != 0.0 {
        return None;
    }
    let points: Vec<_> = stroke
        .points
        .iter()
        .map(|p| crate::ink::inkpen2::source_point(p.x as f32, p.y as f32))
        .collect();
    let times: Vec<i32> = stroke.timestamps.iter().map(|&t| t as i32).collect();
    let pressures: Vec<f32> = stroke.pressures.iter().map(|&p| p as f32).collect();
    let tools = vec![i32::from(rendering.tool_type_raw); points.len()];
    let input = crate::ink::inkpen2::Input {
        points: &points,
        times: &times,
        tools: &tools,
        pressures: &pressures,
        rgb: [(argb >> 16) as u8, (argb >> 8) as u8, argb as u8],
        rainbow,
        size: size.clamp(0.4, 800.0),
        curve_enabled: rendering.properties.compressed,
        is_shape: false,
        runtime: crate::ink::inkpen2::Runtime {
            inverse_scale: [1.0, 1.0],
            spacing: 1.0,
            tolerance,
            tool_threshold: 20.0,
            resolution_min: profile.screen_width.min(profile.screen_height) as i32,
            initial_alternation: false,
            initial_time: *times.first().unwrap_or(&0),
        },
    };
    let output_budget = if active_rainbow {
        MAX_INKPEN2_ACTIVE_RGB_RECORDS
    } else {
        MAX_INKPEN2_OUTPUT_RECORDS
    };
    #[allow(unused_mut)]
    let mut output = crate::ink::inkpen2::redraw_bounded(input, output_budget)?;
    let output_bytes = output.vertices.len().checked_mul(36);
    let markup_bytes = output
        .vertices
        .len()
        .checked_mul(INKPEN2_MARKUP_BUDGET_PER_RECORD)
        .and_then(|bytes| bytes.checked_add(INKPEN2_MARKUP_FIXED_OVERHEAD_BYTES));
    if output.vertices.len() > output_budget
        || output_bytes.is_none_or(|bytes| bytes > output_budget * 36)
        || markup_bytes.is_none_or(|bytes| bytes > MAX_INKPEN2_ESTIMATED_MARKUP_BYTES)
        || output.sample_ends.len() != n
        || output.sample_ends.first() != Some(&0)
        || output.sample_ends.windows(2).any(|pair| pair[0] > pair[1])
        || output.sample_ends.last() != Some(&output.terminal.start)
        || output.terminal.start > output.terminal.end
        || output.terminal.end != output.vertices.len()
    {
        return None;
    }
    if output.vertices.iter().any(|record| {
        record.iter().any(|value| !value.is_finite())
            || record[2] <= 0.0
            || record[3] <= 0.0
            || !(0.0..=1.0).contains(&record[5])
    }) {
        return None;
    }
    if active_rainbow
        && (output.vertices.iter().any(|record| record[5] != 1.0)
            || !inkpen2_vector::valid_record_rgb(&output.vertices))
    {
        return None;
    }
    if output.vertices.is_empty() {
        return None;
    }
    #[cfg(test)]
    if INKPEN2_POSITIVE_Y_CHART.with(|flag| flag.get()) {
        for record in &mut output.vertices {
            record[1] = 96.0 - record[1];
        }
    }
    let mut sample_ends = output.sample_ends;
    *sample_ends.last_mut()? = output.terminal.end;
    let mut bounds: Option<BoundingBox> = None;
    for [cx, cy, rx, ry, ..] in &output.vertices {
        let current = BoundingBox {
            x_min: f64::from(*cx) - f64::from(*rx),
            y_min: f64::from(*cy) - f64::from(*ry),
            x_max: f64::from(*cx) + f64::from(*rx),
            y_max: f64::from(*cy) + f64::from(*ry),
        };
        if let Some(old) = &mut bounds {
            old.x_min = old.x_min.min(current.x_min);
            old.y_min = old.y_min.min(current.y_min);
            old.x_max = old.x_max.max(current.x_max);
            old.y_max = old.y_max.max(current.y_max);
        } else {
            bounds = Some(current);
        }
    }
    let source_color = stroke.color.or(Some(argb_color(argb)));
    let color = if active_rainbow {
        // The native palette supplies each stamp's RGB. Keep the saved base
        // color intact for alpha/fallback semantics; never theme-map stops.
        argb_color(argb)
    } else {
        source_color
            .map(|color| theme.foreground_color(color))
            .unwrap_or_else(|| argb_color(argb))
    };
    Some(crate::ink::PreparedInkPen2V9 {
        per_stamp_rgb: active_rainbow,
        records: output.vertices,
        sample_ends,
        color,
        opacity: f64::from((argb >> 24) as u8 as f32 / 255.0),
        enhanced_antialias: profile.enhanced_antialias,
        bounds: bounds?,
    })
}

fn render_inkpen2_v9(
    svg: &mut Scene,
    ink: &crate::ink::PreparedInkPen2V9,
    replay_index: Option<usize>,
) {
    inkpen2_vector::render(svg, ink, replay_index);
}

fn pencil3_paint_context(profile: Pencil3RenderProfile) -> pencil3::Pencil3PaintContext {
    pencil3::Pencil3PaintContext {
        canvas_width: profile.canvas_width,
        canvas_height: profile.canvas_height,
        background_scale: profile.screen_width.min(profile.screen_height) as f32 / 1440.0,
        paper_sampler: profile.paper_sampler,
        y_up: false,
    }
}

fn prepare_pencil3_v3(
    stroke: &Stroke,
    profile: Pencil3RenderProfile,
    theme: RenderTheme,
    viewport: [u32; 2],
) -> Option<crate::ink::pencil_source::PreparedPencil3V3> {
    if [profile.canvas_width, profile.canvas_height] != viewport
        || viewport.contains(&0)
        || viewport.iter().any(|&size| size > i32::MAX as u32)
    {
        return None;
    }
    let mut ink = crate::ink::pencil_source::prepare_pencil3_v3_source(
        stroke,
        profile.screen_width,
        profile.screen_height,
    )?;
    ink.color = theme.foreground_color(stroke.color.unwrap_or(ink.color));
    Some(ink)
}

/// Prepare replay metadata with a resolved theme and explicit InkPen or
/// InkPen2 screen profile. For canvas-dependent Pencil3 profiles, use
/// [`prepare_stroke_for_viewport_with_theme`] with the actual rendered page size.
pub fn prepare_stroke_for_page_with_theme<'a>(
    stroke: &'a Stroke,
    theme: RenderTheme,
    options: &RenderOptions,
) -> crate::PreparedStroke<'a> {
    if stroke.points.len() <= 1 {
        return empty_saved_stroke_paint(stroke, theme);
    }
    if let Some(profile) = options.inkpen_profile
        && let Some(ink) = crate::ink::prepare_inkpen_v4_for_page(
            stroke,
            profile.screen_width,
            profile.screen_height,
        )
        && let Some(vector) = inkpen_v1_vector_path(
            &ink.points,
            &ink.radii,
            &ink.sample_ends,
            ink.terminal.clone(),
            true,
        )
    {
        let mut prepared = prepare_stroke_for_page(stroke, theme.is_dark());
        prepared.points = Cow::Owned(ink.points);
        prepared.sample_ends = Some(vector.sample_ends);
        prepared.dot_radii = Some(ink.radii.into_iter().map(f64::from).collect());
        prepared.ordered_disk_replay = None;
        prepared.dot_directions = None;
        prepared.rect_stamp = None;
        prepared.coverage_mesh = None;
        prepared.segment_widths = None;
        prepared.bounds = Some(ink.bounds);
        prepared.color = theme.foreground(stroke.color.or(Some(argb_rgb(ink.argb))));
        prepared.opacity = f64::from((ink.argb >> 24) as u8) / 255.0;
        prepared.top_layer = false;
        prepared.profile = Some("InkPen");
        prepared.support = crate::InkSupport::Reconstructed;
        return prepared;
    }
    let Some(ink) = prepare_inkpen2_v9(stroke, options.inkpen2_profile, theme) else {
        return prepare_stroke_for_page(stroke, theme.is_dark());
    };
    let mut prepared = prepare_stroke_for_page(stroke, theme.is_dark());
    prepared.points = Cow::Owned(
        ink.records
            .iter()
            .map(|r| crate::Point {
                x: f64::from(r[0]),
                y: f64::from(r[1]),
            })
            .collect(),
    );
    prepared.sample_ends = Some(ink.sample_ends.clone());
    prepared.dot_radii = None;
    prepared.ordered_disk_replay = None;
    prepared.inkpen2_v9 = Some(ink.clone());
    prepared.dot_directions = None;
    prepared.rect_stamp = None;
    prepared.coverage_mesh = None;
    prepared.segment_widths = None;
    prepared.bounds = Some(ink.bounds);
    prepared.color = vector::color_hex(&ink.color);
    prepared.opacity = ink.opacity;
    prepared.profile = Some("InkPen2V9");
    prepared.support = crate::InkSupport::Reconstructed;
    prepared
}

/// Prepare replay metadata for an actual rendered page viewport. This applies
/// the same canvas eligibility check as SVG/PDF rendering for Pencil3 profiles.
/// Callers rendering transformed descendants must use [`crate::prepare_stroke`]
/// instead, matching the renderer's native-profile exclusion.
pub fn prepare_stroke_for_viewport_with_theme<'a>(
    stroke: &'a Stroke,
    theme: RenderTheme,
    options: &RenderOptions,
    viewport: [u32; 2],
) -> crate::PreparedStroke<'a> {
    if stroke.points.len() <= 1 {
        return empty_saved_stroke_paint(stroke, theme);
    }
    if viewport.contains(&0)
        && options.inkpen_profile.is_some()
        && stroke.rendering.as_ref().is_some_and(|rendering| {
            rendering.pen_name.as_deref() == Some("com.samsung.android.sdk.pen.pen.preload.InkPen")
                && matches!(
                    rendering.advanced_settings.as_deref(),
                    Some("") | Some("4;") | Some("18;0;100;")
                )
        })
    {
        return crate::ink::prepare_stroke_for_context(
            stroke,
            theme.is_dark(),
            crate::ink::InkRenderContext::CanonicalPage,
        );
    }
    let Some(ink) = options.pencil3_profile.and_then(|profile| {
        let ink = prepare_pencil3_v3(stroke, profile, theme, viewport)?;
        pencil3::eligible(&ink, pencil3_paint_context(profile)).then_some(ink)
    }) else {
        return prepare_stroke_for_page_with_theme(stroke, theme, options);
    };
    let mut prepared = prepare_stroke_for_page(stroke, theme.is_dark());
    prepared.points = Cow::Owned(
        ink.records
            .iter()
            .map(|r| crate::Point {
                x: f64::from(r[0]),
                y: f64::from(r[1]),
            })
            .collect(),
    );
    prepared.sample_ends = Some(ink.sample_ends);
    prepared.dot_radii = None;
    prepared.ordered_disk_replay = None;
    prepared.inkpen2_v9 = None;
    prepared.dot_directions = None;
    prepared.rect_stamp = None;
    prepared.coverage_mesh = None;
    prepared.segment_widths = None;
    prepared.bounds = Some(ink.bounds);
    prepared.color = vector::color_hex(&ink.color);
    prepared.opacity = f64::from(ink.opacity);
    prepared.profile = Some("Pencil3V3");
    prepared.support = crate::InkSupport::Reconstructed;
    prepared
}

fn render_prepared_stroke(
    svg: &mut Scene,
    paint: &crate::PreparedStroke<'_>,
    replay: bool,
    slice: Option<PageSlice>,
) {
    render_prepared_stroke_with_fallback(svg, paint, replay, slice, || None);
}

fn render_prepared_stroke_with_fallback<'a>(
    svg: &mut Scene,
    paint: &crate::PreparedStroke<'a>,
    replay: bool,
    slice: Option<PageSlice>,
    fallback: impl FnOnce() -> Option<crate::PreparedStroke<'a>>,
) {
    let color = &paint.color;
    if oblique::render(svg, paint, replay) {
        return;
    }
    if fountain::render(svg, paint, replay, slice) {
        return;
    }
    if let Some(fallback) = fallback() {
        render_prepared_stroke(svg, &fallback, replay, slice);
        return;
    }
    let mut invalid_ordered_replay = false;
    if let (Some(replay_data), Some(radii)) = (&paint.ordered_disk_replay, &paint.dot_radii) {
        let sample_count = replay_data.terminal_part;
        let expected_redraws = sample_count.saturating_sub(2);
        let structurally_valid = sample_count >= 2
            && radii.len() == paint.points.len()
            && replay_data.redraw_ends.len() == expected_redraws
            && replay_data.terminal_part > 0
            && replay_data.terminal_start <= paint.points.len()
            && replay_data
                .redraw_ends
                .iter()
                .all(|&end| end <= replay_data.terminal_start)
            && replay_data
                .redraw_ends
                .windows(2)
                .all(|pair| pair[0] <= pair[1])
            && replay_data.redraw_ends.last().copied().unwrap_or(0) == replay_data.terminal_start
            && paint
                .points
                .iter()
                .all(|point| point.x.is_finite() && point.y.is_finite())
            && radii
                .iter()
                .all(|radius| radius.is_finite() && *radius >= 0.0);
        if structurally_valid {
            let mut start = 0usize;
            for (redraw, &end) in replay_data.redraw_ends.iter().enumerate() {
                for (index, &radius) in radii.iter().enumerate().take(end).skip(start) {
                    let point = &paint.points[index];
                    svg.push(
                        Circle::new()
                            .replay_part(replay.then_some(ReplayPart(redraw + 2)))
                            .cx(point.x)
                            .cy(point.y)
                            .r(radius)
                            .fill(Paint::from_hex(color)),
                    );
                }
                start = end;
            }
            for (index, &radius) in radii
                .iter()
                .enumerate()
                .take(paint.points.len())
                .skip(replay_data.terminal_start)
            {
                let point = &paint.points[index];
                svg.push(
                    Circle::new()
                        .replay_part(replay.then_some(ReplayPart(replay_data.terminal_part)))
                        .cx(point.x)
                        .cy(point.y)
                        .r(radius)
                        .fill(Paint::from_hex(color)),
                );
            }
            return;
        }
        // Invalid metadata falls back to the original polyline below. It
        // never emits a partial prefix or a truncated radii zip.
        invalid_ordered_replay = true;
    }
    if let Some(stamp) = paint.rect_stamp {
        let (sin, cos) = stamp.angle.sin_cos();
        let (w, h) = (stamp.width, stamp.height);
        let (rx, ry) = (w * 25. / 99., h * 25. / 99.);
        let mut path = ReplayPath::default();
        for p in paint.points.iter() {
            let x = p.x * cos + p.y * sin - w / 2.;
            let y = -p.x * sin + p.y * cos - h / 2.;
            let r = (coordinate(rx, 4), coordinate(ry, 4));
            path.push(
                Data::new()
                    .move_to((coordinate(x + rx, 4), coordinate(y, 4)))
                    .horizontal_line_by(coordinate(w - 2. * rx, 4))
                    .elliptical_arc_by((r.0, r.1, 0., false, true, r.0, r.1))
                    .vertical_line_by(coordinate(h - 2. * ry, 4))
                    .elliptical_arc_by((r.0, r.1, 0., false, true, -r.0, r.1))
                    .horizontal_line_by(coordinate(2. * rx - w, 4))
                    .elliptical_arc_by((r.0, r.1, 0., false, true, -r.0, -r.1))
                    .vertical_line_by(coordinate(2. * ry - h, 4))
                    .elliptical_arc_by((r.0, r.1, 0., false, true, r.0, -r.1))
                    .close(),
                replay,
            );
        }
        svg.push(
            path.finish()
                .fill(Paint::from_hex(color))
                .fill_opacity(decimal(paint.opacity, 6))
                .transformed(Transform::rotate_origin(stamp.angle.to_degrees())),
        );
        return;
    }
    if let Some(radii) = paint.dot_radii.as_ref().filter(|_| !invalid_ordered_replay) {
        let mut path = ReplayPath::default();
        for (p, r) in paint.points.iter().zip(radii) {
            let radius = coordinate(*r, 4);
            path.push(
                Data::new()
                    .move_to((coordinate(p.x - r, 4), coordinate(p.y, 4)))
                    .elliptical_arc_by((radius, radius, 0., true, false, coordinate(r * 2., 4), 0.))
                    .elliptical_arc_by((
                        radius,
                        radius,
                        0.,
                        true,
                        false,
                        coordinate(-r * 2., 4),
                        0.,
                    ))
                    .close(),
                replay,
            );
        }
        let mut node = path.finish().fill(Paint::from_hex(color));
        if (paint.opacity - 1.0).abs() > 1e-4 {
            node = node.fill_opacity(decimal(paint.opacity, 4));
        }
        svg.push(node);
        return;
    }
    // Composite approximate InkPen segments before applying source alpha;
    // per-segment opacity would accumulate at joins and self intersections.
    if paint.opacity != 1.0 {
        svg.scope(Group::new().opacity(paint.opacity), |svg| {
            render_generic_stroke(svg, paint, replay);
        });
    } else {
        render_generic_stroke(svg, paint, replay);
    }
}

fn render_generic_stroke(svg: &mut Scene, paint: &crate::PreparedStroke<'_>, replay: bool) {
    let color = &paint.color;
    let base_width = paint.width;
    if let [point] = paint.points.as_ref() {
        svg.push(
            Circle::new()
                .replay_part(replay.then_some(ReplayPart(1)))
                .cx(point.x)
                .cy(point.y)
                .r(base_width / 2.)
                .fill(Paint::from_hex(color)),
        );
        return;
    }
    if paint.points.is_empty() {
        return;
    }
    if let Some(widths) = &paint.segment_widths {
        for j in 1..paint.points.len() {
            let p1 = &paint.points[j - 1];
            let p2 = &paint.points[j];
            svg.push(
                Line::new()
                    .replay_part(replay.then_some(ReplayPart(j + 1)))
                    .x1(decimal(p1.x, 2))
                    .y1(decimal(p1.y, 2))
                    .x2(decimal(p2.x, 2))
                    .y2(decimal(p2.y, 2))
                    .stroke(Paint::from_hex(color.as_str()))
                    .stroke_width(decimal(widths[j - 1], 2))
                    .line_cap(LineCap::Round),
            );
        }
    } else {
        svg.push(
            vector::polyline(&paint.points, replay)
                .fill(Paint::None)
                .stroke(Paint::from_hex(color.as_str()))
                .stroke_width(decimal(base_width, 2))
                .line_cap(LineCap::Round)
                .line_join(LineJoin::Round),
        );
    }
}

/// Shared paint values for Svg output and interactive stroke inspection.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct StrokePaint {
    pub color: String,
    pub width: f64,
    /// One width per point-to-point segment; absent for a constant-width polyline.
    pub segment_widths: Option<Vec<f64>>,
}

fn stroke_source_color(stroke: &Stroke) -> Option<Color> {
    // Preserve an explicit public color; recover saved RGB for bounded profiles
    // supported by native paint controls or source-derived color forwarding.
    stroke.color.or_else(|| {
        chinesebrush_source_color(stroke)
            .or_else(|| crate::ink::saved_inkpen_argb(stroke).map(argb_rgb))
            .or_else(|| crate::ink::saved_airbrush_argb(stroke).map(argb_rgb))
            .or_else(|| saved_textured_pen_argb(stroke).map(argb_rgb))
    })
}

fn argb_rgb(argb: u32) -> Color {
    Color {
        r: (argb >> 16) as u8,
        g: (argb >> 8) as u8,
        b: argb as u8,
    }
}

/// Recover ObjectStroke's omitted-color default only for canonical parsed
/// ChineseBrush records. Public/generic strokes and unsupported profiles keep
/// the established theme fallback.
fn chinesebrush_source_color(stroke: &Stroke) -> Option<Color> {
    let rendering = stroke.rendering.as_ref()?;
    if rendering.metadata.is_none()
        || rendering.pen_name.as_deref()
            != Some("com.samsung.android.sdk.pen.pen.preload.ChineseBrush")
        || !matches!(
            rendering.advanced_settings.as_deref(),
            Some("1;" | "2;" | "3;")
        )
    {
        return None;
    }
    Some(argb_rgb(rendering.style.color_argb.unwrap_or(0xff00_0000)))
}

/// Recover explicit source RGB for the observed ColoredPencil V4,
/// WaterColorBrush V1/V2 and SimpleBrush null-setting routes. SimpleBrush has
/// one native color control; other colors are inferred from the shader's RGB
/// input. Their native paint is still outside the generic vector renderer; this does not
/// transfer ARGB alpha to whole-stroke opacity.
fn saved_textured_pen_argb(stroke: &Stroke) -> Option<u32> {
    const COLORED_PENCIL: &str = "com.samsung.android.sdk.pen.pen.preload.ColoredPencil";
    const WATERCOLOR: &str = "com.samsung.android.sdk.pen.pen.preload.WaterColorBrush";
    const SIMPLE_BRUSH: &str = "com.samsung.android.sdk.pen.pen.preload.Brush";

    let rendering = stroke.rendering.as_ref()?;
    let properties = &rendering.properties;
    if rendering.metadata.is_none()
        || rendering.tool_type_raw != 2
        || properties.replay_only
        || properties.eraser
        || properties.fixed_width
        || properties.top_layer_pen
        || properties.alpha_lock
        || properties.fixed_opacity
        || properties.rainbow_effect
        || properties.straighten
        || properties.reveal_mode
        || rendering
            .style
            .stroke_type_raw
            .is_some_and(|value| value != 0)
        || rendering
            .style
            .color_type_raw
            .is_some_and(|value| value != 0)
    {
        return None;
    }

    let observed_profile = matches!(
        (
            rendering.pen_name.as_deref(),
            rendering.advanced_settings.as_deref(),
        ),
        (Some(COLORED_PENCIL), Some("4;"))
            | (Some(WATERCOLOR), None | Some(""))
            | (Some(SIMPLE_BRUSH), None)
    );
    observed_profile
        .then_some(rendering.style.color_argb)
        .flatten()
}

/// Resolve paint for a default light or dark canvas, without document metadata.
pub fn stroke_paint(stroke: &Stroke, dark_mode: bool) -> StrokePaint {
    let width = normalized_stroke_width(stroke.pen_width);
    let pressure = stroke.pressures.len() >= stroke.points.len().saturating_sub(1)
        && stroke
            .pressures
            .iter()
            .any(|&p| p > PRESSURE_PRESENT_EPSILON);
    StrokePaint {
        color: RenderTheme::for_canvas(dark_mode).foreground(stroke_source_color(stroke)),
        width,
        segment_widths: pressure.then(|| {
            stroke
                .pressures
                .iter()
                .take(stroke.points.len().saturating_sub(1))
                .map(|pressure| width * (0.3 + 0.7 * pressure.clamp(0.05, 1.0)))
                .collect()
        }),
    }
}

fn normalized_stroke_width(pen_width: f32) -> f64 {
    let raw_width = pen_width as f64 / 2.5;
    if raw_width.is_finite() && raw_width > 0.0 {
        raw_width.clamp(0.4, 12.0)
    } else {
        1.0
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ChineseBrushFwidthOutputProfile, ChineseBrushRenderProfile, FountainV14OutputScaleProfile,
        FountainV16OutputScaleProfile, FountainV17OutputScaleProfile,
        FountainV17RainbowRenderProfile, InkPen2RenderProfile, RenderColorMode, RenderOptions,
        RenderTheme, is_dark_background, normalized_stroke_width, prepare_inkpen2_v9,
        prepare_stroke_for_page_with_theme, render_document_svg, render_layout_page_replay_svg,
        render_layout_page_svg, sanitize_hyperlink_target,
    };
    use crate::{
        BoundingBox, Color, Document, DocumentMetadata, Page, PageElement, Point, RichTextBox,
        RichTextParagraph, RichTextParagraphType, RichTextSection, RichTextSpan, RichTextSpanType,
        Stroke, StrokeProperties, StrokeRendering, StrokeStyle, layout_document,
    };

    #[test]
    fn placed_image_alpha_matches_native_fused_quantization_and_clamp() {
        use super::placed_image_alpha;

        for (transparency, expected) in [
            (-0.25, 255),
            (0.0, 255),
            (0.5, 127),
            (1.0, 0),
            (1.25, 0),
            (f32::MAX, 0),
            (f32::NAN, 255),
            // Fused native arithmetic gives 238; (1 - t) * 255 gives 237.
            (f32::from_bits(0x3d88_888c), 238),
        ] {
            assert_eq!(placed_image_alpha(transparency), expected);
        }
    }
    #[cfg(feature = "pdf")]
    use crate::{
        MediaAsset, ObjectSpanLayoutConstraint, ObjectSpanLayoutOption, ObjectType,
        PageObjectContent, PlacedImage, RichTextObjectContent, RichTextObjectSpan,
    };

    #[cfg(feature = "pdf")]
    fn test_rgb_png() -> Vec<u8> {
        let pixels = [
            11, 12, 13, 21, 22, 23, 31, 32, 33, 41, 42, 43, // y=0
            51, 52, 53, 61, 62, 63, 71, 72, 73, 81, 82, 83, // y=1
            91, 92, 93, 101, 102, 103, 111, 112, 113, 121, 122, 123, // y=2
        ];
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, 4, 3);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&pixels)
            .unwrap();
        bytes
    }

    #[cfg(feature = "pdf")]
    fn test_gamma_rgb_png() -> Vec<u8> {
        let pixels = [11, 12, 13, 21, 22, 23, 31, 32, 33, 41, 42, 43];
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, 4, 1);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_source_gamma(png::ScaledFloat::new(0.45455));
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&pixels)
            .unwrap();
        bytes
    }

    #[cfg(feature = "pdf")]
    fn render_test_placed_image(image: PlacedImage, data: Vec<u8>) -> String {
        let mut page = page_with_uncolored_stroke();
        page.objects = vec![PageElement::PlacedImage(image).into()];
        let mut document = document(page);
        document.metadata.media_assets = vec![MediaAsset {
            name: "crop.png".to_owned(),
            archive_id: Some(0),
            mime_type: "image/png".to_owned(),
            data,
        }];
        render_document_svg(&document, &RenderOptions::default())
            .remove(0)
            .svg
    }

    #[cfg(feature = "pdf")]
    fn simple_original_crop(rotation_degrees: Option<f64>) -> PlacedImage {
        PlacedImage {
            bbox: BoundingBox {
                x_min: 20.0,
                y_min: 30.0,
                x_max: 70.0,
                y_max: 50.0,
            },
            rotation_degrees,
            media_id: Some(0),
            media_index: Some(0),
            fill_effect_transparency: 0.0,
            crop_rect: Some([1, 1, 3, 2]),
            original_bbox: Some(BoundingBox {
                x_min: 0.0,
                y_min: 0.0,
                x_max: 4.0,
                y_max: 3.0,
            }),
            border_media_id: None,
            original_media_id: None,
        }
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn original_bbox_rgb_and_rgba_crops_use_display_bounds() {
        for source in [
            test_rgb_png(),
            include_bytes!("../tests/assets/crop-quadrants.png").to_vec(),
        ] {
            let svg = render_test_placed_image(simple_original_crop(Some(0.0)), source);
            let xml = roxmltree::Document::parse(&svg).unwrap();
            let images = xml
                .descendants()
                .filter(|node| node.has_tag_name("image"))
                .collect::<Vec<_>>();
            assert_eq!(images.len(), 1);
            let image = images[0];
            for (attribute, expected) in
                [("x", 20.0), ("y", 30.0), ("width", 50.0), ("height", 20.0)]
            {
                let actual = image.attribute(attribute).unwrap().parse::<f64>().unwrap();
                assert_eq!(actual, expected, "attribute {attribute}");
            }
            assert!(
                image
                    .attribute("href")
                    .is_some_and(|href| href.starts_with("data:image/png;base64,"))
            );
        }
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn rotated_or_unsupported_original_crops_keep_the_full_source_path() {
        let mut extra_media = simple_original_crop(None);
        extra_media.border_media_id = Some(7);
        for (image, source) in [
            (simple_original_crop(Some(15.0)), test_rgb_png()),
            (extra_media, test_rgb_png()),
            (simple_original_crop(None), test_gamma_rgb_png()),
        ] {
            let svg = render_test_placed_image(image, source);
            let xml = roxmltree::Document::parse(&svg).unwrap();
            let image = xml
                .descendants()
                .find(|node| node.has_tag_name("image"))
                .unwrap();
            assert_eq!(image.attribute("width"), Some("4"));
            assert_eq!(image.attribute("height"), Some("3"));
        }
    }

    #[cfg(all(feature = "pdf", feature = "serde"))]
    #[test]
    #[ignore = "large actual-source slice raster oracle; set SDOCX_QUIZ_SDOCX and SDOCX_CS61_SDOCX"]
    fn actual_fountain_slice_culling_matches_same_viewport_without_culling() {
        fn intervals(case: &str) -> Vec<(u32, u32)> {
            match case {
                "quiz" => std::iter::once((0, 1485))
                    .chain((0..10).map(|index| (1485 + index * 1527, 1527)))
                    .collect(),
                "cs61" => vec![
                    (0, 1397),
                    (1397, 1397),
                    (2794, 1398),
                    (4192, 1398),
                    (5590, 1397),
                    (6987, 1398),
                    (8385, 1398),
                    (9783, 1397),
                    (11180, 1398),
                    (12578, 1398),
                    (13976, 1397),
                    (15373, 1398),
                    (16771, 1398),
                    (18169, 1397),
                    (19566, 1398),
                    (20964, 1398),
                    (22362, 1398),
                    (23760, 1397),
                    (25157, 1397),
                    (26554, 1398),
                ],
                _ => panic!("unknown actual fixture {case}"),
            }
        }

        fn raster(svg: &str, width: u32, height: u32) -> resvg::tiny_skia::Pixmap {
            let tree = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default())
                .expect("rendered slice SVG parses");
            let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)
                .expect("slice raster dimensions fit memory");
            resvg::render(
                &tree,
                resvg::tiny_skia::Transform::identity(),
                &mut pixmap.as_mut(),
            );
            pixmap
        }

        fn assert_native_ink_equal(expected: &super::RenderedScene, actual: &super::RenderedScene) {
            let expected_entries = expected.ink.iter().collect::<Vec<_>>();
            let actual_entries = actual.ink.iter().collect::<Vec<_>>();
            assert_eq!(expected_entries.len(), actual_entries.len());
            for ((expected_id, expected), (actual_id, actual)) in
                expected_entries.into_iter().zip(actual_entries)
            {
                assert_eq!(expected_id, actual_id);
                assert_eq!(expected.bounds, actual.bounds);
                assert_eq!(expected.opacity.to_bits(), actual.opacity.to_bits());
                match (&expected.payload, &actual.payload) {
                    (
                        crate::render::native_ink::NativeInkPayload::Mesh {
                            mesh: expected_mesh,
                            color: expected_color,
                        },
                        crate::render::native_ink::NativeInkPayload::Mesh {
                            mesh: actual_mesh,
                            color: actual_color,
                        },
                    ) => {
                        assert_eq!(expected_color, actual_color);
                        assert_eq!(expected_mesh, actual_mesh);
                    }
                    (
                        crate::render::native_ink::NativeInkPayload::Radial {
                            records: expected_records,
                            saved_alpha: expected_alpha,
                            enhanced_antialias: expected_enhanced,
                        },
                        crate::render::native_ink::NativeInkPayload::Radial {
                            records: actual_records,
                            saved_alpha: actual_alpha,
                            enhanced_antialias: actual_enhanced,
                        },
                    ) => {
                        assert_eq!(expected_records, actual_records);
                        assert_eq!(expected_alpha.to_bits(), actual_alpha.to_bits());
                        assert_eq!(expected_enhanced, actual_enhanced);
                    }
                    (
                        crate::render::native_ink::NativeInkPayload::RadialCircles {
                            records: expected_records,
                            saved_alpha: expected_alpha,
                        },
                        crate::render::native_ink::NativeInkPayload::RadialCircles {
                            records: actual_records,
                            saved_alpha: actual_alpha,
                        },
                    ) => {
                        assert_eq!(expected_records, actual_records);
                        assert_eq!(expected_alpha.to_bits(), actual_alpha.to_bits());
                    }
                    _ => panic!("native ink payload route changed"),
                }
            }
        }

        let artifact_dir =
            std::env::var_os("SDOCX_SLICE_EQUIVALENCE_ARTIFACTS").map(std::path::PathBuf::from);
        for (case, variable) in [("quiz", "SDOCX_QUIZ_SDOCX"), ("cs61", "SDOCX_CS61_SDOCX")] {
            let path = std::env::var_os(variable)
                .unwrap_or_else(|| panic!("set {variable} to the actual {case} fixture"));
            let document = crate::parse(&path).expect("actual source note parses");
            let layout = layout_document(&document);
            assert_eq!(
                layout.pages.len(),
                1,
                "fixture must remain one visible page"
            );
            let layout_page = &layout.pages[0];
            let options = RenderOptions::default();
            let fonts = super::fonts::FontBook::default();
            let mut cache = super::DocumentTextCache::default();

            for (index, (y, height)) in intervals(case).into_iter().enumerate() {
                let request = super::PageSceneRequest::sliced(layout_page, y, height, false)
                    .expect("captured native slice is valid");
                let requests = [request.with_fountain_paint_culling(false), request];
                let scenes = cache.render_layout_page_scene_requests(
                    &document,
                    &requests,
                    &options,
                    &fonts,
                    &mut |_| {},
                );
                let (without_culling, with_culling) = (&scenes[0], &scenes[1]);
                assert_eq!(without_culling.page.width, with_culling.page.width);
                assert_eq!(without_culling.page.height, with_culling.page.height);
                assert_eq!(
                    without_culling.page.text_diagnostics,
                    with_culling.page.text_diagnostics
                );
                assert_eq!(
                    without_culling.page.object_diagnostics,
                    with_culling.page.object_diagnostics
                );
                assert_eq!(
                    without_culling.page.geometry_diagnostics,
                    with_culling.page.geometry_diagnostics
                );
                assert_eq!(
                    without_culling.page.paint_diagnostics,
                    with_culling.page.paint_diagnostics
                );
                assert_eq!(without_culling.text.len(), with_culling.text.len());
                assert_native_ink_equal(without_culling, with_culling);

                let width = with_culling.page.width;
                let pixmap_without_culling = raster(&without_culling.page.svg, width, height);
                let pixmap_with_culling = raster(&with_culling.page.svg, width, height);
                let reference_circles = without_culling.page.svg.matches("<circle ").count();
                let culled_circles = with_culling.page.svg.matches("<circle ").count();
                assert!(
                    culled_circles < reference_circles,
                    "{case} slice {index} [{y}, {}) did not cull any Fountain stamps",
                    y + height
                );
                if pixmap_without_culling.data() != pixmap_with_culling.data() {
                    if let Some(directory) = &artifact_dir {
                        std::fs::create_dir_all(directory).unwrap();
                        let stem = format!("{case}-slice-{index:02}-y{y}-h{height}");
                        std::fs::write(
                            directory.join(format!("{stem}-reference.svg")),
                            &without_culling.page.svg,
                        )
                        .unwrap();
                        std::fs::write(
                            directory.join(format!("{stem}-culled.svg")),
                            &with_culling.page.svg,
                        )
                        .unwrap();
                        pixmap_without_culling
                            .save_png(directory.join(format!("{stem}-reference.png")))
                            .unwrap();
                        pixmap_with_culling
                            .save_png(directory.join(format!("{stem}-culled.png")))
                            .unwrap();
                    }
                    let first_difference = pixmap_without_culling
                        .data()
                        .iter()
                        .zip(pixmap_with_culling.data())
                        .position(|(left, right)| left != right)
                        .unwrap();
                    panic!(
                        "{case} slice {index} [{y}, {}) differs at byte {first_difference}",
                        y + height
                    );
                }
                eprintln!(
                    "slice-equivalence case={case} index={index} y={y} height={height} circles={reference_circles}->{culled_circles} pixels=exact",
                );
            }
        }
    }

    #[cfg(feature = "pdf")]
    fn pdf_object_dictionary(object: &lopdf::Object) -> Option<&lopdf::Dictionary> {
        match object {
            lopdf::Object::Stream(stream) => Some(&stream.dict),
            lopdf::Object::Dictionary(dict) => Some(dict),
            _ => None,
        }
    }

    #[cfg(feature = "serde")]
    #[test]
    fn inkpen2_profile_legacy_serde_defaults_enhanced_antialias_off() {
        let profile: InkPen2RenderProfile =
            serde_json::from_str(r#"{"screen_width":1920,"screen_height":1080}"#).unwrap();
        assert!(!profile.enhanced_antialias);
    }

    #[cfg(feature = "pdf")]
    fn assert_inkpen2_pdf_export(
        ink: &crate::PreparedInkPen2V9,
        transform: [f64; 6],
        options: &crate::PdfOptions,
        artifact: Option<(&std::path::Path, &str)>,
    ) {
        use super::vector::{Group, Scene, Styled, Svg, Transform};

        let mut scene = Scene::new(Svg::new().width(128).height(96));
        scene.retain_text();
        scene.scope(
            Group::new().transformed(Transform::matrix_unrounded(transform)),
            |scene| super::inkpen2_vector::render(scene, ink, None),
        );
        let text = scene.take_native_text();
        let registered = scene.take_native_ink();
        assert_eq!(registered.len(), usize::from(ink.opacity != 0.));
        let text_error = scene.take_native_text_error();
        assert!(text_error.is_none(), "{text_error:?}");
        let svg = scene.finish();
        let retained = super::RenderedScene {
            page: super::RenderedPage {
                source_page_index: 0,
                width: 128,
                height: 96,
                svg: svg.clone(),
                text_diagnostics: vec![],
                object_diagnostics: vec![],
                geometry_diagnostics: vec![],
                paint_diagnostics: vec![],
                background_diagnostics: vec![],
            },
            text,
            ink: registered,
            text_error,
        };
        let pdf = crate::pdf::render_native_scene_pdf_for_test(&retained, options).unwrap();
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        assert_eq!(parsed.get_pages().len(), 1);
        assert!(
            !parsed
                .objects
                .values()
                .any(|object| object.as_stream().is_ok_and(|stream| stream
                    .dict
                    .get(b"Subtype")
                    .is_ok_and(|v| v.as_name().is_ok_and(|name| name == b"Image"))))
        );
        if let Some((dir, name)) = artifact {
            write_new_artifact(&dir.join(format!("{name}.pdf")), &pdf);
            write_new_artifact(&dir.join(format!("{name}.svg")), svg.as_bytes());
        }
    }

    #[cfg(feature = "pdf")]
    fn inkpen2_bounds(records: &[[f32; 9]]) -> BoundingBox {
        records.iter().fold(
            BoundingBox {
                x_min: f64::INFINITY,
                y_min: f64::INFINITY,
                x_max: f64::NEG_INFINITY,
                y_max: f64::NEG_INFINITY,
            },
            |mut bounds, record| {
                let [cx, cy, rx, ry] = [record[0], record[1], record[2], record[3]];
                bounds.x_min = bounds.x_min.min(f64::from(cx) - f64::from(rx));
                bounds.y_min = bounds.y_min.min(f64::from(cy) - f64::from(ry));
                bounds.x_max = bounds.x_max.max(f64::from(cx) + f64::from(rx));
                bounds.y_max = bounds.y_max.max(f64::from(cy) + f64::from(ry));
                bounds
            },
        )
    }

    #[cfg(feature = "pdf")]
    fn inkpen2_reflected_chart(
        source: &crate::PreparedInkPen2V9,
        page_height: f32,
    ) -> crate::PreparedInkPen2V9 {
        let mut reflected = source.clone();
        for record in &mut reflected.records {
            record[1] = page_height - record[1];
        }
        reflected.bounds = inkpen2_bounds(&reflected.records);
        reflected
    }

    #[cfg(feature = "pdf")]
    fn sha256_hex(bytes: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        format!("{:x}", Sha256::digest(bytes))
    }

    #[cfg(feature = "pdf")]
    fn decode_hex(value: &str) -> Vec<u8> {
        assert_eq!(value.len() % 2, 0, "capture hex has odd length");
        value
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect()
    }

    #[cfg(feature = "pdf")]
    fn captured_inkpen2_records(bytes: &[u8]) -> Vec<[f32; 9]> {
        assert_eq!(
            bytes.len(),
            25 * 9 * 4,
            "unexpected captured native VBO size"
        );
        bytes
            .as_chunks::<36>()
            .0
            .iter()
            .map(|record| {
                std::array::from_fn(|index| {
                    let start = index * 4;
                    f32::from_le_bytes(record[start..start + 4].try_into().unwrap())
                })
            })
            .collect()
    }

    #[cfg(feature = "pdf")]
    fn record_bytes(records: &[[f32; 9]]) -> Vec<u8> {
        records
            .iter()
            .flatten()
            .flat_map(|value| value.to_le_bytes())
            .collect()
    }

    #[cfg(feature = "pdf")]
    fn write_new_artifact(path: &std::path::Path, bytes: &[u8]) {
        use std::{fs::OpenOptions, io::Write};

        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        file.write_all(bytes).unwrap();
    }

    #[cfg(feature = "pdf")]
    fn transformed_point(matrix: [f64; 6], point: [f64; 2]) -> [f64; 2] {
        let [a, b, c, d, e, f] = matrix;
        [
            a * point[0] + c * point[1] + e,
            b * point[0] + d * point[1] + f,
        ]
    }

    #[cfg(feature = "pdf")]
    fn inkpen2_chart_geometry_max_delta(
        source: &[[f32; 9]],
        reflected: &[[f32; 9]],
        baseline_matrix: [f64; 6],
        reflected_matrix: [f64; 6],
    ) -> f64 {
        assert_eq!(source.len(), reflected.len());
        source
            .iter()
            .zip(reflected)
            .flat_map(|(before, after)| {
                let mut deltas = Vec::with_capacity(6);
                for y_sign in [-1.0, 0.0, 1.0] {
                    let before_point = [
                        f64::from(before[0]),
                        f64::from(before[1]) + y_sign * f64::from(before[3]),
                    ];
                    let after_point = [
                        f64::from(after[0]),
                        f64::from(after[1]) - y_sign * f64::from(after[3]),
                    ];
                    let before_page = transformed_point(baseline_matrix, before_point);
                    let after_page = transformed_point(reflected_matrix, after_point);
                    deltas.extend([
                        (before_page[0] - after_page[0]).abs(),
                        (before_page[1] - after_page[1]).abs(),
                    ]);
                }
                deltas
            })
            .fold(0.0_f64, f64::max)
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn inkpen2_y_chart_conjugation_preserves_translated_asymmetric_geometry() {
        let records = vec![
            [12.375, 19.25, 2.125, 3.75, 0.125, 0.8, 1.0, 0.0, 0.0],
            [20.625, 31.875, 1.5, 2.25, 0.25, 0.6, 0.0, 0.0, 1.0],
        ];
        let baseline = crate::PreparedInkPen2V9 {
            per_stamp_rgb: true,
            enhanced_antialias: true,
            records,
            sample_ends: vec![0, 2],
            color: Color { r: 1, g: 2, b: 3 },
            opacity: 128.0 / 255.0,
            bounds: inkpen2_bounds(&[
                [12.375, 19.25, 2.125, 3.75, 0.125, 0.8, 1.0, 0.0, 0.0],
                [20.625, 31.875, 1.5, 2.25, 0.25, 0.6, 0.0, 0.0, 1.0],
            ]),
        };
        let page_height = 96.0_f32;
        let reflected = inkpen2_reflected_chart(&baseline, page_height);
        let scale = 2.0_f64;
        let baseline_matrix = [scale, 0.0, 0.0, -scale, 7.25, 91.5];
        let reflected_matrix = [
            scale,
            0.0,
            0.0,
            scale,
            7.25,
            91.5 - scale * f64::from(page_height),
        ];

        assert_eq!(reflected.records.len(), baseline.records.len());
        assert_eq!(reflected.sample_ends, baseline.sample_ends);
        assert_eq!(reflected.color, baseline.color);
        assert_eq!(reflected.opacity, baseline.opacity);
        assert_eq!(reflected.per_stamp_rgb, baseline.per_stamp_rgb);
        assert_eq!(reflected.enhanced_antialias, baseline.enhanced_antialias);
        for (source, reflected) in baseline.records.iter().zip(&reflected.records) {
            assert_eq!(reflected[0], source[0]);
            assert_eq!(reflected[1], page_height - source[1]);
            assert_eq!(&reflected[2..], &source[2..]);
            for y_sign in [-1.0, 1.0] {
                let source_edge = [
                    f64::from(source[0]),
                    f64::from(source[1]) + y_sign * f64::from(source[3]),
                ];
                let reflected_edge = [
                    f64::from(reflected[0]),
                    f64::from(reflected[1]) - y_sign * f64::from(reflected[3]),
                ];
                let before = transformed_point(baseline_matrix, source_edge);
                let after = transformed_point(reflected_matrix, reflected_edge);
                assert!((before[0] - after[0]).abs() < 1e-12);
                assert!((before[1] - after[1]).abs() < 1e-12);
            }
        }

        assert_eq!(baseline.bounds.x_min, reflected.bounds.x_min);
        assert_eq!(baseline.bounds.x_max, reflected.bounds.x_max);
        for source_x in [baseline.bounds.x_min, baseline.bounds.x_max] {
            for source_y in [baseline.bounds.y_min, baseline.bounds.y_max] {
                let reflected_y = f64::from(page_height) - source_y;
                let before = transformed_point(baseline_matrix, [source_x, source_y]);
                let after = transformed_point(reflected_matrix, [source_x, reflected_y]);
                assert!((before[0] - after[0]).abs() < 1e-12);
                assert!((before[1] - after[1]).abs() < 1e-12);
            }
        }
        assert!(baseline_matrix[3] < 0.0);
        assert!(reflected_matrix[3] > 0.0);
    }

    #[cfg(feature = "pdf")]
    #[test]
    #[ignore = "writes baseline and reflected-chart SVG/PDF pairs from the six captured native frames"]
    fn write_inkpen2_reflection_chart_artifact_pairs() {
        use serde_json::{Value, json};
        use std::path::PathBuf;

        let output_dir = std::env::var_os("SDOCX_INKPEN2_REFLECTION_ARTIFACT_DIR")
            .expect("set SDOCX_INKPEN2_REFLECTION_ARTIFACT_DIR to a fresh output directory");
        let output_dir = PathBuf::from(output_dir);
        std::fs::create_dir_all(&output_dir).unwrap();
        assert!(
            std::fs::read_dir(&output_dir).unwrap().next().is_none(),
            "artifact output directory must be empty"
        );
        let capture_dir = std::env::var_os("SDOCX_INKPEN2_CAPTURE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../..")
                    .join("tmp/vector-parity-plan/inkpen2-rainbow-paint/captures")
            });
        let cases = [
            ("scale1_aa1_alpha128", 1.0_f32, 128_u8),
            ("scale1_aa1_alpha255", 1.0, 255),
            ("scale2_aa1_alpha128", 2.0, 128),
            ("scale2_aa1_alpha255", 2.0, 255),
            ("scale4_aa1_alpha128", 4.0, 128),
            ("scale4_aa1_alpha255", 4.0, 255),
        ];
        let options =
            crate::PdfOptions::new(std::sync::Arc::new(crate::pdf::fontdb::Database::new()));
        let mut manifest_cases = Vec::new();
        for (case_id, scale, alpha) in cases {
            let capture_path = capture_dir.join(format!("{case_id}.json"));
            let capture_bytes = std::fs::read(&capture_path)
                .unwrap_or_else(|error| panic!("{}: {error}", capture_path.display()));
            let capture: Value = serde_json::from_slice(&capture_bytes).unwrap();
            let producer = &capture["joined_native_paint"]["producer"];
            let vbo = decode_hex(
                producer["native_AddPoint_vector_bytes_hex"]
                    .as_str()
                    .expect("native AddPoint VBO bytes"),
            );
            assert_eq!(vbo.len(), 25 * 9 * 4);
            let vbo_sha256 = sha256_hex(&vbo);
            assert_eq!(
                vbo_sha256,
                "570044bb059690f865cfa6f4f99c8ba7801573144242ee0913654973b0d0e987"
            );
            assert_eq!(producer["sha256"].as_str(), Some(vbo_sha256.as_str()));
            let stages = capture["joined_native_paint"]["native_stage_readbacks_before_invalidate"]
                .as_array()
                .expect("native mask, color and final frame captures");
            assert_eq!(stages.len(), 3);
            let native_stage_sha256 = stages
                .iter()
                .map(|stage| stage["rgba_sha256"].as_str().unwrap())
                .collect::<Vec<_>>();
            assert_eq!(
                capture["input"]["color_argb_u32"].as_u64().unwrap() >> 24,
                u64::from(alpha)
            );
            let source_records = captured_inkpen2_records(&vbo);
            let baseline_bounds = inkpen2_bounds(&source_records);
            let baseline = crate::PreparedInkPen2V9 {
                per_stamp_rgb: true,
                enhanced_antialias: true,
                records: source_records,
                sample_ends: vec![0, 25],
                color: Color {
                    r: 51,
                    g: 102,
                    b: 153,
                },
                opacity: f64::from(f32::from(alpha) / 255.0),
                bounds: baseline_bounds,
            };
            let reflected = inkpen2_reflected_chart(&baseline, 96.0);
            let baseline_matrix = [f64::from(scale), 0.0, 0.0, -f64::from(scale), 0.0, 96.0];
            let reflected_matrix = [
                f64::from(scale),
                0.0,
                0.0,
                f64::from(scale),
                0.0,
                96.0 - 96.0 * f64::from(scale),
            ];
            let physical_geometry_max_delta = inkpen2_chart_geometry_max_delta(
                &baseline.records,
                &reflected.records,
                baseline_matrix,
                reflected_matrix,
            );
            assert!(
                physical_geometry_max_delta <= 1e-4,
                "{case_id}: chart reflection moved geometry by {physical_geometry_max_delta} page points"
            );
            let baseline_name = format!("{case_id}.baseline-reflected");
            let reflected_name = format!("{case_id}.positive-y-chart");
            assert_inkpen2_pdf_export(
                &baseline,
                baseline_matrix,
                &options,
                Some((&output_dir, &baseline_name)),
            );
            assert_inkpen2_pdf_export(
                &reflected,
                reflected_matrix,
                &options,
                Some((&output_dir, &reflected_name)),
            );
            let baseline_svg =
                std::fs::read(output_dir.join(format!("{baseline_name}.svg"))).unwrap();
            let candidate_svg =
                std::fs::read(output_dir.join(format!("{reflected_name}.svg"))).unwrap();
            let baseline_pdf =
                std::fs::read(output_dir.join(format!("{baseline_name}.pdf"))).unwrap();
            let candidate_pdf =
                std::fs::read(output_dir.join(format!("{reflected_name}.pdf"))).unwrap();
            manifest_cases.push(json!({
                "case_id": case_id,
                "scale": scale,
                "alpha": alpha,
                "source_profile": capture["profile"],
                "native_input_fields": {
                    "pen_name": capture["input"]["pen_name"],
                    "stroke_type": capture["input"]["stroke_type"],
                    "tool_type": capture["input"]["tool_type"],
                    "curve_enabled": capture["input"]["curve_enabled"],
                    "advanced_setting": capture["input"]["advanced_setting"],
                    "color_argb_u32": capture["input"]["color_argb_u32"],
                    "points_f32_bits": capture["input"]["points_f32_bits"],
                    "pressure_f32_bits": capture["input"]["pressure_f32_bits"],
                    "timestamps_i32": capture["input"]["timestamps_i32"]
                },
                "native_scope": capture["joined_native_paint"]["boundaries"],
                "native_draw_matrix": capture["joined_native_paint"]["paint"]["readback"]["draw_matrix_f32_row_major"],
                "native_final_rgba_sha256": capture["joined_native_paint"]["paint"]["readback"]["sha256"],
                "native_capture_sha256": sha256_hex(&capture_bytes),
                "native_vbo_sha256": vbo_sha256,
                "native_stage_sha256": native_stage_sha256,
                "source_vbo_unchanged": true,
                "records_sha256": {
                    "baseline": sha256_hex(&record_bytes(&baseline.records)),
                    "positive_y_chart": sha256_hex(&record_bytes(&reflected.records)),
                },
                "page": [128, 96],
                "physical_transform_pair": {
                    "baseline_reflected": baseline_matrix,
                    "positive_y_chart": reflected_matrix,
                    "coordinate_relation": "candidate cy = 96 - source cy; candidate page y = scale * candidate cy + 96 * (1 - scale) = 96 - scale * source cy"
                },
                "physical_geometry_max_delta_page_points": physical_geometry_max_delta,
                "physical_geometry_max_delta_bound_page_points": 0.0001,
                "bounds": {
                    "baseline": [baseline.bounds.x_min, baseline.bounds.y_min, baseline.bounds.x_max, baseline.bounds.y_max],
                    "positive_y_chart": [reflected.bounds.x_min, reflected.bounds.y_min, reflected.bounds.x_max, reflected.bounds.y_max]
                },
                "artifacts": {
                    "baseline_svg": {"path": format!("{baseline_name}.svg"), "sha256": sha256_hex(&baseline_svg)},
                    "positive_y_chart_svg": {"path": format!("{reflected_name}.svg"), "sha256": sha256_hex(&candidate_svg)},
                    "baseline_pdf": {"path": format!("{baseline_name}.pdf"), "sha256": sha256_hex(&baseline_pdf)},
                    "positive_y_chart_pdf": {"path": format!("{reflected_name}.pdf"), "sha256": sha256_hex(&candidate_pdf)}
                }
            }));
        }
        let manifest = json!({
            "schema": "sdocx-inkpen2-reflection-chart-pairs-v1",
            "purpose": "test-only comparison of the same physical field under a reflected baseline CTM and conjugated positive-Y record chart",
            "admission_changed": false,
            "native_vbo_or_phase_changed": false,
            "svg_phase_fit": false,
            "reader_comparison": "compare complete 128x96 premultiplied RGBA from emitted SVG/PDF artifacts against the same captured native frame; reverse native GL rows once; no phase fitting",
            "writer": {
                "test_filter": "render::tests::write_inkpen2_reflection_chart_artifact_pairs",
                "environment": "SDOCX_INKPEN2_REFLECTION_ARTIFACT_DIR",
                "capture_override": "SDOCX_INKPEN2_CAPTURE_DIR",
                "source_sha256": sha256_hex(include_bytes!("render.rs"))
            },
            "cases": manifest_cases
        });
        let manifest_bytes = serde_json::to_vec_pretty(&manifest).unwrap();
        write_new_artifact(&output_dir.join("manifest.json"), &manifest_bytes);
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn inkpen2_native_gradient_records_reach_retained_vector_pdf() {
        use crate::ink::inkpen2::{Input, Rainbow, Runtime, redraw, source_point};
        use sha2::Digest;

        let points = [(0., 0.), (8., 0.), (16., 3.), (24., 3.)].map(|(x, y)| source_point(x, y));
        let output = redraw(Input {
            points: &points,
            times: &[0, 10, 20, 30],
            tools: &[2; 4],
            pressures: &[0.2, 0.4, 0.6, 0.8],
            rgb: [51, 102, 153],
            rainbow: Some(Rainbow {
                distance: 5,
                offset: 2.5,
                palette_argb: &[0xffff0000, 0xff0000ff],
            }),
            size: 4.,
            curve_enabled: true,
            is_shape: false,
            runtime: Runtime {
                inverse_scale: [1., 1.],
                spacing: 1.,
                tolerance: 0.,
                tool_threshold: 20.,
                resolution_min: 1080,
                initial_alternation: false,
                initial_time: 0,
            },
        })
        .unwrap();
        let bytes: Vec<_> = output
            .vertices
            .iter()
            .flatten()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        assert_eq!(bytes.len(), 900);
        assert_eq!(
            format!("{:x}", sha2::Sha256::digest(&bytes)),
            "570044bb059690f865cfa6f4f99c8ba7801573144242ee0913654973b0d0e987"
        );
        let mut bounds = BoundingBox {
            x_min: f64::INFINITY,
            y_min: f64::INFINITY,
            x_max: f64::NEG_INFINITY,
            y_max: f64::NEG_INFINITY,
        };
        for &[x, y, rx, ry, ..] in &output.vertices {
            bounds.x_min = bounds.x_min.min(f64::from(x) - f64::from(rx));
            bounds.y_min = bounds.y_min.min(f64::from(y) - f64::from(ry));
            bounds.x_max = bounds.x_max.max(f64::from(x) + f64::from(rx));
            bounds.y_max = bounds.y_max.max(f64::from(y) + f64::from(ry));
        }
        // Optional paired-export artifacts; ordinary tests stay entirely in memory.
        let artifact_dir = std::env::var_os("SDOCX_INKPEN2_PDF_DIR").map(std::path::PathBuf::from);
        if let Some(dir) = &artifact_dir {
            std::fs::create_dir(dir).unwrap();
        }
        let options =
            crate::PdfOptions::new(std::sync::Arc::new(crate::pdf::fontdb::Database::new()));
        for enhanced in [false, true] {
            for alpha in [0_u8, 128, 255] {
                for scale in [1, 2, 4] {
                    let name = format!("scale{scale}_aa{}_alpha{alpha}", u8::from(enhanced));
                    let ink = crate::PreparedInkPen2V9 {
                        per_stamp_rgb: true,
                        enhanced_antialias: enhanced,
                        records: output.vertices.clone(),
                        sample_ends: output.sample_ends.clone(),
                        color: Color {
                            r: 51,
                            g: 102,
                            b: 153,
                        },
                        opacity: f64::from(f32::from(alpha) / 255.),
                        bounds,
                    };
                    assert_inkpen2_pdf_export(
                        &ink,
                        [f64::from(scale), 0., 0., -f64::from(scale), 0., 96.],
                        &options,
                        artifact_dir.as_deref().map(|dir| (dir, name.as_str())),
                    );
                }
            }
        }
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn inkpen2_native_thin_rainbow_records_reach_retained_vector_pdf() {
        use crate::ink::inkpen2::{Input, Rainbow, Runtime, redraw, source_point};
        use sha2::Digest;

        let points = [(0., 0.), (8., 0.), (16., 3.), (24., 3.)].map(|(x, y)| source_point(x, y));
        let cases = [
            (
                0.4_f32,
                "0_4",
                "9a439505b800212c5dcaf83bd6033dc9a84ed246a6b2bc5ae51b486200f3ac84",
            ),
            (
                0.6_f32,
                "0_6",
                "913714dd5d0f6dcf390f6c7b86a3cc669343cc64e6f1b7f88e07b1100f185809",
            ),
            (
                1.0_f32,
                "1_0",
                "021014a354c5f65195836a8c7a0ba017884a422dacf91813f73455914a4591b4",
            ),
        ];
        let artifact_dir =
            std::env::var_os("SDOCX_INKPEN2_THIN_PDF_DIR").map(std::path::PathBuf::from);
        if let Some(dir) = &artifact_dir {
            std::fs::create_dir(dir).unwrap();
        }
        let options =
            crate::PdfOptions::new(std::sync::Arc::new(crate::pdf::fontdb::Database::new()));
        for (size, size_tag, expected_hash) in cases {
            let output = redraw(Input {
                points: &points,
                times: &[0, 10, 20, 30],
                tools: &[2; 4],
                pressures: &[0.2, 0.4, 0.6, 0.8],
                rgb: [18, 52, 86],
                rainbow: Some(Rainbow {
                    distance: 5,
                    offset: 2.5,
                    palette_argb: &[0xffff0000, 0xff0000ff],
                }),
                size,
                curve_enabled: true,
                is_shape: false,
                runtime: Runtime {
                    inverse_scale: [1., 1.],
                    spacing: 1.,
                    tolerance: 0.,
                    tool_threshold: 20.,
                    resolution_min: 1080,
                    initial_alternation: false,
                    initial_time: 0,
                },
            })
            .unwrap();
            let bytes: Vec<_> = output
                .vertices
                .iter()
                .flatten()
                .flat_map(|value| value.to_le_bytes())
                .collect();
            assert_eq!(bytes.len(), 900);
            assert_eq!(format!("{:x}", sha2::Sha256::digest(&bytes)), expected_hash);
            let mut bounds = BoundingBox {
                x_min: f64::INFINITY,
                y_min: f64::INFINITY,
                x_max: f64::NEG_INFINITY,
                y_max: f64::NEG_INFINITY,
            };
            for &[x, y, rx, ry, ..] in &output.vertices {
                bounds.x_min = bounds.x_min.min(f64::from(x) - f64::from(rx));
                bounds.y_min = bounds.y_min.min(f64::from(y) - f64::from(ry));
                bounds.x_max = bounds.x_max.max(f64::from(x) + f64::from(rx));
                bounds.y_max = bounds.y_max.max(f64::from(y) + f64::from(ry));
            }
            for enhanced in [false, true] {
                for scale in [1, 4] {
                    let name = format!(
                        "thin_size{size_tag}_scale{scale}_aa{}_alpha128",
                        u8::from(enhanced)
                    );
                    let ink = crate::PreparedInkPen2V9 {
                        per_stamp_rgb: true,
                        enhanced_antialias: enhanced,
                        records: output.vertices.clone(),
                        sample_ends: output.sample_ends.clone(),
                        color: Color {
                            r: 18,
                            g: 52,
                            b: 86,
                        },
                        opacity: f64::from(128_f32 / 255.),
                        bounds,
                    };
                    assert_inkpen2_pdf_export(
                        &ink,
                        [f64::from(scale), 0., 0., f64::from(scale), 16., 16.],
                        &options,
                        artifact_dir.as_deref().map(|dir| (dir, name.as_str())),
                    );
                }
            }
        }
    }

    #[test]
    fn inkpen2_enhanced_aa_keeps_max_coverage_and_applies_source_alpha_once() {
        let mut base = inkpen2_color_stroke(0xff33_6699, None);
        base.points = vec![
            Point { x: 0.0, y: 0.0 },
            Point { x: 8.0, y: 0.0 },
            Point { x: 16.0, y: 3.0 },
            Point { x: 24.0, y: 3.0 },
        ];
        base.timestamps = vec![100, 110, 120, 130];
        base.pressures = vec![0.2, 0.4, 0.6, 0.8];
        base.tilts = vec![0.0; 4];
        base.orientations = vec![0.0; 4];
        let profile = InkPen2RenderProfile {
            screen_width: 1920,
            screen_height: 1080,
            enhanced_antialias: true,
        };
        let options = RenderOptions {
            inkpen2_profile: Some(profile),
            chinesebrush_profile: None,
            color_mode: RenderColorMode::Light,
            ..RenderOptions::default()
        };
        let mut control_records = None;
        for alpha in [255_u8, 128, 0] {
            let opacity = f64::from(f32::from(alpha) / 255.0);
            let argb = (u32::from(alpha) << 24) | 0x0033_6699;
            let stroke = Stroke {
                rendering: Some(StrokeRendering {
                    style: StrokeStyle {
                        color_argb: Some(argb),
                        ..base.rendering.as_ref().unwrap().style.clone()
                    },
                    ..base.rendering.as_ref().unwrap().clone()
                }),
                ..base.clone()
            };
            let prepared =
                prepare_inkpen2_v9(&stroke, Some(profile), RenderTheme::for_canvas(false))
                    .expect("AA V9 profile admits the captured overlapping source geometry");
            assert!(prepared.enhanced_antialias);
            assert_eq!(prepared.opacity, opacity);
            if let Some(records) = &control_records {
                assert_eq!(
                    prepared.records, *records,
                    "ARGB alpha does not alter coverage"
                );
            } else {
                control_records = Some(prepared.records.clone());
            }

            let mut page = page_with_uncolored_stroke();
            page.objects = vec![stroke.into()];
            let svg = render_document_svg(&document(page), &options)[0]
                .svg
                .clone();
            if alpha == 0 {
                assert!(!svg.contains("<filter"));
                assert!(!svg.contains("mix-blend-mode:lighten"));
            } else {
                assert!(svg.contains("<filter"));
                assert!(svg.contains("feColorMatrix"));
                assert!(svg.contains("mix-blend-mode:lighten"));
                let root = roxmltree::Document::parse(&svg).unwrap();
                let colorizer = root
                    .descendants()
                    .find(|node| node.attribute("filter").is_some())
                    .expect("one filtered MAX colorizer group");
                let group_opacity = colorizer
                    .attribute("opacity")
                    .unwrap_or("1")
                    .parse::<f64>()
                    .unwrap();
                assert!((group_opacity - opacity).abs() < 1e-7);
            }
        }
    }

    fn calligraphy_stroke(source: &serde_json::Value) -> Stroke {
        let floats = |bits_key: &str, bytes_key: &str| {
            let hex_words = if let Some(values) = source[bits_key].as_array() {
                values
                    .iter()
                    .map(|value| value.as_str().unwrap().to_owned())
                    .collect::<Vec<_>>()
            } else {
                source[bytes_key]
                    .as_str()
                    .unwrap()
                    .as_bytes()
                    .as_chunks::<8>()
                    .0
                    .iter()
                    .map(|word| std::str::from_utf8(word).unwrap().to_owned())
                    .collect()
            };
            hex_words
                .iter()
                .map(|word| {
                    let mut raw = [0_u8; 4];
                    for (index, pair) in word.as_bytes().as_chunks::<2>().0.iter().enumerate() {
                        raw[index] =
                            u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap();
                    }
                    f32::from_le_bytes(raw)
                })
                .collect::<Vec<_>>()
        };
        let point_values = floats("points_f32_bits", "point_bytes_f32_le");
        let points = point_values
            .as_chunks::<2>()
            .0
            .iter()
            .map(|xy| Point {
                x: f64::from(xy[0]),
                y: f64::from(xy[1]),
            })
            .collect::<Vec<_>>();
        let pressures = floats("pressure_f32_bits", "pressure_bytes_f32_le")
            .into_iter()
            .map(f64::from)
            .collect();
        let tilts = floats("tilt_f32_bits", "tilt_bytes_f32_le")
            .into_iter()
            .map(f64::from)
            .collect();
        let orientations = floats("orientation_f32_bits", "orientation_bytes_f32_le")
            .into_iter()
            .map(f64::from)
            .collect();
        let timestamps = source["timestamps_i32"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_i64().unwrap())
            .collect();
        let argb = source["argb_u32"].as_u64().unwrap() as u32;
        let pen_size_bytes = source["pen_size_f32_bits"].as_str().unwrap();
        let pen_size = f32::from_le_bytes(std::array::from_fn(|index| {
            u8::from_str_radix(&pen_size_bytes[index * 2..index * 2 + 2], 16).unwrap()
        }));
        Stroke {
            rendering: Some(StrokeRendering {
                metadata: None,
                pen_name: source["pen_name"].as_str().map(str::to_owned),
                advanced_settings: None,
                tool_type_raw: source["tool_type"].as_u64().unwrap() as u16,
                properties: StrokeProperties {
                    compressed: true,
                    replay_only: false,
                    stylus_channels: true,
                    eraser: false,
                    fixed_width: false,
                    millisecond_timestamps: false,
                    top_layer_pen: false,
                    alpha_lock: false,
                    binary_added: true,
                    generated: false,
                    fixed_opacity: false,
                    rainbow_effect: false,
                    straighten: false,
                    reveal_mode: false,
                },
                style: StrokeStyle {
                    color_argb: Some(argb),
                    pen_size: Some(pen_size),
                    stroke_type_raw: Some(source["stroke_type"].as_u64().unwrap() as u16),
                    color_type_raw: Some(0),
                    ..StrokeStyle::default()
                },
            }),
            bbox: BoundingBox::default(),
            points,
            pressures,
            timestamps,
            tilts,
            orientations,
            color: Some(Color {
                r: (argb >> 16) as u8,
                g: (argb >> 8) as u8,
                b: argb as u8,
            }),
            pen_width: pen_size,
        }
    }

    fn oblique_saved_stroke(selected_v1: bool) -> Stroke {
        let (fixture, case) = if selected_v1 {
            (
                include_str!("../../../conformance/oblique-saved-caller-selected-native.json"),
                "three_points_curve",
            )
        } else {
            (
                include_str!("../../../conformance/oblique-saved-caller-advanced4-native.json"),
                "two_point_at5",
            )
        };
        let fixture: serde_json::Value = serde_json::from_str(fixture).unwrap();
        let input = &fixture["cases"][case]["input"];
        let float = |bits: &str| {
            let bytes = std::array::from_fn(|index| {
                u8::from_str_radix(&bits[index * 2..index * 2 + 2], 16).unwrap()
            });
            f32::from_le_bytes(bytes)
        };
        let points = input["points_f32_bits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|pair| Point {
                x: f64::from(float(pair[0].as_str().unwrap())),
                y: f64::from(float(pair[1].as_str().unwrap())),
            })
            .collect::<Vec<_>>();
        let pressures = input["pressure_f32_bits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|bits| f64::from(float(bits.as_str().unwrap())))
            .collect();
        let argb = 0x8011_2233;
        let pen_size = float(input["pen_size_f32_bits"].as_str().unwrap());
        Stroke {
            rendering: Some(StrokeRendering {
                metadata: None,
                pen_name: input["pen_name"].as_str().map(str::to_owned),
                advanced_settings: input["advanced_setting"].as_str().map(str::to_owned),
                tool_type_raw: input["tool_type"].as_u64().unwrap() as u16,
                properties: StrokeProperties {
                    compressed: input["curve_enabled"].as_bool().unwrap(),
                    replay_only: false,
                    stylus_channels: true,
                    eraser: false,
                    fixed_width: false,
                    millisecond_timestamps: false,
                    top_layer_pen: false,
                    alpha_lock: false,
                    binary_added: true,
                    generated: false,
                    fixed_opacity: false,
                    rainbow_effect: false,
                    straighten: false,
                    reveal_mode: false,
                },
                style: StrokeStyle {
                    color_argb: Some(argb),
                    pen_size: Some(pen_size),
                    stroke_type_raw: Some(input["stroke_type"].as_u64().unwrap() as u16),
                    color_type_raw: Some(0),
                    ..StrokeStyle::default()
                },
            }),
            bbox: BoundingBox::default(),
            points,
            pressures,
            timestamps: input["timestamps_i32"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_i64().unwrap())
                .collect(),
            tilts: Vec::new(),
            orientations: Vec::new(),
            color: Some(Color {
                r: 0x11,
                g: 0x22,
                b: 0x33,
            }),
            pen_width: pen_size,
        }
    }

    fn path_data(svg: &str) -> Vec<String> {
        roxmltree::Document::parse(svg)
            .unwrap()
            .descendants()
            .filter(|node| node.has_tag_name("path"))
            .filter_map(|node| node.attribute("d").map(str::to_owned))
            .collect()
    }

    fn gradient_path_data(svg: &str) -> Vec<String> {
        roxmltree::Document::parse(svg)
            .unwrap()
            .descendants()
            .filter(|node| node.has_tag_name("path"))
            .filter(|node| {
                node.attribute("fill")
                    .is_some_and(|fill| fill.starts_with("url(#"))
            })
            .filter_map(|node| node.attribute("d").map(str::to_owned))
            .collect()
    }

    fn render_objects(objects: Vec<crate::PageObject>, replay: bool) -> String {
        let mut page = page_with_uncolored_stroke();
        page.objects = objects;
        let document = document(page);
        if replay {
            let layout = layout_document(&document);
            super::render_layout_page_replay_svg(&document, &layout, 0, &RenderOptions::default())
                .unwrap()
                .svg
        } else {
            render_document_svg(&document, &RenderOptions::default())[0]
                .svg
                .clone()
        }
    }

    #[test]
    fn defaultpen_public_replay_keeps_each_native_instance_in_source_order() {
        let native: serde_json::Value = serde_json::from_str(include_str!(
            "../../../conformance/defaultpen-saved-caller-native.json"
        ))
        .unwrap();
        let native = &native["cases"]["straight_curve_tool1_default"]["streams"]["draw_object_stroke"]
            ["values_f32_bits"];
        let native_values = native
            .as_array()
            .unwrap()
            .iter()
            .map(|value| {
                let value = value.as_str().unwrap();
                let bytes = std::array::from_fn(|index| {
                    u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).unwrap()
                });
                f32::from_le_bytes(bytes)
            })
            .collect::<Vec<_>>();
        let expected = native_values
            .as_chunks::<3>()
            .0
            .iter()
            .map(|triple| [triple[0], triple[1], triple[2]])
            .collect::<Vec<_>>();
        let points = [(0., 0.), (10., 0.), (20., 0.), (30., 10.)];
        let stroke = Stroke {
            rendering: Some(StrokeRendering {
                metadata: None,
                pen_name: Some("com.samsung.android.sdk.pen.pen.preload.DefaultPen".into()),
                advanced_settings: None,
                tool_type_raw: 1,
                properties: StrokeProperties {
                    compressed: true,
                    replay_only: false,
                    stylus_channels: false,
                    eraser: false,
                    fixed_width: false,
                    millisecond_timestamps: false,
                    top_layer_pen: false,
                    alpha_lock: false,
                    binary_added: true,
                    generated: false,
                    fixed_opacity: false,
                    rainbow_effect: false,
                    straighten: false,
                    reveal_mode: false,
                },
                style: StrokeStyle {
                    color_argb: Some(0x0033_6699),
                    pen_size: Some(10.),
                    stroke_type_raw: Some(0),
                    color_type_raw: Some(0),
                    ..StrokeStyle::default()
                },
            }),
            bbox: BoundingBox::default(),
            points: points.iter().map(|&(x, y)| Point { x, y }).collect(),
            pressures: vec![0.25, 0.5, 0.75, 1.],
            timestamps: vec![10, 20, 30, 40],
            tilts: vec![0.; 4],
            orientations: vec![0.; 4],
            color: Some(Color {
                r: 0x33,
                g: 0x66,
                b: 0x99,
            }),
            pen_width: 10.,
        };
        let rendered = render_objects(vec![stroke.into()], true);
        let xml = roxmltree::Document::parse(&rendered).unwrap();
        let circles = xml
            .descendants()
            .filter(|node| node.has_tag_name("circle"))
            .collect::<Vec<_>>();
        assert_eq!(circles.len(), expected.len());
        assert!(
            circles
                .iter()
                .all(|node| node.attribute("fill") == Some("#336699"))
        );
        assert!(
            circles
                .iter()
                .all(|node| node.attribute("data-replay-part") == Some("4"))
        );
        for (node, expected) in circles.iter().zip(expected) {
            for (attribute, value) in [("cx", expected[0]), ("cy", expected[1]), ("r", expected[2])]
            {
                let actual = node.attribute(attribute).unwrap().parse::<f32>().unwrap();
                assert!(
                    (actual - value).abs() <= 0.000_051,
                    "public SVG {attribute}={actual} differs from native callback {value}"
                );
            }
        }
        assert!(xml.descendants().any(
            |node| node.has_tag_name("g") && node.attribute("data-replay-stroke") == Some("0")
        ));
    }

    #[test]
    fn defaultpen_terminal_only_cases_replay_at_the_terminal_event() {
        for (points, terminal_part) in [
            (vec![(0., 0.), (10., 0.)], 2),
            (vec![(0., 0.), (0., 0.), (0., 0.), (0., 0.)], 4),
        ] {
            let mut stroke = Stroke {
                rendering: Some(StrokeRendering {
                    metadata: None,
                    pen_name: Some("com.samsung.android.sdk.pen.pen.preload.DefaultPen".into()),
                    advanced_settings: Some("anything".into()),
                    tool_type_raw: 2,
                    properties: StrokeProperties {
                        compressed: true,
                        replay_only: false,
                        stylus_channels: false,
                        eraser: false,
                        fixed_width: false,
                        millisecond_timestamps: false,
                        top_layer_pen: false,
                        alpha_lock: false,
                        binary_added: true,
                        generated: false,
                        fixed_opacity: false,
                        rainbow_effect: false,
                        straighten: false,
                        reveal_mode: false,
                    },
                    style: StrokeStyle {
                        pen_size: Some(10.),
                        color_argb: Some(0xff00_0000),
                        color_type_raw: Some(0),
                        stroke_type_raw: Some(0),
                        ..StrokeStyle::default()
                    },
                }),
                bbox: BoundingBox::default(),
                points: points.iter().map(|&(x, y)| Point { x, y }).collect(),
                pressures: vec![1.; points.len()],
                timestamps: (0..points.len() as i64).collect(),
                tilts: vec![0.; points.len()],
                orientations: vec![0.; points.len()],
                color: Some(Color { r: 0, g: 0, b: 0 }),
                pen_width: 10.,
            };
            // Keep the stored curve bit explicit even though these terminal-only
            // captures produce one native EndPen stamp in either mode.
            stroke.rendering.as_mut().unwrap().properties.compressed = true;
            let svg = render_objects(vec![stroke.into()], true);
            let xml = roxmltree::Document::parse(&svg).unwrap();
            let circles = xml
                .descendants()
                .filter(|node| node.has_tag_name("circle"))
                .collect::<Vec<_>>();
            assert_eq!(circles.len(), 1);
            let part = terminal_part.to_string();
            assert_eq!(
                circles[0].attribute("data-replay-part"),
                Some(part.as_str())
            );
            let expected = (0.0, 0.0);
            assert_eq!(
                circles[0].attribute("cx").unwrap().parse::<f32>().unwrap(),
                expected.0
            );
            assert_eq!(
                circles[0].attribute("cy").unwrap().parse::<f32>().unwrap(),
                expected.1
            );
        }
    }

    #[test]
    fn defaultpen_keeps_theme_foreground_adaptation() {
        let mut stroke = Stroke {
            rendering: Some(StrokeRendering {
                metadata: None,
                pen_name: Some("com.samsung.android.sdk.pen.pen.preload.DefaultPen".into()),
                advanced_settings: None,
                tool_type_raw: 1,
                properties: StrokeProperties {
                    compressed: true,
                    replay_only: false,
                    stylus_channels: false,
                    eraser: false,
                    fixed_width: false,
                    millisecond_timestamps: false,
                    top_layer_pen: false,
                    alpha_lock: false,
                    binary_added: true,
                    generated: false,
                    fixed_opacity: false,
                    rainbow_effect: false,
                    straighten: false,
                    reveal_mode: false,
                },
                style: StrokeStyle {
                    color_argb: Some(0xff00_0000),
                    pen_size: Some(10.),
                    color_type_raw: Some(0),
                    stroke_type_raw: Some(0),
                    ..StrokeStyle::default()
                },
            }),
            bbox: BoundingBox::default(),
            points: vec![Point { x: 0., y: 0. }, Point { x: 10., y: 0. }],
            pressures: vec![1.; 2],
            timestamps: vec![0, 10],
            tilts: vec![0.; 2],
            orientations: vec![0.; 2],
            color: Some(Color { r: 0, g: 0, b: 0 }),
            pen_width: 10.,
        };
        let mut page = page_with_uncolored_stroke();
        page.objects = vec![stroke.clone().into()];
        let svg = render_document_svg(
            &document(page),
            &RenderOptions {
                color_mode: RenderColorMode::Dark,
                ..Default::default()
            },
        )[0]
        .svg
        .clone();
        assert!(svg.contains(r##"fill="#ffffff""##));

        // A caller-edited Stroke.color remains the public paint override even
        // when the native style ARGB retained its earlier RGB channels.
        stroke.color = Some(Color { r: 255, g: 0, b: 0 });
        let mut page = page_with_uncolored_stroke();
        page.objects = vec![stroke.into()];
        let svg = render_document_svg(&document(page), &RenderOptions::default())[0]
            .svg
            .clone();
        assert!(svg.contains(r##"fill="#ff0000""##));
    }

    fn expected_calligraphy_paths(strokes: &[Stroke], reset_each: bool) -> Vec<String> {
        let mut scene = super::Scene::new(super::Svg::new().width(100).height(100));
        let mut session = crate::ink::calligraphy::CalligraphySession::new();
        for stroke in strokes {
            if reset_each {
                session.reset();
            }
            if let crate::ink::calligraphy::CalligraphyAdmission::Mesh(mesh) =
                crate::ink::calligraphy::prepare_model_stroke(&mut session, stroke)
            {
                super::calligraphy::render_model(&mut scene, &mesh, false).unwrap();
            }
        }
        path_data(&scene.finish())
    }

    #[test]
    fn public_svg_render_uses_chained_calligraphy_sessions_and_root_boundaries() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../conformance/calligraphy-saved-caller-native.json"
        ))
        .unwrap();
        let sources = fixture["cases"]["two-stroke-object-list-batch"]["input"]["sources"]
            .as_array()
            .unwrap();
        let strokes = sources.iter().map(calligraphy_stroke).collect::<Vec<_>>();
        assert_eq!(strokes.len(), 2);

        let chained = render_objects(strokes.iter().cloned().map(Into::into).collect(), false);
        assert_eq!(
            path_data(&chained),
            expected_calligraphy_paths(&strokes, false)
        );
        assert!(chained.contains("#112233"));

        let mut promoted_calligraphy = strokes[0].clone();
        for point in &mut promoted_calligraphy.points {
            point.x += 37.0;
        }
        let promoted_descendant = crate::PageObject {
            render_layer: crate::ObjectRenderLayer::Base,
            source_offset: None,
            content: crate::PageObjectContent::Descendant(Box::new(
                crate::PageObjectContent::Stroke(promoted_calligraphy),
            )),
        };
        let with_promoted_child = render_objects(
            vec![
                strokes[0].clone().into(),
                promoted_descendant,
                strokes[1].clone().into(),
            ],
            false,
        );
        assert_eq!(
            gradient_path_data(&with_promoted_child),
            gradient_path_data(&chained),
            "a promoted non-container child is drawn generically and leaves the native session untouched"
        );

        let separated = render_objects(
            vec![
                strokes[0].clone().into(),
                PageElement::Image {
                    bbox: BoundingBox::default(),
                    media_index: usize::MAX,
                }
                .into(),
                strokes[1].clone().into(),
            ],
            false,
        );
        assert_eq!(
            path_data(&separated),
            expected_calligraphy_paths(&strokes, true),
            "a root image boundary starts a fresh Standard bitmap Drawing"
        );
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn public_calligraphy_pdf_keeps_mesh_vector_only() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../conformance/calligraphy-saved-caller-native.json"
        ))
        .unwrap();
        let sources = fixture["cases"]["two-stroke-object-list-batch"]["input"]["sources"]
            .as_array()
            .unwrap();
        let strokes = sources.iter().map(calligraphy_stroke).collect::<Vec<_>>();
        let document = document({
            let mut page = page_with_uncolored_stroke();
            page.objects = strokes.into_iter().map(Into::into).collect();
            page
        });
        let bytes = crate::render_document_pdf(
            &document,
            &RenderOptions::default(),
            &crate::PdfOptions::default(),
        )
        .unwrap();
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        let images = pdf
            .objects
            .values()
            .filter_map(pdf_object_dictionary)
            .filter(|dict| {
                dict.get(b"Subtype")
                    .and_then(lopdf::Object::as_name)
                    .is_ok_and(|name| name == b"Image")
            })
            .count();
        assert_eq!(
            images, 0,
            "Calligraphy export must stay in vector paths/shadings"
        );
        assert!(pdf.objects.values().any(|object| {
            object
                .as_dict()
                .ok()
                .and_then(|dict| dict.get(b"Shading").ok())
                .is_some()
        }));
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn public_inkpen2_enhanced_aa_pdf_has_no_raster_image_fallback() {
        let mut stroke = inkpen2_color_stroke(0x80_336699, None);
        stroke.points = vec![
            Point { x: 0.0, y: 0.0 },
            Point { x: 8.0, y: 0.0 },
            Point { x: 16.0, y: 3.0 },
            Point { x: 24.0, y: 3.0 },
        ];
        stroke.timestamps = vec![100, 110, 120, 130];
        stroke.pressures = vec![0.2, 0.4, 0.6, 0.8];
        stroke.tilts = vec![0.0; 4];
        stroke.orientations = vec![0.0; 4];
        let mut page = page_with_uncolored_stroke();
        let profile = InkPen2RenderProfile {
            screen_width: 1920,
            screen_height: 1080,
            enhanced_antialias: true,
        };
        assert!(
            prepare_inkpen2_v9(&stroke, Some(profile), RenderTheme::for_canvas(false))
                .is_some_and(|ink| ink.enhanced_antialias),
            "the PDF case must take the enhanced-AA profile"
        );
        page.objects = vec![stroke.into()];
        let document = document(page);
        let options = RenderOptions {
            inkpen2_profile: Some(profile),
            chinesebrush_profile: None,
            ..RenderOptions::default()
        };
        let bytes =
            crate::render_document_pdf(&document, &options, &crate::PdfOptions::default()).unwrap();
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        let images = pdf
            .objects
            .values()
            .filter_map(pdf_object_dictionary)
            .filter(|dict| {
                dict.get(b"Subtype")
                    .and_then(lopdf::Object::as_name)
                    .is_ok_and(|name| name == b"Image")
            })
            .count();
        assert_eq!(images, 0, "enhanced-AA profile must remain vector-only");
    }

    #[test]
    fn unknown_calligraphy_input_poisons_until_a_root_boundary_resets_the_session() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../conformance/calligraphy-saved-caller-native.json"
        ))
        .unwrap();
        let sources = fixture["cases"]["two-stroke-object-list-batch"]["input"]["sources"]
            .as_array()
            .unwrap();
        let first = calligraphy_stroke(&sources[0]);
        let second = calligraphy_stroke(&sources[1]);
        let mut unsupported = first.clone();
        unsupported.rendering.as_mut().unwrap().style.pen_size = Some(f32::NAN);

        let first_only = render_objects(vec![first.clone().into()], false);
        let poisoned = render_objects(
            vec![
                first.clone().into(),
                unsupported.clone().into(),
                second.clone().into(),
            ],
            false,
        );
        assert_eq!(
            poisoned.matches("<linearGradient").count(),
            first_only.matches("<linearGradient").count(),
            "unsupported matching input prevents a later stroke from reusing stale native state"
        );

        let reset = render_objects(
            vec![
                first.clone().into(),
                unsupported.clone().into(),
                PageElement::Image {
                    bbox: BoundingBox::default(),
                    media_index: usize::MAX,
                }
                .into(),
                second.clone().into(),
            ],
            false,
        );
        let mut expected_fresh_paths =
            path_data(&render_objects(vec![first.clone().into()], false));
        expected_fresh_paths.extend(path_data(&render_objects(vec![unsupported.into()], false)));
        expected_fresh_paths.extend(path_data(&render_objects(
            vec![second.clone().into()],
            false,
        )));
        assert_eq!(
            path_data(&reset),
            expected_fresh_paths,
            "the generic unsupported stroke remains visible, then a native type-3 boundary restores fresh Calligraphy geometry"
        );
    }

    #[test]
    fn replay_part_indices_follow_source_samples_and_terminal_sample() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../conformance/calligraphy-saved-wrapper-native.json"
        ))
        .unwrap();
        let case = fixture["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|case| case["id"] == "six-paired-optionals")
            .unwrap();
        let rows = case["rows"].as_array().unwrap();
        let points = rows
            .iter()
            .map(|row| [row[0].as_f64().unwrap(), row[1].as_f64().unwrap()])
            .collect::<Vec<_>>();
        let pressures = rows
            .iter()
            .map(|row| row[2].as_f64().unwrap())
            .collect::<Vec<_>>();
        let timestamps = rows
            .iter()
            .map(|row| row[3].as_i64().unwrap())
            .collect::<Vec<_>>();
        let argb = 0x7f11_2233;
        let stroke = Stroke {
            rendering: Some(StrokeRendering {
                metadata: None,
                pen_name: Some(
                    "com.samsung.android.sdk.pen.pen.preload.MontblancCalligraphyPen".into(),
                ),
                advanced_settings: None,
                tool_type_raw: 2,
                properties: StrokeProperties {
                    compressed: true,
                    replay_only: false,
                    stylus_channels: true,
                    eraser: false,
                    fixed_width: false,
                    millisecond_timestamps: false,
                    top_layer_pen: false,
                    alpha_lock: false,
                    binary_added: true,
                    generated: false,
                    fixed_opacity: false,
                    rainbow_effect: false,
                    straighten: false,
                    reveal_mode: false,
                },
                style: StrokeStyle {
                    color_argb: Some(argb),
                    pen_size: Some(6.0),
                    stroke_type_raw: Some(0),
                    color_type_raw: Some(0),
                    ..StrokeStyle::default()
                },
            }),
            bbox: BoundingBox::default(),
            points: points.iter().map(|[x, y]| Point { x: *x, y: *y }).collect(),
            pressures,
            timestamps,
            tilts: case["tilt"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_f64().unwrap())
                .collect(),
            orientations: case["orientation"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_f64().unwrap())
                .collect(),
            color: Some(Color {
                r: (argb >> 16) as u8,
                g: (argb >> 8) as u8,
                b: argb as u8,
            }),
            pen_width: 6.0,
        };
        let mut session = crate::ink::calligraphy::CalligraphySession::new();
        let crate::ink::calligraphy::CalligraphyAdmission::Mesh(mesh) =
            crate::ink::calligraphy::prepare_model_stroke(&mut session, &stroke)
        else {
            panic!("six-point Calligraphy fixture must produce a native mesh");
        };
        let mut expected_parts = std::collections::BTreeSet::new();
        let mut previous = [0usize; 3];
        for (sample, ends) in mesh.sample_ends.iter().enumerate() {
            if (0..3).any(|lane| ends[lane] > previous[lane]) {
                expected_parts.insert(sample + 2);
            }
            previous = *ends;
        }
        if (0..3).any(|lane| mesh.terminal_end[lane] > mesh.terminal_start[lane]) {
            expected_parts.insert(mesh.sample_ends.len() + 2);
        }
        let source_count = mesh.sample_ends.len() + 2;
        assert!(
            expected_parts
                .iter()
                .any(|part| { *part >= 2 && *part < source_count }),
            "fixture needs an emitted intermediate source sample: {expected_parts:?}"
        );
        let rendered = render_objects(vec![stroke.into()], true);
        let xml = roxmltree::Document::parse(&rendered).unwrap();
        let parts = xml
            .descendants()
            .filter(|node| node.has_tag_name("path"))
            .filter_map(|node| node.attribute("data-replay-part"))
            .filter_map(|part| part.parse::<usize>().ok())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            parts, expected_parts,
            "every emitted native range must carry its exact source sample boundary"
        );
        assert!(xml.descendants().any(|node| {
            node.has_tag_name("g") && node.attribute("data-replay-stroke") == Some("0")
        }));
    }

    #[test]
    fn native_paths_reject_unrepresentable_coordinates_without_drawing_a_prefix() {
        let mut bytes = 2_u32.to_le_bytes().to_vec();
        for (verb, x, y) in [(1_u8, 10_f64, 20_f64), (2, f64::MAX, 30.)] {
            bytes.push(verb);
            bytes.extend(x.to_le_bytes());
            bytes.extend(y.to_le_bytes());
        }
        assert!(matches!(
            super::native_svg_path(&bytes),
            Err(super::GeometryDiagnosticKind::UnrepresentablePath)
        ));
    }

    fn page_with_uncolored_stroke() -> Page {
        Page {
            uuid: "page".into(),
            width: 100,
            height: 100,
            content_bbox: BoundingBox::default(),
            background_color: None,
            template: None,
            background: Default::default(),
            objects: vec![
                Stroke {
                    rendering: None,
                    bbox: BoundingBox::default(),
                    points: vec![Point { x: 1.0, y: 1.0 }, Point { x: 9.0, y: 9.0 }],
                    pressures: Vec::new(),
                    timestamps: Vec::new(),
                    tilts: Vec::new(),
                    orientations: Vec::new(),
                    color: None,
                    pen_width: 2.0,
                }
                .into(),
            ],
        }
    }

    #[test]
    fn public_oblique_svg_uses_native_saved_mesh_color_and_replay_parts() {
        for selected_v1 in [false, true] {
            let stroke = oblique_saved_stroke(selected_v1);
            let sample_count = stroke.points.len();
            let mut page = page_with_uncolored_stroke();
            page.background_color = Some(Color {
                r: 37,
                g: 37,
                b: 37,
            });
            page.objects = vec![stroke.into()];
            let document = document(page);
            let normal = render_document_svg(&document, &RenderOptions::default())
                .remove(0)
                .svg;
            assert!(normal.contains("mix-blend-mode:lighten"));
            assert!(normal.contains("shape-rendering=\"crispEdges\""));
            let document_svg = roxmltree::Document::parse(&normal).unwrap();
            let opacity = document_svg
                .descendants()
                .find(|node| node.has_tag_name("g") && node.attribute("filter").is_some())
                .unwrap()
                .attribute("opacity")
                .unwrap()
                .parse::<f64>()
                .unwrap();
            assert!((opacity - 128.0 / 255.0).abs() < 1e-6);
            let matrix = document_svg
                .descendants()
                .find(|node| node.has_tag_name("feColorMatrix"))
                .unwrap()
                .attribute("values")
                .unwrap()
                .split_whitespace()
                .map(|value| value.parse::<f64>().unwrap())
                .collect::<Vec<_>>();
            assert_eq!(matrix.len(), 20);
            assert!((matrix[4] - 17.0 / 255.0).abs() < 1e-6);
            assert!((matrix[9] - 34.0 / 255.0).abs() < 1e-6);
            assert!((matrix[14] - 51.0 / 255.0).abs() < 1e-6);

            let layout = layout_document(&document);
            let replay = super::render_layout_page_replay_svg(
                &document,
                &layout,
                0,
                &RenderOptions::default(),
            )
            .unwrap()
            .svg;
            assert!(replay.contains("data-replay-stroke=\"0\""));
            let parts = roxmltree::Document::parse(&replay)
                .unwrap()
                .descendants()
                .filter_map(|node| node.attribute("data-replay-part"))
                .map(|value| value.parse::<usize>().unwrap())
                .collect::<Vec<_>>();
            assert!(!parts.is_empty());
            assert!(parts.iter().all(|part| (1..=sample_count).contains(part)));
            assert!(
                parts.contains(&sample_count),
                "terminal field is attributed to the final saved sample"
            );
        }
    }

    #[test]
    fn descendant_oblique_keeps_the_context_free_fallback() {
        for selected_v1 in [false, true] {
            let stroke = oblique_saved_stroke(selected_v1);
            let mut page = page_with_uncolored_stroke();
            page.objects = vec![crate::PageObject {
                render_layer: crate::ObjectRenderLayer::Base,
                source_offset: None,
                content: crate::PageObjectContent::Descendant(Box::new(
                    crate::PageObjectContent::Stroke(stroke),
                )),
            }];
            let svg = render_document_svg(&document(page), &RenderOptions::default())
                .remove(0)
                .svg;
            assert!(!svg.contains("mix-blend-mode:lighten"));
            assert!(!svg.contains("feColorMatrix"));
            assert!(!svg.contains("sdocx-native-ink"));
        }
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn public_oblique_pdf_lowers_the_saved_mesh_without_raster_images() {
        for selected_v1 in [false, true] {
            let mut page = page_with_uncolored_stroke();
            page.objects = vec![oblique_saved_stroke(selected_v1).into()];
            let bytes = crate::render_document_pdf(
                &document(page),
                &RenderOptions::default(),
                &crate::PdfOptions::default(),
            )
            .unwrap();
            let pdf = lopdf::Document::load_mem(&bytes).unwrap();
            let images = pdf
                .objects
                .values()
                .filter_map(pdf_object_dictionary)
                .filter(|dict| {
                    dict.get(b"Subtype")
                        .and_then(lopdf::Object::as_name)
                        .is_ok_and(|name| name == b"Image")
                })
                .count();
            assert_eq!(images, 0);
            let native_forms = pdf
                .objects
                .values()
                .filter_map(|object| object.as_stream().ok())
                .filter(|stream| {
                    stream
                        .dict
                        .get(b"Subtype")
                        .and_then(lopdf::Object::as_name)
                        .is_ok_and(|name| name == b"Form")
                })
                .filter(|stream| {
                    stream.decompressed_content().is_ok_and(|content| {
                        content.windows(4).any(|window| window == b" rg ")
                            && content.windows(2).any(|window| window == b" f")
                    })
                })
                .count();
            assert!(
                native_forms > 0,
                "native coverage must lower into a colored PDF Form"
            );
        }
    }

    fn inkpen_v1_stroke() -> Stroke {
        Stroke {
            rendering: Some(StrokeRendering {
                metadata: None,
                pen_name: Some("com.samsung.android.sdk.pen.pen.preload.InkPen".into()),
                advanced_settings: None,
                tool_type_raw: 1,
                properties: StrokeProperties {
                    compressed: true,
                    replay_only: false,
                    stylus_channels: false,
                    eraser: false,
                    fixed_width: false,
                    millisecond_timestamps: false,
                    top_layer_pen: false,
                    alpha_lock: false,
                    binary_added: false,
                    generated: false,
                    fixed_opacity: false,
                    rainbow_effect: false,
                    straighten: false,
                    reveal_mode: false,
                },
                style: StrokeStyle {
                    color_argb: Some(0x8033_6699),
                    pen_size: Some(10.0),
                    stroke_type_raw: Some(0),
                    color_type_raw: Some(0),
                    ..StrokeStyle::default()
                },
            }),
            bbox: BoundingBox::default(),
            points: vec![
                Point { x: 0.0, y: 0.0 },
                Point { x: 10.0, y: 0.0 },
                Point { x: 20.0, y: 0.0 },
                Point { x: 30.0, y: 10.0 },
            ],
            pressures: vec![0.25, 0.5, 0.75, 1.0],
            timestamps: vec![10, 20, 30, 40],
            tilts: vec![0.0; 4],
            orientations: vec![0.0; 4],
            color: None,
            pen_width: 10.0,
        }
    }

    fn magicpen_v1_stroke() -> Stroke {
        let mut stroke = inkpen_v1_stroke();
        let rendering = stroke.rendering.as_mut().unwrap();
        rendering.pen_name = Some("com.samsung.android.sdk.pen.pen.preload.MagicPen".into());
        rendering.advanced_settings = Some("1;".into());
        rendering.tool_type_raw = 2;
        rendering.properties.compressed = false;
        stroke.points.truncate(3);
        stroke.pressures.truncate(3);
        stroke.timestamps.truncate(3);
        stroke.tilts.truncate(3);
        stroke.orientations.truncate(3);
        stroke
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn dump_magicpen_public_candidate_and_generic_fallback() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../conformance/magicpen-nocurve-source-events-native.json"
        ))
        .unwrap();
        let case = fixture["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|case| case["id"] == "curve_off")
            .unwrap();
        let input = &case["input"];
        let f32_word = |value: &serde_json::Value| {
            f32::from_bits(u32::from_str_radix(value.as_str().unwrap(), 16).unwrap())
        };
        let words = input["points_f32_u32_hex"].as_array().unwrap();
        let points = words
            .as_chunks::<2>()
            .0
            .iter()
            .map(|xy| Point {
                x: f64::from(f32_word(&xy[0])),
                y: f64::from(f32_word(&xy[1])),
            })
            .collect::<Vec<_>>();
        let pressures = input["pressure_f32_u32_hex"]
            .as_array()
            .unwrap()
            .iter()
            .map(|word| f64::from(f32_word(word)))
            .collect::<Vec<_>>();
        let size = f64::from(f32_word(&input["size_f32_u32_hex"]));
        let mut stroke = inkpen_v1_stroke();
        stroke.rendering.as_mut().unwrap().pen_name =
            Some("com.samsung.android.sdk.pen.pen.preload.MagicPen".into());
        stroke.rendering.as_mut().unwrap().advanced_settings = Some("1;".into());
        stroke.rendering.as_mut().unwrap().tool_type_raw = 2;
        stroke.rendering.as_mut().unwrap().properties.compressed = false;
        stroke.rendering.as_mut().unwrap().style.color_argb = Some(0x8033_6699);
        stroke.points = points;
        stroke.pressures = pressures;
        stroke.timestamps = input["timestamps_i32"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_i64().unwrap())
            .collect();
        stroke.tilts = vec![0.0; stroke.points.len()];
        stroke.orientations = vec![0.0; stroke.points.len()];
        stroke.pen_width = size as f32;
        let prepared = crate::ink::prepare_magicpen_v1_source(&stroke).unwrap();
        let native = case["native"]["xyz_f32_u32_hex"]
            .as_array()
            .unwrap()
            .iter()
            .map(|word| u32::from_str_radix(word.as_str().unwrap(), 16).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            prepared
                .records
                .iter()
                .flatten()
                .copied()
                .collect::<Vec<_>>(),
            native
        );

        let mut page = page_with_uncolored_stroke();
        page.width = 256;
        page.height = 256;
        page.objects = vec![stroke.clone().into()];
        let candidate = document(page.clone());
        let candidate_svg = render_document_svg(&candidate, &RenderOptions::default())[0]
            .svg
            .clone();
        let candidate_pdf = crate::render_document_pdf(
            &candidate,
            &RenderOptions::default(),
            &crate::PdfOptions::default(),
        )
        .unwrap();

        let mut fallback_object = crate::PageObject::from(stroke.clone());
        fallback_object.content = crate::PageObjectContent::Descendant(Box::new(
            crate::PageObjectContent::Stroke(stroke),
        ));
        page.objects = vec![fallback_object];
        let blank_page_seed = page.clone();
        let fallback = document(page);
        let fallback_svg = render_document_svg(&fallback, &RenderOptions::default())[0]
            .svg
            .clone();
        let fallback_pdf = crate::render_document_pdf(
            &fallback,
            &RenderOptions::default(),
            &crate::PdfOptions::default(),
        )
        .unwrap();

        let mut blank_page = blank_page_seed;
        blank_page.objects.clear();
        let blank = document(blank_page);
        let blank_svg = render_document_svg(&blank, &RenderOptions::default())[0]
            .svg
            .clone();
        let blank_pdf = crate::render_document_pdf(
            &blank,
            &RenderOptions::default(),
            &crate::PdfOptions::default(),
        )
        .unwrap();

        if let Some(dir) = std::env::var_os("MAGICPEN_PUBLIC_DUMP_DIR") {
            let dir = std::path::Path::new(&dir);
            std::fs::create_dir_all(dir).unwrap();
            std::fs::write(dir.join("candidate.svg"), candidate_svg).unwrap();
            std::fs::write(dir.join("candidate.pdf"), candidate_pdf).unwrap();
            std::fs::write(dir.join("fallback.svg"), fallback_svg).unwrap();
            std::fs::write(dir.join("fallback.pdf"), fallback_pdf).unwrap();
            std::fs::write(dir.join("blank.svg"), blank_svg).unwrap();
            std::fs::write(dir.join("blank.pdf"), blank_pdf).unwrap();
        }
    }

    #[test]
    fn canonical_magicpen_v1_page_export_uses_source_contours_and_event_prefixes() {
        let stroke = magicpen_v1_stroke();
        let prepared = crate::prepare_stroke_for_page(&stroke, false);
        assert_eq!(prepared.profile, Some("MagicPen"));
        assert_eq!(prepared.support, crate::InkSupport::Reconstructed);
        assert_eq!(prepared.sample_ends.as_deref(), Some(&[0, 6, 12][..]));
        assert_eq!(prepared.opacity, 128.0 / 255.0);
        assert_eq!(prepared.color, "#336699");

        let mut page = page_with_uncolored_stroke();
        page.objects = vec![stroke.into()];
        let source = document(page);
        let normal = render_document_svg(&source, &RenderOptions::default())[0]
            .svg
            .clone();
        let replay = super::render_layout_page_replay_svg(
            &source,
            &layout_document(&source),
            0,
            &RenderOptions::default(),
        )
        .unwrap()
        .svg;
        for (svg, has_replay) in [(&normal, false), (&replay, true)] {
            let xml = roxmltree::Document::parse(svg).unwrap();
            let paths = xml
                .descendants()
                .filter(|node| node.has_tag_name("path"))
                .collect::<Vec<_>>();
            assert_eq!(paths.len(), 1);
            let path = paths[0];
            assert_eq!(path.attribute("fill"), Some("#336699"));
            assert_eq!(path.attribute("d").unwrap().matches('M').count(), 12);
            assert_eq!(path.attribute("d").unwrap().matches('Z').count(), 12);
            if has_replay {
                let lengths = path.attribute("data-replay-lengths").unwrap();
                assert_eq!(lengths.split(',').count(), 12);
                let values = lengths
                    .split(',')
                    .map(|value| value.parse::<usize>().unwrap())
                    .collect::<Vec<_>>();
                let stroke = magicpen_v1_stroke();
                let prepared = crate::prepare_stroke_for_page(&stroke, false);
                for count in prepared.sample_ends.unwrap() {
                    // Match the web ReplaySvg cutoff contract, including the
                    // empty initial event and a six-stamp intermediate event.
                    let end = if count >= values.len() {
                        path.attribute("d").unwrap().len()
                    } else {
                        count.checked_sub(1).map_or(0, |index| values[index])
                    };
                    let prefix = &path.attribute("d").unwrap()[..end];
                    assert_eq!(prefix.matches('M').count(), count);
                    assert!(prefix.is_empty() || prefix.ends_with('Z'));
                }
            } else {
                assert!(path.attribute("data-replay-lengths").is_none());
            }
            let opacity = xml
                .descendants()
                .find(|node| node.has_tag_name("g") && node.attribute("opacity").is_some())
                .and_then(|node| node.attribute("opacity"))
                .unwrap()
                .parse::<f64>()
                .unwrap();
            assert!((opacity - 128.0 / 255.0).abs() < 1e-6);
        }
        assert!(replay.contains(r#"data-replay-stroke="0""#));

        let mut fallback = magicpen_v1_stroke();
        fallback.rendering.as_mut().unwrap().advanced_settings = Some("2;".into());
        assert_eq!(
            crate::prepare_stroke_for_page(&fallback, false).support,
            crate::InkSupport::Approximate
        );
        let mut page = page_with_uncolored_stroke();
        page.objects = vec![fallback.into()];
        let fallback_svg = render_document_svg(&document(page), &RenderOptions::default())[0]
            .svg
            .clone();
        assert!(!fallback_svg.contains("data-replay-lengths"));
        assert!(!fallback_svg.contains(r##"fill="#336699""##));

        let mut budget = magicpen_v1_stroke();
        budget.rendering.as_mut().unwrap().properties.compressed = false;
        let count = 2_000;
        budget.points = (0..count)
            .map(|index| Point {
                x: index as f64 * 2.0,
                y: 0.0,
            })
            .collect();
        budget.pressures = vec![1.0; count];
        budget.timestamps = (0..count as i64).collect();
        budget.tilts = vec![0.0; count];
        budget.orientations = vec![0.0; count];
        let ink = crate::ink::prepare_magicpen_v1_source(&budget).unwrap();
        assert!(super::magicpen_v1_vector_path(&ink, false).is_none());
        let prepared = crate::prepare_stroke_for_page(&budget, false);
        assert_eq!(prepared.support, crate::InkSupport::Approximate);
        assert!(prepared.sample_ends.is_none());
    }

    #[test]
    fn magicpen_v1_curved_public_replay_matches_native_records_and_sample_cutoffs() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../conformance/magicpen-curved-source-events-native.json"
        ))
        .unwrap();
        let word =
            |value: &serde_json::Value| u32::from_str_radix(value.as_str().unwrap(), 16).unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let id = case["id"].as_str().unwrap();
            let input = &case["input"];
            let mut stroke = magicpen_v1_stroke();
            stroke.points = input["points_f32_u32_hex"]
                .as_array()
                .unwrap()
                .as_chunks::<2>()
                .0
                .iter()
                .map(|xy| Point {
                    x: f32::from_bits(word(&xy[0])) as f64,
                    y: f32::from_bits(word(&xy[1])) as f64,
                })
                .collect();
            stroke.pressures = input["pressure_f32_u32_hex"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| f32::from_bits(word(value)) as f64)
                .collect();
            stroke.timestamps = input["timestamps_i32"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_i64().unwrap())
                .collect();
            stroke.tilts = vec![0.0; stroke.points.len()];
            stroke.orientations = vec![0.0; stroke.points.len()];
            stroke.pen_width = f32::from_bits(word(&input["size_f32_u32_hex"]));
            let rendering = stroke.rendering.as_mut().unwrap();
            rendering.properties.compressed = true;
            rendering.style.pen_size = Some(stroke.pen_width);
            let ink = crate::ink::prepare_magicpen_v1_source(&stroke)
                .unwrap_or_else(|| panic!("{id} rejected"));
            let actual: Vec<u32> = ink.records.iter().flatten().copied().collect();
            let expected: Vec<u32> = case["native"]["xyz_f32_u32_hex"]
                .as_array()
                .unwrap()
                .iter()
                .map(word)
                .collect();
            assert_eq!(actual, expected, "{id} native records");
            let count = ink.records.len();
            let mut ends = vec![0];
            ends.extend(
                case["capture"]["native_addpoint"]["source_event_trace"]["curved_line_calls"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|event| event["addpoint_range"][1].as_u64().unwrap() as usize),
            );
            *ends.last_mut().unwrap() = count;
            let prepared = crate::prepare_stroke_for_page(&stroke, false);
            assert_eq!(prepared.support, crate::InkSupport::Reconstructed, "{id}");
            assert_eq!(prepared.sample_ends.as_ref(), Some(&ends), "{id} cutoffs");
            let mut page = page_with_uncolored_stroke();
            page.objects = vec![stroke.clone().into()];
            let document = document(page);
            let normal = render_document_svg(&document, &RenderOptions::default()).remove(0);
            let replay = super::render_layout_page_replay_svg(
                &document,
                &layout_document(&document),
                0,
                &RenderOptions::default(),
            )
            .unwrap();
            let xml = roxmltree::Document::parse(&replay.svg).unwrap();
            let path = xml
                .descendants()
                .find(|node| node.has_tag_name("path"))
                .unwrap();
            let data = path.attribute("d").unwrap();
            let lengths: Vec<usize> = path
                .attribute("data-replay-lengths")
                .unwrap()
                .split(',')
                .map(|value| value.parse().unwrap())
                .collect();
            assert_eq!(lengths.len(), count, "{id} prepared-record prefixes");
            for end in ends {
                let bytes = end.checked_sub(1).map_or(0, |index| lengths[index]);
                assert_eq!(
                    data[..bytes].matches('M').count(),
                    end,
                    "{id} replay cutoff"
                );
            }
            let normal_xml = roxmltree::Document::parse(&normal.svg).unwrap();
            let normal_path = normal_xml
                .descendants()
                .find(|node| node.has_tag_name("path"))
                .unwrap();
            assert_eq!(
                normal_path.attribute("d"),
                Some(data),
                "{id} stable geometry"
            );
            assert!(!normal.svg.contains("<image"));
            if id == "tool2_baseline" {
                #[cfg(feature = "pdf")]
                {
                    let bytes =
                        crate::render_svg_pages_pdf(&[normal], &Default::default()).unwrap();
                    let pdf = lopdf::Document::load_mem(&bytes).unwrap();
                    assert!(!pdf.objects.values().any(|object| {
                        object.as_stream().ok().is_some_and(|stream| {
                            stream
                                .dict
                                .get(b"Subtype")
                                .ok()
                                .and_then(|value| value.as_name().ok())
                                == Some(b"Image")
                        })
                    }));
                }
            }
        }
    }

    #[test]
    fn magicpen_v1_native_zero_pressure_dot_retains_vector_and_zoom_coverage() {
        // Native transparent 1x witness: center (8.25,8.25), stored radius
        // 0.5, one visible pixel RGBA [4,8,12,20]. The old half-coverage
        // contour collapsed this native stamp to a zero-area path.
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../conformance/magicpen-nocurve-edge-paint-native.json"
        ))
        .unwrap();
        let witness = &fixture["captures"]["duplicate_all_points_zero_pressure_alpha255-transparent-1x"]
            ["paint"];
        assert_eq!(witness["native_vector_hex"], "00000441000004410000003f");
        assert_eq!(witness["readback"]["changed_from_background_pixels"], 1);
        assert_eq!(
            witness["source_inputs"]["points"],
            serde_json::json!([[8.25, 8.25], [8.25, 8.25], [8.25, 8.25]])
        );
        assert_eq!(
            witness["source_inputs"]["pressure"],
            serde_json::json!([0, 0, 0])
        );
        assert_eq!(witness["source_inputs"]["size"], 0.4);
        assert_eq!(witness["source_inputs"]["argb"], "0xff336699");
        let mut stroke = magicpen_v1_stroke();
        stroke.points = vec![Point { x: 8.25, y: 8.25 }; 3];
        stroke.pressures = vec![0.0; 3];
        stroke.pen_width = 0.4;
        stroke.rendering.as_mut().unwrap().style.pen_size = Some(0.4);
        stroke.rendering.as_mut().unwrap().style.color_argb = Some(0xff33_6699);
        let ink = crate::ink::prepare_magicpen_v1_source(&stroke).unwrap();
        assert_eq!(
            ink.records,
            vec![[8.25_f32.to_bits(), 8.25_f32.to_bits(), 0.5_f32.to_bits()]]
        );
        assert_eq!(ink.sample_ends, vec![0, 0, 0]);
        assert_eq!(ink.terminal_range, 0..1);
        let raster = |stroke: Option<Stroke>, scale: u32| {
            let mut page = page_with_uncolored_stroke();
            page.objects.clear();
            if let Some(stroke) = stroke {
                page.objects.push(stroke.into());
            }
            let rendered =
                render_document_svg(&document(page), &RenderOptions::default()).remove(0);
            assert!(!rendered.svg.contains("<image"));
            let tree = resvg::usvg::Tree::from_str(&rendered.svg, &Default::default()).unwrap();
            let mut pixels =
                resvg::tiny_skia::Pixmap::new(rendered.width * scale, rendered.height * scale)
                    .unwrap();
            resvg::render(
                &tree,
                resvg::tiny_skia::Transform::from_scale(scale as f32, scale as f32),
                &mut pixels.as_mut(),
            );
            pixels.data().to_vec()
        };
        // resvg still drops this subpixel phase at 1x. Check the unchanged
        // exported contour at 2x/4x; the old zero-area contour stays blank.
        for scale in [2, 4] {
            let blank = raster(None, scale);
            assert!(raster(Some(stroke.clone()), scale) != blank);
            let mut transparent = stroke.clone();
            transparent.rendering.as_mut().unwrap().style.color_argb = Some(0x0033_6699);
            assert!(raster(Some(transparent), scale) == blank);
        }
    }

    #[test]
    fn magicpen_v1_public_exports_use_native_thick_stroke_repeat_counts() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../conformance/magicpen-nocurve-source-events-native.json"
        ))
        .unwrap();
        let float = |word: &serde_json::Value| {
            f32::from_bits(u32::from_str_radix(word.as_str().unwrap(), 16).unwrap())
        };
        for (id, count) in [
            ("repeat_bucket_width_50_len24", 8),
            ("repeat_bucket_width_80_len24", 6),
        ] {
            let case = fixture["cases"]
                .as_array()
                .unwrap()
                .iter()
                .find(|case| case["id"] == id)
                .unwrap();
            assert_eq!(case["native"]["record_count"], count);
            let input = &case["input"];
            let mut stroke = magicpen_v1_stroke();
            stroke.points = input["points_f32_u32_hex"]
                .as_array()
                .unwrap()
                .as_chunks::<2>()
                .0
                .iter()
                .map(|xy| Point {
                    x: f64::from(float(&xy[0])),
                    y: f64::from(float(&xy[1])),
                })
                .collect();
            stroke.pressures = input["pressure_f32_u32_hex"]
                .as_array()
                .unwrap()
                .iter()
                .map(|word| f64::from(float(word)))
                .collect();
            stroke.rendering.as_mut().unwrap().style.pen_size =
                Some(float(&input["size_f32_u32_hex"]));
            let prepared = crate::prepare_stroke_for_page(&stroke, false);
            assert_eq!(prepared.support, crate::InkSupport::Reconstructed, "{id}");
            assert_eq!(
                prepared.sample_ends.as_deref(),
                Some(&[0, count / 2, count][..]),
                "{id}"
            );
            let mut page = page_with_uncolored_stroke();
            page.objects = vec![stroke.into()];
            let svg = render_document_svg(&document(page), &RenderOptions::default()).remove(0);
            let xml = roxmltree::Document::parse(&svg.svg).unwrap();
            let path = xml
                .descendants()
                .find(|node| node.has_tag_name("path"))
                .unwrap();
            assert_eq!(
                path.attribute("d").unwrap().matches('M').count(),
                count,
                "{id}"
            );
        }
    }

    #[test]
    fn magicpen_v1_omitted_native_style_defaults_preserve_public_exports() {
        let exports = |stroke: Stroke| {
            let prepared = crate::prepare_stroke_for_page(&stroke, false);
            assert_eq!(prepared.support, crate::InkSupport::Reconstructed);
            assert_eq!(prepared.sample_ends.as_deref(), Some(&[0, 6, 12][..]));
            let mut page = page_with_uncolored_stroke();
            page.objects = vec![stroke.into()];
            let source = document(page);
            let normal = render_document_svg(&source, &RenderOptions::default()).remove(0);
            let replay = super::render_layout_page_replay_svg(
                &source,
                &layout_document(&source),
                0,
                &RenderOptions::default(),
            )
            .unwrap();
            (normal, replay)
        };
        let expected = exports(magicpen_v1_stroke());
        for (stroke_type, color_type) in [(None, Some(0)), (Some(0), None), (None, None)] {
            let mut stroke = magicpen_v1_stroke();
            let style = &mut stroke.rendering.as_mut().unwrap().style;
            style.stroke_type_raw = stroke_type;
            style.color_type_raw = color_type;
            crate::prepare_stroke_for_page(&stroke, false);
            let style = &stroke.rendering.as_ref().unwrap().style;
            assert_eq!(style.stroke_type_raw, stroke_type);
            assert_eq!(style.color_type_raw, color_type);
            let actual = exports(stroke);
            assert_eq!(actual.0.svg, expected.0.svg);
            assert_eq!(actual.1.svg, expected.1.svg);
            #[cfg(feature = "pdf")]
            for (actual, expected) in [(&actual.0, &expected.0), (&actual.1, &expected.1)] {
                assert_eq!(
                    crate::render_svg_pages_pdf(std::slice::from_ref(actual), &Default::default())
                        .unwrap(),
                    crate::render_svg_pages_pdf(
                        std::slice::from_ref(expected),
                        &Default::default()
                    )
                    .unwrap(),
                );
            }
        }
    }

    #[test]
    fn magicpen_v1_omitted_settings_and_color_match_native_defaults() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../conformance/magicpen-admission-defaults-native.json"
        ))
        .unwrap();
        let float = |word: &serde_json::Value| {
            f32::from_bits(u32::from_str_radix(word.as_str().unwrap(), 16).unwrap())
        };
        let mut source = magicpen_v1_stroke();
        source.points = fixture["source_input"]["points_f32_u32_hex"]
            .as_array()
            .unwrap()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|xy| Point {
                x: float(&xy[0]) as f64,
                y: float(&xy[1]) as f64,
            })
            .collect();
        source.pressures = fixture["source_input"]["pressure_f32_u32_hex"]
            .as_array()
            .unwrap()
            .iter()
            .map(|word| float(word) as f64)
            .collect();
        source.rendering.as_mut().unwrap().style.color_argb = Some(0xff00_0000);
        let exports = |stroke: Stroke| {
            let ink = crate::ink::prepare_magicpen_v1_source(&stroke).unwrap();
            let native_hex = fixture["cases"][0]["post_return_vector_bytes_le_hex"]
                .as_str()
                .unwrap();
            let bytes: Vec<u8> = ink
                .records
                .iter()
                .flatten()
                .flat_map(|word| word.to_le_bytes())
                .collect();
            let actual_hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
            assert_eq!(actual_hex, native_hex);
            assert_eq!(ink.argb, 0xff00_0000);
            let mut page = page_with_uncolored_stroke();
            page.objects = vec![stroke.into()];
            let document = document(page);
            let normal = render_document_svg(&document, &RenderOptions::default()).remove(0);
            let replay = super::render_layout_page_replay_svg(
                &document,
                &layout_document(&document),
                0,
                &RenderOptions::default(),
            )
            .unwrap();
            [normal, replay]
        };
        let expected = exports(source.clone());
        for (advanced, argb) in [(None, Some(0xff00_0000)), (Some("1;"), None), (None, None)] {
            let mut stroke = source.clone();
            let rendering = stroke.rendering.as_mut().unwrap();
            rendering.advanced_settings = advanced.map(str::to_owned);
            rendering.style.color_argb = argb;
            let actual = exports(stroke.clone());
            assert_eq!(
                stroke
                    .rendering
                    .as_ref()
                    .unwrap()
                    .advanced_settings
                    .as_deref(),
                advanced
            );
            assert_eq!(stroke.rendering.as_ref().unwrap().style.color_argb, argb);
            for (actual, expected) in actual.iter().zip(&expected) {
                assert_eq!(actual.svg, expected.svg);
                #[cfg(feature = "pdf")]
                assert_eq!(
                    crate::render_svg_pages_pdf(std::slice::from_ref(actual), &Default::default())
                        .unwrap(),
                    crate::render_svg_pages_pdf(
                        std::slice::from_ref(expected),
                        &Default::default()
                    )
                    .unwrap(),
                );
            }
        }
        for advanced in ["", "2;", "unknown"] {
            let mut stroke = source.clone();
            stroke.rendering.as_mut().unwrap().advanced_settings = Some(advanced.into());
            assert!(crate::ink::prepare_magicpen_v1_source(&stroke).is_none());
        }
    }

    #[test]
    fn magicpen_v1_unknown_saved_effects_stay_on_generic_fallback() {
        let mutations: [fn(&mut crate::StrokeRendering); 11] = [
            |r| r.properties.replay_only = true,
            |r| r.properties.eraser = true,
            |r| r.properties.fixed_width = true,
            |r| r.properties.top_layer_pen = true,
            |r| r.properties.alpha_lock = true,
            |r| r.properties.fixed_opacity = true,
            |r| r.properties.rainbow_effect = true,
            |r| r.properties.straighten = true,
            |r| r.properties.reveal_mode = true,
            |r| r.style.stroke_type_raw = Some(1),
            |r| r.style.color_type_raw = Some(1),
        ];
        for mutate in mutations {
            let mut stroke = magicpen_v1_stroke();
            mutate(stroke.rendering.as_mut().unwrap());
            let prepared = crate::prepare_stroke_for_page(&stroke, false);
            assert_eq!(prepared.support, crate::InkSupport::Approximate);
            assert!(prepared.sample_ends.is_none());

            let mut page = page_with_uncolored_stroke();
            page.objects = vec![stroke.into()];
            let svg = render_document_svg(&document(page), &RenderOptions::default())[0]
                .svg
                .clone();
            assert!(!svg.contains("data-replay-lengths"));
            assert!(!svg.contains(r##"fill="#336699""##));
        }
    }

    #[test]
    fn inkpen_v4_serialized_setting_fallback_preserves_saved_paint() {
        let mut stroke = inkpen_v1_stroke();
        stroke.rendering.as_mut().unwrap().advanced_settings = Some("18;0;100;".into());
        let theme = RenderTheme::for_canvas(false);
        let valid = RenderOptions {
            inkpen_profile: Some(super::InkPenRenderProfile {
                screen_width: 1920,
                screen_height: 1080,
            }),
            ..RenderOptions::default()
        };
        let invalid = RenderOptions {
            inkpen_profile: Some(super::InkPenRenderProfile {
                screen_width: 0,
                screen_height: 1080,
            }),
            ..RenderOptions::default()
        };
        let missing = RenderOptions::default();
        for (options, viewport) in [
            (&missing, [100, 100]),
            (&invalid, [100, 100]),
            (&valid, [0, 100]),
        ] {
            let paint =
                super::prepare_stroke_for_viewport_with_theme(&stroke, theme, options, viewport);
            assert_eq!(paint.color, "#336699");
            assert_eq!(paint.opacity, 128.0 / 255.0);
            assert_eq!(paint.support, crate::InkSupport::Approximate);
        }

        stroke.rendering.as_mut().unwrap().style.color_argb = None;
        for (options, viewport) in [
            (&missing, [100, 100]),
            (&invalid, [100, 100]),
            (&valid, [0, 100]),
        ] {
            let paint =
                super::prepare_stroke_for_viewport_with_theme(&stroke, theme, options, viewport);
            assert_eq!(paint.color, "#000000");
            assert_eq!(paint.opacity, 1.0);
            assert_eq!(paint.support, crate::InkSupport::Approximate);
        }
    }

    #[test]
    fn inkpen_v4_opt_in_matches_replay_and_preserves_fallback() {
        let mut stroke = inkpen_v1_stroke();
        stroke.rendering.as_mut().unwrap().advanced_settings = Some("4;".into());
        let profile = super::InkPenRenderProfile {
            screen_width: 1920,
            screen_height: 1080,
        };
        let options = RenderOptions {
            inkpen_profile: Some(profile),
            ..RenderOptions::default()
        };
        assert!(super::prepare_page_inkpen(&stroke, None).is_none());
        let ink = super::prepare_page_inkpen(&stroke, Some(profile)).unwrap();
        let direct = crate::ink::prepare_inkpen_v4_for_page(&stroke, 1920, 1080).unwrap();
        assert_eq!(ink.points, direct.points);
        assert_eq!(ink.sample_ends, direct.sample_ends);
        let theme = RenderTheme::for_canvas(false);
        let replay_paint =
            super::prepare_stroke_for_viewport_with_theme(&stroke, theme, &options, [100, 100]);
        let vector = super::inkpen_v1_vector_path(
            &ink.points,
            &ink.radii,
            &ink.sample_ends,
            ink.terminal,
            true,
        )
        .unwrap();
        assert_eq!(
            replay_paint.sample_ends.as_deref(),
            Some(vector.sample_ends.as_slice())
        );
        assert_eq!(replay_paint.color, "#336699");
        assert_eq!(replay_paint.opacity, 128.0 / 255.0);
        let zero =
            super::prepare_stroke_for_viewport_with_theme(&stroke, theme, &options, [0, 100]);
        let fallback = crate::ink::prepare_stroke_for_context(
            &stroke,
            false,
            crate::ink::InkRenderContext::CanonicalPage,
        );
        assert_eq!(zero.points, fallback.points);
        assert_eq!(zero.profile, fallback.profile);
        let mut page = page_with_uncolored_stroke();
        page.objects = vec![stroke.clone().into()];
        let source = document(page);
        let baseline = render_document_svg(&source, &RenderOptions::default())[0]
            .svg
            .clone();
        let normal = render_document_svg(&source, &options)[0].svg.clone();
        assert_ne!(normal, baseline);
        assert!(normal.contains(r##"fill="#336699""##));
        let replay =
            super::render_layout_page_replay_svg(&source, &layout_document(&source), 0, &options)
                .unwrap()
                .svg;
        assert!(replay.contains("data-replay-lengths"));
        assert!(replay.contains(r##"fill="#336699""##));
        stroke.rendering.as_mut().unwrap().properties.fixed_width = true;
        assert!(super::prepare_page_inkpen(&stroke, Some(profile)).is_none());
        let mut unsupported = source.clone();
        unsupported.pages[0].objects = vec![stroke.into()];
        assert_eq!(
            render_document_svg(&unsupported, &options)[0].svg,
            render_document_svg(&unsupported, &RenderOptions::default())[0].svg
        );
        assert!(
            super::prepare_page_inkpen(
                &inkpen_v1_stroke(),
                Some(super::InkPenRenderProfile {
                    screen_width: 0,
                    ..profile
                })
            )
            .is_some()
        );
    }

    #[cfg(feature = "serde")]
    #[test]
    fn inkpen_profile_missing_from_legacy_options_defaults_to_none() {
        let mut legacy = serde_json::to_value(RenderOptions::default()).unwrap();
        legacy.as_object_mut().unwrap().remove("inkpen_profile");
        let options: RenderOptions = serde_json::from_value(legacy).unwrap();
        assert!(options.inkpen_profile.is_none());
        let profiled = RenderOptions {
            inkpen_profile: Some(super::InkPenRenderProfile {
                screen_width: 1920,
                screen_height: 1080,
            }),
            ..RenderOptions::default()
        };
        let roundtrip: RenderOptions =
            serde_json::from_value(serde_json::to_value(profiled).unwrap()).unwrap();
        assert_eq!(roundtrip.inkpen_profile.unwrap().screen_width, 1920);
        assert_eq!(roundtrip.inkpen_profile.unwrap().screen_height, 1080);
    }

    #[test]
    fn canonical_v1_page_export_uses_saved_source_geometry_alpha_and_replay_prefixes() {
        let stroke = inkpen_v1_stroke();
        let prepared = crate::prepare_stroke_for_page(&stroke, false);
        assert_eq!(prepared.sample_ends.as_deref(), Some(&[0, 0, 0, 25][..]));
        assert_eq!(prepared.opacity, 128.0 / 255.0);
        assert_eq!(prepared.color, "#336699");

        let mut page = page_with_uncolored_stroke();
        page.objects = vec![stroke.into()];
        let source = document(page);
        let normal = render_document_svg(&source, &RenderOptions::default())[0]
            .svg
            .clone();
        let replay = super::render_layout_page_replay_svg(
            &source,
            &layout_document(&source),
            0,
            &RenderOptions::default(),
        )
        .unwrap()
        .svg;
        for (svg, has_replay) in [(&normal, false), (&replay, true)] {
            let xml = roxmltree::Document::parse(svg).unwrap();
            let paths = xml
                .descendants()
                .filter(|node| node.has_tag_name("path"))
                .collect::<Vec<_>>();
            assert_eq!(paths.len(), 1);
            let path = paths[0];
            assert_eq!(path.attribute("fill"), Some("#336699"));
            if has_replay {
                assert_eq!(
                    path.attribute("data-replay-lengths")
                        .unwrap()
                        .split(',')
                        .count(),
                    25
                );
            } else {
                assert!(path.attribute("data-replay-lengths").is_none());
            }
            assert_eq!(path.attribute("d").unwrap().matches('Z').count(), 25);
            let opacity = xml
                .descendants()
                .find(|node| node.has_tag_name("g") && node.attribute("opacity").is_some())
                .and_then(|node| node.attribute("opacity"))
                .unwrap()
                .parse::<f64>()
                .unwrap();
            assert!((opacity - 128.0 / 255.0).abs() < 1e-6);
        }
        assert!(replay.contains("data-replay-stroke=\"0\""));
        assert!(normal.contains("<path"));
        assert!(!normal.contains("<circle"));

        let mut explicit_color = inkpen_v1_stroke();
        explicit_color.color = Some(Color {
            r: 0xaa,
            g: 0xbb,
            b: 0xcc,
        });
        let explicit = crate::prepare_stroke_for_page(&explicit_color, false);
        assert_eq!(explicit.color, "#aabbcc");
        assert_eq!(explicit.opacity, 128.0 / 255.0);

        let mut non_null_setting = inkpen_v1_stroke();
        non_null_setting
            .rendering
            .as_mut()
            .unwrap()
            .advanced_settings = Some("4;".into());
        let fallback = crate::prepare_stroke_for_page(&non_null_setting, false);
        assert_eq!(fallback.support, crate::InkSupport::Approximate);
        assert!(fallback.sample_ends.is_none());
    }

    #[test]
    fn v1_path_budget_falls_back_consistently_in_metadata_normal_and_replay() {
        let mut stroke = inkpen_v1_stroke();
        let sample_count = 4_000;
        stroke.points = (0..sample_count)
            .map(|i| Point {
                x: i as f64,
                y: 0.0,
            })
            .collect();
        stroke.pressures = vec![1.0; sample_count];
        stroke.timestamps = (0..sample_count as i64).collect();
        stroke.tilts = vec![0.0; sample_count];
        stroke.orientations = vec![0.0; sample_count];

        let ink = crate::ink::prepare_inkpen_v1_for_page(&stroke).unwrap();
        assert!(
            super::inkpen_v1_vector_path(
                &ink.points,
                &ink.radii,
                &ink.sample_ends,
                ink.terminal,
                true,
            )
            .is_none()
        );

        let prepared = crate::prepare_stroke_for_page(&stroke, false);
        assert_ne!(prepared.support, crate::InkSupport::Reconstructed);
        assert!(prepared.sample_ends.is_none());

        let mut page = page_with_uncolored_stroke();
        page.objects = vec![stroke.into()];
        let source = document(page);
        let normal = render_document_svg(&source, &RenderOptions::default())[0]
            .svg
            .clone();
        let replay = super::render_layout_page_replay_svg(
            &source,
            &layout_document(&source),
            0,
            &RenderOptions::default(),
        )
        .unwrap()
        .svg;
        assert!(!normal.contains("data-replay-lengths"));
        assert!(!replay.contains("data-replay-lengths"));
        assert!(replay.contains("data-replay-stroke=\"0\""));
    }

    #[test]
    fn svg_preview_reports_retained_pdf_paper_background_omission() {
        let mut page = page_with_uncolored_stroke();
        page.background.pdf_paper = Some(vec![crate::PdfPaperRecord {
            media_id: -1000,
            page_index: 0,
            rectangle: crate::PdfPaperRectangle::Unspecified([0; 16]),
        }]);
        let imported_document = document(page);
        let rendered = render_document_svg(&imported_document, &RenderOptions::default()).remove(0);
        assert_eq!(
            rendered.background_diagnostics,
            [super::BackgroundDiagnostic::PdfPaperOmitted { record_count: 1 }]
        );

        let mut page = page_with_uncolored_stroke();
        page.background.pdf_paper = Some(Vec::new());
        let document = document(page);
        assert!(
            render_document_svg(&document, &RenderOptions::default())[0]
                .background_diagnostics
                .is_empty()
        );
    }

    fn document(page: Page) -> Document {
        Document {
            pages: vec![page],
            metadata: DocumentMetadata {
                default_page_dimensions: Some((1080, 1527)),
                orientation: Some(0),
                ..DocumentMetadata::default()
            },
        }
    }

    #[test]
    fn public_svg_projects_type8_gradient_and_keeps_rejected_line_body() {
        let mut page = page_with_uncolored_stroke();
        page.width = 640;
        page.height = 640;
        page.objects = vec![
            crate::PageElement::Line(super::gradient::tests::hf02_line_with_gradient()).into(),
        ];
        let rendered = render_document_svg(&document(page), &RenderOptions::default()).remove(0);
        assert!(rendered.paint_diagnostics.is_empty());
        let xml = roxmltree::Document::parse(&rendered.svg).unwrap();
        let gradients = xml
            .descendants()
            .filter(|node| node.has_tag_name("linearGradient"))
            .collect::<Vec<_>>();
        assert_eq!(gradients.len(), 1);
        assert_eq!(
            gradients[0]
                .children()
                .filter(|node| node.has_tag_name("stop"))
                .count(),
            2
        );
        let body = xml
            .descendants()
            .find(|node| node.has_tag_name("line"))
            .expect("saved type-8 body is retained");
        let expected_stroke = format!("url(#{})", gradients[0].attribute("id").unwrap());
        assert_eq!(
            body.ancestors().find_map(|node| node.attribute("stroke")),
            Some(expected_stroke.as_str())
        );

        let mut radial_line = super::gradient::tests::hf02_line_with_gradient();
        let Some(crate::ShapePaintSource::Color(radial_source)) =
            radial_line.style.paint_source.as_mut()
        else {
            unreachable!("fixture has a color gradient source");
        };
        radial_source.gradient_type = 1;
        radial_source.position =
            [0.25_f32, 0.75_f32].map(|value| crate::PaintFloat32::from_bits(value.to_bits()));
        radial_source.stops[0].argb = 0xffff_0000;
        radial_source.stops[0].position = crate::PaintFloat32::from_bits(0.5_f32.to_bits());
        radial_source.stops[1].argb = 0xff00_00ff;
        radial_source.stops[1].position = crate::PaintFloat32::from_bits(0.75_f32.to_bits());
        let mut page = page_with_uncolored_stroke();
        page.width = 640;
        page.height = 640;
        page.objects = vec![crate::PageElement::Line(radial_line).into()];
        let rendered = render_document_svg(&document(page), &RenderOptions::default()).remove(0);
        assert!(rendered.paint_diagnostics.is_empty());
        let xml = roxmltree::Document::parse(&rendered.svg).unwrap();
        let radial_gradients = xml
            .descendants()
            .filter(|node| node.has_tag_name("radialGradient"))
            .collect::<Vec<_>>();
        assert_eq!(radial_gradients.len(), 1);
        for (attribute, expected) in [
            ("cx", f64::from(f32::from_bits(0x4300_deab))),
            ("cy", f64::from(f32::from_bits(0x4406_6f1a))),
            ("r", f64::from(f32::from_bits(0x4339_b42b))),
        ] {
            let actual = radial_gradients[0]
                .attribute(attribute)
                .unwrap()
                .parse::<f64>()
                .unwrap();
            assert!(
                (actual - expected).abs() < 1e-4,
                "{attribute}: {actual} != {expected}"
            );
        }
        let radial_stops = radial_gradients[0]
            .children()
            .filter(|node| node.has_tag_name("stop"))
            .collect::<Vec<_>>();
        assert_eq!(radial_stops.len(), 4);
        assert_eq!(radial_stops[0].attribute("offset"), Some("0"));
        assert_eq!(radial_stops[0].attribute("stop-color"), Some("#0000ff"));
        assert_eq!(radial_stops[1].attribute("offset"), Some("0.5"));
        assert_eq!(radial_stops[1].attribute("stop-color"), Some("#0000ff"));
        assert_eq!(radial_stops[2].attribute("offset"), Some("0.5"));
        assert_eq!(radial_stops[2].attribute("stop-color"), Some("#ff0000"));
        assert_eq!(radial_stops[3].attribute("offset"), Some("1"));
        assert_eq!(radial_stops[3].attribute("stop-color"), Some("#ff0000"));
        assert!(
            !xml.descendants()
                .any(|node| node.has_tag_name("linearGradient"))
        );

        #[cfg(feature = "pdf")]
        {
            let mut pdf_line = super::gradient::tests::hf02_line_with_gradient();
            let Some(crate::ShapePaintSource::Color(pdf_source)) =
                pdf_line.style.paint_source.as_mut()
            else {
                unreachable!("fixture has a color gradient source");
            };
            pdf_source.gradient_type = 1;
            pdf_source.position =
                [0.25_f32, 0.75_f32].map(|value| crate::PaintFloat32::from_bits(value.to_bits()));
            pdf_source.stops[0].argb = 0xffff_0000;
            pdf_source.stops[0].position = crate::PaintFloat32::from_bits(0.5_f32.to_bits());
            pdf_source.stops[1].argb = 0xff00_00ff;
            pdf_source.stops[1].position = crate::PaintFloat32::from_bits(0.75_f32.to_bits());
            let mut page = page_with_uncolored_stroke();
            page.width = 640;
            page.height = 640;
            page.objects = vec![crate::PageElement::Line(pdf_line).into()];
            let bytes = crate::render_document_pdf(
                &document(page),
                &RenderOptions::default(),
                &crate::PdfOptions::default(),
            )
            .unwrap();
            let pdf = lopdf::Document::load_mem(&bytes).unwrap();
            let dictionaries = pdf.objects.values().filter_map(pdf_object_dictionary);
            let mut radial_shadings = 0;
            let mut images = 0;
            for dictionary in dictionaries {
                images += usize::from(
                    dictionary
                        .get(b"Subtype")
                        .and_then(lopdf::Object::as_name)
                        .ok()
                        == Some(b"Image".as_slice()),
                );
                radial_shadings += usize::from(
                    dictionary
                        .get(b"ShadingType")
                        .and_then(lopdf::Object::as_i64)
                        .ok()
                        == Some(3),
                );
            }
            assert_eq!(images, 0, "radial step paint must stay vector-only");
            assert!(radial_shadings > 0, "PDF retains radial shading paint");
        }

        let mut invalid_radial = super::gradient::tests::hf02_line_with_gradient();
        let Some(crate::ShapePaintSource::Color(invalid_source)) =
            invalid_radial.style.paint_source.as_mut()
        else {
            unreachable!("fixture has a color gradient source");
        };
        invalid_source.gradient_type = 1;
        invalid_source.position[0] = crate::PaintFloat32::from_bits(f32::NAN.to_bits());
        let mut page = page_with_uncolored_stroke();
        page.width = 640;
        page.height = 640;
        page.objects = vec![crate::PageElement::Line(invalid_radial).into()];
        let rendered = render_document_svg(&document(page), &RenderOptions::default()).remove(0);
        assert_eq!(
            rendered.paint_diagnostics[0].kind,
            crate::PaintDiagnosticKind::UnrepresentableGradient
        );
        let xml = roxmltree::Document::parse(&rendered.svg).unwrap();
        assert!(
            !xml.descendants()
                .any(|node| node.has_tag_name("radialGradient")
                    || node.has_tag_name("linearGradient"))
        );

        let mut rejected_line = super::gradient::tests::hf02_line_with_gradient();
        rejected_line.metadata.rotation_degrees = Some(1.0);
        let mut page = page_with_uncolored_stroke();
        page.width = 640;
        page.height = 640;
        page.objects = vec![crate::PageElement::Line(rejected_line).into()];
        let rendered = render_document_svg(&document(page), &RenderOptions::default()).remove(0);
        assert_eq!(
            rendered.paint_diagnostics,
            [crate::PaintDiagnostic {
                source_offset: None,
                object_uuid: "type8-gradient".into(),
                role: crate::PaintRole::Outline,
                kind: crate::PaintDiagnosticKind::UnsupportedGradientFrame,
            }]
        );
        let xml = roxmltree::Document::parse(&rendered.svg).unwrap();
        let body = xml
            .descendants()
            .find(|node| node.has_tag_name("line"))
            .expect("rejected paint retains the line body");
        assert_eq!(
            body.ancestors().find_map(|node| node.attribute("stroke")),
            Some("none")
        );
        assert!(
            !xml.descendants()
                .any(|node| node.has_tag_name("linearGradient"))
        );
    }

    #[test]
    fn public_sdk_linear_line_gradient_rotation_pair_matches_native_factory_points() {
        // Native pair receipt SHA-256:
        // a9badc002cc9c9ae51f84f27642df5478097201d04520b244236f5951295cdf4
        // Native RGBA: alpha is byte-identical across both 640x640 frames;
        // 149 covered pixels change RGB. This isolates gradient appearance,
        // while the SDK's line-body endpoints must remain unchanged.
        let render_pair = |rotation| {
            let mut line = super::gradient::tests::hf02_line_with_gradient();
            line.metadata.rotation_degrees = rotation;
            let Some(crate::ShapePaintSource::Color(source)) = line.style.paint_source.as_mut()
            else {
                unreachable!("gradient fixture has a color paint source");
            };
            source.property_flags = 1;
            source.stops = vec![
                crate::GradientStopSource {
                    argb: 0xffff_0000,
                    position: crate::PaintFloat32::from_bits(0x3f00_0000),
                },
                crate::GradientStopSource {
                    argb: 0xff00_00ff,
                    position: crate::PaintFloat32::from_bits(0x3f40_0000),
                },
            ];
            let mut page = page_with_uncolored_stroke();
            page.width = 640;
            page.height = 640;
            page.objects = vec![crate::PageElement::Line(line).into()];
            let document = document(page);
            let rendered = render_document_svg(&document, &RenderOptions::default()).remove(0);
            (rendered, document)
        };
        let (control, control_document) = render_pair(None);
        let (rotated, rotated_document) = render_pair(Some(30.0));
        #[cfg(not(feature = "pdf"))]
        let _ = (&control_document, &rotated_document);
        for page in [&control, &rotated] {
            assert!(page.paint_diagnostics.is_empty());
        }
        let control_xml = roxmltree::Document::parse(&control.svg).unwrap();
        let rotated_xml = roxmltree::Document::parse(&rotated.svg).unwrap();
        let gradient_points = |xml: &roxmltree::Document<'_>| {
            let gradient = xml
                .descendants()
                .find(|node| node.has_tag_name("linearGradient"))
                .expect("public export contains the linear line paint");
            ["x1", "y1", "x2", "y2"].map(|name| {
                gradient
                    .attribute(name)
                    .expect("gradient endpoint attribute")
                    .parse::<f32>()
                    .unwrap()
                    .to_bits()
            })
        };
        assert_eq!(
            gradient_points(&control_xml),
            [0x42c3_4a24, 0x43fb_b093, 0x435e_8b75, 0x43fb_b093]
        );
        assert_eq!(
            gradient_points(&rotated_xml),
            [0x42d4_05e6, 0x43ec_13c6, 0x4356_2d94, 0x4405_a6b0]
        );
        let body_line = |xml: &roxmltree::Document<'_>| {
            let node = xml
                .descendants()
                .find(|node| node.has_tag_name("line"))
                .expect("saved line body remains vector geometry");
            ["x1", "y1", "x2", "y2"].map(|name| node.attribute(name).unwrap().to_owned())
        };
        assert_eq!(body_line(&control_xml), body_line(&rotated_xml));
        for xml in [&control_xml, &rotated_xml] {
            let gradient = xml
                .descendants()
                .find(|node| node.has_tag_name("linearGradient"))
                .unwrap();
            let stops = gradient
                .children()
                .filter(|node| node.has_tag_name("stop"))
                .map(|stop| {
                    (
                        stop.attribute("offset").unwrap(),
                        stop.attribute("stop-color").unwrap(),
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(
                stops,
                [
                    ("0", "#0000ff"),
                    ("0.5", "#0000ff"),
                    ("0.5", "#ff0000"),
                    ("1", "#ff0000")
                ]
            );
        }

        // Optional public-SDK artifact path for the ignored source-aligned
        // native comparison runner. Normal unit tests remain filesystem-free.
        if let Some(directory) = std::env::var_os("SDOCX_LINEAR_ROTATION_ARTIFACT_DIR") {
            let directory = std::path::PathBuf::from(directory);
            std::fs::create_dir_all(&directory).unwrap();
            for (name, rendered, document) in [
                ("rotation-0", &control, &control_document),
                ("rotation-30", &rotated, &rotated_document),
            ] {
                std::fs::write(directory.join(format!("{name}.svg")), &rendered.svg).unwrap();
                let tree = resvg::usvg::Tree::from_str(&rendered.svg, &Default::default()).unwrap();
                let mut pixmap =
                    resvg::tiny_skia::Pixmap::new(rendered.width, rendered.height).unwrap();
                resvg::render(
                    &tree,
                    resvg::tiny_skia::Transform::identity(),
                    &mut pixmap.as_mut(),
                );
                std::fs::write(directory.join(format!("{name}.rgba")), pixmap.data()).unwrap();
                #[cfg(feature = "pdf")]
                std::fs::write(
                    directory.join(format!("{name}.pdf")),
                    crate::render_document_pdf(
                        document,
                        &RenderOptions::default(),
                        &crate::PdfOptions::default(),
                    )
                    .unwrap(),
                )
                .unwrap();
                #[cfg(not(feature = "pdf"))]
                let _ = document;
            }
        }
    }

    #[test]
    fn public_sdk_radial_line_gradient_rotation_pair_matches_native_factory_centers() {
        // Native V4 pair receipt SHA-256:
        // 5fef3cbe04ec9f9d6576aeaa6225719bc55f21909680b9c9b3cbac6ef3f77247
        // Native geometry: centers [4300deab,44066f1a] ->
        // [42e7bff5,44016151], with the same radius 4339b42b.
        // Native RGB changed in 54 pixels while alpha stayed identical across
        // the full 640x640 frames. These paired metrics are not an SDK/native
        // pixel-parity measure; the SDK raster is retained separately below.
        let render_pair = |rotation| {
            let mut line = super::gradient::tests::hf02_line_with_gradient();
            line.metadata.rotation_degrees = rotation;
            let Some(crate::ShapePaintSource::Color(source)) = line.style.paint_source.as_mut()
            else {
                unreachable!("gradient fixture has a color paint source");
            };
            source.gradient_type = 1;
            source.property_flags = 1;
            source.position =
                [0.25_f32, 0.75].map(|value| crate::PaintFloat32::from_bits(value.to_bits()));
            source.stops = vec![
                crate::GradientStopSource {
                    argb: 0xffff_0000,
                    position: crate::PaintFloat32::from_bits(0x3f00_0000),
                },
                crate::GradientStopSource {
                    argb: 0xff00_00ff,
                    position: crate::PaintFloat32::from_bits(0x3f40_0000),
                },
            ];
            let mut page = page_with_uncolored_stroke();
            page.width = 640;
            page.height = 640;
            page.objects = vec![crate::PageElement::Line(line).into()];
            let document = document(page);
            let rendered = render_document_svg(&document, &RenderOptions::default()).remove(0);
            (rendered, document)
        };
        let (control, control_document) = render_pair(None);
        let (rotated, rotated_document) = render_pair(Some(30.0));
        for page in [&control, &rotated] {
            assert!(page.paint_diagnostics.is_empty());
        }
        let control_xml = roxmltree::Document::parse(&control.svg).unwrap();
        let rotated_xml = roxmltree::Document::parse(&rotated.svg).unwrap();
        let radial_geometry = |xml: &roxmltree::Document<'_>| {
            let radial = xml
                .descendants()
                .find(|node| node.has_tag_name("radialGradient"))
                .expect("public export contains radial line paint");
            assert!(
                !xml.descendants().any(|node| node.has_tag_name("image")),
                "radial gradient remains vector-only in SVG"
            );
            ["cx", "cy", "r"].map(|name| {
                radial
                    .attribute(name)
                    .expect("radial gradient geometry attribute")
                    .parse::<f32>()
                    .unwrap()
                    .to_bits()
            })
        };
        assert_eq!(
            radial_geometry(&control_xml),
            [0x4300_deab, 0x4406_6f1a, 0x4339_b42b]
        );
        assert_eq!(
            radial_geometry(&rotated_xml),
            [0x42e7_bff5, 0x4401_6151, 0x4339_b42b]
        );
        let body_line = |xml: &roxmltree::Document<'_>| {
            let node = xml
                .descendants()
                .find(|node| node.has_tag_name("line"))
                .expect("saved line body remains vector geometry");
            ["x1", "y1", "x2", "y2"].map(|name| node.attribute(name).unwrap().to_owned())
        };
        assert_eq!(body_line(&control_xml), body_line(&rotated_xml));
        for xml in [&control_xml, &rotated_xml] {
            let radial = xml
                .descendants()
                .find(|node| node.has_tag_name("radialGradient"))
                .unwrap();
            let stops = radial
                .children()
                .filter(|node| node.has_tag_name("stop"))
                .map(|stop| {
                    (
                        stop.attribute("offset").unwrap(),
                        stop.attribute("stop-color").unwrap(),
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(
                stops,
                [
                    ("0", "#0000ff"),
                    ("0.5", "#0000ff"),
                    ("0.5", "#ff0000"),
                    ("1", "#ff0000")
                ]
            );
        }

        let rasterize = |page: &super::RenderedPage| {
            let tree = resvg::usvg::Tree::from_str(&page.svg, &Default::default()).unwrap();
            let mut pixmap = resvg::tiny_skia::Pixmap::new(page.width, page.height).unwrap();
            resvg::render(
                &tree,
                resvg::tiny_skia::Transform::identity(),
                &mut pixmap.as_mut(),
            );
            pixmap.data().to_vec()
        };
        let control_rgba = rasterize(&control);
        let rotated_rgba = rasterize(&rotated);
        assert_eq!(control_rgba.len(), 640 * 640 * 4);
        assert_eq!(rotated_rgba.len(), control_rgba.len());
        let mut rgb_changed_pixels = 0;
        let mut alpha_changed_pixels = 0;
        let mut active_union = 0;
        let mut rgb_delta_sum = 0_u64;
        const SDK_PAGE_BACKGROUND: [u8; 4] = [252, 252, 252, 255];
        for (before, after) in control_rgba
            .as_chunks::<4>()
            .0
            .iter()
            .zip(rotated_rgba.as_chunks::<4>().0)
        {
            if before[3] != after[3] {
                alpha_changed_pixels += 1;
            }
            let before_is_page_background = *before == SDK_PAGE_BACKGROUND;
            let after_is_page_background = *after == SDK_PAGE_BACKGROUND;
            if !before_is_page_background || !after_is_page_background {
                active_union += 1;
                let changed = (0..3).any(|channel| before[channel] != after[channel]);
                rgb_changed_pixels += usize::from(changed);
                rgb_delta_sum += (0..3)
                    .map(|channel| u64::from(before[channel].abs_diff(after[channel])))
                    .sum::<u64>();
            }
        }
        assert!(
            rgb_changed_pixels > 0,
            "rotation changes visible radial RGB"
        );
        assert_eq!(
            alpha_changed_pixels, 0,
            "opaque paint keeps the stroke mask fixed"
        );
        let sdk_rgb_mae = rgb_delta_sum as f64 / (active_union * 3) as f64;

        #[cfg(feature = "pdf")]
        let mut pdf_stats = Vec::new();
        #[cfg(feature = "pdf")]
        for document in [&control_document, &rotated_document] {
            let bytes = crate::render_document_pdf(
                document,
                &RenderOptions::default(),
                &crate::PdfOptions::default(),
            )
            .unwrap();
            let pdf = lopdf::Document::load_mem(&bytes).unwrap();
            let dictionaries = pdf.objects.values().filter_map(pdf_object_dictionary);
            let mut radial_shadings = 0;
            let mut images = 0;
            for dictionary in dictionaries {
                images += usize::from(
                    dictionary
                        .get(b"Subtype")
                        .and_then(lopdf::Object::as_name)
                        .ok()
                        == Some(b"Image".as_slice()),
                );
                radial_shadings += usize::from(
                    dictionary
                        .get(b"ShadingType")
                        .and_then(lopdf::Object::as_i64)
                        .ok()
                        == Some(3),
                );
            }
            assert_eq!(images, 0, "radial gradient remains vector-only in PDF");
            assert!(radial_shadings > 0, "PDF retains radial shading");
            pdf_stats.push((bytes, radial_shadings, images));
        }

        if let Some(directory) = std::env::var_os("SDOCX_RADIAL_ROTATION_ARTIFACT_DIR") {
            let directory = std::path::PathBuf::from(directory);
            std::fs::create_dir_all(&directory).unwrap();
            for (name, rendered, rgba) in [
                ("rotation-0", &control, &control_rgba),
                ("rotation-30", &rotated, &rotated_rgba),
            ] {
                std::fs::write(directory.join(format!("{name}.svg")), &rendered.svg).unwrap();
                std::fs::write(directory.join(format!("{name}.rgba")), rgba).unwrap();
            }
            std::fs::write(
                directory.join("pair-metrics.json"),
                serde_json::to_vec(&serde_json::json!({
                    "schema": "sdocx.type8-radial-rotation-sdk-pair-metrics.v1",
                    "rgb_changed_pixels": rgb_changed_pixels,
                    "alpha_changed_pixels": alpha_changed_pixels,
                    "sdk_non_background_union_pixels": active_union,
                    "sdk_vs_sdk_rgb_mae_non_background_union": sdk_rgb_mae,
                    "excluded_page_background_rgba": SDK_PAGE_BACKGROUND,
                    "native_comparison_included": false,
                }))
                .unwrap(),
            )
            .unwrap();
            #[cfg(feature = "pdf")]
            for ((name, document), (bytes, radial_shadings, images)) in [
                ("rotation-0", &control_document),
                ("rotation-30", &rotated_document),
            ]
            .into_iter()
            .zip(pdf_stats)
            {
                let _ = document;
                std::fs::write(directory.join(format!("{name}.pdf")), bytes).unwrap();
                std::fs::write(
                    directory.join(format!("{name}.pdf.stats.json")),
                    serde_json::to_vec(&serde_json::json!({
                        "schema": "sdocx.type8-radial-rotation-pdf-stats.v1",
                        "radial_shading_objects": radial_shadings,
                        "image_xobjects": images,
                    }))
                    .unwrap(),
                )
                .unwrap();
            }
        }
    }

    #[test]
    fn public_sdk_linear_line_gradient_partial_alpha_controls_are_vector_only() {
        // Native alpha controls include the original descending .75/.5 pair
        // plus low-stop cache buckets 0 and 51. Optional files feed the
        // offline source-grid comparator; this test never launches the oracle.
        let cases = [
            (
                "half-red-opaque-blue",
                0x80ff_0000,
                0xff00_00ff,
                0x3f00_0000,
                128,
            ),
            (
                "half-red-quarter-blue",
                0x80ff_0000,
                0x4000_00ff,
                0x3f00_0000,
                128,
            ),
            (
                "zero-red-quarter-blue",
                0x00ff_0000,
                0x4000_00ff,
                0x3f00_0000,
                128,
            ),
            (
                "nonprimary-half-quarter",
                0x8059_21cd,
                0x4000_00ff,
                0x3f00_0000,
                128,
            ),
            (
                "low-zero-high-quarter",
                0x8059_21cd,
                0x4000_00ff,
                0x0000_0000,
                0,
            ),
            (
                "low-sub-bucket-high-quarter",
                0x8059_21cd,
                0x4000_00ff,
                0x3b00_0000,
                0,
            ),
            (
                "low-offgrid-high-quarter",
                0x8059_21cd,
                0x4000_00ff,
                0x3e4c_cccd,
                51,
            ),
        ];
        let artifact_dir =
            std::env::var_os("SDOCX_LINEAR_ALPHA_ARTIFACT_DIR").map(std::path::PathBuf::from);
        if let Some(directory) = &artifact_dir {
            std::fs::create_dir_all(directory).unwrap();
        }

        for (name, low_argb, high_argb, low_position_bits, expected_bucket) in cases {
            let mut line = super::gradient::tests::hf02_line_with_gradient();
            line.metadata.rotation_degrees = Some(30.0);
            let Some(crate::ShapePaintSource::Color(source)) = line.style.paint_source.as_mut()
            else {
                unreachable!("gradient fixture has a color paint source");
            };
            source.property_flags = 1;
            source.stops = vec![
                crate::GradientStopSource {
                    argb: low_argb,
                    position: crate::PaintFloat32::from_bits(low_position_bits),
                },
                crate::GradientStopSource {
                    argb: high_argb,
                    position: crate::PaintFloat32::from_bits(0x3f40_0000), // .75
                },
            ];
            let mut page = page_with_uncolored_stroke();
            page.width = 640;
            page.height = 640;
            page.objects = vec![crate::PageElement::Line(line).into()];
            let document = document(page);
            let rendered = render_document_svg(&document, &RenderOptions::default()).remove(0);
            assert!(rendered.paint_diagnostics.is_empty(), "{name}");
            let xml = roxmltree::Document::parse(&rendered.svg).unwrap();
            assert!(
                !xml.descendants().any(|node| node.has_tag_name("image")),
                "linear alpha control remains vector-only: {name}"
            );
            let gradient = xml
                .descendants()
                .find(|node| node.has_tag_name("linearGradient"))
                .expect("public export contains the linear line paint");
            let stops = gradient
                .children()
                .filter(|node| node.has_tag_name("stop"))
                .collect::<Vec<_>>();
            let expected = if expected_bucket == 0 {
                vec![(0.0, low_argb), (1.0, low_argb)]
            } else {
                let boundary = f64::from(expected_bucket) / 256.0;
                vec![
                    (0.0, high_argb),
                    (boundary, high_argb),
                    (boundary, low_argb),
                    (1.0, low_argb),
                ]
            };
            assert_eq!(stops.len(), expected.len(), "{name}");
            for (node, (offset, argb)) in stops.iter().zip(expected) {
                let actual_offset = node.attribute("offset").unwrap().parse::<f64>().unwrap();
                assert!((actual_offset - offset).abs() < 1e-12, "{name}");
                let expected_color = format!(
                    "#{:02x}{:02x}{:02x}",
                    (argb >> 16) as u8,
                    (argb >> 8) as u8,
                    argb as u8
                );
                assert_eq!(node.attribute("stop-color"), Some(expected_color.as_str()));
                let opacity = node
                    .attribute("stop-opacity")
                    .unwrap_or("1")
                    .parse::<f64>()
                    .unwrap();
                assert!((opacity - f64::from((argb >> 24) as u8) / 255.0).abs() < 1e-12);
            }

            #[cfg(feature = "pdf")]
            let pdf_bytes = crate::render_document_pdf(
                &document,
                &RenderOptions::default(),
                &crate::PdfOptions::default(),
            )
            .unwrap();
            #[cfg(feature = "pdf")]
            {
                let pdf = lopdf::Document::load_mem(&pdf_bytes).unwrap();
                let dictionaries = pdf.objects.values().filter_map(pdf_object_dictionary);
                let mut linear_shadings = 0;
                let mut images = 0;
                for dictionary in dictionaries {
                    images += usize::from(
                        dictionary
                            .get(b"Subtype")
                            .and_then(lopdf::Object::as_name)
                            .ok()
                            == Some(b"Image".as_slice()),
                    );
                    linear_shadings += usize::from(
                        dictionary
                            .get(b"ShadingType")
                            .and_then(lopdf::Object::as_i64)
                            .ok()
                            == Some(2),
                    );
                }
                assert_eq!(
                    images, 0,
                    "linear alpha control remains vector-only: {name}"
                );
                assert!(linear_shadings > 0, "PDF retains linear shading: {name}");
                if let Some(directory) = &artifact_dir {
                    let stats = serde_json::json!({
                        "schema": "sdocx.type8-linear-alpha-pdf-stats.v1",
                        "linear_shading_objects": linear_shadings,
                        "image_xobjects": images,
                    });
                    std::fs::write(
                        directory.join(format!("{name}.pdf.stats.json")),
                        serde_json::to_vec(&stats).unwrap(),
                    )
                    .unwrap();
                }
            }

            if let Some(directory) = &artifact_dir {
                std::fs::write(directory.join(format!("{name}.svg")), &rendered.svg).unwrap();
                let tree = resvg::usvg::Tree::from_str(&rendered.svg, &Default::default()).unwrap();
                let mut pixmap =
                    resvg::tiny_skia::Pixmap::new(rendered.width, rendered.height).unwrap();
                resvg::render(
                    &tree,
                    resvg::tiny_skia::Transform::identity(),
                    &mut pixmap.as_mut(),
                );
                std::fs::write(directory.join(format!("{name}.rgba")), pixmap.data()).unwrap();
                #[cfg(feature = "pdf")]
                std::fs::write(directory.join(format!("{name}.pdf")), pdf_bytes).unwrap();
            }
        }
    }

    #[test]
    fn public_radial_line_step_keeps_partial_zero_and_nonprimary_stops_vector_only() {
        let cases = [
            // V35: red alpha 128, blue alpha 255.
            (0x80ff_0000, 0xff00_00ff),
            // V36: red alpha 128, blue alpha 64.
            (0x80ff_0000, 0x4000_00ff),
            // V37: serialized red RGB remains but alpha is zero.
            (0x00ff_0000, 0x4000_00ff),
            // V38: partial-alpha low stop carries nonprimary RGB.
            (0x8059_21cd, 0x4000_00ff),
        ];
        for (low_argb, high_argb) in cases {
            let mut line = super::gradient::tests::hf02_line_with_gradient();
            let Some(crate::ShapePaintSource::Color(source)) = line.style.paint_source.as_mut()
            else {
                unreachable!("fixture has a color gradient source");
            };
            source.gradient_type = 1;
            source.position =
                [0.25_f32, 0.75_f32].map(|value| crate::PaintFloat32::from_bits(value.to_bits()));
            source.stops[0] = crate::GradientStopSource {
                argb: low_argb,
                position: crate::PaintFloat32::from_bits(0x3f00_c49c),
            };
            source.stops[1] = crate::GradientStopSource {
                argb: high_argb,
                position: crate::PaintFloat32::from_bits(0x3f40_4189),
            };

            let mut page = page_with_uncolored_stroke();
            page.width = 640;
            page.height = 640;
            page.objects = vec![crate::PageElement::Line(line).into()];
            let rendered =
                render_document_svg(&document(page.clone()), &RenderOptions::default()).remove(0);
            assert!(rendered.paint_diagnostics.is_empty());
            let xml = roxmltree::Document::parse(&rendered.svg).unwrap();
            assert!(
                !xml.descendants().any(|node| node.has_tag_name("image")),
                "type-8 radial step remains vector-only"
            );
            let gradients = xml
                .descendants()
                .filter(|node| node.has_tag_name("radialGradient"))
                .collect::<Vec<_>>();
            assert_eq!(gradients.len(), 1);
            let stops = gradients[0]
                .children()
                .filter(|node| node.has_tag_name("stop"))
                .collect::<Vec<_>>();
            assert_eq!(stops.len(), 4);
            let expected = [
                (0.0, high_argb),
                (0.5, high_argb),
                (0.5, low_argb),
                (1.0, low_argb),
            ];
            for (node, (offset, argb)) in stops.iter().zip(expected) {
                let actual_offset = node.attribute("offset").unwrap().parse::<f64>().unwrap();
                assert!((actual_offset - offset).abs() < 1e-12);
                assert_eq!(
                    node.attribute("stop-color"),
                    Some(
                        format!(
                            "#{:02x}{:02x}{:02x}",
                            (argb >> 16) as u8,
                            (argb >> 8) as u8,
                            argb as u8
                        )
                        .as_str()
                    )
                );
                let actual_opacity = node
                    .attribute("stop-opacity")
                    .unwrap_or("1")
                    .parse::<f64>()
                    .unwrap();
                let expected_opacity = f64::from((argb >> 24) as u8) / 255.0;
                assert!((actual_opacity - expected_opacity).abs() < 1e-12);
            }

            #[cfg(feature = "pdf")]
            {
                let bytes = crate::render_document_pdf(
                    &document(page),
                    &RenderOptions::default(),
                    &crate::PdfOptions::default(),
                )
                .unwrap();
                let pdf = lopdf::Document::load_mem(&bytes).unwrap();
                let dictionaries = pdf.objects.values().filter_map(pdf_object_dictionary);
                let mut radial_shadings = 0;
                let mut images = 0;
                for dictionary in dictionaries {
                    images += usize::from(
                        dictionary
                            .get(b"Subtype")
                            .and_then(lopdf::Object::as_name)
                            .ok()
                            == Some(b"Image".as_slice()),
                    );
                    radial_shadings += usize::from(
                        dictionary
                            .get(b"ShadingType")
                            .and_then(lopdf::Object::as_i64)
                            .ok()
                            == Some(3),
                    );
                }
                assert_eq!(images, 0, "radial stop controls stay vector-only in PDF");
                assert!(radial_shadings > 0, "PDF retains radial shading paint");
            }
        }
    }

    #[test]
    fn public_sdk_radial_nstop_pairs_match_endpoint_projection() {
        // Native N3, N4 and N10 pairs record identical caches/frames after
        // changing consumed interior ARGB. The native N11 pair changes only
        // unconsumed index 10 and retains identical cache/frame/texture bytes.
        // This SDK export also compares both N11 inputs with their consumed-
        // prefix N10 baseline. These source-grid cases do not claim a saved
        // gradient or device fixture.
        let n4 = vec![
            (0xffff_0000, 0x3dcc_cccd),
            (0xff00_ff00, 0x3e99_999a),
            (0xffff_ff00, 0x3f33_3333),
            (0xff00_00ff, 0x3f66_6666),
        ];
        let n4_variant = vec![
            (0xffff_0000, 0x3dcc_cccd),
            (0x80ff_00ff, 0x3e99_999a),
            (0xffff_ff00, 0x3f33_3333),
            (0xff00_00ff, 0x3f66_6666),
        ];
        let n10 = vec![
            (0xffff_0000, 0x3dcc_cccd),
            (0xff00_ff00, 0x3e38_51ec),
            (0xffff_ff00, 0x3e8a_3d71),
            (0xffff_8000, 0x3eb8_51ec),
            (0xffff_ffff, 0x3ee6_6666),
            (0xff00_ffff, 0x3f0a_3d71),
            (0xffff_00ff, 0x3f21_47ae),
            (0xff80_8080, 0x3f38_51ec),
            (0xff00_8080, 0x3f4f_5c29),
            (0xff00_00ff, 0x3f66_6666),
        ];
        let mut n10_variant = n10.clone();
        n10_variant[5].0 = 0x80ff_00ff;
        let n11 = vec![
            (0xffff_0000, 0x3dcc_cccd),
            (0xff00_ff00, 0x3e38_51ec),
            (0xffff_ff00, 0x3e85_1eb8),
            (0xffff_8000, 0x3eae_147b),
            (0xffff_ffff, 0x3ed7_0a3d),
            (0xff00_ffff, 0x3f00_0000),
            (0xffff_00ff, 0x3f14_7ae1),
            (0xff80_8080, 0x3f28_f5c3),
            (0xff00_8080, 0x3f3d_70a4),
            (0xff80_40c0, 0x3f51_eb85),
            (0xff00_00ff, 0x3f66_6666),
        ];
        let mut n11_unused_index10_variant = n11.clone();
        n11_unused_index10_variant[0].0 = 0xff00_ff00;
        let n11_consumed_prefix_baseline = n11[1..].to_vec();
        let cases = [
            (
                "v2-green",
                "n3",
                vec![
                    (0x0000_ffff, 0x3e4c_cccd),
                    (0xff00_ff00, 0x3f19_999a),
                    (0xff00_0000, 0x3f66_6666),
                ],
                51.0 / 256.0,
            ),
            (
                "v2-magenta",
                "n3",
                vec![
                    (0x0000_ffff, 0x3e4c_cccd),
                    (0xffff_00ff, 0x3f19_999a),
                    (0xff00_0000, 0x3f66_6666),
                ],
                51.0 / 256.0,
            ),
            (
                "opaque-green",
                "n3-opaque",
                vec![
                    (0xffff_0000, 0x3e4c_cccd),
                    (0xff00_ff00, 0x3f19_999a),
                    (0xff00_00ff, 0x3f66_6666),
                ],
                51.0 / 256.0,
            ),
            (
                "opaque-half-magenta",
                "n3-opaque",
                vec![
                    (0xffff_0000, 0x3e4c_cccd),
                    (0x80ff_00ff, 0x3f19_999a),
                    (0xff00_00ff, 0x3f66_6666),
                ],
                51.0 / 256.0,
            ),
            ("n4-control", "n4", n4, 25.0 / 256.0),
            ("n4-interior-change", "n4", n4_variant, 25.0 / 256.0),
            ("n10-control", "n10", n10, 25.0 / 256.0),
            ("n10-interior-change", "n10", n10_variant, 25.0 / 256.0),
            ("n11-control", "n11", n11, 46.0 / 256.0),
            (
                "n11-index10-color-change",
                "n11",
                n11_unused_index10_variant,
                46.0 / 256.0,
            ),
            (
                "n11-consumed-prefix-baseline",
                "n11-prefix",
                n11_consumed_prefix_baseline,
                46.0 / 256.0,
            ),
        ];
        let artifact_dir =
            std::env::var_os("SDOCX_RADIAL_NSTOP_ARTIFACT_DIR").map(std::path::PathBuf::from);
        if let Some(directory) = &artifact_dir {
            std::fs::create_dir_all(directory).unwrap();
        }

        let mut outputs = Vec::new();
        for (name, pair, stops_input, boundary) in cases {
            let mut line = super::gradient::tests::hf02_line_with_gradient();
            let Some(crate::ShapePaintSource::Color(source)) = line.style.paint_source.as_mut()
            else {
                unreachable!("fixture has a color gradient source");
            };
            source.gradient_type = 1;
            source.property_flags = 0;
            source.position = [crate::PaintFloat32::from_bits(0x3f00_0000); 2];
            source.stops = stops_input
                .iter()
                .map(|&(argb, position)| crate::GradientStopSource {
                    argb,
                    position: crate::PaintFloat32::from_bits(position),
                })
                .collect();
            let low_argb = source.stops[source.stops.len().saturating_sub(10)].argb;
            let high_argb = source.stops.last().unwrap().argb;
            let mut page = page_with_uncolored_stroke();
            page.width = 640;
            page.height = 640;
            page.objects = vec![crate::PageElement::Line(line).into()];
            let document = document(page);
            let rendered = render_document_svg(&document, &RenderOptions::default()).remove(0);
            assert!(
                rendered.paint_diagnostics.is_empty(),
                "{name}: {:?}",
                rendered.paint_diagnostics
            );
            let xml = roxmltree::Document::parse(&rendered.svg).unwrap();
            assert!(
                !xml.descendants().any(|node| node.has_tag_name("image")),
                "radial n-stop projection remains vector-only"
            );
            let gradient = xml
                .descendants()
                .find(|node| node.has_tag_name("radialGradient"))
                .expect("public export contains radial line paint");
            let stops = gradient
                .children()
                .filter(|node| node.has_tag_name("stop"))
                .collect::<Vec<_>>();
            assert_eq!(stops.len(), 4, "interior stops are skipped by cache spans");
            for (node, (offset, argb)) in stops.iter().zip([
                (0.0, high_argb),
                (boundary, high_argb),
                (boundary, low_argb),
                (1.0, low_argb),
            ]) {
                assert!(
                    (node.attribute("offset").unwrap().parse::<f64>().unwrap() - offset).abs()
                        < 1e-12
                );
                let color = format!(
                    "#{:02x}{:02x}{:02x}",
                    (argb >> 16) as u8,
                    (argb >> 8) as u8,
                    argb as u8
                );
                assert_eq!(node.attribute("stop-color"), Some(color.as_str()));
                let opacity = node
                    .attribute("stop-opacity")
                    .unwrap_or("1")
                    .parse::<f64>()
                    .unwrap();
                assert!((opacity - f64::from((argb >> 24) as u8) / 255.0).abs() < 1e-12);
            }

            let tree = resvg::usvg::Tree::from_str(&rendered.svg, &Default::default()).unwrap();
            let mut pixmap =
                resvg::tiny_skia::Pixmap::new(rendered.width, rendered.height).unwrap();
            resvg::render(
                &tree,
                resvg::tiny_skia::Transform::identity(),
                &mut pixmap.as_mut(),
            );
            let rgba = pixmap.data().to_vec();

            #[cfg(feature = "pdf")]
            let pdf_bytes = crate::render_document_pdf(
                &document,
                &RenderOptions::default(),
                &crate::PdfOptions::default(),
            )
            .unwrap();
            #[cfg(feature = "pdf")]
            {
                let pdf = lopdf::Document::load_mem(&pdf_bytes).unwrap();
                let dictionaries = pdf.objects.values().filter_map(pdf_object_dictionary);
                let mut radial_shadings = 0;
                let mut images = 0;
                for dictionary in dictionaries {
                    images += usize::from(
                        dictionary
                            .get(b"Subtype")
                            .and_then(lopdf::Object::as_name)
                            .ok()
                            == Some(b"Image".as_slice()),
                    );
                    radial_shadings += usize::from(
                        dictionary
                            .get(b"ShadingType")
                            .and_then(lopdf::Object::as_i64)
                            .ok()
                            == Some(3),
                    );
                }
                assert_eq!(images, 0, "radial n-stop paint stays vector-only");
                assert!(radial_shadings > 0, "PDF retains radial shading");
                if let Some(directory) = &artifact_dir {
                    std::fs::write(directory.join(format!("{name}.pdf")), &pdf_bytes).unwrap();
                    std::fs::write(
                        directory.join(format!("{name}.pdf.stats.json")),
                        serde_json::to_vec(&serde_json::json!({
                            "schema": "sdocx.type8-radial-nstop-pdf-stats.v1",
                            "radial_shading_objects": radial_shadings,
                            "image_xobjects": images,
                        }))
                        .unwrap(),
                    )
                    .unwrap();
                }
            }

            if let Some(directory) = &artifact_dir {
                std::fs::write(directory.join(format!("{name}.svg")), &rendered.svg).unwrap();
                std::fs::write(directory.join(format!("{name}.rgba")), &rgba).unwrap();
            }
            outputs.push((pair, rendered.svg, rgba));
        }
        for pair in ["n3", "n3-opaque", "n4", "n10", "n11"] {
            let paired = outputs
                .iter()
                .filter(|(name, _, _)| *name == pair)
                .collect::<Vec<_>>();
            assert_eq!(paired.len(), 2, "{pair} has a native paired control");
            assert_eq!(paired[0].1, paired[1].1, "{pair} SVG is endpoint-only");
            assert_eq!(paired[0].2, paired[1].2, "{pair} raster is endpoint-only");
        }
        let n11_outputs = outputs
            .iter()
            .filter(|(pair, _, _)| *pair == "n11")
            .collect::<Vec<_>>();
        let n11_prefix = outputs
            .iter()
            .find(|(pair, _, _)| *pair == "n11-prefix")
            .expect("N11 case has the same-first-ten N10 baseline");
        assert_eq!(n11_outputs.len(), 2);
        for output in n11_outputs {
            assert_eq!(
                output.1, n11_prefix.1,
                "N11 export equals the consumed-prefix SVG"
            );
            assert_eq!(
                output.2, n11_prefix.2,
                "N11 raster equals the consumed-prefix baseline"
            );
        }
    }

    #[test]
    fn public_radial_line_cache_bucket_zero_projects_low_stop_as_constant_vector_paint() {
        // The V41/V42 captures retain the full native 256x4 cache and its
        // matching GL_RGBA source upload; capture pins are recorded in the
        // type-8 radial cache section of shape-fill-findings.md.
        for (low_argb, high_argb) in [
            (0xffff_0000, 0xff00_00ff), // V41 opaque control
            (0x8059_21cd, 0x4000_00ff), // V42 partial-alpha, nonprimary control
            (0x00ab_cdef, 0x4000_00ff), // transparent low-stop serialization control
        ] {
            let mut line = super::gradient::tests::hf02_line_with_gradient();
            let Some(crate::ShapePaintSource::Color(source)) = line.style.paint_source.as_mut()
            else {
                unreachable!("fixture has a color gradient source");
            };
            source.gradient_type = 1;
            source.position =
                [0.5_f32; 2].map(|value| crate::PaintFloat32::from_bits(value.to_bits()));
            source.stops[0] = crate::GradientStopSource {
                argb: low_argb,
                position: crate::PaintFloat32::from_bits(0x3b44_9ba6),
            };
            source.stops[1] = crate::GradientStopSource {
                argb: high_argb,
                position: crate::PaintFloat32::from_bits(0x3f40_4189),
            };
            let original_source = line.style.paint_source.clone();

            let mut page = page_with_uncolored_stroke();
            page.width = 640;
            page.height = 640;
            page.objects = vec![crate::PageElement::Line(line).into()];
            let doc = document(page);
            let rendered = render_document_svg(&doc, &RenderOptions::default()).remove(0);
            assert!(rendered.paint_diagnostics.is_empty());
            let xml = roxmltree::Document::parse(&rendered.svg).unwrap();
            let gradients = xml
                .descendants()
                .filter(|node| node.has_tag_name("radialGradient"))
                .collect::<Vec<_>>();
            assert_eq!(gradients.len(), 1);
            let stops = gradients[0]
                .children()
                .filter(|node| node.has_tag_name("stop"))
                .collect::<Vec<_>>();
            assert_eq!(stops.len(), 2);
            let expected_color = format!(
                "#{:02x}{:02x}{:02x}",
                (low_argb >> 16) as u8,
                (low_argb >> 8) as u8,
                low_argb as u8
            );
            let expected_opacity = f64::from((low_argb >> 24) as u8) / 255.0;
            for (stop, offset) in stops.iter().zip(["0", "1"]) {
                assert_eq!(stop.attribute("offset"), Some(offset));
                assert_eq!(stop.attribute("stop-color"), Some(expected_color.as_str()));
                let actual_opacity = stop.attribute("stop-opacity").unwrap_or("1");
                assert!((actual_opacity.parse::<f64>().unwrap() - expected_opacity).abs() < 1e-12);
            }
            assert!(!xml.descendants().any(|node| node.has_tag_name("image")));
            let crate::PageElement::Line(rendered_line) = doc.pages[0].elements().next().unwrap()
            else {
                unreachable!("test document retains its line source")
            };
            assert_eq!(rendered_line.style.paint_source, original_source);

            #[cfg(feature = "pdf")]
            {
                let bytes = crate::render_document_pdf(
                    &doc,
                    &RenderOptions::default(),
                    &crate::PdfOptions::default(),
                )
                .unwrap();
                let pdf = lopdf::Document::load_mem(&bytes).unwrap();
                let dictionaries = pdf.objects.values().filter_map(pdf_object_dictionary);
                let mut images = 0;
                for dictionary in dictionaries {
                    images += usize::from(
                        dictionary
                            .get(b"Subtype")
                            .and_then(lopdf::Object::as_name)
                            .ok()
                            == Some(b"Image".as_slice()),
                    );
                }
                assert_eq!(images, 0, "bucket-zero paint remains vector-only in PDF");
                assert!(
                    pdf.get_pages().contains_key(&1),
                    "bucket-zero PDF retains its vector page"
                );
            }
        }
    }

    fn empty_bullet_body(kind: u32) -> RichTextBox {
        RichTextBox {
            text_area_type: None,
            bbox: BoundingBox::default(),
            rotation_degrees: None,
            text: String::new(),
            color: None,
            highlight_color: None,
            underline: false,
            font_size: Some(18.0),
            runs: Vec::new(),
            spans: Vec::new(),
            paragraphs: vec![RichTextParagraph {
                kind: RichTextParagraphType::Bullet,
                start_paragraph: 0,
                end_paragraph: 1,
                payload: [kind, 1, 0, 1]
                    .into_iter()
                    .flat_map(u32::to_le_bytes)
                    .collect(),
            }],
            object_spans: Vec::new(),
            text_sections: vec![RichTextSection {
                start_utf16: 0,
                length_utf16: 0,
            }],
            margins: None,
            gravity: None,
        }
    }

    #[test]
    fn empty_arrow_body_marker_survives_svg_replay_and_pdf() {
        let mut page = page_with_uncolored_stroke();
        page.objects.clear();
        let mut source = document(page);
        source.metadata.page_mode = Some(0);
        source.metadata.note_text = Some(empty_bullet_body(1));
        let layout = layout_document(&source);
        assert!(layout.pages[0].body_text_slice().is_some());

        let normal = render_document_svg(&source, &RenderOptions::default())
            .remove(0)
            .svg;
        let replay =
            super::render_layout_page_replay_svg(&source, &layout, 0, &RenderOptions::default())
                .unwrap()
                .svg;
        assert!(
            normal.contains("<circle"),
            "empty Arrow marker missing from SVG"
        );
        assert!(
            replay.contains("<circle"),
            "empty Arrow marker missing from replay"
        );

        #[cfg(feature = "pdf")]
        {
            let arrow = crate::render_document_pdf(
                &source,
                &RenderOptions::default(),
                &crate::PdfOptions::default(),
            )
            .unwrap();
            let arrow = lopdf::Document::load_mem(&arrow).unwrap();
            let arrow_page = *arrow.get_pages().values().next().unwrap();
            let arrow_content = arrow.get_page_content(arrow_page).unwrap();
            assert!(
                lopdf::content::Content::decode(&arrow_content)
                    .unwrap()
                    .operations
                    .iter()
                    .any(|operation| operation.operator == "c"),
                "the marker's curved path must survive PDF lowering"
            );

            let mut no_bullet = source.clone();
            no_bullet.metadata.note_text = Some(empty_bullet_body(0));
            let blank = crate::render_document_pdf(
                &no_bullet,
                &RenderOptions::default(),
                &crate::PdfOptions::default(),
            )
            .unwrap();
            let blank = lopdf::Document::load_mem(&blank).unwrap();
            let blank_page = *blank.get_pages().values().next().unwrap();
            assert_ne!(arrow_content, blank.get_page_content(blank_page).unwrap());
        }
    }

    #[test]
    fn saved_singleton_strokes_are_not_admitted_to_page_paint_or_replay() {
        let page = page_with_uncolored_stroke();
        let crate::PageObjectContent::Stroke(generic) = page.objects[0].content.leaf() else {
            panic!("expected stroke fixture");
        };
        let mut generic = generic.clone();
        generic.points.truncate(1);
        generic.points[0] = Point { x: 40.0, y: 40.0 };
        generic.pressures.truncate(1);
        generic.timestamps.truncate(1);
        generic.tilts.truncate(1);
        generic.orientations.truncate(1);

        let mut native = inkpen2_color_stroke(0xff00_0000, None);
        native.points.truncate(1);
        native.points[0] = Point { x: 40.0, y: 40.0 };
        native.pressures.truncate(1);
        native.timestamps.truncate(1);
        native.tilts.truncate(1);
        native.orientations.truncate(1);
        let mut chinese = native.clone();
        chinese.rendering.as_mut().unwrap().pen_name = Some("ChineseBrush".into());

        for stroke in [&generic, &chinese, &native] {
            let options = RenderOptions::default();
            let prepared = [
                super::prepare_stroke_for_page(stroke, false),
                super::prepare_stroke_for_page_with_theme(
                    stroke,
                    RenderTheme::for_canvas(false),
                    &options,
                ),
                super::prepare_stroke_for_viewport_with_theme(
                    stroke,
                    RenderTheme::for_canvas(false),
                    &options,
                    [100, 100],
                ),
            ];
            for prepared in &prepared {
                assert!(prepared.points.is_empty());
                assert!(prepared.bounds.is_none());
                assert_eq!(prepared.sample_ends.as_deref(), Some(&[0][..]));
            }
            #[cfg(feature = "serde")]
            {
                let serialized = serde_json::to_value(&prepared[1]).unwrap();
                assert_eq!(serialized["points"], serde_json::json!([]));
                assert_eq!(serialized["bounds"], serde_json::Value::Null);
            }

            let mut page = page_with_uncolored_stroke();
            page.objects = vec![stroke.clone().into()];
            let source = document(page);
            let normal = render_document_svg(&source, &RenderOptions::default())[0]
                .svg
                .clone();
            let replay = super::render_layout_page_replay_svg(
                &source,
                &layout_document(&source),
                0,
                &RenderOptions::default(),
            )
            .unwrap()
            .svg;
            assert!(!normal.contains("<path"), "singleton page export painted");
            assert!(!normal.contains("<circle"), "singleton page export painted");
            assert!(!replay.contains("<path"), "singleton replay painted");
            assert!(!replay.contains("<circle"), "singleton replay painted");

            #[cfg(feature = "pdf")]
            {
                let bytes = crate::render_document_pdf(
                    &source,
                    &RenderOptions::default(),
                    &crate::PdfOptions::default(),
                )
                .unwrap();
                let pdf = lopdf::Document::load_mem(&bytes).unwrap();
                let page_id = *pdf.get_pages().values().next().unwrap();
                let content = pdf.get_page_content(page_id).unwrap();
                let mut blank_page = page_with_uncolored_stroke();
                blank_page.objects.clear();
                let blank = document(blank_page);
                let blank_bytes = crate::render_document_pdf(
                    &blank,
                    &RenderOptions::default(),
                    &crate::PdfOptions::default(),
                )
                .unwrap();
                let blank_pdf = lopdf::Document::load_mem(&blank_bytes).unwrap();
                let blank_id = *blank_pdf.get_pages().values().next().unwrap();
                assert_eq!(
                    content,
                    blank_pdf.get_page_content(blank_id).unwrap(),
                    "singleton PDF export must match a page with no stroke"
                );
            }
        }

        let mut coincident = generic.clone();
        coincident.points.push(coincident.points[0]);
        let mut page = page_with_uncolored_stroke();
        page.objects = vec![coincident.into()];
        let svg = render_document_svg(&document(page), &RenderOptions::default())[0]
            .svg
            .clone();
        assert!(
            svg.contains("<polyline"),
            "two-point tap was rejected: {svg}"
        );

        assert_eq!(crate::prepare_stroke(&generic, false).points.len(), 1);
    }

    fn inkpen2_color_stroke(argb: u32, color_override: Option<Color>) -> Stroke {
        Stroke {
            rendering: Some(StrokeRendering {
                metadata: None,
                pen_name: Some("com.samsung.android.sdk.pen.pen.preload.InkPen2".into()),
                advanced_settings: Some("9;".into()),
                tool_type_raw: 2,
                properties: StrokeProperties {
                    compressed: true,
                    replay_only: false,
                    stylus_channels: true,
                    eraser: false,
                    fixed_width: false,
                    millisecond_timestamps: false,
                    top_layer_pen: false,
                    alpha_lock: false,
                    binary_added: true,
                    generated: false,
                    fixed_opacity: false,
                    rainbow_effect: false,
                    straighten: false,
                    reveal_mode: false,
                },
                style: StrokeStyle {
                    color_argb: Some(argb),
                    pen_size: Some(4.0),
                    initial_tolerance: Some(0.0),
                    line_type_raw: Some(0),
                    stroke_type_raw: Some(0),
                    ..StrokeStyle::default()
                },
            }),
            bbox: BoundingBox::default(),
            points: vec![Point { x: 0.0, y: 0.0 }, Point { x: 8.0, y: 0.0 }],
            pressures: vec![0.25, 0.5],
            timestamps: vec![100, 110],
            tilts: vec![0.0, 0.0],
            orientations: vec![0.0, 0.0],
            color: color_override,
            pen_width: 4.0,
        }
    }

    fn pencil3_native_profile_stroke() -> Stroke {
        let mut stroke = inkpen2_color_stroke(0xff00_0000, None);
        let rendering = stroke.rendering.as_mut().unwrap();
        rendering.pen_name = Some("com.samsung.android.sdk.pen.pen.preload.Pencil3".into());
        rendering.advanced_settings = Some("4;".into());
        rendering.style = StrokeStyle {
            color_argb: Some(0xff00_0000),
            pen_size: Some(10.),
            ..StrokeStyle::default()
        };
        stroke.points = [[160., 100.], [170., 100.], [180., 100.], [190., 110.]]
            .map(|[x, y]| Point { x, y })
            .to_vec();
        stroke.pressures = vec![0.25, 0.5, 0.75, 1.];
        stroke.timestamps = vec![10, 20, 30, 40];
        stroke.tilts = vec![0.; 4];
        stroke.orientations = vec![0.; 4];
        stroke.pen_width = 10.;
        stroke.bbox = BoundingBox {
            x_min: 160.,
            y_min: 100.,
            x_max: 190.,
            y_max: 110.,
        };
        stroke
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn pencil3_zero_saved_alpha_is_transparent_in_fallback_and_replay() {
        let mut page = page_with_uncolored_stroke();
        page.width = 320;
        page.height = 180;
        page.objects.clear();
        let empty = document(page.clone());
        let raster = |svg: &str| {
            let tree = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default()).unwrap();
            let mut pixels = resvg::tiny_skia::Pixmap::new(320, 180).unwrap();
            resvg::render(
                &tree,
                resvg::tiny_skia::Transform::identity(),
                &mut pixels.as_mut(),
            );
            pixels.data().to_vec()
        };
        let options = RenderOptions {
            pencil3_profile: Some(super::Pencil3RenderProfile {
                screen_width: 2560,
                screen_height: 1440,
                canvas_width: 321, // Force the canvas-dependent profile to fall back.
                canvas_height: 180,
                paper_sampler: super::Pencil3PaperSampler::ClampToEdge,
            }),
            ..RenderOptions::default()
        };
        for color in [None, Some(Color { r: 255, g: 0, b: 0 })] {
            let mut stroke = pencil3_native_profile_stroke();
            stroke.rendering.as_mut().unwrap().style.color_argb = Some(0x0012_3456);
            stroke.color = color;
            let mut page = page.clone();
            page.objects = vec![stroke.clone().into()];
            let doc = document(page);
            for opts in [&RenderOptions::default(), &options] {
                let prepared = super::prepare_stroke_for_viewport_with_theme(
                    &stroke,
                    RenderTheme::for_canvas(false),
                    opts,
                    [320, 180],
                );
                assert_eq!(prepared.opacity, 0.0);
                let layout = layout_document(&doc);
                let svg = render_document_svg(&doc, opts).remove(0).svg;
                let replay = render_layout_page_replay_svg(&doc, &layout, 0, opts)
                    .unwrap()
                    .svg;
                let background = render_document_svg(&empty, opts).remove(0).svg;
                assert_eq!(raster(&svg), raster(&background));
                assert_eq!(raster(&replay), raster(&background));
            }
        }

        let mut unsupported = pencil3_native_profile_stroke();
        unsupported.rendering.as_mut().unwrap().advanced_settings = Some("3;".into());
        unsupported.rendering.as_mut().unwrap().style.color_argb = Some(0x0012_3456);
        assert_eq!(crate::prepare_stroke(&unsupported, false).opacity, 1.0);
        let mut partial = pencil3_native_profile_stroke();
        partial.rendering.as_mut().unwrap().style.color_argb = Some(0x8012_3456);
        assert_eq!(crate::prepare_stroke(&partial, false).opacity, 1.0);
        page.objects = vec![unsupported.into()];
        let doc = document(page);
        let mut supported_canvas = options;
        supported_canvas
            .pencil3_profile
            .as_mut()
            .unwrap()
            .canvas_width = 320;
        assert_eq!(
            render_document_svg(&doc, &supported_canvas).remove(0).svg,
            render_document_svg(&doc, &RenderOptions::default())
                .remove(0)
                .svg,
        );
    }

    #[test]
    fn pencil3_public_profile_retains_native_particles_and_shared_intrinsic_assets() {
        use sha2::Digest;
        let stroke = pencil3_native_profile_stroke();
        let prepared =
            crate::ink::pencil_source::prepare_pencil3_v3_source(&stroke, 2560, 1440).unwrap();
        assert_eq!(prepared.records.len(), 48);
        let bytes: Vec<u8> = prepared
            .records
            .iter()
            .flatten()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        assert_eq!(
            format!("{:x}", sha2::Sha256::digest(bytes)),
            "f5d7ece4ff8260805d3ccdf45d89afbe22c16764855f4ed8b0221b96bf7b09f0"
        );
        let mut page = page_with_uncolored_stroke();
        page.width = 320;
        page.height = 180;
        let mut second = stroke.clone();
        second.color = Some(Color { r: 255, g: 0, b: 0 });
        for point in &mut second.points {
            point.x -= 100.;
        }
        second.bbox.x_min -= 100.;
        second.bbox.x_max -= 100.;
        page.objects = vec![stroke.into(), second.into()];
        let doc = document(page);
        let options = RenderOptions {
            pencil3_profile: Some(super::Pencil3RenderProfile {
                screen_width: 2560,
                screen_height: 1440,
                canvas_width: 320,
                canvas_height: 180,
                paper_sampler: super::Pencil3PaperSampler::ClampToEdge,
            }),
            ..RenderOptions::default()
        };
        let svg = render_document_svg(&doc, &options).remove(0).svg;
        assert_eq!(
            svg.matches("data:image/png;base64,").count(),
            2,
            "both strokes reuse the original brush and paper assets"
        );
        assert_eq!(svg.matches("<mask ").count(), 98);
        assert!(
            svg.contains("fill=\"#ff0000\""),
            "presentation RGB override remains effective"
        );
        #[cfg(feature = "pdf")]
        {
            let tree = usvg::Tree::from_str(&svg, &usvg::Options::default()).unwrap();
            assert_eq!(tree.size().width(), 320.);
        }
        #[cfg(feature = "pdf")]
        {
            let bytes =
                crate::render_document_pdf(&doc, &options, &crate::PdfOptions::default()).unwrap();
            let pdf = lopdf::Document::load_mem(&bytes).unwrap();
            let images = pdf
                .objects
                .values()
                .filter_map(pdf_object_dictionary)
                .filter(|dict| {
                    dict.get(b"Subtype")
                        .and_then(lopdf::Object::as_name)
                        .is_ok_and(|name| name == b"Image")
                })
                .count();
            assert_eq!(
                images, 4,
                "two intrinsic images with their alpha soft masks"
            );
        }
    }

    #[test]
    fn pencil3_tool1_profile_exposes_particles() {
        let mut stroke = pencil3_native_profile_stroke();
        stroke.rendering.as_mut().unwrap().tool_type_raw = 1;
        let options = RenderOptions {
            pencil3_profile: Some(super::Pencil3RenderProfile {
                screen_width: 2560,
                screen_height: 1440,
                canvas_width: 320,
                canvas_height: 180,
                paper_sampler: super::Pencil3PaperSampler::ClampToEdge,
            }),
            ..RenderOptions::default()
        };
        let prepared = super::prepare_stroke_for_viewport_with_theme(
            &stroke,
            RenderTheme::for_canvas(false),
            &options,
            [320, 180],
        );
        assert_eq!(prepared.profile, Some("Pencil3V3"));
        assert!(!prepared.points.is_empty());
    }

    #[test]
    fn pencil3_tool3_profile_exposes_particles_and_replay() {
        let mut stroke = pencil3_native_profile_stroke();
        stroke.rendering.as_mut().unwrap().tool_type_raw = 3;
        let source =
            crate::ink::pencil_source::prepare_pencil3_v3_source(&stroke, 2560, 1440).unwrap();
        assert!(!source.records.is_empty());
        let options = RenderOptions {
            pencil3_profile: Some(super::Pencil3RenderProfile {
                screen_width: 2560,
                screen_height: 1440,
                canvas_width: 320,
                canvas_height: 180,
                paper_sampler: super::Pencil3PaperSampler::ClampToEdge,
            }),
            ..RenderOptions::default()
        };
        let theme = RenderTheme::for_canvas(false);
        let prepared =
            super::prepare_stroke_for_viewport_with_theme(&stroke, theme, &options, [320, 180]);
        assert_eq!(prepared.profile, Some("Pencil3V3"));
        assert_eq!(prepared.points.len(), source.records.len());
        for (point, record) in prepared.points.iter().zip(&source.records) {
            assert_eq!(
                (point.x, point.y),
                (f64::from(record[0]), f64::from(record[1]))
            );
        }
        assert_eq!(prepared.sample_ends.as_ref(), Some(&source.sample_ends));
        assert_eq!(source.sample_ends.last(), Some(&source.records.len()));
        assert_ne!(
            super::prepare_stroke_for_viewport_with_theme(
                &stroke,
                theme,
                &RenderOptions::default(),
                [320, 180],
            )
            .profile,
            Some("Pencil3V3")
        );
        assert_ne!(
            super::prepare_stroke_for_viewport_with_theme(&stroke, theme, &options, [321, 180])
                .profile,
            Some("Pencil3V3")
        );

        let mut page = page_with_uncolored_stroke();
        page.width = 320;
        page.height = 180;
        page.objects = vec![stroke.into()];
        let doc = document(page);
        let layout = layout_document(&doc);
        let normal = super::render_layout_page_svg(&doc, &layout, 0, &options).unwrap();
        let replay = render_layout_page_replay_svg(&doc, &layout, 0, &options).unwrap();
        assert_eq!(normal.svg.matches("data:image/png;base64,").count(), 2);
        assert_eq!(replay.svg.matches("data:image/png;base64,").count(), 2);
        let xml = roxmltree::Document::parse(&replay.svg).unwrap();
        let group = xml
            .descendants()
            .find(|n| n.attribute("data-replay-stroke") == Some("0"))
            .unwrap();
        let parts: Vec<_> = group
            .descendants()
            .filter_map(|n| n.attribute("data-replay-part"))
            .collect();
        assert_eq!(
            parts,
            (1..=source.records.len())
                .map(|i| i.to_string())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn pencil3_replay_retains_particles_resources_and_source_cutoffs() {
        let stroke = pencil3_native_profile_stroke();
        let mut page = page_with_uncolored_stroke();
        page.width = 320;
        page.height = 180;
        page.objects = vec![stroke.clone().into()];
        let doc = document(page);
        let layout = layout_document(&doc);
        for sampler in [
            super::Pencil3PaperSampler::Repeat,
            super::Pencil3PaperSampler::ClampToEdge,
        ] {
            let options = RenderOptions {
                pencil3_profile: Some(super::Pencil3RenderProfile {
                    screen_width: 2560,
                    screen_height: 1440,
                    canvas_width: 320,
                    canvas_height: 180,
                    paper_sampler: sampler,
                }),
                ..RenderOptions::default()
            };
            let prepared = super::prepare_stroke_for_viewport_with_theme(
                &stroke,
                RenderTheme::for_canvas(false),
                &options,
                [320, 180],
            );
            assert_eq!(prepared.profile, Some("Pencil3V3"));
            assert_eq!(prepared.points.len(), 48);
            let ends = prepared.sample_ends.as_ref().unwrap();
            assert_eq!(ends.len(), stroke.points.len());
            assert_eq!(ends[0], 7);
            assert_eq!(ends.last(), Some(&48));
            assert!(ends.windows(2).all(|pair| pair[0] <= pair[1]));
            let normal = super::render_layout_page_svg(&doc, &layout, 0, &options).unwrap();
            let replay = render_layout_page_replay_svg(&doc, &layout, 0, &options).unwrap();
            let xml = roxmltree::Document::parse(&replay.svg).unwrap();
            let group = xml
                .descendants()
                .find(|n| n.attribute("data-replay-stroke") == Some("0"))
                .unwrap();
            let parts: Vec<_> = group
                .descendants()
                .filter_map(|n| n.attribute("data-replay-part"))
                .collect();
            assert_eq!(parts, (1..=48).map(|i| i.to_string()).collect::<Vec<_>>());
            assert!(!group.descendants().any(|n| n.has_tag_name("defs")));
            assert_eq!(replay.svg.matches("data:image/png;base64,").count(), 2);
            #[cfg(feature = "pdf")]
            {
                let raster = |svg: &str| {
                    let tree =
                        resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default()).unwrap();
                    let mut pixmap = resvg::tiny_skia::Pixmap::new(80, 45).unwrap();
                    resvg::render(
                        &tree,
                        resvg::tiny_skia::Transform::from_scale(0.25, 0.25),
                        &mut pixmap.as_mut(),
                    );
                    pixmap
                };
                assert_eq!(raster(&normal.svg).data(), raster(&replay.svg).data());
            }
            #[cfg(not(feature = "pdf"))]
            assert_eq!(normal.width, replay.width);
        }
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn pencil3_profile_slice_preserves_crossing_particles_and_replay_indices() {
        let stroke = pencil3_native_profile_stroke();
        let ink =
            crate::ink::pencil_source::prepare_pencil3_v3_source(&stroke, 2560, 1440).unwrap();
        // Choose an integer boundary through a particle, with at least one
        // other particle outside the one-unit slice.
        let y = ink
            .records
            .iter()
            .find_map(|record| {
                let boundary = f64::from(record[1]).ceil();
                let half = f64::from(record[2]);
                (boundary > 0.0
                    && boundary < 179.0
                    && boundary > f64::from(record[1]) - half
                    && boundary < f64::from(record[1]) + half
                    && ink.records.iter().any(|r| {
                        f64::from(r[1]) + f64::from(r[2]) < boundary
                            || f64::from(r[1]) - f64::from(r[2]) > boundary + 1.0
                    }))
                .then_some(boundary as u32)
            })
            .expect("fixture contains a crossing particle and an off-slice particle");
        let expected: Vec<String> = ink
            .records
            .iter()
            .enumerate()
            .filter(|(_, r)| {
                f64::from(r[1]) + f64::from(r[2]) >= f64::from(y)
                    && f64::from(r[1]) - f64::from(r[2]) <= f64::from(y + 1)
            })
            .map(|(index, _)| (index + 1).to_string())
            .collect();
        let mut page = page_with_uncolored_stroke();
        page.width = 320;
        page.height = 180;
        page.objects = vec![stroke.into()];
        let doc = document(page);
        let layout = layout_document(&doc);
        let options = RenderOptions {
            pencil3_profile: Some(super::Pencil3RenderProfile {
                screen_width: 2560,
                screen_height: 1440,
                canvas_width: 320,
                canvas_height: 180,
                paper_sampler: super::Pencil3PaperSampler::ClampToEdge,
            }),
            ..RenderOptions::default()
        };
        let fonts = super::fonts::FontBook::default();
        let mut cache = super::DocumentTextCache::default();
        cache.bind(&doc, &options, &fonts);
        let render = |cache: &mut super::DocumentTextCache, slice| {
            let (_, scene) = cache.prepare_page(
                &doc,
                &layout.pages[0],
                &options,
                true,
                &fonts,
                false,
                slice,
                &mut |_| {},
            );
            scene.finish()
        };
        let full = render(&mut cache, None);
        let slice = super::PageSlice::new_for_page(&layout.pages[0], y, 1, true).unwrap();
        let sliced = render(&mut cache, Some(slice));
        let xml = roxmltree::Document::parse(&sliced).unwrap();
        let group = xml
            .descendants()
            .find(|n| n.attribute("data-replay-stroke") == Some("0"))
            .unwrap();
        let actual: Vec<_> = group
            .descendants()
            .filter_map(|n| n.attribute("data-replay-part"))
            .map(str::to_owned)
            .collect();
        assert_eq!(actual, expected);
        assert!(actual.len() < ink.records.len());
        // Give the unculled full-page SVG the exact same root viewport as the
        // slice. Comparing different viewports can introduce unrelated usvg
        // rounding differences even when they map to the same document row.
        let sliced_xml = roxmltree::Document::parse(&sliced).unwrap();
        let slice_root = sliced_xml.root_element();
        let mut unculled_slice = full.clone();
        for attribute in ["viewBox", "height"] {
            let value = slice_root.attribute(attribute).unwrap();
            let root_end = unculled_slice.find('>').unwrap();
            let prefix = format!("{attribute}=\"");
            let start = unculled_slice[..root_end].find(&prefix).unwrap() + prefix.len();
            let end = start + unculled_slice[start..root_end].find('"').unwrap();
            unculled_slice.replace_range(start..end, value);
        }
        let raster = |svg: &str| {
            let tree = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default()).unwrap();
            let mut pixmap = resvg::tiny_skia::Pixmap::new(320, 1).unwrap();
            resvg::render(
                &tree,
                resvg::tiny_skia::Transform::identity(),
                &mut pixmap.as_mut(),
            );
            pixmap
        };
        assert_eq!(raster(&unculled_slice).data(), raster(&sliced).data());

        // Keep the slice SVG and transform fixed while changing only the
        // target pixmap height. This isolates a resvg viewport-size effect
        // from the particle culling above; it is not a native-device oracle.
        let full_tree = resvg::usvg::Tree::from_str(&full, &Default::default()).unwrap();
        let slice_tree = resvg::usvg::Tree::from_str(&unculled_slice, &Default::default()).unwrap();
        let mut full_target = resvg::tiny_skia::Pixmap::new(320, 180).unwrap();
        let mut large_slice_target = resvg::tiny_skia::Pixmap::new(320, 180).unwrap();
        resvg::render(
            &full_tree,
            resvg::tiny_skia::Transform::identity(),
            &mut full_target.as_mut(),
        );
        resvg::render(
            &slice_tree,
            resvg::tiny_skia::Transform::identity(),
            &mut large_slice_target.as_mut(),
        );
        let row = &full_target.data()[y as usize * 320 * 4..(y as usize + 1) * 320 * 4];
        let large_row = &large_slice_target.data()[..320 * 4];
        assert!(
            row == large_row,
            "same slice in a full-height target must preserve this control"
        );
        let small_target = raster(&unculled_slice);
        let max_difference = small_target
            .data()
            .iter()
            .zip(large_row)
            .map(|(small, large)| small.abs_diff(*large))
            .max()
            .unwrap_or(0);
        assert!(
            max_difference <= 186,
            "one-row target exceeded the observed sampling residual: {max_difference}"
        );
    }

    #[test]
    fn pencil3_replay_metadata_falls_back_when_paper_exceeds_budget() {
        let stroke = pencil3_native_profile_stroke();
        let mut page = page_with_uncolored_stroke();
        page.width = 4096;
        page.height = 4096;
        page.objects = vec![stroke.clone().into()];
        let doc = document(page);
        let options = RenderOptions {
            pencil3_profile: Some(super::Pencil3RenderProfile {
                screen_width: 2560,
                screen_height: 1440,
                canvas_width: 4096,
                canvas_height: 4096,
                paper_sampler: super::Pencil3PaperSampler::Repeat,
            }),
            ..RenderOptions::default()
        };
        let ink = super::prepare_pencil3_v3(
            &stroke,
            options.pencil3_profile.unwrap(),
            RenderTheme::for_canvas(false),
            [4096, 4096],
        )
        .unwrap();
        let mut context = super::pencil3_paint_context(options.pencil3_profile.unwrap());
        assert!(!super::pencil3::eligible(&ink, context));
        context.canvas_width = 128;
        context.canvas_height = 128;
        assert!(super::pencil3::eligible(&ink, context));
        assert_eq!(
            render_document_svg(&doc, &options).remove(0).svg,
            render_document_svg(&doc, &RenderOptions::default())
                .remove(0)
                .svg,
        );
        let layout = layout_document(&doc);
        let replay = super::render_layout_page_replay_svg(&doc, &layout, 0, &options)
            .unwrap()
            .svg;
        let fallback_replay =
            super::render_layout_page_replay_svg(&doc, &layout, 0, &RenderOptions::default())
                .unwrap()
                .svg;
        assert_eq!(replay, fallback_replay);
        let prepared = super::prepare_stroke_for_viewport_with_theme(
            &stroke,
            RenderTheme::for_canvas(false),
            &options,
            [4096, 4096],
        );
        assert_ne!(prepared.profile, Some("Pencil3V3"));
    }

    #[test]
    fn pencil3_public_profile_falls_back_for_unverified_selector_or_canvas() {
        let mut page = page_with_uncolored_stroke();
        page.width = 320;
        page.height = 180;
        page.objects = vec![pencil3_native_profile_stroke().into()];
        let doc = document(page);
        let fallback = render_document_svg(&doc, &RenderOptions::default())
            .remove(0)
            .svg;
        let options = RenderOptions {
            pencil3_profile: Some(super::Pencil3RenderProfile {
                screen_width: 2560,
                screen_height: 1440,
                canvas_width: 321,
                canvas_height: 180,
                paper_sampler: super::Pencil3PaperSampler::Repeat,
            }),
            ..RenderOptions::default()
        };
        assert_eq!(render_document_svg(&doc, &options).remove(0).svg, fallback);
        let layout = layout_document(&doc);
        assert_eq!(
            render_layout_page_replay_svg(&doc, &layout, 0, &options)
                .unwrap()
                .svg,
            render_layout_page_replay_svg(&doc, &layout, 0, &RenderOptions::default())
                .unwrap()
                .svg,
            "mismatched replay canvas uses the same fallback",
        );
        let geometry = super::prepare_stroke_for_viewport_with_theme(
            &pencil3_native_profile_stroke(),
            RenderTheme::for_canvas(false),
            &options,
            [320, 180],
        )
        .profile;
        assert_ne!(geometry, Some("Pencil3V3"));
        let mut unsupported = pencil3_native_profile_stroke();
        unsupported.rendering.as_mut().unwrap().advanced_settings = Some("3;".into());
        let mut page = doc.pages[0].clone();
        page.objects = vec![unsupported.into()];
        let doc = document(page);
        let mut options = options;
        options.pencil3_profile.as_mut().unwrap().canvas_width = 320;
        assert_eq!(
            render_document_svg(&doc, &options).remove(0).svg,
            render_document_svg(&doc, &RenderOptions::default())
                .remove(0)
                .svg
        );
    }

    #[test]
    fn inkpen2_profile_matches_compact_native_caller_streams_and_rejects_single_point() {
        let profile = InkPen2RenderProfile {
            screen_width: 1920,
            screen_height: 1080,
            enhanced_antialias: false,
        };
        let make_stroke = |points: &[[f64; 2]], times: &[i64], pressures: &[f64], curve: bool| {
            let mut stroke = inkpen2_color_stroke(0xff000000, None);
            stroke.points = points
                .iter()
                .map(|xy| Point { x: xy[0], y: xy[1] })
                .collect();
            stroke.timestamps = times.to_vec();
            stroke.pressures = pressures.to_vec();
            stroke.tilts = vec![0.0; points.len()];
            stroke.orientations = vec![0.0; points.len()];
            let rendering = stroke.rendering.as_mut().unwrap();
            rendering.properties.compressed = curve;
            rendering.style.initial_tolerance = Some(0.0);
            stroke
        };
        let decode = |hex: &str| -> Vec<u8> {
            hex.as_bytes()
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect()
        };
        let cases: [(&str, Stroke, &str); 2] = [
            (
                "native linear source-caller",
                make_stroke(
                    &[[0.0, 0.0], [8.0, 0.0], [16.0, 3.0], [24.0, 3.0]],
                    &[0, 10, 20, 30],
                    &[0.2, 0.4, 0.6, 0.8],
                    false,
                ),
                "faff7f3f00000000175d1440175d14409191913e0000803f000000000000000000000000faffff3f0000000090a7144090a71440e4c8913e0000803f000000000000000000000000fcff3f400000000009f2144009f214400000923e0000803f000000000000000000000000faff7f4000000000823c1540823c1540e436923e0000803f000000000000000000000000fcff9f4000000000fb861540fb861540926d923e0000803f000000000000000000000000fcffbf400000000074d1154074d115400aa4923e0000803f000000000000000000000000fbffdf4000000000ed1b1640ed1b16404bda923e0000803f000000000000000000000000faffff400000000066661640666616405710933e0000803f00000000000000000000000034fb0e4170c6b33edfb01640dfb016402d46933e0000803f00000000000000000000000068f61d4170c6333fd6fc1640d6fc1640e17c933e0000803f0000000000000000000000009cf12c41d4d4863fcc481740cc4817405db3933e0000803f000000000000000000000000d0ec3b4170c6b33fc2941740c2941740a2e9933e0000803f00000000000000000000000004e84a410cb8e03fb9e01740b9e01740b21f943e0000803f00000000000000000000000038e35941d4d40640b02c1840b02c18408c55943e0000803f0000000000000000000000006cde6841a24d1d40a6781840a6781840308b943e0000803f000000000000000000000000a0d9774170c633409cc418409cc418409ec0943e0000803f000000000000000000000000e1a58341000040409210194092101940d7f5943e0000803f000000000000000000000000e1a58b41000040409210194092101940d7f5943e0000803f000000000000000000000000e1a59341000040409210194092101940d7f5943e0000803f000000000000000000000000e1a59b41000040409210194092101940d7f5943e0000803f000000000000000000000000e1a5a341000040409210194092101940d7f5943e0000803f000000000000000000000000e0a5ab41000040409210194092101940d7f5943e0000803f000000000000000000000000e0a5b341000040409210194092101940d7f5943e0000803f000000000000000000000000e0a5bb41000040409210194092101940d7f5943e0000803f0000000000000000000000000000c041000040409210194092101940d7f5943e0000803f000000000000000000000000",
            ),
            (
                "native short two-point source-caller",
                make_stroke(&[[0.0, 0.0], [0.75, 0.0]], &[100, 105], &[0.2, 0.6], true),
                "0000403f000000008b2eba3f8b2eba3fffff1f3e0000803f000000000000000000000000",
            ),
        ];
        for (name, stroke, expected_hex) in cases {
            let prepared =
                prepare_inkpen2_v9(&stroke, Some(profile), RenderTheme::for_canvas(false))
                    .unwrap_or_else(|| {
                        panic!("{name} must be supported by the captured V9 profile")
                    });
            let actual: Vec<u8> = prepared
                .records
                .iter()
                .flat_map(|record| record.iter().flat_map(|value| value.to_le_bytes()))
                .collect();
            let expected = decode(expected_hex);
            assert_eq!(actual, expected, "{name}: full native RTV3 stream");
        }

        let mut one = inkpen2_color_stroke(0xff000000, None);
        one.points.truncate(1);
        one.timestamps.truncate(1);
        one.pressures.truncate(1);
        one.tilts.truncate(1);
        one.orientations.truncate(1);
        assert!(prepare_inkpen2_v9(&one, Some(profile), RenderTheme::for_canvas(false)).is_none());
    }

    #[test]
    fn inkpen2_serialized_style_mapping_reaches_native_v9_records() {
        // Bounded hybrid source control: this type-0 base prefix is borrowed
        // from a Fountain note, while the type-1 style frame is host-built.
        // The native V9 output below was captured after applying that same
        // 106-byte type-1 frame and resolving its IDs through StringIDManager.
        let hex_bytes = |hex: &str| -> Vec<u8> {
            hex.as_bytes()
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect()
        };
        let mut payload = hex_bytes(
            "8900000000006900000002bf010400600400a00f0000240064313630613838652d623534642d313166312d613062392d396663373838333030663136f20edec9f25b06000000000001e55c40000000e0dbbc6940000000e05d9a604000000020769b6c400000000000be7635c6f25b060038070000350a000000000000000000000000000000000000",
        );
        payload.extend(hex_bytes(
            "6a00000001005a000000020504040f000000040000000000000000000000000000000000000100800001600000010080cdcc4c3e330333033303000000000a000a000a00000000000000000000000000000000000000000002000000000001000000563412ff00008040",
        ));
        let mut stroke = crate::decode::decode_stroke(&payload, &crate::ParseLimits::default())
            .expect("parse hybrid native type-1 frame");
        let rendering = stroke.rendering.as_mut().unwrap();
        assert_eq!(rendering.style.legacy_pen_name_id, Some(0));
        assert_eq!(rendering.style.advanced_pen_setting_id, Some(1));
        assert_eq!(rendering.style.color_argb, Some(0xff123456));
        assert_eq!(rendering.style.pen_size, Some(4.0));
        assert_eq!(rendering.style.initial_tolerance, None);
        assert_eq!(rendering.style.stroke_type_raw, None);
        assert_eq!(rendering.style.line_type_raw, None);
        assert_eq!(
            stroke.points,
            [
                crate::Point { x: 0.0, y: 0.0 },
                crate::Point { x: 8.0, y: 0.0 },
                crate::Point { x: 16.0, y: 3.0 },
                crate::Point { x: 24.0, y: 3.0 },
            ]
        );
        crate::StrokeResources::new(&crate::NoteStringTable {
            entries: vec![
                crate::NoteStringId {
                    id: 0,
                    text: "com.samsung.android.sdk.pen.pen.preload.InkPen2".into(),
                },
                crate::NoteStringId {
                    id: 1,
                    text: "9;".into(),
                },
            ],
            trailing_data: Vec::new(),
        })
        .resolve(&mut stroke);
        let rendering = stroke.rendering.as_mut().unwrap();
        assert_eq!(
            rendering.pen_name.as_deref(),
            Some("com.samsung.android.sdk.pen.pen.preload.InkPen2")
        );
        assert_eq!(rendering.advanced_settings.as_deref(), Some("9;"));
        // The frame omits these fields. In pinned libSPenModel, ObjectStrokeImpl's
        // constructor zeros offsets 0x1d0 (GetStrokeType) and 0x180
        // (GetLineType), and the native getters read those fields directly.
        // Thus absent parsed style values mean the native zero defaults here.

        let profile = InkPen2RenderProfile {
            screen_width: 1920,
            screen_height: 1080,
            enhanced_antialias: false,
        };
        let prepared = prepare_stroke_for_page_with_theme(
            &stroke,
            RenderTheme::for_canvas(false),
            &RenderOptions {
                inkpen2_profile: Some(profile),
                chinesebrush_profile: None,
                ..RenderOptions::default()
            },
        );
        let ink = prepared
            .inkpen2_v9
            .expect("decoded style IDs admit explicit V9 profile");
        assert_eq!(ink.records.len(), 25);
        let actual: Vec<u32> = ink
            .records
            .iter()
            .flatten()
            .map(|value| value.to_bits())
            .collect();
        let expected: [u32; 225] = [
            0x3f7ffbf4, 0x00000000, 0x40145d17, 0x40145d17, 0x3e919191, 0x3f800000, 0x3d909091,
            0x3e50d0d1, 0x3eacacad, 0x3fe38c67, 0x00000000, 0x4015d174, 0x4015d174, 0x3e92a40a,
            0x3f800000, 0x3d909091, 0x3e50d0d1, 0x3eacacad, 0x4031c686, 0x00000000, 0x401745d1,
            0x401745d1, 0x3e93b13b, 0x3f800000, 0x3d909091, 0x3e50d0d1, 0x3eacacad, 0x407ffff4,
            0x00000000, 0x4018ba2e, 0x4018ba2e, 0x3e94b94b, 0x3f800000, 0x3d909091, 0x3e50d0d1,
            0x3eacacad, 0x409f73b0, 0x3cb97aa8, 0x401a2e8b, 0x401a2e8b, 0x3e95bc60, 0x3f800000,
            0x3d909091, 0x3e50d0d1, 0x3eacacad, 0x40bee760, 0x3db97aa8, 0x401a7cbd, 0x401a7cbd,
            0x3e95f22a, 0x3f800000, 0x3d909091, 0x3e50d0d1, 0x3eacacad, 0x40de5b10, 0x3e50a9fe,
            0x401acaef, 0x401acaef, 0x3e9627bd, 0x3f800000, 0x3d909091, 0x3e50d0d1, 0x3eacacad,
            0x40fdcec0, 0x3eb97aa8, 0x401b1921, 0x401b1921, 0x3e965d1a, 0x3f800000, 0x3d909091,
            0x3e50d0d1, 0x3eacacad, 0x410ea137, 0x3f10e7d2, 0x401b6753, 0x401b6753, 0x3e969241,
            0x3f800000, 0x3d909091, 0x3e50d0d1, 0x3eacacad, 0x411e5b10, 0x3f50a9fe, 0x401bb585,
            0x401bb585, 0x3e96c733, 0x3f800000, 0x3d909091, 0x3e50d0d1, 0x3eacacad, 0x412e14e7,
            0x3f8e01e8, 0x401c03b7, 0x401c03b7, 0x3e96fbf0, 0x3f800000, 0x3d909091, 0x3e50d0d1,
            0x3eacacad, 0x413dcebf, 0x3fb97aa8, 0x401c51e9, 0x401c51e9, 0x3e973078, 0x3f800000,
            0x3d909091, 0x3e50d0d1, 0x3eacacad, 0x414d889a, 0x3fe67454, 0x401ca01d, 0x401ca01d,
            0x3e9764cd, 0x3f800000, 0x3d909091, 0x3e50d0d1, 0x3eacacad, 0x415d4272, 0x4006df7c,
            0x401ca01d, 0x401ca01d, 0x3e9764cd, 0x3f800000, 0x3d909091, 0x3e50d0d1, 0x3eacacad,
            0x416cfc49, 0x40179ee3, 0x401ca01d, 0x401ca01d, 0x3e9764cd, 0x3f800000, 0x3d909091,
            0x3e50d0d1, 0x3eacacad, 0x417cb621, 0x40257860, 0x401ca01d, 0x401ca01d, 0x3e9764cd,
            0x3f800000, 0x3d909091, 0x3e50d0d1, 0x3eacacad, 0x418637fc, 0x40306bf2, 0x401ca01d,
            0x401ca01d, 0x3e9764cd, 0x3f800000, 0x3d909091, 0x3e50d0d1, 0x3eacacad, 0x418e14e8,
            0x4038799a, 0x401ca01d, 0x401ca01d, 0x3e9764cd, 0x3f800000, 0x3d909091, 0x3e50d0d1,
            0x3eacacad, 0x4195f1d4, 0x403da157, 0x401ca01d, 0x401ca01d, 0x3e9764cd, 0x3f800000,
            0x3d909091, 0x3e50d0d1, 0x3eacacad, 0x419dcec0, 0x403fe329, 0x401ca01d, 0x401ca01d,
            0x3e9764cd, 0x3f800000, 0x3d909091, 0x3e50d0d1, 0x3eacacad, 0x41a73ae6, 0x40400000,
            0x401ca01d, 0x401ca01d, 0x3e9764cd, 0x3f800000, 0x3d909091, 0x3e50d0d1, 0x3eacacad,
            0x41afb9bd, 0x40400000, 0x401ca01d, 0x401ca01d, 0x3e9764cd, 0x3f800000, 0x3d909091,
            0x3e50d0d1, 0x3eacacad, 0x41b67181, 0x40400000, 0x401ca01d, 0x401ca01d, 0x3e9764cd,
            0x3f800000, 0x3d909091, 0x3e50d0d1, 0x3eacacad, 0x41bf60c7, 0x40400000, 0x401ca01d,
            0x401ca01d, 0x3e9764cd, 0x3f800000, 0x3d909091, 0x3e50d0d1, 0x3eacacad, 0x41c00000,
            0x40400000, 0x401ca01d, 0x401ca01d, 0x3e9764cd, 0x3f800000, 0x3d909091, 0x3e50d0d1,
            0x3eacacad,
        ];
        assert_eq!(actual, expected, "all 225 native RTV3 float words");

        type StyleMutation = fn(&mut crate::StrokeStyle);
        let unsupported_types: [(&str, StyleMutation); 2] = [
            ("nonzero stroke type", |style| {
                style.stroke_type_raw = Some(1)
            }),
            ("nonzero line type", |style| style.line_type_raw = Some(1)),
        ];
        for (name, mutate) in unsupported_types {
            let mut unsupported = stroke.clone();
            mutate(&mut unsupported.rendering.as_mut().unwrap().style);
            assert!(
                prepare_stroke_for_page_with_theme(
                    &unsupported,
                    RenderTheme::for_canvas(false),
                    &RenderOptions {
                        inkpen2_profile: Some(profile),
                        chinesebrush_profile: None,
                        ..RenderOptions::default()
                    },
                )
                .inkpen2_v9
                .is_none(),
                "{name} must remain outside the captured V9 contract"
            );
        }
    }

    fn inkpen2_active_rainbow_hybrid_stroke() -> Stroke {
        // The 137-byte source prefix is the same borrowed type-0 source used
        // by inkpen2_serialized_style_mapping_reaches_native_v9_records. This
        // 126-byte style frame is the native NewApplyBinary/OnAttach active
        // type-1 rainbow hybrid (SHA-256 9521a7d4b155e59397788993f8d2796b1f43663d7d6d900a0f6d222be371083e), embedded here to keep the regression durable.
        let hex_bytes = |hex: &str| -> Vec<u8> {
            hex.as_bytes()
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect()
        };
        let mut payload = hex_bytes(
            "8900000000006900000002bf010400600400a00f0000240064313630613838652d623534642d313166312d613062392d396663373838333030663136f20edec9f25b06000000000001e55c40000000e0dbbc6940000000e05d9a604000000020769b6c400000000000be7635c6f25b060038070000350a000000000000000000000000000000000000",
        );
        payload.extend(hex_bytes(
            "7e00000001005a000000020514040f00c003040000000000000000000000000000000000000100800001600000010080cdcc4c3e330333033303000000000a000a000a00000000000000000000000000000000000000000002000000000001000000563412ff00008040050000000000204002000000ffffff0000ff0100",
        ));
        let mut stroke = crate::decode::decode_stroke(&payload, &crate::ParseLimits::default())
            .expect("parse native active-rainbow hybrid");
        crate::StrokeResources::new(&crate::NoteStringTable {
            entries: vec![
                crate::NoteStringId {
                    id: 0,
                    text: "com.samsung.android.sdk.pen.pen.preload.InkPen2".into(),
                },
                crate::NoteStringId {
                    id: 1,
                    text: "9;".into(),
                },
            ],
            trailing_data: Vec::new(),
        })
        .resolve(&mut stroke);
        stroke
    }

    #[cfg(feature = "pdf")]
    #[test]
    #[ignore = "writes public-dispatch experimental AA rainbow artifacts"]
    fn write_public_inkpen2_gradient_aa_artifacts() {
        use serde_json::{Value, json};
        use std::path::PathBuf;

        let output = PathBuf::from(
            std::env::var_os("SDOCX_INKPEN2_PUBLIC_AA_DIR")
                .expect("set SDOCX_INKPEN2_PUBLIC_AA_DIR to a fresh directory"),
        );
        std::fs::create_dir_all(&output).unwrap();
        assert!(
            std::fs::read_dir(&output).unwrap().next().is_none(),
            "output directory must be empty"
        );
        let captures = std::env::var_os("SDOCX_INKPEN2_CAPTURE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../..")
                    .join("tmp/vector-parity-plan/inkpen2-rainbow-paint/captures")
            });
        let profile = InkPen2RenderProfile {
            screen_width: 1920,
            screen_height: 1080,
            enhanced_antialias: true,
        };
        let options = RenderOptions {
            inkpen2_profile: Some(profile),
            ..RenderOptions::default()
        };
        let mut cases = Vec::new();
        let _guard = super::InkPen2ExperimentalAaGuard::enter();
        for scale in [1, 2, 4] {
            for alpha in [128_u8, 255] {
                let id = format!("scale{scale}_aa1_alpha{alpha}");
                let capture_bytes = std::fs::read(captures.join(format!("{id}.json"))).unwrap();
                let capture: Value = serde_json::from_slice(&capture_bytes).unwrap();
                let producer = &capture["joined_native_paint"]["producer"];
                let native = decode_hex(
                    producer["native_AddPoint_vector_bytes_hex"]
                        .as_str()
                        .unwrap(),
                );
                let expected = "570044bb059690f865cfa6f4f99c8ba7801573144242ee0913654973b0d0e987";
                assert_eq!(native.len(), 25 * 9 * 4);
                assert_eq!(sha256_hex(&native), expected);
                assert_eq!(producer["sha256"].as_str(), Some(expected));
                assert_eq!(
                    capture["input"]["color_argb_u32"].as_u64().unwrap() >> 24,
                    u64::from(alpha)
                );
                let stages =
                    capture["joined_native_paint"]["native_stage_readbacks_before_invalidate"]
                        .as_array()
                        .unwrap();
                assert_eq!(stages.len(), 3);
                let mut stroke = inkpen2_active_rainbow_hybrid_stroke();
                stroke.rendering.as_mut().unwrap().style.color_argb = Some(
                    u32::try_from(capture["input"]["color_argb_u32"].as_u64().unwrap()).unwrap(),
                );
                let prepared =
                    prepare_inkpen2_v9(&stroke, Some(profile), RenderTheme::for_canvas(false))
                        .unwrap();
                assert!(prepared.enhanced_antialias && prepared.per_stamp_rgb);
                assert_eq!(record_bytes(&prepared.records), native);
                let mut page = page_with_uncolored_stroke();
                page.width = 128;
                page.height = 96;
                page.objects = vec![stroke.clone().into()];
                let doc = document(page);
                let layout = layout_document(&doc);
                assert_eq!(layout.pages.len(), 1);
                for positive_chart in [false, true] {
                    let _chart_guard =
                        positive_chart.then(super::InkPen2PositiveYChartGuard::enter);
                    let chart_prepared =
                        prepare_inkpen2_v9(&stroke, Some(profile), RenderTheme::for_canvas(false))
                            .unwrap();
                    let expected_chart = if positive_chart {
                        inkpen2_reflected_chart(&prepared, 96.0)
                    } else {
                        prepared.clone()
                    };
                    assert_eq!(chart_prepared.records, expected_chart.records);
                    assert_eq!(chart_prepared.sample_ends, prepared.sample_ends);
                    assert_eq!(chart_prepared.bounds, expected_chart.bounds);
                    if positive_chart {
                        let s = f64::from(scale);
                        assert!(
                            inkpen2_chart_geometry_max_delta(
                                &prepared.records,
                                &chart_prepared.records,
                                [s, 0.0, 0.0, -s, 0.0, 96.0],
                                [s, 0.0, 0.0, s, 0.0, 96.0 - 96.0 * s],
                            ) <= 1e-4,
                            "{id}: chart changed physical stamp geometry"
                        );
                    }
                    let mut cache = super::DocumentTextCache::default();
                    let mut scene = cache
                        .render_layout_page_scenes(
                            &doc,
                            &[&layout.pages[0]],
                            &options,
                            &super::fonts::FontBook::default(),
                            &mut |_| {},
                        )
                        .remove(0);
                    assert_eq!(scene.ink.len(), 1, "public dispatch must retain native ink");
                    assert!(scene.text_error.is_none());
                    let (native_id, retained) = scene.ink.iter().next().unwrap();
                    assert_eq!(retained.bounds, chart_prepared.bounds);
                    match &retained.payload {
                        super::NativeInkPayload::Radial {
                            records,
                            enhanced_antialias,
                            ..
                        } => {
                            assert_eq!(records, &chart_prepared.records);
                            assert!(*enhanced_antialias);
                        }
                        _ => panic!("expected retained radial atlas"),
                    }
                    let source = &scene.page.svg;
                    assert!(
                        source.contains(&native_id.svg_id()),
                        "native atlas marker required"
                    );
                    assert!(
                        source.contains("<mask") && source.contains(r#"mask=""#),
                        "active mask required"
                    );
                    assert!(
                        source.contains("<filter") && source.contains(r#"filter=""#),
                        "active filter required"
                    );
                    // The public scene is in document coordinates. Apply the capture
                    // chart after the background, while keeping the sidecar intact;
                    // the PDF renderer associates its retained ink with the SVG node.
                    let open_end = source.find('>').unwrap() + 1;
                    let close_start = source.rfind("</svg>").unwrap();
                    let body = &source[open_end..close_start];
                    let (background, content) = if let Some(start) = body.find("<rect") {
                        let end = start + body[start..].find("/>").unwrap() + 2;
                        (&body[..end], &body[end..])
                    } else {
                        ("", body)
                    };
                    let transform = if positive_chart {
                        format!("matrix({scale},0,0,{scale},0,{})", 96 - 96 * scale)
                    } else {
                        format!("matrix({scale},0,0,-{scale},0,96)")
                    };
                    let svg = format!(
                        r#"{}{}<g transform="{transform}">{}</g></svg>"#,
                        &source[..open_end],
                        background,
                        content
                    );
                    scene.page.svg = svg.clone();
                    let pdf = crate::pdf::render_native_scene_pdf_for_test(
                        &scene,
                        &crate::PdfOptions::default(),
                    )
                    .unwrap();
                    let parsed = lopdf::Document::load_mem(&pdf).unwrap();
                    assert_eq!(parsed.get_pages().len(), 1);
                    assert!(
                        !parsed
                            .objects
                            .values()
                            .any(|object| object
                                .as_stream()
                                .is_ok_and(|stream| stream.dict.get(b"Subtype").is_ok_and(|v| v
                                    .as_name()
                                    .is_ok_and(|name| name == b"Image"))))
                    );
                    let artifact_id = if positive_chart {
                        format!("{id}_positive_y")
                    } else {
                        id.clone()
                    };
                    write_new_artifact(&output.join(format!("{artifact_id}.svg")), svg.as_bytes());
                    write_new_artifact(&output.join(format!("{artifact_id}.pdf")), &pdf);
                    cases.push(json!({
                    "case_id": artifact_id, "scale": scale, "alpha": alpha, "positive_y_chart": positive_chart,
                    "native_capture_sha256": sha256_hex(&capture_bytes), "native_vbo_sha256": expected,
                    "native_stage_sha256": stages.iter().map(|stage| stage["rgba_sha256"].as_str().unwrap()).collect::<Vec<_>>(),
                    "native_draw_matrix": capture["joined_native_paint"]["paint"]["readback"]["draw_matrix_f32_row_major"],
                    "comparison_transform": if positive_chart { [scale, 0, 0, scale, 0, 96 - 96 * scale] } else { [scale, 0, 0, -scale, 0, 96] },
                    "svg_sha256": sha256_hex(svg.as_bytes()), "pdf_sha256": sha256_hex(&pdf)
                }));
                }
            }
        }
        let manifest = json!({
            "schema": "sdocx-inkpen2-public-aa-gradient-v1", "admission_changed": false,
            "coordinate_note": "The original chart uses [scale, 0, 0, -scale, 0, 96]. The positive-Y chart reflects records before atlas and sidecar registration and uses [scale, 0, 0, scale, 0, 96-96*scale]. Compare 128x96 outputs after reversing native GL rows once. Parity is not established by this writer.",
            "cases": cases
        });
        write_new_artifact(
            &output.join("manifest.json"),
            &serde_json::to_vec_pretty(&manifest).unwrap(),
        );
    }

    #[test]
    fn inkpen2_active_rainbow_aa_off_uses_native_per_stamp_vbo() {
        use sha2::Digest;

        // Native active hybrid after NewApplyBinary/OnAttach and actual
        // Drawing-selected GLV9 redraw: 25 Vector9 records / 900 bytes.
        // The emitted VBO SHA-256 is 570044bb059690f865cfa6f4f99c8ba7801573144242ee0913654973b0d0e987.
        let stroke = inkpen2_active_rainbow_hybrid_stroke();
        let style = &stroke.rendering.as_ref().unwrap().style;
        assert_eq!(
            stroke.rendering.as_ref().unwrap().pen_name.as_deref(),
            Some("com.samsung.android.sdk.pen.pen.preload.InkPen2")
        );
        assert_eq!(
            stroke
                .rendering
                .as_ref()
                .unwrap()
                .advanced_settings
                .as_deref(),
            Some("9;")
        );
        assert!(stroke.rendering.as_ref().unwrap().properties.rainbow_effect);
        assert_eq!(style.color_type_raw, Some(1));
        assert_eq!(style.rainbow_distance, Some(5));
        assert_eq!(style.rainbow_offset, Some(2.5));
        assert_eq!(
            style.gradient_colors_argb.as_deref(),
            Some(&[0xffff_0000, 0xff00_00ff][..])
        );
        assert_eq!(style.color_argb, Some(0xff12_3456));
        assert_eq!(stroke.points.len(), 4);

        let profile = InkPen2RenderProfile {
            screen_width: 1920,
            screen_height: 1080,
            enhanced_antialias: false,
        };
        let prepared = prepare_inkpen2_v9(&stroke, Some(profile), RenderTheme::for_canvas(false))
            .expect("active native type-1 AA-off profile is supported");
        assert!(prepared.per_stamp_rgb);
        assert!(!prepared.enhanced_antialias);
        assert_eq!(prepared.records.len(), 25);
        assert!(prepared.records.iter().all(|record| record[5] == 1.0));
        let bytes: Vec<u8> = prepared
            .records
            .iter()
            .flat_map(|record| record.iter().flat_map(|value| value.to_le_bytes()))
            .collect();
        assert_eq!(bytes.len(), 900);
        assert_eq!(
            format!("{:x}", sha2::Sha256::digest(&bytes)),
            "570044bb059690f865cfa6f4f99c8ba7801573144242ee0913654973b0d0e987"
        );

        // Source ARGB alpha affects final stroke opacity only; palette RGB
        // remains native data and is never changed by RenderTheme.
        for alpha in [0_u8, 128, 255] {
            let mut variant = stroke.clone();
            variant.rendering.as_mut().unwrap().style.color_argb =
                Some((u32::from(alpha) << 24) | 0x0012_3456);
            let light = prepare_inkpen2_v9(&variant, Some(profile), RenderTheme::for_canvas(false))
                .unwrap();
            let dark =
                prepare_inkpen2_v9(&variant, Some(profile), RenderTheme::for_canvas(true)).unwrap();
            assert_eq!(light.records, prepared.records);
            assert_eq!(dark.records, prepared.records);
            assert_eq!(light.color, dark.color);
            assert_eq!(light.opacity, f64::from(f32::from(alpha) / 255.0));
        }

        // Exercise public dispatch from the parsed source, including PDF transport.
        let mut source = stroke.clone();
        source.bbox = BoundingBox {
            x_min: 0.,
            y_min: 0.,
            x_max: 24.,
            y_max: 3.,
        };
        let mut page = page_with_uncolored_stroke();
        page.width = 128;
        page.height = 96;
        page.objects = vec![source.into()];
        let doc = document(page);
        let options = RenderOptions {
            inkpen2_profile: Some(profile),
            chinesebrush_profile: None,
            ..RenderOptions::default()
        };
        let svg = render_document_svg(&doc, &options).remove(0).svg;
        assert_eq!(svg.matches("<radialGradient").count(), 25);
        assert!(!svg.contains("<mask"));
        assert!(!svg.contains("<filter"));

        #[cfg(feature = "pdf")]
        {
            let bytes =
                crate::render_document_pdf(&doc, &options, &crate::PdfOptions::default()).unwrap();
            let pdf = lopdf::Document::load_mem(&bytes).unwrap();
            let images = pdf
                .objects
                .values()
                .filter_map(pdf_object_dictionary)
                .filter(|dict| {
                    dict.get(b"Subtype")
                        .and_then(lopdf::Object::as_name)
                        .is_ok_and(|name| name == b"Image")
                })
                .count();
            assert_eq!(
                images, 0,
                "active AA-off gradients retain vector PDF shading"
            );
        }
    }

    #[test]
    fn inkpen2_out_of_range_rainbow_uses_whole_stroke_fallback() {
        // The native color callback saturates its quotient for this finite
        // offset and can emit RGB far outside [0, 1]. Those values cannot be
        // represented by the admitted SVG and PDF per-stamp paint routes.
        let mut stroke = inkpen2_active_rainbow_hybrid_stroke();
        stroke.rendering.as_mut().unwrap().style.rainbow_offset = Some(1.0e20);
        let profile = InkPen2RenderProfile {
            screen_width: 1920,
            screen_height: 1080,
            enhanced_antialias: false,
        };
        assert!(
            prepare_inkpen2_v9(&stroke, Some(profile), RenderTheme::for_canvas(false)).is_none()
        );

        stroke.bbox = BoundingBox {
            x_min: 0.,
            y_min: 0.,
            x_max: 24.,
            y_max: 3.,
        };
        let mut page = page_with_uncolored_stroke();
        page.width = 128;
        page.height = 96;
        page.objects = vec![stroke.into()];
        let doc = document(page);
        let options = RenderOptions {
            inkpen2_profile: Some(profile),
            chinesebrush_profile: None,
            ..RenderOptions::default()
        };
        let fallback = RenderOptions {
            inkpen2_profile: None,
            ..options.clone()
        };
        assert_eq!(
            render_document_svg(&doc, &options).remove(0).svg,
            render_document_svg(&doc, &fallback).remove(0).svg
        );
        #[cfg(feature = "pdf")]
        crate::render_document_pdf(&doc, &options, &crate::PdfOptions::default())
            .expect("unsupported rainbow RGB must fall back before PDF registration");
    }

    #[test]
    fn inkpen2_active_rainbow_admission_fails_closed_outside_aa0_contract() {
        type Mutation = fn(&mut Stroke);
        let mutations: [(&str, Mutation); 17] = [
            ("color type 0", |s| {
                s.rendering.as_mut().unwrap().style.color_type_raw = Some(0)
            }),
            ("color type 2", |s| {
                s.rendering.as_mut().unwrap().style.color_type_raw = Some(2)
            }),
            ("color type unknown", |s| {
                s.rendering.as_mut().unwrap().style.color_type_raw = Some(3)
            }),
            ("color type absent", |s| {
                s.rendering.as_mut().unwrap().style.color_type_raw = None
            }),
            ("distance absent", |s| {
                s.rendering.as_mut().unwrap().style.rainbow_distance = None
            }),
            ("distance zero", |s| {
                s.rendering.as_mut().unwrap().style.rainbow_distance = Some(0)
            }),
            ("distance negative", |s| {
                s.rendering.as_mut().unwrap().style.rainbow_distance = Some(-1)
            }),
            ("offset absent", |s| {
                s.rendering.as_mut().unwrap().style.rainbow_offset = None
            }),
            ("offset negative", |s| {
                s.rendering.as_mut().unwrap().style.rainbow_offset = Some(-0.25)
            }),
            ("offset nonfinite", |s| {
                s.rendering.as_mut().unwrap().style.rainbow_offset = Some(f32::NAN)
            }),
            ("palette absent", |s| {
                s.rendering.as_mut().unwrap().style.gradient_colors_argb = None
            }),
            ("palette empty", |s| {
                s.rendering.as_mut().unwrap().style.gradient_colors_argb = Some(vec![])
            }),
            ("palette singleton", |s| {
                s.rendering.as_mut().unwrap().style.gradient_colors_argb = Some(vec![0xffff_0000])
            }),
            ("palette exceeds bound", |s| {
                s.rendering.as_mut().unwrap().style.gradient_colors_argb =
                    Some(vec![0xffff_0000; 257])
            }),
            ("source ARGB absent", |s| {
                s.rendering.as_mut().unwrap().style.color_argb = None
            }),
            ("unknown style tail", |s| {
                s.rendering.as_mut().unwrap().style.first_unparsed_field = Some(26)
            }),
            ("style trailing bytes", |s| {
                s.rendering.as_mut().unwrap().style.trailing_data = vec![1]
            }),
        ];
        let profile = InkPen2RenderProfile {
            screen_width: 1920,
            screen_height: 1080,
            enhanced_antialias: false,
        };
        for (label, mutate) in mutations {
            let mut stroke = inkpen2_active_rainbow_hybrid_stroke();
            mutate(&mut stroke);
            assert!(
                prepare_inkpen2_v9(&stroke, Some(profile), RenderTheme::for_canvas(false))
                    .is_none(),
                "{label} must be rejected"
            );
        }

        let mut stroke = inkpen2_active_rainbow_hybrid_stroke();
        let mut inactive = stroke.clone();
        inactive
            .rendering
            .as_mut()
            .unwrap()
            .properties
            .rainbow_effect = false;
        let solid = prepare_inkpen2_v9(&inactive, Some(profile), RenderTheme::for_canvas(false))
            .expect("inactive type-1 palette retains the existing solid route");
        assert!(!solid.per_stamp_rgb);
        let aa = InkPen2RenderProfile {
            enhanced_antialias: true,
            ..profile
        };
        assert!(prepare_inkpen2_v9(&stroke, Some(aa), RenderTheme::for_canvas(false)).is_none());
        let wrong_resolution = InkPen2RenderProfile {
            screen_width: 2560,
            screen_height: 1600,
            ..profile
        };
        assert!(
            prepare_inkpen2_v9(
                &stroke,
                Some(wrong_resolution),
                RenderTheme::for_canvas(false)
            )
            .is_none()
        );
        assert!(prepare_inkpen2_v9(&stroke, None, RenderTheme::for_canvas(false)).is_none());

        // Thin radii produce coverage below one and are excluded from the
        // measured direct vector/PDF contract.
        stroke.rendering.as_mut().unwrap().style.pen_size = Some(0.4);
        stroke.pen_width = 0.4;
        assert!(
            prepare_inkpen2_v9(&stroke, Some(profile), RenderTheme::for_canvas(false)).is_none()
        );

        let mut long = inkpen2_active_rainbow_hybrid_stroke();
        long.points = vec![Point { x: 0., y: 0. }, Point { x: 600., y: 0. }];
        long.timestamps = vec![0, 10];
        long.pressures = vec![0.5; 2];
        long.tilts = vec![0.; 2];
        long.orientations = vec![0.; 2];
        assert!(prepare_inkpen2_v9(&long, Some(profile), RenderTheme::for_canvas(false)).is_none());
    }

    #[test]
    fn inkpen2_inactive_gradient_settings_preserve_native_solid_records() {
        // Native Drawing + saved V9 redraw: type 0 and rainbow=false retain
        // the entire 25-record stream with a populated palette, distance 600,
        // or offset .25. Final native payload SHA-256:
        // f0a840909b835207aaa1add58c5379c1c9ea804985276975844811081025d110.
        let profile = InkPen2RenderProfile {
            screen_width: 1920,
            screen_height: 1080,
            enhanced_antialias: false,
        };
        let mut stroke = inkpen2_color_stroke(0xff336699, None);
        stroke
            .points
            .extend([Point { x: 16., y: 3. }, Point { x: 24., y: 3. }]);
        stroke.timestamps = vec![0, 10, 20, 30];
        stroke.pressures = vec![0.2, 0.4, 0.6, 0.8];
        stroke.tilts = vec![0.; 4];
        stroke.orientations = vec![0.; 4];
        let theme = RenderTheme::for_canvas(false);
        let baseline = prepare_inkpen2_v9(&stroke, Some(profile), theme).unwrap();
        assert_eq!(baseline.records.len(), 25);
        let words: Vec<u8> = baseline
            .records
            .iter()
            .flatten()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        use sha2::Digest;
        assert_eq!(
            format!("{:x}", sha2::Sha256::digest(words)),
            "f0a840909b835207aaa1add58c5379c1c9ea804985276975844811081025d110"
        );
        for (distance, offset) in [(0, 0.), (600, 0.), (0, 0.25)] {
            let style = &mut stroke.rendering.as_mut().unwrap().style;
            style.color_type_raw = Some(0);
            style.gradient_colors_argb = Some(vec![0xff336699, 0xffcc8844]);
            style.rainbow_distance = Some(distance);
            style.rainbow_offset = Some(offset);
            let actual = prepare_inkpen2_v9(&stroke, Some(profile), theme).unwrap();
            assert_eq!(actual.records, baseline.records);
            assert_eq!(actual.sample_ends, baseline.sample_ends);
            assert_eq!(actual.color, baseline.color);
            assert_eq!(actual.opacity, baseline.opacity);
        }
    }

    #[test]
    fn inkpen2_constant_rgb_profile_rejects_active_and_unknown_color_modes() {
        let options = RenderOptions {
            inkpen2_profile: Some(InkPen2RenderProfile {
                screen_width: 1920,
                screen_height: 1080,
                enhanced_antialias: false,
            }),
            ..RenderOptions::default()
        };
        let mut stroke = inkpen2_color_stroke(0xff336699, None);
        for (color_type, rainbow) in [
            (0, true),
            (1, true),
            (2, true),
            (3, false),
            (u16::MAX, false),
        ] {
            // Enabled effects and unknown native modes do not inherit the
            // separately verified inactive-effect contract.
            stroke.rendering.as_mut().unwrap().style.color_type_raw = Some(color_type);
            stroke.rendering.as_mut().unwrap().properties.rainbow_effect = rainbow;
            let prepared = prepare_stroke_for_page_with_theme(
                &stroke,
                RenderTheme::for_canvas(false),
                &options,
            );
            assert!(
                prepared.inkpen2_v9.is_none(),
                "native color mode {color_type}"
            );
        }
    }

    #[test]
    fn inkpen2_inactive_effect_modes_match_native_solid_geometry_and_paint_input() {
        // The actual InkPen2 Pen::GetPatternable returns null (PenCommon
        // RVA 0x46480). Drawing skips pattern setup, and native type 0/2
        // controls produce this same VBO and full RGBA readback.
        // Native type-2 capture SHA-256:
        // 81c2930aff4c3c3ed1271d434c607f1e193fa43d03f49185bde50fd2d9319bdb.
        // Type 1 forwards red/blue palette but disables rainbow; capture:
        // f98563790d775b882098e36bc83aa612167a6eb0f8f6c9bc350725420307a376.
        let mut stroke = inkpen2_color_stroke(0xff336699, None);
        stroke
            .points
            .extend([Point { x: 16., y: 3. }, Point { x: 24., y: 3. }]);
        stroke.timestamps = vec![0, 10, 20, 30];
        stroke.pressures = vec![0.2, 0.4, 0.6, 0.8];
        stroke.tilts = vec![0.; 4];
        stroke.orientations = vec![0.; 4];
        let theme = RenderTheme::for_canvas(false);
        let profile = InkPen2RenderProfile {
            screen_width: 1440,
            screen_height: 2560,
            enhanced_antialias: false,
        };
        let plain = prepare_inkpen2_v9(&stroke, Some(profile), theme).unwrap();
        use sha2::Digest;
        for (color_type, index, scale) in [(1, 1, 2.), (2, 7, 1.75)] {
            let style = &mut stroke.rendering.as_mut().unwrap().style;
            style.color_type_raw = Some(color_type);
            style.pattern_index = Some(index);
            style.pattern_scale = Some(scale);
            style.gradient_colors_argb = Some(vec![0xffff0000, 0xff0000ff]);
            let prepared = prepare_inkpen2_v9(&stroke, Some(profile), theme).unwrap();
            assert_eq!(prepared.records, plain.records);
            assert_eq!(prepared.sample_ends, plain.sample_ends);
            assert_eq!(prepared.color, plain.color);
            assert_eq!(prepared.opacity, plain.opacity);
            let payload: Vec<u8> = prepared
                .records
                .iter()
                .flatten()
                .flat_map(|value| value.to_le_bytes())
                .collect();
            assert_eq!(
                format!("{:x}", sha2::Sha256::digest(payload)),
                "80ead21b09828b444750eda94ed3bbf2fb7c5628fcce575afc4cb9bfd543e436"
            );
        }
    }

    #[test]
    fn inkpen2_profile_rejects_output_amplification_at_the_generation_budget() {
        let profile = InkPen2RenderProfile {
            screen_width: 1920,
            screen_height: 1080,
            enhanced_antialias: false,
        };
        let mut stroke = inkpen2_color_stroke(0xff000000, None);
        stroke.points = vec![
            Point { x: 0.0, y: 0.0 },
            Point {
                x: 40_000.0,
                y: 0.0,
            },
        ];
        stroke.timestamps = vec![0, 10];
        stroke.pressures = vec![0.2, 0.8];
        stroke.tilts = vec![0.0; 2];
        stroke.orientations = vec![0.0; 2];

        assert!(
            prepare_inkpen2_v9(&stroke, Some(profile), RenderTheme::for_canvas(false)).is_none()
        );
    }

    #[test]
    fn inkpen2_profile_adapts_rgb_by_theme_but_preserves_native_records_and_alpha() {
        let profile = InkPen2RenderProfile {
            screen_width: 1920,
            screen_height: 1080,
            enhanced_antialias: false,
        };
        let black = inkpen2_color_stroke(0xff00_0000, None);
        let light =
            prepare_inkpen2_v9(&black, Some(profile), RenderTheme::for_canvas(false)).unwrap();
        let dark =
            prepare_inkpen2_v9(&black, Some(profile), RenderTheme::for_canvas(true)).unwrap();
        assert_eq!(light.color, Color { r: 0, g: 0, b: 0 });
        assert_eq!(
            dark.color,
            Color {
                r: 255,
                g: 255,
                b: 255
            }
        );
        assert_eq!(light.records, dark.records);
        assert!(light.records.iter().all(|record| record[6..9] == [0.0; 3]));

        // Auto on a page without stored background resolves to the light canvas.
        let page = page_with_uncolored_stroke();
        let auto = RenderTheme::resolve(&page, &DocumentMetadata::default(), RenderColorMode::Auto);
        assert_eq!(
            prepare_inkpen2_v9(&black, Some(profile), auto)
                .unwrap()
                .color,
            light.color
        );
        for (mode, expected) in [
            (RenderColorMode::Light, "stop-color=\"#000000\""),
            (RenderColorMode::Dark, "stop-color=\"#ffffff\""),
            (RenderColorMode::Auto, "stop-color=\"#000000\""),
        ] {
            let mut page = page_with_uncolored_stroke();
            page.objects = vec![black.clone().into()];
            let options = RenderOptions {
                inkpen2_profile: Some(profile),
                chinesebrush_profile: None,
                color_mode: mode,
                ..RenderOptions::default()
            };
            let svg = render_document_svg(&document(page), &options)[0]
                .svg
                .clone();
            assert!(svg.contains(expected), "{mode:?}: {expected}");
        }

        // A stored page background plus compatibility=false suppresses the
        // normal dark-canvas foreground adaptation. The same resolved theme
        // must drive both page SVG and replay metadata.
        let mut page = page_with_uncolored_stroke();
        page.background_color = Some(Color {
            r: 16,
            g: 16,
            b: 16,
        });
        page.objects = vec![black.clone().into()];
        let metadata = DocumentMetadata {
            dark_mode_compatibility: Some(false),
            ..DocumentMetadata::default()
        };
        let resolved = RenderTheme::resolve(&page, &metadata, RenderColorMode::Dark);
        let options = RenderOptions {
            inkpen2_profile: Some(profile),
            chinesebrush_profile: None,
            color_mode: RenderColorMode::Dark,
            ..RenderOptions::default()
        };
        let prepared = prepare_stroke_for_page_with_theme(&black, resolved, &options);
        assert_eq!(prepared.color, "#000000");
        let page_svg = render_document_svg(
            &Document {
                pages: vec![page],
                metadata,
            },
            &options,
        )[0]
        .svg
        .clone();
        assert!(page_svg.contains("stop-color=\"#000000\""));
        assert!(!page_svg.contains("stop-color=\"#ffffff\""));

        let argb = 0x80_336699;
        let override_color = Color {
            r: 10,
            g: 30,
            b: 50,
        };
        let partial = inkpen2_color_stroke(argb, Some(override_color));
        let themed =
            prepare_inkpen2_v9(&partial, Some(profile), RenderTheme::for_canvas(false)).unwrap();
        assert_eq!(themed.color, override_color);
        assert_eq!(themed.opacity, f64::from(128.0_f32 / 255.0_f32));
        let raw = [(51.0_f32 / 255.0), (102.0_f32 / 255.0), (153.0_f32 / 255.0)];
        assert!(themed.records.iter().all(|record| record[6..9] == raw));

        let options = RenderOptions {
            inkpen2_profile: Some(profile),
            chinesebrush_profile: None,
            ..RenderOptions::default()
        };
        let prepared_light =
            prepare_stroke_for_page_with_theme(&partial, RenderTheme::for_canvas(false), &options);
        let prepared_dark =
            prepare_stroke_for_page_with_theme(&partial, RenderTheme::for_canvas(true), &options);
        assert_eq!(prepared_light.color, "#0a1e32");
        assert_eq!(prepared_light.opacity, f64::from(128.0_f32 / 255.0_f32));
        assert_eq!(prepared_dark.color, "#cde1f5");
        assert_eq!(
            prepared_dark.inkpen2_v9.as_ref().unwrap().records,
            themed.records
        );

        let mut page = page_with_uncolored_stroke();
        page.objects = vec![partial.into()];
        let mut options = RenderOptions {
            inkpen2_profile: Some(profile),
            chinesebrush_profile: None,
            ..RenderOptions::default()
        };
        options.color_mode = RenderColorMode::Light;
        let svg = render_document_svg(&document(page), &options)[0]
            .svg
            .clone();
        assert!(svg.contains("stop-color=\"#0a1e32\""));
        assert!(svg.contains(&format!(
            "stop-opacity=\"{}\"",
            f64::from(128.0_f32 / 255.0_f32)
        )));
    }

    #[test]
    fn inkpen2_profile_metadata_cutoffs_match_sequential_replay_parts() {
        let profile = InkPen2RenderProfile {
            screen_width: 1920,
            screen_height: 1080,
            enhanced_antialias: false,
        };
        let mut stroke = inkpen2_color_stroke(0x80_336699, None);
        // Duplicate samples exercise equal adjacent event cutoffs; the terminal
        // suffix belongs to the final source event.
        stroke.points = vec![
            Point { x: 0.0, y: 0.0 },
            Point { x: 8.0, y: 0.0 },
            Point { x: 8.0, y: 0.0 },
            Point { x: 16.0, y: 1.0 },
        ];
        stroke.timestamps = vec![100, 110, 120, 130];
        stroke.pressures = vec![0.2, 0.3, 0.4, 0.5];
        stroke.tilts = vec![0.0; 4];
        stroke.orientations = vec![0.0; 4];
        let page = Page {
            objects: vec![stroke.clone().into()],
            ..page_with_uncolored_stroke()
        };
        let document = document(page);
        let options = RenderOptions {
            inkpen2_profile: Some(profile),
            chinesebrush_profile: None,
            ..RenderOptions::default()
        };
        let theme =
            RenderTheme::resolve(&document.pages[0], &document.metadata, options.color_mode);
        let prepared = prepare_stroke_for_page_with_theme(&stroke, theme, &options);
        let native = prepared.inkpen2_v9.as_ref().expect("profiled V9 metadata");
        assert_eq!(
            prepared.sample_ends.as_deref(),
            Some(native.sample_ends.as_slice())
        );
        assert_eq!(native.sample_ends.len(), stroke.points.len());
        assert_eq!(native.sample_ends.last(), Some(&native.records.len()));
        let all_parts: Vec<usize> = (1..=native.records.len()).collect();
        for &cutoff in &native.sample_ends {
            let visible: Vec<_> = all_parts
                .iter()
                .copied()
                .filter(|part| *part <= cutoff)
                .collect();
            assert_eq!(visible.len(), cutoff);
            assert!(
                visible
                    .iter()
                    .enumerate()
                    .all(|(index, part)| *part == index + 1)
            );
        }
        assert_eq!(
            stroke.points[1], stroke.points[2],
            "fixture retains duplicate source event"
        );

        let replay =
            render_layout_page_replay_svg(&document, &layout_document(&document), 0, &options)
                .unwrap()
                .svg;
        let actual_parts: Vec<usize> = roxmltree::Document::parse(&replay)
            .unwrap()
            .descendants()
            .filter_map(|node| node.attribute("data-replay-part"))
            .map(|part| part.parse().unwrap())
            .collect();
        assert_eq!(actual_parts, all_parts);

        let enhanced_options = RenderOptions {
            inkpen2_profile: Some(InkPen2RenderProfile {
                enhanced_antialias: true,
                ..profile
            }),
            ..RenderOptions::default()
        };
        let enhanced_replay = render_layout_page_replay_svg(
            &document,
            &layout_document(&document),
            0,
            &enhanced_options,
        )
        .unwrap()
        .svg;
        let enhanced_parts: Vec<usize> = roxmltree::Document::parse(&enhanced_replay)
            .unwrap()
            .descendants()
            .filter_map(|node| node.attribute("data-replay-part"))
            .map(|part| part.parse().unwrap())
            .collect();
        assert_eq!(enhanced_parts, all_parts, "AA keeps native source cutoffs");

        let legacy = prepare_stroke_for_page_with_theme(&stroke, theme, &RenderOptions::default());
        assert!(
            legacy.inkpen2_v9.is_none(),
            "no-profile preparation retains fallback"
        );
        assert!(
            !render_document_svg(&document, &RenderOptions::default())[0]
                .svg
                .contains("<radialGradient")
        );
    }

    #[test]
    fn normalizes_and_clamps_stroke_widths() {
        assert_eq!(normalized_stroke_width(f32::NAN), 1.0);
        assert_eq!(normalized_stroke_width(f32::INFINITY), 1.0);
        assert_eq!(normalized_stroke_width(0.0), 1.0);
        assert_eq!(normalized_stroke_width(-1.0), 1.0);
        assert_eq!(normalized_stroke_width(0.1), 0.4);
        assert_eq!(normalized_stroke_width(10_000.0), 12.0);
        assert_eq!(normalized_stroke_width(5.0), 2.0);
    }

    #[test]
    fn uses_native_document_density_for_text_units() {
        assert_eq!(
            super::TextSettings {
                scale: 3.0,
                font_size_delta: 0.0,
                ..Default::default()
            }
            .font_size(15.0),
            45.0
        );
    }

    #[test]
    fn detects_the_active_theme_from_canvas_color() {
        assert!(!is_dark_background(Color {
            r: 0xfc,
            g: 0xfc,
            b: 0xfc
        }));
        assert!(is_dark_background(Color {
            r: 0x25,
            g: 0x25,
            b: 0x25
        }));
    }

    #[test]
    fn renders_page_dimensions_background_and_source_index() {
        let mut page = page_with_uncolored_stroke();
        page.width = 1080;
        page.height = 1527;
        page.background_color = Some(Color {
            r: 0xcb,
            g: 0xda,
            b: 0xdd,
        });
        let pages = render_document_svg(&document(page), &RenderOptions::default());

        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].source_page_index, 0);
        assert_eq!((pages[0].width, pages[0].height), (1080, 1527));
        assert!(pages[0].svg.contains(r#"viewBox="0.0 0.0 1080.0 1527.0""#));
        assert!(pages[0].svg.contains(r##"fill="#cbdadd""##));
    }

    #[test]
    fn renders_a_page_from_a_precomputed_layout() {
        let document = document(page_with_uncolored_stroke());
        let layout = layout_document(&document);
        let rendered =
            render_layout_page_svg(&document, &layout, 0, &RenderOptions::default()).unwrap();

        assert_eq!(rendered.source_page_index, 0);
        assert!(rendered.svg.contains(r#"viewBox="0.0 0.0 100.0 100.0""#));
        assert!(render_layout_page_svg(&document, &layout, 1, &RenderOptions::default()).is_none());
    }

    #[cfg(feature = "serde")]
    fn fountain_color_stroke(advanced_setting: &str) -> Stroke {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../../../conformance/fountain-v14.json")).unwrap();
        let mut stroke: Stroke = serde_json::from_value(reference["stroke"].clone()).unwrap();
        stroke.points = vec![
            Point { x: 10., y: 10. },
            Point { x: 30., y: 20. },
            Point { x: 40., y: 40. },
        ];
        stroke.pressures = vec![0.2, 0.7, 0.4];
        stroke.timestamps = vec![0, 10, 20];
        stroke.tilts = vec![0.4, 0.6, 0.5];
        stroke.color = None;
        let rendering = stroke.rendering.as_mut().unwrap();
        rendering.advanced_settings = Some(advanced_setting.into());
        rendering.style.color_argb = None;
        rendering.style.color_type_raw = Some(0);
        rendering.style.pen_size = Some(2.5);
        rendering.style.initial_tolerance = (advanced_setting == "14;").then_some(0.0);
        stroke.pen_width = 2.5;
        stroke
    }

    #[cfg(feature = "serde")]
    #[test]
    fn public_fountain_v14_scale_profile_keeps_svg_and_pdf_vector_output() {
        let mut page = page_with_uncolored_stroke();
        page.width = 64;
        page.height = 64;
        page.objects = vec![fountain_color_stroke("14;").into()];
        let document = document(page);
        let canonical = render_document_svg(&document, &RenderOptions::default())
            .remove(0)
            .svg;
        let options = RenderOptions {
            fountain_v14_output_scale_profile: Some(FountainV14OutputScaleProfile {
                pixels_per_page_unit: 2.0,
            }),
            ..RenderOptions::default()
        };
        let scaled = render_document_svg(&document, &options).remove(0).svg;
        assert_ne!(scaled, canonical);
        assert!(scaled.contains("<mask"));
        assert!(scaled.contains("radialGradient"));
        assert!(!scaled.contains("<image"));
        let invalid = RenderOptions {
            fountain_v14_output_scale_profile: Some(FountainV14OutputScaleProfile {
                pixels_per_page_unit: f32::NAN,
            }),
            ..RenderOptions::default()
        };
        assert_eq!(
            render_document_svg(&document, &invalid).remove(0).svg,
            canonical
        );
        #[cfg(feature = "pdf")]
        {
            let bytes =
                crate::render_document_pdf(&document, &options, &crate::PdfOptions::default())
                    .unwrap();
            let pdf = lopdf::Document::load_mem(&bytes).unwrap();
            let images = pdf
                .objects
                .values()
                .filter_map(pdf_object_dictionary)
                .filter(|dict| {
                    dict.get(b"Subtype")
                        .and_then(lopdf::Object::as_name)
                        .is_ok_and(|name| name == b"Image")
                })
                .count();
            assert_eq!(images, 0, "the V14 scale profile remains vector PDF paint");
        }
    }

    #[cfg(feature = "serde")]
    #[test]
    fn public_fountain_v16_scale_profile_is_explicit_and_v16_only() {
        let mut page = page_with_uncolored_stroke();
        page.width = 64;
        page.height = 64;
        page.objects = vec![fountain_color_stroke("18;0;100;").into()];
        let note = document(page);
        let canonical = render_document_svg(&note, &RenderOptions::default())
            .remove(0)
            .svg;
        let unit = RenderOptions {
            fountain_v16_output_scale_profile: Some(FountainV16OutputScaleProfile {
                pixels_per_page_unit: 1.0,
            }),
            ..RenderOptions::default()
        };
        assert_eq!(
            render_document_svg(&note, &unit).remove(0).svg,
            canonical,
            "explicit unit scale preserves the default SVG bytes"
        );
        let scaled = RenderOptions {
            fountain_v16_output_scale_profile: Some(FountainV16OutputScaleProfile {
                pixels_per_page_unit: 2.0,
            }),
            ..RenderOptions::default()
        };
        let scaled_svg = render_document_svg(&note, &scaled).remove(0).svg;
        assert_ne!(scaled_svg, canonical);
        assert!(!scaled_svg.contains("<image"));
        let invalid = RenderOptions {
            fountain_v16_output_scale_profile: Some(FountainV16OutputScaleProfile {
                pixels_per_page_unit: f32::NAN,
            }),
            ..RenderOptions::default()
        };
        assert_eq!(
            render_document_svg(&note, &invalid).remove(0).svg,
            canonical,
            "invalid scale falls back for the whole stroke"
        );
        let over_budget = RenderOptions {
            fountain_v16_output_scale_profile: Some(FountainV16OutputScaleProfile {
                pixels_per_page_unit: 20_000.0,
            }),
            ..RenderOptions::default()
        };
        assert_eq!(
            render_document_svg(&note, &over_budget).remove(0).svg,
            canonical,
            "an over-budget scale preserves canonical whole-stroke rendering"
        );

        let mut rainbow_stroke = fountain_color_stroke("18;0;100;");
        let rendering = rainbow_stroke.rendering.as_mut().unwrap();
        rendering.properties.rainbow_effect = true;
        rendering.properties.fixed_width = false;
        rendering.properties.stylus_channels = true;
        rendering.tool_type_raw = 2;
        rendering.style.rainbow_distance = Some(37);
        rendering.style.rainbow_offset = Some(2.75);
        rendering.style.gradient_colors_argb = Some(vec![0xffff_0000, 0xff00_ff00, 0xff00_00ff]);
        let mut rainbow_page = page_with_uncolored_stroke();
        rainbow_page.width = 64;
        rainbow_page.height = 64;
        rainbow_page.objects = vec![rainbow_stroke.into()];
        let rainbow_note = document(rainbow_page);
        let rainbow_canonical = render_document_svg(&rainbow_note, &RenderOptions::default())
            .remove(0)
            .svg;
        let rainbow_unit = render_document_svg(&rainbow_note, &unit).remove(0).svg;
        assert_eq!(rainbow_unit, rainbow_canonical);
        let rainbow_scaled = render_document_svg(&rainbow_note, &scaled).remove(0).svg;
        assert_ne!(rainbow_scaled, rainbow_canonical);
        assert!(!rainbow_scaled.contains("<image"));
        assert_eq!(
            render_document_svg(&rainbow_note, &over_budget)
                .remove(0)
                .svg,
            rainbow_canonical,
            "over-budget flag-ignored V16 keeps its canonical sampler"
        );

        let mut v14_page = page_with_uncolored_stroke();
        v14_page.width = 64;
        v14_page.height = 64;
        v14_page.objects = vec![fountain_color_stroke("14;").into()];
        let v14 = document(v14_page);
        assert_eq!(
            render_document_svg(&v14, &scaled).remove(0).svg,
            render_document_svg(&v14, &RenderOptions::default())
                .remove(0)
                .svg,
            "V16 output scale does not affect saved V14 strokes"
        );
    }

    #[cfg(feature = "serde")]
    #[test]
    fn public_inkpen_alpha_is_applied_once_in_normal_and_replay_svg() {
        for setting in [None, Some(""), Some("4;"), Some("18;0;100;")] {
            let mut stroke = fountain_color_stroke("14;");
            let rendering = stroke.rendering.as_mut().unwrap();
            rendering.pen_name = Some("com.samsung.android.sdk.pen.pen.preload.InkPen".into());
            rendering.advanced_settings = setting.map(str::to_owned);
            rendering.tool_type_raw = 1;
            rendering.style.stroke_type_raw = Some(0);
            rendering.style.color_argb = Some(0x8033_6699);
            let mut page = page_with_uncolored_stroke();
            page.objects = vec![stroke.into()];
            let document = document(page);
            let normal = render_document_svg(&document, &RenderOptions::default())
                .remove(0)
                .svg;
            let layout = layout_document(&document);
            let replay = super::render_layout_page_replay_svg(
                &document,
                &layout,
                0,
                &RenderOptions::default(),
            )
            .unwrap()
            .svg;
            for output in [&normal, &replay] {
                let xml = roxmltree::Document::parse(output).unwrap();
                let groups = xml
                    .descendants()
                    .filter(|node| node.has_tag_name("g") && node.attribute("opacity").is_some())
                    .collect::<Vec<_>>();
                assert_eq!(groups.len(), 1);
                let opacity: f64 = groups[0].attribute("opacity").unwrap().parse().unwrap();
                assert!((opacity - 128.0 / 255.0).abs() < 1e-6);
                let lines = groups[0]
                    .descendants()
                    .filter(|node| node.has_tag_name("line"))
                    .collect::<Vec<_>>();
                if setting.is_none() {
                    let paths = groups[0]
                        .descendants()
                        .filter(|node| node.has_tag_name("path"))
                        .collect::<Vec<_>>();
                    assert!(lines.is_empty());
                    assert_eq!(paths.len(), 1);
                    assert_eq!(paths[0].attribute("fill"), Some("#336699"));
                    assert!(paths[0].attribute("fill-opacity").is_none());
                } else {
                    assert_eq!(lines.len(), 2);
                    assert!(lines.iter().all(|node| node.attribute("opacity").is_none()
                        && node.attribute("stroke-opacity").is_none()));
                    assert!(
                        lines
                            .iter()
                            .all(|node| node.attribute("stroke") == Some("#336699"))
                    );
                }
            }
            let xml = roxmltree::Document::parse(&replay).unwrap();
            if setting.is_none() {
                let path = xml
                    .descendants()
                    .find(|node| node.has_tag_name("path"))
                    .unwrap();
                let lengths = path.attribute("data-replay-lengths").unwrap();
                let stamp_count = lengths.split(',').count();
                assert!(stamp_count > 0);
                assert_eq!(
                    path.attribute("d").unwrap().matches('Z').count(),
                    stamp_count
                );
            } else {
                let parts = xml
                    .descendants()
                    .filter(|node| node.has_tag_name("line"))
                    .map(|node| node.attribute("data-replay-part").unwrap())
                    .collect::<Vec<_>>();
                assert_eq!(parts, ["2", "3"]);
            }
        }
    }

    fn public_airbrush_v2_document() -> Document {
        let mut document = crate::parse_bytes(include_bytes!(
            "../tests/assets/chinesebrush/synthetic_v3.sdocx"
        ))
        .unwrap();
        let mut stroke = document.pages[0].strokes().next().unwrap().clone();
        stroke.points = vec![
            Point { x: 1.0, y: -2.0 },
            Point { x: 3.0, y: 1.0 },
            Point { x: 6.0, y: 4.0 },
        ];
        stroke.pressures = vec![0.3, 0.6, 0.85];
        stroke.timestamps = vec![300, 315, 340];
        stroke.tilts = vec![0.12, 0.32, 0.55];
        stroke.orientations = vec![0.2, 0.35, 0.55];
        stroke.color = None;
        stroke.pen_width = 3.144;
        stroke.bbox = BoundingBox {
            x_min: 1.0,
            y_min: -2.0,
            x_max: 6.0,
            y_max: 4.0,
        };
        let rendering = stroke.rendering.as_mut().unwrap();
        rendering.pen_name = Some("com.samsung.android.sdk.pen.pen.preload.AirBrushPen".into());
        rendering.advanced_settings = Some(String::new());
        rendering.tool_type_raw = 2;
        rendering.properties.compressed = true;
        rendering.properties.replay_only = false;
        rendering.properties.eraser = false;
        rendering.properties.fixed_width = false;
        rendering.properties.top_layer_pen = false;
        rendering.properties.alpha_lock = false;
        rendering.properties.fixed_opacity = false;
        rendering.properties.rainbow_effect = false;
        rendering.properties.straighten = false;
        rendering.properties.reveal_mode = false;
        rendering.style.pen_size = Some(3.144);
        rendering.style.color_argb = Some(0xff6c_dd8f);
        rendering.style.stroke_type_raw = Some(0);
        rendering.style.color_type_raw = Some(0);
        let page = &mut document.pages[0];
        page.width = 256;
        page.height = 256;
        page.background_color = None;
        page.objects = vec![stroke.into()];
        document.metadata.dark_mode_compatibility = Some(false);
        document
    }

    #[test]
    fn saved_v1_profile_keeps_unsupported_fallbacks() {
        let mut document = public_airbrush_v2_document();
        document.pages[0]
            .strokes_mut()
            .next()
            .unwrap()
            .rendering
            .as_mut()
            .unwrap()
            .advanced_settings = None;
        let options = RenderOptions {
            airbrush_profile: Some(super::AirBrushRenderProfile::SavedV1),
            ..RenderOptions::default()
        };
        let default = RenderOptions::default();
        let svg =
            |doc: &Document, opts: &RenderOptions| render_document_svg(doc, opts).remove(0).svg;
        assert!(svg(&document, &options).contains("data:image/png;base64,"));
        for setting in ["", "2;", "1;"] {
            document.pages[0]
                .strokes_mut()
                .next()
                .unwrap()
                .rendering
                .as_mut()
                .unwrap()
                .advanced_settings = Some(setting.into());
            assert_eq!(
                svg(&document, &options),
                svg(&document, &default),
                "setting {setting:?}"
            );
        }

        let stroke = document.pages[0].strokes_mut().next().unwrap();
        stroke.rendering.as_mut().unwrap().advanced_settings = None;
        stroke.points = vec![
            Point {
                x: 128.453125,
                y: 128.515625
            };
            5
        ];
        stroke.pressures = vec![0.3; 5];
        stroke.tilts = vec![0.12; 5];
        stroke.orientations = vec![0.2; 5];
        stroke.timestamps = vec![300, 315, 330, 345, 360];
        assert_eq!(
            svg(&document, &options),
            svg(&document, &default),
            "zero-stamp fallback"
        );
        let stroke = document.pages[0].strokes_mut().next().unwrap();
        stroke.points.truncate(3);
        stroke.pressures.truncate(3);
        stroke.tilts.truncate(3);
        stroke.orientations.truncate(3);
        stroke.timestamps.truncate(3);
        assert!(
            svg(&document, &options).contains("data:image/png;base64,"),
            "native three-point stationary stroke emits one stamp"
        );
    }

    #[test]
    fn saved_v1_public_replay_uses_native_curve_boundaries() {
        let mut document = public_airbrush_v2_document();
        let stroke = document.pages[0].strokes_mut().next().unwrap();
        stroke.points = [(0., 0.), (8., 5.), (16., 0.), (24., -5.), (32., 0.)]
            .map(|(x, y)| Point { x, y })
            .into();
        stroke.pressures = vec![0.3; 5];
        stroke.tilts = vec![0.12; 5];
        stroke.orientations = vec![0.2; 5];
        stroke.timestamps = vec![300, 315, 330, 345, 360];
        let rendering = stroke.rendering.as_mut().unwrap();
        rendering.advanced_settings = None;
        rendering.style.pen_size = Some(10.0);
        let options = RenderOptions {
            airbrush_profile: Some(super::AirBrushRenderProfile::SavedV1),
            ..RenderOptions::default()
        };
        let layout = layout_document(&document);
        let replay = render_layout_page_replay_svg(&document, &layout, 0, &options).unwrap();
        let xml = roxmltree::Document::parse(&replay.svg).unwrap();
        let mut parts = std::collections::BTreeMap::new();
        for part in xml
            .descendants()
            .filter_map(|node| node.attribute("data-replay-part"))
        {
            *parts.entry(part.parse::<usize>().unwrap()).or_insert(0) += 1;
        }
        // Each native stamp is rendered as two triangle images. Native drawLine
        // exits are 4, 4, 20 and the terminal endPen exit is 32.
        assert_eq!(parts, [(2, 8), (4, 32), (5, 24)].into());

        document.pages[0]
            .strokes_mut()
            .next()
            .unwrap()
            .rendering
            .as_mut()
            .unwrap()
            .advanced_settings = Some(String::new());
        let v2_options = RenderOptions {
            airbrush_profile: Some(super::AirBrushRenderProfile::SavedV2),
            ..RenderOptions::default()
        };
        let v2_layout = layout_document(&document);
        let v2_replay =
            render_layout_page_replay_svg(&document, &v2_layout, 0, &v2_options).unwrap();
        assert_eq!(replay.svg, v2_replay.svg);
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn saved_v1_public_short_stroke_matches_native_frame() {
        use sha2::{Digest, Sha256};
        let evidence: serde_json::Value = serde_json::from_str(include_str!(
            "../../../conformance/airbrush-short-v1-v2-native.json"
        ))
        .unwrap();
        let case = &evidence["cases"][0];
        assert_eq!(case["selected_drawable_profile_version"], 1);
        let mut document = public_airbrush_v2_document();
        let stroke = document.pages[0].strokes_mut().next().unwrap();
        stroke.points = [128.453125, 128.484375, 128.515625, 128.546875, 128.578125]
            .into_iter()
            .map(|x| Point { x, y: 128.515625 })
            .collect();
        stroke.pressures = vec![0.3; 5];
        stroke.tilts = vec![0.12; 5];
        stroke.orientations = vec![0.2; 5];
        stroke.timestamps = vec![300, 315, 330, 345, 360];
        let rendering = stroke.rendering.as_mut().unwrap();
        rendering.advanced_settings = None;
        rendering.style.color_argb = Some(0xff44aaee);
        let mut expected = vec![0_u8; 256 * 256 * 4];
        for (x, y, rgba) in [
            (127, 126, [0, 1, 1, 1]),
            (128, 126, [6, 16, 22, 23]),
            (129, 126, [0, 1, 1, 1]),
            (127, 127, [6, 14, 20, 22]),
            (128, 127, [14, 34, 48, 52]),
            (129, 127, [3, 8, 11, 12]),
            (127, 128, [0, 1, 1, 1]),
            (128, 128, [4, 9, 13, 14]),
            (129, 128, [0, 1, 1, 1]),
        ] {
            let offset = (y * 256 + x) * 4;
            expected[offset..offset + 4].copy_from_slice(&rgba);
        }
        assert_eq!(
            format!("{:x}", Sha256::digest(&expected)),
            case["post_rgba8_sha256"].as_str().unwrap()
        );
        let options = RenderOptions {
            airbrush_profile: Some(super::AirBrushRenderProfile::SavedV1),
            ..RenderOptions::default()
        };
        let layout = layout_document(&document);
        let request = super::PageSceneRequest::sliced(&layout.pages[0], 0, 256, false).unwrap();
        let mut cache = super::DocumentTextCache::default();
        let rendered = cache
            .render_layout_page_scene_requests(
                &document,
                &[request],
                &options,
                &super::fonts::FontBook::default(),
                &mut |_| {},
            )
            .remove(0)
            .page;
        let tree =
            resvg::usvg::Tree::from_str(&rendered.svg, &resvg::usvg::Options::default()).unwrap();
        let mut pixmap = resvg::tiny_skia::Pixmap::new(256, 256).unwrap();
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::identity(),
            &mut pixmap.as_mut(),
        );
        let flipped: Vec<u8> = expected
            .as_chunks::<{ 256 * 4 }>()
            .0
            .iter()
            .rev()
            .flatten()
            .copied()
            .collect();
        assert_eq!(
            format!("{:x}", Sha256::digest(pixmap.data())),
            format!("{:x}", Sha256::digest(&flipped)),
            "public V1 SVG versus native host frame"
        );
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn saved_v2_advanced_two_public_route_matches_native_v29_full_rgba() {
        use sha2::{Digest, Sha256};

        const NATIVE_INSTANCE_SHA256: &str =
            "086947babde38e1f14f1dbd4a7fe419a3fc64cee8f2921515d5a3b2f8d2154ca";
        const NATIVE_FRAME_SHA256: &str =
            "c0a0b566e23fda9f7aef64415849abd682da9f8260dbc8ec915571546b2edbda";
        const NATIVE_PIXELS: [([u16; 2], [u8; 4]); 19] = [
            ([1, 0], [24, 47, 32, 54]),
            ([2, 0], [54, 111, 71, 127]),
            ([3, 0], [33, 65, 41, 76]),
            ([4, 0], [4, 11, 8, 12]),
            ([2, 1], [17, 35, 24, 41]),
            ([3, 1], [44, 90, 59, 104]),
            ([4, 1], [31, 63, 40, 71]),
            ([5, 1], [0, 1, 1, 1]),
            ([2, 2], [0, 1, 1, 1]),
            ([3, 2], [33, 67, 43, 76]),
            ([4, 2], [40, 83, 54, 95]),
            ([5, 2], [14, 32, 21, 35]),
            ([6, 2], [1, 3, 2, 3]),
            ([4, 3], [22, 44, 30, 52]),
            ([5, 3], [41, 84, 54, 96]),
            ([6, 3], [4, 10, 7, 11]),
            ([4, 4], [1, 3, 2, 3]),
            ([5, 4], [11, 23, 15, 27]),
            ([6, 4], [5, 10, 6, 11]),
        ];

        let evidence: serde_json::Value = serde_json::from_str(include_str!(
            "../../../conformance/airbrush-advanced-two-native.json"
        ))
        .unwrap();
        let version = &evidence["setting_to_version"];
        assert_eq!(version["ObjectStroke_GetAdvancedPenSetting_text"], "2;");
        assert_eq!(version["selected_Pen_getVersion"], 2);
        assert_eq!(version["selected_drawable_profile_version"], 2);
        let setter = &version["Drawing_call_with_exact_getter"][0];
        assert_eq!(
            setter["setting_pointer"],
            version["ObjectStroke_getter_pointer"]
        );

        let mut document = public_airbrush_v2_document();
        let stroke = document.pages[0].strokes_mut().next().unwrap();
        stroke.rendering.as_mut().unwrap().advanced_settings = Some("2;".into());
        assert_eq!(
            stroke
                .rendering
                .as_ref()
                .unwrap()
                .advanced_settings
                .as_deref(),
            Some("2;")
        );
        let sampler = crate::ink::airbrush::saved_v2_from_stroke(stroke)
            .expect("explicit advanced 2; SavedV2 tool-2 source-zero stroke");
        assert_eq!(sampler.rows.len(), 26);
        assert_eq!(sampler.native_argb, 0x326cdd8f);
        let model_instance_bytes: Vec<u8> = sampler
            .rows
            .iter()
            .flatten()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        assert_eq!(model_instance_bytes.len(), 26 * 16);
        assert_eq!(
            format!("{:x}", Sha256::digest(&model_instance_bytes)),
            NATIVE_INSTANCE_SHA256,
            "native V29 26-row AddPoint buffer"
        );

        let mut expected = vec![0_u8; 256 * 256 * 4];
        for ([x, y], rgba) in NATIVE_PIXELS {
            let start = (usize::from(y) * 256 + usize::from(x)) * 4;
            expected[start..start + 4].copy_from_slice(&rgba);
        }
        assert_eq!(
            format!("{:x}", Sha256::digest(&expected)),
            NATIVE_FRAME_SHA256,
            "reconstructed native V29 transparent RGBA8 frame"
        );

        let options = RenderOptions {
            airbrush_profile: Some(super::AirBrushRenderProfile::SavedV2),
            ..RenderOptions::default()
        };
        let layout = layout_document(&document);
        let request = super::PageSceneRequest::sliced(&layout.pages[0], 0, 256, false).unwrap();
        let mut cache = super::DocumentTextCache::default();
        let rendered = cache
            .render_layout_page_scene_requests(
                &document,
                &[request],
                &options,
                &super::fonts::FontBook::default(),
                &mut |_| {},
            )
            .remove(0)
            .page;
        assert_eq!((rendered.width, rendered.height), (256, 256));
        let tree =
            resvg::usvg::Tree::from_str(&rendered.svg, &resvg::usvg::Options::default()).unwrap();
        let mut pixmap = resvg::tiny_skia::Pixmap::new(256, 256).unwrap();
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::identity(),
            &mut pixmap.as_mut(),
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(pixmap.data())),
            NATIVE_FRAME_SHA256,
            "public SavedV2 PageScene RGBA8 versus native V29 advanced-two frame"
        );
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn saved_v2_overlay_profile_matches_native_pageflip_full_rgba() {
        use sha2::{Digest, Sha256};

        const JSON: &str = include_str!("../../../conformance/airbrush-pageflip-native.json");
        const BIN: &[u8] = include_bytes!("../../../conformance/airbrush-pageflip-native.bin");
        const BIN_SHA256: &str = "d73101e402f80ce63d35a5a7a711ebf427bc36b325dae930caf107253ad99a08";
        const CAPTURE_SHA256: &str =
            "ccb761e2b52b631637d66e07cd0d69f9ea452fae138c16006f5070a1a86e95e9";
        const FRAME_SHA256: &str =
            "c0a0b566e23fda9f7aef64415849abd682da9f8260dbc8ec915571546b2edbda";

        assert_eq!(format!("{:x}", Sha256::digest(BIN)), BIN_SHA256);
        let metadata: serde_json::Value = serde_json::from_str(JSON).unwrap();
        assert_eq!(
            metadata["schema"],
            "sdocx.airbrush-pageflip-paint-oracles.v1"
        );
        let capture_sha = metadata["cases"][0]["native_capture_sha256"]
            .as_str()
            .unwrap();
        assert_eq!(capture_sha, CAPTURE_SHA256);
        let case = &metadata["cases"][0];
        assert_eq!(case["native_readback_top_down_sha256"], FRAME_SHA256);
        assert_eq!(case["native_nonzero_pixels"], 19);
        assert_eq!(case["native_instance_count"], 26);
        assert_eq!(
            case["native_instance_sha256"],
            "086947babde38e1f14f1dbd4a7fe419a3fc64cee8f2921515d5a3b2f8d2154ca"
        );
        assert_eq!(case["source"]["advanced_setting_mode"], "explicit-empty");
        assert_eq!(case["source"]["selected_drawable_version"], 2);
        assert_eq!(case["source"]["tool_type_raw"], 2);
        assert_eq!(case["source"]["motion_event_source"], 0);
        assert_eq!(case["source"]["counter_seed"], 0);
        assert_eq!(case["source"]["native_rt_size_u32"], 3);
        assert_eq!(case["source"]["draw_matrix3_column_major_f32"][4], -1.0);
        assert_eq!(case["source"]["draw_matrix3_column_major_f32"][7], 256.0);

        assert_eq!(&BIN[..4], b"ABP1");
        assert_eq!(u16::from_le_bytes(BIN[4..6].try_into().unwrap()), 1);
        assert_eq!(u16::from_le_bytes(BIN[6..8].try_into().unwrap()), 1);
        let mut offset = 8;
        assert_eq!(BIN[offset], 0);
        offset += 1;
        let vector_bytes = u16::from_le_bytes(BIN[offset..offset + 2].try_into().unwrap()) as usize;
        offset += 2;
        let sparse_count = u16::from_le_bytes(BIN[offset..offset + 2].try_into().unwrap()) as usize;
        offset += 2;
        assert_eq!(vector_bytes, 26 * 16);
        assert_eq!(sparse_count, 19);
        let native_rows = &BIN[offset..offset + vector_bytes];
        assert_eq!(
            format!("{:x}", Sha256::digest(native_rows)),
            case["native_instance_sha256"]
        );

        let document = public_airbrush_v2_document();
        let source_stroke = document.pages[0].strokes().next().unwrap();
        let sampler = crate::ink::airbrush::saved_v2_from_stroke(source_stroke)
            .expect("explicit-empty SavedV2 tool-2 source-zero stroke");
        let model_rows: Vec<u8> = sampler
            .rows
            .iter()
            .flatten()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        assert_eq!(
            model_rows, native_rows,
            "production SavedV2 sampler row stream"
        );
        assert_eq!(sampler.native_argb, 0x326cdd8f);

        offset += vector_bytes;
        let mut expected = vec![0_u8; 256 * 256 * 4];
        for _ in 0..sparse_count {
            let x = u16::from_le_bytes(BIN[offset..offset + 2].try_into().unwrap()) as usize;
            let y = u16::from_le_bytes(BIN[offset + 2..offset + 4].try_into().unwrap()) as usize;
            let rgba = &BIN[offset + 4..offset + 8];
            offset += 8;
            let pixel = (y * 256 + x) * 4;
            expected[pixel..pixel + 4].copy_from_slice(rgba);
        }
        assert_eq!(offset, BIN.len());
        assert_eq!(format!("{:x}", Sha256::digest(&expected)), FRAME_SHA256);

        let options = RenderOptions {
            airbrush_profile: Some(super::AirBrushRenderProfile::SavedV2),
            ..RenderOptions::default()
        };
        // The native control starts with a transparent canvas. Exercise the same
        // page-content renderer used by PDF overlays, which omits the paper fill.
        let layout = layout_document(&document);
        let request = super::PageSceneRequest::sliced(&layout.pages[0], 0, 256, false).unwrap();
        let mut cache = super::DocumentTextCache::default();
        let rendered = cache
            .render_layout_page_scene_requests(
                &document,
                &[request],
                &options,
                &super::fonts::FontBook::default(),
                &mut |_| {},
            )
            .remove(0)
            .page;
        assert_eq!((rendered.width, rendered.height), (256, 256));
        assert_eq!(rendered.svg.matches("data:image/png;base64,").count(), 1);
        let xml = roxmltree::Document::parse(&rendered.svg).unwrap();
        assert_eq!(
            xml.descendants()
                .filter(|node| node.has_tag_name("image"))
                .count(),
            1
        );
        assert_eq!(
            xml.descendants()
                .filter(|node| node.has_tag_name("use"))
                .count(),
            52
        );
        assert!(
            xml.descendants()
                .filter(|node| node.has_tag_name("g"))
                .all(|node| node.attribute("opacity").is_none())
        );

        let tree =
            resvg::usvg::Tree::from_str(&rendered.svg, &resvg::usvg::Options::default()).unwrap();
        let mut pixmap = resvg::tiny_skia::Pixmap::new(256, 256).unwrap();
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::identity(),
            &mut pixmap.as_mut(),
        );
        let differences: Vec<_> = pixmap
            .data()
            .as_chunks::<4>()
            .0
            .iter()
            .zip(expected.as_chunks::<4>().0.iter())
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, (a, b))| (i % 256, i / 256, a, b))
            .collect();
        assert!(
            differences.is_empty(),
            "{} pixels differ; first: {:?}",
            differences.len(),
            differences.iter().take(8).collect::<Vec<_>>()
        );

        // Compare the generic fallback on this synthetic tool-2, source-zero,
        // empty-setting control and transparent 1x canvas.
        let fallback = cache
            .render_layout_page_scene_requests(
                &document,
                &[request],
                &RenderOptions::default(),
                &super::fonts::FontBook::default(),
                &mut |_| {},
            )
            .remove(0)
            .page;
        let fallback_tree =
            resvg::usvg::Tree::from_str(&fallback.svg, &resvg::usvg::Options::default()).unwrap();
        let mut fallback_pixels = resvg::tiny_skia::Pixmap::new(256, 256).unwrap();
        resvg::render(
            &fallback_tree,
            resvg::tiny_skia::Transform::identity(),
            &mut fallback_pixels.as_mut(),
        );
        let fallback_error: u64 = fallback_pixels
            .data()
            .iter()
            .zip(&expected)
            .map(|(actual, native)| u64::from(actual.abs_diff(*native)))
            .sum();
        assert_eq!(fallback_error, 2634, "generic fallback absolute RGBA error");
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn saved_v2_integer_edge_overlay_matches_native_full_rgba() {
        use sha2::{Digest, Sha256};

        const JSON: &str = include_str!("../../../conformance/airbrush-pageflip-integer-edge.json");
        const BIN: &[u8] =
            include_bytes!("../../../conformance/airbrush-pageflip-integer-edge.bin");
        const FRAME_SHA256: &str =
            "3ad2010aa04fb518253e6acaa6fd8f733a1a9954bf919b1d90db13c5aadf2613";

        let metadata: serde_json::Value = serde_json::from_str(JSON).unwrap();
        assert_eq!(
            metadata["schema"],
            "sdocx.airbrush-integer-edge-pixel-and-sampler-receipt.v1"
        );
        assert_eq!(
            metadata["case_id"],
            "airbrush-explicit-empty-tool2-v2-material-r125-bg00000000-integer-edge"
        );
        assert_eq!(
            metadata["frame"]["top_down_full_frame_sha256"],
            FRAME_SHA256
        );
        assert_eq!(metadata["frame"]["native_nonzero_pixel_count"], 5);
        assert_eq!(BIN.get(..4), Some(&b"AIE1"[..]));
        assert_eq!(u16::from_le_bytes(BIN[4..6].try_into().unwrap()), 1);
        assert_eq!(u16::from_le_bytes(BIN[6..8].try_into().unwrap()), 5);
        assert_eq!(
            format!("{:x}", Sha256::digest(BIN)),
            "14c1d4f418a967c796c1c8b9f0b7e97b9f20a81431df8b44ab5bd1b8c1b40cb8"
        );

        let case = &metadata["native_draw"];
        assert_eq!(case["draw_call"]["mode"], 5);
        assert_eq!(case["draw_call"]["vertex_count"], 4);
        assert_eq!(case["draw_call"]["instance_count"], 1);
        assert_eq!(
            case["native_instance_hex"],
            "0000004300000043000000000000803f"
        );
        assert_eq!(case["effective_texture_and_sampler"]["texture_2d"], 3);
        assert_eq!(case["effective_texture_and_sampler"]["sampler_object"], 3);
        assert_eq!(case["active_sampler"]["name_bytes_hex"], "73616d706c657230");
        assert_eq!(case["active_sampler"]["units"][0], 0);
        assert_eq!(
            metadata["sampler_row_reference"]["row_file"],
            "actual-sampler-rows.r8"
        );

        let mut expected = vec![0_u8; 256 * 256 * 4];
        let mut offset = 8;
        for _ in 0..5 {
            let x = u16::from_le_bytes(BIN[offset..offset + 2].try_into().unwrap()) as usize;
            let y = u16::from_le_bytes(BIN[offset + 2..offset + 4].try_into().unwrap()) as usize;
            let rgba = &BIN[offset + 4..offset + 8];
            offset += 8;
            let pixel = (y * 256 + x) * 4;
            expected[pixel..pixel + 4].copy_from_slice(rgba);
        }
        assert_eq!(offset, BIN.len());
        assert_eq!(format!("{:x}", Sha256::digest(&expected)), FRAME_SHA256);

        let mut document = public_airbrush_v2_document();
        let mut stroke = document.pages[0].strokes().next().unwrap().clone();
        stroke.points = vec![
            Point { x: 128.0, y: 128.0 },
            Point {
                x: 128.03125,
                y: 128.0,
            },
        ];
        stroke.pressures = vec![0.3, 0.6];
        stroke.timestamps = vec![300, 315];
        stroke.tilts = vec![0.12, 0.32];
        stroke.orientations = vec![0.2, 0.35];
        stroke.bbox = BoundingBox {
            x_min: 128.0,
            y_min: 128.0,
            x_max: 128.03125,
            y_max: 128.0,
        };
        document.pages[0].objects = vec![stroke.into()];
        let source_stroke = document.pages[0].strokes().next().unwrap();
        assert_eq!(
            source_stroke.points,
            [
                Point { x: 128.0, y: 128.0 },
                Point {
                    x: 128.03125,
                    y: 128.0
                }
            ]
        );
        assert_eq!(source_stroke.pressures, [0.3, 0.6]);
        assert_eq!(source_stroke.timestamps, [300, 315]);
        assert_eq!(source_stroke.tilts, [0.12, 0.32]);
        assert_eq!(source_stroke.orientations, [0.2, 0.35]);
        let sampler = crate::ink::airbrush::saved_v2_from_stroke(source_stroke)
            .expect("explicit-empty SavedV2 tool-2 source-zero integer-edge stroke");
        let native_rows: Vec<u8> = case["native_instance_hex"]
            .as_str()
            .unwrap()
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        let model_rows: Vec<u8> = sampler
            .rows
            .iter()
            .flatten()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        assert_eq!(
            model_rows, native_rows,
            "production SavedV2 native one-stamp row"
        );
        assert_eq!(sampler.rows.len(), 1);
        assert_eq!(sampler.native_argb, 0x326cdd8f);

        let options = RenderOptions {
            airbrush_profile: Some(super::AirBrushRenderProfile::SavedV2),
            ..RenderOptions::default()
        };
        let layout = layout_document(&document);
        let request = super::PageSceneRequest::sliced(&layout.pages[0], 0, 256, false).unwrap();
        let mut cache = super::DocumentTextCache::default();
        let rendered = cache
            .render_layout_page_scene_requests(
                &document,
                &[request],
                &options,
                &super::fonts::FontBook::default(),
                &mut |_| {},
            )
            .remove(0)
            .page;
        assert_eq!((rendered.width, rendered.height), (256, 256));
        let tree =
            resvg::usvg::Tree::from_str(&rendered.svg, &resvg::usvg::Options::default()).unwrap();
        let mut pixmap = resvg::tiny_skia::Pixmap::new(256, 256).unwrap();
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::identity(),
            &mut pixmap.as_mut(),
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(pixmap.data())),
            FRAME_SHA256,
            "all PageScene RGBA8 pixels versus native full-frame oracle"
        );
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn saved_v2_public_document_background_matches_native_pageflip_rgba() {
        use sha2::{Digest, Sha256};

        const JSON: &str = include_str!("../../../conformance/airbrush-pageflip-bg-native.json");
        const BIN: &[u8] = include_bytes!("../../../conformance/airbrush-pageflip-bg-native.bin");
        const V27_JSON: &str = include_str!("../../../conformance/airbrush-pageflip-native.json");
        const V27_BIN: &[u8] = include_bytes!("../../../conformance/airbrush-pageflip-native.bin");
        const BG_BIN_SHA256: &str =
            "652b51cb8c0bdf9b7fc58d88a10404117c18bbbaf49cb856eb1029093d0f460b";
        const V27_BIN_SHA256: &str =
            "d73101e402f80ce63d35a5a7a711ebf427bc36b325dae930caf107253ad99a08";
        const NATIVE_FRAME_SHA256: &str =
            "46749a09577a07f94bb21bc1cd532f7d913a3a3db85eec44c1e685d2766c2f65";

        assert_eq!(format!("{:x}", Sha256::digest(BIN)), BG_BIN_SHA256);
        assert_eq!(format!("{:x}", Sha256::digest(V27_BIN)), V27_BIN_SHA256);
        let metadata: serde_json::Value = serde_json::from_str(JSON).unwrap();
        assert_eq!(
            metadata["schema"],
            "sdocx.airbrush-public-bg-pixel-oracle.v1"
        );
        assert_eq!(
            metadata["case_id"],
            "airbrush-explicit-empty-tool2-v2-bgfcfcfcff"
        );
        assert_eq!(
            metadata["background_rgba8"],
            serde_json::json!([252, 252, 252, 255])
        );
        assert_eq!(metadata["dimensions"], serde_json::json!([256, 256]));
        assert_eq!(
            metadata["native_input_checks"]["same_target_pre_draw_equals_input"],
            true
        );
        assert_eq!(
            metadata["native_input_checks"]["same_target_pre_draw_rgba8_sha256"],
            "58451335f520887b25c551445b3bdfdb812bf4ce076de9270c502a0ef73c545a"
        );
        assert_eq!(
            metadata["native_input_checks"]["post_draw_document_top_down_sha256"],
            NATIVE_FRAME_SHA256
        );
        assert_eq!(
            metadata["native_input_checks"]["post_draw_changed_pixels_from_background"],
            19
        );
        assert_eq!(
            metadata["native_source"]["native_instance_sha256"],
            "086947babde38e1f14f1dbd4a7fe419a3fc64cee8f2921515d5a3b2f8d2154ca"
        );
        assert_eq!(metadata["reuse_geometry"]["binary_sha256"], V27_BIN_SHA256);

        let prior: serde_json::Value = serde_json::from_str(V27_JSON).unwrap();
        assert_eq!(
            prior["cases"][0]["source"]["advanced_setting_mode"],
            "explicit-empty"
        );
        assert_eq!(prior["cases"][0]["source"]["selected_drawable_version"], 2);
        assert_eq!(prior["cases"][0]["source"]["tool_type_raw"], 2);
        assert_eq!(prior["cases"][0]["source"]["motion_event_source"], 0);

        assert_eq!(&BIN[..4], b"ABBG");
        assert_eq!(u16::from_le_bytes(BIN[4..6].try_into().unwrap()), 1);
        let sparse_count = u16::from_le_bytes(BIN[6..8].try_into().unwrap()) as usize;
        assert_eq!(sparse_count, 19);
        assert_eq!(BIN.len(), 8 + sparse_count * 8);
        let mut expected = [252_u8, 252, 252, 255].repeat(256 * 256);
        let mut offset = 8;
        for _ in 0..sparse_count {
            let x = u16::from_le_bytes(BIN[offset..offset + 2].try_into().unwrap()) as usize;
            let y = u16::from_le_bytes(BIN[offset + 2..offset + 4].try_into().unwrap()) as usize;
            let pixel = (y * 256 + x) * 4;
            expected[pixel..pixel + 4].copy_from_slice(&BIN[offset + 4..offset + 8]);
            offset += 8;
        }
        assert_eq!(
            format!("{:x}", Sha256::digest(&expected)),
            NATIVE_FRAME_SHA256
        );

        let document = public_airbrush_v2_document();
        let options = RenderOptions {
            airbrush_profile: Some(super::AirBrushRenderProfile::SavedV2),
            ..RenderOptions::default()
        };
        let rendered = render_document_svg(&document, &options).remove(0);
        assert_eq!((rendered.width, rendered.height), (256, 256));
        assert_eq!(rendered.svg.matches("data:image/png;base64,").count(), 1);
        let tree =
            resvg::usvg::Tree::from_str(&rendered.svg, &resvg::usvg::Options::default()).unwrap();
        let mut pixmap = resvg::tiny_skia::Pixmap::new(256, 256).unwrap();
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::identity(),
            &mut pixmap.as_mut(),
        );
        assert!(
            pixmap.data() == expected,
            "public AirBrush SVG background RGBA must match native"
        );
    }

    #[test]
    fn saved_v2_strokes_share_texture_and_keep_unique_mask_resources() {
        let mut document = public_airbrush_v2_document();
        let object = document.pages[0].objects[0].clone();
        document.pages[0].objects.push(object);
        let options = RenderOptions {
            airbrush_profile: Some(super::AirBrushRenderProfile::SavedV2),
            ..RenderOptions::default()
        };
        let rendered = render_document_svg(&document, &options).remove(0);
        let xml = roxmltree::Document::parse(&rendered.svg).unwrap();
        assert_eq!(
            xml.descendants()
                .filter(|n| n.has_tag_name("image"))
                .count(),
            1
        );
        assert_eq!(
            xml.descendants().filter(|n| n.has_tag_name("use")).count(),
            104
        );
        let mut ids = std::collections::HashSet::new();
        for id in xml.descendants().filter_map(|n| n.attribute("id")) {
            assert!(ids.insert(id), "duplicate scene resource {id}");
        }
    }

    #[test]
    fn saved_v2_over_budget_stroke_keeps_whole_generic_fallback() {
        let mut document = public_airbrush_v2_document();
        let stroke = document.pages[0].strokes_mut().next().unwrap();
        stroke.points[1].x = 6_000.0;
        stroke.points[2].x = 12_000.0;
        stroke.bbox.x_max = 12_000.0;
        assert!(
            super::airbrush::prepare(stroke, RenderTheme::for_canvas(false), [256, 256]).is_none()
        );
        let options = RenderOptions {
            airbrush_profile: Some(super::AirBrushRenderProfile::SavedV2),
            ..RenderOptions::default()
        };
        let selected = render_document_svg(&document, &options).remove(0);
        let fallback = render_document_svg(&document, &RenderOptions::default()).remove(0);
        assert_eq!(
            selected.svg, fallback.svg,
            "budget rejection must not drop any part of the stroke"
        );
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn saved_v2_public_transparent_material_r125_matches_native_full_rgba() {
        use sha2::{Digest, Sha256};

        const JSON: &str = include_str!("../../../conformance/airbrush-material-r125-native.json");
        const BIN: &[u8] = include_bytes!("../../../conformance/airbrush-material-r125-native.bin");
        const BIN_SHA256: &str = "98614e0343dc3b98dbccdd747e2aef992fee3765a540adbb23ad510d4191aac0";
        const FRAME_SHA256: &str =
            "a2fd2de1b00bb90edff32b2c083468dac9c8f2839b8ed7a69fcbae746d148acc";
        const NATIVE_VBO_HEX: &str = "0074004300840043000000000000803f";

        assert_eq!(format!("{:x}", Sha256::digest(BIN)), BIN_SHA256);
        let metadata: serde_json::Value = serde_json::from_str(JSON).unwrap();
        assert_eq!(
            metadata["schema"],
            "sdocx.airbrush-material-r125-pixel-oracle.v1"
        );
        assert_eq!(
            metadata["case_id"],
            "airbrush-explicit-empty-tool2-v2-material-r125-bg00000000"
        );
        assert_eq!(metadata["material_witness"]["r8"], 125);
        assert_eq!(
            metadata["material_witness"]["texel_top_down_xy"],
            serde_json::json!([49, 48])
        );
        assert_eq!(
            metadata["material_witness"]["native_rgba8"],
            serde_json::json!([10, 21, 13, 24])
        );
        assert_eq!(metadata["native_vbo"]["row_count"], 1);
        assert_eq!(
            metadata["native_vbo"]["little_endian_f32_hex"],
            NATIVE_VBO_HEX
        );
        assert_eq!(
            metadata["native_capture"]["post_draw_document_top_down_sha256"],
            FRAME_SHA256
        );
        assert_eq!(metadata["native_capture"]["post_draw_changed_pixels"], 7);
        assert_eq!(
            metadata["native_capture"]["pre_post_gl_errors"],
            serde_json::json!([])
        );

        assert_eq!(&BIN[..4], b"ABMT");
        assert_eq!(u16::from_le_bytes(BIN[4..6].try_into().unwrap()), 1);
        let sparse_count = u16::from_le_bytes(BIN[6..8].try_into().unwrap()) as usize;
        assert_eq!(sparse_count, 7);
        assert_eq!(BIN.len(), 8 + sparse_count * 8);
        let mut expected = vec![0_u8; 256 * 256 * 4];
        let mut offset = 8;
        for _ in 0..sparse_count {
            let x = u16::from_le_bytes(BIN[offset..offset + 2].try_into().unwrap()) as usize;
            let y = u16::from_le_bytes(BIN[offset + 2..offset + 4].try_into().unwrap()) as usize;
            let pixel = (y * 256 + x) * 4;
            expected[pixel..pixel + 4].copy_from_slice(&BIN[offset + 4..offset + 8]);
            offset += 8;
        }
        assert_eq!(format!("{:x}", Sha256::digest(&expected)), FRAME_SHA256);
        assert_eq!(expected[(128 * 256 + 128) * 4..][..4], [10, 21, 13, 24]);

        let mut document = public_airbrush_v2_document();
        let stroke = document.pages[0].strokes_mut().next().unwrap();
        stroke.points = vec![
            Point {
                x: 128.453125,
                y: 128.515625,
            },
            Point {
                x: 128.484375,
                y: 128.515625,
            },
        ];
        stroke.pressures = vec![0.3, 0.6];
        stroke.timestamps = vec![300, 315];
        stroke.tilts = vec![0.12, 0.32];
        stroke.orientations = vec![0.2, 0.35];
        stroke.bbox = BoundingBox {
            x_min: 128.453125,
            y_min: 128.515625,
            x_max: 128.484375,
            y_max: 128.515625,
        };
        let sampler = crate::ink::airbrush::saved_v2_from_stroke(stroke)
            .expect("explicit-empty SavedV2 R8 material control");
        assert_eq!(sampler.rows.len(), 1);
        let model_rows: Vec<u8> = sampler
            .rows
            .iter()
            .flatten()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        let native_vbo: Vec<u8> = NATIVE_VBO_HEX
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        assert_eq!(
            model_rows, native_vbo,
            "production sampler row versus captured native AddPoint VBO"
        );
        assert_eq!(sampler.native_argb, 0x326cdd8f);

        let options = RenderOptions {
            airbrush_profile: Some(super::AirBrushRenderProfile::SavedV2),
            ..RenderOptions::default()
        };
        let layout = layout_document(&document);
        let request = super::PageSceneRequest::sliced(&layout.pages[0], 0, 256, false).unwrap();
        let mut cache = super::DocumentTextCache::default();
        let rendered = cache
            .render_layout_page_scene_requests(
                &document,
                &[request],
                &options,
                &super::fonts::FontBook::default(),
                &mut |_| {},
            )
            .remove(0)
            .page;
        assert_eq!((rendered.width, rendered.height), (256, 256));
        let xml = roxmltree::Document::parse(&rendered.svg).unwrap();
        assert_eq!(
            xml.descendants()
                .filter(|node| node.has_tag_name("image"))
                .count(),
            1
        );
        assert_eq!(
            xml.descendants()
                .filter(|node| node.has_tag_name("use"))
                .count(),
            2
        );
        assert_eq!(
            xml.descendants()
                .filter(|node| node.has_tag_name("mask"))
                .count(),
            6
        );
        assert_eq!(
            xml.descendants()
                .filter(|node| node.has_tag_name("linearGradient"))
                .count(),
            6
        );
        let tree =
            resvg::usvg::Tree::from_str(&rendered.svg, &resvg::usvg::Options::default()).unwrap();
        let mut pixmap = resvg::tiny_skia::Pixmap::new(256, 256).unwrap();
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::identity(),
            &mut pixmap.as_mut(),
        );
        assert_eq!(
            pixmap.data(),
            expected,
            "public AirBrush transparent RGBA versus native material control"
        );
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn saved_v2_public_material_matrix_matches_native_page_scene_rgba() {
        use sha2::{Digest, Sha256};

        const JSON: &str =
            include_str!("../../../conformance/airbrush-material-matrix-native.json");
        const BIN: &[u8] =
            include_bytes!("../../../conformance/airbrush-material-matrix-native.bin");
        const JSON_SHA256: &str =
            "8ea53d0dbc087a6dd24aa19c80210f1207cd043b7a546347cd8f24413943d54a";
        const BIN_SHA256: &str = "790f41410ad84341d1291c84272a04131931f8e9e1b8e38ad52c84e17db5315d";
        const NATIVE_PATTERN_SHA256: &str =
            "2bcc637332cb6ca7da347f6bc0010a3312901fc19aa69dee31e4d8b4ced75172";

        fn bytes_from_hex(value: &str) -> Vec<u8> {
            assert!(value.len().is_multiple_of(2));
            value
                .as_bytes()
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect()
        }
        fn f32_values(value: &str) -> Vec<f64> {
            bytes_from_hex(value)
                .as_chunks::<4>()
                .0
                .iter()
                .map(|bytes| f64::from(f32::from_le_bytes(*bytes)))
                .collect()
        }
        fn argb(value: &str) -> u32 {
            u32::from_str_radix(value.strip_prefix("0x").unwrap(), 16).unwrap()
        }

        assert_eq!(
            format!("{:x}", Sha256::digest(JSON.as_bytes())),
            JSON_SHA256
        );
        assert_eq!(format!("{:x}", Sha256::digest(BIN)), BIN_SHA256);
        let metadata: serde_json::Value = serde_json::from_str(JSON).unwrap();
        assert_eq!(
            metadata["schema"],
            "sdocx.airbrush-material-matrix-native-oracles.v1"
        );
        assert_eq!(metadata["dimensions"], serde_json::json!([256, 256]));
        assert_eq!(metadata["pattern_asset_sha256"], NATIVE_PATTERN_SHA256);
        assert_eq!(&BIN[..4], b"ABMX");
        assert_eq!(u16::from_le_bytes(BIN[4..6].try_into().unwrap()), 1);
        let case_count = u16::from_le_bytes(BIN[6..8].try_into().unwrap()) as usize;
        assert_eq!(case_count, 8);
        assert_eq!(metadata["cases"].as_array().unwrap().len(), case_count);

        let options = RenderOptions {
            airbrush_profile: Some(super::AirBrushRenderProfile::SavedV2),
            ..RenderOptions::default()
        };
        let mut failures = Vec::new();
        let mut offset = 8;
        for case_index in 0..case_count {
            let fixture = &metadata["cases"][case_index];
            assert_eq!(fixture["index"], case_index);
            let index = BIN[offset] as usize;
            offset += 1;
            assert_eq!(index, case_index);
            let vbo_bytes =
                u16::from_le_bytes(BIN[offset..offset + 2].try_into().unwrap()) as usize;
            let sparse_count =
                u16::from_le_bytes(BIN[offset + 2..offset + 4].try_into().unwrap()) as usize;
            offset += 4;
            assert_eq!(
                vbo_bytes,
                fixture["native_vbo"]["bytes"].as_u64().unwrap() as usize
            );
            let native_vbo = &BIN[offset..offset + vbo_bytes];
            offset += vbo_bytes;
            assert_eq!(
                format!("{:x}", Sha256::digest(native_vbo)),
                fixture["native_vbo"]["sha256"]
            );
            assert_eq!(
                sparse_count,
                fixture["native_frame"]["painted_pixels"].as_u64().unwrap() as usize
            );
            assert_eq!(
                sparse_count,
                fixture["native_frame"]["binary_sparse_count"]
                    .as_u64()
                    .unwrap() as usize
            );

            let mut expected = vec![0_u8; 256 * 256 * 4];
            for _ in 0..sparse_count {
                let x = u16::from_le_bytes(BIN[offset..offset + 2].try_into().unwrap()) as usize;
                let y =
                    u16::from_le_bytes(BIN[offset + 2..offset + 4].try_into().unwrap()) as usize;
                let pixel = (y * 256 + x) * 4;
                expected[pixel..pixel + 4].copy_from_slice(&BIN[offset + 4..offset + 8]);
                offset += 8;
            }
            assert_eq!(
                format!("{:x}", Sha256::digest(&expected)),
                fixture["native_frame"]["post_document_top_down_sha256"]
            );
            assert_eq!(
                fixture["native_frame"]["background_rgba8"],
                serde_json::json!([0, 0, 0, 0])
            );
            assert_eq!(fixture["native_frame"]["gl_errors"], serde_json::json!([]));
            assert_eq!(
                fixture["native_vbo"]["draw_call"],
                serde_json::json!([5, 0, 4, 1])
            );

            let source = &fixture["native_inputs"];
            assert_eq!(source["advanced_setting_mode"], "explicit-empty");
            assert_eq!(source["tool_type_raw"], 2);
            assert_eq!(source["motion_event_source"], 0);
            let source_size_bytes =
                bytes_from_hex(source["source_size_f32_bits_le"].as_str().unwrap());
            let source_size = f32::from_le_bytes(source_size_bytes.try_into().unwrap());
            let source_argb = argb(source["source_argb_u32"].as_str().unwrap());
            assert_eq!(
                argb(source["model_color_getter_u32"].as_str().unwrap()),
                source_argb
            );
            let source_arrays = &source["source_arrays"];
            let point_values = f32_values(source_arrays["points"]["data_hex"].as_str().unwrap());
            assert_eq!(point_values.len(), 4);
            let points = point_values
                .as_chunks::<2>()
                .0
                .iter()
                .map(|point| Point {
                    x: point[0],
                    y: point[1],
                })
                .collect::<Vec<_>>();
            let pressures = f32_values(source_arrays["pressures"]["data_hex"].as_str().unwrap());
            let timestamps =
                bytes_from_hex(source_arrays["timestamps"]["data_hex"].as_str().unwrap())
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|bytes| i64::from(i32::from_le_bytes(*bytes)))
                    .collect::<Vec<_>>();
            let tilts = f32_values(source_arrays["tilts"]["data_hex"].as_str().unwrap());
            let orientations =
                f32_values(source_arrays["orientations"]["data_hex"].as_str().unwrap());
            assert_eq!(pressures.len(), 2);
            assert_eq!(timestamps.len(), 2);
            assert_eq!(tilts.len(), 2);
            assert_eq!(orientations.len(), 2);

            let mut document = public_airbrush_v2_document();
            let stroke = document.pages[0].strokes_mut().next().unwrap();
            stroke.points = points;
            stroke.pressures = pressures;
            stroke.timestamps = timestamps;
            stroke.tilts = tilts;
            stroke.orientations = orientations;
            stroke.color = None;
            stroke.pen_width = source_size;
            let (x_min, x_max) = stroke
                .points
                .iter()
                .map(|point| point.x)
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), x| {
                    (min.min(x), max.max(x))
                });
            let (y_min, y_max) = stroke
                .points
                .iter()
                .map(|point| point.y)
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), y| {
                    (min.min(y), max.max(y))
                });
            stroke.bbox = BoundingBox {
                x_min,
                y_min,
                x_max,
                y_max,
            };
            let rendering = stroke.rendering.as_mut().unwrap();
            rendering.advanced_settings = Some(String::new());
            rendering.tool_type_raw = 2;
            rendering.style.pen_size = Some(source_size);
            rendering.style.color_argb = Some(source_argb);
            let sampler = crate::ink::airbrush::saved_v2_from_stroke(stroke)
                .expect("explicit-empty SavedV2 native material case");

            let native_paint = &fixture["native_paint"];
            let rt_argb = argb(native_paint["rt_set_pen_data_argb_u32"].as_str().unwrap());
            assert_eq!(sampler.native_argb, rt_argb, "native RTV1 SetPenData color");
            let rt_size_bits = u32::from_str_radix(
                native_paint["rt_set_pen_data_size_f32_bits"]
                    .as_str()
                    .unwrap()
                    .trim_start_matches("0x"),
                16,
            )
            .unwrap();
            assert_eq!(
                rt_size_bits,
                source_size.to_bits(),
                "native RTV1 SetPenData size argument"
            );
            let rgb_a = [
                ((rt_argb >> 16) & 0xff) as u8,
                ((rt_argb >> 8) & 0xff) as u8,
                (rt_argb & 0xff) as u8,
                (rt_argb >> 24) as u8,
            ];
            let native_uniform: Vec<u8> = rgb_a
                .iter()
                .flat_map(|byte| (f32::from(*byte) / 255.0).to_le_bytes())
                .collect();
            assert_eq!(
                native_uniform,
                bytes_from_hex(native_paint["uniform_rgba_f32_le_hex"].as_str().unwrap()),
                "actual captured AirBrush inputColor uniform"
            );

            let model_rows: Vec<u8> = sampler
                .rows
                .iter()
                .flatten()
                .flat_map(|value| value.to_le_bytes())
                .collect();
            assert_eq!(
                model_rows, native_vbo,
                "production sampler versus actual native AddPoint rows"
            );
            assert_eq!(
                sampler.rows.len(),
                fixture["native_vbo"]["row_count"].as_u64().unwrap() as usize
            );

            let layout = layout_document(&document);
            let request = super::PageSceneRequest::sliced(&layout.pages[0], 0, 256, false).unwrap();
            let mut cache = super::DocumentTextCache::default();
            let rendered = cache
                .render_layout_page_scene_requests(
                    &document,
                    &[request],
                    &options,
                    &super::fonts::FontBook::default(),
                    &mut |_| {},
                )
                .remove(0)
                .page;
            assert_eq!((rendered.width, rendered.height), (256, 256));
            let xml = roxmltree::Document::parse(&rendered.svg).unwrap();
            assert_eq!(
                xml.descendants()
                    .filter(|node| node.has_tag_name("image"))
                    .count(),
                1
            );
            assert_eq!(
                xml.descendants()
                    .filter(|node| node.has_tag_name("use"))
                    .count(),
                sampler.rows.len() * 2
            );
            assert_eq!(
                xml.descendants()
                    .filter(|node| node.has_tag_name("mask"))
                    .count(),
                sampler.rows.len() * 6
            );
            assert_eq!(
                xml.descendants()
                    .filter(|node| node.has_tag_name("linearGradient"))
                    .count(),
                sampler.rows.len() * 6
            );
            let tree = resvg::usvg::Tree::from_str(&rendered.svg, &resvg::usvg::Options::default())
                .unwrap();
            let mut pixmap = resvg::tiny_skia::Pixmap::new(256, 256).unwrap();
            resvg::render(
                &tree,
                resvg::tiny_skia::Transform::identity(),
                &mut pixmap.as_mut(),
            );
            if pixmap.data() != expected {
                let differing_pixels = pixmap
                    .data()
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .zip(expected.as_chunks::<4>().0)
                    .filter(|(a, b)| a != b)
                    .count();
                failures.push((
                    fixture["case_id"].as_str().unwrap().to_owned(),
                    differing_pixels,
                    format!("{:x}", Sha256::digest(pixmap.data())),
                ));
            }
        }
        assert_eq!(offset, BIN.len(), "trailing material matrix fixture bytes");
        assert!(
            failures.is_empty(),
            "native full-frame mismatches: {failures:?}"
        );
    }

    #[test]
    fn public_saved_v2_profile_fails_closed_for_unset_v1_and_unsupported_setting() {
        let mut document = public_airbrush_v2_document();
        let options = RenderOptions {
            airbrush_profile: Some(super::AirBrushRenderProfile::SavedV2),
            ..RenderOptions::default()
        };
        for setting in [None, Some("3;")] {
            let stroke = document.pages[0].strokes_mut().next().unwrap();
            stroke.rendering.as_mut().unwrap().advanced_settings = setting.map(str::to_owned);
            let selected = render_document_svg(&document, &options).remove(0).svg;
            let fallback = render_document_svg(&document, &RenderOptions::default())
                .remove(0)
                .svg;
            assert_eq!(selected, fallback);
        }
    }

    #[test]
    fn parsed_airbrush_missing_color_recovers_native_rgb_without_group_opacity() {
        let source = crate::parse_bytes(include_bytes!(
            "../tests/assets/chinesebrush/synthetic_v3.sdocx"
        ))
        .unwrap();
        let make_airbrush = |mut document: Document, metadata_present: bool| {
            let stroke = document.pages[0].strokes_mut().next().unwrap();
            let rendering = stroke.rendering.as_mut().unwrap();
            rendering.pen_name = Some("com.samsung.android.sdk.pen.pen.preload.AirBrushPen".into());
            rendering.advanced_settings = None;
            rendering.tool_type_raw = 2;
            rendering.style.color_argb = None;
            if !metadata_present {
                rendering.metadata = None;
            }
            stroke.color = None;
            document
        };
        let generic = make_airbrush(source.clone(), false);
        let corrected = make_airbrush(source, true);
        let stroke = corrected.pages[0].strokes().next().unwrap();

        // This is an authored parsed-fixture mutation to exercise rendering
        // integration; the native evidence comes from the separate Drawing
        // caller control (null/empty settings -> black GetColor/SetPenData).
        // Native per-stamp alpha remains outside the generic path, whose group
        // opacity must stay unchanged.
        assert!(
            render_document_svg(&generic, &RenderOptions::default())[0]
                .svg
                .contains(r##"stroke="#1a1a1a""##)
        );
        assert!(
            render_document_svg(&corrected, &RenderOptions::default())[0]
                .svg
                .contains(r##"stroke="#000000""##)
        );
        assert_eq!(super::stroke_paint(stroke, false).color, "#000000");
        let prepared = super::prepare_stroke_for_page(stroke, false);
        assert_eq!(prepared.color, "#000000");
        assert_eq!(prepared.opacity, 1.0);
        let layout = layout_document(&corrected);
        assert!(
            super::render_layout_page_replay_svg(&corrected, &layout, 0, &RenderOptions::default())
                .unwrap()
                .svg
                .contains(r##"stroke="#000000""##)
        );

        let mut empty_setting = stroke.clone();
        empty_setting.rendering.as_mut().unwrap().advanced_settings = Some(String::new());
        assert_eq!(super::stroke_paint(&empty_setting, false).color, "#000000");
        assert_eq!(
            super::prepare_stroke_for_page(&empty_setting, false).opacity,
            1.0
        );

        let mut public_color = stroke.clone();
        public_color.color = Some(Color {
            r: 0x33,
            g: 0x66,
            b: 0x99,
        });
        assert_eq!(super::stroke_paint(&public_color, false).color, "#336699");

        let mut saved_color = stroke.clone();
        saved_color.color = None;
        saved_color.rendering.as_mut().unwrap().style.color_argb = Some(0x326c_dd8f);
        assert_eq!(super::stroke_paint(&saved_color, false).color, "#6cdd8f");
        assert_eq!(
            super::prepare_stroke_for_page(&saved_color, false).opacity,
            1.0
        );

        type RejectedCase = (&'static str, fn(&mut Stroke));
        let rejected: [RejectedCase; 5] = [
            ("setting", |s| {
                s.rendering.as_mut().unwrap().advanced_settings = Some("1;".into())
            }),
            ("tool", |s| s.rendering.as_mut().unwrap().tool_type_raw = 1),
            ("fixed opacity", |s| {
                s.rendering.as_mut().unwrap().properties.fixed_opacity = true
            }),
            ("rainbow", |s| {
                s.rendering.as_mut().unwrap().properties.rainbow_effect = true
            }),
            ("metadata", |s| {
                s.rendering.as_mut().unwrap().metadata = None
            }),
        ];
        for (reason, mutate) in rejected {
            let mut unsupported = stroke.clone();
            mutate(&mut unsupported);
            assert_eq!(
                super::stroke_paint(&unsupported, false).color,
                "#1a1a1a",
                "{reason}"
            );
        }
    }

    #[test]
    fn saved_textured_brush_rgb_is_bounded_to_observed_profiles() {
        const COLORED_PENCIL: &str = "com.samsung.android.sdk.pen.pen.preload.ColoredPencil";
        const WATERCOLOR: &str = "com.samsung.android.sdk.pen.pen.preload.WaterColorBrush";
        const SIMPLE_BRUSH: &str = "com.samsung.android.sdk.pen.pen.preload.Brush";
        let source = crate::parse_bytes(include_bytes!(
            "../tests/assets/chinesebrush/synthetic_v3.sdocx"
        ))
        .unwrap();
        let make_document = |pen_name: &str, advanced_setting: Option<&str>| {
            let mut document = source.clone();
            let stroke = document.pages[0].strokes_mut().next().unwrap();
            let rendering = stroke.rendering.as_mut().unwrap();
            rendering.pen_name = Some(pen_name.to_owned());
            rendering.advanced_settings = advanced_setting.map(str::to_owned);
            rendering.tool_type_raw = 2;
            rendering.style.color_argb = Some(0x8044_aaee);
            rendering.style.color_type_raw = Some(0);
            rendering.style.stroke_type_raw = Some(0);
            rendering.properties.replay_only = false;
            rendering.properties.eraser = false;
            rendering.properties.fixed_width = false;
            rendering.properties.top_layer_pen = false;
            rendering.properties.alpha_lock = false;
            rendering.properties.fixed_opacity = false;
            rendering.properties.rainbow_effect = false;
            rendering.properties.straighten = false;
            rendering.properties.reveal_mode = false;
            stroke.color = None;
            document
        };

        // Native Drawing controls preserve saved RGB for the selected factory
        // routes; the SimpleBrush color control is synthetic and uses an identity
        // host conversion. These SVG
        // checks cover RGB selection only; they do not assert mask/texture
        // paint or whole-stroke alpha parity.
        for (pen_name, setting) in [
            (COLORED_PENCIL, Some("4;")),
            (WATERCOLOR, None),
            (WATERCOLOR, Some("")),
            (SIMPLE_BRUSH, None),
        ] {
            let document = make_document(pen_name, setting);
            let stroke = document.pages[0].strokes().next().unwrap();
            assert_eq!(
                super::stroke_source_color(stroke),
                Some(Color {
                    r: 0x44,
                    g: 0xaa,
                    b: 0xee,
                })
            );
            assert_eq!(super::stroke_paint(stroke, false).color, "#44aaee");
            assert_eq!(super::prepare_stroke_for_page(stroke, false).opacity, 1.0);
            let svg = render_document_svg(&document, &RenderOptions::default())[0]
                .svg
                .clone();
            assert!(
                svg.contains(r##"stroke="#44aaee""##),
                "{pen_name} {setting:?}"
            );
        }

        let mut public_override = make_document(SIMPLE_BRUSH, None);
        public_override.pages[0].strokes_mut().next().unwrap().color = Some(Color {
            r: 0x12,
            g: 0x34,
            b: 0x56,
        });
        let public_stroke = public_override.pages[0].strokes().next().unwrap();
        assert_eq!(super::stroke_paint(public_stroke, false).color, "#123456");
        assert!(
            render_document_svg(&public_override, &RenderOptions::default())[0]
                .svg
                .contains(r##"stroke="#123456""##)
        );

        type RejectedCase = (&'static str, fn(&mut Stroke));
        let rejected: [RejectedCase; 16] = [
            ("unobserved ColoredPencil version", |s| {
                s.rendering.as_mut().unwrap().advanced_settings = Some("3;".into())
            }),
            ("unobserved WaterColor version", |s| {
                s.rendering.as_mut().unwrap().advanced_settings = Some("1;".into())
            }),
            ("tool type", |s| {
                s.rendering.as_mut().unwrap().tool_type_raw = 1
            }),
            ("color type", |s| {
                s.rendering.as_mut().unwrap().style.color_type_raw = Some(1)
            }),
            ("stroke type", |s| {
                s.rendering.as_mut().unwrap().style.stroke_type_raw = Some(1)
            }),
            ("rainbow effect", |s| {
                s.rendering.as_mut().unwrap().properties.rainbow_effect = true
            }),
            ("replay only", |s| {
                s.rendering.as_mut().unwrap().properties.replay_only = true
            }),
            ("eraser", |s| {
                s.rendering.as_mut().unwrap().properties.eraser = true
            }),
            ("fixed width", |s| {
                s.rendering.as_mut().unwrap().properties.fixed_width = true
            }),
            ("top layer", |s| {
                s.rendering.as_mut().unwrap().properties.top_layer_pen = true
            }),
            ("alpha lock", |s| {
                s.rendering.as_mut().unwrap().properties.alpha_lock = true
            }),
            ("fixed opacity", |s| {
                s.rendering.as_mut().unwrap().properties.fixed_opacity = true
            }),
            ("straighten", |s| {
                s.rendering.as_mut().unwrap().properties.straighten = true
            }),
            ("reveal mode", |s| {
                s.rendering.as_mut().unwrap().properties.reveal_mode = true
            }),
            ("metadata", |s| {
                s.rendering.as_mut().unwrap().metadata = None
            }),
            ("missing saved color", |s| {
                s.rendering.as_mut().unwrap().style.color_argb = None
            }),
        ];
        for (reason, mutate) in rejected {
            for pen in [WATERCOLOR, SIMPLE_BRUSH] {
                let mut unsupported = make_document(pen, None);
                let stroke = unsupported.pages[0].strokes_mut().next().unwrap();
                mutate(stroke);
                assert_eq!(
                    super::stroke_paint(stroke, false).color,
                    "#1a1a1a",
                    "{pen}: {reason}"
                );
            }
        }

        let unsupported = make_document(SIMPLE_BRUSH, Some(""));
        assert_eq!(
            super::stroke_paint(unsupported.pages[0].strokes().next().unwrap(), false).color,
            "#1a1a1a"
        );

        let unsupported = make_document(COLORED_PENCIL, Some("3;"));
        assert!(
            render_document_svg(&unsupported, &RenderOptions::default())[0]
                .svg
                .contains(r##"stroke="#1a1a1a""##)
        );
    }

    #[test]
    fn parsed_chinesebrush_missing_color_uses_native_objectstroke_default() {
        let document = crate::parse_bytes(include_bytes!(
            "../tests/assets/chinesebrush/synthetic_v3.sdocx"
        ))
        .unwrap();
        let stroke = document.pages[0].strokes().next().unwrap();
        let rendering = stroke.rendering.as_ref().unwrap();
        assert_eq!(
            rendering.pen_name.as_deref(),
            Some("com.samsung.android.sdk.pen.pen.preload.ChineseBrush")
        );
        assert_eq!(rendering.advanced_settings.as_deref(), Some("3;"));
        assert!(rendering.metadata.is_some());
        assert_eq!(rendering.style.color_argb, None);
        assert_eq!(stroke.color, None);

        assert_eq!(
            super::stroke_source_color(stroke),
            Some(Color { r: 0, g: 0, b: 0 })
        );
        assert_eq!(super::stroke_paint(stroke, false).color, "#000000");
        assert_eq!(super::stroke_paint(stroke, true).color, "#ffffff");
        assert_eq!(crate::prepare_stroke(stroke, false).color, "#000000");
        assert_eq!(
            super::prepare_stroke_for_page(stroke, false).color,
            "#000000"
        );
        assert_eq!(
            super::prepare_stroke_for_page_with_theme(
                stroke,
                RenderTheme::for_canvas(false),
                &RenderOptions::default(),
            )
            .color,
            "#000000"
        );
        assert!(
            render_document_svg(&document, &RenderOptions::default())[0]
                .svg
                .contains(r##"stroke="#000000""##)
        );
        let layout = layout_document(&document);
        assert!(
            super::render_layout_page_replay_svg(&document, &layout, 0, &RenderOptions::default(),)
                .unwrap()
                .svg
                .contains(r##"stroke="#000000""##)
        );

        let mut public_override = stroke.clone();
        public_override.color = Some(Color {
            r: 0x33,
            g: 0x66,
            b: 0x99,
        });
        assert_eq!(
            super::stroke_paint(&public_override, false).color,
            "#336699"
        );
        assert_eq!(
            super::prepare_stroke_for_page(&public_override, false).color,
            "#336699"
        );

        let mut saved_override = stroke.clone();
        saved_override.color = None;
        saved_override.rendering.as_mut().unwrap().style.color_argb = Some(0xff12_3456);
        assert_eq!(
            super::stroke_source_color(&saved_override),
            Some(Color {
                r: 0x12,
                g: 0x34,
                b: 0x56
            })
        );

        let mut generic = stroke.clone();
        generic.rendering.as_mut().unwrap().metadata = None;
        assert_eq!(super::stroke_paint(&generic, false).color, "#1a1a1a");

        let mut unknown_version = stroke.clone();
        unknown_version
            .rendering
            .as_mut()
            .unwrap()
            .advanced_settings = Some("4;".into());
        assert_eq!(
            super::stroke_paint(&unknown_version, false).color,
            "#1a1a1a"
        );
    }

    #[test]
    fn opt_in_chinesebrush_paints_parsed_records_and_keeps_atomic_fallback() {
        let document = crate::parse_bytes(include_bytes!(
            "../tests/assets/chinesebrush/synthetic_v3.sdocx"
        ))
        .unwrap();
        let profile = ChineseBrushRenderProfile {
            screen_width: 1920,
            screen_height: 1080,
        };
        let options = RenderOptions {
            chinesebrush_profile: Some(profile),
            ..RenderOptions::default()
        };
        let paint = render_document_svg(&document, &options).remove(0).svg;
        let paint_xml = roxmltree::Document::parse(&paint).unwrap();
        let images = paint_xml
            .descendants()
            .filter(|node| node.has_tag_name("image"))
            .collect::<Vec<_>>();
        assert_eq!(images.len(), 7, "the native atlas has seven used cells");
        assert!(images.iter().all(|image| {
            image
                .attribute("href")
                .is_some_and(|href| href.starts_with("data:image/png;base64,"))
                && image
                    .attribute("image-rendering")
                    .is_some_and(|value| value == "optimizeSpeed")
                && image
                    .attribute("style")
                    .is_some_and(|value| value.contains("image-rendering:pixelated"))
        }));
        let stamps = paint_xml
            .descendants()
            .filter(|node| node.has_tag_name("use"))
            .collect::<Vec<_>>();
        assert_eq!(stamps.len(), 33, "one ordered reference per native dab");
        let oracle = include_bytes!("../tests/assets/chinesebrush/native_packed.bin");
        assert_eq!(oracle.len(), stamps.len() * 16);
        let definition_ids = images
            .iter()
            .map(|image| image.attribute("id").unwrap())
            .collect::<Vec<_>>();
        for (stamp, record) in stamps.iter().zip(oracle.as_chunks::<16>().0) {
            let lane = |index: usize| {
                f64::from(f32::from_le_bytes(
                    record[index * 4..index * 4 + 4].try_into().unwrap(),
                ))
            };
            let group = stamp.parent_element().unwrap();
            let transform = group
                .attribute("transform")
                .unwrap()
                .parse::<svgtypes::Transform>()
                .unwrap();
            assert_eq!(
                [
                    transform.a,
                    transform.b,
                    transform.c,
                    transform.d,
                    transform.e,
                    transform.f
                ],
                [lane(2), 0.0, 0.0, lane(2), lane(0), lane(1)],
                "public paint must retain native dab geometry and order"
            );
            let tile = lane(3).floor() as usize;
            assert_eq!(
                stamp.attribute("href"),
                Some(format!("#{}", definition_ids[tile]).as_str())
            );
            assert!(stamp.attribute("clip-path").is_some());
        }
        let render_with_size = |size| {
            let mut sized = document.clone();
            sized.pages[0]
                .strokes_mut()
                .next()
                .unwrap()
                .rendering
                .as_mut()
                .unwrap()
                .style
                .pen_size = Some(size);
            render_document_svg(&sized, &options).remove(0).svg
        };
        assert_eq!(render_with_size(0.0), render_with_size(0.4));
        assert_eq!(render_with_size(900.0), render_with_size(800.0));

        let layout = layout_document(&document);
        let replay = super::render_layout_page_replay_svg(&document, &layout, 0, &options)
            .unwrap()
            .svg;
        let replay_xml = roxmltree::Document::parse(&replay).unwrap();
        let replay_stamps = replay_xml
            .descendants()
            .filter(|node| node.has_tag_name("g") && node.attribute("data-replay-part").is_some())
            .collect::<Vec<_>>();
        assert_eq!(replay_stamps.len(), 33);

        let generic = render_document_svg(&document, &RenderOptions::default())
            .remove(0)
            .svg;
        let generic_xml = roxmltree::Document::parse(&generic).unwrap();
        assert_eq!(
            generic_xml
                .descendants()
                .filter(|node| node.has_tag_name("image"))
                .count(),
            0,
            "the route remains opt-in"
        );
        assert_eq!(
            generic_xml
                .descendants()
                .filter(|node| node.has_tag_name("use"))
                .count(),
            0,
            "generic fallback has no atlas references"
        );

        let fwidth_options = RenderOptions {
            chinesebrush_profile: Some(profile),
            chinesebrush_fwidth_output_profile: Some(ChineseBrushFwidthOutputProfile {
                pixels_per_page_unit: 10.0,
            }),
            ..RenderOptions::default()
        };
        let fwidth_svg = render_document_svg(&document, &fwidth_options)
            .remove(0)
            .svg;
        let fwidth_xml = roxmltree::Document::parse(&fwidth_svg).unwrap();
        let fwidth_stamps = fwidth_xml
            .descendants()
            .filter(|node| node.has_tag_name("use"))
            .collect::<Vec<_>>();
        assert_eq!(fwidth_stamps.len(), 33);
        assert!(fwidth_stamps.iter().all(|stamp| {
            stamp.attribute("mask").is_some() && stamp.attribute("clip-path").is_none()
        }));
        let masks = fwidth_xml
            .descendants()
            .filter(|node| node.has_tag_name("mask"))
            .collect::<Vec<_>>();
        let distinct_mask_refs = fwidth_stamps
            .iter()
            .map(|stamp| stamp.attribute("mask").unwrap())
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(masks.len(), distinct_mask_refs.len());
        assert!(masks.iter().all(|mask| {
            mask.children()
                .filter(|child| child.has_tag_name("path"))
                .count()
                == 64
                && mask.children().any(|child| child.has_tag_name("circle"))
        }));
        assert_eq!(
            fwidth_xml
                .descendants()
                .filter(|node| node.has_tag_name("radialGradient"))
                .count(),
            masks.len() * 64,
            "masks are reused exactly by source dab radius"
        );

        for invalid_scale in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            let invalid = RenderOptions {
                chinesebrush_profile: Some(profile),
                chinesebrush_fwidth_output_profile: Some(ChineseBrushFwidthOutputProfile {
                    pixels_per_page_unit: invalid_scale,
                }),
                ..RenderOptions::default()
            };
            assert_eq!(
                render_document_svg(&document, &invalid).remove(0).svg,
                generic,
                "invalid explicit output scales fall back atomically"
            );
        }

        let mut budget_case = document.clone();
        let stroke = budget_case.pages[0].strokes_mut().next().unwrap();
        stroke.points = (0..512)
            .map(|index| Point {
                x: f64::from(index) * 16.0,
                y: 100.0 + f64::from((index % 7) * 3) * 0.01,
            })
            .collect();
        stroke.pressures = (0..512)
            .map(|index| 0.1 + f64::from((index * 37) % 512) * (0.8 / 512.0))
            .collect();
        stroke.timestamps = (0..512).map(|index| i64::from(index * 10)).collect();
        let rendering = stroke.rendering.as_ref().unwrap();
        let source_points = stroke
            .points
            .iter()
            .map(|point| [point.x as f32, point.y as f32])
            .collect::<Vec<_>>();
        let source_pressures = stroke
            .pressures
            .iter()
            .map(|pressure| *pressure as f32)
            .collect::<Vec<_>>();
        let source_times = stroke
            .timestamps
            .iter()
            .map(|time| i32::try_from(*time).unwrap())
            .collect::<Vec<_>>();
        let source_samples = crate::ink::chinesebrush_glv123::run_model_bounded_with_sample_ends(
            &source_points,
            &source_pressures,
            &source_times,
            rendering.style.pen_size.unwrap(),
            3,
            profile.screen_width as i32,
            profile.screen_height as i32,
            i32::from(rendering.tool_type_raw),
            rendering.properties.compressed,
            32_768,
        )
        .unwrap();
        let unique_radii = source_samples
            .records
            .iter()
            .map(|record| record[2].to_bits())
            .collect::<std::collections::HashSet<_>>();
        assert!(
            unique_radii.len() > 128,
            "fixture exercises unique-mask budget"
        );
        assert!(
            super::chinesebrush::prepare(
                budget_case.pages[0].strokes().next().unwrap(),
                profile,
                None,
            )
            .is_some(),
            "the hard-circle control admits this otherwise-supported stroke"
        );
        assert!(
            super::chinesebrush::prepare(
                budget_case.pages[0].strokes().next().unwrap(),
                profile,
                fwidth_options.chinesebrush_fwidth_output_profile,
            )
            .is_none(),
            "more than 128 distinct masks exceed the pre-emission budget"
        );
        let budget_generic = render_document_svg(&budget_case, &RenderOptions::default())
            .remove(0)
            .svg;
        assert_eq!(
            render_document_svg(&budget_case, &fwidth_options)
                .remove(0)
                .svg,
            budget_generic,
            "an over-budget Fwidth stroke falls back atomically"
        );

        let mut unsupported = document.clone();
        unsupported.pages[0]
            .strokes_mut()
            .next()
            .unwrap()
            .rendering
            .as_mut()
            .unwrap()
            .advanced_settings = Some("5;".into());
        let fallback = render_document_svg(&unsupported, &options).remove(0).svg;
        let fallback_xml = roxmltree::Document::parse(&fallback).unwrap();
        assert_eq!(
            fallback_xml
                .descendants()
                .filter(|node| node.has_tag_name("image"))
                .count(),
            0,
            "unsupported settings fall back for the whole stroke"
        );

        let mut over_budget = document.clone();
        let stroke = over_budget.pages[0].strokes_mut().next().unwrap();
        stroke.points = vec![
            Point { x: 0.0, y: 0.0 },
            Point {
                x: 100_000_000.0,
                y: 0.0,
            },
        ];
        stroke.pressures = vec![1.0, 1.0];
        stroke.timestamps = vec![0, 100];
        let rendering = stroke.rendering.as_mut().unwrap();
        rendering.properties.compressed = false;
        rendering.style.pen_size = Some(800.0);
        let bounded_fallback = render_document_svg(&over_budget, &options).remove(0).svg;
        let bounded_xml = roxmltree::Document::parse(&bounded_fallback).unwrap();
        assert_eq!(
            bounded_xml
                .descendants()
                .filter(|node| node.has_tag_name("image"))
                .count(),
            0,
            "bounded record generation must fall back atomically"
        );
    }

    #[cfg(feature = "serde")]
    #[test]
    fn public_fountain_missing_color_uses_native_default_without_changing_other_fallbacks() {
        let fountain_rgb = |svg: &str| {
            let xml = roxmltree::Document::parse(svg).unwrap();
            let Some(matrix) = xml
                .descendants()
                .find(|node| node.has_tag_name("feColorMatrix"))
            else {
                let colors = xml
                    .descendants()
                    .filter(|node| node.has_tag_name("stop"))
                    .map(|node| node.attribute("stop-color").unwrap())
                    .collect::<Vec<_>>();
                let color = colors.first().expect("V16 colored radial gradient stops");
                assert!(colors.iter().all(|candidate| candidate == color));
                let color = color.strip_prefix('#').unwrap();
                assert_eq!(color.len(), 6);
                return [0, 2, 4].map(|offset| {
                    f64::from(u8::from_str_radix(&color[offset..offset + 2], 16).unwrap()) / 255.
                });
            };
            let matrix = matrix.attribute("values").unwrap();
            let values = matrix
                .split_whitespace()
                .map(|value| value.parse::<f64>().unwrap())
                .collect::<Vec<_>>();
            assert_eq!(values.len(), 20);
            [values[4], values[9], values[14]]
        };

        for setting in ["14;", "18;0;100;"] {
            for (mode, expected) in [
                (RenderColorMode::Light, [0., 0., 0.]),
                (RenderColorMode::Dark, [1., 1., 1.]),
            ] {
                let mut page = page_with_uncolored_stroke();
                page.objects = vec![fountain_color_stroke(setting).into()];
                let svg = render_document_svg(
                    &document(page),
                    &RenderOptions {
                        color_mode: mode,
                        ..RenderOptions::default()
                    },
                )
                .remove(0)
                .svg;
                assert_eq!(fountain_rgb(&svg), expected, "{setting}, {mode:?}");
            }
        }

        let mut explicit = fountain_color_stroke("18;0;100;");
        explicit.color = Some(Color {
            r: 0x33,
            g: 0x66,
            b: 0x99,
        });
        explicit.rendering.as_mut().unwrap().style.color_argb = Some(0xff33_6699);
        let mut page = page_with_uncolored_stroke();
        page.objects = vec![explicit.into()];
        let svg = render_document_svg(&document(page), &RenderOptions::default())
            .remove(0)
            .svg;
        let actual = fountain_rgb(&svg);
        for (actual, expected) in actual.into_iter().zip([51., 102., 153.].map(|v| v / 255.)) {
            assert!((actual - expected).abs() < 1e-6);
        }

        let mut unsupported = fountain_color_stroke("17;0;100;");
        let mut page = page_with_uncolored_stroke();
        page.objects = vec![unsupported.clone().into()];
        let svg = render_document_svg(&document(page), &RenderOptions::default())
            .remove(0)
            .svg;
        assert!(svg.contains(r##"stroke="#1a1a1a""##));
        assert!(!svg.contains("<feColorMatrix"));

        unsupported.rendering = None;
        let mut page = page_with_uncolored_stroke();
        page.objects = vec![unsupported.into()];
        let svg = render_document_svg(&document(page), &RenderOptions::default())
            .remove(0)
            .svg;
        assert!(svg.contains(r##"stroke="#1a1a1a""##));
    }

    #[cfg(feature = "serde")]
    #[test]
    fn fountain_v17_rainbow_is_explicit_and_invalid_profiles_fall_back_atomically() {
        let mut stroke = fountain_color_stroke("18;0;100;");
        stroke.rendering.as_mut().unwrap().properties.rainbow_effect = true;
        let mut page = page_with_uncolored_stroke();
        page.objects = vec![stroke.into()];
        let fountain_document = document(page);

        let plain_glv16 = render_document_svg(&fountain_document, &RenderOptions::default())
            .remove(0)
            .svg;
        assert!(plain_glv16.contains("radialGradient"));
        let mut unsupported_fallback_stroke = fountain_color_stroke("17;0;100;");
        unsupported_fallback_stroke
            .rendering
            .as_mut()
            .unwrap()
            .properties
            .rainbow_effect = true;
        let mut unsupported_page = page_with_uncolored_stroke();
        unsupported_page.objects = vec![unsupported_fallback_stroke.into()];
        let generic_fallback =
            render_document_svg(&document(unsupported_page), &RenderOptions::default())
                .remove(0)
                .svg;
        assert!(
            roxmltree::Document::parse(&generic_fallback)
                .unwrap()
                .descendants()
                .any(|node| node.has_tag_name(("http://www.w3.org/2000/svg", "line")))
        );
        assert!(!generic_fallback.contains("radialGradient"));

        let invalid = render_document_svg(
            &fountain_document,
            &RenderOptions {
                fountain_v17_rainbow_profile: Some(FountainV17RainbowRenderProfile {
                    palette_argb: vec![0x40e0_2010, 0x8010_20e0, 0x7f40_f010],
                    period: 3,
                    initial_offset: f32::NAN,
                }),
                ..RenderOptions::default()
            },
        )
        .remove(0)
        .svg;
        assert_eq!(
            invalid, generic_fallback,
            "invalid explicit values use whole-stroke fallback"
        );
        assert_ne!(invalid, plain_glv16);

        let output_scale_one = render_document_svg(
            &fountain_document,
            &RenderOptions {
                fountain_v17_rainbow_profile: Some(FountainV17RainbowRenderProfile {
                    palette_argb: vec![0x40e0_2010, 0x8010_20e0, 0x7f40_f010],
                    period: 3,
                    initial_offset: 0.125,
                }),
                fountain_v17_output_scale_profile: Some(FountainV17OutputScaleProfile {
                    pixels_per_page_unit: 1.0,
                }),
                ..RenderOptions::default()
            },
        )
        .remove(0)
        .svg;
        let unscaled_explicit = render_document_svg(
            &fountain_document,
            &RenderOptions {
                fountain_v17_rainbow_profile: Some(FountainV17RainbowRenderProfile {
                    palette_argb: vec![0x40e0_2010, 0x8010_20e0, 0x7f40_f010],
                    period: 3,
                    initial_offset: 0.125,
                }),
                ..RenderOptions::default()
            },
        )
        .remove(0)
        .svg;
        assert_eq!(output_scale_one, unscaled_explicit);
        let v17_scale_with_v16_profile = render_document_svg(
            &fountain_document,
            &RenderOptions {
                fountain_v17_rainbow_profile: Some(FountainV17RainbowRenderProfile {
                    palette_argb: vec![0x40e0_2010, 0x8010_20e0, 0x7f40_f010],
                    period: 3,
                    initial_offset: 0.125,
                }),
                fountain_v17_output_scale_profile: Some(FountainV17OutputScaleProfile {
                    pixels_per_page_unit: 2.0,
                }),
                fountain_v16_output_scale_profile: Some(FountainV16OutputScaleProfile {
                    pixels_per_page_unit: 0.5,
                }),
                ..RenderOptions::default()
            },
        )
        .remove(0)
        .svg;
        let v17_scale_only = render_document_svg(
            &fountain_document,
            &RenderOptions {
                fountain_v17_rainbow_profile: Some(FountainV17RainbowRenderProfile {
                    palette_argb: vec![0x40e0_2010, 0x8010_20e0, 0x7f40_f010],
                    period: 3,
                    initial_offset: 0.125,
                }),
                fountain_v17_output_scale_profile: Some(FountainV17OutputScaleProfile {
                    pixels_per_page_unit: 2.0,
                }),
                ..RenderOptions::default()
            },
        )
        .remove(0)
        .svg;
        assert_eq!(v17_scale_with_v16_profile, v17_scale_only);

        let invalid_output_scale = render_document_svg(
            &fountain_document,
            &RenderOptions {
                fountain_v17_rainbow_profile: Some(FountainV17RainbowRenderProfile {
                    palette_argb: vec![0x40e0_2010, 0x8010_20e0, 0x7f40_f010],
                    period: 3,
                    initial_offset: 0.125,
                }),
                fountain_v17_output_scale_profile: Some(FountainV17OutputScaleProfile {
                    pixels_per_page_unit: f32::NAN,
                }),
                ..RenderOptions::default()
            },
        )
        .remove(0)
        .svg;
        assert_eq!(invalid_output_scale, generic_fallback);
        let mut wide_stroke = fountain_color_stroke("18;0;100;");
        wide_stroke.pen_width = 20.0;
        wide_stroke.rendering.as_mut().unwrap().style.pen_size = Some(20.0);
        wide_stroke
            .rendering
            .as_mut()
            .unwrap()
            .properties
            .rainbow_effect = true;
        let wide_prepared = crate::ink::prepare_stroke_for_context_with_fountain_v17_rainbow(
            &wide_stroke,
            crate::ink::InkRenderContext::CanonicalPage,
            &[0x40e0_2010, 0x8010_20e0, 0x7f40_f010],
            3,
            0.125,
            None,
        );
        assert!(
            wide_prepared
                .dot_radii
                .as_ref()
                .unwrap()
                .iter()
                .any(|radius| *radius > 1.0)
        );
        let mut wide_page = page_with_uncolored_stroke();
        wide_page.objects = vec![wide_stroke.into()];
        let wide_document = document(wide_page);
        let wide_generic = render_document_svg(
            &wide_document,
            &RenderOptions {
                fountain_v17_rainbow_profile: Some(FountainV17RainbowRenderProfile {
                    palette_argb: vec![0x40e0_2010, 0x8010_20e0, 0x7f40_f010],
                    period: 3,
                    initial_offset: f32::NAN,
                }),
                ..RenderOptions::default()
            },
        )
        .remove(0)
        .svg;
        let overflowing_derived_scale = render_document_svg(
            &wide_document,
            &RenderOptions {
                fountain_v17_rainbow_profile: Some(FountainV17RainbowRenderProfile {
                    palette_argb: vec![0x40e0_2010, 0x8010_20e0, 0x7f40_f010],
                    period: 3,
                    initial_offset: 0.125,
                }),
                fountain_v17_output_scale_profile: Some(FountainV17OutputScaleProfile {
                    pixels_per_page_unit: f32::MAX,
                }),
                ..RenderOptions::default()
            },
        )
        .remove(0)
        .svg;
        assert_eq!(overflowing_derived_scale, wide_generic);

        let mut ordinary = fountain_color_stroke("18;0;100;");
        ordinary
            .rendering
            .as_mut()
            .unwrap()
            .properties
            .rainbow_effect = false;
        let mut ordinary_page = page_with_uncolored_stroke();
        ordinary_page.objects = vec![ordinary.clone().into()];
        let ordinary_document = document(ordinary_page);
        let ordinary_default = render_document_svg(&ordinary_document, &RenderOptions::default())
            .remove(0)
            .svg;
        let mut rainbow_flag_control = ordinary.clone();
        rainbow_flag_control
            .rendering
            .as_mut()
            .unwrap()
            .properties
            .rainbow_effect = true;
        let mut rainbow_flag_page = page_with_uncolored_stroke();
        rainbow_flag_page.objects = vec![rainbow_flag_control.into()];
        let rainbow_flag_default =
            render_document_svg(&document(rainbow_flag_page), &RenderOptions::default())
                .remove(0)
                .svg;
        assert_eq!(rainbow_flag_default, ordinary_default);
        let stop_colors = |svg: &str| {
            roxmltree::Document::parse(svg)
                .unwrap()
                .descendants()
                .filter(|node| node.has_tag_name("stop"))
                .filter_map(|node| node.attribute("stop-color").map(str::to_owned))
                .collect::<std::collections::BTreeSet<_>>()
        };
        for (mode, expected) in [
            (RenderColorMode::Light, "#000000"),
            (RenderColorMode::Dark, "#ffffff"),
        ] {
            let themed = render_document_svg(
                &document({
                    let mut page = page_with_uncolored_stroke();
                    page.objects = vec![ordinary.clone().into()];
                    page
                }),
                &RenderOptions {
                    color_mode: mode,
                    ..RenderOptions::default()
                },
            )
            .remove(0)
            .svg;
            let mut themed_rainbow = ordinary.clone();
            themed_rainbow
                .rendering
                .as_mut()
                .unwrap()
                .properties
                .rainbow_effect = true;
            let mut themed_page = page_with_uncolored_stroke();
            themed_page.objects = vec![themed_rainbow.into()];
            let rainbow_svg = render_document_svg(
                &document(themed_page),
                &RenderOptions {
                    color_mode: mode,
                    ..RenderOptions::default()
                },
            )
            .remove(0)
            .svg;
            assert_eq!(
                stop_colors(&rainbow_svg),
                stop_colors(&themed),
                "rainbow-flag GLV16 theme {mode:?}"
            );
            assert_eq!(
                stop_colors(&rainbow_svg),
                [expected.to_owned()]
                    .into_iter()
                    .collect::<std::collections::BTreeSet<_>>()
            );
        }

        let replay_layout = layout_document(&fountain_document);
        let replay = super::render_layout_page_replay_svg(
            &fountain_document,
            &replay_layout,
            0,
            &RenderOptions::default(),
        )
        .unwrap()
        .svg;
        let replay_xml = roxmltree::Document::parse(&replay).unwrap();
        assert!(
            replay_xml
                .descendants()
                .any(|node| node.attribute("data-replay-stroke") == Some("0"))
        );
        assert!(
            replay_xml
                .descendants()
                .any(|node| node.attribute("data-replay-part").is_some())
        );
        assert!(replay.contains("radialGradient"));

        #[cfg(feature = "pdf")]
        {
            let request = [
                super::PageSceneRequest::sliced(&replay_layout.pages[0], 80, 10, false).unwrap(),
            ];
            let mut cache = super::DocumentTextCache::default();
            let mut progress = |_: crate::Progress| {};
            let sliced = cache
                .render_layout_page_scene_requests(
                    &fountain_document,
                    &request,
                    &RenderOptions::default(),
                    &crate::render::fonts::FontBook::default(),
                    &mut progress,
                )
                .remove(0)
                .page
                .svg;
            assert!(!sliced.contains("radialGradient"));
            assert!(!sliced.contains("<polyline"));
        }
        let ordinary_scaled = render_document_svg(
            &ordinary_document,
            &RenderOptions {
                fountain_v17_output_scale_profile: Some(FountainV17OutputScaleProfile {
                    pixels_per_page_unit: 4.0,
                }),
                ..RenderOptions::default()
            },
        )
        .remove(0)
        .svg;
        assert_eq!(ordinary_scaled, ordinary_default);

        let options = RenderOptions {
            fountain_v17_rainbow_profile: Some(FountainV17RainbowRenderProfile {
                palette_argb: vec![0x40e0_2010, 0x8010_20e0, 0x7f40_f010],
                period: 3,
                initial_offset: 0.125,
            }),
            ..RenderOptions::default()
        };
        let actual = render_document_svg(&fountain_document, &options)
            .remove(0)
            .svg;
        let xml = roxmltree::Document::parse(&actual).unwrap();
        let colors = xml
            .descendants()
            .filter(|node| node.has_tag_name("stop"))
            .filter_map(|node| node.attribute("stop-color"))
            .collect::<std::collections::BTreeSet<_>>();
        assert!(
            colors.len() > 3,
            "per-stamp RGB retains fractional channels"
        );
        assert!(
            xml.descendants()
                .any(|node| node.has_tag_name("radialGradient"))
        );

        let layout = layout_document(&fountain_document);
        let replay = super::render_layout_page_replay_svg(&fountain_document, &layout, 0, &options)
            .unwrap()
            .svg;
        let replay_xml = roxmltree::Document::parse(&replay).unwrap();
        let parts = replay_xml
            .descendants()
            .filter_map(|node| node.attribute("data-replay-part"))
            .collect::<Vec<_>>();
        assert!(!parts.is_empty());
        assert!(parts.iter().all(|part| part.parse::<usize>().unwrap() > 0));

        #[cfg(feature = "pdf")]
        {
            let bytes = crate::render_document_pdf(
                &fountain_document,
                &options,
                &crate::PdfOptions::default(),
            )
            .unwrap();
            let pdf = lopdf::Document::load_mem(&bytes).unwrap();
            assert_eq!(pdf.get_pages().len(), 1);
            let dictionaries = pdf.objects.values().filter_map(pdf_object_dictionary);
            let mut radial_shadings = 0;
            let mut initial_palette_color = false;
            for dictionary in dictionaries {
                // The first native V17 color is fractional RGB. The PDF SVG
                // parser rounds it to bytes, so check the actual transported
                // color as well as the presence of vector shading.
                for key in [b"C0", b"C1"] {
                    if let Ok(channels) = dictionary.get(key).and_then(lopdf::Object::as_array)
                        && channels.len() == 3
                    {
                        initial_palette_color |=
                            channels
                                .iter()
                                .zip([198., 32., 42.])
                                .all(|(channel, byte)| {
                                    channel
                                        .as_float()
                                        .is_ok_and(|value| (value - byte / 255.).abs() < 0.00002)
                                });
                    }
                }
                assert_ne!(
                    dictionary
                        .get(b"Subtype")
                        .and_then(lopdf::Object::as_name)
                        .ok(),
                    Some(b"Image".as_slice()),
                    "the explicit rainbow profile must retain vector paint"
                );
                radial_shadings += usize::from(
                    dictionary
                        .get(b"ShadingType")
                        .and_then(lopdf::Object::as_i64)
                        .ok()
                        == Some(3),
                );
            }
            assert!(
                radial_shadings > 0,
                "stamp coverage must remain vector shading"
            );
            assert!(
                initial_palette_color,
                "PDF must retain the initial palette color"
            );
        }
    }

    #[cfg(feature = "serde")]
    #[test]
    fn fountain_v17_profile_serde_preserves_legacy_default_and_explicit_controls() {
        let mut legacy = serde_json::to_value(RenderOptions::default()).unwrap();
        legacy
            .as_object_mut()
            .unwrap()
            .remove("fountain_v17_rainbow_profile");
        legacy
            .as_object_mut()
            .unwrap()
            .remove("fountain_v17_output_scale_profile");
        legacy
            .as_object_mut()
            .unwrap()
            .remove("fountain_v16_output_scale_profile");
        let restored: RenderOptions = serde_json::from_value(legacy).unwrap();
        assert!(restored.fountain_v17_rainbow_profile.is_none());
        assert!(restored.fountain_v17_output_scale_profile.is_none());
        assert!(restored.fountain_v16_output_scale_profile.is_none());
        let profile = FountainV17RainbowRenderProfile {
            palette_argb: vec![0x40e0_2010, 0x8010_20e0, 0x7f40_f010],
            period: 3,
            initial_offset: 0.125,
        };
        let restored: FountainV17RainbowRenderProfile =
            serde_json::from_value(serde_json::to_value(&profile).unwrap()).unwrap();
        assert_eq!(restored, profile);
        let output_scale = FountainV17OutputScaleProfile {
            pixels_per_page_unit: 2.0,
        };
        let restored: FountainV17OutputScaleProfile =
            serde_json::from_value(serde_json::to_value(output_scale).unwrap()).unwrap();
        assert_eq!(restored, output_scale);
        let output_scale = FountainV16OutputScaleProfile {
            pixels_per_page_unit: 0.5,
        };
        let restored: FountainV16OutputScaleProfile =
            serde_json::from_value(serde_json::to_value(output_scale).unwrap()).unwrap();
        assert_eq!(restored, output_scale);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn fountain_svg_keeps_reconstructed_ink_as_vector_primitives() {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../../../conformance/fountain-v14.json")).unwrap();
        let mut stroke: Stroke = serde_json::from_value(reference["stroke"].clone()).unwrap();
        stroke.points = vec![
            crate::Point { x: 10., y: 10. },
            crate::Point { x: 30., y: 20. },
            crate::Point { x: 40., y: 40. },
        ];
        stroke.pressures = vec![0.2, 0.7, 0.4];
        stroke.timestamps = vec![0, 10, 20];
        stroke.tilts = vec![0.4, 0.6, 0.5];
        for (tool, fixed_width) in [1, 2, 3]
            .into_iter()
            .flat_map(|tool| [false, true].map(|fixed| (tool, fixed)))
        {
            stroke.rendering.as_mut().unwrap().tool_type_raw = tool;
            stroke.rendering.as_mut().unwrap().properties.fixed_width = fixed_width;
            assert!(
                crate::prepare_stroke(&stroke, false)
                    .dot_directions
                    .is_some()
            );
            let mut page = page_with_uncolored_stroke();
            page.objects = vec![stroke.clone().into()];
            let rendered = render_document_svg(&document(page), &RenderOptions::default());
            assert!(rendered[0].svg.contains("<linearGradient"));
            assert!(rendered[0].svg.contains("<circle "));
            assert!(!rendered[0].svg.contains("<image"));
            assert_eq!(rendered[0].svg.matches("<feColorMatrix").count(), 1);
        }
    }

    #[cfg(feature = "serde")]
    #[test]
    fn replay_annotations_preserve_fountain_and_highlighter_composition() {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../../../conformance/fountain-v14.json")).unwrap();
        let mut fountain: Stroke = serde_json::from_value(reference["stroke"].clone()).unwrap();
        fountain.points = vec![
            crate::Point { x: 10., y: 10. },
            crate::Point { x: 60., y: 40. },
        ];
        fountain.pressures = vec![0.2, 0.8];
        fountain.timestamps = vec![0, 10];
        let mut page = page_with_uncolored_stroke();
        page.objects.push(fountain.into());
        page.objects.push(marker(true, 20.).into());
        let doc = document(page);
        let layout = crate::layout_document(&doc);
        for color_mode in [RenderColorMode::Light, RenderColorMode::Dark] {
            let options = RenderOptions {
                color_mode,
                ..Default::default()
            };
            let normal = super::render_layout_page_svg(&doc, &layout, 0, &options).unwrap();
            let replay = super::render_layout_page_replay_svg(&doc, &layout, 0, &options).unwrap();
            let preview = |svg: &str| {
                let tree =
                    resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default()).unwrap();
                let mut image = resvg::tiny_skia::Pixmap::new(normal.width, normal.height).unwrap();
                resvg::render(
                    &tree,
                    resvg::tiny_skia::Transform::identity(),
                    &mut image.as_mut(),
                );
                image
            };
            assert_eq!(preview(&normal.svg).data(), preview(&replay.svg).data());
            assert_eq!(replay.svg.matches("data-replay-stroke=").count(), 3);
            assert!(replay.svg.contains("data-replay-part="));
            assert!(replay.svg.contains("data-replay-lengths="));
            assert!(!replay.svg.contains("<image"));
            assert_eq!(replay.svg.matches("<feColorMatrix").count(), 1);
        }
    }

    #[test]
    fn explicit_color_modes_select_matching_ink_and_canvas() {
        let doc = document(page_with_uncolored_stroke());
        let light = render_document_svg(
            &doc,
            &RenderOptions {
                color_mode: RenderColorMode::Light,
                ..Default::default()
            },
        );
        let dark = render_document_svg(
            &doc,
            &RenderOptions {
                color_mode: RenderColorMode::Dark,
                ..Default::default()
            },
        );

        assert!(light[0].svg.contains(r##"fill="#fcfcfc""##));
        assert!(light[0].svg.contains(r##"stroke="#1a1a1a""##));
        assert!(dark[0].svg.contains(r##"fill="#252525""##));
        assert!(dark[0].svg.contains(r##"stroke="#ffffff""##));
    }

    #[test]
    fn stored_backgrounds_and_foregrounds_resolve_together() {
        let gray = |v| Color { r: v, g: v, b: v };
        for (
            page_background,
            document_background,
            compatibility,
            mode,
            expected_bg,
            expected_ink,
        ) in [
            (
                Some(gray(252)),
                Some(gray(252)),
                Some(true),
                RenderColorMode::Dark,
                "#252525",
                "#ffffff",
            ),
            (
                Some(gray(37)),
                Some(gray(37)),
                Some(true),
                RenderColorMode::Light,
                "#fcfcfc",
                "#1a1a1a",
            ),
            (
                Some(gray(0)),
                Some(gray(255)),
                Some(true),
                RenderColorMode::Auto,
                "#000000",
                "#ffffff",
            ),
            (
                Some(gray(255)),
                Some(gray(0)),
                Some(true),
                RenderColorMode::Auto,
                "#ffffff",
                "#1a1a1a",
            ),
            (
                None,
                Some(gray(37)),
                None,
                RenderColorMode::Auto,
                "#252525",
                "#ffffff",
            ),
            (
                Some(gray(252)),
                None,
                Some(false),
                RenderColorMode::Dark,
                "#fcfcfc",
                "#1a1a1a",
            ),
            (
                Some(gray(37)),
                None,
                Some(false),
                RenderColorMode::Light,
                "#252525",
                "#ffffff",
            ),
            (
                Some(Color {
                    r: 203,
                    g: 218,
                    b: 221,
                }),
                None,
                Some(true),
                RenderColorMode::Dark,
                "#cbdadd",
                "#1a1a1a",
            ),
        ] {
            let mut page = page_with_uncolored_stroke();
            page.background_color = page_background;
            let mut doc = document(page);
            doc.metadata.background_color = document_background;
            doc.metadata.dark_mode_compatibility = compatibility;
            let svg = render_document_svg(
                &doc,
                &RenderOptions {
                    color_mode: mode,
                    ..Default::default()
                },
            )
            .remove(0)
            .svg;
            let xml = roxmltree::Document::parse(&svg).unwrap();
            let background = xml
                .root_element()
                .children()
                .find(|n| n.has_tag_name("rect"))
                .unwrap();
            assert_eq!(background.attribute("fill"), Some(expected_bg));
            assert!(
                xml.descendants()
                    .any(|n| n.attribute("stroke") == Some(expected_ink))
            );
        }
    }

    fn theme_test_text() -> RichTextBox {
        RichTextBox {
            text_area_type: None,
            bbox: BoundingBox {
                x_min: 10.,
                y_min: 20.,
                x_max: 95.,
                y_max: 90.,
            },
            rotation_degrees: None,
            text: "visible".into(),
            color: Some(Color { r: 0, g: 0, b: 0 }),
            highlight_color: None,
            underline: false,
            font_size: Some(12.),
            runs: vec![],
            spans: vec![],
            paragraphs: vec![],
            object_spans: vec![],
            text_sections: vec![],
            margins: None,
            gravity: None,
        }
    }

    fn preparation_document() -> Document {
        let mut body = theme_test_text();
        body.bbox = BoundingBox::default();
        body.text = "visible body ".repeat(40);
        let mut page = page_with_uncolored_stroke();
        page.objects.clear();
        let mut document = document(page.clone());
        document.pages = vec![page; 4];
        document.metadata.note_text = Some(body);
        document.metadata.default_page_dimensions = Some((360, 100));
        document.metadata.page_mode = Some(0);
        document.metadata.flow_page_padding = Some((0, 0));
        document
    }

    #[test]
    fn body_preparation_reuses_identity_across_svg_replay_and_native_pdf_paint() {
        let document = preparation_document();
        let layout = layout_document(&document);
        let fonts = super::fonts::FontBook::default();
        let options = RenderOptions::default();
        let mut cache = super::DocumentTextCache::default();
        cache.render_layout_page_svg(&document, &layout, 0, &options, &fonts);
        let plan = super::Rc::clone(&cache.plans[0].1);
        for index in 0..layout.pages.len() {
            cache.render_layout_page_svg(&document, &layout, index, &options, &fonts);
            cache.render_layout_page_replay_svg(&document, &layout, index, &options, &fonts);
            assert_eq!(cache.plans.len(), 1);
            assert!(super::Rc::ptr_eq(&plan, &cache.plans[0].1));
        }
        #[cfg(feature = "pdf")]
        {
            let pages = layout.pages.iter().collect::<Vec<_>>();
            let scenes =
                cache.render_layout_page_scenes(&document, &pages, &options, &fonts, &mut |_| {});
            assert!(scenes.iter().all(|scene| scene.text_error.is_none()));
            assert!(super::Rc::ptr_eq(&plan, &cache.plans[0].1));
        }
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn pdf_page_slices_keep_note_coordinates_canvas_policy_and_retained_text() {
        let document = preparation_document();
        let layout = layout_document(&document);
        let layout_page = &layout.pages[0];
        let fonts = super::fonts::FontBook::default();
        let options = RenderOptions::default();
        let mut cache = super::DocumentTextCache::default();

        let full_svg = cache
            .render_layout_page_svg(&document, &layout, 0, &options, &fonts)
            .unwrap();
        assert_eq!(full_svg.height, layout_page.page.height);
        let full_request = [super::PageSceneRequest::full_page(layout_page)];
        let full_scene = cache.render_layout_page_scene_requests(
            &document,
            &full_request,
            &options,
            &fonts,
            &mut |_| {},
        );
        assert_eq!(full_scene[0].page.height, layout_page.page.height);
        let full_view_box = roxmltree::Document::parse(&full_scene[0].page.svg).unwrap();
        assert_eq!(
            view_box_numbers(full_view_box.root_element()),
            vec![
                0.0,
                0.0,
                f64::from(layout_page.page.width),
                f64::from(layout_page.page.height),
            ]
        );
        assert!(
            full_scene[0].text.len() > 0,
            "fixture should retain body glyphs"
        );

        let cut_line_top = cache.plans[0]
            .1
            .layout
            .lines
            .get(1)
            .expect("fixture has multiple body lines")
            .top;
        let y = cut_line_top.floor() as u32;
        assert!(y > 0, "fixture must exercise a nonzero body slice");
        let height = (layout_page.page.height - y).min(60);
        let overlay_request =
            [super::PageSceneRequest::sliced(layout_page, y, height, false).unwrap()];
        let overlay = cache.render_layout_page_scene_requests(
            &document,
            &overlay_request,
            &options,
            &fonts,
            &mut |_| {},
        );
        assert_eq!(overlay[0].page.height, height);
        let xml = roxmltree::Document::parse(&overlay[0].page.svg).unwrap();
        assert_eq!(
            view_box_numbers(xml.root_element()),
            vec![
                0.0,
                f64::from(y),
                f64::from(layout_page.page.width),
                f64::from(height),
            ]
        );
        assert!(
            !xml.root_element()
                .children()
                .any(|node| node.has_tag_name("rect"))
        );
        assert!(
            overlay[0].text.len() > 0,
            "visible slice should retain glyphs"
        );
        let glyph_keys = |scene: &super::RenderedScene| {
            scene
                .text
                .iter()
                .flat_map(|(_, block)| block.runs.iter())
                .flat_map(|run| run.glyphs.iter())
                .map(|glyph| (glyph.glyph_id, glyph.origin, glyph.source.clone()))
                .collect::<Vec<_>>()
        };
        let full_glyphs = glyph_keys(&full_scene[0]);
        let overlay_glyphs = glyph_keys(&overlay[0]);
        assert!(
            overlay_glyphs
                .iter()
                .any(|glyph| full_glyphs.contains(glyph)),
            "visible slice retains a glyph at the same absolute note coordinate"
        );

        let legacy_full = cache.render_layout_page_scenes(
            &document,
            &[layout_page],
            &options,
            &fonts,
            &mut |_| {},
        );
        assert_eq!(legacy_full[0].page.svg, full_scene[0].page.svg);

        let tail_request = [super::PageSceneRequest::sliced(layout_page, 80, 40, true).unwrap()];
        let tail = cache.render_layout_page_scene_requests(
            &document,
            &tail_request,
            &options,
            &fonts,
            &mut |_| {},
        );
        assert_eq!(tail[0].page.height, 40);
        let xml = roxmltree::Document::parse(&tail[0].page.svg).unwrap();
        assert_eq!(
            view_box_numbers(xml.root_element()),
            vec![0.0, 80.0, f64::from(layout_page.page.width), 40.0,]
        );
        let canvas = xml
            .root_element()
            .children()
            .find(|node| node.has_tag_name("rect"))
            .expect("ordinary continuation keeps its canvas");
        assert_eq!(number_attribute(canvas, "y"), 80.0);
        assert_eq!(number_attribute(canvas, "height"), 40.0);

        let full_pdf = crate::pdf::render_scenes_pdf_with_progress(
            &full_scene,
            &crate::pdf::PdfOptions::from_font_book(&fonts),
            &mut |_| {},
        )
        .unwrap();
        let pdf = crate::pdf::render_scenes_pdf_with_progress(
            &overlay,
            &crate::pdf::PdfOptions::from_font_book(&fonts),
            &mut |_| {},
        )
        .unwrap();
        let full_origins = tagged_text_origins(&full_pdf);
        let slice_origins = tagged_text_origins(&pdf);
        let expected_y_delta =
            (f64::from(height) - f64::from(layout_page.page.height) + f64::from(y)) * 0.75;
        assert!(
            full_origins.iter().any(|(full_text, full_origin)| {
                slice_origins.iter().any(|(slice_text, slice_origin)| {
                    full_text == slice_text
                        && (full_origin[0] - slice_origin[0]).abs() < 0.02
                        && ((slice_origin[1] - full_origin[1]) - expected_y_delta).abs() < 0.02
                })
            }),
            "PDF text matrix should be rebased by the slice root viewBox"
        );
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        assert_eq!(parsed.get_pages().len(), 1);
        let root_id = parsed.trailer.get(b"Root").unwrap().as_reference().unwrap();
        assert!(
            parsed
                .get_dictionary(root_id)
                .unwrap()
                .has(b"StructTreeRoot")
        );
    }

    // Measured Block Image cases only; this does not claim a generic native query adapter.
    #[cfg(feature = "pdf")]
    #[test]
    fn continuous_body_block_image_lines_match_native_layout_cases() {
        // anchor, image height, page width, density, font delta, native top/bottom bits
        let cases = [
            (0, 100.0, 1080, 1, 0, [1092616192, 1122494185]),
            (1, 100.0, 1080, 1, 0, [1107545293, 1124787880]),
            (7, 100.0, 1080, 1, 0, [1126868582, 1133137133]),
            (1, 200.0, 1080, 1, 0, [1107545293, 1131341480]),
            (1, 681.36181640625, 1080, 1, 0, [1107545293, 1144262866]),
            (1, 1000.0, 1080, 1, 0, [1107545293, 1149361365]),
            // Native 36px / 48,30px maps to logical 17px / 16,10px at density 3, delta -5.
            (7, 681.36181640625, 1812, 3, -5, [1136204187, 1149568310]),
            (112, 681.36181640625, 1812, 3, -5, [1168837030, 1170258266]),
        ];

        for (anchor, image_height, width, density, delta, native_bits) in cases {
            let quiz = anchor == 112;
            let frame_width = if quiz { 1_080 } else { width };
            let page_height = 9_162;
            let flow_width = if quiz { 1_080 } else { width };
            let image_top = if quiz { 5_473.2021484375 } else { 100.0 };
            let mut body = theme_test_text();
            body.bbox = if quiz {
                BoundingBox::default()
            } else {
                BoundingBox {
                    x_min: 0.0,
                    y_min: 0.0,
                    x_max: f64::from(width),
                    y_max: f64::from(page_height),
                }
            };
            body.text = format!("{}\u{fffc}\n\n\n", "\n".repeat(anchor));
            body.font_size = Some(17.0);
            body.margins = Some([16.0, 10.0, 16.0, 10.0]);
            let left = 16.0 * density as f32;
            body.object_spans = vec![RichTextObjectSpan {
                object_type: ObjectType::Image,
                object_data: Vec::new(),
                content: Some(RichTextObjectContent::Image(Box::new(PlacedImage {
                    bbox: BoundingBox {
                        x_min: f64::from(left),
                        y_min: image_top,
                        x_max: f64::from(left + 519.48206),
                        y_max: image_top + image_height,
                    },
                    rotation_degrees: None,
                    media_id: Some(0),
                    media_index: Some(0),
                    crop_rect: None,
                    original_bbox: None,
                    border_media_id: None,
                    original_media_id: None,
                    fill_effect_transparency: 0.0,
                }))),
                text_index_utf16: anchor as i32,
                layout_option: ObjectSpanLayoutOption::Block,
                layout_constraint: ObjectSpanLayoutConstraint::Normal,
            }];

            let mut page = page_with_uncolored_stroke();
            page.width = frame_width;
            page.height = page_height;
            let mut document = document(page);
            document.metadata.page_mode = Some(1);
            document.metadata.default_page_dimensions = Some((360 * density, 1_527));
            document.metadata.flow_dimensions = Some((flow_width, 9_162));
            document.metadata.flow_page_padding = Some((0, 24));
            document.metadata.body_font_size_delta = Some(delta);
            document.metadata.note_text = Some(body);
            if quiz {
                document.metadata.media_assets = vec![MediaAsset {
                    name: "0.png".to_owned(),
                    archive_id: Some(0),
                    mime_type: "image/png".to_owned(),
                    data: include_bytes!("../tests/assets/crop-quadrants.png").to_vec(),
                }];
            }

            let layout = layout_document(&document);
            let fonts = super::fonts::FontBook::default();
            let mut cache = super::DocumentTextCache::default();
            let _ = cache.render_layout_page_svg(
                &document,
                &layout,
                0,
                &RenderOptions::default(),
                &fonts,
            );
            let plan = &cache
                .plans
                .first()
                .expect("continuous body plan prepared")
                .1;
            let source = &plan.text.text;
            let utf16 = source.encode_utf16().collect::<Vec<_>>();
            let to_utf16 = |i: usize| source.chars().take(i).map(char::len_utf16).sum::<usize>();
            assert_eq!(source.chars().nth(anchor), Some('\u{fffc}'));
            assert_eq!(plan.layout.lines.len(), anchor + 4);
            let (line_index, line) = plan
                .layout
                .lines
                .iter()
                .enumerate()
                .find(|(_, line)| line.line.source.start <= anchor && anchor < line.line.source.end)
                .expect("image marker belongs to an SDK line");
            assert_eq!(line_index, anchor);
            let image_range = to_utf16(line.line.source.start)..to_utf16(line.line.source.end);
            assert_eq!(image_range, anchor..anchor + 1);
            assert!(super::text::native_text_range_has_visible_content(
                &utf16,
                image_range,
                &[]
            ));
            assert_eq!(
                (line.top as f32).to_bits(),
                native_bits[0],
                "top at anchor {anchor}"
            );
            assert_eq!(
                (line.bottom as f32).to_bits(),
                native_bits[1],
                "bottom at anchor {anchor}"
            );

            let (top, bottom) = (
                f32::from_bits(native_bits[0]),
                f32::from_bits(native_bits[1]),
            );
            let visible = |a: f32, b: f32| {
                if a >= b {
                    return false;
                }
                let selected = plan
                    .layout
                    .lines
                    .iter()
                    .filter(|line| line.top < f64::from(b) && line.bottom > f64::from(a))
                    .collect::<Vec<_>>();
                let Some(first) = selected.first() else {
                    return false;
                };
                let last = selected.last().unwrap();
                super::text::native_text_range_has_visible_content(
                    &utf16,
                    to_utf16(first.line.source.start)..to_utf16(last.line.source.end),
                    &[],
                )
            };
            let mid = (top + bottom) * 0.5;
            for (a, b, expected) in [
                (top - 1.0, top, false),
                (top, top + 1.0, true),
                (top, bottom, true),
                (bottom - 1.0, bottom, true),
                (bottom, bottom + 1.0, false),
                (top, top, false),
                (mid, mid, false),
                (bottom, bottom, false),
            ] {
                assert_eq!(visible(a, b), expected, "band {a}..{b} at anchor {anchor}");
            }
            if quiz {
                assert!((line.baseline - 6_154.56884765625).abs() < 0.001);
                for (a, b, expected) in [
                    (5_424.60595703125_f64, 5_473.2060546875_f64, false),
                    (5_473.2060546875, 5_474.2060546875, true),
                    (6_065.0, 6_066.0, true),
                    (6_154.0, 6_155.0, true),
                    (6_166.1689453125, 6_167.1689453125, true),
                    (6_167.1689453125, 6_168.1689453125, false),
                ] {
                    assert_eq!(visible(a as f32, b as f32), expected, "quiz band {a}..{b}");
                }

                let cached_plan = super::Rc::clone(&cache.plans[0].1);
                let snapshot = cache
                    .continuous_body_query_snapshot(
                        &document,
                        &layout.pages[0],
                        &RenderOptions::default(),
                        &fonts,
                    )
                    .expect("bounded LF-plus-resolved-image body query snapshot");
                assert!(super::Rc::ptr_eq(&cached_plan, &cache.plans[0].1));
                let first_line = snapshot.line_rect(0).unwrap();
                assert_eq!(first_line[0].to_bits(), 0.0_f32.to_bits());
                assert_eq!(first_line[2].to_bits(), 0.0_f32.to_bits());
                let image_line = snapshot.line_rect(112).unwrap();
                assert_eq!(image_line[0].to_bits(), 48.0_f32.to_bits());
                assert_eq!(image_line[1].to_bits(), native_bits[0]);
                assert_eq!(
                    image_line[2].to_bits(),
                    f32::from_bits(0x440ddeda).to_bits()
                );
                assert_eq!(image_line[3].to_bits(), native_bits[1]);
                assert_eq!(
                    snapshot
                        .query([
                            0.0,
                            f32::from_bits(0x45ab09a6),
                            1_080.0,
                            f32::from_bits(0x45ab11a6)
                        ])
                        .line_count,
                    116
                );
                assert_eq!(
                    snapshot.line_index(f32::from_bits(native_bits[0])),
                    Some(111)
                );
                assert_eq!(
                    snapshot.line_index(f32::from_bits(native_bits[0] + 1)),
                    Some(112)
                );
                for (left, top, right, bottom, expected) in [
                    (
                        0.0,
                        f32::from_bits(0x45a984d9),
                        1_080.0,
                        f32::from_bits(0x45ab09a6),
                        false,
                    ),
                    (
                        0.0,
                        f32::from_bits(0x45ab09a6),
                        1_080.0,
                        f32::from_bits(0x45ab11a6),
                        true,
                    ),
                    (0.0, 6_155.0, 1_080.0, 6_160.0, true),
                    (
                        0.0,
                        f32::from_bits(0x45c0b15a),
                        1_080.0,
                        f32::from_bits(0x45c0b95a),
                        true,
                    ),
                    (
                        0.0,
                        f32::from_bits(0x45c0b95a),
                        1_080.0,
                        f32::from_bits(0x45c0c15a),
                        false,
                    ),
                    (0.0, 6_170.0, 1_080.0, 6_180.0, false),
                    (2_000.0, 5_500.0, 2_010.0, 5_510.0, false),
                ] {
                    assert_eq!(
                        snapshot.query([left, top, right, bottom]).visible,
                        expected,
                        "native body query band ({left}, {top}, {right}, {bottom})"
                    );
                }
            }
        }
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn continuous_body_two_block_images_keep_one_sdk_owner_per_anchor() {
        let mut body = theme_test_text();
        body.bbox = BoundingBox::default();
        body.text = "\u{fffc}\n\u{fffc}".into();
        body.font_size = Some(20.0);
        body.margins = Some([0.0; 4]);
        body.object_spans = [(0_i32, 0.0_f64, 0_u32), (2, 24.0, 1)]
            .into_iter()
            .map(|(anchor, x, media_id)| RichTextObjectSpan {
                object_type: ObjectType::Image,
                object_data: Vec::new(),
                content: Some(RichTextObjectContent::Image(Box::new(PlacedImage {
                    bbox: BoundingBox {
                        x_min: x,
                        y_min: 0.0,
                        x_max: x + 20.0,
                        y_max: 20.0,
                    },
                    rotation_degrees: None,
                    media_id: Some(media_id),
                    media_index: Some(media_id as usize),
                    crop_rect: None,
                    original_bbox: None,
                    border_media_id: None,
                    original_media_id: None,
                    fill_effect_transparency: 0.0,
                }))),
                text_index_utf16: anchor,
                layout_option: ObjectSpanLayoutOption::Block,
                layout_constraint: ObjectSpanLayoutConstraint::Normal,
            })
            .collect();

        let mut page = page_with_uncolored_stroke();
        page.width = 200;
        page.height = 200;
        page.objects.clear();
        let mut document = document(page);
        document.metadata.page_mode = Some(1);
        // Density 1 preserves the native 20px body font and 200px Widget width.
        document.metadata.default_page_dimensions = Some((360, 1_527));
        document.metadata.flow_dimensions = Some((200, 200));
        document.metadata.flow_page_padding = Some((0, 0));
        document.metadata.note_text = Some(body);
        document.metadata.media_assets = [0_u32, 1_u32]
            .into_iter()
            .map(|archive_id| MediaAsset {
                name: format!("{archive_id}.png"),
                archive_id: Some(archive_id),
                mime_type: "image/png".to_owned(),
                data: include_bytes!("../tests/assets/crop-quadrants.png").to_vec(),
            })
            .collect();

        let layout = layout_document(&document);
        let fonts = super::fonts::FontBook::default();
        let mut cache = super::DocumentTextCache::default();
        cache
            .render_layout_page_svg(&document, &layout, 0, &RenderOptions::default(), &fonts)
            .expect("page renders");
        let plan = &cache.plans[0].1;
        assert_eq!(plan.text.text, "\u{fffc}\n\u{fffc}");
        assert_eq!(plan.text.object_spans.len(), 2);
        assert!(plan.object_issues.is_empty());
        assert_eq!(plan.layout.lines.len(), 2);
        let owners = plan
            .layout
            .lines
            .iter()
            .flat_map(|line| line.line.objects.iter())
            .map(|object| (object.object.span_index, object.object.source.clone()))
            .collect::<Vec<_>>();
        assert_eq!(owners, [(0, 0..1), (1, 2..3)]);
        for (span_index, anchor) in [(0, 0), (1, 2)] {
            assert_eq!(
                plan.text.text.chars().nth(anchor),
                Some('\u{fffc}'),
                "span {span_index} must own a replacement-character anchor"
            );
            let span = &plan.text.object_spans[span_index];
            assert_eq!(span.text_index_utf16, anchor as i32);
            let Some(RichTextObjectContent::Image(image)) = span.content.as_ref() else {
                panic!("span {span_index} should retain resolved image content");
            };
            assert_eq!(image.media_index, Some(span_index));
            assert_eq!(image.media_id, Some(span_index as u32));
        }

        // Reject duplicate anchors before layout can collapse distinct spans.
        let mut duplicate_anchor = document.metadata.note_text.clone().unwrap();
        duplicate_anchor.object_spans[1].text_index_utf16 = 0;
        assert!(super::body_query::query_image_sources(&duplicate_anchor, &document).is_none());

        // The recovered multi-image contract has LF-separated Block objects.
        let mut adjacent = document.metadata.note_text.clone().unwrap();
        adjacent.text = "\u{fffc}\u{fffc}".into();
        adjacent.object_spans[1].text_index_utf16 = 1;
        assert!(super::body_query::query_image_sources(&adjacent, &document).is_none());

        let mut missing_span = document.metadata.note_text.clone().unwrap();
        missing_span.object_spans.pop();
        assert!(super::body_query::query_image_sources(&missing_span, &document).is_none());

        let mut extra_span = document.metadata.note_text.clone().unwrap();
        extra_span
            .object_spans
            .push(extra_span.object_spans[0].clone());
        assert!(super::body_query::query_image_sources(&extra_span, &document).is_none());

        let mut out_of_range_anchor = document.metadata.note_text.clone().unwrap();
        out_of_range_anchor.object_spans[1].text_index_utf16 = 3;
        assert!(super::body_query::query_image_sources(&out_of_range_anchor, &document).is_none());

        let snapshot = cache
            .continuous_body_query_snapshot(
                &document,
                &layout.pages[0],
                &RenderOptions::default(),
                &fonts,
            )
            .expect("both resolved image anchors have one measured owner");
        let expected_lines = [
            [0, 0, 1_101_004_800, 1_104_675_340],
            [0, 1_104_675_340, 1_101_004_800, 1_113_063_948],
        ];
        for (index, expected) in expected_lines.into_iter().enumerate() {
            assert_eq!(
                snapshot.line_rect(index).unwrap().map(f32::to_bits),
                expected,
                "native two-image Block line {index}"
            );
        }
        let second_top = f32::from_bits(1_104_675_340);
        assert!(snapshot.query([0.0, 0.0, 200.0, 20.0]).visible);
        assert!(
            snapshot
                .query([0.0, second_top, 200.0, second_top + 20.0])
                .visible
        );
        assert!(!snapshot.query([0.0, second_top, 200.0, second_top]).visible);
        assert_eq!(snapshot.line_index(second_top), Some(0));
        assert_eq!(
            snapshot.line_index(f32::from_bits(second_top.to_bits() + 1)),
            Some(1)
        );

        let mut gap_document = document.clone();
        let gap_body = gap_document.metadata.note_text.as_mut().unwrap();
        gap_body.text = "\u{fffc}\n\n\u{fffc}".into();
        gap_body.object_spans[1].text_index_utf16 = 3;
        let gap_layout = layout_document(&gap_document);
        let mut gap_cache = super::DocumentTextCache::default();
        gap_cache
            .render_layout_page_svg(
                &gap_document,
                &gap_layout,
                0,
                &RenderOptions::default(),
                &fonts,
            )
            .expect("LF-gap page renders");
        let gap_plan = &gap_cache.plans[0].1;
        assert_eq!(gap_plan.layout.lines.len(), 3);
        let gap_owners = gap_plan
            .layout
            .lines
            .iter()
            .flat_map(|line| line.line.objects.iter())
            .map(|object| (object.object.span_index, object.object.source.clone()))
            .collect::<Vec<_>>();
        assert_eq!(gap_owners, [(0, 0..1), (1, 3..4)]);
        let gap_snapshot = gap_cache
            .continuous_body_query_snapshot(
                &gap_document,
                &gap_layout.pages[0],
                &RenderOptions::default(),
                &fonts,
            )
            .expect("LF-gap image anchors retain unique owners");
        let expected_gap_lines = [
            [0, 0, 1_101_004_800, 1_104_675_340],
            [0, 1_104_675_340, 0, 1_113_063_686],
            [0, 1_113_063_686, 1_101_004_800, 1_117_913_350],
        ];
        for (index, expected) in expected_gap_lines.into_iter().enumerate() {
            assert_eq!(
                gap_snapshot.line_rect(index).unwrap().map(f32::to_bits),
                expected,
                "native two-image LF-gap line {index}"
            );
        }
        let gap_top = f32::from_bits(1_104_675_340);
        let second_image_top = f32::from_bits(1_113_063_686);
        assert!(!gap_snapshot.query([0.0, 30.0, 200.0, 50.0]).visible);
        assert!(
            !gap_snapshot
                .query([0.0, gap_top, 200.0, second_image_top])
                .visible
        );
        assert!(gap_snapshot.query([0.0, 0.0, 200.0, 20.0]).visible);
        assert!(
            gap_snapshot
                .query([0.0, second_image_top, 200.0, f32::from_bits(1_116_995_715)])
                .visible
        );
        assert_eq!(gap_snapshot.line_index(gap_top), Some(0));
        assert_eq!(
            gap_snapshot.line_index(f32::from_bits(gap_top.to_bits() + 1)),
            Some(1)
        );
        assert_eq!(gap_snapshot.line_index(second_image_top), Some(1));
        assert_eq!(
            gap_snapshot.line_index(f32::from_bits(second_image_top.to_bits() + 1)),
            Some(2)
        );
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn single_page_continuous_uses_full_source_without_replacing_inspection_slice() {
        let mut body = theme_test_text();
        body.text = format!("{}\u{fffc}tail", "\n".repeat(112));
        body.text_sections = vec![RichTextSection {
            start_utf16: 0,
            length_utf16: 36,
        }];
        body.object_spans = vec![RichTextObjectSpan {
            object_type: ObjectType::Image,
            object_data: Vec::new(),
            content: Some(RichTextObjectContent::Image(Box::new(PlacedImage {
                bbox: BoundingBox {
                    x_min: 40.0,
                    y_min: 5_400.0,
                    x_max: 180.0,
                    y_max: 5_600.0,
                },
                rotation_degrees: None,
                media_id: Some(7),
                media_index: Some(0),
                crop_rect: None,
                original_bbox: None,
                border_media_id: None,
                original_media_id: None,
                fill_effect_transparency: 0.0,
            }))),
            text_index_utf16: 112,
            layout_option: ObjectSpanLayoutOption::Block,
            layout_constraint: ObjectSpanLayoutConstraint::Normal,
        }];
        let mut page = page_with_uncolored_stroke();
        page.width = 1_812;
        page.height = 15_372;
        let mut document = document(page);
        document.metadata.page_mode = Some(1);
        document.metadata.default_page_dimensions = Some((1_080, 1_527));
        document.metadata.flow_dimensions = Some((1_080, 9_162));
        document.metadata.note_text = Some(body);
        document.metadata.media_assets = vec![MediaAsset {
            name: "0.svg".to_owned(),
            archive_id: Some(0),
            mime_type: "image/svg+xml".to_owned(),
            data: br#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="10" height="10" fill="red"/></svg>"#.to_vec(),
        }];

        let layout = layout_document(&document);
        let layout_page = &layout.pages[0];
        let inspection = layout_page.body_text_slice().unwrap();
        assert_eq!(inspection.source_range, 0..36);
        let PageObjectContent::Element(PageElement::TextBox(inspection_text)) =
            layout_page.page.objects[inspection.object_index]
                .content
                .leaf()
        else {
            panic!("expected the generated inspection text box");
        };
        assert_eq!(inspection_text.text.encode_utf16().count(), 36);
        assert!(inspection_text.object_spans.is_empty());

        let fonts = super::fonts::FontBook::default();
        let options = RenderOptions::default();
        let mut cache = super::DocumentTextCache::default();
        let inspection_svg = cache
            .render_layout_page_svg(&document, &layout, 0, &options, &fonts)
            .unwrap();
        assert!(inspection_svg.svg.contains("data:image/svg+xml;base64,"));
        assert_eq!(cache.plans[0].1.text.text.encode_utf16().count(), 117);
        assert_eq!(cache.plans[0].1.text.object_spans[0].text_index_utf16, 112);
        let first_plan = super::Rc::clone(&cache.plans[0].1);

        let replay_svg = cache
            .render_layout_page_replay_svg(&document, &layout, 0, &options, &fonts)
            .unwrap();
        assert!(replay_svg.svg.contains("data:image/svg+xml;base64,"));
        assert!(super::Rc::ptr_eq(&first_plan, &cache.plans[0].1));

        let requests = [super::PageSceneRequest::full_page(layout_page)];
        let scene = cache.render_layout_page_scene_requests(
            &document,
            &requests,
            &options,
            &fonts,
            &mut |_| {},
        );
        assert!(scene[0].page.svg.contains("data:image/svg+xml;base64,"));
        assert_eq!(cache.plans[0].1.text.text.encode_utf16().count(), 117);
        assert_eq!(cache.plans[0].1.text.object_spans[0].text_index_utf16, 112);
        assert!(super::Rc::ptr_eq(&first_plan, &cache.plans[0].1));
        document
            .metadata
            .note_text
            .as_mut()
            .unwrap()
            .text
            .push_str(" changed-source");
        let edited_layout = layout_document(&document);
        let request = [super::PageSceneRequest::full_page(&edited_layout.pages[0])];
        let changed = cache.render_layout_page_scene_requests(
            &document,
            &request,
            &options,
            &fonts,
            &mut |_| {},
        );
        assert!(changed[0].page.svg.contains("data:image/svg+xml;base64,"));
        assert_eq!(cache.plans.len(), 1);
        assert!(!super::Rc::ptr_eq(&first_plan, &cache.plans[0].1));
        assert!(cache.plans[0].1.text.text.ends_with(" changed-source"));

        let mut edited_layout = layout_document(&document);
        let object_index = edited_layout.pages[0]
            .body_text_slice()
            .unwrap()
            .object_index;
        let PageObjectContent::Element(PageElement::TextBox(inspection_text)) =
            &mut edited_layout.pages[0].page.objects[object_index].content
        else {
            panic!("expected the generated inspection text box");
        };
        inspection_text.text.push_str(" edited-layout");
        let request = [super::PageSceneRequest::full_page(&edited_layout.pages[0])];
        let edited = cache.render_layout_page_scene_requests(
            &document,
            &request,
            &options,
            &fonts,
            &mut |_| {},
        );
        assert!(!edited[0].page.svg.contains("data:image/svg+xml;base64,"));

        let mut stale_document = Document {
            pages: document.pages.clone(),
            metadata: document.metadata.clone(),
        };
        stale_document
            .metadata
            .note_text
            .as_mut()
            .unwrap()
            .text
            .replace_range(..1, "x");
        let request = [super::PageSceneRequest::full_page(&layout.pages[0])];
        let stale = cache.render_layout_page_scene_requests(
            &stale_document,
            &request,
            &options,
            &fonts,
            &mut |_| {},
        );
        assert!(!stale[0].page.svg.contains("data:image/svg+xml;base64,"));
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn overshooting_slice_keeps_canvas_tail_without_retaining_out_of_page_glyphs() {
        let mut document = preparation_document();
        // Disable the certified native page-line map, so the slice viewport is
        // the authority that keeps another physical page's text out.
        document.metadata.page_mode = Some(1);
        let layout = layout_document(&document);
        assert!(layout.pages.len() >= 2);
        let options = RenderOptions::default();
        let fonts = super::fonts::FontBook::default();
        let mut cache = super::DocumentTextCache::default();
        let requests = [
            super::PageSceneRequest::sliced(&layout.pages[0], 80, 40, true).unwrap(),
            super::PageSceneRequest::full_page(&layout.pages[0]),
            super::PageSceneRequest::full_page(&layout.pages[1]),
        ];
        let scenes = cache.render_layout_page_scene_requests(
            &document,
            &requests,
            &options,
            &fonts,
            &mut |_| {},
        );
        let glyph_keys = |scene: &super::RenderedScene| {
            scene
                .text
                .iter()
                .flat_map(|(_, block)| block.runs.iter())
                .flat_map(|run| run.glyphs.iter())
                .map(|glyph| (glyph.glyph_id, glyph.origin, glyph.source.clone()))
                .collect::<Vec<_>>()
        };
        let tail_glyphs = glyph_keys(&scenes[0]);
        let first_page_glyphs = glyph_keys(&scenes[1]);
        let next_page_glyphs = glyph_keys(&scenes[2]);
        assert!(
            !tail_glyphs.is_empty(),
            "tail includes its visible body line"
        );
        assert!(
            !next_page_glyphs.is_empty(),
            "second physical page retains its body glyphs"
        );
        assert!(
            !first_page_glyphs.is_empty(),
            "first physical page retains its body glyphs"
        );
        assert!(
            tail_glyphs
                .iter()
                .all(|glyph| first_page_glyphs.contains(glyph)),
            "overshooting slice does not retain glyphs beyond the current page's full-page registry"
        );

        // The clip is only for body selection: output viewBox and canvas still
        // span the native 80..120 tail even though the note page ends at 100.
        assert_eq!(scenes[0].page.height, 40);
        let xml = roxmltree::Document::parse(&scenes[0].page.svg).unwrap();
        assert_eq!(
            view_box_numbers(xml.root_element()),
            vec![0.0, 80.0, f64::from(layout.pages[0].page.width), 40.0,]
        );
        let canvas = xml
            .root_element()
            .children()
            .find(|node| node.has_tag_name("rect"))
            .expect("ordinary continuation retains its canvas");
        assert_eq!(number_attribute(canvas, "y"), 80.0);
        assert_eq!(number_attribute(canvas, "height"), 40.0);
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn nonzero_slice_preserves_native_oblique_ink_marker_and_payload() {
        let mut page = page_with_uncolored_stroke();
        let mut stroke = oblique_saved_stroke(false);
        for point in &mut stroke.points {
            point.y += 40.0;
        }
        page.objects = vec![stroke.into()];
        let document = document(page);
        let layout = layout_document(&document);
        let options = RenderOptions::default();
        let fonts = super::fonts::FontBook::default();
        let mut cache = super::DocumentTextCache::default();
        let requests = [
            super::PageSceneRequest::full_page(&layout.pages[0]),
            super::PageSceneRequest::sliced(&layout.pages[0], 30, 40, true).unwrap(),
        ];
        let scenes = cache.render_layout_page_scene_requests(
            &document,
            &requests,
            &options,
            &fonts,
            &mut |_| {},
        );
        assert_eq!(
            scenes[0].ink.len(),
            1,
            "full scene registers native oblique ink"
        );
        assert_eq!(
            scenes[1].ink.len(),
            1,
            "intersecting slice retains native ink"
        );
        let (full_id, full_ink) = scenes[0].ink.iter().next().unwrap();
        let (slice_id, slice_ink) = scenes[1].ink.iter().next().unwrap();
        assert_eq!(full_id.marker_id(0), slice_id.marker_id(0));
        assert_eq!(full_ink.bounds, slice_ink.bounds);
        assert_eq!(full_ink.opacity.to_bits(), slice_ink.opacity.to_bits());
        match (&full_ink.payload, &slice_ink.payload) {
            (
                crate::render::native_ink::NativeInkPayload::Mesh {
                    mesh: full_mesh,
                    color: full_color,
                },
                crate::render::native_ink::NativeInkPayload::Mesh {
                    mesh: slice_mesh,
                    color: slice_color,
                },
            ) => {
                assert_eq!(full_mesh, slice_mesh);
                assert_eq!(full_color, slice_color);
            }
            _ => panic!("slice changed the retained native ink payload kind"),
        }
        assert!(slice_ink.bounds.y_min < 70.0 && slice_ink.bounds.y_max > 30.0);
        assert!(scenes[1].page.svg.contains("id=\"sdocx-native-ink-0\""));

        let pdf_options = crate::pdf::PdfOptions::from_font_book(&fonts);
        let full_pdf =
            crate::pdf::render_scenes_pdf_with_progress(&scenes[..1], &pdf_options, &mut |_| {})
                .unwrap();
        let slice_pdf =
            crate::pdf::render_scenes_pdf_with_progress(&scenes[1..], &pdf_options, &mut |_| {})
                .unwrap();
        let assert_field_form = |bytes: &[u8], expected_height: f64| {
            let pdf = lopdf::Document::load_mem(bytes).unwrap();
            assert_eq!(pdf.get_pages().len(), 1);
            let page_id = *pdf.get_pages().values().next().unwrap();
            let page = pdf.get_dictionary(page_id).unwrap();
            let media_box = page.get(b"MediaBox").unwrap().as_array().unwrap();
            let coordinates = media_box
                .iter()
                .map(|value| match value {
                    lopdf::Object::Integer(value) => *value as f64,
                    lopdf::Object::Real(value) => f64::from(*value),
                    _ => panic!("unexpected MediaBox coordinate: {value:?}"),
                })
                .collect::<Vec<_>>();
            assert!((coordinates[2] - coordinates[0] - 75.0).abs() < 0.01);
            assert!((coordinates[3] - coordinates[1] - expected_height).abs() < 0.01);

            let field_forms = pdf
                .objects
                .values()
                .filter_map(|object| object.as_stream().ok())
                .filter(|stream| {
                    stream
                        .dict
                        .get(b"Subtype")
                        .and_then(lopdf::Object::as_name)
                        .is_ok_and(|name| name == b"Form")
                        && stream
                            .dict
                            .get(b"BBox")
                            .and_then(lopdf::Object::as_array)
                            .is_ok_and(|bbox| {
                                bbox.len() == 4
                                    && bbox.iter().zip([0.0, 0.0, 1.0, 1.0]).all(
                                        |(value, expected)| match value {
                                            lopdf::Object::Integer(value) => {
                                                *value as f64 == expected
                                            }
                                            lopdf::Object::Real(value) => {
                                                f64::from(*value) == expected
                                            }
                                            _ => false,
                                        },
                                    )
                            })
                        && stream.decompressed_content().is_ok_and(|content| {
                            let marker_operator = b" rg 0 0 1 1 re f";
                            content
                                .windows(marker_operator.len())
                                .any(|window| window == marker_operator)
                        })
                })
                .count();
            assert_eq!(
                field_forms, 1,
                "serialized PDF contains one native ink field Form"
            );
        };
        assert_field_form(&full_pdf, 75.0);
        assert_field_form(&slice_pdf, 30.0);

        if let Some(directory) = std::env::var_os("SDOCX_SLICE_INK_ARTIFACTS") {
            let directory = std::path::PathBuf::from(directory);
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(directory.join("full-page.pdf"), &full_pdf).unwrap();
            std::fs::write(directory.join("slice-y30-h40.pdf"), &slice_pdf).unwrap();
        }
    }

    #[cfg(feature = "pdf")]
    fn view_box_numbers(root: roxmltree::Node<'_, '_>) -> Vec<f64> {
        root.attribute("viewBox")
            .unwrap()
            .split_whitespace()
            .map(|component| component.parse().unwrap())
            .collect()
    }

    #[cfg(feature = "pdf")]
    fn number_attribute(node: roxmltree::Node<'_, '_>, name: &str) -> f64 {
        node.attribute(name).unwrap().parse().unwrap()
    }

    #[cfg(feature = "pdf")]
    fn tagged_text_origins(bytes: &[u8]) -> Vec<(Vec<u8>, [f64; 2])> {
        let pdf = lopdf::Document::load_mem(bytes).unwrap();
        let page_id = *pdf.get_pages().values().next().unwrap();
        let content =
            lopdf::content::Content::decode(&pdf.get_page_content(page_id).unwrap()).unwrap();
        let number = |object: &lopdf::Object| match object {
            lopdf::Object::Integer(value) => *value as f64,
            lopdf::Object::Real(value) => f64::from(*value),
            _ => panic!("expected PDF numeric operand, got {object:?}"),
        };
        let mut ctm = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
        let mut graphics_stack = Vec::new();
        let mut tags: Vec<Option<Vec<u8>>> = Vec::new();
        let mut origins = Vec::new();
        for operation in content.operations {
            match operation.operator.as_str() {
                "q" => graphics_stack.push(ctm),
                "Q" => ctm = graphics_stack.pop().unwrap(),
                "cm" => {
                    let next: [f64; 6] = operation
                        .operands
                        .iter()
                        .map(number)
                        .collect::<Vec<_>>()
                        .try_into()
                        .unwrap();
                    let [a, b, c, d, e, f] = ctm;
                    let [na, nb, nc, nd, ne, nf] = next;
                    ctm = [
                        a * na + c * nb,
                        b * na + d * nb,
                        a * nc + c * nd,
                        b * nc + d * nd,
                        a * ne + c * nf + e,
                        b * ne + d * nf + f,
                    ];
                }
                "BDC" => {
                    let actual = operation
                        .operands
                        .get(1)
                        .and_then(|properties| properties.as_dict().ok())
                        .and_then(|properties| properties.get(b"ActualText").ok())
                        .and_then(|text| text.as_str().ok())
                        .map(<[u8]>::to_vec);
                    tags.push(actual.or_else(|| tags.last().cloned().flatten()));
                }
                "BMC" => tags.push(tags.last().cloned().flatten()),
                "EMC" => {
                    tags.pop().unwrap();
                }
                "Tm" => {
                    let Some(actual_text) = tags.last().and_then(Option::as_ref) else {
                        continue;
                    };
                    let [_, _, _, _, x, y]: [f64; 6] = operation
                        .operands
                        .iter()
                        .map(number)
                        .collect::<Vec<_>>()
                        .try_into()
                        .unwrap();
                    origins.push((
                        actual_text.clone(),
                        [
                            ctm[0] * x + ctm[2] * y + ctm[4],
                            ctm[1] * x + ctm[3] * y + ctm[5],
                        ],
                    ));
                }
                _ => {}
            }
        }
        origins
    }

    #[test]
    fn body_preparation_invalidates_native_name_configuration_with_shared_database() {
        let mut document = preparation_document();
        let body = document.metadata.note_text.as_mut().unwrap();
        body.text = "AV To office".into();
        body.font_size = Some(17.0);
        let layout = layout_document(&document);
        let native_fonts = super::fonts::FontBook::default();
        let compatibility_fonts = super::fonts::FontBook::new(native_fonts.database());
        let equivalent_fonts = super::fonts::FontBook::new(native_fonts.database())
            .with_native_name_config(native_fonts.native_name_config().unwrap().clone());
        let options = RenderOptions::default();
        let cold = |fonts: &super::fonts::FontBook| {
            let mut cache = super::DocumentTextCache::default();
            let page = cache
                .render_layout_page_svg(&document, &layout, 0, &options, fonts)
                .unwrap();
            (page, super::Rc::clone(&cache.plans[0].1))
        };
        let has_native_facts = |plan: &super::PreparedBodyPlan| {
            plan.layout.lines[0].line.placements[0]
                .cluster
                .run
                .native_entries
                .as_ref()
                .is_some_and(|native| {
                    native
                        .entry_facts_at_utf16(native.geometry().source_range_utf16().start)
                        .is_some()
                })
        };
        let (native_page, native_cold_plan) = cold(&native_fonts);
        let (compatibility_page, _) = cold(&compatibility_fonts);
        assert!(has_native_facts(&native_cold_plan));
        let positions = |svg: &str| {
            roxmltree::Document::parse(svg)
                .unwrap()
                .descendants()
                .filter(|node| node.has_tag_name("tspan"))
                .map(|node| node.attribute("x").map(str::to_owned))
                .collect::<Vec<_>>()
        };
        assert_ne!(
            positions(&native_page.svg),
            positions(&compatibility_page.svg)
        );

        let mut cache = super::DocumentTextCache::default();
        cache.render_layout_page_svg(&document, &layout, 0, &options, &native_fonts);
        let native_plan = super::Rc::clone(&cache.plans[0].1);
        cache.render_layout_page_svg(&document, &layout, 0, &options, &native_fonts.clone());
        assert!(super::Rc::ptr_eq(&native_plan, &cache.plans[0].1));
        let equivalent_warm = cache
            .render_layout_page_svg(&document, &layout, 0, &options, &equivalent_fonts)
            .unwrap();
        let (equivalent_cold, equivalent_cold_plan) = cold(&equivalent_fonts);
        assert_eq!(
            equivalent_warm.text_diagnostics,
            equivalent_cold.text_diagnostics
        );
        assert_eq!(equivalent_warm.svg, equivalent_cold.svg);
        assert!(!super::Rc::ptr_eq(&native_plan, &cache.plans[0].1));
        assert!(!has_native_facts(&equivalent_cold_plan));
        assert_eq!(
            has_native_facts(&cache.plans[0].1),
            has_native_facts(&equivalent_cold_plan)
        );
        let equivalent_plan = super::Rc::clone(&cache.plans[0].1);
        let rebuilt_fonts = super::fonts::FontBook::new(native_fonts.database())
            .with_native_name_config(native_fonts.native_name_config().unwrap().clone());
        cache.render_layout_page_svg(&document, &layout, 0, &options, &rebuilt_fonts);
        assert!(super::Rc::ptr_eq(&equivalent_plan, &cache.plans[0].1));

        let compatibility_warm = cache
            .render_layout_page_svg(&document, &layout, 0, &options, &compatibility_fonts)
            .unwrap();
        assert!(!super::Rc::ptr_eq(&equivalent_plan, &cache.plans[0].1));
        assert_eq!(compatibility_warm.svg, compatibility_page.svg);
        assert_eq!(
            compatibility_warm.text_diagnostics,
            compatibility_page.text_diagnostics
        );
        let compatibility_plan = super::Rc::clone(&cache.plans[0].1);
        let native_warm = cache
            .render_layout_page_svg(&document, &layout, 0, &options, &native_fonts)
            .unwrap();
        assert!(!super::Rc::ptr_eq(&compatibility_plan, &cache.plans[0].1));
        assert_eq!(native_warm.svg, native_page.svg);
        assert_eq!(native_warm.text_diagnostics, native_page.text_diagnostics);
        assert_eq!(
            has_native_facts(&cache.plans[0].1),
            has_native_facts(&native_cold_plan)
        );
    }

    #[test]
    fn body_preparation_keeps_distinct_capture_windows_and_themes_bounded() {
        let mut document = preparation_document();
        let fonts = super::fonts::FontBook::default();
        let options = RenderOptions::default();
        let mut cache = super::DocumentTextCache::default();
        for (index, page) in document.pages.iter_mut().enumerate() {
            let gray = (index * 70) as u8;
            page.background_color = Some(Color {
                r: gray,
                g: gray,
                b: gray,
            });
        }
        let layout = layout_document(&document);
        for index in 0..layout.pages.len() {
            cache.render_layout_page_svg(&document, &layout, index, &options, &fonts);
            assert_eq!(
                cache.plans.len(),
                (index + 1).min(super::BODY_PLAN_CACHE_CAPACITY)
            );
        }
        let source = document.metadata.note_text.as_mut().unwrap();
        source.text_sections = (0..4)
            .map(|index| crate::RichTextSection {
                start_utf16: index * 40,
                length_utf16: 40,
            })
            .collect();
        let layout = layout_document(&document);
        for index in 0..layout.pages.len() {
            cache.render_layout_page_svg(&document, &layout, index, &options, &fonts);
            assert_eq!(
                cache.plans.len(),
                (index + 1).min(super::BODY_PLAN_CACHE_CAPACITY)
            );
            assert_eq!(cache.plans.last().unwrap().0.last_page_index, index);
        }
    }

    #[test]
    fn body_preparation_reuses_unchanged_nan_and_invalidates_changed_nan() {
        let mut document = preparation_document();
        document.metadata.note_text.as_mut().unwrap().font_size = Some(f32::from_bits(0x7fc00001));
        let fonts = super::fonts::FontBook::default();
        let options = RenderOptions::default();
        let layout = layout_document(&document);
        let mut cache = super::DocumentTextCache::default();
        cache.render_layout_page_svg(&document, &layout, 0, &options, &fonts);
        let plan = super::Rc::clone(&cache.plans[0].1);
        cache.render_layout_page_replay_svg(&document, &layout, 1, &options, &fonts);
        assert!(super::Rc::ptr_eq(&plan, &cache.plans[0].1));
        document.metadata.note_text.as_mut().unwrap().font_size = Some(f32::from_bits(0x7fc00002));
        cache.render_layout_page_svg(&document, &layout, 0, &options, &fonts);
        assert!(cache.plans.is_empty());
        let layout = layout_document(&document);
        cache.render_layout_page_svg(&document, &layout, 0, &options, &fonts);
        assert!(!super::Rc::ptr_eq(&plan, &cache.plans[0].1));
    }

    #[test]
    fn collapsed_native_clone_keeps_source_without_stale_code_artwork() {
        let fixture = |padding: u32| {
            let mut body = theme_test_text();
            body.bbox = BoundingBox::default();
            body.text = "A\u{fffc}".into();
            body.object_spans.push(crate::RichTextObjectSpan {
                object_type: crate::ObjectType::CodeBlock,
                object_data: Vec::new(),
                content: Some(crate::RichTextObjectContent::CodeBlock(Box::new(
                    crate::RichTextCodeBlock {
                        bbox: BoundingBox {
                            x_min: 0.0,
                            y_min: 0.0,
                            x_max: 1.0,
                            y_max: 20.0,
                        },
                        rotation_degrees: None,
                        title: None,
                        body: None,
                    },
                ))),
                text_index_utf16: 1,
                layout_option: crate::ObjectSpanLayoutOption::Inline,
                layout_constraint: crate::ObjectSpanLayoutConstraint::Normal,
            });
            Document {
                pages: vec![Page {
                    uuid: "collapsed-clone".into(),
                    width: 2 * padding + 100,
                    height: 500,
                    content_bbox: BoundingBox::default(),
                    background_color: None,
                    template: None,
                    background: Default::default(),
                    objects: Vec::new(),
                }],
                metadata: DocumentMetadata {
                    note_text: Some(body),
                    page_mode: Some(0),
                    default_page_dimensions: Some((360, 500)),
                    orientation: Some(0),
                    flow_page_padding: Some((padding, 0)),
                    ..Default::default()
                },
            }
        };
        for padding in [0, 16_777_216] {
            let document = fixture(padding);
            let layout = layout_document(&document);
            for rendered in [
                render_layout_page_svg(&document, &layout, 0, &Default::default()).unwrap(),
                super::render_layout_page_replay_svg(&document, &layout, 0, &Default::default())
                    .unwrap(),
            ] {
                let xml = roxmltree::Document::parse(&rendered.svg).unwrap();
                let source = xml
                    .descendants()
                    .filter(|node| node.has_tag_name("tspan"))
                    .filter_map(|node| node.text())
                    .collect::<String>();
                let artwork = xml
                    .descendants()
                    .any(|node| node.attribute("data-sdocx-object") == Some("code-block"));
                if padding == 0 {
                    assert_eq!(source, "A");
                    assert!(artwork);
                    assert!(rendered.object_diagnostics.is_empty());
                } else {
                    assert_eq!(source, "A\u{fffc}");
                    assert!(!artwork);
                    assert!(
                        rendered
                            .object_diagnostics
                            .iter()
                            .any(|issue| issue.anchor_utf16 == 1
                                && issue.kind == crate::ObjectDiagnosticKind::InvalidBounds)
                    );
                }
            }
        }
    }

    #[test]
    fn explicit_ink_and_text_adapt_together_and_honor_compatibility() {
        for (compatible, background, foreground) in
            [(true, "#252525", "#ffffff"), (false, "#fcfcfc", "#000000")]
        {
            let mut page = page_with_uncolored_stroke();
            page.background_color = Some(Color {
                r: 252,
                g: 252,
                b: 252,
            });
            page.strokes_mut().next().unwrap().color = Some(Color { r: 0, g: 0, b: 0 });
            page.objects
                .push(PageElement::TextBox(theme_test_text()).into());
            let mut doc = document(page);
            doc.metadata.dark_mode_compatibility = Some(compatible);
            let options = RenderOptions {
                color_mode: RenderColorMode::Dark,
                ..Default::default()
            };
            let svg = render_document_svg(&doc, &options).remove(0).svg;
            let xml = roxmltree::Document::parse(&svg).unwrap();
            assert!(
                xml.descendants()
                    .any(|n| n.has_tag_name("rect") && n.attribute("fill") == Some(background))
            );
            assert!(
                xml.descendants()
                    .any(|n| n.attribute("stroke") == Some(foreground))
            );
            assert!(
                xml.descendants()
                    .any(|n| n.has_tag_name("tspan") && n.attribute("fill") == Some(foreground))
            );
            let theme =
                super::RenderTheme::resolve(&doc.pages[0], &doc.metadata, options.color_mode);
            assert_eq!(
                super::shape_paint(&crate::ShapePaint::Solid(0x80000000), theme),
                (foreground.into(), 128. / 255.)
            );
        }
    }

    #[test]
    fn dark_mode_keeps_black_text_on_a_white_highlight() {
        let mut page = page_with_uncolored_stroke();
        let mut text = theme_test_text();
        text.highlight_color = Some(Color {
            r: 255,
            g: 255,
            b: 255,
        });
        page.objects.push(PageElement::TextBox(text).into());
        let svg = render_document_svg(
            &document(page),
            &RenderOptions {
                color_mode: RenderColorMode::Dark,
                ..Default::default()
            },
        )
        .remove(0)
        .svg;
        let xml = roxmltree::Document::parse(&svg).unwrap();
        assert!(
            xml.descendants()
                .any(|n| n.has_tag_name("tspan") && n.attribute("fill") == Some("#000000"))
        );
        assert!(
            xml.descendants()
                .any(|n| n.has_tag_name("rect") && n.attribute("fill") == Some("#ffffff"))
        );
    }

    #[test]
    fn light_mode_adapts_white_ink_when_dark_paper_changes_to_light() {
        let mut page = page_with_uncolored_stroke();
        page.background_color = Some(Color {
            r: 37,
            g: 37,
            b: 37,
        });
        page.strokes_mut().next().unwrap().color = Some(Color {
            r: 255,
            g: 255,
            b: 255,
        });
        let mut text = theme_test_text();
        text.color = page.strokes().next().unwrap().color;
        page.objects.push(PageElement::TextBox(text).into());
        let doc = document(page);
        for (mode, ink) in [
            (RenderColorMode::Auto, "#ffffff"),
            (RenderColorMode::Light, "#000000"),
        ] {
            let svg = render_document_svg(
                &doc,
                &RenderOptions {
                    color_mode: mode,
                    ..Default::default()
                },
            )
            .remove(0)
            .svg;
            let xml = roxmltree::Document::parse(&svg).unwrap();
            assert!(
                xml.descendants()
                    .any(|n| n.attribute("stroke") == Some(ink))
            );
            assert!(
                xml.descendants()
                    .any(|n| n.has_tag_name("tspan") && n.attribute("fill") == Some(ink))
            );
        }
    }

    #[test]
    fn light_page_text_adapts_to_a_dark_local_highlight() {
        let mut page = page_with_uncolored_stroke();
        let mut text = theme_test_text();
        text.highlight_color = Some(Color { r: 0, g: 0, b: 0 });
        page.objects.push(PageElement::TextBox(text).into());
        let svg = render_document_svg(&document(page), &RenderOptions::default())
            .remove(0)
            .svg;
        let xml = roxmltree::Document::parse(&svg).unwrap();
        assert!(
            xml.descendants()
                .any(|n| n.has_tag_name("tspan") && n.attribute("fill") == Some("#ffffff"))
        );
    }

    #[test]
    fn cell_text_uses_its_own_surface_and_inherited_paper_stays_custom() {
        let mut page = page_with_uncolored_stroke();
        page.background_color = Some(Color {
            r: 20,
            g: 30,
            b: 40,
        });
        let theme =
            super::RenderTheme::resolve(&page, &DocumentMetadata::default(), RenderColorMode::Auto);
        let mut cell = crate::RichTextTableCell {
            border: None,
            metadata: Default::default(),
            column_index: 0,
            row_span: 1,
            column_span: 1,
            background_color: 0xff000000,
            has_own_background_color: true,
            bbox: BoundingBox::default(),
            editable: false,
            content: theme_test_text(),
        };
        let table = super::table::tests::grid(&[10.0], &[20.0]);
        let fill = super::table::CellFill::resolve(&table.style, 0, &cell, theme);
        assert_eq!(
            super::StyledText::new(
                &cell.content,
                super::TextContext::Flow,
                super::TextSettings::from_document(&DocumentMetadata::default())
            )
            .style_at(0, fill.theme(theme), None)
            .color,
            "#000000"
        );
        cell.has_own_background_color = false;
        assert_eq!(
            super::table::CellFill::resolve(&table.style, 0, &cell, theme)
                .theme(theme)
                .background(),
            page.background_color.unwrap()
        );
        assert_eq!(
            super::StyledText::new(
                &cell.content,
                super::TextContext::Flow,
                super::TextSettings::from_document(&DocumentMetadata::default())
            )
            .style_at(0, theme, None)
            .color,
            "#ffffff"
        );
    }

    #[test]
    fn clamps_pressure_while_rendering_strokes() {
        let mut page = page_with_uncolored_stroke();
        page.strokes_mut().next().unwrap().pressures = vec![f64::MAX, f64::MAX];
        let pages = render_document_svg(&document(page), &RenderOptions::default());

        assert!(pages[0].svg.contains(r#"stroke-width="0.80""#));
        assert!(!pages[0].svg.contains("inf"));
    }

    #[test]
    fn dark_mode_makes_compatibility_text_visible() {
        let mut page = page_with_uncolored_stroke();
        page.clear_strokes();
        page.objects.push(
            PageElement::TextBox(RichTextBox {
                text_area_type: None,
                bbox: BoundingBox::default(),
                rotation_degrees: None,
                text: "visible body text".into(),
                color: Some(Color {
                    r: 0x25,
                    g: 0x25,
                    b: 0x25,
                }),
                highlight_color: None,
                underline: false,
                font_size: None,
                runs: Vec::new(),
                spans: Vec::new(),
                paragraphs: Vec::new(),
                object_spans: Vec::new(),
                text_sections: Vec::new(),
                margins: None,
                gravity: None,
            })
            .into(),
        );
        let pages = render_document_svg(
            &document(page),
            &RenderOptions {
                color_mode: RenderColorMode::Dark,
                ..Default::default()
            },
        );

        let xml = roxmltree::Document::parse(&pages[0].svg).unwrap();
        assert!(xml.descendants().any(|node| node.has_tag_name("text")
            && node.attribute("x") == Some("48.00")
            && node.attribute("y") == Some("51.00")));
        assert!(pages[0].svg.contains(r##"<tspan fill="#dadada""##));
        assert!(!pages[0].svg.contains(r##"<tspan fill="#252525""##));
    }

    #[test]
    fn renders_hyperlink_color_and_target_from_sdk_span() {
        let target = "https://example.com/markdown-test";
        let mut payload = Vec::new();
        payload.extend_from_slice(&3_u32.to_le_bytes());
        payload.extend_from_slice(&0_u32.to_le_bytes());
        payload.extend_from_slice(&(target.encode_utf16().count() as u32).to_le_bytes());
        payload.extend(target.encode_utf16().flat_map(u16::to_le_bytes));
        let mut page = page_with_uncolored_stroke();
        page.width = 1080;
        page.clear_strokes();
        page.objects.push(
            PageElement::TextBox(RichTextBox {
                text_area_type: None,
                bbox: BoundingBox::default(),
                rotation_degrees: None,
                text: "Example link".into(),
                color: None,
                highlight_color: None,
                underline: false,
                font_size: Some(15.0),
                runs: Vec::new(),
                spans: vec![RichTextSpan {
                    kind: RichTextSpanType::Hyperlink,
                    start_utf16: 0,
                    end_utf16: 12,
                    interval_type: crate::SpanIntervalType::from(0),
                    payload,
                }],
                paragraphs: Vec::new(),
                object_spans: Vec::new(),
                text_sections: Vec::new(),
                margins: None,
                gravity: None,
            })
            .into(),
        );
        let pages = render_document_svg(&document(page), &RenderOptions::default());

        assert!(
            pages[0]
                .svg
                .contains(r##"<a href="https://example.com/markdown-test">"##)
        );
        assert!(pages[0].svg.contains(r##"fill="#0054ff""##));
        let xml = roxmltree::Document::parse(&pages[0].svg).unwrap();
        assert!(
            xml.descendants()
                .any(|node| node.has_tag_name("rect") && node.attribute("fill") == Some("#0054ff"))
        );
    }

    #[test]
    fn allows_only_safe_svg_hyperlink_targets() {
        for target in [
            "https://example.com/note",
            "HTTP://example.com/note",
            "mailto:notes@example.com",
            "tel:+15551234567",
            "/notes/one",
            "./notes/one",
            "../notes/one",
            "notes/one?mode=preview#section",
            "#section",
            "?page=2",
        ] {
            assert_eq!(
                sanitize_hyperlink_target(target.to_string()).as_deref(),
                Some(target),
                "expected {target:?} to remain linkable"
            );
        }

        for target in [
            "javascript:alert(1)",
            "JaVaScRiPt:alert(1)",
            "data:text/html,unsafe",
            "file:///tmp/note",
            "blob:https://example.com/id",
            "//example.com/note",
            r"\\example.com\note",
            "https://example.com/line\nfeed",
        ] {
            assert_eq!(
                sanitize_hyperlink_target(target.to_string()),
                None,
                "expected {target:?} to be stripped"
            );
        }

        assert_eq!(
            sanitize_hyperlink_target("  https://example.com/note  ".to_string()).as_deref(),
            Some("https://example.com/note")
        );
    }

    fn marker(top_layer: bool, x: f64) -> Stroke {
        Stroke {
            rendering: Some(StrokeRendering {
                metadata: None,
                pen_name: Some("com.samsung.android.sdk.pen.pen.preload.Marker2".into()),
                advanced_settings: Some("2;".into()),
                tool_type_raw: 2,
                properties: StrokeProperties {
                    compressed: false,
                    replay_only: false,
                    stylus_channels: false,
                    eraser: false,
                    fixed_width: false,
                    millisecond_timestamps: false,
                    top_layer_pen: top_layer,
                    alpha_lock: false,
                    binary_added: true,
                    generated: false,
                    fixed_opacity: false,
                    rainbow_effect: false,
                    straighten: false,
                    reveal_mode: false,
                },
                style: StrokeStyle {
                    color_argb: Some(0x80ff_ee00),
                    pen_size: Some(8.),
                    ..StrokeStyle::default()
                },
            }),
            bbox: BoundingBox::default(),
            // Saved Drawing requires two source events even for a stationary tap.
            points: vec![Point { x, y: 20. }; 2],
            pressures: vec![0.4; 2],
            timestamps: vec![],
            tilts: vec![],
            orientations: vec![],
            color: Some(Color {
                r: 255,
                g: 238,
                b: 0,
            }),
            pen_width: 8.,
        }
    }

    #[test]
    fn marker_zero_saved_size_does_not_paint_a_generic_fallback() {
        let mut page = page_with_uncolored_stroke();
        page.width = 64;
        page.height = 64;
        page.objects.clear();
        page.clear_elements();
        let pixels = |page| {
            let rendered =
                render_document_svg(&document(page), &RenderOptions::default()).remove(0);
            let tree = resvg::usvg::Tree::from_str(&rendered.svg, &Default::default()).unwrap();
            let mut pixmap = resvg::tiny_skia::Pixmap::new(64, 64).unwrap();
            resvg::render(
                &tree,
                resvg::tiny_skia::Transform::identity(),
                &mut pixmap.as_mut(),
            );
            pixmap.data().to_vec()
        };
        let blank = pixels(page.clone());
        for (saved_size, presentation_width) in [(Some(0.), 0.), (None, 0.8)] {
            let mut ink = marker(false, 10.);
            ink.points = [(10., 20.), (18., 25.), (26., 20.), (34., 25.)]
                .map(|(x, y)| Point { x, y })
                .to_vec();
            ink.pressures = vec![1.; 4];
            ink.timestamps = vec![0, 8, 16, 24];
            ink.rendering.as_mut().unwrap().tool_type_raw = 1;
            ink.rendering.as_mut().unwrap().style.pen_size = saved_size;
            ink.pen_width = presentation_width;
            let mut candidate = page.clone();
            candidate.objects.push(ink.into());
            assert_eq!(pixels(candidate), blank, "saved size {saved_size:?}");
        }
        page.objects.push(marker(false, 20.).into());
        assert_ne!(pixels(page), blank, "nonzero size must still paint");
    }

    #[test]
    fn top_layer_marker_is_one_darken_batch_after_text() {
        let mut page = page_with_uncolored_stroke();
        page.objects = vec![marker(false, 10.).into(), marker(true, 40.).into()];
        page.objects.push(
            PageElement::TextBox(RichTextBox {
                text_area_type: None,
                bbox: BoundingBox {
                    x_min: 4.,
                    y_min: 4.,
                    x_max: 80.,
                    y_max: 40.,
                },
                rotation_degrees: None,
                text: "under the highlighter".into(),
                color: Some(Color { r: 0, g: 0, b: 0 }),
                highlight_color: None,
                underline: false,
                font_size: Some(12.),
                runs: Vec::new(),
                spans: Vec::new(),
                paragraphs: Vec::new(),
                object_spans: Vec::new(),
                text_sections: Vec::new(),
                margins: None,
                gravity: None,
            })
            .into(),
        );
        let svg = render_document_svg(&document(page), &RenderOptions::default())[0]
            .svg
            .clone();
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let text = xml
            .descendants()
            .filter(|node| node.has_tag_name("tspan"))
            .filter_map(|node| node.text())
            .collect::<String>();
        assert_eq!(text, "under the highlighter");
        let text_at = svg.find("<text").unwrap();
        let group_at = svg.find(r#"<g style="mix-blend-mode:darken">"#).unwrap();
        assert!(text_at < group_at);
        assert_eq!(svg.matches(r#"mix-blend-mode:darken"#).count(), 1);
        let body = &svg[..group_at];
        let batch = &svg[group_at..];
        assert_eq!(body.matches("fill-opacity=\"0.5020\"").count(), 1);
        assert_eq!(batch.matches("fill-opacity=\"0.5020\"").count(), 1);
        assert!(body.contains("M6,20"));
        assert!(batch.contains("M36,20"));
        assert!(!batch.contains("M6,20"));
    }

    #[test]
    fn top_layer_marker_darkens_covered_ink_and_leaves_bare_paper() {
        let mut page = page_with_uncolored_stroke();
        page.width = 32;
        page.height = 32;
        page.background_color = Some(Color {
            r: 255,
            g: 255,
            b: 255,
        });
        let mut red = marker(false, 16.);
        red.rendering.as_mut().unwrap().pen_name = None;
        red.rendering.as_mut().unwrap().advanced_settings = None;
        red.points = vec![Point { x: 16., y: 16. }, Point { x: 17., y: 16. }];
        red.color = Some(Color { r: 255, g: 0, b: 0 });
        red.pen_width = 30.;
        red.pressures.clear();
        let mut cyan = marker(true, 16.);
        cyan.points = vec![Point { x: 16., y: 16. }; 2];
        cyan.rendering.as_mut().unwrap().style.color_argb = Some(0xff00_ffff);
        cyan.color = Some(Color {
            r: 0,
            g: 255,
            b: 255,
        });
        page.objects = vec![red.into(), cyan.into()];
        page.clear_elements();
        let svg = &render_document_svg(&document(page), &RenderOptions::default())[0].svg;
        let tree = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default()).unwrap();
        let mut pixmap = resvg::tiny_skia::Pixmap::new(32, 32).unwrap();
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::identity(),
            &mut pixmap.as_mut(),
        );
        let pixel = |x, y| {
            let p = pixmap.pixel(x, y).unwrap();
            (p.red(), p.green(), p.blue())
        };
        assert_eq!(pixel(0, 0), (255, 255, 255), "bare paper");
        assert_eq!(pixel(16, 16), (0, 0, 0), "cyan over red darkens to black");
    }
    #[test]
    fn dark_paper_highlighter_is_visible_and_preserves_white_ink() {
        let mut page = page_with_uncolored_stroke();
        page.background_color = Some(Color { r: 0, g: 0, b: 0 });
        page.width = 40;
        page.height = 40;
        page.strokes_mut().next().unwrap().points =
            vec![Point { x: 24., y: 18. }, Point { x: 24., y: 22. }];
        page.strokes_mut().next().unwrap().color = Some(Color {
            r: 255,
            g: 255,
            b: 255,
        });
        page.strokes_mut().next().unwrap().pen_width = 10.;
        page.objects
            .extend([marker(true, 12.).into(), marker(true, 24.).into()]);
        let doc = document(page);
        let layout = layout_document(&doc);
        let options = RenderOptions::default();
        let normal = render_document_svg(&doc, &options).remove(0);
        let replay = super::render_layout_page_replay_svg(&doc, &layout, 0, &options).unwrap();
        let mut pixels = Vec::new();
        for rendered in [&normal, &replay] {
            assert!(!rendered.svg.contains("<image"));
            assert!(!rendered.svg.contains("<filter"));
            let tree = resvg::usvg::Tree::from_str(&rendered.svg, &Default::default()).unwrap();
            let mut pixmap = resvg::tiny_skia::Pixmap::new(40, 40).unwrap();
            resvg::render(
                &tree,
                resvg::tiny_skia::Transform::identity(),
                &mut pixmap.as_mut(),
            );
            let yellow = pixmap.pixel(12, 20).unwrap();
            assert!(yellow.red() > 100 && yellow.green() > 100 && yellow.blue() == 0);
            let white = pixmap.pixel(24, 20).unwrap();
            assert_eq!((white.red(), white.green(), white.blue()), (255, 255, 255));
            pixels.push(pixmap.data().to_vec());
        }
        assert_eq!(pixels[0], pixels[1]);
        #[cfg(feature = "pdf")]
        {
            let bytes =
                crate::render_svg_pages_pdf(&[normal], &crate::PdfOptions::default()).unwrap();
            let pdf = lopdf::Document::load_mem(&bytes).unwrap();
            let mut lighten = false;
            for object in pdf.objects.values() {
                let dict = match object {
                    lopdf::Object::Dictionary(dict) => dict,
                    lopdf::Object::Stream(stream) => &stream.dict,
                    _ => continue,
                };
                assert_ne!(
                    dict.get(b"Subtype").and_then(lopdf::Object::as_name).ok(),
                    Some(b"Image".as_slice())
                );
                lighten |= dict.get(b"BM").and_then(lopdf::Object::as_name).ok()
                    == Some(b"Lighten".as_slice());
            }
            assert!(lighten);
        }
    }

    #[test]
    fn marker4_keeps_full_width_and_applies_alpha_once_across_overlapping_stamps() {
        for (settings, height) in [("7;", 36.828), ("8;", 35.64)] {
            let mut stroke = marker(true, 30.);
            stroke.pen_width = 36.2;
            stroke.points = vec![
                Point { x: 30., y: 30. },
                Point { x: 50., y: 30. },
                Point { x: 70., y: 30. },
            ];
            stroke.pressures = vec![0.1, 0.9, 0.1];
            stroke.timestamps = vec![0, 8, 16];
            let rendering = stroke.rendering.as_mut().unwrap();
            rendering.pen_name = Some("com.samsung.android.sdk.pen.pen.preload.Marker4".into());
            rendering.advanced_settings = Some(settings.into());
            rendering.style.pen_size = Some(36.2);
            let ink = crate::prepare_stroke(&stroke, false);
            let stamp = ink.rect_stamp.unwrap();
            assert!((stamp.height - height).abs() < 1e-4);
            assert!(ink.segment_widths.is_none());
            assert_eq!(
                ink.sample_ends.as_ref().unwrap().last(),
                Some(&ink.points.len())
            );
            let mut page = page_with_uncolored_stroke();
            page.width = 100;
            page.height = 60;
            page.background_color = Some(Color {
                r: 255,
                g: 255,
                b: 255,
            });
            page.objects = vec![stroke.into()];
            page.clear_elements();
            let svg = &render_document_svg(&document(page), &RenderOptions::default())[0].svg;
            assert!(!svg.contains("<image"));
            assert!(!svg.contains("<filter"));
            let tree = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default()).unwrap();
            let mut pixmap = resvg::tiny_skia::Pixmap::new(100, 60).unwrap();
            resvg::render(
                &tree,
                resvg::tiny_skia::Transform::identity(),
                &mut pixmap.as_mut(),
            );
            // Native ARGB 0x80ffee00 over white, even where many stamps overlap.
            for (x, y) in [(40, 30), (50, 20), (60, 40)] {
                let p = pixmap.pixel(x, y).unwrap();
                assert_eq!((p.red(), p.green(), p.blue()), (255, 246, 127));
            }
            assert_eq!(pixmap.pixel(50, 5).unwrap().blue(), 255);
        }
    }
}
