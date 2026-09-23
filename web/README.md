# sdocx-web

Browser-only `.sdocx` viewer and converter for
[`sdocx.twango.dev`](https://sdocx.twango.dev). Documents never leave the
browser.

## PDF export

Choose **Export document → PDF** to download the current page or all pages in
one PDF. The worker calls the same Rust `sdocx` / Krilla PDF engine as the CLI:

```sh
sdocx-cli note.sdocx -o note.pdf
```

Handwritten strokes and shapes stay vector; embedded photos remain raster.
Export uses the document color mode and runs entirely locally. Roboto and
Roboto Mono fonts are downloaded from this site's own static assets on the
first PDF export, then embedded as subsets in the PDF. Characters outside
these fonts' coverage may be missing. The CLI can use additional `--font` files.
Generic font fallback can differ from the CLI's system fonts. Font provenance
and licenses are in `static/pdf-fonts/`.

The WASM API exposes `DocumentSession.add_pdf_font(bytes)` and
`DocumentSession.render_pdf(pageIndex, colorMode)`. Pass `undefined` for all
visible pages, or a zero-based index for one page; the result is a `Uint8Array`.
Supply TTF/OTF fonts before exporting typed text. WASM cannot load system fonts.

## Development

Requires Bun, `wasm-pack`, and the `wasm32-unknown-unknown` Rust target.

```sh
bun install
bun run dev
```

```sh
bun run check
bun run test
bun run build
bunx playwright install chromium firefox webkit
bun run test:e2e
```
