# sdocx

[![CI](https://img.shields.io/github/actions/workflow/status/twangodev/sdocx/ci.yml?label=CI)](https://github.com/twangodev/sdocx/actions/workflows/ci.yml)
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
supported packed stroke channels follow observed SDK contracts. Standalone text
boxes use bounded native frames and preserve Unicode, placement and rich-text
records. Image objects also use native frames and resolve their displayed asset
through media-manifest bind IDs. Shapes and lines decode native geometry,
outline/fill styles and embedded shape text; common templates, straight lines
and supported curves render to SVG. Text layout, image crop/border effects,
advanced shape styles and other page objects remain best-effort.

A successful parse may omit unsupported objects or properties; it does not
guarantee a lossless decode. Preserve original documents and validate output
against Samsung Notes when fidelity matters. Protected documents must be
unlocked or exported before parsing.

Use `parse_detailed` or `parse_bytes_detailed` to inspect `ParseReport`, including
detected unsupported text/image/shape features and unresolved media. The CLI prints
these findings during conversion, and WASM exposes them through document
inspection. An empty report does not guarantee complete rendering fidelity.

Native image objects are exposed as `PageElement::PlacedImage`, including their
media ID, optional resolved asset index, bounds and rotation. Existing
caller-created `PageElement::Image` values remain renderable. See the
[image findings](docs/reverse-engineering/image-findings.md) for the supported
fields and resolution rules.

Native shapes and lines are exposed as `PageElement::Shape` and
`PageElement::Line`, with explicit geometry, styles and pen-resource references.
See the [shape/line findings](docs/reverse-engineering/shape-line-findings.md)
for supported templates, paths and remaining rendering limits.

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
