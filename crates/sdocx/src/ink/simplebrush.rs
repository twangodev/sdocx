//! Test-only SimpleBrush saved-ObjectStroke geometry reconstruction.
//!
//! This is a numeric geometry oracle only. It does not model the shader,
//! color upload, blending, GPU pixels, or SVG/PDF rendering.

use super::path::{P, Quad};

#[derive(Clone, Copy)]
struct EdgePair([P; 2]);

fn midpoint(a: P, b: P) -> P {
    a.mid(b)
}

fn pressure_step(previous: f32, input: f32, tool_type: i32) -> f32 {
    let requested = if tool_type == 1 { 0.5 } else { input };
    requested.max(previous * 0.9).min(previous * 1.1).min(1.0)
}

fn half_width(size: f32, pressure: f32) -> f32 {
    size.mul_add(pressure, size) * 0.5
}

// Source CurveStyle::setValue (SimpleBrush DSO RVA 0x1c408) computes both
// endpoint offsets in float32. Preserve the fused multiply-add sites and the
// scalar multiply order from that source routine.
fn offset_edges(point: P, dx: f32, dy: f32, width: f32) -> [P; 2] {
    let dx_squared = dx * dx;
    let length_squared = (-dy).mul_add(-dy, dx_squared);
    let length = length_squared.sqrt();
    let determinant = (-point.y).mul_add(dx, point.x * dy);
    let negative_dot = (-dy).mul_add(point.y, (-point.x) * dx);
    let half_width = width * 0.5;

    let edge = |offset: f32| {
        let scaled_offset = offset.mul_add(length, determinant);
        let x_term = scaled_offset * dy;
        let y_term = scaled_offset * (-dx);
        P {
            x: (-dx).mul_add(negative_dot, x_term) / length_squared,
            y: (-dy).mul_add(negative_dot, y_term) / length_squared,
        }
    };

    [edge(-half_width), edge(half_width)]
}

// setValue stores the midpoint of its previous/current endpoint edge pairs.
fn set_value_edges(previous: P, current: P, old_width: f32, new_width: f32) -> EdgePair {
    let dx = current.x - previous.x;
    let dy = current.y - previous.y;
    let old_edges = offset_edges(previous, dx, dy, old_width);
    let new_edges = offset_edges(current, dx, dy, new_width);
    EdgePair([
        midpoint(old_edges[0], new_edges[0]),
        midpoint(old_edges[1], new_edges[1]),
    ])
}

// drawLine and endPen measure the stored cross-section as the next path's
// start width. The subtraction and FMA order matches the source helper.
fn edge_pair_width(pair: EdgePair) -> f32 {
    let dy = pair.0[0].y - pair.0[1].y;
    let dx = pair.0[0].x - pair.0[1].x;
    dx.mul_add(dx, dy * dy).sqrt()
}

fn get_repeat(length: f32, width: f32) -> u32 {
    let scaled = if width <= 5.0 {
        length * 0.5
    } else if width <= 10.0 {
        length / 3.0
    } else if width <= 15.0 {
        length * 0.25
    } else if width <= 20.0 {
        length / 5.0
    } else if width <= 30.0 {
        length / 6.0
    } else if width <= 40.0 {
        length / 7.0
    } else if width <= 55.0 {
        length * 0.125
    } else {
        length / 10.0
    };
    (scaled as u32).max(2) + 1
}

fn append_path(curve: &Quad, start_width: f32, end_width: f32, out: &mut Vec<[f32; 3]>) {
    let repeat = get_repeat(curve.length, start_width.min(end_width));
    let distance_step = curve.length / repeat as f32;
    let width_step = (end_width - start_width) / repeat as f32;
    let (mut distance, mut width) = (0.0_f32, start_width);

    for _ in 0..=repeat {
        let point = curve.at(distance);
        let radius = (width + 1.0) * 0.5;
        // RT AddPoint stores radius + 0.5 in its third float lane.
        out.push([point.x, point.y, radius + 0.5]);
        distance += distance_step;
        width += width_step;
    }
}

fn reconstruct(
    points: &[[f32; 2]],
    pressures: &[f32],
    size: f32,
    tool_type: i32,
    curve_enabled: bool,
) -> Result<Vec<[f32; 3]>, &'static str> {
    if !curve_enabled || points.len() < 2 || points.len() != pressures.len() {
        return Err("unsupported SimpleBrush input shape");
    }
    if !size.is_finite() || size <= 0.0 {
        return Err("invalid pen size");
    }

    let mut previous_pressure = if tool_type == 1 {
        0.5
    } else {
        pressures[0].min(1.0)
    };
    let mut previous_width = half_width(size, previous_pressure).max(1.0);
    let mut edge_pair = None;
    let mut samples = Vec::new();

    for index in 1..points.len() {
        let previous = P {
            x: points[index - 1][0],
            y: points[index - 1][1],
        };
        let current = P {
            x: points[index][0],
            y: points[index][1],
        };
        if (current.x - previous.x).abs() < 1.5 && (current.y - previous.y).abs() < 1.5 {
            return Err("SimpleBrush drawLine movement is below the native gate");
        }

        let pressure = pressure_step(previous_pressure, pressures[index], tool_type);
        let next_width = half_width(size, pressure);
        let mean_width = (previous_width + next_width) * 0.5;
        let (start, control, end) = if index == 1 {
            (previous, previous, midpoint(previous, current))
        } else {
            let prior = P {
                x: points[index - 2][0],
                y: points[index - 2][1],
            };
            (
                midpoint(prior, previous),
                previous,
                midpoint(previous, current),
            )
        };
        let curve = Quad::new(start, control, end);
        let start_width = if index == 1 {
            mean_width
        } else {
            edge_pair
                .map(edge_pair_width)
                .ok_or("missing source-derived cross-section")?
        };
        append_path(&curve, start_width, mean_width, &mut samples);

        edge_pair = Some(set_value_edges(
            previous,
            current,
            previous_width,
            next_width,
        ));
        previous_pressure = pressure;
        previous_width = next_width;
    }

    let last = points.len() - 1;
    let terminal_pressure = pressure_step(previous_pressure, pressures[last], tool_type);
    let terminal_width = half_width(size, terminal_pressure);
    let terminal_start_width = edge_pair
        .map(edge_pair_width)
        .ok_or("missing terminal source-derived cross-section")?;
    let a = P {
        x: points[last - 1][0],
        y: points[last - 1][1],
    };
    let b = P {
        x: points[last][0],
        y: points[last][1],
    };
    let curve = Quad::new(midpoint(a, b), a, b);
    append_path(&curve, terminal_start_width, terminal_width, &mut samples);
    Ok(samples)
}

#[cfg(test)]
mod tests {
    use super::reconstruct;
    use serde_json::Value;
    use sha2::{Digest, Sha256};

    const FIXTURE: &str = include_str!("../../../../conformance/simplebrush-native.json");
    const ORACLE: &[u8] = include_bytes!("../../../../conformance/simplebrush-native.bin");

    fn decode_hex(text: &str) -> Vec<u8> {
        assert_eq!(text.len() % 2, 0);
        (0..text.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&text[index..index + 2], 16).unwrap())
            .collect()
    }

    fn float_from_bits(value: &Value) -> f32 {
        let bytes: [u8; 4] = decode_hex(value.as_str().unwrap()).try_into().unwrap();
        f32::from_le_bytes(bytes)
    }

    #[test]
    fn saved_object_stroke_geometry_matches_native_vectors() {
        let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
        let cases = fixture["cases"].as_array().unwrap();
        let oracle_hash = format!("{:x}", Sha256::digest(ORACLE));
        assert_eq!(
            fixture["provenance"]["binary_oracle"]["sha256"],
            oracle_hash
        );

        for case in cases {
            let input = &case["input"];
            let points: Vec<[f32; 2]> = input["points_f32_bits"]
                .as_array()
                .unwrap()
                .iter()
                .map(|point| {
                    let xy = point.as_array().unwrap();
                    [float_from_bits(&xy[0]), float_from_bits(&xy[1])]
                })
                .collect();
            let pressures: Vec<f32> = input["pressure_f32_bits"]
                .as_array()
                .unwrap()
                .iter()
                .map(float_from_bits)
                .collect();
            let size = float_from_bits(&input["pen_size_f32_bits"]);
            let tool_type = input["tool_type"].as_i64().unwrap() as i32;
            let samples = reconstruct(
                &points,
                &pressures,
                size,
                tool_type,
                input["curve_enabled"].as_bool().unwrap(),
            )
            .unwrap_or_else(|error| panic!("{}: {error}", case["id"].as_str().unwrap()));

            let mut bytes = Vec::with_capacity(samples.len() * 12);
            for [x, y, radius_plus_half] in samples {
                bytes.extend_from_slice(&x.to_le_bytes());
                bytes.extend_from_slice(&y.to_le_bytes());
                bytes.extend_from_slice(&radius_plus_half.to_le_bytes());
            }

            let native = &case["native_oracle"];
            let offset = native["vector_offset_bytes"].as_u64().unwrap() as usize;
            let length = native["vector_length_bytes"].as_u64().unwrap() as usize;
            let expected = &ORACLE[offset..offset + length];
            assert_eq!(
                bytes,
                expected,
                "{} vector mismatch",
                case["id"].as_str().unwrap()
            );
            assert_eq!(
                format!("{:x}", Sha256::digest(&bytes)),
                native["vector_sha256"].as_str().unwrap(),
                "{} native vector SHA mismatch",
                case["id"].as_str().unwrap()
            );
        }
    }

    // This compares a source-derived 1px radial fringe with the actual generic
    // page renderer. It is a single host-Mesa control, not a parity claim.
    #[cfg(feature = "pdf")]
    #[test]
    fn v25_candidate_and_generic_fallback_against_host_paint() {
        use crate::{
            BoundingBox, Color, Document, Page, Point, Stroke, StrokeProperties, StrokeRendering,
            StrokeStyle,
        };
        use std::fmt::Write;

        // OpenGL bottom-up rows, cropped at x=0..32, y=0..16. Full-frame
        // SHA-256 87b8a6ac6ef0fef178230c78a1fa8c8f49946e41c61074b59e4d533a12de638e;
        // all nonzero alpha lies in this crop. Source: V25 host-Mesa capture.
        let native = include_bytes!("../../../../conformance/simplebrush-paint-v25-crop.rgba");
        let receipt: Value = serde_json::from_str(include_str!(
            "../../../../conformance/simplebrush-paint-v25.json"
        ))
        .unwrap();
        assert_eq!(
            format!("{:x}", Sha256::digest(native)),
            receipt["paint_observation"]["crop"]["sha256"]
                .as_str()
                .unwrap()
        );
        let points = [[0., 0.], [10., 0.], [20., 5.]];
        let pressures = [0.5, 0.25, 0.75];
        let samples = reconstruct(&points, &pressures, 8., 2, true).unwrap();
        assert_eq!(samples.len(), 14);
        let geometry: Vec<u8> = samples
            .iter()
            .flatten()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        assert_eq!(
            format!("{:x}", Sha256::digest(&geometry)),
            "498063f7e145bf52f54dfd893061c7f3dd293d35d43c0cfce7c31bb2e88e1530"
        );
        let mut svg = String::from(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="1920" height="1080" viewBox="0 0 1920 1080"><defs>"##,
        );
        for (i, &[x, y, r]) in samples.iter().enumerate() {
            let y = 1080.0 - f64::from(y);
            let inner = (f64::from(r) - 1.0).max(0.0) / f64::from(r);
            write!(svg, r##"<radialGradient id="g{i}" gradientUnits="userSpaceOnUse" cx="{x}" cy="{y}" r="{r}"><stop offset="0" stop-color="#44aaee"/><stop offset="{inner}" stop-color="#44aaee"/><stop offset="1" stop-color="#44aaee" stop-opacity="0"/></radialGradient>"##).unwrap();
        }
        svg.push_str("</defs>");
        for (i, &[x, y, r]) in samples.iter().enumerate() {
            let y = 1080.0 - f64::from(y);
            write!(
                svg,
                r##"<circle cx="{x}" cy="{y}" r="{r}" fill="url(#g{i})"/>"##
            )
            .unwrap();
        }
        svg.push_str("</svg>");

        let stroke = Stroke {
            rendering: Some(StrokeRendering {
                metadata: None,
                pen_name: Some("com.samsung.android.sdk.pen.pen.preload.Brush".into()),
                advanced_settings: None,
                tool_type_raw: 2,
                properties: StrokeProperties {
                    compressed: true,
                    replay_only: false,
                    stylus_channels: false,
                    eraser: false,
                    fixed_width: false,
                    millisecond_timestamps: false,
                    top_layer_pen: false,
                    alpha_lock: false,
                    binary_added: true,
                    generated: false,
                    fixed_opacity: false,
                    rainbow_effect: false,
                    straighten: false,
                    reveal_mode: false,
                },
                style: StrokeStyle {
                    color_argb: Some(0x8044_aaee),
                    pen_size: Some(8.),
                    ..StrokeStyle::default()
                },
            }),
            bbox: BoundingBox::default(),
            points: points
                .iter()
                .map(|p| Point {
                    x: f64::from(p[0]),
                    y: 1080. - f64::from(p[1]),
                })
                .collect(),
            pressures: pressures.iter().copied().map(f64::from).collect(),
            timestamps: vec![],
            tilts: vec![],
            orientations: vec![],
            color: Some(Color {
                r: 0x44,
                g: 0xaa,
                b: 0xee,
            }),
            pen_width: 8.,
        };
        // The native fragment shader ignores input alpha in this capture. The
        // generic Brush route also uses whole-stroke opacity 1, despite the
        // saved 0x80 alpha; compare the actual routes, not an assumed opacity.
        assert_eq!(crate::prepare_stroke(&stroke, false).opacity, 1.0);
        let document = Document {
            pages: vec![Page {
                uuid: "simplebrush-v25".into(),
                width: 1920,
                height: 1080,
                content_bbox: BoundingBox::default(),
                background_color: None,
                template: None,
                background: Default::default(),
                objects: vec![stroke.into()],
            }],
            metadata: Default::default(),
        };
        let layout = crate::layout_document(&document);
        let request =
            crate::render::PageSceneRequest::sliced(&layout.pages[0], 0, 1080, false).unwrap();
        let mut cache = crate::render::DocumentTextCache::default();
        let fallback = cache
            .render_layout_page_scene_requests(
                &document,
                &[request],
                &crate::RenderOptions::default(),
                &crate::render::fonts::FontBook::default(),
                &mut |_| {},
            )
            .remove(0)
            .page
            .svg;
        if let Some(directory) = std::env::var_os("SDOCX_SIMPLEBRUSH_EXPORT_DIR") {
            let directory = std::path::Path::new(&directory);
            std::fs::create_dir_all(directory).unwrap();
            for (name, source) in [("candidate", &svg), ("fallback", &fallback)] {
                let page = crate::RenderedPage::new(0, 1920, 1080, source.clone());
                let pdf = crate::render_svg_pages_pdf(&[page], &Default::default()).unwrap();
                std::fs::write(directory.join(format!("{name}.pdf")), pdf).unwrap();
            }
        }
        let raster = |source: &str| {
            let tree =
                resvg::usvg::Tree::from_str(source, &resvg::usvg::Options::default()).unwrap();
            let mut pixmap = resvg::tiny_skia::Pixmap::new(1920, 1080).unwrap();
            resvg::render(
                &tree,
                resvg::tiny_skia::Transform::identity(),
                &mut pixmap.as_mut(),
            );
            pixmap
        };
        let candidate = raster(&svg);
        let fallback = raster(&fallback);
        // Both use the same transparent, bottom-up native reference. Count
        // support intersection/union and absolute alpha error across the crop.
        let metrics = |image: &resvg::tiny_skia::Pixmap| {
            let mut intersection = 0usize;
            let mut union = 0usize;
            let mut alpha_error = 0u64;
            let mut rgba_error = 0u64;
            let mut support = 0usize;
            for y in 0..16 {
                for x in 0..32 {
                    let n = native[(y * 32 + x) * 4 + 3];
                    let a = image.data()[((1079 - y) * 1920 + x) * 4 + 3];
                    intersection += usize::from(n != 0 && a != 0);
                    union += usize::from(n != 0 || a != 0);
                    support += usize::from(a != 0);
                    alpha_error += u64::from(n.abs_diff(a));
                    let native_pixel = &native[(y * 32 + x) * 4..][..4];
                    let output_pixel = &image.data()[((1079 - y) * 1920 + x) * 4..][..4];
                    rgba_error += native_pixel
                        .iter()
                        .zip(output_pixel)
                        .map(|(native, actual)| u64::from(native.abs_diff(*actual)))
                        .sum::<u64>();
                }
            }
            (intersection, union, support, alpha_error, rgba_error)
        };
        let candidate_metrics = metrics(&candidate);
        let fallback_metrics = metrics(&fallback);
        for (name, image, inside_support) in [
            ("candidate", &candidate, candidate_metrics.2),
            ("fallback", &fallback, fallback_metrics.2),
        ] {
            let full_support = image
                .data()
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|p| p[3] != 0)
                .count();
            assert_eq!(
                full_support, inside_support,
                "{name} extends outside the crop"
            );
        }
        eprintln!(
            "v25 candidate (intersection, union, support, alpha error, RGBA error): {candidate_metrics:?}; fallback: {fallback_metrics:?}"
        );
        assert_eq!(
            native
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|p| p[3] != 0)
                .count(),
            137
        );
        assert_eq!(
            candidate_metrics.2, 137,
            "retained source-derived candidate support"
        );
        assert!(
            candidate_metrics.0 * fallback_metrics.1 > fallback_metrics.0 * candidate_metrics.1
        );
        assert!(candidate_metrics.3 < fallback_metrics.3);
        assert!(candidate_metrics.4 < fallback_metrics.4);
    }
}
