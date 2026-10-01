#![cfg(feature = "pdf")]

use sdocx::{
    BoundingBox, Document, DocumentMetadata, ObjectDiagnostic, ObjectDiagnosticKind,
    ObjectSpanLayoutConstraint, ObjectSpanLayoutOption, ObjectType, Page, PageElement, PdfError,
    PdfOptions, RichTextBox, RichTextObjectSpan, RichTextSpan, RichTextSpanType, SpanIntervalType,
    TextDiagnostic, TextDiagnosticKind, fonts::FontBook,
    render_layout_pages_pdf_detailed_with_fonts, render_layout_pages_pdf_with_fonts,
};

fn text(value: &str) -> RichTextBox {
    RichTextBox {
        text_area_type: None,
        bbox: BoundingBox {
            x_min: 10.0,
            y_min: 10.0,
            x_max: 310.0,
            y_max: 200.0,
        },
        rotation_degrees: None,
        text: value.into(),
        color: None,
        highlight_color: None,
        underline: false,
        font_size: Some(20.0),
        runs: Vec::new(),
        spans: Vec::new(),
        paragraphs: Vec::new(),
        object_spans: Vec::new(),
        text_sections: Vec::new(),
        margins: None,
        gravity: None,
    }
}

fn document() -> Document {
    let family = "Unavailable review font";
    let mut noisy = text("B");
    noisy.spans.push(RichTextSpan {
        kind: RichTextSpanType::FontName,
        start_utf16: 0,
        end_utf16: 1,
        interval_type: SpanIntervalType::ClosedOpen,
        payload: [
            vec![0; 8],
            ((family.len() + 1) as u16).to_le_bytes().to_vec(),
            family.as_bytes().to_vec(),
            vec![0],
        ]
        .concat(),
    });
    noisy.object_spans.push(RichTextObjectSpan {
        object_type: ObjectType::Image,
        object_data: Vec::new(),
        content: None,
        text_index_utf16: 0,
        layout_option: ObjectSpanLayoutOption::Inline,
        layout_constraint: ObjectSpanLayoutConstraint::Normal,
    });
    Document {
        metadata: DocumentMetadata {
            default_page_dimensions: Some((360, 400)),
            ..Default::default()
        },
        pages: [text("A"), noisy]
            .into_iter()
            .enumerate()
            .map(|(index, content)| Page {
                uuid: format!("review-{index}"),
                width: 360,
                height: 400,
                content_bbox: Default::default(),
                background_color: None,
                template: None,
                background: Default::default(),
                objects: vec![PageElement::TextBox(content).into()],
            })
            .collect(),
    }
}

#[test]
fn detailed_pdf_preserves_selected_page_diagnostics_and_source_order() {
    let document = document();
    let layout = sdocx::layout_document(&document);
    let fonts = FontBook::default();
    let output = render_layout_pages_pdf_detailed_with_fonts(
        &document,
        &layout,
        &[1, 0, 1],
        &Default::default(),
        &PdfOptions::default(),
        &fonts,
    )
    .unwrap();
    assert_eq!(
        output
            .pages
            .iter()
            .map(|page| page.page_index)
            .collect::<Vec<_>>(),
        [1, 0, 1]
    );
    assert_eq!(output.pages[0], output.pages[2]);
    assert_eq!(
        output.pages[0].text_diagnostics,
        [TextDiagnostic {
            kind: TextDiagnosticKind::UnavailableFamily,
            family: "Unavailable review font".into(),
            codepoints: Vec::new(),
        }]
    );
    assert_eq!(
        output.pages[0].object_diagnostics,
        [ObjectDiagnostic {
            anchor_utf16: 0,
            kind: ObjectDiagnosticKind::NonReplacementAnchor,
        }]
    );
    assert!(output.pages[1].text_diagnostics.is_empty());
    assert!(output.pages[1].object_diagnostics.is_empty());
    let pdf = lopdf::Document::load_mem(&output.bytes).unwrap();
    assert_eq!(pdf.get_pages().len(), 3);
    for (page, expected) in [(1, "B"), (2, "A"), (3, "B")] {
        assert_eq!(pdf.extract_text(&[page]).unwrap().trim(), expected);
    }
}

#[test]
fn diagnostics_from_unselected_pages_do_not_leak_into_export() {
    let document = document();
    let layout = sdocx::layout_document(&document);
    let fonts = FontBook::default();
    let options = PdfOptions::default();
    let output = render_layout_pages_pdf_detailed_with_fonts(
        &document,
        &layout,
        &[0],
        &Default::default(),
        &options,
        &fonts,
    )
    .unwrap();
    assert_eq!(output.pages.len(), 1);
    assert_eq!(output.pages[0].page_index, 0);
    assert!(output.pages[0].text_diagnostics.is_empty());
    assert!(output.pages[0].object_diagnostics.is_empty());
    let legacy = render_layout_pages_pdf_with_fonts(
        &document,
        &layout,
        &[0],
        &Default::default(),
        &options,
        &fonts,
    )
    .unwrap();
    for bytes in [&output.bytes, &legacy] {
        let pdf = lopdf::Document::load_mem(bytes).unwrap();
        assert_eq!(pdf.get_pages().len(), 1);
        assert_eq!(pdf.extract_text(&[1]).unwrap().trim(), "A");
    }
    assert!(matches!(
        render_layout_pages_pdf_detailed_with_fonts(
            &document,
            &layout,
            &[0, 8],
            &Default::default(),
            &options,
            &fonts,
        ),
        Err(PdfError::InvalidPageIndex { page_index: 8 })
    ));
    assert!(matches!(
        render_layout_pages_pdf_detailed_with_fonts(
            &document,
            &layout,
            &[],
            &Default::default(),
            &options,
            &fonts,
        ),
        Err(PdfError::EmptyDocument)
    ));
    let mut invalid_options = options;
    invalid_options.dpi = f32::NAN;
    assert!(matches!(
        render_layout_pages_pdf_detailed_with_fonts(
            &document,
            &layout,
            &[0],
            &Default::default(),
            &invalid_options,
            &fonts,
        ),
        Err(PdfError::InvalidDpi)
    ));
}
