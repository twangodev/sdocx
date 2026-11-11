#[cfg(feature = "pdf")]
use super::{document, page_with_uncolored_stroke, theme_test_text};
#[cfg(feature = "pdf")]
use crate::render::{RenderOptions, render_document_svg};
#[cfg(feature = "pdf")]
use crate::{
    BoundingBox, MediaAsset, ObjectSpanLayoutConstraint, ObjectSpanLayoutOption, ObjectType,
    PageElement, PlacedImage, RichTextObjectContent, RichTextObjectSpan,
};

#[test]
fn placed_image_alpha_matches_native_fused_quantization_and_clamp() {
    use crate::render::images::placed_image_alpha;

    for (transparency, expected) in [
        (-0.25, 255),
        (0.0, 255),
        (0.5, 127),
        (1.0, 0),
        (1.25, 0),
        (f32::MAX, 0),
        (f32::NAN, 255),
        // Fused native arithmetic gives 238; (1 - t) * 255 gives 237.
        (f32::from_bits(0x3d88_888c), 238),
    ] {
        assert_eq!(placed_image_alpha(transparency), expected);
    }
}

#[cfg(feature = "pdf")]
fn test_rgb_png() -> Vec<u8> {
    let pixels = [
        11, 12, 13, 21, 22, 23, 31, 32, 33, 41, 42, 43, // y=0
        51, 52, 53, 61, 62, 63, 71, 72, 73, 81, 82, 83, // y=1
        91, 92, 93, 101, 102, 103, 111, 112, 113, 121, 122, 123, // y=2
    ];
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, 4, 3);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&pixels)
        .unwrap();
    bytes
}

#[cfg(feature = "pdf")]
fn test_gamma_rgb_png() -> Vec<u8> {
    let pixels = [11, 12, 13, 21, 22, 23, 31, 32, 33, 41, 42, 43];
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, 4, 1);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_source_gamma(png::ScaledFloat::new(0.45455));
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&pixels)
        .unwrap();
    bytes
}

#[cfg(feature = "pdf")]
fn render_test_placed_image(image: PlacedImage, data: Vec<u8>) -> String {
    let mut page = page_with_uncolored_stroke();
    page.objects = vec![PageElement::PlacedImage(image).into()];
    let mut document = document(page);
    document.metadata.media_assets = vec![MediaAsset {
        name: "crop.png".to_owned(),
        archive_id: Some(0),
        mime_type: "image/png".to_owned(),
        data,
    }];
    render_document_svg(&document, &RenderOptions::default())
        .remove(0)
        .svg
}

#[cfg(feature = "pdf")]
fn simple_original_crop(rotation_degrees: Option<f64>) -> PlacedImage {
    PlacedImage {
        bbox: BoundingBox {
            x_min: 20.0,
            y_min: 30.0,
            x_max: 70.0,
            y_max: 50.0,
        },
        rotation_degrees,
        media_id: Some(0),
        media_index: Some(0),
        fill_effect_transparency: 0.0,
        crop_rect: Some([1, 1, 3, 2]),
        original_bbox: Some(BoundingBox {
            x_min: 0.0,
            y_min: 0.0,
            x_max: 4.0,
            y_max: 3.0,
        }),
        border_media_id: None,
        original_media_id: None,
        nine_patch: None,
    }
}

#[cfg(feature = "pdf")]
#[test]
fn nine_patch_uses_page_width_and_crop_takes_precedence() {
    let mut image = simple_original_crop(None);
    image.bbox = BoundingBox {
        x_min: 20.0,
        y_min: 30.0,
        x_max: 70.0,
        y_max: 60.0,
    };
    image.crop_rect = None;
    image.original_bbox = None;
    image.nine_patch = Some(([1, 1, 3, 2], 500));
    let svg = render_test_placed_image(image.clone(), test_rgb_png());
    let xml = roxmltree::Document::parse(&svg).unwrap();
    let patches: Vec<_> = xml
        .descendants()
        .filter(|n| n.has_tag_name("svg") && n.attribute("viewBox").is_some())
        .collect();
    assert_eq!(patches.len(), 10); // page and nine source views
    assert_eq!(
        xml.descendants()
            .filter(|n| n.has_tag_name("image"))
            .count(),
        1
    );
    assert_eq!(
        xml.descendants().filter(|n| n.has_tag_name("use")).count(),
        9
    );
    let middle = patches
        .iter()
        .find(|n| n.attribute("viewBox") == Some("1 1 2 1"))
        .unwrap();
    assert_eq!(middle.attribute("x"), Some("1"));
    assert_eq!(middle.attribute("y"), Some("1"));
    assert_eq!(middle.attribute("width"), Some("2"));
    assert_eq!(middle.attribute("height"), Some("1"));
    // The source image, rather than an additional coincident viewport
    // clip, bounds the outside edge of the top-left patch.
    let corner = patches
        .iter()
        .find(|n| n.attribute("viewBox") == Some("-1 -1 2 2"))
        .unwrap();
    assert_eq!(corner.attribute("x"), Some("-1"));
    assert_eq!(corner.attribute("y"), Some("-1"));
    assert_eq!(corner.attribute("width"), Some("2"));
    assert_eq!(corner.attribute("height"), Some("2"));
    image.rotation_degrees = Some(30.0);
    let rotated = render_test_placed_image(image.clone(), test_rgb_png());
    assert!(rotated.contains("rotate(30 45 45)"));
    let mut page = page_with_uncolored_stroke();
    page.objects = vec![PageElement::PlacedImage(image.clone()).into()];
    let mut doc = document(page);
    doc.metadata.media_assets = vec![MediaAsset {
        name: "patch.png".into(),
        archive_id: Some(0),
        mime_type: "image/png".into(),
        data: test_rgb_png(),
    }];
    assert!(
        crate::render_document_pdf(
            &doc,
            &RenderOptions::default(),
            &crate::PdfOptions::default()
        )
        .is_ok()
    );
    image.rotation_degrees = None;
    image.crop_rect = Some([1, 1, 3, 2]);
    let svg = render_test_placed_image(image, test_rgb_png());
    let xml = roxmltree::Document::parse(&svg).unwrap();
    assert_eq!(
        xml.descendants()
            .filter(|n| n.has_tag_name("image"))
            .count(),
        1
    );
}

#[cfg(feature = "pdf")]
#[test]
fn embedded_nine_patch_uses_scene_page_width() {
    let mut image = simple_original_crop(None);
    image.crop_rect = None;
    image.original_bbox = None;
    image.nine_patch = Some(([1, 1, 3, 2], 500));
    let mut body = theme_test_text();
    body.bbox = BoundingBox::default();
    body.text = "\u{fffc}".into();
    body.object_spans = vec![RichTextObjectSpan {
        object_type: ObjectType::Image,
        object_data: Vec::new(),
        content: Some(RichTextObjectContent::Image(Box::new(image))),
        text_index_utf16: 0,
        layout_option: ObjectSpanLayoutOption::Block,
        layout_constraint: ObjectSpanLayoutConstraint::Normal,
    }];
    let mut page = page_with_uncolored_stroke();
    page.width = 500;
    page.height = 200;
    page.objects.clear();
    let mut doc = document(page);
    doc.metadata.page_mode = Some(1);
    doc.metadata.default_page_dimensions = Some((360, 1527));
    doc.metadata.flow_dimensions = Some((500, 200));
    doc.metadata.flow_page_padding = Some((0, 0));
    doc.metadata.note_text = Some(body);
    doc.metadata.media_assets = vec![MediaAsset {
        name: "patch.png".into(),
        archive_id: Some(0),
        mime_type: "image/png".into(),
        data: test_rgb_png(),
    }];
    let svg = render_document_svg(&doc, &RenderOptions::default())
        .remove(0)
        .svg;
    let xml = roxmltree::Document::parse(&svg).unwrap();
    assert_eq!(
        xml.descendants().filter(|n| n.has_tag_name("use")).count(),
        9
    );
    let center = xml
        .descendants()
        .find(|n| n.attribute("viewBox") == Some("1 1 2 1"))
        .unwrap();
    assert_eq!(center.attribute("x"), Some("1"));
    assert_eq!(center.attribute("width"), Some("2"));
}

#[cfg(feature = "pdf")]
#[test]
fn nine_patch_svg_raster_preserves_nine_colored_regions() {
    let mut pixels = Vec::new();
    for y in 0..9 {
        for x in 0..9 {
            pixels.extend_from_slice(&[(x / 3 * 100) as u8, (y / 3 * 100) as u8, 30]);
        }
    }
    let mut data = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut data, 9, 9);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&pixels)
            .unwrap();
    }
    let mut image = simple_original_crop(None);
    image.bbox = BoundingBox {
        x_min: 20.0,
        y_min: 30.0,
        x_max: 70.0,
        y_max: 60.0,
    };
    image.crop_rect = None;
    image.original_bbox = None;
    image.nine_patch = Some(([3, 3, 6, 6], 200));
    let svg = render_test_placed_image(image, data);
    let tree = resvg::usvg::Tree::from_str(&svg, &Default::default()).unwrap();
    let mut pixmap = resvg::tiny_skia::Pixmap::new(100, 100).unwrap();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    for (row, y) in [31, 45, 58].into_iter().enumerate() {
        for (col, x) in [21, 45, 68].into_iter().enumerate() {
            let pixel = pixmap.pixel(x, y).unwrap();
            for (actual, expected) in [pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()]
                .into_iter()
                .zip([(col * 100) as u8, (row * 100) as u8, 30, 255])
            {
                assert!(actual.abs_diff(expected) <= 1, "{actual} vs {expected}");
            }
        }
    }
    // The nine source viewports must not leak beyond the placement, even
    // without a second clip around the transformed group.
    for (x, y) in [(19, 45), (70, 45), (45, 29), (45, 60)] {
        let pixel = pixmap.pixel(x, y).unwrap();
        assert_eq!([pixel.red(), pixel.green(), pixel.blue()], [252; 3]);
    }
}

#[cfg(feature = "pdf")]
#[test]
fn original_bbox_rgb_and_rgba_crops_use_display_bounds() {
    for source in [
        test_rgb_png(),
        include_bytes!("../../../tests/assets/crop-quadrants.png").to_vec(),
    ] {
        let svg = render_test_placed_image(simple_original_crop(Some(0.0)), source);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let images = xml
            .descendants()
            .filter(|node| node.has_tag_name("image"))
            .collect::<Vec<_>>();
        assert_eq!(images.len(), 1);
        let image = images[0];
        for (attribute, expected) in [("x", 20.0), ("y", 30.0), ("width", 50.0), ("height", 20.0)] {
            let actual = image.attribute(attribute).unwrap().parse::<f64>().unwrap();
            assert_eq!(actual, expected, "attribute {attribute}");
        }
        assert!(
            image
                .attribute("href")
                .is_some_and(|href| href.starts_with("data:image/png;base64,"))
        );
    }
}

#[cfg(feature = "pdf")]
#[test]
fn rotated_or_unsupported_original_crops_keep_the_full_source_path() {
    let mut extra_media = simple_original_crop(None);
    extra_media.border_media_id = Some(7);
    for (image, source) in [
        (simple_original_crop(Some(15.0)), test_rgb_png()),
        (extra_media, test_rgb_png()),
        (simple_original_crop(None), test_gamma_rgb_png()),
    ] {
        let svg = render_test_placed_image(image, source);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let image = xml
            .descendants()
            .find(|node| node.has_tag_name("image"))
            .unwrap();
        assert_eq!(image.attribute("width"), Some("4"));
        assert_eq!(image.attribute("height"), Some("3"));
    }
}

#[cfg(feature = "pdf")]
#[test]
fn empty_crop_ignores_original_placement() {
    for rotation in [None, Some(15.0)] {
        let mut image = simple_original_crop(rotation);
        image.crop_rect = None;
        let expected = render_test_placed_image(image.clone(), test_rgb_png());
        for crop in [[1, 1, 1, 2], [1, 2, 3, 2], [3, 1, 1, 2], [1, 2, 3, 1]] {
            image.crop_rect = Some(crop);
            assert_eq!(
                render_test_placed_image(image.clone(), test_rgb_png()),
                expected
            );
        }
    }
}
