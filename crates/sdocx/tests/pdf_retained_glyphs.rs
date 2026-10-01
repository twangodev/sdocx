#![cfg(feature = "pdf")]

#[path = "support/pdf_geometry.rs"]
mod pdf_geometry;

use std::sync::Arc;

use lopdf::{Dictionary, Object, content::Content};
use rustybuzz::ttf_parser::{Face, GlyphId, OutlineBuilder};
use sdocx::{
    BoundingBox, Color, Document, DocumentMetadata, Page, PageElement, PdfOptions, RichTextBox,
    RichTextSpan, RichTextSpanType, SpanIntervalType,
};
use svgtypes::Transform;

const FONT: &[u8] = include_bytes!("assets/fonts/DejaVuSans.ttf");

fn options() -> PdfOptions {
    use sha2::{Digest, Sha256};

    assert_eq!(FONT.len(), 759_720);
    assert_eq!(
        format!("{:x}", Sha256::digest(FONT)),
        "57f73e11f51999432bf7ab22ce55b6f945d5eca1bf824404cfa9ec2e3718c84e"
    );
    let mut database = sdocx::pdf::fontdb::Database::new();
    database.load_font_data(FONT.to_vec());
    database.set_sans_serif_family("DejaVu Sans");
    PdfOptions::new(Arc::new(database))
}

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
        font_size: Some(20.0),
        runs: Vec::new(),
        spans: vec![RichTextSpan {
            kind: RichTextSpanType::FontName,
            start_utf16: 0,
            end_utf16: source.encode_utf16().count() as u32,
            interval_type: SpanIntervalType::ClosedOpen,
            payload: [vec![0; 8], vec![12, 0], b"DejaVu Sans\0".to_vec()].concat(),
        }],
        paragraphs: Vec::new(),
        object_spans: Vec::new(),
        text_sections: Vec::new(),
        margins: None,
        gravity: None,
    }
}

fn document(content: RichTextBox) -> Document {
    Document {
        metadata: DocumentMetadata {
            default_page_dimensions: Some((360, 400)),
            ..Default::default()
        },
        pages: vec![Page {
            uuid: "retained-pdf".into(),
            width: 400,
            height: 400,
            content_bbox: Default::default(),
            background_color: None,
            template: None,
            background: Default::default(),
            objects: vec![PageElement::TextBox(content).into()],
        }],
    }
}

fn export(content: RichTextBox) -> Vec<u8> {
    sdocx::render_document_pdf(&document(content), &Default::default(), &options()).unwrap()
}

#[derive(Default, Debug, PartialEq)]
struct Outline(Vec<(u8, Vec<u32>)>);

impl Outline {
    fn push(&mut self, kind: u8, coordinates: &[f32]) {
        self.0.push((
            kind,
            coordinates.iter().map(|value| value.to_bits()).collect(),
        ));
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

fn outline(face: &Face<'_>, id: u16) -> Outline {
    let mut outline = Outline::default();
    face.outline_glyph(GlyphId(id), &mut outline).unwrap();
    outline
}

fn dictionary<'a>(pdf: &'a lopdf::Document, object: &'a Object) -> &'a Dictionary {
    pdf.dereference(object).unwrap().1.as_dict().unwrap()
}

fn stream(pdf: &lopdf::Document, object: &Object) -> Vec<u8> {
    let stream = pdf.dereference(object).unwrap().1.as_stream().unwrap();
    stream
        .decompressed_content()
        .unwrap_or_else(|_| stream.content.clone())
}

#[derive(Debug)]
struct Glyph {
    outline: Outline,
    advance: u16,
    x: f64,
    y: f64,
    operation: usize,
    clips: usize,
}

fn number(object: &Object) -> f64 {
    f64::from(object.as_float().unwrap())
}

fn compose(left: Transform, right: Transform) -> Transform {
    Transform::new(
        left.a * right.a + left.c * right.b,
        left.b * right.a + left.d * right.b,
        left.a * right.c + left.c * right.d,
        left.b * right.c + left.d * right.d,
        left.a * right.e + left.c * right.f + left.e,
        left.b * right.e + left.d * right.f + left.f,
    )
}

fn cid_width(font: &Dictionary, cid: u16) -> f64 {
    if let Ok(widths) = font.get(b"W") {
        let widths = widths.as_array().unwrap();
        let mut index = 0;
        while index < widths.len() {
            let first = widths[index].as_i64().unwrap() as u16;
            if let Ok(values) = widths[index + 1].as_array() {
                if let Some(value) = cid
                    .checked_sub(first)
                    .and_then(|index| values.get(usize::from(index)))
                {
                    return number(value);
                }
                index += 2;
            } else {
                let last = widths[index + 1].as_i64().unwrap() as u16;
                if (first..=last).contains(&cid) {
                    return number(&widths[index + 2]);
                }
                index += 3;
            }
        }
    }
    font.get(b"DW").map_or(1000.0, number)
}

fn glyphs(bytes: &[u8]) -> Vec<Glyph> {
    glyphs_on_page(bytes, 1)
}

fn glyphs_on_page(bytes: &[u8], page_number: u32) -> Vec<Glyph> {
    let pdf = lopdf::Document::load_mem(bytes).unwrap();
    let page_id = pdf.get_pages()[&page_number];
    let page = pdf.get_dictionary(page_id).unwrap();
    let height = number(&page.get(b"MediaBox").unwrap().as_array().unwrap()[3]);
    let fonts = pdf.get_page_fonts(page_id).unwrap();
    let content = Content::decode(&pdf.get_page_content(page_id).unwrap()).unwrap();
    let mut result = Vec::new();
    let mut ctm = Transform::default();
    let mut stack = Vec::new();
    let mut tm = Transform::default();
    let mut font_name = Vec::new();
    let mut font_size = 0.0;
    let mut cursor = 0.0;
    let mut clips = 0;
    for (operation_index, operation) in content.operations.into_iter().enumerate() {
        match operation.operator.as_str() {
            "q" => stack.push((ctm, clips)),
            "Q" => (ctm, clips) = stack.pop().unwrap(),
            "W" | "W*" => clips += 1,
            "cm" | "Tm" => {
                let values = operation.operands.iter().map(number).collect::<Vec<_>>();
                let matrix = Transform::new(
                    values[0], values[1], values[2], values[3], values[4], values[5],
                );
                if operation.operator == "cm" {
                    ctm = compose(ctm, matrix);
                } else {
                    tm = matrix;
                    cursor = 0.0;
                }
            }
            "Tf" => {
                font_name = operation.operands[0].as_name().unwrap().to_vec();
                font_size = number(&operation.operands[1]);
            }
            "Tj" | "TJ" => {
                let root_font = fonts[&font_name];
                let font = dictionary(
                    &pdf,
                    &root_font
                        .get(b"DescendantFonts")
                        .unwrap()
                        .as_array()
                        .unwrap()[0],
                );
                let descriptor = dictionary(&pdf, font.get(b"FontDescriptor").unwrap());
                let data = stream(&pdf, descriptor.get(b"FontFile2").unwrap());
                let face = Face::parse(&data, 0).unwrap();
                assert_eq!(face.units_per_em(), 2048);
                let mapping = font.get(b"CIDToGIDMap").unwrap();
                let mapping_data = mapping.as_name().is_err().then(|| stream(&pdf, mapping));
                if mapping_data.is_none() {
                    assert_eq!(mapping.as_name().unwrap(), b"Identity");
                }
                let operands = operation.operands[0]
                    .as_array()
                    .map_or(operation.operands.as_slice(), Vec::as_slice);
                for value in operands {
                    if let Ok(bytes) = value.as_str() {
                        let (cids, remainder) = bytes.as_chunks::<2>();
                        assert!(remainder.is_empty());
                        for cid in cids.iter().copied().map(u16::from_be_bytes) {
                            let gid = mapping_data.as_ref().map_or(cid, |mapping| {
                                u16::from_be_bytes(
                                    mapping[usize::from(cid) * 2..usize::from(cid) * 2 + 2]
                                        .try_into()
                                        .unwrap(),
                                )
                            });
                            let mut shape = Outline::default();
                            if face.outline_glyph(GlyphId(gid), &mut shape).is_some() {
                                let transform = compose(ctm, tm);
                                result.push(Glyph {
                                    outline: shape,
                                    advance: face.glyph_hor_advance(GlyphId(gid)).unwrap(),
                                    x: (transform.e + transform.a * cursor) / 0.75,
                                    y: (height - transform.f - transform.b * cursor) / 0.75,
                                    operation: operation_index,
                                    clips,
                                });
                            }
                            cursor += cid_width(font, cid) * font_size / 1000.0;
                        }
                    } else {
                        cursor -= number(value) * font_size / 1000.0;
                    }
                }
            }
            _ => {}
        }
    }
    assert!(
        !pdf.objects
            .values()
            .any(|object| object.as_stream().is_ok_and(|stream| {
                stream
                    .dict
                    .get(b"Subtype")
                    .is_ok_and(|value| value.as_name().is_ok_and(|name| name == b"Image"))
            }))
    );
    result
}

fn assert_glyph(actual: &[Glyph], id: u16, advance: u16, x: f64, y: f64) {
    let face = Face::parse(FONT, 0).unwrap();
    let expected = outline(&face, id);
    let matches = actual
        .iter()
        .filter(|glyph| glyph.outline == expected)
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "expected one original GID{id}: {actual:?}"
    );
    let glyph = matches[0];
    assert_eq!(glyph.advance, advance);
    assert!((glyph.x - x).abs() < 0.0002, "GID{id} x {} != {x}", glyph.x);
    assert!((glyph.y - y).abs() < 0.0002, "GID{id} y {} != {y}", glyph.y);
}

fn black_rectangles(bytes: &[u8]) -> Vec<[f64; 4]> {
    let pdf = lopdf::Document::load_mem(bytes).unwrap();
    let page_id = pdf.get_pages()[&1];
    let page = pdf.get_dictionary(page_id).unwrap();
    let height = number(&page.get(b"MediaBox").unwrap().as_array().unwrap()[3]);
    let content = Content::decode(&pdf.get_page_content(page_id).unwrap()).unwrap();
    let mut ctm = Transform::default();
    let mut black = true;
    let mut stack = Vec::new();
    let mut points = Vec::new();
    let mut rectangles = Vec::new();
    for operation in content.operations {
        let values = operation.operands.iter().map(number);
        match operation.operator.as_str() {
            "q" => stack.push((ctm, black)),
            "Q" => (ctm, black) = stack.pop().unwrap(),
            "cm" => {
                let values = values.collect::<Vec<_>>();
                ctm = compose(
                    ctm,
                    Transform::new(
                        values[0], values[1], values[2], values[3], values[4], values[5],
                    ),
                );
            }
            "g" | "rg" | "sc" | "scn" => black = values.clone().all(|value| value == 0.0),
            "m" | "l" => {
                let values = values.collect::<Vec<_>>();
                points.push((values[0], values[1]));
            }
            "re" => {
                let values = values.collect::<Vec<_>>();
                let (x, y, w, h) = (values[0], values[1], values[2], values[3]);
                points.extend([(x, y), (x + w, y), (x + w, y + h), (x, y + h)]);
            }
            "f" | "f*" => {
                if black && !points.is_empty() {
                    let points = points
                        .iter()
                        .map(|&(x, y)| {
                            (
                                (ctm.a * x + ctm.c * y + ctm.e) / 0.75,
                                (height - ctm.b * x - ctm.d * y - ctm.f) / 0.75,
                            )
                        })
                        .collect::<Vec<_>>();
                    let left = points
                        .iter()
                        .map(|point| point.0)
                        .fold(f64::INFINITY, f64::min);
                    let right = points
                        .iter()
                        .map(|point| point.0)
                        .fold(f64::NEG_INFINITY, f64::max);
                    let top = points
                        .iter()
                        .map(|point| point.1)
                        .fold(f64::INFINITY, f64::min);
                    let bottom = points
                        .iter()
                        .map(|point| point.1)
                        .fold(f64::NEG_INFINITY, f64::max);
                    assert!(
                        points.iter().all(|&(x, y)| {
                            (x == left || x == right) && (y == top || y == bottom)
                        })
                    );
                    rectangles.push([left, top, right - left, bottom - top]);
                }
                points.clear();
            }
            "n" | "S" | "s" => points.clear(),
            "c" | "v" | "y" => panic!("expected vector rectangles, found a curved path"),
            _ => {}
        }
    }
    rectangles
}

fn decoration(
    kind: RichTextSpanType,
    start_utf16: u32,
    end_utf16: u32,
    enabled: bool,
) -> RichTextSpan {
    RichTextSpan {
        kind,
        start_utf16,
        end_utf16,
        interval_type: SpanIntervalType::ClosedOpen,
        payload: u16::from(enabled).to_le_bytes().to_vec(),
    }
}

fn assert_rectangle(actual: [f64; 4], expected: [f64; 4]) {
    for (actual, expected) in actual.into_iter().zip(expected) {
        assert!((actual - expected).abs() < 0.0002, "{actual} != {expected}");
    }
}

#[test]
fn a_retained_cluster_uses_its_anchor_decoration_and_ignores_nonanchor_changes() {
    for source in ["لا", "لَا"] {
        for (kind, top) in [
            (RichTextSpanType::Underline, 42.22222223877907),
            (RichTextSpanType::Strikethrough, 34.28571403026581),
        ] {
            let mut nonanchor = text(source);
            nonanchor.spans.push(decoration(kind, 1, 2, true));
            let bytes = export(nonanchor);
            assert!(black_rectangles(&bytes).is_empty(), "{source:?} {kind:?}");
            assert_glyph(&glyphs(&bytes), 5365, 1168, 10.0, 40.0);
            assert_eq!(selected_source(&bytes), source);

            let mut anchor = text(source);
            anchor.spans.push(decoration(kind, 0, 1, true));
            anchor.spans.push(decoration(kind, 1, 2, false));
            let bytes = export(anchor);
            let rectangles = black_rectangles(&bytes);
            assert_eq!(rectangles.len(), 1, "{source:?} {kind:?}");
            assert_rectangle(rectangles[0], [10.0, top, 11.40625, 1.111111119389534]);
            assert_glyph(&glyphs(&bytes), 5365, 1168, 10.0, 40.0);
            if source == "لَا" {
                assert_glyph(&glyphs(&bytes), 1399, 0, 13.466796875, 35.60546875);
            }
            assert_eq!(selected_source(&bytes), source);
        }
    }
}

#[test]
fn adjacent_identical_underlined_glyphs_draw_one_native_rectangle() {
    let mut content = text("AB");
    content.underline = true;
    let bytes = export(content);
    let rectangles = black_rectangles(&bytes);
    assert_eq!(rectangles.len(), 1);
    // HarfBuzz 10.2.0 / DejaVu 2.37: GIDs 36/37 advance 1401/1405 at UPEM 2048.
    assert_rectangle(
        rectangles[0],
        [10.0, 42.22222223877907, 27.40234375, 1.111111119389534],
    );
    assert_glyph(&glyphs(&bytes), 36, 1401, 10.0, 40.0);
    assert_glyph(&glyphs(&bytes), 37, 1405, 23.681640625, 40.0);
    assert_eq!(selected_source(&bytes), "AB");
}

fn selected_source(bytes: &[u8]) -> String {
    let geometry = pdf_geometry::read(bytes, 96.0);
    assert_eq!(geometry.image_resources, 0);
    geometry.source
}

#[test]
fn arabic_ligature_keeps_the_selected_glyph_and_logical_source() {
    let bytes = export(text("لا"));
    assert_glyph(&glyphs(&bytes), 5365, 1168, 10.0, 40.0);
    let geometry = pdf_geometry::read(&bytes, 96.0);
    assert_eq!(geometry.image_resources, 0);
    assert_eq!(geometry.source, "لا");
    assert_eq!(geometry.extracted_text.replace('\n', ""), "لا");
}

#[test]
fn contextual_ligature_in_an_isolate_keeps_native_paragraph_positions() {
    let source = "\u{2066}لا\u{2069}_123";
    let bytes = export(text(source));
    let glyphs = glyphs(&bytes);
    assert_glyph(&glyphs, 5365, 1168, 58.173828125, 40.0);
    assert_glyph(&glyphs, 20, 1303, 10.0, 40.0);
    assert_glyph(&glyphs, 66, 1024, 48.173828125, 40.0);
    assert_eq!(selected_source(&bytes), source);
}

#[test]
fn arabic_combining_mark_keeps_independent_native_offsets() {
    let bytes = export(text("لَا"));
    let actual = glyphs(&bytes);
    assert_glyph(&actual, 5365, 1168, 10.0, 40.0);
    assert_glyph(&actual, 1399, 0, 13.466796875, 35.60546875);
    assert_eq!(selected_source(&bytes), "لَا");
}

#[test]
fn native_contextual_glyph_positions_follow_the_text_box_rotation() {
    let source = "\u{2066}لا\u{2069}_123";
    let mut content = text(source);
    content.bbox = BoundingBox {
        x_min: 30.0,
        y_min: 80.0,
        x_max: 190.0,
        y_max: 180.0,
    };
    content.rotation_degrees = Some(90.0);
    let bytes = export(content);
    assert_glyph(&glyphs(&bytes), 5365, 1168, 140.0, 98.173828125);
    assert_eq!(selected_source(&bytes), source);
}

#[test]
fn native_body_text_keeps_the_page_clip_and_paragraph_rank_geometry() {
    let source = "\u{2066}لا\u{2069}_123";
    let mut content = text(source);
    content.bbox = BoundingBox::default();
    let mut doc = document(text(""));
    doc.pages[0].objects.clear();
    doc.metadata.note_text = Some(content);
    doc.metadata.page_mode = Some(0);
    doc.metadata.flow_page_padding = Some((0, 0));
    let bytes = sdocx::render_document_pdf(&doc, &Default::default(), &options()).unwrap();
    let actual = glyphs(&bytes);
    assert_glyph(&actual, 5365, 1168, 48.173828125, 30.0);
    assert!(actual.iter().all(|glyph| glyph.clips > 0));
    assert_eq!(selected_source(&bytes), source);
}

#[test]
fn glyph_dispatch_stays_between_the_surrounding_vector_paint() {
    let mut first = text("لا");
    first.highlight_color = Some(Color { r: 255, g: 0, b: 0 });
    let mut second = text("B");
    second.highlight_color = Some(Color { r: 0, g: 0, b: 255 });
    let mut doc = document(first);
    doc.pages[0]
        .objects
        .push(PageElement::TextBox(second).into());
    let bytes = sdocx::render_document_pdf(&doc, &Default::default(), &options()).unwrap();
    let actual = glyphs(&bytes);
    let face = Face::parse(FONT, 0).unwrap();
    let first = actual
        .iter()
        .find(|glyph| glyph.outline == outline(&face, 5365))
        .unwrap();
    let second = actual
        .iter()
        .find(|glyph| glyph.outline == outline(&face, 37))
        .unwrap();
    let pdf = lopdf::Document::load_mem(&bytes).unwrap();
    let content = Content::decode(&pdf.get_page_content(pdf.get_pages()[&1]).unwrap()).unwrap();
    assert!(first.operation < second.operation);
    assert!(
        content.operations[..first.operation]
            .iter()
            .any(|operation| operation.operator == "f")
    );
    assert!(
        content.operations[first.operation + 1..second.operation]
            .iter()
            .any(|operation| operation.operator == "f")
    );
    assert_eq!(selected_source(&bytes), "لاB");
}

#[test]
fn separate_arabic_source_slots_keep_visual_positions_and_logical_selection() {
    let bytes = export(text("اب"));
    let actual = glyphs(&bytes);
    assert_glyph(&actual, 1365, 569, 28.828125, 40.0);
    assert_glyph(&actual, 1366, 1928, 10.0, 40.0);
    assert_eq!(selected_source(&bytes), "اب");
}

#[test]
fn outer_actual_text_replaces_nested_glyph_content_once() {
    use lopdf::dictionary;

    let mut pdf = lopdf::Document::with_version("1.7");
    let pages = pdf.new_object_id();
    let font = pdf.add_object(dictionary! {
        "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica", "Encoding" => "WinAnsiEncoding",
    });
    let content = pdf.add_object(lopdf::Stream::new(Dictionary::new(),
        b"BT /F1 12 Tf 1 0 0 1 10 20 Tm /Span << /ActualText <FEFF006F0075007400650072> >> BDC /Span << /ActualText (inner) >> BDC (A) Tj EMC (B) Tj EMC ET".to_vec()));
    let page = pdf.add_object(dictionary! {
        "Type" => "Page", "Parent" => pages, "MediaBox" => vec![0.into(),0.into(),100.into(),100.into()],
        "Resources" => dictionary! { "Font" => dictionary! { "F1" => font } }, "Contents" => content,
    });
    pdf.objects.insert(
        pages,
        dictionary! { "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1 }.into(),
    );
    let catalog = pdf.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
    pdf.trailer.set("Root", catalog);
    let mut bytes = Vec::new();
    pdf.save_to(&mut bytes).unwrap();
    let geometry = pdf_geometry::read(&bytes, 96.0);
    assert_eq!(geometry.actual_text, ["outer", "inner"]);
    assert_eq!(geometry.source, "outer");
}

#[test]
fn selected_layout_pages_keep_requested_order_and_explicit_font_book() {
    let mut doc = document(text("لا"));
    let mut second = document(text("B")).pages.remove(0);
    second.uuid = "second-retained-pdf".into();
    doc.pages.push(second);
    let layout = sdocx::layout_document(&doc);
    let fonts = sdocx::fonts::FontBook::new(options().font_database);
    for pdf_options in [
        PdfOptions::default(),
        PdfOptions::new(Arc::new(sdocx::pdf::fontdb::Database::new())),
    ] {
        let bytes = sdocx::render_layout_pages_pdf_with_fonts(
            &doc,
            &layout,
            &[1, 0],
            &Default::default(),
            &pdf_options,
            &fonts,
        )
        .unwrap();
        assert_glyph(&glyphs_on_page(&bytes, 1), 37, 1405, 10.0, 40.0);
        assert_glyph(&glyphs_on_page(&bytes, 2), 5365, 1168, 10.0, 40.0);
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        assert_eq!(pdf.get_pages().len(), 2);
        assert_eq!(pdf.extract_text(&[1]).unwrap().replace('\n', ""), "B");
        assert_eq!(pdf.extract_text(&[2]).unwrap().replace('\n', ""), "لا");
    }
}

#[test]
fn zero_ink_native_text_keeps_exact_selection_without_painting_a_carrier_glyph() {
    let fonts = [
        (false, sdocx::fonts::FontBook::default()),
        (true, sdocx::fonts::FontBook::new(options().font_database)),
    ];
    let pdf_options = PdfOptions::new(Arc::new(sdocx::pdf::fontdb::Database::new()));
    for (named_font, fonts) in fonts {
        for source in [" ", "  ", "\u{a0}", "\u{200b}", "\u{2066}\u{2069}"] {
            let mut content = text(source);
            if !named_font {
                content.spans.clear();
            }
            let doc = document(content);
            let layout = sdocx::layout_document(&doc);
            let bytes = sdocx::render_layout_pages_pdf_with_fonts(
                &doc,
                &layout,
                &[0],
                &Default::default(),
                &pdf_options,
                &fonts,
            )
            .unwrap();
            let geometry = pdf_geometry::read(&bytes, 96.0);
            assert_eq!(geometry.source, source, "named font {named_font}");
            assert!(geometry.actual_text.iter().any(|value| value == source));
            assert!(
                geometry
                    .actual_text
                    .iter()
                    .all(|value| !value.contains('.'))
            );
            assert_eq!(geometry.image_resources, 0);
            assert!(
                glyphs(&bytes).is_empty(),
                "{source:?}, named font {named_font}"
            );
        }
    }
}

#[test]
fn editing_the_layout_text_changes_retained_glyphs_without_rebuilding_the_document() {
    let doc = document(text("A"));
    let mut layout = sdocx::layout_document(&doc);
    let PageElement::TextBox(content) = layout.pages[0].page.elements_mut().next().unwrap() else {
        panic!()
    };
    *content = text("لا");
    let options = options();
    let fonts = sdocx::fonts::FontBook::new(options.font_database.clone());
    let bytes = sdocx::render_layout_pages_pdf_with_fonts(
        &doc,
        &layout,
        &[0],
        &Default::default(),
        &options,
        &fonts,
    )
    .unwrap();
    assert_glyph(&glyphs(&bytes), 5365, 1168, 10.0, 40.0);
    assert_eq!(selected_source(&bytes), "لا");
    let PageElement::TextBox(content) = doc.pages[0].elements().next().unwrap() else {
        panic!()
    };
    assert_eq!(content.text, "A");
}

#[test]
fn an_inline_code_object_keeps_neighboring_contextual_glyphs_and_its_own_text() {
    let mut content = text("A\u{fffc}لا");
    content.object_spans.push(sdocx::RichTextObjectSpan {
        object_type: sdocx::ObjectType::CodeBlock,
        text_index_utf16: 1,
        object_data: Vec::new(),
        content: Some(sdocx::RichTextObjectContent::CodeBlock(Box::new(
            sdocx::RichTextCodeBlock {
                bbox: BoundingBox {
                    x_min: 0.0,
                    y_min: 0.0,
                    x_max: 160.0,
                    y_max: 100.0,
                },
                rotation_degrees: None,
                title: None,
                body: Some(text("Q")),
            },
        ))),
        layout_option: sdocx::ObjectSpanLayoutOption::Inline,
        layout_constraint: sdocx::ObjectSpanLayoutConstraint::Normal,
    });
    let bytes = export(content);
    let actual = glyphs(&bytes);
    assert_glyph(&actual, 36, 1401, 10.0, 120.001);
    assert_glyph(&actual, 5365, 1168, 183.681640625, 120.001);
    assert_glyph(&actual, 52, 1612, 39.681640625, 84.001);
    let source = selected_source(&bytes);
    assert!(
        !source.contains('\u{fffc}'),
        "supported object marker became parent text: {source:?}"
    );
    assert_eq!(source.matches('Q').count(), 1);
    assert_eq!(source.replace('Q', ""), "Aلا");
}
