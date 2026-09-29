//! Presentation-oriented Svg rendering for parsed Samsung Notes documents.

use crate::{
    BoundingBox, BulletType, Color, Document, HyperlinkType, LayoutDocument, LineSpacingType,
    MediaAsset, Page, PageElement, ParagraphAlignment, ParagraphBullet, ParagraphLineSpacing,
    PlacedImage, PredefinedTextStyle, RichTextBox, RichTextObjectContent, RichTextObjectSpan,
    RichTextParagraphType, RichTextRun, RichTextSpanType, Stroke, layout_document,
};
use std::ops::Range;
use vector::{
    Anchor, Blend, Circle, Clip, ClipPath, Data, Definitions, Ellipse, FontFamily, Group, Image,
    Line, LineCap, LineJoin, ObjectKind, PageTemplate, Paint, Path, Polygon, Rectangle, ReplayPart,
    ReplayPath, Scene, StrokeIndex, Styled, Svg, TSpan, Text, TextAnchor, TextDecoration,
    Transform, ViewBox, color_hex, coordinate, decimal,
};

mod fountain;
mod theme;
pub use theme::RenderTheme;
#[cfg(test)]
use theme::is_dark_background;
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
}

/// Render every visible page in presentation order.
pub fn render_document_svg(document: &Document, options: &RenderOptions) -> Vec<RenderedPage> {
    let layout = layout_document(document);
    layout
        .pages
        .iter()
        .map(|layout_page| render_layout_page(document, layout_page, options, false))
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
    layout
        .pages
        .get(page_index)
        .map(|layout_page| render_layout_page(document, layout_page, options, false))
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
    layout
        .pages
        .get(page_index)
        .map(|page| render_layout_page(document, page, options, true))
}

fn render_layout_page(
    document: &Document,
    layout_page: &crate::LayoutPage,
    options: &RenderOptions,
    replay: bool,
) -> RenderedPage {
    let page = &layout_page.page;
    let theme = RenderTheme::resolve(page, &document.metadata, options.color_mode);
    let svg = render_page_contents_svg(
        page,
        &document.metadata,
        &document.metadata.media_assets,
        document.metadata.flow_page_padding,
        theme,
        replay,
    );
    RenderedPage {
        source_page_index: layout_page.source_page_index,
        width: page.width,
        height: page.height,
        svg,
    }
}

// Default ink for uncolored strokes, by canvas: light on dark, dark on light.
const DEFAULT_INK_DARK_MODE: &str = "#ffffff";
const DEFAULT_INK_LIGHT_MODE: &str = "#1a1a1a";
// Pressure channel on v4.4.x files can be present but all-zero; treat as absent.
const PRESSURE_PRESENT_EPSILON: f64 = 0.01;

fn render_page_contents_svg(
    page: &Page,
    metadata: &crate::DocumentMetadata,
    media_assets: &[MediaAsset],
    flow_page_padding: Option<(u32, u32)>,
    theme: RenderTheme,
    replay: bool,
) -> String {
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

    let mut highlighter = Vec::new();
    for (index, stroke) in page.strokes().enumerate() {
        if stroke
            .rendering
            .as_ref()
            .is_some_and(|rendering| rendering.properties.top_layer_pen)
        {
            highlighter.push((index, stroke));
        } else {
            render_stroke(&mut svg, stroke, theme, replay.then_some(index));
        }
    }
    for element in page.elements() {
        render_element(
            &mut svg,
            element,
            page,
            media_assets,
            flow_page_padding,
            theme,
        );
    }
    if !highlighter.is_empty() {
        let blend = if theme.is_dark() {
            Blend::Lighten
        } else {
            Blend::Darken
        };
        svg.scope(Group::new().blend(blend), |svg| {
            for (index, stroke) in highlighter {
                render_stroke(svg, stroke, theme, replay.then_some(index));
            }
        });
    }
    svg.finish()
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
    page: &Page,
    media_assets: &[MediaAsset],
    flow_page_padding: Option<(u32, u32)>,
    theme: RenderTheme,
) {
    match element {
        PageElement::Image { bbox, media_index } => {
            render_image(svg, *bbox, Some(*media_index), None, media_assets);
        }
        PageElement::PlacedImage(image) => render_placed_image(svg, image, media_assets),
        PageElement::TextBox(text_box) => {
            render_text_box(svg, text_box, page, media_assets, flow_page_padding, theme)
        }
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
                render_text_box(svg, text, page, media_assets, flow_page_padding, theme);
            }
        }
        PageElement::Line(line) => render_line(svg, line, theme),
    }
}

// Both shape and line writers serialize the drawing path in page coordinates,
// including rotation. Reject an unsupported path as a whole, never draw a prefix.
fn native_svg_path(bytes: &[u8]) -> Option<Data> {
    let mut data = Data::new();
    let mut representable = true;
    let parsed = crate::shape::visit_path(bytes, |verb, values| {
        let v: Vec<f32> = values.iter().map(|&value| coordinate(value, 2)).collect();
        if v.iter().any(|value| !value.is_finite()) {
            representable = false;
            return;
        }
        data = match (verb, v.as_slice()) {
            (1, &[x, y]) => std::mem::take(&mut data).move_to((x, y)),
            (2, &[x, y]) => std::mem::take(&mut data).line_to((x, y)),
            (3, &[x1, y1, x, y]) => std::mem::take(&mut data).quadratic_curve_to((x1, y1, x, y)),
            (4, &[x1, y1, x2, y2, x, y]) => {
                std::mem::take(&mut data).cubic_curve_to((x1, y1, x2, y2, x, y))
            }
            (6, &[]) => std::mem::take(&mut data).close(),
            _ => return,
        };
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
) {
    let text = text_box.text.trim_end_matches('\n');
    if text.trim().is_empty() {
        return;
    }

    let is_note_body =
        text_box.bbox.x_max <= text_box.bbox.x_min || text_box.bbox.y_max <= text_box.bbox.y_min;
    if is_note_body {
        render_flow_text_box(svg, text_box, page, media_assets, flow_page_padding, theme);
        return;
    }
    let (x, y, width, height) = (
        text_box.bbox.x_min,
        text_box.bbox.y_min,
        text_box.bbox.x_max - text_box.bbox.x_min,
        text_box.bbox.y_max - text_box.bbox.y_min,
    );
    let theme = text_box
        .highlight_color
        .map_or(theme, |color| theme.on_background(color));
    let color = theme.foreground(text_box.color);
    let font_size = text_box.font_size.map(samsung_font_to_svg).unwrap_or(37.0);
    let line_height = font_size * 1.35;
    let mut group = Group::new();
    if let Some(rotation) = text_box.rotation_degrees {
        let cx = x + width / 2.0;
        let cy = y + height / 2.0;
        group = group.transformed(Transform::rotate(rotation, cx, cy, 2));
    }
    svg.scope(group, |svg| {
        if let Some(highlight) = text_box.highlight_color.as_ref() {
            svg.push(rectangle(text_box.bbox, 0., 2).fill(Paint::from_hex(&color_hex(highlight))));
        }
        for (line_idx, line) in text.lines().enumerate() {
            if line.is_empty() {
                continue;
            }
            let text_y = y + font_size + line_idx as f64 * line_height;
            let line_start = text
                .lines()
                .take(line_idx)
                .map(|line| line.chars().count() + 1)
                .sum::<usize>();
            let spans = styled_line_spans(line, line_start, &text_box.runs);
            let mut node = Text::new("")
                .x(decimal(x, 2))
                .y(decimal(text_y, 2))
                .fill(Paint::from_hex(&color))
                .family(FontFamily::Arial)
                .font_size(decimal(font_size, 2));
            if text_box.underline {
                node = node.decoration(TextDecoration::Underline);
            }
            svg.scope(node, |svg| {
                for span in spans {
                    let mut node = TSpan::new(span.text);
                    if span.bold {
                        node = node.bold();
                    }
                    if span.italic {
                        node = node.italic();
                    }
                    svg.push(node);
                }
            });
        }
        for span in &text_box.object_spans {
            if let Some(RichTextObjectContent::Image(image)) = &span.content {
                render_placed_image(svg, image, media_assets);
            }
        }
    });
}

const SAMSUNG_TEXT_SCALE: f64 = 3.0;
const IMAGE_FLOW_LINE_HEIGHT_RATIO: f64 = 1.35;
const FLOW_HORIZONTAL_PADDING: f64 = 48.0;
const FLOW_INDENT: f64 = 48.0;

#[derive(Clone)]
struct SvgTextStyle {
    font_size: f64,
    color: String,
    bold: bool,
    italic: bool,
    underline: bool,
    strikethrough: bool,
    link_target: Option<String>,
}

#[derive(Default)]
struct ParagraphLayout {
    alignment: Option<ParagraphAlignment>,
    indent_level: u32,
    line_spacing: Option<ParagraphLineSpacing>,
    bullet: Option<ParagraphBullet>,
    spacing_before: f64,
    spacing_after: f64,
    predefined_style: Option<PredefinedTextStyle>,
}

fn render_flow_text_box(
    svg: &mut Scene,
    text_box: &RichTextBox,
    page: &Page,
    media_assets: &[MediaAsset],
    flow_page_padding: Option<(u32, u32)>,
    theme: RenderTheme,
) {
    let (horizontal_padding, vertical_padding) = flow_page_padding
        .map(|(horizontal, vertical)| (f64::from(horizontal), f64::from(vertical)))
        .unwrap_or((FLOW_HORIZONTAL_PADDING, 0.0));
    let margins = text_box.margins.unwrap_or([0.0; 4]);
    let content_left = horizontal_padding + f64::from(margins[0]) * SAMSUNG_TEXT_SCALE;
    let image_flow = text_box.is_image_flow();
    let content_top = f64::from(margins[1]) * SAMSUNG_TEXT_SCALE
        + if image_flow { 0.0 } else { vertical_padding };
    let content_right =
        f64::from(page.width) - horizontal_padding - f64::from(margins[2]) * SAMSUNG_TEXT_SCALE;
    let characters = text_box.text.chars().collect::<Vec<_>>();
    let utf16_offsets = char_utf16_offsets(&text_box.text);
    let byte_offsets = char_byte_offsets(&text_box.text);
    let mut paragraph_start = 0_usize;
    let mut cursor_y = content_top;

    let paragraphs = text_box.text.split_inclusive('\n').collect::<Vec<_>>();
    svg.scope(Group::new().flow(), |svg| {
        for (paragraph_index, paragraph) in paragraphs.iter().enumerate() {
            let content = paragraph.trim_end_matches(['\n', '\r']);
            let content_length = content.chars().count();
            let paragraph_end = paragraph_start + content_length;
            let layout = paragraph_layout(text_box, paragraph_index as u32);
            let previous_is_list_item = paragraph_index > 0
                && paragraph_layout(text_box, paragraph_index as u32 - 1)
                    .bullet
                    .and_then(bullet_marker)
                    .is_some();
            let current_is_list_item = layout.bullet.and_then(bullet_marker).is_some();
            let next_is_list_item = paragraph_index + 1 < paragraphs.len()
                && paragraph_layout(text_box, paragraph_index as u32 + 1)
                    .bullet
                    .and_then(bullet_marker)
                    .is_some();
            if paragraph_start != 0 && !(previous_is_list_item && current_is_list_item) {
                cursor_y += layout.spacing_before;
            }

            let paragraph_start_utf16 = utf16_offsets[paragraph_start];
            let paragraph_end_utf16 = utf16_offsets[paragraph_end];
            let base_style = text_style_at(
                text_box,
                paragraph_start_utf16,
                theme,
                layout.predefined_style,
            );
            let embedded = text_box
                .object_spans
                .iter()
                .filter(|object| {
                    u32::try_from(object.text_index_utf16).is_ok_and(|index| {
                        index >= paragraph_start_utf16 && index <= paragraph_end_utf16
                    })
                })
                .collect::<Vec<_>>();
            if !embedded.is_empty() {
                for object in embedded {
                    if let Some(bottom) =
                        render_embedded_object(svg, object, cursor_y, media_assets, theme)
                    {
                        let bottom_margin =
                            if matches!(object.content, Some(RichTextObjectContent::Image(_))) {
                                base_style.font_size * (IMAGE_FLOW_LINE_HEIGHT_RATIO - 1.0)
                            } else {
                                object_bottom_margin(object)
                            };
                        cursor_y = cursor_y.max(bottom + bottom_margin);
                    }
                }
                cursor_y += layout.spacing_after;
                paragraph_start += paragraph.chars().count();
                continue;
            }

            let marker = layout
                .bullet
                .and_then(|bullet| bullet_marker_for_indent(bullet, layout.indent_level));
            let marker_width = marker.as_ref().map_or(0.0, |(_, width, _, _)| *width);
            let base_x = content_left + f64::from(layout.indent_level) * FLOW_INDENT;
            let text_x = base_x + marker_width;
            let available_width = (content_right - text_x).max(base_style.font_size);
            let lines = if content.is_empty() {
                std::iter::once(paragraph_start..paragraph_start).collect::<Vec<_>>()
            } else {
                wrap_paragraph(
                    text_box,
                    &characters,
                    &utf16_offsets,
                    paragraph_start..paragraph_end,
                    available_width,
                    theme,
                    layout.predefined_style,
                )
            };
            let line_height = if image_flow && layout.line_spacing.is_none() {
                base_style.font_size * IMAGE_FLOW_LINE_HEIGHT_RATIO
            } else {
                paragraph_line_height(base_style.font_size, layout.line_spacing)
            };

            for (line_index, line_range) in lines.iter().enumerate() {
                let baseline = cursor_y + base_style.font_size;
                if line_index == 0
                    && let Some((marker, _, marker_size, marker_offset)) = marker.as_ref()
                {
                    svg.scope(
                        Text::new("")
                            .x(decimal(base_x + marker_offset, 2))
                            .y(decimal(
                                baseline - if *marker_size < 40.0 { 8.0 } else { 0.0 },
                                2,
                            ))
                            .fill(Paint::from_hex(&base_style.color))
                            .family(FontFamily::Roboto)
                            .font_size(decimal(*marker_size, 2)),
                        |svg| {
                            svg.push(TSpan::new(marker));
                        },
                    );
                }
                render_flow_line(
                    svg,
                    text_box,
                    &utf16_offsets,
                    &byte_offsets,
                    line_range.clone(),
                    text_x,
                    content_right,
                    baseline,
                    layout.alignment,
                    theme,
                    layout.predefined_style,
                );
                cursor_y += line_height;
            }
            if !(current_is_list_item && next_is_list_item) {
                cursor_y += layout.spacing_after;
            }
            paragraph_start += paragraph.chars().count();
        }
    });
}

fn paragraph_layout(text_box: &RichTextBox, paragraph_index: u32) -> ParagraphLayout {
    let mut layout = ParagraphLayout::default();
    for paragraph in text_box.paragraphs.iter().filter(|paragraph| {
        paragraph.start_paragraph <= paragraph_index && paragraph.end_paragraph > paragraph_index
    }) {
        match paragraph.kind {
            RichTextParagraphType::Alignment => layout.alignment = paragraph.alignment(),
            RichTextParagraphType::IndentLevel => {
                if let Some(indent) = paragraph.indent() {
                    layout.indent_level = indent.level;
                }
            }
            RichTextParagraphType::LineSpacing => layout.line_spacing = paragraph.line_spacing(),
            RichTextParagraphType::Bullet => layout.bullet = paragraph.bullet(),
            RichTextParagraphType::SpacingBefore => {
                layout.spacing_before = paragraph
                    .spacing()
                    .filter(|spacing| spacing.is_finite() && *spacing > 0.0)
                    .map_or(0.0, |spacing| f64::from(spacing) * SAMSUNG_TEXT_SCALE);
            }
            RichTextParagraphType::SpacingAfter => {
                layout.spacing_after = paragraph
                    .spacing()
                    .filter(|spacing| spacing.is_finite() && *spacing > 0.0)
                    .map_or(0.0, |spacing| f64::from(spacing) * SAMSUNG_TEXT_SCALE);
            }
            RichTextParagraphType::PredefinedStyle => {
                layout.predefined_style = paragraph.predefined_style().map(|style| style.style)
            }
            _ => {}
        }
    }
    layout
}

fn paragraph_line_height(font_size: f64, spacing: Option<ParagraphLineSpacing>) -> f64 {
    match spacing {
        Some(spacing)
            if spacing.value.is_finite()
                && spacing.value > 0.0
                && spacing.kind == LineSpacingType::Percent =>
        {
            font_size * f64::from(spacing.value)
        }
        Some(spacing)
            if spacing.value.is_finite()
                && spacing.value > 0.0
                && spacing.kind == LineSpacingType::Pixels =>
        {
            f64::from(spacing.value) * SAMSUNG_TEXT_SCALE
        }
        _ => font_size * 1.6,
    }
}

fn bullet_marker(bullet: ParagraphBullet) -> Option<(String, f64, f64, f64)> {
    let marker_kind = bullet.kind;
    let marker = match marker_kind {
        BulletType::None => return None,
        BulletType::Arrow => "➤".to_string(),
        BulletType::Checker => {
            if bullet.checked {
                "☑".to_string()
            } else {
                "☐".to_string()
            }
        }
        BulletType::Diamond => "◆".to_string(),
        BulletType::Digit => format!("{}.", bullet.number),
        BulletType::CircledDigit => format!("{}", bullet.number),
        BulletType::Alphabet => alphabetic_marker(bullet.number, false),
        BulletType::RomanNumeral => roman_marker(bullet.number),
        BulletType::SolidCircle => "●".to_string(),
        BulletType::WhiteCircle => "○".to_string(),
        BulletType::UppercaseAlphabet => alphabetic_marker(bullet.number, true),
        BulletType::BlackSquare => "■".to_string(),
        BulletType::WhiteSquare => "□".to_string(),
        _ => "•".to_string(),
    };
    let (width, font_size, offset) = match marker_kind {
        BulletType::Digit => (64.0, 45.0, 0.0),
        BulletType::SolidCircle => (48.0, 24.0, 20.0),
        BulletType::WhiteCircle => (78.0, 27.0, 20.0),
        _ => (78.0, 32.0, 12.0),
    };
    Some((marker, width, font_size, offset))
}

fn bullet_marker_for_indent(
    mut bullet: ParagraphBullet,
    indent_level: u32,
) -> Option<(String, f64, f64, f64)> {
    if bullet.kind == BulletType::SolidCircle && indent_level % 2 == 1 {
        bullet.kind = BulletType::WhiteCircle;
    }
    bullet_marker(bullet)
}

fn alphabetic_marker(number: u32, uppercase: bool) -> String {
    let offset = number.saturating_sub(1) % 26;
    let base = if uppercase { b'A' } else { b'a' };
    format!("{}.", char::from(base + offset as u8))
}

fn roman_marker(number: u32) -> String {
    const VALUES: &[(u32, &str)] = &[
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut remaining = number.max(1);
    let mut result = String::new();
    for (value, numeral) in VALUES {
        while remaining >= *value {
            result.push_str(numeral);
            remaining -= value;
        }
    }
    result.make_ascii_lowercase();
    result.push('.');
    result
}

fn wrap_paragraph(
    text_box: &RichTextBox,
    characters: &[char],
    utf16_offsets: &[u32],
    range: Range<usize>,
    max_width: f64,
    theme: RenderTheme,
    predefined_style: Option<PredefinedTextStyle>,
) -> Vec<Range<usize>> {
    let mut lines = Vec::new();
    let mut start = range.start;
    while start < range.end {
        let mut width = 0.0;
        let mut index = start;
        let mut last_break = None;
        while index < range.end {
            let character = characters[index];
            let style = text_style_at(text_box, utf16_offsets[index], theme, predefined_style);
            let next_width = width + estimated_character_width(character, &style);
            if next_width > max_width && index > start {
                break;
            }
            width = next_width;
            index += 1;
            if character.is_whitespace() {
                last_break = Some((index - 1, index));
            } else if matches!(character, '/' | '?' | '&' | '#' | '-' | '.')
                && text_box.spans.iter().any(|span| {
                    span.kind == RichTextSpanType::Hyperlink
                        && span.start_utf16 <= utf16_offsets[index - 1]
                        && span.end_utf16 > utf16_offsets[index - 1]
                })
            {
                // Samsung's URL line breaker keeps a link with its prefix and
                // prefers URL punctuation over an arbitrary character split.
                last_break = Some((index, index));
            }
        }
        if index == range.end {
            lines.push(start..range.end);
            break;
        }
        let (end, next) = last_break
            .filter(|(end, _)| *end > start)
            .unwrap_or((index, index));
        lines.push(start..end);
        start = next;
        while start < range.end && characters[start].is_whitespace() {
            start += 1;
        }
    }
    lines
}

fn estimated_character_width(character: char, style: &SvgTextStyle) -> f64 {
    // The analyzed Samsung PDF exporter embeds Roboto-Regular with these
    // advances. Printable ASCII glyph IDs are codepoint - 27 in that font.
    const ROBOTO_ADVANCES: [u16; 100] = [
        443, 0, 0, 248, 248, 248, 257, 320, 615, 562, 732, 622, 174, 342, 348, 430, 567, 196, 276,
        263, 412, 562, 562, 562, 562, 562, 562, 562, 562, 562, 562, 242, 211, 508, 548, 522, 472,
        897, 652, 623, 650, 656, 568, 552, 681, 713, 271, 551, 627, 538, 873, 713, 687, 630, 687,
        616, 593, 596, 648, 636, 887, 626, 600, 599, 265, 410, 265, 417, 451, 309, 543, 561, 523,
        563, 530, 347, 561, 550, 243, 239, 506, 243, 876, 552, 570, 561, 568, 338, 516, 327, 551,
        484, 751, 496, 473, 496, 338, 244, 338, 680,
    ];
    let latin_base = match character {
        'À'..='Å' => Some('A'),
        'Ç' => Some('C'),
        'È'..='Ë' => Some('E'),
        'Ì'..='Ï' => Some('I'),
        'Ñ' => Some('N'),
        'Ò'..='Ö' => Some('O'),
        'Ù'..='Ü' => Some('U'),
        'Ý' => Some('Y'),
        'à'..='å' => Some('a'),
        'ç' => Some('c'),
        'è'..='ë' => Some('e'),
        'ì'..='ï' => Some('i'),
        'ñ' => Some('n'),
        'ò'..='ö' => Some('o'),
        'ù'..='ü' => Some('u'),
        'ý' | 'ÿ' => Some('y'),
        _ => None,
    };
    let metric_character = latin_base.unwrap_or(character);
    let factor = if (' '..='~').contains(&metric_character) {
        let glyph = metric_character as usize - 27;
        f64::from(ROBOTO_ADVANCES[glyph]) / 1000.0
    } else if character.is_whitespace() {
        0.248
    } else if character == 'ß' || ('\u{0370}'..='\u{052f}').contains(&character) {
        0.62
    } else if ('\u{2e80}'..='\u{d7af}').contains(&character) {
        1.0
    } else {
        0.65
    };
    style.font_size * factor
}

#[allow(clippy::too_many_arguments)]
fn render_flow_line(
    svg: &mut Scene,
    text_box: &RichTextBox,
    utf16_offsets: &[u32],
    byte_offsets: &[usize],
    range: Range<usize>,
    left: f64,
    right: f64,
    baseline: f64,
    alignment: Option<ParagraphAlignment>,
    theme: RenderTheme,
    predefined_style: Option<PredefinedTextStyle>,
) {
    if range.is_empty() {
        return;
    }
    let (x, anchor) = match alignment {
        Some(ParagraphAlignment::Center) => ((left + right) / 2.0, TextAnchor::Middle),
        Some(ParagraphAlignment::Right) => (right, TextAnchor::End),
        _ => (left, TextAnchor::Start),
    };
    let mut boundaries = vec![range.start, range.end];
    for span in &text_box.spans {
        if let Some(start) = utf16_to_char_index(&text_box.text, span.start_utf16)
            && start > range.start
            && start < range.end
        {
            boundaries.push(start);
        }
        if let Some(end) = utf16_to_char_index(&text_box.text, span.end_utf16)
            && end > range.start
            && end < range.end
        {
            boundaries.push(end);
        }
    }
    boundaries.sort_unstable();
    boundaries.dedup();

    svg.scope(
        Text::new("")
            .x(decimal(x, 2))
            .y(decimal(baseline, 2))
            .anchor(anchor)
            .family(FontFamily::Roboto)
            .preserve_space(),
        |svg| {
            for segment in boundaries.windows(2) {
                let start = segment[0];
                let end = segment[1];
                let style = text_style_at(text_box, utf16_offsets[start], theme, predefined_style);
                write_styled_tspan(
                    svg,
                    &text_box.text[byte_offsets[start]..byte_offsets[end]],
                    &style,
                );
            }
        },
    );
}

fn text_style_at(
    text_box: &RichTextBox,
    utf16_index: u32,
    theme: RenderTheme,
    predefined_style: Option<PredefinedTextStyle>,
) -> SvgTextStyle {
    let mut font_size = text_box.font_size.map(samsung_font_to_svg).unwrap_or(45.0);
    if let Some(style) = predefined_style {
        font_size = match style {
            PredefinedTextStyle::Heading1 => 63.0,
            PredefinedTextStyle::Heading2 => 57.0,
            PredefinedTextStyle::Heading3 => 51.0,
            PredefinedTextStyle::Body1 | PredefinedTextStyle::Other(_) => font_size,
        };
    }
    let mut style = SvgTextStyle {
        font_size,
        color: theme.foreground(text_box.color),
        bold: false,
        italic: false,
        underline: false,
        strikethrough: false,
        link_target: None,
    };
    let mut is_hyperlink = false;
    for span in text_box
        .spans
        .iter()
        .filter(|span| span.start_utf16 <= utf16_index && span.end_utf16 > utf16_index)
    {
        match span.kind {
            RichTextSpanType::ForegroundColor => {
                style.color = theme.foreground(span.color_value());
            }
            RichTextSpanType::FontSize => {
                if let Some(size) = span.font_size_value() {
                    style.font_size = samsung_font_to_svg(size);
                }
            }
            RichTextSpanType::Bold => style.bold = span.boolean_value() == Some(true),
            RichTextSpanType::Italic => style.italic = span.boolean_value() == Some(true),
            RichTextSpanType::Underline => style.underline = span.boolean_value() == Some(true),
            RichTextSpanType::Strikethrough => {
                style.strikethrough = span.boolean_value() == Some(true)
            }
            RichTextSpanType::Hyperlink => {
                is_hyperlink = true;
                style.link_target = hyperlink_target(text_box, span);
            }
            _ => {}
        }
    }
    if is_hyperlink {
        style.color = theme.foreground(Some(Color {
            r: 0,
            g: 84,
            b: 255,
        }));
        style.underline = true;
    }
    if matches!(
        predefined_style,
        Some(
            PredefinedTextStyle::Heading1
                | PredefinedTextStyle::Heading2
                | PredefinedTextStyle::Heading3
        )
    ) {
        // Markdown headings carry a bold span, but Samsung's PDF exporter uses
        // the heading face at regular weight.
        style.bold = false;
    }
    style
}

fn hyperlink_target(text_box: &RichTextBox, span: &crate::RichTextSpan) -> Option<String> {
    let hyperlink = span.hyperlink_value()?;
    if let Some(target) = hyperlink.custom_data.filter(|target| !target.is_empty()) {
        return sanitize_hyperlink_target(target);
    }
    let start = utf16_to_char_index(&text_box.text, span.start_utf16)?;
    let end = utf16_to_char_index(&text_box.text, span.end_utf16)?;
    let byte_offsets = char_byte_offsets(&text_box.text);
    let visible_text = &text_box.text[byte_offsets[start]..byte_offsets[end]];
    let target = match hyperlink.kind {
        HyperlinkType::Email => Some(format!("mailto:{visible_text}")),
        HyperlinkType::Telephone => Some(format!("tel:{visible_text}")),
        HyperlinkType::Url => Some(visible_text.to_string()),
        _ => None,
    }?;
    sanitize_hyperlink_target(target)
}

fn sanitize_hyperlink_target(target: String) -> Option<String> {
    let target = target.trim();
    if target.is_empty() || target.chars().any(char::is_control) {
        return None;
    }

    if target.starts_with('#') || target.starts_with('?') {
        return Some(target.to_string());
    }
    if target.starts_with("//") || target.starts_with('\\') {
        return None;
    }
    if target.starts_with('/') || target.starts_with("./") || target.starts_with("../") {
        return Some(target.to_string());
    }

    if let Some(colon_index) = target.find(':') {
        let path_delimiter = target
            .char_indices()
            .find_map(|(index, character)| matches!(character, '/' | '?' | '#').then_some(index));
        if path_delimiter.is_none_or(|index| colon_index < index) {
            let scheme = &target[..colon_index];
            if !["http", "https", "mailto", "tel"]
                .iter()
                .any(|allowed| scheme.eq_ignore_ascii_case(allowed))
            {
                return None;
            }
        }
    }

    Some(target.to_string())
}

fn write_styled_tspan(svg: &mut Scene, text: &str, style: &SvgTextStyle) {
    let mut span = TSpan::new(text)
        .fill(Paint::from_hex(&style.color))
        .font_size(decimal(style.font_size, 2));
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
        span = span
            .stroke(Paint::from_hex(style.color.as_str()))
            .stroke_width(0.45)
            .stroke_under_fill();
    }
    if style.italic {
        span = span.italic();
    }
    if let Some(target) = &style.link_target {
        svg.scope(Anchor::new(target), |svg| svg.push(span));
    } else {
        svg.push(span);
    }
}

fn render_embedded_object(
    svg: &mut Scene,
    object: &RichTextObjectSpan,
    cursor_y: f64,
    media_assets: &[MediaAsset],
    theme: RenderTheme,
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
            let offset_y = object_flow_offset(drawn.y_min, cursor_y, 0.0);
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
            let offset_y =
                object_flow_offset(table.bbox.y_min, cursor_y, object_top_margin(object));
            svg.scope(Group::new().object(ObjectKind::Table), |svg| {
                let stroke = if theme.is_dark() {
                    "#777777"
                } else {
                    "#b8b0a3"
                };
                let clip = svg.definition::<Clip>();
                svg.push(
                    Definitions::new()
                        .add(ClipPath::new(&clip).add(rectangle(table.bbox, offset_y, 2).rx(24))),
                );
                svg.scope(Group::new().clipped(&clip), |svg| {
                    for row in &table.rows {
                        for cell in &row.cells {
                            let cell_background = table_cell_background(cell, theme);
                            let cell_theme = theme.on_background(cell_background);
                            svg.push(
                                rectangle(cell.bbox, offset_y, 2)
                                    .fill(Paint::from_hex(&color_hex(&cell_background))),
                            );
                            if cell.bbox.x_min > table.bbox.x_min + 1.0 {
                                svg.push(
                                    Line::new()
                                        .x1(decimal(cell.bbox.x_min, 2))
                                        .y1(decimal(cell.bbox.y_min + offset_y, 2))
                                        .x2(decimal(cell.bbox.x_min, 2))
                                        .y2(decimal(cell.bbox.y_max + offset_y, 2))
                                        .stroke(Paint::from_hex(stroke))
                                        .stroke_width(1),
                                );
                            }
                            if cell.bbox.y_min > table.bbox.y_min + 1.0 {
                                svg.push(
                                    Line::new()
                                        .x1(decimal(cell.bbox.x_min, 2))
                                        .y1(decimal(cell.bbox.y_min + offset_y, 2))
                                        .x2(decimal(cell.bbox.x_max, 2))
                                        .y2(decimal(cell.bbox.y_min + offset_y, 2))
                                        .stroke(Paint::from_hex(stroke))
                                        .stroke_width(1),
                                );
                            }
                            if let Some(line) = cell.content.text.lines().next() {
                                let mut style = text_style_at(&cell.content, 0, cell_theme, None);
                                style.bold = false;
                                svg.scope(
                                    Text::new("")
                                        .x(decimal(cell.bbox.x_min + 23., 2))
                                        .y(decimal(cell.bbox.y_min + offset_y + 81., 2))
                                        .family(FontFamily::Roboto)
                                        .preserve_space(),
                                    |svg| {
                                        write_styled_tspan(svg, line, &style);
                                    },
                                );
                            }
                        }
                    }
                });
                svg.push(
                    rectangle(table.bbox, offset_y, 2)
                        .rx(24)
                        .fill(Paint::None)
                        .stroke(Paint::from_hex(stroke))
                        .stroke_width(1),
                );
            });
            Some(table.bbox.y_max + offset_y)
        }
        Some(RichTextObjectContent::CodeBlock(code)) => {
            let offset_y = object_flow_offset(code.bbox.y_min, cursor_y, object_top_margin(object));
            let background = argb_color(if theme.is_dark() { 0x333333 } else { 0xefefef });
            let theme = theme.on_background(background);
            let fill = color_hex(&background);
            let stroke = if theme.is_dark() {
                "#5f5f5f"
            } else {
                "#dddddd"
            };
            svg.scope(Group::new().object(ObjectKind::CodeBlock), |svg| {
                svg.push(
                    rectangle(code.bbox, offset_y, 2)
                        .rx(36)
                        .fill(Paint::from_hex(&fill))
                        .stroke(Paint::from_hex(stroke))
                        .stroke_width(1),
                );
                let object_top = code.bbox.y_min + offset_y;
                let text_x = code.bbox.x_min + 81.75;
                if let Some(title) = &code.title {
                    render_embedded_line(
                        svg,
                        title,
                        title.text.lines().next().unwrap_or_default(),
                        0,
                        text_x,
                        object_top + 81.6,
                        FontFamily::Roboto,
                        theme,
                    );
                }
                let icon_stroke = if theme.is_dark() {
                    "#b7b7b7"
                } else {
                    "#8b8b8b"
                };
                let icon = Data::new()
                    .move_to((
                        coordinate(code.bbox.x_min + 895., 2),
                        coordinate(object_top + 61., 2),
                    ))
                    .vertical_line_by(-4)
                    .quadratic_curve_by((0., -8., 8., -8.))
                    .horizontal_line_by(17)
                    .quadratic_curve_by((8., 0., 8., 8.))
                    .vertical_line_by(29);
                svg.push(
                    Group::new()
                        .fill(Paint::None)
                        .stroke(Paint::from_hex(icon_stroke))
                        .stroke_width(6)
                        .line_join(LineJoin::Round)
                        .add(Path::new().data(icon))
                        .add(
                            Rectangle::new()
                                .x(decimal(code.bbox.x_min + 879., 2))
                                .y(decimal(object_top + 59., 2))
                                .width(31)
                                .height(38)
                                .rx(5),
                        ),
                );
                if let Some(body) = &code.body {
                    let mut baseline = object_top + 177.6;
                    let mut character_start = 0_usize;
                    for (line_index, line) in body.text.lines().enumerate() {
                        render_embedded_line(
                            svg,
                            body,
                            line,
                            character_start,
                            text_x,
                            baseline,
                            FontFamily::Roboto,
                            theme,
                        );
                        character_start += line.chars().count() + 1;
                        baseline += if line_index == 0 { 98.25 } else { 60.75 };
                    }
                }
            });
            Some(code.bbox.y_max + offset_y)
        }
        None => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn render_embedded_line(
    svg: &mut Scene,
    text_box: &RichTextBox,
    line: &str,
    character_start: usize,
    x: f64,
    baseline: f64,
    font_family: FontFamily,
    theme: RenderTheme,
) {
    let utf16_index = text_box
        .text
        .chars()
        .take(character_start)
        .map(|character| character.len_utf16() as u32)
        .sum();
    let style = text_style_at(text_box, utf16_index, theme, None);
    svg.scope(
        Text::new("")
            .x(decimal(x, 2))
            .y(decimal(baseline, 2))
            .family(font_family)
            .preserve_space(),
        |svg| {
            write_styled_tspan(svg, line, &style);
        },
    );
}

fn object_flow_offset(stored_top: f64, cursor_y: f64, top_margin: f64) -> f64 {
    if stored_top < 0.0 {
        0.0
    } else {
        cursor_y + top_margin - stored_top
    }
}

fn argb_color(argb: u32) -> Color {
    Color {
        r: (argb >> 16) as u8,
        g: (argb >> 8) as u8,
        b: argb as u8,
    }
}

fn table_cell_background(cell: &crate::RichTextTableCell, theme: RenderTheme) -> Color {
    if !cell.has_own_background_color {
        return theme.background();
    }
    if cell.background_color == 0 {
        return argb_color(if theme.is_dark() { 0x45413d } else { 0xeeebe7 });
    }
    argb_color(cell.background_color)
}

fn object_top_margin(object: &RichTextObjectSpan) -> f64 {
    match object.layout_option {
        crate::ObjectSpanLayoutOption::Block => 38.0,
        crate::ObjectSpanLayoutOption::Inline => 0.0,
        crate::ObjectSpanLayoutOption::BlockWithSmallMargin => 18.0,
        crate::ObjectSpanLayoutOption::BlockWithMediumMargin => 36.0,
        _ => 0.0,
    }
}

fn object_bottom_margin(object: &RichTextObjectSpan) -> f64 {
    match object.layout_option {
        // The paragraph itself contributes 12 px after the object. Together
        // these reproduce the 40 px block-to-text gap in Samsung's PDF.
        crate::ObjectSpanLayoutOption::Block => 28.0,
        crate::ObjectSpanLayoutOption::BlockWithSmallMargin => 12.0,
        crate::ObjectSpanLayoutOption::BlockWithMediumMargin => 24.0,
        _ => 0.0,
    }
}

fn char_utf16_offsets(text: &str) -> Vec<u32> {
    let mut offsets = Vec::with_capacity(text.chars().count() + 1);
    let mut offset = 0_u32;
    for character in text.chars() {
        offsets.push(offset);
        offset = offset.saturating_add(character.len_utf16() as u32);
    }
    offsets.push(offset);
    offsets
}

fn utf16_to_char_index(text: &str, target: u32) -> Option<usize> {
    let target = usize::try_from(target).ok()?;
    let mut utf16_offset = 0_usize;
    for (char_index, character) in text.chars().enumerate() {
        if utf16_offset == target {
            return Some(char_index);
        }
        utf16_offset = utf16_offset.checked_add(character.len_utf16())?;
        if utf16_offset > target {
            return None;
        }
    }
    (utf16_offset == target).then_some(text.chars().count())
}

struct StyledSpan<'a> {
    text: &'a str,
    bold: bool,
    italic: bool,
}

fn styled_line_spans<'a>(
    line: &'a str,
    line_start: usize,
    runs: &[RichTextRun],
) -> Vec<StyledSpan<'a>> {
    let char_count = line.chars().count();
    let mut boundaries = vec![0, char_count];
    for run in runs {
        let start = run.start.saturating_sub(line_start).min(char_count);
        let end = run.end.saturating_sub(line_start).min(char_count);
        if start < end {
            boundaries.push(start);
            boundaries.push(end);
        }
    }
    boundaries.sort_unstable();
    boundaries.dedup();

    let byte_offsets = char_byte_offsets(line);
    let mut spans = Vec::new();
    for pair in boundaries.windows(2) {
        let start = pair[0];
        let end = pair[1];
        if start == end {
            continue;
        }
        let global_start = line_start + start;
        let global_end = line_start + end;
        let mut bold = false;
        let mut italic = false;
        for run in runs {
            if run.start < global_end && run.end > global_start {
                bold |= run.bold;
                italic |= run.italic;
            }
        }
        spans.push(StyledSpan {
            text: &line[byte_offsets[start]..byte_offsets[end]],
            bold,
            italic,
        });
    }
    spans
}

fn char_byte_offsets(text: &str) -> Vec<usize> {
    let mut offsets: Vec<usize> = text.char_indices().map(|(offset, _)| offset).collect();
    offsets.push(text.len());
    offsets
}

fn samsung_font_to_svg(size: f32) -> f64 {
    let size = size as f64;
    if size.is_finite() && size > 0.0 {
        (size * SAMSUNG_TEXT_SCALE).clamp(8.0, 144.0)
    } else {
        37.0
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
        object_flow_offset, render_document_svg, render_layout_page_svg, samsung_font_to_svg,
        sanitize_hyperlink_target,
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
            metadata: DocumentMetadata::default(),
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
    fn uses_pdf_measured_text_and_object_flow_units() {
        assert_eq!(samsung_font_to_svg(15.0), 45.0);
        assert_eq!(object_flow_offset(949.5, 984.0, 36.0), 70.5);
        assert_eq!(object_flow_offset(-229.25, 42.0, 38.0), 0.0);
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
            let options = RenderOptions { color_mode };
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
            },
        );
        let dark = render_document_svg(
            &doc,
            &RenderOptions {
                color_mode: RenderColorMode::Dark,
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
            let svg = render_document_svg(&doc, &RenderOptions { color_mode: mode })
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
                    .any(|n| n.has_tag_name("text") && n.attribute("fill") == Some(foreground))
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
            },
        )
        .remove(0)
        .svg;
        let xml = roxmltree::Document::parse(&svg).unwrap();
        assert!(
            xml.descendants()
                .any(|n| n.has_tag_name("text") && n.attribute("fill") == Some("#000000"))
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
            let svg = render_document_svg(&doc, &RenderOptions { color_mode: mode })
                .remove(0)
                .svg;
            let xml = roxmltree::Document::parse(&svg).unwrap();
            assert!(
                xml.descendants()
                    .any(|n| n.attribute("stroke") == Some(ink))
            );
            assert!(
                xml.descendants()
                    .any(|n| n.has_tag_name("text") && n.attribute("fill") == Some(ink))
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
                .any(|n| n.has_tag_name("text") && n.attribute("fill") == Some("#ffffff"))
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
            background_color: 0xffffffff,
            has_own_background_color: true,
            bbox: BoundingBox::default(),
            vertical_alignment: 0,
            content: theme_test_text(),
        };
        let surface = super::table_cell_background(&cell, theme);
        assert_eq!(
            super::text_style_at(&cell.content, 0, theme.on_background(surface), None).color,
            "#000000"
        );
        cell.has_own_background_color = false;
        assert_eq!(
            super::table_cell_background(&cell, theme),
            page.background_color.unwrap()
        );
        assert_eq!(
            super::text_style_at(&cell.content, 0, theme, None).color,
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
            },
        );

        let xml = roxmltree::Document::parse(&pages[0].svg).unwrap();
        assert!(xml.descendants().any(|node| node.has_tag_name("text")
            && node.attribute("x") == Some("48.00")
            && node.attribute("y") == Some("45.00")));
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
                    expand: false,
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
        assert!(pages[0].svg.contains(r#"text-decoration="underline""#));
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
        let text_at = svg.find("under the highlighter").unwrap();
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
