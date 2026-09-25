# Fountain pen vector parity

The supported saved V14 fountain profile now uses Rust-generated vector
shading in SVG previews and PDF exports. V14/V16 positions and widths retain
the native reconstruction; no global width adjustment is applied.

## Geometry and appearance

V14 retains SmPath's direction vectors, including its small-vector
normalization threshold, zero-vector substitution, and final stamp's retained
direction. Native fixtures compare positions, radii, directions and
original-sample boundaries. Pressure, tilt and movement continue through the
existing native saved-stroke reconstruction.

Previously every V14 stamp was filled opaquely. The native directional gradient
is flat over the central half and falls linearly to 0.07 at its tangent-aligned
ends. `render/fountain.rs` expresses that function as an SVG linear gradient,
transformed with each stamp's native radius and direction.

Opaque grayscale stamps blend with Lighten over black in a luminance mask.
This implements maximum coverage in the vector interior instead of accumulating
opacity where stamps overlap. Stroke color and opacity are applied once to the
result. PDF export preserves the masks as vector forms, the gradients as PDF
shadings, and the Lighten blend mode. There are no image or filter elements in
the generated fountain SVG, and no bitmap API or custom rasterizer.

These are scale-independent vector shapes. The SVG/PDF consumer owns edge
antialiasing. The APK's device-pixel expansion and subpixel compensation are
not baked into document geometry, so this is not a claim of byte-identical
screen output at every zoom level.

## Validation on 2026-09-25

- The available `handwritten.sdocx` contains 2,769 V14 strokes and 56,713 stamps.
  The native saved-redraw oracle and Rust geometry agreed exactly on x, y,
  radius, direction x/y and original-sample boundaries.
- New regressions check gradient orientation and interior maximum blending with
  repeated stamps, including stroke opacity. PDF object checks require native
  shadings and Lighten and reject image XObjects.
- The real SVG is 10,739,219 bytes. The PDF is 42,045,542 bytes and contains zero
  image XObjects across all PDF objects, including mask resources.
- A local Chromium decode-and-draw probe measured about 1.11 seconds for the
  shaded SVG versus 0.16 seconds for the previous opaque-path SVG. This is one
  cold probe, not an interaction benchmark. Vector shading increases document
  complexity and PDF size; it is not flattened to reduce either.

```sh
cargo test --offline -p sdocx --all-features --lib fountain
cargo test --offline -p sdocx --all-features --test pdf_export
```

The optional hash-pinned native geometry oracles are documented in the
[conformance guide](../../conformance/README.md#native-geometry-checks).

## Scope limits

The existing debugger's partial-stroke canvas replay still uses its previous
circle paths; the completed page and exports use the vector shading above.
Unsupported saved settings remain explicitly approximate, including fixed
width, non-stylus input, other historical profiles and effects. Saved redraw
validation does not establish live prediction or temporary-tip parity.
