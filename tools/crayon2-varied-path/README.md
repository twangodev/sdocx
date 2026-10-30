# Crayon2 changed-path paint diagnostic

This synthetic native Model/Drawing control uses V2, size 10, ARGB `0x80336699`, points `(0,0), (8,6), (18,-4)`, and pressures `0.3, 0.6, 1`. The Drawing output joins the native queued RTV1 upload and host Mesa draw. The transparent and opaque captures retain source getters, native vector, shader/GL evidence and RGBA readbacks. The 303-record vector SHA-256 is `4e7d8107c77d80a2d7fa9d16816c7fa994109cd692a2c38a51d807f4b1a15ec9`.

The V16 SVG paint prototype consumes that captured vector as input. On the transparent target resvg 0.47 differs from native at 134 pixels, with maximum premultiplied channel error 4/255 and aggregate RGBA error 432. This tests paint transport only: no Rust source-to-record or public SVG/PDF export is exercised. The opaque capture is retained but was not compared. No direct PDF implementation of this arithmetic-filter candidate is established. This is not saved-note or device parity.

`capture.py --_capture-background transparent` and `opaque` reproduce the two separate captures. Run from the repository root with `.venv/bin/python`, `PYTHONPATH=tmp/conformance-cleanup/native-capture-a241793/conformance:scratch/apk-analysis-runtime/python` and `LD_LIBRARY_PATH=scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu`. It requires ignored local native DSOs, emulator, archived helper tree and the original project paths; the archive helpers are hash-pinned by the script but their upstream acquisition and the full runtime environment are not established here. The capture writes its JSON to standard output and does not create intermediate files.

For the SVG comparison, run `.venv/bin/python tools/crayon2-varied-path/prepare.py`; it prints a newly created private directory. Set `work` to that path, then build and run `$work/generator/Cargo.toml` with `generate $work/output k4-zero`, using the shared Cargo environment specified in AGENTS.md. Render `$work/output/80336699-transparent.svg` with the existing `sdocx-render-svg-047` helper, and run `.venv/bin/python tools/crayon2-varied-path/compare.py <rendered.png>`. The preparation depends on the ignored prior V16 source, observer receipt, textures and archived original fixture; these are not a clean-checkout reproduction setup. The comparator verifies the native vector and frame hashes and the measured error.

The receipt distinguishes the native bottom-up GL readback hash from the top-down premultiplied native and rendered comparison hashes. The caller owns the private directory and may remove it after the comparison.

A Rust regression fed the same source points, pressures, size and captured host profile into `ink::crayon2::sample`. It produced 303 records with the identical native vector SHA-256 above (`cargo test -p sdocx --features render changed_path_paint_capture_matches_source_derived_records --lib`, one test passed). This joins the existing bounded source geometry sampler to this paint control by exact record identity; the SVG experiment itself still consumed the captured vector and public export remains untested.

## Experimental standalone host API

With `render`, `render_crayon2_host_svg` accepts a `Stroke` and explicit
`Crayon2HostMaterial`: captured host phase tables, 10×30 point and 150×150
background texture planes, projection, color and background. Normal document
rendering remains generic. The sampler requires Crayon2 V2, stylus tool 2,
curve enabled, explicit stroke type zero and native pen size. The material
rejects non-axis-aligned projections and nonzero `k4`, limits dimensions to
4096, caps aggregate full-viewport filter area at 33,554,432 pixels and SVG
size at 32 MiB. Reader upscaling may require additional resources.

The matched source control uses `(64,64), (72,70), (82,60)` with pressures
`0.3, 0.6, 1`. Its 303 sampled records match the retained native upload byte
for byte. The standalone SVG differs from native in 174 pixels (maximum
channel error 34, RGB absolute error 732). SVG-page PDF differs in 363 pixels
(82 / 22,346) in MuPDF and 314 (75 / 15,348) in PDFium. These nonzero errors
and the single synthetic host control do not establish arbitrary-input,
Android, saved-page or direct-document PDF parity.

Reproduce from the repository root, using the shared Cargo environment in
AGENTS.md. The ignored writer verifies the native texture hashes; those local
native assets are not distributed in a clean checkout. Python requires Pillow,
PyMuPDF and pypdfium2 (locally the latter is under `/tmp/pdfium-lib`).

```sh
export SDOCX_CRAYON2_TEXTURE_DIR="$PWD/tmp/vector-parity-plan/crayon2-native-next/native-uploads"
export SDOCX_CRAYON2_SVG=/tmp/crayon-host.svg
cargo test -p sdocx --features render write_stroke_material_svg --lib -- --ignored
cargo run -p sdocx --features pdf --example crayon2_host_export --   /tmp/crayon-host.svg /tmp/crayon-host.png /tmp/crayon-host.pdf
PYTHONPATH=/tmp/pdfium-lib .venv/bin/python tools/crayon2-varied-path/compare_host.py   /tmp/crayon-host.svg /tmp/crayon-host.png /tmp/crayon-host.pdf
```

The comparator checks all three reader metrics against the retained matched
native frame. [host-api-receipt.json](host-api-receipt.json) records the source,
export and fixture hashes and reader versions. Remove temporary exports after
comparison. The native capture was retained from the prior matched-source
regression; this API validation regenerates exports, not the native runtime.
