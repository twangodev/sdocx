# Fountain pen parity work

The target is Samsung Notes 4.4.45.37's native fountain pen behavior, including
visible ink, not only its centerline and radii. This work is incomplete.

## Implementation ownership

Fountain geometry lives in Rust and SVG previews/exports retain vector paths.
Native shading for vector output remains unfinished; opaque circle coverage is still the current vector
approximation. The regression test `fountain_svg_keeps_reconstructed_ink_as_vector_paths`
checks that a supported native profile does not emit embedded images.
After restoring vectors, the handwriting fixture's SVG has 2,769 paths and
zero images. Its PDF has 2,770 vector drawing objects (including the page
background) and zero image objects, checked with PyMuPDF. This verifies the
representation; it does not close the outstanding shading-parity gap.

The Rust rasterizer in `crates/sdocx/src/ink/raster.rs` provides a shader
comparison target and viewport-sized canvas replay. Replay calls that
implementation through `rasterize_fountain_ink` in the WASM bindings;
its TypeScript adapter supplies the viewport and displays returned RGBA pixels.
There are no production fountain GPU shaders.

The original APK shaders still run in the optional conformance harness, where
they are an independent reference for the Rust implementation. The harness
allows one coverage level of float32 interpolation/UNORM rounding difference;
passing it does not establish byte-exact output.

## Verified on 2026-09-25

- The saved V14 reconstruction now retains SmPath's direction vectors, including
  the small-vector normalization threshold, zero-vector substitution, and final
  stamp's retained direction. All five attributes are checked against the native
  synthetic fixtures in the existing Rust fountain conformance test.
- The available `handwritten.sdocx` has 2,769 V14 strokes and 56,713 stamps.
  Re-running the native saved-redraw oracle and the current Rust renderer gives
  exactly equal x, y, radius, direction x/y and original-sample boundaries.
- The Rust renderer implements scale-dependent extent, inner radius, subpixel
  alpha compensation, V14 texture gradient and maximum coverage within each
  stroke. Stroke opacity is applied once when compositing onto the page.
- The pixel harness exercises 960 synthetic cases: both backends, six radii,
  seven uniform and three unequal scale pairs, four directions, and single or
  overlapping stamps. Optional real strokes are individually fitted into a
  128-pixel target. It compares opaque white coverage, so does not establish
  Android driver parity, cross-stroke/page compositing or full color parity.
- The final rebuilt Rust/WASM adapter passes all 3,729 cases (960 synthetic
  plus 2,769 handwriting strokes). Twenty pixels differ from the APK shader
  reference, each by one coverage level; no case exceeds the tolerance of one.
  This result uses Chromium/SwiftShader and the current production adapter.

The native UV mapping matters: texture y follows the supplied tangent. An
earlier description called these transverse edges, which would produce the
wrong gradient orientation. The differential pixel test caught that mistake.
The reference must also use Canvas's retained transform, not the original
double-precision arguments to `setTransform`: this browser rounds those values.
Unequal inputs caused apparent errors of up to 16 coverage levels on real
handwriting. Matching the actual inputs eliminates those discrepancies.

## Reproduction

From the repository root, install the browser dependencies from the lockfile,
build the current WASM, and install the matching Playwright Chromium once:

```sh
cd web
bun install --frozen-lockfile
bun run build:wasm
bunx playwright install chromium
bun run vite --host 127.0.0.1 --port 5194
```

In another terminal at the repository root, run the checks below. The shader
extractor needs Python 3, `nm`, and the locally extracted, hash-pinned native
library (override its location with `--library`). The pixel runner uses Node.js
and the Playwright dependency installed in `web/`; it reports its tolerance,
maximum byte error, differing pixels, and up to ten failures, and exits nonzero
if any case exceeds the tolerance. `--help` prints its argument order.

```sh
cargo test --offline -p sdocx --all-features --lib fountain
python3 conformance/fountain_shaders.py --output /tmp/fountain-shaders.json
node conformance/fountain_pixels.mjs /tmp/fountain-shaders.json
# Optionally include real strokes, after comparing their geometry to the oracle:
cargo run --offline -q -p sdocx --features render,serde --example ink_geometry -- \
  tmp/stroke-conformance/handwritten.sdocx > /tmp/fountain-prepared.json
node conformance/fountain_pixels.mjs /tmp/fountain-shaders.json \
  http://127.0.0.1:5194 /tmp/fountain-prepared.json
```

The extractor verifies the native library hash and reads the exported shader
symbols through ELF load segments. No vendor shader source is committed.
The optional fourth positional argument to the pixel runner supplies native
Rust masks from the `fountain_raster` example. Its input is an array of
`{stroke, viewport}` objects, and its output must retain the geometry rows'
order. Omitting masks exercises the production WASM/browser adapter directly.

Rasterization limits stamp storage and each cropped RGBA output to 64 MiB;
the alpha mask uses at most 16 MiB. Browser copies and canvases use additional
memory. Replay surfaces loading/render errors rather than substituting solid ink.

## Remaining requirements

- Implement native shading in scalable vector SVG/PDF output without embedding
  raster ink. Validate preview/export appearance across scales against the APK.
  Keep the algorithm in Rust and avoid a second browser implementation.
- Broaden pixel comparisons to viewport/tile boundaries, colored/translucent
  ink and the actual native composite shaders.
- Measure dense-page replay latency and peak memory, including browser copies
  and canvases, and tune the allocation limits if needed.
- Characterize and implement the other saved profiles/settings that currently
  use approximations, including fixed width, non-stylus input, other historical
  versions and effects. Validate their selection against the native dispatcher.
- Check complete page compositing and Samsung reference exports. Saved redraw
  validation does not prove live prediction or temporary-tip behavior; those
  paths need their own native comparisons wherever the application exposes them.

Passing the current tests is a milestone, not completion of the parity goal.
