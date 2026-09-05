use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;
use sha2::{Digest, Sha256};

const MANIFEST: &str = include_str!("../../../conformance/corpus.json");

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    version: u32,
    fixtures: Vec<Fixture>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    id: String,
    sdocx: Asset,
    reference_pdf: Asset,
    expected: Expectations,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Asset {
    path: PathBuf,
    sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Expectations {
    stored_pages: usize,
    visible_pages: usize,
    title: Option<String>,
    body: Option<BodyExpectations>,
    flow: Option<FlowExpectations>,
    page_objects: Option<PageObjectExpectations>,
    #[serde(default)]
    diagnostics: BTreeMap<String, usize>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BodyExpectations {
    #[serde(default)]
    minimum_characters: usize,
    #[serde(default)]
    required_text: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FlowExpectations {
    text_sections: Option<usize>,
    hyperlinks: Option<usize>,
    tables: Option<usize>,
    code_blocks: Option<usize>,
    #[serde(default)]
    required_link_targets: Vec<String>,
    #[serde(default)]
    required_table_text: Vec<String>,
    #[serde(default)]
    required_code_text: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct PageObjectExpectations {
    strokes: Option<usize>,
    images: Option<usize>,
    text_boxes: Option<usize>,
    shapes: Option<usize>,
    lines: Option<usize>,
}

fn read_manifest(json: &str) -> Result<Manifest, String> {
    let manifest: Manifest = serde_json::from_str(json).map_err(|error| error.to_string())?;
    if manifest.version != 1 || manifest.fixtures.is_empty() {
        return Err("manifest must use version 1 and contain fixtures".into());
    }
    let mut ids = HashSet::new();
    for fixture in &manifest.fixtures {
        if fixture.id.is_empty()
            || !fixture
                .id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
            || !ids.insert(&fixture.id)
        {
            return Err(format!("invalid or duplicate fixture ID: {}", fixture.id));
        }
        if fixture.expected.visible_pages == 0
            || fixture.expected.stored_pages < fixture.expected.visible_pages
        {
            return Err(format!("{}: invalid page counts", fixture.id));
        }
        if fixture
            .expected
            .diagnostics
            .values()
            .any(|count| *count == 0)
        {
            return Err(format!(
                "{}: diagnostic counts must be positive",
                fixture.id
            ));
        }
        let required_text = fixture
            .expected
            .body
            .iter()
            .flat_map(|body| &body.required_text)
            .chain(fixture.expected.flow.iter().flat_map(|flow| {
                flow.required_link_targets
                    .iter()
                    .chain(&flow.required_table_text)
                    .chain(&flow.required_code_text)
            }));
        if required_text.into_iter().any(|text| text.trim().is_empty()) {
            return Err(format!("{}: required text must not be empty", fixture.id));
        }
        for asset in [&fixture.sdocx, &fixture.reference_pdf] {
            if asset.path.as_os_str().is_empty()
                || asset.path.to_string_lossy().contains('\\')
                || asset.path.to_string_lossy().contains(':')
                || asset
                    .path
                    .components()
                    .any(|part| !matches!(part, Component::Normal(_)))
                || asset.sha256.len() != 64
                || !asset
                    .sha256
                    .bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
            {
                return Err(format!("{}: invalid asset path or SHA-256", fixture.id));
            }
        }
    }
    Ok(manifest)
}

fn corpus_root() -> PathBuf {
    std::env::var_os("SDOCX_CORPUS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../hf"))
}

fn verified_asset(root: &Path, asset: &Asset) -> Result<PathBuf, String> {
    let path = root
        .join(&asset.path)
        .canonicalize()
        .map_err(|error| error.to_string())?;
    if !path.starts_with(root) {
        return Err(format!("{} is outside the corpus", asset.path.display()));
    }
    let bytes = fs::read(&path).map_err(|error| error.to_string())?;
    let actual = format!("{:x}", Sha256::digest(bytes));
    if actual != asset.sha256 {
        return Err(format!("{}: SHA-256 mismatch", asset.path.display()));
    }
    Ok(path)
}

fn check_count(label: &str, actual: usize, expected: Option<usize>) -> Result<(), String> {
    if let Some(expected) = expected
        && actual != expected
    {
        return Err(format!("{label}: expected {expected}, got {actual}"));
    }
    Ok(())
}

fn check_document(parsed: &sdocx::ParsedDocument, expected: &Expectations) -> Result<(), String> {
    check_count(
        "stored pages",
        parsed.stored_pages.len(),
        Some(expected.stored_pages),
    )?;
    check_count(
        "visible pages",
        sdocx::layout_document(&parsed.document).pages.len(),
        Some(expected.visible_pages),
    )?;
    if let Some(title) = &expected.title
        && parsed.note.as_ref().map(|note| &note.title.text) != Some(title)
    {
        return Err("note title differs".into());
    }
    if let Some(expected) = &expected.body {
        let body = &parsed
            .note
            .as_ref()
            .ok_or("structured note is missing")?
            .body
            .text;
        if body.chars().count() < expected.minimum_characters {
            return Err("body is unexpectedly short".into());
        }
        for text in &expected.required_text {
            if !body.contains(text) {
                return Err(format!("required body text is missing: {text}"));
            }
        }
    }
    if let Some(expected) = &expected.flow {
        check_flow(
            parsed
                .document
                .metadata
                .note_text
                .as_ref()
                .ok_or("document-level text flow is missing")?,
            expected,
        )?;
    }
    if let Some(expected) = &expected.page_objects {
        check_page_objects(&parsed.document, expected)?;
    }
    let mut diagnostics = BTreeMap::new();
    for diagnostic in &parsed.report.diagnostics {
        *diagnostics
            .entry(format!("{:?}", diagnostic.code))
            .or_insert(0) += 1;
    }
    if diagnostics != expected.diagnostics {
        return Err(format!(
            "diagnostics: expected {:?}, got {diagnostics:?}",
            expected.diagnostics
        ));
    }
    Ok(())
}

fn check_flow(flow: &sdocx::RichTextBox, expected: &FlowExpectations) -> Result<(), String> {
    check_count(
        "text sections",
        flow.text_sections.len(),
        expected.text_sections,
    )?;
    let hyperlinks: Vec<_> = flow
        .spans
        .iter()
        .filter(|span| span.kind == sdocx::RichTextSpanType::Hyperlink)
        .collect();
    check_count("hyperlinks", hyperlinks.len(), expected.hyperlinks)?;
    for target in &expected.required_link_targets {
        if !hyperlinks.iter().any(|span| {
            span.hyperlink_value()
                .and_then(|link| link.custom_data)
                .as_ref()
                == Some(target)
        }) {
            return Err(format!("required hyperlink target is missing: {target}"));
        }
    }
    let tables: Vec<_> = flow
        .object_spans
        .iter()
        .filter_map(|span| match span.content.as_ref() {
            Some(sdocx::RichTextObjectContent::Table(table)) => Some(table.as_ref()),
            _ => None,
        })
        .collect();
    check_count("tables", tables.len(), expected.tables)?;
    for text in &expected.required_table_text {
        if !tables.iter().any(|table| {
            table.rows.iter().any(|row| {
                row.cells
                    .iter()
                    .any(|cell| cell.content.text.contains(text))
            })
        }) {
            return Err(format!("required table text is missing: {text}"));
        }
    }
    let code_blocks: Vec<_> = flow
        .object_spans
        .iter()
        .filter_map(|span| match span.content.as_ref() {
            Some(sdocx::RichTextObjectContent::CodeBlock(code)) => Some(code.as_ref()),
            _ => None,
        })
        .collect();
    check_count("code blocks", code_blocks.len(), expected.code_blocks)?;
    for text in &expected.required_code_text {
        if !code_blocks.iter().any(|code| {
            code.body
                .as_ref()
                .is_some_and(|body| body.text.contains(text))
        }) {
            return Err(format!("required code-block text is missing: {text}"));
        }
    }
    Ok(())
}

fn check_page_objects(
    document: &sdocx::Document,
    expected: &PageObjectExpectations,
) -> Result<(), String> {
    let mut images = 0;
    let mut text_boxes = 0;
    let mut shapes = 0;
    let mut lines = 0;
    for page in &document.pages {
        for element in &page.elements {
            match element {
                sdocx::PageElement::Image { .. } | sdocx::PageElement::PlacedImage(_) => {
                    images += 1
                }
                sdocx::PageElement::TextBox(_) => text_boxes += 1,
                sdocx::PageElement::Shape(_) => shapes += 1,
                sdocx::PageElement::Line(_) => lines += 1,
                _ => {}
            }
        }
    }
    check_count(
        "page strokes",
        document.pages.iter().map(|page| page.strokes.len()).sum(),
        expected.strokes,
    )?;
    check_count("page images", images, expected.images)?;
    check_count("page text boxes", text_boxes, expected.text_boxes)?;
    check_count("page shapes", shapes, expected.shapes)?;
    check_count("page lines", lines, expected.lines)
}

#[test]
fn locked_manifest_is_valid_without_external_files() {
    read_manifest(MANIFEST).unwrap();
}

#[allow(dead_code)]
mod support;

fn drawing_fixture() -> (sdocx::ParsedDocument, Expectations) {
    let archive = support::archive(&support::page(&[vec![]], 0, &[]));
    let parsed = sdocx::parse_bytes_detailed(&archive).unwrap();
    let expected = serde_json::from_value(serde_json::json!({
        "stored_pages": 1,
        "visible_pages": 1,
        "page_objects": {"strokes": 0, "images": 0, "text_boxes": 0, "shapes": 0, "lines": 0},
        "diagnostics": {"MissingPageManifest": 1, "UnlistedPageEntry": 1}
    }))
    .unwrap();
    (parsed, expected)
}

#[test]
fn drawing_notes_do_not_require_a_title_or_flow_text() {
    let (parsed, expected) = drawing_fixture();
    assert!(parsed.note.is_none());
    assert!(parsed.document.metadata.note_text.is_none());
    check_document(&parsed, &expected).unwrap();
}

#[test]
fn missing_or_unexpected_page_objects_fail_exact_counts() {
    let (mut parsed, mut expected) = drawing_fixture();
    parsed.document.pages[0]
        .elements
        .push(sdocx::PageElement::Image {
            bbox: Default::default(),
            media_index: 0,
        });
    assert!(
        check_document(&parsed, &expected)
            .unwrap_err()
            .contains("page images")
    );
    expected.page_objects.as_mut().unwrap().images = Some(1);
    check_document(&parsed, &expected).unwrap();
    parsed.document.pages[0].elements.clear();
    assert!(
        check_document(&parsed, &expected)
            .unwrap_err()
            .contains("page images")
    );
    for kind in ["strokes", "text_boxes", "shapes", "lines"] {
        let mut counts = serde_json::json!({});
        counts[kind] = serde_json::json!(1);
        let counts = serde_json::from_value(counts).unwrap();
        assert!(
            check_page_objects(&parsed.document, &counts).is_err(),
            "{kind}"
        );
    }
}

#[test]
fn expected_diagnostics_are_exact_counts_not_a_warning_allowlist() {
    let (mut parsed, mut expected) = drawing_fixture();
    let warning = sdocx::ParseDiagnostic {
        severity: sdocx::DiagnosticSeverity::Warning,
        code: sdocx::DiagnosticCode::UnsupportedShapeFeature,
        archive_entry: Some("page.page".into()),
        message: "unsupported fill".into(),
    };
    parsed.report.diagnostics.push(warning.clone());
    assert!(
        check_document(&parsed, &expected)
            .unwrap_err()
            .contains("diagnostics")
    );
    expected
        .diagnostics
        .insert("UnsupportedShapeFeature".into(), 1);
    check_document(&parsed, &expected).unwrap();
    parsed.report.diagnostics.push(warning);
    assert!(
        check_document(&parsed, &expected)
            .unwrap_err()
            .contains("diagnostics")
    );
    parsed.report.diagnostics.clear();
    assert!(
        check_document(&parsed, &expected)
            .unwrap_err()
            .contains("diagnostics")
    );
}

#[test]
fn zero_flow_object_counts_do_not_require_a_table_link_or_code_block() {
    let flow = sdocx::RichTextBox {
        text_area_type: None,
        bbox: Default::default(),
        rotation_degrees: None,
        text: "simple text".into(),
        color: None,
        highlight_color: None,
        underline: false,
        font_size: None,
        runs: vec![],
        spans: vec![],
        paragraphs: vec![],
        object_spans: vec![],
        text_sections: vec![],
        margins: None,
        gravity: None,
    };
    let expected = serde_json::from_value(serde_json::json!({
        "text_sections": 0, "hyperlinks": 0, "tables": 0, "code_blocks": 0
    }))
    .unwrap();
    check_flow(&flow, &expected).unwrap();
    for key in [
        "required_link_targets",
        "required_table_text",
        "required_code_text",
    ] {
        let mut expected = serde_json::json!({});
        expected[key] = serde_json::json!(["must exist"]);
        assert!(
            check_flow(&flow, &serde_json::from_value(expected).unwrap()).is_err(),
            "{key}"
        );
    }
}

#[test]
fn manifest_rejects_unknown_fields_duplicates_and_invalid_metadata() {
    let original: serde_json::Value = serde_json::from_str(MANIFEST).unwrap();
    let mut value = original.clone();
    value["version"] = serde_json::json!(2);
    assert!(read_manifest(&value.to_string()).is_err());
    value = original.clone();
    let fixture = value["fixtures"][0].clone();
    value["fixtures"].as_array_mut().unwrap().push(fixture);
    assert!(
        read_manifest(&value.to_string())
            .unwrap_err()
            .contains("duplicate")
    );
    for key in ["title_typo", "images"] {
        value = original.clone();
        value["fixtures"][0]["expected"][key] = serde_json::json!(1);
        assert!(
            read_manifest(&value.to_string())
                .unwrap_err()
                .contains("unknown field")
        );
    }
    for path in ["../note.sdocx", "/note.sdocx", "C:\\note.sdocx"] {
        value = original.clone();
        value["fixtures"][0]["sdocx"]["path"] = serde_json::json!(path);
        assert!(read_manifest(&value.to_string()).is_err());
    }
    for pages in [0, 7] {
        value = original.clone();
        value["fixtures"][0]["expected"]["visible_pages"] = serde_json::json!(pages);
        assert!(read_manifest(&value.to_string()).is_err());
    }
    value = original.clone();
    value["fixtures"][0]["expected"]["body"]["required_text"] = serde_json::json!([""]);
    assert!(
        read_manifest(&value.to_string())
            .unwrap_err()
            .contains("must not be empty")
    );
    value = original;
    value["fixtures"][0]["expected"]["diagnostics"] = serde_json::json!({"MissingPageManifest": 0});
    assert!(
        read_manifest(&value.to_string())
            .unwrap_err()
            .contains("must be positive")
    );
}

#[test]
#[ignore = "requires the external Hugging Face compatibility corpus"]
fn external_corpus_matches_locked_expectations() {
    let root = corpus_root()
        .canonicalize()
        .expect("corpus directory; see conformance/README.md");
    for fixture in read_manifest(MANIFEST).unwrap().fixtures {
        let path = verified_asset(&root, &fixture.sdocx)
            .unwrap_or_else(|error| panic!("{}: {error}", fixture.id));
        let reference = verified_asset(&root, &fixture.reference_pdf)
            .unwrap_or_else(|error| panic!("{}: {error}", fixture.id));
        let reference = lopdf::Document::load(reference)
            .unwrap_or_else(|error| panic!("{}: invalid reference PDF: {error}", fixture.id));
        check_count(
            "reference PDF pages",
            reference.get_pages().len(),
            Some(fixture.expected.visible_pages),
        )
        .unwrap_or_else(|error| panic!("{}: {error}", fixture.id));
        let parsed =
            sdocx::parse_detailed(path).unwrap_or_else(|error| panic!("{}: {error}", fixture.id));
        check_document(&parsed, &fixture.expected)
            .unwrap_or_else(|error| panic!("{}: {error}", fixture.id));
    }
}
