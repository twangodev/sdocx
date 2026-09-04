mod support;

use sdocx::{DiagnosticCode, Error, PageElement, PlacedImage};
use std::io::{Cursor, Write};
use support::{object, page};

fn frame(kind: i16, fields: u32, fixed: &[u8], flexible: &[u8]) -> Vec<u8> {
    let offset = 17 + fixed.len();
    let mut bytes = ((offset + flexible.len()) as u32).to_le_bytes().to_vec();
    bytes.extend_from_slice(&kind.to_le_bytes());
    bytes.extend_from_slice(&(offset as u32).to_le_bytes());
    bytes.extend_from_slice(&[1, 0, 4]);
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
    let mut fixed = 1_u32.to_le_bytes().to_vec();
    for value in [0.0_f64, 0.0, 100.0, 80.0] {
        fixed.extend_from_slice(&value.to_le_bytes());
    }
    fixed.extend_from_slice(&0.0_f32.to_le_bytes()); // radius
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
            page(
                &[
                    vec![object(250, &image(7), &[])],
                    vec![object(4, b"group", &[object(3, &image(42), &[])])],
                ],
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
