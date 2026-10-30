use super::prepare_stroke;
use crate::Stroke;
#[test]
fn terminal_opacity_matches_native_interpolation() {
    let data: serde_json::Value =
        serde_json::from_slice(include_bytes!("assets/terminal-source.json")).unwrap();
    let mut data = data;
    let stroke_data = &mut data["strokes"][0]["stroke"];
    let xs = [80.0, 90.0, 100.0, 110.0, 120.0, 130.0, 140.0, 140.3];
    stroke_data["points"] = serde_json::json!(xs.map(|x| serde_json::json!({"x": x, "y": 100.0})));
    stroke_data["pressures"] = serde_json::json!([0.8, 0.8, 0.8, 0.8, 0.8, 0.8, 0.8, 0.0]);
    for field in ["timestamps", "tilts", "orientations"] {
        stroke_data[field].as_array_mut().unwrap().truncate(8);
    }
    let mut stroke: Stroke = serde_json::from_value(data["strokes"][0]["stroke"].clone()).unwrap();
    stroke.rendering.as_mut().unwrap().pen_name =
        Some("com.samsung.android.sdk.pen.pen.preload.Pencil2".into());
    {
        let source_rendering = stroke.rendering.as_ref().unwrap();
        let mut style = serde_json::to_value(crate::StrokeStyle::default()).unwrap();
        style["pen_size"] = serde_json::json!(stroke.pen_width);
        style["color_argb"] = serde_json::json!(source_rendering.style.color_argb);
        style["size_level"] = serde_json::json!(0);
        style["stroke_type_raw"] = serde_json::json!(0);
        style["color_type_raw"] = serde_json::json!(0);
        let props = serde_json::json!({"compressed":true,"stylus_channels":true,"replay_only":false,"eraser":false,"fixed_width":false,"millisecond_timestamps":false,"top_layer_pen":false,"alpha_lock":false,"binary_added":false,"generated":false,"fixed_opacity":false,"rainbow_effect":false,"straighten":false,"reveal_mode":false});
        stroke.rendering = Some(serde_json::from_value(serde_json::json!({"metadata":null,"pen_name":"com.samsung.android.sdk.pen.pen.preload.Pencil2","advanced_settings":"14;","tool_type_raw":2,"properties":props,"style":style})).unwrap());
    }
    let records = prepare_stroke(&stroke, 1.).unwrap();
    assert_eq!(records.last().unwrap().lanes[4].to_bits(), 0x380b_a400);
    let bytes: Vec<u8> = records
        .iter()
        .flat_map(|r| r.lanes.iter().flat_map(|f| f.to_le_bytes()))
        .collect();
    assert_eq!(bytes, include_bytes!("assets/terminal-zero.vbo"));
}

#[test]
fn terminal_variants_match_native_geometry() {
    let cases: [([f64; 8], &[u8]); 3] = [
        (
            [80.0, 90.0, 100.0, 110.0, 120.0, 130.0, 140.0, 140.1],
            include_bytes!("assets/terminal-short.vbo"),
        ),
        (
            [80.0, 90.0, 100.0, 110.0, 120.0, 130.0, 140.0, 145.0],
            include_bytes!("assets/terminal-long.vbo"),
        ),
        (
            [140.0, 130.0, 120.0, 110.0, 100.0, 90.0, 80.0, 79.7],
            include_bytes!("assets/terminal-reverse.vbo"),
        ),
    ];
    for (xs, expected) in cases {
        let data: serde_json::Value =
            serde_json::from_slice(include_bytes!("assets/terminal-source.json")).unwrap();
        let mut data = data;
        let stroke_data = &mut data["strokes"][0]["stroke"];
        stroke_data["points"] =
            serde_json::json!(xs.map(|x| serde_json::json!({"x": x, "y": 100.0})));
        stroke_data["pressures"] = serde_json::json!([0.8, 0.8, 0.8, 0.8, 0.8, 0.8, 0.8, 0.0]);
        for field in ["timestamps", "tilts", "orientations"] {
            stroke_data[field].as_array_mut().unwrap().truncate(8);
        }
        let mut stroke: Stroke =
            serde_json::from_value(data["strokes"][0]["stroke"].clone()).unwrap();
        stroke.rendering.as_mut().unwrap().pen_name =
            Some("com.samsung.android.sdk.pen.pen.preload.Pencil2".into());
        {
            let source_rendering = stroke.rendering.as_ref().unwrap();
            let mut style = serde_json::to_value(crate::StrokeStyle::default()).unwrap();
            style["pen_size"] = serde_json::json!(stroke.pen_width);
            style["color_argb"] = serde_json::json!(source_rendering.style.color_argb);
            style["size_level"] = serde_json::json!(0);
            style["stroke_type_raw"] = serde_json::json!(0);
            style["color_type_raw"] = serde_json::json!(0);
            let props = serde_json::json!({"compressed":true,"stylus_channels":true,"replay_only":false,"eraser":false,"fixed_width":false,"millisecond_timestamps":false,"top_layer_pen":false,"alpha_lock":false,"binary_added":false,"generated":false,"fixed_opacity":false,"rainbow_effect":false,"straighten":false,"reveal_mode":false});
            stroke.rendering = Some(serde_json::from_value(serde_json::json!({"metadata":null,"pen_name":"com.samsung.android.sdk.pen.pen.preload.Pencil2","advanced_settings":"14;","tool_type_raw":2,"properties":props,"style":style})).unwrap());
        }
        let records = prepare_stroke(&stroke, 1.).unwrap();
        let bytes: Vec<u8> = records
            .iter()
            .flat_map(|r| r.lanes.iter().flat_map(|f| f.to_le_bytes()))
            .collect();
        assert_eq!(bytes, expected);
    }
}
