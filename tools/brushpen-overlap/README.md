# BrushPen overlap capture

These frames capture two synthetic ObjectStroke submissions on one Unicorn/Mesa host canvas, not device or saved-note output. The retained JSON receipts record the frame digests and capture observations; the three compressed frames used by Rust tests live under `crates/sdocx/tests/assets/brushpen-overlap/`.

`build_capture.py` generates `/tmp/brush-overlap-run.py`; set `BRUSHPEN_OVERLAP_PRESSURE=1` or `BRUSHPEN_OVERLAP_COLOR=1` when generating the corresponding variant. Running the generated script requires the project-local native capture runtime, native libraries and host graphics stack referenced by `tools/brushpen-transform/second_mapdraw.py`. The runtime closure and graphics stack are not fully pinned, and the script intentionally terminates after writing `/tmp/brush-overlap-frame.rgba`.

The retained color receipt records the original builder SHA-256 `0b1cf519ed24538b244f6f2079c27c6f9aefeb2e786cdbf4c5b3496743d8a1e2`. The checked-in builder differs only in resolving the checkout root relative to the script. The earlier baseline and pressure receipts record a prior builder revision.

The historical source manifest records former local copies of these three frames. To verify that snapshot, copy the byte-identical crate assets to this directory temporarily; remove the copies afterward.
