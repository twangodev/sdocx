mod support;

use sdocx::{
    DiagnosticCode, Error, PageElement, ParseLimits, ParseOptions, RichTextBox, RichTextSpanType,
};
use support::{archive, object, page};

// Native ObjectTextBox serialization is ObjectBase (0), ObjectShape (6),
// ObjectShapeText (7), then ComponentImage's Textbox record (2). Deliberately
// use different mask widths from the stroke fixtures and a non-UUID identity.
fn frame(kind: i16, fields: &[u8], fixed: &[u8], flexible: &[u8]) -> Vec<u8> {
    let offset = 13 + fields.len() + fixed.len();
    let mut bytes = ((offset + flexible.len()) as u32).to_le_bytes().to_vec();
    bytes.extend_from_slice(&kind.to_le_bytes());
    bytes.extend_from_slice(&(offset as u32).to_le_bytes());
    bytes.extend_from_slice(&[1, u8::from(kind == 0) << 3, fields.len() as u8]);
    bytes.extend_from_slice(fields);
    bytes.extend_from_slice(fixed);
    bytes.extend_from_slice(flexible);
    bytes
}

fn base(bbox: [f64; 4], rotation: f32) -> Vec<u8> {
    let mut fixed = 5500_u32.to_le_bytes().to_vec();
    fixed.extend_from_slice(&2_u16.to_le_bytes());
    fixed.extend_from_slice(b"tx");
    fixed.extend_from_slice(&1234_i64.to_le_bytes());
    for value in bbox {
        fixed.extend_from_slice(&value.to_le_bytes());
    }
    fixed.extend_from_slice(&0_i32.to_le_bytes());
    fixed.push(0);
    frame(0, &[1, 0, 0, 0, 0], &fixed, &rotation.to_le_bytes())
}

#[test]
fn hidden_text_is_retained_without_entering_svg_exports() {
    let mut hidden = simple("hidden phrase");
    hidden[11] &= !(1 << 3);
    let raw = page(
        &[vec![
            object(2, &hidden, &[]),
            object(2, &simple("visible phrase"), &[]),
        ]],
        0,
        &[],
    );
    let parsed = sdocx::parse_bytes_detailed(&archive(&raw)).unwrap();
    assert_eq!(parsed.document.pages[0].elements.len(), 1);
    let stored = &parsed.stored_pages[0].page.layers.layers[0].objects;
    assert_eq!(stored.len(), 2);
    assert_eq!(stored[0].payload(&raw).unwrap(), hidden);
    assert!(!stored[0].base_metadata(&raw).unwrap().visible);
    #[cfg(feature = "render")]
    {
        let rendered =
            sdocx::render_page_svg(&parsed.document, 0, &sdocx::RenderOptions::default()).unwrap();
        assert!(rendered.svg.contains("visible phrase"));
        assert!(!rendered.svg.contains("hidden phrase"));
    }
}

fn span(kind: u32, start: u32, end: u32, payload: &[u8]) -> Vec<u8> {
    let mut bytes = ((16 + payload.len()) as u16).to_le_bytes().to_vec();
    for value in [kind, start, end, 1] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(payload);
    bytes
}

fn common(text: &str, spans: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = (text.encode_utf16().count() as u32).to_le_bytes().to_vec();
    for unit in text.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    bytes.extend_from_slice(&(spans.len() as u32).to_le_bytes());
    for span in spans {
        bytes.extend_from_slice(span);
    }
    bytes.extend_from_slice(&0_u32.to_le_bytes()); // paragraphs
    for value in [2.0_f32, 3.0, 4.0, 5.0] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.push(0); // gravity
    bytes.extend_from_slice(&0_u16.to_le_bytes()); // text sections
    bytes.extend_from_slice(&[0; 8]); // object-span flags/reserved
    bytes
}

fn text_payload(common: &[u8], bbox: [f64; 4], rotation: f32, tail: &[u8]) -> Vec<u8> {
    let mut bytes = base(bbox, rotation);
    bytes.extend(frame(6, &[], &[], &[]));
    let mut text = (common.len() as u32).to_le_bytes().to_vec();
    text.extend_from_slice(common);
    bytes.extend(frame(7, &[1], &[], &text));
    bytes.extend_from_slice(tail);
    bytes
}

fn simple(text: &str) -> Vec<u8> {
    text_payload(
        &common(text, &[]),
        [0.0, 0.0, 1.0, 1.0],
        0.0,
        &frame(2, &[], &[], &[]),
    )
}

fn single(payload: &[u8]) -> Vec<u8> {
    archive(&page(&[vec![object(2, payload, &[])]], 0, &[]))
}

fn text_box(element: &PageElement) -> &RichTextBox {
    let PageElement::TextBox(text) = element else {
        panic!("expected a text box")
    };
    text
}

#[test]
fn preserves_short_unicode_empty_and_whitespace_text_without_scanning() {
    for text in ["", "a", "中", "こんにちは", "😀", "a😀中\nβ", " \n "] {
        let parsed = sdocx::parse_bytes_detailed(&single(&simple(text))).unwrap();
        let page = &parsed.document.pages[0];
        assert_eq!(page.elements.len(), 1);
        let decoded = text_box(&page.elements[0]);
        assert_eq!(decoded.text, text);
        assert_eq!(decoded.bbox.x_min, 0.0);
        assert_eq!(decoded.bbox.x_max, 1.0);
        assert_eq!(decoded.margins, Some([2.0, 3.0, 4.0, 5.0]));
        assert!(
            !parsed
                .report
                .diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::UnsupportedTextBoxFeature)
        );
    }
}

#[test]
fn uses_declared_bounds_rotation_and_utf16_style_ranges() {
    let spans = [
        span(5, 1, 3, &[1, 0]), // bold emoji, two UTF-16 code units
        span(6, 3, 4, &[1, 0]), // italic CJK, one code unit
        span(1, 0, 4, &[0x56, 0x34, 0x12, 0xff]),
        span(3, 0, 4, &12.0_f32.to_le_bytes()),
    ];
    let payload = text_payload(
        &common("a😀中", &spans),
        [-20.0, 40.0, 180.0, 140.0],
        30.0,
        &frame(2, &[], &[], &[]),
    );
    let raw = page(&[vec![object(2, &payload, &[])]], 0, &[]);
    let parsed = sdocx::parse_bytes_detailed(&archive(&raw)).unwrap();
    let decoded = text_box(&parsed.document.pages[0].elements[0]);
    assert_eq!(decoded.text, "a😀中");
    assert_eq!(decoded.rotation_degrees, Some(30.0));
    assert_eq!(decoded.bbox.x_min, -20.0);
    assert_eq!(decoded.bbox.y_max, 140.0);
    assert_eq!(decoded.font_size, Some(12.0));
    assert_eq!(
        decoded.color,
        Some(sdocx::Color {
            r: 0x12,
            g: 0x34,
            b: 0x56
        })
    );
    assert_eq!(decoded.spans.len(), 4);
    assert_eq!(
        (
            decoded.runs[0].start,
            decoded.runs[0].end,
            decoded.runs[0].bold
        ),
        (1, 2, true)
    );
    assert_eq!(
        (
            decoded.runs[1].start,
            decoded.runs[1].end,
            decoded.runs[1].italic
        ),
        (2, 3, true)
    );
    let stored = &parsed.stored_pages[0].page.layers.layers[0].objects[0];
    assert_eq!(stored.base_metadata(&raw).unwrap().uuid, "tx");
    assert_eq!(stored.base_metadata(&raw).unwrap().bbox, decoded.bbox);
    #[cfg(feature = "render")]
    {
        let svg = sdocx::render_page_svg(&parsed.document, 0, &sdocx::RenderOptions::default())
            .unwrap()
            .svg;
        assert!(svg.contains("rotate(30.00 80.00 90.00)"));
        assert!(svg.contains("x=\"-20.00\""));
        assert!(svg.contains("fill=\"#123456\""));
        assert!(svg.contains("font-weight=\"bold\">😀"));
        assert!(svg.contains("font-style=\"italic\">中"));
    }
}

#[test]
fn reads_text_across_layers_and_children_alongside_strokes() {
    let mut stroke = base([0.0, 0.0, 1.0, 1.0], 0.0);
    stroke.extend(frame(1, &[], &[0, 0, 1, 0], &[])); // empty uncompressed stroke
    let child = object(2, &simple("child"), &[]);
    let unknown = object(250, &simple("decoy text"), &[]);
    let bytes = archive(&page(
        &[
            vec![
                object(1, &stroke, &[]),
                object(4, b"group", &[child]),
                unknown,
            ],
            vec![object(2, &simple("second layer"), &[])],
        ],
        0,
        &[],
    ));
    let parsed = sdocx::parse_bytes_detailed(&bytes).unwrap();
    let page = &parsed.document.pages[0];
    assert_eq!(page.strokes.len(), 1);
    assert_eq!(page.elements.len(), 2);
    assert_eq!(text_box(&page.elements[0]).text, "child");
    assert_eq!(text_box(&page.elements[1]).text, "second layer");
    assert!(
        parsed
            .report
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::UnknownObjectType)
    );
}

#[test]
fn malformed_text_frames_cannot_borrow_from_the_next_frame_or_object() {
    let valid = simple("bounded");
    let shape_offset = base([0.0; 4], 0.0).len();
    let text_offset = shape_offset + frame(6, &[], &[], &[]).len();
    let common_offset = text_offset + 14;
    let mut mutations = Vec::new();
    for (offset, value) in [
        (text_offset, u32::MAX),
        (text_offset + 6, 13),
        (common_offset, 1000),
        (common_offset + 4, 1000),
    ] {
        let mut data = valid.clone();
        data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        mutations.push(data);
    }
    for offset in [shape_offset + 4, text_offset + 4, valid.len() - 9] {
        let mut data = valid.clone();
        data[offset..offset + 2].copy_from_slice(&3_i16.to_le_bytes());
        mutations.push(data);
    }
    for end in 0..valid.len() {
        mutations.push(valid[..end].to_vec());
    }
    for invalid in mutations {
        let bytes = archive(&page(
            &[vec![object(2, &invalid, &[]), object(2, &valid, &[])]],
            0,
            &[],
        ));
        let error = sdocx::parse_bytes(&bytes).unwrap_err();
        assert!(matches!(error, Error::Format(_)), "{error}");
        assert!(
            error.to_string().contains("page page: text box at 0x"),
            "{error}"
        );
    }
}

#[test]
fn reports_borders_and_unknown_styles_while_retaining_readable_text() {
    let unknown = span(999, 0, 1, &[0xde, 0xad]);
    let mut border = 0xff123456_u32.to_le_bytes().to_vec();
    border.extend_from_slice(&2.0_f32.to_le_bytes());
    border.extend_from_slice(&1_u16.to_le_bytes());
    let payload = text_payload(
        &common("中", &[unknown]),
        [0.0, 0.0, 100.0, 100.0],
        0.0,
        &frame(2, &[14, 0], &[], &border),
    );
    let parsed = sdocx::parse_bytes_detailed(&single(&payload)).unwrap();
    let decoded = text_box(&parsed.document.pages[0].elements[0]);
    assert_eq!(decoded.text, "中");
    assert_eq!(decoded.spans[0].kind, RichTextSpanType::Other(999));
    assert_eq!(decoded.spans[0].payload, [0xde, 0xad]);
    let findings: Vec<_> = parsed
        .report
        .diagnostics
        .iter()
        .filter(|d| d.code == DiagnosticCode::UnsupportedTextBoxFeature)
        .collect();
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].archive_entry.as_deref(), Some("page.page"));
    assert!(findings[0].message.contains("text box at 0x"));
    assert!(findings[0].message.contains("unknown text style spans"));
    assert!(findings[0].message.contains("border"));
}

#[test]
fn applies_text_limits_before_allocating_declared_contents() {
    let options = ParseOptions {
        limits: ParseLimits {
            max_text_characters: 2,
            max_text_spans: 0,
            ..ParseLimits::default()
        },
        ..Default::default()
    };
    assert!(matches!(
        sdocx::parse_bytes_with_options(&single(&simple("a😀")), &options),
        Err(Error::LimitExceeded {
            limit: 2,
            actual: 3,
            ..
        })
    ));
    let payload = text_payload(
        &common("a", &[span(5, 0, 1, &[1, 0])]),
        [0.0; 4],
        0.0,
        &frame(2, &[], &[], &[]),
    );
    assert!(matches!(
        sdocx::parse_bytes_with_options(&single(&payload), &options),
        Err(Error::LimitExceeded {
            resource: "text spans",
            limit: 0,
            actual: 1
        })
    ));
}

fn embedded_common(kind: u32, payload: &[u8]) -> Vec<u8> {
    let mut bytes = common("\u{fffc}", &[]);
    bytes.truncate(bytes.len() - 8);
    for value in [
        1_u32,
        0,
        1,
        (payload.len() + 20) as u32,
        payload.len() as u32,
        kind,
    ] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(payload);
    for value in [0_u32, 0, 2] {
        // UTF-16 index, layout option, constraint
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

#[test]
fn preserves_unsupported_inline_objects_and_reports_the_omission() {
    let payload = text_payload(
        &embedded_common(250, b"unknown object bytes"),
        [0.0; 4],
        0.0,
        &frame(2, &[], &[], &[]),
    );
    let parsed = sdocx::parse_bytes_detailed(&single(&payload)).unwrap();
    let decoded = text_box(&parsed.document.pages[0].elements[0]);
    assert_eq!(decoded.text, "\u{fffc}");
    assert_eq!(decoded.object_spans[0].object_data, b"unknown object bytes");
    assert_eq!(
        decoded.object_spans[0].object_type,
        sdocx::ObjectType::Other(250)
    );
    assert!(decoded.object_spans[0].content.is_none());
    assert!(
        parsed
            .report
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::UnsupportedTextBoxFeature
                && d.message.contains("unsupported embedded text objects"))
    );
    let options = ParseOptions {
        limits: ParseLimits {
            max_text_object_spans: 0,
            ..ParseLimits::default()
        },
        ..Default::default()
    };
    assert!(matches!(
        sdocx::parse_bytes_with_options(&single(&payload), &options),
        Err(Error::LimitExceeded {
            resource: "text object spans",
            ..
        })
    ));
}

#[test]
fn bounds_recursion_through_embedded_code_block_text() {
    // text -> code -> text -> code -> text: five object levels. These inner
    // objects are length-prefixed text spans, not StoredObject children.
    let mut text = text_payload(&common("leaf", &[]), [0.0; 4], 0.0, &[]);
    for _ in 0..2 {
        let mut code = base([0.0; 4], 0.0);
        let mut body = (text.len() as u32).to_le_bytes().to_vec();
        body.extend(text);
        code.extend(frame(23, &[2], &[], &body));
        text = text_payload(&embedded_common(23, &code), [0.0; 4], 0.0, &[]);
    }
    text.extend(frame(2, &[], &[], &[]));
    let bytes = single(&text);
    let mut options = ParseOptions {
        limits: ParseLimits {
            max_object_nesting_depth: 5,
            ..ParseLimits::default()
        },
        ..Default::default()
    };
    let parsed = sdocx::parse_bytes_with_options(&bytes, &options).unwrap();
    let mut decoded = text_box(&parsed.pages[0].elements[0]);
    for _ in 0..2 {
        let Some(sdocx::RichTextObjectContent::CodeBlock(code)) = &decoded.object_spans[0].content
        else {
            panic!("expected code block")
        };
        decoded = code.body.as_ref().unwrap();
    }
    assert_eq!(decoded.text, "leaf");
    options.limits.max_object_nesting_depth = 4;
    assert!(matches!(
        sdocx::parse_bytes_with_options(&bytes, &options),
        Err(Error::LimitExceeded {
            resource: "rich-text object nesting depth",
            limit: 4,
            actual: 5
        })
    ));
}

#[test]
fn preserves_paragraphs_and_sections_without_treating_payloads_as_text() {
    let mut data = common("one\ntwo", &[]);
    let paragraph_offset = 4 + 7 * 2 + 4;
    data[paragraph_offset..paragraph_offset + 4].copy_from_slice(&1_u32.to_le_bytes());
    let mut paragraph = 16_u16.to_le_bytes().to_vec();
    for value in [3_u32, 0, 1, 2] {
        // alignment, range, centered
        paragraph.extend_from_slice(&value.to_le_bytes());
    }
    data.splice(paragraph_offset + 4..paragraph_offset + 4, paragraph);
    let section_offset = data.len() - 10;
    data[section_offset..section_offset + 2].copy_from_slice(&1_u16.to_le_bytes());
    let mut section = 0_u32.to_le_bytes().to_vec();
    section.extend_from_slice(&7_u32.to_le_bytes());
    data.splice(section_offset + 2..section_offset + 2, section);
    let payload = text_payload(&data, [0.0; 4], 0.0, &frame(2, &[], &[], &[]));
    let doc = sdocx::parse_bytes(&single(&payload)).unwrap();
    let decoded = text_box(&doc.pages[0].elements[0]);
    assert_eq!(decoded.text, "one\ntwo");
    assert_eq!(
        decoded.paragraphs[0].alignment(),
        Some(sdocx::ParagraphAlignment::Center)
    );
    assert_eq!(decoded.text_sections[0].length_utf16, 7);
    let options = ParseOptions {
        limits: ParseLimits {
            max_text_paragraphs: 0,
            ..ParseLimits::default()
        },
        ..Default::default()
    };
    assert!(matches!(
        sdocx::parse_bytes_with_options(&single(&payload), &options),
        Err(Error::LimitExceeded {
            resource: "text paragraphs",
            ..
        })
    ));
}

#[test]
fn future_fields_and_frames_are_bounded_and_reported() {
    for tail in [
        frame(2, &[0, 0, 0, 0, 1], &[], b"future border"),
        frame(2, &[], &[], &[]),
    ] {
        let mut data = common("kept", &[]);
        let flags_offset = data.len() - 8;
        data[flags_offset..flags_offset + 4].copy_from_slice(&2_u32.to_le_bytes());
        let mut payload = text_payload(&data, [0.0; 4], 0.0, &tail);
        payload.extend(frame(123, &[], &[], &[]));
        let parsed = sdocx::parse_bytes_detailed(&single(&payload)).unwrap();
        assert_eq!(text_box(&parsed.document.pages[0].elements[0]).text, "kept");
        let finding = parsed
            .report
            .diagnostics
            .iter()
            .find(|d| d.code == DiagnosticCode::UnsupportedTextBoxFeature)
            .unwrap();
        assert!(finding.message.contains("text-common extension data"));
        assert!(finding.message.contains("additional text box frames"));
    }
}

#[test]
fn empty_text_without_a_common_field_is_a_valid_object() {
    let mut payload = base([0.0; 4], 0.0);
    payload.extend(frame(6, &[], &[], &[]));
    payload.extend(frame(7, &[], &[], &[]));
    payload.extend(frame(2, &[], &[], &[]));
    let doc = sdocx::parse_bytes(&single(&payload)).unwrap();
    assert!(text_box(&doc.pages[0].elements[0]).text.is_empty());
}

#[test]
fn rejects_non_finite_placement_and_truncated_declared_borders() {
    for bbox in [[f64::NAN, 0.0, 1.0, 1.0], [0.0, 0.0, f64::INFINITY, 1.0]] {
        let payload = text_payload(&common("a", &[]), bbox, 0.0, &frame(2, &[], &[], &[]));
        assert!(matches!(
            sdocx::parse_bytes(&single(&payload)),
            Err(Error::Format(_))
        ));
    }
    for rotation in [f32::NAN, f32::INFINITY] {
        let payload = text_payload(
            &common("a", &[]),
            [0.0; 4],
            rotation,
            &frame(2, &[], &[], &[]),
        );
        assert!(matches!(
            sdocx::parse_bytes(&single(&payload)),
            Err(Error::Format(_))
        ));
    }
    for border in [
        &[][..],
        &[0; 3],
        &f32::NAN.to_le_bytes(),
        &(-1.0_f32).to_le_bytes(),
    ] {
        let payload = text_payload(
            &common("a", &[]),
            [0.0; 4],
            0.0,
            &frame(2, &[4], &[], border),
        );
        assert!(matches!(
            sdocx::parse_bytes(&single(&payload)),
            Err(Error::Format(_))
        ));
    }
    let mut data = common("a", &[]);
    data[14..18].copy_from_slice(&f32::NAN.to_le_bytes());
    let payload = text_payload(&data, [0.0; 4], 0.0, &frame(2, &[], &[], &[]));
    assert!(matches!(
        sdocx::parse_bytes(&single(&payload)),
        Err(Error::Format(_))
    ));
}
