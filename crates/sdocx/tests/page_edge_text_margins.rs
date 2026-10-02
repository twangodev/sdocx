#![cfg(all(feature = "render", feature = "serde"))]

use base64::Engine;
use sdocx::{
    BoundingBox, Document, DocumentMetadata, LayoutDocument, MediaAsset,
    ObjectSpanLayoutConstraint, ObjectSpanLayoutOption, ObjectType, Page, PlacedImage, RichTextBox,
    RichTextObjectContent, RichTextObjectSpan, RichTextParagraph, RichTextParagraphType,
    RichTextSection,
};

fn bounds(width: f64, height: f64) -> BoundingBox {
    BoundingBox {
        x_min: 0.0,
        y_min: 0.0,
        x_max: width,
        y_max: height,
    }
}

fn document(
    source: &str,
    anchor: i32,
    top: f32,
    width: u32,
    constraint: ObjectSpanLayoutConstraint,
    before: Option<f32>,
) -> Document {
    let length_utf16 = i32::try_from(source.encode_utf16().count()).unwrap();
    Document {
        pages: (0..2).map(|index| Page {
            uuid: format!("edge-margin-{index}"), width, height: 80,
            content_bbox: bounds(f64::from(width), 80.0),
            background_color: None, template: None, background: Default::default(), objects: Vec::new(),
        }).collect(),
        metadata: DocumentMetadata {
            default_page_dimensions: Some((360, 80)), orientation: Some(0), page_mode: Some(0),
            flow_page_padding: Some((0, 0)),
            media_assets: vec![MediaAsset {
                name: "media/native-margin.png".into(), archive_id: None, mime_type: "image/png".into(),
                data: base64::engine::general_purpose::STANDARD.decode(
                    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC"
                ).unwrap(),
            }],
            note_text: Some(RichTextBox {
                text_area_type: None, bbox: BoundingBox::default(), rotation_degrees: None,
                text: source.into(), color: None, highlight_color: None, underline: false,
                font_size: Some(10.0), runs: Vec::new(), spans: Vec::new(),
                paragraphs: before.map_or_else(Vec::new, |value| vec![RichTextParagraph {
                    kind: RichTextParagraphType::SpacingBefore, start_paragraph: 0, end_paragraph: 1,
                    payload: value.to_le_bytes().to_vec(),
                }]),
                object_spans: vec![RichTextObjectSpan {
                    object_type: ObjectType::Image, object_data: Vec::new(),
                    content: Some(RichTextObjectContent::Image(Box::new(
                        serde_json::from_value::<PlacedImage>(serde_json::json!({
                            "bbox": bounds(10.0, 10.0), "rotation_degrees": null,
                            "media_id": null, "media_index": 0, "crop_rect": null,
                            "original_bbox": null, "border_media_id": null, "original_media_id": null
                        })).unwrap()
                    ))),
                    text_index_utf16: anchor,
                    layout_option: ObjectSpanLayoutOption::BlockWithSmallMargin,
                    layout_constraint: constraint,
                }],
                text_sections: vec![RichTextSection { start_utf16: 0, length_utf16 }; 2],
                margins: Some([0.0, top, 0.0, 0.0]), gravity: None,
            }),
            ..Default::default()
        },
    }
}

fn render(
    doc: &Document,
    layout: &LayoutDocument,
    page: usize,
    replay: bool,
    fonts: &sdocx::fonts::FontBook,
) -> sdocx::RenderedPage {
    if replay {
        sdocx::render_layout_page_replay_svg_with_fonts(
            doc,
            layout,
            page,
            &Default::default(),
            fonts,
        )
        .unwrap()
    } else {
        sdocx::render_layout_page_svg_with_fonts(doc, layout, page, &Default::default(), fonts)
            .unwrap()
    }
}

fn point(node: roxmltree::Node<'_, '_>) -> (f64, f64) {
    let coordinate = |attribute| {
        node.attribute(attribute)
            .or_else(|| node.parent().unwrap().attribute(attribute))
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .parse::<f64>()
            .unwrap()
    };
    let mut point = (coordinate("x"), coordinate("y"));
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

fn assert_geometry(page: &sdocx::RenderedPage, lines: &[(&str, f64)], image_y: f64) {
    assert_eq!(page.source_page_index, 1);
    assert!(
        page.text_diagnostics.is_empty(),
        "{:?}",
        page.text_diagnostics
    );
    assert!(
        page.object_diagnostics.is_empty(),
        "{:?}",
        page.object_diagnostics
    );
    let xml = roxmltree::Document::parse(&page.svg).unwrap();
    let text: Vec<_> = xml
        .descendants()
        .filter(|node| node.has_tag_name("text"))
        .collect();
    assert_eq!(text.len(), lines.len());
    for (node, &(expected, y)) in text.into_iter().zip(lines) {
        let source: String = node
            .descendants()
            .filter(|child| child.has_tag_name("tspan"))
            .filter_map(|child| child.text())
            .collect();
        assert_eq!(source, expected);
        let actual = point(
            node.descendants()
                .find(|child| child.has_tag_name("tspan"))
                .unwrap(),
        );
        assert_eq!(actual.0, 0.0);
        assert!(
            (actual.1 - y).abs() < 1e-8,
            "{expected}: {actual:?}, expected y {y}"
        );
    }
    let images: Vec<_> = xml
        .descendants()
        .filter(|node| node.has_tag_name("image"))
        .collect();
    assert_eq!(images.len(), 1);
    let actual = point(images[0]);
    assert_eq!(actual.0, 0.0);
    assert!(
        (actual.1 - image_y).abs() < 1e-8,
        "image {actual:?}, expected y {image_y}"
    );
}

fn assert_modes(doc: &Document, lines: &[(&str, f64)], image_y: f64) {
    let layout = sdocx::layout_document(doc);
    assert_eq!(
        layout.pages[1]
            .body_text_slice()
            .unwrap()
            .capture_window
            .as_ref()
            .unwrap()
            .first_page_index,
        0
    );
    assert_eq!(
        layout.pages[1].body_text_capture(doc).unwrap().text,
        doc.metadata.note_text.as_ref().unwrap().text
    );
    let fonts = sdocx::fonts::FontBook::default();
    for replay in [false, true] {
        let page = render(doc, &layout, 1, replay, &fonts);
        assert_geometry(&page, lines, image_y);
        let first = render(doc, &layout, 0, replay, &fonts);
        let xml = roxmltree::Document::parse(&first.svg).unwrap();
        assert!(
            !xml.descendants()
                .any(|node| node.has_tag_name("tspan") || node.has_tag_name("image"))
        );
        assert_eq!(render(doc, &layout, 1, replay, &fonts), page);
        #[cfg(feature = "pdf")]
        {
            let bytes = sdocx::render_svg_pages_pdf(&[page], &Default::default()).unwrap();
            let pdf = lopdf::Document::load_mem(&bytes).unwrap();
            let expected: String = lines.iter().map(|line| line.0).collect();
            assert_eq!(
                pdf.extract_text(&[1]).unwrap().replace(['\n', ' '], ""),
                expected
            );
        }
    }
}

#[test]
fn flagged_band_end_reduces_block_margin_but_touching_the_outside_does_not() {
    // AdjustedBlockTopMargin probes [Q-1,Q]. The 70..90 band reduces the
    // native Small margin 10 to 0 at Q90; Q91 only touches its outside edge.
    for (top, image_y, after_y) in [(90.0, 10.001, 40.001), (91.0, 21.001, 51.00101)] {
        let doc = document(
            "\u{fffc}\nAfter",
            0,
            top,
            360,
            ObjectSpanLayoutConstraint::Normal,
            None,
        );
        assert_modes(&doc, &[("After", after_y)], image_y);
    }
}

#[test]
fn flagged_margin_probe_suppresses_previous_bottom_for_following_ordinary_text() {
    // Over-page min 0 lets the image occupy 80..90. Its outgoing 90.001 probe
    // overlaps the band, so ordinary top 0 ignores the pending bottom 10.
    let doc = document(
        "\u{fffc}\nAfter",
        0,
        70.0,
        360,
        ObjectSpanLayoutConstraint::OverPages,
        None,
    );
    assert_modes(&doc, &[("After", 20.001)], 0.001);
}

#[test]
fn paragraph_before_stays_enabled_for_margin_probes_on_wrapped_lines() {
    // Pinned F10 Roboto four-A width 26.09375 fits 30. Before 13.5 puts that
    // line at 90; the next raw 103.5 probe subtracts 13.5 and hits the band.
    // It omits margin 10; the final ordinary line later consumes bottom 10.
    let doc = document(
        "AAAA\u{fffc}B",
        4,
        76.5,
        30,
        ObjectSpanLayoutConstraint::Normal,
        Some(13.5),
    );
    assert_modes(&doc, &[("AAAA", 20.0), ("B", 53.50101)], 23.501);
}
