# View input transforms before stroke recording

## Evidence and scope

Confirmed by static inspection of Samsung Notes 4.4.45.37 ARM64
`libSPenView.so`, `libSPenBase.so`, `libSPenComposer.so`,
`libSPenDrawing.so`, `libSPenMarker2.so` and `libSPenPenCommon.so`
from the APK identified in the
[knowledge base](README.md#sources-and-validation).

Child-view dispatch copies the event, subtracts the child's position and
applies the inverse child matrix. Both current and historical X/Y samples
are transformed. Matrix application rounds coordinates to floats, while
the separate location-offset operation adds doubles. Neither operation
changes the other pointer-coordinate channels.

These findings establish the shared dispatch mechanics and their bindings
in the note views. They do not yet identify every ancestor matrix, zoom
configuration or special input path needed for an exact screen-to-note
conversion. They precede the separate
[page-local insertion translation](stroke-insertion-findings.md).

The preceding [Android event adapter](motion-event-adapter-findings.md)
already supplies float-derived coordinates and copies pressure and pen
axes without normalization. It gives current samples independent raw X/Y,
but initializes historical raw X/Y from the corresponding ordinary X/Y.

## Note views use the shared child-dispatch machinery

Composer `ContentsView::DispatchTouch`, `0x4182f4`, forwards its ordinary
enabled path to `ViewGroup::DispatchTouch` at `0x4183b4`.
`NoteWritingView::DispatchTouch`, `0x425bc0`, has ruler-related branches;
with that branch disabled it forwards the same event to `0x546644`
at `0x425cac`. That helper reaches `0x532a1c` at `0x546700` or
`0x54675c`. Its dispatch paths call `ViewGroup::DispatchTouch` at
`0x532ce0` and `0x532cf8` after their gesture and document checks.

View `ViewGroup::DispatchTouch`, `0x72be8`, sends action 0 through
`dispatchTouchDownToChildren`, `0x72c9c`. Other accepted actions use
virtual slot 920 with touch-type argument 1 at `0x72c34`.
Composer relocations `0x5720b0` and `0x575660` bind that slot for
`ContentsView` and `NoteWritingView` to View's
`dispatchTouchToChildren`, `0x73ee0`.

Both child paths copy the event before adjusting it. The initial copy
receives a negative Y offset from group member 440 when that value is
nonzero, at `0x72cfc` or `0x73f40`. Its application-level meaning is not
assigned here. Each eligible child then receives a separate copy and
the shared transform helper:

| Path | Per-child copy | Transform helper | Child dispatch slot 136 |
| --- | --- | --- | --- |
| Down | `0x72f70` | `0x72f80` | `0x72fa4` |
| Subsequent touch | `0x73f74` | `0x73f84` | `0x74080` |

The original parent event is not the object transformed for each child.
Child eligibility, event consumption and ruler or gesture interception
remain separate from this coordinate operation.

## Child position is removed before the inverse matrix

`ViewGroup::applyTransformToEvent`, View `0x73e20`, performs the following
steps for a nonnull child:

1. Read the child's float members 276 and 280, negate them, promote them
   to doubles and call `MotionEvent::OffsetLocation` at `0x73e68`.
2. Obtain the child matrix through slot 344 at `0x73e7c` and test whether
   it is identity.
3. For a nonidentity matrix, obtain it again, invert it at `0x73ea8`,
   and call `MotionEvent::Transform` at `0x73eb4`.

View `GetLeft`, `0x7099c`, and `GetTop`, `0x709a4`, identify the two
position fields. Composer relocations `0x571e70` and `0x575420` bind
slot 344 in the contents and note-writing views to `View::GetMatrix`,
`0x709bc`.

That getter either calls `Transform::GetMatrix` on view member 64 or
copies the matrix at member 152, selected by byte 310. Thus the helper
uses the view's configured matrix; a class or method name alone does
not supply its runtime zoom, pivot or translation values.

For a single child with a pure scale matrix, the recovered order is
equivalent to subtracting the child position and then dividing by the
scale. A position `(10, 20)`, scale 2 and parent point `(110, 220)` produce
child point `(50, 100)`. This is an arithmetic example, not a captured
Samsung Notes configuration. Multiple ancestors apply their own steps.

## Location offsets update samples and transform bookkeeping

Base `MotionEvent::OffsetLocation(double, double)`, `0xc0c18`, traverses
the history vector at implementation member 80 and the current-pointer
vector at member 48. It adds the supplied double pair to each coordinate
pair at pointer-record offset 40, at `0xc0c64` and `0xc0c90`.

It also converts the offset values to floats at `0xc0ca0`–`0xc0ca4`,
builds a translation matrix, combines it with the existing accumulated
matrix through helper `0xbf260`, and stores the result at implementation
member 104 at `0xc0ce4`–`0xc0ce8`.

`MotionEvent::GetTransforms`, `0xc12d0`, returns that member-104 matrix.
The copy constructor, `0xbf52c`, explicitly copies its 36 bytes at
`0xbf594`–`0xbf5a8`, in addition to copying the event's samples.
The child-dispatch copies therefore retain prior transform bookkeeping.

The matrix is a float representation of accumulated operations. It is
not an exact substitute for the actual double additions used by
`OffsetLocation` in every numerical case.

## Matrix application transforms history and current samples

Base `MotionEvent::Transform`, `0xc0d2c`, invokes
`PointerCoords::Transform` over the history vector at `0xc0d70` and
the current vector at `0xc0d94`. It combines the supplied matrix with
the accumulated matrix through `0xbf260` at `0xc0db4`, then writes
the result back to member 104.

`GetX`, `0xc0ad8`, reads the current vector at member 48 and coordinate
offset 40. `GetHistoricalPos`, `0xc0858`, reads the vector at member 80.
These are the samples later obtained by the
[stroke recorder](stroke-recording-findings.md#drawing-and-recording-are-separate-operations).

The complete `PointerCoords::Transform` implementation is
`0xc0e08`–`0xc0e54`. For a matrix stored as nine consecutive floats
`m[0]` through `m[8]`, its arithmetic is:

```text
xf = float(stored_double_x)
yf = float(stored_double_y)
w  = (m[2] * xf + m[5] * yf) + m[8]
x  = ((m[0] * xf + m[3] * yf) + m[6]) / w
y  = ((m[1] * xf + m[4] * yf) + m[7]) / w
```

The actual instructions first multiply the Y terms, then use fused
multiply-add for the X terms at `0xc0e2c` and `0xc0e34`. Translation
addition and division follow at `0xc0e3c`–`0xc0e48`. The two output floats
are promoted to doubles at `0xc0e4c` and stored at offset 40.
There is no zero-denominator guard in this method; behavior of a singular
matrix's earlier inverse is a separate question.

Only the X/Y pair is written. The method does not rotate orientation,
rescale pressure or tilt, or modify timestamps and pointer counts.
Those observations apply to this coordinate operation, not every live
input filter that may also process the event.

Float conversion means that applying a matrix and its inverse need not
recover the original double coordinates exactly. For example, an identity
matrix passed directly to this primitive maps X `16777217.0` to
`16777216.0`. The child helper skips matrix application when its matrix
is identity, so that example illustrates the primitive's precision rather
than an unconditional loss at every view boundary.

## Recorded pen width comes from a separate getter

Drawing `TouchStrokeDrawing::createObjectStrokeByPenData`, `0xb7658`,
loads the pen from PenData member 16 at `0xb76ac`. It calls pen slot 24
at `0xb76c4` and passes the returned float directly to
`ObjectStroke::SetPenSize` at `0xb76cc`.

For Marker2, primary-vtable relocation `0x2eae0` binds this getter to
PenCommon `Pen::GetSize`, `0x4600c`, which returns pen member 24.
No event transformation matrix is supplied to this getter or to the
object-size setter at this call site. The previously recovered
[size clamp and drawable conversion](marker2-rendering-findings.md#size-clamping-and-stamp-geometry)
are separate pen operations.

This establishes the source of width at object creation. It does not
establish how every upstream setting changes that pen member when zoom
or content scale changes. In particular, the sample transform above is
not evidence that stored width should be divided by the same matrix scale.

## Validation and remaining work

The APK digest and all six library byte streams were verified. Imported
view-method and pen-getter bindings, sample-vector accesses, copy-constructor
matrix preservation, coordinate stores and float arithmetic were checked
against the ARM64 instructions. Disposable arithmetic reconstruction checked
the scale/offset and precision examples. These are static results; no new
device fixture or native execution was used, and no SDK code changed.

Keep decoded stroke points and widths authoritative for replay. Input
coordinates can already include view conversion, filtering, optional
finalization and later page translation. Reapplying those editing-time
operations to a saved stroke would introduce additional changes.

The [zoom scale trace](zoom-scale-findings.md) now connects Composer's
registered scroller callback to the contents-view scale and translation
setters. It also resolves `NoteWritingView::SetScale`, `0x4284f0`, to
cutter/eraser scale updates and the diagram transformer. The ordinary
upstream pen-setting path remains a separate investigation.
