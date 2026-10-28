use crate::{
    BoundingBox, Color, Document, Page, Point, RenderOptions, Stroke, StrokeProperties,
    StrokeRendering, StrokeStyle,
};
fn document(case: &str, tool: u16, count: usize) -> Document {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../conformance/legacy-marker-source-geometry-controls.json"
    ))
    .unwrap();
    let hex = fixture["cases"][case]["input"]["source_arrays_hex"]["points"]
        .as_str()
        .unwrap();
    let bytes = hex
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|v| u8::from_str_radix(std::str::from_utf8(v).unwrap(), 16).unwrap())
        .collect::<Vec<_>>();
    let points = bytes
        .as_chunks::<8>()
        .0
        .iter()
        .map(|v| Point {
            x: f32::from_le_bytes(v[..4].try_into().unwrap()) as f64,
            y: f32::from_le_bytes(v[4..].try_into().unwrap()) as f64,
        })
        .collect();
    let stroke = Stroke {
        rendering: Some(StrokeRendering {
            metadata: None,
            pen_name: Some("com.samsung.android.sdk.pen.pen.preload.Marker".into()),
            advanced_settings: None,
            tool_type_raw: tool,
            properties: StrokeProperties {
                compressed: true,
                replay_only: false,
                stylus_channels: false,
                eraser: false,
                fixed_width: false,
                millisecond_timestamps: false,
                top_layer_pen: true,
                alpha_lock: false,
                binary_added: true,
                generated: false,
                fixed_opacity: false,
                rainbow_effect: false,
                straighten: false,
                reveal_mode: false,
            },
            style: StrokeStyle {
                color_argb: Some(0x80ffcb30),
                pen_size: Some(12.),
                stroke_type_raw: Some(0),
                ..Default::default()
            },
        }),
        bbox: BoundingBox::default(),
        points,
        pressures: vec![],
        timestamps: vec![],
        tilts: vec![],
        orientations: vec![],
        color: Some(Color {
            r: 255,
            g: 203,
            b: 48,
        }),
        pen_width: 12.,
    };
    let page = Page {
        uuid: "marker-test".into(),
        width: 1920,
        height: 1080,
        content_bbox: BoundingBox::default(),
        background_color: None,
        template: None,
        background: Default::default(),
        objects: std::iter::repeat_n(stroke, count).map(Into::into).collect(),
    };
    Document {
        pages: vec![page],
        metadata: Default::default(),
    }
}
fn options() -> RenderOptions {
    RenderOptions {
        legacy_marker_vector_profile: Some(super::super::LegacyMarkerCenterlineProfile {
            screen_width: 1920,
            screen_height: 1080,
        }),
        ..Default::default()
    }
}
fn error(svg: &str, native: &[u8]) -> u64 {
    let tree = resvg::usvg::Tree::from_str(svg, &Default::default()).unwrap();
    let mut pix = resvg::tiny_skia::Pixmap::new(1920, 1080).unwrap();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pix.as_mut(),
    );
    let data = pix.data();
    let bg = [data[0], data[1], data[2]];
    let mut expected = vec![0u8; 1920 * 1080 * 4];
    for row in native.as_chunks::<8>().0.iter() {
        let i = u32::from_le_bytes(row[..4].try_into().unwrap()) as usize;
        expected[i * 4..i * 4 + 4].copy_from_slice(&row[4..]);
    }
    let mut err = 0u64;
    for (actual, native) in data
        .as_chunks::<4>()
        .0
        .iter()
        .zip(expected.as_chunks::<4>().0.iter())
    {
        for c in 0..3 {
            let wanted = (f64::from(native[c])
                + f64::from(bg[c]) * (1. - f64::from(native[3]) / 255.))
                .round()
                .clamp(0., 255.) as i32;
            err += u64::from(actual[c].abs_diff(wanted as u8));
        }
    }
    err
}
#[test]
fn bounded_saved_host_controls_export_svg_and_pdf() {
    for (case, native) in [
        (
            "straight",
            include_bytes!(
                "../../../../../conformance/legacy-marker-source-straight-sparse-rgba8.bin"
            )
            .as_slice(),
        ),
        (
            "bend",
            include_bytes!("../../../../../conformance/legacy-marker-source-bend-sparse-rgba8.bin")
                .as_slice(),
        ),
    ] {
        let doc = document(case, 2, 1);
        let svg = crate::render_document_svg(&doc, &options()).remove(0).svg;
        assert!(svg.contains("linearGradient"));
        let l1 = error(&svg, native);
        let fallback = crate::render_document_svg(&doc, &RenderOptions::default())
            .remove(0)
            .svg;
        let fallback_l1 = error(&fallback, native);
        assert!(
            l1 < 5_000 && l1 < fallback_l1,
            "{case}: vector {l1}, fallback {fallback_l1}"
        );
        let pdf =
            crate::render_document_pdf(&doc, &options(), &crate::PdfOptions::default()).unwrap();
        let parsed = lopdf::Document::load_mem(&pdf).unwrap();
        let forms = parsed
            .objects
            .values()
            .filter_map(|o| o.as_stream().ok())
            .filter(|o| o.dict.get(b"Subtype").ok().and_then(|v| v.as_name().ok()) == Some(b"Form"))
            .filter_map(|o| o.decompressed_content().ok())
            .filter(|data| {
                data.windows(b"/M1 gs /C1 Do".len())
                    .any(|w| w == b"/M1 gs /C1 Do")
            })
            .collect::<Vec<_>>();
        assert_eq!(forms.len(), 1, "one retained Marker form is expected");
        let content = std::str::from_utf8(&forms[0]).unwrap();
        let body = content.find("/A0 gs").unwrap();
        let start = content.find("/M1 gs /C1 Do").unwrap();
        let last = content.rfind(" gs ").unwrap();
        assert!(
            body < start && start < last,
            "body, start cap and final paint must retain order"
        );
    }
}
#[test]
fn invalid_metadata_and_page_budget_fall_back_atomically() {
    let invalid = crate::render_document_svg(&document("bend", 3, 1), &options())
        .remove(0)
        .svg;
    assert!(!invalid.contains("linearGradient"));
    let svg = crate::render_document_svg(&document("bend", 2, 8), &options())
        .remove(0)
        .svg;
    let xml = roxmltree::Document::parse(&svg).unwrap();
    let gradients = xml
        .descendants()
        .filter(|n| n.has_tag_name("linearGradient"))
        .count();
    let single = crate::render_document_svg(&document("bend", 2, 1), &options())
        .remove(0)
        .svg;
    let single = roxmltree::Document::parse(&single).unwrap();
    let single_gradients = single
        .descendants()
        .filter(|n| n.has_tag_name("linearGradient"))
        .count();
    assert!(single_gradients > 0);
    assert_eq!(gradients, 4 * single_gradients);
    assert!(xml.descendants().any(|n| n.has_tag_name("polyline")));
    let pdf = crate::render_document_pdf(
        &document("bend", 2, 8),
        &options(),
        &crate::PdfOptions::default(),
    )
    .unwrap();
    assert!(lopdf::Document::load_mem(&pdf).is_ok());
}

#[test]
fn invalid_cap_gradients_and_empty_mesh_reject_the_whole_stroke() {
    let valid_cap = vec![
        [0., 0., 0., 0.],
        [1., 0., 1., 0.],
        [0., 1., 0., 1.],
        [1., 0., 1., 0.],
        [1., 1., 1., 1.],
        [0., 1., 0., 1.],
    ];
    let mut mesh = crate::ink::legacy_marker_mesh::Mesh {
        body: vec![[0., 0.], [1., 0.], [0., 1.]],
        fringe: [vec![], vec![]],
        caps: [valid_cap.clone(), valid_cap.clone()],
    };
    assert!(super::run(&mesh, 0x80ffcb30, 10_000., &[], [1920., 1080.]).is_some());
    mesh.caps[0][2] = mesh.caps[0][0];
    assert!(super::run(&mesh, 0x80ffcb30, 10_000., &[], [1920., 1080.]).is_none());
    mesh.caps[0] = valid_cap;
    mesh.caps[1][0] = [999_999., 0., 0., 0.];
    mesh.caps[1][1] = [1_000_000., 0., 1., 0.];
    mesh.caps[1][2] = [999_999., 1., 0., 1.];
    assert!(super::run(&mesh, 0x80ffcb30, 10_000., &[], [1920., 1080.]).is_none());
    mesh.body.clear();
    assert!(super::run(&mesh, 0x80ffcb30, 10_000., &[], [1920., 1080.]).is_none());
}

#[test]
fn admitted_marker_advances_rendering_progress() {
    let document = document("bend", 2, 1);
    let layout = crate::layout_document(&document);
    let mut events = Vec::new();
    let rendered = crate::DocumentTextCache::default()
        .render_layout_page_svg_with_progress(
            &document,
            &layout,
            0,
            &options(),
            &crate::fonts::FontBook::default(),
            &mut |event| events.push(event),
        )
        .unwrap();
    assert!(rendered.svg.contains("linearGradient"));
    assert!(events.contains(&crate::Progress::new(
        crate::ProgressStage::Rendering,
        1,
        Some(1)
    )));
}

#[test]
fn top_layer_marker_keeps_order_after_base_stroke() {
    let mut doc = document("bend", 2, 1);
    let marker = doc.pages[0].objects.remove(0);
    let mut base = marker.clone();
    let crate::PageObjectContent::Stroke(stroke) = &mut base.content else {
        panic!("stroke")
    };
    let rendering = stroke.rendering.as_mut().unwrap();
    rendering.tool_type_raw = 3;
    rendering.properties.top_layer_pen = false;
    doc.pages[0].objects = vec![marker, base];
    let svg = crate::render_document_svg(&doc, &options()).remove(0).svg;
    let xml = roxmltree::Document::parse(&svg).unwrap();
    let polyline = xml
        .descendants()
        .find(|n| n.has_tag_name("polyline"))
        .unwrap();
    let gradient = xml
        .descendants()
        .find(|n| n.has_tag_name("linearGradient"))
        .unwrap();
    assert!(polyline.range().start < gradient.range().start);
    assert!(gradient.ancestors().any(|n| {
        n.attribute("style")
            .is_some_and(|s| s.contains("mix-blend-mode"))
    }));
    let pdf = crate::render_document_pdf(&doc, &options(), &crate::PdfOptions::default()).unwrap();
    let pdf = lopdf::Document::load_mem(&pdf).unwrap();
    assert!(
        pdf.objects
            .values()
            .filter_map(|o| o.as_stream().ok())
            .filter_map(|s| s.decompressed_content().ok())
            .any(|s| s
                .windows(b"/M1 gs /C1 Do".len())
                .any(|w| w == b"/M1 gs /C1 Do"))
    );
}

#[test]
fn finer_grid_admits_varied_blue_native_control() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../conformance/legacy-marker-varied-screen-control.json"
    ))
    .unwrap();
    let mut doc = document("bend", 2, 1);
    let crate::PageObjectContent::Stroke(stroke) = &mut doc.pages[0].objects[0].content else {
        panic!("stroke")
    };
    stroke.points = fixture["points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| Point {
            x: p[0].as_f64().unwrap(),
            y: p[1].as_f64().unwrap(),
        })
        .collect();
    stroke.pen_width = 8.;
    stroke.color = Some(Color {
        r: 0x40,
        g: 0x80,
        b: 0xf0,
    });
    let style = &mut stroke.rendering.as_mut().unwrap().style;
    style.pen_size = Some(8.);
    style.color_argb = Some(0xc84080f0);
    let mut opts = options();
    opts.legacy_marker_vector_profile = Some(super::super::LegacyMarkerCenterlineProfile {
        screen_width: 720,
        screen_height: 480,
    });
    let svg = crate::render_document_svg(&doc, &opts).remove(0).svg;
    let fallback = crate::render_document_svg(&doc, &RenderOptions::default())
        .remove(0)
        .svg;
    let native =
        include_bytes!("../../../../../conformance/legacy-marker-varied-screen-sparse-rgba8.bin");
    assert!(svg.contains("linearGradient"));
    let actual = error(&svg, native);
    let generic = error(&fallback, native);
    assert!(
        actual < 10_000 && actual < generic,
        "vector {actual}, fallback {generic}"
    );
    let pdf = crate::render_document_pdf(&doc, &opts, &crate::PdfOptions::default()).unwrap();
    let pdf = lopdf::Document::load_mem(&pdf).unwrap();
    assert!(
        pdf.objects
            .values()
            .filter_map(|o| o.as_stream().ok())
            .filter_map(|s| s.decompressed_content().ok())
            .any(|s| s
                .windows(b"/M1 gs /C1 Do".len())
                .any(|w| w == b"/M1 gs /C1 Do"))
    );
}

#[test]
fn self_overlap_native_control_uses_bounded_vector_paint() {
    let mut doc = document("bend", 2, 1);
    let crate::PageObjectContent::Stroke(stroke) = &mut doc.pages[0].objects[0].content else {
        panic!("stroke")
    };
    stroke.points = [
        [30., 30.],
        [50., 30.],
        [50., 50.],
        [30., 50.],
        [30., 30.],
        [50., 30.],
    ]
    .into_iter()
    .map(|[x, y]| Point { x, y })
    .collect();
    let native =
        include_bytes!("../../../../../conformance/legacy-marker-self-overlap-sparse-rgba8.bin");
    let svg = crate::render_document_svg(&doc, &options()).remove(0).svg;
    let fallback = crate::render_document_svg(&doc, &RenderOptions::default())
        .remove(0)
        .svg;
    assert!(svg.contains("linearGradient"));
    let actual = error(&svg, native);
    let generic = error(&fallback, native);
    assert!(
        actual < 8_000 && actual < generic,
        "vector {actual}, fallback {generic}"
    );
    let pdf = crate::render_document_pdf(&doc, &options(), &crate::PdfOptions::default()).unwrap();
    let pdf = lopdf::Document::load_mem(&pdf).unwrap();
    assert!(
        pdf.objects
            .values()
            .filter_map(|o| o.as_stream().ok())
            .filter_map(|s| s.decompressed_content().ok())
            .any(|s| s
                .windows(b"/M1 gs /C1 Do".len())
                .any(|w| w == b"/M1 gs /C1 Do"))
    );
}

#[test]
fn measured_svg_budget_covers_real_nodes_with_large_ids() {
    let doc = document("bend", 2, 1);
    let crate::PageObjectContent::Stroke(stroke) = &doc.pages[0].objects[0].content else {
        panic!("stroke")
    };
    let source = super::SourceStroke {
        points: stroke
            .points
            .iter()
            .map(|p| [p.x as f32, p.y as f32])
            .collect(),
        size: 12.,
        screen: [1920, 1080],
        argb: 0x80ffcb30,
        scale: 1.,
        page: [1920., 1080.],
    };
    let mut budget = super::SourcePageBudget::default();
    let prepared = super::execute(&source, &mut budget).unwrap();
    let reserved = budget.svg_bytes;
    let mut scene =
        super::super::vector::Scene::new(super::super::vector::Svg::new().width(1920).height(1080));
    for _ in 0..10_000 {
        let _ = scene.definition::<super::super::vector::Gradient>();
    }
    assert!(super::emit(&mut scene, prepared));
    assert!(scene.finish().len() <= reserved);
}

#[test]
fn narrow_effective_widths_choose_backend_fallback() {
    for size in [2.0, 2.5, 3.0, 4.0, 5.0, 5.99, 6.0] {
        let mut doc = document("straight", 2, 1);
        let crate::PageObjectContent::Stroke(stroke) = &mut doc.pages[0].objects[0].content else {
            panic!("stroke");
        };
        stroke.pen_width = size;
        stroke.rendering.as_mut().unwrap().style.pen_size = Some(size);
        let vector_svg = crate::render_document_svg(&doc, &options()).remove(0).svg;
        let fallback_svg = crate::render_document_svg(&doc, &RenderOptions::default())
            .remove(0)
            .svg;
        assert_eq!(vector_svg == fallback_svg, size < 3.0, "SVG size {size}");
        let vector_pdf =
            crate::render_document_pdf(&doc, &options(), &crate::PdfOptions::default()).unwrap();
        let fallback_pdf = crate::render_document_pdf(
            &doc,
            &RenderOptions::default(),
            &crate::PdfOptions::default(),
        )
        .unwrap();
        assert_eq!(vector_pdf == fallback_pdf, size < 6.0, "PDF size {size}");
    }
}

#[test]
fn width_six_svg_improves_on_native_host_control() {
    let mut doc = document("straight", 2, 1);
    let crate::PageObjectContent::Stroke(stroke) = &mut doc.pages[0].objects[0].content else {
        panic!("stroke");
    };
    stroke.pen_width = 6.0;
    stroke.rendering.as_mut().unwrap().style.pen_size = Some(6.0);
    let candidate = crate::render_document_svg(&doc, &options()).remove(0).svg;
    let fallback = crate::render_document_svg(&doc, &RenderOptions::default())
        .remove(0)
        .svg;
    let native = include_bytes!("../../../../../conformance/legacy-marker-size6-sparse-rgba8.bin");
    assert!(error(&candidate, native) < error(&fallback, native));
}
