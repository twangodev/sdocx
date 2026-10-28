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
    for i in 6..224 {
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
            profile.then_some(SimpleBrushRenderProfile::HostV25),
            scaled,
            None,
            false,
            None,
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
        (0, 7.0, None),
        (3, 21.0, None),
        (0, 8.0, Some("1;")),
        (3, 8.0, Some("1;")),
        (2, 2.99, None),
        (2, 32.01, None),
        (2, 8.0, Some("1;")),
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
    let mut unsupported = stroke();
    unsupported.points = (0..33)
        .map(|i| Point {
            x: i as f64 * 2.0,
            y: 20.0,
        })
        .collect();
    unsupported.pressures = vec![0.5; 33];
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
    for size in [3.0, 4.0, 32.0] {
        let mut source = stroke();
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
    let doc = document(stroke());
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

#[cfg(feature = "pdf")]
#[test]
fn two_pixel_native_export_fixture() {
    let mut source = stroke();
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
                if count >= 16 {
                    Point {
                        x: 25.0 + i as f64 * 2.0,
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
            source.points = (0..32)
                .map(|i| Point {
                    x: 27.0 + 2.0 * i as f64,
                    y: 208.0 - heights[i % 8] as f64,
                })
                .collect();
            source.pressures = (0..32)
                .map(|i| f64::from([1.0_f32, 0.3, 0.6, 0.8, 0.4][i % 5]))
                .collect();
        }
        _ => {}
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
        r: 0x44,
        g: 0xaa,
        b: 0xee,
    });
    source.rendering.as_mut().unwrap().tool_type_raw = std::env::var("SDOCX_TOOL")
        .unwrap_or("2".into())
        .parse()
        .unwrap();
    let mut doc = document(source);
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
        if count <= 32 {
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
    for count in [2, 3, 4, 16, 17, 32, 33] {
        for tool in [0, 2, 3] {
            for size in [3.0, 7.0, 8.0, 20.0, 21.0, 32.0] {
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
                let admitted = count <= 32
                    && (tool != 0 || count <= 16)
                    && ((tool == 2 && count == 3) || (8.0..=20.0).contains(&size));
                let actual = &render_document_svg(&doc, &two)[0].svg;
                let fallback = &render_document_svg(&doc, &RenderOptions::default())[0].svg;
                assert_eq!(
                    actual != fallback,
                    admitted,
                    "count={count} tool={tool} size={size}"
                );
            }
        }
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
