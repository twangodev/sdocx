//! Synthetic serialized name derivative: parser channels versus native Model getters.
use sha2::{Digest, Sha256};

#[test]
fn synthetic_marker_parser_matches_native_model_channels() {
    let receipt: serde_json::Value = serde_json::from_str(include_str!(
        "../../../conformance/legacy-marker-synthetic-serialized-drawing.json"
    ))
    .unwrap();
    let bytes = include_bytes!("assets/legacy-marker/synthetic-serialized.sdocx");

    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    use std::io::Read;
    let mut note = Vec::new();
    archive
        .by_name("note.note")
        .unwrap()
        .read_to_end(&mut note)
        .unwrap();
    let table_size = u32::from_le_bytes(note[765..769].try_into().unwrap()) as usize;
    assert_eq!(
        format!("{:x}", Sha256::digest(&note[769..769 + table_size])),
        receipt["table_sha256"].as_str().unwrap()
    );
    let mut page = Vec::new();
    archive
        .by_name("a2ab6718-b54d-11f1-8a68-83a46e5f73c2.page")
        .unwrap()
        .read_to_end(&mut page)
        .unwrap();
    let frame_size = u32::from_le_bytes(page[481..485].try_into().unwrap()) as usize;
    assert_eq!(
        format!("{:x}", Sha256::digest(&page[481..481 + frame_size])),
        receipt["frame_sha256"].as_str().unwrap()
    );
    let doc = sdocx::parse_bytes(bytes).unwrap();
    assert_eq!(doc.pages.len(), 1);
    assert_eq!(doc.pages[0].objects.len(), 1);
    assert_eq!(doc.pages[0].strokes().count(), 1);
    let stroke = doc
        .pages
        .iter()
        .flat_map(|page| page.strokes())
        .find(|stroke| {
            stroke
                .rendering
                .as_ref()
                .and_then(|r| r.pen_name.as_deref())
                == Some("com.samsung.android.sdk.pen.pen.preload.Marker")
        })
        .unwrap();
    let rendering = stroke.rendering.as_ref().unwrap();
    assert_eq!(stroke.points.len(), 204);
    assert_eq!(rendering.advanced_settings, None);
    assert_eq!(rendering.style.color_argb, None);
    assert_eq!(rendering.tool_type_raw, 2);
    assert_eq!(stroke.pen_width.to_bits(), 0x4111_de6a);
    let points: Vec<u8> = stroke
        .points
        .iter()
        .flat_map(|p| [(p.x as f32).to_le_bytes(), (p.y as f32).to_le_bytes()])
        .flatten()
        .collect();
    let floats = |values: &[f64]| {
        values
            .iter()
            .flat_map(|value| (*value as f32).to_le_bytes())
            .collect::<Vec<u8>>()
    };
    let timestamps: Vec<u8> = stroke
        .timestamps
        .iter()
        .flat_map(|value| i32::try_from(*value).unwrap().to_le_bytes())
        .collect();
    for (name, data) in [
        ("points", points),
        ("pressure", floats(&stroke.pressures)),
        ("timestamps", timestamps),
        ("tilt", floats(&stroke.tilts)),
        ("orientation", floats(&stroke.orientations)),
    ] {
        assert_eq!(
            format!("{:x}", Sha256::digest(data)),
            receipt["native_and_parser_arrays"][name]["native_and_rust_sha256"]
                .as_str()
                .unwrap(),
            "{name}"
        );
    }
}

#[cfg(feature = "pdf")]
#[test]
fn serialized_marker_without_argb_uses_native_opaque_default_width() {
    let doc = sdocx::parse_bytes(include_bytes!(
        "assets/legacy-marker/synthetic-serialized.sdocx"
    ))
    .unwrap();
    let stroke = doc.pages[0].strokes().next().unwrap();
    assert_eq!(stroke.rendering.as_ref().unwrap().style.color_argb, None);
    let prepared = sdocx::prepare_stroke(stroke, false);
    assert_eq!(prepared.opacity, 1.0);
    assert_eq!(prepared.width, 9.0);
    assert!(prepared.segment_widths.is_none());
    let mut page = doc.pages[0].clone();
    page.width = 1920;
    page.height = 1080;
    page.template = None;
    page.background = Default::default();
    page.background_color = None;
    let isolated = sdocx::Document {
        pages: vec![page],
        metadata: sdocx::DocumentMetadata::default(),
    };
    let options = sdocx::RenderOptions::default();
    let svg = sdocx::render_document_svg(&isolated, &options);
    assert!(
        svg[0].svg.contains("stroke-width=\"9.00\""),
        "{}",
        svg[0].svg
    );
    assert!(svg[0].svg.contains("stroke=\"#000000\""));
    let tree = resvg::usvg::Tree::from_str(&svg[0].svg, &Default::default()).unwrap();
    let mut pixmap = resvg::tiny_skia::Pixmap::new(1920, 1080).unwrap();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    let sparse =
        include_bytes!("../../../conformance/legacy-marker-serialized-host-sparse-rgba8.bin");
    let mut native = vec![[0_u8; 4]; 1920 * 1080];
    for row in sparse.as_chunks::<8>().0 {
        let index = u32::from_le_bytes(row[..4].try_into().unwrap()) as usize;
        native[index].copy_from_slice(&row[4..]);
    }
    let mut rgb_error = 0_u64;
    for (actual, expected) in pixmap.data().as_chunks::<4>().0.iter().zip(native) {
        let actual_rgb = std::array::from_fn::<_, 3, _>(|i| {
            (u16::from(actual[i]) + u16::from(255 - actual[3])).min(255)
        });
        let expected_rgb = std::array::from_fn::<_, 3, _>(|i| {
            (u16::from(expected[i]) + u16::from(255 - expected[3])).min(255)
        });
        if actual_rgb.iter().chain(&expected_rgb).any(|&v| v < 250) {
            for (a, b) in actual_rgb.iter().zip(expected_rgb) {
                rgb_error += u64::from(a.abs_diff(b));
            }
        }
    }
    assert!(
        rgb_error < 100_000,
        "resvg/native RGB absolute error: {rgb_error}"
    );
    let pdf =
        sdocx::render_document_pdf(&isolated, &options, &sdocx::PdfOptions::default()).unwrap();
    assert_eq!(
        lopdf::Document::load_mem(&pdf).unwrap().get_pages().len(),
        1
    );
}

#[cfg(feature = "pdf")]
#[test]
fn opt_in_marker_centerline_improves_serialized_host_control_and_preserves_replay() {
    let doc = sdocx::parse_bytes(include_bytes!(
        "assets/legacy-marker/synthetic-serialized.sdocx"
    ))
    .unwrap();
    let mut page = doc.pages[0].clone();
    page.width = 1920;
    page.height = 1080;
    page.template = None;
    page.background = Default::default();
    page.background_color = None;
    let isolated = sdocx::Document {
        pages: vec![page],
        metadata: sdocx::DocumentMetadata::default(),
    };
    let baseline = sdocx::RenderOptions::default();
    let mut profiled = baseline.clone();
    profiled.legacy_marker_centerline_profile = Some(sdocx::LegacyMarkerCenterlineProfile {
        screen_width: 1920,
        screen_height: 1080,
    });
    let old = sdocx::render_document_svg(&isolated, &baseline).remove(0);
    let new = sdocx::render_document_svg(&isolated, &profiled).remove(0);
    assert_ne!(old.svg, new.svg);
    let sparse =
        include_bytes!("../../../conformance/legacy-marker-serialized-host-sparse-rgba8.bin");
    let mut native = vec![[0_u8; 4]; 1920 * 1080];
    for row in sparse.as_chunks::<8>().0 {
        let i = u32::from_le_bytes(row[..4].try_into().unwrap()) as usize;
        native[i].copy_from_slice(&row[4..]);
    }
    let error = |svg: &str| {
        let tree = resvg::usvg::Tree::from_str(svg, &Default::default()).unwrap();
        let mut pix = resvg::tiny_skia::Pixmap::new(1920, 1080).unwrap();
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::identity(),
            &mut pix.as_mut(),
        );
        let mut sum = 0u64;
        for (actual, expected) in pix.data().as_chunks::<4>().0.iter().zip(&native) {
            let a = std::array::from_fn::<_, 3, _>(|i| {
                (u16::from(actual[i]) + u16::from(255 - actual[3])).min(255)
            });
            let b = std::array::from_fn::<_, 3, _>(|i| {
                (u16::from(expected[i]) + u16::from(255 - expected[3])).min(255)
            });
            if a.iter().chain(&b).any(|&x| x < 250) {
                for (a, b) in a.iter().zip(b) {
                    sum += u64::from(a.abs_diff(b));
                }
            }
        }
        sum
    };
    let old_error = error(&old.svg);
    let new_error = error(&new.svg);
    assert!(
        new_error < old_error,
        "profile error {new_error}, fallback error {old_error}"
    );
    let pdf_options = sdocx::PdfOptions::default();
    let old_pdf = sdocx::render_document_pdf(&isolated, &baseline, &pdf_options).unwrap();
    let new_pdf = sdocx::render_document_pdf(&isolated, &profiled, &pdf_options).unwrap();
    assert_ne!(old_pdf, new_pdf);
    assert_eq!(
        lopdf::Document::load_mem(&new_pdf)
            .unwrap()
            .get_pages()
            .len(),
        1
    );
    let layout = sdocx::layout_document(&isolated);
    let old_replay =
        sdocx::render_layout_page_replay_svg(&isolated, &layout, 0, &baseline).unwrap();
    let new_replay =
        sdocx::render_layout_page_replay_svg(&isolated, &layout, 0, &profiled).unwrap();
    assert_eq!(old_replay.svg, new_replay.svg);
    profiled.legacy_marker_centerline_profile = Some(sdocx::LegacyMarkerCenterlineProfile {
        screen_width: 0,
        screen_height: 1080,
    });
    assert_eq!(
        sdocx::render_document_svg(&isolated, &profiled)[0].svg,
        old.svg
    );
}
