use super::offset_candidate;

use super::prepare_stroke;
use crate::{
    BoundingBox, Color, InkRasterCanvas, InkRasterContext, InkRasterInverseScale, InkRasterOrigin,
    Point, Stroke, StrokeRendering, StrokeStyle,
};
fn compare() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "test_assets/drawing/mixed-drawing/receipt.json"
    ))
    .unwrap();
    let mut style = serde_json::to_value(StrokeStyle::default()).unwrap();
    for (k, v) in [
        ("pen_size", serde_json::json!(2.)),
        ("color_argb", serde_json::json!(0x80336699u32)),
        ("size_level", serde_json::json!(0)),
        ("stroke_type_raw", serde_json::json!(0)),
        ("color_type_raw", serde_json::json!(0)),
    ] {
        style[k] = v;
    }
    let props = serde_json::json!({"compressed":true,"stylus_channels":true,"replay_only":false,"eraser":false,"fixed_width":false,"millisecond_timestamps":false,"top_layer_pen":false,"alpha_lock":false,"binary_added":false,"generated":false,"fixed_opacity":false,"rainbow_effect":false,"straighten":false,"reveal_mode":false});
    let rendering:StrokeRendering=serde_json::from_value(serde_json::json!({"metadata":null,"pen_name":"com.samsung.android.sdk.pen.pen.preload.Pencil2","advanced_settings":"14;","tool_type_raw":2,"properties":props,"style":style})).unwrap();
    let s = Stroke {
        bbox: BoundingBox::default(),
        points: vec![
            Point { x: 0., y: 0. },
            Point { x: 10., y: 0. },
            Point { x: 20., y: 0. },
            Point { x: 30., y: 10. },
        ],
        pressures: vec![0.25, 0.5, 0.75, 1.],
        tilts: vec![0.4; 4],
        orientations: vec![0.; 4],
        timestamps: vec![10, 20, 30, 40],
        color: Some(Color {
            r: 0x33,
            g: 0x66,
            b: 0x99,
        }),
        pen_width: 2.,
        rendering: Some(rendering),
    };
    let records = prepare_stroke(&s, 1.).unwrap();
    let actual: String = records
        .iter()
        .flat_map(|r| r.lanes.iter().flat_map(|f| f.to_le_bytes()))
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(
        actual,
        fixture["geometry_data_hex"].as_str().unwrap(),
        "geometry stream"
    );
    let mut c = InkRasterCanvas::new(
        InkRasterContext {
            width: 192,
            height: 192,
            pixels_per_unit: 1,
            origin: InkRasterOrigin::BottomLeft,
        },
        [64, 128, 191, 255],
    )
    .unwrap();
    use std::io::Read;
    let projection = [
        0x3c2aaaab, 0, 0, 0, 0, 0x3c2aaaab, 0, 0, 0, 0, 0xb8000100, 0, 0xbf800000, 0xbf800000,
        0x80000000, 0x3f800000,
    ]
    .map(f32::from_bits);
    let mut grain = offset_candidate::Pencil2V14HostGrainState::default();
    for (translation, offset, frame) in [
        (
            [6., 9.],
            [6., 9.],
            include_bytes!("test_assets/drawing/independent-drawing/first.zlib").as_slice(),
        ),
        (
            [12., 12.],
            [6., 9.],
            include_bytes!("test_assets/drawing/mixed-drawing/second.zlib").as_slice(),
        ),
    ] {
        let mut matrix = [0.; 16];
        for i in 0..4 {
            matrix[5 * i] = 1.;
        }
        matrix[12] = translation[0];
        matrix[13] = translation[1];
        if translation == [6., 9.] {
            grain.set_start_point([s.points[0].x as f32, s.points[0].y as f32]);
        }
        let computed = grain.draw(projection, matrix, [192, 192]).unwrap();
        assert_eq!(computed.map(f32::to_bits), offset.map(f32::to_bits));
        c.paint_pencil2_with_grain_offset(&s, InkRasterInverseScale::One, translation, computed)
            .unwrap();
        let mut raw = Vec::new();
        flate2::read::ZlibDecoder::new(frame)
            .read_to_end(&mut raw)
            .unwrap();
        let mut diff = 0;
        let mut max = 0;
        let mut sum = 0u64;
        for (i, p) in c.pixels().iter().enumerate() {
            let x = i % 192;
            let y = i / 192;
            let n = &raw[((191 - y) * 192 + x) * 4..][..4];
            for k in 0..4 {
                let d = p[k].abs_diff(n[k]);
                if d != 0 {
                    diff += 1;
                    max = max.max(d);
                    sum += u64::from(d);
                }
            }
        }
        assert_eq!(diff, 0, "translation {translation:?}: max={max}, sum={sum}");
    }
}

#[test]
fn mixed_native() {
    compare();
}
