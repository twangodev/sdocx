#![cfg(feature = "render")]

use sdocx::{
    BoundingBox, Color, Document, DocumentMetadata, Page, PageElement, RichTextBox,
    RichTextParagraph, RichTextParagraphType, RichTextSpan, RichTextSpanType,
};

fn text(value: &str) -> RichTextBox {
    RichTextBox {
        text_area_type: None,
        bbox: BoundingBox {
            x_min: 20.0,
            y_min: 20.0,
            x_max: 220.0,
            y_max: 220.0,
        },
        rotation_degrees: None,
        text: value.into(),
        color: Some(Color { r: 0, g: 0, b: 0 }),
        highlight_color: None,
        underline: false,
        font_size: Some(15.0),
        runs: Vec::new(),
        spans: Vec::new(),
        paragraphs: Vec::new(),
        object_spans: Vec::new(),
        text_sections: Vec::new(),
        margins: Some([2.0, 3.0, 4.0, 5.0]),
        gravity: None,
    }
}

fn document(element: PageElement) -> Document {
    Document {
        pages: vec![Page {
            uuid: "placed-layout".into(),
            width: 1080,
            height: 1527,
            content_bbox: BoundingBox::default(),
            background_color: Some(Color {
                r: 255,
                g: 255,
                b: 255,
            }),
            template: None,
            background: Default::default(),
            objects: vec![element.into()],
        }],
        metadata: DocumentMetadata {
            default_page_dimensions: Some((1080, 1527)),
            orientation: Some(0),
            ..Default::default()
        },
    }
}

fn render(content: RichTextBox) -> String {
    sdocx::render_page_svg(
        &document(PageElement::TextBox(content)),
        0,
        &Default::default(),
    )
    .unwrap()
    .svg
}

fn lines(svg: &str) -> Vec<(String, f64, f64)> {
    let xml = roxmltree::Document::parse(svg).unwrap();
    xml.descendants()
        .filter(|node| node.has_tag_name("text"))
        .map(|node| {
            let value = node
                .descendants()
                .filter(|node| node.is_text())
                .map(|node| node.text().unwrap())
                .collect();
            (
                value,
                node.attribute("x").unwrap().parse().unwrap(),
                node.descendants()
                    .find(|child| child.has_tag_name("tspan") && child.attribute("y").is_some())
                    .and_then(|child| child.attribute("y"))
                    .or_else(|| node.attribute("y"))
                    .unwrap()
                    .parse()
                    .unwrap(),
            )
        })
        .collect()
}

fn paragraph(
    kind: RichTextParagraphType,
    start: u32,
    end: u32,
    payload: Vec<u8>,
) -> RichTextParagraph {
    RichTextParagraph {
        kind,
        start_paragraph: start,
        end_paragraph: end,
        payload,
    }
}

#[test]
fn pinned_roboto_width_fits_exactly_and_wraps_below_the_boundary() {
    // Roboto-Regular hmtx/shaping fixture: ABC = 3944 / 2048 * 45 px.
    let mut content = text("ABC");
    content.bbox.x_max = 320.0;
    content.margins = Some([0.0, 0.0, 71.0 + 29.0 / 256.0, 0.0]);
    assert_eq!(
        lines(&render(content.clone())),
        vec![("ABC".into(), 20.0, 65.0)]
    );
    content.margins.as_mut().unwrap()[2] += 1.0 / 1024.0;
    assert_eq!(
        lines(&render(content)),
        vec![("AB".into(), 20.0, 65.0), ("C".into(), 20.0, 125.75),]
    );
}

#[test]
fn native_outer_width_is_rounded_up_before_text_is_wrapped() {
    for outer_width in [86.66, 87.0] {
        let mut content = text("ABC");
        content.margins = None;
        content.bbox.x_max = 20.0 + outer_width;
        assert_eq!(lines(&render(content)), vec![("ABC".into(), 20.0, 65.0)]);
    }
}

#[test]
fn density_scales_margins_without_changing_stored_geometry() {
    let content = text("ABC\nABC");
    let doc = document(PageElement::TextBox(content));
    let svg = sdocx::render_page_svg(&doc, 0, &Default::default())
        .unwrap()
        .svg;
    assert_eq!(
        lines(&svg),
        vec![("ABC".into(), 26.0, 74.0), ("ABC".into(), 26.0, 134.75),]
    );
    let PageElement::TextBox(original) = doc.pages[0].elements().next().unwrap() else {
        panic!()
    };
    assert_eq!(original.margins, Some([2.0, 3.0, 4.0, 5.0]));
    assert_eq!(original.bbox.x_min, 20.0);
    assert_eq!(original.text, "ABC\nABC");
}

#[test]
fn alignment_uses_measured_inner_width_and_clamps_oversized_lines() {
    for (alignment, expected_x) in [(0_u32, 26.0), (1, 121.34), (2, 73.67)] {
        let mut content = text("ABC");
        content.paragraphs.push(paragraph(
            RichTextParagraphType::Alignment,
            0,
            1,
            alignment.to_le_bytes().to_vec(),
        ));
        assert_eq!(
            lines(&render(content.clone())),
            vec![("ABC".into(), expected_x, 74.0)]
        );
        content.text = "W".into();
        content.bbox.x_max = 40.0;
        assert_eq!(lines(&render(content)), vec![("W".into(), 26.0, 74.0)]);
    }
}

#[test]
fn wrapped_text_preserves_spaces_and_combining_clusters() {
    let source = "A  e\u{301}  B  ";
    let mut content = text(source);
    content.bbox.x_max = 77.0;
    let svg = render(content);
    let output = lines(&svg);
    assert!(output.len() > 1);
    assert_eq!(
        output
            .iter()
            .map(|line| line.0.as_str())
            .collect::<String>(),
        source
    );
    assert!(output.iter().any(|line| line.0.contains("e\u{301}")));
    assert!(
        !output
            .iter()
            .any(|line| line.0.starts_with('\u{301}') || line.0.ends_with('e'))
    );
    let xml = roxmltree::Document::parse(&svg).unwrap();
    for node in xml.descendants().filter(|node| node.has_tag_name("text")) {
        assert_eq!(
            node.attribute(("http://www.w3.org/XML/1998/namespace", "space")),
            Some("preserve")
        );
    }
}

#[test]
fn paragraph_spacing_and_crlf_ordinals_are_applied_to_placed_text() {
    let mut content = text("ABC\r\nABC");
    content.paragraphs = vec![
        paragraph(
            RichTextParagraphType::SpacingBefore,
            0,
            1,
            2.0_f32.to_le_bytes().to_vec(),
        ),
        paragraph(
            RichTextParagraphType::SpacingAfter,
            0,
            1,
            4.0_f32.to_le_bytes().to_vec(),
        ),
        paragraph(
            RichTextParagraphType::SpacingBefore,
            2,
            3,
            3.0_f32.to_le_bytes().to_vec(),
        ),
        paragraph(
            RichTextParagraphType::Alignment,
            2,
            3,
            1_u32.to_le_bytes().to_vec(),
        ),
    ];
    assert_eq!(
        lines(&render(content)),
        vec![("ABC".into(), 26.0, 80.0), ("ABC".into(), 121.34, 161.75),]
    );
}

#[test]
fn wrapped_lines_use_their_own_local_font_size_maximum() {
    let mut content = text("A B");
    content.bbox.x_max = 95.0;
    content.spans.push(RichTextSpan {
        kind: RichTextSpanType::FontSize,
        start_utf16: 2,
        end_utf16: 3,
        expand: true,
        payload: 20.0_f32.to_le_bytes().to_vec(),
    });
    assert_eq!(
        lines(&render(content)),
        vec![("A ".into(), 26.0, 74.0), ("B".into(), 26.0, 149.75),]
    );
}

#[test]
fn rotation_remains_centered_on_the_stored_box_after_insets() {
    let mut content = text("ABC");
    content.rotation_degrees = Some(30.0);
    let svg = render(content);
    let xml = roxmltree::Document::parse(&svg).unwrap();
    assert!(
        xml.descendants()
            .any(|node| node.attribute("transform") == Some("rotate(30.00 120.00 120.00)"))
    );
    assert_eq!(lines(&svg), vec![("ABC".into(), 26.0, 74.0)]);
}

#[cfg(feature = "serde")]
#[test]
fn unsupported_shape_text_retains_the_measured_saved_frame() {
    for (spacing, first, second) in [
        (None, 65.0, 125.75),
        (Some((0_u32, 4.0_f32)), 61.25, 118.25),
        (Some((1_u32, 1.5_f32)), 71.75, 139.25),
    ] {
        let mut content = text("ABCABC");
        content.bbox.x_max = 106.66015625;
        content.margins = None;
        if let Some((kind, value)) = spacing {
            content.paragraphs.push(paragraph(
                RichTextParagraphType::LineSpacing,
                0,
                1,
                [kind.to_le_bytes(), value.to_le_bytes()].concat(),
            ));
        }
        let shape: sdocx::NativeShape = serde_json::from_value(serde_json::json!({
            "text_editable": true, "text_area_type": null, "shape_type": 0,
            "metadata": {
                "format_version": 1, "uuid": "shape", "modified_time_raw": 0,
                "bbox": content.bbox, "replay_timestamp_raw": 0, "resize_mode_raw": 0,
                "rotatable": true, "selectable": true, "movable": true, "visible": true,
                "replayable": true, "out_of_canvas_enabled": false, "template": false,
                "flip_enabled": false, "float_drawn_rect": false, "locked": false,
                "removable": true, "rotation_degrees": null, "property_mask": [],
                "field_mask": [], "fixed_trailing_data": [], "flexible_trailing_data": []
            },
            "geometry_bbox": content.bbox, "drawn_bbox": content.bbox, "rotation_degrees": 0.0,
            "control_points": [], "path_data": [], "style": sdocx::ShapeStyle::default(),
            "fill": "None", "pen_name_id": null, "pen_settings_id": null, "text": content,
        }))
        .unwrap();
        let page =
            sdocx::render_page_svg(&document(PageElement::Shape(shape)), 0, &Default::default())
                .unwrap();
        assert_eq!(page.text_diagnostics.len(), 1);
        assert_eq!(
            page.text_diagnostics[0].kind,
            sdocx::TextDiagnosticKind::UnsupportedTextFrame
        );
        assert_eq!(
            lines(&page.svg),
            vec![("ABC".into(), 20.0, first), ("ABC".into(), 20.0, second)],
            "{spacing:?}"
        );
    }
}

#[test]
fn vertical_gravity_uses_measured_line_height_and_scaled_margins() {
    // Native ordinary drawing height: top9 + line60.75 + bottom15 = 84.75.
    for (gravity, baseline) in [(0, 74.0), (1, 131.625), (2, 189.25), (99, 74.0)] {
        let mut content = text("ABC");
        content.gravity = Some(gravity);
        assert_eq!(
            lines(&render(content)),
            vec![("ABC".into(), 26.0, baseline)]
        );
    }
    // Two lines measure 145.5; the same outer height leaves 54.5 unused.
    for (gravity, first, second) in [(1, 101.25, 162.0), (2, 128.5, 189.25)] {
        let mut content = text("ABC\nABC");
        content.gravity = Some(gravity);
        assert_eq!(
            lines(&render(content)),
            vec![("ABC".into(), 26.0, first), ("ABC".into(), 26.0, second),]
        );
    }
}

#[test]
fn explicit_spacing_positions_baselines_before_gravity_translation() {
    for (kind, spacing, gravity, first, second) in [
        (0_u32, 4.0_f32, 0, 70.25, 127.25),
        (0, 4.0, 1, 101.25, 158.25),
        (0, 4.0, 2, 132.25, 189.25),
        (1, 1.5, 0, 80.75, 148.25),
        (1, 1.5, 1, 101.25, 168.75),
        (1, 1.5, 2, 121.75, 189.25),
        (1, 0.5, 0, 35.75, 58.25),
        (1, 0.5, 1, 101.25, 123.75),
        (1, 0.5, 2, 166.75, 189.25),
    ] {
        let mut content = text("ABC\nABC");
        content.gravity = Some(gravity);
        content.paragraphs.push(paragraph(
            RichTextParagraphType::LineSpacing,
            0,
            2,
            [kind.to_le_bytes(), spacing.to_le_bytes()].concat(),
        ));
        assert_eq!(
            lines(&render(content)),
            vec![("ABC".into(), 26.0, first), ("ABC".into(), 26.0, second)],
            "kind={kind}, spacing={spacing}, gravity={gravity}"
        );
    }
}

#[test]
fn gravity_clamps_oversized_content_without_moving_it_upward() {
    for gravity in [1, 2] {
        let mut content = text("ABC\nABC");
        content.gravity = Some(gravity);
        content.bbox.y_max = 100.0;
        assert_eq!(
            lines(&render(content)),
            vec![("ABC".into(), 26.0, 74.0), ("ABC".into(), 26.0, 134.75),]
        );
    }
}

#[test]
fn gravity_rounds_outer_height_up_and_preserves_the_original_rotation_pivot() {
    let mut content = text("ABC");
    content.bbox.y_max = 220.2;
    content.gravity = Some(1);
    content.rotation_degrees = Some(30.0);
    let doc = document(PageElement::TextBox(content));
    let svg = sdocx::render_page_svg(&doc, 0, &Default::default())
        .unwrap()
        .svg;
    assert_eq!(lines(&svg), vec![("ABC".into(), 26.0, 132.125)]);
    let xml = roxmltree::Document::parse(&svg).unwrap();
    assert!(
        xml.descendants()
            .any(|node| node.attribute("transform") == Some("rotate(30.00 120.00 120.10)"))
    );
    let PageElement::TextBox(original) = doc.pages[0].elements().next().unwrap() else {
        panic!()
    };
    assert_eq!(original.bbox.y_max, 220.2);
    assert_eq!(original.gravity, Some(1));
    assert_eq!(doc.metadata.document_density(), 3.0);
}

#[test]
fn gravity_excludes_final_paragraph_after_spacing_but_keeps_between_paragraphs() {
    let mut single = text("ABC");
    single.gravity = Some(1);
    single.paragraphs.push(paragraph(
        RichTextParagraphType::SpacingAfter,
        0,
        1,
        20.0_f32.to_le_bytes().to_vec(),
    ));
    assert_eq!(lines(&render(single)), vec![("ABC".into(), 26.0, 131.625)]);

    let mut multiple = text("ABC\nABC");
    multiple.gravity = Some(1);
    multiple.paragraphs = vec![
        paragraph(
            RichTextParagraphType::SpacingAfter,
            0,
            1,
            4.0_f32.to_le_bytes().to_vec(),
        ),
        paragraph(
            RichTextParagraphType::SpacingAfter,
            1,
            2,
            20.0_f32.to_le_bytes().to_vec(),
        ),
    ];
    assert_eq!(
        lines(&render(multiple)),
        vec![("ABC".into(), 26.0, 95.25), ("ABC".into(), 26.0, 168.0),]
    );
}

#[test]
fn whitespace_only_placed_text_retains_source_and_receives_gravity() {
    let mut content = text("  ");
    content.gravity = Some(2);
    let svg = render(content);
    assert_eq!(lines(&svg), vec![("  ".into(), 26.0, 189.25)]);
    let xml = roxmltree::Document::parse(&svg).unwrap();
    assert_eq!(
        xml.descendants()
            .find(|node| node.has_tag_name("text"))
            .unwrap()
            .attribute(("http://www.w3.org/XML/1998/namespace", "space")),
        Some("preserve")
    );
}

#[test]
fn empty_placed_text_keeps_its_highlight_without_an_empty_text_node() {
    let mut content = text("");
    content.margins = Some([0.0; 4]);
    content.gravity = Some(1);
    content.highlight_color = Some(Color {
        r: 255,
        g: 255,
        b: 0,
    });
    let svg = render(content);
    assert!(lines(&svg).is_empty());
    let xml = roxmltree::Document::parse(&svg).unwrap();
    let highlight = xml
        .descendants()
        .find(|node| node.has_tag_name("rect") && node.attribute("fill") == Some("#ffff00"))
        .unwrap();
    for (attribute, expected) in [
        ("x", "20.00"),
        ("y", "20.00"),
        ("width", "200.00"),
        ("height", "200.00"),
    ] {
        assert_eq!(highlight.attribute(attribute), Some(expected));
    }
}
