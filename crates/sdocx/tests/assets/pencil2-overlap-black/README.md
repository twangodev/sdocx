# Pencil2 black overlap host replay

These synthetic strokes were sampled independently through the public sampler
and replayed on host Mesa GLV14/RTV4, over an opaque patterned prior. This is a
bounded host comparison, not Android device, screenshot, saved-note or general
transform parity. The original receipt includes historical PDF and Wasm results;
the Rust regression compares canvas and SVG bytes and checks PDF construction.
The separate Node test checks WASM pixels; external PDF reader comparisons remain
historical.

The receipt's readable `delta_rows` contain derived changed pixels, not a new
native capture. Each row holds a top-down pixel index and RGBA. Rust and Node
reconstruct historical eight-byte records and verify their hash, then reconstruct
the complete frame over the deterministic prior and verify the native hash
before comparing every pixel. The original native frame
remains in the historical workspace. `flat.bin` is the independently sampled
second input; the first stream is retained at `../pencil2-translation/ramp.bin`.
The capture script `tools/pencil2-public-replay/overlap_capture.py` now reads
these canonical streams; its former tool-directory copies were verified
byte-identical before removal. Only input paths changed. The receipt preserves
the original acquisition script hash, which differs from the current script.
It still depends on an ignored historical capture harness and native libraries
not included here, so
these files alone cannot recapture the evidence. Host and library pins are in
`overlap-capture-info.json`.

`tools/pencil2-public-replay/overlap_capture.py` writes a transient capture-info
copy beside the script on recapture; inspect it before refreshing this fixture.
