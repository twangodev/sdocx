use sdocx::{
    BoundingBox, Document, DocumentMetadata, Page, PageElement, PageObjectContent, RichTextBox,
    RichTextSection, RichTextSpan, RichTextSpanType, SpanIntervalType, layout_document,
};

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
        runs: Vec::new(),
        spans: Vec::new(),
        paragraphs: Vec::new(),
        object_spans: Vec::new(),
        text_sections: Vec::new(),
        margins: None,
        gravity: None,
    }
}

fn document(body: RichTextBox, page_count: usize, height: u32) -> Document {
    Document {
        pages: (0..page_count)
            .map(|index| Page {
                uuid: format!("layout-review-{index}"),
                width: 360,
                height,
                content_bbox: BoundingBox::default(),
                background_color: None,
                template: None,
                background: Default::default(),
                objects: Vec::new(),
            })
            .collect(),
        metadata: DocumentMetadata {
            note_text: Some(body),
            ..Default::default()
        },
    }
}

fn nan_document() -> Document {
    let mut body = text(&"A ".repeat(500));
    body.margins = Some([f32::from_bits(0x7fc0_0042), 0.0, 0.0, 0.0]);
    document(body, 2, 100)
}

#[test]
fn unchanged_nan_source_and_inspection_retain_authoritative_reflow() {
    let document = nan_document();
    let layout = layout_document(&document);
    for page in &layout.pages {
        let source = page.body_text_reflow(&document).unwrap();
        assert_eq!(source.text, "A ".repeat(500));
        assert_eq!(source.margins.unwrap()[0].to_bits(), 0x7fc0_0042);
    }
}

#[test]
fn malformed_source_identity_rejects_changed_nan_bits_locations_and_content() {
    let original = nan_document();
    let layout = layout_document(&original);
    for edit in 0..4 {
        let mut changed = original.clone();
        let body = changed.metadata.note_text.as_mut().unwrap();
        match edit {
            0 => body.margins.as_mut().unwrap()[0] = f32::from_bits(0x7fc0_0043),
            1 => body.margins.as_mut().unwrap().swap(0, 1),
            2 => body.text.replace_range(..1, "B"),
            3 => body.font_size = Some(11.0),
            _ => unreachable!(),
        }
        for page in &layout.pages {
            assert!(page.body_text_reflow(&changed).is_none(), "edit {edit}");
        }
    }
}

#[test]
fn malformed_source_identity_does_not_resurrect_edited_inspection() {
    let document = nan_document();
    for edit in 0..3 {
        let mut layout = layout_document(&document);
        let page = &mut layout.pages[1];
        let index = page.body_text.as_ref().unwrap().object_index;
        let PageObjectContent::Element(PageElement::TextBox(inspection)) =
            &mut page.page.objects[index].content
        else {
            unreachable!()
        };
        match edit {
            0 => inspection.margins.as_mut().unwrap()[0] = f32::from_bits(0x7fc0_0043),
            1 => inspection.margins.as_mut().unwrap().swap(0, 1),
            2 => inspection.underline = true,
            _ => unreachable!(),
        }
        assert!(page.body_text_reflow(&document).is_none(), "edit {edit}");
    }
}

#[test]
fn signed_zero_identity_is_consistent_with_and_without_nan() {
    for nan in [false, true] {
        let mut original = nan_document();
        if !nan {
            original
                .metadata
                .note_text
                .as_mut()
                .unwrap()
                .margins
                .as_mut()
                .unwrap()[0] = 0.0;
        }
        let layout = layout_document(&original);
        original
            .metadata
            .note_text
            .as_mut()
            .unwrap()
            .margins
            .as_mut()
            .unwrap()[1] = -0.0;
        assert!(layout.pages[1].body_text_reflow(&original).is_some());
    }
}

#[cfg(feature = "render")]
#[test]
fn invalid_margin_recovery_preserves_second_page_text() {
    let malformed = nan_document();
    let mut valid = malformed.clone();
    valid.metadata.note_text.as_mut().unwrap().margins = Some([0.0; 4]);
    let render = |document: &Document| {
        sdocx::render_layout_page_svg(document, &layout_document(document), 1, &Default::default())
            .unwrap()
    };
    let valid = render(&valid);
    let recovered = render(&malformed);
    let text_nodes = |svg: &str| {
        let xml = roxmltree::Document::parse(svg).unwrap();
        xml.descendants()
            .filter(|node| node.has_tag_name("text"))
            .map(|node| {
                (
                    node.attribute("x").map(str::to_owned),
                    node.attribute("y").map(str::to_owned),
                    node.descendants()
                        .filter(|child| child.is_text())
                        .filter_map(|child| child.text())
                        .collect::<String>(),
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(text_nodes(&valid.svg).len(), 8);
    assert_eq!(text_nodes(&recovered.svg), text_nodes(&valid.svg));
    assert!(
        recovered
            .text_diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.kind == sdocx::TextDiagnosticKind::InvalidGeometry })
    );
}

fn font_caret(anchor: u32, interval: SpanIntervalType) -> RichTextSpan {
    RichTextSpan {
        kind: RichTextSpanType::FontSize,
        start_utf16: anchor,
        end_utf16: anchor,
        interval_type: interval,
        payload: 60_f32.to_le_bytes().to_vec(),
    }
}

#[test]
fn full_slice_and_capture_preserve_native_font_caret_intervals() {
    for interval in [
        SpanIntervalType::ClosedOpen,
        SpanIntervalType::ClosedClosed,
        SpanIntervalType::OpenOpen,
        SpanIntervalType::OpenClosed,
        SpanIntervalType::Other(77),
    ] {
        let mut body = text("\nA");
        body.spans.push(font_caret(0, interval));
        body.text_sections.push(RichTextSection {
            start_utf16: 0,
            length_utf16: 2,
        });
        assert_eq!(body.slice_chars(0..2).unwrap().spans, body.spans);
        let mut document = document(body.clone(), 1, 300);
        document.metadata.page_mode = Some(0);
        let capture = layout_document(&document).pages[0]
            .body_text_capture(&document)
            .unwrap();
        assert_eq!(capture.spans, body.spans);
    }
}

#[test]
fn slices_preserve_both_boundary_carets_without_creating_glyph_style_ranges() {
    let mut body = text("A😀B");
    body.spans = vec![
        font_caret(1, SpanIntervalType::ClosedClosed),
        font_caret(4, SpanIntervalType::OpenClosed),
        font_caret(2, SpanIntervalType::ClosedOpen),
        font_caret(5, SpanIntervalType::ClosedOpen),
    ];
    assert_eq!(
        body.slice_chars(0..1).unwrap().spans,
        [font_caret(1, SpanIntervalType::ClosedClosed)]
    );
    let following = body.slice_chars(1..2).unwrap();
    assert_eq!(
        following.spans,
        [font_caret(0, SpanIntervalType::ClosedClosed)]
    );
    assert_eq!(
        body.slice_chars(1..1).unwrap().spans,
        [font_caret(0, SpanIntervalType::ClosedClosed)]
    );
    assert_eq!(
        body.slice_chars(2..3).unwrap().spans,
        [font_caret(1, SpanIntervalType::OpenClosed)]
    );
    assert_eq!(
        body.slice_chars(3..3).unwrap().spans,
        [font_caret(0, SpanIntervalType::OpenClosed)]
    );
    let mut empty = text("");
    empty
        .spans
        .push(font_caret(0, SpanIntervalType::ClosedOpen));
    assert_eq!(empty.slice_chars(0..0).unwrap().spans, empty.spans);
}

#[test]
fn capture_rejects_source_or_inspection_edits_to_preserved_font_carets() {
    let mut body = text("\nA");
    body.spans.push(font_caret(0, SpanIntervalType::ClosedOpen));
    body.text_sections.push(RichTextSection {
        start_utf16: 0,
        length_utf16: 2,
    });
    let source = document(body, 1, 300);
    let original = layout_document(&source);
    let mut changed_source = source.clone();
    changed_source.metadata.note_text.as_mut().unwrap().spans[0].payload =
        40_f32.to_le_bytes().to_vec();
    assert!(
        original.pages[0]
            .body_text_capture(&changed_source)
            .is_none()
    );
    let mut changed_layout = original.clone();
    let page = &mut changed_layout.pages[0];
    let index = page.body_text.as_ref().unwrap().object_index;
    let PageObjectContent::Element(PageElement::TextBox(inspection)) =
        &mut page.page.objects[index].content
    else {
        unreachable!()
    };
    inspection.spans.clear();
    assert!(page.body_text_capture(&source).is_none());
}

#[cfg(feature = "render")]
#[test]
fn capture_keeps_start_caret_font_geometry_without_promoting_it_to_glyph_style() {
    for (interval, expected_baseline) in [
        (SpanIntervalType::ClosedOpen, 91.0),
        (SpanIntervalType::ClosedClosed, 91.0),
        (SpanIntervalType::OpenOpen, 23.5),
        (SpanIntervalType::OpenClosed, 91.0),
        (SpanIntervalType::Other(77), 91.0),
    ] {
        let mut body = text("\nA");
        body.spans.push(font_caret(0, interval));
        body.text_sections.push(RichTextSection {
            start_utf16: 0,
            length_utf16: 2,
        });
        let mut source = document(body.clone(), 1, 300);
        source.metadata.page_mode = Some(0);
        let capture = sdocx::render_layout_page_svg(
            &source,
            &layout_document(&source),
            0,
            &Default::default(),
        )
        .unwrap();
        source.metadata.note_text = None;
        source.pages[0]
            .objects
            .push(PageElement::TextBox(body).into());
        let direct = sdocx::render_page_svg(&source, 0, &Default::default()).unwrap();
        for rendered in [capture, direct] {
            let xml = roxmltree::Document::parse(&rendered.svg).unwrap();
            let glyph = xml
                .descendants()
                .find(|node| node.has_tag_name("text"))
                .unwrap();
            assert_eq!(
                glyph.attribute("y").unwrap().parse::<f64>().unwrap(),
                expected_baseline
            );
            let span = glyph
                .descendants()
                .find(|node| node.has_tag_name("tspan"))
                .unwrap();
            assert_eq!(span.attribute("font-size"), Some("10.00"));
        }
    }
}
