use super::*;
use crate::fonts::FontBook;
use crate::{RichTextBox, RichTextParagraph, RichTextParagraphType};

#[test]
fn parsed_native_indent_remains_outside_paper_certificate() {
    let receipt: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../../../../../../conformance/pdf-paper-parsed-indent-native.json"
    ))
    .unwrap();
    let record = &receipt["serialized_paragraph_record"];
    let settings = super::super::super::TextSettings {
        scale: 3.0,
        ..Default::default()
    };
    for case in receipt["cases"].as_array().unwrap() {
        let input = &case["input"];
        let content = RichTextBox {
            text_area_type: None,
            bbox: BoundingBox::default(),
            rotation_degrees: None,
            text: input["source_utf8"].as_str().unwrap().into(),
            color: None,
            highlight_color: None,
            underline: false,
            font_size: None,
            runs: Vec::new(),
            spans: Vec::new(),
            paragraphs: vec![RichTextParagraph {
                kind: RichTextParagraphType::IndentLevel,
                start_paragraph: record["start_paragraph"].as_u64().unwrap() as u32,
                end_paragraph: record["end_paragraph"].as_u64().unwrap() as u32,
                payload: [
                    (record["level"].as_u64().unwrap() as u32).to_le_bytes(),
                    (record["direction"].as_u64().unwrap() as u32).to_le_bytes(),
                ]
                .concat(),
            }],
            object_spans: Vec::new(),
            text_sections: Vec::new(),
            margins: None,
            gravity: None,
        };
        assert!(
            case["produced"]["model_source"]["paragraphs"]
                .as_array()
                .unwrap()
                .iter()
                .any(|paragraph| paragraph["kind"] == record["kind"]
                    && paragraph["paragraph_index_range"]
                        == serde_json::json!([record["start_paragraph"], record["end_paragraph"]])
                    && paragraph["indent"]["level"] == record["level"]
                    && paragraph["indent"]["direction"] == record["direction"])
        );
        let width = f32::from_bits(input["requested_width_bits"].as_u64().unwrap() as u32) as i32;
        let styled = StyledText::new(&content, super::super::super::TextContext::Flow, settings);
        assert!(NativePageLayout::checked_source_length(&styled, width, true).is_err());
        if case["name"] == "scale3-wide" {
            let fonts = FontBook::default();
            let renderer = TextRenderer::new(settings, &fonts);
            let frame = TextFrame {
                bbox: BoundingBox {
                    x_min: 0.0,
                    y_min: 0.0,
                    x_max: f64::from(width),
                    y_max: 0.0,
                },
                gravity: None,
                exclusions: &[],
            };
            let layout = try_layout_text_with_context(
                &styled,
                frame,
                RenderTheme::for_canvas(false),
                &renderer,
                LayoutContext::Capture,
                None,
                Some(NativeCellMeasurement {
                    width: NativeCellMeasurementWidth::Fixed(width),
                    height_limit: f32::INFINITY,
                }),
            )
            .unwrap();
            let native_x = f32::from_bits(
                case["produced"]["lines"][0]["background_rect_bits"][0]
                    .as_u64()
                    .unwrap() as u32,
            );
            assert_eq!(native_x, 16.0);
            assert_eq!(layout.lines[0].x, 48.0);
        }
    }
}
