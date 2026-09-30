use sdocx::{BoundingBox, Document, DocumentMetadata, NativeShape, Page, PageElement, RichTextBox};

pub(crate) fn bounds(left: f64, top: f64, width: f64, height: f64) -> BoundingBox {
    BoundingBox {
        x_min: left,
        y_min: top,
        x_max: left + width,
        y_max: top + height,
    }
}

pub(crate) fn text(source: &str) -> RichTextBox {
    RichTextBox {
        text_area_type: None,
        bbox: bounds(71.0, 73.0, 25.0, 60.0),
        rotation_degrees: None,
        text: source.into(),
        color: None,
        highlight_color: None,
        underline: false,
        font_size: Some(10.0),
        runs: Vec::new(),
        spans: Vec::new(),
        paragraphs: Vec::new(),
        object_spans: Vec::new(),
        text_sections: Vec::new(),
        margins: None,
        gravity: None,
    }
}

pub(crate) fn shape(kind: u32, content: RichTextBox) -> NativeShape {
    let geometry = bounds(0.0, 0.0, 200.0, 100.0);
    serde_json::from_value(serde_json::json!({
        "text_editable": true, "text_area_type": null, "shape_type": kind,
        "metadata": {
            "format_version": 1, "uuid": "shape-text-frame", "modified_time_raw": 0,
            "bbox": geometry, "replay_timestamp_raw": 0, "resize_mode_raw": 0,
            "rotatable": true, "selectable": true, "movable": true, "visible": true,
            "replayable": true, "out_of_canvas_enabled": false, "template": false,
            "flip_enabled": false, "float_drawn_rect": false, "locked": false,
            "removable": true, "rotation_degrees": null, "property_mask": [],
            "field_mask": [], "fixed_trailing_data": [], "flexible_trailing_data": []
        },
        "geometry_bbox": geometry, "drawn_bbox": geometry, "rotation_degrees": 0.0,
        "control_points": [], "path_data": [], "style": sdocx::ShapeStyle::default(),
        "fill": "None", "pen_name_id": null, "pen_settings_id": null, "text": content,
    }))
    .unwrap()
}

pub(crate) fn document(shape: NativeShape, density: u32) -> Document {
    Document {
        pages: vec![Page {
            uuid: "shape-frame".into(),
            width: 1080,
            height: 1527,
            content_bbox: bounds(0.0, 0.0, 1080.0, 1527.0),
            background_color: None,
            template: None,
            background: Default::default(),
            objects: vec![PageElement::Shape(shape).into()],
        }],
        metadata: DocumentMetadata {
            default_page_dimensions: Some((360 * density, 1527)),
            orientation: Some(0),
            ..Default::default()
        },
    }
}

pub(crate) fn render(doc: &Document, replay: bool) -> sdocx::RenderedPage {
    let layout = sdocx::layout_document(doc);
    if replay {
        sdocx::render_layout_page_replay_svg(doc, &layout, 0, &Default::default()).unwrap()
    } else {
        sdocx::render_layout_page_svg(doc, &layout, 0, &Default::default()).unwrap()
    }
}

pub(crate) fn assert_lines(page: &sdocx::RenderedPage, expected: &[(&str, f64, f64)]) {
    let xml = roxmltree::Document::parse(&page.svg).unwrap();
    let lines: Vec<_> = xml
        .descendants()
        .filter(|node| node.has_tag_name("text"))
        .collect();
    assert_eq!(lines.len(), expected.len());
    for (line, &(source, x, y)) in lines.into_iter().zip(expected) {
        let spans: Vec<_> = line
            .descendants()
            .filter(|node| node.has_tag_name("tspan"))
            .collect();
        let actual: String = spans.iter().filter_map(|node| node.text()).collect();
        assert_eq!(actual, source);
        let coordinate = |attribute| {
            spans[0]
                .attribute(attribute)
                .or_else(|| line.attribute(attribute))
                .unwrap()
                .split_whitespace()
                .next()
                .unwrap()
                .parse::<f64>()
                .unwrap()
        };
        let actual = (coordinate("x"), coordinate("y"));
        assert!((actual.0 - x).abs() <= 1e-4, "{source}: {actual:?}");
        assert!((actual.1 - y).abs() <= 1e-4, "{source}: {actual:?}");
    }
    assert!(page.object_diagnostics.is_empty());
    assert!(!xml.descendants().any(|node| node.has_tag_name("image")));
}
