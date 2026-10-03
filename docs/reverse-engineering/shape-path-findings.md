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
| `libSPenSkia.so` | `42636cb9ac06cc286114b42c1b9d8f4b78d33761843251cde2b443b101ffb88d` |
| `libSPenBase.so` | `e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb` |
| `libc++_shared.so` | `4397241b4bd20a8e579bfb41d21107857e12985f6a01ca0c2a5f83380d1270b4` |

Serialization, named template and Drawing dispatch were inspected statically.
Native execution covers Skia paths, the separate Model arc helper, and common
template normalization with actual Path storage and bounds below. Saved binary
and co-edit loading, named Arc fill generation and paired Samsung appearance
remain unexecuted.
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
bounds additionally reach `f64` `pow` and `acosf`. Saved loading, co-edit dispatch,
named template postprocessors, serialization and Drawing do not execute.

## Executed Model helper and named Arc template

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
Named Arc template/fill dispatch was not executed. These helper inputs are
direct bounds, not serialized Arc fields; they do not settle the normalization route's extent interpretation.

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

Static named `ObjectShapeTemplateArc` generation (`0x2243cc`) calls that
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
outline; its dedicated fill can remain derived from normalized base geometry.
That split is statically established, not an executed co-edit result.

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
