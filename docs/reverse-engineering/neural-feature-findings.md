# Neural prediction feature preparation

## Evidence and scope

Confirmed by static inspection of Samsung Notes 4.4.45.37 ARM64
`libSPenPredictor.so` from the [identified APK](README.md#sources-and-validation).
The [bundled models](neural-model-findings.md) consume a flat float buffer
of per-interval features in `xysotp` order. These are differences between
successive records after coordinate rotation, rather than absolute pen
positions or raw orientation/pressure values.

This recovers the native input preparation. It does not establish tensor
rank from a decoded model, execute inference, or reproduce saved strokes.

## The task requires an exact sample count

`PredictionTask::Run`, `0x2c530`, calls
`PredictorBase::CopyRealPointsToVector`, `0x31e78`, at `0x2c6a0`.
The copy holds the real-point critical section while copying the retained
deque into a vector.

The task compares that vector's 88-byte record count with virtual slot 88
at `0x2c6e0`–`0x2c708`. Neural vtable address point `0x40998` binds
slot 88 to `GetMinPointsCount`, `0x300e8`. A count mismatch follows
the completion path at `0x2c8a0`–`0x2c8f0` without running inference.

The configured minimum is `InputSize + 1`: 13 for M16, 20 for M20
and 30 for M22. An accepted window produces one fewer interval record
than input samples.

## Rotation uses the final segment

The task calls `NNPredictorHelper::GetSampleData`, `0x264f4`, at
`0x2c720`. That helper requires at least two records and examines the
last two coordinates at `0x26560`–`0x265a0`.

For finite coordinate differences, its movement check is equivalent to
requiring a nonzero final X or Y difference. Each absolute difference is
converted to double and compared with itself multiplied by `1e-5`, the
constant at `0x15700`. This is not a fixed minimum-distance threshold.
An identical final pair fails even if earlier records moved.

For a moving final segment, define its float differences as `dx` and
`dy`. The rotation angle is formed as:

```text
angle = double(atan2f(dx, dy)) + pi / 4
```

`0x265a4` supplies `dx` as the first `atan2f` argument, while `dy`
remains the second. PLT `0x3b9f0`, relocation `0x422c8`, names
`atan2f`. The double constant at `0x156f0` is pi/4. The angle is
stored as double at `0x265cc` and converted to float before calling
`RotateOverIndex`, `0x2666c`, with the final record's index.

`RotateOverIndex` applies the rotation about that final coordinate.
For finite values, its geometric form is:

```text
relative_x = point_x - final_x
relative_y = point_y - final_y
rotated_x = final_x + cos(angle) * relative_x - sin(angle) * relative_y
rotated_y = final_y + sin(angle) * relative_x + cos(angle) * relative_y
```

The native implementation uses float sine/cosine, vector multiply and
fused multiply-add instructions at `0x26748`–`0x26794`. The geometric
formula alone does not specify its exact rounding. In ideal arithmetic,
the final segment points toward `(-1/sqrt(2), 1/sqrt(2))` after rotation.

The loop copies timestamps and other channels, replacing XY at
`0x267b0`. Orientation and tilt are not rotated alongside the coordinates.
Rotation happens before per-axis DPI scaling.

## The helper constructs differences and checks time order

`NNPredictorHelper::ExtractFeature`, `0x2699c`, walks adjacent rotated
records starting at index 1. For each pair it constructs an 88-byte
derived record:

| Derived offset | Value |
| --- | --- |
| 16 | Current millisecond time minus previous millisecond time |
| 32/36 | Current rotated XY minus previous rotated XY |
| 48 | Current tilt minus previous tilt |
| 52 | `min(current_pressure, 1) - min(previous_pressure, 1)` for finite inputs |
| 56/60 | Differences of the two orientation fields |
| 64 | Length of the rotated XY difference |
| 68 | Optional angle feature; zero when `A` is absent |

The time subtraction is at `0x26a58`–`0x26a64`; coordinate, tilt
and capped-pressure differences are at `0x26b3c`–`0x26b44`.
The record stores are at `0x26b78`–`0x26b98` and their allocation
counterparts at `0x26c34`–`0x26c58`.

The underlying real-record time field comes from
`MotionEvent::GetEventTime()` at `0x2f1d0`, paired with the separate
nanosecond value at `0x2f254`. The feature helper uses the millisecond
field, not the separate nanosecond or VSync-aligned timestamp.

Its time rules are:

- A negative interval immediately returns failure through `0x26d14`.
- A zero interval still produces a row and increments a counter.
- Two or more zero intervals anywhere in the window cause failure at
  `0x26cf8`–`0x26d34`. They need not be consecutive.
- One zero interval is accepted by this helper.

Pressure is capped above at 1 before subtraction, with no lower clamp
in this helper. Orientation differences outside the double interval
[-pi, pi] are logged at `0x26aa8`–`0x26adc`; this branch does not wrap
or reject them. Later feature checks remain separate.

All bundled feature strings omit `A`, so its optional calculation at
`0x26aec`–`0x26b38` is skipped for these configurations.

## DPI and time normalization

`PredictionTask::SetTFParams`, `0x2be88`, obtains X/Y DPI through
predictor slot 200 at `0x2bf18`–`0x2bf1c`. The neural vtable binds
this to `PredictorBase::GetDpi`, `0x300f8`.

That getter returns false when either axis equals zero. In that case,
`0x2bf24`–`0x2bf30` replaces both task DPI values with 411.0. It is
a zero check, not a positive-finite validation rule.

Task offset 108 receives the time normalizer. With `f32` denoting rounding
to IEEE-754 binary32 after each indicated operation:

```text
sample_period_us = f32(1_000_000 / f32(model_sampling_rate))
padded_period_us = f32(sample_period_us * f32(1.15))
time_normalizer_us = f32(padded_period_us * f32(minimum_sample_count - 1))
```

`0x2bf38`–`0x2bf78` performs these operations in that order. The
rate is converted from unsigned integer. The constants at `0x15628`
and `0x15648` have bit patterns `0x49742400` and `0x3f933333`:
1,000,000 and the binary32 approximation to 1.15.

For the bundled configuration values, the normalizers are:

| Model | Normalizer in microseconds |
| --- | --- |
| M16 | 38,333.33203125 |
| M20 | 60,694.44140625 |
| M22 | 69,479.1640625 |

These depend on each model's sampling rate and sample count, independently
of the display refresh period in the
[prediction callback timing entity](predictor-timing-findings.md).

## Each interval produces six adjacent floats

`PredictionTask::ExtractFeature`, `0x2d194`, selects a scalar by the
feature character. For the bundled `xysotp` sequence:

| Character | Value from the derived record | Native evidence |
| --- | --- | --- |
| `x` | `f32(f32(delta_x / dpi_x) * f32(25.4))` | `0x2d1c0`, `0x2d1e4`, `0x2d274`–`0x2d280` |
| `y` | `f32(f32(delta_y / dpi_y) * f32(25.4))` | `0x2d26c`–`0x2d280` |
| `s` | `f32(f32(f32(delta_ms) * 1000) / time_normalizer_us)` | `0x2d214`–`0x2d228`, `0x2d25c` |
| `o` | `f32(delta_orientation / f32(pi))` | `0x2d1fc`, `0x2d244`–`0x2d25c` |
| `t` | `f32(delta_tilt / f32(pi / 2))` | `0x2d250`–`0x2d25c` |
| `p` | Derived pressure difference unchanged | `0x2d264`–`0x2d268` |

Thus `s` encodes a normalized time interval. It is not a distance divided
by elapsed time. The XY conversion expresses differences in millimetres
when the configured DPI matches the input coordinate units.

The exact scale constants are:

| Address | Bits | Float value |
| --- | --- | --- |
| `0x15650` | `0x41cb3333` | 25.399999618530273 |
| `0x15640` | `0x40490fdb` | 3.1415927410125732 |
| `0x15620` | `0x3fc90fdb` | 1.5707963705062866 |

The task loops over derived records at `0x2c784`–`0x2c878`, with an
inner loop over feature-string characters. It calls scalar extraction at
`0x2c7cc` and writes one float while advancing the destination by four
bytes at `0x2c7d4`. The resulting layout is:

```text
x1, y1, s1, o1, t1, p1, x2, y2, s2, o2, t2, p2, ...
```

| Model | Interval rows | Flat float count |
| --- | --- | --- |
| M16 | 12 | 72 |
| M20 | 19 | 114 |
| M22 | 29 | 174 |

These are counts and memory order derived from the native caller.
They do not identify the tensor's declared rank or dimensions. The
signature-runner path obtains the tensor named `input` through
`0x2c604`–`0x2c62c`; the alternate path obtains the interpreter's
first input and requires its type code to equal 1.

A registered per-feature checker can reject a scalar at
`0x2c858`–`0x2c860`, after the buffer store and before inference.
Passing the sample-count and time checks is therefore insufficient to
prove that the model runs.

## Independent numerical checks

For a derived interval with `delta_x = 10`, `delta_y = 20`,
`delta_ms = 4`, `delta_orientation = f32(pi/2)`,
`delta_tilt = f32(pi/4)` and `delta_pressure = 0.25`, use X/Y DPI
254/508 and the M16 normalizer. The reconstructed float row is:

```text
1.0, 1.0, 0.1043478325009346, 0.5, 0.5, 0.25
```

Its binary32 bit patterns are `3f800000`, `3f800000`, `3dd5b451`,
`3f000000`, `3f000000` and `3e800000` respectively. This example starts
with already-derived coordinates, keeping rotation-library accuracy
separate from the scalar conversion.

A disposable reference calculation checked this row, all three normalizers
and buffer widths, one versus two zero intervals, decreasing time,
stationary-tail rejection, pressure saturation, negative pressure and
unwrapped orientation differences. Ideal rotation geometry was checked
for five segment directions without claiming bit-exact native trigonometry.

## Validation and remaining work

The APK digest and extracted library byte stream were verified. Function
bindings, referenced instructions, record fields, vtable slots, constants,
buffer strides and Markdown links were checked against the local evidence.

Remaining work includes declared tensor metadata, per-feature checker
configuration, output selection and rejection, the output rotation and
actual device behavior. The reference checks do not run Samsung's native
code or establish rendering conformance. No SDK code changed.
