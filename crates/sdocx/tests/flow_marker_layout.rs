#![cfg(feature = "render")]

use sdocx::{
    BoundingBox, Document, DocumentMetadata, Page, PageElement, RichTextBox, RichTextParagraph,
    RichTextParagraphType,
};

fn document(default_width: u32, left: u32, marker: u32, indent: u32) -> Document {
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
            uuid: "flow-marker".into(),
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
            flow_page_padding: Some((left, 0)),
            ..Default::default()
        },
    }
}

fn assert_item_position(doc: &Document, expected: f64) {
    let layout = sdocx::layout_document(doc);
    for page in [
        sdocx::render_layout_page_svg(doc, &layout, 0, &Default::default()).unwrap(),
        sdocx::render_layout_page_replay_svg(doc, &layout, 0, &Default::default()).unwrap(),
    ] {
        let xml = roxmltree::Document::parse(&page.svg).unwrap();
        let spans = xml
            .descendants()
            .filter(|node| node.has_tag_name("tspan") && node.text() == Some("item"))
            .collect::<Vec<_>>();
        assert_eq!(spans.len(), 1);
        let item = spans[0];
        assert_eq!(item.text(), Some("item"));
        let x = item
            .attribute("x")
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .parse::<f64>()
            .unwrap();
        assert_eq!(x, expected);
        assert!(page.object_diagnostics.is_empty());
    }
}

#[test]
fn all_four_point_markers_reserve_the_native_density_scaled_button_and_gap() {
    // Native point marker constants: logical button20 and right gap6.
    for marker in [8, 9, 11, 12] {
        for (default_width, expected) in [(360, 36.0), (720, 62.0), (1080, 88.0)] {
            assert_item_position(&document(default_width, 10, marker, 0), expected);
        }
    }
}

#[test]
fn odd_and_even_indents_keep_the_same_point_marker_reservation() {
    for marker in [8, 9, 11, 12] {
        for (default_width, odd, even) in
            [(360, 52.0, 68.0), (720, 94.0, 126.0), (1080, 136.0, 184.0)]
        {
            assert_item_position(&document(default_width, 10, marker, 1), odd);
            assert_item_position(&document(default_width, 10, marker, 2), even);
        }
    }
}

#[test]
fn native_density_three_body_padding_places_point_list_text_at_126() {
    assert_item_position(&document(1080, 48, 8, 0), 126.0);
}
