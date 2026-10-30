# Independent Crayon2 source control

The native wrapper adapts the hash-gated `crayon2-varied-path/capture.py` to a
new 192×192 viewport, path `(40,45), (53,28), (75,49)`, and ARGB `aab84c29`.
The actual native drawing and Mesa host paint route produce the retained
compressed native vector and framebuffer. The Rust regression samples those
source channels independently and compares all 479 records; the retained
vector and pixels are never supplied to the renderer.

The ignored `write_independent_stroke_material_svg` test calls the public
standalone source API using host phase tables and external native textures.
The resulting SVG and SVG-page PDF were rasterized with resvg, MuPDF and
PDFium. Their nonzero comparisons against the native framebuffer are recorded
in `receipt.json`. This is a bounded host control, not Android device or saved
note parity. PDF reader results demonstrate additional reader-dependent error.
The native vector regression fixture is packaged with the crate. Native capture
helpers, libraries, and material textures remain external local dependencies;
they are not present in a clean checkout.

## Reproduce

Use a fresh temporary directory so the capture and exports cannot overwrite
retained evidence. The native capture writes JSON to stdout. The wrapper pins
the base script hash and checks the exact substitution counts; the base script
checks its native dependencies. This command regenerates a native capture:

```sh
work_dir=$(mktemp -d /tmp/sdocx-crayon-independent.XXXXXX)
export PYTHONPATH="$PWD/tmp/conformance-cleanup/native-capture-a241793/conformance:$PWD/conformance:$PWD/scratch/apk-analysis-runtime/python"
export LD_LIBRARY_PATH="$PWD/scratch/apk-analysis-runtime/binutils-aarch64/usr/lib/x86_64-linux-gnu"
.venv/bin/python tools/crayon2-independent-control/capture.py > "$work_dir/native.json"
.venv/bin/python tools/crayon2-independent-control/verify_native.py "$work_dir/native.json"
```

Regenerate the source-derived exports and compare against the retained frame:

```sh
export CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
export SDOCX_CRAYON2_TEXTURE_DIR="$PWD/tmp/vector-parity-plan/crayon2-native-next/native-uploads"
export SDOCX_CRAYON2_SVG="$work_dir/host.svg"
cargo test -p sdocx --features render --lib write_independent_stroke_material_svg -- --ignored
cargo run -p sdocx --features pdf --example crayon2_host_export -- "$work_dir/host.svg" "$work_dir/host.png" "$work_dir/host.pdf"
PYTHONPATH=/tmp/pdfium-lib .venv/bin/python tools/crayon2-independent-control/compare.py "$work_dir/host.svg" "$work_dir/host.png" "$work_dir/host.pdf"
cargo test -p sdocx --features render --lib independent_source_matches_native_sampled_records
```

The comparator checks input and export hashes, reader versions, image dimensions,
the native framebuffer hash, and all three recorded metrics. Its exact export
hashes make this a check of the recorded toolchain; changed encoders or source
require a reviewed receipt update. On the fresh rerun the native vector and
compressed framebuffer matched the retained artifacts byte for byte.

## Corrected native texture scale

A fresh native replay retained `glUniform2fv` at location 6: both background
scale components are `1.7066665887832642`. The initial writer used the next
float value, `1.7066667`, without capturing that uniform. The writer now uses
the observed value. The native frame and geometry hashes are unchanged.

The corrected source API results are 286 / 124 / 5572 for resvg, 647 / 148 /
48085 for MuPDF, and 567 / 147 / 36679 for PDFium (differing pixels / maximum
channel error / total RGB error). The original results and provenance remain in
`receipt.json`; `corrected-scale-receipt.json` records the new exports. This
input correction reduces the SVG error but does not explain the remaining
error or establish a general paint fix. The matched 256-pixel control remains
174 / 34 / 732 in resvg; its bounded SVG-page PDF comparison remains 357 / 92 /
22490 in MuPDF and 314 / 75 / 15348 in PDFium.

## Isolated stamp diagnostic

`paint-state-receipt.json` retains shader sources, observed uniforms, uploaded
texture hashes, blend state, and draw calls from fresh captures of both the
matched and independent controls. Both fresh vectors and frames matched their
retained controls. The isolated independent stamp 1 is selected by replacing
and shortening the native vector immediately before upload; this intervention
is not a normal full-stroke capture. Its pixel at top-down (36,143) differs
from the corresponding standalone SVG stamp, showing the large residual does
not require overlap. The point lies inside the projected stamp rectangle, but
texture sampling, edge coverage and intermediate alpha remain unresolved.

To reproduce the native isolated capture, use the environment from the capture
instructions above and run:

```sh
.venv/bin/python tools/crayon2-independent-control/capture-stamp.py 1 > "$work_dir/stamp-1.json"
```

It depends on the same local native helpers and preserves the other wrapper
hash checks. Generate the source SVG with the source-derived export command above, then run
the strict comparator (the native capture is never passed to the renderer):

```sh
.venv/bin/python tools/crayon2-independent-control/verify-stamp.py \
  "$work_dir/stamp-1.json" "$work_dir/host.svg" \
  /tmp/sdocx-agent-target/debug/sdocx-render-svg-047
```

The comparator checks the source SVG digest and full stamp sequence, retains
only painted rectangle 1, verifies native vector and framebuffer against the
retained compressed frame, rasterizes the isolated SVG in a temporary directory,
and compares RGBA readback after flipping GL rows. It requires the same local
resvg 0.47 helper, and verifies its binary SHA-256 against the receipt. The
helper is a local dependency: its temporary source and Cargo manifest/lock hashes
are recorded in the receipt; the version is a manifest claim because the helper
has no runtime version interface. The result is one differing RGB pixel, maximum channel
error 42, total 93. This comparison is a host intervention, not a device or
saved-note fidelity result.
