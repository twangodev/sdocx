use super::{
    RenderOptions, SimpleBrushRenderProfile, render_document_svg, render_layout_page_replay_svg,
};
use crate::{
    BoundingBox, Color, Document, Page, Point, Stroke, StrokeProperties, StrokeRendering,
    StrokeStyle, layout_document,
};

fn stroke() -> Stroke {
    Stroke {
        rendering: Some(StrokeRendering {
            metadata: None,
            pen_name: Some("com.samsung.android.sdk.pen.pen.preload.Brush".into()),
            advanced_settings: None,
            tool_type_raw: 2,
            properties: StrokeProperties {
                compressed: true,
                replay_only: false,
                stylus_channels: false,
                eraser: false,
                fixed_width: false,
                millisecond_timestamps: false,
                top_layer_pen: false,
                alpha_lock: false,
                binary_added: true,
                generated: false,
                fixed_opacity: false,
                rainbow_effect: false,
                straighten: false,
                reveal_mode: false,
            },
            style: StrokeStyle {
                color_argb: Some(0x8044_aaee),
                pen_size: Some(8.0),
                ..StrokeStyle::default()
            },
        }),
        bbox: BoundingBox::default(),
        points: vec![
            Point { x: 10.0, y: 20.0 },
            Point { x: 20.0, y: 20.0 },
            Point { x: 30.0, y: 25.0 },
        ],
        pressures: vec![0.5, 0.25, 0.75],
        timestamps: vec![],
        tilts: vec![],
        orientations: vec![],
        color: Some(Color {
            r: 0x12,
            g: 0x34,
            b: 0x56,
        }),
        pen_width: 8.0,
    }
}

fn document(stroke: Stroke) -> Document {
    Document {
        pages: vec![Page {
            uuid: "simplebrush".into(),
            width: 100,
            height: 100,
            content_bbox: BoundingBox::default(),
            background_color: None,
            template: None,
            background: Default::default(),
            objects: vec![stroke.into()],
        }],
        metadata: Default::default(),
    }
}

fn options() -> RenderOptions {
    RenderOptions {
        simplebrush_profile: Some(SimpleBrushRenderProfile::HostV25),
        ..RenderOptions::default()
    }
}

#[test]
fn compressed_frame_reaches_opted_in_dispatch() {
    // The base frame comes from the retained native channel capture. This
    // three-point stroke frame and its resolved pen metadata are synthetic;
    // they establish decoder-to-dispatch reachability, not saved-note parity.
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/object-stroke-channels-native.json"
    ))
    .unwrap();
    let hex = fixture["source"]["payload_hex"].as_str().unwrap();
    let source: Vec<u8> = hex
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect();
    let offset = fixture["source"]["frame_offset"].as_u64().unwrap() as usize;
    let mut fixed = Vec::new();
    fixed.extend_from_slice(&3_u16.to_le_bytes());
    fixed.extend_from_slice(&10_f64.to_le_bytes());
    fixed.extend_from_slice(&20_f64.to_le_bytes());
    for word in [320_u16, 0, 320, 160] {
        fixed.extend_from_slice(&word.to_le_bytes());
    }
    fixed.extend_from_slice(&0.5_f32.to_le_bytes());
    for word in [0x8400_u16, 0x0800] {
        fixed.extend_from_slice(&word.to_le_bytes());
    }
    fixed.extend_from_slice(&0_i32.to_le_bytes());
    for word in [1_u16, 1, 2] {
        fixed.extend_from_slice(&word.to_le_bytes());
    }
    let size = 14 + fixed.len();
    let mut payload = source[..offset].to_vec();
    payload.extend_from_slice(&(size as u32).to_le_bytes());
    payload.extend_from_slice(&1_i16.to_le_bytes());
    payload.extend_from_slice(&0_u32.to_le_bytes());
    payload.extend_from_slice(&[2, 1, 4, 0]);
    payload.extend_from_slice(&fixed);
    let mut decoded =
        crate::decode::decode_stroke(&payload, &crate::ParseLimits::default()).unwrap();
    assert_eq!(decoded.points, stroke().points);
    assert_eq!(decoded.pressures, stroke().pressures);
    assert_eq!(decoded.timestamps, [0, 1, 2]);
    let rendering = decoded.rendering.as_mut().unwrap();
    rendering.pen_name = Some("com.samsung.android.sdk.pen.pen.preload.Brush".into());
    rendering.style.pen_size = Some(8.0);
    rendering.style.color_argb = Some(0x8044_aaee);
    decoded.pen_width = 8.0;
    assert!(crate::ink::simplebrush::prepare_stroke(&decoded, true, 32_768).is_ok());
    let doc = document(decoded);
    assert_ne!(
        render_document_svg(&doc, &options())[0].svg,
        render_document_svg(&doc, &RenderOptions::default())[0].svg
    );
}

#[test]
fn size_four_admits_three_and_four_points() {
    let mut source = stroke();
    source.pen_width = 4.0;
    source.rendering.as_mut().unwrap().style.pen_size = Some(4.0);
    let doc = document(source.clone());
    assert_ne!(
        render_document_svg(&doc, &options()).remove(0).svg,
        render_document_svg(&doc, &RenderOptions::default())
            .remove(0)
            .svg
    );
    source.points.push(Point { x: 39.0, y: 31.0 });
    source.pressures.push(f64::from(0.4_f32));
    let doc = document(source);
    assert_ne!(
        render_document_svg(&doc, &options()).remove(0).svg,
        render_document_svg(&doc, &RenderOptions::default())
            .remove(0)
            .svg
    );
}

#[test]
fn size_sixteen_admits_five_points() {
    let mut source = stroke();
    source.pen_width = 16.0;
    source.rendering.as_mut().unwrap().style.pen_size = Some(16.0);
    assert!(crate::ink::simplebrush::prepare_stroke(&source, true, 32_768).is_ok());
    source.points.push(Point { x: 39.0, y: 31.0 });
    source.pressures.push(f64::from(0.4_f32));
    assert!(crate::ink::simplebrush::prepare_stroke(&source, true, 32_768).is_ok());
    source.points.push(Point { x: 45.0, y: 35.0 });
    source.pressures.push(f64::from(0.4_f32));
    assert!(crate::ink::simplebrush::prepare_stroke(&source, true, 32_768).is_ok());
    let doc = document(source);
    assert_ne!(
        render_document_svg(&doc, &options()).remove(0).svg,
        render_document_svg(&doc, &RenderOptions::default())
            .remove(0)
            .svg
    );
}

#[test]
fn profile_accepts_six_points_across_captured_sizes() {
    let mut source = stroke();
    for point in [
        Point { x: 39.0, y: 31.0 },
        Point { x: 45.0, y: 35.0 },
        Point { x: 52.0, y: 39.0 },
    ] {
        source.points.push(point);
        source.pressures.push(f64::from(0.4_f32));
    }
    for size in [4.0, 8.0, 12.0, 16.0] {
        source.pen_width = size;
        source.rendering.as_mut().unwrap().style.pen_size = Some(size);
        assert!(crate::ink::simplebrush::prepare_stroke(&source, true, 32_768).is_ok());
    }
    for i in 6..320 {
        source.points.push(Point {
            x: 60.0 + (i as f64) * 8.0,
            y: 42.0,
        });
        source.pressures.push(f64::from(0.4_f32));
    }
    assert!(crate::ink::simplebrush::prepare_stroke(&source, true, 32_768).is_ok());
    source.points.push(Point { x: 200.0, y: 42.0 });
    source.pressures.push(f64::from(0.4_f32));
    assert!(crate::ink::simplebrush::prepare_stroke(&source, true, 32_768).is_err());
}

#[cfg(feature = "serde")]
#[test]
fn options_default_for_legacy_data_and_roundtrip_profile() {
    let mut legacy = serde_json::to_value(RenderOptions::default()).unwrap();
    legacy
        .as_object_mut()
        .unwrap()
        .remove("simplebrush_profile");
    let restored: RenderOptions = serde_json::from_value(legacy).unwrap();
    assert_eq!(restored.simplebrush_profile, None);
    let restored: RenderOptions =
        serde_json::from_value(serde_json::to_value(options()).unwrap()).unwrap();
    assert_eq!(
        restored.simplebrush_profile,
        Some(SimpleBrushRenderProfile::HostV25)
    );
}

#[test]
fn opted_in_paint_uses_explicit_color_and_unsupported_setting_falls_back() {
    let mut source = stroke();
    assert!(crate::ink::simplebrush::prepare_stroke(&source, true, 32_768).is_ok());
    let doc = document(source.clone());
    let profiled = render_document_svg(&doc, &options()).remove(0).svg;
    let baseline = render_document_svg(&doc, &RenderOptions::default())
        .remove(0)
        .svg;
    assert_ne!(profiled, baseline);
    assert!(profiled.contains("#123456"));
    source.rendering.as_mut().unwrap().advanced_settings = Some(String::new());
    let empty_setting = document(source.clone());
    assert_eq!(
        render_document_svg(&empty_setting, &options())
            .remove(0)
            .svg,
        profiled
    );
    #[cfg(feature = "pdf")]
    assert_eq!(
        crate::render_document_pdf(&empty_setting, &options(), &Default::default()).unwrap(),
        crate::render_document_pdf(&doc, &options(), &Default::default()).unwrap()
    );
    source.rendering.as_mut().unwrap().advanced_settings = Some("1;".into());
    let literal_one = document(source.clone());
    assert_eq!(
        render_document_svg(&literal_one, &options()).remove(0).svg,
        profiled
    );
    #[cfg(feature = "pdf")]
    assert_eq!(
        crate::render_document_pdf(&literal_one, &options(), &Default::default()).unwrap(),
        crate::render_document_pdf(&doc, &options(), &Default::default()).unwrap()
    );
    source.rendering.as_mut().unwrap().advanced_settings = Some("2;".into());
    let unsupported = document(source);
    assert_eq!(
        render_document_svg(&unsupported, &options()).remove(0).svg,
        render_document_svg(&unsupported, &RenderOptions::default())
            .remove(0)
            .svg
    );
}

#[test]
fn replay_remains_on_generic_route() {
    let doc = document(stroke());
    let layout = layout_document(&doc);
    let profiled = render_layout_page_replay_svg(&doc, &layout, 0, &options())
        .unwrap()
        .svg;
    let baseline = render_layout_page_replay_svg(&doc, &layout, 0, &RenderOptions::default())
        .unwrap()
        .svg;
    assert_eq!(profiled, baseline);
    assert!(profiled.contains("data-replay-part"));
}

#[test]
fn unsupported_effects_types_and_channels_use_whole_stroke_fallback() {
    let mutations: &[fn(&mut Stroke)] = &[
        |s| s.rendering.as_mut().unwrap().properties.rainbow_effect = true,
        |s| s.rendering.as_mut().unwrap().properties.fixed_opacity = true,
        |s| s.rendering.as_mut().unwrap().properties.stylus_channels = true,
        |s| s.rendering.as_mut().unwrap().style.stroke_type_raw = Some(1),
        |s| s.rendering.as_mut().unwrap().style.color_type_raw = Some(1),
        |s| {
            s.pressures.pop();
        },
        |s| s.tilts = vec![0.0],
        |s| s.orientations = vec![0.0, f64::NAN, 0.0],
        |s| s.pressures[1] = f64::INFINITY,
    ];
    for mutate in mutations {
        let mut source = stroke();
        mutate(&mut source);
        assert!(crate::ink::simplebrush::prepare_stroke(&source, true, 32_768).is_err());
        let doc = document(source);
        assert_eq!(
            render_document_svg(&doc, &options()).remove(0).svg,
            render_document_svg(&doc, &RenderOptions::default())
                .remove(0)
                .svg
        );
    }
}

#[test]
fn rejected_vector_samples_do_not_mutate_scene_or_consume_definition_ids() {
    use super::vector::{Scene, Svg};

    let mut expected = Scene::new(Svg::new());
    let mut actual = Scene::new(Svg::new());
    let valid = [[10.0, 20.0, 3.0]];
    assert!(super::simplebrush::render(
        &mut expected,
        &valid,
        "#123456",
        None
    ));
    assert!(super::simplebrush::render(
        &mut actual,
        &valid,
        "#123456",
        None
    ));
    for invalid in [
        vec![],
        vec![[f32::NAN, 20.0, 3.0]],
        vec![[10.0, 20.0, 0.0]],
        vec![[10.0, 20.0, 3.0]; 32_769],
    ] {
        assert!(!super::simplebrush::render(
            &mut actual,
            &invalid,
            "#123456",
            None
        ));
    }
    assert!(!super::simplebrush::render(
        &mut actual,
        &valid,
        "invalid-color",
        None
    ));
    assert!(super::simplebrush::render(
        &mut expected,
        &valid,
        "#123456",
        None
    ));
    assert!(super::simplebrush::render(
        &mut actual,
        &valid,
        "#123456",
        None
    ));
    assert_eq!(actual.finish(), expected.finish());
}

#[test]
fn stamp_budget_is_cumulative_and_rejection_is_atomic() {
    use super::vector::{Scene, Svg};
    use std::cell::Cell;

    fn render_one(source: &Stroke, remaining: &Cell<usize>, profile: bool, scaled: bool) -> String {
        let doc = document(source.clone());
        let theme = super::RenderTheme::resolve(
            &doc.pages[0],
            &doc.metadata,
            RenderOptions::default().color_mode,
        );
        let mut scene = Scene::new(Svg::new());
        let mut session = crate::ink::calligraphy::CalligraphySession::new();
        super::render_stroke(
            &mut scene,
            source,
            theme,
            None,
            &mut session,
            true,
            [100, 100],
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            &std::cell::Cell::new(65_536),
            None,
            false,
            false,
            None,
            None,
            profile.then_some(SimpleBrushRenderProfile::HostV25),
            scaled,
            None,
            false,
            None,
            None,
            &std::cell::Cell::new(32768),
            &std::cell::Cell::new(512),
            &std::cell::Cell::new(8192),
            remaining,
            &std::cell::Cell::new(usize::MAX),
            None,
        );
        scene.finish()
    }

    let mut source = stroke();
    let mut small = source.clone();
    for i in 3..224 {
        source.points.push(Point {
            x: (i * 8) as f64,
            y: (4 + i % 3 * 4) as f64,
        });
        source.pressures.push(f64::from(0.4_f32));
    }
    let count = crate::ink::simplebrush::prepare_stroke(&source, true, 32_768)
        .unwrap()
        .samples
        .len();
    small.points[1] = Point { x: 12.0, y: 20.0 };
    small.points[2] = Point { x: 14.0, y: 20.0 };
    let small_count = crate::ink::simplebrush::prepare_stroke(&small, true, 32_768)
        .unwrap()
        .samples
        .len();
    assert!(small_count < count);

    let budget = Cell::new(count + small_count);
    let native = render_one(&source, &budget, true, false);
    assert_eq!(budget.get(), small_count);
    let rejected = render_one(&source, &budget, true, false);
    assert_eq!(
        budget.get(),
        small_count,
        "rejection must not consume budget"
    );
    assert_eq!(
        rejected,
        render_one(&source, &Cell::new(0), false, false),
        "an exhausted stroke must use the entire generic fallback"
    );
    assert_ne!(native, rejected);
    let small_native = render_one(&small, &budget, true, false);
    assert_eq!(budget.get(), 0);
    assert_ne!(
        small_native,
        render_one(&small, &Cell::new(0), false, false)
    );

    // A new page composition starts with a fresh budget.
    let fresh_page_budget = Cell::new(count + small_count);
    assert_eq!(render_one(&source, &fresh_page_budget, true, false), native);
    assert_eq!(fresh_page_budget.get(), small_count);

    for point_count in [129, 256] {
        let mut long = stroke();
        long.points = (0..point_count)
            .map(|i| Point {
                x: 10.0 + 2.0 * i as f64,
                y: 20.0 + 2.0 * (i % 5) as f64,
            })
            .collect();
        long.pressures = vec![0.5; point_count];
        long.pen_width = 12.0;
        long.rendering.as_mut().unwrap().style.pen_size = Some(12.0);
        let n = crate::ink::simplebrush::prepare_stroke_for_scale(&long, true, 32_768, true)
            .unwrap()
            .samples
            .len();
        let budget = Cell::new(n - 1);
        let rejected = render_one(&long, &budget, false, true);
        assert_eq!(budget.get(), n - 1);
        assert_eq!(rejected, render_one(&long, &Cell::new(0), false, false));
        let budget = Cell::new(n);
        assert_ne!(render_one(&long, &budget, false, true), rejected);
        assert_eq!(budget.get(), 0);
    }

    source.points.truncate(32);
    source.pressures.truncate(32);
    source.pen_width = 12.0;
    source.rendering.as_mut().unwrap().style.pen_size = Some(12.0);
    let long_count = crate::ink::simplebrush::prepare_stroke(&source, true, 32_768)
        .unwrap()
        .samples
        .len();
    let budget = Cell::new(long_count - 1);
    let rejected = render_one(&source, &budget, false, true);
    assert_eq!(budget.get(), long_count - 1);
    assert_eq!(rejected, render_one(&source, &Cell::new(0), false, false));
    let budget = Cell::new(long_count);
    let admitted = render_one(&source, &budget, false, true);
    assert_eq!(budget.get(), 0);
    assert_ne!(admitted, rejected);
}
#[cfg(feature = "pdf")]
#[test]
fn simplebrush_pdf_slice_scene_matches_full_page_across_stamp() {
    fn raster(svg: &str, height: u32) -> resvg::tiny_skia::Pixmap {
        let tree = resvg::usvg::Tree::from_str(svg, &Default::default()).unwrap();
        let mut pixels = resvg::tiny_skia::Pixmap::new(100, height).unwrap();
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::identity(),
            &mut pixels.as_mut(),
        );
        pixels
    }

    let mut source = stroke();
    for point in &mut source.points {
        point.y += 30.0;
    }
    let paint = crate::ink::simplebrush::prepare_stroke(&source, true, 32_768).unwrap();
    assert!(
        paint
            .samples
            .iter()
            .any(|sample| sample[1] - sample[2] < 50.0 && sample[1] + sample[2] > 50.0),
        "fixture must contain a stamp crossing the split"
    );
    let doc = document(source);
    let layout = layout_document(&doc);
    let page = &layout.pages[0];
    let requests = [
        super::PageSceneRequest::full_page(page),
        super::PageSceneRequest::sliced(page, 0, 50, true).unwrap(),
        super::PageSceneRequest::sliced(page, 50, 50, true).unwrap(),
    ];
    let mut cache = super::DocumentTextCache::default();
    let scenes = cache.render_layout_page_scene_requests(
        &doc,
        &requests,
        &options(),
        &super::fonts::FontBook::default(),
        &mut |_| {},
    );
    for scene in &scenes {
        assert!(
            scene.page.svg.contains("radialGradient"),
            "experimental route must be active"
        );
    }
    let vector_elements = |svg: &str| {
        let document = roxmltree::Document::parse(svg).unwrap();
        document
            .descendants()
            .filter(|node| node.has_tag_name("radialGradient") || node.has_tag_name("circle"))
            .map(|node| {
                let y = node.attribute("cy").unwrap().parse::<f64>().unwrap();
                let radius = node.attribute("r").unwrap().parse::<f64>().unwrap();
                let stops = node
                    .children()
                    .filter(|child| child.has_tag_name("stop"))
                    .map(|stop| {
                        format!(
                            "{:?}:{:?}:{:?}",
                            stop.attribute("offset"),
                            stop.attribute("stop-color"),
                            stop.attribute("stop-opacity")
                        )
                    })
                    .collect::<Vec<_>>();
                let signature = format!(
                    "{}:{:?}:{y}:{radius}:{stops:?}",
                    node.tag_name().name(),
                    node.attribute("cx")
                );
                (y, radius, signature)
            })
            .collect::<Vec<_>>()
    };
    let expected_vectors = vector_elements(&scenes[0].page.svg);
    assert_eq!(expected_vectors.len(), paint.samples.len() * 2);
    for (scene, (top, bottom)) in scenes[1..].iter().zip([(0.0, 50.0), (50.0, 100.0)]) {
        let visible: Vec<_> = expected_vectors
            .iter()
            .filter(|(y, radius, _)| y + radius >= top && y - radius <= bottom)
            .cloned()
            .collect();
        assert_eq!(vector_elements(&scene.page.svg), visible);
    }
    let full = raster(&scenes[0].page.svg, 100);
    let upper = raster(&scenes[1].page.svg, 50);
    let lower = raster(&scenes[2].page.svg, 50);
    let row_bytes = 100 * 4;
    for (name, actual, expected) in [
        ("upper", upper.data(), &full.data()[..50 * row_bytes]),
        ("lower", lower.data(), &full.data()[50 * row_bytes..]),
    ] {
        let differences: Vec<_> = actual
            .iter()
            .zip(expected)
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .collect();
        let max = differences
            .iter()
            .map(|(_, (a, b))| a.abs_diff(**b))
            .max()
            .unwrap_or(0);
        eprintln!(
            "{name}: {} differing bytes, max {max}, first {:?}",
            differences.len(),
            differences.first()
        );
        // resvg may quantize the clipped gradient differently when the root
        // viewBox changes. Geometry is exact above; bound the residual pixels.
        assert!(
            max <= 8 && differences.len() <= 24,
            "{name}: {} differing channels, max {max}",
            differences.len()
        );
    }
}

#[test]
fn tool_zero_native_controls_reach_public_exports() {
    use std::fmt::Write;
    let mut receipt: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/simplebrush-tool0-native.json"
    ))
    .unwrap();
    let large: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/simplebrush-tool0-large-native.json"
    ))
    .unwrap();
    receipt["controls"]
        .as_object_mut()
        .unwrap()
        .extend(large["controls"].as_object().unwrap().clone());
    let multi_large: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/simplebrush-tool0-multi-large-native.json"
    ))
    .unwrap();
    receipt["controls"]
        .as_object_mut()
        .unwrap()
        .extend(multi_large["controls"].as_object().unwrap().clone());
    let size32: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/simplebrush-tool0-size32-native.json"
    ))
    .unwrap();
    receipt["controls"]
        .as_object_mut()
        .unwrap()
        .extend(size32["controls"].as_object().unwrap().clone());
    let terminal: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/simplebrush-tool0-terminal-native.json"
    ))
    .unwrap();
    receipt["controls"]
        .as_object_mut()
        .unwrap()
        .extend(terminal["controls"].as_object().unwrap().clone());
    let vertical_bent: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/simplebrush-tool0-vertical-bent-native.json"
    ))
    .unwrap();
    receipt["controls"]
        .as_object_mut()
        .unwrap()
        .extend(vertical_bent["controls"].as_object().unwrap().clone());
    let decode = |value: &serde_json::Value| {
        let hex = value.as_str().unwrap();
        let mut bytes = [0_u8; 4];
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).unwrap();
        }
        f32::from_le_bytes(bytes)
    };
    for name in [
        "vertical",
        "reverse",
        "curve",
        "two-4",
        "two-12",
        "curve-4",
        "curve-12",
        "short",
        "seven",
        "sixteen",
        "two-20",
        "two-32",
        "curve-20",
        "curve-32",
        "short-terminal",
        "boundary",
        "excursion-return",
        "vertical-bent-short-terminal",
        "extra-reverse4",
        "extra-return3",
        "extra-skip4",
        "extra-vertical3",
    ] {
        let input = &receipt["controls"][name]["input"];
        let size = decode(&input["pen_size_f32_bits"]);
        let points: Vec<[f32; 2]> = input["points_f32_bits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|point| [decode(&point[0]), decode(&point[1])])
            .collect();
        let pressures: Vec<f32> = input["pressure_f32_bits"]
            .as_array()
            .unwrap()
            .iter()
            .map(decode)
            .collect();
        let samples = crate::ink::simplebrush::generate(crate::ink::simplebrush::NativeInput {
            points: &points,
            pressures: &pressures,
            size,
            tool_type: 0,
            curve_enabled: true,
            max_samples: 100_000,
        })
        .unwrap();
        let mut hex = String::new();
        for value in samples.iter().flatten() {
            for byte in value.to_le_bytes() {
                write!(hex, "{byte:02x}").unwrap();
            }
        }
        assert_eq!(
            hex,
            receipt["controls"][name]["geometry_hex"].as_str().unwrap()
        );
        let mut zero = stroke();
        zero.pen_width = size;
        zero.rendering.as_mut().unwrap().style.pen_size = Some(size);
        zero.points = points
            .iter()
            .map(|p| Point {
                x: f64::from(p[0]),
                y: f64::from(p[1]),
            })
            .collect();
        zero.pressures = pressures.iter().copied().map(f64::from).collect();
        zero.rendering.as_mut().unwrap().tool_type_raw = 0;
        let mut two = zero.clone();
        two.rendering.as_mut().unwrap().tool_type_raw = 2;
        let zero_doc = document(zero);
        let two_doc = document(two);
        let zero_svg = render_document_svg(&zero_doc, &options())[0].svg.clone();
        assert_eq!(zero_svg, render_document_svg(&two_doc, &options())[0].svg);
        assert_ne!(
            zero_svg,
            render_document_svg(&zero_doc, &RenderOptions::default())[0].svg
        );
        #[cfg(feature = "pdf")]
        assert_eq!(
            crate::render_document_pdf(&zero_doc, &options(), &Default::default()).unwrap(),
            crate::render_document_pdf(&two_doc, &options(), &Default::default()).unwrap()
        );
    }
}

#[cfg(feature = "pdf")]
#[test]
fn tool_zero_large_curve_svg_improves_against_native_frame() {
    for (size, native, max_error, support) in [
        (
            20.0,
            include_bytes!("../../../../conformance/simplebrush-tool0-multi-large-crop.rgba"),
            1_200,
            462,
        ),
        (
            32.0,
            include_bytes!("../../../../conformance/simplebrush-tool0-size32-crop.rgba"),
            1_500,
            698,
        ),
    ] {
        let mut input = stroke();
        input.rendering.as_mut().unwrap().tool_type_raw = 0;
        input.rendering.as_mut().unwrap().style.pen_size = Some(size);
        input.pen_width = size;
        input.color = Some(Color {
            r: 0x44,
            g: 0xaa,
            b: 0xee,
        });
        input.points = [(0., 4.), (8., 8.), (16., 4.), (24., 12.)]
            .into_iter()
            .map(|(x, y)| Point { x, y: 256. - y })
            .collect();
        input.pressures = vec![0.25, 0.5, 0.75, 1.0];
        let mut doc = document(input);
        doc.pages[0].width = 256;
        doc.pages[0].height = 256;
        let metrics = |options: &RenderOptions| {
            let layout = crate::layout_document(&doc);
            let request =
                crate::render::PageSceneRequest::sliced(&layout.pages[0], 0, 256, false).unwrap();
            let svg = crate::render::DocumentTextCache::default()
                .render_layout_page_scene_requests(
                    &doc,
                    &[request],
                    options,
                    &crate::render::fonts::FontBook::default(),
                    &mut |_| {},
                )
                .remove(0)
                .page
                .svg;
            let tree = resvg::usvg::Tree::from_str(&svg, &Default::default()).unwrap();
            let mut pixmap = resvg::tiny_skia::Pixmap::new(256, 256).unwrap();
            resvg::render(
                &tree,
                resvg::tiny_skia::Transform::identity(),
                &mut pixmap.as_mut(),
            );
            let mut error = 0_u64;
            let mut intersection = 0_usize;
            let mut union = 0_usize;
            for y in 0..48 {
                for x in 0..48 {
                    let expected = native[(y * 48 + x) * 4 + 3];
                    let actual = pixmap.data()[((255 - y) * 256 + x) * 4 + 3];
                    error += u64::from(expected.abs_diff(actual));
                    intersection += usize::from(expected != 0 && actual != 0);
                    union += usize::from(expected != 0 || actual != 0);
                }
            }
            (error, intersection, union)
        };
        let profiled = metrics(&options());
        let fallback = metrics(&RenderOptions::default());
        eprintln!(
            "native alpha error, intersection, union: profile {profiled:?}, fallback {fallback:?}"
        );
        assert!(profiled.0 <= max_error && profiled.0 < fallback.0);
        assert_eq!((profiled.1, profiled.2), (support, support));
        assert!(profiled.1 > fallback.1);
    }
}

#[cfg(feature = "pdf")]
#[test]
fn tool_zero_short_terminal_svg_improves_against_native_frame() {
    for (second_x, third_x, third_y, native) in [
        (
            20.0,
            20.5,
            10.5,
            include_bytes!("../../../../conformance/simplebrush-tool0-short-terminal-crop.rgba"),
        ),
        (
            11.5,
            12.0,
            10.0,
            include_bytes!("../../../../conformance/simplebrush-tool0-boundary-crop.rgba"),
        ),
    ] {
        let mut input = stroke();
        input.rendering.as_mut().unwrap().tool_type_raw = 0;
        input.color = Some(Color {
            r: 0x44,
            g: 0xaa,
            b: 0xee,
        });
        input.points = [(10.0, 10.0), (second_x, 10.0), (third_x, third_y)]
            .into_iter()
            .map(|(x, y)| Point { x, y: 256.0 - y })
            .collect();
        input.pressures = vec![0.75, 0.25, 1.0];
        let mut doc = document(input);
        doc.pages[0].width = 256;
        doc.pages[0].height = 256;
        let error = |options: &RenderOptions| {
            let layout = crate::layout_document(&doc);
            let request =
                crate::render::PageSceneRequest::sliced(&layout.pages[0], 0, 256, false).unwrap();
            let svg = crate::render::DocumentTextCache::default()
                .render_layout_page_scene_requests(
                    &doc,
                    &[request],
                    options,
                    &crate::render::fonts::FontBook::default(),
                    &mut |_| {},
                )
                .remove(0)
                .page
                .svg;
            let tree = resvg::usvg::Tree::from_str(&svg, &Default::default()).unwrap();
            let mut pixmap = resvg::tiny_skia::Pixmap::new(256, 256).unwrap();
            resvg::render(
                &tree,
                resvg::tiny_skia::Transform::identity(),
                &mut pixmap.as_mut(),
            );
            (0..48)
                .flat_map(|y| (0..48).map(move |x| (x, y)))
                .map(|(x, y)| {
                    u64::from(
                        native[(y * 48 + x) * 4 + 3]
                            .abs_diff(pixmap.data()[((255 - y) * 256 + x) * 4 + 3]),
                    )
                })
                .sum::<u64>()
        };
        let profile = error(&options());
        let fallback = error(&RenderOptions::default());
        eprintln!("profile {profile}, fallback {fallback}");
        assert!(profile < fallback);
    }
}

#[test]
fn tool_zero_unsupported_size_and_terminal_step_fall_back() {
    let mut input = stroke();
    input.rendering.as_mut().unwrap().tool_type_raw = 0;
    input.points[1] = Point { x: 11.0, y: 20.0 };
    input.points.push(Point { x: 30.5, y: 25.5 });
    input.pressures.push(0.5);
    let terminal = document(input.clone());
    assert_eq!(
        render_document_svg(&terminal, &options())[0].svg,
        render_document_svg(&terminal, &RenderOptions::default())[0].svg
    );
    input.points.pop();
    input.pressures.pop();
    input.pen_width = 33.0;
    input.rendering.as_mut().unwrap().style.pen_size = Some(33.0);
    let size = document(input);
    assert_eq!(
        render_document_svg(&size, &options())[0].svg,
        render_document_svg(&size, &RenderOptions::default())[0].svg
    );
}

#[test]
fn two_pixel_profile_is_explicit_and_conflicts_fall_back() {
    let doc = document(stroke());
    let two = RenderOptions {
        simplebrush_two_pixels_per_page_unit: true,
        ..Default::default()
    };
    let baseline = render_document_svg(&doc, &RenderOptions::default())[0]
        .svg
        .clone();
    let one = render_document_svg(&doc, &options())[0].svg.clone();
    let scaled = render_document_svg(&doc, &two)[0].svg.clone();
    assert_ne!(scaled, baseline);
    assert_ne!(scaled, one);
    let conflict = RenderOptions {
        simplebrush_profile: Some(SimpleBrushRenderProfile::HostV25),
        ..two.clone()
    };
    assert_eq!(render_document_svg(&doc, &conflict)[0].svg, baseline);
    for (tool, size, setting) in [
        (0, 2.99, None),
        (0, 32.01, None),
        (3, 2.99, None),
        (3, 32.01, None),
        (2, 2.99, None),
        (2, 32.01, None),
    ] {
        let mut unsupported = stroke();
        let rendering = unsupported.rendering.as_mut().unwrap();
        rendering.tool_type_raw = tool;
        rendering.advanced_settings = setting.map(str::to_owned);
        rendering.style.pen_size = Some(size);
        unsupported.pen_width = size;
        let unsupported = document(unsupported);
        assert_eq!(
            render_document_svg(&unsupported, &two)[0].svg,
            render_document_svg(&unsupported, &RenderOptions::default())[0].svg
        );
    }
    for tool in [0, 2, 3] {
        for setting in ["2;", "1", "1; "] {
            let mut unsupported = stroke();
            let rendering = unsupported.rendering.as_mut().unwrap();
            rendering.tool_type_raw = tool;
            rendering.advanced_settings = Some(setting.into());
            let unsupported = document(unsupported);
            assert_eq!(
                render_document_svg(&unsupported, &two)[0].svg,
                render_document_svg(&unsupported, &RenderOptions::default())[0].svg
            );
        }
    }
    let mut unsupported = stroke();
    unsupported.points = (0..257)
        .map(|i| Point {
            x: i as f64 * 2.0,
            y: 20.0,
        })
        .collect();
    unsupported.pressures = vec![0.5; 257];
    let unsupported = document(unsupported);
    assert_eq!(
        render_document_svg(&unsupported, &two)[0].svg,
        render_document_svg(&unsupported, &RenderOptions::default())[0].svg
    );
}

#[test]
fn two_pixel_tool_two_size_boundaries_reach_svg_and_pdf() {
    let two = RenderOptions {
        simplebrush_two_pixels_per_page_unit: true,
        ..Default::default()
    };
    for (tool, size) in [(2, 3.0), (2, 4.0), (2, 32.0), (3, 3.0), (3, 32.0)] {
        let mut source = stroke();
        source.rendering.as_mut().unwrap().tool_type_raw = tool;
        source.pen_width = size;
        source.rendering.as_mut().unwrap().style.pen_size = Some(size);
        let doc = document(source);
        assert_ne!(
            render_document_svg(&doc, &two)[0].svg,
            render_document_svg(&doc, &RenderOptions::default())[0].svg
        );
        #[cfg(feature = "pdf")]
        assert_ne!(
            crate::render_document_pdf(&doc, &two, &Default::default()).unwrap(),
            crate::render_document_pdf(&doc, &RenderOptions::default(), &Default::default())
                .unwrap()
        );
    }
}

#[test]
fn two_pixel_tool_three_three_point_gate_falls_back_outside_bounds() {
    let two = RenderOptions {
        simplebrush_two_pixels_per_page_unit: true,
        ..Default::default()
    };
    for count in [2, 4] {
        for size in [3.0, 32.0] {
            let mut source = stroke();
            source.points.truncate(count);
            source.pressures.truncate(count);
            if count == 4 {
                source.points.push(Point { x: 40.0, y: 20.0 });
                source.pressures.push(0.5);
            }
            source.pen_width = size;
            let rendering = source.rendering.as_mut().unwrap();
            rendering.tool_type_raw = 3;
            rendering.style.pen_size = Some(size);
            let doc = document(source);
            assert_eq!(
                render_document_svg(&doc, &two)[0].svg,
                render_document_svg(&doc, &RenderOptions::default())[0].svg
            );
        }
    }
    let mut source = stroke();
    source.pen_width = 3.0;
    let rendering = source.rendering.as_mut().unwrap();
    rendering.tool_type_raw = 3;
    rendering.style.pen_size = Some(3.0);
    let conflict = RenderOptions {
        simplebrush_profile: Some(SimpleBrushRenderProfile::HostV25),
        ..two.clone()
    };
    let eligible = document(source.clone());
    assert_eq!(
        render_document_svg(&eligible, &conflict)[0].svg,
        render_document_svg(&eligible, &RenderOptions::default())[0].svg
    );
    source.rendering.as_mut().unwrap().properties.rainbow_effect = true;
    let doc = document(source);
    assert_eq!(
        render_document_svg(&doc, &two)[0].svg,
        render_document_svg(&doc, &RenderOptions::default())[0].svg
    );
}

#[test]
fn two_pixel_tool_zero_and_three_match_two() {
    let two = RenderOptions {
        simplebrush_two_pixels_per_page_unit: true,
        ..Default::default()
    };
    for size in [8.0, 12.0, 20.0] {
        let mut source = stroke();
        source.pen_width = size;
        source.rendering.as_mut().unwrap().style.pen_size = Some(size);
        let reference = document(source.clone());
        let svg = render_document_svg(&reference, &two)[0].svg.clone();
        #[cfg(feature = "pdf")]
        let pdf = crate::render_document_pdf(&reference, &two, &Default::default()).unwrap();
        assert_ne!(
            svg,
            render_document_svg(&reference, &RenderOptions::default())[0].svg
        );
        for tool in [0, 3] {
            source.rendering.as_mut().unwrap().tool_type_raw = tool;
            let doc = document(source.clone());
            assert_eq!(render_document_svg(&doc, &two)[0].svg, svg);
            #[cfg(feature = "pdf")]
            assert_eq!(
                crate::render_document_pdf(&doc, &two, &Default::default()).unwrap(),
                pdf
            );
        }
    }
}

#[cfg(feature = "serde")]
#[test]
fn two_pixel_profile_deserializes_old_options_and_roundtrips() {
    let mut value = serde_json::to_value(RenderOptions::default()).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .remove("simplebrush_two_pixels_per_page_unit");
    let old: RenderOptions = serde_json::from_value(value).unwrap();
    assert!(!old.simplebrush_two_pixels_per_page_unit);
    let value = RenderOptions {
        simplebrush_two_pixels_per_page_unit: true,
        ..Default::default()
    };
    let restored: RenderOptions =
        serde_json::from_value(serde_json::to_value(value).unwrap()).unwrap();
    assert!(restored.simplebrush_two_pixels_per_page_unit);
}

#[test]
fn two_pixel_profile_falls_back_for_replay_and_slices() {
    for tool in [0, 2, 3] {
        let mut source = stroke();
        source.pen_width = 3.0;
        source.rendering.as_mut().unwrap().style.pen_size = Some(3.0);
        source.rendering.as_mut().unwrap().tool_type_raw = tool;
        let doc = document(source);
        let layout = layout_document(&doc);
        let two = RenderOptions {
            simplebrush_two_pixels_per_page_unit: true,
            ..Default::default()
        };
        let baseline = RenderOptions::default();
        let replay = |options: &RenderOptions| {
            render_layout_page_replay_svg(&doc, &layout, 0, options)
                .unwrap()
                .svg
        };
        assert_eq!(replay(&two), replay(&baseline));
        #[cfg(feature = "pdf")]
        {
            let request = super::PageSceneRequest::sliced(&layout.pages[0], 0, 50, true).unwrap();
            let scene = |options: &RenderOptions| {
                super::DocumentTextCache::default()
                    .render_layout_page_scene_requests(
                        &doc,
                        &[request],
                        options,
                        &super::fonts::FontBook::default(),
                        &mut |_| {},
                    )
                    .remove(0)
                    .page
                    .svg
            };
            assert_eq!(scene(&two), scene(&baseline));
        }
    }
}

#[cfg(feature = "pdf")]
#[test]
fn two_pixel_profile_reaches_direct_pdf() {
    let doc = document(stroke());
    let two = RenderOptions {
        simplebrush_two_pixels_per_page_unit: true,
        ..Default::default()
    };
    let render = |options: &RenderOptions| {
        crate::render_document_pdf(&doc, options, &Default::default()).unwrap()
    };
    assert_ne!(render(&two), render(&RenderOptions::default()));
    let conflict = RenderOptions {
        simplebrush_profile: Some(SimpleBrushRenderProfile::HostV25),
        ..two
    };
    assert_eq!(render(&conflict), render(&RenderOptions::default()));
}

#[test]
fn tool0_long_two_pixel_profile_boundaries() {
    let mut source = stroke();
    source.pen_width = 8.0;
    let rendering = source.rendering.as_mut().unwrap();
    rendering.style.pen_size = Some(8.0);
    rendering.tool_type_raw = 0;
    source.points = (0..64)
        .map(|i| Point {
            x: 25.0 + i as f64 * 0.75,
            y: 216.0 - [0, 2, 5, 7, 8, 7, 5, 2][i % 8] as f64,
        })
        .collect();
    source.pressures = (0..64).map(|i| [0.25, 0.5, 0.75, 1.0][i % 4]).collect();
    assert!(crate::ink::simplebrush::prepare_stroke(&source, true, 32_768).is_ok());
    let long =
        crate::ink::simplebrush::prepare_stroke_for_scale(&source, true, 32_768, true).unwrap();
    assert!(
        crate::ink::simplebrush::prepare_stroke_for_scale(
            &source,
            true,
            long.samples.len() - 1,
            true
        )
        .is_err()
    );
    let mut paired = source.clone();
    paired.rendering.as_mut().unwrap().tool_type_raw = 2;
    assert_eq!(
        long.samples,
        crate::ink::simplebrush::prepare_stroke_for_scale(&paired, true, 32_768, true)
            .unwrap()
            .samples
    );
    let doc = document(source.clone());
    let paired_doc = document(paired);
    let two = RenderOptions {
        simplebrush_two_pixels_per_page_unit: true,
        ..Default::default()
    };
    let generic = RenderOptions::default();
    assert_eq!(
        render_document_svg(&doc, &two)[0].svg,
        render_document_svg(&paired_doc, &two)[0].svg
    );
    assert_ne!(
        render_document_svg(&doc, &two)[0].svg,
        render_document_svg(&doc, &generic)[0].svg
    );
    assert_ne!(
        render_document_svg(&doc, &options())[0].svg,
        render_document_svg(&doc, &generic)[0].svg
    );
    #[cfg(feature = "pdf")]
    assert_eq!(
        crate::render_document_pdf(&doc, &two, &Default::default()).unwrap(),
        crate::render_document_pdf(&paired_doc, &two, &Default::default()).unwrap()
    );
    let mut short_terminal = source.clone();
    let previous = short_terminal.points[62];
    short_terminal.points[63] = Point {
        x: previous.x + 0.5,
        y: previous.y + 0.5,
    };
    assert!(
        crate::ink::simplebrush::prepare_stroke_for_scale(&short_terminal, true, 32_768, true)
            .is_ok()
    );
    assert!(
        crate::ink::simplebrush::prepare_stroke_for_scale(&short_terminal, true, 32_768, false)
            .is_err()
    );
    let mut size3_terminal = short_terminal.clone();
    size3_terminal.pen_width = 3.0;
    size3_terminal.rendering.as_mut().unwrap().style.pen_size = Some(3.0);
    assert!(
        crate::ink::simplebrush::prepare_stroke_for_scale(&size3_terminal, true, 32_768, true)
            .is_err()
    );
    let short_doc = document(short_terminal);
    assert_ne!(
        render_document_svg(&short_doc, &two)[0].svg,
        render_document_svg(&short_doc, &generic)[0].svg
    );
    for i in 64..257 {
        source.points.push(Point {
            x: 25.0 + i as f64 * 0.75,
            y: 216.0 - [0, 2, 5, 7, 8, 7, 5, 2][i % 8] as f64,
        });
        source.pressures.push(0.25);
    }
    let boundary = document(source);
    assert_eq!(
        render_document_svg(&boundary, &two)[0].svg,
        render_document_svg(&boundary, &generic)[0].svg
    );
    #[cfg(feature = "pdf")]
    assert_eq!(
        crate::render_document_pdf(&boundary, &two, &Default::default()).unwrap(),
        crate::render_document_pdf(&boundary, &generic, &Default::default()).unwrap()
    );
}

#[cfg(feature = "pdf")]
#[test]
fn two_pixel_native_export_fixture() {
    let mut source = stroke();
    if std::env::var_os("SDOCX_ALT_PRESSURE").is_some() {
        source.pressures = vec![0.75, 0.25, 1.0];
    }
    source.points = vec![
        Point { x: 32.0, y: 224.0 },
        Point { x: 42.0, y: 224.0 },
        Point { x: 52.0, y: 219.0 },
    ];
    if std::env::var_os("SDOCX_REVERSE").is_some() {
        source.points = vec![
            Point { x: 80.0, y: 186.0 },
            Point { x: 65.0, y: 178.0 },
            Point { x: 48.0, y: 189.0 },
        ];
    }
    if let Ok(count) = std::env::var("SDOCX_POINTS_COUNT") {
        let count: usize = count.parse().unwrap();
        let heights = [0, 2, 5, 7, 8, 7, 5, 2];
        source.points = (0..count)
            .map(|i| {
                if count > 128 {
                    Point {
                        x: 25.0 + (i % 64) as f64,
                        y: 221.0 - (i / 64) as f64 * 18.0 - heights[i % 8] as f64,
                    }
                } else if count >= 16 {
                    Point {
                        x: 25.0 + i as f64 * if count >= 33 { 0.75 } else { 2.0 },
                        y: 216.0 - heights[i % 8] as f64,
                    }
                } else {
                    Point {
                        x: 32.0 + i as f64 * 6.0,
                        y: 224.0 - (i % 3) as f64 * 3.0,
                    }
                }
            })
            .collect();
        source.pressures = (0..count)
            .map(|i| {
                if count >= 16 {
                    [0.25, 0.5, 0.75, 1.0][i % 4]
                } else {
                    0.5
                }
            })
            .collect();
    }
    match std::env::var("VARIANT").as_deref() {
        Ok("reverse") => {
            source.points.reverse();
            source.pressures.reverse();
        }
        Ok("alternate") => {
            let heights = [0, -3, -5, -4, 0, 5, 7, 3];
            let count: usize = std::env::var("SDOCX_POINTS_COUNT")
                .unwrap_or_else(|_| "32".into())
                .parse()
                .unwrap();
            source.points = (0..count)
                .map(|i| Point {
                    x: if count > 128 {
                        27.0 + (i % 64) as f64
                    } else {
                        27.0 + 2.0 * i as f64
                    },
                    y: if count > 128 {
                        221.0 - (i / 64) as f64 * 18.0
                    } else {
                        208.0
                    } - heights[i % 8] as f64,
                })
                .collect();
            source.pressures = (0..count)
                .map(|i| f64::from([1.0_f32, 0.3, 0.6, 0.8, 0.4][i % 5]))
                .collect();
        }
        _ => {}
    }
    if std::env::var_os("SDOCX_SIMPLEBRUSH_NOTE29").is_some() {
        let saved: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/assets/fountain-v14/saved-source-29.json"
        ))
        .unwrap();
        let count = if std::env::var("SDOCX_TOOL").as_deref() == Ok("1") {
            32
        } else {
            57
        };
        source.points = saved["points"]
            .as_array()
            .unwrap()
            .iter()
            .take(count)
            .map(|p| Point {
                x: f64::from((p["x"].as_f64().unwrap() - 96.0) as f32),
                y: f64::from((256.0 - (p["y"].as_f64().unwrap() - 268.0)) as f32),
            })
            .collect();
        source.pressures = saved["pressures"]
            .as_array()
            .unwrap()
            .iter()
            .take(count)
            .map(|p| f64::from(p.as_f64().unwrap() as f32))
            .collect();
        if std::env::var_os("SDOCX_REVERSE_NOTE").is_some() {
            source.points.reverse();
            source.pressures.reverse();
        }
        if std::env::var_os("SDOCX_CROSS_NOTE").is_some() {
            let corners = [[32.0, 32.0], [64.0, 64.0], [32.0, 64.0], [64.0, 32.0]];
            source.points = (0..57)
                .map(|i| {
                    let a = corners[(i / 14) % 4];
                    let b = corners[(i / 14 + 1) % 4];
                    let t = (i % 14) as f64 / 14.0;
                    Point {
                        x: f64::from((a[0] + (b[0] - a[0]) * t) as f32),
                        y: f64::from((256.0 - (a[1] + (b[1] - a[1]) * t)) as f32),
                    }
                })
                .collect();
            if std::env::var_os("SDOCX_CROSS_OPEN").is_none() {
                source.points[56] = source.points[55];
            }
        }
    }
    if std::env::var_os("SDOCX_SIMPLEBRUSH_SHORT").is_some() {
        source.points = [32.0, 32.25, 32.5]
            .into_iter()
            .map(|x| Point { x, y: 224.0 })
            .collect();
        source.pressures = [0.1_f32, 0.9, 0.2].into_iter().map(f64::from).collect();
    }
    let default_size = if std::env::var_os("SDOCX_REVERSE").is_some() {
        "20"
    } else {
        "12"
    };
    let size: f32 = std::env::var("PROBE_SIZE")
        .unwrap_or(default_size.into())
        .parse()
        .unwrap();
    source.pen_width = size;
    source.rendering.as_mut().unwrap().style.pen_size = Some(size);
    source.color = Some(Color {
        r: if std::env::var_os("SDOCX_ALT_COLOR").is_some() {
            0xe0
        } else {
            0x44
        },
        g: if std::env::var_os("SDOCX_ALT_COLOR").is_some() {
            0x40
        } else {
            0xaa
        },
        b: if std::env::var_os("SDOCX_ALT_COLOR").is_some() {
            0x20
        } else {
            0xee
        },
    });
    source.rendering.as_mut().unwrap().tool_type_raw = std::env::var("SDOCX_TOOL")
        .unwrap_or("2".into())
        .parse()
        .unwrap();
    if let Ok(native_file) = std::env::var("SDOCX_V62_NATIVE") {
        let native: serde_json::Value =
            serde_json::from_slice(&std::fs::read(native_file).unwrap()).unwrap();
        let points: Vec<[f32; 2]> = source
            .points
            .iter()
            .map(|p| [p.x as f32, (256.0 - p.y) as f32])
            .collect();
        let pressures: Vec<f32> = source.pressures.iter().map(|p| *p as f32).collect();
        let samples = crate::ink::simplebrush::generate(crate::ink::simplebrush::NativeInput {
            points: &points,
            pressures: &pressures,
            size,
            tool_type: source.rendering.as_ref().unwrap().tool_type_raw.into(),
            curve_enabled: true,
            max_samples: 32_768,
        })
        .unwrap();
        let hex: String = samples
            .iter()
            .flatten()
            .flat_map(|v| v.to_le_bytes())
            .map(|v| format!("{v:02x}"))
            .collect();
        assert_eq!(
            hex,
            native["drawing_vector"]["vector_bytes"].as_str().unwrap()
        );
    }
    if std::env::var_os("SDOCX_SIMPLEBRUSH_NOTE29").is_some()
        && let Err(error) =
            crate::ink::simplebrush::prepare_stroke_for_scale(&source, true, 32_768, true)
    {
        panic!("note-derived profile rejected: {error}");
    }
    let mut second = source.clone();
    let mut doc = document(source);
    if std::env::var_os("SDOCX_SIMPLEBRUSH_OVERLAP").is_some() {
        for point in &mut second.points {
            point.x += 1.25;
            point.y += 1.5;
        }
        doc.pages[0].objects.push(second.into());
    }
    doc.pages[0].width = 256;
    doc.pages[0].height = 256;
    if let Some(directory) = std::env::var_os("SDOCX_SIMPLEBRUSH_TWO_EXPORT") {
        let directory = std::path::Path::new(&directory);
        std::fs::create_dir_all(directory).unwrap();
        let mut blank = doc.clone();
        blank.pages[0].objects.clear();
        std::fs::write(
            directory.join("paper.pdf"),
            crate::render_document_pdf(&blank, &Default::default(), &Default::default()).unwrap(),
        )
        .unwrap();
        let two = RenderOptions {
            simplebrush_two_pixels_per_page_unit: true,
            ..Default::default()
        };
        for (name, options) in [
            ("generic", RenderOptions::default()),
            ("one", options()),
            ("two", two),
        ] {
            let svg = render_document_svg(&doc, &options).remove(0).svg;
            let pdf = crate::render_document_pdf(&doc, &options, &Default::default()).unwrap();
            std::fs::write(directory.join(format!("{name}.svg")), svg).unwrap();
            std::fs::write(directory.join(format!("{name}.pdf")), pdf).unwrap();
        }
    }
}

#[test]
fn tool3_two_pixel_interior_sizes_match_native_geometry() {
    let receipt: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/simplebrush-tool3-2x-interior.json"
    ))
    .unwrap();
    for case in receipt["cases"].as_array().unwrap() {
        let points: Vec<[f32; 2]> = case["points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| [p[0].as_f64().unwrap() as f32, p[1].as_f64().unwrap() as f32])
            .collect();
        let pressures: Vec<f32> = case["pressure"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p.as_f64().unwrap() as f32)
            .collect();
        let samples = crate::ink::simplebrush::generate(crate::ink::simplebrush::NativeInput {
            points: &points,
            pressures: &pressures,
            size: case["size"].as_f64().unwrap() as f32,
            tool_type: 3,
            curve_enabled: true,
            max_samples: 32_768,
        })
        .unwrap();
        let hex: String = samples
            .iter()
            .flatten()
            .flat_map(|v| v.to_le_bytes())
            .map(|v| format!("{v:02x}"))
            .collect();
        assert_eq!(
            hex,
            case["geometry_hex"].as_str().unwrap(),
            "{}",
            case["case"]
        );
    }
}

#[test]
fn two_pixel_point_counts_match_native_geometry_and_dispatch() {
    let receipt: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/simplebrush-scale-2x-points.json"
    ))
    .unwrap();
    let two = RenderOptions {
        simplebrush_two_pixels_per_page_unit: true,
        ..Default::default()
    };
    for case in receipt["cases"].as_array().unwrap() {
        let count = case["count"].as_u64().unwrap() as usize;
        let heights = [0, 2, 5, 7, 8, 7, 5, 2];
        let points: Vec<[f32; 2]> = (0..count)
            .map(|i| {
                if count >= 16 {
                    [25.0 + i as f32 * 2.0, 40.0 + heights[i % 8] as f32]
                } else {
                    [32.0 + i as f32 * 6.0, 32.0 + (i % 3) as f32 * 3.0]
                }
            })
            .collect();
        let pressure: Vec<f32> = (0..count)
            .map(|i| {
                if count >= 16 {
                    [0.25, 0.5, 0.75, 1.0][i % 4]
                } else {
                    0.5
                }
            })
            .collect();
        let samples = crate::ink::simplebrush::generate(crate::ink::simplebrush::NativeInput {
            points: &points,
            pressures: &pressure,
            size: 12.0,
            tool_type: 2,
            curve_enabled: true,
            max_samples: 32_768,
        })
        .unwrap();
        let hex: String = samples
            .iter()
            .flatten()
            .flat_map(|v| v.to_le_bytes())
            .map(|v| format!("{v:02x}"))
            .collect();
        assert_eq!(hex, case["geometry_hex"].as_str().unwrap());
        let mut source = stroke();
        source.points = points
            .iter()
            .map(|p| Point {
                x: f64::from(p[0]),
                y: 256.0 - f64::from(p[1]),
            })
            .collect();
        source.pressures = pressure.iter().map(|p| f64::from(*p)).collect();
        source.pen_width = 12.0;
        source.rendering.as_mut().unwrap().style.pen_size = Some(12.0);
        let doc = document(source);
        let actual = &render_document_svg(&doc, &two)[0].svg;
        let fallback = &render_document_svg(&doc, &RenderOptions::default())[0].svg;
        if count <= 128 {
            assert_ne!(actual, fallback);
        } else {
            assert_eq!(actual, fallback);
        }
    }
}

#[test]
fn two_pixel_point_and_size_boundaries() {
    let two = RenderOptions {
        simplebrush_two_pixels_per_page_unit: true,
        ..Default::default()
    };
    for count in [
        2, 3, 4, 15, 16, 17, 32, 33, 64, 65, 66, 128, 129, 224, 256, 257,
    ] {
        for tool in [0, 2, 3] {
            for size in [2.99, 3.0, 3.01, 7.0, 8.0, 20.0, 21.0, 32.0] {
                let mut source = stroke();
                source.points = (0..count)
                    .map(|i| Point {
                        x: 10.0 + 4.0 * i as f64,
                        y: 20.0 + 2.0 * (i % 2) as f64,
                    })
                    .collect();
                source.pressures = vec![0.5; count];
                source.pen_width = size;
                let rendering = source.rendering.as_mut().unwrap();
                rendering.tool_type_raw = tool;
                rendering.style.pen_size = Some(size);
                let doc = document(source);
                let admitted = count <= 256
                    && (count == 3 && (3.0..=32.0).contains(&size)
                        || (8.0..=20.0).contains(&size)
                        || (tool == 0 && size == 3.0 && (4..=128).contains(&count)));
                let actual = &render_document_svg(&doc, &two)[0].svg;
                let fallback = &render_document_svg(&doc, &RenderOptions::default())[0].svg;
                assert_eq!(
                    actual != fallback,
                    admitted,
                    "count={count} tool={tool} size={size}"
                );
                if tool == 0 && size == 3.0 && count == 128 {
                    assert_eq!(render_document_svg(&doc, &options())[0].svg, *fallback);
                }
                #[cfg(feature = "pdf")]
                if tool == 0 && size == 3.0 && (count == 128 || count == 129) {
                    let generic = crate::render_document_pdf(
                        &doc,
                        &RenderOptions::default(),
                        &Default::default(),
                    )
                    .unwrap();
                    assert_eq!(
                        crate::render_document_pdf(&doc, &two, &Default::default()).unwrap()
                            != generic,
                        count == 128
                    );
                    assert_eq!(
                        crate::render_document_pdf(&doc, &options(), &Default::default()).unwrap(),
                        generic
                    );
                }
            }
        }
    }
}

#[test]
fn tool_zero_size_three_128_point_sample_budget_is_enforced() {
    let mut source = stroke();
    source.points = (0..128)
        .map(|i| Point {
            x: 25.0 + i as f64 * 0.75,
            y: 216.0 - [0, 2, 5, 7, 8, 7, 5, 2][i % 8] as f64,
        })
        .collect();
    source.pressures = vec![0.5; 128];
    source.pen_width = 3.0;
    let rendering = source.rendering.as_mut().unwrap();
    rendering.tool_type_raw = 0;
    rendering.style.pen_size = Some(3.0);
    let prepare =
        |budget| crate::ink::simplebrush::prepare_stroke_for_scale(&source, true, budget, true);
    let count = prepare(32_768).unwrap().samples.len();
    assert!(count > 0 && count <= 32_768);
    assert!(prepare(count - 1).is_err());
    assert_eq!(prepare(count).unwrap().samples.len(), count);
}

#[test]
fn size_three_128_point_controls_match_native_geometry() {
    for (alternate, receipt) in [
        (
            false,
            include_str!("../../../../tools/simplebrush-size3-128/baseline-native.json"),
        ),
        (
            true,
            include_str!("../../../../tools/simplebrush-size3-128/alternate-native.json"),
        ),
    ] {
        let native: serde_json::Value = serde_json::from_str(receipt).unwrap();
        let points: Vec<[f32; 2]> = (0..128)
            .map(|i| {
                if alternate {
                    [
                        27.0 + i as f32,
                        48.0 + [0, -3, -5, -4, 0, 5, 7, 3][i % 8] as f32,
                    ]
                } else {
                    [
                        25.0 + i as f32 * 0.75,
                        40.0 + [0, 2, 5, 7, 8, 7, 5, 2][i % 8] as f32,
                    ]
                }
            })
            .collect();
        let pressures: Vec<f32> = (0..128)
            .map(|i| {
                if alternate {
                    [1.0, 0.3, 0.6, 0.8, 0.4][i % 5]
                } else {
                    [0.25, 0.5, 0.75, 1.0][i % 4]
                }
            })
            .collect();
        let samples = crate::ink::simplebrush::generate(crate::ink::simplebrush::NativeInput {
            points: &points,
            pressures: &pressures,
            size: 3.0,
            tool_type: 0,
            curve_enabled: true,
            max_samples: 32_768,
        })
        .unwrap();
        let hex: String = samples
            .iter()
            .flatten()
            .flat_map(|v| v.to_le_bytes())
            .map(|v| format!("{v:02x}"))
            .collect();
        assert_eq!(
            hex,
            native["drawing_vector"]["vector_bytes"].as_str().unwrap()
        );
    }
}

#[test]
fn two_pixel_long_controls_match_native_geometry() {
    let receipt: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/simplebrush-scale-2x-points-32-variants.json"
    ))
    .unwrap();
    for name in ["reverse", "alternate"] {
        let mut points: Vec<[f32; 2]> = (0..32)
            .map(|i| {
                if name == "reverse" {
                    [
                        25.0 + 2.0 * i as f32,
                        40.0 + [0, 2, 5, 7, 8, 7, 5, 2][i % 8] as f32,
                    ]
                } else {
                    [
                        27.0 + 2.0 * i as f32,
                        48.0 + [0, -3, -5, -4, 0, 5, 7, 3][i % 8] as f32,
                    ]
                }
            })
            .collect();
        let mut pressures: Vec<f32> = (0..32)
            .map(|i| {
                if name == "reverse" {
                    [0.25, 0.5, 0.75, 1.0][i % 4]
                } else {
                    [1.0, 0.3, 0.6, 0.8, 0.4][i % 5]
                }
            })
            .collect();
        if name == "reverse" {
            points.reverse();
            pressures.reverse();
        }
        let samples = crate::ink::simplebrush::generate(crate::ink::simplebrush::NativeInput {
            points: &points,
            pressures: &pressures,
            size: 12.0,
            tool_type: 2,
            curve_enabled: true,
            max_samples: 32_768,
        })
        .unwrap();
        let hex: String = samples
            .iter()
            .flatten()
            .flat_map(|v| v.to_le_bytes())
            .map(|v| format!("{v:02x}"))
            .collect();
        assert_eq!(
            hex,
            receipt["cases"][name]["geometry_hex"].as_str().unwrap()
        );
    }
}

#[test]
fn two_pixel_setting_aliases_match_null_svg_and_pdf() {
    let two = RenderOptions {
        simplebrush_two_pixels_per_page_unit: true,
        ..Default::default()
    };
    for (size, points) in [
        (12.0, [(32.0, 32.0), (42.0, 32.0), (52.0, 37.0)]),
        (20.0, [(80.0, 70.0), (65.0, 78.0), (48.0, 67.0)]),
    ] {
        for tool in [0, 2, 3] {
            let mut source = stroke();
            source.pen_width = size;
            source.points = points.into_iter().map(|(x, y)| Point { x, y }).collect();
            let rendering = source.rendering.as_mut().unwrap();
            rendering.style.pen_size = Some(size);
            rendering.tool_type_raw = tool;
            rendering.advanced_settings = None;
            let reference = document(source.clone());
            let svg = render_document_svg(&reference, &two)[0].svg.clone();
            assert_ne!(
                svg,
                render_document_svg(&reference, &RenderOptions::default())[0].svg
            );
            #[cfg(feature = "pdf")]
            let pdf = crate::render_document_pdf(&reference, &two, &Default::default()).unwrap();
            for setting in ["", "1;"] {
                source.rendering.as_mut().unwrap().advanced_settings = Some(setting.into());
                let doc = document(source.clone());
                assert_eq!(render_document_svg(&doc, &two)[0].svg, svg);
                #[cfg(feature = "pdf")]
                assert_eq!(
                    crate::render_document_pdf(&doc, &two, &Default::default()).unwrap(),
                    pdf
                );
            }
        }
    }
}

#[test]
fn long_two_pixel_controls_match_native_geometry_and_boundary() {
    let receipt: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/simplebrush-long-controls.json"
    ))
    .unwrap();
    let expanded: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/simplebrush-expanded-long-native.json"
    ))
    .unwrap();
    let two = RenderOptions {
        simplebrush_two_pixels_per_page_unit: true,
        ..Default::default()
    };
    for case in receipt["cases"]
        .as_array()
        .unwrap()
        .iter()
        .chain(expanded["cases"].as_array().unwrap())
    {
        let count = case["count"].as_u64().unwrap() as usize;
        let size = case["size"].as_f64().unwrap() as f32;
        let heights = [0, 2, 5, 7, 8, 7, 5, 2];
        let mut points: Vec<[f32; 2]> = (0..count)
            .map(|i| [25.0 + i as f32 * 0.75, 40.0 + heights[i % 8] as f32])
            .collect();
        let mut pressures: Vec<f32> = (0..count).map(|i| [0.25, 0.5, 0.75, 1.0][i % 4]).collect();
        if case["reverse"].as_bool().unwrap() {
            points.reverse();
            pressures.reverse();
        }
        for tool in [0, 2, 3] {
            let samples = crate::ink::simplebrush::generate(crate::ink::simplebrush::NativeInput {
                points: &points,
                pressures: &pressures,
                size,
                tool_type: tool,
                curve_enabled: true,
                max_samples: 32_768,
            })
            .unwrap();
            let hex: String = samples
                .iter()
                .flatten()
                .flat_map(|v| v.to_le_bytes())
                .map(|v| format!("{v:02x}"))
                .collect();
            assert_eq!(hex, case["geometry_hex"].as_str().unwrap());
            let mut source = stroke();
            source.points = points
                .iter()
                .map(|p| Point {
                    x: f64::from(p[0]),
                    y: 256.0 - f64::from(p[1]),
                })
                .collect();
            source.pressures = pressures.iter().map(|p| f64::from(*p)).collect();
            source.pen_width = size;
            source.rendering.as_mut().unwrap().style.pen_size = Some(size);
            source.rendering.as_mut().unwrap().tool_type_raw = tool as u16;
            let doc = document(source);
            assert_ne!(
                render_document_svg(&doc, &two)[0].svg,
                render_document_svg(&doc, &RenderOptions::default())[0].svg
            );
        }
    }
    for tool in [0, 2, 3] {
        let mut source = stroke();
        source.points = (0..257)
            .map(|i| Point {
                x: 25.0 + i as f64 * 0.75,
                y: 216.0,
            })
            .collect();
        source.pressures = vec![0.5; 257];
        source.pen_width = 20.0;
        source.rendering.as_mut().unwrap().style.pen_size = Some(20.0);
        source.rendering.as_mut().unwrap().tool_type_raw = tool;
        let doc = document(source);
        assert_eq!(
            render_document_svg(&doc, &two)[0].svg,
            render_document_svg(&doc, &RenderOptions::default())[0].svg
        );
        #[cfg(feature = "pdf")]
        assert_eq!(
            crate::render_document_pdf(&doc, &two, &Default::default()).unwrap(),
            crate::render_document_pdf(&doc, &RenderOptions::default(), &Default::default())
                .unwrap()
        );
    }
}

#[test]
fn long_two_pixel_svg_improves_against_native_frames() {
    use sha2::{Digest, Sha256};
    use std::io::Read;

    let controls: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/simplebrush-long-controls.json"
    ))
    .unwrap();
    let tool3: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/simplebrush-tool3-long-identity.json"
    ))
    .unwrap();
    let tool0: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/simplebrush-tool0-long-identity.json"
    ))
    .unwrap();
    let expanded: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/simplebrush-expanded-long-native.json"
    ))
    .unwrap();
    assert_eq!(controls["cases"].as_array().unwrap().len(), 3);
    assert_eq!(tool3["cases"].as_array().unwrap().len(), 3);
    assert_eq!(tool0["cases"].as_array().unwrap().len(), 3);

    for (count, size, reverse, max_error, compressed) in [
        (
            33,
            8.0_f32,
            false,
            5140,
            include_bytes!("../../../../conformance/simplebrush-long-33.rgba.zlib").as_slice(),
        ),
        (
            64,
            20.0,
            false,
            8282,
            include_bytes!("../../../../conformance/simplebrush-long-64b.rgba.zlib").as_slice(),
        ),
        (
            64,
            20.0,
            true,
            8500,
            include_bytes!("../../../../conformance/simplebrush-long-64r.rgba.zlib").as_slice(),
        ),
        (
            65,
            8.0,
            false,
            7700,
            include_bytes!("../../../../conformance/simplebrush-expanded-65.rgba.zlib").as_slice(),
        ),
        (
            128,
            20.0,
            true,
            12210,
            include_bytes!("../../../../conformance/simplebrush-expanded-128.rgba.zlib").as_slice(),
        ),
    ] {
        let mut native = Vec::new();
        flate2::read::ZlibDecoder::new(compressed)
            .read_to_end(&mut native)
            .unwrap();
        assert_eq!(native.len(), 256 * 256 * 4);
        let find_case = |receipt: &serde_json::Value| {
            let matches: Vec<_> = receipt["cases"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|case| {
                    case["count"].as_u64() == Some(count as u64)
                        && case["size"].as_f64() == Some(f64::from(size))
                        && case["reverse"].as_bool() == Some(reverse)
                })
                .collect();
            assert_eq!(matches.len(), 1);
            matches[0].clone()
        };
        let reference = if count > 64 {
            find_case(&expanded)
        } else {
            find_case(&controls)
        };
        let identity = if count > 64 {
            reference["tools"]["3"].clone()
        } else {
            find_case(&tool3)
        };
        let zero_identity = if count > 64 {
            reference["tools"]["0"].clone()
        } else {
            find_case(&tool0)
        };
        if count > 64 {
            assert_eq!(
                reference["tools"]["2"]["frame_sha256"],
                reference["frame_sha256"]
            );
            assert_eq!(
                reference["tools"]["2"]["geometry_sha256"],
                reference["geometry_sha256"]
            );
            for tool in ["0", "2", "3"] {
                assert_eq!(reference["tools"][tool]["paint_clean"], true);
            }
        }
        assert_eq!(identity["geometry_sha256"], reference["geometry_sha256"]);
        assert_eq!(identity["frame_sha256"], reference["frame_sha256"]);
        assert_eq!(
            zero_identity["geometry_sha256"],
            reference["geometry_sha256"]
        );
        assert_eq!(zero_identity["frame_sha256"], reference["frame_sha256"]);
        assert_eq!(
            format!("{:x}", Sha256::digest(&native)),
            identity["frame_sha256"].as_str().unwrap()
        );
        let heights = [0, 2, 5, 7, 8, 7, 5, 2];
        let mut input = stroke();
        input.points = (0..count)
            .map(|i| Point {
                x: 25.0 + i as f64 * 0.75,
                y: 216.0 - heights[i % 8] as f64,
            })
            .collect();
        input.pressures = (0..count).map(|i| [0.25, 0.5, 0.75, 1.0][i % 4]).collect();
        if reverse {
            input.points.reverse();
            input.pressures.reverse();
        }
        input.pen_width = size;
        input.rendering.as_mut().unwrap().style.pen_size = Some(size);
        input.color = Some(Color {
            r: 0x44,
            g: 0xaa,
            b: 0xee,
        });
        for tool in [0, 2, 3] {
            input.rendering.as_mut().unwrap().tool_type_raw = tool;
            let mut doc = document(input.clone());
            doc.pages[0].width = 256;
            doc.pages[0].height = 256;
            let error = |options: &RenderOptions| {
                let svg = render_document_svg(&doc, options).remove(0).svg;
                let body = &svg[svg.find('>').unwrap() + 1..svg.rfind("</svg>").unwrap()];
                let mut body = body.to_string();
                let start = body
                    .find("<rect fill=\"#fcfcfc\"")
                    .expect("expected page background");
                let end = start
                    + body[start..]
                        .find("/>")
                        .expect("expected self-closing page background")
                    + 2;
                body.replace_range(start..end, "");
                let svg = format!(
                    "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"256\" height=\"256\"><g transform=\"matrix(2 0 0 -2 .25 512.5)\">{body}</g></svg>"
                );
                let tree = resvg::usvg::Tree::from_str(&svg, &Default::default()).unwrap();
                let mut pixmap = resvg::tiny_skia::Pixmap::new(256, 256).unwrap();
                resvg::render(
                    &tree,
                    resvg::tiny_skia::Transform::identity(),
                    &mut pixmap.as_mut(),
                );
                native
                    .iter()
                    .skip(3)
                    .step_by(4)
                    .zip(pixmap.data().iter().skip(3).step_by(4))
                    .map(|(expected, actual)| u64::from(expected.abs_diff(*actual)))
                    .sum::<u64>()
            };
            let two = error(&RenderOptions {
                simplebrush_two_pixels_per_page_unit: true,
                ..Default::default()
            });
            let one = error(&options());
            let generic = error(&RenderOptions::default());
            eprintln!("{count} reverse={reverse}: two={two}, one={one}, generic={generic}");
            assert!(two <= max_error && two < one && one < generic);
        }
    }
}

#[test]
fn tool_zero_three_point_two_pixel_bounds_and_fallback() {
    let two = RenderOptions {
        simplebrush_two_pixels_per_page_unit: true,
        ..Default::default()
    };
    for (size, admitted) in [
        (2.99, false),
        (3.0, true),
        (5.5, true),
        (32.0, true),
        (32.01, false),
    ] {
        let mut source = stroke();
        source.pen_width = size;
        let rendering = source.rendering.as_mut().unwrap();
        rendering.tool_type_raw = 0;
        rendering.style.pen_size = Some(size);
        let doc = document(source);
        let generic = RenderOptions::default();
        assert_eq!(
            render_document_svg(&doc, &two)[0].svg != render_document_svg(&doc, &generic)[0].svg,
            admitted
        );
        #[cfg(feature = "pdf")]
        assert_eq!(
            crate::render_document_pdf(&doc, &two, &Default::default()).unwrap()
                != crate::render_document_pdf(&doc, &generic, &Default::default()).unwrap(),
            admitted
        );
        if size == 3.0 {
            assert_eq!(
                render_document_svg(&doc, &options())[0].svg,
                render_document_svg(&doc, &generic)[0].svg
            );
        }
    }
    for count in [2, 4] {
        let mut source = stroke();
        source.points.truncate(count);
        source.pressures.truncate(count);
        if count == 4 {
            source.points.push(Point { x: 40.0, y: 20.0 });
            source.pressures.push(0.5);
        }
        source.pen_width = 3.0;
        let rendering = source.rendering.as_mut().unwrap();
        rendering.tool_type_raw = 0;
        rendering.style.pen_size = Some(3.0);
        let doc = document(source);
        assert_eq!(
            render_document_svg(&doc, &two)[0].svg
                != render_document_svg(&doc, &RenderOptions::default())[0].svg,
            count == 4
        );
        #[cfg(feature = "pdf")]
        assert_eq!(
            crate::render_document_pdf(&doc, &two, &Default::default()).unwrap()
                != crate::render_document_pdf(&doc, &RenderOptions::default(), &Default::default())
                    .unwrap(),
            count == 4
        );
    }
}

#[cfg(feature = "pdf")]
#[test]
fn exploratory_long_budget_preserves_short_export() {
    let mut long = stroke();
    long.points = (0..256)
        .map(|i| Point {
            x: 5.0 + (i % 64) as f64,
            y: 70.0 - (i / 64) as f64 * 18.0 - [0, 2, 5, 7, 8, 7, 5, 2][i % 8] as f64,
        })
        .collect();
    long.pressures = vec![0.5; 256];
    let count = crate::ink::simplebrush::prepare_stroke_for_scale(&long, true, 32768, true)
        .unwrap()
        .samples
        .len();
    assert!(count > 0 && count <= 4096, "{count}");
    let short = stroke();
    let options = RenderOptions {
        simplebrush_two_pixels_per_page_unit: true,
        ..Default::default()
    };
    let short_svg = render_document_svg(&document(short.clone()), &options)[0]
        .svg
        .clone();
    let mut doc = document(long.clone());
    let copies = 4096 / count + 2;
    doc.pages[0].objects = (0..copies).map(|_| long.clone().into()).collect();
    doc.pages[0].objects.push(short.clone().into());
    let svg = render_document_svg(&doc, &options)[0].svg.clone();
    let circles = |xml: &str| -> Vec<(String, String, String)> {
        roxmltree::Document::parse(xml)
            .unwrap()
            .descendants()
            .filter(|n| n.has_tag_name("circle"))
            .map(|n| {
                (
                    n.attribute("cx").unwrap().into(),
                    n.attribute("cy").unwrap().into(),
                    n.attribute("r").unwrap().into(),
                )
            })
            .collect()
    };
    let expected = circles(&short_svg);
    let observed = circles(&svg);
    assert_eq!(&observed[observed.len() - expected.len()..], expected);
    assert_eq!(observed.len(), (4096 / count) * count + expected.len());
    let pdf = crate::render_document_pdf(&doc, &options, &Default::default()).unwrap();
    let generic_pdf =
        crate::render_document_pdf(&doc, &RenderOptions::default(), &Default::default()).unwrap();
    assert_ne!(pdf, generic_pdf);
    let mut twice = doc.clone();
    twice.pages.push(doc.pages[0].clone());
    let both = render_document_svg(&twice, &options);
    assert_eq!(both[0].svg, both[1].svg);
    let twice_pdf = crate::render_document_pdf(&twice, &options, &Default::default()).unwrap();
    eprintln!(
        "samples={count} copies={copies} circles={} pdf={} two_page_pdf={}",
        observed.len(),
        pdf.len(),
        twice_pdf.len()
    );
}

#[test]
fn exploratory_long_and_short_budgets_are_independent_and_fallback_is_ordered() {
    let mut long = stroke();
    long.points = (0..256)
        .map(|i| Point {
            x: 5.0 + (i % 64) as f64,
            y: 70.0 - (i / 64) as f64 * 18.0 - [0, 2, 5, 7, 8, 7, 5, 2][i % 8] as f64,
        })
        .collect();
    long.pressures = vec![0.5; 256];
    let count = crate::ink::simplebrush::prepare_stroke_for_scale(&long, true, 32768, true)
        .unwrap()
        .samples
        .len();
    let mut rejected = long.clone();
    rejected.color = Some(Color {
        r: 0xde,
        g: 0xad,
        b: 0xbe,
    });
    rejected.rendering.as_mut().unwrap().style.color_argb = Some(0xffde_adbe);
    let mut short = stroke();
    short.color = Some(Color { r: 0, g: 255, b: 0 });
    short.rendering.as_mut().unwrap().style.color_argb = Some(0xff00_ff00);
    let short_count = crate::ink::simplebrush::prepare_stroke_for_scale(&short, true, 32768, true)
        .unwrap()
        .samples
        .len();
    let options = RenderOptions {
        simplebrush_two_pixels_per_page_unit: true,
        ..Default::default()
    };
    let mut doc = document(long.clone());
    doc.pages[0].objects.clear();
    for _ in 0..4096 / count {
        doc.pages[0].objects.push(short.clone().into());
        doc.pages[0].objects.push(long.clone().into());
    }
    doc.pages[0].objects.push(rejected.clone().into());
    doc.pages[0].objects.push(short.clone().into());
    let svg = render_document_svg(&doc, &options)[0].svg.clone();
    let expected = render_document_svg(&document(rejected), &RenderOptions::default())[0]
        .svg
        .clone();
    let lines = |xml: &str| -> Vec<String> {
        let tree = roxmltree::Document::parse(xml).unwrap();
        tree.descendants()
            .filter(|n| n.has_tag_name("line") && n.attribute("stroke") == Some("#deadbe"))
            .map(|n| xml[n.range()].to_owned())
            .collect()
    };
    let fallback = lines(&expected);
    assert!(!fallback.is_empty());
    assert_eq!(lines(&svg), fallback);
    let first = svg.find(&fallback[0]).unwrap();
    let last = svg.rfind(fallback.last().unwrap()).unwrap();
    assert!(svg[..first].contains("radialGradient"));
    assert!(svg[last..].contains("#00ff00"));
    assert_eq!(
        svg.matches("<circle").count(),
        (4096 / count) * (count + short_count) + short_count
    );
}

#[test]
fn long_two_pixel_route_preserves_guards_and_one_pixel_limits() {
    let two = RenderOptions {
        simplebrush_two_pixels_per_page_unit: true,
        ..Default::default()
    };
    for count in [129, 256] {
        for tool in [0, 2, 3] {
            let mut source = stroke();
            source.points = (0..count)
                .map(|i| Point {
                    x: 10.0 + 2.0 * i as f64,
                    y: 20.0 + 2.0 * (i % 5) as f64,
                })
                .collect();
            source.pressures = vec![0.5; count];
            source.pen_width = 12.0;
            source.rendering.as_mut().unwrap().style.pen_size = Some(12.0);
            source.rendering.as_mut().unwrap().tool_type_raw = tool;
            assert!(
                crate::ink::simplebrush::prepare_stroke_for_scale(&source, true, 32_768, true)
                    .is_ok()
            );
            assert_eq!(
                crate::ink::simplebrush::prepare_stroke_for_scale(&source, true, 32_768, false)
                    .is_ok(),
                tool != 0
            );
            let doc = document(source.clone());
            let layout = layout_document(&doc);
            assert_eq!(
                render_layout_page_replay_svg(&doc, &layout, 0, &two)
                    .unwrap()
                    .svg,
                render_layout_page_replay_svg(&doc, &layout, 0, &RenderOptions::default())
                    .unwrap()
                    .svg
            );
            #[cfg(feature = "pdf")]
            {
                let request =
                    super::PageSceneRequest::sliced(&layout.pages[0], 0, 50, true).unwrap();
                let scene = |o: &RenderOptions| {
                    super::DocumentTextCache::default()
                        .render_layout_page_scene_requests(
                            &doc,
                            &[request],
                            o,
                            &super::fonts::FontBook::default(),
                            &mut |_| {},
                        )
                        .remove(0)
                        .page
                        .svg
                };
                assert_eq!(scene(&two), scene(&RenderOptions::default()));
            }
            source.pressures.pop();
            let invalid = document(source);
            assert_eq!(
                render_document_svg(&invalid, &two)[0].svg,
                render_document_svg(&invalid, &RenderOptions::default())[0].svg
            );
        }
    }
}

#[test]
fn long_candidate_preserves_public_one_pixel_boundaries_and_mixed_order() {
    let two = RenderOptions {
        simplebrush_two_pixels_per_page_unit: true,
        ..Default::default()
    };
    let generic = RenderOptions::default();
    for (tool, boundary) in [(0, 128), (2, 320), (3, 256)] {
        for count in [boundary, boundary + 1] {
            let mut s = stroke();
            s.points = (0..count)
                .map(|i| Point {
                    x: 10.0 + i as f64 * 2.0,
                    y: 20.0 + (i % 5) as f64,
                })
                .collect();
            s.pressures = vec![0.5; count];
            s.rendering.as_mut().unwrap().tool_type_raw = tool;
            let doc = document(s);
            let actual = &render_document_svg(&doc, &options())[0].svg;
            let fallback = &render_document_svg(&doc, &generic)[0].svg;
            assert_eq!(
                actual != fallback,
                count == boundary,
                "tool={tool} count={count}"
            );
        }
    }
    let mut admitted = stroke();
    admitted.points = (0..129)
        .map(|i| Point {
            x: 10.0 + i as f64 * 2.0,
            y: 20.0 + (i % 5) as f64,
        })
        .collect();
    admitted.pressures = vec![0.5; 129];
    let mut fallback = stroke();
    fallback.color = Some(Color {
        r: 0xde,
        g: 0xad,
        b: 0xbe,
    });
    fallback.rendering.as_mut().unwrap().advanced_settings = Some("unsupported".into());
    for reverse in [false, true] {
        let mut doc = document(if reverse {
            fallback.clone()
        } else {
            admitted.clone()
        });
        doc.pages[0].objects.push(if reverse {
            admitted.clone().into()
        } else {
            fallback.clone().into()
        });
        let svg = &render_document_svg(&doc, &two)[0].svg;
        let native_pos = svg.find("radialGradient").unwrap();
        let fallback_pos = svg.find("#deadbe").unwrap();
        assert_eq!(fallback_pos < native_pos, reverse);
        assert!(svg.contains("<line stroke=\"#deadbe\""));
        assert_ne!(svg, &render_document_svg(&doc, &generic)[0].svg);
    }
}

#[test]
fn tool1_two_pixel_profile_matches_native_geometry_and_short_movement() {
    for (count, size, reverse, native) in [
        (
            3,
            12.0,
            false,
            &include_bytes!("../ink/fixtures/simplebrush-tool1/3.vbo")[..],
        ),
        (
            16,
            8.0,
            false,
            &include_bytes!("../ink/fixtures/simplebrush-tool1/capture.vbo")[..],
        ),
        (
            32,
            12.0,
            true,
            &include_bytes!("../ink/fixtures/simplebrush-tool1/32.vbo")[..],
        ),
    ] {
        let heights = [0, 2, 5, 7, 8, 7, 5, 2];
        let mut points: Vec<[f32; 2]> = (0..count)
            .map(|i| {
                if count >= 16 {
                    [25.0 + i as f32 * 2.0, 40.0 + heights[i % 8] as f32]
                } else {
                    [32.0 + i as f32 * 6.0, 32.0 + (i % 3) as f32 * 3.0]
                }
            })
            .collect();
        let mut pressures: Vec<f32> = (0..count)
            .map(|i| {
                if count >= 16 {
                    [0.25, 0.5, 0.75, 1.0][i % 4]
                } else {
                    0.5
                }
            })
            .collect();
        if reverse {
            points.reverse();
            pressures.reverse();
        }
        let samples = crate::ink::simplebrush::generate(crate::ink::simplebrush::NativeInput {
            points: &points,
            pressures: &pressures,
            size,
            tool_type: 1,
            curve_enabled: true,
            max_samples: 32_768,
        })
        .unwrap();
        let bytes: Vec<u8> = samples
            .iter()
            .flatten()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        assert_eq!(bytes, native);
    }
    let mut short = stroke();
    short.rendering.as_mut().unwrap().tool_type_raw = 1;
    short.points = vec![
        Point { x: 32.0, y: 224.0 },
        Point { x: 32.25, y: 224.0 },
        Point { x: 32.5, y: 224.0 },
    ];
    short.pressures = vec![0.5; 3];
    assert!(crate::ink::simplebrush::prepare_stroke_for_scale(&short, true, 32_768, true).is_ok());
}

#[cfg(feature = "pdf")]
#[test]
fn tool1_two_pixel_public_dispatch_and_boundaries() {
    let two = RenderOptions {
        simplebrush_two_pixels_per_page_unit: true,
        ..Default::default()
    };
    let fallback = RenderOptions::default();
    for (count, size, admitted) in [
        (3, 8.0, true),
        (32, 12.0, true),
        (2, 8.0, false),
        (33, 8.0, false),
        (3, 7.0, false),
        (3, 13.0, false),
    ] {
        let mut source = stroke();
        source.points = (0..count)
            .map(|i| Point {
                x: 30.0 + i as f64 * 2.0,
                y: 50.0 + (i % 3) as f64 * 2.0,
            })
            .collect();
        source.pressures = vec![0.5; count];
        source.pen_width = size;
        let rendering = source.rendering.as_mut().unwrap();
        rendering.tool_type_raw = 1;
        rendering.style.pen_size = Some(size);
        let doc = document(source);
        let svg = |options: &RenderOptions| render_document_svg(&doc, options).remove(0).svg;
        let pdf = |options: &RenderOptions| {
            crate::render_document_pdf(&doc, options, &Default::default()).unwrap()
        };
        assert_eq!(
            svg(&two) != svg(&fallback),
            admitted,
            "SVG count {count}, size {size}"
        );
        assert_eq!(
            pdf(&two) != pdf(&fallback),
            admitted,
            "PDF count {count}, size {size}"
        );
        let one = RenderOptions {
            simplebrush_profile: Some(SimpleBrushRenderProfile::HostV25),
            ..Default::default()
        };
        assert_eq!(svg(&one), svg(&fallback));
    }
}

#[test]
fn visible_128_point_native_geometry_matches_captured_stream() {
    let heights = [0, 2, 5, 7, 8, 7, 5, 2];
    let mut points: Vec<[f32; 2]> = (0..128)
        .map(|i| {
            [
                25.0 + (i % 64) as f32,
                35.0 + (i / 64) as f32 * 18.0 + heights[i % 8] as f32,
            ]
        })
        .collect();
    let mut pressures: Vec<f32> = (0..128).map(|i| [0.25, 0.5, 0.75, 1.0][i % 4]).collect();
    points.reverse();
    pressures.reverse();
    let actual = crate::ink::simplebrush::generate(crate::ink::simplebrush::NativeInput {
        points: &points,
        pressures: &pressures,
        size: 3.0,
        tool_type: 0,
        curve_enabled: true,
        max_samples: 32_768,
    })
    .unwrap();
    let bytes: Vec<u8> = actual
        .iter()
        .flatten()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    assert_eq!(
        bytes,
        include_bytes!("../../../../conformance/simplebrush-visible-128/tool0-geometry.bin")
    );
}

#[test]
fn one_pixel_long_native_geometry_matches_captured_streams() {
    for (count, size, reverse, expected) in [
        (
            225,
            20.0,
            false,
            &include_bytes!("../ink/simplebrush_assets/225.bin")[..],
        ),
        (
            256,
            8.0,
            true,
            &include_bytes!("../ink/simplebrush_assets/256.bin")[..],
        ),
        (
            320,
            8.0,
            true,
            &include_bytes!("../ink/simplebrush_assets/320.bin")[..],
        ),
    ] {
        let mut points: Vec<[f32; 2]> = (0..count)
            .map(|i| {
                [
                    25.0 + (i % 64) as f32,
                    35.0 + (i / 64) as f32 * 18.0 + [0, 2, 5, 7, 8, 7, 5, 2][i % 8] as f32,
                ]
            })
            .collect();
        let mut pressures: Vec<f32> = (0..count).map(|i| [0.25, 0.5, 0.75, 1.0][i % 4]).collect();
        if reverse {
            points.reverse();
            pressures.reverse();
        }
        let actual = crate::ink::simplebrush::generate(crate::ink::simplebrush::NativeInput {
            points: &points,
            pressures: &pressures,
            size,
            tool_type: 2,
            curve_enabled: true,
            max_samples: 32_768,
        })
        .unwrap();
        let bytes: Vec<u8> = actual
            .iter()
            .flatten()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        assert_eq!(bytes, expected, "count={count}");
    }
}

#[test]
fn tool0_size3_32_point_native_control_and_dispatch_guards() {
    let receipt: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../conformance/simplebrush-size3-32-native.json"
    ))
    .unwrap();
    let heights = [0, 2, 5, 7, 8, 7, 5, 2];
    let mut source = stroke();
    source.points = (0..32)
        .map(|i| Point {
            x: 25.0 + 2.0 * i as f64,
            y: 216.0 - heights[i % 8] as f64,
        })
        .collect();
    source.pressures = (0..32).map(|i| [0.25, 0.5, 0.75, 1.0][i % 4]).collect();
    source.pen_width = 3.0;
    let rendering = source.rendering.as_mut().unwrap();
    rendering.style.pen_size = Some(3.0);
    rendering.tool_type_raw = 0;
    let points: Vec<[f32; 2]> = source
        .points
        .iter()
        .map(|p| [p.x as f32, (256.0 - p.y) as f32])
        .collect();
    let pressures: Vec<f32> = source.pressures.iter().map(|p| *p as f32).collect();
    let samples = crate::ink::simplebrush::generate(crate::ink::simplebrush::NativeInput {
        points: &points,
        pressures: &pressures,
        size: 3.0,
        tool_type: 0,
        curve_enabled: true,
        max_samples: 32_768,
    })
    .unwrap();
    let hex: String = samples
        .iter()
        .flatten()
        .flat_map(|v| v.to_le_bytes())
        .map(|v| format!("{v:02x}"))
        .collect();
    assert_eq!(hex, receipt["geometry"]["vector_bytes"].as_str().unwrap());
    let two = RenderOptions {
        simplebrush_two_pixels_per_page_unit: true,
        ..Default::default()
    };
    let render =
        |s: Stroke, opts: &RenderOptions| render_document_svg(&document(s), opts).remove(0).svg;
    assert_ne!(
        render(source.clone(), &two),
        render(source.clone(), &RenderOptions::default())
    );
    let mut below = source.clone();
    below.pen_width = 2.99;
    below.rendering.as_mut().unwrap().style.pen_size = Some(2.99);
    let mut settings = source.clone();
    settings.rendering.as_mut().unwrap().advanced_settings = Some("unsupported".into());
    let mut tool = source.clone();
    tool.rendering.as_mut().unwrap().tool_type_raw = 1;
    for rejected in [below, settings, tool] {
        assert_eq!(
            render(rejected.clone(), &two),
            render(rejected, &RenderOptions::default())
        );
    }
}

#[test]
fn tool0_size3_endpoint_native_geometry() {
    for (count, json) in [
        (
            16,
            include_str!("../../../../conformance/simplebrush-size3-16-native.json"),
        ),
        (
            64,
            include_str!("../../../../conformance/simplebrush-size3-64-native.json"),
        ),
    ] {
        let receipt: serde_json::Value = serde_json::from_str(json).unwrap();
        let heights = [0, 2, 5, 7, 8, 7, 5, 2];
        let points: Vec<[f32; 2]> = (0..count)
            .map(|i| {
                [
                    25.0 + i as f32 * if count >= 33 { 0.75 } else { 2.0 },
                    40.0 + heights[i % 8] as f32,
                ]
            })
            .collect();
        let pressures: Vec<f32> = (0..count).map(|i| [0.25, 0.5, 0.75, 1.0][i % 4]).collect();
        let samples = crate::ink::simplebrush::generate(crate::ink::simplebrush::NativeInput {
            points: &points,
            pressures: &pressures,
            size: 3.0,
            tool_type: 0,
            curve_enabled: true,
            max_samples: 32_768,
        })
        .unwrap();
        let hex: String = samples
            .iter()
            .flatten()
            .flat_map(|v| v.to_le_bytes())
            .map(|v| format!("{v:02x}"))
            .collect();
        assert_eq!(hex, receipt["geometry"]["vector_bytes"].as_str().unwrap());
    }
}

#[test]
fn tool0_size3_short_native_geometry() {
    for (count, json) in [
        (
            4,
            include_str!("../../../../tools/simplebrush-v208/native-4.json"),
        ),
        (
            7,
            include_str!("../../../../tools/simplebrush-v208/native-7.json"),
        ),
    ] {
        let native: serde_json::Value = serde_json::from_str(json).unwrap();
        let points: Vec<[f32; 2]> = (0..count)
            .map(|i| [32.0 + i as f32 * 6.0, 32.0 + (i % 3) as f32 * 3.0])
            .collect();
        let samples = crate::ink::simplebrush::generate(crate::ink::simplebrush::NativeInput {
            points: &points,
            pressures: &vec![0.5; count],
            size: 3.0,
            tool_type: 0,
            curve_enabled: true,
            max_samples: 32_768,
        })
        .unwrap();
        let hex: String = samples
            .iter()
            .flatten()
            .flat_map(|v| v.to_le_bytes())
            .map(|v| format!("{v:02x}"))
            .collect();
        assert_eq!(
            hex,
            native["drawing_vector"]["vector_bytes"].as_str().unwrap()
        );
    }
}

#[test]
fn tool0_size3_short_profile_guards() {
    let two = RenderOptions {
        simplebrush_two_pixels_per_page_unit: true,
        ..Default::default()
    };
    let one = options();
    let fallback = RenderOptions::default();
    let render = |s: Stroke, o: &RenderOptions| render_document_svg(&document(s), o).remove(0).svg;
    let make = |count: usize| {
        let mut s = stroke();
        s.points = (0..count)
            .map(|i| Point {
                x: 32.0 + 6.0 * i as f64,
                y: 224.0 - 3.0 * (i % 3) as f64,
            })
            .collect();
        s.pressures = vec![0.5; count];
        s.pen_width = 3.0;
        let r = s.rendering.as_mut().unwrap();
        r.style.pen_size = Some(3.0);
        r.tool_type_raw = 0;
        s
    };
    let short = make(4);
    assert_eq!(render(short.clone(), &one), render(short, &fallback));
    for count in [2, 129] {
        let s = make(count);
        assert_eq!(render(s.clone(), &two), render(s, &fallback));
    }
    let mut stalled = make(7);
    stalled.points[6] = stalled.points[5];
    assert_eq!(render(stalled.clone(), &two), render(stalled, &fallback));
}
