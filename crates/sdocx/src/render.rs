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
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;
use vector::{
    Anchor, Blend, Circle, Clip, ClipPath, Data, Definitions, Ellipse, FontFamily, Group, Image,
    Line, LineCap, LineJoin, ObjectKind, PageTemplate, Paint, Path, Polygon, Rectangle, ReplayPart,
    ReplayPath, Scene, StrokeIndex, Styled, Svg, TSpan, Text, TextAnchor, TextDecoration,
    Transform, ViewBox, color_hex, coordinate, decimal,
};

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
mod native_body_pages;
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

/// Options controlling Svg rendering.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct RenderOptions {
    /// Explicit InkPen2 V9 paint profile. `None` preserves fallback behavior;
    /// `Some` opts into fresh state and identity page transform; the profile
    /// explicitly selects its enhanced-antialiasing paint route.
    #[cfg_attr(feature = "serde", serde(default))]
    pub inkpen2_profile: Option<InkPen2RenderProfile>,
    /// Explicit fresh Pencil3 V3 canvas and intrinsic texture policy.
    #[cfg_attr(feature = "serde", serde(default))]
    pub pencil3_profile: Option<Pencil3RenderProfile>,
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
        let settings = TextSettings::from_document(&document.metadata);
        let text_renderer = TextRenderer::new(settings, fonts)
            .with_point_marker_target(options.point_marker_target)
            .with_native_object_page_obstacles(text::NativeObjectPageObstacles::from_document(
                document,
            ))
            .with_table_export_page(table::TableExportPage::for_page(page, &document.metadata))
            .with_page_exclusions(text::PageExclusions::for_page(
                document,
                layout_page.source_page_index,
                settings,
            ));
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
            options.inkpen2_profile,
            options.pencil3_profile,
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
    inkpen2_profile: Option<InkPen2RenderProfile>,
    pencil3_profile: Option<Pencil3RenderProfile>,
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
        inkpen2_profile,
        pencil3_profile,
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
    inkpen2_profile: Option<InkPen2RenderProfile>,
    pencil3_profile: Option<Pencil3RenderProfile>,
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
                context.inkpen2_profile,
                context.pencil3_profile,
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
        PageElement::FormulaImage(image) => render_placed_image(svg, image, media_assets),
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
    let outline = project_shape_paint(
        svg,
        &line.style.paint,
        line.style.paint_source.as_ref(),
        None,
        PaintRole::Outline,
        theme,
    )
    .unwrap_or_else(|kind| {
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
    let width = bbox.x_max - bbox.x_min;
    let height = bbox.y_max - bbox.y_min;
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return;
    }
    let mut image = Image::embedded(&asset.data, &asset.mime_type)
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
    inkpen2_profile: Option<InkPen2RenderProfile>,
    pencil3_profile: Option<Pencil3RenderProfile>,
    slice: Option<PageSlice>,
) {
    // Saved ObjectDrawing admission requires at least two source points before
    // pen-name dispatch. Keep the context-free prepare_stroke compatibility
    // path unchanged; this renderer only paints persisted page objects.
    if stroke.points.len() <= 1 {
        return;
    }
    if native_allowed
        && let Some(profile) = pencil3_profile
        && let Some(ink) = prepare_pencil3_v3(stroke, profile, theme, viewport)
    {
        let context = pencil3::Pencil3PaintContext {
            canvas_width: profile.canvas_width,
            canvas_height: profile.canvas_height,
            background_scale: profile.screen_width.min(profile.screen_height) as f32 / 1440.0,
            paper_sampler: profile.paper_sampler,
            y_up: false,
        };
        if pencil3::render(svg, &ink, context, replay_index) {
            return;
        }
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
        && let Some(ink) = crate::ink::prepare_inkpen_v1_for_page(stroke)
        && let Some(vector) = inkpen_v1_vector_path(
            &ink.points,
            &ink.radii,
            &ink.sample_ends,
            ink.terminal.clone(),
            replay_index.is_some(),
        )
    {
        let color = theme.foreground(stroke_source_color(stroke));
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
    let mut paint = if native_allowed {
        crate::ink::prepare_stroke_for_context(
            stroke,
            false,
            crate::ink::InkRenderContext::CanonicalPage,
        )
    } else {
        crate::prepare_stroke(stroke, false)
    };
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
    let slice = if native_allowed { slice } else { None };
    if let Some(index) = replay_index {
        svg.scope(Group::new().replay_stroke(StrokeIndex(index)), |svg| {
            render_prepared_stroke(svg, &paint, true, slice);
        });
    } else {
        render_prepared_stroke(svg, &paint, false, slice);
    }
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
        if profile.enhanced_antialias
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
    let output = crate::ink::inkpen2::redraw_bounded(input, output_budget)?;
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
    if active_rainbow && output.vertices.iter().any(|record| record[5] != 1.0) {
        return None;
    }
    if output.vertices.is_empty() {
        return None;
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

/// Prepare replay metadata with a resolved theme and explicit InkPen2 screen
/// profile. For canvas-dependent Pencil3 profiles, use
/// [`prepare_stroke_for_viewport_with_theme`] with the actual rendered page size.
pub fn prepare_stroke_for_page_with_theme<'a>(
    stroke: &'a Stroke,
    theme: RenderTheme,
    options: &RenderOptions,
) -> crate::PreparedStroke<'a> {
    if stroke.points.len() <= 1 {
        return empty_saved_stroke_paint(stroke, theme);
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
    let Some(ink) = options
        .pencil3_profile
        .and_then(|profile| prepare_pencil3_v3(stroke, profile, theme, viewport))
    else {
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
    let color = &paint.color;
    if oblique::render(svg, paint, replay) {
        return;
    }
    if fountain::render(svg, paint, replay, slice) {
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
    // Preserve an explicit public color; recover saved RGB only for the
    // ordinary InkPen profiles whose source-color contract is established.
    stroke.color.or_else(|| {
        crate::ink::saved_inkpen_argb(stroke).map(|argb| Color {
            r: (argb >> 16) as u8,
            g: (argb >> 8) as u8,
            b: argb as u8,
        })
    })
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
        InkPen2RenderProfile, RenderColorMode, RenderOptions, RenderTheme, is_dark_background,
        normalized_stroke_width, prepare_inkpen2_v9, prepare_stroke_for_page_with_theme,
        render_document_svg, render_layout_page_replay_svg, render_layout_page_svg,
        sanitize_hyperlink_target,
    };
    use crate::{
        BoundingBox, Color, Document, DocumentMetadata, Page, PageElement, Point, RichTextBox,
        RichTextSpan, RichTextSpanType, Stroke, StrokeProperties, StrokeRendering, StrokeStyle,
        layout_document,
    };
    #[cfg(feature = "pdf")]
    use crate::{
        MediaAsset, ObjectSpanLayoutConstraint, ObjectSpanLayoutOption, ObjectType,
        PageObjectContent, PlacedImage, RichTextObjectContent, RichTextObjectSpan, RichTextSection,
    };

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
            std::fs::write(dir.join(format!("{name}.pdf")), &pdf).unwrap();
            std::fs::write(dir.join(format!("{name}.svg")), &svg).unwrap();
        }
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
    fn public_inkpen_alpha_is_applied_once_in_normal_and_replay_svg() {
        for setting in [None, Some(""), Some("4;")] {
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
            let page_height = if quiz { 15_372 } else { 9_162 };
            let flow_width = if quiz { 1_080 } else { width };
            let image_top = if quiz { 5_473.2021484375 } else { 100.0 };
            let mut body = theme_test_text();
            body.bbox = BoundingBox {
                x_min: 0.0,
                y_min: 0.0,
                x_max: f64::from(width),
                y_max: f64::from(page_height),
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
                }))),
                text_index_utf16: anchor as i32,
                layout_option: ObjectSpanLayoutOption::Block,
                layout_constraint: ObjectSpanLayoutConstraint::Normal,
            }];

            let mut page = page_with_uncolored_stroke();
            page.width = width;
            page.height = page_height;
            let mut document = document(page);
            document.metadata.page_mode = Some(1);
            document.metadata.default_page_dimensions = Some((360 * density, 1_527));
            document.metadata.flow_dimensions = Some((flow_width, 9_162));
            document.metadata.flow_page_padding = Some((0, 24));
            document.metadata.body_font_size_delta = Some(delta);
            document.metadata.note_text = Some(body);

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
            }
        }
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
