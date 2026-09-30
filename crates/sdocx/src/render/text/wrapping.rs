use std::ops::Range;

use crate::PredefinedTextStyle;
use crate::render::RenderTheme;

use super::breaks::{BreakKind, break_candidates};
use super::measurement::{MeasuredCluster, MeasurementError, ParagraphMeasurer};
use super::{StyledText, TextRenderer};

pub(in crate::render) struct WrappedLine {
    pub source: Range<usize>,
    pub font_size: f64,
    pub advance: f64,
    pub placements: Vec<PositionedCluster>,
}

pub(in crate::render) struct PositionedCluster {
    pub cluster: MeasuredCluster,
    pub x: f64,
}

impl WrappedLine {
    pub fn unmeasured(source: Range<usize>, font_size: f64) -> Self {
        Self {
            source,
            font_size,
            advance: 0.0,
            placements: Vec::new(),
        }
    }
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
    let mut breaks = break_candidates(text);
    let measurer = ParagraphMeasurer::new(styled, range.clone(), theme, predefined, renderer)?;
    let measured = measurer.measure_line(range.clone())?;
    let mut advances = vec![0.0; range.len() + 1];
    let mut cluster_ends = vec![false; range.len() + 1];
    cluster_ends[0] = true;
    let mut source_end = range.start;
    let mut advance = 0.0;
    for cluster in &measured.clusters {
        if cluster.source.start != source_end
            || cluster.source.end <= source_end
            || cluster.source.end > range.end
        {
            return Err(MeasurementError::InvalidCluster);
        }
        advance += cluster.advance;
        let end = cluster.source.end - range.start;
        advances[end] = advance;
        cluster_ends[end] = true;
        source_end = cluster.source.end;
    }
    if source_end != range.end {
        return Err(MeasurementError::InvalidCluster);
    }
    advances[range.len()] = measured.advance;
    if breaks
        .candidates
        .iter()
        .any(|candidate| candidate.kind == BreakKind::Mandatory && !cluster_ends[candidate.end])
    {
        return Err(MeasurementError::InvalidCluster);
    }
    breaks
        .candidates
        .retain(|candidate| cluster_ends[candidate.end]);
    breaks.emergency.retain(|&end| cluster_ends[end]);
    let width = if max_width.is_nan() {
        0.0
    } else {
        max_width.max(0.0)
    };
    let mut lines = Vec::new();
    let mut start = 0;
    let mut candidate_index = 0;
    let mut cluster_index = 0;
    while start < range.len() {
        let mut selected = None;
        let mut overflow_end = None;
        while breaks
            .candidates
            .get(candidate_index)
            .is_some_and(|candidate| candidate.end <= start)
        {
            candidate_index += 1;
        }
        let mut probe = candidate_index;
        for candidate in &breaks.candidates[candidate_index..] {
            if advances[candidate.end] - advances[start] > width {
                overflow_end = Some(candidate.end);
                break;
            }
            selected = Some(candidate.end);
            probe += 1;
            if candidate.kind == BreakKind::Mandatory {
                break;
            }
        }
        candidate_index = probe;
        if selected.is_none() {
            let overflow_end = overflow_end.ok_or(MeasurementError::InvalidRange)?;
            let first = breaks.emergency.partition_point(|&end| end <= start);
            let last = breaks.emergency.partition_point(|&end| end <= overflow_end);
            let ends = &breaks.emergency[first..last];
            for &end in ends {
                if advances[end] - advances[start] > width {
                    break;
                }
                selected = Some(end);
            }
            selected = selected.or_else(|| ends.first().copied());
        }
        let end = selected.ok_or(MeasurementError::InvalidRange)?;
        let source = range.start + start..range.start + end;
        let font_size = if source == range {
            measured.font_size
        } else {
            measurer.font_size(source.clone())?
        };
        let mut placements = Vec::new();
        let mut x = 0.0;
        while let Some(cluster) = measured.clusters.get(cluster_index)
            && cluster.source.end <= source.end
        {
            placements.push(PositionedCluster {
                cluster: cluster.clone(),
                x,
            });
            x += cluster.advance;
            cluster_index += 1;
        }
        lines.push(WrappedLine {
            source,
            font_size,
            advance: advances[end] - advances[start],
            placements,
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
    fn wrapped_placements_keep_shared_paragraph_glyphs_and_kerning_positions() {
        let lines = wrap(&text("AVA"), 55.0);
        assert_eq!(ranges(&lines), [0..2, 2..3]);
        assert_eq!(lines[0].advance, 54.42626953125);
        assert_eq!(lines[1].advance, 29.35546875);
        assert_eq!(lines[0].placements.len(), 2);
        assert_eq!(lines[1].placements.len(), 1);
        assert_eq!(lines[0].placements[0].x, 0.0);
        assert_eq!(lines[0].placements[1].x, 27.44384765625);
        assert_eq!(lines[1].placements[0].x, 0.0);
        let run = &lines[0].placements[0].cluster.run;
        assert!(Arc::ptr_eq(run, &lines[0].placements[1].cluster.run));
        assert!(Arc::ptr_eq(run, &lines[1].placements[0].cluster.run));
        assert_eq!(run.glyphs[1].raw.x_advance, 1228);
        for (placement, character) in lines
            .iter()
            .flat_map(|line| &line.placements)
            .zip("AVA".chars())
        {
            let offset = placement
                .cluster
                .paint_offset(&character.to_string())
                .unwrap()
                .unwrap();
            assert_eq!(offset.x, 0.0);
            assert_eq!(offset.y, 0.0);
        }
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
    fn paragraph_kerning_is_retained_across_emergency_line_breaks() {
        let face = rustybuzz::Face::from_slice(
            include_bytes!("../../../assets/fonts/Roboto-Regular.ttf"),
            0,
        )
        .unwrap();
        let upstream_advances = |value: &str| {
            let mut buffer = rustybuzz::UnicodeBuffer::new();
            buffer.push_str(value);
            buffer.guess_segment_properties();
            rustybuzz::shape(&face, &[], buffer)
                .glyph_positions()
                .iter()
                .map(|position| position.x_advance)
                .collect::<Vec<_>>()
        };
        let paragraph_advances = upstream_advances("AVA");
        let isolated_advances = upstream_advances("AV");
        assert_eq!(paragraph_advances, [1249, 1228, 1336]);
        assert_eq!(isolated_advances, [1249, 1303]);
        assert_eq!(
            f64::from(paragraph_advances[..2].iter().sum::<i32>()) / 2048.0 * 45.0,
            54.42626953125
        );
        assert_eq!(
            f64::from(isolated_advances.iter().sum::<i32>()) / 2048.0 * 45.0,
            56.07421875
        );
        assert_eq!(ranges(&wrap(&text("AVA"), 55.0)), [0..2, 2..3]);
        assert_eq!(ranges(&wrap(&text("AVAV"), 56.0)), [0..2, 2..3, 3..4]);
    }

    #[test]
    fn line_maximum_font_size_excludes_neighboring_style_ranges() {
        let mut content = text("AAA");
        content.spans = vec![span(
            RichTextSpanType::FontSize,
            1,
            2,
            &90.0_f32.to_le_bytes(),
        )];
        let lines = wrap(&content, 60.0);
        assert_eq!(ranges(&lines), [0..1, 1..2, 2..3]);
        assert_eq!(
            lines.iter().map(|line| line.font_size).collect::<Vec<_>>(),
            [45.0, 90.0, 45.0]
        );
    }

    #[test]
    fn arabic_combining_graphemes_preserve_contiguous_logical_source_ranges() {
        let content = text("Aب\u{64e}ت\u{64e}Z");
        let lines = wrap(&content, 0.0);
        assert_eq!(ranges(&lines), [0..1, 1..3, 3..5, 5..6]);
    }

    #[test]
    fn long_unbroken_token_retains_every_scalar_in_expected_line_ranges() {
        let content = text(&"A".repeat(4096));
        let lines = wrap(&content, 60.0);
        assert_eq!(lines.len(), 2048);
        for (index, line) in lines.iter().enumerate() {
            assert_eq!(line.source, index * 2..index * 2 + 2);
            assert_eq!(line.font_size, 45.0);
        }
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
