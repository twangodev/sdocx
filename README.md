# sdocx

[![CI](https://img.shields.io/github/actions/workflow/status/twangodev/sdocx/ci.yml?label=CI)](https://github.com/twangodev/sdocx/actions/workflows/ci.yml)
[![crates.io (sdocx)](https://img.shields.io/crates/v/sdocx)](https://crates.io/crates/sdocx)
[![npm](https://img.shields.io/npm/v/@twango/sdocx)](https://www.npmjs.com/package/@twango/sdocx)
[![docs.rs](https://img.shields.io/docsrs/sdocx)](https://docs.rs/sdocx)
[![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/twangodev/sdocx)
[![License](https://img.shields.io/crates/l/sdocx)](https://github.com/twangodev/sdocx/blob/main/LICENSE)

A toolkit for viewing, converting, and building with Samsung Notes (`.sdocx`) files.

- **SDK:** Rust and JavaScript/WebAssembly APIs for parsing and rendering.
- **Viewer:** [Browse, inspect, and export your notes](https://sdocx.twango.dev), with files processed locally in your browser.
- **CLI:** Export SVG, PNG, and PDF from the command line.
- **Fidelity:** Preserve supported vector content in SVG and PDF, with full Samsung Notes parity as the goal. [Current support](docs/rendering-support.md).

## Install

```sh
cargo install sdocx-cli      # CLI
cargo add sdocx              # Rust SDK
npm install @twango/sdocx    # JavaScript / WebAssembly
```

## CLI

```sh
sdocx-cli note.sdocx                      # SVG (default)
sdocx-cli note.sdocx -o note.png
sdocx-cli note.sdocx -o note.pdf --pages "1-3, 5"
```

## Rust

```rust
let doc = sdocx::parse("note.sdocx")?;
let layout = sdocx::layout_document(&doc);
println!("{} visible page(s)", layout.pages.len());
```

## JavaScript

```js
import init, { parse } from "@twango/sdocx";

await init();
const doc = parse(new Uint8Array(await file.arrayBuffer()));
console.log(doc.pages);
```
