# Saved shape paths and native curve construction

## Evidence boundary

These contracts come from Samsung Notes 4.4.45.37 arm64 disassembly,
the decompiled SDK `SpenPath` class, and the bounded native execution
described below. Addresses are
ELF virtual addresses, not file offsets. The libraries are the APK copies in
`scratch/apk-analysis-native/arm64-v8a/`.

| Library | SHA-256 |
| --- | --- |
| `libSPenModel.so` | `4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a` |
| `libSPenDrawing.so` | `788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd` |
| `libSPenGraphics.so` | `aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621` |
| `libSPenObjectControl.so` | `3211b70ad105e285b57aaa085e2bcd543f197238d702f9233aedea1c7caa1aea` |
| `libSPenComposer.so` | `52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f` |
| `libSPenSkia.so` | `42636cb9ac06cc286114b42c1b9d8f4b78d33761843251cde2b443b101ffb88d` |
| `libSPenBase.so` | `e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb` |
| `libc++_shared.so` | `4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4` |

Serialization, SDK entry points, Drawing routes and export failure propagation
were inspected statically.
Native execution covers Skia paths, Model arc/normalization helpers, named Arc
generation and common co-edit wrappers with actual native Path storage below.
Saved binary loading, application selection of co-edit context and paired
Samsung appearance remain unexecuted.
The [shape and line findings](shape-line-findings.md) describe enclosing
frames and the rotated saved path; [outline findings](shape-style-findings.md)
describe paint, dashes and arrows. This document concerns command identity,
precision, contours and curve construction.

## A double-coordinate wire format with a float-coordinate model

`Path::GetBinary` (Model `0x2efbb8`) writes a four-byte count followed by
one-byte verbs. Document-type values 0 and 1 select float coordinates;
unsigned values greater than 1 select double coordinates
(`0x2efbf4–0x2efc14`). Shape `GetOwnBinary` explicitly passes document type 2
both to `GetBinarySize` and to `GetBinary`
(`0x3a8e6c–0x3a8e8c`). This is the WDoc branch used by saved shapes, not
an assumption that every native serialization uses doubles.

The runtime segment stride is 28 bytes: a four-byte verb followed by six
`f32` slots. Those slots correspond to Java `Segment.type`, `x`, `y`, `x1`,
`y1`, `x2`, `y2`. Quadratic endpoints occupy the last pair, leaving the
middle pair unused. The compact wire format omits those unused slots.

| Verb | Java/native operation | WDoc values after verb | Runtime float byte offsets | Total WDoc command bytes |
| --- | --- | --- | --- | --- |
| 1 | `MoveTo` | `x, y` | 4, 8 | 17 |
| 2 | `LineTo` | `x, y` | 4, 8 | 17 |
| 3 | `QuadTo` | control `x, y`, endpoint `x2, y2` | 4, 8, 20, 24 | 33 |
| 4 | `CubicTo` | first control, second control, endpoint | 4, 8, 12, 16, 20, 24 | 49 |
| 5 | `ArcTo` | left, top, right, bottom, start angle, sweep angle | 4, 8, 12, 16, 20, 24 | 49 |
| 6 | `Close` | None | None | 1 |
| 7 | `AddOval` | left, top, right, bottom | 4, 8, 12, 16 | 33 |

The constructors establish these identities at Model `0x2ef724` (move),
`0x2ef688` (line), `0x2ef864` (quadratic), `0x2ef7c0` (cubic), `0x2ef904`
(arc), `0x2ef9a8` (close), and `0x2efa40` (oval). There is no rational-conic
weight in any of these seven saved commands.

The WDoc writer widens each runtime float to double before writing it
(`0x2efe74–0x2eff94`). The WDoc loader `Path::ApplyBinary` (`0x2effe8`)
reads doubles in pairs and narrows them with `fcvtn v0.2s, v0.2d`
(`0x2f03d0–0x2f04fc`). Consequently a document created through this native
writer normally contains exact widenings of runtime `f32` values. Arbitrary
saved `f64` coordinates are nevertheless narrowed when Samsung loads them.
Retaining their original bytes and reproducing that runtime narrowing are
different preservation contracts.

Neither the writer nor the inspected loader has a finite-coordinate check
in these branches. The loader checks buffer availability, but unknown verbs
consume only their one-byte verb and do not reject the path at its dispatch
(`0x2f0444–0x2f0500`). This native behavior does not make unknown future
payload widths knowable. Rust's bounded opaque retention remains distinct
from guessing unknown coordinates or reproducing unsafe native parsing.

## The native drawing consumer

`ShapeDrawingCommon::convertToPath` (Drawing `0x9bd24`) reads those runtime
segments with the same 28-byte stride. Move, line, quadratic, cubic and close
are forwarded directly. Quadratic reads offsets 4/8 and 20/24
(`0x9bd90–0x9bda4`), confirming the compact saved ordering.

Arc copies the first four floats to a rectangle and passes the last two as
start/sweep angles to `Spen_SkPath::arcTo(rect, start, sweep, true)`
(`0x9bd54–0x9bd74`). Its `true` argument starts a new contour at the arc's
first point even when the preceding command has a current endpoint. There
is no implicit connector line from that preceding endpoint. It also does
not automatically close the arc.

Oval calls `Spen_SkPath::addOval(rect, Direction(1))`
(`0x9bdb8–0x9bdd4`). The bundled implementation starts at right-center and
moves downward first for this value, making it clockwise in y-down note
coordinates. It appends a closed contour, rather than an endpoint arc that
continues the previous contour.

Java `SpenPath.copyToAndroidPath` is a separate consumer. Its arc case calls
Android's three-argument `Path.arcTo(rect, start, sweep)`; it does not pass
native drawing's explicit force-move value. Its oval case names
`Path.Direction.CW`. Agreement of saved commands does not establish
identical contour linking between these Java and native drawing routes.
The app's `office.base.SpenToBitmap` conversion does explicitly pass
`true` in its Android arc case, matching the native contour-start choice;
it remains a separate consumer rather than proof of how arc records are
created.

## The bundled oval is eight ordinary quadratics

The APK's `Spen_SkPath::addOval` (Skia `0x1d6204`) calls `quadTo` eight
times, then appends close (`0x1d6380–0x1d6560`). It does not call `conicTo`.
This remains true even though the same library exports a separate
`conicTo` function at `0x1d53d4`. Assuming modern upstream Skia's ellipse
representation would describe a different implementation.

For ordered finite bounds `(l, t, r, b)`, let the following quantities be
computed in the native `f32` operation order:

- `rx = (r - l) * 0.5`, `ry = (b - t) * 0.5`.
- `cx = (l + r) * 0.5`, `cy = (t + b) * 0.5`.
- `d = f32_bits(0x3f3504f3) = 0.7071067690849304`.
- `k = f32_bits(0x3ed413cd) = 0.4142135679721832`.

The emitted clockwise contour is:

| Segment | Quadratic control | Endpoint |
| --- | --- | --- |
| Start | — | `(r, cy)` |
| 1 | `(r, cy + ry*k)` | `(cx + rx*d, cy + ry*d)` |
| 2 | `(cx + rx*k, b)` | `(cx, b)` |
| 3 | `(cx - rx*k, b)` | `(cx - rx*d, cy + ry*d)` |
| 4 | `(l, cy + ry*k)` | `(l, cy)` |
| 5 | `(l, cy - ry*k)` | `(cx - rx*d, cy - ry*d)` |
| 6 | `(cx - rx*k, t)` | `(cx, t)` |
| 7 | `(cx + rx*k, t)` | `(cx + rx*d, cy - ry*d)` |
| 8 | `(r, cy - ry*k)` | `(r, cy)` |

Every multiplication and addition above rounds as `f32` in the inspected
instructions (`0x1d632c–0x1d6518`). A native initial move is added even if
there was already a contour. The opposite direction branch reverses the
vertical traversal (`0x1d6380–0x1d6440`); shape drawing specifically selects
the clockwise branch.

An SVG `<ellipse>` or exact SVG elliptical arc can preserve vector output
while changing this approximate quadratic contour. The native route can be
expressed with ordinary typed `Move`, eight `Quadratic`, and `Close`
commands; it requires no rasterization and no new conic command.

## Rectangle arcs use a quadratic arc builder

`Spen_SkPath::arcTo(rect, start, sweep, forceMove)` (Skia `0x1d6974`)
rejects negative width or height before constructing points
(`0x1d6994–0x1d69b8`). It obtains a sequence from internal helper
`0x1d6ab0`. It then emits an initial move for an empty path or `forceMove`,
and otherwise emits a line to that start point. Remaining point pairs are
ordinary `quadTo(control, endpoint)` calls
(`0x1d69f0–0x1d6a80`); no conic weights are forwarded.

The helper multiplies degree angles by
`f32_bits(0x3c8efa35) = 0.01745329238474369`, then calls `sincosf` for start
and for `f32(start + sweep)` (`0x1d6b34–0x1d6ba0`). Each unit-vector
component whose absolute value is at most
`f32_bits(0x39800000) = 1/4096` is snapped to positive zero. This affects
axis-adjacent endpoints and branch selection before curve construction.

If start/end snapped vectors coincide while `359 < abs(sweep) < 360`, the
helper repeatedly subtracts a signed `1/512` radians from the ending angle
until the snapped vectors differ (`0x1d6bcc–0x1d6c60`). This is a native
near-full-circle workaround, not a general SVG large-arc rule. The inspected
branches do not justify replacing arbitrary multiple-turn sweep values with
repeated revolutions.

`SkBuildQuadArc` (`0x1bb294`) receives those vectors, direction selected
from the sign of sweep, and the rectangle scale/translation matrix
(`0x1d6c64–0x1d6d1c`). Its initial unit-arc table at `0xa4ebc` contains 17
points for eight quadratics around the full circle. The first quarter is:

| Point role | Coordinate |
| --- | --- |
| Start | `(1, 0)` |
| First control | `(1, 0.4142135679721832)` |
| First endpoint | `(0.7071070075035095, 0.7071070075035095)` |
| Second control | `(0.4142135679721832, 1)` |
| Second endpoint | `(0, 1)` |

The remaining quarters follow sign/reflection symmetry. Notably the
**arc table's diagonal value differs from the oval immediate**. Sharing one
constant for both would erase a native distinction.

The builder selects an octant using the dot/cross product of start/end
vectors, copies complete preceding quadratic pieces, and solves a scalar
quadratic with `SkFindUnitQuadRoots` for the final partial piece
(`0x1bb2d4–0x1bb46c`). It forms that partial control using fused multiply-add
(`0x1bb434`). Matrix concatenation and mapping rotate the unit arc to the
start vector and scale/translate it into the rectangle
(`0x1bb470–0x1bb528`). These paths do not use an SVG endpoint-arc
interpretation of the six saved numbers.

Degenerate cases stay observable: a zero sweep with start exactly 0 or 360
returns one right-center point (`0x1d6ad8–0x1d6afc`, `0x1d6d24`). A rectangle
whose width and height are both zero returns its single point
(`0x1d6b00–0x1d6b30`). The caller still emits the initial move. Detailed non-finite behavior is not established here.

## Executed Skia geometry boundary

A temporary Rust Unicorn capture executed the unchanged native Skia path
constructor (`0x1d383c`), oval/arc operations, point/verb counts, raw iterator
constructor/set/next (`0x1d855c`, `0x1d858c`, `0x1d41ac`), and destructor
(`0x1d3988`). All reached internal geometry and path-storage functions ran.
A separate six-case set directly executed `SkBuildQuadArc` with supplied
unit vectors and optional matrices. These tests do not run the Model
normalizer, Drawing's object producer, saved-path serialization, paint,
SVG output, GPU work or pixel rendering.

The 31 path cases and six direct builder cases agreed across fresh native
mappings and five object/stack/heap fills `[0, 85, 165, 255, 0]`, including a
repeated zero run. An independent binary rerun produced identical JSON.
The temporary output `/tmp/sdocx-native-shape-paths.json` has SHA-256
`2202ab5ebd76016856d4ea0b4777cafa9531564c8f7191b92a4d18a43313cd9b`;
it is not an SDK parity fixture or a committed runtime implementation.

| Executed input | Native result |
| --- | --- |
| Ordered oval, direction 1 or 2 | Move, eight quadratics, close; 17 stored points and 10 path verbs. |
| Point, zero-width, zero-height or inverted oval bounds | Still emits the same verb structure; no upfront empty/inverted guard. |
| Existing move/line followed by oval | Starts an additional closed contour. |
| Existing move/line followed by arc, force-move false | Adds a line to the arc start before its quadratics. |
| Same arc with force-move true | Adds a new move at the arc start. |
| Arc start 0, sweep exactly `+360`, `-360`, `+720` or `-720` | One initial point and move. |
| Arc start 37 with `+360`/`+720`, or start 1/−37 with `-360` | Eight quadratics and 17 stored points under the captured host math route. |
| Arc with inverted bounds | No added contour. |

Host replacements were bounded allocation/memory operations plus Linux
`libm`'s `sincosf`. That library has SHA-256
`6d567d53e895273ca14a1f9dc164fc6c8d39aed2f60aa46a733c2784228915f3`.
It is not Samsung's Android math library, so angle-to-vector bit equivalence
is not certified. Oval and direct vector-builder cases do not call
transcendental imports. Full-turn handling depends on the resulting vectors and their cross-product
sign; it cannot be generalized to an unconditional full oval or unconditional
move-only collapse. These results describe the direct Skia route, not a
saved shape after Model's positive-360 normalization.

## Supplied paths are normalized before template rotation

The common template path loader (Model `0x20bad0`) preprocesses supplied
arc and oval commands before installing its paths. It counts verbs for which
`verb | 2 == 7`, identifying precisely 5 and 7 (`0x20bc18–0x20bc24`),
then expands them through the separate Model point builder `0x211814`.
Oval becomes a move, quadratic pieces and close
(`0x20c118–0x20c184`). Arc becomes quadratic pieces; an initial move is added
unless its start matches the preceding supported segment endpoint
(`0x20c0ac–0x20c114`, `0x20c188–0x20c1c4`). For a preceding move or line,
each `f32` absolute coordinate difference is widened to double and must be
strictly less than the `f64` constant `0.0005` at `0x12d490`; a match rewrites
the preceding point to the arc start
(`0x20bf60–0x20bf68`, `0x20c10c–0x20c110`). The earlier matching prepass
occurs before output copying, so the first installed path also contains the
snapped point. For a preceding quadratic or cubic, its endpoint must match
exactly as `f32`
(`0x20c194–0x20c1ac`). This normalization therefore
has different contour linking from directly feeding the original arc verb
to Drawing's force-move consumer.

Positive sweep exactly 360 is replaced with
`f32_bits(0x43b3feb8) = 359.989990234375` and remembered for an additional
close (`0x20bd9c–0x20bdc4`, `0x20c204–0x20c214`). Oval uses start 0 and the
same near-360 sweep before adding close (`0x20be1c–0x20be2c`). The Model
point builder has its own elliptical and circular branches; it is not a
call to the bundled Skia rectangle-arc helper. Their quadratic coordinates
differ in the captured ellipse case below.

The arc call also changes the rectangle interpretation: it loads the first
four runtime floats `(a, b, c, d)` but supplies `(a, b, a+c, b+d)` to the
point builder (`0x20be7c–0x20bec4`). The builder subtracts its first pair
from the second to obtain dimensions (`0x211838–0x211848`), so this route
uses `c` and `d` as extents. Direct Drawing passes `(a, b, c, d)` as literal
left/top/right/bottom. Both Java `SpenPath.arcTo` and native `Path::ArcTo`
store the caller's four numbers without that addition. Thus the producer
and direct consumer are observably different contracts when the origin is
nonzero; the symbol name alone does not resolve them. Oval normalization
instead orders the two corner pairs before passing them to the builder
(`0x20bde8–0x20be18`). These static operand mappings do not establish which
nonzero-origin arc cases Samsung's app actually produces.

This producer link is reached by the public custom-path route:
`ObjectShapeData::SetPath(Path*, RectF*)` (`0x3abe9c`) constructs an unknown
shape template (`0x2121a0`) and invokes its load slot (`0x3abefc–0x3abf14`).
That slot is `0x2122a8`, which calls the common loader at `0x2122cc`.
The vtable identity is backed by relative relocations at `0x4a41f0`
(pointing to `0x48f5c8`) and `0x48f5e8` (pointing to `0x2122a8`).
The preceding JNI route copies Java type and six float fields directly into
28-byte native segments (`0x31a9a4–0x31aa9c`); it does not flatten primitives.
Public `SetPath` obtains native input bounds and passes angle 0, flips 0 and
input-frame bit 0 (`0x3abefc–0x3abf14`). Ordinary saved loading instead passes
saved bounds/angle/flips and input-frame bit 1 (`0x3a95a0–0x3a95c0`).

Common loading stores/sorts bounds and toggles flip state for reversed axes
before expansion (`0x20bb3c–0x20bbd0`); this is not immediate point reflection.
With input-frame bit 1, expanded geometry is rotated by negative saved angle
(`0x20c278–0x20c298`). Installer `0x20c688` keeps that base outline at
implementation offset 32 and rotates a separate display copy by positive angle
at offset 24. Saved path writing retrieves this display copy (`0x3a8e64`),
so negative rotation is not the final displayed transform.

A conditional co-edit saved-load branch (`0x3a94a8–0x3a94d4`) invokes wrapper
`0x20b9cc`: after virtual loading it replaces only the display outline with
a copy of the caller's original segments (`0x20ba04–0x20ba60`), including
source mutations. This supplies a concrete static producer route retaining
raw Arc/Oval beside a normalized base path. It does not describe every co-edit
file or prove that this transient load context is encoded in saved path bytes.

The general template rotation helper (`0x20c968`) itself rotates the two
rectangle corner pairs of an unexpanded arc or oval. Arc's start/sweep
values are copied without rotation. Rotating those corners is not a general
representation of a rotated ellipse. The reached preprocessing route avoids
retaining those verbs there; it does not certify every template producer or
arbitrary manually supplied runtime path. Existing saved rotation must not
be applied a second time merely because a path retains its angle metadata.

## Executed normalization, mutation and native bounds

Two temporary probes execute actual common template/Path constructors, loader,
base/display storage and getters, plus bundled libc++ recursive-mutex lifecycle.
Each of twelve cases loads one native input Path twice into one template, then
into a fresh template reusing that same Path. Used command fields agree across
five fresh memory fills and independent strict-compiled replays. Generated
unused Move/Quad/Close slots vary with allocation fill and are omitted.

| Captured input | Installed output |
| --- | --- |
| Arc `(10,20,200,100,0,360)` | First load: Move, eight Quad, Close; subsequent loads: no Close |
| Move/Line endpoint `(209.99969482421875,70)` before quarter Arc | Source and first output endpoint snap to `(210,70)`; no new Move |
| Move endpoint `(209.99940490722656,70)` before same Arc | Separate Move added; no snap |
| Close followed by Arc; consecutive Arcs | Arc inserts a new Move in both cases |
| Oval followed by explicit Close | Generated Close and supplied Close both remain |

The first full-turn load mutates the caller's sweep to `359.989990234375`.
Close disappears even with the fresh template, proving a reused input Path
carries this state. Native const-qualified loading is not an immutable geometry
contract. Supplied flip flags and reversed caller bounds change stored flag
state without reflecting the captured expanded outline.

The second probe also executes `GetBounds` and `Path::Refresh`. Fresh raw
Arc/Oval-only input returns zero bounds before and after Refresh. Native bounds
use generated Bezier records (`0x2f274c–0x2f2800`); raw verbs 5 and 7 do not
contribute curves in `UpdateBezier` (`0x2f1a04`). With a preceding line, bounds
built before loading retain right edge `209.99969482421875` after source snapping;
Refresh (`0x2f0ee4`) recomputes it as 210. These source mutations do not invalidate
an existing Bezier cache. They are not bounds of the expanded display path.

Temporary probe SHA-256 values are
`776776f1909a42e2a99295adbd45d215ad1367947d3ff46ac53b95cae27ac1be`
(`/tmp/sdocx-native-shape-producer.json`) and
`521aaf117c5cd945a52acc7f90ecdb27e55fc2eb25bde711eedcd905c1f3152c`
(`/tmp/sdocx-native-shape-producer-bounds.json`). These are separate from the
reusable primitive fixture below. Caller bounds, angle, frame and flips are
explicit inputs, not decoded file state. Host boundaries are bounded allocation,
memory, single-thread pthread operations and the verified Linux math library;
bounds additionally reach `f64` `pow` and `acosf`. These two probes do not
execute saved/co-edit loading, named postprocessors, serialization or Drawing.

## Executed Model helper and named Arc production

The separate Model helper `0x211814` was executed with direct `f32`
left/top/right/bottom/start/sweep arguments and bounded output storage.
Its internal quadratic/root routines and Base `PointF::GetRotatedPoint`
(`0xb0f10`) also executed. Thirteen numeric cases agreed across five fresh
fills `[0, 85, 165, 255, 0]` and an independent strict-compiled replay.
Temporary `/tmp/sdocx-native-model-arc.json` has SHA-256
`dc491bffebd7a05c232b7a57f21b86469dfdcee3b995f2ad4e5cc1018a9fb60b`.

| Direct helper input | Returned point count |
| --- | --- |
| Ellipse `(10,20,210,120)`, start 0, sweep 360 | 17 |
| Circle `(10,20,110,120)`, start 0, sweep 360 | 1 |
| Same ellipse, start 37, sweep 0 | 3 coincident points |
| Same ellipse/circle, start 37, sweep 123 | 5 / 7 |

Reached external functions were `memcpy`, `atan2f`, `tanf`, `sincosf` and
`f64` `sincos` from the runtime-hash-verified host math library above.
Base rotation widens the angle, computes degree conversion and rotation with
`f64` operations including fused multiply-add, then narrows to `f32`.
These helper inputs are direct bounds, not serialized Arc fields; they do not
settle the normalization route's extent interpretation. Separate named-template
execution follows below.

The ordered ellipse `(10,20,210,120)`, start 37, sweep 123 also demonstrates
a different angle contract. Model returns first point
`(165.28851318359375,111.66287231445312)` and two quadratics; bundled Skia
returns `(189.86355590820312,100.09075164794922)` and three. Model's point lies
on the center's 37-degree polar ray, whereas Skia's axis-scaled sine/cosine
point has polar angle about 20.645 degrees. Model's ellipse branch intersects
the angle's ray using `tanf`/root arithmetic, then converts coordinates using
`atan2f` (`0x2119f8–0x211a30`, `0x211c40`, `0x211c84`). This is a geometric
contract difference, beyond float rounding; arbitrary inverted/degenerate
rectangles are not covered by this paired case.

Named `ObjectShapeTemplateArc` generation (`0x2243cc`) calls that
Model helper (`0x2244dc`) and builds an open outline using one move followed
by quadratics (`0x224518–0x224558`). It does not create a retained Arc verb.
The class identity is backed by RTTI name `0x1636be` and vtable `0x48fcb0`.
Its separate fill builder (`0x224130`) copies the outline, appends a line to
the bounds' center, then close (`0x2241e4–0x224214`), and installs fill index 0
with type 1 (`0x22422c–0x22423c`). Named shape identity, saved command identity
and generated fill geometry are therefore distinct. Fill refresh requests the
zero-angle getter (`0x224c58–0x224c88`), which returns the base outline
(`0x20d6b8–0x20d6c0`), then the common fill setter creates a separate rotated
display fill. The conditional co-edit wrapper replaces only the display
outline; its dedicated fill remains derived from base geometry in the
separately executed wrapper cases below. Selection of that wrapper by saved
loading remains a static link.

Actual named Arc constructor `0x223ffc` and rectangle setter `0x224cf4`
also executed for six rectangles across five fresh memory fills, with independent
strict-compiled replay. Default angles 270 to 0 produce an open Move/Quad/Quad
outline and separate Move/Quad/Quad/Line(center)/Close fill of type 1. Singular
point/line rectangles have no outline and zero fills; reversed corners change
the captured sector. Temporary `/tmp/sdocx-native-named-arc.json` has SHA-256
`362b79f985a6ad4eb329d70b681b82b88bf9e82a1a13f4b8dfe11d45332b82e6`.
Native template/Path/Bezier and libc++ code execute; bounded allocation/memory,
logging/string length, single-thread once/TLS/mutex and verified host math remain
boundaries. No full ObjectShape/SDK constructor or canvas executes.

A clean co-edit probe copies that actual generated outline using native
`Path::Construct`, then executes wrapper `0x20b9cc` twice. Four supplied
bounds/rotation/flip cases agree across five fills, including recovered
angle/text state. Source and display retain the generated Move/Quad/Quad;
fill remains the separate center-closed sector. Temporary
`/tmp/sdocx-native-generated-coedit-arc.json` has SHA-256
`34da6a82a050c6b0cc728351ead7a47a8ca9f025a182b19ae8ceeecc16d44671`.
These are native-generated input paths, not decoded saved samples.

The actual unknown-template constructor (`0x2121a0`), virtual loader and
postprocessors also executed through the wrapper for five supplied raw Arc/Oval
cases, twice each across five fills. Display keeps raw verbs 5/7, while base
outline contains Move/Quad with Close where normalization adds it. Dedicated
fill count is zero;
text margins are five on all sides. Positive-360 reuse loses base Close and
reduces native edit-point count from nine to eight. The edit list getters
(`0x20ee48`, `0x20edc8`) access distinct storage from the serialized control-point
list; edit state also agrees across fills. Temporary
`/tmp/sdocx-native-unknown-coedit.json` has SHA-256
`0a932e0d8fba350ee1a6ba475a1141de662a3ed7874dd568c9726efeb486aeb9`.
This establishes the general-template wrapper behavior on explicit inputs,
without claiming a saved-file producer or executing Drawing.

## Executed Drawing conversion of retained primitives

A separate probe passes both paths from the actual unknown-template wrapper
through Drawing's segment dispatcher (`0x9bd24`) and bundled Skia construction,
mutation and raw iteration. Five supplied Arc/Oval cases, loaded twice each,
agree across five fresh VM fills and independent strict-compiled replay.
For Arc `(10,20,200,100,37,123)`, normalized base begins at
`(165.28851318359375,111.66287231445312)` with two quadratics; raw display
begins at `(180.87037658691406,84.07260131835938)` with three. This comparison
combines the extent/LTRB and polar/parametric angle differences. The equal-bounds
helper comparison above isolates angle interpretation instead. Temporary
`/tmp/sdocx-native-unknown-drawing.json` has SHA-256
`9b988bfacbf28d95c4475e3b067e1f65b500630a594411df74fd2212cbe424d6`.
Model/Path, Drawing dispatch and Skia execute; host allocation/memory,
single-thread pthread operations and verified Linux math remain boundaries.
This probe reaches no full ObjectShape, fill effects, canvas, saved-file parser
or application-selected co-edit context.

A further bounded probe executes Drawing's solid-color consumer with an actual
constructed effect whose dedicated-fill count is zero. It forwards the complete
native outline copy to canvas virtual slot 96 with its main paint, matching the
fallback branch at `0x9cda8–0x9cdb0`. Actual Model `GetType` executes over explicit
type-7 caller storage; the ObjectShape itself is not constructed. The canvas
callback records arguments without drawing. Temporary
`/tmp/sdocx-native-unknown-solid.json` has SHA-256
`7a96eb48dcb65e6aec9d3d79b6dc9a29889e571135af6961bbf1d18b9fb9c845`.
No native `SetEffect` installation or template-fill-list transfer executes, so
this proves the conditional fallback independently of a complete scene bridge.

Drawing's solid-color shape branch retrieves each dedicated
`ObjectShape::GetFillPath`, converts its segments and draws it
(`0x9cda8–0x9ce38`). It does not simply fill the open outline in that branch.
Current Rust `render_shape` attaches fill and outline style to the same saved
path. For a native Arc outline, its implicit SVG closing chord differs from
the template's two center edges. No general fill rule for every template or
non-solid effect follows from this inspected branch.

The reusable [native path capture](../../conformance/README.md#native-shape-path-capture)
combines these 31 Skia path cases, six explicit-vector cases and thirteen Model
helper cases in [one fixture](../../conformance/shape-paths.json), SHA-256
`0e86e3458be734b4996aae3083264f8fbda60d0f16f3ca30dca18c30b0fb86e2`.
Each case matches the independently replayed temporary proofs. The driver pins
the actual loaded math-library identity, traps unbound imports and restores
native writable state between fills; its scope remains the primitive/helper
execution described above.

## Saved control points can regenerate the preceding outline

Saved loading restores the path first, then reads a one-byte control count and
`f64` coordinate pairs, narrows them to `f32` and invokes template slot 32
(`0x3a95c4–0x3a963c`). For named Arc, shape type 20, this reaches setter
`0x224628`. It inverse-rotates a control point when template rotation is nonzero,
recovers its polar angle, projects it onto the ellipse and regenerates base and
display outlines plus the separate center-closed fill
(`0x2246b4–0x2247e8`). Thus nonempty saved path bytes are not an unconditional
geometry authority for this template. Later
[saved-bounds refresh](text-layout-findings.md#saved-bounds-refresh) and load
scaling can transform that regenerated geometry; the complete saved dispatcher
has not been executed in these probes.

Native saved writing retrieves controls using getter `0x20ec9c` with the
enclosing saved rotation, then widens returned `f32` coordinates to `f64`
(`0x3a8ee4–0x3a8f20`). Stored controls and generated drawable paths remain
distinct; the native edit-point list above is a third representation.

Static factory/vtable inspection establishes that saved template identity
selects the control operation: type 0 Unknown reaches common slot `0x20e45c`,
which accepts the call without editing geometry; type 20 Arc reaches the
regenerating setter above. Common loading stores the saved rotation before
control dispatch. Public writer `NewGetBinary` snapshots outer rotation into
owning ShapeImpl+264 (`0x399bbc`), temporarily zeroes only the outer/base angle,
then restores it. `GetOwnBinary` uses that preserved snapshot for both saved
angle and control-getter frame; +264 is not a live alias of template angle.

Actual constructor/getter/setter execution for ordered positive ellipse and
circle bounds confirms that reapplying the default generated controls leaves
captured path and control-getter records byte-identical. Supplying `(20,30)` and
`(40,100)` instead projects onto the ellipse and regenerates quadratic outline
and sector fill. Two rectangles with four sequential changes agree across five
VM fills and independent replay; temporary `/tmp/sdocx-native-arc-controls.json`
has SHA-256 `5c2a7c06e3f4e05bcf3048d9af8b324a3da1d1459521b8ace9806f8978dd9f5a`.
This zero-angle probe does not execute the inverse-rotation branch.

A separate probe invokes actual template rotation setter `0x20cb6c`, then
reapplies native getter controls to both control indices twice. Default ellipse
±37 and circle 37/90-degree cases preserve base/display/fill/control-getter
records across five VM fills and independent replay. Temporary
`/tmp/sdocx-native-arc-rotated-controls.json` has SHA-256
`fded9eadf70b07651c7cd42fa2798c992ddbc77efeb63377630a3a1d17ce3a77`.
This executes template rotation and inverse control rotation, not the public
SDK/object rotation setter, enclosing saved-angle state or serialization.
Host boundaries match the named Arc construction probe above.

A separate moved-oblique probe first supplies the two points above, then
rotates and reapplies getter controls for the same four cases. In circle 37,
the first end-control reapplication shifts the getter's x coordinate by four
`f32` ULP and y by one, while base/display/fill commands remain bit-identical;
the second reapplication is stable. This is getter-coordinate drift, not a
direct snapshot of stored control arrays or a general native-state guarantee.
Five fills and independent replay agree; temporary
`/tmp/sdocx-native-arc-oblique-rotated-controls.json` has SHA-256
`556e4686577d4bacb1dbf6dcf5df017037929941df31e299cb0970b081c90dc6`.

## Empty saved Triangle is distinct from ordinary construction

Static type-2 Triangle loading accepts an empty Path through `0x2140dc` and
common loading; it does not invoke the default generator. Its late control
setter `0x2144ec` rejects a null or empty base outline. Rectangle setter
`0x2141a8` generates the default Move/Line/Line/Close only when the *previous*
stored rectangle is all zero; otherwise it resizes existing geometry. With
positive saved geometry bounds already stored, no implicit Triangle outline
is recovered by those routes. Ordinary construction starts with zero bounds
and does reach the generator, so construction and saved loading differ.

For the ordinary unscaled load with matching saved/current owner bounds,
the final owner rectangle/angle writes and `ClearChanged` do not regenerate
the object's template. These are static geometry findings, not a complete
Drawing appearance claim. Rust's pathless Triangle fixtures exercise its
fixed-vertex fallback and known flip flags; they do not establish native saved
parity. All five shapes in the recorded [rendering corpus](rendering-corpus-findings.md)
have stored paths; no application-produced positive pathless Triangle is known.

## Drawing routes separate geometry, commands and success

The ordinary [object dispatcher](object-drawing-findings.md) reaches
`ObjectDrawing::drawObjectShape` for type 7 (`0x7fb18`), which calls
`ObjectShapeDrawing::DrawPath` at Drawing `0x80744`. A separate native view
consumer, ObjectControl `ObjectShapeView::onDraw`, calls **Draw** at
`0x124850` and ignores its result. This establishes distinct callers, rather
than one universal UI route.

`Draw`, Drawing `0x8a5b8`, still composites its created bitmap after inner
`drawPath` failure (`0x8a648`, `0x8a6fc`); its result tracks bitmap allocation.
`DrawPath`, `0x8aaa8`, instead saves the inner result at `0x8adb4`. If the
wrapped bitmap exists, it attempts `drawSolidRectangle` at `0x8ae10`
regardless of that result, composites the bitmap only if the shortcut returns
false, then returns the original inner result at `0x8aeec`.

The shortcut requires type 7, failed line-color retrieval or literal packed
line solid color zero, successful fill-color retrieval, translated fill solid
color nonzero and color type 0 (`0x8af5c`–`0x8b000`). It submits
`[0,0,bitmap width,bitmap height]` with a fresh paint, style 0 and the translated
color at `0x8b05c`; it forwards no bitmap mask or shader. There is no template
test inside this helper, but its other gates and inherited destination state
remain material. `DrawPath` saves/translates/scales the canvas first. Graphics
`SPCanvasImpl::DrawRectRT`, `0xa71cc`, rejects zero-width/height extents and
nonintersecting regions, then enables existing clipping at `0xa72d8`/`0xa73d8`.
This source trace does not establish final coverage or imply that ordinary
filled triangles appear as rectangles.

For type 7, the `cset`/`orn`/`tbnz` sequence at Drawing `0x80750`–`0x80758`
skips subsequent text drawing when **DrawPath is false**. A true result enters
`drawTextContent` or, when alpha processing is enabled and alpha differs from
1, `drawObjectTextBox`. The original status is still returned at `0x807c8`;
these calls are not missing-outline geometry fallbacks. `DrawObjectList`
ignores per-object results at `0x7f43c`, `0x7f488` and `0x7f4dc`. The
[Standard list-PDF bitmap batch](standard-pdf-composition-findings.md#ordinary-objects-retain-interleaving-and-flush-the-tail)
also ignores its list-renderer result at Composer `0x37c554` before writing
the bitmap. An inner missing-outline result alone therefore need not fail
this export batch or supply replacement vector geometry.

The [separate native vector PDF paint route](shape-fill-findings.md#pdf-export-is-a-separate-paint-capability)
has different admission: `ObjectShapePdfExporter::ExportObject` returns false
directly if `setPathPoints` rejects the outline (`0x34ef8c` → `0x34f104`).
Its image-adapter fallback is reached only after the later path-adapter
`ExportObject` fails (`0x34f214` → `0x34f224`/`0x34f22c`). These are static
consumer boundaries, not executed export or saved pathless-shape appearance.

## Current Rust preservation and rendering boundaries

`NativeShape::path_data` and `NativeLine::path_data` retain raw WDoc path
bytes. `shape::visit_path` already identifies all seven known verbs and their
`f64` payloads, including typed `Arc([f64; 6])` and `Oval([f64; 4])`.
Arc and oval are therefore retained and structurally understood; they are
explicitly excluded from supported rendering. A path containing either is
rejected as a whole by `render::native_svg_path`, so even its otherwise
supported line/cubic prefix is not painted. A nonempty saved shape path does
not fall back to a guessed basic template when rejected.

The visitor also requires an initial move for supported paths. Native arc
and oval drawing create their own initial move, so this Rust condition
excludes additional valid native contours independently of the unsupported
verb check. Unknown commands remain opaque; trailing bytes prevent an exact
render. Non-finite saved doubles are a Rust format error before rendering.

Supported saved coordinates currently pass through
`render::vector::path::coordinate(value, 2)`: two decimal places are formatted
before parsing to `f32`. Native loading instead directly narrows the saved
double to float. Thus current Rust path output adds decimal quantization
beyond Samsung's float narrowing; preserving raw bytes alone does not remove
that geometric difference.

One typed retained command representation can describe saved geometry and
feed vector outputs. Arc start/sweep angles must remain named scalar fields,
not be transformed as an endpoint pair; contour-start and implicit-close
semantics must remain explicit. Native quadratic expansion can serve SVG
and PDF without a raster intermediate. Saved command precision, native
runtime precision and output decimal precision remain separate contracts.
Native normalization can mutate caller coordinates and cached bounds
independently of retained source bytes.

Rust retains `control_points`, but `render_shape` immediately paints a supported
nonempty `path_data` and returns. That branch does not apply named template
control setters or derive their separate fill path. Raw data retention therefore
does not establish equivalence to the native regeneration stage above.
