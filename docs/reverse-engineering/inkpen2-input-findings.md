# InkPen2 input queue and result filtering

## Evidence and scope

Confirmed by static inspection of Samsung Notes 4.4.45.37 ARM64
`libSPenPenCommon.so`, `libSPenComposer.so` and `libSPenBase.so` from the
APK identified in the [knowledge base](README.md#sources-and-validation).
The [pen-action trace](stroke-input-findings.md#the-handwriting-filter-selects-inkpen2)
establishes the caller: enabled handwriting beautification for the exact
InkPen2 name and tool type 2 or 6. Marker2 does not enter that branch.

`PointBeautifier` has its own sample queue, time checks and pressure cap
before it produces a replacement event. This trace establishes queue
admission and result routing. Separate traces recover numerical
[prediction](inkpen2-prediction-findings.md) and
[Kalman filtering](inkpen2-kalman-findings.md); the input queue is not
equated with final stored stroke points.

## Move events select history or current input

PenCommon `PointBeautifier::OnTouch`, `0x5c028`, saves the input's event
metadata, including down time at `0x5c080`. It calls
`addRealPenEvent(MotionEvent*, int)`, `0x5c32c`, using different sample
selection for each ordinary action:

| Action | Selected input | Evidence |
| --- | --- | --- |
| Down, 0 | Current sample after resetting the queue | `0x5c378`, `0x5c4a0`–`0x5c54c` |
| Move, 2, with history | Each historical sample in ascending index order | `0x5c1cc`–`0x5c208` |
| Move, 2, without history | Current sample, using index -1 | `0x5c20c`–`0x5c224` |
| Up, 1 | Current sample | `0x5c384`–`0x5c448` |

The move-with-history branch does not additionally enqueue the current
sample in this method. Down and up use their current sample irrespective
of the supplied index; their paths do not loop over event history.

After each historical admission attempt, `OnTouch` calls `doPredict()` at
`0x5c1f4`, even if admission returned false. The no-history move path also
calls it after the current-sample attempt at `0x5c22c`. Down/up call it
after successful admission at `0x5c26c`.

These rules describe the beautifier's internal input stream. They do not
mean that an InkPen2 move necessarily produces the same number of output
points, or that the ordinary recorder discards the current sample from
every Android event. The recorder receives this path's result or fallback
event, as described below.

## Queue records retain both time channels

The internal `HistoricalEvent` is 56 bytes:

| Offset | Representation | Field |
| --- | --- | --- |
| 0 | 32-bit integer | Action |
| 8 | 64-bit integer | Relative millisecond time |
| 16 | 64-bit integer | Nanosecond time |
| 24, 28 | Floats | X, Y |
| 32 | Float | Tilt |
| 36 | Float | Pressure |
| 40 | Float | Orientation |
| 44, 48 | Floats | Minor, major |
| 52 | 32-bit integer | Resampled state |

This is an in-memory preprocessing record, not a serialized SDOCX layout.
The down path explicitly writes zero into its millisecond field at
`0x5c5e8`, alongside the supplied nanosecond time. The up path uses both
current time getters at `0x5c39c` and `0x5c3a8`. Historical/current move
paths use their matching getters at `0x5c654`/`0x5c664` and
`0x5c720`/`0x5c72c`.

The [native adapter trace](motion-event-adapter-findings.md#millisecond-getters-subtract-down-time)
establishes that these millisecond getters subtract down time, while the
nanosecond getters retain their separate time origin. Queue reconstruction
does not interchange the channels.

## Move admission rejects older milliseconds and caps pressure

For non-down/up input, `addRealPenEvent` obtains the last queued record's
millisecond time from offset 8 at `0x5c53c`. With an empty queue it uses
-1 instead, at `0x5c620`.

For a historical candidate, the signed comparison at `0x5c638` rejects
it only when the preceding queued time is greater. For a current candidate,
`0x5c700`–`0x5c704` implements the same rule. Rejection returns false
at `0x5c708`. Equal timestamps are admitted, and this decision does not
compare nanosecond times or X/Y equality.

For example, after a queued time of 12, candidate times 11, 12 and 13
are respectively rejected, accepted and accepted. The initial down and
up paths bypass this move-time comparison. This is not a universal
monotonicity invariant for every queue entry or serialized stroke.

Every ordinary admission path reads pressure and executes `fminnm` with
1.0 before enqueuing:

| Sample path | Pressure operation |
| --- | --- |
| Down | `0x5c5ac` |
| Up | `0x5c3f8` |
| Historical move | `0x5c6bc` |
| Current move | `0x5c77c` |

For finite pressure this is `min(pressure, 1)`. There is no lower clamp
in this admission routine: -0.25 remains -0.25, 0.5 remains 0.5 and 1.25
becomes 1.0. Those are instruction-derived examples, not assertions about
values emitted by a particular device. Tilt and orientation are copied
without an axis conversion here. The resampled value is copied into
offset 52; this admission decision does not reject it for being 1 or -1.

The deque append helper at `0x5d44c` copies all 56 bytes at
`0x5d63c`–`0x5d648` and increments its logical count at `0x5d650`–`0x5d654`.
Move/up admission then trims old entries while count exceeds 11, through
`0x5c44c`–`0x5c498` or `0x5c7d0`–`0x5c81c`. This is the beautifier's
working queue size, not a maximum stroke length or point-count field.

## Result filtering and the no-result fallback differ

`PointBeautifier::SetFilterEnabled`, `0x5bfd4`, writes byte 192 and creates
a `PenKalmanFilter` in member 200 when enabling an absent filter, at
`0x5c000`–`0x5c004`. The pen-action constructor enables it at Composer
`0x4223a0`.

`GetResult`, PenCommon `0x5cfe8`, first obtains a replacement event through
`getPredictedPenEvent()` at `0x5cffc`. For a nonnull result, an enabled,
nonnull Kalman filter processes it at `0x5d018`; the returned event replaces
the previous result. The function applies its saved matrix at `0x5d034`.
Composer then applies the original event's accumulated matrix at
`0x422e20` and dispatches the resulting event to its drawing interface
at `0x422e6c`.

The event-construction helper, `initNewPenEvent`, `0x5d354`, combines the
saved down time with the queued relative milliseconds at `0x5d3b4`,
while retaining nanoseconds. It calls the native pointer-array MotionEvent
constructor at `0x5d3f0`. The reconstruction therefore restores the
millisecond getter's expected input origin rather than treating the
relative queue value as an absolute event time.

When `GetResult` supplies no event, Composer's down/up fallback instead
calls `ApplyFilter` on its copied input at `0x422ee0`, then dispatches the
original input argument at `0x422ef4`. PenCommon `ApplyFilter`, `0x5d304`,
calls the enabled Kalman filter at `0x5d32c` and destroys any returned
temporary event at `0x5d338`–`0x5d340`. It returns a boolean, not that
event. The fallback thus does not dispatch the Kalman call's return value.
No-result move input receives no fallback drawing dispatch in this branch.

This result processing occurs before model recording for the selected
InkPen2 path. It is separate from Marker2's
[temporary prediction drawing](stroke-prediction-findings.md). Method
names containing prediction do not establish whether their output is
temporary presentation or input to the recorder; the caller determines
that boundary.

## Validation and remaining work

The APK digest and all three library byte streams were verified. Named
imports, action branches, both time fields, pressure instructions, queue
copies and result/fallback argument identities were checked against the
ARM64 code. Disposable reconstruction checked admission equality, the
finite pressure examples and queue trimming. No SDK code changed and no
new device fixture or native execution was used.

The [prediction trace](inkpen2-prediction-findings.md) now recovers
`doPredict`'s linear fits, horizon, distance rejection and timestamp
retention. The [Kalman trace](inkpen2-kalman-findings.md) recovers channel
defaults and correction equations. The
[result trace](inkpen2-result-findings.md) establishes geometric selection,
resampled-state rewriting and candidate lifetime. Synthetic queue cases can
bound those algorithms, but actual InkPen2 SDOCX/PDF pairs are still needed
to test the complete stored geometry and rendered appearance. The SDK
should preserve already decoded points and channels rather than applying
these live-input queue rules again during export.
