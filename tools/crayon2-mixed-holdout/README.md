# Crayon2 mixed-input host holdout

This synthetic native Model/Drawing and Mesa GLES capture uses points (46, 37), (69, 65), (103, 48), pressures 0.3, 0.6, 1, ARGB `b1ce538b`, a 192×192 opaque target with RGB (64, 128, 192), and view translation (13, 5). The independently sampled document vector matches the native upload byte for byte; the native frame is never supplied to the renderer. The capture is host evidence and does not establish Android or saved-note parity.

In the historical export comparison, actual document SVG rasterized by resvg differs in 360 pixels, maximum RGB channel error 2, and total absolute RGB error 517. The direct document PDF differs in 543/5/1495 in MuPDF and 541/5/1472 in PDFium. These residuals do not establish a paint correction. The receipt retains uniforms, texture allocations, blend and draw evidence, hashes and metrics; the retained frame is bottom-up RGBA8.

Use the native prerequisites and environment in [the document holdout](../crayon2-document-holdout/README.md), then:

```sh
work_dir=$(mktemp -d /tmp/sdocx-crayon-mixed.XXXXXX)
.venv/bin/python tools/crayon2-mixed-holdout/capture.py > "$work_dir/native.json"
.venv/bin/python tools/crayon2-mixed-holdout/verify_native.py "$work_dir/native.json"
```

The historical comparison used direct document PDF; its export generator and comparator have been retired. Native texture inputs and emulation dependencies are local, hash-gated prerequisites and are not supplied by a clean checkout.
