#![cfg(feature = "render")]
use sdocx::{
    BoundingBox, Color, Document, ObjectType, Page, ParseLimits, RenderOptions, StoredObject,
    render_document_svg,
};
fn stroke(argb: u32) -> sdocx::Stroke {
    let mut b = include_bytes!(
        "../../../conformance/old-pencil-counterexample/serialized-pencil-color.payload"
    )
    .to_vec();
    b[2627..2631].copy_from_slice(&argb.to_le_bytes());
    let o = StoredObject {
        object_type: ObjectType::Stroke,
        declared_size: (b.len() + 32) as u32,
        payload_offset: 0,
        payload_size: b.len(),
        integrity_trailer: [0; 32],
        children: vec![],
    };
    o.decode_stroke(&b, &ParseLimits::default()).unwrap()
}
fn doc(stroke: sdocx::Stroke) -> Document {
    Document {
        pages: vec![Page {
            uuid: "test".into(),
            width: 1920,
            height: 1080,
            objects: vec![stroke.into()],
            background_color: Some(Color {
                r: 255,
                g: 255,
                b: 255,
            }),
            content_bbox: BoundingBox::default(),
            template: None,
            background: Default::default(),
        }],
        metadata: Default::default(),
    }
}
fn svg(doc: &Document, opt: &RenderOptions) -> String {
    render_document_svg(doc, opt).remove(0).svg
}
#[test]
fn explicit_profile_and_fallback_guards() {
    let plain = RenderOptions::default();
    let mut opt = plain.clone();
    opt.old_pencil_svg_profile = true;
    for (name, color) in [("blue", 0x8024a0d8), ("alt", 0x40f130a0)] {
        let d = doc(stroke(color));
        let baseline = svg(&d, &plain);
        let candidate = svg(&d, &opt);
        assert!(!baseline.contains("crispEdges"));
        assert!(candidate.contains("crispEdges"));
        let _ = (name, candidate);
        #[cfg(feature = "pdf")]
        assert_eq!(
            sdocx::render_document_pdf(&d, &plain, &sdocx::PdfOptions::default()).unwrap(),
            sdocx::render_document_pdf(&d, &opt, &sdocx::PdfOptions::default()).unwrap()
        );
    }
    let mut bad = stroke(0x8024a0d8);
    bad.rendering.as_mut().unwrap().pen_name = Some("other".into());
    let d = doc(bad);
    assert_eq!(svg(&d, &plain), svg(&d, &opt));
    let mut bad = stroke(0x8024a0d8);
    bad.rendering.as_mut().unwrap().style.stroke_type_raw = Some(1);
    let d = doc(bad);
    assert_eq!(svg(&d, &plain), svg(&d, &opt));
    let mut bad = stroke(0x8024a0d8);
    bad.rendering
        .as_mut()
        .unwrap()
        .metadata
        .as_mut()
        .unwrap()
        .rotation_degrees = Some(1.);
    let d = doc(bad);
    assert_eq!(svg(&d, &plain), svg(&d, &opt));
    let mut d = doc(stroke(0x8024a0d8));
    d.pages[0].width = 1919;
    assert_eq!(svg(&d, &plain), svg(&d, &opt));
}

#[test]
fn bounds_and_page_budget_fail_closed() {
    let mut opt = RenderOptions::default();
    opt.old_pencil_svg_profile = true;
    let plain = RenderOptions::default();
    for value in [f64::NAN, 1e38] {
        let mut bad = stroke(0x8024a0d8);
        bad.points[0].x = value;
        let d = doc(bad);
        assert_eq!(svg(&d, &opt), svg(&d, &plain));
    }
    let mut long = stroke(0x8024a0d8);
    long.points = vec![
        sdocx::Point { x: 1.0, y: 1.0 },
        sdocx::Point { x: 1919.0, y: 1.0 },
    ];
    long.pressures = vec![0.5; 2];
    long.timestamps = vec![0; 2];
    long.tilts = vec![0.0; 2];
    long.orientations = vec![0.0; 2];
    let d = doc(long);
    assert_eq!(svg(&d, &opt), svg(&d, &plain));
    let mut d = doc(stroke(0x8024a0d8));
    d.pages[0].objects = (0..17).map(|_| stroke(0x8024a0d8).into()).collect();
    let output = svg(&d, &opt);
    let one = svg(&doc(stroke(0x8024a0d8)), &opt);
    let n = one.matches("shape-rendering=\"crispEdges\"").count();
    assert!(n > 0);
    assert_eq!(
        output.matches("shape-rendering=\"crispEdges\"").count(),
        16 * n
    );
}
#[test]
fn rejected_attempts_consume_source_budget() {
    let mut opt = RenderOptions::default();
    opt.old_pencil_svg_profile = true;
    let mut attempted = stroke(0x8024a0d8);
    let p = attempted.points[0];
    attempted.points = vec![p; 4096];
    attempted.pressures = vec![0.5; 4096];
    attempted.tilts = vec![0.0; 4096];
    attempted.orientations = vec![0.0; 4096];
    attempted.timestamps = vec![0; 4096];
    attempted.pressures[4095] = f64::NAN;
    let mut d = doc(stroke(0x8024a0d8));
    d.pages[0].objects = vec![
        attempted.clone().into(),
        attempted.into(),
        stroke(0x8024a0d8).into(),
    ];
    assert!(!svg(&d, &opt).contains("crispEdges"));
}
#[test]
fn unrelated_large_stroke_does_not_consume_pencil_work_budget() {
    let mut other = stroke(0x8024a0d8);
    other.rendering.as_mut().unwrap().pen_name = Some("unrelated".into());
    let point = other.points[0];
    other.points = vec![point; 9000];
    other.pressures = vec![0.5; 9000];
    other.tilts = vec![0.0; 9000];
    other.orientations = vec![0.0; 9000];
    other.timestamps = vec![0; 9000];
    let mut d = doc(stroke(0x8024a0d8));
    d.pages[0].objects = vec![other.into(), stroke(0x8024a0d8).into()];
    let mut opt = RenderOptions::default();
    opt.old_pencil_svg_profile = true;
    assert!(svg(&d, &opt).contains("crispEdges"));
}

#[cfg(feature = "serde")]
#[test]
fn absent_profile_deserializes_off() {
    let mut json = serde_json::to_value(RenderOptions::default()).unwrap();
    json.as_object_mut()
        .unwrap()
        .remove("old_pencil_svg_profile");
    let options: RenderOptions = serde_json::from_value(json).unwrap();
    assert!(!options.old_pencil_svg_profile);
}

#[test]
fn atlas_matches_retained_native_upload() {
    use sha2::{Digest, Sha256};
    let bytes = include_bytes!("../src/render/old_pencil_atlas.bin");
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "5149e5daea6e172fb1798904209904676bcb49da168993657868c0ef4858d440"
    );
}

#[test]
fn synthetic_serialized_controls_track_retained_native_frames() {
    use std::io::Read;
    fn decompress(data: &[u8]) -> Vec<u8> {
        let mut output = Vec::new();
        flate2::read::ZlibDecoder::new(data)
            .read_to_end(&mut output)
            .unwrap();
        output
    }
    let pre = decompress(include_bytes!(
        "../../../conformance/old-pencil-color/varied-pre.rgba.zlib"
    ));
    assert_eq!(pre.len(), 1920 * 1080 * 4);
    assert!(pre.as_chunks::<4>().0.iter().all(|pixel| pixel[3] == 255));
    let cases = [
        (0x8024a0d8, include_bytes!("../../../conformance/old-pencil-counterexample/serialized-pencil-color-post.rgba.zlib").as_slice(), 469u64, 420usize),
        (0x40f130a0, include_bytes!("../../../conformance/old-pencil-counterexample/serialized-pencil-color-alt-post.rgba.zlib").as_slice(), 598u64, 417usize),
    ];
    let mut opt = RenderOptions::default();
    opt.old_pencil_svg_profile = true;
    for (color, expected, expected_error, expected_union) in cases {
        let source = svg(&doc(stroke(color)), &opt);
        let body = &source[source.find("<defs").unwrap()..source.rfind("</svg>").unwrap()];
        let transported = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1920\" height=\"1080\"><g transform=\"translate(0 1080) scale(1 -1)\">{body}</g></svg>"
        );
        let tree = resvg::usvg::Tree::from_str(&transported, &Default::default()).unwrap();
        let mut pixmap = resvg::tiny_skia::Pixmap::new(1920, 1080).unwrap();
        pixmap.data_mut().copy_from_slice(&pre);
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::identity(),
            &mut pixmap.as_mut(),
        );
        let native = decompress(expected);
        assert_eq!(native.len(), 1920 * 1080 * 4);
        assert_eq!(pixmap.data().len(), 1920 * 1080 * 4);
        let mut union = 0;
        let mut error = 0u64;
        let mut max_error = 0u8;
        for ((actual, expected), prior) in pixmap
            .data()
            .as_chunks::<4>()
            .0
            .iter()
            .zip(native.as_chunks::<4>().0.iter())
            .zip(pre.as_chunks::<4>().0.iter())
        {
            if actual != prior || expected != prior {
                union += 1;
            }
            for i in 0..3 {
                let difference = actual[i].abs_diff(expected[i]);
                error += u64::from(difference);
                max_error = max_error.max(difference);
            }
        }
        assert_eq!(
            (union, error, max_error),
            (expected_union, expected_error, 2)
        );
    }
}

#[test]
fn synthetic_serialized_size8_svg_tracks_native() {
    use std::io::Read;
    let decode = |bytes: &[u8]| {
        let mut out = Vec::new();
        flate2::read::ZlibDecoder::new(bytes)
            .read_to_end(&mut out)
            .unwrap();
        out
    };
    let pre = decode(include_bytes!(
        "../../../conformance/old-pencil-color/varied-pre.rgba.zlib"
    ));
    assert_eq!(pre.len(), 1920 * 1080 * 4);
    let cases: [(bool, &[u8], usize, u64); 2] = [
        (
            false,
            include_bytes!(
                "../../../conformance/old-pencil-counterexample/serialized8-post.rgba.zlib"
            ),
            367,
            384,
        ),
        (
            true,
            include_bytes!(
                "../../../conformance/old-pencil-counterexample/serialized8-reflect-1.rgba.zlib"
            ),
            355,
            411,
        ),
    ];
    for (reflected, post, expected_union, expected_error) in cases {
        let mut payload = include_bytes!(
            "../../../conformance/old-pencil-counterexample/serialized-pencil-color.payload"
        )
        .to_vec();
        for offset in [2631, 2639] {
            assert_eq!(&payload[offset..offset + 4], &9.1168_f32.to_le_bytes());
            payload[offset..offset + 4].copy_from_slice(&8.0_f32.to_le_bytes());
        }
        if reflected {
            for i in 0..203 {
                payload[173 + i * 4 + 1] ^= 0x80;
            }
        }
        let object = StoredObject {
            object_type: ObjectType::Stroke,
            declared_size: (payload.len() + 32) as u32,
            payload_offset: 0,
            payload_size: payload.len(),
            integrity_trailer: [0; 32],
            children: vec![],
        };
        let stroke = object
            .decode_stroke(&payload, &ParseLimits::default())
            .unwrap();
        assert_eq!(stroke.pen_width, 8.0);
        let mut opt = RenderOptions::default();
        opt.old_pencil_svg_profile = true;
        let source = svg(&doc(stroke), &opt);
        assert!(source.contains("crispEdges"));
        let body = &source[source.find("<defs").unwrap()..source.rfind("</svg>").unwrap()];
        let transported = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1920\" height=\"1080\"><g transform=\"translate(0 1080) scale(1 -1)\">{body}</g></svg>"
        );
        let tree = resvg::usvg::Tree::from_str(&transported, &Default::default()).unwrap();
        let mut pixmap = resvg::tiny_skia::Pixmap::new(1920, 1080).unwrap();
        pixmap.data_mut().copy_from_slice(&pre);
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::identity(),
            &mut pixmap.as_mut(),
        );
        let native = decode(post);
        assert_eq!(native.len(), pre.len());
        assert_eq!(pixmap.data().len(), pre.len());
        let mut union = 0;
        let mut error = 0u64;
        for ((actual, expected), prior) in pixmap
            .data()
            .as_chunks::<4>()
            .0
            .iter()
            .zip(native.as_chunks::<4>().0.iter())
            .zip(pre.as_chunks::<4>().0.iter())
        {
            if actual != prior || expected != prior {
                union += 1;
            }
            for i in 0..3 {
                error += u64::from(actual[i].abs_diff(expected[i]));
            }
        }
        assert_eq!((union, error), (expected_union, expected_error));
    }
}

#[test]
fn synthetic_serialized_unobserved_size_uses_fallback() {
    for size in [
        f32::from_bits(3.0_f32.to_bits() - 1),
        f32::from_bits(12.0_f32.to_bits() + 1),
        f32::NAN,
        -1.0,
    ] {
        let mut s = stroke(0x8024a0d8);
        s.pen_width = size;
        let style = &mut s.rendering.as_mut().unwrap().style;
        style.pen_size = Some(size);
        style.fixed_width = Some(size);
        let d = doc(s);
        let plain = RenderOptions::default();
        let mut opt = plain.clone();
        opt.old_pencil_svg_profile = true;
        assert_eq!(svg(&d, &plain), svg(&d, &opt), "size {size:?}");
    }
}

#[test]
fn synthetic_serialized_size5_svg_tracks_native() {
    use std::io::Read;
    let decode = |bytes: &[u8]| {
        let mut out = Vec::new();
        flate2::read::ZlibDecoder::new(bytes)
            .read_to_end(&mut out)
            .unwrap();
        out
    };
    let cases: [(bool, &[u8], usize, u64); 2] = [
        (
            false,
            include_bytes!("../../../conformance/old-pencil-counterexample/serialized5-plain-1.z"),
            224,
            357,
        ),
        (
            true,
            include_bytes!(
                "../../../conformance/old-pencil-counterexample/serialized5-reflect-1.z"
            ),
            228,
            295,
        ),
    ];
    for (reflected, post, expected_union, expected_error) in cases {
        let pre = decode(if reflected {
            include_bytes!(
                "../../../conformance/old-pencil-counterexample/serialized5-reflect-pre.z"
            )
        } else {
            include_bytes!("../../../conformance/old-pencil-counterexample/serialized5-plain-pre.z")
        });
        assert_eq!(pre.len(), 1920 * 1080 * 4);
        let mut payload = include_bytes!(
            "../../../conformance/old-pencil-counterexample/serialized-pencil-color.payload"
        )
        .to_vec();
        for offset in [2631, 2639] {
            assert_eq!(&payload[offset..offset + 4], &9.1168_f32.to_le_bytes());
            payload[offset..offset + 4].copy_from_slice(&5.0_f32.to_le_bytes());
        }
        if reflected {
            payload[2627..2631].copy_from_slice(&0x40f130a0_u32.to_le_bytes());
        }
        if reflected {
            for i in 0..203 {
                payload[173 + i * 4 + 1] ^= 0x80;
            }
        }
        let object = StoredObject {
            object_type: ObjectType::Stroke,
            declared_size: (payload.len() + 32) as u32,
            payload_offset: 0,
            payload_size: payload.len(),
            integrity_trailer: [0; 32],
            children: vec![],
        };
        let stroke = object
            .decode_stroke(&payload, &ParseLimits::default())
            .unwrap();
        assert_eq!(stroke.pen_width, 5.0);
        let mut opt = RenderOptions::default();
        opt.old_pencil_svg_profile = true;
        let source = svg(&doc(stroke), &opt);
        assert!(source.contains("crispEdges"));
        let body = &source[source.find("<defs").unwrap()..source.rfind("</svg>").unwrap()];
        let transported = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1920\" height=\"1080\"><g transform=\"translate(0 1080) scale(1 -1)\">{body}</g></svg>"
        );
        let tree = resvg::usvg::Tree::from_str(&transported, &Default::default()).unwrap();
        let mut pixmap = resvg::tiny_skia::Pixmap::new(1920, 1080).unwrap();
        pixmap.data_mut().copy_from_slice(&pre);
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::identity(),
            &mut pixmap.as_mut(),
        );
        let native = decode(post);
        assert_eq!(native.len(), pre.len());
        assert_eq!(pixmap.data().len(), pre.len());
        let mut union = 0;
        let mut error = 0u64;
        for ((actual, expected), prior) in pixmap
            .data()
            .as_chunks::<4>()
            .0
            .iter()
            .zip(native.as_chunks::<4>().0.iter())
            .zip(pre.as_chunks::<4>().0.iter())
        {
            if actual != prior || expected != prior {
                union += 1;
            }
            for i in 0..3 {
                error += u64::from(actual[i].abs_diff(expected[i]));
            }
        }
        assert_eq!((union, error), (expected_union, expected_error));
    }
}

#[cfg(feature = "pdf")]
#[test]
fn synthetic_serialized_size5_pdf_uses_fallback() {
    for size in [
        3.0_f32,
        5.0,
        7.25,
        f32::from_bits(12.0_f32.to_bits() - 1),
        f32::from_bits(12.0_f32.to_bits() + 1),
    ] {
        let mut s = stroke(0x8024a0d8);
        s.pen_width = size;
        let style = &mut s.rendering.as_mut().unwrap().style;
        style.pen_size = Some(size);
        style.fixed_width = Some(size);
        let d = doc(s);
        let plain = RenderOptions::default();
        let mut opt = plain.clone();
        opt.old_pencil_pdf_profile = true;
        assert_eq!(
            sdocx::render_document_pdf(&d, &plain, &sdocx::PdfOptions::default()).unwrap(),
            sdocx::render_document_pdf(&d, &opt, &sdocx::PdfOptions::default()).unwrap(),
            "size {size}"
        );
    }
}

#[test]
fn serialized_interval_svg_tracks_native() {
    use std::io::Read;
    let decode = |bytes: &[u8]| {
        let mut out = Vec::new();
        flate2::read::ZlibDecoder::new(bytes)
            .read_to_end(&mut out)
            .unwrap();
        out
    };
    type Case<'a> = (
        f32,
        bool,
        bool,
        &'a [u8],
        &'a [u8],
        [(usize, u64, u8, u64, u8); 2],
    );
    let cases: [Case<'_>; 7] = [
        (
            3.0,
            false,
            false,
            include_bytes!("../../../conformance/old-pencil-counterexample/interval-3_plain-1.z"),
            include_bytes!("../../../conformance/old-pencil-counterexample/interval-3_plain-2.z"),
            [(131, 209, 2, 12528, 152), (533, 829, 2, 66555, 190)],
        ),
        (
            3.0,
            true,
            false,
            include_bytes!("../../../conformance/old-pencil-counterexample/interval-3_reflect-1.z"),
            include_bytes!("../../../conformance/old-pencil-counterexample/interval-3_reflect-2.z"),
            [(132, 171, 2, 15164, 197), (523, 679, 2, 59019, 196)],
        ),
        (
            12.0,
            false,
            false,
            include_bytes!("../../../conformance/old-pencil-counterexample/interval-12_plain-1.z"),
            include_bytes!("../../../conformance/old-pencil-counterexample/interval-12_plain-2.z"),
            [(545, 845, 2, 74342, 183), (2165, 3334, 2, 321060, 190)],
        ),
        (
            12.0,
            true,
            false,
            include_bytes!(
                "../../../conformance/old-pencil-counterexample/interval-12_reflect-1.z"
            ),
            include_bytes!(
                "../../../conformance/old-pencil-counterexample/interval-12_reflect-2.z"
            ),
            [(549, 771, 3, 64350, 203), (2171, 3116, 3, 253887, 203)],
        ),
        (
            4.25,
            false,
            false,
            include_bytes!(
                "../../../conformance/old-pencil-counterexample/interval-4_25_plain-1.z"
            ),
            include_bytes!(
                "../../../conformance/old-pencil-counterexample/interval-4_25_plain-2.z"
            ),
            [(195, 315, 10, 23624, 177), (765, 1221, 2, 107378, 189)],
        ),
        (
            7.25,
            false,
            true,
            include_bytes!("../../../conformance/old-pencil-counterexample/interval-7_25_flat-1.z"),
            include_bytes!("../../../conformance/old-pencil-counterexample/interval-7_25_flat-2.z"),
            [(333, 513, 2, 47185, 182), (1340, 2055, 3, 193584, 190)],
        ),
        (
            4.25,
            true,
            true,
            include_bytes!("../../../conformance/old-pencil-counterexample/stretched-1.z"),
            include_bytes!("../../../conformance/old-pencil-counterexample/stretched-2.z"),
            [(321, 420, 2, 34048, 202), (1292, 1672, 3, 142437, 203)],
        ),
    ];
    for (size, reflected, flat, post1, post2, metrics) in cases {
        let pre = decode(if reflected {
            include_bytes!(
                "../../../conformance/old-pencil-counterexample/serialized5-reflect-pre.z"
            )
        } else {
            include_bytes!("../../../conformance/old-pencil-counterexample/serialized5-plain-pre.z")
        });
        assert_eq!(pre.len(), 1920 * 1080 * 4);
        let mut payload = include_bytes!(
            "../../../conformance/old-pencil-counterexample/serialized-pencil-color.payload"
        )
        .to_vec();
        for offset in [2631, 2639] {
            payload[offset..offset + 4].copy_from_slice(&size.to_le_bytes());
        }
        if reflected {
            payload[2627..2631].copy_from_slice(&0x40f130a0_u32.to_le_bytes());
            for i in 0..203 {
                let off = 173 + i * 4;
                if flat {
                    let value = u16::from_le_bytes([payload[off], payload[off + 1]]);
                    payload[off..off + 2].copy_from_slice(
                        &((value & 0x8000) | ((value & 0x7fff) * 2)).to_le_bytes(),
                    );
                } else {
                    payload[off + 1] ^= 0x80;
                }
            }
        }
        if flat {
            payload[985..989].copy_from_slice(&0.5_f32.to_le_bytes());
            payload[989..1395].fill(0);
        }
        let object = StoredObject {
            object_type: ObjectType::Stroke,
            declared_size: (payload.len() + 32) as u32,
            payload_offset: 0,
            payload_size: payload.len(),
            integrity_trailer: [0; 32],
            children: vec![],
        };
        let stroke = object
            .decode_stroke(&payload, &ParseLimits::default())
            .unwrap();
        assert_eq!(stroke.pen_width, size);
        let doc = doc(stroke);
        let plain = RenderOptions::default();
        let mut opt = plain.clone();
        opt.old_pencil_svg_profile = true;
        let profile_svg = svg(&doc, &opt);
        let fallback_svg = svg(&doc, &plain);
        assert!(profile_svg.contains("crispEdges"));
        assert!(!fallback_svg.contains("crispEdges"));
        for (index, post) in [post1, post2].into_iter().enumerate() {
            let native = decode(post);
            assert_eq!(native.len(), pre.len());
            let evaluate = |source: &str, start: &str| {
                let body = &source[source.find(start).unwrap()..source.rfind("</svg>").unwrap()];
                let scale = index + 1;
                let transported = format!(
                    "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1920\" height=\"1080\"><g transform=\"translate(0 1080) scale(1 -1) scale({scale})\">{body}</g></svg>"
                );
                let tree = resvg::usvg::Tree::from_str(&transported, &Default::default()).unwrap();
                let mut pixmap = resvg::tiny_skia::Pixmap::new(1920, 1080).unwrap();
                pixmap.data_mut().copy_from_slice(&pre);
                resvg::render(
                    &tree,
                    resvg::tiny_skia::Transform::identity(),
                    &mut pixmap.as_mut(),
                );
                assert_eq!(pixmap.data().len(), pre.len());
                let (mut union, mut error, mut maximum) = (0, 0u64, 0u8);
                for ((a, b), prior) in pixmap
                    .data()
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .zip(native.as_chunks::<4>().0.iter())
                    .zip(pre.as_chunks::<4>().0.iter())
                {
                    if a != prior || b != prior {
                        union += 1;
                    }
                    for i in 0..3 {
                        let d = a[i].abs_diff(b[i]);
                        error += u64::from(d);
                        maximum = maximum.max(d);
                    }
                }
                (union, error, maximum)
            };
            let profile = evaluate(&profile_svg, "<defs");
            let fallback = evaluate(&fallback_svg, "<line");
            let (union, error, max, baseline_error, baseline_max) = metrics[index];
            assert_eq!(profile, (union, error, max));
            assert_eq!((fallback.1, fallback.2), (baseline_error, baseline_max));
            assert!(error < baseline_error);
        }
    }
}

#[test]
fn serialized_interval_inward_neighbors_are_admitted() {
    for size in [
        f32::from_bits(3.0_f32.to_bits() + 1),
        f32::from_bits(12.0_f32.to_bits() - 1),
    ] {
        let mut s = stroke(0x8024a0d8);
        s.pen_width = size;
        let style = &mut s.rendering.as_mut().unwrap().style;
        style.pen_size = Some(size);
        style.fixed_width = Some(size);
        let mut opt = RenderOptions::default();
        opt.old_pencil_svg_profile = true;
        assert!(svg(&doc(s), &opt).contains("crispEdges"));
    }
}

#[test]
fn stretched_transparent_native_rgba_comparison() {
    use std::io::Read;
    let decode = |bytes: &[u8]| {
        let mut out = Vec::new();
        flate2::read::ZlibDecoder::new(bytes)
            .read_to_end(&mut out)
            .unwrap();
        out
    };
    let pre = decode(include_bytes!(
        "../../../conformance/old-pencil-counterexample/stretched-transparent-pre.z"
    ));
    assert_eq!(pre.len(), 1920 * 1080 * 4);
    let mut payload = include_bytes!(
        "../../../conformance/old-pencil-counterexample/serialized-pencil-color.payload"
    )
    .to_vec();
    for offset in [2631, 2639] {
        payload[offset..offset + 4].copy_from_slice(&4.25_f32.to_le_bytes());
    }
    payload[2627..2631].copy_from_slice(&0x40f130a0_u32.to_le_bytes());
    for i in 0..203 {
        let off = 173 + i * 4;
        let value = u16::from_le_bytes([payload[off], payload[off + 1]]);
        payload[off..off + 2]
            .copy_from_slice(&((value & 0x8000) | ((value & 0x7fff) * 2)).to_le_bytes());
    }
    payload[985..989].copy_from_slice(&0.5_f32.to_le_bytes());
    payload[989..1395].fill(0);
    let object = StoredObject {
        object_type: ObjectType::Stroke,
        declared_size: (payload.len() + 32) as u32,
        payload_offset: 0,
        payload_size: payload.len(),
        integrity_trailer: [0; 32],
        children: vec![],
    };
    let stroke = object
        .decode_stroke(&payload, &ParseLimits::default())
        .unwrap();
    let doc = doc(stroke);
    let plain = RenderOptions::default();
    let mut opt = plain.clone();
    opt.old_pencil_svg_profile = true;
    let profile_svg = svg(&doc, &opt);
    let fallback_svg = svg(&doc, &plain);
    assert!(profile_svg.contains("crispEdges"));
    assert!(!fallback_svg.contains("crispEdges"));
    type Case<'a> = (&'a [u8], usize, [u64; 4], [u8; 4], [u64; 4], [u8; 4]);
    let cases: [Case<'_>; 2] = [
        (
            include_bytes!(
                "../../../conformance/old-pencil-counterexample/stretched-transparent-1.z"
            ),
            163,
            [71, 95, 99, 46],
            [2, 2, 2, 1],
            [28398, 5772, 18879, 30026],
            [240, 48, 160, 254],
        ),
        (
            include_bytes!(
                "../../../conformance/old-pencil-counterexample/stretched-transparent-2.z"
            ),
            630,
            [306, 384, 382, 233],
            [2, 2, 2, 2],
            [118498, 23752, 78738, 125419],
            [241, 48, 160, 255],
        ),
    ];
    for (index, (post, expected_diff, expected_sum, expected_max, baseline_sum, baseline_max)) in
        cases.into_iter().enumerate()
    {
        let native = decode(post);
        assert_eq!(native.len(), pre.len());
        let evaluate = |source: &str, start: &str| {
            let body = &source[source.find(start).unwrap()..source.rfind("</svg>").unwrap()];
            let scale = index + 1;
            let transported = format!(
                "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1920\" height=\"1080\"><g transform=\"translate(0 1080) scale(1 -1) scale({scale})\">{body}</g></svg>"
            );
            let tree = resvg::usvg::Tree::from_str(&transported, &Default::default()).unwrap();
            let mut pixmap = resvg::tiny_skia::Pixmap::new(1920, 1080).unwrap();
            pixmap.data_mut().copy_from_slice(&pre);
            resvg::render(
                &tree,
                resvg::tiny_skia::Transform::identity(),
                &mut pixmap.as_mut(),
            );
            assert_eq!(pixmap.data().len(), pre.len());
            let (mut diff, mut sums, mut max) = (0, [0u64; 4], [0u8; 4]);
            for (a, b) in pixmap
                .data()
                .as_chunks::<4>()
                .0
                .iter()
                .zip(native.as_chunks::<4>().0.iter())
            {
                if a != b {
                    diff += 1;
                }
                for i in 0..4 {
                    let d = a[i].abs_diff(b[i]);
                    sums[i] += u64::from(d);
                    max[i] = max[i].max(d);
                }
            }
            (diff, sums, max)
        };
        assert_eq!(
            evaluate(&profile_svg, "<defs"),
            (expected_diff, expected_sum, expected_max)
        );
        let fallback = evaluate(&fallback_svg, "<line");
        assert_eq!((fallback.1, fallback.2), (baseline_sum, baseline_max));
    }
}

#[test]
fn zero_alpha_svg_profile_leaves_native_prior_unchanged() {
    use std::io::Read;
    let mut pre = Vec::new();
    flate2::read::ZlibDecoder::new(
        include_bytes!("../../../conformance/old-pencil-color/varied-pre.rgba.zlib").as_slice(),
    )
    .read_to_end(&mut pre)
    .unwrap();
    assert_eq!(pre.len(), 1920 * 1080 * 4);
    let d = doc(stroke(0x0024a0d8));
    let mut opt = RenderOptions::default();
    opt.old_pencil_svg_profile = true;
    let source = svg(&d, &opt);
    assert!(source.contains("crispEdges"));
    let body = &source[source.find("<defs").unwrap()..source.rfind("</svg>").unwrap()];
    let transported = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1920\" height=\"1080\"><g transform=\"translate(0 1080) scale(1 -1)\">{body}</g></svg>"
    );
    let tree = resvg::usvg::Tree::from_str(&transported, &Default::default()).unwrap();
    let mut pixmap = resvg::tiny_skia::Pixmap::new(1920, 1080).unwrap();
    pixmap.data_mut().copy_from_slice(&pre);
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    assert_eq!(pixmap.data(), pre);
}

#[test]
fn serialized_dots_and_threshold_svg_track_native() {
    use std::io::Read;
    let decode = |bytes: &[u8]| {
        let mut out = Vec::new();
        flate2::read::ZlibDecoder::new(bytes)
            .read_to_end(&mut out)
            .unwrap();
        out
    };
    type Case<'a> = (
        f32,
        u32,
        Option<f32>,
        u16,
        bool,
        &'a [u8],
        (usize, u64, u8),
        u64,
    );
    let cases: [Case<'_>; 8] = [
        (
            12.0,
            0x8024a0d8,
            Some(0.0),
            2,
            false,
            include_bytes!("../../../conformance/old-pencil-counterexample/dot-12-visible-1.z"),
            (22, 27, 1),
            4012,
        ),
        (
            5.0,
            0x40f130a0,
            Some(0.5),
            2,
            false,
            include_bytes!("../../../conformance/old-pencil-counterexample/dot-5-post.z"),
            (6, 10, 1),
            1151,
        ),
        (
            4.25,
            0x8024a0d8,
            None,
            0,
            false,
            include_bytes!("../../../conformance/old-pencil-counterexample/dot-425-post.z"),
            (4, 5, 1),
            761,
        ),
        (
            8.0,
            0x40f130a0,
            Some(0.5),
            2,
            false,
            include_bytes!("../../../conformance/old-pencil-counterexample/dot-8-post.z"),
            (22, 32, 1),
            2695,
        ),
        (
            3.0,
            0xff24a0d8,
            Some(1.0),
            2,
            false,
            include_bytes!("../../../conformance/old-pencil-counterexample/dot-3_full-post.z"),
            (4, 2, 1),
            1417,
        ),
        (
            12.0,
            0x0024a0d8,
            Some(0.0),
            2,
            false,
            include_bytes!(
                "../../../conformance/old-pencil-counterexample/serialized5-plain-pre.z"
            ),
            (0, 0, 0),
            6212,
        ),
        (
            8.125,
            0x40f130a0,
            Some(0.5),
            51,
            true,
            include_bytes!(
                "../../../conformance/old-pencil-counterexample/dot-threshold_below-post.z"
            ),
            (27, 42, 1),
            3103,
        ),
        (
            8.125,
            0x40f130a0,
            Some(0.5),
            52,
            true,
            include_bytes!(
                "../../../conformance/old-pencil-counterexample/dot-threshold_exact-post.z"
            ),
            (35, 68, 2),
            3554,
        ),
    ];
    let pre = decode(include_bytes!(
        "../../../conformance/old-pencil-counterexample/serialized5-plain-pre.z"
    ));
    assert_eq!(pre.len(), 1920 * 1080 * 4);
    for (size, argb, pressure, delta, origin100, post, expected, baseline) in cases {
        let mut payload = include_bytes!(
            "../../../conformance/old-pencil-counterexample/serialized-pencil-color.payload"
        )
        .to_vec();
        for offset in [2631, 2639] {
            payload[offset..offset + 4].copy_from_slice(&size.to_le_bytes());
        }
        payload[2627..2631].copy_from_slice(&argb.to_le_bytes());
        payload[173..985].fill(0);
        payload[173..175].copy_from_slice(&delta.to_le_bytes());
        if origin100 {
            payload[157..165].copy_from_slice(&100.0_f64.to_le_bytes());
            payload[165..173].copy_from_slice(&100.0_f64.to_le_bytes());
        }
        if let Some(p) = pressure {
            payload[985..989].copy_from_slice(&p.to_le_bytes());
            payload[989..1395].fill(0);
        }
        let object = StoredObject {
            object_type: ObjectType::Stroke,
            declared_size: (payload.len() + 32) as u32,
            payload_offset: 0,
            payload_size: payload.len(),
            integrity_trailer: [0; 32],
            children: vec![],
        };
        let stroke = object
            .decode_stroke(&payload, &ParseLimits::default())
            .unwrap();
        let d = doc(stroke);
        let plain = RenderOptions::default();
        let mut opt = plain.clone();
        opt.old_pencil_svg_profile = true;
        let profile = svg(&d, &opt);
        let fallback = svg(&d, &plain);
        assert!(profile.contains("crispEdges"));
        assert!(!fallback.contains("crispEdges"));
        let native = decode(post);
        assert_eq!(native.len(), pre.len());
        let evaluate = |source: &str, start: &str| {
            let body = &source[source.find(start).unwrap()..source.rfind("</svg>").unwrap()];
            let transported = format!(
                "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1920\" height=\"1080\"><g transform=\"translate(0 1080) scale(1 -1)\">{body}</g></svg>"
            );
            let tree = resvg::usvg::Tree::from_str(&transported, &Default::default()).unwrap();
            let mut pixmap = resvg::tiny_skia::Pixmap::new(1920, 1080).unwrap();
            pixmap.data_mut().copy_from_slice(&pre);
            resvg::render(
                &tree,
                resvg::tiny_skia::Transform::identity(),
                &mut pixmap.as_mut(),
            );
            assert_eq!(pixmap.data().len(), pre.len());
            let (mut union, mut error, mut max) = (0, 0u64, 0u8);
            for ((a, b), p) in pixmap
                .data()
                .as_chunks::<4>()
                .0
                .iter()
                .zip(native.as_chunks::<4>().0.iter())
                .zip(pre.as_chunks::<4>().0.iter())
            {
                if a != p || b != p {
                    union += 1;
                }
                for i in 0..3 {
                    let d = a[i].abs_diff(b[i]);
                    error += u64::from(d);
                    max = max.max(d);
                }
            }
            (union, error, max)
        };
        assert_eq!(evaluate(&profile, "<defs"), expected);
        let fallback_start = if fallback.contains("<line") {
            "<line"
        } else {
            "<polyline"
        };
        assert_eq!(evaluate(&fallback, fallback_start).1, baseline);
    }
}

#[test]
fn one_point_saved_stroke_keeps_svg_fallback() {
    let mut s = stroke(0x8024a0d8);
    s.points.truncate(1);
    s.pressures.truncate(1);
    s.timestamps.truncate(1);
    s.tilts.truncate(1);
    s.orientations.truncate(1);
    let d = doc(s);
    let plain = RenderOptions::default();
    let mut opt = plain.clone();
    opt.old_pencil_svg_profile = true;
    assert_eq!(svg(&d, &plain), svg(&d, &opt));
}

#[test]
fn serialized_two_point_dot_svg_matches_204_point_native_control() {
    let mut payload = include_bytes!(
        "../../../conformance/old-pencil-counterexample/serialized-pencil-color.payload"
    )
    .to_vec();
    for off in [2631, 2639] {
        payload[off..off + 4].copy_from_slice(&8.0_f32.to_le_bytes());
    }
    payload[2627..2631].copy_from_slice(&0x40f130a0_u32.to_le_bytes());
    payload[173..985].fill(0);
    payload[173..175].copy_from_slice(&2u16.to_le_bytes());
    payload[985..989].copy_from_slice(&0.5_f32.to_le_bytes());
    payload[989..1395].fill(0);
    let object = |len| StoredObject {
        object_type: ObjectType::Stroke,
        declared_size: (len + 32) as u32,
        payload_offset: 0,
        payload_size: len,
        integrity_trailer: [0; 32],
        children: vec![],
    };
    let original = object(payload.len())
        .decode_stroke(&payload, &ParseLimits::default())
        .unwrap();
    assert_eq!(original.points.len(), 204);
    let frame = payload[137..].to_vec();
    let mut short = frame[..18].to_vec();
    short.extend_from_slice(&2u16.to_le_bytes());
    for part in [
        &frame[20..40],
        &frame[848..854],
        &frame[1258..1264],
        &frame[1668..1674],
        &frame[2078..2084],
        &frame[2488..2490],
        &frame[2490..],
    ] {
        short.extend_from_slice(part);
    }
    short[6..10].copy_from_slice(&66u32.to_le_bytes());
    let len = short.len() as u32;
    short[..4].copy_from_slice(&len.to_le_bytes());
    payload.truncate(137);
    payload.extend_from_slice(&short);
    let two = object(payload.len())
        .decode_stroke(&payload, &ParseLimits::default())
        .unwrap();
    assert_eq!(two.points.len(), 2);
    let mut opt = RenderOptions::default();
    opt.old_pencil_svg_profile = true;
    let actual = svg(&doc(two), &opt);
    assert!(actual.contains("crispEdges"));
    assert_eq!(actual, svg(&doc(original), &opt));
}

#[test]
fn serialized_rapid_bend_svg_tracks_native() {
    use std::io::Read;
    let decode = |bytes: &[u8]| {
        let mut out = Vec::new();
        flate2::read::ZlibDecoder::new(bytes)
            .read_to_end(&mut out)
            .unwrap();
        out
    };
    let pre = decode(include_bytes!(
        "../../../conformance/old-pencil-counterexample/serialized5-plain-pre.z"
    ));
    assert_eq!(pre.len(), 1920 * 1080 * 4);
    let mut payload = include_bytes!(
        "../../../conformance/old-pencil-counterexample/serialized-pencil-color.payload"
    )
    .to_vec();
    for off in [2631, 2639] {
        payload[off..off + 4].copy_from_slice(&12.0_f32.to_le_bytes());
    }
    payload[2627..2631].copy_from_slice(&0xc0ee8811_u32.to_le_bytes());
    for i in 0..203 {
        let off = 173 + 4 * i;
        let value = u16::from_le_bytes([payload[off], payload[off + 1]]);
        payload[off..off + 2]
            .copy_from_slice(&((value & 0x8000) | ((value & 0x7fff) * 2)).to_le_bytes());
        if i >= 100 {
            payload[off + 3] ^= 0x80;
        }
    }
    payload[985..989].copy_from_slice(&0.125_f32.to_le_bytes());
    for i in 0..203 {
        let value: u16 = if i % 2 == 0 { 3072 } else { 0x8c00 };
        payload[989 + 2 * i..991 + 2 * i].copy_from_slice(&value.to_le_bytes());
    }
    let object = StoredObject {
        object_type: ObjectType::Stroke,
        declared_size: (payload.len() + 32) as u32,
        payload_offset: 0,
        payload_size: payload.len(),
        integrity_trailer: [0; 32],
        children: vec![],
    };
    let stroke = object
        .decode_stroke(&payload, &ParseLimits::default())
        .unwrap();
    let document = doc(stroke);
    let plain = RenderOptions::default();
    let mut opt = plain.clone();
    opt.old_pencil_svg_profile = true;
    let profile_svg = svg(&document, &opt);
    let fallback_svg = svg(&document, &plain);
    assert!(profile_svg.contains("crispEdges"));
    assert!(!fallback_svg.contains("crispEdges"));
    let mut altered_payload = payload.clone();
    altered_payload[2647..2651].copy_from_slice(&100.0_f32.to_le_bytes());
    let altered = object
        .decode_stroke(&altered_payload, &ParseLimits::default())
        .unwrap();
    assert_eq!(
        altered.rendering.as_ref().unwrap().style.initial_tolerance,
        Some(100.0)
    );
    let altered_doc = doc(altered);
    assert_eq!(svg(&altered_doc, &opt), profile_svg);
    assert_eq!(svg(&altered_doc, &plain), fallback_svg);
    #[cfg(feature = "pdf")]
    {
        let mut pdf_opt = plain.clone();
        pdf_opt.old_pencil_pdf_profile = true;
        assert_eq!(
            sdocx::render_document_pdf(&altered_doc, &pdf_opt, &sdocx::PdfOptions::default())
                .unwrap(),
            sdocx::render_document_pdf(&document, &pdf_opt, &sdocx::PdfOptions::default()).unwrap()
        );
    }
    let mut density_payload = payload.clone();
    density_payload[2643..2647].copy_from_slice(&8_i32.to_le_bytes());
    let density = object
        .decode_stroke(&density_payload, &ParseLimits::default())
        .unwrap();
    assert_eq!(
        density.rendering.as_ref().unwrap().style.particle_density,
        Some(8)
    );
    let density_doc = doc(density);
    assert_eq!(svg(&density_doc, &opt), profile_svg);
    assert_eq!(svg(&density_doc, &plain), fallback_svg);
    #[cfg(feature = "pdf")]
    {
        let mut pdf_opt = plain.clone();
        pdf_opt.old_pencil_pdf_profile = true;
        assert_eq!(
            sdocx::render_document_pdf(&density_doc, &pdf_opt, &sdocx::PdfOptions::default())
                .unwrap(),
            sdocx::render_document_pdf(&document, &pdf_opt, &sdocx::PdfOptions::default()).unwrap()
        );
    }
    let mut width_payload = payload.clone();
    width_payload[2639..2643].copy_from_slice(&20.0_f32.to_le_bytes());
    let width = object
        .decode_stroke(&width_payload, &ParseLimits::default())
        .unwrap();
    assert_eq!(
        width.rendering.as_ref().unwrap().style.fixed_width,
        Some(20.0)
    );
    let width_doc = doc(width);
    assert_eq!(svg(&width_doc, &opt), profile_svg);
    assert_eq!(svg(&width_doc, &plain), fallback_svg);
    #[cfg(feature = "pdf")]
    {
        let mut pdf_opt = plain.clone();
        pdf_opt.old_pencil_pdf_profile = true;
        assert_eq!(
            sdocx::render_document_pdf(&width_doc, &pdf_opt, &sdocx::PdfOptions::default())
                .unwrap(),
            sdocx::render_document_pdf(&document, &pdf_opt, &sdocx::PdfOptions::default()).unwrap()
        );
    }
    let mut type_payload = payload.clone();
    assert_eq!(type_payload[153], 0);
    type_payload[153] |= 1;
    type_payload.extend_from_slice(&0_u16.to_le_bytes());
    type_payload[137..141].copy_from_slice(&2516_u32.to_le_bytes());
    let type_object = StoredObject {
        payload_size: type_payload.len(),
        declared_size: (type_payload.len() + 32) as u32,
        ..object
    };
    let explicit = type_object
        .decode_stroke(&type_payload, &ParseLimits::default())
        .unwrap();
    assert_eq!(
        explicit.rendering.as_ref().unwrap().style.stroke_type_raw,
        Some(0)
    );
    let explicit = doc(explicit);
    assert_eq!(svg(&explicit, &opt), profile_svg);
    assert_eq!(svg(&explicit, &plain), fallback_svg);
    type Case<'a> = (&'a [u8], usize, u64, u8, u64, u8);
    let cases: [Case<'_>; 2] = [
        (
            include_bytes!("../../../conformance/old-pencil-counterexample/rapid-bend-1.z"),
            751,
            1188,
            3,
            70608,
            162,
        ),
        (
            include_bytes!("../../../conformance/old-pencil-counterexample/rapid-bend-2.z"),
            3088,
            4738,
            3,
            288853,
            162,
        ),
    ];
    for (
        index,
        (post, expected_union, expected_error, expected_max, baseline_error, baseline_max),
    ) in cases.into_iter().enumerate()
    {
        let native = decode(post);
        assert_eq!(native.len(), pre.len());
        let evaluate = |source: &str, start: &str| {
            let body = &source[source.find(start).unwrap()..source.rfind("</svg>").unwrap()];
            let scale = index + 1;
            let transported = format!(
                "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1920\" height=\"1080\"><g transform=\"translate(0 1080) scale(1 -1) scale({scale})\">{body}</g></svg>"
            );
            let tree = resvg::usvg::Tree::from_str(&transported, &Default::default()).unwrap();
            let mut pixmap = resvg::tiny_skia::Pixmap::new(1920, 1080).unwrap();
            pixmap.data_mut().copy_from_slice(&pre);
            resvg::render(
                &tree,
                resvg::tiny_skia::Transform::identity(),
                &mut pixmap.as_mut(),
            );
            assert_eq!(pixmap.data().len(), pre.len());
            let (mut union, mut error, mut max) = (0, 0u64, 0u8);
            for ((a, b), p) in pixmap
                .data()
                .as_chunks::<4>()
                .0
                .iter()
                .zip(native.as_chunks::<4>().0.iter())
                .zip(pre.as_chunks::<4>().0.iter())
            {
                if a != p || b != p {
                    union += 1;
                }
                for i in 0..3 {
                    let d = a[i].abs_diff(b[i]);
                    error += u64::from(d);
                    max = max.max(d);
                }
            }
            (union, error, max)
        };
        assert_eq!(
            evaluate(&profile_svg, "<defs"),
            (expected_union, expected_error, expected_max)
        );
        let fallback = evaluate(&fallback_svg, "<line");
        assert_eq!((fallback.1, fallback.2), (baseline_error, baseline_max));
    }
}

#[test]
fn resolved_synthetic_archive_svg_tracks_native() {
    use std::io::Read;
    let decode = |bytes: &[u8]| {
        let mut out = Vec::new();
        flate2::read::ZlibDecoder::new(bytes)
            .read_to_end(&mut out)
            .unwrap();
        out
    };
    let pre = decode(include_bytes!(
        "../../../conformance/old-pencil-color/varied-pre.rgba.zlib"
    ));
    assert_eq!(pre.len(), 1920 * 1080 * 4);
    let archive = sdocx::parse_bytes(include_bytes!(
        "../../../conformance/old-pencil-counterexample/resolved-synthetic.sdocx"
    ))
    .unwrap();
    let strokes: Vec<_> = archive
        .pages
        .iter()
        .flat_map(|p| p.strokes())
        .filter(|s| {
            s.rendering.as_ref().is_some_and(|r| {
                r.pen_name.as_deref() == Some("com.samsung.android.sdk.pen.pen.preload.Pencil")
            })
        })
        .collect();
    assert_eq!(strokes.len(), 1);
    let stroke = strokes[0];
    assert_eq!(stroke.points.len(), 204);
    assert_eq!(
        stroke.rendering.as_ref().unwrap().style.color_argb,
        Some(0x8024a0d8)
    );
    let document = doc(stroke.clone());
    let plain = RenderOptions::default();
    let mut opt = plain.clone();
    opt.old_pencil_svg_profile = true;
    let profile = svg(&document, &opt);
    let fallback = svg(&document, &plain);
    assert!(profile.contains("crispEdges"));
    assert!(!fallback.contains("crispEdges"));
    type Case<'a> = (&'a [u8], usize, u64, u8, u64, u8);
    let cases: [Case<'_>; 2] = [
        (
            include_bytes!(
                "../../../conformance/old-pencil-counterexample/serialized-pencil-color-post.rgba.zlib"
            ),
            420,
            469,
            2,
            23166,
            60,
        ),
        (
            include_bytes!(
                "../../../conformance/old-pencil-counterexample/serialized_color_scale2-top.rgba.zlib"
            ),
            1654,
            1856,
            2,
            96750,
            63,
        ),
    ];
    for (index, (post, union, error, max, base_error, base_max)) in cases.into_iter().enumerate() {
        let native = decode(post);
        assert_eq!(native.len(), pre.len());
        let evaluate = |source: &str, start: &str| {
            let body = &source[source.find(start).unwrap()..source.rfind("</svg>").unwrap()];
            let scale = index + 1;
            let transported = format!(
                "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1920\" height=\"1080\"><g transform=\"translate(0 1080) scale(1 -1) scale({scale})\">{body}</g></svg>"
            );
            let tree = resvg::usvg::Tree::from_str(&transported, &Default::default()).unwrap();
            let mut pixmap = resvg::tiny_skia::Pixmap::new(1920, 1080).unwrap();
            pixmap.data_mut().copy_from_slice(&pre);
            resvg::render(
                &tree,
                resvg::tiny_skia::Transform::identity(),
                &mut pixmap.as_mut(),
            );
            assert_eq!(pixmap.data().len(), pre.len());
            let (mut n, mut sum, mut maximum) = (0, 0u64, 0u8);
            for ((a, b), p) in pixmap
                .data()
                .as_chunks::<4>()
                .0
                .iter()
                .zip(native.as_chunks::<4>().0.iter())
                .zip(pre.as_chunks::<4>().0.iter())
            {
                if a != p || b != p {
                    n += 1;
                }
                for i in 0..3 {
                    let d = a[i].abs_diff(b[i]);
                    sum += u64::from(d);
                    maximum = maximum.max(d);
                }
            }
            (n, sum, maximum)
        };
        assert_eq!(evaluate(&profile, "<defs"), (union, error, max));
        let actual = evaluate(&fallback, "<line");
        assert_eq!((actual.1, actual.2), (base_error, base_max));
    }
}

#[test]
fn serialized_positive_zero_rotation_is_admitted() {
    use std::io::Read;
    let mut b = include_bytes!(
        "../../../conformance/old-pencil-counterexample/serialized-pencil-color.payload"
    )
    .to_vec();
    b[14] |= 1;
    b.splice(105..105, 0.0f32.to_le_bytes());
    b[..4].copy_from_slice(&141u32.to_le_bytes());
    for off in [161, 169] {
        let v = f64::from_le_bytes(b[off..off + 8].try_into().unwrap()) + 100.;
        b[off..off + 8].copy_from_slice(&v.to_le_bytes());
    }
    let o = StoredObject {
        object_type: ObjectType::Stroke,
        declared_size: (b.len() + 32) as u32,
        payload_offset: 0,
        payload_size: b.len(),
        integrity_trailer: [0; 32],
        children: vec![],
    };
    let s = o.decode_stroke(&b, &ParseLimits::default()).unwrap();
    assert_eq!(
        s.rendering
            .as_ref()
            .unwrap()
            .metadata
            .as_ref()
            .unwrap()
            .rotation_degrees
            .unwrap()
            .to_bits(),
        0
    );
    let bounds = &s
        .rendering
        .as_ref()
        .unwrap()
        .metadata
        .as_ref()
        .unwrap()
        .bbox;
    assert!(s.points[0].x > bounds.x_max && s.points[0].y > bounds.y_max);
    let mut channels = Vec::new();
    for point in &s.points {
        for value in [point.x, point.y] {
            channels.extend_from_slice(&(value as f32).to_le_bytes());
        }
    }
    for value in &s.pressures {
        channels.extend_from_slice(&(*value as f32).to_le_bytes());
    }
    for value in &s.timestamps {
        channels.extend_from_slice(&(*value as i32).to_le_bytes());
    }
    for value in s.tilts.iter().chain(&s.orientations) {
        channels.extend_from_slice(&(*value as f32).to_le_bytes());
    }
    assert_eq!(
        channels,
        include_bytes!(
            "../../../conformance/old-pencil-counterexample/rotation-pointshift.channels"
        )
    );
    let d = doc(s.clone());
    let plain = RenderOptions::default();
    let mut opt = plain.clone();
    opt.old_pencil_svg_profile = true;
    let profile = svg(&d, &opt);
    let fallback = svg(&d, &plain);
    assert!(profile.contains("crispEdges"));
    assert!(!fallback.contains("crispEdges"));
    let decode = |bytes: &[u8]| {
        let mut out = Vec::new();
        flate2::read::ZlibDecoder::new(bytes)
            .read_to_end(&mut out)
            .unwrap();
        out
    };
    let pre = decode(include_bytes!(
        "../../../conformance/old-pencil-color/varied-pre.rgba.zlib"
    ));
    let native = decode(include_bytes!(
        "../../../conformance/old-pencil-counterexample/rotation-pointshift-top.z"
    ));
    assert_eq!(native.len(), 1920 * 1080 * 4);
    assert_eq!(pre.len(), native.len());
    let evaluate = |source: &str, start: &str| {
        let body = &source[source.find(start).unwrap()..source.rfind("</svg>").unwrap()];
        let transport = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1920\" height=\"1080\"><g transform=\"translate(0 1080) scale(1 -1)\">{body}</g></svg>"
        );
        let tree = resvg::usvg::Tree::from_str(&transport, &Default::default()).unwrap();
        let mut pix = resvg::tiny_skia::Pixmap::new(1920, 1080).unwrap();
        pix.data_mut().copy_from_slice(&pre);
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::identity(),
            &mut pix.as_mut(),
        );
        let (mut diff, mut sum, mut max) = (0, 0u64, 0u8);
        for (a, b) in pix
            .data()
            .as_chunks::<4>()
            .0
            .iter()
            .zip(native.as_chunks::<4>().0.iter())
        {
            if a != b {
                diff += 1
            }
            for i in 0..3 {
                let d = a[i].abs_diff(b[i]);
                sum += u64::from(d);
                max = max.max(d);
            }
        }
        (diff, sum, max)
    };
    assert_eq!(evaluate(&profile, "<defs"), (350, 469, 2));
    assert_eq!(evaluate(&fallback, "<line"), (421, 23166, 60));
    let mut no_rotation = s.clone();
    no_rotation
        .rendering
        .as_mut()
        .unwrap()
        .metadata
        .as_mut()
        .unwrap()
        .rotation_degrees = None;
    let no_rotation = doc(no_rotation);
    assert_eq!(profile, svg(&no_rotation, &opt));
    #[cfg(feature = "pdf")]
    {
        let mut p = plain.clone();
        p.old_pencil_pdf_profile = true;
        let admitted = sdocx::render_document_pdf(&d, &p, &sdocx::PdfOptions::default()).unwrap();
        assert_ne!(
            admitted,
            sdocx::render_document_pdf(&d, &plain, &sdocx::PdfOptions::default()).unwrap()
        );
        assert_eq!(
            admitted,
            sdocx::render_document_pdf(&no_rotation, &p, &sdocx::PdfOptions::default()).unwrap()
        );
    }
    for angle in [-0.0, f64::from(f32::from_bits(1)), 30.0, f64::NAN] {
        let mut bad = s.clone();
        bad.rendering
            .as_mut()
            .unwrap()
            .metadata
            .as_mut()
            .unwrap()
            .rotation_degrees = Some(angle);
        let d = doc(bad);
        assert_eq!(svg(&d, &opt), svg(&d, &plain), "rotation {angle:?}");
        #[cfg(feature = "pdf")]
        {
            let mut p = plain.clone();
            p.old_pencil_pdf_profile = true;
            assert_eq!(
                sdocx::render_document_pdf(&d, &p, &sdocx::PdfOptions::default()).unwrap(),
                sdocx::render_document_pdf(&d, &plain, &sdocx::PdfOptions::default()).unwrap()
            );
        }
    }
}

#[test]
fn synthetic_serialized_pencil_tolerance_variants_preserve_vector_output() {
    let payload = include_bytes!(
        "../../../conformance/old-pencil-counterexample/serialized-pencil-color.payload"
    )
    .to_vec();
    let decode = |b: &[u8]| {
        let o = StoredObject {
            object_type: ObjectType::Stroke,
            declared_size: (b.len() + 32) as u32,
            payload_offset: 0,
            payload_size: b.len(),
            integrity_trailer: [0; 32],
            children: vec![],
        };
        o.decode_stroke(b, &ParseLimits::default()).unwrap()
    };
    let base = decode(&payload);
    let plain = RenderOptions::default();
    let mut svg_opt = plain.clone();
    svg_opt.old_pencil_svg_profile = true;
    #[cfg(feature = "pdf")]
    let mut pdf_opt = plain.clone();
    #[cfg(feature = "pdf")]
    {
        pdf_opt.old_pencil_pdf_profile = true;
    }
    let base_doc = doc(base.clone());
    let expected = svg(&base_doc, &svg_opt);
    assert!(expected.contains("crispEdges"));
    #[cfg(feature = "pdf")]
    let expected_pdf =
        sdocx::render_document_pdf(&base_doc, &pdf_opt, &sdocx::PdfOptions::default()).unwrap();
    for tolerance in [None, Some(0.0_f32), Some(0.5), Some(100.0)] {
        let mut bytes = payload.clone();
        if let Some(value) = tolerance {
            bytes[2647..2651].copy_from_slice(&value.to_le_bytes());
        } else {
            assert!(bytes[152] & 0x20 != 0);
            bytes[152] &= !0x20;
            bytes.drain(2647..2651);
            bytes[137..141].copy_from_slice(&2510_u32.to_le_bytes());
        }
        let actual = decode(&bytes);
        assert_eq!(
            actual.rendering.as_ref().unwrap().style.initial_tolerance,
            tolerance
        );
        let mut adjusted = actual.clone();
        adjusted.rendering.as_mut().unwrap().style.initial_tolerance =
            base.rendering.as_ref().unwrap().style.initial_tolerance;
        assert_eq!(
            adjusted.rendering.as_ref().unwrap().style,
            base.rendering.as_ref().unwrap().style
        );
        assert_eq!(adjusted.points, base.points);
        assert_eq!(adjusted.pressures, base.pressures);
        assert_eq!(adjusted.timestamps, base.timestamps);
        assert_eq!(adjusted.tilts, base.tilts);
        assert_eq!(adjusted.orientations, base.orientations);
        let d = doc(actual);
        assert_eq!(svg(&d, &svg_opt), expected);
        #[cfg(feature = "pdf")]
        assert_eq!(
            sdocx::render_document_pdf(&d, &pdf_opt, &sdocx::PdfOptions::default()).unwrap(),
            expected_pdf
        );
    }
    for value in [
        f32::from_bits(0x80000001),
        f32::from_bits(100f32.to_bits() + 1),
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
    ] {
        let mut bad = base.clone();
        bad.rendering.as_mut().unwrap().style.initial_tolerance = Some(value);
        let d = doc(bad);
        assert_eq!(svg(&d, &svg_opt), svg(&d, &plain));
        #[cfg(feature = "pdf")]
        assert_eq!(
            sdocx::render_document_pdf(&d, &pdf_opt, &sdocx::PdfOptions::default()).unwrap(),
            sdocx::render_document_pdf(&d, &plain, &sdocx::PdfOptions::default()).unwrap()
        );
    }
}

#[test]
fn synthetic_serialized_pencil_density_variants_preserve_vector_output() {
    let payload = include_bytes!(
        "../../../conformance/old-pencil-counterexample/serialized-pencil-color.payload"
    )
    .to_vec();
    let decode = |b: &[u8]| {
        let o = StoredObject {
            object_type: ObjectType::Stroke,
            declared_size: (b.len() + 32) as u32,
            payload_offset: 0,
            payload_size: b.len(),
            integrity_trailer: [0; 32],
            children: vec![],
        };
        o.decode_stroke(b, &ParseLimits::default()).unwrap()
    };
    let base = decode(&payload);
    let plain = RenderOptions::default();
    let mut svg_opt = plain.clone();
    svg_opt.old_pencil_svg_profile = true;
    #[cfg(feature = "pdf")]
    let mut pdf_opt = plain.clone();
    #[cfg(feature = "pdf")]
    {
        pdf_opt.old_pencil_pdf_profile = true;
    }
    let base_doc = doc(base.clone());
    let expected = svg(&base_doc, &svg_opt);
    assert!(expected.contains("crispEdges"));
    #[cfg(feature = "pdf")]
    let expected_pdf =
        sdocx::render_document_pdf(&base_doc, &pdf_opt, &sdocx::PdfOptions::default()).unwrap();
    for density in [None, Some(0_i32), Some(4), Some(8)] {
        let mut bytes = payload.clone();
        if let Some(value) = density {
            bytes[2643..2647].copy_from_slice(&value.to_le_bytes());
        } else {
            assert!(bytes[152] & 0x04 != 0);
            bytes[152] &= !0x04;
            bytes.drain(2643..2647);
            bytes[137..141].copy_from_slice(&2510_u32.to_le_bytes());
        }
        let actual = decode(&bytes);
        assert_eq!(
            actual.rendering.as_ref().unwrap().style.particle_density,
            density
        );
        let mut adjusted = actual.clone();
        adjusted.rendering.as_mut().unwrap().style.particle_density =
            base.rendering.as_ref().unwrap().style.particle_density;
        assert_eq!(
            adjusted.rendering.as_ref().unwrap().style,
            base.rendering.as_ref().unwrap().style
        );
        assert_eq!(adjusted.points, base.points);
        assert_eq!(adjusted.pressures, base.pressures);
        assert_eq!(adjusted.timestamps, base.timestamps);
        assert_eq!(adjusted.tilts, base.tilts);
        assert_eq!(adjusted.orientations, base.orientations);
        let d = doc(actual);
        assert_eq!(svg(&d, &svg_opt), expected);
        #[cfg(feature = "pdf")]
        assert_eq!(
            sdocx::render_document_pdf(&d, &pdf_opt, &sdocx::PdfOptions::default()).unwrap(),
            expected_pdf
        );
    }
    let mut alt_payload = payload.clone();
    alt_payload[2627..2631].copy_from_slice(&0x40f130a0_u32.to_le_bytes());
    let alt = doc(decode(&alt_payload));
    alt_payload[2643..2647].copy_from_slice(&4_i32.to_le_bytes());
    let alt_density = decode(&alt_payload);
    assert_eq!(
        alt_density
            .rendering
            .as_ref()
            .unwrap()
            .style
            .particle_density,
        Some(4)
    );
    let alt_density = doc(alt_density);
    assert_eq!(svg(&alt, &svg_opt), svg(&alt_density, &svg_opt));
    #[cfg(feature = "pdf")]
    assert_eq!(
        sdocx::render_document_pdf(&alt, &pdf_opt, &sdocx::PdfOptions::default()).unwrap(),
        sdocx::render_document_pdf(&alt_density, &pdf_opt, &sdocx::PdfOptions::default()).unwrap()
    );
    for value in [-1_i32, 9, i32::MIN, i32::MAX] {
        let mut bad = base.clone();
        bad.rendering.as_mut().unwrap().style.particle_density = Some(value);
        let d = doc(bad);
        assert_eq!(svg(&d, &svg_opt), svg(&d, &plain));
        #[cfg(feature = "pdf")]
        assert_eq!(
            sdocx::render_document_pdf(&d, &pdf_opt, &sdocx::PdfOptions::default()).unwrap(),
            sdocx::render_document_pdf(&d, &plain, &sdocx::PdfOptions::default()).unwrap()
        );
    }
}

#[test]
fn synthetic_serialized_pencil_fixed_width_variants_preserve_vector_output() {
    let payload = include_bytes!(
        "../../../conformance/old-pencil-counterexample/serialized-pencil-color.payload"
    )
    .to_vec();
    let decode = |b: &[u8]| {
        let o = StoredObject {
            object_type: ObjectType::Stroke,
            declared_size: (b.len() + 32) as u32,
            payload_offset: 0,
            payload_size: b.len(),
            integrity_trailer: [0; 32],
            children: vec![],
        };
        o.decode_stroke(b, &ParseLimits::default()).unwrap()
    };
    let base = decode(&payload);
    let plain = RenderOptions::default();
    let mut svg_opt = plain.clone();
    svg_opt.old_pencil_svg_profile = true;
    #[cfg(feature = "pdf")]
    let mut pdf_opt = plain.clone();
    #[cfg(feature = "pdf")]
    {
        pdf_opt.old_pencil_pdf_profile = true;
    }
    let base_doc = doc(base.clone());
    let expected = svg(&base_doc, &svg_opt);
    assert!(expected.contains("crispEdges"));
    #[cfg(feature = "pdf")]
    let expected_pdf =
        sdocx::render_document_pdf(&base_doc, &pdf_opt, &sdocx::PdfOptions::default()).unwrap();
    for width in [None, Some(0.0_f32), Some(20.0)] {
        let mut bytes = payload.clone();
        if let Some(value) = width {
            bytes[2639..2643].copy_from_slice(&value.to_le_bytes());
        } else {
            assert!(bytes[152] & 0x01 != 0);
            bytes[152] &= !0x01;
            bytes.drain(2639..2643);
            bytes[137..141].copy_from_slice(&2510_u32.to_le_bytes());
        }
        let actual = decode(&bytes);
        assert_eq!(actual.rendering.as_ref().unwrap().style.fixed_width, width);
        let mut adjusted = actual.clone();
        adjusted.rendering.as_mut().unwrap().style.fixed_width =
            base.rendering.as_ref().unwrap().style.fixed_width;
        assert_eq!(
            adjusted.rendering.as_ref().unwrap().style,
            base.rendering.as_ref().unwrap().style
        );
        assert_eq!(adjusted.points, base.points);
        assert_eq!(adjusted.pressures, base.pressures);
        assert_eq!(adjusted.timestamps, base.timestamps);
        assert_eq!(adjusted.tilts, base.tilts);
        assert_eq!(adjusted.orientations, base.orientations);
        let d = doc(actual);
        assert_eq!(svg(&d, &svg_opt), expected);
        #[cfg(feature = "pdf")]
        assert_eq!(
            sdocx::render_document_pdf(&d, &pdf_opt, &sdocx::PdfOptions::default()).unwrap(),
            expected_pdf
        );
    }
    let mut alt_payload = payload.clone();
    alt_payload[2627..2631].copy_from_slice(&0x40f130a0_u32.to_le_bytes());
    let alt = doc(decode(&alt_payload));
    alt_payload[2639..2643].copy_from_slice(&0.0_f32.to_le_bytes());
    let alt_width = decode(&alt_payload);
    assert_eq!(
        alt_width.rendering.as_ref().unwrap().style.fixed_width,
        Some(0.0)
    );
    let alt_width = doc(alt_width);
    assert_eq!(svg(&alt, &svg_opt), svg(&alt_width, &svg_opt));
    #[cfg(feature = "pdf")]
    assert_eq!(
        sdocx::render_document_pdf(&alt, &pdf_opt, &sdocx::PdfOptions::default()).unwrap(),
        sdocx::render_document_pdf(&alt_width, &pdf_opt, &sdocx::PdfOptions::default()).unwrap()
    );
    let mut enabled_payload = payload.clone();
    enabled_payload[148] |= 0x10;
    enabled_payload[2639..2643].copy_from_slice(&20.0_f32.to_le_bytes());
    let enabled = decode(&enabled_payload);
    assert!(enabled.rendering.as_ref().unwrap().properties.fixed_width);
    let enabled = doc(enabled);
    assert_eq!(svg(&enabled, &svg_opt), svg(&enabled, &plain));
    #[cfg(feature = "pdf")]
    assert_eq!(
        sdocx::render_document_pdf(&enabled, &pdf_opt, &sdocx::PdfOptions::default()).unwrap(),
        sdocx::render_document_pdf(&enabled, &plain, &sdocx::PdfOptions::default()).unwrap()
    );
    for value in [
        f32::from_bits(0x80000001),
        f32::from_bits(20f32.to_bits() + 1),
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
    ] {
        let mut bad = base.clone();
        bad.rendering.as_mut().unwrap().style.fixed_width = Some(value);
        let d = doc(bad);
        assert_eq!(svg(&d, &svg_opt), svg(&d, &plain));
        #[cfg(feature = "pdf")]
        assert_eq!(
            sdocx::render_document_pdf(&d, &pdf_opt, &sdocx::PdfOptions::default()).unwrap(),
            sdocx::render_document_pdf(&d, &plain, &sdocx::PdfOptions::default()).unwrap()
        );
    }
}

#[test]
fn explicit_serialized_pencil_stroke_type_zero_matches_implicit() {
    let mut payload = include_bytes!(
        "../../../conformance/old-pencil-counterexample/serialized-pencil-color.payload"
    )
    .to_vec();
    let decode = |b: &[u8]| {
        let o = StoredObject {
            object_type: ObjectType::Stroke,
            declared_size: (b.len() + 32) as u32,
            payload_offset: 0,
            payload_size: b.len(),
            integrity_trailer: [0; 32],
            children: vec![],
        };
        o.decode_stroke(b, &ParseLimits::default()).unwrap()
    };
    let implicit = decode(&payload);
    assert_eq!(
        implicit.rendering.as_ref().unwrap().style.stroke_type_raw,
        None
    );
    assert_eq!(payload[153], 0);
    payload[153] |= 1;
    payload.extend_from_slice(&0_u16.to_le_bytes());
    payload[137..141].copy_from_slice(&2516_u32.to_le_bytes());
    let explicit = decode(&payload);
    assert_eq!(
        explicit.rendering.as_ref().unwrap().style.stroke_type_raw,
        Some(0)
    );
    assert_eq!(explicit.points, implicit.points);
    assert_eq!(explicit.pressures, implicit.pressures);
    let plain = RenderOptions::default();
    let mut opt = plain.clone();
    opt.old_pencil_svg_profile = true;
    let implicit = doc(implicit);
    let explicit = doc(explicit);
    let expected = svg(&implicit, &opt);
    assert!(expected.contains("crispEdges"));
    assert_eq!(svg(&explicit, &opt), expected);
    #[cfg(feature = "pdf")]
    {
        let mut p = plain.clone();
        p.old_pencil_pdf_profile = true;
        assert_eq!(
            sdocx::render_document_pdf(&implicit, &p, &sdocx::PdfOptions::default()).unwrap(),
            sdocx::render_document_pdf(&explicit, &p, &sdocx::PdfOptions::default()).unwrap()
        );
    }
    for value in [1_u16, u16::MAX] {
        let len = payload.len();
        payload[len - 2..].copy_from_slice(&value.to_le_bytes());
        let bad = decode(&payload);
        assert_eq!(
            bad.rendering.as_ref().unwrap().style.stroke_type_raw,
            Some(value)
        );
        let bad = doc(bad);
        assert_eq!(svg(&bad, &opt), svg(&bad, &plain));
        #[cfg(feature = "pdf")]
        {
            let mut p = plain.clone();
            p.old_pencil_pdf_profile = true;
            assert_eq!(
                sdocx::render_document_pdf(&bad, &p, &sdocx::PdfOptions::default()).unwrap(),
                sdocx::render_document_pdf(&bad, &plain, &sdocx::PdfOptions::default()).unwrap()
            );
        }
    }
}
