//! Conservative vector lowering for grayscale source-image colorizer filters.
//!
//! This handles the single sRGB color matrix used by the FountainPen field
//! renderer. The source graph is retained as PDF vector content inside a
//! luminosity mask; unsupported SVG contexts remain for the generic filter
//! raster fallback.

use krilla::color::rgb;
use krilla::geom::Rect;
use krilla::mask::{Mask, MaskType};
use krilla::num::NormalizedF32;
use krilla::paint::{Fill, FillRule};
use krilla::surface::Surface;
use usvg::filter::{ColorInterpolation, ColorMatrixKind, Input, Kind};
use usvg::{BlendMode, Color, Node, Paint};

use super::util::{RectExt, UsvgTransformExt};
use super::{ProcessContext, group};

/// Lower the recognized source-graphic colorizer directly to vector PDF.
///
/// Returns `false` when any part of the filter or source graph falls outside
/// the structurally verified subset, allowing the normal filter renderer to
/// handle it. Successful lowering preserves the source paths, radial/linear
/// shadings, child Lighten groups, filtered group transform, filter-region
/// clip, and the filtered group's opacity.
pub(super) fn try_render(
    source_group: &usvg::Group,
    surface: &mut Surface<'_>,
    process_context: &mut ProcessContext<'_, '_, '_>,
) -> bool {
    let Some((filter_rect, source, color, opacity)) = recognize(source_group) else {
        return false;
    };

    let mut stream_builder = surface.stream_builder();
    let mut mask_surface = stream_builder.surface();
    mask_surface.push_clip_path(&filter_rect.to_clip_path(), &FillRule::NonZero);
    group::render(source, &mut mask_surface, process_context);
    mask_surface.pop();
    mask_surface.finish();
    if process_context.error.is_some() {
        return false;
    }
    let mask_stream = stream_builder.finish();

    let old_fill = surface.get_fill().cloned();
    let old_stroke = surface.get_stroke().cloned();

    // A filter result is one composited object. The mask contents retain the
    // original Lighten groups; filtered-group opacity is applied once outside
    // that result, as required by SVG group compositing.
    surface.push_isolated();
    surface.push_transform(&source_group.transform().to_krilla());
    surface.push_clip_path(&filter_rect.to_clip_path(), &FillRule::NonZero);
    surface.push_opacity(NormalizedF32::new(opacity).unwrap());
    surface.push_mask(Mask::new(mask_stream, MaskType::Luminosity));
    surface.set_fill(Some(Fill {
        paint: rgb::Color::new(color[0], color[1], color[2]).into(),
        opacity: NormalizedF32::ONE,
        rule: FillRule::NonZero,
    }));
    surface.draw_path(&filter_rect.to_clip_path());
    surface.pop(); // mask
    surface.pop(); // filtered group opacity
    surface.pop(); // filter region clip
    surface.pop(); // filtered group transform
    surface.pop(); // isolated filter result
    surface.set_fill(old_fill);
    surface.set_stroke(old_stroke);
    true
}

fn recognize(group: &usvg::Group) -> Option<(Rect, &usvg::Group, [u8; 3], f32)> {
    if group.filters().len() != 1
        || group.clip_path().is_some()
        || group.mask().is_some()
        || group.blend_mode() != BlendMode::Normal
        || !group.opacity().get().is_finite()
    {
        return None;
    }

    let filter = &group.filters()[0];
    if filter.primitives().len() != 1 {
        return None;
    }
    let primitive = &filter.primitives()[0];
    if primitive.color_interpolation() != ColorInterpolation::SRGB {
        return None;
    }
    let Kind::ColorMatrix(matrix) = primitive.kind() else {
        return None;
    };
    if matrix.input() != &Input::SourceGraphic {
        return None;
    }
    let ColorMatrixKind::Matrix(values) = matrix.kind() else {
        return None;
    };
    let color = recognize_color_matrix(values)?;
    let filter_rect = filter.rect();
    let primitive_rect = primitive.rect();
    if filter_rect != primitive_rect {
        return None;
    }
    let filter_rect = Rect::from_ltrb(
        filter_rect.left(),
        filter_rect.top(),
        filter_rect.right(),
        filter_rect.bottom(),
    )?;

    // The accepted source has one isolated source group. Its first child is an
    // opaque black rectangle covering the complete filter region. This makes
    // source alpha opaque everywhere in the filter region, so the source
    // graphic's gray RGB is also the luminosity mask value.
    if group.children().len() != 1 {
        return None;
    }
    let Node::Group(source) = &group.children()[0] else {
        return None;
    };
    if source.transform() != usvg::Transform::identity()
        || source.opacity() != usvg::Opacity::ONE
        || source.blend_mode() != BlendMode::Normal
        || !source.isolate()
        || source.clip_path().is_some()
        || source.mask().is_some()
        || !source.filters().is_empty()
        || source.children().len() < 2
    {
        return None;
    }

    let Node::Path(backdrop) = &source.children()[0] else {
        return None;
    };
    if backdrop.abs_transform() != source.abs_transform()
        || !is_opaque_black_backdrop(backdrop, filter_rect)
    {
        return None;
    }
    if !source.children()[1..].iter().all(is_gray_source_node) {
        return None;
    }

    Some((filter_rect, source, color, group.opacity().get()))
}

fn recognize_color_matrix(values: &[f32]) -> Option<[u8; 3]> {
    if values.len() != 20 || values.iter().any(|value| !value.is_finite()) {
        return None;
    }
    let is_zero = |value: f32| value == 0.0;
    let is_one = |value: f32| value == 1.0;
    for row in 0..3 {
        let start = row * 5;
        if !values[start..start + 4].iter().all(|value| is_zero(*value)) {
            return None;
        }
    }
    if !is_one(values[15]) || !values[16..20].iter().all(|value| is_zero(*value)) {
        return None;
    }

    let mut color = [0; 3];
    for (index, component) in color.iter_mut().enumerate() {
        let value = values[index * 5 + 4];
        if !(0.0..=1.0).contains(&value) {
            return None;
        }
        let quantized = (value * 255.0).round();
        if value != quantized / 255.0 {
            return None;
        }
        *component = quantized as u8;
    }
    Some(color)
}

fn is_opaque_black_backdrop(path: &usvg::Path, rect: Rect) -> bool {
    if !path.is_visible() || path.stroke().is_some() {
        return false;
    }
    let Some(fill) = path.fill() else {
        return false;
    };
    if fill.opacity() != usvg::Opacity::ONE
        || !matches!(
            fill.paint(),
            Paint::Color(Color {
                red: 0,
                green: 0,
                blue: 0
            })
        )
    {
        return false;
    }

    let mut segments = path.data().segments();
    let Some(usvg::tiny_skia_path::PathSegment::MoveTo(p0)) = segments.next() else {
        return false;
    };
    let Some(usvg::tiny_skia_path::PathSegment::LineTo(p1)) = segments.next() else {
        return false;
    };
    let Some(usvg::tiny_skia_path::PathSegment::LineTo(p2)) = segments.next() else {
        return false;
    };
    let Some(usvg::tiny_skia_path::PathSegment::LineTo(p3)) = segments.next() else {
        return false;
    };
    if !matches!(
        segments.next(),
        Some(usvg::tiny_skia_path::PathSegment::Close)
    ) || segments.next().is_some()
    {
        return false;
    }

    let points = [(p0.x, p0.y), (p1.x, p1.y), (p2.x, p2.y), (p3.x, p3.y)];
    let expected = [
        (rect.left(), rect.top()),
        (rect.right(), rect.top()),
        (rect.right(), rect.bottom()),
        (rect.left(), rect.bottom()),
    ];
    points == expected
}

fn is_gray_source_node(node: &Node) -> bool {
    match node {
        Node::Path(path) => {
            if !path.is_visible() {
                return true;
            }
            path.stroke().is_none() && path.fill().is_some_and(|fill| is_gray_paint(fill.paint()))
        }
        Node::Group(group) => {
            (group.blend_mode() == BlendMode::Normal || group.blend_mode() == BlendMode::Lighten)
                && group.clip_path().is_none()
                && group.mask().is_none()
                && group.filters().is_empty()
                && group.children().iter().all(is_gray_source_node)
        }
        Node::Image(_) | Node::Text(_) => false,
    }
}

fn is_gray_paint(paint: &Paint) -> bool {
    match paint {
        Paint::Color(color) => color.red == color.green && color.green == color.blue,
        Paint::LinearGradient(gradient) => {
            gradient.spread_method() == usvg::SpreadMethod::Pad
                && !gradient.stops().is_empty()
                && gradient
                    .stops()
                    .iter()
                    .all(|stop| is_gray_color(stop.color()))
        }
        Paint::RadialGradient(gradient) => {
            gradient.spread_method() == usvg::SpreadMethod::Pad
                && !gradient.stops().is_empty()
                && gradient
                    .stops()
                    .iter()
                    .all(|stop| is_gray_color(stop.color()))
        }
        Paint::Pattern(_) => false,
    }
}

fn is_gray_color(color: Color) -> bool {
    color.red == color.green && color.green == color.blue
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use fontdb::Database;
    use krilla::{Document, page::PageSettings};
    use lopdf::{Document as PdfDocument, Object};
    use usvg::Tree;

    use super::*;
    use crate::pdf::svg::{ProcessContext, SvgSettings};

    const FILTER_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64" viewBox="0 0 64 64"><defs>
      <radialGradient id="g0" gradientUnits="userSpaceOnUse" cx="0" cy="0" r="1"><stop offset="0" stop-color="white" stop-opacity="0.2"/><stop offset="1" stop-color="white" stop-opacity="0"/></radialGradient>
      <radialGradient id="g1" gradientUnits="userSpaceOnUse" cx="0" cy="0" r="1"><stop offset="0" stop-color="white" stop-opacity="0.6"/><stop offset="1" stop-color="white" stop-opacity="0"/></radialGradient>
      <radialGradient id="g2" gradientUnits="userSpaceOnUse" cx="0" cy="0" r="1"><stop offset="0" stop-color="white" stop-opacity="1"/><stop offset="0.0909090909" stop-color="white" stop-opacity="1"/><stop offset="1" stop-color="white" stop-opacity="0"/></radialGradient>
      <filter id="ink" filterUnits="userSpaceOnUse" x="0" y="0" width="64" height="64" color-interpolation-filters="sRGB"><feColorMatrix type="matrix" values="0 0 0 0 0.101960784 0 0 0 0 0.101960784 0 0 0 0 0.101960784 1 0 0 0 0"/></filter>
    </defs><rect width="64" height="64" fill="white"/><g filter="url(#ink)" opacity="1"><g style="isolation:isolate"><rect width="64" height="64" fill="black"/>
      <g style="mix-blend-mode:lighten;isolation:isolate" transform="matrix(1,0,0,1,32.25,32.25)"><circle r="1" fill="black"/><circle r="1" fill="url(#g0)"/></g>
      <g style="mix-blend-mode:lighten;isolation:isolate" transform="matrix(1,0,0,1,32.55,32.25)"><circle r="1" fill="black"/><circle r="1" fill="url(#g1)"/></g>
      <g style="mix-blend-mode:lighten;isolation:isolate" transform="matrix(1.1,0,0,1.1,32.4,32.4)"><circle r="1" fill="black"/><circle r="1" fill="url(#g2)"/></g>
    </g></g></svg>"#;

    fn tree(svg: &str) -> Tree {
        let mut fontdb = Database::new();
        fontdb.load_font_data(include_bytes!("../../../assets/fonts/Roboto-Regular.ttf").to_vec());
        Tree::from_str(
            svg,
            &usvg::Options {
                fontdb: Arc::new(fontdb),
                ..Default::default()
            },
        )
        .unwrap()
    }

    fn ink_group(tree: &Tree) -> &usvg::Group {
        fn find(group: &usvg::Group) -> Option<&usvg::Group> {
            if !group.filters().is_empty() {
                return Some(group);
            }
            group.children().iter().find_map(|node| match node {
                Node::Group(child) => find(child),
                _ => None,
            })
        }
        find(tree.root()).expect("filtered group missing")
    }

    #[test]
    fn lowers_fountain_filter_to_luminosity_mask_and_vector_shadings() {
        let tree = tree(FILTER_SVG);
        let ink = ink_group(&tree);
        let mut document = Document::new();
        let mut page = document.start_page_with(PageSettings::new(
            krilla::geom::Size::from_wh(64.0, 64.0).unwrap(),
        ));
        let mut surface = page.surface();
        let mut fonts = tree.fontdb().clone();
        let mut process_context =
            ProcessContext::new(Arc::make_mut(&mut fonts), SvgSettings::default());

        assert!(try_render(ink, &mut surface, &mut process_context));
        surface.finish();
        page.finish();
        let bytes = document.finish().unwrap();
        let pdf = PdfDocument::load_mem(&bytes).unwrap();

        let image_xobjects = pdf
            .objects
            .values()
            .filter_map(|object| match object {
                Object::Dictionary(dict) => Some(dict),
                Object::Stream(stream) => Some(&stream.dict),
                _ => None,
            })
            .filter(|dict| {
                dict.get(b"Subtype")
                    .and_then(Object::as_name)
                    .is_ok_and(|name| name == b"Image")
            })
            .count();
        assert_eq!(image_xobjects, 0);
        assert!(
            bytes
                .windows(b"/Luminosity".len())
                .any(|w| w == b"/Luminosity")
        );
        assert!(
            bytes
                .windows(b"/ShadingType".len())
                .any(|w| w == b"/ShadingType")
        );
        assert!(bytes.windows(b"/Lighten".len()).any(|w| w == b"/Lighten"));
    }

    #[test]
    fn rejects_non_colorizer_filters_and_unsafe_source_graphs() {
        let source = FILTER_SVG;
        let non_srgb = source.replace(
            "color-interpolation-filters=\"sRGB\"",
            "color-interpolation-filters=\"linearRGB\"",
        );
        let two_primitives =
            source.replace("</filter>", "<feGaussianBlur stdDeviation=\"1\"/></filter>");
        let image = source.replace(
            "<g style=\"mix-blend-mode:lighten;isolation:isolate\"",
            "<image width=\"4\" height=\"3\" href=\"data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAQAAAADCAYAAAC09K7GAAAAGElEQVR4nGP4z8DwH4SRIJoAlA8EDGAMAGPrGef+nm3iAAAAAElFTkSuQmCC\"/><g style=\"mix-blend-mode:lighten;isolation:isolate\"",
        );
        let same_bounds_nonrectangle = source.replace(
            "<rect width=\"64\" height=\"64\" fill=\"black\"/>",
            "<path d=\"M0 0H64V64H4Z\" fill=\"black\"/>",
        );
        let shifted_backdrop = source.replace(
            "<rect width=\"64\" height=\"64\" fill=\"black\"/>",
            "<rect width=\"64\" height=\"64\" fill=\"black\" transform=\"translate(1,0)\"/>",
        );
        let image_tree = tree(&image);
        fn has_image(group: &usvg::Group) -> bool {
            group.children().iter().any(|node| match node {
                Node::Image(_) => true,
                Node::Group(group) => has_image(group),
                _ => false,
            })
        }
        assert!(
            has_image(image_tree.root()),
            "test image must be retained by usvg"
        );
        assert!(recognize(ink_group(&image_tree)).is_none());

        for svg in [
            non_srgb,
            two_primitives,
            same_bounds_nonrectangle,
            shifted_backdrop,
        ] {
            let tree = tree(&svg);
            assert!(recognize(ink_group(&tree)).is_none());
        }
    }

    #[test]
    fn color_matrix_requires_exact_source_red_alpha_contract() {
        let values = [
            0.0,
            0.0,
            0.0,
            0.0,
            0.101960786,
            0.0,
            0.0,
            0.0,
            0.0,
            0.101960786,
            0.0,
            0.0,
            0.0,
            0.0,
            0.101960786,
            1.0,
            0.0,
            0.0,
            0.0,
            0.0,
        ];
        assert_eq!(recognize_color_matrix(&values), Some([26, 26, 26]));
        let mut wrong_alpha = values;
        wrong_alpha[16] = 1.0;
        assert_eq!(recognize_color_matrix(&wrong_alpha), None);
        let mut nonconstant_rgb = values;
        nonconstant_rgb[1] = 0.25;
        assert_eq!(recognize_color_matrix(&nonconstant_rgb), None);
    }

    #[test]
    fn admits_outer_transform_and_saved_opacity_for_single_application() {
        let transformed = FILTER_SVG.replace(
            "<g filter=\"url(#ink)\" opacity=\"1\">",
            "<g filter=\"url(#ink)\" opacity=\"0.5\" transform=\"matrix(1,0,0,1,3,4)\">",
        );
        let tree = tree(&transformed);
        let group = ink_group(&tree);
        let (_, _, _, opacity) = recognize(group).expect("candidate should be admitted");
        assert_eq!(opacity, 0.5);
        assert_eq!(group.transform().tx, 3.0);
        assert_eq!(group.transform().ty, 4.0);

        let mut document = Document::new();
        let mut page = document.start_page_with(PageSettings::new(
            krilla::geom::Size::from_wh(64.0, 64.0).unwrap(),
        ));
        let mut surface = page.surface();
        let mut fonts = tree.fontdb().clone();
        let mut process_context =
            ProcessContext::new(Arc::make_mut(&mut fonts), SvgSettings::default());
        assert!(try_render(group, &mut surface, &mut process_context));
        surface.finish();
        page.finish();
        let bytes = document.finish().unwrap();
        assert!(
            bytes
                .windows(b"/Luminosity".len())
                .any(|w| w == b"/Luminosity")
        );
        assert!(bytes.windows(b"/ca".len()).any(|w| w == b"/ca"));
    }
}
