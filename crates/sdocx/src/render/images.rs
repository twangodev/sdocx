use super::image_crop;
use super::vector::{
    Definitions, Group, Image, ImageId, Scene, Styled, Svg, Transform, Use, ViewBox,
};
use crate::{BoundingBox, MediaAsset, PlacedImage};

pub(super) fn render_image(
    svg: &mut Scene,
    bbox: BoundingBox,
    media_index: Option<usize>,
    rotation: Option<f64>,
    media_assets: &[MediaAsset],
) {
    let Some(asset) = media_index.and_then(|index| media_assets.get(index)) else {
        return;
    };
    render_image_data(svg, bbox, rotation, &asset.data, &asset.mime_type);
}

fn render_image_data(
    svg: &mut Scene,
    bbox: BoundingBox,
    rotation: Option<f64>,
    data: &[u8],
    mime_type: &str,
) {
    let width = bbox.x_max - bbox.x_min;
    let height = bbox.y_max - bbox.y_min;
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return;
    }
    let mut image = Image::embedded(data, mime_type)
        .x(bbox.x_min)
        .y(bbox.y_min)
        .width(width)
        .height(height)
        .stretched();
    if let Some(angle) = rotation.filter(|angle| angle.is_finite()) {
        image = image.transformed(Transform::rotate_unrounded(
            angle,
            bbox.x_min + width / 2.,
            bbox.y_min + height / 2.,
        ));
    }
    svg.push(image);
}

pub(super) fn image_drawn_bbox(image: &PlacedImage) -> BoundingBox {
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

pub(super) fn render_placed_image(
    svg: &mut Scene,
    image: &PlacedImage,
    media_assets: &[MediaAsset],
    page_width: Option<u32>,
) {
    let alpha = placed_image_alpha(image.fill_effect_transparency);
    if alpha == 0 {
        return;
    }
    if alpha < 255 {
        svg.scope(Group::new().opacity(f64::from(alpha) / 255.0), |svg| {
            render_placed_image_content(svg, image, media_assets, page_width);
        });
    } else {
        render_placed_image_content(svg, image, media_assets, page_width);
    }
}

// The native Drawing path computes 255 - transparency * 255 as one f32 fused
// operation, converts toward zero, then clamps to a byte. Nonpositive values
// bypass the setter and retain the opaque paint alpha.
pub(super) fn placed_image_alpha(transparency: f32) -> u8 {
    if transparency.is_nan() || transparency <= 0.0 {
        return 255;
    }
    let scaled = (-transparency).mul_add(255.0, 255.0);
    (scaled as i32).clamp(0, 255) as u8
}

// The source and destination cuts follow Drawing's f32 arithmetic. Restrict
// this SVG route to positive, in-bounds patches; native also submits degenerate
// and out-of-bounds patches, whose sampling behavior is not established here.
fn render_nine_patch(
    svg: &mut Scene,
    image: &PlacedImage,
    assets: &[MediaAsset],
    page_width: Option<u32>,
) -> bool {
    if image.border_media_id.is_some()
        || image.original_media_id.is_some()
        || image.original_bbox.is_some()
    {
        return false;
    }
    if image.crop_rect.is_some_and(|[l, t, r, b]| l < r && t < b) {
        return false;
    }
    let Some(([l, t, r, b], reference_width)) = image.nine_patch else {
        return false;
    };
    let Some(asset) = image.media_index.and_then(|i| assets.get(i)) else {
        return false;
    };
    let Ok(size) = imagesize::blob_size(&asset.data) else {
        return false;
    };
    let (Ok(w), Ok(h)) = (i32::try_from(size.width), i32::try_from(size.height)) else {
        return false;
    };
    if !(0 < l && l < r && r < w && 0 < t && t < b && b < h) {
        return false;
    }
    let q = if reference_width == 0 {
        1.0
    } else if let Some(width) = page_width.and_then(|v| i32::try_from(v).ok()) {
        (width as f32) / (reference_width as f32)
    } else {
        return false;
    };
    let bbox = image.bbox;
    let [x0, y0, x3, y3] = [0.0_f32, 0.0, w as f32, h as f32];
    let width = bbox.x_max - bbox.x_min;
    let height = bbox.y_max - bbox.y_min;
    if ![bbox.x_min, bbox.y_min, width, height]
        .iter()
        .all(|v| v.is_finite())
        || width <= 0.0
        || height <= 0.0
        || !q.is_finite()
    {
        return false;
    }
    let (mut x1, mut x2) = ((l as f32).mul_add(q, x0), (-(w - r) as f32).mul_add(q, x3));
    let (mut y1, mut y2) = ((t as f32).mul_add(q, y0), (-(h - b) as f32).mul_add(q, y3));
    if x1 >= x2 || y1 >= y2 {
        (x1, x2, y1, y2) = (x0 + l as f32, x0 + r as f32, y0 + t as f32, y0 + b as f32);
    }
    let dx = [x0, x1.ceil(), x2.floor(), x3];
    let dy = [y0, y1.ceil(), y2.floor(), y3];
    if dx.windows(2).any(|v| v[0] >= v[1]) || dy.windows(2).any(|v| v[0] >= v[1]) {
        return false;
    }
    let sx = [0, l, r, w];
    let sy = [0, t, b, h];
    let angle = image.rotation_degrees.unwrap_or(0.0);
    if !angle.is_finite() {
        return false;
    }
    let source = svg.definition::<ImageId>();
    svg.scope(Definitions::new(), |svg| {
        svg.push(
            Image::embedded(&asset.data, &asset.mime_type)
                .id(&source.id_text())
                .x(0)
                .y(0)
                .width(f64::from(w))
                .height(f64::from(h))
                .stretched(),
        );
    });
    svg.scope(
        Group::new().transformed(Transform::rotate_unrounded(
            angle,
            (bbox.x_min + bbox.x_max) / 2.0,
            (bbox.y_min + bbox.y_max) / 2.0,
        )),
        |svg| {
            svg.scope(
                Group::new().transformed(Transform::matrix_unrounded([
                    width / f64::from(w),
                    0.0,
                    0.0,
                    height / f64::from(h),
                    bbox.x_min,
                    bbox.y_min,
                ])),
                |svg| {
                    for y in 0..3 {
                        for x in 0..3 {
                            // The source image itself bounds the outside edges. Do not
                            // clip them a second time at the same fractional device
                            // coordinates: two antialiased clips attenuate coverage.
                            let sw = sx[x + 1] - sx[x];
                            let sh = sy[y + 1] - sy[y];
                            let dw = dx[x + 1] - dx[x];
                            let dh = dy[y + 1] - dy[y];
                            let left = i32::from(x == 0);
                            let right = i32::from(x == 2);
                            let top = i32::from(y == 0);
                            let bottom = i32::from(y == 2);
                            svg.scope(
                                Svg::new()
                                    .x(f64::from(dx[x] - dw * left as f32))
                                    .y(f64::from(dy[y] - dh * top as f32))
                                    .width(f64::from(dw * (1 + left + right) as f32))
                                    .height(f64::from(dh * (1 + top + bottom) as f32))
                                    .view_box(ViewBox::unrounded(
                                        f64::from(sx[x]) - f64::from(sw) * f64::from(left),
                                        f64::from(sy[y]) - f64::from(sh) * f64::from(top),
                                        f64::from(sw) * f64::from(1 + left + right),
                                        f64::from(sh) * f64::from(1 + top + bottom),
                                    ))
                                    .preserve_aspect_none()
                                    .clipped_viewport(),
                                |svg| {
                                    svg.push(Use::image(&source));
                                },
                            );
                        }
                    }
                },
            );
        },
    );
    true
}

fn render_placed_image_content(
    svg: &mut Scene,
    image: &PlacedImage,
    media_assets: &[MediaAsset],
    page_width: Option<u32>,
) {
    if render_nine_patch(svg, image, media_assets, page_width) {
        return;
    }
    let simple_original_crop = image.original_bbox.is_none()
        || (image.border_media_id.is_none() && image.original_media_id.is_none());
    if simple_original_crop
        && image
            .rotation_degrees
            .is_none_or(|rotation| rotation.is_finite() && rotation == 0.0)
        && let (Some(crop), Some(asset)) = (
            image.crop_rect,
            image.media_index.and_then(|index| media_assets.get(index)),
        )
        && asset.mime_type == "image/png"
        && let Some(cropped_png) = image_crop::crop_rgb8_or_rgba8_png(&asset.data, crop)
    {
        // Keep this an SVG image at the displayed bounds. Only exact source
        // pixels are copied, so PDF export retains its image XObject.
        render_image_data(svg, image.bbox, None, &cropped_png, "image/png");
        return;
    }

    render_placed_image_body(svg, image, media_assets);
}

pub(super) fn render_placed_image_body(
    svg: &mut Scene,
    image: &PlacedImage,
    media_assets: &[MediaAsset],
) {
    // Drawing treats zero-area and inverted crop rectangles as empty, even
    // when original-placement metadata is present.
    let original = image
        .crop_rect
        .filter(|[l, t, r, b]| l < r && t < b)
        .and_then(|crop| {
            image.original_bbox.or_else(|| {
                let asset = media_assets.get(image.media_index?)?;
                image_crop::crop_only_placement(&asset.data, crop, image.bbox)
            })
        });
    if let Some(original) = original {
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
            Group::new().transformed(Transform::rotate_unrounded(angle, cx, cy)),
            |svg| {
                svg.scope(
                    Svg::new()
                        .x(bbox.x_min)
                        .y(bbox.y_min)
                        .width(width)
                        .height(height)
                        .view_box(ViewBox::unrounded(bbox.x_min, bbox.y_min, width, height))
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
