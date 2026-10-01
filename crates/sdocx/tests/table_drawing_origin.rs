#![cfg(all(feature = "render", feature = "serde"))]

use sdocx::{
    BoundingBox, Document, DocumentMetadata, ObjectSpanLayoutConstraint, ObjectSpanLayoutOption,
    ObjectType, Page, PageElement, RichTextBox, RichTextObjectContent, RichTextObjectSpan,
    RichTextTable, RichTextTableCell, RichTextTableRow,
};

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
        bbox: BoundingBox::default(),
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

fn document(left: f64, top: f64, constraint: ObjectSpanLayoutConstraint) -> Document {
    let mut row_metadata = sdocx::TableRecordMetadata::default();
    row_metadata.fixed_trailing_data = vec![41, 42];
    let mut cell_metadata = sdocx::TableRecordMetadata::default();
    cell_metadata.flexible_trailing_data = vec![51, 52];
    let table = RichTextTable {
        style: serde_json::from_str(
            r#"{
            "heading_column_enabled": false, "heading_row_enabled": false,
            "max_height_enabled": false,
            "metadata": {"property_mask": [], "field_mask": [],
                "fixed_trailing_data": [31], "flexible_trailing_data": [32]}
        }"#,
        )
        .unwrap(),
        bbox: bounds(5.25, 7.75, 100.0, 54.0),
        rotation_degrees: None,
        column_widths: vec![100.0],
        rows: vec![RichTextTableRow {
            max_height: None,
            min_height: None,
            metadata: row_metadata,
            index: 0,
            height: 54.0,
            cells: vec![RichTextTableCell {
                border: None,
                metadata: cell_metadata,
                column_index: 0,
                row_span: 1,
                column_span: 1,
                background_color: 0xffabcdef,
                has_own_background_color: true,
                bbox: bounds(700.0, 700.0, 100.0, 54.0),
                editable: false,
                content: text("A\nB"),
            }],
        }],
    };
    let mut parent = text("\u{fffc}\nEnd");
    parent.bbox = bounds(left, top, 300.0, 200.0);
    parent.object_spans.push(RichTextObjectSpan {
        object_type: ObjectType::Table,
        object_data: Vec::new(),
        content: Some(RichTextObjectContent::Table(Box::new(table))),
        text_index_utf16: 0,
        layout_option: ObjectSpanLayoutOption::Inline,
        layout_constraint: constraint,
    });
    Document {
        pages: vec![Page {
            uuid: "table-drawing-origin".into(),
            width: 360,
            height: 300,
            content_bbox: bounds(0.0, 0.0, 360.0, 300.0),
            background_color: None,
            template: None,
            background: Default::default(),
            objects: vec![PageElement::TextBox(parent).into()],
        }],
        metadata: DocumentMetadata {
            default_page_dimensions: Some((360, 300)),
            orientation: Some(0),
            ..Default::default()
        },
    }
}

fn render(doc: &Document, replay: bool) -> sdocx::RenderedPage {
    let layout = sdocx::layout_document(doc);
    if replay {
        sdocx::render_layout_page_replay_svg(doc, &layout, 0, &Default::default()).unwrap()
    } else {
        sdocx::render_layout_page_svg(doc, &layout, 0, &Default::default()).unwrap()
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

fn assert_page(
    page: &sdocx::RenderedPage,
    expected: &[(&str, f64, f64)],
    table: [f64; 4],
    cell: [f64; 4],
) {
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
    assert!(!xml.descendants().any(|node| node.has_tag_name("image")));
    let lines: Vec<_> = xml
        .descendants()
        .filter(|node| node.has_tag_name("text"))
        .collect();
    assert_eq!(lines.len(), expected.len());
    for (line, &(source, x, y)) in lines.into_iter().zip(expected) {
        let actual: String = line
            .descendants()
            .filter(|node| node.has_tag_name("tspan"))
            .filter_map(|node| node.text())
            .collect();
        assert_eq!(actual, source);
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
        .find(|node| {
            node.has_tag_name("rect")
                && node
                    .parent()
                    .is_some_and(|parent| parent.has_tag_name("clipPath"))
                && node
                    .ancestors()
                    .any(|ancestor| ancestor.attribute("data-sdocx-object") == Some("table"))
        })
        .unwrap();
    let background = xml
        .descendants()
        .find(|node| node.has_tag_name("rect") && node.attribute("fill") == Some("#abcdef"))
        .unwrap();
    for (node, expected) in [(panel, table), (background, cell)] {
        let (x, y) = point(node);
        let width: f64 = node.attribute("width").unwrap().parse().unwrap();
        let height: f64 = node.attribute("height").unwrap().parse().unwrap();
        assert_eq!([x, y, x + width, y + height], expected);
    }
}

#[test]
fn fresh_table_drawing_rounds_world_bounds_without_changing_parent_reservation() {
    // Native Composer 0x37e4dc–0x37e4f4 rounds world table endpoints, then
    // 0x37ee14–0x37ee94 rounds each cached cell against that rounded origin.
    // Cold row 54 shrinks on fresh warm layout to two F10 lines: 27, plus border 1.
    // The parent retains its separate 55px callback/saved reservation. Native
    // SpanRunFunctor 0x77674 and GetBlockInfo 0x6aef8 retain the anchor's F10:
    // End follows height 55 + leading 3.5 + object epsilon .001 + next font 10.
    for constraint in [
        ObjectSpanLayoutConstraint::Normal,
        ObjectSpanLayoutConstraint::OverPagesOverlapPadding,
        ObjectSpanLayoutConstraint::OverPages,
    ] {
        for (left, top, expected, table, cell) in [
            (
                -0.25,
                -1.751,
                [("A", -1.0, 8.0), ("B", -1.0, 21.5), ("End", -0.25, 66.75)],
                [-1.0, -2.0, 101.0, 27.0],
                [-1.0, -2.0, 100.0, 26.0],
            ),
            (
                20.25,
                20.249,
                [("A", 20.0, 30.0), ("B", 20.0, 43.5), ("End", 20.25, 88.75)],
                [20.0, 20.0, 122.0, 49.0],
                [20.0, 20.0, 121.0, 48.0],
            ),
        ] {
            let doc = document(left, top, constraint);
            let source = serde_json::to_value(&doc).unwrap();
            for replay in [false, true] {
                let page = render(&doc, replay);
                assert_page(&page, &expected, table, cell);
                assert_eq!(render(&doc, replay), page);
            }
            assert_eq!(serde_json::to_value(&doc).unwrap(), source);
        }
    }
}

#[cfg(feature = "pdf")]
#[test]
fn rounded_table_pdf_keeps_selectable_text_and_embedded_vector_fonts() {
    for replay in [false, true] {
        let page = render(
            &document(20.25, 20.249, ObjectSpanLayoutConstraint::OverPages),
            replay,
        );
        assert_page(
            &page,
            &[("A", 20.0, 30.0), ("B", 20.0, 43.5), ("End", 20.25, 88.75)],
            [20.0, 20.0, 122.0, 49.0],
            [20.0, 20.0, 121.0, 48.0],
        );
        let bytes = sdocx::render_svg_pages_pdf(&[page], &Default::default()).unwrap();
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        assert_eq!(
            pdf.extract_text(&[1]).unwrap().replace(['\n', ' '], ""),
            "ABEnd"
        );
        assert!(
            !pdf.objects
                .values()
                .any(|object| object.as_stream().is_ok_and(|stream| stream
                    .dict
                    .get(b"Subtype")
                    .is_ok_and(|value| value.as_name().is_ok_and(|name| name == b"Image"))))
        );
        assert!(pdf.objects.values().any(|object| {
            object
                .as_dict()
                .is_ok_and(|dict| dict.has(b"FontFile2") || dict.has(b"FontFile3"))
        }));
    }
}
