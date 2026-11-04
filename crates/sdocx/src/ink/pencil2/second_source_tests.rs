use super::prepare_stroke;
use crate::{Stroke, StrokeProperties, StrokeRendering, StrokeStyle};
use sha2::{Digest, Sha256};

// This remaps decoded Fountain channels to a fresh setter-built Pencil2 host
// profile. The native fixture is a pre-Update descriptor, not a device VBO.
#[test]
#[allow(clippy::filter_map_bool_then)]
fn second_source_preserves_native_geometry_except_three_known_sincos_lanes() {
    let data: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../conformance/fountain-v14-saved-scene-inputs.json"
    ))
    .unwrap();
    let mut stroke: Stroke = serde_json::from_value(data["strokes"][1]["stroke"].clone()).unwrap();
    assert_eq!(stroke.points.len(), 87);
    stroke.pen_width = f32::from_bits(0x4111de6a);
    stroke.rendering = Some(StrokeRendering {
        metadata: None,
        pen_name: Some("com.samsung.android.sdk.pen.pen.preload.Pencil2".into()),
        advanced_settings: Some("14;".into()),
        tool_type_raw: 2,
        properties: StrokeProperties {
            compressed: true,
            stylus_channels: true,
            replay_only: false,
            eraser: false,
            fixed_width: false,
            millisecond_timestamps: false,
            top_layer_pen: false,
            alpha_lock: false,
            binary_added: false,
            generated: false,
            fixed_opacity: false,
            rainbow_effect: false,
            straighten: false,
            reveal_mode: false,
        },
        style: StrokeStyle {
            pen_size: Some(stroke.pen_width),
            color_argb: Some(4294425088),
            size_level: Some(0),
            stroke_type_raw: Some(0),
            color_type_raw: Some(0),
            ..Default::default()
        },
    });
    let records = prepare_stroke(&stroke, 1.).unwrap();
    assert_eq!(records.len(), 83);
    let bytes: Vec<u8> = records
        .iter()
        .flat_map(|r| r.lanes.iter().flat_map(|f| f.to_le_bytes()))
        .collect();
    let native = include_bytes!("../../../../../tools/pencil2-second-source/native-geometry.vbo");
    assert_eq!(
        format!("{:x}", Sha256::digest(native)),
        "1119d446324f6f3a61ff231b61b73a154010304e9bf8ebd4fc9e33cddbb1c9ab"
    );
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        "ff88eef234bde541d59b1a69edab340173eb9f84b9124af1427abafd12ce0720"
    );
    assert_eq!(bytes.len(), native.len());
    let differences: Vec<_> = native
        .as_chunks::<4>()
        .0
        .iter()
        .zip(bytes.as_chunks::<4>().0.iter())
        .enumerate()
        .filter_map(|(i, (a, b))| {
            (a != b).then(|| {
                (
                    i / 14,
                    i % 14,
                    u32::from_le_bytes(*a),
                    u32::from_le_bytes(*b),
                )
            })
        })
        .collect();
    assert_eq!(
        differences,
        vec![
            (76, 8, 0x3dec5b1a, 0x3dec5b19),
            (77, 8, 0x3dec5b1a, 0x3dec5b19),
            (78, 8, 0x3dec5b1a, 0x3dec5b19)
        ]
    );
}

// This uses already tracked real-note-derived Fountain coordinates and channels.
// They are not anonymous; normalizing pen metadata does not anonymize handwriting.
// Reference the existing fixtures without copying raw note data into this test.
// The fresh Pencil2 host descriptor is pre-Update, not a verified GPU upload;
// this does not establish saved Pencil2 admission or device/pixel parity.
#[test]
#[allow(clippy::filter_map_bool_then)]
fn note_source_preserves_native_geometry_except_two_known_ulp_lanes() {
    let data: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../conformance/fountain-v14-saved-scene-inputs.json"
    ))
    .unwrap();
    let mut stroke: Stroke = serde_json::from_value(data["strokes"][2]["stroke"].clone()).unwrap();
    assert_eq!(stroke.points.len(), 231);
    stroke.pen_width = f32::from_bits(0x4111de6a);
    stroke.color = None;
    stroke.rendering = Some(StrokeRendering {
        metadata: None,
        pen_name: Some("com.samsung.android.sdk.pen.pen.preload.Pencil2".into()),
        advanced_settings: Some("14;".into()),
        tool_type_raw: 2,
        properties: StrokeProperties {
            compressed: true,
            stylus_channels: true,
            replay_only: false,
            eraser: false,
            fixed_width: false,
            millisecond_timestamps: false,
            top_layer_pen: false,
            alpha_lock: false,
            binary_added: false,
            generated: false,
            fixed_opacity: false,
            rainbow_effect: false,
            straighten: false,
            reveal_mode: false,
        },
        style: StrokeStyle {
            pen_size: Some(stroke.pen_width),
            color_argb: Some(4294425088),
            size_level: Some(0),
            stroke_type_raw: Some(0),
            color_type_raw: Some(0),
            ..Default::default()
        },
    });
    let records = prepare_stroke(&stroke, 1.).unwrap();
    assert_eq!(records.len(), 124);
    let bytes: Vec<u8> = records
        .iter()
        .flat_map(|r| r.lanes.iter().flat_map(|f| f.to_le_bytes()))
        .collect();
    let native = include_bytes!("../../../../../tools/pencil2-mesh-v309/note-native-geometry.vbo");
    assert_eq!(
        format!("{:x}", Sha256::digest(native)),
        "7d88c4f7e2f8fa148de58e515194d59736ceda859abd6297a0de7b330cc987cf"
    );
    assert_eq!(bytes.len(), native.len());
    let differences: Vec<_> = native
        .as_chunks::<4>()
        .0
        .iter()
        .zip(bytes.as_chunks::<4>().0.iter())
        .enumerate()
        .filter_map(|(i, (a, b))| {
            (a != b).then(|| {
                (
                    i / 14,
                    i % 14,
                    u32::from_le_bytes(*a),
                    u32::from_le_bytes(*b),
                )
            })
        })
        .collect();
    // Either known discrepancy may disappear as fidelity improves. Reject
    // every new location or bit pattern, including a larger error at these lanes.
    let allowed = [
        (52, 8, 0x3dd57232, 0x3dd57231),
        (60, 8, 0x3dc8cfaf, 0x3dc8cfae),
    ];
    for difference in differences {
        assert!(
            allowed.contains(&difference),
            "unexpected native/Rust difference: {difference:?}"
        );
    }
}
