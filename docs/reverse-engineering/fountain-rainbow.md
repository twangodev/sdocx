# Fountain pen rainbow colors

> The reproduction commands in this note refer to experiment scripts removed
> during test cleanup, including `conformance/fountain_rainbow_model.py` and
> `conformance/fountain_rainbow_native.py`. Recover them from Git revision
> `40de721` in a separate checkout. Saved V16 geometry and
> `conformance/fountain_native.py` remain in this checkout. Rainbow rendering
> is not implemented. See
> [current validation](../../conformance/README.md#native-geometry-checks).

This describes the hash-pinned Samsung Notes 4.4.45.37 ARM64 FountainPen
library. `conformance/fountain_native.py` in this checkout covers saved V16
geometry only. The rainbow model and native comparisons named below are at
`40de721`. No APK code is distributed.

## Palette interpolation

The saved V17, live-tip and preview functions implement the same cyclic
RGB interpolation for a nonempty palette:

| Path | Shared-data pointer field | Palette selection | RGB interpolation |
| --- | ---: | ---: | ---: |
| Saved V17 | 72 | `0x7c164` | `0x7c5b0` |
| Live tip | 320 | `0xa5860` | `0xa5b54` |
| Preview | 64 | `0xa98d4` | `0xa9a78` |

For palette length `n`, the native code computes `step = float32(1.0 / n)`
using a double-precision division followed by a float conversion. It
divides the float phase by that step, truncates to a signed integer, and
caps the index at `n - 1`. The next index wraps from the last entry to zero.
The within-segment fraction is the float32 fused result
`phase - index * step`, divided by `step` in float32. Computing the index
as `phase * n` is not the same floating-point operation.

Each RGB channel is interpolated in its original 0–255 space using a
float32 fused multiply-add, then divided by 255. There is no intermediate
integer channel rounding, gamma conversion, or fraction clamp. Palette
alpha is ignored by this function. Overall pen alpha is still a separate
renderer input, as checked by the render-property conformance test.

The native function accepts phase 1 and slightly larger phases without
wrapping phase itself; the caller normally provides a remainder-normalized
phase. The model tests phase 1 and 1.125 as well as values immediately
adjacent to palette boundaries. Negative and nonfinite phases are outside
the model's supported palette-selection domain: native code has no lower
index clamp, so those inputs must not be assumed safe.

V17 explicitly handles an empty palette by converting the configured pen
color to RGB through native `PenUtil::ConvertToRGBA`. Tip and preview
selection functions have no empty-palette guard. The test executes the
V17 fallback; it does not deliberately execute out-of-bounds palette reads
in the other paths.

## Distance and offset

V17 drawLine's phase block (`0x7b46c..0x7b484`) computes:

```
period = float32(shared_data.rainbow_distance)
phase = float32(fmodf(float32(distance + current_offset), period) / period)
```

The integer period is at shared-data offset 244 and current offset at 248.
The tip endPenOrig block (`0xa4fa4..0xa4fc0`) uses the same arithmetic but
reads the snapshot offset at 236. Conformance cases put different values
in the two fields to catch accidental interchange. Native instruction
blocks execute through `fmodf`; that C-library operation is supplied by the
host with float32 input/output. Cases include integer periods above the
exact float32 integer range.

This proves the phase arithmetic at those two call sites. The saved redraw
test below extends the V17 evidence to complete redraw passes and distance
accumulation. Preview period selection and transfer of distance state into
an executing tip draw still need separate end-to-end validation.

## Native configuration and ownership

The shared-data initialization block in the FountainPen constructor
(`0x61194..0x611d8`) establishes:

| Field | Offset | Initial value |
| --- | ---: | --- |
| Snapshot distance offset | 236 | 0 |
| Rainbow enabled | 240 | false |
| Distance period | 244 | 1580 |
| Current distance offset | 248 | 0 |
| Gradient vector | 256 | five zero-valued packed colors |

The test executes this block, including its native vector allocation. It
does not run the entire FountainPen constructor or claim these are the
effective UI settings after application configuration.

Native public setters/getters at `0x622a0..0x623f4` read/write these fields.
Enable, distance and offset setters return true. Distance setters accept
zero and negative integers without validation; offset setters also accept
negative values. Setter success therefore does not establish a valid
rendering configuration. Invalid periods are only tested for round-trip
storage, not passed through the rendering arithmetic.

SetGradientColors copies the caller's vector into owned storage; it can
grow, shrink, clear and regrow the vector. GetGradientColors returns a
separate vector allocation using the AArch64 indirect return pointer in
`x8`. Tests mutate caller input after setting and returned data after
getting, and confirm neither mutation changes the stored palette.

## Evidence and remaining scope

The native test checks 1,854 direct interpolation cases, 5,457 cyclic
palette cases across the three paths, and 240 distance-phase cases against
the independent float32 model. Palette lengths are 1, 2, 3, 5, 7, 16, 31
and 256, with deterministic random colors/phases and one-ULP boundary
neighbors. Configuration defaults, setter/getter round trips, vector copy
ownership, and V17's empty-palette fallback are additional assertions.

These tests establish color arithmetic and configuration behavior in the
pinned library. They do not establish complete V17 geometry, GPU rainbow
compositing, effective application palettes, or behavior in older APKs.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_rainbow_native.py
```

## Saved V17 redraw and distance accumulation

`conformance/fountain_v17_native.py` executes the complete native
`FountainPenStrokeDrawableGLV17::redraw(ObjectStroke const*, RectF*, bool)`
at `0x7bba4`, including its drawLine, endPen, drawPoint, SmPath, tolerance
and width-smoothing calls. V17 here names the native drawable class; it
must not be interpreted as a file's advanced-settings version number.

The fixture reproduces the outer saved redraw's two-pass orchestration
from `0x7b8d0..0x7b9ec`: reset width smoothing, execute a fill pass,
complete the smoothing array, switch to replay, reset tolerance and the
original rainbow offset, and execute the output pass. The native fill
pass must submit no RT stamps. ObjectStroke channel access, endpoint
MotionEvent access, initial RT buffer/scale and GPU upload are host
boundaries. Native RTV6 AddPoint and Update execute, as described below.
The outer cache lookup, renderer messages and GPU execution are excluded.

Twenty-six synthetic paths reuse the established V16 reference inputs:
taps, stationary points, jitter, reversals, pressure/tilt changes, duplicate
and backward timestamps, pen-size extremes, and seeded paths. Plain V17
stamp positions/radii and sample-to-stamp mapping match those references
within `9.5367431640625e-7`. Each path is also replayed in rainbow mode with
three `(initial offset, period)` pairs: `(0, 1580)`, `(599.75, 600)` and
`(2.5, 3)`. All 78 rainbow redraws preserve the plain V17 geometry and
sample mapping exactly and submit 5,607 colored stamps.

The test checks 11,082 color selections against the independent palette
model, and every submitted RGB triple against the most recently computed
or retained color. Stamp opacity at this interface is 1; it is distinct
from the pen-alpha uniform used during compositing.

Distance verification reuses the independently modeled quadratic path
subdivision/length calculation from `fountain_v14_model.py` at `40de721`.
Observed
native moveTo/quadTo/lineTo arguments construct the model path; 1,464
native length results match it exactly. The test then tracks the current
offset independently and verifies native phase inputs and results, each
conditional offset addition, snapshot offset writes, and matching final
offsets after the fill and replay passes. This checks accumulation over
independently computed lengths, while native code still chooses which
curve commands and tolerated samples to emit.

The native update sites add the entire processed path length to offset
248, using float32 addition. They do not advance the offset only by the
last stamp distance or reduce the stored offset modulo the period. The
remainder operation is applied when selecting a color. The drawLine
snapshot copies the updated offset into field 236, which the tip phase
code reads. Paths rejected or bypassed before the update sites do not
perform those additions.

This establishes complete inner saved redraw behavior for the tested
variable-width stylus configuration at canonical scale. Fixed-width and
other tool configurations, rainbow framebuffer compositing, saved-cache
lookup/reuse, and live V17-to-tip handoff
remain outside this fixture's evidence.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_v17_native.py
```

## RTV6 attributes and upload

Native RTV6 AddPoint (`0xa1280`) uses inverse-scale components at RT
offsets 352/356. Like RTV5, it widens subpixel radii to at least half a pixel,
compensates opacity, and adds half a pixel for the antialiasing edge. Its
instance record extends the seven-float RTV5 record with three RGB floats:

```
x, y, expanded_radius_x, expanded_radius_y,
opacity, inner_edge, subpixel_alpha, red, green, blue
```

The stride is 40 bytes. RGB is copied as float32 without premultiplication
or clamping at this stage. Either zero inverse-scale component causes an
early return with no instance. Shared `attributes` and `native_attributes`
helpers in `fountain_raster.py` support RTV6; the existing RTV4/RTV5/tip
models retain their previous behavior. The shared GPU export now also
includes RTV6 cases and its native shaders.

`conformance/fountain_v17_raster.py` compares 3,807 native attribute cases
byte-for-byte against the shared model. Cases include half-pixel boundary
radii, anisotropic and zoom scales, variable opacity, black/white/random
RGB, and zero-scale exits. There are 3,615 emitted records and 192 exits.
The existing 3,765 RTV4/RTV5/tip cases also pass unchanged.

The saved redraw fixture now observes and executes actual RTV6 AddPoint
instead of replacing it with a stamp collector. Native vector allocation
and growth execute from an initially empty vector. Every generated byte
is compared with the modeled attributes for the recorded stamps, and the
width fill pass must leave the native vector empty.

The fixture then executes native SendDataToGPU and RTV6 Update (`0xa1aec`),
observing the final geometry-interface upload. Update(false) reads the
normal vector at RT offset 88; Update(true) reads the cache vector at 96
after native SetCacheBuffer. Both upload stream 1 with an instance count
equal to buffer bytes divided by 40, stored at RT offset 360. The mode byte
at 193 tracks the selected source. The inactive source is null in each
test, and a subsequent normal Update with only cached data available must
not upload. Both selected-source uploads must exactly match the generated
mesh bytes. This tests cache-buffer selection, not saved-cache lookup or
invalidation policy.

This closes the saved geometry-to-native-attribute-to-upload boundary for
these cases. RTV6 Draw, shader execution, blend/composite behavior and final
framebuffer parity remain separate work.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_v17_raster.py
```

## RTV6 Draw and rainbow compositing

`conformance/fountain_v17_draw.py` executes the complete native RTV6 Draw
(`0xa1b4c`) and its alpha-mask, color-mask, drawn-region and rainbow-composite
helpers. The real RT constructor initializes its matrix, inverse scale and
default copy rectangle. The three blend descriptors are built by executing
the Init block at `0xa0430..0xa0508`, through the actual descriptor setters;
the factory captures the descriptors and supplies observed blend objects.

The verified descriptor combinations, for both color and alpha, are:

| RT field | Function | Source factor | Destination factor | Purpose |
| ---: | --- | --- | --- | --- |
| 232 | ADD | ONE | ONE_MINUS_SRC_ALPHA | Premultiplied source-over |
| 240 | MAX | ONE | ONE | Maximum coverage |
| 248 | ADD | ONE | ZERO | Replacement |

The renderer enum mappings are also covered by the native GLES activation
tests in `fountain_blend_native.py`. This Draw fixture observes activation
order rather than executing GL calls.

With enhanced antialiasing disabled, Draw binds the destination, selects
source-over, uploads the composed projection matrix and pen RGBA, and draws
the RTV6 instances with the color shader. With enhanced antialiasing enabled,
it executes this sequence:

1. Select the alpha-mask tile matching the destination tile index and color
   mask tile zero.
2. Draw instances into the alpha mask with MAX blending and the alpha shader.
3. Draw the same instances into the color mask with source-over blending and
   the color shader.
4. Update copy geometry from the dirty region or default rectangle.
5. If the redraw flag is false, activate replacement blending. Otherwise,
   retain the source-over blend selected for the color-mask pass.
6. Composite onto the destination using alpha texture unit 0 and color
   texture unit 1, then unbind both textures and resolve the destination.

The native composite helper resolves the destination once, and the outer
Draw resolves it again. The command test preserves and checks both calls.
The source vector is selected by RT mode byte 193: normal vector field 88
or cached vector field 96. Null destination, null selected source, and empty
selected source return without render commands.

The hash-pinned shader sources clarify why the masks are separate. The
alpha shader at `0x41f2f` produces stamp coverage times pen alpha in its
single output channel. The color shader at `0x425ac` produces the stamp RGB
premultiplied by that same coverage and pen alpha. Source-over blending
accumulates the latter. The composite shader at `0x428cb` samples maximum
coverage from the alpha texture's red channel, discards nonpositive coverage,
divides accumulated color RGB by accumulated color alpha, and multiplies
that normalized RGB by maximum coverage. Its output alpha is maximum
coverage. It does not use maximum-per-channel RGB as the final color.

Pen alpha is already applied in the two mask shaders. Although the native
composite wrapper offers a `uInputColor` upload, that uniform is unused by
the inspected composite GLSL. Effective binding behavior after a real
driver optimizes the shader is not established by the supplied bindings.

Thirty-two command comparisons cross identity/transformed matrices,
enhanced AA, redraw, normal/cache sources, and destination tile indices 0
and 2. They check complete command order, typed native uniform wrappers,
three-instance draw counts, default six-vertex copy geometry, texture units,
and texture unbinding. Eight null/empty guard cases submit nothing. The
nonempty instance vector/count, GPU resources, shader-program bindings,
bitmap interfaces and pen RGBA are supplied fixtures. This test does not
run full RTV6 Init, dirty-region clipping cases, mask lifetime/clearing,
shader compilation or framebuffer pixels.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_v17_draw.py
```

## Mask creation, reuse and clearing

`conformance/fountain_v17_canvas.py` executes native RTV6 CreatePenCanvas
(`0xa105c`), its lowercase factory helper (`0xa0fc8`), ClearPenCanvas
(`0xa122c`), and per-buffer Clear (`0xa1ac0`). Resource factories and the
data-manager acquisition interface supply observed canvas/bitmap objects;
shader Init is an observed callback that sets the initialized flag. This
is a lifecycle/dispatch test, not a framebuffer or manager-cache test.

Every CreatePenCanvas invocation asks the data manager for the alpha mask
and clears the returned canvas with argument zero. This request uses the
queue obtained from the data manager, which can differ from the queue
argument passed to CreatePenCanvas. The test deliberately supplies different
queue pointers and checks each native use. The alpha request's optional
flags and layout arguments are all zero at the manager boundary.

The color mask at RT field 208 has a different policy. If its bitmap width
and height match the requested dimensions, the native function retains it
without clearing it. Otherwise it creates a new bitmap with memory-layout
argument 6 and target-layout argument 2, creates its canvas with mode 0,
releases the temporary bitmap reference, and clears the new canvas. It then
stores the new canvas and unreferences the previous color canvas. This
factory path uses the explicit queue argument. The enum values are checked
at the factory boundary; this fixture does not establish the resulting
physical GPU formats.

Shader initialization runs only when RT byte 192 is false. Width, height
and alpha-mask tile count are saved at fields 364, 368 and 372. Seven
transitions cover first creation, equal-dimension reuse, width-only and
height-only changes, returning to an earlier size, and zero dimensions.
Zero-dimension cases establish that this native caller forwards those
arguments; the host factory returning a fixture bitmap does not prove a
real graphics factory would accept them. Alpha-cache reuse in the fixture
is supplied by the manager boundary, not evidence of native cache policy.

ClearPenCanvas clears each nonnull mask independently. All four combinations
of mask presence are tested. In contrast, per-buffer Clear resets the redraw
flag, empties and detaches the normal vector, and resets copy geometry; it
does not call either canvas's clear operation. Complete Draw likewise issues
no mask clears. Its drawing operations can therefore accumulate into the
mask storage between explicit clears; this does not mean Draw leaves pixel
contents unchanged.

The shared GL `SetCanvasCleared` implementation at `0x62c34` supplies the
queued connection: it creates a native task targeting RT virtual slot 80.
The fixture captures the emitted task, verifies its `(RT, 80, 1)` target,
and executes the native task body at `0x63804`. Only then does native
ClearPenCanvas clear both masks. With a null manager queue, the GL method
submits no task. The test uses the shared GL base operation with the RTV6
target; it does not execute the entire V17 public-canvas attachment path
or establish Android worker ordering.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_v17_canvas.py
```

## GPU coverage and two-mask pixel comparisons

The shared `fountain_raster.py --output` exporter now verifies 7,572 native
attribute cases across RTV4/RTV5/RTV6/tip and extracts the RTV6 alpha, color
and rainbow-composite shaders from the hash-verified library. Shader text
is written only to the requested local fixture; no APK shaders are checked
into the repository. RTV6 case generation is shared with the standalone
attribute test.

`conformance/fountain_gpu.mjs` runs these unmodified shaders with Chromium
WebGL2/SwiftShader. Existing compile/link, stamp setup, readback, and
coverage-model helpers also serve RTV6. Isolated RTV6 stamps check RGB as
well as alpha against scalar circle/edge coverage and subpixel compensation,
including the GPU's reported rasterization grid. The combined run checks
3,768 isolated stamps, 15,433,728 coverage pixels and 23,187,456 RTV6 RGB
channels. Maximum normalized channel error is `0.0019608022`, approximately
half an RGBA8 byte. The tested backend reports four subpixel bits.

The rainbow pass checks three differently colored stamps at overlapping
positions, with a repeated stamp. Four radii and four pen alpha values
(including zero) are crossed with forward/reverse orders of the same stamp
multiset, replacement/source-over composite blending, and full/partial/empty
copy rectangles. This produces 192 composite cases.

The pixel checks are staged so their evidence is explicit:

- Isolated coverage and RGB use the independent scalar stamp model.
- Each source-over color-mask step is compared with the independently
  computed blend of an isolated source readback and the prior destination
  readback. The error bound accounts for quantization of source RGB,
  source alpha and destination storage. Maximum observed error is
  `1.2156863` bytes, within that bound.
- The alpha-mask red channel must exactly equal the per-pixel maximum of
  isolated stamp alpha readbacks. Reversing stamp order leaves this mask
  unchanged, while 2,623 accumulated RGB channels change in these fixtures.
- Each composite is compared with the scalar normalization
  `color.rgb / color.a * maximum`, followed by replacement or source-over
  blending. This checks 3,145,728 channels with maximum error 0.5 bytes.
  Another 2,228,224 mask-channel comparisons check the preceding passes.
  Pixels outside the copy rectangle or with zero maximum coverage retain
  the background, including under replacement blending.

All previous V4/V5 blending, tip direct/intermediate blending, tip-distance
and presenter-copy GPU checks pass in the same run. The older fixture
format without RTV6 shaders remains accepted; the rainbow checks run only
when those shaders are present.

The original experiment used RGBA8 masks with nearest sampling. The current
check uses native R8 alpha/RGBA8 color masks with linear clamp sampling,
64×64 dimensions, and identity texture coordinates. This establishes the extracted shader
equations and their composition under this SwiftShader configuration. It
does not establish the Android driver's precision, multi-tile sampling, color space, or framebuffer parity
for a complete app-rendered document. Those remain required evidence for
native-level rendering parity.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_raster.py --output /tmp/fountain-v6-raster.json
node conformance/fountain_gpu.mjs /tmp/fountain-v6-raster.json
```

## Native texture descriptor and sampler allocation

`conformance/fountain_texture_native.py` narrows the texture-format gap above.
It executes Graphics `SPSubBitmapRT::createTextureRT` (`0x8d9ac`) with
positive dimensions, captures the descriptor passed to the renderer, and
feeds that exact descriptor into Renderer `createTextureInternalsSampler`
(`0x5c75c`). Execution stops at `0x5d3e4`, after allocation commands and before
result-object construction and parameter-map ownership. GL calls are
observed interfaces; no graphics driver executes in this test.

For the color mask's `(TextureMemoryLayout=6, TextureTargetLayout=2)` pair,
the native allocation is `GL_TEXTURE_2D`, internal `GL_RGBA8`, external
`GL_RGBA`, type `GL_UNSIGNED_BYTE`. The `(0,0)` pair maps to `GL_R8`,
`GL_RED`, `GL_UNSIGNED_BYTE`. The latter is a candidate alpha-mask format:
RTV6 requests `(0,0)` from the manager, but the complete manager cache and
bitmap-to-subbitmap creation chain has not yet been executed in this test.

The subbitmap factory explicitly supplies parameter pairs `(4,1),(5,1)`.
The sampler backend resolves these to `GL_LINEAR` for both magnification
and minification, with `GL_CLAMP_TO_EDGE` on both axes, unpack alignment 1,
and minimum/maximum LOD zero. A manually supplied two-node libc++ map checks
those exact overrides; an empty map produces identical native defaults.
The test checks 12 combinations: three dimensions, two format pairs, and
explicit/default filter parameters. No texel payload is supplied.

This shows why the preceding nearest-filter RGBA8 shader experiment is not
yet a complete native texture reproduction. Subsequent GPU checks need
linear sampling and the confirmed alpha allocation path, especially for
fractional texture coordinates and tile edges. The test does not establish
sampler-backend selection on a real device, the legacy backend, resource
cache reuse, later parameter changes, or Android framebuffer precision.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_texture_native.py
```

## Native alpha-mask manager forwarding and cache lifetime

`conformance/fountain_v17_manager.py` replaces the supplied manager in the
RTV6 canvas fixture with native `PenGLDataManager::GetPenCanvas`
(`0x486d8`) and `CreatePenCanvas` (`0x49260`). Native map lookup, insertion,
erasure, same-size selection, `SetMsgQueue`, and forwarding to the bitmap
factory execute. String conversion, managed-object registration, graphics
factories and canvas interfaces remain supplied boundaries. This extends
the previous lifecycle test; its Python manager cache was only a fixture,
not evidence for the application's cache policy.

The alpha request reaches `SPGraphicsFactory::CreateBitmap` unchanged with
memory/target layouts `(0,0)` and a tag ending in `;0;RED_UINT8;`. The native
manager builds that tag from the fountain pen name and appends the red-byte
format marker. The private color request remains `(6,2)` with a separate
RTV6 tag. Thus the manager-forwarding gap in the preceding section is
closed for this path. The full bitmap/subbitmap constructor and queued
resource-creation chain still needs verification before treating the
format-to-GL test as end-to-end texture allocation evidence.

Eight dimension transitions check the complete observed operation order,
canvas identities, dimensions, forwarded format pairs and native tree size:

- An identical-size request reuses both surfaces and clears only alpha.
- A changed-size request releases the old alpha canvas through the graphics
  factory, erases its cache entry and creates a replacement. There is one
  entry for this key, not one entry per historical size. Returning to an
  earlier size allocates a new alpha canvas.
- The color surface is independently replaced, cleared and the old color
  canvas unreferenced. It is not stored in this manager map.
- Before creating its canvas, the manager invokes bitmap slot 144, then
  slot 24 on the returned interface with argument zero. Its semantic name
  is not yet established; the test records the command without guessing.
- A second RTV6 instance using the same manager, pen key and dimensions
  receives the same alpha canvas, clears it, and allocates its own color
  canvas. This verifies sharing and operation order, not concurrent use or
  ownership safety after a different instance resizes the shared entry.

The eight transitions allocate six mask pairs. The second-instance check
adds one private color allocation. Manager queue replacement, other pen
keys, prediction-key fallback reuse, full reference counting, worker
execution and real framebuffer contents remain outside this fixture.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_v17_manager.py
```

## Bitmap factory, native vertical tiling and texture allocation

`conformance/fountain_bitmap_native.py` executes the remaining mode-0
bitmap-construction path: Graphics `SPGraphicsFactory::CreateBitmap`
(`0xc04ac`), `SPTextureBitmap` constructor (`0x8f140`), `SPSubBitmapRT`
constructor (`0x8d89c`) and `createTextureRT` (`0x8d9ac`). The bitmap and
subbitmap `SetTag` methods execute too. Required vtable relocations are
explicitly patched; device capabilities, host allocation and final renderer
texture objects remain supplied interfaces.

For the mode-0 calls observed in both fountain masks, the factory selects
`SPSubBitmapRT`. Its constructor calls texture creation synchronously in
this path; no supplied queue executor or reconstructed subbitmap loop is
needed. Every resulting descriptor is then fed to the same native sampler
allocation prefix used by `fountain_texture_native.py`.

The experiment checks 60 combinations of supplied maximum texture sizes
64/128, widths 1/63/97, heights 1/64/65/128/129, and the two fountain format
pairs. It verifies 90 subbitmaps and their GL allocation commands:

- Subbitmaps form vertical bands of at most `GetMaxTextureSize()` rows. Each
  has rectangle `(0, top, width, bottom)` and a sequential tile index.
- The last band uses its actual remaining height, without padding to the
  tile limit. Renderer buffer metadata is `(0, top, full bitmap width)`.
- The full width passes through unchanged. In particular, width 97 with a
  supplied limit of 64 still requests a 97-wide texture. This proves that
  this constructor does not horizontally tile or reject that input; it
  does **not** prove that a real driver accepts an oversized texture.
- Layouts `(0,0)` survive the constructors and allocate `R8/RED/UNSIGNED_BYTE`;
  `(6,2)` allocate `RGBA8/RGBA/UNSIGNED_BYTE`. Both preserve the explicit
  linear min/mag filtering, clamp-to-edge and zero LOD range.
- Native bitmap and subbitmap tag propagation reaches each observed texture
  object. Tile pointers, rectangles, indices, vector length, texture handles
  and dimensions are checked independently of the factory capture.
- Five factory guards return null before texture creation: zero/negative
  width, zero/negative height, and null queue.

Together with the manager forwarding test, this closes the constructor
format-preservation gap for the fountain's mode-0 mask allocation. The
manager and allocation fixtures are separate native executions joined by
checked factory arguments; they are not a single app frame. Non-mode-0
queued subbitmaps, live device capability/backend selection, allocation
failure, destruction, uploaded pixel payloads and Android GPU results remain
unverified. The original rainbow GPU experiment used RGBA8/nearest masks. The following
checks apply these native formats and linear sampling; a complete native
tiled-frame comparison remains outstanding.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_bitmap_native.py
```

## GPU validation with native formats and linear sampling

The rainbow section of `fountain_gpu.mjs` now allocates R8 for its maximum
alpha mask and RGBA8 for its source-over color mask. Both use linear min/mag
sampling and clamp-to-edge. All 192 overlap/order/composite cases still
pass, with the same mask/composite error measurements reported above. R8
readback also verifies the per-pixel maximum against independently rendered
stamp coverage. Existing tip and bitmap-copy checks remain unchanged.

Another 30 cases use the unmodified native rainbow-composite shaders with
synthetic premultiplied color textures and independent scalar bilinear
sampling. Five viewport configurations exercise texel centers, fractional
coordinates, magnification, minification and clipped viewports. Alpha
texture heights 64, 17 and 1 exercise full-height, short-band and single-row
sampling, paired with 64×64 color textures. Both replacement and source-over
blending are checked. The scalar model converts normalized coordinates to
texel-center coordinates, clamps each neighbor independently, interpolates
four values, and then applies the native RGB normalization/composite formula.
It does not read sampled GPU values to construct expected pixels.

These cases check 491,520 channels and 36,534 pixel visits requiring edge
clamping. Maximum error is `0.5002564141` bytes; the assertion allows 0.51
bytes for final RGBA8 rounding and floating-point arithmetic. Pixels outside
the viewport must remain exactly unchanged. Synthetic color alpha is at
least 128, each RGB channel is no greater than alpha, and maximum coverage
is at most 128; low-alpha/discard behavior also remains covered by the
preceding stamp-driven tests. These are SwiftShader measurements, not a
portable precision bound for every GLES driver.

The varied-height fixtures verify shader sampling semantics. They do not
claim to reproduce the app's multi-tile matrices or its selection of color
versus alpha tiles in a complete frame. A native tiled draw linked to pixel
readback and an Android rendering comparison remain required.

## Native tile projection and dirty-copy geometry

`conformance/fountain_v17_tiles.py` executes Graphics
`SPSubBitmapRT::GetProjectionMatrix` (`0x8df24`) and installs that actual
method as the destination subbitmap's projection interface in complete
RTV6 `Draw` calls. The supplied subbitmap rectangle is the explicit fixture
boundary; this is not a native tile scheduler or real framebuffer test.

For rectangle `(left, top, right, bottom)`, the projection has x/y scales
`2/(right-left)` and `2/(bottom-top)`, and translations
`-(left+right)/(right-left)` and `-(top+bottom)/(bottom-top)`. Float32
conversion, intermediate sums/differences and divisions are modeled in
native order. The z coefficient has bits `0xb8000100` and z translation
is negative zero. Seven matrices match byte-for-byte, including translated
rectangles, a one-pixel rectangle, large coordinates and a final 17-row band.

The complete Draw tests verify that the stored/rendered matrix is
`destinationProjection * suppliedTransform`, using the native y-product,
x/z/w FMA accumulation order. Both alpha and color passes receive this same
matrix. For destination tiles 0, 1 and 2, the draw selects alpha tile 0, 1
and 2 respectively, while selecting color tile 0 every time. Identity and
scale/shear/translation transforms are crossed with direct/enhanced and
redraw/replacement modes, producing 24 complete Draw cases. In particular,
the final short band's projection is used for both mask draws; the test does
not replace it with the first band's projection for the color pass.

The test also reuses all 542 independent rectangle-projection fixtures from
`fountain_rect_native.py` with alpha tile counts 0, 1 and 3. The resulting
1,626 actual `updateDrawnRegion` calls establish:

- A nonempty dirty rectangle with exactly one alpha tile uses the four-corner
  `ConvertPenRectToNDC` overload, the supplied transform and full mask
  dimensions. It does not use the composed destination projection here.
- The upload orders projected corners as triangles `(0,1,2),(2,1,3)`.
  Rotation and shear therefore retain four transformed corners rather than
  substituting an axis-aligned bounding box.
- An empty dirty rectangle or any tile count other than one uploads the
  default full-tile quad. Complete enhanced multi-tile Draw calls verify
  this same fallback even with a nonempty dirty rectangle.

These checks establish native command/coordinate behavior. The repeated
color-tile-0 selection, accumulation between tiled calls, framebuffer sizes
and linear resampling must still be combined in a GPU frame experiment;
separate passing allocation, shader and matrix checks do not prove that
complete frame or Android driver parity.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_v17_tiles.py
```

## Captured tiled draws replayed on the GPU

`conformance/fountain_v17_frame.py` now connects actual RTV6 `AddPoint`,
`Update` and complete `Draw` execution. Seven colored stamps span both tile
boundaries and the short final band of a 64×145 fixture with tile limit 64.
The initial empty stamp vector is supplied explicitly. All seven native
40-byte records match the independent attribute model byte-for-byte, and
native `Update` uploads exactly those 280 bytes with stream index 1 and
instance count 7.

The capture runs tile order `0,1,2,1,0` in both redraw/source-over and
replacement modes. Each of the ten command traces is checked for the native
projection, two seven-instance draws, matching alpha tile/color tile 0,
blend order, full copy geometry and absence of mask clears. The RT/resource
setup and tile submission order are explicit fixture inputs. The capture
does not claim to execute a native frame scheduler. Its optional JSON export
contains attributes, commands, dimensions and library hashes, with no APK
shader source.

`fountain_gpu.mjs` accepts that export as an optional second input file. It
replays the captured commands using the unmodified native shaders, real
instanced draws and interleaved 40-byte instance records. Three R8 alpha
surfaces and three destination surfaces have heights 64,64,17. The color
surface remains the selected 64-row RGBA8 tile 0 throughout each scenario.
All masks start clear once per scenario; commands do not clear them between
tiles or repeated calls. The initial replay treated deactivation as a no-op. It now applies the
verified depth/stencil invalidation and SDK-dependent flush operations
described below, while retaining single-sample FBOs.

After each draw, a scalar bilinear/composite calculation compares the
result with the destination's pre-draw pixels and actual mask readbacks.
Across ten frames, all 139,776 destination channels pass with maximum error
0.5 bytes (0.51-byte threshold). There are 4,792 pre-draw color-pixel visits
with retained nonzero alpha, confirming the accumulation path is exercised.
Unknown command kinds fail replay; providing a frame without RTV6 shaders
also fails instead of silently skipping it. Existing isolated shader,
overlap, fractional-sampling, tip and bitmap-copy checks pass in the same run.

This is a connected native-command/GPU-composite experiment, with a staged
oracle: native geometry/upload/projection are independently checked, while
this frame's composite expectation uses GPU mask readbacks. It is not an
independent scalar prediction of every mask pixel in the tiled frame, and
it does not establish native scheduler order, resource lifetime across
strokes, Android driver precision or a document rendered by the app.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_v17_frame.py --output /tmp/fountain-v17-frame.json
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_raster.py --output /tmp/fountain-v6-raster.json
node conformance/fountain_gpu.mjs /tmp/fountain-v6-raster.json /tmp/fountain-v17-frame.json
```

## Native framebuffer activation and deactivation

`conformance/fountain_framebuffer_native.py` executes the complete method
chain from Graphics `SPSubBitmapRT::ActivateFrameBufferRT` (`0x8dfc4`) and
`DeactivateFrameBufferRT` (`0x8e094`) through Renderer
`GLES::OffscreenRenderTarget` (`0x54d04`, `0x54d10`) and
`RenderTargetBase` (`0x55eb4`, `0x55f70`). Existing target fields, the SDK
version, successful GL status and current binding are supplied; GL calls
and requested `Unref` operations are recorded.

Twelve activation cases confirm binding the stored framebuffer, checking
its completeness and setting `glViewport(0,0,width,height)` from target
fields. Width/height include a 17-row final tile and a one-pixel target.
The dimensions do not come from the full bitmap or its tile origin.

The 160 deactivation cases cross SDKs 27/28/29/30/35, default/offscreen
bindings, discard values 0 through 7, and the requested-Unref flag:

- Discard value 1 selects depth, 2 selects stencil, and 3 selects both.
  Other tested values select neither; even the empty list is passed to
  `glInvalidateFramebuffer`.
- The currently bound framebuffer determines attachment enums: default
  uses `GL_DEPTH/GL_STENCIL`, nonzero uses
  `GL_DEPTH_ATTACHMENT/GL_STENCIL_ATTACHMENT`.
- No color attachment is invalidated, and no replacement framebuffer is
  bound. Deactivation preserves the current binding.
- The base implementation flushes only for SDK 29. The offscreen wrapper
  additionally flushes for SDK >=28. Thus SDK 29 emits two flushes,
  SDK 28/30/35 one, and SDK 27 none.
- A requested Unref follows these operations. With no attached target,
  deactivation skips the GL work but still honors that request. The test
  observes the request without asserting reference-count lifetime behavior.

The tiled frame export now specifies SDK 30 as an explicit fixture input.
GPU replay performs native discard translation and flush conditions rather
than treating its `resolve` trace entries as no-ops. All ten frames retain
the same pixel result with 40 invalidations and 40 flushes; all existing
shader checks pass in the same run. The native evidence establishes that
these trace entries are deactivation operations, not a color-buffer resolve.

Framebuffer construction, multisample attachment selection, completeness
failure handling, real device SDK/capabilities and reference-counted target
lifetime remain unverified. In particular, successful activation of a
supplied target does not establish how the app constructed that target.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_framebuffer_native.py
```

## Native framebuffer construction and sample count

`conformance/fountain_framebuffer_create.py` extends the activation test to
execute Graphics `SPSubBitmapRT::CreateFrameBufferRT` (`0x8ddf0`),
`SPFrameBuffer` construction (`0x81d20`), both renderer factory entry points,
and the full GLES offscreen constructor (`0x5475c`), `CreateFbo` (`0x547ec`)
and successful `AttachColorToFbo` (`0x54900`). Texture interfaces, GL names
and completeness results, allocation and managed-object registration remain
supplied boundaries. Required vtable relocations are explicit.

Eight successful chains cross four texture dimensions with the caller's
Unref flag. Each runs construction, reuse, activation and deactivation:

- `glGenFramebuffers(1, ...)` obtains a framebuffer name. The native target
  stores dimensions returned by the texture's width/height interfaces.
  Deliberately different subbitmap rectangles confirm that the redundant
  `SPFrameBuffer(width,height,texture)` arguments do not determine these
  stored target dimensions.
- Native attachment unbinds `GL_TEXTURE_2D`, binds the new framebuffer and
  calls `glFramebufferTexture2D(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0,
  GL_TEXTURE_2D, textureName, 0)`, then checks completeness.
- This executed path makes no multisample texture/renderbuffer allocation
  or multisample attachment call. Combined with the verified ordinary
  R8/RGBA8 texture allocation, it confirms the replay's single-sample color
  attachment for these fountain masks. It does not describe every other
  framebuffer or rendering path in Samsung Notes.
- A second `CreateFrameBufferRT` call reuses the same wrapper and increments
  its actual native reference count from one to two; there are no GL calls.
  The subsequent native activation/deactivation uses the just-constructed
  target and produces the viewport/discard/flush commands verified above.
- A GL-generated name of zero skips attachment, retains the constructor's
  framebuffer sentinel `0xffffffff` and zero dimensions, and leaves the
  completion byte clear. The wrapper/target still exist and registration
  still occurs. A missing texture produces no wrapper or GL calls and
  honors a requested caller Unref.

These checks close the successful construction and sample-count gap for
this mode-0 offscreen mask path. Non-complete attachment status after a
nonzero allocation, texture/target destruction, reattachment after context
loss, depth/stencil attachment creation and physical Android GPU behavior
remain outside the experiment. Caller Unref operations are observed rather
than executing object destruction.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_framebuffer_create.py
```

## Independent tiled mask and complete-frame prediction

The captured-frame GPU test now also maintains an independent scalar state
for every alpha mask, the shared color mask and each destination tile. GPU
readback is used only for comparison; it never updates that model state.
The existing mask-readback composite check remains as a separate diagnostic.

For each captured instance, the model transforms the expanded stamp quad,
snaps its edges to the GPU's reported rasterizer grid, interpolates local
UV coordinates and evaluates the shared scalar circle/AA/opacity equation.
It predicts MAX accumulation for R8 and source-over accumulation for RGBA8,
rounding to byte storage after each instance. These modeled masks persist
across the supplied tile order, including revisits and color-tile reuse.
The coverage equation is shared with the isolated-stamp scalar checks.

After each captured composite, the model bilinearly samples its own masks,
normalizes RGB by its predicted color alpha, applies MAX coverage and the
captured replacement/source-over mode, then stores the predicted destination
bytes for subsequent draws. Initial destination bytes come from the supplied
clear color, not from a framebuffer readback.

For the ten captured draws:

- 198,784 mask-channel checks pass. R8 coverage must match exactly;
  accumulated RGBA8 color/alpha may differ by at most one byte, which is also
  the maximum observed difference.
- 139,776 independently predicted destination channels pass with maximum
  difference one byte and a one-byte assertion threshold.
- The separate readback-based composite check still passes within 0.5 bytes.
  All previous isolated, overlap, sampling, tip and bitmap-copy checks pass.

The one-byte agreement is specific to these fixtures and SwiftShader; it
is not a proof of bit-identical accumulation or a driver-independent error
bound. This model explicitly requires positive axis-aligned affine
projections, as used by the current identity-transform frame capture, and
rejects a fixture sample on the shader's undefined circle boundary. Rotated
or sheared whole-frame rasterization, native scheduling, real-document
frames and Android-device comparison remain required evidence. Native
rotation/shear matrix and dirty-region command tests remain separate.

## Affine whole-frame rasterization

The native frame capture now crosses five transforms with both composite
modes and the same five tile visits: identity, a translated quarter-turn,
an oblique rotation with sine/cosine 0.8/0.6, shear and horizontal reflection.
Transform inputs are rounded to float32 before both native submission and
the independent matrix comparison. All 50 complete Draw captures preserve
the checked upload, tile selection, native matrix composition and blend order.

The scalar mask model now projects all four expanded stamp vertices and
uses barycentric interpolation on the two actual strip triangles. The same
interpolation helper serves the isolated-stamp checks, avoiding a separate
rotation/shear implementation. It handles either triangle orientation,
including reflection, and snaps projected vertices to the reported raster
subpixel grid. The former positive axis-aligned restriction is removed;
non-affine homogeneous transforms remain explicitly rejected. Undefined
circle-boundary samples also remain rejected rather than assigned an
invented expected value.

All 50 GPU replays pass without loosening the numerical limits:

- 993,920 mask-channel comparisons: exact R8 MAX coverage and at most one
  byte difference in accumulated RGBA8 channels.
- 698,880 independent destination-channel comparisons: at most one byte.
- The separate mask-readback composite check remains within 0.5 bytes,
  aside from floating-point reporting noise below `1e-12` bytes.
- 22,432 pre-draw color-pixel visits retain nonzero alpha, and replay issues
  200 depth/stencil invalidations and 200 SDK-30 flushes.

The complete existing shader suite also passes with the shared interpolation
helper. These results extend the independent frame experiment to affine
rotation, shear and reflection; they do not establish perspective behavior,
native scheduler order, real-document parity or Android driver agreement.

## Native RTV6 geometry initialization consumed by replay

`conformance/fountain_v17_geometry.py` executes the geometry portion of
RTV6 `Init`, from `0xa0268` through the boundary at `0xa0430`, before blend
and shader setup. Native `VertexDescriptor::addAttribute` and attribute-size
calculation execute against the resolved renderer size table. Existing
geometry/upload observation helpers are reused from the tip initializer.

The captured stamp geometry has two streams and primitive enum 5. Stream 0
contains four vertices with four floats each: local position `(x,y)` and
UV `(u,v)`, in order `(-1,-1,0,1), (-1,1,0,0), (1,-1,1,1), (1,1,1,0)`.
Its stride is 16 bytes and attribute divisor zero. Stream 1 contains
attributes with sizes `4,1,2,3`, byte offsets `0,16,20,28`, total stride
40 bytes, native type enum 3 and divisor one for every attribute. This
matches position/radii, opacity, inner-edge/subpixel-alpha and RGB.

The copy geometry has one two-float attribute, stride 8, divisor zero and
primitive enum 4. Its initial six vertices match the previously verified
full-tile triangle pair. All four combinations of existing/missing stamp
and copy resources execute the native reuse guards: existing pointers are
preserved, only missing resources are created, and only their expected
quad/copy data is uploaded.

The frame exporter now includes these captured quad/layout values. Tiled
GPU replay consumes them for the vertex buffer, stride, offsets and
instance divisors instead of repeating independent layout literals. All 50
frame replays and the existing shader suite pass with unchanged numerical
results. Native geometry capture is a separate initialization execution
joined to the frame by its verified layout; full Init shader/cache setup
and the concrete native graphics-object implementation remain outside this
geometry-prefix test.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_v17_geometry.py
```

## Complete RTV6 initialization with shader cache hits

`conformance/fountain_v17_init.py` extends the geometry fixture through the
complete native RTV6 `Init()` function. It supplies shader-manager key/tree
lookup, cached shader objects, lock boundaries, managed-object registration
and GPU factories. Descriptor construction, guard branches, native cache
reference increments, field assignments and readiness writes execute.

The initial call creates two geometry objects and three blend objects
(source-over, MAX and replacement). It acquires **four** shaders, in order:
`FountainPenStrokeAlphaShaderV6`, `FountainPenStrokeCompositeShaderV6`,
`FountainPenStrokeShaderV6`, and `FountainPenRainbowCompositeShaderV6`.
The normal composite shader is initialized even though the enhanced Draw
path previously traced uses the rainbow composite. Native fields
256/264/272/280 receive the corresponding shader handles, each supplied
cache node's reference count becomes one, and byte 192 becomes one.

Three repeated Init calls preserve every geometry/blend/shader handle and
perform no new factory calls, uploads, cache lookup or reference increment.
They still call managed-object registration. Four selective-missing-field
cases each reacquire only the cleared shader field, increment only that
cache node and restore the original handle without rebuilding other
resources. Finally, a second constructed RTV6 gets its own geometry and
blends, shares all four shaders, increments their native cache counters and
sets its readiness byte. Lock/unlock order is checked around each lookup.

This proves complete Init control flow on supplied cache hits; it does not
prove the shader manager's key comparator/tree, real mutex scheduling,
cache-miss shader construction, compilation/linking failures, teardown or
context restoration. Selectively clearing a field is a test input, not a
claim about native resource-release policy.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_v17_init.py
```

## RTV6 shader construction on cache misses

`conformance/fountain_v17_shaders.py` executes complete native Init for all
16 hit/miss combinations of its four shaders. It shares cache/program
observation interfaces with the older tip shader fixture. Native constructors
at `0xa3970`, `0xa3a70`, `0xa3b70` and `0xa3c70` execute and choose these
sources and uniform requests:

| Shader | Vertex / fragment addresses | Uniform requests, in order |
| --- | --- | --- |
| Alpha V6 | `0x41cfd` / `0x41f2f` | `ProjectionMatrix`, `inputColor` |
| Composite V6 | `0x4216f` / `0x42225` | `uSrcTexture`, `uInputColor` |
| Stroke V6 | `0x4232d` / `0x425ac` | `ProjectionMatrix`, `inputColor` |
| Rainbow composite V6 | `0x42815` / `0x428cb` | `uSrcOverTexture`, `uMaxTexture`, `uInputColor` |

Only missing entries cause program construction and cache insertion. The
fixture checks program labels, uniform order, each shader's stored program
handle, RTV6's shader pointers, native cache reference counts and readiness.
A repeated Init performs no additional construction, binding request,
insertion or reference increment. The existing eight tip hit/miss cases
also pass after extracting the shared observation interfaces.

This closes the cache-miss constructor gap in the preceding Init fixture.
Cache lookup/insertion and typed parameter objects are still supplied host
interfaces; actual cache-tree behavior, GLES compilation/linking failures,
shader destruction and context restoration remain unverified here. Source
selection agrees with the separately tested GPU programs; this fixture
does not itself compile shaders or establish Android framebuffer parity.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_v17_shaders.py
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_shader_creation.py
```

## RTV6 release and local geometry restoration

`conformance/fountain_v17_lifetime.py` extends the native constructor fixture
through `Release()` at `0xa0978` and the four native shader destructors.
Two constructed RTV6 instances share the four shaders, with native cache
reference counts of two. Releasing the first decrements those counts to one
without destroying a shader. Releasing the second invokes each shader's
destructor, deletes its allocation and requests cache erasure, in Init's
shader order. Inside each destructor the program's disposal interface runs
first, followed by parameter-binding disposal in reverse field order.

Both releases independently dispose of stamp geometry, copy geometry,
source-over blend, MAX blend and replacement blend, in that order. Release
zeros all nine geometry/blend/shader fields and the readiness byte, then
unregisters the managed object. It preserves the canvas pointers at offsets
200 and 208. Repeating Release causes no resource disposal, but still calls
Unregister. Reinitializing a released RTV6 recreates all nine resources and
restores readiness and cache reference counts of one.

`RestoreGLObject()` at `0xa0ea8` has a narrower responsibility than complete
context recovery: it uploads the four base-quad vertices to the existing
stamp geometry and six copy vertices to the existing copy geometry. The
copy vector at offset 376 is ordered **left, right, top, bottom**, as checked
with both the default vector and an asymmetric rectangle. Neither restore
case creates programs, geometry or blends, changes the nine handles, nor
uploads instance/stroke data. These observations concern this method, not
the application's broader context-loss orchestration.

Cache erasure, GPU/program/binding disposal, allocation deletion and managed
registration are supplied observation interfaces. The erase interface
supplies a fresh zeroed node for a later insertion. Actual cache-tree
deallocation, GPU lifetime, canvas destruction, exception paths and the
missing/mismatched-cache diagnostic branches are outside this fixture.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_v17_lifetime.py
```

## V17 outer saved-stroke redraw policy

`conformance/fountain_v17_redraw_policy.py` executes the complete outer
`RedrawPen(ObjectStroke const*, RectF*)` at `0x7b67c`. Inner redraw results,
stroke/cache getters, callback objects, inverse-scale input and queued-task
factories are supplied observation boundaries. Native branch selection,
pass ordering, saved-offset resets, rectangle transfer and cache-vector
copying execute. This complements the separate native inner-redraw fixture;
it does not yet join both into one end-to-end execution.

The 64 combinations vary stroke-data existence, point-data existence,
matching inverse scale, dirty flag, original runtime handle and nonzero tip
length. Native behavior is:

- Lookup uses the original runtime handle if it is not `-1`, otherwise the
  current runtime handle. A cache hit requires both data objects, matching
  inverse scale and a clear dirty flag.
- A hit supplies the cached vector to `PenCacheRedrawCallback`, returns the
  cached pen rectangle and performs no inner redraw or rainbow-offset read.
- Rejected existing point data is unreferenced before regeneration.
- Regeneration reads the saved rainbow offset and initial tolerance. With
  nonzero tip length it resets smoothing for the tool, runs mode 1, completes
  the smoothing array, switches to mode 2 and draws again. Zero tip length
  selects mode 0 and a single drawing pass. Before the drawing pass it resets
  tolerance and reads the saved rainbow offset again, independently of the
  first pass's accumulated distance.
- Successful regeneration with an existing stroke-data object makes a
  separate vector copy, stores inverse scale and pen bounds, installs point
  data, clears the dirty flag, unreferences the new point-data object and
  resets the original handle to `-1`. When an original handle was used, dirty
  clearing targets its found object. No stroke-data object means no cache
  insertion. The fixture checks four copied float values and distinct vector
  ownership, rather than supplying the copy operation.

Two failed-redraw cases verify no cache insertion or dirty clearing, callback
cleanup and smoothing-mode reset. A failed fill-pass result does not suppress
the later drawing pass; its result determines success. Successful paths reset
drawable fields 272 and 348; failed paths preserve their supplied values.
Five guard cases verify null source/rectangle and zero points set error 7,
while missing RT/canvas simply returns false. Zero points is checked after
four property/clear task-factory calls; other guard cases queue none.

Actual cache implementation, callback lifetime/worker execution, fixed-width
and non-stylus geometry remain outside this policy fixture. The supplied
matching-scale boolean is not proof of the native comparison's tolerance.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_v17_redraw_policy.py
```

## Saved V17 fixed-width and shape geometry

`conformance/fountain_v17_native.py` now accepts supplied raw tool codes and
fixed-width settings, with an explicit `shape` argument representing
`ObjectStroke::IsShape()`. It still executes native redraw, drawLine, endPen,
drawPoint, smoothing and RTV6 AddPoint/Update. The fixture writes the native
fixed-width flag at data offset 57 and fixed width at offset 60; it does not
prove the public API or file-to-settings assignment of those fields.

The native drawPoint branch at `0x7c264` replaces the interpolated radius
when fixed width is enabled. Ordinary strokes use `fixed_width * 0.5`;
shape strokes use `fixed_width * 0.25`. Both subsequently use a float32
minimum radius of `0.1`. This same selection feeds normal and rainbow
stamps. The shape factor is not an opacity or diameter adjustment later in
the GPU pipeline: it directly changes the radius passed to RTV6 AddPoint.

Fixed-width shape behavior also changes sampling. In drawLine,
`0x7b108..0x7b134` checks both IsShape and the fixed-width flag and branches
past the tolerance rejection and short-distance alternating-update logic
at `0x7b158..0x7b194`. Thus replacing only the emitted radii would miss a
native shape-specific geometry difference. Ordinary fixed-width strokes
still follow those filtering branches.

`conformance/fountain_v17_widths.py` exercises all 26 synthetic reference
paths with raw tool codes 0–4, fixed widths 0.05 and 7.25, and both shape
states. It checks the radius rule on every emitted stamp, ordinary-stroke
center/sample-boundary equality against variable-width rendering, and
absence of visits to drawLine's tolerance branch for fixed-width shapes.
A rainbow run for each path/tool verifies the same fixed-shape geometry.
Every run retains the existing independent path-length and color checks,
byte-for-byte RTV6 attribute construction, and normal/cache upload checks.
These are supplied tool codes; no new device-type interpretation is inferred.

The inner redraw disassembly additionally shows codes 1 and 3 substituting
pressure 0.5 for the initial and intermediate samples, with code 1 selecting
a 50-unit distance threshold versus 5 for the other tested codes. Complete
independent models of each tool's variable-width behavior and the public
fixed-width settings path remain separate work. Smoothing orchestration is
still supplied as two passes, consistent with the nonzero-tip-length outer
path; this fixture does not cover the zero-tip-length mode-0 path.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_v17_widths.py
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_v17_native.py
```

## Public fixed-width API and Engine settings transfer

`conformance/fountain_fixed_width_api.py` executes FountainPen's public
`SetFixedWidthEnabled` (`0x6217c`), `SetFixedWidth` (`0x621b4`), getters and
their IPenMorphable thunks. The primary object reads its data pointer at
offset 144; the subobject at primary+96 reads it at subobject+48. Setters
write only data byte 57 and float 60 respectively and return true. They do
not clamp or couple enablement to the width. Fourteen cases combine both
enable states with positive/negative zero, small/ordinary positive values,
a negative value and both infinities. Exact getter bits and every untouched
neighboring byte are checked. These are accessor semantics, not claims that
all these values are valid rendering inputs.

The same fixture executes Engine's `0xd30f4..0xd3150` block inside
`StrokeShapeObjectFactory::CreateStrokeShapeObject`. It supplies the selected
stroke and pen-interface lookup, then executes the native getter-to-setter
transfer through the real FountainPen morphable thunks. The block reads the
stroke's fixed-width enablement and then its width, and sets both even when
disabled. All fourteen resulting data buffers exactly match direct public
setter calls. The enclosing shape-conversion flow, JNI and document parsing
remain outside this bounded execution.

The saved V17 geometry fixture now calls those public setters on a supplied
pen shell sharing the drawable's data, replacing direct flag/width writes.
It also preserves explicit zero: only an absent (`None`) width falls back to
the ordinary pen size. The width suite includes zero alongside 0.05 and 7.25
to verify that enabled zero reaches the native radius minimum rather than
silently selecting the ordinary size. This supersedes the preceding section's
direct-field setup limitation, but does not establish end-to-end file loading.

Decompiled `SpenPen.java` methods `setFixedWidth` and `setFixedWidthEnabled`
check that the native pen handle exists and delegate to JNI, throwing on a
false result. They show no Java-side width clamp; JNI's own validation has
not been executed by this fixture.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_fixed_width_api.py
```

## Zero-tip-length saved redraw and smoothing mode 0

The outer policy fixture establishes that a zero tip length selects smoothing
mode 0 and one saved redraw, without reset/fill/complete operations. The
geometry fixture now reproduces that orchestration with `smooth=False`,
alongside its existing nonzero-tip-length two-pass default. Both execute the
same actual V17 geometry methods and RTV6 upload paths.

`conformance/fountain_v17_unsmoothed.py` first executes the native common
`WidthSmoothManager::getSmoothedWidthFromList` at `0x58f18` in mode 0.
Its mode selector at manager offset 44 bypasses list insertion/lookup and
returns the input width in `s2`. Across 54 combinations of widths, timestamps
and call-source values, the result is bit-identical, including signed zero,
and all 64 supplied manager bytes remain unchanged. This bypass applies to
the width-history stage; V17's earlier pressure, direction, interpolation
and width-limit calculations still execute.

For every synthetic reference path and raw tool code 0–4, the fixture
compares mode-0 normal/rainbow variable-width rendering and ordinary/shape
fixed-width rendering with the corresponding two-pass results. All runs
retain native stamp emission, independent path/color checks, and exact
RTV6 attribute and normal/cache buffer-upload verification. The comparison
checks center positions and sample boundaries separately from radii, and
requires the corpus to include a variable-width difference between modes.
The 520 mode-0 redraws emit 36,850 stamps. Seventy-six path/tool combinations
change variable radii compared with the two-pass output; centers and sample
boundaries remain equal in all cases. Fixed-width ordinary and shape stamps
match their two-pass counterparts exactly, while mode-0 rainbow preserves
the normal-color geometry.

This exercises the previously missing zero-tip-length geometry branch. The
tip-length choice and surrounding callback/queue orchestration remain
supplied to the inner geometry fixture; combining the outer and inner
fixtures into one execution is still outstanding. Mode 0 does not imply
that a stroke is fixed-width or that its curve geometry is unsmoothed.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_v17_unsmoothed.py
```

## Joined outer and inner V17 saved redraw

`conformance/fountain_v17_saved_pipeline.py` closes the outer/inner execution
gap for regeneration without a stroke-data cache entry. The shared geometry
fixture exposes a `redraw_saved` orchestration method; its existing version
supplies the independently checked pass sequence. The joined version invokes
the complete native outer `RedrawPen` and lets it call the native inner
`redraw`, native width-history reset/fill/complete/draw operations, curve
geometry and RTV6 AddPoint in the same emulator instance.

The harness observes entry to each inner pass without replacing it. It checks
native mode order `[1, 2]` or `[0]`, zero emitted stamps from the fill pass,
the saved rainbow offset at every pass entry, successful completion, callback
create/rectangle/destroy order, and final smoothing mode 0. It also checks
the outer method resets drawable fields 272 and 348. After the outer call,
the shared fixture verifies the emitted attribute bytes independently and
executes the existing native normal/cache handoff and RTV6 Update checks.

The comparisons cover all 26 synthetic reference paths with four settings:
variable-width stylus with and without smoothing, a smoothed fixed-width
shape using raw tool 1, and a mode-0 fixed-width shape using raw tool 3. Each
runs normal and rainbow color, with saved offset 17.25 and period 600.
Complete result dictionaries are compared against the supplied-orchestration
fixture, including stamps, sample boundaries, accumulated distance,
independent color/path check counts and mesh size.

ObjectStroke channel/endpoint access, canvas/inverse scale, absent cache
lookup, callback ownership and queued-task factories remain host interfaces.
The real callback does not perform the upload in this fixture: handoff and
Update are explicitly invoked after the native outer call by the shared
verification path. This is joined native CPU geometry/orchestration evidence,
not end-to-end worker execution or Android framebuffer parity. Cache-hit
execution and native cache insertion remain separate policy/lifetime work.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_v17_saved_pipeline.py
```

## V17 inverse scale and cache comparison at zoom boundaries

`conformance/fountain_v17_scale_cache.py` executes native `getInverseScale()`
at `0x7af08` with a supplied canvas matrix. For a finite matrix with nonzero
first two column lengths, inverse X/Y are the reciprocals of those lengths.
The independent model follows native float32 multiplication, fused sums,
square root and division order. The method writes inverse X/Y at offsets
328/332 and stamp spacing `0.82 * min(inverse_x, inverse_y)` at offset 96.
Translation does not contribute to these lengths; reflection and rotation
still produce nonnegative scale magnitudes. Nonuniform scale and shear are
therefore relevant to spacing and cache reuse.

The 104 matrix cases include identity, translated nonuniform scale,
quarter-turn rotation, reflection/shear and deterministic finite general
matrices. All match the float32 model exactly. Repeating a byte-identical
matrix skips recomputation, verified by preserving a deliberately changed
spacing field. A missing canvas returns the stored inverse scale and also
preserves spacing. These checks do not cover zero-length columns, NaNs or
infinite matrix values.

The real common `PenStrokePointData::SetInverseScale` at `0x52484` stores
the two floats at offsets 52/56 and sets validity byte 48. Without that byte,
`hasSameInverseScale` at `0x52448` returns false even for matching zero values.
With it, both absolute float32 differences must be **strictly less than**
the float32 constant `1e-6`; equality is a miss. This is an absolute tolerance,
not a relative comparison, so rounding at the stored scale's magnitude
affects the boundary.

Seventy stored/requested scale pairs cover five magnitudes and changes around
the threshold on both axes. Each is checked against an independent model,
then passed through the complete outer RedrawPen policy with its prior
supplied comparison replaced by the actual native comparator. Matching pairs
reuse the cached buffer; mismatches run both regeneration passes. Other
cache, stroke and queue interfaces remain supplied as in the policy fixture.
This supersedes its unknown scale-comparison-tolerance limitation.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_v17_scale_cache.py
```

## Joined saved redraw at nonunit and anisotropic scale

The shared V17 geometry fixture now accepts a finite positive inverse-scale
pair. Its supplied-orchestration path sets spacing to the independently
verified float32 `0.82 * min(scale)` rule and supplies the same scale to RTV6.
The joined outer path now executes actual `getInverseScale()` using a
supplied diagonal canvas matrix instead of returning scale from a host
callback. It checks the resulting stored scale and spacing. This removes
the joined fixture's earlier supplied-inverse-scale boundary; canvas matrix
delivery and the other stated cache/callback/task boundaries remain.

`conformance/fountain_v17_zoom.py` runs all 26 synthetic paths at inverse
scales `(0.25, 0.25)`, `(0.5, 2)`, `(2, 0.5)` and `(4, 4)`, in normal and
rainbow color. These power-of-two values make the supplied diagonal matrix
and its recovered inverse scale exact. Each normal-color joined result is
compared with independently supplied pass orchestration and spacing;
rainbow must preserve its geometry. Both paths retain independent curve
length checks and byte-for-byte RTV6 attribute/upload verification using
the actual scale pair, including separate X/Y expansion and subpixel alpha.

Swapping `(0.5, 2)` to `(2, 0.5)` must preserve CPU stamp geometry and sample
boundaries because their minimum-scale spacing is identical. Their GPU
instance records are separately verified against the anisotropic attribute
model. Corpus stamp totals must increase as inverse scale decreases; this
checks that zoom changes sampling density rather than only the final quad
expansion. The unit-scale joined suite is also rerun after replacing the
inverse-scale callback with native matrix processing.

These tests join matrix processing, outer regeneration, curve/stamp output
and native instance uploads. They do not add GPU framebuffer comparisons at
these scales or establish native scheduling across multiple strokes. General
rotation/shear matrix math remains covered by the separate scale fixture;
this joined fixture uses diagonal matrices.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_v17_zoom.py
```

## Joined regeneration, cache copy and cache-hit redraw

`conformance/fountain_v17_saved_cache.py` extends the joined geometry fixture
with an existing stroke-data object whose point data is initially absent.
The outer native RedrawPen regenerates real V17 geometry, allocates and
copies its vertex vector into new point data, sets inverse scale through
the real common accessor, stores bounds and requests dirty-flag clearing.
The cache object/getters, point-data constructor and ownership operations
remain supplied interfaces; the vector allocation/copy instructions execute.

The copied vector must have a distinct vector object and backing allocation,
with bytes identical to the actual generated mesh. Its validity byte and
stored inverse scales are checked. The fixture overwrites the original
vertex allocation with `0xa5`, then invokes the complete native outer redraw
again. Native matrix reuse and the real scale comparator select the cached
buffer; the cached callback receives its pointer and original bounds.
No inner pass or normal return callback executes, and the cached bytes remain
unchanged. The original vector is restored only afterward for the shared
independent attribute/upload checks.

Finally, the fixture explicitly passes the separately allocated cached
vector through native SetCacheBuffer and RTV6 Update(true), checking its
uploaded count and bytes. This verifies the copied cache mesh is consumable
by the native upload path; the callback and worker do not schedule that
handoff themselves in this fixture.

The 26 paths each run unit-scale smoothed normal color, anisotropic smoothed
rainbow, and anisotropic mode-0 rainbow, with complete generated results
compared to the no-cache joined fixture. Real cache reference ownership,
deferred-release worker execution, eviction and multithreaded access remain
outside this check. The test neither frees the source allocation nor claims
the native application poisons it; overwriting is an independence probe.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_v17_saved_cache.py
```

## Native cached geometry ownership and destruction

`conformance/fountain_cache_lifetime.py` executes common PenStrokeData and
PenStrokePointData constructors, actual SetPointData/GetPointData, base
RefCount operations and deleting/non-deleting destructors. Locks, allocation
and deallocation remain host interfaces. Required vtable relocations and
destructor aliases are explicit; native atomic reference instructions run
in the single-threaded emulator.

The sixteen chains cover every occupied/null combination of point-data
vector fields 16/24/32/40, paired with its complementary replacement:

- Both data objects start with a native reference count of one. Point-data
  vectors, scale validity, scales and radius start zeroed.
- SetPointData retains the new point data, giving it count two, stores the
  pointer at stroke-data offset 16 and clears byte 64. Dropping the creator's
  reference leaves the cache's one reference.
- GetPointData retains the returned point data. Replacing it in the cache
  releases the cache reference, but the outstanding getter reference keeps
  the old object and all its vectors alive.
- Releasing that final getter reference destroys old vector storage and
  vector objects in field order, then the reference counter and point-data
  allocation. Clearing the cache with SetPointData(null) likewise releases
  its last reference to the replacement. GetPointData then returns null.
- Final stroke-data release destroys its critical-section allocation,
  reference counter and object. Lock/unlock pairs around the six tested
  guarded operations are checked in order.

The reference-type setup is shared with the joined saved-cache fixture,
which now executes the actual point-data constructor instead of zeroing a
supplied shell. Its cache ownership callbacks are still supplied; the full
ownership chain above is a separate execution. Real mutex contention,
allocator behavior, deferred worker release and cache eviction remain
unverified by these tests.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_cache_lifetime.py
```

## Native cache ownership joined to saved geometry and upload

The joined saved-cache fixture now uses a real PenStrokeData constructor,
GetPointData/SetPointData, GetPenRect/SetPenRect and native Ref/Unref instead
of their earlier host ownership callbacks. Point-data construction and the
native vector copy remain part of the same regeneration execution. Mutex
interfaces and lookup of the stroke-data object are still supplied.

Each regeneration finishes with exactly one point-data reference owned by
the cache. Its following native cache-hit lookup raises the count to two.
The deferred task-factory boundary is checked to request Unref on that same
point-data object, with a zero member-pointer adjustment. After explicit
native upload of the copied buffer, the fixture calls that Unref operation,
which returns the count to one without destroying the buffer.

Clearing the real cache through SetPointData(null) then destroys the copied
vertex storage, vector, point-data counter and point-data object, in order.
Releasing the stroke-data object destroys its lock allocation, counter and
object. This is verified for every existing regeneration/cache-hit pair,
including the earlier poisoned-source independence checks. Allocation frees
are observed host calls; native destructor and reference branches execute.

This supersedes the supplied cache ownership/getter/rectangle boundaries in
the preceding joined-cache sections. Callback ownership, actual queued-task
allocation/execution, worker scheduling and mutex concurrency remain
unverified. In particular, the fixture chooses when to run the observed
deferred release after upload; it does not prove Android's scheduling order.

## Native deferred cache-release tasks

`conformance/fountain_cache_task.py` executes the actual FountainPen task
factory at `0x663ec`, RefCount member-task body at `0x66b44`, and deleting
task destructor at `0x66b40`. The supplied queue observes its enqueue call
and returns acceptance or rejection. Native task allocation stores five
64-bit fields: vtable `0xd2048`, zero, point-data pointer, Unref function
pointer, and zero member-pointer adjustment. Construction does not retain
the point-data object; it relies on the reference already obtained by the
cache getter.

Twelve cases combine queue acceptance/rejection, initial reference counts
one/two, and zero/one/four populated vector slots. Accepted task execution
calls native Unref, exercising both retained and final-destruction paths;
subsequent task destruction deletes the task itself. Rejection deletes the
task immediately without executing Unref or otherwise reducing the target's
reference count. The fixture explicitly cleans up remaining references.
This establishes this factory's behavior, not the application's higher-level
queue-failure recovery policy or the prevalence of rejection.

The joined saved-cache fixture now uses this real task factory with an
accepting observed queue, checks every captured task field, and executes
the native body and destructor after upload. This replaces its former
factory skip and directly supplied Unref call. Native geometry, cache copy,
retained cache hit, task submission, instance upload, task execution and
final cache destruction are consequently exercised in one chain. Other
property/render task factories and render callbacks remain supplied; the
test controls dispatch timing and does not emulate Android worker scheduling.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_cache_task.py
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_v17_saved_cache.py
```

## Native cache-redraw callback submission

`conformance/fountain_cache_callback.py` executes the actual common
PenCacheRedrawCallback constructor, buffer/bounds/background setters and
non-deleting destructor at `0x46580`. The destructor obtains the canvas queue
and submits two independently allocated messages in this order:

1. A Member1 task targeting native PenDrawableRT::SetCacheBuffer with the
   supplied vector pointer and zero member-pointer adjustment.
2. A PenGLRenderMsg carrying the RT, canvas, background bitmap, pen bounds,
   converted canvas matrix and cached-render flag byte 136 set to one.

Both enqueue calls occur even if the first is rejected. Each rejected message
is independently deleted; rejection of the buffer message does not cancel
the render submission. Merely constructing/submitting either message does
not change the RT's cached-buffer field. This describes callback behavior,
not higher-level queue failure recovery.

Forty-eight cases combine all enqueue acceptance pairs, three canvas
matrices, optional background and optional cached buffer. They check every
stored message field, native Matrix3-to-Matrix4 placement, exact bounds,
deletion decisions and submission order. For an accepted buffer message,
the native task body at `0x46af4` executes SetCacheBuffer and RTV6 Update(true)
uploads the three distinct supplied V6 instance records byte-for-byte. A
null vector produces no upload. The records come from the shared attribute
model; this fixture isolates handoff rather than regenerating strokes.

Canvas virtual methods, queue acceptance and GPU upload are supplied
interfaces. Render-message contents are checked but its render body is not
dispatched here. The joined saved-cache fixture still supplies this callback;
joining callback submission with its geometry and deferred-release chain
remains outstanding.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_cache_callback.py
```

## Cache callback joined to regenerated geometry

The saved-cache chain now executes the real PenCacheRedrawCallback
constructor, setters and destructor. It supplies the callback's canvas queue
access through the same observed queue used by the deferred-release factory.
Common callback relocations and destructor aliases are shared with the
standalone callback fixture.

For each real cache hit, the captured submission order must be exactly the
native SetCacheBuffer task, cached PenGLRenderMsg, and RefCount::Unref task.
The first task must carry the separately copied vertex vector; the render
message must target that RT, carry the cached bounds and set cached-render
mode. The deferred release must target the retained point-data object.
These are produced by native callback/factory code, not assembled messages.

The joined fixture executes the captured native buffer task instead of
directly calling SetCacheBuffer. RTV6 Update then uploads the copied vector,
and the native buffer-task destructor runs. The render message's fields are
checked and its destructor runs, but its rendering body is still omitted.
The captured deferred task subsequently releases the borrowed reference,
followed by the existing native cache-clear/destruction checks.

This removes the supplied cache-redraw callback and direct buffer-handoff
boundaries from the joined chain. The normal regeneration return callback,
property/clear task factories, GPU interfaces and scheduling remain supplied.
The derived saved-render fixture below adds cached PenGLRenderMsg execution
to this same chain.

## Cached render-message execution

`conformance/fountain_v17_cache_render.py` separately closes the render-body
boundary with the existing cache-callback, RTV6 drawing and task-interface
helpers. The real callback produces both messages; the fixture dispatches
the captured SetCacheBuffer task followed by PenGLRenderMsg at common
`0x4676c`. RTV6 Update, Draw and Clear execute through native virtual slots.

The 96 cases cover zero, one and two destination tiles; identity and vertical
translation matrices; direct and two-mask rendering; both redraw flags; and
bounds inside either tile, spanning both or outside the destination. The
native message uploads the cached vector once even when no tile intersects,
clips and restores every enumerated tile, and draws only intersecting tiles.
The two-mask path selects the corresponding alpha tile and color tile zero.
Shader selection and replacement blending follow the existing RTV6 policy.

Clear runs after tile traversal, empties and detaches the pending normal
vector, resets the redraw flag, and preserves the cached vector and its
bytes. Cached mode remains set after the task. Bounds transformation,
intersection and traversal execute natively; expected tile membership is
computed independently for the supplied translations.

This fixture supplies three V6 records, destination geometry, canvas access
and observed GPU interfaces. It does not run an Android queue or compare
Android framebuffer pixels.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_v17_cache_render.py
```

## Saved regeneration through cached drawing and release

`conformance/fountain_v17_saved_render.py` extends the saved-cache chain with
the real captured render-message body. Native regeneration, separately owned
cache copying, cache-hit lookup, callback submission, buffer-task execution,
RTV6 Update/Draw/Clear, deferred Unref and final cache destruction now run in
one emulator instance. The existing draw and render-surface setup helpers
are shared rather than duplicated.

The 156 chains cover all 26 synthetic saved paths under unit-scale normal,
anisotropic rainbow with smoothing, and anisotropic rainbow without
smoothing, each with direct and two-mask drawing. They generate 18,534
stamps and draw 180 intersecting destination tiles. Geometry and color
results must equal the saved pipeline without caching; uploaded bytes must
equal the independently checked, separately copied cache vector. The real
message transforms cached bounds using its captured canvas matrix. Its tile
selection, shader order and instance counts are checked before native
deferred reference release and final backing/vector/cache destruction.

The RT shell, draw properties, GPU resources, destination projection and
canvas clipping remain supplied. Two 64-by-64 destination tiles deliberately
exclude portions of some synthetic paths. This checks command selection and
ownership, not framebuffer output or whole-stroke visibility. Task dispatch
is explicit; normal-regeneration callbacks and property/clear task factories
remain supplied as in the saved-cache fixture. Android scheduling, actual
GPU execution and device framebuffer comparison remain separate boundaries.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_v17_saved_render.py
```

## Normal return callback and buffer ownership joined

`conformance/fountain_v17_return_render.py` replaces the supplied normal
PenReturnCallback in the joined saved renderer with the APK implementation.
The constructor at common `0x503fc` zeros its owned-buffer fields. GetBuffer
at `0x50640` lazily allocates a 24-byte vector, distinct from the initially
supplied RT vector. The shared saved harness now follows this returned
vector through geometry and independent attribute verification.

The native destructor at `0x50424` detaches the RT's pending buffer pointers
and submits a Member2 SendDataToGPU task, a noncached PenGLRenderMsg, and
a vector-deletion task. The fixture captures those actual tasks, checks the
normal and cached bounds/matrices agree, and executes normal upload and
rendering before deletion. RTV6 Clear empties the normal vector; the deletion
task frees its backing allocation and vector object. Task destructors are
also executed and their deletions observed.

The copied cache must remain byte-identical after this destruction. The
normal backing allocation and vector are poisoned, and a native memory-read
hook rejects any subsequent access to those freed ranges. The queued cache
handoff, rendering, deferred reference release and final cache destruction
then execute using the same shared renderer resources. The 156 normal/cache
pairs cover 18,534 generated stamps and 180 intersecting tiles per pass,
with both direct and two-mask rendering. Geometry results still match the
saved pipeline with supplied callbacks and no caching.

This removes the normal-return callback boundary from this derived fixture.
Queue acceptance and FIFO dispatch are supplied; mutexes, GPU interfaces,
draw properties, property/clear task factories and canvas operations retain
the earlier boundaries. Poisoning validates native reads under this bounded
execution, not Android allocator reuse or concurrent worker scheduling.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_v17_return_render.py
```

## Saved property and mask-clear task chain

`conformance/fountain_v17_saved_properties.py` enables the native factories
at FountainPen `0x6525c`, `0x652f4`, `0x62cc4` and `0x64b30` in the joined
normal/cache renderer. Each redraw queues these five tasks, in order:

1. SetRedrawState(true).
2. SetPenData(width, ARGB).
3. SetEnhancedAntiAlias(settings byte 52).
4. Virtual RT slot 80, RTV6 ClearPenCanvas.
5. SetRect with the extended bounds.

The complete captured queue must place those five tasks before normal
handoff/render/vector deletion, then another five before cached
handoff/render/deferred Unref. The fixture executes the actual task bodies,
property setters, virtual clear dispatch and task destructors. It poisons
the supplied render properties first so native tasks must replace them.
RTV6 ClearPenCanvas calls both mask canvases' Clear(0) interfaces before
each draw. Draw itself does not issue those clear calls.

All 156 chains check native ARGB conversion, integer width storage,
antialiasing, redraw state, bounds assignment, and the resulting direct or
two-mask shader color uniforms. They retain the native geometry, callback,
copied cache, normal-vector poison/read guard, and reference cleanup checks
from the return-render fixture. The shader resources and mask canvas Clear
interfaces remain observed boundaries; no actual mask pixels are mutated
by this Python fixture.

This removes the supplied draw-property and clear-task boundaries from
this derived chain. The manager cleanup factory at `0x66324`, mutexes,
canvas/GPU resources and explicit FIFO dispatch remain supplied or skipped.
Device framebuffer comparison and worker scheduling remain unverified.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_v17_saved_properties.py
```

## Stroke-manager ownership and queued release

`conformance/fountain_cache_manager.py` executes the common manager constructor
at `0x50c44`, GetStrokeData at `0x50df8`, ReleaseStrokeData at `0x50fd0`, and
destructor at `0x50cc0`. Its ordered tree insertion, lookup, balancing and
erase code runs natively. The fixture checks parent links, signed key order,
minimum-node pointer, entry count and stored object identity after mutations.
Mutex and allocator operations remain observed host interfaces.

A cache miss constructs a PenStrokeData with reference count one, increments
it to two, and inserts it into the manager's tree. The returned pointer thus
has a borrowed reference in addition to the manager's ownership. A hit
increments the existing object's count. ReleaseStrokeData searches by handle
and decrements a count above one without removing the entry. At count one,
it destroys the stroke data and erases the node. Missing handles, a disabled
manager, or a missing manager mutex cause no release. Disabled lookup returns
null without creating an entry.

This explains the task at the end of outer RedrawPen: FountainPen factory
`0x66324` captures the manager, ReleaseStrokeData member and signed handle.
The fixture executes that actual factory, its Member1 body at `0x6698c`,
and task destructor at `0x66988`. The first queued release after a new lookup
normally leaves the manager's cache reference intact.

Six insertion/removal sequences cover nine signed handles, including int32
extremes, and 162 counted queued releases. Separate missing/disabled release
checks are also exercised. Real point-data payloads are attached to each
stroke so final releases must destroy backing storage, vectors, point and
stroke references, mutexes and tree nodes. Manager destruction releases its
ownership of every entry; an externally borrowed stroke survives until its
borrower releases it. Tree nodes are removed in either case.

This is standalone manager coverage. The joined saved renderer still supplies
manager lookup and skips its final release task; integration must account for
the additional manager/borrower references instead of applying the earlier
standalone stroke ownership assumptions. Singleton initialization, document
switching and concurrent worker behavior remain unverified here.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_cache_manager.py
```

## Manager ownership joined to the complete saved task sequence

`conformance/fountain_v17_managed_render.py` joins the actual manager into
the property-enabled saved renderer. A fresh native manager starts empty;
outer RedrawPen calls its native GetStrokeData, which constructs/inserts the
stroke owner on the initial miss. The next outer call obtains the same owner
through the native tree lookup. Singleton access supplies this manager's
pointer, but lookup, allocation, tree operations and ownership execute in
the APK code.

Both redraws are submitted before the supplied worker queue drains. The
observed stroke-owner reference counts are therefore two after the first
lookup and three after the second. The complete queue has eighteen tasks:
the previously verified normal and cached sequences each end with the real
manager ReleaseStrokeData task. Their member target, manager pointer and
handle 73 are checked. Dispatching the normal release leaves count two;
dispatching the cached release after point-data Unref leaves count one.
Neither release destroys the manager's retained cache.

After drawing and both releases, native manager destruction replaces the
fixture's direct point-clear/stroke-Unref cleanup. The expected deletion
order is cached backing storage, vector, point reference counter, point
object, stroke mutex, stroke reference counter, stroke object, tree node,
and manager mutex. The resulting tree must be empty. The normal-buffer
poison/read guard remains active through these operations.

The 156 cases retain independent geometry/attribute checks, native normal
and cached callbacks, property and clear tasks, tile selection, shader
uniform checks, rendering, vector deletion and deferred reference cleanup.
No saved outer task factory in this path remains skipped. GPU/canvas
resources, ObjectStroke channel access, dirty-flag operations, mutexes,
singleton access and explicit FIFO dispatch remain supplied boundaries.
This still does not prove Android scheduling or framebuffer pixel parity.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_v17_managed_render.py
```

## Document registration and whole-manager invalidation

`conformance/fountain_cache_documents.py` executes native SetDocument at
common `0x51148`, including its std::function callable copies, document-map
insertion/erase, and the stored whole-cache-clear callable at `0x519fc`.
The fixture initializes the global listener container's empty fields and
observes WNote registration/deregistration; it does not execute WNote event
delivery or the application's static initialization.

Registering a new document handle retains existing stroke-cache entries and
registers the listener with the supplied WNote. Passing another nonnull note
for an existing handle neither replaces its stored note nor registers again.
Passing null for an existing handle invokes the whole-cache-clear callable
before deregistering that handle's note and erasing its document-map node.
The callable releases the manager's reference to every cached stroke and
empties the entire stroke tree, even if other documents remain registered.
This happens regardless of the manager's enabled flag. A missing manager
mutex causes SetDocument to return without changing registration state.

Eight native ownership chains cover both registration orders, enabled and
disabled managers, and retained versus externally borrowed stroke data.
Real point-data vectors verify that unborrowed cached geometry is destroyed,
while borrowed stroke and point storage survive until direct Unref. After
the tree is cleared, ReleaseStrokeData(handle) cannot find that surviving
borrower and does not decrement it. This is evidence that application queue
draining and document-close ordering matter; it is not evidence of an
application leak, because the real close/worker ordering has not been traced.

One edge case also follows the native instructions: a null note for an
unknown handle inserts a null document entry. Repeating that call finds the
entry, clears the stroke cache and removes the entry, without WNote
registration calls. These observed edge cases describe the tested APK;
they are not recommendations for a production API.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_cache_documents.py
```

## Application document-close ordering audit

The caller of PenStrokeDataManager::SetDocument in the extracted APK is
`libSPenComposer.so` (SHA-256
`52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f`).
An import scan of the extracted native libraries found this caller in
Composer, not Engine. The decompiled Java bridge is
`com/samsung/android/sdk/composer/SpenComposerImpl.java`.

The verified native call chain is:

| Location in Composer | Operation |
| --- | --- |
| `0x30c31c` | ComposerGlue::Native_setDocument; null Java note selects a null WNote. |
| `0x30c430` | Calls Composer::SetDocument at `0x3889fc`. |
| `0x388abc` | Nonnull branch calls manager SetDocument with the Composer address as the long key and the WNote pointer as its value. |
| `0x388d7c` | Null branch calls PageManager::SetDocument(null). |
| `0x388d88` | Null branch calls ContentsView::SetDocument(null). |
| `0x388dbc` | Calls manager SetDocument with the same Composer address and a null note, triggering the tested whole-cache clear. |

Thus the registration key is a Composer instance address, not a document
identifier. ContentsView detaches its NoteWritingView at `0x417b68` and
NoteObjectView at `0x417b78` before returning to the manager-clear call.
NoteObjectView::SetDocument calls onPageDeleteAll at `0x3c8584`; that method
removes child views and invokes their virtual destructors. NoteWritingView's
document update traverses internal helpers at `0x5324c8` and `0x50af4c`,
including virtual slot 88 on its drawing/control objects. These virtual
targets and child destructors remain the relevant synchronization boundary;
their worker-drain behavior has not been established by this audit.

The application-level decompiled
`com/samsung/android/support/senl/nt/composer/main/base/view/composer/ComposerView.java`
calls `setDocument(null)` before `close()` in releaseComposerView. Its
requestReadyForSave call is conditional. ComposerViewPresenter.changeNoteType
also detaches and reattaches the document without closing the view.
SpenComposerImpl.close later closes its draw loop. For the AGLL implementation,
SpenDrawLoopAGLL.close sets the destroy flag and calls
SPenRendererAdapterAGLL.callOnProcess(true), with native-finalize fallback when
that call returns false. The meaning and completion guarantees of that
adapter call have not been executed here.

These observations narrow the earlier ownership question: the later
draw-loop close does not establish ordering before cache invalidation.
Verification needs the native view/drawing teardown path or a device trace
showing queued manager releases complete before the whole-cache clear.
This is a static source/disassembly audit, not a concurrent runtime test or
a finding of a Samsung application leak. The standalone ownership tests
remain valid for the explicit call order they execute.
