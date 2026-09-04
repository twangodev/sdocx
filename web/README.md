# sdocx-web

Browser-only `.sdocx` viewer and converter for
[`sdocx.twango.dev`](https://sdocx.twango.dev). Documents never leave the
browser.

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
