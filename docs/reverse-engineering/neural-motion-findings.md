# Neural output motion and deviation gates

## Evidence and scope

Confirmed by static inspection of Samsung Notes 4.4.45.37 ARM64
`libSPenPredictor.so` from the [identified APK](README.md#sources-and-validation).
This continues the [admission and expiry trace](neural-admission-findings.md)
through whole-task and per-candidate rejection after inference.

Three distinct checks appear here: a DPI-scaled minimum displacement for
the 480 Hz model, speed statistics for the other bundled models, and a
distance cap applied to each candidate that reaches the output loop.

## The 480 Hz model requires recent movement

After the post-inference expiry check, `Run` reads model member 128 and
compares it with 480 at `0x2ca84`–`0x2ca8c`. This is the configured
model sample rate, not the display refresh rate. Among the
[bundled models](neural-model-findings.md), only M22 takes this branch.

For a copied real-point vector with `n` entries, index construction at
`0x2ca90`–`0x2cacc` selects points `n - 1` and `n - 5`. These endpoints
span four intervals. It calculates:

```text
dx_mm = f32(f32(f32(last.x - earlier.x) / dpi_x) * f32(25.4))
dy_mm = f32(f32(f32(last.y - earlier.y) / dpi_y) * f32(25.4))
distance_mm = sqrt_f32(fma_f32(dx_mm, dx_mm, f32(dy_mm * dy_mm)))
```

The per-axis divisions are at `0x2cae4` and `0x2caf4`; scaling and
the fused squared norm follow through `0x2cb14`. Task offsets 100/104
supply the [configured or fallback DPI](neural-feature-findings.md#dpi-and-time-normalization).

For finite distances, `0x2cb18`–`0x2cb1c` accepts
`distance_mm >= f32(0.09)`. The threshold at `0x1562c` has bits
`0x3db851ec`, approximately 0.0900000036 mm. A smaller displacement
routes to `OnPredictionComplete` at `0x2cb58` without appending outputs.

This gate uses net displacement, so a path that moves away and returns
near its earlier endpoint can fail despite having a longer travelled
distance. Equality passes. The accepted 480 Hz branch skips the
deviation function below.

## Other models compare real and predicted speed statistics

The non-480 branch calls `PredictionTask::checkPredictionDeviation`,
`0x2d368`, at `0x2cb70`. A true result rejects the whole prediction;
a false result continues to horizon selection and the append loop.

The function reads the copied real-point vector and reconstructs every
configured output using the same DPI conversion, inverse rotation and
last-real anchor as the [output conversion trace](neural-output-findings.md).
It runs before output selection and discarding: its loop compares against
the full `OutputSize` at `0x2d8b4`–`0x2d8bc`, without reading the
discard count at task offset 112.

### Real speeds use up to five adjacent intervals

For the normal bundled windows, `0x2d468`–`0x2d510` visits these
five adjacent pairs, newest first:

```text
(n - 2, n - 1), (n - 3, n - 2), ..., (n - 6, n - 5)
```

For fewer points it stops after `n - 1` intervals. Each speed is:

```text
elapsed_us = i64(current.relative_ms - previous.relative_ms) * 1000
speed = f32(distance_xy(current, previous) / f32(elapsed_us))
```

The integer multiplication precedes conversion to float at
`0x2d4a8`–`0x2d4bc`. Distance uses float coordinate differences,
a float Y square, fused X-square addition and float square root at
`0x2d498`–`0x2d4b8`. The result is in coordinate units per microsecond.
There is no DPI conversion in this real-speed calculation.

### Predicted speeds use successive reconstructed outputs

The initial position comes from a separate copy of the unfiltered last
real record at predictor offset 136. `Run` copies it at
`0x2c6b4`–`0x2c6c0`; the deviation function loads its XY at
`0x2d550`. The copied real vector's final record remains the anchor
passed to `GetPenEvent` through `0x2d580` and `0x2d654`–`0x2d658`.
These two input records must not be assumed to have identical XY after
filtering.

For reconstructed output coordinates `Q[i]` and configured horizons
`H[i]` in microseconds:

```text
previous_xy = unfiltered_last_xy
previous_horizon = 0
for each configured output i:
    distance = distance_xy(Q[i], previous_xy)
    elapsed_us = f32(H[i] - previous_horizon)
    predicted_speed[i] = f32(distance / elapsed_us)
    previous_xy = Q[i]
    previous_horizon = H[i]
```

Coordinate differences and distance are calculated through
`0x2d670`–`0x2d694`. Horizon subtraction is at `0x2d738`, state
updates occur at `0x2d74c`–`0x2d754` or `0x2d7bc`–`0x2d7e8`,
and division is at `0x2d800`.

Thus M20 uses durations 5,600, 5,500 and 8,300 microseconds for these
statistics. Each model output is still positioned independently relative
to the final real-point anchor; only the speed calculation uses consecutive
output coordinates. This stage does not use the truncated millisecond
increments later written into candidate events.

### Single and multiple outputs have different rejection rules

For each speed collection, the function accumulates a float sum and a
fused sum of squares. Real accumulation is at `0x2d4f0`–`0x2d4f8`;
predicted accumulation is at `0x2d870`–`0x2d890`.

For count `k`, sum `S` and squared sum `SS`, the finite arithmetic is:

```text
mean = f32(S / k)
variance = fma_f32(-mean, mean, f32(SS / k))
sample_variance = f32(variance * f32(k / f32(k - 1)))
sample_std = sqrt_f32(sample_variance)
```

`0x2d90c`–`0x2d92c` computes the real statistics. With multiple
configured outputs, `0x2d934`–`0x2d958` computes predicted statistics
and compares:

```text
reject if predicted_sample_std > f32(3 * real_sample_std)
```

With one output, `0x2d960`–`0x2d968` instead compares:

```text
reject if predicted_mean_speed > f32(4 * real_mean_speed)
```

The first rule applies to bundled M20; the second applies to M16.
M22 bypasses this function through its sample-rate branch. Both comparisons
are strict, so equality passes.

For a simple arithmetic example, real speeds `[1, 2, 3, 4, 5]` produce
a sample standard deviation of approximately 1.58114. Predicted speeds
`[1, 1, 9]` produce 4.61880 and pass the three-times rule;
`[1, 1, 10]` produce 5.19615 and fail. Constant predicted speeds
`[100, 100, 100]` pass this variation check even though their mean is
larger; the separate candidate-distance check remains applicable.

These comparisons do not constitute a general finite-value validator.
The real-speed loop has no separate zero-duration rejection, and the
variance path has no clamp before square root. An unordered comparison
does not set the final `GT` result at `0x2d970`. Failure of `GetPenEvent`
also returns false through `0x2d8a8`–`0x2d98c`, meaning this function
does not reject on that failure. The later output loop still has its own
conversion and append conditions.

## Each candidate has a separate distance cap

After output XY and timestamps have been reconstructed, the main loop
checks the real-point count at `0x2ce68`–`0x2ce8c`. With fewer than
five records, it skips this cap. Otherwise it selects `last = n - 1`
and `earlier = n - 5` through `0x2ce90`–`0x2ceb4`.

It subtracts their relative millisecond timestamps at
`0x2ceb8`–`0x2cec0`. If the difference is less than 1, the branch at
`0x2cec8` skips this cap instead of rejecting the candidate.

For a positive interval:

```text
real_distance = distance_xy(last, earlier)
real_speed = f32(real_distance / f32(elapsed_ms))
time_value = GetPredictionTime()
if time_value == 0:
    time_value = f32(16.7)
distance_limit = f32(f32(real_speed * time_value) * 10)
reject_candidate = distance_xy(candidate, last) > distance_limit
```

The getter is virtual slot 112, resolving to `NNPredictor::GetPredictionTime`,
`0x25690`. `0x2cf10`–`0x2cf2c` calls it again on the nonzero path.
The fallback at `0x15634` has bits `0x4185999a`, approximately
16.7000008. Multiplications and the strict comparison are at
`0x2cf54`–`0x2cf6c`.

The [maximum-time trace](neural-selection-findings.md#the-explicit-maximum-time-setter-uses-milliseconds)
shows that the explicit setter stores milliseconds, while constructor and
model-length defaults store the raw last microsecond horizon in the same
getter-backed field. This cap uses the value unchanged and performs no
unit correction. Its effective scale therefore depends on configuration.
The [Composer selection sequence](neural-selection-findings.md#composer-enables-multiple-outputs-after-predictor-selection)
applies the millisecond setter immediately after creating a predictor,
resolving the raw-default discrepancy for that path.

For a real displacement of 4 coordinate units over 4 ms and an explicit
time value of 20, the cap is 200 coordinate units from the last point.
A candidate exactly 200 units away passes; a larger distance fails.
The cap uses neither candidate horizon differences nor travelled path length.

Each distance normally uses float differences and the fused squared norm.
If the float squared norm makes the zero-product self-comparison unordered,
`0x2d068`–`0x2d09c` recomputes squares and the norm in double, then
converts the resulting distance back to float. Coordinate subtraction
has already happened in float before this fallback.

An over-limit candidate branches to `0x2d040` and advances the loop
through `0x2d010`, without reaching `AddPredictedPoint` at `0x2cf98`.
Later candidates can still be considered. This differs from the two
whole-task checks above and can remove a candidate marked by horizon
selection.

## Validation and remaining work

The native byte stream was matched to the identified APK. Model-rate
dispatch, endpoint indices, time units, statistics, comparison branches,
getter mapping and the candidate skip path were checked against ARM64
instructions and relocations.

Disposable numerical checks used binary32 operations and fused arithmetic
to verify anisotropic-DPI movement, the five-interval window, successive
horizon differences, both deviation rules, equality boundaries, zero-time
cap bypass and the wider squared-norm fallback. These are independent
arithmetic checks, not execution of the bundled model or device validation.

The [acceleration estimator](predictor-acceleration-findings.md) and
[low-speed admission](predictor-speed-findings.md) are now traced.
Readiness/chronology checks and application configuration/runtime evidence
remain relevant to the [unmarked-vector reachability question](neural-admission-findings.md#member-112-is-the-discarded-output-count).
No SDK code or corpus fixture changed.
