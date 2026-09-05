# Predictor input speed and low-speed admission

## Evidence and scope

Confirmed by static inspection of Samsung Notes 4.4.45.37 ARM64
`libSPenPredictor.so` and `libSPenComposer.so` from the
[identified APK](README.md#sources-and-validation).
This recovers the speed measurement and threshold used before
`NNPredictor::DoPredict` and its [acceleration/expiry gates](neural-admission-findings.md).

The speed gate uses a separate history from both the model input window
and the [50-record acceleration estimator](predictor-acceleration-findings.md).
Its stored measurements are DPI-scaled interval speeds, averaged without
time weighting.

## Each real-point append can add an interval speed

`PredictorBase::AddRealPoint`, `0x302b8`, calls `AddRealInputSpeed`
at `0x302dc` before appending the supplied record to the retained-point
deque. `AddRealInputSpeed`, `0x32294`, returns immediately if the
retained deque is empty at `0x322bc`–`0x322c0`.

Otherwise it compares the supplied record with the existing final retained
record. Their relative millisecond fields are loaded and subtracted at
`0x322d4` and `0x32310`–`0x32314`. A nonpositive interval skips
the new speed measurement through `0x32318`–`0x32400`.

For a positive interval, the measured speed is:

```text
scale_x = f32(25.4 / dpi_x) if dpi_x != 0 else stored_zero_dpi_scale
scale_y = f32(25.4 / dpi_y) if dpi_y != 0 else stored_zero_dpi_scale
dx_mm = f32(f32(current.x - previous.x) * scale_x)
dy_mm = f32(f32(current.y - previous.y) * scale_y)
distance_mm = sqrt_f32(fma_f32(dx_mm, dx_mm, f32(dy_mm * dy_mm)))
speed = f32(distance_mm / f32(current.relative_ms - previous.relative_ms))
```

Scale selection is at `0x32320`–`0x32380`; coordinate differences,
scaling, norm and division are at `0x324f8`–`0x3252c`. The divisions
that create the per-axis scale happen before multiplication by the
coordinate difference.

The float constant 25.4 is at Predictor `0x15650`. Each zero-DPI axis
independently uses `0x15664`, bits `0x3d820c4a`, approximately
0.0635000020 millimeters per coordinate unit. This is separate from the
neural task's [411-DPI fallback](neural-feature-findings.md#dpi-and-time-normalization).

With DPI `(254, 508)`, a 10-unit X movement or 20-unit Y movement over
4 ms both produce 0.25 mm/ms in the reference arithmetic.

## Measurement precedes filtering of the current batch

`AddRealPenEvent` invokes `UpdatePointerSpeed` at `0x2f768` before
returning to `Predict`. `Predict` then saves the unfiltered endpoint,
filters the newly admitted batch and calculates acceleration through
`0x2eaf4`, `0x2eb20` and `0x2eb38`.

The current point's speed measurement therefore precedes that batch's
Kalman filtering. Its preceding retained point can already have been
filtered during a previous batch. Consecutive appends within the same
batch precede the batch filter. Recomputing this queue from the final
filtered model window would not preserve that ordering.

## The speed queue retains endpoint times within a separate window

Each queued measurement occupies 16 bytes. `0x32518` stores the current
relative millisecond timestamp at record offset 8; `0x32530` stores
the float speed at offset 0. Predictor member 488 tracks the queue count.

After an append, `0x3254c`–`0x325bc` removes oldest entries while:

```text
last_measurement.relative_ms - first_measurement.relative_ms > window_ms
```

`window_ms` comes from predictor member 28 at `0x32574`. The base
constructor loads `(11, 83)` from `0x156d8` and stores them into
members 24/28 at `0x2de1c`; its initial speed-history window is thus
83 ms. The 11 is the initial retained-point limit, a separate field later
configured for the selected model.

Equality at 83 ms retains the oldest measurement. A larger difference
removes entries until the endpoint-time span fits. This bounds measurement
endpoints, not each speed's underlying interval start: a newly appended
speed derived from a long interval can remain as the only queue entry.

On a nonpositive incoming interval, `0x32400`–`0x32404` can still
enter the trimming loop for an existing queue. It compares the queue's
stored endpoints; it does not use that incoming timestamp to age entries.

`UpdatePointerSpeed`, `0x3111c`, walks these 16-byte entries and sums
their speed floats at `0x31160`–`0x31178`. For a positive count it
divides by that count and stores the result at predictor member 496
through `0x31180`–`0x31190`:

```text
pointer_speed = f32(f32_sum(queued_speeds) / f32(queue_count))
```

Each interval has equal weight regardless of duration. Speeds 0.1 and
0.3 therefore average to `f32(0.2)`, rather than a duration-weighted
speed. An empty queue leaves member 496 unchanged. The base constructor
initializes it to zero at `0x2deb8`, and the new-stroke setup also clears
it at `0x2f154`.

## The base gate compares against a supplied or fallback threshold

`IsLowInputSpeed`, `0x2e724`, reads member 496 and an optional threshold
object at member 1760. With no object, it uses Predictor `0x1560c`,
bits `0x3bab3aff`, approximately 0.00522553874 mm/ms.

With a threshold object, it calls that object's virtual slot 16 at
`0x2e748`–`0x2e74c` and uses the returned float. For finite values,
the comparison at `0x2e750` or `0x2e7b0` is:

```text
low_input_speed = pointer_speed < threshold
```

Equality is accepted. `Predict` calls the gate at `0x2eb40`; a true
result branches at `0x2eb78` to the separate completion/control path at
`0x2ec40`, bypassing virtual `DoPredict` for that invocation. Other
input-readiness checks can still prevent prediction when speed passes.
Unbuffered dispatch can affect callback handling later in that branch;
it does not turn this low-speed branch into a model invocation.

`SetLowInputSpeedThresold`, `0x328f8`, stores the supplied object pointer
at member 1760. The spelling is the exported native symbol's spelling.

## Composer derives a threshold from the configured prediction time

The Composer presenter allocates a 16-byte threshold at
`0x4d70f0`–`0x4d7110`, stores it in presenter member 352 and initializes
its mode byte at offset 12 to 1. GOT `0x5a2ba0` resolves to vtable
`0x580c28`, whose RTTI identifies `SPen::LowInputSpeedThreshold`.
The primary address point is `0x580c38`.

During input configuration, `0x4d82a8`–`0x4d82b8` calls threshold
slot 24 with the predictor proxy. This resolves to `0x4d1d58`, which
reads the proxy's `GetPredictionTime` through slot 112. Its update is:

```text
base_threshold = f32(stored_distance / prediction_time) if prediction_time != 0
                 else stored_fallback
```

Composer `0x1f91d8` supplies `stored_distance`, bits `0x3db2b8c7`,
approximately 0.0872664973. The zero-time fallback at `0x1f90e4`
has the same bits as Predictor's fallback threshold. The result is stored
at threshold-object offset 8 by `0x4d1db0`.

Threshold slot 16, `0x4d1e04`, returns:

```text
threshold = base_threshold if mode_byte == 0
            else fma_f32(base_threshold, 5, base_threshold)
```

The mode-enabled result is six times the base threshold, using fused
arithmetic at `0x4d1e14`. Function `0x4d1e20` can overwrite the mode
byte; this trace establishes its constructor value and getter behavior,
not every possible later caller.

The presenter installs this object on its proxy at `0x4d82bc`–`0x4d82cc`.
Proxy slot 104 resolves to `0x4daf58`, which forwards to concrete
predictor slot 104 at `0x4daf64`–`0x4daf68`; the neural vtable binds
that slot to `0x328f8`.

For the [Composer-configured millisecond limits](neural-selection-findings.md#composer-enables-multiple-outputs-after-predictor-selection):

| Prediction time | Base threshold, mm/ms | Mode-enabled threshold, mm/ms |
| --- | --- | --- |
| Zero-time fallback | 0.005225539 | 0.031353232 |
| M16, 16 ms | 0.005454156 | 0.032724936 |
| M20, `f32(19.4)` ms | 0.004498273 | 0.026989639 |
| M22, `f32(22.9)` ms | 0.003810764 | 0.022864584 |

This update consumes the getter unchanged. Direct factory users retaining
the raw microsecond default would produce a different threshold scale if
they installed this threshold without Composer's millisecond setter sequence.

## Validation and remaining work

Both native libraries were matched to the identified APK. Measurement
ordering, scale constants, 16-byte queue records, strict endpoint trimming,
mean updates, threshold RTTI/vtables and cross-library installation were
checked against ARM64 instructions and relocations.

Disposable arithmetic checks covered independent zero-DPI axes, unequal
axis scales, nonpositive intervals, the 83/84 ms queue boundary, empty-queue
retention, unweighted averaging, long-interval retention and all four
threshold rows with adjacent float comparisons. These checks do not execute
the native input pipeline.

Remaining admission work includes the separate readiness/chronology gate
and its callback scheduling, plus a complete audit of threshold-mode changes.
Actual device sampling and prediction outcomes remain unmeasured.
No SDK code or corpus fixture changed.
