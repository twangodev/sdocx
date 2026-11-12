//! Presentation-oriented Svg rendering for parsed Samsung Notes documents.

use crate::progress::WorkProgress;
use crate::{Progress, ProgressStage};

use crate::{
    BoundingBox, Color, Document, LayoutDocument, MediaAsset, Page, PageElement,
    ParagraphAlignment, PredefinedTextStyle, RichTextBox, Stroke, layout_document,
};
use crate::{PageObject, PageObjectContent, composition::RenderPass};
use std::borrow::Cow;
use std::cell::Cell;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;
use vector::{
    Anchor, Blend, Circle, Clip, ClipPath, Data, Definitions, FontFamily, Group, Line, LineCap,
    LineJoin, ObjectKind, PageTemplate, Paint, Path, Rectangle, ReplayPart, ReplayPath, Scene,
    StrokeIndex, Styled, Svg, TSpan, Text, TextAnchor, TextDecoration, Transform, ViewBox,
    color_hex, coordinate, decimal,
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
mod images;
#[cfg(feature = "pdf")]
mod ink_canvas;
mod inkpen2_half_grid;
mod inkpen2_preparation;
use inkpen2_preparation::{prepare_inkpen2_internal, prepare_inkpen2_v1, prepare_inkpen2_v9};
mod inkpen2_vector;
mod native_translation;
mod object_drawing;
#[cfg(feature = "pdf")]
pub(crate) use ink_canvas::raster_scene as raster_canvas_scene;
#[cfg(all(test, feature = "pdf"))]
mod legacy_marker_alpha_tests;
mod marker2_scale;
mod marker3_scale;
mod simplebrush;
#[cfg(test)]
mod simplebrush_tests;
pub use geometry::{GeometryDiagnostic, GeometryDiagnosticKind};
mod old_pencil;
#[cfg(all(test, feature = "serde", feature = "pdf"))]
mod pencil2_tests;
mod pencil3;
pub use pencil3::Pencil3PaperSampler;
mod paint_diagnostic;
pub use paint_diagnostic::{PaintDiagnostic, PaintDiagnosticKind, PaintRole};
#[allow(clippy::all, dead_code, unused_imports, unexpected_cfgs)]
mod harfrust;
#[cfg(all(test, feature = "serde"))]
mod inkpen2;
#[cfg(feature = "pdf")]
pub(crate) mod legacy_marker_vector;
mod marker;
#[cfg(all(test, feature = "serde"))]
mod marker3;
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
mod shapes;
#[cfg(test)]
use shapes::{native_svg_path, shape_paint};
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

mod options;
pub use options::{
    AirBrushRenderProfile, ChineseBrushFwidthOutputProfile, ChineseBrushRenderProfile,
    Crayon2DocumentGridProfile, DefaultPenOutputScale, FountainV14OutputScaleProfile,
    FountainV16OutputScaleProfile, FountainV17OutputScaleProfile, FountainV17RainbowRenderProfile,
    InkPen2HalfGridViewport, InkPen2RenderProfile, InkPen2V1OutputScale, InkPenRenderProfile,
    LegacyMarkerCenterlineProfile, Marker3OutputScale, MontblancRenderProfile,
    Pencil3RenderProfile, RenderColorMode, RenderOptions, SimpleBrushRenderProfile,
};

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
    #[cfg(feature = "pdf")]
    paper_body_query: bool,
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
    #[cfg(feature = "pdf")]
    paper_body_query: bool,
}

const BODY_PLAN_CACHE_CAPACITY: usize = 2;

impl DocumentTextCache {
    #[cfg(feature = "pdf")]
    pub(crate) fn for_paper_export() -> Self {
        Self {
            paper_body_query: true,
            ..Self::default()
        }
    }
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
        {
            return None;
        }
        self.bind(document, options, fonts);
        let theme = RenderTheme::resolve(&layout_page.page, &document.metadata, options.color_mode);
        let renderer = page_text_renderer(document, layout_page, options, fonts);
        let body = PreparedBodyText::new(document, layout_page, theme, &renderer, self, None)?;
        body.plan
            .native_query_layout
            .as_ref()
            .and_then(|layout| {
                ContinuousBodyQuerySnapshot::from_native_text(&body.plan.text, layout)
            })
            .or_else(|| ContinuousBodyQuerySnapshot::from_plan(&body.plan, document))
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
                let (mut page, scene) = self.prepare_page(
                    document,
                    layout_page,
                    options,
                    false,
                    fonts,
                    true,
                    slice,
                    observer,
                );
                observer(Progress::pending(ProgressStage::Finalizing));
                let vector::FinishedRetainedScene {
                    svg,
                    text,
                    ink,
                    text_error,
                } = scene.finish_retained();
                page.svg = svg;
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
            options,
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
    #[cfg(feature = "pdf")]
    paper_body: bool,
}

struct PreparedBodyPlan {
    text: RichTextBox,
    layout: text::TextLayout,
    text_issues: Vec<text::SourceTextDiagnostic>,
    object_issues: Vec<text::SourceObjectDiagnostic>,
    exclusions: Option<text::PageExclusions>,
    native_pages: Result<native_body_pages::NativeBodyPages, text::NativePageIndexUnavailable>,
    #[cfg(feature = "pdf")]
    native_query_layout: Option<text::NativePageLayout>,
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
                (Cow::Borrowed(text), source_range, 0, 0, false)
            } else if let Some(window) = body.capture_window.as_ref() {
                (
                    Cow::Owned(page.body_text_capture(document)?),
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
                    Cow::Owned(page.body_text_reflow(document)?),
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
            #[cfg(feature = "pdf")]
            paper_body_query: cache.paper_body_query,
        };
        let plan = if let Some((_, plan)) = cache.plans.iter().find(|(stored, _)| *stored == key) {
            Rc::clone(plan)
        } else {
            let plan = Rc::new(PreparedBodyPlan::new(
                document,
                text.into_owned(),
                &key,
                renderer,
            )?);
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
            #[cfg(feature = "pdf")]
            paper_body: cache.paper_body_query,
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
                    #[cfg(feature = "pdf")]
                    if self.paper_body {
                        target = target.with_paper_body();
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
        let planner = TextRenderer::new(renderer.settings, renderer.font_book())
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
            && matches!(
                document.metadata.page_mode(),
                None | Some(crate::PageMode::PageList)
            )
            && document.metadata.note_text.as_ref().is_some_and(|source| {
                source.text_sections.is_empty() && crate::layout::same_text_source(source, &text)
            });
        #[cfg(feature = "pdf")]
        let paper_source = key.paper_body_query
            && !key.detached
            && document.metadata.page_mode() == Some(crate::PageMode::ContinuousCanvas)
            && key.first_page_index == 0
            && key.last_page_index == 0
            && document.pages.len() == 1
            && document.metadata.note_text.as_ref().is_some_and(|source| {
                source.text_area_type.is_none()
                    && source.bbox == BoundingBox::default()
                    && source.rotation_degrees.is_none()
                    && crate::layout::same_text_source(source, &text)
            });
        #[cfg(feature = "pdf")]
        let mut native_query_layout = None;
        #[cfg(not(feature = "pdf"))]
        let paper_source = false;
        let (layout, native_page_layout) = if paper_source {
            let (native_layout, measured) =
                text::layout_native_paper_text(&styled, frame, key.theme, &planner);
            #[cfg(feature = "pdf")]
            if let Ok(measured) = measured {
                native_query_layout = Some(measured);
                (
                    native_layout,
                    Err(text::NativePageIndexUnavailable::OutsideCertificate),
                )
            } else {
                let fallback_frame = text::TextFrame {
                    bbox: BoundingBox {
                        x_min: padding,
                        y_min: 0.0,
                        x_max: frame_width - padding,
                        y_max: 0.0,
                    },
                    gravity: None,
                    exclusions: &bands,
                };
                let mut layout =
                    text::layout_capture_text(&styled, fallback_frame, key.theme, &planner);
                layout.native_object_entry = None;
                (
                    layout,
                    Err(text::NativePageIndexUnavailable::OutsideCertificate),
                )
            }
            #[cfg(not(feature = "pdf"))]
            {
                (native_layout, measured)
            }
        } else if complete_source {
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
            #[cfg(feature = "pdf")]
            native_query_layout,
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
                if viewport.intersects(object_drawing::object_paint_bounds(
                    object,
                    left,
                    line.baseline,
                )) {
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
    options: &RenderOptions,
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

    let mut scene_text_renderer = text_renderer.clone();
    scene_text_renderer.image_page_width = Some(page.width);
    let brush_policy = BrushPolicy::new(options);
    let composition = CompositionContext {
        page,
        media_assets,
        flow_page_padding: metadata.flow_page_padding,
        text_renderer: &scene_text_renderer,
        theme,
        replay,
        body_text,
        brush_policy: &brush_policy,
        budgets: RenderBudgets::new(options),
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
    brush_policy: &'a BrushPolicy<'a>,
    budgets: RenderBudgets,
    slice: Option<PageSlice>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SimpleBrushMode {
    Disabled,
    HostV25,
    TwoPixelsPerPageUnit,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MontblancMode {
    Disabled,
    HostUnitScale,
    TwoPixelsPerPageUnit,
}

impl SimpleBrushMode {
    fn supports_stroke(self, stroke: &Stroke, slice: Option<PageSlice>) -> bool {
        match self {
            Self::Disabled => false,
            Self::HostV25 => true,
            Self::TwoPixelsPerPageUnit => {
                (2..=256).contains(&stroke.points.len())
                    && stroke.rendering.as_ref().is_some_and(|rendering| {
                        matches!(
                            rendering.advanced_settings.as_deref(),
                            None | Some("") | Some("1;")
                        ) && rendering.style.pen_size.is_some_and(|size| {
                            match rendering.tool_type_raw {
                                0 | 2 | 3 if stroke.points.len() == 3 => {
                                    (3.0..=32.0).contains(&size)
                                }
                                1 => {
                                    (3..=32).contains(&stroke.points.len())
                                        && (8.0..=12.0).contains(&size)
                                }
                                2 => (8.0..=20.0).contains(&size),
                                0 if (4..=128).contains(&stroke.points.len()) && size == 3.0 => {
                                    true
                                }
                                0 => (8.0..=20.0).contains(&size),
                                3 => (8.0..=20.0).contains(&size),
                                _ => false,
                            }
                        })
                    })
                    && slice.is_none()
            }
        }
    }
}

impl MontblancMode {
    fn supports_stroke(self, stroke: &Stroke) -> bool {
        match self {
            Self::Disabled => false,
            Self::HostUnitScale => true,
            Self::TwoPixelsPerPageUnit => {
                (stroke.points.len() <= 11
                    || ((12..=24).contains(&stroke.points.len())
                        && (0.5..=2.5).contains(&stroke.pen_width)))
                    && (!(10..=11).contains(&stroke.points.len())
                        || (0.5..=2.5).contains(&stroke.pen_width))
            }
        }
    }
}

struct BrushPolicy<'a> {
    options: &'a RenderOptions,
    simplebrush_mode: SimpleBrushMode,
    montblanc_mode: MontblancMode,
}

impl<'a> BrushPolicy<'a> {
    fn new(options: &'a RenderOptions) -> Self {
        Self {
            options,
            simplebrush_mode: match (
                options.simplebrush_profile,
                options.simplebrush_two_pixels_per_page_unit,
            ) {
                (Some(SimpleBrushRenderProfile::HostV25), false) => SimpleBrushMode::HostV25,
                (None, true) => SimpleBrushMode::TwoPixelsPerPageUnit,
                _ => SimpleBrushMode::Disabled,
            },
            montblanc_mode: match (
                options.montblanc_profile,
                options.montblanc_two_pixels_per_page_unit,
            ) {
                (Some(MontblancRenderProfile::HostUnitScale), false) => {
                    MontblancMode::HostUnitScale
                }
                (None, true) => MontblancMode::TwoPixelsPerPageUnit,
                _ => MontblancMode::Disabled,
            },
        }
    }
}

struct RenderBudgets {
    inkpen2_half_grid_budget: Cell<inkpen2_half_grid::Budget>,
    marker2_remaining_vertices: Cell<usize>,
    crayon2_grid_remaining: Cell<u64>,
    #[cfg(feature = "pdf")]
    legacy_marker_budget: std::cell::RefCell<legacy_marker_vector::SourcePageBudget>,
    // Shared across all passes in this composition; each scene gets its own budget.
    #[cfg(feature = "pdf")]
    old_pencil_pdf_remaining_forms: Cell<usize>,
    marker4_v7_remaining_stamps: Cell<usize>,
    old_pencil_remaining_stamps: Cell<usize>,
    old_pencil_remaining_points: Cell<usize>,
    simplebrush_remaining_stamps: Cell<usize>,
    simplebrush_long_remaining_stamps: Cell<usize>,
    montblanc_remaining_patches: Cell<usize>,
}

impl RenderBudgets {
    fn new(options: &RenderOptions) -> Self {
        Self {
            inkpen2_half_grid_budget: Cell::new({
                #[cfg(test)]
                {
                    let [generated_stamps, tested_fragments, retained_cells] = options
                        .inkpen2_half_grid_test_budget
                        .unwrap_or([32_768, 1_000_000, 100_000]);
                    inkpen2_half_grid::Budget {
                        generated_stamps,
                        tested_fragments,
                        retained_cells,
                    }
                }
                #[cfg(not(test))]
                {
                    inkpen2_half_grid::Budget {
                        generated_stamps: 32_768,
                        tested_fragments: 1_000_000,
                        retained_cells: 100_000,
                    }
                }
            }),
            marker2_remaining_vertices: Cell::new(65_536),
            crayon2_grid_remaining: Cell::new(
                options
                    .crayon2_grid_profile
                    .as_ref()
                    .map_or(0, |p| p.max_samples.min(100_000)),
            ),
            #[cfg(feature = "pdf")]
            legacy_marker_budget: std::cell::RefCell::new(
                legacy_marker_vector::SourcePageBudget::default(),
            ),
            #[cfg(feature = "pdf")]
            old_pencil_pdf_remaining_forms: Cell::new(2),
            marker4_v7_remaining_stamps: Cell::new(32768),
            old_pencil_remaining_stamps: Cell::new(512),
            old_pencil_remaining_points: Cell::new(8192),
            simplebrush_remaining_stamps: Cell::new(32_768),
            simplebrush_long_remaining_stamps: Cell::new(4_096),
            montblanc_remaining_patches: Cell::new(32_768),
        }
    }
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
            let handled = native_allowed
                && !context.replay
                && context.slice.is_none()
                && try_render_full_page_stroke(svg, stroke, context) == StrokeDispatch::Handled;
            if !handled {
                render_stroke(
                    svg,
                    stroke,
                    context.theme,
                    context.replay.then_some(*stroke_index),
                    calligraphy_session,
                    native_allowed,
                    [context.page.width, context.page.height],
                    context.brush_policy,
                    &context.budgets,
                    context.slice,
                );
            }
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

fn try_render_full_page_stroke(
    svg: &mut Scene,
    stroke: &Stroke,
    context: &CompositionContext<'_>,
) -> StrokeDispatch {
    if let Some(profile) = context.brush_policy.options.crayon2_grid_profile.as_ref()
        && [context.page.width, context.page.height] == [profile.width, profile.height]
        && stroke.color.is_some_and(|color| {
            let effective = context.theme.foreground_color(color);
            [effective.r, effective.g, effective.b]
                == [profile.rgba[0], profile.rgba[1], profile.rgba[2]]
                    .map(|v| (v * 255.).round() as u8)
        })
        && let (Ok(phase_x), Ok(phase_y)) = (
            profile.phase_x.as_slice().try_into(),
            profile.phase_y.as_slice().try_into(),
        )
    {
        let grid = crate::Crayon2HostGrid {
            phase_x,
            phase_y,
            point_atlas: &profile.point_atlas,
            background_texture: &profile.background_texture,
            width: profile.width,
            height: profile.height,
            clip_scale: profile.clip_scale,
            clip_translation: profile.clip_translation,
            background_scale: profile.background_scale,
            rgba: profile.rgba,
            background: None,
            max_samples: context.budgets.crayon2_grid_remaining.get(),
        };
        if let Ok((fragments, used)) = crate::ink::crayon2::sample_host_grid(stroke, &grid) {
            for fragment in fragments {
                let color = format!(
                    "#{:02x}{:02x}{:02x}",
                    fragment.rgb[0], fragment.rgb[1], fragment.rgb[2]
                );
                let bounds = BoundingBox {
                    x_min: f64::from(fragment.x),
                    y_min: f64::from(fragment.y),
                    x_max: f64::from(fragment.x + 1),
                    y_max: f64::from(fragment.y + 1),
                };
                svg.push(
                    rectangle(bounds, 0., 5)
                        .fill(Paint::from_hex(&color))
                        .fill_opacity(f64::from(fragment.alpha)),
                );
            }
            context
                .budgets
                .crayon2_grid_remaining
                .set(context.budgets.crayon2_grid_remaining.get() - used);
            return StrokeDispatch::Handled;
        }
    }
    #[cfg(feature = "pdf")]
    if context.brush_policy.options.old_pencil_pdf_profile
        && context.budgets.old_pencil_pdf_remaining_forms.get() > 0
        && [context.page.width, context.page.height] == [1920, 1080]
        && old_pencil::render_pdf(
            svg,
            stroke,
            context.theme,
            &context.budgets.old_pencil_remaining_stamps,
            &context.budgets.old_pencil_remaining_points,
        )
    {
        context
            .budgets
            .old_pencil_pdf_remaining_forms
            .set(context.budgets.old_pencil_pdf_remaining_forms.get() - 1);
        return StrokeDispatch::Handled;
    }
    #[cfg(feature = "pdf")]
    if let Some(profile) = context.brush_policy.options.legacy_marker_vector_profile
        && legacy_marker_vector::render(
            svg,
            stroke,
            context.theme,
            [context.page.width, context.page.height],
            profile,
            &mut context.budgets.legacy_marker_budget.borrow_mut(),
        )
    {
        return StrokeDispatch::Handled;
    }
    if let Some(view) = context.brush_policy.options.inkpen2_half_grid_viewport
        && context
            .brush_policy
            .options
            .inkpen2_profile
            .is_some_and(|p| {
                matches!(
                    (p.screen_width, p.screen_height),
                    (1440, 2560) | (2560, 1440)
                )
            })
        && view.width.checked_mul(2) == Some(context.page.width)
        && view.height.checked_mul(2) == Some(context.page.height)
        && let Some(rendering) = stroke.rendering.as_ref()
        && rendering
            .style
            .color_argb
            .is_some_and(|argb| argb >> 24 == 255)
        && let Some(ink) = prepare_inkpen2_internal(
            stroke,
            context.brush_policy.options.inkpen2_profile,
            context.theme,
            Some(InkPen2V1OutputScale::Half),
            true,
        )
        && inkpen2_half_grid::render(svg, &ink, view, &context.budgets.inkpen2_half_grid_budget)
    {
        return StrokeDispatch::Handled;
    }
    StrokeDispatch::Fallback
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
            images::render_image(svg, *bbox, Some(*media_index), None, media_assets);
        }
        PageElement::PlacedImage(image) => {
            images::render_placed_image(svg, image, media_assets, Some(page.width))
        }
        // Native fill-effect transparency is defined only for type-3 images.
        PageElement::FormulaImage(image) => {
            images::render_placed_image_body(svg, image, media_assets)
        }
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
            if let Err(kind) = shapes::render_shape(
                svg,
                shape,
                theme,
                source_offset,
                context
                    .brush_policy
                    .options
                    .rotated_arc_sector_approximation,
            ) {
                svg.report_geometry_issue(GeometryDiagnostic {
                    source_offset,
                    object_uuid: shape.metadata.uuid.clone(),
                    kind,
                });
            }
            if let Some(text) = &shape.text {
                let theme = match shape.fill {
                    crate::ShapePaint::Solid(argb) => theme.on_surface(
                        theme.foreground_color(argb_rgb(argb)),
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
            if let Err(kind) = shapes::render_line(svg, line, theme, source_offset) {
                svg.report_geometry_issue(GeometryDiagnostic {
                    source_offset,
                    object_uuid: line.metadata.uuid.clone(),
                    kind,
                });
            }
        }
    }
}

fn rectangle(bbox: BoundingBox, offset_y: f64, precision: usize) -> Rectangle {
    Rectangle::new()
        .x(decimal(bbox.x_min, precision))
        .y(decimal(bbox.y_min + offset_y, precision))
        .width(decimal(bbox.x_max - bbox.x_min, precision))
        .height(decimal(bbox.y_max - bbox.y_min, precision))
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
                    #[cfg(feature = "pdf")]
                    let first_block = (target.is_paper_body() && svg.retains_text())
                        .then(|| svg.native_text().len());
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
                    #[cfg(feature = "pdf")]
                    if let Some(first_block) = first_block {
                        svg.native_text().set_new_paper_lines(
                            first_block,
                            index,
                            line.top,
                            line.bottom,
                        );
                    }
                }
            }
            target.paint_conservative(svg, |svg| {
                object_drawing::paint_line_objects(
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

fn aligned_line_left(
    line: &text::WrappedLine,
    left: f64,
    width: f64,
    alignment: Option<ParagraphAlignment>,
) -> f64 {
    left + text::line_alignment_offset(line.advance, width, alignment)
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

#[derive(Debug, PartialEq)]
enum MagicPenMinimumDot {
    Invisible,
    Cell { x: i32, y: i32, alpha: f64 },
}

fn magicpen_minimum_dot_cell(
    records: &[[u32; 3]],
    viewport: [u32; 2],
) -> Option<MagicPenMinimumDot> {
    let [record] = records else { return None };
    let [x, y, radius] = record.map(|word| f64::from(f32::from_bits(word)));
    if radius != 0.5
        || viewport[0] != viewport[1]
        || !(32..=256).contains(&viewport[0])
        || !viewport[1].is_multiple_of(2)
        || !x.is_finite()
        || !y.is_finite()
        || x < 2.0
        || y < 2.0
        || x + 2.0 >= f64::from(viewport[0])
        || y + 2.0 >= f64::from(viewport[1])
        || (x * 4.0).fract() != 0.0
        || (y * 4.0).fract() != 0.0
        || x > 200.5
        || y > 150.5
    {
        return None;
    }
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let distance = |px: i32, py: i32| (f64::from(px) + 0.5 - x).hypot(f64::from(py) + 0.5 - y);
    let remaining = radius - distance(ix, iy);
    if remaining <= 0.0 {
        return Some(MagicPenMinimumDot::Invisible);
    }
    let qx = ix.div_euclid(2) * 2;
    let qy = iy.div_euclid(2) * 2;
    let width = (distance(qx + 1, iy) - distance(qx, iy)).abs()
        + (distance(ix, qy + 1) - distance(ix, qy)).abs();
    let alpha = if width == 0.0 {
        1.0
    } else {
        (remaining / width).clamp(0.0, 1.0)
    };
    Some(MagicPenMinimumDot::Cell {
        x: ix,
        y: iy,
        alpha: (alpha * 255.0).round() / 255.0,
    })
}

#[cfg(test)]
#[test]
fn magicpen_minimum_dot_cell_is_bounded_to_captured_grid() {
    let record = |x: f32, y: f32, r: f32| [x.to_bits(), y.to_bits(), r.to_bits()];
    assert_eq!(
        magicpen_minimum_dot_cell(&[record(8.0, 8.0, 0.5)], [32, 32]),
        Some(MagicPenMinimumDot::Invisible)
    );
    assert!(matches!(
        magicpen_minimum_dot_cell(&[record(8.5, 8.5, 0.5)], [32, 32]),
        Some(MagicPenMinimumDot::Cell { x: 8, y: 8, .. })
    ));
    assert_eq!(
        magicpen_minimum_dot_cell(&[record(8.1, 8.5, 0.5)], [32, 32]),
        None
    );
    assert_eq!(
        magicpen_minimum_dot_cell(&[record(8.5, 8.5, 0.5)], [32, 31]),
        None
    );
    assert_eq!(
        magicpen_minimum_dot_cell(&[record(1.5, 8.5, 0.5)], [32, 32]),
        None
    );
    assert_eq!(
        magicpen_minimum_dot_cell(&[record(8.5, 8.5, 1.0)], [32, 32]),
        None
    );
    assert_eq!(
        magicpen_minimum_dot_cell(&[record(8.5, 8.5, 0.5); 2], [32, 32]),
        None
    );
}

fn magicpen_v1_vector_path(
    ink: &crate::ink::PreparedMagicPenV1,
    replay: bool,
    scale: u32,
) -> Option<Path> {
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
        let data = crate::ink::magicpen_source_contour_path(std::slice::from_ref(record), scale)?;
        if !path.push_serialized(&data, replay, MAX_PATH_BYTES) {
            return None;
        }
    }
    Some(path.finish())
}

fn magicpen_source_color(stroke: &Stroke, argb: u32) -> Option<Color> {
    stroke.color.or(Some(argb_rgb(argb)))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum StrokeDispatch {
    Handled,
    Fallback,
}

fn try_render_fountain_profile(
    svg: &mut Scene,
    stroke: &Stroke,
    theme: RenderTheme,
    replay_index: Option<usize>,
    slice: Option<PageSlice>,
    options: &RenderOptions,
) -> StrokeDispatch {
    let fountain_v17_rainbow_profile = options.fountain_v17_rainbow_profile.as_ref();
    let fountain_v17_output_scale_profile = options.fountain_v17_output_scale_profile;
    let fountain_v16_output_scale_profile = options.fountain_v16_output_scale_profile;
    if stroke.rendering.as_ref().is_some_and(|rendering| {
        rendering.pen_name.as_deref() == Some("com.samsung.android.sdk.pen.pen.preload.FountainPen")
            && rendering.properties.rainbow_effect
    }) && let Some(profile) = fountain_v17_rainbow_profile
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
            render_fountain_profile_stroke(svg, stroke, &paint, theme, replay_index, slice);
            return StrokeDispatch::Handled;
        }
    }
    if fountain_v17_rainbow_profile.is_none()
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
            render_fountain_profile_stroke(svg, stroke, &paint, theme, replay_index, slice);
            return StrokeDispatch::Handled;
        }
    }
    StrokeDispatch::Fallback
}

fn render_fountain_profile_stroke(
    svg: &mut Scene,
    stroke: &Stroke,
    paint: &crate::PreparedStroke<'_>,
    theme: RenderTheme,
    replay_index: Option<usize>,
    slice: Option<PageSlice>,
) {
    let draw = |svg: &mut Scene| {
        render_prepared_stroke_with_fallback(svg, paint, replay_index.is_some(), slice, || {
            Some(prepare_context_stroke_with_theme(stroke, theme))
        });
    };
    if let Some(index) = replay_index {
        svg.scope(Group::new().replay_stroke(StrokeIndex(index)), draw);
    } else {
        draw(svg);
    }
}

fn try_render_montblanc_profile(
    svg: &mut Scene,
    stroke: &Stroke,
    theme: RenderTheme,
    viewport: [u32; 2],
    montblanc_mode: MontblancMode,
    montblanc_remaining_patches: &Cell<usize>,
) -> StrokeDispatch {
    if montblanc_mode.supports_stroke(stroke)
        && viewport.iter().all(|d| *d > 0)
        && let Some((buffers, color)) = crate::ink::montblanc::prepare(stroke)
        && montblanc_candidate::render(
            svg,
            &buffers,
            &theme.foreground(Some(color)),
            montblanc_remaining_patches,
            if montblanc_mode == MontblancMode::TwoPixelsPerPageUnit {
                2.0
            } else {
                1.0
            },
        )
    {
        return StrokeDispatch::Handled;
    }
    StrokeDispatch::Fallback
}

fn try_render_simplebrush_profile(
    svg: &mut Scene,
    stroke: &Stroke,
    theme: RenderTheme,
    viewport: [u32; 2],
    slice: Option<PageSlice>,
    simplebrush_mode: SimpleBrushMode,
    budgets: &RenderBudgets,
) -> StrokeDispatch {
    let simplebrush_remaining_stamps =
        if simplebrush_mode == SimpleBrushMode::TwoPixelsPerPageUnit && stroke.points.len() > 128 {
            &budgets.simplebrush_long_remaining_stamps
        } else {
            &budgets.simplebrush_remaining_stamps
        };
    // The experimental renderer is deliberately disabled for replay until it
    // carries correct per-source-sample and terminal part boundaries.
    if simplebrush_mode.supports_stroke(stroke, slice)
        && viewport.iter().all(|dimension| *dimension > 0)
        && let Ok(paint) = crate::ink::simplebrush::prepare_stroke_for_scale(
            stroke,
            true,
            simplebrush_remaining_stamps.get(),
            simplebrush_mode == SimpleBrushMode::TwoPixelsPerPageUnit,
        )
        && simplebrush::render_at_scale(
            svg,
            &paint.samples,
            &theme.foreground(Some(paint.color)),
            if simplebrush_mode == SimpleBrushMode::TwoPixelsPerPageUnit {
                2.0
            } else {
                1.0
            },
            slice.map(|slice| {
                let top = f64::from(slice.y());
                (top, top + f64::from(slice.height()))
            }),
        )
    {
        simplebrush_remaining_stamps.set(simplebrush_remaining_stamps.get() - paint.samples.len());
        return StrokeDispatch::Handled;
    }
    StrokeDispatch::Fallback
}

fn try_render_marker_output_profile(
    svg: &mut Scene,
    stroke: &Stroke,
    paint: &crate::PreparedStroke<'_>,
    policy: &BrushPolicy<'_>,
    budgets: &RenderBudgets,
) -> StrokeDispatch {
    let marker3_output_scale = policy.options.marker3_output_scale;
    let marker2_output_scale = policy.options.marker2_output_scale;
    let marker2_remaining_vertices = &budgets.marker2_remaining_vertices;
    if marker2_output_scale == Some(2)
        && paint.profile == Some("Marker2")
        && stroke.points.len() >= 2
        && stroke
            .points
            .iter()
            .all(|p| p.x.is_finite() && p.y.is_finite() && p.x.abs() <= 256.0 && p.y.abs() <= 256.0)
        && stroke.rendering.as_ref().is_some_and(|r| {
            r.tool_type_raw == 2
                && r.advanced_settings.as_deref() == Some("")
                && r.properties.compressed
                && r.style.stroke_type_raw == Some(0)
        })
        && paint.dot_radii.as_ref().is_some_and(|r| {
            !r.is_empty()
                && (r.iter().all(|&radius| radius == 1.0) || r.iter().all(|&radius| radius == 1.5))
        })
        && marker2_scale::render(
            svg,
            paint,
            Marker3OutputScale::Two,
            marker2_remaining_vertices,
        )
    {
        return StrokeDispatch::Handled;
    }
    if matches!(paint.profile, Some("Marker3" | "StraightMarker"))
        && stroke.rendering.as_ref().is_some_and(|rendering| {
            (rendering.tool_type_raw == 2
                || (rendering.tool_type_raw == 1
                    && (marker3_output_scale == Some(Marker3OutputScale::One)
                        || marker3_output_scale == Some(Marker3OutputScale::Two)
                        || marker3_output_scale == Some(Marker3OutputScale::Four))))
                && matches!(
                    rendering.pen_name.as_deref(),
                    Some(
                        "com.samsung.android.sdk.pen.pen.preload.Marker3"
                            | "com.samsung.android.sdk.pen.pen.preload.StraightMarker"
                    )
                )
        })
        && let Some(profile) = marker3_output_scale
        && marker3_scale::render(svg, paint, profile)
    {
        return StrokeDispatch::Handled;
    }
    StrokeDispatch::Fallback
}

#[allow(clippy::too_many_arguments)]
fn render_stroke(
    svg: &mut Scene,
    stroke: &Stroke,
    theme: RenderTheme,
    replay_index: Option<usize>,
    calligraphy_session: &mut crate::ink::calligraphy::CalligraphySession,
    native_allowed: bool,
    viewport: [u32; 2],
    policy: &BrushPolicy<'_>,
    budgets: &RenderBudgets,
    slice: Option<PageSlice>,
) {
    let inkpen2_profile = policy.options.inkpen2_profile;
    let inkpen2_v1_output_scale = policy.options.inkpen2_v1_output_scale;
    let chinesebrush_profile = policy.options.chinesebrush_profile;
    let chinesebrush_fwidth_output_profile = policy.options.chinesebrush_fwidth_output_profile;
    let fountain_v16_output_scale_profile = policy.options.fountain_v16_output_scale_profile;
    let fountain_v14_output_scale_profile = policy.options.fountain_v14_output_scale_profile;
    let defaultpen_output_scale = policy.options.defaultpen_output_scale;
    let magicpen_output_scale = policy.options.magicpen_output_scale;
    let magicpen_host_minimum_dot_grid = policy.options.magicpen_host_minimum_dot_grid;
    let old_pencil_svg_profile = policy.options.old_pencil_svg_profile;
    let pencil3_profile = policy.options.pencil3_profile;
    let airbrush_profile = policy.options.airbrush_profile;
    let legacy_marker_centerline_profile = policy.options.legacy_marker_centerline_profile;
    let marker4_v7_finger_output_scale = policy.options.marker4_v7_finger_output_scale;
    let marker4_v7_remaining_stamps = &budgets.marker4_v7_remaining_stamps;
    let old_pencil_remaining_stamps = &budgets.old_pencil_remaining_stamps;
    let old_pencil_remaining_points = &budgets.old_pencil_remaining_points;

    // Saved ObjectDrawing admission requires at least two source points before
    // pen-name dispatch. Keep the context-free prepare_stroke compatibility
    // path unchanged; this renderer only paints persisted page objects.
    if stroke.points.len() <= 1 {
        return;
    }
    if native_allowed
        && try_render_fountain_profile(svg, stroke, theme, replay_index, slice, policy.options)
            == StrokeDispatch::Handled
    {
        return;
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
        && old_pencil_svg_profile
        && replay_index.is_none()
        && slice.is_none()
        && viewport == [1920, 1080]
        && old_pencil::render(
            svg,
            stroke,
            theme,
            old_pencil_remaining_stamps,
            old_pencil_remaining_points,
        )
    {
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
    if native_allowed && airbrush_profile == Some(AirBrushRenderProfile::SavedV1) {
        match airbrush::prepare_source(
            stroke,
            theme,
            viewport,
            crate::ink::airbrush::SavedVersion::V1,
        ) {
            Some(airbrush::SourcePreparation::Empty) => return,
            Some(airbrush::SourcePreparation::Paint(paint))
                if airbrush::render(svg, &paint, replay_index) =>
            {
                return;
            }
            _ => {}
        }
    }
    if native_allowed
        && airbrush_profile == Some(AirBrushRenderProfile::SavedV2)
        && let Some(paint) = airbrush::prepare(stroke, theme, viewport)
        && airbrush::render(svg, &paint, replay_index)
    {
        return;
    }
    if native_allowed
        && replay_index.is_none()
        && slice.is_none()
        && try_render_montblanc_profile(
            svg,
            stroke,
            theme,
            viewport,
            policy.montblanc_mode,
            &budgets.montblanc_remaining_patches,
        ) == StrokeDispatch::Handled
    {
        return;
    }
    // Replay is excluded until the renderer carries per-source-sample and terminal boundaries.
    if native_allowed
        && replay_index.is_none()
        && try_render_simplebrush_profile(
            svg,
            stroke,
            theme,
            viewport,
            slice,
            policy.simplebrush_mode,
            budgets,
        ) == StrokeDispatch::Handled
    {
        return;
    }
    if native_allowed
        && replay_index.is_none()
        && let Some(scale) = inkpen2_v1_output_scale
        && let Some(ink) = prepare_inkpen2_v1(stroke, inkpen2_profile, theme, scale)
    {
        render_inkpen2_v9(svg, &ink, None);
        return;
    }
    if native_allowed && let Some(ink) = prepare_inkpen2_v9(stroke, inkpen2_profile, theme) {
        render_inkpen2_v9(svg, &ink, replay_index);
        return;
    }
    if native_allowed
        && viewport.iter().all(|dimension| *dimension > 0)
        && let Some(ink) = crate::ink::prepare_magicpen_v1_source(stroke)
        && let Some(path) = magicpen_v1_vector_path(
            &ink,
            replay_index.is_some(),
            magicpen_output_scale
                .filter(|scale| matches!(scale, 1 | 2 | 4))
                .filter(|_| replay_index.is_none() && slice.is_none())
                .unwrap_or(1),
        )
    {
        let color = theme.foreground(magicpen_source_color(stroke, ink.argb));
        let opacity = f64::from((ink.argb >> 24) as u8) / 255.0;
        if magicpen_host_minimum_dot_grid
            && magicpen_output_scale == Some(1)
            && replay_index.is_none()
            && slice.is_none()
            && let Some(cell) = magicpen_minimum_dot_cell(&ink.records, viewport)
        {
            if let MagicPenMinimumDot::Cell { x, y, alpha } = cell {
                svg.push(
                    Rectangle::new()
                        .x(f64::from(x))
                        .y(f64::from(y))
                        .width(1.0)
                        .height(1.0)
                        .fill(Paint::from_hex(&color))
                        .fill_opacity(alpha * opacity),
                );
            }
            return;
        }
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
        && let Some(page) = prepare_page_inkpen_geometry(
            stroke,
            policy.options,
            InkPenV1Admission::Canonical,
            replay_index.is_some(),
        )
    {
        let color = theme.foreground(stroke.color.or(Some(argb_rgb(page.ink.argb))));
        let opacity = f64::from((page.ink.argb >> 24) as u8) / 255.0;
        let draw = |svg: &mut Scene| {
            svg.scope(Group::new().opacity(opacity), |svg| {
                svg.push(page.vector.path.fill(Paint::from_hex(&color)));
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
        && replay_index.is_none()
        && slice.is_none()
        && let Some(profile) = defaultpen_output_scale
        && paint.ordered_disk_replay.is_some()
        && let Some(radii) = paint.dot_radii.as_mut()
    {
        let scale = match profile {
            DefaultPenOutputScale::Half => 0.5,
            DefaultPenOutputScale::One => 1.0,
            DefaultPenOutputScale::Two => 2.0,
            DefaultPenOutputScale::Four => 4.0,
            DefaultPenOutputScale::Eight => 8.0,
        };
        for radius in radii {
            *radius = (*radius - 0.2 / scale).max(0.0);
        }
    }
    if native_allowed
        && paint.profile == Some("Marker3")
        && paint
            .dot_radii
            .as_ref()
            .is_some_and(|radii| !radii.is_empty() && radii.iter().all(|&radius| radius == 0.0))
    {
        return;
    }
    if native_allowed
        && replay_index.is_none()
        && slice.is_none()
        && try_render_marker_output_profile(svg, stroke, &paint, policy, budgets)
            == StrokeDispatch::Handled
    {
        return;
    }
    if native_allowed
        && replay_index.is_none()
        && slice.is_none()
        && matches!(marker4_v7_finger_output_scale, Some(1 | 2 | 4))
        && let Some(ink) =
            crate::ink::marker4::prepare_v7_finger_bounded(stroke, marker4_v7_remaining_stamps)
    {
        paint.replace_geometry(crate::ink::PreparedGeometry::RectangularStamps {
            points: ink.points,
            sample_ends: ink.sample_ends,
            stamp: ink.stamp,
        });
        paint.opacity = ink.opacity;
    }
    if native_allowed
        && replay_index.is_none()
        && let Some(profile) = legacy_marker_centerline_profile
        && let Some(points) = crate::ink::legacy_marker_mesh::prepare(
            stroke,
            [profile.screen_width, profile.screen_height],
        )
    {
        paint.replace_geometry(crate::ink::PreparedGeometry::UnindexedPolyline { points });
        if let Some((argb, size)) = stroke.rendering.as_ref().and_then(|r| {
            if matches!(r.advanced_settings.as_deref(), Some("" | "1;")) {
                Some((r.style.color_argb.unwrap_or(0xff00_0000), r.style.pen_size?))
            } else {
                None
            }
        }) {
            paint.width = f64::from(size.clamp(0.4, 800.0).trunc());
            paint.opacity = f64::from(argb >> 24) / 255.0;
            paint.color = theme.foreground(stroke.color.or(Some(argb_rgb(argb))));
        }
    }
    if native_allowed
        && paint.profile == Some("FountainPen")
        && paint.dot_directions.is_some()
        && let Some(profile) = fountain_v14_output_scale_profile
        && valid_fountain_v14_output_scale(profile.pixels_per_page_unit)
    {
        crate::ink::apply_fountain_v14_output_scale(
            &mut paint,
            stroke,
            profile.pixels_per_page_unit,
        );
    }
    if !native_allowed {
        apply_context_stroke_theme(stroke, theme, &mut paint);
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

#[cfg(test)]
fn prepare_page_inkpen(
    stroke: &Stroke,
    profile: Option<InkPenRenderProfile>,
) -> Option<crate::ink::PreparedInkPenV1> {
    select_page_inkpen(stroke, profile, None, InkPenV1Admission::Canonical).map(|(ink, _)| ink)
}

#[derive(Clone, Copy)]
enum InkPenV1Admission {
    // Canonical exports also admit unscaled V1 without fixed-width support.
    Canonical,
    // The themed replay override only tries V1 early for a configured scale.
    ConfiguredScaleOnly,
}

fn select_page_inkpen(
    stroke: &Stroke,
    profile: Option<InkPenRenderProfile>,
    v1_scale: Option<u32>,
    admission: InkPenV1Admission,
) -> Option<(crate::ink::PreparedInkPenV1, bool)> {
    profile
        .and_then(|profile| {
            crate::ink::prepare_inkpen_v4_for_page(
                stroke,
                profile.screen_width,
                profile.screen_height,
            )
        })
        .map(|ink| (ink, true))
        .or_else(|| {
            let configured_scale = v1_scale.is_some_and(|scale| matches!(scale, 1..=8));
            if matches!(admission, InkPenV1Admission::ConfiguredScaleOnly) && !configured_scale {
                return None;
            }
            crate::ink::prepare_inkpen_v1_for_page_with_fixed_width(stroke, configured_scale)
                .map(|ink| (ink, false))
        })
}

struct PageInkPenGeometry {
    ink: crate::ink::PreparedInkPenV1,
    vector: InkPenV1VectorPath,
}

impl PageInkPenGeometry {
    fn new(ink: crate::ink::PreparedInkPenV1, replay: bool, output_scale: u32) -> Option<Self> {
        let vector = inkpen_v1_vector_path(
            &ink.points,
            &ink.radii,
            &ink.sample_ends,
            ink.terminal.clone(),
            replay,
            output_scale,
        )?;
        Some(Self { ink, vector })
    }

    fn apply_to_prepared(self, prepared: &mut crate::PreparedStroke<'_>, color: String) {
        prepared.replace_geometry(crate::ink::PreparedGeometry::SampledDisks {
            points: self.ink.points,
            sample_ends: self.vector.sample_ends,
            radii: self.ink.radii.into_iter().map(f64::from).collect(),
        });
        prepared.bounds = Some(self.ink.bounds);
        prepared.color = color;
        prepared.opacity = f64::from((self.ink.argb >> 24) as u8) / 255.0;
        prepared.top_layer = false;
        prepared.profile = Some("InkPen");
        prepared.support = crate::InkSupport::Reconstructed;
    }
}

fn prepare_page_inkpen_geometry(
    stroke: &Stroke,
    options: &RenderOptions,
    admission: InkPenV1Admission,
    replay: bool,
) -> Option<PageInkPenGeometry> {
    let (ink, is_v4) = select_page_inkpen(
        stroke,
        options.inkpen_profile,
        options.inkpen_v1_output_scale,
        admission,
    )?;
    let output_scale = if is_v4 {
        options.inkpen_v4_output_scale
    } else {
        options.inkpen_v1_output_scale
    }
    .filter(|scale| matches!(scale, 1..=8))
    .unwrap_or(1);
    PageInkPenGeometry::new(ink, replay, output_scale)
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
        && magicpen_v1_vector_path(&ink, false, 1).is_some()
    {
        let color =
            RenderTheme::for_canvas(dark_mode).foreground(magicpen_source_color(stroke, ink.argb));
        let mut sample_ends = ink.sample_ends;
        if let Some(last) = sample_ends.last_mut() {
            *last = ink.points.len();
        }
        prepared.replace_geometry(crate::ink::PreparedGeometry::SampledDisks {
            points: ink.points,
            sample_ends,
            radii: ink.radii.into_iter().map(f64::from).collect(),
        });
        prepared.bounds = Some(ink.bounds);
        prepared.color = color;
        prepared.opacity = f64::from((ink.argb >> 24) as u8) / 255.0;
        prepared.top_layer = false;
        prepared.profile = Some("MagicPen");
        prepared.support = crate::InkSupport::Reconstructed;
    }
    if let Some(ink) = crate::ink::prepare_inkpen_v1_for_page(stroke)
        && let Some(page) = PageInkPenGeometry::new(ink, true, 1)
    {
        let color = RenderTheme::for_canvas(dark_mode).foreground(stroke_source_color(stroke));
        page.apply_to_prepared(&mut prepared, color);
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
    output_scale: u32,
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
                let contour = (radius - coverage_offset / f64::from(output_scale)).max(0.0);
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
    let vector = inkpen_v1_vector_path(points, radii, sample_ends, terminal, replay, 1)?;
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
    if let Some(page) = prepare_page_inkpen_geometry(
        stroke,
        options,
        InkPenV1Admission::ConfiguredScaleOnly,
        true,
    ) {
        let mut prepared = crate::ink::prepare_stroke_for_context(
            stroke,
            theme.is_dark(),
            crate::ink::InkRenderContext::CanonicalPage,
        );
        let color = theme.foreground(stroke.color.or(Some(argb_rgb(page.ink.argb))));
        page.apply_to_prepared(&mut prepared, color);
        return prepared;
    }
    let Some(ink) = prepare_inkpen2_v9(stroke, options.inkpen2_profile, theme) else {
        return prepare_stroke_for_page(stroke, theme.is_dark());
    };
    let mut prepared = prepare_stroke_for_page(stroke, theme.is_dark());
    prepared.replace_geometry(crate::ink::PreparedGeometry::InkPen2V9(ink.clone()));
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
        && options
            .inkpen_v1_output_scale
            .is_some_and(|scale| matches!(scale, 1..=8))
        && crate::ink::prepare_inkpen_v1_for_page_with_fixed_width(stroke, true).is_some()
    {
        return crate::ink::prepare_stroke_for_context(
            stroke,
            theme.is_dark(),
            crate::ink::InkRenderContext::CanonicalPage,
        );
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
    prepared.replace_geometry(crate::ink::PreparedGeometry::SampledPolyline {
        points: ink
            .records
            .iter()
            .map(|r| crate::Point {
                x: f64::from(r[0]),
                y: f64::from(r[1]),
            })
            .collect(),
        sample_ends: ink.sample_ends,
    });
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
    let invalid_ordered_replay = match paint.ordered_disk_stamps() {
        Ok(Some(stamps)) => {
            for batch in stamps.batches() {
                for (point, &radius) in batch.points.iter().zip(batch.radii) {
                    svg.push(
                        Circle::new()
                            .replay_part(replay.then_some(ReplayPart(batch.source_sample)))
                            .cx(point.x)
                            .cy(point.y)
                            .r(radius)
                            .fill(Paint::from_hex(color)),
                    );
                }
            }
            return;
        }
        Ok(None) => false,
        // Invalid metadata falls back to the original polyline below. It
        // never emits a partial prefix or a truncated radii zip.
        Err(_) => true,
    };
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
            .or_else(|| crate::ink::saved_legacy_marker_argb(stroke).map(argb_rgb))
            .or_else(|| crate::ink::saved_airbrush_argb(stroke).map(argb_rgb))
            .or_else(|| crate::ink::pencil_source::saved_pencil3_v3_argb(stroke).map(argb_rgb))
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
            None | Some("0;" | "1;" | "2;" | "3;" | "4;")
        )
    {
        return None;
    }
    Some(argb_rgb(rendering.style.color_argb.unwrap_or(0xff00_0000)))
}

/// Recover explicit source RGB for the observed Pencil2 V14, ColoredPencil V4,
/// WaterColorBrush V1/V2 and SimpleBrush null-setting routes. SimpleBrush has
/// one native color control; other colors are inferred from the shader's RGB
/// input. Their native paint is still outside the generic vector renderer; this does not
/// transfer ARGB alpha to whole-stroke opacity.
fn saved_textured_pen_argb(stroke: &Stroke) -> Option<u32> {
    const PENCIL2: &str = "com.samsung.android.sdk.pen.pen.preload.Pencil2";
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
        (Some(PENCIL2), Some("" | "14;"))
            | (Some(COLORED_PENCIL), Some("4;"))
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
mod tests;

mod montblanc_candidate;

#[cfg(test)]
mod montblanc_tests;

#[cfg(all(test, feature = "pdf"))]
mod marker2_guard_tests;
