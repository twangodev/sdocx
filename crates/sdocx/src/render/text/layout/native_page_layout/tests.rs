use super::*;
use crate::fonts::FontBook;
use crate::{Color, RichTextBox};

fn source(input: &serde_json::Value) -> RichTextBox {
    RichTextBox {
        text_area_type: None,
        bbox: BoundingBox::default(),
        rotation_degrees: None,
        text: input["source_utf8"].as_str().unwrap().into(),
        color: Some(Color { r: 0, g: 0, b: 0 }),
        highlight_color: None,
        underline: false,
        font_size: Some(
            input["requested_font_size_bits"]
                .as_u64()
                .map_or(17.0, |bits| f32::from_bits(bits as u32)),
        ),
        runs: Vec::new(),
        spans: Vec::new(),
        paragraphs: Vec::new(),
        object_spans: Vec::new(),
        text_sections: Vec::new(),
        margins: input["requested_margin_bits"].as_array().map(|margins| {
            std::array::from_fn(|index| f32::from_bits(margins[index].as_u64().unwrap() as u32))
        }),
        gravity: Some(0),
    }
}

fn frame(width: f64) -> TextFrame<'static> {
    TextFrame {
        bbox: BoundingBox {
            x_min: 0.0,
            y_min: 0.0,
            x_max: width,
            y_max: 0.0,
        },
        gravity: None,
        exclusions: &[],
    }
}

#[test]
fn body_measurement_preserves_native_bands_without_admitting_cached_paint() {
    let capture: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../../../../../../conformance/table-bodytext-page-ranges.json"
    ))
    .unwrap();
    let fonts = FontBook::default();
    let mut measured_lines = 0;
    let mut profiles = 0;
    for case in capture["cases"].as_array().unwrap() {
        let content = source(&case["input"]);
        let original = serde_json::to_value(&content).unwrap();
        let width = f64::from(f32::from_bits(
            case["input"]["requested_width_bits"].as_u64().unwrap() as u32,
        ));
        for settings in [
            super::super::super::TextSettings::resolved(),
            super::super::super::TextSettings::default(),
        ] {
            let styled =
                StyledText::new(&content, super::super::super::TextContext::Flow, settings);
            let renderer = TextRenderer::new(settings, &fonts);
            let (layout, native) = layout_native_page_text(
                &styled,
                frame(width),
                RenderTheme::for_canvas(false),
                &renderer,
            );
            assert!(
                layout.native_frame.is_none(),
                "{}: cached paint certificate",
                case["name"]
            );
            if content.margins.is_some_and(|margins| margins != [0.0; 4]) {
                assert!(matches!(
                    native,
                    Err(NativePageIndexUnavailable::OutsideCertificate)
                ));
                continue;
            }
            let native = native.unwrap();
            assert_eq!(
                native.text_length_utf16,
                case["produced"]["text_length_utf16"].as_i64().unwrap() as i32
            );
            let expected = case["produced"]["lines"].as_array().unwrap();
            assert_eq!(
                native.lines.len(),
                expected.len(),
                "{}: lines",
                case["name"]
            );
            for (line, expected) in native.lines.iter().zip(expected) {
                let expected_source: [i32; 2] =
                    serde_json::from_value(expected["source_inclusive_utf16"].clone()).unwrap();
                let expected_background: [u32; 4] =
                    serde_json::from_value(expected["background_rect_bits"].clone()).unwrap();
                assert_eq!(
                    line.source_inclusive_utf16, expected_source,
                    "{}: owner",
                    case["name"]
                );
                assert_eq!(
                    line.top.to_bits(),
                    expected["line_top_bits"].as_u64().unwrap() as u32,
                    "{}: top",
                    case["name"]
                );
                assert_eq!(
                    line.background.map(f32::to_bits),
                    expected_background,
                    "{}: background",
                    case["name"]
                );
                measured_lines += 1;
            }
            let paint = super::super::super::native_paint_plan::native_paint_plan(
                &styled,
                &layout,
                RenderTheme::for_canvas(false),
            );
            assert!(matches!(paint, Err(super::super::super::native_paint_plan::NativePaintPlanUnavailable::OutsideCertificate("layout"))));
            profiles += 1;
        }
        assert_eq!(serde_json::to_value(&content).unwrap(), original);
    }
    assert_eq!((profiles, measured_lines), (20, 54));
}

#[test]
fn genuine_document_zero_outer_height_preserves_ordinary_measured_lines() {
    use sha2::{Digest, Sha256};
    let bytes =
        include_bytes!("../../../../../../../conformance/table-bodytext-one-page-placement.json");
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "cc0810421dceddffaa3454e8010ece57d7afddcce1f67dba1b662eeea4acd15a"
    );
    let capture: serde_json::Value = serde_json::from_slice(bytes).unwrap();
    let fonts = FontBook::default();
    let settings = super::super::super::TextSettings::default();
    let renderer = TextRenderer::new(settings, &fonts);
    let mut cases = 0;
    let mut lines = 0;
    for case in capture["cases"].as_array().unwrap() {
        let native = &case["writer"];
        if native["supplied_text_utf8"].is_null() {
            continue;
        }
        let content = source(&serde_json::json!({
            "source_utf8": native["supplied_text_utf8"],
            "requested_font_size_bits": native["supplied_font_size_setter_bits"],
            "requested_margin_bits": null,
        }));
        let width = native["supplied_page_record"][3].as_i64().unwrap() as f64;
        let styled = StyledText::new(&content, super::super::super::TextContext::Flow, settings);
        let (layout, actual) = layout_native_page_text(
            &styled,
            frame(width),
            RenderTheme::for_canvas(false),
            &renderer,
        );
        assert_eq!(native["widget_layout_height_bits"].as_u64().unwrap(), 0);
        assert_eq!(native["height_without_last_page"].as_i64().unwrap(), 0);
        assert!(layout.native_frame.is_none());
        let actual = actual.unwrap();
        let expected = native["measured_lines"].as_array().unwrap();
        assert_eq!(actual.lines.len(), expected.len());
        for (actual, expected) in actual.lines.iter().zip(expected) {
            assert_eq!(
                actual.source_inclusive_utf16,
                serde_json::from_value::<[i32; 2]>(expected["source_inclusive_utf16"].clone())
                    .unwrap()
            );
            assert_eq!(
                actual.top.to_bits(),
                expected["line_top_bits"].as_u64().unwrap() as u32
            );
            assert_eq!(
                actual.background.map(f32::to_bits),
                serde_json::from_value::<[u32; 4]>(expected["background_rect_bits"].clone())
                    .unwrap()
            );
            lines += 1;
        }
        if native["supplied_font_size_setter_bits"].is_null() {
            assert_eq!(content.font_size, Some(17.0));
            assert!(
                native["source"]["font_size_spans"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
            assert!(
                native["source"]["foreground_spans"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
        }
        cases += 1;
    }
    assert_eq!((cases, lines), (2, 9));
}

fn line_fields(layout: &TextLayout) -> Vec<(std::ops::Range<usize>, [u64; 5])> {
    layout
        .lines
        .iter()
        .map(|line| {
            (
                line.line.source.clone(),
                [
                    line.x,
                    line.width,
                    line.baseline,
                    line.background_top,
                    line.bottom,
                ]
                .map(f64::to_bits),
            )
        })
        .collect()
}

#[test]
fn unsupported_body_profiles_use_the_existing_capture_layout() {
    let fonts = FontBook::default();
    let settings = super::super::super::TextSettings::default();
    let renderer = TextRenderer::new(settings, &fonts);
    let input = serde_json::json!({"source_utf8":"AV abc", "requested_font_size_bits":null, "requested_margin_bits":null});
    for unsupported in 0..4 {
        let mut content = source(&input);
        let mut bbox = frame(90.0).bbox;
        match unsupported {
            0 => {
                bbox.x_min = 48.0;
                bbox.x_max = 138.0;
            }
            1 => content.font_size = Some(24.0),
            2 => content.margins = Some([1.25, 2.5, 3.75, 4.5]),
            3 => content.text = "AV\tbc".into(),
            _ => unreachable!(),
        }
        let styled = StyledText::new(&content, super::super::super::TextContext::Flow, settings);
        let make_frame = || TextFrame {
            bbox,
            gravity: None,
            exclusions: &[],
        };
        let existing = layout_capture_text(
            &styled,
            make_frame(),
            RenderTheme::for_canvas(false),
            &renderer,
        );
        let (actual, pages) = layout_native_page_text(
            &styled,
            make_frame(),
            RenderTheme::for_canvas(false),
            &renderer,
        );
        assert!(matches!(
            pages,
            Err(NativePageIndexUnavailable::OutsideCertificate)
        ));
        assert_eq!(
            line_fields(&actual),
            line_fields(&existing),
            "unsupported {unsupported}"
        );
        assert!(actual.native_frame.is_none());
    }
}
