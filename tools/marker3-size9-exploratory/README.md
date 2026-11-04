# Marker3 tool-1 size 9 exploratory controls

Two synthetic size-9 strokes exercise a short turn and a self-crossing path at 1× with fractional input coordinates. `capture.py OUTPUT` adapts the pinned native Model/Drawing and RTV1 bridge in `../marker3-zero`; it uses the same local dependencies and hash checks. The same retained Drawing vector is explicitly bridged into the selected native RT after the wrapper clears its slots. Mesa llvmpipe paints the transparent frame; this is not device or saved-note parity. Compressed native frames and caller cases, compact native receipts, hashes, and reader versions are retained here.

On these captures the baseline and candidate SVG, MuPDF and PDFium aggregate L1 errors respectively were 14811/43626/15464 versus 6689/37360/6347 for the short stroke, and 54252/71810/55975 versus 19325/46686/26118 for the crossing. Both modes cover all native support. The historical candidate was exploratory: two 1× cases do not establish a width interval, other phases, 2× behavior, arbitrary transforms, or device parity. These measurements do not define current admission; the current tool-1 page size range is 4–9 subject to other guards.

Historical replay used revision `4361933850036a3c2e5a333814f04b499f47fe6a` with a temporary size-bound change. The matrix was identity; fractional phase came from input coordinates. `replay-Cargo.lock` remains because other Marker3 export workflows consume it.

The experimental exporter, comparator and replay workflow were retired; native
inputs, frames, receipts and capture scripts remain. `SHA256SUMS` checks retained
files only. Retired source hashes remain in Git history.

From the repository root on the prepared host, use a fresh output directory:

```sh
.venv/bin/python tools/marker3-size9-exploratory/capture.py /tmp/marker3-size9-exploratory-fresh
```

Compare decompressed frames and native vectors with the retained receipt hashes.

Native recapture needs local Samsung DSOs, Unicorn, AArch64 binutils, Mesa/EGL and the ignored helpers described in `tools/marker3-zero/hashes.json`. The full runtime closure is not provided.
