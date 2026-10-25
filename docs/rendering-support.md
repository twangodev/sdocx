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
| 3 | Image | Geometry, media references, supported crop fields and fill-effect transparency | Page and text-flow images with quantized fill alpha; native PNG decode/draw and crop-edge clamp controls verified; partial-source alpha rounding and other effects remain partial |
| 4 | Container | Common metadata and ordered children | Nested page objects; supported parent visibility and root selection |
| 7 | Shape | Geometry, styles, saved paths, controls and embedded text | Supported page templates/paths, solid paint and bounded linear/radial fill/outline gradients; partial styling |
| 8 | Line | Geometry, styles, controls and saved paths | Straight lines and supported paths; bounded linear/radial outlines, including descending two-stop linear gradients with partial alpha and radial endpoint projection using the first ten distinct attached stops; partial styling/routing |
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
Saved-page rendering rejects sources with fewer than two points before pen
selection, matching native Drawing. The standalone `prepare_stroke` API retains
its compatibility tap geometry.
A [native stroke-frame capture](../conformance/object-stroke-channels-native.json)
checks a real 204-point frame, three float-channel and six XY delta controls.
Compressed coordinates and float channels round each addition to float32;
outer SDK loading is not exercised.

| Feature | Rust rendering | Evidence and remaining limits |
| --- | --- | --- |
| Fountain V14, saved `14;` | Reconstructed stamps/directions with [saved defaults](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/fountain-v14-defaults-native.json); vector directional shading, colorizer and maximum blending; tools 1–3 and fixed width | Native geometry, real-document comparisons and [host shader captures](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/fountain-v14-field-native.json) a [full saved-stroke replay](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/fountain-v14-real-native.json) and an [original-frame Drawing check](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/fountain-persisted-drawing-native.json). V14 colorizer witnesses render in Chromium and Firefox with vector-only PDF export. Native [expansion parameters](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/fountain-v14-addpoint-native.json) and [synthetic thin-stamp shader checks](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/fountain-v14-fringe-native.json) cover fringe behavior. An explicit `FountainV14OutputScaleProfile` paints scale-aware radial fringes and minimum-width alpha before maximum blending; 180 compact shader tiles and a saved handwriting-cache comparison show lower alpha error. Default output is unchanged. The profile lowers SVG alpha error but worsens 1× MuPDF alpha error on the saved handwriting reference; small-mark mask sampling remains under investigation. Thresholded ink support and native PDF parity remain unresolved. |
| Fountain V16, saved `18;0;100;` | Reconstructed stamps with pressure/tilt/time/movement and fixed width; saved size/tolerance defaults and relative endpoint time; tools 1–3; ordered source-over radial stamps with ARGB alpha per instance at canonical unit inverse scale | Executed native geometry, [saved Drawing defaults](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/fountain-v16-defaults-native.json), [endpoint timing](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/fountain-v16-event-time-native.json), [radial field records](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/fountain-v16-addpoint-native.json) scalar coverage checks and a [persisted native queue replay](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/fountain-saved-render-native.json). Native queue controls with zero, partial and opaque ARGB establish per-instance alpha and source-over accumulation. Pixel-center source-over matches the partial-alpha native field far more closely than MAX; SVG/PDF edge sampling still differs. Full-coverage circular stamps use vector circles in PDF with per-stamp saved ARGB alpha at unit scale; opaque stamps also retain the existing scale support. A 76-stamp opaque persisted AS18 host-Mesa comparison reduces union alpha MAE from 27.44 to 3.76 bytes in MuPDF and 27.52 to 3.14 in PDFium at fixed 96 DPI. At unit scale, the same full-coverage profile uses the native quad bounds in SVG, including partial saved alpha, reducing union alpha MAE from 4.07 to 1.44 bytes in the opaque AS18 control. Other stamps preserve vector gradients/masks. Device/zoom compensation and native pixel parity remain unverified. This setting selects drawing V16, not V18. |
| DefaultPen, ordinary saved tools 0–3 | Native positions/radii rendered as ordered source-over circles; omitted size uses the native 0.4 clamp | Native geometry, replay-boundary and [admission controls](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/defaultpen-admission-native.json); paired vector exports improve on the generic fallback. [Joined host-Mesa captures](reverse-engineering/stroke-rendering-findings.md#defaultpen-curve-enabled-branch) check RGB-only shader alpha behavior and 56 size/phase/rotation controls; vector consumers retain edge-coverage differences. Device pixel parity remains unverified. |
| ChineseBrush V1–V3, saved `1;`–`3;`, explicit source-device dimensions | Reconstructed ordered dabs, native 144×144 atlas, saved RGB and selector opacity; hard-circle coverage or optional output-scale fringe masks | Paired SDK SVG/PDF exports improve host-capture error over generic paint. Explicit uniform-scale fringe masks reduce alpha/color error further but can lose faint edge pixels. Device appearance and dark/invert parity remain unverified; unsupported sources and exceeded budgets retain generic rendering. |
| Marker2 | Sampled circular stamps, native integer size and saved alpha | [Native caller controls](../conformance/marker2-saved-caller-native.json) check full XY streams and size defaults; [host paint controls](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/marker2-zero-paint-native.json) confirm zero-width strokes produce no ink. A [12-case V2 host-Mesa matrix](reverse-engineering/marker2-rendering-findings.md#v2-changes-edge-smoothing-for-thin-strokes) verifies finite 2×2 coverage and post-MAX alpha at widths 1–3; paired SVG/PDF readers retain thin-edge coverage differences. V2 thin-edge shading, partial-alpha output and complete device appearance remain incomplete. |
| Marker4 / StraightHighlighter V7, saved `7;` | Stylus midpoint sampling, saved style size and fractional rounded rectangular tip | Native geometry and [Drawing size/alias controls](../conformance/marker4-saved-caller-native.json); public support remains approximate because filtering/zoom coverage differ. |
| Marker4 / StraightHighlighter V8, saved `8;` | Sampled rounded rectangular tip centered on saved AddPoint positions, native integer size, tool-specific rotation and saved alpha | Executed Drawing size/alias controls and controlled native RTV4-to-Mesa frames verify saved XY through the instance VBO and rotation about the centered quad; filtering/zoom coverage remain approximate. |
| Other pens, unknown names or rejected settings | Generic straight segments with pressure width, polyline or tap circle | No pen-specific native texture, smoothing or width-law guarantee. |
| Rainbow, eraser and straighten effects | Metadata retained; InkPen2 V9 and saved Fountain V17 rainbow have explicit profiles | Bounded AS18 variable-width Fountain inputs retain plain GLV16 geometry by default even with the rainbow flag, matching native factory selection and controlled callback arguments; real archived rainbow and GPU appearance remain unverified. An experimental host profile instead applies GLV17 rainbow paint using an explicit palette, period and offset. An optional declared uniform output scale adjusts SVG fringe coverage; paired SDK SVG exports match native support at 1×, 2× and 4×. Native CPU colors and bounded RTV6/Mesa phase/alpha controls are checked against public SVG/PDF exports. Full-coverage profile stamps use ordered vector circles in PDF, improving color and support agreement over radial shadings in five canonical controls; partial-coverage or over-budget strokes retain gradients. Native initialization, device appearance and arbitrary-scale parity remain open. Other native effect semantics remain unsupported. V8 accepts straighten/fixed-width bits without implementing their separate effect semantics. |
| Live prediction and temporary tips | Saved samples and saved-vector replay only | Native live-state research does not establish a production live renderer. |

A [native RTV4 paint oracle](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/fountain-rtv4-paint-native.json)
replays three V14 tuples, a synthetic control and a complete 24-stamp source
tuple stream through native constructor, upload and Draw code on host Mesa.
The full-stroke 1× crop matches all 64 shader-reference support pixels, with
maximum channel error 6 and active-region mean error 1.4844. Color and matrix
controls remain bounded; natural Drawing admission and device pixels are unverified.

Native [matrix-scale captures](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/fountain-inverse-scale-native.json)
cover changed and cached V14/V16 getters under five controlled canvas matrices;
the full zoom-dependent Drawing/RT pipeline remains unverified.

Admitted Fountain V14/V16 strokes with neither public RGB nor saved ARGB use
native default black through the existing page theme. The [color oracle](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/fountain-saved-color-native.json)
replays two embedded corpus frames and eight separate synthetic Drawing controls;
it does not establish outer SDK loading or final native pixels. Paired 1×
Chromium, Firefox, MuPDF and PDFium renders lower mean error against the host
shader references for one real V14 stroke and one synthetic V16 control;
PDF sampling differences remain.

A separate [native Fountain PDF writer](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/fountain-pdf-writer-native.json)
emits circle paths into a mocked sink. Its connection to public Notes export and
final PDFium output remain unverified; it is not the GL paint reference.

Profile admission and validation are narrower than the names above. See
[stroke rendering](reverse-engineering/stroke-rendering-findings.md),
[fountain parity](reverse-engineering/fountain-parity.md),
[Marker2](reverse-engineering/marker2-rendering-findings.md),
[Marker4 V7](reverse-engineering/marker4-v7.md), and
[Marker4 V8](reverse-engineering/marker4-rendering-findings.md).

A [Crayon2 saved-playback paint oracle](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/crayon2-saved-playback-paint-native.json)
joins native source getters, 285 packed records, queued upload and host Mesa Draw,
with clipped/interior and transparent/opaque controls. That V1 route supplies
renderer size through a host write and uses modeled RNG. A separate
[V2 Drawing oracle](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/crayon2-drawing-admission-native.json)
checks native settings dispatch and Model size/color forwarding through the same
RT, shader uniform and six opaque/partial/zero-alpha readbacks. Persisted-note
admission, production vector paint and device parity remain open.

The [Pencil3 V1 source trace](reverse-engineering/pencil3-source-findings.md)
separates saved samples from generated particles and configured paper coverage.
Pencil profiles default to the generic pressure approximation. An explicit
`Pencil3RenderProfile` admits bounded V3 intrinsic-texture paint for ordinary
tool-2 strokes with empty/`4;` settings, supplied screen/canvas dimensions and
a chosen paper sampler; canvas dimensions must match the page. A test-only
Pencil3 V1 sampler matches ten saved native cases and eight boundary witnesses.
The [Drawing caller](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/pencil3-saved-caller-native.json) captures
null/V1 and empty/V3; V1 matches all 48 particles with a modeled bionic RNG.
The [V3 particle kernel](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/pencil3-v3-native.json)
matches all 48 particles in two redraw phases, including private RNG state and
reset behavior. The [V3 saved sampler](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/pencil3-sampler-inputs-native.json)
reconstructs complete native particle streams from source channels across five
profiles, including varied pressure, tilt, tools and context dimensions.
A [V3 host paint join](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/pencil-v3-paint-join.json) replays those
captured streams through the extracted shader and masks at three scales, with
synthetic projection, color and paper phase; native device/FBO paint is unverified.
A [native V3 draw handoff](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/pencil3-draw-handoff-native.json)
captures color, projection, paper scale and texture bindings across five explicit
profiles using mocked renderer resources and context. A [scale-producer replay](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/pencil3-paper-scale-native.json)
checks native `min(screen width,height) / 1440` through task dispatch to the RT
field in four contexts; application paper inputs, paint and device pixels remain
unverified. A [bitmap-consumer replay](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/pencil3-paper-consumer-native.json)
joins the Drawing producer to the queued RT depth-map setter and cached dimensions.
That capture stops at a missing host framebuffer target. A separate
[native paper paint oracle](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/pencil3-paper-paint-native.json)
completes same-RT Mesa Draw with the native paper resource and white/zero controls,
retaining full RGBA readbacks. Bitmap/texture ownership and lifecycle remain host
adapters; natural context and device parity remain open.
A [one-particle vector sampling witness](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/pencil-vector-sampling.json)
matches the host shader at three scales with group-scoped SVG `crispEdges`,
including embedded paper. That vectorized-texel hint is lost in PDF; the
intrinsic-image route below remains subject to consumer sampling.
Fresh selectors distinguish empty/`4;` (V3) from `3;` (V1). V3 preparation has
source/output budgets and native size clamps. Public SVG/PDF reuse the original
brush and paper textures with vector stamp geometry; these are intrinsic image
assets, not rendered-stroke bitmaps. Matching sampler controls improve on the
generic fallback across five consumers; PDF residuals remain. Supplied-paper
sampling is producer-owned, while built-in paper repeats. Context defaults,
real-document/device validation remain integration gaps. Profile-aware replay
uses source-event particle cutoffs; viewport-aware metadata checks the same
canvas eligibility as paint.
Marker3 V5 page exports use native-sampled compound disks for the captured
tool-2 base-pen profile with explicit saved size/color and ordinary
style. Other profiles retain the approximation. The sampler checks stamp
positions, radii and callback boundaries in ten
[synthetic native cases](../conformance/marker3-saved-native.json).
The [Drawing caller capture](../conformance/marker3-saved-caller-native.json)
checks null selecting V1, empty/explicit settings selecting V5, source ARGB
forwarding and linked XY streams; the V5 sampler excludes null settings.
Its [positive paint capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/marker3-paint-positive-native.json)
executes native mask/composite draws on separate complete framebuffer targets
in host Mesa, retaining the full RGBA hash and GL error checks. The
[scale capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/marker3-paint-scale-native.json) retains full masks
and RGBA output for 17 linked-path and isolated-stamp cases across sizes 4/8/12
and scales 1/2/4, checking uploaded XY against the saved caller buffers. Points
enter directly at RTV1. A [joined Drawing paint capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/marker3-joined-paint-native.json)
retains the Drawing-selected RT and its original 176-byte vector through native
Init/Update/Draw, with a full host-Mesa RGBA readback and 239 zero GL error checks.
The [queue control](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/marker3-queue-handoff-native.json) executes
native `SendDataToGPU` with correct host queue wiring and detects missing
dispatch with the wrong queue; no host vector-slot assignment is needed.
In the joined paint capture, renderer-message and vector-delete
tasks remain deferred; complete queue/lifetime integration, vector edge coverage
and device pixel parity remain outstanding.
Further host controls isolate the edge law to finite 2×2 fragment-quad
derivatives; a continuous-gradient approximation is insufficient. See
[Marker3 coverage](reverse-engineering/stroke-rendering-findings.md#marker3-fragment-coverage).
DefaultPen's sampler checks
position/radius streams in sixteen synthetic
[cases including degenerate paths](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/defaultpen-saved-native.json).
Its [Drawing caller](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/defaultpen-saved-caller-native.json) verifies
native Model getters, factory selection and identical 132-byte Drawing/saved
streams on the same RT under an explicit 1920×1080 display profile.
A [redraw-boundary capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/defaultpen-redraw-boundaries-native.json)
retains all 22 stamps and native history/end-call boundaries for a six-point
stroke; saved replay constructs a native event with source 0.
DefaultPen's [Draw capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/defaultpen-draw-native.json) retains
native blend setup and shader text that replaces input alpha with screen-space
coverage. The [joined capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/defaultpen-joined-paint-native.json)
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
curve modes, fixed width and a degenerate curve. The [Drawing caller](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/inkpen-saved-caller-native.json)
captures null settings selecting V1 and empty settings selecting V4 under an
explicit 1920×1080 display profile. The V4 numeric model reproduces the complete
312-byte Drawing stream bit for bit, plus all streams in a
[seven-case display matrix](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/inkpen-display-profiles-native.json);
null settings remain outside that V4 model. The separate V1 core matches
all fourteen emitted streams in the [V1 matrix](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/inkpen-v1-saved-native.json),
including no-curve, duplicate-point, timing and display controls; Drawing rejects
the separate one-point case before pen creation. A [nine-case size/tool capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/inkpen-admission-native.json)
checks omitted, zero, subminimum and oversized source sizes plus tool codes 0–4.
The test-only model matches every captured callback word, using the saved style
size (missing means zero) and native 0.4–800 clamp rather than the public width.
A [reuse capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/inkpen-reuse-native.json) checks short/long first
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
A [joined paint capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/inkpen-joined-paint-native.json) executes
native Renderer factories and RTV1 Init/Update/Draw, checking the 312-byte upload
and full RGBA readbacks. With correct host queue wiring, native `SendDataToGPU`
installs the saved vector and `Update(false)` uploads it without a host vector-slot
write. A [three-color capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/inkpen-color-native.json) checks an
unchanged MAX mask and one source-over composite for zero, partial and opaque
alpha under identity host color conversion. A [V1 joined capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/inkpen-v1-joined-paint-native.json)
checks null settings, the unchanged 300-byte upload and partial-alpha paint on
the same Drawing-selected RT. The
[scalar derivative check](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/inkpen_coverage_scalar.py) reproduces
all 32,768 bytes of the captured 128×64 MAX mask using fine 2×2 differences.
This is one host Mesa witness, not a universal device derivative contract.
A [V1 scale replay](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/inkpen-v1-scale-native.json) checks fresh
1×/2×/4× native Draw targets and separate identity-transform controls. Its
pixel-dependent edge band prevents treating a fixed 1× contour as a general paint law.
A [V1 reuse oracle](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/inkpen-v1-reuse-native.json) checks source-event
and terminal vector prefixes, short/long preceding strokes and distinct source
objects. Rust matches the complete first/target streams and their native boundaries.
A [17-case V1 boundary matrix](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/inkpen-v1-boundaries-native.json)
checks complete native Drawing/redraw vectors and source-event/terminal prefixes,
including curved tools 0–4 and one no-curve tool1 control; Rust matches every word
and cutoff. These synthetic setter controls do not establish serialized-note admission.
A [serialized-frame replay](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/inkpen-serialized-drawing-native.json)
loads embedded string-table and stroke bytes through native Model attachment
and Drawing. Controlled Fountain-to-InkPen derivatives select V1/null and V4;
V1 defaults absent ARGB to opaque black and emits 68 records with exact event
prefixes. These are synthetic derivatives, not app-captured InkPen notes.
A [ten-case no-curve tool-3/4 capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/inkpen-v1-nocurve-tool34-native.json)
checks pressure and size boundaries; Rust matches complete vectors and native
event cutoffs. Paired SVG/PDF exports improve native-referenced error across
1×/2×/4× host comparisons; tool5 and advanced V4 retain generic fallback.
General V4 admission, app display setup, device parity and exact scale-dependent
coverage remain outstanding.
The [InkPen2 V9 saved trace](reverse-engineering/inkpen2-saved-findings.md)
establishes selected time/history radius and derived cache geometry. An explicit
`RenderOptions::inkpen2_profile` enables bounded V9 vector stamps for ordinary
solid-color tool-2 strokes with `9;`, zero tolerance, and complete saved channels. It selects
fresh per-stroke state and identity inverse scale. `enhanced_antialias` selects
MAX coverage with one saved-alpha composite; false retains per-stamp source-over.
It does not infer the app's display or antialias policy.
Without that option, InkPen2 retains the generic approximation. The V9 core
matches every RTV3 float in three fresh, type-0 saved redraw cases. The [Drawing caller capture](../conformance/inkpen2-saved-caller-native.json)
checks null settings selecting V1, empty/explicit settings selecting V9, and a
25-record V9 buffer match. The Rust core checks both Drawing and saved replay
using native body-normalized sampling state. The [reuse oracle](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/inkpen2-reuse-native.json)
and Rust sequence test retain the native alternation byte between strokes:
a shared Drawing retains it across distinct source objects too. A short
preceding stroke changes tool-1 output, while the captured long
history and tool-2 controls match fresh receivers. General caller/runtime
state, shape modes and paint integration remain open. A
[source controls](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/inkpen2-source-caller-native.json) check six
V9 streams and one-point Drawing rejection under an explicit host profile.
The model consumes native getter arrays and selected drawable size, including
the 0.4–800 clamp; these remain synthetic setter controls. The
[RTV3 paint capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/inkpen2-paint-native.json) checks native
MAX coverage, source-over color and final composition in six synthetic host
Mesa cases, plus four direct-branch source-over cases. Both include source ARGB
opacity controls. A test-only paint model matches every full-target readback;
direct same-color overlap accumulates while the alternate branch preserves
its MAX result. The [joined capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/inkpen2-joined-paint-native.json)
passes one Drawing-selected V9 receiver's unchanged 900-byte stream through
native upload and RGBA paint, with an explicit host canvas/context bridge.
The [R8 mask oracle](../conformance/inkpen2-max-mask-native.json) checks every
mask byte at 1×/2×/4× in six bounded host captures; SVG rasterization and PDF
appearance remain separate validation gaps.
Native global-AA configuration reaches the per-pen setting and queued RT flag.
The solid-color enhanced route uses opaque grayscale fields before MAX blending;
Rust regressions check exact duplicate invariance and pointwise MAX at subpixel
edges, with vector-only PDF export. Intermediate RGBA8 color normalization can
differ from the ideal vector field. Natural app canvas setup
and device defaults remain unverified. For one 25-record native stream at 1×/2×/4×, the explicit profile
improves support and color error over the generic fallback in Chromium, Firefox,
MuPDF and PDFium; a half-alpha 1× control also improves. SVG support IoU is
about 0.992–0.996, while PDF remains about 0.812–0.955. Independent single-stamp
PDF controls retain the radial-shading sampling error even without the
converter’s shape bounds or RGB gradient; gradient transport is not pixel parity.
Nine enhanced-AA controls (1×/2×/4×, alpha 0/128/255) retain the same native VBO.
The enhanced projection improves SVG and partial-alpha PDF error against those
native frames compared with selecting the direct route. Opaque PDF error is
worse at 1×/2× and better at 4×; zero-alpha output is blank and PDFs remain vector-only.
The explicit profile also admits active type-1 rainbow with AA disabled, a
1080-pixel minimum screen dimension, positive distance, nonnegative offset and
an explicit 2–256-color palette. Admission requires unit coverage throughout
and at most 512 records; palette RGB remains separate from theme conversion.
Enhanced-AA gradients and thin coverage retain fallback. SVG uses direct radial
gradients and PDF retains vector shading; native-referenced PDF appearance and
real-document evidence remain open. See the V9 saved trace above.
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
at a synthetic SPTextureBitmap UpdateBitmap boundary. The [Drawing caller](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/airbrush-saved-caller-native.json)
selects V1 for null settings and V2 for empty settings. The numeric model matches
both 416-byte streams bit for bit under an explicit 1920×1080 display profile.
Admitted source-owned tool-2 profiles preserve saved RGB or native omitted-color
black in generic rendering. Native per-stamp alpha is not applied as whole-stroke opacity.
The [color oracle](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/airbrush-color-native.json) checks all 256 source
alpha bytes and ten full Drawing routes: native output alpha is
`clamp(trunc(f32(f32(alpha) * 50) / 255), 3, 50)`, preserving RGB.
This includes source alpha zero becoming output alpha three; GPU compositing
is not established by this getter contract. A native
[texture capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/airbrush-texture-native.json) now checks Renderer
allocation, nearest/clamped sampling and exact GL upload bytes. An
[instanced Draw capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/airbrush-draw-native.json) checks native
buffer uploads, attributes and blend calls with a separately captured stamp
stream injected by the host. A [joined paint capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/airbrush-joined-paint-native.json)
checks the Drawing-produced vector through native task-body publication, upload
and full RGBA readback. Link-time shader snapshots and live interface/sampler
queries verify the native shader and nearest/clamped pattern; the target is zero
before Draw. Scheduling and Renderer capabilities are host-supplied; general
admission, vector paint and device parity remain outstanding.
The [BrushPen native oracle](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/brushpen-saved-native.json) captures
default GLV10 selection and 11-float instance records. The separate
[Drawing caller](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/brushpen-saved-caller-native.json) selects GLV10
with an explicitly empty saved setting and captures 26 records under a
1920×1080 context. The numeric model reproduces all 1144 bytes using the native
`1440 / min(display limits)` speed factor; direct-wrapper cases retain the
constructor’s 1440×2560 limits. The [reuse capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/brushpen-reuse-native.json)
checks short/long first strokes and a distinct-object control on the same pen,
drawable and RT: all second streams equal the fresh 1144-byte stream. Native
redraw clears retained speed, and the next accepted segment seeds radius.
The observed cache slot is not established as the drawable cache key. A
[texture capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/brushpen-texture-native.json) reaches intercepted
native texture/instance uploads, attribute setup, blend mapping and Draw-binding
calls. A test-only source sampler matches
every instance bit in ten synthetic MotionEvent cases and nine
[saved-wrapper style profiles](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/brushpen-saved-wrapper-native.json),
including tools 1–3 and both curve modes.
A [positive paint capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/brushpen-paint-positive-native.json)
executes native Draw and shaders on host Mesa with a fixture-linked instance,
a separate output framebuffer and an unused-atlas-cell sensitivity check.
The [same-object capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/brushpen-same-run-native.json) checks
26 native-generated records through native upload and host paint, including
a full-stream unused-atlas-cell control. The [handoff capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/brushpen-handoff-native.json)
also completes native endPen and callback destruction, then executes the queued
`SendDataToGPU` task. The [queued paint capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/brushpen-queue-native.json)
completes native clip setup, upload, draw and deferred vector deletion with a
distinct cleared native canvas on host Mesa. Its style/color and scheduling are
explicit host inputs; general caller admission, vector paint, production
integration and Samsung device parity remain outstanding.
A [Drawing-to-paint capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/brushpen-drawing-paint-native.json)
joins synthetic Model input, native queued color/data tasks, the unchanged
1,144-byte upload and same-RT uniform color to a complete host Mesa draw.
It also checks target preclear and deferred deletion; production vector paint remains open.
The [MagicPen Drawing caller](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/magicpen-saved-caller-native.json)
checks null/`1;` selecting V1 and empty/`2;` selecting V2. It retains actual
RTV1 vectors separately from callback arguments for synthetic Model inputs;
the [Drawing-to-paint capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/magicpen-drawing-paint-native.json)
joins native color, the unchanged upload and interior partial-alpha paint on
transparent/opaque host Mesa targets, retaining full RGBA buffers. Scratch texture
initialization is host-supplied. Canonical SVG/PDF now reconstruct ordinary
tool-2 curved and NoCurve null/`1;` sources with explicit size, ordinary style codes and
native replay cutoffs, using one compound vector contour and one source-alpha
composite. Omitted ARGB uses native opaque black, verified with
[Drawing defaults controls](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/magicpen-admission-defaults-native.json);
empty advanced settings remain distinct. Omitted stroke/color type fields use the native zero defaults,
verified by a [saved-frame Model check](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/object-stroke-style-defaults-native.json).
Paired public exports
improve native-referenced error in 12 host comparisons across four engines at
1×/2×/4×. The fixed contour now preserves the canonical radial coverage integral,
including positive area for minimum-size dots. Edge controls improve across
Chromium, Firefox, MuPDF and PDFium, but still underfill at 4×; resvg can drop
the smallest dot at 1×. Scale-dependent shader edges and device parity remain
open. Unknown effects and budgets retain fallback.
A [NoCurve paint replay](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/magicpen-nocurve-paint-native.json)
retains the same native source stream and full RGBA buffers at 1×/2×/4× on
transparent targets, with separate opaque-background blend controls.
The [edge replay](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/magicpen-nocurve-edge-paint-native.json) adds
18 captures covering duplicate points, rejected moves, minimum-size dots,
subpixel phase and zero-alpha ink, with exact native uploads and full RGBA buffers.
A [paint-control matrix](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/magicpen-paint-controls-native.json)
checks eight scale/phase views plus GLV1/GLV2 and opaque-background controls.
The source-derived shader model matches transparent support and stays within
three RGBA bytes on these host captures; it does not establish export parity.
A [GLV1 source matrix](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/magicpen-glv1-source-native.json)
checks native source getters and complete XYZ streams; the Rust source model
matches all 35 two- to eight-point cases, including duplicate/nearby-event
admission, native width transitions and the 0.4–800 size clamp. A
[NoCurve event matrix](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/magicpen-nocurve-source-events-native.json)
adds 18 controls for admission cutoffs, first-admitted width seeding, terminal
ranges, direct dots and thick-stroke sampling, with exact complete streams. A separate
[curved event capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/magicpen-curved-source-events-native.json)
records line/terminal callback ranges for 33 bounded inputs, including rejected
final points, duplicate endpoints, direct dots, size limits and low-factor
terminal pressure controls. Rust matches every
captured XYZ word and callback range. Pinned native loops exclude line endpoints
and include terminal endpoints; terminal arguments retain the raw final input.
Curved SVG/PDF admission uses the same bounds and approximate contour. The
[four-profile curved paint replay](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/magicpen-curved-paint-native.json)
supports paired comparisons at 1×/2×/4× in five engines: active-support error
improves over fallback in all 60 rows, while subpixel and zoom coverage still differ.
The [ChineseBrush caller](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/chinesebrush-saved-caller-native.json)
checks seven setting states and captures five four-float records from synthetic
Model input. A [source matrix](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/chinesebrush-source-native.json)
retains complete four-lane streams for 52 two- to eight-event controls across
GLV1/2/3; a shared bounded source-event model matches callback Vector4/scalar
inputs and every packed stream word in these bounded captures.
An authored synthetic SDOCX regression checks note-string resolution and all five
parsed source channels before comparing 33 packed native records.
Parsed GLV1/2/3 fallback colors retain saved RGB or ObjectStroke's omitted-color black;
brush geometry and textured paint remain approximate.
An [adversarial extension](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/chinesebrush-source-extension-native.json)
adds 24 cases for pressure, size and method-specific speed/width precision;
The established Rust matrix matches 1,762 native records across 143 cases, with generated
long/short controls extending coverage through 128 events, including
rejected terminal movements and zero-time speed seeding. Bounded sampling
checks source channels and output budgets; terminal geometry retains the raw
endpoint and pressure while V2/V3 widths retain accepted-move speed.
NoCurve controls retain rejected raw terminal segments, skip duplicate tails,
and place all-rejected dots at the last source point with pressure-scaled width.
Additional Rust controls pin 77 paint-input records at sizes 0.4, 1, 3.144 and
24; 18 versioned controls verify cumulative source-event record boundaries
for curved and NoCurve saved redraws. These cuts do not establish live replay policy.
A [reuse matrix](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/chinesebrush-reuse-native.json) checks fresh-equivalent
target streams after short, long and different preludes and version changes,
with explicit host lifecycle adapters. A bounded
[GLV3 paint replay](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/chinesebrush-paint-native.json) joins the
synthetic source stream to native queued VBO/color uploads, texture bytes and a
full host Mesa readback. These host captures do not establish device pixel parity.
Live program/sampler queries identify the alpha-atlas shader and native nearest/
clamp sampling. Opaque, partial and zero source-alpha controls produce identical
host readbacks. Production paint requires explicit source-device dimensions.
`ChineseBrushFwidthOutputProfile::pixels_per_page_unit` separately opts into
64-sector fringe masks at a declared uniform output scale; the default keeps
hard-circle coverage. Public SDK exports reduce alpha error in 116 of 117 paired
host-capture comparisons across resvg, MuPDF and PDFium, with one tie, but lose
some faint edge support. Identical-radius masks are shared and output budgets
are checked before emission. Texture minification, arbitrary output affines and
device pixel parity remain unverified. Three captured output transforms also show
that an analytic radial derivative differs from native finite helper-lane
derivatives before SVG/PDF rasterization; the masks remain an approximation.
A thin host-replay diagnostic also identifies nearest-atlas boundary sensitivity
to tiny UV interpolation differences, separate from the fringe approximation.
The [Oblique V4 source trace](reverse-engineering/oblique-v4-source-findings.md)
establishes an integer-size diagonal core and separate fixed-width bounds.
Hash-pinned oracles capture saved traversal, core/fringe triangles and host GLES
coverage/compositing. Production SVG/PDF uses the bounded V4 mesh in canonical
page coordinates for ordinary strokes with color type 0 or absent. Omitted
size/ARGB use the [native defaults](../conformance/oblique_defaults_native.json)
of size 0.4 and opaque black; unsupported settings and Descendant objects retain
the generic approximation.
Source RGB/alpha and max coverage are preserved without image objects.
The [native Draw oracle](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/oblique-draw-native.json) also verifies
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
| Fonts and Unicode | Pinned/caller fonts, coverage fallback including indivisible CJK graphemes across supplied faces, bidi and embedded physical-face identities; supplied SVG-table color glyphs preserve vector paint and source text in PDF | Plain/gzip SVG, currentColor and square/non-square viewport transport have Rust regressions and paired MuPDF checks. Native production measurement has bounded pinned-Regular admission; bundled CJK/emoji coverage, device fallback order and variable-font instances remain incomplete or unverified. |
| Flow italics | Native-name-selected Regular face with synthetic shear; generic fallback when native name resolution is unavailable | Native style captures and one paired export verify the face policy; PDF and eligible SVG text use native −0.25 shear. Positioned mixed-style fragments preserve source order and neighboring positions. Eligible non-bold combining clusters use per-glyph SVG outlines with independent origins and retained source text; unsupported mixed positioning, fonts and styles retain fallback. Rust geometry/tagging and transformed-clip tests cover transport; complete font/device parity remains unverified. |
| Complex shaping and emoji | Source-preserving SVG fallback; retained PDF glyph geometry where available | SVG cannot reproduce every retained cluster; drawable kind-4 states and native emoji resource production remain unsupported. See [resource ownership](reverse-engineering/text-draw-identity-findings.md#line-feeds-and-emoji-resources). |
| Spans, headings, lists and links | Supported styles, underline/strike/background bands, vector list markers including empty bullet paragraphs, and gated SVG hyperlinks | Native conversion confirms empty-bullet visibility; Mobile controls distinguish body/cell sizing and large-font growth. Complete marker appearance remains unverified. Intra-cluster background changes and unsafe object positions are conservative; PDF hyperlink annotations are absent. |
| Composition/suggestion/correction styles | Supported modern composition appearance; ordinary text preserved elsewhere | Legacy/incomplete composition and special suggestion/correction appearance produce diagnostics. |
| Tables | Dense unmerged/merged preparation, vector fills/borders/artwork and measured content | Sparse/invalid grids use diagnosed fallback; rotated/nested preparation and general split-page composition remain incomplete or unverified. |
| Table text clipping | Conservative table-wide scope; native per-run clipping in certified ordinary one-page contexts | Certification is bounded by source, font metrics, geometry and transforms; arbitrary cell-loop/per-run parity is not established. |
| Code blocks | Shared title/body layout, panels, chrome/minimum geometry and continuation handling | Captured supplied-metric geometry is verified separately from complete native split-page composition. |
| Pagination | Saved inspection sections, full-source fallback reflow, single-page continuous bodies and bounded native page-range routing | SVG, replay and PDF use the full body when its inspection node is source-owned; quiz’s image at UTF-16 112 is retained. Inspection data still selects 0..36. General native repagination, obstacle production and page-bound construction remain outside the verified route. |

## Graphics, paper and composition

| Feature | Rust rendering | Evidence and remaining limits |
| --- | --- | --- |
| Shapes and lines | Basic templates, saved move/line/quadratic/cubic/close and bounded Arc/Oval paths (Model normalization for shapes, Drawing semantics for lines), rotation, solid fills/outlines and embedded text; bounded type-7 linear/radial fill/outline gradients and simple type-8 linear/radial outlines with bounded linear and radial line-paint metadata rotation; named positive-width dashes and bounded straight-line arrowheads | Native helper/load captures verify admitted path geometry separately from paired appearance. Gradients require admitted common frames and consumed stops. Rotatable linear/radial line paint follows metadata rotation without transforming saved body geometry; radial V4 pins the off-center center/radius and cache, with its native 0°/30° pair changing RGB while alpha stays fixed; descending two-stop cache steps preserve partial-alpha stop colors, with four synthetic `.75`/`.5` controls confirming the same 128/128 split across alpha/RGB variants, plus native bucket-zero and off-grid controls. Paired synthetic 0°/30° captures verify geometry and visible paint change; SVG/PDF sampling residuals remain. Two-stop type-8 radial-line cache steps preserve captured stop alpha and RGB, including cache bucket 0, zero-alpha and nonprimary controls. Synthetic source-grid SVG matches V35–V37 and opaque V41 exactly, and V38/V42 within 1 LSB; vector PDFs retain measured MuPDF color residuals. The corpus still has no saved-gradient witness. Rectangular/other gradients, other type-8 styles and saved radial-gradient appearance, image fills, specialized templates, connectors, pen simulation and compound styles remain incomplete. Arrowheads require simple solid-color straight lines with admitted coordinates, widths, lengths and separate cleanup regions; curved/short/overlapping contexts remain diagnosed. Style inputs and SDK vector transport are checked separately from paired appearance coverage. See [outline styles](reverse-engineering/shape-style-findings.md#rust-dash-projection), [arrow admission](reverse-engineering/shape-style-findings.md#bounded-rust-arrow-projection), [paint admission](reverse-engineering/shape-fill-findings.md#consequences-for-this-codebase) and [shape/line findings](reverse-engineering/shape-line-findings.md). |
| Images | Media resolution, placement, rotation, original-placement cropping and in-bounds source-pixel crop geometry; exact source-pixel crop for simple unrotated RGB8/RGBA8 PNGs, preserving sBIT/sRGB metadata and using displayed bounds even when original placement is stored; field 17 original bounds use native integer conversion | The native nonuniform-alpha upscale control confirms crop-edge-clamped sampling; exact source-row copying improves the paired crop-edge result. The PDF pair reduced MuPDF alpha error from 1,006 to 137 (maximum error 31 to 1); PDFium remained at 203. The real fixture 03 crop reduces page-3 changed pixels from 0.0822% to 0.0118% and crop-interior RGB error from 3.33 to 1.92 bytes; its other pages are unchanged. This does not establish full sampler or rounding parity. Other image inputs retain the geometry path; native filtering, out-of-bounds crops, border/original references, fill transforms, tiling and active nine-patch remain incomplete. Nonrectangular inherited image paths produce diagnostics; the traced native effect route uses a rectangle. See [image findings](reverse-engineering/image-findings.md) and [image-filled vector shapes](reverse-engineering/image-effects-findings.md#ordinary-image-effect-paths-differ-from-image-filled-vector-shapes). |
| GIF image sources | No typed GIF asset admission or animation renderer | Native animation opens the current main file; reader replacement has separate loader/restart gates. See [GIF source findings](reverse-engineering/image-findings.md#native-gif-import-and-current-file-animation). |
| Page paper | Solid color and supported built-in ruled/dotted templates | Image/URI-backed and rotated paper are rejected by [template admission](../crates/sdocx/src/page_background.rs); other templates are retained without drawing. |
| Page custom objects | No typed custom-list decoding or rendering | Separate from layer objects and their diagnostics. Sticky notes can bind nested `.sdocx` source; original page/archive bytes remain required. See [custom source ownership](reverse-engineering/vector-retention-findings.md#page-custom-objects-and-attached-source). |
| Physical layers and visibility | Saved current layer; hidden recognized objects/subtrees omitted | Inactive layers remain structurally available; no captured multilayer appearance comparison. See [layer selection](reverse-engineering/page-layer-selection-findings.md). |
| Object order and containers | Stored order within Base/Top/Masking root passes; selected containers recurse in place | Child render IDs do not reselect root passes. Saved child rotation is not applied again through a parent transform. Full mixed-container pixel parity remains unverified. |
| Root intersection filtering | No native geometric selection filter | Native collector behavior is traced, but comparing stored bounding boxes alone does not implement it. See [selection findings](reverse-engineering/object-selection-findings.md). |
| Light/dark treatment | Shared modes, native lightness reversal, local contrast and alpha-preserving paint | [Native Context-to-Pen dispatch](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/drawing-color-theme-native.json) is verified for three explicit host-selected themes. Android theme discovery is unverified; paper/contrast selection remains export policy. See [themes](render-themes.md). |
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
| Document PDF | Shared scene plus retained selected fonts/glyphs/XY, vector masks/shadings/clips and selectable logical source with authoritative outer PDF text replacements; embedded image resolver/constructor rejections become export errors | Inherits rendering gaps; source images remain images; deferred image decoding and universal PDF accessibility are not certified. |
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
PDF export now preserves source PDF paper for a bounded continuous-note class:
one authoritative source PDF with contiguous, top-aligned, full-width integer
paper records and consecutive source-page indices. Source CropBox origins and
quarter-turn rotations are preserved; UserUnit must be 1. The note must use the
native-default background and supported saved unrotated strokes.
Native-derived query bands select whole strokes;
source pages retain their physical boxes, while continuation pages use the
native 600-point width and integer-truncated height, independently of SDK DPI.
An explicit `PdfOptions::pdf_paper_fixed_axis_size` normalizes saved page/stroke
geometry using native f32 rules; its default preserves the stored coordinate
profile. Body coordinates and source PDF boxes stay in their own domains.
Empty or LF-only body text with supported formatting is accepted, including
foreground/font-size spans, finite margins and valid saved text sections;
bullets remain unsupported. LF-separated resolved, unrotated block images are
also accepted when the page and body flow axes agree; every image must have a
unique source anchor and measured owner. Body-aware page cuts use
the same cached layout as painting. The real quiz exports seven pages with an
explicit 1080 load width. MuPDF and PDFium render every page; its image pixels
are preserved and its page-5 bounds match native model placement within
0.00002 points. Two-image and blank-gap controls match native line geometry and
queries; multi-image SDK export has synthetic coverage. This is model/transport
validation, not native pixel parity.
A saved stroke's generated flag alone does not exclude it. Other visible text,
image transforms, multiple source PDFs, gaps between records and unverified
cases still fail explicitly. SVG previews continue to report omitted
PDF backgrounds; converting those diagnostic-aware pages to PDF fails explicitly.
The [PDF preservation constraints](pdf-paper-preservation.md) connect the
recovered contracts to the Rust model and verified vector PDF transport.
Rust assembly tests cover scoped resources, overlay tags, reordered/duplicate
pages and ordinary tails; public export tests cover source preservation,
cross-band whole-stroke admission and DPI-independent physical page sizes.
The real `cs61bl_su22` note exports 20 pages: both MuPDF and PDFium render them,
and source page boxes, text, font streams and image bytes survive. This verifies
SDK transport; a connected native multi-record export comparison remains open.
Real-source and retained-text transport match MuPDF/PDFium at 1×/2×. The stroke
selector matches native identities and order across all 11 observed bands of
one quiz archive loaded at width 1812: 3,228 unique strokes and 3,261 admissions,
including cross-band repeats. This verifies query admission in that loaded
state, not exported appearance. General native body layout and placement remain
open beyond the bounded LF-plus-image route.
A [native source-PDF export probe](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/pdf_paper_pdfium_quiz_source_native.py)
preserves one retained document's image bytes and page geometry while adding a
vector path. A [PNG overlay capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/pdf-paper-pdfium-image-export.json)
also executes native PNG decoding and image placement for a synthetic rectangle.
A [Composer branch capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/pdf-paper-composer-export.json) verifies
the imported-paper DrawPDF skip and two manual-background extent controls;
its AddPages bridge does not join native PDF serialization.
A later archive-backed SingleX capture loads and imports the quiz's actual
cached PDF, then stops at FountainPen loading before completed output.
A separate native loader control verifies FountainPen initialization and factory
construction; the joined archive export remains unverified.
A [SingleX segmentation capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/pdf-paper-single-segments.json)
uses retained quiz dimensions with explicit host text/object queries; full
export and final page appearance remain unverified.
The [body-writer transform capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/pdf-paper-body-writer-matrix.json)
observes native note-to-PDF scale and marker rectangles for rotated pages with
two CropBox origins, then calls PDFium matrix helpers separately. The
[image-adapter capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/pdf-image-adapter-native.json) joins the
native String-overload constructor/setters to PDFium image insertion and export
for four rotations and shifted origins, with host-supplied placement and PNG.
The [bitmap-overload capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/pdf-image-bitmap-native.json) executes
native bitmap methods, adapter and libpng encoding with synthetic pixels,
simulated host GL and pinned host zlib. The separate
[RGBA bitmap capture](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/pdf-image-rgba-native.json) executes native
texture/subbitmap readback through actual host Mesa textures and framebuffers,
then native adapter/libpng encoding. Two synthetic 2×2 controls preserve alpha
and verify rounded, clamped RGB unpremultiplication, including zero alpha;
queue scheduling and zlib remain host bridges. An archive-backed Standard probe
also completes a [native plain-bitmap draw and deactivation](reverse-engineering/fountain-rasterization.md#presenter-copy-replaces-transparent-pixels-too)
with host EGL/GLES adapters; complete PDF output remains unverified. The native raster-batch producer and
Drawing-to-insertion and general saved-note/body placement remain outstanding;
the production PDF-paper subset above does not establish those contracts.

The native [shape style contracts](reverse-engineering/shape-style-findings.md)
and [image effect contracts](reverse-engineering/image-effects-findings.md)
have static evidence for features absent from these documents; they do not add
real-document appearance coverage. See also
[PDF findings](reverse-engineering/pdf-export-findings.md).
The [solid shape paint oracle](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/shape-solid-paint-native.json)
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
