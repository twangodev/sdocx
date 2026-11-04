# Crayon2 host paint diagnostics

These synthetic Crayon2 V2 controls use size 10, ARGB `0x80336699`, pressures `0.3, 0.6, 1`, and either points `(0,0), (8,6), (18,-4)` or matched translated source points `(64,64), (72,70), (82,60)`. The native captures retain source getters, the 303-record upload, shader/GL evidence and host-Mesa RGBA frames. Rust reproduces both record streams byte for byte. The sampler seeds randomness from the first source x coordinate: translating sampled output is not equivalent to translating source points. Use the matched capture for source-to-export comparisons.

## Experimental standalone host API

With `render`, the experimental `render_crayon2_host_svg(Stroke, Crayon2HostMaterial)` route accepts explicit phase tables, 10×30 point and 150×150 background texture planes, projection, color and background. Normal document rendering remains generic. The sampler requires the Crayon2 name, V2 setting, stylus tool 2, curve enabled, explicit stroke type zero and native pen size. The material rejects non-axis-aligned projections and nonzero `k4`, limits dimensions to 4096, aggregate full-viewport filter area to 33,554,432 pixels, and SVG size to 32 MiB. Reader upscaling can require more resources.

The matched standalone SVG differs from native in 174 pixels (maximum channel error 34, RGB absolute error 732); SVG-page PDF conversion differs in 363 pixels (maximum 82, total 22,346) in MuPDF and 314 (75 / 15,348) in PDFium. These unresolved errors and one synthetic host control do not establish arbitrary-input, Android, saved-note, or direct-document PDF parity. [Host API receipt](host-api-receipt.json) records hashes and reader versions.

The historical export comparison used local native textures and reader tools; the export generator has been retired.

## Retained controls

Rust regressions reproduce both captured record streams. Generic fallback retains
visible error against the matched translated control. Historical filter-bound,
soft-mask and clipping experiments improved selected measurements but did not
establish a general PDF correction; their scaffolding has been removed.

## Native capture and generic comparison

Native capture requires ignored DSOs, emulator, archived helpers and original project paths. Helpers are hash-pinned; their acquisition and the full runtime environment are not established here. Capture JSON goes to stdout:

```sh
export PYTHONPATH=tmp/conformance-cleanup/native-capture-a241793/conformance:scratch/apk-analysis-runtime/python
export LD_LIBRARY_PATH=scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu
.venv/bin/python tools/crayon2-varied-path/capture.py --_capture-background transparent
.venv/bin/python tools/crayon2-varied-path/capture.py --_capture-background opaque
.venv/bin/python tools/crayon2-varied-path/capture.py --_capture-background opaque --translated-source
```

The matched input/vector/frame is retained in `crates/sdocx/tests/assets/crayon2-v2-native/matched-opaque.json.gz`. For generic public comparison, use a new output directory and put `sdocx-render-svg-047` on `PATH`:

```sh
cargo run -p sdocx --features pdf,serde --example crayon2_varied_public_probe -- /tmp/crayon2-public
sdocx-render-svg-047 /tmp/crayon2-public/public.svg /tmp/crayon2-public/public.png
PYTHONPATH=/tmp/pdfium-lib .venv/bin/python tools/crayon2-varied-path/public_compare.py /tmp/crayon2-public
```
