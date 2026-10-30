    fn variant_serialized_brushpen_native() {
        let mut stroke = crate::decode::decode_stroke(
            include_bytes!("../../../../../conformance/brushpen-serialized/v10-payload.bin"),
            &crate::ParseLimits::default(),
        )
        .unwrap();
        stroke.pen_width = 16.0;
        stroke.points[2] = stroke.points[0];
        let rendering = stroke.rendering.as_mut().unwrap();
        rendering.pen_name = Some("com.samsung.android.sdk.pen.pen.preload.BrushPen".into());
        rendering.advanced_settings = Some("10;".into());
        rendering.style.pen_size = Some(16.0);
        let records = crate::sample_brushpen_glv10_host(&stroke, 1920, 1080).unwrap();
        let bytes: Vec<_> = records
            .iter()
            .flat_map(|r| r.iter().flat_map(|v| v.to_le_bytes()))
            .collect();
        assert_eq!(
            bytes,
            std::fs::read(std::env::var_os("BRUSHPEN_VARIANT_STREAM").unwrap()).unwrap().as_slice()
        );
        let (atlas, grain) = source_textures();
        let mut canvas = crate::InkRasterCanvas::new(
            crate::InkRasterContext {
                width: 321,
                height: 228,
                pixels_per_unit: 1,
                origin: crate::InkRasterOrigin::TopLeft,
            },
            [0; 4],
        )
        .unwrap();
        canvas
            .paint_brushpen_glv10_host(
                &stroke,
                crate::BrushPenPaintContext {
                    screen_width: 1920,
                    screen_height: 1080,
                    projection: [
                        0.006_230_529_4,
                        0.,
                        0.,
                        0.,
                        0.,
                        0.008_771_93,
                        0.,
                        0.,
                        0.,
                        0.,
                        -0.000_030_518_51,
                        0.,
                        -1.,
                        -1.,
                        0.,
                        1.,
                    ],
                    rgb: [0., 0., 0.],
                    grain_period: [321., 228.],
                    grain_offset: [132.823_96, 211.381_33],
                    num_tiles: 3.,
                    atlas: crate::BrushPenTexture {
                        width: 144,
                        height: 144,
                        rgba: &atlas,
                    },
                    grain: crate::BrushPenTexture {
                        width: 321,
                        height: 228,
                        rgba: &grain,
                    },
                },
            )
            .unwrap();
        let actual: Vec<u8> = canvas
            .pixels()
            .as_chunks::<321>()
            .0
            .iter()
            .rev()
            .flatten()
            .flatten()
            .copied()
            .collect();
        let page = canvas.rendered_page(0).unwrap();
        let mut options = crate::PdfOptions::default();
        options.dpi = 72.0;
        let dir = std::path::PathBuf::from(std::env::var_os("BRUSHPEN_VARIANT_EXPORT").unwrap());
        std::fs::write(dir.join("canvas.png"), canvas.to_png().unwrap()).unwrap();
        std::fs::write(dir.join("canvas.svg"), &page.svg).unwrap();
        std::fs::write(dir.join("direct.pdf"), canvas.to_pdf(&options).unwrap()).unwrap();
        std::fs::write(dir.join("svg.pdf"), crate::render_svg_pages_pdf(&[page], &options).unwrap()).unwrap();
    }
