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

A separate actual HF04 archive load selected stroke 35 by its saved UUID
`f0ef06a2-b6d1-11f1-b1b6-8f2ef4b7d1ae`. Its 253 pressure, timestamp, tilt and
orientation samples matched the current source decoder byte for byte. All 506
XY floats matched `f32(source * f32(1080 / 1848))`; the same factor mapped saved
width 36.21053695678711 to native width 21.16200065612793. The harness passed
1080 to `WNote::ConstructLoad`; the source object's owner dimensions are
1848×2613. Model `ObjectStroke::NewApplyBinary` calls `GetLoadMagnification`
at `0x2e58d8`; the compressed restore scales points at
`0x2ee8bc`–`0x2ee8f0` and flexible width at `0x2ed920`–`0x2ed934`.
The raw capture's SHA-256 is
`62ef0868161d36904d56334d388491c46f20430894be8f555135593f7ad3650e`.
It establishes post-load data, not Standard export size handling; the scale's
runtime S0 was not captured. The earlier pressure-hash discrepancy came from
a retained parser report predating float32 delta accumulation, not this load.

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

A further height experiment compared the current vector height `0.99 * size`
with `0.99 * (size + 1)`, reflecting the full native quad extent. For the
HF04 source SHA-256 `8299abbf763e8e3e5af746815d13dfdf45443da27b0eb79192c328571905cdd2`
and Samsung PDF SHA-256 `b951e506c2d5ebd0ae100ac5cf7179e2575e6fcc8dcdea59be822de4341f8a9a`,
MuPDF 1.28.2 rendered each public PDF and reference to 600×848 and 1200×1696.
Using RGB channel range greater than 35 as the color mask, intersection over
union changed from 0.94149 to 0.92515 at 1× and from 0.97027 to 0.97005 at
2×. Absolute RGB error summed over the color-mask union increased from 296,271
to 310,238 and from 726,454 to 757,895 respectively. The larger height reduced
missing color pixels but increased extra pixels. This does not support changing
the approximation; quad extent is not equivalent to filtered mask coverage.
The scratch comparison is `tmp/marker4-height-comparison/compare.py` and uses
the baseline and candidate CLI PDFs in `/tmp`. It does not establish general
device or PDF-reader behavior.

The comparison includes narrow ruled paper (template 1). Observed differences
include native mask filtering and typed-text placement/size. Other Marker4
versions, rainbow effects and live prediction remain unsupported by this path;
[V7 stylus geometry](marker4-v7.md) has separate evidence.
