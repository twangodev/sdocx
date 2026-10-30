# WaterColor pixel continuation

`watercolor_raster` verifies WaterColor over captured Pencil2 pixels exactly at
1x and 2x. Sampling both tools locally matches at 1x; at 2x the existing Pencil2
precision difference remains at one pixel, with two channels differing by one.
SVG, PNG and both PDF routes preserve these canvas pixels under MuPDF and PDFium.
This is a captured pixel handoff, not a shared native Drawing lifecycle.

`mixed_capture.py` pins its WaterColor frame helper and Pencil2 receipt. The
Pencil2 receipt was captured by `../pencil2_v14/context_capture.py`; its original
pinned base harness is retained as `context-base-capture.py.gz`. To recapture,
decompress that harness to its original
`tmp/vector-parity-plan/pen-family-audit/pencil2-v14-mesa-draw-v38/capture_candidate.py`
path. Native recapture additionally requires the original ARM64 libraries,
Python runtime and transitive historical `tmp/` receipts/helpers named by that
harness; those external dependencies are not bundled. Rust replay and export
comparison require no native runtime. The retained receipts were replayed;
this change does not claim a fresh native capture.
