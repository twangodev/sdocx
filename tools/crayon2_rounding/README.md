# Crayon2 inline host rounding control

This experiment draws one synthetic, replayed Crayon2 RTV1 record on a fresh native target in two separate processes. `LP_NUM_THREADS=0` selects inline llvmpipe rasterization in the retained Mesa source: `lp_screen.c` reads the option and `lp_rast.c` runs `rasterize_scene` directly in the zero-thread branch. That source is host evidence, not an observation of a threaded renderer. The experiment changes only the caller's MXCSR rounding bits just before the actual `glDrawArraysInstanced` callback, then records MXCSR before and after the call and hashes the complete RGBA8 readback. No images are retained.

The compact receipt records all 12 differing channels, DSO and loaded Gallium hashes, and both full-frame hashes. It shows sensitivity to rounding in this mixed pipeline; it does not identify the stage responsible or establish the final float-to-byte rounding rule. Draw execution may be deferred until a later flush; the changed MXCSR remains set for the remainder of each short-lived process. It makes no claim about the default threaded path, saved-note admission, or device rendering. There is no production change or Rust regression: this control does not define a Rust behavior.

Run from the project root on the captured x86-64 host:

```sh
python3 tools/crayon2_rounding/capture.py > /tmp/crayon-rounding-receipt.json
```

The runner requires `cc`, the project `.venv`, the project-local Unicorn runtime at `scratch/apk-analysis-runtime/python`, native input DSOs, and the pinned V26 capture at `tmp/vector-parity-plan/crayon2-v26-upload-owner-join/capture_one_record.py`. The V26 script checks its own dependent native inputs and observer pins; this runner checks the V26 script hash and records the loaded native and host library hashes. These scratch inputs are ignored and absent from a clean checkout, so the receipt is reproducible only where that existing local capture environment is present. The subprocesses each start with a fresh native context and the script checks both draws, no reported GL operation errors, matching native and host library hashes, and the mode bits. V26's capture also validates its texture joins and framebuffer state.

Pinned consulted Mesa source hashes: `lp_screen.c` `c486c5a69583ef16b678f890c66bb444716f809dd021da066aba4d8faf83ed5b`, `lp_rast.c` `6faba59fd59c62241d7d2626a4af840b772f5b5206731f666fd6a1ff7990afeb`. Source files live in ignored `tmp/vector-parity-plan/mesa-precision-source-audit`; the captured loaded Gallium binary is separately hashed in the receipt. This source correspondence is not an independent proof of the running binary's exact control flow.
