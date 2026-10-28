// Analytic shader approximation for the three observed draw programs.
pub fn pieces(buffers: &[Vec<[f32; 5]>; 3], n: usize) -> Vec<([[f64; 2]; 3], f64)> {
    let mut result = Vec::new();
    for (program, buffer) in [3, 4, 5].into_iter().zip(buffers) {
        for t in buffer.chunks_exact(3) {
            let triangle = [
                t[0].map(f64::from),
                t[1].map(f64::from),
                t[2].map(f64::from),
            ];
            let p = triangle.map(|v| [128.0 + v[0], 128.0 - v[1]]);
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
                        (i + j <= n - 2).then(|| [b, vertex(i + 1, j + 1), c]),
                    ]
                    .into_iter()
                    .flatten()
                    {
                        let centre = std::array::from_fn::<_, 5, _>(|k| {
                            (patch[0][k] + patch[1][k] + patch[2][k]) / 3.0
                        });
                        let u = centre[2];
                        let v = centre[3];
                        let d = (u - 0.5).hypot(v - 0.5);
                        let fw = if d == 0.0 {
                            0.0
                        } else {
                            (((u - 0.5) * derivative[0][0] + (v - 0.5) * derivative[1][0]) / d)
                                .abs()
                                + (((u - 0.5) * derivative[0][1] + (v - 0.5) * derivative[1][1])
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
                            _ => centre[4] * (2.0 * (1.0 - v)).clamp(0.0, 1.0) * (1.0 - smooth(d)),
                        }
                        .clamp(0.0, 1.0);
                        if alpha > 0.0 {
                            result.push((
                                patch.map(|v| [128.0 + v[0], 128.0 - v[1]]),
                                (alpha * 10000.0).round() / 10000.0,
                            ));
                        }
                    }
                }
            }
        }
    }
    result
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
        let triangles = pieces(
            &crate::ink::montblanc::tests::captured_control_buffers(&stroke, case),
            8,
        );
        assert_eq!(triangles.len(), if case == 3 { 3700 } else { 2500 });
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
            let [a, b, c] = points;
            let data = Data::new()
                .move_to((a[0], a[1]))
                .line_to((b[0], b[1]))
                .line_to((c[0], c[1]))
                .close();
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
        assert_eq!(
            svg.matches("<path").count(),
            if case == 3 { 3700 } else { 2500 }
        );
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
