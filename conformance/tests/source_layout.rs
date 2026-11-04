use std::fs;
use std::io::Read;
use std::path::Path;

fn find_scripts(directory: &Path, scripts: &mut Vec<String>) {
    for entry in fs::read_dir(directory).expect("read conformance directory") {
        let entry = entry.expect("read conformance entry");
        let path = entry.path();
        let file_type = entry.file_type().expect("read conformance entry type");
        if file_type.is_dir() {
            find_scripts(&path, scripts);
        } else if file_type.is_file() || (file_type.is_symlink() && path.is_file()) {
            // Never recurse through directory symlinks; file aliases are inspected.
            let extension = path
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            let script_extension = matches!(
                extension.as_str(),
                "py" | "pyc"
                    | "pyo"
                    | "pyw"
                    | "ipynb"
                    | "sh"
                    | "bash"
                    | "js"
                    | "mjs"
                    | "cjs"
                    | "ts"
                    | "rb"
                    | "pl"
                    | "ps1"
                    | "lua"
            );
            let mut prefix = Vec::new();
            fs::File::open(&path)
                .expect("open conformance file")
                .take(256)
                .read_to_end(&mut prefix)
                .expect("read conformance file");
            // Rust inner attributes begin with #![, possibly with whitespace.
            let has_shebang = prefix.strip_prefix(b"#!").is_some_and(|rest| {
                rest.iter()
                    .copied()
                    .find(|byte| !byte.is_ascii_whitespace())
                    .is_some_and(|byte| byte != b'[')
            });
            if script_extension || has_shebang {
                scripts.push(path.display().to_string());
            }
        }
    }
}

#[test]
fn conformance_keeps_non_rust_scripts_out() {
    let mut scripts = Vec::new();
    find_scripts(Path::new(env!("CARGO_MANIFEST_DIR")), &mut scripts);
    scripts.sort();
    assert!(
        scripts.is_empty(),
        "Keep capture scripts in tools/ and conformance tests in Rust: {scripts:#?}"
    );
}
