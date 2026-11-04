# Pencil2 translated host replay

These synthetic controls replay production-sampled `pressure_ramp` records on a
192 × 192 RGBA8 target using host Mesa and native GLV14/RTV4 paint. They test
paint conditionally on those records; they do not establish independent native
geometry, Android device, saved-note, or arbitrary-transform parity. See the
`translation-matrix-receipt.json` for input matrices, host details,
native frame hashes, and capture pins. `ramp.bin` is the sampled record stream;
its SHA-256 is recorded in that receipt.

The receipt's three result entries retain readable `delta_rows` containing
[top-down pixel index, R, G, B, A]. These are derived native pixels, not new
captures. Rust and Node reconstruct the historical eight-byte records in memory
and verify their hashes, then reconstruct each frame over the deterministic
opaque prior and check the original native frame hash. Redundant compressed readbacks were removed after exact reconstruction; original native frame hashes remain in the receipt.

`tools/pencil2-public-replay/translation_matrix_capture.py` now reads the canonical
`ramp.bin` retained here. Its former tool-directory copy was verified byte-identical
before removal. Only the input path changed; the receipt preserves the original
acquisition script hash, which differs from the current script. The script still
requires an ignored historical `tmp/.../capture_candidate.py` harness and native
libraries. The historical harness and libraries are
not included, so the capture is not reproducible from these files alone.

The PDF reader measurements in the receipt are historical. This regression test
checks canvas pixels, SVG rendering and PDF construction. The separate Node
test checks WASM pixels; neither reruns external PDF reader comparisons.
