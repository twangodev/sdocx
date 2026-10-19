//! Test-only vector transport for the captured Calligraphy mesh drawn through RTV1.
//!
//! This consumes the renderer's already-generated triangles. It does not implement
//! Calligraphy's saved-point geometry or decide when that geometry is admitted.

use super::vector::{
    ColorValue, Data, Definitions, Gradient, Group, LinearGradient, Paint, Path, Scene, Stop,
    Styled,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct NativeVertex {
    pub position: [f32; 2],
    /// Native core-shader interpolation value (the fourth uploaded component).
    pub w: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct NativeTriangle(pub [NativeVertex; 3]);

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct NativeMeshBatch {
    /// Renderer slot 232, drawn first with the core shader.
    pub core_232: Vec<NativeTriangle>,
    /// Renderer slot 240, drawn second with the core shader.
    pub core_240: Vec<NativeTriangle>,
    /// Renderer slot 224, drawn last with the opaque fringe shader.
    pub fringe_224: Vec<NativeTriangle>,
    /// SetPenData RGB. Saved alpha is absent because neither captured fragment
    /// shader uses the uniform alpha component.
    pub rgb: [u8; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Viewport {
    pub width: u32,
    pub height: u32,
    pub origin: [f32; 2],
    pub scale: [f32; 2],
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            width: 64,
            height: 64,
            origin: [32.0, 32.0],
            scale: [3.0, 3.0],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AdapterError {
    InvalidViewport,
    NonFiniteVertex,
    UnrepresentableGradient,
}

/// Preserve each native mesh triangle as one SVG path. Ordinary sequential paint
/// implements source-over in the actual renderer order: 232, 240, then 224.
pub(super) fn render(
    scene: &mut Scene,
    mesh: &NativeMeshBatch,
    viewport: Viewport,
) -> Result<(), AdapterError> {
    if viewport.width == 0
        || viewport.height == 0
        || viewport.scale.iter().any(|v| !v.is_finite() || *v == 0.0)
        || viewport.origin.iter().any(|v| !v.is_finite())
    {
        return Err(AdapterError::InvalidViewport);
    }
    let color = ColorValue::Rgb(crate::Color {
        r: mesh.rgb[0],
        g: mesh.rgb[1],
        b: mesh.rgb[2],
    });
    let mut definitions = Definitions::new();
    let mut core_paths = Vec::new();
    for triangle in mesh.core_232.iter().chain(&mesh.core_240) {
        let (gradient, path) = core_path(scene, triangle, viewport, color)?;
        if let Some(gradient) = gradient {
            definitions = definitions.add(gradient);
        }
        if let Some(path) = path {
            core_paths.push(path);
        }
    }
    let mut fringe_paths = Vec::with_capacity(mesh.fringe_224.len());
    for triangle in &mesh.fringe_224 {
        let points = transformed(triangle, viewport)?;
        if !zero_area(points) {
            fringe_paths.push(
                Path::new()
                    .data(path_data(points))
                    .fill(Paint::Solid(color)),
            );
        }
    }
    scene.scope(definitions, |_| {});
    scene.scope(Group::new().linear_rgb(), |scene| {
        for path in core_paths.into_iter().chain(fringe_paths) {
            scene.push(path);
        }
    });
    Ok(())
}

fn transformed(
    triangle: &NativeTriangle,
    viewport: Viewport,
) -> Result<[[f64; 2]; 3], AdapterError> {
    let mut points = [[0.0; 2]; 3];
    for (dst, vertex) in points.iter_mut().zip(triangle.0) {
        let [x, y] = vertex.position;
        if !x.is_finite() || !y.is_finite() || !vertex.w.is_finite() {
            return Err(AdapterError::NonFiniteVertex);
        }
        *dst = [
            f64::from(viewport.origin[0]) + f64::from(viewport.scale[0]) * f64::from(x),
            f64::from(viewport.origin[1]) + f64::from(viewport.scale[1]) * f64::from(y),
        ];
        if dst.iter().any(|v| !v.is_finite()) {
            return Err(AdapterError::NonFiniteVertex);
        }
    }
    Ok(points)
}

fn path_data(points: [[f64; 2]; 3]) -> Data {
    Data::new()
        .move_to((points[0][0], points[0][1]))
        .line_to((points[1][0], points[1][1]))
        .line_to((points[2][0], points[2][1]))
        .close()
}

fn zero_area(points: [[f64; 2]; 3]) -> bool {
    let area = (points[1][0] - points[0][0]) * (points[2][1] - points[0][1])
        - (points[2][0] - points[0][0]) * (points[1][1] - points[0][1]);
    area == 0.0
}

fn native_alpha(w: f64) -> f64 {
    ((0.5 - (0.5 - w).abs()) / 0.3).clamp(0.0, 1.0)
}

fn core_path(
    scene: &mut Scene,
    triangle: &NativeTriangle,
    viewport: Viewport,
    color: ColorValue,
) -> Result<(Option<LinearGradient>, Option<Path>), AdapterError> {
    let points = transformed(triangle, viewport)?;
    if zero_area(points) {
        return Ok((None, None));
    }
    let data = path_data(points);
    let [w0, w1, w2] = triangle.0.map(|vertex| f64::from(vertex.w));
    if w0 == w1 && w1 == w2 {
        let path = Path::new()
            .data(data)
            .fill(Paint::Solid(color))
            .fill_opacity(native_alpha(w0));
        return Ok((None, Some(path)));
    }

    let det = (points[1][0] - points[0][0]) * (points[2][1] - points[0][1])
        - (points[2][0] - points[0][0]) * (points[1][1] - points[0][1]);
    if !det.is_finite() || det.abs() <= 1e-18 {
        return Err(AdapterError::UnrepresentableGradient);
    }
    let gx = ((w1 - w0) * (points[2][1] - points[0][1])
        - (w2 - w0) * (points[1][1] - points[0][1]))
        / det;
    let gy = ((points[1][0] - points[0][0]) * (w2 - w0)
        - (points[2][0] - points[0][0]) * (w1 - w0))
        / det;
    let norm = gx * gx + gy * gy;
    if !norm.is_finite() || norm <= 1e-30 {
        return Err(AdapterError::UnrepresentableGradient);
    }
    let constant = w0 - gx * points[0][0] - gy * points[0][1];
    let x1 = -constant * gx / norm;
    let y1 = -constant * gy / norm;
    let x2 = (1.0 - constant) * gx / norm;
    let y2 = (1.0 - constant) * gy / norm;
    if [x1, y1, x2, y2].iter().any(|v| !v.is_finite()) {
        return Err(AdapterError::UnrepresentableGradient);
    }

    let definition = scene.definition::<Gradient>();
    let gradient = LinearGradient::new(&definition)
        .x1(x1)
        .y1(y1)
        .x2(x2)
        .y2(y2)
        .add(Stop::new(0.0, Some(color)).opacity(0.0))
        .add(Stop::new(0.3, Some(color)).opacity(1.0))
        .add(Stop::new(0.7, Some(color)).opacity(1.0))
        .add(Stop::new(1.0, Some(color)).opacity(0.0));
    let path = Path::new().data(data).fill(Paint::Gradient(definition));
    Ok((Some(gradient), Some(path)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn tri(vertices: [[f32; 3]; 3]) -> NativeTriangle {
        NativeTriangle(vertices.map(|[x, y, w]| NativeVertex {
            position: [x, y],
            w,
        }))
    }

    fn test_scene(mesh: &NativeMeshBatch) -> String {
        let mut scene = Scene::new(super::super::vector::Svg::new().width(64).height(64));
        render(&mut scene, mesh, Viewport::default()).unwrap();
        scene.finish()
    }

    fn fixture() -> NativeMeshBatch {
        NativeMeshBatch {
            core_232: vec![tri([[0.0, 0.0, 0.1], [2.0, 0.0, 0.5], [0.0, 2.0, 0.9]])],
            core_240: vec![tri([[1.0, 0.0, 0.2], [3.0, 0.0, 0.6], [1.0, 2.0, 1.0]])],
            fringe_224: vec![tri([[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 2.0, 0.0]])],
            rgb: [0x44, 0xaa, 0xee],
        }
    }

    fn captured_mesh(case: &Value) -> NativeMeshBatch {
        let argb = u32::from_str_radix(
            case["input"]["argb"]
                .as_str()
                .unwrap()
                .trim_start_matches("0x"),
            16,
        )
        .unwrap();
        let rgb = [(argb >> 16) as u8, (argb >> 8) as u8, argb as u8];
        let mut mesh = NativeMeshBatch {
            rgb,
            ..Default::default()
        };
        let uploads = case["queued_update"]["uploads"].as_array().unwrap();
        for upload in uploads {
            assert_eq!(upload["vertex_format"], "Vector4<f32> little-endian");
            let slot = upload["source_vector_slot"].as_u64().unwrap();
            let bytes = decode_hex(upload["bytes_hex"].as_str().unwrap());
            assert_eq!(bytes.len(), upload["byte_count"].as_u64().unwrap() as usize);
            assert_eq!(
                bytes.len(),
                upload["vertex_count"].as_u64().unwrap() as usize * 16
            );
            let floats = bytes
                .chunks_exact(4)
                .map(|bytes| f32::from_le_bytes(bytes.try_into().unwrap()))
                .collect::<Vec<_>>();
            assert!(floats.len().is_multiple_of(12));
            let triangles = floats
                .chunks_exact(12)
                .map(|vertices| {
                    NativeTriangle(std::array::from_fn(|index| NativeVertex {
                        position: [vertices[index * 4], vertices[index * 4 + 1]],
                        w: vertices[index * 4 + 3],
                    }))
                })
                .collect::<Vec<_>>();
            match slot {
                232 => mesh.core_232 = triangles,
                240 => mesh.core_240 = triangles,
                224 => mesh.fringe_224 = triangles,
                other => panic!("unexpected native source vector slot {other}"),
            }
        }
        assert!(!mesh.core_232.is_empty());
        assert!(!mesh.core_240.is_empty());
        assert!(!mesh.fringe_224.is_empty());
        mesh
    }

    fn decode_hex(text: &str) -> Vec<u8> {
        assert!(text.len().is_multiple_of(2));
        text.as_bytes()
            .chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect()
    }

    fn attribute<'a>(tag: &'a str, name: &str) -> &'a str {
        let needle = format!("{name}=\"");
        let value = tag.split_once(&needle).unwrap().1;
        value.split_once('"').unwrap().0
    }

    fn tags<'a>(svg: &'a str, element: &str) -> Vec<&'a str> {
        let prefix = format!("<{element} ");
        svg.split(&prefix)
            .skip(1)
            .map(|tail| tail.split_once('>').unwrap().0)
            .collect()
    }

    fn captured_fixture() -> Value {
        serde_json::from_str(include_str!(
            "../../../../conformance/calligraphy-draw-native.json"
        ))
        .unwrap()
    }

    #[test]
    fn affine_core_mesh_keeps_source_triangles_and_native_slot_order() {
        let svg = test_scene(&fixture());
        let gradients = svg
            .match_indices("<linearGradient")
            .map(|x| x.0)
            .collect::<Vec<_>>();
        let paths = svg.match_indices("<path").map(|x| x.0).collect::<Vec<_>>();
        assert_eq!(gradients.len(), 2);
        assert_eq!(paths.len(), 3);
        assert!(gradients[0] < gradients[1] && gradients[1] < paths[0]);
        assert!(svg.contains("stop-opacity=\"0\""));
        assert!(svg.contains("offset=\"0.3\""));
        assert!(svg.contains("offset=\"0.7\""));
        assert!(svg.contains("color-interpolation=\"linearRGB\""));
        assert!(!svg.contains("mix-blend-mode"));
    }

    #[test]
    fn committed_native_uploads_retain_vertex_lanes_slots_and_geometry_groups() {
        let fixture = captured_fixture();
        for case in fixture["cases"].as_array().unwrap() {
            let mesh = captured_mesh(case);
            let argb = u32::from_str_radix(
                case["input"]["argb"]
                    .as_str()
                    .unwrap()
                    .trim_start_matches("0x"),
                16,
            )
            .unwrap();
            assert_eq!(
                mesh.rgb,
                [(argb >> 16) as u8, (argb >> 8) as u8, argb as u8]
            );
            let native_core_count = case["queued_update"]["uploads"]
                .as_array()
                .unwrap()
                .iter()
                .find(|upload| upload["source_vector_slot"] == 232)
                .unwrap()["vertex_count"]
                .as_u64()
                .unwrap() as usize;
            assert_eq!(mesh.core_232.len() * 3, native_core_count);
            let svg = test_scene(&mesh);
            let expected_triangles = mesh
                .core_232
                .iter()
                .chain(&mesh.core_240)
                .chain(&mesh.fringe_224)
                .filter(|triangle| {
                    transformed(triangle, Viewport::default())
                        .is_ok_and(|points| !zero_area(points))
                })
                .collect::<Vec<_>>();
            let active_core_count = mesh
                .core_232
                .iter()
                .chain(&mesh.core_240)
                .filter(|triangle| {
                    transformed(triangle, Viewport::default())
                        .is_ok_and(|points| !zero_area(points))
                })
                .count();
            let paths = tags(&svg, "path");
            assert_eq!(paths.len(), expected_triangles.len());
            for (tag, triangle) in paths.iter().zip(expected_triangles.iter()) {
                let data = attribute(tag, "d");
                let values = data
                    .split(['M', 'L', 'Z', 'z', ' ', ','])
                    .filter(|part| !part.is_empty())
                    .map(|part| part.parse::<f64>().unwrap())
                    .collect::<Vec<_>>();
                let points = transformed(triangle, Viewport::default()).unwrap();
                let expected = points.into_iter().flatten().collect::<Vec<_>>();
                assert_eq!(values.len(), 6);
                for (actual, expected) in values.into_iter().zip(expected) {
                    assert!((actual - expected).abs() < 1e-5);
                }
            }

            // For every captured affine core triangle, the generated 0/1 ramp
            // reconstructs the native w lane at all three source vertices.
            let gradients = tags(&svg, "linearGradient");
            let gradient_by_id = gradients
                .iter()
                .map(|tag| (attribute(tag, "id"), *tag))
                .collect::<std::collections::HashMap<_, _>>();
            assert!(svg.contains("offset=\"0\""));
            assert!(svg.contains("offset=\"0.3\""));
            assert!(svg.contains("offset=\"0.7\""));
            assert!(svg.contains("offset=\"1\""));
            assert!(svg.contains("stop-opacity=\"0\""));
            assert!(svg.contains("stop-opacity=\"1\""));
            for (path, triangle) in paths
                .iter()
                .zip(expected_triangles.iter())
                .take(active_core_count)
            {
                let fill = attribute(path, "fill");
                let Some(id) = fill
                    .strip_prefix("url(#")
                    .and_then(|fill| fill.strip_suffix(')'))
                else {
                    // Constant-w source triangles are represented by opacity.
                    continue;
                };
                let gradient = gradient_by_id[id];
                let start = [
                    attribute(gradient, "x1").parse::<f64>().unwrap(),
                    attribute(gradient, "y1").parse::<f64>().unwrap(),
                ];
                let end = [
                    attribute(gradient, "x2").parse::<f64>().unwrap(),
                    attribute(gradient, "y2").parse::<f64>().unwrap(),
                ];
                let delta = [end[0] - start[0], end[1] - start[1]];
                let norm = delta[0] * delta[0] + delta[1] * delta[1];
                for vertex in triangle.0 {
                    let point = [
                        f64::from(Viewport::default().origin[0])
                            + f64::from(Viewport::default().scale[0])
                                * f64::from(vertex.position[0]),
                        f64::from(Viewport::default().origin[1])
                            + f64::from(Viewport::default().scale[1])
                                * f64::from(vertex.position[1]),
                    ];
                    let t = ((point[0] - start[0]) * delta[0] + (point[1] - start[1]) * delta[1])
                        / norm;
                    assert!(
                        (t - f64::from(vertex.w)).abs() < 2e-5,
                        "gradient {id} produced {t} for native w {}",
                        vertex.w
                    );
                }
                assert!(gradient.contains("gradientUnits=\"userSpaceOnUse\""));
                assert_eq!(native_alpha(0.3), 1.0);
                assert_eq!(native_alpha(0.7), 1.0);
            }
        }
    }

    #[test]
    fn saved_object_sampler_addpoint_and_mesh_builder_feed_native_svg_transport() {
        let lanes = crate::ink::calligraphy_saved_native_mesh_for_test()
            .expect("saved channels and AddPoint lifecycle match native geometry capture");
        let fixture = captured_fixture();
        let case = fixture["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|case| case["input"]["saved_wrapper_case"] == "three-samples")
            .unwrap();
        let uploads = case["queued_update"]["uploads"].as_array().unwrap();
        for (index, upload) in uploads.iter().enumerate() {
            let actual: Vec<u8> = lanes[index]
                .iter()
                .flat_map(|vertex| vertex.iter().flat_map(|lane| lane.to_le_bytes()))
                .collect();
            let expected = decode_hex(upload["bytes_hex"].as_str().unwrap());
            if actual != expected {
                let mismatch = actual
                    .chunks_exact(4)
                    .zip(expected.chunks_exact(4))
                    .position(|(left, right)| left != right)
                    .unwrap_or(actual.len().min(expected.len()) / 4);
                panic!(
                    "saved ObjectStroke -> RTV1 slot {} first differing float lane {mismatch}: {:?} vs {:?}; lengths {} vs {}",
                    upload["source_vector_slot"],
                    actual.get(mismatch * 4..mismatch * 4 + 4),
                    expected.get(mismatch * 4..mismatch * 4 + 4),
                    actual.len(),
                    expected.len()
                );
            }
        }

        let triangles = |vertices: &[[f32; 4]]| {
            assert!(vertices.len().is_multiple_of(3));
            vertices
                .chunks_exact(3)
                .map(|triangle| {
                    NativeTriangle(std::array::from_fn(|index| NativeVertex {
                        position: [triangle[index][0], triangle[index][1]],
                        w: triangle[index][3],
                    }))
                })
                .collect::<Vec<_>>()
        };
        let mesh = NativeMeshBatch {
            fringe_224: triangles(&lanes[0]),
            core_232: triangles(&lanes[1]),
            core_240: triangles(&lanes[2]),
            rgb: [0x11, 0x22, 0x33],
        };
        let svg = test_scene(&mesh);
        let emitted_paths = tags(&svg, "path").len();
        let source_triangles = mesh.core_232.len() + mesh.core_240.len() + mesh.fringe_224.len();
        assert!(emitted_paths > 0 && emitted_paths <= source_triangles);
    }

    #[test]
    fn native_piecewise_alpha_and_rgb_do_not_apply_saved_argb_alpha() {
        for (w, alpha) in [
            (0.0, 0.0),
            (0.15, 0.5),
            (0.3, 1.0),
            (0.5, 1.0),
            (0.7, 1.0),
            (0.85, 0.5),
            (1.0, 0.0),
        ] {
            assert!((native_alpha(w) - alpha).abs() < 1e-15);
        }
        let mut mesh = fixture();
        mesh.rgb = [0x11, 0x22, 0x33];
        let svg = test_scene(&mesh);
        assert!(svg.contains("#112233"));
        assert!(!svg.contains("#80112233"));
    }

    #[test]
    fn degenerate_mesh_triangles_have_no_emitted_area() {
        let mesh = NativeMeshBatch {
            core_232: vec![tri([[1.0, 1.0, 0.15], [1.0, 1.0, 0.15], [1.0, 1.0, 0.15]])],
            rgb: [1, 2, 3],
            ..Default::default()
        };
        let svg = test_scene(&mesh);
        assert!(!svg.contains("<path"));
    }

    #[test]
    fn constant_and_out_of_range_w_use_native_alpha_endpoints_and_keep_fringe_last() {
        let mesh = NativeMeshBatch {
            core_232: vec![tri([
                [0.0, 0.0, -0.25],
                [2.0, 0.0, -0.25],
                [0.0, 2.0, -0.25],
            ])],
            core_240: vec![
                tri([[0.0, 0.0, 0.5], [2.0, 0.0, 0.5], [0.0, 2.0, 0.5]]),
                tri([[0.0, 0.0, -0.2], [2.0, 0.0, 0.5], [0.0, 2.0, 1.2]]),
            ],
            fringe_224: vec![tri([[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 2.0, 0.0]])],
            rgb: [0x11, 0x22, 0x33],
        };
        let svg = test_scene(&mesh);
        assert_eq!(svg.matches("<path").count(), 4);
        let paths = tags(&svg, "path");
        assert!(paths[0].contains("fill-opacity=\"0\""));
        assert!(paths[1].contains("fill-opacity=\"1\""));
        assert!(paths[2].contains("fill=\"url(#"));
        assert!(paths[3].contains("fill=\"#112233\""));
        assert!(!paths[3].contains("fill-opacity"));
        assert_eq!(native_alpha(-0.25), 0.0);
        assert_eq!(native_alpha(1.25), 0.0);
    }

    #[test]
    fn invalid_native_triangle_fails_without_emitting_a_partial_batch() {
        let mesh = NativeMeshBatch {
            core_232: vec![tri([[0.0, 0.0, 0.1], [2.0, 0.0, 0.5], [0.0, 2.0, 0.9]])],
            core_240: vec![tri([
                [f32::NAN, 0.0, 0.2],
                [3.0, 0.0, 0.6],
                [1.0, 2.0, 1.0],
            ])],
            rgb: [1, 2, 3],
            ..Default::default()
        };
        let mut scene = Scene::new(super::super::vector::Svg::new().width(64).height(64));
        assert_eq!(
            render(&mut scene, &mesh, Viewport::default()),
            Err(AdapterError::NonFiniteVertex)
        );
        let svg = scene.finish();
        assert!(!svg.contains("<defs"));
        assert!(!svg.contains("<path"));
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn sdk_pdf_keeps_captured_affine_mesh_as_vector_shadings_without_images() {
        let captured = captured_fixture();
        let mesh = captured_mesh(&captured["cases"][1]);
        let svg = test_scene(&mesh);
        let page = crate::RenderedPage::new(0, 64, 64, svg.clone());
        let bytes = crate::render_svg_pages_pdf(&[page], &crate::PdfOptions::default()).unwrap();
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        let images = pdf
            .objects
            .values()
            .filter_map(|object| object.as_dict().ok())
            .filter(|dict| {
                dict.get(b"Subtype")
                    .and_then(lopdf::Object::as_name)
                    .is_ok_and(|name| name == b"Image")
            })
            .count();
        assert_eq!(images, 0);
        assert!(pdf.objects.values().any(|object| {
            object
                .as_dict()
                .ok()
                .and_then(|dict| dict.get(b"Shading").ok())
                .is_some()
        }));
    }
}
