# ColoredPencil V4 host raster and RGB

The explicit `ColoredPencilV4Host` canvas profile reconstructs source geometry,
nearest atlas sampling and ordered RGBA8 compositing onto the actual prior
pixels. It uses the shared rectangle varying-plane setup and host byte arithmetic
with Pencil2. Normal saved-document dispatch remains generic.

## Native controls

The [native canvas receipt](../../conformance/colored-pencil-v4-canvas-native.json)
retains 100 frames in about 250 KiB. Five source-owned GLV4 streams cover sizes 8
and 16, negative initial X and tilt values at/above the 0.65 branch. The existing
[geometry receipt](../../conformance/colored-pencil-native.json) supplies the
original native records; Rust regenerates those records from public `Stroke`
channels with the explicit glibc TYPE_3 RNG and portable libm profile.

The new capture reconstructs each input vector through native
`PenReturnCallback::GetBuffer`, `SetBuffer`, `AddPoint` and `SendDataToGPU`, then
calls unmodified RTV1 `Update(false)` and `Draw`. It does not rewrite vector or
drawable fields, shaders or uniforms. Public `ObjectStroke::SetColor` supplies
ARGB `8044aaee`, `ff44aaee`, `0044aaee` and `80c04020`. The latter three use the
baseline size-8 stream. Each stream/color exercises top/bottom source origins,
1x/2x projection and transparent, opaque-white and premultiplied-purple priors.
Each prior is uploaded to the actual native target and verified before Draw.

Four additional white-paper controls use non-square 384x320 and 768x640
frames at 1x/2x density, for both source origins. The earlier 96 cases are
unchanged.

In these original controls, the optional native depth map remains null. The static pattern
bitmap is not substituted for that depth map. The profile therefore establishes
the point-atlas branch, not depth/paper-dependent painting.

The captured library is `libSPenColoredPencil.so`, SHA-256
`39078a41b0687941c4e22b18df36addb1cda093303e26df844078474d3ca9504`,
with Mesa 25.0.7 llvmpipe/LLVM 19.1.7. All four earlier baseline frame hashes
are reproduced. The older paint receipt's incorrect standalone library digest
was corrected after checking all four original full capture files against their
retained capture digests; their native pixel bytes and frame hashes did not change.
No REA provider was connected; acquisition uses the existing project-local,
source-pinned emulator/GLES harness without registration changes.

## Pixel model and public admission

The vertex shader exports point-atlas V as `(row + W) * float32(1/3)`.
The rectangle's W values are 1 then 0 along its local Y axis. This differs from
Pencil2's coordinate expression. Projection and viewport retain F32 ordering;
triangle plane setup and pixel interpolation use the shared captured host model.
The lowp point sample normalizes R8 and narrows to F16 with RTZ before widening.
Source alpha is `(savedAlpha/255 * sampledRed) * instanceAlpha`, quantized to a
byte before premultiplication and the shared host source-over recurrence.

The [public adapter](../../crates/sdocx/src/ink/colored_pencil/source.rs) requires
literal `4;`, normal curve-enabled stylus input, explicit matching size/ARGB,
complete finite source channels, and selected PenData dimensions 1920x1080 with
spacing-extra zero in the original controls. The expanded profile accepts explicit
system-display dimensions in 1..=16384, independently of output frame dimensions.
Finite source size follows native `Pen::SetSize`'s 0.4..=800 selected-size clamp.
Uncaptured settings and transforms/extensions remain explicit errors. The caller
asserts a fresh redraw and this host RNG/math profile. Ordinary `paint` selects
the null-depth branch; `paint_with_depth_map` supplies the optional bitmap explicitly.

The [source-context receipt](../../conformance/colored-pencil-v4-source-context-native.json)
adds 31 native Drawing/selected-Redraw controls. These cover requested sizes -1,
0, 0.4, 1, 1.5, 2, 4, 6, 8, 12, 16, 32, 64, 256, 800 and 1024; seven alternate
display profiles from 1x16384 through 16384x16384; two bent variable-channel paths;
and six displacement-gate controls. Native `ObjectStroke` normalizes the negative
requested size to zero before `Pen::SetSize` clamps it. Rust matches every source
record byte for byte. Source dimensions come from `Context::GetDisplay()` and
Android real display metrics, not a note's page dimensions.

GLV4 `drawLine` at RVAs 0x2acc4..0x2ad6c skips inputs whose F32 Euclidean
displacement from its last processed point is below
`(selectedSize * 0.01) * (min(displayWidth, displayHeight)/360)`.
The early return preserves all path and endpoint state. `endPen` reads that
latched point at 0x2a1f4/0x2a244. The previous sampler processed every input and
used the penultimate source point; that was wrong for wide pens and dense tails.
The corrected sampler reproduces controls just below, exactly at and above the
gate, and the size-8 short-tail control. The clamp is independently visible in
PenCommon `Pen::SetSize` at 0x45fdc..0x46008. Rotation retains native delta-before-
center F32 narrowing; no trigonometric ULP correction was needed.

The [context-paint receipt](../../conformance/colored-pencil-v4-context-paint-native.json)
replays the 25 distinct selected-size/record pairs through public native APIs,
including `SetPenData`, and retains 300 full frames in about 660 KiB. Six source
controls share native paint inputs and are recorded as aliases. Each distinct
pair exercises both origins, 1x/2x projection and all three verified priors.
All 300 frames match the public Rust and compiled Node/WASM canvases exactly.
The 100 opaque frames also match SVG/resvg, PNG and both Rust PDF routes in
MuPDF/PDFium; the WASM direct-PDF output matches both readers. Alias controls independently
check public size clamping and equivalent display contexts. This extends host
profile admission; it does not infer missing source context from a saved note.

Source input is capped at 4,096 points, intermediate dabs/path samples at 65,536,
and paint at 16,384 records plus the shared frame/work limits. Source arithmetic
that overflows the derived dab size returns an error before calling jitter math.
Admission, invalid-channel and budget failures leave canvas pixels and counters
unchanged. Test-only path/dab traces do not allocate in production sampling.

## Optional native depth bitmap

The [depth receipt](../../conformance/colored-pencil-v4-depth-native.json) retains
360 native frames in about 1.2 MB. Native `SPGraphicsFactory::CreateBitmap`
constructs each supplied RGBA map; captured GL upload hashes must equal the
input bytes. Unmodified RTV1 `SetDepthMapBitmap` (0x2bc00) retains the map and
reads its native dimensions. `SetBackgroundScale` supplies an explicit scale.
The capture checks the latched pointer, dimensions and scale, unit-1 texture
binding, native uniforms and every prior upload/readback. It changes no native
fields, shader, uniform or geometry-buffer bytes. The original null-depth
baseline frame remains unchanged before the added draws.

Five maps cover red 255, 0 and 128 constants, a 31x17 varying red grid and the
same grid with alpha zero. Two native source streams (sizes 8/64), background
scales 0.5/0.75/2, both origins, 1x/2x projection and three backdrops produce
the 360 controls. Native all-red-255 frames equal their null-depth counterparts;
red-zero leaves the prior unchanged; changing only grid alpha changes no output.
This verifies a red coverage texture, not RGBA backdrop compositing.

The shipped vertex shader computes paper UV as `(clip.xy / clip.w + 1) * 0.5`
before interpolation. RTV1 uploads
`uPaperScale = (outputDimensions / depthDimensions) / backgroundScale` in F32.
Paper therefore anchors to the framebuffer's bottom edge, independently of
source origin and source output scale. Actual native sampler state is linear
with clamp-to-edge and no mip levels. Its RGBA8 red sample follows the same
8-bit subtexel-coordinate conversion and ordered byte X/Y lerps already modeled
by Smudge and OilBrush3, then the shared F16 RTZ conversion. Shader alpha is
`((savedAlpha/255 * pointSample) * paperSample) * instanceAlpha`, quantized before
premultiplication and source-over. The Rust model matches all 360 frames exactly.

The public method accepts a bounded borrowed bitmap and background scale;
Rust regenerates the source records and compares complete output pixels plus
opaque SVG output against all native frames. Map shape, dimensions, scale,
family and work failures are atomic. The map is not retained between submissions.
Native `SetDepthMapBitmap(null)` is a no-op; ordinary public `paint` asserts a
fresh null-depth profile and does not emulate clearing native sticky bitmap state.
Real-note depth selection, GL queue ownership and device display selection
remain unverified. The display-derived arithmetic and native queued setter
have separate evidence below.

The [compiled Node/WASM depth controls](../../conformance/colored-pencil-depth-wasm-canvas.json)
match all 360 native/Rust frames and reject malformed depth storage atomically.
The [Rust paired exports](../../conformance/colored-pencil-depth-public-canvas-export.json)
match all 120 opaque native frames through PNG, SVG/resvg, SVG-page PDF and
retained-scene direct PDF. MuPDF and PDFium each match both Rust PDF routes;
the [WASM direct-PDF controls](../../conformance/colored-pencil-depth-wasm-canvas-export.json)
also match both readers at intrinsic 256x256 pixels and 72 DPI. Together with
the earlier controls, public Rust/WASM coverage is 760 native frames and 256
opaque export controls. These PDFs carry the resolved raster frame.

## Display-derived background scale

The [producer receipt](../../conformance/colored-pencil-v4-background-scale-native.json)
retains eight native Drawing/selected-redraw controls in about 15 KiB. Read-only
hooks at saved GLV4 `RedrawPen(MotionEvent)` RVAs 0x2a6b0/0x2a6bc/0x2a6cc observe
PenData dimensions, the selected integer minimum and the F32 division result.
Queued native RTV1 `SetBackgroundScale` at 0x2c974/0x2c978 receives and stores
the exact same bits. Both phases reproduce their earlier geometry rows unchanged.
The observed equation is `float32(min(displayWidth, displayHeight)) / 1440`,
with no clamping. Display profiles include portrait/landscape 1080 minima,
600, 480, 1440, 2160, 1 and 16384. This is the system display supplied by
Drawing's context, independently of output page dimensions.

The [96-frame paint join](../../conformance/colored-pencil-v4-display-depth-native.json)
reuses each native source stream and its observed scale through unmodified
RTV1 setters and depth paint. Each case verifies a fresh native grid bitmap
upload, shader scale, texture binding and actual prior pixels. It covers the
31x17 map, three priors, both origins and 1x/2x projection. It joins two measured
stages; it is not a full native Drawing paint pass or SDK deserialization.

The public Rust `InkRasterDepthContext::from_colored_pencil_display` and WASM
`colored_pencil_depth_context` reproduce this producer from caller-supplied
display dimensions. Depth admission now covers the resulting
`1/1440..=16384/1440` range. Rust source-to-pixel and SVG tests match all 96
native frames; existing null-depth and explicit-depth controls remain exact.
In the 800x600 bottom-origin 1x white control, replacing the native scale with
0.75 produces 461 differing channels, absolute sum 2133 and maximum 37. The
derived context removes all differences. Native display selection, depth-map
selection and persistent bitmap lifecycle still require separate caller context.

The [Node/WASM controls](../../conformance/colored-pencil-display-wasm-canvas.json)
also match all 96 frames and check native scale equality plus invalid-display
rejection. All [32 opaque Rust exports](../../conformance/colored-pencil-display-public-canvas-export.json)
match PNG, SVG/resvg and both PDF routes in MuPDF/PDFium; the corresponding
[WASM PDFs](../../conformance/colored-pencil-display-wasm-canvas-export.json)
match both readers. Combined public coverage is 856 native paint frames and
288 opaque export controls. Browser execution and complete saved-note/device
rendering remain unverified.

## Native Drawing and retained depth

The [Drawing lifecycle receipt](../../conformance/colored-pencil-v4-drawing-lifecycle-native.json)
adds 96 full native Drawing frames in about 369 KiB. A native
`SPGraphicsFactory::CreatePenCanvas` supplies the pen canvas over the existing
native output bitmap. Native Model size/color setters, cached PenManager
selection, Drawing depth handoff, GLV4 saved redraw, callback task, RTV1 update
and GLES Draw execute together. Each Drawing call emits exactly one native
draw and regenerates its earlier size-8/16 geometry stream unchanged. No
geometry replay, native-field, shader, uniform or scheduler rewrite is used
for these added frames. Context/display, identity color conversion, queue
execution, host math and Mesa remain explicit host boundaries.

Twelve eight-stroke sequences cover both origins, 1x/2x projection and three
verified initial priors. The source changes between sizes 8/16 and ARGB
8044aaee/80c04020/ff44aaee. Real native factory bitmaps supply a red grid and
red-zero coverage. All intermediate frames are retained, including depth
replacement, nulls and geometry clear. These are synthetic native Model inputs,
not SDK document deserialization or a full mixed-tool saved page.

| Operation before redraw | Drawing pointer | GL pointer | RT depth used for paint |
| --- | --- | --- | --- |
| Drawing `SetDepthmap(null)` | Cleared | Retained | Retained |
| GL `SetDepthMapBitmap(null)` with null Drawing depth | Null | Cleared | Retained |
| RT `SetDepthMapBitmap(null)` | Unchanged | Unchanged | Retained |
| RT `Clear()` | Unchanged | Unchanged | Retained |
| Drawing selects a valid replacement | Replacement | Replacement | Replacement |

Drawing's setter at 0x80564 stores its pointer at +48. `drawObjectStroke`
at 0x81e00 skips the handoff when that pointer is null. The GL setter at
0x2b130 releases/clears its +280 pointer and admits a replacement only when
`GetTexture` returns non-null. The callback carries that pointer into the
native render task. The RT setter's null case preserves +248; `Clear` only
clears its geometry submission. Red-zero replacement frames leave the previous
pixels unchanged. The retained map is separate from the already painted backdrop.

The [geometry-shell lifecycle receipt](../../crates/sdocx/tests/assets/colored-pencil/colored-pencil-v4-depth-lifecycle-native.json)
separately retains ten pointer/reference controls, including a host bitmap
without a texture. That shell acknowledges the callback render task without
executing it, so its RT depth remains null. Its evidence stops at the GL and
callback handoff; the native pen canvas above closes that observation gap.

The public Rust and Node/WASM regressions independently reconstruct Drawing,
GL and RT depth state, then resubmit the retained map through
`paint_with_depth_map` in stroke order. Ordinary `paint` still asserts a fresh
null-depth profile. Substituting it after GL null in the top-origin 1x white
sequence produces 333 differing channels, sum 2666 and maximum 47; retained
depth has zero errors. Higher-level note reset/disposal and actual depth-map
selection remain unverified.

The [Rust paired controls](../../conformance/colored-pencil-drawing-public-canvas-export.json)
and [Node/WASM controls](../../conformance/colored-pencil-drawing-wasm-canvas.json)
match all 96 native Drawing frames. Their 32 opaque prefixes also match PNG,
SVG/resvg and both Rust PDF routes in MuPDF/PDFium; [WASM PDFs](../../conformance/colored-pencil-drawing-wasm-canvas-export.json)
match both readers. Combined public coverage is 952 native frames and 320
opaque export controls, with persistent state supplied explicitly per submission.

## Mixed Pencil2 prior

The [joined capture](../../conformance/colored-pencil-v4-pencil2-drawing-native.json)
adds twelve native Drawing prefixes in about 75 KiB, including four compressed
prior images. Each prior comes from a retained native Pencil2 mixed-record
frame, with forward/reversed record order and 1x/2x projection. Its 192x192
pixels are placed at GL (0,64) in a white 256x256 frame without resampling.
The native target upload is read back and verified before the first Drawing
call. ColoredPencil then changes size/color and exercises Drawing-null and
GL-null depth transitions through the existing native Drawing/callback/GLES
route. Original baseline and regenerated geometry hashes remain unchanged.

The lasting Rust regression verifies the upstream receipt hash, every placed
prior pixel, distinct outputs for the two Pencil2 record orders, operational
depth transitions and every cumulative frame. All twelve frames match native
exactly. For the 1x mixed-prior GL-null prefix, a fresh null-depth submission
produces 328 channel differences, sum 2573 and maximum 46; retaining the map
removes all differences.

The [Rust exports](../../conformance/colored-pencil-mixed-public-canvas-export.json)
match all twelve opaque frames through PNG, SVG/resvg and both PDF routes
in MuPDF/PDFium. These controls join separate native producers through RGBA
bytes; they do not establish one native mixed-tool Drawing invocation, SDK
deserialization or saved-note/device parity. The controls use synthetic source
inputs; real saved ColoredPencil admission remains unverified.

The [compiled Node/WASM canvas](../../conformance/colored-pencil-mixed-wasm-canvas.json)
also matches all twelve frames, reconstructing the same retained-depth state.
Its SVG carriers equal Rust's, and [direct-PDF output](../../conformance/colored-pencil-mixed-wasm-canvas-export.json)
matches both readers. Combined ColoredPencil coverage is 964 native frames
and 332 opaque export controls.

## Public outputs and remaining work

The [Rust public regression](../../crates/sdocx/tests/colored_pencil_raster.rs)
compares all 100 native frames through `Stroke -> InkRasterCanvas::paint`. The
[paired export receipt](../../conformance/colored-pencil-public-canvas-export.json)
checks all 36 opaque-white frames through PNG, SVG/resvg, SVG-page PDF and
retained-scene direct PDF. MuPDF and PDFium match native exactly at intrinsic
destination pixels and 72 DPI, including the non-square frames. These PDFs
carry the resolved raster image.

The [compiled Node/WASM controls](../../conformance/colored-pencil-wasm-canvas.json)
also match all 100 native/Rust frames, with rejected-context atomicity and matching
SVG carriers for the 36 opaque frames. Their [direct-PDF measurements](../../conformance/colored-pencil-wasm-canvas-export.json)
match native in both readers. Browser UI execution remains untested.

The expanded context controls have separate Rust/WASM export receipts linked in
the tools README. These receipts record historical runs and their script, reader
and binary identities; some script hashes describe earlier revisions, not the
current helpers. They are not a fresh build attestation.

The [transparent export receipt](../../conformance/colored-pencil-transparent-export.json)
covers 64 clear/translucent-prior base frames. PNG straight-alpha conversion and
SVG carriers agree; both PDF routes match PNG in PDFium. MuPDF differs from native
in 56 frames by at most one channel value. This bounds base-profile transport
at intrinsic size; expanded context, depth and mixed controls do not establish
general translucent-reader parity. Composing a final transparent layer over paper
is not a substitute for painting each stamp onto that paper. Real saved-note PenData/context, other source settings,
other depth sampler/scale profiles, object transforms, replay, other zooms and Samsung-device/Bionic
RNG/libm behavior remain unverified. A complete native mixed-tool Drawing page
is separate from the verified caller-owned RGBA backdrop contract.

Reproduction commands and retained harness dependencies are in
[tools/colored_pencil](../../tools/colored_pencil/README.md). Rust owns lasting
regressions; Python in tools acquires and measures native evidence.
