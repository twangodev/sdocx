# Third synthetic serialized BrushPen control

This modifies the retained FountainPen-derived GLV10 serialized frame before native `NewApplyBinary`: the pressure seed is 0.70 and a path delta is 48. This is synthetic, not a saved BrushPen note. Native string resolution, Model attachment, Drawing and RTV1 run under host adapters; the complete note loader and device rendering are outside this control. Principal native libraries and helpers are pinned; transitive adapters and toolchain are not.

Run `python3 tools/brushpen-third/verify_inputs.py` to verify the source-derived frame, payload and native geometry and RGBA hashes. Reproduce native capture with `BRUSHPEN_CAPTURE_ROOT="$PWD" PYTHONPATH=scratch/apk-analysis-runtime/python .venv/bin/python tools/brushpen-third/capture.py`; this writes the frame and native fixture outputs. The retained receipt includes the input hashes as well as the capture result.

The experimental output hook and external comparator have been retired. Run the
in-memory native regression with the project shared Cargo settings:

```sh
CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo test -p sdocx --lib --features pdf,serde third_serialized_brushpen_native
```

The Rust decoder matches the native getter bytes for all 204 points, pressures, tilts, orientations and timestamps. All 45 native geometry records and the native frame match the public model. Historical PNG, resvg SVG and PDFium PDF comparisons matched the frame exactly; MuPDF's direct and SVG-derived PDF alpha error sum is 71, maximum 1. Reader versions and hashes are in `export-receipt.json`. These observations do not establish general saved-note, device or PDF-reader parity.

The test retains PNG and both PDF serialization checks in memory. External reader
pixel comparisons remain historical measurements, not current Rust assertions.
