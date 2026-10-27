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
    for i in 6..192 {
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

    fn render_one(source: &Stroke, remaining: &Cell<usize>, profile: bool) -> String {
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
            profile.then_some(SimpleBrushRenderProfile::HostV25),
            None,
            remaining,
            None,
        );
        scene.finish()
    }

    let source = stroke();
    let count = crate::ink::simplebrush::prepare_stroke(&source, true, 32_768)
        .unwrap()
        .samples
        .len();
    let mut small = source.clone();
    small.points[1] = Point { x: 12.0, y: 20.0 };
    small.points[2] = Point { x: 14.0, y: 20.0 };
    let small_count = crate::ink::simplebrush::prepare_stroke(&small, true, 32_768)
        .unwrap()
        .samples
        .len();
    assert!(small_count < count);

    let budget = Cell::new(count + small_count);
    let native = render_one(&source, &budget, true);
    assert_eq!(budget.get(), small_count);
    let rejected = render_one(&source, &budget, true);
    assert_eq!(
        budget.get(),
        small_count,
        "rejection must not consume budget"
    );
    assert_eq!(
        rejected,
        render_one(&source, &Cell::new(0), false),
        "an exhausted stroke must use the entire generic fallback"
    );
    assert_ne!(native, rejected);
    let small_native = render_one(&small, &budget, true);
    assert_eq!(budget.get(), 0);
    assert_ne!(small_native, render_one(&small, &Cell::new(0), false));

    // A new page composition starts with a fresh budget.
    let fresh_page_budget = Cell::new(count + small_count);
    assert_eq!(render_one(&source, &fresh_page_budget, true), native);
    assert_eq!(fresh_page_budget.get(), small_count);
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
