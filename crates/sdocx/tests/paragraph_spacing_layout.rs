#![cfg(feature = "render")]

use sdocx::{
    BoundingBox, Document, DocumentMetadata, ObjectSpanLayoutConstraint, ObjectSpanLayoutOption,
    ObjectType, Page, PageElement, RichTextBox, RichTextCodeBlock, RichTextObjectContent,
    RichTextObjectSpan, RichTextParagraph, RichTextParagraphType,
};

#[derive(Clone, Copy, Debug)]
enum Context {
    Flow,
    Placed,
}

fn text(value: &str) -> RichTextBox {
    RichTextBox {
        text_area_type: None,
        bbox: BoundingBox::default(),
        rotation_degrees: None,
        text: value.into(),
        color: None,
        highlight_color: None,
        underline: false,
        font_size: Some(15.0),
        runs: Vec::new(),
        spans: Vec::new(),
        paragraphs: Vec::new(),
        object_spans: Vec::new(),
        text_sections: Vec::new(),
        margins: None,
        gravity: None,
    }
}

fn paragraph(kind: RichTextParagraphType, ordinal: u32, payload: Vec<u8>) -> RichTextParagraph {
    RichTextParagraph {
        kind,
        start_paragraph: ordinal,
        end_paragraph: ordinal + 1,
        payload,
    }
}

fn spacing(content: &mut RichTextBox, kind: RichTextParagraphType, ordinal: u32, gap: f32) {
    content
        .paragraphs
        .push(paragraph(kind, ordinal, gap.to_le_bytes().to_vec()));
}

fn bullet(content: &mut RichTextBox, ordinal: u32, raw: u32) {
    content.paragraphs.push(paragraph(
        RichTextParagraphType::Bullet,
        ordinal,
        [raw, 1, 0, 1]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect(),
    ));
}

fn block_object(content: &mut RichTextBox) {
    let anchor = content
        .text
        .chars()
        .position(|character| character == '\u{fffc}')
        .unwrap();
    content.object_spans.push(RichTextObjectSpan {
        object_type: ObjectType::CodeBlock,
        object_data: Vec::new(),
        content: Some(RichTextObjectContent::CodeBlock(Box::new(
            RichTextCodeBlock {
                bbox: BoundingBox {
                    x_min: 0.0,
                    y_min: 0.0,
                    x_max: 300.0,
                    y_max: 192.0,
                },
                rotation_degrees: None,
                title: None,
                body: None,
            },
        ))),
        text_index_utf16: anchor as i32,
        layout_option: ObjectSpanLayoutOption::BlockWithMediumMargin,
        layout_constraint: ObjectSpanLayoutConstraint::Normal,
    });
}

fn render(context: Context, content: RichTextBox) -> String {
    render_with_page_padding(context, content, 0)
}

fn render_with_page_padding(context: Context, mut content: RichTextBox, vertical: u32) -> String {
    if matches!(context, Context::Placed) {
        content.bbox = BoundingBox {
            x_min: 0.0,
            y_min: 0.0,
            x_max: 1000.0,
            y_max: 2000.0,
        };
    }
    let document = Document {
        pages: vec![Page {
            uuid: "paragraph-spacing".into(),
            width: 1080,
            height: 2000,
            content_bbox: BoundingBox::default(),
            background_color: None,
            template: None,
            background: Default::default(),
            objects: vec![PageElement::TextBox(content).into()],
        }],
        metadata: DocumentMetadata {
            default_page_dimensions: Some((1080, 2000)),
            orientation: Some(0),
            flow_page_padding: Some((48, vertical)),
            ..Default::default()
        },
    };
    let result = sdocx::render_page_svg(&document, 0, &Default::default()).unwrap();
    assert!(
        result.object_diagnostics.is_empty(),
        "{context:?}: {:?}",
        result.object_diagnostics
    );
    result.svg
}

fn glyph_y(xml: &roxmltree::Document<'_>, value: &str) -> f64 {
    let node = xml
        .descendants()
        .find(|node| node.has_tag_name("tspan") && node.text() == Some(value))
        .unwrap_or_else(|| panic!("missing glyph {value}"));
    let coordinate = |attribute| {
        node.ancestors()
            .find_map(|node| node.attribute(attribute))
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .parse::<f64>()
            .unwrap()
    };
    let mut position = (coordinate("x"), coordinate("y"));
    for ancestor in node.ancestors() {
        if let Some(value) = ancestor.attribute("transform") {
            let transform: svgtypes::Transform = value.parse().unwrap();
            position = (
                transform.a * position.0 + transform.c * position.1 + transform.e,
                transform.b * position.0 + transform.d * position.1 + transform.f,
            );
        }
    }
    position.1
}

fn positions(context: Context, content: RichTextBox) -> [f64; 2] {
    let svg = render(context, content);
    let xml = roxmltree::Document::parse(&svg).unwrap();
    [glyph_y(&xml, "A"), glyph_y(&xml, "B")]
}

#[test]
fn ordinary_before_and_after_gaps_add_twelve_density_scaled_units() {
    for context in [Context::Flow, Context::Placed] {
        for (kind, ordinal) in [
            (RichTextParagraphType::SpacingBefore, 1),
            (RichTextParagraphType::SpacingAfter, 0),
        ] {
            let mut zero = text("A\nB");
            spacing(&mut zero, kind, ordinal, 0.0);
            assert_eq!(positions(context, zero), [45.0, 105.75]);
            let mut enabled = text("A\nB");
            spacing(&mut enabled, kind, ordinal, 4.0);
            assert_eq!(
                positions(context, enabled),
                [45.0, 117.75],
                "{context:?} {kind:?}"
            );
        }
        let mut initial = text("A\nB");
        spacing(&mut initial, RichTextParagraphType::SpacingBefore, 0, 4.0);
        assert_eq!(positions(context, initial), [57.0, 117.75]);
    }
}

#[test]
fn initial_body_spacing_uses_text_margins_without_adding_page_padding() {
    let mut content = text("A\nB");
    content.margins = Some([0.0, 2.0, 0.0, 0.0]);
    spacing(&mut content, RichTextParagraphType::SpacingBefore, 0, 4.0);
    let svg = render_with_page_padding(Context::Flow, content, 900);
    let xml = roxmltree::Document::parse(&svg).unwrap();
    assert_eq!([glyph_y(&xml, "A"), glyph_y(&xml, "B")], [63.0, 123.75]);
}

#[test]
fn every_known_bullet_kind_suppresses_gaps_between_adjacent_list_items() {
    for context in [Context::Flow, Context::Placed] {
        for raw in 1..=12 {
            for (kind, ordinal) in [
                (RichTextParagraphType::SpacingBefore, 1),
                (RichTextParagraphType::SpacingAfter, 0),
            ] {
                for gap in [0.0, 4.0] {
                    let mut content = text("A\nB");
                    bullet(&mut content, 0, 1);
                    bullet(&mut content, 1, raw);
                    spacing(&mut content, kind, ordinal, gap);
                    assert_eq!(
                        positions(context, content),
                        [45.0, 105.75],
                        "{context:?}, bullet {raw}, {kind:?}, gap {gap}"
                    );
                }
            }
        }
    }
}

#[test]
fn missing_none_and_unknown_bullets_do_not_suppress_paragraph_gaps() {
    for context in [Context::Flow, Context::Placed] {
        for raw in [None, Some(0), Some(13), Some(99)] {
            for (kind, ordinal) in [
                (RichTextParagraphType::SpacingBefore, 1),
                (RichTextParagraphType::SpacingAfter, 0),
            ] {
                for (gap, second_y) in [(0.0, 105.75), (4.0, 117.75)] {
                    let mut content = text("A\nB");
                    bullet(&mut content, 0, 1);
                    if let Some(raw) = raw {
                        bullet(&mut content, 1, raw);
                    }
                    spacing(&mut content, kind, ordinal, gap);
                    assert_eq!(
                        positions(context, content),
                        [45.0, second_y],
                        "{context:?}, bullet {raw:?}, {kind:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn only_object_margins_at_the_respective_paragraph_edge_suppress_gaps() {
    for context in [Context::Flow, Context::Placed] {
        for (source, before_suppressed, after_suppressed) in [
            ("\u{fffc}A\nB", true, false),
            ("A\u{fffc}\nB", false, true),
            ("A\u{fffc}X\nB", false, false),
        ] {
            let mut content = text(source);
            block_object(&mut content);
            let zero = positions(context, content.clone());
            for (kind, suppressed) in [
                (RichTextParagraphType::SpacingBefore, before_suppressed),
                (RichTextParagraphType::SpacingAfter, after_suppressed),
            ] {
                let mut enabled = content.clone();
                spacing(&mut enabled, kind, 0, 4.0);
                let delta = if suppressed { 0.0 } else { 12.0 };
                let first_delta = if kind == RichTextParagraphType::SpacingBefore {
                    delta
                } else {
                    0.0
                };
                assert_eq!(
                    positions(context, enabled),
                    [zero[0] + first_delta, zero[1] + delta],
                    "{context:?}, {source:?}, {kind:?}"
                );
            }
        }
    }
}

#[test]
fn an_intervening_empty_paragraph_breaks_list_adjacency() {
    for context in [Context::Flow, Context::Placed] {
        for (kind, ordinal) in [
            (RichTextParagraphType::SpacingBefore, 2),
            (RichTextParagraphType::SpacingAfter, 0),
        ] {
            for (gap, second_y) in [(0.0, 166.5), (4.0, 178.5)] {
                let mut content = text("A\n\nB");
                bullet(&mut content, 0, 7);
                bullet(&mut content, 2, 11);
                spacing(&mut content, kind, ordinal, gap);
                assert_eq!(
                    positions(context, content),
                    [45.0, second_y],
                    "{context:?}, {kind:?}, gap {gap}"
                );
            }
        }
    }
}
