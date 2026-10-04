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

Line kind, direction and control data are direct stores without editing
setters (`0x38c05c–0x38c0ec`). Endpoints replace the two magnetic points
(`0x38c150–0x38c15c`). Unlike [named Arc control restoration](shape-path-findings.md#saved-control-points-can-regenerate-the-preceding-outline),
reading saved controls does not itself regenerate line geometry.

Flexible bit 3 loads a new Path at implementation `+72` through
`Path::ApplyBinary` (`0x38c464–0x38c4c0`), bypassing editing `SetPath`.
It does not reconcile endpoints against saved segments: drawing commands,
endpoints and controls remain distinct states. An absent bit retains the
existing Path instead of generating one from the newly read fields.

Let D be the first saved rectangle (drawn bounds), C the currently loaded
common rectangle and G the second saved rectangle (original common bounds).
The loader compares narrowed D with C using native `GetRect`, not
`GetDrawnRect` (`0x38c1a0–0x38c1e0`). Base `RectF::operator!=` (`0xb1a58`)
compares float coordinates directly, without an epsilon. If D equals C,
G becomes the restored rectangle (`0x38c4ec–0x38c4f0`). A mismatch invokes
`RefreshRect` (`0x38c4e4`), which computes axis scale ratios after inverse
bound rotation, transforms G, clamps its resulting dimensions to at least
one and calls `ResizePath` (`0x38ad14–0x38ae28`). That first stage transforms
existing segments and active controls, then derives endpoints from the path.

The next branch can regenerate the route. For ordinary finite inputs, it
truncates the saved rotation toward zero to a signed integer and takes its
remainder modulo 360. A zero remainder, or both float axis ratios strictly
within 0.0001 of one, selects magnetic-point refresh only. Otherwise it
adds float one to both coordinates of both endpoints and invokes public
`SetConnectorPosition` twice (`0x38ae2c–0x38af30`). Those setters can reach
disconnect, magnetic assignment, connection update and `RearrangePath`
(`0x388a6c–0x388ac8`), replacing the transformed commands with a fresh route.
This is an integer-angle gate, not a continuous near-zero-angle test.

The endpoint setter skips editing when both coordinate differences are
strictly below 0.0005; adding one can also round away at large float values.
History/context, disconnect or subsequent operations can fail; `RefreshRect`
ignores the resize and endpoint-setter results. This establishes conditional
routing reachability, not successful replacement or actual saved route output.

If the inherited common rotation is nonzero, loading also invokes
`RefreshRotation` with that angle and the final saved float
(`0x38c4f4–0x38c50c`). It rotates the current path, endpoints and control
storage, then stores the sum of the angles at `+212`
(`0x38af78–0x38b050`). Its path utility transforms three coordinate pairs
per segment without verb dispatch; it does not expand Arc/Oval primitives
or invoke the router. When inherited rotation is zero, the loader stores
the final float directly at `+212`. `NewApplyBinary` finally restores the
common rectangle/rotation from `+192/+212` through data-only setters.
Its last virtual call resolves to base `ClearChanged`, without own-line
route reconstruction (`0x386dd0–0x386df8`).

The type-8 object factory constructs straight kind and zero endpoints
(`0x36d910`); construction invokes routing and installs Move/Line before
the saved own-frame stage (`0x3819d8`, `0x38ecd8`). An omitted path can
therefore retain constructor geometry, or another path on a reused object.
It does not establish a native endpoint fallback or null drawing geometry.
No application-produced pathless line is established by the recorded corpus.

Those compatibility transformations matter for deliberately inconsistent
or rescaled records. A coherent modern writer record whose D equals loaded
C and whose inherited rotation is zero bypasses both compatibility producers;
G remains its separately restored common rectangle. That narrow contract
does not establish universal equivalence of raw saved commands with every
native load path. Current Rust rendering does not reproduce these adjustments.

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

Ordinary WDoc loading factory-creates each object before invoking its virtual
saved loader (`0x3597e4–0x359834`); a line reaches `NewApplyBinary`.
`WLayer::Load` completes `Load_ObjectList_WDoc` (`0x342248`) before its
deferred connection pass. Thus the geometry compatibility stages above
precede graph resolution. That later line setter can overwrite endpoint
fields without rebuilding the path, so endpoint/path agreement is not a
universal loaded-state invariant. This ordering remains static evidence.

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

## Copying and transfer treat identity and geometry differently

Layer `ObjectManager::Copy` and page `CopyNAppendObject` factory-create
destination objects before invoking virtual `Copy` (`0x35f13c–0x35f17c`,
`0x3649e4–0x364a24`). `ObjectBase::Construct` allocates a new `Uuid` at
common data `+136` (`0x2c9478–0x2c9490`); Base `Uuid::Uuid` generates it
through `uuid_create/make/export` (`0xaa99c–0xaa9bc`). Common property copy
does not overwrite that slot (`0x2d83bc–0x2d8710`). Fresh copies retain
their newly constructed UUIDs, rather than inheriting source UUIDs.

Shape copy clears outgoing/incoming associations (`0x37acdc–0x37ace8`).
Line copy initially clones saved segments through `SetPath` and copies
the control count and all three stored control slots
(`0x389260–0x38928c`). Layer copy then uses an old-to-new **runtime handle**
map in `LayerDocImpl::CopyConnectionInfo` (`0x34f258`, called at
`0x35f2ec`). These ephemeral handles are not the persisted target UUIDs.
For each type-8 line, both endpoint indices 0/1 are considered, but only
target index 0 and its first target-point metadata entry are inspected
(`0x34f374–0x34f394`). Both line and target must map to copied objects;
an unmapped target is skipped without reconnecting the original or trying
later targets. Page `CopyNAppendObject` has a separate equivalent loop
(`0x364b1c–0x364ce0`).

Both loops invoke destination `Connect`, then replay active source controls
through `MoveControlPoint` (`0x34f3fc–0x34f43c`). Connection creation reaches
`RearrangePath` (`0x388984–0x38899c`); control editing can reach `MakePath`
(`0x38f350`). Reconnection can therefore regenerate the initially cloned
vector route. The loops ignore these edit results and do not restore the
source path afterward. This is not proof of exact geometry preservation
or successful restoration of every relationship.

Grouping creates a type-4 container and moves original children into it
(`0x34ef24`, `0x34efa4–0x34efe0`). Container copying instead factory-clones
children through `CopyObjectInList` (`0x374b60–0x374bfc`), with no child
correspondence map or connector reconstruction. Outer layer/page rebind
loops inspect type-8 objects, so a type-4 container bypasses them.
Container `AppendObject` (`0x373bb4–0x373f60`) and line attachment
(`0x386f64`, `0x37dd0c`, `0x38957c`) bind resources/context and membership
without a connector rebinder. Within this reviewed copy/append/attach
lifecycle, copied child lines have their old graph cleared and bypass the
flat-list reconnect pass. This does not establish every UI paste flow.

`TransferObjects` and `TransferObjectList` (`0x35f404`, `0x35f630`) move
existing pointers, scale/offset their rectangles, call `OnTransfer`, then
`SetRect(rect, false, true)`. Base transfer changes resource context without
changing the UUID (`0x2d0248–0x2d046c`). Identity survives this boundary;
geometry and attachments need not. Line resize in connection mode 0 first
requires successful disconnection of both endpoints
(`0x387fc0–0x387fec`). `ResizePath` (`0x388010–0x3884f8`) transforms all
three coordinate pairs per existing 28-byte segment and active controls
in `f32`, preserving segment count/type words. It writes `Path::SetSegment`
and derives endpoints (`0x388474`, `0x38849c`), without `MakePath` or
`RearrangePath`. This generic pair transform does not establish semantic
transformation of arbitrary nonstandard arc/oval line segments. These are
static native operation boundaries, not executed copy/save captures or a
guarantee that cross-page references remain valid. The current Rust
connection-block omission is described below.

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

`NativeShape.base_source` and `NativeLine.base_source` retain boxed type-6 source:
f64 magnetic points and raw sized `connection_data`, excluding its length prefix
but including the count and opaque remainder. Populated connection records or opaque
connection remainders still produce an unsupported warning; there is no typed resolved
or unresolved connector graph or edit-time routing. Object format version remains in
common metadata; page/document versions are separate source context. Unowned type-6
masks, reserved bytes and fixed/flexible/style extensions still require original
uncompressed page bytes. The raw connection bytes retain the saved relationship
payload without resolving target identity or recomputing routes.
The locked `hf/02-shapes-and-dot-calibration.sdocx` contains five shapes and one line
with 31 retained magnetic points. All six connection payloads are exactly four zero
bytes, so this provides no nonempty-graph or native-appearance witness.

For existing diagrams, emitting complete saved geometry avoids making
vector export dependent on a second routing implementation or a guessed
screen size. Source path commands, attached-object identities and route
recomputation are different preservation surfaces. Missing saved paths,
compatibility transformations, side-pair numeric parity and an actual
connected-diagram Samsung export remain outside the evidence established
here. None of those limits justifies replacing a saved connector path with
a raster image.
