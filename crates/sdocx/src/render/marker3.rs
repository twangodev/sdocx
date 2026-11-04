//! Test-only vector projection of the captured Marker3 mask coverage field.
//!
//! This uses 32 opacity levels and 64-sided contours to approximate the analytic
//! L1 inward ramp at a canonical 1 SVG-unit/device-pixel scale. It is not the
//! captured shader's exact `fwidth` evaluation and is not production-selected.
//!
//! This module is deliberately not selected by the production stroke renderer.
use super::vector::{Data, Paint, Path, Scene, Styled, Svg, ViewBox, color_hex, decimal};
#[cfg(feature = "pdf")]
use crate::RenderedPage;
use crate::{Color, Point};

const COVERAGE_LEVELS: usize = 32;
const CONTOUR_SIDES: usize = 64;

fn contour_vertex(point: Point, radius: f64, threshold: f64, vertex: usize) -> (f64, f64) {
    let angle = std::f64::consts::TAU * vertex as f64 / CONTOUR_SIDES as f64;
    let (sin, cos) = angle.sin_cos();
    let contour_radius = (radius - threshold * (cos.abs() + sin.abs())).max(0.0);
    (
        point.x + contour_radius * cos,
        point.y + contour_radius * sin,
    )
}

/// Append one compound fill per nested alpha level, unioning stamps within a
/// level so overlapping centers approximate the native component-wise MAX.
pub(super) fn append_coverage(
    scene: &mut Scene,
    points: &[Point],
    radii: &[f64],
    color: &Color,
) -> bool {
    if points.is_empty()
        || points.len() != radii.len()
        || points.len() > 4_096
        || points.iter().any(|point| {
            !point.x.is_finite()
                || !point.y.is_finite()
                || point.x.abs() > 1e7
                || point.y.abs() > 1e7
        })
        || radii
            .iter()
            .any(|radius| !radius.is_finite() || !(0.0..=400.0).contains(radius))
    {
        return false;
    }

    let color = Paint::from_hex(&color_hex(color));
    for layer in 0..COVERAGE_LEVELS {
        let threshold = layer as f64 / COVERAGE_LEVELS as f64;
        let opacity = 1.0 / (COVERAGE_LEVELS - layer) as f64;
        let mut data = Data::new();
        for (point, radius) in points.iter().zip(radii) {
            let mut first = true;
            for vertex in 0..CONTOUR_SIDES {
                let position = contour_vertex(*point, *radius, threshold, vertex);
                data = if first {
                    first = false;
                    data.move_to(position)
                } else {
                    data.line_to(position)
                };
            }
            data = data.close();
        }
        scene.push(
            Path::new()
                .data(data)
                .fill(color)
                .fill_opacity(decimal(opacity, 6)),
        );
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BoundingBox, Stroke, StrokeProperties, StrokeRendering, StrokeStyle};
    use serde::Deserialize;
    use std::path::PathBuf;

    #[derive(Deserialize)]
    struct Fixture {
        cases: Vec<Case>,
    }

    #[derive(Deserialize)]
    struct Case {
        id: String,
        input: Input,
        native: Native,
    }

    #[derive(Deserialize)]
    pub(super) struct Input {
        pub(super) samples: Vec<[f64; 2]>,
        pub(super) width: f32,
        pub(super) pressures: Option<Vec<f64>>,
        pub(super) tool_type_raw: u16,
        pub(super) fixed_width: bool,
    }

    #[derive(Deserialize)]
    struct Native {
        stamps: Vec<[f32; 3]>,
    }

    pub(super) fn marker3_stroke(input: &Input) -> Stroke {
        Stroke {
            rendering: Some(StrokeRendering {
                metadata: None,
                pen_name: Some("com.samsung.android.sdk.pen.pen.preload.Marker3".into()),
                advanced_settings: Some("5;".into()),
                tool_type_raw: input.tool_type_raw,
                properties: StrokeProperties {
                    compressed: false,
                    replay_only: false,
                    stylus_channels: false,
                    eraser: false,
                    fixed_width: input.fixed_width,
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
                style: StrokeStyle {
                    color_argb: Some(0xff33_6699),
                    pen_size: Some(input.width),
                    ..StrokeStyle::default()
                },
            }),
            bbox: BoundingBox::default(),
            points: input
                .samples
                .iter()
                .map(|[x, y]| Point { x: *x, y: *y })
                .collect(),
            pressures: input.pressures.clone().unwrap_or_default(),
            timestamps: Vec::new(),
            tilts: Vec::new(),
            orientations: Vec::new(),
            color: Some(Color {
                r: 0x33,
                g: 0x66,
                b: 0x99,
            }),
            pen_width: input.width,
        }
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn tool_one_public_svg_matches_native_host_better_than_fallback() {
        let input = Input {
            samples: vec![[40.0, 40.0], [50.0, 40.0], [60.0, 50.0]],
            pressures: Some(vec![1.0, 0.05, 1.0]),
            width: 8.0,
            tool_type_raw: 1,
            fixed_width: false,
        };
        let render = |setting: &str| {
            let mut stroke = marker3_stroke(&input);
            stroke.rendering.as_mut().unwrap().advanced_settings = Some(setting.into());
            let document = crate::Document {
                pages: vec![crate::Page {
                    uuid: "marker3-tool-one".into(),
                    width: 256,
                    height: 256,
                    content_bbox: BoundingBox {
                        x_min: 30.0,
                        y_min: 30.0,
                        x_max: 70.0,
                        y_max: 60.0,
                    },
                    background_color: None,
                    template: None,
                    background: Default::default(),
                    objects: vec![crate::PageObject::from(stroke)],
                }],
                metadata: Default::default(),
            };
            let svg = crate::render_document_svg(&document, &Default::default())
                .remove(0)
                .svg;
            let pdf =
                crate::render_document_pdf(&document, &Default::default(), &Default::default())
                    .unwrap();
            assert!(pdf.starts_with(b"%PDF"));
            let tree = resvg::usvg::Tree::from_str(&svg, &Default::default()).unwrap();
            let mut pixmap = resvg::tiny_skia::Pixmap::new(256, 256).unwrap();
            resvg::render(
                &tree,
                resvg::tiny_skia::Transform::identity(),
                &mut pixmap.as_mut(),
            );
            pixmap
        };
        let native = include_bytes!("../../../../conformance/marker3-tool1-host-crop.rgba");
        let error = |pixels: &resvg::tiny_skia::Pixmap| -> u64 {
            let mut sum = 0_u64;
            for y in 0..30 {
                for x in 0..40 {
                    let i = (y * 40 + x) * 4;
                    let j = ((y + 30) * 256 + x + 30) * 4;
                    let alpha = u32::from(native[i + 3]);
                    for channel in 0..3 {
                        let expected =
                            u32::from(native[i + channel]) + (252 * (255 - alpha) + 127) / 255;
                        sum += u64::from(
                            (i32::from(pixels.data()[j + channel]) - expected as i32)
                                .unsigned_abs(),
                        );
                    }
                }
            }
            sum
        };
        let candidate = error(&render("5;"));
        let fallback = error(&render("4;"));
        assert!(
            candidate < 25_000 && candidate * 2 < fallback,
            "candidate {candidate}, fallback {fallback}"
        );
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn zero_width_v5_native_frames_emit_no_public_ink() {
        use base64::Engine;
        use sha2::{Digest, Sha256};
        use std::io::Read;
        let native: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../conformance/marker3-v5-zero-width-native.json"
        ))
        .unwrap();
        for (size, expected_visible) in [("0.9999999403953552", false), ("1", true)] {
            let control = &native["separate_rt_set_pen_data_controls"][size];
            assert_eq!(control["input"]["color_state"]["argb"], "0xff336699");
            assert_eq!(
                control["input"]["color_state"]["size"].as_f64().unwrap(),
                size.parse::<f64>().unwrap()
            );
            let compressed = base64::engine::general_purpose::STANDARD
                .decode(control["frame_zlib_base64"].as_str().unwrap())
                .unwrap();
            let mut frame = Vec::new();
            flate2::read::ZlibDecoder::new(compressed.as_slice())
                .read_to_end(&mut frame)
                .unwrap();
            assert_eq!(frame.len(), 256 * 256 * 4);
            assert_eq!(
                format!("{:x}", Sha256::digest(&frame)),
                control["frame_sha256"].as_str().unwrap()
            );
            assert_eq!(frame.iter().any(|&byte| byte != 0), expected_visible);
        }
        let raster = |svg: &str| {
            let tree = resvg::usvg::Tree::from_str(svg, &Default::default()).unwrap();
            let mut pixmap = resvg::tiny_skia::Pixmap::new(128, 64).unwrap();
            resvg::render(
                &tree,
                resvg::tiny_skia::Transform::identity(),
                &mut pixmap.as_mut(),
            );
            pixmap.data().to_vec()
        };
        for (key, case) in native["cases"].as_object().unwrap() {
            let tool = case["input"]["tool_type"].as_u64().unwrap() as u16;
            let bits = case["source_size_f32_bits"].as_str().unwrap();
            let size = f32::from_le_bytes(std::array::from_fn(|i| {
                u8::from_str_radix(&bits[i * 2..i * 2 + 2], 16).unwrap()
            }));
            let get_bits = |key: &str| {
                let hex = case[key].as_str().unwrap();
                f32::from_le_bytes(std::array::from_fn(|i| {
                    u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap()
                }))
            };
            let native_size = size.max(0.4);
            assert_eq!(get_bits("pen_size_f32_bits"), native_size);
            assert_eq!(get_bits("drawable_size_f32_bits"), native_size);
            assert_eq!(case["selected_version"], "5");
            assert_eq!(
                case["input"]["pen_name"],
                "com.samsung.android.sdk.pen.pen.preload.Marker3"
            );
            assert_eq!(case["input"]["advanced_setting"], "5;");
            assert_eq!(case["input"]["stroke_type"], 0);
            assert_eq!(case["input"]["color_argb"], "0xff336699");
            assert_eq!(
                case["input"]["points_f32_bits"],
                serde_json::json!([["00000000", "00000000"], ["0000a041", "00000000"]])
            );
            assert_eq!(
                case["input"]["pressure_f32_bits"],
                serde_json::json!(["cdcc4c3d", "0000803f"])
            );
            let compressed = base64::engine::general_purpose::STANDARD
                .decode(case["frame_zlib_base64"].as_str().unwrap())
                .unwrap();
            let mut frame = Vec::new();
            flate2::read::ZlibDecoder::new(compressed.as_slice())
                .read_to_end(&mut frame)
                .unwrap();
            assert_eq!(frame.len(), 256 * 256 * 4);
            assert_eq!(
                format!("{:x}", Sha256::digest(&frame)),
                case["frame_sha256"].as_str().unwrap()
            );
            assert_eq!(
                frame
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .filter(|rgba| **rgba != [0, 0, 0, 0])
                    .count(),
                case["nonzero_pixels"].as_u64().unwrap() as usize
            );
            let input = Input {
                samples: vec![[16.0, 24.0], [36.0, 24.0]],
                width: size,
                pressures: Some(vec![0.05, 1.0]),
                tool_type_raw: tool,
                fixed_width: false,
            };
            let mut stroke = marker3_stroke(&input);
            assert_eq!(case["input"]["curve_enabled"], true);
            stroke.rendering.as_mut().unwrap().properties.compressed = true;
            let make_document = |strokes: Vec<Stroke>| crate::Document {
                pages: vec![crate::Page {
                    uuid: "marker3-zero".into(),
                    width: 128,
                    height: 64,
                    content_bbox: BoundingBox::default(),
                    background_color: None,
                    template: None,
                    background: Default::default(),
                    objects: strokes.into_iter().map(crate::PageObject::from).collect(),
                }],
                metadata: Default::default(),
            };
            let blank = make_document(vec![]);
            let source = make_document(vec![stroke.clone()]);
            let options = crate::RenderOptions::default();
            let svg = crate::render_document_svg(&source, &options).remove(0).svg;
            let blank_svg = crate::render_document_svg(&blank, &options).remove(0).svg;
            let empty = size < 1.0;
            assert_eq!(raster(&svg) == raster(&blank_svg), empty, "{key}");
            let layout = crate::layout_document(&source);
            let replay = crate::render_layout_page_replay_svg(&source, &layout, 0, &options)
                .unwrap()
                .svg;
            assert_eq!(raster(&replay) == raster(&blank_svg), empty, "replay {key}");
            if empty {
                assert!(
                    !replay.contains("data-replay-part"),
                    "empty replay prefix {key}"
                );
                let mut other = stroke.clone();
                other.points.iter_mut().for_each(|point| point.y += 16.0);
                other.rendering.as_mut().unwrap().style.pen_size = Some(8.0);
                let only = make_document(vec![other.clone()]);
                let mixed = make_document(vec![stroke.clone(), other.clone()]);
                let reversed = make_document(vec![other, stroke]);
                for document in [&only, &mixed, &reversed] {
                    let _ = crate::render_document_pdf(document, &options, &Default::default())
                        .unwrap();
                }
                assert_eq!(
                    raster(&crate::render_document_svg(&only, &options).remove(0).svg),
                    raster(&crate::render_document_svg(&mixed, &options).remove(0).svg),
                    "mixed {key}"
                );
                assert_eq!(
                    raster(&crate::render_document_svg(&only, &options).remove(0).svg),
                    raster(
                        &crate::render_document_svg(&reversed, &options)
                            .remove(0)
                            .svg
                    ),
                    "reversed mixed {key}"
                );
            }
            for document in [&source, &blank] {
                let _ =
                    crate::render_document_pdf(document, &options, &Default::default()).unwrap();
            }
        }
        assert_eq!(native["cases"].as_object().unwrap().len(), 8);
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn scale_profile_preserves_support_and_improves_native_host_controls() {
        use base64::Engine;
        use sha2::{Digest, Sha256};
        use std::io::Read;
        let native: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../conformance/marker3-v5-scale-shell-native.json"
        ))
        .unwrap();
        assert!(
            crate::RenderOptions::default()
                .marker3_output_scale
                .is_none()
        );
        #[cfg(feature = "serde")]
        {
            let mut old = serde_json::to_value(crate::RenderOptions::default()).unwrap();
            old.as_object_mut().unwrap().remove("marker3_output_scale");
            assert!(
                serde_json::from_value::<crate::RenderOptions>(old)
                    .unwrap()
                    .marker3_output_scale
                    .is_none()
            );
            for scale in [
                crate::Marker3OutputScale::Half,
                crate::Marker3OutputScale::One,
                crate::Marker3OutputScale::Two,
                crate::Marker3OutputScale::Four,
                crate::Marker3OutputScale::Eight,
            ] {
                let opts = crate::RenderOptions {
                    marker3_output_scale: Some(scale),
                    ..Default::default()
                };
                let roundtrip: crate::RenderOptions =
                    serde_json::from_value(serde_json::to_value(opts).unwrap()).unwrap();
                assert_eq!(roundtrip.marker3_output_scale, Some(scale));
            }
        }
        let zero_alpha = &native["zero_alpha_control"];
        assert_eq!(zero_alpha["input"]["color_state"]["argb"], "0x336699");
        let compressed = base64::engine::general_purpose::STANDARD
            .decode(zero_alpha["frame_zlib_base64"].as_str().unwrap())
            .unwrap();
        let mut empty_frame = Vec::new();
        flate2::read::ZlibDecoder::new(compressed.as_slice())
            .read_to_end(&mut empty_frame)
            .unwrap();
        assert_eq!(empty_frame.len(), 256 * 256 * 4);
        assert_eq!(
            format!("{:x}", Sha256::digest(&empty_frame)),
            zero_alpha["frame_sha256"].as_str().unwrap()
        );
        assert!(empty_frame.iter().all(|&byte| byte == 0));
        let transparent = Input {
            samples: vec![[24.0, 16.0], [34.0, 16.0], [44.0, 26.0]],
            width: 8.0,
            pressures: Some(vec![1.0, 0.05, 1.0]),
            tool_type_raw: 2,
            fixed_width: false,
        };
        let mut stroke = marker3_stroke(&transparent);
        stroke.rendering.as_mut().unwrap().style.color_argb = Some(0x0033_6699);
        let document = crate::Document {
            pages: vec![crate::Page {
                uuid: "marker3-transparent".into(),
                width: 128,
                height: 32,
                content_bbox: BoundingBox::default(),
                background_color: None,
                template: None,
                background: Default::default(),
                objects: vec![stroke.into()],
            }],
            metadata: Default::default(),
        };
        let mut blank = document.clone();
        blank.pages[0].objects.clear();
        let raster = |doc: &crate::Document, opts: &crate::RenderOptions| {
            let svg = crate::render_document_svg(doc, opts).remove(0).svg;
            let tree = resvg::usvg::Tree::from_str(&svg, &Default::default()).unwrap();
            let mut pix = resvg::tiny_skia::Pixmap::new(128, 32).unwrap();
            resvg::render(
                &tree,
                resvg::tiny_skia::Transform::identity(),
                &mut pix.as_mut(),
            );
            pix.data().to_vec()
        };
        let canonical = crate::RenderOptions::default();
        let opts = crate::RenderOptions {
            marker3_output_scale: Some(crate::Marker3OutputScale::One),
            ..Default::default()
        };
        assert_eq!(
            raster(&document, &opts),
            raster(&blank, &canonical),
            "zero-alpha source"
        );
        for (name, case) in native["cases"].as_object().unwrap() {
            let scale = case["scale"].as_f64().unwrap();
            let phase = case["phase"].as_f64().unwrap();
            let profile = match scale {
                0.5 => crate::Marker3OutputScale::Half,
                1.0 => crate::Marker3OutputScale::One,
                2.0 => crate::Marker3OutputScale::Two,
                4.0 => crate::Marker3OutputScale::Four,
                8.0 => crate::Marker3OutputScale::Eight,
                _ => panic!("unexpected scale"),
            };
            let argb = u32::from_str_radix(
                case["input"]["color_state"]["argb"]
                    .as_str()
                    .unwrap()
                    .trim_start_matches("0x"),
                16,
            )
            .unwrap();
            let points: Vec<[f64; 2]> = case["samples"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| {
                    [
                        p[0].as_f64().unwrap() + 24.0 + phase / scale,
                        p[1].as_f64().unwrap() + 16.0 + phase / scale,
                    ]
                })
                .collect();
            let pressure: Vec<f64> = case["pressures"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| p.as_f64().unwrap())
                .collect();
            let size = case["input"]["rt_size"].as_f64().unwrap() as f32;
            let input = Input {
                samples: points,
                width: size,
                pressures: Some(pressure),
                tool_type_raw: 2,
                fixed_width: false,
            };
            let mut stroke = marker3_stroke(&input);
            let rendering = stroke.rendering.as_mut().unwrap();
            rendering.properties.compressed = true;
            rendering.style.color_argb = Some(argb);
            stroke.color = Some(Color {
                r: (argb >> 16) as u8,
                g: (argb >> 8) as u8,
                b: argb as u8,
            });
            let document = crate::Document {
                pages: vec![crate::Page {
                    uuid: "marker3-scale".into(),
                    width: 128,
                    height: 32,
                    content_bbox: BoundingBox::default(),
                    background_color: None,
                    template: None,
                    background: Default::default(),
                    objects: vec![stroke.clone().into()],
                }],
                metadata: Default::default(),
            };
            let baseline_options = crate::RenderOptions::default();
            let baseline = crate::render_document_svg(&document, &baseline_options)
                .remove(0)
                .svg;
            let layout = crate::layout_document(&document);
            let replay =
                crate::render_layout_page_replay_svg(&document, &layout, 0, &baseline_options)
                    .unwrap()
                    .svg;
            let options = crate::RenderOptions {
                marker3_output_scale: Some(profile),
                ..Default::default()
            };
            let candidate = crate::render_document_svg(&document, &options)
                .remove(0)
                .svg;
            let mut alias = stroke;
            alias.rendering.as_mut().unwrap().pen_name =
                Some("com.samsung.android.sdk.pen.pen.preload.StraightMarker".into());
            let mut alias_document = document.clone();
            alias_document.pages[0].objects = vec![alias.into()];
            assert_eq!(
                candidate,
                crate::render_document_svg(&alias_document, &options)[0].svg,
                "alias SVG {name}"
            );
            #[cfg(feature = "pdf")]
            assert_eq!(
                crate::render_document_pdf(&document, &options, &Default::default()).unwrap(),
                crate::render_document_pdf(&alias_document, &options, &Default::default()).unwrap(),
                "alias PDF {name}"
            );
            assert_eq!(
                replay,
                crate::render_layout_page_replay_svg(&document, &layout, 0, &options)
                    .unwrap()
                    .svg,
                "replay {name}"
            );
            let tree = roxmltree::Document::parse(&baseline).unwrap();
            let path = tree
                .descendants()
                .find(|n| n.has_tag_name("path"))
                .unwrap()
                .attribute("d")
                .unwrap();
            let moves: Vec<_> = path.split('z').filter(|v| !v.is_empty()).collect();
            let native_points = case["input"]["points"].as_array().unwrap();
            assert_eq!(
                moves.len(),
                native_points.len(),
                "native stream count {name}"
            );
            let radius = f64::from(size.trunc()) / 2.0 + 0.5;
            let y_offset = if case["case"] == "saved1" { 80.0 } else { 48.0 };
            for (m, p) in moves.iter().zip(native_points) {
                let (xy, _) = m.strip_prefix('M').unwrap().split_once(' ').unwrap();
                let (x, y) = xy.split_once(',').unwrap();
                let x = x.parse::<f64>().unwrap() + radius;
                let y = y.parse::<f64>().unwrap();
                assert!(
                    (x - (p[0].as_f64().unwrap() - 8.0 + phase / scale)).abs() < 0.00011,
                    "x {name}"
                );
                assert!(
                    (y - (p[1].as_f64().unwrap() - y_offset + phase / scale)).abs() < 0.00011,
                    "y {name}"
                );
            }
            let compressed = base64::engine::general_purpose::STANDARD
                .decode(case["frame_zlib_base64"].as_str().unwrap())
                .unwrap();
            let mut frame = Vec::new();
            flate2::read::ZlibDecoder::new(compressed.as_slice())
                .read_to_end(&mut frame)
                .unwrap();
            assert_eq!(frame.len(), 256 * 256 * 4);
            assert_eq!(
                format!("{:x}", Sha256::digest(&frame)),
                case["frame_sha256"].as_str().unwrap()
            );
            let w = (128.0 * scale) as usize;
            let h = (32.0 * scale) as usize;
            let compare = |svg: &str| {
                let svg = svg.replacen(
                    "<rect fill=\"#fcfcfc\" height=\"32\" width=\"128\" x=\"0\" y=\"0\"/>",
                    "",
                    1,
                );
                let tree = resvg::usvg::Tree::from_str(&svg, &Default::default()).unwrap();
                let mut pix = resvg::tiny_skia::Pixmap::new(w as u32, h as u32).unwrap();
                resvg::render(
                    &tree,
                    resvg::tiny_skia::Transform::from_scale(scale as f32, scale as f32),
                    &mut pix.as_mut(),
                );
                let xoff = (128.0 - 24.0 * scale) as i32;
                let yoff =
                    (128.0 - if case["case"] == "saved1" { 16.0 } else { 21.0 } * scale) as i32;
                let mut error = 0_u64;
                let mut missing = 0usize;
                for y in 0..h {
                    for x in 0..w {
                        let nx = x as i32 + xoff;
                        let ny = y as i32 + yoff;
                        if !(0..256).contains(&nx) || !(0..256).contains(&ny) {
                            continue;
                        }
                        let ni = (ny as usize * 256 + nx as usize) * 4;
                        let ri = (y * w + x) * 4;
                        for c in 0..4 {
                            error += u64::from(frame[ni + c].abs_diff(pix.data()[ri + c]));
                        }
                        if frame[ni + 3] != 0 && pix.data()[ri + 3] == 0 {
                            missing += 1
                        }
                    }
                }
                (error, missing)
            };
            let (before, _) = compare(&baseline);
            let (after, missing) = compare(&candidate);
            assert!(after < before, "{name}: {after} >= {before}");
            assert_eq!(missing, 0, "native support {name}");
            if name == "saved3-8-80336699-1" {
                let bytes =
                    crate::render_document_pdf(&document, &options, &Default::default()).unwrap();
                let pdf = lopdf::Document::load_mem(&bytes).unwrap();
                let mut isolated_two_fill_form = false;
                let mut opacities = Vec::new();
                for object in pdf.objects.values() {
                    if let Ok(dict) = object.as_dict()
                        && dict.get(b"Type").and_then(lopdf::Object::as_name).ok()
                            == Some(b"ExtGState")
                        && let Ok(alpha) = dict.get(b"ca").and_then(lopdf::Object::as_float)
                    {
                        opacities.push(alpha);
                    }
                    if let Ok(stream) = object.as_stream() {
                        assert_ne!(
                            stream
                                .dict
                                .get(b"Subtype")
                                .and_then(lopdf::Object::as_name)
                                .ok(),
                            Some(b"Image".as_slice())
                        );
                        if stream
                            .dict
                            .get(b"Subtype")
                            .and_then(lopdf::Object::as_name)
                            .ok()
                            == Some(b"Form".as_slice())
                        {
                            let isolated = stream
                                .dict
                                .get(b"Group")
                                .and_then(lopdf::Object::as_dict)
                                .ok()
                                .and_then(|d| d.get(b"I").ok())
                                .and_then(|o| o.as_bool().ok())
                                == Some(true);
                            let content = stream.decompressed_content().unwrap_or_default();
                            let fills = lopdf::content::Content::decode(&content)
                                .ok()
                                .map_or(0, |c| {
                                    c.operations.iter().filter(|op| op.operator == "f").count()
                                });
                            isolated_two_fill_form |= isolated && fills >= 2;
                        }
                    }
                }
                assert!(
                    isolated_two_fill_form,
                    "two compound unions must share an isolated PDF group"
                );
                assert!(opacities.iter().any(|a| (*a - 0.5).abs() < 0.0001));
                assert!(
                    opacities
                        .iter()
                        .any(|a| (*a - 128.0 / 255.0).abs() < 0.0001)
                );
            }
            if let Some(dir) = std::env::var_os("SDOCX_MARKER3_SCALE_EXPORT") {
                let dir = std::path::Path::new(&dir);
                std::fs::write(dir.join(format!("{name}-baseline.svg")), &baseline).unwrap();
                std::fs::write(dir.join(format!("{name}-profile.svg")), &candidate).unwrap();
                for (label, opts) in [("baseline", &baseline_options), ("profile", &options)] {
                    std::fs::write(
                        dir.join(format!("{name}-{label}.pdf")),
                        crate::render_document_pdf(&document, opts, &Default::default()).unwrap(),
                    )
                    .unwrap();
                }
                if matches!(
                    name.as_str(),
                    "saved3-8-80336699-1"
                        | "saved3-8-80336699-2"
                        | "saved3-12-40be5520-1"
                        | "saved3-12-40be5520-2"
                ) {
                    for (paper_name, paper) in [
                        (
                            "e0c0a0",
                            Color {
                                r: 224,
                                g: 192,
                                b: 160,
                            },
                        ),
                        (
                            "445566",
                            Color {
                                r: 68,
                                g: 85,
                                b: 102,
                            },
                        ),
                    ] {
                        let mut tinted = document.clone();
                        tinted.pages[0].background_color = Some(paper);
                        tinted.metadata.dark_mode_compatibility = Some(false);
                        for (label, opts) in
                            [("baseline", &baseline_options), ("profile", &options)]
                        {
                            let stem = format!("paper-{name}-{paper_name}-{label}");
                            let svg = crate::render_document_svg(&tinted, opts).remove(0).svg;
                            std::fs::write(dir.join(format!("{stem}.svg")), svg).unwrap();
                            std::fs::write(
                                dir.join(format!("{stem}.pdf")),
                                crate::render_document_pdf(&tinted, opts, &Default::default())
                                    .unwrap(),
                            )
                            .unwrap();
                        }
                    }
                }
            }
        }
        assert_eq!(native["cases"].as_object().unwrap().len(), 27);
    }

    #[test]
    fn scale_profile_preserves_fallback_and_stamp_budget() {
        let input = Input {
            samples: vec![[10.0, 20.0], [5010.0, 20.0]],
            width: 8.0,
            pressures: Some(vec![1.0, 1.0]),
            tool_type_raw: 2,
            fixed_width: false,
        };
        let stroke = marker3_stroke(&input);
        let doc = |stroke: Stroke| crate::Document {
            pages: vec![crate::Page {
                uuid: "marker3-budget".into(),
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
        let baseline = crate::RenderOptions::default();
        let profile = crate::RenderOptions {
            marker3_output_scale: Some(crate::Marker3OutputScale::One),
            ..Default::default()
        };
        let long = doc(stroke.clone());
        let plain = crate::render_document_svg(&long, &baseline).remove(0).svg;
        assert!(plain.matches('M').count() > 4096);
        assert_eq!(
            plain,
            crate::render_document_svg(&long, &profile).remove(0).svg
        );
        #[cfg(feature = "pdf")]
        assert_eq!(
            crate::render_document_pdf(&long, &baseline, &Default::default()).unwrap(),
            crate::render_document_pdf(&long, &profile, &Default::default()).unwrap(),
            "over-budget direct PDF retains canonical paint"
        );
        for setting in [None, Some("4;"), Some("invalid")] {
            let mut s = stroke.clone();
            s.points.truncate(2);
            s.points[1].x = 30.0;
            s.rendering.as_mut().unwrap().advanced_settings = setting.map(str::to_owned);
            let document = doc(s);
            assert_eq!(
                crate::render_document_svg(&document, &baseline)[0].svg,
                crate::render_document_svg(&document, &profile)[0].svg,
                "setting {setting:?}"
            );
        }
        let mut unsupported_tool = stroke.clone();
        unsupported_tool.points[1].x = 30.0;
        unsupported_tool.rendering.as_mut().unwrap().tool_type_raw = 3;
        let document = doc(unsupported_tool);
        assert_eq!(
            crate::render_document_svg(&document, &baseline)[0].svg,
            crate::render_document_svg(&document, &profile)[0].svg,
            "tool 3"
        );
        let mut alias = stroke.clone();
        alias.points[1].x = 30.0;
        alias.rendering.as_mut().unwrap().pen_name =
            Some("com.samsung.android.sdk.pen.pen.preload.StraightMarker".into());
        let mut canonical = stroke.clone();
        canonical.points[1].x = 30.0;
        let alias_document = doc(alias);
        let canonical_document = doc(canonical);
        assert_ne!(
            crate::render_document_svg(&alias_document, &baseline)[0].svg,
            crate::render_document_svg(&alias_document, &profile)[0].svg
        );
        for scale in [
            crate::Marker3OutputScale::Half,
            crate::Marker3OutputScale::One,
            crate::Marker3OutputScale::Two,
            crate::Marker3OutputScale::Four,
            crate::Marker3OutputScale::Eight,
        ] {
            let options = crate::RenderOptions {
                marker3_output_scale: Some(scale),
                ..Default::default()
            };
            assert_eq!(
                crate::render_document_svg(&alias_document, &options)[0].svg,
                crate::render_document_svg(&canonical_document, &options)[0].svg
            );
            #[cfg(feature = "pdf")]
            assert_eq!(
                crate::render_document_pdf(&alias_document, &options, &Default::default()).unwrap(),
                crate::render_document_pdf(&canonical_document, &options, &Default::default())
                    .unwrap()
            );
        }
        #[cfg(feature = "pdf")]
        {
            let mut source = stroke.clone();
            source.points[1].x = 30.0;
            let document = doc(source);
            let layout = crate::layout_document(&document);
            let request =
                super::super::PageSceneRequest::sliced(&layout.pages[0], 0, 32, false).unwrap();
            let render = |options: &crate::RenderOptions| {
                let mut cache = super::super::DocumentTextCache::default();
                cache
                    .render_layout_page_scene_requests(
                        &document,
                        &[request],
                        options,
                        &super::super::fonts::FontBook::default(),
                        &mut |_| {},
                    )
                    .remove(0)
                    .page
                    .svg
            };
            assert_eq!(
                render(&baseline),
                render(&profile),
                "sliced output retains canonical paint"
            );
        }
        let mut zero = stroke;
        zero.points[1].x = 30.0;
        zero.rendering.as_mut().unwrap().style.pen_size = Some(0.4);
        let document = doc(zero);
        assert_eq!(
            crate::render_document_svg(&document, &baseline)[0].svg,
            crate::render_document_svg(&document, &profile)[0].svg
        );
    }

    #[test]
    fn public_marker3_exports_preserve_saved_radius_and_apply_alpha_once() {
        let fixture: Fixture = serde_json::from_str(include_str!(
            "../../../../conformance/marker3-saved-native.json"
        ))
        .unwrap();
        let case = fixture
            .cases
            .iter()
            .find(|case| case.id == "marker3-v5-saved-1")
            .unwrap();
        for curve in [false, true] {
            let mut stroke = marker3_stroke(&case.input);
            // Native Drawing controls cover both curve flags with saved size 8.
            // Presentation width and pressure must not shrink those native disks.
            stroke.pen_width = 1.07;
            let rendering = stroke.rendering.as_mut().unwrap();
            rendering.properties.compressed = curve;
            rendering.style.color_argb = Some(0x8033_6699);
            for point in &mut stroke.points {
                point.x += 24.0;
                point.y += 16.0;
            }
            let document = crate::Document {
                pages: vec![crate::Page {
                    uuid: "marker3".into(),
                    width: 128,
                    height: 32,
                    content_bbox: BoundingBox {
                        x_min: 19.5,
                        y_min: 11.5,
                        x_max: 48.5,
                        y_max: 20.5,
                    },
                    background_color: None,
                    template: None,
                    background: Default::default(),
                    objects: vec![crate::PageObject::from(stroke)],
                }],
                metadata: Default::default(),
            };
            let options = crate::RenderOptions::default();
            let pages = crate::render_document_svg(&document, &options);
            let parsed = roxmltree::Document::parse(&pages[0].svg).unwrap();
            let paths = parsed
                .descendants()
                .filter(|node| node.has_tag_name("path"))
                .collect::<Vec<_>>();
            assert_eq!(paths.len(), 1, "one compound MAX-union fill");
            let path = paths[0];
            let data = path.attribute("d").unwrap();
            assert_eq!(data.matches('M').count(), case.native.stamps.len());
            assert!(
                data.contains("a4.5,4.5"),
                "saved radius survives presentation width"
            );
            let alpha: f64 = path.attribute("fill-opacity").unwrap().parse().unwrap();
            assert!((alpha - 128.0 / 255.0).abs() < 0.0001);
            assert!(!parsed.descendants().any(|node| node.has_tag_name("image")));
            #[cfg(feature = "pdf")]
            {
                let pdf =
                    crate::render_document_pdf(&document, &options, &Default::default()).unwrap();
                let parsed = lopdf::Document::load_mem(&pdf).unwrap();
                assert!(!parsed.objects.values().any(|object| {
                    object.as_stream().is_ok_and(|stream| {
                        stream
                            .dict
                            .get(b"Subtype")
                            .is_ok_and(|value| value.as_name().is_ok_and(|name| name == b"Image"))
                    })
                }));
            }
        }
    }

    #[test]
    fn public_tool_three_exports_match_native_control_and_alias() {
        for samples in [
            vec![[0.0, 0.0], [20.0, 0.0]],
            vec![[0.0, 0.0], [10.0, 0.0], [20.0, 10.0]],
            vec![[10.0, 12.0], [24.0, 8.0], [38.0, 25.0], [55.0, 17.0]],
        ] {
            let input = Input {
                pressures: Some(if samples.len() == 4 {
                    vec![0.15, 0.9, 0.35, 0.75]
                } else {
                    vec![0.05; samples.len()]
                }),
                samples: samples.clone(),
                width: if samples.len() == 4 { 13.0 } else { 8.0 },
                tool_type_raw: 2,
                fixed_width: false,
            };
            let render = |tool, name: &str| {
                let mut stroke = marker3_stroke(&input);
                if input.samples.len() == 4 {
                    stroke.timestamps = vec![13, 26, 80, 151];
                }
                for point in &mut stroke.points {
                    point.x += 24.0;
                    point.y += 16.0;
                }
                let rendering = stroke.rendering.as_mut().unwrap();
                rendering.tool_type_raw = tool;
                rendering.pen_name =
                    Some(format!("com.samsung.android.sdk.pen.pen.preload.{name}"));
                rendering.style.color_argb = Some(0x803f_7d29);
                stroke.color = Some(Color {
                    r: 0x3f,
                    g: 0x7d,
                    b: 0x29,
                });
                let document = crate::Document {
                    pages: vec![crate::Page {
                        uuid: "marker3-control".into(),
                        width: 128,
                        height: 64,
                        content_bbox: BoundingBox {
                            x_min: 19.5,
                            y_min: 11.5,
                            x_max: 49.0,
                            y_max: 31.0,
                        },
                        background_color: None,
                        template: None,
                        background: Default::default(),
                        objects: vec![crate::PageObject::from(stroke)],
                    }],
                    metadata: Default::default(),
                };
                let options = crate::RenderOptions::default();
                let pages = crate::render_document_svg(&document, &options);
                #[cfg(feature = "pdf")]
                let pdf =
                    crate::render_document_pdf(&document, &options, &Default::default()).unwrap();
                #[cfg(feature = "pdf")]
                let svg_pdf = crate::render_svg_pages_pdf(&pages, &Default::default()).unwrap();
                (
                    pages[0].svg.clone(),
                    #[cfg(feature = "pdf")]
                    pdf,
                    #[cfg(feature = "pdf")]
                    svg_pdf,
                )
            };
            let expected = render(2, "Marker3");
            assert_eq!(render(3, "Marker3"), expected);
            assert_eq!(render(3, "StraightMarker"), expected);
            if input.samples.len() == 2 {
                assert_eq!(render(1, "Marker3"), expected);
            } else {
                assert_ne!(render(1, "Marker3").0, expected.0);
            }
            let parsed = roxmltree::Document::parse(&expected.0).unwrap();
            let path = parsed
                .descendants()
                .find(|n| n.has_tag_name("path"))
                .unwrap();
            let radius = if input.width == 13.0 {
                "a7,7"
            } else {
                "a4.5,4.5"
            };
            assert!(path.attribute("d").unwrap().contains(radius));
            assert!(
                (path
                    .attribute("fill-opacity")
                    .unwrap()
                    .parse::<f64>()
                    .unwrap()
                    - 128.0 / 255.0)
                    .abs()
                    < 0.0001
            );
        }
    }

    #[test]
    fn native_saved_case_builds_compound_vector_coverage_and_sdk_pdf() {
        let fixture: Fixture = serde_json::from_str(include_str!(
            "../../../../conformance/marker3-saved-native.json"
        ))
        .unwrap();
        let case = fixture
            .cases
            .into_iter()
            .find(|case| case.id == "marker3-v5-saved-1")
            .unwrap();
        let ink = crate::ink::marker3::prepare(&marker3_stroke(&case.input)).unwrap();
        assert_eq!(ink.points.len(), case.native.stamps.len());
        assert_eq!(ink.radii.len(), case.native.stamps.len());
        for ((point, radius), [x, y, native_radius]) in
            ink.points.iter().zip(&ink.radii).zip(&case.native.stamps)
        {
            assert_eq!((point.x as f32).to_bits(), x.to_bits());
            assert_eq!((point.y as f32).to_bits(), y.to_bits());
            assert_eq!((*radius as f32).to_bits(), native_radius.to_bits());
        }
        // Match the linked comparison harness: native stamps are translated by
        // (24, 16) into a 128x32 page. Original saved source points remain the
        // distinct generic-renderer input; these are sampled native paint stamps.
        let translated_points = ink
            .points
            .iter()
            .map(|point| Point {
                x: point.x + 24.0,
                y: point.y + 16.0,
            })
            .collect::<Vec<_>>();
        let mut scene = Scene::new(Svg::new().view_box(ViewBox::new(0.0, 0.0, 128.0, 32.0, 3)));
        assert!(append_coverage(
            &mut scene,
            &translated_points,
            &ink.radii,
            &Color {
                r: 0x33,
                g: 0x66,
                b: 0x99,
            },
        ));
        let svg = scene.finish();
        let parsed = roxmltree::Document::parse(&svg).unwrap();
        let paths = parsed
            .descendants()
            .filter(|node| node.has_tag_name("path"))
            .collect::<Vec<_>>();
        assert_eq!(paths.len(), COVERAGE_LEVELS);
        assert_eq!(
            paths.first().unwrap().attribute("fill-opacity"),
            Some("0.031250")
        );
        assert_eq!(
            paths.last().unwrap().attribute("fill-opacity"),
            Some("1.000000")
        );
        let first_path = paths.first().unwrap();
        let first_data = first_path.attribute("d").unwrap();
        let first_stamp_radius = ink.radii[0];
        // Level zero has the native radius unchanged. Check emitted cardinal
        // and diagonal vertices, not only the sampler's positions/radii.
        let cardinal = contour_vertex(translated_points[0], first_stamp_radius, 0.0, 0);
        let diagonal = contour_vertex(translated_points[0], first_stamp_radius, 0.0, 8);
        let emitted = first_data
            .split_whitespace()
            .take(CONTOUR_SIDES)
            .map(|token| {
                let (_, pair) = token.split_at(1);
                let (x, y) = pair.split_once(',').unwrap();
                (x.parse::<f64>().unwrap(), y.parse::<f64>().unwrap())
            })
            .collect::<Vec<_>>();
        for (index, expected) in [(0, cardinal), (8, diagonal)] {
            assert!((emitted[index].0 - expected.0).abs() < 1e-4);
            assert!((emitted[index].1 - expected.1).abs() < 1e-4);
        }
        if let Ok(directory) = std::env::var("SDOCX_MARKER3_DUMP") {
            let directory = PathBuf::from(directory);
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(directory.join("marker3-linked.svg"), &svg).unwrap();
        }
        #[cfg(feature = "pdf")]
        {
            let page = RenderedPage::new(0, 128, 32, svg);
            let bytes = crate::render_svg_pages_pdf(&[page], &Default::default()).unwrap();
            assert!(bytes.starts_with(b"%PDF-"));
            let document = lopdf::Document::load_mem(&bytes).unwrap();
            let image_xobjects = document
                .objects
                .values()
                .filter_map(|object| object.as_stream().ok())
                .filter(|stream| {
                    stream
                        .dict
                        .get(b"Subtype")
                        .is_ok_and(|value| value.as_name().is_ok_and(|name| name == b"Image"))
                })
                .count();
            assert_eq!(
                image_xobjects, 0,
                "vector Marker3 PDF must contain no image XObjects"
            );
            if let Ok(directory) = std::env::var("SDOCX_MARKER3_DUMP") {
                std::fs::write(PathBuf::from(directory).join("marker3-linked.pdf"), bytes).unwrap();
            }
        }
    }

    #[test]
    fn compound_layers_union_duplicate_centers_and_quantize_max_opacity() {
        let point = Point { x: 3.0, y: 4.0 };
        let mut scene = Scene::new(Svg::new().view_box(ViewBox::new(0.0, 0.0, 16.0, 16.0, 3)));
        assert!(append_coverage(
            &mut scene,
            &[point, point],
            &[4.5, 4.5],
            &Color { r: 1, g: 2, b: 3 }
        ));
        let svg = scene.finish();
        let parsed = roxmltree::Document::parse(&svg).unwrap();
        let paths = parsed
            .descendants()
            .filter(|node| node.has_tag_name("path"))
            .collect::<Vec<_>>();
        assert_eq!(paths.len(), COVERAGE_LEVELS);
        for (layer, path) in paths.iter().enumerate() {
            let data = path.attribute("d").unwrap();
            assert_eq!(
                data.matches('M').count(),
                2,
                "two duplicate subpaths retained in one MAX-union fill"
            );
            let opacity: f64 = path.attribute("fill-opacity").unwrap().parse().unwrap();
            assert!((opacity - 1.0 / (COVERAGE_LEVELS - layer) as f64).abs() < 1e-6);
        }
        for vertex in [0, 8, 16] {
            let (x, y) = contour_vertex(point, 4.5, 1.0 / 32.0, vertex);
            if vertex == 0 || vertex == 16 {
                assert!((x - 7.46875).abs() < 1e-12 || (y - 8.46875).abs() < 1e-12);
            } else {
                let expected = (4.5 - 2.0_f64.sqrt() / 32.0) / 2.0_f64.sqrt();
                assert!((x - (3.0 + expected)).abs() < 1e-12);
                assert!((y - (4.0 + expected)).abs() < 1e-12);
            }
        }
        // Source-over stacking of the quantized nested levels yields exactly
        // k/32 cumulative coverage after level k (up to decimal serialization).
        let mut accumulated = 0.0;
        for (layer, path) in paths.iter().enumerate() {
            let opacity: f64 = path.attribute("fill-opacity").unwrap().parse().unwrap();
            accumulated += (1.0 - accumulated) * opacity;
            assert!((accumulated - (layer + 1) as f64 / 32.0).abs() < 2e-6);
        }
    }
}

#[cfg(all(test, feature = "serde", feature = "pdf"))]
mod tool_one_export_control {
    use super::tests::{Input, marker3_stroke};
    use crate::{BoundingBox, Color, Document, Page, RenderOptions};

    #[test]
    fn export_tool_one_profile_control() {
        let input = Input {
            samples: vec![
                [25.5, 28.75],
                [32.5, 39.75],
                [42.5, 30.75],
                [49.5, 41.75],
                [55.5, 31.75],
            ],
            width: 8.0,
            pressures: Some(vec![0.1, 0.9, 0.3, 0.8, 0.2]),
            tool_type_raw: 1,
            fixed_width: false,
        };
        let mut stroke = marker3_stroke(&input);
        stroke.rendering.as_mut().unwrap().style.color_argb = Some(0x8033cc55);
        stroke.color = Some(Color {
            r: 0x33,
            g: 0xcc,
            b: 0x55,
        });
        let document = Document {
            pages: vec![Page {
                uuid: "marker3-tool1".into(),
                width: 128,
                height: 128,
                content_bbox: BoundingBox::default(),
                background_color: None,
                template: None,
                background: Default::default(),
                objects: vec![stroke.into()],
            }],
            metadata: Default::default(),
        };
        let baseline = crate::render_document_svg(&document, &RenderOptions::default())[0]
            .svg
            .clone();
        let options = RenderOptions {
            marker3_output_scale: Some(crate::Marker3OutputScale::One),
            ..Default::default()
        };
        let profile = crate::render_document_svg(&document, &options)[0]
            .svg
            .clone();
        assert_ne!(baseline, profile);
        let double = RenderOptions {
            marker3_output_scale: Some(crate::Marker3OutputScale::Two),
            ..Default::default()
        };
        assert_ne!(
            baseline,
            crate::render_document_svg(&document, &double)[0].svg
        );
        let quadruple = RenderOptions {
            marker3_output_scale: Some(crate::Marker3OutputScale::Four),
            ..Default::default()
        };
        assert_ne!(
            baseline,
            crate::render_document_svg(&document, &quadruple)[0].svg
        );
        let mut straight = document.clone();
        if let crate::PageObjectContent::Stroke(stroke) = &mut straight.pages[0].objects[0].content
        {
            stroke.rendering.as_mut().unwrap().pen_name =
                Some("com.samsung.android.sdk.pen.pen.preload.StraightMarker".into());
        } else {
            panic!("expected stroke");
        }
        for setting in ["", "5;"] {
            if let crate::PageObjectContent::Stroke(stroke) =
                &mut straight.pages[0].objects[0].content
            {
                stroke.rendering.as_mut().unwrap().advanced_settings = Some(setting.into());
            }
            assert_eq!(
                crate::render_document_svg(&document, &RenderOptions::default())[0].svg,
                crate::render_document_svg(&straight, &RenderOptions::default())[0].svg
            );
            assert_eq!(
                crate::render_document_svg(&document, &double)[0].svg,
                crate::render_document_svg(&straight, &double)[0].svg
            );
            assert_eq!(
                crate::render_document_svg(&document, &quadruple)[0].svg,
                crate::render_document_svg(&straight, &quadruple)[0].svg
            );
        }
        for (size, color) in [
            (Some(3.99), Some(0x8033cc55)),
            (Some(9.01), Some(0x8033cc55)),
            (Some(8.0), None),
        ] {
            let mut unsupported = document.clone();
            if let crate::PageObjectContent::Stroke(stroke) =
                &mut unsupported.pages[0].objects[0].content
            {
                let style = &mut stroke.rendering.as_mut().unwrap().style;
                style.pen_size = size;
                style.color_argb = color;
            } else {
                panic!("expected stroke");
            }
            assert_eq!(
                crate::render_document_svg(&unsupported, &RenderOptions::default())[0].svg,
                crate::render_document_svg(&unsupported, &double)[0].svg
            );
            assert_eq!(
                crate::render_document_svg(&unsupported, &RenderOptions::default())[0].svg,
                crate::render_document_svg(&unsupported, &quadruple)[0].svg
            );
        }
        for scale in [
            crate::Marker3OutputScale::Half,
            crate::Marker3OutputScale::Eight,
        ] {
            let outside = RenderOptions {
                marker3_output_scale: Some(scale),
                ..Default::default()
            };
            assert_eq!(
                baseline,
                crate::render_document_svg(&document, &outside)[0].svg
            );
            assert_eq!(
                crate::render_document_pdf(
                    &document,
                    &RenderOptions::default(),
                    &Default::default()
                )
                .unwrap(),
                crate::render_document_pdf(&document, &outside, &Default::default()).unwrap()
            );
        }
        let mut unsupported = document.clone();
        if let crate::PageObjectContent::Stroke(stroke) =
            &mut unsupported.pages[0].objects[0].content
        {
            stroke.rendering.as_mut().unwrap().advanced_settings = Some("unknown".into());
        } else {
            panic!("expected stroke");
        }
        assert_eq!(
            crate::render_document_svg(&unsupported, &RenderOptions::default())[0].svg,
            crate::render_document_svg(&unsupported, &options)[0].svg
        );
        assert_eq!(
            crate::render_document_svg(&unsupported, &RenderOptions::default())[0].svg,
            crate::render_document_svg(&unsupported, &quadruple)[0].svg
        );
        let Some(dir) = std::env::var_os("SDOCX_MARKER3_TOOL1_EXPORT") else {
            return;
        };
        for (name, scale) in [
            ("baseline", None),
            ("profile", Some(crate::Marker3OutputScale::One)),
        ] {
            let options = RenderOptions {
                marker3_output_scale: scale,
                ..Default::default()
            };
            std::fs::write(
                std::path::Path::new(&dir).join(format!("{name}.svg")),
                &crate::render_document_svg(&document, &options)[0].svg,
            )
            .unwrap();
            std::fs::write(
                std::path::Path::new(&dir).join(format!("{name}.pdf")),
                crate::render_document_pdf(&document, &options, &Default::default()).unwrap(),
            )
            .unwrap();
        }
    }
}
