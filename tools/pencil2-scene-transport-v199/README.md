# Pencil2 source-grid scene transport prototype

Run `tools/pencil2-scene-transport-v199/run.sh`. It uses the v198 source-record generator on a pinned clean archive, compares its two native Drawing prefixes, then tests a separate bounded scene transport. The two object sequences come solely from freshly generated SVG rectangles, not captured output. It checks rectangle and encoded-byte limits at their exact boundaries before joining the scene, and renders the original and reversed order with resvg. The original matches the v198 SVG pixels; the reversed scene changes pixels.

This is a diagnostic, not a public rendering option. It buffers generated rectangle strings while measuring them, has no document-wide resource accounting or direct PDF implementation, and does not support arbitrary caller matrices. Its retained comparison is one 192×192 colored host-Mesa scene. The v198 native comparison and direct PDF experiments document substantial error and their separate limitations; this transport test does not establish device or saved-document parity.

`PYTHONPATH=/tmp/sdocx-marker3-v133-readers tools/pencil2-scene-transport-v199/pdf-run.sh` builds a second isolated pinned snapshot. Its typed native-ink markers replace only the two tested stroke slots, retaining the page and their order. Each colored stamp is represented by ordered alpha rectangles in a transparent PDF Form. The source record hashes are checked before rendering; captured pixels are comparison targets only. The runner tests the same blue backdrop and 1× independent Drawing prefixes against MuPDF 1.28.2 and PDFium 156.0.8076.0. Absolute RGB error (sum / max / differing channels):

| Prefix | Marker MuPDF | Marker PDFium | Generic MuPDF | Generic PDFium |
| --- | --- | --- | --- | --- |
| First | 477 / 5 / 221 | 267 / 3 / 193 | 911 / 20 / 226 | 1804 / 29 / 240 |
| Both | 8481 / 25 / 1126 | 4304 / 12 / 1061 | 38410 / 140 / 1148 | 39294 / 140 / 1164 |

The hard-coded two-object harness bounds each field to 84 frames and each generated PDF Form stream to 2 MiB, and rejects unexpectedly large PDF output. These are not prevalidated aggregate document budgets: admission is tied to the two known source hashes, and scene-wide failure/fallback atomicity is not implemented. The measurements show considerable accumulated reader-dependent color error, and the PDF test does not establish a general colored scene profile, transparent backgrounds, arbitrary transforms or device parity.

The PDF runner checks both patch hashes and the pinned base inputs, creates a fresh temporary snapshot with separate marker and generic output directories, and removes it on exit. Its comparator checks compressed native-reference SHA-256 values, decoded and reader dimensions, and exact Pillow, MuPDF and PDFium versions; it rejects optimized Python because its comparisons use assertions. The clean rerun reproduced the table above. The reader environment must be supplied separately as shown; the runner does not install dependencies.
