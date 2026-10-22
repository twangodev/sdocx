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
  Captured static quad vertices have local bounds X=±0.77*(size/2+0.5)
  and Y=±(size/2+0.5), centered at zero. The vertex shader rotates these
  bounds and adds the saved AddPoint XY unchanged. This was verified with
  native task2 frames for an unrotated stylus stroke and horizontal and
  rotated tool-0 strokes: the instance VBO XY matched the saved AddPoint
  coordinates exactly, and the static quad's local center was (0,0). The
  frames are controlled Unicorn/Mesa replays of native Marker4 RTV4 code,
  not Samsung-device pixel captures.
- `CreatePenCanvas` (`0x5e9c0`) creates a 200×200 mask and draws the
  rounded rectangle [1,1,199,199] with corner radii 50. The vector tip
  approximates those proportions, with alpha applied once to the stamp union.

The initialized GLV8 `GetDrawnPointData` callback (`0x54d44`, slot `+0x38`)
has a separate fractional geometry contract. Two native executions reconstructed
HF04 stroke 35 from its 253 saved points and five source channels, changing
only size from 36.21053695678711 to 36.0. Each returned 341 `RectF` records:
stamp widths were 28.652099609375 and 28.489990234375 respectively, with
heights approximately 37.21054 and 37.0. Raw callback checkpoints matched
the captured records. These controls establish private callback geometry,
not archive loading, Standard PDF behavior or raster coverage.

Standard PDF export uses `RasterListX` and `ObjectPDFWriter::WriteObjectListAsBitmap`
(Composer `0x37c3bc`). Its batch dimensions are bounds divided by export scale,
then truncated to integers (`0x37c45c`, `0x37c488`); floating-point bounds remain
for placement. This allocation rule does not establish per-stroke size handling.
An actual Standard export must identify the selected Marker4 drawable and size
before the private callback can justify changing the public vector profile.

## Real-file comparison

At 600×848, Chromium SVG output was compared with the Samsung PDF rendered
by PyMuPDF. A colored pixel is one whose maximum RGB channel minus minimum
channel exceeds 35. Colored-mask intersection over union improved from
28.45% to 91.75%; missing reference-colored pixels fell from 71.44% to 2.75%.
Mean absolute channel error within the shared colored mask fell from 35.18
to 4.73. These measurements cover this fixture, not general native parity.

The later V8 center correction improves the paired PDF's MuPDF color-region
comparison at 1×/2×. PDFium's color-region error worsens at 1× and improves at
2×; thresholded color support worsens slightly at both scales. The correction
follows the executed native center contract; filtering remains approximate.

The comparison includes narrow ruled paper (template 1). Observed differences
include native mask filtering and typed-text placement/size. Other Marker4
versions, rainbow effects and live prediction remain unsupported by this path;
[V7 stylus geometry](marker4-v7.md) has separate evidence.
