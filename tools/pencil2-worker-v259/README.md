# Matched factory handoff control

The v258 Rust input differed from the native handoff: pressure 0.5, zero tilt
and opaque black versus native pressures `[0.25, 0.5, 0.75, 1.0]`, tilt
`[0.4; 4]` and ARGB `0x80336699`. Geometry differed in 83 alpha lanes and all
84 RGB triples; other numeric lanes matched. Its retained metrics compare
different strokes and do not isolate every possible mismatch.

On the prepared native host, run:

```sh
python3 tools/pencil2-worker-v259/probe.py
```

This replays the pinned v257 capture and compares its 4,704-byte GL instance
upload with `native-geometry.vbo` (SHA-256
`be8483b83e8918f18ffd4b9f23377a28b0bec8b095b58e6c201ac26c566828a8`).
The maintained Rust test compares all 84 14-float records, verifies grain-offset
bits for `[6, 183]`, and matches every canvas RGBA byte to the native frame.
It also checks PNG/SVG alpha and constructs a 72-dpi PDF in memory:

```sh
CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 \
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
cargo test -p sdocx --all-features --lib worker_v259_geometry_probe --offline
```

Historically, the matched input painted a transparent 192×192 Rust canvas
identical to the v256 native frame after row flipping. Its top-row-first SHA-256
was `44e57b9754e76dd9883c7a9944a9015f0ccb0c11e788dd71d8c5f1a889174e36`.
The comparison also reproduced from revision
`0ec11639ef0f5e35d1baac493f49c3b34e6a28a8` with the diagnostic copied in.
At 72 dpi, SVG/resvg and direct-PDF/PDFium matched all 36,864 alpha pixels;
MuPDF differed at 80 pixels by one alpha level (sum 80). Reader environment:
Pillow 12.1.1, MuPDF 1.28.2 and PDFium 156.0.8076.0.

The standalone Rust harness and both comparators were retired. External PDF-reader results remain historical; they are not rerun by Rust. Alpha equality does not establish full
color export fidelity.

Native acquisition retains the synthetic queue, manual canvas handoff, emulated
ARM64, host Mesa and v256 RGBA8 substitution for the factory's R8 attachment.
It requires the existing local native dependencies and proves neither normal
factory attachment output, saved-page lifecycle nor device behavior. These
controls justify no production change.
