# Colored independent Drawing source-grid SVG control

The experimental source patches, harnesses and comparators were retired. The
findings below are historical; native inputs, captures and receipts remain.
These prototype assertions and external-reader measurements are not claimed
as equivalent maintained Rust coverage.

Historical source revision: `2110c042a0631b120ab06cac371a06ba78fa1a6b`. Generated stream hashes matched native: first `be8483b83e8918f18ffd4b9f23377a28b0bec8b095b58e6c201ac26c566828a8`, second `ab8c231108800d1a9437783f27d12c57e59c4d81a00538268343f662a1c64389`. Captured pixels were comparison targets, not renderer inputs.

This isolated experiment extends v197's per-stamp fixed-grid idea to the two-object native capture in `../../crates/sdocx/src/ink/pencil2/test_assets/drawing/independent-drawing/receipt.json`. It uses source-generated records and separate transparent RGBA8 stamp frames, converts each premultiplied pixel back to SVG color and opacity, and paints rectangles in order on the captured opaque [64,128,191] prior. It uses the capture's individual translations (6,9), (12,12) and grain offsets (6,9), (12,13.999998), and emits 84 and 82 stamps. The generic public SVG uses corresponding translated top-down strokes. This is an independently assembled SVG experiment, not v197's black-only public dispatch. Native reference is synthetic queue, ARM64 emulation and host Mesa, not a device capture.

At 192×192, 1x, resvg 0.45.1, full-frame RGBA absolute error versus retained native readback:

| Prefix | Output | Sum | Max | Differing channels | SVG bytes |
|---|---|---:|---:|---:|---:|
| first | source-grid | 153 | 3 | 116 | 44,467 |
| first | generic | 1,022 | 19 | 233 | 503 |
| both | source-grid | 2,234 | 20 | 532 | 608,273 |
| both | generic | 38,142 | 140 | 1,154 | 845 |

Color is reconstructed from quantized premultiplied stamp bytes; source-over and reader rounding accumulate error, so this is not a claim of exact native blending. The second stroke's path, pressure, size, color and grain offset differ from the first. This tests overlap and order on one opaque backdrop, not transparent backgrounds, SVG transforms, other readers, 2x native Drawing, saved pages, or device parity. The rectangles sample a fixed grid and produce substantial markup; no scalable representation is established.

The clean-source replay reproduced all generated SVG hashes and table metrics. Native compressed frame hashes are `ab11d63776cd76aff35f5365b55cf7e7e11b8eb1efaf5bec5ea51735e15dfddd` and `84b5770be474561d848b359da6b4084930412220b28c62080e04254f52d616fd`; the retired comparator read the existing tracked fixtures under `crates/sdocx/src/ink/pencil2/test_assets/drawing/independent-drawing`, without duplicating capture artifacts.
