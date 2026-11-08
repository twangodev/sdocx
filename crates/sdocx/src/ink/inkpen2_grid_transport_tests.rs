//! Test-only explicit final-grid transport. This does not enable renderer dispatch.
use super::{Input, Runtime, redraw_v1_bounded};
use crate::ink::path::P;
use std::fmt::Write as _;

mod model {
    #![allow(dead_code)]
    include!("../../../../tools/inkpen2-v1-tablet-half/bounded_grid_zero_width.rs");
}

#[derive(Clone, Copy)]
struct Grid {
    width: i32,
    height: i32,
    translation: [f64; 2],
}

// The budget is divided across submissions before any SVG is returned, so a
// failure cannot publish a partial stroke or bypass the aggregate limits.
fn transport(grid: Grid, strokes: &[Vec<u8>], limits: [usize; 4]) -> Option<String> {
    transport_colored(
        grid,
        &strokes
            .iter()
            .map(|s| (s.as_slice(), [0x33, 0x66, 0x99]))
            .collect::<Vec<_>>(),
        limits,
    )
}

fn transport_colored(
    grid: Grid,
    strokes: &[(&[u8], [u8; 3])],
    limits: [usize; 4],
) -> Option<String> {
    if !(1..=8192).contains(&grid.width)
        || !(1..=8192).contains(&grid.height)
        || strokes.is_empty()
        || strokes.len() > 256
    {
        return None;
    }
    let count = strokes.len();
    let per_stroke = [limits[0] / count, limits[1] / count, limits[2] / count];
    let mut layers = Vec::with_capacity(count);
    for &(stroke, color) in strokes {
        layers.push((
            model::grid(
                stroke,
                grid.width,
                grid.height,
                0.5,
                grid.translation[0],
                grid.translation[1],
                per_stroke,
            )?,
            color,
        ));
    }
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\">",
        grid.width, grid.height, grid.width, grid.height
    );
    for (layer, color) in layers {
        for ((x, y), alpha) in layer {
            write!(svg, "<rect x=\"{x}\" y=\"{}\" width=\"1\" height=\"1\" fill=\"#{:02x}{:02x}{:02x}\" fill-opacity=\"{}\"/>", grid.height - 1 - y, color[0], color[1], color[2], f64::from(alpha) / 255.).ok()?;
            if svg.len() > limits[3] {
                return None;
            }
        }
    }
    svg.push_str("</svg>");
    (svg.len() <= limits[3]).then_some(svg)
}

#[test]
fn zero_width_native_frames_match_explicit_grid_model() {
    use base64::Engine as _;
    use sha2::{Digest, Sha256};
    use std::io::Read as _;

    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/inkpen2-v1-screen-paint-native.json"
    ))
    .unwrap();
    let originals = &fixture["half_grid_controls_original"];
    let read = |name: &str| -> serde_json::Value {
        serde_json::from_str(originals[name].as_str().unwrap()).unwrap()
    };
    let zero = read("zero-width-native.json");
    let integer = read("integer-phase.json");
    let translated = read("translated-phase.json");
    assert_eq!(zero["cases"].as_array().unwrap().len(), 3);
    for row in zero["cases"].as_array().unwrap() {
        let geometry = row["geometry_hex"]
            .as_str()
            .unwrap()
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect::<Vec<_>>();
        let compressed = base64::engine::general_purpose::STANDARD
            .decode(row["rgba_zlib_base64"].as_str().unwrap())
            .unwrap();
        let mut rgba = Vec::new();
        flate2::read::ZlibDecoder::new(compressed.as_slice())
            .read_to_end(&mut rgba)
            .unwrap();
        assert_eq!(rgba.len(), 64 * 48 * 4);
        assert_eq!(format!("{:x}", Sha256::digest(&rgba)), row["rgba_sha256"]);
        let grid = model::grid(&geometry, 64, 48, 0.5, 17., 17., [1, 1000, 1000]).unwrap();
        for (i, pixel) in rgba.as_chunks::<4>().0.iter().enumerate() {
            let alpha = grid
                .get(&((i % 64) as i32, (i / 64) as i32))
                .copied()
                .unwrap_or(0);
            assert_eq!(pixel[3], alpha);
        }
    }
    let integer_cases = integer["cases"].as_array().unwrap();
    assert_eq!(integer_cases.len(), 8);
    let matching = integer_cases
        .iter()
        .find(|row| row["case"] == "short" && row["translation"] == serde_json::json!([1, 1]))
        .unwrap();
    assert_eq!(matching["rgba_sha256"], zero["cases"][0]["rgba_sha256"]);
    // The historical predictor recorded a known error at this quad phase.
    assert_eq!(matching["metrics"]["mismatches"], 4);
    let captures: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tools/inkpen2-v1-tablet-half/receipt.json"
    ))
    .unwrap();
    let translated_cases = translated["cases"].as_array().unwrap();
    assert_eq!(translated_cases.len(), 8);
    for row in translated_cases {
        let name = row["case"].as_str().unwrap();
        let hex = captures["cases"][name]["geometry_hex"].as_str().unwrap();
        let geometry: Vec<_> = hex
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        let compressed = base64::engine::general_purpose::STANDARD
            .decode(row["rgba_zlib_base64"].as_str().unwrap())
            .unwrap();
        let mut rgba = Vec::new();
        flate2::read::ZlibDecoder::new(compressed.as_slice())
            .read_to_end(&mut rgba)
            .unwrap();
        assert_eq!(rgba.len(), 64 * 48 * 4);
        assert_eq!(format!("{:x}", Sha256::digest(&rgba)), row["rgba_sha256"]);
        let dx = row["translation"][0].as_f64().unwrap();
        let dy = row["translation"][1].as_f64().unwrap();
        let grid = model::grid(
            &geometry,
            64,
            48,
            0.5,
            16. + dx,
            16. + dy,
            [2000, 200_000, 6144],
        )
        .unwrap();
        let mut sum = 0_u64;
        let mut max = 0_u8;
        for (index, pixel) in rgba.as_chunks::<4>().0.iter().enumerate() {
            let alpha = grid
                .get(&((index % 64) as i32, (index / 64) as i32))
                .copied()
                .unwrap_or(0);
            let error = pixel[3].abs_diff(alpha);
            sum += u64::from(error);
            max = max.max(error);
        }
        assert!(
            sum <= row["metrics"]["absolute_error"].as_u64().unwrap(),
            "{name}: {dx}, {dy}"
        );
        assert!(max <= 1, "{name}: {dx}, {dy}");
    }
}

#[test]
fn viewport_and_phase_controls_match_native_error_bounds() {
    use base64::Engine as _;
    use sha2::{Digest, Sha256};
    use std::io::Read as _;
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/inkpen2-v1-screen-paint-native.json"
    ))
    .unwrap();
    let captures: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tools/inkpen2-v1-tablet-half/receipt.json"
    ))
    .unwrap();
    let originals = &fixture["half_grid_more_original"];
    let read = |name: &str| -> serde_json::Value {
        serde_json::from_str(originals[name].as_str().unwrap()).unwrap()
    };
    let decode_hex = |value: &str| {
        value
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect::<Vec<_>>()
    };
    let compare = |row: &serde_json::Value,
                   geometry: &[u8],
                   width: i32,
                   height: i32,
                   scale: f64,
                   x: f64,
                   y: f64| {
        let compressed = base64::engine::general_purpose::STANDARD
            .decode(row["rgba_zlib_base64"].as_str().unwrap())
            .unwrap();
        let mut rgba = Vec::new();
        flate2::read::ZlibDecoder::new(compressed.as_slice())
            .read_to_end(&mut rgba)
            .unwrap();
        assert_eq!(rgba.len(), width as usize * height as usize * 4);
        assert_eq!(format!("{:x}", Sha256::digest(&rgba)), row["rgba_sha256"]);
        let grid =
            model::grid(geometry, width, height, scale, x, y, [2000, 200_000, 6144]).unwrap();
        let mut sum = 0_u64;
        let mut max = 0_u8;
        for (i, pixel) in rgba.as_chunks::<4>().0.iter().enumerate() {
            let actual = grid
                .get(&((i % width as usize) as i32, (i / width as usize) as i32))
                .copied()
                .unwrap_or(0);
            let error = pixel[3].abs_diff(actual);
            sum += u64::from(error);
            max = max.max(error);
        }
        (sum, max)
    };
    let phases = read("phase-model.json");
    assert_eq!(phases["cases"].as_array().unwrap().len(), 5);
    for row in phases["cases"].as_array().unwrap() {
        let geometry = decode_hex(row["geometry_hex"].as_str().unwrap());
        assert_eq!(
            compare(
                row,
                &geometry,
                64,
                48,
                0.5,
                16. + row["phase"][0].as_f64().unwrap(),
                16. + row["phase"][1].as_f64().unwrap()
            ),
            (0, 0)
        );
    }
    for (file, expected_count) in [("viewport-controls.json", 8), ("edge-controls.json", 6)] {
        let report = read(file);
        let rows = report["cases"].as_array().unwrap();
        assert_eq!(rows.len(), expected_count);
        for row in rows {
            let name = row["source"]
                .as_str()
                .or_else(|| row["case"].as_str())
                .unwrap();
            let geometry = decode_hex(captures["cases"][name]["geometry_hex"].as_str().unwrap());
            let c = &row["control"];
            let (sum, max) = compare(
                row,
                &geometry,
                c[0].as_i64().unwrap() as i32,
                c[1].as_i64().unwrap() as i32,
                c[2].as_f64().unwrap(),
                c[3].as_f64().unwrap(),
                c[4].as_f64().unwrap(),
            );
            assert!(sum <= row["metrics"]["absolute_error"].as_u64().unwrap());
            assert!(max <= 1);
        }
    }
    let pins = read("phase-pins.json");
    assert_eq!(
        pins["tools/inkpen2-v1-tablet-half/receipt.json"],
        "ff4a42391828bf32d21675fc993a85f29d0f2407d976681551934f5733b7ccc0"
    );
}

#[test]
fn colored_native_overlap_matches_explicit_grid_transport() {
    use base64::Engine as _;
    use sha2::{Digest, Sha256};
    use std::io::Read as _;
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/inkpen2-v1-screen-paint-native.json"
    ))
    .unwrap();
    let originals = &fixture["half_grid_color_original"];
    let read = |name: &str| -> serde_json::Value {
        serde_json::from_str(originals[name].as_str().unwrap()).unwrap()
    };
    let frame = |capture: &serde_json::Value| {
        let data = base64::engine::general_purpose::STANDARD
            .decode(capture["rgba_zlib_base64"].as_str().unwrap())
            .unwrap();
        let mut rgba = Vec::new();
        flate2::read::ZlibDecoder::new(data.as_slice())
            .read_to_end(&mut rgba)
            .unwrap();
        assert_eq!(rgba.len(), 64 * 48 * 4);
        assert_eq!(
            format!("{:x}", Sha256::digest(&rgba)),
            capture["rgba_sha256"]
        );
        rgba
    };
    let white = read("white-native-turn.json");
    frame(&white);
    for name in [
        "nonwhite-native-turn.json",
        "overlap-native.json",
        "overlap-colors-native.json",
    ] {
        let receipt = read(name);
        for target in ["transparent", "background"] {
            frame(&receipt["captures"][target]);
        }
    }
    let receipt = read("overlap-colors-native.json");
    let capture = &receipt["captures"]["transparent"];
    let decode = |name: &str| {
        capture[name]
            .as_str()
            .unwrap()
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect::<Vec<_>>()
    };
    // The overlap capture's geometry field was overwritten by the second
    // stroke. The first stroke is retained in the pinned turn receipt.
    let source: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tools/inkpen2-v1-tablet-half/receipt.json"
    ))
    .unwrap();
    let first = source["cases"]["turn"]["geometry_hex"]
        .as_str()
        .unwrap()
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect::<Vec<_>>();
    let second = decode("second_geometry_hex");
    let svg = transport_colored(
        Grid {
            width: 64,
            height: 48,
            translation: [16., 16.],
        },
        &[(&first, [255, 0, 0]), (&second, [0, 0, 255])],
        [4000, 400_000, 12_288, 1_000_000],
    )
    .unwrap();
    let tree = resvg::usvg::Tree::from_str(&svg, &Default::default()).unwrap();
    let mut pixels = resvg::tiny_skia::Pixmap::new(64, 48).unwrap();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixels.as_mut(),
    );
    let native = frame(capture);
    let mut l1 = 0_u64;
    for y in 0..48 {
        for x in 0..64 {
            let a = (y * 64 + x) * 4;
            let b = ((47 - y) * 64 + x) * 4;
            for c in 0..4 {
                l1 += u64::from(pixels.data()[a + c].abs_diff(native[b + c]));
            }
        }
    }
    assert!(
        l1 <= receipt["measurements"]["svg_premultiplied_rgba_l1"]
            .as_u64()
            .unwrap()
    );
}

#[test]
fn mixed_scene_native_frames_and_line_defaults_are_preserved() {
    use sha2::{Digest, Sha256};
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/inkpen2-v1-screen-paint-native.json"
    ))
    .unwrap();
    let originals = &fixture["mixed_scene_original"];
    let read = |name: &str| -> serde_json::Value {
        serde_json::from_str(originals[name].as_str().unwrap()).unwrap()
    };
    let receipt = read("mixed-scene-native-line-receipt-v2.json");
    let getter = read("mixed-scene-native-line-getters-receipt.json");
    let style = crate::shape::ShapeStyle::default();
    let native = &getter["line_getters"];
    assert_eq!(style.cap, native["cap"].as_u64().unwrap() as u8);
    assert_eq!(style.join, native["join"].as_u64().unwrap() as u8);
    assert_eq!(style.dash, native["dash"].as_u64().unwrap() as u8);
    assert_eq!(style.compound, native["compound"].as_u64().unwrap() as u8);
    assert_eq!(
        style.begin_arrow[0],
        native["begin_arrow_type"].as_u64().unwrap() as u8
    );
    assert_eq!(
        style.end_arrow[0],
        native["end_arrow_type"].as_u64().unwrap() as u8
    );
    assert!(matches!(
        style.paint,
        crate::shape::ShapePaint::Solid(0xff000000)
    ));
    assert_eq!(native["solid_color_argb"], "0xff000000");
    assert_eq!(native["width"], getter["paint_trace"]["width"]);
    for (key, name) in [
        ("offset", "mixed-scene-offset-input.json"),
        ("rotated", "mixed-scene-rotated-input.json"),
    ] {
        let text = originals[name].as_str().unwrap();
        assert_eq!(
            format!("{:x}", Sha256::digest(text.as_bytes())),
            receipt["additional_controls"][key]["sha256"][name]
        );
    }
    let mut count = 0;
    for frames in [
        &receipt["retained_frames"],
        &receipt["additional_controls"]["offset"]["retained_frames"],
        &receipt["additional_controls"]["rotated"]["retained_frames"],
    ] {
        for frame in frames.as_object().unwrap().values() {
            let palette = frame["palette"].as_array().unwrap();
            let mut raw = Vec::new();
            for run in frame["runs"].as_array().unwrap() {
                let repeat = run[0].as_u64().unwrap() as usize;
                let pixel = palette[run[1].as_u64().unwrap() as usize]
                    .as_array()
                    .unwrap();
                assert_eq!(pixel.len(), 4);
                for _ in 0..repeat {
                    raw.extend(pixel.iter().map(|v| v.as_u64().unwrap() as u8));
                }
            }
            assert_eq!(raw.len(), 64 * 48 * 4);
            assert_eq!(format!("{:x}", Sha256::digest(&raw)), frame["raw_sha256"]);
            count += 1;
        }
    }
    assert_eq!(count, 6);
    assert_eq!(
        getter["readback"]["before_sha256"],
        receipt["observation"]["before_sha256"]
    );
    assert_eq!(
        getter["readback"]["after_sha256"],
        receipt["observation"]["after_sha256"]
    );
}

#[test]
fn native_color_matrix_preserves_alpha_and_grid_transport() {
    use base64::Engine as _;
    use sha2::{Digest, Sha256};
    use std::io::Read as _;
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/inkpen2-v1-screen-paint-native.json"
    ))
    .unwrap();
    let packed = base64::engine::general_purpose::STANDARD
        .decode(
            fixture["color_matrix_original_gzip_base64"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
    let mut original = Vec::new();
    flate2::read::GzDecoder::new(packed.as_slice())
        .read_to_end(&mut original)
        .unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(&original)),
        "bf7289ea03c49cb59103067d7647036587c7ab509fd079740491c30c074c6520"
    );
    let receipt: serde_json::Value = serde_json::from_slice(&original).unwrap();
    let cases = receipt["cases"].as_object().unwrap();
    assert_eq!(cases.len(), 10);
    for (name, row) in cases {
        let data = base64::engine::general_purpose::STANDARD
            .decode(row["rgba_zlib_base64"].as_str().unwrap())
            .unwrap();
        let mut rgba = Vec::new();
        flate2::read::ZlibDecoder::new(data.as_slice())
            .read_to_end(&mut rgba)
            .unwrap();
        assert_eq!(rgba.len(), 64 * 48 * 4);
        assert_eq!(
            format!("{:x}", Sha256::digest(&rgba)),
            row["rgba_sha256"],
            "{name}"
        );
        let geometry = row["geometry_hex"]
            .as_str()
            .unwrap()
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            format!("{:x}", Sha256::digest(&geometry)),
            row["geometry_sha256"]
        );
        if name.ends_with(":transparent") {
            let argb = row["source_color_argb"].as_u64().unwrap() as u32;
            let color = [(argb >> 16) as u8, (argb >> 8) as u8, argb as u8];
            let svg = transport_colored(
                Grid {
                    width: 64,
                    height: 48,
                    translation: [16., 16.],
                },
                &[(&geometry, color)],
                [2000, 200_000, 6144, 500_000],
            )
            .unwrap();
            let tree = resvg::usvg::Tree::from_str(&svg, &Default::default()).unwrap();
            let mut pixels = resvg::tiny_skia::Pixmap::new(64, 48).unwrap();
            resvg::render(
                &tree,
                resvg::tiny_skia::Transform::identity(),
                &mut pixels.as_mut(),
            );
            let mut error = 0_u64;
            for y in 0..48 {
                for x in 0..64 {
                    let a = (y * 64 + x) * 4;
                    let b = ((47 - y) * 64 + x) * 4;
                    for c in 0..4 {
                        error += u64::from(pixels.data()[a + c].abs_diff(rgba[b + c]));
                    }
                }
            }
            assert!(error <= 200, "{name}: RGBA error {error}");
        }
    }
    for background in ["transparent", "white"] {
        assert_eq!(
            cases[&format!("80336699:{background}")]["rgba_sha256"],
            cases[&format!("20336699:{background}")]["rgba_sha256"]
        );
    }
}

#[test]
fn explicit_final_grid_source_public_transport() {
    use base64::Engine as _;
    use std::io::Read as _;
    let captures: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tools/inkpen2-v1-tablet-half/receipt.json"
    ))
    .unwrap();
    let phases: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tools/inkpen2-v1-tablet-half/multi-phase-model.json"
    ))
    .unwrap();
    let limits = [2000, 200_000, 6144, 500_000];
    for row in phases["cases"].as_array().unwrap() {
        let name = row["case"].as_str().unwrap();
        let input = &captures["cases"][name]["input"];
        let points: Vec<P> = input["points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| P {
                x: p[0].as_f64().unwrap() as f32,
                y: p[1].as_f64().unwrap() as f32,
            })
            .collect();
        let output = redraw_v1_bounded(
            Input {
                points: &points,
                times: &[0, 10, 20, 30],
                pressures: &[0.2, 0.4, 0.6, 0.8],
                tools: &[3; 4],
                rgb: [0; 3],
                rainbow: None,
                size: input["size"].as_f64().unwrap() as f32,
                curve_enabled: true,
                is_shape: false,
                runtime: Runtime {
                    inverse_scale: [1.; 2],
                    spacing: 1.,
                    tolerance: 2.,
                    tool_threshold: 20.,
                    resolution_min: 1440,
                    initial_alternation: false,
                    initial_time: 0,
                },
            },
            10000,
        )
        .unwrap();
        let geometry: Vec<u8> = output
            .vertices
            .iter()
            .flat_map(|v| v[..3].iter().flat_map(|f| f.to_le_bytes()))
            .collect();
        let geometry_hex = captures["cases"][name]["geometry_hex"].as_str().unwrap();
        let native_geometry: Vec<u8> = geometry_hex
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        assert_eq!(geometry, native_geometry, "{name}: native geometry");
        let grid = Grid {
            width: 64,
            height: 48,
            translation: [
                16. + row["phase"][0].as_f64().unwrap(),
                16. + row["phase"][1].as_f64().unwrap(),
            ],
        };
        if name == "turn"
            && row["phase"][0].as_f64() == Some(0.)
            && row["phase"][1].as_f64() == Some(0.)
        {
            let shifted_points: Vec<P> = points
                .iter()
                .map(|p| P {
                    x: p.x + 4.,
                    y: p.y + 2.,
                })
                .collect();
            let shifted = redraw_v1_bounded(
                Input {
                    points: &shifted_points,
                    times: &[0, 10, 20, 30],
                    pressures: &[0.2, 0.4, 0.6, 0.8],
                    tools: &[3; 4],
                    rgb: [0; 3],
                    rainbow: None,
                    size: input["size"].as_f64().unwrap() as f32,
                    curve_enabled: true,
                    is_shape: false,
                    runtime: Runtime {
                        inverse_scale: [1.; 2],
                        spacing: 1.,
                        tolerance: 2.,
                        tool_threshold: 20.,
                        resolution_min: 1440,
                        initial_alternation: false,
                        initial_time: 0,
                    },
                },
                10000,
            )
            .unwrap();
            let shifted_geometry: Vec<u8> = shifted
                .vertices
                .iter()
                .flat_map(|v| v[..3].iter().flat_map(|f| f.to_le_bytes()))
                .collect();
            // Transparent and background captures independently produced the same
            // 252-byte native geometry stream.
            use sha2::{Digest as _, Sha256};
            assert_eq!(shifted_geometry.len(), 252);
            assert_eq!(
                format!("{:x}", Sha256::digest(&shifted_geometry)),
                "fe20f56a73c5bbab7488038fc19afc4265767b29d66c92ea8c7b9c53697ec1d2"
            );
            let svg = transport_colored(
                grid,
                &[(&geometry, [255, 0, 0]), (&shifted_geometry, [0, 0, 255])],
                [4000, 400_000, 12_288, 1_000_000],
            )
            .unwrap();
            let pdf = crate::render_svg_pages_pdf(
                &[crate::RenderedPage::new(0, 64, 48, svg.clone())],
                &Default::default(),
            )
            .unwrap();
            assert!(pdf.starts_with(b"%PDF"));
        }
        // Two differently colored source-derived layers exercise ordering and
        // alpha composition independently of the single-color capture.
        let overlap = transport_colored(
            grid,
            &[(&geometry, [255, 0, 0]), (&geometry, [0, 0, 255])],
            [4000, 400_000, 12_288, 1_000_000],
        )
        .unwrap();
        let reversed = transport_colored(
            grid,
            &[(&geometry, [0, 0, 255]), (&geometry, [255, 0, 0])],
            [4000, 400_000, 12_288, 1_000_000],
        )
        .unwrap();
        let render = |svg: &str| {
            let tree = resvg::usvg::Tree::from_str(svg, &Default::default()).unwrap();
            let mut pixels = resvg::tiny_skia::Pixmap::new(64, 48).unwrap();
            resvg::render(
                &tree,
                resvg::tiny_skia::Transform::identity(),
                &mut pixels.as_mut(),
            );
            pixels
        };
        let forward_pixels = render(&overlap);
        let reverse_pixels = render(&reversed);
        assert!(
            forward_pixels
                .data()
                .as_chunks::<4>()
                .0
                .iter()
                .zip(reverse_pixels.data().as_chunks::<4>().0.iter())
                .any(|(a, b)| a[2] > a[0] && b[0] > b[2]),
            "{name}: overlap order"
        );
        for svg in [overlap, reversed] {
            let pdf = crate::render_svg_pages_pdf(
                &[crate::RenderedPage::new(0, 64, 48, svg)],
                &Default::default(),
            )
            .unwrap();
            assert!(pdf.starts_with(b"%PDF"));
        }
        let svg = transport(grid, std::slice::from_ref(&geometry), limits).unwrap();
        let tree = resvg::usvg::Tree::from_str(&svg, &Default::default()).unwrap();
        let mut pixmap = resvg::tiny_skia::Pixmap::new(64, 48).unwrap();
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::identity(),
            &mut pixmap.as_mut(),
        );
        let compressed = base64::engine::general_purpose::STANDARD
            .decode(row["rgba_zlib_base64"].as_str().unwrap())
            .unwrap();
        let mut expected = Vec::new();
        flate2::read::ZlibDecoder::new(&compressed[..])
            .read_to_end(&mut expected)
            .unwrap();
        let error: u32 = pixmap
            .data()
            .as_chunks::<4>()
            .0
            .iter()
            .enumerate()
            .map(|(i, p)| u32::from(p[3].abs_diff(expected[((47 - i / 64) * 64 + i % 64) * 4 + 3])))
            .sum();
        assert!(error <= 8, "{name}: alpha L1 {error}");
        // A slice keeps the original grid's bottom-left parity; only its
        // top-left SVG viewBox is rebased. Use an odd-sized final viewport.
        let odd = Grid {
            width: 65,
            height: 47,
            translation: grid.translation,
        };
        let odd_svg = transport(odd, std::slice::from_ref(&geometry), limits).unwrap();
        let odd_tree = resvg::usvg::Tree::from_str(&odd_svg, &Default::default()).unwrap();
        let mut full = resvg::tiny_skia::Pixmap::new(65, 47).unwrap();
        resvg::render(
            &odd_tree,
            resvg::tiny_skia::Transform::identity(),
            &mut full.as_mut(),
        );
        let slice_svg = odd_svg.replacen(
            "height=\"47\" viewBox=\"0 0 65 47\"",
            "height=\"23\" viewBox=\"0 24 65 23\"",
            1,
        );
        assert_ne!(slice_svg, odd_svg);
        let slice_tree = resvg::usvg::Tree::from_str(&slice_svg, &Default::default()).unwrap();
        let mut slice = resvg::tiny_skia::Pixmap::new(65, 23).unwrap();
        resvg::render(
            &slice_tree,
            resvg::tiny_skia::Transform::identity(),
            &mut slice.as_mut(),
        );
        assert_eq!(slice.data(), &full.data()[24 * 65 * 4..]);
        let odd_pdf = crate::render_svg_pages_pdf(
            &[crate::RenderedPage::new(0, 65, 23, slice_svg)],
            &Default::default(),
        )
        .unwrap();
        assert!(odd_pdf.starts_with(b"%PDF"));
        let page = crate::RenderedPage::new(0, 64, 48, svg);
        let pdf = crate::render_svg_pages_pdf(&[page], &Default::default()).unwrap();
        assert!(pdf.starts_with(b"%PDF"));
        assert!(
            transport(
                grid,
                &[geometry.clone(), geometry.clone()],
                [0, limits[1], limits[2], limits[3]]
            )
            .is_none()
        );
        assert!(
            transport(
                grid,
                std::slice::from_ref(&geometry),
                [limits[0], 0, limits[2], limits[3]]
            )
            .is_none()
        );
        assert!(
            transport(
                grid,
                std::slice::from_ref(&geometry),
                [limits[0], limits[1], 0, limits[3]]
            )
            .is_none()
        );
        assert!(transport(grid, &[geometry], [limits[0], limits[1], limits[2], 1]).is_none());
    }
}

#[test]
fn aggregate_grid_budgets_are_atomic() {
    let grid = Grid {
        width: 16,
        height: 16,
        translation: [0., 0.],
    };
    let stamp: Vec<u8> = [4_f32, 4., 4.]
        .into_iter()
        .flat_map(f32::to_le_bytes)
        .collect();
    let strokes = [stamp.clone(), stamp];
    let pixels = model::grid(&strokes[0], 16, 16, 0.5, 0., 0., [1, 16, 16])
        .unwrap()
        .len();
    assert!(pixels > 0);
    let limits = [2, 32, 2 * pixels, 10_000];
    let svg = transport(grid, &strokes, limits).unwrap();
    assert!(transport(grid, &strokes, [1, limits[1], limits[2], limits[3]]).is_none());
    assert!(transport(grid, &strokes, [limits[0], 31, limits[2], limits[3]]).is_none());
    assert!(
        transport(
            grid,
            &strokes,
            [limits[0], limits[1], 2 * pixels - 1, limits[3]]
        )
        .is_none()
    );
    assert!(
        transport(
            grid,
            &strokes,
            [limits[0], limits[1], limits[2], svg.len() - 1]
        )
        .is_none()
    );
    assert_eq!(
        transport(grid, &strokes, [limits[0], limits[1], limits[2], svg.len()]).unwrap(),
        svg
    );
}
