use std::collections::{BTreeMap, HashMap};
use std::ops::Range;

use crate::Color;
use crate::render::Viewport;
use crate::render::color_hex;
use crate::render::vector::{FontFamily, Scene, Text, TextAnchor};

use super::super::layout::{TextLayout, TextLine};
use super::super::measurement::MeasuredCluster;
use super::super::native_paint_plan::{
    NativePaintPlan, NativePaintPlanUnavailable, NativePaintRun, native_paint_plan,
};
use super::*;

struct NativeSvgSpan {
    source: Range<usize>,
    positions: Vec<f64>,
    baseline: f64,
    style: TextStyle,
}

struct NativePaintGroup {
    run: NativePaintRun,
    spans: Vec<NativeSvgSpan>,
    #[cfg(feature = "pdf")]
    retained: Option<super::super::native::NativeTextBlock>,
    #[cfg(feature = "pdf")]
    inkless_carrier: bool,
}

pub(in crate::render) struct NativePaintDispatcher {
    plan: NativePaintPlan,
    groups: Vec<NativePaintGroup>,
}

impl NativePaintDispatcher {
    pub fn new(
        styled: &StyledText<'_>,
        layout: &TextLayout,
        theme: RenderTheme,
        renderer: &TextRenderer<'_>,
        visible_lines: &[bool],
        retain_text: bool,
        viewport: Option<Viewport>,
    ) -> Option<Self> {
        let plan = match native_paint_plan(styled, layout, theme) {
            Ok(plan) => plan,
            Err(NativePaintPlanUnavailable::OutsideCertificate(_)) => return None,
            Err(_) => {
                report_unavailable(styled, layout, visible_lines, renderer);
                return None;
            }
        };
        #[cfg(feature = "pdf")]
        let bridge = super::super::native::NativePaintBridge::new(&plan);
        let clusters = layout
            .lines
            .iter()
            .flat_map(|line| &line.line.placements)
            .map(|placement| (placement.cluster.source.start, &placement.cluster))
            .collect::<BTreeMap<_, _>>();
        let groups =
            project_groups(&plan, styled, layout, visible_lines, viewport).and_then(|runs| {
                runs.into_iter()
                    .map(|run| {
                        let spans = if retain_text {
                            vec![carrier_span(&plan, styled, &run, theme)?]
                        } else {
                            svg_spans(&plan, styled, &clusters, &run, theme)?
                        };
                        #[cfg(feature = "pdf")]
                        let retained = retain_text
                            .then(|| bridge.block(&run))
                            .transpose()
                            .map_err(|_| ())?;
                        #[cfg(feature = "pdf")]
                        let inkless_carrier = retain_text
                            && run.glyphs.iter().all(|glyph| {
                                run.face
                                    .glyph_ink_bounds(glyph.glyph_id)
                                    .is_ok_and(|bounds| bounds.is_none())
                            });
                        Ok(NativePaintGroup {
                            run,
                            spans,
                            #[cfg(feature = "pdf")]
                            retained,
                            #[cfg(feature = "pdf")]
                            inkless_carrier,
                        })
                    })
                    .collect::<Result<Vec<_>, ()>>()
            });
        match groups {
            Ok(groups) => Some(Self { plan, groups }),
            Err(()) => {
                report_unavailable(styled, layout, visible_lines, renderer);
                None
            }
        }
    }

    pub fn paint_line(
        &mut self,
        svg: &mut Scene,
        styled: &StyledText<'_>,
        line_index: usize,
        line: &TextLine,
        theme: RenderTheme,
        renderer: &TextRenderer<'_>,
    ) {
        for group in self
            .groups
            .iter_mut()
            .filter(|group| group.run.first_line == line_index)
        {
            let origin = [
                f64::from(group.run.origin[0]) + self.plan.translation[0],
                f64::from(group.run.origin[1]) + self.plan.translation[1],
            ];
            let node = Text::new("")
                .x(origin[0])
                .y(origin[1])
                .anchor(TextAnchor::Start)
                .family(FontFamily::Named(&group.run.face.svg_family()))
                .preserve_space();
            #[cfg(feature = "pdf")]
            let node = if let Some(block) = group.retained.take() {
                match svg
                    .native_text()
                    .register(group.run.source.characters().start, block)
                {
                    Ok(id) => node.native_id(id),
                    Err(error) => {
                        svg.reject_native_text(error.to_string());
                        continue;
                    }
                }
            } else {
                node
            };
            svg.scope(node, |svg| {
                for span in &group.spans {
                    let style = renderer.output_style_with_face(&span.style, &group.run.face);
                    renderer
                        .for_source(span.source.clone())
                        .report_resolution(&span.style, styled.context());
                    let node = styled_tspan(
                        styled.index.slice(span.source.clone()).unwrap(),
                        &style,
                        styled.context(),
                    )
                    .x_coordinates(&span.positions)
                    .y(span.baseline)
                    .font_face(group.run.face.weight, group.run.face.style)
                    .font_synthesis(group.run.synthesis());
                    push_text_span(svg, node, &style);
                }
                #[cfg(feature = "pdf")]
                if group.inkless_carrier {
                    svg.push(
                        crate::render::vector::TSpan::new(".")
                            .font_size(1)
                            .family(FontFamily::Named(&group.run.face.svg_family()))
                            .font_face(group.run.face.weight, group.run.face.style),
                    );
                }
            });
        }
        let x = line.x
            + super::super::line_alignment_offset(line.line.advance, line.width, line.alignment);
        render_retained_decorations(
            svg,
            styled,
            &line.line,
            x,
            line.baseline,
            theme,
            line.predefined,
        );
    }
}

fn report_unavailable(
    styled: &StyledText<'_>,
    layout: &TextLayout,
    visible_lines: &[bool],
    renderer: &TextRenderer<'_>,
) {
    for (line, visible) in layout.lines.iter().zip(visible_lines) {
        if *visible && !line.line.source.is_empty() {
            renderer
                .for_source(line.line.source.clone())
                .glyph_positioning_unsupported(
                    "native",
                    styled
                        .index
                        .slice(line.line.source.clone())
                        .unwrap_or_default(),
                );
        }
    }
}

fn project_groups(
    plan: &NativePaintPlan,
    styled: &StyledText<'_>,
    layout: &TextLayout,
    visible_lines: &[bool],
    viewport: Option<Viewport>,
) -> Result<Vec<NativePaintRun>, ()> {
    let mut windows: Vec<Range<usize>> = Vec::new();
    for (line, visible) in layout.lines.iter().zip(visible_lines) {
        if !visible || line.line.source.is_empty() {
            continue;
        }
        let source = &line.line.source;
        if let Some(previous) = windows.last_mut()
            && previous.end == source.start
        {
            previous.end = source.end;
        } else {
            windows.push(source.clone());
        }
    }
    let mut groups = Vec::new();
    for run in &plan.runs {
        let bounds = if run.ink.0[0] < run.ink.0[2] && run.ink.0[1] < run.ink.0[3] {
            run.ink
        } else {
            run.layout
        };
        let [left, top, right, bottom] = bounds.0.map(f64::from);
        let bounds = crate::BoundingBox {
            x_min: left + plan.translation[0],
            y_min: top + plan.translation[1],
            x_max: right + plan.translation[0],
            y_max: bottom + plan.translation[1],
        };
        if viewport.is_some_and(|viewport| !viewport.intersects(bounds)) {
            continue;
        }
        let first = windows.partition_point(|window| window.end <= run.source.characters().start);
        for window in windows[first..]
            .iter()
            .take_while(|window| window.start < run.source.characters().end)
        {
            let source = run.source.characters();
            let source = source.start.max(window.start)..source.end.min(window.end);
            if source.is_empty() {
                continue;
            }
            let glyphs = run
                .glyphs
                .iter()
                .filter(|glyph| source.contains(&glyph.source.characters().start))
                .map(|glyph| {
                    (glyph.source.characters().end <= source.end)
                        .then(|| glyph.clone())
                        .ok_or(())
                })
                .collect::<Result<Vec<_>, _>>()?;
            if glyphs.is_empty() {
                continue;
            }
            let owner = glyphs[0].source.characters().start;
            let first_line = layout
                .lines
                .partition_point(|line| line.line.source.end <= owner);
            if !layout
                .lines
                .get(first_line)
                .is_some_and(|line| line.line.source.contains(&owner))
            {
                return Err(());
            }
            let mut projected = run.clone();
            projected.source = styled.index.source(source).ok_or(())?;
            projected.first_line = first_line;
            projected.glyphs = glyphs;
            groups.push(projected);
        }
    }
    Ok(groups)
}

fn paint_style(
    styled: &StyledText<'_>,
    source: usize,
    run: &NativePaintRun,
    theme: RenderTheme,
) -> TextStyle {
    let mut style = styled.style_at(source, theme, None);
    style.font_size = f64::from(run.font_size);
    style.color = color_hex(&Color {
        r: (run.foreground >> 16) as u8,
        g: (run.foreground >> 8) as u8,
        b: run.foreground as u8,
    });
    style.bold = run.style_bits & 1 != 0;
    style.italic = run.style_bits & 2 != 0;
    style.underline = false;
    style.strikethrough = false;
    style
}

fn carrier_span(
    plan: &NativePaintPlan,
    styled: &StyledText<'_>,
    run: &NativePaintRun,
    theme: RenderTheme,
) -> Result<NativeSvgSpan, ()> {
    let source = run.source.characters().clone();
    styled.index.slice(source.clone()).ok_or(())?;
    Ok(NativeSvgSpan {
        style: paint_style(styled, source.start, run, theme),
        source,
        positions: vec![f64::from(run.origin[0]) + plan.translation[0]],
        baseline: f64::from(run.origin[1]) + plan.translation[1],
    })
}

fn svg_spans(
    plan: &NativePaintPlan,
    styled: &StyledText<'_>,
    clusters: &BTreeMap<usize, &MeasuredCluster>,
    run: &NativePaintRun,
    theme: RenderTheme,
) -> Result<Vec<NativeSvgSpan>, ()> {
    let mut spans: Vec<NativeSvgSpan> = Vec::new();
    let mut owners: HashMap<usize, Vec<_>> = HashMap::new();
    for glyph in &run.glyphs {
        owners
            .entry(glyph.source.characters().start)
            .or_default()
            .push(glyph);
    }
    for (_, cluster) in clusters.range(run.source.characters().clone()) {
        if cluster.source.end > run.source.characters().end {
            return Err(());
        }
        let glyphs = owners.get(&cluster.source.start).ok_or(())?;
        if glyphs
            .iter()
            .any(|glyph| glyph.source.characters() != &cluster.source)
        {
            return Err(());
        }
        let original = &cluster.run.glyphs[cluster.glyphs.clone()];
        if original.is_empty() || glyphs.len() != original.len() {
            return Err(());
        }
        let text = styled.index.slice(cluster.source.clone()).ok_or(())?;
        let offset = cluster
            .native_paint_offset(text)
            .map_err(|_| ())?
            .ok_or(())?;
        let base = [
            f64::from(glyphs[0].origin[0]) - original[0].owner_offset[0],
            f64::from(glyphs[0].origin[1]) - original[0].owner_offset[1],
        ];
        if original.iter().zip(glyphs).any(|(original, glyph)| {
            glyph.glyph_id != original.id
                || (0..2).any(|axis| {
                    (base[axis] + original.owner_offset[axis]) as f32 != glyph.origin[axis]
                })
        }) {
            return Err(());
        }
        let position = base[0] + offset.x + plan.translation[0];
        let baseline = base[1] + offset.y + plan.translation[1];
        if ![position, baseline].into_iter().all(f64::is_finite) {
            return Err(());
        }
        let style = paint_style(styled, cluster.source.start, run, theme);
        if cluster.source.len() == 1
            && let Some(previous) = spans.last_mut()
            && previous.source.len() == previous.positions.len()
            && previous.source.end == cluster.source.start
            && previous.baseline == baseline
            && previous.style.link_target == style.link_target
        {
            previous.source.end = cluster.source.end;
            previous.positions.push(position);
        } else {
            spans.push(NativeSvgSpan {
                source: cluster.source.clone(),
                positions: vec![position],
                baseline,
                style,
            });
        }
    }
    Ok(spans)
}

#[cfg(all(test, feature = "serde"))]
mod tests;
