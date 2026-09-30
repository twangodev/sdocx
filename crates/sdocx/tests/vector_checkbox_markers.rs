#![cfg(feature = "render")]

use sdocx::{
    BoundingBox, Document, DocumentMetadata, Page, PageElement, PointMarkerTarget, RenderColorMode,
    RenderOptions, RichTextBox, RichTextParagraph, RichTextParagraphType, RichTextSpan,
    RichTextSpanType,
};

fn document(width: u32, checked: bool) -> Document {
    let content = RichTextBox {
        text_area_type: None,
        bbox: BoundingBox::default(),
        rotation_degrees: None,
        text: "item".into(),
        color: None,
        highlight_color: None,
        underline: false,
        font_size: Some(15.0),
        runs: Vec::new(),
        spans: Vec::new(),
        paragraphs: vec![RichTextParagraph {
            kind: RichTextParagraphType::Bullet,
            start_paragraph: 0,
            end_paragraph: 1,
            payload: [2, 1, u32::from(checked), 1]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect(),
        }],
        object_spans: Vec::new(),
        text_sections: Vec::new(),
        margins: None,
        gravity: None,
    };
    Document {
        pages: vec![Page {
            uuid: "vector-checkbox".into(),
            width: 1080,
            height: 1527,
            content_bbox: BoundingBox::default(),
            background_color: None,
            template: None,
            background: Default::default(),
            objects: vec![PageElement::TextBox(content).into()],
        }],
        metadata: DocumentMetadata {
            default_page_dimensions: Some((width, 1527)),
            orientation: Some(0),
            flow_page_padding: Some((10, 0)),
            ..Default::default()
        },
    }
}

fn content(document: &mut Document) -> &mut RichTextBox {
    let PageElement::TextBox(content) = document.pages[0].elements_mut().next().unwrap() else {
        panic!()
    };
    content
}

fn modes(
    doc: &Document,
    target: PointMarkerTarget,
    color: RenderColorMode,
) -> [sdocx::RenderedPage; 2] {
    let mut options = RenderOptions::default();
    options.point_marker_target = target;
    options.color_mode = color;
    let layout = sdocx::layout_document(doc);
    [
        sdocx::render_layout_page_svg(doc, &layout, 0, &options).unwrap(),
        sdocx::render_layout_page_replay_svg(doc, &layout, 0, &options).unwrap(),
    ]
}

fn number(node: roxmltree::Node<'_, '_>, name: &str) -> f64 {
    node.ancestors()
        .find_map(|node| node.attribute(name))
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap()
}

fn point(node: roxmltree::Node<'_, '_>, mut point: (f64, f64)) -> (f64, f64) {
    for ancestor in node.ancestors() {
        if let Some(value) = ancestor.attribute("transform") {
            let transform: svgtypes::Transform = value.parse().unwrap();
            point = (
                transform.a * point.0 + transform.c * point.1 + transform.e,
                transform.b * point.0 + transform.d * point.1 + transform.f,
            );
        }
    }
    point
}

fn checkbox<'a>(xml: &'a roxmltree::Document<'a>) -> roxmltree::Node<'a, 'a> {
    let paths: Vec<_> = xml
        .descendants()
        .filter(|node| node.has_tag_name("path"))
        .collect();
    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0].attribute("fill"), Some("none"));
    assert!(
        !xml.descendants()
            .any(|node| node.has_tag_name("image") || node.has_tag_name("foreignObject"))
    );
    paths[0]
}

fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 0.0001, "{actual} != {expected}");
}

fn geometry(
    page: &sdocx::RenderedPage,
    side: f64,
    center: (f64, f64),
    body: (f64, f64),
    source: &str,
) {
    let xml = roxmltree::Document::parse(&page.svg).unwrap();
    let path = checkbox(&xml);
    let p0 = point(path, (0.0, 0.0));
    let p24 = point(path, (24.0, 0.0));
    close((p24.0 - p0.0).hypot(p24.1 - p0.1), side);
    let actual = point(path, (12.0, 12.0));
    close(actual.0, center.0);
    close(actual.1, center.1);
    let spans: Vec<_> = xml
        .descendants()
        .filter(|node| node.has_tag_name("tspan"))
        .collect();
    assert_eq!(
        spans
            .iter()
            .filter_map(|node| node.text())
            .collect::<String>(),
        source
    );
    let position = point(spans[0], (number(spans[0], "x"), number(spans[0], "y")));
    close(position.0, body.0);
    assert!((position.1 - body.1).abs() < 0.011);
    assert!(
        page.text_diagnostics.is_empty(),
        "{:?}",
        page.text_diagnostics
    );
    assert!(page.object_diagnostics.is_empty());
}

#[test]
fn checked_and_unchecked_assets_keep_native_compound_geometry_tint_and_alpha() {
    for checked in [false, true] {
        for (mode, foreground) in [
            (RenderColorMode::Light, "#262626"),
            (RenderColorMode::Dark, "#d9d9d9"),
        ] {
            for page in modes(&document(1080, checked), PointMarkerTarget::Mobile, mode) {
                geometry(&page, 48.0, (40.0, 29.375), (88.0, 45.0), "item");
                let xml = roxmltree::Document::parse(&page.svg).unwrap();
                let path = checkbox(&xml);
                assert_eq!(path.attribute("stroke"), Some(foreground));
                assert_eq!(number(path, "stroke-width"), 1.5);
                assert_eq!(path.attribute("stroke-linecap"), Some("round"));
                assert_eq!(path.attribute("stroke-linejoin"), Some("round"));
                let segments = svgtypes::PathParser::from(path.attribute("d").unwrap())
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap();
                assert_eq!(
                    segments
                        .iter()
                        .filter(|segment| matches!(segment, svgtypes::PathSegment::MoveTo { .. }))
                        .count(),
                    if checked { 2 } else { 1 }
                );
                assert_eq!(
                    segments
                        .iter()
                        .filter(|segment| matches!(segment, svgtypes::PathSegment::CurveTo { .. }))
                        .count(),
                    if checked { 5 } else { 4 }
                );
                assert!(matches!(
                    segments[0],
                    svgtypes::PathSegment::MoveTo {
                        abs: true,
                        x: 17.0,
                        y: 19.5
                    }
                ));
                assert_eq!(
                    segments
                        .iter()
                        .filter(|segment| matches!(
                            segment,
                            svgtypes::PathSegment::ClosePath { .. }
                        ))
                        .count(),
                    1
                );
                if checked {
                    assert!(
                        matches!(segments.last().unwrap(), svgtypes::PathSegment::LineTo { abs: true, x, y } if *x == 8.013 && *y == 11.257)
                    );
                }
                let alphas: Vec<f64> = path
                    .ancestors()
                    .flat_map(|node| [node.attribute("opacity"), node.attribute("stroke-opacity")])
                    .flatten()
                    .map(|value| value.parse::<f64>().unwrap())
                    .filter(|value| *value != 1.0)
                    .collect();
                assert_eq!(alphas, if checked { vec![0.4] } else { vec![] });
            }
        }
    }
}

#[test]
fn display_profiles_and_density_use_native_checkbox_viewports_and_reservation() {
    for target in [
        PointMarkerTarget::Mobile,
        PointMarkerTarget::Tablet,
        PointMarkerTarget::Uwp,
    ] {
        for (width, side, center, body) in [
            (360, 16.0, (20.0, 9.125), (36.0, 15.0)),
            (720, 32.0, (30.0, 19.25), (62.0, 30.0)),
            (1080, 48.0, (40.0, 29.375), (88.0, 45.0)),
        ] {
            for page in modes(&document(width, false), target, RenderColorMode::Light) {
                geometry(&page, side, center, body, "item");
            }
        }
    }
    for (target, side) in [
        (PointMarkerTarget::Mobile, 18.0),
        (PointMarkerTarget::Tablet, 18.0),
        (PointMarkerTarget::Uwp, 14.0),
    ] {
        let mut doc = document(720, false);
        content(&mut doc).font_size = Some(1.0);
        for page in modes(&doc, target, RenderColorMode::Light) {
            geometry(&page, side, (30.0, 0.35), (62.0, 2.0), "item");
        }
    }
}

#[test]
fn native_integer_ratio_order_preserves_the_checkbox_size_boundary() {
    let mut doc = document(360, false);
    content(&mut doc).font_size = Some(f32::from_bits(0x4153_c3c3));
    for page in modes(&doc, PointMarkerTarget::Mobile, RenderColorMode::Light) {
        geometry(
            &page,
            14.0,
            (20.0, 7.933823037147523),
            (36.0, 13.2352933883667),
            "item",
        );
    }
}

#[test]
fn maximum_checkbox_artwork_does_not_expand_the_native_button_reservation() {
    let mut doc = document(1080, false);
    content(&mut doc).font_size = Some(100.0);
    for target in [
        PointMarkerTarget::Mobile,
        PointMarkerTarget::Tablet,
        PointMarkerTarget::Uwp,
    ] {
        for page in modes(&doc, target, RenderColorMode::Light) {
            geometry(&page, 96.0, (40.0, 201.5), (88.0, 300.0), "item");
        }
    }
}

#[test]
fn font_delta_and_first_content_font_control_checkbox_size_independently_of_line_maximum() {
    let mut doc = document(1080, false);
    doc.metadata.body_font_size_delta = Some(1);
    for page in modes(&doc, PointMarkerTarget::Mobile, RenderColorMode::Light) {
        geometry(&page, 52.0, (40.0, 31.4), (88.0, 48.0), "item");
    }
    doc.metadata.body_font_size_delta = None;
    let content = content(&mut doc);
    content.text = "AB".into();
    for (start, end, size) in [(0, 1, 5.0_f32), (1, 2, 30.0_f32)] {
        content.spans.push(RichTextSpan {
            kind: RichTextSpanType::FontSize,
            start_utf16: start,
            end_utf16: end,
            interval_type: sdocx::SpanIntervalType::from(1),
            payload: size.to_le_bytes().to_vec(),
        });
    }
    for page in modes(&doc, PointMarkerTarget::Mobile, RenderColorMode::Light) {
        geometry(&page, 26.0, (40.0, 59.75), (88.0, 90.0), "AB");
    }
}

#[test]
fn explicit_pixel_spacing_centers_checkbox_from_default_face_cap_height() {
    let mut doc = document(1080, false);
    content(&mut doc).paragraphs.push(RichTextParagraph {
        kind: RichTextParagraphType::LineSpacing,
        start_paragraph: 0,
        end_paragraph: 1,
        payload: [0_u32.to_le_bytes().to_vec(), 4.0_f32.to_le_bytes().to_vec()].concat(),
    });
    for page in modes(&doc, PointMarkerTarget::Mobile, RenderColorMode::Light) {
        geometry(&page, 48.0, (40.0, 24.25390625), (88.0, 41.25), "item");
    }
}

#[test]
fn placed_vertical_gravity_moves_the_retained_marker_and_body_together() {
    for (gravity, center_y, baseline) in
        [(0, 49.375, 65.0), (1, 119.0, 134.625), (2, 188.625, 204.25)]
    {
        let mut doc = document(1080, true);
        let content = content(&mut doc);
        content.bbox = BoundingBox {
            x_min: 10.0,
            y_min: 20.0,
            x_max: 1010.0,
            y_max: 220.0,
        };
        content.gravity = Some(gravity);
        for page in modes(&doc, PointMarkerTarget::Mobile, RenderColorMode::Light) {
            geometry(&page, 48.0, (40.0, center_y), (88.0, baseline), "item");
        }
    }
}

#[cfg(feature = "pdf")]
#[test]
fn checkbox_pdf_keeps_selectable_body_and_vector_artwork_without_images() {
    for checked in [false, true] {
        let page = modes(
            &document(1080, checked),
            PointMarkerTarget::Mobile,
            RenderColorMode::Dark,
        )
        .into_iter()
        .next()
        .unwrap();
        let bytes = sdocx::render_svg_pages_pdf(&[page], &Default::default()).unwrap();
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        assert_eq!(
            pdf.extract_text(&[1]).unwrap().replace('\n', "").trim(),
            "item"
        );
        let mut paints = 0;
        let page_content = pdf.get_page_content(pdf.get_pages()[&1]).unwrap();
        let mut streams = vec![page_content];
        for object in pdf.objects.values() {
            if let Ok(stream) = object.as_stream()
                && let Ok(subtype) = stream.dict.get(b"Subtype").and_then(lopdf::Object::as_name)
            {
                assert_ne!(subtype, b"Image");
                if subtype == b"Form" {
                    streams.push(stream.decompressed_content().unwrap());
                }
            }
        }
        for stream in streams {
            paints += lopdf::content::Content::decode(&stream)
                .unwrap()
                .operations
                .iter()
                .filter(|operation| {
                    matches!(
                        operation.operator.as_str(),
                        "f" | "f*" | "S" | "s" | "B" | "B*"
                    )
                })
                .count();
        }
        assert!(paints >= 2);
    }
}
