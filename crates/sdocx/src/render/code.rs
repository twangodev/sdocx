use crate::{BoundingBox, ObjectSpanLayoutConstraint, RichTextBox, RichTextCodeBlock};

use super::RenderTheme;
use super::text::{
    ObjectDiagnosticKind, StyledText, TextContext, TextFrame, TextLayout, TextRenderer,
    VerticalExclusion, finite_native_geometry,
};

pub(super) struct PreparedCode {
    pub copy: BoundingBox,
    pub panel_bbox: BoundingBox,
    pub title_layout: Option<TextLayout>,
    pub body_layout: Option<TextLayout>,
    pub min_first_page_height: f64,
    constraint: ObjectSpanLayoutConstraint,
}

impl PreparedCode {
    pub fn minimum_first_page_height(&self) -> Option<f64> {
        matches!(
            self.constraint,
            ObjectSpanLayoutConstraint::OverPages
                | ObjectSpanLayoutConstraint::OverPagesOverlapPadding
        )
        .then_some(self.min_first_page_height)
    }
}

fn valid_box(bbox: BoundingBox) -> bool {
    let width = bbox.x_max - bbox.x_min;
    let height = bbox.y_max - bbox.y_min;
    width > 0.0
        && height > 0.0
        && [
            bbox.x_min, bbox.y_min, bbox.x_max, bbox.y_max, width, height,
        ]
        .into_iter()
        .all(|value| finite_native_geometry(value).is_some())
}

fn valid_layout(layout: &TextLayout) -> bool {
    finite_native_geometry(layout.height()).is_some()
        && layout.lines.iter().all(|line| {
            line.width >= 0.0
                && line.line.font_size >= 0.0
                && [
                    line.x,
                    line.width,
                    line.baseline,
                    line.line.font_size,
                    line.line.advance,
                ]
                .into_iter()
                .all(|value| finite_native_geometry(value).is_some())
        })
}

fn layout_code_text(
    content: &RichTextBox,
    bbox: BoundingBox,
    exclusions: &[VerticalExclusion],
    theme: RenderTheme,
    renderer: &TextRenderer<'_>,
) -> TextLayout {
    let styled = StyledText::with_span_producer(
        content,
        TextContext::Flow,
        renderer.settings,
        super::text::TextSpanProducer::Drawing,
    );
    super::text::layout_text(
        &styled,
        TextFrame {
            bbox,
            gravity: content.gravity,
            exclusions,
        },
        theme,
        renderer,
    )
}

pub(super) fn prepare_code(
    code: &RichTextCodeBlock,
    constraint: ObjectSpanLayoutConstraint,
    candidate_top: f64,
    theme: RenderTheme,
    renderer: &TextRenderer<'_>,
) -> Result<PreparedCode, ObjectDiagnosticKind> {
    prepare_code_frame(code, code.bbox, constraint, candidate_top, theme, renderer)
}

fn prepare_code_frame(
    code: &RichTextCodeBlock,
    bbox: BoundingBox,
    constraint: ObjectSpanLayoutConstraint,
    candidate_top: f64,
    theme: RenderTheme,
    renderer: &TextRenderer<'_>,
) -> Result<PreparedCode, ObjectDiagnosticKind> {
    let settings = renderer.settings;
    let background = super::argb_color(if theme.is_dark() { 0x333333 } else { 0xefefef });
    let theme = theme.on_background(background);
    let object_top = candidate_top;
    let left = settings.pixels(16.0);
    let top = settings.pixels(12.0);
    let right = settings.pixels(16.0);
    let bottom = settings.pixels(12.0);
    let title_copy_gap = settings.pixels(12.0);
    let body_gap = settings.pixels(8.0);
    let copy_size = settings.pixels(24.0);
    let copy = BoundingBox {
        x_min: bbox.x_max - right - copy_size,
        y_min: object_top + top,
        x_max: bbox.x_max - right,
        y_max: object_top + top + copy_size,
    };
    let title_bbox = BoundingBox {
        x_min: bbox.x_min + left,
        y_min: copy.y_min,
        x_max: copy.x_min - title_copy_gap,
        y_max: copy.y_max,
    };
    let body_top = copy.y_max + body_gap;
    let body_bbox = BoundingBox {
        x_min: bbox.x_min + left,
        y_min: body_top,
        x_max: bbox.x_max - right,
        y_max: body_top,
    };
    let exclusions = renderer.object_exclusions(constraint, candidate_top, 0.0);
    let title_layout = code
        .title
        .as_ref()
        .map(|title| layout_code_text(title, title_bbox, &[], theme, renderer));
    let body_layout = code
        .body
        .as_ref()
        .map(|body| layout_code_text(body, body_bbox, &exclusions, theme, renderer));
    let body_height = body_layout.as_ref().map_or(0.0, TextLayout::height);
    let title_height = title_layout.as_ref().map_or(0.0, TextLayout::height);
    let first_body_line_height = body_layout
        .as_ref()
        .and_then(|layout| layout.lines.first())
        .map_or(0.0, |line| line.bottom - line.top);
    let min_first_page_height = top + title_height + body_gap + first_body_line_height;
    let panel_bbox = BoundingBox {
        y_min: object_top,
        y_max: body_top + body_height + body_gap + bottom,
        ..bbox
    };
    if !valid_box(panel_bbox)
        || !valid_box(copy)
        || !title_layout
            .iter()
            .chain(body_layout.iter())
            .all(valid_layout)
    {
        return Err(ObjectDiagnosticKind::InvalidBounds);
    }
    Ok(PreparedCode {
        copy,
        panel_bbox,
        title_layout,
        body_layout,
        min_first_page_height,
        constraint,
    })
}

pub(super) fn prepare_code_drawing(
    code: &RichTextCodeBlock,
    constraint: ObjectSpanLayoutConstraint,
    target: BoundingBox,
    theme: RenderTheme,
    renderer: &TextRenderer<'_>,
) -> Result<PreparedCode, ObjectDiagnosticKind> {
    let bbox = super::embedded::cloned_raw_bounds(code.bbox, code.bbox, target)?;
    prepare_code_frame(code, bbox, constraint, bbox.y_min, theme, renderer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fonts::FontBook;
    use crate::render::text::{PageExclusions, TextSettings};
    use crate::{Document, DocumentMetadata, Page};

    fn code() -> RichTextCodeBlock {
        RichTextCodeBlock {
            bbox: BoundingBox {
                x_min: 100.0,
                y_min: 200.0,
                x_max: 700.0,
                y_max: 250.0,
            },
            rotation_degrees: None,
            title: None,
            body: Some(RichTextBox {
                text_area_type: None,
                bbox: BoundingBox::default(),
                rotation_degrees: None,
                text: "A\nB".into(),
                color: None,
                highlight_color: None,
                underline: false,
                font_size: Some(15.0),
                runs: Vec::new(),
                spans: Vec::new(),
                paragraphs: Vec::new(),
                object_spans: Vec::new(),
                text_sections: Vec::new(),
                margins: None,
                gravity: None,
            }),
        }
    }

    fn prepare(code: &RichTextCodeBlock, candidate_top: f64) -> PreparedCode {
        let fonts = FontBook::default();
        let renderer = TextRenderer::new(
            TextSettings {
                scale: 3.0,
                font_size_delta: 0.0,
                ..Default::default()
            },
            &fonts,
        );
        prepare_code(
            code,
            ObjectSpanLayoutConstraint::OverPages,
            candidate_top,
            RenderTheme::for_canvas(false),
            &renderer,
        )
        .unwrap()
    }

    #[test]
    fn cloned_drawing_frame_rewraps_without_replacing_callback_height() {
        let mut content = code();
        content.body.as_mut().unwrap().text = "AAAAAAAA".into();
        let callback = prepare(&content, 200.0);
        let fonts = FontBook::default();
        let renderer = TextRenderer::new(
            TextSettings {
                scale: 3.0,
                ..Default::default()
            },
            &fonts,
        );
        let drawing = prepare_code_drawing(
            &content,
            ObjectSpanLayoutConstraint::OverPages,
            BoundingBox {
                x_min: 10.0,
                y_min: 20.0,
                x_max: 190.0,
                y_max: 300.0,
            },
            RenderTheme::for_canvas(false),
            &renderer,
        )
        .unwrap();
        assert_eq!(callback.body_layout.as_ref().unwrap().lines.len(), 1);
        assert_eq!(drawing.body_layout.as_ref().unwrap().lines.len(), 4);
        assert_eq!(drawing.panel_bbox.x_min, 10.000001907348633);
        assert_eq!(drawing.panel_bbox.x_max, 190.00001525878906);
        assert!(
            drawing.panel_bbox.y_max - drawing.panel_bbox.y_min
                > callback.panel_bbox.y_max - callback.panel_bbox.y_min
        );
        assert_eq!(content.bbox.x_max - content.bbox.x_min, 600.0);
    }

    #[test]
    fn minimum_first_page_height_uses_measured_title_and_first_line_only() {
        let mut content = code();
        let mut title = content.body.clone().unwrap();
        title.text = "language".into();
        title.gravity = Some(1);
        content.title = Some(title);
        let plain = prepare(&content, 200.0);
        assert_eq!(plain.min_first_page_height, 181.5);
        assert!((plain.title_layout.as_ref().unwrap().height() - 60.75).abs() < 1e-10);
        assert_eq!(plain.copy.y_max - plain.copy.y_min, 72.0);

        let body = content.body.as_mut().unwrap();
        body.paragraphs = [
            (crate::RichTextParagraphType::SpacingBefore, 7.0_f32),
            (crate::RichTextParagraphType::SpacingAfter, 9.0_f32),
        ]
        .into_iter()
        .map(|(kind, value)| crate::RichTextParagraph {
            kind,
            start_paragraph: 0,
            end_paragraph: 1,
            payload: value.to_le_bytes().to_vec(),
        })
        .collect();
        let spaced = prepare(&content, 200.0);
        assert_eq!(spaced.min_first_page_height, 181.5);
        assert_eq!(spaced.panel_bbox.y_max - plain.panel_bbox.y_max, 48.0);
    }

    #[test]
    fn first_body_line_minimum_retains_object_margin_and_native_epsilon() {
        let mut content = code();
        let mut title = content.body.clone().unwrap();
        title.text = "language".into();
        content.title = Some(title);
        let body = content.body.as_mut().unwrap();
        body.text = "\u{fffc}".into();
        body.object_spans.push(crate::RichTextObjectSpan {
            object_type: crate::ObjectType::Image,
            object_data: Vec::new(),
            content: Some(crate::RichTextObjectContent::Image(Box::new(
                crate::PlacedImage {
                    bbox: BoundingBox {
                        x_min: 0.0,
                        y_min: 0.0,
                        x_max: 50.0,
                        y_max: 100.0,
                    },
                    rotation_degrees: None,
                    media_id: None,
                    media_index: None,
                    crop_rect: None,
                    original_bbox: None,
                    border_media_id: None,
                    original_media_id: None,
                },
            ))),
            text_index_utf16: 0,
            layout_option: crate::ObjectSpanLayoutOption::BlockWithSmallMargin,
            layout_constraint: ObjectSpanLayoutConstraint::Normal,
        });
        let prepared = prepare(&content, 200.0);
        let line = &prepared.body_layout.as_ref().unwrap().lines[0];
        assert!((line.bottom - line.top - 130.001).abs() < 1e-10);
        assert!((prepared.min_first_page_height - 250.751).abs() < 1e-10);
    }

    #[test]
    fn absent_title_has_no_measured_title_height_in_the_sdk_minimum() {
        let prepared = prepare(&code(), 200.0);
        assert!(prepared.title_layout.is_none());
        assert_eq!(prepared.min_first_page_height, 120.75);
    }

    #[test]
    fn measured_body_and_native_chrome_determine_panel_height() {
        let mut code = code();
        for saved_height in [50.0, 300.0] {
            code.bbox.y_max = code.bbox.y_min + saved_height;
            let prepared = prepare(&code, 200.0);
            assert_eq!(
                prepared.copy,
                BoundingBox {
                    x_min: 580.0,
                    y_min: 236.0,
                    x_max: 652.0,
                    y_max: 308.0,
                }
            );
            assert_eq!(prepared.panel_bbox.y_max, 513.5);
            let layout = prepared.body_layout.unwrap();
            assert!((layout.height() - 121.5).abs() < 1e-10);
            assert_eq!(
                layout
                    .lines
                    .iter()
                    .map(|line| line.baseline)
                    .collect::<Vec<_>>(),
                [377.0, 437.75]
            );
        }
    }

    #[test]
    fn translation_changes_positions_without_changing_measured_height() {
        let prepared = prepare(&code(), 273.25);
        assert_eq!(prepared.panel_bbox.y_min, 273.25);
        assert_eq!(prepared.panel_bbox.y_max, 586.75);
        let layout = prepared.body_layout.unwrap();
        assert!((layout.height() - 121.5).abs() < 1e-10);
        assert_eq!(
            layout
                .lines
                .iter()
                .map(|line| line.baseline)
                .collect::<Vec<_>>(),
            [450.25, 511.0]
        );
    }

    #[test]
    fn absent_body_retains_the_native_empty_panel_chrome() {
        let mut code = code();
        code.body = None;
        let prepared = prepare(&code, 200.0);
        assert!(prepared.body_layout.is_none());
        assert_eq!(prepared.panel_bbox.y_min, 200.0);
        assert_eq!(prepared.panel_bbox.y_max, 392.0);
    }

    #[test]
    fn retained_title_uses_the_header_frame_and_preserves_unicode_source_ranges() {
        let mut content = code();
        let mut title = code().body.unwrap();
        title.text = "Aé".into();
        title.font_size = Some(10.0);
        title.gravity = Some(1);
        title.bbox.y_min = -999.0;
        title.bbox.y_max = -998.0;
        content.title = Some(title);
        let prepared = prepare(&content, 200.0);
        assert_eq!(prepared.panel_bbox.y_max, 513.5);
        drop(content);
        let layout = prepared.title_layout.unwrap();
        assert_eq!(layout.lines.len(), 1);
        assert_eq!(layout.lines[0].line.source, 0..2);
        assert_eq!(layout.lines[0].x, 148.0);
        assert_eq!(layout.lines[0].baseline, 281.75);
        assert!((layout.height() - 40.5).abs() < 1e-10);
    }

    fn prepare_at_page_boundary(code: &RichTextCodeBlock, candidate_top: f64) -> PreparedCode {
        let document = Document {
            pages: (0..2)
                .map(|index| Page {
                    uuid: format!("page-{index}"),
                    width: 1080,
                    height: 500,
                    content_bbox: BoundingBox::default(),
                    background_color: None,
                    template: None,
                    background: Default::default(),
                    objects: Vec::new(),
                })
                .collect(),
            metadata: DocumentMetadata {
                page_mode: Some(0),
                default_page_dimensions: Some((1080, 500)),
                ..Default::default()
            },
        };
        let settings = TextSettings::from_document(&document.metadata);
        let fonts = FontBook::default();
        let renderer = TextRenderer::new(settings, &fonts)
            .with_page_exclusions(PageExclusions::for_page(&document, 0, settings));
        prepare_code(
            code,
            ObjectSpanLayoutConstraint::OverPages,
            candidate_top,
            RenderTheme::for_canvas(false),
            &renderer,
        )
        .unwrap()
    }

    #[test]
    fn candidate_top_controls_exclusions_independently_of_saved_origin() {
        let mut code = code();
        for saved_top in [0.0, 200.0, 900.0] {
            code.bbox.y_min = saved_top;
            code.bbox.y_max = saved_top + 50.0;
            let prepared = prepare_at_page_boundary(&code, 250.0);
            assert_eq!(prepared.panel_bbox.y_min, 250.0);
            assert_eq!(prepared.panel_bbox.y_max, 650.75);
            let layout = prepared.body_layout.unwrap();
            assert!((layout.height() - 208.75).abs() < 1e-10);
            assert_eq!(
                layout
                    .lines
                    .iter()
                    .map(|line| line.baseline - 250.0)
                    .collect::<Vec<_>>(),
                [177.0, 325.0]
            );
        }
    }

    #[test]
    fn moving_candidate_across_the_page_band_changes_the_measured_skip() {
        let code = code();
        for (candidate, height, baselines) in [
            (200.0, 121.5, [377.0, 437.75]),
            (250.0, 208.75, [427.0, 575.0]),
        ] {
            let prepared = prepare_at_page_boundary(&code, candidate);
            let layout = prepared.body_layout.unwrap();
            assert!((layout.height() - height).abs() < 1e-10);
            assert_eq!(
                layout
                    .lines
                    .iter()
                    .map(|line| line.baseline)
                    .collect::<Vec<_>>(),
                baselines
            );
        }
    }

    #[test]
    fn valid_negative_candidate_positions_remain_renderable() {
        let prepared = prepare(&code(), -200.0);
        assert_eq!(prepared.panel_bbox.y_min, -200.0);
        assert!((prepared.panel_bbox.y_max - 113.5).abs() < 1e-10);
    }

    #[test]
    fn individually_valid_font_sizes_cannot_overflow_the_derived_panel() {
        let mut code = code();
        code.body.as_mut().unwrap().font_size = Some(f32::MAX);
        let fonts = FontBook::default();
        let renderer = TextRenderer::new(
            TextSettings {
                scale: 1.0,
                font_size_delta: 0.0,
                ..Default::default()
            },
            &fonts,
        );
        assert!(matches!(
            prepare_code(
                &code,
                ObjectSpanLayoutConstraint::OverPages,
                200.0,
                RenderTheme::for_canvas(false),
                &renderer
            ),
            Err(ObjectDiagnosticKind::InvalidBounds)
        ));
    }

    #[test]
    fn signed_margins_cannot_produce_a_negative_panel_height() {
        let mut code = code();
        code.body.as_mut().unwrap().margins = Some([0.0, -400.0, 0.0, 0.0]);
        let fonts = FontBook::default();
        let renderer = TextRenderer::new(
            TextSettings {
                scale: 3.0,
                font_size_delta: 0.0,
                ..Default::default()
            },
            &fonts,
        );
        assert!(matches!(
            prepare_code(
                &code,
                ObjectSpanLayoutConstraint::OverPages,
                200.0,
                RenderTheme::for_canvas(false),
                &renderer
            ),
            Err(ObjectDiagnosticKind::InvalidBounds)
        ));
    }

    #[test]
    fn title_overflow_is_rejected_even_when_the_body_keeps_panel_bounds_valid() {
        let mut content = code();
        let fonts = FontBook::default();
        let renderer = TextRenderer::new(
            TextSettings {
                scale: 1.0,
                font_size_delta: 0.0,
                ..Default::default()
            },
            &fonts,
        );
        let prepare = |content: &RichTextCodeBlock| {
            prepare_code(
                content,
                ObjectSpanLayoutConstraint::OverPages,
                200.0,
                RenderTheme::for_canvas(false),
                &renderer,
            )
        };
        assert_eq!(prepare(&content).unwrap().panel_bbox.y_max, 304.5);
        let mut title = code().body.unwrap();
        title.font_size = Some(f32::MAX);
        content.title = Some(title);
        assert!(matches!(
            prepare(&content),
            Err(ObjectDiagnosticKind::InvalidBounds)
        ));
    }

    #[test]
    fn zero_width_text_frames_preserve_source_in_a_narrow_valid_panel() {
        let mut content = code();
        content.bbox.x_max = 110.0;
        let mut title = code().body.unwrap();
        title.text = "A".into();
        content.title = Some(title);
        let prepared = prepare(&content, 200.0);
        let layout = prepared.title_layout.unwrap();
        assert_eq!(layout.lines.len(), 1);
        assert_eq!(layout.lines[0].width, 0.0);
        assert_eq!(layout.lines[0].line.source, 0..1);
    }
}
