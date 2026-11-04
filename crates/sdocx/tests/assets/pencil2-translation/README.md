# Pencil2 translated host replay

These synthetic controls replay production-sampled `pressure_ramp` records on a
192 × 192 RGBA8 target using host Mesa and native GLV14/RTV4 paint. They test
paint conditionally on those records; they do not establish independent native
geometry, Android device, saved-note, or arbitrary-transform parity. See the
unaltered `translation-matrix-receipt.json` for input matrices, host details,
native frame hashes, and capture pins. `ramp.bin` is the sampled record stream;
its SHA-256 is recorded in that receipt.

The three `.delta` files are compact derived fixtures, not original captures.
Reconstruct each full top-origin frame from the deterministic opaque prior in
the receipt. Each 8-byte record stores a little-endian u32 pixel index followed
by its four RGBA bytes. The test checks the reconstructed frame SHA-256 against
the original receipt before comparing every output pixel. The original complete
compressed readbacks remain in the historical workspace and are not included
here.

`tools/pencil2-public-replay/translation_matrix_capture.py` is the unmodified historical capture script,
retained for provenance. Its original relative paths refer to
`tools/pencil2-public-replay/` and an ignored historical `tmp/.../capture_candidate.py`
harness, as well as native libraries. The historical harness and libraries are
not included, so the capture is not reproducible from these files alone.

The PDF reader measurements in the receipt are historical. This regression test
checks canvas pixels and SVG rendering only; it does not rerun PDF comparisons.
