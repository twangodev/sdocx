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

Counts 2, 4, 7, 16 and 32 are admitted controls at size 12 and tool 2. The combined gate admits 2–32 points at sizes 8–20 for tools 2 and 3, and 2–16 points for tool 0, while the separate three-point tool-2 size controls admit sizes 3–32. Other combinations extrapolate from those controls. Two additional 32-point controls use `VARIANT=reverse` or `VARIANT=alternate` with `tools/simplebrush-points/capture.py`. Use the same variant and `SDOCX_POINTS_COUNT=32` for export, then pass the variant to `compare.py`. The size-12, tool-2 controls do not establish general PDF improvement: for the original 32-point control, matched PDF RGB error is 38,344 for 2× versus 38,161 for 1×; the reversed and alternate controls yield 38,745 versus 38,747, and 39,368 versus 44,867. Generic errors are 676,814, 787,174 and 889,751 respectively. The other point counts, sizes and tools extrapolate from these controls. The native wrapper checks the source, paint script and DSO pins inherited from the existing 2× receipt and checks each resulting full-frame and geometry hash. Other dynamically loaded inputs are not independently pinned. The comparator requires resvg 0.45.1 and MuPDF 1.28.2; it checks retained native crop hashes and compares against matched blank-page PDF rasterization. Rasterizer differences can affect metrics.

To reproduce either added 32-point variant, use a fresh directory and set the
same variant on capture and export:

```sh
mkdir -p /tmp/simplebrush-32-variant
VARIANT=alternate POINT_COUNT=32 python3 tools/simplebrush-points/capture.py fraction12 /tmp/simplebrush-32-variant/native.json
CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
VARIANT=alternate SDOCX_POINTS_COUNT=32 SDOCX_SIMPLEBRUSH_TWO_EXPORT=/tmp/simplebrush-32-variant \
cargo test -p sdocx --features pdf,serde --lib two_pixel_native_export_fixture
.venv/bin/python tools/simplebrush-points/compare.py /tmp/simplebrush-32-variant alternate
```
