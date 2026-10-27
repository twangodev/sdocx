//! Bounded reconstruction of two four-point curved legacy Marker controls.
use super::path::{P, Quad};

type Point = [f32; 2];
#[derive(Default)]
pub(super) struct Mesh {
    pub(super) body: Vec<Point>,
    pub(super) fringe: [Vec<[f32; 3]>; 2],
    pub(super) caps: [Vec<[f32; 4]>; 2],
}
fn add(a: Point, b: Point) -> Point {
    [a[0] + b[0], a[1] + b[1]]
}
fn sub(a: Point, b: Point) -> Point {
    [a[0] - b[0], a[1] - b[1]]
}
fn scale(a: Point, b: f32) -> Point {
    [a[0] * b, a[1] * b]
}
fn mid(a: Point, b: Point) -> Point {
    scale(add(a, b), 0.5)
}
fn normal(a: Point) -> Option<(Point, f32)> {
    let length = a[0].hypot(a[1]);
    (length > 0.000001).then(|| ([a[1] / length, -a[0] / length], length))
}
fn subdivide(
    p: [Point; 3],
    lo: usize,
    hi: usize,
    radius: f32,
    edge: &mut [Vec<Point>; 2],
) -> Option<()> {
    if hi - lo < 2 {
        return Some(());
    }
    let [a, b, c] = p;
    let center = [
        (a[0] * 0.25 + b[0] * 0.5) + c[0] * 0.25,
        (a[1] * 0.25 + b[1] * 0.5) + c[1] * 0.25,
    ];
    let (n, _) = normal(sub(c, a))?;
    let offset = scale(n, radius);
    let m = (lo + hi) / 2;
    edge[0][m] = add(center, offset);
    edge[1][m] = sub(center, offset);
    subdivide([a, mid(a, b), center], lo, m, radius, edge)?;
    subdivide([center, mid(b, c), c], m, hi, radius, edge)
}
fn cap(mesh: &mut Mesh, index: usize, corners: [Point; 4], outward: [Point; 2]) {
    mesh.caps[index] = [
        (0, 0., 0.),
        (2, 1., 0.),
        (1, 0., 1.),
        (2, 1., 0.),
        (3, 1., 1.),
        (1, 0., 1.),
    ]
    .into_iter()
    .map(|(i, u, v)| [corners[i][0], corners[i][1], u, v])
    .collect();
    for side in 0..2 {
        for p in [corners[side], corners[side + 2]] {
            let outer = add(p, outward[side]);
            mesh.fringe[side].extend([[p[0], p[1], 0.], [outer[0], outer[1], 1.]]);
        }
    }
}
pub(super) fn generate(points: [Point; 4], size: f32) -> Option<Mesh> {
    if !size.is_finite() || size <= 1.0 || points.iter().flatten().any(|x| !x.is_finite()) {
        return None;
    }
    let radius = (size - 1.) * 0.5;
    let width = size * 0.5;
    let cap_length = size * 0.4;
    let dirs = [
        sub(points[1], points[0]),
        sub(points[2], points[1]),
        sub(points[3], points[2]),
    ];
    let (n0, l0) = normal(dirs[0])?;
    let (n1, l1) = normal(dirs[1])?;
    let (n2, l2) = normal(dirs[2])?;
    let start = add(points[0], scale(dirs[0], cap_length / l0));
    let end = mid(start, points[1]);
    let middle = mid(points[1], points[2]);
    let q = Quad::new(
        P {
            x: end[0],
            y: end[1],
        },
        P {
            x: points[1][0],
            y: points[1][1],
        },
        P {
            x: middle[0],
            y: middle[1],
        },
    );
    if q.length <= 1. {
        return None;
    }
    let a = q.at(q.length - 1.);
    let curves = [
        [start, mid(start, end), end],
        [end, points[1], middle],
        [[a.x, a.y], points[2], points[3]],
    ];
    let normals = [[n0, n0], [n0, n1], [n1, n2]];
    let lengths = [[0., l0], [l0, l1], [l1, l2]];
    let mut mesh = Mesh::default();
    cap(
        &mut mesh,
        0,
        [
            sub(start, scale(n0, width)),
            add(start, scale(n0, width)),
            sub(points[0], scale(n0, width)),
            add(points[0], scale(n0, width)),
        ],
        [scale(n0, -1.), n0],
    );
    for i in 0..3 {
        let dot = normals[i][0][0] * normals[i][1][0] + normals[i][0][1] * normals[i][1][1];
        let count = ((((lengths[i][0] + lengths[i][1]) * 0.5) as i32 as f32 * (dot - 1.).abs())
            as usize
            + 3)
        .min(20);
        let mut edge = [vec![[0.; 2]; count + 1], vec![[0.; 2]; count + 1]];
        for (j, (point, n)) in [(curves[i][0], normals[i][0]), (curves[i][2], normals[i][1])]
            .into_iter()
            .enumerate()
        {
            let k = if j == 0 { 0 } else { count };
            edge[0][k] = add(point, scale(n, radius));
            edge[1][k] = sub(point, scale(n, radius));
        }
        subdivide(curves[i], 0, count, radius, &mut edge)?;
        for (first, second) in edge[0].iter().zip(&edge[1]) {
            mesh.body.extend([*second, *first]);
            let delta = scale(sub(*first, *second), 1. / (2. * radius));
            let out = [sub(*second, delta), add(*first, delta)];
            for (side, p) in [*second, *first].into_iter().enumerate() {
                mesh.fringe[side].extend([[p[0], p[1], 0.], [out[side][0], out[side][1], 1.]]);
            }
        }
    }
    let tail = add(points[3], scale(dirs[2], cap_length / l2));
    cap(
        &mut mesh,
        1,
        [
            add(points[3], scale(n2, width)),
            sub(points[3], scale(n2, width)),
            add(tail, scale(n2, width)),
            sub(tail, scale(n2, width)),
        ],
        [n2, scale(n2, -1.)],
    );
    Some(mesh)
}

#[test]
fn generated_complete_mesh_matches_synthetic_native_controls() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/legacy-marker-source-geometry-controls.json"
    ))
    .unwrap();
    let decode = |hex: &str| -> Vec<f32> {
        let bytes: Vec<u8> = hex
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| f32::from_le_bytes(*b))
            .collect()
    };
    for case in ["straight", "bend"] {
        let native = &fixture["cases"][case];
        let raw = decode(
            native["input"]["source_arrays_hex"]["points"]
                .as_str()
                .unwrap(),
        );
        let points: [Point; 4] = raw.as_chunks::<2>().0.try_into().unwrap();
        let size = f32::from_bits(
            u32::from_str_radix(native["input"]["size_f32_bits"].as_str().unwrap(), 16).unwrap(),
        );
        let mesh = generate(points, size).unwrap();
        let outputs = &native["mesh_vector_outputs"];
        for (index, generated) in [
            (9, mesh.body.iter().flatten().copied().collect::<Vec<_>>()),
            (13, mesh.fringe[0].iter().flatten().copied().collect()),
            (14, mesh.fringe[1].iter().flatten().copied().collect()),
            (0, mesh.caps[0].iter().flatten().copied().collect()),
            (12, mesh.caps[1].iter().flatten().copied().collect()),
        ] {
            let expected = decode(
                outputs[index]["return_vector"]["data_hex"]
                    .as_str()
                    .unwrap(),
            );
            assert_eq!(generated.len(), expected.len(), "{case} mesh {index}");
            for (i, (actual, expected)) in generated.iter().zip(expected).enumerate() {
                assert!(
                    (actual - expected).abs() < 0.00005,
                    "{case} mesh {index} lane {i}: {actual} != {expected}"
                );
            }
        }
    }
}

#[test]
fn native_exact_reversal_circle_uses_shared_interior_center() {
    let row: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/legacy-marker-reversal-circle-control.json"
    ))
    .unwrap();
    let point = |v: &serde_json::Value| -> Point {
        [v[0].as_f64().unwrap() as f32, v[1].as_f64().unwrap() as f32]
    };
    let c = row["control_points_f32"]
        .as_array()
        .unwrap()
        .iter()
        .map(point)
        .collect::<Vec<_>>();
    let dist = |a: Point, b: Point| {
        let d = sub(a, b);
        d[0].hypot(d[1])
    };
    let ac = dist(c[0], c[2]);
    let first = if dist(c[0], c[1]) > ac { c[1] } else { c[2] };
    let second = if dist(c[2], c[1]) > ac {
        sub(add(c[2], c[1]), first)
    } else {
        c[0]
    };
    let center = mid(first, second);
    let a = row["first_edge"].as_array().unwrap();
    let b = row["second_edge"].as_array().unwrap();
    assert_eq!(a.len(), 21);
    assert_eq!(a.len(), b.len());
    // Endpoint pairs are written by calcBezierPath; makeCircle fills the interior.
    for (a, b) in a[1..a.len() - 1].iter().zip(&b[1..b.len() - 1]) {
        let center_from_edges = mid(point(a), point(b));
        assert!((center_from_edges[0] - center[0]).abs() < 0.00001);
        assert!((center_from_edges[1] - center[1]).abs() < 0.00001);
    }
}

fn centerline_subdivision(points: [Point; 3], lo: usize, hi: usize, samples: &mut [Point]) {
    if hi - lo < 2 {
        return;
    }
    let [a, b, c] = points;
    let center = [
        (a[0] * 0.25 + b[0] * 0.5) + c[0] * 0.25,
        (a[1] * 0.25 + b[1] * 0.5) + c[1] * 0.25,
    ];
    let k = (lo + hi) / 2;
    samples[k] = center;
    centerline_subdivision([a, mid(a, b), center], lo, k, samples);
    centerline_subdivision([center, mid(b, c), c], k, hi, samples);
}

// Only the non-circle and exact-reversal branches of the captured four-point
// controls are modeled. Sector turns and skipped points return None.
fn source_centerline(points: [Point; 4], size: f32) -> Option<Vec<Point>> {
    let dirs = [
        sub(points[1], points[0]),
        sub(points[2], points[1]),
        sub(points[3], points[2]),
    ];
    let (n0, l0) = normal(dirs[0])?;
    let (n1, l1) = normal(dirs[1])?;
    let (n2, l2) = normal(dirs[2])?;
    let start = add(points[0], scale(dirs[0], size * 0.4 / l0));
    let end = mid(start, points[1]);
    let middle = mid(points[1], points[2]);
    let q = Quad::new(
        P {
            x: end[0],
            y: end[1],
        },
        P {
            x: points[1][0],
            y: points[1][1],
        },
        P {
            x: middle[0],
            y: middle[1],
        },
    );
    if q.length <= 1.0 {
        return None;
    }
    let tail = q.at(q.length - 1.0);
    let curves = [
        [start, mid(start, end), end],
        [end, points[1], middle],
        [[tail.x, tail.y], points[2], points[3]],
    ];
    let normals = [[n0, n0], [n0, n1], [n1, n2]];
    let lengths = [[0.0, l0], [l0, l1], [l1, l2]];
    let mut output = Vec::new();
    for i in 0..3 {
        let dot = normals[i][0][0] * normals[i][1][0] + normals[i][0][1] * normals[i][1][1];
        let circle = dot <= -0.7 && normal(sub(curves[i][0], curves[i][1]))?.1 > 0.6;
        let count = if circle {
            if !(-1.004..=-0.996).contains(&dot) {
                return None;
            }
            20
        } else {
            ((((lengths[i][0] + lengths[i][1]) * 0.5) as i32 as f32 * (dot - 1.0).abs()) as usize
                + 3)
            .min(20)
        };
        let mut samples = vec![[0.0; 2]; count + 1];
        samples[0] = curves[i][0];
        samples[count] = curves[i][2];
        if circle {
            let [a, b, c] = curves[i];
            let length = |a, b| normal(sub(a, b)).map_or(0.0, |x| x.1);
            let ac = length(a, c);
            let first = if length(a, b) > ac { b } else { c };
            let second = if length(c, b) > ac {
                sub(add(c, b), first)
            } else {
                a
            };
            samples[1..count].fill(mid(first, second));
        } else {
            centerline_subdivision(curves[i], 0, count, &mut samples);
        }
        output.extend(samples);
    }
    Some(output)
}

#[test]
fn source_centerline_matches_native_straight_bend_and_exact_reversal() {
    let geometry: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/legacy-marker-source-geometry-controls.json"
    ))
    .unwrap();
    let circle: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/legacy-marker-reversal-circle-control.json"
    ))
    .unwrap();
    let trace: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/legacy-marker-subdivision-controls.json"
    ))
    .unwrap();
    for case in ["straight", "bend", "reverse"] {
        let input = if case == "reverse" {
            &trace["circle_controls"][0]["source_arrays_hex"]["points"]
        } else {
            &geometry["cases"][case]["input"]["source_arrays_hex"]["points"]
        };
        let hex = input.as_str().unwrap();
        let bytes: Vec<u8> = hex
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|v| u8::from_str_radix(std::str::from_utf8(v).unwrap(), 16).unwrap())
            .collect();
        let floats: Vec<f32> = bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| f32::from_le_bytes(*b))
            .collect();
        let points: [Point; 4] = floats.as_chunks::<2>().0.try_into().unwrap();
        let predicted = source_centerline(points, 12.0).unwrap();
        let expected = if case == "reverse" {
            let mut pairs = Vec::new();
            for row in circle["mesh_pairs"].as_array().unwrap() {
                let a = row["first"].as_array().unwrap();
                let b = row["second"].as_array().unwrap();
                pairs.extend(a.iter().zip(b).map(|(a, b)| {
                    [
                        (a[0].as_f64().unwrap() as f32 + b[0].as_f64().unwrap() as f32) * 0.5,
                        (a[1].as_f64().unwrap() as f32 + b[1].as_f64().unwrap() as f32) * 0.5,
                    ]
                }));
            }
            pairs
        } else {
            let calls = geometry["cases"][case]["mesh_vector_outputs"]
                .as_array()
                .unwrap();
            let mut pairs = Vec::new();
            for row in calls.iter().filter(|r| {
                r["method"].as_str().unwrap().contains("::mergePoints(")
                    && r["return_vector"]["element_stride"] == 8
            }) {
                let input = &row["geometry_inputs"];
                let a = input[0]["components_f32"].as_array().unwrap();
                let b = input[1]["components_f32"].as_array().unwrap();
                pairs.extend(a.iter().zip(b).map(|(a, b)| {
                    [
                        (a[0].as_f64().unwrap() as f32 + b[0].as_f64().unwrap() as f32) * 0.5,
                        (a[1].as_f64().unwrap() as f32 + b[1].as_f64().unwrap() as f32) * 0.5,
                    ]
                }));
            }
            pairs
        };
        let actual = &predicted[..];
        assert_eq!(actual.len(), expected.len(), "{case}");
        for (a, b) in actual.iter().zip(expected) {
            for j in 0..2 {
                assert!((a[j] - b[j]).abs() < 0.00003, "{case}: {a:?} != {b:?}");
            }
        }
    }
}
