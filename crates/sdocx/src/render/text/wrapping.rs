use std::ops::Range;

use crate::PredefinedTextStyle;
use crate::render::RenderTheme;

use super::breaks::{BreakKind, break_candidates};
use super::measurement::{MeasurementError, ParagraphMeasurer};
use super::{StyledText, TextRenderer};

pub(in crate::render) struct WrappedLine {
    pub source: Range<usize>,
    pub font_size: f64,
}

pub(in crate::render) fn wrap_paragraph(
    styled: &StyledText<'_>,
    range: Range<usize>,
    max_width: f64,
    theme: RenderTheme,
    predefined: Option<PredefinedTextStyle>,
    renderer: &TextRenderer<'_>,
) -> Result<Vec<WrappedLine>, MeasurementError> {
    let text = styled
        .index
        .slice(range.clone())
        .ok_or(MeasurementError::InvalidRange)?;
    let breaks = break_candidates(text);
    let measurer = ParagraphMeasurer::new(styled, range.clone(), theme, predefined, renderer)?;
    let width = if max_width.is_nan() {
        0.0
    } else {
        max_width.max(0.0)
    };
    let mut lines = Vec::new();
    let mut start = 0;
    while start < range.len() {
        let mut selected = None;
        let mut overflow = None;
        for candidate in breaks
            .candidates
            .iter()
            .filter(|candidate| candidate.end > start)
        {
            let measured =
                measurer.measure_line(range.start + start..range.start + candidate.end)?;
            if measured.advance > width {
                overflow = Some((candidate.end, measured));
                break;
            }
            selected = Some((candidate.end, measured.font_size));
            if candidate.kind == BreakKind::Mandatory {
                break;
            }
        }
        if selected.is_none() {
            let (overflow_end, overflowing) = overflow.ok_or(MeasurementError::InvalidRange)?;
            let ends = breaks
                .emergency
                .iter()
                .copied()
                .filter(|&end| end > start && end <= overflow_end)
                .collect::<Vec<_>>();
            let mut hint = start;
            let mut advance = 0.0;
            for cluster in &overflowing.clusters {
                advance += cluster.advance;
                if advance > width {
                    break;
                }
                hint = cluster.source.end - range.start;
            }
            let mut probe = ends.partition_point(|&end| end <= hint).saturating_sub(1);
            let end = *ends.get(probe).ok_or(MeasurementError::InvalidRange)?;
            let mut measured = measurer.measure_line(range.start + start..range.start + end)?;
            while measured.advance > width && probe > 0 {
                probe -= 1;
                measured = measurer.measure_line(range.start + start..range.start + ends[probe])?;
            }
            selected = Some((ends[probe], measured.font_size));
            if measured.advance <= width {
                for &end in &ends[probe + 1..] {
                    let measured = measurer.measure_line(range.start + start..range.start + end)?;
                    if measured.advance > width {
                        break;
                    }
                    selected = Some((end, measured.font_size));
                }
            }
        }
        let (end, font_size) = selected.ok_or(MeasurementError::InvalidRange)?;
        lines.push(WrappedLine {
            source: range.start + start..range.start + end,
            font_size,
        });
        start = end;
    }
    Ok(lines)
}
#[cfg(test)]
mod tests {
    use super::super::{TextContext, TextSettings};
    use super::*;
    use crate::fonts::{FontBook, fontdb};
    use crate::{BoundingBox, RichTextBox, RichTextSpan, RichTextSpanType};
    use std::sync::Arc;

    fn text(value: &str) -> RichTextBox {
        RichTextBox {
            text_area_type: None,
            bbox: BoundingBox::default(),
            rotation_degrees: None,
            text: value.into(),
            color: None,
            highlight_color: None,
            underline: false,
            font_size: Some(45.0),
            runs: Vec::new(),
            spans: Vec::new(),
            paragraphs: Vec::new(),
            object_spans: Vec::new(),
            text_sections: Vec::new(),
            margins: None,
            gravity: None,
        }
    }

    fn span(kind: RichTextSpanType, start: u32, end: u32, payload: &[u8]) -> RichTextSpan {
        RichTextSpan {
            kind,
            start_utf16: start,
            end_utf16: end,
            expand: false,
            payload: payload.into(),
        }
    }

    fn wrap_with_fonts(
        content: &RichTextBox,
        range: Range<usize>,
        width: f64,
        fonts: &FontBook,
    ) -> Result<Vec<WrappedLine>, MeasurementError> {
        let settings = TextSettings {
            scale: 1.0,
            font_size_delta: 0.0,
        };
        let styled = StyledText::new(content, TextContext::Placed, settings);
        let renderer = TextRenderer::new(settings, fonts);
        wrap_paragraph(
            &styled,
            range,
            width,
            RenderTheme::for_canvas(false),
            None,
            &renderer,
        )
    }

    fn wrap(content: &RichTextBox, width: f64) -> Vec<WrappedLine> {
        wrap_with_fonts(
            content,
            0..content.text.chars().count(),
            width,
            &FontBook::default(),
        )
        .unwrap()
    }

    fn ranges(lines: &[WrappedLine]) -> Vec<Range<usize>> {
        lines.iter().map(|line| line.source.clone()).collect()
    }

    fn assert_single_line(lines: &[WrappedLine], expected: Range<usize>) {
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].source, expected);
    }

    #[test]
    fn pinned_abc_fits_exact_advance_but_wraps_below_it() {
        let content = text("ABC");
        let exact = wrap(&content, 86.66015625);
        assert_single_line(&exact, 0..3);
        assert_eq!(exact[0].font_size, 45.0);
        assert_eq!(
            ranges(&wrap(&content, 86.66015625 - 0.00000001)),
            [0..2, 2..3]
        );
    }

    #[test]
    fn local_font_size_and_selected_styles_change_breaks() {
        let content = text("ABC");
        assert_single_line(&wrap(&content, 87.0), 0..3);
        let mut larger = content.clone();
        larger.spans = vec![span(
            RichTextSpanType::FontSize,
            1,
            2,
            &90.0_f32.to_le_bytes(),
        )];
        let lines = wrap(&larger, 87.0);
        assert_eq!(ranges(&lines), [0..2, 2..3]);
        assert_eq!(
            lines.iter().map(|line| line.font_size).collect::<Vec<_>>(),
            [90.0, 45.0]
        );
        let mut bold = content.clone();
        bold.spans = vec![span(RichTextSpanType::Bold, 0, 3, &[1, 0])];
        assert_eq!(ranges(&wrap(&bold, 87.0)), [0..2, 2..3]);
        let mut italic = content.clone();
        italic.spans = vec![span(RichTextSpanType::Italic, 0, 3, &[1, 0])];
        assert_eq!(ranges(&wrap(&content, 85.0)), [0..2, 2..3]);
        assert_single_line(&wrap(&italic, 85.0), 0..3);
    }

    #[test]
    fn wrapped_ranges_preserve_all_leading_repeated_and_trailing_spaces() {
        let content = text(" A  B   C  ");
        let lines = wrap(&content, 50.0);
        assert!(lines.len() > 1);
        assert_eq!(lines.first().unwrap().source.start, 0);
        assert_eq!(lines.last().unwrap().source.end, 11);
        for pair in lines.windows(2) {
            assert_eq!(pair[0].source.end, pair[1].source.start);
        }
        let retained = lines
            .iter()
            .map(|line| {
                content
                    .text
                    .chars()
                    .skip(line.source.start)
                    .take(line.source.len())
                    .collect::<String>()
            })
            .collect::<String>();
        assert_eq!(retained, " A  B   C  ");
    }

    #[test]
    fn emergency_breaks_keep_combining_and_emoji_clusters_with_scalar_offsets() {
        let content = text("Ae\u{301}👩‍👩‍👧‍👦Z");
        let lines = wrap_with_fonts(&content, 1..10, 0.0, &FontBook::default()).unwrap();
        assert_eq!(ranges(&lines), [1..3, 3..10]);
    }

    #[test]
    fn nonbreaking_space_is_only_broken_when_the_whole_group_overflows() {
        let content = text("A\u{a0}B");
        assert_single_line(&wrap(&content, 80.0), 0..3);
        assert_eq!(ranges(&wrap(&content, 60.0)), [0..2, 2..3]);
    }

    #[test]
    fn crlf_mandatory_break_is_retained_as_one_source_boundary() {
        let content = text("A\r\nB");
        assert_eq!(ranges(&wrap(&content, f64::INFINITY)), [0..3, 3..4]);
    }

    #[test]
    fn empty_text_and_oversized_graphemes_terminate_without_dropping_source() {
        assert!(wrap(&text(""), 0.0).is_empty());
        assert_single_line(&wrap(&text("e\u{301}"), 0.0), 0..2);
        assert_single_line(&wrap(&text("👩‍👩‍👧‍👦"), 0.0), 0..7);
        assert_eq!(ranges(&wrap(&text("ABC"), f64::NAN)), [0..1, 1..2, 2..3]);
        assert_eq!(ranges(&wrap(&text("ABC"), -1.0)), [0..1, 1..2, 2..3]);
    }

    #[test]
    fn caller_monospace_font_changes_breaks_from_default_roboto() {
        let mut database = fontdb::Database::new();
        database.load_font_data(
            include_bytes!("../../../assets/fonts/RobotoMono-Regular.ttf").to_vec(),
        );
        database.set_sans_serif_family("Roboto Mono");
        let fonts = FontBook::new(Arc::new(database));
        let content = text("ABC");
        assert_eq!(ranges(&wrap(&content, 83.0)), [0..2, 2..3]);
        let caller = wrap_with_fonts(&content, 0..3, 83.0, &fonts).unwrap();
        assert_single_line(&caller, 0..3);
    }

    #[test]
    fn unavailable_fonts_return_typed_error_instead_of_estimated_widths() {
        let fonts = FontBook::new(Arc::new(fontdb::Database::new()));
        let content = text("ABC");
        let result = wrap_with_fonts(&content, 0..3, 1000.0, &fonts);
        assert!(
            matches!(result, Err(MeasurementError::UnavailableFace(family)) if family == "Roboto")
        );
    }
}
