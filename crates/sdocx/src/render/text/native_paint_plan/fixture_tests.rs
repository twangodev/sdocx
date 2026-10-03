use sha2::{Digest, Sha256};

use super::test_support::*;
use super::*;
use crate::fonts::FontBook;
use crate::{RichTextParagraph, RichTextParagraphType};

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
