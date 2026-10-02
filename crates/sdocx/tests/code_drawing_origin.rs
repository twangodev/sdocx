#![cfg(all(feature = "render", feature = "serde"))]

use base64::Engine;
use sdocx::{
    BoundingBox, Document, DocumentMetadata, LayoutDocument, MediaAsset,
    ObjectSpanLayoutConstraint, ObjectSpanLayoutOption, ObjectType, Page, PlacedImage, RichTextBox,
    RichTextCodeBlock, RichTextObjectContent, RichTextObjectSpan, RichTextSection,
};

fn bounds(width: f64, height: f64) -> BoundingBox {
    BoundingBox {
        x_min: 0.0,
        y_min: 0.0,
        x_max: width,
        y_max: height,
    }
}

fn text(source: &str, font_size: f32) -> RichTextBox {
    RichTextBox {
        text_area_type: None,
        bbox: BoundingBox::default(),
        rotation_degrees: None,
        text: source.into(),
        color: None,
        highlight_color: None,
        underline: false,
        font_size: Some(font_size),
        runs: Vec::new(),
        spans: Vec::new(),
        paragraphs: Vec::new(),
        object_spans: Vec::new(),
        text_sections: Vec::new(),
        margins: None,
        gravity: None,
    }
}

fn document() -> Document {
    let mut body = text("\u{fffc}\n\u{fffc}\nEnd", 10.0);
    body.text_sections = vec![
        RichTextSection {
            start_utf16: 0,
            length_utf16: 7
        };
        2
    ];
    body.object_spans = vec![
        RichTextObjectSpan {
            object_type: ObjectType::Image,
            object_data: Vec::new(),
            content: Some(RichTextObjectContent::Image(Box::new(
                serde_json::from_value::<PlacedImage>(serde_json::json!({
                    "bbox": bounds(10.0, 10.0), "rotation_degrees": null,
                    "media_id": null, "media_index": 0, "crop_rect": null,
                    "original_bbox": null, "border_media_id": null, "original_media_id": null
                }))
                .unwrap(),
            ))),
            text_index_utf16: 0,
            layout_option: ObjectSpanLayoutOption::BlockWithSmallMargin,
            layout_constraint: ObjectSpanLayoutConstraint::Normal,
        },
        RichTextObjectSpan {
            object_type: ObjectType::CodeBlock,
            object_data: Vec::new(),
            content: Some(RichTextObjectContent::CodeBlock(Box::new(
                RichTextCodeBlock {
                    bbox: bounds(300.0, 100.0),
                    rotation_degrees: None,
                    title: Some(text("T", 1.0)),
                    body: Some(text("A\nB", 10.0)),
                },
            ))),
            text_index_utf16: 2,
            layout_option: ObjectSpanLayoutOption::Block,
            layout_constraint: ObjectSpanLayoutConstraint::OverPagesOverlapPadding,
        },
    ];
    Document {
        pages: (0..2).map(|index| Page {
            uuid: format!("code-origin-{index}"), width: 360, height: 100,
            content_bbox: bounds(360.0, 100.0), background_color: None, template: None,
            background: Default::default(), objects: Vec::new(),
        }).collect(),
        metadata: DocumentMetadata {
            note_text: Some(body), default_page_dimensions: Some((360, 100)), orientation: Some(0),
            page_mode: Some(0), flow_page_padding: Some((0, 10)),
            media_assets: vec![MediaAsset {
                name: "media/code-origin.png".into(), archive_id: None, mime_type: "image/png".into(),
                data: base64::engine::general_purpose::STANDARD.decode(
                    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC"
                ).unwrap(),
            }],
            ..Default::default()
        },
    }
}

fn render(
    doc: &Document,
    layout: &LayoutDocument,
    index: usize,
    replay: bool,
    fonts: &sdocx::fonts::FontBook,
) -> sdocx::RenderedPage {
    if replay {
        sdocx::render_layout_page_replay_svg_with_fonts(
            doc,
            layout,
            index,
            &Default::default(),
            fonts,
        )
        .unwrap()
    } else {
        sdocx::render_layout_page_svg_with_fonts(doc, layout, index, &Default::default(), fonts)
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

fn assert_page(page: &sdocx::RenderedPage, index: usize, expected: &[(&str, f64, f64)]) {
    assert_eq!(page.source_page_index, index);
    assert!(
        page.text_diagnostics.is_empty(),
        "{:?}",
        page.text_diagnostics
    );
    assert_eq!(
        page.object_diagnostics,
        [sdocx::ObjectDiagnostic {
            anchor_utf16: 2,
            kind: sdocx::ObjectDiagnosticKind::UnsupportedWidthLimitContext,
        }]
    );
    let xml = roxmltree::Document::parse(&page.svg).unwrap();
    let lines: Vec<_> = xml
        .descendants()
        .filter(|node| node.has_tag_name("text"))
        .collect();
    assert_eq!(lines.len(), expected.len());
    for (line, &(source, x, y)) in lines.into_iter().zip(expected) {
        let actual_source: String = line
            .descendants()
            .filter(|node| node.has_tag_name("tspan"))
            .filter_map(|node| node.text())
            .collect();
        assert_eq!(actual_source, source);
        let position = point(
            line.descendants()
                .find(|node| node.has_tag_name("tspan"))
                .unwrap(),
        );
        assert!((position.0 - x).abs() <= 1e-4, "{source}: {position:?}");
        assert!((position.1 - y).abs() <= 1e-4, "{source}: {position:?}");
    }
    let panel = xml
        .descendants()
        .find(|node| node.has_tag_name("rect") && node.attribute("fill") == Some("#efefef"))
        .unwrap();
    // Native chrome 64 plus fresh body height 40.498 rounds to 104.50.
    assert_eq!(
        panel.attribute("height").unwrap().parse::<f64>().unwrap(),
        104.5
    );
    assert!((point(panel).1 - (30.002 - index as f64 * 100.0)).abs() <= 1e-4);
    let images: Vec<_> = xml
        .descendants()
        .filter(|node| node.has_tag_name("image"))
        .collect();
    if index == 0 {
        assert_eq!(images.len(), 1);
        assert!((point(images[0]).1 - 10.001).abs() <= 1e-4);
    } else {
        assert!(images.is_empty());
    }
}

#[test]
fn code_drawing_origin_is_remeasured_without_replacing_the_callback_parent_height() {
    let doc = document();
    let layout = sdocx::layout_document(&doc);
    let fonts = sdocx::fonts::FontBook::default();
    // Copy-height minimum 57.5 fits the 59.999-unit gap. Callback body height
    // 27 reserves 91; drawing at 30.002 moves B past 100..101 and measures 104.498.
    for replay in [false, true] {
        let second = render(&doc, &layout, 1, replay, &fonts);
        assert_page(&second, 1, &[("B", 16.0, 11.0), ("End", 0.0, 34.502)]);
        let first = render(&doc, &layout, 0, replay, &fonts);
        assert_page(&first, 0, &[("T", 16.0, 43.002), ("A", 16.0, 84.002)]);
        assert_eq!(render(&doc, &layout, 1, replay, &fonts), second);
        assert_eq!(render(&doc, &layout, 0, replay, &fonts), first);
    }
}

#[cfg(feature = "pdf")]
#[test]
fn drawing_origin_pdf_keeps_only_visible_selectable_text_and_the_original_tiny_image() {
    let doc = document();
    let layout = sdocx::layout_document(&doc);
    let fonts = sdocx::fonts::FontBook::default();
    for replay in [false, true] {
        let pages = [
            render(&doc, &layout, 0, replay, &fonts),
            render(&doc, &layout, 1, replay, &fonts),
        ];
        assert_page(&pages[0], 0, &[("T", 16.0, 43.002), ("A", 16.0, 84.002)]);
        assert_page(&pages[1], 1, &[("B", 16.0, 11.0), ("End", 0.0, 34.502)]);
        let bytes = sdocx::render_svg_pages_pdf(&pages, &Default::default()).unwrap();
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        assert_eq!(
            pdf.extract_text(&[1]).unwrap().replace(['\n', ' '], ""),
            "TA"
        );
        assert_eq!(
            pdf.extract_text(&[2]).unwrap().replace(['\n', ' '], ""),
            "BEnd"
        );
        let images: Vec<_> = pdf
            .objects
            .values()
            .filter_map(|object| object.as_stream().ok())
            .filter(|stream| {
                stream
                    .dict
                    .get(b"Subtype")
                    .is_ok_and(|value| value.as_name().is_ok_and(|name| name == b"Image"))
            })
            .collect();
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].dict.get(b"Width").unwrap().as_i64().unwrap(), 1);
        assert_eq!(images[0].dict.get(b"Height").unwrap().as_i64().unwrap(), 1);
        assert!(pdf.objects.values().any(|object| {
            object
                .as_dict()
                .is_ok_and(|dict| dict.has(b"FontFile2") || dict.has(b"FontFile3"))
        }));
    }
}

#[test]
fn copy_height_reserves_the_header_when_the_title_is_tiny_or_absent() {
    let fonts = sdocx::fonts::FontBook::default();
    for title_size in [Some(1.0), Some(10.0), None] {
        let mut doc = document();
        doc.metadata.default_page_dimensions = Some((360, 80));
        for page in &mut doc.pages {
            page.height = 80;
            page.content_bbox = bounds(360.0, 80.0);
        }
        let body = doc.metadata.note_text.as_mut().unwrap();
        let Some(RichTextObjectContent::CodeBlock(code)) = body.object_spans[1].content.as_mut()
        else {
            panic!("expected code object");
        };
        if let Some(size) = title_size {
            code.title.as_mut().unwrap().font_size = Some(size);
        } else {
            code.title = None;
        }
        let layout = sdocx::layout_document(&doc);
        for replay in [false, true] {
            let first = render(&doc, &layout, 0, replay, &fonts);
            let first_xml = roxmltree::Document::parse(&first.svg).unwrap();
            assert!(
                first_xml
                    .descendants()
                    .all(|node| node.attribute("data-sdocx-object") != Some("code-block"))
            );
            let second = render(&doc, &layout, 1, replay, &fonts);
            let second_xml = roxmltree::Document::parse(&second.svg).unwrap();
            let panel = second_xml
                .descendants()
                .find(|node| node.has_tag_name("rect") && node.attribute("fill") == Some("#efefef"))
                .unwrap();
            assert!((point(panel).1 - 10.001).abs() <= 1e-4);
            assert_eq!(
                second_xml
                    .descendants()
                    .filter(|node| node.has_tag_name("image"))
                    .count(),
                0
            );
        }
    }
}
