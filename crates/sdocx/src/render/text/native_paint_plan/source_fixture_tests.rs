use sha2::{Digest, Sha256};

use super::test_support::*;
use super::*;
use crate::fonts::FontBook;

fn capture() -> Capture {
    let bytes = include_bytes!("../../../../../../conformance/table-text-cell-source-inputs.json");
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "8529637cdcc67b8c5e4747f0dd6105d060ee58390809b9f252cccab35de7b9de"
    );
    let capture: Capture = serde_json::from_slice(bytes).unwrap();
    assert_eq!(capture.cases.len(), 13);
    capture
}

fn check_model_source(case: &Case) {
    let observed = case.source_input.as_ref().unwrap();
    assert!(observed.common_present);
    assert_eq!(observed.text_utf8, case.text_utf8);
    let size = case.requested_font_size_bits.unwrap_or(50_f32.to_bits());
    let source_length = u32::try_from(case.source_utf16.len()).unwrap();
    let range = [0, source_length];
    assert_eq!(
        observed.font_size_at_utf16_including_end_bits,
        vec![size; case.source_utf16.len() + 1]
    );
    assert_eq!(
        observed.foreground_at_utf16_including_end_argb,
        vec![0xff25_2525; case.source_utf16.len() + 1]
    );
    assert_eq!(observed.gravity, 1);
    assert_eq!(
        observed.margin_bits,
        case.supplied_margins_bits.unwrap_or([0; 4])
    );
    assert_eq!(observed.font_size_spans.len(), 1);
    let font = &observed.font_size_spans[0];
    assert_eq!(font.kind, 3);
    assert_eq!(font.utf16_range, range);
    assert_eq!(font.interval, crate::SpanIntervalType::ClosedClosed.raw());
    assert_eq!(font.font_size_bits, size);
    assert_eq!(observed.foreground_spans.len(), 1);
    let foreground = &observed.foreground_spans[0];
    assert_eq!(foreground.kind, 1);
    assert_eq!(foreground.utf16_range, range);
    assert_eq!(
        foreground.interval,
        crate::SpanIntervalType::ClosedClosed.raw()
    );
    assert_eq!(foreground.color_argb, 0xff25_2525);
    assert_eq!(foreground.color_type, 0);

    let alignments: Vec<_> = observed
        .paragraphs
        .iter()
        .filter(|paragraph| paragraph.kind == 3)
        .collect();
    assert_eq!(alignments.len(), 1);
    assert_eq!(alignments[0].paragraph_index_range, [0, 1]);
    assert_eq!(alignments[0].alignment, Some(2));
    let parsing: Vec<_> = observed
        .paragraphs
        .iter()
        .filter(|paragraph| paragraph.kind == 6)
        .collect();
    assert_eq!(observed.paragraphs.len(), alignments.len() + parsing.len());
    assert_eq!(parsing.len(), case.text_utf8.split('\n').count());
    for (index, paragraph) in parsing.into_iter().enumerate() {
        let start = u32::try_from(index).unwrap();
        assert_eq!(paragraph.paragraph_index_range, [start, start + 1]);
        assert_eq!(paragraph.alignment, None);
    }
}

#[test]
fn rust_cell_producer_matches_native_stacked_marks_wrapping_and_source_observations() {
    let capture = capture();
    let fonts = FontBook::default();
    let font_hash = &capture
        .dependencies
        .font_files
        .iter()
        .find(|font| font.file == "Roboto-Regular.ttf")
        .unwrap()
        .sha256;
    let mut counts = [0; 4];
    for case in &capture.cases {
        if [
            "supplementary-mark-wrap",
            "mixed-hebrew-wrap",
            "mixed-arabic-wrap",
            "newline-mixed-maps",
            "rtl-tab-newline",
        ]
        .contains(&case.name.as_str())
        {
            continue;
        }
        assert!(case.emitted_runs.accepted);
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
        counts[3] += case.source_utf16.len();
    }
    assert_eq!(counts, [8, 14, 33, 33]);
}

#[test]
fn rust_cell_producer_rejects_uncertified_rtl_and_missing_glyph_source_profiles() {
    let capture = capture();
    let fonts = FontBook::default();
    for name in [
        "supplementary-mark-wrap",
        "mixed-hebrew-wrap",
        "mixed-arabic-wrap",
        "newline-mixed-maps",
        "rtl-tab-newline",
    ] {
        let case = capture.cases.iter().find(|case| case.name == name).unwrap();
        let error = prepare(case, &fonts).unwrap_err();
        match error {
            PreparationError::Paint(NativePaintPlanUnavailable::OutsideCertificate("layout")) => {}
            PreparationError::Layout(kind) => panic!("{name}: unexpected layout failure {kind:?}"),
            PreparationError::Paint(error) => panic!("{name}: unexpected paint failure {error:?}"),
        }
    }
}

fn prepare(case: &Case, fonts: &FontBook) -> Result<NativePaintPlan, PreparationError> {
    check_model_source(case);
    super::test_support::prepare(case, fonts)
}
