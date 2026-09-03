# sdocx web

This is a static SvelteKit application for `sdocx.twango.dev`. It provides:

- a local `.sdocx` converter with SVG, PNG, sanitized JSON, and ZIP exports;
- a commit-to-commit regression workspace backed by the locked compatibility manifest;
- structural checks plus split, swipe, and difference views for renderer output.

User-selected documents stay in the browser. The compatibility runner downloads
public fixture pairs directly from Hugging Face when requested. There is no
application server, persistence layer, or processing API.

## Development

Install Bun dependencies and make sure `wasm-pack` plus the
`wasm32-unknown-unknown` Rust target are available:

```sh
bun install
bun run dev
```

`bun run dev` generates the runtime WASM package before starting Vite. The
generated `static/wasm/` directory is intentionally ignored by Git.

To compare two Git commits, prepare their renderer bundles before opening the
regression route:

```sh
bun run regression:prepare -- <from-ref> <to-ref>
```

The command builds both commits into ignored `static/renderers/` assets and
writes the manifest consumed by `/regressions/<from>/vs/<to>`. The page runs the
same public compatibility corpus through both WASM modules entirely in the
browser. CI prepares the pull request base/head pair, or the previous/current
commit for branch builds, without external object storage.

Run the complete local checks with:

```sh
bun run check
bun run test
bun run build
bunx playwright install chromium firefox webkit
bun run test:e2e
```

## Deployment

`wrangler.jsonc` contains an assets-only Workers configuration: it has no
JavaScript Worker entry point or service bindings. CI builds WASM and the static
site once, runs browser tests against that output, uploads `web/build` as an
artifact, and deploys that exact artifact after a successful push to `main`.

The repository must provide `CLOUDFLARE_API_TOKEN` and
`CLOUDFLARE_ACCOUNT_ID` as GitHub Actions secrets. A local deployment can be
started with `bun run deploy`, but it is not part of normal development.
