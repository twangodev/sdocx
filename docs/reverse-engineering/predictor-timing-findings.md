# Prediction callback timing producers

## Evidence and scope

Confirmed by static inspection of Samsung Notes 4.4.45.37 ARM64
`libSPenPredictor.so`, `libSPenComposer.so` and `libSPenBase.so` from
the APK identified in the [knowledge base](README.md#sources-and-validation).

The [callback trace](predictor-callback-findings.md) establishes how the
external predictor delivers a 40-byte `MotionEventEntity` to Composer.
This trace identifies the fields consumed by the
[uniform-latency calculation](uniform-latency-findings.md). These are
live prediction inputs, not SDOCX serialization fields.

## The base predictor constructs the initial entity

Predictor `PredictorBase::Predict`, `0x2e7e4`, first admits real input
through `AddRealPenEvent` at `0x2eae4` and saves its unfiltered endpoint
at `0x2eaf4`. It then samples `GetNano` at `0x2eaf8`, preserving that
result in `x21`, and separately reads the incoming event's
`GetEventTimeNano` at `0x2eb04`, preserving it in `x22`.

Base `GetNano`, `0x9a26c`, calls `clock_gettime` with clock ID 1 at
`0x9a284`–`0x9a290` and returns `seconds * 1_000_000_000 + nanoseconds`
at `0x9a2a8`–`0x9a2b0`. The callback's clock sample is therefore an
actual native clock reading, not a value reconstructed from the
incoming event's millisecond timestamp.

On the branch that reaches virtual `DoPredict`, the base predictor
reads the final retained real record's nanosecond field into `x23` at
`0x2ebc8`. The record is 88 bytes and its nanosecond field is at
offset 24. The field identity follows from current-event getter/store
`0x2f1dc`/`0x2f254` and historical getter/store
`0x2f4b0`/`0x2f528`, followed by the full 88-byte record copy at
`0x304b8`.

This is the last retained record, which need not be the incoming
event's current point. Its separately aligned timestamp is at record
offset 80, written at `0x30534`; that field is not the value loaded
for the entity's reference.

At `0x2ec00`–`0x2ec0c`, the entity passed to virtual slot 256 is:

| Entity offset | Value on the `DoPredict` branch |
| --- | --- |
| 0 | Zero |
| 8 | Last retained real record's nanosecond field |
| 16 | Earlier `GetNano()` sample from `0x2eaf8` |
| 24 | Zero |
| 32 | Zero |

The entity is passed at `0x2ec10`. The linear and neural vtables
bind this slot to `LinearPredictor::DoPredict`, `0x242cc`, and
`NNPredictor::DoPredict`, `0x25abc` respectively.

Other base branches call `OnPredictionComplete` directly. For example,
`0x2ed00`–`0x2ed0c` supplies the incoming event's nanosecond timestamp
at entity offset 8, the same earlier clock sample at offset 16, and
zeros at 0/24/32. The paths at `0x2ed7c`–`0x2eda4` instead use the
retained-record reference. A complete callback schema must therefore
preserve the reference's branch-dependent origin.

## Neural prediction adds VSync and refresh-period fields

At the start of `NNPredictor::DoPredict`, the code gets refresh rate
through slot 208, bound to `PredictorBase::GetRefreshRate`, `0x30170`.
That getter reads float member 20, written by `SetRefreshRate` at
`0x3015c`. The [predictor factory](predictor-callback-findings.md)
supplies the configured rate through setter slot 144.

The neural path computes:

```text
frame_ns = trunc_to_i64(1_000_000_000.0 / f64(refresh_rate_f32))
```

The double constant is at Predictor `0x156c0`; conversion/division/
truncation occur at `0x25af4`, `0x25b0c` and `0x25b10`.
`0x25afc` reads neural member 1800, and `0x25b14` writes that value
and the derived period to entity offsets 24 and 32.

The resulting meanings are:

| Entity offset | Composer consumer name | Neural producer |
| --- | --- | --- |
| 8 | `reference_ns` | Retained real record's nanosecond field |
| 16 | `timing_ns` | Earlier base `Predict` clock sample |
| 24 | `alignment_ns` | Latest stored `OnVSync` argument |
| 32 | `frame_ns` | Double-derived period from predictor refresh rate |

`NNPredictor::OnVSync`, `0x259a0`, stores its argument in member 1800
at `0x259c4`. The adjusted-interface thunk, `0x259cc`, writes the
same member relative to the secondary interface at offset 1776.
Construction clears it at `0x24fa4`; the action-zero path in
`NNPredictor::Predict` clears it again at `0x25314`.

Consequently a neural `DoPredict` call can supply zero alignment and
a nonzero period before receiving a VSync update. Those two fields
must not be treated as a single present/absent flag.

The base branches that bypass `DoPredict` retain their zero fields.
The linear implementation forwards its entity to completion at
`0x2436c`–`0x2439c` or `0x248e0`–`0x248f8` without adding the
neural VSync/period values.

## The predictor and display helper use different period calculations

For ordinary positive rates, disposable reconstruction gives:

| Selected rate | Neural entity period, ns | Composer presentation-helper period, ns |
| --- | --- | --- |
| 60 | 16,666,666 | 16,666,667 |
| 90 | 11,111,111 | 11,111,111 |
| 120 | 8,333,333 | 8,333,333 |

The [presentation helper](presentation-time-findings.md) divides in
float and converts to signed 32-bit before widening. The neural entity
uses double division and converts directly to signed 64-bit. Its rate
also comes from predictor configuration, whereas presentation setup can
prefer the separately configured hardware refresh rate. Neither the
rate nor its resulting period should be forced to match across these
two paths.

## The neural task's later time remains separate

Further into `NNPredictor::DoPredict`, `0x25bc4` samples `GetNano`
again. Virtual slot 232 at `0x25bd8`–`0x25bdc` aligns that later time
through `NNPredictor::GetAlignedToVSyncTimeNano`, `0x256a0`.

For a positive period, the alignment is:

```text
if latest_vsync_ns == 0:
    aligned_task_ns = later_clock_ns
else:
    aligned_task_ns = latest_vsync_ns + floor((later_clock_ns - latest_vsync_ns) / frame_ns) * frame_ns
```

Signed division at `0x25708` truncates toward zero; the conditional
period subtraction at `0x2571c`–`0x25724` corrects the result when the
candidate exceeds the supplied time. The zero-VSync branch at
`0x257e8` returns the supplied time unchanged.

Task allocation copies the 40-byte entity into task offsets 48–87 and
stores this separate aligned time at offset 88. The synchronous stores
are at `0x25c4c`/`0x25c5c`; the worker-path stores are at
`0x25e14`/`0x25e24`. The exported `PredictionTask` constructor,
`0x2be08`, has the same layout at `0x2be38`/`0x2be40`.

`PredictionTask::Run`, `0x2c530`, forwards the entity from offsets
48–87 on its completion paths. The five call sites are `0x2c5b4`,
`0x2c8f0`, `0x2c9a8`, `0x2cb58` and `0x2d0b8`. Those copies do not
substitute task member 88 for entity member 16. Completion and
[consumer delivery](predictor-callback-findings.md) preserve that
distinction through to Composer.

## Reconstructed consumer examples

Let the retained real reference be 1,000,000,000 ns, the base clock
sample 1,005,000,000 ns, and the final predicted timestamp
1,032,000,000 ns. For neural prediction use rate 120 and, when present,
VSync origin 1,000,000,000 ns. Applying Composer's existing coefficient
equation gives:

| Supplied entity | Bottom delay, ns | Float coefficient |
| --- | --- | --- |
| Base completion, zero alignment and period | 0 | 0.15625 |
| Neural, VSync present | 0 | 0.5208333134651184 |
| Neural, VSync still zero | 0 | 0.6770833134651184 |
| Neural, VSync present | 4,000,000 | 0.6458333134651184 |

A later task clock of 1,010,000,000 ns aligns to 1,008,333,333 ns.
The callback still carries 1,005,000,000 ns at entity offset 16.
Replacing it with the task time would change the coefficient and would
not reproduce this data flow.

These examples reconstruct numerical inputs to the controller. They do
not assert that uniform latency is enabled for every producer path, or
that the supplied synthetic prediction timestamps are emitted by a device.

## Validation and remaining work

The APK digest and all three native byte streams were verified. Exported
function addresses, rate getter/setter bindings, double constants,
record timestamp stores, entity/task layouts and completion copies were
checked against ARM64 instructions. Disposable reconstruction checked
three rates, both sides of alignment boundaries, zero-VSync behavior,
the separate task time and four consumer coefficient examples.

The [VSync delivery trace](vsync-delivery-findings.md) identifies the
`OnVSync` argument as the unchanged Java frame-callback timestamp and
recovers subscription/removal. Active device configuration and neural
model behavior remain separate work. No SDK code changed and no native
device execution or new SDOCX fixture was used. The callback timing must
not be reapplied to stored timestamps during export.
