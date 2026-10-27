use serde_json::Value;

type Point = [f32; 2];

fn point(value: &Value) -> Point {
    [
        value[0].as_f64().unwrap() as f32,
        value[1].as_f64().unwrap() as f32,
    ]
}

fn points(value: &Value) -> Vec<Point> {
    value.as_array().unwrap().iter().map(point).collect()
}

fn state_float(hex: &str, offset: usize) -> f32 {
    let start = 2 * (offset - 644);
    let bytes: Vec<u8> = (0..4)
        .map(|i| u8::from_str_radix(&hex[start + 2 * i..start + 2 * i + 2], 16).unwrap())
        .collect();
    f32::from_le_bytes(bytes.try_into().unwrap())
}

fn near(a: Point, b: Point, context: &str) {
    for i in 0..2 {
        assert!((a[i] - b[i]).abs() < 0.00003, "{context}: {a:?} != {b:?}");
    }
}

#[test]
fn captured_subdivision_edges_and_fringe() {
    // Synthetic straight and bent Drawing controls; this is not renderer dispatch.
    let geometry: Value = serde_json::from_str(include_str!(
        "../../../conformance/legacy-marker-source-geometry-controls.json"
    ))
    .unwrap();
    let subdivision: Value = serde_json::from_str(include_str!(
        "../../../conformance/legacy-marker-subdivision-controls.json"
    ))
    .unwrap();
    for case in ["straight", "bend"] {
        let meshes: Vec<_> = geometry["cases"][case]["mesh_vector_outputs"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["method"].as_str().unwrap().contains("::mergePoints("))
            .collect();
        let rows = subdivision["cases"][case]["rows"].as_array().unwrap();
        let mut groups: Vec<Vec<&Value>> = Vec::new();
        for row in rows {
            if row["method"] == "calcBezierPath" {
                groups.push(vec![row]);
            } else {
                groups.last_mut().unwrap().push(row);
            }
        }
        assert_eq!(groups.len(), 3);
        assert_eq!(meshes.len(), groups.len() * 3);
        for (group_index, group) in groups.iter().enumerate() {
            let body = meshes[group_index * 3];
            let inputs = &body["geometry_inputs"];
            let first = points(&inputs[0]["components_f32"]);
            let second = points(&inputs[1]["components_f32"]);
            assert_eq!(first.len(), second.len());
            let state = group[0]["rt_644_716_hex"].as_str().unwrap();
            let radius = state_float(state, 644);
            assert_eq!(radius, 5.5);
            let controls = points(&group[0]["control_points_f32"]);
            for (index, center, normal) in [
                (
                    0,
                    controls[0],
                    [state_float(state, 700), state_float(state, 704)],
                ),
                (
                    first.len() - 1,
                    controls[2],
                    [state_float(state, 692), state_float(state, 696)],
                ),
            ] {
                near(
                    first[index],
                    [
                        center[0] + radius * normal[0],
                        center[1] + radius * normal[1],
                    ],
                    case,
                );
                near(
                    second[index],
                    [
                        center[0] - radius * normal[0],
                        center[1] - radius * normal[1],
                    ],
                    case,
                );
            }
            for row in group.iter().skip(1) {
                let range = row["index_range"].as_array().unwrap();
                let lo = range[0].as_u64().unwrap() as usize;
                let hi = range[1].as_u64().unwrap() as usize;
                if hi - lo < 2 {
                    continue;
                }
                let index = (lo + hi) / 2;
                let p = points(&row["control_points_f32"]);
                let center = [
                    (p[0][0] + 2.0 * p[1][0] + p[2][0]) * 0.25,
                    (p[0][1] + 2.0 * p[1][1] + p[2][1]) * 0.25,
                ];
                let tangent = [p[2][0] - p[0][0], p[2][1] - p[0][1]];
                let length = tangent[0].hypot(tangent[1]);
                assert!(length > 0.000001);
                let n = [tangent[1] / length, -tangent[0] / length];
                near(
                    first[index],
                    [center[0] + radius * n[0], center[1] + radius * n[1]],
                    case,
                );
                near(
                    second[index],
                    [center[0] - radius * n[0], center[1] - radius * n[1]],
                    case,
                );
            }
            let left = &meshes[group_index * 3 + 1]["geometry_inputs"];
            let right = &meshes[group_index * 3 + 2]["geometry_inputs"];
            let left_outer = points(&left[0]["components_f32"]);
            let left_inner = points(&left[1]["components_f32"]);
            let right_outer = points(&right[0]["components_f32"]);
            let right_inner = points(&right[1]["components_f32"]);
            assert_eq!(left_inner, second);
            assert_eq!(right_inner, first);
            for i in 0..first.len() {
                let delta = [
                    (first[i][0] - second[i][0]) / (2.0 * radius),
                    (first[i][1] - second[i][1]) / (2.0 * radius),
                ];
                near(
                    left_outer[i],
                    [second[i][0] - delta[0], second[i][1] - delta[1]],
                    case,
                );
                near(
                    right_outer[i],
                    [first[i][0] + delta[0], first[i][1] + delta[1]],
                    case,
                );
            }
        }
    }
}
