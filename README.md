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

For stored-hash checks, enable `ParseOptions.verify_integrity` with a detailed
parse API. `ParsedDocument.integrity` reports matched, mismatched and unavailable
checks for notes, objects, layers, pages and manifest links. These checks follow
Samsung's hash formulas; object hashes exclude geometry and content. See the
[integrity findings](docs/reverse-engineering/integrity-findings.md) for coverage.

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

To include stored-hash diagnostics and coverage counts during conversion:

```sh
sdocx-cli note.sdocx --verify-integrity
```

Hash mismatches and unavailable checks are reported on stderr and do not stop
conversion or change its exit status. A successful conversion with this flag
does not establish that every integrity check passed.

For PNG or PDF export, supply font files when the document's fonts are unavailable
locally. Repeat `--font` for additional faces; explicit faces take precedence
over matching system fonts:

```sh
sdocx-cli note.sdocx -o note.png --font /path/to/Roboto-Regular.ttf --font /path/to/Roboto-Italic.ttf
```

Fonts are loaded once per document. Missing or invalid explicit font files
produce an error. SVG output references font families and does not embed fonts;
`--font` applies to PNG and PDF export.

PDF export writes all visible pages into one file:

```sh
sdocx-cli note.sdocx -o note.pdf --font /path/to/Roboto-Regular.ttf
sdocx-cli note.sdocx --format pdf
```

`--format` overrides the output extension. SVG remains the default when neither
is supplied. SVG and PNG use separate files for multiple pages; PDF uses one
file. `--pdf-dpi 144` sets the physical scale to 144 SVG units per inch; the
default is 96. This option applies only to PDF and does not rasterize vectors.

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

### PDF export

Rust 1.92 or newer is required. Enable the optional `pdf` feature (`cargo add sdocx --features pdf`):

```rust
use sdocx::{PdfOptions, RenderOptions, parse, render_document_pdf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let document = parse("notes.sdocx")?;
    let pdf_options = PdfOptions::default();
    let bytes = render_document_pdf(&document, &RenderOptions::default(), &pdf_options)?;
    std::fs::write("notes.pdf", bytes)?;
    Ok(())
}
```

`render_document_pdf` exports all visible pages into one PDF through the shared
SVG renderer. `render_svg_pages_pdf` accepts an existing `Vec<RenderedPage>`.
Vectors remain vectors and available fonts are embedded with selectable text.
For controlled fonts, populate `sdocx::pdf::fontdb::Database` and pass it in an
`Arc` to `PdfOptions::new`; that constructor does not discover system fonts.

Page dimensions use 96 SVG units per inch by default (one unit = 0.75 PDF
points). Set `PdfOptions::dpi` to change physical size. This is an explicit
export scale, not a decoded Samsung print setting. PDF pages are limited to
14,400 points per side. PNG images must decode within a 64 MiB buffer limit.

PDF inherits the SVG renderer's fidelity limits. Font fallback depends on the
provided fonts, some SVG filters rasterize, and PDF link annotations and
semantic document tags are not exported. The `pdf` feature is independent of
the browser/WASM bindings.

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
dataset, tracked at `hf/` as a Git submodule. Each SDK revision pins the dataset
commit used by its conformance checks. See
[`conformance/README.md`](conformance/README.md) for Git LFS setup, the locked
manifest and the local runner.

## Format Documentation

The maintained [reverse-engineering documentation](docs/reverse-engineering/README.md)
describes the archive format, native serializers, parser behavior and remaining
fidelity gaps. Start with the [file-format map](docs/reverse-engineering/file-format.md)
for record layouts and the [source map](docs/reverse-engineering/source-map.md)
for supporting APK/native evidence.

## License

[GPL-3.0](LICENSE)
