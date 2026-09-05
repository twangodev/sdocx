# Neural admission, expiry and discarded horizons

## Evidence and scope

Confirmed by static inspection of Samsung Notes 4.4.45.37 ARM64
`libSPenPredictor.so` from the [identified APK](README.md#sources-and-validation).
This extends [horizon selection](neural-selection-findings.md) with the
acceleration gate, the task's discarded-output count and two expiry checks.

These rules control transient prediction. They do not establish additional
SDOCX fields or saved-stroke behavior.

## Acceleration controls task admission and the processed prefix

`NNPredictor::DoPredict`, `0x25abc`, reads two float members at
`0x25b40`–`0x25b44`:

| Symbol used below | Object offset | Exported getter |
| --- | --- | --- |
| `A` | 1752 | `GetAverageAcceleration`, `0x2e7d4` |
| `G` | 1756 | `GetAverageAngleAcceleration`, `0x2e7dc` |

`PredictorBase::calculateAcceleration`, `0x2e200`, clears both fields
at `0x2e23c` and writes its calculated results at `0x2e568`–`0x2e56c`.
The gate below uses the stored values directly. The estimator's sampling,
weighting and physical units require a separate trace.

Let `P = f32(A * G)` and `N = model.OutputSize`. For finite values,
the single-output path at `0x25b48`–`0x25b78` rejects when either:

```text
A > f32(0.1)
P > f32(0.001)
```

Otherwise it sets the discarded count to zero. This is the bundled M16 path.

For multiple outputs, `0x25b7c`–`0x25bb0` first rejects when
`P > f32(0.02)`. Otherwise it calculates:

```text
discarded = (N - 1 if P > f32(0.001) else 0)
discarded += (1 if A > f32(0.5) else 0)
reject if discarded >= N
```

Equality at each threshold takes the lower-discard branch. The constants
are the stored binary32 values, not exact decimal fractions:

| Constant | Native address | Binary32 bits |
| --- | --- | --- |
| `0.1` | `0x15638` | `0x3dcccccd` |
| `0.001` | `0x15660` | `0x3a83126f` |
| `0.02` | `0x1563c` | `0x3ca3d70a` |
| `0.5` | Instruction immediate at `0x25b9c` | `0x3f000000` |

For bundled three-output models M20 and M22:

| Finite inputs | Result |
| --- | --- |
| `A <= 0.5`, `P <= f32(0.001)` | Process indices 0, 1 and 2 |
| `A > 0.5`, `P <= f32(0.001)` | Process indices 0 and 1 |
| `A <= 0.5`, `f32(0.001) < P <= f32(0.02)` | Process index 0 |
| `A > 0.5`, `P > f32(0.001)` | Reject task |
| `P > f32(0.02)` | Reject task |

The exported `accelerationCheck`, `0x25ec0`, implements the same finite
admission decision with a referenced discard counter. The actual
`DoPredict` path has this logic inlined and initializes the count itself.
Its rejected branch at `0x25cf8` reaches `OnPredictionComplete` through
`0x25d64`–`0x25d68` without constructing a task.

## Member 112 is the discarded-output count

The direct-execution constructor stores `discarded` at task offset 112
with `0x25c80`; the worker-queue constructor does the same at `0x25e4c`.
Both stores pair it with the initial normalizer at offset 108.
`PredictionTask::SetDiscardPointCnt`, `0x2dc04`, writes the same field,
but these two creation paths do not call that exported setter.

`Run` processes the prefix `[0, N - discarded)` through
`0x2cce0`–`0x2ccec` and `0x2d020`–`0x2d038`. Discarding therefore
removes the latest configured horizons. Other gates can still reject
individual members of that prefix.

The [selector](neural-selection-findings.md#selection-compares-truncated-whole-milliseconds)
uses all `N` horizons before this bound is applied. There is no adjustment
of the selected range to the reduced prefix between `0x2cc24` and
`0x2cce0`.

For example, the finite gate inputs `A = 0.75`, `G = 0` admit indices
`[0, 1]` for M20. Single-output selection of its 19.4 ms horizon marks
only index 2, leaving the processed prefix entirely unmarked if either
candidate survives. This is a concrete static configuration that exposes
the previously identified callback invariant. It is not evidence that
the application supplies this combination or that a device crashes.
The estimator, configuration callers and candidate gates still need a
combined reachability check.

## Two expiry checks use a shared aligned origin

`DoPredict` obtains a clock reading at `0x25bc4`, aligns it through
virtual slot 232 at `0x25bd8`–`0x25bdc`, and passes the result to
`NNPredictor::IsExpired` at `0x25bec`. Both task constructors preserve
that same aligned origin at task offset 88 through `0x25c5c` and
`0x25e24`. It is separate from the
[callback timing entity](predictor-timing-findings.md).

For a positive finite refresh rate stored as binary32 in predictor member 20:

```text
frame_us = trunc_i64(f32(f32(1000 / refresh_rate) * 1000))
admission_us = trunc_i64(f32(f32(frame_us) * f32(0.85)))
age_us = trunc_i64(f64(now_ns - aligned_origin_ns) * f64(0.001))
expired = age_us < 0 or age_us > budget_us
```

`NNPredictor::IsExpired`, `0x25f64`, uses `admission_us` as the
budget. Instructions `0x25f80`–`0x25f9c` include the intermediate
integer truncation before multiplication by the binary32 0.85 constant
at `0x15618`. Age conversion and comparison are at
`0x25fa4`–`0x25fc4`; the multiplier at `0x15720` is binary64 0.001.

`PredictionTask::IsExpired`, `0x2d2c4`, uses `frame_us` without the
85% step. It reads task offset 88 at `0x2d2f4` and compares the converted
age through `0x2d330`–`0x2d350`. `Run` calls it at `0x2ca64`,
after a successful interpreter invocation.

| Refresh rate | Admission budget, microseconds | Post-inference budget, microseconds |
| --- | --- | --- |
| 60 Hz | 14,166 | 16,666 |
| 90 Hz | 9,444 | 11,111 |
| 120 Hz | 7,083 | 8,333 |

These float-derived microsecond budgets are distinct from the callback's
double-derived integer-nanosecond frame period. Each expiry check obtains
a fresh clock reading; the task check includes time spent waiting and
performing inference since the captured aligned origin.

Equality is accepted. At 60 Hz, an admission age of 14,166,999 ns still
truncates to 14,166 microseconds, while 14,167,000 ns expires. A negative
age of -999 ns also truncates to zero and passes; -1,000 ns expires.

## Unbuffered dispatch bypasses both time rejections

The admission branch checks virtual slot 160 at
`0x25bf4`–`0x25c04`; the post-inference branch checks it at
`0x2ca6c`–`0x2ca7c`. In the neural vtable this resolves to
`PredictorBase::GetUnbufferedDispatch`, `0x301c0`, reading byte 1768.
`SetUnbufferedDispatch` writes it at `0x301b8`.

A true value allows an expired prediction to continue at either stage.
A false value routes an expired result to completion without the output
append loop. This does not imply absence of a callback: the
[completion path](predictor-callback-findings.md) can deliver a null event.
Unbuffered dispatch does not bypass the preceding acceleration gate.

## Validation and remaining work

The library bytes were matched to the identified APK. Getter mappings,
threshold loads, inline constructor stores, output bounds and both expiry
comparisons were checked against ARM64 instructions and relocations.

Disposable calculations checked 578 finite acceleration combinations
against the separately exported helper, threshold neighbors, the
three refresh-rate budgets and both sides of their expiry boundaries.
They also checked the disjoint selected-range/processed-prefix example.
These checks reproduce recovered arithmetic; they do not execute the
native predictor or its model weights.

The [motion trace](neural-motion-findings.md) recovers post-inference
displacement, deviation and candidate-distance gates. Remaining work includes
the acceleration estimator and application-level reachability of the
unmarked-vector case. No SDK code or corpus fixture changed.
