# Third synthetic serialized BrushPen control

This modifies the retained FountainPen-derived GLV10 serialized frame before native `NewApplyBinary`: the pressure seed is 0.70 and a path delta is 48. This is synthetic, not a saved BrushPen note. Native string resolution, Model attachment, Drawing and RTV1 run under host adapters; the complete note loader and device rendering are outside this control. Principal native libraries and helpers are pinned; transitive adapters and toolchain are not.

Run `python3 tools/brushpen-third/verify_inputs.py` to verify the source-derived frame, payload and native geometry and RGBA hashes. Reproduce native capture with `BRUSHPEN_CAPTURE_ROOT="$PWD" PYTHONPATH=scratch/apk-analysis-runtime/python .venv/bin/python tools/brushpen-third/capture.py`; this writes the frame and native fixture outputs. The retained receipt includes the input hashes as well as the capture result.

To verify the public route and generate exports, create `/tmp/brushpen-third-export` and run:

```sh
CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 SDOCX_BRUSHPEN_THIRD_EXPORT=/tmp/brushpen-third-export cargo test -p sdocx --lib --features pdf,serde third_serialized_brushpen_native
.venv/bin/python tools/brushpen-third/compare.py /tmp/brushpen-third-export
```

The comparison requires NumPy, Pillow, PyMuPDF, pypdfium2 and resvg. The Rust decoder matches the native getter bytes for all 204 points, pressures, tilts, orientations and timestamps. All 45 native geometry records and the native frame match the public model. PNG, resvg SVG and PDFium PDF match the frame exactly; MuPDF's direct and SVG-derived PDF alpha error sum is 71, maximum 1. Reader versions and hashes are in `export-receipt.json`. These observations do not establish general saved-note, device or PDF-reader parity.
