#![cfg(all(feature = "pdf", feature = "serde"))]

use sdocx::{BoundingBox, Color, Document, DocumentMetadata, Page, RenderOptions, Stroke};
fn stroke() -> Stroke {
    let base: serde_json::Value =
        serde_json::from_str(include_str!("../../../conformance/marker4-v7.json")).unwrap();
    let mut s: Stroke = serde_json::from_value(base["stroke"].clone()).unwrap();
    s.points = vec![
        sdocx::Point { x: 30., y: 30. },
        sdocx::Point { x: 50., y: 40. },
        sdocx::Point { x: 70., y: 55. },
    ];
    s.pressures = vec![0.5; 3];
    s.tilts = vec![0.; 3];
    s.timestamps = vec![0, 8, 16];
    let r = s.rendering.as_mut().unwrap();
    r.tool_type_raw = 0;
    r.style.pen_size = Some(4.);
    s.pen_width = 4.;
    s.color = Some(Color {
        r: 0x33,
        g: 0x96,
        b: 0xff,
    });
    s
}
fn svg(s: Stroke, scale: Option<u32>) -> String {
    let doc = Document {
        pages: vec![Page {
            uuid: "test".into(),
            width: 256,
            height: 256,
            content_bbox: BoundingBox::default(),
            background_color: Some(Color {
                r: 255,
                g: 255,
                b: 255,
            }),
            template: None,
            background: Default::default(),
            objects: vec![s.into()],
        }],
        metadata: DocumentMetadata::default(),
    };
    let mut options = RenderOptions::default();
    options.marker4_v7_finger_output_scale = scale;
    sdocx::render_page_svg(&doc, 0, &options).unwrap().svg
}
#[test]
fn gate_and_fallback() {
    let s = stroke();
    let fallback = svg(s.clone(), None);
    assert_ne!(svg(s.clone(), Some(1)), fallback);
    let mut alias = s.clone();
    alias.rendering.as_mut().unwrap().pen_name =
        Some("com.samsung.android.sdk.pen.pen.preload.StraightHighlighter".into());
    assert_eq!(svg(alias, Some(1)), svg(s.clone(), Some(1)));
    let document = |stroke: Stroke| Document {
        pages: vec![Page {
            uuid: "test".into(),
            width: 256,
            height: 256,
            content_bbox: BoundingBox::default(),
            background_color: Some(Color {
                r: 255,
                g: 255,
                b: 255,
            }),
            template: None,
            background: Default::default(),
            objects: vec![stroke.into()],
        }],
        metadata: DocumentMetadata::default(),
    };
    let mut alias = s.clone();
    alias.rendering.as_mut().unwrap().pen_name =
        Some("com.samsung.android.sdk.pen.pen.preload.StraightHighlighter".into());
    let mut options = RenderOptions::default();
    options.marker4_v7_finger_output_scale = Some(1);
    let pdf = |stroke| {
        sdocx::render_document_pdf(&document(stroke), &options, &Default::default()).unwrap()
    };
    assert_eq!(pdf(alias), pdf(s.clone()));
    // Exercise both public export aliases at the upper admitted width (35).
    let mut upper = s.clone();
    upper.rendering.as_mut().unwrap().style.pen_size = Some(35.);
    upper.pen_width = 35.;
    let mut upper_alias = upper.clone();
    upper_alias.rendering.as_mut().unwrap().pen_name =
        Some("com.samsung.android.sdk.pen.pen.preload.StraightHighlighter".into());
    assert_ne!(svg(upper.clone(), Some(1)), svg(upper.clone(), None));
    assert_eq!(
        svg(upper_alias.clone(), Some(1)),
        svg(upper.clone(), Some(1))
    );
    assert_eq!(pdf(upper_alias), pdf(upper));
    for scale in [0, 3, 5, u32::MAX] {
        assert_eq!(svg(s.clone(), Some(scale)), fallback);
    }
    for width in [0.4, 2., 3.99, 35.01, 800.] {
        let mut v = s.clone();
        v.rendering.as_mut().unwrap().style.pen_size = Some(width);
        assert_eq!(svg(v.clone(), Some(1)), svg(v, None));
    }
    for mutation in 0..5 {
        let mut v = s.clone();
        let r = v.rendering.as_mut().unwrap();
        match mutation {
            0 => r.tool_type_raw = 2,
            1 => r.advanced_settings = Some("8;".into()),
            2 => r.pen_name = Some("com.samsung.android.sdk.pen.pen.preload.UnknownMarker".into()),
            3 => r.properties.fixed_width = true,
            _ => r.properties.straighten = true,
        };
        assert_eq!(svg(v.clone(), Some(1)), svg(v, None));
    }
    for mutation in 0..8 {
        let mut v = s.clone();
        let r = v.rendering.as_mut().unwrap();
        match mutation {
            0 => r.properties.replay_only = true,
            1 => r.properties.alpha_lock = true,
            2 => r.properties.fixed_opacity = true,
            3 => r.properties.rainbow_effect = true,
            4 => r.properties.reveal_mode = true,
            5 => r.properties.generated = true,
            6 => r.style.color_type_raw = Some(1),
            _ => r.style.line_type_raw = Some(1),
        }
        assert_eq!(svg(v.clone(), Some(1)), svg(v, None));
    }
    let mut missing = s.clone();
    missing.color = None;
    assert_eq!(svg(missing.clone(), Some(1)), svg(missing, None));
    let mut missing_argb = s.clone();
    missing_argb.rendering.as_mut().unwrap().style.color_argb = None;
    assert_eq!(svg(missing_argb.clone(), Some(1)), svg(missing_argb, None));
    let mut different = s.clone();
    different.color = Some(Color { r: 1, g: 2, b: 3 });
    assert_eq!(svg(different.clone(), Some(1)), svg(different, None));
    let mut v = s.clone();
    v.pressures.pop();
    assert_eq!(svg(v.clone(), Some(1)), svg(v, None));
    let mut v = s.clone();
    v.points[1].x = 10000.;
    assert_eq!(svg(v.clone(), Some(1)), svg(v, None));
}

#[test]
fn replay_and_descendants_use_fallback() {
    let s = stroke();
    let make_doc = |object| Document {
        pages: vec![Page {
            uuid: "test".into(),
            width: 256,
            height: 256,
            content_bbox: BoundingBox::default(),
            background_color: Some(Color {
                r: 255,
                g: 255,
                b: 255,
            }),
            template: None,
            background: Default::default(),
            objects: vec![object],
        }],
        metadata: DocumentMetadata::default(),
    };
    let doc = make_doc(s.clone().into());
    let layout = sdocx::layout_document(&doc);
    let fallback = RenderOptions::default();
    let mut selected = RenderOptions::default();
    selected.marker4_v7_finger_output_scale = Some(1);
    assert_eq!(
        sdocx::render_layout_page_replay_svg(&doc, &layout, 0, &fallback)
            .unwrap()
            .svg,
        sdocx::render_layout_page_replay_svg(&doc, &layout, 0, &selected)
            .unwrap()
            .svg
    );
    let mut child: sdocx::PageObject = s.into();
    child.content = sdocx::PageObjectContent::Descendant(Box::new(child.content));
    let descendant_doc = make_doc(child);
    assert_eq!(
        sdocx::render_page_svg(&descendant_doc, 0, &fallback)
            .unwrap()
            .svg,
        sdocx::render_page_svg(&descendant_doc, 0, &selected)
            .unwrap()
            .svg
    );
}
