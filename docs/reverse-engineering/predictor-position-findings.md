# Screen-position coefficients for prediction pacing

## Evidence and scope

Confirmed by static inspection of Samsung Notes 4.4.45.37 ARM64
`libSPenComposer.so` and `libSPenPredictor.so` from the
[identified APK](README.md#sources-and-validation).

Composer supplies the neural [VSync pacing coefficient](predictor-chrono-findings.md)
from a point's presentation delay. It subtracts 3 ms, clamps the result
at zero, and divides by the presenter's float refresh period. Its sample
selection and setter ordering are distinct from the uniform-latency
controller's saved anchor and prediction-cutoff operation.

## The presenter obtains a position delay before dispatching prediction

`TouchPresenter::PresentTouch`, Composer `0x4d76bc`, reaches the update
block at `0x4d848c` for the eligible-prediction branch. The alternate
unbuffered drawing checks at `0x4d8468`–`0x4d8488` can also reach it.
The earlier missing-drawable and payload checks at `0x4d842c`–`0x4d8438`
can bypass the block. This is not an unconditional update for every event.

The update calls the member-528 controller's slot 64 at `0x4d849c`,
then slot 88 at `0x4d84b0`, both with the original payload's motion event.
The controller is `PredStrokeLengthController`, whose primary vtable is
`0x580f20`. Slot 64 is `SetLastEvent`, `0x4d6928`; slot 88 resolves to
`0x4d6c5c` through relocation `0x580f78`.

Slot 88 returns the position delay used for pacing. It constructs its
own sample record from the supplied event. It does not read the anchor
just saved by `SetLastEvent`.

## Position delay uses the last history entry whenever one exists

At `0x4d6ca8`–`0x4d6cb0`, the delay method calls `GetHistorySize` and
chooses history index `size - 1` when nonzero. If there is no history,
it takes the current sample at `0x4d6d3c`.

Both paths copy pointer-0 coordinates into a temporary 72-byte
`PointerCoords` record. The historical X/Y getters are called at
`0x4d6cf0`/`0x4d6d04`, with stores at `0x4d6d00`/`0x4d6d10`.
The current X/Y calls are at `0x4d6d6c`/`0x4d6d7c`, with stores at
`0x4d6d78`/`0x4d6d88`. Millisecond time is reconstructed using down time,
and nanoseconds, pressure, tilt and orientation are also copied.

This method does not test current or historical resampled flags, and it
does not check controller enable byte 104. For example, with historical
states `[0, 1]` and current state 0, it still chooses the last historical
sample, whose state is 1. In contrast, the
[saved-anchor method](stroke-prediction-findings.md#prediction-length-control-selects-a-non-resampled-anchor)
uses the current sample for state 0 and has additional historical-state
rules. The two inputs must not be merged into one selection policy.

The delay method obtains `MotionEvent::GetTransforms` at `0x4d6db8`,
computes `Matrix3<float>::inverse` at `0x4d6dc0` and applies it to the
temporary record at `0x4d6dcc`. Composer relocations identify those calls:

| Operation | PLT | Relocation |
| --- | --- | --- |
| `GetTransforms` | `0x55a0a0` | `0x5a9448` |
| `Matrix3<float>::inverse` | `0x54d730` | `0x5a2f90` |
| `PointerCoords::Transform` | `0x55e110` | `0x5ab480` |

The transformed double X/Y pair is narrowed to floats and duplicated into
the point rectangle `[x, y, x, y]` at `0x4d6dd0`–`0x4d6df0`.
It calls `PresentTimeFinder::CalcPresentTime`, `0x4d4be4`, at `0x4d6df4`.

Before that call, `0x4d6de4` zeros the top and bottom output slots.
The argument registers supply the top pointer at stack offset 8 and bottom
pointer at offset 0. The return load at `0x4d6df8` therefore selects the
bottom delay. A helper guard that leaves outputs unchanged returns zero
here. The [presentation-time trace](presentation-time-findings.md) recovers
its visible rectangles, display dimensions, rotation and unclamped position
arithmetic.

## Composer converts nanoseconds to a dimensionless coefficient

The signed integer delay returned by slot 88 is converted directly to
float at `0x4d84b8`. For ordinary finite inputs and a positive presenter
refresh rate, the subsequent arithmetic is:

```text
period_ns_f32 = f32(f32(1_000_000_000) / presenter_refresh_rate_f32)
adjusted_delay_ns_f32 = f32(f32(bottom_delay_ns) + f32(-3_000_000))
nonnegative_delay_ns_f32 = max(adjusted_delay_ns_f32, f32(0))
coefficient_f32 = f32(nonnegative_delay_ns_f32 / period_ns_f32)
```

The numerator is rounded to float before the subtraction. Constants
`0x1f91a8` and `0x1f90d4` contain 1,000,000,000 and -3,000,000
respectively. Period division occurs at `0x4d84d0`, addition at
`0x4d84d8`, the lower clamp at `0x4d84e4`–`0x4d84e8`, and final division
at `0x4d84ec`.

There is no upper clamp. A beyond-screen point can produce a coefficient
greater than one. A helper guard, negative delay or delay at or below 3 ms
produces zero under these finite-input assumptions. This formula does not
establish a general nonfinite-input policy.

The denominator reads presenter float member 396 at `0x4d84bc`.
Construction writes the float bits for 60 there through `0x4d6fc0` and
`0x4d6fe4`. The rate-update method at `0x4d470c` stores its supplied float
into member 396 at `0x4d4728` and forwards the same value through the
predictor proxy's refresh setter at `0x4d4734`.

The position-delay helper can instead use its configured hardware refresh
rate, falling back to the supplied presenter rate only when that hardware
value is zero. Consequently the period used to calculate `bottom_delay_ns`
need not equal the denominator period above. Even equal rate inputs use
different conversions: the helper stores a truncated signed integer period,
whereas the coefficient denominator remains float.

## The coefficient reaches two separate chronometers

The first recipient is the predictor proxy at presenter offset 64.
`0x4d84f0` calls proxy slot 168 with the calculated float. Primary vtable
`0x5810b0` binds that slot to `0x4dafec`, which forwards to the concrete
predictor's slot 168 when its member-8 pointer is non-null. With no concrete
predictor, it returns without saving the value for a future predictor.

Both concrete predictor vtables bind slot 168 to
`PredictorBase::SetPenLocationInScreenCoef`, `0x301c8`. That method forwards
through `TaskChrono` slot 56. As the
[pacing trace](predictor-chrono-findings.md#linear-prediction-and-coefficient-forwarding-differ)
shows, this updates only the currently active backend. The time backend's
setter is a no-op; the VSync backend stores its coefficient at offset 28.

The second recipient is the separate `UBDDrawChrono` at presenter offset
520. Composer reconstructs the denominator from the presenter rate again
at `0x4d84f4`–`0x4d8508` and calls this object's slot 56 at `0x4d850c`.
The shared numerator is retained in `s9`; the denominator is reread and
recomputed rather than retaining the first division's result.

Its class identity follows from constructor `0x4db3a8`, GOT `0x5a2bf0`,
primary vtable `0x5812c0` and RTTI `0x59bdc8`, named `SPen::UBDDrawChrono`.
Slot 56 resolves to `0x4db774`, which forwards to its active backend at
offset 32. Its time setter, `0x4db26c`, is a no-op; its VSync setter,
`0x4db3a0`, stores offset 28. This is independent state from the neural
predictor's chronometer.

## Setter ordering explains the first update after a mode change

In the eligible-prediction branch, both coefficient setters run before
the proxy prediction call at `0x4d8540`. That call reaches neural
`Predict`, then the base dispatch that invokes
`NNPredictor::CheckExpiredAndReset`. The mode switch occurs inside that
later check at Predictor `0x25a34`.

For a fresh chronometer, the active backend is time and the VSync
coefficient is zero. In the sequential path with installed VSync registration
and unbuffered dispatch:

1. Composer computes a coefficient and calls the time backend's no-op setter.
2. The later neural pacing check switches to the VSync backend.
3. That check sees VSync's existing coefficient, initially zero.
4. A later Composer update while VSync remains active stores the new coefficient.

For an existing chronometer switched back from time to VSync, the same
ordering can expose an older VSync coefficient rather than zero. Switching
backends preserves their state and does not replay the setter. This follows
from the traced sequential calls; it does not measure device incidence or
establish behavior under concurrent reconfiguration.

## Numerical reconstruction and validation

For identical view/screen origins, real height 2000, rotation 0, matching
helper/presenter rate 120, and an already active VSync backend:

| Selected Y | Bottom delay, ns | Coefficient | Neural phase budget, us |
| --- | --- | --- | --- |
| 0 | 0 | 0 | 0 |
| 500 | 2,083,333 | 0 | 0 |
| 1000 | 4,166,666 | 0.1399999112 | 1,166 |
| 2000 | 8,333,333 | 0.6399999261 | 5,333 |
| 3000 | 12,500,000 | 1.1399999857 | 9,499 |

The last column applies the separately recovered native float-to-integer
[phase-budget calculation](predictor-chrono-findings.md). It is not the
complete due decision: the outer one/two-frame interval conditions still
apply. In particular, a coefficient above one does not remove the two-frame
forced-due condition.

With helper hardware rate 60 but presenter rate 120, Y = 1000 instead
produces bottom delay 8,333,333 ns and coefficient 0.6399999261.
Assuming one shared refresh period would miss that difference.

Both native byte streams were matched to the APK. Controller/proxy/backend
vtables, RTTI, getter and transform relocations, copied fields, constants,
float arithmetic and call ordering were checked against ARM64 instructions.
Disposable reconstruction covered three rates, the 3 ms boundary, last-history
selection, zero output from helper guards, values above one, differing helper
and presenter rates, and coefficient retention across backend changes.

These checks do not execute native drawing or prediction. The
[drawing-cadence trace](unbuffered-draw-findings.md) recovers the separate
chronometer's checks and reset sites while preserving the unresolved initial
registration edge. Runtime configuration and worker scheduling remain
additional work. No SDK code or corpus fixture changed.
