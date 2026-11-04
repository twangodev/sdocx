# Independent Crayon2 source control

The native wrapper adapts the hash-gated `crayon2-varied-path/capture.py` to a
new 192×192 viewport, path `(40,45), (53,28), (75,49)`, and ARGB `aab84c29`.
The actual native drawing and Mesa host paint route produce the retained
compressed native vector and framebuffer. The Rust regression samples those
source channels independently and compares all 479 records; the retained
vector and pixels are never supplied to the renderer.

A historical export experiment called the public
standalone source API using host phase tables and external native textures.
The resulting SVG and SVG-page PDF were rasterized with resvg, MuPDF and
PDFium. Their nonzero comparisons against the native framebuffer are recorded
in `receipt.json`. This is a bounded host control, not Android device or saved
note parity. PDF reader results demonstrate additional reader-dependent error.
The native vector regression fixture is packaged with the crate. Native capture
helpers, libraries, and material textures remain external local dependencies;
they are not present in a clean checkout.

## Reproduce

Use a fresh temporary directory so the capture cannot overwrite
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

The source-derived export generator and comparison scripts have been retired. The maintained geometry regression remains available:

```sh
CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo test -p sdocx --features render --lib independent_source_matches_native_sampled_records
```

## Corrected native texture scale

A fresh native replay retained `glUniform2fv` at location 6: both background
scale components are `1.7066665887832642`. The initial writer used the next
float value, `1.7066667`, without capturing that uniform. The corrected historical writer used the observed value. The native frame and geometry hashes are unchanged.

The historical corrected source API results were 286 / 124 / 5572 for resvg, 647 / 148 /
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

It depends on the same local native helpers and preserves the other wrapper hash checks. The historical SVG comparator required a generated source SVG; its generator has been retired. The retained comparison differed at one RGB pixel, with maximum channel error 42 and total 93. This intervention does not establish device or saved-note fidelity.

## Isolated fragment sampling diagnostic

`shader-sample-probe.py` replays isolated stamp 1 with a recompiled fragment
shader, writing the selected intermediate as opaque grayscale. Small nonzero
sentinel terms keep the original uniforms active; this is an intervention and
is not a direct measurement of the original shader's intermediates. It uses
the same environment and native capture dependencies above:

```sh
for stage in point background alpha xfrac xcoord ycoord; do
  .venv/bin/python tools/crayon2-independent-control/shader-sample-probe.py "$stage" > "$work_dir/$stage.json"
done
```

At top-down pixel (36,143), the instrumented point and background samples are
255 and 76, and the computed alpha is 253, in RGBA8 grayscale. Separate SVG
filter-stage renders yield point 255, background 54, and product alpha 180.
The [compact receipt](shader-sampling-receipt.json) retains the measurements
and input hashes. The background mask image has 54 and 76 in adjacent columns 101 and 102;
the SVG mapping lands at approximately 101.999995 texels. The recompiled
shader's fractional-coordinate output rounds to zero at 8-bit precision,
consistent with sampling the adjacent column. A 0.00001-pixel horizontal
shift of all SVG background images reduces independent-control RGB error from
5572 to 5452; shifts of 0.0001 or 0.001 increase it to 6445. This does not
establish a general transform correction, and no production shift was applied.
