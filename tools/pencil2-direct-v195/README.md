# Pencil2 source-stamp export experiment

The experimental source patches, harnesses and comparators were retired. The
findings below are historical; native inputs, captures and receipts remain.
These prototype assertions and external-reader measurements are not claimed
as equivalent maintained Rust coverage.

This historical test-only fixed-grid experiment used revision `61fdc795adb2e2b075b6417ec9533f41cf884861`. Native output was used only as a comparison target. An isolated snapshot adds the patch and example to call the existing Rust source sampler and per-stamp fragment model. Each of 84 individual transparent stamp frames is transported either as a cropped PNG or as ordered unit rectangles. SVG and direct PyMuPDF PDF are written independently; the PDF is not the sdocx direct PDF renderer. The normal public sdocx SVG and direct PDF renderers supply the generic control. The 2x native capture changes the draw matrix on the same geometry stream; it is not a new Drawing sampling or device capture.

The selected input is the `v5_exact_combined` case in `conformance/pencil2-v14-native.json`, and the native crops and their capture limits are described in `conformance/pencil2-v14-device-profile/receipt.json`. The public control uses the corresponding top-down coordinates and 0.228 width, ramp pressure, and matching tilt/orientation channels. The 2x control transforms the public output about the bottom-left corner.

Absolute alpha error sums (maximum in parentheses), at 1x / 2x:

| Output | resvg | MuPDF | PDFium |
| --- | --- | --- | --- |
| Per-stamp PNG | 0 (0) / 0 (0) | 107 (4) / 402 (4) | 51 (3) / 171 (3) |
| Per-pixel vector rectangles | 36 (2) / 102 (3) | 98 (4) / 368 (6) | 3283 (190) / 13117 (181) |
| Generic sdocx | 2882 (184) / 11828 (181) | 2738 (180) / 11128 (183) | 3153 (233) / 10604 (239) |

The per-stamp images are raster, and the rectangles are tied to the output pixel grid. Neither provides scalable vector paint. PDFium's rectangle output is materially different. No general production or saved-page parity follows.

Source snapshot SHA-256: `pencil2.rs` 6b6dd7e52ecf776fabc3b311520ac4fbbddd61e6ae19a2e18ebbed5a3f481983; `device_raster.rs` b4de1f6e94f002ae7a5c83439e532255aad77e9bbd1ce76168b0c8a19b8cedcd. The historical harness checked generated record bytes against the retained native stream before producing layers; results were reader-specific.

The exact source input is four points `(0,0), (10,0), (20,0), (30,10)`, pressures `[0.25, 0.5, 0.75, 1]`, tilt and orientation each `[0, 0.25, 0.5, 0.75]`, timestamps `[10, 20, 30, 40]`, ARGB `ff000000`, source size bits `3e6978d5`, effective size `0.4`, and V14 tool 2 source 0. Projection is a 192 × 192 host frame with zero translation and grain offset. The target crop is top-origin `[0,170,62,22]`; the retained native capture checked support within that crop. Native crop SHA-256 is `fcfee743a623d8c4885088ff6b647fd814903788d8a82c43f1b974807065922b` at 1x and `e063a063cd9c238ac54883f823f9d7a680711f8d40db304f55b6d8b8e16758b4` at 2x. Packaged grain and point texture SHA-256 values are `dbe4f006debde4b765c1e5ee6966798dd8b14cdc939b149c9e22770a2c0f97f9` and `475d71a47445657c58c944fb14a4746a248b57d33aff7bff864f30dc72d9ad6f`.

Observed versions: resvg 0.45.1, MuPDF 1.28.2, PDFium 156.0.8076.0, Pillow 12.1.1, and rustc 1.98.0. The PDF image writer used PyMuPDF `insert_image`; the vector writer used PyMuPDF `draw_rect`. Generic 2x PDF used PyMuPDF `show_pdf_page` to transform the direct sdocx PDF.
