//! Pinned native endPen width arithmetic for zero tilt and an initialized redraw.
//! These controls validate width state only, not complete streams or rendering.

fn width_state(
    middle: [f32; 2],
    end: [f32; 2],
    size: f32,
    end_pressure: f32,
    previous: f32,
) -> (f32, f32, f32, f32) {
    let dx = end[0] - middle[0];
    let dy = end[1] - middle[1];
    let distance = dx.mul_add(dx, dy * dy).sqrt();
    let factor = if distance < 4.2 {
        1.0
    } else if distance < 9.5 {
        ((distance + -4.2) / -5.3).mul_add(0.4, 0.4) + 0.6
    } else if distance < 35.0 {
        ((distance + -9.5) / -25.5).mul_add(0.5, 0.5) + 0.1
    } else {
        0.1
    };
    let pressure = end_pressure.min(1.0);
    let pressure_term = ((pressure * 3.0) * 0.5) * size * 0.5;
    let intermediate = size / 3.0 + pressure_term;
    let candidate = factor * intermediate;
    let bounded = if (previous - candidate).abs() > 1.0 {
        previous + if previous > candidate { -1.0 } else { 1.0 }
    } else {
        candidate
    };
    let target = bounded.max(pressure * size).max(5.0);
    (intermediate, factor, target, target.min(previous))
}

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
