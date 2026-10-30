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
    let text = scene.take_native_text();
    let ink = scene.take_native_ink();
    let text_error = scene.take_native_text_error();
    RenderedScene {
        page: RenderedPage::new(0, width, height, scene.finish()),
        text,
        ink,
        text_error,
    }
}
