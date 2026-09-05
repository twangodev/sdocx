# Neural horizon selection and callback event construction

## Evidence and scope

Confirmed by static inspection of Samsung Notes 4.4.45.37 ARM64
`libSPenPredictor.so` and `libSPenBase.so` from the
[identified APK](README.md#sources-and-validation).
This extends the [output conversion trace](neural-output-findings.md)
through horizon selection and construction of the callback event.

The selected horizon is determined by integer-millisecond comparisons.
The predictor can retain unselected candidate records internally while
including only marked candidates in the delivered current/history event.

## The explicit maximum-time setter uses milliseconds

`NNPredictor::SetMaxPredictionTime`, `0x25564`, obtains the current
model and writes the requested multi-output byte at object offset 1796
through `0x255b8`. It divides the first and last configured horizons by
1,000 at `0x255e0` and `0x2560c`.

For finite inputs, the comparisons at `0x25618`–`0x25638` clamp the
supplied float to that millisecond range and store it at member 1792:

| Model | Minimum explicit limit, milliseconds | Maximum explicit limit, milliseconds |
| --- | --- | --- |
| M16 | `f32(16.0)` | `f32(16.0)` |
| M20 | `f32(5.6)` | `f32(19.4)` |
| M22 | `f32(6.3)` | `f32(22.9)` |

`GetMaxPredictionTime`, `0x25688`, and `GetPredictionTime`,
`0x25690`, both return member 1792 unchanged. `IsMultiOutput`,
`0x25698`, reads member 1796.

The constructor and model-length setter use a different initialization:
`0x24f9c`–`0x24fa0` and `0x25464`–`0x25468` copy the last raw
microsecond horizon into member 1792 without division. They also clear
the multi-output byte. The explicit maximum-time setter therefore cannot
be assumed to have run from the field's presence alone. With these raw
default values, the selector below chooses the last configured horizon.
The order of application configuration calls remains a device-level question.

## Selection compares truncated whole milliseconds

`PredictionTask::Run` scans configured horizons backward at
`0x2cbe0`–`0x2cc18`. For each index `i`, it compares:

```text
horizon_ms = trunc_i32(f32(PredictTime[i] / 1000))
limit_ms = trunc_i32(GetMaxPredictionTime())
```

`0x2cc04` and `0x2cc10` perform the two signed float-to-integer
conversions. The scan stops at the highest index whose `horizon_ms`
is less than or equal to `limit_ms`. For the bundled positive horizons,
this loses each value's fractional millisecond before comparison.

Let `last` be that index. The multi-output check at
`0x2cc24`–`0x2cc3c` chooses:

```text
first = 0 if multi_output else last
selected index range = [first, last]
```

For M20's 5.6, 11.1 and 19.4 ms horizons:

| Limit passed to the selector | Last selected index | Selected horizon | Single-output indices | Multi-output indices |
| --- | --- | --- | --- | --- |
| 5.6 ms | 0 | 5.6 ms | `[0]` | `[0]` |
| 10.999 ms | 0 | 5.6 ms | `[0]` | `[0]` |
| 11.0 ms | 1 | 11.1 ms | `[1]` | `[0, 1]` |
| 18.999 ms | 1 | 11.1 ms | `[1]` | `[0, 1]` |
| 19.0 ms | 2 | 19.4 ms | `[2]` | `[0, 1, 2]` |

Thus an explicit 19.0 ms limit can select the 19.4 ms horizon. This is
the recovered selection rule, not a continuous cutoff at the exact float
limit. The separately traced
[Composer time-fraction cutoff](uniform-latency-findings.md) is another stage.

## Selection is stored as a candidate byte

The task initializes candidate byte 73 to 1. At
`0x2cf70`–`0x2cf88`, it clears that byte for indices outside the
selected range. It still calls `AddPredictedPoint` at `0x2cf98` for
an unselected candidate that passes the other per-candidate gates.

The output-processing loop begins at index 0. Its upper bound is
`OutputSize - task_member_112`, tested at `0x2cce0`–`0x2ccec` and
`0x2d020`–`0x2d038`. The task constructor zeroes member 112 as part
of its eight-byte store at `0x2be54`; function `0x2dc04` can overwrite
that member. Callers and semantics of that overwrite remain untraced.

These are separate controls: the loop bound limits which outputs are
processed, while byte 73 marks which processed candidates belong in the
callback event. Additional rejection can also prevent an append.

## The last marked candidate becomes the current point

`PredictorBase::GetPredictedPenEvent`, `0x313e0`, locks the real-point
critical section and returns null for an empty prediction vector at
`0x31420`–`0x31428` and `0x31b38`.

For a nonempty vector, it scans backward for a nonzero byte 73 at
`0x31450`–`0x31470`. That candidate is passed to
`InitNewPenEvent` with action value 2 at `0x314f0`–`0x31500`.
Earlier records are visited in vector order; only those with byte 73
set are passed to `AddHistoricalPoint` at `0x3151c`–`0x31530`.

For a complete M20 candidate set and a 19.0 ms selector limit:

| Mode | Current candidate | Historical candidates |
| --- | --- | --- |
| Single output | Index 2, 19.4 ms horizon | None |
| Multiple outputs | Index 2, 19.4 ms horizon | Indices 0 and 1, 5.6 and 11.1 ms horizons |

This assumes those candidates survive the other gates. The function also
copies the final vector record, including an unmarked one, to base member
224 at `0x31540`–`0x31554` for separate retained state.

An all-unmarked nonempty vector leaves the backward scan index at -1.
There is no separate rejection between that scan and the address calculation
at `0x314f4`. This establishes an edge requiring a caller-invariant audit;
it does not establish that normal application settings reach it.

## Event builders restore the millisecond time base

Base `MotionEvent::GetDownTime`, `0xc0628`, reads private offset 16.
`GetEventTime`, `0xc0634`, loads the current pointer coordinate's
stored millisecond timestamp and subtracts that down time at `0xc0648`.
`GetEventTimeNano`, `0xc0650`, returns its stored nanosecond field
without that subtraction.

The predictor stores the relative millisecond value in record offset 16
and the nanosecond value in offset 24. Base-object member 48 comes from
`GetDownTime` at Predictor `0x2f120`–`0x2f124`; `AddRealPoint`
copies it to retained-record offset 8 at `0x30530`–`0x30540`.
Output candidates preserve offset 8 and add their horizon independently
to offsets 16 and 24.

`InitNewPenEvent`, Predictor `0x31c7c`, constructs a one-pointer event
with the predictor's current tool type. At `0x31cd8`–`0x31ce0`, it
restores the absolute millisecond coordinate time as:

```text
event_coordinate_ms = record.down_time_ms + record.relative_time_ms
event_coordinate_ns = record.time_ns
```

It passes `record.down_time_ms` separately to the `MotionEvent`
constructor at `0x31cfc`–`0x31d14`. XY is widened from float to
double; orientation, pressure and tilt populate the pointer-coordinate
structure from the copied candidate channels.

`AddHistoricalPoint`, Predictor `0x31d68`, performs the same
millisecond addition at `0x31db0` and passes the separate nanosecond
field to `MotionEvent::AddBatch` at `0x31db4`.
Base `AddBatch`, `0xc01b4`, allocates a 72-byte historical coordinate
record and appends its pointer to the separate history vector at private
offsets 80/88/96 through `0xc0258`–`0xc02e0`. It does not replace
the current coordinate used by the event-time getters.

The completed event then follows the previously recovered
[consumer callback and lifetime path](predictor-callback-findings.md).

## Validation and remaining work

Both native byte streams were matched to the identified APK. Maximum-time
stores, float conversions, index comparisons, candidate-byte handling,
event builders, Base time getters and history-vector writes were checked
against ARM64 instructions.

Disposable calculations checked explicit-limit clamping, default selection,
fractional-millisecond boundaries, single/multiple output ranges and the
independent horizon timestamp conversions. The reference is not native
execution or device conformance evidence.

Next work is task expiry, candidate rejection, the member-112 setter's
callers and the invariant ensuring a nonempty prediction vector contains
a marked candidate. No SDK code or corpus fixture changed.
