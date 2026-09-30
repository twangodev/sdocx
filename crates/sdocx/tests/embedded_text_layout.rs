#![cfg(feature = "render")]

use sdocx::{
    BoundingBox, Color, Document, DocumentMetadata, ObjectSpanLayoutConstraint,
    ObjectSpanLayoutOption, ObjectType, Page, PageElement, RichTextBox, RichTextCodeBlock,
    RichTextObjectContent, RichTextObjectSpan, RichTextSpan, RichTextSpanType,
};
#[cfg(feature = "serde")]
use sdocx::{RichTextParagraph, RichTextParagraphType};

fn bounds() -> BoundingBox {
    BoundingBox {
        x_min: 20.0,
        y_min: 20.0,
        x_max: 420.0,
        y_max: 420.0,
    }
}

fn text(value: &str) -> RichTextBox {
    RichTextBox {
        text_area_type: None,
        bbox: BoundingBox::default(),
        rotation_degrees: None,
        text: value.into(),
        color: Some(Color { r: 0, g: 0, b: 0 }),
        highlight_color: None,
        underline: false,
        font_size: Some(15.0),
        runs: Vec::new(),
        spans: Vec::new(),
        paragraphs: Vec::new(),
        object_spans: Vec::new(),
        text_sections: Vec::new(),
        margins: Some([2.0, 3.0, 4.0, 5.0]),
        gravity: None,
    }
}

fn span(kind: RichTextSpanType, start: u32, end: u32, payload: &[u8]) -> RichTextSpan {
    RichTextSpan {
        kind,
        start_utf16: start,
        end_utf16: end,
        expand: true,
        payload: payload.into(),
    }
}

fn document(content: RichTextObjectContent) -> Document {
    let kind = match &content {
        RichTextObjectContent::CodeBlock(_) => ObjectType::CodeBlock,
        RichTextObjectContent::Table(_) => ObjectType::Table,
        _ => panic!("unsupported fixture"),
    };
    let mut flow = text("\u{fffc}");
    flow.margins = None;
    flow.object_spans.push(RichTextObjectSpan {
        object_type: kind,
        object_data: Vec::new(),
        content: Some(content),
        text_index_utf16: 0,
        layout_option: ObjectSpanLayoutOption::Inline,
        layout_constraint: ObjectSpanLayoutConstraint::Normal,
    });
    Document {
        pages: vec![Page {
            uuid: "embedded-layout".into(),
            width: 1080,
            height: 1527,
            content_bbox: bounds(),
            background_color: Some(Color {
                r: 255,
                g: 255,
                b: 255,
            }),
            template: None,
            background: Default::default(),
            objects: vec![PageElement::TextBox(flow).into()],
        }],
        metadata: DocumentMetadata {
            default_page_dimensions: Some((1080, 1527)),
            orientation: Some(0),
            flow_page_padding: Some((20, 20)),
            ..Default::default()
        },
    }
}

fn code(title: RichTextBox, body: RichTextBox) -> RichTextObjectContent {
    RichTextObjectContent::CodeBlock(Box::new(RichTextCodeBlock {
        bbox: bounds(),
        rotation_degrees: None,
        title: Some(title),
        body: Some(body),
    }))
}

#[cfg(feature = "serde")]
fn table(content: RichTextBox, cell_width: f64) -> RichTextObjectContent {
    let style = serde_json::from_str(r#"{
        "heading_column_enabled": false, "heading_row_enabled": false, "max_height_enabled": false,
        "metadata": {"property_mask": [], "field_mask": [], "fixed_trailing_data": [], "flexible_trailing_data": []}
    }"#).unwrap();
    let cell_bounds = BoundingBox {
        x_max: 20.0 + cell_width,
        ..bounds()
    };
    RichTextObjectContent::Table(Box::new(sdocx::RichTextTable {
        style,
        bbox: cell_bounds,
        rotation_degrees: None,
        column_widths: vec![cell_width as f32],
        rows: vec![sdocx::RichTextTableRow {
            max_height: None,
            min_height: None,
            metadata: Default::default(),
            index: 0,
            height: 400.0,
            cells: vec![sdocx::RichTextTableCell {
                border: None,
                metadata: Default::default(),
                column_index: 0,
                row_span: 1,
                column_span: 1,
                background_color: 0,
                has_own_background_color: false,
                bbox: cell_bounds,
                editable: false,
                content,
            }],
        }],
    }))
}

fn render(content: RichTextObjectContent) -> String {
    sdocx::render_page_svg(&document(content), 0, &Default::default())
        .unwrap()
        .svg
}

fn lines(svg: &str) -> Vec<(String, f64, f64)> {
    let xml = roxmltree::Document::parse(svg).unwrap();
    xml.descendants()
        .filter(|node| node.has_tag_name("text"))
        .map(|node| {
            let value = node
                .descendants()
                .filter(|child| child.is_text())
                .filter_map(|child| child.text())
                .collect();
            let y = node
                .descendants()
                .find(|child| child.has_tag_name("tspan") && child.attribute("y").is_some())
                .and_then(|child| child.attribute("y"))
                .or_else(|| node.attribute("y"))
                .unwrap();
            let mut point = (
                node.attribute("x").unwrap().parse::<f64>().unwrap(),
                y.parse::<f64>().unwrap(),
            );
            for ancestor in node.ancestors() {
                if let Some(value) = ancestor.attribute("transform") {
                    let transform: svgtypes::Transform = value.parse().unwrap();
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

#[cfg(feature = "serde")]
#[test]
fn table_cells_render_all_lines_with_margins_and_force_top_gravity() {
    let mut content = text("ABC\nDEF");
    content.gravity = Some(2);
    content.spans = vec![
        span(RichTextSpanType::FontSize, 4, 7, &20.0_f32.to_le_bytes()),
        span(RichTextSpanType::ForegroundColor, 4, 7, &[0, 0, 255, 255]),
        span(RichTextSpanType::Italic, 4, 7, &[1, 0]),
    ];
    let doc = document(table(content, 200.0));
    let svg = sdocx::render_page_svg(&doc, 0, &Default::default())
        .unwrap()
        .svg;
    assert_eq!(
        lines(&svg),
        vec![("ABC".into(), 26.0, 54.0), ("DEF".into(), 26.0, 129.75)]
    );
    let xml = roxmltree::Document::parse(&svg).unwrap();
    let styled = xml
        .descendants()
        .find(|node| node.has_tag_name("tspan") && node.text() == Some("DEF"))
        .unwrap();
    assert_eq!(styled.attribute("font-size"), Some("60.00"));
    assert_eq!(styled.attribute("fill"), Some("#ff0000"));
    assert_eq!(styled.attribute("font-style"), Some("italic"));
    let PageElement::TextBox(flow) = doc.pages[0].elements().next().unwrap() else {
        panic!()
    };
    let Some(RichTextObjectContent::Table(table)) = &flow.object_spans[0].content else {
        panic!()
    };
    assert_eq!(table.rows[0].cells[0].content.gravity, Some(2));
    assert_eq!(table.rows[0].cells[0].content.text, "ABC\nDEF");
}

#[cfg(feature = "serde")]
#[test]
fn table_cell_wrap_and_alignment_use_the_measured_inner_frame() {
    let mut content = text("ABC");
    content.paragraphs.push(RichTextParagraph {
        kind: RichTextParagraphType::Alignment,
        start_paragraph: 0,
        end_paragraph: 1,
        payload: 2_u32.to_le_bytes().to_vec(),
    });
    let svg = render(table(content.clone(), 200.0));
    assert_eq!(lines(&svg), vec![("ABC".into(), 73.67, 54.0)]);
    let xml = roxmltree::Document::parse(&svg).unwrap();
    let positioned = xml
        .descendants()
        .find(|node| node.has_tag_name("tspan"))
        .unwrap();
    assert_eq!(
        positioned.attribute("x").unwrap().split_whitespace().next(),
        Some("73.66992")
    );
    content.paragraphs.clear();
    assert_eq!(
        lines(&render(table(content, 104.0))),
        vec![("AB".into(), 26.0, 54.0), ("C".into(), 26.0, 114.75),]
    );
}

#[test]
fn code_title_and_body_render_every_paragraph_inside_native_frames() {
    // Native density3: padding48/36/48/36, copy72, title gap36, body gap24.
    // Body origin0 plus title/body margins9 and font45 gives90/186.
    let title = text("ABC\nDEF");
    let mut body = text("GHI\r\nJKL");
    body.spans = vec![
        span(RichTextSpanType::ForegroundColor, 5, 8, &[0, 0, 255, 255]),
        span(RichTextSpanType::FontSize, 5, 8, &20.0_f32.to_le_bytes()),
    ];
    let svg = render(code(title, body));
    assert_eq!(
        lines(&svg),
        vec![
            ("ABC".into(), 74.0, 90.001),
            ("DEF".into(), 74.0, 150.751),
            ("GHI".into(), 74.0, 186.001),
            ("JKL".into(), 74.0, 261.751),
        ]
    );
    let xml = roxmltree::Document::parse(&svg).unwrap();
    let styled = xml
        .descendants()
        .find(|node| node.has_tag_name("tspan") && node.text() == Some("JKL"))
        .unwrap();
    assert_eq!(styled.attribute("font-size"), Some("60.00"));
    assert_eq!(styled.attribute("fill"), Some("#ff0000"));
}

#[test]
fn code_title_and_body_wrap_using_their_separate_native_frame_widths() {
    let svg = render(code(text("ABCABCABC"), text("ABCABCABCABC")));
    assert_eq!(
        lines(&svg),
        vec![
            ("ABCABC".into(), 74.0, 90.001),
            ("ABC".into(), 74.0, 150.751),
            ("ABCABCABC".into(), 74.0, 186.001),
            ("ABC".into(), 74.0, 246.751),
        ]
    );
}

#[test]
fn code_text_preserves_spaces_combining_source_and_positioned_glyphs() {
    let source = "office e\u{301}  ";
    let mut body = text(source);
    body.margins = None;
    let svg = render(code(text("Title"), body));
    let output = lines(&svg);
    assert_eq!(
        output
            .iter()
            .skip(1)
            .map(|line| line.0.as_str())
            .collect::<String>(),
        source
    );
    assert_eq!(output[1].2, 177.001);
    let xml = roxmltree::Document::parse(&svg).unwrap();
    let combined = xml
        .descendants()
        .find(|node| node.has_tag_name("tspan") && node.text() == Some("e\u{301}"))
        .unwrap();
    assert!(combined.attribute("x").is_some());
    assert!(combined.attribute("y").is_some());
    let office = xml
        .descendants()
        .find(|node| {
            node.has_tag_name("tspan") && node.text().is_some_and(|text| text.starts_with("office"))
        })
        .unwrap();
    assert_eq!(
        office.attribute("x").unwrap().split_whitespace().count(),
        office.text().unwrap().chars().count()
    );
    for node in xml.descendants().filter(|node| node.has_tag_name("text")) {
        assert_eq!(
            node.attribute(("http://www.w3.org/XML/1998/namespace", "space")),
            Some("preserve")
        );
    }
}

#[cfg(feature = "serde")]
#[test]
fn narrow_table_cell_keeps_complete_repeated_space_and_combining_source() {
    let source = "A  e\u{301}  B  ";
    let svg = render(table(text(source), 57.0));
    let output = lines(&svg);
    assert!(output.len() > 1);
    assert_eq!(
        output
            .iter()
            .map(|line| line.0.as_str())
            .collect::<String>(),
        source
    );
    assert!(output.iter().any(|line| line.0.contains("e\u{301}")));
    assert!(
        !output
            .iter()
            .any(|line| line.0.starts_with('\u{301}') || line.0.ends_with('e'))
    );
}
fn code_page_document(mode: Option<u16>, constraint: ObjectSpanLayoutConstraint) -> Document {
    let mut doc = document(code(text("Title"), text("A\nB\nC")));
    doc.pages[0].height = 300;
    doc.metadata.page_mode = mode;
    let sdocx::PageObjectContent::Element(PageElement::TextBox(flow)) =
        &mut doc.pages[0].objects[0].content
    else {
        panic!()
    };
    flow.object_spans[0].layout_constraint = constraint;
    doc
}

fn code_panel_height(svg: &str) -> f64 {
    let xml = roxmltree::Document::parse(svg).unwrap();
    let group = xml
        .descendants()
        .find(|node| node.attribute("data-sdocx-object") == Some("code-block"))
        .unwrap();
    group
        .children()
        .find(|node| node.has_tag_name("rect"))
        .unwrap()
        .attribute("height")
        .unwrap()
        .parse()
        .unwrap()
}

#[test]
fn list_page_constraints_shift_code_lines_and_panel_height_by_the_observed_gap() {
    // Page boundary300; density3 padding bands270..330. Font45 advances60.75.
    // Body origin141 yields candidates141..201.75,201.75..262.5,262.5..323.25.
    for (constraint, second, third, panel_height) in [
        (ObjectSpanLayoutConstraint::Normal, 246.751, 307.501, 398.25),
        (
            ObjectSpanLayoutConstraint::OverPagesOverlapPadding,
            246.751,
            346.0,
            436.75,
        ),
        (
            ObjectSpanLayoutConstraint::OverPages,
            246.751,
            375.0,
            465.75,
        ),
    ] {
        let doc = code_page_document(Some(0), constraint);
        let svg = sdocx::render_page_svg(&doc, 0, &Default::default())
            .unwrap()
            .svg;
        assert_eq!(
            lines(&svg),
            vec![
                ("Title".into(), 74.0, 90.001),
                ("A".into(), 74.0, 186.001),
                ("B".into(), 74.0, second),
                ("C".into(), 74.0, third),
            ]
        );
        assert_eq!(code_panel_height(&svg), panel_height);
    }
}

#[test]
fn vertical_page_padding_does_not_replace_native_body_text_margins() {
    for vertical_padding in [0, 20, 100] {
        let mut doc = document(code(text("Title"), text("A\nB")));
        doc.pages[0].objects = vec![PageElement::TextBox(text("A\nB")).into()];
        doc.metadata.flow_page_padding = Some((20, vertical_padding));
        let layout = sdocx::layout_document(&doc);
        for page in [
            sdocx::render_layout_page_svg(&doc, &layout, 0, &Default::default()).unwrap(),
            sdocx::render_layout_page_replay_svg(&doc, &layout, 0, &Default::default()).unwrap(),
        ] {
            // Scaled top margin9 plus native60.75 line advance minus.35*45.
            assert_eq!(
                lines(&page.svg),
                [("A".into(), 26.0, 54.0), ("B".into(), 26.0, 114.75)]
            );
        }
        let mut code = code_page_document(Some(0), ObjectSpanLayoutConstraint::OverPages);
        code.metadata.flow_page_padding = Some((20, vertical_padding));
        let layout = sdocx::layout_document(&code);
        for page in [
            sdocx::render_layout_page_svg(&code, &layout, 0, &Default::default()).unwrap(),
            sdocx::render_layout_page_replay_svg(&code, &layout, 0, &Default::default()).unwrap(),
        ] {
            assert_eq!(
                lines(&page.svg),
                [
                    ("Title".into(), 74.0, 90.001),
                    ("A".into(), 74.0, 186.001),
                    ("B".into(), 74.0, 246.751),
                    ("C".into(), 74.0, 375.0),
                ]
            );
            assert_eq!(code_panel_height(&page.svg), 465.75);
        }
    }
}

#[test]
fn continuous_and_unknown_page_modes_do_not_invent_exclusion_bands() {
    for mode in [None, Some(1), Some(2), Some(99)] {
        for constraint in [
            ObjectSpanLayoutConstraint::OverPagesOverlapPadding,
            ObjectSpanLayoutConstraint::OverPages,
        ] {
            let doc = code_page_document(mode, constraint);
            let svg = sdocx::render_page_svg(&doc, 0, &Default::default())
                .unwrap()
                .svg;
            assert_eq!(
                lines(&svg),
                vec![
                    ("Title".into(), 74.0, 90.001),
                    ("A".into(), 74.0, 186.001),
                    ("B".into(), 74.0, 246.751),
                    ("C".into(), 74.0, 307.501),
                ]
            );
            assert_eq!(code_panel_height(&svg), 398.25);
        }
    }
}

#[test]
fn continuation_code_uses_page_local_exclusions_with_its_negative_stored_top() {
    let mut doc = code_page_document(Some(0), ObjectSpanLayoutConstraint::OverPages);
    let sdocx::PageObjectContent::Element(PageElement::TextBox(flow)) =
        &mut doc.pages[0].objects[0].content
    else {
        panic!()
    };
    flow.object_spans[0].layout_option = ObjectSpanLayoutOption::Block;
    let Some(RichTextObjectContent::CodeBlock(code)) = &mut flow.object_spans[0].content else {
        panic!()
    };
    code.bbox.y_min = -100.0;
    code.bbox.y_max = 300.0;
    code.body = Some(text("A\nB\nC\nD"));
    let svg = sdocx::render_page_svg(&doc, 0, &Default::default())
        .unwrap()
        .svg;
    assert_eq!(
        lines(&svg),
        vec![
            ("Title".into(), 74.0, -10.0),
            ("A".into(), 74.0, 86.0),
            ("B".into(), 74.0, 146.75),
            ("C".into(), 74.0, 207.5),
            ("D".into(), 74.0, 375.0),
        ]
    );
    assert_eq!(code_panel_height(&svg), 565.75);
}

#[test]
fn positive_saved_code_y_does_not_move_the_actual_candidate_exclusions() {
    let mut title = text("Title");
    title.margins = None;
    let mut body = text("A\nB\nC");
    body.margins = None;
    let mut content = code(title, body);
    let RichTextObjectContent::CodeBlock(code) = &mut content else {
        panic!()
    };
    code.bbox.y_min = 1297.751953125;
    code.bbox.y_max = 1697.751953125;
    let mut doc = document(content);
    doc.metadata.page_mode = Some(0);
    let sdocx::PageObjectContent::Element(PageElement::TextBox(flow)) =
        &mut doc.pages[0].objects[0].content
    else {
        panic!()
    };
    flow.object_spans[0].layout_constraint = ObjectSpanLayoutConstraint::OverPagesOverlapPadding;
    let svg = sdocx::render_page_svg(&doc, 0, &Default::default())
        .unwrap()
        .svg;
    // The live candidate is0, so these lines do not meet the page1527 band.
    // The saved positive top must not invent the captured37.498046875 gap.
    assert_eq!(
        lines(&svg),
        vec![
            ("Title".into(), 68.0, 81.001),
            ("A".into(), 68.0, 177.001),
            ("B".into(), 68.0, 237.751),
            ("C".into(), 68.0, 298.501),
        ]
    );
    assert_eq!(code_panel_height(&svg), 374.25);
    let xml = roxmltree::Document::parse(&svg).unwrap();
    let group = xml
        .descendants()
        .find(|node| node.attribute("data-sdocx-object") == Some("code-block"))
        .unwrap();
    let panel = group
        .children()
        .find(|node| node.has_tag_name("rect"))
        .unwrap();
    assert_eq!(panel.attribute("y").unwrap().parse::<f64>().unwrap(), 0.001);
}

#[test]
fn placed_live_code_candidate_reproduces_the_captured_page_gap() {
    let mut title = text("Title");
    title.margins = None;
    let mut body = text("A\nB\nC");
    body.margins = None;
    let mut doc = document(code(title, body));
    doc.metadata.page_mode = Some(0);
    let sdocx::PageObjectContent::Element(PageElement::TextBox(parent)) =
        &mut doc.pages[0].objects[0].content
    else {
        panic!()
    };
    parent.bbox = BoundingBox {
        x_min: 20.0,
        y_min: 1297.751953125,
        x_max: 420.0,
        y_max: 1997.751953125,
    };
    parent.object_spans[0].layout_constraint = ObjectSpanLayoutConstraint::OverPagesOverlapPadding;
    let layout = sdocx::layout_document(&doc);
    for page in [
        sdocx::render_layout_page_svg(&doc, &layout, 0, &Default::default()).unwrap(),
        sdocx::render_layout_page_replay_svg(&doc, &layout, 0, &Default::default()).unwrap(),
    ] {
        // Captured native top/page1527 produces a37.498046875px skip
        // after the first body line when this is the live candidate.
        let output = lines(&page.svg);
        assert_eq!(
            output
                .iter()
                .map(|line| line.0.as_str())
                .collect::<Vec<_>>(),
            ["Title", "A", "B", "C"]
        );
        for ((_, x, y), expected_y) in output
            .into_iter()
            .zip([1378.75295, 1474.75295, 1573.0, 1633.75])
        {
            assert_eq!(x, 68.0);
            assert!(
                (y - expected_y).abs() < 1e-8,
                "actual {y}, expected {expected_y}"
            );
        }
        assert_eq!(code_panel_height(&page.svg), 411.75);
    }
}

#[cfg(feature = "serde")]
#[test]
fn table_explicit_percentage_spacing_uses_the_native_ordinary_baseline() {
    let mut content = text("ABC\nDEF");
    content.paragraphs.push(RichTextParagraph {
        kind: RichTextParagraphType::LineSpacing,
        start_paragraph: 0,
        end_paragraph: 2,
        payload: [1_u32.to_le_bytes().to_vec(), 1.6_f32.to_le_bytes().to_vec()].concat(),
    });
    let svg = render(table(content, 200.0));
    assert_eq!(
        lines(&svg),
        vec![("ABC".into(), 26.0, 65.25), ("DEF".into(), 26.0, 137.25),]
    );
}
