# Fresh OilBrush3 public export control

This control uses four changed points forming a curved path at (600,420) through
(631,436), while retaining the admitted size, color, metadata, transparent
initial state, textures, canvas dimensions, and host profile. The native
Drawing capture produced 17 P4 draws and returned no final Mesa error. The
public `paint_oilbrush3_cold_host` method exactly matches its complete
transparent RGBA8 readback after accounting for framebuffer origin. After white
compositing, public SVG, direct PDF, and SVG-page PDF rasterizations match the
native frame exactly; the two PDFs were checked with MuPDF 1.28.2 and PDFium
150.0.7869.0. The retained native capture identifies its inputs. This one synthetic host control does not establish device, saved-note,
other initialization, scale, or lifecycle parity.

Capture requires the project-local `scratch/oilbrush3-public-drawing-v17` native
runner, its preflight dependencies, native libraries, and Unicorn 2.1.4. Run
`/tmp/sdocx-unicorn-venv/bin/python tools/oilbrush3-fresh-trajectory/capture.py`.
The capture writes the retained compressed frame and receipt.

```
CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo test -p sdocx --lib --features pdf,serde oilbrush3_fresh_trajectory_public_export
```

No model correction was indicated by this control.

The optional artifact exporter, reader comparator, and derived export receipt
were removed. Rust retains native/SVG assertions and in-memory PDF construction;
PDF-reader results above are historical measurements.
