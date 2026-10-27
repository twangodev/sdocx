#![cfg(feature = "pdf")]

fn points(hex: &str) -> Vec<sdocx::Point> {
    let bytes: Vec<u8> = hex
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect();
    let values: Vec<f32> = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|v| f32::from_le_bytes(*v))
        .collect();
    values
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| sdocx::Point {
            x: f64::from(p[0]),
            y: f64::from(p[1]),
        })
        .collect()
}

fn native_error(svg: &str, sparse: &[u8]) -> u64 {
    let tree = resvg::usvg::Tree::from_str(svg, &Default::default()).unwrap();
    let mut pixmap = resvg::tiny_skia::Pixmap::new(1920, 1080).unwrap();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    let mut native = vec![[0_u8; 4]; 1920 * 1080];
    for row in sparse.as_chunks::<8>().0 {
        let i = u32::from_le_bytes(row[..4].try_into().unwrap()) as usize;
        native[i].copy_from_slice(&row[4..]);
    }
    let mut sum = 0;
    for (a, b) in pixmap.data().as_chunks::<4>().0.iter().zip(native) {
        let a =
            std::array::from_fn::<_, 3, _>(|i| (u16::from(a[i]) + u16::from(255 - a[3])).min(255));
        let b =
            std::array::from_fn::<_, 3, _>(|i| (u16::from(b[i]) + u16::from(255 - b[3])).min(255));
        if a.iter().chain(&b).any(|&x| x < 250) {
            for (x, y) in a.iter().zip(b) {
                sum += u64::from(x.abs_diff(y));
            }
        }
    }
    sum
}

#[test]
fn profile_improves_varied_native_controls_and_falls_back_for_unsupported_metadata() {
    let geometry: serde_json::Value = serde_json::from_str(include_str!(
        "../../../conformance/legacy-marker-source-geometry-controls.json"
    ))
    .unwrap();
    let sector: serde_json::Value = serde_json::from_str(include_str!(
        "../../../conformance/legacy-marker-sector-center-control.json"
    ))
    .unwrap();
    let alpha40: serde_json::Value = serde_json::from_str(include_str!(
        "../../../conformance/legacy-marker-alpha40-control.json"
    ))
    .unwrap();
    let trace: serde_json::Value = serde_json::from_str(include_str!(
        "../../../conformance/legacy-marker-subdivision-controls.json"
    ))
    .unwrap();
    let long: serde_json::Value = serde_json::from_str(include_str!(
        "../../../conformance/legacy-marker-long-native-controls.json"
    ))
    .unwrap();
    let finished: serde_json::Value = serde_json::from_str(include_str!(
        "../../../conformance/legacy-marker-finished-prefix-native.json"
    ))
    .unwrap();
    let repeated = &finished["cases"][3];
    let sparse =
        include_bytes!("../../../conformance/legacy-marker-finished-prefix-sparse-rgba8.bin");
    let offset = repeated["sparse_offset"].as_u64().unwrap() as usize;
    let length = repeated["sparse_bytes"].as_u64().unwrap() as usize;
    let doc = sdocx::parse_bytes(include_bytes!(
        "assets/legacy-marker/synthetic-serialized.sdocx"
    ))
    .unwrap();
    let original = doc.pages[0].strokes().next().unwrap();
    let cases = [
        (
            "repeat-final",
            repeated["input"]["source_arrays_hex"]["points"]
                .as_str()
                .unwrap(),
            &sparse[offset..offset + length],
        ),
        (
            "straight",
            geometry["cases"]["straight"]["input"]["source_arrays_hex"]["points"]
                .as_str()
                .unwrap(),
            include_bytes!("../../../conformance/legacy-marker-source-straight-sparse-rgba8.bin")
                .as_slice(),
        ),
        (
            "alpha40",
            alpha40["input"]["source_arrays_hex"]["points"]
                .as_str()
                .unwrap(),
            include_bytes!("../../../conformance/legacy-marker-alpha40-sparse-rgba8.bin")
                .as_slice(),
        ),
        (
            "bend",
            geometry["cases"]["bend"]["input"]["source_arrays_hex"]["points"]
                .as_str()
                .unwrap(),
            include_bytes!("../../../conformance/legacy-marker-source-bend-sparse-rgba8.bin")
                .as_slice(),
        ),
        (
            "turn",
            sector["input"]["source_arrays_hex"]["points"]
                .as_str()
                .unwrap(),
            include_bytes!("../../../conformance/legacy-marker-turn-sparse-rgba8.bin").as_slice(),
        ),
        (
            "mirrored",
            sector["mirrored"]["input"]["source_arrays_hex"]["points"]
                .as_str()
                .unwrap(),
            include_bytes!("../../../conformance/legacy-marker-mirrored-sparse-rgba8.bin")
                .as_slice(),
        ),
        (
            "reverse",
            trace["circle_controls"][0]["source_arrays_hex"]["points"]
                .as_str()
                .unwrap(),
            include_bytes!("../../../conformance/legacy-marker-reverse-sparse-rgba8.bin")
                .as_slice(),
        ),
        (
            "long-skipped",
            long["cases"][0]["input"]["source_arrays_hex"]["points"]
                .as_str()
                .unwrap(),
            include_bytes!("../../../conformance/legacy-marker-source-bend-sparse-rgba8.bin")
                .as_slice(),
        ),
        (
            "long-turns",
            long["cases"][1]["input"]["source_arrays_hex"]["points"]
                .as_str()
                .unwrap(),
            include_bytes!("../../../conformance/legacy-marker-long-turns-sparse-rgba8.bin")
                .as_slice(),
        ),
    ];
    let fallback = sdocx::RenderOptions::default();
    let mut profile = fallback.clone();
    profile.legacy_marker_centerline_profile = Some(sdocx::LegacyMarkerCenterlineProfile {
        screen_width: 1920,
        screen_height: 1080,
    });
    for (name, encoded, native) in cases {
        let mut stroke = original.clone();
        stroke.points = points(encoded);
        stroke.pen_width = 12.0;
        stroke.pressures = vec![0.5; stroke.points.len()];
        stroke.color = Some(sdocx::Color {
            r: 255,
            g: 203,
            b: 48,
        });
        let rendering = stroke.rendering.as_mut().unwrap();
        rendering.style.pen_size = Some(12.0);
        rendering.style.color_argb = Some(if name == "alpha40" {
            0x40ffcb30
        } else {
            0x80ffcb30
        });
        let mut page = doc.pages[0].clone();
        page.objects = vec![sdocx::PageObject::from(stroke.clone())];
        page.width = 1920;
        page.height = 1080;
        page.template = None;
        page.background = Default::default();
        page.background_color = None;
        let isolated = sdocx::Document {
            pages: vec![page],
            metadata: sdocx::DocumentMetadata::default(),
        };
        let old = sdocx::render_document_svg(&isolated, &fallback).remove(0);
        let new = sdocx::render_document_svg(&isolated, &profile).remove(0);
        let old_error = native_error(&old.svg, native);
        let new_error = native_error(&new.svg, native);
        assert!(
            new_error <= old_error,
            "{name}: fallback {old_error}, profile {new_error}"
        );
        let old_pdf =
            sdocx::render_document_pdf(&isolated, &fallback, &sdocx::PdfOptions::default())
                .unwrap();
        let new_pdf =
            sdocx::render_document_pdf(&isolated, &profile, &sdocx::PdfOptions::default()).unwrap();
        if let Some(directory) = std::env::var_os("SDOCX_MARKER_PUBLIC_EXPORT_DIR") {
            let directory = std::path::PathBuf::from(directory);
            std::fs::write(directory.join(format!("{name}-fallback.svg")), &old.svg).unwrap();
            std::fs::write(directory.join(format!("{name}-profile.svg")), &new.svg).unwrap();
            std::fs::write(directory.join(format!("{name}-fallback.pdf")), &old_pdf).unwrap();
            std::fs::write(directory.join(format!("{name}-profile.pdf")), &new_pdf).unwrap();
        }
        assert_ne!(old_pdf, new_pdf, "{name}");
        assert_eq!(
            lopdf::Document::load_mem(&new_pdf)
                .unwrap()
                .get_pages()
                .len(),
            1
        );
        if name == "bend" {
            let mut invalid = isolated.clone();
            let stroke = match &mut invalid.pages[0].objects[0].content {
                sdocx::PageObjectContent::Stroke(s) => s,
                _ => unreachable!(),
            };
            stroke.rendering.as_mut().unwrap().advanced_settings = Some("unknown".into());
            assert_eq!(
                sdocx::render_document_svg(&invalid, &fallback)[0].svg,
                sdocx::render_document_svg(&invalid, &profile)[0].svg
            );
            for (label, samples) in [
                (
                    "sample budget",
                    vec![sdocx::Point { x: 10.0, y: 20.0 }; 16_385],
                ),
                (
                    "nonfinite coordinate",
                    vec![
                        sdocx::Point {
                            x: f64::NAN,
                            y: 20.0,
                        },
                        sdocx::Point { x: 20.0, y: 20.0 },
                        sdocx::Point { x: 30.0, y: 20.0 },
                    ],
                ),
                (
                    "no accepted move",
                    vec![
                        sdocx::Point { x: 10.0, y: 20.0 },
                        sdocx::Point { x: 10.5, y: 20.0 },
                        sdocx::Point { x: 11.0, y: 20.0 },
                    ],
                ),
                (
                    "two-point output scale gap",
                    vec![
                        sdocx::Point { x: 10.0, y: 20.0 },
                        sdocx::Point { x: 30.0, y: 20.0 },
                    ],
                ),
                (
                    "degenerate terminal",
                    vec![
                        sdocx::Point { x: 10.0, y: 20.0 },
                        sdocx::Point { x: 20.0, y: 20.0 },
                        sdocx::Point { x: 20.0, y: 20.0 },
                    ],
                ),
            ] {
                let mut unsupported = isolated.clone();
                let stroke = match &mut unsupported.pages[0].objects[0].content {
                    sdocx::PageObjectContent::Stroke(s) => s,
                    _ => unreachable!(),
                };
                stroke.pressures = vec![0.5; samples.len()];
                stroke.points = samples;
                assert_eq!(
                    sdocx::render_document_svg(&unsupported, &fallback)[0].svg,
                    sdocx::render_document_svg(&unsupported, &profile)[0].svg,
                    "{label} must use fallback"
                );
            }
            for (width, height) in [(0, 1080), (1920, 0), (u32::MAX, 1080), (1920, u32::MAX)] {
                let mut invalid_screen = profile.clone();
                invalid_screen.legacy_marker_centerline_profile =
                    Some(sdocx::LegacyMarkerCenterlineProfile {
                        screen_width: width,
                        screen_height: height,
                    });
                assert_eq!(
                    sdocx::render_document_svg(&isolated, &fallback)[0].svg,
                    sdocx::render_document_svg(&isolated, &invalid_screen)[0].svg,
                    "invalid screen {width}x{height} must use fallback"
                );
            }
            let mut rotated = isolated.clone();
            let stroke = match &mut rotated.pages[0].objects[0].content {
                sdocx::PageObjectContent::Stroke(s) => s,
                _ => unreachable!(),
            };
            let metadata = stroke
                .rendering
                .as_mut()
                .unwrap()
                .metadata
                .as_mut()
                .expect("captured metadata");
            metadata.rotation_degrees = Some(45.0);
            assert_eq!(
                sdocx::render_document_svg(&rotated, &fallback)[0].svg,
                sdocx::render_document_svg(&rotated, &profile)[0].svg
            );
        }
    }
}

#[test]
fn profile_improves_varied_screen_native_control() {
    let receipt: serde_json::Value = serde_json::from_str(include_str!(
        "../../../conformance/legacy-marker-varied-screen-control.json"
    ))
    .unwrap();
    let doc = sdocx::parse_bytes(include_bytes!(
        "assets/legacy-marker/synthetic-serialized.sdocx"
    ))
    .unwrap();
    let mut stroke = doc.pages[0].strokes().next().unwrap().clone();
    stroke.points = receipt["points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| sdocx::Point {
            x: p[0].as_f64().unwrap(),
            y: p[1].as_f64().unwrap(),
        })
        .collect();
    stroke.pen_width = 8.0;
    stroke.pressures = vec![0.5; stroke.points.len()];
    stroke.color = Some(sdocx::Color {
        r: 64,
        g: 128,
        b: 240,
    });
    let rendering = stroke.rendering.as_mut().unwrap();
    rendering.style.pen_size = Some(8.0);
    rendering.style.color_argb = Some(0xc84080f0);
    let mut page = doc.pages[0].clone();
    page.objects = vec![sdocx::PageObject::from(stroke)];
    page.width = 1920;
    page.height = 1080;
    page.template = None;
    page.background = Default::default();
    page.background_color = None;
    let isolated = sdocx::Document {
        pages: vec![page],
        metadata: sdocx::DocumentMetadata::default(),
    };
    let fallback = sdocx::RenderOptions::default();
    let mut profile = fallback.clone();
    profile.legacy_marker_centerline_profile = Some(sdocx::LegacyMarkerCenterlineProfile {
        screen_width: 720,
        screen_height: 480,
    });
    let native =
        include_bytes!("../../../conformance/legacy-marker-varied-screen-sparse-rgba8.bin");
    let old = sdocx::render_document_svg(&isolated, &fallback).remove(0);
    let new = sdocx::render_document_svg(&isolated, &profile).remove(0);
    assert!(native_error(&new.svg, native) < native_error(&old.svg, native));
    let old_pdf =
        sdocx::render_document_pdf(&isolated, &fallback, &sdocx::PdfOptions::default()).unwrap();
    let new_pdf =
        sdocx::render_document_pdf(&isolated, &profile, &sdocx::PdfOptions::default()).unwrap();
    assert_ne!(old_pdf, new_pdf);
    assert_eq!(
        lopdf::Document::load_mem(&new_pdf)
            .unwrap()
            .get_pages()
            .len(),
        1
    );
}
