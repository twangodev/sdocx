#![cfg(feature = "render")]

use sdocx::{
    BoundingBox, Color, Document, DocumentMetadata, Page, PageElement, RenderColorMode,
    RenderOptions, RichTextBox, RichTextSpan, RichTextSpanType, SpanIntervalType, render_page_svg,
};

fn text_box(spans: Vec<RichTextSpan>) -> RichTextBox {
    RichTextBox {
        text_area_type: None,
        bbox: BoundingBox {
            x_min: 0.0,
            y_min: 0.0,
            x_max: 100_000.0,
            y_max: 100.0,
        },
        rotation_degrees: None,
        text: "A".repeat(10_000),
        color: Some(Color {
            r: 38,
            g: 38,
            b: 38,
        }),
        highlight_color: None,
        underline: false,
        font_size: Some(10.0),
        runs: Vec::new(),
        spans,
        paragraphs: Vec::new(),
        object_spans: Vec::new(),
        text_sections: Vec::new(),
        margins: None,
        gravity: None,
    }
}

fn font_span(start: u32, end: u32) -> RichTextSpan {
    RichTextSpan {
        kind: RichTextSpanType::FontSize,
        start_utf16: start,
        end_utf16: end,
        interval_type: SpanIntervalType::ClosedOpen,
        payload: 10_f32.to_le_bytes().to_vec(),
    }
}

fn document(text: RichTextBox) -> Document {
    Document {
        metadata: DocumentMetadata::default(),
        pages: vec![Page {
            uuid: "style-boundaries".into(),
            width: 100_000,
            height: 100,
            content_bbox: text.bbox,
            background_color: None,
            template: None,
            background: Default::default(),
            objects: vec![PageElement::TextBox(text).into()],
        }],
    }
}

#[test]
fn ten_thousand_style_boundaries_preserve_rendered_output_in_both_themes() {
    let single = document(text_box(vec![font_span(0, 10_000)]));
    let separate = document(text_box((0..10_000).map(|i| font_span(i, i + 1)).collect()));
    for color_mode in [RenderColorMode::Light, RenderColorMode::Dark] {
        let mut options = RenderOptions::default();
        options.color_mode = color_mode;
        let expected = render_page_svg(&single, 0, &options).unwrap();
        let actual = render_page_svg(&separate, 0, &options).unwrap();
        let expected_tree = roxmltree::Document::parse(&expected.svg).unwrap();
        let actual_tree = roxmltree::Document::parse(&actual.svg).unwrap();
        let glyph_paint = |tree: &roxmltree::Document<'_>| {
            tree.descendants()
                .filter(|node| node.has_tag_name("tspan"))
                .flat_map(|node| {
                    let characters = node.text().unwrap().chars();
                    let positions = node.attribute("x").unwrap().split_whitespace();
                    characters.zip(positions).map(move |(character, x)| {
                        (
                            character,
                            x.to_owned(),
                            node.attribute("y").unwrap().to_owned(),
                            node.attribute("font-size").unwrap().to_owned(),
                            node.attribute("fill").unwrap().to_owned(),
                        )
                    })
                })
                .collect::<Vec<_>>()
        };
        let expected_glyphs = glyph_paint(&expected_tree);
        let actual_glyphs = glyph_paint(&actual_tree);
        assert_eq!(actual_glyphs.len(), 10_000);
        assert_eq!(actual_glyphs, expected_glyphs);
        assert_eq!(actual.text_diagnostics, expected.text_diagnostics);
        assert_eq!(actual.object_diagnostics, expected.object_diagnostics);
        assert!(actual.text_diagnostics.is_empty());
        let tree = roxmltree::Document::parse(&actual.svg).unwrap();
        let source = tree
            .descendants()
            .filter(|node| node.has_tag_name("tspan"))
            .filter_map(|node| node.text())
            .collect::<String>();
        assert_eq!(source, "A".repeat(10_000));
    }
}
