#![cfg(feature = "render")]

use base64::Engine;
use sdocx::{
    BoundingBox, Document, DocumentMetadata, LayoutDocument, ObjectSpanLayoutConstraint,
    ObjectSpanLayoutOption, ObjectType, Page, PageElement, PageObjectContent, RichTextBox,
    RichTextCodeBlock, RichTextObjectContent, RichTextObjectSpan, RichTextSection, RichTextSpan,
    RichTextSpanType,
};

fn family(start: u32, end: u32, value: &str) -> RichTextSpan {
    RichTextSpan {
        kind: RichTextSpanType::FontName,
        start_utf16: start,
        end_utf16: end,
        interval_type: sdocx::SpanIntervalType::from(1),
        payload: [
            vec![0; 8],
            u16::try_from(value.len() + 1)
                .unwrap()
                .to_le_bytes()
                .to_vec(),
            value.as_bytes().to_vec(),
            vec![0],
        ]
        .concat(),
    }
}

fn document(styled: bool) -> Document {
    let body = RichTextBox {
        text_area_type: None,
        bbox: BoundingBox::default(),
        rotation_degrees: None,
        text: "First\nSecond\n".into(),
        color: None,
        highlight_color: None,
        underline: false,
        font_size: Some(10.0),
        runs: Vec::new(),
        spans: if styled {
            vec![
                family(0, 5, "Unavailable First Face"),
                family(6, 12, "Roboto Mono"),
            ]
        } else {
            Vec::new()
        },
        paragraphs: Vec::new(),
        object_spans: Vec::new(),
        text_sections: vec![
            RichTextSection {
                start_utf16: 0,
                length_utf16: 6,
            },
            RichTextSection {
                start_utf16: 5,
                length_utf16: 8,
            },
        ],
        margins: None,
        gravity: None,
    };
    Document {
        pages: (0..2)
            .map(|index| Page {
                uuid: format!("capture-page-{index}"),
                width: 360,
                height: 40,
                content_bbox: BoundingBox {
                    x_min: 0.0,
                    y_min: 0.0,
                    x_max: 360.0,
                    y_max: 40.0,
                },
                background_color: None,
                template: None,
                background: Default::default(),
                objects: Vec::new(),
            })
            .collect(),
        metadata: DocumentMetadata {
            note_text: Some(body),
            page_mode: Some(0),
            orientation: Some(0),
            default_page_dimensions: Some((360, 40)),
            flow_page_padding: Some((10, 0)),
            ..Default::default()
        },
    }
}

fn render(
    doc: &Document,
    layout: &LayoutDocument,
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

fn source(svg: &str) -> String {
    roxmltree::Document::parse(svg)
        .unwrap()
        .descendants()
        .filter(|node| node.has_tag_name("tspan"))
        .filter_map(|node| node.text())
        .collect()
}

fn assert_visible_line(page: &sdocx::RenderedPage, expected: &str, source_page: usize) {
    assert_eq!(source(&page.svg), expected);
    assert_eq!(page.source_page_index, source_page);
    let xml = roxmltree::Document::parse(&page.svg).unwrap();
    let spans: Vec<_> = xml
        .descendants()
        .filter(|node| node.has_tag_name("tspan"))
        .collect();
    assert_eq!(spans.len(), 1);
    assert_eq!(span_position(spans[0]), (10.0, 20.0));
}

fn span_position(node: roxmltree::Node<'_, '_>) -> (f64, f64) {
    let value = |attribute| {
        node.attribute(attribute)
            .or_else(|| node.parent().unwrap().attribute(attribute))
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .parse::<f64>()
            .unwrap()
    };
    let mut point = (value("x"), value("y"));
    for ancestor in node.ancestors() {
        if let Some(transform) = ancestor.attribute("transform") {
            let transform: svgtypes::Transform = transform.parse().unwrap();
            point = (
                transform.a * point.0 + transform.c * point.1 + transform.e,
                transform.b * point.0 + transform.d * point.1 + transform.f,
            );
        }
    }
    point
}

fn font_css(svg: &str) -> Vec<String> {
    roxmltree::Document::parse(svg)
        .unwrap()
        .descendants()
        .filter(|node| node.has_tag_name("style"))
        .filter_map(|node| node.text())
        .map(str::to_owned)
        .collect()
}

#[test]
fn grouped_capture_emits_only_its_requested_viewport_independently_of_render_order() {
    let doc = document(false);
    let layout = sdocx::layout_document(&doc);
    assert_eq!(layout.pages.len(), 2);
    let capture = layout.pages[1]
        .body_text_slice()
        .unwrap()
        .capture_window
        .as_ref()
        .unwrap();
    assert_eq!(capture.first_page_index, 0);
    assert_eq!(capture.requested_page_index, 1);
    assert_eq!(capture.source_range, 0..13);
    let fonts = sdocx::fonts::FontBook::default();
    // Native bands around 0/40/80 shift 10px-font baselines to 20 and 60.
    // The second viewport removes 40 from the retained second-line baseline.
    for replay in [false, true] {
        let second = render(&doc, &layout, 1, replay, &fonts);
        assert_visible_line(&second, "Second", 1);
        let first = render(&doc, &layout, 0, replay, &fonts);
        assert_visible_line(&first, "First", 0);
        assert_eq!(render(&doc, &layout, 1, replay, &fonts), second);
        assert_eq!(render(&doc, &layout, 0, replay, &fonts), first);
    }
}

#[test]
fn offscreen_font_fallback_diagnostics_and_faces_do_not_leak_into_the_requested_page() {
    let doc = document(true);
    let layout = sdocx::layout_document(&doc);
    let fonts = sdocx::fonts::FontBook::default();
    for replay in [false, true] {
        let second = render(&doc, &layout, 1, replay, &fonts);
        assert_visible_line(&second, "Second", 1);
        assert!(
            second.text_diagnostics.is_empty(),
            "{:?}",
            second.text_diagnostics
        );
        assert!(second.object_diagnostics.is_empty());
        let css = font_css(&second.svg);
        assert_eq!(css.len(), 1);
        assert_eq!(css[0].matches("@font-face").count(), 1);
        assert!(css[0].contains("font-family:\"Roboto Mono\";"));
        let encoded = css[0]
            .split_once("data:font/ttf;base64,")
            .unwrap()
            .1
            .split_once('"')
            .unwrap()
            .0;
        assert_eq!(
            base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .unwrap(),
            include_bytes!("../assets/fonts/RobotoMono-Regular.ttf")
        );
        let first = render(&doc, &layout, 0, replay, &fonts);
        assert_visible_line(&first, "First", 0);
        assert_eq!(first.text_diagnostics.len(), 1);
        assert_eq!(
            first.text_diagnostics[0].kind,
            sdocx::TextDiagnosticKind::UnavailableFamily
        );
        assert_eq!(first.text_diagnostics[0].family, "Unavailable First Face");
        assert_eq!(render(&doc, &layout, 1, replay, &fonts), second);
    }
}

#[test]
fn a_single_page_debugger_clone_keeps_its_physical_capture_viewport() {
    let doc = document(true);
    let layout = sdocx::layout_document(&doc);
    let mut debugger = layout.clone();
    debugger.pages = vec![layout.pages[1].clone()];
    let fonts = sdocx::fonts::FontBook::default();
    for replay in [false, true] {
        let expected = render(&doc, &layout, 1, replay, &fonts);
        let actual = render(&doc, &debugger, 0, replay, &fonts);
        assert_visible_line(&actual, "Second", 1);
        assert_eq!(actual, expected);
    }
}

#[test]
fn edited_or_removed_body_objects_are_not_resurrected_from_capture_metadata() {
    let doc = document(true);
    let layout = sdocx::layout_document(&doc);
    let fonts = sdocx::fonts::FontBook::default();
    let mut edited = layout.clone();
    let index = edited.pages[1].body_text_slice().unwrap().object_index;
    let PageObjectContent::Element(PageElement::TextBox(body)) =
        &mut edited.pages[1].page.objects[index].content
    else {
        panic!()
    };
    body.text = "Edited".into();
    let mut removed = layout.clone();
    removed.pages[1].page.objects.clear();
    for replay in [false, true] {
        assert_eq!(
            source(&render(&doc, &edited, 1, replay, &fonts).svg),
            "Edited"
        );
        let empty = render(&doc, &removed, 1, replay, &fonts);
        assert_eq!(source(&empty.svg), "");
        assert!(empty.text_diagnostics.is_empty());
        assert!(font_css(&empty.svg).is_empty());
    }
}

#[cfg(feature = "pdf")]
#[test]
fn capture_pdf_contains_only_selectable_visible_text_and_its_embedded_face() {
    let doc = document(true);
    let layout = sdocx::layout_document(&doc);
    let fonts = sdocx::fonts::FontBook::default();
    for replay in [false, true] {
        let page = render(&doc, &layout, 1, replay, &fonts);
        assert_visible_line(&page, "Second", 1);
        let bytes = sdocx::render_svg_pages_pdf(&[page], &Default::default()).unwrap();
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        assert_eq!(
            pdf.extract_text(&[1]).unwrap().replace(['\n', ' '], ""),
            "Second"
        );
        let mut font_names = Vec::new();
        for object in pdf.objects.values() {
            if let Ok(stream) = object.as_stream() {
                assert!(
                    !stream
                        .dict
                        .get(b"Subtype")
                        .is_ok_and(|value| value.as_name().is_ok_and(|name| name == b"Image"))
                );
            }
            if let Ok(dictionary) = object.as_dict()
                && dictionary
                    .get(b"Type")
                    .is_ok_and(|value| value.as_name().is_ok_and(|name| name == b"FontDescriptor"))
            {
                font_names.push(
                    std::str::from_utf8(dictionary.get(b"FontName").unwrap().as_name().unwrap())
                        .unwrap(),
                );
            }
        }
        assert_eq!(font_names.len(), 1);
        assert!(font_names[0].contains("RobotoMono"), "{:?}", font_names);
    }
}

fn code_capture_document(styled: bool) -> Document {
    let mut doc = document(false);
    for page in &mut doc.pages {
        page.height = 80;
        page.content_bbox.y_max = 80.0;
    }
    doc.metadata.default_page_dimensions = Some((360, 80));
    doc.metadata.flow_page_padding = Some((0, 0));
    let text = |value: &str| RichTextBox {
        text: value.into(),
        text_sections: Vec::new(),
        ..doc.metadata.note_text.as_ref().unwrap().clone()
    };
    let mut title = text("Hidden title");
    let mut body = text("A\nB\nC\nD");
    if styled {
        title.spans = vec![family(0, 12, "Unavailable Code Title")];
        body.spans = vec![family(0, 7, "Roboto Mono")];
    }
    let parent = doc.metadata.note_text.as_mut().unwrap();
    parent.text = "\u{fffc}\nEnd".into();
    parent.text_sections = vec![
        RichTextSection {
            start_utf16: 0,
            length_utf16: 2,
        },
        RichTextSection {
            start_utf16: 0,
            length_utf16: 5,
        },
    ];
    parent.object_spans.push(RichTextObjectSpan {
        object_type: ObjectType::CodeBlock,
        object_data: Vec::new(),
        content: Some(RichTextObjectContent::CodeBlock(Box::new(
            RichTextCodeBlock {
                bbox: BoundingBox {
                    x_min: 0.0,
                    y_min: 0.0,
                    x_max: 300.0,
                    y_max: 10.0,
                },
                rotation_degrees: None,
                title: Some(title),
                body: Some(body),
            },
        ))),
        text_index_utf16: 0,
        layout_option: ObjectSpanLayoutOption::Inline,
        layout_constraint: ObjectSpanLayoutConstraint::Normal,
    });
    doc
}

fn assert_visible_code_body(page: &sdocx::RenderedPage) {
    assert_eq!(page.source_page_index, 1);
    assert_eq!(source(&page.svg), "CD");
    let xml = roxmltree::Document::parse(&page.svg).unwrap();
    let spans: Vec<_> = xml
        .descendants()
        .filter(|node| node.has_tag_name("tspan"))
        .collect();
    assert_eq!(spans.len(), 2);
    // Native chrome places the body 44px below the object. Four 13.5px lines
    // plus its bottom gap and padding give 118px, despite the saved 10px box.
    for (span, expected) in spans.into_iter().zip([(16.0, 11.001), (16.0, 24.501)]) {
        let actual = span_position(span);
        assert!((actual.0 - expected.0).abs() < 1e-8, "{actual:?}");
        assert!((actual.1 - expected.1).abs() < 1e-8, "{actual:?}");
    }
    let panel = xml
        .descendants()
        .find(|node| node.has_tag_name("rect") && node.attribute("fill") == Some("#efefef"))
        .unwrap();
    assert_eq!(
        panel.attribute("width").unwrap().parse::<f64>().unwrap(),
        300.0
    );
    assert_eq!(
        panel.attribute("height").unwrap().parse::<f64>().unwrap(),
        118.0
    );
}

#[test]
fn visible_prepared_code_bounds_survive_an_offscreen_saved_reservation() {
    let doc = code_capture_document(false);
    let layout = sdocx::layout_document(&doc);
    let fonts = sdocx::fonts::FontBook::default();
    for replay in [false, true] {
        let page = render(&doc, &layout, 1, replay, &fonts);
        assert_visible_code_body(&page);
        assert!(page.text_diagnostics.is_empty());
        assert!(page.object_diagnostics.is_empty());
        #[cfg(feature = "pdf")]
        {
            let bytes = sdocx::render_svg_pages_pdf(&[page], &Default::default()).unwrap();
            let pdf = lopdf::Document::load_mem(&bytes).unwrap();
            assert_eq!(
                pdf.extract_text(&[1]).unwrap().replace(['\n', ' '], ""),
                "CD"
            );
        }
    }
}

#[test]
fn offscreen_code_title_does_not_leak_its_font_or_diagnostic_into_visible_body() {
    let doc = code_capture_document(true);
    let layout = sdocx::layout_document(&doc);
    let fonts = sdocx::fonts::FontBook::default();
    for replay in [false, true] {
        let page = render(&doc, &layout, 1, replay, &fonts);
        assert_visible_code_body(&page);
        assert!(
            page.text_diagnostics.is_empty(),
            "{:?}",
            page.text_diagnostics
        );
        let css = font_css(&page.svg);
        assert_eq!(css.len(), 1);
        assert_eq!(css[0].matches("@font-face").count(), 1);
        assert!(css[0].contains("font-family:\"Roboto Mono\";"));
        let first = render(&doc, &layout, 0, replay, &fonts);
        assert!(first.text_diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == sdocx::TextDiagnosticKind::UnavailableFamily
                && diagnostic.family == "Unavailable Code Title"
        }));
        assert_eq!(render(&doc, &layout, 1, replay, &fonts), page);
    }
}
