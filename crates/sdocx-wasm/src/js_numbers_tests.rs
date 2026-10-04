use super::JsSafe;
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[test]
fn integers_cross_the_js_boundary_without_losing_sign_or_precision() {
    let signed = [
        0_i64,
        9_007_199_254_740_991,
        -9_007_199_254_740_991,
        9_007_199_254_740_992,
        -9_007_199_254_740_992,
        i64::MAX,
        i64::MIN,
    ];
    assert_eq!(
        serde_json::to_value(JsSafe(&signed)).unwrap(),
        json!([
            0,
            9_007_199_254_740_991_i64,
            -9_007_199_254_740_991_i64,
            "9007199254740992",
            "-9007199254740992",
            "9223372036854775807",
            "-9223372036854775808"
        ])
    );
    assert_eq!(
        serde_json::to_value(JsSafe([
            9_007_199_254_740_991_u64,
            9_007_199_254_740_992,
            u64::MAX,
        ]))
        .unwrap(),
        json!([
            9_007_199_254_740_991_u64,
            "9007199254740992",
            "18446744073709551615"
        ])
    );
}

#[derive(Serialize)]
struct Newtype(i64);

#[derive(Serialize)]
struct Pair(i64, u64);

#[derive(Serialize)]
enum Variant {
    Unit,
    Newtype(i64),
    Tuple(i64, u64),
    Struct { count: i64 },
}

struct Bytes<'a>(&'a [u8]);

impl Serialize for Bytes<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(self.0)
    }
}

#[derive(Serialize)]
struct Shapes<'a> {
    some: Option<Newtype>,
    none: Option<i64>,
    tuple: (i64, u64),
    pair: Pair,
    map: BTreeMap<&'a str, Vec<Variant>>,
    bytes: Bytes<'a>,
    floats: [f64; 2],
    small: (bool, char, i32, u32),
    #[serde(skip_serializing_if = "Option::is_none")]
    skipped: Option<i64>,
}

fn shapes(number: i64, unsigned: u64) -> Shapes<'static> {
    Shapes {
        some: Some(Newtype(number)),
        none: None,
        tuple: (number, unsigned),
        pair: Pair(number, unsigned),
        map: BTreeMap::from([(
            "entries",
            vec![
                Variant::Unit,
                Variant::Newtype(number),
                Variant::Tuple(number, unsigned),
                Variant::Struct { count: number },
            ],
        )]),
        bytes: Bytes(&[0, 128, 255]),
        floats: [-0.0, 9_007_199_254_740_992.0],
        small: (true, 'λ', i32::MIN, u32::MAX),
        skipped: None,
    }
}

#[test]
fn borrowed_transport_preserves_shapes_and_only_changes_unsafe_integer_values() {
    let safe = shapes(-42, 42);
    let baseline = serde_json::to_value(&safe).unwrap();
    let wrapped = serde_json::to_value(JsSafe(&safe)).unwrap();
    assert_eq!(wrapped, baseline);
    assert_eq!(
        wrapped["floats"][0].as_f64().unwrap().to_bits(),
        (-0.0_f64).to_bits()
    );
    assert_eq!(wrapped["floats"][1], json!(9_007_199_254_740_992.0));
    assert_eq!(wrapped["bytes"], json!([0, 128, 255]));
    assert!(!wrapped.as_object().unwrap().contains_key("skipped"));

    let unsafe_model = shapes(i64::MIN, u64::MAX);
    let output = serde_json::to_value(JsSafe(&unsafe_model)).unwrap();
    let signed = "-9223372036854775808";
    let unsigned = "18446744073709551615";
    assert_eq!(output["some"], signed);
    assert_eq!(output["none"], Value::Null);
    assert_eq!(output["tuple"], json!([signed, unsigned]));
    assert_eq!(output["pair"], json!([signed, unsigned]));
    assert_eq!(
        output["map"]["entries"],
        json!(["Unit", {"Newtype":signed}, {"Tuple":[signed,unsigned]}, {"Struct":{"count":signed}}])
    );
    for field in ["bytes", "floats", "small"] {
        assert_eq!(output[field], baseline[field]);
    }
    assert_eq!(
        serde_json::to_value(JsSafe(BTreeMap::from([(u64::MAX, Some(i64::MIN))]))).unwrap(),
        json!({"18446744073709551615":"-9223372036854775808"})
    );
}

#[test]
fn parsed_stroke_and_shape_metadata_stay_exact_in_physical_and_layout_inspection() {
    let bytes = include_bytes!("../../sdocx/tests/fixtures/inspection_integers.sdocx");
    let parsed = sdocx::parse_bytes_detailed(bytes).unwrap();
    let layout = sdocx::layout_document(&parsed.document);
    let original = serde_json::to_value(&parsed.document).unwrap();
    let output = serde_json::to_value(JsSafe(crate::inspection_data(&parsed, &layout))).unwrap();
    let controls = [
        (42_i64, json!(42)),
        (9_007_199_254_740_991, json!(9_007_199_254_740_991_i64)),
        (-9_007_199_254_740_991, json!(-9_007_199_254_740_991_i64)),
        (9_007_199_254_740_992, json!("9007199254740992")),
        (-9_007_199_254_740_992, json!("-9007199254740992")),
        (i64::MAX, json!("9223372036854775807")),
        (i64::MIN, json!("-9223372036854775808")),
    ];
    assert_eq!(
        original["pages"][0]["objects"].as_array().unwrap().len(),
        14
    );
    for (index, (timestamp, expected)) in controls.iter().enumerate() {
        let rendering =
            &original["pages"][0]["objects"][index * 2]["content"]["Stroke"]["rendering"];
        assert_eq!(rendering["metadata"]["modified_time_raw"], json!(timestamp));

        for page in [
            &output["document"]["pages"][0],
            &output["layout"]["pages"][0]["page"],
        ] {
            assert_eq!(page["objects"].as_array().unwrap().len(), 14);
            let stroke = &page["objects"][index * 2]["content"]["Stroke"];
            let shape = &page["objects"][index * 2 + 1]["content"]["Element"]["Shape"];
            assert_eq!(
                stroke["rendering"]["metadata"]["modified_time_raw"],
                *expected
            );
            assert_eq!(shape["metadata"]["modified_time_raw"], *expected);
            assert_eq!(stroke["timestamps"], json!([17]));
            assert_eq!(stroke["rendering"]["metadata"]["uuid"], "stamp");
        }

        let mut historical = rendering.clone();
        historical.as_object_mut().unwrap().remove("metadata");
        let historical: sdocx::StrokeRendering = serde_json::from_value(historical).unwrap();
        assert!(historical.metadata.is_none());
    }
    let restored: sdocx::Document = serde_json::from_value(original).unwrap();
    assert_eq!(restored.pages[0].strokes().count(), controls.len());
    for (stroke, (timestamp, _)) in restored.pages[0].strokes().zip(&controls) {
        let retained = stroke
            .rendering
            .as_ref()
            .unwrap()
            .metadata
            .as_ref()
            .unwrap();
        assert_eq!(retained.modified_time_raw, *timestamp);
    }
}
