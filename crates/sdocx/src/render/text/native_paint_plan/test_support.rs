use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::*;
use crate::fonts::FontBook;
use crate::render::text::{
    NativeCellTextConstraints, TextContext, TextFrame, TextRenderer, TextSettings,
    layout_table_cell_text,
};
use crate::{
    BoundingBox, Color, ObjectDiagnosticKind, RichTextBox, RichTextParagraph, RichTextParagraphType,
};

#[derive(Deserialize)]
pub(super) struct Capture {
    pub(super) dependencies: Dependencies,
    pub(super) cases: Vec<Case>,
}

#[derive(Deserialize)]
pub(super) struct Dependencies {
    pub(super) font_files: Vec<FontFile>,
}

#[derive(Deserialize)]
pub(super) struct FontFile {
    pub(super) file: String,
    pub(super) sha256: String,
}

#[derive(Deserialize)]
pub(super) struct Case {
    pub(super) name: String,
    pub(super) text_utf8: String,
    pub(super) native_text_utf8: Option<String>,
    pub(super) source_utf16: Vec<u16>,
    pub(super) requested_font_size_bits: Option<u32>,
    pub(super) supplied_margins_bits: Option<[u32; 4]>,
    pub(super) measure_widths: Vec<i32>,
    pub(super) emission_input: EmissionInput,
    pub(super) emitted_runs: Emission,
    pub(super) source_input: Option<SourceInput>,
}

#[derive(Deserialize)]
pub(super) struct EmissionInput {
    layout_paragraphs: Vec<ExpectedParagraph>,
}

#[derive(Deserialize)]
pub(super) struct ExpectedParagraph {
    source_start_utf16: u32,
    source_length_utf16: u32,
    inverse_logical_map_utf16: Vec<usize>,
}

#[derive(Deserialize)]
pub(super) struct Emission {
    pub(super) accepted: bool,
    pub(super) runs: Vec<ExpectedRun>,
}

#[derive(Deserialize)]
pub(super) struct ExpectedRun {
    range_inclusive: [usize; 2],
    codewords: Vec<u32>,
    position_bits: Vec<u32>,
    origin_bits: [u32; 2],
    ink_rect_bits: [u32; 4],
    layout_rect_bits: [u32; 4],
    font_size_bits: u32,
    foreground: u32,
    style_byte: u8,
    background: u32,
    is_object: bool,
}

#[derive(Deserialize)]
pub(super) struct SourceInput {
    pub(super) text_utf8: String,
    pub(super) common_present: bool,
    pub(super) font_size_at_utf16_including_end_bits: Vec<u32>,
    pub(super) foreground_at_utf16_including_end_argb: Vec<u32>,
    pub(super) gravity: u32,
    pub(super) margin_bits: [u32; 4],
    pub(super) font_size_spans: Vec<SourceFontSpan>,
    pub(super) foreground_spans: Vec<SourceForegroundSpan>,
    pub(super) paragraphs: Vec<SourceParagraph>,
}

#[derive(Deserialize)]
pub(super) struct SourceFontSpan {
    pub(super) kind: u32,
    pub(super) utf16_range: [u32; 2],
    pub(super) interval: u32,
    pub(super) font_size_bits: u32,
}

#[derive(Deserialize)]
pub(super) struct SourceForegroundSpan {
    pub(super) kind: u32,
    pub(super) utf16_range: [u32; 2],
    pub(super) interval: u32,
    pub(super) color_argb: u32,
    pub(super) color_type: u32,
}

#[derive(Deserialize)]
pub(super) struct SourceParagraph {
    pub(super) kind: u32,
    pub(super) paragraph_index_range: [u32; 2],
    pub(super) alignment: Option<u32>,
}

pub(super) fn model_content(case: &Case) -> RichTextBox {
    assert_eq!(
        case.native_text_utf8.as_deref(),
        (!case.text_utf8.is_empty()).then_some(case.text_utf8.as_str())
    );
    assert_eq!(
        case.text_utf8.encode_utf16().collect::<Vec<_>>(),
        case.source_utf16
    );
    RichTextBox {
        text_area_type: None,
        bbox: BoundingBox::default(),
        rotation_degrees: None,
        text: case.text_utf8.clone(),
        color: Some(Color {
            r: 37,
            g: 37,
            b: 37,
        }),
        highlight_color: None,
        underline: false,
        font_size: Some(case.requested_font_size_bits.map_or(50.0, f32::from_bits)),
        runs: Vec::new(),
        spans: Vec::new(),
        paragraphs: vec![RichTextParagraph {
            kind: RichTextParagraphType::Alignment,
            start_paragraph: 0,
            end_paragraph: 1,
            payload: 2_u32.to_le_bytes().to_vec(),
        }],
        object_spans: Vec::new(),
        text_sections: Vec::new(),
        margins: case
            .supplied_margins_bits
            .map(|bits| bits.map(f32::from_bits)),
        gravity: Some(0),
    }
}

#[derive(Debug)]
pub(super) enum PreparationError {
    Layout(ObjectDiagnosticKind),
    Paint(NativePaintPlanUnavailable),
}

pub(super) fn prepare(case: &Case, fonts: &FontBook) -> Result<NativePaintPlan, PreparationError> {
    let content = model_content(case);
    prepare_content(&content, case.measure_widths[0], fonts)
}

pub(super) fn prepare_content(
    content: &RichTextBox,
    width: i32,
    fonts: &FontBook,
) -> Result<NativePaintPlan, PreparationError> {
    let settings = TextSettings::resolved();
    let styled = StyledText::new(content, TextContext::Flow, settings);
    let renderer = TextRenderer::new(settings, fonts);
    let theme = RenderTheme::for_canvas(false);
    let layout = layout_table_cell_text(
        &styled,
        TextFrame {
            bbox: BoundingBox {
                x_min: 0.0,
                y_min: 0.0,
                x_max: f64::from(width),
                y_max: 1000.0,
            },
            gravity: Some(0),
            exclusions: &[],
        },
        NativeCellTextConstraints {
            width,
            height_limit: 1000.0,
        },
        theme,
        &renderer,
    )
    .map_err(PreparationError::Layout)?;
    native_paint_plan(&styled, &layout, theme).map_err(PreparationError::Paint)
}

pub(super) fn check_plan(case: &Case, actual: &NativePaintPlan, font_hash: &str) {
    assert_eq!(
        Some(actual.source.as_ref()),
        case.native_text_utf8.as_deref()
    );
    assert_eq!(actual.translation, [0.0; 2]);
    assert_eq!(
        actual.paragraphs.len(),
        case.emission_input.layout_paragraphs.len(),
        "{}: paragraph count",
        case.name
    );
    for (actual, expected) in actual
        .paragraphs
        .iter()
        .zip(&case.emission_input.layout_paragraphs)
    {
        assert_eq!(
            actual.source.utf16(),
            &(expected.source_start_utf16
                ..expected.source_start_utf16 + expected.source_length_utf16),
            "{}: paragraph source",
            case.name
        );
        assert_eq!(
            actual.inverse_logical_map_utf16, expected.inverse_logical_map_utf16,
            "{}: inverse paragraph map",
            case.name
        );
    }
    assert_eq!(
        actual.runs.len(),
        case.emitted_runs.runs.len(),
        "{}: run count",
        case.name
    );
    for (actual, expected) in actual.runs.iter().zip(&case.emitted_runs.runs) {
        assert_eq!(
            actual.source.utf16(),
            &(expected.range_inclusive[0] as u32..expected.range_inclusive[1] as u32 + 1),
            "{}: typed source",
            case.name
        );
        assert_eq!(
            actual
                .glyphs
                .iter()
                .map(|glyph| glyph.glyph_id)
                .collect::<Vec<_>>(),
            expected.codewords,
            "{}: glyph IDs",
            case.name
        );
        assert_eq!(
            actual
                .glyphs
                .iter()
                .map(|glyph| glyph.origin[0].to_bits())
                .collect::<Vec<_>>(),
            expected.position_bits,
            "{}: glyph X",
            case.name
        );
        for glyph in &actual.glyphs {
            assert_eq!(
                glyph.origin[1].to_bits(),
                expected.origin_bits[1],
                "{}: glyph Y",
                case.name
            );
            assert!(
                glyph.source.relative_to(&actual.source).is_some(),
                "{}: retained glyph source",
                case.name
            );
        }
        assert_eq!(
            actual.origin.map(f32::to_bits),
            expected.origin_bits,
            "{}: origin",
            case.name
        );
        assert_eq!(
            actual.ink.0.map(f32::to_bits),
            expected.ink_rect_bits,
            "{}: ink rectangle",
            case.name
        );
        assert_eq!(
            actual.layout.0.map(f32::to_bits),
            expected.layout_rect_bits,
            "{}: layout rectangle",
            case.name
        );
        assert_eq!(
            actual.font_size.to_bits(),
            expected.font_size_bits,
            "{}: font size",
            case.name
        );
        assert_eq!(
            actual.foreground, expected.foreground,
            "{}: foreground",
            case.name
        );
        assert_eq!(
            actual.style_bits, expected.style_byte,
            "{}: style",
            case.name
        );
        assert_eq!(
            actual.background, expected.background,
            "{}: background",
            case.name
        );
        assert!(!expected.is_object, "{}: native text profile", case.name);
        assert_eq!(
            format!("{:x}", Sha256::digest(actual.face.bytes())),
            font_hash,
            "{}: physical font",
            case.name
        );
    }
}
