# Prediction acceleration estimation

## Evidence and scope

Confirmed by static inspection of Samsung Notes 4.4.45.37 ARM64
`libSPenPredictor.so` from the [identified APK](README.md#sources-and-validation).
This recovers `PredictorBase::calculateAcceleration`, `0x2e200`, and
`sumAngleAccelerator`, `0x2e61c`, which supply the two fields used by
the [neural admission gate](neural-admission-findings.md).

The estimator uses a separate 50-record history, five-sample endpoint
steps, a time cutoff, cached contributions and asymmetric weighting.
Its rules are more specific than averaging conventional acceleration
over the model's input window.

## Estimation follows filtering of newly admitted samples

`PredictorBase::Predict` calls `AddRealPenEvent` at `0x2eae4` and
extracts its upper 32-bit return field at `0x2eaec`. That field counts
newly appended records, bounded by the retained-point limit. The append
path calculates its count at `0x2f604` or `0x2f700`–`0x2f704`,
applies the bound through `0x2f770`–`0x2f7a0`, and packs the return
at `0x2f318`.

The predictor first saves the unfiltered endpoint at `0x2eaf4`. It then
passes the count and event action to `ApplyKalmanFilter` at `0x2eb20`
and to `calculateAcceleration` at `0x2eb38`, in that order. The filter
receives references to retained XY at `0x2f960`–`0x2f964` and can
modify them through `0x2f98c` before estimation.

Consequently the estimator consumes the post-filter retained coordinates,
while the separate unfiltered endpoint remains available to the
[output deviation check](neural-motion-findings.md#predicted-speeds-use-successive-reconstructed-outputs).

## A separate array retains positions, times and cached contributions

The array begins at predictor offset 552 and contains 50 records of 24
bytes. For index `j`, its address is `predictor + 552 + 24*j`:

| Record offset | Value |
| --- | --- |
| 0 | X, float |
| 4 | Y, float |
| 8 | Relative event time, signed 64-bit milliseconds |
| 16 | Cached acceleration contribution, float |
| 20 | Cached angular-acceleration contribution, float |

Every call first clears the published values at predictor offsets 1752
and 1756 with `0x2e23c`. Action 1 returns immediately. Action 0 clears
all array coordinates and contributions and sets every array timestamp
to -1 at `0x2e254`–`0x2e26c`; it returns without inserting that down
event into this array. Other actions with zero new records also return.

For a positive new-record count, let `m = min(count, 50)`. When `m < 50`,
`0x2e2a8`–`0x2e2c8` shifts array entries `[m, 50)` left, copying
all 24 bytes, including their cached contributions. The subsequent loop
copies the final `m` retained points into array slots `[50 - m, 50)`.

The new tail stores at `0x2e33c`–`0x2e348` overwrite only XY and
relative milliseconds, sourced from retained-record offsets 32/36 and 16.
They do not clear the destination's cached contribution fields. The
reuse guard below determines whether a contribution may be reused.

## The walk spans five-sample intervals and can exceed 50 ms

The loop starts at array endpoint 49 and advances backward by five at
`0x2e478`–`0x2e484`. Its possible endpoint pairs are:

```text
(44, 49), (39, 44), (34, 39), (29, 34), (24, 29),
(19, 24), (14, 19), (9, 14), (4, 9)
```

There are at most nine steps, spanning the latest 46 array positions.
For each step, `dt = time[j] - time[j - 5]` is measured in milliseconds.
The loop checks conditions in this order:

1. If endpoint `time[j] == -1`, return immediately with both published
   results still zero, even if earlier steps accumulated contributions.
2. If previously accumulated elapsed time is greater than 50 ms, stop
   walking and proceed to final averaging.
3. Otherwise process this step, then add its `dt` to accumulated time.

The sentinel check is at `0x2e390`–`0x2e398`, the strict time comparison
at `0x2e39c`–`0x2e3a0`, and elapsed-time addition at `0x2e474`.
Equality at 50 ms permits another step. There is no independent sentinel
check for `time[j - 5]`, nor a positive-duration check before division.

For a fully populated array with regular timestamp spacing:

| Time between adjacent records | Processed steps | Total interval span |
| --- | --- | --- |
| 1 ms | 9 | 45 ms |
| 2 ms | 6 | 60 ms |
| 3 ms | 4 | 60 ms |
| 11 ms | 1 | 55 ms; insufficient steps for publication |

This is not a clipped 50 ms moving average. With only the final 20 array
slots populated at one-millisecond spacing, the walk reaches an invalid
endpoint and returns zero instead of publishing a shorter-window estimate.

## Fresh speed differences have asymmetric weighting

For a recomputed step, `0x2e3f8`–`0x2e414` calculates float XY
differences, a fused squared norm, a float square root and division by
`f32(dt)`:

```text
speed = f32(distance_xy(point[j], point[j - 5]) / f32(dt))
```

The newest step initializes the previous speed. For subsequent recomputed
steps, `0x2e41c`–`0x2e438` calculates and caches:

```text
change = abs(f32(speed - previous_speed))
weighted_change = f32(2 * change) if speed > previous_speed else change
contribution = f32(weighted_change / f32(dt))
acceleration_sum = f32(acceleration_sum + contribution)
previous_speed = speed
```

The walk runs from newer segments toward older ones. With equal segment
durations, the doubled branch therefore corresponds to slowing toward the
newest point. For a 10 ms step, a newly visited older speed of 3 compared
with a newer speed of 1 contributes `f32(0.4)`; reversing those speeds
contributes `f32(0.2)`.

The contribution has units of retained-coordinate units per ms². This
function performs no DPI normalization. `previous_speed` is updated at
`0x2e46c`, after the angular helper returns.

## Angular history retains a whole-degree previous direction

`sumAngleAccelerator` receives the segment's `(dx, dy)` and calls
`atan2f(dy, dx)` at `0x2e654`. The angle calculation through `0x2e67c`
is:

```text
radians = atan2_f32(dy, dx)
degrees = f32(f32(radians * -180) / pi_approx)
angle = fmod_f32(f32(degrees + 360), 360)
```

`pi_approx` at `0x1564c` has bits `0x40490fd8`, value
3.141592025756836. It differs from the nearest binary32 representation
of pi used elsewhere in the predictor.

The newest step initializes direction history. On subsequent recomputed
steps, `0x2e688`–`0x2e6bc` compares the current float angle with the
previous direction stored as an integer:

```text
angle_change = abs(f32(angle - f32(previous_angle_integer)))
if angle_change > 180:
    angle_change = f32(360 - angle_change)
angular_speed = f32(angle_change / f32(dt))
```

The helper stores `trunc_i32(angle)` at `0x2e6fc`–`0x2e704` for
the next recomputed step. The wrap chooses the shorter turn for ordinary
angles; it does not restore the discarded fractional degree.

The angular-acceleration contribution at `0x2e6d0`–`0x2e6f8` is:

```text
contribution = f32(abs(f32(angular_speed - previous_angular_speed)) / f32(dt))
angular_sum = f32(angular_sum + contribution)
previous_angular_speed = angular_speed
```

Its units are degrees per ms². The helper skips this contribution when
the endpoint index is exactly 48. The actual five-step walk uses 49,
44, 39 and so on, so that exclusion never applies: the second step at
44 already contributes against an initial previous angular speed of zero.

Independent binary32 calculations, using host `atan2f` and `fmodf`, give:

| Segment direction | Float angle | Retained integer |
| --- | --- | --- |
| `(1, 0)` | 0 | 0 |
| `(0, 1)` | 269.9999694824219 | 269 |
| `(-1, 0)` | 179.9999542236328 | 179 |
| `(0, -1)` | 90.00003051757812 | 90 |
| `(1, 1)` | 315 | 315 |

For a fully populated, uncached, constant leftward path with one-millisecond
sample spacing, that calculation produces published angular acceleration
approximately 0.005714024 degrees/ms², while speed acceleration remains zero.
This demonstrates the effect of integer direction history and the recovered
constant in the reference arithmetic. Exact device math-library results
and resulting admission behavior remain unmeasured.

## Cached steps do not refresh the running comparison state

For ordinary finite values, a step reuses its two cached contributions when:

```text
j < 44 - m
f64(cached_acceleration[j]) > f64(0.00001)
```

The first comparison is at `0x2e3ac`–`0x2e3b8`. The second widens
the cached float to double and compares it with binary64 0.00001 from
`0x15700` at `0x2e3bc`–`0x2e3c8`. Equality takes the recomputation
path. In particular, `f32(0.00001)` is slightly smaller than that double
threshold and is recomputed; its next higher binary32 neighbor is reusable.

The cached path at `0x2e3cc`–`0x2e3e4` adds the two contributions
and jumps directly to the shared count/time update. It does not recalculate
speed or direction, call the angular helper, or update previous speed,
previous angle and previous angular speed.

Thus an older freshly recomputed step after a cached step compares with
the last freshly recomputed state. A calculation that recomputes every
step is not equivalent to this implementation. Cache values also travel
with the full-record shift, while fresh tail writes preserve existing cache
bytes until recomputation overwrites them.

## Final normalization uses different denominators

Let `k` be the number of processed steps, including cached ones. At
`0x2e4f0`–`0x2e514`, fewer than two steps leave both published fields
zero. Otherwise:

```text
A = f32(acceleration_sum / f32(k - 1))
G = f32(angular_sum / f32(k - 2)) if k > 2 else angular_sum
```

`0x2e568`–`0x2e56c` stores `A` and `G` at offsets 1752 and 1756.
The angular numerator can include the second-step contribution even
though its normalizer uses `k - 2`; it is not the usual average of only
three-segment second differences. The neural gate reads these exact fields
and their float product.

## Validation and remaining work

The library bytes were matched to the identified APK. Input-call ordering,
count packing, array layout and copies, index/cutoff branches, float constants,
cache reuse, angular integer conversion and publication stores were checked
against ARM64 instructions.

Disposable numerical checks covered shifts and preserved cache bytes,
action resets, regular-spacing windows, partial-history sentinel behavior,
asymmetric weighting, cache-threshold neighbors and a cached step followed
by recomputation. Separate checks exercised direction conversion and the
constant-motion angular example. They reproduce the recovered arithmetic
and control flow; they do not execute the native estimator.

The earlier low-input-speed gate and its threshold/configuration sources
remain to be traced. Actual device sampling, cache histories and admission
outcomes require runtime evidence. No SDK code or corpus fixture changed.
