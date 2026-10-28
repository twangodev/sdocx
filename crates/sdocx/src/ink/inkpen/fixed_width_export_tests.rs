use super::*;

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
        "../../../../../conformance/inkpen-v1-fixed-width-extended-native.json"
    ))
    .unwrap();
    fixture["cases"]
        .as_array_mut()
        .unwrap()
        .extend(extended["cases"].as_array().unwrap().iter().cloned());
    for (i, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        let mut stroke = native_caller_stroke(&case["input"]);
        let r = stroke.rendering.as_mut().unwrap();
        r.properties.fixed_width = true;
        r.style.fixed_width = Some(case["width"].as_f64().unwrap() as f32);
        r.style.color_argb = Some(0xff000000);
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
