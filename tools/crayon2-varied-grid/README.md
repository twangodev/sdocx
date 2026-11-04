# Crayon2 varied viewport and color control

This fresh synthetic native Model/Drawing and Mesa GLES host capture uses the
source points and pressure of the document holdout, but changes the target to
192×192, view translation to (7, 21), and ARGB to `997bd247`. The native
projection scale is 0.010416666977107525 and the texture scale is
1.7066665887832642 on both axes. The native vector matches the independently
sampled source used by the actual public document SVG and direct document PDF;
the native frame is never supplied to the renderer. The retained frame is
bottom-up RGBA8. This is host evidence, not Android or saved-note parity.

Against the native frame, the SVG rasterized by resvg differs in 331 pixels,
maximum RGB channel error 8, and total absolute RGB error 466. The direct PDF
rasterized by MuPDF differs in 523/7/1486; PDFium differs in 525/7/1453. These
residuals do not by themselves identify a paint correction. They do not prove
general transform or color parity. The receipt retains native uniforms,
texture allocations, blend, draw calls, hashes and comparison metrics.

Reproduce with the local native prerequisites and environment from
[the document holdout](../crayon2-document-holdout/README.md), substituting:

```sh
work_dir=$(mktemp -d /tmp/sdocx-crayon-varied.XXXXXX)
.venv/bin/python tools/crayon2-varied-grid/capture.py > "$work_dir/native.json"
.venv/bin/python tools/crayon2-varied-grid/verify_native.py "$work_dir/native.json"
export CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
```

The historical comparison used direct document PDF; its export generator has been retired. Native texture inputs and emulation dependencies are local,
hash-gated prerequisites and are not supplied by a clean checkout.
