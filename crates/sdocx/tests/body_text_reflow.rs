#![cfg(feature = "render")]

use sdocx::{
    BoundingBox, Document, DocumentMetadata, LayoutDocument, Page, PageElement, PageObjectContent,
    RichTextBox, RichTextSection,
};

const SOURCE: &str = "AAAAAAAAAAAAAAAA";

fn document(sections: &[(i32, i32)]) -> Document {
    Document {
        pages: (0..4)
            .map(|index| Page {
                uuid: format!("reflow-{index}"),
                width: 30,
                height: 40,
                content_bbox: BoundingBox {
                    x_min: 0.0,
                    y_min: 0.0,
                    x_max: 30.0,
                    y_max: 40.0,
                },
                background_color: None,
                template: None,
                background: Default::default(),
                objects: Vec::new(),
            })
            .collect(),
        metadata: DocumentMetadata {
            note_text: Some(RichTextBox {
                text_area_type: None,
                bbox: BoundingBox::default(),
                rotation_degrees: None,
                text: SOURCE.into(),
                color: None,
                highlight_color: None,
                underline: false,
                font_size: Some(10.0),
                runs: Vec::new(),
                spans: Vec::new(),
                paragraphs: Vec::new(),
                object_spans: Vec::new(),
                text_sections: sections
                    .iter()
                    .map(|&(start_utf16, length_utf16)| RichTextSection {
                        start_utf16,
                        length_utf16,
                    })
                    .collect(),
                margins: None,
                gravity: None,
            }),
            default_page_dimensions: Some((360, 40)),
            orientation: Some(0),
            page_mode: Some(0),
            flow_page_padding: Some((0, 0)),
            ..Default::default()
        },
    }
}

fn render(
    doc: &Document,
    layout: &LayoutDocument,
    page: usize,
    replay: bool,
    fonts: &sdocx::fonts::FontBook,
) -> sdocx::RenderedPage {
    if replay {
        sdocx::render_layout_page_replay_svg_with_fonts(
            doc,
            layout,
            page,
            &Default::default(),
            fonts,
        )
        .unwrap()
    } else {
        sdocx::render_layout_page_svg_with_fonts(doc, layout, page, &Default::default(), fonts)
            .unwrap()
    }
}

fn source(svg: &str) -> String {
    roxmltree::Document::parse(svg)
        .unwrap()
        .descendants()
        .filter(|node| node.has_tag_name("tspan"))
        .filter_map(|node| node.text())
        .collect()
}

fn assert_page(page: &sdocx::RenderedPage, physical_index: usize) {
    assert_page_source(page, physical_index, "AAAA");
}

fn assert_page_source(page: &sdocx::RenderedPage, physical_index: usize, expected: &str) {
    assert_eq!(page.source_page_index, physical_index);
    assert_eq!(source(&page.svg), expected);
    assert!(
        page.text_diagnostics.is_empty(),
        "{:?}",
        page.text_diagnostics
    );
    let xml = roxmltree::Document::parse(&page.svg).unwrap();
    let spans: Vec<_> = xml
        .descendants()
        .filter(|node| node.has_tag_name("tspan"))
        .collect();
    assert_eq!(spans.len(), usize::from(!expected.is_empty()));
    if expected.is_empty() {
        return;
    }
    let span = spans[0];
    assert_eq!(
        span.attribute("font-size").unwrap().parse::<f64>().unwrap(),
        10.0
    );
    let coordinate = |attribute| {
        span.attribute(attribute)
            .or_else(|| span.parent().unwrap().attribute(attribute))
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .parse::<f64>()
            .unwrap()
    };
    let mut point = (coordinate("x"), coordinate("y"));
    for ancestor in span.ancestors() {
        if let Some(transform) = ancestor.attribute("transform") {
            let transform: svgtypes::Transform = transform.parse().unwrap();
            point = (
                transform.a * point.0 + transform.c * point.1 + transform.e,
                transform.b * point.0 + transform.d * point.1 + transform.f,
            );
        }
    }
    // Four native 13.5px lines cross successive ±10px bands on 40px pages:
    // global baselines 20/60/100/140 become the same local baseline 20.
    assert_eq!(point, (0.0, 20.0));
}

#[test]
fn full_source_reflows_when_balanced_inspection_pages_are_empty() {
    let face =
        rustybuzz::ttf_parser::Face::parse(include_bytes!("../assets/fonts/Roboto-Regular.ttf"), 0)
            .unwrap();
    assert_eq!(face.units_per_em(), 2048);
    assert_eq!(
        face.glyph_hor_advance(face.glyph_index('A').unwrap()),
        Some(1336)
    );
    // The pinned upstream hmtx gives 6.5234375px per A at F10: four letters
    // occupy 26.09375px, while five exceed the physical 30px page width.
    let doc = document(&[]);
    let layout = sdocx::layout_document(&doc);
    assert_eq!(layout.pages.len(), 4);
    let fonts = sdocx::fonts::FontBook::default();
    for (index, page) in layout.pages.iter().enumerate() {
        let metadata = page.body_text_slice().unwrap();
        assert!(metadata.capture_window.is_none());
        assert_eq!(
            metadata.source_range,
            if index == 0 { 0..16 } else { 16..16 }
        );
        let reflow = metadata.reflow.as_ref().unwrap();
        assert_eq!(reflow.source_range, 0..16);
        assert_eq!(reflow.first_page_index, 0);
        assert_eq!(reflow.requested_page_index, index);
        assert_eq!(page.body_text_reflow(&doc).unwrap().text, SOURCE);
        let PageObjectContent::Element(PageElement::TextBox(inspection)) =
            &page.page.objects[metadata.object_index].content
        else {
            panic!()
        };
        assert_eq!(inspection.text, if index == 0 { SOURCE } else { "" });
    }
    for replay in [false, true] {
        let expected: Vec<_> = (0..4)
            .map(|index| render(&doc, &layout, index, replay, &fonts))
            .collect();
        for index in [3, 1, 0, 2, 3, 0] {
            let page = render(&doc, &layout, index, replay, &fonts);
            assert_page(&page, index);
            assert_eq!(page, expected[index]);
        }
        assert_eq!(
            expected
                .iter()
                .map(|page| source(&page.svg))
                .collect::<String>(),
            SOURCE
        );
    }
}

#[test]
fn unusable_saved_sections_fall_back_to_full_source_measured_pagination() {
    for sections in [
        vec![(0, 16)],
        vec![(0, 0); 4],
        vec![(-1, 16), (0, 16), (0, 16), (0, 16)],
        vec![(0, 17), (0, 16), (0, 16), (0, 16)],
        vec![(8, 8), (0, 16), (0, 16), (0, 16)],
    ] {
        let doc = document(&sections);
        let layout = sdocx::layout_document(&doc);
        let fonts = sdocx::fonts::FontBook::default();
        for replay in [false, true] {
            for index in 0..4 {
                assert!(
                    layout.pages[index].body_text_reflow(&doc).is_some(),
                    "{sections:?}"
                );
                assert_page(&render(&doc, &layout, index, replay, &fonts), index);
            }
        }
    }
}

#[test]
fn edited_or_removed_inspection_objects_disable_stale_full_source_reflow() {
    let doc = document(&[]);
    let layout = sdocx::layout_document(&doc);
    let mut edited = layout.clone();
    let object_index = edited.pages[1].body_text_slice().unwrap().object_index;
    let PageObjectContent::Element(PageElement::TextBox(inspection)) =
        &mut edited.pages[1].page.objects[object_index].content
    else {
        panic!()
    };
    inspection.text = "Edited".into();
    assert!(edited.pages[1].body_text_reflow(&doc).is_none());
    let mut removed = layout.clone();
    removed.pages[1].page.objects.clear();
    assert!(removed.pages[1].body_text_reflow(&doc).is_none());
    let fonts = sdocx::fonts::FontBook::default();
    for replay in [false, true] {
        assert_eq!(
            source(&render(&doc, &edited, 1, replay, &fonts).svg),
            "Edited"
        );
        assert_eq!(source(&render(&doc, &removed, 1, replay, &fonts).svg), "");
    }
}

#[test]
fn stale_authoritative_text_or_style_is_not_restored_through_old_reflow_metadata() {
    let original = document(&[]);
    let layout = sdocx::layout_document(&original);
    let mut changed_text = original.clone();
    changed_text.metadata.note_text.as_mut().unwrap().text = "BBBBBBBBBBBBBBBB".into();
    let mut changed_style = original.clone();
    changed_style.metadata.note_text.as_mut().unwrap().font_size = Some(20.0);
    let fonts = sdocx::fonts::FontBook::default();
    for doc in [changed_text, changed_style] {
        assert!(layout.pages[1].body_text_reflow(&doc).is_none());
        for replay in [false, true] {
            assert_eq!(source(&render(&doc, &layout, 1, replay, &fonts).svg), "");
        }
    }
}

#[test]
fn debugger_presentation_clone_keeps_the_original_physical_reflow_page() {
    let doc = document(&[]);
    let layout = sdocx::layout_document(&doc);
    let mut debugger = layout.clone();
    debugger.pages = vec![layout.pages[2].clone()];
    let fonts = sdocx::fonts::FontBook::default();
    for replay in [false, true] {
        let page = render(&doc, &debugger, 0, replay, &fonts);
        assert_page(&page, 2);
        assert_eq!(page, render(&doc, &layout, 2, replay, &fonts));
    }
}

#[test]
fn physical_maximum_width_is_used_consistently_for_all_reflow_pages() {
    let mut doc = document(&[]);
    for (page, width) in doc.pages.iter_mut().zip([30, 40, 50, 60]) {
        page.width = width;
        page.content_bbox.x_max = f64::from(width);
    }
    let layout = sdocx::layout_document(&doc);
    let fonts = sdocx::fonts::FontBook::default();
    // Native GetPageMaxWidth measures at 60px: nine A's occupy 58.7109375px,
    // while ten occupy 65.234375px. Each SVG retains its own viewport width.
    let expected_sources = ["AAAAAAAAA", "AAAAAAA", "", ""];
    for replay in [false, true] {
        let expected: Vec<_> = (0..4)
            .map(|index| render(&doc, &layout, index, replay, &fonts))
            .collect();
        for index in [3, 0, 2, 1, 3] {
            let page = render(&doc, &layout, index, replay, &fonts);
            assert_page_source(&page, index, expected_sources[index]);
            assert_eq!(page.width, doc.pages[index].width);
            assert_eq!(page, expected[index]);
        }
        let mut projection = layout.clone();
        projection.pages[1].page.width = 90;
        projection.pages[1].page.content_bbox.x_max = 90.0;
        let projected = render(&doc, &projection, 1, replay, &fonts);
        assert_page_source(&projected, 1, "AAAAAAA");
        assert_eq!(projected.width, 90);
        assert_eq!(source(&projected.svg), source(&expected[1].svg));
    }
}

#[cfg(feature = "pdf")]
#[test]
fn measured_reflow_pdf_keeps_each_page_selectable_without_hidden_source_duplicates() {
    let doc = document(&[]);
    let layout = sdocx::layout_document(&doc);
    let fonts = sdocx::fonts::FontBook::default();
    for replay in [false, true] {
        let pages: Vec<_> = (0..4)
            .map(|index| render(&doc, &layout, index, replay, &fonts))
            .collect();
        for (index, page) in pages.iter().enumerate() {
            assert_page(page, index);
        }
        let bytes = sdocx::render_svg_pages_pdf(&pages, &Default::default()).unwrap();
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        assert_eq!(pdf.get_pages().len(), 4);
        for index in 1..=4 {
            assert_eq!(
                pdf.extract_text(&[index]).unwrap().replace(['\n', ' '], ""),
                "AAAA"
            );
        }
        assert_eq!(
            pdf.extract_text(&[1, 2, 3, 4])
                .unwrap()
                .replace(['\n', ' '], ""),
            SOURCE
        );
        assert!(
            !pdf.objects
                .values()
                .any(|object| object.as_stream().is_ok_and(|stream| stream
                    .dict
                    .get(b"Subtype")
                    .is_ok_and(|value| value.as_name().is_ok_and(|name| name == b"Image"))))
        );
    }
}
