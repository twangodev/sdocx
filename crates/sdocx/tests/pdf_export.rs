#![cfg(feature = "pdf")]

use std::sync::Arc;

use sdocx::{
    PdfError, PdfOptions, RenderedPage, render_layout_pages_pdf_with_fonts, render_svg_pages_pdf,
};

fn page(width: u32, height: u32, content: &str) -> RenderedPage {
    RenderedPage {
        source_page_index: 0,
        text_diagnostics: Vec::new(),
        object_diagnostics: Vec::new(),
        geometry_diagnostics: Vec::new(),
        width,
        height,
        svg: format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}">{content}</svg>"#
        ),
    }
}

fn no_fonts() -> PdfOptions {
    PdfOptions::new(Arc::new(sdocx::pdf::fontdb::Database::new()))
}

#[test]
fn pages_keep_order_dimensions_vectors_and_selectable_embedded_text() {
    let options = PdfOptions::default();
    let pages = [
        page(
            400,
            200,
            r#"<path d="M5 5 L100 50" stroke="red"/><text x="10" y="70" font-size="20" font-family="sans-serif">First page</text>"#,
        ),
        page(
            200,
            400,
            r#"<text x="10" y="70" font-size="20" font-family="sans-serif">Second page</text>"#,
        ),
    ];
    let bytes = render_svg_pages_pdf(&pages, &options).unwrap();
    let pdf = lopdf::Document::load_mem(&bytes).unwrap();
    let ids = pdf.get_pages();
    assert_eq!(ids.len(), 2);
    for ((_, id), expected) in ids.iter().zip([[300.0, 150.0], [150.0, 300.0]]) {
        let bounds = pdf
            .get_dictionary(*id)
            .unwrap()
            .get(b"MediaBox")
            .unwrap()
            .as_array()
            .unwrap();
        let dimensions: Vec<_> = bounds
            .iter()
            .map(|number| number.as_float().unwrap())
            .collect();
        assert_eq!(dimensions, [0.0, 0.0, expected[0], expected[1]]);
    }
    let first = pdf.extract_text(&[1]).unwrap().replace('\n', "");
    let second = pdf.extract_text(&[2]).unwrap().replace('\n', "");
    assert!(first.contains("First page"), "extracted: {first:?}");
    assert!(!first.contains("Second page"));
    assert!(second.contains("Second page"), "extracted: {second:?}");
    assert!(pdf.objects.values().any(|object| {
        object
            .as_dict()
            .is_ok_and(|dict| dict.has(b"FontFile2") || dict.has(b"FontFile3"))
    }));
    assert!(
        !pdf.objects
            .values()
            .any(|object| object.as_stream().is_ok_and(|stream| stream
                .dict
                .get(b"Subtype")
                .is_ok_and(|value| value.as_name().is_ok_and(|name| name == b"Image"))))
    );
    let content = pdf.get_page_content(ids[&1]).unwrap();
    let operations = lopdf::content::Content::decode(&content)
        .unwrap()
        .operations;
    assert!(
        operations.iter().any(|op| op.operator == "S"),
        "line stays a vector stroke"
    );
}

#[test]
fn scale_controls_physical_size_and_embedded_images_survive() {
    let mut options = no_fonts();
    options.dpi = 144.0;
    let image = r#"<image width="40" height="20" href="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC"/>"#;
    let bytes = render_svg_pages_pdf(&[page(200, 100, image)], &options).unwrap();
    let pdf = lopdf::Document::load_mem(&bytes).unwrap();
    let id = pdf.get_pages()[&1];
    let bounds = pdf
        .get_dictionary(id)
        .unwrap()
        .get(b"MediaBox")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(bounds[2].as_float().unwrap(), 100.0);
    assert_eq!(bounds[3].as_float().unwrap(), 50.0);
    assert!(
        pdf.objects
            .values()
            .any(|object| object.as_stream().is_ok_and(|stream| stream
                .dict
                .get(b"Subtype")
                .is_ok_and(|value| value.as_name().is_ok_and(|name| name == b"Image"))))
    );
}

#[test]
fn invalid_input_returns_errors_instead_of_a_partial_pdf() {
    let mut options = no_fonts();
    assert!(matches!(
        render_svg_pages_pdf(&[], &options),
        Err(PdfError::EmptyDocument)
    ));
    for dpi in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        options.dpi = dpi;
        assert!(matches!(
            render_svg_pages_pdf(&[page(100, 100, "")], &options),
            Err(PdfError::InvalidDpi)
        ));
    }
    options.dpi = 96.0;
    for (width, height) in [(0, 100), (100, 0), (u32::MAX, 100)] {
        assert!(matches!(
            render_svg_pages_pdf(&[page(width, height, "")], &options),
            Err(PdfError::InvalidPageSize { page_index: 0 })
        ));
    }
    let mut broken = page(100, 100, "");
    broken.svg = "invalid SVG".into();
    assert!(matches!(
        render_svg_pages_pdf(&[page(100, 100, ""), broken], &options),
        Err(PdfError::InvalidSvg { page_index: 1, .. })
    ));
    let mut mismatched = page(100, 100, "");
    mismatched.width = 200;
    assert!(matches!(
        render_svg_pages_pdf(&[mismatched], &options),
        Err(PdfError::InvalidPageSize { page_index: 0 })
    ));
}

#[test]
fn corrupt_png_returns_an_error_before_the_converter_can_panic() {
    let image = r#"<image width="40" height="20" href="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aV1cAAAAASUVORK5CYII="/>"#;
    assert!(matches!(
        render_svg_pages_pdf(&[page(200, 100, image)], &no_fonts()),
        Err(PdfError::InvalidImage { page_index: 0, .. })
    ));
}

#[test]
fn document_export_uses_visible_layout_and_color_options() {
    let page = sdocx::Page {
        uuid: "page".into(),
        width: 200,
        height: 100,
        content_bbox: Default::default(),
        background_color: Some(sdocx::Color {
            r: 252,
            g: 252,
            b: 252,
        }),
        template: None,
        background: Default::default(),
        objects: vec![],
    };
    let document = sdocx::Document {
        pages: vec![page.clone(), page],
        metadata: sdocx::DocumentMetadata {
            page_mode: Some(0),
            flow_dimensions: Some((200, 200)),
            flow_page_padding: Some((0, 0)),
            note_text: Some(sdocx::RichTextBox {
                text_area_type: None,
                bbox: Default::default(),
                rotation_degrees: None,
                text: "AV".into(),
                color: None,
                highlight_color: None,
                underline: false,
                font_size: Some(45.0),
                runs: vec![],
                spans: vec![],
                paragraphs: vec![],
                object_spans: vec![],
                text_sections: vec![],
                margins: None,
                gravity: None,
            }),
            ..Default::default()
        },
    };
    let mut render_options = sdocx::RenderOptions::default();
    render_options.color_mode = sdocx::RenderColorMode::Dark;
    let options = PdfOptions::default();
    let bytes = sdocx::render_document_pdf(&document, &render_options, &options).unwrap();
    let pdf = lopdf::Document::load_mem(&bytes).unwrap();
    assert_eq!(pdf.get_pages().len(), 1, "omit trailing storage page");
    let svg_bytes = render_svg_pages_pdf(
        &sdocx::render_document_svg(&document, &render_options),
        &options,
    )
    .unwrap();
    for bytes in [&bytes, &svg_bytes] {
        let pdf = lopdf::Document::load_mem(bytes).unwrap();
        let page_id = pdf.get_pages()[&1];
        assert_eq!(page_dimensions(&pdf, page_id), [150.0, 75.0]);
        assert_eq!(
            pdf.extract_text(&[1]).unwrap().replace('\n', "").trim(),
            "AV"
        );
        assert!(
            !pdf.objects
                .values()
                .any(|object| object.as_stream().is_ok_and(|stream| stream
                    .dict
                    .get(b"Subtype")
                    .is_ok_and(|kind| kind.as_name().is_ok_and(|name| name == b"Image"))))
        );
        let operations = lopdf::content::Content::decode(&pdf.get_page_content(page_id).unwrap())
            .unwrap()
            .operations;
        let colors = operations
            .iter()
            .filter(|op| matches!(op.operator.as_str(), "scn" | "rg" | "g"))
            .map(|op| {
                op.operands
                    .iter()
                    .map(|value| value.as_float().unwrap())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        for expected in [37.0 / 255.0, 217.0 / 255.0] {
            assert!(
                colors.iter().any(|color| matches!(color.len(), 1 | 3)
                    && color
                        .iter()
                        .all(|&value| (value - expected).abs() < 0.00001)),
                "colors: {colors:?}"
            );
        }
        let glyphs = pdf_glyph_origins(&pdf, page_id, 96.0);
        assert_eq!(glyphs.len(), 2);
        assert_eq!(
            glyphs
                .iter()
                .map(|glyph| glyph.0.as_str())
                .collect::<String>(),
            "AV"
        );
        for ((source, x, y), (expected_source, expected_x)) in
            glyphs.iter().zip([("A", 0.0), ("V", 27.44384765625)])
        {
            assert_eq!(source, expected_source);
            assert!((x - expected_x).abs() < 0.001, "{glyphs:?}");
            assert!((y - 45.0).abs() < 0.001, "{glyphs:?}");
        }
    }
}

fn page_dimensions(pdf: &lopdf::Document, id: lopdf::ObjectId) -> [f32; 2] {
    let bounds = pdf
        .get_dictionary(id)
        .unwrap()
        .get(b"MediaBox")
        .unwrap()
        .as_array()
        .unwrap();
    [bounds[2].as_float().unwrap(), bounds[3].as_float().unwrap()]
}

fn pdf_glyph_origins(
    pdf: &lopdf::Document,
    page_id: lopdf::ObjectId,
    dpi: f64,
) -> Vec<(String, f64, f64)> {
    use lopdf::Object;
    use svgtypes::Transform;
    let number = |value: &Object| f64::from(value.as_float().unwrap());
    let matrix = |values: &[Object]| {
        Transform::new(
            number(&values[0]),
            number(&values[1]),
            number(&values[2]),
            number(&values[3]),
            number(&values[4]),
            number(&values[5]),
        )
    };
    let compose = |a: Transform, b: Transform| {
        Transform::new(
            a.a * b.a + a.c * b.b,
            a.b * b.a + a.d * b.b,
            a.a * b.c + a.c * b.d,
            a.b * b.c + a.d * b.d,
            a.a * b.e + a.c * b.f + a.e,
            a.b * b.e + a.d * b.f + a.f,
        )
    };
    let fonts = pdf.get_page_fonts(page_id).unwrap();
    let height = f64::from(page_dimensions(pdf, page_id)[1]);
    let mut ctm = Transform::default();
    let mut stack = Vec::new();
    let mut tm = Transform::default();
    let mut font_name = Vec::new();
    let mut size = 0.0;
    let mut cursor = 0.0;
    let mut output = Vec::new();
    for operation in lopdf::content::Content::decode(&pdf.get_page_content(page_id).unwrap())
        .unwrap()
        .operations
    {
        match operation.operator.as_str() {
            "q" => stack.push(ctm),
            "Q" => ctm = stack.pop().unwrap(),
            "cm" => ctm = compose(ctm, matrix(&operation.operands)),
            "Tm" => {
                tm = matrix(&operation.operands);
                cursor = 0.0;
            }
            "Tf" => {
                font_name = operation.operands[0].as_name().unwrap().to_vec();
                size = number(&operation.operands[1]);
            }
            "Tj" | "TJ" => {
                let font = fonts[&font_name];
                let encoding = font.get_font_encoding(pdf).unwrap();
                let descendant = pdf
                    .dereference(&font.get(b"DescendantFonts").unwrap().as_array().unwrap()[0])
                    .unwrap()
                    .1
                    .as_dict()
                    .unwrap();
                let widths = descendant.get(b"W").unwrap().as_array().unwrap();
                let values = operation.operands[0]
                    .as_array()
                    .map_or(operation.operands.as_slice(), Vec::as_slice);
                for value in values {
                    if let Object::String(bytes, _) = value {
                        let (cids, remainder) = bytes.as_chunks::<2>();
                        assert!(remainder.is_empty());
                        for cid_bytes in cids {
                            let cid = i64::from(u16::from_be_bytes(*cid_bytes));
                            let mut width = None;
                            let mut index = 0;
                            while index < widths.len() {
                                let start = widths[index].as_i64().unwrap();
                                if let Ok(entries) = widths[index + 1].as_array() {
                                    if cid >= start
                                        && let Some(entry) = entries.get((cid - start) as usize)
                                    {
                                        width = Some(number(entry));
                                    }
                                    index += 2;
                                } else {
                                    let end = widths[index + 1].as_i64().unwrap();
                                    if (start..=end).contains(&cid) {
                                        width = Some(number(&widths[index + 2]));
                                    }
                                    index += 3;
                                }
                            }
                            let transform = compose(ctm, tm);
                            output.push((
                                lopdf::Document::decode_text(&encoding, cid_bytes).unwrap(),
                                (transform.a * cursor + transform.e) * dpi / 72.0,
                                (height - transform.b * cursor - transform.f) * dpi / 72.0,
                            ));
                            cursor += width.expect("embedded glyph width") * size / 1000.0;
                        }
                    } else {
                        cursor -= number(value) * size / 1000.0;
                    }
                }
            }
            _ => {}
        }
    }
    output
}

fn selected_document() -> sdocx::Document {
    sdocx::Document {
        metadata: Default::default(),
        pages: [(200, 100, 10_u8), (300, 200, 70), (100, 300, 130)]
            .into_iter()
            .enumerate()
            .map(|(index, (width, height, red))| sdocx::Page {
                uuid: format!("selected-{index}"),
                width,
                height,
                content_bbox: Default::default(),
                background_color: Some(sdocx::Color {
                    r: red,
                    g: 20,
                    b: 30,
                }),
                template: None,
                background: Default::default(),
                objects: Vec::new(),
            })
            .collect(),
    }
}

#[test]
fn selected_layout_pages_preserve_requested_order_and_duplicates() {
    let document = selected_document();
    let layout = sdocx::layout_document(&document);
    let fonts = sdocx::fonts::FontBook::default();
    let mut options = PdfOptions::default();
    options.dpi = 144.0;
    let bytes = render_layout_pages_pdf_with_fonts(
        &document,
        &layout,
        &[2, 0, 2, 1],
        &Default::default(),
        &options,
        &fonts,
    )
    .unwrap();
    let pdf = lopdf::Document::load_mem(&bytes).unwrap();
    let pages = pdf.get_pages();
    assert_eq!(pages.len(), 4);
    for ((_, &page_id), expected) in
        pages
            .iter()
            .zip([[50.0, 150.0], [100.0, 50.0], [50.0, 150.0], [150.0, 100.0]])
    {
        assert_eq!(page_dimensions(&pdf, page_id), expected);
    }
    assert_eq!(
        pdf.get_page_content(pages[&1]).unwrap(),
        pdf.get_page_content(pages[&3]).unwrap(),
        "duplicate selection keeps page artwork"
    );
}

#[test]
fn selected_layout_pages_reject_empty_indices_invalid_indices_and_dpi() {
    let document = selected_document();
    let layout = sdocx::layout_document(&document);
    let fonts = sdocx::fonts::FontBook::default();
    let mut options = PdfOptions::default();
    assert!(matches!(
        render_layout_pages_pdf_with_fonts(
            &document,
            &layout,
            &[],
            &Default::default(),
            &options,
            &fonts
        ),
        Err(PdfError::EmptyDocument)
    ));
    for indices in [&[3][..], &[0, 3][..], &[usize::MAX][..]] {
        assert!(
            matches!(render_layout_pages_pdf_with_fonts(&document, &layout, indices, &Default::default(), &options, &fonts), Err(PdfError::InvalidPageIndex { page_index }) if page_index == *indices.last().unwrap())
        );
    }
    for dpi in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        options.dpi = dpi;
        assert!(matches!(
            render_layout_pages_pdf_with_fonts(
                &document,
                &layout,
                &[0],
                &Default::default(),
                &options,
                &fonts
            ),
            Err(PdfError::InvalidDpi)
        ));
    }
}

fn image_document(mime_type: &str, data: Vec<u8>) -> sdocx::Document {
    let blank = sdocx::Page {
        uuid: "pdf-image".into(),
        width: 100,
        height: 100,
        content_bbox: Default::default(),
        background_color: None,
        template: None,
        background: Default::default(),
        objects: vec![],
    };
    let mut image = blank.clone();
    image.objects.push(
        sdocx::PageElement::Image {
            bbox: sdocx::BoundingBox {
                x_min: 10.0,
                y_min: 10.0,
                x_max: 50.0,
                y_max: 50.0,
            },
            media_index: 0,
        }
        .into(),
    );
    sdocx::Document {
        pages: vec![blank, image],
        metadata: sdocx::DocumentMetadata {
            media_assets: vec![sdocx::MediaAsset {
                name: "media/image".into(),
                archive_id: None,
                mime_type: mime_type.into(),
                data,
            }],
            ..Default::default()
        },
    }
}

#[test]
fn rejected_native_image_headers_report_output_page_order() {
    let fonts = sdocx::fonts::FontBook::default();
    for (mime, data) in [
        ("image/jpeg", vec![0xff, 0xd8]),
        ("image/webp", b"RIFF\x04\0\0\0WEBP".to_vec()),
    ] {
        let document = image_document(mime, data);
        let layout = sdocx::layout_document(&document);
        assert_eq!(layout.pages.len(), 2);
        let result = render_layout_pages_pdf_with_fonts(
            &document,
            &layout,
            &[0, 0, 1],
            &Default::default(),
            &no_fonts(),
            &fonts,
        );
        assert!(
            matches!(result, Err(PdfError::InvalidImage { page_index: 2, .. })),
            "{mime}: {result:?}"
        );
        let svg_pages = sdocx::render_document_svg(&document, &Default::default());
        assert!(
            render_svg_pages_pdf(&svg_pages, &no_fonts()).is_ok(),
            "arbitrary SVG keeps its compatibility policy"
        );
    }
}

#[test]
fn native_png_jpeg_and_webp_keep_images_and_vector_backgrounds() {
    use base64::Engine;

    // JPEG and lossless WebP: FFmpeg 7.1.5 encodings of a one-pixel red RGB image.
    for (mime, encoded) in [
        (
            "image/png",
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC",
        ),
        (
            "image/jpeg",
            "/9j//gAQTGF2YzYxLjE5LjEwMQD/2wBDAAgEBAQEBAUFBQUFBQYGBgYGBgYGBgYGBgYHBwcICAgHBwcGBgcHCAgICAkJCQgICAgJCQoKCgwMCwsODg4RERT/xABNAAEBAAAAAAAAAAAAAAAAAAAABgEBAQEAAAAAAAAAAAAAAAAAAAYHEAEAAAAAAAAAAAAAAAAAAAAAEQEAAAAAAAAAAAAAAAAAAAAA/8AAEQgAAQABAwESAAISAAMSAP/aAAwDAQACEQMRAD8AixKDfx//2Q==",
        ),
        (
            "image/webp",
            "UklGRhwAAABXRUJQVlA4TA8AAAAvAAAAAAcQ/Y/+ByKi/wEA",
        ),
    ] {
        let data = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .unwrap();
        let mut document = image_document(mime, data);
        document.pages[1].background_color = Some(sdocx::Color {
            r: 255,
            g: 255,
            b: 255,
        });
        let bytes =
            sdocx::render_document_pdf(&document, &Default::default(), &no_fonts()).unwrap();
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        assert!(
            pdf.objects
                .values()
                .any(|object| object.as_stream().is_ok_and(|stream| stream
                    .dict
                    .get(b"Subtype")
                    .is_ok_and(|value| value.as_name().is_ok_and(|name| name == b"Image")))),
            "{mime}: missing image"
        );
        let page_id = pdf.get_pages()[&2];
        let content =
            lopdf::content::Content::decode(&pdf.get_page_content(page_id).unwrap()).unwrap();
        assert!(
            content
                .operations
                .iter()
                .any(|operation| operation.operator == "f"),
            "{mime}: missing vector background"
        );
    }
}
