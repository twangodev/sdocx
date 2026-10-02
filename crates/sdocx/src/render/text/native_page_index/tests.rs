use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::*;
use crate::fonts::FontBook;
use crate::render::RenderTheme;
use crate::render::text::{
    NativeCellTextConstraints, TextContext, TextFrame, TextRenderer, TextSettings,
    layout_table_cell_text,
};
use crate::{BoundingBox, Color, RichTextBox};

#[derive(Deserialize)]
struct MeasuredCapture {
    cases: Vec<MeasuredCase>,
}

#[derive(Deserialize)]
struct MeasuredCase {
    name: String,
    input: MeasuredInput,
    produced: Produced,
    boundaries: Vec<Boundary>,
    stages: Vec<Stage>,
}

#[derive(Deserialize)]
struct MeasuredInput {
    source_utf8: String,
    requested_width_bits: u32,
    requested_height_bits: u32,
    requested_font_size_bits: Option<u32>,
    requested_margin_bits: Option<[u32; 4]>,
    page_records: Vec<[i32; 5]>,
}

#[derive(Deserialize)]
struct Boundary {
    page: usize,
    line: i32,
    down: bool,
    up: bool,
}

#[derive(Deserialize)]
struct Stage {
    first_page: usize,
    line_sections: Vec<[i32; 2]>,
    text_sections: Vec<[i32; 2]>,
}

#[derive(Deserialize)]
struct Produced {
    text_length_utf16: i32,
    layout_width: i32,
    lines: Vec<ProducedLine>,
    first_empty_rect_bits: [u32; 4],
    default_cursor_rect_bits: [u32; 4],
}

#[derive(Deserialize)]
struct ProducedLine {
    source_inclusive_utf16: [i32; 2],
    line_top_bits: u32,
    background_rect_bits: [u32; 4],
}

fn measured_capture() -> MeasuredCapture {
    let bytes = include_bytes!("../../../../../../conformance/table-bodytext-page-ranges.json");
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "da1a8415277dd42697822f50921f4579561b90c1079296d0cc5d28afeeeb9d40"
    );
    serde_json::from_slice(bytes).unwrap()
}

fn content(input: &MeasuredInput) -> RichTextBox {
    RichTextBox {
        text_area_type: None,
        bbox: BoundingBox::default(),
        rotation_degrees: None,
        text: input.source_utf8.clone(),
        color: Some(Color { r: 0, g: 0, b: 0 }),
        highlight_color: None,
        underline: false,
        font_size: Some(input.requested_font_size_bits.map_or(17.0, f32::from_bits)),
        runs: Vec::new(),
        spans: Vec::new(),
        paragraphs: Vec::new(),
        object_spans: Vec::new(),
        text_sections: Vec::new(),
        margins: input
            .requested_margin_bits
            .map(|bits| bits.map(f32::from_bits)),
        gravity: Some(0),
    }
}

fn measure(input: &MeasuredInput, fonts: &FontBook) -> NativePageLayout {
    let content = content(input);
    let settings = TextSettings::resolved();
    let styled = StyledText::new(&content, TextContext::Flow, settings);
    let renderer = TextRenderer::new(settings, fonts);
    let width = f32::from_bits(input.requested_width_bits) as i32;
    let height = f32::from_bits(input.requested_height_bits);
    let layout = layout_table_cell_text(
        &styled,
        TextFrame {
            bbox: BoundingBox {
                x_min: 0.0,
                y_min: 0.0,
                x_max: f64::from(width),
                y_max: f64::from(height),
            },
            gravity: Some(0),
            exclusions: &[],
        },
        NativeCellTextConstraints {
            width,
            height_limit: height,
        },
        RenderTheme::for_canvas(false),
        &renderer,
    )
    .unwrap();
    NativePageLayout::from_measured(&styled, &layout, width).unwrap()
}

#[test]
fn genuine_source_produces_native_measured_page_inputs() {
    let capture = measured_capture();
    let fonts = FontBook::default();
    assert_eq!(capture.cases.len(), 12);
    let mut line_count = 0;
    for case in capture.cases {
        let actual = measure(&case.input, &fonts);
        let expected = case.produced;
        assert_eq!(
            actual.text_length_utf16, expected.text_length_utf16,
            "{}: source length",
            case.name
        );
        assert_eq!(
            f32::from_bits(case.input.requested_width_bits) as i32,
            expected.layout_width,
            "{}: width",
            case.name
        );
        assert_eq!(
            actual.lines.len(),
            expected.lines.len(),
            "{}: lines",
            case.name
        );
        assert_eq!(
            actual.first_empty.map(f32::to_bits),
            expected.first_empty_rect_bits,
            "{}: first-empty",
            case.name
        );
        assert_eq!(
            actual.default_cursor.map(f32::to_bits),
            expected.default_cursor_rect_bits,
            "{}: cursor",
            case.name
        );
        for (index, (line, native)) in actual.lines.iter().zip(expected.lines).enumerate() {
            assert_eq!(
                line.source_inclusive_utf16, native.source_inclusive_utf16,
                "{}: line {index} owner",
                case.name
            );
            assert_eq!(
                line.top.to_bits(),
                native.line_top_bits,
                "{}: line {index} top",
                case.name
            );
            assert_eq!(
                line.background.map(f32::to_bits),
                native.background_rect_bits,
                "{}: line {index} background",
                case.name
            );
            line_count += 1;
        }
    }
    assert_eq!(line_count, 32);
}

fn pages(records: &[[i32; 5]]) -> Vec<NativePageRecord> {
    records
        .iter()
        .map(
            |&[cumulative_y, left, top, right, bottom]| NativePageRecord {
                cumulative_y,
                local_bounds: [left, top, right, bottom],
            },
        )
        .collect()
}

fn section_values(sections: &[NativePageSection]) -> Vec<[i32; 2]> {
    sections
        .iter()
        .map(|section| [section.start, section.count])
        .collect()
}

#[test]
fn genuine_measured_lines_produce_native_page_sections() {
    let fonts = FontBook::default();
    let mut boundaries = 0;
    let mut stages = 0;
    for case in measured_capture().cases {
        let layout = measure(&case.input, &fonts);
        let pages = pages(&case.input.page_records);
        for boundary in case.boundaries {
            let page = pages.get(boundary.page).copied();
            assert_eq!(
                layout.is_down_line(page, boundary.line),
                boundary.down,
                "{}: down page {} line {}",
                case.name,
                boundary.page,
                boundary.line
            );
            assert_eq!(
                layout.is_up_line(page, boundary.line),
                boundary.up,
                "{}: up page {} line {}",
                case.name,
                boundary.page,
                boundary.line
            );
            boundaries += 1;
        }
        let mut sections = NativePageSections::default();
        for stage in case.stages {
            layout
                .update_sections(Some(&pages), stage.first_page, &mut sections)
                .unwrap();
            assert_eq!(
                section_values(&sections.lines),
                stage.line_sections,
                "{}: line sections rescan {}",
                case.name,
                stage.first_page
            );
            assert_eq!(
                section_values(&sections.text),
                stage.text_sections,
                "{}: text sections rescan {}",
                case.name,
                stage.first_page
            );
            stages += 1;
        }
    }
    assert_eq!((boundaries, stages), (231, 36));
}

#[derive(Deserialize)]
struct KernelCapture {
    cases: Vec<KernelCase>,
}

#[derive(Deserialize)]
struct KernelCase {
    name: String,
    pages: Vec<[i32; 5]>,
    lines: Vec<KernelLine>,
    text_length_utf16: i32,
    first_empty_rect: [f32; 4],
    default_cursor_rect: [f32; 4],
    invalid_background_rect: [f32; 4],
    first_page: usize,
    initial_line_sections: Vec<[i32; 2]>,
    initial_text_sections: Vec<[i32; 2]>,
    null_document: bool,
    missing_page: Option<usize>,
    boundaries: Vec<Boundary>,
    indexer_executed: bool,
    line_sections: Vec<[i32; 2]>,
    text_sections: Vec<[i32; 2]>,
}

#[derive(Deserialize)]
struct KernelLine {
    background: [f32; 4],
    top: f32,
    source_inclusive: [i32; 2],
}

fn section_inputs(sections: &[[i32; 2]]) -> Vec<NativePageSection> {
    sections
        .iter()
        .map(|&[start, count]| NativePageSection { start, count })
        .collect()
}

#[test]
fn bounded_page_kernel_matches_actual_native_predicates_and_mutation() {
    let bytes = include_bytes!("../../../../../../conformance/table-page-text-ranges.json");
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "768293b00e5f880166add1683c12cf76fe9c5ebf293e1b522803ca69b395b05c"
    );
    let capture: KernelCapture = serde_json::from_slice(bytes).unwrap();
    assert_eq!(capture.cases.len(), 47);
    let mut boundaries = 0;
    let mut updates = 0;
    for case in capture.cases {
        let layout = NativePageLayout {
            text_length_utf16: case.text_length_utf16,
            lines: case
                .lines
                .into_iter()
                .map(|line| NativePageLine {
                    background: line.background,
                    top: line.top,
                    source_inclusive_utf16: line.source_inclusive,
                })
                .collect(),
            first_empty: case.first_empty_rect,
            default_cursor: case.default_cursor_rect,
            invalid_background: case.invalid_background_rect,
        };
        let pages = pages(&case.pages);
        for boundary in case.boundaries {
            let page = if case.null_document || case.missing_page == Some(boundary.page) {
                None
            } else {
                pages.get(boundary.page).copied()
            };
            assert_eq!(
                layout.is_down_line(page, boundary.line),
                boundary.down,
                "{}: down page {} line {}",
                case.name,
                boundary.page,
                boundary.line
            );
            assert_eq!(
                layout.is_up_line(page, boundary.line),
                boundary.up,
                "{}: up page {} line {}",
                case.name,
                boundary.page,
                boundary.line
            );
            boundaries += 1;
        }
        if case.indexer_executed {
            let mut sections = NativePageSections {
                lines: section_inputs(&case.initial_line_sections),
                text: section_inputs(&case.initial_text_sections),
            };
            layout
                .update_sections(
                    (!case.null_document).then_some(pages.as_slice()),
                    case.first_page,
                    &mut sections,
                )
                .unwrap();
            assert_eq!(
                section_values(&sections.lines),
                case.line_sections,
                "{}: line sections",
                case.name
            );
            assert_eq!(
                section_values(&sections.text),
                case.text_sections,
                "{}: text sections",
                case.name
            );
            updates += 1;
        }
    }
    assert_eq!((boundaries, updates), (251, 46));
}

fn bounded_layout() -> NativePageLayout {
    NativePageLayout {
        text_length_utf16: 3,
        lines: vec![
            NativePageLine {
                background: [0.0, 1.0, 10.0, 10.0],
                top: 1.0,
                source_inclusive_utf16: [0, 0],
            },
            NativePageLine {
                background: [0.0, 10.0, 10.0, 19.0],
                top: 10.0,
                source_inclusive_utf16: [1, 2],
            },
        ],
        first_empty: [0.0; 4],
        default_cursor: [0.0; 4],
        invalid_background: [0.0; 4],
    }
}

fn retained_sections() -> NativePageSections {
    NativePageSections {
        lines: section_inputs(&[[0, 2], [1, 1], [2, 0]]),
        text: section_inputs(&[[0, 3], [1, 2], [3, 0]]),
    }
}

#[test]
fn unsupported_page_and_owner_inputs_leave_retained_sections_unchanged() {
    let page = NativePageRecord {
        cumulative_y: 0,
        local_bounds: [0, 0, 90, 20],
    };
    for invalid in 0..8 {
        let mut layout = bounded_layout();
        let mut page = page;
        let mut first_page = 0;
        let mut sections = retained_sections();
        match invalid {
            0 => page.cumulative_y = -1,
            1 => first_page = 2,
            2 => layout.lines[1].source_inclusive_utf16 = [1, 3],
            3 => {
                layout.lines[0].source_inclusive_utf16 = [2, 2];
                layout.lines[1].source_inclusive_utf16 = [0, 0];
            }
            4 => layout.lines[0].background[1] = f32::NAN,
            5 => {
                first_page = 1;
                sections.lines[0] = NativePageSection { start: 5, count: 5 };
            }
            6 => {
                first_page = 1;
                sections.lines[0] = NativePageSection {
                    start: i32::MAX,
                    count: i32::MAX,
                };
            }
            7 => layout.invalid_background[3] = 1.0,
            _ => unreachable!(),
        }
        let before = sections.clone();
        assert!(
            layout
                .update_sections(Some(&[page]), first_page, &mut sections)
                .is_err(),
            "invalid {invalid}"
        );
        assert_eq!(sections, before, "invalid {invalid}");
    }
}

#[test]
fn producer_certificate_rejects_unsupported_source_without_mutating_it() {
    let base = measured_capture().cases.remove(0).input;
    let fonts = FontBook::default();
    let baseline = content(&base);
    let settings = TextSettings::resolved();
    let styled = StyledText::new(&baseline, TextContext::Flow, settings);
    let renderer = TextRenderer::new(settings, &fonts);
    let layout = layout_table_cell_text(
        &styled,
        TextFrame {
            bbox: BoundingBox {
                x_min: 0.0,
                y_min: 0.0,
                x_max: 90.0,
                y_max: 1000.0,
            },
            gravity: Some(0),
            exclusions: &[],
        },
        NativeCellTextConstraints {
            width: 90,
            height_limit: 1000.0,
        },
        RenderTheme::for_canvas(false),
        &renderer,
    )
    .unwrap();
    assert!(NativePageLayout::from_measured(&styled, &layout, 90).is_ok());
    for invalid in 0..6 {
        let mut content = content(&base);
        match invalid {
            0 => content.text = "AV\tbc".into(),
            1 => content.text = "AV😀".into(),
            2 => content.text = "AV\0".into(),
            3 => content.font_size = Some(24.0),
            4 => content.margins = Some([0.0, f32::INFINITY, 0.0, 0.0]),
            5 => content.rotation_degrees = Some(90.0),
            _ => unreachable!(),
        }
        let before = serde_json::to_value(&content).unwrap();
        let settings = TextSettings::resolved();
        let styled = StyledText::new(&content, TextContext::Flow, settings);
        assert!(matches!(
            NativePageLayout::from_measured(&styled, &layout, 90),
            Err(NativePageIndexUnavailable::OutsideCertificate)
        ));
        assert_eq!(serde_json::to_value(&content).unwrap(), before);
    }
    let placed = StyledText::new(&baseline, TextContext::Placed, settings);
    assert!(matches!(
        NativePageLayout::from_measured(&placed, &layout, 90),
        Err(NativePageIndexUnavailable::OutsideCertificate)
    ));
    assert!(matches!(
        NativePageLayout::from_measured(&styled, &layout, 20),
        Err(NativePageIndexUnavailable::OutsideCertificate)
    ));
}
