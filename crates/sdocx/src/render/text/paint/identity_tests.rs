use std::{ops::Range, sync::Arc};

use super::*;
use crate::fonts::FontBook;
use crate::render::text::{ObjectMeasurementContext, TextContext, TextSettings, wrap_paragraph};
use crate::render::vector::Svg;
use crate::{BoundingBox, RichTextBox, RichTextSpan, RichTextSpanType, SpanIntervalType};

#[derive(Debug, PartialEq)]
struct RetainedGlyph {
    id: u32,
    origin: [f64; 2],
    advance: [f64; 2],
    source: Range<usize>,
}

struct RetainedLine {
    source: String,
    runs: Vec<Vec<RetainedGlyph>>,
    paints: Vec<super::super::native::NativeTextPaint>,
    measured_groups: usize,
}

impl RetainedLine {
    fn glyphs(&self) -> Vec<&RetainedGlyph> {
        self.runs.iter().flatten().collect()
    }

    fn assert_geometry(&self, other: &Self) {
        assert_eq!(self.source, other.source);
        assert_eq!(self.glyphs(), other.glyphs());
    }
}

fn text_box(source: &str) -> RichTextBox {
    RichTextBox {
        text_area_type: None,
        bbox: BoundingBox::default(),
        rotation_degrees: None,
        text: source.into(),
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

fn span(kind: RichTextSpanType, range: Range<u32>, payload: Vec<u8>) -> RichTextSpan {
    RichTextSpan {
        kind,
        start_utf16: range.start,
        end_utf16: range.end,
        interval_type: SpanIntervalType::ClosedOpen,
        payload,
    }
}

fn observe(content: &RichTextBox) -> RetainedLine {
    let fonts = FontBook::default();
    let settings = TextSettings::default();
    let renderer = TextRenderer::new(settings, &fonts);
    let styled = StyledText::new(content, TextContext::Placed, settings);
    let theme = RenderTheme::for_canvas(false);
    let lines = wrap_paragraph(
        &styled,
        0..styled.index.len(),
        1000.0,
        theme,
        None,
        &renderer,
        ObjectMeasurementContext::Frame,
    )
    .unwrap();
    assert_eq!(lines.len(), 1);
    let line = &lines[0];
    let measured_groups = usize::from(!line.placements.is_empty())
        + line
            .placements
            .windows(2)
            .filter(|pair| !Arc::ptr_eq(&pair[0].cluster.run, &pair[1].cluster.run))
            .count();
    let mut scene = Scene::new(Svg::new());
    scene.retain_text();
    render_retained_line(
        &mut scene,
        &styled,
        line,
        [20.0, 1000.0, 100.0],
        None,
        theme,
        None,
        &renderer,
    )
    .unwrap();
    let blocks: Vec<_> = scene.native_text().iter().collect();
    assert_eq!(
        blocks.len(),
        1,
        "the line keeps one selectable source block"
    );
    let block = blocks[0].1;
    RetainedLine {
        source: block.source.to_string(),
        runs: block
            .runs
            .iter()
            .map(|run| {
                run.glyphs
                    .iter()
                    .map(|glyph| RetainedGlyph {
                        id: glyph.glyph_id,
                        origin: glyph.origin,
                        advance: glyph.advance,
                        source: glyph.source.clone(),
                    })
                    .collect()
            })
            .collect(),
        paints: block.runs.iter().map(|run| run.paint).collect(),
        measured_groups,
    }
}

#[test]
fn native_draw_fields_split_retained_runs_without_reshaping_equal_paint() {
    let baseline = observe(&text_box("AV"));
    assert_eq!(baseline.measured_groups, 1);
    assert_eq!(baseline.runs.len(), 1);
    for (name, attribute) in [
        (
            "ordinary background",
            span(
                RichTextSpanType::BackgroundColor,
                1..2,
                0x0011_2233_u32.to_le_bytes().to_vec(),
            ),
        ),
        (
            "composing background",
            span(
                RichTextSpanType::ComposingBackgroundColor,
                1..2,
                [0x0011_2233_u32.to_le_bytes(), [0; 4]].concat(),
            ),
        ),
        (
            "underline",
            span(RichTextSpanType::Underline, 1..2, vec![1, 0]),
        ),
        (
            "strike",
            span(RichTextSpanType::Strikethrough, 1..2, vec![1, 0]),
        ),
        (
            "suggestion underline",
            span(
                RichTextSpanType::Suggestion,
                1..2,
                [
                    0_u32.to_le_bytes(),
                    0xff11_2233_u32.to_le_bytes(),
                    0_u32.to_le_bytes(),
                ]
                .concat(),
            ),
        ),
    ] {
        let mut content = text_box("AV");
        content.spans.push(attribute);
        let retained = observe(&content);
        assert_eq!(retained.measured_groups, 1, "{name}: shared shaping");
        assert_eq!(retained.runs.len(), 2, "{name}: native draw boundary");
        assert_eq!(
            retained.paints,
            vec![baseline.paints[0]; 2],
            "{name}: equal paint"
        );
        retained.assert_geometry(&baseline);
    }
}

#[test]
fn hyperlink_flags_split_equal_blue_retained_paint_without_reshaping() {
    let mut content = text_box("AV");
    content.color = Some(crate::Color {
        r: 0,
        g: 84,
        b: 255,
    });
    content.underline = true;
    let baseline = observe(&content);
    content.spans.push(span(
        RichTextSpanType::Hyperlink,
        1..2,
        [9_u32.to_le_bytes(), [0; 4], [0; 4]].concat(),
    ));
    let retained = observe(&content);
    assert_eq!(retained.measured_groups, 1);
    assert_eq!(retained.runs.len(), 2);
    assert_eq!(retained.paints, vec![baseline.paints[0]; 2]);
    retained.assert_geometry(&baseline);
}

#[test]
fn transparent_foreground_alpha_splits_shaping_with_equal_retained_rgb() {
    let mut content = text_box("AV");
    content.spans.push(span(
        RichTextSpanType::ForegroundColor,
        0..1,
        0x0026_2626_u32.to_le_bytes().to_vec(),
    ));
    let retained = observe(&content);
    assert_eq!(retained.measured_groups, 2);
    assert_eq!(retained.runs.len(), 2);
    assert_eq!(retained.paints[0], retained.paints[1]);
    let a = observe(&text_box("A"));
    let v = observe(&text_box("V"));
    let glyphs = retained.glyphs();
    assert_eq!(retained.source, "AV");
    assert_eq!(glyphs[0].id, a.glyphs()[0].id);
    assert_eq!(glyphs[1].id, v.glyphs()[0].id);
    assert_eq!(
        glyphs[1].origin[0],
        glyphs[0].origin[0] + a.glyphs()[0].advance[0]
    );
}

#[test]
fn equivalent_font_names_and_no_op_decorations_keep_one_retained_run() {
    let font = |padding: u8| {
        [
            vec![padding; 8],
            7_u16.to_le_bytes().to_vec(),
            b"Roboto\0".to_vec(),
        ]
        .concat()
    };
    let mut content = text_box("AV");
    content.spans.extend([
        span(RichTextSpanType::FontName, 0..1, font(0)),
        span(RichTextSpanType::FontName, 1..2, font(0xa5)),
        span(RichTextSpanType::Underline, 1..2, vec![0, 0]),
    ]);
    let retained = observe(&content);
    assert_eq!(retained.measured_groups, 1);
    assert_eq!(retained.runs.len(), 1);
    retained.assert_geometry(&observe(&text_box("AV")));
}

#[test]
fn unavailable_correction_identity_does_not_coalesce_adjacent_clusters() {
    let mut content = text_box("AVX");
    let baseline = observe(&content);
    content
        .spans
        .push(span(RichTextSpanType::SpellCorrection, 0..3, Vec::new()));
    let retained = observe(&content);
    assert_eq!(retained.measured_groups, 1);
    assert_eq!(retained.runs.len(), 3);
    assert!(retained.runs.iter().all(|run| run.len() == 1));
    retained.assert_geometry(&baseline);
}

#[test]
fn an_intra_cluster_identity_boundary_keeps_the_shaped_source_cluster_intact() {
    let mut content = text_box("e\u{301}");
    let baseline = observe(&content);
    content.spans.push(span(
        RichTextSpanType::BackgroundColor,
        1..2,
        0xff11_2233_u32.to_le_bytes().to_vec(),
    ));
    let retained = observe(&content);
    assert_eq!(retained.measured_groups, 1);
    assert_eq!(retained.runs.len(), 1);
    assert_eq!(retained.glyphs().len(), 1);
    assert_eq!(retained.glyphs()[0].source, 0.."e\u{301}".len());
    retained.assert_geometry(&baseline);
}
