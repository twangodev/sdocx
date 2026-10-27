use super::*;

#[test]
fn legacy_marker_argb_alpha_survives_svg_and_pdf() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/legacy-marker-source-geometry-controls.json"
    ))
    .unwrap();
    fn values(hex: &str) -> Vec<f32> {
        let b: Vec<u8> = hex
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|x| u8::from_str_radix(std::str::from_utf8(x).unwrap(), 16).unwrap())
            .collect();
        b.as_chunks::<4>()
            .0
            .iter()
            .map(|x| f32::from_le_bytes(*x))
            .collect()
    }
    for (case, bytes) in [
        (
            "straight",
            include_bytes!(
                "../../../../conformance/legacy-marker-source-straight-sparse-rgba8.bin"
            )
            .as_slice(),
        ),
        (
            "bend",
            include_bytes!("../../../../conformance/legacy-marker-source-bend-sparse-rgba8.bin")
                .as_slice(),
        ),
        (
            "alpha40",
            include_bytes!("../../../../conformance/legacy-marker-alpha40-sparse-rgba8.bin")
                .as_slice(),
        ),
        (
            "alpha01",
            include_bytes!("../../../../conformance/legacy-marker-alpha01-sparse-rgba8.bin")
                .as_slice(),
        ),
        (
            "alphac8",
            include_bytes!("../../../../conformance/legacy-marker-alphac8-sparse-rgba8.bin")
                .as_slice(),
        ),
        (
            "size2",
            include_bytes!("../../../../conformance/legacy-marker-size2-sparse-rgba8.bin")
                .as_slice(),
        ),
        (
            "size3",
            include_bytes!("../../../../conformance/legacy-marker-size3-sparse-rgba8.bin")
                .as_slice(),
        ),
        (
            "size6",
            include_bytes!("../../../../conformance/legacy-marker-size6-sparse-rgba8.bin")
                .as_slice(),
        ),
        (
            "size24",
            include_bytes!("../../../../conformance/legacy-marker-size24-sparse-rgba8.bin")
                .as_slice(),
        ),
        (
            "size48",
            include_bytes!("../../../../conformance/legacy-marker-size48-sparse-rgba8.bin")
                .as_slice(),
        ),
    ] {
        let inp = &fixture["cases"][if case.starts_with("alpha") || case.starts_with("size") {
            "straight"
        } else {
            case
        }]["input"];
        let raw = values(inp["source_arrays_hex"]["points"].as_str().unwrap());
        let stroke = crate::Stroke {
            rendering: Some(crate::StrokeRendering {
                metadata: None,
                pen_name: Some("com.samsung.android.sdk.pen.pen.preload.Marker".into()),
                advanced_settings: None,
                tool_type_raw: 2,
                properties: crate::StrokeProperties {
                    compressed: true,
                    replay_only: false,
                    stylus_channels: false,
                    eraser: false,
                    fixed_width: false,
                    millisecond_timestamps: false,
                    top_layer_pen: true,
                    alpha_lock: false,
                    binary_added: true,
                    generated: false,
                    fixed_opacity: false,
                    rainbow_effect: false,
                    straighten: false,
                    reveal_mode: false,
                },
                style: crate::StrokeStyle {
                    color_argb: Some(match case {
                        "alpha01" => 0x01ffcb30,
                        "alpha40" => 0x40ffcb30,
                        "alphac8" => 0xc8ffcb30,
                        _ => 0x80ffcb30,
                    }),
                    pen_size: Some(match case {
                        "size2" => 2.0,
                        "size3" => 3.0,
                        "size6" => 6.0,
                        "size24" => 24.0,
                        "size48" => 48.0,
                        _ => 12.0,
                    }),
                    stroke_type_raw: Some(0),
                    ..Default::default()
                },
            }),
            bbox: crate::BoundingBox::default(),
            points: raw
                .as_chunks::<2>()
                .0
                .iter()
                .map(|p| crate::Point {
                    x: p[0] as f64,
                    y: p[1] as f64,
                })
                .collect(),
            pressures: values(inp["source_arrays_hex"]["pressure"].as_str().unwrap())
                .iter()
                .map(|v| *v as f64)
                .collect(),
            timestamps: vec![0, 8, 16, 24],
            tilts: values(inp["source_arrays_hex"]["tilt"].as_str().unwrap())
                .iter()
                .map(|v| *v as f64)
                .collect(),
            orientations: values(inp["source_arrays_hex"]["orientation"].as_str().unwrap())
                .iter()
                .map(|v| *v as f64)
                .collect(),
            color: Some(crate::Color {
                r: 255,
                g: 203,
                b: 48,
            }),
            pen_width: match case {
                "size2" => 2.0,
                "size3" => 3.0,
                "size6" => 6.0,
                "size24" => 24.0,
                "size48" => 48.0,
                _ => 12.0,
            },
        };
        let mut scene = Scene::new(Svg::new().width(1920).height(1080));
        render_stroke(
            &mut scene,
            &stroke,
            RenderTheme::for_canvas(false),
            None,
            &mut crate::ink::calligraphy::CalligraphySession::new(),
            true,
            [1920, 1080],
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            &std::cell::Cell::new(usize::MAX),
            None,
        );
        let svg = scene.finish();
        let alpha = match case {
            "alpha01" => 1,
            "alpha40" => 64,
            "alphac8" => 200,
            _ => 128,
        };
        let expected_alpha = f64::from(alpha) / 255.0;
        assert!(
            svg.contains(&format!("opacity=\"{expected_alpha}\"")),
            "{svg}"
        );
        let page = crate::RenderedPage::new(0, 1920, 1080, svg.clone());
        let pdf = crate::render_svg_pages_pdf(&[page], &crate::PdfOptions::default()).unwrap();
        let doc = lopdf::Document::load_mem(&pdf).unwrap();
        let has_alpha = doc
            .objects
            .values()
            .filter_map(|o| o.as_dict().ok())
            .any(|d| {
                [b"ca".as_slice(), b"CA".as_slice()].iter().any(|key| {
                    d.get(key)
                        .ok()
                        .and_then(|v| v.as_float().ok())
                        .is_some_and(|v| (f64::from(v) - expected_alpha).abs() < 0.00001)
                })
            });
        assert!(has_alpha, "{case}: PDF has no expected alpha state");
        let tree = resvg::usvg::Tree::from_str(&svg, &Default::default()).unwrap();
        let mut pix = resvg::tiny_skia::Pixmap::new(1920, 1080).unwrap();
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::identity(),
            &mut pix.as_mut(),
        );
        let mut expected = vec![0u8; 1920 * 1080 * 4];
        for r in bytes.as_chunks::<8>().0 {
            let i = u32::from_le_bytes(r[..4].try_into().unwrap()) as usize;
            expected[i * 4..i * 4 + 4].copy_from_slice(&r[4..]);
        }
        let mut n = 0;
        let mut sum = 0u64;
        let mut max = 0;
        for (a, b) in pix
            .data()
            .as_chunks::<4>()
            .0
            .iter()
            .zip(expected.as_chunks::<4>().0)
        {
            if a != b {
                n += 1;
            }
            for i in 0..4 {
                let d = a[i].abs_diff(b[i]);
                sum += u64::from(d);
                max = max.max(d);
            }
        }
        let (max_different_pixels, max_absolute_sum, max_channel) = match case {
            "bend" => (248, 48983, 179),
            "alpha40" => (208, 24584, 115),
            "alpha01" => (180, 9828, 52),
            "alphac8" => (208, 56418, 251),
            "size2" => (10, 1850, 104),
            "size3" => (16, 2969, 128),
            "size6" => (53, 10367, 128),
            "size24" => (758, 144762, 181),
            "size48" => (1403, 298130, 180),
            _ => (208, 39534, 179),
        };
        assert!(
            n <= max_different_pixels && sum <= max_absolute_sum && max <= max_channel,
            "{case}: {n} pixels, sum {sum}, max {max}"
        );
    }
}
