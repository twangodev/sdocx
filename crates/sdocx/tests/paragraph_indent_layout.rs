#![cfg(feature = "render")]

use sdocx::{
    BoundingBox, Document, DocumentMetadata, Page, PageElement, RichTextBox, RichTextParagraph,
    RichTextParagraphType,
};

fn document(alignment: u32, direction: u32, value: &str, width: f64) -> Document {
    let paragraph = |kind, payload| RichTextParagraph {
        kind,
        start_paragraph: 0,
        end_paragraph: 1,
        payload,
    };
    let content = RichTextBox {
        text_area_type: None,
        bbox: BoundingBox {
            x_min: 20.0,
            y_min: 20.0,
            x_max: 20.0 + width,
            y_max: 220.0,
        },
        rotation_degrees: None,
        text: value.into(),
        color: None,
        highlight_color: None,
        underline: false,
        font_size: Some(10.0),
        runs: Vec::new(),
        spans: Vec::new(),
        paragraphs: vec![
            paragraph(
                RichTextParagraphType::Alignment,
                alignment.to_le_bytes().to_vec(),
            ),
            paragraph(
                RichTextParagraphType::IndentLevel,
                [1_u32.to_le_bytes(), direction.to_le_bytes()].concat(),
            ),
        ],
        object_spans: Vec::new(),
        text_sections: Vec::new(),
        margins: None,
        gravity: None,
    };
    Document {
        pages: vec![Page {
            uuid: "indent".into(),
            width: 360,
            height: 300,
            content_bbox: BoundingBox::default(),
            background_color: None,
            template: None,
            background: Default::default(),
            objects: vec![PageElement::TextBox(content).into()],
        }],
        metadata: DocumentMetadata {
            default_page_dimensions: Some((360, 300)),
            orientation: Some(0),
            ..Default::default()
        },
    }
}

fn lines(doc: &Document) -> Vec<(String, f64)> {
    let page = sdocx::render_page_svg(doc, 0, &Default::default()).unwrap();
    assert!(page.text_diagnostics.is_empty());
    let xml = roxmltree::Document::parse(&page.svg).unwrap();
    xml.descendants()
        .filter(|node| node.has_tag_name("text"))
        .map(|node| {
            let source = node
                .descendants()
                .filter(|node| node.is_text())
                .filter_map(|node| node.text())
                .collect();
            (source, node.attribute("x").unwrap().parse().unwrap())
        })
        .collect()
}

#[test]
fn native_indent_masks_choose_physical_insets_independently_of_text_direction() {
    // Native paint entries for ABC at F10 total 19.26; indent level one is 16px.
    for (alignment, direction, expected_x) in [
        (0, 1, 36.0),
        (0, 2, 20.0),
        (1, 1, 200.74),
        (1, 2, 184.74),
        (2, 1, 118.37),
        (2, 2, 102.37),
        (3, 1, 20.0),
        (3, 2, 20.0),
        (4, 1, 36.0),
        (4, 2, 20.0),
        (0, 0, 36.0),
        (0, 3, 36.0),
    ] {
        assert_eq!(
            lines(&document(alignment, direction, "ABC", 200.0)),
            vec![("ABC".into(), expected_x)],
            "alignment={alignment}, indent direction={direction}"
        );
    }
}

#[test]
fn right_indent_reduces_wrap_width_without_reordering_latin_source() {
    for (direction, x) in [(1, 36.0), (2, 20.0)] {
        assert_eq!(
            lines(&document(4, direction, "AAAA", 30.0)),
            vec![("AA".into(), x), ("AA".into(), x)]
        );
    }
    assert_eq!(
        lines(&document(0, 2, "AAAA", 30.0)),
        vec![("AAAA".into(), 20.0)]
    );
}

#[test]
fn aligned_markers_and_text_share_the_native_offset() {
    let abc = f64::from(6.52_f32 + 6.23_f32 + 6.51_f32);
    for (alignment, direction, marker_left) in [
        (1, 2, 204.0 - 26.0 - abc),
        (2, 2, 20.0 + (184.0 - 26.0 - abc) / 2.0),
        (2, 1, 36.0 + (184.0 - 26.0 - abc) / 2.0),
    ] {
        let mut doc = document(alignment, direction, "ABC", 200.0);
        let sdocx::PageObjectContent::Element(PageElement::TextBox(content)) =
            &mut doc.pages[0].objects[0].content
        else {
            panic!()
        };
        content.paragraphs.push(RichTextParagraph {
            kind: RichTextParagraphType::Bullet,
            start_paragraph: 0,
            end_paragraph: 1,
            payload: [8_u32, 1, 0, 1]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect(),
        });
        let layout = sdocx::layout_document(&doc);
        for page in [
            sdocx::render_layout_page_svg(&doc, &layout, 0, &Default::default()).unwrap(),
            sdocx::render_layout_page_replay_svg(&doc, &layout, 0, &Default::default()).unwrap(),
        ] {
            assert!(page.text_diagnostics.is_empty());
            let xml = roxmltree::Document::parse(&page.svg).unwrap();
            let circle = xml
                .descendants()
                .find(|node| node.has_tag_name("circle"))
                .unwrap();
            let center = circle.attribute("cx").unwrap().parse::<f64>().unwrap();
            assert!((center - marker_left - 10.0).abs() < 0.00001);
            let text = xml
                .descendants()
                .find(|node| node.has_tag_name("text"))
                .unwrap();
            let left = text.attribute("x").unwrap().parse::<f64>().unwrap();
            assert!((left - marker_left - 26.0).abs() <= 0.00001);
        }
    }
}
