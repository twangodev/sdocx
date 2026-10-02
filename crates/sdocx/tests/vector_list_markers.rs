#![cfg(feature = "render")]

use sdocx::{
    BoundingBox, Document, DocumentMetadata, Page, PageElement, RenderColorMode, RenderOptions,
    RichTextBox, RichTextParagraph, RichTextParagraphType,
};

fn document(default_width: u32, marker: u32, indent: u32) -> Document {
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
        paragraphs: vec![
            RichTextParagraph {
                kind: RichTextParagraphType::Bullet,
                start_paragraph: 0,
                end_paragraph: 1,
                payload: [marker, 1, 0, 1]
                    .into_iter()
                    .flat_map(u32::to_le_bytes)
                    .collect(),
            },
            RichTextParagraph {
                kind: RichTextParagraphType::IndentLevel,
                start_paragraph: 0,
                end_paragraph: 1,
                payload: [indent, 1].into_iter().flat_map(u32::to_le_bytes).collect(),
            },
        ],
        object_spans: Vec::new(),
        text_sections: Vec::new(),
        margins: None,
        gravity: None,
    };
    Document {
        pages: vec![Page {
            uuid: "vector-list-markers".into(),
            width: 1080,
            height: 1527,
            content_bbox: BoundingBox::default(),
            background_color: None,
            template: None,
            background: Default::default(),
            objects: vec![PageElement::TextBox(content).into()],
        }],
        metadata: DocumentMetadata {
            default_page_dimensions: Some((default_width, 1527)),
            orientation: Some(0),
            flow_page_padding: Some((10, 0)),
            ..Default::default()
        },
    }
}

fn modes(doc: &Document, color_mode: RenderColorMode) -> [sdocx::RenderedPage; 2] {
    modes_with_fonts(doc, color_mode, &sdocx::fonts::FontBook::default())
}

fn modes_with_fonts(
    doc: &Document,
    color_mode: RenderColorMode,
    fonts: &sdocx::fonts::FontBook,
) -> [sdocx::RenderedPage; 2] {
    let mut options = RenderOptions::default();
    options.color_mode = color_mode;
    let layout = sdocx::layout_document(doc);
    [
        sdocx::render_layout_page_svg_with_fonts(doc, &layout, 0, &options, fonts).unwrap(),
        sdocx::render_layout_page_replay_svg_with_fonts(doc, &layout, 0, &options, fonts).unwrap(),
    ]
}

fn number(node: roxmltree::Node<'_, '_>, attribute: &str) -> f64 {
    node.attribute(attribute).unwrap().parse().unwrap()
}

#[cfg(feature = "serde")]
fn svg_decimal(value: f32) -> f64 {
    (f64::from(value) * 100_000.0).round() / 100_000.0
}

#[cfg(feature = "serde")]
fn mixed_object_document(pixel_spacing: f32, alternate_family: bool) -> Document {
    use base64::Engine;
    let mut doc = document(360, 8, 0);
    doc.metadata.media_assets.push(sdocx::MediaAsset {
        name: "media/marker-object.png".into(), archive_id: None, mime_type: "image/png".into(),
        data: base64::engine::general_purpose::STANDARD.decode(
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC"
        ).unwrap(),
    });
    let sdocx::PageObjectContent::Element(PageElement::TextBox(content)) =
        &mut doc.pages[0].objects[0].content
    else {
        panic!()
    };
    content.text = "A\u{fffc}B".into();
    content.font_size = Some(20.0);
    content.paragraphs.push(RichTextParagraph {
        kind: RichTextParagraphType::LineSpacing,
        start_paragraph: 0,
        end_paragraph: 1,
        payload: [
            0_u32.to_le_bytes().to_vec(),
            pixel_spacing.to_le_bytes().to_vec(),
        ]
        .concat(),
    });
    content.object_spans.push(sdocx::RichTextObjectSpan {
        object_type: sdocx::ObjectType::Image,
        object_data: Vec::new(),
        content: Some(sdocx::RichTextObjectContent::Image(Box::new(
            serde_json::from_value::<sdocx::PlacedImage>(serde_json::json!({
                "bbox": {"x_min":0.0,"y_min":0.0,"x_max":30.0,"y_max":100.0},
                "rotation_degrees":null,"media_id":null,"media_index":0,"crop_rect":null,
                "original_bbox":null,"border_media_id":null,"original_media_id":null,
            }))
            .unwrap(),
        ))),
        text_index_utf16: 1,
        layout_option: sdocx::ObjectSpanLayoutOption::Inline,
        layout_constraint: sdocx::ObjectSpanLayoutConstraint::Normal,
    });
    if alternate_family {
        content.spans.push(sdocx::RichTextSpan {
            kind: sdocx::RichTextSpanType::FontName,
            start_utf16: 0,
            end_utf16: 3,
            interval_type: sdocx::SpanIntervalType::from(1),
            payload: [
                vec![0; 8],
                12_u16.to_le_bytes().to_vec(),
                b"Roboto Mono\0".to_vec(),
            ]
            .concat(),
        });
    }
    doc
}

#[cfg(feature = "serde")]
fn assert_mixed_marker_y(
    doc: &Document,
    expected_center: f64,
    expected_baseline: f64,
    fonts: &sdocx::fonts::FontBook,
    unsupported_family: Option<&str>,
) {
    for page in modes_with_fonts(doc, RenderColorMode::Light, fonts) {
        let xml = roxmltree::Document::parse(&page.svg).unwrap();
        let circle = xml
            .descendants()
            .find(|node| node.has_tag_name("circle"))
            .unwrap();
        assert_eq!(number(circle, "cy"), expected_center);
        let spans: Vec<_> = xml
            .descendants()
            .filter(|node| node.has_tag_name("tspan"))
            .collect();
        assert_eq!(
            spans
                .iter()
                .filter_map(|node| node.text())
                .collect::<String>(),
            "AB"
        );
        for span in spans {
            assert_eq!(number(span, "y"), expected_baseline);
        }
        assert_embedded_face(&xml, fonts, unsupported_family.unwrap_or("Roboto"));
        assert_eq!(
            xml.descendants()
                .filter(|node| node.has_tag_name("image"))
                .count(),
            1
        );
        if let Some(family) = unsupported_family {
            assert_unsupported_font(&page, family, 1);
        } else {
            assert!(
                page.text_diagnostics.is_empty(),
                "{:?}",
                page.text_diagnostics
            );
        }
        assert!(page.object_diagnostics.is_empty());
    }
}

#[cfg(feature = "serde")]
fn assert_embedded_face(
    svg: &roxmltree::Document<'_>,
    fonts: &sdocx::fonts::FontBook,
    family: &str,
) {
    use base64::Engine;
    let selected = fonts.resolve(family, false, false).unwrap();
    let css = svg
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .unwrap()
        .text()
        .unwrap();
    assert!(css.contains(&format!("font-family:\"{}\";", selected.svg_family())));
    let encoded_fonts: Vec<_> = css.split("base64,").skip(1).collect();
    assert_eq!(encoded_fonts.len(), 1);
    let embedded = base64::engine::general_purpose::STANDARD
        .decode(encoded_fonts[0].split('"').next().unwrap())
        .unwrap();
    assert_eq!(embedded, selected.bytes());
}

#[cfg(feature = "serde")]
fn assert_unsupported_font(page: &sdocx::RenderedPage, family: &str, diagnostic_count: usize) {
    assert_eq!(page.text_diagnostics.len(), diagnostic_count);
    let unsupported = page
        .text_diagnostics
        .iter()
        .find(|diagnostic| diagnostic.kind == sdocx::TextDiagnosticKind::UnsupportedMeasurementFont)
        .unwrap();
    assert_eq!(unsupported.family, family);
    assert_eq!(unsupported.codepoints, [u32::from('A'), u32::from('B')]);
}

#[cfg(feature = "serde")]
#[test]
fn zero_pixel_spacing_centers_points_from_the_post_line_cursor_and_object_height() {
    // Native mixed line:100px object +7px ordinary leading +.001 epsilon.
    // Its marker center is107.001−.675*100=39.501, not the font-only center.
    assert_mixed_marker_y(
        &mixed_object_document(0.0, false),
        39.501,
        100.001,
        &sdocx::fonts::FontBook::default(),
        None,
    );
}

#[cfg(feature = "serde")]
#[test]
fn nonzero_pixel_spacing_uses_default_face_caps_even_for_an_alternate_span_family() {
    let regular =
        rustybuzz::Face::from_slice(include_bytes!("../assets/fonts/Roboto-Regular.ttf"), 0)
            .unwrap();
    assert_eq!(regular.units_per_em(), 2048);
    assert_eq!(regular.capital_height(), Some(1456));
    let baseline = 20.0_f32.mul_add(-0.35, 100.0 + 4.0) + 0.001;
    let cap_ratio = 1456.0_f32 / 2048.0;
    let center = baseline - (cap_ratio * 100.0) * 0.5;
    for alternate_family in [false, true] {
        assert_mixed_marker_y(
            &mixed_object_document(4.0, alternate_family),
            svg_decimal(center),
            svg_decimal(baseline),
            &controlled_fonts(1456, 1024),
            alternate_family.then_some("Roboto Mono"),
        );
    }
}

#[cfg(feature = "serde")]
fn font_with_cap_height(source: &[u8], cap_height: i16) -> Vec<u8> {
    let face = rustybuzz::Face::from_slice(source, 0).unwrap();
    let os2 = face
        .raw_face()
        .table(rustybuzz::ttf_parser::Tag::from_bytes(b"OS/2"))
        .unwrap();
    assert_eq!(face.units_per_em(), 2048);
    assert_eq!(face.capital_height(), Some(1456));
    assert_eq!(i16::from_be_bytes(os2[88..90].try_into().unwrap()), 1456);
    let offset = os2.as_ptr() as usize - source.as_ptr() as usize;
    let mut patched = source.to_vec();
    patched[offset + 88..offset + 90].copy_from_slice(&cap_height.to_be_bytes());
    assert_eq!(
        rustybuzz::Face::from_slice(&patched, 0)
            .unwrap()
            .capital_height(),
        Some(cap_height)
    );
    patched
}

#[cfg(feature = "serde")]
fn controlled_fonts(default_caps: i16, span_caps: i16) -> sdocx::fonts::FontBook {
    let mut db = sdocx::fonts::fontdb::Database::new();
    db.load_font_data(font_with_cap_height(
        include_bytes!("../assets/fonts/Roboto-Regular.ttf"),
        default_caps,
    ));
    db.load_font_data(font_with_cap_height(
        include_bytes!("../assets/fonts/RobotoMono-Regular.ttf"),
        span_caps,
    ));
    db.set_sans_serif_family("Roboto");
    let native_names = sdocx::fonts::NativeFontNameConfig::new("Roboto")
        .unwrap()
        .with_family_alias("Roboto", "Roboto")
        .unwrap()
        .with_family_alias("Roboto Mono", "Roboto Mono")
        .unwrap();
    sdocx::fonts::FontBook::new(std::sync::Arc::new(db)).with_native_name_config(native_names)
}

#[cfg(feature = "serde")]
#[test]
fn unusable_default_caps_omit_only_pixel_markers_while_zero_spacing_keeps_artwork() {
    let fonts = controlled_fonts(0, 1024);
    for page in modes_with_fonts(
        &mixed_object_document(4.0, true),
        RenderColorMode::Light,
        &fonts,
    ) {
        let xml = roxmltree::Document::parse(&page.svg).unwrap();
        assert_eq!(
            xml.descendants()
                .filter(|node| node.has_tag_name("tspan"))
                .filter_map(|node| node.text())
                .collect::<String>(),
            "AB"
        );
        assert!(!xml.descendants().any(|node| node.has_tag_name("circle")));
        assert_eq!(
            xml.descendants()
                .filter(|node| node.has_tag_name("image"))
                .count(),
            1
        );
        assert_unsupported_font(&page, "Roboto Mono", 2);
        let marker_failure = page
            .text_diagnostics
            .iter()
            .find(|diagnostic| diagnostic.kind == sdocx::TextDiagnosticKind::MeasurementFailure)
            .unwrap();
        assert_eq!(marker_failure.family, "sans-serif");
        assert!(marker_failure.codepoints.is_empty());
        assert_embedded_face(&xml, &fonts, "Roboto Mono");
        assert!(page.object_diagnostics.is_empty());
    }
    assert_mixed_marker_y(
        &mixed_object_document(0.0, true),
        39.501,
        100.001,
        &fonts,
        Some("Roboto Mono"),
    );
}

#[test]
fn mobile_point_artwork_uses_native_sizes_centers_and_theme_foregrounds() {
    for (width, centers_x, center_y, solid_radius, open_radius, stroke, square_size) in [
        (360, [36.0, 52.0], 10.125, 3.0, 2.625, 0.75, 6.0),
        (720, [62.0, 94.0], 20.25, 5.0, 4.375, 1.25, 10.0),
        (1080, [88.0, 136.0], 30.375, 8.0, 7.0, 2.0, 16.0),
    ] {
        for marker in [8, 9, 11, 12] {
            for (indent, center_x) in [1, 2].into_iter().zip(centers_x) {
                for (color_mode, foreground) in [
                    (RenderColorMode::Auto, "#262626"),
                    (RenderColorMode::Light, "#262626"),
                    (RenderColorMode::Dark, "#d9d9d9"),
                ] {
                    for page in modes(&document(width, marker, indent), color_mode) {
                        let xml = roxmltree::Document::parse(&page.svg).unwrap();
                        let body = xml
                            .descendants()
                            .find(|node| node.has_tag_name("tspan") && node.text() == Some("item"))
                            .unwrap();
                        assert_eq!(
                            body.ancestors().find_map(|node| node.attribute("fill")),
                            Some(foreground)
                        );
                        let shapes: Vec<_> = xml
                            .descendants()
                            .filter(|node| {
                                node.has_tag_name("circle")
                                    || (node.has_tag_name("rect")
                                        && number(*node, "width") != 1080.0)
                            })
                            .collect();
                        assert_eq!(shapes.len(), 1);
                        let shape = shapes[0];
                        let outlined = marker == 9 || marker == 12 || (marker == 8 && indent == 1);
                        if marker == 8 || marker == 9 {
                            assert!(shape.has_tag_name("circle"));
                            assert_eq!(number(shape, "cx"), center_x);
                            assert_eq!(number(shape, "cy"), center_y);
                            assert_eq!(
                                number(shape, "r"),
                                if outlined { open_radius } else { solid_radius }
                            );
                        } else {
                            assert!(shape.has_tag_name("rect"));
                            let inset = if outlined { stroke / 2.0 } else { 0.0 };
                            assert_eq!(number(shape, "x"), center_x - square_size / 2.0 + inset);
                            assert_eq!(number(shape, "y"), center_y - square_size / 2.0 + inset);
                            assert_eq!(number(shape, "width"), square_size - inset * 2.0);
                            assert_eq!(number(shape, "height"), square_size - inset * 2.0);
                        }
                        if outlined {
                            assert_eq!(shape.attribute("fill"), Some("none"));
                            assert_eq!(shape.attribute("stroke"), Some(foreground));
                            assert_eq!(number(shape, "stroke-width"), stroke);
                        } else {
                            assert_eq!(shape.attribute("fill"), Some(foreground));
                            assert!(
                                shape
                                    .attribute("stroke")
                                    .is_none_or(|value| value == "none")
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn arrow_and_diamond_aliases_use_native_point_artwork_and_reservation() {
    for marker in [1, 3] {
        for (width, body_x, center_x, center_y, radius) in [
            (360, 36.0, 20.0, 10.125, 3.0),
            (720, 62.0, 30.0, 20.25, 5.0),
            (1080, 88.0, 40.0, 30.375, 8.0),
        ] {
            for page in modes(&document(width, marker, 0), RenderColorMode::Light) {
                let xml = roxmltree::Document::parse(&page.svg).unwrap();
                let circles: Vec<_> = xml
                    .descendants()
                    .filter(|node| node.has_tag_name("circle"))
                    .collect();
                assert_eq!(circles.len(), 1);
                assert_eq!(number(circles[0], "cx"), center_x);
                assert_eq!(number(circles[0], "cy"), center_y);
                assert_eq!(number(circles[0], "r"), radius);
                let spans: Vec<_> = xml
                    .descendants()
                    .filter(|node| node.has_tag_name("tspan"))
                    .collect();
                assert_eq!(spans.len(), 1);
                assert_eq!(spans[0].text(), Some("item"));
                assert_eq!(
                    spans[0]
                        .attribute("x")
                        .unwrap()
                        .split_whitespace()
                        .next()
                        .unwrap()
                        .parse::<f64>()
                        .unwrap(),
                    body_x
                );
            }
        }
    }
}

#[test]
fn point_markers_keep_only_the_body_as_selectable_text_in_preview_and_replay() {
    for marker in [8, 9, 11, 12] {
        for default_width in [360, 720, 1080] {
            for indent in [1, 2] {
                for color_mode in [
                    RenderColorMode::Auto,
                    RenderColorMode::Light,
                    RenderColorMode::Dark,
                ] {
                    for page in modes(&document(default_width, marker, indent), color_mode) {
                        let xml = roxmltree::Document::parse(&page.svg).unwrap();
                        let source = xml
                            .descendants()
                            .filter(|node| node.has_tag_name("tspan"))
                            .filter_map(|node| node.text())
                            .collect::<String>();
                        assert_eq!(
                            source, "item",
                            "marker{marker}, width{default_width}, indent{indent}, {color_mode:?}"
                        );
                        assert!(
                            !xml.descendants().any(|node| node.has_tag_name("image")
                                || node.has_tag_name("foreignObject"))
                        );
                        assert!(
                            page.text_diagnostics.is_empty(),
                            "{:?}",
                            page.text_diagnostics
                        );
                        assert!(page.object_diagnostics.is_empty());
                    }
                }
            }
        }
    }
}

#[cfg(feature = "pdf")]
fn vector_paints(data: &[u8]) -> usize {
    lopdf::content::Content::decode(data)
        .unwrap()
        .operations
        .iter()
        .filter(|operation| {
            matches!(
                operation.operator.as_str(),
                "f" | "F" | "f*" | "S" | "s" | "B" | "B*" | "b" | "b*"
            )
        })
        .count()
}

#[cfg(feature = "pdf")]
#[test]
fn point_marker_pdf_exports_keep_selectable_body_and_vector_artwork_without_images() {
    for marker in [8, 9, 11, 12] {
        for default_width in [360, 720, 1080] {
            for indent in [1, 2] {
                for color_mode in [
                    RenderColorMode::Auto,
                    RenderColorMode::Light,
                    RenderColorMode::Dark,
                ] {
                    let page = modes(&document(default_width, marker, indent), color_mode)
                        .into_iter()
                        .next()
                        .unwrap();
                    let bytes = sdocx::render_svg_pages_pdf(&[page], &Default::default()).unwrap();
                    let pdf = lopdf::Document::load_mem(&bytes).unwrap();
                    assert_eq!(
                        pdf.extract_text(&[1]).unwrap().replace('\n', "").trim(),
                        "item"
                    );
                    let mut paints =
                        vector_paints(&pdf.get_page_content(pdf.get_pages()[&1]).unwrap());
                    let mut fonts = 0;
                    for object in pdf.objects.values() {
                        if let Ok(stream) = object.as_stream()
                            && let Ok(subtype) =
                                stream.dict.get(b"Subtype").and_then(lopdf::Object::as_name)
                        {
                            assert_ne!(subtype, b"Image");
                            if subtype == b"Form" {
                                paints += vector_paints(&stream.decompressed_content().unwrap());
                            }
                        }
                        if let Ok(dictionary) = object.as_dict() {
                            fonts += usize::from(
                                dictionary.has(b"FontFile2") || dictionary.has(b"FontFile3"),
                            );
                        }
                    }
                    assert!(fonts > 0);
                    // Paper and marker each require a vector paint operation.
                    assert!(paints >= 2, "marker{marker}: {paints} vector paints");
                }
            }
        }
    }
}
