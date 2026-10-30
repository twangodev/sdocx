# Crayon2 translated document control

This fresh host Mesa Model/Drawing capture repeats the independent 160×160
source described in [the document holdout](../crayon2-document-holdout/README.md)
with a nonidentity view translation of (13, 9). The native uploaded vector has
the same SHA-256 as the independently sampled source; the observed clip matrix
and texture uniforms are recorded in `receipt.json`. The public document SVG
and direct PDF are produced from source points and pinned texture inputs, not
from the captured pixels. The retained frame is bottom-up native RGBA8.

With the Mesa viewport FMA correction, resvg differs from that frame in 342
pixels, maximum RGB error 3, and total absolute RGB error 478. Direct PDF differs
in 459/3/1145 for MuPDF and 459/3/1142 for PDFium. The previous receipt
retains the baseline 347/15/577, 461/14/1237 and 461/14/1235 respectively.
The independent 160×160 identity and 192×192 controls retain their prior reader
metrics. These are synthetic host results, not device or saved-note parity.

The Crayon2 record 139 original paint shader's Mesa JIT diagnostic shows the
viewport scale and translation in `llvm.fmuladd`, with `vfmadd213ps` in the
assembly; compact excerpts and the diagnostic hash are in
[viewport-ir-receipt.json](viewport-ir-receipt.json). The correction applies
that operation before triangle interpolation. It selects the diagnostic texel
(114, 3), and the retained translated UV controls match 22/22 axes; run
`tools/crayon2-f32-interpolation/viewport_verify.py` to repeat the checks.
This is evidence about the captured host pipeline, not other GPUs.

Run the capture and exports using the prerequisites and environment in the
document holdout README, replacing its capture with
`tools/crayon2-transform-control/capture.py` and test name with
`write_translated_document_grid`; use `tools/crayon2-transform-control/compare.py`
for comparison. Verify the fresh native JSON with
`tools/crayon2-transform-control/verify_native.py "$work_dir/native.json"`;
this checks the source, clip matrix, texture uniforms, texture uploads, blend
and draw evidence, and retained vector and frame bytes. The comparator checks the retained frame, vector, metrics and
export hashes. Native texture inputs and emulation dependencies remain local.

## Isolated translated stamp intervention

In the historical pre-correction baseline, at SVG pixel (52, 102), native
record 139 alone renders RGB (60, 146, 215), while the public document SVG
gives (62, 139, 207). The original shader's
isolated paint draw and recompiled fragment diagnostics are retained in
`intervention-receipt.json`. The recompiled diagnostic reports point 255 and background
111. The historical baseline calculation in `model139.py` selects point 255 and
background 41 at (113, 4). The expected adjacent texel (114, 3) has value 111;
the observed value alone does not uniquely identify a texel or establish the
original shader sample.
Packed varying probes encode the low float bits; their high bytes are inferred
from expected coordinate ranges. Both axes are at a texture-cell boundary.
The interventions recompile the shader and do not establish original shader
intermediates or a general correction.

Use the environment from the document holdout, then run
`intervention.py 139 MODE` for MODE `paint`, `point`, `background`, `xfrac`,
`yfrac`, `xuv`, or `yuv`. Output is native capture JSON; read the lower-left
RGBA pixel (52, 57). `model139.py` requires NumPy and the hash-gated native
texture files described in the holdout. Its calculation is a diagnostic of
this selected sample, not a separate production implementation.

To reproduce and verify every mode (with the holdout environment already set):

```sh
work_dir=$(mktemp -d /tmp/sdocx-crayon-intervention.XXXXXX)
for mode in paint point background xfrac yfrac xuv yuv; do
  .venv/bin/python tools/crayon2-transform-control/intervention.py 139 "$mode" > "$work_dir/$mode.json"
done
.venv/bin/python tools/crayon2-transform-control/verify_intervention.py "$work_dir"
.venv/bin/python tools/crayon2-transform-control/model139.py
```

The verifier checks the source, selected native record, native dependency and
host library hashes, shader source hashes, uniforms, textures, blend, draw,
frame and selected pixel against the receipt.
