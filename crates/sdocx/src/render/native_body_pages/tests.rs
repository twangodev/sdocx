use std::io::{Cursor, Write};

use super::*;
use crate::fonts::FontBook;
use crate::render::{DocumentTextCache, RenderOptions};
use sha2::{Digest, Sha256};

fn sized(bytes: &[u8]) -> Vec<u8> {
    [(bytes.len() as u32).to_le_bytes().as_slice(), bytes].concat()
}

fn utf16(source: &str) -> Vec<u8> {
    [
        (source.encode_utf16().count() as u16)
            .to_le_bytes()
            .to_vec(),
        source.encode_utf16().flat_map(u16::to_le_bytes).collect(),
    ]
    .concat()
}

fn frame(kind: u16, fixed: &[u8], flexible: &[u8], fields: u32) -> Vec<u8> {
    let offset = 18 + fixed.len();
    [
        ((offset + flexible.len()) as u32).to_le_bytes().to_vec(),
        kind.to_le_bytes().to_vec(),
        (offset as u32).to_le_bytes().to_vec(),
        vec![2],
        (if kind == 0 { 8_u16 } else { 0 }).to_le_bytes().to_vec(),
        vec![4],
        fields.to_le_bytes().to_vec(),
        fixed.to_vec(),
        flexible.to_vec(),
    ]
    .concat()
}

fn text_object(
    source: &str,
    width: u32,
    height: u32,
    font_span: bool,
    sections: &[[u32; 2]],
) -> Vec<u8> {
    let fixed = [
        5500_u32.to_le_bytes().to_vec(),
        utf16("text"),
        vec![0; 8],
        [0.0, 0.0, f64::from(width), f64::from(height)]
            .into_iter()
            .flat_map(f64::to_le_bytes)
            .collect(),
        vec![0; 5],
    ]
    .concat();
    let count = source.encode_utf16().count() as u32;
    let span = if font_span {
        [
            20_u16.to_le_bytes().to_vec(),
            [3_u32, 0, count, 1, 17.0_f32.to_bits()]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect(),
        ]
        .concat()
    } else {
        Vec::new()
    };
    let common = [
        count.to_le_bytes().to_vec(),
        source.encode_utf16().flat_map(u16::to_le_bytes).collect(),
        u32::from(font_span).to_le_bytes().to_vec(),
        span,
        0_u32.to_le_bytes().to_vec(),
        vec![0; 16 + 1],
        (sections.len() as u16).to_le_bytes().to_vec(),
        sections
            .iter()
            .flatten()
            .flat_map(|value| value.to_le_bytes())
            .collect(),
        vec![0; 8],
    ]
    .concat();
    [
        frame(0, &fixed, &0_f32.to_le_bytes(), 1),
        frame(6, &[], &[], 0),
        frame(7, &[], &sized(&common), 1),
    ]
    .concat()
}

fn page(id: &str, width: u32, height: u32) -> Vec<u8> {
    let mut header = [
        vec![0; 8],
        vec![1, 0, 5],
        vec![0; 5],
        [0_u32, width, height, 0, 0]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect(),
        utf16(id),
        vec![0; 8],
        [5500_u32, 4000]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect(),
    ]
    .concat();
    let length = header.len() as u32;
    header[..4].copy_from_slice(&length.to_le_bytes());
    header[4..8].copy_from_slice(&length.to_le_bytes());
    [
        header,
        [1_u16, 0].into_iter().flat_map(u16::to_le_bytes).collect(),
        20_u32.to_le_bytes().to_vec(),
        vec![0; 4],
        vec![2, 2, 0, 3, 0, 0, 0],
        vec![0; 5 + 4 + 32 + 32],
        b"Page for SAMSUNG S-Pen SDK".to_vec(),
    ]
    .concat()
}

fn parsed(case: &serde_json::Value, padding: u32, font_span: bool) -> Document {
    parsed_with_dimensions(case, padding, font_span, None, &[])
}

fn parsed_with_dimensions(
    case: &serde_json::Value,
    padding: u32,
    font_span: bool,
    default_dimensions: Option<(u32, u32)>,
    source_sections: &[[u32; 2]],
) -> Document {
    let input = &case["input"];
    let records = input["page_records"].as_array().unwrap();
    let width = records[0][3].as_u64().unwrap() as u32;
    let height = records[0][4].as_u64().unwrap() as u32;
    let body = text_object(
        input["source_utf8"].as_str().unwrap(),
        width,
        1000,
        font_span,
        source_sections,
    );
    let title = text_object("", 0, 0, false, &[]);
    let mut note = [
        vec![0; 4],
        vec![1, 0, 1, 0],
        5500_u32.to_le_bytes().to_vec(),
        utf16("body"),
        12_u32.to_le_bytes().to_vec(),
        vec![0; 16],
        [width, 1000, padding, 0, 4000]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect(),
        sized(&title),
        sized(&body),
    ]
    .concat();
    if let Some((width, height)) = default_dimensions {
        note.extend(width.to_le_bytes());
        note.extend(height.to_le_bytes());
    }
    let length = note.len() as u32;
    note[..4].copy_from_slice(&length.to_le_bytes());
    note.extend(utf16("Samsung Notes"));
    note.extend(Sha256::digest(&note));
    let tag = [
        5500_u32.to_le_bytes().to_vec(),
        utf16("body"),
        vec![0; 8],
        0_u32.to_le_bytes().to_vec(),
        utf16(""),
        width.to_le_bytes().to_vec(),
        (height as f32).to_le_bytes().to_vec(),
        utf16("Samsung Notes"),
        [4_u32, 4].into_iter().flat_map(u32::to_le_bytes).collect(),
        utf16(""),
        4000_u32.to_le_bytes().to_vec(),
        vec![0; 8 + 4 + 2],
        b"Document for S-Pen SDK".to_vec(),
    ]
    .concat();
    let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in [
        ("note.note", note),
        (
            "end_tag.bin",
            [(tag.len() as u16).to_le_bytes().to_vec(), tag].concat(),
        ),
    ] {
        archive
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        archive.write_all(&bytes).unwrap();
    }
    for index in 0..records.len() {
        let id = format!("body{index}");
        archive
            .start_file(
                format!("{id}.page"),
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        archive.write_all(&page(&id, width, height)).unwrap();
    }
    crate::parse_bytes(&archive.finish().unwrap().into_inner()).unwrap()
}

fn captures() -> Vec<serde_json::Value> {
    let capture: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../../../../conformance/table-bodytext-page-ranges.json"
    ))
    .unwrap();
    capture["cases"].as_array().unwrap().clone()
}

#[test]
fn parsed_no_band_profile_retains_native_line_and_source_sections() {
    let fonts = FontBook::default();
    let mut covered = 0;
    for case in captures().into_iter().filter(|case| {
        case["input"]["requested_font_size_bits"].is_null()
            && case["input"]["requested_margin_bits"].is_null()
    }) {
        let document = parsed(&case, 0, false);
        let original = serde_json::to_value(&document).unwrap();
        let source = document.metadata.note_text.as_ref().unwrap();
        assert_eq!(source.text, case["input"]["source_utf8"].as_str().unwrap());
        assert!(source.spans.is_empty());
        assert!(source.text_sections.is_empty());
        assert_eq!(document.metadata.default_page_dimensions, None);
        let layout = crate::layout_document(&document);
        let mut cache = DocumentTextCache::default();
        let options = RenderOptions::default();
        let cold = (0..layout.pages.len())
            .map(|index| {
                cache
                    .render_layout_page_svg(&document, &layout, index, &options, &fonts)
                    .unwrap()
                    .svg
            })
            .collect::<Vec<_>>();
        assert_eq!(cache.plans.len(), 1, "{}", case["name"]);
        let plan = &cache.plans[0].1;
        let pages = plan
            .native_pages
            .as_ref()
            .unwrap_or_else(|error| panic!("{}: {error:?}", case["name"]));
        let expected = &case["stages"][0];
        let section_pairs =
            |sections: &[super::super::text::native_page_index::NativePageSection]| {
                sections
                    .iter()
                    .map(|section| [section.start, section.count])
                    .collect::<Vec<_>>()
            };
        assert_eq!(
            serde_json::to_value(section_pairs(&pages.sections.lines)).unwrap(),
            expected["line_sections"],
            "{}: lines",
            case["name"]
        );
        assert_eq!(
            serde_json::to_value(section_pairs(&pages.sections.text)).unwrap(),
            expected["text_sections"],
            "{}: source",
            case["name"]
        );
        assert!(plan.layout.native_frame.is_none());
        for (index, svg) in cold.iter().enumerate() {
            let retained = cache
                .render_layout_page_svg(&document, &layout, index, &options, &fonts)
                .unwrap();
            let fresh = DocumentTextCache::default()
                .render_layout_page_svg(&document, &layout, index, &options, &fonts)
                .unwrap();
            assert_eq!(*svg, retained.svg);
            assert_eq!(*svg, fresh.svg);
        }
        assert_eq!(serde_json::to_value(&document).unwrap(), original);
        covered += 1;
    }
    assert_eq!(covered, 9);
}

#[test]
fn parsed_padding_and_font_spans_preserve_fallback_source() {
    let case = captures()
        .into_iter()
        .find(|case| case["name"] == "default-wide")
        .unwrap();
    let fonts = FontBook::default();
    for (padding, span) in [(1, false), (0, true)] {
        let document = parsed(&case, padding, span);
        let original = serde_json::to_value(&document).unwrap();
        let layout = crate::layout_document(&document);
        let mut cache = DocumentTextCache::default();
        let rendered = cache
            .render_layout_page_svg(&document, &layout, 0, &RenderOptions::default(), &fonts)
            .unwrap();
        assert!(!rendered.svg.is_empty());
        assert!(matches!(
            cache.plans[0].1.native_pages,
            Err(NativePageIndexUnavailable::OutsideCertificate)
        ));
        assert_eq!(serde_json::to_value(&document).unwrap(), original);
        assert_eq!(
            document
                .metadata
                .note_text
                .as_ref()
                .unwrap()
                .spans
                .is_empty(),
            !span
        );
    }
}

#[test]
fn parsed_default_page_dimensions_preserve_native_padding_bands() {
    let case = captures()
        .into_iter()
        .find(|case| case["name"] == "default-wide")
        .unwrap();
    let document = parsed_with_dimensions(&case, 0, false, Some((360, 20)), &[]);
    let original = serde_json::to_value(&document).unwrap();
    let layout = crate::layout_document(&document);
    let fonts = FontBook::default();
    let mut cache = DocumentTextCache::default();
    cache
        .render_layout_page_svg(&document, &layout, 0, &RenderOptions::default(), &fonts)
        .unwrap();
    let plan = &cache.plans[0].1;
    assert!(plan.exclusions.is_some());
    assert!(matches!(
        plan.native_pages,
        Err(NativePageIndexUnavailable::OutsideCertificate)
    ));
    assert_eq!(serde_json::to_value(&document).unwrap(), original);
}

#[test]
fn parsed_saved_page_sections_remain_separate_from_measured_page_ownership() {
    let case = captures()
        .into_iter()
        .find(|case| case["name"] == "default-wide")
        .unwrap();
    let sections = vec![[0, 6]; case["input"]["page_records"].as_array().unwrap().len()];
    let document = parsed_with_dimensions(&case, 0, false, None, &sections);
    let original = serde_json::to_value(&document).unwrap();
    let layout = crate::layout_document(&document);
    assert!(
        layout.pages[0]
            .body_text_slice()
            .unwrap()
            .capture_window
            .is_some()
    );
    let fonts = FontBook::default();
    let mut cache = DocumentTextCache::default();
    cache
        .render_layout_page_svg(&document, &layout, 0, &RenderOptions::default(), &fonts)
        .unwrap();
    assert!(matches!(
        cache.plans[0].1.native_pages,
        Err(NativePageIndexUnavailable::OutsideCertificate)
    ));
    let plan = &cache.plans[0].1;
    let settings = super::super::text::TextSettings::from_document(&document.metadata);
    let styled = super::super::text::StyledText::new(
        &plan.text,
        super::super::text::TextContext::Flow,
        settings,
    );
    let renderer = super::super::text::TextRenderer::new(settings, &fonts);
    let existing = super::super::text::layout_capture_text(
        &styled,
        super::super::text::TextFrame {
            bbox: crate::BoundingBox {
                x_min: 0.0,
                y_min: 0.0,
                x_max: 90.0,
                y_max: 0.0,
            },
            gravity: None,
            exclusions: &[],
        },
        super::super::RenderTheme::for_canvas(false),
        &renderer,
    );
    let positions = |layout: &super::super::text::TextLayout| {
        layout
            .lines
            .iter()
            .map(|line| {
                let metrics = [
                    line.x,
                    line.width,
                    line.baseline,
                    line.background_top,
                    line.bottom,
                ]
                .map(f64::to_bits);
                let clusters = line
                    .line
                    .placements
                    .iter()
                    .enumerate()
                    .map(|(index, placement)| {
                        let canonical = line.line.text_position(index, true);
                        (
                            placement.cluster.source.clone(),
                            [placement.x, placement.extra_advance].map(f64::to_bits),
                            canonical.map(|position| {
                                (
                                    [position.x, position.glyph_offset_x, position.extra_advance]
                                        .map(f64::to_bits),
                                    position.visual_rank,
                                )
                            }),
                        )
                    })
                    .collect::<Vec<_>>();
                (metrics, clusters)
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(positions(&plan.layout), positions(&existing));
    assert_eq!(
        document
            .metadata
            .note_text
            .as_ref()
            .unwrap()
            .text_sections
            .len(),
        sections.len()
    );
    assert_eq!(serde_json::to_value(&document).unwrap(), original);
}

#[cfg(feature = "pdf")]
#[test]
fn parsed_no_band_pdf_reuses_page_selection_and_keeps_empty_pages_empty() {
    let fonts = FontBook::default();
    for name in ["default-wide", "leading-newline", "empty-default"] {
        let case = captures()
            .into_iter()
            .find(|case| case["name"] == name)
            .unwrap();
        let document = parsed(&case, 0, false);
        let original = serde_json::to_value(&document).unwrap();
        let layout = crate::layout_document(&document);
        let mut cache = DocumentTextCache::default();
        let options = RenderOptions::default();
        cache
            .render_layout_page_svg(&document, &layout, 0, &options, &fonts)
            .unwrap();
        let indices = (0..layout.pages.len()).collect::<Vec<_>>();
        let pdf = crate::render_layout_pages_pdf_detailed_with_cache(
            &document,
            &layout,
            &indices,
            &options,
            &Default::default(),
            &fonts,
            &mut cache,
        )
        .unwrap();
        assert_eq!(cache.plans.len(), 1);
        let pdf = lopdf::Document::load_mem(&pdf.bytes).unwrap();
        assert_eq!(pdf.get_pages().len(), indices.len());
        let pages = cache.plans[0].1.native_pages.as_ref().unwrap();
        for (index, source_section) in pages.sections.text.iter().enumerate() {
            let text = pdf.extract_text(&[index as u32 + 1]).unwrap();
            if source_section.count == 0 {
                assert!(text.trim().is_empty(), "{name}: page {index}: {text:?}");
            }
        }
        assert_eq!(serde_json::to_value(&document).unwrap(), original);
    }
}
