# sdocx

[![CI](https://img.shields.io/github/actions/workflow/status/twangodev/sdocx/rust.yml?label=CI)](https://github.com/twangodev/sdocx/actions/workflows/rust.yml)
[![crates.io (sdocx)](https://img.shields.io/crates/v/sdocx)](https://crates.io/crates/sdocx)
[![npm](https://img.shields.io/npm/v/@twango/sdocx)](https://www.npmjs.com/package/@twango/sdocx)
[![docs.rs](https://img.shields.io/docsrs/sdocx)](https://docs.rs/sdocx)
[![License](https://img.shields.io/crates/l/sdocx)](https://github.com/twangodev/sdocx/blob/main/LICENSE)

Reverse-engineered tooling and SDK for converting Samsung Notes (`.sdocx`) files.

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
sdocx-cli samples/handwritten.sdocx
```

```
Page dimensions: 1848 x 7838
Background: #252525
1 page(s)
  Page 0: 1848 x 7838, 2769 strokes, 321776 points, 3 colors, 2769 with pressure
```

With Docker:

```sh
docker run --rm -v "$(pwd)":/data ghcr.io/twangodev/sdocx /data/samples/handwritten.sdocx
```

## Library Usage

```rust
use sdocx::parse;

fn main() -> sdocx::Result<()> {
    let doc = parse("notes.sdocx")?;

    println!("{} page(s)", doc.pages.len());

    for page in &doc.pages {
        for stroke in &page.strokes {
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

## Format Documentation

Samsung Notes `.sdocx` files are ZIP archives containing binary stroke data, metadata, and page definitions. The [`notebooks/`](notebooks/) directory contains Jupyter notebooks that document the reverse-engineering process:

- [`01_container.ipynb`](notebooks/01_container.ipynb) — Archive structure and container parsing
- [`02_strokes.ipynb`](notebooks/02_strokes.ipynb) — Stroke decoding and coordinate parsing
- [`03_ink.ipynb`](notebooks/03_ink.ipynb) — Ink color and metadata extraction

## License

[GPL-3.0](LICENSE)
