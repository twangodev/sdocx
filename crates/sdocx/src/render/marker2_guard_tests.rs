use crate::{
    BoundingBox, Color, Document, Page, Point, RenderOptions, Stroke, StrokeProperties,
    StrokeRendering, StrokeStyle,
};

fn stroke() -> Stroke {
    Stroke {
        rendering: Some(StrokeRendering {
            metadata: None,
            pen_name: Some("com.samsung.android.sdk.pen.pen.preload.Marker2".into()),
            advanced_settings: Some(String::new()),
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
                color_argb: Some(0x80994422),
                pen_size: Some(2.0),
                stroke_type_raw: Some(0),
                ..Default::default()
            },
        }),
        bbox: BoundingBox::default(),
        points: vec![
            Point { x: 20., y: 20. },
            Point { x: 30., y: 20. },
            Point { x: 40., y: 30. },
            Point { x: 50., y: 30. },
        ],
        pressures: vec![1.; 4],
        timestamps: vec![],
        tilts: vec![],
        orientations: vec![],
        color: Some(Color {
            r: 0x99,
            g: 0x44,
            b: 0x22,
        }),
        pen_width: 2.,
    }
}
fn document(strokes: Vec<Stroke>) -> Document {
    Document {
        pages: vec![Page {
            uuid: "marker2-guard".into(),
            width: 256,
            height: 256,
            content_bbox: BoundingBox::default(),
            background_color: None,
            template: None,
            background: Default::default(),
            objects: strokes.into_iter().map(Into::into).collect(),
        }],
        metadata: Default::default(),
    }
}
fn opts(scale: Option<u32>) -> RenderOptions {
    RenderOptions {
        marker2_output_scale: scale,
        ..Default::default()
    }
}
fn svg(doc: &Document, scale: Option<u32>) -> String {
    crate::render_document_svg(doc, &opts(scale))[0].svg.clone()
}

#[cfg(feature = "serde")]
#[test]
fn marker2_scale_serialization_preserves_opt_in_and_legacy_defaults() {
    assert_eq!(RenderOptions::default().marker2_output_scale, None);
    let mut legacy = serde_json::to_value(RenderOptions::default()).unwrap();
    legacy
        .as_object_mut()
        .unwrap()
        .remove("marker2_output_scale");
    assert_eq!(
        serde_json::from_value::<RenderOptions>(legacy)
            .unwrap()
            .marker2_output_scale,
        None
    );
    for scale in [None, Some(2), Some(3)] {
        let serialized = serde_json::to_value(opts(scale)).unwrap();
        assert_eq!(serialized["marker2_output_scale"], serde_json::json!(scale));
        let decoded: RenderOptions = serde_json::from_value(serialized).unwrap();
        assert_eq!(decoded.marker2_output_scale, scale);
    }
}
#[test]
fn marker2_guard_replay_slice_and_invalid() {
    let original = stroke();
    let doc = document(vec![original.clone()]);
    assert_ne!(
        svg(&doc, None),
        svg(&doc, Some(2)),
        "positive control must select candidate"
    );
    for scale in [Some(0), Some(1), Some(3), Some(u32::MAX)] {
        assert_eq!(svg(&doc, None), svg(&doc, scale), "scale {scale:?}");
    }
    for count in [2, 3, 5, 6, 7, 16] {
        let mut s = original.clone();
        s.points.truncate(count);
        while s.points.len() < count {
            s.points.push(*s.points.last().unwrap());
        }
        let d = document(vec![s]);
        assert_ne!(svg(&d, None), svg(&d, Some(2)), "source count {count}");
    }
    let layout = crate::layout_document(&doc);
    let replay = |scale| {
        crate::render_layout_page_replay_svg(&doc, &layout, 0, &opts(scale))
            .unwrap()
            .svg
    };
    assert_eq!(replay(None), replay(Some(2)), "replay fallback");
    let request = super::PageSceneRequest::sliced(&layout.pages[0], 0, 96, false).unwrap();
    let slice = |scale| {
        let mut cache = super::DocumentTextCache::default();
        cache
            .render_layout_page_scene_requests(
                &doc,
                &[request],
                &opts(scale),
                &super::fonts::FontBook::default(),
                &mut |_| {},
            )
            .remove(0)
            .page
            .svg
    };
    assert_eq!(slice(None), slice(Some(2)), "slice fallback");
    let mut cases = Vec::new();
    let mut single = original.clone();
    single.points.truncate(1);
    cases.push(single);
    for setting in [None, Some("2;"), Some("invalid")] {
        let mut s = original.clone();
        s.rendering.as_mut().unwrap().advanced_settings = setting.map(str::to_owned);
        cases.push(s);
    }
    for tool in [0, 1, 3] {
        let mut s = original.clone();
        s.rendering.as_mut().unwrap().tool_type_raw = tool;
        cases.push(s);
    }
    let mut s = original.clone();
    s.rendering.as_mut().unwrap().properties.compressed = false;
    cases.push(s);
    let mut s = original.clone();
    s.rendering.as_mut().unwrap().style.stroke_type_raw = Some(1);
    cases.push(s);
    let mut s = original.clone();
    s.rendering.as_mut().unwrap().style.pen_size = Some(4.);
    s.pen_width = 4.;
    cases.push(s);
    let mut s = original.clone();
    s.points[0].x = 300.;
    cases.push(s);
    for (i, s) in cases.into_iter().enumerate() {
        let d = document(vec![s]);
        assert_eq!(svg(&d, None), svg(&d, Some(2)), "guard case {i}");
        assert_eq!(
            crate::render_document_pdf(&d, &opts(None), &Default::default()).unwrap(),
            crate::render_document_pdf(&d, &opts(Some(2)), &Default::default()).unwrap(),
            "PDF guard case {i}"
        );
    }
}
fn svg_fragment(doc: &Document, scale: Option<u32>) -> String {
    let empty = svg(&document(vec![]), scale);
    let rendered = svg(doc, scale);
    let closing = "</svg>";
    let prefix = empty.strip_suffix(closing).expect("empty SVG closing tag");
    rendered
        .strip_prefix(prefix)
        .and_then(|s| s.strip_suffix(closing))
        .expect("stroke SVG must share the empty page wrapper")
        .to_owned()
}
#[test]
fn marker2_multistroke_preserves_unsupported_neighbor() {
    let supported = stroke();
    let mut unsupported = stroke();
    unsupported.rendering.as_mut().unwrap().advanced_settings = Some("2;".into());
    for p in &mut unsupported.points {
        p.y += 40.;
    }
    let supported_doc = document(vec![supported.clone()]);
    let unsupported_doc = document(vec![unsupported.clone()]);
    let baseline_supported = svg_fragment(&supported_doc, None);
    let candidate_supported = svg_fragment(&supported_doc, Some(2));
    let baseline_unsupported = svg_fragment(&unsupported_doc, None);
    assert!(!baseline_supported.is_empty() && !baseline_unsupported.is_empty());
    assert_ne!(
        baseline_supported, candidate_supported,
        "positive control must select candidate"
    );
    assert_eq!(
        baseline_unsupported,
        svg_fragment(&unsupported_doc, Some(2)),
        "unsupported standalone fallback"
    );
    for (strokes, expected_baseline, expected_candidate) in [
        (
            vec![supported.clone(), unsupported.clone()],
            format!("{baseline_supported}{baseline_unsupported}"),
            format!("{candidate_supported}{baseline_unsupported}"),
        ),
        (
            vec![unsupported, supported],
            format!("{baseline_unsupported}{baseline_supported}"),
            format!("{baseline_unsupported}{candidate_supported}"),
        ),
    ] {
        let combined = document(strokes);
        assert_eq!(
            svg_fragment(&combined, None),
            expected_baseline,
            "baseline source order"
        );
        assert_eq!(
            svg_fragment(&combined, Some(2)),
            expected_candidate,
            "candidate and unsupported fallback in source order"
        );
    }
}

#[test]
fn marker2_page_budget_atomic_fallback_and_reset() {
    for width in [2., 3.] {
        let mut s = stroke();
        s.pen_width = width;
        s.rendering.as_mut().unwrap().style.pen_size = Some(width);
        marker2_page_budget_for_stroke(s);
    }
}

fn marker2_page_budget_for_stroke(s: Stroke) {
    let single = document(vec![s.clone()]);
    let candidate = svg_fragment(&single, Some(2));
    let fallback = svg_fragment(&single, None);
    assert_ne!(candidate, fallback);
    let subpaths = candidate.matches('M').count();
    assert_eq!(subpaths % 4, 0);
    let cost = subpaths / 4 * 4 * 128;
    assert!(cost > 0 && cost < 65_536);
    let admitted = 65_536 / cost;
    assert!(admitted < 100);
    let mut small = s.clone();
    for (i, p) in small.points.iter_mut().enumerate() {
        p.x = 20.0 + i as f64 * 0.5;
        p.y = 20.0;
    }
    let small_candidate = svg_fragment(&document(vec![small.clone()]), Some(2));
    let small_fallback = svg_fragment(&document(vec![small.clone()]), None);
    assert_ne!(small_candidate, small_fallback);
    let small_cost = small_candidate.matches('M').count() * 128;
    assert!(small_cost <= 65_536 - admitted * cost);
    assert_eq!(65_536 % small_cost, 0);
    let exact = 65_536 / small_cost;
    let boundary = document(vec![small.clone(); exact + 1]);
    assert_eq!(
        svg_fragment(&boundary, Some(2)),
        format!("{}{}", small_candidate.repeat(exact), small_fallback)
    );
    fn pdf_fragment(doc: &Document, scale: Option<u32>) -> (Vec<u8>, lopdf::Dictionary) {
        let bytes = crate::render_document_pdf(doc, &opts(scale), &Default::default()).unwrap();
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        let page = *pdf.get_pages().values().next().unwrap();
        let content = pdf.get_page_content(page).unwrap();
        let marker = b"\nf\nQ\n";
        let start = content
            .windows(marker.len())
            .position(|w| w == marker)
            .unwrap()
            + marker.len();
        let (direct, inherited) = pdf.get_page_resources(page).unwrap();
        let resources = direct
            .or_else(|| inherited.iter().find_map(|id| pdf.get_dictionary(*id).ok()))
            .unwrap();
        let states = pdf
            .dereference(resources.get(b"ExtGState").unwrap())
            .unwrap()
            .1
            .as_dict()
            .unwrap();
        let mut resolved = lopdf::Dictionary::new();
        for (name, state) in states.iter() {
            resolved.set(name.clone(), pdf.dereference(state).unwrap().1.clone());
        }
        (
            content[start..]
                .strip_suffix(b"\nQ\n")
                .or_else(|| content[start..].strip_suffix(b"\nQ"))
                .unwrap()
                .to_vec(),
            resolved,
        )
    }
    let (pdf_candidate, candidate_states) = pdf_fragment(&document(vec![small.clone()]), Some(2));
    let (pdf_fallback, fallback_states) = pdf_fragment(&document(vec![small.clone()]), None);
    let (pdf_boundary, boundary_states) = pdf_fragment(&boundary, Some(2));
    let mut expected_pdf = Vec::new();
    for _ in 0..exact {
        if !expected_pdf.is_empty() {
            expected_pdf.push(b'\n');
        }
        expected_pdf.extend_from_slice(&pdf_candidate);
    }
    expected_pdf.push(b'\n');
    expected_pdf.extend_from_slice(&pdf_fallback);
    assert_eq!(
        boundary_states.get(b"g0").unwrap(),
        candidate_states.get(b"g0").unwrap()
    );
    assert_eq!(
        boundary_states.get(b"g1").unwrap(),
        fallback_states.get(b"g0").unwrap()
    );
    let normalized = String::from_utf8(pdf_boundary)
        .unwrap()
        .replace("/g1 gs", "/g0 gs");
    assert_eq!(
        normalized.as_bytes(),
        expected_pdf,
        "PDF ordered candidate and fallback operators (graphics state names normalized)"
    );
    let mut mixed = vec![s.clone(); admitted + 1];
    mixed.push(small);
    assert_eq!(
        svg_fragment(&document(mixed), Some(2)),
        format!(
            "{}{}{}",
            candidate.repeat(admitted),
            fallback,
            small_candidate
        )
    );
    let page = document(vec![s.clone(); admitted + 2]);
    let expected = format!("{}{}", candidate.repeat(admitted), fallback.repeat(2));
    assert_eq!(svg_fragment(&page, Some(2)), expected);
    let mut two_pages = page.clone();
    two_pages.pages.push(page.pages[0].clone());
    let rendered = crate::render_document_svg(&two_pages, &opts(Some(2)));
    assert_eq!(rendered[0].svg, rendered[1].svg);
    let pdf = crate::render_document_pdf(&two_pages, &opts(Some(2)), &Default::default()).unwrap();
    let baseline_pdf =
        crate::render_document_pdf(&two_pages, &opts(None), &Default::default()).unwrap();
    let baseline = lopdf::Document::load_mem(&baseline_pdf).unwrap();
    let baseline_first = *baseline.get_pages().values().next().unwrap();
    let parsed = lopdf::Document::load_mem(&pdf).unwrap();
    let pages = parsed.get_pages();
    let content: Vec<_> = pages
        .values()
        .map(|id| parsed.get_page_content(*id).unwrap())
        .collect();
    assert_eq!(content.len(), 2);
    assert_eq!(content[0], content[1]);
    assert_ne!(
        content[0],
        baseline.get_page_content(baseline_first).unwrap(),
        "PDF must select candidate"
    );
}
