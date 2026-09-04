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
    bytes.extend_from_slice(&[1, 0, fields.len() as u8]);
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
