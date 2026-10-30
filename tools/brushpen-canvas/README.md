# Explicit BrushPen canvas control

The native source, textures and uniforms are retained in the existing
`conformance/brushpen-second-native` fixtures. The new nonwhite RGBA fixture
uses clear `[32, 160, 96, 255]`; its hash is checked in the Rust regression.

Generate PDFs from the same public canvas used by the native-frame assertion:

```sh
SDOCX_BRUSHPEN_CANVAS_EXPORT=/tmp/brushpen-canvas cargo test -p sdocx --features render,serde,pdf --lib ink::brushpen::paint::tests::second_native_control_compares_geometry_and_paint -- --exact
python tools/brushpen-canvas/export_compare.py /tmp/brushpen-canvas
```

The comparison requires PyMuPDF, pypdfium2 and Pillow. PNG and image-backed SVG
are also checked exactly in Rust. The receipt covers this opaque host control
at its original pixel grid, not device or arbitrary input parity.
