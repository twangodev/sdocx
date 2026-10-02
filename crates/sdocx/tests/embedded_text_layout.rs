#![cfg(feature = "render")]

use sdocx::{
    BoundingBox, Color, Document, DocumentMetadata, ObjectSpanLayoutConstraint,
    ObjectSpanLayoutOption, ObjectType, Page, PageElement, RichTextBox, RichTextCodeBlock,
    RichTextObjectContent, RichTextObjectSpan, RichTextSpan, RichTextSpanType,
};
#[cfg(feature = "serde")]
use sdocx::{RichTextParagraph, RichTextParagraphType};

#[cfg(all(feature = "serde", feature = "pdf"))]
#[path = "support/pdf_geometry.rs"]
mod pdf_geometry;

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
        interval_type: sdocx::SpanIntervalType::from(1),
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
fn saved_grid(rows: usize, columns: usize, spans: &[(usize, u32, u32)]) -> RichTextObjectContent {
    let RichTextObjectContent::Table(mut table) = table(text("A0"), 100.0) else {
        panic!()
    };
    let template = table.rows[0].clone();
    table.bbox.x_max = 20.0 + columns as f64 * 100.0;
    table.bbox.y_max = 20.0 + rows as f64 * 100.0;
    table.column_widths = vec![100.0; columns];
    table.rows = (0..rows)
        .map(|row| {
            let mut record = template.clone();
            record.index = row as u32;
            record.height = 100.0;
            record.cells = (0..columns)
                .map(|column| {
                    let mut cell = template.cells[0].clone();
                    cell.column_index = column as u32;
                    cell.content = text(&format!("A{}", row * columns + column));
                    cell.bbox = BoundingBox {
                        x_min: 20.0 + column as f64 * 100.0,
                        y_min: 20.0 + row as f64 * 100.0,
                        x_max: 120.0 + column as f64 * 100.0,
                        y_max: 120.0 + row as f64 * 100.0,
                    };
                    cell
                })
                .collect();
            record
        })
        .collect();
    for &(index, row_span, column_span) in spans {
        let cell = &mut table.rows[index / columns].cells[index % columns];
        cell.row_span = row_span;
        cell.column_span = column_span;
    }
    RichTextObjectContent::Table(table)
}

#[cfg(feature = "serde")]
fn edge_border(edges: [(u32, f32, f32, f32); 4]) -> sdocx::TableBorder {
    let [left, top, right, bottom] = edges.map(|(color, width, start_radius, end_radius)| {
        serde_json::json!({ "color": color, "width": width,
            "start_radius": start_radius, "end_radius": end_radius })
    });
    serde_json::from_value(serde_json::json!({ "left": left, "top": top,
        "right": right, "bottom": bottom,
        "metadata": { "property_mask": [], "field_mask": [],
            "fixed_trailing_data": [], "flexible_trailing_data": [] } }))
    .unwrap()
}

#[cfg(feature = "serde")]
#[test]
fn table_fills_keep_native_heading_inheritance_and_alpha_in_vector_exports() {
    for constraint in [
        ObjectSpanLayoutConstraint::Normal,
        ObjectSpanLayoutConstraint::OverPages,
    ] {
        let RichTextObjectContent::Table(mut table) = saved_grid(2, 3, &[]) else {
            panic!()
        };
        table.style.heading_row_enabled = true;
        table.style.heading_column_enabled = true;
        table.style.heading_background_color = Some(0x80223344);
        table.style.default_cell_background_color = Some(0x80445566);
        for cell in table.rows.iter_mut().flat_map(|row| &mut row.cells) {
            cell.background_color = 0xffff0000;
            cell.has_own_background_color = true;
        }
        table.rows[1].cells[1].has_own_background_color = false;
        table.rows[1].cells[2].background_color = 0;
        let mut doc = document(RichTextObjectContent::Table(table));
        let sdocx::PageObjectContent::Element(PageElement::TextBox(flow)) =
            &mut doc.pages[0].objects[0].content
        else {
            panic!()
        };
        flow.object_spans[0].layout_constraint = constraint;
        let layout = sdocx::layout_document(&doc);
        for color_mode in [sdocx::RenderColorMode::Light, sdocx::RenderColorMode::Dark] {
            let mut options = sdocx::RenderOptions::default();
            options.color_mode = color_mode;
            let preview = sdocx::render_layout_page_svg(&doc, &layout, 0, &options).unwrap();
            let replay = sdocx::render_layout_page_replay_svg(&doc, &layout, 0, &options).unwrap();
            assert_eq!(preview.svg, replay.svg);
            assert!(
                preview.object_diagnostics.is_empty(),
                "{:?}",
                preview.object_diagnostics
            );
            let xml = roxmltree::Document::parse(&preview.svg).unwrap();
            let fills: Vec<_> = xml
                .descendants()
                .filter(|node| {
                    node.has_tag_name("rect")
                        && node.attribute("fill-opacity").is_some()
                        && node.ancestors().any(|ancestor| {
                            ancestor.attribute("data-sdocx-object") == Some("table")
                        })
                })
                .collect();
            assert_eq!(fills.len(), 6);
            let colors: Vec<_> = fills
                .iter()
                .map(|node| node.attribute("fill").unwrap())
                .collect();
            let expected = if color_mode == sdocx::RenderColorMode::Light {
                [
                    "#223344", "#223344", "#223344", "#223344", "#445566", "#000000",
                ]
            } else {
                [
                    "#bbccdd", "#bbccdd", "#bbccdd", "#bbccdd", "#99aabb", "#ffffff",
                ]
            };
            assert_eq!(colors, expected);
            for (index, node) in fills.iter().enumerate() {
                let opacity: f64 = node.attribute("fill-opacity").unwrap().parse().unwrap();
                let expected = if index == 5 { 0.0 } else { 128.0 / 255.0 };
                assert!((opacity - expected).abs() < 0.000001);
            }
            let transparent_text = xml
                .descendants()
                .find(|node| node.has_tag_name("tspan") && node.text() == Some("A5"))
                .unwrap();
            assert_eq!(
                transparent_text.attribute("fill"),
                Some(if color_mode == sdocx::RenderColorMode::Light {
                    "#000000"
                } else {
                    "#ffffff"
                })
            );
            let source = lines(&preview.svg)
                .iter()
                .map(|line| line.0.as_str())
                .collect::<String>();
            assert_eq!(source, "A0A1A2A3A4A5");
            #[cfg(feature = "pdf")]
            {
                let bytes =
                    sdocx::render_document_pdf(&doc, &options, &Default::default()).unwrap();
                let geometry = pdf_geometry::read(&bytes, 96.0);
                assert_eq!(geometry.source, source);
                assert_eq!(geometry.image_resources, 0);
                let pdf = lopdf::Document::load_mem(&bytes).unwrap();
                assert!(
                    pdf.objects
                        .values()
                        .any(|object| object.as_dict().is_ok_and(|dict| dict
                            .get(b"ca")
                            .is_ok_and(|alpha| alpha.as_float().is_ok_and(|alpha| (f64::from(
                                alpha
                            ) - 128.0
                                / 255.0)
                                .abs()
                                < 0.00001))))
                );
            }
        }
    }
}

#[cfg(feature = "serde")]
#[test]
fn table_borders_keep_native_edge_precedence_alpha_and_vector_transport() {
    let RichTextObjectContent::Table(mut table) = saved_grid(2, 2, &[]) else {
        panic!()
    };
    let outer = [
        (0xffff0000, 2.0, 0.0, 0.0),
        (0xff00ff00, 3.0, 0.0, 0.0),
        (0xff0000ff, 4.0, 0.0, 0.0),
        (0x80441100, 5.0, 0.0, 0.0),
    ];
    table.style.border = Some(edge_border(outer));
    let inactive = (0, 0.0, 0.0, 0.0);
    for cell in table.rows.iter_mut().flat_map(|row| &mut row.cells) {
        cell.border = Some(edge_border([
            inactive,
            inactive,
            (0xffff00ff, 40.0, 0.0, 0.0),
            (0xffff00ff, 50.0, 0.0, 0.0),
        ]));
    }
    table.rows[0].cells[1].border.as_mut().unwrap().left =
        edge_border([(0x80441100, 2.5, 0.0, 0.0); 4]).left;
    table.rows[1].cells[0].border.as_mut().unwrap().top =
        edge_border([(0xff336699, 4.0, 0.0, 0.0); 4]).top;
    table.rows[1].cells[1].border = Some(edge_border([
        (0xff77aa33, 6.0, 0.0, 0.0),
        (0xff993366, 8.0, 0.0, 0.0),
        (0xffff00ff, 40.0, 0.0, 0.0),
        (0xffff00ff, 50.0, 0.0, 0.0),
    ]));
    let doc = document(RichTextObjectContent::Table(table));
    let layout = sdocx::layout_document(&doc);
    for color_mode in [sdocx::RenderColorMode::Light, sdocx::RenderColorMode::Dark] {
        let mut options = sdocx::RenderOptions::default();
        options.color_mode = color_mode;
        let preview = sdocx::render_layout_page_svg(&doc, &layout, 0, &options).unwrap();
        let replay = sdocx::render_layout_page_replay_svg(&doc, &layout, 0, &options).unwrap();
        assert_eq!(preview.svg, replay.svg);
        assert!(preview.object_diagnostics.is_empty());
        let xml = roxmltree::Document::parse(&preview.svg).unwrap();
        let paths: Vec<_> = xml
            .descendants()
            .filter(|node| {
                node.has_tag_name("line")
                    && node
                        .ancestors()
                        .any(|ancestor| ancestor.attribute("data-sdocx-object") == Some("table"))
            })
            .collect();
        assert_eq!(paths.len(), 8);
        let widths: Vec<f64> = paths
            .iter()
            .map(|node| node.attribute("stroke-width").unwrap().parse().unwrap())
            .collect();
        assert_eq!(widths, [2.5, 4.0, 6.0, 8.0, 2.0, 3.0, 4.0, 5.0]);
        for (index, path) in paths.iter().enumerate() {
            let opacity: f64 = path.attribute("stroke-opacity").unwrap().parse().unwrap();
            let expected = if index == 0 || index == 7 {
                128.0 / 255.0
            } else {
                1.0
            };
            assert!((opacity - expected).abs() < 0.000001);
        }
        if color_mode == sdocx::RenderColorMode::Light {
            let colors: Vec<_> = paths
                .iter()
                .map(|node| node.attribute("stroke").unwrap())
                .collect();
            assert_eq!(
                colors,
                [
                    "#441100", "#336699", "#77aa33", "#993366", "#ff0000", "#00ff00", "#0000ff",
                    "#441100"
                ]
            );
        }
        let source = lines(&preview.svg)
            .iter()
            .map(|line| line.0.as_str())
            .collect::<String>();
        assert_eq!(source, "A0A1A2A3");
        #[cfg(feature = "pdf")]
        {
            let bytes = sdocx::render_document_pdf(&doc, &options, &Default::default()).unwrap();
            let geometry = pdf_geometry::read(&bytes, 96.0);
            assert_eq!(geometry.source, source);
            assert_eq!(geometry.image_resources, 0);
            let pdf = lopdf::Document::load_mem(&bytes).unwrap();
            let mut widths = Vec::new();
            for op in
                lopdf::content::Content::decode(&pdf.get_page_content(pdf.get_pages()[&1]).unwrap())
                    .unwrap()
                    .operations
            {
                if op.operator == "w" {
                    widths.push(f64::from(op.operands[0].as_float().unwrap()));
                }
            }
            for object in pdf.objects.values() {
                let Ok(stream) = object.as_stream() else {
                    continue;
                };
                if stream
                    .dict
                    .get(b"Subtype")
                    .is_ok_and(|kind| kind.as_name().is_ok_and(|name| name == b"Form"))
                {
                    let decoded = stream
                        .decompressed_content()
                        .unwrap_or_else(|_| stream.content.clone());
                    for op in lopdf::content::Content::decode(&decoded)
                        .unwrap()
                        .operations
                    {
                        if op.operator == "w" {
                            widths.push(f64::from(op.operands[0].as_float().unwrap()));
                        }
                    }
                }
            }
            for expected in [2.5, 6.0, 8.0] {
                assert!(widths.contains(&expected), "{widths:?}");
            }
            assert!(
                pdf.objects
                    .values()
                    .any(|object| object.as_dict().is_ok_and(|dict| {
                        [b"CA".as_slice(), b"ca".as_slice()].into_iter().any(|key| {
                            dict.get(key).is_ok_and(|value| {
                                value.as_float().is_ok_and(|alpha| {
                                    (f64::from(alpha) - 128.0 / 255.0).abs() < 0.00001
                                })
                            })
                        })
                    }))
            );
        }
    }
}

#[cfg(feature = "serde")]
#[test]
fn rounded_table_outline_uses_native_axis_maxima_and_last_drawable_color() {
    let RichTextObjectContent::Table(mut table) = saved_grid(1, 1, &[]) else {
        panic!()
    };
    table.style.border = Some(edge_border([
        (0xffff0000, 2.0, 4.0, 8.0),
        (0xff00ff00, 0.25, 3.0, 7.0),
        (0xff0000ff, 5.0, 9.0, 2.0),
        (0x80441100, 3.0, 6.0, 1.0),
    ]));
    let svg = render(RichTextObjectContent::Table(table));
    let xml = roxmltree::Document::parse(&svg).unwrap();
    let outline = xml
        .descendants()
        .find(|node| {
            node.has_tag_name("rect")
                && node.attribute("fill") == Some("none")
                && node.attribute("stroke") == Some("#441100")
        })
        .unwrap();
    assert_eq!(outline.attribute("rx"), Some("7.00000"));
    assert_eq!(outline.attribute("ry"), Some("9.00000"));
    assert_eq!(outline.attribute("stroke-width"), Some("5.00000"));
    assert_eq!(outline.attribute("stroke-opacity"), Some("0.501961"));
    assert!(!xml.descendants().any(|node| node.has_tag_name("line")));
    assert_eq!(
        lines(&svg)
            .iter()
            .map(|line| line.0.as_str())
            .collect::<String>(),
        "A0"
    );
}

#[cfg(feature = "serde")]
#[test]
fn merged_prepared_perimeters_use_styles_on_covered_boundary_cells() {
    let RichTextObjectContent::Table(mut table) = saved_grid(2, 2, &[(0, 2, 2)]) else {
        panic!()
    };
    table.style.border = Some(edge_border([(0, 0.0, 0.0, 0.0); 4]));
    for (cell, (color, width)) in table.rows.iter_mut().flat_map(|row| &mut row.cells).zip([
        (0xffff0000, 2.0),
        (0xff00ff00, 3.0),
        (0xff0000ff, 4.0),
        (0xffff00ff, 5.0),
    ]) {
        cell.border = Some(edge_border([(color, width, 0.0, 0.0); 4]));
    }
    let doc = document(RichTextObjectContent::Table(table));
    let page = sdocx::render_page_svg(&doc, 0, &Default::default()).unwrap();
    assert!(
        page.object_diagnostics.is_empty(),
        "{:?}",
        page.object_diagnostics
    );
    let xml = roxmltree::Document::parse(&page.svg).unwrap();
    let edges: Vec<_> = xml
        .descendants()
        .filter(|node| node.has_tag_name("line"))
        .map(|node| {
            (
                node.attribute("stroke").unwrap(),
                node.attribute("stroke-width")
                    .unwrap()
                    .parse::<f64>()
                    .unwrap(),
            )
        })
        .collect();
    assert_eq!(
        edges,
        [
            ("#ff0000", 2.0),
            ("#0000ff", 4.0),
            ("#ff0000", 2.0),
            ("#00ff00", 3.0)
        ]
    );
    assert_eq!(
        lines(&page.svg)
            .iter()
            .map(|line| line.0.as_str())
            .collect::<String>(),
        "A0"
    );
}

#[cfg(feature = "serde")]
#[test]
fn merged_prepared_cells_keep_native_visibility_across_vector_outputs() {
    for (rows, columns, spans, visible) in [
        (1, 4, vec![(0, 1, 2)], vec![0, 2, 3]),
        (4, 1, vec![(0, 2, 1)], vec![0, 2, 3]),
        (2, 2, vec![(0, 2, 2)], vec![0]),
        (1, 4, vec![(0, 1, 2), (1, 1, 3)], vec![0, 2, 3]),
        (4, 1, vec![(0, 2, 1), (1, 3, 1)], vec![0, 2, 3]),
    ] {
        for constraint in [
            ObjectSpanLayoutConstraint::Normal,
            ObjectSpanLayoutConstraint::OverPages,
            ObjectSpanLayoutConstraint::OverPagesOverlapPadding,
        ] {
            let mut doc = document(saved_grid(rows, columns, &spans));
            let sdocx::PageObjectContent::Element(PageElement::TextBox(flow)) =
                &mut doc.pages[0].objects[0].content
            else {
                panic!()
            };
            flow.object_spans[0].layout_constraint = constraint;
            let layout = sdocx::layout_document(&doc);
            for color_mode in [sdocx::RenderColorMode::Light, sdocx::RenderColorMode::Dark] {
                let mut options = sdocx::RenderOptions::default();
                options.color_mode = color_mode;
                let preview = sdocx::render_layout_page_svg(&doc, &layout, 0, &options).unwrap();
                let replay =
                    sdocx::render_layout_page_replay_svg(&doc, &layout, 0, &options).unwrap();
                assert_eq!(preview.svg, replay.svg);
                assert_eq!(preview.object_diagnostics, replay.object_diagnostics);
                let svg_text = lines(&preview.svg);
                assert_eq!(
                    svg_text
                        .iter()
                        .map(|line| line.0.clone())
                        .collect::<Vec<_>>(),
                    visible
                        .iter()
                        .map(|index| format!("A{index}"))
                        .collect::<Vec<_>>()
                );
                assert!(
                    preview.object_diagnostics.is_empty(),
                    "{:?}",
                    preview.object_diagnostics
                );
                #[cfg(feature = "pdf")]
                {
                    let pdf_options = sdocx::PdfOptions::default();
                    let bytes = sdocx::render_document_pdf(&doc, &options, &pdf_options).unwrap();
                    let pdf = pdf_geometry::read(&bytes, f64::from(pdf_options.dpi));
                    assert_eq!(
                        pdf.source,
                        svg_text
                            .iter()
                            .map(|line| line.0.as_str())
                            .collect::<String>()
                    );
                    assert_eq!(pdf_geometry::tagged_source(&bytes), pdf.source);
                    assert_eq!(
                        pdf.extracted_text.split_whitespace().collect::<String>(),
                        pdf.source
                    );
                    assert_eq!(pdf.image_resources, 0);
                    assert_eq!(pdf.text.len(), svg_text.len());
                    for (pdf_line, svg_line) in pdf.text.iter().zip(&svg_text) {
                        assert_eq!(pdf_line.0, svg_line.0);
                        assert!((pdf_line.1 - svg_line.1).abs() < 0.02);
                        assert!((pdf_line.2 - svg_line.2).abs() < 0.02);
                    }
                }
            }
        }
    }
}

#[cfg(feature = "serde")]
#[test]
fn sparse_and_malformed_tables_preserve_saved_text_with_diagnostics() {
    for variant in 0..3 {
        let RichTextObjectContent::Table(mut table) = saved_grid(1, 2, &[]) else {
            panic!()
        };
        match variant {
            0 => {
                table.rows[0].cells.remove(0);
            }
            1 => table.rows[0].cells[0].column_span = 0,
            2 => table.rows[0].cells[0].column_span = u32::MAX,
            _ => unreachable!(),
        }
        let mut doc = document(RichTextObjectContent::Table(table));
        let sdocx::PageObjectContent::Element(PageElement::TextBox(flow)) =
            &mut doc.pages[0].objects[0].content
        else {
            panic!()
        };
        flow.text = "BEFORE\n\u{fffc}\nAFTER".into();
        flow.object_spans[0].text_index_utf16 = 7;
        let layout = sdocx::layout_document(&doc);
        for rendered in [
            sdocx::render_layout_page_svg(&doc, &layout, 0, &Default::default()).unwrap(),
            sdocx::render_layout_page_replay_svg(&doc, &layout, 0, &Default::default()).unwrap(),
        ] {
            assert_eq!(
                rendered.object_diagnostics,
                [sdocx::ObjectDiagnostic {
                    anchor_utf16: 7,
                    kind: sdocx::ObjectDiagnosticKind::UnsupportedContent,
                }]
            );
            let mut actual: Vec<_> = lines(&rendered.svg)
                .into_iter()
                .map(|line| line.0)
                .collect();
            actual.sort();
            assert_eq!(
                actual,
                if variant == 0 {
                    vec!["A1", "AFTER", "BEFORE"]
                } else {
                    vec!["A0", "A1", "AFTER", "BEFORE"]
                }
            );
        }
    }
}

#[cfg(feature = "serde")]
#[test]
fn merged_table_layout_keeps_original_anchor_and_neighbor_text() {
    let mut doc = document(saved_grid(1, 4, &[(0, 1, 2), (1, 1, 3)]));
    let sdocx::PageObjectContent::Element(PageElement::TextBox(flow)) =
        &mut doc.pages[0].objects[0].content
    else {
        panic!()
    };
    flow.text = "BEFORE\n\u{fffc}\nAFTER".into();
    flow.object_spans[0].text_index_utf16 = 7;
    let layout = sdocx::layout_document(&doc);
    for rendered in [
        sdocx::render_layout_page_svg(&doc, &layout, 0, &Default::default()).unwrap(),
        sdocx::render_layout_page_replay_svg(&doc, &layout, 0, &Default::default()).unwrap(),
    ] {
        assert!(
            rendered.object_diagnostics.is_empty(),
            "{:?}",
            rendered.object_diagnostics
        );
        let mut actual: Vec<_> = lines(&rendered.svg)
            .into_iter()
            .map(|line| line.0)
            .collect();
        actual.sort();
        assert_eq!(actual, ["A0", "A2", "A3", "AFTER", "BEFORE"]);
    }
}

#[cfg(feature = "serde")]
#[test]
fn saved_height_limits_keep_preview_replay_and_pdf_geometry() {
    for enable_table_limit in [false, true] {
        for constraint in [
            ObjectSpanLayoutConstraint::Normal,
            ObjectSpanLayoutConstraint::OverPages,
            ObjectSpanLayoutConstraint::OverPagesOverlapPadding,
        ] {
            let mut baseline = document(table(text("ABC\nDEF\nGHI"), 200.0));
            let sdocx::PageObjectContent::Element(PageElement::TextBox(flow)) =
                &mut baseline.pages[0].objects[0].content
            else {
                panic!()
            };
            flow.object_spans[0].layout_constraint = constraint;
            let mut limited = baseline.clone();
            let sdocx::PageObjectContent::Element(PageElement::TextBox(flow)) =
                &mut limited.pages[0].objects[0].content
            else {
                panic!()
            };
            let Some(RichTextObjectContent::Table(table)) = &mut flow.object_spans[0].content
            else {
                panic!()
            };
            table.rows[0].max_height = Some(5.0);
            table.style.max_height_enabled = enable_table_limit;
            table.style.max_height = Some(5.0);
            let baseline_layout = sdocx::layout_document(&baseline);
            let limited_layout = sdocx::layout_document(&limited);
            for color_mode in [sdocx::RenderColorMode::Light, sdocx::RenderColorMode::Dark] {
                let mut options = sdocx::RenderOptions::default();
                options.color_mode = color_mode;
                let expected =
                    sdocx::render_layout_page_svg(&baseline, &baseline_layout, 0, &options)
                        .unwrap();
                assert_eq!(lines(&expected.svg).len(), 3);
                for actual in [
                    sdocx::render_layout_page_svg(&limited, &limited_layout, 0, &options).unwrap(),
                    sdocx::render_layout_page_replay_svg(&limited, &limited_layout, 0, &options)
                        .unwrap(),
                ] {
                    assert!(
                        actual.svg == expected.svg,
                        "saved height limits changed {constraint:?} {color_mode:?} SVG geometry"
                    );
                    assert_eq!(actual.object_diagnostics, expected.object_diagnostics);
                    assert_eq!(actual.text_diagnostics, expected.text_diagnostics);
                }
                #[cfg(feature = "pdf")]
                {
                    let pdf_options = sdocx::PdfOptions::default();
                    let expected = pdf_geometry::read(
                        &sdocx::render_document_pdf(&baseline, &options, &pdf_options).unwrap(),
                        f64::from(pdf_options.dpi),
                    );
                    let actual = pdf_geometry::read(
                        &sdocx::render_document_pdf(&limited, &options, &pdf_options).unwrap(),
                        f64::from(pdf_options.dpi),
                    );
                    assert!(!actual.text.is_empty());
                    assert_eq!(actual.text, expected.text);
                    assert_eq!(actual.source, expected.source);
                    assert_eq!(actual.extracted_text, expected.extracted_text);
                    assert_eq!(actual.image_resources, 0);
                }
            }
        }
    }
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
        vec![("ABC".into(), 38.0, 54.0), ("DEF".into(), 38.0, 129.75)]
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
    // Roboto hmtx [1336,1275,1333], paint size 4500, truncated vector 24.8 advances.
    const ABC_PAINT_ADVANCES: [i32; 3] = [751500, 717187, 749812];
    let abc_advance = ABC_PAINT_ADVANCES
        .into_iter()
        .fold(0.0_f32, |width, advance| {
            width + advance as f32 / 256.0 / 100.0
        });
    let centered_x = 38.0 + (182.0 - f64::from(abc_advance)) / 2.0;
    let mut content = text("ABC");
    content.paragraphs.push(RichTextParagraph {
        kind: RichTextParagraphType::Alignment,
        start_paragraph: 0,
        end_paragraph: 1,
        payload: 2_u32.to_le_bytes().to_vec(),
    });
    let svg = render(table(content.clone(), 200.0));
    assert_eq!(lines(&svg), vec![("ABC".into(), 85.67, 54.0)]);
    let xml = roxmltree::Document::parse(&svg).unwrap();
    let positioned = xml
        .descendants()
        .find(|node| node.has_tag_name("tspan"))
        .unwrap();
    assert_eq!(
        positioned.attribute("x").unwrap().split_whitespace().next(),
        Some(format!("{centered_x:.5}").as_str())
    );
    content.paragraphs.clear();
    assert_eq!(
        lines(&render(table(content, 104.0))),
        vec![("AB".into(), 38.0, 54.0), ("C".into(), 38.0, 114.75),]
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
            ("ABC".into(), 86.0, 90.00101),
            ("DEF".into(), 86.0, 150.75101),
            ("GHI".into(), 86.0, 186.00101),
            ("JKL".into(), 86.0, 322.50101),
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
            ("ABCABC".into(), 86.0, 90.00101),
            ("ABC".into(), 86.0, 150.75101),
            ("ABCABCABC".into(), 86.0, 186.00101),
            ("ABC".into(), 86.0, 246.75101),
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
    assert_eq!(output[1].2, 177.00101);
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
        (
            ObjectSpanLayoutConstraint::Normal,
            246.75101,
            307.50101,
            398.25,
        ),
        (
            ObjectSpanLayoutConstraint::OverPagesOverlapPadding,
            246.75101,
            346.0,
            436.75,
        ),
        (
            ObjectSpanLayoutConstraint::OverPages,
            246.75101,
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
                ("Title".into(), 86.0, 90.00101),
                ("A".into(), 86.0, 186.00101),
                ("B".into(), 86.0, second),
                ("C".into(), 86.0, third),
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
                    ("Title".into(), 86.0, 90.00101),
                    ("A".into(), 86.0, 186.00101),
                    ("B".into(), 86.0, 246.75101),
                    ("C".into(), 86.0, 375.0),
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
                    ("Title".into(), 86.0, 90.00101),
                    ("A".into(), 86.0, 186.00101),
                    ("B".into(), 86.0, 246.75101),
                    ("C".into(), 86.0, 307.50101),
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
            ("Title".into(), 80.0, 81.00098),
            ("A".into(), 80.0, 177.00098),
            ("B".into(), 80.0, 237.75098),
            ("C".into(), 80.0, 298.50098),
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
    assert_eq!(
        panel.attribute("y").unwrap().parse::<f64>().unwrap(),
        0.00098
    );
}

#[test]
fn placed_live_code_candidate_reproduces_the_captured_page_gap() {
    let native_top = 1297.751953125;
    let page_boundary = 1527.0;
    let second_body_candidate = native_top + 132.0 + 60.75;
    assert_eq!(page_boundary + 1.0 - second_body_candidate, 37.498046875);
    let panel_height = 411.75_f32;
    let native_object_offset = f64::from(panel_height + 0.001_f32) - f64::from(panel_height);
    assert_eq!(native_object_offset, 0.001007080078125);
    let serialized_coordinate = |value: f64| format!("{value:.5}").parse::<f64>().unwrap();
    let expected_baselines = [
        native_top + native_object_offset + 81.0,
        native_top + native_object_offset + 177.0,
        page_boundary + 1.0 + 45.0,
        page_boundary + 1.0 + 45.0 + 60.75,
    ]
    .map(serialized_coordinate);
    let reference_baselines = [1378.75295, 1474.75295, 1573.0, 1633.75];
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
        y_min: native_top,
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
        for (((_, x, y), expected_y), reference_y) in output
            .into_iter()
            .zip(expected_baselines)
            .zip(reference_baselines)
        {
            assert_eq!(x, 68.0);
            assert_eq!((y as f32).to_bits(), (reference_y as f32).to_bits());
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
        vec![("ABC".into(), 38.0, 65.25), ("DEF".into(), 38.0, 137.25),]
    );
}
