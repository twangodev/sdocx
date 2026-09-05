# InkPen2 coordinate prediction

## Evidence and scope

Confirmed by static inspection of Samsung Notes 4.4.45.37 ARM64
`libSPenPenCommon.so` from the APK identified in the
[knowledge base](README.md#sources-and-validation). This continues the
[InkPen2 input-queue trace](inkpen2-input-findings.md) into
`PointBeautifier::doPredict`, `0x5c864`.

The method fits X and Y independently against relative millisecond time,
evaluates the fits slightly ahead, and rejects excessive displacement.
An accepted candidate retains the last queued point's timestamps and
other channels. This is a coordinate-preprocessing stage; later result
selection and Kalman filtering still affect the event sent to drawing.
It is separate from the presenter's temporary Marker2 prediction bitmap.

All addresses below belong to PenCommon. The equations describe ordinary
finite input; examples are disposable numerical reconstructions, not
captured device output or native execution.

## Fitting skips nonincreasing adjacent times

The entry guard at `0x5c898`–`0x5c89c` requires at least three queued
records. The input queue is already bounded to 11 by the admission path.

The loop at `0x5c910`–`0x5cbe8` includes the first record. For each later
record, it compares its millisecond time with the immediately preceding
queued record at `0x5c994`. It includes the record only if its time is
strictly greater, through `0x5c998`; otherwise it increments a skipped
counter at `0x5c9bc`–`0x5c9c4` and omits all three fit inputs for that record.

This comparison uses the preceding record in the original queue, not the
preceding record retained by this fit. Queue admission accepts equal
timestamps, while this fitting loop skips them. Down/up admission has
different time rules, so the distinction also matters for a queue whose
times decrease.

For each included record, the method builds parallel double arrays:

```text
elapsed[i] = double(record[i].milliseconds - first_record.milliseconds)
x[i] = double(record[i].x)
y[i] = double(record[i].y)
```

Time subtraction occurs before integer-to-double conversion at
`0x5c9dc`–`0x5c9e0`. X/Y are promoted from their queued floats at
`0x5ca90` and `0x5cb3c`. Nanosecond times, pressure and pen axes are not
inputs to the fit.

## Each axis uses a two-coefficient least-squares fit

The calls at `0x5cc3c` and `0x5cc78` reach helper `0x5d9a8`, once for
X and once for Y. Each output is an initially zeroed pair of doubles.
For `m` retained records, time values `t` and one coordinate channel `v`,
the helper computes this unweighted straight-line fit in real-number
notation:

```text
sum_t  = sum(t)
sum_tt = sum(t * t)
sum_v  = sum(v)
sum_tv = sum(t * v)
determinant = m * sum_tt - sum_t * sum_t
slope     = (m * sum_tv - sum_t * sum_v) / determinant
intercept = (sum_tt * sum_v - sum_t * sum_tv) / determinant
```

The implementation obtains time powers through `pow` at `0x5da30` and
`0x5dab8`. It uses double sums and fused multiply-add for the weighted
sums and coefficient combinations. The output pair at `0x5db00` is
`[slope, intercept]`. The equations above should not replace that operation
order when investigating exact rounding.

At `0x5da64`–`0x5da68`, a determinant below the double constant `1e-7`
at `0x2c370` returns without changing the zeroed coefficients. This is a
signed comparison with the determinant, not its absolute value.

The caller then rejects an axis whose two coefficients both compare
equal to zero, at `0x5cc7c`–`0x5cca8`. Either rejected axis prevents a
candidate. Consequently a constant coordinate of exactly zero also
triggers this check, even when its fit is mathematically valid. A constant
nonzero coordinate does not trigger it. The three-record entry guard does
not require three distinct timestamps; fewer retained fit records can
reach the helper, where degeneracy is handled by the determinant check.

## The extrapolation horizon depends on retained count

The caller keeps both original queue count and skipped count. At
`0x5cd18`–`0x5cd28` it calculates the integer horizon:

```text
retained_count = queue_count - skipped_count
horizon_ms = 16 / (12 - retained_count)
```

The division is signed integer division, truncating toward zero. Under
the recovered queue bound, the divisor is positive. Example horizons are:

| Retained records | Horizon in milliseconds |
| --- | --- |
| 2 or 3 | 1 |
| 4, 5 or 6 | 2 |
| 7 | 3 |
| 8 | 4 |
| 9 | 5 |
| 10 | 8 |
| 11 | 16 |

The evaluation time uses the last record of the original queue, even
if that record was skipped by the fit:

```text
span_ms = last_record.milliseconds - first_record.milliseconds
evaluation_time = double(span_ms + horizon_ms)
predicted_x = float(fma(x_slope, evaluation_time, x_intercept))
predicted_y = float(fma(y_slope, evaluation_time, y_intercept))
```

The addition is at `0x5cd38`; the double evaluations and conversion to
floats are at `0x5cd40`–`0x5cd50`.

## Excessive displacement rejects the whole candidate

For a span of at least one millisecond, the method computes:

```text
average_speed = distance(first_record.xy, last_record.xy) / float(span_ms)
distance_limit = (average_speed * 16) * 5
predicted_distance = distance(last_record.xy, predicted_xy)
```

The ordinary distance calculation uses float differences, squared
distance with fused multiply-add, and a square root. Speed division
occurs at `0x5cd9c`; the two multiplications are at `0x5cdb0` and
`0x5cdbc`. The code also has a double-distance fallback when its
float arithmetic encounters a nonfinite intermediate.

The comparison at `0x5cdcc`–`0x5cdd0` accepts a predicted distance equal
to the limit. A greater distance skips candidate insertion; it does not
shorten the candidate to the limit. If the span is less than one, the
alternative branch at `0x5cdf8` uses a fixed distance limit of 300 and
rejects strictly greater distances at `0x5ce40`.

These are distances in the beautifier's coordinate space. This trace
does not assign a device-independent physical unit to 300 or to the
speed calculation. With identical first and last coordinates and a
positive span, the ordinary limit is zero; any nonzero predicted
displacement is rejected.

## Candidate coordinates advance while timestamps stay unchanged

Before the distance check, `0x5cd08`–`0x5cd20` copies both time fields
from the last original queued record into the candidate. It does not add
the horizon to either timestamp. After acceptance, `0x5ce44`–`0x5ce60`
copies that record's tilt, pressure, orientation, minor/major values and
resampled state. `addPredictedPoint` is called at `0x5ce6c` and appends
the complete 56-byte record.

For the three samples below, both axes fit exactly:

| Milliseconds | X | Y |
| --- | --- | --- |
| 0 | 10 | 20 |
| 8 | 26 | 44 |
| 16 | 42 | 68 |

The one-millisecond horizon evaluates the line at 17, producing `(44, 71)`.
The candidate's millisecond field remains 16, and its nanosecond field
remains the last sample's supplied value. With eleven samples following
the same line from time 0 through 80, the horizon is 16, the candidate is
`(202, 308)`, and its stored millisecond field remains 80.

This prevents treating coordinate extrapolation as proof of a new future
timestamp. The downstream event constructor adds down time back to the
retained relative milliseconds, as established in the
[result-routing trace](inkpen2-input-findings.md#result-filtering-and-the-no-result-fallback-differ).

## Validation and remaining work

The APK digest and PenCommon byte stream were verified. The fit helper,
`pow` import, determinant constant, sample skip, integer horizon, candidate
stores and distance guards were checked against their ARM64 instructions.
Disposable reconstruction used double fused multiply-add and explicit
float rounding for ten cases: short/long lines, duplicate/equal times,
zero/nonzero constant axes, closed motion, zero-span distance limits and
an insufficient queue.

These checks establish the recovered candidate computation, not measured
Samsung rendering parity. Result construction can still select candidates
geometrically, and the enabled Kalman stage can replace their coordinates.
Those are the next numerical targets. Saved SDOCX points already reflect
their recording path; applying this prediction again during export would
change the decoded geometry.
