use super::*;

fn measure_cell(
    content: &RichTextBox,
    width: i32,
    fonts: &FontBook,
) -> Result<TextLayout, ObjectDiagnosticKind> {
    let settings = TextSettings::resolved();
    let renderer = TextRenderer::new(settings, fonts);
    let styled = StyledText::new(content, TextContext::Flow, settings);
    layout_table_cell_text(
        &styled,
        TextFrame {
            bbox: BoundingBox {
                x_min: 0.0,
                y_min: 0.0,
                x_max: f64::from(width),
                y_max: 100.0,
            },
            gravity: Some(0),
            exclusions: &[],
        },
        width,
        RenderTheme::for_canvas(false),
        &renderer,
    )
}

#[test]
fn automatic_cell_width_resolves_each_native_paragraph_independently() {
    let fonts = FontBook::default();
    let mut content = text("AV\nToTo");
    content.font_size = Some(17.0);
    let layout = measure_cell(&content, 0, &fonts).unwrap();
    assert_eq!(layout.lines.len(), 2);
    assert_eq!(layout.lines[0].width, 22.0);
    assert_eq!(layout.lines[0].line.source, 0..2);
    assert_eq!(layout.lines[1].width, 39.0);
    assert_eq!(layout.lines[1].line.source, 3..7);

    content.text = "\u{200b}\n\u{200b}".into();
    let layout = measure_cell(&content, 0, &fonts).unwrap();
    assert_eq!(
        layout
            .lines
            .iter()
            .map(|line| line.width)
            .collect::<Vec<_>>(),
        [1.0, 1.0]
    );
}

#[test]
fn automatic_cell_measurement_diagnoses_unadmitted_native_inputs() {
    let fonts = FontBook::default();
    let mut content = text("AV");
    content.font_size = Some(17.0);
    let generic = FontBook::new(fonts.database());
    assert!(matches!(
        measure_cell(&content, 0, &generic),
        Err(ObjectDiagnosticKind::UnsupportedContent)
    ));
    assert!(measure_cell(&content, 22, &generic).is_ok());

    content.text = "A\tV".into();
    assert!(matches!(
        measure_cell(&content, 0, &fonts),
        Err(ObjectDiagnosticKind::UnsupportedContent)
    ));
    assert!(measure_cell(&content, 60, &fonts).is_ok());

    content.paragraphs.clear();
    for value in ["\nAV", "AV\n"] {
        content.text = value.into();
        assert!(matches!(
            measure_cell(&content, 0, &fonts),
            Err(ObjectDiagnosticKind::UnsupportedContent)
        ));
        assert!(measure_cell(&content, 60, &fonts).is_ok());
    }

    content.text = "AV".into();
    content.paragraphs = vec![RichTextParagraph {
        kind: RichTextParagraphType::IndentLevel,
        start_paragraph: 0,
        end_paragraph: 1,
        payload: [1_u32.to_le_bytes(), 0_u32.to_le_bytes()].concat(),
    }];
    assert!(matches!(
        measure_cell(&content, 0, &fonts),
        Err(ObjectDiagnosticKind::UnsupportedContent)
    ));
    assert!(measure_cell(&content, 60, &fonts).is_ok());

    content.paragraphs[0] = RichTextParagraph {
        kind: RichTextParagraphType::Bullet,
        start_paragraph: 0,
        end_paragraph: 1,
        payload: [0_u32.to_le_bytes(); 4].concat(),
    };
    assert_eq!(
        measure_cell(&content, 0, &fonts).unwrap().lines[0].width,
        22.0
    );
    content.paragraphs[0].payload[..4].copy_from_slice(&1_u32.to_le_bytes());
    assert!(matches!(
        measure_cell(&content, 0, &fonts),
        Err(ObjectDiagnosticKind::UnsupportedContent)
    ));
}

#[test]
fn empty_cell_does_not_enter_the_native_automatic_width_fold() {
    let content = text("");
    let fonts = FontBook::new(FontBook::default().database());
    let layout = measure_cell(&content, 0, &fonts).unwrap();
    assert!(layout.lines.is_empty());
    assert_eq!(layout.height(), 0.0);
}
