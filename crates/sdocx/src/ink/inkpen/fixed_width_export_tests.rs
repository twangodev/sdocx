use super::*;

#[test]
fn v1_fixed_width_same_input_export_comparison() {
    let Some(directory) = std::env::var_os("SDOCX_V1_SAME_INPUT_EXPORT_DIR") else {
        return;
    };
    let fixture_data = std::env::var_os("SDOCX_V1_SAME_INPUT_FIXTURE")
        .map(std::fs::read_to_string)
        .transpose()
        .unwrap()
        .unwrap_or_else(|| {
            include_str!("../../../tests/fixtures/inkpen-v1-high-scale-color-native.json")
                .to_owned()
        });
    let fixture: Value = serde_json::from_str(&fixture_data).unwrap();
    for (i, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        let mut stroke = native_caller_stroke(&case["input"]);
        stroke.color = None;
        let rendering = stroke.rendering.as_mut().unwrap();
        rendering.properties.fixed_width = true;
        rendering.style.fixed_width = Some(case["width"].as_f64().unwrap() as f32);
        rendering.style.color_argb = Some(case["color"].as_u64().unwrap() as u32);
        let doc = crate::Document {
            pages: vec![crate::Page {
                uuid: "probe".into(),
                width: 128,
                height: 64,
                content_bbox: BoundingBox::default(),
                background_color: None,
                template: None,
                background: Default::default(),
                objects: vec![stroke.into()],
            }],
            metadata: Default::default(),
        };
        for (name, scale) in [
            ("candidate", Some(case["scale"].as_u64().unwrap() as u32)),
            ("generic", None),
        ] {
            let options = crate::RenderOptions {
                inkpen_v1_output_scale: scale,
                ..Default::default()
            };
            let svg = crate::render_document_svg(&doc, &options).remove(0).svg;
            let pdf =
                crate::render_document_pdf(&doc, &options, &crate::PdfOptions::default()).unwrap();
            for (extension, bytes) in [("svg", svg.as_bytes()), ("pdf", pdf.as_slice())] {
                use std::io::Write;
                std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(std::path::Path::new(&directory).join(format!("{i}-{name}.{extension}")))
                    .unwrap()
                    .write_all(bytes)
                    .unwrap();
            }
        }
    }
}

#[test]
fn v1_fixed_width_export_comparison() {
    let Some(directory) = std::env::var_os("SDOCX_V1_FIXED_EXPORT_DIR") else {
        return;
    };
    assert!(
        std::path::Path::new(&directory)
            .read_dir()
            .unwrap()
            .next()
            .is_none(),
        "export directory must be empty"
    );
    let mut fixture: Value = serde_json::from_str(include_str!(
        "../../../../../conformance/inkpen-v1-fixed-width-native.json"
    ))
    .unwrap();
    let extended: Value = serde_json::from_str(include_str!(
        "../../../../../conformance/inkpen-v1-fixed-width-native.json"
    ))
    .unwrap();
    let extended = &extended["retained_controls"]["extended"];
    fixture["cases"]
        .as_array_mut()
        .unwrap()
        .extend(extended["cases"].as_array().unwrap().iter().cloned());
    let high_scale: Value = serde_json::from_str(include_str!(
        "../../../../../tools/inkpen-v1/high-scale-native.json"
    ))
    .unwrap();
    fixture["cases"]
        .as_array_mut()
        .unwrap()
        .extend(high_scale["cases"].as_array().unwrap().iter().cloned());
    let high_scale_color: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/inkpen-v1-high-scale-color-native.json"
    ))
    .unwrap();
    fixture["cases"].as_array_mut().unwrap().extend(
        high_scale_color["cases"]
            .as_array()
            .unwrap()
            .iter()
            .cloned(),
    );
    for (i, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        let mut stroke = native_caller_stroke(&case["input"]);
        let r = stroke.rendering.as_mut().unwrap();
        r.properties.fixed_width = true;
        r.style.fixed_width = Some(case["width"].as_f64().unwrap() as f32);
        r.style.color_argb = Some(case["color"].as_u64().unwrap_or(0xff000000) as u32);
        let ink = prepare_v1(
            &stroke,
            RuntimeContext {
                curve_enabled: case["curve"].as_bool().unwrap(),
                screen_width: 1920,
                screen_height: 1080,
            },
        )
        .unwrap();
        let stream = case["streams"]["draw_object_stroke"]["values_f32_bits"]
            .as_array()
            .unwrap();
        let mut matched = stream.len() == ink.points.len() * 3;
        if matched {
            for (j, p) in ink.points.iter().enumerate() {
                matched &= (p.x as f32).to_bits()
                    == native_f32(stream[3 * j].as_str().unwrap()).to_bits()
                    && (p.y as f32).to_bits()
                        == native_f32(stream[3 * j + 1].as_str().unwrap()).to_bits()
                    && ink.radii[j].to_bits()
                        == native_f32(stream[3 * j + 2].as_str().unwrap()).to_bits();
            }
        }
        assert!(matched, "case {i}: native stream differs");
        for name in ["candidate", "generic"] {
            let mut selected = stroke.clone();
            if name == "generic" {
                selected.rendering.as_mut().unwrap().advanced_settings = Some("unsupported".into());
            }
            let doc = crate::Document {
                pages: vec![crate::Page {
                    uuid: "probe".into(),
                    width: 128,
                    height: 64,
                    content_bbox: BoundingBox::default(),
                    background_color: None,
                    template: None,
                    background: Default::default(),
                    objects: vec![selected.into()],
                }],
                metadata: Default::default(),
            };
            let options = crate::RenderOptions {
                inkpen_v1_output_scale: Some(case["scale"].as_u64().unwrap_or(1) as u32),
                ..Default::default()
            };
            let svg = crate::render_document_svg(&doc, &options).remove(0).svg;
            let pdf =
                crate::render_document_pdf(&doc, &options, &crate::PdfOptions::default()).unwrap();
            use std::io::Write;
            for (extension, bytes) in [("svg", svg.as_bytes()), ("pdf", pdf.as_slice())] {
                std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(std::path::Path::new(&directory).join(format!("{i}-{name}.{extension}")))
                    .unwrap()
                    .write_all(bytes)
                    .unwrap();
            }
        }
    }
}

#[test]
fn v1_fixed_width_fallback_saved_paint_public_exports() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/inkpen-v1-high-scale-color-native.json"
    ))
    .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let mut stroke = native_caller_stroke(&case["input"]);
        let color = case["color"].as_u64().unwrap() as u32;
        let rendering = stroke.rendering.as_mut().unwrap();
        rendering.properties.fixed_width = true;
        rendering.style.fixed_width = Some(case["width"].as_f64().unwrap() as f32);
        rendering.style.color_argb = Some(color);
        stroke.color = None;
        for mode in [crate::RenderColorMode::Light, crate::RenderColorMode::Dark] {
            let page = crate::Page {
                uuid: "fallback".into(),
                width: 128,
                height: 64,
                content_bbox: BoundingBox::default(),
                background_color: None,
                template: None,
                background: Default::default(),
                objects: vec![stroke.clone().into()],
            };
            let doc = crate::Document {
                pages: vec![page],
                metadata: Default::default(),
            };
            let options = crate::RenderOptions {
                color_mode: mode,
                ..Default::default()
            };
            let svg = crate::render_document_svg(&doc, &options).remove(0).svg;
            let expected_color = match (color, mode) {
                (0xff33_6699, _) => "#336699",
                (0x80d0_4721, _) => "#d04721",
                (0x20a0_30e0, _) => "#a030e0",
                (0x0019_aa44, crate::RenderColorMode::Light) => "#19aa44",
                (0x0019_aa44, crate::RenderColorMode::Dark) => "#55e680",
                _ => panic!("unexpected native color or mode"),
            };
            let expected_opacity = f64::from((color >> 24) as u8) / 255.0;
            assert!(svg.contains(expected_color), "{svg}");
            if expected_opacity < 1.0 {
                assert!(
                    svg.contains(&format!("opacity=\"{}\"", expected_opacity)),
                    "{svg}"
                );
            }
            let pdf =
                crate::render_document_pdf(&doc, &options, &crate::PdfOptions::default()).unwrap();
            let pdf = lopdf::Document::load_mem(&pdf).unwrap();
            let rgb = [
                u8::from_str_radix(&expected_color[1..3], 16).unwrap(),
                u8::from_str_radix(&expected_color[3..5], 16).unwrap(),
                u8::from_str_radix(&expected_color[5..7], 16).unwrap(),
            ];
            let has_rgb = pdf
                .objects
                .values()
                .filter_map(|o| o.as_stream().ok())
                .any(|stream| {
                    let Ok(bytes) = stream.decompressed_content() else {
                        return false;
                    };
                    let Ok(content) = lopdf::content::Content::decode(&bytes) else {
                        return false;
                    };
                    content.operations.iter().any(|op| {
                        matches!(op.operator.as_str(), "rg" | "RG")
                            && op.operands.len() == 3
                            && op.operands.iter().zip(rgb).all(|(v, channel)| {
                                let value = match v {
                                    lopdf::Object::Real(value) => f64::from(*value),
                                    lopdf::Object::Integer(value) => *value as f64,
                                    _ => return false,
                                };
                                (value - f64::from(channel) / 255.0).abs() < 0.0001
                            })
                    })
                });
            assert!(has_rgb, "missing PDF RGB {expected_color}");
            let has_alpha = pdf
                .objects
                .values()
                .filter_map(|o| o.as_dict().ok())
                .any(|d| {
                    [b"ca".as_slice(), b"CA".as_slice()].iter().any(|key| {
                        d.get(key)
                            .ok()
                            .and_then(|v| v.as_float().ok())
                            .is_some_and(|v| (f64::from(v) - expected_opacity).abs() < 0.00001)
                    })
                });
            assert!(
                expected_opacity == 1.0 || has_alpha,
                "missing PDF alpha {}",
                expected_opacity
            );
        }
    }
}

#[test]
fn v1_fixed_width_native_pixel_receipts_and_fallback_admission() {
    use base64::Engine;
    use std::io::Read;

    let fixture: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/inkpen-v1-high-scale-color-native.json"
    ))
    .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let color = case["color"].as_u64().unwrap() as u32;
        let mut stroke = native_caller_stroke(&case["input"]);
        let rendering = stroke.rendering.as_mut().unwrap();
        rendering.properties.fixed_width = true;
        rendering.style.color_argb = Some(color);
        rendering.style.fixed_width = Some(case["width"].as_f64().unwrap() as f32);
        assert_eq!(crate::ink::saved_inkpen_argb(&stroke), Some(color));

        let packed = base64::engine::general_purpose::STANDARD
            .decode(case["rgba_zlib_base64"].as_str().unwrap())
            .unwrap();
        let mut pixels = Vec::new();
        flate2::read::ZlibDecoder::new(&packed[..])
            .read_to_end(&mut pixels)
            .unwrap();
        let alpha = (color >> 24) as u8;
        if alpha == 0 {
            assert!(pixels.iter().all(|v| *v == 0));
        } else {
            let rgb = [(color >> 16) as u8, (color >> 8) as u8, color as u8];
            let expected = rgb.map(|v| ((u32::from(v) * u32::from(alpha) + 127) / 255) as u8);
            assert!(
                pixels.as_chunks::<4>().0.iter().any(|p| p[3] == alpha
                    && p[..3].iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 1))
            );
        }
        for unsupported in 0..4 {
            let mut excluded = stroke.clone();
            excluded.color = None;
            let r = excluded.rendering.as_mut().unwrap();
            match unsupported {
                0 => r.tool_type_raw = 3,
                1 => r.tool_type_raw = 5,
                2 => r.advanced_settings = Some("4;".into()),
                _ => r.advanced_settings = Some("unsupported".into()),
            }
            assert_eq!(crate::ink::saved_inkpen_argb(&excluded), None);
            let prepared = crate::prepare_stroke(&excluded, false);
            assert_eq!(prepared.opacity, 1.0);
            assert_eq!(prepared.color, "#1a1a1a");
        }
    }
}
