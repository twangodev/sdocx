#![cfg(all(feature = "render", feature = "serde"))]

use sdocx::{
    BoundingBox, Color, InkRasterCanvas, InkRasterContext, InkRasterError, InkRasterInverseScale,
    InkRasterOrigin, Point, Stroke, StrokeRendering, StrokeStyle,
};
use serde_json::{Value, json};

fn stroke(case_id: &str, argb: u32) -> Stroke {
    let fixture: Value =
        serde_json::from_str(include_str!("../../../conformance/pencil2-v14-native.json")).unwrap();
    let input = &fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["id"] == case_id)
        .unwrap()["input"];
    let hex = input["object_stroke_size_f32_bits"].as_str().unwrap();
    let size = f32::from_le_bytes(std::array::from_fn(|i| {
        u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap()
    }));
    let mut style = serde_json::to_value(StrokeStyle::default()).unwrap();
    for (key, value) in [
        ("color_argb", json!(argb)),
        ("pen_size", json!(size)),
        ("size_level", json!(0)),
        ("stroke_type_raw", json!(0)),
        ("color_type_raw", json!(0)),
    ] {
        style[key] = value;
    }
    let rendering:StrokeRendering=serde_json::from_value(json!({
        "metadata":null,"pen_name":"com.samsung.android.sdk.pen.pen.preload.Pencil2","advanced_settings":"14;","tool_type_raw":2,
        "style":style,"properties":{"compressed":true,"stylus_channels":true,"replay_only":false,"eraser":false,"fixed_width":false,"millisecond_timestamps":false,"top_layer_pen":false,"alpha_lock":false,"binary_added":false,"generated":false,"fixed_opacity":false,"rainbow_effect":false,"straighten":false,"reveal_mode":false}
    })).unwrap();
    let channel = |name: &str| {
        input[name]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap())
            .collect()
    };
    Stroke {
        bbox: BoundingBox::default(),
        points: input["points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| Point {
                x: p[0].as_f64().unwrap(),
                y: p[1].as_f64().unwrap(),
            })
            .collect(),
        pressures: channel("pressure"),
        tilts: channel("tilt"),
        orientations: channel("orientation"),
        timestamps: input["timestamps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_i64().unwrap())
            .collect(),
        rendering: Some(rendering),
        color: Some(Color {
            r: (argb >> 16) as u8,
            g: (argb >> 8) as u8,
            b: argb as u8,
        }),
        pen_width: size,
    }
}

fn context(origin: InkRasterOrigin, scale: u32) -> InkRasterContext {
    InkRasterContext {
        width: 192,
        height: 192,
        pixels_per_unit: scale,
        origin,
    }
}

#[test]
fn grain_offsets_are_per_submission_and_atomic() {
    let source = stroke("channel_baseline", 0x80336699);
    for origin in [InkRasterOrigin::BottomLeft, InkRasterOrigin::TopLeft] {
        let make = || InkRasterCanvas::new(context(origin, 1), [0; 4]).unwrap();
        let mut old = make();
        let mut zero = make();
        old.paint_pencil2_translated(&source, InkRasterInverseScale::One, [6., 9.])
            .unwrap();
        zero.paint_pencil2_with_grain_offset(
            &source,
            InkRasterInverseScale::One,
            [6., 9.],
            [0., 0.],
        )
        .unwrap();
        assert_eq!(old.pixels(), zero.pixels());
        for offset in [[6., 9.], [-7.25, 4.5], [0.125, -9.75]] {
            let mut candidate = make();
            candidate
                .paint_pencil2_with_grain_offset(
                    &source,
                    InkRasterInverseScale::One,
                    [6., 9.],
                    offset,
                )
                .unwrap();
            assert_ne!(candidate.pixels(), zero.pixels(), "{origin:?} {offset:?}");
            let mut repeated = make();
            repeated
                .paint_pencil2_with_grain_offset(
                    &source,
                    InkRasterInverseScale::One,
                    [6., 9.],
                    offset,
                )
                .unwrap();
            repeated
                .paint_pencil2_translated(&source, InkRasterInverseScale::One, [6., 9.])
                .unwrap();
            candidate
                .paint_pencil2_with_grain_offset(
                    &source,
                    InkRasterInverseScale::One,
                    [6., 9.],
                    [0., 0.],
                )
                .unwrap();
            assert_eq!(candidate.pixels(), repeated.pixels());
        }
        let before = zero.pixels().to_vec();
        for offset in [
            [f32::NAN, 0.],
            [0., f32::INFINITY],
            [16_384.5, 0.],
            [0., -16_384.5],
        ] {
            assert!(matches!(
                zero.paint_pencil2_with_grain_offset(
                    &source,
                    InkRasterInverseScale::One,
                    [0., 0.],
                    offset
                ),
                Err(InkRasterError::InvalidContext)
            ));
            assert_eq!(zero.pixels(), before);
        }
        let mut expensive = source.clone();
        expensive.points.last_mut().unwrap().x = 100_000.;
        assert!(matches!(
            zero.paint_pencil2_with_grain_offset(
                &expensive,
                InkRasterInverseScale::One,
                [0., 0.],
                [6., 9.]
            ),
            Err(InkRasterError::WorkLimit)
        ));
        assert_eq!(zero.pixels(), before);
        let mut reference = make();
        reference
            .paint_pencil2_translated(&source, InkRasterInverseScale::One, [6., 9.])
            .unwrap();
        reference
            .paint_pencil2_with_grain_offset(
                &source,
                InkRasterInverseScale::One,
                [0., 0.],
                [6., 9.],
            )
            .unwrap();
        zero.paint_pencil2_with_grain_offset(
            &source,
            InkRasterInverseScale::One,
            [0., 0.],
            [6., 9.],
        )
        .unwrap();
        assert_eq!(zero.pixels(), reference.pixels());
    }
}

#[test]
fn rejected_offsets_do_not_consume_submission_budget() {
    let source = stroke("channel_baseline", 0x80336699);
    let mut canvas = InkRasterCanvas::new(
        InkRasterContext {
            width: 1,
            height: 1,
            ..context(InkRasterOrigin::BottomLeft, 1)
        },
        [0; 4],
    )
    .unwrap();
    assert!(matches!(
        canvas.paint_pencil2_with_grain_offset(
            &source,
            InkRasterInverseScale::One,
            [0., 0.],
            [f32::NAN, 0.]
        ),
        Err(InkRasterError::InvalidContext)
    ));
    let mut bad = source.clone();
    bad.points.last_mut().unwrap().x = 100_000.;
    assert!(matches!(
        canvas.paint_pencil2_with_grain_offset(
            &bad,
            InkRasterInverseScale::One,
            [0., 0.],
            [6., 9.]
        ),
        Err(InkRasterError::WorkLimit)
    ));
    for _ in 0..256 {
        canvas
            .paint_pencil2_with_grain_offset(
                &source,
                InkRasterInverseScale::One,
                [0., 0.],
                [6., 9.],
            )
            .unwrap();
    }
    let before = canvas.pixels().to_vec();
    assert!(matches!(
        canvas.paint_pencil2_with_grain_offset(
            &source,
            InkRasterInverseScale::One,
            [0., 0.],
            [6., 9.]
        ),
        Err(InkRasterError::WorkLimit)
    ));
    assert_eq!(canvas.pixels(), before);
}
