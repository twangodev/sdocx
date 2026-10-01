//! Bundled krilla-svg 0.8.1 converter; provenance and licenses are recorded beside this module.

use std::io::Read;
use std::sync::Arc;

use fontdb::Database;
use krilla::color::rgb;
use krilla::geom::{Rect, Size, Transform};
use krilla::paint::FillRule;
use krilla::surface::Surface;
use krilla::text::GlyphId;
use usvg::{Node, Tree, fontdb, roxmltree};

use self::text::Fonts;
use self::util::RectExt;

mod clip_path;
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
}

impl Default for SvgSettings {
    fn default() -> Self {
        Self {
            embed_text: true,
            filter_scale: 4.0,
        }
    }
}

/// An extension trait for the `Surface` struct that allows you to draw SVGs onto a surface.
pub(super) trait SurfaceExt {
    /// Draw a `usvg` tree onto a surface with the given size and settings.
    fn draw_svg(&mut self, tree: &Tree, size: Size, svg_settings: SvgSettings) -> Option<()>;

    fn draw_svg_with_text(
        &mut self,
        tree: &Tree,
        size: Size,
        svg_settings: SvgSettings,
        hook: &mut TextHook<'_>,
    ) -> Result<(), String>;
}

pub(super) type TextHook<'a> =
    dyn FnMut(&usvg::Text, &mut Surface<'_>) -> Result<bool, String> + 'a;

impl SurfaceExt for Surface<'_> {
    fn draw_svg(&mut self, tree: &Tree, size: Size, svg_settings: SvgSettings) -> Option<()> {
        draw_svg(self, tree, size, svg_settings, None).ok()
    }

    fn draw_svg_with_text(
        &mut self,
        tree: &Tree,
        size: Size,
        svg_settings: SvgSettings,
        hook: &mut TextHook<'_>,
    ) -> Result<(), String> {
        draw_svg(self, tree, size, svg_settings, Some(hook))
    }
}

fn draw_svg(
    surface: &mut Surface<'_>,
    tree: &Tree,
    size: Size,
    svg_settings: SvgSettings,
    hook: Option<&mut TextHook<'_>>,
) -> Result<(), String> {
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
    let result = render_tree_with_text(tree, svg_settings, surface, hook);
    surface.pop();
    surface.pop();

    surface.set_fill(old_fill);
    surface.set_stroke(old_stroke);

    result
}

struct ProcessContext<'a, 'h, 'o> {
    pub(super) fonts: Fonts<'a>,
    svg_settings: SvgSettings,
    text_hook: Option<&'h mut TextHook<'o>>,
    error: Option<String>,
}

impl<'a, 'h, 'o> ProcessContext<'a, 'h, 'o> {
    fn new(tree_fontdb: &'a mut Database, svg_settings: SvgSettings) -> Self {
        Self {
            fonts: Fonts::new(tree_fontdb),
            svg_settings,
            text_hook: None,
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
            Some(Err(error)) => self.error = Some(error),
            None | Some(Ok(false)) => text::render(node, surface, self),
        }
    }
}

pub(super) fn render_tree(tree: &Tree, svg_settings: SvgSettings, surface: &mut Surface) {
    let _ = render_tree_with_text(tree, svg_settings, surface, None);
}

fn render_tree_with_text(
    tree: &Tree,
    svg_settings: SvgSettings,
    surface: &mut Surface<'_>,
    hook: Option<&mut TextHook<'_>>,
) -> Result<(), String> {
    let mut db = tree.fontdb().clone();
    let mut fc = ProcessContext::new(Arc::make_mut(&mut db), svg_settings);
    fc.text_hook = hook;
    group::render(tree.root(), surface, &mut fc);
    fc.error.map_or(Ok(()), Err)
}

pub(super) fn render_node(
    node: &Node,
    mut tree_fontdb: Arc<Database>,
    svg_settings: SvgSettings,
    surface: &mut Surface,
) {
    let mut fc = ProcessContext::new(Arc::make_mut(&mut tree_fontdb), svg_settings);
    group::render_node(node, surface, &mut fc);
}

/// Render an SVG glyph from an OpenType font into a surface. You can plug this method into the
/// `render_svg_glyph_fn` field of `SerializeSettings` in krilla..
#[allow(dead_code)]
pub(super) fn render_svg_glyph(
    data: &[u8],
    context_color: rgb::Color,
    glyph: GlyphId,
    default_size: (f32, f32),
    surface: &mut Surface,
) -> Option<()> {
    let mut data = data;
    let settings = SvgSettings::default();

    let default_size = usvg::Size::from_wh(default_size.0, default_size.1).unwrap();

    let mut decoded = vec![];
    if data.starts_with(&[0x1f, 0x8b]) {
        let mut decoder = flate2::read::GzDecoder::new(data);
        decoder.read_to_end(&mut decoded).ok()?;
        data = &decoded;
    }

    let xml = std::str::from_utf8(data).ok()?;
    // Incredibly hacky, but hopefully that's enough for SVG glyphs.
    let has_viewbox = xml.contains("viewBox");
    let document = roxmltree::Document::parse(xml).ok()?;

    // Reparsing every time might be pretty slow in some cases, because Noto Color Emoji
    // for example contains hundreds of glyphs in the same SVG document, meaning that we have
    // to reparse it every time. However, Twitter Color Emoji does have each glyph in a
    // separate SVG document, and since we use COLRv1 for Noto Color Emoji anyway, this is
    // good enough.
    let opts = usvg::Options {
        style_sheet: Some(format!(
            "svg {{ color: rgb({}, {}, {}) }}",
            context_color.red(),
            context_color.green(),
            context_color.blue()
        )),
        default_size,
        ..Default::default()
    };
    let tree = Tree::from_xmltree(&document, &opts).ok()?;

    let apply_scale = default_size != tree.size() && has_viewbox;

    // From the specification:
    //
    // The size of the initial viewport for the SVG document is the em square:
    // height and width both equal to head.unitsPerEm. If a viewBox
    // attribute is specified on the <svg> element with width or
    // height values different from the unitsPerEm value,
    // this will have the effect of a scale transformation on the SVG “user” coordinate
    // system.
    if apply_scale {
        let scale = (default_size.width() / tree.size().width())
            .min(default_size.height() / tree.size().height());
        surface.push_transform(&Transform::from_scale(scale, scale))
    }

    if let Some(node) = tree.node_by_id(&format!("glyph{}", glyph.to_u32())) {
        render_node(node, tree.fontdb().clone(), settings, surface)
    } else {
        // Twitter Color Emoji SVGs contain the glyph ID on the root element, which isn't saved by
        // usvg. So in this case, we simply draw the whole document.
        render_tree(&tree, settings, surface)
    };

    if apply_scale {
        surface.pop();
    }

    Some(())
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
            Err("retained glyph rejected".into())
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
        assert_eq!(result, Err("mask failure".into()));
        assert_eq!(visited, ["mask-text"]);
        assert_eq!(surface.cur_transform(), Transform::identity());
        surface.finish();
        page.finish();
        document.finish().unwrap();
    }
}
