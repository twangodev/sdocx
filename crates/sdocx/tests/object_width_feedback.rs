#![cfg(all(feature = "render", feature = "serde"))]

use sdocx::{
    BoundingBox, Document, DocumentMetadata, ObjectSpanLayoutConstraint, ObjectSpanLayoutOption,
    ObjectType, RenderedPage, RichTextBox, RichTextObjectContent, RichTextObjectSpan,
    RichTextParagraph, RichTextParagraphType, RichTextSection, RichTextTable, RichTextTableCell,
    RichTextTableRow,
};

const A_UNITS: f64 = 1336.0;
const B_UNITS: f64 = 1275.0;
const ROBOTO_UNITS_PER_EM: f64 = 2048.0;

struct Fixture {
    constraint: ObjectSpanLayoutConstraint,
    density: u32,
    page_width: u32,
    column_width: f32,
    alignment: u32,
}

impl Fixture {
    fn new(constraint: ObjectSpanLayoutConstraint, density: u32, page_width: u32) -> Self {
        Self {
            constraint,
            density,
            page_width,
            column_width: 60.0 * density as f32,
            alignment: 0,
        }
    }

    fn font_size(&self) -> f64 {
        10.0 * f64::from(self.density)
    }

    fn a_advance(&self) -> f64 {
        A_UNITS * self.font_size() / ROBOTO_UNITS_PER_EM
    }

    fn b_advance(&self) -> f64 {
        B_UNITS * self.font_size() / ROBOTO_UNITS_PER_EM
    }

    fn initial_advance(&self) -> f64 {
        20.0 * f64::from(self.density) + 1.0 + 8.0 * f64::from(self.density)
    }

    fn callback_width(&self) -> f64 {
        f64::from(self.column_width) + 1.0
    }

    fn document(&self) -> Document {
        let saved_bounds = BoundingBox {
            x_min: 0.0,
            y_min: 0.0,
            x_max: 20.0 * f64::from(self.density),
            y_max: 20.0 * f64::from(self.density),
        };
        let mut cell_text = text("T");
        cell_text.paragraphs.push(RichTextParagraph {
            kind: RichTextParagraphType::LineSpacing,
            start_paragraph: 0,
            end_paragraph: 1,
            payload: [1_u32.to_le_bytes(), 1.0_f32.to_le_bytes()].concat(),
        });
        let table = RichTextTable {
            style: serde_json::from_str(
                r#"{"heading_column_enabled":false,"heading_row_enabled":false,
                    "max_height_enabled":false,"metadata":{"property_mask":[],
                    "field_mask":[],"fixed_trailing_data":[],"flexible_trailing_data":[]}}"#,
            )
            .unwrap(),
            bbox: saved_bounds,
            rotation_degrees: None,
            column_widths: vec![self.column_width],
            rows: vec![RichTextTableRow {
                max_height: None,
                min_height: None,
                metadata: Default::default(),
                index: 0,
                height: 20.0 * self.density as f32,
                cells: vec![RichTextTableCell {
                    border: None,
                    metadata: Default::default(),
                    column_index: 0,
                    row_span: 1,
                    column_span: 1,
                    background_color: 0,
                    has_own_background_color: false,
                    bbox: saved_bounds,
                    editable: false,
                    content: cell_text,
                }],
            }],
        };
        let mut body = text("A\u{fffc}B");
        body.text_sections.push(RichTextSection {
            start_utf16: 0,
            length_utf16: 3,
        });
        body.paragraphs.push(RichTextParagraph {
            kind: RichTextParagraphType::Alignment,
            start_paragraph: 0,
            end_paragraph: 1,
            payload: self.alignment.to_le_bytes().into(),
        });
        body.object_spans.push(RichTextObjectSpan {
            object_type: ObjectType::Table,
            object_data: vec![],
            content: Some(RichTextObjectContent::Table(Box::new(table))),
            text_index_utf16: 1,
            layout_option: ObjectSpanLayoutOption::Inline,
            layout_constraint: self.constraint,
        });
        Document {
            pages: vec![sdocx::Page {
                uuid: "object-width-feedback".into(),
                width: self.page_width,
                height: 500 * self.density,
                content_bbox: BoundingBox::default(),
                background_color: None,
                template: None,
                background: Default::default(),
                objects: vec![],
            }],
            metadata: DocumentMetadata {
                note_text: Some(body),
                page_mode: Some(0),
                default_page_dimensions: Some((360 * self.density, 500 * self.density)),
                orientation: Some(0),
                flow_page_padding: Some((0, 0)),
                ..Default::default()
            },
        }
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
        runs: vec![],
        spans: vec![],
        paragraphs: vec![],
        object_spans: vec![],
        text_sections: vec![],
        margins: None,
        gravity: None,
    }
}

fn modes(fixture: &Fixture) -> [RenderedPage; 2] {
    let document = fixture.document();
    document_modes(&document)
}

fn document_modes(document: &Document) -> [RenderedPage; 2] {
    let layout = sdocx::layout_document(document);
    [
        sdocx::render_layout_page_svg(document, &layout, 0, &Default::default()).unwrap(),
        sdocx::render_layout_page_replay_svg(document, &layout, 0, &Default::default()).unwrap(),
    ]
}

fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 0.00001,
        "actual {actual}, expected {expected}"
    );
}

fn point(node: roxmltree::Node<'_, '_>) -> (f64, f64) {
    let coordinate = |key| {
        node.ancestors()
            .find_map(|ancestor| ancestor.attribute(key))
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .parse::<f64>()
            .unwrap()
    };
    let mut point = (coordinate("x"), coordinate("y"));
    for ancestor in node.ancestors() {
        if let Some(value) = ancestor.attribute("transform") {
            let transform: svgtypes::Transform = value.parse().unwrap();
            point = (
                transform.a * point.0 + transform.c * point.1 + transform.e,
                transform.b * point.0 + transform.d * point.1 + transform.f,
            );
        }
    }
    point
}

struct Geometry {
    a: (f64, f64),
    b: (f64, f64),
    table_x: f64,
    table_width: f64,
}

fn geometry(page: &RenderedPage) -> Geometry {
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
    for source in ["A", "B", "T"] {
        assert_eq!(
            xml.descendants()
                .filter(|node| node.has_tag_name("tspan") && node.text() == Some(source))
                .count(),
            1,
            "source {source}"
        );
    }
    let glyph = |source| {
        point(
            xml.descendants()
                .find(|node| node.has_tag_name("tspan") && node.text() == Some(source))
                .unwrap(),
        )
    };
    let table = xml
        .descendants()
        .find(|node| node.attribute("data-sdocx-object") == Some("table"))
        .unwrap();
    let rectangle = table
        .descendants()
        .find(|node| {
            node.has_tag_name("rect") && node.attribute("fill").is_some_and(|fill| fill != "none")
        })
        .unwrap();
    Geometry {
        a: glyph("A"),
        b: glyph("B"),
        table_x: point(rectangle).0,
        table_width: rectangle.attribute("width").unwrap().parse().unwrap(),
    }
}

fn overpage_constraints() -> [ObjectSpanLayoutConstraint; 2] {
    [
        ObjectSpanLayoutConstraint::OverPagesOverlapPadding,
        ObjectSpanLayoutConstraint::OverPages,
    ]
}

#[test]
fn table_callback_width_replaces_advance_without_readding_body_margins() {
    for constraint in overpage_constraints() {
        for density in [1, 3] {
            let fixture = Fixture::new(constraint, density, 260 * density);
            for page in modes(&fixture) {
                let actual = geometry(&page);
                close(actual.a.0, 0.0);
                close(actual.b.0, fixture.a_advance() + fixture.callback_width());
                close(actual.b.1, actual.a.1);
                close(actual.table_width, fixture.callback_width());
                close(actual.table_x, if density == 1 { 11.0 } else { 32.0 });
            }
        }
    }
}

#[test]
fn current_object_admission_uses_the_width_from_before_its_callback() {
    let face = sdocx::fonts::FontBook::default()
        .resolve("Roboto", false, false)
        .unwrap();
    let mut buffer = sdocx::fonts::UnicodeBuffer::new();
    buffer.push_str("AA");
    buffer.set_direction(sdocx::fonts::Direction::LeftToRight);
    let pair = face.shape(buffer, &[]).unwrap();
    assert_eq!(pair.advance_x(), 2672);
    assert_eq!(pair.metrics.units_per_em, 2048);
    let prefix_advance = pair.advance_x() as f64 * 10.0 / 2048.0;
    for constraint in overpage_constraints() {
        let fixture = Fixture::new(constraint, 1, 70);
        assert!(prefix_advance + fixture.initial_advance() < 70.0);
        assert!(prefix_advance + fixture.callback_width() > 70.0);
        let mut document = fixture.document();
        let body = document.metadata.note_text.as_mut().unwrap();
        body.text = "X\nAA\u{fffc}".into();
        body.text_sections[0].length_utf16 = 5;
        body.object_spans[0].text_index_utf16 = 4;
        for page in document_modes(&document) {
            assert!(page.text_diagnostics.is_empty());
            assert!(page.object_diagnostics.is_empty());
            let xml = roxmltree::Document::parse(&page.svg).unwrap();
            let prefix = xml
                .descendants()
                .find(|node| node.has_tag_name("tspan") && node.text() == Some("AA"))
                .unwrap();
            close(point(prefix).0, 0.0);
            let preceding = xml
                .descendants()
                .find(|node| node.has_tag_name("tspan") && node.text() == Some("X"))
                .unwrap();
            assert!(point(prefix).1 > point(preceding).1);
            let table = xml
                .descendants()
                .find(|node| node.attribute("data-sdocx-object") == Some("table"))
                .unwrap();
            let rectangle = table
                .descendants()
                .find(|node| {
                    node.has_tag_name("rect")
                        && node.attribute("fill").is_some_and(|fill| fill != "none")
                })
                .unwrap();
            close(point(rectangle).0, (prefix_advance + 4.0).floor());
            close(rectangle.attribute("width").unwrap().parse().unwrap(), 61.0);
        }
    }
}

#[test]
fn downstream_text_wraps_after_the_object_callback_changes_width() {
    for constraint in overpage_constraints() {
        for density in [1, 3] {
            let fixture = Fixture::new(constraint, density, 70 * density);
            assert!(fixture.a_advance() + fixture.callback_width() < f64::from(fixture.page_width));
            assert!(
                fixture.a_advance() + fixture.callback_width() + fixture.b_advance()
                    > f64::from(fixture.page_width)
            );
            for page in modes(&fixture) {
                let actual = geometry(&page);
                close(actual.a.0, 0.0);
                close(actual.b.0, 0.0);
                assert!(actual.b.1 > actual.a.1);
                close(actual.table_x, if density == 1 { 11.0 } else { 32.0 });
            }
        }
    }
}

#[test]
fn right_alignment_uses_the_updated_line_width_and_preserves_object_inset() {
    for constraint in overpage_constraints() {
        for density in [1, 3] {
            let mut fixture = Fixture::new(constraint, density, 260 * density);
            fixture.alignment = 1;
            let offset = f64::from(fixture.page_width)
                - fixture.a_advance()
                - fixture.callback_width()
                - fixture.b_advance();
            for page in modes(&fixture) {
                let actual = geometry(&page);
                close(actual.a.0, offset);
                close(
                    actual.b.0,
                    f64::from(fixture.page_width) - fixture.b_advance(),
                );
                close(actual.b.1, actual.a.1);
                close(actual.table_x, if density == 1 { 197.0 } else { 593.0 });
            }
        }
    }
}

#[test]
fn normal_constraint_keeps_the_saved_object_advance() {
    for density in [1, 3] {
        let fixture = Fixture::new(ObjectSpanLayoutConstraint::Normal, density, 260 * density);
        for page in modes(&fixture) {
            let actual = geometry(&page);
            close(actual.a.0, 0.0);
            close(actual.b.0, fixture.a_advance() + fixture.initial_advance());
            close(actual.b.1, actual.a.1);
        }
    }
}

#[test]
fn callback_width_deltas_below_native_epsilon_keep_the_existing_advance() {
    for constraint in overpage_constraints() {
        for column_width in [20.0, 20.0005] {
            let mut fixture = Fixture::new(constraint, 1, 260);
            fixture.column_width = column_width;
            for page in modes(&fixture) {
                let actual = geometry(&page);
                close(actual.b.0, fixture.a_advance() + fixture.initial_advance());
            }
        }
    }
}

#[test]
fn callback_width_deltas_above_native_epsilon_replace_the_existing_advance() {
    for constraint in overpage_constraints() {
        let mut fixture = Fixture::new(constraint, 1, 260);
        fixture.column_width = 20.002;
        for page in modes(&fixture) {
            let actual = geometry(&page);
            close(actual.b.0, fixture.a_advance() + fixture.callback_width());
            close(actual.table_x, 10.0);
        }
    }
}

fn set_table_max_width(document: &mut Document, maximum: Option<f32>) {
    let Some(RichTextObjectContent::Table(table)) =
        document.metadata.note_text.as_mut().unwrap().object_spans[0]
            .content
            .as_mut()
    else {
        panic!("expected table fixture");
    };
    table.style.max_width = maximum;
}

#[test]
fn table_maximum_width_caps_the_callback_but_keeps_the_full_painted_grid() {
    for constraint in overpage_constraints() {
        for density in [1, 3] {
            let fixture = Fixture::new(constraint, density, 260 * density);
            let scale = f64::from(density);
            for (maximum, advance, table_x) in [
                (
                    10.0,
                    10.0 * scale + 0.5,
                    if density == 1 { 10.0 } else { 31.0 },
                ),
                (
                    20.0,
                    fixture.initial_advance(),
                    if density == 1 { 10.0 } else { 31.0 },
                ),
                (
                    40.0,
                    40.0 * scale + 2.0,
                    if density == 1 { 11.0 } else { 32.0 },
                ),
                (
                    80.0,
                    fixture.callback_width(),
                    if density == 1 { 11.0 } else { 32.0 },
                ),
            ] {
                let mut document = fixture.document();
                set_table_max_width(&mut document, Some(maximum * density as f32));
                for page in document_modes(&document) {
                    let actual = geometry(&page);
                    close(actual.a.0, 0.0);
                    close(actual.b.0, fixture.a_advance() + advance);
                    close(actual.b.1, actual.a.1);
                    close(actual.table_x, table_x);
                    close(actual.table_width, fixture.callback_width());
                }
            }
        }
    }
}

#[test]
fn capped_callback_width_still_pushes_following_text_onto_the_next_line() {
    for constraint in overpage_constraints() {
        for density in [1, 3] {
            let fixture = Fixture::new(constraint, density, 50 * density);
            let callback_width = 40.0 * f64::from(density) + 2.0;
            assert!(fixture.a_advance() + callback_width < f64::from(fixture.page_width));
            assert!(
                fixture.a_advance() + callback_width + fixture.b_advance()
                    > f64::from(fixture.page_width)
            );
            let mut document = fixture.document();
            set_table_max_width(&mut document, Some(40.0 * density as f32));
            for page in document_modes(&document) {
                let actual = geometry(&page);
                close(actual.a.0, 0.0);
                close(actual.b.0, 0.0);
                assert!(actual.b.1 > actual.a.1);
                close(actual.table_x, if density == 1 { 11.0 } else { 32.0 });
                close(actual.table_width, fixture.callback_width());
            }
        }
    }
}

#[test]
fn absent_table_maximum_uses_global_content_width_without_paragraph_or_object_insets() {
    for constraint in overpage_constraints() {
        for density in [1, 3] {
            for maximum in [None, Some(0.0)] {
                let fixture = Fixture::new(constraint, density, 70 * density);
                let mut document = fixture.document();
                set_table_max_width(&mut document, maximum);
                let body = document.metadata.note_text.as_mut().unwrap();
                body.margins = Some([10.0, 0.0, 10.0, 0.0]);
                body.paragraphs.push(RichTextParagraph {
                    kind: RichTextParagraphType::IndentLevel,
                    start_paragraph: 0,
                    end_paragraph: 1,
                    payload: [u32::MAX.to_le_bytes(), 1_u32.to_le_bytes()].concat(),
                });
                for page in document_modes(&document) {
                    let actual = geometry(&page);
                    close(actual.a.0, -6.0 * f64::from(density));
                    close(
                        actual.b.0,
                        if density == 1 {
                            53.0234375
                        } else {
                            154.0703125
                        },
                    );
                    close(actual.b.1, actual.a.1);
                    close(actual.table_x, if density == 1 { 5.0 } else { 14.0 });
                    close(actual.table_width, fixture.callback_width());
                }
            }
        }
    }
}

#[cfg(feature = "pdf")]
#[path = "support/pdf_geometry.rs"]
mod pdf_geometry;

#[cfg(feature = "pdf")]
fn selectable_geometry(page: RenderedPage) -> pdf_geometry::PdfGeometry {
    let expected = geometry(&page);
    let xml = roxmltree::Document::parse(&page.svg).unwrap();
    let table_text = xml
        .descendants()
        .find(|node| node.has_tag_name("tspan") && node.text() == Some("T"))
        .unwrap();
    let table_position = point(table_text);
    let bytes = sdocx::render_svg_pages_pdf(&[page], &Default::default()).unwrap();
    let pdf = pdf_geometry::read(&bytes, 96.0);
    assert_eq!(pdf.image_resources, 0);
    assert!(pdf.images.is_empty());
    for (source, position) in [("A", expected.a), ("B", expected.b), ("T", table_position)] {
        assert_eq!(pdf.source.matches(source).count(), 1);
        assert_eq!(pdf.extracted_text.matches(source).count(), 1);
        let glyph = pdf.text.iter().find(|(text, _, _)| text == source).unwrap();
        for (actual, expected) in [(glyph.1, position.0), (glyph.2, position.1)] {
            assert!((actual - expected).abs() < 0.0002, "{actual} != {expected}");
        }
    }
    pdf
}

#[cfg(feature = "pdf")]
#[test]
fn selectable_pdf_uses_the_same_callback_positions_as_normal_and_replay_svg() {
    for constraint in overpage_constraints() {
        for density in [1, 3] {
            for (width, alignment, wrapped) in [(260, 0, false), (70, 0, true), (260, 1, false)] {
                let mut fixture = Fixture::new(constraint, density, width * density);
                fixture.alignment = alignment;
                for page in modes(&fixture) {
                    let pdf = selectable_geometry(page);
                    let position = |source| {
                        let glyph = pdf.text.iter().find(|(text, _, _)| text == source).unwrap();
                        (glyph.1, glyph.2)
                    };
                    let a = position("A");
                    let b = position("B");
                    let close_pdf = |actual: f64, expected: f64| {
                        assert!((actual - expected).abs() < 0.0002, "{actual} != {expected}");
                    };
                    let native_b = if wrapped {
                        0.0
                    } else if alignment == 1 {
                        f64::from(fixture.page_width) - fixture.b_advance()
                    } else {
                        fixture.a_advance() + fixture.callback_width()
                    };
                    close_pdf(b.0, native_b);
                    assert_eq!(b.1 > a.1, wrapped);
                }
            }
        }
    }
}

#[cfg(feature = "pdf")]
#[test]
fn selectable_pdf_keeps_capped_reservations_and_unscaled_table_text() {
    for constraint in overpage_constraints() {
        for density in [1, 3] {
            for (width, wrapped) in [(260, false), (50, true)] {
                let fixture = Fixture::new(constraint, density, width * density);
                let mut document = fixture.document();
                set_table_max_width(&mut document, Some(40.0 * density as f32));
                for page in document_modes(&document) {
                    let actual = geometry(&page);
                    close(actual.table_width, fixture.callback_width());
                    let xml = roxmltree::Document::parse(&page.svg).unwrap();
                    let table_text = xml
                        .descendants()
                        .find(|node| node.has_tag_name("tspan") && node.text() == Some("T"))
                        .unwrap();
                    close(
                        table_text.attribute("font-size").unwrap().parse().unwrap(),
                        fixture.font_size(),
                    );
                    let pdf = selectable_geometry(page);
                    let a = pdf.text.iter().find(|(text, _, _)| text == "A").unwrap();
                    let b = pdf.text.iter().find(|(text, _, _)| text == "B").unwrap();
                    let native_b = if wrapped {
                        0.0
                    } else {
                        fixture.a_advance() + 40.0 * f64::from(density) + 2.0
                    };
                    assert!((b.1 - native_b).abs() < 0.0002, "{} != {native_b}", b.1);
                    assert_eq!(b.2 > a.2, wrapped);
                }
            }
        }
    }
}
