# FountainPen stamp rasterization

Current implementation work is tracked in [fountain parity](fountain-parity.md).
Canvas replay coverage lives in Rust; vector export shading remains unfinished.
The GPU shaders are used only as a conformance reference. Historical
statements below describe earlier implementation stages, not current coverage.

> The reproduction commands in this note refer to experiment scripts removed
> during test cleanup, including `conformance/fountain_raster.py` and
> `conformance/fountain_gpu.mjs`. Recover them from Git revision `40de721` in
> a separate checkout. Saved V16 stamp positions are implemented; these
> shaders are not. See
> [current validation](../../conformance/README.md#native-geometry-checks).

This extends the saved-geometry findings with the rendering backend from the
same hash-pinned Samsung Notes 4.4.45.37 ARM64 libraries. It does not yet enable
these shaders in the SDK. `FountainPenStrokeDrawableGLV14` uses RTV4; GLV16 uses
RTV5. Drawing versions and shader versions are different numbering schemes.

## Native instance attributes

`RTV4::AddPoint` at `0x9de50` and `RTV5::AddPoint` at `0x9f700` share the
following calculation. Every operation is float32; preserving those roundings
matters for a numerical comparison.

For input radius `r` and positive inverse canvas scales `(sx, sy)`:

```text
rx = r / sx; ry = r / sy
factor = min(rx, ry) < 0.5 ? 0.5 / min(rx, ry) : 1
alphaByWidth = 1 / factor
ex = rx * max(factor, 1) + 0.5
ey = ry * max(factor, 1) + 0.5
extent = (ex * sx, ey * sy)
innerRadius = 0.5 * (max(ex, ey) - 1) / max(ex, ey)
```

Zero scale returns without appending a stamp. Negative scales and radii have
not been characterized. The instance record is center, extent, transparency,
innerRadius, alphaByWidth; RTV4 inserts the supplied direction vector between
extent and transparency. RTV4 does not normalize that vector in AddPoint.

For example, radius 0.1 at unit inverse scale produces extent `(1, 1)`,
innerRadius 0, and alphaByWidth 0.2. This is substantially different from
rasterizing an opaque radius-0.1 circle with a browser's ordinary antialiasing.

`conformance/fountain_raster.py` executes the actual native AddPoint methods
and compares each emitted float's bits against this reconstruction. All 2,496
cases match, covering both backends, boundary radii, unequal scales, opacity,
zero-scale early exits, and deterministic random inputs. It constructs only
the vector and scale fields these functions read; GPU initialization is not
emulated in this test.

## Vertex and fragment behavior

RTV4's initialization selects StrokeShaderV4, StrokeAlphaShaderV4 and
StrokeCompositeShaderV4. The V4 vertex shader orients its quad using direction
`d` and perpendicular `(d.y, -d.x)`; it then scales each resulting coordinate
by the corresponding extent. Under unequal scales, this order matters.

For texture coordinates relative to the stamp center, `u` and `v`:

```text
distance = hypot(u, v)
edge = distance > innerRadius
     ? 1 - (distance - innerRadius) / (0.5 - innerRadius)
     : 1
directionalGradient = clamp(1 - (0.93 / 0.25) * (abs(v) - 0.25), 0, 1)
V4 pointAlpha = transparency * directionalGradient * alphaByWidth * edge
```

The V4 gradient is flat over the central half of the quad, then decreases
toward its texture-y edges. The native vertex/UV arrays map texture y along
the supplied tangent, not along its perpendicular. The production-versus-native
pixel comparison added on 2026-09-25 verifies that mapping. V4's direct color
shader multiplies `(RGB, 1)` by pointAlpha;
its alpha-only shader writes pointAlpha, and its composite shader applies the
stroke color alpha afterward. The draw-time choice and blend state are traced
below; the SDK does not yet implement these paths.

V5 uses an axis-aligned quad and omits the directional gradient. Its direct
color shader applies a circular mask, the same edge and width compensation,
then multiplies by both stamp transparency and input color alpha. The native
mask expression divides by zero exactly at distance 0.5; this boundary is
undefined for a portable scalar reference and is explicitly counted/skipped
by the conformance tool if encountered.

## GPU conformance and its limits

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_raster.py --output /tmp/fountain-raster.json
node conformance/fountain_gpu.mjs /tmp/fountain-raster.json
```

The first command extracts the unmodified V4/V5 and live-tip vertex and fragment
shaders into the local output JSON, after verifying the library hash. No APK
shader source is checked into this repository. The second uses the web app's
Playwright installation and Chromium/SwiftShader to execute those shaders.

The reference interpolates native quad UVs after float32 vertex arithmetic
and snapping to the GPU's reported subpixel grid. Omitting that step caused
an apparent opacity error of 0.0153 in tiny rotated stamps; this host reports
four subpixel bits (a 1/16-pixel grid). Reading interpolated UVs in a float
render target confirmed that snapping explains the discrepancy.

The original V4/V5 subset, with snapping accounted for, has 1,252 visible
stamp cases / 5,128,192 pixel
comparisons pass, with maximum alpha error 0.00196081, approximately half an
RGBA8 quantization step. No undefined circle-boundary samples occurred. Cases
with either extent above 20 are excluded to keep the full quad inside the
64-pixel target; zero-scale cases emit no stamp. The test tolerance is 0.003.
This verifies isolated stamp coverage, including rotated/unequally scaled
cases, on this software GPU. It does not prove Samsung device precision,
live tip prediction, erasing, or complete stroke parity.

## Draw-time compositing

RTV4::Draw (`0x9e66c`) and RTV5::Draw (`0x9fda4`) have the same three-way
selection based on two flags. PenCommon `SetEnhancedAntiAlias` (`0x4a5b0`)
writes byte 68; `SetRedrawState` (`0x4a4a4`) writes byte 25. These are not
opacity flags. `SetPenSettingData` also copies the enhanced-AA setting from
PenSettingData byte 52 to byte 68.

| Enhanced AA | Redraw | Stamp stage | Page stage |
| --- | --- | --- | --- |
| false | either | Direct color shader, source-over | Same target |
| true | false | Direct color shader, componentwise maximum | Same target |
| true | true | Alpha shader, maximum into an intermediate surface | Composite shader, source-over |

The two blend descriptors created by each Init method are `(0, 1, 7)` and
`(4, 1, 1)` for both RGB and alpha. Their meaning is verified by executing
the native descriptor setters and GLES `BlendStateObject::Activate` from
libSPenRenderer, intercepting its final GL calls:

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_blend_native.py
```

- `(0, 1, 7)` becomes `FUNC_ADD`, `ONE`, `ONE_MINUS_SRC_ALPHA`.
- `(4, 1, 1)` becomes `MAX`, `ONE`, `ONE`.

The Renderer library hash is pinned alongside the three geometry libraries.
For enhanced-AA saved redraw, stamp overlap therefore takes the maximum
coverage, not repeated source-over accumulation. The final composite applies
the stroke color/alpha once. V4's intermediate coverage is read from alpha;
V5's is read from red. This distinction is explicit in their native composite
fragment shaders.

`fountain_gpu.mjs` now also executes the unmodified native alpha/composite
shaders, using the verified blend modes. It checks 24 combinations of backend,
radius and stroke alpha, three overlapping stamp transparencies, a repeated
stamp, and a translucent background. Maximum coverage matches exactly; all
393,216 final RGBA channel comparisons differ by at most half a byte. The
isolated-stamp check above independently verifies the underlying coverage
formula; this additional check isolates compositing from that calculation.
The test stores coverage in RGBA8 for readback, including V5's red output;
it does not claim to reproduce every native intermediate texture format or
device driver. Native Draw's branch selection was inspected in disassembly;
its renderer/texture object lifecycle was not emulated in this GPU test.

The SDK still draws opaque circular paths for its reconstructed V16 profile.
Its geometry checks remain valid, but native pixel coverage requires a backend
that implements these scale-dependent attributes, shader coverage and the
correct compositing path. V14 additionally requires tangents and its separate
saved-stroke geometry algorithm.

## Live-tip attributes and shader execution

The same tools now cover FountainPenStrokeTipDrawableRT::AddPoint
(`0xa6a8c`) and SetTotalLength (`0xa7398`). There are 1,269 additional
deterministic tip cases: boundary and random widths, unequal X/Y inverse
scales, opacity, RGB, distance, endpoint alpha, and positive/zero/negative or
absent total lengths. All eleven float32 attributes match native bytes exactly,
including the positive-length normalization that ordinary live Draw sequences
did not exercise. The combined attribute suite has 3,765 cases. The live Draw
harness reuses this model instead of maintaining its own tip attribute formula.

The tip record is `[x, y, extent_x, extent_y, opacity, distance_alpha,
inner_radius, alpha_by_width, r, g, b]`. Its extent, inner radius and subpixel
alpha calculation are shared with RTV5. With a positive total length, the
distance field becomes `fma(-(1 - endpoint_alpha), distance / total_length, 1)`;
the subtraction and division are rounded to float32 before the final fused
operation. Non-positive total lengths leave the attribute unchanged.

The unmodified native tip vertex/fragment shaders are extracted into the
temporary JSON, not checked into the repository. The GPU test verifies
629 visible tip stamps in addition to V4/V5: 1,881 stamps and 7,704,576 pixel
coverage comparisons overall, plus 7,729,152 tip RGB channel comparisons.
Maximum channel error is 0.0019608022, approximately half an RGBA8 step.
There were no skipped undefined circle-boundary pixels in this run.

The fragment shader uses per-stamp RGB, circle coverage, width compensation,
input opacity and uniform color alpha. It does not use the distance varying.
For each visible tip case, replacing that attribute with -100, 0, 1 and 100
produces byte-identical RGBA pixels: 2,516 separate checks. This supports
omitting a distance fade for this shader, despite the normalization code.

Tip Init constructs source-over at `0xa5f84..0xa5fb8` and componentwise maximum
at `0xa5fcc..0xa6000`, using the same descriptors already resolved by
`fountain_blend_native.py`. In tip Draw, byte 68 selects enhanced AA. When
it is false, the direct path uses source-over (`0xa7530`). When true and
bytes 25 and 237 are both zero, it uses maximum (`0xa7528`). The other branch
at `0xa75c4` involves an intermediate surface; its shader and copy-geometry
checks are described below.

The GPU test checks both direct modes over a translucent background, varying
size and uniform alpha, three overlapping differently colored stamps, and a
repeated stamp. All 24 combinations / 1,572,864 RGBA channel comparisons pass.
Maximum mode matches exactly. Source-over's largest error is 1.14902 bytes
against a reference reconstructed from separately read RGBA8 source pixels.
The bound includes half a byte each for source color and target quantization,
plus `destination_byte / 510` for source-alpha quantization and a 0.01-byte
precision margin. It is not a widened coverage-model tolerance.

These results concern Chromium/SwiftShader. Native Android driver behavior,
canvas clearing and complete live-frame pixel comparison remain unverified.

## Intermediate live-tip surface

The tip's intermediate branch uses FountainPenStrokeTipAlphaShader (native
vertex `0x42c2a`, fragment `0x42ea9`) and then
FountainPenStrokeTipCompositeShader (vertex `0x430fd`, fragment `0x431b3`).
Init stores those shader objects at offsets 200 and 208 respectively. Draw
selects maximum blending at `0xa761c`, runs the alpha shader, then selects
source-over at `0xa76a4` and runs the composite shader on the page target.
The extraction tool now exports these unmodified shaders into its temporary
JSON alongside the existing shaders.

Despite its name, the tip alpha shader stores **straight RGB and coverage
alpha separately**, not just a scalar coverage mask. Its RGB is the stamp's
color over the entire rasterized quad; circle coverage only affects alpha.
Consequently, zero-alpha corners can still contain RGB. Componentwise MAX
then independently selects each RGB channel and alpha across overlapping
quads. For different stamp colors, the result need not be one of those colors.
This behavior must not be replaced by premultiplied-color maximum or by
selecting the color belonging to the highest-coverage stamp.

The composite shader reads that RGBA texture, computes
`alpha = texture_alpha * uniform_color_alpha`, and outputs
`(texture_rgb * alpha, alpha)`. It discards fragments when alpha is zero.
Uniform RGB does not tint this path. Source-over blends the result into the
page. The test independently checks intermediate RGB against the quad bounds
and stamp color, and intermediate alpha against the already-verified direct
shader at unit uniform alpha. It then checks channelwise maximum and the
final composite equation over a translucent background.

`fountain_blend_native.py` now executes the actual branch instructions at
`0xa7510` for all eight combinations of enhanced AA, redraw and rainbow flags:

| Enhanced AA | Redraw or rainbow | Selected path |
| --- | --- | --- |
| false | either | Direct source-over |
| true | false | Direct maximum |
| true | true | Intermediate maximum, then source-over |

The branch harness stops at the selected successor; it does not emulate the
surrounding renderer object lifecycle. It also executes the complete native
`updateCopyGeometryData` method (`0xa6308`), intercepting only its final
geometry upload. Given `[left, right, bottom, top]`, the six uploaded vertices
are `[left,bottom, left,top, right,bottom, right,bottom, left,top, right,top]`.
Four input rectangles, including degenerate and inverted bounds, match that
mapping exactly. A null geometry pointer performs no upload.

The GPU check uses those six vertices for full, cropped and empty copy
rectangles. Native composite UVs come from absolute normalized-device
coordinates, so cropping does not stretch the intermediate texture. With
four stamp sizes, four uniform alpha values (including zero), three overlapping
colors and a repeated stamp, 48 composite cases / 786,432 final RGBA channel
comparisons pass with maximum error 0.5 byte. Intermediate stamp color and
alpha checks cover 196,608 pixels. The original direct and saved-stroke
checks continue to pass in the same run.

This verifies the shader stages, their arithmetic/blending, the local branch
selection and copy-quad construction. Intermediate texture creation/format,
surface clearing and reuse, conversion of dirty rectangles into these bounds,
asynchronous renderer messages, and complete Android live-frame output are
still outside this test. The GPU harness uses an RGBA8 intermediate surface
with nearest sampling; it does not claim to have established every native
surface allocation parameter.

## Canvas acquisition, reuse and clipped clearing

`conformance/fountain_canvas_native.py` executes the tip's
CreatePenCanvas, ClearPenCanvas, Clear and SetCanvasCleared methods. It uses
host canvas/manager interfaces and replaces label construction with empty
strings; it does not execute the manager's string-key cache or allocate a
native GL texture. Sixteen acquisitions cover four dimensions, two
initialization states and one/two-sub-bitmap responses.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_canvas_native.py
```

CreatePenCanvas (`0xa68ec`) requests a surface through the manager's virtual
GetPenCanvas method, using the **manager's queue**, not its own supplied queue
argument. It passes width/height, integer 0, boolean false, texture-memory
layout value 6 and target-layout value 2. These are observed numeric enum
values; their underlying native formats are not yet resolved here. The
PenCommon vtable relocation at `0x76d98` identifies the manager call as
GetPenCanvas, rather than CreatePenCanvas.

Immediately after acquiring the returned surface, native code stores it at
RT offset 160 and calls its Clear(0) interface. This happens on every tested
acquisition, including when the host returns the same surface repeatedly.
It then requests initialization if RT byte 144 is false, records width/height
at offsets 240/244 and sub-bitmap count at 248. The test intercepts
initialization; it does not emulate its renderer allocations.

The manager's cache-hit size gate at PenCommon `0x487e8..0x48868` is also
executed directly for sixteen cached/requested dimension pairs. It selects
reuse only when both width and height match. A mismatch reaches the branch
that releases the cached canvas and replaces it. This check starts after
name lookup has found an entry; cache-key equality, replacement allocation
and reference-counting are not part of it.

The three clear-related operations have different effects:

- **ClearPenCanvas** (`0xa6a70`) dispatches Clear(0) to the retained surface;
  a null surface makes it a no-op. Repeated calls repeat the dispatch.
- **Clear** (`0xa7444`) resets the consumed draw vector's end to its beginning,
  drops its pointer at RT offset 88 and clears redraw byte 25. It preserves
  the fill-vector pointer at 80 and the vector's allocated capacity. It does
  not issue a pixel clear. Empty, populated and already-null cases are tested.
- **SetCanvasCleared** (`0xa5bc0`) enqueues a virtual ClearPenCanvas member
  call (slot 80) when the manager has a queue. It does not clear synchronously;
  with no queue it enqueues nothing. The harness captures the task and
  explicitly executes its native body, as detailed below; Android worker
  scheduling remains outside this test.

The graphics implementation adds a crucial qualification: Clear(0) respects
the active clipping state. The harness now also loads hash-pinned
libSPenGraphics (`aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621`).
Its SPPenCanvasRT::Clear(int), at `0xb0400`, converts color zero to RGBA
`(0,0,0,0)` and calls SPPenCanvasImpl::ClearRT (`0xafd0c`). For texture-backed
bitmaps, that method visits each sub-bitmap, enables its clip, dispatches the
transparent clear, then disables the clip. With no texture bitmap it uses
the fallback surface and clip index -1. An empty sub-bitmap collection issues
no surface clear. All four paths (fallback, zero, one and three sub-bitmaps)
execute in the harness; clipping setup and final GPU clear are recorded host
boundaries.

The queued task's vtable relocation at FountainPen `0xd28a8` identifies its
Member0 execution body at `0x63804`. It loads the saved target and member
encoding, adjusts the target by the signed member adjustment, resolves
virtual slot 80, then tail-calls it. The harness now executes this actual
body using the task captured from `SetCanvasCleared`, with slot 80 bound to
native `ClearPenCanvas` and the retained surface bound to native
`SPPenCanvasRT::Clear(int)`. All four clear paths above execute through this
complete dispatch chain. The harness triggers execution explicitly; it does
not run Android's queue scheduler or replace clipping with a whole-surface
clear.

These checks establish dispatch and buffer-state behavior, not that every
tip Draw starts from a wholly blank texture. Proving absence of stale pixels
requires the enclosing frame's clear notification and clip/dirty-region
sequence, plus actual surface allocation/reuse and worker execution. Treating
either Clear() or clipped ClearPenCanvas() as an unconditional whole-texture
clear would exceed the evidence.

## Dirty-rectangle projection and tip copy-bound selection

`conformance/fountain_rect_native.py` reconstructs both PenUtil
ConvertPenRectToNDC overloads and executes them unmodified. It verifies
1,084 projections byte-for-byte, then executes the tip's actual copy-bound
selection block for 1,626 combinations. Cases include identity, translation,
unequal scale, reflection, rotation, shear, singular matrices, empty/inverted
rectangles, off-canvas coordinates and seeded general matrices. Viewport
dimensions are positive in this suite.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_rect_native.py
```

Input RectF is `[left, top, right, bottom]`. The matrix is column-major; points
are multiplied as `(x,y,0,1)`. For each component the native helper first
rounds `column1 * y` to float32, then performs successive float32 fused
multiply-adds for columns 0, 2 and 3 with x, 0 and 1. It does **not** divide
by the resulting homogeneous w. UV coordinates are transformed x/viewport
width and y/viewport height, with NDC `2 * UV - 1`. No clamp, corner sorting
or additional vertical flip occurs in either utility.

The overload at PenCommon `0x537a0`, used by the fountain tip, transforms only
top-left and bottom-right. It returns both NDC and UV vectors in the order
`[x_top_left, x_bottom_right, y_bottom_right, y_top_left]`. NDC uses two
separate float32 adds (`UV + UV`, then `-1`). This is a two-corner conversion,
not a general rotated-rectangle bounding-box operation.

The overload at `0x53884` transforms all four corners and returns PointF
arrays ordered top-left, top-right, bottom-left, bottom-right. Its final NDC
step uses a fused multiply-add. The independent model preserves the native
operation order for both overloads. Its general-matrix cases also establish
that changing the homogeneous row does not introduce a perspective divide
in these utilities; this is not a claim about every later shader transform.

The caller block at FountainPen `0xa7714..0xa7758` selects the two-corner
conversion only when RectF::IsEmpty is false and the stored sub-bitmap count
is exactly 1. Empty/inverted dirty rectangles, zero sub-bitmaps and multiple
sub-bitmaps use the RT object's default copy bounds at offset 252. The test
supplies `[-1,1,-1,1]` for those defaults and verifies the selected vector.
It stops before upload, which is covered by the separate copy-geometry test.

Together these checks establish how a supplied dirty rectangle reaches copy
geometry. The enclosing frame's production of that rectangle, active clear
clip, notification timing and surface allocation still require integration
evidence. Rotation/shear coverage here documents the utility's literal
behavior, not a guarantee that all such transforms reach this tip branch in
the Android application.

## Touch presenter: bitmap copy precedes tip clear notification and drawing

Engine `0x10314c` is `TouchPresenter::OnPredictTouch`, identified by its
native log signature at `0x69a76`. The nonempty dirty-region branch at
`0x1036e4..0x1038ac` supplies the missing caller ordering around the tip
drawable. `conformance/fountain_presenter_native.py` executes this block,
including the complete bitmap-copy helper at `0x102a8c`, actual SPPaint
construction, native matrix operations and actual fountain tip Draw.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_presenter_native.py
```

The observed submission order is:

1. Save the canvas at presenter offset 104; copy from the bitmap at offset
   96 with SPPaint transfer mode **8**; restore that canvas. The target
   dimensions come from the bitmap at offset 88. The numeric transfer mode
   is verified both at its setter and in the paint passed to DrawBitmap.
   Its pixel equation is verified separately below.
2. Call the tip drawable's `SetCanvasCleared`. This immediately submits the
   previously verified `(RT object, virtual slot 80, member flag 1)` task
   for `ClearPenCanvas`. It does not synchronously clear pixels here.
3. Get the tip's canvas, save it, set the presenter's matrix, and pre-concat
   the inverse of the supplied frame matrix. Apply the page clip, using
   clip operation 1, if that rectangle is nonempty.
4. Query the main drawable's actual `IsTip` method. V16 returns true, so the
   presenter calls the tip's `Draw(RectF*)`. That native wrapper forwards
   null as the event argument to `Draw(MotionEvent const*, RectF*)`; the
   fountain implementation obtains its event from PointTipManager anyway.
5. Restore the tip canvas after drawing.

All **112 frames** pass across page clip present/absent, supplied event
present/absent, predictions present/absent and presenter byte 394 clear/set.
With that byte clear, the identity-transform fixture copies the supplied
`[10,20,90,100]` dirty region. Setting it makes the helper select the full
`[0,0,256,256]` bitmap. Every generated stamp and attribute-buffer byte
matches a separate direct native tip Draw, and the main drawable's 512
bytes remain unchanged.

The test enters an already-selected branch with a supplied dirty rectangle;
it does not execute the enclosing prediction/latency policy or dirty-region
calculation. Canvas virtual methods capture calls and the queue accepts
messages without running them. Thus this establishes native submission
order and connects the real tip geometry to it, but does not yet prove
worker execution order, actual surface clipping during the queued clear,
complete-frame pixel coverage, or absence of stale pixels in the final
Android framebuffer. The copy canvas, tip canvas and private intermediate
pen canvas must not be assumed to be the same object.

### Presenter copy replaces transparent pixels too

Graphics `SPBitmapDrawable::DrawBitmapRT` reads paint offset 44 at `0x9632c`
and resolves it with `SPGraphicsUtil::GetBlendingStateDescriptor` (`0xc7fe0`).
`fountain_blend_native.py` now executes this resolver for the presenter's
mode 8, followed by the actual GLES blend-state activation. Both color and
alpha resolve to `GL_FUNC_ADD`, source factor `GL_ONE`, destination factor
`GL_ZERO`, with all color channels enabled. The equation is therefore
`output = source`, including alpha; previous destination color does not
contribute.

Mode 8 bypasses the mode-16/17 advanced blend branch and mode-19 special
branch. The ordinary SPBitmapShader and ColorClamp variant do not discard
zero-alpha source texels. With no tint, each outputs sampled premultiplied
RGBA multiplied by paint alpha and vertex coverage. ColorClamp additionally
clamps RGB to the output alpha. Both agree on valid premultiplied input.

The shader exporter now reads both bitmap variants from the hash-pinned
Graphics library. The GPU suite executes their unmodified sources with the
verified mode-8 blending. Its **36 cases** cover both variants, full/cropped/
degenerate copy rectangles, scissor on/off and paint alpha 0/0.5/1. A
deterministic RGBA8 texture contains transparent, partially transparent and
opaque premultiplied pixels. All **589,824 channel comparisons** pass within
half a byte; pixels outside the copy/scissor intersection remain unchanged.
The copied regions include **11,280 transparent pixels** that erase the
previous destination instead of leaving it behind.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_blend_native.py
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_canvas_native.py
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_raster.py --output /tmp/fountain-raster.json
node conformance/fountain_gpu.mjs /tmp/fountain-raster.json
```

These pixel checks use nearest sampling, identity mapping and full vertex
coverage on an RGBA8 Chromium/SwiftShader target. They establish the copy
equation and transparent replacement, not native bitmap filtering, edge
geometry, color-space conversion or an end-to-end Android frame. The real
queued clear dispatch and the real copy shader are verified components;
their complete framebuffer integration remains unfinished.

### Presenter dirty-region reconstruction

`conformance/fountain_presenter_rect.py` now executes the complete Engine
helper at `0x103d24`, including the actual fountain tip's
`CalculateUpdateRect`, rather than supplying only a chosen copy rectangle.
Its independent presenter model matches **630 native results**, and **42
computed regions** are fed into the real copy/clear-notification/tip-draw
block from the preceding test.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_presenter_rect.py
```

The helper performs these operations in order:

1. With a null event, query `HasMainStrokeEventsForDraw`. For the fountain
   tip this tests whether PointTipManager has a nonzero retained point count.
   If false, return an empty rectangle immediately, leaving the output frame
   matrix untouched. A nonempty accumulated input does not override this
   early return; the harness checks this explicitly.
2. Obtain the tip's raw bounds with native `CalculateUpdateRect`, then
   expand each side by the pen width returned from pen virtual slot 24.
3. Transform all four corners through the presenter's column-major 3x3
   matrix at offset 120. For each homogeneous component, calculate
   `f32(f32(fma(column0, x, f32(column1 * y))) + column2)` and divide the
   resulting x/y by w. Take the four-corner bounding rectangle. Unlike the
   RT copy-quad utility documented above, this stage **does divide by w**.
4. Union that rectangle with the retained rectangle at presenter offset
   252, then with the caller's accumulated rectangle. Empty or inverted
   union operands contribute nothing.
5. Expand every side by eight pixels, floor left/top, and ceil right/bottom.
6. Intersect with the rectangle returned by the bitmap at presenter offset
   96. Return empty if the rectangles do not overlap.

The cases use actual evolving tip events, a width of eight, identity,
unequal scale, translation, quarter-turn rotation, shear and a projective
matrix with nonzero homogeneous terms. They cover absent/present event
arguments, empty/inverted/separated accumulated rectangles, contained and
partially overlapping viewport bounds, and completely offscreen bounds.
The 42 integrated cases use identity transforms and verify that both source
and destination copy rectangles equal the helper's computed result before
the native tip is drawn.

This model deliberately starts from native raw tip geometry; it independently
checks the presenter's rectangle processing, not a second implementation of
the pen's bounds calculation. Supplied retained/accumulated rectangles test
the helper's contract. Their complete lifetime through the surrounding
OnPredictTouch state machine, singular projective transforms, queue timing
and final pixel coverage remain outside this test. In particular, a padded
rectangle alone does not prove the previous tip is always fully removed.

### Rectangle retention through successive presenter frames

`conformance/fountain_presenter_state.py` extends the frame trace past canvas
restore, through presentation and rectangle-state updates. **96 successive
frame cases** match its independent state model: 42 draw a tip and 54 defer
drawing. This includes **32 complete calls** to native
`TouchPresenter::OnPredictTouch` at Engine `0x10314c`, on the no-new-event
route, without skipping its prologue, branch decisions or epilogue.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_presenter_state.py
```

The initial native union at `0x1031d0` combines the previous tip region at
offset 236 with the pending main region at offset 220. This becomes the
accumulated rectangle passed to the dirty-region helper. The tested state
transitions are:

| Presenter field | Visible redraw branch | Empty redraw branch |
| --- | --- | --- |
| 236: previous tip region | Replace with actual tip Draw bounds, transformed, rounded outward and expanded by two if nonempty | Empty |
| 252: retained region | Empty after drawing | Union the old retained region, pending main region and previous tip region |
| 188: presented region | Union the initial accumulated region and new tip region | Initial accumulated region |
| 204 / 220: consumed / pending main region | Copy 220 to 204 and empty 220 when an event exists or the tip queue is nonempty | Same condition |

The visible branch presents the bitmap at offset 88. The empty branch
presents the source bitmap at offset 96 and does not submit a bitmap copy
or tip Draw. Consequently, the region sent for presentation is distinct
from the clipped copy rectangle; applying the copy's viewport clipping to
all stored rectangles would change this state machine.

Fixtures alternate visible and offscreen viewport bounds, call each frame
twice without adding points, and finally empty PointTipManager. With no
event and no retained tip, two further presentations preserve the nonempty
pending main region at offset 220. Each frame checks all five rectangle
fields, the bitmap selected for presentation, the presentation rectangle,
and whether a copy was submitted. The draw branch uses the actual native
tip geometry and its actual post-draw rectangle processing.

The complete-method fixtures supply FountainPen metadata through a checked
String comparison boundary (the native query is for LaserPen), existing
canvas/bitmap interfaces, no pending main-event batch, initial timestamps
of -1, and disabled optional frame recording. The main controller's newly
committed regions are supplied explicitly. Presentation and control
interfaces record calls without Android effects. Thus the complete
no-new-event route is exercised for these states; incoming prediction
processing, other latency/timestamp branches, pending-event-batch drawing,
optional recording and actual displayed pixels remain unverified here.
