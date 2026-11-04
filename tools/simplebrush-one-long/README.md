# SimpleBrush 1× long controls

The experimental public-export generator and replay callers have been retired. Export hashes and external SVG/PDF reader measurements below are historical observations, not current reproduction instructions or full source/build attestations. Native capture commands, inputs and receipts remain retained.

Two synthetic ObjectStroke controls use tool 2, 225 points at size 20 and 256 reversed points at size 8, with varying pressure and a fractional 1× matrix. The native Drawing/ARM64 emulation and host Mesa capture clears the target before painting. The complete generated geometry matches the Rust sampler byte for byte for both controls. Native compressed frames and vectors and source/export hashes are retained in `receipt.json`. This is not evidence of native page traversal, native PDF, device, or saved-note parity. The capture depends on project-local scratch inputs; hashes do not attest all transitive dependencies or the build environment.

The candidate extends the existing one-pixel profile for tools 2 and 3 to 256 points, retaining all other guards and budgets. A paired tool-3 capture for the 256-point input matches the complete tool-2 native geometry and frame, and its public SVG and direct PDF exports are byte-identical. Tool-3 coverage of other paths extrapolates. The SVG resvg alpha errors improve from 432,314 to 3,336 and 564,440 to 7,994 over generic output. The direct document PDF MuPDF matched-paper RGB errors improve from 486,218 to 33,234 and 621,815 to 63,234. PDFium errors improve from 481,471 to 48,806 and 617,605 to 115,914, but PDFium's unshifted grid differs from the fractional native matrix and includes phase error. Errors remain nonzero and other trajectories extrapolate.

To reproduce, create fresh `/tmp/sb1-225` and `/tmp/sb1-256` directories and run:

```sh
POINT_COUNT=225 PROBE_SIZE=20 python3 tools/simplebrush-one-long/capture.py fraction12 /tmp/sb1-225/native.json
POINT_COUNT=256 PROBE_SIZE=8 VARIANT=reverse python3 tools/simplebrush-one-long/capture.py fraction12 /tmp/sb1-256/native.json
```

 `generic.pdf` and `one.pdf` come from `render_document_pdf`; `paper.pdf` is generated from the same empty page.  The saved export hashes came from a source snapshot containing the candidate gate; they do not attest the full build snapshot.

For the paired tool-3 control, repeat the 256-point capture and export with `--tool 3` and `SDOCX_TOOL=3`, respectively; the native geometry and full frame hashes and all export hashes match the corresponding tool-2 control. The fixture-backed Rust test checks both tool-2 geometry streams during ordinary testing.

A 320-point reversed size-8 tool-2 control extends the 1× gate to 320 for tool 2. The complete native vector matches the sampler; SVG alpha error falls from 707,892 to 10,010, matched-paper MuPDF RGB error from 779,807 to 78,058, and PDFium RGB error from 774,571 to 143,015. PDFium uses an unshifted grid. The retained frame, vector and [receipt](320-receipt.json) support this synthetic host comparison; other newly admitted inputs extrapolate. Reproduce with the commands above substituting `320` for `256`.

 The profile SVG and PDF are 467,629 and 2,265,187 bytes, versus 36,528 and 3,756 for fallback.
