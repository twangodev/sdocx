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
    struct Input {
        samples: Vec<[f64; 2]>,
        width: f32,
        pressures: Option<Vec<f64>>,
        tool_type_raw: u16,
        fixed_width: bool,
    }

    #[derive(Deserialize)]
    struct Native {
        stamps: Vec<[f32; 3]>,
    }

    fn marker3_stroke(input: &Input) -> Stroke {
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
