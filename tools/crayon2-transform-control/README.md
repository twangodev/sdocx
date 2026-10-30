# Crayon2 translated document control

This fresh host Mesa Model/Drawing capture repeats the independent 160×160
source described in [the document holdout](../crayon2-document-holdout/README.md)
with a nonidentity view translation of (13, 9). The native uploaded vector has
the same SHA-256 as the independently sampled source; the observed clip matrix
and texture uniforms are recorded in `receipt.json`. The public document SVG
and direct PDF are produced from source points and pinned texture inputs, not
from the captured pixels. The retained frame is bottom-up native RGBA8.

Compared with that frame, resvg differs in 347 pixels, maximum RGB error 15,
and total absolute RGB error 577. Direct PDF differs in 461/14/1237 for MuPDF
and 461/14/1235 for PDFium. Compared with the identity control, the larger
maximum residual leaves the transformed edge behavior unresolved. These are
synthetic host results, not device or saved-note parity. No production paint
correction is established.

Run the capture and exports using the prerequisites and environment in the
document holdout README, replacing its capture with
`tools/crayon2-transform-control/capture.py` and test name with
`write_translated_document_grid`; use `tools/crayon2-transform-control/compare.py`
for comparison. Verify the fresh native JSON with
`tools/crayon2-transform-control/verify_native.py "$work_dir/native.json"`;
this checks the source, clip matrix, texture uniforms, texture uploads, blend
and draw evidence, and retained vector and frame bytes. The comparator checks the retained frame, vector, metrics and
export hashes. Native texture inputs and emulation dependencies remain local.
