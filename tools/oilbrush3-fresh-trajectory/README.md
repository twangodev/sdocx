# Fresh OilBrush3 public export control

This control uses four changed points forming a curved path at (600,420) through
(631,436), while retaining the admitted size, color, metadata, transparent
initial state, textures, canvas dimensions, and host profile. The native
Drawing capture produced 17 P4 draws and returned no final Mesa error. The
public `paint_oilbrush3_cold_host` method exactly matches its complete
transparent RGBA8 readback after accounting for framebuffer origin. After white
compositing, public SVG, direct PDF, and SVG-page PDF rasterizations match the
native frame exactly; the two PDFs were checked with MuPDF 1.28.2 and PDFium
150.0.7869.0. The retained capture and export receipts identify inputs and
versions. This one synthetic host control does not establish device, saved-note,
other initialization, scale, or lifecycle parity.

Capture requires the project-local `scratch/oilbrush3-public-drawing-v17` native
runner, its preflight dependencies, native libraries, and Unicorn 2.1.4. Run
`/tmp/sdocx-unicorn-venv/bin/python tools/oilbrush3-fresh-trajectory/capture.py`.
The capture writes the retained compressed frame and receipt.

```
mkdir -p /tmp/sdocx-oilbrush3-fresh-export
SDOCX_OILBRUSH3_FRESH_EXPORT_DIR=/tmp/sdocx-oilbrush3-fresh-export CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo test -p sdocx --lib --features pdf,serde oilbrush3_fresh_trajectory_public_export
/tmp/sdocx-v190-readers/bin/python tools/oilbrush3-fresh-trajectory/compare.py /tmp/sdocx-oilbrush3-fresh-export --check
```

The comparison requires PyMuPDF and pypdfium2; the interpreter path above is a
local environment, not a portable dependency. No model correction was indicated
by this control.
