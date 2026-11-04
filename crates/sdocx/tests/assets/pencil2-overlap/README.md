# Pencil2 translucent overlap host replay

This synthetic control replays production-generated sampled records for two
colored, tilted strokes on host Mesa RTV4 over a deterministic premultiplied
translucent patterned prior. It tests raster output conditional on those sampled
records; it does not establish independent native geometry, screenshot, Android
device, saved-note, or arbitrary-transform parity. The unchanged receipt records
the inputs, frame and stream hashes and historical PDF and Wasm results. The
regression test checks canvas and SVG pixels; it does not rerun PDF or Wasm.
The historical PDF measurements include reader differences on translucent
pixels, including untouched prior pixels.

`top2-translucent.delta` is derived from the native capture, not the original
capture. Start with the deterministic prior in the test. Each eight-byte record
is a little-endian u32 pixel index followed by four RGBA bytes. The test checks
the reconstructed complete frame against the native SHA-256 in the receipt and
compares every canvas and SVG pixel. The original compressed native frame remains
in the historical workspace.

The unmodified historical capture script is
`tools/pencil2-public-replay/colored_overlap_top2_capture.py`. Its original
relative input paths point to `tools/pencil2-public-replay/`; copies of the
sampled input streams are retained here, with hashes in the receipt. The script
also requires an ignored historical `tmp/.../capture_candidate.py` harness and
native libraries, neither included here, so these files alone cannot recapture
this evidence. `colored-top2-translucent-info.json` records host and library pins.

`tools/pencil2-public-replay/colored_overlap_top2_capture.py` writes a transient capture-info
copy beside the script on recapture; inspect it before refreshing this fixture.
