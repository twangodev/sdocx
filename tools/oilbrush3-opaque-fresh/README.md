# OilBrush3 fresh opaque-page control

A fresh curved trajectory through (600,420), (608,429), (619,423), and
(631,436) was replayed through native Drawing on host Mesa over the captured
uniform opaque page `[41,83,127,255]`. The capture verifies the initial page,
clear=0 retention and clear=1 clearing. The clear=0 native run produced 17 page
draws and no final Mesa error. The public `paint_oilbrush3_opaque_host` frame
and intrinsic SVG match its full final RGBA frame exactly. Direct PDF and
SVG-page PDF match the native RGB frame at 1x in MuPDF and PDFium. These are
synthetic host controls, not device, saved-note, arbitrary page or lifecycle
parity.

The capture requires the pinned project-local runner and Unicorn 2.1.4; the
comparison requires PyMuPDF and pypdfium2. Local reproduction:

```sh
/tmp/sdocx-unicorn-venv/bin/python tools/oilbrush3-opaque-fresh/capture.py
mkdir -p /tmp/sdocx-oilbrush3-opaque-fresh-export
SDOCX_OILBRUSH3_OPAQUE_FRESH_EXPORT_DIR=/tmp/sdocx-oilbrush3-opaque-fresh-export CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo test -p sdocx --lib --features pdf,serde oilbrush3_opaque_fresh_public_export
/tmp/sdocx-v190-readers/bin/python tools/oilbrush3-opaque-fresh/compare.py /tmp/sdocx-oilbrush3-opaque-fresh-export
```
