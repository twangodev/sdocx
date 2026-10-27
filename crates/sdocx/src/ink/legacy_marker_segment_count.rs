//! Test-only native Marker RTV1 non-circle segment count.
//!
//! This does not reconstruct the branch selector or circle path and is not
//! connected to rendering. The capture state precedes calcBezierPath.

fn count_non_circle(dot: f32, previous_length: f32, current_length: f32) -> i32 {
    let mean = ((previous_length + current_length) * 0.5).trunc();
    let scaled = ((dot - 1.0).abs() * mean).trunc() as i32;
    (scaled.max(-2) + 3).min(20)
}

#[test]
fn non_circle_count_matches_native_straight_and_bent_controls() {
    let receipt: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/legacy-marker-subdivision-controls.json"
    ))
    .unwrap();
    for (name, expected) in [("straight", [3, 3, 3]), ("bend", [3, 11, 11])] {
        let rows = receipt["cases"][name]["rows"].as_array().unwrap();
        let mut counts = Vec::new();
        for pair in rows.windows(2) {
            if pair[0]["method"] != "calcBezierPath" {
                continue;
            }
            assert_eq!(pair[1]["method"], "subdivideBezierPath");
            assert_eq!(pair[1]["rt_644_716_hex"].as_str().unwrap().len(), 152);
            let hex = pair[0]["rt_644_716_hex"].as_str().unwrap();
            let (chunks, remainder) = hex.as_bytes().as_chunks::<2>();
            assert!(remainder.is_empty());
            let bytes: Vec<u8> = chunks
                .iter()
                .map(|chunk| u8::from_str_radix(std::str::from_utf8(chunk).unwrap(), 16).unwrap())
                .collect();
            let field = |offset: usize| {
                f32::from_le_bytes(bytes[offset - 644..offset - 640].try_into().unwrap())
            };
            // calcBezierPath forms this dot product at 0x32ffc–0x33004.
            let dot = field(704).mul_add(field(696), field(700) * field(692));
            let observed_flag =
                u8::from_str_radix(&pair[1]["rt_644_716_hex"].as_str().unwrap()[144..146], 16)
                    .unwrap();
            assert_eq!(observed_flag, 0, "{name}: circle branch was selected");
            let count = count_non_circle(dot, field(708), field(712));
            assert_eq!(pair[1]["index_range"][0], 0);
            assert_eq!(
                pair[1]["index_range"][1].as_i64().unwrap(),
                i64::from(count)
            );
            counts.push(count);
        }
        assert_eq!(counts, expected, "{name}");
    }
}

#[test]
fn circle_selector_and_count_match_reversal_controls() {
    let receipt: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/legacy-marker-subdivision-controls.json"
    ))
    .unwrap();
    for case in receipt["circle_controls"].as_array().unwrap() {
        let rows = case["rows"].as_array().unwrap();
        let (pairs, remainder) = rows.as_chunks::<2>();
        assert!(remainder.is_empty());
        for pair in pairs {
            assert_eq!(pair[0]["method"], "calcBezierPath");
            assert_eq!(pair[1]["method"], "calcSegmentsNumber");
            let hex = pair[0]["rt_644_716_hex"].as_str().unwrap();
            let (chunks, remainder) = hex.as_bytes().as_chunks::<2>();
            assert!(remainder.is_empty());
            let bytes: Vec<u8> = chunks
                .iter()
                .map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 16).unwrap())
                .collect();
            let field = |offset: usize| {
                f32::from_le_bytes(bytes[offset - 644..offset - 640].try_into().unwrap())
            };
            let dot = field(704).mul_add(field(696), field(700) * field(692));
            assert_eq!(
                dot.to_bits(),
                u32::from_str_radix(pair[1]["dot_f32_bits"].as_str().unwrap(), 16).unwrap()
            );
            let points = pair[0]["control_points_f32"].as_array().unwrap();
            let coords = |i: usize, j: usize| points[i][j].as_f64().unwrap() as f32;
            let dx = coords(0, 0) - coords(1, 0);
            let dy = coords(0, 1) - coords(1, 1);
            let length = dx.mul_add(dx, dy * dy).sqrt();
            let circle = f64::from(dot) <= -0.7 && length > 0.6_f32;
            let state = pair[1]["rt_644_716_hex"].as_str().unwrap();
            let flag = u8::from_str_radix(&state[144..146], 16).unwrap();
            assert_eq!(u8::from(circle), flag);
            let count = if circle {
                let angle = if (0.996_f32..=1.004_f32).contains(&dot.abs()) {
                    180.0
                } else {
                    (f64::from(dot.acos() * 180.0) / std::f64::consts::PI) as f32
                };
                ((angle / 9.0 * 0.5) as i32) * 2
            } else {
                count_non_circle(dot, field(708), field(712))
            };
            assert_eq!(i64::from(count), pair[1]["return_count"].as_i64().unwrap());
        }
    }
}
