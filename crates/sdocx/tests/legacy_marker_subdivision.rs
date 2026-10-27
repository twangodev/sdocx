use serde::Deserialize;
use std::collections::BTreeMap;

type Points = [[f32; 2]; 3];

#[derive(Deserialize)]
struct Fixture {
    cases: BTreeMap<String, Case>,
}

#[derive(Deserialize)]
struct Case {
    rows: Vec<Row>,
}

#[derive(Deserialize)]
struct Row {
    method: String,
    phase: String,
    control_points_f32: Points,
    index_range: Option<[i32; 2]>,
}

fn midpoint(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5]
}

fn subdivide(points: Points, range: [i32; 2], entries: &mut Vec<(Points, [i32; 2])>) {
    entries.push((points, range));
    if range[1] - range[0] < 2 {
        return;
    }
    let [a, b, c] = points;
    let ab = midpoint(a, b);
    let bc = midpoint(b, c);
    // Native uses separately rounded weighted terms, rather than midpoint(ab, bc).
    let center = [
        (a[0] * 0.25 + b[0] * 0.5) + c[0] * 0.25,
        (a[1] * 0.25 + b[1] * 0.5) + c[1] * 0.25,
    ];
    let middle = (range[0] + range[1]) / 2;
    subdivide([a, ab, center], [range[0], middle], entries);
    subdivide([center, bc, c], [middle, range[1]], entries);
}

#[test]
fn captured_legacy_marker_quadratic_subdivision_entries() {
    let fixture: Fixture = serde_json::from_str(include_str!(
        "../../../conformance/legacy-marker-subdivision-controls.json"
    ))
    .unwrap();
    for (name, expected_count) in [("straight", 15), ("bend", 47)] {
        let rows = &fixture.cases[name].rows;
        let mut position = 0;
        let mut count = 0;
        while position < rows.len() {
            let calc = &rows[position];
            assert_eq!(calc.method, "calcBezierPath");
            assert_eq!(calc.index_range, None);
            position += 1;
            let root = &rows[position];
            assert_eq!(root.control_points_f32, calc.control_points_f32);
            assert_eq!(root.phase, calc.phase);
            let mut entries = Vec::new();
            subdivide(
                root.control_points_f32,
                root.index_range.unwrap(),
                &mut entries,
            );
            for (points, range) in entries {
                let native = &rows[position];
                assert_eq!(
                    native.method, "subdivideBezierPath",
                    "{name} row {position}"
                );
                assert_eq!(native.phase, calc.phase, "{name} row {position}");
                assert_eq!(native.index_range, Some(range), "{name} row {position}");
                for (actual, expected) in points
                    .iter()
                    .flatten()
                    .zip(native.control_points_f32.iter().flatten())
                {
                    assert!(
                        actual.to_bits() == expected.to_bits(),
                        "{name} row {position}: {actual} != {expected}"
                    );
                }
                count += 1;
                position += 1;
            }
        }
        assert_eq!(count, expected_count, "{name}");
    }
}
