# Stroke rendering findings

## Root cause of top-right artifacts

The former visible-stroke parser bypassed the structural page/layer/object
parser and advanced through records using offsets that happened to cancel on
common files.

- `0x79` is the low byte of the 121-byte base-frame size, not an extension
  marker.
- The value called `data_len` is the complete stroke-frame size.
- The `StartPointMinusThree` fallback begins three bytes early.
- At that shifted position, common property mask `0x25` is interpreted as a
  `u16` point count: decimal 37.
- The fallback prefers the parse containing more points, so every legitimate
  stroke shorter than 37 points can be replaced by misaligned data.

That misalignment produces the long strokes landing near the top-right corner.
It is deterministic, not a coordinate transform or SVG renderer problem.

## Correct traversal

```text
page layer collection
  -> layer object tree
  -> outer object type 1
  -> payload frame type 0 (base)
  -> payload frame type 1 (stroke)
  -> point_count at stroke frame +18
  -> point channels bounded by flexible_offset
  -> style fields selected by field mask
```

Compressed channels are stored by channel: first X/Y as `f64`, Q10.5 signed-
magnitude coordinate deltas, first pressure as `f32`, Q3.12 pressure deltas,
first timestamp as `i32`, `u16` timestamp deltas, then optional tilt and
orientation channels in the same Q3.12 form. Two tool/input bytes finish the
fixed channel block.

The full bit tables and uncompressed form are in
[`file-format.md`](file-format.md#stroke-frame).

## Implemented fix

The Rust parser now walks `StoredPage` layers and recursive object records,
then decodes type-0/type-1 frames within each stroke's payload. It no longer has
selection-by-larger-point-count or `StartPointMinusThree`. Channel presence
comes from the stroke property mask; fixed channel bytes must end exactly at
the declared flexible-data boundary. Malformed strokes return an error with
their page ID and payload offset instead of silently returning partial content.

Unknown objects remain in the stored tree, and unexposed flexible data stays
bounded by its frame. Non-stroke semantic interpretation remains best-effort.

`structural_strokes.rs` covers the synthetic format cases in normal CI.
The historical three-document audit passed on the new decoder and failed on
the old decoder's point count for fixture A (322,406 versus the expected
321,776). Those documents and their runner are retired; the measurements remain
in [`fixture-validation.md`](fixture-validation.md).

## Drawing-time curves and width interpolation

Fresh static inspection of the Samsung Notes 4.4.45.37 ARM64 libraries
confirms a rendering gap independent of the historical parser issue above.
This is instruction-level evidence, not a native runtime or pixel-equivalence
comparison. The selected pen for a particular reported rough stroke still
needs to be identified before choosing its native algorithm.

### Current SDK behavior

`render.rs::render_stroke` draws one straight, round-capped SVG line per
point pair when pressure is present. Each line has a constant width derived
from its starting sample. Otherwise it draws a straight polyline.
`stroke_paint` applies `pen_width / 2.5`, clamped to 0.4–12, then multiplies
by `0.3 + 0.7 * clamp(pressure, 0.05, 1)`. This is a common approximation,
not a recovered per-pen width law. Neither centerline curves nor continuous
width transitions are constructed. The web debugger's
`replay-raster.ts::drawStroke` repeats the same segment geometry in Canvas.

Consequently, round caps do not guarantee a smooth stroke silhouette:
straight segments can show angular turns, and independently changing widths
can show shoulders or bumps. Increasing raster resolution cannot correct
those geometry differences. Their contribution to any particular screenshot
still requires matching the stroke and comparing renderings.

The high-level `Stroke` currently lacks the pen identity and fixed-width
settings available separately through `StrokeMetadata`; the renderer also
does not use timestamps, tilt or orientation to choose its width behavior.

### DefaultPen curve-enabled branch

In `libSPenDefaultPen.so`:

- `RedrawPen(ObjectStroke const*, RectF*)`, `0x18b54`, retrieves stored
  points, pressures, timestamps, tilt and orientation, constructs a
  `MotionEvent` and dispatches redraw. This path applies to saved strokes.
- `RedrawPen(MotionEvent const*, RectF*)` calls `redrawLine` at `0x18694`.
  `redrawLine`, `0x18928`, forms a midpoint between the previous input
  point and the next accepted point. It calls `SmPath::moveTo` at `0x18a04`
  and `quadTo` at `0x18a14`, with the previous point as the control point
  and the midpoint as the endpoint.
- It obtains curve length at `0x18a48`, chooses a repeat count through
  `getRepeat`, and samples position/tangent at `0x18ab8`. Width changes
  are divided across the repeats at `0x18a70` / `0x18a80` and accumulated
  at `0x18adc`. The drawing call receives half the interpolated width,
  with a lower bound of 1 (`0x18ac0` onward).
- The live `drawLine`, `0x18c80`, has the analogous curve construction
  (`0x18d5c`, `0x18d6c`) and distance sampling (`0x18e10`).

These routines also have sample-acceptance thresholds and startup/end state.
A generic midpoint spline alone is not a full reproduction. This trace
establishes width interpolation, not the upstream pressure-to-width law.
There is also an explicit no-curve branch; this algorithm is not universal.

### Marker2 and shared smoothing helpers

In `libSPenMarker2.so`, V2 saved-event redraw calls `drawLine` at `0x22a24`
and `0x22aa0`. That routine (`0x22c10`) constructs the midpoint quadratic
at `0x22ce4`–`0x22d08`, obtains its length at `0x22d1c`, samples positions
at `0x22d58`, and submits them to `Marker2StrokeDrawableRTV2::AddPoint`
at `0x22d68`. It carries the residual sample distance across segments.
This establishes curved, distance-spaced rendering, not pressure-dependent
width for Marker2.

PenCommon `WidthSmoothManager::getSmoothedWidth`, `0x593ec`, computes
`second + factor * (first - second)`. However, the base
`PenStrokeTipDrawableGL::CalculateSmoothedWidths`, `0x525dc`, is just a
return instruction. Symbol names alone therefore cannot prove a selected
pen actually applies smoothing. Callers and overrides must be traced.

The optional post-input coordinate smoother is a separate mechanism:
[stroke finalization](stroke-finalization-findings.md) documents that the
ordinary constructor selects no transformer. Do not rerun input prediction
or optional beautification blindly on samples already saved by the app.

### Implementation direction

Resolve each stroke's saved pen identity/settings into a rendering profile,
then implement the verified profile's curve, sampling and width rules in a
shared Rust geometry layer. Preserve original samples for inspection. SVG
exports and Canvas replay should consume that geometry, including consistent
partial-stroke handling, rather than independently rebuilding straight lines.
Keep unsupported pens explicitly approximate. Validate with enlarged curves,
pressure transitions, dots, sharp corners and stroke ends against paired
Samsung exports before replacing the current renderer.

## Implementation progress: saved rendering inputs

The semantic `Stroke` now carries optional `StrokeRendering`, populated by the
existing bounded style decoder. It retains the complete known style,
properties and tool type, plus resolved pen/settings strings. Both document
parsing and debugger inspection/replay use `StrokeResources` to resolve IDs.
The modern name takes precedence; only an absent or -1 modern ID uses the
legacy ID. Original sample arrays are unchanged. Old serialized strokes default
to no rendering metadata; Rust callers constructing `Stroke` literals must add
`rendering: None` (or supply decoded settings).

All 77 strokes in fixture 02 resolve to
`com.samsung.android.sdk.pen.pen.preload.FountainPen`, settings `18;0;100;`.
DefaultPen curve findings alone therefore cannot establish parity for this
fixture. FountainPen has separate renderer versions, pressure/speed width
calculation, width smoothing and tip handling requiring their own trace.

## Implementation progress: shared preparation and replay

`prepare_stroke` supplies geometry inputs, paint, conservative bounds and
profile/support status for both SVG exports and Canvas replay. The initial
profile preserves the old pressure/width approximation and borrows original
samples. Single-sample strokes now produce a dot. The debugger uses prepared
bounds instead of independently estimating them, and exposes profile/support
status without asserting native parity.

The native registry is represented by `PEN_PROFILES`: 44 names, with aliases
kept separate and 30 distinct bundled library stems. Recognized profiles still
report `Approximate`; names alone do not activate an unverified renderer.
The existing 48 MiB replay tile cache and optimized backend primitives are
retained. A filled-capsule path experiment was rejected after cold seeks rose
from approximately 322 ms to 461 ms; splitting it into fragments did not fix
that regression. None of that experimental path/cache code is shipped.

Native per-pen curves, width laws, textures and blending remain unfinished.
Shared preparation is a foundation, not a claim of full Samsung parity.

### FountainPen version selection refinement

FountainPen `GetStrokeDrawableGL` (`0x61a8c`) clamps the setting version to
1–18. GOT relocation `0xd5e78` resolves to `versionTable` (`0xdb8f0`), whose
8-byte entries select drawing and outline versions independently. Entry 18
is `(16, 12)`: fixture 02 selects drawing V16, not V17 or a hypothetical V18.
V16 `RedrawPen(MotionEvent...)` (`0x777c4`) calls `drawLine` (`0x77fd4`) at
`0x77c88`. That drawing routine constructs midpoint quadratics and passes
computed width through `WidthSmoothManager::getSmoothedWidthFromList` at
`0x7830c`, then interpolates widths across distance-spaced samples at
`0x78358`–`0x7839c`.

This is not a pure pressure law. The drawing routine uses a three-entry
geometric-ratio history, pressure, a tilt-derived parameter, previous width
and tool type. Saved-event redraw transforms tilt from radians to degrees,
clamps to 75 degrees and maps the portion above 15 degrees to 0–3
(`0x77c08`–`0x77c48`). Tolerance, width-manager mode, endpoint/tip processing
and the render-thread coverage shader must be matched before enabling V16.

The pure V16 width-limiter helper at `0x79298`–`0x792f4` now has a bounded
native oracle in [`conformance/native_ink.py`](../../conformance/native_ink.py).
It checks the library SHA-256, executes only that helper in ARM64 emulation,
and compares a float32 reconstruction over 4,128 deterministic cases. All
cases match bit-for-bit, exercising all 23 helper instructions. The helper
limits width changes, enforces pressure/size lower bounds and retains prior
width in its non-stylus unchanged-pressure branch. This validates one numeric
primitive, not the complete pressure law or full rendered pen.

After rejecting filled paths, the final prepared-primitive backend measures
346 ms cold seek versus 322 ms baseline, and 50 ms median/p95 warm seeks in
both builds. The reverse-seek test reproduces identical pixels; accounted
raster surfaces remain approximately 654 KiB for that measured viewport.
These are single-run host measurements, not a cross-device performance claim.
