//! Test-only reconstruction of the outer Model saved-path normalizer.
//!
//! Arc and Oval point generation remains supplied by an independently executed
//! Model:0x211814 helper record in `shape-normalization.json`; this module does
//! not replace it with Drawing/Skia geometry.

use serde_json::Value;

const HELPER_TOLERANCE: f64 = 0.0005;
const FULL_TURN_NEAR: f32 = f32::from_bits(0x43b3_feb8);
const PI: f64 = f64::from_bits(0x4009_21fb_5444_2d18);

#[derive(Clone, Debug, PartialEq, Eq)]
struct Path(Vec<[u32; 7]>);

fn float(bits: u32) -> f32 {
    f32::from_bits(bits)
}

fn point(bits: &[u32], index: usize) -> [f32; 2] {
    [float(bits[index * 2]), float(bits[index * 2 + 1])]
}

fn command(tag: u32, points: &[[f32; 2]]) -> [u32; 7] {
    let mut result = [0; 7];
    result[0] = tag;
    match points {
        [p] => {
            result[1] = p[0].to_bits();
            result[2] = p[1].to_bits();
        }
        [a, b] => {
            result[1] = a[0].to_bits();
            result[2] = a[1].to_bits();
            result[5] = b[0].to_bits();
            result[6] = b[1].to_bits();
        }
        [a, b, c] => {
            result[1] = a[0].to_bits();
            result[2] = a[1].to_bits();
            result[3] = b[0].to_bits();
            result[4] = b[1].to_bits();
            result[5] = c[0].to_bits();
            result[6] = c[1].to_bits();
        }
        [] => {}
        _ => unreachable!(),
    }
    result
}

fn helper_points(helper: &Value) -> Vec<[f32; 2]> {
    let bits: Vec<u32> = helper["point_bits"]
        .as_array()
        .expect("helper points")
        .iter()
        .map(|value| value.as_u64().expect("point bits") as u32)
        .collect();
    assert_eq!(bits.len() % 2, 0);
    let count = helper["point_count"].as_u64().unwrap() as usize;
    assert_eq!(bits.len(), count * 2);
    (0..count).map(|index| point(&bits, index)).collect()
}

fn helper_input(segment: &[u32; 7]) -> [f32; 6] {
    match segment[0] {
        5 => {
            let left = float(segment[1]);
            let top = float(segment[2]);
            let width = float(segment[3]);
            let height = float(segment[4]);
            let sweep = float(segment[6]);
            [
                left,
                top,
                left + width,
                top + height,
                float(segment[5]),
                if sweep.to_bits() == 360.0f32.to_bits() {
                    FULL_TURN_NEAR
                } else {
                    sweep
                },
            ]
        }
        7 => {
            let x1 = float(segment[1]);
            let y1 = float(segment[2]);
            let x2 = float(segment[3]);
            let y2 = float(segment[4]);
            [
                x1.min(x2),
                y1.min(y2),
                x1.max(x2),
                y1.max(y2),
                0.0,
                FULL_TURN_NEAR,
            ]
        }
        tag => panic!("not a Model Arc/Oval command: {tag}"),
    }
}

fn linked(previous: &[u32; 7], start: [f32; 2]) -> bool {
    match previous[0] {
        1 | 2 => {
            let dx = (f64::from(float(previous[1])) - f64::from(start[0])).abs();
            let dy = (f64::from(float(previous[2])) - f64::from(start[1])).abs();
            dx < HELPER_TOLERANCE && dy < HELPER_TOLERANCE
        }
        3 => {
            float(previous[5]).to_bits() == start[0].to_bits()
                && float(previous[6]).to_bits() == start[1].to_bits()
        }
        4 => {
            float(previous[5]).to_bits() == start[0].to_bits()
                && float(previous[6]).to_bits() == start[1].to_bits()
        }
        _ => false,
    }
}

fn snap(previous: &mut [u32; 7], start: [f32; 2]) {
    previous[1] = start[0].to_bits();
    previous[2] = start[1].to_bits();
}

fn model_commands(points: &[[f32; 2]], linked: bool, close: bool, output: &mut Vec<[u32; 7]>) {
    if !linked {
        output.push(command(1, &points[..1]));
    }
    let mut index = 1;
    while index + 1 < points.len() {
        output.push(command(3, &points[index..index + 2]));
        index += 2;
    }
    assert_eq!(
        index,
        points.len(),
        "Model helper emits start + quadratic pairs"
    );
    if close {
        output.push(command(6, &[]));
    }
}

fn rotate_point(point: [f32; 2], center: [f32; 2], angle: f32) -> [f32; 2] {
    if angle == 0.0 {
        return point;
    }
    let radians = (f64::from(angle) / 180.0) * PI;
    let (sine, cosine) = radians.sin_cos();
    let dy = f64::from(point[1] - center[1]);
    let dx = f64::from(point[0] - center[0]);
    let x = dx.mul_add(cosine, -(sine * dy));
    let cosine_dy = cosine * dy;
    let y = dx.mul_add(sine, cosine_dy);
    [center[0] + x as f32, center[1] + y as f32]
}

fn rotate_path(path: &mut Path, bounds: [f32; 4], angle: f32) {
    let center = [(bounds[0] + bounds[2]) * 0.5, (bounds[1] + bounds[3]) * 0.5];
    for segment in &mut path.0 {
        let count = match segment[0] {
            1 | 2 => 1,
            3 => 2,
            4 => 3,
            6 => 0,
            tag => panic!("unexpected normalized command tag {tag}"),
        };
        for index in 0..count {
            let slots = if segment[0] == 3 && index == 1 {
                5
            } else {
                1 + index * 2
            };
            let rotated = rotate_point(
                [float(segment[slots]), float(segment[slots + 1])],
                center,
                angle,
            );
            segment[slots] = rotated[0].to_bits();
            segment[slots + 1] = rotated[1].to_bits();
        }
    }
}

fn normalize(case: &Value, source: &[[u32; 7]]) -> (Path, Path, Vec<[u32; 7]>, [u32; 2]) {
    let helpers = case["model_helpers"].as_array().unwrap();
    let mut output = Vec::new();
    let mut mutated = source.to_vec();
    let mut helper_cursor = 0;
    for index in 0..mutated.len() {
        match mutated[index][0] {
            5 | 7 => {
                let helper = &helpers[helper_cursor];
                helper_cursor += 1;
                assert_eq!(helper["segment_index"].as_u64().unwrap() as usize, index);
                let arguments = helper_input(&mutated[index]);
                let argument_bits = arguments.map(f32::to_bits);
                let captured_arguments: Vec<u32> = helper["input_bits"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| value.as_u64().unwrap() as u32)
                    .collect();
                assert_eq!(captured_arguments, argument_bits, "Model helper input");
                let captured_points = helper_points(helper);
                // Isolate the outer loader contract from the independent
                // point-builder test below. The fixture values here are
                // direct native outputs from Model:0x211814.
                let points = captured_points;
                assert!(!points.is_empty(), "native helper output: {helper:?}");
                let is_arc = mutated[index][0] == 5;
                let is_full = is_arc && float(mutated[index][6]).to_bits() == 360.0f32.to_bits();
                if is_full {
                    mutated[index][6] = FULL_TURN_NEAR.to_bits();
                }
                let is_linked = is_arc && index > 0 && linked(&mutated[index - 1], points[0]);
                if is_linked && matches!(mutated[index - 1][0], 1 | 2) {
                    snap(&mut mutated[index - 1], points[0]);
                    *output.last_mut().expect("linked predecessor was copied") = mutated[index - 1];
                }
                model_commands(&points, is_linked, is_full || !is_arc, &mut output);
            }
            1 => output.push(mutated[index]),
            2 => output.push(mutated[index]),
            3 | 4 | 6 => output.push(mutated[index]),
            tag => panic!("unsupported source tag {tag}"),
        }
    }
    assert_eq!(helper_cursor, helpers.len());

    let input_bounds: [f32; 4] = case["bounds_bits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| float(value.as_u64().unwrap() as u32))
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    let mut bounds = input_bounds;
    let mut flip_x = case["flip_inputs"][0].as_u64().unwrap() != 0;
    let mut flip_y = case["flip_inputs"][1].as_u64().unwrap() != 0;
    if bounds[0] > bounds[2] {
        bounds.swap(0, 2);
        flip_x = !flip_x;
    }
    if bounds[1] > bounds[3] {
        bounds.swap(1, 3);
        flip_y = !flip_y;
    }
    let angle = float(case["angle_bits"].as_u64().unwrap() as u32);
    let input_frame = case["input_frame"].as_u64().unwrap() != 0;
    let mut base = Path(output);
    if input_frame && angle != 0.0 {
        rotate_path(&mut base, bounds, -angle);
    }
    let mut display = base.clone();
    if angle != 0.0 {
        rotate_path(&mut display, bounds, angle);
    }
    let orientation = [u32::from(flip_x), u32::from(flip_y)];
    (base, display, mutated, orientation)
}

fn segments(value: &Value, field: &str) -> Vec<[u32; 7]> {
    value[field]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            row.as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u32)
                .collect::<Vec<_>>()
                .try_into()
                .unwrap()
        })
        .collect()
}

fn assert_active_fields(actual: &[[u32; 7]], expected: &[[u32; 7]], context: &str) {
    assert_eq!(actual.len(), expected.len(), "segment count: {context}");
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        assert_eq!(actual[0], expected[0], "verb at {context}[{index}]");
        let fields: &[usize] = match actual[0] {
            1 | 2 => &[1, 2],
            3 => &[1, 2, 5, 6],
            4 => &[1, 2, 3, 4, 5, 6],
            6 => &[],
            tag => panic!("unsupported active-field comparison tag {tag}"),
        };
        for field in fields {
            assert_eq!(
                actual[*field], expected[*field],
                "field {field} at {context}[{index}]"
            );
        }
    }
}

fn assert_source_fields(actual: &[[u32; 7]], expected: &[[u32; 7]], context: &str) {
    assert_eq!(
        actual.len(),
        expected.len(),
        "source segment count: {context}"
    );
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        assert_eq!(actual[0], expected[0], "source verb at {context}[{index}]");
        let fields: &[usize] = match actual[0] {
            1 | 2 => &[1, 2],
            3 => &[1, 2, 5, 6],
            4 | 5 => &[1, 2, 3, 4, 5, 6],
            6 => &[],
            7 => &[1, 2, 3, 4],
            tag => panic!("unsupported source-field comparison tag {tag}"),
        };
        for field in fields {
            assert_eq!(
                actual[*field], expected[*field],
                "source field {field} at {context}[{index}]"
            );
        }
    }
}

fn encode_native_path(source: &[[u32; 7]]) -> Vec<u8> {
    let mut bytes = (source.len() as u32).to_le_bytes().to_vec();
    for row in source {
        bytes.push(row[0] as u8);
        let fields: &[usize] = match row[0] {
            1 | 2 => &[1, 2],
            3 => &[1, 2, 5, 6],
            4 | 5 => &[1, 2, 3, 4, 5, 6],
            6 => &[],
            7 => &[1, 2, 3, 4],
            tag => panic!("unsupported fixture source tag {tag}"),
        };
        for field in fields {
            bytes.extend(f64::from(float(row[*field])).to_le_bytes());
        }
    }
    bytes
}

fn model_commands_bits(commands: &[super::NativePathCommand]) -> Vec<[u32; 7]> {
    commands
        .iter()
        .map(|command| match command {
            super::NativePathCommand::Move([x, y]) => {
                [1, (*x as f32).to_bits(), (*y as f32).to_bits(), 0, 0, 0, 0]
            }
            super::NativePathCommand::Line([x, y]) => {
                [2, (*x as f32).to_bits(), (*y as f32).to_bits(), 0, 0, 0, 0]
            }
            super::NativePathCommand::Quadratic([[cx, cy], [x, y]]) => [
                3,
                (*cx as f32).to_bits(),
                (*cy as f32).to_bits(),
                0,
                0,
                (*x as f32).to_bits(),
                (*y as f32).to_bits(),
            ],
            super::NativePathCommand::Cubic([[x1, y1], [x2, y2], [x, y]]) => [
                4,
                (*x1 as f32).to_bits(),
                (*y1 as f32).to_bits(),
                (*x2 as f32).to_bits(),
                (*y2 as f32).to_bits(),
                (*x as f32).to_bits(),
                (*y as f32).to_bits(),
            ],
            super::NativePathCommand::Close => [6, 0, 0, 0, 0, 0, 0],
            super::NativePathCommand::Arc(_) | super::NativePathCommand::Oval(_) => {
                panic!("Model path retained an unexpanded command")
            }
        })
        .collect()
}

#[test]
fn model_outer_normalizer_matches_native_model_helper_and_path_captures() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../conformance/shape-normalization.json"
    ))
    .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let source = segments(case, "input_bits");
        let (base, display, mutated, orientation) = normalize(case, &source);
        let expected_base = segments(case, "base_segments");
        let expected_display = segments(case, "display_segments");
        let expected_source = segments(case, "input_after");
        assert_active_fields(&base.0, &expected_base, "base path");
        assert_active_fields(&display.0, &expected_display, "display path");
        assert_source_fields(&mutated, &expected_source, "mutated source");
        let expected_orientation: [u32; 2] = case["stored_orientation"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as u32)
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        assert_eq!(
            orientation, expected_orientation,
            "orientation: {}",
            case["name"]
        );
    }
}

#[test]
fn rust_model_arc_builder_matches_direct_native_helper_bits() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../conformance/shape-normalization.json"
    ))
    .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        for helper in case["model_helpers"].as_array().unwrap() {
            let arguments: [f32; 6] = helper["input_bits"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| float(value.as_u64().unwrap() as u32))
                .collect::<Vec<_>>()
                .try_into()
                .unwrap();
            let actual = super::model_arc::expand(arguments)
                .unwrap_or_else(|| panic!("Model helper rejected case {helper:?}"));
            assert_eq!(
                actual
                    .iter()
                    .map(|point| point.map(f32::to_bits))
                    .collect::<Vec<_>>(),
                helper_points(helper)
                    .iter()
                    .map(|point| point.map(f32::to_bits))
                    .collect::<Vec<_>>(),
                "Model:0x211814 output: {helper:?}"
            );
        }
    }
}

#[test]
fn production_typed_model_path_matches_native_saved_load_captures() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../conformance/shape-normalization.json"
    ))
    .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let source = segments(case, "input_bits");
        let bytes = encode_native_path(&source);
        let bounds: [f64; 4] = case["bounds_bits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| f64::from(float(value.as_u64().unwrap() as u32)))
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        let flips = &case["flip_inputs"];
        let normalized = super::native_path::normalize_model_path(
            &bytes,
            bounds,
            float(case["angle_bits"].as_u64().unwrap() as u32),
            flips[0].as_u64().unwrap() != 0,
            flips[1].as_u64().unwrap() != 0,
            case["input_frame"].as_u64().unwrap() != 0,
        )
        .unwrap()
        .unwrap_or_else(|| panic!("native path rejected case {}", case["name"]));
        assert_active_fields(
            &model_commands_bits(&normalized.commands),
            &segments(case, "display_segments"),
            &format!("production Model display path {}", case["name"]),
        );
        let actual_orientation = [
            u32::from(normalized.horizontal_flip),
            u32::from(normalized.vertical_flip),
        ];
        let expected_orientation: [u32; 2] = case["stored_orientation"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_u64().unwrap() as u32)
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        assert_eq!(
            actual_orientation, expected_orientation,
            "production Model orientation: {}",
            case["name"]
        );
        assert_eq!(
            bytes,
            encode_native_path(&source),
            "raw source is preserved"
        );
    }
}

#[test]
fn repeated_full_arc_load_observes_first_load_source_mutation() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../conformance/shape-normalization.json"
    ))
    .unwrap();
    let case = fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == "arc-positive-full-turn")
        .unwrap();
    let first_source = segments(case, "input_bits");
    let (first, _, mutated, _) = normalize(case, &first_source);
    assert_active_fields(
        &first.0,
        &segments(case, "base_segments"),
        "first base path",
    );
    assert_source_fields(
        &mutated,
        &segments(case, "input_after"),
        "first source mutation",
    );
    let (second, _, second_mutated, _) = normalize(case, &mutated);
    assert_active_fields(
        &second.0,
        &segments(case, "repeat_base_segments"),
        "repeat base path",
    );
    assert_source_fields(
        &second_mutated,
        &segments(case, "repeat_input_after"),
        "repeat source mutation",
    );
    assert_eq!(first.0.last().unwrap()[0], 6);
    assert_ne!(second.0.last().unwrap()[0], 6);
}
