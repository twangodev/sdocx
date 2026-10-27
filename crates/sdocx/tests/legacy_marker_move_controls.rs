use serde_json::Value;

type Point = [f32; 2];

fn add(a: Point, b: Point) -> Point {
    [a[0] + b[0], a[1] + b[1]]
}
fn scale(a: Point, s: f32) -> Point {
    [a[0] * s, a[1] * s]
}
fn midpoint(a: Point, b: Point) -> Point {
    scale(add(a, b), 0.5)
}
fn point(v: &Value) -> Point {
    [v[0].as_f64().unwrap() as f32, v[1].as_f64().unwrap() as f32]
}
fn floats(hex: &str) -> Vec<f32> {
    let bytes: Vec<_> = hex
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|x| u8::from_str_radix(std::str::from_utf8(x).unwrap(), 16).unwrap())
        .collect();
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|x| f32::from_le_bytes(*x))
        .collect()
}
fn near(a: Point, b: Point, case: &str) {
    for i in 0..2 {
        assert!((a[i] - b[i]).abs() < 0.000003, "{case}: {a:?} != {b:?}");
    }
}

#[test]
fn observed_first_and_second_moves_produce_quadratic_controls() {
    let source: Value = serde_json::from_str(include_str!(
        "../../../conformance/legacy-marker-source-geometry-controls.json"
    ))
    .unwrap();
    let traced: Value = serde_json::from_str(include_str!(
        "../../../conformance/legacy-marker-subdivision-controls.json"
    ))
    .unwrap();
    for case in ["straight", "bend"] {
        let raw = floats(
            source["cases"][case]["input"]["source_arrays_hex"]["points"]
                .as_str()
                .unwrap(),
        );
        let p: Vec<Point> = raw.as_chunks::<2>().0.to_vec();
        assert_eq!(p.len(), 4);
        let entries: Vec<_> = traced["cases"][case]["rows"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["method"] == "calcBezierPath")
            .collect();
        assert_eq!(entries.len(), 3);
        let rt = floats(entries[0]["rt_644_716_hex"].as_str().unwrap());
        let cap_length = rt[1];
        let delta = [p[1][0] - p[0][0], p[1][1] - p[0][1]];
        let length = delta[0].hypot(delta[1]);
        assert!(length > 0.000001);
        let start = add(p[0], scale(delta, cap_length / length));
        let end = midpoint(start, p[1]);
        let expected = [
            [start, midpoint(start, end), end],
            [end, p[1], midpoint(p[1], p[2])],
        ];
        for (row, expected) in entries.iter().zip(expected) {
            for (actual, expected) in row["control_points_f32"]
                .as_array()
                .unwrap()
                .iter()
                .zip(expected)
            {
                near(point(actual), expected, case);
            }
        }
    }
}

fn generate_interior(
    points: [Point; 3],
    lo: usize,
    hi: usize,
    radius: f32,
    edges: &mut [Vec<Point>; 2],
) {
    if hi - lo < 2 {
        return;
    }
    let [a, b, c] = points;
    let ab = midpoint(a, b);
    let bc = midpoint(b, c);
    let center = [
        (a[0] * 0.25 + b[0] * 0.5) + c[0] * 0.25,
        (a[1] * 0.25 + b[1] * 0.5) + c[1] * 0.25,
    ];
    let tangent = [c[0] - a[0], c[1] - a[1]];
    let length = tangent[0].hypot(tangent[1]);
    assert!(length > 0.000001);
    let offset = scale([tangent[1] / length, -tangent[0] / length], radius);
    let mid = (lo + hi) / 2;
    edges[0][mid] = add(center, offset);
    edges[1][mid] = add(center, scale(offset, -1.0));
    generate_interior([a, ab, center], lo, mid, radius, edges);
    generate_interior([center, bc, c], mid, hi, radius, edges);
}

#[test]
fn source_points_generate_first_two_native_body_meshes() {
    let source: Value = serde_json::from_str(include_str!(
        "../../../conformance/legacy-marker-source-geometry-controls.json"
    ))
    .unwrap();
    for case in ["straight", "bend"] {
        let raw = floats(
            source["cases"][case]["input"]["source_arrays_hex"]["points"]
                .as_str()
                .unwrap(),
        );
        let p = raw.as_chunks::<2>().0;
        let meshes: Vec<_> = source["cases"][case]["mesh_vector_outputs"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| {
                r["method"].as_str().unwrap().contains("::mergePoints(")
                    && r["return_vector"]["element_stride"] == 8
            })
            .collect();
        let size_bits = u32::from_str_radix(
            source["cases"][case]["input"]["size_f32_bits"]
                .as_str()
                .unwrap(),
            16,
        )
        .unwrap();
        let size = f32::from_bits(size_bits);
        let delta = [p[1][0] - p[0][0], p[1][1] - p[0][1]];
        let length = delta[0].hypot(delta[1]);
        assert!(length > 0.000001);
        let start = add(p[0], scale(delta, (size * 0.4) / length));
        let end = midpoint(start, p[1]);
        let curves = [
            [start, midpoint(start, end), end],
            [end, p[1], midpoint(p[1], p[2])],
        ];
        for (i, curve) in curves.into_iter().enumerate() {
            let radius = (size - 1.0) * 0.5;
            let directions = [
                [p[1][0] - p[0][0], p[1][1] - p[0][1]],
                [p[2][0] - p[1][0], p[2][1] - p[1][1]],
            ];
            let lengths = directions.map(|d| d[0].hypot(d[1]));
            let normals = directions.map(|d| {
                let length = d[0].hypot(d[1]);
                assert!(length > 0.000001);
                [d[1] / length, -d[0] / length]
            });
            let dot = if i == 0 {
                1.0
            } else {
                normals[0][0] * normals[1][0] + normals[0][1] * normals[1][1]
            };
            let previous_length = if i == 0 { 0.0 } else { lengths[0] };
            let segments = ((((lengths[i] + previous_length) * 0.5) as i32 as f32
                * (dot - 1.0).abs()) as usize
                + 3)
            .min(20);
            let input = &meshes[i]["geometry_inputs"];
            let captured: Vec<Vec<Point>> = (0..2)
                .map(|side| {
                    input[side]["components_f32"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(point)
                        .collect()
                })
                .collect();
            let n = segments + 1;
            assert_eq!(n, captured[0].len());
            let mut edges = [vec![[0.0; 2]; n], vec![[0.0; 2]; n]];
            let normals = [normals[0], normals[i]];
            for (j, (center, normal)) in [(curve[0], normals[0]), (curve[2], normals[1])]
                .into_iter()
                .enumerate()
            {
                let index = if j == 0 { 0 } else { n - 1 };
                let offset = scale(normal, radius);
                edges[0][index] = add(center, offset);
                edges[1][index] = add(center, scale(offset, -1.0));
            }
            generate_interior(curve, 0, n - 1, radius, &mut edges);
            for side in 0..2 {
                for (actual, expected) in edges[side].iter().zip(&captured[side]) {
                    for component in 0..2 {
                        assert!(
                            (actual[component] - expected[component]).abs() < 0.00003,
                            "{case} curve {i}: {actual:?} != {expected:?}"
                        );
                    }
                }
            }
        }
    }
}
