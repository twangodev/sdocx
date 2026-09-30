use std::ops::Range;

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
    let remaining = (available_width - line.advance).max(0.0);
    let x = left
        + match alignment {
            Some(ParagraphAlignment::Center) => remaining / 2.0,
            Some(ParagraphAlignment::Right) => remaining,
            _ => 0.0,
        };
    if line.placements.is_empty()
        || line
            .objects
            .iter()
            .any(|object| matches!(object.prepared_code, Some(Err(_))))
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
                style.underline = false;
                style.strikethrough = false;
                let node = styled_tspan(
                    styled.index.slice(span.source.clone()).unwrap(),
                    &style,
                    styled.context(),
                )
                .x_positions(&span.positions, 5)
                .y(decimal(baseline + span.offset_y, 5));
                push_text_span(svg, node, &style);
            }
        },
    );
    for segment in text_ranges(line)
        .into_iter()
        .flat_map(|range| styled.segments(range))
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
        let start = x + line.placements[first].x;
        let end = x + line.placements[last - 1].x + line.placements[last - 1].cluster.advance;
        let thickness = style.font_size * f64::from(1.0_f32 / 18.0);
        for offset in [
            style.underline.then_some(f64::from(1.0_f32 / 9.0)),
            style.strikethrough.then_some(f64::from(-2.0_f32 / 7.0)),
        ]
        .into_iter()
        .flatten()
        {
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

fn text_ranges(line: &WrappedLine) -> Vec<Range<usize>> {
    let mut ranges = Vec::with_capacity(line.objects.len() + 1);
    let mut start = line.source.start;
    for object in &line.objects {
        if matches!(object.prepared_code, Some(Err(_))) {
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
                        object.x + (object.object.bounds.x_max - object.object.bounds.x_min)
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
    let mut offsets = Vec::with_capacity(line.placements.len());
    let mut supported = x.is_finite() && baseline.is_finite();
    for placement in &line.placements {
        let cluster = &placement.cluster;
        let text = styled.index.slice(cluster.source.clone())?;
        if cluster.run.glyphs[cluster.glyphs.clone()]
            .iter()
            .any(|glyph| glyph.raw.id == 0)
        {
            renderer.missing_glyphs(&cluster.run.face.family, text);
            supported = false;
            offsets.push(None);
            continue;
        }
        let offset = cluster.paint_offset(text).ok().flatten();
        if offset.is_none_or(|offset| {
            !(x + placement.x + offset.x).is_finite() || !(baseline + offset.y).is_finite()
        }) {
            renderer.glyph_positioning_unsupported(&cluster.run.face.family, text);
            supported = false;
        }
        offsets.push(offset);
    }
    if !supported {
        return None;
    }
    let mut result = Vec::new();
    let mut index = 0;
    for segment in styled.segments(line.source.clone()) {
        let style = styled.style_at(segment.start, theme, predefined);
        while index < line.placements.len()
            && line.placements[index].cluster.source.start < segment.end
        {
            let first = &line.placements[index];
            if first.cluster.source.start < segment.start || first.cluster.source.end > segment.end
            {
                renderer.glyph_positioning_unsupported(
                    &first.cluster.run.face.family,
                    styled.index.slice(first.cluster.source.clone())?,
                );
                return None;
            }
            let offset_y = offsets[index]?.y;
            let mut source = first.cluster.source.clone();
            let mut positions = vec![x + first.x + offsets[index]?.x];
            let face = &first.cluster.run.face;
            index += 1;
            if source.len() == 1 {
                while let Some(next) = line.placements.get(index)
                    && next.cluster.source.start == source.end
                    && next.cluster.source.end <= segment.end
                    && next.cluster.source.len() == 1
                    && next.cluster.run.face.id == face.id
                    && offsets[index]?.y == offset_y
                {
                    source.end = next.cluster.source.end;
                    positions.push(x + next.x + offsets[index]?.x);
                    index += 1;
                }
            }
            result.push(PositionedSpan {
                source,
                positions,
                offset_y,
                style: renderer.output_style_with_face(&style, face),
            });
        }
    }
    Some(result)
}
