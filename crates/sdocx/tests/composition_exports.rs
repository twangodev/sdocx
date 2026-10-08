#![cfg(feature = "render")]

#[allow(dead_code)]
mod support;

use base64::Engine;
use sdocx::{
    BoundingBox, Color, Document, DocumentMetadata, MediaAsset, ObjectRenderLayer, Page,
    PageElement, PageObject, PageObjectContent, Point, RenderOptions, ShapePaint, Stroke,
};

fn bounds(left: f64, top: f64, right: f64, bottom: f64) -> BoundingBox {
    BoundingBox {
        x_min: left,
        y_min: top,
        x_max: right,
        y_max: bottom,
    }
}

fn stroke(color: Color, points: [[f64; 2]; 2]) -> PageObject {
    Stroke {
        bbox: bounds(10.0, 10.0, 110.0, 90.0),
        points: points.into_iter().map(|[x, y]| Point { x, y }).collect(),
        pressures: Vec::new(),
        timestamps: vec![1000, 1010],
        tilts: Vec::new(),
        orientations: Vec::new(),
        color: Some(color),
        pen_width: 30.0,
        rendering: None,
    }
    .into()
}

fn container(children: Vec<PageObject>) -> PageObject {
    PageObject {
        render_layer: ObjectRenderLayer::Base,
        source_offset: None,
        content: PageObjectContent::Container(children),
    }
}

fn document(objects: Vec<PageObject>) -> Document {
    Document {
        pages: vec![Page {
            uuid: "composition".into(),
            width: 120,
            height: 100,
            content_bbox: bounds(0.0, 0.0, 120.0, 100.0),
            background_color: Some(Color {
                r: 255,
                g: 255,
                b: 255,
            }),
            template: None,
            background: Default::default(),
            objects,
        }],
        metadata: DocumentMetadata::default(),
    }
}

fn render_modes(document: &Document) -> [sdocx::RenderedPage; 2] {
    let layout = sdocx::layout_document(document);
    [
        sdocx::render_layout_page_svg(document, &layout, 0, &RenderOptions::default()).unwrap(),
        sdocx::render_layout_page_replay_svg(document, &layout, 0, &RenderOptions::default())
            .unwrap(),
    ]
}

fn pixels(svg: &str) -> resvg::tiny_skia::Pixmap {
    let tree = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default()).unwrap();
    let mut image = resvg::tiny_skia::Pixmap::new(120, 100).unwrap();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut image.as_mut(),
    );
    image
}

fn assert_pixel(image: &resvg::tiny_skia::Pixmap, x: u32, y: u32, expected: [u8; 4]) {
    let pixel = image.pixel(x, y).unwrap();
    assert_eq!(
        [pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()],
        expected,
        "pixel ({x}, {y})"
    );
}

fn image(rotation: f64) -> PageObject {
    let bbox = [40.0_f64, 30.0, 80.0, 70.0]
        .into_iter()
        .flat_map(f64::to_le_bytes)
        .collect::<Vec<_>>();
    let base = [
        5500_u32.to_le_bytes().to_vec(),
        vec![2, 0, b'i', b'm'],
        vec![0; 8],
        bbox.clone(),
        vec![0; 5],
    ]
    .concat();
    let outline = [0_u32, 4, 0]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .chain([0])
        .collect::<Vec<_>>();
    let geometry = [4_u32.to_le_bytes().to_vec(), bbox, vec![0; 9]].concat();
    let fill = [vec![0], 7_i32.to_le_bytes().to_vec(), vec![0; 57]].concat();
    let fields = [(fill.len() as u32).to_le_bytes().to_vec(), vec![2], fill].concat();
    let payload = [
        frame(0, &base),
        frame(6, &outline),
        frame_fields(7, &geometry, 32, &fields),
        frame(3, &[]),
    ]
    .concat();
    let raw = support::page(&[vec![support::object(3, &payload, &[])]], 0, &[]);
    let mut document = sdocx::parse_bytes(&support::archive(&raw)).unwrap();
    let mut object = document.pages[0].objects.pop().unwrap();
    let PageObjectContent::Element(PageElement::PlacedImage(image)) = &mut object.content else {
        panic!("native image")
    };
    image.rotation_degrees = Some(rotation);
    image.media_index = Some(0);
    object
}

fn with_image(mut document: Document) -> Document {
    document.metadata.media_assets.push(MediaAsset {
        name: "media/7.png".into(),
        archive_id: Some(7),
        mime_type: "image/png".into(),
        data: base64::engine::general_purpose::STANDARD.decode(
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC"
        ).unwrap(),
    });
    document
}

fn frame(kind: i16, fixed: &[u8]) -> Vec<u8> {
    frame_fields(kind, fixed, 0, &[])
}

fn frame_fields(kind: i16, fixed: &[u8], fields: u8, flexible: &[u8]) -> Vec<u8> {
    let offset = 14 + fixed.len();
    [
        ((offset + flexible.len()) as u32).to_le_bytes().to_vec(),
        kind.to_le_bytes().to_vec(),
        (offset as u32).to_le_bytes().to_vec(),
        vec![1, u8::from(kind == 0) << 3, 1, fields],
        fixed.to_vec(),
        flexible.to_vec(),
    ]
    .concat()
}

fn shape(bbox: BoundingBox, rotation: f32) -> PageObject {
    let coords = [bbox.x_min, bbox.y_min, bbox.x_max, bbox.y_max]
        .into_iter()
        .flat_map(f64::to_le_bytes)
        .collect::<Vec<_>>();
    let base = [
        5500_u32.to_le_bytes().to_vec(),
        vec![2, 0, b's', b'h'],
        vec![0; 8],
        coords.clone(),
        vec![0; 5],
    ]
    .concat();
    let outline = [0_u32, 4, 0]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .chain([0])
        .collect::<Vec<_>>();
    let geometry = [
        4_u32.to_le_bytes().to_vec(),
        coords.clone(),
        rotation.to_le_bytes().to_vec(),
        vec![0; 5],
        coords,
    ]
    .concat();
    let payload = [frame(0, &base), frame(6, &outline), frame(7, &geometry)].concat();
    let raw = support::page(&[vec![support::object(7, &payload, &[])]], 0, &[]);
    let mut document = sdocx::parse_bytes(&support::archive(&raw)).unwrap();
    let mut object = document.pages[0].objects.pop().unwrap();
    let PageObjectContent::Element(PageElement::Shape(shape)) = &mut object.content else {
        panic!("native shape")
    };
    shape.fill = ShapePaint::Solid(0xff00ff00);
    shape.style.paint = ShapePaint::None;
    object
}

const RED: Color = Color { r: 255, g: 0, b: 0 };
const BLUE: Color = Color { r: 0, g: 0, b: 255 };

fn overlap_document() -> Document {
    document(vec![
        stroke(RED, [[10.0, 50.0], [110.0, 50.0]]),
        shape(bounds(40.0, 30.0, 80.0, 70.0), 0.0),
        stroke(BLUE, [[60.0, 10.0], [60.0, 90.0]]),
    ])
}

#[test]
fn stroke_image_stroke_exports_keep_interleaving_and_independent_overlap_pixels() {
    let document = with_image(document(vec![
        stroke(BLUE, [[10.0, 50.0], [110.0, 50.0]]),
        image(0.0),
        stroke(Color { r: 0, g: 255, b: 0 }, [[60.0, 10.0], [60.0, 90.0]]),
    ]));
    let [normal, replay] = render_modes(&document);
    for page in [&normal, &replay] {
        let xml = roxmltree::Document::parse(&page.svg).unwrap();
        let order = xml
            .descendants()
            .filter_map(|node| {
                if node.has_tag_name("image") {
                    Some("image")
                } else {
                    node.attribute("stroke").filter(|color| *color != "none")
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(order, ["#0000ff", "image", "#00ff00"]);
        let image = pixels(&page.svg);
        assert_pixel(&image, 20, 50, [0, 0, 255, 255]);
        assert_pixel(&image, 45, 50, [255, 0, 0, 255]);
        assert_pixel(&image, 60, 50, [0, 255, 0, 255]);
        assert_pixel(&image, 20, 20, [255, 255, 255, 255]);
    }
    assert_eq!(pixels(&normal.svg).data(), pixels(&replay.svg).data());
}

#[test]
fn vector_shape_stroke_overlap_matches_literal_colors_in_normal_and_replay_svg() {
    let [normal, replay] = render_modes(&overlap_document());
    for page in [&normal, &replay] {
        let image = pixels(&page.svg);
        for (x, y, color) in [
            (20, 50, [255, 0, 0, 255]),
            (45, 50, [0, 255, 0, 255]),
            (60, 50, [0, 0, 255, 255]),
            (70, 40, [0, 255, 0, 255]),
            (20, 20, [255, 255, 255, 255]),
        ] {
            assert_pixel(&image, x, y, color);
        }
    }
    assert_eq!(pixels(&normal.svg).data(), pixels(&replay.svg).data());
}

#[test]
fn nested_containers_keep_leaf_rotation_and_placement_in_page_coordinates() {
    let shape = shape(bounds(10.0, 10.0, 30.0, 20.0), 90.0);
    let mut placed_image = image(90.0);
    let PageObjectContent::Element(PageElement::PlacedImage(image)) = &mut placed_image.content
    else {
        unreachable!()
    };
    image.bbox = bounds(70.0, 10.0, 90.0, 20.0);
    let document = with_image(document(vec![container(vec![container(vec![
        shape,
        placed_image,
    ])])]));
    for page in render_modes(&document) {
        let xml = roxmltree::Document::parse(&page.svg).unwrap();
        let transforms = xml
            .descendants()
            .filter_map(|node| node.attribute("transform"))
            .collect::<Vec<_>>();
        use svgtypes::TransformListToken::{Rotate, Translate};
        assert_eq!(transforms.len(), 2);
        for (transform, x) in transforms.iter().zip([20.0, 80.0]) {
            let tokens = svgtypes::TransformListParser::from(*transform)
                .map(Result::unwrap)
                .collect::<Vec<_>>();
            assert_eq!(
                tokens,
                [
                    Translate { tx: x, ty: 15.0 },
                    Rotate { angle: 90.0 },
                    Translate { tx: -x, ty: -15.0 }
                ]
            );
        }
        let image = pixels(&page.svg);
        assert_pixel(&image, 20, 8, [0, 255, 0, 255]);
        assert_pixel(&image, 12, 15, [255, 255, 255, 255]);
        assert_pixel(&image, 80, 8, [255, 0, 0, 255]);
        assert_pixel(&image, 72, 15, [255, 255, 255, 255]);
    }
}

#[cfg(feature = "pdf")]
fn text(label: &str) -> PageObject {
    PageElement::TextBox(sdocx::RichTextBox {
        text_area_type: None,
        bbox: bounds(5.0, 75.0, 115.0, 100.0),
        rotation_degrees: None,
        text: label.into(),
        color: Some(Color { r: 0, g: 0, b: 0 }),
        highlight_color: None,
        underline: false,
        font_size: Some(4.0),
        runs: Vec::new(),
        spans: Vec::new(),
        paragraphs: Vec::new(),
        object_spans: Vec::new(),
        text_sections: Vec::new(),
        margins: None,
        gravity: None,
    })
    .into()
}

#[cfg(feature = "pdf")]
fn dictionary<'a>(pdf: &'a lopdf::Document, value: &'a lopdf::Object) -> &'a lopdf::Dictionary {
    if let Ok(reference) = value.as_reference() {
        pdf.get_dictionary(reference).unwrap()
    } else {
        value.as_dict().unwrap()
    }
}

#[cfg(feature = "pdf")]
fn pdf_paints(pdf: &lopdf::Document, data: &[u8], resources: &lopdf::Dictionary) -> Vec<[u8; 3]> {
    let mut paints = Vec::new();
    let mut fill = [0; 3];
    let mut stroke = [0; 3];
    let mut stack = Vec::new();
    for operation in lopdf::content::Content::decode(data).unwrap().operations {
        match operation.operator.as_str() {
            "q" => stack.push((fill, stroke)),
            "Q" => {
                (fill, stroke) = stack.pop().unwrap();
            }
            "rg" | "RG" => {
                let color = std::array::from_fn(|index| {
                    (operation.operands[index].as_float().unwrap() * 255.0).round() as u8
                });
                if operation.operator == "rg" {
                    fill = color;
                } else {
                    stroke = color;
                }
            }
            "S" | "s" => paints.push(stroke),
            "f" | "F" | "f*" => paints.push(fill),
            "B" | "B*" | "b" | "b*" => {
                paints.push(fill);
                paints.push(stroke);
            }
            "Do" => {
                let name = operation.operands[0].as_name().unwrap();
                let objects = dictionary(pdf, resources.get(b"XObject").unwrap());
                let reference = objects.get(name).unwrap().as_reference().unwrap();
                let stream = pdf.get_object(reference).unwrap().as_stream().unwrap();
                if stream.dict.get(b"Subtype").unwrap().as_name().unwrap() == b"Form" {
                    let child_resources = stream
                        .dict
                        .get(b"Resources")
                        .map(|value| dictionary(pdf, value))
                        .unwrap_or(resources);
                    paints.extend(pdf_paints(
                        pdf,
                        &stream.decompressed_content().unwrap(),
                        child_resources,
                    ));
                }
            }
            _ => {}
        }
    }
    paints
}

#[cfg(feature = "pdf")]
#[test]
fn mixed_vector_pdf_preserves_paint_order_and_selectable_text_without_image_objects() {
    let mut document = overlap_document();
    document.pages[0]
        .objects
        .insert(2, text("selectable composition"));
    let fonts = sdocx::fonts::FontBook::default();
    let options = sdocx::PdfOptions::from_font_book(&fonts);
    let layout = sdocx::layout_document(&document);
    let pages = [
        sdocx::render_layout_page_svg_with_fonts(
            &document,
            &layout,
            0,
            &RenderOptions::default(),
            &fonts,
        )
        .unwrap(),
        sdocx::render_layout_page_replay_svg_with_fonts(
            &document,
            &layout,
            0,
            &RenderOptions::default(),
            &fonts,
        )
        .unwrap(),
    ];
    for page in pages {
        let bytes = sdocx::render_svg_pages_pdf(&[page], &options).unwrap();
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        let page_id = pdf.get_pages()[&1];
        let resources = dictionary(
            &pdf,
            pdf.get_dictionary(page_id)
                .unwrap()
                .get(b"Resources")
                .unwrap(),
        );
        let paints = pdf_paints(&pdf, &pdf.get_page_content(page_id).unwrap(), resources)
            .into_iter()
            .filter(|color| matches!(color, [255, 0, 0] | [0, 255, 0] | [0, 0, 255]))
            .collect::<Vec<_>>();
        assert_eq!(paints, [[255, 0, 0], [0, 255, 0], [0, 0, 255]]);
        assert!(
            pdf.extract_text(&[1])
                .unwrap()
                .replace('\n', "")
                .contains("selectable composition")
        );
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
    }
}

#[test]
fn vector_progress_preserves_svg_and_repeated_page_pdf_bytes() {
    use sdocx::{DocumentTextCache, PdfOptions, Progress, ProgressStage};
    let document = document(vec![container(vec![stroke(
        Color { r: 255, g: 0, b: 0 },
        [[10.0, 10.0], [110.0, 90.0]],
    )])]);
    let mut events = Vec::new();
    let layout = sdocx::layout_document_with_progress(&document, &mut |event| events.push(event));
    assert!(events.contains(&Progress::new(ProgressStage::Layout, 1, Some(1))));
    let fonts = sdocx::fonts::FontBook::default();
    let options = RenderOptions::default();
    let mut cache = DocumentTextCache::default();
    let normal = cache
        .render_layout_page_svg(&document, &layout, 0, &options, &fonts)
        .unwrap();
    events.clear();
    let reported = cache
        .render_layout_page_svg_with_progress(
            &document,
            &layout,
            0,
            &options,
            &fonts,
            &mut |event| events.push(event),
        )
        .unwrap();
    assert_eq!(normal.svg, reported.svg);
    assert!(events.contains(&Progress::new(ProgressStage::Rendering, 1, Some(2))));
    assert!(events.contains(&Progress::new(ProgressStage::Rendering, 2, Some(2))));
    let normal = sdocx::render_layout_pages_pdf_detailed_with_cache(
        &document,
        &layout,
        &[0, 0],
        &options,
        &PdfOptions::default(),
        &fonts,
        &mut cache,
    )
    .unwrap();
    events.clear();
    let reported = sdocx::render_layout_pages_pdf_detailed_with_cache_and_progress(
        &document,
        &layout,
        &[0, 0],
        &options,
        &PdfOptions::default(),
        &fonts,
        &mut cache,
        &mut |event| events.push(event),
    )
    .unwrap();
    assert_eq!(normal.bytes, reported.bytes);
    assert_eq!(normal.pages, reported.pages);
    let counts = events
        .iter()
        .filter(|event| event.stage == ProgressStage::WritingPdf)
        .map(|event| (event.completed, event.total))
        .collect::<Vec<_>>();
    assert_eq!(counts, [(0, Some(2)), (1, Some(2)), (2, Some(2))]);
    assert_eq!(events.last().unwrap().stage, ProgressStage::Finalizing);
    assert_eq!(events.last().unwrap().total, None);
}
