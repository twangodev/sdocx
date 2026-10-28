# Pencil3 saved-stroke source authority

## Evidence and scope

Samsung Notes 4.4.45.37 APK SHA-256
`daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667`.
Static ARM64 traces are supplemented by bounded native geometry, event and
texture execution and synthetic host GLES comparison below. Pencil3's extracted
library matches its APK entry. Addresses are ELF virtual addresses.

| Source | SHA-256 |
| --- | --- |
| `libSPenPencil3.so` | `41a995cac7a597af53359401df26187d4299221f3460c91b56540596fc73d359` |
| `libSPenDrawing.so` | `788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd` |
| `libSPenPenCommon.so` | `afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d` |
| Decompiled `pen/pen/SpenPen.java` | `75fe3f0d71e4ea5e979bd5a6b6803454283571e01c6ffcfecbb91a45e5adb9c7` |

The [registry](pen-selection-findings.md#built-in-native-registry) resolves
Pencil3 independently of Pencil/Pencil2. This finding covers one selected V1
profile and the single ordinary saved-stroke route, rather than every version,
queued batch, preview or live gesture. The [recorder](stroke-recording-findings.md)
establishes saved source channels separately from these rendering inputs.

## Saved channels feed generated particles

Pencil3 `createPenInst`, `0x106678`, constructs the plugin. Its main slot 232
relocation `0x11a8f0` binds `GetStrokeDrawableGL`, `0x106404`. The getter reads
the configured version and `versionTable` (`0x106418`, GOT `0x11c298 →
0x120b18`); entry 1 selects V1 construction at `0x10649c`.

Native Drawing probes distinguish null settings (V1) from empty settings (V3).
Explicit `0;`/`1;`/`3;` select V1, `2;` selects V2, and `4;` matches empty V3
bit for bit against the Rust-consumed V3 fixture. The getter clamps indices to
1–4 and reads table `[0,1,2,1,3]` at `0x120b18` (`0x106418–0x1064ec`).
These are synthetic Model setter inputs, not persisted-note mapping; a literal
`basepen` control through host C-`atoi` establishes no serialized syntax.

Drawing `ObjectDrawing::redrawIPen`, `0x82abc`, requires count 1 and
StrokeType != 1 for its ordinary single-object branch. It obtains that drawable
through pen slot 232 and calls secondary-interface slot 40 (`0x82b54–0x82b70`).
V1 installs that interface at `this+8`; relocation `0x11abc8` binds its saved
adapter thunk `0x1088d8`, which adjusts `this` and calls `0x1087d8`.
The adapter reads stored tool, count, XY, pressure, time, tilt and orientation
(`0x108808–0x108854`), then constructs a MotionEvent with those arrays and
the first timestamp (`0x108860–0x108884`). Primary slot 144, relocation
`0x11ab78`, dispatches to V1 saved-event redraw `0x107b04`. The adapter and
thunk ignore the inner result and return true; that is not rendering success.
Saved redraw calls libc `srand(1)` at entry (`0x107b3c`, `0x107b50`), before
its event/canvas guards. Its reached `drawPoint` and `drawLine` calls
(`0x107e54`, `0x107f58`, `0x108724`) reach `drawPoint`, `0x108338`, which
calls `rand` three times (`0x108398`, `0x1083d0`, `0x1083ec`). Two values
produce size-dependent offsets around sampled XY. The third becomes a signed
remainder by 16. RTV1::AddPoint receives the resulting tuple (`0x108454`).

The saved array constructor in Base (`0xbfd84`) leaves the default event source
at zero (`0xbf46c`); `GetSource`, `0xc0a9c`, reads that field. Executing the
actual constructor and getters for the ten saved cases confirms source zero,
including tool 2. Saved replay therefore bypasses the tool-2/source-`0x1002`
pressure branches (`0x10751c–0x107544`, `0x1077e8–0x107804`). This does not
establish live input-device sources.

Ordinary Drawing also skips morphable configuration when `GetMorphable`
returns null (`0x81eec–0x81ef0`). Pencil3's relocation `0x11a928` selects
PenCommon's null-returning implementation (`0x46468`), so the saved fixed-opacity
setter (`0x81f98–0x81fac`) is not reached for this pen. Input-color alpha and
generated particle alpha remain separate shader inputs.

That consumer appends X, Y, half-size, random remainder and alpha
(`0x10c880`, `0x10ca30/40`, `0x10caf4`). Its descriptor and update bind a
four-component point attribute plus alpha, with count=bufferFloats/5
(`0x10c378/390`, `0x10cca8–0x10ccd8`). The bound vertex source at `0xf9730`
uses the fourth component to select a row of the 16-piece brush pattern.
This value is neither a saved random seed nor another source coordinate.
Fixed reseeding does not establish whole-application repeatability: the C
random implementation, call ordering, profile and sampling remain separate
conditions. No Android-versus-host sequence or numerical parity is established.

## Paper coverage has separate runtime ownership

Drawing `SetDepthmap`, `0x80564`, stores a supplied bitmap at renderer+48.
Saved drawing passes it at `0x81e00–0x81e0c` to `SetPenDepthmapBitmap`,
`0x750c8`. For StrokeType 0, pen slot 232, GL slot 56 and depth-map interface slot 16
reach V1's setter
(`0x1089c8`, interface relocation `0x11ac10 → 0x1089ac → 0x108954`).
The renderer's upstream document/paper producer is not resolved here.
Separately, SDK `SpenPen.setDepthMapBitmap` (`:1043–:1056`) passes its supplied
Java Bitmap to PenCommon JNI `0x41120`. It creates a graphics bitmap from
those pixels and dimensions (`0x41188`, `0x41224`), calls the same interface
(`0x41254`), then releases its local reference (`0x41260`). This proves a
configuration input, without establishing a concrete app paper-file producer.

APK `SpenPaintingViewImpl.setPaintingDoc` loads the bundled 770×965
`paper_kent_depthmap.jpg` through `Native_setPaperDepthMap`. That painting API
is distinct from the pen depth-map setter; the extracted smali does not connect
it to Pencil3, so it is not evidence for this pen's runtime paper default.

V1's GL setter releases its old member 488 bitmap and references an accepted
new pointer (`0x108964–0x108998`); its destructor releases it (`0x106f68`).
Saved redraw supplies it to PenReturnCallback (`0x107c4c`), whose destructor
copies callback+56 into queued message+16 (PenCommon `0x50524–0x50538`).
The message's actual execute slot (`0x774d8 → 0x4676c`) supplies it through
the RT depth-map interface before Draw (`0x46920–0x46970`). RTV1 caches it
at +248; null or same-pointer setting is a no-op (`0x10c7e4–0x10c854`).
These bodies do not establish queue-wide failure or lifetime guarantees.
RTV1 Draw selects that supplied paper bitmap or default member 240
(`0x10cde4–0x10ce74`). The default 150×150 paper comes from embedded
`g_backgroundPtn` (`0x11c2c0 → 0xf3c8c`, copy `0x10c248–0x10c29c`);
the brush pattern separately comes from `g_penPtn` (`0x11c2b8 → 0x12c8c`).
The bound fragment source, `0xf9948`, multiplies paper-red coverage, brush-red
coverage, input color alpha and point alpha, with factor 1.5. This identifies
coverage inputs rather than proving texture or compositing equivalence; see
the separate [opacity findings](pen-opacity-findings.md).

A [V3 scale-producer replay](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/pencil3-paper-scale-native.json)
executes the screen-resolution handoff, native queued task and RTV1 setter in
four contexts. GLV3 `RedrawPen` computes `min(width,height) / 1440`
(`0x10b1e4–0x10b200`); the task carries it to RTV1+264. The observed scales are
0.75, 0.5, 1.0 and 0.75, including portrait. Context/bitmap adapters and queue
scheduling are host supplied; this witness stops before paint.

The [bitmap-consumer replay](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/pencil3-paper-consumer-native.json)
joins native Drawing particle callbacks to the queued message's RT receiver,
then executes its depth-map interface thunk and RTV1 setter. The synthetic
320×180 ISPBitmap is stored at RT+248 and its dimensions at +256/+260.
The harness explicitly calls native Init on the selected RT and runs the
queue synchronously. Native Draw then faults at Graphics `0x8e0d0` because
the host subbitmap has no framebuffer target; completed paint is not established.

Paper UV is `(clip.xy / clip.w + 1) / 2`, multiplied by
`(canvasWidth / paperWidth, canvasHeight / paperHeight) / backgroundScale`.
Native controls vary these inputs; projection and viewport determine phase,
so page dimensions alone cannot recover paper placement.
Fresh native callbacks configure built-in `RTV1+240` paper to NEAREST/REPEAT.
Supplied `+248` paper is activated without sampler setters and inherits producer
state. The host oracle's NEAREST/CLAMP initialization does not establish device
state or the built-in repeat policy.

The paper bitmap is distinct from an [editable Painting source](painting-source-findings.md)
and does not replace saved XY. No inspected route proves this runtime input
is serialized in an ordinary stroke. Preserve original samples, optional
channel presence, tool, pen/settings identities and source bytes independently
of generated particles. Pencil profiles use the [pressure approximation](stroke-rendering-findings.md#current-sdk-behavior)
by default. An explicit `RenderOptions::pencil3_profile` now selects bounded V3
source reconstruction and intrinsic-texture paint; it requires a matching page
canvas, supplied screen dimensions and an explicit paper sampler. [Native PDF stroke bitmaps](native-pdf-stroke-findings.md)
are derived output; original vector channels and configured coverage inputs
remain separate source boundaries.

The generic fallback preserves explicit saved RGB for the same bounded V3
source profile. This follows the shader's input color; it does not reproduce
the texture or apply partial saved alpha to the whole approximate stroke.

The conformance witnesses include [`pencil_native.py`](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/pencil_native.py)
for native `drawPoint`, RTV1 `AddPoint`, bounds and saved redraw; ten saved
cases use an AOSP bionic RNG model checked against CTS vectors, not Samsung
runtime libc execution. The test-only Rust sampler matches all ten saved cases
and eight compact boundary witnesses, including the 200-unit debounce cutoff
and 50-entry tilt average. The compact witnesses hash every packed f32 attribute;
the tilt-modulation divisor at `0x12b50` is exactly `0x3ef5c290`, not `0.48f32`.
[`pencil_event_native.py`](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/pencil_event_native.py) separately
executes the native saved-array constructor and source/tool/history getters.
[`pencil_texture_native.py`](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/pencil_texture_native.py)
executes native `buildTextures`/`setTextures` with explicit bitmap interfaces
and asserts byte-exact brush-atlas and paper uploads. The sampler mapping is
anchored to Renderer SHA-256
`f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed`:
`SetParam` at VA `0x5f840` and `getGlPname` at `0x5f9bc` read wrap, filter,
and pname tables at `0x30170`, `0x301c0`, and `0x31768`. Decoding those bytes
maps enum values to GL wraps `0x8370, 0x812f, 0x812d, 0x2901`, filters
`0x2600, 0x2601`, and parameter IDs 0–2 to `0x2802, 0x2803, 0x8072`;
Pencil3 requests nearest filtering, built-in paper repeat wrapping, and brush
clamp-to-edge defaults; supplied depth-map sampling is producer-owned. [`pencil_paint_native.py`](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/pencil_paint_native.py)
executes the embedded V1 shaders on host Mesa GLES with the native quad,
textures and saved particle vectors; its projection, paper scale and color
are synthetic, and its pixel fixture does not establish Android/device
parity. The app's real render-queue execution and framebuffer remain outside
these captures.

V3 preparation now stops at 2,048 particles and 65,536 source points, with no
partial stream or caller RNG changes on rejection. Admission requires exact
Pencil3 empty/`4;` settings, tools 1, 2 or 3, complete finite channels, explicit
ARGB and supported style defaults. Explicit-profile replay reveals the same
saved-redraw particle stream using source-event cutoffs.
A [curve-disabled native control](../../conformance/pencil3-no-curve-native.json) matches the complete tool 2 and tool 3 curve-enabled streams; the retained tool 1 curve-disabled and curve-enabled captures also match all 77 records. The V3 profile therefore admits either saved curve flag. These controls use synthetic Model setters and host adapters.
A [curve-enabled tool 1 capture](../../conformance/pencil3-v3-tool1-curve-native.json) verifies the complete 77-record V3 stream through the native Drawing route; Rust matches every packed word. The earlier tool 1 fixture used a straight input and did not establish this admitted route.
Native `Pen::SetSize` at PenCommon `0x45fdc` clamps to [0.4,800]. Drawing forwards
omitted size as zero, yielding 0.4; explicit 0/0.1 and 800/900 controls match
complete 1,048- and 7-record streams respectively in Rust. Supplied and omitted
optional axes remain distinct. The strict ordinary-style gate does not establish
native behavior for the rejected effects or all property combinations.

The [negative-size Drawing probe](../../conformance/pencil3_negative_size_capture.py)
and its [receipt](../../conformance/pencil3-negative-size-native.json)
also captures -10 and -0.1. The native Model setter's getter returns 0 for
both; Drawing's Pen setter then clamps that to 0.4. Both produce the same complete
1,048-record stream (SHA-256 `e597079876c3b10ab3def4cf395d8db944ba565d744c33cf5f682f09f04108a4`)
as 0.4; Drawing and saved redraw agree. This supports finite negative sizes in
manually constructed public strokes under the native setter's clamp. The Rust
serialized-stroke parser rejects negative pen sizes before this renderer is
reached. Native serialized-input acceptance remains unverified; these captures
are synthetic Model inputs, not serialized notes or device output.

Four identical saved points produce seven particles at each tested constant
pressure (0, 0.5 and 1), rather than an empty stream. The complete native
Drawing and saved-redraw streams match each other and the Rust adapter in the
[stationary controls](../../conformance/pencil3-stationary-native.json). These
are synthetic Model inputs under the same host adapters, not device frames.

A [hybrid serialized-frame
control](../../conformance/pencil3-generated-serialized-native.json) supplies
four synthetic points and channels, a synthetic string table, and a 166-byte
type-1 frame retaining source-derived structural and style bytes, including
pen size f32 bytes `6ade1141` to native Model attachment and Drawing under the
host Unicorn adapters. Drawing selects Pencil3 GLV3 and emits 51 records. A
Rust regression decodes the same type-1 frame following a synthetic type-0
base, manually resolves the two relevant string IDs, and matches every byte of
the native record stream at the explicit 1920×1080 screen setting; the public
viewport preparer also selects its V3 profile. The receipt retains the hybrid
frame and synthetic table, native stream, library and local script hashes, and
the original invocation. The capture depends on ignored scratch and tmp
harnesses and an operator-local source note, so it is not portable to a clean
checkout. This does not exercise Rust archive or note-table parsing, the whole
native note loader, public SVG/PDF paint, or Android device output.

A [short-stroke control](../../conformance/pencil3-short-stroke-native.json)
found that native Drawing returns failure without selecting a Pencil3 drawable
for one source point. Two and three points produce 19 and 31 particles,
respectively, with matching Drawing and saved-redraw streams. These synthetic
host controls support the existing two-point admission boundary; they do not
establish behavior for every input or device.

Single-stamp transport probes also separate source sampling from export
rasterization. Nearest texel selection matched all 25/100 tested fragment
centers at 1×/2×, but vectorizing one brush row required 4,354 source-texel
rectangles (about 247 KB of PDF for one stamp). Over opaque white, MuPDF and
PDFium still differed from the host GLES stamp, with maximum channel errors
of 68–73. These hand-built PDF probes do not exercise the SDK's shared
SVG-to-PDF route; they establish neither device parity nor a production
transport. The scripts and measurements remain in the ignored
`tmp/vector-parity-plan/pencil-stamp-*` experiments.

The explicit public route retains the original brush atlas and repeats/clamps
the recovered paper texels at their intrinsic density in a bounded image.
This avoids resvg's Bicubic SVG-pattern pass and rounded repeat periods;
Rust checks nearest samples across fractional seams in both Y conventions.
The materials are shared by sampler/canvas/scale context, with a 64 MiB decoded
paper budget checked before scene mutation. Stamp geometry remains vector.
Canonical top-down host controls use a 320×180 canvas and explicit screen
2560×1440, yielding paper scale 1. The native matrix is supplied through the
host GL adapter, not recovered from Samsung's natural app context.
Paired CLAMP/REPEAT controls share the complete 48-record native stream and
source textures; the matching vector sampler lowers error across Chromium,
Firefox, resvg, MuPDF and PDFium against the no-profile fallback. PDF raster residuals,
consumer sampling, real paper/projection inputs, broader source
profiles and Samsung-device comparisons remain open. Scratch captures and
cross-renderer reports stay outside maintained conformance files.

Saved-redraw attribution hooks identify seven initial `drawPoint` calls at
Pencil3 RVA `0x10b430`, interior calls at `0x10bb24` with loop index W24,
and terminal calls at `0x10ad84`. Baseline cumulative ends are [7,13,25,48];
a varied-pressure/tilt control with a skipped 0.02236 movement has
[7,14,14,47]. Rust tests pin both complete streams and cutoffs. The ignored
receipt SHA-256 is `281722a1a62dda32d57534c103c311c5f9fdd5678f12b86f33637f33c28e0f70`.
This establishes source ownership within a full saved redraw, not live gesture
behavior or equality with redrawing truncated source prefixes.
