use std::{ops::Range, path::PathBuf};

use sdocx::{PageElement, PageObjectContent, RichTextObjectContent, RichTextSpanType};
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
struct Corpus {
    version: u32,
    fixtures: Vec<Fixture>,
}

#[derive(Deserialize)]
struct Fixture {
    id: String,
    sdocx: Asset,
    reference_pdf: Asset,
}

#[derive(Deserialize)]
struct Asset {
    path: PathBuf,
    sha256: String,
}

fn utf16_offset(source: &str, scalar_offset: usize) -> usize {
    assert!(scalar_offset <= source.chars().count());
    source
        .chars()
        .take(scalar_offset)
        .map(char::len_utf16)
        .sum()
}

fn utf16_range(source: &str, scalars: &Range<usize>) -> Range<usize> {
    utf16_offset(source, scalars.start)..utf16_offset(source, scalars.end)
}

#[test]
#[ignore = "requires the external Hugging Face compatibility corpus"]
fn native_last_page_capture_retains_the_overlapping_section_and_code_separator_style() {
    let corpus: Corpus =
        serde_json::from_str(include_str!("../../../conformance/corpus.json")).unwrap();
    assert_eq!(corpus.version, 1);
    let fixture = corpus
        .fixtures
        .into_iter()
        .find(|fixture| fixture.id == "01-basic-formatting")
        .unwrap();
    let root = std::env::var_os("SDOCX_CORPUS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../hf"));
    let source = std::fs::read(root.join(fixture.sdocx.path)).unwrap();
    let pdf = std::fs::read(root.join(fixture.reference_pdf.path)).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(&source)),
        fixture.sdocx.sha256
    );
    assert_eq!(
        format!("{:x}", Sha256::digest(&pdf)),
        fixture.reference_pdf.sha256
    );
    let document = sdocx::parse_bytes(&source).unwrap();
    let body = document.metadata.note_text.as_ref().unwrap();
    let full_utf16: Vec<_> = body.text.encode_utf16().collect();
    assert_eq!(full_utf16[1236], 0x000a);
    assert_eq!(
        body.text_sections
            .iter()
            .take(5)
            .map(|section| [
                section.start_utf16,
                section.start_utf16 + section.length_utf16
            ])
            .collect::<Vec<_>>(),
        [
            [0, 485],
            [485, 915],
            [915, 1236],
            [1236, 1430],
            [1428, 1670]
        ]
    );
    assert_eq!(
        document.metadata.default_page_dimensions,
        Some((1080, 1527))
    );
    assert_eq!(document.metadata.orientation, Some(0));

    let layout = sdocx::layout_document(&document);
    assert_eq!(layout.pages.len(), 5);
    let last = &layout.pages[4];
    let inspection = last.body_text_slice().unwrap();
    assert_eq!(
        utf16_range(&body.text, &inspection.source_range),
        1429..1670
    );
    let capture = inspection.capture_window.as_ref().unwrap();
    // GetStartPageGroup (0xcef14) joins sections 3–4; CopyText (0xdf48c)
    // removes the initial LF at 1236 while retaining the code separator 1428.
    assert_eq!(capture.first_page_index, 3);
    assert_eq!(capture.requested_page_index, 4);
    assert_eq!(
        utf16_range(&body.text, &capture.saved_source_range),
        1236..1670
    );
    assert_eq!(utf16_range(&body.text, &capture.source_range), 1237..1670);
    assert_eq!(
        utf16_offset(&body.text, capture.reported_source_start),
        1237
    );

    assert_eq!(&full_utf16[1428..1431], [0x000a, 0xfffc, 0x000a]);
    let measured = body.slice_chars(capture.source_range.clone()).unwrap();
    let measured_utf16: Vec<_> = measured.text.encode_utf16().collect();
    assert_eq!(&measured_utf16[191..194], [0x000a, 0xfffc, 0x000a]);
    let font = measured
        .spans
        .iter()
        .find(|span| {
            span.kind == RichTextSpanType::FontSize
                && span.start_utf16 == 184
                && span.end_utf16 == 194
        })
        .unwrap();
    assert_eq!(font.font_size_value(), Some(15.0));
    let code = measured
        .object_spans
        .iter()
        .find(|span| span.text_index_utf16 == 192)
        .unwrap();
    assert!(matches!(
        code.content.as_ref(),
        Some(RichTextObjectContent::CodeBlock(_))
    ));

    let PageObjectContent::Element(PageElement::TextBox(display)) =
        &last.page.objects[inspection.object_index].content
    else {
        panic!()
    };
    assert!(display.text.starts_with('\u{fffc}'));
    assert!(!display.text.starts_with('\n'));
}
