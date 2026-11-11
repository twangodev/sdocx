use super::*;

pub(crate) fn raster_scene(width: u32, height: u32, png: &[u8]) -> RenderedScene {
    let mut scene = Scene::new(Svg::new().width(width).height(height).view_box(
        ViewBox::unrounded(0., 0., f64::from(width), f64::from(height)),
    ));
    scene.retain_text();
    scene.push(
        Image::embedded(png, "image/png")
            .width(width)
            .height(height)
            .stretched()
            .pixelated(),
    );
    let vector::FinishedRetainedScene {
        svg,
        text,
        ink,
        text_error,
    } = scene.finish_retained();
    RenderedScene {
        page: RenderedPage::new(0, width, height, svg),
        text,
        ink,
        text_error,
    }
}
