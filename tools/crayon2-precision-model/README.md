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
