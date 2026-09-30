use crate::BoundingBox;

use super::RenderTheme;
use super::fonts::ResolvedFace;
use super::text::{StyledText, TextLine, WrappedLine, body_text_ranges};

#[derive(Clone, Copy)]
pub(super) struct Viewport {
    bounds: BoundingBox,
}

impl Viewport {
    pub fn new(bounds: BoundingBox) -> Self {
        Self { bounds }
    }

    pub fn translated(self, dx: f64, dy: f64) -> Self {
        Self::new(BoundingBox {
            x_min: self.bounds.x_min + dx,
            y_min: self.bounds.y_min + dy,
            x_max: self.bounds.x_max + dx,
            y_max: self.bounds.y_max + dy,
        })
    }

    pub fn intersects(self, bounds: BoundingBox) -> bool {
        valid_bounds(self.bounds)
            && valid_bounds(bounds)
            && bounds.x_min < self.bounds.x_max
            && bounds.x_max > self.bounds.x_min
            && self.overlaps_height(bounds.y_min, bounds.y_max)
    }

    pub fn line_visible(self, line: &TextLine) -> bool {
        self.overlaps_height(line.top, line.bottom)
    }

    pub fn background_visible(self, line: &TextLine) -> bool {
        self.overlaps_height(line.background_top, line.bottom)
    }

    pub fn text_visible(
        self,
        styled: &StyledText<'_>,
        line: &TextLine,
        theme: RenderTheme,
    ) -> bool {
        self.line_visible(line)
            && (self.body_visible(&line.line, line.baseline)
                || self.decorations_visible(styled, line, theme))
    }

    pub fn body_visible(self, line: &WrappedLine, baseline: f64) -> bool {
        if !baseline.is_finite() || line.source.is_empty() || !valid_bounds(self.bounds) {
            return false;
        }
        let rejected = line
            .objects
            .iter()
            .any(|object| matches!(object.prepared, Some(Err(_))));
        if rejected && (line.font_size <= 0.0 || self.unmeasured_visible(line.font_size, baseline))
        {
            return true;
        }
        if line.placements.is_empty() {
            return line.objects.is_empty() && self.unmeasured_visible(line.font_size, baseline);
        }
        let mut has_ink = false;
        for placement in &line.placements {
            let cluster = &placement.cluster;
            let run = &cluster.run;
            let size = run.style.font_size;
            let scale = size / f64::from(run.face.metrics.units_per_em);
            if !scale.is_finite() || scale <= 0.0 {
                return true;
            }
            if !cluster.supports_positioned_text()
                && self.metrics_visible(&run.face, size, baseline)
            {
                return true;
            }
            let Some(glyphs) = run.glyphs.get(cluster.glyphs.clone()) else {
                return true;
            };
            for glyph in glyphs {
                match run.face.glyph_ink_bounds(glyph.raw.id) {
                    Ok(Some(ink)) => {
                        has_ink = true;
                        let pen = glyph.pen_y as f64 + f64::from(glyph.raw.y_offset);
                        let stroke = if run.style.bold { 0.225 } else { 0.0 };
                        let top = baseline - (pen + f64::from(ink.y_max)) * scale - stroke;
                        let bottom = baseline - (pen + f64::from(ink.y_min)) * scale + stroke;
                        if !top.is_finite() || !bottom.is_finite() {
                            return true;
                        }
                        if self.overlaps_height(top, bottom) {
                            return true;
                        }
                    }
                    Ok(None) => {}
                    Err(_) => {
                        if self.metrics_visible(&run.face, size, baseline) {
                            return true;
                        }
                    }
                }
            }
        }
        !has_ink
            && line.placements.iter().any(|placement| {
                let run = &placement.cluster.run;
                self.metrics_visible(&run.face, run.style.font_size, baseline)
            })
    }

    fn decorations_visible(
        self,
        styled: &StyledText<'_>,
        line: &TextLine,
        theme: RenderTheme,
    ) -> bool {
        body_text_ranges(&line.line)
            .into_iter()
            .flat_map(|range| styled.segments(range))
            .any(|segment| {
                let style = styled.style_at(segment.start, theme, line.predefined);
                let thickness = style.font_size * f64::from(1.0_f32 / 18.0);
                [
                    style.underline.then_some(f64::from(1.0_f32 / 9.0)),
                    style.strikethrough.then_some(f64::from(-2.0_f32 / 7.0)),
                ]
                .into_iter()
                .flatten()
                .any(|offset| {
                    let top = line.baseline + style.font_size * offset;
                    self.overlaps_height(top, top + thickness)
                })
            })
    }

    fn overlaps_height(self, top: f64, bottom: f64) -> bool {
        valid_bounds(self.bounds)
            && top.is_finite()
            && bottom.is_finite()
            && top < bottom
            && top < self.bounds.y_max
            && bottom > self.bounds.y_min
    }

    fn metrics_visible(self, face: &ResolvedFace, size: f64, baseline: f64) -> bool {
        let scale = size / f64::from(face.metrics.units_per_em);
        let top = baseline - f64::from(face.metrics.ascent) * scale;
        let bottom = baseline - f64::from(face.metrics.descent) * scale;
        self.overlaps_height(top.min(bottom), top.max(bottom))
    }

    fn unmeasured_visible(self, size: f64, baseline: f64) -> bool {
        size <= 0.0 || self.overlaps_height(baseline - size, baseline + size)
    }
}

fn valid_bounds(bounds: BoundingBox) -> bool {
    [bounds.x_min, bounds.y_min, bounds.x_max, bounds.y_max]
        .into_iter()
        .all(f64::is_finite)
        && bounds.x_min < bounds.x_max
        && bounds.y_min < bounds.y_max
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::fonts::FontBook;
    use crate::render::text::{
        ObjectDiagnosticKind, StyledText, TextContext, TextFrame, TextRenderer, TextSettings,
        layout_text,
    };
    use crate::{
        DocumentMetadata, ObjectSpanLayoutConstraint, ObjectSpanLayoutOption, ObjectType,
        PlacedImage, RichTextBox, RichTextObjectContent, RichTextObjectSpan, RichTextSpan,
        RichTextSpanType,
    };
    use std::sync::Arc;

    fn bounds(left: f64, top: f64, right: f64, bottom: f64) -> BoundingBox {
        BoundingBox {
            x_min: left,
            y_min: top,
            x_max: right,
            y_max: bottom,
        }
    }

    fn viewport(top: f64, bottom: f64) -> Viewport {
        Viewport::new(bounds(0.0, top, 100.0, bottom))
    }

    fn content(value: &str) -> RichTextBox {
        RichTextBox {
            text_area_type: None,
            bbox: bounds(0.0, 0.0, 500.0, 600.0),
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

    fn settings() -> TextSettings {
        TextSettings::from_document(&DocumentMetadata {
            default_page_dimensions: Some((360, 800)),
            ..Default::default()
        })
    }

    fn measured(content: &RichTextBox) -> TextLine {
        let settings = settings();
        let fonts = FontBook::default();
        let renderer = TextRenderer::new(settings, &fonts);
        let styled = StyledText::new(content, TextContext::Placed, settings);
        layout_text(
            &styled,
            TextFrame {
                bbox: content.bbox,
                gravity: None,
                exclusions: &[],
            },
            RenderTheme::for_canvas(false),
            &renderer,
        )
        .lines
        .into_iter()
        .next()
        .unwrap()
    }

    fn image(anchor: i32) -> RichTextObjectSpan {
        RichTextObjectSpan {
            object_type: ObjectType::Image,
            object_data: Vec::new(),
            content: Some(RichTextObjectContent::Image(Box::new(PlacedImage {
                bbox: bounds(0.0, 0.0, 30.0, 400.0),
                rotation_degrees: None,
                media_id: None,
                media_index: None,
                crop_rect: None,
                original_bbox: None,
                border_media_id: None,
                original_media_id: None,
            }))),
            text_index_utf16: anchor,
            layout_option: ObjectSpanLayoutOption::Inline,
            layout_constraint: ObjectSpanLayoutConstraint::Normal,
        }
    }

    #[test]
    fn intersection_is_strict_in_both_axes_and_rejects_invalid_rectangles() {
        let visible = viewport(0.0, 100.0);
        assert!(visible.intersects(bounds(-10.0, -10.0, 1.0, 1.0)));
        for rectangle in [
            bounds(100.0, 0.0, 110.0, 10.0),
            bounds(-10.0, 0.0, 0.0, 10.0),
            bounds(0.0, 100.0, 10.0, 110.0),
            bounds(0.0, -10.0, 10.0, 0.0),
            bounds(0.0, 0.0, 0.0, 10.0),
            bounds(0.0, 0.0, 10.0, -10.0),
            bounds(f64::NAN, 0.0, 10.0, 10.0),
            bounds(0.0, 0.0, f64::INFINITY, 10.0),
        ] {
            assert!(!visible.intersects(rectangle));
        }
        assert!(!Viewport::new(BoundingBox::default()).intersects(bounds(0.0, 0.0, 1.0, 1.0)));
    }

    #[test]
    fn translating_the_viewport_changes_its_coordinate_space() {
        let local = viewport(0.0, 100.0).translated(-10.0, 20.0);
        assert!(local.intersects(bounds(-10.0, 20.0, 0.0, 30.0)));
        assert!(!local.intersects(bounds(90.0, 20.0, 100.0, 30.0)));
        assert!(!local.intersects(bounds(0.0, 0.0, 10.0, 20.0)));
    }

    #[test]
    fn native_line_band_is_independent_of_font_baseline_and_horizontal_width() {
        let mut line = measured(&content("A"));
        line.top = -20.0;
        line.bottom = 0.0;
        line.baseline = 40.0;
        assert!(!viewport(0.0, 100.0).line_visible(&line));
        line.bottom = 0.001;
        line.x = 1000.0;
        line.width = 0.0;
        assert!(viewport(0.0, 100.0).line_visible(&line));
        line.top = 100.0;
        line.bottom = 120.0;
        assert!(!viewport(0.0, 100.0).line_visible(&line));
    }

    #[test]
    fn actual_cap_ink_and_combining_accent_decide_upper_edge_visibility() {
        let plain = measured(&content("A"));
        let accented = measured(&content("A\u{301}"));
        assert!(!viewport(0.0, 8.0).body_visible(&plain.line, 40.0));
        assert!(viewport(0.0, 8.0).body_visible(&accented.line, 40.0));
        assert!(viewport(0.0, 8.01).body_visible(&plain.line, 40.0));
    }

    #[test]
    fn descender_ink_crosses_the_edge_while_a_cap_only_touches_it() {
        let cap = measured(&content("A"));
        let descender = measured(&content("g"));
        assert!(!viewport(0.0, 2.0).body_visible(&cap.line, 0.0));
        assert!(viewport(0.0, 2.0).body_visible(&descender.line, 0.0));
    }

    #[test]
    fn glyph_pen_and_y_offset_use_font_y_up_coordinates() {
        let mut line = measured(&content("A"));
        assert!(!viewport(0.0, 35.0).body_visible(&line.line, 100.0));
        let run = Arc::get_mut(&mut line.line.placements[0].cluster.run).unwrap();
        run.glyphs[0].pen_y = 1024;
        run.glyphs[0].raw.y_offset = 512;
        assert!(viewport(0.0, 35.0).body_visible(&line.line, 100.0));
        assert!(!viewport(66.25, 100.0).body_visible(&line.line, 100.0));
        assert!(viewport(66.24, 100.0).body_visible(&line.line, 100.0));
    }

    #[test]
    fn unsupported_and_missing_glyphs_keep_a_conservative_metric_band() {
        let mut line = measured(&content("A"));
        assert!(!viewport(0.0, 8.0).body_visible(&line.line, 40.0));
        let run = Arc::get_mut(&mut line.line.placements[0].cluster.run).unwrap();
        run.direction = crate::render::fonts::Direction::RightToLeft;
        assert!(viewport(0.0, 8.0).body_visible(&line.line, 40.0));
        let run = Arc::get_mut(&mut line.line.placements[0].cluster.run).unwrap();
        run.direction = crate::render::fonts::Direction::LeftToRight;
        run.glyphs[0].raw.id = 0;
        assert!(viewport(0.0, 8.0).body_visible(&line.line, 40.0));
        assert!(!viewport(0.0, 100.0).body_visible(&line.line, 400.0));
    }

    #[test]
    fn bold_stroke_expands_ink_visibility_at_the_edge() {
        let mut line = measured(&content("A"));
        assert!(!viewport(0.0, 8.0).body_visible(&line.line, 40.0));
        Arc::get_mut(&mut line.line.placements[0].cluster.run)
            .unwrap()
            .style
            .bold = true;
        assert!(viewport(0.0, 8.0).body_visible(&line.line, 40.0));
        assert!(!viewport(0.0, 8.0).body_visible(&line.line, f64::NAN));
    }

    #[test]
    fn inkless_spaces_retain_selection() {
        let mut line = measured(&content("  "));
        assert!(viewport(0.0, 100.0).body_visible(&line.line, 20.0));
        assert!(!viewport(0.0, 100.0).body_visible(&line.line, 400.0));
        line.line.placements.clear();
        assert!(viewport(0.0, 100.0).body_visible(&line.line, 20.0));
    }

    #[test]
    fn visible_decorations_use_actual_segments_inside_a_shared_shaping_run() {
        for (source, kind, top, baseline, viewport_bottom) in [
            ("AB", RichTextSpanType::Underline, -49.0, -4.0, 2.0),
            ("__", RichTextSpanType::Strikethrough, -28.0, 17.0, 8.0),
        ] {
            let mut content = content(source);
            content.bbox.y_min = top;
            content.spans.push(RichTextSpan {
                kind,
                start_utf16: 1,
                end_utf16: 2,
                expand: false,
                payload: vec![1, 0],
            });
            let line = measured(&content);
            assert!((line.baseline - baseline).abs() < 1e-9);
            assert!(Arc::ptr_eq(
                &line.line.placements[0].cluster.run,
                &line.line.placements[1].cluster.run
            ));
            let first_style = &line.line.placements[0].cluster.run.style;
            assert!(!first_style.underline && !first_style.strikethrough);
            let visible = viewport(0.0, viewport_bottom);
            assert!(visible.line_visible(&line));
            assert!(!visible.body_visible(&line.line, line.baseline));
            let styled = StyledText::new(&content, TextContext::Placed, settings());
            assert!(visible.text_visible(&styled, &line, RenderTheme::for_canvas(false)));
            content.spans.clear();
            let styled = StyledText::new(&content, TextContext::Placed, settings());
            assert!(!visible.text_visible(&styled, &line, RenderTheme::for_canvas(false)));
        }
    }

    #[test]
    fn tall_visible_object_does_not_admit_its_off_page_text() {
        let mut mixed = content("A\u{fffc}");
        mixed.object_spans.push(image(1));
        let line = measured(&mixed);
        assert!(viewport(0.0, 100.0).line_visible(&line));
        assert!(!viewport(0.0, 100.0).body_visible(&line.line, line.baseline));
        assert!(viewport(0.0, 100.0).body_visible(&line.line, 40.0));
    }

    #[test]
    fn object_only_body_is_absent_but_rejected_replacement_is_preserved() {
        let mut object = content("\u{fffc}");
        object.object_spans.push(image(0));
        let mut line = measured(&object);
        assert_eq!(line.line.font_size, 45.0);
        assert_eq!(line.line.text_height, 0.0);
        assert!(!viewport(0.0, 100.0).body_visible(&line.line, line.baseline));
        line.line.objects[0].prepared = Some(Err(ObjectDiagnosticKind::InvalidBounds));
        assert!(!viewport(0.0, 100.0).body_visible(&line.line, line.baseline));
        assert!(viewport(0.0, 100.0).body_visible(&line.line, 40.0));
    }
}
