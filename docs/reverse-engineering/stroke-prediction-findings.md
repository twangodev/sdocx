# Stroke presentation, prediction and recorded samples

## Evidence and scope

Confirmed by static inspection of Samsung Notes 4.4.45.37 ARM64
`libSPenComposer.so`, `libSPenDrawing.so` and `libSPenMarker2.so` from the
APK identified in the [knowledge base](README.md#sources-and-validation).
This continues the [pen-action input trace](stroke-input-findings.md)
through the raster stroke view into its presenter.

Ordinary Marker2 recording and prediction rendering use different event
paths. The presenter forwards real events to the touch recorder, while
prediction draws use a separate drawable and bitmap. Marker2 V1 exposes no
prediction drawable; V2 exposes one. The recovered paths do not justify
adding a predicted tail to stored-array replay.

## The stroke view forwards a list of real events

The previously resolved `LowLatencyStrokeView` primary vtable starts at
`0x580c78`. Its slot 136, relocation `0x580d00`, resolves to `0x4d33e0`.
This method puts the incoming event pointer into a one-element list at
`0x4d35f4`–`0x4d3618` and passes it to the helper at `0x4d36ec`.

That helper builds a presentation payload. Its ordinary payload builder,
slot 376 at `0x4d3304`, stores the first event at offset 0; the caller
stores the list pointer at offset 8 at `0x4d3f88`. Pen data and the drawing
bitmap are separate payload fields. A nonnull member-296 provider can
replace this payload builder, so the ordinary builder is an explicit
scope boundary.

The stroke view's member 72 contains a `TouchPresenter`, constructed by
`0x4d6f20` and assigned at `0x4d1fb0`. The presenter constructor loads GOT
entry `0x5a2bd0`, installs primary vtable address point `0x580f90` and
creates the member-40 `TouchStrokeDrawing` at `0x4d7044`–`0x4d7048`.
RTTI identifies `TouchPresenter`; its slot 16 at `0x580fa0` resolves to
`0x4d76bc`. The stroke view calls this slot with the payload at `0x4d4024`.
An embedded method-signature string identifies the implementation as
`TouchPresenter::PresentTouch(PresentTouchData&)`.

`PresentTouch` tests the ordinary pen drawable's `IsTip` slot 96 at
`0x4d7844`–`0x4d7854` and saves the result in presenter byte 8. Ordinary
Marker2 V1/V2 return false here, as established in the
[recording findings](stroke-recording-findings.md#drawing-and-recording-are-separate-operations).

For that false result, the real-event path reaches `0x4d7b98` and calls
the list-drawing helper at `0x4d7ba8`, with a null secondary event. This
selection is independent of whether prediction is otherwise available.

The helper at `0x4d8acc`, identified by its signature string as
`TouchPresenter::DrawStroke`, iterates the supplied list in order. At each
iteration it passes:

| Argument | Source | Call-site evidence |
| --- | --- | --- |
| Primary MotionEvent | Current list node's event pointer | `0x4d8bc8` |
| Recorder | Presenter member 40 | `0x4d8bcc` |
| Secondary MotionEvent | Helper's separate argument | `0x4d8bd4` |
| Updated rectangle | Local output rectangle | `0x4d8bd0` |

The call at `0x4d8bdc` reaches Drawing
`TouchStrokeDrawing::OnTouch(MotionEvent&, MotionEvent*, RectF*)`,
`0xb71a4`. Setting a canvas matrix and transforming output rectangles in
this helper do not themselves replace the event pointer or construct new
input samples.

## The recorder does not directly append the secondary event

Drawing `OnTouch` saves the primary argument in `x20` at `0xb71c8` and
the secondary argument in `x22` at `0xb71dc`. It passes only the primary
event to `addEventPointsToObjectStroke` at `0xb756c`–`0xb7570`.

The secondary event has a different consumer. When the ordinary drawable
reports `IsTip == true`, drawable slot 64 receives both events at
`0xb7514`–`0xb751c`, followed by slot 72 to obtain drawn bounds. When
`IsTip == false`, slot 80 receives only the primary event and the output
rectangle at `0xb7544`–`0xb754c`.

Consequently the ordinary Marker2 recording path receives neither its
prediction event as a direct append argument nor the tip-specific pair of
events. Its optional post-append coordinate provider is also null in both
versions, as established in the
[provider trace](stroke-recording-findings.md#marker2-bypasses-the-optional-coordinate-replacement).
For other pens, a nonnull coordinate provider remains a separate route
through which drawable processing could affect final stored coordinates.

## Marker2 V2 has a separate prediction drawable

There are two distinct pen interface slots:

| Slot | Marker2 relocation | Method |
| --- | --- | --- |
| 232 | `0x2ebb0` | `GetStrokeDrawableGL`, `0x1f050` |
| 248 | `0x2ebc0` | `GetStrokeTipStrokeDrawableGL`, `0x1f13c` |

The first selects the ordinary V1/V2 stroke renderer. The second clamps
the version index to 1–2, reads the same `versionTable` at `0x34b60`, then
accepts only the resulting value 2 at `0x1f170`–`0x1f178`. The table
contains 0, 1 and 2. The getter therefore behaves as follows:

| Selected version | Prediction getter result |
| --- | --- |
| V1 | Null, through `0x1f1c4`–`0x1f200` |
| V2 | Reused or newly constructed `Marker2StrokeTipDrawableGL`, held in pen member 104 |

The V2 allocation calls the constructor at `0x1f1b8` and stores the result
at `0x1f1bc`. This renderer receives shared `Marker2Data` and a GL data
manager; it is not an `ObjectStroke` constructed from predicted samples.

Drawing `IsPenSupportPredictionDraw`, `0x74e0c`, checks a separate optional
pen feature first. If that permits prediction, it returns whether pen
slot 248 is nonnull at `0x74e50`–`0x74e64`. Thus V1 cannot pass this
helper, while V2's available drawable does not by itself prove prediction
is enabled. Presenter configuration and tool support add further gates.

Drawing `SetPredictionPenBitmap`, `0x74d6c`, creates a pen canvas from the
supplied bitmap and sends it to slot 248's drawable at `0x74ddc`. The
presenter supplies its separately allocated prediction bitmap through this
helper, including the call at `0x4d8230`. The ordinary recorder's canvas
is configured separately at `0x4d77fc`.

## Prediction draws bypass the ordinary Marker2 recorder

The signature string at Composer `0x1f496a` identifies `0x4d94e0` as
`TouchPresenter::OnPredictTouch(MotionEvent*, MotionEventEntity const&)`.
It accepts a separate event, and can replace that event through a
member-528 processing interface at `0x4d96d4`. The prediction algorithm
and this processing interface still need independent numerical tracing.

For its drawable path, it retrieves pen slot 248 at `0x4d9abc`, saves the
event's original action at `0x4d9ad8`, then temporarily sets the action to
move, value 2, at `0x4d9ae8`. After checking the ordinary drawable's
`IsTip`, the false branch calls the prediction drawable's slot 72 with
this event at `0x4d9c24`. It restores the action at `0x4d9d80`.

Marker2's prediction vtable is `0x2f240`, with primary address point
`0x2f250`. Slot 72 at `0x2f298` resolves to
`Marker2StrokeTipDrawableGL::Draw`, `0x26514`. This method accepts action
2 and invokes its own `startPen` and `movePen` at `0x265ac` and `0x265bc`.
It emits GL callback buffers and drawing bounds. This dispatch does not
call the touch recorder with the prediction event as primary input.

Tip-based ordinary pens have additional behavior: `PresentTouch` can
clone real events into a presenter list at member 16, and
`OnPredictTouch` can drain that list through `DrawStroke`. One such call
at `0x4d9844` supplies the prediction event as the secondary argument,
then clears the queue at `0x4d986c`. This is why the recorder's two event
arguments must be distinguished. It is not evidence that the prediction
event itself is directly appended to the stored point array.

## Input source and later transforms remain separate boundaries

Before building the event list, the stroke-view method calls latency
configuration slot 128 at `0x4d35c0`. If true and the event's tool type
is 2, it replaces the source with `source | 0x4002` at
`0x4d35dc`–`0x4d35f0`. The default value and configuration meaning of
that flag have not been established here.

The conditional mutation matters to the
[source-specific recorder behavior](stroke-recording-findings.md#replay-resets-the-input-source):
an incoming source `0x1002` becomes `0x5002`, which no longer equals the
recorder's special source constant `0x1002`. Source tests at the recorder
must use the value reaching it, not an assumed untouched platform source.

There is also a retained-event reprojection branch. When stroke-view byte
248 is set, the method first transforms each retained event by the inverse
of its own accumulated matrix at `0x4d353c`, then applies the incoming
event's matrix at `0x4d356c`, and invokes helper `0x4d3110`. This establishes
the matrix sequence, not the complete semantics of the retained-event list.

Later presenter processing can call
`TouchStrokeDrawing::TransformStroke` at `0x4d9404`. The
[finalization trace](stroke-finalization-findings.md) resolves this as
optional processing whose constructor default is disabled. Its CSAPS
implementation can replace recorded X/Y while retaining the original count
and parallel channels. This is separate from the still-unresolved
document-insertion coordinate transform.

## Validation and SDK implications

The APK digest and all three library byte streams were checked. Vtable
bindings, RTTI, the version table, event argument registers and the source
bitwise operation were verified against the binary. The source example
is a static derivation; no device prediction trace or new document pair
was used, and no SDK code changed.

Keep ordinary Marker2 stored-array rendering distinct from prediction
presentation. Preserve recorded samples and per-object boundaries, use
the recovered ordinary version's replay rules, and do not manufacture
future samples to imitate the temporary live tail. Predictions, final
coordinate transforms and pen-specific replacement providers should be
validated independently as more of their native paths are recovered.
