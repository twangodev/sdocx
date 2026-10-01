use krilla::geom::Rect;
use krilla::mask::Mask;
use krilla::paint::FillRule;
use krilla::surface::Surface;

use super::util::{RectExt, convert_mask_type};
use super::{ProcessContext, group};

/// Render a usvg `Mask` into a surface.
#[must_use]
pub(super) fn render(
    mask: &usvg::Mask,
    surface: &mut Surface,
    process_context: &mut ProcessContext,
) -> u16 {
    let mut stream_builder = surface.stream_builder();
    let mut sub_surface = stream_builder.surface();
    let mut pop_count = 0;
    if let Some(sub_mask) = mask.mask() {
        pop_count += render(sub_mask, &mut sub_surface, process_context)
    }

    let clip_path = {
        let rect = mask.rect().to_rect();
        Rect::from_ltrb(rect.left(), rect.top(), rect.right(), rect.bottom())
            .unwrap()
            .to_clip_path()
    };

    sub_surface.push_clip_path(&clip_path, &FillRule::NonZero);
    pop_count += 1;
    group::render(mask.root(), &mut sub_surface, process_context);

    for _ in 0..pop_count {
        sub_surface.pop();
    }

    sub_surface.finish();
    let stream = stream_builder.finish();

    surface.push_mask(Mask::new(stream, convert_mask_type(&mask.kind())));
    1
}
