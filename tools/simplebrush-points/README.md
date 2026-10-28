# SimpleBrush 2× point-count controls

The synthetic Drawing captures use the project-local native and Mesa inputs described in `../simplebrush-scale/README.md`. They do not establish device or saved-note parity. Run from the repository root, supplying a fresh directory:

```sh
mkdir -p /tmp/simplebrush-points-run
POINT_COUNT=16 python3 tools/simplebrush-points/capture.py fraction12 /tmp/simplebrush-points-run/native.json
CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
SDOCX_POINTS_COUNT=16 SDOCX_SIMPLEBRUSH_TWO_EXPORT=/tmp/simplebrush-points-run \
cargo test -p sdocx --features pdf,serde --lib two_pixel_native_export_fixture
.venv/bin/python tools/simplebrush-points/compare.py /tmp/simplebrush-points-run 16
```

Counts 2, 4, 7 and 16 are admitted controls at size 12 and tool 2. The combined gate admits 2–16 points at sizes 8–20 for tools 0, 2 and 3, while the separate three-point tool-2 size controls admit sizes 3–32. Other combinations extrapolate from those controls. Count 32 is a diagnostic: reproduce its comparison only after temporarily widening the count gate in `render.rs` to 32. The native wrapper checks the source, paint script and DSO pins inherited from the existing 2× receipt and checks each resulting full-frame and geometry hash. Other dynamically loaded inputs are not independently pinned. The comparator requires resvg 0.45.1 and MuPDF 1.28.2; it checks retained native crop hashes and compares against matched blank-page PDF rasterization. Rasterizer differences can affect metrics.
