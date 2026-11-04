# Explicit BrushPen canvas control

The native source, textures, uniforms and nonwhite RGBA frame remain in
`conformance/brushpen-second-native`. The control uses clear `[32, 160, 96, 255]`.
The Rust regression checks the native frame hash and exact canvas, PNG and
image-backed SVG pixels, and constructs direct and SVG-derived PDFs in memory:

```sh
CARGO_TARGET_DIR=/tmp/sdocx-agent-target CARGO_INCREMENTAL=0 \
CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
cargo test -p sdocx --features pdf,serde --lib ink::brushpen::paint::tests::second_native_control_compares_geometry_and_paint -- --exact
```

The retired exporter/comparator measured exact MuPDF and PDFium matches for both
PDF paths; `export-receipt.json` retains those historical results and reader
versions. External-reader pixels are not current Rust assertions. This opaque
host-Mesa control covers its original pixel grid, not device or arbitrary input
parity.
