# SimpleBrush 1× long controls

Two synthetic ObjectStroke controls use tool 2, 225 points at size 20 and 256 reversed points at size 8, with varying pressure and a fractional 1× matrix. The native Drawing/ARM64 emulation and host Mesa capture clears the target before painting. The complete generated geometry matches the Rust sampler byte for byte for both controls. Native compressed frames and vectors and source/export hashes are retained in `receipt.json`. This is not evidence of native page traversal, native PDF, device, or saved-note parity. The capture depends on project-local scratch inputs; hashes do not attest all transitive dependencies or the build environment.

The candidate extends the existing one-pixel profile for tools 2 and 3 to 256 points, retaining all other guards and budgets. A paired tool-3 capture for the 256-point input matches the complete tool-2 native geometry and frame, and its public SVG and direct PDF exports are byte-identical. Tool-3 coverage of other paths extrapolates. The SVG resvg alpha errors improve from 432,314 to 3,336 and 564,440 to 7,994 over generic output. The direct document PDF MuPDF matched-paper RGB errors improve from 486,218 to 33,234 and 621,815 to 63,234. PDFium errors improve from 481,471 to 48,806 and 617,605 to 115,914, but PDFium's unshifted grid differs from the fractional native matrix and includes phase error. Errors remain nonzero and other trajectories extrapolate.

To reproduce, create fresh `/tmp/sb1-225` and `/tmp/sb1-256` directories and run:

```sh
POINT_COUNT=225 PROBE_SIZE=20 python3 tools/simplebrush-one-long/capture.py fraction12 /tmp/sb1-225/native.json
POINT_COUNT=256 PROBE_SIZE=8 VARIANT=reverse python3 tools/simplebrush-one-long/capture.py fraction12 /tmp/sb1-256/native.json
CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 SDOCX_POINTS_COUNT=225 PROBE_SIZE=20 SDOCX_V62_NATIVE=/tmp/sb1-225/native.json SDOCX_SIMPLEBRUSH_TWO_EXPORT=/tmp/sb1-225 cargo test -p sdocx --features pdf,serde --lib two_pixel_native_export_fixture
CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 SDOCX_POINTS_COUNT=256 PROBE_SIZE=8 VARIANT=reverse SDOCX_V62_NATIVE=/tmp/sb1-256/native.json SDOCX_SIMPLEBRUSH_TWO_EXPORT=/tmp/sb1-256 cargo test -p sdocx --features pdf,serde --lib two_pixel_native_export_fixture
.venv/bin/python tools/simplebrush-one-long/compare.py /tmp/sb1-225
.venv/bin/python tools/simplebrush-one-long/compare.py /tmp/sb1-256
PYTHONPATH=/path/to/pypdfium2 .venv/bin/python tools/simplebrush-one-long/pdfium.py /tmp/sb1-225
PYTHONPATH=/path/to/pypdfium2 .venv/bin/python tools/simplebrush-one-long/pdfium.py /tmp/sb1-256
```

The native geometry assertion in the export fixture compares the same generated points, pressure, size and tool to the captured stream. `generic.pdf` and `one.pdf` come from `render_document_pdf`; `paper.pdf` is generated from the same empty page. Comparators print metrics; compare them with the receipt. The saved export hashes came from a source snapshot containing the candidate gate; they do not attest the full build snapshot.

For the paired tool-3 control, repeat the 256-point capture and export with `--tool 3` and `SDOCX_TOOL=3`, respectively; the native geometry and full frame hashes and all export hashes match the corresponding tool-2 control. The fixture-backed Rust test checks both tool-2 geometry streams during ordinary testing.
