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
