use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[allow(dead_code)]
#[path = "../../sdocx/tests/support/mod.rs"]
mod support;

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        Self::with_ids(&["one1", "two2"])
    }

    fn with_ids(ids: &[&str]) -> Self {
        Self::with_objects(ids, &[])
    }

    fn with_objects(ids: &[&str], objects: &[Vec<u8>]) -> Self {
        let pages = ids.iter().map(|&id| (id, objects)).collect::<Vec<_>>();
        Self::with_page_objects(&pages)
    }

    fn with_page_objects(pages: &[(&str, &[Vec<u8>])]) -> Self {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
        let directory = std::env::temp_dir().join(format!(
            "sdocx-pdf-cli-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&directory).unwrap();
        let file = std::fs::File::create(directory.join("note.sdocx")).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        for (id, objects) in pages {
            let mut bytes = support::page(&[objects.to_vec()], 0, &[]);
            let original: Vec<_> = "page".encode_utf16().flat_map(u16::to_le_bytes).collect();
            let offset = bytes
                .windows(original.len())
                .position(|window| window == original)
                .unwrap();
            let replacement: Vec<_> = id.encode_utf16().flat_map(u16::to_le_bytes).collect();
            bytes[offset..offset + replacement.len()].copy_from_slice(&replacement);
            zip.start_file(
                format!("{id}.page"),
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(&bytes).unwrap();
        }
        zip.finish().unwrap();
        Self(directory)
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_sdocx-cli"))
            .current_dir(&self.0)
            .arg("note.sdocx")
            .args(args)
            .output()
            .unwrap()
    }
}

fn text_object(value: &str) -> Vec<u8> {
    fn frame(kind: i16, fields: &[u8], fixed: &[u8], flexible: &[u8]) -> Vec<u8> {
        let offset = 13 + fields.len() + fixed.len();
        let mut bytes = ((offset + flexible.len()) as u32).to_le_bytes().to_vec();
        bytes.extend_from_slice(&kind.to_le_bytes());
        bytes.extend_from_slice(&(offset as u32).to_le_bytes());
        bytes.extend_from_slice(&[1, u8::from(kind == 0) << 3, fields.len() as u8]);
        bytes.extend_from_slice(fields);
        bytes.extend_from_slice(fixed);
        bytes.extend_from_slice(flexible);
        bytes
    }
    let mut base = 5500_u32.to_le_bytes().to_vec();
    base.extend_from_slice(&2_u16.to_le_bytes());
    base.extend_from_slice(b"tx");
    base.extend_from_slice(&1234_i64.to_le_bytes());
    for coordinate in [20.0_f64, 20.0, 800.0, 200.0] {
        base.extend_from_slice(&coordinate.to_le_bytes());
    }
    base.extend_from_slice(&[0; 5]);
    let mut common = (value.encode_utf16().count() as u32).to_le_bytes().to_vec();
    common.extend(value.encode_utf16().flat_map(u16::to_le_bytes));
    common.extend_from_slice(&[0; 35]);
    let mut text = (common.len() as u32).to_le_bytes().to_vec();
    text.extend_from_slice(&common);
    let payload = [
        frame(0, &[1, 0, 0, 0, 0], &base, &0_f32.to_le_bytes()),
        frame(6, &[], &[], &[]),
        frame(7, &[1], &[], &text),
        frame(2, &[], &[], &[]),
    ]
    .concat();
    support::object(2, &payload, &[])
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn assert_pdf(path: &Path, size: [f32; 2]) {
    let pdf = lopdf::Document::load(path).unwrap();
    assert_eq!(pdf.get_pages().len(), 2);
    for id in pdf.get_pages().values() {
        let bounds = pdf
            .get_dictionary(*id)
            .unwrap()
            .get(b"MediaBox")
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(bounds[2].as_float().unwrap(), size[0]);
        assert_eq!(bounds[3].as_float().unwrap(), size[1]);
    }
}

#[test]
fn integrity_flag_reports_hash_failures_and_missing_coverage_during_conversion() {
    let fixture = Fixture::new();
    let ordinary = fixture.run(&["-o", "ordinary.pdf"]);
    assert!(ordinary.status.success());
    assert!(!String::from_utf8_lossy(&ordinary.stderr).contains("Integrity"));
    let checked = fixture.run(&["--verify-integrity", "-o", "checked.pdf"]);
    assert!(
        checked.status.success(),
        "{}",
        String::from_utf8_lossy(&checked.stderr)
    );
    let diagnostics = String::from_utf8_lossy(&checked.stderr);
    assert!(diagnostics.contains("Warning [IntegrityMismatch]"));
    assert!(diagnostics.contains("Warning [IntegrityUnavailable]"));
    assert!(diagnostics.contains("Integrity layers: 0 matched, 0 mismatched, 2 unavailable"));
    assert!(diagnostics.contains("Integrity pages: 0 matched, 2 mismatched, 0 unavailable"));
    assert!(diagnostics.contains("Integrity manifest: 0 matched, 0 mismatched, 1 unavailable"));
    assert_pdf(&fixture.0.join("ordinary.pdf"), [810.0, 1145.25]);
    assert_pdf(&fixture.0.join("checked.pdf"), [810.0, 1145.25]);
}

#[test]
fn pdf_extension_flag_override_and_default_path_write_one_document() {
    let fixture = Fixture::new();
    for (args, output) in [
        (vec!["--output", "inferred.PDF"], "inferred.PDF"),
        (vec!["--format", "pdf"], "note.pdf"),
        (
            vec!["--format", "pdf", "--output", "forced.svg"],
            "forced.svg",
        ),
    ] {
        let result = fixture.run(&args);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_pdf(&fixture.0.join(output), [810.0, 1145.25]);
    }
    assert_eq!(
        std::fs::read_dir(&fixture.0).unwrap().count(),
        4,
        "input and three PDFs, no per-page files"
    );
    let result = fixture.run(&["-o", "scaled.pdf", "--pdf-dpi", "144"]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_pdf(&fixture.0.join("scaled.pdf"), [540.0, 763.5]);
}

#[test]
fn invalid_font_or_scale_does_not_overwrite_output() {
    let fixture = Fixture::new();
    std::fs::write(fixture.0.join("existing.pdf"), b"keep existing output").unwrap();
    std::fs::write(fixture.0.join("bad.ttf"), b"invalid font").unwrap();
    for args in [
        vec!["--font", "missing.ttf"],
        vec!["--font", "bad.ttf"],
        vec!["--pdf-dpi", "0"],
        vec!["--pdf-dpi", "NaN"],
        vec!["--pdf-dpi", "0.001"],
    ] {
        let mut command = vec!["-o", "existing.pdf"];
        command.extend(args);
        let result = fixture.run(&command);
        assert!(!result.status.success());
        assert_eq!(
            std::fs::read(fixture.0.join("existing.pdf")).unwrap(),
            b"keep existing output"
        );
    }
    for format in ["svg", "png"] {
        let result = fixture.run(&["-f", format, "--pdf-dpi", "144"]);
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains("--pdf-dpi applies to PDF"));
        assert!(!fixture.0.join(format!("note.{format}")).exists());
    }
}

#[test]
fn svg_embeds_the_explicitly_selected_font_and_validates_font_input() {
    use base64::Engine;
    let fixture = Fixture::with_objects(&["one1"], &[text_object("Caller font")]);
    let mut font = include_bytes!("../../sdocx/assets/fonts/Roboto-Regular.ttf").to_vec();
    font.extend_from_slice(b"sdocx CLI selected font");
    std::fs::write(fixture.0.join("caller.ttf"), &font).unwrap();
    let result = fixture.run(&["--font", "caller.ttf", "-o", "selected.svg"]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let svg = std::fs::read_to_string(fixture.0.join("selected.svg")).unwrap();
    assert!(svg.contains("Caller font"));
    assert!(svg.contains(&base64::engine::general_purpose::STANDARD.encode(&font)));
    for output in ["selected.png", "selected.pdf"] {
        let result = fixture.run(&["--font", "caller.ttf", "-o", output]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    assert!(
        std::fs::read(fixture.0.join("selected.png"))
            .unwrap()
            .starts_with(b"\x89PNG\r\n\x1a\n")
    );
    let extracted = lopdf::Document::load(fixture.0.join("selected.pdf"))
        .unwrap()
        .extract_text(&[1])
        .unwrap();
    assert!(
        extracted.replace('\n', "").contains("Caller font"),
        "{extracted:?}"
    );

    std::fs::write(fixture.0.join("bad.ttf"), b"invalid font").unwrap();
    for invalid in ["bad.ttf", "missing.ttf"] {
        let result = fixture.run(&["--font", invalid, "-o", "selected.svg"]);
        assert!(!result.status.success());
        assert_eq!(
            std::fs::read_to_string(fixture.0.join("selected.svg")).unwrap(),
            svg
        );
    }
}

#[test]
fn selected_page_render_warnings_are_reported_without_failing_export() {
    let fixture = Fixture::with_page_objects(&[
        ("one1", &[text_object("A")]),
        ("two2", &[text_object("אא")]),
    ]);
    let expected = "Warning [MissingGlyphs] visible page 2: font \"Roboto\"; codepoints U+05D0";
    for format in ["svg", "png", "pdf"] {
        let noisy_output = format!("noisy.{format}");
        let result = fixture.run(&["--pages", "2,2", "-o", &noisy_output]);
        let diagnostics = String::from_utf8_lossy(&result.stderr);
        assert!(result.status.success(), "{diagnostics}");
        assert!(result.stdout.is_empty());
        assert_eq!(diagnostics.matches(expected).count(), 1, "{diagnostics}");
        assert_eq!(diagnostics.matches("Warning [MissingGlyphs]").count(), 1);
        let bytes = std::fs::read(fixture.0.join(&noisy_output)).unwrap();
        match format {
            "svg" => assert!(String::from_utf8(bytes).unwrap().contains("אא")),
            "png" => assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n")),
            "pdf" => assert_eq!(
                lopdf::Document::load_mem(&bytes).unwrap().get_pages().len(),
                1
            ),
            _ => unreachable!(),
        }

        let clean_output = format!("clean.{format}");
        let result = fixture.run(&["--pages", "1", "-o", &clean_output]);
        let diagnostics = String::from_utf8_lossy(&result.stderr);
        assert!(result.status.success(), "{diagnostics}");
        assert!(result.stdout.is_empty());
        assert!(!diagnostics.contains("] visible page "), "{diagnostics}");
        assert!(fixture.0.join(clean_output).exists());
    }
}

#[test]
fn page_ranges_select_pdf_pages_and_keep_image_page_numbers() {
    let fixture = Fixture::with_ids(&["one1", "two2", "tri3"]);
    let result = fixture.run(&["--pages", "3, 1-1, 3", "-o", "selected.pdf"]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_pdf(&fixture.0.join("selected.pdf"), [810.0, 1145.25]);
    for format in ["svg", "png"] {
        let result = fixture.run(&["--pages", "3,1", "-f", format]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(fixture.0.join(format!("note_page0.{format}")).exists());
        assert!(fixture.0.join(format!("note_page2.{format}")).exists());
        assert!(!fixture.0.join(format!("note_page1.{format}")).exists());
    }
    let result = fixture.run(&["--pages", "2", "-o", "single.pdf"]);
    assert!(result.status.success());
    assert_eq!(
        lopdf::Document::load(fixture.0.join("single.pdf"))
            .unwrap()
            .get_pages()
            .len(),
        1
    );
    let result = fixture.run(&["--pages", "2", "-o", "single.svg"]);
    assert!(result.status.success());
    assert!(fixture.0.join("single.svg").exists());
}

#[test]
fn invalid_ranges_do_not_overwrite_output() {
    let fixture = Fixture::new();
    let output = fixture.0.join("existing.pdf");
    std::fs::write(&output, b"existing").unwrap();
    for selection in ["", "0", "3", "2-1", "1,", "1-999999999999999999999999"] {
        let result = fixture.run(&["--pages", selection, "-o", "existing.pdf"]);
        assert!(!result.status.success(), "{selection:?}");
        assert_eq!(std::fs::read(&output).unwrap(), b"existing");
    }
}
