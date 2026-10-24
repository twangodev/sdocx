mod support;

use sdocx::{DiagnosticCode, Error, PageElement, PlacedImage};
use std::io::{Cursor, Write};
use support::{object, page, page_with_current_layer};

fn frame(kind: i16, fields: u32, fixed: &[u8], flexible: &[u8]) -> Vec<u8> {
    let offset = 17 + fixed.len();
    let mut bytes = ((offset + flexible.len()) as u32).to_le_bytes().to_vec();
    bytes.extend_from_slice(&kind.to_le_bytes());
    bytes.extend_from_slice(&(offset as u32).to_le_bytes());
    bytes.extend_from_slice(&[1, u8::from(kind == 0) << 3, 4]);
    bytes.extend_from_slice(&fields.to_le_bytes());
    bytes.extend_from_slice(fixed);
    bytes.extend_from_slice(flexible);
    bytes
}

fn base() -> Vec<u8> {
    let mut fixed = 5500_u32.to_le_bytes().to_vec();
    fixed.extend_from_slice(&5_u16.to_le_bytes());
    fixed.extend_from_slice(b"image");
    fixed.extend_from_slice(&1234_i64.to_le_bytes());
    for value in [-10.0_f64, 20.0, 90.0, 100.0] {
        fixed.extend_from_slice(&value.to_le_bytes());
    }
    fixed.extend_from_slice(&0_i32.to_le_bytes());
    fixed.push(0);
    frame(0, 1, &fixed, &30.0_f32.to_le_bytes())
}

fn fill(id: i32) -> Vec<u8> {
    let mut bytes = vec![0]; // stretch mode
    bytes.extend_from_slice(&id.to_le_bytes());
    for value in [0.0_f32, 0.0, 0.0, 0.0, 0.0, 0.0, 100.0, 100.0, 0.0] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.push(0); // fill rotatable
    bytes.extend_from_slice(&[0; 20]); // nine-patch rectangle and width
    assert_eq!(bytes.len(), 62);
    bytes
}

fn shape_fixed() -> Vec<u8> {
    let mut fixed = 4_u32.to_le_bytes().to_vec();
    for value in [0.0_f64, 0.0, 100.0, 80.0] {
        fixed.extend_from_slice(&value.to_le_bytes());
    }
    fixed.extend_from_slice(&0.0_f32.to_le_bytes()); // shape rotation
    fixed.extend_from_slice(&0_u32.to_le_bytes()); // path size
    fixed.push(0); // control points
    fixed
}

fn image_with_fill(
    fill_kind: u8,
    fill_data: &[u8],
    prefix_mask: u32,
    prefix: &[u8],
    tail: &[u8],
) -> Vec<u8> {
    let mut bytes = base();
    bytes.extend(frame(6, 0, &[], &[]));
    let mut flexible = prefix.to_vec();
    flexible.extend_from_slice(&(fill_data.len() as u32).to_le_bytes());
    flexible.push(fill_kind);
    flexible.extend_from_slice(fill_data);
    bytes.extend(frame(7, 32 | prefix_mask, &shape_fixed(), &flexible));
    bytes.extend_from_slice(tail);
    bytes
}

fn image(id: i32) -> Vec<u8> {
    image_with_fill(2, &fill(id), 0, &[], &frame(3, 0, &[], &[]))
}

fn image_with_transparency(id: i32, transparency: f32) -> Vec<u8> {
    image_with_fill(
        2,
        &fill(id),
        0,
        &[],
        &frame(3, 1 << 7, &[], &transparency.to_le_bytes()),
    )
}

#[cfg(all(feature = "render", feature = "pdf"))]
fn uniform_rgba_png(width: u32, height: u32, rgba: [u8; 4]) -> Vec<u8> {
    let pixels = rgba.repeat((width * height) as usize);
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&pixels).unwrap();
    }
    bytes
}

#[cfg(feature = "render")]
fn cropped_type3_document(
    source: &[u8],
    crop: [i32; 4],
    bbox: sdocx::BoundingBox,
    page_size: (u32, u32),
) -> sdocx::Document {
    let mut document = sdocx::parse_bytes_detailed(&archive(
        &one_page(vec![object(
            3,
            &image_with_transparency(7, f32::from_bits(0x3efe_feff)),
            &[],
        )]),
        Some(&[(7, "native-alpha.png")]),
        &[("native-alpha.png", source)],
    ))
    .unwrap()
    .document;
    let mut image = placed(document.pages[0].elements().next().unwrap()).clone();
    image.bbox = bbox;
    image.crop_rect = Some(crop);
    image.original_bbox = None;
    image.rotation_degrees = None;
    document.pages[0].width = page_size.0;
    document.pages[0].height = page_size.1;
    document.pages[0].content_bbox = sdocx::BoundingBox {
        x_min: 0.0,
        y_min: 0.0,
        x_max: f64::from(page_size.0),
        y_max: f64::from(page_size.1),
    };
    document.pages[0].background_color = None;
    document.pages[0].objects = vec![PageElement::PlacedImage(image).into()];
    document
}

#[cfg(feature = "render")]
fn decode_rgba8_png(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
    let mut reader = png::Decoder::new(Cursor::new(bytes)).read_info().unwrap();
    assert_eq!(reader.info().color_type, png::ColorType::Rgba);
    assert_eq!(reader.info().bit_depth, png::BitDepth::Eight);
    let mut decoded = vec![0; reader.output_buffer_size().unwrap()];
    let frame = reader.next_frame(&mut decoded).unwrap();
    decoded.truncate(frame.buffer_size());
    (frame.width, frame.height, decoded)
}

#[cfg(feature = "render")]
#[test]
fn type3_crop_embeds_exact_native_png_pixels_at_existing_svg_bounds() {
    const SOURCE: &[u8] = include_bytes!("assets/crop-native-alpha-4x4.png");
    let document = cropped_type3_document(
        SOURCE,
        [1, 1, 3, 3],
        sdocx::BoundingBox {
            x_min: 10.0,
            y_min: 20.0,
            x_max: 50.0,
            y_max: 50.0,
        },
        (64, 64),
    );
    let rendered = sdocx::render_page_svg(&document, 0, &Default::default()).unwrap();
    let xml = roxmltree::Document::parse(&rendered.svg).unwrap();
    let image = xml
        .descendants()
        .find(|node| node.has_tag_name("image"))
        .expect("cropped type-3 image remains an SVG image");
    assert_eq!(image.attribute("x"), Some("10"));
    assert_eq!(image.attribute("y"), Some("20"));
    assert_eq!(image.attribute("width"), Some("40"));
    assert_eq!(image.attribute("height"), Some("30"));
    let opacity: f64 = image
        .ancestors()
        .find_map(|node| node.attribute("opacity"))
        .expect("native fill-effect alpha remains an SVG group opacity")
        .parse()
        .unwrap();
    assert!((opacity - 128.0 / 255.0).abs() < 0.000_001);

    use base64::Engine as _;
    let href = image.attribute("href").expect("embedded crop PNG");
    let encoded = href.strip_prefix("data:image/png;base64,").unwrap();
    let cropped_png = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .unwrap();
    let (source_width, source_height, source_pixels) = decode_rgba8_png(SOURCE);
    let (crop_width, crop_height, crop_pixels) = decode_rgba8_png(&cropped_png);
    assert_eq!((source_width, source_height), (4, 4));
    assert_eq!((crop_width, crop_height), (2, 2));
    let mut expected = Vec::new();
    for y in 1..3_usize {
        expected.extend_from_slice(&source_pixels[(y * 4 + 1) * 4..(y * 4 + 3) * 4]);
    }
    assert_eq!(
        crop_pixels, expected,
        "crop keeps exact straight RGBA samples"
    );
}

#[cfg(feature = "pdf")]
#[test]
fn type3_png_crop_remains_a_pdf_image_with_a_cropped_soft_mask() {
    const SOURCE: &[u8] = include_bytes!("assets/crop-native-alpha-4x4.png");
    let document = cropped_type3_document(
        SOURCE,
        [1, 1, 3, 3],
        sdocx::BoundingBox {
            x_min: 10.0,
            y_min: 20.0,
            x_max: 50.0,
            y_max: 50.0,
        },
        (64, 64),
    );
    let options = sdocx::PdfOptions::new(std::sync::Arc::new(Default::default()));
    let bytes = sdocx::render_document_pdf(&document, &Default::default(), &options).unwrap();
    let pdf = lopdf::Document::load_mem(&bytes).unwrap();
    let images: Vec<_> = pdf
        .objects
        .iter()
        .filter_map(|(id, object)| {
            let stream = object.as_stream().ok()?;
            (stream.dict.get(b"Subtype").ok()?.as_name().ok()? == b"Image")
                .then_some((*id, &stream.dict))
        })
        .collect();
    assert_eq!(
        images.len(),
        2,
        "cropped color image and alpha soft mask stay embedded images"
    );
    let (mask_id, mask) = images
        .iter()
        .find(|(_, dict)| {
            dict.get(b"ColorSpace")
                .is_ok_and(|value| value.as_name().is_ok_and(|name| name == b"DeviceGray"))
        })
        .expect("cropped alpha soft-mask image");
    let (color_id, color) = images
        .iter()
        .find(|(_, dict)| dict.get(b"SMask").is_ok())
        .expect("cropped color image with soft-mask reference");
    assert_ne!(color_id, mask_id);
    assert_eq!(color.get(b"Width").unwrap().as_i64().unwrap(), 2);
    assert_eq!(color.get(b"Height").unwrap().as_i64().unwrap(), 2);
    assert_eq!(mask.get(b"Width").unwrap().as_i64().unwrap(), 2);
    assert_eq!(mask.get(b"Height").unwrap().as_i64().unwrap(), 2);
    assert_eq!(
        color.get(b"SMask").unwrap().as_reference().unwrap(),
        *mask_id
    );
}

#[cfg(feature = "render")]
#[test]
fn native_upscale_crop_edges_reduce_alpha_error_versus_legacy_full_image_clip() {
    const SOURCE: &[u8] = include_bytes!("assets/crop-native-upscale-context-32x16.png");
    // Captured native raw.json SHA-256:
    // dc03743e509e5552616247011f24468c52056ad74cec10fb0c5867623361c106.
    // The compact 6 KiB target is SHA-256:
    // 7e80428f42a832f4a195f00d5696a9f6e00329febc450551edd0f1bbf6f30426.
    // It covers source crop [73,50,89,58] and destination [8,8,40,24].
    const NATIVE_TARGET: &[u8] =
        include_bytes!("assets/native-image-alpha-upscale-target-48x32.rgba");
    assert_eq!(SOURCE.len(), 2059);
    assert_eq!(NATIVE_TARGET.len(), 48 * 32 * 4);

    let document = cropped_type3_document(
        SOURCE,
        [8, 4, 24, 12],
        sdocx::BoundingBox {
            x_min: 8.0,
            y_min: 8.0,
            x_max: 40.0,
            y_max: 24.0,
        },
        (48, 32),
    );
    let cropped_svg = sdocx::render_page_svg(&document, 0, &Default::default())
        .unwrap()
        .svg;

    // Supplying the previously emitted full-source placement forces the
    // legacy image-plus-viewport path while preserving the same source, crop,
    // destination, and alpha.
    let mut legacy = document.clone();
    let mut image = placed(legacy.pages[0].elements().next().unwrap()).clone();
    image.original_bbox = Some(sdocx::BoundingBox {
        x_min: -8.0,
        y_min: 0.0,
        x_max: 56.0,
        y_max: 32.0,
    });
    legacy.pages[0].objects = vec![PageElement::PlacedImage(image).into()];
    let legacy_svg = sdocx::render_page_svg(&legacy, 0, &Default::default())
        .unwrap()
        .svg;

    fn rgba_from_svg(svg: &str) -> Vec<u8> {
        let tree = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default()).unwrap();
        let mut pixmap = resvg::tiny_skia::Pixmap::new(48, 32).unwrap();
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::identity(),
            &mut pixmap.as_mut(),
        );
        pixmap.data().to_vec()
    }
    fn alpha_error(actual: &[u8], expected: &[u8]) -> u64 {
        actual
            .as_chunks::<4>()
            .0
            .iter()
            .zip(expected.as_chunks::<4>().0)
            .map(|(actual, expected)| u64::from(actual[3].abs_diff(expected[3])))
            .sum()
    }

    fn without_full_page_paper(svg: &str) -> String {
        let xml = roxmltree::Document::parse(svg).unwrap();
        let mut paper = xml
            .root_element()
            .children()
            .filter(|node| node.is_element() && node.has_tag_name("rect"))
            .filter(|node| {
                ["x", "y", "width", "height"]
                    .into_iter()
                    .zip([0.0_f64, 0.0, 48.0, 32.0])
                    .all(|(attribute, expected)| {
                        node.attribute(attribute)
                            .and_then(|value| value.parse::<f64>().ok())
                            == Some(expected)
                    })
            });
        let rect = paper.next().expect("exact full-page paper rectangle");
        assert!(paper.next().is_none(), "only one full-page paper rectangle");
        let range = rect.range();
        let mut layer = String::with_capacity(svg.len());
        layer.push_str(&svg[..range.start]);
        layer.push_str(&svg[range.end..]);
        layer
    }

    let cropped = rgba_from_svg(&without_full_page_paper(&cropped_svg));
    let legacy = rgba_from_svg(&without_full_page_paper(&legacy_svg));
    let cropped_error = alpha_error(&cropped, NATIVE_TARGET);
    let legacy_error = alpha_error(&legacy, NATIVE_TARGET);
    assert!(
        cropped_error * 4 < legacy_error * 3,
        "exact source crop should improve whole-image alpha error: cropped={cropped_error}, legacy={legacy_error}"
    );
}

fn rectangle_path(points: [[f64; 2]; 4]) -> Vec<u8> {
    let mut bytes = 5_u32.to_le_bytes().to_vec();
    for (index, point) in points.into_iter().enumerate() {
        bytes.push(if index == 0 { 1 } else { 2 });
        bytes.extend(point.into_iter().flat_map(f64::to_le_bytes));
    }
    bytes.push(6);
    bytes
}

fn image_outline(kind: u8, width: f32) -> Vec<u8> {
    let mut fixed = 4_u32.to_le_bytes().to_vec();
    for point in [[40.0_f64, 20.0], [90.0, 60.0], [40.0, 100.0], [-10.0, 60.0]] {
        fixed.extend(point.into_iter().flat_map(f64::to_le_bytes));
    }
    fixed.extend(4_u32.to_le_bytes());
    fixed.extend([0; 5]);
    let mut color = vec![1, 0, kind];
    color.extend(0xff000000_u32.to_le_bytes());
    color.extend([0; 12]);
    let mut flexible = (color.len() as u32).to_le_bytes().to_vec();
    flexible.extend(color);
    flexible.extend(12_u32.to_le_bytes());
    flexible.extend(width.to_le_bytes());
    flexible.extend([0; 8]);
    frame(6, 12, &fixed, &flexible)
}

fn native_image(rotation: f32, path: &[u8], outline: &[u8], fill_data: &[u8]) -> Vec<u8> {
    let mut bytes = base();
    let rotation_offset = bytes.len() - 4;
    bytes[rotation_offset..].copy_from_slice(&rotation.to_le_bytes());
    bytes.extend(outline);
    let mut fixed = 4_u32.to_le_bytes().to_vec();
    for coordinate in [-10.0_f64, 20.0, 90.0, 100.0] {
        fixed.extend(coordinate.to_le_bytes());
    }
    fixed.extend(rotation.to_le_bytes());
    fixed.extend((path.len() as u32).to_le_bytes());
    fixed.extend(path);
    fixed.push(0);
    let mut flexible = (fill_data.len() as u32).to_le_bytes().to_vec();
    flexible.push(2);
    flexible.extend(fill_data);
    bytes.extend(frame(7, 32, &fixed, &flexible));
    bytes.extend(frame(3, 0, &[], &[]));
    bytes
}

fn parse_native_image(payload: &[u8]) -> sdocx::ParsedDocument {
    sdocx::parse_bytes_detailed(&with_note(
        archive(
            &one_page(vec![object(3, payload, &[])]),
            Some(&[(7, "main.png")]),
            &[("main.png", b"image")],
        ),
        "\n\u{fffc}",
        &[(1, payload.to_vec())],
    ))
    .unwrap()
}

#[test]
fn standard_rectangular_images_accept_inactive_outline_and_nine_patch_settings() {
    let mut fill_data = fill(7);
    fill_data[58..62].copy_from_slice(&1080_i32.to_le_bytes());
    for (rotation, points) in [
        (
            0.0,
            [[-10.0, 20.0], [90.0, 20.0], [90.0, 100.0], [-10.0, 100.0]],
        ),
        (
            90.0,
            [[80.0, 10.0], [80.0, 110.0], [0.0, 110.0], [0.0, 10.0]],
        ),
    ] {
        for outline in [
            image_outline(2, 0.0),
            image_outline(2, 2.0),
            image_outline(0, 0.0),
        ] {
            let payload = native_image(rotation, &rectangle_path(points), &outline, &fill_data);
            let parsed = parse_native_image(&payload);
            assert!(
                !parsed.report.diagnostics.iter().any(|diagnostic| matches!(
                    diagnostic.code,
                    DiagnosticCode::UnsupportedImageFeature
                        | DiagnosticCode::UnresolvedImageMedia
                        | DiagnosticCode::InferredImageMediaReference
                )),
                "{:?}",
                parsed.report.diagnostics
            );
            assert_eq!(
                placed(parsed.document.pages[0].elements().next().unwrap()).media_index,
                Some(0)
            );
            let span = &parsed.note.as_ref().unwrap().body.object_spans[0];
            assert_eq!(embedded_image(span).media_index, Some(0));
            assert_eq!(span.object_data, payload);
        }
    }
}

#[test]
fn custom_image_paths_visible_outlines_and_active_fill_effects_still_warn() {
    let points = [[-10.0, 20.0], [90.0, 20.0], [90.0, 100.0], [-10.0, 100.0]];
    let rectangular = rectangle_path(points);
    let outline = image_outline(2, 0.0);
    let plain_fill = fill(7);
    let mut custom_points = points;
    custom_points[1][0] -= 1.0;
    let mut open_path = rectangular.clone();
    open_path[..4].copy_from_slice(&4_u32.to_le_bytes());
    open_path.pop();
    let mut trailing_path = rectangular.clone();
    trailing_path.push(0);
    let mut cases = vec![
        (
            native_image(0.0, &rectangle_path(custom_points), &outline, &plain_fill),
            "image shape geometry",
        ),
        (
            native_image(90.0, &rectangular, &outline, &plain_fill),
            "image shape geometry",
        ),
        (
            native_image(0.0, &open_path, &outline, &plain_fill),
            "image shape geometry",
        ),
        (
            native_image(0.0, &trailing_path, &outline, &plain_fill),
            "image shape geometry",
        ),
        (
            native_image(0.0, &rectangular, &image_outline(0, 2.0), &plain_fill),
            "image outlines",
        ),
    ];
    for (offset, replacement) in [
        (0, vec![1]),
        (5, 1.0_f32.to_le_bytes().to_vec()),
        (37, 25.0_f32.to_le_bytes().to_vec()),
        (41, vec![1]),
        (
            42,
            [1_i32, 2, 30, 40]
                .into_iter()
                .flat_map(i32::to_le_bytes)
                .collect(),
        ),
    ] {
        let mut fill_data = plain_fill.clone();
        fill_data[offset..offset + replacement.len()].copy_from_slice(&replacement);
        cases.push((
            native_image(0.0, &rectangular, &outline, &fill_data),
            "image fill transforms",
        ));
    }
    let mut extended_outline = outline.clone();
    extended_outline[11] = 1;
    cases.push((
        native_image(0.0, &rectangular, &extended_outline, &plain_fill),
        "shape connections or base extensions",
    ));
    let mut different_template = native_image(0.0, &rectangular, &outline, &plain_fill);
    let shape_fixed_offset = base().len() + outline.len() + 17;
    different_template[shape_fixed_offset..shape_fixed_offset + 4]
        .copy_from_slice(&1_u32.to_le_bytes());
    cases.push((different_template, "image shape geometry"));
    let mut different_bounds = native_image(0.0, &rectangular, &outline, &plain_fill);
    different_bounds[shape_fixed_offset + 4..shape_fixed_offset + 12]
        .copy_from_slice(&0.0_f64.to_le_bytes());
    cases.push((different_bounds, "image shape geometry"));
    for (payload, expected) in cases {
        let parsed = parse_native_image(&payload);
        let warnings: Vec<_> = parsed
            .report
            .diagnostics
            .iter()
            .filter(|d| d.code == DiagnosticCode::UnsupportedImageFeature)
            .collect();
        assert_eq!(warnings.len(), 2);
        assert!(
            warnings.iter().all(|d| d.message.contains(expected)),
            "{warnings:?}"
        );
        assert!(
            warnings
                .iter()
                .any(|d| d.message.starts_with("embedded image at UTF-16 index 1:"))
        );
    }
}

#[test]
fn native_image_paths_and_inherited_settings_remain_bounded() {
    let path = rectangle_path([[-10.0, 20.0], [90.0, 20.0], [90.0, 100.0], [-10.0, 100.0]]);
    let outline = image_outline(2, 0.0);
    let fill_data = fill(7);
    let mut invalid_paths: Vec<_> = (1..path.len()).map(|end| path[..end].to_vec()).collect();
    let mut nonfinite = path.clone();
    nonfinite[5..13].copy_from_slice(&f64::NAN.to_le_bytes());
    invalid_paths.push(nonfinite);
    let mut payloads: Vec<_> = invalid_paths
        .iter()
        .map(|path| native_image(0.0, path, &outline, &fill_data))
        .collect();
    let mut invalid_outline = outline.clone();
    invalid_outline[17..21].copy_from_slice(&u32::MAX.to_le_bytes());
    payloads.push(native_image(0.0, &path, &invalid_outline, &fill_data));
    for payload in payloads {
        let bytes = archive(&one_page(vec![object(3, &payload, &[])]), None, &[]);
        assert!(matches!(sdocx::parse_bytes(&bytes), Err(Error::Format(_))));
    }
}

fn embedded_text(text: &str, objects: &[(i32, Vec<u8>)]) -> Vec<u8> {
    let mut common = (text.encode_utf16().count() as u32).to_le_bytes().to_vec();
    common.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
    common.extend([0; 8]);
    for margin in [16.0_f32, 10.0, 16.0, 10.0] {
        common.extend(margin.to_le_bytes());
    }
    common.extend([0; 3]);
    common.extend(1_u32.to_le_bytes());
    common.extend(0_u32.to_le_bytes());
    common.extend((objects.len() as u32).to_le_bytes());
    for (index, payload) in objects {
        common.extend(((payload.len() + 20) as u32).to_le_bytes());
        common.extend((payload.len() as u32).to_le_bytes());
        common.extend(3_u32.to_le_bytes());
        common.extend(payload);
        common.extend(index.to_le_bytes());
        common.extend([0; 8]);
    }
    let mut fixed = 5500_u32.to_le_bytes().to_vec();
    fixed.extend(4_u16.to_le_bytes());
    fixed.extend(b"flow");
    fixed.extend([0; 45]);
    let mut flexible = (common.len() as u32).to_le_bytes().to_vec();
    flexible.extend(common);
    [
        frame(0, 0, &fixed, &[]),
        frame(6, 0, &[], &[]),
        frame(7, 1, &[], &flexible),
    ]
    .concat()
}

fn with_note(archive: Vec<u8>, text: &str, objects: &[(i32, Vec<u8>)]) -> Vec<u8> {
    let mut note = vec![0; 6];
    note.extend(5500_u32.to_le_bytes());
    note.extend([0; 22]);
    for value in [1080_u32, 1527, 0, 24, 4000] {
        note.extend(value.to_le_bytes());
    }
    for text in [embedded_text("", &[]), embedded_text(text, objects)] {
        note.extend((text.len() as u32).to_le_bytes());
        note.extend(text);
    }
    let offset = note.len() as u32;
    note[..4].copy_from_slice(&offset.to_le_bytes());
    note.extend([0; 32]);
    let mut writer = zip::ZipWriter::new_append(Cursor::new(archive)).unwrap();
    writer
        .start_file("note.note", zip::write::SimpleFileOptions::default())
        .unwrap();
    writer.write_all(&note).unwrap();
    writer.finish().unwrap().into_inner()
}

fn embedded_image(span: &sdocx::RichTextObjectSpan) -> &PlacedImage {
    let Some(sdocx::RichTextObjectContent::Image(image)) = &span.content else {
        panic!("expected decoded embedded image");
    };
    image
}

#[test]
fn embedded_images_resolve_media_and_preserve_utf16_anchors_and_raw_records() {
    let payloads = [(3, image(7)), (5, image(9))];
    let parsed = sdocx::parse_bytes_detailed(&with_note(
        archive(
            &one_page(vec![]),
            Some(&[(7, "90@red.png"), (9, "80@blue.png")]),
            &[("80@blue.png", b"blue"), ("90@red.png", b"red")],
        ),
        "🖊\n\u{fffc}\n\u{fffc}",
        &payloads,
    ))
    .unwrap();
    for text in [
        &parsed.note.as_ref().unwrap().body,
        parsed.document.metadata.note_text.as_ref().unwrap(),
    ] {
        assert_eq!(text.object_spans.len(), 2);
        for (span, (index, payload)) in text.object_spans.iter().zip(&payloads) {
            assert_eq!(span.text_index_utf16, *index);
            assert_eq!(span.object_data, *payload);
        }
        for (span, expected) in text
            .object_spans
            .iter()
            .zip([b"red".as_slice(), b"blue".as_slice()])
        {
            let asset =
                &parsed.document.metadata.media_assets[embedded_image(span).media_index.unwrap()];
            assert_eq!(asset.data, expected);
        }
    }
    assert!(
        !parsed
            .report
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::UnresolvedImageMedia)
    );
    #[cfg(feature = "render")]
    {
        let pages = sdocx::render_document_svg(&parsed.document, &sdocx::RenderOptions::default());
        assert_eq!(pages[0].svg.matches("<image ").count(), 2);
        let xml = roxmltree::Document::parse(&pages[0].svg).unwrap();
        for node in xml.descendants().filter(|node| node.has_tag_name("image")) {
            assert_svg_rotation(node, 30.0, [40.0, 60.0]);
        }
    }
}

#[test]
fn missing_and_ambiguous_embedded_media_remain_unresolved_with_diagnostics() {
    for bindings in [
        vec![(7, "missing.png")],
        vec![(7, "red.png"), (7, "blue.png")],
    ] {
        let parsed = sdocx::parse_bytes_detailed(&with_note(
            archive(
                &one_page(vec![]),
                Some(&bindings),
                &[("red.png", b"red"), ("blue.png", b"blue")],
            ),
            "\u{fffc}",
            &[(0, image(7))],
        ))
        .unwrap();
        let span = &parsed
            .document
            .metadata
            .note_text
            .as_ref()
            .unwrap()
            .object_spans[0];
        assert_eq!(embedded_image(span).media_id, Some(7));
        assert_eq!(embedded_image(span).media_index, None);
        assert!(
            parsed
                .report
                .diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::UnresolvedImageMedia
                    && d.archive_entry.as_deref() == Some("note.note"))
        );
        #[cfg(feature = "render")]
        assert!(
            !sdocx::render_document_svg(&parsed.document, &sdocx::RenderOptions::default())[0]
                .svg
                .contains("<image ")
        );
    }
}

#[test]
fn hidden_embedded_images_retain_bytes_without_media_warnings_or_rendering() {
    let mut payload = image(99);
    payload[11] &= !(1 << 3);
    let parsed = sdocx::parse_bytes_detailed(&with_note(
        archive(&one_page(vec![]), None, &[]),
        "\u{fffc}",
        &[(0, payload.clone())],
    ))
    .unwrap();
    let span = &parsed
        .document
        .metadata
        .note_text
        .as_ref()
        .unwrap()
        .object_spans[0];
    assert_eq!(span.object_data, payload);
    assert!(span.content.is_none());
    assert!(!parsed.report.diagnostics.iter().any(|d| matches!(
        d.code,
        DiagnosticCode::UnresolvedImageMedia | DiagnosticCode::UnsupportedImageFeature
    )));
    #[cfg(feature = "render")]
    assert!(
        !sdocx::render_document_svg(&parsed.document, &sdocx::RenderOptions::default())[0]
            .svg
            .contains("<image ")
    );
}

#[test]
fn embedded_image_frames_cannot_borrow_from_sibling_spans() {
    let payload = image(7);
    for end in 0..payload.len() {
        let bytes = with_note(
            archive(&one_page(vec![]), None, &[]),
            "\u{fffc}\u{fffc}",
            &[(0, payload[..end].to_vec()), (1, payload.clone())],
        );
        assert!(
            sdocx::parse_bytes_detailed(&bytes).is_err(),
            "truncated at {end}"
        );
    }
    let bytes = with_note(
        archive(&one_page(vec![]), None, &[]),
        "\u{fffc}",
        &[(0, payload)],
    );
    let options = sdocx::ParseOptions {
        limits: sdocx::ParseLimits {
            max_text_object_spans: 0,
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(matches!(
        sdocx::parse_bytes_detailed_with_options(&bytes, &options),
        Err(Error::LimitExceeded {
            resource: "text object spans",
            ..
        })
    ));
}

#[test]
fn cropped_embedded_images_keep_original_placement_and_clip_the_render() {
    let mut tail = Vec::new();
    for value in [10_i32, 20, 110, 100] {
        tail.extend(value.to_le_bytes());
    }
    for value in [-20.0_f64, 0.0, 180.0, 160.0] {
        tail.extend(value.to_le_bytes());
    }
    let payload = image_with_fill(
        2,
        &fill(7),
        0,
        &[],
        &frame(3, (1 << 1) | (1 << 17), &[], &tail),
    );
    let parsed = sdocx::parse_bytes_detailed(&with_note(
        archive(
            &one_page(vec![]),
            Some(&[(7, "red.png")]),
            &[("red.png", b"red")],
        ),
        "\u{fffc}",
        &[(0, payload)],
    ))
    .unwrap();
    let image = embedded_image(
        &parsed
            .document
            .metadata
            .note_text
            .as_ref()
            .unwrap()
            .object_spans[0],
    );
    assert_eq!(image.crop_rect, Some([10, 20, 110, 100]));
    assert_eq!(
        image.original_bbox,
        Some(sdocx::BoundingBox {
            x_min: -20.0,
            y_min: 0.0,
            x_max: 180.0,
            y_max: 160.0,
        })
    );
    assert!(
        !parsed
            .report
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == DiagnosticCode::UnsupportedImageFeature })
    );
    #[cfg(feature = "render")]
    {
        let svg =
            &sdocx::render_document_svg(&parsed.document, &sdocx::RenderOptions::default())[0].svg;
        let xml = roxmltree::Document::parse(svg).unwrap();
        let node = xml
            .descendants()
            .find(|node| node.has_tag_name("image"))
            .unwrap();
        assert_eq!(svg_rectangle(node), [-20.0, 0.0, 200.0, 160.0]);
        let viewport = node.parent().unwrap();
        assert!(viewport.has_tag_name("svg"));
        assert_eq!(viewport.attribute("overflow"), Some("hidden"));
        let view_box: svgtypes::ViewBox = viewport.attribute("viewBox").unwrap().parse().unwrap();
        assert_eq!(
            [view_box.x, view_box.y, view_box.w, view_box.h],
            [-10.0, 20.0, 100.0, 80.0]
        );
        assert_svg_rotation(viewport.parent().unwrap(), 30.0, [40.0, 60.0]);
    }
}

#[cfg(feature = "render")]
#[test]
fn parsed_fractional_original_rect_uses_integer_bounds_at_the_visible_clip_edge() {
    const SOURCE: &[u8] = include_bytes!("assets/crop-quadrants.png");
    let mut tail: Vec<_> = [0_i32, 0, 4, 3]
        .into_iter()
        .flat_map(i32::to_le_bytes)
        .collect();
    tail.extend(
        [-2.75_f64, -1.75, 7.75, 8.75]
            .into_iter()
            .flat_map(f64::to_le_bytes),
    );
    let mut payload = image_with_fill(
        2,
        &fill(7),
        0,
        &[],
        &frame(3, (1 << 1) | (1 << 17), &[], &tail),
    );
    for (index, value) in [0.0_f64, 0.0, 8.0, 8.0].into_iter().enumerate() {
        payload[36 + index * 8..44 + index * 8].copy_from_slice(&value.to_le_bytes());
    }
    payload[base().len() - 4..base().len()].copy_from_slice(&0.0_f32.to_le_bytes());
    let pages = one_page(vec![object(3, &payload, &[])]);
    let parsed = sdocx::parse_bytes_detailed(&archive(
        &pages,
        Some(&[(7, "quadrants.png")]),
        &[("quadrants.png", SOURCE)],
    ))
    .unwrap();
    let image = placed(parsed.document.pages[0].elements().next().unwrap());
    assert_eq!(image.crop_rect, Some([0, 0, 4, 3]));
    assert_eq!(
        image.original_bbox,
        Some(sdocx::BoundingBox {
            x_min: -2.0,
            y_min: -1.0,
            x_max: 7.0,
            y_max: 8.0,
        })
    );

    let rendered = sdocx::render_page_svg(&parsed.document, 0, &Default::default()).unwrap();
    let xml = roxmltree::Document::parse(&rendered.svg).unwrap();
    let node = xml
        .descendants()
        .find(|node| node.has_tag_name("image"))
        .unwrap();
    assert_eq!(svg_rectangle(node), [-2.0, -1.0, 9.0, 9.0]);
    let viewport = node.parent().unwrap();
    assert_eq!(viewport.attribute("overflow"), Some("hidden"));
    let view_box: svgtypes::ViewBox = viewport.attribute("viewBox").unwrap().parse().unwrap();
    assert_eq!(
        [view_box.x, view_box.y, view_box.w, view_box.h],
        [0.0, 0.0, 8.0, 8.0]
    );

    let tree = resvg::usvg::Tree::from_str(&rendered.svg, &Default::default()).unwrap();
    let mut pixels = resvg::tiny_skia::Pixmap::new(rendered.width, rendered.height).unwrap();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixels.as_mut(),
    );
    let background = [252, 252, 252, 255];
    let edge = pixels.pixel(7, 1).unwrap();
    assert_eq!(
        [edge.red(), edge.green(), edge.blue(), edge.alpha()],
        background
    );
    let interior = pixels.pixel(0, 1).unwrap();
    assert_ne!(
        [
            interior.red(),
            interior.green(),
            interior.blue(),
            interior.alpha()
        ],
        background
    );
}

#[test]
fn parsed_image_original_rect_matches_native_integer_conversion_controls() {
    for (values, expected) in [
        ([-2.75, -1.75, 7.75, 8.75], [-2.0, -1.0, 7.0, 8.0]),
        (
            [
                2_147_483_647.75,
                -2_147_483_648.75,
                2_147_483_648.75,
                -2_147_483_649.75,
            ],
            [
                2_147_483_647.0,
                -2_147_483_648.0,
                -2_147_483_648.0,
                2_147_483_647.0,
            ],
        ),
        (
            [
                4_294_967_303.75,
                -4_294_967_299.25,
                8_589_934_592.5,
                -8_589_934_592.5,
            ],
            [7.0, -3.0, 0.0, 0.0],
        ),
        ([1e100, -1e100, 1e300, -1e300], [-1.0, 0.0, -1.0, 0.0]),
    ] {
        let mut tail: Vec<_> = [1_i32, 2, 3, 4]
            .into_iter()
            .flat_map(i32::to_le_bytes)
            .collect();
        tail.extend(values.into_iter().flat_map(f64::to_le_bytes));
        let payload = image_with_fill(
            2,
            &fill(7),
            0,
            &[],
            &frame(3, (1 << 1) | (1 << 17), &[], &tail),
        );
        let parsed = parse_native_image(&payload);
        let span = &parsed.note.as_ref().unwrap().body.object_spans[0];
        let image = embedded_image(span);
        assert_eq!(image.crop_rect, Some([1, 2, 3, 4]));
        assert_eq!(
            image.original_bbox,
            Some(sdocx::BoundingBox {
                x_min: expected[0],
                y_min: expected[1],
                x_max: expected[2],
                y_max: expected[3],
            })
        );
        assert!(
            !parsed
                .report
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.code == DiagnosticCode::UnsupportedImageFeature })
        );
        assert_eq!(span.object_data, payload);
    }
}

#[test]
fn image_flow_sections_render_each_anchor_once_and_preserve_page_margins() {
    let mut document = sdocx::parse_bytes_detailed(&with_note(
        archive(
            &one_page(vec![]),
            Some(&[(7, "red.png")]),
            &[("red.png", b"red")],
        ),
        "\u{fffc}\n\u{fffc}\n",
        &[(0, image(7)), (2, image(7))],
    ))
    .unwrap()
    .document;
    document.pages = vec![document.pages[0].clone(); 3];
    document.metadata.note_text.as_mut().unwrap().text_sections = vec![
        sdocx::RichTextSection {
            start_utf16: 0,
            length_utf16: 2,
        },
        sdocx::RichTextSection {
            start_utf16: 2,
            length_utf16: 2,
        },
        sdocx::RichTextSection {
            start_utf16: 4,
            length_utf16: 0,
        },
    ];
    let layout = sdocx::layout_document(&document);
    assert_eq!(layout.pages.len(), 3);
    for page in layout.pages.iter().take(2) {
        let PageElement::TextBox(text) = page.page.elements().next().unwrap() else {
            panic!("text flow")
        };
        assert_eq!(text.object_spans.len(), 1);
        assert_eq!(text.object_spans[0].text_index_utf16, 0);
        assert_eq!(text.margins.unwrap()[1], 10.0);
    }
    assert!(layout.pages[2].body_text_slice().is_none());
    #[cfg(feature = "render")]
    for (index, page) in sdocx::render_document_svg(&document, &sdocx::RenderOptions::default())
        .iter()
        .enumerate()
    {
        assert_eq!(page.svg.matches("<image ").count(), usize::from(index < 2));
    }
}

#[test]
fn hidden_images_keep_media_references_without_resolving_or_drawing_them() {
    let mut payload = image(42);
    payload[11] &= !(1 << 3);
    let raw = page(&[vec![object(3, &payload, &[])]], 0, &[]);
    let parsed = sdocx::parse_bytes_detailed(&support::archive(&raw)).unwrap();
    assert!(parsed.document.pages[0].elements().next().is_none());
    let stored = &parsed.stored_pages[0].page.layers.layers[0].objects[0];
    assert_eq!(stored.payload(&raw).unwrap(), payload);
    assert!(!stored.base_metadata(&raw).unwrap().visible);
    assert!(
        !parsed
            .report
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == DiagnosticCode::UnresolvedImageMedia })
    );
    #[cfg(feature = "render")]
    {
        let rendered =
            sdocx::render_page_svg(&parsed.document, 0, &sdocx::RenderOptions::default()).unwrap();
        assert!(!rendered.svg.contains("<image "));
    }
}

fn manifest(bindings: &[(u32, &str)]) -> Vec<u8> {
    let mut bytes = 5500_u32.to_le_bytes().to_vec();
    bytes.extend_from_slice(&(bindings.len() as u16).to_le_bytes());
    for (id, name) in bindings {
        let mut entry = id.to_le_bytes().to_vec();
        entry.extend_from_slice(&(name.encode_utf16().count() as u16).to_le_bytes());
        for unit in name.encode_utf16() {
            entry.extend_from_slice(&unit.to_le_bytes());
        }
        entry.extend_from_slice(&[0; 12]); // empty hash, ref count, timestamp
        entry.push(1);
        bytes.extend_from_slice(&(entry.len() as u32).to_le_bytes());
        bytes.extend(entry);
    }
    bytes.extend_from_slice(b"EOFX");
    bytes
}

fn archive(
    pages: &[(&str, Vec<u8>)],
    bindings: Option<&[(u32, &str)]>,
    assets: &[(&str, &[u8])],
) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in assets {
        writer
            .start_file(
                format!("media/{name}"),
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        writer.write_all(data).unwrap();
    }
    if let Some(bindings) = bindings {
        writer
            .start_file(
                "media/mediaInfo.dat",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        writer.write_all(&manifest(bindings)).unwrap();
    }
    for (name, data) in pages {
        writer
            .start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(data).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn one_page(objects: Vec<Vec<u8>>) -> [(&'static str, Vec<u8>); 1] {
    [("page.page", page(&[objects], 0, &[]))]
}
fn placed(element: &PageElement) -> &PlacedImage {
    let PageElement::PlacedImage(image) = element else {
        panic!("expected structural image")
    };
    image
}
fn asset_bytes<'a>(doc: &'a sdocx::Document, element: &PageElement) -> &'a [u8] {
    &doc.metadata.media_assets[placed(element).media_index.unwrap()].data
}

#[cfg(feature = "render")]
fn svg_rectangle(node: roxmltree::Node<'_, '_>) -> [f64; 4] {
    ["x", "y", "width", "height"].map(|name| node.attribute(name).unwrap().parse().unwrap())
}

#[cfg(feature = "render")]
fn assert_svg_rotation(node: roxmltree::Node<'_, '_>, angle: f64, pivot: [f64; 2]) {
    use svgtypes::TransformListToken::{Rotate, Translate};
    let actual: Vec<_> = svgtypes::TransformListParser::from(node.attribute("transform").unwrap())
        .map(Result::unwrap)
        .collect();
    assert_eq!(
        actual,
        [
            Translate {
                tx: pivot[0],
                ty: pivot[1]
            },
            Rotate { angle },
            Translate {
                tx: -pivot[0],
                ty: -pivot[1]
            }
        ]
    );
}

#[test]
fn manifest_ids_override_filename_prefixes_archive_order_and_encounter_order() {
    let pages = one_page(vec![
        object(3, &image(42), &[]),
        object(3, &image(7), &[]),
        object(3, &image(42), &[]),
    ]);
    let bindings = [(7, "9@red.png"), (42, "2@blue.png")];
    let assets = [
        ("9@red.png", &b"red"[..]),
        ("0@unused.png", &b"unused"[..]),
        ("2@blue.png", &b"blue"[..]),
    ];
    for files in [assets.to_vec(), assets.into_iter().rev().collect()] {
        let parsed =
            sdocx::parse_bytes_detailed(&archive(&pages, Some(&bindings), &files)).unwrap();
        let doc = &parsed.document;
        let elements: Vec<_> = doc.pages[0].elements().collect();
        assert_eq!(elements.len(), 3);
        assert_eq!(asset_bytes(doc, elements[0]), b"blue");
        assert_eq!(asset_bytes(doc, elements[1]), b"red");
        assert_eq!(asset_bytes(doc, elements[2]), b"blue");
        assert_eq!(placed(elements[0]).media_id, Some(42));
        assert_eq!(
            placed(elements[0]).media_index,
            placed(elements[2]).media_index
        );
        assert_eq!(placed(elements[0]).rotation_degrees, Some(30.0));
        assert_eq!(placed(elements[0]).bbox.x_min, -10.0);
        assert!(!parsed.report.diagnostics.iter().any(|d| matches!(
            d.code,
            DiagnosticCode::UnresolvedImageMedia
                | DiagnosticCode::InferredImageMediaReference
                | DiagnosticCode::UnsupportedImageFeature
        )));
        #[cfg(feature = "render")]
        {
            let svg = sdocx::render_page_svg(doc, 0, &sdocx::RenderOptions::default())
                .unwrap()
                .svg;
            assert_eq!(svg.matches("<image ").count(), 3);
            assert_eq!(svg.matches("data:image/png;base64,Ymx1ZQ==").count(), 2);
            assert!(svg.contains("data:image/png;base64,cmVk"));
            let xml = roxmltree::Document::parse(&svg).unwrap();
            for node in xml.descendants().filter(|node| node.has_tag_name("image")) {
                assert_eq!(svg_rectangle(node), [-10.0, 20.0, 100.0, 80.0]);
                assert_svg_rotation(node, 30.0, [40.0, 60.0]);
            }
            assert!(!svg.contains("dW51c2Vk"));
        }
    }
}

#[test]
fn missing_unsupported_and_ambiguous_bindings_never_select_a_different_asset() {
    let pages = one_page(vec![object(3, &image(42), &[])]);
    let assets = [
        ("42@decoy.png", &b"decoy"[..]),
        ("other.png", &b"other"[..]),
        ("document.pdf", &b"pdf"[..]),
    ];
    let cases: &[(&[(u32, &str)], &str)] = &[
        (&[], "no binding"),
        (&[(42, "missing.png")], "missing archive entry"),
        (&[(42, "document.pdf")], "unsupported media"),
        (
            &[(42, "42@decoy.png"), (42, "other.png")],
            "ambiguous bindings",
        ),
    ];
    for (bindings, message) in cases {
        let parsed =
            sdocx::parse_bytes_detailed(&archive(&pages, Some(bindings), &assets)).unwrap();
        assert_eq!(parsed.document.pages[0].elements().count(), 1);
        assert_eq!(
            placed(parsed.document.pages[0].elements().next().unwrap()).media_index,
            None
        );
        let finding = parsed
            .report
            .diagnostics
            .iter()
            .find(|d| d.code == DiagnosticCode::UnresolvedImageMedia)
            .unwrap();
        assert_eq!(finding.archive_entry.as_deref(), Some("page.page"));
        assert!(finding.message.contains("image at 0x"));
        assert!(finding.message.contains(message), "{}", finding.message);
        #[cfg(feature = "render")]
        assert!(
            !sdocx::render_page_svg(&parsed.document, 0, &sdocx::RenderOptions::default())
                .unwrap()
                .svg
                .contains("<image ")
        );
    }
}

#[test]
fn absent_manifest_allows_only_an_unambiguous_numeric_id_with_a_warning() {
    let pages = one_page(vec![object(3, &image(7), &[])]);
    let assets = [
        ("1@unused.png", &b"unused"[..]),
        ("7@actual.png", &b"actual"[..]),
    ];
    let parsed = sdocx::parse_bytes_detailed(&archive(&pages, None, &assets)).unwrap();
    assert_eq!(
        asset_bytes(
            &parsed.document,
            parsed.document.pages[0].elements().next().unwrap()
        ),
        b"actual"
    );
    assert!(
        parsed
            .report
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::InferredImageMediaReference)
    );
    for name in ["7@duplicate.png", "7@unsupported.spi"] {
        let mut duplicate = assets.to_vec();
        duplicate.push((name, b"different"));
        let parsed = sdocx::parse_bytes_detailed(&archive(&pages, None, &duplicate)).unwrap();
        assert!(
            placed(parsed.document.pages[0].elements().next().unwrap())
                .media_index
                .is_none()
        );
        assert!(
            parsed
                .report
                .diagnostics
                .iter()
                .any(|d| d.message.contains("ambiguous bindings"))
        );
    }
}

#[test]
fn image_references_are_global_across_pages_layers_and_children() {
    let pages = [
        ("b.page", page(&[vec![object(3, &image(7), &[])]], 0, &[])),
        (
            "a.page",
            page_with_current_layer(
                &[
                    vec![object(250, &image(7), &[])],
                    vec![object(4, b"group", &[object(3, &image(42), &[])])],
                ],
                1,
                0,
                &[],
            ),
        ),
    ];
    let parsed = sdocx::parse_bytes_detailed(&archive(
        &pages,
        Some(&[(42, "blue.png"), (7, "red.png")]),
        &[("red.png", b"red"), ("blue.png", b"blue")],
    ))
    .unwrap();
    assert_eq!(parsed.document.pages.len(), 2);
    assert_eq!(parsed.document.pages[0].elements().count(), 1);
    assert_eq!(
        asset_bytes(
            &parsed.document,
            parsed.document.pages[0].elements().next().unwrap()
        ),
        b"blue"
    );
    assert_eq!(
        asset_bytes(
            &parsed.document,
            parsed.document.pages[1].elements().next().unwrap()
        ),
        b"red"
    );
}

#[test]
fn optional_fields_and_border_original_ids_cannot_replace_the_displayed_image() {
    let mut prefix = 4_u32.to_le_bytes().to_vec();
    prefix.extend_from_slice(b"text");
    prefix.push(1);
    prefix.extend_from_slice(&99_u32.to_le_bytes());
    prefix.extend_from_slice(&0xff123456_u32.to_le_bytes());
    let mut tail = Vec::new();
    for value in [1_i32, 2, 30, 40] {
        tail.extend_from_slice(&value.to_le_bytes());
    }
    tail.extend_from_slice(&8_u32.to_le_bytes());
    tail.extend_from_slice(&9_u32.to_le_bytes());
    let payload = image_with_fill(
        2,
        &fill(7),
        1 | 2 | 4 | 16,
        &prefix,
        &frame(3, 2 | 512 | (1 << 18), &[], &tail),
    );
    let parsed = sdocx::parse_bytes_detailed(&archive(
        &one_page(vec![object(3, &payload, &[])]),
        Some(&[(7, "main.png"), (8, "border.png"), (9, "original.png")]),
        &[
            ("border.png", b"border"),
            ("original.png", b"original"),
            ("main.png", b"main"),
        ],
    ))
    .unwrap();
    let img = placed(parsed.document.pages[0].elements().next().unwrap());
    assert_eq!(img.media_id, Some(7));
    assert_eq!(img.crop_rect, Some([1, 2, 30, 40]));
    assert_eq!(img.border_media_id, Some(8));
    assert_eq!(img.original_media_id, Some(9));
    assert_eq!(
        asset_bytes(
            &parsed.document,
            parsed.document.pages[0].elements().next().unwrap()
        ),
        b"main"
    );
    assert!(
        parsed
            .report
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::UnsupportedImageFeature)
    );
}

#[test]
fn malformed_image_frames_cannot_consume_siblings_or_return_partial_pages() {
    let valid = image(7);
    let shape = base().len() + frame(6, 0, &[], &[]).len();
    let fill_offset = shape + 17 + shape_fixed().len();
    let mut mutations: Vec<_> = (0..valid.len()).map(|end| valid[..end].to_vec()).collect();
    for offset in [0, shape, fill_offset] {
        let mut bytes = valid.clone();
        bytes[offset..offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        mutations.push(bytes);
    }
    let mut wrong_kind = valid.clone();
    wrong_kind[shape + 4..shape + 6].copy_from_slice(&3_i16.to_le_bytes());
    mutations.push(wrong_kind);
    let mut nan = fill(7);
    nan[5..9].copy_from_slice(&f32::NAN.to_le_bytes());
    mutations.push(image_with_fill(2, &nan, 0, &[], &frame(3, 0, &[], &[])));
    mutations.push(image_with_fill(
        2,
        &fill(7),
        0,
        &[],
        &frame(3, 512, &[], &[0; 3]),
    ));
    for invalid in mutations {
        let bytes = support::archive(&page(
            &[vec![
                object(3, &valid, &[]),
                object(3, &invalid, &[]),
                object(3, &valid, &[]),
            ]],
            0,
            &[],
        ));
        let error = sdocx::parse_bytes(&bytes).unwrap_err();
        assert!(matches!(error, Error::Format(_)), "{error}");
        assert!(
            error.to_string().contains("page page: image at 0x"),
            "{error}"
        );
    }
}

#[test]
fn unknown_fills_and_negative_ids_do_not_invent_bindings() {
    for payload in [
        image(-1),
        image(i32::MIN),
        image_with_fill(3, &fill(7), 0, &[], &frame(3, 0, &[], &[])),
        image_with_fill(2, &[b'a'; 122], 0, &[], &frame(3, 0, &[], &[])),
        image_with_fill(2, &fill(7), 8, b"unknown", &frame(3, 0, &[], &[])),
    ] {
        let parsed = sdocx::parse_bytes_detailed(&archive(
            &one_page(vec![object(3, &payload, &[])]),
            Some(&[(7, "main.png")]),
            &[("main.png", b"main")],
        ))
        .unwrap();
        let img = placed(parsed.document.pages[0].elements().next().unwrap());
        assert_eq!(img.media_id, None);
        assert_eq!(img.media_index, None);
        assert!(
            parsed
                .report
                .diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::UnresolvedImageMedia)
        );
    }
}

#[test]
fn zero_id_tiny_bounds_and_wider_masks_preserve_the_reference() {
    let mut flexible = 62_u32.to_le_bytes().to_vec();
    flexible.push(2);
    flexible.extend(fill(0));
    flexible.extend_from_slice(b"future payload contains decoy media ID 7");
    let mut shape = frame(7, 32, &shape_fixed(), &flexible);
    // Five-byte mask with a future bit: all following fields move by one byte.
    shape[12] = 5;
    shape.insert(17, 1);
    let size = shape.len() as u32;
    shape[..4].copy_from_slice(&size.to_le_bytes());
    shape[6..10].copy_from_slice(&((18 + shape_fixed().len()) as u32).to_le_bytes());
    let mut payload = base();
    for (index, value) in [0.0_f64, 0.0, 0.5, 0.25].into_iter().enumerate() {
        payload[36 + index * 8..44 + index * 8].copy_from_slice(&value.to_le_bytes());
    }
    payload.extend(frame(6, 0, &[], &[]));
    payload.extend(shape);
    payload.extend(frame(3, 1 << 31, &[], b"future settings"));
    payload.extend(frame(123, 0, &[], &[]));
    let parsed = sdocx::parse_bytes_detailed(&archive(
        &one_page(vec![object(3, &payload, &[])]),
        Some(&[(0, "zero.png")]),
        &[("zero.png", b"zero")],
    ))
    .unwrap();
    let image = placed(parsed.document.pages[0].elements().next().unwrap());
    assert_eq!(image.media_id, Some(0));
    assert_eq!(image.bbox.x_min, 0.0);
    assert_eq!(image.bbox.x_max, 0.5);
    assert_eq!(image.bbox.y_max, 0.25);
    assert_eq!(
        asset_bytes(
            &parsed.document,
            parsed.document.pages[0].elements().next().unwrap()
        ),
        b"zero"
    );
    let warning = parsed
        .report
        .diagnostics
        .iter()
        .find(|d| d.code == DiagnosticCode::UnsupportedImageFeature)
        .unwrap();
    assert!(warning.message.contains("additional shape fields"));
    assert!(warning.message.contains("additional image frames"));
}

#[test]
fn images_obey_object_limits_and_do_not_scan_non_image_payloads() {
    let payload = image(7);
    let raw = page(&[vec![object(3, &payload, &[])]], 0, &[]);
    let options = sdocx::ParseOptions {
        limits: sdocx::ParseLimits {
            max_objects_per_page: 0,
            ..sdocx::ParseLimits::default()
        },
        ..Default::default()
    };
    assert!(matches!(
        sdocx::parse_bytes_with_options(&support::archive(&raw), &options),
        Err(Error::LimitExceeded { .. })
    ));
    let bytes = archive(
        &one_page(vec![object(250, &payload, &[])]),
        Some(&[(7, "main.png")]),
        &[("main.png", b"main")],
    );
    let parsed = sdocx::parse_bytes_detailed(&bytes).unwrap();
    assert!(parsed.document.pages[0].elements().next().is_none());
    assert!(
        parsed
            .report
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::UnknownObjectType)
    );
}

#[test]
fn no_fill_means_no_main_image_even_when_an_original_asset_exists() {
    let mut payload = base();
    payload.extend(frame(6, 0, &[], &[]));
    payload.extend(frame(7, 0, &shape_fixed(), &[]));
    payload.extend(frame(3, 1 << 18, &[], &7_u32.to_le_bytes()));
    let parsed = sdocx::parse_bytes_detailed(&archive(
        &one_page(vec![object(3, &payload, &[])]),
        Some(&[(7, "original.png")]),
        &[("original.png", b"original")],
    ))
    .unwrap();
    let image = placed(parsed.document.pages[0].elements().next().unwrap());
    assert_eq!(image.media_id, None);
    assert_eq!(image.original_media_id, Some(7));
    assert_eq!(image.media_index, None);
}

#[cfg(feature = "render")]
#[test]
fn saved_image_placement_preserves_fractional_attributes_and_rotation() {
    let bounds = [1.0009765625_f64, 2.0009765625, 1.0029296875, 2.00390625];
    let mut payload = image(7);
    for (index, value) in bounds.into_iter().enumerate() {
        payload[36 + index * 8..44 + index * 8].copy_from_slice(&value.to_le_bytes());
    }
    payload[base().len() - 4..base().len()]
        .copy_from_slice(&f32::from_bits(0x41f8_0100).to_le_bytes());
    let pages = one_page(vec![object(3, &payload, &[])]);
    let parsed = sdocx::parse_bytes_detailed(&archive(
        &pages,
        Some(&[(7, "main.png")]),
        &[("main.png", b"precision")],
    ))
    .unwrap();
    let element = parsed.document.pages[0].elements().next().unwrap();
    let svg = sdocx::render_page_svg(&parsed.document, 0, &Default::default())
        .unwrap()
        .svg;
    let xml = roxmltree::Document::parse(&svg).unwrap();
    let node = xml
        .descendants()
        .find(|node| node.has_tag_name("image"))
        .unwrap();
    assert_eq!(
        svg_rectangle(node),
        [1.0009765625, 2.0009765625, 0.001953125, 0.0029296875]
    );
    assert_svg_rotation(node, 31.00048828125, [1.001953125, 2.00244140625]);
    let image = placed(element);
    assert_eq!(image.bbox.x_min, bounds[0]);
    assert_eq!(image.bbox.y_min, bounds[1]);
    assert_eq!(image.bbox.x_max, bounds[2]);
    assert_eq!(image.bbox.y_max, bounds[3]);
    assert_eq!(image.rotation_degrees, Some(31.00048828125));
    assert_eq!(image.media_id, Some(7));
    assert_eq!(asset_bytes(&parsed.document, element), b"precision");
    let stored = &parsed.stored_pages[0].page.layers.layers[0].objects[0];
    assert_eq!(stored.payload(&pages[0].1).unwrap(), payload);
}

#[cfg(feature = "render")]
#[test]
fn saved_image_crop_preserves_fine_viewport_and_integer_original_bounds() {
    let unit = 1.0_f64 / 131072.0;
    let bounds = [1.0 + unit, 2.0 + unit, 1.0 + 3.0 * unit, 2.0 + 5.0 * unit];
    let mut tail = Vec::new();
    for value in [10_i32, 20, 110, 100] {
        tail.extend(value.to_le_bytes());
    }
    for value in [-20.0_f64, 0.0, 180.0, 160.0] {
        tail.extend(value.to_le_bytes());
    }
    let mut payload = image_with_fill(2, &fill(7), 0, &[], &frame(3, 2 | (1 << 17), &[], &tail));
    for (index, value) in bounds.into_iter().enumerate() {
        payload[36 + index * 8..44 + index * 8].copy_from_slice(&value.to_le_bytes());
    }
    payload[base().len() - 4..base().len()]
        .copy_from_slice(&f32::from_bits(0x41f8_0100).to_le_bytes());
    let pages = one_page(vec![object(3, &payload, &[])]);
    let parsed = sdocx::parse_bytes_detailed(&archive(
        &pages,
        Some(&[(7, "main.png")]),
        &[("main.png", b"precision")],
    ))
    .unwrap();
    let element = parsed.document.pages[0].elements().next().unwrap();
    let svg = sdocx::render_page_svg(&parsed.document, 0, &Default::default())
        .unwrap()
        .svg;
    let xml = roxmltree::Document::parse(&svg).unwrap();
    let node = xml
        .descendants()
        .find(|node| node.has_tag_name("image"))
        .unwrap();
    let viewport = node.parent().unwrap();
    assert!(viewport.has_tag_name("svg"));
    assert_eq!(viewport.attribute("overflow"), Some("hidden"));
    let expected = [1.0 + unit, 2.0 + unit, 2.0 * unit, 4.0 * unit];
    for (node, expected) in [(viewport, expected), (node, [-20.0, 0.0, 200.0, 160.0])] {
        assert_eq!(svg_rectangle(node), expected);
    }
    let view_box: svgtypes::ViewBox = viewport.attribute("viewBox").unwrap().parse().unwrap();
    assert_eq!([view_box.x, view_box.y, view_box.w, view_box.h], expected);
    assert_svg_rotation(
        viewport.parent().unwrap(),
        31.00048828125,
        [1.0 + 2.0 * unit, 2.0 + 3.0 * unit],
    );
    let image = placed(element);
    assert_eq!(image.bbox.x_min, bounds[0]);
    assert_eq!(image.bbox.y_min, bounds[1]);
    assert_eq!(image.bbox.x_max, bounds[2]);
    assert_eq!(image.bbox.y_max, bounds[3]);
    assert_eq!(image.crop_rect, Some([10, 20, 110, 100]));
    let original = image.original_bbox.unwrap();
    assert_eq!(original.x_min, -20.0);
    assert_eq!(original.y_min, 0.0);
    assert_eq!(original.x_max, 180.0);
    assert_eq!(original.y_max, 160.0);
    assert_eq!(image.rotation_degrees, Some(31.00048828125));
    assert_eq!(image.media_id, Some(7));
    assert_eq!(asset_bytes(&parsed.document, element), b"precision");
    let stored = &parsed.stored_pages[0].page.layers.layers[0].objects[0];
    assert_eq!(stored.payload(&pages[0].1).unwrap(), payload);
}

#[cfg(feature = "render")]
#[test]
fn pixel_crop_without_original_placement_selects_the_source_region() {
    const SOURCE: &[u8] = include_bytes!("assets/crop-quadrants.png");
    let tail: Vec<_> = [1_i32, 2, 3, 3]
        .into_iter()
        .flat_map(i32::to_le_bytes)
        .collect();
    let payload = image_with_fill(2, &fill(7), 0, &[], &frame(3, 1 << 1, &[], &tail));
    let parsed = sdocx::parse_bytes_detailed(&archive(
        &one_page(vec![object(3, &payload, &[])]),
        Some(&[(7, "quadrants.png")]),
        &[("quadrants.png", SOURCE)],
    ))
    .unwrap();
    let mut document = parsed.document;
    let mut image = placed(document.pages[0].elements().next().unwrap()).clone();
    assert_eq!(image.crop_rect, Some([1, 2, 3, 3]));
    assert_eq!(image.original_bbox, None);
    image.bbox = sdocx::BoundingBox {
        x_min: 10.0,
        y_min: 20.0,
        x_max: 50.0,
        y_max: 50.0,
    };
    image.rotation_degrees = Some(0.0);
    document.pages[0].objects = vec![PageElement::PlacedImage(image.clone()).into()];
    let rendered = sdocx::render_page_svg(&document, 0, &Default::default()).unwrap();
    let xml = roxmltree::Document::parse(&rendered.svg).unwrap();
    let node = xml.descendants().find(|n| n.has_tag_name("image")).unwrap();
    assert_eq!(svg_rectangle(node), [10.0, 20.0, 40.0, 30.0]);
    assert_eq!(node.parent().unwrap().attribute("overflow"), None);
    use base64::Engine as _;
    let href = node.attribute("href").expect("embedded source crop");
    let cropped_png = base64::engine::general_purpose::STANDARD
        .decode(href.strip_prefix("data:image/png;base64,").unwrap())
        .unwrap();
    let (crop_width, crop_height, cropped_pixels) = decode_rgba8_png(&cropped_png);
    assert_eq!((crop_width, crop_height), (2, 1));
    let (source_width, _, source_pixels) = decode_rgba8_png(SOURCE);
    let crop_start = (2 * source_width as usize + 1) * 4;
    assert_eq!(
        cropped_pixels,
        source_pixels[crop_start..crop_start + 2 * 4]
    );
    let tree = resvg::usvg::Tree::from_str(&rendered.svg, &Default::default()).unwrap();
    let mut pixels = resvg::tiny_skia::Pixmap::new(rendered.width, rendered.height).unwrap();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixels.as_mut(),
    );
    // The selected bottom-row source pixels are blue/yellow, not red/green.
    for (x, expected) in [(15, [0, 0, 255, 255]), (45, [255, 255, 0, 255])] {
        let pixel = pixels.pixel(x, 45).unwrap();
        assert_eq!(
            [pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()],
            expected
        );
    }
    assert_eq!(
        document.metadata.media_assets[image.media_index.unwrap()].data,
        SOURCE
    );
}

#[cfg(feature = "render")]
#[test]
fn image_rendering_keeps_extent_and_cropped_transform_admission() {
    let mut document = sdocx::parse_bytes(&archive(
        &one_page(vec![object(3, &image(7), &[])]),
        Some(&[(7, "main.png")]),
        &[("main.png", b"precision")],
    ))
    .unwrap();
    let original = placed(document.pages[0].elements().next().unwrap()).clone();
    for (bounds, angle, cropped, visible) in [
        ([0.0, 0.0, 0.0, 1.0], 30.0, false, false),
        ([1.0, 0.0, 0.0, 1.0], 30.0, false, false),
        ([0.0, 0.0, 1.0, f64::INFINITY], 30.0, false, false),
        ([0.0, 0.0, 1.0, 1.0], f64::NAN, true, false),
        ([f64::MAX / 2.0, 0.0, f64::MAX, 1.0], 30.0, true, false),
        ([0.0, 0.0, 1.0, 1.0], f64::NAN, false, true),
    ] {
        let mut image = original.clone();
        image.bbox = sdocx::BoundingBox {
            x_min: bounds[0],
            y_min: bounds[1],
            x_max: bounds[2],
            y_max: bounds[3],
        };
        image.rotation_degrees = Some(angle);
        if cropped {
            image.crop_rect = Some([0, 0, 1, 1]);
            image.original_bbox = Some(original.bbox);
        }
        document.pages[0].objects = vec![PageElement::PlacedImage(image).into()];
        let svg = sdocx::render_page_svg(&document, 0, &Default::default())
            .unwrap()
            .svg;
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let images: Vec<_> = xml
            .descendants()
            .filter(|node| node.has_tag_name("image"))
            .collect();
        assert_eq!(images.len(), usize::from(visible));
        if let Some(image) = images.first() {
            assert!(image.attribute("transform").is_none());
        }
    }
}

#[cfg(feature = "render")]
#[test]
fn legacy_image_values_still_render_with_their_explicit_asset_index() {
    let bytes = archive(
        &one_page(vec![object(3, &image(7), &[])]),
        Some(&[(7, "main.png")]),
        &[("main.png", b"main")],
    );
    let mut document = sdocx::parse_bytes(&bytes).unwrap();
    let image = placed(document.pages[0].elements().next().unwrap());
    document.pages[0].objects = vec![
        PageElement::Image {
            bbox: image.bbox,
            media_index: image.media_index.unwrap(),
        }
        .into(),
    ];
    let svg = sdocx::render_page_svg(&document, 0, &sdocx::RenderOptions::default())
        .unwrap()
        .svg;
    assert_eq!(svg.matches("<image ").count(), 1);
    assert!(svg.contains("data:image/png;base64,bWFpbg=="));
}

#[test]
fn matching_shape_rotation_is_not_mistaken_for_a_corner_radius() {
    let mut payload = image(7);
    let rotation_offset = base().len() + frame(6, 0, &[], &[]).len() + 17 + 4 + 32;
    payload[rotation_offset..rotation_offset + 4].copy_from_slice(&30.0_f32.to_le_bytes());
    let parsed = sdocx::parse_bytes_detailed(&archive(
        &one_page(vec![object(3, &payload, &[])]),
        Some(&[(7, "7@image.png")]),
        &[("7@image.png", b"image")],
    ))
    .unwrap();
    assert_eq!(
        placed(parsed.document.pages[0].elements().next().unwrap()).rotation_degrees,
        Some(30.0)
    );
    assert!(
        !parsed
            .report
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::UnsupportedImageFeature)
    );
}

#[test]
fn type3_field7_transparency_is_parsed_without_hiding_unknown_fields() {
    for value in [0.0_f32, 0.5, 1.0, -0.25, 1.25] {
        let parsed = parse_native_image(&image_with_transparency(7, value));
        let image = embedded_image(&parsed.note.as_ref().unwrap().body.object_spans[0]);
        assert_eq!(image.fill_effect_transparency.to_bits(), value.to_bits());
        assert!(
            !parsed
                .report
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.code == DiagnosticCode::UnsupportedImageFeature })
        );
    }

    #[cfg(feature = "serde")]
    {
        let legacy: PlacedImage = serde_json::from_value(serde_json::json!({
            "bbox": {"x_min": 0.0, "y_min": 0.0, "x_max": 10.0, "y_max": 10.0}
        }))
        .unwrap();
        assert_eq!(legacy.fill_effect_transparency, 0.0);
    }

    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let malformed = archive(
            &one_page(vec![object(3, &image_with_transparency(7, value), &[])]),
            None,
            &[],
        );
        assert!(sdocx::parse_bytes_detailed(&malformed).is_err());
    }

    let unknown = parse_native_image(&image_with_fill(
        2,
        &fill(7),
        0,
        &[],
        &frame(3, 1 << 8, &[], &[]),
    ));
    assert!(
        unknown
            .report
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == DiagnosticCode::UnsupportedImageFeature })
    );
}

#[cfg(feature = "render")]
#[test]
fn parsed_type3_transparency_wraps_only_the_cropped_image_in_svg() {
    const SOURCE: &[u8] = include_bytes!("assets/crop-quadrants.png");
    let mut tail: Vec<_> = [0_i32, 0, 2, 2]
        .into_iter()
        .flat_map(i32::to_le_bytes)
        .collect();
    tail.extend(0.5_f32.to_le_bytes());
    tail.extend(
        [0.0_f64, 0.0, 4.0, 3.0]
            .into_iter()
            .flat_map(f64::to_le_bytes),
    );
    let payload = image_with_fill(
        2,
        &fill(7),
        0,
        &[],
        &frame(3, (1 << 1) | (1 << 7) | (1 << 17), &[], &tail),
    );
    let parsed = sdocx::parse_bytes_detailed(&archive(
        &one_page(vec![object(3, &payload, &[])]),
        Some(&[(7, "quadrants.png")]),
        &[("quadrants.png", SOURCE)],
    ))
    .unwrap();
    let svg = sdocx::render_page_svg(&parsed.document, 0, &Default::default())
        .unwrap()
        .svg;
    let xml = roxmltree::Document::parse(&svg).unwrap();
    let image = xml
        .descendants()
        .find(|node| node.has_tag_name("image"))
        .unwrap();
    assert!(image.parent().unwrap().has_tag_name("svg"));
    let opacity = image
        .ancestors()
        .find(|node| node.attribute("opacity").is_some())
        .unwrap();
    let emitted: f64 = opacity.attribute("opacity").unwrap().parse().unwrap();
    assert!((emitted - 127.0 / 255.0).abs() < 0.000_001);
    assert_eq!(svg_rectangle(image), [0.0, 0.0, 4.0, 3.0]);

    let mut opaque_tail: Vec<_> = [0_i32, 0, 2, 2]
        .into_iter()
        .flat_map(i32::to_le_bytes)
        .collect();
    opaque_tail.extend(
        [0.0_f64, 0.0, 4.0, 3.0]
            .into_iter()
            .flat_map(f64::to_le_bytes),
    );
    let opaque_payload = image_with_fill(
        2,
        &fill(7),
        0,
        &[],
        &frame(3, (1 << 1) | (1 << 17), &[], &opaque_tail),
    );
    let opaque = sdocx::parse_bytes_detailed(&archive(
        &one_page(vec![object(3, &opaque_payload, &[])]),
        Some(&[(7, "quadrants.png")]),
        &[("quadrants.png", SOURCE)],
    ))
    .unwrap();
    let opaque_svg = sdocx::render_page_svg(&opaque.document, 0, &Default::default())
        .unwrap()
        .svg;
    let opaque_xml = roxmltree::Document::parse(&opaque_svg).unwrap();
    let opaque_image = opaque_xml
        .descendants()
        .find(|node| node.has_tag_name("image"))
        .unwrap();
    assert!(
        !opaque_image
            .ancestors()
            .any(|node| node.attribute("opacity").is_some())
    );
}

#[cfg(feature = "render")]
#[test]
fn fully_transparent_type3_image_skips_its_image_element() {
    const SOURCE: &[u8] = include_bytes!("assets/crop-quadrants.png");
    let parsed = sdocx::parse_bytes_detailed(&archive(
        &one_page(vec![object(3, &image_with_transparency(7, 1.0), &[])]),
        Some(&[(7, "quadrants.png")]),
        &[("quadrants.png", SOURCE)],
    ))
    .unwrap();
    let svg = sdocx::render_page_svg(&parsed.document, 0, &Default::default())
        .unwrap()
        .svg;
    assert!(!svg.contains("<image "));

    // The native getter returns zero for Formula/type 11. A public model's
    // type-3-only setting must not alter formula image pixels.
    let mut formula = parsed.document.clone();
    let image = placed(formula.pages[0].elements().next().unwrap()).clone();
    formula.pages[0].objects = vec![PageElement::FormulaImage(image).into()];
    let svg = sdocx::render_page_svg(&formula, 0, &Default::default())
        .unwrap()
        .svg;
    let xml = roxmltree::Document::parse(&svg).unwrap();
    let image = xml
        .descendants()
        .find(|node| node.has_tag_name("image"))
        .unwrap();
    assert!(
        !image
            .ancestors()
            .any(|node| node.attribute("opacity").is_some())
    );
}

#[cfg(feature = "pdf")]
#[test]
fn type3_transparency_remains_vector_image_alpha_in_pdf() {
    const SOURCE: &[u8] = include_bytes!("assets/crop-quadrants.png");
    let payload = image_with_transparency(7, 0.5);
    let parsed = sdocx::parse_bytes_detailed(&archive(
        &one_page(vec![object(3, &payload, &[])]),
        Some(&[(7, "quadrants.png")]),
        &[("quadrants.png", SOURCE)],
    ))
    .unwrap();
    let options = sdocx::PdfOptions::new(std::sync::Arc::new(Default::default()));
    let bytes =
        sdocx::render_document_pdf(&parsed.document, &Default::default(), &options).unwrap();
    let pdf = lopdf::Document::load_mem(&bytes).unwrap();
    let images: Vec<_> = pdf
        .objects
        .values()
        .filter_map(|object| {
            let stream = object.as_stream().ok()?;
            (stream.dict.get(b"Subtype").ok()?.as_name().ok()? == b"Image").then_some(&stream.dict)
        })
        .collect();
    assert_eq!(
        images.len(),
        1,
        "retain the source image without rasterizing the page"
    );
    assert_eq!(images[0].get(b"Width").unwrap().as_i64().unwrap(), 4);
    assert_eq!(images[0].get(b"Height").unwrap().as_i64().unwrap(), 3);
    assert!(pdf.objects.values().any(|object| {
        object.as_dict().is_ok_and(|dictionary| {
            dictionary.get(b"ca").is_ok_and(|value| {
                value
                    .as_float()
                    .is_ok_and(|alpha| (alpha - 127.0 / 255.0).abs() < 0.0001)
            })
        })
    }));
}

#[cfg(all(feature = "render", feature = "pdf"))]
#[test]
fn type3_transparency_sdk_artifact_triplet_matches_captured_draw_geometry() {
    check_image_alpha_controls(false);
}

#[cfg(all(feature = "render", feature = "pdf"))]
#[test]
#[ignore = "native Skia partial-source alpha is 63; resvg rounds to 64 (image-effects-findings.md)"]
fn native_partial_source_alpha_keeps_skia_byte_rounding() {
    check_image_alpha_controls(true);
}

#[cfg(all(feature = "render", feature = "pdf"))]
fn check_image_alpha_controls(include_native_rounding_edge: bool) {
    let artifact_dir =
        std::env::var_os("SDOCX_IMAGE_TRANSPARENCY_ARTIFACT_DIR").map(std::path::PathBuf::from);
    // The two partial-source cases correspond to the V14 native capture:
    // straight PNG RGBA [32,128,224,127] must decode to premul [16,64,112,127]
    // before any group-alpha difference is attributed. Native evidence receipt:
    // /tmp/sdocx-drawimage-transparency-v1/source-alpha-v14-offline-validation.json
    // SHA-256 eb102facd4db6f9f884cbc2cb92fc8e9c99cf49a04f8e7836394457aec1802bb.
    let cases = [
        (
            "opaque",
            [32, 128, 224, 255],
            0.0_f32,
            255_u8,
            [32, 128, 224, 255],
        ),
        (
            "half",
            [32, 128, 224, 255],
            0.5_f32,
            127_u8,
            [16, 64, 112, 127],
        ),
        (
            "transparent",
            [32, 128, 224, 255],
            1.0_f32,
            0_u8,
            [0, 0, 0, 0],
        ),
        (
            "partial-source-opaque",
            [32, 128, 224, 127],
            0.0_f32,
            255_u8,
            [16, 64, 112, 127],
        ),
        (
            "partial-source-native-groupalpha",
            [32, 128, 224, 127],
            f32::from_bits(0x3efe_feff),
            128_u8,
            [8, 32, 56, 63],
        ),
    ];
    let mut captures = Vec::with_capacity(cases.len());
    for (label, source_rgba, transparency, expected_alpha, expected_native_rgba) in cases {
        if !include_native_rounding_edge && label == "partial-source-native-groupalpha" {
            continue;
        }
        let media = uniform_rgba_png(512, 256, source_rgba);
        let payload = image_with_transparency(7, transparency);
        let mut document = sdocx::parse_bytes_detailed(&archive(
            &one_page(vec![object(3, &payload, &[])]),
            Some(&[(7, "source.png")]),
            &[("source.png", &media)],
        ))
        .unwrap()
        .document;
        let mut image = placed(document.pages[0].elements().next().unwrap()).clone();
        assert_eq!(
            image.fill_effect_transparency.to_bits(),
            transparency.to_bits()
        );
        image.bbox = sdocx::BoundingBox {
            x_min: 8.0,
            y_min: 8.0,
            x_max: 40.0,
            y_max: 24.0,
        };
        image.crop_rect = Some([73, 50, 405, 222]);
        // The native direct-draw control has orientation zero; the parser
        // fixture's unrelated stored rotation must not enter this comparison.
        image.rotation_degrees = Some(0.0);
        image.original_bbox = None;
        document.pages[0].width = 48;
        document.pages[0].height = 32;
        document.pages[0].content_bbox = sdocx::BoundingBox {
            x_min: 0.0,
            y_min: 0.0,
            x_max: 48.0,
            y_max: 32.0,
        };
        // Keep the normal SDK page background in the public artifacts. A
        // separately derived layer SVG below removes only that exact paper
        // rectangle for comparison with the native transparent target.
        document.pages[0].background_color = Some(sdocx::Color {
            r: 252,
            g: 252,
            b: 252,
        });
        let actual_transparency_bits = image.fill_effect_transparency.to_bits();
        document.pages[0].objects = vec![PageElement::PlacedImage(image).into()];

        let rendered = sdocx::render_page_svg(&document, 0, &Default::default()).unwrap();
        let xml = roxmltree::Document::parse(&rendered.svg).unwrap();
        let svg_image = xml.descendants().find(|node| node.has_tag_name("image"));
        let root = xml.root_element();
        let full_page_rects: Vec<_> = root
            .children()
            .filter(|node| node.is_element() && node.has_tag_name("rect"))
            .filter(|node| {
                ["x", "y", "width", "height"]
                    .into_iter()
                    .zip([0.0_f64, 0.0, 48.0, 32.0])
                    .all(|(attribute, expected)| {
                        node.attribute(attribute)
                            .and_then(|value| value.parse::<f64>().ok())
                            == Some(expected)
                    })
            })
            .collect();
        let layer_svg = if full_page_rects.len() == 1 {
            let range = full_page_rects[0].range();
            let mut layer = String::with_capacity(rendered.svg.len());
            layer.push_str(&rendered.svg[..range.start]);
            layer.push_str(&rendered.svg[range.end..]);
            Some(layer)
        } else {
            None
        };
        let public_tree = resvg::usvg::Tree::from_str(&rendered.svg, &Default::default()).unwrap();
        let mut public_pixels =
            resvg::tiny_skia::Pixmap::new(rendered.width, rendered.height).unwrap();
        resvg::render(
            &public_tree,
            resvg::tiny_skia::Transform::identity(),
            &mut public_pixels.as_mut(),
        );
        let raster_svg = layer_svg.as_deref().unwrap_or(&rendered.svg);
        let tree = resvg::usvg::Tree::from_str(raster_svg, &Default::default()).unwrap();
        let mut pixels = resvg::tiny_skia::Pixmap::new(rendered.width, rendered.height).unwrap();
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::identity(),
            &mut pixels.as_mut(),
        );
        if let Some(directory) = &artifact_dir {
            std::fs::create_dir_all(directory).unwrap();
            std::fs::write(directory.join(format!("{label}.svg")), &rendered.svg).unwrap();
            if let Some(layer_svg) = &layer_svg {
                std::fs::write(directory.join(format!("{label}.layer.svg")), layer_svg).unwrap();
            }
            let options = sdocx::PdfOptions::new(std::sync::Arc::new(Default::default()));
            let pdf = sdocx::render_document_pdf(&document, &Default::default(), &options).unwrap();
            std::fs::write(directory.join(format!("{label}.pdf")), pdf).unwrap();
            std::fs::write(
                directory.join(format!("{label}.layer.premul.rgba")),
                pixels.data(),
            )
            .unwrap();
            let mut png = Vec::new();
            {
                let mut encoder = png::Encoder::new(&mut png, rendered.width, rendered.height);
                encoder.set_color(png::ColorType::Rgba);
                encoder.set_depth(png::BitDepth::Eight);
                let mut writer = encoder.write_header().unwrap();
                writer.write_image_data(public_pixels.data()).unwrap();
            }
            std::fs::write(directory.join(format!("{label}.rgba.png")), png).unwrap();
        }

        let expected_inside = expected_native_rgba;
        let (image_present, image_view_box, opacity) = if let Some(image) = svg_image {
            let image_bounds = ["x", "y", "width", "height"].map(|attribute| {
                image
                    .attribute(attribute)
                    .and_then(|value| value.parse::<f64>().ok())
            });
            let view_box = image_bounds
                .into_iter()
                .collect::<Option<Vec<_>>>()
                .and_then(|values| <[f64; 4]>::try_from(values).ok());
            let opacity = image
                .ancestors()
                .find_map(|node| node.attribute("opacity"))
                .map(str::to_owned);
            (true, view_box, opacity)
        } else {
            (false, None, None)
        };
        // Accumulate all case readbacks before any native-pixel or geometry
        // assertion, so a control mismatch cannot hide the edge-case result.
        captures.push((
            label,
            transparency,
            expected_alpha,
            expected_inside,
            actual_transparency_bits,
            (rendered.width, rendered.height),
            pixels.data().to_vec(),
            image_present,
            image_view_box,
            opacity,
            full_page_rects.len(),
        ));
    }

    for (
        label,
        transparency,
        expected_alpha,
        expected_inside,
        actual_transparency_bits,
        dimensions,
        pixels,
        has_image,
        view_box,
        opacity,
        paper_rect_count,
    ) in captures
    {
        assert_eq!(
            actual_transparency_bits,
            transparency.to_bits(),
            "{label}: decoded transparency"
        );
        assert_eq!(dimensions, (48, 32), "{label}: target dimensions");
        assert_eq!(
            paper_rect_count, 1,
            "{label}: remove only exact full-page paper rectangle"
        );
        if expected_alpha == 0 {
            assert!(!has_image, "{label}: fully transparent image is omitted");
        } else {
            assert!(has_image, "{label}: visible image element is retained");
            assert_eq!(
                view_box,
                Some([8.0, 8.0, 32.0, 16.0]),
                "{label}: cropped image viewport"
            );
            if expected_alpha == 255 {
                assert!(
                    opacity.is_none(),
                    "{label}: opaque image has no group opacity"
                );
            } else {
                let value: f64 = opacity
                    .as_deref()
                    .expect("translucent image has group opacity")
                    .parse()
                    .unwrap();
                assert!((value - f64::from(expected_alpha) / 255.0).abs() < 0.000_001);
            }
        }
        assert_eq!(
            pixels.len(),
            48 * 32 * 4,
            "{label}: complete page layer readback"
        );
        for y in 0..32 {
            for x in 0..48 {
                let offset = ((y * 48 + x) * 4) as usize;
                let actual: [u8; 4] = pixels[offset..offset + 4].try_into().unwrap();
                let expected = if (8..40).contains(&x) && (8..24).contains(&y) && has_image {
                    expected_inside
                } else {
                    [0, 0, 0, 0]
                };
                assert_eq!(actual, expected, "{label}: premultiplied RGBA at ({x},{y})");
            }
        }
    }
}
