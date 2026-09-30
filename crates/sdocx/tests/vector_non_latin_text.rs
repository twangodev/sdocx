#![cfg(feature = "render")]

use resvg::usvg;
use sdocx::fonts::FontBook;
use sdocx::{
    BoundingBox, Color, Document, DocumentMetadata, Page, PageElement, RenderedPage, RichTextBox,
    RichTextSpan, RichTextSpanType,
};

const SIZE: f64 = 20.0;
const UNITS: f64 = 2048.0;

#[derive(Clone, Copy, Debug)]
enum Context {
    Placed,
    Flow,
}

const CONTEXTS: [Context; 2] = [Context::Placed, Context::Flow];

fn text(source: &str) -> RichTextBox {
    RichTextBox {
        text_area_type: None,
        bbox: BoundingBox {
            x_min: 10.0,
            y_min: 20.0,
            x_max: 310.0,
            y_max: 220.0,
        },
        rotation_degrees: None,
        text: source.into(),
        color: Some(Color { r: 0, g: 0, b: 0 }),
        highlight_color: None,
        underline: false,
        font_size: Some(SIZE as f32),
        runs: Vec::new(),
        spans: Vec::new(),
        paragraphs: Vec::new(),
        object_spans: Vec::new(),
        text_sections: Vec::new(),
        margins: None,
        gravity: None,
    }
}

fn color_span(kind: RichTextSpanType, start: u32, end: u32, argb: u32) -> RichTextSpan {
    RichTextSpan {
        kind,
        start_utf16: start,
        end_utf16: end,
        interval_type: sdocx::SpanIntervalType::from(0),
        payload: argb.to_le_bytes().to_vec(),
    }
}

fn document(context: Context, mut content: RichTextBox) -> Document {
    if matches!(context, Context::Flow) {
        content.bbox = BoundingBox::default();
    }
    Document {
        metadata: DocumentMetadata {
            default_page_dimensions: Some((360, 400)),
            orientation: Some(0),
            ..Default::default()
        },
        pages: vec![Page {
            uuid: "non-latin-positions".into(),
            width: 360,
            height: 400,
            content_bbox: content.bbox,
            background_color: Some(Color {
                r: 255,
                g: 255,
                b: 255,
            }),
            template: None,
            background: Default::default(),
            objects: vec![PageElement::TextBox(content).into()],
        }],
    }
}

fn render(context: Context, content: RichTextBox, replay: bool) -> RenderedPage {
    let document = document(context, content);
    let layout = sdocx::layout_document(&document);
    let page = if replay {
        sdocx::render_layout_page_replay_svg(&document, &layout, 0, &Default::default())
    } else {
        sdocx::render_layout_page_svg(&document, &layout, 0, &Default::default())
    }
    .unwrap();
    assert!(
        page.text_diagnostics.is_empty(),
        "{context:?}: {:?}",
        page.text_diagnostics
    );
    page
}

fn origin(context: Context) -> (f64, f64) {
    match context {
        Context::Placed => (10.0, 40.0),
        Context::Flow => (48.0, 20.0),
    }
}

#[derive(Debug)]
struct Glyph {
    id: u16,
    source: String,
    x: f64,
    y: f64,
}

fn collect_glyphs(group: &usvg::Group, output: &mut Vec<Glyph>) {
    for node in group.children() {
        match node {
            usvg::Node::Group(group) => collect_glyphs(group, output),
            usvg::Node::Text(text) => {
                for span in text.layouted() {
                    for glyph in &span.positioned_glyphs {
                        let position = glyph.transform();
                        output.push(Glyph {
                            id: glyph.id.0,
                            source: glyph.text.clone(),
                            x: f64::from(position.tx),
                            y: f64::from(position.ty),
                        });
                    }
                }
            }
            _ => {}
        }
    }
}

fn glyphs(page: &RenderedPage) -> Vec<Glyph> {
    let tree = usvg::Tree::from_str(
        &page.svg,
        &usvg::Options {
            fontdb: FontBook::default().database(),
            ..Default::default()
        },
    )
    .unwrap();
    let mut result = Vec::new();
    collect_glyphs(tree.root(), &mut result);
    result
}

fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 0.0001, "{actual} != {expected}");
}

fn assert_geometry(
    page: &RenderedPage,
    context: Context,
    source: &str,
    ids: &[u16],
    advances: &[f64],
) {
    let actual = glyphs(page);
    assert_eq!(actual.iter().map(|glyph| glyph.id).collect::<Vec<_>>(), ids);
    assert_eq!(
        actual
            .iter()
            .map(|glyph| glyph.source.as_str())
            .collect::<String>(),
        source
    );
    let (left, baseline) = origin(context);
    let mut x = left;
    for (glyph, advance) in actual.iter().zip(advances) {
        close(glyph.x, x);
        close(glyph.y, baseline);
        x += advance / UNITS * SIZE;
    }
    let xml = roxmltree::Document::parse(&page.svg).unwrap();
    let spans: Vec<_> = xml
        .descendants()
        .filter(|node| node.has_tag_name("tspan"))
        .collect();
    assert_eq!(
        spans
            .iter()
            .filter_map(|node| node.text())
            .collect::<String>(),
        source
    );
    assert!(spans.iter().all(|node| node.attribute("x").is_some()));
    assert!(
        !xml.descendants()
            .any(|node| node.has_tag_name("image") || node.has_tag_name("foreignObject"))
    );
}

#[test]
fn covered_greek_and_cyrillic_keep_pinned_glyph_positions_in_normal_and_replay_svg() {
    for context in CONTEXTS {
        for replay in [false, true] {
            let page = render(context, text("λΩЖя"), replay);
            assert_geometry(
                &page,
                context,
                "λΩЖя",
                &[579, 569, 624, 663],
                &[1134.0, 1362.0, 1859.0, 1124.0],
            );
            #[cfg(feature = "pdf")]
            assert_pdf(&page, context, "λΩЖя", &[1134.0, 1362.0, 1859.0, 1124.0]);
        }
    }
}

#[test]
fn greek_kerning_survives_positioned_paint_but_color_runs_shape_independently() {
    for context in CONTEXTS {
        let page = render(context, text("λλ"), false);
        assert_geometry(&page, context, "λλ", &[579, 579], &[1150.0, 1134.0]);
        let mut colored = text("λλ");
        colored.spans.push(color_span(
            RichTextSpanType::ForegroundColor,
            1,
            2,
            0xffff0000,
        ));
        let page = render(context, colored, false);
        assert_geometry(&page, context, "λλ", &[579, 579], &[1134.0, 1134.0]);
        let xml = roxmltree::Document::parse(&page.svg).unwrap();
        assert_eq!(
            xml.descendants()
                .filter(|node| node.has_tag_name("tspan"))
                .map(|node| node.attribute("fill").unwrap())
                .collect::<Vec<_>>(),
            ["#000000", "#ff0000"]
        );
        #[cfg(feature = "pdf")]
        assert_pdf(&page, context, "λλ", &[1134.0, 1134.0]);
    }
}

#[test]
fn uniform_non_latin_background_preserves_shaping_and_measured_width() {
    for context in CONTEXTS {
        for (source, ids, advances) in [
            ("λλ", vec![579, 579], vec![1150.0, 1134.0]),
            ("Жя", vec![624, 663], vec![1859.0, 1124.0]),
        ] {
            let plain = render(context, text(source), false);
            let mut highlighted = text(source);
            highlighted.spans.push(color_span(
                RichTextSpanType::BackgroundColor,
                0,
                2,
                0xff00ff00,
            ));
            let page = render(context, highlighted, false);
            assert_geometry(&page, context, source, &ids, &advances);
            assert_eq!(
                glyphs(&plain)
                    .iter()
                    .map(|glyph| (glyph.id, glyph.x, glyph.y))
                    .collect::<Vec<_>>(),
                glyphs(&page)
                    .iter()
                    .map(|glyph| (glyph.id, glyph.x, glyph.y))
                    .collect::<Vec<_>>()
            );
            let xml = roxmltree::Document::parse(&page.svg).unwrap();
            let rectangles: Vec<_> = xml
                .descendants()
                .filter(|node| {
                    node.has_tag_name("rect") && node.attribute("fill") == Some("#00ff00")
                })
                .collect();
            assert_eq!(rectangles.len(), 1);
            let rect = rectangles[0];
            let (left, baseline) = origin(context);
            close(rect.attribute("x").unwrap().parse().unwrap(), left);
            close(
                rect.attribute("y").unwrap().parse().unwrap(),
                baseline - SIZE,
            );
            close(
                rect.attribute("width").unwrap().parse().unwrap(),
                advances.iter().sum::<f64>() / UNITS * SIZE,
            );
            close(rect.attribute("height").unwrap().parse().unwrap(), 27.0);
            #[cfg(feature = "pdf")]
            if source == "Жя" {
                assert_pdf(&page, context, source, &advances);
            }
        }
    }
}

#[cfg(feature = "pdf")]
fn assert_pdf(page: &RenderedPage, context: Context, source: &str, advances: &[f64]) {
    let bytes =
        sdocx::render_svg_pages_pdf(std::slice::from_ref(page), &Default::default()).unwrap();
    let pdf = lopdf::Document::load_mem(&bytes).unwrap();
    assert_eq!(pdf.extract_text(&[1]).unwrap().replace('\n', ""), source);
    assert!(
        pdf.objects
            .values()
            .any(|object| object.as_dict().is_ok_and(|dict| dict.has(b"FontFile2")))
    );
    assert!(
        !pdf.objects
            .values()
            .any(|object| object.as_stream().is_ok_and(|stream| stream
                .dict
                .get(b"Subtype")
                .is_ok_and(|value| value.as_name().is_ok_and(|name| name == b"Image"))))
    );
    let widths: Vec<_> = pdf
        .objects
        .values()
        .filter_map(|object| object.as_dict().ok())
        .filter_map(|dict| dict.get(b"W").ok())
        .flat_map(|object| object.as_array().unwrap().as_chunks::<3>().0)
        .map(|group| f64::from(group[2].as_float().unwrap()))
        .collect();
    for advance in advances {
        assert!(
            widths
                .iter()
                .any(|width| (*width - advance / UNITS * 1000.0).abs() < 0.0001),
            "missing width for {advance}: {widths:?}"
        );
    }
    let mut operations = Vec::new();
    for object in pdf.objects.values() {
        if let Ok(stream) = object.as_stream()
            && stream
                .dict
                .get(b"Subtype")
                .is_ok_and(|value| value.as_name().is_ok_and(|name| name == b"Form"))
        {
            operations.extend(
                lopdf::content::Content::decode(&stream.decompressed_content().unwrap())
                    .unwrap()
                    .operations,
            );
        }
    }
    for id in pdf.get_pages().values() {
        operations.extend(
            lopdf::content::Content::decode(&pdf.get_page_content(*id).unwrap())
                .unwrap()
                .operations,
        );
    }
    assert!(operations.iter().any(|operation| operation.operator == "Tf"
        && (f64::from(operation.operands[1].as_float().unwrap()) - SIZE).abs() < 0.0001));
    assert!(operations.iter().any(|operation| {
        operation.operator == "Tm"
            && operation
                .operands
                .iter()
                .map(|value| f64::from(value.as_float().unwrap()))
                .zip([1.0, 0.0, 0.0, -1.0, 0.0, 0.0])
                .all(|(actual, expected)| (actual - expected).abs() < 0.0001)
    }));
    let (mut x, baseline) = origin(context);
    for advance in advances {
        let matrix = [0.75, 0.0, 0.0, -0.75, x * 0.75, (400.0 - baseline) * 0.75];
        assert!(
            operations.iter().any(|operation| operation.operator == "cm"
                && operation
                    .operands
                    .iter()
                    .map(|value| f64::from(value.as_float().unwrap()))
                    .zip(matrix)
                    .all(|(actual, expected)| (actual - expected).abs() < 0.0001)),
            "{context:?}: missing glyph transform {matrix:?}"
        );
        x += advance / UNITS * SIZE;
    }
}
