# Rendering support

This document maps the Rust SDK's saved-document rendering surface. It separates
structural inspection, semantic decoding, drawing, and evidence of Samsung
behavior. It is not a claim of complete Samsung Notes compatibility.

“Implemented” means a Rust route exists for the stated scope. “Approximate” means
the route preserves content with geometry or appearance that does not reproduce
the native feature. Native evidence may establish a serialized field, an isolated
arithmetic kernel, a bounded producer, or a real-document comparison; these are
different guarantees. Successful parsing and an empty diagnostic report do not
certify visual parity. Chromium preview and vector SVG/PDF are the supported
rendering target. Bounded Fountain V14/V16 witnesses also render in Firefox.

## Object types and locations

The [object identifiers](../crates/sdocx/src/types.rs) and
[physical page decoder](../crates/sdocx/src/page.rs) define the first boundary.
Every stored object retains its declared type, payload location and child
structure through [storage](../crates/sdocx/src/storage.rs). Borrowing its original
payload requires the original uncompressed page bytes. Retention does not imply
that the object has semantic content or a renderer.

| Native ID | Object | Semantic decoding or inspection | Rendering |
| --- | --- | --- | --- |
| 0 | None | Stored/common metadata | No dedicated renderer |
| 1 | Stroke | Sample channels, properties and pen resources | Page vectors; supported reconstructions or generic approximation |
| 2 | TextBox | Rich text, styles, paragraphs and object spans | Page text and supported embedded content |
| 3 | Image | Geometry, media references and supported crop fields | Page and text-flow images; effects are partial |
| 4 | Container | Common metadata and ordered children | Nested page objects; supported parent visibility and root selection |
| 7 | Shape | Geometry, styles, saved paths, controls and embedded text | Supported page templates/paths, solid paint and bounded linear/radial fill/outline gradients; partial styling |
| 8 | Line | Geometry, styles, controls and saved paths | Straight lines and supported paths; partial styling/routing |
| 9 | DeprecatedDummyStroke | Stored/common metadata | No dedicated renderer |
| 10 | Voice | Stored/common metadata; [own attachment/source contract](reverse-engineering/voice-source-findings.md) is distinct from typed note VoiceData | No voice-object renderer |
| 11 | Formula | Bounded expressions, result/image rectangles, media ID, strokes and label graphs | PNG placement with resolved native bounds; source/answer ink fallback for absent or corrupt images; other image formats unsupported |
| 12 | DeprecatedTable | Stored/common metadata | No dedicated renderer |
| 13 | Web | Stored/common metadata | No dedicated renderer |
| 14 | Painting | Stored/common metadata | No dedicated renderer |
| 15 | StrokeDevelopmentVersion | Stored/common metadata | Not routed through the type-1 stroke renderer |
| 16 | Video | Stored/common metadata; [source/thumbnail binding contract](reverse-engineering/vector-retention-findings.md#physical-video-source-bindings), without a typed Video projection | No dedicated renderer |
| 17 | Link | Stored/common metadata; separate rich-text hyperlink spans exist | No link-object renderer |
| 18 | StrokeBrush | Stored/common metadata | Not routed through the type-1 stroke renderer |
| 19 | Unknown | Explicit Samsung unknown marker | No dedicated renderer |
| 20 | Plot | Bounded plot/graph expressions, colors, styles and visibility | Inspection only; no graph evaluation or plotting |
| 21 | Math | Bounded formula envelopes, margins and connected plot UUIDs | Inspection only; no automatic child-formula drawing |
| 22 | Table | Typed table when embedded in rich text | Text-flow span only; no physical page-object renderer |
| 23 | CodeBlock | Typed title/body when embedded in rich text | Text-flow span only; no physical page-object renderer |
| 24 | AttachedFile | Stored/common metadata, including common attachment reference | No attachment-card renderer |
| 100 | StrokeGroup | Stored/common metadata and children | No group renderer; supported children can be traversed separately |
| Other IDs | Future/unrecognized objects | Stored structure and unknown-type diagnostics | No own renderer; supported children can be traversed separately |

Rich-text routing is separate: [initial table/code decoding](../crates/sdocx/src/note.rs),
[image resolution](../crates/sdocx/src/image.rs), and
[embedded rendering](../crates/sdocx/src/render.rs) support only image, table and
code content. A table inside a text box therefore differs from a physical
type-22 page record. Flattened supported children under an unsupported parent
do not establish that parent's transform or composition semantics.

Native [card contracts](reverse-engineering/card-source-findings.md) distinguish
saved Web/Link/AttachedFile text, action metadata and source bindings from
regenerated card artwork. A thumbnail does not retain the contents of a bound
Web or attached document.

The [formula](reverse-engineering/formula-findings.md),
[math](reverse-engineering/math-findings.md), and
[plot](reverse-engineering/plot-findings.md) findings describe inspection APIs.
Native [formula drawing](reverse-engineering/formula-rendering-findings.md)
has separate image/stroke selection rules; decoded expressions do not require
or establish an SDK evaluator.

## Ink

Sample coordinates, pressure, timestamps and optional tilt/orientation are
decoded and retained. A retained channel is not necessarily used by a pen's
drawing route. The [pen registry](../crates/sdocx/src/ink.rs) identifies names and
libraries, not a renderer for every registered pen.
A [native stroke-frame capture](../conformance/object-stroke-channels-native.json)
checks a real 204-point frame, three float-channel and six XY delta controls.
Compressed coordinates and float channels round each addition to float32;
outer SDK loading is not exercised.

| Feature | Rust rendering | Evidence and remaining limits |
| --- | --- | --- |
| Fountain V14, saved `14;` | Reconstructed stamps/directions with [saved defaults](../conformance/fountain-v14-defaults-native.json); vector directional shading, colorizer and maximum blending; tools 1–3 and fixed width | Native geometry, real-document comparisons and [host shader captures](../conformance/fountain-v14-field-native.json) and a [full saved-stroke replay](../conformance/fountain-v14-real-native.json). V14 colorizer witnesses render in Chromium and Firefox with vector-only PDF export. Native [expansion parameters](../conformance/fountain-v14-addpoint-native.json) and [synthetic thin-stamp shader checks](../conformance/fountain-v14-fringe-native.json) cover fringe behavior, which is not yet painted; minimum-width dots can disappear, and partial-alpha/PDF raster appearance still differ. |
| Fountain V16, saved `18;0;100;` | Reconstructed stamps with pressure/tilt/time/movement and fixed width; saved size/tolerance defaults and relative endpoint time; tools 1–3; vector radial coverage at canonical unit inverse scale | Executed native geometry, [saved Drawing defaults](../conformance/fountain-v16-defaults-native.json), [endpoint timing](../conformance/fountain-v16-event-time-native.json), [radial field records](../conformance/fountain-v16-addpoint-native.json) and scalar coverage checks. Four of five paired default cases lower mean error across Chromium, Firefox, MuPDF and PDFium; the fifth is unchanged. PDF peak errors and raster sampling gaps remain. PDF preserves vector gradients/masks. Device/zoom compensation and native pixel parity remain unverified. This setting selects drawing V16, not V18. |
| DefaultPen, ordinary saved tools 0–3 | Native positions/radii rendered as ordered source-over circles; omitted size uses the native 0.4 clamp | Native geometry, replay-boundary and [admission controls](../conformance/defaultpen-admission-native.json); paired vector exports improve on the generic fallback. Shader edge coverage and device pixel parity remain unverified. |
| Marker2 | Sampled circular stamps, native integer size and saved alpha | [Native caller controls](../conformance/marker2-saved-caller-native.json) check full XY streams and size defaults; [host paint controls](../conformance/marker2-zero-paint-native.json) confirm zero-width strokes produce no ink. V2 thin-edge shading and complete device appearance remain unverified. |
| Marker4 / StraightHighlighter V7, saved `7;` | Stylus midpoint sampling, saved style size and fractional rounded rectangular tip | Native geometry and [Drawing size/alias controls](../conformance/marker4-saved-caller-native.json); public support remains approximate because filtering/zoom coverage differ. |
| Marker4 / StraightHighlighter V8, saved `8;` | Sampled rounded rectangular tip, native integer size, tool-specific rotation and saved alpha | Executed Drawing size/alias controls, static native paint tracing and one paired Samsung PDF comparison; filtering/zoom coverage remain approximate. |
| Other pens, unknown names or rejected settings | Generic straight segments with pressure width, polyline or tap circle | No pen-specific native texture, smoothing or width-law guarantee. |
| Rainbow, eraser and straighten effects | Metadata retained; native effect semantics not reproduced | Gates vary by profile. V8 accepts straighten/fixed-width bits without implementing their separate effect semantics. |
| Live prediction and temporary tips | Saved samples and saved-vector replay only | Native live-state research does not establish a production live renderer. |

A [native RTV4 paint oracle](../conformance/fountain-rtv4-paint-native.json)
replays three V14 tuples, a synthetic control and a complete 24-stamp source
tuple stream through native constructor, upload and Draw code on host Mesa.
The full-stroke 1× crop matches all 64 shader-reference support pixels, with
maximum channel error 6 and active-region mean error 1.4844. Color and matrix
controls remain bounded; natural Drawing admission and device pixels are unverified.

Native [matrix-scale captures](../conformance/fountain-inverse-scale-native.json)
cover changed and cached V14/V16 getters under five controlled canvas matrices;
the full zoom-dependent Drawing/RT pipeline remains unverified.

Admitted Fountain V14/V16 strokes with neither public RGB nor saved ARGB use
native default black through the existing page theme. The [color oracle](../conformance/fountain-saved-color-native.json)
replays two embedded corpus frames and eight separate synthetic Drawing controls;
it does not establish outer SDK loading or final native pixels. Paired 1×
Chromium, Firefox, MuPDF and PDFium renders lower mean error against the host
shader references for one real V14 stroke and one synthetic V16 control;
PDF sampling differences remain.

A separate [native Fountain PDF writer](../conformance/fountain-pdf-writer-native.json)
emits circle paths into a mocked sink. Its connection to public Notes export and
final PDFium output remain unverified; it is not the GL paint reference.

Profile admission and validation are narrower than the names above. See
[stroke rendering](reverse-engineering/stroke-rendering-findings.md),
[fountain parity](reverse-engineering/fountain-parity.md),
[Marker2](reverse-engineering/marker2-rendering-findings.md),
[Marker4 V7](reverse-engineering/marker4-v7.md), and
[Marker4 V8](reverse-engineering/marker4-rendering-findings.md).

A [Crayon2 saved-playback paint oracle](../conformance/crayon2-saved-playback-paint-native.json)
joins native source getters, 285 packed records, queued upload and host Mesa Draw,
with clipped/interior and transparent/opaque controls. That V1 route supplies
renderer size through a host write and uses modeled RNG. A separate
[V2 Drawing oracle](../conformance/crayon2-drawing-admission-native.json)
checks native settings dispatch and Model size/color forwarding through the same
RT, shader uniform and six opaque/partial/zero-alpha readbacks. Persisted-note
admission, production vector paint and device parity remain open.

The [Pencil3 V1 source trace](reverse-engineering/pencil3-source-findings.md)
separates saved samples from generated particles and configured paper coverage.
Pencil profiles currently use the generic pressure approximation. A test-only
Pencil3 V1 sampler matches ten saved native cases and eight boundary witnesses.
The [Drawing caller](../conformance/pencil3-saved-caller-native.json) captures
null/V1 and empty/V3; V1 matches all 48 particles with a modeled bionic RNG.
The test-only [V3 particle kernel](../conformance/pencil3-v3-native.json)
matches all 48 particles in two redraw phases, including private RNG state and
reset behavior. The test-only [V3 saved sampler](../conformance/pencil3-sampler-inputs-native.json)
reconstructs complete native particle streams from source channels across five
profiles, including varied pressure, tilt, tools and context dimensions.
A [V3 host paint join](../conformance/pencil-v3-paint-join.json) replays those
captured streams through the extracted shader and masks at three scales, with
synthetic projection, color and paper phase; native device/FBO paint is unverified.
A [native V3 draw handoff](../conformance/pencil3-draw-handoff-native.json)
captures color, projection, paper scale and texture bindings across five explicit
profiles using mocked renderer resources and context. A [scale-producer replay](../conformance/pencil3-paper-scale-native.json)
checks native `min(screen width,height) / 1440` through task dispatch to the RT
field in four contexts; application paper inputs, paint and device pixels remain
unverified. A [bitmap-consumer replay](../conformance/pencil3-paper-consumer-native.json)
joins the Drawing producer to the queued RT depth-map setter and cached dimensions.
That capture stops at a missing host framebuffer target. A separate
[native paper paint oracle](../conformance/pencil3-paper-paint-native.json)
completes same-RT Mesa Draw with the native paper resource and white/zero controls,
retaining full RGBA readbacks. Bitmap/texture ownership and lifecycle remain host
adapters; production vector paint and device parity remain open.
A [one-particle vector sampling witness](../conformance/pencil-vector-sampling.json)
matches the host shader at three scales with group-scoped SVG `crispEdges`,
including embedded paper. Public PDF export drops that hint; general runtime
paper coverage and production vector paint integration remain outstanding.
The test-only Marker3 V5 sampler checks stamp positions, radii and callback
boundaries in ten [synthetic native cases](../conformance/marker3-saved-native.json).
The [Drawing caller capture](../conformance/marker3-saved-caller-native.json)
checks null selecting V1, empty/explicit settings selecting V5, source ARGB
forwarding and linked XY streams; the V5 sampler excludes null settings.
Its [positive paint capture](../conformance/marker3-paint-positive-native.json)
executes native mask/composite draws on separate complete framebuffer targets
in host Mesa, retaining the full RGBA hash and GL error checks. The
[scale capture](../conformance/marker3-paint-scale-native.json) retains full masks
and RGBA output for 17 linked-path and isolated-stamp cases across sizes 4/8/12
and scales 1/2/4, checking uploaded XY against the saved caller buffers. Points
enter directly at RTV1. A [joined Drawing paint capture](../conformance/marker3-joined-paint-native.json)
retains the Drawing-selected RT and its original 176-byte vector through native
Init/Update/Draw, with a full host-Mesa RGBA readback and 239 zero GL error checks.
The [queue control](../conformance/marker3-queue-handoff-native.json) executes
native `SendDataToGPU` with correct host queue wiring and detects missing
dispatch with the wrong queue; no host vector-slot assignment is needed.
In the joined paint capture, renderer-message and vector-delete
tasks remain deferred; complete queue/lifetime integration, vector edge coverage
and device pixel parity remain outstanding.
DefaultPen's sampler checks
position/radius streams in sixteen synthetic
[cases including degenerate paths](../conformance/defaultpen-saved-native.json).
Its [Drawing caller](../conformance/defaultpen-saved-caller-native.json) verifies
native Model getters, factory selection and identical 132-byte Drawing/saved
streams on the same RT under an explicit 1920×1080 display profile.
A [redraw-boundary capture](../conformance/defaultpen-redraw-boundaries-native.json)
retains all 22 stamps and native history/end-call boundaries for a six-point
stroke; saved replay constructs a native event with source 0.
DefaultPen's [Draw capture](../conformance/defaultpen-draw-native.json) retains
native blend setup and shader text that replaces input alpha with screen-space
coverage. The [joined capture](../conformance/defaultpen-joined-paint-native.json)
passes that Drawing-selected RT’s 132-byte vector through native upload and
draw to a host RGBA8 target, with an explicit same-vector-object handoff.
Admitted saved profiles render the native disks in order; other states retain
the generic approximation. Natural queue/lifecycle setup and native shader edge
coverage remain outstanding.
The [InkPen V4 source trace](reverse-engineering/inkpen-v4-findings.md) establishes
selected saved curve width/sampling and cache boundaries. Canonical-page SVG/PDF
now reconstruct ordinary V1 null-setting geometry for tools 0–4 in both curve
modes, using saved ARGB or the native opaque-black default when it is absent;
a bounded compound contour approximates native coverage at one SVG pixel per
source unit. Replay preserves source-event cutoffs and folds the terminal suffix
into the final event. Unsupported inputs and exhausted budgets retain fallback;
the context-free preparer remains unchanged. Ordinary V1 and V4
(empty or `4;` settings) use saved RGB when the public stroke color is absent,
and apply saved ARGB alpha once per stroke in SVG/PDF.
A test-only numeric core matches twenty-one
configured-context native redraw cases and four setter cases, including both
curve modes, fixed width and a degenerate curve. The [Drawing caller](../conformance/inkpen-saved-caller-native.json)
captures null settings selecting V1 and empty settings selecting V4 under an
explicit 1920×1080 display profile. The V4 numeric model reproduces the complete
312-byte Drawing stream bit for bit, plus all streams in a
[seven-case display matrix](../conformance/inkpen-display-profiles-native.json);
null settings remain outside that V4 model. The separate V1 core matches
all fourteen emitted streams in the [V1 matrix](../conformance/inkpen-v1-saved-native.json),
including no-curve, duplicate-point, timing and display controls; Drawing rejects
the separate one-point case before pen creation. A [nine-case size/tool capture](../conformance/inkpen-admission-native.json)
checks omitted, zero, subminimum and oversized source sizes plus tool codes 0–4.
The test-only model matches every captured callback word, using the saved style
size (missing means zero) and native 0.4–800 clamp rather than the public width.
A [reuse capture](../conformance/inkpen-reuse-native.json) checks short/long first
strokes and a distinct-object control on the same pen, drawable and RT; every
target stream matches the fresh 312 bytes. Direct redraw of the distinct object
returns without callbacks after Drawing clears its cache-dirty flag. Broader
reuse states and vector edge coverage remain open before V4 production admission.
The [property helper oracle](../conformance/object-stroke-curve-native.json)
executes native property read/write and getter/setter controls, confirming
serialized bit 0 selects curve state; full-record deserialization is not exercised.
The [flag audit](../conformance/object-stroke-properties-native.json) checks
137 zero/one-hot/pair masks against named native getters and the Rust parser;
bit 2 is separately exercised through tilt/orientation channel storage.
A [joined paint capture](../conformance/inkpen-joined-paint-native.json) executes
native Renderer factories and RTV1 Init/Update/Draw, checking the 312-byte upload
and full RGBA readbacks. With correct host queue wiring, native `SendDataToGPU`
installs the saved vector and `Update(false)` uploads it without a host vector-slot
write. A [three-color capture](../conformance/inkpen-color-native.json) checks an
unchanged MAX mask and one source-over composite for zero, partial and opaque
alpha under identity host color conversion. A [V1 joined capture](../conformance/inkpen-v1-joined-paint-native.json)
checks null settings, the unchanged 300-byte upload and partial-alpha paint on
the same Drawing-selected RT. The
[scalar derivative check](../conformance/inkpen_coverage_scalar.py) reproduces
all 32,768 bytes of the captured 128×64 MAX mask using fine 2×2 differences.
This is one host Mesa witness, not a universal device derivative contract.
A [V1 scale replay](../conformance/inkpen-v1-scale-native.json) checks fresh
1×/2×/4× native Draw targets and separate identity-transform controls. Its
pixel-dependent edge band prevents treating a fixed 1× contour as a general paint law.
A [V1 reuse oracle](../conformance/inkpen-v1-reuse-native.json) checks source-event
and terminal vector prefixes, short/long preceding strokes and distinct source
objects. Rust matches the complete first/target streams and their native boundaries.
A [17-case V1 boundary matrix](../conformance/inkpen-v1-boundaries-native.json)
checks complete native Drawing/redraw vectors and source-event/terminal prefixes,
including curved tools 0–4 and one no-curve tool1 control; Rust matches every word
and cutoff. These synthetic setter controls do not establish serialized-note admission.
A [serialized-frame replay](../conformance/inkpen-serialized-drawing-native.json)
loads embedded string-table and stroke bytes through native Model attachment
and Drawing. Controlled Fountain-to-InkPen derivatives select V1/null and V4;
V1 defaults absent ARGB to opaque black and emits 68 records with exact event
prefixes. These are synthetic derivatives, not app-captured InkPen notes.
A [ten-case no-curve tool-3/4 capture](../conformance/inkpen-v1-nocurve-tool34-native.json)
checks pressure and size boundaries; Rust matches complete vectors and native
event cutoffs. Paired SVG/PDF exports improve native-referenced error across
1×/2×/4× host comparisons; tool5 and advanced V4 retain generic fallback.
General V4 admission, app display setup, device parity and exact scale-dependent
coverage remain outstanding.
The [InkPen2 V9 saved trace](reverse-engineering/inkpen2-saved-findings.md)
establishes selected time/history radius and derived cache geometry; InkPen2
also uses the generic approximation, separately from native live filtering.
A test-only V9 core matches every RTV3 float in three fresh, type-0 saved
redraw cases. The [Drawing caller capture](../conformance/inkpen2-saved-caller-native.json)
checks null settings selecting V1, empty/explicit settings selecting V9, and a
25-record V9 buffer match. The Rust core checks both Drawing and saved replay
using native body-normalized sampling state. The [reuse oracle](../conformance/inkpen2-reuse-native.json)
and Rust sequence test retain the native alternation byte between strokes:
a shared Drawing retains it across distinct source objects too. A short
preceding stroke changes tool-1 output, while the captured long
history and tool-2 controls match fresh receivers. General caller/runtime
state, shape modes and paint integration remain open. A
[source controls](../conformance/inkpen2-source-caller-native.json) check six
V9 streams and one-point Drawing rejection under an explicit host profile.
The model consumes native getter arrays and selected drawable size, including
the 0.4–800 clamp; these remain synthetic setter controls. The
[RTV3 paint capture](../conformance/inkpen2-paint-native.json) checks native
MAX coverage, source-over color and final composition in six synthetic host
Mesa cases, plus four direct-branch source-over cases. Both include source ARGB
opacity controls. A test-only paint model matches every full-target readback;
direct same-color overlap accumulates while the alternate branch preserves
its MAX result. The [joined capture](../conformance/inkpen2-joined-paint-native.json)
passes one Drawing-selected V9 receiver's unchanged 900-byte stream through
native upload and RGBA paint, with an explicit host canvas/context bridge.
Natural app canvas setup, production vector admission and device parity remain
unverified. A paired radial-gradient experiment preserves native support in
SVG at 1×/2×/4×, but the public vector-only PDF path has active-region RGBA
mean errors of 18.65/12.44/6.64 bytes in MuPDF, with large edge alpha errors
also present in PDFium. Its forms and gradient bounds match the source;
scale-dependent raster sampling remains unresolved. This candidate is not
production-admitted.
The [Calligraphy V1 source trace](reverse-engineering/calligraphy-source-findings.md)
establishes orientation-derived nib geometry and retained drawable state.
Production SVG/PDF rendering now uses its three native mesh lanes for admitted
ordinary saved strokes, preserving source RGB, core alpha gradients and native
draw order without image objects. State follows Standard export bitmap batches;
containers and Formula fallback lists retain their enclosing Drawing state.
The [Drawing caller capture](../conformance/calligraphy-saved-caller-native.json)
checks fresh/reused source conversion and two-stroke native list traversal;
Rust matches complete buffers and retained state. The
[saved-wrapper capture](../conformance/calligraphy-saved-wrapper-native.json)
covers fixed width, raw tool normalization and paired curve flags. The
[native defaults capture](../conformance/object-stroke-defaults-native.json)
checks omitted size and ARGB; the adapter uses effective size 0.4 and opaque
black. The [queued Draw capture](../conformance/calligraphy-draw-native.json) records uploads,
uniforms and blend calls. Unsupported inputs retain the generic approximation;
unknown state stays in fallback until a batch reset. Standalone `prepare_stroke`
also retains that approximation because it has no Drawing lifetime. Object/group
opacity, unsupported model routes, PDF raster edges and device pixel parity
remain open; host captures and vector-only PDFs do not establish full parity.
AirBrush has a test-only sampler matching all stamp bits in three synthetic
native saved-redraw cases; native V2 reproduces V1 records and CPU pattern bytes
at a synthetic SPTextureBitmap UpdateBitmap boundary. The [Drawing caller](../conformance/airbrush-saved-caller-native.json)
selects V1 for null settings and V2 for empty settings. The numeric model matches
both 416-byte streams bit for bit under an explicit 1920×1080 display profile;
the [color oracle](../conformance/airbrush-color-native.json) checks all 256 source
alpha bytes and ten full Drawing routes: native output alpha is
`clamp(trunc(f32(f32(alpha) * 50) / 255), 3, 50)`, preserving RGB.
This includes source alpha zero becoming output alpha three; GPU compositing
is not established by this getter contract. A native
[texture capture](../conformance/airbrush-texture-native.json) now checks Renderer
allocation, nearest/clamped sampling and exact GL upload bytes. An
[instanced Draw capture](../conformance/airbrush-draw-native.json) checks native
buffer uploads, attributes and blend calls with a separately captured stamp
stream injected by the host. A [joined paint capture](../conformance/airbrush-joined-paint-native.json)
checks the Drawing-produced vector through native task-body publication, upload
and full RGBA readback. Link-time shader snapshots and live interface/sampler
queries verify the native shader and nearest/clamped pattern; the target is zero
before Draw. Scheduling and Renderer capabilities are host-supplied; general
admission, vector paint and device parity remain outstanding.
The [BrushPen native oracle](../conformance/brushpen-saved-native.json) captures
default GLV10 selection and 11-float instance records. The separate
[Drawing caller](../conformance/brushpen-saved-caller-native.json) selects GLV10
with an explicitly empty saved setting and captures 26 records under a
1920×1080 context. The numeric model reproduces all 1144 bytes using the native
`1440 / min(display limits)` speed factor; direct-wrapper cases retain the
constructor’s 1440×2560 limits. The [reuse capture](../conformance/brushpen-reuse-native.json)
checks short/long first strokes and a distinct-object control on the same pen,
drawable and RT: all second streams equal the fresh 1144-byte stream. Native
redraw clears retained speed, and the next accepted segment seeds radius.
The observed cache slot is not established as the drawable cache key. A
[texture capture](../conformance/brushpen-texture-native.json) reaches intercepted
native texture/instance uploads, attribute setup, blend mapping and Draw-binding
calls. A test-only source sampler matches
every instance bit in ten synthetic MotionEvent cases and nine
[saved-wrapper style profiles](../conformance/brushpen-saved-wrapper-native.json),
including tools 1–3 and both curve modes.
A [positive paint capture](../conformance/brushpen-paint-positive-native.json)
executes native Draw and shaders on host Mesa with a fixture-linked instance,
a separate output framebuffer and an unused-atlas-cell sensitivity check.
The [same-object capture](../conformance/brushpen-same-run-native.json) checks
26 native-generated records through native upload and host paint, including
a full-stream unused-atlas-cell control. The [handoff capture](../conformance/brushpen-handoff-native.json)
also completes native endPen and callback destruction, then executes the queued
`SendDataToGPU` task. The [queued paint capture](../conformance/brushpen-queue-native.json)
completes native clip setup, upload, draw and deferred vector deletion with a
distinct cleared native canvas on host Mesa. Its style/color and scheduling are
explicit host inputs; general caller admission, vector paint, production
integration and Samsung device parity remain outstanding.
A [Drawing-to-paint capture](../conformance/brushpen-drawing-paint-native.json)
joins synthetic Model input, native queued color/data tasks, the unchanged
1,144-byte upload and same-RT uniform color to a complete host Mesa draw.
It also checks target preclear and deferred deletion; production vector paint remains open.
The [MagicPen Drawing caller](../conformance/magicpen-saved-caller-native.json)
checks null/`1;` selecting V1 and empty/`2;` selecting V2. It retains actual
RTV1 vectors separately from callback arguments for synthetic Model inputs;
the [Drawing-to-paint capture](../conformance/magicpen-drawing-paint-native.json)
joins native color, the unchanged upload and interior partial-alpha paint on
transparent/opaque host Mesa targets, retaining full RGBA buffers. Scratch texture
initialization is host-supplied. Canonical SVG/PDF now reconstruct ordinary
tool-2 NoCurve `1;` sources with explicit size/ARGB, ordinary style codes and
native replay cutoffs, using one compound vector contour and one source-alpha
composite. Omitted stroke/color type fields use the native zero defaults,
verified by a [saved-frame Model check](../conformance/object-stroke-style-defaults-native.json).
Paired public exports
improve native-referenced error in 12 host comparisons across four engines at
1×/2×/4×. The fixed contour approximates scale-dependent shader edges and can
lose minimum-size dots. Unknown effects and budgets retain fallback, and device
parity remains open.
A [NoCurve paint replay](../conformance/magicpen-nocurve-paint-native.json)
retains the same native source stream and full RGBA buffers at 1×/2×/4× on
transparent targets, with separate opaque-background blend controls.
A [paint-control matrix](../conformance/magicpen-paint-controls-native.json)
checks eight scale/phase views plus GLV1/GLV2 and opaque-background controls.
The source-derived shader model matches transparent support and stays within
three RGBA bytes on these host captures; it does not establish export parity.
A [GLV1 source matrix](../conformance/magicpen-glv1-source-native.json)
checks native source getters and complete XYZ streams; the Rust source model
matches all 35 two- to eight-point cases, including duplicate/nearby-event
admission, native width transitions and the 0.4–800 size clamp. A
[NoCurve event matrix](../conformance/magicpen-nocurve-source-events-native.json)
adds 16 controls for admission cutoffs, first-admitted width seeding, terminal
ranges and direct-dot fallback, with exact complete streams. A separate
[curved event capture](../conformance/magicpen-curved-source-events-native.json)
records line/terminal callback ranges for 25 bounded inputs, including rejected
final points, duplicate endpoints, direct dots, size limits and low-factor
terminal pressure controls. Rust matches every
captured XYZ word and callback range. Pinned native loops exclude line endpoints
and include terminal endpoints; terminal arguments retain the raw final input.
General curved sampling and curved production admission remain open.
The [ChineseBrush caller](../conformance/chinesebrush-saved-caller-native.json)
checks seven setting states and captures five four-float records from synthetic
Model input. A [source matrix](../conformance/chinesebrush-source-native.json)
retains complete four-lane streams for 52 two- to eight-event controls across
GLV1/2/3; a shared test-only source-event model matches callback Vector4/scalar
inputs and every packed stream word in these bounded captures.
An [adversarial extension](../conformance/chinesebrush-source-extension-native.json)
adds 24 cases for pressure, size and method-specific speed/width precision;
the model matches all 953 records across the combined 76 cases.
A [reuse matrix](../conformance/chinesebrush-reuse-native.json) checks fresh-equivalent
target streams after short, long and different preludes and version changes,
with explicit host lifecycle adapters. A bounded
[GLV3 paint replay](../conformance/chinesebrush-paint-native.json) joins the
synthetic source stream to native queued VBO/color uploads, texture bytes and a
full host Mesa readback. This does not establish device or export parity;
arbitrary-length sampling and production vector integration remain open.
The [Oblique V4 source trace](reverse-engineering/oblique-v4-source-findings.md)
establishes an integer-size diagonal core and separate fixed-width bounds.
Hash-pinned oracles capture saved traversal, core/fringe triangles and host GLES
coverage/compositing. Production SVG/PDF uses the bounded V4 mesh in canonical
page coordinates for ordinary strokes with color type 0 or absent. Omitted
size/ARGB use the [native defaults](../conformance/oblique_defaults_native.json)
of size 0.4 and opaque black; unsupported settings and Descendant objects retain
the generic approximation.
Source RGB/alpha and max coverage are preserved without image objects.
The [native Draw oracle](../conformance/oblique-draw-native.json) also verifies
explicit ARGB forwarding and executed MAX/source-over blend descriptors, with
synthetic resource adapters and no device-pixel claim. The
[reuse witness](../conformance/oblique-saved-caller-reuse-native.json) checks a
three-point stroke followed by a short stroke on the same drawable/RT; the
second callback stream and geometry match its fresh reference. Standalone
`prepare_stroke` remains approximate because it lacks a canvas context. Fixed
width, alternate stroke modes, arbitrary reuse and device pixel parity remain
open. Paired host exports improve on the pre-integration renderer, but broader
light-background PDF cases show material coverage/color differences at low
raster scales; SVG stays close in those captures. PDF transport remains under
investigation.
Selected V1 now uses the shared RTV1 vector transport in canonical page
coordinates for ordinary tool-0–3 strokes with the same native style defaults,
excluding fixed width and effects. The
[stress](../conformance/oblique-v1-stress-native.json) and
[edge](../conformance/oblique-v1-edge-native.json) oracles verify callback pairs,
return-state boundaries and complete mesh buffers, including short and
duplicate curve-disabled transitions. The [tool oracle](../conformance/oblique-v1-tool1-native.json)
verifies tool 1’s 50-unit distance threshold (5 for other tools), including
exact-threshold callbacks and tool-3 curve/linear meshes. Paired 1×/2× SVG checks against vendor shader
replay are close; vector-only PDFs retain material active-region errors.
PDF triangle fields use rounded 8-bit vector coverage contours within precision
and work limits, retaining the existing calculator field otherwise. Two of seven
paired host cases improve; five preserve the previous PDF bytes. Device parity
and broader contour coverage remain open.
That replay and the host-modeled overlap controls do not establish native
multi-stroke paint, device pixels or complete parity.

## Text and embedded layout

The detailed [text support table](text-vector-support.md) owns the admission
rules and native capture boundaries. These rows summarize it rather than
extending its guarantees.

| Feature | Rust rendering | Evidence and remaining limits |
| --- | --- | --- |
| Body, standalone and supported shape text | Shared measurement/layout; density, margins, gravity, wrapping and paragraph spacing | Selected native origins/wrap kernels verified; all standalone modes and complete native layout are not established. |
| Text fit and overflow | Saved text retained; no typed auto-fit/ellipsis projection | Present auto-fit/ellipsis fields produce extension diagnostics. Native dots/cached cutoff and separate triangle cues are derived output; full fitting and Free/Path semantics remain unverified. See [saved options](reverse-engineering/text-box-findings.md#persisted-auto-fit-and-ellipsis). |
| Fonts and Unicode | Pinned/caller fonts, coverage fallback, bidi and embedded physical-face identities | Native production measurement has bounded pinned-Regular admission; device fallback order and variable-font instances are unsupported or unverified. |
| Complex shaping and emoji | Source-preserving SVG fallback; retained PDF glyph geometry where available | SVG cannot reproduce every retained cluster; drawable kind-4 states and native emoji resource production remain unsupported. See [resource ownership](reverse-engineering/text-draw-identity-findings.md#line-feeds-and-emoji-resources). |
| Spans, headings, lists and links | Supported styles, underline/strike/background bands, vector list markers and gated SVG hyperlinks | Intra-cluster background changes and unsafe object positions are conservative; PDF hyperlink annotations are absent. |
| Composition/suggestion/correction styles | Supported modern composition appearance; ordinary text preserved elsewhere | Legacy/incomplete composition and special suggestion/correction appearance produce diagnostics. |
| Tables | Dense unmerged/merged preparation, vector fills/borders/artwork and measured content | Sparse/invalid grids use diagnosed fallback; rotated/nested preparation and general split-page composition remain incomplete or unverified. |
| Table text clipping | Conservative table-wide scope; native per-run clipping in certified ordinary one-page contexts | Certification is bounded by source, font metrics, geometry and transforms; arbitrary cell-loop/per-run parity is not established. |
| Code blocks | Shared title/body layout, panels, chrome/minimum geometry and continuation handling | Captured supplied-metric geometry is verified separately from complete native split-page composition. |
| Pagination | Saved sections, full-source fallback reflow and bounded native page-range routing | General native repagination, obstacle production and page-bound construction remain outside the verified route. |

## Graphics, paper and composition

| Feature | Rust rendering | Evidence and remaining limits |
| --- | --- | --- |
| Shapes and lines | Basic templates, saved move/line/quadratic/cubic/close and bounded Arc/Oval paths (Model normalization for shapes, Drawing semantics for lines), rotation, solid fills/outlines and embedded text; bounded type-7 linear/radial fill/outline gradients; named positive-width dashes and bounded straight-line arrowheads | Native helper/load captures verify admitted path geometry separately from paired appearance. Gradients require admitted common frames and consumed stops; rectangular/other gradients, type-8 gradient outlines, image fills, specialized templates, connectors, pen simulation and compound styles remain incomplete. Arrowheads require simple solid-color straight lines with admitted coordinates, widths, lengths and separate cleanup regions; curved/short/overlapping contexts remain diagnosed. Style inputs and SDK vector transport are checked separately from paired appearance coverage. See [outline styles](reverse-engineering/shape-style-findings.md#rust-dash-projection), [arrow admission](reverse-engineering/shape-style-findings.md#bounded-rust-arrow-projection), [paint admission](reverse-engineering/shape-fill-findings.md#consequences-for-this-codebase) and [shape/line findings](reverse-engineering/shape-line-findings.md). |
| Images | Media resolution, placement, rotation, original-placement cropping and in-bounds source-pixel crop geometry | Crop-only bitmap resampling remains diagnosed; native filtering, out-of-bounds crops, border/original references, fill transforms, tiling and active nine-patch remain incomplete. Nonrectangular inherited image paths produce diagnostics; the traced native effect route uses a rectangle. See [image findings](reverse-engineering/image-findings.md) and [image-filled vector shapes](reverse-engineering/image-effects-findings.md#ordinary-image-effect-paths-differ-from-image-filled-vector-shapes). |
| GIF image sources | No typed GIF asset admission or animation renderer | Native animation opens the current main file; reader replacement has separate loader/restart gates. See [GIF source findings](reverse-engineering/image-findings.md#native-gif-import-and-current-file-animation). |
| Page paper | Solid color and supported built-in ruled/dotted templates | Image/URI-backed and rotated paper are rejected by [template admission](../crates/sdocx/src/page_background.rs); other templates are retained without drawing. |
| Page custom objects | No typed custom-list decoding or rendering | Separate from layer objects and their diagnostics. Sticky notes can bind nested `.sdocx` source; original page/archive bytes remain required. See [custom source ownership](reverse-engineering/vector-retention-findings.md#page-custom-objects-and-attached-source). |
| Physical layers and visibility | Saved current layer; hidden recognized objects/subtrees omitted | Inactive layers remain structurally available; no captured multilayer appearance comparison. See [layer selection](reverse-engineering/page-layer-selection-findings.md). |
| Object order and containers | Stored order within Base/Top/Masking root passes; selected containers recurse in place | Child render IDs do not reselect root passes. Saved child rotation is not applied again through a parent transform. Full mixed-container pixel parity remains unverified. |
| Root intersection filtering | No native geometric selection filter | Native collector behavior is traced, but comparing stored bounding boxes alone does not implement it. See [selection findings](reverse-engineering/object-selection-findings.md). |
| Light/dark treatment | Shared modes, native lightness reversal, local contrast and alpha-preserving paint | [Native Context-to-Pen dispatch](../conformance/drawing-color-theme-native.json) is verified for three explicit host-selected themes. Android theme discovery is unverified; paper/contrast selection remains export policy. See [themes](render-themes.md). |
| Highlighter blending | Whole Top batch uses Darken on light paper and Lighten on dark paper | Capture-route selection/shader math has static evidence; native Standard list PDF uses Darken. SDK shares its SVG policy with PDF; native pixel parity is not claimed. |
| Tape visibility/reveal | Saved note preference and per-stroke reveal flag retained | Generic pen approximation does not implement native presentation controls or destination attenuation. See [Tape findings](reverse-engineering/stroke-metadata-findings.md#tape-visibility-and-reveal-controls). |

Native [image editing](reverse-engineering/image-edit-source-findings.md) can replace
the main resource independently of its original binding. SPI sources under `media/`
are retained as opaque resources but remain unsupported image inputs;
complete original archive bytes remain a separate carrier. Detailed parsing can
opt into retaining original page sources.

The [SVG composition description](svg-rendering.md) and
[native capture findings](reverse-engineering/capture-composition-findings.md)
give the ordering and export-route boundaries. “Masking” is a pass name; it
does not mean the SDK implements eraser subtraction. Layer alpha-lock/shadow
metadata and runtime object draw-state alpha are not a proven general saved
group-opacity contract.

## Output and verification

| Output | Implemented contract | Limits |
| --- | --- | --- |
| SVG preview/export | Typed Rust elements, validated numeric/path values, escaped content, vector ink and embedded fonts | Supported geometry and text transport do not imply complete native appearance. |
| Replay | Reveals the same generated vector geometry at saved sample boundaries | No pen geometry in the UI; no native live prediction or all-layer replay-state guarantee. |
| Document PDF | Shared scene plus retained selected fonts/glyphs/XY, vector masks/shadings/clips and selectable logical source; embedded image resolver/constructor rejections become export errors | Inherits rendering gaps; source images remain images; deferred image decoding and universal PDF accessibility are not certified. |
| Arbitrary SVG to PDF | Compatibility conversion through the SVG importer | Can reshape text and rasterize imported filter effects; lacks the document exporter's private retained-glyph guarantee. |

The [conformance guide](../conformance/README.md) records fixture identities,
independent native capture boundaries and real-document reference coverage.
The locked corpus contains four document/PDF pairs, not an exhaustive feature
inventory. Synthetic archives establish SDK behavior and malformed-input
handling; they cannot establish Samsung visual equivalence for absent features.
The corpus has no real gradient witness. Parsed Rust SVG and vector PDF
gradient checks verify synthetic transport, separately from native
helper/dispatcher captures and paired Samsung appearance.
The browser displays parser diagnostics separately from Rust's text, embedded-object,
shape/line geometry and fill/outline paint notices for previews and completed
exports. Legacy WASM and debugger SVG methods remain payload-only. See the
[diagnostic transport boundary](reverse-engineering/vector-retention-findings.md#diagnostic-interpretation).
The [real-document inventory](reverse-engineering/rendering-corpus-findings.md)
records feature occurrence separately from native research. At that revision its only
parse warnings were PDF-backed paper in two local research notes. Rust now retains
all ordered signed PDF records, version-dependent rectangles and raw template type;
modern manifest bindings and `media/` source bytes are retained separately. See
[page background findings](reverse-engineering/page-background-findings.md).
PDF-backed paper remains unrendered; retained records and bytes do not certify PDF
validity or native composition.
The [PDF preservation constraints](pdf-paper-preservation.md) connect the
recovered contracts to the current Rust model and verified vector PDF transport;
a test-only SDK graft preserves the inline source graph and scoped vector
resources. A [native source-PDF export probe](../conformance/pdf_paper_pdfium_quiz_source_native.py)
preserves one retained document's image bytes and page geometry while adding a
vector path. A [PNG overlay capture](../conformance/pdf-paper-pdfium-image-export.json)
also executes native PNG decoding and image placement for a synthetic rectangle.
A [Composer branch capture](../conformance/pdf-paper-composer-export.json) verifies
the imported-paper DrawPDF skip and two manual-background extent controls;
its AddPages bridge does not join native PDF serialization.
A [SingleX segmentation capture](../conformance/pdf-paper-single-segments.json)
uses retained quiz dimensions with explicit host text/object queries; full
export and final page appearance remain unverified.
The [body-writer transform capture](../conformance/pdf-paper-body-writer-matrix.json)
observes native note-to-PDF scale and marker rectangles for rotated pages with
two CropBox origins, then calls PDFium matrix helpers separately. The
[image-adapter capture](../conformance/pdf-image-adapter-native.json) joins the
native String-overload constructor/setters to PDFium image insertion and export
for four rotations and shifted origins, with host-supplied placement and PNG.
The [bitmap-overload capture](../conformance/pdf-image-bitmap-native.json) executes
native bitmap methods, adapter and libpng encoding with synthetic pixels,
simulated host GL and pinned host zlib. The separate
[RGBA bitmap capture](../conformance/pdf-image-rgba-native.json) executes native
texture/subbitmap readback through actual host Mesa textures and framebuffers,
then native adapter/libpng encoding. Two synthetic 2×2 controls preserve alpha
and verify rounded, clamped RGB unpremultiplication, including zero alpha;
queue scheduling and zlib remain host bridges. The native raster-batch producer and
Drawing-to-insertion, saved-note placement and production PDF-paper rendering
remain outstanding.

The native [shape style contracts](reverse-engineering/shape-style-findings.md)
and [image effect contracts](reverse-engineering/image-effects-findings.md)
have static evidence for features absent from these documents; they do not add
real-document appearance coverage. See also
[PDF findings](reverse-engineering/pdf-export-findings.md).
The [solid shape paint oracle](../conformance/shape-solid-paint-native.json)
adds bounded native fill/outline raster evidence for an axis-aligned rectangle;
odd-width edge coverage and broader paired appearance remain open.

The [vector retention findings](reverse-engineering/vector-retention-findings.md)
separate source-byte ownership, semantic decoding and drawable output. Physical
payload offsets do not own their bytes, and vector output can still quantize or
omit source geometry. Recovered
[path](reverse-engineering/shape-path-findings.md),
[fill](reverse-engineering/shape-fill-findings.md) and
[connector](reverse-engineering/connector-routing-findings.md) contracts identify
additional typed geometry and resource boundaries without adding SDK support.
[Container edits](reverse-engineering/object-transform-findings.md) and
[eraser cuts](reverse-engineering/eraser-preservation-findings.md) can modify saved
child or stroke geometry before drawing. The
[brush and painting findings](reverse-engineering/brush-record-findings.md)
distinguish compatibility dispatch and editable resources from previews.
The [painting source contract](reverse-engineering/painting-source-findings.md)
identifies a separate archive with ordinary stroke packets, replay channels and
bitmap state. The shared native reducer uses different coordinate widths from
WDoc; neither a thumbnail nor decoding WDoc frames preserves that source.
