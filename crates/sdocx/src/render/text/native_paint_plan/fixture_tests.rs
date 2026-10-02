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
struct Capture {
    dependencies: Dependencies,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Dependencies {
    font_files: Vec<FontFile>,
}

#[derive(Deserialize)]
struct FontFile {
    file: String,
    sha256: String,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    text_utf8: String,
    native_text_utf8: Option<String>,
    source_utf16: Vec<u16>,
    requested_font_size_bits: Option<u32>,
    supplied_margins_bits: Option<[u32; 4]>,
    measure_widths: Vec<i32>,
    emission_input: EmissionInput,
    emitted_runs: Emission,
}

#[derive(Deserialize)]
struct EmissionInput {
    layout_paragraphs: Vec<ExpectedParagraph>,
}

#[derive(Deserialize)]
struct ExpectedParagraph {
    source_start_utf16: u32,
    source_length_utf16: u32,
    inverse_logical_map_utf16: Vec<usize>,
}

#[derive(Deserialize)]
struct Emission {
    accepted: bool,
    runs: Vec<ExpectedRun>,
}

#[derive(Deserialize)]
struct ExpectedRun {
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

fn capture() -> Capture {
    let bytes = include_bytes!("../../../../../../conformance/table-text-cell-emission.json");
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "0098cfcd93274b210892fd0653677fd4cef3a35ebc4f0419b4b661857cbfa4ce"
    );
    let capture: Capture = serde_json::from_slice(bytes).unwrap();
    assert_eq!(capture.cases.len(), 17);
    capture
}

fn model_content(case: &Case) -> RichTextBox {
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
enum PreparationError {
    Layout(ObjectDiagnosticKind),
    Paint(NativePaintPlanUnavailable),
}

fn prepare(case: &Case, fonts: &FontBook) -> Result<NativePaintPlan, PreparationError> {
    let content = model_content(case);
    prepare_content(&content, case.measure_widths[0], fonts)
}

fn prepare_content(
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

fn check_plan(case: &Case, actual: &NativePaintPlan, font_hash: &str) {
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
            [*actual.source_utf16.start(), *actual.source_utf16.end()],
            expected.range_inclusive,
            "{}: emitted source",
            case.name
        );
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

#[test]
fn rust_cell_producer_matches_actual_native_paint_runs_and_paragraphs() {
    let capture = capture();
    let fonts = FontBook::default();
    let font_hash = &capture
        .dependencies
        .font_files
        .iter()
        .find(|font| font.file == "Roboto-Regular.ttf")
        .unwrap()
        .sha256;
    let mut counts = [0; 3];
    for case in &capture.cases {
        if [
            "supplementary-wrap",
            "tab-newline",
            "mixed-hebrew-direction1",
            "empty-auto",
            "newline-only",
        ]
        .contains(&case.name.as_str())
        {
            continue;
        }
        assert!(
            case.emitted_runs.accepted,
            "{}: native admission",
            case.name
        );
        let actual =
            prepare(case, &fonts).unwrap_or_else(|error| panic!("{}: {error:?}", case.name));
        check_plan(case, &actual, font_hash);
        counts[0] += 1;
        counts[1] += actual.runs.len();
        counts[2] += actual
            .runs
            .iter()
            .map(|run| run.glyphs.len())
            .sum::<usize>();
    }
    assert_eq!(counts, [12, 20, 50]);
}

#[test]
fn rust_cell_producer_keeps_newline_only_admission_and_source_maps() {
    let capture = capture();
    let case = capture
        .cases
        .iter()
        .find(|case| case.name == "newline-only")
        .unwrap();
    assert!(case.emitted_runs.accepted);
    assert!(case.emitted_runs.runs.is_empty());
    let actual = prepare(case, &FontBook::default()).unwrap();
    check_plan(case, &actual, "");
}

#[test]
fn rust_cell_producer_returns_typed_fallback_for_uncertified_inputs() {
    let capture = capture();
    let fonts = FontBook::default();
    let names = [
        "supplementary-wrap",
        "tab-newline",
        "mixed-hebrew-direction1",
    ];
    for name in names {
        let case = capture.cases.iter().find(|case| case.name == name).unwrap();
        let error = prepare(case, &fonts).unwrap_err();
        match error {
            PreparationError::Paint(NativePaintPlanUnavailable::OutsideCertificate("layout")) => {}
            PreparationError::Layout(kind) => panic!("{name}: unexpected layout failure {kind:?}"),
            PreparationError::Paint(error) => panic!("{name}: unexpected paint failure {error:?}"),
        }
    }
}

#[test]
fn rust_cell_producer_keeps_native_empty_source_rejection() {
    let capture = capture();
    let case = capture
        .cases
        .iter()
        .find(|case| case.name == "empty-auto")
        .unwrap();
    assert!(!case.emitted_runs.accepted);
    assert!(case.emitted_runs.runs.is_empty());
    assert!(matches!(
        prepare(case, &FontBook::default()),
        Err(PreparationError::Paint(
            NativePaintPlanUnavailable::OutsideCertificate("empty source")
        ))
    ));
}

#[test]
fn checked_none_bullet_requires_compatibility_paint_without_invalidating_layout() {
    let capture = capture();
    let case = capture
        .cases
        .iter()
        .find(|case| case.name == "subpixel-auto")
        .unwrap();
    let font_hash = &capture
        .dependencies
        .font_files
        .iter()
        .find(|font| font.file == "Roboto-Regular.ttf")
        .unwrap()
        .sha256;
    let fonts = FontBook::default();
    let mut content = model_content(case);
    content.paragraphs.push(RichTextParagraph {
        kind: RichTextParagraphType::Bullet,
        start_paragraph: 0,
        end_paragraph: 1,
        payload: [0_u32.to_le_bytes(); 4].concat(),
    });
    let actual = prepare_content(&content, case.measure_widths[0], &fonts).unwrap();
    check_plan(case, &actual, font_hash);

    content.paragraphs.last_mut().unwrap().payload[8..12].copy_from_slice(&1_u32.to_le_bytes());
    assert!(matches!(
        prepare_content(&content, case.measure_widths[0], &fonts),
        Err(PreparationError::Paint(
            NativePaintPlanUnavailable::OutsideCertificate("layout")
        ))
    ));
}
