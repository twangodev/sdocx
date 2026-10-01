#![cfg(all(feature = "render", feature = "serde"))]

use sdocx::{
    BoundingBox, Document, DocumentMetadata, ObjectSpanLayoutConstraint, ObjectSpanLayoutOption,
    ObjectType, Page, RichTextBox, RichTextObjectContent, RichTextObjectSpan, RichTextParagraph,
    RichTextParagraphType, RichTextSection, RichTextTable, RichTextTableCell, RichTextTableRow,
};

fn text(source: &str) -> RichTextBox {
    RichTextBox {
        text_area_type: None,
        bbox: BoundingBox::default(),
        rotation_degrees: None,
        text: source.into(),
        color: None,
        highlight_color: None,
        underline: false,
        font_size: Some(10.0),
        runs: vec![],
        spans: vec![],
        paragraphs: vec![],
        object_spans: vec![],
        text_sections: vec![],
        margins: None,
        gravity: None,
    }
}

fn document(constraint: ObjectSpanLayoutConstraint, percent: f32) -> Document {
    let bounds = BoundingBox {
        x_min: 0.0,
        y_min: 0.0,
        x_max: 200.0,
        y_max: 100.0,
    };
    let mut empty = text("");
    empty.font_size = Some(100.0);
    empty.margins = Some([0.0, 2.0, 0.0, 3.0]);
    empty.paragraphs.push(RichTextParagraph {
        kind: RichTextParagraphType::LineSpacing,
        start_paragraph: 0,
        end_paragraph: 1,
        payload: [1_u32.to_le_bytes(), percent.to_le_bytes()].concat(),
    });
    let table = RichTextTable {
        style: serde_json::from_str(
            r#"{"heading_column_enabled":false,"heading_row_enabled":false,
                "max_height_enabled":false,"metadata":{"property_mask":[],
                "field_mask":[],"fixed_trailing_data":[],"flexible_trailing_data":[]}}"#,
        )
        .unwrap(),
        bbox: bounds,
        rotation_degrees: None,
        column_widths: vec![200.0],
        rows: vec![RichTextTableRow {
            max_height: None,
            min_height: None,
            metadata: Default::default(),
            index: 0,
            height: 100.0,
            cells: vec![RichTextTableCell {
                border: None,
                metadata: Default::default(),
                column_index: 0,
                row_span: 1,
                column_span: 1,
                background_color: 0,
                has_own_background_color: false,
                bbox: bounds,
                editable: false,
                content: empty,
            }],
        }],
    };
    let mut body = text("\u{fffc}\nEnd");
    body.text_sections.push(RichTextSection {
        start_utf16: 0,
        length_utf16: 5,
    });
    body.object_spans.push(RichTextObjectSpan {
        object_type: ObjectType::Table,
        object_data: vec![],
        content: Some(RichTextObjectContent::Table(Box::new(table))),
        text_index_utf16: 0,
        layout_option: ObjectSpanLayoutOption::Inline,
        layout_constraint: constraint,
    });
    Document {
        pages: vec![Page {
            uuid: "empty-table".into(),
            width: 360,
            height: 500,
            content_bbox: BoundingBox::default(),
            background_color: None,
            template: None,
            background: Default::default(),
            objects: vec![],
        }],
        metadata: DocumentMetadata {
            note_text: Some(body),
            page_mode: Some(0),
            default_page_dimensions: Some((360, 500)),
            orientation: Some(0),
            flow_page_padding: Some((0, 0)),
            ..Default::default()
        },
    }
}

#[test]
fn empty_table_cells_reserve_default_cursor_height_without_emitting_text() {
    for constraint in [
        ObjectSpanLayoutConstraint::OverPages,
        ObjectSpanLayoutConstraint::OverPagesOverlapPadding,
    ] {
        for (percent, baseline) in [(1.0, 129.501), (1.6, 189.501)] {
            let doc = document(constraint, percent);
            let layout = sdocx::layout_document(&doc);
            for replay in [false, true] {
                let page = if replay {
                    sdocx::render_layout_page_replay_svg(&doc, &layout, 0, &Default::default())
                } else {
                    sdocx::render_layout_page_svg(&doc, &layout, 0, &Default::default())
                }
                .unwrap();
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
                let spans = xml
                    .descendants()
                    .filter(|node| node.has_tag_name("tspan"))
                    .collect::<Vec<_>>();
                assert_eq!(spans.len(), 1);
                assert_eq!(spans[0].text(), Some("End"));
                let y = spans[0]
                    .attribute("y")
                    .unwrap()
                    .split_whitespace()
                    .next()
                    .unwrap();
                assert!(
                    (y.parse::<f64>().unwrap() - baseline).abs() < 0.00001,
                    "{y}"
                );
                assert!(!xml.descendants().any(|node| node.has_tag_name("image")));
            }
        }
    }
}
