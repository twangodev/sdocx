# SimpleBrush size-three alternate trajectory

The experimental public-export generator and replay callers have been retired. Export hashes and external SVG/PDF reader measurements below are historical observations, not current reproduction instructions or full source/build attestations. Native capture commands, inputs and receipts remain retained.

A fresh synthetic 64-point tool-0, size-3 Drawing capture uses the alternate pressure-varying trajectory and fractional 2× transform. The public export fixture checked its complete native vector byte for byte. The existing 2× route reduced resvg alpha error from 531,930 to 14,169, MuPDF matched-paper RGB error from 578,720 to 61,389, and PDFium unshifted RGB error from 581,037 to 99,945. Reader versions and export hashes are in `receipt.json`. This extends evidence within the existing gate; it does not justify admitting additional sizes, phases, or lengths, and does not establish device or saved-page parity.

 The PDFium comparison uses `tools/simplebrush-v209/pdfium.py`, which reuses the v208 comparator with an explicit version 150.0.7869.0 check; it uses an unshifted grid. The capture relies on project-local native dependencies that are not fully pinned. The compressed complete native receipt and frame are retained here. This clean-source replay passed. The retired verifier replayed retained native data; it does not run a fresh native capture. Comparator scripts come from the archived source revision.  Reader binaries and Python packages are external prerequisites; their versions are checked by the comparators.

For native recapture, create a fresh output directory and run `POINT_COUNT=64 VARIANT=alternate PROBE_SIZE=3 python3 tools/simplebrush-v208/capture.py fraction12 /tmp/NEW/native.json --tool 0`.
