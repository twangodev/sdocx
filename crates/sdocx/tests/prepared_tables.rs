#![cfg(all(feature = "render", feature = "serde"))]

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

fn font_family(start: u32, end: u32, family: &str) -> RichTextSpan {
    RichTextSpan {
        kind: RichTextSpanType::FontName,
        start_utf16: start,
        end_utf16: end,
        expand: true,
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
                ("A", 0.5, 20.501),
                ("B", 0.5, 34.001),
                ("C", 0.5, 47.501),
                ("D", 0.5, 61.001),
                ("E", 0.5, 74.501),
            ],
            vec![("F", 0.5, 11.001)],
        ),
        (
            ObjectSpanLayoutConstraint::OverPages,
            vec![
                ("A", 0.5, 20.501),
                ("B", 0.5, 34.001),
                ("C", 0.5, 47.501),
                ("D", 0.5, 61.001),
            ],
            vec![("E", 0.5, 20.001), ("F", 0.5, 33.501)],
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
            assert!(
                second.text_diagnostics.is_empty(),
                "{:?}",
                second.text_diagnostics
            );
            let xml = roxmltree::Document::parse(&second.svg).unwrap();
            let css: String = xml
                .descendants()
                .filter(|node| node.has_tag_name("style"))
                .filter_map(|node| node.text())
                .collect();
            assert_eq!(css.matches("@font-face").count(), 1);
            assert!(css.contains("font-family:\"Roboto Mono\";"));
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
fn cold_paginated_rows_keep_saved_heights_without_warm_compression() {
    for (constraint, first, second) in [
        (
            ObjectSpanLayoutConstraint::OverPagesOverlapPadding,
            vec![("A", 0.5, 20.501), ("B", 0.5, 74.501)],
            Vec::new(),
        ),
        (
            ObjectSpanLayoutConstraint::OverPages,
            vec![("A", 0.5, 20.501)],
            vec![("B", 0.5, 20.001)],
        ),
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
        // The incoming candidate is already 10: no parent retry turns this
        // cold pass into a warm pass that could shrink the saved 54px rows.
        doc.metadata.note_text.as_mut().unwrap().margins = Some([0.0, 10.0, 0.0, 0.0]);
        let layout = sdocx::layout_document(&doc);
        let fonts = sdocx::fonts::FontBook::default();
        for replay in [false, true] {
            assert_lines(
                &render_capture_page(&doc, &layout, 1, replay, &fonts),
                &second,
            );
            assert_lines(
                &render_capture_page(&doc, &layout, 0, replay, &fonts),
                &first,
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
    let (drawn_height, middle, last) = match constraint {
        ObjectSpanLayoutConstraint::OverPagesOverlapPadding => (
            85.0,
            vec![
                ("A", 0.5, 20.501),
                ("B", 0.5, 34.001),
                ("C", 0.5, 47.501),
                ("D", 0.5, 61.001),
                ("E", 0.5, 74.501),
            ],
            vec![("F", 0.5, 11.001), ("End", 0.0, 28.501)],
        ),
        ObjectSpanLayoutConstraint::OverPages => (
            107.5,
            vec![
                ("A", 0.5, 20.501),
                ("B", 0.5, 34.001),
                ("C", 0.5, 47.501),
                ("D", 0.5, 61.001),
            ],
            vec![("E", 0.5, 20.001), ("F", 0.5, 33.501), ("End", 0.0, 51.001)],
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
    // MeasureParagraph 0x78a64 skips ordinal-zero prefix metrics; the first
    // object-only paragraph keeps F0. Inline advances H+.001, then End adds 10.
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
            assert_eq!(
                page,
                render_capture_page(&direct, &direct_layout, index, replay, &fonts)
            );
            if index == 1 {
                let xml = roxmltree::Document::parse(&page.svg).unwrap();
                assert!(xml.descendants().any(|node| {
                    node.has_tag_name("rect")
                        && node
                            .attribute("width")
                            .and_then(|value| value.parse::<f64>().ok())
                            == Some(301.0)
                        && node
                            .attribute("height")
                            .and_then(|value| value.parse::<f64>().ok())
                            == Some(drawn_height)
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

#[test]
fn preceding_lf_seeds_the_native_inline_object_text_metric() {
    for (constraint, expected) in [
        (
            ObjectSpanLayoutConstraint::OverPagesOverlapPadding,
            vec![("F", 0.5, 11.001), ("End", 0.0, 28.501)],
        ),
        (
            ObjectSpanLayoutConstraint::OverPages,
            vec![("E", 0.5, 20.001), ("F", 0.5, 33.501), ("End", 0.0, 51.001)],
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
fn unsupported_table_band_width_preserves_saved_cell_text_in_preview_and_pdf() {
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
        assert_eq!(page.object_diagnostics.len(), 1);
        assert_eq!(page.object_diagnostics[0].anchor_utf16, 0);
        assert_eq!(
            page.object_diagnostics[0].kind,
            sdocx::ObjectDiagnosticKind::UnsupportedContent
        );
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
