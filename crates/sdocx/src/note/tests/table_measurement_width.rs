use super::*;
use crate::{
    BoundingBox, Document, DocumentMetadata, ObjectSpanLayoutOption, Page, PageElement,
    RichTextObjectContent, RichTextObjectSpan,
};

#[path = "../../../tests/support/binary_records.rs"]
mod binary_records;

fn text_object(value: &str, margins: [f32; 4]) -> Vec<u8> {
    let units = value.encode_utf16().collect::<Vec<_>>();
    let mut common = (units.len() as u32).to_le_bytes().to_vec();
    for unit in units {
        common.extend(unit.to_le_bytes());
    }
    common.extend(1_u32.to_le_bytes());
    common.extend(20_u16.to_le_bytes());
    for field in [3_u32, 0, value.encode_utf16().count() as u32, 0] {
        common.extend(field.to_le_bytes());
    }
    common.extend(17.0_f32.to_le_bytes());
    common.extend(1_u32.to_le_bytes());
    common.extend(16_u16.to_le_bytes());
    for field in [3_u32, 0, 1, 2] {
        common.extend(field.to_le_bytes());
    }
    for margin in margins {
        common.extend(margin.to_le_bytes());
    }
    common.push(0);
    common.extend(0_u16.to_le_bytes());
    common.extend([0; 8]);
    binary_records::text_object(&object_base_frame(), true, &binary_records::sized(&common))
}

fn parsed_table(value: &str, column_width: f32, margins: [f32; 4]) -> crate::RichTextTable {
    let text = text_object(value, margins);
    let bytes = binary_records::single_cell_table(&object_base_frame(), &text, column_width, 100.0);
    parse_table_object(
        &bytes,
        &ParseLimits::default(),
        "measurement-width table",
        0,
    )
    .unwrap()
}

fn document(table: crate::RichTextTable) -> Document {
    let mut parent = table.rows[0].cells[0].content.clone();
    parent.text = "\u{fffc}".into();
    parent.spans.clear();
    parent.paragraphs.clear();
    parent.margins = None;
    parent.gravity = None;
    parent.object_spans = vec![RichTextObjectSpan {
        object_type: ObjectType::Table,
        object_data: Vec::new(),
        content: Some(RichTextObjectContent::Table(Box::new(table))),
        text_index_utf16: 0,
        layout_option: ObjectSpanLayoutOption::Inline,
        layout_constraint: ObjectSpanLayoutConstraint::OverPages,
    }];
    let bounds = BoundingBox {
        x_min: 0.0,
        y_min: 0.0,
        x_max: 360.0,
        y_max: 400.0,
    };
    Document {
        pages: vec![Page {
            uuid: "measurement-width".into(),
            width: 360,
            height: 400,
            content_bbox: bounds,
            background_color: None,
            template: None,
            background: Default::default(),
            objects: vec![PageElement::TextBox(parent).into()],
        }],
        metadata: DocumentMetadata {
            default_page_dimensions: Some((360, 400)),
            flow_page_padding: Some((0, 0)),
            ..Default::default()
        },
    }
}

#[test]
fn parsed_subpixel_table_width_resolves_native_automatic_measurement_for_vector_exports() {
    for (value, width, margins, expected_lines) in [
        ("AV", 0.75, [0.0; 4], 1),
        ("AV", 1.0, [0.0; 4], 2),
        ("AV abc", 80.9, [0.0; 4], 1),
        ("AV\nToTo", 0.75, [0.0; 4], 2),
        ("AV abc", 0.75, [1.25, 2.5, 3.75, 4.5], 1),
    ] {
        let table = parsed_table(value, width, margins);
        assert_eq!(table.column_widths, [width]);
        assert_eq!(table.rows[0].cells[0].content.font_size, Some(17.0));
        let doc = document(table);
        let layout = crate::layout_document(&doc);
        let page = crate::render_layout_page_svg(&doc, &layout, 0, &Default::default()).unwrap();
        assert!(
            page.text_diagnostics.is_empty(),
            "{value:?} {width}: {:?}",
            page.text_diagnostics
        );
        let xml = roxmltree::Document::parse(&page.svg).unwrap();
        assert_eq!(
            xml.descendants()
                .filter(|node| node.has_tag_name("text"))
                .count(),
            expected_lines,
            "{value:?} {width}: {}",
            page.svg
        );
        assert!(!xml.descendants().any(|node| node.has_tag_name("image")));
        let retained = xml
            .descendants()
            .filter(|node| node.has_tag_name("tspan"))
            .filter_map(|node| node.text())
            .collect::<String>();
        assert_eq!(retained, value.replace('\n', ""));
        let replay =
            crate::render_layout_page_replay_svg(&doc, &layout, 0, &Default::default()).unwrap();
        assert_eq!(page.svg, replay.svg);
        #[cfg(feature = "pdf")]
        {
            let pdf =
                crate::render_document_pdf(&doc, &Default::default(), &Default::default()).unwrap();
            let pdf = lopdf::Document::load_mem(&pdf).unwrap();
            assert_eq!(
                pdf.extract_text(&[1])
                    .unwrap()
                    .split_whitespace()
                    .collect::<String>(),
                value.split_whitespace().collect::<String>()
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
        }
    }
}
