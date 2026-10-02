use super::native_geometry::NativeCodeGeometry;
use crate::BoundingBox;
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
struct Capture {
    apk_version: String,
    allocation_fills: Vec<u8>,
    measure_address: String,
    capture_boundary: String,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    bounds: [f32; 4],
    density: f32,
    split_rectangles: Vec<[f32; 4]>,
    supplied_body_height: f32,
    supplied_body_present: bool,
    supplied_first_line_height: f32,
    cold: Snapshot,
    warm: Snapshot,
    cleared: Snapshot,
}

#[derive(Deserialize)]
struct Snapshot {
    copy: [f32; 4],
    title: [f32; 4],
    body: [f32; 4],
    measured: [f32; 4],
    content: [f32; 4],
    minimum_first_page_height: f32,
    title_child: Child,
    body_child: Child,
}

#[derive(Deserialize)]
struct Child {
    frame: Option<[f32; 4]>,
    padding: Vec<[f32; 4]>,
    measures: usize,
}

fn bounds(rectangle: [f32; 4]) -> BoundingBox {
    let [x_min, y_min, x_max, y_max] = rectangle.map(f64::from);
    BoundingBox {
        x_min,
        y_min,
        x_max,
        y_max,
    }
}

fn bits(rectangle: BoundingBox) -> [u32; 4] {
    [
        rectangle.x_min,
        rectangle.y_min,
        rectangle.x_max,
        rectangle.y_max,
    ]
    .map(|value| (value as f32).to_bits())
}

fn compare(case: &Case, source: [f32; 4], body_height: f32, snapshot: &Snapshot) {
    let rectangles: Vec<_> = case.split_rectangles.iter().copied().map(bounds).collect();
    let geometry =
        NativeCodeGeometry::new(bounds(source), case.density, rectangles.first().copied()).unwrap();
    for (label, actual, expected) in [
        ("copy", geometry.copy, snapshot.copy),
        ("title", geometry.title, snapshot.title),
        (
            "body frame",
            geometry.body,
            snapshot.body_child.frame.unwrap_or(snapshot.body),
        ),
        (
            "panel",
            geometry.measured_bounds(body_height).unwrap(),
            snapshot.measured,
        ),
    ] {
        assert_eq!(
            bits(actual),
            expected.map(f32::to_bits),
            "{} {label}",
            case.name
        );
    }
    assert_eq!(snapshot.content, snapshot.measured, "{} content", case.name);
    assert_eq!(
        snapshot.title_child.frame,
        Some(snapshot.title),
        "{} title frame",
        case.name
    );
    let mut measured_body = geometry.body;
    measured_body.y_max = f64::from(measured_body.y_min as f32 + body_height);
    assert_eq!(
        bits(measured_body),
        snapshot.body.map(f32::to_bits),
        "{} body",
        case.name
    );
    assert_eq!(
        (geometry
            .minimum_first_page_height(case.supplied_first_line_height)
            .unwrap() as f32)
            .to_bits(),
        snapshot.minimum_first_page_height.to_bits(),
        "{} minimum",
        case.name,
    );
    let padding = geometry.body_padding(&rectangles).unwrap();
    if case.supplied_body_present {
        assert_eq!(padding.len(), snapshot.body_child.padding.len());
        for (actual, expected) in padding.into_iter().zip(&snapshot.body_child.padding) {
            assert_eq!(
                bits(actual),
                expected.map(f32::to_bits),
                "{} body padding",
                case.name
            );
        }
    } else {
        assert_eq!(snapshot.body_child.measures, 0);
        assert!(snapshot.body_child.padding.is_empty());
    }
}

#[test]
fn chrome_and_supplied_child_metrics_match_native_cold_and_cleared_measurement() {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../conformance/table-code-layout.json"
    ));
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "cf98c08c0a67850b3db1bf044b5056161742c7853456cdcc9f7b4c2c044f6f0f"
    );
    let capture: Capture = serde_json::from_slice(bytes).unwrap();
    assert_eq!(capture.apk_version, "4.4.45.37");
    assert_eq!(capture.allocation_fills, [0, 165, 255]);
    assert_eq!(capture.measure_address, "Drawing+0x732fc");
    assert!(
        capture
            .capture_boundary
            .contains("actual child shaping/wrapping/margins")
    );
    assert_eq!(capture.cases.len(), 18);
    for case in &capture.cases {
        let body_height = if case.supplied_body_present {
            case.supplied_body_height
        } else {
            0.0
        };
        compare(case, case.bounds, body_height, &case.cold);
        assert_eq!(
            case.warm.measured, case.cold.measured,
            "{} warm snapshot",
            case.name
        );
        assert_eq!(case.warm.title_child.measures, 1);
        assert_eq!(
            case.warm.body_child.measures,
            usize::from(case.supplied_body_present)
        );
        let mutated_source =
            std::array::from_fn(|axis| case.bounds[axis] + [17.0, 25.0, 111.0, 1000.0][axis]);
        compare(
            case,
            mutated_source,
            if case.supplied_body_present {
                body_height + 100.0
            } else {
                0.0
            },
            &case.cleared,
        );
    }
}
