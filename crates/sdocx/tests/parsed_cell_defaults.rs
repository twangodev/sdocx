use sdocx::{RichTextBox, RichTextObjectContent, RichTextSpanType, SpanIntervalType, StoredNote};
use serde::Deserialize;

#[derive(Deserialize)]
struct Capture {
    cases: Vec<NativeCase>,
}

#[derive(Deserialize)]
struct NativeCase {
    name: String,
    serialized_common_present: bool,
    serialized_payload_bytes: Vec<u8>,
    parsed: NativeSource,
    #[cfg(feature = "render")]
    measured_entry_font_size_bits: Vec<u32>,
}

#[derive(Deserialize)]
struct NativeSource {
    text: Option<String>,
    font_size_spans: Vec<NativeFontSpan>,
    foreground_spans: Vec<NativeColorSpan>,
}

#[derive(Debug, Deserialize, PartialEq)]
struct NativeFontSpan {
    start: u32,
    end: u32,
    interval: u32,
    font_size_bits: u32,
}

#[derive(Debug, Deserialize, PartialEq)]
struct NativeColorSpan {
    start: u32,
    end: u32,
    interval: u32,
    color_argb: u32,
    color_type: u32,
}

fn cases() -> Vec<NativeCase> {
    serde_json::from_str::<Capture>(include_str!(
        "../../../conformance/table-parsed-cell-text.json"
    ))
    .unwrap()
    .cases
}

fn frame(kind: i16, fields: &[u8], fixed: &[u8], flexible: &[u8]) -> Vec<u8> {
    let offset = 12 + fields.len() + fixed.len();
    [
        ((offset + flexible.len()) as u32).to_le_bytes().to_vec(),
        kind.to_le_bytes().to_vec(),
        (offset as u32).to_le_bytes().to_vec(),
        vec![0, fields.len() as u8],
        fields.to_vec(),
        fixed.to_vec(),
        flexible.to_vec(),
    ]
    .concat()
}

fn object_base() -> Vec<u8> {
    let fixed = [
        5500_u32.to_le_bytes().to_vec(),
        0_u16.to_le_bytes().to_vec(),
        0_i64.to_le_bytes().to_vec(),
        [0.0_f64, 0.0, 400.0, 1000.0]
            .into_iter()
            .flat_map(f64::to_le_bytes)
            .collect(),
        vec![0; 5],
    ]
    .concat();
    frame(0, &[], &fixed, &[])
}

fn text_object(common_present: bool, payload: &[u8]) -> Vec<u8> {
    [
        object_base(),
        frame(6, &[], &[], &[]),
        frame(7, &[u8::from(common_present)], &[], payload),
    ]
    .concat()
}

fn sized(data: &[u8]) -> Vec<u8> {
    [(data.len() as u32).to_le_bytes().as_slice(), data].concat()
}

fn table_object(cell_text: &[u8]) -> Vec<u8> {
    let cell = [
        vec![0; 6],
        [0_u32, 1, 1, 0]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect(),
        [0.0_f64, 0.0, 400.0, 1000.0]
            .into_iter()
            .flat_map(f64::to_le_bytes)
            .collect(),
        vec![0],
        sized(cell_text),
    ]
    .concat();
    let row = [
        vec![0; 6],
        1000_f32.to_le_bytes().to_vec(),
        0_u32.to_le_bytes().to_vec(),
        1_u32.to_le_bytes().to_vec(),
        sized(&cell),
    ]
    .concat();
    let flexible = [
        1_u32.to_le_bytes().to_vec(),
        400_f32.to_le_bytes().to_vec(),
        1_u32.to_le_bytes().to_vec(),
        sized(&row),
    ]
    .concat();
    [object_base(), frame(22, &[12], &[], &flexible)].concat()
}

fn body_object(table: &[u8]) -> Vec<u8> {
    let object_span = [
        (table.len() as u32).to_le_bytes().to_vec(),
        22_u32.to_le_bytes().to_vec(),
        table.to_vec(),
        [0_u32, 1, 2]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect(),
    ]
    .concat();
    let common = [
        1_u32.to_le_bytes().to_vec(),
        0xfffc_u16.to_le_bytes().to_vec(),
        vec![0; 8 + 16 + 1 + 2],
        [1_u32, 0, 1]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect(),
        sized(&object_span),
    ]
    .concat();
    text_object(true, &sized(&common))
}

fn note_binary(case: &NativeCase) -> Vec<u8> {
    let title = text_object(
        case.serialized_common_present,
        &case.serialized_payload_bytes,
    );
    let body = body_object(&table_object(&title));
    let mut note = [
        vec![0; 6],
        5500_u32.to_le_bytes().to_vec(),
        0_u16.to_le_bytes().to_vec(),
        0_u32.to_le_bytes().to_vec(),
        vec![0; 16],
        [500_u32, 1600, 0, 0, 4000]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect(),
        sized(&title),
        sized(&body),
    ]
    .concat();
    let length = note.len() as u32;
    note[..4].copy_from_slice(&length.to_le_bytes());
    note
}

fn parse_case(case: &NativeCase) -> StoredNote {
    sdocx::parse_note_bytes(&note_binary(case)).unwrap()
}

fn cell(body: &RichTextBox) -> &RichTextBox {
    let Some(RichTextObjectContent::Table(table)) = &body.object_spans[0].content else {
        panic!("expected parsed table");
    };
    &table.rows[0].cells[0].content
}

#[test]
fn parsed_cells_retain_native_constructor_spans_only_for_zero_serialized_count() {
    for case in cases() {
        let note = parse_case(&case);
        let content = cell(&note.body);
        assert_eq!(content.text, case.parsed.text.as_deref().unwrap_or(""));
        if case.name == "nonempty-caret-font50" {
            assert!(case.parsed.font_size_spans.is_empty());
            assert_eq!(content.spans.len(), 1);
            assert_eq!(content.spans[0].start_utf16, 0);
            assert_eq!(content.spans[0].end_utf16, 0);
            assert_eq!(content.spans[0].font_size_value(), Some(50.0));
            continue;
        }
        let fonts: Vec<_> = content
            .spans
            .iter()
            .filter(|span| span.kind == RichTextSpanType::FontSize)
            .map(|span| NativeFontSpan {
                start: span.start_utf16,
                end: span.end_utf16,
                interval: span.interval_type.raw(),
                font_size_bits: span.font_size_value().unwrap().to_bits(),
            })
            .collect();
        let colors: Vec<_> = content
            .spans
            .iter()
            .filter(|span| span.kind == RichTextSpanType::ForegroundColor)
            .map(|span| NativeColorSpan {
                start: span.start_utf16,
                end: span.end_utf16,
                interval: span.interval_type.raw(),
                color_argb: span.argb_value().unwrap(),
                color_type: u32::from_le_bytes(span.payload[4..8].try_into().unwrap()),
            })
            .collect();
        assert_eq!(fonts, case.parsed.font_size_spans, "{}", case.name);
        assert_eq!(colors, case.parsed.foreground_spans, "{}", case.name);
        if case.name.ends_with("common-no-spans") {
            assert!(note.title.spans.is_empty());
            assert_eq!(note.title.font_size, None);
            assert_eq!(note.title.color, None);
        }
        assert!(note.body.spans.is_empty());
    }
}

#[test]
fn supplied_unknown_span_payload_and_interval_survive_cell_loading() {
    let mut case = cases()
        .into_iter()
        .find(|case| case.name == "nonempty-font50")
        .unwrap();
    let data = &mut case.serialized_payload_bytes;
    data[18..22].copy_from_slice(&0xffff_u32.to_le_bytes());
    data[30..34].copy_from_slice(&99_u32.to_le_bytes());
    data[34..42].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
    let note = parse_case(&case);
    let content = cell(&note.body);
    assert_eq!(content.spans.len(), 1);
    let span = &content.spans[0];
    assert_eq!(span.kind, RichTextSpanType::Other(0xffff));
    assert_eq!(span.interval_type, SpanIntervalType::Other(99));
    assert_eq!(span.payload, [1, 2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(content.font_size, None);
    assert_eq!(content.color, None);
    #[cfg(feature = "serde")]
    {
        let source = serde_json::to_value(&note.body).unwrap();
        let restored: RichTextBox = serde_json::from_value(source.clone()).unwrap();
        assert_eq!(serde_json::to_value(restored).unwrap(), source);
    }
}

#[test]
fn retained_constructor_spans_respect_the_effective_object_span_limit() {
    let case = cases()
        .into_iter()
        .find(|case| case.name == "nonempty-common-no-spans")
        .unwrap();
    let data = note_binary(&case);
    for limit in [0, 1, 2] {
        let result = sdocx::parse_note_bytes_with_limits(
            &data,
            &sdocx::ParseLimits {
                max_text_spans: limit,
                ..Default::default()
            },
        );
        if limit < 2 {
            assert!(matches!(
                result,
                Err(sdocx::Error::LimitExceeded {
                    resource: "text spans",
                    limit: actual_limit,
                    actual: 2,
                }) if actual_limit == limit as u64
            ));
        } else {
            assert_eq!(cell(&result.unwrap().body).spans.len(), 2);
        }
    }
}

#[cfg(feature = "render")]
fn document(body: RichTextBox) -> sdocx::Document {
    sdocx::Document {
        pages: vec![sdocx::Page {
            uuid: "parsed-cell".into(),
            width: 500,
            height: 1600,
            content_bbox: Default::default(),
            background_color: None,
            template: None,
            background: Default::default(),
            objects: vec![],
        }],
        metadata: sdocx::DocumentMetadata {
            note_text: Some(body),
            page_mode: Some(0),
            default_page_dimensions: Some((360, 1600)),
            orientation: Some(0),
            flow_page_padding: Some((0, 0)),
            ..Default::default()
        },
    }
}

#[cfg(feature = "render")]
#[test]
fn parsed_native_cell_sizes_reach_normal_and_replay_vector_svg() {
    for case in cases() {
        let document = document(parse_case(&case).body);
        let source = document.metadata.note_text.clone();
        let layout = sdocx::layout_document(&document);
        for replay in [false, true] {
            let page = if replay {
                sdocx::render_layout_page_replay_svg(&document, &layout, 0, &Default::default())
            } else {
                sdocx::render_layout_page_svg(&document, &layout, 0, &Default::default())
            }
            .unwrap();
            assert!(
                page.object_diagnostics.is_empty(),
                "{}: {:?}",
                case.name,
                page.object_diagnostics
            );
            assert!(
                page.text_diagnostics.is_empty(),
                "{}: {:?}",
                case.name,
                page.text_diagnostics
            );
            let xml = roxmltree::Document::parse(&page.svg).unwrap();
            assert!(!xml.descendants().any(|node| node.has_tag_name("image")));
            let text = xml
                .descendants()
                .find(|node| node.has_tag_name("tspan") && node.text() == Some("AV"));
            if let Some(&size) = case.measured_entry_font_size_bits.first() {
                let text = text.unwrap();
                let size_attribute = text
                    .ancestors()
                    .find_map(|node| node.attribute("font-size"))
                    .unwrap();
                assert_eq!(
                    size_attribute.parse::<f32>().unwrap().to_bits(),
                    size,
                    "{}",
                    case.name
                );
                if let Some(color) = case.parsed.foreground_spans.first() {
                    let fill = text
                        .ancestors()
                        .find_map(|node| node.attribute("fill"))
                        .unwrap();
                    assert_eq!(fill, format!("#{:06x}", color.color_argb & 0xffffff));
                }
            } else {
                assert!(text.is_none());
            }
        }
        assert_eq!(document.metadata.note_text, source);
    }
}

#[cfg(feature = "pdf")]
#[path = "support/pdf_geometry.rs"]
mod pdf_geometry;

#[cfg(feature = "pdf")]
#[test]
fn parsed_constructor_styles_export_selectable_vector_pdf() {
    let case = cases()
        .into_iter()
        .find(|case| case.name == "nonempty-common-no-spans")
        .unwrap();
    let document = document(parse_case(&case).body);
    let layout = sdocx::layout_document(&document);
    let page = sdocx::render_layout_page_svg(&document, &layout, 0, &Default::default()).unwrap();
    let bytes = sdocx::render_svg_pages_pdf(&[page], &Default::default()).unwrap();
    let pdf = pdf_geometry::read(&bytes, 96.0);
    assert_eq!(
        pdf.extracted_text.split_whitespace().collect::<String>(),
        "AV"
    );
    assert_eq!(pdf.source, "AV");
    assert_eq!(pdf.image_resources, 0);
    assert!(pdf.images.is_empty());
}
