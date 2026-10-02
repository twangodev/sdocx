#![cfg(feature = "render")]

use base64::Engine;
use sdocx::fonts::{FontBook, fontdb};
use sha2::{Digest, Sha256};
use std::sync::Arc;

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

#[cfg(feature = "pdf")]
fn uses_prepared_table_drawing(context: Context) -> bool {
    match context {
        #[cfg(feature = "serde")]
        Context::Table => true,
        _ => false,
    }
}

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
        interval_type: sdocx::SpanIntervalType::from(1),
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
                editable: false,
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

fn assert_decoration(
    xml: &roxmltree::Document<'_>,
    node: roxmltree::Node<'_, '_>,
    expected: Option<&str>,
) {
    if node.attribute("text-decoration") == expected {
        return;
    }
    assert_eq!(node.attribute("text-decoration"), None);
    let offset = match expected.unwrap() {
        "underline" => f64::from(1.0_f32 / 9.0),
        "line-through" => f64::from(-2.0_f32 / 7.0),
        other => panic!("unexpected decoration {other}"),
    };
    let size: f64 = node.attribute("font-size").unwrap().parse().unwrap();
    let x: f64 = node
        .attribute("x")
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap();
    let baseline: f64 = node.attribute("y").unwrap().parse().unwrap();
    assert!(
        xml.descendants().any(|rectangle| {
            if !rectangle.has_tag_name("rect")
                || rectangle.attribute("fill") != node.attribute("fill")
            {
                return false;
            }
            let number = |name| {
                rectangle
                    .attribute(name)
                    .and_then(|value| value.parse::<f64>().ok())
            };
            number("x").is_some_and(|value| (value - x).abs() < 0.0001)
                && number("y")
                    .is_some_and(|value| (value - baseline - size * offset).abs() < 0.0001)
                && number("height").is_some_and(|value| (value - size / 18.0).abs() < 0.0001)
                && number("width").is_some_and(|value| value > 0.0)
        }),
        "missing {expected:?} on {:?}",
        node.text()
    );
}

fn font_css(svg: &str) -> Vec<String> {
    let xml = roxmltree::Document::parse(svg).unwrap();
    xml.descendants()
        .filter(|node| node.has_tag_name("style"))
        .map(|node| node.text().unwrap().to_owned())
        .collect()
}

fn embedded_font_bytes(css: &str) -> Vec<u8> {
    let encoded = css
        .split_once("data:font/ttf;base64,")
        .unwrap()
        .1
        .split_once('"')
        .unwrap()
        .0;
    base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .unwrap()
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
    assert_eq!(node.attribute("text-anchor"), Some("start"));
    assert_eq!(node.attribute("x"), Some("1735.17"));
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
            assert_decoration(&xml, node, decoration);
        }
        let anchor = tspan(&xml, "C").parent().unwrap();
        assert!(anchor.has_tag_name("a"));
        assert_eq!(anchor.attribute("href"), Some("https://example.com/styled"));
    }
}

#[test]
fn hyperlink_actions_match_native_hypertext_flags_in_every_context() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../conformance/table-text-span-identity.json"
    ))
    .unwrap();
    let mut checked = 0;
    for case in fixture["cases"].as_array().unwrap() {
        if !case["name"]
            .as_str()
            .unwrap()
            .starts_with("hyperlink-type-")
        {
            continue;
        }
        let kind = case["right_inputs"][0]["type"].as_u64().unwrap() as u32;
        let enabled = case["right"]["flags"].as_u64().unwrap() & 1 != 0;
        for &context in CONTEXTS {
            let mut content = text("Link");
            let mut link = hyperlink(0, 4);
            link.payload[..4].copy_from_slice(&kind.to_le_bytes());
            content.spans.push(link);
            let svg = render(context, content);
            let xml = roxmltree::Document::parse(&svg).unwrap();
            let node = tspan(&xml, "Link");
            if enabled {
                assert_eq!(node.attribute("fill"), Some("#0054ff"));
                assert_decoration(&xml, node, Some("underline"));
                assert!(node.parent().unwrap().has_tag_name("a"));
            } else {
                assert_eq!(svg, render(context, text("Link")), "{context:?}/{kind}");
                assert!(!xml.descendants().any(|node| node.has_tag_name("a")));
            }
        }
        checked += 1;
    }
    assert_eq!(checked, 5);
}

#[cfg(feature = "pdf")]
#[test]
fn ignored_native_styles_report_the_same_diagnostics_in_svg_and_vector_pdf() {
    use sdocx::TextDiagnosticKind::{
        UnsupportedCompositionStyle, UnsupportedCorrectionStyle, UnsupportedMeasurementStyle,
        UnsupportedSuggestionStyle,
    };
    let fonts = FontBook::default();
    let mut content = text("ABCDE");
    for (index, kind) in [
        RichTextSpanType::ComposingBackgroundColor,
        RichTextSpanType::Composing,
        RichTextSpanType::ComposingTag,
        RichTextSpanType::Suggestion,
        RichTextSpanType::SpellCorrection,
    ]
    .into_iter()
    .enumerate()
    {
        content
            .spans
            .push(span(kind, index as u32, index as u32 + 1, &[]));
    }
    for &context in CONTEXTS {
        let document = document(context, content.clone());
        let layout = sdocx::layout_document(&document);
        let svg = sdocx::render_layout_page_svg_with_fonts(
            &document,
            &layout,
            0,
            &Default::default(),
            &fonts,
        )
        .unwrap();
        let mut expected = vec![
            sdocx::TextDiagnostic {
                kind: UnsupportedCompositionStyle,
                family: "Roboto".into(),
                codepoints: vec![65, 66, 67],
            },
            sdocx::TextDiagnostic {
                kind: UnsupportedSuggestionStyle,
                family: "Roboto".into(),
                codepoints: vec![68],
            },
            sdocx::TextDiagnostic {
                kind: UnsupportedCorrectionStyle,
                family: "Roboto".into(),
                codepoints: vec![69],
            },
            sdocx::TextDiagnostic {
                kind: UnsupportedMeasurementStyle,
                family: "Roboto".into(),
                codepoints: vec![65, 66, 67, 68, 69],
            },
        ];
        if uses_prepared_table_drawing(context) {
            expected.rotate_right(1);
        }
        assert_eq!(svg.text_diagnostics, expected, "{context:?}");
        let pdf = sdocx::pdf::render_layout_pages_pdf_detailed_with_fonts(
            &document,
            &layout,
            &[0],
            &Default::default(),
            &Default::default(),
            &fonts,
        )
        .unwrap();
        assert_eq!(
            pdf.pages[0].text_diagnostics, svg.text_diagnostics,
            "{context:?}"
        );
        let parsed = lopdf::Document::load_mem(&pdf.bytes).unwrap();
        let composition = PdfComposition::read(&pdf.bytes);
        composition.assert_source("ABCDE", &format!("{context:?}"));
        assert_eq!(
            composition.actual_text,
            [b"ABCDE".to_vec()],
            "{context:?}: complete selectable source survives run boundaries"
        );
        assert_eq!(
            parsed.extract_text(&[1]).unwrap().replace('\n', ""),
            "ABCDE",
            "{context:?}: glyph source survives extractor run separators"
        );
    }
}

fn style_documents(content: &RichTextBox) -> Vec<(String, Document)> {
    let documents: Vec<_> = CONTEXTS
        .iter()
        .map(|&context| (format!("{context:?}"), document(context, content.clone())))
        .collect();
    #[cfg(feature = "serde")]
    let documents = {
        let mut documents = documents;
        let mut cell = text("\u{fffc}");
        cell.object_spans.push(RichTextObjectSpan {
            object_type: ObjectType::CodeBlock,
            object_data: Vec::new(),
            content: Some(RichTextObjectContent::CodeBlock(Box::new(
                RichTextCodeBlock {
                    bbox: bounds(),
                    rotation_degrees: None,
                    title: None,
                    body: Some(content.clone()),
                },
            ))),
            text_index_utf16: 0,
            layout_option: ObjectSpanLayoutOption::Block,
            layout_constraint: ObjectSpanLayoutConstraint::Normal,
        });
        documents.push(("NestedTableCode".into(), document(Context::Table, cell)));
        documents
    };
    documents
}

fn modern_composition_flag(kind: RichTextSpanType, start: u32, enabled: bool) -> RichTextSpan {
    span(
        kind,
        start,
        start + 1,
        &[u8::from(enabled), 0xab, 0xcd, 0xef, 0x12, 0x34, 0x56, 0x78],
    )
}

#[test]
fn modern_composition_flags_preserve_native_styling_in_every_text_context() {
    let mut content = text("A B C D");
    content.spans = vec![
        span(
            RichTextSpanType::BackgroundColor,
            6,
            7,
            &0xff34_6578_u32.to_le_bytes(),
        ),
        modern_composition_flag(RichTextSpanType::Composing, 0, false),
        modern_composition_flag(RichTextSpanType::Composing, 2, true),
        modern_composition_flag(RichTextSpanType::ComposingTag, 4, true),
        modern_composition_flag(RichTextSpanType::ComposingTag, 6, false),
    ];
    let fonts = FontBook::default();
    for (context, document) in style_documents(&content) {
        let layout = sdocx::layout_document(&document);
        let rendered = sdocx::render_layout_page_svg_with_fonts(
            &document,
            &layout,
            0,
            &Default::default(),
            &fonts,
        )
        .unwrap();
        assert_eq!(
            rendered.text_diagnostics,
            vec![sdocx::TextDiagnostic {
                kind: sdocx::TextDiagnosticKind::UnsupportedMeasurementStyle,
                family: "Roboto".into(),
                codepoints: vec![u32::from('D')],
            }],
            "{context}",
        );
        let xml = roxmltree::Document::parse(&rendered.svg).unwrap();
        for value in ["A", "B", "D"] {
            assert_decoration(&xml, tspan(&xml, value), Some("underline"));
        }
        let tag_false = tspan(&xml, "D");
        assert_eq!(
            tag_false.attribute("font-style"),
            Some("italic"),
            "{context}"
        );
        assert!(
            tag_false.attribute("font-weight") == Some("bold")
                || tag_false.attribute("stroke").is_some(),
            "{context}: tag=false lost bold"
        );
        let backgrounds: Vec<_> = xml
            .descendants()
            .filter(|node| node.has_tag_name("rect"))
            .collect();
        let tag_background = backgrounds
            .iter()
            .find(|node| node.attribute("fill") == Some("#252525"))
            .unwrap_or_else(|| panic!("{context}: tag=true lost background"));
        let opacity: f64 = tag_background
            .attribute("fill-opacity")
            .unwrap()
            .parse()
            .unwrap();
        assert!(
            (opacity - 25.0 / 255.0).abs() < 0.00001,
            "{context}: {opacity}"
        );
        assert!(
            backgrounds
                .iter()
                .any(|node| node.attribute("fill") == Some("#346578")),
            "{context}: tag=false erased prior background"
        );
        assert_eq!(tspan(&xml, "C").attribute("font-style"), None, "{context}");
        #[cfg(feature = "pdf")]
        {
            let retained = sdocx::pdf::render_layout_pages_pdf_detailed_with_fonts(
                &document,
                &layout,
                &[0],
                &Default::default(),
                &Default::default(),
                &fonts,
            )
            .unwrap();
            assert_eq!(
                retained.pages[0].text_diagnostics, rendered.text_diagnostics,
                "{context}"
            );
            let inspected = PdfComposition::read(&retained.bytes);
            inspected.assert_source("A B C D", &context);
            let colors = inspected.fill_colors;
            for expected in [[37, 37, 37], [52, 101, 120]] {
                assert!(
                    colors.contains(&expected),
                    "{context}: retained PDF lost {expected:?}: {colors:?}"
                );
            }
        }
    }
}

#[cfg(feature = "pdf")]
#[test]
fn composing_background_is_preview_only_and_svg_pdf_preserves_supplied_appearance() {
    let fonts = FontBook::default();
    for composing in [0_u32, 0x0012_3456, 0x8012_3456] {
        let mut content = text("Mark");
        content.spans = vec![
            span(
                RichTextSpanType::BackgroundColor,
                0,
                4,
                &0xff34_6578_u32.to_le_bytes(),
            ),
            span(
                RichTextSpanType::ComposingBackgroundColor,
                0,
                4,
                &[composing.to_le_bytes(), 0xdead_beef_u32.to_le_bytes()].concat(),
            ),
        ];
        for (context, document) in style_documents(&content) {
            let layout = sdocx::layout_document(&document);
            let rendered = sdocx::render_layout_page_svg_with_fonts(
                &document,
                &layout,
                0,
                &Default::default(),
                &fonts,
            )
            .unwrap();
            assert!(
                rendered.text_diagnostics.is_empty(),
                "{context}: {composing:#x}"
            );
            let xml = roxmltree::Document::parse(&rendered.svg).unwrap();
            let has_color = |color| {
                xml.descendants()
                    .any(|node| node.has_tag_name("rect") && node.attribute("fill") == Some(color))
            };
            assert_eq!(
                has_color("#346578"),
                composing == 0,
                "{context}: {composing:#x}"
            );
            assert_eq!(
                has_color("#123456"),
                composing >> 24 != 0,
                "{context}: {composing:#x}"
            );
            if composing >> 24 != 0 {
                let rectangle = xml
                    .descendants()
                    .find(|node| {
                        node.has_tag_name("rect") && node.attribute("fill") == Some("#123456")
                    })
                    .unwrap();
                let opacity: f64 = rectangle
                    .attribute("fill-opacity")
                    .unwrap()
                    .parse()
                    .unwrap();
                assert!(
                    (opacity - 128.0 / 255.0).abs() < 0.00001,
                    "{context}: {opacity}"
                );
            }
            let retained = sdocx::pdf::render_layout_pages_pdf_detailed_with_fonts(
                &document,
                &layout,
                &[0],
                &Default::default(),
                &Default::default(),
                &fonts,
            )
            .unwrap();
            assert!(retained.pages[0].text_diagnostics.is_empty(), "{context}");
            let inspected = PdfComposition::read(&retained.bytes);
            inspected.assert_source("Mark", &context);
            let retained_colors = inspected.fill_colors;
            assert!(
                retained_colors.contains(&[52, 101, 120]),
                "{context}: {retained_colors:?}"
            );
            assert!(
                !retained_colors.contains(&[18, 52, 86]),
                "{context}: {retained_colors:?}"
            );
            let supplied_svg =
                sdocx::render_svg_pages_pdf(&[rendered], &Default::default()).unwrap();
            let inspected = PdfComposition::read(&supplied_svg);
            inspected.assert_source("Mark", &context);
            let svg_colors = inspected.fill_colors;
            assert_eq!(
                svg_colors.contains(&[52, 101, 120]),
                composing == 0,
                "{context}: {composing:#x}: {svg_colors:?}"
            );
            assert_eq!(
                svg_colors.contains(&[18, 52, 86]),
                composing >> 24 != 0,
                "{context}: {composing:#x}: {svg_colors:?}"
            );
        }
    }
}

#[cfg(feature = "pdf")]
#[test]
fn dark_composing_background_selection_tests_the_mapped_argb() {
    let fonts = FontBook::default();
    let mut options = sdocx::RenderOptions::default();
    options.color_mode = sdocx::RenderColorMode::Dark;
    for composing in [0_u32, 0x00ff_ffff] {
        let mut content = text("Mark");
        content.spans = vec![
            span(
                RichTextSpanType::BackgroundColor,
                0,
                4,
                &0xff34_6578_u32.to_le_bytes(),
            ),
            span(
                RichTextSpanType::ComposingBackgroundColor,
                0,
                4,
                &[composing.to_le_bytes(), [0; 4]].concat(),
            ),
        ];
        for (context, document) in style_documents(&content) {
            assert!(
                sdocx::RenderTheme::resolve(
                    &document.pages[0],
                    &document.metadata,
                    options.color_mode
                )
                .is_dark(),
                "{context}"
            );
            let layout = sdocx::layout_document(&document);
            let rendered =
                sdocx::render_layout_page_svg_with_fonts(&document, &layout, 0, &options, &fonts)
                    .unwrap();
            assert!(
                rendered.text_diagnostics.is_empty(),
                "{context}: {composing:#x}"
            );
            let xml = roxmltree::Document::parse(&rendered.svg).unwrap();
            assert_eq!(
                xml.descendants()
                    .any(|node| node.has_tag_name("rect")
                        && node.attribute("fill") == Some("#87b8cb")),
                composing == 0x00ff_ffff,
                "{context}: {composing:#x}"
            );
            let retained = sdocx::pdf::render_layout_pages_pdf_detailed_with_fonts(
                &document,
                &layout,
                &[0],
                &options,
                &Default::default(),
                &fonts,
            )
            .unwrap();
            assert!(retained.pages[0].text_diagnostics.is_empty(), "{context}");
            let retained = PdfComposition::read(&retained.bytes);
            retained.assert_source("Mark", &context);
            assert!(
                retained.fill_colors.contains(&[135, 184, 203]),
                "{context}: {composing:#x}: {:?}",
                retained.fill_colors
            );
            let supplied_svg =
                sdocx::render_svg_pages_pdf(&[rendered], &Default::default()).unwrap();
            let supplied_svg = PdfComposition::read(&supplied_svg);
            supplied_svg.assert_source("Mark", &context);
            assert_eq!(
                supplied_svg.fill_colors.contains(&[135, 184, 203]),
                composing == 0x00ff_ffff,
                "{context}: {composing:#x}: {:?}",
                supplied_svg.fill_colors
            );
        }
    }
}

#[test]
fn object_composition_styles_keep_preview_and_pdf_diagnostics_consistent() {
    let fonts = FontBook::default();
    for layout_option in [
        ObjectSpanLayoutOption::Inline,
        ObjectSpanLayoutOption::Block,
    ] {
        for enabled in [false, true] {
            let mut content = text("\u{fffc}");
            content.spans = vec![modern_composition_flag(
                RichTextSpanType::ComposingTag,
                0,
                enabled,
            )];
            content.object_spans.push(RichTextObjectSpan {
                object_type: ObjectType::CodeBlock,
                object_data: Vec::new(),
                content: Some(RichTextObjectContent::CodeBlock(Box::new(
                    RichTextCodeBlock {
                        bbox: bounds(),
                        rotation_degrees: None,
                        title: None,
                        body: Some(text("Inside")),
                    },
                ))),
                text_index_utf16: 0,
                layout_option,
                layout_constraint: ObjectSpanLayoutConstraint::Normal,
            });
            for (context, document) in style_documents(&content) {
                let layout = sdocx::layout_document(&document);
                let rendered = sdocx::render_layout_page_svg_with_fonts(
                    &document,
                    &layout,
                    0,
                    &Default::default(),
                    &fonts,
                )
                .unwrap();
                let composition_issues: Vec<_> = rendered
                    .text_diagnostics
                    .iter()
                    .filter(|issue| {
                        issue.kind == sdocx::TextDiagnosticKind::UnsupportedCompositionStyle
                    })
                    .collect();
                assert_eq!(
                    composition_issues.len(),
                    0,
                    "{context}: {layout_option:?}: {enabled}: {:?}",
                    rendered.text_diagnostics
                );
                #[cfg(feature = "pdf")]
                {
                    let retained = sdocx::pdf::render_layout_pages_pdf_detailed_with_fonts(
                        &document,
                        &layout,
                        &[0],
                        &Default::default(),
                        &Default::default(),
                        &fonts,
                    )
                    .unwrap();
                    assert_eq!(
                        retained.pages[0].text_diagnostics, rendered.text_diagnostics,
                        "{context}: {layout_option:?}: {enabled}"
                    );
                }
            }
        }
    }
}

#[cfg(feature = "serde")]
fn composition_image_text(source: &str, layout_option: ObjectSpanLayoutOption) -> RichTextBox {
    let mut content = text(source);
    let length = source.encode_utf16().count() as u32;
    let anchor = source
        .chars()
        .position(|character| character == '\u{fffc}')
        .unwrap() as u32;
    let image: sdocx::PlacedImage = serde_json::from_value(serde_json::json!({
        "bbox": {"x_min":0.0,"y_min":0.0,"x_max":30.0,"y_max":40.0},
        "rotation_degrees": null, "media_id": null, "media_index": 0,
        "crop_rect": null, "original_bbox": null,
        "border_media_id": null, "original_media_id": null
    }))
    .unwrap();
    content.object_spans.push(RichTextObjectSpan {
        object_type: ObjectType::Image,
        object_data: Vec::new(),
        content: Some(RichTextObjectContent::Image(Box::new(image))),
        text_index_utf16: anchor as i32,
        layout_option,
        layout_constraint: ObjectSpanLayoutConstraint::Normal,
    });
    content.spans = vec![
        span(
            RichTextSpanType::FontSize,
            0,
            length,
            &1.0_f32.to_le_bytes(),
        ),
        span(
            RichTextSpanType::ForegroundColor,
            0,
            length,
            &0xff34_6578_u32.to_le_bytes(),
        ),
    ];
    if source.starts_with('A') {
        content.spans.push(span(
            RichTextSpanType::BackgroundColor,
            0,
            1,
            &0xff12_3456_u32.to_le_bytes(),
        ));
    }
    content
}

#[cfg(feature = "serde")]
fn add_composition_image_asset(document: &mut Document) {
    document.metadata.media_assets.push(sdocx::MediaAsset {
        name: "media/composition-object.png".into(),
        archive_id: None,
        mime_type: "image/png".into(),
        data: base64::engine::general_purpose::STANDARD.decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC").unwrap(),
    });
}

#[cfg(any(feature = "serde", feature = "pdf"))]
fn svg_style_point(node: roxmltree::Node<'_, '_>) -> [f64; 2] {
    let coordinate = |name| {
        node.ancestors()
            .find_map(|node| node.attribute(name))
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .parse::<f64>()
            .unwrap()
    };
    let mut point = [coordinate("x"), coordinate("y")];
    for ancestor in node.ancestors() {
        if let Some(transform) = ancestor.attribute("transform") {
            let transform: svgtypes::Transform = transform.parse().unwrap();
            point = [
                transform.a * point[0] + transform.c * point[1] + transform.e,
                transform.b * point[0] + transform.d * point[1] + transform.f,
            ];
        }
    }
    point
}

#[cfg(feature = "serde")]
#[test]
fn composing_tag_object_bands_use_the_measured_object_line() {
    let fonts = FontBook::default();
    for source in ["A\u{fffc}B", "\u{fffc}"] {
        for option in [
            ObjectSpanLayoutOption::Inline,
            ObjectSpanLayoutOption::Block,
        ] {
            let baseline = composition_image_text(source, option);
            let anchor = baseline.object_spans[0].text_index_utf16 as u32;
            let mut tagged = baseline.clone();
            tagged.spans.push(modern_composition_flag(
                RichTextSpanType::ComposingTag,
                anchor,
                true,
            ));
            let baselines = style_documents(&baseline);
            for ((context, mut document), (_, mut control_document)) in
                style_documents(&tagged).into_iter().zip(baselines)
            {
                add_composition_image_asset(&mut document);
                add_composition_image_asset(&mut control_document);
                let layout = sdocx::layout_document(&document);
                let rendered = sdocx::render_layout_page_svg_with_fonts(
                    &document,
                    &layout,
                    0,
                    &Default::default(),
                    &fonts,
                )
                .unwrap();
                let control =
                    sdocx::render_page_svg(&control_document, 0, &Default::default()).unwrap();
                let xml = roxmltree::Document::parse(&rendered.svg).unwrap();
                let control_xml = roxmltree::Document::parse(&control.svg).unwrap();
                let band = xml
                    .descendants()
                    .find(|node| {
                        node.has_tag_name("rect") && node.attribute("fill") == Some("#252525")
                    })
                    .unwrap_or_else(|| {
                        panic!("{context}: {source:?}: {option:?}: object band missing")
                    });
                let image = xml
                    .descendants()
                    .find(|node| node.has_tag_name("image"))
                    .unwrap();
                let image_point = svg_style_point(image);
                let band_point = svg_style_point(band);
                let margin = if context == "Flow" && option == ObjectSpanLayoutOption::Inline {
                    12.0
                } else {
                    0.0
                };
                let width: f64 = band.attribute("width").unwrap().parse().unwrap();
                assert!(
                    (band_point[0] - image_point[0] + margin).abs() < 0.0001,
                    "{context}: {source:?}: {option:?}: object band origin"
                );
                assert!(
                    (width - 30.0 - 2.0 * margin).abs() < 0.0001,
                    "{context}: {source:?}: {option:?}: object band width {width}"
                );
                let height: f64 = band.attribute("height").unwrap().parse().unwrap();
                assert!(
                    height >= 40.0
                        && band_point[1] <= image_point[1]
                        && band_point[1] + height >= image_point[1] + 40.0,
                    "{context}: {source:?}: {option:?}: measured line must cover object"
                );
                if source.starts_with('A') && option == ObjectSpanLayoutOption::Inline {
                    let glyph_band = xml
                        .descendants()
                        .find(|node| {
                            node.has_tag_name("rect") && node.attribute("fill") == Some("#123456")
                        })
                        .unwrap();
                    assert!(
                        (band_point[1] - svg_style_point(glyph_band)[1]).abs() < 0.0001,
                        "{context}: object/glyph line top"
                    );
                    assert!(
                        (height
                            - glyph_band
                                .attribute("height")
                                .unwrap()
                                .parse::<f64>()
                                .unwrap())
                        .abs()
                            < 0.0001,
                        "{context}: object/glyph line height"
                    );
                }
                for value in ["A", "B"].into_iter().filter(|_| source.starts_with('A')) {
                    let node = tspan(&xml, value);
                    let control_node = tspan(&control_xml, value);
                    assert_eq!(
                        svg_style_point(node),
                        svg_style_point(control_node),
                        "{context}: {value}: glyph geometry changed"
                    );
                    assert_eq!(
                        node.attribute("fill"),
                        control_node.attribute("fill"),
                        "{context}: {value}: glyph color changed"
                    );
                }
                #[cfg(feature = "pdf")]
                {
                    let retained = sdocx::pdf::render_layout_pages_pdf_detailed_with_fonts(
                        &document,
                        &layout,
                        &[0],
                        &Default::default(),
                        &Default::default(),
                        &fonts,
                    )
                    .unwrap();
                    let pdf = PdfComposition::read(&retained.bytes);
                    pdf.assert_object_background(
                        &[[37; 3]],
                        (context != "Flow").then_some((
                            [37; 3],
                            25.0 / 255.0,
                            [band_point[0], band_point[1], width, height],
                        )),
                        &format!("{context}: {source:?}: {option:?}"),
                    );
                    assert_eq!(
                        pdf.source,
                        if source.starts_with('A') { "AB" } else { "" },
                        "{context}: {source:?}: {option:?}"
                    );
                    assert_eq!(
                        pdf.image_resources, 1,
                        "{context}: embedded image preserved"
                    );
                    assert!(
                        pdf.text_positions
                            .iter()
                            .all(|glyph| glyph.fill_color == [52, 101, 120]),
                        "{context}: {source:?}: {option:?}: glyph color changed"
                    );
                }
            }
        }
    }
}

#[cfg(feature = "serde")]
#[test]
fn mixed_object_backgrounds_survive_transparent_glyph_overwrites_in_widget_contexts() {
    let fonts = FontBook::default();
    let mut content = composition_image_text("A\u{fffc}B", ObjectSpanLayoutOption::Inline);
    content
        .spans
        .retain(|span| span.kind != RichTextSpanType::BackgroundColor);
    let mut tag = modern_composition_flag(RichTextSpanType::ComposingTag, 0, true);
    tag.end_utf16 = 3;
    content.spans.extend([
        tag,
        span(
            RichTextSpanType::BackgroundColor,
            0,
            3,
            &0x0011_2233_u32.to_le_bytes(),
        ),
    ]);
    for (context, mut document) in style_documents(&content) {
        add_composition_image_asset(&mut document);
        let layout = sdocx::layout_document(&document);
        let rendered = sdocx::render_layout_page_svg_with_fonts(
            &document,
            &layout,
            0,
            &Default::default(),
            &fonts,
        )
        .unwrap();
        let xml = roxmltree::Document::parse(&rendered.svg).unwrap();
        let bands: Vec<_> = xml
            .descendants()
            .filter(|node| {
                node.has_tag_name("rect")
                    && matches!(
                        node.attribute("fill"),
                        Some("#112233" | "#123456" | "#252525")
                    )
            })
            .collect();
        let widget = matches!(context.as_str(), "Flow" | "Table");
        assert_eq!(bands.len(), usize::from(widget), "{context}");
        if widget {
            let band = bands[0];
            assert_eq!(band.attribute("fill"), Some("#252525"), "{context}");
            let image = xml
                .descendants()
                .find(|node| node.has_tag_name("image"))
                .unwrap();
            let margin = if context == "Flow" { 12.0 } else { 0.0 };
            let band_point = svg_style_point(band);
            assert!(
                (band_point[0] - svg_style_point(image)[0] + margin).abs() < 0.0001,
                "{context}: surviving object band origin"
            );
            let width: f64 = band.attribute("width").unwrap().parse().unwrap();
            assert!(
                (width - 30.0 - 2.0 * margin).abs() < 0.0001,
                "{context}: surviving object band width"
            );
        }
        for value in ["A", "B"] {
            assert_eq!(
                tspan(&xml, value).attribute("fill"),
                Some("#346578"),
                "{context}"
            );
        }
        #[cfg(feature = "pdf")]
        {
            let retained = sdocx::pdf::render_layout_pages_pdf_detailed_with_fonts(
                &document,
                &layout,
                &[0],
                &Default::default(),
                &Default::default(),
                &fonts,
            )
            .unwrap();
            let pdf = PdfComposition::read(&retained.bytes);
            let expected = (context == "Table").then(|| {
                let band = bands[0];
                let point = svg_style_point(band);
                (
                    [37; 3],
                    25.0 / 255.0,
                    [
                        point[0],
                        point[1],
                        band.attribute("width").unwrap().parse().unwrap(),
                        band.attribute("height").unwrap().parse().unwrap(),
                    ],
                )
            });
            pdf.assert_object_background(
                &[[17, 34, 51], [18, 52, 86], [37; 3]],
                expected,
                &context,
            );
            assert_eq!(pdf.source, "AB", "{context}: selectable source");
            assert_eq!(pdf.image_resources, 1, "{context}: image preserved");
            for glyph in &pdf.text_positions {
                assert_eq!(glyph.fill_color, [52, 101, 120], "{context}");
                let node = tspan(&xml, &glyph.source);
                assert!(
                    (glyph.x - svg_style_point(node)[0]).abs() < 0.002,
                    "{context}: {} glyph advance changed",
                    glyph.source
                );
            }
        }
    }
}

#[cfg(feature = "serde")]
#[test]
fn object_backgrounds_keep_converter_guards_and_span_overwrite_order() {
    let fonts = FontBook::default();
    let ordinary = span(
        RichTextSpanType::BackgroundColor,
        0,
        1,
        &0xff11_2233_u32.to_le_bytes(),
    );
    let composing = span(
        RichTextSpanType::ComposingBackgroundColor,
        0,
        1,
        &[0x8012_3456_u32.to_le_bytes(), [0; 4]].concat(),
    );
    let tag = modern_composition_flag(RichTextSpanType::ComposingTag, 0, true);
    for (name, spans, drawing_color, widget_color, _retained_drawing_color) in [
        (
            "ordinary",
            vec![ordinary.clone()],
            Some("#112233"),
            None,
            Some("#112233"),
        ),
        (
            "composing",
            vec![composing.clone()],
            Some("#123456"),
            None,
            None,
        ),
        (
            "ordinary then tag",
            vec![ordinary.clone(), tag.clone()],
            Some("#252525"),
            Some("#252525"),
            Some("#252525"),
        ),
        (
            "tag then ordinary",
            vec![tag.clone(), ordinary],
            Some("#112233"),
            Some("#252525"),
            Some("#112233"),
        ),
        (
            "composing then tag",
            vec![composing.clone(), tag.clone()],
            Some("#123456"),
            Some("#252525"),
            Some("#252525"),
        ),
        (
            "tag then composing",
            vec![tag, composing],
            Some("#123456"),
            Some("#252525"),
            Some("#252525"),
        ),
    ] {
        let mut content = composition_image_text("\u{fffc}", ObjectSpanLayoutOption::Inline);
        content.spans.extend(spans);
        for (context, mut document) in style_documents(&content) {
            add_composition_image_asset(&mut document);
            let drawing = !matches!(context.as_str(), "Flow" | "Table");
            let expected = if drawing { drawing_color } else { widget_color };
            let layout = sdocx::layout_document(&document);
            let rendered = sdocx::render_layout_page_svg_with_fonts(
                &document,
                &layout,
                0,
                &Default::default(),
                &fonts,
            )
            .unwrap();
            let xml = roxmltree::Document::parse(&rendered.svg).unwrap();
            let bands: Vec<_> = xml
                .descendants()
                .filter(|node| {
                    node.has_tag_name("rect")
                        && matches!(
                            node.attribute("fill"),
                            Some("#112233" | "#123456" | "#252525")
                        )
                })
                .collect();
            assert_eq!(
                bands.len(),
                usize::from(expected.is_some()),
                "{context}: {name}: converter guard"
            );
            if let Some(expected) = expected {
                assert_eq!(
                    bands[0].attribute("fill"),
                    Some(expected),
                    "{context}: {name}: span overwrite order"
                );
                let opacity: f64 = bands[0]
                    .attribute("fill-opacity")
                    .unwrap_or("1")
                    .parse()
                    .unwrap();
                let alpha = match expected {
                    "#123456" => 128.0,
                    "#252525" => 25.0,
                    _ => 255.0,
                };
                assert!(
                    (opacity - alpha / 255.0).abs() < 0.00001,
                    "{context}: {name}: alpha"
                );
            }
            #[cfg(feature = "pdf")]
            {
                let retained = sdocx::pdf::render_layout_pages_pdf_detailed_with_fonts(
                    &document,
                    &layout,
                    &[0],
                    &Default::default(),
                    &Default::default(),
                    &fonts,
                )
                .unwrap();
                let pdf = PdfComposition::read(&retained.bytes);
                let expected = if context == "Flow" {
                    None
                } else if drawing {
                    _retained_drawing_color
                } else {
                    widget_color
                };
                pdf.assert_object_background(
                    &[[17, 34, 51], [18, 52, 86], [37; 3]],
                    expected.map(|color| {
                        let band = bands[0];
                        let point = svg_style_point(band);
                        let (rgb, alpha) = match color {
                            "#112233" => ([17, 34, 51], 1.0),
                            "#252525" => ([37; 3], 25.0 / 255.0),
                            _ => unreachable!(),
                        };
                        (
                            rgb,
                            alpha,
                            [
                                point[0],
                                point[1],
                                band.attribute("width").unwrap().parse().unwrap(),
                                band.attribute("height").unwrap().parse().unwrap(),
                            ],
                        )
                    }),
                    &format!("{context}: {name}"),
                );
                assert_eq!(pdf.source, "", "{context}: {name}: object-only source");
                assert_eq!(
                    pdf.image_resources, 1,
                    "{context}: {name}: embedded image preserved"
                );
            }
        }
    }
}

struct MeasureJoinCase {
    name: &'static str,
    spans: Vec<RichTextSpan>,
    joined: bool,
    dark: bool,
    paint_colors: [&'static str; 2],
}

fn measure_join_cases() -> Vec<MeasureJoinCase> {
    vec![
        MeasureJoinCase {
            name: "unchanged source",
            spans: vec![],
            joined: true,
            dark: false,
            paint_colors: ["#334455"; 2],
        },
        MeasureJoinCase {
            name: "alpha-only foreground",
            spans: vec![span(
                RichTextSpanType::ForegroundColor,
                1,
                2,
                &0x8033_4455_u32.to_le_bytes(),
            )],
            joined: false,
            dark: false,
            paint_colors: ["#334455"; 2],
        },
        MeasureJoinCase {
            name: "null versus empty family",
            spans: vec![font_name(1, 2, "")],
            joined: false,
            dark: false,
            paint_colors: ["#334455"; 2],
        },
        MeasureJoinCase {
            name: "hyperlink paint only",
            spans: vec![hyperlink(1, 2)],
            joined: true,
            dark: false,
            paint_colors: ["#334455", "#0054ff"],
        },
        MeasureJoinCase {
            name: "source foreground beneath hyperlink",
            spans: vec![
                hyperlink(0, 2),
                span(
                    RichTextSpanType::ForegroundColor,
                    1,
                    2,
                    &0xff77_8899_u32.to_le_bytes(),
                ),
            ],
            joined: false,
            dark: false,
            paint_colors: ["#0054ff"; 2],
        },
        MeasureJoinCase {
            name: "native mapped foreground versus display contrast",
            spans: vec![
                span(
                    RichTextSpanType::ForegroundColor,
                    0,
                    1,
                    &0xff26_2626_u32.to_le_bytes(),
                ),
                span(
                    RichTextSpanType::ForegroundColor,
                    1,
                    2,
                    &0xffd9_d9d9_u32.to_le_bytes(),
                ),
            ],
            joined: false,
            dark: true,
            paint_colors: ["#d9d9d9"; 2],
        },
    ]
}

fn measure_join_text(source: &str, case: &MeasureJoinCase) -> RichTextBox {
    let mut content = text(source);
    let length = source.encode_utf16().count() as u32;
    content.spans = vec![
        span(
            RichTextSpanType::FontSize,
            0,
            length,
            &20.0_f32.to_le_bytes(),
        ),
        span(
            RichTextSpanType::ForegroundColor,
            0,
            length,
            &0xff33_4455_u32.to_le_bytes(),
        ),
        span(
            RichTextSpanType::BackgroundColor,
            0,
            2,
            &0xff12_3456_u32.to_le_bytes(),
        ),
    ];
    if source.ends_with('|') {
        content.spans.push(span(
            RichTextSpanType::Underline,
            length - 1,
            length,
            &[1, 0],
        ));
    }
    content.spans.extend(case.spans.clone());
    content
}

fn roboto_advance(source: &str) -> f64 {
    const FONT: &[u8] = include_bytes!("../assets/fonts/Roboto-Regular.ttf");
    assert_eq!(
        format!("{:x}", Sha256::digest(FONT)),
        "56a45233d29f11b4dfb86d248e921939d115778f87325e7ae8cc108383d6664d"
    );
    let face = rustybuzz::Face::from_slice(FONT, 0).unwrap();
    let mut buffer = rustybuzz::UnicodeBuffer::new();
    buffer.push_str(source);
    buffer.guess_segment_properties();
    let shaped = rustybuzz::shape(&face, &[], buffer);
    let units: i32 = shaped
        .glyph_positions()
        .iter()
        .map(|glyph| glyph.x_advance)
        .sum();
    f64::from(units) * 60.0 / f64::from(face.units_per_em())
}

#[test]
fn native_measure_join_identity_preserves_font_advances_across_text_contexts() {
    let fonts = FontBook::default();
    let joined = roboto_advance("AV");
    let separated = roboto_advance("A") + roboto_advance("V");
    assert!(
        separated - joined > 1.0,
        "font must distinguish joined and split AV"
    );
    assert!((roboto_advance("AV|") - roboto_advance("|") - joined).abs() < 0.0001);
    for case in measure_join_cases() {
        let expected = if case.joined { joined } else { separated };
        let mut options = sdocx::RenderOptions::default();
        if case.dark {
            options.color_mode = sdocx::RenderColorMode::Dark;
        }
        for (context, document) in style_documents(&measure_join_text("AV|", &case)) {
            let layout = sdocx::layout_document(&document);
            let rendered =
                sdocx::render_layout_page_svg_with_fonts(&document, &layout, 0, &options, &fonts)
                    .unwrap();
            assert!(
                rendered.text_diagnostics.is_empty(),
                "{context}: {}: {:?}",
                case.name,
                rendered.text_diagnostics
            );
            let xml = roxmltree::Document::parse(&rendered.svg).unwrap();
            assert_eq!(
                xml.descendants()
                    .filter(|node| node.has_tag_name("tspan"))
                    .filter_map(|node| node.text())
                    .collect::<String>(),
                "AV|",
                "{context}: {}",
                case.name
            );
            let background_color = if case.dark { "#a9cbed" } else { "#123456" };
            let background = xml
                .descendants()
                .find(|node| {
                    node.has_tag_name("rect") && node.attribute("fill") == Some(background_color)
                })
                .unwrap();
            let width: f64 = background.attribute("width").unwrap().parse().unwrap();
            assert!(
                (width - expected).abs() < 0.0001,
                "{context}: {}: background width {width} != {expected}",
                case.name
            );
            let first = xml
                .descendants()
                .find(|node| {
                    node.has_tag_name("tspan")
                        && node.text().is_some_and(|text| text.starts_with('A'))
                })
                .unwrap();
            let paint_colors: Vec<_> = xml
                .descendants()
                .filter(|node| node.has_tag_name("tspan"))
                .flat_map(|node| {
                    node.text()
                        .unwrap_or_default()
                        .chars()
                        .filter(|character| matches!(character, 'A' | 'V'))
                        .map(move |_| node.attribute("fill").unwrap())
                })
                .collect();
            assert_eq!(paint_colors, case.paint_colors, "{context}: {}", case.name);
            let x = |node: roxmltree::Node<'_, '_>| {
                node.attribute("x")
                    .unwrap()
                    .split_whitespace()
                    .next()
                    .unwrap()
                    .parse::<f64>()
                    .unwrap()
            };
            assert!(
                (x(tspan(&xml, "|")) - x(first) - expected).abs() < 0.0001,
                "{context}: {}: SVG retained advance",
                case.name
            );
            #[cfg(feature = "pdf")]
            {
                let retained = sdocx::pdf::render_layout_pages_pdf_detailed_with_fonts(
                    &document,
                    &layout,
                    &[0],
                    &options,
                    &Default::default(),
                    &fonts,
                )
                .unwrap();
                let pdf = PdfComposition::read(&retained.bytes);
                pdf.assert_source("AV|", &context);
                let origin = pdf
                    .text_positions
                    .iter()
                    .find(|glyph| glyph.source.starts_with('A'))
                    .unwrap()
                    .x;
                let suffix = pdf
                    .text_positions
                    .iter()
                    .find(|glyph| glyph.source == "|")
                    .unwrap_or_else(|| {
                        panic!(
                            "{context}: {}: missing suffix position: {:?}",
                            case.name, pdf.text_positions
                        )
                    })
                    .x;
                assert!(
                    (suffix - origin - expected).abs() < 0.001,
                    "{context}: {}: PDF retained advance {} != {expected}",
                    case.name,
                    suffix - origin
                );
                let actual_colors: Vec<_> = pdf
                    .text_positions
                    .iter()
                    .flat_map(|glyph| {
                        glyph
                            .source
                            .chars()
                            .filter(|character| matches!(character, 'A' | 'V'))
                            .map(move |_| glyph.fill_color)
                    })
                    .collect();
                let expected_colors = case.paint_colors.map(|color| {
                    std::array::from_fn(|channel| {
                        u8::from_str_radix(&color[1 + channel * 2..3 + channel * 2], 16).unwrap()
                    })
                });
                assert_eq!(
                    actual_colors, expected_colors,
                    "{context}: {}: PDF glyph colors",
                    case.name
                );
            }
        }
    }
}

fn narrow_measure_document(document: &mut Document, width: f64) {
    fn narrow_text(content: &mut RichTextBox, width: f64) {
        if content.bbox != BoundingBox::default() {
            content.bbox.x_max = content.bbox.x_min + width;
        }
        for object in &mut content.object_spans {
            match object.content.as_mut() {
                Some(RichTextObjectContent::CodeBlock(code)) => {
                    code.bbox.x_max = code.bbox.x_min + width + 96.0;
                    if let Some(body) = &mut code.body {
                        narrow_text(body, width);
                    }
                }
                Some(RichTextObjectContent::Table(table)) => {
                    let nested_code = table.rows[0].cells[0]
                        .content
                        .object_spans
                        .iter()
                        .any(|object| object.object_type == ObjectType::CodeBlock);
                    let cell_width = width + if nested_code { 96.0 } else { 0.0 };
                    table.bbox.x_max = table.bbox.x_min + cell_width;
                    table.column_widths = vec![cell_width as f32];
                    table.rows[0].cells[0].bbox.x_max =
                        table.rows[0].cells[0].bbox.x_min + cell_width;
                    narrow_text(&mut table.rows[0].cells[0].content, width);
                }
                _ => {}
            }
        }
    }
    let page = &mut document.pages[0];
    for object in &mut page.objects {
        if let sdocx::PageObjectContent::Element(PageElement::TextBox(content)) =
            &mut object.content
        {
            if content.bbox == BoundingBox::default() && content.object_spans.is_empty() {
                page.width = width as u32 + 96;
            }
            narrow_text(content, width);
        }
    }
}

#[test]
fn native_measure_join_identity_changes_wraps_at_the_actual_font_width() {
    let fonts = FontBook::default();
    let joined = roboto_advance("AV");
    let separated = roboto_advance("A") + roboto_advance("V");
    let width = ((joined + separated) * 0.5).round();
    assert!(joined < width && width < separated);
    for case in measure_join_cases() {
        let mut options = sdocx::RenderOptions::default();
        if case.dark {
            options.color_mode = sdocx::RenderColorMode::Dark;
        }
        for (context, mut document) in style_documents(&measure_join_text("AV", &case)) {
            narrow_measure_document(&mut document, width);
            let layout = sdocx::layout_document(&document);
            let rendered =
                sdocx::render_layout_page_svg_with_fonts(&document, &layout, 0, &options, &fonts)
                    .unwrap();
            let xml = roxmltree::Document::parse(&rendered.svg).unwrap();
            let spans: Vec<_> = xml
                .descendants()
                .filter(|node| node.has_tag_name("tspan"))
                .collect();
            assert_eq!(
                spans
                    .iter()
                    .filter_map(|node| node.text())
                    .collect::<String>(),
                "AV",
                "{context}: {}",
                case.name
            );
            let baselines: std::collections::HashSet<_> = spans
                .iter()
                .map(|node| {
                    node.ancestors()
                        .find_map(|node| node.attribute("y"))
                        .unwrap()
                        .split_whitespace()
                        .next()
                        .unwrap()
                        .parse::<f64>()
                        .unwrap()
                        .to_bits()
                })
                .collect();
            assert_eq!(
                baselines.len(),
                if case.joined { 1 } else { 2 },
                "{context}: {}: width{width} between {joined}/{separated}",
                case.name
            );
            #[cfg(feature = "pdf")]
            {
                let retained = sdocx::pdf::render_layout_pages_pdf_detailed_with_fonts(
                    &document,
                    &layout,
                    &[0],
                    &options,
                    &Default::default(),
                    &fonts,
                )
                .unwrap();
                let pdf = PdfComposition::read(&retained.bytes);
                pdf.assert_source("AV", &context);
                let baselines: std::collections::HashSet<_> = pdf
                    .text_positions
                    .iter()
                    .map(|glyph| (glyph.y * 1000.0).round() as i64)
                    .collect();
                assert_eq!(
                    baselines.len(),
                    if case.joined { 1 } else { 2 },
                    "{context}: {}: PDF wrapping",
                    case.name
                );
            }
        }
    }
}

#[cfg(feature = "pdf")]
#[derive(Default)]
struct PdfComposition {
    source: String,
    actual_text: Vec<Vec<u8>>,
    text_positions: Vec<PdfTextGlyph>,
    fill_colors: Vec<[u8; 3]>,
    rectangles: Vec<PdfRectangle>,
    image_resources: usize,
}

#[cfg(feature = "pdf")]
#[derive(Debug)]
struct PdfTextGlyph {
    source: String,
    x: f64,
    y: f64,
    fill_color: [u8; 3],
}

#[cfg(feature = "pdf")]
#[derive(Debug)]
struct PdfRectangle {
    bounds: [f64; 4],
    fill_color: [u8; 3],
    opacity: f64,
}

#[cfg(feature = "pdf")]
impl PdfComposition {
    fn assert_object_background(
        &self,
        colors: &[[u8; 3]],
        expected: Option<([u8; 3], f64, [f64; 4])>,
        context: &str,
    ) {
        let backgrounds: Vec<_> = self
            .rectangles
            .iter()
            .filter(|rectangle| colors.contains(&rectangle.fill_color))
            .collect();
        assert_eq!(
            backgrounds.len(),
            usize::from(expected.is_some()),
            "{context}: {backgrounds:?}"
        );
        if let Some((color, opacity, bounds)) = expected {
            let rectangle = backgrounds[0];
            assert_eq!(
                rectangle.fill_color, color,
                "{context}: retained ordinary background"
            );
            assert!(
                (rectangle.opacity - opacity).abs() < 0.00001,
                "{context}: {rectangle:?}"
            );
            for (actual, expected) in rectangle.bounds.into_iter().zip(bounds) {
                assert!(
                    (actual - expected).abs() < 0.002,
                    "{context}: {rectangle:?}, expected {bounds:?}"
                );
            }
        }
    }

    fn assert_source(&self, expected: &str, context: &str) {
        assert_eq!(self.source, expected, "{context}");
        assert_eq!(
            self.image_resources, 0,
            "{context}: text exported as an image"
        );
    }

    fn read(bytes: &[u8]) -> Self {
        #[derive(Clone)]
        struct PaintState {
            fill: [u8; 3],
            font: Vec<u8>,
            transform: svgtypes::Transform,
            text_matrix: svgtypes::Transform,
            font_size: f64,
            text_cursor: f64,
            opacity: f64,
        }
        impl Default for PaintState {
            fn default() -> Self {
                Self {
                    fill: [0; 3],
                    font: Vec::new(),
                    transform: svgtypes::Transform::default(),
                    text_matrix: svgtypes::Transform::default(),
                    font_size: 0.0,
                    text_cursor: 0.0,
                    opacity: 1.0,
                }
            }
        }
        #[derive(Default)]
        struct RectanglePath {
            points: Vec<[f64; 2]>,
            subpaths: usize,
            curved: bool,
        }
        impl RectanglePath {
            fn point(&mut self, state: &PaintState, x: f64, y: f64) {
                let matrix = state.transform;
                self.points.push([
                    matrix.a * x + matrix.c * y + matrix.e,
                    matrix.b * x + matrix.d * y + matrix.f,
                ]);
            }
            fn rectangle(&self, state: &PaintState, page_height: f64) -> Option<PdfRectangle> {
                if self.curved || self.subpaths != 1 || self.points.len() != 4 {
                    return None;
                }
                let minimum = |axis| {
                    self.points
                        .iter()
                        .map(|point| point[axis])
                        .fold(f64::INFINITY, f64::min)
                };
                let maximum = |axis| {
                    self.points
                        .iter()
                        .map(|point| point[axis])
                        .fold(f64::NEG_INFINITY, f64::max)
                };
                let (left, bottom, right, top) = (minimum(0), minimum(1), maximum(0), maximum(1));
                let corners: std::collections::HashSet<_> = self
                    .points
                    .iter()
                    .map(|point| (point[0].to_bits(), point[1].to_bits()))
                    .collect();
                if corners.len() != 4
                    || right <= left
                    || top <= bottom
                    || !self.points.iter().all(|point| {
                        (point[0] == left || point[0] == right)
                            && (point[1] == bottom || point[1] == top)
                    })
                {
                    return None;
                }
                Some(PdfRectangle {
                    bounds: [left, page_height - top, right - left, top - bottom]
                        .map(|value| value * 96.0 / 72.0),
                    fill_color: state.fill,
                    opacity: state.opacity,
                })
            }
        }
        fn cid_width(font: &lopdf::Dictionary, cid: u16) -> f64 {
            if let Ok(widths) = font.get(b"W") {
                let widths = widths.as_array().unwrap();
                let mut index = 0;
                while index < widths.len() {
                    let first = widths[index].as_i64().unwrap() as u16;
                    if let Ok(values) = widths[index + 1].as_array() {
                        if let Some(value) = cid
                            .checked_sub(first)
                            .and_then(|index| values.get(usize::from(index)))
                        {
                            return f64::from(value.as_float().unwrap());
                        }
                        index += 2;
                    } else {
                        let last = widths[index + 1].as_i64().unwrap() as u16;
                        if (first..=last).contains(&cid) {
                            return f64::from(widths[index + 2].as_float().unwrap());
                        }
                        index += 3;
                    }
                }
            }
            font.get(b"DW")
                .map_or(1000.0, |value| f64::from(value.as_float().unwrap()))
        }
        fn transform(operands: &[lopdf::Object]) -> svgtypes::Transform {
            let values: Vec<_> = operands
                .iter()
                .map(|value| f64::from(value.as_float().unwrap()))
                .collect();
            svgtypes::Transform::new(
                values[0], values[1], values[2], values[3], values[4], values[5],
            )
        }
        fn compose(left: svgtypes::Transform, right: svgtypes::Transform) -> svgtypes::Transform {
            svgtypes::Transform::new(
                left.a * right.a + left.c * right.b,
                left.b * right.a + left.d * right.b,
                left.a * right.c + left.c * right.d,
                left.b * right.c + left.d * right.d,
                left.a * right.e + left.c * right.f + left.e,
                left.b * right.e + left.d * right.f + left.f,
            )
        }
        fn dictionary<'a>(
            pdf: &'a lopdf::Document,
            value: &'a lopdf::Object,
        ) -> &'a lopdf::Dictionary {
            value
                .as_reference()
                .map(|reference| pdf.get_dictionary(reference).unwrap())
                .unwrap_or_else(|_| value.as_dict().unwrap())
        }
        fn visit(
            pdf: &lopdf::Document,
            data: &[u8],
            resources: &lopdf::Dictionary,
            mut state: PaintState,
            page_height: f64,
            result: &mut PdfComposition,
        ) {
            let mut stack = Vec::new();
            let mut path = RectanglePath::default();
            for operation in lopdf::content::Content::decode(data).unwrap().operations {
                match operation.operator.as_str() {
                    "BDC" => {
                        if let Some(bytes) = operation
                            .operands
                            .get(1)
                            .and_then(|value| value.as_dict().ok())
                            .and_then(|properties| properties.get(b"ActualText").ok())
                            .and_then(|value| value.as_str().ok())
                        {
                            result.actual_text.push(bytes.to_vec());
                        }
                    }
                    "q" => stack.push(state.clone()),
                    "Q" => state = stack.pop().unwrap(),
                    "gs" => {
                        let states = dictionary(pdf, resources.get(b"ExtGState").unwrap());
                        let attributes = dictionary(
                            pdf,
                            states
                                .get(operation.operands[0].as_name().unwrap())
                                .unwrap(),
                        );
                        if let Ok(alpha) = attributes.get(b"ca") {
                            state.opacity = f64::from(alpha.as_float().unwrap());
                        }
                    }
                    "m" | "l" => {
                        if operation.operator == "m" {
                            path.subpaths += 1;
                        }
                        path.point(
                            &state,
                            f64::from(operation.operands[0].as_float().unwrap()),
                            f64::from(operation.operands[1].as_float().unwrap()),
                        );
                    }
                    "re" => {
                        path.subpaths += 1;
                        let values: Vec<_> = operation
                            .operands
                            .iter()
                            .map(|value| f64::from(value.as_float().unwrap()))
                            .collect();
                        let [x, y, width, height] = values.as_slice() else {
                            unreachable!()
                        };
                        for (x, y) in [
                            (*x, *y),
                            (x + width, *y),
                            (x + width, y + height),
                            (*x, y + height),
                        ] {
                            path.point(&state, x, y);
                        }
                    }
                    "c" | "v" | "y" => path.curved = true,
                    "n" | "S" | "s" => path = RectanglePath::default(),
                    "cm" => {
                        state.transform = compose(state.transform, transform(&operation.operands))
                    }
                    "Tm" => {
                        state.text_matrix = transform(&operation.operands);
                        state.text_cursor = 0.0;
                    }
                    "rg" | "scn" | "g" => {
                        let values: Vec<_> = operation
                            .operands
                            .iter()
                            .filter_map(|value| value.as_float().ok())
                            .collect();
                        match values.as_slice() {
                            [gray] => state.fill = [(*gray * 255.0).round() as u8; 3],
                            [red, green, blue] => {
                                state.fill =
                                    [*red, *green, *blue].map(|value| (value * 255.0).round() as u8)
                            }
                            _ => {}
                        }
                    }
                    "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" => {
                        result.fill_colors.push(state.fill);
                        if let Some(rectangle) = path.rectangle(&state, page_height) {
                            result.rectangles.push(rectangle);
                        }
                        path = RectanglePath::default();
                    }
                    "Tf" => {
                        state.font = operation.operands[0].as_name().unwrap().to_vec();
                        state.font_size = f64::from(operation.operands[1].as_float().unwrap());
                    }
                    "Tj" | "TJ" => {
                        let fonts = dictionary(pdf, resources.get(b"Font").unwrap());
                        let font = dictionary(pdf, fonts.get(&state.font).unwrap());
                        let encoding = font.get_font_encoding(pdf).unwrap();
                        let descendant = dictionary(
                            pdf,
                            &font.get(b"DescendantFonts").unwrap().as_array().unwrap()[0],
                        );
                        let values = operation.operands[0]
                            .as_array()
                            .map(Vec::as_slice)
                            .unwrap_or(&operation.operands);
                        for value in values {
                            if let lopdf::Object::String(bytes, _) = value {
                                let (cids, remainder) = bytes.as_chunks::<2>();
                                assert!(remainder.is_empty());
                                for bytes in cids {
                                    let source =
                                        lopdf::Document::decode_text(&encoding, bytes).unwrap();
                                    let matrix = compose(state.transform, state.text_matrix);
                                    result.text_positions.push(PdfTextGlyph {
                                        source: source.clone(),
                                        x: (matrix.e + matrix.a * state.text_cursor) * 96.0 / 72.0,
                                        y: (matrix.f + matrix.b * state.text_cursor) * 96.0 / 72.0,
                                        fill_color: state.fill,
                                    });
                                    result.source.push_str(&source);
                                    state.text_cursor +=
                                        cid_width(descendant, u16::from_be_bytes(*bytes))
                                            * state.font_size
                                            / 1000.0;
                                }
                            } else {
                                state.text_cursor -=
                                    f64::from(value.as_float().unwrap()) * state.font_size / 1000.0;
                            }
                        }
                    }
                    "Do" => {
                        let objects = dictionary(pdf, resources.get(b"XObject").unwrap());
                        let stream = pdf
                            .get_object(
                                objects
                                    .get(operation.operands[0].as_name().unwrap())
                                    .unwrap()
                                    .as_reference()
                                    .unwrap(),
                            )
                            .unwrap()
                            .as_stream()
                            .unwrap();
                        if stream.dict.get(b"Subtype").unwrap().as_name().unwrap() == b"Form" {
                            let child_resources = stream
                                .dict
                                .get(b"Resources")
                                .map(|value| dictionary(pdf, value))
                                .unwrap_or(resources);
                            visit(
                                pdf,
                                &stream.decompressed_content().unwrap(),
                                child_resources,
                                state.clone(),
                                page_height,
                                result,
                            );
                        }
                    }
                    _ => {}
                }
            }
        }
        let pdf = lopdf::Document::load_mem(bytes).unwrap();
        let mut result = Self {
            image_resources: pdf
                .objects
                .values()
                .filter(|object| {
                    object.as_stream().is_ok_and(|stream| {
                        stream
                            .dict
                            .get(b"Subtype")
                            .is_ok_and(|value| value.as_name().is_ok_and(|name| name == b"Image"))
                    })
                })
                .count(),
            ..Self::default()
        };
        let page = pdf.get_pages()[&1];
        let page_height = f64::from(
            pdf.get_dictionary(page)
                .unwrap()
                .get(b"MediaBox")
                .unwrap()
                .as_array()
                .unwrap()[3]
                .as_float()
                .unwrap(),
        );
        let resources = dictionary(
            &pdf,
            pdf.get_dictionary(page).unwrap().get(b"Resources").unwrap(),
        );
        visit(
            &pdf,
            &pdf.get_page_content(page).unwrap(),
            resources,
            PaintState::default(),
            page_height,
            &mut result,
        );
        result
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
            assert_eq!(node.attribute("text-decoration"), None, "{context:?}");
            match context {
                Context::Standalone => assert_eq!(node.attribute("font-weight"), Some("bold")),
                _ => {
                    assert_eq!(node.attribute("font-weight"), None, "{context:?}");
                    assert_eq!(node.attribute("stroke"), Some("#000000"), "{context:?}");
                    assert_eq!(node.attribute("stroke-width"), Some("0.45"), "{context:?}");
                }
            }
        }
        let position = |value| {
            tspan(&xml, value)
                .attribute("x")
                .unwrap()
                .split_whitespace()
                .next()
                .unwrap()
                .parse::<f64>()
                .unwrap()
        };
        let plain_start = position("b");
        let plain_end = position("c");
        let decorations = xml
            .descendants()
            .filter(|node| {
                node.has_tag_name("rect")
                    && node.attribute("fill") == Some("#000000")
                    && node
                        .attribute("fill-opacity")
                        .is_none_or(|opacity| opacity.parse::<f64>().unwrap() > 0.0)
            })
            .collect::<Vec<_>>();
        assert_eq!(decorations.len(), 4, "{context:?}");
        let mut before_plain = 0;
        let mut after_plain = 0;
        for rectangle in decorations {
            let start = rectangle.attribute("x").unwrap().parse::<f64>().unwrap();
            let width = rectangle
                .attribute("width")
                .unwrap()
                .parse::<f64>()
                .unwrap();
            assert!(width > 0.0, "{context:?}");
            if start < plain_start {
                assert!(
                    start + width <= plain_start,
                    "{context:?}: decoration crosses plain b"
                );
                before_plain += 1;
            } else {
                assert!(
                    start >= plain_end,
                    "{context:?}: decoration crosses plain b"
                );
                after_plain += 1;
            }
        }
        assert_eq!((before_plain, after_plain), (2, 2), "{context:?}");
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
        for (value, family) in [("😀", "Roboto Mono"), ("B", "Roboto")] {
            let attribute = tspan(&xml, value)
                .attribute("font-family")
                .unwrap_or_else(|| panic!("{context:?}: missing family on {value:?}"));
            assert_eq!(
                svgtypes::parse_font_families(attribute).unwrap(),
                [
                    svgtypes::FontFamily::Named(
                        FontBook::default()
                            .resolve(family, false, false)
                            .unwrap()
                            .svg_family()
                            .to_string()
                    ),
                    svgtypes::FontFamily::SansSerif,
                ],
                "{context:?} {value}"
            );
        }
        let regular = FontBook::default().resolve("Roboto", false, false).unwrap();
        for value in ["A", " C"] {
            assert_eq!(
                svgtypes::parse_font_families(tspan(&xml, value).attribute("font-family").unwrap())
                    .unwrap(),
                [
                    svgtypes::FontFamily::Named(regular.svg_family().to_string()),
                    svgtypes::FontFamily::SansSerif
                ],
            );
        }
    }
}

#[test]
fn native_cesu8_font_names_reach_font_resolution_in_every_context() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../conformance/table-text-span-binary.json"
    ))
    .unwrap();
    let case = fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == "font-cesu8-v8")
        .unwrap();
    let output = case["output"]
        .as_array()
        .unwrap()
        .iter()
        .map(|byte| byte.as_u64().unwrap() as u8)
        .collect::<Vec<_>>();
    let family = case["decoded"]["name"].as_str().unwrap();
    let mut content = text("ABC");
    content
        .spans
        .push(span(RichTextSpanType::FontName, 0, 1, &output[16..]));
    for &context in CONTEXTS {
        let document = document(context, content.clone());
        let page = sdocx::render_page_svg(&document, 0, &Default::default()).unwrap();
        assert!(
            page.text_diagnostics.iter().any(|issue| {
                issue.kind == sdocx::TextDiagnosticKind::UnavailableFamily
                    && issue.family == family
                    && issue.codepoints.is_empty()
            }),
            "{context:?}: {:?}",
            page.text_diagnostics
        );
        assert_eq!(content.spans[0].payload, output[16..]);
    }
}

#[test]
fn font_names_are_one_css_family_and_preserve_raw_source() {
    let family = r#"ACME "Ink", Serif\ <svg onload="boom"> & 'quoted'"#;
    let defaults = FontBook::default();
    let regular = defaults.resolve("Roboto", false, false).unwrap();
    let mut alias = defaults.database().face(regular.id).unwrap().clone();
    alias.id = fontdb::ID::dummy();
    alias.families[0].0 = family.into();
    alias.families.truncate(1);
    let mut database = fontdb::Database::new();
    database.push_face_info(alias);
    database.set_sans_serif_family(family);
    let fonts = FontBook::new(Arc::new(database));
    let mut content = text("safe");
    content.spans = vec![font_name(0, 4, family)];
    let original = content.spans[0].payload.clone();
    assert_eq!(content.spans[0].font_name_value(), Some(family));
    for &context in CONTEXTS {
        let document = document(context, content.clone());
        let page = sdocx::render_document_svg_with_fonts(&document, &Default::default(), &fonts)
            .pop()
            .unwrap();
        assert_eq!(
            page.text_diagnostics,
            vec![sdocx::TextDiagnostic {
                kind: sdocx::TextDiagnosticKind::UnsupportedMeasurementFont,
                family: family.into(),
                codepoints: vec![97, 101, 102, 115],
            }],
            "{context:?}",
        );
        let svg = page.svg;
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let attribute = tspan(&xml, "safe").attribute("font-family").unwrap();
        let selected = fonts.resolve(family, false, false).unwrap();
        assert_eq!(
            attribute,
            format!("\"{}\", sans-serif", selected.svg_family()),
            "{context:?}"
        );
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
fn svg_embeds_only_used_pinned_faces_and_replay_has_the_same_font_css() {
    let fonts = FontBook::default();
    assert_eq!(fonts.database().faces().count(), 8);
    let face = fonts.resolve("Roboto", false, false).unwrap();
    for &context in CONTEXTS {
        let document = document(context, text("Used font"));
        let layout = sdocx::layout_document(&document);
        let normal = sdocx::render_layout_page_svg_with_fonts(
            &document,
            &layout,
            0,
            &Default::default(),
            &fonts,
        )
        .unwrap();
        let replay = sdocx::render_layout_page_replay_svg_with_fonts(
            &document,
            &layout,
            0,
            &Default::default(),
            &fonts,
        )
        .unwrap();
        assert!(normal.text_diagnostics.is_empty(), "{context:?}");
        assert_eq!(normal.text_diagnostics, replay.text_diagnostics);
        let css = font_css(&normal.svg);
        assert_eq!(css, font_css(&replay.svg), "{context:?}");
        assert_eq!(css.len(), 1, "{context:?}: unused faces must stay absent");
        assert_eq!(css[0].matches("@font-face").count(), 1);
        assert!(css[0].contains(&format!(
            "font-family:\"{}\";font-weight:400;font-style:normal;",
            face.svg_family()
        )));
        let bytes = embedded_font_bytes(&css[0]);
        assert_eq!(bytes, face.bytes(), "{context:?}");
        assert_eq!(
            format!("{:x}", Sha256::digest(&bytes)),
            "56a45233d29f11b4dfb86d248e921939d115778f87325e7ae8cc108383d6664d"
        );
    }
}

#[test]
fn empty_caller_database_reports_unavailable_family_and_keeps_text() {
    let family = "Unavailable in empty database";
    let fonts = FontBook::new(Arc::new(fontdb::Database::new()));
    let mut content = text("empty");
    content.spans = vec![font_name(0, 5, family)];
    for &context in CONTEXTS {
        let document = document(context, content.clone());
        let page = sdocx::render_document_svg_with_fonts(&document, &Default::default(), &fonts)
            .pop()
            .unwrap();
        let layout = sdocx::layout_document(&document);
        let replay = sdocx::render_layout_page_replay_svg_with_fonts(
            &document,
            &layout,
            0,
            &Default::default(),
            &fonts,
        )
        .unwrap();
        let expected = vec![
            sdocx::TextDiagnostic {
                kind: sdocx::TextDiagnosticKind::UnsupportedMeasurementFont,
                family: family.into(),
                codepoints: vec![101, 109, 112, 116, 121],
            },
            sdocx::TextDiagnostic {
                kind: sdocx::TextDiagnosticKind::UnavailableFamily,
                family: family.into(),
                codepoints: Vec::new(),
            },
            sdocx::TextDiagnostic {
                kind: sdocx::TextDiagnosticKind::MeasurementFailure,
                family: family.into(),
                codepoints: Vec::new(),
            },
        ];
        assert_eq!(page.text_diagnostics, expected, "{context:?}");
        assert_eq!(
            page.text_diagnostics, replay.text_diagnostics,
            "{context:?}"
        );
        assert!(font_css(&page.svg).is_empty(), "{context:?}");
        let xml = roxmltree::Document::parse(&page.svg).unwrap();
        assert_eq!(tspan(&xml, "empty").text(), Some("empty"));
    }
}

#[test]
fn invalid_caller_font_data_is_not_silently_replaced_with_fallback_font() {
    let family = "Damaged Native Typeface";
    let defaults = FontBook::default();
    let regular = defaults.resolve("Roboto", false, false).unwrap();
    let mut database = defaults.database().as_ref().clone();
    let mut invalid = database.face(regular.id).unwrap().clone();
    invalid.id = fontdb::ID::dummy();
    invalid.families[0].0 = family.into();
    invalid.families.truncate(1);
    invalid.source = fontdb::Source::Binary(Arc::new(vec![0, 1, 2, 3]));
    database.push_face_info(invalid);
    let fonts = FontBook::new(Arc::new(database));
    let mut content = text("damaged");
    content.spans = vec![font_name(0, 7, family)];
    for &context in CONTEXTS {
        let document = document(context, content.clone());
        let page = sdocx::render_document_svg_with_fonts(&document, &Default::default(), &fonts)
            .pop()
            .unwrap();
        let layout = sdocx::layout_document(&document);
        let replay = sdocx::render_layout_page_replay_svg_with_fonts(
            &document,
            &layout,
            0,
            &Default::default(),
            &fonts,
        )
        .unwrap();
        let expected = vec![
            sdocx::TextDiagnostic {
                kind: sdocx::TextDiagnosticKind::UnsupportedMeasurementFont,
                family: family.into(),
                codepoints: vec![97, 100, 101, 103, 109],
            },
            sdocx::TextDiagnostic {
                kind: sdocx::TextDiagnosticKind::UnusableFontData,
                family: family.into(),
                codepoints: Vec::new(),
            },
            sdocx::TextDiagnostic {
                kind: sdocx::TextDiagnosticKind::MeasurementFailure,
                family: family.into(),
                codepoints: Vec::new(),
            },
        ];
        assert_eq!(page.text_diagnostics, expected, "{context:?}");
        assert_eq!(
            page.text_diagnostics, replay.text_diagnostics,
            "{context:?}"
        );
        assert!(font_css(&page.svg).is_empty(), "{context:?}");
        let xml = roxmltree::Document::parse(&page.svg).unwrap();
        assert_eq!(tspan(&xml, "damaged").text(), Some("damaged"));
    }
}

#[test]
fn embedded_faces_match_mixed_native_weight_and_slant_selection() {
    let mut content = text("abcd");
    content.spans = vec![
        span(RichTextSpanType::Bold, 1, 2, &[1, 0]),
        span(RichTextSpanType::Italic, 2, 3, &[1, 0]),
        span(RichTextSpanType::Bold, 3, 4, &[1, 0]),
        span(RichTextSpanType::Italic, 3, 4, &[1, 0]),
    ];
    for &context in CONTEXTS {
        let document = document(context, content.clone());
        let page = sdocx::render_page_svg(&document, 0, &Default::default()).unwrap();
        assert_eq!(
            page.text_diagnostics,
            vec![sdocx::TextDiagnostic {
                kind: sdocx::TextDiagnosticKind::UnsupportedMeasurementStyle,
                family: "Roboto".into(),
                codepoints: vec![99, 100],
            }],
            "{context:?}",
        );
        let css = font_css(&page.svg);
        let placed = matches!(context, Context::Standalone);
        assert_eq!(css.len(), if placed { 3 } else { 2 }, "{context:?}");
        for (weight, slant, hash) in [
            (
                400,
                "normal",
                "56a45233d29f11b4dfb86d248e921939d115778f87325e7ae8cc108383d6664d",
            ),
            (
                400,
                "italic",
                "fa0b17bb4aaac4a1b2ee149dd4ca3b55e97d3077aa6ba9bb02541b316e7c46ce",
            ),
            (
                700,
                "normal",
                "61f89f8db49261c2f6106e8dccc35df7b2f7ed909020db40a3fc905e95f99334",
            ),
            (
                700,
                "italic",
                "40083ed54338397cf49d2c49f59eddcd963a30fdb301813d4bd3abbb37a13d12",
            ),
        ] {
            let descriptor = format!("font-weight:{weight};font-style:{slant};");
            let selected = css.iter().find(|style| style.contains(&descriptor));
            if weight == 700 && (!placed || slant == "normal") {
                assert!(
                    selected.is_none(),
                    "{context:?}: synthetic bold keeps regular faces"
                );
            } else {
                let bytes = embedded_font_bytes(selected.unwrap());
                assert_eq!(
                    format!("{:x}", Sha256::digest(bytes)),
                    hash,
                    "{context:?}: {descriptor}"
                );
            }
        }
    }
}

#[test]
fn coverage_fallback_preserves_requested_synthetic_italic_and_bold() {
    let defaults = FontBook::default();
    let mut database = fontdb::Database::new();
    for (family, italic) in [("Roboto", true), ("Roboto Mono", false)] {
        let face = defaults.resolve(family, false, italic).unwrap();
        database.load_font_data(face.bytes().to_vec());
    }
    database.set_sans_serif_family("Roboto");
    let fonts = FontBook::new(Arc::new(database));
    let mut content = text("A∕B");
    content.spans = vec![
        font_name(0, 3, "Roboto"),
        span(RichTextSpanType::Italic, 0, 3, &[1, 0]),
        span(RichTextSpanType::Bold, 0, 3, &[1, 0]),
    ];
    for &context in CONTEXTS {
        let document = document(context, content.clone());
        let page = sdocx::render_document_svg_with_fonts(&document, &Default::default(), &fonts)
            .pop()
            .unwrap();
        let xml = roxmltree::Document::parse(&page.svg).unwrap();
        let fallback = tspan(&xml, "∕");
        assert_eq!(
            fallback.attribute("font-family"),
            Some(
                format!(
                    "\"{}\", sans-serif",
                    fonts
                        .resolve("Roboto Mono", false, false)
                        .unwrap()
                        .svg_family()
                )
                .as_str()
            )
        );
        assert_eq!(fallback.attribute("font-style"), Some("italic"));
        if matches!(context, Context::Standalone) {
            assert_eq!(fallback.attribute("font-weight"), Some("bold"));
        } else {
            assert_eq!(fallback.attribute("font-weight"), Some("400"));
            assert_eq!(fallback.attribute("stroke-width"), Some("0.45"));
        }
        assert_eq!(
            page.text_diagnostics,
            vec![sdocx::TextDiagnostic {
                kind: sdocx::TextDiagnosticKind::UnsupportedMeasurementStyle,
                family: "Roboto".into(),
                codepoints: vec![65, 66, 8725],
            }],
            "{context:?}",
        );
        assert_eq!(
            xml.descendants()
                .filter(|node| node.has_tag_name("tspan"))
                .filter_map(|node| node.text())
                .collect::<String>(),
            "A∕B"
        );
    }
}

#[test]
fn complex_svg_fallback_keeps_synthesis_local_to_the_requested_span() {
    let mut database = fontdb::Database::new();
    database.load_font_data(include_bytes!("assets/fonts/DejaVuSans.ttf").to_vec());
    let fonts = FontBook::new(Arc::new(database));
    let mut content = text("لالا");
    content.spans = vec![
        font_name(0, 4, "DejaVu Sans"),
        span(RichTextSpanType::Italic, 0, 2, &[1, 0]),
        span(RichTextSpanType::Bold, 0, 2, &[1, 0]),
    ];
    for &context in CONTEXTS {
        let document = document(context, content.clone());
        let page = sdocx::render_document_svg_with_fonts(&document, &Default::default(), &fonts)
            .pop()
            .unwrap();
        let xml = roxmltree::Document::parse(&page.svg).unwrap();
        let spans: Vec<_> = xml
            .descendants()
            .filter(|node| node.has_tag_name("tspan"))
            .collect();
        assert_eq!(
            spans
                .iter()
                .filter_map(|node| node.text())
                .collect::<String>(),
            "لالا"
        );
        assert_eq!(spans.len(), 2, "{context:?}: {}", page.svg);
        assert_eq!(spans[0].attribute("font-style"), Some("italic"));
        assert_ne!(
            spans[1].attribute("font-style"),
            Some("italic"),
            "{context:?}"
        );
        assert_ne!(
            spans[1].attribute("font-weight"),
            Some("bold"),
            "{context:?}"
        );
        let expected = vec![
            sdocx::TextDiagnostic {
                kind: sdocx::TextDiagnosticKind::UnsupportedMeasurementStyle,
                family: "DejaVu Sans".into(),
                codepoints: vec![0x627, 0x644],
            },
            sdocx::TextDiagnostic {
                kind: sdocx::TextDiagnosticKind::UnsupportedMeasurementFont,
                family: "DejaVu Sans".into(),
                codepoints: vec![0x627, 0x644],
            },
            sdocx::TextDiagnostic {
                kind: sdocx::TextDiagnosticKind::UnsupportedGlyphPositioning,
                family: "DejaVu Sans".into(),
                codepoints: vec![0x627, 0x644],
            },
        ];
        assert_eq!(page.text_diagnostics, expected, "{context:?}");
    }
}

#[test]
fn caller_controlled_oblique_face_keeps_actual_weight_and_style_in_css() {
    let family = "Caller Controlled";
    let defaults = FontBook::default();
    let italic = defaults.resolve("Roboto", false, true).unwrap();
    let mut face = defaults.database().face(italic.id).unwrap().clone();
    face.id = fontdb::ID::dummy();
    face.families[0].0 = family.into();
    face.families.truncate(1);
    face.weight = fontdb::Weight(550);
    face.style = fontdb::Style::Oblique;
    let mut database = fontdb::Database::new();
    database.push_face_info(face);
    database.set_sans_serif_family(family);
    let fonts = FontBook::new(Arc::new(database));
    let mut content = text("oblique");
    content.spans = vec![
        font_name(0, 7, family),
        span(RichTextSpanType::Italic, 0, 7, &[1, 0]),
    ];
    for &context in CONTEXTS {
        let document = document(context, content.clone());
        let page = sdocx::render_document_svg_with_fonts(&document, &Default::default(), &fonts)
            .pop()
            .unwrap();
        assert_eq!(
            page.text_diagnostics,
            vec![sdocx::TextDiagnostic {
                kind: sdocx::TextDiagnosticKind::UnsupportedMeasurementStyle,
                family: family.into(),
                codepoints: vec![98, 101, 105, 108, 111, 113, 117],
            }],
            "{context:?}",
        );
        let css = font_css(&page.svg);
        assert_eq!(css.len(), 1, "{context:?}");
        assert!(
            css[0].contains(&format!(
                "font-family:\"{}\";font-weight:550;font-style:oblique;",
                fonts.resolve(family, false, true).unwrap().svg_family()
            )),
            "{context:?}"
        );
        assert_eq!(embedded_font_bytes(&css[0]), italic.bytes(), "{context:?}");
        let xml = roxmltree::Document::parse(&page.svg).unwrap();
        assert_eq!(
            svgtypes::parse_font_families(tspan(&xml, "oblique").attribute("font-family").unwrap())
                .unwrap(),
            [
                svgtypes::FontFamily::Named(
                    fonts
                        .resolve(family, false, true)
                        .unwrap()
                        .svg_family()
                        .to_string()
                ),
                svgtypes::FontFamily::SansSerif
            ]
        );
    }
}

#[test]
fn stroke_only_pages_do_not_embed_font_styles() {
    let mut document = document(Context::Standalone, text("replaced"));
    document.pages[0].objects = vec![
        sdocx::Stroke {
            bbox: bounds(),
            rendering: None,
            points: vec![
                sdocx::Point { x: 20.0, y: 20.0 },
                sdocx::Point { x: 40.0, y: 40.0 },
            ],
            pressures: Vec::new(),
            timestamps: Vec::new(),
            tilts: Vec::new(),
            orientations: Vec::new(),
            color: None,
            pen_width: 2.0,
        }
        .into(),
    ];
    let page = sdocx::render_page_svg(&document, 0, &Default::default()).unwrap();
    let xml = roxmltree::Document::parse(&page.svg).unwrap();
    assert!(page.text_diagnostics.is_empty());
    assert!(
        !xml.descendants()
            .any(|node| node.has_tag_name("text") || node.has_tag_name("style"))
    );
    assert!(!page.svg.contains("data:font/"));
}

#[test]
fn unavailable_font_family_uses_roboto_and_reports_the_original_request() {
    let family = "Missing Native Typeface";
    let mut content = text("safe");
    content.spans = vec![font_name(0, 4, family)];
    let payload = content.spans[0].payload.clone();
    for &context in CONTEXTS {
        let document = document(context, content.clone());
        let page = sdocx::render_page_svg(&document, 0, &Default::default()).unwrap();
        assert_eq!(
            page.text_diagnostics,
            vec![sdocx::TextDiagnostic {
                kind: sdocx::TextDiagnosticKind::UnavailableFamily,
                family: family.into(),
                codepoints: Vec::new(),
            }],
            "{context:?}"
        );
        let xml = roxmltree::Document::parse(&page.svg).unwrap();
        assert_eq!(
            svgtypes::parse_font_families(tspan(&xml, "safe").attribute("font-family").unwrap())
                .unwrap(),
            [
                svgtypes::FontFamily::Named(
                    FontBook::default()
                        .resolve("Roboto", false, false)
                        .unwrap()
                        .svg_family()
                        .to_string()
                ),
                svgtypes::FontFamily::SansSerif
            ],
            "{context:?}"
        );
        assert_eq!(content.spans[0].font_name_value(), Some(family));
        assert_eq!(content.spans[0].payload, payload);
    }
}

#[test]
fn missing_cjk_and_emoji_glyphs_are_reported_without_losing_text() {
    for &context in CONTEXTS {
        let document = document(context, text("中😀中😀"));
        let page = sdocx::render_page_svg(&document, 0, &Default::default()).unwrap();
        let expected = vec![
            sdocx::TextDiagnostic {
                kind: sdocx::TextDiagnosticKind::UnsupportedMeasurementFont,
                family: "Roboto".into(),
                codepoints: vec![0x4e2d, 0x1f600],
            },
            sdocx::TextDiagnostic {
                kind: sdocx::TextDiagnosticKind::MissingGlyphs,
                family: "Roboto".into(),
                codepoints: vec![0x4e2d, 0x1f600],
            },
        ];
        assert_eq!(page.text_diagnostics, expected, "{context:?}");
        let xml = roxmltree::Document::parse(&page.svg).unwrap();
        assert_eq!(tspan(&xml, "中😀中😀").text(), Some("中😀中😀"));
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
fn ordinary_line_spacing_uses_the_largest_local_size_and_native_spacing_units() {
    for (spacing, first_baseline, next_baseline) in [
        (None, 60.0, 111.0),
        (Some((0_u32, 4.0_f32)), 51.0, 103.5),
        (Some((1_u32, 1.5_f32)), 69.0, 124.5),
    ] {
        for (context, top) in [(Context::Standalone, 20.0), (Context::Flow, 0.0)] {
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
            let svg = render(context, content);
            let xml = roxmltree::Document::parse(&svg).unwrap();
            for value in ["a", "B"] {
                assert_eq!(
                    tspan(&xml, value).parent().unwrap().attribute("y"),
                    Some(format!("{:.2}", top + first_baseline).as_str()),
                    "{context:?}, {spacing:?}: {value}"
                );
            }
            assert_eq!(
                tspan(&xml, "c").parent().unwrap().attribute("y"),
                Some(format!("{:.2}", top + next_baseline).as_str()),
                "{context:?}, {spacing:?}"
            );
            assert_eq!(tspan(&xml, "c").attribute("font-size"), Some("30.00"));
        }
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
        assert_decoration(&xml, tspan(&xml, "b"), Some("underline"));
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
        let spans: Vec<_> = xml
            .descendants()
            .filter(|node| node.has_tag_name("tspan"))
            .collect();
        assert_eq!(
            spans
                .iter()
                .filter_map(|node| node.text())
                .collect::<String>(),
            "a😀b"
        );
        for node in spans {
            assert_eq!(node.attribute("fill"), Some("#000000"), "{context:?}");
            assert_eq!(node.attribute("font-size"), Some("30.00"), "{context:?}");
            assert_eq!(node.attribute("text-decoration"), None, "{context:?}");
        }
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
        let exports = [
            sdocx::render_svg_pages_pdf(&[rendered], &options).unwrap(),
            sdocx::render_document_pdf(&document, &Default::default(), &options).unwrap(),
        ];
        for bytes in exports {
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
}
