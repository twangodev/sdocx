# Crayon2 binary32 interpolation check

The retained identity-view 192×192 recompiled-shader controls match the
triangle-plane candidate for all 36 original and 24 holdout axis observations
when its multiply-add uses libc `fmaf`, which rounds once to binary32. This
replaces the earlier float64 approximation for this check; it does not establish
which instructions Mesa executed.

A new synthetic 160×160 native Model/Drawing capture uses view translation
(13, 9), the source and native setup from the translated document control, and
isolates records 20, 100, and 200. Recompiled fragment shaders encode the low
24 bits of each varying; the high byte is inferred from its expected range.
The retained receipt covers 11 pixels and 22 axis observations. The existing
triangle-plane candidate with `fmaf` matches 21/22. Fusing the vertex clip
multiply-add changes the result to 17/22, so that intervention does not justify
changing the Rust renderer. The separate translated record 139 diagnostic
shows the candidate selecting background texel (113, 4), while the encoded
varyings and binary32 scale/wrap calculation select (114, 3), whose texture
value matches the diagnostic background sample. Recompiled shader results do
not prove the unmodified shader's intermediates. These are host-only controls,
not device or saved-page parity. Further view translations remain untested.

Run from the repository root:

```sh
.venv/bin/python tools/crayon2-f32-interpolation/verify.py
.venv/bin/python tools/crayon2-f32-interpolation/translated_model.py
.venv/bin/python tools/crayon2-f32-interpolation/translated_model.py --fused-vertex
.venv/bin/python tools/crayon2-f32-interpolation/record139.py
```

For fresh native inputs use the environment and dependencies documented in
`tools/crayon2-document-holdout/README.md` and capture each combination of
records 20, 100, 200 and modes `xuv`, `yuv` with
`tools/crayon2-transform-control/intervention.py`. Name the resulting files
`xuv20.json`, `yuv20.json`, etc. in one directory and pass that directory to
`tools/crayon2-f32-interpolation/collect.py` to verify against the receipt.
The compact receipt keeps frame hashes and observed values rather than the
large reproducible capture JSON files. SVG and direct PDF behavior remains as
reported in the translated document control; no paint correction is established.
