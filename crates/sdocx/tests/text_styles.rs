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
            .filter(|node| node.has_tag_name("rect") && node.attribute("fill") == Some("#000000"))
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
        assert!(page.text_diagnostics.is_empty(), "{context:?}");
        let svg = page.svg;
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
        assert!(css[0].contains("font-family:\"Roboto\";font-weight:400;font-style:normal;"));
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
        let expected = vec![
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
        let expected = vec![
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
        assert!(page.text_diagnostics.is_empty(), "{context:?}");
        let css = font_css(&page.svg);
        let placed = matches!(context, Context::Standalone);
        assert_eq!(css.len(), if placed { 4 } else { 2 }, "{context:?}");
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
            if weight == 700 && !placed {
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
        assert!(page.text_diagnostics.is_empty(), "{context:?}");
        let css = font_css(&page.svg);
        assert_eq!(css.len(), 1, "{context:?}");
        assert!(
            css[0]
                .contains("font-family:\"Caller Controlled\";font-weight:550;font-style:oblique;"),
            "{context:?}"
        );
        assert_eq!(embedded_font_bytes(&css[0]), italic.bytes(), "{context:?}");
        let xml = roxmltree::Document::parse(&page.svg).unwrap();
        assert_eq!(
            svgtypes::parse_font_families(tspan(&xml, "oblique").attribute("font-family").unwrap())
                .unwrap(),
            [
                svgtypes::FontFamily::Named(family.into()),
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
                svgtypes::FontFamily::Named("Roboto".into()),
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
        assert_eq!(
            page.text_diagnostics,
            vec![sdocx::TextDiagnostic {
                kind: sdocx::TextDiagnosticKind::MissingGlyphs,
                family: "Roboto".into(),
                codepoints: vec![0x4e2d, 0x1f600],
            }],
            "{context:?}"
        );
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
