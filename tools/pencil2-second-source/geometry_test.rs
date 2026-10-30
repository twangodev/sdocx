use super::prepare_stroke;
use crate::Stroke;
use sha2::{Digest, Sha256};
#[test]
fn serialized_geometry_dump() {
    let data: serde_json::Value = serde_json::from_slice(&std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../conformance/fountain-v14-saved-scene-inputs.json")).unwrap()).unwrap();
    let mut stroke: Stroke = serde_json::from_value(data["strokes"][1]["stroke"].clone()).unwrap();
    stroke.pen_width = f32::from_bits(0x4111de6a) as _;
    stroke.rendering.as_mut().unwrap().style.color_argb = Some(4294425088);
    stroke.rendering.as_mut().unwrap().pen_name = Some("com.samsung.android.sdk.pen.pen.preload.Pencil2".into());
    {
    let source_rendering = stroke.rendering.as_ref().unwrap();
    let mut style = serde_json::to_value(crate::StrokeStyle::default()).unwrap();
    style["pen_size"] = serde_json::json!(stroke.pen_width);
    style["color_argb"] = serde_json::json!(source_rendering.style.color_argb);
    style["size_level"] = serde_json::json!(0); style["stroke_type_raw"] = serde_json::json!(0); style["color_type_raw"] = serde_json::json!(0);
    let props = serde_json::json!({"compressed":true,"stylus_channels":true,"replay_only":false,"eraser":false,"fixed_width":false,"millisecond_timestamps":false,"top_layer_pen":false,"alpha_lock":false,"binary_added":false,"generated":false,"fixed_opacity":false,"rainbow_effect":false,"straighten":false,"reveal_mode":false});
    stroke.rendering = Some(serde_json::from_value(serde_json::json!({"metadata":null,"pen_name":"com.samsung.android.sdk.pen.pen.preload.Pencil2","advanced_settings":"14;","tool_type_raw":2,"properties":props,"style":style})).unwrap());
    }
let records = prepare_stroke(&stroke, 1.).unwrap();
    assert_eq!(records.len(), 83);
    let bytes: Vec<u8> = records.iter().flat_map(|r| r.lanes.iter().flat_map(|f| f.to_le_bytes())).collect();
    let native = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tools/pencil2-second-source/native-geometry.vbo"));
    assert_eq!(format!("{:x}", Sha256::digest(native)), "1119d446324f6f3a61ff231b61b73a154010304e9bf8ebd4fc9e33cddbb1c9ab");
    assert_eq!(format!("{:x}", Sha256::digest(&bytes)), "ff88eef234bde541d59b1a69edab340173eb9f84b9124af1427abafd12ce0720");
    assert_eq!(bytes.len(), native.len());
    let differences: Vec<_> = native.chunks_exact(4).zip(bytes.chunks_exact(4)).enumerate()
        .filter_map(|(i, (a, b))| (a != b).then(|| (i / 14, i % 14, u32::from_le_bytes(a.try_into().unwrap()), u32::from_le_bytes(b.try_into().unwrap())))).collect();
    assert_eq!(differences, vec![(76, 8, 0x3dec5b1a, 0x3dec5b19), (77, 8, 0x3dec5b1a, 0x3dec5b19), (78, 8, 0x3dec5b1a, 0x3dec5b19)]);
}
