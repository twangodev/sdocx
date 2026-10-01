//! Presentation-oriented Svg rendering for parsed Samsung Notes documents.

use crate::{
    BoundingBox, Color, Document, LayoutDocument, MediaAsset, Page, PageElement,
    ParagraphAlignment, PlacedImage, PredefinedTextStyle, RichTextBox, RichTextObjectContent,
    RichTextObjectSpan, Stroke, layout_document,
};
use crate::{PageObject, PageObjectContent, composition::RenderPass};
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;
use vector::{
    Anchor, Blend, Circle, Clip, ClipPath, Data, Definitions, Ellipse, FontFamily, Group, Image,
    Line, LineCap, LineJoin, ObjectKind, PageTemplate, Paint, Path, Polygon, Rectangle, ReplayPart,
    ReplayPath, Scene, StrokeIndex, Styled, Svg, TSpan, Text, TextAnchor, TextDecoration,
    Transform, ViewBox, color_hex, coordinate, decimal,
};

mod code;
mod embedded;
pub mod fonts;
mod fountain;
mod marker;
pub use marker::PointMarkerTarget;
mod placed;
mod table;
mod text;
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

/// Options controlling Svg rendering.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct RenderOptions {
    /// Color treatment for page backgrounds, ink, and compatibility text.
    pub color_mode: RenderColorMode,
    /// Native display profile used to size point-list artwork.
    #[cfg_attr(feature = "serde", serde(default))]
    pub point_marker_target: PointMarkerTarget,
}

/// One visible page rendered as a standalone Svg document.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
    options: RenderOptions,
}

struct BodyPreparationContext {
    source: Option<RichTextBox>,
    metrics: BodyPreparationMetrics,
    fonts: Arc<fonts::fontdb::Database>,
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
            options: options.clone(),
        };
        let database = fonts.database();
        let compatible = self.context.as_ref().is_some_and(|context| {
            context.metrics == metrics
                && Arc::ptr_eq(&context.fonts, &database)
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
    pub text_error: Option<String>,
}

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
        let (page, scene) = self.prepare_page(document, layout_page, options, replay, fonts, false);
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
    ) -> Vec<RenderedScene> {
        self.bind(document, options, fonts);
        pages
            .iter()
            .map(|page| {
                let (mut page, mut scene) =
                    self.prepare_page(document, page, options, false, fonts, true);
                let text = scene.take_native_text();
                let text_error = scene.take_native_text_error();
                page.svg = scene.finish();
                RenderedScene {
                    page,
                    text,
                    text_error,
                }
            })
            .collect()
    }

    fn prepare_page(
        &mut self,
        document: &Document,
        layout_page: &crate::LayoutPage,
        options: &RenderOptions,
        replay: bool,
        fonts: &fonts::FontBook,
        retain_text: bool,
    ) -> (RenderedPage, Scene) {
        let page = &layout_page.page;
        let theme = RenderTheme::resolve(page, &document.metadata, options.color_mode);
        let settings = TextSettings::from_document(&document.metadata);
        let text_renderer = TextRenderer::new(settings, fonts)
            .with_point_marker_target(options.point_marker_target)
            .with_page_exclusions(text::PageExclusions::for_page(
                document,
                layout_page.source_page_index,
                settings,
            ));
        let body_text = PreparedBodyText::new(document, layout_page, theme, &text_renderer, self);
        let svg = render_page_contents_svg(
            page,
            &document.metadata,
            &document.metadata.media_assets,
            theme,
            replay,
            &text_renderer,
            body_text.as_ref(),
            retain_text,
        );
        (
            RenderedPage {
                source_page_index: layout_page.source_page_index,
                width: page.width,
                height: page.height,
                svg: String::new(),
                text_diagnostics: text_renderer.diagnostics(),
                object_diagnostics: text_renderer.object_diagnostics(),
            },
            svg,
        )
    }
}

struct PreparedBodyText {
    plan: Rc<PreparedBodyPlan>,
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
}

impl PreparedBodyText {
    fn new(
        document: &Document,
        page: &crate::LayoutPage,
        theme: RenderTheme,
        renderer: &TextRenderer<'_>,
        cache: &mut DocumentTextCache,
    ) -> Option<Self> {
        let body = page.body_text_slice()?;
        let (text, first_page_index, last_page_index) =
            if let Some(window) = body.capture_window.as_ref() {
                (
                    page.body_text_capture(document)?,
                    window.first_page_index,
                    window.requested_page_index,
                )
            } else {
                (
                    page.body_text_reflow(document)?,
                    body.reflow.as_ref()?.first_page_index,
                    document.pages.len().checked_sub(1)?,
                )
            };
        let detached = body.capture_window.as_ref().is_some_and(|window| {
            window.first_page_index != 0
                || document.metadata.note_text.as_ref().is_some_and(|source| {
                    window.saved_source_range != (0..source.text.chars().count())
                })
        });
        let key = BodyPlanKey {
            source_range: body.capture_window.as_ref().map_or_else(
                || body.reflow.as_ref().unwrap().source_range.clone(),
                |window| window.source_range.clone(),
            ),
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
        Some(Self {
            plan,
            object_index: body.object_index,
            page_top,
            viewport: Viewport::new(BoundingBox {
                x_min: 0.0,
                x_max: f64::from(page.page.width),
                y_min: page_top,
                y_max: page_top + f64::from(page.page.height),
            }),
        })
    }

    fn paint(&self, svg: &mut Scene, context: &CompositionContext<'_>) {
        let plan = &self.plan;
        let renderer = context
            .text_renderer
            .clone()
            .with_page_exclusions(plan.exclusions.clone());
        let styled = StyledText::new(&plan.text, TextContext::Flow, renderer.settings);
        let sources = VisibleTextSources::new(&styled, &plan.layout, self.viewport, context.theme);
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
                    paint_text_layout_in_viewport(
                        svg,
                        &styled,
                        &plan.layout,
                        context.media_assets,
                        context.theme,
                        &renderer,
                        Some(self.viewport),
                    );
                },
            );
        });
        renderer.report_text_issues(
            &plan
                .text_issues
                .iter()
                .filter(|issue| {
                    is_layout_issue(&issue.diagnostic)
                        && sources.contains(
                            issue.owner.as_ref(),
                            issue.diagnostic.kind == TextDiagnosticKind::InvalidGeometry,
                        )
                })
                .cloned()
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
        let layout = text::layout_capture_text(
            &styled,
            text::TextFrame {
                bbox: BoundingBox {
                    x_min: padding,
                    y_min: 0.0,
                    x_max: frame_width - padding,
                    y_max: 0.0,
                },
                gravity: None,
                exclusions: &bands,
            },
            key.theme,
            &planner,
        );
        Some(Self {
            text,
            layout,
            text_issues: planner.scoped_diagnostics(),
            object_issues: planner.scoped_object_diagnostics(),
            exclusions,
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
    ) -> Self {
        let mut sources = Self {
            text: Vec::new(),
            layout: Vec::new(),
            objects: Vec::new(),
        };
        for line in &layout.lines {
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
) -> Scene {
    let bg = color_hex(&theme.background());
    let vb_x = 0.0;
    let vb_y = 0.0;
    let vb_w = page.width as f64;
    let vb_h = page.height as f64;
    let svg_w = page.width;
    let svg_h = page.height;

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
    svg.push(
        Rectangle::new()
            .x(vb_x)
            .y(vb_y)
            .width(vb_w)
            .height(vb_h)
            .fill(Paint::from_hex(&bg)),
    );

    if let Ok(Some(pattern)) = crate::page_background::template_pattern(page, metadata) {
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
    };
    render_pass(&mut svg, &composition, RenderPass::Base);
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
            render_pass(svg, &composition, RenderPass::Top);
        });
    }
    render_pass(&mut svg, &composition, RenderPass::Masking);
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
}

fn render_pass(svg: &mut Scene, context: &CompositionContext<'_>, pass: RenderPass) {
    let mut stroke_index = 0;
    for (object_index, object) in context.page.objects.iter().enumerate() {
        if let Some(selected) = object.render_pass() {
            if selected == pass {
                if let Some(body) = context.body_text
                    && body.object_index == object_index
                {
                    body.paint(svg, context);
                } else {
                    render_object(svg, context, object, &mut stroke_index);
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
) {
    match &object.content {
        PageObjectContent::Stroke(stroke) => {
            render_stroke(
                svg,
                stroke,
                context.theme,
                context.replay.then_some(*stroke_index),
            );
            *stroke_index += 1;
        }
        PageObjectContent::Element(element) => render_element(svg, element, context),
        PageObjectContent::Container(children) => {
            for child in children {
                render_object(svg, context, child, stroke_index);
            }
        }
    }
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

fn render_element(svg: &mut Scene, element: &PageElement, context: &CompositionContext<'_>) {
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
            render_shape(svg, shape, theme);
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
        PageElement::Line(line) => render_line(svg, line, theme),
    }
}

// Both shape and line writers serialize the drawing path in page coordinates,
// including rotation. Reject an unsupported path as a whole, never draw a prefix.
fn native_svg_path(bytes: &[u8]) -> Option<Data> {
    use crate::shape::NativePathCommand;

    fn coordinates<const N: usize>(values: [f64; N]) -> Option<[f32; N]> {
        let values = values.map(|value| coordinate(value, 2));
        values
            .iter()
            .all(|value| value.is_finite())
            .then_some(values)
    }

    fn append(data: Data, command: NativePathCommand) -> Option<Data> {
        Some(match command {
            NativePathCommand::Move(point) => {
                let [x, y] = coordinates(point)?;
                data.move_to((x, y))
            }
            NativePathCommand::Line(point) => {
                let [x, y] = coordinates(point)?;
                data.line_to((x, y))
            }
            NativePathCommand::Quadratic([[x1, y1], [x, y]]) => {
                let [x1, y1, x, y] = coordinates([x1, y1, x, y])?;
                data.quadratic_curve_to((x1, y1, x, y))
            }
            NativePathCommand::Cubic([[x1, y1], [x2, y2], [x, y]]) => {
                let [x1, y1, x2, y2, x, y] = coordinates([x1, y1, x2, y2, x, y])?;
                data.cubic_curve_to((x1, y1, x2, y2, x, y))
            }
            NativePathCommand::Close => data.close(),
            NativePathCommand::Arc(_) | NativePathCommand::Oval(_) => data,
        })
    }

    let mut data = Data::new();
    let mut representable = true;
    let parsed = crate::shape::visit_path(bytes, |command| {
        if let Some(next) = append(std::mem::take(&mut data), command) {
            data = next;
        } else {
            representable = false;
        }
    });
    parsed
        .ok()
        .filter(|(size, supported)| representable && *supported && *size == bytes.len())?;
    Some(data)
}

fn render_shape(svg: &mut Scene, shape: &crate::NativeShape, theme: RenderTheme) {
    let (fill, opacity) = shape_paint(&shape.fill, theme);
    let mut style = shape_outline(&shape.style, theme)
        .fill(Paint::from_hex(&fill))
        .fill_opacity(decimal(opacity, 4));
    if !shape.path_data.is_empty() {
        if let Some(path) = native_svg_path(&shape.path_data) {
            svg.push(style.add(Path::new().data(path)));
        }
        return;
    }
    let bbox = shape.geometry_bbox;
    let width = bbox.x_max - bbox.x_min;
    let height = bbox.y_max - bbox.y_min;
    if ![bbox.x_min, bbox.y_min, width, height]
        .iter()
        .all(|v| v.is_finite())
        || width <= 0.0
        || height <= 0.0
        || !shape.rotation_degrees.is_finite()
    {
        return;
    }
    let cx = bbox.x_min + width / 2.0;
    let cy = bbox.y_min + height / 2.0;
    style = style.transformed(Transform::rotate(shape.rotation_degrees.into(), cx, cy, 2));
    let points = match shape.shape_type {
        1 => {
            svg.push(
                style.add(
                    Ellipse::new()
                        .cx(decimal(cx, 2))
                        .cy(decimal(cy, 2))
                        .rx(decimal(width / 2.0, 2))
                        .ry(decimal(height / 2.0, 2)),
                ),
            );
            return;
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
            svg.push(style.add(rectangle(bbox, 0., 2)));
            return;
        }
        8 => vec![
            (cx, bbox.y_min),
            (bbox.x_max, cy),
            (cx, bbox.y_max),
            (bbox.x_min, cy),
        ],
        _ => return,
    };
    svg.push(style.add(Polygon::new().points(&points, 2)));
}

fn render_line(svg: &mut Scene, line: &crate::NativeLine, theme: RenderTheme) {
    // Serialized endpoints already include the native rotation.
    if line.line_type > 2 || line.begin.iter().chain(&line.end).any(|v| !v.is_finite()) {
        return;
    }
    let style = shape_outline(&line.style, theme).fill(Paint::None);
    if !line.path_data.is_empty() {
        if let Some(path) = native_svg_path(&line.path_data) {
            svg.push(style.add(Path::new().data(path)));
        }
    } else if line.line_type == 0 {
        svg.push(
            style.add(
                Line::new()
                    .x1(decimal(line.begin[0], 2))
                    .y1(decimal(line.begin[1], 2))
                    .x2(decimal(line.end[0], 2))
                    .y2(decimal(line.end[1], 2)),
            ),
        );
    }
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

fn shape_outline(style: &crate::ShapeStyle, theme: RenderTheme) -> Group {
    let (paint, opacity) = shape_paint(&style.paint, theme);
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
    Group::new()
        .stroke(Paint::from_hex(&paint))
        .stroke_opacity(decimal(opacity, 4))
        .stroke_width(decimal(width.into(), 2))
        .line_cap(cap)
        .line_join(join)
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
        .x(decimal(bbox.x_min, 2))
        .y(decimal(bbox.y_min, 2))
        .width(decimal(width, 2))
        .height(decimal(height, 2))
        .stretched();
    if let Some(angle) = rotation.filter(|angle| angle.is_finite()) {
        image = image.transformed(Transform::rotate(
            angle,
            bbox.x_min + width / 2.,
            bbox.y_min + height / 2.,
            2,
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
    if let (Some(_), Some(original)) = (image.crop_rect, image.original_bbox) {
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
            Group::new().transformed(Transform::rotate(angle, cx, cy, 4)),
            |svg| {
                svg.scope(
                    Svg::new()
                        .x(decimal(bbox.x_min, 4))
                        .y(decimal(bbox.y_min, 4))
                        .width(decimal(width, 4))
                        .height(decimal(height, 4))
                        .view_box(ViewBox::new(bbox.x_min, bbox.y_min, width, height, 4))
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
                    Some(Viewport::new(frame.background_bounds)),
                );
            });
            paint_text_foreground(svg, &styled, &layout, media_assets, theme, renderer, None);
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
    paint_text_backgrounds(svg, styled, layout, theme, renderer, viewport);
    paint_text_foreground(svg, styled, layout, media_assets, theme, renderer, viewport);
}

fn paint_text_backgrounds(
    svg: &mut Scene,
    styled: &StyledText<'_>,
    layout: &text::TextLayout,
    theme: RenderTheme,
    renderer: &TextRenderer<'_>,
    viewport: Option<Viewport>,
) {
    for line in &layout.lines {
        for issue in text::render_line_backgrounds(svg, styled, line, theme, viewport) {
            renderer
                .for_source(issue.source)
                .report_geometry_issues(&[TextDiagnostic {
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
    viewport: Option<Viewport>,
) {
    svg.text_source(|svg| {
        for line in &layout.lines {
            if let Some(marker) = &line.marker
                && viewport.is_none_or(|viewport| {
                    marker
                        .marker
                        .bounds(marker.x, marker.center_y)
                        .is_some_and(|bounds| viewport.intersects(bounds))
                })
            {
                let style = styled.style_at(marker.source, theme, line.predefined);
                svg.text_marker(marker.source, |svg| {
                    paint_positioned_marker(svg, marker, &style, theme, renderer);
                });
            }
            if viewport.is_none_or(|viewport| viewport.text_visible(styled, line, theme)) {
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
            }
            paint_line_objects(
                svg,
                styled,
                &line.line,
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
        }
    });
}

#[allow(clippy::too_many_arguments)]
fn paint_line_objects(
    svg: &mut Scene,
    styled: &StyledText<'_>,
    line: &text::WrappedLine,
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
                        table::prepare_table_clone_drawing(
                            table,
                            span.layout_constraint,
                            target,
                            theme,
                            &drawing_renderer,
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
            if viewport.is_some_and(|viewport| !viewport.intersects(paint_bounds)) {
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
                                    renderer,
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
                        &renderer.for_object_source(object.source.clone()),
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
        viewport.map(|viewport| VisibleTextSources::new(&styled, &layout, viewport, theme));
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
            .filter(|issue| {
                is_layout_issue(&issue.diagnostic)
                    && sources.as_ref().is_none_or(|sources| {
                        sources.contains(
                            issue.owner.as_ref(),
                            issue.diagnostic.kind == TextDiagnosticKind::InvalidGeometry,
                        )
                    })
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

fn is_layout_issue(issue: &TextDiagnostic) -> bool {
    matches!(
        issue.kind,
        TextDiagnosticKind::MeasurementFailure | TextDiagnosticKind::InvalidGeometry
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
            .filter(|issue| is_layout_issue(&issue.diagnostic))
            .collect::<Vec<_>>(),
    );
    renderer.report_owned_object_issues(&drawing.scoped_object_diagnostics());
}

struct TablePaint<'a> {
    measured_bbox: BoundingBox,
    rows: &'a [table::PreparedTableRow],
    pending_gaps: &'a [f64],
}

impl<'a> From<&'a table::PreparedTable> for TablePaint<'a> {
    fn from(table: &'a table::PreparedTable) -> Self {
        Self {
            measured_bbox: table.measured_bbox,
            rows: &table.rows,
            pending_gaps: &table.pending_gaps,
        }
    }
}

impl<'a> From<&'a table::PreparedTableDrawing> for TablePaint<'a> {
    fn from(table: &'a table::PreparedTableDrawing) -> Self {
        Self {
            measured_bbox: table.measured_bbox,
            rows: &table.rows,
            pending_gaps: &table.pending_gaps,
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
    let grid = table::TableGrid::new(table);
    if prepared.is_none()
        && grid
            .as_ref()
            .map_or(true, |grid| grid.requires_merged_layout())
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
        .and_then(|prepared| {
            let first = prepared.rows.first()?.cells.first()?.frame;
            let last = prepared.rows.last()?.cells.last()?.frame;
            Some(BoundingBox {
                x_max: last.x_max,
                y_max: last.y_max,
                ..first
            })
        })
        .unwrap_or(table.style.content_bbox.unwrap_or(table.bbox));
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
        cells.push(TableCellPaint {
            cell,
            fill: table::CellFill::resolve(&table.style, table.rows[row].index, cell, theme),
            frame: measured.map_or(cell.bbox, |cell| cell.frame),
            layout: measured.map(|cell| &cell.layout),
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
    cells.retain(|paint| {
        viewport.is_none_or(|viewport| {
            viewport.intersects(BoundingBox {
                y_min: paint.frame.y_min + offset_y,
                y_max: paint.frame.y_max + offset_y,
                ..paint.frame
            })
        })
    });
    svg.scope(Group::new().object(ObjectKind::Table), |svg| {
        let clip = svg.definition::<Clip>();
        svg.push(
            Definitions::new().add(ClipPath::new(&clip).add(rectangle(table_bbox, offset_y, 2))),
        );
        svg.scope(Group::new().clipped(&clip), |svg| {
            let shape = [table.rows.len(), table.column_widths.len()];
            for paint in &cells {
                paint_table_cell_background(svg, paint, shape, [outline.rx, outline.ry], offset_y);
                if let Some(borders) = &borders {
                    for path in borders.cell_paths(paint.position) {
                        if path.selected(paint.position, shape, outline.active(), paint.gap) {
                            let path = if prepared.is_some() {
                                path.on_frame(paint.frame)
                            } else {
                                path
                            };
                            paint_table_border(svg, path, offset_y, theme);
                        }
                    }
                }
            }
        });
        if outline.active() {
            if outline.rounded() {
                svg.push(
                    rectangle(content_bbox, offset_y, 5)
                        .rx(decimal(f64::from(outline.rx), 5))
                        .ry(decimal(f64::from(outline.ry), 5))
                        .fill(Paint::None)
                        .stroke(Paint::from_hex(&color_hex(
                            &theme.foreground_color(argb_color(outline.color)),
                        )))
                        .stroke_opacity(decimal(f64::from(outline.color >> 24) / 255.0, 6))
                        .stroke_width(decimal(f64::from(outline.width), 5)),
                );
            } else if let Some(borders) = &borders {
                for path in borders.outer_paths(Some(content_bbox)) {
                    paint_table_border(svg, path, offset_y, theme);
                }
            }
        }
        svg.scope(Group::new().clipped(&clip), |svg| {
            for paint in &cells {
                let cell_theme = paint.fill.theme(theme);
                if let Some(layout) = paint.layout {
                    let styled =
                        StyledText::new(&paint.cell.content, TextContext::Flow, renderer.settings);
                    paint_text_layout_in_viewport(
                        svg,
                        &styled,
                        layout,
                        media_assets,
                        cell_theme,
                        renderer,
                        viewport,
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
                    render_text_frame(
                        svg,
                        &paint.cell.content,
                        frame,
                        media_assets,
                        cell_theme,
                        renderer,
                        viewport,
                    );
                }
            }
        });
    });
}

struct TableCellPaint<'a> {
    cell: &'a crate::RichTextTableCell,
    fill: table::CellFill,
    frame: BoundingBox,
    layout: Option<&'a text::TextLayout>,
    position: table::CellPosition,
    gap: bool,
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
    let [rx, ry] = radii;
    let first_row = paint.position.row == 0;
    let first_column = paint.position.column == 0;
    let last_row = paint.position.row.checked_add(paint.cell.row_span as usize) == Some(shape[0]);
    let last_column = paint
        .position
        .column
        .checked_add(paint.cell.column_span as usize)
        == Some(shape[1]);
    let corners = [
        first_row && first_column,
        first_row && last_column,
        last_row && last_column,
        last_row && first_column,
    ];
    if rx <= 0.0 || ry <= 0.0 || !corners.into_iter().any(|corner| corner) {
        svg.push(rectangle(paint.frame, offset_y, 5).fill(fill));
        return;
    }
    svg.push(
        rectangle(paint.frame, offset_y, 5)
            .rx(decimal(f64::from(rx), 5))
            .ry(decimal(f64::from(ry), 5))
            .fill(fill),
    );
    let BoundingBox {
        x_min,
        y_min,
        x_max,
        y_max,
    } = paint.frame;
    let [left, top, right, bottom] = [x_min, y_min, x_max, y_max].map(|value| value as f32);
    for (rounded, [x_min, y_min, x_max, y_max]) in corners.into_iter().zip([
        [left, top, left + rx, top + ry],
        [right - rx, top, right, top + ry],
        [right - rx, bottom - ry, right, bottom],
        [left, bottom - ry, left + rx, bottom],
    ]) {
        if !rounded {
            let [x_min, y_min, x_max, y_max] = [x_min, y_min, x_max, y_max].map(f64::from);
            svg.push(
                rectangle(
                    BoundingBox {
                        x_min,
                        y_min,
                        x_max,
                        y_max,
                    },
                    offset_y,
                    5,
                )
                .fill(fill),
            );
        }
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
            let styled = StyledText::new(title, TextContext::Flow, settings);
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
            let styled = StyledText::new(body, TextContext::Flow, settings);
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

fn render_stroke(
    svg: &mut Scene,
    stroke: &Stroke,
    theme: RenderTheme,
    replay_index: Option<usize>,
) {
    let mut paint = crate::prepare_stroke(stroke, false);
    paint.color = theme.foreground(stroke.color);
    if let Some(index) = replay_index {
        svg.scope(Group::new().replay_stroke(StrokeIndex(index)), |svg| {
            render_prepared_stroke(svg, &paint, true);
        });
    } else {
        render_prepared_stroke(svg, &paint, false);
    }
}

fn render_prepared_stroke(svg: &mut Scene, paint: &crate::PreparedStroke<'_>, replay: bool) {
    let color = &paint.color;
    let base_width = paint.width;
    if fountain::render(svg, paint, replay) {
        return;
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
    if let Some(radii) = &paint.dot_radii {
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

/// Resolve paint for a default light or dark canvas, without document metadata.
pub fn stroke_paint(stroke: &Stroke, dark_mode: bool) -> StrokePaint {
    let width = normalized_stroke_width(stroke.pen_width);
    let pressure = stroke.pressures.len() >= stroke.points.len().saturating_sub(1)
        && stroke
            .pressures
            .iter()
            .any(|&p| p > PRESSURE_PRESENT_EPSILON);
    StrokePaint {
        color: RenderTheme::for_canvas(dark_mode).foreground(stroke.color),
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
        RenderColorMode, RenderOptions, is_dark_background, normalized_stroke_width,
        render_document_svg, render_layout_page_svg, sanitize_hyperlink_target,
    };
    use crate::{
        BoundingBox, Color, Document, DocumentMetadata, Page, PageElement, Point, RichTextBox,
        RichTextSpan, RichTextSpanType, Stroke, StrokeProperties, StrokeRendering, StrokeStyle,
        layout_document,
    };

    #[test]
    fn native_paths_reject_unrepresentable_coordinates_without_drawing_a_prefix() {
        let mut bytes = 2_u32.to_le_bytes().to_vec();
        for (verb, x, y) in [(1_u8, 10_f64, 20_f64), (2, f64::MAX, 30.)] {
            bytes.push(verb);
            bytes.extend(x.to_le_bytes());
            bytes.extend(y.to_le_bytes());
        }
        assert!(super::native_svg_path(&bytes).is_none());
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
    #[test]
    fn fountain_svg_keeps_reconstructed_ink_as_vector_paths() {
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
            assert!(rendered[0].svg.contains("<path "));
            assert!(!rendered[0].svg.contains("<image"));
            assert!(!rendered[0].svg.contains("<filter"));
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
            assert!(!replay.svg.contains("<filter"));
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
            let scenes = cache.render_layout_page_scenes(&document, &pages, &options, &fonts);
            assert!(scenes.iter().all(|scene| scene.text_error.is_none()));
            assert!(super::Rc::ptr_eq(&plan, &cache.plans[0].1));
        }
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
                    ..StrokeStyle::default()
                },
            }),
            bbox: BoundingBox::default(),
            points: vec![Point { x, y: 20. }],
            pressures: vec![0.4],
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
        cyan.points = vec![Point { x: 16., y: 16. }];
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
