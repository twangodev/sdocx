# Pencil2 source-grid scene transport prototype

The experimental source patches, harnesses and comparators were retired. The
findings below are historical; native inputs, captures and receipts remain.
These prototype assertions and external-reader measurements are not claimed
as equivalent maintained Rust coverage.

The retired source-grid prototype checked the v198 native Drawing prefixes, exact rectangle/byte boundaries and original/reversed SVG order. The original matched v198 SVG pixels; reversal changed pixels. Its separate PDF experiment replaced two stroke slots with ordered colored Forms, preserving the page. Historical 1× blue-backdrop RGB error (sum / max / differing channels), MuPDF 1.28.2 and PDFium 156.0.8076.0:

This is a diagnostic, not a public rendering option. It buffers generated rectangle strings while measuring them, has no document-wide resource accounting or direct PDF implementation, and does not support arbitrary caller matrices. Its retained comparison is one 192×192 colored host-Mesa scene. The v198 native comparison and direct PDF experiments document substantial error and their separate limitations; this transport test does not establish device or saved-document parity.

| Prefix | Marker MuPDF | Marker PDFium | Generic MuPDF | Generic PDFium |
| --- | --- | --- | --- | --- |
| First | 477 / 5 / 221 | 267 / 3 / 193 | 911 / 20 / 226 | 1804 / 29 / 240 |
| Both | 8481 / 25 / 1126 | 4304 / 12 / 1061 | 38410 / 140 / 1148 | 39294 / 140 / 1164 |

The hard-coded two-object harness bounds each field to 84 frames and each generated PDF Form stream to 2 MiB, and rejects unexpectedly large PDF output. These are not prevalidated aggregate document budgets: admission is tied to the two known source hashes, and scene-wide failure/fallback atomicity is not implemented. The measurements show considerable accumulated reader-dependent color error, and the PDF test does not establish a general colored scene profile, transparent backgrounds, arbitrary transforms or device parity.
