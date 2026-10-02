#![cfg(feature = "pdf")]

use std::ops::Range;

use rustybuzz::ttf_parser::{Face, GlyphId, OutlineBuilder};
use sdocx::fonts::{FontBook, svg_font_resolver};
use sdocx::{
    BoundingBox, Color, Document, DocumentMetadata, Page, PageElement, PdfOptions, RichTextBox,
    RichTextSpan, RichTextSpanType, SpanIntervalType,
};

fn span(kind: RichTextSpanType, range: Range<u32>, payload: Vec<u8>) -> RichTextSpan {
    RichTextSpan {
        kind,
        start_utf16: range.start,
        end_utf16: range.end,
        interval_type: SpanIntervalType::ClosedOpen,
        payload,
    }
}

fn font_name(range: Range<u32>, name: &str) -> RichTextSpan {
    span(
        RichTextSpanType::FontName,
        range,
        [
            vec![0; 8],
            ((name.len() + 1) as u16).to_le_bytes().to_vec(),
            name.as_bytes().to_vec(),
            vec![0],
        ]
        .concat(),
    )
}

fn document() -> Document {
    let bbox = BoundingBox {
        x_min: 10.0,
        y_min: 20.0,
        x_max: 310.0,
        y_max: 220.0,
    };
    let text = RichTextBox {
        text_area_type: None,
        bbox,
        rotation_degrees: None,
        text: "WW".into(),
        color: Some(Color { r: 0, g: 0, b: 0 }),
        highlight_color: None,
        underline: false,
        font_size: Some(20.0),
        runs: Vec::new(),
        spans: vec![
            font_name(1..2, "Roboto-Bold"),
            span(RichTextSpanType::Bold, 0..2, vec![1, 0]),
        ],
        paragraphs: Vec::new(),
        object_spans: Vec::new(),
        text_sections: Vec::new(),
        margins: None,
        gravity: None,
    };
    Document {
        metadata: DocumentMetadata {
            default_page_dimensions: Some((400, 400)),
            ..Default::default()
        },
        pages: vec![Page {
            uuid: "physical-faces".into(),
            width: 400,
            height: 400,
            content_bbox: bbox,
            background_color: None,
            template: None,
            background: Default::default(),
            objects: vec![PageElement::TextBox(text).into()],
        }],
    }
}

fn glyph_fonts(group: &usvg::Group, result: &mut Vec<usvg::fontdb::ID>) {
    for node in group.children() {
        match node {
            usvg::Node::Group(group) => glyph_fonts(group, result),
            usvg::Node::Text(text) => {
                for span in text.layouted() {
                    result.extend(span.positioned_glyphs.iter().map(|glyph| glyph.font));
                }
            }
            _ => {}
        }
    }
}

#[derive(Default, Debug, PartialEq)]
struct Outline(Vec<(u8, Vec<u32>)>);
impl Outline {
    fn push(&mut self, kind: u8, values: &[f32]) {
        self.0
            .push((kind, values.iter().map(|value| value.to_bits()).collect()));
    }
}
impl OutlineBuilder for Outline {
    fn move_to(&mut self, x: f32, y: f32) {
        self.push(0, &[x, y]);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.push(1, &[x, y]);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.push(2, &[x1, y1, x, y]);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.push(3, &[x1, y1, x2, y2, x, y]);
    }
    fn close(&mut self) {
        self.push(4, &[]);
    }
}
fn outline(face: &Face<'_>, id: GlyphId) -> Option<Outline> {
    let mut output = Outline::default();
    face.outline_glyph(id, &mut output)?;
    Some(output)
}

#[test]
fn mixed_physical_faces_keep_selected_outlines_under_synthetic_bold() {
    let fonts = FontBook::default();
    let regular = fonts.resolve("Roboto", false, false).unwrap();
    let bold = fonts.resolve("Roboto", true, false).unwrap();
    assert_ne!(regular.svg_family(), bold.svg_family());
    let document = document();
    let page =
        sdocx::render_document_svg_with_fonts(&document, &Default::default(), &fonts).remove(0);
    let xml = roxmltree::Document::parse(&page.svg).unwrap();
    let spans: Vec<_> = xml
        .descendants()
        .filter(|node| node.has_tag_name("tspan") && node.text() == Some("W"))
        .collect();
    assert_eq!(spans.len(), 2);
    for (span, face) in spans.iter().zip([&regular, &bold]) {
        let family = svgtypes::parse_font_families(span.attribute("font-family").unwrap()).unwrap();
        assert_eq!(
            family[0],
            svgtypes::FontFamily::Named(face.svg_family().to_string())
        );
        assert_eq!(span.attribute("font-weight"), Some("bold"));
    }
    assert_eq!(
        xml.descendants()
            .filter(|node| node.has_tag_name("image") || node.has_tag_name("foreignObject"))
            .count(),
        0
    );
    let database = fonts.database();
    let options = usvg::Options {
        font_resolver: svg_font_resolver(&database),
        fontdb: database,
        ..Default::default()
    };
    let tree = usvg::Tree::from_str(&page.svg, &options).unwrap();
    let mut actual = Vec::new();
    glyph_fonts(tree.root(), &mut actual);
    assert_eq!(actual, [regular.id, bold.id]);
    let unbridged = usvg::Tree::from_str(
        &page.svg,
        &usvg::Options {
            fontdb: fonts.database(),
            ..Default::default()
        },
    )
    .unwrap();
    let mut incorrect = Vec::new();
    glyph_fonts(unbridged.root(), &mut incorrect);
    assert_eq!(incorrect, [bold.id, bold.id]);

    let options = PdfOptions::from_font_book(&fonts);
    let exports = [
        sdocx::render_document_pdf(&document, &Default::default(), &options).unwrap(),
        sdocx::render_svg_pages_pdf(&[page], &options).unwrap(),
    ];
    let expected: Vec<_> = [&regular, &bold]
        .into_iter()
        .map(|face| {
            let parsed = Face::parse(face.bytes(), face.index).unwrap();
            outline(&parsed, parsed.glyph_index('W').unwrap()).unwrap()
        })
        .collect();
    assert_ne!(expected[0], expected[1]);
    for bytes in exports {
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        let mut embedded = Vec::new();
        for object in pdf.objects.values() {
            let Ok(dictionary) = object.as_dict() else {
                continue;
            };
            let Ok(font) = dictionary.get(b"FontFile2") else {
                continue;
            };
            let stream = pdf.dereference(font).unwrap().1.as_stream().unwrap();
            let data = stream
                .decompressed_content()
                .unwrap_or_else(|_| stream.content.clone());
            let face = Face::parse(&data, 0).unwrap();
            embedded.push(
                (0..face.number_of_glyphs())
                    .filter_map(|id| outline(&face, GlyphId(id)))
                    .collect::<Vec<_>>(),
            );
        }
        assert_eq!(embedded.len(), 2);
        for expected in &expected {
            assert!(embedded.iter().any(|font| font.contains(expected)));
        }
    }
}
