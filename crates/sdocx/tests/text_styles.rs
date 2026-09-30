#![cfg(feature = "render")]

use sdocx::{
    BoundingBox, Color, Document, DocumentMetadata, ObjectSpanLayoutConstraint,
    ObjectSpanLayoutOption, ObjectType, Page, PageElement, RichTextBox, RichTextCodeBlock,
    RichTextObjectContent, RichTextObjectSpan, RichTextRun, RichTextSpan, RichTextSpanType,
};

#[derive(Clone, Copy, Debug)]
enum Context {
    Standalone,
    Flow,
    Code,
    #[cfg(feature = "serde")]
    Table,
}

const CONTEXTS: &[Context] = &[
    Context::Standalone,
    Context::Flow,
    Context::Code,
    #[cfg(feature = "serde")]
    Context::Table,
];

fn bounds() -> BoundingBox {
    BoundingBox {
        x_min: 20.0,
        y_min: 20.0,
        x_max: 1620.0,
        y_max: 420.0,
    }
}

fn text(value: &str) -> RichTextBox {
    RichTextBox {
        text_area_type: None,
        bbox: bounds(),
        rotation_degrees: None,
        text: value.into(),
        color: Some(Color { r: 0, g: 0, b: 0 }),
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

fn span(kind: RichTextSpanType, start: u32, end: u32, payload: &[u8]) -> RichTextSpan {
    RichTextSpan {
        kind,
        start_utf16: start,
        end_utf16: end,
        expand: true,
        payload: payload.into(),
    }
}

fn hyperlink(start: u32, end: u32) -> RichTextSpan {
    let target = "https://example.com/styled";
    let payload = [
        9_u32.to_le_bytes().to_vec(),
        0_u32.to_le_bytes().to_vec(),
        (target.encode_utf16().count() as u32)
            .to_le_bytes()
            .to_vec(),
        target.encode_utf16().flat_map(u16::to_le_bytes).collect(),
    ]
    .concat();
    span(RichTextSpanType::Hyperlink, start, end, &payload)
}

#[cfg(feature = "serde")]
fn table(content: RichTextBox) -> RichTextObjectContent {
    use sdocx::{RichTextTable, RichTextTableCell, RichTextTableRow};
    let style = serde_json::from_str(r#"{
        "heading_column_enabled": false, "heading_row_enabled": false, "max_height_enabled": false,
        "metadata": {"property_mask": [], "field_mask": [], "fixed_trailing_data": [], "flexible_trailing_data": []}
    }"#).unwrap();
    RichTextObjectContent::Table(Box::new(RichTextTable {
        style,
        bbox: bounds(),
        rotation_degrees: None,
        column_widths: vec![1600.0],
        rows: vec![RichTextTableRow {
            max_height: None,
            min_height: None,
            metadata: Default::default(),
            index: 0,
            height: 400.0,
            cells: vec![RichTextTableCell {
                border: None,
                metadata: Default::default(),
                column_index: 0,
                row_span: 1,
                column_span: 1,
                background_color: 0,
                has_own_background_color: false,
                bbox: bounds(),
                vertical_alignment: 0,
                content,
            }],
        }],
    }))
}

fn document(context: Context, mut content: RichTextBox) -> Document {
    let embedded = match context {
        Context::Standalone => None,
        Context::Flow => {
            content.bbox = BoundingBox::default();
            None
        }
        Context::Code => Some((
            ObjectType::CodeBlock,
            RichTextObjectContent::CodeBlock(Box::new(RichTextCodeBlock {
                bbox: bounds(),
                rotation_degrees: None,
                title: None,
                body: Some(content.clone()),
            })),
        )),
        #[cfg(feature = "serde")]
        Context::Table => Some((ObjectType::Table, table(content.clone()))),
    };
    if let Some((object_type, embedded)) = embedded {
        content = text("\u{fffc}");
        content.bbox = BoundingBox::default();
        content.object_spans.push(RichTextObjectSpan {
            object_type,
            object_data: Vec::new(),
            content: Some(embedded),
            text_index_utf16: 0,
            layout_option: ObjectSpanLayoutOption::Block,
            layout_constraint: ObjectSpanLayoutConstraint::Normal,
        });
    }
    Document {
        pages: vec![Page {
            uuid: "styles".into(),
            width: 1800,
            height: 800,
            content_bbox: bounds(),
            background_color: Some(Color {
                r: 255,
                g: 255,
                b: 255,
            }),
            template: None,
            background: Default::default(),
            objects: vec![PageElement::TextBox(content).into()],
        }],
        metadata: DocumentMetadata::default(),
    }
}

fn render(context: Context, content: RichTextBox) -> String {
    sdocx::render_page_svg(&document(context, content), 0, &Default::default())
        .unwrap()
        .svg
}

fn tspan<'a>(xml: &'a roxmltree::Document<'a>, value: &str) -> roxmltree::Node<'a, 'a> {
    xml.descendants()
        .find(|node| node.has_tag_name("tspan") && node.text() == Some(value))
        .unwrap_or_else(|| panic!("missing text span {value:?}"))
}

#[test]
fn all_text_contexts_preserve_mixed_unicode_styles_and_hyperlinks() {
    let mut content = text("A😀B  C");
    content.spans = vec![
        span(RichTextSpanType::ForegroundColor, 1, 3, &[0, 0, 255, 255]),
        span(RichTextSpanType::FontSize, 1, 3, &20.0_f32.to_le_bytes()),
        span(RichTextSpanType::Underline, 1, 3, &[1, 0]),
        span(RichTextSpanType::Strikethrough, 3, 4, &[1, 0]),
        hyperlink(6, 7),
    ];
    for &context in CONTEXTS {
        let svg = render(context, content.clone());
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let values = xml
            .descendants()
            .filter(|node| node.has_tag_name("tspan"))
            .map(|node| node.text().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(values, ["A", "😀", "B", "  ", "C"], "{context:?}");
        for (value, color, size, decoration) in [
            ("A", "#000000", "30.00", None),
            ("😀", "#ff0000", "60.00", Some("underline")),
            ("B", "#000000", "30.00", Some("line-through")),
            ("  ", "#000000", "30.00", None),
            ("C", "#0054ff", "30.00", Some("underline")),
        ] {
            let node = tspan(&xml, value);
            assert_eq!(node.attribute("fill"), Some(color), "{context:?} {value}");
            assert_eq!(
                node.attribute("font-size"),
                Some(size),
                "{context:?} {value}"
            );
            assert_eq!(
                node.attribute("text-decoration"),
                decoration,
                "{context:?} {value}"
            );
        }
        let anchor = tspan(&xml, "C").parent().unwrap();
        assert!(anchor.has_tag_name("a"));
        assert_eq!(anchor.attribute("href"), Some("https://example.com/styled"));
    }
}

#[test]
fn explicit_false_spans_override_legacy_runs_and_prior_true_spans() {
    let mut content = text("abc");
    content.runs = vec![RichTextRun {
        start: 0,
        end: 3,
        bold: true,
        italic: true,
    }];
    for kind in [
        RichTextSpanType::Bold,
        RichTextSpanType::Italic,
        RichTextSpanType::Underline,
        RichTextSpanType::Strikethrough,
    ] {
        content.spans.push(span(kind, 0, 3, &[1, 0]));
        content.spans.push(span(kind, 1, 2, &[0, 0]));
    }
    for &context in CONTEXTS {
        let svg = render(context, content.clone());
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let plain = tspan(&xml, "b");
        for attribute in ["font-weight", "font-style", "text-decoration", "stroke"] {
            assert_eq!(plain.attribute(attribute), None, "{context:?} {attribute}");
        }
        for value in ["a", "c"] {
            let node = tspan(&xml, value);
            assert_eq!(node.attribute("font-style"), Some("italic"), "{context:?}");
            assert_eq!(
                node.attribute("text-decoration"),
                Some("underline line-through"),
                "{context:?}"
            );
            match context {
                Context::Standalone => assert_eq!(node.attribute("font-weight"), Some("bold")),
                #[cfg(feature = "serde")]
                Context::Table => {
                    assert_eq!(node.attribute("font-weight"), None);
                    assert_eq!(node.attribute("stroke"), None);
                }
                _ => {
                    assert_eq!(node.attribute("stroke"), Some("#000000"), "{context:?}");
                    assert_eq!(node.attribute("stroke-width"), Some("0.45"), "{context:?}");
                }
            }
        }
    }
}

#[test]
fn repeated_spaces_are_preserved_in_each_text_context() {
    for &context in CONTEXTS {
        let svg = render(context, text("A  😀   B"));
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let spans = xml
            .descendants()
            .filter(|node| node.has_tag_name("tspan"))
            .collect::<Vec<_>>();
        assert_eq!(
            spans
                .iter()
                .filter_map(|node| node.text())
                .collect::<String>(),
            "A  😀   B",
            "{context:?}"
        );
        for node in spans {
            assert!(
                node.ancestors().any(|parent| parent
                    .attribute(("http://www.w3.org/XML/1998/namespace", "space"))
                    == Some("preserve")),
                "{context:?}"
            );
        }
    }
}

#[test]
fn crlf_offsets_preserve_second_line_unicode_styles() {
    let mut content = text("a\r\n😀b");
    content.spans = vec![
        span(RichTextSpanType::ForegroundColor, 3, 5, &[0, 0, 255, 255]),
        span(RichTextSpanType::FontSize, 3, 5, &20.0_f32.to_le_bytes()),
        span(RichTextSpanType::Underline, 5, 6, &[1, 0]),
    ];
    for context in [Context::Standalone, Context::Flow, Context::Code] {
        let svg = render(context, content.clone());
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let values = xml
            .descendants()
            .filter(|node| node.has_tag_name("tspan"))
            .map(|node| node.text().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(values, ["a", "😀", "b"], "{context:?}");
        assert_eq!(tspan(&xml, "a").attribute("fill"), Some("#000000"));
        assert_eq!(tspan(&xml, "😀").attribute("fill"), Some("#ff0000"));
        assert_eq!(tspan(&xml, "😀").attribute("font-size"), Some("60.00"));
        assert_eq!(tspan(&xml, "b").attribute("fill"), Some("#000000"));
        assert_eq!(
            tspan(&xml, "b").attribute("text-decoration"),
            Some("underline")
        );
    }
}

#[test]
fn spans_with_half_surrogate_boundaries_are_ignored_entirely() {
    let mut content = text("a😀b");
    content.spans = vec![
        span(RichTextSpanType::ForegroundColor, 1, 2, &[0, 0, 255, 255]),
        span(RichTextSpanType::FontSize, 2, 4, &20.0_f32.to_le_bytes()),
        span(RichTextSpanType::Underline, 0, 2, &[1, 0]),
        span(RichTextSpanType::Strikethrough, 2, 4, &[1, 0]),
        hyperlink(1, 2),
    ];
    for &context in CONTEXTS {
        let svg = render(context, content.clone());
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let node = tspan(&xml, "a😀b");
        assert_eq!(node.attribute("fill"), Some("#000000"), "{context:?}");
        assert_eq!(node.attribute("font-size"), Some("30.00"), "{context:?}");
        assert_eq!(node.attribute("text-decoration"), None, "{context:?}");
        assert!(
            !xml.descendants().any(|node| node.has_tag_name("a")),
            "{context:?}"
        );
    }
}

#[cfg(feature = "pdf")]
#[test]
fn mixed_styles_remain_selectable_vector_text_with_bundled_roboto() {
    let mut fonts = sdocx::pdf::fontdb::Database::new();
    fonts.load_font_data(
        include_bytes!("../../../web/static/pdf-fonts/Roboto-Regular.ttf").to_vec(),
    );
    fonts
        .load_font_data(include_bytes!("../../../web/static/pdf-fonts/Roboto-Italic.ttf").to_vec());
    fonts.set_sans_serif_family("Roboto");
    let options = sdocx::PdfOptions::new(std::sync::Arc::new(fonts));
    let mut content = text("Alpha Beta");
    content.spans = vec![
        span(RichTextSpanType::ForegroundColor, 0, 5, &[0, 0, 255, 255]),
        span(RichTextSpanType::FontSize, 0, 5, &14.0_f32.to_le_bytes()),
        span(RichTextSpanType::Italic, 6, 10, &[1, 0]),
        span(RichTextSpanType::Strikethrough, 0, 5, &[1, 0]),
        hyperlink(6, 10),
    ];
    for &context in CONTEXTS {
        let document = document(context, content.clone());
        let rendered = sdocx::render_page_svg(&document, 0, &Default::default()).unwrap();
        let bytes = sdocx::render_svg_pages_pdf(&[rendered], &options).unwrap();
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        let extracted = pdf.extract_text(&[1]).unwrap().replace('\n', "");
        let alpha = extracted
            .find("Alpha")
            .unwrap_or_else(|| panic!("{context:?}: {extracted:?}"));
        let beta = extracted
            .find("Beta")
            .unwrap_or_else(|| panic!("{context:?}: {extracted:?}"));
        assert!(alpha < beta, "{context:?}: {extracted:?}");
        assert!(
            pdf.objects.values().any(|object| object
                .as_dict()
                .is_ok_and(|dict| dict.has(b"FontFile2") || dict.has(b"FontFile3"))),
            "{context:?}"
        );
        assert!(
            !pdf.objects
                .values()
                .any(|object| object.as_stream().is_ok_and(|stream| {
                    stream
                        .dict
                        .get(b"Subtype")
                        .is_ok_and(|value| value.as_name().is_ok_and(|name| name == b"Image"))
                })),
            "{context:?}"
        );
    }
}
