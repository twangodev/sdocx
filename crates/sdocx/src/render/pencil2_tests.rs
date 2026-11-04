//! Paired scene transport controls for the fixed-resolution Pencil2 hypothesis.
use super::*;
use crate::ink::pencil2::{device_raster, mixed_rgb_tests};

fn png(image: &[[u8; 4]]) -> Vec<u8> {
    let rgba: Vec<_> = image
        .iter()
        .flat_map(|&[r, g, b, a]| {
            let straight = |v: u8| {
                if a == 0 {
                    0
                } else {
                    ((u32::from(v) * 255 + u32::from(a) / 2) / u32::from(a)).min(255) as u8
                }
            };
            [straight(r), straight(g), straight(b), a]
        })
        .collect();
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, 192, 192);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&rgba)
            .unwrap();
    }
    bytes
}

fn scene(image: &[[u8; 4]], background: Option<Color>) -> RenderedScene {
    let mut scene = Scene::new(
        Svg::new()
            .width(192)
            .height(192)
            .view_box(ViewBox::unrounded(0., 0., 192., 192.)),
    );
    scene.retain_text();
    if let Some(color) = background {
        scene.push(
            Rectangle::new()
                .width(192)
                .height(192)
                .fill(Paint::from_hex(&color_hex(&color))),
        );
    }
    scene.push(
        Image::embedded(&png(image), "image/png")
            .width(192)
            .height(192)
            .stretched()
            .pixelated(),
    );
    let text = scene.take_native_text();
    let ink = scene.take_native_ink();
    let text_error = scene.take_native_text_error();
    RenderedScene {
        page: RenderedPage::new(0, 192, 192, scene.finish()),
        text,
        ink,
        text_error,
    }
}

fn raster(svg: &str) -> Vec<[u8; 4]> {
    let tree = resvg::usvg::Tree::from_str(svg, &Default::default()).unwrap();
    let mut pixmap = resvg::tiny_skia::Pixmap::new(192, 192).unwrap();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    pixmap.data().as_chunks::<4>().0.to_vec()
}

fn assert_pdf_transport(scene: &RenderedScene) {
    let mut options = crate::PdfOptions::default();
    options.dpi = 72.;
    let direct = crate::pdf::render_native_scene_pdf_for_test(scene, &options).unwrap();
    let from_svg =
        crate::render_svg_pages_pdf(std::slice::from_ref(&scene.page), &options).unwrap();
    for bytes in [&direct, &from_svg] {
        let pdf = lopdf::Document::load_mem(bytes).unwrap();
        assert!(
            pdf.objects
                .values()
                .any(|object| object.as_stream().is_ok_and(|stream| {
                    stream
                        .dict
                        .get(b"Subtype")
                        .is_ok_and(|value| value.as_name().is_ok_and(|name| name == b"Image"))
                }))
        );
    }
}

#[test]
fn pencil2_rgba_raster_svg_and_direct_pdf_transport() {
    let receipt = mixed_rgb_tests::receipt();
    let mut mismatches = Vec::new();
    for name in ["colored", "mixed", "mixed_reversed"] {
        let image = device_raster::render(
            &mixed_rgb_tests::control_records(name),
            device_raster::Context::default(),
        )
        .unwrap();
        for (suffix, background) in [
            ("", None),
            (
                "_white",
                Some(Color {
                    r: 255,
                    g: 255,
                    b: 255,
                }),
            ),
            (
                "_paper",
                Some(Color {
                    r: 64,
                    g: 32,
                    b: 96,
                }),
            ),
        ] {
            let scene = scene(&image, background);
            let output = raster(&scene.page.svg);
            let target = receipt.results[&format!("{name}{suffix}")].image();
            let errors = mixed_rgb_tests::error(&output, &target);
            // Native-frame discrepancies introduced by transparent image transport.
            let expected = match (name, suffix) {
                (_, "") => (0, 0, 0),
                ("colored", "_white") => (53, 68, 3),
                ("colored", "_paper") => (72, 79, 2),
                ("mixed", "_white") => (75, 102, 4),
                ("mixed", "_paper") => (244, 268, 3),
                ("mixed_reversed", "_white") => (67, 83, 2),
                ("mixed_reversed", "_paper") => (238, 255, 3),
                _ => unreachable!("unknown transport control"),
            };
            if errors != expected {
                mismatches.push((format!("{name}{suffix}"), errors, expected));
            }
            if background.is_none() {
                assert!(mixed_rgb_tests::error(&output, &image).2 <= 1);
            }
            assert_pdf_transport(&scene);
        }
    }
    assert!(mismatches.is_empty(), "{mismatches:?}");
}

#[test]
fn pencil2_backdrop_seeded_svg_and_direct_pdf_transport() {
    let receipt = mixed_rgb_tests::receipt();
    for (name, cases) in [
        ("colored", [("_white", (0, 0, 0)), ("_paper", (0, 0, 0))]),
        ("mixed", [("_white", (0, 0, 0)), ("_paper", (0, 0, 0))]),
        (
            "mixed_reversed",
            [("_white", (0, 0, 0)), ("_paper", (0, 0, 0))],
        ),
    ] {
        let records = mixed_rgb_tests::control_records(name);
        for (suffix, expected) in cases {
            let color = if suffix == "_white" {
                [255; 4]
            } else {
                [64, 32, 96, 255]
            };
            let backdrop = vec![color; 192 * 192];
            let image = device_raster::render_with_backdrop(
                &records,
                device_raster::Context::default(),
                Some(&backdrop),
            )
            .unwrap();
            let scene = scene(&image, None);
            let output = raster(&scene.page.svg);
            assert_eq!(output, image);
            assert_eq!(
                mixed_rgb_tests::error(
                    &output,
                    &receipt.results[&format!("{name}{suffix}")].image()
                ),
                expected,
                "{name}{suffix}"
            );
            assert_pdf_transport(&scene);
        }
    }
}

#[test]
fn pencil2_colored_generic_svg_and_direct_pdf_baseline() {
    let stroke = Stroke {
        points: vec![
            crate::Point { x: 0., y: 192. },
            crate::Point { x: 10., y: 192. },
            crate::Point { x: 20., y: 192. },
            crate::Point { x: 30., y: 182. },
        ],
        pressures: vec![0.5; 4],
        tilts: vec![0.; 4],
        orientations: vec![0.; 4],
        timestamps: vec![10, 20, 30, 40],
        color: Some(Color {
            r: 0x33,
            g: 0x66,
            b: 0x99,
        }),
        pen_width: 2.,
        rendering: Some(crate::StrokeRendering {
            metadata: None,
            pen_name: Some("com.samsung.android.sdk.pen.pen.preload.Pencil2".into()),
            advanced_settings: Some("14;".into()),
            tool_type_raw: 2,
            properties: crate::StrokeProperties {
                compressed: true,
                stylus_channels: true,
                replay_only: false,
                eraser: false,
                fixed_width: false,
                millisecond_timestamps: false,
                top_layer_pen: false,
                alpha_lock: false,
                binary_added: false,
                generated: false,
                fixed_opacity: false,
                rainbow_effect: false,
                straighten: false,
                reveal_mode: false,
            },
            style: crate::StrokeStyle {
                pen_size: Some(2.),
                color_argb: Some(0x80336699),
                size_level: Some(0),
                stroke_type_raw: Some(0),
                color_type_raw: Some(0),
                ..Default::default()
            },
        }),
        bbox: BoundingBox::default(),
    };
    let document = Document {
        pages: vec![Page {
            uuid: "pencil2".into(),
            width: 192,
            height: 192,
            objects: vec![stroke.into()],
            background_color: Some(Color {
                r: 255,
                g: 255,
                b: 255,
            }),
            content_bbox: BoundingBox::default(),
            template: None,
            background: Default::default(),
        }],
        metadata: Default::default(),
    };
    let options = RenderOptions::default();
    let rendered = render_document_svg(&document, &options).remove(0);
    let native = mixed_rgb_tests::receipt().results["colored_white"].image();
    let errors = mixed_rgb_tests::error(&raster(&rendered.svg), &native);
    let baseline = 4_135;
    assert!(
        errors.1 <= baseline,
        "generic colored white SVG/native error {errors:?} exceeds retained baseline {baseline}"
    );
    let mut pdf_options = crate::PdfOptions::default();
    pdf_options.dpi = 72.;
    crate::render_document_pdf(&document, &options, &pdf_options).unwrap();
}
