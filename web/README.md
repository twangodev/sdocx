# sdocx-web

Browser-only `.sdocx` viewer and converter for
[`sdocx.twango.dev`](https://sdocx.twango.dev). Documents never leave the
browser.

## PDF export

Choose **Export document → PDF** to download the current page or all pages in
one PDF. Export runs locally, uses the document color mode, and preserves
handwritten strokes and shapes as vectors. Embedded photos remain raster images.
PDF generation loads only when requested. Typed text uses the PDF's standard
fonts; fonts and characters outside their supported character set may differ
from the browser preview. SVG blend effects may also differ.

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
