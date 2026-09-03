# sdocx

[![CI](https://img.shields.io/github/actions/workflow/status/twangodev/sdocx/rust.yml?label=CI)](https://github.com/twangodev/sdocx/actions/workflows/rust.yml)
[![crates.io (sdocx)](https://img.shields.io/crates/v/sdocx)](https://crates.io/crates/sdocx)
[![npm](https://img.shields.io/npm/v/@twango/sdocx)](https://www.npmjs.com/package/@twango/sdocx)
[![docs.rs](https://img.shields.io/docsrs/sdocx)](https://docs.rs/sdocx)
[![License](https://img.shields.io/crates/l/sdocx)](https://github.com/twangodev/sdocx/blob/main/LICENSE)

Reverse-engineered tooling and SDK for converting Samsung Notes (`.sdocx`) files.

## Browser application

The static application in [`web/`](web/) provides a local converter and a
continuous document preview. Parsing, rendering, and export happen in the
browser; user-selected documents are not uploaded. The generated site is
configured for Workers Static Assets at `sdocx.twango.dev`.

## Parser accuracy

`sdocx` is a reverse-engineered parser, not a drop-in implementation of
Samsung's S Pen SDK. Archive structure, format versions, page ordering, and
supported packed stroke channels follow observed SDK contracts. Higher-level
page objects, rich text, templates, and media associations are currently
best-effort.

A successful parse may omit unsupported objects or properties; it does not
guarantee a lossless decode. Preserve original documents and validate output
against Samsung Notes when fidelity matters. Protected documents must be
unlocked or exported before parsing.

## Installation

### CLI

```sh
cargo install sdocx-cli
```

### Library

```sh
cargo add sdocx
```

### npm (WASM)

```sh
npm install @twango/sdocx
```

### Docker

```sh
docker pull ghcr.io/twangodev/sdocx
```

## CLI Usage

```sh
sdocx-cli note.sdocx
```

With Docker:

```sh
docker run --rm -v "$(pwd)":/data ghcr.io/twangodev/sdocx /data/note.sdocx
```

## Library Usage

```rust
use sdocx::{layout_document, parse};

fn main() -> sdocx::Result<()> {
    let doc = parse("notes.sdocx")?;
    let layout = layout_document(&doc);

    println!(
        "{} visible page(s), {} stored page record(s)",
        layout.pages.len(),
        doc.pages.len()
    );

    for visible_page in &layout.pages {
        for stroke in &visible_page.page.strokes {
            println!(
                "Stroke: {} points, color {:?}, width {}",
                stroke.points.len(),
                stroke.color,
                stroke.pen_width
            );
            for point in &stroke.points {
                println!("  ({}, {})", point.x, point.y);
            }
        }
    }

    Ok(())
}
```

## JavaScript Usage

```js
import init, { parse } from "@twango/sdocx";

await init();

const bytes = new Uint8Array(await file.arrayBuffer());
const doc = parse(bytes);

for (const page of doc.pages) {
  for (const stroke of page.strokes) {
    console.log(`${stroke.points.length} points, color:`, stroke.color);
  }
}
```

## Compatibility corpus

Large test documents and Samsung reference PDFs are kept in the
[`twangodev/sdocx-compatibility`](https://huggingface.co/datasets/twangodev/sdocx-compatibility)
dataset rather than committed to this repository. See
[`conformance/README.md`](conformance/README.md) for the locked manifest and
local runner.

## Format Documentation

Samsung Notes `.sdocx` files are ZIP archives containing binary stroke data, metadata, and page definitions. The [`notebooks/`](notebooks/) directory contains Jupyter notebooks that document the reverse-engineering process:

- [`01_container.ipynb`](notebooks/01_container.ipynb) — Archive structure and container parsing
- [`02_strokes.ipynb`](notebooks/02_strokes.ipynb) — Stroke decoding and coordinate parsing
- [`03_ink.ipynb`](notebooks/03_ink.ipynb) — Ink color and metadata extraction

The notebooks read external documents from `SDOCX_SAMPLE`, `SDOCX_HANDWRITTEN_SAMPLE`, and
`SDOCX_MEDIA_SAMPLE` environment variables.

## License

[GPL-3.0](LICENSE)
