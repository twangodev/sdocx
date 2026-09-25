# sdocx

[![CI](https://img.shields.io/github/actions/workflow/status/twangodev/sdocx/ci.yml?label=CI)](https://github.com/twangodev/sdocx/actions/workflows/ci.yml)
[![crates.io (sdocx)](https://img.shields.io/crates/v/sdocx)](https://crates.io/crates/sdocx)
[![npm](https://img.shields.io/npm/v/@twango/sdocx)](https://www.npmjs.com/package/@twango/sdocx)
[![docs.rs](https://img.shields.io/docsrs/sdocx)](https://docs.rs/sdocx)
[![License](https://img.shields.io/crates/l/sdocx)](https://github.com/twangodev/sdocx/blob/main/LICENSE)

Convert Samsung Notes (`.sdocx`) files to SVG, PNG, or PDF with a Rust SDK, CLI, and WebAssembly bindings.

Try the [browser app](https://sdocx.twango.dev) to preview, convert, and debug documents locally. Files are processed in your browser and are not uploaded.

## Installation

```sh
cargo install sdocx-cli      # CLI
cargo add sdocx              # Rust library
npm install @twango/sdocx    # JavaScript / WASM
```

## CLI

```sh
sdocx-cli note.sdocx                          # SVG (default)
sdocx-cli note.sdocx -o note.png
sdocx-cli note.sdocx -o note.pdf --pages "1-3, 5"
sdocx-cli note.sdocx -o note.pdf --font /path/to/Roboto-Regular.ttf
```

PDF combines selected pages into one file; SVG and PNG produce separate files per page. Use `--help` for all options.

Or run with Docker:

```sh
docker run --rm -v "$(pwd)":/data ghcr.io/twangodev/sdocx /data/note.sdocx
```

## Rust

```rust
use sdocx::{layout_document, parse};

fn main() -> sdocx::Result<()> {
    let doc = parse("note.sdocx")?;
    let layout = layout_document(&doc);
    println!("{} visible page(s)", layout.pages.len());
    Ok(())
}
```

For PDF export, enable the `pdf` feature (Rust 1.92+) and use `render_document_pdf`. See the [API docs](https://docs.rs/sdocx).

## JavaScript

```js
import init, { parse } from "@twango/sdocx";

await init();
const doc = parse(new Uint8Array(await file.arrayBuffer()));
console.log(doc.pages);
```

## Limitations

The format is reverse-engineered, and conversion is best-effort. Unsupported objects or styles may be omitted even when parsing succeeds. Keep original files and check output against Samsung Notes when fidelity matters. Protected documents must be unlocked or exported first.

Use the detailed Rust parse APIs or CLI diagnostics to inspect unsupported features. `--verify-integrity` adds stored-hash checks; it does not guarantee complete fidelity or fail conversion on mismatches.

## Documentation

- [Format and reverse-engineering notes](docs/reverse-engineering/README.md)
- [Web app](web/README.md) and [debugger guide](docs/web-debugger.md)
- [Conformance testing](conformance/README.md) and [compatibility dataset](https://huggingface.co/datasets/twangodev/sdocx-compatibility)

## License

[GPL-3.0](LICENSE)
