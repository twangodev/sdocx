#![cfg(all(feature = "render", feature = "serde"))]

#[cfg(feature = "pdf")]
#[path = "support/pdf_geometry.rs"]
mod pdf_geometry;

use sdocx::{
    BoundingBox, Document, DocumentMetadata, ObjectSpanLayoutConstraint, ObjectSpanLayoutOption,
    ObjectType, Page, PageElement, RichTextBox, RichTextObjectContent, RichTextObjectSpan,
    RichTextParagraph, RichTextParagraphType, RichTextSection, RichTextSpan, RichTextSpanType,
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

#[test]
fn translated_public_cell_exports_keep_the_captured_native_plan() {
    use sha2::{Digest, Sha256};

    let bytes = include_bytes!("../../../conformance/table-text-cell-emission.json");
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "0098cfcd93274b210892fd0653677fd4cef3a35ebc4f0419b4b661857cbfa4ce"
    );
    let capture: serde_json::Value = serde_json::from_slice(bytes).unwrap();
    let native_float =
        |value: &serde_json::Value| f64::from(f32::from_bits(value.as_u64().unwrap() as u32));
    for name in ["default-narrow", "fractional-width", "consecutive-newlines"] {
        let case = capture["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|case| case["name"] == name)
            .unwrap();
        let source = case["text_utf8"].as_str().unwrap();
        let width = case["measure_widths"][0].as_f64().unwrap();
        let expected = case["emitted_runs"]["runs"].as_array().unwrap();
        let mut original: Option<Vec<(Vec<f64>, f64)>> = None;
        for offset in [0.0, 1.0] {
            let mut table = table(&[&[source]]);
            table.bbox = bounds(0.0, 0.0, width, 1000.0);
            table.column_widths = vec![width as f32];
            table.rows[0].height = 1000.0;
            let cell = &mut table.rows[0].cells[0];
            cell.bbox = table.bbox;
            cell.content = text(source);
            cell.content.font_size = Some(
                case["requested_font_size_bits"]
                    .as_u64()
                    .map_or(50.0, |bits| f32::from_bits(bits as u32)),
            );
            cell.content.paragraphs = vec![RichTextParagraph {
                kind: RichTextParagraphType::Alignment,
                start_paragraph: 0,
                end_paragraph: 1,
                payload: 2_u32.to_le_bytes().to_vec(),
            }];
            cell.content.margins = None;
            let mut doc = document(table, ObjectSpanLayoutConstraint::Normal);
            doc.metadata.default_page_dimensions = Some((360, 1600));
            let sdocx::PageObjectContent::Element(PageElement::TextBox(body)) =
                &mut doc.pages[0].objects[0].content
            else {
                unreachable!()
            };
            body.margins = Some([offset as f32, 0.0, 0.0, 0.0]);
            let rendered = render(&doc, false);
            assert!(
                rendered.text_diagnostics.is_empty(),
                "{name}: {:?}",
                rendered.text_diagnostics
            );
            assert!(
                rendered.object_diagnostics.is_empty(),
                "{name}: {:?}",
                rendered.object_diagnostics
            );
            let xml = roxmltree::Document::parse(&rendered.svg).unwrap();
            let groups = xml
                .descendants()
                .filter(|node| node.has_tag_name("text"))
                .collect::<Vec<_>>();
            assert_eq!(groups.len(), expected.len(), "{name}");
            let coordinates = groups
                .iter()
                .map(|group| {
                    let spans = group
                        .descendants()
                        .filter(|node| node.has_tag_name("tspan"))
                        .collect::<Vec<_>>();
                    let positions = spans
                        .iter()
                        .flat_map(|span| span.attribute("x").unwrap().split_whitespace())
                        .map(|position| position.parse::<f64>().unwrap())
                        .collect::<Vec<_>>();
                    let baseline = spans[0].attribute("y").unwrap().parse::<f64>().unwrap();
                    (positions, baseline)
                })
                .collect::<Vec<_>>();
            let translation = [
                coordinates[0].0[0] - native_float(&expected[0]["position_bits"][0]),
                coordinates[0].1 - native_float(&expected[0]["origin_bits"][1]),
            ];
            for ((positions, baseline), captured) in coordinates.iter().zip(expected) {
                let expected_positions = captured["position_bits"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| native_float(value) + translation[0])
                    .collect::<Vec<_>>();
                assert_eq!(positions, &expected_positions, "{name}");
                assert_eq!(
                    *baseline,
                    native_float(&captured["origin_bits"][1]) + translation[1],
                    "{name}"
                );
            }
            if let Some(original) = &original {
                for ((positions, baseline), (previous, previous_baseline)) in
                    coordinates.iter().zip(original)
                {
                    assert_eq!(
                        positions,
                        &previous.iter().map(|x| x + offset).collect::<Vec<_>>(),
                        "{name}"
                    );
                    assert_eq!(baseline, previous_baseline, "{name}");
                }
            } else {
                original = Some(coordinates);
            }
            #[cfg(feature = "pdf")]
            {
                let pdf =
                    sdocx::render_document_pdf(&doc, &Default::default(), &Default::default())
                        .unwrap();
                let geometry = pdf_geometry::read(&pdf, 96.0);
                let expected_source = expected
                    .iter()
                    .map(|run| {
                        let range = run["range_inclusive"].as_array().unwrap();
                        let start = range[0].as_u64().unwrap() as usize;
                        let end = range[1].as_u64().unwrap() as usize;
                        source
                            .chars()
                            .skip(start)
                            .take(end + 1 - start)
                            .collect::<String>()
                    })
                    .collect::<Vec<_>>();
                assert_eq!(geometry.actual_text, expected_source, "{name}");
                assert_eq!(geometry.source, source.replace('\n', ""), "{name}");
                assert_eq!(
                    geometry
                        .extracted_text
                        .chars()
                        .filter(|character| !character.is_whitespace())
                        .collect::<String>(),
                    source
                        .chars()
                        .filter(|character| !character.is_whitespace())
                        .collect::<String>(),
                    "{name}"
                );
                assert_eq!(geometry.image_resources, 0);
            }
        }
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
fn fresh_over_pages_drawing_rounds_cell_frames_and_forces_top_gravity() {
    // Native drawing rounds table world endpoints, then cell endpoints. Cell
    // origins .5/492.5 become 0/492; margins12+before12+line72−baseline15.75=80.25.
    for constraint in [
        ObjectSpanLayoutConstraint::OverPages,
        ObjectSpanLayoutConstraint::OverPagesOverlapPadding,
    ] {
        let doc = document(table(&[&["A", "B"], &["C", "D"]]), constraint);
        for replay in [false, true] {
            assert_lines(
                &render(&doc, replay),
                &[
                    ("A", 24.0, 80.25),
                    ("B", 516.0, 80.25),
                    ("C", 24.0, 188.25),
                    ("D", 516.0, 188.25),
                ],
            );
        }
    }
}

#[test]
fn fresh_cold_measurement_grows_a_row_before_warm_drawing_positions_later_cells() {
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
                &[("A", 24.0, 80.25), ("B", 24.0, 152.25), ("C", 24.0, 260.25)],
            );
        }
    }
}

#[test]
fn saved_table_fallbacks_preserve_source_and_report_unsupported_preparation() {
    let mut sparse = table(&[&["A", "B"], &["C", "D"]]);
    sparse.rows[0].cells.remove(0);
    let mut merged = table(&[&["A", "B"], &["C", "D"]]);
    merged.rows[0].cells[0].column_span = 2;
    merged.rows[0].cells.pop();
    for (table, constraint, page_mode, expected, unsupported) in [
        (
            sparse,
            ObjectSpanLayoutConstraint::OverPages,
            None,
            "BCD",
            true,
        ),
        (
            merged,
            ObjectSpanLayoutConstraint::OverPages,
            None,
            "ACD",
            true,
        ),
        (
            table(&[&["A", "B"], &["C", "D"]]),
            ObjectSpanLayoutConstraint::Normal,
            None,
            "ABCD",
            false,
        ),
        (
            table(&[&["A", "B"], &["C", "D"]]),
            ObjectSpanLayoutConstraint::OverPagesOverlapPadding,
            Some(0),
            "ABCD",
            false,
        ),
    ] {
        let mut doc = document(table, constraint);
        doc.metadata.page_mode = page_mode;
        for replay in [false, true] {
            let page = render(&doc, replay);
            let source: String = lines(&page.svg).into_iter().map(|line| line.0).collect();
            assert_eq!(source, expected);
            let diagnostics = if unsupported {
                vec![sdocx::ObjectDiagnostic {
                    anchor_utf16: 0,
                    kind: sdocx::ObjectDiagnosticKind::UnsupportedContent,
                }]
            } else {
                Vec::new()
            };
            assert_eq!(page.object_diagnostics, diagnostics);
        }
    }
}

#[test]
fn dense_table_images_use_prepared_geometry_and_code_keeps_scoped_fallback() {
    use base64::Engine;

    for image in [false, true] {
        let mut grid = table(&[&["\u{fffc}A", "B"]]);
        grid.bbox = bounds(0.0, 0.0, 400.0, 180.0);
        grid.column_widths = vec![200.0; 2];
        grid.rows[0].height = 180.0;
        for (index, cell) in grid.rows[0].cells.iter_mut().enumerate() {
            cell.bbox = bounds(index as f64 * 200.0, 0.0, 200.0, 180.0);
            cell.content.paragraphs.clear();
            cell.content.margins = None;
        }
        let nested = if image {
            RichTextObjectContent::Image(Box::new(
                serde_json::from_value(serde_json::json!({
                    "bbox": bounds(0.0, 0.0, 30.0, 40.0), "rotation_degrees": null,
                    "media_id": null, "media_index": 0, "crop_rect": null,
                    "original_bbox": null, "border_media_id": null, "original_media_id": null,
                }))
                .unwrap(),
            ))
        } else {
            RichTextObjectContent::CodeBlock(Box::new(sdocx::RichTextCodeBlock {
                bbox: bounds(0.0, 0.0, 140.0, 80.0),
                rotation_degrees: None,
                title: None,
                body: Some(text("CODE")),
            }))
        };
        grid.rows[0].cells[0]
            .content
            .object_spans
            .push(RichTextObjectSpan {
                object_type: if image {
                    ObjectType::Image
                } else {
                    ObjectType::CodeBlock
                },
                object_data: Vec::new(),
                content: Some(nested),
                text_index_utf16: 0,
                layout_option: ObjectSpanLayoutOption::Inline,
                layout_constraint: ObjectSpanLayoutConstraint::Normal,
            });
        let mut doc = document(grid, ObjectSpanLayoutConstraint::OverPages);
        let sdocx::PageObjectContent::Element(PageElement::TextBox(parent)) =
            &mut doc.pages[0].objects[0].content
        else {
            panic!()
        };
        parent.text = "Head \u{fffc}".into();
        parent.object_spans[0].text_index_utf16 = 5;
        if image {
            doc.metadata.media_assets.push(sdocx::MediaAsset {
                name: "media/nested.png".into(),
                archive_id: None,
                mime_type: "image/png".into(),
                data: base64::engine::general_purpose::STANDARD.decode(
                    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC",
                ).unwrap(),
            });
        }
        let original = serde_json::to_value(&doc).unwrap();
        let normal = render(&doc, false);
        let replay = render(&doc, true);
        assert_eq!(normal, replay);
        let source: String = lines(&normal.svg).into_iter().map(|line| line.0).collect();
        assert!(source.contains("Head"), "{source:?}");
        assert!(source.contains('A') && source.contains('B'), "{source:?}");
        if !image {
            assert!(source.contains("CODE"), "{source:?}");
        }
        let xml = roxmltree::Document::parse(&normal.svg).unwrap();
        assert_eq!(
            xml.descendants()
                .filter(|node| node.has_tag_name("image"))
                .count(),
            usize::from(image)
        );
        assert!(
            !xml.descendants()
                .any(|node| node.has_tag_name("foreignObject"))
        );
        let expected_diagnostics = if image {
            Vec::new()
        } else {
            vec![sdocx::ObjectDiagnostic {
                anchor_utf16: 5,
                kind: sdocx::ObjectDiagnosticKind::UnsupportedContent,
            }]
        };
        assert_eq!(normal.object_diagnostics, expected_diagnostics);
        #[cfg(feature = "pdf")]
        {
            let bytes = sdocx::render_svg_pages_pdf(&[normal], &Default::default()).unwrap();
            let pdf = lopdf::Document::load_mem(&bytes).unwrap();
            let selectable = pdf.extract_text(&[1]).unwrap();
            assert_eq!(
                selectable.split_whitespace().collect::<String>(),
                if image { "HeadAB" } else { "HeadACODEB" }
            );
            let image_count =
                pdf.objects
                    .values()
                    .filter(|object| {
                        object.as_stream().is_ok_and(|stream| {
                            stream.dict.get(b"Subtype").is_ok_and(|value| {
                                value.as_name().is_ok_and(|name| name == b"Image")
                            })
                        })
                    })
                    .count();
            assert_eq!(image_count, usize::from(image));
        }
        assert_eq!(serde_json::to_value(&doc).unwrap(), original);
    }
}

#[test]
fn hidden_rotated_table_fallback_does_not_emit_visible_diagnostics() {
    let mut grid = table(&[&["A", "B"]]);
    grid.rotation_degrees = Some(15.0);
    let mut doc = paginated_grid_document(grid, ObjectSpanLayoutConstraint::OverPages);
    doc.metadata.note_text.as_mut().unwrap().margins = Some([0.0, 250.0, 0.0, 0.0]);
    let layout = sdocx::layout_document(&doc);
    let fonts = sdocx::fonts::FontBook::default();
    for replay in [false, true] {
        for index in 0..2 {
            let page = render_capture_page(&doc, &layout, index, replay, &fonts);
            assert!(
                page.object_diagnostics.is_empty(),
                "{:?}",
                page.object_diagnostics
            );
            assert!(lines(&page.svg).is_empty());
            let xml = roxmltree::Document::parse(&page.svg).unwrap();
            assert!(
                !xml.descendants()
                    .any(|node| node.attribute("data-sdocx-object") == Some("table"))
            );
        }
    }
}

#[test]
fn dense_table_rotation_fallback_is_diagnosed_without_rejecting_zero_rotation() {
    for table_rotation in [false, true] {
        for rotation in [None, Some(0.0), Some(15.0)] {
            let mut grid = table(&[&["A", "B"]]);
            if table_rotation {
                grid.rotation_degrees = rotation;
            } else {
                grid.rows[0].cells[0].content.rotation_degrees = rotation;
            }
            let doc = document(grid, ObjectSpanLayoutConstraint::OverPages);
            for replay in [false, true] {
                let page = render(&doc, replay);
                let source: String = lines(&page.svg).into_iter().map(|line| line.0).collect();
                assert_eq!(source, "AB");
                let expected = if rotation == Some(15.0) {
                    vec![sdocx::ObjectDiagnostic {
                        anchor_utf16: 0,
                        kind: sdocx::ObjectDiagnosticKind::UnsupportedContent,
                    }]
                } else {
                    Vec::new()
                };
                assert_eq!(
                    page.object_diagnostics, expected,
                    "table rotation {table_rotation}, {rotation:?}"
                );
            }
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
                ("A", 24.0, 80.25),
                ("B", 516.0, 80.25),
                ("C", 24.0, 188.25),
                ("D", 516.0, 188.25),
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

fn font_family(start: u32, end: u32, family: &str) -> RichTextSpan {
    RichTextSpan {
        kind: RichTextSpanType::FontName,
        start_utf16: start,
        end_utf16: end,
        interval_type: sdocx::SpanIntervalType::from(1),
        payload: [
            vec![0; 8],
            u16::try_from(family.len() + 1)
                .unwrap()
                .to_le_bytes()
                .to_vec(),
            family.as_bytes().to_vec(),
            vec![0],
        ]
        .concat(),
    }
}

fn paginated_document(constraint: ObjectSpanLayoutConstraint, styled: bool) -> Document {
    let mut grid = table(&[&["A\nB\nC\nD\nE\nF"]]);
    grid.bbox = bounds(0.0, 0.0, 300.0, 81.0);
    grid.column_widths = vec![300.0];
    grid.rows[0].height = 81.0;
    let content = &mut grid.rows[0].cells[0].content;
    content.font_size = Some(10.0);
    content.paragraphs.clear();
    content.margins = None;
    if styled {
        content.spans = vec![
            font_family(0, 7, "Unavailable First Table Lines"),
            font_family(8, 11, "Roboto Mono"),
        ];
    }
    paginated_grid_document(grid, constraint)
}

fn paginated_grid_document(
    grid: RichTextTable,
    constraint: ObjectSpanLayoutConstraint,
) -> Document {
    let mut doc = document(grid, constraint);
    let sdocx::PageObjectContent::Element(PageElement::TextBox(mut body)) =
        doc.pages[0].objects.remove(0).content
    else {
        panic!()
    };
    body.text = "\u{fffc}\n".into();
    body.font_size = Some(10.0);
    body.text_sections = vec![
        RichTextSection {
            start_utf16: 0,
            length_utf16: 2,
        },
        RichTextSection {
            start_utf16: 0,
            length_utf16: 2,
        },
    ];
    doc.metadata.note_text = Some(body);
    doc.metadata.page_mode = Some(0);
    doc.metadata.default_page_dimensions = Some((360, 80));
    doc.pages[0].width = 360;
    doc.pages[0].height = 80;
    doc.pages[0].content_bbox = bounds(0.0, 0.0, 360.0, 80.0);
    let mut second = doc.pages[0].clone();
    second.uuid = "prepared-table-second".into();
    doc.pages.push(second);
    doc
}

fn render_capture_page(
    doc: &Document,
    layout: &sdocx::LayoutDocument,
    page: usize,
    replay: bool,
    fonts: &sdocx::fonts::FontBook,
) -> sdocx::RenderedPage {
    if replay {
        sdocx::render_layout_page_replay_svg_with_fonts(
            doc,
            layout,
            page,
            &Default::default(),
            fonts,
        )
        .unwrap()
    } else {
        sdocx::render_layout_page_svg_with_fonts(doc, layout, page, &Default::default(), fonts)
            .unwrap()
    }
}

#[test]
fn paginated_first_row_uses_child_bands_when_its_first_line_fits() {
    // F10 gives 13.5px lines and a 14px first-page minimum including half border.
    // Parent candidate 10 fits before the 70px band; whole-table avoidance would
    // lose A-D. Raw constraint 1 uses [80,81], while raw 2 uses [70,90].
    for (constraint, first, second) in [
        (
            ObjectSpanLayoutConstraint::OverPagesOverlapPadding,
            vec![
                ("A", 4.0, 20.0),
                ("B", 4.0, 33.5),
                ("C", 4.0, 47.0),
                ("D", 4.0, 60.5),
                ("E", 4.0, 74.0),
            ],
            vec![("F", 4.0, 10.48071)],
        ),
        (
            ObjectSpanLayoutConstraint::OverPages,
            vec![
                ("A", 4.0, 20.0),
                ("B", 4.0, 33.5),
                ("C", 4.0, 47.0),
                ("D", 4.0, 60.5),
            ],
            vec![("E", 4.0, 19.34351), ("F", 4.0, 32.84351)],
        ),
    ] {
        let doc = paginated_document(constraint, false);
        let layout = sdocx::layout_document(&doc);
        assert_eq!(layout.pages.len(), 2);
        let capture = layout.pages[1]
            .body_text_slice()
            .unwrap()
            .capture_window
            .as_ref()
            .unwrap();
        assert_eq!(capture.first_page_index, 0);
        assert_eq!(capture.source_range, 0..2);
        assert_eq!(
            layout.pages[1].body_text_capture(&doc).unwrap().text,
            "\u{fffc}\n"
        );
        let fonts = sdocx::fonts::FontBook::default();
        for replay in [false, true] {
            let page_one = render_capture_page(&doc, &layout, 1, replay, &fonts);
            assert_lines(&page_one, &second);
            let page_zero = render_capture_page(&doc, &layout, 0, replay, &fonts);
            assert_lines(&page_zero, &first);
            assert_eq!(
                render_capture_page(&doc, &layout, 1, replay, &fonts),
                page_one
            );
        }
    }
}

#[test]
fn paginated_table_paints_only_visible_cell_fonts_and_diagnostics() {
    for constraint in [
        ObjectSpanLayoutConstraint::OverPagesOverlapPadding,
        ObjectSpanLayoutConstraint::OverPages,
    ] {
        let doc = paginated_document(constraint, true);
        let layout = sdocx::layout_document(&doc);
        let fonts = sdocx::fonts::FontBook::default();
        for replay in [false, true] {
            let second = render_capture_page(&doc, &layout, 1, replay, &fonts);
            let expected_source = if constraint == ObjectSpanLayoutConstraint::OverPages {
                "EF"
            } else {
                "F"
            };
            assert_eq!(
                lines(&second.svg)
                    .into_iter()
                    .map(|line| line.0)
                    .collect::<String>(),
                expected_source
            );
            assert_eq!(
                second.text_diagnostics,
                [sdocx::TextDiagnostic {
                    kind: sdocx::TextDiagnosticKind::UnsupportedMeasurementFont,
                    family: "Roboto Mono".into(),
                    codepoints: expected_source.chars().map(u32::from).collect(),
                }],
            );
            let xml = roxmltree::Document::parse(&second.svg).unwrap();
            let css: String = xml
                .descendants()
                .filter(|node| node.has_tag_name("style"))
                .filter_map(|node| node.text())
                .collect();
            assert_eq!(css.matches("@font-face").count(), 1);
            let selected = fonts.resolve("Roboto Mono", false, false).unwrap();
            assert!(css.contains(&format!("font-family:\"{}\";", selected.svg_family())));
            let first = render_capture_page(&doc, &layout, 0, replay, &fonts);
            assert!(
                first
                    .text_diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.kind
                        == sdocx::TextDiagnosticKind::UnavailableFamily
                        && diagnostic.family == "Unavailable First Table Lines")
            );
            assert_eq!(
                render_capture_page(&doc, &layout, 1, replay, &fonts),
                second
            );
            #[cfg(feature = "pdf")]
            {
                let bytes = sdocx::render_svg_pages_pdf(&[second], &Default::default()).unwrap();
                let pdf = lopdf::Document::load_mem(&bytes).unwrap();
                assert_eq!(
                    pdf.extract_text(&[1]).unwrap().replace(['\n', ' '], ""),
                    expected_source
                );
                assert!(
                    !pdf.objects
                        .values()
                        .any(|object| object.as_stream().is_ok_and(|stream| stream
                            .dict
                            .get(b"Subtype")
                            .is_ok_and(|value| value
                                .as_name()
                                .is_ok_and(|name| name == b"Image"))))
                );
            }
        }
    }
}

#[test]
fn paginated_merged_owners_match_unmerged_projection_with_same_saved_bounds() {
    for constraint in [
        ObjectSpanLayoutConstraint::OverPagesOverlapPadding,
        ObjectSpanLayoutConstraint::OverPages,
    ] {
        let fonts = sdocx::fonts::FontBook::default();
        for merge_rows in [false, true] {
            let mut grid = if merge_rows {
                table(&[&["A\nB\nC\nD\nE\nF"], &["COVERED"]])
            } else {
                table(&[&["A\nB\nC\nD\nE\nF", "COVERED"]])
            };
            grid.column_widths = if merge_rows {
                vec![300.0]
            } else {
                vec![300.0, 100.0]
            };
            grid.bbox = bounds(
                0.0,
                0.0,
                if merge_rows { 300.0 } else { 400.0 },
                if merge_rows { 162.0 } else { 81.0 },
            );
            for row in &mut grid.rows {
                row.height = 81.0;
                for cell in &mut row.cells {
                    cell.content.font_size = Some(10.0);
                    cell.content.paragraphs.clear();
                    cell.content.margins = None;
                }
            }
            if merge_rows {
                grid.rows[0].cells[0].row_span = 2;
            } else {
                grid.rows[0].cells[0].column_span = 2;
            }
            let mut baseline = paginated_document(constraint, false);
            let Some(RichTextObjectContent::Table(projected)) =
                &mut baseline.metadata.note_text.as_mut().unwrap().object_spans[0].content
            else {
                panic!()
            };
            projected.bbox = grid.bbox;
            let baseline_layout = sdocx::layout_document(&baseline);
            let doc = paginated_grid_document(grid, constraint);
            let original = serde_json::to_value(&doc).unwrap();
            let layout = sdocx::layout_document(&doc);
            for replay in [false, true] {
                let mut pages = Vec::new();
                for index in 0..2 {
                    let actual = render_capture_page(&doc, &layout, index, replay, &fonts);
                    let expected =
                        render_capture_page(&baseline, &baseline_layout, index, replay, &fonts);
                    assert_eq!(
                        lines(&actual.svg),
                        lines(&expected.svg),
                        "{constraint:?}, row merge {merge_rows}, page {index}"
                    );
                    assert!(
                        actual.object_diagnostics.is_empty(),
                        "{:?}",
                        actual.object_diagnostics
                    );
                    assert!(
                        actual.text_diagnostics.is_empty(),
                        "{:?}",
                        actual.text_diagnostics
                    );
                    pages.push(actual);
                }
                assert_eq!(serde_json::to_value(&doc).unwrap(), original);
                #[cfg(feature = "pdf")]
                {
                    let expected = if constraint == ObjectSpanLayoutConstraint::OverPages {
                        "EF"
                    } else {
                        "F"
                    };
                    let bytes = sdocx::render_svg_pages_pdf(&pages, &Default::default()).unwrap();
                    let pdf = lopdf::Document::load_mem(&bytes).unwrap();
                    assert_eq!(
                        pdf.extract_text(&[2])
                            .unwrap()
                            .split_whitespace()
                            .collect::<String>(),
                        expected
                    );
                    let bytes =
                        sdocx::render_document_pdf(&doc, &Default::default(), &Default::default())
                            .unwrap();
                    let pdf = lopdf::Document::load_mem(&bytes).unwrap();
                    assert_eq!(
                        pdf.extract_text(&[2])
                            .unwrap()
                            .split_whitespace()
                            .collect::<String>(),
                        expected
                    );
                    assert!(
                        !pdf.objects
                            .values()
                            .any(|object| object.as_stream().is_ok_and(|stream| stream
                                .dict
                                .get(b"Subtype")
                                .is_ok_and(|value| value
                                    .as_name()
                                    .is_ok_and(|name| name == b"Image"))))
                    );
                }
            }
        }
    }
}

#[test]
fn fresh_warm_drawing_compresses_rows_while_the_cold_callback_reservation_is_retained() {
    for constraint in [
        ObjectSpanLayoutConstraint::OverPagesOverlapPadding,
        ObjectSpanLayoutConstraint::OverPages,
    ] {
        let mut grid = table(&[&["A"], &["B"]]);
        grid.bbox = bounds(0.0, 0.0, 300.0, 108.0);
        grid.column_widths = vec![300.0];
        for row in &mut grid.rows {
            row.height = 54.0;
            let content = &mut row.cells[0].content;
            content.font_size = Some(10.0);
            content.paragraphs.clear();
            content.margins = None;
        }
        let mut doc = paginated_grid_document(grid, constraint);
        let body = doc.metadata.note_text.as_mut().unwrap();
        body.margins = Some([0.0, 10.0, 0.0, 0.0]);
        body.text = "\u{fffc}\nEnd".into();
        for section in &mut body.text_sections {
            section.length_utf16 = 5;
        }
        // Callback rows retain54 each, reserving109 including border. Fresh
        // drawing warms both to13.5, so cell origins floor10.5→10 and24→24.
        // End retains callback cursor10+109+3.5+.001, then adds its F10 baseline.
        let layout = sdocx::layout_document(&doc);
        let fonts = sdocx::fonts::FontBook::default();
        for replay in [false, true] {
            assert_lines(
                &render_capture_page(&doc, &layout, 1, replay, &fonts),
                &[("End", 0.0, 52.50101)],
            );
            assert_lines(
                &render_capture_page(&doc, &layout, 0, replay, &fonts),
                &[("A", 4.0, 20.0), ("B", 4.0, 34.0)],
            );
        }
    }
}

fn retry_document(constraint: ObjectSpanLayoutConstraint, top: f32) -> Document {
    let mut doc = paginated_document(constraint, false);
    let mut third = doc.pages[1].clone();
    third.uuid = "prepared-table-third".into();
    doc.pages.push(third);
    let body = doc.metadata.note_text.as_mut().unwrap();
    body.text = "\u{fffc}\nEnd".into();
    body.margins = Some([0.0, top, 0.0, 0.0]);
    body.text_sections = vec![
        RichTextSection {
            start_utf16: 0,
            length_utf16: 5
        };
        3
    ];
    doc
}

#[test]
fn warm_stripe_retry_shrinks_to_direct_native_placement() {
    assert_warm_candidate_retry(ObjectSpanLayoutConstraint::OverPagesOverlapPadding);
}

#[test]
fn warm_full_band_retry_shrinks_to_direct_native_placement() {
    assert_warm_candidate_retry(ObjectSpanLayoutConstraint::OverPages);
}

fn assert_warm_candidate_retry(constraint: ObjectSpanLayoutConstraint) {
    let (rounded_panel_height, middle, last) = match constraint {
        ObjectSpanLayoutConstraint::OverPagesOverlapPadding => (
            85.0,
            vec![
                ("A", 4.0, 20.0),
                ("B", 4.0, 33.5),
                ("C", 4.0, 47.0),
                ("D", 4.0, 60.5),
                ("E", 4.0, 74.0),
            ],
            vec![("F", 4.0, 10.48070), ("End", 0.0, 28.50101)],
        ),
        ObjectSpanLayoutConstraint::OverPages => (
            108.0,
            vec![
                ("A", 4.0, 20.0),
                ("B", 4.0, 33.5),
                ("C", 4.0, 47.0),
                ("D", 4.0, 60.5),
            ],
            vec![
                ("E", 4.0, 19.34351),
                ("F", 4.0, 32.84351),
                ("End", 0.0, 51.00101),
            ],
        ),
        _ => panic!(),
    };
    let retry = retry_document(constraint, 70.0);
    let direct = retry_document(constraint, 90.0);
    let retry_layout = sdocx::layout_document(&retry);
    let direct_layout = sdocx::layout_document(&direct);
    let fonts = sdocx::fonts::FontBook::default();
    // At 70 the raw1/raw2 cold row grows to 104/113; the parent minimum
    // moves it to 90. Warm layout shrinks to 84/106.5 plus unit outer border.
    // Native inverse maps saved drawn height82 to85/107.5, giving fresh
    // origins90.01930236816406/90.156494140625 for the last-page bands.
    // Its cell source is reused after frame rounding; callback reservation
    // remains85/107.5 plus anchor-font leading3.5+.001, then End adds10.
    for replay in [false, true] {
        let mut pages = Vec::new();
        for index in [2, 0, 1, 2] {
            let page = render_capture_page(&retry, &retry_layout, index, replay, &fonts);
            let expected = match index {
                0 => &[][..],
                1 => &middle[..],
                _ => &last[..],
            };
            assert_lines(&page, expected);
            let direct_page = render_capture_page(&direct, &direct_layout, index, replay, &fonts);
            assert_eq!(
                checked_retry_table_layout(&page, index, false),
                checked_retry_table_layout(&direct_page, index, true),
            );
            if index == 1 {
                let xml = roxmltree::Document::parse(&page.svg).unwrap();
                assert!(xml.descendants().any(|node| {
                    node.has_tag_name("rect")
                        && node
                            .parent()
                            .is_some_and(|parent| parent.has_tag_name("clipPath"))
                        && node
                            .attribute("width")
                            .and_then(|value| value.parse::<f64>().ok())
                            == Some(301.0)
                        && node
                            .attribute("height")
                            .and_then(|value| value.parse::<f64>().ok())
                            == Some(rounded_panel_height)
                }));
            }
            if pages.len() < 3 {
                pages.push(page);
            }
        }
        #[cfg(feature = "pdf")]
        {
            // The deliberately reversed rendering order must not alter
            // selectable source in either retry or direct vector export.
            let expected = if constraint == ObjectSpanLayoutConstraint::OverPages {
                ["EFEnd", "", "ABCD"]
            } else {
                ["FEnd", "", "ABCDE"]
            };
            let direct_pages: Vec<_> = [2, 0, 1]
                .into_iter()
                .map(|index| render_capture_page(&direct, &direct_layout, index, replay, &fonts))
                .collect();
            for exported in [&pages, &direct_pages] {
                let bytes = sdocx::render_svg_pages_pdf(exported, &Default::default()).unwrap();
                let pdf = lopdf::Document::load_mem(&bytes).unwrap();
                for (index, source) in (1..=3).zip(expected) {
                    assert_eq!(
                        pdf.extract_text(&[index]).unwrap().replace(['\n', ' '], ""),
                        source
                    );
                }
            }
        }
    }
}

fn checked_retry_table_layout(
    page: &sdocx::RenderedPage,
    index: usize,
    direct: bool,
) -> sdocx::RenderedPage {
    let xml = roxmltree::Document::parse(&page.svg).unwrap();
    let mut result = page.clone();
    let Some(table) = xml
        .descendants()
        .find(|node| node.attribute("data-sdocx-object") == Some("table"))
    else {
        assert_eq!(index, 0);
        return result;
    };
    let artwork = table
        .children()
        .find(|node| node.has_tag_name("g"))
        .unwrap();
    let id = artwork
        .attribute("clip-path")
        .unwrap()
        .strip_prefix("url(#")
        .unwrap()
        .strip_suffix(')')
        .unwrap();
    let clip = table
        .descendants()
        .find(|node| node.attribute("id") == Some(id))
        .unwrap();
    let rectangle = clip.children().find(|node| node.has_tag_name("rect"));
    if direct {
        assert!(
            rectangle.is_none(),
            "body top margin 90 exceeds the 80-unit page"
        );
    } else {
        let rectangle = rectangle.unwrap();
        let number = |name| rectangle.attribute(name).unwrap().parse::<f64>().unwrap();
        let expected = if index == 1 {
            [3.0, 149.0, 303.0, 12.0]
        } else {
            [-1.0, 229.0, 362.0, 12.0]
        };
        assert_eq!(
            [number("x"), number("y"), number("width"), number("height")],
            expected
        );
    }
    result.svg.replace_range(clip.range(), "");
    result
}

#[test]
fn preceding_lf_seeds_the_native_inline_object_text_metric() {
    for (constraint, expected) in [
        (
            ObjectSpanLayoutConstraint::OverPagesOverlapPadding,
            vec![("F", 4.0, 10.48070), ("End", 0.0, 28.50101)],
        ),
        (
            ObjectSpanLayoutConstraint::OverPages,
            vec![
                ("E", 4.0, 19.34351),
                ("F", 4.0, 32.84351),
                ("End", 0.0, 51.00101),
            ],
        ),
    ] {
        let mut doc = retry_document(constraint, 56.5);
        let body = doc.metadata.note_text.as_mut().unwrap();
        body.text = "\n\u{fffc}\nEnd".into();
        body.object_spans[0].text_index_utf16 = 1;
        for section in &mut body.text_sections {
            section.length_utf16 = 6;
        }
        // The leading empty paragraph advances 13.5, giving candidate 70.
        // Ordinal 1 consumes its Type4 separator metrics (0x78a64–0x78acc):
        // the settled Inline object advances H+3.5+.001, then End adds 10.
        let layout = sdocx::layout_document(&doc);
        let fonts = sdocx::fonts::FontBook::default();
        for replay in [false, true] {
            let page = render_capture_page(&doc, &layout, 2, replay, &fonts);
            assert_lines(&page, &expected);
            #[cfg(feature = "pdf")]
            {
                let bytes = sdocx::render_svg_pages_pdf(&[page], &Default::default()).unwrap();
                let pdf = lopdf::Document::load_mem(&bytes).unwrap();
                let expected_source = if constraint == ObjectSpanLayoutConstraint::OverPages {
                    "EFEnd"
                } else {
                    "FEnd"
                };
                assert_eq!(
                    pdf.extract_text(&[1]).unwrap().replace(['\n', ' '], ""),
                    expected_source
                );
            }
        }
    }
}

#[test]
fn a_partial_width_band_outside_cell_text_keeps_the_full_vector_grid() {
    let mut grid = table(&[&["Visible Cell"]]);
    grid.bbox = bounds(0.0, 0.0, 200.0, 20.0);
    grid.column_widths = vec![200.0];
    grid.rows[0].height = 20.0;
    let cell = &mut grid.rows[0].cells[0];
    cell.bbox = grid.bbox;
    cell.content.font_size = Some(10.0);
    cell.content.paragraphs.clear();
    cell.content.margins = None;
    let mut doc = document(grid, ObjectSpanLayoutConstraint::OverPages);
    doc.pages[0].width = 100;
    doc.pages[0].height = 80;
    doc.pages[0].content_bbox = bounds(0.0, 0.0, 100.0, 80.0);
    doc.metadata.default_page_dimensions = Some((360, 80));
    doc.metadata.page_mode = Some(0);
    assert!(doc.metadata.note_text.is_none());
    for replay in [false, true] {
        let page = render(&doc, replay);
        assert_eq!(
            lines(&page.svg)
                .into_iter()
                .map(|line| line.0)
                .collect::<String>(),
            "Visible Cell"
        );
        assert!(
            page.text_diagnostics.is_empty(),
            "{:?}",
            page.text_diagnostics
        );
        assert!(page.object_diagnostics.is_empty());
        #[cfg(feature = "pdf")]
        {
            let bytes = sdocx::render_svg_pages_pdf(&[page], &Default::default()).unwrap();
            let pdf = lopdf::Document::load_mem(&bytes).unwrap();
            assert_eq!(
                pdf.extract_text(&[1]).unwrap().replace(['\n', ' '], ""),
                "VisibleCell"
            );
        }
    }
}
