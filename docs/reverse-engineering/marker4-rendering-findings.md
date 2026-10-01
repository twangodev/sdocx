# Marker4 V8 saved rendering

Fixture `04-marker4-highlighter` contains 12 Marker4 strokes with settings
`8;`, plus 29 FountainPen strokes. Its highlights have width 36.21 and 50% alpha.

The shared `ink/marker4.rs` path preserves that alpha and reconstructs
midpoint sampling at canonical inverse scale 1. SVG, PDF and replay consume the
same centers, original-sample boundaries and rounded rectangular tip.
The profile remains `Approximate`: the vector outline does not reproduce
native texture filtering or zoom-dependent GPU coverage.

## Native evidence

Addresses are from the existing Samsung Notes 4.4.45.37 ARM64
`libSPenMarker4.so`:

- GLV8 constructor `0x530e8` selects RTV4. Saved redraw `0x54a14`
  processes interior samples, then calls `endPen` (`0x536c0`).
- `drawLine` (`0x542fc`) uses midpoint quadratics and SmPath measurement.
  At unit inverse scale, spacing is 1 and minimum movement is 2. Tool 0
  enables alternating short-move filtering and locks the tip angle from
  the first sampled tangent; stylus tool 2 uses an unrotated tip.
- `endPen` draws the terminal quadratic and an endpoint, or falls back
  to one stamp when no samples were emitted.
- RTV4 `Update` (`0x5f0f8`) uses the truncated integer pen size.
  `setRectData` (`0x5e5b4`) gives X extents ±0.77*(size/2+0.5) and
  Y extents 0.5±size/2, before tip rotation.
- `CreatePenCanvas` (`0x5e9c0`) creates a 200×200 mask and draws the
  rounded rectangle [1,1,199,199] with corner radii 50. The vector tip
  uses those proportions, with alpha applied once to the stamp union.

## Real-file comparison

At 600×848, Chromium SVG output was compared with the Samsung PDF rendered
by PyMuPDF. A colored pixel is one whose maximum RGB channel minus minimum
channel exceeds 35. Colored-mask intersection over union improved from
28.45% to 91.75%; missing reference-colored pixels fell from 71.44% to 2.75%.
Mean absolute channel error within the shared colored mask fell from 35.18
to 4.73. These measurements cover this fixture, not general native parity.

The comparison includes narrow ruled paper (template 1). Observed differences
include native mask filtering and typed-text placement/size. Other Marker4
versions, rainbow effects and live prediction remain unsupported by this path;
[V7 stylus geometry](marker4-v7.md) has separate evidence.
