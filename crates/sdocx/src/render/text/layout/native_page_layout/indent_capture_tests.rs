use super::*;
use crate::fonts::FontBook;
use crate::{RichTextBox, RichTextParagraph, RichTextParagraphType};

#[test]
fn parsed_native_indent_paper_certificate_matches_density_three_control() {
    let original: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../../../../../../conformance/pdf-paper-parsed-indent-native.json"
    ))
    .unwrap();
    let density3: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../../../../../../conformance/pdf-paper-parsed-indent-density3-native.json"
    ))
    .unwrap();
    let record = &original["serialized_paragraph_record"];
    let settings = super::super::super::TextSettings {
        scale: 3.0,
        ..Default::default()
    };
    for (receipt, expected_native_x) in [(&original, 16.0), (&density3, 48.0)] {
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
                            == serde_json::json!([
                                record["start_paragraph"],
                                record["end_paragraph"]
                            ])
                        && paragraph["indent"]["level"] == record["level"]
                        && paragraph["indent"]["direction"] == record["direction"])
            );
            let width =
                f32::from_bits(input["requested_width_bits"].as_u64().unwrap() as u32) as i32;
            let styled =
                StyledText::new(&content, super::super::super::TextContext::Flow, settings);
            let admitted = !content.text.contains('\n');
            if admitted {
                assert!(NativePageLayout::checked_source_length(&styled, width, false).is_err());
                let mut variants = Vec::new();
                let mut other = content.clone();
                other.paragraphs[0].end_paragraph = 2;
                variants.push(other);
                let mut other = content.clone();
                other.paragraphs[0].payload[0] = 2;
                variants.push(other);
                let mut other = content.clone();
                other.paragraphs.push(other.paragraphs[0].clone());
                variants.push(other);
                let mut other = content.clone();
                let mut parsing_state = other.paragraphs[0].clone();
                parsing_state.kind = RichTextParagraphType::ParsingState;
                other.paragraphs.push(parsing_state);
                variants.push(other);
                let mut other = content.clone();
                other.font_size = Some(17.0);
                variants.push(other);
                for other in &variants {
                    let rejected =
                        StyledText::new(other, super::super::super::TextContext::Flow, settings);
                    assert!(
                        NativePageLayout::checked_source_length(&rejected, width, true).is_err()
                    );
                }
            }
            assert_eq!(
                NativePageLayout::checked_source_length(&styled, width, true).is_ok(),
                admitted
            );
            if admitted && expected_native_x == 48.0 {
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
                let (layout, result) = layout_native_paper_text(
                    &styled,
                    frame,
                    RenderTheme::for_canvas(false),
                    &renderer,
                );
                let native = result.unwrap();
                assert_eq!(
                    layout.lines.len(),
                    case["produced"]["lines"].as_array().unwrap().len()
                );
                for (line, expected) in layout
                    .lines
                    .iter()
                    .zip(case["produced"]["lines"].as_array().unwrap())
                {
                    let bands = line.native_bands.unwrap();
                    assert_eq!(
                        [
                            line.line.source.start as i32,
                            line.line.source.end as i32 - 1
                        ],
                        serde_json::from_value::<[i32; 2]>(
                            expected["source_inclusive_utf16"].clone()
                        )
                        .unwrap()
                    );
                    let right = line
                        .line
                        .native_placed
                        .as_ref()
                        .unwrap()
                        .positions
                        .last()
                        .unwrap()
                        .right;
                    assert_eq!(
                        [line.x as f32, bands.top, right, bands.bottom].map(f32::to_bits),
                        serde_json::from_value::<[u32; 4]>(
                            expected["background_rect_bits"].clone()
                        )
                        .unwrap()
                    );
                }
                let pages: Vec<_> = case["input"]["page_records"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|page| {
                        let p: Vec<i32> = serde_json::from_value(page.clone()).unwrap();
                        super::super::super::native_page_index::NativePageRecord {
                            cumulative_y: p[0],
                            local_bounds: [p[1], p[2], p[3], p[4]],
                        }
                    })
                    .collect();
                let mut sections =
                    super::super::super::native_page_index::NativePageSections::default();
                native
                    .update_sections(Some(&pages), 0, &mut sections)
                    .unwrap();
                let pairs =
                    |items: &[super::super::super::native_page_index::NativePageSection]| {
                        items.iter().map(|s| [s.start, s.count]).collect::<Vec<_>>()
                    };
                assert_eq!(
                    serde_json::to_value(pairs(&sections.lines)).unwrap(),
                    case["stages"][0]["line_sections"]
                );
                assert_eq!(
                    serde_json::to_value(pairs(&sections.text)).unwrap(),
                    case["stages"][0]["text_sections"]
                );
            }
            if expected_native_x == 48.0 && case["name"] != "scale3-leading" {
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
                assert_eq!(native_x, expected_native_x);
                assert_eq!(layout.lines[0].x, 48.0);
                let expected = case["produced"]["lines"].as_array().unwrap();
                assert_eq!(layout.lines.len(), expected.len(), "{}", case["name"]);
                for (line, native) in layout.lines.iter().zip(expected) {
                    let bands = line.native_bands.unwrap();
                    let rect =
                        serde_json::from_value::<[u32; 4]>(native["background_rect_bits"].clone())
                            .unwrap();
                    assert_eq!(
                        [line.x as f32, bands.top, bands.bottom].map(f32::to_bits),
                        [rect[0], rect[1], rect[3]],
                        "{}",
                        case["name"]
                    );
                }
            }
        }
    }
    let wrapped = &density3["cases"][1]["stages"][0];
    assert_eq!(
        wrapped["line_sections"],
        serde_json::json!([[0, 1], [1, 1], [1, 2], [2, 2], [3, 2], [4, 2]])
    );
    assert_eq!(
        wrapped["text_sections"],
        serde_json::json!([[0, 2], [2, 1], [2, 3], [3, 4], [5, 4], [7, 3]])
    );
}
