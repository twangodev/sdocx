use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[allow(dead_code)]
#[path = "../../sdocx/tests/support/mod.rs"]
mod support;

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
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
        for id in ["one1", "two2"] {
            let mut bytes = support::page(&[vec![]], 0, &[]);
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
