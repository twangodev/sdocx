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
}

impl<'a> Fixture<'a> {
    fn parse(line: &'a str) -> Self {
        let fields = line.split('\t').collect::<Vec<_>>();
        assert_eq!(fields.len(), 10, "invalid corpus manifest row: {line}");
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
        assert!(
            parsed.report.diagnostics.is_empty(),
            "{}: unexpected diagnostics: {:?}",
            fixture.id,
            parsed.report.diagnostics
        );
    }
}
