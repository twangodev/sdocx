# Colored independent Drawing source-grid SVG control

This isolated experiment extends v197's per-stamp fixed-grid idea to the two-object native capture in `../../crates/sdocx/src/ink/pencil2/test_assets/drawing/independent-drawing/receipt.json`. It uses source-generated records and separate transparent RGBA8 stamp frames, converts each premultiplied pixel back to SVG color and opacity, and paints rectangles in order on the captured opaque [64,128,191] prior. It uses the capture's individual translations (6,9), (12,12) and grain offsets (6,9), (12,13.999998), and emits 84 and 82 stamps. The generic public SVG uses corresponding translated top-down strokes. This is an independently assembled SVG experiment, not v197's black-only public dispatch. Native reference is synthetic queue, ARM64 emulation and host Mesa, not a device capture.

At 192×192, 1x, resvg 0.45.1, full-frame RGBA absolute error versus retained native readback:

| Prefix | Output | Sum | Max | Differing channels | SVG bytes |
|---|---|---:|---:|---:|---:|
| first | source-grid | 153 | 3 | 116 | 44,467 |
| first | generic | 1,022 | 19 | 233 | 503 |
| both | source-grid | 2,234 | 20 | 532 | 608,273 |
| both | generic | 38,142 | 140 | 1,154 | 845 |

Color is reconstructed from quantized premultiplied stamp bytes; source-over and reader rounding accumulate error, so this is not a claim of exact native blending. The second stroke's path, pressure, size, color and grain offset differ from the first. This tests overlap and order on one opaque backdrop, not transparent backgrounds, SVG transforms, other readers, 2x native Drawing, saved pages, or device parity. The rectangles sample a fixed grid and produce substantial markup; no scalable representation is established.

Run `tools/pencil2-svg-scene-v198/run.sh` from the repository root. It archives clean commit `2110c042a0631b120ab06cac371a06ba78fa1a6b` into a newly created temporary directory, uses the shared Cargo target and hash-pinned lock file, asserts generated record hashes, requires resvg 0.45.1 and Pillow 12.1.1, and checks SVG hashes, frame dimensions, and table metrics. Outputs are exclusively created in that directory, with a per-frame record and 2 MB markup bound; the runner removes its temporary directory. The source-generated stream hashes equal the retained native hashes: first `be8483b83e8918f18ffd4b9f23377a28b0bec8b095b58e6c201ac26c566828a8` and second `ab8c231108800d1a9437783f27d12c57e59c4d81a00538268343f662a1c64389`. Captured stream bytes are used only for the preexisting hashes, never as renderer input. The vector SVG uses top-down coordinates directly, and the generic SVG uses source points transformed to the same page coordinates; exact SVG hashes are checked in `compare.py`. Frame checks include all 192×192×4 channels.

The clean-source replay reproduced all generated SVG hashes and table metrics. Native compressed frame hashes are `ab11d63776cd76aff35f5365b55cf7e7e11b8eb1efaf5bec5ea51735e15dfddd` and `84b5770be474561d848b359da6b4084930412220b28c62080e04254f52d616fd`; the comparator reads the existing tracked fixtures under `crates/sdocx/src/ink/pencil2/test_assets/drawing/independent-drawing`, without duplicating capture artifacts.
