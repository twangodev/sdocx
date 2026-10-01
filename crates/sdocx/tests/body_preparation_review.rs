#![cfg(feature = "render")]

use std::sync::Arc;

use sdocx::{
    BoundingBox, Color, Document, DocumentMetadata, LayoutDocument, Page, PageElement,
    PageObjectContent, RenderColorMode, RenderOptions, RenderedPage, RichTextBox,
    RichTextParagraph, RichTextParagraphType, RichTextSection, RichTextSpan, RichTextSpanType,
    SpanIntervalType,
};

fn document(source: &str, width: u32, height: u32, count: usize) -> Document {
    Document {
        pages: (0..count)
            .map(|index| Page {
                uuid: format!("body-preparation-{index}"),
                width,
                height,
                content_bbox: BoundingBox {
                    x_min: 0.0,
                    y_min: 0.0,
                    x_max: f64::from(width),
                    y_max: f64::from(height),
                },
                background_color: Some(if index % 2 == 0 {
                    Color {
                        r: 252,
                        g: 252,
                        b: 252,
                    }
                } else {
                    Color {
                        r: 37,
                        g: 37,
                        b: 37,
                    }
                }),
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
                text: source.into(),
                color: Some(Color { r: 0, g: 0, b: 0 }),
                highlight_color: None,
                underline: false,
                font_size: Some(10.0),
                runs: Vec::new(),
                spans: Vec::new(),
                paragraphs: Vec::new(),
                object_spans: Vec::new(),
                text_sections: Vec::new(),
                margins: None,
                gravity: None,
            }),
            default_page_dimensions: Some((360, height)),
            orientation: Some(0),
            page_mode: Some(0),
            flow_page_padding: Some((0, 0)),
            ..Default::default()
        },
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

fn custom_fonts(bundled: &sdocx::fonts::FontBook) -> sdocx::fonts::FontBook {
    let mut database = bundled.database();
    Arc::make_mut(&mut database)
        .load_font_data(include_bytes!("assets/fonts/DejaVuSans.ttf").to_vec());
    sdocx::fonts::FontBook::new(database)
}

fn use_custom_family(document: &mut Document) {
    let body = document.metadata.note_text.as_mut().unwrap();
    body.spans.push(RichTextSpan {
        kind: RichTextSpanType::FontName,
        start_utf16: 0,
        end_utf16: body.text.encode_utf16().count() as u32,
        interval_type: SpanIntervalType::ClosedOpen,
        payload: [vec![0; 8], vec![12, 0], b"DejaVu Sans\0".to_vec()].concat(),
    });
}

fn with_inspection_change(
    layout: &LayoutDocument,
    page_index: usize,
    replacement: Option<&str>,
) -> LayoutDocument {
    let mut changed = layout.clone();
    let object_index = changed.pages[page_index]
        .body_text_slice()
        .unwrap()
        .object_index;
    if let Some(source) = replacement {
        let PageObjectContent::Element(PageElement::TextBox(text)) =
            &mut changed.pages[page_index].page.objects[object_index].content
        else {
            panic!("expected inspection text")
        };
        text.text = source.into();
    } else {
        changed.pages[page_index].page.objects.remove(object_index);
    }
    changed
}

fn individual_pages(
    document: &Document,
    layout: &LayoutDocument,
    options: &RenderOptions,
    fonts: &sdocx::fonts::FontBook,
) -> Vec<RenderedPage> {
    (0..layout.pages.len())
        .map(|index| {
            sdocx::render_layout_page_svg_with_fonts(document, layout, index, options, fonts)
                .unwrap()
        })
        .collect()
}

fn assert_cache_matches_individual(
    cache: &mut sdocx::DocumentTextCache,
    document: &Document,
    fonts: &sdocx::fonts::FontBook,
) {
    let layout = sdocx::layout_document(document);
    for color_mode in [
        RenderColorMode::Auto,
        RenderColorMode::Light,
        RenderColorMode::Dark,
    ] {
        let mut options = RenderOptions::default();
        options.color_mode = color_mode;
        let individual = individual_pages(document, &layout, &options, fonts);
        assert_eq!(
            sdocx::render_document_svg_with_fonts(document, &options, fonts),
            individual,
            "{color_mode:?}"
        );
        for index in (0..individual.len()).rev().chain(0..individual.len()) {
            let cached = cache
                .render_layout_page_svg(document, &layout, index, &options, fonts)
                .unwrap();
            assert_eq!(
                cached, individual[index],
                "cached {color_mode:?} page {index}"
            );
            let cached_replay = cache
                .render_layout_page_replay_svg(document, &layout, index, &options, fonts)
                .unwrap();
            assert_eq!(
                cached_replay, individual[index],
                "cached replay {color_mode:?} page {index}"
            );
            let replay = sdocx::render_layout_page_replay_svg_with_fonts(
                document, &layout, index, &options, fonts,
            )
            .unwrap();
            assert_eq!(replay, individual[index], "{color_mode:?} page {index}");
        }
    }
}

fn assert_batch_matches_individual(document: &Document, fonts: &sdocx::fonts::FontBook) {
    assert_cache_matches_individual(&mut sdocx::DocumentTextCache::default(), document, fonts);
}

#[test]
fn full_body_batch_preserves_per_page_themes_and_padding() {
    let document = document("AAAABBBBCCCCDDDD", 30, 40, 4);
    assert_batch_matches_individual(&document, &sdocx::fonts::FontBook::default());
    let pages = sdocx::render_document_svg(&document, &Default::default());
    assert_eq!(
        pages
            .iter()
            .map(|page| source(&page.svg))
            .collect::<Vec<_>>(),
        ["AAAA", "BBBB", "CCCC", "DDDD"]
    );
}

#[test]
fn overlapping_saved_capture_windows_match_individual_page_plans() {
    let mut document = document("First\nSecond\n", 360, 40, 2);
    document.metadata.flow_page_padding = Some((10, 0));
    document.metadata.note_text.as_mut().unwrap().text_sections = vec![
        RichTextSection {
            start_utf16: 0,
            length_utf16: 6,
        },
        RichTextSection {
            start_utf16: 5,
            length_utf16: 8,
        },
    ];
    assert_batch_matches_individual(&document, &sdocx::fonts::FontBook::default());
    let pages = sdocx::render_document_svg(&document, &Default::default());
    assert_eq!(source(&pages[0].svg), "First");
    assert_eq!(source(&pages[1].svg), "Second");
}

#[test]
fn prepared_numbered_markers_follow_each_page_theme() {
    let mut document = document("one\ntwo\nthree\nfour", 360, 40, 4);
    document.metadata.note_text.as_mut().unwrap().paragraphs = (0..4)
        .map(|index| RichTextParagraph {
            kind: RichTextParagraphType::Bullet,
            start_paragraph: index,
            end_paragraph: index + 1,
            payload: [4_u32, index + 1, 0, 1]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect(),
        })
        .collect();
    assert_batch_matches_individual(&document, &sdocx::fonts::FontBook::default());
    let pages = sdocx::render_document_svg(&document, &Default::default());
    for (index, page) in pages.iter().enumerate() {
        let xml = roxmltree::Document::parse(&page.svg).unwrap();
        let marker = xml
            .descendants()
            .find(|node| {
                node.has_tag_name("tspan")
                    && node.text() == Some(format!("{}.", index + 1).as_str())
            })
            .unwrap();
        assert_eq!(
            marker.attribute("fill"),
            Some(if index % 2 == 0 { "#000000" } else { "#ffffff" })
        );
    }
}

#[test]
fn preparation_does_not_reuse_different_font_books() {
    let mut document = document("iiiiMMMMWWWWmmmm", 60, 40, 4);
    use_custom_family(&mut document);
    let bundled = sdocx::fonts::FontBook::default();
    let custom = custom_fonts(&bundled);
    let mut cache = sdocx::DocumentTextCache::default();
    assert_cache_matches_individual(&mut cache, &document, &bundled);
    assert_cache_matches_individual(&mut cache, &document, &custom);
    assert_cache_matches_individual(&mut cache, &document, &bundled);
    let fallback = sdocx::render_document_svg_with_fonts(&document, &Default::default(), &bundled);
    let resolved = sdocx::render_document_svg_with_fonts(&document, &Default::default(), &custom);
    assert_ne!(fallback[0].svg, resolved[0].svg);
    assert!(!fallback[0].text_diagnostics.is_empty());
    assert!(resolved.iter().all(|page| page.text_diagnostics.is_empty()));
}

#[test]
fn preparation_does_not_reuse_a_different_source_or_page_geometry() {
    let first = document("AAAABBBBCCCCDDDD", 30, 40, 4);
    let mut second = first.clone();
    second.metadata.note_text.as_mut().unwrap().text = "DDDDCCCCBBBBAAAA".into();
    second.pages[1].height = 55;
    second.pages[1].content_bbox.y_max = 55.0;
    let fonts = sdocx::fonts::FontBook::default();
    let mut cache = sdocx::DocumentTextCache::default();
    assert_cache_matches_individual(&mut cache, &first, &fonts);
    assert_cache_matches_individual(&mut cache, &second, &fonts);
    assert_cache_matches_individual(&mut cache, &first, &fonts);
    assert_eq!(
        source(&sdocx::render_document_svg_with_fonts(&second, &Default::default(), &fonts)[0].svg),
        "DDDD"
    );
}

#[test]
fn preparation_invalidates_changed_density_font_delta_and_flow_padding() {
    let original = document("AAAABBBBCCCCDDDD", 90, 80, 2);
    let mut density = original.clone();
    density.metadata.default_page_dimensions = Some((720, 80));
    let mut font_delta = original.clone();
    font_delta.metadata.body_font_size_delta = Some(5);
    let mut padding = original.clone();
    padding.metadata.flow_page_padding = Some((10, 0));
    let fonts = sdocx::fonts::FontBook::default();
    let original_svg =
        sdocx::render_document_svg_with_fonts(&original, &Default::default(), &fonts)[0]
            .svg
            .clone();
    let mut cache = sdocx::DocumentTextCache::default();
    assert_cache_matches_individual(&mut cache, &original, &fonts);
    for document in [&density, &font_delta, &padding, &original] {
        assert_cache_matches_individual(&mut cache, document, &fonts);
        if !std::ptr::eq(document, &original) {
            assert_ne!(
                sdocx::render_document_svg_with_fonts(document, &Default::default(), &fonts)[0].svg,
                original_svg
            );
        }
    }
}

#[test]
fn warm_preparation_retains_recovered_nan_source_and_its_diagnostics() {
    let mut document = document("AAAABBBBCCCCDDDD", 30, 40, 4);
    document.metadata.note_text.as_mut().unwrap().margins = Some([f32::NAN, 0.0, 0.0, 0.0]);
    let fonts = sdocx::fonts::FontBook::default();
    assert_cache_matches_individual(&mut sdocx::DocumentTextCache::default(), &document, &fonts);
    let pages = sdocx::render_document_svg_with_fonts(&document, &Default::default(), &fonts);
    assert_eq!(
        pages
            .iter()
            .map(|page| source(&page.svg))
            .collect::<String>(),
        "AAAABBBBCCCCDDDD"
    );
    assert!(pages.iter().any(|page| !page.text_diagnostics.is_empty()));
}

#[test]
fn warm_cache_never_restores_removed_or_edited_inspection_objects() {
    let document = document("AAAABBBBCCCCDDDD", 30, 40, 4);
    let layout = sdocx::layout_document(&document);
    let fonts = sdocx::fonts::FontBook::default();
    let options = RenderOptions::default();
    let mut cache = sdocx::DocumentTextCache::default();
    assert_cache_matches_individual(&mut cache, &document, &fonts);
    for replacement in [Some("Z"), None] {
        let changed = with_inspection_change(&layout, 1, replacement);
        for index in [1, 0, 1] {
            let page = cache
                .render_layout_page_svg(&document, &changed, index, &options, &fonts)
                .unwrap();
            assert_eq!(
                source(&page.svg),
                if index == 1 {
                    replacement.unwrap_or("")
                } else {
                    "AAAA"
                }
            );
        }
    }
    let restored = cache
        .render_layout_page_svg(&document, &layout, 1, &options, &fonts)
        .unwrap();
    assert_eq!(source(&restored.svg), "BBBB");
}

#[test]
fn changed_authoritative_body_does_not_reuse_an_old_layouts_cached_full_source() {
    let original = document("AAAABBBBCCCCDDDD", 30, 40, 4);
    let layout = sdocx::layout_document(&original);
    let fonts = sdocx::fonts::FontBook::default();
    let options = RenderOptions::default();
    let mut cache = sdocx::DocumentTextCache::default();
    let initial = cache
        .render_layout_page_svg(&original, &layout, 1, &options, &fonts)
        .unwrap();
    assert_eq!(source(&initial.svg), "BBBB");
    let mut changed_text = original.clone();
    changed_text.metadata.note_text.as_mut().unwrap().text = "DDDDCCCCBBBBAAAA".into();
    let mut changed_style = original.clone();
    changed_style.metadata.note_text.as_mut().unwrap().font_size = Some(20.0);
    for document in [&changed_text, &changed_style] {
        let page = cache
            .render_layout_page_svg(document, &layout, 1, &options, &fonts)
            .unwrap();
        assert_eq!(source(&page.svg), "");
    }
}

#[cfg(feature = "pdf")]
fn export(document: &Document, layout: &LayoutDocument, indices: &[usize]) -> lopdf::Document {
    let fonts = sdocx::fonts::FontBook::default();
    let bytes = sdocx::render_layout_pages_pdf_with_fonts(
        document,
        layout,
        indices,
        &Default::default(),
        &Default::default(),
        &fonts,
    )
    .unwrap();
    lopdf::Document::load_mem(&bytes).unwrap()
}

#[cfg(feature = "pdf")]
fn text_matrices(pdf: &lopdf::Document, page_index: u32) -> Vec<Vec<lopdf::Object>> {
    let page_id = pdf.get_pages()[&page_index];
    lopdf::content::Content::decode(&pdf.get_page_content(page_id).unwrap())
        .unwrap()
        .operations
        .into_iter()
        .filter(|operation| operation.operator == "Tm")
        .map(|operation| operation.operands)
        .collect()
}

#[cfg(feature = "pdf")]
#[test]
fn retained_body_pdf_keeps_selected_page_order_positions_and_repeated_source() {
    let document = document("AAAABBBBCCCCDDDD", 30, 40, 4);
    let layout = sdocx::layout_document(&document);
    let selected = [3, 1, 0, 2, 3];
    let batch = export(&document, &layout, &selected);
    assert_eq!(batch.get_pages().len(), selected.len());
    for (output_index, &source_index) in selected.iter().enumerate() {
        let single = export(&document, &layout, &[source_index]);
        let output_page = output_index as u32 + 1;
        assert_eq!(
            batch.extract_text(&[output_page]).unwrap(),
            single.extract_text(&[1]).unwrap()
        );
        assert_eq!(
            text_matrices(&batch, output_page),
            text_matrices(&single, 1)
        );
        assert_eq!(
            batch
                .extract_text(&[output_page])
                .unwrap()
                .replace(['\n', ' '], ""),
            ["AAAA", "BBBB", "CCCC", "DDDD"][source_index]
        );
    }
}

#[cfg(feature = "pdf")]
#[test]
fn pdf_reuses_preview_preparation_without_stale_fonts_or_theme() {
    let mut document = document("iiiiMMMMWWWWmmmm", 60, 40, 4);
    use_custom_family(&mut document);
    let layout = sdocx::layout_document(&document);
    let bundled = sdocx::fonts::FontBook::default();
    let custom = custom_fonts(&bundled);
    let selected = [3, 1, 0, 2, 3];
    let mut cache = sdocx::DocumentTextCache::default();
    for (fonts, color_mode) in [
        (&bundled, RenderColorMode::Auto),
        (&custom, RenderColorMode::Dark),
        (&bundled, RenderColorMode::Light),
    ] {
        let mut options = RenderOptions::default();
        options.color_mode = color_mode;
        for &index in &selected {
            cache
                .render_layout_page_svg(&document, &layout, index, &options, fonts)
                .unwrap();
        }
        let warm = sdocx::render_layout_pages_pdf_detailed_with_cache(
            &document,
            &layout,
            &selected,
            &options,
            &Default::default(),
            fonts,
            &mut cache,
        )
        .unwrap();
        let cold = sdocx::render_layout_pages_pdf_detailed_with_fonts(
            &document,
            &layout,
            &selected,
            &options,
            &Default::default(),
            fonts,
        )
        .unwrap();
        assert_eq!(warm.pages, cold.pages);
        let warm = lopdf::Document::load_mem(&warm.bytes).unwrap();
        let cold = lopdf::Document::load_mem(&cold.bytes).unwrap();
        for index in 1..=selected.len() as u32 {
            assert_eq!(
                warm.extract_text(&[index]).unwrap(),
                cold.extract_text(&[index]).unwrap()
            );
            assert_eq!(text_matrices(&warm, index), text_matrices(&cold, index));
        }
    }
}

#[cfg(feature = "pdf")]
#[test]
fn retained_body_pdf_never_restores_removed_or_edited_inspection_objects() {
    let document = document("AAAABBBBCCCCDDDD", 30, 40, 4);
    let layout = sdocx::layout_document(&document);
    for replacement in [Some("Z"), None] {
        let changed = with_inspection_change(&layout, 1, replacement);
        let pdf = export(&document, &changed, &[1, 0, 1]);
        for index in [1, 3] {
            assert_eq!(
                pdf.extract_text(&[index]).unwrap().replace(['\n', ' '], ""),
                replacement.unwrap_or("")
            );
        }
        assert_eq!(
            pdf.extract_text(&[2]).unwrap().replace(['\n', ' '], ""),
            "AAAA"
        );
    }
}
