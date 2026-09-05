# InkPen2 Kalman filtering

## Evidence and scope

Confirmed by static inspection of Samsung Notes 4.4.45.37 ARM64
`libSPenPenCommon.so` from the APK identified in the
[knowledge base](README.md#sources-and-validation). The
[input trace](inkpen2-input-findings.md#result-filtering-and-the-no-result-fallback-differ)
establishes the caller: an enabled `PointBeautifier` filter processes the
replacement event before its coordinate transforms and drawing dispatch.

This trace recovers the filter's channel assignments, constructor defaults,
reset, sample order and numerical recurrence. These are live input rules;
they do not prescribe smoothing already stored SDOCX points during export.
The earlier [coordinate prediction](inkpen2-prediction-findings.md) is a
separate operation with its own time-based least-squares fit.

## Four handlers exist; only position is enabled by default

`PenKalmanFilter::PenKalmanFilter`, `0x595e8`, allocates four 208-byte
handlers. Its vector constant at `0x2c280` contains the four integers
1, 2, 4 and 8. The constructor stores these channel bits in parent offsets
32, 36, 40 and 44 at `0x59698`, then writes enabled mask 1 into offset 48
at `0x5969c`.

`transformPenEvent`, `0x59a90`, supplies these four-component measurements:

| Channel | Handler pointer offset | Mask offset / bit | Measurement vector | Call evidence |
| --- | --- | --- | --- | --- |
| Position | 0 | 32 / 1 | `(x, y, 0, 0)` | Historical X/Y getters `0x59b04`, `0x59b18`; filter `0x59b40` |
| Orientation | 8 | 36 / 2 | `(orientation, 0, 0, 0)` | Historical getter `0x59fd0`; filter `0x59ff4` |
| Tilt | 16 | 40 / 4 | `(tilt, 0, 0, 0)` | Historical getter `0x59e98`; filter `0x59ebc` |
| Pressure | 24 | 44 / 8 | `(pressure, 0, 0, 0)` | Current getter `0x59c28`; filter `0x59c4c` |

The position vector contains two coordinates and two zeros; its latter
components do not receive velocity or acceleration. The native X/Y getters
return doubles, converted to floats at `0x59b1c`–`0x59b20` before filtering.

At the recovered constructor default, this stage filters X/Y and passes
orientation, tilt and pressure through. That statement is local to this
stage: for example, upstream beautifier admission already caps finite
pressure at 1. It also does not prove the enabled mask can never change
elsewhere. `isActivated`, `0x5b2d0`, tests the stored mask against its
argument.

## Noise matrices use exact float constants

The private handler constructor at `0x596d0` receives process variance
first and measurement variance second. It initializes the matrices as
`Q = q I` and `R = r I`: identity writes occur at
`0x59720`–`0x59734` and `0x59748`–`0x5975c`, followed by scalar
multiplication through helper `0x5b2e0`.

| Channels | Parameter | Approximate value | Constant address | IEEE 754 float bits |
| --- | --- | --- | --- | --- |
| Position | q | 0.000006 | `0x2c1f8` | `0x36c9539c` |
| Position | r | 0.0002 | `0x2c1e0` | `0x3951b718` |
| Orientation, tilt | q | 0.0000005 | `0x2c1fc` | `0x350637bd` |
| Orientation, tilt | r | 0.0001 | `0x2c1e4` | `0x38d1b717` |
| Pressure | q | 0.000002 | `0x2c1c8` | `0x360637bd` |
| Pressure | r | 0.00015 | `0x2c1d4` | `0x391d4952` |

Position's r is exactly `0.00020000000949949026`, one float step above
the usual nearest-float conversion of decimal `0.0002`
(`0x3951b717`). Reconstructing it from the short decimal changes the native
parameter. The bit column preserves the actual constants for every channel.

The parent constructor loads the position pair at `0x59610`/`0x59614`,
the orientation pair at `0x59634`/`0x59638`, reuses that pair for tilt at
`0x59658`/`0x5965c`, and loads the pressure pair at
`0x5967c`/`0x59680`.

Each handler has this in-memory layout:

| Offset | Size | Meaning |
| --- | --- | --- |
| 0 | 64 | Q, four-by-four float matrix |
| 64 | 64 | R, four-by-four float matrix |
| 128 | 16 | Estimated four-component value |
| 144 | 64 | P, four-by-four covariance matrix |

These offsets are native working state, not SDOCX fields.

## Down resets the first sample; later samples correct the estimate

`getFilteredValue`, `0x5b238`, combines the enabled mask with the requested
channel bit at `0x5b250`. An inactive channel returns the supplied vector
without updating its handler. For an active channel:

- Action 0 copies the supplied vector into the estimate at offset 128 and
  resets P to identity, at `0x5b278`–`0x5b2bc`.
- Other actions call covariance prediction at `0x5b260`, measurement
  correction at `0x5b26c`, and return the updated estimate.

With event history, `transformPenEvent` processes historical index 0 through
this action-aware helper. The position call is at `0x59b40`; without
history, its current-sample equivalent is `0x59bd4`.

After historical index 0, the position loop processes indices 1 through
`history_size - 1` in ascending order. Active positions call the prediction
and correction helpers directly at `0x5a19c` and `0x5a1a8`. The current
position follows history and calls them at `0x5a6c0` and `0x5a6cc`.
Consequently, a down event carrying history resets on its first sample,
then corrects on subsequent positions; it does not reset for every sample
in that batch.

This history handling belongs to the filter's supplied event. The upstream
beautifier has its own different admission rules and constructs replacement
events before this stage.

## The correction adds fixed process variance per sample

The covariance prediction helper, `0x5b3c4`, adds Q to P through the matrix
addition helper at `0x5b3f8`. It leaves the estimate unchanged. Measurement
correction, `0x5b444`, performs:

```text
P_prior = P + Q
S = P_prior + R
D = diag(1 / S[0,0], 1 / S[1,1], 1 / S[2,2], 1 / S[3,3])
K = P_prior * D
estimate = estimate + K * (measurement - estimate)
P = (I - K) * P_prior
```

The S addition is called at `0x5b494`. The inverse-building loop explicitly
writes zero off the diagonal and divides 1 by each diagonal entry at
`0x5b4cc`–`0x5b4d0`; it does not compute a general matrix inverse.
Gain multiplication is called at `0x5b5a8`. Residual subtraction and
estimate correction occupy `0x5b5c0`–`0x5b5f4`. The final covariance
multiplication is called at `0x5b71c` and copied back at
`0x5b72c`/`0x5b734`.

For finite arithmetic after the down reset, diagonal P, Q and R remain
diagonal. X and Y therefore evolve independently with the same variance
and gain. No elapsed-time parameter enters these helpers, and prediction
adds the same Q on every correction. Sample count affects smoothing even
when two sequences span the same time interval.

The equivalent scalar recurrence must retain the float operation order.
Here `f32` means rounding to a single-precision float:

```text
p_prior = f32(p + q)
inverse_s = f32(1 / f32(p_prior + r))
gain = f32(p_prior * inverse_s)
residual = f32(measurement - estimate)
estimate = f32(estimate + f32(gain * residual))
p = f32(f32(1 - gain) * p_prior)
```

The native gain uses reciprocal followed by multiplication, rather than
one division of P by S. Matrix multiplication at `0x5b928` and the
matrix-vector correction accumulate component 1 first, then fused
multiply-add components 0, 2 and 3. The old estimate is added separately
at `0x5b5f0`. For the diagonal finite case, this reduces to the scalar
operations above.

## Reconstructed numerical examples and output boundary

Starting from a down position with X = 0 and P = I, four subsequent
measurements give the following results using the exact position constants:

| Measured X | Filtered X | Gain |
| --- | --- | --- |
| 10 | 9.998000145 | 0.999800026 |
| 20 | 15.072600365 | 0.507358551 |
| 30 | 20.290229797 | 0.349533647 |
| 30 | 22.961555481 | 0.275117338 |

These values are a disposable reconstruction of the instructions, not
captured device output. A scalar calculation and a separate four-by-four
calculation agreed on each estimate, gain and covariance in this sequence.
Constant measurements also remained constant over 500 corrections for
each of the three native parameter pairs.

The event reconstruction helper at `0x597e0` uses the filtered channel
vectors. It restores current absolute milliseconds by adding the input
down time at `0x59858` and takes nanoseconds from their separate getter.
Historical reconstruction similarly adds down time at `0x599e4` before
`AddBatch` at `0x599f8`. These timestamps are metadata for the output event;
they do not enter the recovered Kalman equations.

## Validation and remaining work

The APK digest and PenCommon byte stream were verified. Exported function
entries, imported sample getters, channel-mask data flow, exact constant
bits, down reset and arithmetic instructions were checked against the
ARM64 image. Numerical checks exercised the diagonal scalar/matrix
equivalence, including 100 updates with varying X/Y, and constant-input
behavior with explicit float rounding.
No SDK code changed and no native execution or new device fixture was used.

Result construction's geometric selection remains a separate numerical
target. Real InkPen2 SDOCX/PDF pairs are still required to establish which
settings and input paths produced stored geometry, and to measure export
fidelity. Reapplying this live-input filter to decoded points would change
the geometry that the document already stores.
