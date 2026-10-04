# Touch stroke recording and replay inputs

## Evidence and scope

Confirmed by static inspection of Samsung Notes 4.4.45.37 ARM64
`libSPenDrawing.so`, `libSPenModel.so`, `libSPenBase.so` and
`libSPenMarker2.so`, with the voice-session trace also using `libSPenWDoc.so`
and `libSPenComposer.so`, from the APK identified in the
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

The [Android adapter trace](motion-event-adapter-findings.md#millisecond-getters-subtract-down-time)
resolves those timestamp getters: they return sample time minus the event's
down time. The append loop uses this millisecond channel, not the separate
nanosecond getters, and does not subtract the first recorded timestamp.

## Straight-stroke creation materializes new channels

This source-only producer trace additionally uses ARM64 `libSPenPenCommon.so`
(SHA-256 `afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d`)
and `libSPenEngine.so`
(`79a8024586ce58ceeca613ddfce159e92274c597c5a863dd0aaf3e8e32e1b505`).
No gesture or complete saved-file roundtrip was executed; appearance parity remains unproved.

Engine's installed callback `0x13c754` belongs to `WritingViewStraightStrokeAction`:
constructor `0x13c3dc–0x13c3f0` installs the vtable whose `0x17e0a0` entry targets
it; typeinfo `0x187ba0` names the class at `0x72251`. Its actual builder chain
`0x13cc5c–0x13cd44` supplies current PenData, stored start XY,
MotionEvent end XY narrowed to float, screen rectangle, note dimensions,
alignment=true and density DPI; full UI selection remains unproved here.

PenCommon `StraightStrokeBuilder::CreateStroke`, `0x58abc`, derives a short-distance
threshold from `DPI / 160 * 5`, runs alignment when enabled and calls `createStroke`, `0x57fd0`.
Alignment can change an endpoint to starting X or Y (`0x57fa0–0x57fb4`). A fresh
ObjectStroke receives the current pen name/style and tool type 0 (`0x58038`,
`0x580d8`). The loop `0x58424–0x58480` supplies generated linear XY to AddPoint
at `0x58474`, with pressure `0.5`, tilt f32 bits `0x3f2e147b` (about `0.68`)
and orientation `0xbd4ccccd` (about `-0.05`). Timestamp stages use `logf(i+1)`,
division by double `log(n+1)`, scaling by n, narrowing to float, multiplication
by 6 and integer truncation (`0x583f8–0x5846c`). For subdivision count n≥5 it
attempts n+1 additions, without copying an original freehand sample history.
The short TapePen branch holds Y at starting Y (`0x583c8–0x583e8`, `0x58450`).

`convertAngledObjectStroke`, `0x585e4`, transforms generated XY about its midpoint,
calls ReplacePoint (`0x58874`) and then SetRotation through slot 120 (`0x58888`,
Model relocation `0x493098`). Replacement changes the XY vector, not the other
channels. SetRotation is not metadata-only: `0x2de6d8` updates common angle and
`0x2de714` invokes SetRotationPoint, which changes actual XY. The established
[writer connection](object-transform-findings.md#double-wire-coordinates-do-not-imply-double-editing-geometry)
serializes mutated samples without undoing rotation; this establishes no additional
export rotation. AddPoint rejects count
65,535 (`0x2e019c–0x2e01a8`); this producer ignores append/replacement/rotation
returns, so attempted additions are not an unconditional retained-count guarantee.

Both producer stages call SetAsShape(true) (`0x58100`, `0x58894`), without a
SetStraighten call in these bodies. On a changed value, Model SetAsShape writes
member 380 (`0x2e2684`) and its true branch stores integer 1 under
`extra_key_stroke_shape` (`0x2e2698`, literal `0x13ecbd`) in common ExtraData.
The [typed Bundle field 5](object-flexible-findings.md#modern-typed-frame-field-order)
is separate from straighten member 493/property bit 13. Ordinary and modern
loaders restore member 380 from key==1 (`0x2e4f1c`, `0x2e5938`). No replay
straighten instruction was established by the examined adapters/getter callers,
without a claim about every plugin or flag consumer.

Rust's `ObjectFlexibleMetadata.extra_data` supports typed Bundle integers, while
`PageObjectContent::Stroke` does not carry every common metadata field. Generated
channels, the common/source inspection and original record/page bytes remain
separate preservation inputs. The straighten flag cannot reconstruct alignment/DPI inputs
or an earlier gesture, and establishes no second generation pass.

## Nonline recognition creates generated strokes before private admission

This distinct source-only Engine path starts with recognition geometry;
it does not establish a completed recognition gesture or saved-file roundtrip.
AutoResultToStrokeConverter::ConvertShapeResultToStroke, `0xcd4a8`, obtains a
ShapePath and samples native segments into a temporary PointF vector.
buildBezier dispatches quadratic/cubic/arc segments (`0xcda80`, `0xcdaf4`,
`0xcdb18`); its named createStroke helper, `0xce31c`, appends XY rather than
constructing ObjectStroke. Slot 48 at `0xcd7f0` supplies the generated vector,
recognition angle and closed flag to IStrokeShapeObject.

One concrete binding is StrokeShapeObject: its factory admits type 1
(`0xd1f8c–0xd1f94`), and vtable relocation `0x179b98` resolves that slot to
CreateObject, `0xd1a34`. This creator supplies copied generated XY, pressure
`0.5` (`0xd1b28`, `0xd1b54`) and timestamp `0` (`0xd1b2c`, `0xd1b5c`) to a fresh
ObjectStroke. Model Construct at `0x2ddf18` passes null tilt/orientation arrays;
the supplied-array branch skips their insertion (`0x2ddda8` -> `0x2dddd4`).
These are absent supplied/copied stylus samples, not recovered original zeros.
Construct rejects counts >=65,536 (`0x2ddc54–0x2ddc58`), and the creator checks
its return (`0xd1bcc`), so sampling does not guarantee successful construction.

On success it sets tool type 1 (`0xd1bd8`), disables curves for closed results
(`0xd1c0c`), uses current PenData name/size/color/advanced settings, enables
fixed width, sets common alpha 1 and marks AsShape (`0xd1c44–0xd1cb4`). It does
not copy the original pressure/time/stylus channels or UUID in this body.
applyRotation (`0xd1b98`, implementation `0xd1de8`) conditionally changes actual
XY and its bounds before Construct; this is no additional export transform.

AutoRecognitionHandler's nonline branch adds a nonnull created object to the
result ObjectList (`0xcc65c–0xcc670` -> converter `0xccb14`). The transformer
hands its own result list to a callable (`0xcf254–0xcf288`). This is temporary
result admission, without establishing a PageDoc/LayerDoc append or removal.
A concrete StrokeShapeView constructor lambda is installed at `0x115c80–0x115c9c`;
RTTI `0x186858`, name `0x70de7`, identifies its ObjectList callback, and
relocation `0x17c7f0` resolves slot 48 to `0x116b84`.

Its guarded callback selects nonnull results into private member 688 (`0x116c04`),
calls a geometry helper and SetRect(false) (`0x116c58`, `0x116cb0`), then stores
the selected result in member 640 (`0x116e0c`). These later edits mean factory
XY are not established as final saved coordinates or document-space geometry.
The view's separate input path copies a stroke into member 720 (`0x11681c`),
adjusts its bounds, wraps it and requests recognition (`0x116850–0x1168c4`).
Private input copy and generated result are distinct objects; their existence
does not establish coexistence or replacement in the source document.

The callback's next slot 128 is View::Invalidate: the installed address point
`0x17c2b8` maps relocation `0x17c338` to that import (`0x116e14`). Its following
function wrapper at member 528 is also bound: the actual view creator
`0x164b58–0x164b94` installs a captured WritingView lambda, whose slot relocation
`0x182328` resolves to `0x1669c4`. It forwards to the captured WritingView's optional
callable (`0x1669c8–0x1669d8`), initially null in its constructor (`0x161768`).
The next concrete callable is not established by this inspected caller chain.

If a generated stroke is stored, its admitted channels and current typed style/
common metadata are source. Reuse the Rust channels and retain original record/
page bytes; its outline or AsShape flag cannot restore an earlier gesture.
Final document order, UUIDs, hidden/history originals and save membership remain
unproved beyond the optional parent callback; no second generator is implied.

## Voice synchronization uses append time and original objects

Voice actions and the checked vector append producer share Base `GetTimeStamp`,
`0x9a1f0`: `clock_gettime` with clock ID 0 returns realtime microseconds as
seconds × 1,000,000 + nanoseconds / 1,000 (`0x9a208–0x9a254`). WDoc
`AddVoicePropertyList`, `0x8ebe0`, stores that result directly. Composer's
recorder callbacks invoke Start/Pause/Resume/Stop (`0x43fa40`, `0x43fd54`,
`0x43fdf4`, `0x43fb90`); their timestamps belong to callback execution,
not merely a recording request. Model's selected history insertion branch
calls GetTimeStamp → SetAppendTime (`0x359ebc–0x359ec8`); fallback insertion
does not stamp it. The setter can preserve an existing nonzero value when
impl+132 is set (`0x2cc6e4–0x2cc708`), so admission need not retime an object.

This signed 64-bit append time is saved [optional field 13](object-flexible-findings.md#modern-typed-frame-field-order),
separate from common signed 32-bit replay time and relative sample milliseconds.
Painting's actual common-time producer, `m_RecordObject`, derives a handler
counter from clock deltas, incrementing small deltas and dividing larger
microsecond deltas by 10,000 (`0x363778–0x363814`); stroke duration further
adjusts the assigned value (`0x363828–0x363894`). The voice selector instead
calls GetAppendTime (`Composer 0x448830`), without merging these clocks.

`NoteVoiceReplayController::Init`, `0x447134`, takes the voice manager's current
VoiceData. Playback callbacks validate its ModelContext NoteCookie against the
WNote runtime handle (`WDoc 0x9a0e4–0x9a104`): note ownership, without a saved
per-object voice ID list in this selector. Init derives the session start and
pause-gap accumulator from saved actions, then adds media length × 1,000
(`0x4472b8–0x447360`). Update likewise converts media progress milliseconds
to microseconds (`0x4475bc–0x4475c4`); `updateBaseTime`, `0x448138`, compensates
resume-minus-pause gaps and can rewind the base/traversal when crossing a pause.

`updateObjectList`, `0x448318`, reads original WNote page object lists;
`getAlphaObjectList`, `0x448728`, skips BodyText, selected, hidden and
nonpositive-append-time objects. Effective base + progress strictly greater
than append time queues nonunit-alpha objects for restoration to alpha 1.
The other side queues dimming
only for unit-alpha objects strictly before session start + accumulator;
equality remains on the future side. Locked runtime handles are excluded.
These temporary lists contain original pointers, not regenerated vectors or
authoritative saved session membership. BodyText is dispatched separately.

The page applies runtime alpha; Model's true SetAlpha branch stores data+168
(`0x352e60–0x352e6c → 0x2d1b60–0x2d1b68`). It does not literally leave runtime
objects immutable, but this controller does not remove objects or rewrite their
sample channels, append/replay times, visibility flag or saved order. Reset
restores eligible alpha to 1 (`0x447fc8–0x448034`). Init's null attached-file
path exits without media-duration setup (`0x447318 → 0x4473d0`), without deleting
vectors; a nonnull path does not certify an existing/decodable audio file.
Rust exposes [voice records](note-metadata-findings.md#sized-records) and separate
common/flexible timestamp inspection; PageObject does not carry those clocks
or an automatic voice-session source map. Retain original page bytes. Full
reopened playback, BodyText effects and missing-file/player failures remain
unexecuted; alpha lists must not filter source or define universal session ownership.

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
This establishes a post-append replacement path. The provider semantics
for other pens are not established by this trace.

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
The model can consequently hold a one-point object. The common completed-touch
sequence does not establish a minimum stored count of two.

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

The APK digest and all four recorder library byte streams were verified. Constructor
bindings, provider relocations, append count updates, writer count accesses
and source initialization were checked against their instructions. The
down/up and source-flag cases are static derivations; no device fixtures were
used.

The [pen-action input trace](stroke-input-findings.md) establishes a
separate InkPen2 filter and an ordinary long-gesture split at 65501 recorded
points. The [presentation trace](stroke-prediction-findings.md) separates
ordinary Marker2 recording from prediction drawing, and the
[finalization trace](stroke-finalization-findings.md) identifies an optional
coordinate replacement that preserves the count and parallel channels.
The [insertion trace](stroke-insertion-findings.md) resolves first-point
page selection and page-offset translation. The
[view-input trace](view-input-transform-findings.md) records transforms before
recording. Nonnull coordinate providers and single-point import handling are
outside this trace. The synthetic tap cases do not establish device-exported
appearance.
