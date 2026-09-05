# Uniform-latency prediction cutoff

## Evidence and scope

Confirmed by static inspection of Samsung Notes 4.4.45.37 ARM64
`libSPenComposer.so` from the APK identified in the
[knowledge base](README.md#sources-and-validation). This continues
[`PredStrokeLengthController`](prediction-length-findings.md) into the
optional uniform-latency calculation inside `TransformPredStrokeLength`,
`0x4d555c`.

The stage computes a time fraction, retains a prefix of the prediction
sequence and can interpolate the cutoff segment's X/Y and timestamps.
Its cumulative distance calculations do not set the interpolation weight.
The separate final index budget still limits the result afterward.
These are temporary presentation rules, not stored Marker2 replay rules.

All equations and examples below assume finite arithmetic, a nonempty
sample list and a positive prediction span representable as a signed
32-bit nanosecond count. The examples reconstruct the instructions; no
native execution or device prediction capture was used.

## Setup controls the stage independently of the sample budget

The controller constructor initializes byte 104 to zero at `0x4d54cc`.
Slot 56, `0x4d67c4`, is identified by signature string `0x1bec9e` as
`EnableUniformLatency(float)`. It loads configuration through
`LatencyConfigurationFactory::GetInstance`, including the enable flag
through configuration slot 160 at `0x4d6880`, and writes its low bit into
byte 104 at `0x4d6888`.

The setup method can clear that flag again when either the visible-view
rectangle at offset 108 or visible-screen rectangle at offset 124 is
empty, through `0x4d68a8`–`0x4d68dc`. The diagnostic at `0x1bece9`
names both rectangles. Setters `0x4d6c2c` and `0x4d6c44` copy them into
the controller and its member-144 presentation-time helper.

The method obtains a rate from configuration slot 208. When that rate
compares equal to zero, `0x4d685c` uses the supplied float argument.
It computes a frame duration as float `1,000,000,000 / rate`, truncates
to a signed 32-bit integer, and stores it at controller offset 140 and
helper offset 40, at `0x4d6860`–`0x4d6870`.

This helper duration is distinct from the period supplied in the
prediction callback's timing structure below. Their runtime relationship
must be established at the producers rather than assumed from similar
names. Constructor state alone also does not prove the stage is enabled
for any device or pen configuration.

## Working records preserve axes but replace other metadata

After removing the input event's accumulated matrix, the method collects
pointer-0 history, then current, into 72-byte `PointerCoords` records.
Historical stores at `0x4d56b0`–`0x4d56c8` and equivalent current stores
at `0x4d5858`–`0x4d5874` establish:

| Fields | Working-record values |
| --- | --- |
| Milliseconds | Native relative getter plus input down time |
| Nanoseconds | Separate native nanosecond getter |
| X/Y | Native double coordinates after inverse transformation |
| Pressure, tilt, orientation | Their supplied float channels |
| Minor, major | Zero |
| Resampled state | -1 |
| Raw X/Y | Zero |

Thus this reconstruction already changes touch-size, raw-coordinate and
resampled metadata even if uniform latency is disabled. The final native
`AddBatch` implementation has its own raw-coordinate behavior when these
working records become history, as described in the
[adapter trace](motion-event-adapter-findings.md).

The saved anchor comes from the controller's
[non-resampled selection](stroke-prediction-findings.md#prediction-length-control-selects-a-non-resampled-anchor).
The method copies it and applies the same inverse matrix at `0x4d55f4`.
It converts the anchor's X/Y to floats at `0x4d5988`, then constructs a
zero-area rectangle `(x, y, x, y)` at `0x4d5994`–`0x4d5998` for the
presentation-time calculation.

## The coefficient combines callback timing and presentation delay

The third method argument is a `MotionEventEntity` timing structure.
The following names describe the fields' role in this consumer; they do
not assert the complete producer-side schema:

| Entity offset | Local name | Read evidence |
| --- | --- | --- |
| 8 | reference_ns | `0x4d597c`, `0x4d59fc` |
| 16 | timing_ns | `0x4d59c4` |
| 24 | alignment_ns | `0x4d59c4` |
| 32 | frame_ns | `0x4d59dc` |

For a positive `frame_ns`, the alignment calculation is:

```text
if alignment_ns == 0:
    aligned_ns = timing_ns
else:
    aligned_ns = alignment_ns + floor((timing_ns - alignment_ns) / frame_ns) * frame_ns
```

The native code uses signed integer division at `0x4d59e8`, followed by
subtracting one period if the candidate is later than `timing_ns`, at
`0x4d59f0`–`0x4d59f8`. That correction gives the floor behavior above
even when `timing_ns` precedes the alignment origin.

`PresentTimeFinder::CalcPresentTime`, `0x4d4be4`, receives the transformed
anchor rectangle at `0x4d59b0`. Its signature at `0x1de00a` and diagnostic
strings identify top and bottom presentation delays. Both outputs are
initially zero at `0x4d5984`; this consumer uses the bottom output loaded
at `0x4d5a08`. Let that output be `bottom_delay_ns`.

The coefficient follows:

```text
span_ns = signed32(last_prediction.nanoseconds - reference_ns)
requested_ns = aligned_ns + 2 * frame_ns - reference_ns + bottom_delay_ns
selected_ns = min(requested_ns, span_ns)
coefficient = f32(f32(selected_ns) / f32(span_ns))
```

The original 64-bit span subtraction is at `0x4d59cc`; subsequent uses
explicitly take signed `w25`, including `0x4d5a18`–`0x4d5a20` and
`0x4d5b10`. It is not a general 64-bit duration calculation throughout.
The two-period addition and bottom-delay addition are at `0x4d5a00` and
`0x4d5a14`; the minimum is selected at `0x4d5a20`. The float division is
at `0x4d5b14`, with an equivalent path through `0x4d5aac` when logging
is enabled.

There is no lower clamp of `requested_ns` to zero here. The denominator
also has no explicit zero guard in this recovered calculation. Those
facts should not be replaced by an assumed generic clamp to `[0, 1]`.

The timing reference comes from entity offset 8, rather than the saved
anchor record's nanosecond field. One local presenter producer supplies
member 328 at entity offset 8, at `0x4d8a34`. Member 328 is the last
historical nanosecond time when history exists, otherwise current time,
at `0x4d7a74`–`0x4d7a88`. That local producer stores `GetNano()` at
offset 16 and zero at offsets 24/32. The external callback at `0x4da4bc`
instead copies all 40 bytes of its supplied entity before forwarding it.
Its upstream timing source remains a separate trace target.

## The cutoff uses time fractions, not fractions of path length

The first pass constructs a 96-byte auxiliary record per prediction:

| Offset | Meaning |
| --- | --- |
| 0 | Float distance from the preceding coordinate |
| 8 | Signed 64-bit nanosecond delta from the preceding time |
| 16, 20 | Current X/Y converted to floats |
| 24 | Copy of the 72-byte working `PointerCoords` |

The first predecessor is the transformed saved anchor for X/Y, and
`reference_ns` for time. Subsequent predecessors are original prediction
samples. Time delta is calculated at `0x4d5bcc`, distance at
`0x4d5bd0`–`0x4d5be0`, and total distance accumulates at `0x4d5dc0`.

In the second pass, each segment receives this fraction:

```text
step = f32(double(segment_delta_ns) / double(span_ns))
after = f32(before + step)
```

The division is double precision before conversion to float at
`0x4d5e94`–`0x4d5ea0`; accumulation is float at `0x4d5f48`.
Distance ratios at `0x4d5eb8`–`0x4d5ed0` belong to the logging branch.
They do not determine `step` or the cutoff weight.

The exact finite-input control flow is:

```text
before = 0
for each original prediction sample:
    if before > coefficient:
        stop
    after = f32(before + step)
    if coefficient <= 1 and before < coefficient and after > coefficient:
        weight = f32(f32(coefficient - before) / step)
        append interpolated sample
    else:
        append original sample
    before = after
```

Stopping uses strict greater-than at `0x4d5f38`–`0x4d5f3c`.
Interpolation requires both strict inequalities at
`0x4d5f74`–`0x4d5f8c`, as well as the upper test at `0x4d5f50`.
If this pass produces no records, `0x4d6254`–`0x4d6318` supplies the
first original prediction record as a fallback. A negative coefficient
therefore need not yield an empty event.

## Interpolation changes X/Y and both timestamps only

For a crossing segment, the method updates X/Y using float subtraction,
multiplication and addition at `0x4d5f9c`, `0x4d5fb4` and `0x4d5fbc`:

```text
coordinate = f32(previous_coordinate + f32(f32(endpoint - previous_coordinate) * weight))
```

For each timestamp channel, it subtracts the integer endpoints, converts
to doubles, uses double fused multiply-add with the float-derived weight,
then truncates back to a signed integer at `0x4d5fa0`–`0x4d5fc0`:

```text
time = trunc(fma(double(endpoint_time - previous_time), double(weight), double(previous_time)))
```

For the first segment, the previous nanoseconds are `reference_ns` and
previous milliseconds are `trunc(double(reference_ns) / 1,000,000)` from
`0x4d59c8`–`0x4d59d8`. Later segments use their preceding original
sample's two timestamps. The saved anchor's own millisecond field is not
used for that first interpolation.

The stores at `0x4d5fd8`–`0x4d5fdc` replace the copied record's two
timestamps and double X/Y. They leave pressure, tilt and orientation at
the segment endpoint values. The stage does not interpolate these channels
alongside coordinates.

The X/Y store also runs for retained endpoints that do not need
interpolation. Those doubles are promoted from the auxiliary float X/Y
at `0x4d5fc8`, so enabling this stage can round coordinates to float
precision even when the coefficient retains the full prediction sequence.

## Exact equality can retain the following whole sample

Consider an anchor at X = 0 with reference time 1000 ms, and predictions
at `(1008 ms, X=10)`, `(1016 ms, X=30)`, `(1024 ms, X=60)` and
`(1032 ms, X=100)`. Their nanosecond values match those milliseconds.
Y is zero throughout. Each segment has time fraction 0.25 despite their
different coordinate lengths.

| Coefficient | Working output X values | Last output milliseconds |
| --- | --- | --- |
| 0.125 | `[5]` | 1004 |
| 0.375 | `[10, 20]` | 1012 |
| 0.5 | `[10, 30, 60]` | 1024 |
| 0 | `[10]` | 1008 |
| -0.1 | `[10]`, via empty-pass fallback | 1008 |
| 1 | `[10, 30, 60, 100]` | 1032 |

At coefficient 0.5, the third iteration starts with `before == 0.5`.
It passes the strict stop test but fails the strict interpolation test,
so it appends the entire third sample. Only the following iteration stops.
At coefficient zero the same equality retains the first whole sample.

The adjacent floats illustrate the discontinuity in this synthetic case:

| Coefficient bits | Coefficient | Last working X | Last milliseconds / nanoseconds |
| --- | --- | --- | --- |
| `0x3effffff` | 0.4999999701976776 | 29.999998092651367 | 1015 / 1015999999 |
| `0x3f000000` | 0.5 | 60 | 1024 / 1024000000 |
| `0x3f000001` | 0.5000000596046448 | 30.00000762939453 | 1016 / 1016000001 |

These are pre-limit working results. The
[final index budget](prediction-length-findings.md#the-controller-constructs-a-new-event-from-a-bounded-prefix)
can select an earlier sample, so the table does not predict the visible
tail without the controller's counter state and caller configuration.
It also does not establish that any specific hardware input hits these
exact synthetic boundary values.

## Validation and remaining work

The APK digest and Composer byte stream were verified. Setup gates,
getter bindings, timing-field loads, signed span conversion, coefficient
arithmetic, strict cutoff branches, coordinate/time stores and fallback
were checked against the ARM64 image. Disposable calculations covered
six coefficient cases, six cutoff cases, the adjacent-float boundary,
both timestamp channels and retention of endpoint pressure.

The presentation-time helper's complete orientation/geometry contract
and the external callback's timing producer remain to be documented.
Configuration values and runtime enablement also need their own evidence.
These findings establish the local numerical behavior without new SDOCX
files, but matching device exports remain necessary to measure rendering
fidelity. No SDK code changed.
