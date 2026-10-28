//! Bounded saved Marker centerline approximation.
use super::path::{P, Quad};

type Point = [f32; 2];
#[cfg(any(test, feature = "pdf"))]
#[derive(Default)]
pub(crate) struct Mesh {
    pub(crate) body: Vec<Point>,
    pub(crate) fringe: [Vec<[f32; 3]>; 2],
    pub(crate) caps: [Vec<[f32; 4]>; 2],
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
#[cfg(any(test, feature = "pdf"))]
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
#[cfg(any(test, feature = "pdf"))]
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
#[cfg(test)]
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
    for i in 0..curves.len() {
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

// Monotonic cubic and tangent-angle reconstruction for one in-range root.
// Other native roots are unsupported and use the fallback.
fn sector_centerline(points: [Point; 3], count: usize) -> Option<Vec<Point>> {
    let [a, b, c] = points.map(|p| [f64::from(p[0]), f64::from(p[1])]);
    let d = [b[0] - a[0], b[1] - a[1]];
    let q = [c[0] - 2.0 * b[0] + a[0], c[1] - 2.0 * b[1] + a[1]];
    let dot = d[0] * q[0] + d[1] * q[1];
    let norm = d[0] * d[0] + d[1] * d[1];
    let aa = q[0] * q[0] + q[1] * q[1];
    let bb = 3.0 * dot;
    let cc = 2.0 * norm - dot;
    let dd = -norm;
    if aa <= 0.0 || bb * bb - 3.0 * aa * cc >= 0.0 {
        return None;
    }
    let polynomial = |t: f64| ((aa * t + bb) * t + cc) * t + dd;
    if polynomial(0.0) > 0.0 || polynomial(1.0) < 0.0 {
        return None;
    }
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..64 {
        let middle = (lo + hi) * 0.5;
        if polynomial(middle) < 0.0 {
            lo = middle;
        } else {
            hi = middle;
        }
    }
    let split = (lo + hi) * 0.5;
    let direction = [d[0] + split * q[0], d[1] + split * q[1]];
    let base = direction[1].atan2(direction[0]);
    let cross = |x: [f64; 2], y: [f64; 2]| x[0] * y[1] - x[1] * y[0];
    let sign = cross(d, q).signum();
    if sign == 0.0 {
        return None;
    }
    let mut samples = vec![[0.0; 2]; count + 1];
    samples[0] = points[0];
    samples[count] = points[2];
    for (index, sample) in samples.iter_mut().enumerate().take(count).skip(1) {
        let angle = base + (index as f64 - (count / 2) as f64) * sign * std::f64::consts::PI / 20.0;
        let axis = [angle.cos(), angle.sin()];
        let denominator = cross(q, axis);
        if denominator.abs() < 1e-12 {
            return None;
        }
        let t = -cross(d, axis) / denominator;
        if !(0.0..=1.0).contains(&t) {
            return None;
        }
        for j in 0..2 {
            sample[j] =
                ((1.0 - t) * (1.0 - t) * a[j] + 2.0 * (1.0 - t) * t * b[j] + t * t * c[j]) as f32;
        }
    }
    Some(samples)
}

// The observed GLV1 move filter and RT curve state for explicit screen dimensions.
// Degenerate event sequences use the fallback.
fn source_centerline(
    points: &[Point],
    size: f32,
    pen_data_dimensions: [u32; 2],
) -> Option<Vec<Point>> {
    if points.len() < 3 {
        return None;
    }
    if !size.is_finite() {
        return None;
    }
    let threshold = size.clamp(0.4, 800.0) * 0.4;
    let cap_length = size.clamp(0.4, 800.0).trunc() * 0.4;
    let mut moves = Vec::new();
    let mut last = points[0];
    for &point in &points[1..points.len() - 1] {
        let delta = sub(point, last);
        // This models the observed bool=false call path; dimensions come from
        // PenData and must not be inferred from an arbitrary export viewport.
        let short_side = pen_data_dimensions[0].min(pen_data_dimensions[1]);
        let display_factor = if short_side == 0 {
            1.0
        } else {
            short_side as f32 / 360.0
        };
        let move_threshold = if moves.is_empty() {
            threshold
        } else {
            1.5 * display_factor
        };
        if delta[0].abs() > move_threshold || delta[1].abs() > move_threshold {
            moves.push(point);
            last = point;
        }
    }
    let first = *moves.first()?;
    let first_delta = sub(first, points[0]);
    let (mut previous_normal, mut previous_length) = normal(first_delta)?;
    let start = add(points[0], scale(first_delta, cap_length / previous_length));
    let mut curve_end = mid(start, first);
    let mut curves = vec![[start, mid(start, curve_end), curve_end]];
    let mut normals = vec![[previous_normal, previous_normal]];
    let mut lengths = vec![[0.0, previous_length]];
    let mut current = first;
    let mut path = [points[0], points[0], curve_end];
    for &next in moves.iter().skip(1) {
        let (next_normal, next_length) = normal(sub(next, current))?;
        let next_end = mid(current, next);
        path = [curve_end, current, next_end];
        curves.push(path);
        normals.push([previous_normal, next_normal]);
        lengths.push([previous_length, next_length]);
        previous_normal = next_normal;
        previous_length = next_length;
        current = next;
        curve_end = next_end;
    }
    let q = Quad::new(
        P {
            x: path[0][0],
            y: path[0][1],
        },
        P {
            x: path[1][0],
            y: path[1][1],
        },
        P {
            x: path[2][0],
            y: path[2][1],
        },
    );
    if q.length <= 1.0 {
        return None;
    }
    let tail = q.at(q.length - 1.0);
    let final_point = *points.last()?;
    let tail_point = [tail.x, tail.y];
    let (end_normal, end_length, end_control) = if final_point == current {
        if points.len() < 4 {
            return None;
        }
        // EndPen retains the previous normal for a zero-length final movement
        // and uses the midpoint of its sampled tail and endpoint as control.
        (previous_normal, 0.0, mid(tail_point, final_point))
    } else {
        let (normal, length) = normal(sub(final_point, current))?;
        (normal, length, current)
    };
    curves.push([tail_point, end_control, final_point]);
    normals.push([previous_normal, end_normal]);
    lengths.push([previous_length, end_length]);
    let mut output = Vec::new();
    for i in 0..curves.len() {
        let dot = normals[i][0][0] * normals[i][1][0] + normals[i][0][1] * normals[i][1][1];
        if !dot.is_finite() || !(-1.004..=1.004).contains(&dot) {
            return None;
        }
        let circle = dot <= -0.7 && normal(sub(curves[i][0], curves[i][1]))?.1 > 0.6;
        let exact_reversal = circle && (-1.004..=-0.996).contains(&dot);
        let count = if exact_reversal {
            20
        } else if circle {
            (((dot.acos().to_degrees() / 9.0 * 0.5) as usize) * 2).min(20)
        } else {
            ((((lengths[i][0] + lengths[i][1]) * 0.5) as i32 as f32 * (dot - 1.0).abs()) as usize
                + 3)
            .min(20)
        };
        let mut samples = vec![[0.0; 2]; count + 1];
        samples[0] = curves[i][0];
        samples[count] = curves[i][2];
        if exact_reversal {
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
        } else if circle {
            samples = sector_centerline(curves[i], count)?;
        } else {
            centerline_subdivision(curves[i], 0, count, &mut samples);
        }
        output.extend(samples);
    }
    Some(output)
}

#[test]
fn source_centerline_matches_native_controls_including_skipped_move() {
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
    let sector: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/legacy-marker-sector-center-control.json"
    ))
    .unwrap();
    let skipped: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/legacy-marker-skipped-center-control.json"
    ))
    .unwrap();
    let long: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/legacy-marker-long-native-controls.json"
    ))
    .unwrap();
    for case in [
        "straight",
        "bend",
        "reverse",
        "turn",
        "mirrored",
        "skipped",
        "repeat",
        "long-skipped",
        "long-turns",
    ] {
        let input = if case == "reverse" {
            &trace["circle_controls"][0]["source_arrays_hex"]["points"]
        } else if case == "turn" {
            &sector["input"]["source_arrays_hex"]["points"]
        } else if case == "mirrored" {
            &sector["mirrored"]["input"]["source_arrays_hex"]["points"]
        } else if case == "skipped" {
            &skipped["input"]["source_arrays_hex"]["points"]
        } else if case == "repeat" {
            &skipped["repeat_control"]["input"]["source_arrays_hex"]["points"]
        } else if case.starts_with("long-") {
            &long["cases"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["name"] == case)
                .unwrap()["input"]["source_arrays_hex"]["points"]
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
        let points = floats.as_chunks::<2>().0;
        let predicted = source_centerline(points, 12.0, [1920, 1080]).unwrap();
        let expected = if case.starts_with("long-") {
            let row = long["cases"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["name"] == case)
                .unwrap()["native_mesh_vector_outputs"]
                .as_array()
                .unwrap()
                .iter()
                .rev()
                .find(|r| {
                    r["method"].as_str().unwrap().contains("::mergePoints(") && r["stride"] == 8
                })
                .unwrap();
            let hex = row["after_hex"].as_str().unwrap();
            let bytes: Vec<u8> = hex
                .as_bytes()
                .as_chunks::<2>()
                .0
                .iter()
                .map(|v| u8::from_str_radix(std::str::from_utf8(v).unwrap(), 16).unwrap())
                .collect();
            let values: Vec<f32> = bytes
                .as_chunks::<4>()
                .0
                .iter()
                .map(|b| f32::from_le_bytes(*b))
                .collect();
            values
                .as_chunks::<4>()
                .0
                .iter()
                .map(|p| [(p[0] + p[2]) * 0.5, (p[1] + p[3]) * 0.5])
                .collect::<Vec<_>>()
        } else if matches!(case, "reverse" | "turn" | "mirrored" | "skipped" | "repeat") {
            let mut pairs = Vec::new();
            let receipt = match case {
                "reverse" => &circle,
                "turn" => &sector,
                "mirrored" => &sector["mirrored"],
                _ => &skipped,
            };
            for row in receipt["mesh_pairs"].as_array().unwrap() {
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
            let reference_case = if case == "long-skipped" { "bend" } else { case };
            let calls = geometry["cases"][reference_case]["mesh_vector_outputs"]
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
                assert!((a[j] - b[j]).abs() < 0.00005, "{case}: {a:?} != {b:?}");
            }
        }
    }
}

#[test]
fn sector_dividing_parameter_matches_native_cubic_entry_and_return() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/legacy-marker-sector-center-control.json"
    ))
    .unwrap();
    let row = &fixture["dividing_calls"][0];
    assert_eq!(row["bool_arg"], false);
    let point = |i: usize| -> [f32; 2] {
        [
            row["control_points_f32"][i][0].as_f64().unwrap() as f32,
            row["control_points_f32"][i][1].as_f64().unwrap() as f32,
        ]
    };
    let [a, b, c] = [point(0), point(1), point(2)];
    let d = [f64::from(b[0] - a[0]), f64::from(b[1] - a[1])];
    let q = [f64::from(c[0] - b[0]) - d[0], f64::from(c[1] - b[1]) - d[1]];
    let dot = d[0] * q[0] + d[1] * q[1];
    let norm = d[0] * d[0] + d[1] * d[1];
    let coeff = [
        q[0] * q[0] + q[1] * q[1],
        3.0 * dot,
        2.0 * norm - dot,
        -norm,
    ];
    let native = &fixture["cubic_calls"][0]["d0_d3_bits"];
    for i in 0..4 {
        let bits = u64::from_str_radix(native[i].as_str().unwrap(), 16).unwrap();
        assert!((coeff[i] - f64::from_bits(bits)).abs() < 0.00001);
    }
    let f = |t: f64| ((coeff[0] * t + coeff[1]) * t + coeff[2]) * t + coeff[3];
    assert!(coeff[1] * coeff[1] - 3.0 * coeff[0] * coeff[2] < 0.0);
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..64 {
        let mid = (lo + hi) * 0.5;
        if f(mid) < 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let bits = u32::from_str_radix(row["return_s0_bits"].as_str().unwrap(), 16).unwrap();
    assert!(((lo + hi) * 0.5 - f64::from(f32::from_bits(bits))).abs() < 0.000001);
}

/// Approximate a canonical-page Marker using caller-supplied native PenData dimensions.
pub(crate) fn prepare(stroke: &crate::Stroke, screen: [u32; 2]) -> Option<Vec<crate::Point>> {
    if screen.contains(&0)
        || screen.iter().any(|&x| x > i32::MAX as u32)
        || stroke.points.len() < 3
        || stroke.points.len() > 16_384
        || stroke.points.iter().any(|p| {
            !p.x.is_finite()
                || !p.y.is_finite()
                || p.x.abs() > 10_000_000.0
                || p.y.abs() > 10_000_000.0
        })
        || super::legacy_marker_profile_argb(stroke).is_none()
    {
        return None;
    }
    let r = stroke.rendering.as_ref()?;
    if r.metadata
        .as_ref()
        .and_then(|m| m.rotation_degrees)
        .is_some_and(|degrees| degrees != 0.0)
    {
        return None;
    }
    let size = r.style.pen_size?;
    if !size.is_finite() || size.clamp(0.4, 800.0).trunc() < 2.0 {
        return None;
    }
    let points: Vec<Point> = stroke
        .points
        .iter()
        .map(|p| [p.x as f32, p.y as f32])
        .collect();
    let centers = source_centerline(&points, size, screen)?;
    if centers
        .iter()
        .flatten()
        .any(|v| !v.is_finite() || v.abs() > 10_000_000.0)
    {
        return None;
    }
    let mut result = Vec::with_capacity(centers.len() + 2);
    result.push(stroke.points[0]);
    result.extend(centers.into_iter().map(|p| crate::Point {
        x: f64::from(p[0]),
        y: f64::from(p[1]),
    }));
    result.push(*stroke.points.last()?);
    Some(result)
}

#[test]
fn repeated_final_centerline_matches_native_finished_stroke() {
    let receipt: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/legacy-marker-finished-prefix-native.json"
    ))
    .unwrap();
    let bend = receipt["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["name"] == "repeat-final")
        .unwrap();
    for (points, case) in [
        (
            [[10.0, 20.0], [10.0, 28.0], [18.0, 28.0], [18.0, 28.0]],
            bend,
        ),
        (
            [[10.0, 20.0], [18.0, 20.0], [26.0, 20.0], [26.0, 20.0]],
            &receipt["repeat_straight_control"],
        ),
    ] {
        let predicted = source_centerline(&points, 12.0, [1920, 1080]).unwrap();
        let expected: Vec<[f32; 2]> = case["native_centerlines"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|curve| curve.as_array().unwrap())
            .map(|p| [p[0].as_f64().unwrap() as f32, p[1].as_f64().unwrap() as f32])
            .collect();
        assert_eq!(predicted.len(), expected.len());
        for (a, b) in predicted.iter().zip(expected) {
            assert!(
                (a[0] - b[0]).abs() < 0.00005 && (a[1] - b[1]).abs() < 0.00005,
                "{a:?} vs {b:?}"
            );
        }
    }
}

#[cfg(feature = "pdf")]
mod native_vector {
    use super::*;
    pub(crate) fn generate_bounded_mesh(
        points: &[Point],
        size: f32,
        pen_data_dimensions: [u32; 2],
    ) -> Option<Mesh> {
        if points.len() < 3
            || points.len() > 256
            || !size.is_finite()
            || size <= 1.0
            || size > 800.0
            || pen_data_dimensions.contains(&0)
            || points
                .iter()
                .flatten()
                .any(|v| !v.is_finite() || v.abs() > 1_000_000.0)
        {
            return None;
        }
        let threshold = size.clamp(0.4, 800.0) * 0.4;
        let cap_length = size.clamp(0.4, 800.0).trunc() * 0.4;
        let mut moves = Vec::new();
        let mut last = points[0];
        for &point in &points[1..points.len() - 1] {
            let delta = sub(point, last);
            // This models the observed bool=false call path; dimensions come from
            // PenData and must not be inferred from an arbitrary export viewport.
            let short_side = pen_data_dimensions[0].min(pen_data_dimensions[1]);
            let display_factor = if short_side == 0 {
                1.0
            } else {
                short_side as f32 / 360.0
            };
            let move_threshold = if moves.is_empty() {
                threshold
            } else {
                1.5 * display_factor
            };
            if delta[0].abs() > move_threshold || delta[1].abs() > move_threshold {
                moves.push(point);
                last = point;
            }
        }
        let first = *moves.first()?;
        let first_delta = sub(first, points[0]);
        let (mut previous_normal, mut previous_length) = normal(first_delta)?;
        let start = add(points[0], scale(first_delta, cap_length / previous_length));
        let mut curve_end = mid(start, first);
        let mut curves = vec![[start, mid(start, curve_end), curve_end]];
        let mut normals = vec![[previous_normal, previous_normal]];
        let mut lengths = vec![[0.0, previous_length]];
        let mut current = first;
        let mut path = [points[0], points[0], curve_end];
        for &next in moves.iter().skip(1) {
            let (next_normal, next_length) = normal(sub(next, current))?;
            let next_end = mid(current, next);
            path = [curve_end, current, next_end];
            curves.push(path);
            normals.push([previous_normal, next_normal]);
            lengths.push([previous_length, next_length]);
            previous_normal = next_normal;
            previous_length = next_length;
            current = next;
            curve_end = next_end;
        }
        let q = Quad::new(
            P {
                x: path[0][0],
                y: path[0][1],
            },
            P {
                x: path[1][0],
                y: path[1][1],
            },
            P {
                x: path[2][0],
                y: path[2][1],
            },
        );
        if q.length <= 1.0 {
            return None;
        }
        let tail = q.at(q.length - 1.0);
        let final_point = *points.last()?;
        let tail_point = [tail.x, tail.y];
        let (end_normal, end_length, end_control) = if final_point == current {
            if points.len() < 4 {
                return None;
            }
            // EndPen retains the previous normal for a zero-length final movement
            // and uses the midpoint of its sampled tail and endpoint as control.
            (previous_normal, 0.0, mid(tail_point, final_point))
        } else {
            let (normal, length) = normal(sub(final_point, current))?;
            (normal, length, current)
        };
        curves.push([tail_point, end_control, final_point]);
        normals.push([previous_normal, end_normal]);
        lengths.push([previous_length, end_length]);
        let radius = (size - 1.) * 0.5;
        let width = size * 0.5;
        let mut mesh = Mesh::default();
        let n0 = normals[0][0];
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
        for i in 0..curves.len() {
            let dot = normals[i][0][0] * normals[i][1][0] + normals[i][0][1] * normals[i][1][1];
            let circle = dot <= -0.7 && normal(sub(curves[i][0], curves[i][1]))?.1 > 0.6;
            if circle {
                return None;
            }
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
        if end_length <= 0.0 {
            return None;
        }
        let tail = add(
            final_point,
            scale(sub(final_point, current), cap_length / end_length),
        );
        cap(
            &mut mesh,
            1,
            [
                add(final_point, scale(end_normal, width)),
                sub(final_point, scale(end_normal, width)),
                add(tail, scale(end_normal, width)),
                sub(tail, scale(end_normal, width)),
            ],
            [end_normal, scale(end_normal, -1.)],
        );
        Some(mesh)
    }
}
#[cfg(feature = "pdf")]
pub(crate) use native_vector::generate_bounded_mesh;

#[cfg(all(test, feature = "pdf"))]
#[test]
fn bounded_vector_mesh_matches_long_native_streams() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/legacy-marker-long-native-controls.json"
    ))
    .unwrap();
    let decode = |hex: &str| -> Vec<f32> {
        let bytes = hex
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
            .collect::<Vec<_>>();
        bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| f32::from_le_bytes(*p))
            .collect()
    };
    let control = &fixture["cases"][1];
    let raw = decode(
        control["input"]["source_arrays_hex"]["points"]
            .as_str()
            .unwrap(),
    );
    let points = raw
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| [p[0], p[1]])
        .collect::<Vec<_>>();
    let mesh = generate_bounded_mesh(&points, 12., [1920, 1080]).unwrap();
    let rows = &control["native_mesh_vector_outputs"];
    for (index, actual) in [
        (15, mesh.body.iter().flatten().copied().collect::<Vec<_>>()),
        (19, mesh.fringe[0].iter().flatten().copied().collect()),
        (20, mesh.fringe[1].iter().flatten().copied().collect()),
        (0, mesh.caps[0].iter().flatten().copied().collect()),
        (18, mesh.caps[1].iter().flatten().copied().collect()),
    ] {
        let native = decode(rows[index]["after_hex"].as_str().unwrap());
        assert_eq!(actual.len(), native.len());
        let max = actual
            .iter()
            .zip(native)
            .map(|(a, b)| (a - b).abs())
            .fold(0f32, f32::max);
        assert!(max < 0.00005, "stream {index}: {max}");
    }
}
