# Crayon2 host UV precision diagnostic

This intervention replays isolated records 1, 100, 250 and 478 of the independent control using its native Drawing/Mesa route. `capture.py` recompiles the fragment shader to encode the low 24 bits of `floatBitsToUint` of the interpolated background UV or the UV multiplied by its scale into RGB. It keeps the original uniforms active with sentinel terms and writes opaque output. It does not observe the unmodified shader's intermediates. The high byte is reconstructed by selecting the float nearest the coordinate implied by the source SVG; stamp 478 requires adding one texture repeat. Thus the decoding assumes the expected coordinate range and is not independent evidence of the exponent. Pixels matching the opaque background are excluded. This is host-only evidence, not Android parity.

`model.py` computes each axis's two projected vertices and background UVs with `numpy.float32`, then linearly interpolates them with float32 operations at pixel centers. Over 18 covered pixels (36 axis samples), this model matches 31 observed UVs; five differ by 2^-25. All 36 observed scaled UVs match float32 multiplication of the observed UV by the captured scale `1.7066665887832642`. At stamp 1 pixel (36,143), observed x UV is `0.3984375298023224` and scaled x UV is `0.6800000071525574`. The interpolation mismatch means the model is not an exact llvmpipe interpolation rule, and it does not establish a correction for SVG or PDF readers.

Reproduce with the environment and source-derived SVG commands in [the independent control](../crayon2-independent-control/README.md). Capture each mode and stamp into a private temporary directory, then run the model and numerical comparison with that SVG:

```sh
for stamp in 1 100 250 478; do
  for mode in xuv xsample yuv ysample; do
    .venv/bin/python tools/crayon2-precision-model/capture.py "$mode" "$stamp" > "$work_dir/$mode$stamp.json"
  done
done
.venv/bin/python tools/crayon2-precision-model/verify.py "$work_dir" "$work_dir/host.svg"
.venv/bin/python tools/crayon2-precision-model/analyze.py "$work_dir" "$work_dir/host.svg"
```

These scripts require the same ignored native inputs and helper environment described in the linked control. JSON capture files are reproducible intermediates and need not be retained.

The compact [receipt](receipt.json) retains each encoded covered pixel and full-frame hashes, alongside script, vector and source SVG hashes. The verifier checks all four coverage masks, native vector bytes, draw calls, frame sizes, observations, hashes and model counts. The recovered high byte remains inferred as described above.

## Interpolation arithmetic order

`interpolation_orders.py` evaluates the retained 36 decoded UV observations
against several arithmetic orders using the same projected f32 vertices. The
existing left-endpoint expression, a fused multiply-add, a right-endpoint
expression, its fused variant, and interpolation in double precision rounded
once to f32 each match 31/36 samples; all five mismatches are the same as in
the original model. Two slope-and-origin fused variants match 28/36 and 26/36.
The fused operation is emulated with Python double arithmetic over f32 inputs,
then rounded to f32. Float64 intermediate rounding can double-round in general, so this is not a universally exact f32 fused multiply-add implementation. This is a diagnostic of candidate arithmetic, not a claim
about the implementation in the running Mesa binary. It uses the retained
intervention receipt, including its inferred float high bytes, and the retained
native vector; it does not regenerate captures.

```sh
.venv/bin/python tools/crayon2-precision-model/interpolation_orders.py
```

These observations do not identify a source-derived vector paint correction.
In particular, the matched and independent controls' existing paired SVG/PDF
errors remain applicable; no arbitrary image shift or reader correction follows
from these arithmetic comparisons. More discriminating native observations
and independent varied-stamp export checks are needed before changing the
standalone material route.

The pinned Mesa 25.0.7-2+deb13u1 source offers a more specific next model.
The [source and validation receipt](triangle-plane-validation.json) records its
URL, SHA-256, relevant line ranges, and the diagnostic input/script hashes. Its
`src/gallium/drivers/llvmpipe/lp_state_setup.c` `init_args` and `calc_coef4`
construct a two-dimensional interpolation plane from three window-space
vertices, an f32 reciprocal determinant, and the pixel-center-adjusted first
vertex. This differs materially from the one-dimensional endpoint formulas
above. The selected triangle, actual processed vertex values, generated
instructions, and running binary's setup path must be established before
using those source operations as a prediction or vector correction. Merely
matching an arithmetic ordering to these observations would not establish it.

## Triangle-plane candidate and independent stamp observations

A fresh isolated stamp 100 capture records a 64-byte quad upload with SHA-256
`a39b6ddc1dbf16f1c1efb544817dc4ca4de6b575365af723878516f6afb558e4`.
The four uploaded position pairs are `(-1,-1), (-1,1), (1,-1), (1,1)`;
the draw is `GL_TRIANGLE_STRIP`, first 0, count 4, one instance. The candidate
models vertex projection using the shader operations and then models the Mesa
source's 2D determinant and plane coefficient construction in f32, including
pixel center adjustment. It chooses the geometric strip triangle, and uses
float64 multiply-adds rounded to f32 to approximate fused plane evaluation; float64 intermediate rounding can double-round, so this is not a universally exact f32 fused multiply-add. The chosen first vertex order
and fused execution are candidate assumptions; captured shader outputs do not
independently expose setup coefficients or machine instructions.

The candidate matches all 36 previously observed UV axis values, including
all five endpoint-model mismatches. Fresh diagnostic captures of stamps 20,
200 and 350 provide 12 additional covered pixels, or 24 UV values; the same
candidate matches all 24. Other vertex permutations also fit some or all
observations, so these controls do not uniquely prove the setup order. The
fragment shader is recompiled and the high float byte is inferred as above;
these are host intervention results. The result is not an SVG or PDF paint
correction, and a reader at another output scale cannot be assumed to sample
the same framebuffer pixel centers.

The [compact holdout receipt](triangle-plane-holdout.json) retains encoded
pixel observations and frame hashes. With the native environment above, capture
`xuv` and `yuv` for each of stamps 20, 200, 350 to `xuv20.json`, etc., then run:

```sh
.venv/bin/python tools/crayon2-precision-model/collect_holdout.py "$work_dir"
.venv/bin/python tools/crayon2-precision-model/triangle_plane.py
.venv/bin/python tools/crayon2-precision-model/triangle_plane.py \
  tools/crayon2-precision-model/triangle-plane-holdout.json
```

The model prints the matches and mismatches across candidate vertex orders and
fused versus non-fused evaluation. The native capture verifies its vector,
draw, frame size and coverage during receipt comparison. Shader stage probes,
processed vertex values and setup machine instructions remain separate from
the predicted f32 vertex and coefficient arithmetic.

The [quad receipt](quad-receipt.json) retains the actual 64 upload bytes,
all four vec4 inputs, the isolated source vector digest and draw arguments.
With the capture environment above, regenerate stamp 100 and verify it:

```sh
.venv/bin/python tools/crayon2-independent-control/capture-stamp.py 100 > "$work_dir/quad100.json"
.venv/bin/python tools/crayon2-precision-model/verify_quad.py "$work_dir/quad100.json"
```

## Fixed output-grid experiment

`output_grid.py` generates an SVG containing unit vector rectangles, one for
each covered stamp sample on the **192 by 192** output grid. The rectangle
opacity comes from the source point/background texture planes and the candidate
triangle-plane interpolation; overlapping rectangles are composed in stamp
order. The Rust ignored test supplies records sampled from the independent
stroke inputs. The generator does not read the native vector or framebuffer.
It is a bounded diagnostic for this exact projection, color and output grid,
not a general SVG material or a scale-independent vector representation.
Resizing the result does not replay native sampling. Point interpolation,
coverage and blending are candidates, and the plane's chosen order and
float64 emulation retain the limits above.

A fresh independent native capture reproduced the retained vector and frame
hashes using `verify_native.py`. The [receipt](output-grid-receipt.json)
records resvg, MuPDF and PDFium comparisons with that native frame for the
public standalone SVG and public SVG-page PDF. The respective differing pixel /
maximum channel error / total RGB error counts are 313 / 102 / 4803,
414 / 102 / 5297, and 415 / 102 / 5274. This is lower PDF total error than
the prior filter transport, but errors remain; the SVG has more differing
pixels than before. This does not establish device, arbitrary source, scaled
output, or direct `render_document_pdf` fidelity.

The generator requires the ignored local directory
`tmp/vector-parity-plan/crayon2-native-next/native-uploads` and its two files:
`g_Crayon2PointPtn.source.bin` (SHA-256
`673423c6fdae1ba344c452deefeb51b7b11365fead0bfbdd4b345eed4027f405`)
and `g_Crayon2BgPtn.source.bin` (SHA-256
`2df7013777862212381e424903db7b1194379db9c6fe6bb3dafa71a56573b2f3`).
They are local native capture dependencies, absent from a clean checkout. The
script verifies both hashes. Reproduce with these inputs and the readers as in
the independent control; receipt hashes use semantic SVG, PNG, and PDF keys
independent of the temporary filenames:

```sh
work_dir=$(mktemp -d /tmp/sdocx-crayon-grid.XXXXXX)
export CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
export SDOCX_CRAYON2_RECORDS="$work_dir/records.bin"
cargo test -p sdocx --features render --lib write_independent_source_records -- --ignored
.venv/bin/python tools/crayon2-precision-model/output_grid.py "$work_dir/records.bin" "$work_dir/grid.svg"
cargo run -p sdocx --features pdf --example crayon2_host_export -- "$work_dir/grid.svg" "$work_dir/grid.png" "$work_dir/grid.pdf"
PYTHONPATH=/tmp/pdfium-lib .venv/bin/python tools/crayon2-precision-model/compare_output_grid.py "$work_dir/grid.svg" "$work_dir/grid.png" "$work_dir/grid.pdf"
```

## Rust fixed-grid control

The test-only `ink::crayon2::grid_test` samples Rust-produced stroke records
with an explicit axis-aligned clip transform, viewport, RGBA and texture
planes. It emits transparent ordered rectangles, leaving the prior to its
caller. Its checked work limit is capped at one million samples. This remains
an output-grid experiment, not a document renderer profile: rotation,
perspective, arbitrary saved metadata and scalable replay are not established.
It is compiled only for tests and does not enable public renderer dispatch.
The generator reads source stroke inputs and the two hash-checked texture
planes, never the captured native vector or framebuffer; only the comparison
script reads the native framebuffer.

The ignored `write_independent_rust_grid` test produces the independent 192 ×
192 source control with the same hash-checked local texture inputs above.
The [receipt](rust-output-grid-receipt.json) compares resvg, MuPDF and PDFium
against the native frame: respectively 287 / 128 / 1239, 410 / 128 / 1869,
and 412 / 128 / 1861 differing pixels / maximum channel error / total RGB
error. These differ from the Python experiment; this control has not established
why, and does not prove numerical equivalence to that implementation. The PDF
is the SVG-page export, not the direct document PDF.
The checked readers are resvg 0.47, PyMuPDF 1.28.2 and pypdfium2 5.9.0
(PDFium 150.0.7869.0); the export example and dependency lockfile are tracked.

```sh
export CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
export SDOCX_CRAYON2_GRID_SVG=/tmp/crayon-grid-rust.svg
cargo test -p sdocx --features render --lib write_independent_rust_grid -- --ignored
cargo run -p sdocx --features pdf --example crayon2_host_export -- /tmp/crayon-grid-rust.svg /tmp/crayon-grid-rust.png /tmp/crayon-grid-rust.pdf
PYTHONPATH=/tmp/pdfium-lib .venv/bin/python tools/crayon2-precision-model/compare_output_grid.py /tmp/crayon-grid-rust.svg /tmp/crayon-grid-rust.png /tmp/crayon-grid-rust.pdf
```

A controlled arithmetic comparison identifies the Rust/Python discrepancy:
Python promotes each already-rounded binary32 texture coordinate to binary64
for the final multiplication by the texture dimension before flooring. Rust
performs that multiplication in binary32. Changing only those final
multiplications in Rust reproduces the Python receipt exactly, including SVG,
PNG and PDF hashes and all three reader metrics. The binary32 version has
lower total RGB error but a larger worst pixel. Its worst resvg pixel is
(75,148), where native RGB is (167,84,64) and the candidate leaves the prior
(64,128,192); nearby pixels also differ. The captured scaled UV is binary32,
but the precision of the texture unit's dimension multiplication is not
established by that observation. Neither aggregate comparison alone proves
which boundary rule the native texture unit uses; the targeted intervention
below provides additional evidence. No production dispatch follows from this
experiment.

A subsequent isolated-stamp intervention resolves that pixel for this host
control. Four records (442, 443, 456 and 462) cover it. Recompiled shader
captures report identical scaled background coordinates in all four; their
low 24 bits, with the high byte inferred from the candidate coordinate range,
decode as x `0x3f8369d0` and y `0x3f4962fc`. The point sample is zero for the
first three and 241 for record 462. All four report background sample 69;
the texture has 69 at repeated texel (3,117), versus 0 at (4,117), 94 at
(3,118), and 20 at (4,118). The binary64 final product selects (3,117), while
binary32 rounding selects (4,117). Its record 462 alpha is 217, and the
binary64 grid now reproduces native RGB (167,84,64) at this pixel. The
[receipt](worst-pixel-receipt.json) retains the pixel observations and frame
hashes; the shader was recompiled and the coordinate high bytes are inferred.
The test-only Rust grid retains the lower-total-error binary32 candidate. This does not establish the
native implementation's internal precision or a general rule: the aggregate
independent-control error increases to the Python receipt above, and other
interpolation or sampling differences remain unresolved. The public standalone
and direct document renderers remain unchanged.

To reproduce, set the native capture environment from the independent control
above, then write the exact filenames consumed by the verifier:

```sh
work_dir=$(mktemp -d /tmp/sdocx-crayon-worst.XXXXXX)
for index in 442 443 456 462; do
  for mode in xsample ysample; do
    .venv/bin/python tools/crayon2-precision-model/capture.py "$mode" "$index" > "$work_dir/$mode$index.json"
  done
  for mode in point background alpha; do
    .venv/bin/python tools/crayon2-independent-control/shader-sample-index-probe.py "$mode" "$index" > "$work_dir/$mode$index.json"
  done
done
.venv/bin/python tools/crayon2-precision-model/verify_worst_pixel.py "$work_dir"
```

The verifier checks pinned scripts, source vector and texture hashes, the
selected native upload records, per-mode diagnostic shader hashes, draw calls,
complete framebuffer hashes, pixel values and texture candidates. Capture
outputs are large, reproducible intermediates.
