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
    for (index, translation, offset, frame) in [
        (
            0,
            [6., 9.],
            [6., 9.],
            include_bytes!("test_assets/drawing/independent-drawing/first.zlib").as_slice(),
        ),
        (
            1,
            [12., 12.],
            [12., 13.999_998],
            include_bytes!("test_assets/drawing/independent-drawing/second.zlib").as_slice(),
        ),
    ] {
        let mut current = s.clone();
        if index == 1 {
            current.points = vec![
                Point { x: 0., y: 2. },
                Point { x: 10., y: 2. },
                Point { x: 20., y: 4. },
                Point { x: 30., y: 12. },
            ];
            current.pressures = vec![0.9, 0.7, 0.5, 0.3];
            current.pen_width = 10.;
            current.color = Some(Color {
                r: 0xcc,
                g: 0x33,
                b: 0x44,
            });
            let mut r = serde_json::to_value(current.rendering.as_ref().unwrap()).unwrap();
            r["style"]["pen_size"] = serde_json::json!(10.);
            r["style"]["color_argb"] = serde_json::json!(0xc0cc3344u32);
            current.rendering = Some(serde_json::from_value(r).unwrap());
        }
        if index == 1 {
            let geometry: Vec<u8> = prepare_stroke(&current, 1.)
                .unwrap()
                .iter()
                .flat_map(|record| record.lanes.iter().flat_map(|lane| lane.to_le_bytes()))
                .collect();
            assert_eq!(
                geometry,
                include_bytes!("test_assets/drawing/independent-drawing/second.vbo")
            );
        }
        let mut matrix = [0.; 16];
        for i in 0..4 {
            matrix[5 * i] = 1.;
        }
        matrix[12] = translation[0];
        matrix[13] = translation[1];
        grain.set_start_point([current.points[0].x as f32, current.points[0].y as f32]);
        let computed = grain.draw(projection, matrix, [192, 192]).unwrap();
        assert_eq!(computed.map(f32::to_bits), offset.map(f32::to_bits));
        c.paint_pencil2_with_grain_offset(
            &current,
            InkRasterInverseScale::One,
            translation,
            computed,
        )
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
        assert_eq!(diff, 0, "frame {index}: max={max} sum={sum}");
        let page = c.rendered_page(0).unwrap();
        let tree = resvg::usvg::Tree::from_str(&page.svg, &Default::default()).unwrap();
        let mut rendered = resvg::tiny_skia::Pixmap::new(192, 192).unwrap();
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::identity(),
            &mut rendered.as_mut(),
        );
        assert_eq!(
            rendered.data().as_chunks::<4>().0,
            c.pixels(),
            "SVG prefix {index}"
        );
    }
}

#[test]
fn independent_native() {
    compare();
}

#[test]
fn worker_v259_geometry_probe() {
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
    let data: Vec<u8> = records
        .iter()
        .flat_map(|r| r.lanes.iter().flat_map(|v| v.to_le_bytes()))
        .collect();
    assert_eq!(
        data,
        include_bytes!("../../../../../tools/pencil2-worker-v259/native-geometry.vbo")
    );
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let hash = |bytes: &[u8]| format!("{:x}", Sha256::digest(bytes));
    let compressed = include_bytes!("../../../../../tools/pencil2-worker-v256/rgba8-frame.zlib");
    assert_eq!(
        hash(compressed),
        "4f33e63b6c260b7fecc281ccf5891b19b3a96403b552da3a757e6faf2ca8e983"
    );
    let mut bottom = Vec::new();
    flate2::read::ZlibDecoder::new(compressed.as_slice())
        .read_to_end(&mut bottom)
        .unwrap();
    assert_eq!(bottom.len(), 192 * 192 * 4);
    assert_eq!(
        hash(&bottom),
        "a0230ba18bf716851b4079facb415b265ade62b80ef28bbfcaa0b1853776672e"
    );
    let native: Vec<u8> = bottom
        .as_chunks::<{ 192 * 4 }>()
        .0
        .iter()
        .rev()
        .flatten()
        .copied()
        .collect();
    assert_eq!(
        hash(&native),
        "44e57b9754e76dd9883c7a9944a9015f0ccb0c11e788dd71d8c5f1a889174e36"
    );
    assert_eq!(
        native
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[3] != 0)
            .count(),
        87
    );
    let projection = [
        0x3c2aaaab, 0, 0, 0, 0, 0x3c2aaaab, 0, 0, 0, 0, 0xb8000100, 0, 0xbf800000, 0xbf800000,
        0x80000000, 0x3f800000,
    ]
    .map(f32::from_bits);
    let mut matrix = [0.; 16];
    for i in 0..4 {
        matrix[5 * i] = 1.;
    }
    matrix[5] = -1.;
    matrix[12] = 6.;
    matrix[13] = 183.;
    let mut grain = crate::Pencil2V14HostGrainState::default();
    grain.set_start_point([s.points[0].x as f32, s.points[0].y as f32]);
    let offset = grain.draw(projection, matrix, [192, 192]).unwrap();
    assert_eq!(offset.map(f32::to_bits), [0x40c00000, 0x43370000]);
    let mut canvas = InkRasterCanvas::new(
        InkRasterContext {
            width: 192,
            height: 192,
            pixels_per_unit: 1,
            origin: InkRasterOrigin::TopLeft,
        },
        [0; 4],
    )
    .unwrap();
    canvas
        .paint_pencil2_with_grain_offset(&s, InkRasterInverseScale::One, [6., -9.], offset)
        .unwrap();
    let actual: Vec<u8> = canvas.pixels().iter().flatten().copied().collect();
    assert_eq!(actual, native);
    let png = resvg::tiny_skia::Pixmap::decode_png(&canvas.to_png().unwrap()).unwrap();
    assert_eq!((png.width(), png.height()), (192, 192));
    let page = canvas.rendered_page(0).unwrap();
    let tree = resvg::usvg::Tree::from_str(&page.svg, &Default::default()).unwrap();
    let mut svg = resvg::tiny_skia::Pixmap::new(192, 192).unwrap();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut svg.as_mut(),
    );
    // Readers may premultiply RGB differently; the historical export control checked alpha.
    for image in [&png, &svg] {
        assert_eq!(
            image
                .data()
                .as_chunks::<4>()
                .0
                .iter()
                .map(|p| p[3])
                .collect::<Vec<_>>(),
            native
                .as_chunks::<4>()
                .0
                .iter()
                .map(|p| p[3])
                .collect::<Vec<_>>()
        );
    }
    #[cfg(feature = "pdf")]
    {
        let mut options = crate::PdfOptions::default();
        options.dpi = 72.;
        canvas.to_pdf(&options).unwrap();
    }
}
