#![cfg(all(feature = "render", feature = "serde"))]

use base64::Engine;
use sdocx::{
    BoundingBox, Document, DocumentMetadata, MediaAsset, ObjectSpanLayoutConstraint,
    ObjectSpanLayoutOption, ObjectType, Page, PageElement, PlacedImage, RenderedPage, RichTextBox,
    RichTextObjectContent, RichTextObjectSpan, RichTextParagraph, RichTextParagraphType,
    RichTextSpan, RichTextSpanType,
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
    option: ObjectSpanLayoutOption,
    spacing: Option<(u32, f32)>,
    anchor_font: f32,
) -> Document {
    let content = RichTextBox {
        text_area_type: None,
        bbox: BoundingBox::default(),
        rotation_degrees: None,
        text: source.into(),
        color: None,
        highlight_color: None,
        underline: false,
        font_size: Some(20.0),
        runs: Vec::new(),
        spans: vec![RichTextSpan {
            kind: RichTextSpanType::FontSize,
            start_utf16: anchor as u32,
            end_utf16: anchor as u32 + 1,
            expand: false,
            payload: anchor_font.to_le_bytes().to_vec(),
        }],
        paragraphs: spacing.map_or_else(Vec::new, |(kind, value)| {
            vec![RichTextParagraph {
                kind: RichTextParagraphType::LineSpacing,
                start_paragraph: 0,
                end_paragraph: 1,
                payload: [kind.to_le_bytes(), value.to_le_bytes()].concat(),
            }]
        }),
        object_spans: vec![RichTextObjectSpan {
            object_type: ObjectType::Image,
            object_data: Vec::new(),
            content: Some(RichTextObjectContent::Image(Box::new(
                serde_json::from_value::<PlacedImage>(serde_json::json!({
                    "bbox": bounds(30.0, 100.0), "rotation_degrees": null,
                    "media_id": null, "media_index": 0, "crop_rect": null,
                    "original_bbox": null, "border_media_id": null, "original_media_id": null,
                }))
                .unwrap(),
            ))),
            text_index_utf16: anchor,
            layout_option: option,
            layout_constraint: ObjectSpanLayoutConstraint::Normal,
        }],
        text_sections: Vec::new(),
        margins: None,
        gravity: None,
    };
    Document {
        pages: vec![Page { uuid: "object-font".into(), width: 360, height: 800,
            content_bbox: bounds(360.0, 800.0), background_color: None, template: None,
            background: Default::default(), objects: vec![PageElement::TextBox(content).into()],
        }],
        metadata: DocumentMetadata {
            default_page_dimensions: Some((360, 800)), flow_page_padding: Some((0, 0)),
            media_assets: vec![MediaAsset {
                name: "media/metrics.png".into(), archive_id: None, mime_type: "image/png".into(),
                data: base64::engine::general_purpose::STANDARD.decode(
                    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC"
                ).unwrap(),
            }], ..Default::default()
        },
    }
}

fn modes(document: &Document) -> [RenderedPage; 2] {
    let layout = sdocx::layout_document(document);
    [
        sdocx::render_layout_page_svg(document, &layout, 0, &Default::default()).unwrap(),
        sdocx::render_layout_page_replay_svg(document, &layout, 0, &Default::default()).unwrap(),
    ]
}

fn source(xml: &roxmltree::Document<'_>) -> String {
    xml.descendants()
        .filter(|node| node.has_tag_name("tspan"))
        .filter_map(|node| node.text())
        .collect()
}

fn number(node: roxmltree::Node<'_, '_>, name: &str) -> f64 {
    node.attribute(name)
        .or_else(|| node.parent().unwrap().attribute(name))
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap()
}

fn image_baseline(xml: &roxmltree::Document<'_>) -> f64 {
    let image = xml
        .descendants()
        .find(|node| node.has_tag_name("image"))
        .unwrap();
    let mut point = (
        number(image, "x"),
        number(image, "y") + number(image, "height"),
    );
    for ancestor in image.ancestors() {
        if let Some(transform) = ancestor.attribute("transform") {
            let transform: svgtypes::Transform = transform.parse().unwrap();
            point = (
                transform.a * point.0 + transform.c * point.1 + transform.e,
                transform.b * point.0 + transform.d * point.1 + transform.f,
            );
        }
    }
    point.1
}

fn baseline(xml: &roxmltree::Document<'_>, value: &str) -> f64 {
    number(
        xml.descendants()
            .find(|node| node.has_tag_name("tspan") && node.text() == Some(value))
            .unwrap(),
        "y",
    )
}

fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 0.0001, "{actual} != {expected}");
}

#[test]
fn source_first_object_uses_its_font_for_leading_and_preserves_its_own_height() {
    for option in [
        ObjectSpanLayoutOption::Inline,
        ObjectSpanLayoutOption::Block,
    ] {
        for (spacing, object_baseline, cursor) in [
            (None, 100.001, 107.001),
            (Some((1, 1.6)), 105.001, 112.001),
            (Some((0, 20.0)), 113.001, 120.001),
        ] {
            for page in modes(&document("\u{fffc}\nB", 0, option, spacing, 20.0)) {
                assert!(
                    page.text_diagnostics.is_empty(),
                    "{:?}",
                    page.text_diagnostics
                );
                assert!(page.object_diagnostics.is_empty());
                let xml = roxmltree::Document::parse(&page.svg).unwrap();
                assert_eq!(source(&xml), "B");
                close(image_baseline(&xml), object_baseline);
                close(baseline(&xml, "B"), cursor + 20.0);
            }
        }
    }
}

#[test]
fn an_anchor_font_changes_percentage_leading_without_inflating_the_object_height() {
    for page in modes(&document(
        "A\u{fffc}B\nC",
        1,
        ObjectSpanLayoutOption::Inline,
        None,
        900.0,
    )) {
        assert!(page.text_diagnostics.is_empty());
        assert!(page.object_diagnostics.is_empty());
        let xml = roxmltree::Document::parse(&page.svg).unwrap();
        assert_eq!(source(&xml), "ABC");
        close(image_baseline(&xml), 100.001);
        close(baseline(&xml, "A"), 100.001);
        close(baseline(&xml, "B"), 100.001);
        close(baseline(&xml, "C"), 435.001);
        assert!(
            xml.descendants()
                .filter(|node| node.has_tag_name("tspan"))
                .all(|node| node.attribute("font-size") == Some("20.00"))
        );
    }
}

#[test]
fn margin_bearing_objects_use_the_separate_native_baseline_branch() {
    for spacing in [None, Some((1, 1.6)), Some((0, 20.0))] {
        for page in modes(&document(
            "\u{fffc}\nB",
            0,
            ObjectSpanLayoutOption::BlockWithSmallMargin,
            spacing,
            20.0,
        )) {
            let xml = roxmltree::Document::parse(&page.svg).unwrap();
            close(image_baseline(&xml), 110.001);
            close(baseline(&xml, "B"), 140.001);
            assert!(page.text_diagnostics.is_empty());
            assert!(page.object_diagnostics.is_empty());
        }
    }
}

#[test]
fn a_later_object_paragraph_preserves_full_source_separator_context() {
    for page in modes(&document(
        "\n\u{fffc}\nB",
        1,
        ObjectSpanLayoutOption::Inline,
        None,
        20.0,
    )) {
        let xml = roxmltree::Document::parse(&page.svg).unwrap();
        assert_eq!(source(&xml), "B");
        close(image_baseline(&xml), 127.001);
        close(baseline(&xml, "B"), 154.001);
        assert!(page.text_diagnostics.is_empty());
        assert!(page.object_diagnostics.is_empty());
    }
}

#[cfg(feature = "pdf")]
#[test]
fn object_font_metrics_keep_neighboring_text_selectable_in_pdf() {
    let fonts = sdocx::fonts::FontBook::default();
    for page in modes(&document(
        "A\u{fffc}B\nC",
        1,
        ObjectSpanLayoutOption::Inline,
        None,
        900.0,
    )) {
        let bytes = sdocx::render_svg_pages_pdf(&[page], &sdocx::PdfOptions::new(fonts.database()))
            .unwrap();
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        assert_eq!(pdf.extract_text(&[1]).unwrap().replace('\n', ""), "ABC");
    }
}
