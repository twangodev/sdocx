use krilla::surface::Surface;
use usvg::{Fill, PaintOrder, Stroke};

use super::ProcessContext;
use super::util::{PathExt, convert_fill, convert_stroke};

/// Render a path into a surface.
pub(super) fn render(
    path: &usvg::Path,
    surface: &mut Surface,
    process_context: &mut ProcessContext,
) {
    if !path.is_visible() {
        return;
    }

    match path.paint_order() {
        PaintOrder::FillAndStroke => {
            draw_path(path, path.fill(), path.stroke(), surface, process_context);
        }
        PaintOrder::StrokeAndFill => {
            draw_path(path, None, path.stroke(), surface, process_context);
            draw_path(path, path.fill(), None, surface, process_context);
        }
    }
}

/// Render a filled and/or stroked path into a surface.
pub(super) fn draw_path(
    path: &usvg::Path,
    fill: Option<&Fill>,
    stroke: Option<&Stroke>,
    surface: &mut Surface,
    process_context: &mut ProcessContext,
) {
    if process_context.error.is_some() {
        return;
    }
    let fill = fill.map(|f| {
        convert_fill(
            f,
            surface.stream_builder(),
            process_context,
            usvg::Transform::identity(),
        )
    });

    let stroke = stroke.map(|s| {
        convert_stroke(
            s,
            surface.stream_builder(),
            process_context,
            usvg::Transform::identity(),
        )
    });

    // Otherwise krilla will fill with black by default.
    if process_context.error.is_none() && (fill.is_some() || stroke.is_some()) {
        surface.set_fill(fill);
        surface.set_stroke(stroke);

        surface.draw_path(&path.to_krilla());
    }
}
