// Analytic shader approximation for the three observed draw programs.
pub fn pieces(buffers: &[Vec<[f32; 5]>; 3], n: usize, scale: f64) -> Vec<(Vec<[f64; 2]>, f64)> {
    let mut result = Vec::new();
    let planes: [(f64, f64); 32] = std::array::from_fn(|side| {
        let angle = std::f64::consts::TAU * (side as f64 + 0.5) / 32.0;
        angle.sin_cos()
    });
    let limit = 0.5 * (std::f64::consts::PI / 32.0).cos();
    let mut poly = Vec::with_capacity(35);
    let mut next = Vec::with_capacity(35);
    for (program, buffer) in [3, 4, 5].into_iter().zip(buffers) {
        for t in buffer.as_chunks::<3>().0 {
            let triangle = [
                t[0].map(f64::from),
                t[1].map(f64::from),
                t[2].map(f64::from),
            ];
            let p = triangle.map(|v| [scale * v[0], -scale * v[1]]);
            let det = (p[1][0] - p[0][0]) * (p[2][1] - p[0][1])
                - (p[2][0] - p[0][0]) * (p[1][1] - p[0][1]);
            let mut derivative = [[0.0; 2]; 2];
            if det.abs() >= 1e-14 {
                for k in 0..2 {
                    let a = triangle[1][k + 2] - triangle[0][k + 2];
                    let b = triangle[2][k + 2] - triangle[0][k + 2];
                    derivative[k] = [
                        (a * (p[2][1] - p[0][1]) - b * (p[1][1] - p[0][1])) / det,
                        ((p[1][0] - p[0][0]) * b - (p[2][0] - p[0][0]) * a) / det,
                    ];
                }
            }
            let vertex = |i: usize, j: usize| {
                let u = i as f64 / n as f64;
                let v = j as f64 / n as f64;
                std::array::from_fn::<_, 5, _>(|k| {
                    (1.0 - u - v) * triangle[0][k] + u * triangle[1][k] + v * triangle[2][k]
                })
            };
            for i in 0..n {
                for j in 0..n - i {
                    let a = vertex(i, j);
                    let b = vertex(i + 1, j);
                    let c = vertex(i, j + 1);
                    for patch in [
                        Some([a, b, c]),
                        (i + j + 2 <= n).then(|| [b, vertex(i + 1, j + 1), c]),
                    ]
                    .into_iter()
                    .flatten()
                    {
                        poly.clear();
                        poly.extend_from_slice(&patch);
                        for &(sn, cs) in &planes {
                            let distance =
                                |v: &[f64; 5]| limit - ((v[2] - 0.5) * cs + (v[3] - 0.5) * sn);
                            next.clear();
                            if !poly.is_empty() {
                                for i in 0..poly.len() {
                                    let a = poly[i];
                                    let b = poly[(i + 1) % poly.len()];
                                    let da = distance(&a);
                                    let db = distance(&b);
                                    if da >= 0.0 {
                                        next.push(a);
                                    }
                                    if (da >= 0.0) != (db >= 0.0) {
                                        let t = da / (da - db);
                                        next.push(std::array::from_fn(|k| {
                                            a[k] + t * (b[k] - a[k])
                                        }));
                                    }
                                }
                            }
                            std::mem::swap(&mut poly, &mut next);
                            if poly.is_empty() {
                                break;
                            }
                        }
                        if poly.len() >= 3 {
                            let centre = std::array::from_fn::<_, 5, _>(|k| {
                                if poly.len() == 3 {
                                    (poly[0][k] + poly[1][k] + poly[2][k]) / 3.0
                                } else {
                                    poly.iter().map(|v| v[k]).sum::<f64>() / poly.len() as f64
                                }
                            });
                            let u = centre[2];
                            let v = centre[3];
                            let d = (u - 0.5).hypot(v - 0.5);
                            let fw = if d == 0.0 {
                                0.0
                            } else {
                                (((u - 0.5) * derivative[0][0] + (v - 0.5) * derivative[1][0]) / d)
                                    .abs()
                                    + (((u - 0.5) * derivative[0][1]
                                        + (v - 0.5) * derivative[1][1])
                                        / d)
                                        .abs()
                            };
                            let smooth = |x: f64| {
                                let z = if fw == 0.0 {
                                    if x < 0.5 { 0.0 } else { 1.0 }
                                } else {
                                    ((x - (0.5 - fw)) / fw).clamp(0.0, 1.0)
                                };
                                z * z * (3.0 - 2.0 * z)
                            };
                            let alpha = match program {
                                3 => {
                                    centre[4]
                                        * (1.0 - (0.93 / 0.4) * (0.4 - (0.5 - (0.5 - v).abs())))
                                            .clamp(0.0, 1.0)
                                        * (if d < 0.5 {
                                            1.0
                                        } else if d > 0.5 {
                                            -1.0
                                        } else {
                                            0.0
                                        })
                                        * (1.0 - smooth((0.5 - u).abs()))
                                }
                                4 => centre[4] * v * (1.0 - smooth(d)),
                                _ => {
                                    centre[4]
                                        * (2.0 * (1.0 - v)).clamp(0.0, 1.0)
                                        * (1.0 - smooth(d))
                                }
                            }
                            .clamp(0.0, 1.0);
                            if alpha > 0.0 {
                                result.push((
                                    poly.iter().map(|v| [v[0], -v[1]]).collect(),
                                    (alpha * 10000.0).round() / 10000.0,
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
    result
}

pub(super) fn render(
    scene: &mut super::vector::Scene,
    buffers: &[Vec<[f32; 5]>; 3],
    color: &str,
    remaining: &std::cell::Cell<usize>,
    scale: f64,
) -> bool {
    use super::vector::{Data, Group, Paint, Path, Styled};
    let Some(count) = buffers
        .iter()
        .map(|b| b.len() / 3)
        .sum::<usize>()
        .checked_mul(64)
    else {
        return false;
    };
    if count > remaining.get() || count > 32_768 {
        return false;
    }
    let patches = pieces(buffers, 8, scale);
    if patches
        .iter()
        .any(|(p, a)| !a.is_finite() || p.iter().flatten().any(|v| !v.is_finite()))
    {
        return false;
    }
    let mut group = Group::new().fill(Paint::from_hex(color));
    for (p, alpha) in patches {
        let mut data = Data::new().move_to((p[0][0], p[0][1]));
        for point in &p[1..] {
            data = data.line_to((point[0], point[1]));
        }
        group = group.add(Path::new().data(data.close()).fill_opacity(alpha));
    }
    scene.push(group);
    remaining.set(remaining.get() - count);
    true
}

#[test]
fn shared_fill_preserves_individual_patch_opacity() {
    use super::vector::{Scene, Svg};
    let triangle = vec![
        [0., 0., 0.3, 0.3, 0.8],
        [8., 0., 0.7, 0.3, 0.8],
        [0., 8., 0.3, 0.7, 0.8],
    ];
    let buffers = [triangle.clone(), triangle, vec![]];
    let expected = pieces(&buffers, 8, 1.0).len();
    let mut scene = Scene::new(Svg::new().width(16).height(16));
    let budget = std::cell::Cell::new(128);
    assert!(render(&mut scene, &buffers, "#336699", &budget, 1.0));
    let svg = scene.finish();
    assert_eq!(budget.get(), 0);
    assert_eq!(svg.matches("fill=\"#336699\"").count(), 1);
    assert_eq!(svg.matches("<path").count(), expected);
    assert_eq!(svg.matches("fill-opacity=").count(), expected);
}

#[cfg(feature = "pdf")]
#[test]
fn inherited_fill_matches_separate_overlapping_patches_under_transform() {
    use super::vector::{Data, Paint, Path, Scene, Styled, Svg};
    let triangle = vec![
        [1., -1., 0.3, 0.3, 0.8],
        [9., -1., 0.7, 0.3, 0.8],
        [1., -9., 0.3, 0.7, 0.8],
    ];
    let buffers = [triangle.clone(), triangle, vec![]];
    let mut grouped = Scene::new(Svg::new().width(32).height(32));
    assert!(render(
        &mut grouped,
        &buffers,
        "#336699",
        &std::cell::Cell::new(128),
        1.0
    ));
    let mut separate = Scene::new(Svg::new().width(32).height(32));
    for (points, alpha) in pieces(&buffers, 8, 1.0) {
        let mut data = Data::new().move_to((points[0][0], points[0][1]));
        for point in &points[1..] {
            data = data.line_to((point[0], point[1]));
        }
        separate.push(
            Path::new()
                .data(data.close())
                .fill(Paint::from_hex("#336699"))
                .fill_opacity(alpha),
        );
    }
    let raster = |svg: String| {
        let svg = svg
            .replacen('>', "><g transform=\"translate(5 4) scale(1.4)\">", 1)
            .replace("</svg>", "</g></svg>");
        let tree = resvg::usvg::Tree::from_str(&svg, &Default::default()).unwrap();
        let mut pixels = resvg::tiny_skia::Pixmap::new(32, 32).unwrap();
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::identity(),
            &mut pixels.as_mut(),
        );
        pixels.data().to_vec()
    };
    assert_eq!(raster(grouped.finish()), raster(separate.finish()));
}

#[cfg(feature = "pdf")]
#[test]
fn captured_controls_shared_scene_exports() {
    use super::vector::{Data, Paint, Path, Rectangle, Scene, Styled, Svg};
    use std::io::Write;
    let output = std::env::var_os("SDOCX_MONTBLANC_CAPTURE_DIR").map(std::path::PathBuf::from);
    for (case, (name, end, pressure, width)) in [
        ("baseline", 20., 0.6, 2.),
        ("pressure", 20., 0.85, 2.),
        ("size", 20., 0.6, 1.5),
        ("far_endpoint", 30., 0.6, 2.),
    ]
    .into_iter()
    .enumerate()
    {
        let mut stroke = crate::Stroke {
            rendering: None,
            bbox: crate::BoundingBox {
                x_min: 0.,
                y_min: 0.,
                x_max: 256.,
                y_max: 256.,
            },
            points: vec![],
            pressures: vec![],
            timestamps: vec![],
            tilts: vec![],
            orientations: vec![],
            color: None,
            pen_width: width,
        };
        stroke.points = vec![
            crate::Point { x: 128., y: 128. },
            crate::Point { x: 138., y: 128. },
            crate::Point {
                x: 128. + end,
                y: 126.,
            },
        ];
        stroke.pressures = vec![0.3, pressure, if name == "pressure" { 0.9 } else { 1. }];
        stroke.pen_width = width;
        let mut buffers = crate::ink::montblanc::tests::captured_control_buffers(&stroke, case);
        for vertex in buffers.iter_mut().flatten() {
            vertex[0] += 128.0;
            vertex[1] -= 128.0;
        }
        let triangles = pieces(&buffers, 8, 1.0);
        assert!(!triangles.is_empty());
        let piece_count = triangles.len();
        assert!(
            triangles
                .iter()
                .all(|(_, alpha)| *alpha > 0.0 && *alpha <= 1.0)
        );
        let mut scene = Scene::new(Svg::new().width(256).height(256));
        scene.retain_text();
        scene.push(
            Rectangle::new()
                .width(256)
                .height(256)
                .fill(Paint::from_hex("#fcfcfc")),
        );
        for (points, alpha) in triangles {
            let mut data = Data::new().move_to((points[0][0], points[0][1]));
            for point in &points[1..] {
                data = data.line_to((point[0], point[1]));
            }
            let data = data.close();
            scene.push(
                Path::new()
                    .data(data)
                    .fill(Paint::from_hex("#336699"))
                    .fill_opacity(alpha),
            );
        }
        let text = scene.take_native_text();
        let ink = scene.take_native_ink();
        let text_error = scene.take_native_text_error();
        let svg = scene.finish();
        let retained = super::RenderedScene {
            page: super::RenderedPage {
                source_page_index: 0,
                width: 256,
                height: 256,
                svg: svg.clone(),
                text_diagnostics: vec![],
                object_diagnostics: vec![],
                geometry_diagnostics: vec![],
                paint_diagnostics: vec![],
                background_diagnostics: vec![],
            },
            text,
            ink,
            text_error,
        };
        let pdf =
            crate::pdf::render_native_scene_pdf_for_test(&retained, &crate::PdfOptions::default())
                .unwrap();
        assert_eq!(svg.matches("<path").count(), piece_count);
        let document = lopdf::Document::load_mem(&pdf).unwrap();
        assert_eq!(document.get_pages().len(), 1);
        assert!(
            !document
                .get_page_content(*document.get_pages().values().next().unwrap())
                .unwrap()
                .is_empty()
        );
        if let Some(output) = &output {
            for (extension, data) in [("svg", svg.as_bytes()), ("pdf", pdf.as_slice())] {
                let path = output.join(format!("{name}.{extension}"));
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(path)
                    .unwrap();
                file.write_all(data).unwrap();
            }
        }
    }
}

#[test]
fn cutoff_clips_to_circle_and_keeps_inside_triangle() {
    let inside = vec![
        [0., 0., 0.4, 0.4, 0.8],
        [8., 0., 0.6, 0.4, 0.8],
        [0., 8., 0.4, 0.6, 0.8],
    ];
    let pieces = pieces(&[inside.clone(), vec![], vec![]], 1, 1.0);
    assert_eq!(pieces.len(), 1);
    assert_eq!(pieces[0].0, vec![[0., 0.], [8., 0.], [0., -8.]]);
    let crossing = vec![
        [0., 0., 0.5, 0.5, 0.8],
        [8., 0., 1.2, 0.5, 0.8],
        [0., 8., 0.5, 1.2, 0.8],
    ];
    let clipped = super::montblanc_candidate::pieces(&[crossing, vec![], vec![]], 1, 1.0);
    assert_eq!(clipped.len(), 1);
    assert!(clipped[0].0.len() > 3 && clipped[0].0.len() <= 35);
    for p in &clipped[0].0 {
        let u = 0.5 + p[0] * 0.7 / 8.0;
        let v = 0.5 - p[1] * 0.7 / 8.0;
        assert!((u - 0.5).hypot(v - 0.5) <= 0.50000001);
    }
}
