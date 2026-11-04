# Pencil2 black overlap host replay

These synthetic strokes were sampled independently through the public sampler
and replayed on host Mesa GLV14/RTV4, over an opaque patterned prior. This is a
bounded host comparison, not Android device, screenshot, saved-note or general
transform parity. The original receipt includes historical PDF and Wasm results;
the regression test compares canvas and SVG bytes; it does not rerun PDF or
Wasm comparisons.

`overlap.delta` contains derived changed pixels, not the original native frame.
Each eight-byte record holds a little-endian u32 pixel index followed by RGBA;
the test reconstructs the complete frame from the deterministic prior, verifies
its native receipt hash, and compares every byte. The original native frame
remains in the historical workspace. `flat.bin` is the independently sampled
second input; the first stream is retained at `../pencil2-translation/ramp.bin`.
The unchanged script is `tools/pencil2-public-replay/overlap_capture.py`. The
original script refers to streams at its historical path and depends on an
ignored historical capture harness and native libraries not included here, so
these files alone cannot recapture the evidence. Host and library pins are in
`overlap-capture-info.json`.

`tools/pencil2-public-replay/overlap_capture.py` writes a transient capture-info
copy beside the script on recapture; inspect it before refreshing this fixture.
