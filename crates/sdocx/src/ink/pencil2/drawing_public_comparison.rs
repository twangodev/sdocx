use super::prepare_stroke;
use crate::{
    BoundingBox, Color, InkRasterCanvas, InkRasterContext, InkRasterInverseScale, InkRasterOrigin,
    Point, Stroke, StrokeRendering, StrokeStyle,
};
fn compare(
    translation: [f32; 2],
    offset: [f32; 2],
    frame: &[u8],
    origin: InkRasterOrigin,
    scale: u32,
) {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "test_assets/drawing/public-replay/drawing-native-target-translated-receipt.json"
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
        fixture["streams"]["draw_object_stroke"]["data_hex"]
            .as_str()
            .unwrap(),
        "geometry stream"
    );
    let mut legacy = InkRasterCanvas::new(
        InkRasterContext {
            width: 192,
            height: 192,
            pixels_per_unit: scale,
            origin,
        },
        [0; 4],
    )
    .unwrap();
    legacy
        .paint_pencil2_translated(&s, InkRasterInverseScale::One, translation)
        .unwrap();
    let mut zero = InkRasterCanvas::new(
        InkRasterContext {
            width: 192,
            height: 192,
            pixels_per_unit: scale,
            origin,
        },
        [0; 4],
    )
    .unwrap();
    zero.paint_pencil2_with_grain_offset(&s, InkRasterInverseScale::One, translation, [0., 0.])
        .unwrap();
    assert_eq!(legacy.pixels(), zero.pixels());
    let mut c = InkRasterCanvas::new(
        InkRasterContext {
            width: 192,
            height: 192,
            pixels_per_unit: scale,
            origin,
        },
        [0; 4],
    )
    .unwrap();
    c.paint_pencil2_with_grain_offset(&s, InkRasterInverseScale::One, translation, offset)
        .unwrap();
    use std::io::Read;
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
                sum += u64::from(d)
            }
        }
    }
    assert!(
        diff <= usize::from(origin == InkRasterOrigin::TopLeft && scale == 1) && max <= 1,
        "{origin:?} scale {scale}: diff={diff} max={max} sum={sum}"
    );
    println!(
        "stream={} records; rgba channel differences={diff}, max={max}, sum={sum}",
        records.len()
    );
}

#[test]
fn drawing_public_comparison() {
    compare(
        [32., 32.],
        [f32::from_bits(32f32.to_bits() + 1); 2],
        include_bytes!(
            "test_assets/drawing/public-replay/drawing-native-target-translated.rgba.zlib"
        ),
        InkRasterOrigin::BottomLeft,
        1,
    );
    compare(
        [6., 9.],
        [6., 9.],
        include_bytes!("test_assets/drawing/origin-offset/bottom-1.zlib"),
        InkRasterOrigin::BottomLeft,
        1,
    );
}

#[test]
fn drawing_origin_offset_comparison() {
    for (origin, scale, offset, frame) in [
        (
            InkRasterOrigin::BottomLeft,
            1,
            [6., 9.],
            &include_bytes!("test_assets/drawing/origin-offset/bottom-1.zlib")[..],
        ),
        (
            InkRasterOrigin::TopLeft,
            1,
            [6., 151.],
            &include_bytes!("test_assets/drawing/origin-offset/top-1.zlib")[..],
        ),
        (
            InkRasterOrigin::BottomLeft,
            2,
            [6., 9.],
            &include_bytes!("test_assets/drawing/origin-offset/bottom-2.zlib")[..],
        ),
        (
            InkRasterOrigin::TopLeft,
            2,
            [6., 151.],
            &include_bytes!("test_assets/drawing/origin-offset/top-2.zlib")[..],
        ),
    ] {
        compare(
            [
                6.,
                if origin == InkRasterOrigin::TopLeft {
                    -41.
                } else {
                    9.
                },
            ],
            offset,
            frame,
            origin,
            scale,
        );
    }
}
