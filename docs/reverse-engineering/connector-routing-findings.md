# Saved connectors and line routing

## Evidence boundary

These findings use static arm64 disassembly from Samsung Notes 4.4.45.37
and its decompiled `SpenObjectLine`/`SpenObjectShapeBase` APIs. Addresses are
ELF virtual addresses. The input libraries have these SHA-256 identities:

| Library | SHA-256 |
| --- | --- |
| `libSPenModel.so` | `4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a` |
| `libSPenBase.so` | `e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb` |
| `libSPenDrawing.so` | `788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd` |
| `libSPenComposer.so` | `52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f` |
| `libSPenWordDocCoedit.so` | `82a73d24732efe4f5c3c385b9fcb0ccfb05970507ba50039d252c2abb7f0c2af` |

No connector routing execution or paired connected-diagram export was
captured here. [Shape and line frames](shape-line-findings.md) cover the
existing decoder and saved path verbs;
[outline findings](shape-style-findings.md) cover dash, compound and arrows.
The [available corpus](rendering-corpus-findings.md) contains one saved line,
which does not establish the connected elbow/curve behavior below.

## Geometry, direction and rotation are separate saved values

`ObjectLineBinaryHandler::GetOwnBinary` (`0x38bc34`, Model) writes the
type-8 fixed region in the following order. Offsets start at the first
fixed byte, after the frame header. `n` is the stored control-point count.

| Fixed offset | Encoding | Native source |
| --- | --- | --- |
| 0 | `u8` | Line type, implementation integer at `+60` |
| 1 | `u8` | Start direction, implementation integer at `+64` |
| 2 | `u8` | Control-point count at `+112` |
| `3 + 16i` | Two `f64`, for `i < n` | Control point from the `f32` array at `+88` |
| `3 + 16n` | Two `f64` | Begin connector from `f32` coordinates at `+116` |
| `19 + 16n` | Two `f64` | End connector from `f32` coordinates at `+124` |
| `35 + 16n` | Four `f64` | Saved drawn rectangle at `+176` |
| `67 + 16n` | Four `f64` | Saved common rectangle at `+160` |
| `99 + 16n` | `f32` bits | Saved common rotation at `+208` |

The final four bytes are not an untyped integer routing flag.
`ObjectLine::NewGetBinary` (`0x386a64`) snapshots the object's common
rectangle, drawn rectangle and rotation into precisely those slots
(`0x386aa8–0x386ae0`). The fixed writer copies the final word unchanged
at `0x38bd70–0x38bd74`; the reader loads it into float register `s9`
at `0x38c244`. Rust's `NativeLine::raw_setting: u32` retains these bits,
but its current semantic name does not express this rotation contract.

The second byte is connected to `Properties::GetStartDirection`
(`0x390f20`), rather than a generic routing enable switch. The properties
builder loads the line's `+60/+64` integers together at `0x388d2c` and
passes them to the properties constructor (`0x388d94–0x388db4`); the
constructor stores them at `+8/+12` (`0x390dd4`). The getter returns
the second integer from `+12`. This identifies its role without assigning
unverified numeric up/down/left/right names.

Named Java line kinds remain straight 0, elbow 1 and curve 2; connectors
are begin 0 and end 1. Public setters reject other values, while the
saved type-8 reader stores the loaded bytes directly
(`0x38c058–0x38c084`). Java setter validation therefore does not prove
that unknown saved byte values cannot exist.

## The saved path is the drawing result

The type-8 flexible field at bit 3 writes the path already held at line
implementation `+72` (`0x38bdfc–0x38be2c`). It does not invoke the
router first. The saved WDoc path carries its own command count and
verb coordinates; it has no additional byte-length prefix in this field.

`NewGetBinary` temporarily changes the common type-0 rectangle to the drawn
rectangle and sets common rotation to zero (`0x386ad8–0x386af8`). It then
writes the inherited frames and type-8 frame, restoring the original
rectangle and rotation afterward (`0x386b34–0x386b50`). The endpoints and
path were already updated by native rotation operations. Thus a normally
saved modern line's type-8 angle restores object state; applying that
angle again to its saved vector path would rotate the geometry twice.

The drawing consumer `ObjectShapeDrawing::LinePathType::GetPath`
(`0x8bb9c`, Drawing) directly calls `ObjectLine::GetPath()`. That no-argument
getter returns implementation `+72` directly (`0x381e34–0x381e40`).
The separate `GetPath(float)` overload reaches `ObjectLineImpl::GetPath`
(`0x38869c`). That implementation returns `+72` unchanged when common
rotation is zero; otherwise it creates a rotated copy using the requested
angle minus current rotation (`0x3886c4–0x38874c`). Neither getter searches
connections or generates a new route.

Composer's `ObjectShapePDFWriter::WriteObject` (`0x37c810`) also reads
`ObjectLine::GetPath()` (`0x37c8c8–0x37c8cc`), then requests samples with
`Path::GetPoints` (`0x37c908–0x37c914`). This establishes consumption of
the existing path, not fidelity of the subsequent sampled PDF route to
the saved curve commands. It does not establish a need to sample curves
in this project's SVG output.

## Native loading can normalize inconsistent geometry

`ApplyOwnBinary` (`0x38bed8`) narrows saved control points, endpoints and
rectangles from `f64` to `f32` using `FCVTN`
(`0x38c0e4`, `0x38c11c`, `0x38c148`, `0x38c1d4`, `0x38c228`). The
writer widens the original runtime floats to doubles. Double wire storage
is therefore not evidence that Samsung routes its connectors with double
coordinate precision.

After reading endpoints, the loader replaces the line's two magnetic
points with those endpoints (`0x38c150–0x38c15c`). It compares the first
saved rectangle, after narrowing, with the currently loaded common
rectangle (`0x38c1e0`). A mismatch invokes `RefreshRect`
(`0x38c4cc–0x38c4e4`), which rescales geometry through native line setters.
When they agree, the second saved rectangle is copied as the restored
rectangle (`0x38c4ec–0x38c4f0`).

If the inherited common rotation is nonzero, loading also invokes
`RefreshRotation` with that angle and the final saved float
(`0x38c4f4–0x38c50c`). That helper rotates the path, endpoints and control
points, then stores the sum of the supplied angles at implementation
`+212` (`0x38af78`, `0x38b004–0x38b050`). When inherited rotation is zero,
the loader stores the final float directly at `+212`. `NewApplyBinary`
finally restores the common rectangle/rotation from `+192/+212`
(`0x386dd0–0x386de8`).

Those compatibility transformations matter for deliberately inconsistent
or rescaled records. The normal writer's drawn-rectangle/zero-rotation
normalization is a narrower contract than universal equivalence of raw
saved coordinates with every native load path. Current Rust rendering
does not reproduce these native load adjustments.

## Connection identities live in the inherited shape frame

The type-6 writer (`0x37c6b4`) serializes magnetic points followed by a
sized connection block. For the modern WDoc writer, the block is:

| Order | Encoding |
| --- | --- |
| Record count | `u32` |
| Each connection point | Two `f64`, widened from runtime `f32` |
| Number of connected objects at that point | `u32` |
| Each connected object | Native UUID binary |

The point/object-count writes occur at `0x37c824–0x37c844`; each object
uses `ObjectBase::GetUuid` followed by `Uuid::GetBinary`
(`0x37c858–0x37c874`). Base's UUID writer (`0xaaa54`) emits a two-byte
length 36 and 36 UUID bytes, returning 38. This writer contract should
not become an assumption that every future UUID record has a fixed size:
`Uuid::ApplyBinary` (`0xaac84`) returns the actual prefixed length plus
two, which the connection readers use to advance.

`LayerDocLoadHandler::Load_ConnectedInfo` (`0x356b68`, Model) resolves
WDoc references through `LayerDocImpl::v_FindObjectByUuid`
(`0x356d24–0x356d50`). Its older non-WDoc branch reads four-byte object
IDs and looks them up in the supplied object map
(`0x356d6c–0x356db4`). Coordinate width also has a version distinction:
WDoc with unsigned load format version greater than 2033 reads doubles;
the alternative branch reads floats (`0x356c10–0x356cb4`). Those routes
must not be merged into one invented integer-ID format.

Missing UUID targets are skipped from the resolved object list, and a
connection record with no resolved targets is removed
(`0x356d48–0x356dec`). The loader then releases the original connection
bytes before calling the object's setter (`0x356e0c–0x356e28`).
`ReleaseConnectedInfoRawData` deletes and nulls implementation `+88`
(`0x377a98–0x377aa4`). The modern `NewApplyConnectedInfoListBinary`
resolver also filters unresolved UUIDs (`0x377e3c–0x377eb0`) and releases
that raw block when resolution completes (`0x378030–0x378054`).

The type-6 writer subsequently traverses implementation `+24` and writes
UUIDs obtained from resolved object pointers (`0x37c7b8–0x37c874`); it has
no fallback to the unresolved raw block. On a fresh loaded graph, a UUID
discarded by these resolution paths therefore cannot be reproduced by
that writer. This is a static resolver-to-writer conclusion, not a captured
whole-document load/save round trip. Preserving the unresolved saved
identity remains useful even where native runtime resolution discards it.
The serialized connection record describes a point and object list,
not an explicit begin/end enum alongside every UUID.

The base setter accepts an incoming record only at magnetic points whose
two `f32` coordinate differences are each strictly below 0.02
(`0x378448–0x378470`; float bits `0x3ca3d70a` at `0x16450c`). When its
boolean argument is true, it rebuilds target magnetic-point indices and
can remove targets whose point lookup returns `−1`
(`0x3785ec–0x378690`). Matching a UUID alone does not guarantee retention
of its attachment.

For a line, `SetConnectedInfo` invokes the shape-base setter and then
`UpdateConnectionInfo` (`0x382f20–0x382f30`). The latter obtains magnetic
connection records 0 and 1, copies their points into begin/end coordinates,
and retains the first object of each list at implementation `+136/+144`
(`0x3889f0–0x388a5c`). The examined base/line setters and endpoint-update
function neither replace the saved path at `+72` nor call `RearrangePath`.
At this boundary, graph pruning changes attachment state while retaining
the saved route; endpoint updates do not themselves regenerate it.
The public connection action does call it, through `ObjectLineImpl::Connect`
(`0x388970–0x38899c`). Loading a saved graph and interactively connecting
a line are therefore distinct operations.

Modern `WLayer::Load` calls `NewApplyConnectedInfoListBinary(false)`
(`0x342288–0x34228c`), then separately calls `LoadFollowerList`
(`0x342294–0x3422a0`). Followers have a different missing-target policy:
`FollowerManager::m_LoadFollowerInstance` retains unmatched pending UUIDs
and returns false with error 9 (`0x3223c8–0x32249c`). The layer loader checks
that result and takes its failure branch. Silent connector-reference
pruning is therefore not a universal rule for every saved relationship.

Coedit also has a distinct deferral boundary. `CoeditNote::checkConnectedInfo`
(`0x42684`, WordDocCoedit) tracks received shape UUIDs and pending reference
UUIDs per owner page (`0x42714`, `0x42bac`). It enumerates raw connection
UUIDs with `GetConnectedShapeUuidFromRawBinary(true)` (`0x431b8`) and adds
identities absent from that page's accumulated received map to its pending
map (`0x43258–0x43668`). While the pending count is nonzero, it skips
resolution (`0x436f0–0x436f8`); that cleanup destroys the temporary string
vector, retaining the shape's raw connection block.

When the pending count reaches zero, the route walks the received shapes
and calls `NewApplyConnectedInfoListBinary(true)` (`0x43790–0x43798`).
This variant consumes an extra signed four-byte value after each target
UUID (`0x377e54–0x377e60`) and passes false to the base setter
(`0x37804c–0x378054`), unlike the modern layer loader's false variant.
The coedit caller ignores the resolver's return value and clears its
received map afterward (`0x4379c–0x437d4`). This establishes deferred
resolution with raw identity retention, not guaranteed later attachment,
network arrival order, or behavior when dependencies remain missing
permanently. Its pending map is not a test of the entire model's UUID set.

## Routing is an edit-time geometry producer

`ObjectLineImpl::RearrangePath` (`0x387b88`) uses both connector points
and finds their connected shapes. For elbows/curves it obtains each
shape's common rectangle and rotation. An unattached endpoint supplies
a zero-area rectangle at its own point instead
(`0x387bf8–0x387cc0`). Those values enter
`ObjectLineUtil::RearrangeInflectionPath` (`0x38ed70`), which calls
`ObjectLineFindUtil::FindControlPoint` and then `UpdateInflectionInfo`.
The produced path is installed through `SetPath`
(`0x387cc0–0x387cf0`).

`FindControlPoint` (`0x38db64`) derives a connection case from the two
endpoints, shape rectangles and rotations, then dispatches ten side-pair
helpers (`0x38dd30–0x38ddb8`). This trace establishes a geometry-dependent
native producer, not a generic Manhattan-grid or obstacle-avoidance
algorithm. It does not establish behavior around unrelated third shapes.

The producer also has device-dependent state.
`UpdateBasicConnectionOffeset` (`0x38c5dc`, spelling as exported) computes
the signed 32-bit screen-width × screen-height product. For product
3,686,400 it stores `f32` 60; otherwise it computes
`60 * sqrt(product / 3,686,400)` in `f64`, then narrows to `f32`
(`0x38c5e8–0x38c638`; divisor at `0x12d410`). The line implementation
constructor calls this initialization (`0x3877b0`). This value is used by
the side-pair helpers: `FindControlPoint_U_D` loads it and expands candidate
coordinates with `f32` subtraction/addition (`0x38de4c–0x38de70`);
GOT entry `0x4a3e50` relocates to `BASIC_CONNECTION_OFFSET` at `0x4b8820`.
A route reconstructed
without the native screen context can change its spacing even when its
endpoints match. The formula alone does not establish valid behavior for
overflowing products or invalid screen dimensions.

`OnConnectedPointMoved` (`0x38abd4`) first updates connection endpoints.
Its cause-specific branches resize or rearrange the path and update the
magnetic points (`0x38ac20–0x38acec`). This is another route producer;
it is not called by the drawing getter. Public loosely/tightly coupled
connection modes are 1/0. Their editing consequences must not be inferred
from the start-direction byte merely because both participate in editing.

## Consequences for Rust vector preservation

`NativeLine` already retains the type-8 controls, endpoints, both saved
rectangles, final word bits and optional path. The renderer gives a
supported saved path precedence over endpoint fallback, including for a
straight line. Supported path verbs become SVG commands; a path-less
straight line uses its saved endpoints. A path-less elbow/curve is omitted
with an unsupported-feature diagnostic rather than routed speculatively.

This is vector output, but it is not coordinate-exact serialization.
`native_svg_path` rounds coordinates to two decimal places and passes
`f32` values to the typed path builder; the straight-line fallback also
uses two decimal places. Raw path bytes and decoded double endpoints
retain finer source values independently of that export precision.

The current decoder's `routing != 0 || raw_setting != 0` warning combines
known direction/rotation state with unknown geometry extensions. A nonzero
value in either field does not itself show that the saved path is unusable.
Conversely, accepting such state must not imply native edit-time routing
or the compatibility load transformations are implemented.

Type-6 magnetic coordinates are read and discarded; the connection block
is retained only through the original bounded object payload and yields
an unsupported warning when populated. There is no typed resolved or
unresolved connector graph in the SDK. Preserving visual saved paths
therefore does not currently preserve editable attachment relationships.
`StoredPage` payload boundaries require the original uncompressed page
bytes to recover those relationships.

For existing diagrams, emitting complete saved geometry avoids making
vector export dependent on a second routing implementation or a guessed
screen size. Source path commands, attached-object identities and route
recomputation are different preservation surfaces. Missing saved paths,
compatibility transformations, side-pair numeric parity and an actual
connected-diagram Samsung export remain outside the evidence established
here. None of those limits justifies replacing a saved connector path with
a raster image.
