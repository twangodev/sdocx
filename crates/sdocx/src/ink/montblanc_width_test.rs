//! Pinned native endPen width arithmetic for zero tilt and an initialized redraw.
//! These controls validate width state only, not complete streams or rendering.

use super::width_state;

fn word(value: &serde_json::Value) -> u32 {
    let text = value.as_str().unwrap();
    let bytes: [u8; 4] =
        std::array::from_fn(|i| u8::from_str_radix(&text[i * 2..i * 2 + 2], 16).unwrap());
    u32::from_le_bytes(bytes)
}

fn number(value: &serde_json::Value) -> f32 {
    value.as_f64().unwrap() as f32
}

fn check(name: &str, input: &serde_json::Value, registers: &serde_json::Value) {
    let point = |index: usize| {
        [
            number(&input["points"][index][0]),
            number(&input["points"][index][1]),
        ]
    };
    let read = |rva: &str, reg: &str| word(&registers[rva][reg]);
    let previous = f32::from_bits(read("0x1dbb4", "s1_f32_bits"));
    let actual = width_state(
        point(1),
        point(2),
        number(&input["pen_size"]),
        number(&input["pressure"][2]),
        previous,
    );
    let expected = [
        read("0x1db88", "s0_f32_bits"),
        read("0x1db88", "s11_f32_bits"),
        read("0x1dbb8", "s14_f32_bits"),
        read("0x1dbc4", "s9_f32_bits"),
    ];
    assert_eq!(
        [
            actual.0.to_bits(),
            actual.1.to_bits(),
            actual.2.to_bits(),
            actual.3.to_bits()
        ],
        expected,
        "{name}"
    );
}

#[test]
fn native_width_state_receipts() {
    let receipt: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/montblanc-wide-width-state-native.json"
    ))
    .unwrap();
    let paths: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/montblanc-wide-paths-native.json"
    ))
    .unwrap();
    let controls = receipt["controls"].as_object().unwrap();
    assert_eq!(controls.len(), 19);
    for (name, case) in controls {
        check(name, &case["input"], &case["registers"]);
    }
    let cases = receipt["cases"].as_object().unwrap();
    assert_eq!(cases.len(), 3);
    for (name, rows) in cases {
        let mut registers = serde_json::Map::new();
        for row in rows.as_array().unwrap() {
            registers.insert(
                row["rva"].as_str().unwrap().to_owned(),
                row["s_registers"].clone(),
            );
        }
        check(
            name,
            &paths["cases"][name]["input"],
            &serde_json::Value::Object(registers),
        );
    }
}

#[test]
fn native_narrow_size_selection_matches_sampler() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/montblanc-wide-width-state-native.json"
    ))
    .unwrap();
    let originals = &fixture["narrow_state_original"];
    let initial: serde_json::Value =
        serde_json::from_str(originals["state-receipt.json"].as_str().unwrap()).unwrap();
    let trace: serde_json::Value =
        serde_json::from_str(originals["state-v8-receipt.json"].as_str().unwrap()).unwrap();
    let captured = word(&initial["observation"]["source_model_get_pen_size_f32_bits"]);
    let selected = word(&initial["observation"]["native_pen_getsize_f32_bits"]);
    assert_eq!(captured, 0.25_f32.to_bits());
    assert_eq!(selected, 0.4_f32.to_bits());
    assert_eq!(word(&trace["model_size_f32_bits"]), captured);
    assert_eq!(word(&trace["pen_getter_f32_bits"]), selected);
    assert_eq!(
        word(&initial["drawable_pen_data_width_source_chain"]["gl_data_first_f32_bits"]),
        selected
    );
    let native_base = word(&trace["end_pen_width_register_trace"][2]["s_registers"]["s0_f32_bits"]);
    assert_eq!((f32::from_bits(selected) / 3.).to_bits(), native_base);
    let points = [
        [0., 0.],
        [7., 3.],
        [14., -2.],
        [21., 4.],
        [28., -1.],
        [35., 3.],
    ];
    let pressure = [0., 1., 0.1, 0.9, 0.2, 1.];
    let run = |pen_size| {
        super::many_point::many_point(super::Input {
            points: &points,
            pressure: &pressure,
            pen_size,
        })
        .unwrap()
    };
    let a = run(f32::from_bits(captured));
    let b = run(f32::from_bits(selected));
    assert!(!a.is_empty());
    assert_eq!(a.len(), b.len());
    for (a, b) in a.iter().zip(b) {
        assert_eq!(a.call, b.call);
        assert_eq!(a.lanes.map(f32::to_bits), b.lanes.map(f32::to_bits));
    }
}
