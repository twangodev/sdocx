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
