#![cfg(all(feature = "render", feature = "serde"))]

use sdocx::{
    BoundingBox, Color, Document, DocumentMetadata, MediaAsset, ObjectSpanLayoutConstraint,
    ObjectSpanLayoutOption, ObjectType, Page, PageElement, PlacedImage, RichTextBox,
    RichTextCodeBlock, RichTextObjectContent, RichTextObjectSpan, RichTextSection, RichTextSpan,
    RichTextSpanType, RichTextTable, RichTextTableCell, RichTextTableRow,
};

const RED: u32 = 0xffff0011;
const BLUE: u32 = 0x800011ff;

#[derive(Clone, Copy, Debug)]
enum Context {
    Placed,
    Flow,
    Shape,
    Table,
    CodeTitle,
    CodeBody,
}

const CONTEXTS: [Context; 6] = [
    Context::Placed,
    Context::Flow,
    Context::Shape,
    Context::Table,
    Context::CodeTitle,
    Context::CodeBody,
];

fn bounds(left: f64, top: f64, width: f64, height: f64) -> BoundingBox {
    BoundingBox {
        x_min: left,
        y_min: top,
        x_max: left + width,
        y_max: top + height,
    }
}

fn text(source: &str) -> RichTextBox {
    RichTextBox {
        text_area_type: None,
        bbox: bounds(20.0, 20.0, 200.0, 200.0),
        rotation_degrees: None,
        text: source.into(),
        color: Some(Color { r: 0, g: 0, b: 0 }),
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

fn span(kind: RichTextSpanType, start: u32, end: u32, payload: Vec<u8>) -> RichTextSpan {
    RichTextSpan {
        kind,
        start_utf16: start,
        end_utf16: end,
        interval_type: sdocx::SpanIntervalType::from(1),
        payload,
    }
}

fn background(start: u32, end: u32, argb: u32) -> RichTextSpan {
    span(
        RichTextSpanType::BackgroundColor,
        start,
        end,
        argb.to_le_bytes().to_vec(),
    )
}

fn font_name(start: u32, end: u32, family: &str) -> RichTextSpan {
    span(
        RichTextSpanType::FontName,
        start,
        end,
        [
            vec![0; 8],
            u16::try_from(family.len() + 1)
                .unwrap()
                .to_le_bytes()
                .to_vec(),
            family.as_bytes().to_vec(),
            vec![0],
        ]
        .concat(),
    )
}

fn object(content: RichTextObjectContent, kind: ObjectType) -> RichTextObjectSpan {
    RichTextObjectSpan {
        object_type: kind,
        object_data: Vec::new(),
        content: Some(content),
        text_index_utf16: 0,
        layout_option: ObjectSpanLayoutOption::Inline,
        layout_constraint: ObjectSpanLayoutConstraint::Normal,
    }
}

fn document(context: Context, mut content: RichTextBox) -> Document {
    let element = match context {
        Context::Placed => PageElement::TextBox(content),
        Context::Flow => {
            content.bbox = BoundingBox::default();
            content.margins = Some([0.0, 20.0, 0.0, 0.0]);
            PageElement::TextBox(content)
        }
        Context::Shape => PageElement::Shape(
            serde_json::from_value(serde_json::json!({
                "text_editable": true, "text_area_type": null, "shape_type": 4,
                "metadata": {
                    "format_version": 1, "uuid": "background-shape", "modified_time_raw": 0,
                    "bbox": content.bbox, "replay_timestamp_raw": 0, "resize_mode_raw": 0,
                    "rotatable": true, "selectable": true, "movable": true, "visible": true,
                    "replayable": true, "out_of_canvas_enabled": false, "template": false,
                    "flip_enabled": false, "float_drawn_rect": false, "locked": false,
                    "removable": true, "rotation_degrees": null, "property_mask": [],
                    "field_mask": [], "fixed_trailing_data": [], "flexible_trailing_data": []
                },
                "geometry_bbox": content.bbox, "drawn_bbox": content.bbox, "rotation_degrees": 0.0,
                "control_points": [], "path_data": [], "style": sdocx::ShapeStyle::default(),
                "fill": "None", "pen_name_id": null, "pen_settings_id": null, "text": content
            }))
            .unwrap(),
        ),
        Context::Table | Context::CodeTitle | Context::CodeBody => {
            let bbox = content.bbox;
            let (kind, embedded) = match context {
                Context::Table => (
                    ObjectType::Table,
                    RichTextObjectContent::Table(Box::new(RichTextTable {
                        style: serde_json::from_str(
                            r#"{
                        "heading_column_enabled": false, "heading_row_enabled": false,
                        "max_height_enabled": false,
                        "metadata": {"property_mask": [], "field_mask": [],
                            "fixed_trailing_data": [], "flexible_trailing_data": []}
                    }"#,
                        )
                        .unwrap(),
                        bbox,
                        rotation_degrees: None,
                        column_widths: vec![200.0],
                        rows: vec![RichTextTableRow {
                            max_height: None,
                            min_height: None,
                            metadata: Default::default(),
                            index: 0,
                            height: 200.0,
                            cells: vec![RichTextTableCell {
                                border: None,
                                metadata: Default::default(),
                                column_index: 0,
                                row_span: 1,
                                column_span: 1,
                                background_color: 0,
                                has_own_background_color: false,
                                bbox,
                                editable: false,
                                content,
                            }],
                        }],
                    })),
                ),
                Context::CodeTitle => (
                    ObjectType::CodeBlock,
                    RichTextObjectContent::CodeBlock(Box::new(RichTextCodeBlock {
                        bbox,
                        rotation_degrees: None,
                        title: Some(content),
                        body: None,
                    })),
                ),
                Context::CodeBody => (
                    ObjectType::CodeBlock,
                    RichTextObjectContent::CodeBlock(Box::new(RichTextCodeBlock {
                        bbox,
                        rotation_degrees: None,
                        title: None,
                        body: Some(content),
                    })),
                ),
                _ => unreachable!(),
            };
            let mut parent = text("\u{fffc}");
            parent.object_spans.push(object(embedded, kind));
            PageElement::TextBox(parent)
        }
    };
    Document {
        pages: vec![Page {
            uuid: "background-spans".into(),
            width: 360,
            height: 400,
            content_bbox: bounds(0.0, 0.0, 360.0, 400.0),
            background_color: None,
            template: None,
            background: Default::default(),
            objects: vec![element.into()],
        }],
        metadata: DocumentMetadata {
            default_page_dimensions: Some((360, 400)),
            orientation: Some(0),
            flow_page_padding: Some((20, 0)),
            ..Default::default()
        },
    }
}

fn render(doc: &Document, page: usize, replay: bool) -> sdocx::RenderedPage {
    let layout = sdocx::layout_document(doc);
    if replay {
        sdocx::render_layout_page_replay_svg(doc, &layout, page, &Default::default()).unwrap()
    } else {
        sdocx::render_layout_page_svg(doc, &layout, page, &Default::default()).unwrap()
    }
}

fn source(svg: &str) -> String {
    roxmltree::Document::parse(svg)
        .unwrap()
        .descendants()
        .filter(|node| node.has_tag_name("tspan"))
        .filter_map(|node| node.text())
        .collect()
}

fn point(node: roxmltree::Node<'_, '_>) -> (f64, f64) {
    let number = |attribute| {
        node.attribute(attribute)
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .parse::<f64>()
            .unwrap()
    };
    let mut point = (number("x"), number("y"));
    for ancestor in node.ancestors() {
        if let Some(transform) = ancestor.attribute("transform") {
            let transform: svgtypes::Transform = transform.parse().unwrap();
            point = (
                transform.a * point.0 + transform.c * point.1 + transform.e,
                transform.b * point.0 + transform.d * point.1 + transform.f,
            );
        }
    }
    point
}

fn rectangles(svg: &str, color: &str) -> Vec<[f64; 5]> {
    let xml = roxmltree::Document::parse(svg).unwrap();
    xml.descendants()
        .filter(|node| node.has_tag_name("rect") && node.attribute("fill") == Some(color))
        .map(|node| {
            let (x, y) = point(node);
            let number = |attribute| node.attribute(attribute).unwrap().parse::<f64>().unwrap();
            let opacity = node
                .attribute("fill-opacity")
                .or_else(|| node.attribute("opacity"))
                .unwrap_or("1")
                .parse::<f64>()
                .unwrap();
            [x, y, number("width"), number("height"), opacity]
        })
        .filter(|rectangle| rectangle[4] > 0.0)
        .collect()
}

fn assert_rectangles(svg: &str, color: &str, expected: &[[f64; 5]]) {
    let actual = rectangles(svg, color);
    assert_eq!(actual.len(), expected.len(), "{color}: {actual:?}");
    for (actual, expected) in actual.iter().zip(expected) {
        for (actual_value, expected_value) in actual.iter().zip(expected) {
            assert!(
                (actual_value - expected_value).abs() <= 1e-4,
                "{color}: actual {actual:?}, expected {expected:?}"
            );
        }
    }
}

fn glyph_geometry(svg: &str) -> Vec<(String, String, String)> {
    let xml = roxmltree::Document::parse(svg).unwrap();
    xml.descendants()
        .filter(|node| node.has_tag_name("tspan"))
        .flat_map(|node| {
            let xs: Vec<_> = node.attribute("x").unwrap().split_whitespace().collect();
            node.text()
                .unwrap()
                .chars()
                .zip(xs)
                .map(move |(character, x)| {
                    (
                        character.to_string(),
                        x.to_owned(),
                        node.attribute("y").unwrap().to_owned(),
                    )
                })
        })
        .collect()
}

fn foreground_nodes(svg: &str) -> Vec<String> {
    let xml = roxmltree::Document::parse(svg).unwrap();
    xml.descendants()
        .filter(|node| node.has_tag_name("text"))
        .map(|node| svg[node.range()].to_owned())
        .collect()
}

#[test]
fn partial_background_uses_retained_glyph_advance_and_full_line_height_in_every_context() {
    // Roboto A/B hmtx = 1336/1275 units, UPEM 2048. F10 line height is 13.5.
    for context in CONTEXTS {
        let (left, top) = match context {
            Context::CodeTitle => (36.0, 32.001),
            Context::CodeBody => (36.0, 64.001),
            _ => (20.0, 20.0),
        };
        let mut content = text("ABC");
        content.spans.push(background(1, 2, RED));
        let doc = document(context, content);
        let original = serde_json::to_value(&doc).unwrap();
        for replay in [false, true] {
            let page = render(&doc, 0, replay);
            assert_eq!(source(&page.svg), "ABC");
            assert_rectangles(
                &page.svg,
                "#ff0011",
                &[[left + 6.5234375, top, 6.2255859375, 13.5, 1.0]],
            );
            assert!(
                page.text_diagnostics.is_empty(),
                "{context:?}: {:?}",
                page.text_diagnostics
            );
            assert!(page.object_diagnostics.is_empty());
        }
        assert_eq!(serde_json::to_value(&doc).unwrap(), original);
    }
}

#[test]
fn last_background_span_wins_even_when_it_clears_color_with_zero_alpha() {
    let mut content = text("ABC");
    content.spans = vec![
        background(0, 3, RED),
        background(1, 2, 0x00ff0011),
        background(2, 3, BLUE),
    ];
    for replay in [false, true] {
        let page = render(&document(Context::Placed, content.clone()), 0, replay);
        assert_rectangles(&page.svg, "#ff0011", &[[20.0, 20.0, 6.5234375, 13.5, 1.0]]);
        assert_rectangles(
            &page.svg,
            "#0011ff",
            &[[32.7490234375, 20.0, 6.5087890625, 13.5, 128.0 / 255.0]],
        );
        assert_eq!(source(&page.svg), "ABC");
    }
}

#[test]
fn multiline_background_follows_each_line_maximum_size_without_changing_layout() {
    let mut plain = text("AB\nC");
    plain.spans.push(span(
        RichTextSpanType::FontSize,
        1,
        2,
        20_f32.to_le_bytes().to_vec(),
    ));
    let mut highlighted = plain.clone();
    highlighted.spans.push(background(0, 4, RED));
    for replay in [false, true] {
        let control = render(&document(Context::Placed, plain.clone()), 0, replay);
        let page = render(&document(Context::Placed, highlighted.clone()), 0, replay);
        assert_eq!(glyph_geometry(&page.svg), glyph_geometry(&control.svg));
        assert_eq!(source(&page.svg), "ABC");
        assert_rectangles(
            &page.svg,
            "#ff0011",
            &[
                [20.0, 20.0, 18.974609375, 27.0, 1.0],
                [20.0, 47.0, 6.5087890625, 13.5, 1.0],
            ],
        );
    }
}

#[test]
fn background_does_not_split_kerning_or_change_resolved_font_selection() {
    let mut mono = text("AB");
    mono.spans.push(font_name(1, 2, "Roboto Mono"));
    for plain in [text("AV"), mono] {
        let mut highlighted = plain.clone();
        highlighted.spans.push(background(1, 2, RED));
        for replay in [false, true] {
            let control = render(&document(Context::Placed, plain.clone()), 0, replay);
            let page = render(&document(Context::Placed, highlighted.clone()), 0, replay);
            assert_eq!(glyph_geometry(&page.svg), glyph_geometry(&control.svg));
            let expected = if plain.text == "AV" {
                // Direct pinned rustybuzz AV: A 1249, V 1303, total 2552 units.
                [26.0986328125, 20.0, 6.3623046875, 13.5, 1.0]
            } else {
                let font = rustybuzz::ttf_parser::Face::parse(
                    include_bytes!("../assets/fonts/RobotoMono-Regular.ttf"),
                    0,
                )
                .unwrap();
                let glyph = font.glyph_index('B').unwrap();
                let width = f64::from(font.glyph_hor_advance(glyph).unwrap()) * 10.0
                    / f64::from(font.units_per_em());
                [26.5234375, 20.0, width, 13.5, 1.0]
            };
            assert_rectangles(&page.svg, "#ff0011", &[expected]);
            assert!(page.text_diagnostics.is_empty());
        }
    }
}

#[test]
fn invalid_utf16_ranges_and_carets_never_paint_but_an_aligned_span_after_emoji_does() {
    for invalid in [
        background(0, 0, RED),
        background(3, 2, RED),
        background(0, 9, RED),
        background(2, 4, RED),
    ] {
        let mut content = text("A😀B");
        content.spans.push(invalid);
        for replay in [false, true] {
            let page = render(&document(Context::Placed, content.clone()), 0, replay);
            assert_eq!(source(&page.svg), "A😀B");
            assert!(rectangles(&page.svg, "#ff0011").is_empty());
        }
    }
    let mut content = text("A😀B");
    content.spans.push(background(3, 4, RED));
    for replay in [false, true] {
        let control = render(&document(Context::Placed, text("A😀B")), 0, replay);
        let page = render(&document(Context::Placed, content.clone()), 0, replay);
        assert_eq!(source(&page.svg), "A😀B");
        let backgrounds = rectangles(&page.svg, "#ff0011");
        assert_eq!(backgrounds.len(), 1);
        assert!((backgrounds[0][2] - 6.2255859375).abs() <= 1e-4);
        assert_eq!(glyph_geometry(&page.svg), glyph_geometry(&control.svg));
    }
}

#[test]
fn backgrounds_exclude_valid_embedded_object_anchors_and_their_horizontal_extent() {
    use base64::Engine;
    let mut content = text("A\u{fffc}B");
    let image = serde_json::from_value::<PlacedImage>(serde_json::json!({
        "bbox": bounds(0.0, 0.0, 30.0, 40.0), "rotation_degrees": null,
        "media_id": null, "media_index": 0, "crop_rect": null, "original_bbox": null,
        "border_media_id": null, "original_media_id": null
    }))
    .unwrap();
    let mut anchor = object(
        RichTextObjectContent::Image(Box::new(image)),
        ObjectType::Image,
    );
    anchor.text_index_utf16 = 1;
    content.object_spans.push(anchor);
    for range in [(0, 3), (1, 2)] {
        let mut highlighted = content.clone();
        highlighted.spans.push(background(range.0, range.1, RED));
        let mut doc = document(Context::Placed, highlighted);
        doc.metadata.media_assets.push(MediaAsset {
            name: "media/background-object.png".into(), archive_id: None,
            mime_type: "image/png".into(), data: base64::engine::general_purpose::STANDARD.decode(
                "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC"
            ).unwrap(),
        });
        for replay in [false, true] {
            let page = render(&doc, 0, replay);
            assert_eq!(source(&page.svg), "AB");
            let highlights = rectangles(&page.svg, "#ff0011");
            if range == (1, 2) {
                assert!(highlights.is_empty());
            } else {
                assert_eq!(highlights.len(), 2);
                for (rectangle, (x, width)) in highlights
                    .iter()
                    .zip([(20.0, 6.5234375), (56.5234375, 6.2255859375)])
                {
                    assert!((rectangle[0] - x).abs() <= 1e-4);
                    assert!((rectangle[2] - width).abs() <= 1e-4);
                }
            }
        }
    }
}

#[test]
fn native_whole_span_suppresses_the_summary_box_but_author_only_highlight_remains() {
    let mut author = text("ABC");
    author.highlight_color = Some(Color {
        r: 255,
        g: 0,
        b: 17,
    });
    let mut native = author.clone();
    native.spans.push(background(0, 3, RED));
    for replay in [false, true] {
        let author_page = render(&document(Context::Placed, author.clone()), 0, replay);
        assert_rectangles(
            &author_page.svg,
            "#ff0011",
            &[[20.0, 20.0, 200.0, 200.0, 1.0]],
        );
        let native_page = render(&document(Context::Placed, native.clone()), 0, replay);
        assert_rectangles(
            &native_page.svg,
            "#ff0011",
            &[[20.0, 20.0, 19.2578125, 13.5, 1.0]],
        );
        assert_eq!(
            glyph_geometry(&native_page.svg),
            glyph_geometry(&author_page.svg)
        );
    }
}

fn assert_background_clip(svg: &str, expected: [f64; 4]) {
    let xml = roxmltree::Document::parse(svg).unwrap();
    let background = xml
        .descendants()
        .find(|node| node.has_tag_name("rect") && node.attribute("fill") == Some("#ff0011"))
        .unwrap();
    let clipped = background
        .ancestors()
        .find(|node| node.attribute("clip-path").is_some())
        .unwrap();
    let reference = clipped.attribute("clip-path").unwrap();
    let id = reference
        .strip_prefix("url(#")
        .unwrap()
        .strip_suffix(')')
        .unwrap();
    let clip = xml
        .descendants()
        .find(|node| node.attribute("id") == Some(id))
        .unwrap();
    assert!(clip.has_tag_name("clipPath"));
    let rectangle = clip
        .children()
        .find(|node| node.has_tag_name("rect"))
        .unwrap();
    let coordinates = ["x", "y", "width", "height"].map(|attribute| {
        rectangle
            .attribute(attribute)
            .unwrap()
            .parse::<f64>()
            .unwrap()
    });
    assert_eq!(coordinates, expected);
    assert!(
        xml.descendants()
            .filter(|node| node.has_tag_name("text"))
            .all(|node| node
                .ancestors()
                .all(|ancestor| ancestor.attribute("clip-path").is_none()))
    );
}

#[test]
fn placed_background_clips_to_fractional_original_bounds_without_shortening_line_height() {
    let mut content = text("A\nB");
    content.bbox = bounds(20.0, 20.0, 200.0, 20.2);
    content.spans.push(background(0, 3, RED));
    for replay in [false, true] {
        let page = render(&document(Context::Placed, content.clone()), 0, replay);
        assert_rectangles(
            &page.svg,
            "#ff0011",
            &[
                [20.0, 20.0, 6.5234375, 13.5, 1.0],
                [20.0, 33.5, 6.2255859375, 13.5, 1.0],
            ],
        );
        assert_background_clip(&page.svg, [20.0, 20.0, 200.0, 20.2]);
        assert_eq!(source(&page.svg), "AB");
        assert!(page.text_diagnostics.is_empty());
    }
}

#[test]
fn rotated_ellipse_background_clips_to_original_geometry_rather_than_the_inset_text_frame() {
    let mut content = text("ABC");
    content.spans.push(background(1, 2, RED));
    let mut doc = document(Context::Shape, content);
    let sdocx::PageObjectContent::Element(PageElement::Shape(shape)) =
        &mut doc.pages[0].objects[0].content
    else {
        panic!()
    };
    shape.shape_type = 1;
    shape.geometry_bbox = bounds(0.0, 0.0, 200.0, 100.0);
    shape.rotation_degrees = 30.0;
    for replay in [false, true] {
        let page = render(&doc, 0, replay);
        assert_background_clip(&page.svg, [0.0, 0.0, 200.0, 100.0]);
        let xml = roxmltree::Document::parse(&page.svg).unwrap();
        let rectangle = xml
            .descendants()
            .find(|node| node.has_tag_name("rect") && node.attribute("fill") == Some("#ff0011"))
            .unwrap();
        assert_eq!(
            rectangle.attribute("y").unwrap().parse::<f64>().unwrap(),
            15.0
        );
        assert!(
            rectangle
                .ancestors()
                .any(|node| node.attribute("transform") == Some("rotate(30.00 100.00 50.00)"))
        );
        assert_eq!(source(&page.svg), "ABC");
        assert!(page.text_diagnostics.is_empty());
    }
}

#[test]
fn unsupported_rtl_background_reports_positioning_without_painting_logical_order_rectangles() {
    for (source_text, start, end) in [("אב", 0, 1), ("\u{202e}AV\u{202c}", 1, 2)] {
        let plain = text(source_text);
        let mut highlighted = plain.clone();
        highlighted.spans.push(background(start, end, RED));
        for replay in [false, true] {
            let control = render(&document(Context::Placed, plain.clone()), 0, replay);
            let page = render(&document(Context::Placed, highlighted.clone()), 0, replay);
            assert_eq!(source(&page.svg), source_text);
            assert!(rectangles(&page.svg, "#ff0011").is_empty());
            assert!(
                page.text_diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.kind
                        == sdocx::TextDiagnosticKind::UnsupportedBackgroundPositioning)
            );
            assert_eq!(foreground_nodes(&page.svg), foreground_nodes(&control.svg));
        }
    }
}

#[test]
fn partial_combining_cluster_background_keeps_foreground_geometry_and_reports_unsupported_bounds() {
    for (start, end) in [(0, 1), (1, 2)] {
        let plain = text("e\u{301}");
        let mut highlighted = plain.clone();
        highlighted.spans.push(background(start, end, RED));
        for replay in [false, true] {
            let control = render(&document(Context::Placed, plain.clone()), 0, replay);
            let page = render(&document(Context::Placed, highlighted.clone()), 0, replay);
            assert_eq!(source(&page.svg), "e\u{301}");
            assert!(rectangles(&page.svg, "#ff0011").is_empty());
            assert_eq!(foreground_nodes(&page.svg), foreground_nodes(&control.svg));
            assert_eq!(
                page.text_diagnostics.len(),
                1,
                "{:?}",
                page.text_diagnostics
            );
            assert_eq!(
                page.text_diagnostics[0].kind,
                sdocx::TextDiagnosticKind::UnsupportedBackgroundPositioning
            );
        }
    }
}

#[test]
fn whitespace_background_is_visible_only_inside_its_capture_viewport() {
    let mut body = text(" \n ");
    body.bbox = BoundingBox::default();
    body.spans.push(background(0, 3, RED));
    body.text_sections = vec![
        RichTextSection {
            start_utf16: 0,
            length_utf16: 2,
        },
        RichTextSection {
            start_utf16: 1,
            length_utf16: 2,
        },
    ];
    let mut doc = document(Context::Placed, text(""));
    doc.metadata.note_text = Some(body);
    doc.metadata.page_mode = Some(0);
    doc.metadata.flow_page_padding = Some((0, 10));
    doc.metadata.default_page_dimensions = Some((360, 40));
    doc.pages[0].height = 40;
    doc.pages[0].content_bbox = bounds(0.0, 0.0, 360.0, 40.0);
    doc.pages[0].objects.clear();
    let mut second = doc.pages[0].clone();
    second.uuid = "background-second".into();
    doc.pages.push(second);
    // Each physical page has one F10 line at top 10, baseline 20. Space hmtx 507.
    for replay in [false, true] {
        let second = render(&doc, 1, replay);
        assert_rectangles(
            &second.svg,
            "#ff0011",
            &[[0.0, 10.0, 2.4755859375, 13.5, 1.0]],
        );
        assert_eq!(source(&second.svg), " ");
        let first = render(&doc, 0, replay);
        assert_rectangles(
            &first.svg,
            "#ff0011",
            &[[0.0, 10.0, 2.4755859375, 13.5, 1.0]],
        );
        assert_eq!(source(&first.svg), " ");
        assert_eq!(render(&doc, 1, replay), second);
    }
}

#[cfg(feature = "pdf")]
#[test]
fn styled_background_pdf_keeps_selectable_vector_text_in_every_context() {
    for context in CONTEXTS {
        let mut content = text("ABC");
        content.spans.push(background(1, 2, BLUE));
        for replay in [false, true] {
            let page = render(&document(context, content.clone()), 0, replay);
            assert_eq!(rectangles(&page.svg, "#0011ff").len(), 1);
            let bytes = sdocx::render_svg_pages_pdf(&[page], &Default::default()).unwrap();
            let pdf = lopdf::Document::load_mem(&bytes).unwrap();
            assert_eq!(
                pdf.extract_text(&[1]).unwrap().replace(['\n', ' '], ""),
                "ABC"
            );
            assert!(
                !pdf.objects
                    .values()
                    .any(|object| object
                        .as_stream()
                        .is_ok_and(|stream| stream.dict.get(b"Subtype").is_ok_and(|value| value
                            .as_name()
                            .is_ok_and(|name| name == b"Image"))))
            );
            assert!(pdf.objects.values().any(|object| {
                object
                    .as_dict()
                    .is_ok_and(|dict| dict.has(b"FontFile2") || dict.has(b"FontFile3"))
            }));
        }
    }
}
