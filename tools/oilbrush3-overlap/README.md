# OilBrush3 opaque caller-page control

This synthetic native host-Mesa Drawing control supplies an entire opaque
1920×1080 `[41, 83, 127, 255]` page, so the stroke intersects the supplied
background. The capture verifies that `clear=0` retains the initial page and
`clear=1` clears it before the first draw. The retained prefix bundle covers all
15 page draws with clearing disabled. It does not establish multi-object
lifecycle, arbitrary backgrounds, saved-note rendering, or device parity.

The explicit `paint_oilbrush3_opaque_host` public method accepts only this
uniform page and the existing bounded source and texture profile. Its extension
to other admitted trajectories is model-based and has not been compared with
native on this page. The scoped
Rust test compares its entire final RGBA frame and intrinsic SVG with the native
frame. Historical direct PDF and SVG-page PDF comparisons at 1× matched native RGB
exactly in MuPDF and PDFium. The source model generates the paint; it does not substitute
captured output.

Capture depends on the pinned project-local native runner and Unicorn:

```sh
/tmp/sdocx-unicorn-venv/bin/python tools/oilbrush3-overlap/capture.py
CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo test -p sdocx --lib --features pdf,serde oilbrush3_opaque_page_public_export
```

The optional artifact exporter, reader comparator, and derived export receipt
were removed. Rust retains native/SVG assertions and in-memory PDF construction;
PDF-reader results above are historical measurements.
