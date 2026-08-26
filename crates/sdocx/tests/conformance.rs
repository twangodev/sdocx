use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

const MANIFEST: &str = include_str!("../../../conformance/corpus.tsv");

struct Fixture<'a> {
    id: &'a str,
    sdocx: &'a str,
    sdocx_sha256: &'a str,
    reference_pdf: &'a str,
    reference_pdf_sha256: &'a str,
    stored_pages: usize,
    visible_pages: usize,
    title: &'a str,
    minimum_body_characters: usize,
    required_text: &'a str,
    text_sections: usize,
    hyperlinks: usize,
    tables: usize,
    code_blocks: usize,
    required_link_target: &'a str,
    required_table_text: &'a str,
    required_code_text: &'a str,
}

impl<'a> Fixture<'a> {
    fn parse(line: &'a str) -> Self {
        let fields = line.split('\t').collect::<Vec<_>>();
        assert_eq!(fields.len(), 17, "invalid corpus manifest row: {line}");
        Self {
            id: fields[0],
            sdocx: fields[1],
            sdocx_sha256: fields[2],
            reference_pdf: fields[3],
            reference_pdf_sha256: fields[4],
            stored_pages: fields[5].parse().expect("stored page count"),
            visible_pages: fields[6].parse().expect("visible page count"),
            title: fields[7],
            minimum_body_characters: fields[8].parse().expect("minimum body characters"),
            required_text: fields[9],
            text_sections: fields[10].parse().expect("text section count"),
            hyperlinks: fields[11].parse().expect("hyperlink count"),
            tables: fields[12].parse().expect("table count"),
            code_blocks: fields[13].parse().expect("code-block count"),
            required_link_target: fields[14],
            required_table_text: fields[15],
            required_code_text: fields[16],
        }
    }
}

fn corpus_root() -> PathBuf {
    std::env::var_os("SDOCX_CORPUS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join("hf")
        })
}

fn assert_sha256(path: &Path, expected: &str, fixture_id: &str) {
    let bytes = fs::read(path)
        .unwrap_or_else(|error| panic!("{fixture_id}: cannot read {}: {error}", path.display()));
    let actual = format!("{:x}", Sha256::digest(bytes));
    assert_eq!(actual, expected, "{fixture_id}: SHA-256 mismatch");
}

#[test]
#[ignore = "requires the external Hugging Face compatibility corpus"]
fn external_corpus_matches_locked_expectations() {
    let root = corpus_root();
    assert!(
        root.is_dir(),
        "corpus directory {} is missing; see conformance/README.md",
        root.display()
    );

    for line in MANIFEST
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
    {
        let fixture = Fixture::parse(line);
        let sdocx_path = root.join(fixture.sdocx);
        let reference_pdf_path = root.join(fixture.reference_pdf);
        assert_sha256(&sdocx_path, fixture.sdocx_sha256, fixture.id);
        assert_sha256(
            &reference_pdf_path,
            fixture.reference_pdf_sha256,
            fixture.id,
        );

        let parsed = sdocx::parse_detailed(&sdocx_path)
            .unwrap_or_else(|error| panic!("{}: parse failed: {error}", fixture.id));
        let layout = sdocx::layout_document(&parsed.document);
        assert_eq!(
            parsed.stored_pages.len(),
            fixture.stored_pages,
            "{}: stored page count",
            fixture.id
        );
        assert_eq!(
            layout.pages.len(),
            fixture.visible_pages,
            "{}: visible page count",
            fixture.id
        );
        assert_eq!(
            parsed.note.as_ref().map(|note| note.title.text.as_str()),
            Some(fixture.title),
            "{}: note title",
            fixture.id
        );
        let body = &parsed.note.as_ref().expect("structured note").body.text;
        assert!(
            body.chars().count() >= fixture.minimum_body_characters,
            "{}: body is unexpectedly short",
            fixture.id
        );
        assert!(
            body.contains(fixture.required_text),
            "{}: required body text is missing",
            fixture.id
        );
        let flow = parsed
            .document
            .metadata
            .note_text
            .as_ref()
            .expect("document-level text flow");
        assert_eq!(
            flow.text_sections.len(),
            fixture.text_sections,
            "{}: text section count",
            fixture.id
        );
        let hyperlinks = flow
            .spans
            .iter()
            .filter(|span| span.kind == sdocx::RichTextSpanType::Hyperlink)
            .collect::<Vec<_>>();
        assert_eq!(
            hyperlinks.len(),
            fixture.hyperlinks,
            "{}: hyperlink count",
            fixture.id
        );
        assert!(
            hyperlinks.iter().any(|span| {
                span.hyperlink_value()
                    .and_then(|link| link.custom_data)
                    .is_some_and(|target| target == fixture.required_link_target)
            }),
            "{}: required hyperlink target is missing",
            fixture.id
        );
        let tables = flow
            .object_spans
            .iter()
            .filter_map(|span| match span.content.as_ref() {
                Some(sdocx::RichTextObjectContent::Table(table)) => Some(table.as_ref()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(tables.len(), fixture.tables, "{}: table count", fixture.id);
        assert!(
            tables.iter().any(|table| table.rows.iter().any(|row| {
                row.cells
                    .iter()
                    .any(|cell| cell.content.text.contains(fixture.required_table_text))
            })),
            "{}: required table text is missing",
            fixture.id
        );
        let code_blocks = flow
            .object_spans
            .iter()
            .filter_map(|span| match span.content.as_ref() {
                Some(sdocx::RichTextObjectContent::CodeBlock(code)) => Some(code.as_ref()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            code_blocks.len(),
            fixture.code_blocks,
            "{}: code-block count",
            fixture.id
        );
        assert!(
            code_blocks.iter().any(|code| {
                code.body
                    .as_ref()
                    .is_some_and(|body| body.text.contains(fixture.required_code_text))
            }),
            "{}: required code-block text is missing",
            fixture.id
        );
        assert!(
            parsed.report.diagnostics.is_empty(),
            "{}: unexpected diagnostics: {:?}",
            fixture.id,
            parsed.report.diagnostics
        );
    }
}
