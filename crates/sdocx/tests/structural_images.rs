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
                placed(&parsed.document.pages[0].elements[0]).media_index,
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
        assert!(pages[0].svg.contains("rotate(30.00"));
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
    assert_eq!(image.original_bbox.unwrap().x_min, -20.0);
    #[cfg(feature = "render")]
    {
        let svg =
            &sdocx::render_document_svg(&parsed.document, &sdocx::RenderOptions::default())[0].svg;
        assert!(svg.contains("overflow=\"hidden\""));
        assert!(svg.contains("viewBox=\"-10.0000 20.0000 100.0000 80.0000\""));
        assert!(svg.contains("rotate(30.0000"));
        assert!(svg.contains("<image x=\"-20.00\" y=\"0.00\" width=\"200.00\" height=\"160.00\""));
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
    assert_eq!(layout.pages.len(), 2);
    for page in &layout.pages {
        let PageElement::TextBox(text) = &page.page.elements[0] else {
            panic!("text flow")
        };
        assert_eq!(text.object_spans.len(), 1);
        assert_eq!(text.object_spans[0].text_index_utf16, 0);
        assert_eq!(text.margins.unwrap()[1], 10.0);
    }
    #[cfg(feature = "render")]
    for page in sdocx::render_document_svg(&document, &sdocx::RenderOptions::default()) {
        assert_eq!(page.svg.matches("<image ").count(), 1);
    }
}

#[test]
fn hidden_images_keep_media_references_without_resolving_or_drawing_them() {
    let mut payload = image(42);
    payload[11] &= !(1 << 3);
    let raw = page(&[vec![object(3, &payload, &[])]], 0, &[]);
    let parsed = sdocx::parse_bytes_detailed(&support::archive(&raw)).unwrap();
    assert!(parsed.document.pages[0].elements.is_empty());
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
        let elements = &doc.pages[0].elements;
        assert_eq!(elements.len(), 3);
        assert_eq!(asset_bytes(doc, &elements[0]), b"blue");
        assert_eq!(asset_bytes(doc, &elements[1]), b"red");
        assert_eq!(asset_bytes(doc, &elements[2]), b"blue");
        assert_eq!(placed(&elements[0]).media_id, Some(42));
        assert_eq!(
            placed(&elements[0]).media_index,
            placed(&elements[2]).media_index
        );
        assert_eq!(placed(&elements[0]).rotation_degrees, Some(30.0));
        assert_eq!(placed(&elements[0]).bbox.x_min, -10.0);
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
            assert!(svg.contains("rotate(30.00 40.00 60.00)"));
            assert!(svg.contains("x=\"-10.00\" y=\"20.00\" width=\"100.00\" height=\"80.00\""));
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
        assert_eq!(parsed.document.pages[0].elements.len(), 1);
        assert_eq!(
            placed(&parsed.document.pages[0].elements[0]).media_index,
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
        asset_bytes(&parsed.document, &parsed.document.pages[0].elements[0]),
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
            placed(&parsed.document.pages[0].elements[0])
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
    assert_eq!(parsed.document.pages[0].elements.len(), 1);
    assert_eq!(
        asset_bytes(&parsed.document, &parsed.document.pages[0].elements[0]),
        b"blue"
    );
    assert_eq!(
        asset_bytes(&parsed.document, &parsed.document.pages[1].elements[0]),
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
    let img = placed(&parsed.document.pages[0].elements[0]);
    assert_eq!(img.media_id, Some(7));
    assert_eq!(img.crop_rect, Some([1, 2, 30, 40]));
    assert_eq!(img.border_media_id, Some(8));
    assert_eq!(img.original_media_id, Some(9));
    assert_eq!(
        asset_bytes(&parsed.document, &parsed.document.pages[0].elements[0]),
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
        let img = placed(&parsed.document.pages[0].elements[0]);
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
    let image = placed(&parsed.document.pages[0].elements[0]);
    assert_eq!(image.media_id, Some(0));
    assert_eq!(image.bbox.x_min, 0.0);
    assert_eq!(image.bbox.x_max, 0.5);
    assert_eq!(image.bbox.y_max, 0.25);
    assert_eq!(
        asset_bytes(&parsed.document, &parsed.document.pages[0].elements[0]),
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
    assert!(parsed.document.pages[0].elements.is_empty());
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
    let image = placed(&parsed.document.pages[0].elements[0]);
    assert_eq!(image.media_id, None);
    assert_eq!(image.original_media_id, Some(7));
    assert_eq!(image.media_index, None);
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
    let image = placed(&document.pages[0].elements[0]);
    document.pages[0].elements = vec![PageElement::Image {
        bbox: image.bbox,
        media_index: image.media_index.unwrap(),
    }];
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
        placed(&parsed.document.pages[0].elements[0]).rotation_degrees,
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
