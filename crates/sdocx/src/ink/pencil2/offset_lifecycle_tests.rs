// The fixture snapshots tools/pencil2-offset-trace/lifecycle-receipt.json. The
// capture script writes that receipt; refresh the fixture when adopting new
// evidence. The trace replay models recorded cache transitions; it does not
// test a production cache or native scheduling.
use super::offset_candidate as candidate;

fn floats<const N: usize>(hex: &str) -> [f32; N] {
    assert_eq!(hex.len(), N * 8);
    std::array::from_fn(|i| {
        let bytes = std::array::from_fn(|j| {
            u8::from_str_radix(&hex[i * 8 + j * 2..i * 8 + j * 2 + 2], 16).unwrap()
        });
        f32::from_le_bytes(bytes)
    })
}

#[test]
fn candidate_arithmetic_matches_native_start_point_trace() {
    let receipt: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/pencil2-offset-lifecycle-native.json"
    ))
    .unwrap();
    compare_receipt(&receipt);
}

#[test]
fn candidate_matches_non_square_native_start_point_trace() {
    let receipt: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/pencil2-offset-lifecycle-sized-native.json"
    ))
    .unwrap();
    compare_receipt(&receipt);
}

fn compare_receipt(receipt: &serde_json::Value) {
    let cases = receipt["cases"].as_object().unwrap();
    assert_eq!(cases.len(), 7);
    for (name, case) in cases {
        let dimensions = case
            .get("dimensions")
            .map(|v| [v[0].as_u64().unwrap() as u32, v[1].as_u64().unwrap() as u32])
            .unwrap_or([192, 192]);
        let mut point = None;
        let mut matrix = None;
        let mut state = candidate::Pencil2V14HostGrainState::default();
        let mut dirty = false;
        let mut uniforms = 0;
        for event in case["events"].as_array().unwrap() {
            match event["kind"].as_str().unwrap() {
                "set" => {
                    point = Some(floats::<2>(event["point"].as_str().unwrap()));
                    state.set_start_point(point.unwrap());
                    dirty = true;
                }
                "matrix" => {
                    matrix = Some((
                        floats::<16>(event["b"].as_str().unwrap()),
                        floats::<16>(event["d"].as_str().unwrap()),
                    ));
                }
                "repeat_matrix" => {
                    let (b, previous_d) = matrix.unwrap();
                    let d = floats::<16>(event["d"].as_str().unwrap());
                    assert_ne!(previous_d.map(f32::to_bits), d.map(f32::to_bits));
                    assert_ne!(
                        candidate::pencil2_v14_host_grain_offset(
                            b,
                            previous_d,
                            point.unwrap(),
                            dimensions
                        )
                        .unwrap()
                        .map(f32::to_bits),
                        candidate::pencil2_v14_host_grain_offset(b, d, point.unwrap(), dimensions)
                            .unwrap()
                            .map(f32::to_bits),
                        "{name}: changed matrix must distinguish recomputation"
                    );
                    matrix = Some((b, d));
                }
                "transform" => {
                    assert!(dirty, "{name}: transform without dirty start point");
                    let state = event["state"].as_str().unwrap();
                    assert_eq!(&state[16..], "01");
                    assert_eq!(
                        floats::<2>(&state[..16]).map(f32::to_bits),
                        point.unwrap().map(f32::to_bits)
                    );
                    dirty = false;
                }
                "uniform" => {
                    assert!(!dirty, "{name}: uniform before transform");
                    let (b, d) = matrix.unwrap();
                    assert_eq!(
                        state.draw(b, d, dimensions).unwrap().map(f32::to_bits),
                        floats::<2>(event["bits"].as_str().unwrap()).map(f32::to_bits),
                        "{name} uniform {uniforms}"
                    );
                    uniforms += 1;
                }
                kind => panic!("unexpected event: {kind}"),
            }
        }
        assert_eq!(uniforms, 4, "{name}");
        assert_eq!(
            case["gl_errors"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap())
                .collect::<Vec<_>>(),
            [0, 0, 0, 0]
        );
    }
}

#[test]
fn invalid_draw_preserves_pending_point() {
    let mut state = candidate::Pencil2V14HostGrainState::default();
    let mut identity = [0.; 16];
    for i in 0..4 {
        identity[5 * i] = 1.;
    }
    assert!(state.draw(identity, identity, [192, 192]).is_none());
    state.set_start_point([1., 2.]);
    assert!(state.draw(identity, identity, [0, 192]).is_none());
    assert_eq!(
        state.draw(identity, identity, [192, 192]),
        Some([192., 288.])
    );
}
