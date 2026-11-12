use super::text::{StyledText, TextRenderer};
use super::vector::{Group, ObjectKind, Scene, Styled, Transform};
use super::viewport::Viewport;
use super::{
    ObjectDiagnostic, ObjectDiagnosticKind, RenderTheme, code, embedded, images,
    is_preparation_issue, render_flow_line, table, text,
};
use crate::{
    BoundingBox, MediaAsset, ParagraphAlignment, PredefinedTextStyle, RichTextObjectContent,
    RichTextObjectSpan,
};

enum ObjectDrawingPreparation<'a> {
    Code {
        code: &'a crate::RichTextCodeBlock,
        result: Result<Box<code::PreparedCode>, ObjectDiagnosticKind>,
    },
    Table {
        table: &'a crate::RichTextTable,
        result: Option<Result<table::PreparedTableDrawing, ObjectDiagnosticKind>>,
    },
    Unprepared,
}

#[derive(Clone, Copy)]
enum ObjectDrawingDecision<'a> {
    BodyCode(&'a crate::RichTextCodeBlock, &'a code::PreparedCode),
    DrawingTable(&'a crate::RichTextTable, &'a table::PreparedTableDrawing),
    TranslatedCode(&'a crate::RichTextCodeBlock, &'a code::PreparedCode),
    RetainedTable(&'a crate::RichTextTable, &'a table::PreparedTable),
    BodyTextFallback,
    EmbeddedFallback,
}

impl ObjectDrawingPreparation<'_> {
    fn diagnostic(&self) -> Option<ObjectDiagnosticKind> {
        match self {
            Self::Code {
                result: Err(kind), ..
            }
            | Self::Table {
                result: Some(Err(kind)),
                ..
            } => Some(*kind),
            _ => None,
        }
    }

    fn paint_bounds(&self, mut bounds: BoundingBox, body_clone: bool) -> BoundingBox {
        match self {
            Self::Code {
                result: Ok(prepared),
                ..
            } if body_clone => prepared.panel_bbox,
            Self::Code {
                result: Ok(prepared),
                ..
            } => {
                bounds.y_max = bounds.y_min + prepared.panel_bbox.y_max - prepared.panel_bbox.y_min;
                bounds
            }
            Self::Table {
                result: Some(Ok(prepared)),
                ..
            } => prepared.measured_bbox,
            _ => bounds,
        }
    }

    fn has_native_table_clipping(&self) -> bool {
        matches!(self, Self::Table { result: Some(Ok(prepared)), .. }
            if prepared.has_native_text_clipping())
    }

    fn resolve<'a>(
        &'a self,
        retained: &'a Option<Result<embedded::PreparedObject, ObjectDiagnosticKind>>,
        body_clone: bool,
    ) -> ObjectDrawingDecision<'a> {
        match self {
            Self::Code {
                code,
                result: Ok(prepared),
            } if body_clone => ObjectDrawingDecision::BodyCode(code, prepared),
            Self::Code {
                code,
                result: Ok(prepared),
            } => ObjectDrawingDecision::TranslatedCode(code, prepared),
            Self::Table {
                table,
                result: Some(Ok(prepared)),
            } => ObjectDrawingDecision::DrawingTable(table, prepared),
            _ if body_clone && self.diagnostic().is_some() => {
                ObjectDrawingDecision::BodyTextFallback
            }
            Self::Code { code, .. } => match retained {
                Some(Ok(embedded::PreparedObject::Code(prepared))) => {
                    ObjectDrawingDecision::TranslatedCode(code, prepared)
                }
                _ => ObjectDrawingDecision::EmbeddedFallback,
            },
            Self::Table { table, .. } => match retained {
                Some(Ok(embedded::PreparedObject::Table(prepared))) => {
                    ObjectDrawingDecision::RetainedTable(table, prepared)
                }
                _ => ObjectDrawingDecision::EmbeddedFallback,
            },
            Self::Unprepared => ObjectDrawingDecision::EmbeddedFallback,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn paint_line_objects(
    svg: &mut Scene,
    styled: &StyledText<'_>,
    line: &text::WrappedLine,
    native_entry: Option<text::NativeObjectEntryBounds>,
    left: f64,
    available_width: f64,
    baseline: f64,
    alignment: Option<ParagraphAlignment>,
    predefined: Option<PredefinedTextStyle>,
    media_assets: &[MediaAsset],
    theme: RenderTheme,
    renderer: &TextRenderer<'_>,
    viewport: Option<Viewport>,
) {
    let canonical = svg.retains_text();
    let advance = line.advance_for_paint(canonical).unwrap_or(line.advance);
    let left = left + text::line_alignment_offset(advance, available_width, alignment);
    for (index, placement) in line.objects.iter().enumerate() {
        let Some(position) = line.object_position(index, canonical) else {
            continue;
        };
        let placement_x = position.x;
        let object = &placement.object;
        let Some(span) = styled.object_span(object.span_index) else {
            continue;
        };
        if matches!(placement.prepared, Some(Err(_))) {
            continue;
        }
        svg.inline_text_object(object.source.start, |svg| {
            let height = object.height;
            let drawing_renderer = renderer
                .for_object_source(object.source.clone())
                .planning_scope();
            let body_clone = object.context == text::ObjectMeasurementContext::Body
                && match &span.content {
                    Some(RichTextObjectContent::CodeBlock(code)) => {
                        code.rotation_degrees.is_none_or(|rotation| rotation == 0.0)
                    }
                    Some(RichTextObjectContent::Table(table)) => table
                        .rotation_degrees
                        .is_none_or(|rotation| rotation == 0.0),
                    _ => false,
                };
            let target = BoundingBox {
                x_min: left + placement_x,
                x_max: left + placement_x + object.width(),
                y_min: baseline - height,
                y_max: baseline,
            };
            let preparation = match (&placement.prepared, &span.content) {
                (
                    Some(Ok(embedded::PreparedObject::Code(_))),
                    Some(RichTextObjectContent::CodeBlock(code)),
                ) => ObjectDrawingPreparation::Code {
                    code,
                    result: if body_clone {
                        code::prepare_code_drawing(
                            code,
                            span.layout_constraint,
                            target,
                            theme,
                            &drawing_renderer,
                        )
                    } else {
                        code::prepare_code(
                            code,
                            span.layout_constraint,
                            baseline - height,
                            theme,
                            &drawing_renderer,
                        )
                    }
                    .map(Box::new),
                },
                (_, Some(RichTextObjectContent::Table(table))) => ObjectDrawingPreparation::Table {
                    table,
                    result: if body_clone {
                        let native_source = native_entry
                            .filter(|_| {
                                renderer
                                    .table_export_page
                                    .is_none_or(table::TableExportPage::has_zero_origin)
                            })
                            .and_then(|entry| {
                                let Some(Ok(embedded::PreparedObject::Table(prepared))) =
                                    &placement.prepared
                                else {
                                    return None;
                                };
                                Some(table::NativeTableDocumentSource::from_entry(
                                    entry, prepared,
                                ))
                            });
                        table::prepare_table_clone_drawing_with_native_entry(
                            table,
                            span.layout_constraint,
                            target,
                            theme,
                            &drawing_renderer,
                            native_source,
                        )
                    } else {
                        table::prepare_table_drawing(
                            table,
                            span.layout_constraint,
                            [left + placement_x, baseline - height],
                            theme,
                            &drawing_renderer,
                        )
                    },
                },
                _ => ObjectDrawingPreparation::Unprepared,
            };
            let paint_bounds = preparation.paint_bounds(
                object_paint_bounds(placement, left + placement_x - placement.x, baseline),
                body_clone,
            );
            if !preparation.has_native_table_clipping()
                && viewport.is_some_and(|viewport| !viewport.intersects(paint_bounds))
            {
                return;
            }
            let decision = preparation.resolve(&placement.prepared, body_clone);
            match decision {
                ObjectDrawingDecision::BodyCode(code, prepared) => {
                    code::paint(svg, code, prepared, media_assets, theme, renderer, viewport);
                    report_drawing_layout_issues(renderer, &drawing_renderer);
                    return;
                }
                ObjectDrawingDecision::DrawingTable(table, prepared) => {
                    table::render_table(
                        svg,
                        table,
                        span.text_index_utf16,
                        Some(prepared.into()),
                        0.0,
                        media_assets,
                        theme,
                        renderer,
                        viewport,
                    );
                    report_drawing_layout_issues(renderer, &drawing_renderer);
                    return;
                }
                _ => {}
            }
            report_drawing_layout_issues(renderer, &drawing_renderer);
            if let Some(kind) = preparation.diagnostic() {
                renderer.report_object_issues(&[ObjectDiagnostic {
                    anchor_utf16: span.text_index_utf16,
                    kind,
                }]);
            }
            if matches!(decision, ObjectDrawingDecision::BodyTextFallback) {
                render_flow_line(
                    svg,
                    styled,
                    object.source.clone(),
                    target.x_min,
                    target.x_max,
                    baseline,
                    None,
                    theme,
                    predefined,
                    renderer,
                );
                return;
            }
            let offset = (
                left + placement_x - object.bounds.x_min,
                baseline - height - object.bounds.y_min,
            );
            svg.scope(
                Group::new().transformed(Transform::translate(offset.0, 0.0, 5)),
                |svg| {
                    if let ObjectDrawingDecision::TranslatedCode(code, prepared) = decision {
                        let shift = baseline - height - prepared.panel_bbox.y_min;
                        svg.scope(
                            Group::new().transformed(Transform::translate(0.0, shift, 5)),
                            |svg| {
                                code::paint(
                                    svg,
                                    code,
                                    prepared,
                                    media_assets,
                                    theme,
                                    renderer,
                                    viewport.map(|viewport| viewport.translated(-offset.0, -shift)),
                                );
                            },
                        );
                    } else if let ObjectDrawingDecision::RetainedTable(table, prepared) = decision {
                        let shift = (object.bounds.x_min, baseline - height);
                        svg.scope(
                            Group::new().transformed(Transform::translate(shift.0, shift.1, 5)),
                            |svg| {
                                table::render_table(
                                    svg,
                                    table,
                                    span.text_index_utf16,
                                    Some(prepared.into()),
                                    0.0,
                                    media_assets,
                                    theme,
                                    &renderer.translated_paint(-offset.0 - shift.0, -shift.1),
                                    viewport.map(|viewport| {
                                        viewport.translated(-offset.0 - shift.0, -shift.1)
                                    }),
                                );
                            },
                        );
                    } else if render_embedded_object(
                        svg,
                        span,
                        offset.1,
                        media_assets,
                        theme,
                        &renderer
                            .for_object_source(object.source.clone())
                            .translated_paint(-offset.0, 0.0),
                        viewport.map(|viewport| viewport.translated(-offset.0, 0.0)),
                    )
                    .is_none()
                    {
                        render_flow_line(
                            svg,
                            styled,
                            object.source.clone(),
                            object.bounds.x_min,
                            object.bounds.x_min,
                            baseline,
                            None,
                            theme,
                            predefined,
                            renderer,
                        );
                    }
                },
            );
        });
    }
}
pub(super) fn object_paint_bounds(
    object: &text::PositionedObject,
    left: f64,
    baseline: f64,
) -> BoundingBox {
    let (x_min, width, height) = match &object.prepared {
        Some(Ok(embedded::PreparedObject::Code(prepared))) => (
            left + object.x + prepared.panel_bbox.x_min - object.object.bounds.x_min,
            prepared.panel_bbox.x_max - prepared.panel_bbox.x_min,
            prepared.panel_bbox.y_max - prepared.panel_bbox.y_min,
        ),
        Some(Ok(embedded::PreparedObject::Table(prepared))) => (
            left + object.x,
            prepared.measured_bbox.x_max - prepared.measured_bbox.x_min,
            prepared.measured_bbox.y_max - prepared.measured_bbox.y_min,
        ),
        _ => (
            left + object.x,
            object.object.bounds.x_max - object.object.bounds.x_min,
            object.object.height,
        ),
    };
    let y_min = baseline - object.object.height;
    BoundingBox {
        x_min,
        x_max: x_min + width,
        y_min,
        y_max: y_min + height,
    }
}
fn render_embedded_object(
    svg: &mut Scene,
    object: &RichTextObjectSpan,
    offset_y: f64,
    media_assets: &[MediaAsset],
    theme: RenderTheme,
    renderer: &TextRenderer<'_>,
    viewport: Option<Viewport>,
) -> Option<f64> {
    match object.content.as_ref() {
        Some(RichTextObjectContent::Image(image)) => {
            let drawn = images::image_drawn_bbox(image);
            if ![drawn.x_min, drawn.y_min, drawn.x_max, drawn.y_max]
                .iter()
                .all(|value| value.is_finite())
                || drawn.x_max <= drawn.x_min
                || drawn.y_max <= drawn.y_min
            {
                return None;
            }
            svg.scope(
                Group::new()
                    .object(ObjectKind::Image)
                    .transformed(Transform::translate(0., offset_y, 4)),
                |svg| {
                    images::render_placed_image(
                        svg,
                        image,
                        media_assets,
                        renderer.image_page_width,
                    );
                },
            );
            Some(drawn.y_max + offset_y)
        }
        Some(RichTextObjectContent::Table(table)) => {
            table::render_table(
                svg,
                table,
                object.text_index_utf16,
                None,
                offset_y,
                media_assets,
                theme,
                renderer,
                viewport,
            );
            Some(table.bbox.y_max + offset_y)
        }
        Some(RichTextObjectContent::CodeBlock(code)) => {
            let prepared = match code::prepare_code(
                code,
                object.layout_constraint,
                code.bbox.y_min + offset_y,
                theme,
                renderer,
            ) {
                Ok(prepared) => prepared,
                Err(kind) => {
                    renderer.report_object_issues(&[text::ObjectDiagnostic {
                        anchor_utf16: object.text_index_utf16,
                        kind,
                    }]);
                    return None;
                }
            };
            code::paint(
                svg,
                code,
                &prepared,
                media_assets,
                theme,
                renderer,
                viewport,
            );
            Some(prepared.panel_bbox.y_max)
        }
        None => None,
    }
}

fn report_drawing_layout_issues(renderer: &TextRenderer<'_>, drawing: &TextRenderer<'_>) {
    renderer.report_text_issues(
        &drawing
            .scoped_diagnostics()
            .into_iter()
            .filter(|issue| is_preparation_issue(&issue.diagnostic))
            .collect::<Vec<_>>(),
    );
    renderer.report_owned_object_issues(&drawing.scoped_object_diagnostics());
}
