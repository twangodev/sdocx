use super::super::native_page_index::{NativePageIndexUnavailable, NativePageLayout};
use super::*;

pub(in crate::render) fn layout_native_page_text(
    styled: &StyledText<'_>,
    frame: TextFrame<'_>,
    theme: RenderTheme,
    renderer: &TextRenderer<'_>,
) -> (
    TextLayout,
    Result<NativePageLayout, NativePageIndexUnavailable>,
) {
    let width = checked_width(styled, &frame);
    let width = match width {
        Ok(width) => width,
        Err(error) => {
            return (
                layout_capture_text(styled, frame, theme, renderer),
                Err(error),
            );
        }
    };
    let mut layout = try_layout_text_with_context(
        styled,
        frame,
        theme,
        renderer,
        LayoutContext::Capture,
        None,
        Some(NativeCellMeasurement {
            width: NativeCellMeasurementWidth::Fixed(width),
            height_limit: f32::INFINITY,
        }),
    )
    .expect("fixed-width body layout does not require native automatic measurement");
    let page_layout = NativePageLayout::from_measured(styled, &layout, width);
    layout.native_frame = None;
    (layout, page_layout)
}

fn checked_width(
    styled: &StyledText<'_>,
    frame: &TextFrame<'_>,
) -> Result<i32, NativePageIndexUnavailable> {
    use NativePageIndexUnavailable as Error;
    if frame.bbox.x_min != 0.0
        || frame.bbox.y_min != 0.0
        || frame.bbox.y_max != 0.0
        || frame.gravity.is_some_and(|gravity| gravity != 0)
        || !frame.exclusions.is_empty()
        || !frame.bbox.x_max.is_finite()
        || frame.bbox.x_max <= 0.0
        || frame.bbox.x_max >= f64::from(i32::MAX)
        || frame.bbox.x_max.fract() != 0.0
        || styled
            .text_box
            .margins
            .is_some_and(|margins| margins != [0.0; 4])
    {
        return Err(Error::OutsideCertificate);
    }
    let width = frame.bbox.x_max as i32;
    NativePageLayout::checked_source_length(styled, width)?;
    Ok(width)
}

#[cfg(all(test, feature = "serde"))]
mod tests;
