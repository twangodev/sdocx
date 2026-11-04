# Pencil2 synthetic public SVG and PDF injection

The experimental source patches, harnesses and comparators were retired. The
findings below are historical; native inputs, captures and receipts remain.
These prototype assertions and external-reader measurements are not claimed
as equivalent maintained Rust coverage.

The historical experiment patched revision `61fdc795adb2e2b075b6417ec9533f41cf884861`; v195 records input, native crop provenance and reader versions. The generic 2× control transformed its 1× output about the bottom-left corner.

The test-only environment switch for SVG inserts ordered black translucent unit rectangles at the normal stroke dispatch position. It derives stamps from the public stroke's saved settings and input channels through `prepare_stroke`, flipping top-down input y about the 192-unit page height. It does not embed the native image. It admits only the exact source record stream SHA-256 `770651fc1b27bfa8a73731f1a3e24cf1a37847e920581aa36855c5107835c2fe`, matching the retained native geometry record stream; failed admission uses the generic SVG path. That byte identity does not establish general source equivalence. The rectangles are fixed to the output grid. The 2x result samples the same stroke with a doubled draw matrix in the same 192 x 192 viewport, not a PDF viewer zoom or a newly sampled Drawing. SVG dispatch is disabled without the test switch.

The PDF test switch invokes `render_document_pdf`, obtains its normal sdocx PDF, and replaces the sole synthetic page's content and resources with an ordered luminosity mask generated from the same public stroke. This is a destructive, fixture-specific diagnostic, **not** a PDF stroke-position implementation: it discards original page content and cannot demonstrate ordinary composition, mixed colors, clipping or multi-object behavior. It is limited to one 192 x 192 page and one object, black source ink and the exact source record hash. No production admission is proposed. The 2x PDF is separately sampled in the same fixed viewport.

Before per-stamp frame allocations, this experiment checks at most 84 records, 16 MiB of frame bytes, and a 256-byte-per-grid-cell SVG staging estimate capped at 800 MiB. The SVG estimate is not an enforced measurement of emitted markup; the scene has no incremental output-byte guard in this experiment. PDF estimates the stream before generation and checks an additional 40-byte margin before each command; its stream cap is 128 MiB. The underlying sampler also has its own per-call pixel, record and sampling bounds. These are experimental fixed-grid limits, not a practical page-wide resource policy.

Opaque-white absolute alpha-equivalent error sums (maximum) against the retained native crop, 1x / 2x:

| output | resvg | MuPDF | PDFium |
| --- | --- | --- | --- |
| source SVG / synthetic injected PDF | 27 (2) / 73 (2) | 94 (4) / 341 (4) | 60 (3) / 242 (3) |
| generic public sdocx | 2882 (184) / 11828 (181) | 2738 (180) / 11128 (183) | 3153 (233) / 10604 (239) |

SVG is 17,301 / 61,721 bytes and injected PDF 27,228 / 37,701 bytes in this run. The native reference is the retained 62 x 22 crop; the comparison assumes zero outside based on its capture receipt, and does not recheck full-frame native support. Crop hashes, output dimensions, and the exact reader error results in the source SVG / synthetic injected PDF row were asserted by the retired runner. The generic control reported results without asserting them. This is narrow host evidence, not saved-page, device, or scalable vector parity.
