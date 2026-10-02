#![cfg(all(feature = "render", feature = "serde"))]

#[path = "support/svg_fonts.rs"]
mod svg_fonts;

use base64::Engine;
use resvg::usvg;
use sdocx::{
    BoundingBox, Color, Document, DocumentMetadata, MediaAsset, ObjectDiagnosticKind,
    ObjectSpanLayoutConstraint, ObjectSpanLayoutOption, ObjectType, Page, PageElement, PlacedImage,
    RenderedPage, RichTextBox, RichTextCodeBlock, RichTextObjectContent, RichTextObjectSpan,
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

fn text(source: &str, density: u32) -> RichTextBox {
    RichTextBox {
        text_area_type: None,
        bbox: BoundingBox::default(),
        rotation_degrees: None,
        text: source.into(),
        color: Some(Color { r: 0, g: 0, b: 0 }),
        highlight_color: None,
        underline: false,
        font_size: Some(45.0 / density as f32),
        runs: Vec::new(),
        spans: Vec::new(),
        paragraphs: Vec::new(),
        object_spans: Vec::new(),
        text_sections: Vec::new(),
        margins: None,
        gravity: None,
    }
}

fn object(anchor: i32, content: RichTextObjectContent) -> RichTextObjectSpan {
    let object_type = match &content {
        RichTextObjectContent::Image(_) => ObjectType::Image,
        RichTextObjectContent::CodeBlock(_) => ObjectType::CodeBlock,
        RichTextObjectContent::Table(_) => ObjectType::Table,
        _ => panic!("unsupported test object"),
    };
    RichTextObjectSpan {
        object_type,
        object_data: Vec::new(),
        content: Some(content),
        text_index_utf16: anchor,
        layout_option: ObjectSpanLayoutOption::Inline,
        layout_constraint: ObjectSpanLayoutConstraint::Normal,
    }
}

fn image(anchor: i32, bbox: BoundingBox) -> RichTextObjectSpan {
    object(
        anchor,
        RichTextObjectContent::Image(Box::new(
            serde_json::from_value::<PlacedImage>(serde_json::json!({
                "bbox": bbox, "rotation_degrees": null, "media_id": null, "media_index": 0,
                "crop_rect": null, "original_bbox": null, "border_media_id": null,
                "original_media_id": null
            }))
            .unwrap(),
        )),
    )
}

fn code(anchor: i32, child: RichTextBox) -> RichTextObjectSpan {
    object(
        anchor,
        RichTextObjectContent::CodeBlock(Box::new(RichTextCodeBlock {
            bbox: bounds(-31.0, -19.0, 600.0, 300.0),
            rotation_degrees: None,
            title: None,
            body: Some(child),
        })),
    )
}

fn table(anchor: i32, child: RichTextBox) -> RichTextObjectSpan {
    let frame = bounds(-31.0, -19.0, 600.0, 180.0);
    object(
        anchor,
        RichTextObjectContent::Table(Box::new(RichTextTable {
            style: serde_json::from_str(
                r#"{
            "heading_column_enabled": false, "heading_row_enabled": false,
            "max_height_enabled": false,
            "metadata": {"property_mask": [], "field_mask": [],
                "fixed_trailing_data": [], "flexible_trailing_data": []}
        }"#,
            )
            .unwrap(),
            bbox: frame,
            rotation_degrees: None,
            column_widths: vec![600.0],
            rows: vec![RichTextTableRow {
                max_height: None,
                min_height: None,
                metadata: Default::default(),
                index: 0,
                height: 180.0,
                cells: vec![RichTextTableCell {
                    border: None,
                    metadata: Default::default(),
                    column_index: 0,
                    row_span: 1,
                    column_span: 1,
                    background_color: 0,
                    has_own_background_color: false,
                    bbox: frame,
                    editable: false,
                    content: child,
                }],
            }],
        })),
    )
}

fn document(mut content: RichTextBox, width: u32, density: u32, flow: bool) -> Document {
    if !flow {
        content.bbox = bounds(10.0, 20.0, f64::from(width), 1200.0);
    }
    Document {
        metadata: DocumentMetadata {
            default_page_dimensions: Some((360 * density, 1400 * density)),
            orientation: Some(0),
            media_assets: vec![MediaAsset {
                name: "media/bidi.png".into(), archive_id: None, mime_type: "image/png".into(),
                data: base64::engine::general_purpose::STANDARD.decode(
                    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC"
                ).unwrap(),
            }],
            ..Default::default()
        },
        pages: vec![Page {
            uuid: "vector-bidi-objects".into(), width: width + 96, height: 1400,
            content_bbox: content.bbox,
            background_color: Some(Color { r: 255, g: 255, b: 255 }),
            template: None, background: Default::default(),
            objects: vec![PageElement::TextBox(content).into()],
        }],
    }
}

fn modes(document: &Document) -> [RenderedPage; 2] {
    let layout = sdocx::layout_document(document);
    [
        sdocx::render_layout_page_svg(document, &layout, 0, &Default::default()).unwrap(),
        sdocx::render_layout_page_replay_svg(document, &layout, 0, &Default::default()).unwrap(),
    ]
}

fn source(svg: &str) -> String {
    let xml = roxmltree::Document::parse(svg).unwrap();
    xml.descendants()
        .filter(|node| node.has_tag_name("tspan"))
        .filter_map(|node| node.text())
        .collect()
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

#[derive(Debug)]
struct Glyph {
    text: String,
    x: f64,
    y: f64,
}

fn collect_glyphs(group: &usvg::Group, glyphs: &mut Vec<Glyph>) {
    for node in group.children() {
        match node {
            usvg::Node::Group(group) => collect_glyphs(group, glyphs),
            usvg::Node::Text(text) => {
                for layout in text.layouted() {
                    for glyph in &layout.positioned_glyphs {
                        let transform = text.abs_transform().pre_concat(glyph.transform());
                        glyphs.push(Glyph {
                            text: glyph.text.clone(),
                            x: f64::from(transform.tx),
                            y: f64::from(transform.ty),
                        });
                    }
                }
            }
            _ => {}
        }
    }
}

fn glyphs(page: &RenderedPage) -> Vec<Glyph> {
    let database = sdocx::fonts::FontBook::default().database();
    let options = usvg::Options {
        font_resolver: svg_fonts::font_resolver(&database),
        fontdb: database,
        ..Default::default()
    };
    let tree = usvg::Tree::from_str(&page.svg, &options).unwrap();
    let mut output = Vec::new();
    collect_glyphs(tree.root(), &mut output);
    output
}

fn glyph<'a>(glyphs: &'a [Glyph], character: &str) -> &'a Glyph {
    glyphs
        .iter()
        .find(|glyph| glyph.text == character)
        .unwrap_or_else(|| panic!("missing {character:?}: {glyphs:?}"))
}

fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 0.0001, "{actual} != {expected}");
}

#[test]
fn inline_images_follow_native_visual_order_and_context_specific_margins() {
    for density in [1, 3] {
        for flow in [false, true] {
            for (left, top) in [(0.0, 0.0), (13.0, 25.0), (-31.0, -19.0)] {
                let mut content = text("A\u{202e}B\u{fffc}C\u{202c}D", density);
                content
                    .object_spans
                    .push(image(3, bounds(left, top, 20.0, 12.0)));
                for page in modes(&document(content, 900, density, flow)) {
                    let xml = roxmltree::Document::parse(&page.svg).unwrap();
                    let image = xml
                        .descendants()
                        .find(|node| node.has_tag_name("image"))
                        .unwrap();
                    let actual = glyphs(&page);
                    let origin = glyph(&actual, "A").x;
                    let margin = if flow { 4.0 * f64::from(density) } else { 0.0 };
                    close(point(image).0, origin + 58.64501953125 + margin);
                    close(glyph(&actual, "C").x, origin + 29.35546875);
                    close(
                        glyph(&actual, "B").x,
                        origin + 78.64501953125 + 2.0 * margin,
                    );
                    close(glyph(&actual, "D").x, origin + 106.66015625 + 2.0 * margin);
                    close(point(image).1 + 12.0, glyph(&actual, "A").y);
                    assert_eq!(source(&page.svg), "A\u{202e}BC\u{202c}D");
                    assert!(
                        page.object_diagnostics.is_empty(),
                        "{:?}",
                        page.object_diagnostics
                    );
                    assert!(
                        page.text_diagnostics.is_empty(),
                        "{:?}",
                        page.text_diagnostics
                    );
                }
            }
        }
    }
}

#[test]
fn multiple_images_reverse_visually_while_retaining_logical_child_order() {
    for density in [1, 3] {
        for flow in [false, true] {
            let mut content = text("A\u{202e}B\u{fffc}\u{fffc}C\u{202c}D", density);
            content.object_spans = vec![
                image(3, bounds(13.0, 25.0, 20.0, 12.0)),
                image(4, bounds(-31.0, -19.0, 30.0, 12.0)),
            ];
            for page in modes(&document(content, 900, density, flow)) {
                let xml = roxmltree::Document::parse(&page.svg).unwrap();
                let images = xml
                    .descendants()
                    .filter(|node| node.has_tag_name("image"))
                    .collect::<Vec<_>>();
                assert_eq!(images.len(), 2);
                assert_eq!(
                    images
                        .iter()
                        .map(|node| node.attribute("width").unwrap().parse::<f64>().unwrap())
                        .collect::<Vec<_>>(),
                    [20.0, 30.0]
                );
                let actual = glyphs(&page);
                let origin = glyph(&actual, "A").x;
                let margin = if flow { 4.0 * f64::from(density) } else { 0.0 };
                close(point(images[1]).0, origin + 58.64501953125 + margin);
                close(point(images[0]).0, origin + 88.64501953125 + 3.0 * margin);
                close(
                    glyph(&actual, "B").x,
                    origin + 108.64501953125 + 4.0 * margin,
                );
                close(glyph(&actual, "D").x, origin + 136.66015625 + 4.0 * margin);
                assert_eq!(source(&page.svg), "A\u{202e}BC\u{202c}D");
                assert!(page.object_diagnostics.is_empty());
                assert!(page.text_diagnostics.is_empty());
            }
        }
    }
}

#[test]
fn wrapping_filters_objects_into_the_retained_paragraph_order() {
    let mut content = text("A\u{202e}B\u{fffc}C\u{202c}D", 1);
    content
        .object_spans
        .push(image(3, bounds(-31.0, -19.0, 20.0, 12.0)));
    for page in modes(&document(content, 85, 1, false)) {
        let xml = roxmltree::Document::parse(&page.svg).unwrap();
        let image = xml
            .descendants()
            .find(|node| node.has_tag_name("image"))
            .unwrap();
        let actual = glyphs(&page);
        let first = glyph(&actual, "A");
        close(point(image).0, first.x + 29.35546875);
        close(glyph(&actual, "B").x, first.x + 49.35546875);
        close(glyph(&actual, "C").x, first.x);
        close(glyph(&actual, "D").x, first.x + 29.28955078125);
        close(glyph(&actual, "C").y - first.y, 60.75);
        assert_eq!(source(&page.svg), "A\u{202e}BC\u{202c}D");
        assert!(page.object_diagnostics.is_empty());
        assert!(page.text_diagnostics.is_empty());
    }
}

#[test]
fn justification_expands_visual_spaces_without_expanding_object_margins() {
    for flow in [false, true] {
        let mut content = text("A\u{202e}B \u{fffc}C\u{202c}D", 1);
        content.paragraphs.push(RichTextParagraph {
            kind: RichTextParagraphType::Alignment,
            start_paragraph: 0,
            end_paragraph: 1,
            payload: 3_u32.to_le_bytes().to_vec(),
        });
        content
            .object_spans
            .push(image(4, bounds(-31.0, -19.0, 20.0, 12.0)));
        for page in modes(&document(content, 300, 1, flow)) {
            let xml = roxmltree::Document::parse(&page.svg).unwrap();
            let image = xml
                .descendants()
                .find(|node| node.has_tag_name("image"))
                .unwrap();
            let actual = glyphs(&page);
            let origin = glyph(&actual, "A").x;
            close(glyph(&actual, "C").x, origin + 29.35546875);
            close(
                point(image).0,
                origin + 58.64501953125 + if flow { 4.0 } else { 0.0 },
            );
            close(glyph(&actual, "B").x, origin + 242.4755859375);
            close(glyph(&actual, "D").x, origin + 270.49072265625);
            assert_eq!(source(&page.svg), "A\u{202e}B C\u{202c}D");
            assert!(page.object_diagnostics.is_empty());
            assert!(page.text_diagnostics.is_empty());
        }
    }
}

#[test]
fn nested_code_and_table_frames_keep_zero_inline_margins() {
    for density in [1, 3] {
        for (container, expected_diagnostics) in [
            (
                code as fn(i32, RichTextBox) -> RichTextObjectSpan,
                Vec::new(),
            ),
            (
                table,
                vec![sdocx::ObjectDiagnostic {
                    anchor_utf16: 0,
                    kind: ObjectDiagnosticKind::UnsupportedContent,
                }],
            ),
        ] {
            let mut child = text("A\u{202e}B\u{fffc}C\u{202c}D", density);
            child
                .object_spans
                .push(image(3, bounds(-31.0, -19.0, 20.0, 12.0)));
            let mut parent = text("\u{fffc}", density);
            parent.object_spans.push(container(0, child));
            for flow in [false, true] {
                for page in modes(&document(parent.clone(), 900, density, flow)) {
                    let xml = roxmltree::Document::parse(&page.svg).unwrap();
                    let image = xml
                        .descendants()
                        .find(|node| node.has_tag_name("image"))
                        .unwrap();
                    let actual = glyphs(&page);
                    let origin = glyph(&actual, "A").x;
                    close(point(image).0, origin + 58.64501953125);
                    close(glyph(&actual, "B").x, origin + 78.64501953125);
                    close(glyph(&actual, "C").x, origin + 29.35546875);
                    assert_eq!(source(&page.svg), "A\u{202e}BC\u{202c}D");
                    assert_eq!(page.object_diagnostics, expected_diagnostics);
                    assert!(
                        page.text_diagnostics.is_empty(),
                        "{:?}",
                        page.text_diagnostics
                    );
                }
            }
        }
    }
}

#[test]
fn valid_code_and_table_children_follow_parent_rtl_object_placement() {
    for (container, width, placed_child_x, flow_child_x) in [
        (
            code as fn(i32, RichTextBox) -> RichTextObjectSpan,
            600.0,
            84.64501953125,
            126.64501953125,
        ),
        (table, 601.0, 68.0, 110.0),
    ] {
        for flow in [false, true] {
            let mut parent = text("A\u{202e}B\u{fffc}C\u{202c}D", 1);
            parent.object_spans.push(container(3, text("K", 1)));
            for page in modes(&document(parent, 900, 1, flow)) {
                let actual = glyphs(&page);
                let a = glyph(&actual, "A");
                let b = glyph(&actual, "B");
                let c = glyph(&actual, "C");
                let child = glyph(&actual, "K");
                close(c.x, a.x + 29.35546875);
                close(child.x, if flow { flow_child_x } else { placed_child_x });
                close(
                    b.x,
                    a.x + 58.64501953125 + width + if flow { 8.0 } else { 0.0 },
                );
                close(glyph(&actual, "D").x, b.x + 28.01513671875);
                assert_eq!(source(&page.svg), "A\u{202e}BC\u{202c}DK");
                assert!(
                    page.object_diagnostics.is_empty(),
                    "{:?}",
                    page.object_diagnostics
                );
                assert!(
                    page.text_diagnostics.is_empty(),
                    "{:?}",
                    page.text_diagnostics
                );
            }
        }
    }
}

#[test]
fn rejected_code_retains_its_replacement_and_typed_recovery_diagnostic() {
    for flow in [false, true] {
        let mut parent = text("A\u{202e}B\u{fffc}C\u{202c}D", 1);
        let mut child = text("K", 1);
        child.margins = Some([0.0, -1000.0, 0.0, 0.0]);
        parent.object_spans.push(code(3, child));
        for page in modes(&document(parent, 900, 1, flow)) {
            assert_eq!(source(&page.svg), "A\u{202e}B\u{fffc}C\u{202c}D");
            assert!(
                page.object_diagnostics
                    .iter()
                    .any(|issue| issue.anchor_utf16 == 3
                        && issue.kind == ObjectDiagnosticKind::InvalidBounds)
            );
            let xml = roxmltree::Document::parse(&page.svg).unwrap();
            assert!(
                !xml.descendants()
                    .any(|node| node.attribute("data-sdocx-object") == Some("code-block"))
            );
        }
    }
}

#[cfg(feature = "pdf")]
#[path = "support/pdf_geometry.rs"]
mod pdf_geometry;

#[cfg(feature = "pdf")]
#[test]
fn pdf_preserves_mixed_image_geometry_and_control_source() {
    for flow in [false, true] {
        let mut content = text("A\u{202e}B\u{fffc}C\u{202c}D", 1);
        content
            .object_spans
            .push(image(3, bounds(-31.0, -19.0, 20.0, 12.0)));
        for page in modes(&document(content, 900, 1, flow)) {
            let bytes = sdocx::render_svg_pages_pdf(&[page], &Default::default()).unwrap();
            let pdf = pdf_geometry::read(&bytes, 96.0);
            assert_eq!(pdf.source, "A\u{202e}BC\u{202c}D");
            assert_eq!(
                pdf.extracted_text
                    .chars()
                    .filter(|character| matches!(character, 'A'..='D'))
                    .collect::<String>(),
                "ABCD"
            );
            assert!(pdf.actual_text.iter().any(|text| text.contains('\u{202c}')));
            assert_eq!(pdf.image_resources, 1);
            assert_eq!(pdf.images.len(), 1);
            let origin = if flow { 48.0 } else { 10.0 };
            let margin = if flow { 4.0 } else { 0.0 };
            let baseline = pdf.text.iter().find(|(text, _, _)| text == "A").unwrap().2;
            for (character, x) in [
                ("A", 0.0),
                ("B", 78.64501953125 + 2.0 * margin),
                ("C", 29.35546875),
                ("D", 106.66015625 + 2.0 * margin),
            ] {
                let (_, actual_x, y) = pdf
                    .text
                    .iter()
                    .find(|(text, _, _)| text == character)
                    .unwrap();
                close(*actual_x, origin + x);
                close(*y, baseline);
            }
            let [left, top, right, bottom] = pdf.images[0];
            close(left, origin + 58.64501953125 + margin);
            close(right - left, 20.0);
            close(bottom - top, 12.0);
            close(bottom, baseline);
        }
    }
}

#[cfg(feature = "pdf")]
#[test]
fn pdf_keeps_code_and_table_children_vector_and_in_native_object_order() {
    for (container, width, placed_child_x, flow_child_x) in [
        (
            code as fn(i32, RichTextBox) -> RichTextObjectSpan,
            600.0,
            84.64501953125,
            126.64501953125,
        ),
        (table, 601.0, 68.0, 110.0),
    ] {
        for flow in [false, true] {
            let mut parent = text("A\u{202e}B\u{fffc}C\u{202c}D", 1);
            parent.object_spans.push(container(3, text("K", 1)));
            for page in modes(&document(parent, 900, 1, flow)) {
                let bytes = sdocx::render_svg_pages_pdf(&[page], &Default::default()).unwrap();
                let pdf = pdf_geometry::read(&bytes, 96.0);
                assert_eq!(pdf.source, "A\u{202e}BC\u{202c}DK");
                assert_eq!(
                    pdf.extracted_text
                        .chars()
                        .filter(|character| matches!(character, 'A'..='D' | 'K'))
                        .collect::<String>(),
                    "ABCDK"
                );
                assert!(pdf.actual_text.iter().any(|text| text.contains('\u{202c}')));
                assert_eq!(pdf.image_resources, 0);
                assert!(pdf.images.is_empty());
                let origin = if flow { 48.0 } else { 10.0 };
                let margin = if flow { 4.0 } else { 0.0 };
                for (character, x) in [
                    ("A", origin),
                    ("B", origin + 58.64501953125 + width + 2.0 * margin),
                    ("C", origin + 29.35546875),
                    ("D", origin + 86.66015625 + width + 2.0 * margin),
                    ("K", if flow { flow_child_x } else { placed_child_x }),
                ] {
                    let (_, actual_x, _) = pdf
                        .text
                        .iter()
                        .find(|(text, _, _)| text == character)
                        .unwrap();
                    close(*actual_x, x);
                }
            }
        }
    }
}
