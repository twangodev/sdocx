use serde_json::Value;
use std::collections::BTreeMap;

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

#[test]
fn legacy_marker_observed_merge_and_cap_calls() {
    // These are models of the captured straight and bent controls, not renderer dispatch.
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../conformance/legacy-marker-source-geometry-controls.json"
    ))
    .unwrap();
    for case in ["straight", "bend"] {
        let rows = fixture["cases"][case]["mesh_vector_outputs"]
            .as_array()
            .unwrap();
        let mut vectors: BTreeMap<usize, Vec<f32>> = BTreeMap::new();
        assert_eq!(rows.len(), 15);
        for (index, row) in rows.iter().enumerate() {
            let input = row["geometry_inputs"].as_array().unwrap();
            let method = row["method"].as_str().unwrap();
            let stride = row["return_vector"]["element_stride"].as_u64().unwrap();
            let kind = if method.contains("::mergePoints(") {
                if stride == 8 { "merge2" } else { "merge3" }
            } else if method.contains("::addStartEndTexExtraMesh(") {
                "cap_fringe"
            } else if method.contains("::addStartEndTexMesh(") {
                "cap"
            } else {
                panic!("unexpected method {method}")
            };
            let actual: Vec<Vec<f32>> = match kind {
                "merge2" | "merge3" => {
                    let first = points(&input[0]["components_f32"]);
                    let second = points(&input[1]["components_f32"]);
                    assert_eq!(first.len(), second.len());
                    first
                        .iter()
                        .zip(&second)
                        .flat_map(|(a, b)| {
                            if kind == "merge2" {
                                [vec![b[0], b[1]], vec![a[0], a[1]]]
                            } else {
                                [vec![b[0], b[1], 0.0], vec![a[0], a[1], 1.0]]
                            }
                        })
                        .collect()
                }
                "cap" => {
                    let p: Vec<Point> = input.iter().map(point).collect();
                    [
                        (0, 0.0, 0.0),
                        (2, 1.0, 0.0),
                        (1, 0.0, 1.0),
                        (2, 1.0, 0.0),
                        (3, 1.0, 1.0),
                        (1, 0.0, 1.0),
                    ]
                    .into_iter()
                    .map(|(i, u, v)| vec![p[i][0], p[i][1], u, v])
                    .collect()
                }
                "cap_fringe" => {
                    let a = point(&input[0]);
                    let b = point(&input[1]);
                    let n = point(&input[2]);
                    [a, b]
                        .into_iter()
                        .flat_map(|p| [vec![p[0], p[1], 0.0], vec![p[0] + n[0], p[1] + n[1], 1.0]])
                        .collect()
                }
                _ => panic!("unknown captured call {kind}"),
            };
            // The receipt omits output pointers; the two fringe destinations are
            // identified by their fixed call positions in these two traces.
            let channel = match index {
                0 | 12 => 3 + index,
                1 | 4 | 7 | 10 | 13 => 1,
                2 | 5 | 8 | 11 | 14 => 2,
                3 | 6 | 9 => 0,
                _ => unreachable!(),
            };
            let previous = vectors.entry(channel).or_default();
            previous.extend(actual.into_iter().flatten());
            let hex = row["return_vector"]["data_hex"].as_str().unwrap();
            let bytes: Vec<u8> = hex
                .as_bytes()
                .as_chunks::<2>()
                .0
                .iter()
                .map(|chunk| u8::from_str_radix(std::str::from_utf8(chunk).unwrap(), 16).unwrap())
                .collect();
            let expected: Vec<f32> = bytes
                .as_chunks::<4>()
                .0
                .iter()
                .map(|chunk| f32::from_le_bytes(*chunk))
                .collect();
            assert_eq!(previous.len(), expected.len(), "{case} {index} {kind}");
            assert_eq!(*previous, expected, "{case} {index} {kind}");
        }
    }
}
