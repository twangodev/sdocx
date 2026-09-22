# Stroke rendering findings

> Parser findings and the FountainPen V16 geometry in
> `crates/sdocx/src/ink/fountain.rs` describe this checkout.
> `conformance/fountain-v16.json`, `conformance/fountain_native.py`, and
> `conformance/ink_visual.py` are here. `conformance/native_ink.py` and the
> other retired experiment scripts are at Git revision `40de721`. V14 is not
> implemented in Rust. See
> [current validation](../../conformance/README.md#native-geometry-checks).

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
comparison. Fixture 02's rough strokes were later identified as FountainPen
`18;0;100;`. The DefaultPen trace below does not describe that fixture.

### Current SDK behavior

`prepare_stroke` chooses the geometry, and SVG export and Canvas replay share
it. A saved `FountainPen` stroke with settings `18;0;100;`, stylus tool type
2, and inputs accepted by `ink/fountain.rs` becomes circular stamps.
`render_stroke` draws those stamps as one filled SVG path. The debugger's
`replay-raster.ts::drawStroke` fills the same circles when `dot_radii` is
present. That profile is `InkSupport::Reconstructed`: stamp positions and
radii follow the native saved-stroke helpers. SVG and Canvas antialiasing are
not Samsung's GPU shader, and zoom does not resample the stamps.

A saved `Marker2` stroke whose settings are absent or whose first settings
token is a nonnegative integer uses the same stamp path with one radius for
the whole stroke. Pressure does not change that radius. The saved ARGB alpha
is one `fill-opacity` on the path. V1 and V2 share this geometry; the V2
thin-edge shader is not ported. `Marker` and `Marker3` stay on the pressure fallback. Marker4 settings `8;`
now use a shared rounded rectangular stamp approximation; see
[Marker4 findings](marker4-rendering-findings.md). Other Marker4 settings and
straight-line aliases stay on the fallback. Strokes with `top_layer_pen` are
drawn after the page's other objects, in stored order, inside one
`mix-blend-mode:darken` group. Debugger canvas replay applies the same per-stroke opacity to the stamp
union, but does not apply that batch blend.

Every other pen stays on the older approximation. That includes FountainPen
settings `14;`, fixed width, rainbow, eraser, straighten, and any V16 or
Marker2 input the checks reject. `stroke_paint` uses `pen_width / 2.5`, clamped to 0.4–12,
then multiplies each segment by `0.3 + 0.7 * clamp(pressure, 0.05, 1)` when a
pressure channel is present. `render_stroke` draws one straight, round-capped
SVG line per point pair at that segment's starting width, or a straight
polyline when pressure is absent. A single sample becomes a circle. This path
does not build centerline curves or continuous width transitions, so straight
segments can show angular turns and width changes can show shoulders. Raising
raster resolution does not remove those geometry differences.

`Stroke` carries optional `StrokeRendering`: pen name, settings, tool type,
and style. The approximation ignores timestamps, tilt, and orientation. The
V16 profile uses pressure, tilt, and timestamps.

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
Samsung exports before replacing a pen's renderer. Saved FountainPen V16 and Marker2 now use this shared layer.

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
kept separate and 30 distinct bundled library stems. At this stage every
recognized profile reported `Approximate`; a pen name alone still does not
activate an unverified renderer. FountainPen V16 and Marker2 are later exceptions, and
only when their saved inputs match.
The existing 48 MiB replay tile cache and optimized backend primitives are
retained. A filled-capsule path experiment was rejected after cold seeks rose
from approximately 322 ms to 461 ms; splitting it into fragments did not fix
that regression. None of that experimental path/cache code is shipped.

Textures, live tips, and the other bundled pens remain unfinished. Saved
FountainPen V16 and Marker2 are the reconstructed profiles. Shared preparation
is not a claim of full Samsung parity.

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
(`0x77c08`–`0x77c48`). Tolerance, width-manager mode, and endpoint processing
are included in the V16 geometry port below. The render-thread coverage
shader is still not ported.

The pure V16 width-limiter helper at `0x79298`–`0x792f4` now has a bounded
native oracle in `conformance/native_ink.py` at revision `40de721`.
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

## FountainPen V16 saved geometry implementation

`ink/fountain.rs` now reconstructs the saved stylus profile selected by
`FountainPen` / `18;0;100;`. This is active shared rendering, not just registry
recognition. Other versions, fixed-width strokes, unsupported inputs and
expansion beyond the preparation budget retain `Approximate`. The new profile
reports `Reconstructed`: native geometry is reproduced, while SVG/Canvas
antialiasing is not the Samsung GPU shader.

The native saved-object entry (`0x7851c`) performs a width collection pass,
completes smoothing, then redraws with timestamp-indexed widths. A nonzero tip
length enables these passes (`0x78794`–`0x7886c`); `18;0;100;` sets tip unit 0
and length 100 through `SetAdvancedSetting`. The saved redraw itself calls
`drawLine` for interior samples and `endPen` for the last sample; it does not
run the live PointTipManager event prediction again.

The implementation includes:

- PenTolerance's large/small thresholds, direction checks and farthest dropped
  event, plus V16's alternating short-movement filtering.
- Midpoint quadratics, native SmPath subdivision/length parameterization and
  distance-spaced circular stamps. The canonical inverse scale is one, giving
  repeat distance 0.82. Native `getInverseScale` multiplies that repeat distance
  by the minimum inverse canvas scale; our prepared geometry is currently
  stable across zoom levels rather than resampled for each native GPU scale.
- Float32 pressure, direction-history and tilt calculations, width limits,
  timestamp deduplication/interpolation and forward-window width smoothing.
  The stylus smoothing configuration is `(4, 8)` with factor 0.15. The native
  collection pass finalizes index `count - window + 1`; reproducing that timing
  matters, rather than applying a generic moving average afterward.
- The native ratio ring surviving between collection and redraw, constant-width
  final quadratic/endpoint, tap radius and minimum radius 0.1.
- Original-sample-to-generated-dot mapping, bounds from actual dot radii,
  bounded geometry expansion, and one fill per stroke in each backend.

`conformance/fountain_native.py` executes the actual hash-pinned FountainPen,
PenCommon and Base helpers in Unicorn. Memory allocation is bounded, each
native call has an instruction limit, and unresolved imports fail closed.
Object initialization/two-pass orchestration are reconstructed; native
MotionEvent endpoint getters are stubbed, and `drawPoint` is intercepted to
capture circles. This is an independent geometry oracle, not a GPU capture.
The checked-in `fountain-v16.json` contains synthetic channels and native
results for 26 cases (1,869 dots), covering taps, short strokes, repeats,
turns, reversals, tilt/pressure changes, timestamps, sizes and seeded paths.
It is checked by ordinary Rust tests without needing the APK or Unicorn.

To check the native reference and a real parsed document:

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_native.py
cargo run --offline -p sdocx --features render,serde --example ink_geometry -- hf/02-shapes-and-dot-calibration.sdocx > /tmp/ink-geometry.json
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_native.py --prepared /tmp/ink-geometry.json
```

All 77 fixture-02 strokes produce the same 3,777 dots and sample mappings as
the native helpers. Maximum position difference is 0.0000611 page units;
maximum radius difference is 0.000000716. The ordinary whole-page PDF visual
comparison improves from 1.25% missing / 0.72% extra ink to 0.72% missing /
0.69% extra ink (0.83% changed pixels). These percentages include the page's
shapes and background; they are not per-pen or cross-device parity guarantees.
Texture pens, other FountainPen versions, fixed-width mode and the native
scale-dependent shader's thin-line/antialias coverage remain outside this
profile's validated scope.

The union of all 77 prepared stroke bounds, padded by three pixels, gives a
more relevant pen-focused comparison than the whole page: at the existing
ink threshold 32 and one-pixel tolerance, missing ink improves from 3.7626%
to 0.0360%, with 0% extra ink in both cases. The measurement uses 13,900
reference ink pixels; gray background dots or other objects overlapping these
regions are included. Reproduce it with:

```sh
uv run --project conformance --locked python conformance/ink_visual.py \
  /tmp/ink-geometry.json /path/to/reference_page0.png /path/to/sdk.png
```

A same-fixture Chromium comparison against the isolated pre-preparation
`119705d` build measures cold seek at 26.1 ms before and 25.2 ms after; both
measure 50 ms median / 50.1 ms p95 warm seeks, identical pixels on reverse
seeks, and approximately 1.38 MB accounted raster surfaces. These are single
host runs. The browser metric now counts fills as well as stroke calls so
native circular paths are included in drawing-work reports. Chromium and
Firefox both pass the replay interaction checks for this fixture.

The older dense `handwritten.sdocx` fixture resolves to FountainPen `14;`.
It intentionally remains `Approximate`; V16 is not silently applied to that
older drawing version. Its native renderer needs a separate port/reference
check before it can use reconstructed geometry.

The same APK already contains `FountainPenStrokeDrawableGLV14`, so another
APK download is not currently necessary to investigate `14;`. Its saved
redraw (`0x727dc`, inner loop `0x72c20`) differs from V16: it uses a single
pass without the width-history smoothing, a simpler distance filter, and
includes the final sample in the line loop before `endPen`. Its `drawPoint`
(`0x731b0`) passes a tangent to `FountainPenStrokeDrawableRTV4::AddPoint`.
That tangent-dependent backend still needs tracing before circular stamps
can be assumed to reproduce V14 coverage. These are investigation findings,
not enabled V14 support.
