use crate::{BoundingBox, ObjectSpanLayoutConstraint, RichTextCodeBlock};

use super::RenderTheme;
use super::text::{StyledText, TextBaseline, TextContext, TextFrame, TextLayout, TextRenderer};

pub(super) struct PreparedCode {
    pub copy: BoundingBox,
    pub title_bbox: BoundingBox,
    pub panel_bbox: BoundingBox,
    pub body_layout: Option<TextLayout>,
}

pub(super) fn prepare_code(
    code: &RichTextCodeBlock,
    constraint: ObjectSpanLayoutConstraint,
    offset_y: f64,
    theme: RenderTheme,
    renderer: &TextRenderer<'_>,
) -> PreparedCode {
    let settings = renderer.settings;
    let background = super::argb_color(if theme.is_dark() { 0x333333 } else { 0xefefef });
    let theme = theme.on_background(background);
    let object_top = code.bbox.y_min + offset_y;
    let left = settings.pixels(16.0);
    let top = settings.pixels(12.0);
    let right = settings.pixels(16.0);
    let bottom = settings.pixels(12.0);
    let title_copy_gap = settings.pixels(12.0);
    let body_gap = settings.pixels(8.0);
    let copy_size = settings.pixels(24.0);
    let copy = BoundingBox {
        x_min: code.bbox.x_max - right - copy_size,
        y_min: object_top + top,
        x_max: code.bbox.x_max - right,
        y_max: object_top + top + copy_size,
    };
    let title_bbox = BoundingBox {
        x_min: code.bbox.x_min + left,
        y_min: copy.y_min,
        x_max: copy.x_min - title_copy_gap,
        y_max: copy.y_max,
    };
    let body_top = copy.y_max + body_gap;
    let body_bbox = BoundingBox {
        x_min: code.bbox.x_min + left,
        y_min: body_top,
        x_max: code.bbox.x_max - right,
        y_max: body_top,
    };
    let exclusions = renderer.object_exclusions(constraint, code.bbox.y_min, offset_y);
    let body_layout = code.body.as_ref().map(|body| {
        let styled = StyledText::new(body, TextContext::Flow, settings);
        super::text::layout_text(
            &styled,
            TextFrame {
                bbox: body_bbox,
                gravity: body.gravity,
                baseline: TextBaseline::LineAdvance,
                exclusions: &exclusions,
            },
            theme,
            renderer,
        )
    });
    let body_height = body_layout.as_ref().map_or(0.0, TextLayout::height);
    let panel_bbox = BoundingBox {
        y_min: object_top,
        y_max: body_top + body_height + body_gap + bottom,
        ..code.bbox
    };
    PreparedCode {
        copy,
        title_bbox,
        panel_bbox,
        body_layout,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RichTextBox;
    use crate::fonts::FontBook;
    use crate::render::text::TextSettings;

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

    fn prepare(code: &RichTextCodeBlock, offset_y: f64) -> PreparedCode {
        let fonts = FontBook::default();
        let renderer = TextRenderer::new(
            TextSettings {
                scale: 3.0,
                font_size_delta: 0.0,
            },
            &fonts,
        );
        prepare_code(
            code,
            ObjectSpanLayoutConstraint::OverPages,
            offset_y,
            RenderTheme::for_canvas(false),
            &renderer,
        )
    }

    #[test]
    fn measured_body_and_native_chrome_determine_panel_height() {
        let mut code = code();
        for saved_height in [50.0, 300.0] {
            code.bbox.y_max = code.bbox.y_min + saved_height;
            let prepared = prepare(&code, 0.0);
            assert_eq!(
                prepared.copy,
                BoundingBox {
                    x_min: 580.0,
                    y_min: 236.0,
                    x_max: 652.0,
                    y_max: 308.0,
                }
            );
            assert_eq!(
                prepared.title_bbox,
                BoundingBox {
                    x_min: 148.0,
                    y_min: 236.0,
                    x_max: 544.0,
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
        let prepared = prepare(&code(), 73.25);
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
        let prepared = prepare(&code, 0.0);
        assert!(prepared.body_layout.is_none());
        assert_eq!(prepared.panel_bbox.y_min, 200.0);
        assert_eq!(prepared.panel_bbox.y_max, 392.0);
    }
}
