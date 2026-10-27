# Stroke rendering findings

Saved FountainPen V14/V16 geometry is implemented in
`crates/sdocx/src/ink/fountain.rs`. The
[vector parity findings](fountain-parity.md) describe directional shading and
appearance limits; [native geometry checks](../../conformance/README.md#native-geometry-checks)
describe the retained validation harnesses.

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

## Rust decoding

The Rust parser walks `StoredPage` layers and recursive object records,
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

Static inspection of the Samsung Notes 4.4.45.37 ARM64 libraries
identifies drawing-time curves independent of the historical parser issue above.
This is instruction-level evidence, not a native runtime or pixel-equivalence
comparison. Fixture 02's rough strokes use FountainPen
`18;0;100;`. The DefaultPen trace below does not describe that fixture.

### Current SDK behavior

`prepare_stroke` chooses geometry shared by SVG export, vector PDF and replay.
Saved `FountainPen` settings `14;` and `18;0;100;` support input modes 1–3
and fixed width when the saved inputs pass `ink/fountain.rs` validation.
Both profiles produce circular stamps and report `InkSupport::Reconstructed`.
V14 also retains tangent directions for vector gradient shading. SVG consumers
perform edge antialiasing; zoom does not resample the native stamp geometry.

A saved `Marker2` stroke whose settings are absent or whose first settings
token is a nonnegative integer uses the same stamp path with one radius for
the whole stroke. Pressure does not change that radius. The saved ARGB alpha
is one `fill-opacity` on the path. V1 and V2 share this geometry; the V2
thin-edge shader is not ported. `Marker` stays on the pressure fallback.
`Marker3` and `StraightMarker` use the bounded V5 page route with empty or `5;`
settings and tool types 2 or 3; other settings remain generic. Marker4 and `StraightHighlighter`
settings `7;` and `8;` use rounded rectangular stamps; see [V7 findings](marker4-v7.md)
and [V8 findings](marker4-rendering-findings.md). Other Marker4 settings remain
on the fallback.
Composition selects root objects into Base, Top and Masking passes, preserving
stored order within each pass. The shared top batch uses Darken on light paper
and Lighten on dark paper; replay retains those SVG blend groups. See
[composition](../svg-rendering.md) and [theme policy](../render-themes.md).

Other pens and rejected saved settings use the pressure approximation. Rainbow,
eraser and straighten effects are not reproduced. Profile gates reject many
such inputs; Marker4 V8 accepts straighten and fixed-width flags without
implementing separate effect behavior. `stroke_paint` uses `pen_width / 2.5`,
clamped to 0.4–12, then multiplies each segment by
`0.3 + 0.7 * clamp(pressure, 0.05, 1)` when a
pressure channel is present. `render_stroke` draws one straight, round-capped
SVG line per point pair at that segment's starting width, or a straight
polyline when pressure is absent. A single sample becomes a circle. This path
does not build centerline curves or continuous width transitions, so straight
segments can show angular turns and width changes can show shoulders. Raising
output resolution does not remove those geometry differences.

`Stroke` carries optional `StrokeRendering`: pen name, settings, tool type,
and style. The approximation ignores timestamps, tilt, and orientation. The
fountain profiles use pressure, tilt, timestamps and geometric movement history.

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

A bounded host-Mesa paint capture joins one synthetic Drawing-selected
11-stamp stream (132 bytes) through native RT Update/Draw and host VBO upload,
with explicit factory/GLES adapters. Its 27 cases vary
projected diameter (1–3 pixels), phase (0, 0.25, 0.5 pixels), and native
`SetPenData` ARGB alpha (0, 128, 255). Red and alpha planes are byte-identical
across alpha controls, matching the fragment shader's use of `inputColor.xyz`
and derivative-smoothed coverage. The same vector is explicitly materialized
at RT+88; natural queue transfer, archive loading and device pixels are outside
this capture. Rust's circle SVG primitives and their vector-PDF conversion
cover more nonzero pixels than the native reference in all nine opaque controls.
Source-derived fringes reduce amplitude error but retain support/PDF tradeoffs.
Matrix receipt SHA-256:
`cc5379bc06dabd33f46e25fe9da4a5fea87f11c53812b3339c6ab9658847fde8`.

A second matrix holds that stream and opaque paint fixed across 56 cases:
projected diameters 0.5, 1, 2, 3, 4, 8 and 16 pixels, four XY phases, and
0°/45° rotations. All 112 red/alpha planes pass integrity and stream-join
checks with zero GL errors. An offline aligned 2×2 derivative model differs
by at most three alpha and two red byte values; continuous analytic derivatives are less
accurate. Unchanged source circles overpaint native support in both resvg and
MuPDF. These remain component comparisons, not full Rust exports or device
parity. Matrix receipt SHA-256:
`ff7d825dded274cd851547185293b3be4e6b8184ec51601c4f6599587cc6d6fe`.

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
pen actually applies smoothing. This base-method trace does not establish
the behavior of pen-specific callers or overrides.

The optional post-input coordinate smoother is a separate mechanism:
[stroke finalization](stroke-finalization-findings.md) documents that the
ordinary constructor selects no transformer. Input prediction and optional
beautification are distinct from saved-stroke redraw.

### Marker3 fragment coverage

The captured Marker3 mask uses `clamp((0.5-d)/fwidth(d),0,1) * alpha`,
where `d` is distance in the stamp's UV coordinates. On host Mesa, `fwidth`
uses per-lane finite differences across a 2×2 fragment quad. Substituting the
continuous distance gradient changes the fringe. Replaying the captured VBO,
projection and quad differences matches isolated-stamp support exactly and
alpha within two bytes across the tested 1×/2×/4× phase controls. At alpha 128,
duplicate stamps preserve the single mask and overlapping masks combine by MAX.
The bounded production route preserves native centers/radii in one compound
disk fill, applying source alpha once. On the 22-stamp saved control at
1×/2×/4×, disk geometry improves support and alpha error over the generic
pressure-width fallback in Firefox, resvg, MuPDF and PDFium. A 32-layer
continuous-fringe candidate is about 700 times larger and worsens high-zoom
error; it remains test-only. Neither transport reproduces fragment-quad
derivatives or proves device parity. The audit is in ignored
`tmp/vector-parity-plan/marker3-admission-next/`.

A synthetic [tool-3 control](../../conformance/marker3-tool3-native.json) matches tool 2 in curved native geometry and joined host-Mesa paint for two colors. The StraightMarker alias also matches the opaque control, and public SVG/PDF exports agree across these inputs. These captures use an explicit host bridge and do not establish device or saved-note parity.

On this Marker3 route, Drawing's `GetMorphable` call resolves to PenCommon's
null-return stub (`0x46468`); the guard at Drawing `0x81ef0` skips the curve
getter and setter. Two- and three-point controls produce identical complete
XY streams with curve enabled or disabled, so both use the same page sampler.
Drawing rejects point counts ≤1 (`0x81a64–0x81a78`); invoking an already selected
V5 redraw directly emits coincident initial/terminal stamps for one point,
which does not establish one-point Drawing admission.

## Saved rendering inputs

The semantic `Stroke` carries optional `StrokeRendering`, populated by the
existing bounded style decoder. It retains the complete known style,
properties and tool type, plus resolved pen/settings strings. Both document
parsing and debugger inspection/replay use `StrokeResources` to resolve IDs.
The modern name takes precedence; only an absent or -1 modern ID uses the
legacy ID. Original sample arrays are unchanged. Serialized strokes without
rendering metadata default to `None`.

All 77 strokes in fixture 02 resolve to
`com.samsung.android.sdk.pen.pen.preload.FountainPen`, settings `18;0;100;`.
DefaultPen curve findings alone therefore cannot establish parity for this
fixture. The [FountainPen findings](fountain-parity.md) cover its separate
renderer versions, pressure/speed width calculation, width smoothing and tips.

## Shared preparation and replay

`prepare_stroke` supplies geometry inputs, paint, conservative bounds and
profile/support status for SVG exports and vector replay. The fallback profile
borrows original samples for its pressure/width approximation. Single-sample
strokes produce a dot. The debugger uses prepared bounds instead of independently
estimating them, and exposes profile/support
status without asserting native parity.

The native registry is represented by `PEN_PROFILES`: 44 names, with aliases
kept separate and 30 distinct bundled library stems. A pen name alone does not
activate a reconstructed renderer. Saved FountainPen V14/V16 and Marker2 report
`Reconstructed` only when their inputs pass the profile's validation; Marker4
V7/V8 remain `Approximate` because vector outlines approximate native coverage.

Texture pens and live tips remain unsupported. Shared preparation does not
establish full Samsung parity.

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

The pure V16 width-limiter helper at `0x79298`–`0x792f4` has a bounded
native oracle in `conformance/native_ink.py` at revision `40de721`.
It checks the library SHA-256, executes only that helper in ARM64 emulation,
and compares a float32 reconstruction over 4,128 deterministic cases. All
cases match bit-for-bit, exercising all 23 helper instructions. The helper
limits width changes, enforces pressure/size lower bounds and retains prior
width in its non-stylus unchanged-pressure branch. This validates one numeric
primitive, not the complete pressure law or full rendered pen.

## FountainPen V16 saved geometry implementation

`ink/fountain.rs` reconstructs saved `FountainPen` / `18;0;100;` geometry.
Input modes 1–3 and fixed width are covered by native captures described in
[vector parity](fountain-parity.md). Unsupported inputs and expansion beyond
the preparation budget retain `Approximate`. `Reconstructed` describes native
geometry agreement, not identical edge antialiasing across SVG consumers.

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
  by the minimum inverse canvas scale; prepared geometry is
  stable across zoom levels rather than resampled for each native GPU scale.
- Float32 pressure, direction-history and tilt calculations, width limits,
  timestamp deduplication/interpolation and forward-window width smoothing.
  The stylus smoothing configuration is `(4, 8)` with factor 0.15. The native
  collection pass finalizes index `count - window + 1`; reproducing that timing
  matters, rather than applying a generic moving average afterward.
- The native ratio ring surviving between collection and redraw, constant-width
  final quadratic/endpoint, tap radius and minimum radius 0.1.
- Original-sample-to-generated-dot mapping, bounds from actual dot radii,
  bounded geometry expansion, and opacity applied once to each stroke union.

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
Texture pens, other FountainPen versions and the native
scale-dependent shader's thin-line/antialias coverage remain outside this
profile's validated scope.

The union of all 77 prepared stroke bounds, padded by three pixels, gives a
more relevant pen-focused comparison than the whole page: at the existing
ink threshold 32 and one-pixel tolerance, missing ink improves from 3.7626%
to 0.0360%, with 0% extra ink in both cases. The measurement uses 13,900
reference ink pixels; gray background dots or other objects overlapping these
regions are included. Reproduce it with:

```sh
cargo run -p sdocx-conformance --bin ink -- \
  /tmp/ink-geometry.json /path/to/reference_page0.png /path/to/sdk.png
```

The dense `handwritten.sdocx` fixture resolves to FountainPen `14;`.
Its separate Rust reconstruction retains native positions, radii, tangent
directions and sample boundaries. SVG/PDF use directional vector shading;
Bounded Chromium and Firefox appearance checks pass; native fringe and PDF
sampling differences remain. The [V14 findings](fountain-v14.md) describe its saved redraw;
[vector parity](fountain-parity.md) records current coverage and measured limits.

## ChineseBrush source and paint controls

A synthetic native Model/Drawing control with literal setting `4;` and no
`ObjectStroke::SetColor` call returned `0xff000000` from `GetColor` and passed
that value to both `Pen::SetColor` and `PenDrawableRT::SetPenData`. It selected
GLV3. The [compact receipt](../../conformance/chinesebrush-setting4-omitted-color-native.json)
records the inputs, native library hashes and calls. The capture uses Unicorn
and synthetic host inputs; it does not establish device or pixel parity. The
capture adapter can be run with its four pinned helpers restored from the parent
of commit `2ffac9f` into a temporary `conformance` directory, with `scratch`
linked to the native inputs. Its output is a full capture, of which the receipt
retains the color and selection evidence.

The imported shared `Pen::SetSize` clamps finite sizes to 0.4–800 before storing
them (PenCommon RVA `0x45fdc`, SHA-256
`afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d`).

For the pinned ChineseBrush DSO (`25d06b71e525f129e53c3734b957cdc9279ff9bd8dbca19948351c5e5ad237f5`),
GLV1/2/3 `endPen` at RVAs `0x32624`/`0x35868`/`0x385b4` reads the raw final
MotionEvent even when its movement was rejected. Quadratic tails retain the
accepted control point and use the raw endpoint and pressure. V2/V3 terminal
width uses stored accepted speed (`+280`); the spatial gate precedes timestamp
updates, and a zero previous timestamp selects the 50,000 time fallback.
The [Rust source regressions](../../crates/sdocx/src/ink/chinesebrush_glv123.rs)
pin native input bytes and complete packed streams for long and short controls,
plus exact output-budget boundaries. They establish synthetic saved-redraw
behavior, not app-deserialized admission or device pixels. The NoCurve matrix
adds 45 cases and 294 records: rejected raw endpoints still produce a terminal
line, duplicate tails emit no segment, and all-rejected inputs produce a
pressure-scaled dot at the last source point for multi-event sources.

Shared `ObjectDrawing::drawObjectStroke` rejects stored point counts 0 and 1
before `GetPenName`: Drawing RVAs `0x81a64–0x81a74` call `GetPointCount`, compare
against 1 and set error 7 on rejection. In-bounds singleton and empty native
Model arrays produce no pen instance, MotionEvent reads or queued work;
coincident and separated two-point controls both pass. This is a saved-object
Drawing gate, distinct from a standalone sampler's ability to construct a dot.
The capture receipt hashes to
`2a3f9031c334033fe4ead376d9a5a011a9672c8bc24d2f0a6db9038eda0bb279`.

The live alpha atlas uses nearest/clamp sampling; its complete 144×144 RGBA
upload hashes to `da75f5d1190e6fe086aa11007b7ed739421cf5d4522e34cb246df273353d2942`.
It reconstructs byte-for-byte from exported `SPen::g_chinesePtn` at file offset
`0x14480`: seven 48×48 RGBA cells occupy slots 0–6 of a zero-filled 3×3 atlas.
Fixed 321×228 logical-page, Y-down host captures use an overridden projection
and growing framebuffer for a synthetic 1×/2×/4× scale sweep; this is not native
canvas zoom. The shader's `fwidth(distance)` depends on angle and
fragment-quad phase. Constant radial gradients lose thin-stamp PDF support;
corrected angular alpha bands improve size-3.144 1× support IoU to 1.000 in
MuPDF and 0.971 in PDFium, with lower color error than matched generic lines.
These are per-scale host comparisons, not device or arbitrary-zoom parity.
Minimum-size 0.4 controls still miss sparse native pixels at 2×/4×; an empty
native 1× frame has no active-region error metric and cannot establish parity.

The Y-down projection maps the quad's negative-Y edge to page top, where the
shader samples atlas row zero. Original atlas rows therefore need no SVG image
flip; the positive-Y capture requires the opposite orientation. Masks keep RGB
white and fade alpha alone. The corrected atlas extraction uses the full
144-pixel row stride; earlier contiguous-cell extraction results are invalid.
The corrected comparison receipt hashes to
`f4b2b0388f3984f5b2f23d5dd911fed39f029ec17762ba6ba4400b555d736a1e`.
Actual applied host matrices were checked with `glGetUniformfv`; they override
the native matrix and do not establish Samsung's stroke-cache projection.

Native source-array read hooks and AddPoint boundaries also establish cumulative
record cuts for curved and NoCurve saved redraws. Rust checks all three versions,
including rejected and duplicate terminal events; these cuts describe saved
redraw attribution, not live app replay. The source-boundary receipt hashes to
`76f44ce5613fe5253fe39ade58782d61e7272578b28e40707c593d75b1934730`.

The RTV1 `CreatePenCanvas(int,int,queue)` name does not establish target sizing:
at `0x3b5d0` it discards those arguments and dispatches `Init()` through vtable
slot `+0x30` (`0x49690 → 0x3b0f4`). `Draw` at `0x3b9f4` receives an external
`ISPSubBitmap` and combines its matrix with the caller's matrix before shader
submission. The fixed 144×144 pattern atlas is a separate allocation. Native callback
workers enumerate tiles of the existing canvas bitmap; the bitmap's upstream
size producer and application/device transform remain unresolved.

The native matrix helper at `0x3bc98` computes `M_subbitmap × embed(M_canvas)`.
A separate capture calls native canvas `SetMatrix` and forwards the shader
uniform unchanged, then verifies it with `glGetUniformfv`. At fixed 321×228
allocation, canvas scales 1/2/4 produce X/Y terms
`(2/321, 2/228) × scale` and 33/125/501 alpha-support pixels for one synthetic
stroke. Growing allocation alone divides the projection terms and preserves
pixel scale. The capture receipt hashes to
`dc948f2d0e21aa730881ff717d89e6d052a13550f1be119187783361a01e1b70`;
it does not establish application matrix provenance or device rasterization.

A CPU mirror of the shader's 2×2 derivative lanes reproduces the minimum-size
synthetic host readbacks byte-for-byte, including their faint singleton pixels.
Changing alpha-band counts trades missed coverage for false positives; more
bands do not consistently improve support. These derivative/quantization
controls use the overridden Y-down sweep, not the native canvas-zoom capture.
Their receipt hashes to
`c680c9ec6f88866287a06110f5a8cd7e8f3586a2c49128ce2340455c2e851cfe`.

## Crayon2 V2 saved playback

The [Rust geometry checks](../../crates/sdocx/src/ink/crayon2.rs) compare three
synthetic Model/Drawing controls, including negative coordinates and a phase
transition, against all six packed native lanes. They use an explicit captured
host TYPE_3 RNG adapter; Android runtime RNG equivalence remains unverified.

A passive observer of the 268-instance, 256×256 host-Mesa paint control preserved
the original readback SHA-256
`a19f9befa9e35ad2aef2251cd90c28d0f13b1be0041334c196b6692571b20c00`.
The projection maps one source unit to one pixel with translation `(64,64)`.
Background sampling uses `(subbitmap dimension / 150) / background scale`.
GLV2 derives the scale from the runtime display as
`f32(min(screen_width, screen_height)) / 1440.0`; the divisor is the native
binary32 literal `0x44b40000`. The observed display and Pen setter both supplied
`1920×1080`, yielding `0.75` and `uBgTextureScale = 2.2755556` on both axes. The background sampler uses unit 0 with nearest/repeat; the point atlas
uses unit 1 with nearest/clamp. Paper sampling was disabled.

Live vertex-attribute queries join attribute 0 to the actual 64-byte quad upload
(stride 16, divisor 0); attributes 1 and 2 join the 6,432-byte instance upload
(stride 24, divisor 1, offsets 0 and 16). The quad's `(x,y,u,v)` rows are
`(-1,-1,0,1), (-1,1,0,0), (1,-1,1,1), (1,1,1,0)`, so the top of a stamp
uses point-atlas row 0 within its selected tile. Its upload hashes to
`a39b6ddc1dbf16f1c1efb544817dc4ca4de6b575365af723878516f6afb558e4`;
no later update to that buffer occurs before the observed draw. The unchanged
frame and actual attribute bindings close the source-upload-to-draw join,
without requiring a GPU buffer readback. These are host playback settings;
exported SVG/PDF texture fidelity remains open.

An intrinsic-image SVG prototype using SDK resvg 0.47 reproduces the same 184
covered pixels for both opaque and half-alpha source colors on a transparent
background. Alpha differs by at most 3/255; mean absolute error over coverage is
0.315/255 and 0.712/255 respectively. Zero-alpha controls match exactly on both
backgrounds, but colored pixels still differ, so this is not paint parity or a
production route. Focused probes isolate two SVG transport losses: stamp-sized
mask regions clip subpixel coverage, and nearest-filtered images still have
antialiased outer boundaries. Full-canvas mask regions and atlas padding beyond
one output pixel restore the sampled point/background bytes exactly (229 and 53
at pixel `(83,186)`), while retaining the original quad and texture coordinates.
Padding uses adjacent atlas rows and clamps at the complete atlas edges; it does
not clamp each selected tile independently. This correction reduces the opaque
source's maximum alpha error from 208/255 to 3/255; filter quantization and
compositing remain unresolved. These measurements apply only to this captured
host-Mesa scene and renderer, not arbitrary scale, PDF export or device output.

## Legacy Marker source-to-mesh controls

Two synthetic four-point native Model strokes, one straight and one bent, now
exercise cold Drawing through GLV1 and the queued RTV1 producer. The
[compact receipt](../../conformance/legacy-marker-source-geometry-controls.json)
retains the source arrays, GL geometry calls, RTV1 mesh inputs and output bytes,
GL upload hashes and final host-Mesa frame hashes. The five native draw vertex
counts are 24/6/32/32/6 for the straight control and 56/6/64/64/6 for the bent
control. Both completed the guarded queue and passive readbacks without GL errors.
These are synthetic host controls, not serialized-note or device captures.

The producer seam is `MarkerStrokeDrawableRTV1::calcBezierPath`, its recursive
`subdivideBezierPath` and `calcSegmentsNumber`, followed by `MeshMathV1::mergePoints`
and cap-mesh construction. The receipt records entry registers and mesh vectors;
it does not recover the subdivision decision, intermediate control-point memory,
or a source-to-mesh implementation. Consequently the native output cannot yet
justify admitting arbitrary saved strokes into a vector renderer.

The copied control scripts are in `tmp/legacy-marker-source-controls/`. Each
retains the V31 preflight and pins its own four-point input and expected draw
counts. Run from the repository root with `PYTHONPATH=scratch/apk-analysis-runtime/python:tmp/vector-parity-plan/montblanc-drawing-capture-root-v2/conformance`,
then `python3 tmp/legacy-marker-source-controls/straight.py --capture --advanced-setting null --output <new-json> --texture-payload-output <new-bin> --quiet-json`;
substitute `bend.py` for the second control. Output paths must be new. The scripts
require the pinned local native inputs and helpers checked by their preflight.

A [synthetic serialized control](../../conformance/legacy-marker-synthetic-serialized-drawing.json)
replaces a FountainPen frame's pen-name table entry with Marker and omits its
advanced setting. Native binary decoding and the Rust parser agree byte for
byte on all five 204-point channels. After running the Model initializer, the
native constructor, binary reader and
`OnAttach` resolve the name without supplying the implementation pointer,
visibility or name. Native Drawing selects GLV1 and its 20 queued tasks produce
a host-Mesa frame identical to the earlier seeded control. Host no-op UUID
callbacks and synthetic ModelContext, canvas and queue adapters remain. Against
that frame on white, the previous generic SVG/PDF support IoUs at
1× are 0.367/0.369 (threshold 250); native GL row indices map directly to page
Y under the supplied host matrix. This is one synthetic frame, not a real Marker
note, Android or full native note-loader comparison. The reduced parser fixture
retains only the selected handwritten stroke, source identifiers/timestamps and
original string table; other objects, pages and media are removed. It adapts
[SDOCX Compatibility Corpus](https://huggingface.co/datasets/twangodev/sdocx-compatibility)
by its contributors under CC BY 4.0. Its integrity trailers are not regenerated
for Samsung loading.

Two [six-point synthetic controls](../../conformance/legacy-marker-long-native-controls.json)
cover multiple turns and short rejected moves. Two 0.5-unit intermediate moves
are rejected by native GLV1; the resulting native vectors and frame match the
corresponding four-point bend. A separate multi-turn stroke completes 12 queued
tasks and records 21 native mesh-vector outputs. On the paired host-Mesa frame,
generic SVG/PDF support intersections over unions are 311/425 and 311/427 for
the skipped control, and 475/602 and 475/607 for the multi-turn control. These
are 1× MuPDF comparisons of synthetic strokes with the explicit host matrix,
not native PDF or Android-device parity.

Four two-point native controls include vertical, horizontal, diagonal and
reverse strokes. Native EndPen's initial-state branch adds the stored f32 `0.2`
to the final X coordinate; a straight centerline candidate improves RGB L1 at
1× in the captured controls. A [2× host projection override control](../../conformance/legacy-marker-two-point-controls.json),
verified by uniform readback, worsens SVG/PDF L1 (50732/53759 versus
50586/53588 for fallback). The profile therefore keeps two-point strokes on
fallback. The override does not establish Android canvas zoom or device parity.

Four [finished-stroke host controls](../../conformance/legacy-marker-finished-prefix-native.json)
cover stationary input, two- and three-point prefixes and a repeated final point.
Stationary input rejects both intermediate moves but paints 70 host pixels.
A repeated final point changes the completed frame and native draw geometry
relative to the three-point prefix. For four-point bent and straight controls,
the opt-in centerline model retains the previous normal, uses zero final length
and sets the terminal control to the midpoint of the sampled tail and endpoint.
Its centerlines match the native controls within 0.00005 page units. At 1× on
MuPDF, the bent control's SVG/PDF RGB L1 error changes from 13776/14162 to
13139/13456; the straight control is unchanged. The comparison is against the
host frame on #fcfcfc, not native PDF output. These are completed redraws, not captures of
native progressive replay; replay cutoff semantics and device appearance remain
unverified.

A separate [720 by 480 screen control](../../conformance/legacy-marker-varied-screen-control.json)
uses a six-point stroke, size 8 and blue at alpha 200. Profiled SVG/PDF white
RGB error falls from 25227/25973 to 13347/13974 against its host frame. This
supports a second explicit context, not general screen or device parity.

### Quadratic subdivision controls

Two synthetic native Model/Drawing controls exercise GLV1 and RTV1 with straight
and bent four-point strokes. The [entry trace](../../conformance/legacy-marker-subdivision-controls.json)
records each `calcBezierPath` and `subdivideBezierPath` control-point tuple,
index interval and relevant RTV1 state bytes. These are host Unicorn/Mesa runs,
not serialized-note or Samsung-device controls.

In the pinned Marker library (SHA-256
`f874166ba82cb4491b2551c9ea02c5891787a755167e67008234d50c8ba6e7a6`),
`subdivideBezierPath` at RVA `0x34868` stops when `end - start < 2` and
otherwise uses the integer midpoint `(start + end) / 2`, writes the midpoint
pair to two RT vectors, then recurses left and right. Its float operations form
quadratic half subdivisions. A separate test script in
`tmp/legacy-marker-source-controls/check_subdivision.py` recursively computes
quadratic de Casteljau midpoints from the captured root tuples and checks all
native entry intervals and points. All 15 straight and 47 bent recursive entries
match the interval order. A Rust reconstruction follows the native weighted-term
operation order and matches the captured coordinates bit for bit in all 62
recursive entries. Full source-to-mesh generation remains unverified.

The initial interval comes from `calcSegmentsNumber` at RVA `0x34138`, whose
branch depends on RT byte offset 716 and other state. In the non-circle branch,
the code truncates the average of floats at offsets 708 and 712, multiplies it
by `abs(input - 1)`, truncates, adds three and caps the result at 20. The other
branch uses an angle-derived count. Initialization of the state and complete
source-to-mesh generation remain unresolved.

The trace scripts `straight-trace.py` and `bend-trace.py` in
`tmp/legacy-marker-source-controls/` extend the earlier capture scripts with
read-only entry snapshots. They require the pinned native inputs and preflight
of the V31 runner. The trace receipt hashes each full capture and script.

The captured straight control has direction dots 1, 1, 1 and lengths 8/0,
8/8, 9/8, yielding counts 3, 3, 3. The bent control has dots 1, 0, 0 and
lengths 8/0, 8/8, 8/8, yielding counts 3, 11, 11. The [test-only Rust model](../../crates/sdocx/src/ink/legacy_marker_segment_count.rs)
checks all six against the native intervals. These are finite synthetic host
controls; general initialization remains unverified.

For finite values, the native selector sets the circular flag when the direction
normal dot is at most the double-precision constant -0.7 and the distance from
the first to second control point exceeds the float constant 0.6. Its circular
count uses 180 degrees when `abs(dot)` is between the stored f32 thresholds
0.996 and 1.004; otherwise it computes `acosf(dot) * 180 / pi`, then returns
twice the truncated result divided by 18. The same receipt adds two independent
host controls: an exact reversal selects 20 segments, and an oblique reversal
selects 16. The latter executes host glibc `acosf`, `cbrt` and `sincosf`
callbacks in Unicorn; it does not establish Android libm behavior, boundary
rounding or device rendering. The test-only model checks the selector and count
for all six calls in those two controls. Capture scripts are `reverse-trace.py`
and `turn-trace.py` in `tmp/legacy-marker-source-controls/`.

Two additional native straight and bent controls retain StartPen and EndPen
entry state and the subsequent task-state deltas in the subdivision receipt.
StartPen copies the initial point to all five position slots at RT offsets
652–691, zeros offsets 692–715 and sets byte 717 to 1. A test-only model matches
both complete captured 80-byte post-task slices. For a noninitial EndPen,
RTV1 constructs an SmPath quadratic from the point at offset 684, control at
660 and endpoint at 676, then replaces offset 684 with `getPosTan(length - 1)`
before emitting the terminal curve. The observed sampled points are
(20.8753967, 20) and (12.7874975, 27.9563293) for the straight and bent
controls. The shared Rust SmPath quadratic reproduces both within 0.00001 page
units. Short or empty paths and the initial-EndPen branch remain open; these
synthetic host observations do not establish device or saved-note parity.

In two synthetic curve-disabled Drawing controls, `calcPathNoCurve` uses two
edge points for aligned normals and 23 for turns. Its native branch uses 23
when the normals' dot product is negative or the difference between their
length product and absolute dot is at least the stored f32 0.08; it then calls
`makeCircleInternal`. All six observed edge-vector counts match the test-only
model. These host controls do not establish threshold-edge, device or full
no-curve mesh parity. The existing subdivision receipt retains state and vector
hashes; temporary scripts retain the full captures.

Eight [thin Marker host controls](../../conformance/legacy-marker-thin-host-controls.json)
cover saved sizes 0.4, 1.0 and 1.9, straight/bent/shifted points and source
alpha 0, 128 or 255. In five controls quantizing to width 1, the native core
edge pairs coincide, fringe inputs contain NaNs and all five host-Mesa draw
prefixes leave a transparent frame. The two 0.4-unit straight controls and one
bent control instead paint pixels. Generic SVG/PDF exports paint 26–52 pixels
at width 1 in four paired controls. The empty frames are host observations:
NaN handling, other transforms, output scales and Android appearance remain
unverified, so these results do not justify suppressing thin strokes generally.

### Legacy Marker subdivision edges and fringe

The [edge test](../../crates/sdocx/tests/legacy_marker_edges.rs) joins the
subdivision-entry controls to the existing mesh-input receipt. For the two
captured strokes, each interior subdivision sample is the quadratic midpoint,
offset on each side by the normal perpendicular to the vector from the local
start to end, scaled by the radius at RT offset 644. The first and last sample
instead use the saved normals at offsets 700 and 692 respectively. The test
checks every captured sample within 0.00003 coordinate units; that tolerance
covers float evaluation-order differences and is not a pixel-parity claim.

Native instructions at RVA `0x34aa8`–`0x34ae4` construct the fringe from the
pair of body edges: `delta = (first - second) / (2 * radius)`, then
`first_outer = first + delta` and `second_outer = second - delta`. The observed
radius is 5.5 in both controls. The [merge and cap test](../../crates/sdocx/tests/legacy_marker_mesh.rs)
checks all 30 captured merge and cap calls, including each accumulated output
vector, against the original return bytes. These checks cover only the captured
nondegenerate subdivision branch and cap calls; they do not establish behavior
for tiny tangents, other branches, arbitrary saved strokes, or device output.

A test-only four-point reconstruction now produces all five body, fringe and
cap vertex streams from the source coordinates and size; both host controls
match the native streams within 0.00005 coordinate units. Feeding those streams
through the test-only native-paint model, including cap textures and depth,
differs from the final native frames at 12 straight and 11 bent pixels, with
maximum channel error one byte. The sparse retained frames have SHA-256
`7266a5a2d494b566f7e5643c013bfd1f30a5c417c348d03115f5863586d920fd`
and `62f15c9c6302de5dcae527b1b463e5cc65c695a37c4dcd9cc521f6006407cc36`;
the test reconstructs and verifies their full-frame hashes against the source
geometry receipt. A diagnostic SVG union with solid caps and no fringe differs
from native at 180 and 174 pixels in resvg (maximum channel error 83). Its public
SVG-to-PDF exports rasterized with PyMuPDF differ from native at 180 and 177
pixels. This simplified vector prototype does not reproduce cap masking, depth
ownership or fringe interpolation and does not justify public admission. Its
reproduction is retained in `scratch/legacy-marker-vector-prototype.rs`.

The generic Marker fallback now uses saved ARGB alpha for compressed tool-2
strokes with null settings and supported metadata. For the captured 128-alpha straight
and bent controls, resvg differing pixels fall from 386/341 to 328/288 and
PyMuPDF PDF differences from 386/341 to 337/296. Captures at
[1](../../conformance/legacy-marker-alpha01-control.json),
[64](../../conformance/legacy-marker-alpha40-control.json) and
[200](../../conformance/legacy-marker-alphac8-control.json) also improve absolute
channel error in both outputs. The [zero-alpha capture](../../conformance/legacy-marker-alpha00-control.json)
still has residual cap ink, omitted by whole-stroke zero opacity; its absolute
error nonetheless falls from 77698 to 9672. Intermediate alpha values and the
fallback geometry remain approximations, and device parity is unverified. Unknown
metadata keeps the prior fallback. The original solid-union prototype also improves
on the old opaque fallback for these controls but omits cap texture, fringe and
depth behavior; it is not admitted. Reproduction is retained in
`scratch/legacy-marker-fallback-prototype.rs` and
`scratch/legacy-marker-vector-prototype.rs`.

For the same Marker metadata profile, the generic fallback now uses a constant
width derived from the saved size when the native integer width is at least two;
it retains the previous pressure-based approximation for smaller widths and
unknown metadata. Native `Pen::SetSize` clamps finite values to 0.4–800, then
`PenDrawableRT::SetPenData` at RVA `0x4a588` truncates the float to an unsigned
integer; RTV1 computes its body radius from that integer. The Model controls at
2.5 and 12.75 produced identical vector and final-frame hashes to 2 and 12,
respectively. Replacing the four pressure samples with all ones also left every
native vector and the final frame unchanged for the straight size-12 control.
The [width receipt](../../conformance/legacy-marker-width-controls.json) records
these host controls and the audited size-24 and size-48 queue shapes.

On the straight controls at sizes 2, 3, 6, 24 and 48, constant width reduced
resvg absolute RGBA error from 15386/22816/48643/289559/718745 to
1850/2969/10367/144762/298130; PyMuPDF comparisons likewise improved. The size-12
bent control improved from 97063 to 48983. The fallback remains a round polyline,
not the native mesh or textured cap. At size 1 and at 1.25, 1.5 and 1.75, both
integer- and half-pixel straight controls yielded transparent native frames;
all tested nominal-width and blending approximations increased error against
them, so these thin widths retain the previous fallback. Size 0.4 produced
nonzero pixels and remains unverified beyond the captured straight stroke.
Other curves, subpixel alignments, size values and device output remain outside
the comparisons; the integer threshold at 2 follows native quantization but the
fallback is not a native rendering of the thin bucket.

The source-centerline model reconstructs the observed
quadratic subdivision and exact-reversal circle centers from source points,
including the terminal `SmPath` sample. The reversal's interior edge-pair
midpoints agree within 0.00001 units with the center selected by `makeCircle`
at RVA `0x3423c`; the [compact reversal receipt](../../conformance/legacy-marker-reversal-circle-control.json)
records native edge pairs and the source capture and script hashes. The turn
control selects `makeCircularSector`: the current test-only model
reconstructs its unique dividing-parameter cubic root and samples the quadratic
at nine-degree tangent increments. The [sector receipt](../../conformance/legacy-marker-sector-center-control.json)
retains native cubic entry, dividing-parameter return and edge pairs. The model
rejects non-monotonic or multiple-root cases and many stroke shapes. A diagnostic
round-polyline through captured native centers reduced
resvg absolute RGBA error on bend, turn and reversal from 48998/21527/11923 to
39207/17138/4571; source-generated centers reproduce those diagnostic metrics
on the four controls. Other branch and paint coverage remain insufficient for native paint parity.

Before the first accepted move, GLV1 compares each coordinate delta to
`0.4 * size` and queues a move only when either absolute delta is strictly
greater than that threshold. After acceptance it clears its first-move flag and
uses the display-dependent threshold instead. In six [queue controls](../../conformance/legacy-marker-move-filter-controls.json),
size-12 deltas below or equal to 4.8 were skipped, a 4.9 delta was accepted,
and a diagonal `(4,4)` was skipped despite exceeding 4.8 in Euclidean length.
A repeated final point still queued EndPen. These queue observations do not by
themselves establish execution of every rejected-guard task. For the captured
bool-false path with PenData dimensions 1920 by 1080, the
later threshold is `1.5 * min(1920,1080) / 360 = 4.5`; post-first-move queue
controls reject 4.5 and accept 4.6. Static code selects a factor of 10 instead
of 1.5 when the bool argument is true; that branch has not been runtime-tested.
A separate [guarded dispatch receipt](../../conformance/legacy-marker-skipped-center-control.json)
confirms the complete native output for one skipped move; replacing it with a
repeated point produced identical native mesh and frame hashes. The
source-centerline model takes the PenData dimensions explicitly,
follows the observed bool-false filter and matches the retained center samples.
The true-bool branch, missing moves and other root configurations remain
outside the verified boundary. Longer captured paths are
covered by the same model; unsupported cases fall back.

The mirrored sharp-turn sector is now included in the sector receipt and passes
the test-only source-centerline comparison. On that control, generated-center
round-polyline error decreased from 21608 to 17205 absolute RGBA units in resvg
and from 20958 to 16966 in the SVG-to-PDF/PyMuPDF comparison. These are host
controls and do not resolve native cap or compositing differences. A separate
context trace shows native Drawing invoking two host context callbacks returning
1920 and 1080, then calling `Pen::SetScreenResolution(1920,1080)` before GLV1
reads those values. The callbacks are explicit synthetic context inputs, not
proof that exported page dimensions equal a real device screen resolution.

Independent cold strokes made from the first two and first three source points
queue different native event sequences: the two-point case has StartPen then
EndPen with no MovePen; the three-point case has one MovePen before EndPen. The
two-point guarded paint completed with four draws. These are prefix controls,
not an observed incremental replay session; the generated-centerline model does
not yet reconstruct the no-MovePen case or replay cutoffs. Their queue and
capture provenance is retained in the move-filter receipt.

For the reduced synthetic serialized Marker fixture, native Model returns
opaque black when ARGB is absent, while the parser preserves the absence. The
bounded Marker fallback now recovers that default, applies the captured
quantized width, and preserves an explicitly supplied public color. On the
isolated 204-point stroke, white-background PyMuPDF RGB L1 decreased from
290682 to 72066 for SVG and from 289413 to 72190 for PDF. The tracked regression
checks the parsed missing field, opaque width and RGB, SVG versus the host
native frame, and a valid public PDF export. This is a host comparison on a
reduced synthetic archive, not complete-note or Android-device parity.

In the pinned Drawing library, `ObjectDrawing::drawObjectStroke` at RVAs
`0x8238c`–`0x823f4` reads two resolution values through the drawing context's
profile vtable and passes them to the selected pen's `SetScreenResolution`.
The host control supplies those callback values explicitly. The retained
serialized fixture does not establish the original device resolution, and a
page's dimensions do not determine it; an experimental centerline profile would
need an explicit source for the PenData resolution rather than infer it from
page export size.

The opt-in Marker centerline route also accepts explicit empty advanced settings
with saved ARGB or omitted color. In five [paired host controls](../../conformance/legacy-marker-empty-setting-host-pairs.json),
null settings select native GLV1 and empty settings select GLV2; all draw-prefix
frames and uploaded geometry buffers match between each pair. The controls cover
straight, bent and longer strokes, multiple colors/alphas and a verified 2× host
projection override. Public empty-setting SVG/PDF RGB L1 improves over fallback
in all five comparisons. Three additional empty-setting controls omit the native SetColor call: Model
returns opaque black, and their host frames match explicit-black controls at
every draw prefix. Public SVG/PDF errors improve over generic fallback on
straight and bent 1× and bent 2× controls. The default generic route is
unchanged; other settings retain fallback. These are synthetic host-Mesa
controls, not native PDF, Android zoom or device parity. Literal `1;` also
selects GLV1 and matches those null/empty host controls in five straight, bent,
long, color/alpha and 1×/2× pairs, including all draw-prefix and upload hashes.
One omitted-color `1;` control returns opaque black and matches the corresponding
empty-setting frame. Public SVG/PDF comparisons have the same error metrics as
the empty-setting opt-in controls. Other literal settings remain fallback.

An explicit `LegacyMarkerCenterlineProfile` now lets a caller provide the
PenData screen dimensions for an experimental vector approximation. It uses
the recovered centerline only for guarded legacy Marker metadata and supported
geometry; unsupported cases and replay use the existing fallback. Round caps,
fringe and overlap are approximations, not the native textured paint. On seven
synthetic host controls, public SVG/PDF white-background RGB L1 against native
was no worse than fallback: bend changed from 17658/18448 to 14405/15085,
reversal from 5061/5761 to 2600/3231, and a longer multi-turn stroke from
21333/22676 to 15406/16540. The 204-point synthetic serialized control changed
from 72066/72190 to 35973/36092. These comparisons use explicitly supplied
1920 by 1080 host dimensions and do not establish saved-device or full-note
parity.

An experimental triangle-paint SVG uses the source-generated four-point mesh,
the pinned native cap texture as a binary clip, per-triangle fringe gradients
and ownership masks. Against the straight/bend native host frames on white,
resvg RGB L1 was 3700/2939 and public SVG-to-PDF raster L1 was 6067/4505;
the current centerline profile measured 13502/13401 and 14074/13715,
respectively. Direct MuPDF SVG rendering instead painted mask regions black,
raising error to 107323/102841. An alpha-mask variant did not fix this reader
incompatibility. The prototype covers only the two four-point source controls,
uses the pinned native texture and approximates native depth ownership; it is
not admitted to production. The ignored scratch scripts
`scratch/legacy-marker-generated-mesh-export.rs` and
`scratch/legacy-marker-triangle-paint-probe.py`, together with
`tmp/marker-triangle-export`, retain the experiment. The generated-mesh export
requires temporarily including the scratch Rust test in the mesh module; remove
that include after running it. Render comparisons do not establish device parity.

A separate ordered-paint scratch prototype consumes the pinned five-draw VBO
and cap textures, retaining sequential depth ownership, affine fringe and cap
alpha, and the final cap's source-over operation. On the captured 1920 by 1080
host control composited on white at one pixel per page unit, its SVG/PDF RGB L1
errors were 9452/11195, with maximum channel error 82. This is not evidence for
production admission or device parity. Replacing repeated ownership paths with
nested shared SVG definitions reduced source size from 1,100,452 to 103,136
bytes while producing byte-identical rendered PNG and PDF. The PDF remained
471,778 bytes after the export pipeline expanded the definitions; budgets must
therefore account for expanded work and page totals, not just SVG bytes. Masks
were bounded to the 39 by 20 stroke region. The ignored
`tmp/marker-ordered-prototype/` retains both scripts, the export harness and
comparators; its limits are experimental per-stroke limits, not a page budget.

The direct MuPDF SVG black pixels were traced to its handling of SVG linear
gradients: a standalone gradient also rendered black. A gradient-free scratch
variant subdivides fringe triangles and cap UV into constant-opacity bands.
At two bands, straight/bend white RGB L1 was 5495/4161 in resvg, 4071/3641
in direct MuPDF SVG, and 6086/4679 in public PDF rasterization. Four and eight
bands increased error on these controls, consistent with antialiased band seams.
Even two bands produced 35/77 KB SVG and 82/186 KB PDF per stroke, and only
the four-point mesh is source-generated. This remains a diagnostic, not a
scalable general-stroke implementation. The ignored
`scratch/legacy-marker-triangle-band-probe.py` retains the band experiment.

A scratch general-length ordinary-quadratic mesh reproduces all five native
vertex streams within 0.00005 for the six-point long-turn and size-8 blue
720×480 controls. It rejects circular/sector branches. Grouping same-opacity
fringe polygons reduces repeated PDF objects and seams: on white, the long-turn
resvg/PDF RGB L1 is 4651/5495 versus 13568/14240 for the current profile;
the PDF is 18 KB versus 2.5 KB. For the blue control it is 10265/10414 versus
12646/13146. Grouping does not prove native depth ownership for self-overlap,
and the SVG remains about 96 KB for each six-point example. This is a scratch
experiment with no production dispatch; resource budgets, overlap, sector and
scale behavior remain unresolved. Reproduction uses
`scratch/legacy-marker-long-generated-mesh.rs` and
`scratch/legacy-marker-triangle-grouped-probe.py` with the temporary export
harness in `tmp/marker-triangle-export`.

The scratch general-length mesh also reconstructs original and mirrored sector
edges within 0.000029 units. Grouped two-band paint reduces white-background
resvg RGB L1 from 5847/5867 to 3425/3306 and public PDF L1 from 6514/6541 to
4052/3923 (original/mirrored), but produces roughly 56 KB SVG and 15 KB PDF per
stroke. It uses the pinned cap texture; scale and arbitrary cubic roots remain
unverified.

A six-point closed self-overlap control exposes an ownership error: grouped paint
worsens resvg RGB L1 from 11401 to 16874 and PDF L1 from 13119 to 18130, even
though generated geometry matches native within 0.00005. The retained
`tmp/legacy-marker-source-controls/self-overlap.py` capture script has SHA-256
`1ebe79c76474f7bede40b006fcc800a62a4002e787d17a4dc4d0c16402f8448b`; its
`self-overlap-confirmed.json` capture has SHA-256
`63135813951f5a4821430cdbe7628ebec3befd5aadd96154df9d06b82cbd5aba`.
The existing scratch mesh and grouped-paint probes reproduce the comparison.
This route requires correct overlap ownership and output budgets before admission.

Normalizing body-triangle winding removes cancellation at the self-overlap:
on the original host frame, profile versus normalized grouped mesh white RGB
L1 is 11401 versus 8725 in resvg and 13119 versus 10521 in public PDF. A
blue, alpha-200 overlap control gives 18535 versus 14730 and 19837 versus
15216. On the verified explicit 2× projection bend, the corresponding values
are 55870 versus 8974 and 57691 versus 11875. These remain approximations:
the captured native order depth-tests the body, start cap and both fringe
strips, then paints the end cap with depth disabled. Grouping bands does not
reproduce first-fragment ownership in overlapping fringe triangles or the final
cap's source-over behavior. A source-geometry overlap guard and per-page output
budget have not been established, so no production paint profile is admitted.
The normalized `scratch/legacy-marker-triangle-winding-probe.py` SHA-256 is
`867f3b05c2e8ad7e4e65355999ee9784cb7705ad5ba02c2547439c67a9f12ddb`;
`tmp/legacy-marker-source-controls/self-overlap-blue.py` and its JSON capture
hash to `6ff9a01bc5de4480eccece2e974b9199ed10765df86822f8fdf04f5002ae495d`
and `c940623a8cb4dccb87c963db2dc2c58526867e441b9468873e11862a48c19f0e`.
The retained `bend-null-scale2.json` capture hashes to
`d739615d3cd7e3a9d677b03cfe14677e85cbf8bf0775d0f65ab7d46c2fdcdab0`.

A scratch Boolean prototype subtracts already owned triangle regions and unions
visible fringe by opacity band. On white, native RGB L1 for resvg/public PDF
was 5107/6829 for self-overlap, 4340/5003 for a long turn, 8717/8750 for a
blue stroke and 8536/11195 for the verified 2× bend; the existing profile gave
11401/13119, 13568/14240, 12646/13146 and 55870/57691 respectively. PDFs were
7–15 KB per stroke. These use captured meshes separately checked against the
source model; first-fragment coverage and arbitrary transforms remain unverified.

A checked fixed-scale variant changed the long comparison by at most five RGB
L1 units. A shared local origin stabilized its geometry under a ±10-million-unit
translation, but this does not establish native translated parity: converting
that mesh to native f32 loses up to 0.5 units. Experimental pair/vertex limits
accepted straight, bend, long and one sector control but rejected the mirrored
sector, blue and overlap cases. Stress tests and a wasm32 compile did not prove
worst-case allocations or browser runtime behavior. No production route is
admitted without safe intermediate-allocation and page-output budgets. The
retained probes are `tmp/marker-owner-boolean/src/main.rs`, its `src/bin/checked_local.rs`
and `pair_budget*.rs`, and `scratch/legacy-marker-triangle-boolean-grouped-probe.py`.

A single local integer grid avoids repeated floating-point adapter recentering.
At 10,000 grid units per page unit and the experimental 65,536 pair limit,
the turn, mirrored turn, long, blue varied and bend controls pass; the
self-overlap exceeds the body-triangle limit and falls back. Matched-white
resvg/PDF RGB L1 is 2436/3072 and 2444/3056 for turn/mirrored, 4340/5001
for long, 8717/8746 for blue, and 8537/11191 for the verified 2× bend. These
paint diagnostics use captured mesh streams previously compared with the source
model. The grid choice is an approximation, not native precision evidence.
`tmp/marker-owner-boolean/src/bin/integer_grid.rs` and `integer_budget.rs` hash
to `688d56d07ecf054662c73591ff3ce0c38805ed67425928ee6130885cd3c65ce7` and
`cacaef78c0a6748f45d7578b54c721c7d449dac2ebc2a9c4ad6f5a215fec8169`.

The experimental pair check bounds only the edges supplied to each overlay,
not the library's internal work. In the locally installed `i_overlay` 9.1.0,
`split/solver_list.rs` can repeat intersection scans after splitting rounded
intersections and switch to the tree solver as the segment list grows;
`split/solver.rs` reserves and appends split segments within the overlay call.
The caller cannot enforce its input pair limit on those intermediate segments.
This source inspection does not quantify a worst case, so admission still needs
an enforceable internal work/allocation limit or a proved geometry restriction,
as well as a cumulative per-page output and work budget.
The budget diagnostic compiles for wasm32 but uses unchecked casts and panic
on guard failure; deterministic intermediate allocations, atomic page budget,
browser runtime and general native parity are not established. No production
mesh-paint route is admitted.

A separate ordered scratch experiment unions the constant-alpha body triangles,
then subtracts prior ownership from each fringe triangle on a 10,000-unit-per-page-unit
integer grid and paints its visible polygon with an affine gradient. It retains
start-cap depth masking and the final cap's source-over order. On white at the
captured host resolution, resvg/public PDF RGB L1 was 2195/4830 for straight,
1934/3663 for bend, 2259/4931 for a long turn, 2786/7057 for self-overlap,
5821/9941 for a blue varied stroke, and 5455/11684 for the 2× bend. The
corresponding PDFs were approximately 34, 71, 121, 374, 176 and 72 KB per
stroke. These are captured meshes previously checked against the source model;
they do not establish device or arbitrary-transform parity. The experiment has
no enforceable overlay intermediate-allocation or cumulative page budget and
is not admitted to production. Reproduction is retained in
`scratch/legacy-marker-ordered-visible-gradient.py`,
`scratch/legacy-marker-ordered-visible-controls.py`,
`scratch/legacy-marker-ordered-visible-compare.py`, and
`tmp/marker-owner-boolean/src/bin/visible_gradient.rs`, using the existing
`tmp/marker-ordered-prototype/harness` for resvg and public PDF export.

A bounded convex-subtraction scratch probe replaces the overlay library with
half-plane clipping. Its floating-point version changes fragment counts under
large translations. A checked-i128 rational variant rejects arithmetic overflow,
collapsed nonzero triangles, nonconvex caps, and work or vertex-budget overruns.
For the long and 2× bend controls, resvg/PDF RGB L1 was 2260/4978 and
5455/11688, with PDFs of approximately 184 and 100 KB. The checked 2× mesh
retained 506 polygons and 1902 vertices when translated by ±10,000 and ±900,000
units; translating back changed converted coordinates by at most 5.9e-11. The
instrumented binary rejects the blue control because four nonzero triangles
collapse on the grid, and self-overlap at its 256-piece limit. A Python rational
mirror with a higher piece limit reaches its 12,000-output-vertex limit; that
does not establish the checked binary's next rejection reason. These experiments
do not prove general numerical or native parity or implement cumulative atomic
page budgets. No production route is admitted. Retained probes are
`scratch/legacy-marker-bounded-clip.py`,
`scratch/legacy-marker-ordered-bounded.py`,
`scratch/legacy-marker-rational-clip.py`,
`tmp/marker-owner-boolean/src/bin/rational_checked.rs`,
`scratch/legacy-marker-ordered-checked.py`, and
`scratch/legacy-marker-checked-clip-stress.py`.

A finer 100,000-unit grid admits the blue control under the checked clipper's
existing per-stroke limits, with resvg/PDF RGB L1 5821/10041 and a 242 KB PDF;
the 1,000,000-unit grid rejects under its combined failure condition. A scratch
page harness preflights grid collapse, retries at the finer grid and charges
both attempts to a shared operation budget. Under a 500,000-operation limit,
eight blue strokes admit two and fall back for six, with 12,282 admitted fringe
vertices. This does not yet budget source mesh generation or expanded SVG/PDF
work and is not integrated into production. The probes are
`tmp/marker-owner-boolean/src/bin/rational_page.rs` and
`scratch/legacy-marker-ordered-checked.py`.

An end-to-end scratch harness feeds the recovered source-point mesh into the
checked adaptive clipper. For long and 2× bend controls, all five generated
vertex streams differ from captured mesh by at most 0.000020 and 0.000040 page
units, respectively. White resvg/PDF RGB L1 is 2260/4978 and 5455/11691;
the PDFs are about 184 and 100 KB. Source-generated blue still falls back
because nonzero triangles collapse even on the finer grid, and self-overlap
exceeds the piece limit. A page with a degenerate stroke and eight long strokes
falls back for the degenerate stroke, admits three long strokes, then reaches
the 500,000-operation limit and falls back for the rest. The scratch generator
also limits cumulative source points and mesh vertices. SVG/PDF expansion and
intermediate allocation are not fully page-budgeted, and this is not a
production route. Reproduction uses `tmp/marker-owner-boolean/src/bin/source_to_mesh.rs`,
`tmp/marker-owner-boolean/src/bin/rational_page.rs`,
`scratch/legacy-marker-source-page-driver.py` and
`scratch/legacy-marker-ordered-source-page.py` with the comparison harness above.

A direct PDF scratch exporter paints the ordered clipped geometry with axial
luminosity masks and a separate start-cap ownership mask. Against the long and
2× bend native controls, white MuPDF/PDFium RGB L1 was 4425/3974 and
10157/9357; the existing SVG-to-PDF path was 4978/4250 and 11691/10019. The
direct PDFs were 93 and 50 KB versus 184 and 100 KB. A source-size, path-command,
object-count and staged-byte guard rejects oversized scratch inputs before
saving; an oversized-source check left no PDF. These controls do not establish
general reader parity, production allocator limits or atomic page fallback.
The retained diagnostic is `scratch/legacy-marker-direct-pdf-probe.py`, with
`scratch/legacy-marker-direct-pdf-compare.py` for paired readers. The PDFium
check used the local pypdfium2 installation under the temporary verification
virtual environment.

A checked Rust scratch writer consumes a typed, bounded direct-PDF page plan and
emits the same ordered path and cap-mask structure. On source-generated long and 2× bend
controls, MuPDF/PDFium RGB L1 was 4951/4249 and 11410/10018, with 86 and 48 KB
PDFs. Its probe bounds plan bytes, items, path commands, object count and staged
stream bytes, plus serialized output during writing. An oversized plan was
rejected while preserving the existing output. These bounds do
not prove the PDF library's internal allocation bound, and the writer's color
and reader behavior still needs validation before integration. The retained
probes are `tmp/marker-owner-boolean/src/bin/direct_pdf_checked.rs` and
`scratch/legacy-marker-direct-plan.py`.

A scratch unit-square Form variant normalizes the direct geometry to explicit
stroke bounds and reapplies the corresponding page transform. For the long and
2× bend controls its MuPDF/PDFium RGB L1 was 4951/4249 and 11410/10019,
with PDFs of about 86 and 48 KB. This tests the coordinate strategy for the
existing retained-ink marker transform. A temporary test-only payload then
ran actual Krilla marker emission and replacement; the imported form needed a
unit-space vertical flip. Long and 2× bend MuPDF/PDFium RGB L1 was 4951/4249
and 11410/10019, with 107 and 59 KB PDFs. This test imports a capped scratch
PDF graph, not a production form compiler or cumulative page budget. The
retained prototype and temporary test patch are
`tmp/marker-owner-boolean/src/bin/direct_pdf_form.rs` and
`scratch/legacy-marker-krilla-replacement.patch`. No Marker production dispatch
is enabled.

A scratch typed Form compiler now prepares its resource graph before marker
replacement, charging attempted path work to a page budget and committing
resource reservations only after successful compilation. In a temporary
Krilla test, the source-generated long control rendered at MuPDF/PDFium RGB L1
4951/4249, with a 107 KB PDF. The detached graph must be made reachable from a
temporary trailer root before `lopdf` renumbers its object references; omitting
that step produced invalid resources and a large visual error. Three focused
scratch tests passed, including budget reservation rollback. This remains a
test-only graph import without production scene integration or a proven PDF
library allocation bound. Reproduction is retained in
`tmp/marker-owner-boolean/src/bin/direct_pdf_prepared.rs` and
`scratch/legacy-marker-prepared-krilla.patch`.

A scratch Rust pipeline now carries the recovered source mesh and clipped
polygons as typed values into an ordered PDF plan, without the earlier SVG/JSON
path-plan intermediary. It accepts explicit ARGB and projection scale and
tracks cumulative source points, mesh vertices, clipping work, visible
vertices and prepared-form resources. Long and 2× bend source controls pass;
blue and self-overlap fall back. The 2× direct PDF's MuPDF/PDFium RGB L1 was
11410/9995 and its size 49 KB. This is not wired to `CompositionContext` and
does not prove production page fallback or PDF-library allocation bounds.
The retained pipeline is `tmp/marker-owner-boolean/src/bin/source_to_plan.rs`
and `clip_typed.inc`, using the neighboring source mesh and prepared-form
modules.
The typed scratch preparation now returns a precompiled Form and namespaced SVG
carrier together, reserving both only after successful preparation; a focused
page-budget rollback test passed. The carrier is still raw SVG, not Scene nodes.
