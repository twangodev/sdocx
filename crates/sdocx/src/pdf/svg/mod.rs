//! Bundled krilla-svg 0.8.1 converter; provenance and licenses are recorded beside this module.

use std::sync::Arc;

use fontdb::Database;
use krilla::geom::{Rect, Size, Transform};
use krilla::paint::FillRule;
use krilla::surface::Surface;
use usvg::{Tree, fontdb};

use self::text::Fonts;
use self::util::{RectExt, UsvgTransformExt};

mod clip_path;
mod colorize;
mod filter;
mod group;
mod image;
mod mask;
mod path;
mod text;
mod util;

/// Settings that should be applied when converting a SVG.
#[derive(Copy, Clone, Debug)]
pub(super) struct SvgSettings {
    /// Whether text should be embedded as properly selectable text. Otherwise,
    /// it will be drawn as outlined paths instead.
    pub embed_text: bool,
    /// How much filters, which will be converted to bitmaps, should be scaled. Higher values
    /// mean better quality, but also bigger file sizes.
    pub filter_scale: f32,
    pub reject_images: bool,
    /// Require generated document filters to stay vector content.
    pub reject_rasterized_filters: bool,
}

impl Default for SvgSettings {
    fn default() -> Self {
        Self {
            embed_text: true,
            filter_scale: 4.0,
            reject_images: false,
            reject_rasterized_filters: false,
        }
    }
}

/// An extension trait for the `Surface` struct that allows you to draw SVGs onto a surface.
pub(super) trait SurfaceExt {
    /// Draw a `usvg` tree onto a surface with the given size and settings.
    fn draw_svg(&mut self, tree: &Tree, size: Size, svg_settings: SvgSettings) -> Option<()>;

    #[cfg(test)]
    fn draw_svg_with_text(
        &mut self,
        tree: &Tree,
        size: Size,
        svg_settings: SvgSettings,
        hook: &mut TextHook<'_>,
    ) -> Result<(), SvgError>;

    fn draw_svg_with_text_and_ink(
        &mut self,
        tree: &Tree,
        size: Size,
        svg_settings: SvgSettings,
        text_hook: &mut TextHook<'_>,
        ink_ids: &std::collections::HashSet<String>,
        ink_hook: &mut InkHook<'_>,
    ) -> Result<(), SvgError>;
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum SvgError {
    Text(String),
    Image(String),
    Filter(String),
    Ink(String),
}

pub(super) fn raster_image(kind: &usvg::ImageKind) -> Result<Option<krilla::image::Image>, String> {
    use krilla::image::Image;
    use usvg::ImageKind;

    match kind {
        ImageKind::JPEG(bytes) => Image::from_jpeg(bytes.clone().into(), false),
        ImageKind::PNG(bytes) => Image::from_png(bytes.clone().into(), false),
        ImageKind::GIF(bytes) => Image::from_gif(bytes.clone().into(), false),
        ImageKind::WEBP(bytes) => Image::from_webp(bytes.clone().into(), false),
        ImageKind::SVG(_) => return Ok(None),
    }
    .map(Some)
}

/// Render an SVG-table color glyph through the same vector and image handlers used for
/// document SVG content. Krilla already applies the glyph origin and em-space Y flip before
/// invoking this callback; this function only adapts the SVG viewport to the font's em square.
pub(super) fn render_svg_glyph(
    data: &[u8],
    context_color: krilla::color::rgb::Color,
    glyph: krilla::text::GlyphId,
    default_size: (f32, f32),
    surface: &mut Surface<'_>,
) -> Option<()> {
    let default_size = usvg::Size::from_wh(default_size.0, default_size.1)?;
    let decoded;
    let data = if data.starts_with(&[0x1f, 0x8b]) {
        decoded = usvg::decompress_svgz(data).ok()?;
        decoded.as_slice()
    } else {
        data
    };
    let xml = std::str::from_utf8(data).ok()?;
    let document = roxmltree::Document::parse(xml).ok()?;
    let root_element = document.root_element();
    let options = usvg::Options {
        style_sheet: Some(format!(
            "svg {{ color: rgb({}, {}, {}) }}",
            context_color.red(),
            context_color.green(),
            context_color.blue()
        )),
        default_size,
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_data: usvg::ImageHrefResolver::default_data_resolver(),
            // Font glyph resources are self-contained. Do not allow font bytes to cause
            // host filesystem reads through a relative or absolute image href.
            resolve_string: Box::new(|_, _| None),
        },
        ..Default::default()
    };
    let tree = Tree::from_xmltree(&document, &options).ok()?;
    let root_transform = root_glyph_transform(root_element, default_size)?;
    let parsed_root_transform = root_view_box_transform(root_element, tree.size())?;
    let node = tree.node_by_id(&format!("glyph{}", glyph.to_u32()));
    let requires_raster_filter = node.map_or_else(
        || {
            tree.root()
                .children()
                .iter()
                .any(node_requires_raster_filter)
        },
        node_requires_raster_filter,
    );
    if requires_raster_filter {
        return None;
    }

    let settings = SvgSettings {
        reject_rasterized_filters: true,
        ..SvgSettings::default()
    };
    let mut database = tree.fontdb().clone();
    let mut context = ProcessContext::new(Arc::make_mut(&mut database), settings);
    if let Some(node) = node {
        // OpenType requires glyph elements to behave like SVG <use> references. The root
        // viewport transform and the glyph element's own transform apply; transforms on
        // intervening ancestor groups do not.
        surface.push_transform(&root_transform.to_krilla());
        group::render_node(node, surface, &mut context);
        surface.pop();
    } else {
        // Some fonts put the glyph identifier on the root <svg>, which usvg does not retain.
        let correction = root_transform.pre_concat(parsed_root_transform.invert()?);
        surface.push_transform(&correction.to_krilla());
        group::render(tree.root(), surface, &mut context);
        surface.pop();
    }
    // The preflight and permissive image setting prevent all known renderer errors. Keep the
    // callback consumed on unexpected partial failures so Krilla does not overlay a monochrome
    // outline on vector/image content that was already emitted.
    Some(())
}

fn root_glyph_transform(
    root: roxmltree::Node<'_, '_>,
    em_size: usvg::Size,
) -> Option<usvg::Transform> {
    use std::str::FromStr;
    use svgtypes::LengthUnit;

    let dimension = |attribute: &str, default: f32| {
        let Some(value) = root.attribute(attribute) else {
            return Some(default);
        };
        let length = svgtypes::Length::from_str(value).ok()?;
        let value = match length.unit {
            LengthUnit::None | LengthUnit::Px => length.number as f32,
            LengthUnit::In => length.number as f32 * 96.0,
            LengthUnit::Cm => length.number as f32 * (96.0 / 2.54),
            LengthUnit::Mm => length.number as f32 * (96.0 / 25.4),
            LengthUnit::Pt => length.number as f32 * (96.0 / 72.0),
            LengthUnit::Pc => length.number as f32 * 16.0,
            LengthUnit::Percent => default * (length.number as f32 / 100.0),
            LengthUnit::Em | LengthUnit::Ex => return None,
        };
        (value.is_finite() && value > 0.0).then_some(value)
    };
    let viewport_size = usvg::Size::from_wh(
        dimension("width", em_size.width())?,
        dimension("height", em_size.height())?,
    )?;
    let view_box_transform = root_view_box_transform(root, viewport_size)?;
    let viewport_to_em = usvg::Transform::from_scale(
        em_size.width() / viewport_size.width(),
        em_size.height() / viewport_size.height(),
    );
    Some(viewport_to_em.pre_concat(view_box_transform))
}

fn root_view_box_transform(
    root: roxmltree::Node<'_, '_>,
    size: usvg::Size,
) -> Option<usvg::Transform> {
    use std::str::FromStr;
    use svgtypes::Align;

    let Some(view_box) = root.attribute("viewBox") else {
        return Some(usvg::Transform::identity());
    };
    let view_box = svgtypes::ViewBox::from_str(view_box).ok()?;
    let aspect = root
        .attribute("preserveAspectRatio")
        .map(svgtypes::AspectRatio::from_str)
        .transpose()
        .ok()?
        .unwrap_or_default();

    let mut sx = size.width() / view_box.w as f32;
    let mut sy = size.height() / view_box.h as f32;
    if aspect.align != Align::None {
        let scale = if aspect.slice { sx.max(sy) } else { sx.min(sy) };
        sx = scale;
        sy = scale;
    }

    let x = -(view_box.x as f32) * sx;
    let y = -(view_box.y as f32) * sy;
    let extra_x = size.width() - view_box.w as f32 * sx;
    let extra_y = size.height() - view_box.h as f32 * sy;
    let align_x = match aspect.align {
        Align::XMidYMin | Align::XMidYMid | Align::XMidYMax => 0.5,
        Align::XMaxYMin | Align::XMaxYMid | Align::XMaxYMax => 1.0,
        _ => 0.0,
    };
    let align_y = match aspect.align {
        Align::XMinYMid | Align::XMidYMid | Align::XMaxYMid => 0.5,
        Align::XMinYMax | Align::XMidYMax | Align::XMaxYMax => 1.0,
        _ => 0.0,
    };
    Some(usvg::Transform::from_row(
        sx,
        0.0,
        0.0,
        sy,
        x + extra_x * align_x,
        y + extra_y * align_y,
    ))
}

fn node_requires_raster_filter(node: &usvg::Node) -> bool {
    match node {
        usvg::Node::Group(group) => {
            (!group.filters().is_empty() && !colorize::is_supported(group))
                || group.children().iter().any(node_requires_raster_filter)
        }
        usvg::Node::Image(image) => match image.kind() {
            usvg::ImageKind::SVG(tree) => tree
                .root()
                .children()
                .iter()
                .any(node_requires_raster_filter),
            _ => false,
        },
        usvg::Node::Path(path) => {
            path_patterns(path).any(|root| root.children().iter().any(node_requires_raster_filter))
        }
        usvg::Node::Text(_) => false,
    }
}

pub(super) type TextHook<'a> =
    dyn FnMut(&usvg::Text, &mut Surface<'_>) -> Result<bool, String> + 'a;
pub(super) type InkHook<'a> = dyn FnMut(&usvg::Group, &mut Surface<'_>) -> Result<(), String> + 'a;

impl SurfaceExt for Surface<'_> {
    fn draw_svg(&mut self, tree: &Tree, size: Size, svg_settings: SvgSettings) -> Option<()> {
        draw_svg(self, tree, size, svg_settings, None, None, None).ok()
    }

    #[cfg(test)]
    fn draw_svg_with_text(
        &mut self,
        tree: &Tree,
        size: Size,
        svg_settings: SvgSettings,
        hook: &mut TextHook<'_>,
    ) -> Result<(), SvgError> {
        draw_svg(self, tree, size, svg_settings, Some(hook), None, None)
    }

    fn draw_svg_with_text_and_ink(
        &mut self,
        tree: &Tree,
        size: Size,
        svg_settings: SvgSettings,
        text_hook: &mut TextHook<'_>,
        ink_ids: &std::collections::HashSet<String>,
        ink_hook: &mut InkHook<'_>,
    ) -> Result<(), SvgError> {
        draw_svg(
            self,
            tree,
            size,
            svg_settings,
            Some(text_hook),
            Some(ink_ids),
            Some(ink_hook),
        )
    }
}

fn draw_svg<'h, 'o>(
    surface: &mut Surface<'_>,
    tree: &Tree,
    size: Size,
    svg_settings: SvgSettings,
    hook: Option<&'h mut TextHook<'o>>,
    ink_ids: Option<&'h std::collections::HashSet<String>>,
    ink_hook: Option<&'h mut InkHook<'o>>,
) -> Result<(), SvgError> {
    let old_fill = surface.get_fill().cloned();
    let old_stroke = surface.get_stroke().cloned();

    let transform = Transform::from_scale(
        size.width() / tree.size().width(),
        size.height() / tree.size().height(),
    );
    surface.push_transform(&transform);
    surface.push_clip_path(
        &Rect::from_xywh(0.0, 0.0, tree.size().width(), tree.size().height())
            .unwrap()
            .to_clip_path(),
        &FillRule::NonZero,
    );
    let result = render_tree_with_text(tree, svg_settings, surface, hook, ink_ids, ink_hook);
    surface.pop();
    surface.pop();

    surface.set_fill(old_fill);
    surface.set_stroke(old_stroke);

    result
}

struct ProcessContext<'a, 'h, 'o> {
    pub(super) fonts: Fonts<'a>,
    svg_settings: SvgSettings,
    cull_viewport: Option<usvg::Rect>,
    cull_candidates: std::collections::HashMap<usize, usvg::Rect>,
    text_hook: Option<&'h mut TextHook<'o>>,
    ink_ids: Option<&'h std::collections::HashSet<String>>,
    ink_hook: Option<&'h mut InkHook<'o>>,
    error: Option<SvgError>,
}

impl<'a, 'h, 'o> ProcessContext<'a, 'h, 'o> {
    fn new(tree_fontdb: &'a mut Database, svg_settings: SvgSettings) -> Self {
        Self {
            fonts: Fonts::new(tree_fontdb),
            svg_settings,
            cull_viewport: None,
            cull_candidates: std::collections::HashMap::new(),
            text_hook: None,
            ink_ids: None,
            ink_hook: None,
            error: None,
        }
    }

    fn render_text(&mut self, node: &usvg::Text, surface: &mut Surface<'_>) {
        match self
            .text_hook
            .as_deref_mut()
            .map(|hook| hook(node, surface))
        {
            Some(Ok(true)) => {}
            Some(Err(error)) => self.error = Some(SvgError::Text(error)),
            None | Some(Ok(false)) => text::render(node, surface, self),
        }
    }

    pub(super) fn has_native_ink(&self, group: &usvg::Group) -> bool {
        self.ink_ids.is_some_and(|ids| ids.contains(group.id()))
    }

    pub(super) fn render_native_ink(&mut self, group: &usvg::Group, surface: &mut Surface<'_>) {
        if let Some(hook) = self.ink_hook.as_deref_mut() {
            if let Err(error) = hook(group, surface) {
                self.error = Some(SvgError::Ink(error));
            }
        } else {
            self.error = Some(SvgError::Ink(
                "registered native ink group has no renderer".into(),
            ));
        }
    }

    fn set_cull_viewport(&mut self, root: &usvg::Group, viewport: usvg::Rect) {
        self.cull_viewport = Some(viewport);
        collect_cull_candidates(root, usvg::Transform::default(), &mut self.cull_candidates);
    }

    pub(super) fn should_cull_node(&self, node: &usvg::Node) -> bool {
        let Some(bbox) = self.cull_candidates.get(&(node as *const _ as usize)) else {
            return false;
        };
        let Some(viewport) = self.cull_viewport else {
            return false;
        };
        if ![bbox.left(), bbox.top(), bbox.right(), bbox.bottom()]
            .into_iter()
            .all(f32::is_finite)
        {
            return false;
        }
        let outside = bbox.right() < viewport.left()
            || bbox.left() > viewport.right()
            || bbox.bottom() < viewport.top()
            || bbox.top() > viewport.bottom();
        if !outside {
            return false;
        }

        if self.svg_settings.reject_images && node_contains_image(node) {
            return false;
        }
        if self.svg_settings.reject_rasterized_filters
            && (node_contains_pattern(node) || node_contains_unsupported_filter(node))
        {
            return false;
        }
        let has_custom_painter =
            self.text_hook.is_some() || self.ink_ids.is_some_and(|ids| !ids.is_empty());
        let validates_hidden_content =
            self.svg_settings.reject_images || self.svg_settings.reject_rasterized_filters;
        if (has_custom_painter || validates_hidden_content) && node_contains_mask_or_clip(node) {
            return false;
        }
        if (self.text_hook.is_some() || validates_hidden_content) && node_contains_text(node) {
            return false;
        }
        if let Some(ink_ids) = self.ink_ids
            && !ink_ids.is_empty()
            && node_contains_native_ink(node, ink_ids)
        {
            return false;
        }

        true
    }
}

fn collect_cull_candidates(
    group: &usvg::Group,
    parent_transform: usvg::Transform,
    candidates: &mut std::collections::HashMap<usize, usvg::Rect>,
) {
    // A filter can move pixels from an offscreen child into the viewport. The filtered group's
    // own layer bounds are a safe culling boundary, but its descendants are not.
    if !group.filters().is_empty() {
        return;
    }

    let group_transform = parent_transform.pre_concat(group.transform());
    for node in group.children() {
        let paint_bounds = match node {
            usvg::Node::Group(child) => child
                .layer_bounding_box()
                .to_rect()
                .transform(group_transform.pre_concat(child.transform())),
            _ => node.stroke_bounding_box().transform(group_transform),
        };
        if let Some(paint_bounds) = paint_bounds {
            candidates.insert(node as *const _ as usize, paint_bounds);
        }
        if let usvg::Node::Group(child) = node {
            collect_cull_candidates(child, group_transform, candidates);
        }
    }
}

fn node_contains_text(node: &usvg::Node) -> bool {
    match node {
        usvg::Node::Text(_) => true,
        usvg::Node::Group(group) => group.children().iter().any(node_contains_text),
        usvg::Node::Image(image) => match image.kind() {
            usvg::ImageKind::SVG(tree) => tree.root().children().iter().any(node_contains_text),
            _ => false,
        },
        usvg::Node::Path(path) => {
            path_patterns(path).any(|root| root.children().iter().any(node_contains_text))
        }
    }
}

fn node_contains_native_ink(
    node: &usvg::Node,
    ink_ids: &std::collections::HashSet<String>,
) -> bool {
    match node {
        usvg::Node::Group(group) => {
            ink_ids.contains(group.id())
                || group
                    .children()
                    .iter()
                    .any(|child| node_contains_native_ink(child, ink_ids))
        }
        usvg::Node::Image(image) => match image.kind() {
            usvg::ImageKind::SVG(tree) => tree
                .root()
                .children()
                .iter()
                .any(|child| node_contains_native_ink(child, ink_ids)),
            _ => false,
        },
        usvg::Node::Path(path) => path_patterns(path).any(|root| {
            root.children()
                .iter()
                .any(|child| node_contains_native_ink(child, ink_ids))
        }),
        usvg::Node::Text(_) => false,
    }
}

fn node_contains_image(node: &usvg::Node) -> bool {
    match node {
        usvg::Node::Image(_) => true,
        usvg::Node::Group(group) => group_contains_image(group),
        usvg::Node::Path(path) => path_patterns(path).any(group_contains_image),
        usvg::Node::Text(_) => false,
    }
}

fn node_contains_pattern(node: &usvg::Node) -> bool {
    match node {
        usvg::Node::Group(group) => group.children().iter().any(node_contains_pattern),
        usvg::Node::Path(path) => path_patterns(path).next().is_some(),
        usvg::Node::Image(image) => match image.kind() {
            usvg::ImageKind::SVG(tree) => tree.root().children().iter().any(node_contains_pattern),
            _ => false,
        },
        usvg::Node::Text(_) => false,
    }
}

fn node_contains_unsupported_filter(node: &usvg::Node) -> bool {
    match node {
        usvg::Node::Group(group) => {
            (!group.filters().is_empty() && !colorize::is_supported(group))
                || group
                    .children()
                    .iter()
                    .any(node_contains_unsupported_filter)
        }
        usvg::Node::Image(image) => match image.kind() {
            usvg::ImageKind::SVG(tree) => tree
                .root()
                .children()
                .iter()
                .any(node_contains_unsupported_filter),
            _ => false,
        },
        usvg::Node::Path(path) => path_patterns(path)
            .any(|root| root.children().iter().any(node_contains_unsupported_filter)),
        usvg::Node::Text(_) => false,
    }
}

fn node_contains_mask_or_clip(node: &usvg::Node) -> bool {
    match node {
        usvg::Node::Group(group) => group_contains_mask_or_clip(group),
        usvg::Node::Image(image) => match image.kind() {
            usvg::ImageKind::SVG(tree) => group_contains_mask_or_clip(tree.root()),
            _ => false,
        },
        usvg::Node::Path(path) => path_patterns(path).any(group_contains_mask_or_clip),
        usvg::Node::Text(_) => false,
    }
}

fn group_contains_mask_or_clip(group: &usvg::Group) -> bool {
    group.mask().is_some()
        || group.clip_path().is_some()
        || group.children().iter().any(node_contains_mask_or_clip)
}

fn group_contains_image(group: &usvg::Group) -> bool {
    group.filters().iter().any(|filter| {
        filter
            .primitives()
            .iter()
            .any(|primitive| matches!(primitive.kind(), usvg::filter::Kind::Image(_)))
    }) || group.children().iter().any(node_contains_image)
}

fn path_patterns(path: &usvg::Path) -> impl Iterator<Item = &usvg::Group> {
    [
        path.fill().map(|fill| fill.paint()),
        path.stroke().map(|stroke| stroke.paint()),
    ]
    .into_iter()
    .flatten()
    .filter_map(|paint| match paint {
        usvg::Paint::Pattern(pattern) => Some(pattern.root()),
        _ => None,
    })
}

fn render_tree_with_text<'h, 'o>(
    tree: &Tree,
    svg_settings: SvgSettings,
    surface: &mut Surface<'_>,
    hook: Option<&'h mut TextHook<'o>>,
    ink_ids: Option<&'h std::collections::HashSet<String>>,
    ink_hook: Option<&'h mut InkHook<'o>>,
) -> Result<(), SvgError> {
    let mut db = tree.fontdb().clone();
    let mut fc = ProcessContext::new(Arc::make_mut(&mut db), svg_settings);
    fc.text_hook = hook;
    fc.ink_ids = ink_ids;
    fc.ink_hook = ink_hook;
    if let Some(viewport) =
        usvg::Rect::from_xywh(0.0, 0.0, tree.size().width(), tree.size().height())
    {
        fc.set_cull_viewport(tree.root(), viewport);
    }
    group::render(tree.root(), surface, &mut fc);
    fc.error.map_or(Ok(()), Err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use krilla::{Document, page::PageSettings};

    fn tree(content: &str) -> Tree {
        let mut fontdb = Database::new();
        fontdb.load_font_data(include_bytes!("../../../assets/fonts/Roboto-Regular.ttf").to_vec());
        Tree::from_str(
            &format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><g font-family="Roboto" font-size="20">{content}</g></svg>"#
            ),
            &usvg::Options {
                fontdb: Arc::new(fontdb),
                ..Default::default()
            },
        )
        .unwrap()
    }

    #[test]
    fn root_view_box_transform_honors_aspect_ratio_and_nonzero_origin() {
        let size = usvg::Size::from_wh(200.0, 200.0).unwrap();
        let transform_for = |attributes: &str| {
            let xml = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" {attributes}/>"#);
            let document = roxmltree::Document::parse(&xml).unwrap();
            root_glyph_transform(document.root_element(), size).unwrap()
        };

        assert_eq!(
            transform_for(r#"viewBox="10 20 100 50""#),
            usvg::Transform::from_row(2.0, 0.0, 0.0, 2.0, -20.0, 10.0)
        );
        assert_eq!(
            transform_for(r#"viewBox="10 20 100 50" preserveAspectRatio="none""#),
            usvg::Transform::from_row(2.0, 0.0, 0.0, 4.0, -20.0, -80.0)
        );
        assert_eq!(
            transform_for(r#"viewBox="10 20 100 50" preserveAspectRatio="xMaxYMax slice""#),
            usvg::Transform::from_row(4.0, 0.0, 0.0, 4.0, -240.0, -80.0)
        );
        assert_eq!(
            transform_for(
                r#"viewBox="10 20 100 50" width="400" height="100" preserveAspectRatio="xMidYMid""#
            ),
            usvg::Transform::from_row(1.0, 0.0, 0.0, 4.0, 40.0, -80.0)
        );
        assert_eq!(
            transform_for(r#"width="400" height="400""#),
            usvg::Transform::from_scale(0.5, 0.5)
        );
    }

    #[test]
    fn hook_uses_borrowed_nodes_inside_ancestor_transforms_and_can_delegate() {
        let tree = tree(
            r#"<g transform="translate(7 11)"><g transform="scale(2)"><text id="native" x="1" y="20">A</text><text id="default" x="30" y="20">B</text></g></g>"#,
        );
        let usvg::Node::Text(expected) = tree.node_by_id("native").unwrap() else {
            panic!("text node absent");
        };
        let mut document = Document::new();
        let mut page =
            document.start_page_with(PageSettings::new(Size::from_wh(200.0, 200.0).unwrap()));
        let mut surface = page.surface();
        let mut visited = Vec::new();
        surface
            .draw_svg_with_text(
                &tree,
                Size::from_wh(200.0, 200.0).unwrap(),
                SvgSettings::default(),
                &mut |node, surface| {
                    visited.push(node.id().to_owned());
                    assert_eq!(
                        surface.cur_transform(),
                        Transform::from_row(4.0, 0.0, 0.0, 4.0, 14.0, 22.0)
                    );
                    if node.id() == "native" {
                        assert!(std::ptr::eq(node, expected.as_ref()));
                        Ok(true)
                    } else {
                        Ok(false)
                    }
                },
            )
            .unwrap();
        assert_eq!(visited, ["native", "default"]);
        surface.finish();
        page.finish();
        let bytes = document.finish().unwrap();
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        assert_eq!(pdf.extract_text(&[1]).unwrap().trim(), "B");
    }

    #[test]
    fn first_hook_failure_stops_traversal_and_restores_surface_state() {
        let tree = tree(
            r#"<defs><clipPath id="clip"><rect width="90" height="90"/></clipPath></defs><g transform="translate(7 11)" opacity="0.5" clip-path="url(#clip)"><text x="1" y="20">A</text><text x="30" y="20">B</text></g><text x="1" y="80">C</text>"#,
        );
        let mut document = Document::new();
        let mut page =
            document.start_page_with(PageSettings::new(Size::from_wh(100.0, 100.0).unwrap()));
        let mut surface = page.surface();
        surface.push_transform(&Transform::from_translate(3.0, 5.0));
        let before = surface.cur_transform();
        let mut visits = 0;
        assert_eq!(
            surface.draw_svg_with_text(
                &tree,
                Size::from_wh(100.0, 100.0).unwrap(),
                SvgSettings::default(),
                &mut |_, surface| {
                    visits += 1;
                    surface.set_fill(Some(Default::default()));
                    Err("retained glyph rejected".into())
                },
            ),
            Err(SvgError::Text("retained glyph rejected".into()))
        );
        assert_eq!(visits, 1);
        assert_eq!(surface.cur_transform(), before);
        assert!(surface.get_fill().is_none());
        surface.pop();
        surface.finish();
        page.finish();
        document.finish().unwrap();
    }

    #[test]
    fn typed_ink_group_is_intercepted_before_filtered_children_in_draw_order() {
        let tree = tree(
            r#"<text id="before" x="1" y="20">A</text><defs><filter id="unsupported"><feGaussianBlur stdDeviation="2"/></filter></defs><g id="typed-ink" transform="translate(7 11)" opacity="0.5" filter="url(#unsupported)"><text id="child" x="1" y="20">B</text></g><text id="after" x="1" y="80">C</text>"#,
        );
        let mut document = Document::new();
        let mut page =
            document.start_page_with(PageSettings::new(Size::from_wh(200.0, 200.0).unwrap()));
        let mut surface = page.surface();
        let ink_ids = std::collections::HashSet::from(["typed-ink".to_owned()]);
        let order = std::cell::RefCell::new(Vec::new());
        surface
            .draw_svg_with_text_and_ink(
                &tree,
                Size::from_wh(200.0, 200.0).unwrap(),
                SvgSettings {
                    reject_rasterized_filters: true,
                    ..Default::default()
                },
                &mut |text, _| {
                    order.borrow_mut().push(text.id().to_owned());
                    Ok(true)
                },
                &ink_ids,
                &mut |group, surface| {
                    assert_eq!(group.id(), "typed-ink");
                    assert_eq!(group.opacity().get(), 0.5);
                    assert_eq!(
                        group.transform(),
                        usvg::tiny_skia_path::Transform::from_translate(7.0, 11.0)
                    );
                    // Opacity isolates this group into a Form; its transform
                    // is therefore on the Form invocation, outside this local
                    // drawing surface.
                    assert_eq!(surface.cur_transform(), Transform::identity());
                    order.borrow_mut().push("ink".to_owned());
                    Ok(())
                },
            )
            .unwrap();
        assert_eq!(*order.borrow(), ["before", "ink", "after"]);
        surface.finish();
        page.finish();
        document.finish().unwrap();
    }

    #[test]
    fn typed_ink_hook_failure_restores_surface_state() {
        let tree = tree(
            r#"<defs><filter id="unsupported"><feGaussianBlur stdDeviation="2"/></filter></defs><g id="typed-ink" transform="translate(7 11)" opacity="0.5" filter="url(#unsupported)"><text id="child" x="1" y="20">B</text></g>"#,
        );
        let mut document = Document::new();
        let mut page =
            document.start_page_with(PageSettings::new(Size::from_wh(200.0, 200.0).unwrap()));
        let mut surface = page.surface();
        let before = surface.cur_transform();
        let ink_ids = std::collections::HashSet::from(["typed-ink".to_owned()]);
        let result = surface.draw_svg_with_text_and_ink(
            &tree,
            Size::from_wh(200.0, 200.0).unwrap(),
            SvgSettings {
                reject_rasterized_filters: true,
                ..Default::default()
            },
            &mut |_, _| Ok(false),
            &ink_ids,
            &mut |_, _| Err("invalid typed field".into()),
        );
        assert_eq!(result, Err(SvgError::Ink("invalid typed field".into())));
        assert_eq!(surface.cur_transform(), before);
        surface.finish();
        page.finish();
        document.finish().unwrap();
    }

    #[test]
    fn mask_text_uses_the_same_hook_and_failure_prevents_body_text() {
        let tree = tree(
            r#"<defs><mask id="mask"><text id="mask-text" fill="white" x="0" y="20">A</text></mask></defs><g mask="url(#mask)"><text id="body-text" x="0" y="20">B</text></g>"#,
        );
        let mut document = Document::new();
        let mut page =
            document.start_page_with(PageSettings::new(Size::from_wh(100.0, 100.0).unwrap()));
        let mut surface = page.surface();
        let mut visited = Vec::new();
        let result = surface.draw_svg_with_text(
            &tree,
            Size::from_wh(100.0, 100.0).unwrap(),
            SvgSettings::default(),
            &mut |node, _| {
                visited.push(node.id().to_owned());
                Err("mask failure".into())
            },
        );
        assert_eq!(result, Err(SvgError::Text("mask failure".into())));
        assert_eq!(visited, ["mask-text"]);
        assert_eq!(surface.cur_transform(), Transform::identity());
        surface.finish();
        page.finish();
        document.finish().unwrap();
    }
    #[test]
    fn unsupported_filter_fails_vector_export_but_keeps_svg_compatibility_fallback() {
        let tree = tree(
            r#"<defs><filter id="near-miss" color-interpolation-filters="sRGB"><feColorMatrix type="matrix" values="0 0 0 0 0.1 0 0 0 0 0.2 0 0 0 0 0.3 0 1 0 0 0"/></filter></defs><g transform="translate(7 11)" opacity="0.5"><g filter="url(#near-miss)"><rect width="20" height="20" fill="white"/></g><text x="0" y="20">A</text></g>"#,
        );
        for strict in [false, true] {
            let mut document = Document::new();
            let mut page =
                document.start_page_with(PageSettings::new(Size::from_wh(100.0, 100.0).unwrap()));
            let mut surface = page.surface();
            surface.push_transform(&Transform::from_translate(3.0, 5.0));
            let before = surface.cur_transform();
            let mut visits = 0;
            let result = surface.draw_svg_with_text(
                &tree,
                Size::from_wh(100.0, 100.0).unwrap(),
                SvgSettings {
                    reject_rasterized_filters: strict,
                    ..Default::default()
                },
                &mut |_, _| {
                    visits += 1;
                    Ok(true)
                },
            );
            if strict {
                assert!(matches!(result, Err(SvgError::Filter(_))), "{result:?}");
                assert_eq!(visits, 0);
            } else {
                assert_eq!(result, Ok(()));
                assert_eq!(visits, 1);
            }
            assert_eq!(surface.cur_transform(), before);
            assert!(surface.get_fill().is_none());
            assert!(surface.get_stroke().is_none());
            surface.pop();
            surface.finish();
            page.finish();
            let bytes = document.finish().unwrap();
            let pdf = lopdf::Document::load_mem(&bytes).unwrap();
            let images = pdf
                .objects
                .values()
                .filter_map(|object| match object {
                    lopdf::Object::Dictionary(dict) => Some(dict),
                    lopdf::Object::Stream(stream) => Some(&stream.dict),
                    _ => None,
                })
                .filter(|dict| {
                    dict.get(b"Subtype")
                        .and_then(lopdf::Object::as_name)
                        .is_ok_and(|name| name == b"Image")
                })
                .count();
            assert_eq!(images > 0, !strict);
        }
    }

    #[test]
    fn rejected_image_stops_native_traversal_and_restores_surface_state() {
        use base64::Engine;

        let header = [
            0xff, 0xd8, 0xff, 0xc0, 0, 17, 8, 0, 1, 0, 1, 3, 1, 0x11, 0, 2, 0x11, 0, 3, 0x11, 0,
        ];
        let data = base64::engine::general_purpose::STANDARD.encode(header);
        let tree = tree(&format!(
            r#"<g transform="translate(7 11)" opacity="0.5"><image id="broken" width="10" height="10" href="data:image/jpeg;base64,{data}"/><text x="0" y="20">A</text></g>"#
        ));
        let Some(usvg::Node::Group(group)) = tree.node_by_id("broken") else {
            panic!("image placement group absent");
        };
        let [usvg::Node::Image(image)] = group.children() else {
            panic!("image absent from placement group");
        };
        assert_eq!(image.size().width(), 1.0);
        assert_eq!(image.size().height(), 1.0);
        assert!(matches!(image.kind(), usvg::ImageKind::JPEG(bytes) if bytes.as_slice() == header));
        for reject_images in [false, true] {
            let mut document = Document::new();
            let mut page =
                document.start_page_with(PageSettings::new(Size::from_wh(100.0, 100.0).unwrap()));
            let mut surface = page.surface();
            surface.push_transform(&Transform::from_translate(3.0, 5.0));
            let before = surface.cur_transform();
            let mut visits = 0;
            let result = surface.draw_svg_with_text(
                &tree,
                Size::from_wh(100.0, 100.0).unwrap(),
                SvgSettings {
                    reject_images,
                    ..Default::default()
                },
                &mut |_, _| {
                    visits += 1;
                    Ok(true)
                },
            );
            if reject_images {
                assert!(matches!(result, Err(SvgError::Image(_))), "{result:?}");
                assert_eq!(visits, 0);
            } else {
                assert_eq!(result, Ok(()));
                assert_eq!(visits, 1);
            }
            assert_eq!(surface.cur_transform(), before);
            assert!(surface.get_fill().is_none());
            assert!(surface.get_stroke().is_none());
            surface.pop();
            surface.finish();
            page.finish();
            document.finish().unwrap();
        }
    }

    #[test]
    fn offscreen_gradient_does_not_add_pdf_paint_or_resources() {
        fn export(content: &str) -> lopdf::Document {
            let tree = tree(content);
            let mut document = Document::new();
            let mut page =
                document.start_page_with(PageSettings::new(Size::from_wh(100.0, 100.0).unwrap()));
            let mut surface = page.surface();
            surface
                .draw_svg(
                    &tree,
                    Size::from_wh(100.0, 100.0).unwrap(),
                    SvgSettings::default(),
                )
                .unwrap();
            surface.finish();
            page.finish();
            lopdf::Document::load_mem(&document.finish().unwrap()).unwrap()
        }

        let visible = r##"<rect x="10" y="10" width="30" height="40" fill="#7a38c1"/>"##;
        let baseline = export(visible);
        let candidate = export(&format!(
            r#"<defs><linearGradient id="unused"><stop stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient></defs>{visible}<g transform="translate(0 200)"><circle cx="30" cy="30" r="20" fill="url(#unused)"/></g>"#
        ));
        let baseline_page = baseline.get_pages()[&1];
        let candidate_page = candidate.get_pages()[&1];
        assert_eq!(
            candidate.get_page_content(candidate_page).unwrap(),
            baseline.get_page_content(baseline_page).unwrap()
        );
        assert_eq!(candidate.objects.len(), baseline.objects.len());
    }

    #[test]
    fn transformed_use_image_uses_paint_bounds_for_culling() {
        use base64::Engine;

        fn image_tree(transform: &str) -> Tree {
            let png = include_bytes!("../../../tests/assets/crop-quadrants.png");
            let href = base64::engine::general_purpose::STANDARD.encode(png);
            Tree::from_str(
                &format!(
                    r##"<svg xmlns="http://www.w3.org/2000/svg" width="256" height="256" viewBox="0 0 256 256"><defs><image id="pattern" x="-1.572" y="-1.572" width="3.144" height="3.144" preserveAspectRatio="none" href="data:image/png;base64,{href}"/></defs><use id="stamp" href="#pattern" opacity="0.1960784" transform="{transform}"/></svg>"##
                ),
                &usvg::Options::default(),
            )
            .unwrap()
        }

        fn image_count(pdf: &lopdf::Document) -> usize {
            pdf.objects
                .values()
                .filter_map(|object| match object {
                    lopdf::Object::Dictionary(dict) => Some(dict),
                    lopdf::Object::Stream(stream) => Some(&stream.dict),
                    _ => None,
                })
                .filter(|dict| {
                    dict.get(b"Subtype")
                        .and_then(lopdf::Object::as_name)
                        .is_ok_and(|name| name == b"Image")
                })
                .count()
        }

        let visible =
            image_tree("matrix(-0.9961947,0.087155715,-0.087155715,-0.9961947,5.704998,252.29625)");
        let Some(node) = visible.node_by_id("stamp") else {
            panic!("visible use node absent");
        };
        let mut db = visible.fontdb().clone();
        let mut context = ProcessContext::new(Arc::make_mut(&mut db), SvgSettings::default());
        context.set_cull_viewport(
            visible.root(),
            usvg::Rect::from_xywh(0.0, 0.0, 256.0, 256.0).unwrap(),
        );
        assert!(!context.should_cull_node(node));

        let mut document = Document::new();
        let mut page =
            document.start_page_with(PageSettings::new(Size::from_wh(256.0, 256.0).unwrap()));
        let mut surface = page.surface();
        surface
            .draw_svg(
                &visible,
                Size::from_wh(256.0, 256.0).unwrap(),
                SvgSettings::default(),
            )
            .unwrap();
        surface.finish();
        page.finish();
        let visible_pdf = lopdf::Document::load_mem(&document.finish().unwrap()).unwrap();
        assert!(
            image_count(&visible_pdf) > 0,
            "visible use image was culled from PDF"
        );
        let visible_page = visible_pdf.get_pages()[&1];
        let visible_content = visible_pdf.get_page_content(visible_page).unwrap();
        assert!(
            visible_content.windows(3).any(|bytes| bytes == b"Do ")
                || visible_content.windows(2).any(|bytes| bytes == b"Do")
        );

        let offscreen = image_tree("translate(400 400)");
        let Some(node) = offscreen.node_by_id("stamp") else {
            panic!("offscreen use node absent");
        };
        let mut db = offscreen.fontdb().clone();
        let mut context = ProcessContext::new(Arc::make_mut(&mut db), SvgSettings::default());
        context.set_cull_viewport(
            offscreen.root(),
            usvg::Rect::from_xywh(0.0, 0.0, 256.0, 256.0).unwrap(),
        );
        assert!(context.should_cull_node(node));

        let mut document = Document::new();
        let mut page =
            document.start_page_with(PageSettings::new(Size::from_wh(256.0, 256.0).unwrap()));
        let mut surface = page.surface();
        surface
            .draw_svg(
                &offscreen,
                Size::from_wh(256.0, 256.0).unwrap(),
                SvgSettings::default(),
            )
            .unwrap();
        surface.finish();
        page.finish();
        let offscreen_pdf = lopdf::Document::load_mem(&document.finish().unwrap()).unwrap();
        assert_eq!(
            image_count(&offscreen_pdf),
            0,
            "offscreen use image emitted PDF resources"
        );
    }

    #[test]
    fn offscreen_rejected_image_is_still_validated() {
        use base64::Engine;

        let header = [
            0xff, 0xd8, 0xff, 0xc0, 0, 17, 8, 0, 1, 0, 1, 3, 1, 0x11, 0, 2, 0x11, 0, 3, 0x11, 0,
        ];
        let data = base64::engine::general_purpose::STANDARD.encode(header);
        let tree = tree(&format!(
            r#"<g transform="translate(0 200)"><image id="offscreen-broken" width="10" height="10" href="data:image/jpeg;base64,{data}"/></g>"#
        ));
        let mut document = Document::new();
        let mut page =
            document.start_page_with(PageSettings::new(Size::from_wh(100.0, 100.0).unwrap()));
        let mut surface = page.surface();
        let result = surface.draw_svg_with_text(
            &tree,
            Size::from_wh(100.0, 100.0).unwrap(),
            SvgSettings {
                reject_images: true,
                ..Default::default()
            },
            &mut |_, _| Ok(false),
        );
        assert!(matches!(result, Err(SvgError::Image(_))), "{result:?}");
        surface.finish();
        page.finish();
        document.finish().unwrap();
    }

    #[test]
    fn offscreen_pattern_text_keeps_strict_filter_validation_without_a_hook() {
        let tree = tree(
            r#"<defs><filter id="blur"><feGaussianBlur stdDeviation="1"/></filter><pattern id="pattern" patternUnits="userSpaceOnUse" width="10" height="10"><g filter="url(#blur)"><rect width="10" height="10" fill="red"/></g></pattern></defs><text id="hidden" x="10" y="220" fill="url(#pattern)">A</text>"#,
        );
        assert!(matches!(
            tree.node_by_id("hidden"),
            Some(usvg::Node::Text(_))
        ));
        let mut document = Document::new();
        let mut page =
            document.start_page_with(PageSettings::new(Size::from_wh(100.0, 100.0).unwrap()));
        let mut surface = page.surface();
        let result = surface.draw_svg(
            &tree,
            Size::from_wh(100.0, 100.0).unwrap(),
            SvgSettings {
                reject_rasterized_filters: true,
                ..Default::default()
            },
        );
        assert!(result.is_none());
        surface.finish();
        page.finish();
        document.finish().unwrap();
    }

    #[test]
    fn offscreen_unsupported_filter_is_still_validated() {
        let tree = tree(
            r#"<defs><filter id="near-miss" color-interpolation-filters="sRGB"><feColorMatrix type="matrix" values="0 0 0 0 0.1 0 0 0 0 0.2 0 0 0 0 0.3 0 1 0 0 0"/></filter></defs><g transform="translate(0 200)"><g filter="url(#near-miss)"><rect width="20" height="20" fill="white"/></g></g>"#,
        );
        let mut document = Document::new();
        let mut page =
            document.start_page_with(PageSettings::new(Size::from_wh(100.0, 100.0).unwrap()));
        let mut surface = page.surface();
        let result = surface.draw_svg_with_text(
            &tree,
            Size::from_wh(100.0, 100.0).unwrap(),
            SvgSettings {
                reject_rasterized_filters: true,
                ..Default::default()
            },
            &mut |_, _| Ok(false),
        );
        assert!(matches!(result, Err(SvgError::Filter(_))), "{result:?}");
        surface.finish();
        page.finish();
        document.finish().unwrap();
    }

    #[test]
    fn filtered_group_whose_region_meets_viewport_is_retained() {
        let tree = tree(
            r#"<defs><filter id="offset" filterUnits="userSpaceOnUse" x="0" y="80" width="100" height="40"><feOffset dy="-20"/></filter></defs><g id="effect" filter="url(#offset)"><rect x="10" y="110" width="20" height="10" fill="black"/></g>"#,
        );
        let Some(node @ usvg::Node::Group(effect)) = tree.node_by_id("effect") else {
            panic!("filtered group absent");
        };
        assert!(effect.abs_layer_bounding_box().top() <= 100.0);
        assert!(effect.abs_layer_bounding_box().bottom() >= 100.0);

        let mut db = tree.fontdb().clone();
        let mut context = ProcessContext::new(Arc::make_mut(&mut db), SvgSettings::default());
        context.set_cull_viewport(
            tree.root(),
            usvg::Rect::from_xywh(0.0, 0.0, 100.0, 100.0).unwrap(),
        );
        assert!(!context.should_cull_node(node));
    }
}
