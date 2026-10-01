use std::ops::Range;

use crate::fonts::fontdb;
use unicode_bidi::{BidiClass, bidi_class};

use crate::render::vector::{
    FontFamily, Paint, Rectangle, Scene, Styled, Text, TextAnchor, decimal,
};
use crate::render::{RenderTheme, push_text_span, render_flow_line, styled_tspan};
use crate::{ParagraphAlignment, PredefinedTextStyle};

use super::{StyledText, TextRenderer, TextStyle, WrappedLine};

struct PositionedSpan {
    source: Range<usize>,
    positions: Vec<f64>,
    offset_y: f64,
    style: TextStyle,
    font_face: Option<(fontdb::Weight, fontdb::Style)>,
    positioned: bool,
}

#[allow(clippy::too_many_arguments)]
pub(in crate::render) fn render_measured_line(
    svg: &mut Scene,
    styled: &StyledText<'_>,
    line: &WrappedLine,
    left: f64,
    available_width: f64,
    baseline: f64,
    alignment: Option<ParagraphAlignment>,
    theme: RenderTheme,
    predefined: Option<PredefinedTextStyle>,
    renderer: &TextRenderer<'_>,
) {
    if line.source.is_empty() {
        return;
    }
    #[cfg(feature = "pdf")]
    if svg.retains_text() {
        if let Err(message) = render_retained_line(
            svg,
            styled,
            line,
            [left, available_width, baseline],
            alignment,
            theme,
            predefined,
            renderer,
        ) {
            svg.reject_native_text(message);
        }
        return;
    }
    let scoped_renderer = renderer.for_source(line.source.clone());
    let renderer = &scoped_renderer;
    if line.placements.is_empty() && line.objects.is_empty() {
        render_flow_line(
            svg,
            styled,
            line.source.clone(),
            left,
            left + available_width,
            baseline,
            alignment,
            theme,
            predefined,
            renderer,
        );
        return;
    }
    let x = left + super::line_alignment_offset(line.advance, available_width, alignment);
    if line.placements.is_empty()
        || line
            .objects
            .iter()
            .any(|object| matches!(object.prepared, Some(Err(_))))
    {
        render_text_fragments(svg, styled, line, x, baseline, theme, predefined, renderer);
        return;
    }
    let spans = positioned_spans(styled, line, x, baseline, theme, predefined, renderer);
    let Some(spans) = spans else {
        render_text_fragments(svg, styled, line, x, baseline, theme, predefined, renderer);
        return;
    };
    svg.scope(
        Text::new("")
            .x(decimal(x, 2))
            .y(decimal(baseline, 2))
            .anchor(TextAnchor::Start)
            .family(FontFamily::Roboto)
            .preserve_space(),
        |svg| {
            for span in &spans {
                let mut style = span.style.clone();
                if span.positioned {
                    style.underline = false;
                    style.strikethrough = false;
                }
                let mut node = styled_tspan(
                    styled.index.slice(span.source.clone()).unwrap(),
                    &style,
                    styled.context(),
                )
                .y(decimal(baseline + span.offset_y, 5));
                if !span.positions.is_empty() {
                    node = node.x_positions(&span.positions, 5);
                }
                if let Some((weight, style)) = span.font_face {
                    node = node.font_face(weight, style);
                }
                push_text_span(svg, node, &style);
            }
        },
    );
    for segment in spans
        .iter()
        .filter(|span| span.positioned)
        .flat_map(|span| styled.foreground_segments(span.source.clone()))
    {
        let style = styled.style_at(segment.start, theme, predefined);
        if !style.underline && !style.strikethrough {
            continue;
        }
        let first = line
            .placements
            .partition_point(|placement| placement.cluster.source.end <= segment.start);
        let last = line
            .placements
            .partition_point(|placement| placement.cluster.source.start < segment.end);
        if first == last {
            continue;
        }
        let intervals = line.placements[first..last].iter().map(|placement| {
            let start = x + placement.x;
            (
                placement.visual_rank,
                start,
                start + placement.cluster.advance + placement.extra_advance,
            )
        });
        render_decoration_intervals(svg, &style, baseline, intervals);
    }
}

#[cfg(feature = "pdf")]
#[allow(clippy::too_many_arguments)]
fn render_retained_line(
    svg: &mut Scene,
    styled: &StyledText<'_>,
    line: &WrappedLine,
    frame: [f64; 3],
    alignment: Option<ParagraphAlignment>,
    theme: RenderTheme,
    predefined: Option<PredefinedTextStyle>,
    renderer: &TextRenderer<'_>,
) -> Result<(), String> {
    let advance = line
        .advance_for_paint(true)
        .ok_or_else(|| format!("unavailable canonical line geometry: {}", line.geometry))?;
    if line
        .objects
        .iter()
        .any(|object| matches!(object.prepared, Some(Err(_))))
    {
        return Err("rejected embedded object has no retained placeholder plan".into());
    }
    if line.placements.is_empty() {
        return if line.objects.is_empty() {
            Err("unmeasured text has no retained glyph plan".into())
        } else {
            Ok(())
        };
    }
    let [left, width, baseline] = frame;
    let x = left + super::line_alignment_offset(advance, width, alignment);
    for range in text_ranges(line) {
        render_retained_fragment(
            svg,
            styled,
            line,
            range,
            [x, baseline],
            theme,
            predefined,
            renderer,
        )?;
    }
    render_retained_decorations(svg, styled, line, x, baseline, theme, predefined);
    Ok(())
}

#[cfg(feature = "pdf")]
#[allow(clippy::too_many_arguments)]
fn render_retained_fragment(
    svg: &mut Scene,
    styled: &StyledText<'_>,
    line: &WrappedLine,
    range: Range<usize>,
    origin: [f64; 2],
    theme: RenderTheme,
    predefined: Option<PredefinedTextStyle>,
    renderer: &TextRenderer<'_>,
) -> Result<(), String> {
    use super::native::{NativeGlyph, NativeGlyphRun, NativeTextBlock, NativeTextPaint};
    use std::sync::Arc;
    let [x, baseline] = origin;
    let first_cluster = line
        .placements
        .partition_point(|placement| placement.cluster.source.end <= range.start);
    let last_cluster = line
        .placements
        .partition_point(|placement| placement.cluster.source.start < range.end);
    let byte_start = styled
        .index
        .char_to_byte(range.start)
        .ok_or("invalid retained line source")?;
    let source: Arc<str> = styled
        .index
        .slice(range)
        .ok_or("invalid retained line source")?
        .into();
    let mut runs = Vec::new();
    let mut index = first_cluster;
    while index < last_cluster {
        let first = &line.placements[index];
        let measured = &first.cluster.run;
        let style = styled.style_at(first.cluster.source.start, theme, predefined);
        renderer
            .for_source(first.cluster.source.clone())
            .report_resolution(&style, styled.context());
        let mut clusters = Vec::new();
        while index < last_cluster
            && let Some(placement) = line.placements.get(index)
            && Arc::ptr_eq(&placement.cluster.run, measured)
        {
            clusters.push((index, placement));
            index += 1;
        }
        clusters.sort_unstable_by_key(|(_, placement)| placement.cluster.glyphs.start);
        let mut glyphs = Vec::new();
        for (index, placement) in clusters {
            let position = line
                .text_position(index, true)
                .ok_or("missing canonical cluster position")?;
            for glyph in placement
                .cluster
                .retained_glyphs(&styled.index)
                .map_err(|error| error.to_string())?
            {
                if glyph.glyph_id == 0 {
                    renderer.for_source(glyph.source.clone()).missing_glyphs(
                        &glyph.face.family,
                        styled.index.slice(glyph.source.clone()).unwrap_or(""),
                    );
                }
                glyphs.push(NativeGlyph {
                    glyph_id: glyph.glyph_id,
                    origin: [
                        x + position.x + position.glyph_offset_x + glyph.offset_x,
                        baseline + glyph.offset_y,
                    ],
                    advance: [glyph.advance_x, glyph.advance_y],
                    source: glyph.source_bytes.start - byte_start
                        ..glyph.source_bytes.end - byte_start,
                });
            }
        }
        let Some(crate::render::vector::ColorValue::Rgb(color)) =
            crate::render::vector::ColorValue::from_hex(&style.color)
        else {
            return Err("invalid retained text color".into());
        };
        runs.push(NativeGlyphRun {
            face: measured.face.clone(),
            font_size: measured.style.font_size,
            paint: NativeTextPaint {
                color,
                stroke_width: (style.bold && matches!(styled.context(), super::TextContext::Flow))
                    .then_some(0.45),
            },
            glyphs,
            variable: measured.variable,
            synthetic_italic: style.italic && measured.face.style == fontdb::Style::Normal,
            synthetic_bold: style.bold
                && matches!(styled.context(), super::TextContext::Placed)
                && measured.face.weight < fontdb::Weight::BOLD,
        });
    }
    let inkless = runs.iter().all(|run| {
        run.glyphs.iter().all(|glyph| {
            run.face
                .glyph_ink_bounds(glyph.glyph_id)
                .is_ok_and(|bounds| bounds.is_none())
        })
    });
    let id = svg
        .native_text()
        .register(NativeTextBlock { source, runs })
        .map_err(|error| error.to_string())?;
    svg.scope(
        Text::new("")
            .native_id(id)
            .x(decimal(x, 5))
            .y(decimal(baseline, 5))
            .anchor(TextAnchor::Start)
            .family(FontFamily::Roboto)
            .preserve_space(),
        |svg| {
            for (index, placement) in line
                .placements
                .iter()
                .enumerate()
                .take(last_cluster)
                .skip(first_cluster)
            {
                let position = line.text_position(index, true).unwrap();
                let style = styled.style_at(placement.cluster.source.start, theme, predefined);
                let mut style =
                    renderer.output_style_with_face(&style, &placement.cluster.run.face);
                style.underline = false;
                style.strikethrough = false;
                let span = styled_tspan(
                    styled
                        .index
                        .slice(placement.cluster.source.clone())
                        .unwrap(),
                    &style,
                    styled.context(),
                )
                .x_positions(&[x + position.x], 5)
                .font_face(
                    placement.cluster.run.face.weight,
                    placement.cluster.run.face.style,
                );
                push_text_span(svg, span, &style);
            }
            if inkless {
                let face = &line.placements[first_cluster].cluster.run.face;
                svg.push(
                    crate::render::vector::TSpan::new(".")
                        .font_size(1)
                        .family(FontFamily::Named(&face.family))
                        .font_face(face.weight, face.style),
                );
            }
        },
    );
    Ok(())
}

#[cfg(feature = "pdf")]
fn render_retained_decorations(
    svg: &mut Scene,
    styled: &StyledText<'_>,
    line: &WrappedLine,
    x: f64,
    baseline: f64,
    theme: RenderTheme,
    predefined: Option<PredefinedTextStyle>,
) {
    let mut index = 0;
    while let Some(first) = line.placements.get(index) {
        let style = styled.style_at(first.cluster.source.start, theme, predefined);
        if !style.underline && !style.strikethrough {
            index += 1;
            continue;
        }
        let run = &first.cluster.run;
        let start_index = index;
        let mut intervals = Vec::new();
        while let Some(placement) = line.placements.get(index) {
            let next = styled.style_at(placement.cluster.source.start, theme, predefined);
            if !same_decoration_style(&style, &next)
                || placement.cluster.run.face.id != run.face.id
                || placement.cluster.run.direction != run.direction
                || (index > start_index
                    && line.placements[index - 1].cluster.source.end
                        != placement.cluster.source.start)
            {
                break;
            }
            let position = line.text_position(index, true).unwrap();
            let start = x + position.x;
            intervals.push((
                position.visual_rank,
                start,
                start + placement.cluster.advance + position.extra_advance,
            ));
            index += 1;
        }
        render_decoration_intervals(svg, &style, baseline, intervals);
    }
}

#[cfg(feature = "pdf")]
fn same_decoration_style(left: &TextStyle, right: &TextStyle) -> bool {
    left.font_size == right.font_size
        && left.color == right.color
        && left.bold == right.bold
        && left.italic == right.italic
        && left.underline == right.underline
        && left.strikethrough == right.strikethrough
}

fn render_decoration_intervals(
    svg: &mut Scene,
    style: &TextStyle,
    baseline: f64,
    intervals: impl IntoIterator<Item = (usize, f64, f64)>,
) {
    let mut intervals = intervals
        .into_iter()
        .filter(|&(_, start, end)| end > start)
        .collect::<Vec<_>>();
    intervals.sort_unstable_by_key(|&(rank, _, _)| rank);
    let mut joined: Vec<(usize, f64, f64)> = Vec::new();
    for (rank, start, end) in intervals {
        if let Some(previous) = joined.last_mut()
            && (rank == previous.0 + 1 || start <= previous.2)
        {
            previous.0 = rank;
            previous.1 = previous.1.min(start);
            previous.2 = previous.2.max(end);
        } else {
            joined.push((rank, start, end));
        }
    }
    let thickness = style.font_size * f64::from(1.0_f32 / 18.0);
    for offset in [
        style.underline.then_some(f64::from(1.0_f32 / 9.0)),
        style.strikethrough.then_some(f64::from(-2.0_f32 / 7.0)),
    ]
    .into_iter()
    .flatten()
    {
        for &(_, start, end) in &joined {
            svg.push(
                Rectangle::new()
                    .x(decimal(start, 5))
                    .y(decimal(baseline + style.font_size * offset, 5))
                    .width(decimal((end - start).max(0.0), 5))
                    .height(decimal(thickness, 5))
                    .fill(Paint::from_hex(&style.color)),
            );
        }
    }
}

pub(in crate::render) fn text_ranges(line: &WrappedLine) -> Vec<Range<usize>> {
    let mut ranges = Vec::with_capacity(line.objects.len() + 1);
    let mut start = line.source.start;
    for object in &line.objects {
        if matches!(object.prepared, Some(Err(_))) {
            continue;
        }
        if start < object.object.source.start {
            ranges.push(start..object.object.source.start);
        }
        start = object.object.source.end;
    }
    if start < line.source.end {
        ranges.push(start..line.source.end);
    }
    ranges
}

#[allow(clippy::too_many_arguments)]
fn render_text_fragments(
    svg: &mut Scene,
    styled: &StyledText<'_>,
    line: &WrappedLine,
    x: f64,
    baseline: f64,
    theme: RenderTheme,
    predefined: Option<PredefinedTextStyle>,
    renderer: &TextRenderer<'_>,
) {
    for range in text_ranges(line) {
        let first = line
            .placements
            .partition_point(|placement| placement.cluster.source.end <= range.start);
        let last = line
            .placements
            .partition_point(|placement| placement.cluster.source.start < range.end);
        let placements = &line.placements[first..last];
        let offset = placements.first().map_or_else(
            || {
                line.objects
                    .iter()
                    .rev()
                    .find(|object| object.object.source.end <= range.start)
                    .map_or(0.0, |object| {
                        object.x - object.object.left_margin + object.object.advance()
                    })
            },
            |placement| placement.x,
        );
        let advance = placements
            .iter()
            .map(|placement| placement.cluster.advance)
            .sum::<f64>();
        render_flow_line(
            svg,
            styled,
            range,
            x + offset,
            x + offset + advance,
            baseline,
            None,
            theme,
            predefined,
            renderer,
        );
    }
}

fn positioned_spans(
    styled: &StyledText<'_>,
    line: &WrappedLine,
    x: f64,
    baseline: f64,
    theme: RenderTheme,
    predefined: Option<PredefinedTextStyle>,
    renderer: &TextRenderer<'_>,
) -> Option<Vec<PositionedSpan>> {
    let contexts = if line.native_positioned {
        Vec::new()
    } else {
        paragraph_bidi_contexts(styled, line.source.start)
    };
    let mut context_index = 0;
    let mut cluster_contexts = Vec::with_capacity(line.placements.len());
    let mut offsets = Vec::with_capacity(line.placements.len());
    if !x.is_finite() || !baseline.is_finite() {
        return None;
    }
    for placement in &line.placements {
        let cluster = &placement.cluster;
        let text = styled.index.slice(cluster.source.clone())?;
        while contexts
            .get(context_index)
            .is_some_and(|range| range.end <= cluster.source.start)
        {
            context_index += 1;
        }
        let context = contexts
            .get(context_index)
            .filter(|range| range.start < cluster.source.end)
            .map(|_| context_index);
        cluster_contexts.push(context);
        if cluster.run.glyphs[cluster.glyphs.clone()]
            .iter()
            .any(|glyph| glyph.raw.id == 0)
        {
            renderer
                .for_source(cluster.source.clone())
                .missing_glyphs(&cluster.run.face.family, text);
            offsets.push(None);
            continue;
        }
        let offset = if line.native_positioned {
            cluster.native_paint_offset(text).ok().flatten()
        } else {
            context
                .is_none()
                .then(|| cluster.paint_offset(text).ok().flatten())
                .flatten()
        };
        if offset.is_none_or(|offset| {
            !(x + placement.x + offset.x).is_finite() || !(baseline + offset.y).is_finite()
        }) {
            renderer
                .for_source(cluster.source.clone())
                .glyph_positioning_unsupported(&cluster.run.face.family, text);
            offsets.push(None);
        } else {
            offsets.push(offset);
        }
    }
    let mut result = Vec::new();
    let mut index = 0;
    for segment in styled.foreground_segments(line.source.clone()) {
        let style = styled.style_at(segment.start, theme, predefined);
        while index < line.placements.len()
            && line.placements[index].cluster.source.start < segment.end
        {
            let first = &line.placements[index];
            let crosses_style = first.cluster.source.start < segment.start
                || first.cluster.source.end > segment.end;
            if crosses_style || offsets[index].is_none() {
                if crosses_style {
                    renderer
                        .for_source(first.cluster.source.clone())
                        .glyph_positioning_unsupported(
                            &first.cluster.run.face.family,
                            styled.index.slice(first.cluster.source.clone())?,
                        );
                }
                let mut source = first.cluster.source.clone();
                let origin = x + first.x;
                let run = &first.cluster.run;
                let context = cluster_contexts[index];
                index += 1;
                while let Some(next) = line.placements.get(index)
                    && offsets[index].is_none()
                    && (line.placements[index - 1].extra_advance == 0.0 || context.is_some())
                    && next.cluster.source.start == source.end
                    && next.cluster.run.face.id == run.face.id
                    && (next.cluster.run.direction == run.direction
                        || context.is_some() && cluster_contexts[index] == context)
                    && next.cluster.run.style.font_size == run.style.font_size
                    && next.cluster.run.coverage_fallback == run.coverage_fallback
                {
                    source.end = next.cluster.source.end;
                    index += 1;
                }
                for (part, source) in styled.foreground_segments(source).enumerate() {
                    let style = styled.style_at(source.start, theme, predefined);
                    renderer
                        .for_source(source.clone())
                        .report_resolution(&style, styled.context());
                    result.push(PositionedSpan {
                        source,
                        positions: if part == 0 { vec![origin] } else { Vec::new() },
                        offset_y: 0.0,
                        style: renderer.output_style_with_face(&style, &run.face),
                        font_face: run
                            .coverage_fallback
                            .then_some((run.face.weight, run.face.style)),
                        positioned: false,
                    });
                }
                continue;
            }
            let offset_y = offsets[index]?.y;
            let mut source = first.cluster.source.clone();
            let mut positions = vec![x + first.x + offsets[index]?.x];
            let face = &first.cluster.run.face;
            let font_face = first
                .cluster
                .run
                .coverage_fallback
                .then_some((face.weight, face.style));
            index += 1;
            if source.len() == 1 {
                while let Some(next) = line.placements.get(index)
                    && next.cluster.source.start == source.end
                    && next.cluster.source.end <= segment.end
                    && next.cluster.source.len() == 1
                    && next.cluster.run.face.id == face.id
                    && offsets[index].is_some()
                    && offsets[index]?.y == offset_y
                {
                    source.end = next.cluster.source.end;
                    positions.push(x + next.x + offsets[index]?.x);
                    index += 1;
                }
            }
            renderer
                .for_source(source.clone())
                .report_resolution(&style, styled.context());
            result.push(PositionedSpan {
                source,
                positions,
                offset_y,
                style: renderer.output_style_with_face(&style, face),
                font_face,
                positioned: true,
            });
        }
    }
    Some(result)
}

pub(in crate::render) fn paragraph_bidi_contexts(
    styled: &StyledText<'_>,
    source_start: usize,
) -> Vec<Range<usize>> {
    styled
        .index
        .paragraph_index(source_start)
        .and_then(|ordinal| styled.index.native_paragraphs().nth(ordinal as usize))
        .and_then(|paragraph| {
            styled
                .index
                .slice(paragraph.content.clone())
                .map(|text| bidi_contexts(text, paragraph.content.start))
        })
        .unwrap_or_default()
}

fn bidi_contexts(text: &str, source_start: usize) -> Vec<Range<usize>> {
    #[derive(PartialEq)]
    enum Context {
        Embedding,
        Isolate,
    }
    let mut contexts = Vec::new();
    let mut stack = Vec::new();
    let mut isolate_depth = 0;
    let mut start = source_start;
    let mut end = source_start;
    for (index, scalar) in text.chars().enumerate() {
        let index = source_start + index;
        end = index + 1;
        let class = bidi_class(scalar);
        match class {
            BidiClass::LRE
            | BidiClass::RLE
            | BidiClass::LRO
            | BidiClass::RLO
            | BidiClass::LRI
            | BidiClass::RLI
            | BidiClass::FSI => {
                if stack.is_empty() {
                    start = index;
                }
                let kind = if matches!(class, BidiClass::LRI | BidiClass::RLI | BidiClass::FSI) {
                    isolate_depth += 1;
                    Context::Isolate
                } else {
                    Context::Embedding
                };
                stack.push(kind);
            }
            BidiClass::PDF if stack.last() == Some(&Context::Embedding) => {
                stack.pop();
                if stack.is_empty() {
                    contexts.push(start..end);
                }
            }
            BidiClass::PDI if isolate_depth > 0 => {
                while stack.pop() != Some(Context::Isolate) {}
                isolate_depth -= 1;
                if stack.is_empty() {
                    contexts.push(start..end);
                }
            }
            _ => {}
        }
    }
    if !stack.is_empty() {
        contexts.push(start..end);
    }
    contexts
}
