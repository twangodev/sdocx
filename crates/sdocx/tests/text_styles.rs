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

fn font_name(start: u32, end: u32, family: &str) -> RichTextSpan {
    let payload = [
        vec![0; 8],
        u16::try_from(family.len() + 1)
            .unwrap()
            .to_le_bytes()
            .to_vec(),
        family.as_bytes().to_vec(),
        vec![0],
    ]
    .concat();
    span(RichTextSpanType::FontName, start, end, &payload)
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
        metadata: DocumentMetadata {
            default_page_dimensions: Some((1080, 1527)),
            orientation: Some(0),
            ..DocumentMetadata::default()
        },
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
fn unstyled_text_uses_native_defaults_in_each_context() {
    for &context in CONTEXTS {
        let mut content = text("default");
        content.color = None;
        content.font_size = None;
        let svg = render(context, content);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let node = tspan(&xml, "default");
        assert_eq!(node.attribute("fill"), Some("#262626"), "{context:?}");
        assert_eq!(node.attribute("font-size"), Some("51.00"), "{context:?}");
    }
}

#[test]
fn font_size_conversion_preserves_native_minimum_and_large_sizes() {
    for (size, expected) in [
        (0.5_f32, "3.00"),
        (0.0, "3.00"),
        (-2.0, "3.00"),
        (80.0, "240.00"),
    ] {
        for &context in CONTEXTS {
            let mut content = text("size");
            content.font_size = Some(size);
            let svg = render(context, content);
            let xml = roxmltree::Document::parse(&svg).unwrap();
            assert_eq!(
                tspan(&xml, "size").attribute("font-size"),
                Some(expected),
                "{context:?}"
            );
            let mut local = text("size");
            local.spans = vec![span(RichTextSpanType::FontSize, 0, 4, &size.to_le_bytes())];
            let svg = render(context, local);
            let xml = roxmltree::Document::parse(&svg).unwrap();
            assert_eq!(
                tspan(&xml, "size").attribute("font-size"),
                Some(expected),
                "{context:?}"
            );
        }
    }
}

#[test]
fn font_sizes_use_document_density_in_each_text_context() {
    for (dimensions, orientation, default_size, local_size) in [
        (Some((720, 1527)), Some(0), "20.00", "40.00"),
        (Some((1527, 720)), Some(1), "20.00", "40.00"),
        (None, Some(0), "10.00", "20.00"),
    ] {
        let mut content = text("AB");
        content.spans = vec![span(
            RichTextSpanType::FontSize,
            1,
            2,
            &20.0_f32.to_le_bytes(),
        )];
        for &context in CONTEXTS {
            let mut document = document(context, content.clone());
            document.metadata.default_page_dimensions = dimensions;
            document.metadata.orientation = orientation;
            let svg = sdocx::render_page_svg(&document, 0, &Default::default())
                .unwrap()
                .svg;
            let xml = roxmltree::Document::parse(&svg).unwrap();
            for (value, size) in [("A", default_size), ("B", local_size)] {
                assert_eq!(
                    tspan(&xml, value).attribute("font-size"),
                    Some(size),
                    "{context:?}: {dimensions:?}, {orientation:?}, {value}"
                );
            }
        }
    }
}

#[test]
fn body_font_delta_is_applied_before_density_and_native_minimum() {
    for (delta, default_size, local_size) in [
        (2, "24.00", "44.00"),
        (-20, "2.00", "2.00"),
        (i32::MIN, "20.00", "40.00"),
    ] {
        let mut content = text("AB");
        content.spans = vec![span(
            RichTextSpanType::FontSize,
            1,
            2,
            &20.0_f32.to_le_bytes(),
        )];
        for &context in CONTEXTS {
            let mut document = document(context, content.clone());
            document.metadata.default_page_dimensions = Some((720, 1527));
            document.metadata.body_font_size_delta = Some(delta);
            let svg = sdocx::render_page_svg(&document, 0, &Default::default())
                .unwrap()
                .svg;
            let xml = roxmltree::Document::parse(&svg).unwrap();
            for (value, size) in [("A", default_size), ("B", local_size)] {
                assert_eq!(
                    tspan(&xml, value).attribute("font-size"),
                    Some(size),
                    "{context:?}: delta {delta}, {value}"
                );
            }
            assert_eq!(document.metadata.body_font_size_delta, Some(delta));
        }
    }
}

#[test]
fn flow_alignment_uses_native_paragraph_ordinals_after_crlf() {
    let mut content = text("a\r\nb");
    content.paragraphs = vec![sdocx::RichTextParagraph {
        kind: sdocx::RichTextParagraphType::Alignment,
        start_paragraph: 2,
        end_paragraph: 3,
        payload: 1_u32.to_le_bytes().to_vec(),
    }];
    let svg = render(Context::Flow, content);
    let xml = roxmltree::Document::parse(&svg).unwrap();
    let node = tspan(&xml, "b").parent().unwrap();
    assert_eq!(node.attribute("text-anchor"), Some("end"));
    assert_eq!(node.attribute("x"), Some("1752.00"));
    assert_eq!(
        tspan(&xml, "a").parent().unwrap().attribute("text-anchor"),
        Some("start")
    );
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
fn font_name_spans_apply_locally_in_each_text_context() {
    let mut content = text("A😀B C");
    content.spans = vec![
        font_name(1, 3, "Roboto Mono"),
        font_name(3, 4, "sans-serif"),
    ];
    for &context in CONTEXTS {
        let svg = render(context, content.clone());
        let xml = roxmltree::Document::parse(&svg).unwrap();
        for (value, family) in [("😀", "Roboto Mono"), ("B", "sans-serif")] {
            let attribute = tspan(&xml, value)
                .attribute("font-family")
                .unwrap_or_else(|| panic!("{context:?}: missing family on {value:?}"));
            assert_eq!(
                svgtypes::parse_font_families(attribute).unwrap(),
                [
                    svgtypes::FontFamily::Named(family.into()),
                    svgtypes::FontFamily::SansSerif,
                ],
                "{context:?} {value}"
            );
        }
        assert_eq!(tspan(&xml, "A").attribute("font-family"), None);
        assert_eq!(tspan(&xml, " C").attribute("font-family"), None);
    }
}

#[test]
fn font_names_are_one_css_family_and_preserve_raw_source() {
    let family = r#"ACME "Ink", Serif\ <svg onload="boom"> & 'quoted'"#;
    let mut content = text("safe");
    content.spans = vec![font_name(0, 4, family)];
    let original = content.spans[0].payload.clone();
    assert_eq!(content.spans[0].font_name_value(), Some(family));
    for &context in CONTEXTS {
        let document = document(context, content.clone());
        let svg = sdocx::render_page_svg(&document, 0, &Default::default())
            .unwrap()
            .svg;
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let attribute = tspan(&xml, "safe").attribute("font-family").unwrap();
        let expected = r#""ACME \"Ink\", Serif\\ <svg onload=\"boom\"> & 'quoted'", sans-serif"#;
        assert_eq!(attribute, expected, "{context:?}");
        let families = svgtypes::parse_font_families(attribute).unwrap();
        assert!(
            matches!(
                families.as_slice(),
                [
                    svgtypes::FontFamily::Named(_),
                    svgtypes::FontFamily::SansSerif
                ]
            ),
            "{context:?}: {families:?}"
        );
        assert_eq!(
            xml.descendants()
                .filter(|node| node.has_tag_name("svg"))
                .count(),
            1,
            "{context:?}"
        );
        assert!(
            !xml.descendants()
                .any(|node| node.attribute("onload").is_some()),
            "{context:?}"
        );
        assert_eq!(content.spans[0].payload, original, "{context:?}");
        assert_eq!(&original[10..original.len() - 1], family.as_bytes());
    }
}

#[test]
fn mixed_font_sizes_position_each_placed_line_using_its_own_maximum() {
    let mut content = text("aB\nc");
    content.spans = vec![span(
        RichTextSpanType::FontSize,
        1,
        2,
        &20.0_f32.to_le_bytes(),
    )];
    let svg = render(Context::Standalone, content);
    let xml = roxmltree::Document::parse(&svg).unwrap();
    for (value, baseline, size) in [
        ("a", "80.00", "30.00"),
        ("B", "80.00", "60.00"),
        ("c", "131.00", "30.00"),
    ] {
        let node = tspan(&xml, value);
        assert_eq!(node.parent().unwrap().attribute("y"), Some(baseline));
        assert_eq!(node.attribute("font-size"), Some(size));
    }
}

#[test]
fn flow_line_spacing_uses_the_largest_local_size_and_native_spacing_units() {
    for (spacing, next_baseline) in [
        (None, "111.00"),
        (Some((0_u32, 4.0_f32)), "102.00"),
        (Some((1_u32, 1.5_f32)), "120.00"),
    ] {
        let mut content = text("aB\nc");
        content.spans = vec![span(
            RichTextSpanType::FontSize,
            1,
            2,
            &20.0_f32.to_le_bytes(),
        )];
        if let Some((kind, value)) = spacing {
            content.paragraphs.push(sdocx::RichTextParagraph {
                kind: sdocx::RichTextParagraphType::LineSpacing,
                start_paragraph: 0,
                end_paragraph: 2,
                payload: [kind.to_le_bytes(), value.to_le_bytes()].concat(),
            });
        }
        let svg = render(Context::Flow, content);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        for value in ["a", "B"] {
            assert_eq!(
                tspan(&xml, value).parent().unwrap().attribute("y"),
                Some("60.00"),
                "{spacing:?}: {value}"
            );
        }
        assert_eq!(
            tspan(&xml, "c").parent().unwrap().attribute("y"),
            Some(next_baseline),
            "{spacing:?}"
        );
        assert_eq!(tspan(&xml, "c").attribute("font-size"), Some("30.00"));
    }
}

#[test]
fn wrapped_flow_lines_do_not_inherit_a_previous_lines_largest_font_size() {
    let mut content = text("aB c");
    content.spans = vec![span(
        RichTextSpanType::FontSize,
        1,
        2,
        &20.0_f32.to_le_bytes(),
    )];
    let mut document = document(Context::Flow, content);
    document.pages[0].width = 160;
    let svg = sdocx::render_page_svg(&document, 0, &Default::default())
        .unwrap()
        .svg;
    let xml = roxmltree::Document::parse(&svg).unwrap();
    assert_eq!(
        tspan(&xml, "B").parent().unwrap().attribute("y"),
        Some("60.00")
    );
    assert_eq!(
        tspan(&xml, "c").parent().unwrap().attribute("y"),
        Some("111.00")
    );
    assert_eq!(tspan(&xml, "c").attribute("font-size"), Some("30.00"));
}

#[test]
fn wrapping_keeps_spaces_in_selectable_text() {
    let source = "a  b    c  d";
    let mut document = document(Context::Flow, text(source));
    document.pages[0].width = 160;
    let page = sdocx::render_page_svg(&document, 0, &Default::default()).unwrap();
    let xml = roxmltree::Document::parse(&page.svg).unwrap();
    assert!(
        xml.descendants()
            .filter(|node| node.has_tag_name("text"))
            .count()
            > 1
    );
    let rendered = xml
        .descendants()
        .filter(|node| node.has_tag_name("tspan"))
        .filter_map(|node| node.text())
        .collect::<String>();
    assert_eq!(rendered, source);
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
    fonts.load_font_data(include_bytes!("../assets/fonts/Roboto-Regular.ttf").to_vec());
    fonts.load_font_data(include_bytes!("../assets/fonts/Roboto-Italic.ttf").to_vec());
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
