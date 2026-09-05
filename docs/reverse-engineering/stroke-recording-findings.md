# Touch stroke recording and replay inputs

## Evidence and scope

Confirmed by static inspection of Samsung Notes 4.4.45.37 ARM64
`libSPenDrawing.so`, `libSPenModel.so`, `libSPenBase.so` and
`libSPenMarker2.so` from the APK identified in the
[knowledge base](README.md#sources-and-validation). This follows the
[Marker2 sampling findings](marker2-sampling-findings.md) upstream into the
touch recorder and downstream into its stored-array replay wrapper.

The traced recorder appends event samples independently of the stamps a
pen draws. Repeated coordinates survive these append operations. Ordinary
Marker2 V1/V2 bypass the recorder's optional coordinate replacement, and
their stored-array replay reconstructs events with source 0. These findings
do not establish every transformation before events reach this recorder or
every later operation on a saved stroke.

## Drawing and recording are separate operations

Drawing `TouchStrokeDrawing::OnTouch(MotionEvent&, MotionEvent*, RectF*)`,
`0xb71a4`, reads the event action at `0xb7270`. Action 3 cancels the stroke.
Action 0 creates an object through `createObjectStrokeByPenData` at
`0xb72f8`. That creator constructs the object with the pen name at
`0xb76b0`, sets its tool type at `0xb7764` and copies advanced settings at
`0xb7784`, along with color, size and the other supported pen properties.

For the ordinary Marker2 branch, pen slot 232 retrieves the drawable and
drawable slot 96 reports `IsTip() == false`. The call at `0xb754c` therefore
uses drawable slot 80, `Draw(MotionEvent const*, RectF*)`. Marker2's pen
relocation `0x2ebb0` resolves slot 232 to `GetStrokeDrawableGL`, and its V1/V2
relocations `0x2ee08`/`0x2ef48` resolve slot 96 to the false-returning
`IPenStrokeDrawableGL::IsTip`, `0x20a64`.

After drawing, `OnTouch` updates tolerance and bounds, then calls
`addEventPointsToObjectStroke` at `0xb7570`. It does not branch on the
ordinary `Draw` return value before this append. In particular, Marker2's
distance and alternating-skip filters govern its rendered stamps; they do
not remove the corresponding event samples at this recording stage.

The append routine, Drawing `0xb7dc0`, accepts actions 0, 1 and 2 and copies
the MotionEvent at `0xb7e0c`. It appends every historical sample in order,
then the current sample:

| Channel | Historical getter | Current getter |
| --- | --- | --- |
| X | `0xb7e34` | `0xb7ebc` |
| Y | `0xb7e48` | `0xb7ecc` |
| Pressure | `0xb7e58` | `0xb7edc` |
| Timestamp | `0xb7e68` | `0xb7ee8` |
| Tilt | `0xb7e78` | `0xb7ef8` |
| Orientation | `0xb7e88` | `0xb7f08` |

Historical appends call `ObjectStroke::AddPoint` at `0xb7ea8`, current
appends at `0xb7f28`. X/Y are converted from the event getters' doubles to
floats, and timestamp arguments use the low 32 bits for this integer API.
There is no coordinate-equality check or extra extrapolated endpoint in
these loops.

## Repeated coordinates are retained by the model

Model `ObjectStroke::AddPoint`, `0x2e011c`, validates its implementation,
point-count limit and channel compatibility, then forwards the supplied
point and channels to `ObjectStrokeImpl::AddPoint`. The history-enabled and
history-free calls are at `0x2e0398` and `0x2e0554`. The latter forwards X/Y,
pressure, timestamp, tilt and orientation directly at `0x2e0538`–`0x2e0554`.

The implementation at `0x2e9d6c` has two storage paths:

| Storage | Operation |
| --- | --- |
| Temporary arrays present | Store X/Y at current count in member-168 array, then the associated channels (`0x2e9de0`–`0x2e9e0c`) |
| Ordinary arrays | Append X/Y to the member-48 vector and append pressure/time plus supported tilt/orientation (`0x2e9e14`–`0x2ea1b8`) |

Both reach the single increment of member 36 at `0x2ea1bc`–`0x2ea1c8`.
Neither compares incoming X/Y with the preceding point. Their vector growth
and temporary-array capacity handling do not insert an additional logical
point. The public count guard rejects an append when the count is already
65535 at `0x2e019c`–`0x2e01a8`.

`GetPointCount`, `0x2dfa98`, returns member 36. `GetPoint`, `0x2dfa28`, first
materializes temporary data through `CopyTempPointToRealPoint`, `0x2ea5bc`.
That routine copies the logical count of coordinates and channels, then
clears the temporary arrays. Its X/Y source range at `0x2ea624`–`0x2ea634`
ends at `start + count * 8`; it adds no final coordinate.

The binary writer, Model `ObjectStrokeBinaryHandler::GetBinary`,
`0x2ebe38`, writes the two count bytes directly from member 36 at
`0x2ebe88`–`0x2ebea0`. Its compressed branch passes that count to
`sm_ReduceStroke` at `0x2ebed0`; its uncompressed branch copies exactly
`count * 8` coordinate bytes at `0x2ebf18`–`0x2ebf28`. The writer therefore
does not impose a minimum of two or append a point to the array count.
The separate [channel encoding trace](stroke-rendering-findings.md)
describes the compressed representation.

## Marker2 bypasses the optional coordinate replacement

After appending the current sample, the recorder asks the ordinary drawable
for an optional provider through slot 88 at Drawing `0xb7f48`. On action 1,
if that provider exists, its slot 40 returns a coordinate vector at
`0xb7fa0`. A nonempty vector is passed to `ObjectStroke::ReplacePoint` at
`0xb7fe8`, after which provider slot 16 is called with false at `0xb7ffc`.
That is a real post-append replacement path; its provider semantics need
separate investigation for pens that implement it.

Marker2 V1/V2 do not enter it:

| Binding | V1 | V2 |
| --- | --- | --- |
| Primary drawable vtable address point | `0x2eda8` | `0x2eee8` |
| Slot-88 relocation | `0x2ee00` | `0x2ef40` |
| Resolved implementation | `0x20a5c` | `0x20a5c` |

The resolved implementation is just `mov x0, xzr; ret`. It returns no
provider. The constructor vtable bindings and both relative relocations
agree, so this result does not depend on guessing a method name from its
slot number.

`GetStrokeInfo`, Drawing `0xb8098`, hands out the recorded object and bounds.
Its optional release branch clears the recorder's object pointer and bounds
at `0xb80e8`–`0xb80f8`; it does not add a terminal point. Later controller,
transformation, import and model-insertion paths remain distinct boundaries.

## A down/up tap can contain two equal points

For a successful ordinary recording with a down event and an up event at
the same coordinate, with no intervening history or movement, the traced
append path stores two equal X/Y entries. Their pressure and timestamp
channels remain separate. This explains how a tap can reach Marker2's
redraw wrapper with a usable historical first point even when its geometric
extent is zero.

The [replay wrapper](marker2-sampling-findings.md#stored-point-replay-adds-no-terminal-extrapolation)
puts the first entry in history and the second in the current channel.
Marker2 emits its initial stamp from the historical entry; the current
entry has zero movement and is rejected by `drawLine`'s distance filter.
This is a derivation from the recovered control flow, not a measured tap
fixture or a claim that every Samsung tap has exactly two samples.

There is also no universal two-point model invariant. Model's array-based
`ObjectStroke::Construct`, `0x2ddbf8`, enters its copy path for a count of
1 at `0x2ddd60`–`0x2ddd64` and stores the supplied count at `0x2dddd4`.
The model can consequently hold a one-point object. Importers and direct
object construction must be investigated separately from a completed touch
sequence; parsers should not reject a stored one-point record merely to
match the common touch path.

## Replay resets the input source

Drawing `checkUncommonPenType`, `0xb7c30`, checks tool type 2 and source
`0x1002`. When both match, it overwrites every historical pressure with 0.5
at `0xb7c88` and current pressure with 0.5 at `0xb7cb0`. `OnTouch` calls
this before normal drawing and recording at `0xb7420`. It changes pressure,
not coordinates or tool type.

Marker2's [sampling flag](marker2-rendering-findings.md#shared-point-generation)
also treats that tool/source combination specially: the alternating skip
branch is enabled for tool 1, or for tool 2 with source `0x1002`.

The stored-array replay constructor has different source handling. Base's
array constructor, `0xbfd84`, calls the default MotionEvent constructor at
`0xbfdd4`. The default constructor writes source member 8 to zero at
`0xbf46c`. The array constructor installs its supplied tool type, arrays
and timestamps, but never assigns a source. Base `GetSource`, `0xc0a9c`,
reads that same member.

Consequently, for ordinary Marker2 replay through the ObjectStroke wrapper:

| Input | Alternating skip flag |
| --- | --- |
| Live tool 1, any source | Enabled |
| Live tool 2, source `0x1002` | Enabled |
| Live tool 2, another source | Disabled |
| Replayed stored tool 1 | Enabled |
| Replayed stored tool 2 | Disabled |

Other ordinary tool values leave this flag disabled. Both Marker2 versions
use the same ObjectStroke wrapper and point-generation logic. The source
reset can therefore change which samples pass the alternating filter
between this live input path and stored-object redraw. A pressure value of
0.5 does not justify inventing source `0x1002` during replay.

## SDK implications and validation

Preserve repeated coordinates and their parallel channels in the decoded
model. Deduplicating them can destroy tap replay context and alter the
input sequence consumed by other pens. Keep stored input samples distinct
from generated mask stamps, and use the recovered stored-array replay
rules when reproducing document export.

The APK digest and all four library byte streams were verified. Constructor
bindings, provider relocations, append count updates, writer count accesses
and source initialization were checked against their instructions. The
down/up and source-flag cases are static derivations; no SDK rendering code
changed and no new device fixtures were used.

Next APK targets include event preprocessing before `TouchStrokeDrawing`,
the nonnull coordinate provider, later stroke insertion/transformation and
single-point import handling. Tap and short-stroke SDOCX/PDF pairs can test
the stored counts, repeated coordinates, tool types and resulting marks.
