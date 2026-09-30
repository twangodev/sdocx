#![cfg(all(feature = "render", feature = "serde"))]

use sdocx::{
    BoundingBox, Document, DocumentMetadata, ObjectSpanLayoutConstraint, ObjectSpanLayoutOption,
    ObjectType, Page, PageElement, RichTextBox, RichTextObjectContent, RichTextObjectSpan,
    RichTextParagraph, RichTextParagraphType, RichTextTable, RichTextTableCell, RichTextTableRow,
};

fn bounds(left: f64, top: f64, width: f64, height: f64) -> BoundingBox {
    BoundingBox {
        x_min: left,
        y_min: top,
        x_max: left + width,
        y_max: top + height,
    }
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
        paragraphs: vec![
            RichTextParagraph {
                kind: RichTextParagraphType::SpacingBefore,
                start_paragraph: 0,
                end_paragraph: 1,
                payload: 4.0_f32.to_le_bytes().to_vec(),
            },
            RichTextParagraph {
                kind: RichTextParagraphType::LineSpacing,
                start_paragraph: 0,
                end_paragraph: 2,
                payload: [1_u32.to_le_bytes(), 1.6_f32.to_le_bytes()].concat(),
            },
        ],
        object_spans: Vec::new(),
        text_sections: Vec::new(),
        margins: Some([4.0; 4]),
        gravity: Some(2),
    }
}

fn table(values: &[&[&str]]) -> RichTextTable {
    let width = 492.0 * values[0].len() as f64;
    RichTextTable {
        style: serde_json::from_str(
            r#"{
                "heading_column_enabled": false, "heading_row_enabled": false,
                "max_height_enabled": false,
                "metadata": {"property_mask": [], "field_mask": [],
                    "fixed_trailing_data": [], "flexible_trailing_data": []}
            }"#,
        )
        .unwrap(),
        bbox: bounds(0.0, 0.0, width, 108.0 * values.len() as f64),
        rotation_degrees: None,
        column_widths: vec![492.0; values[0].len()],
        rows: values
            .iter()
            .enumerate()
            .map(|(row, values)| RichTextTableRow {
                max_height: None,
                min_height: None,
                metadata: Default::default(),
                index: row as u32,
                height: 108.0,
                cells: values
                    .iter()
                    .enumerate()
                    .map(|(column, value)| RichTextTableCell {
                        border: None,
                        metadata: Default::default(),
                        column_index: column as u32,
                        row_span: 1,
                        column_span: 1,
                        background_color: 0,
                        has_own_background_color: false,
                        bbox: bounds(700.0, 700.0, 500.0, 300.0),
                        editable: false,
                        content: text(value),
                    })
                    .collect(),
            })
            .collect(),
    }
}

fn document(table: RichTextTable, constraint: ObjectSpanLayoutConstraint) -> Document {
    let mut parent = text("\u{fffc}");
    parent.paragraphs.clear();
    parent.margins = None;
    parent.gravity = None;
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
            uuid: "prepared-table".into(),
            width: 1080,
            height: 1527,
            content_bbox: bounds(0.0, 0.0, 1080.0, 1527.0),
            background_color: None,
            template: None,
            background: Default::default(),
            objects: vec![PageElement::TextBox(parent).into()],
        }],
        metadata: DocumentMetadata {
            default_page_dimensions: Some((1080, 1527)),
            orientation: Some(0),
            flow_page_padding: Some((0, 0)),
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

fn lines(svg: &str) -> Vec<(String, f64, f64)> {
    let xml = roxmltree::Document::parse(svg).unwrap();
    xml.descendants()
        .filter(|node| node.has_tag_name("text"))
        .map(|node| {
            let value = node
                .descendants()
                .filter(|child| child.has_tag_name("tspan"))
                .filter_map(|child| child.text())
                .collect();
            let span = node
                .descendants()
                .find(|child| child.has_tag_name("tspan"))
                .unwrap();
            let coordinate = |attribute| {
                span.attribute(attribute)
                    .or_else(|| node.attribute(attribute))
                    .unwrap()
                    .split_whitespace()
                    .next()
                    .unwrap()
                    .parse::<f64>()
                    .unwrap()
            };
            let mut point = (coordinate("x"), coordinate("y"));
            for ancestor in span.ancestors() {
                if let Some(transform) = ancestor.attribute("transform") {
                    let transform: svgtypes::Transform = transform.parse().unwrap();
                    point = (
                        transform.a * point.0 + transform.c * point.1 + transform.e,
                        transform.b * point.0 + transform.d * point.1 + transform.f,
                    );
                }
            }
            (value, point.0, point.1)
        })
        .collect()
}

fn assert_lines(page: &sdocx::RenderedPage, expected: &[(&str, f64, f64)]) {
    let actual = lines(&page.svg);
    assert_eq!(actual.len(), expected.len(), "{actual:?}");
    for ((source, x, y), &(expected_source, expected_x, expected_y)) in actual.iter().zip(expected)
    {
        assert_eq!(source, expected_source);
        assert!((x - expected_x).abs() < 1e-8, "{actual:?}");
        assert!((y - expected_y).abs() < 1e-8, "{actual:?}");
    }
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
}

#[test]
fn over_pages_grid_regenerates_cell_frames_and_forces_top_gravity() {
    // TableLayout::init starts frames at half the unit border. At density 3,
    // margins 12, before 12, lineheight 72 and baseline offset 15.75 yield 80.75.
    // Object placement contributes the independent .001 baseline epsilon.
    // The over-pages branch selects cached frames at 0xa6b04–0xa6b24.
    // Normal drawing uses model cell rectangles; page-band state is separate.
    for constraint in [
        ObjectSpanLayoutConstraint::OverPages,
        ObjectSpanLayoutConstraint::OverPagesOverlapPadding,
    ] {
        let doc = document(table(&[&["A", "B"], &["C", "D"]]), constraint);
        for replay in [false, true] {
            assert_lines(
                &render(&doc, replay),
                &[
                    ("A", 12.5, 80.751),
                    ("B", 504.5, 80.751),
                    ("C", 12.5, 188.751),
                    ("D", 504.5, 188.751),
                ],
            );
        }
    }
}

#[test]
fn cold_measurement_grows_a_row_and_moves_later_cell_frames() {
    // Two native 72px lines grow the first row from 108 to 180; the next origin
    // must follow the grown endpoint, independently of either saved cell box.
    for constraint in [
        ObjectSpanLayoutConstraint::OverPages,
        ObjectSpanLayoutConstraint::OverPagesOverlapPadding,
    ] {
        let doc = document(table(&[&["A\nB"], &["C"]]), constraint);
        for replay in [false, true] {
            assert_lines(
                &render(&doc, replay),
                &[
                    ("A", 12.5, 80.751),
                    ("B", 12.5, 152.751),
                    ("C", 12.5, 260.751),
                ],
            );
        }
    }
}

#[test]
fn sparse_merged_normal_and_paged_tables_keep_all_selectable_cell_source() {
    let mut sparse = table(&[&["A", "B"], &["C", "D"]]);
    sparse.rows[0].cells.remove(0);
    let mut merged = table(&[&["A", "B"], &["C", "D"]]);
    merged.rows[0].cells[0].column_span = 2;
    merged.rows[0].cells.pop();
    for (table, constraint, page_mode, expected) in [
        (sparse, ObjectSpanLayoutConstraint::OverPages, None, "BCD"),
        (merged, ObjectSpanLayoutConstraint::OverPages, None, "ACD"),
        (
            table(&[&["A", "B"], &["C", "D"]]),
            ObjectSpanLayoutConstraint::Normal,
            None,
            "ABCD",
        ),
        (
            table(&[&["A", "B"], &["C", "D"]]),
            ObjectSpanLayoutConstraint::OverPagesOverlapPadding,
            Some(0),
            "ABCD",
        ),
    ] {
        let mut doc = document(table, constraint);
        doc.metadata.page_mode = page_mode;
        for replay in [false, true] {
            let page = render(&doc, replay);
            let source: String = lines(&page.svg).into_iter().map(|line| line.0).collect();
            assert_eq!(source, expected);
            assert!(
                page.object_diagnostics.is_empty(),
                "{:?}",
                page.object_diagnostics
            );
        }
    }
}

#[cfg(feature = "pdf")]
#[test]
fn regenerated_table_text_stays_selectable_and_vector_in_pdf() {
    let doc = document(
        table(&[&["A", "B"], &["C", "D"]]),
        ObjectSpanLayoutConstraint::OverPages,
    );
    for replay in [false, true] {
        let page = render(&doc, replay);
        assert_lines(
            &page,
            &[
                ("A", 12.5, 80.751),
                ("B", 504.5, 80.751),
                ("C", 12.5, 188.751),
                ("D", 504.5, 188.751),
            ],
        );
        let bytes = sdocx::render_svg_pages_pdf(&[page], &Default::default()).unwrap();
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        assert_eq!(
            pdf.extract_text(&[1]).unwrap().replace(['\n', ' '], ""),
            "ABCD"
        );
        assert!(pdf.objects.values().all(|object| {
            object.as_stream().map_or(true, |stream| {
                !stream
                    .dict
                    .get(b"Subtype")
                    .is_ok_and(|value| value.as_name().is_ok_and(|name| name == b"Image"))
            })
        }));
    }
}
