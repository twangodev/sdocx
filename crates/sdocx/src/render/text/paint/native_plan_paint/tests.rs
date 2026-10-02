use super::*;
use crate::fonts::FontBook;
use crate::render::text::{
    NativeCellTextConstraints, TextContext, TextFrame, TextSettings, layout_table_cell_text,
};
use crate::render::vector::Svg;
use crate::{BoundingBox, RichTextBox, RichTextParagraph, RichTextParagraphType};
use sha2::{Digest, Sha256};

fn capture() -> serde_json::Value {
    let bytes = include_bytes!("../../../../../../../conformance/table-text-cell-emission.json");
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "0098cfcd93274b210892fd0653677fd4cef3a35ebc4f0419b4b661857cbfa4ce"
    );
    serde_json::from_slice(bytes).unwrap()
}

fn content(case: &serde_json::Value) -> RichTextBox {
    RichTextBox {
        text_area_type: None,
        bbox: BoundingBox::default(),
        rotation_degrees: None,
        text: case["text_utf8"].as_str().unwrap().into(),
        color: Some(Color {
            r: 37,
            g: 37,
            b: 37,
        }),
        highlight_color: None,
        underline: false,
        font_size: Some(
            case["requested_font_size_bits"]
                .as_u64()
                .map_or(50.0, |bits| f32::from_bits(bits as u32)),
        ),
        runs: Vec::new(),
        spans: Vec::new(),
        paragraphs: vec![RichTextParagraph {
            kind: RichTextParagraphType::Alignment,
            start_paragraph: 0,
            end_paragraph: 1,
            payload: 2_u32.to_le_bytes().to_vec(),
        }],
        object_spans: Vec::new(),
        text_sections: Vec::new(),
        margins: case["supplied_margins_bits"].as_array().map(|values| {
            std::array::from_fn(|index| f32::from_bits(values[index].as_u64().unwrap() as u32))
        }),
        gravity: Some(0),
    }
}

fn layout(
    styled: &StyledText<'_>,
    case: &serde_json::Value,
    origin: [f64; 2],
    renderer: &TextRenderer<'_>,
) -> TextLayout {
    let width = case["measure_widths"][0].as_i64().unwrap() as i32;
    let mut layout = layout_table_cell_text(
        styled,
        TextFrame {
            bbox: BoundingBox {
                x_min: 0.0,
                y_min: 0.0,
                x_max: f64::from(width),
                y_max: 1000.0,
            },
            gravity: Some(0),
            exclusions: &[],
        },
        NativeCellTextConstraints {
            width,
            height_limit: 1000.0,
        },
        RenderTheme::for_canvas(false),
        renderer,
    )
    .unwrap();
    layout.translate(origin[0], origin[1]);
    layout
}

fn captured_float(value: &serde_json::Value) -> f64 {
    f64::from(f32::from_bits(value.as_u64().unwrap() as u32))
}

#[test]
fn full_source_svg_groups_follow_captured_cached_origins_and_combining_owners() {
    let capture = capture();
    let fonts = FontBook::default();
    let settings = TextSettings::resolved();
    let theme = RenderTheme::for_canvas(false);
    for name in [
        "default-auto",
        "combining-auto",
        "multi-paragraph-auto",
        "leading-newline",
    ] {
        let case = capture["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|case| case["name"] == name)
            .unwrap();
        let content = content(case);
        let styled = StyledText::new(&content, TextContext::Flow, settings);
        let renderer = TextRenderer::new(settings, &fonts);
        let translation = [11.125, 23.375];
        let layout = layout(&styled, case, translation, &renderer);
        let mut dispatcher = NativePaintDispatcher::new(
            &styled,
            &layout,
            theme,
            &renderer,
            &vec![true; layout.lines.len()],
            false,
            None,
        )
        .unwrap_or_else(|| panic!("capture {name} must reach the certified SVG writer"));
        let expected = case["emitted_runs"]["runs"].as_array().unwrap();
        assert_eq!(dispatcher.groups.len(), expected.len(), "{name}");
        let mut scene = Scene::new(Svg::new());
        scene.text_source(|scene| {
            for (index, line) in layout.lines.iter().enumerate() {
                dispatcher.paint_line(scene, &styled, index, line, theme, &renderer);
            }
        });
        let svg = scene.finish();
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let groups = xml
            .descendants()
            .filter(|node| node.has_tag_name("text"))
            .collect::<Vec<_>>();
        assert_eq!(groups.len(), expected.len(), "{name}");
        for (group, captured) in groups.iter().zip(expected) {
            for (axis, shift) in ["x", "y"].into_iter().zip(translation) {
                let index = usize::from(axis == "y");
                let actual: f64 = group.attribute(axis).unwrap().parse().unwrap();
                let expected = captured_float(&captured["origin_bits"][index]) + shift;
                assert_eq!(actual, expected, "{name} {axis}");
            }
            let spans = group
                .descendants()
                .filter(|node| node.has_tag_name("tspan"))
                .collect::<Vec<_>>();
            let positions = spans
                .iter()
                .flat_map(|span| span.attribute("x").unwrap().split_whitespace())
                .map(|position| position.parse::<f64>().unwrap())
                .collect::<Vec<_>>();
            let captured_positions = captured["position_bits"].as_array().unwrap();
            assert_eq!(positions.len(), captured_positions.len(), "{name}");
            for (actual, expected) in positions.into_iter().zip(captured_positions) {
                let expected = captured_float(expected) + translation[0];
                assert_eq!(actual, expected, "{name} glyph x");
            }
            for span in spans {
                let baseline: f64 = span.attribute("y").unwrap().parse().unwrap();
                let expected = captured_float(&captured["origin_bits"][1]) + translation[1];
                assert_eq!(baseline, expected, "{name} glyph y");
                assert_eq!(span.attribute("fill"), Some("#252525"));
            }
        }
        let source = groups
            .iter()
            .flat_map(|group| {
                group
                    .descendants()
                    .filter(|node| node.has_tag_name("tspan"))
                    .filter_map(|node| node.text())
            })
            .collect::<String>();
        assert_eq!(source, content.text.replace('\n', ""), "{name}");
        assert!(
            renderer.diagnostics().is_empty(),
            "{name}: {:?}",
            renderer.diagnostics()
        );
    }
}

#[cfg(feature = "pdf")]
#[test]
fn visible_source_projection_retains_cached_pdf_geometry_without_hidden_actual_text() {
    let capture = capture();
    let case = capture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == "default-narrow")
        .unwrap();
    let content = content(case);
    let fonts = FontBook::default();
    let settings = TextSettings::resolved();
    let renderer = TextRenderer::new(settings, &fonts);
    let styled = StyledText::new(&content, TextContext::Flow, settings);
    let translation = [11.125, 23.375];
    let theme = RenderTheme::for_canvas(false);
    let layout = layout(&styled, case, translation, &renderer);
    let mut visible = vec![false; layout.lines.len()];
    visible[1] = true;
    let mut dispatcher =
        NativePaintDispatcher::new(&styled, &layout, theme, &renderer, &visible, true, None)
            .unwrap();
    assert_eq!(dispatcher.groups.len(), 1);
    let mut scene = Scene::new(Svg::new());
    scene.retain_text();
    scene.text_source(|scene| {
        dispatcher.paint_line(scene, &styled, 1, &layout.lines[1], theme, &renderer)
    });
    let registry = scene.take_native_text();
    let blocks = registry.iter().map(|(_, block)| block).collect::<Vec<_>>();
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].source.as_ref(), "V");
    assert_eq!(blocks[0].runs.len(), 1);
    assert_eq!(blocks[0].runs[0].glyphs.len(), 1);
    let glyph = &blocks[0].runs[0].glyphs[0];
    let expected = &case["emitted_runs"]["runs"][1];
    assert_eq!(
        glyph.glyph_id as u64,
        expected["codewords"][0].as_u64().unwrap()
    );
    assert_eq!(glyph.source.characters(), &(0..1));
    assert_eq!(glyph.source.utf16(), &(0..1));
    assert_eq!(
        glyph.origin,
        [
            captured_float(&expected["position_bits"][0]) + translation[0],
            captured_float(&expected["origin_bits"][1]) + translation[1]
        ]
    );
    assert!(scene.take_native_text_error().is_none());
    assert!(renderer.diagnostics().is_empty());
}

#[cfg(feature = "pdf")]
#[test]
fn svg_rejects_internal_mark_displacement_while_pdf_keeps_the_cached_group() {
    let capture = capture();
    let case = capture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == "combining-auto")
        .unwrap();
    let mut content = content(case);
    content.text = "x\u{327}\u{301}".into();
    let fonts = FontBook::default();
    let settings = TextSettings::resolved();
    let renderer = TextRenderer::new(settings, &fonts);
    let styled = StyledText::new(&content, TextContext::Flow, settings);
    let theme = RenderTheme::for_canvas(false);
    let layout = layout(&styled, case, [0.0; 2], &renderer);
    let plan = native_paint_plan(&styled, &layout, theme).unwrap();
    assert_eq!(plan.runs.len(), 1);
    assert!(plan.runs[0].glyphs.len() > 1);
    let visible = vec![true; layout.lines.len()];
    assert!(
        NativePaintDispatcher::new(&styled, &layout, theme, &renderer, &visible, false, None,)
            .is_none()
    );
    assert_eq!(
        renderer.diagnostics(),
        [super::super::super::TextDiagnostic {
            kind: super::super::super::TextDiagnosticKind::UnsupportedGlyphPositioning,
            family: "native".into(),
            codepoints: vec![120, 769, 807],
        }]
    );
    let renderer = TextRenderer::new(settings, &fonts);
    let mut dispatcher =
        NativePaintDispatcher::new(&styled, &layout, theme, &renderer, &visible, true, None)
            .unwrap();
    let mut scene = Scene::new(Svg::new());
    scene.retain_text();
    scene.text_source(|scene| {
        dispatcher.paint_line(scene, &styled, 0, &layout.lines[0], theme, &renderer)
    });
    let registry = scene.take_native_text();
    let block = registry.iter().next().unwrap().1;
    assert_eq!(block.source.as_ref(), content.text);
    assert_eq!(block.runs[0].glyphs.len(), plan.runs[0].glyphs.len());
    for (actual, expected) in block.runs[0].glyphs.iter().zip(&plan.runs[0].glyphs) {
        assert_eq!(actual.glyph_id, expected.glyph_id);
        assert_eq!(actual.origin, expected.origin.map(f64::from));
    }
    assert!(renderer.diagnostics().is_empty());
}
