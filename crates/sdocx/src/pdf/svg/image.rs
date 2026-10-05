use krilla::geom::{Rect, Size};
use krilla::paint::FillRule;
use krilla::surface::Surface;
use usvg::ImageKind;

use super::util::RectExt;
use super::{ProcessContext, group, raster_image};

/// Render an image into a surface.
///
/// Returns an error if the image could not be rendered.
pub(super) fn render(
    image: &usvg::Image,
    surface: &mut Surface,
    process_context: &mut ProcessContext,
) -> Result<(), String> {
    if !image.is_visible() {
        return Ok(());
    }

    let size = Size::from_wh(image.size().width(), image.size().height()).unwrap();

    if let Some(image) = raster_image(image.kind())? {
        surface.draw_image(image, size);
    } else if let ImageKind::SVG(t) = image.kind() {
        let clip_path = Rect::from_xywh(0.0, 0.0, t.size().width(), t.size().height())
            .ok_or("invalid embedded SVG image dimensions")?
            .to_clip_path();
        surface.push_clip_path(&clip_path, &FillRule::NonZero);
        group::render(t.root(), surface, process_context);
        surface.pop();
    }

    Ok(())
}
