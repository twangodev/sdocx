# Display position and prediction presentation time

## Evidence and scope

Confirmed by static inspection of Samsung Notes 4.4.45.37 ARM64
`libSPenComposer.so`, `libSPenEngine.so` and `libSPenBase.so`, plus fresh
decompilation of `SpenLatencyConfiguration` from the APK identified in the
[knowledge base](README.md#sources-and-validation).

Composer's `PresentTimeFinder::CalcPresentTime`, `0x4d4be4`, converts a
rectangle's position along the display scan direction into top and bottom
presentation delays. The [uniform-latency controller](uniform-latency-findings.md)
uses the bottom delay for a point rectangle at the saved anchor. This is
an input to temporary prediction timing, not a document coordinate
transform or an SDOCX timestamp conversion.

## Java setters identify the native configuration fields

Engine `LatencyConfigurationFactory::GetInstance`, `0xc4a40`, returns the
singleton at `0x194000`, constructed through `0xf9cb8`. Its GOT entry
`0x18bdd8` resolves to vtable `0x17bb40`, with primary address point
`0x17bb50`; RTTI at `0x185e18` names `SPen::LatencyConfiguration`.

JNI registration at `0xc57c0`–`0xc57d4` binds 12 entries at `0x192de0`
to `com/samsung/android/sdk/pen/engine/SpenLatencyConfiguration`.
The relevant entries and their native dispatch establish:

| JNI method | Table entry | JNI function | Configuration setter / field |
| --- | --- | --- | --- |
| `Native_setUniformLatency(Z)V` | `0x192e88` | `0xc5670` | Slot 152, `0xfae40`, byte 122 |
| `Native_setHWRotation(I)V` | `0x192eb8` | `0xc56d0` | Slot 184, `0xfaee0`, integer 124 |
| `Native_setHWRefreshRate(F)V` | `0x192ed0` | `0xc56fc` | Slot 200, `0xfaf30`, float 128 |
| `Native_setScreenOrientation(III)V` | `0x192ee8` | `0xc5728` | Slot 216, `0xfaf88`, integers 132/136/140 |

Fresh decompilation of `updateRefreshRate` confirms that the last setter
receives `Display.getRotation()`, then `DisplayMetrics.widthPixels` and
`heightPixels` after `getRealMetrics`. Its separate
`Native_updateRefreshRate` call receives `Display.getRefreshRate()`.

The constructor supplies `getHwRotation()` and `getHwRefreshRate()` to
the hardware setters. These methods include device-specific configuration
and fallback behavior, so hardware rate must not be equated with the
current `getRefreshRate()` result. The native constructor initializes
hardware rotation/rate and screen orientation/dimensions to zero, while
its separate refresh-rate member 112 starts at 60.

This establishes the native field identities and their Java sources for
this APK. It does not establish the values active on a particular device.

## Controller setup copies configuration into the helper

Composer `PredStrokeLengthController::EnableUniformLatency`, `0x4d67c4`,
obtains these getter slots from the configuration singleton:

| Getter slot | Engine implementation | Value | Helper offset |
| --- | --- | --- | --- |
| 224 | `0xfaf94` | Screen orientation | 32 |
| 232 | `0xfaf9c` | Real display width | 52 |
| 240 | `0xfafa4` | Real display height | 56 |
| 192 | `0xfaf28` | Hardware rotation | 48 |
| 208 | `0xfaf80` | Hardware refresh rate | Used to derive offset 40 |

The Composer stores are at `0x4d681c`, `0x4d6824` and `0x4d6838`.
The helper's first two 16-byte fields are the visible-view and
visible-screen rectangles, copied by setters `0x4d6c2c`/`0x4d6c44`.
Its 64-bit member 40 is the frame duration in nanoseconds; member 60 is
the diagnostic level.

When hardware rate compares equal to zero, setup uses its supplied rate
argument instead. It computes:

```text
frame_duration_ns = trunc_to_i32(f32(1_000_000_000 / selected_rate))
```

The division at `0x4d6860` is float, conversion at `0x4d6864` is signed
32-bit, and `0x4d6868` sign-extends before storing the helper's 64-bit
field. For selected rates 60, 90 and 120, disposable reconstruction gives
16,666,667, 11,111,111 and 8,333,333 ns respectively. The constructor's
initial 16,666,666 ns literal is not the result of the 60-rate float
calculation; setup can change that value by one nanosecond.

## The guard checks view and screen rectangles, not the input point

`CalcPresentTime` returns without changing either output when the
visible-view rectangle is empty, the visible-screen rectangle is empty,
or either real display dimension equals zero. Those tests occur at
`0x4d4c40`–`0x4d4c60`. Base `RectF::IsEmpty`, `0xb11bc`, tests
left >= right or top >= bottom for ordinary finite coordinates.

The helper does not perform that emptiness check on its input rectangle.
Consequently the controller's zero-area point rectangle remains usable.
The controller initializes both output delays to zero before the call at
`0x4d59b0`; a guard return leaves those zeros in that caller. It is not an
unconditional zeroing operation by `CalcPresentTime` itself.

## Rotation selects the scan axis and direction

At `0x4d4c64`–`0x4d4c7c`, the helper adds screen orientation and hardware
rotation as 32-bit integers and takes a signed remainder modulo 4. For
the ordinary nonnegative rotation values, let:

```text
rotation = (screen_orientation + hardware_rotation) % 4
sx0 = rectangle.left - visible_view.left + visible_screen.left
sx1 = rectangle.right - visible_view.left + visible_screen.left
sy0 = rectangle.top - visible_view.top + visible_screen.top
sy1 = rectangle.bottom - visible_view.top + visible_screen.top
```

The resulting position-to-time map in real-number notation is:

| Effective rotation | Top position | Bottom position | Divisor |
| --- | --- | --- | --- |
| 0 | sy0 | sy1 | Real height |
| 1 | sx0 | sx1 | Real width |
| 2 | Real height - sy1 | Real height - sy0 | Real height |
| 3 | Real width - sx1 | Real width - sx0 | Real width |

The forward-X branch starts at `0x4d4d44`, forward-Y at `0x4d4d60`,
reverse-Y at `0x4d4d04`, and reverse-X at `0x4d4ca8`. Reversed directions
exchange the rectangle edges as well as subtracting from the display
extent. The width/height divisor selection is at `0x4d4c80`–`0x4d4c90`.

The native code reconstructs the far edge as `left + Width()` or
`top + Height()` before the translation. Base `Width`/`Height`,
`0xb108c`/`0xb109c`, subtract the corresponding float endpoints.
Each subtraction/addition above is float arithmetic, so simplifying
the expressions algebraically can change rounding.

For each resulting position, the final conversion is:

```text
delay_ns = trunc_to_i64(f32(f32(position / f32(display_extent)) * f32(frame_duration_ns)))
```

Division, period conversion, multiplication and integer truncation occur
at `0x4d4d88`–`0x4d4dbc`. No clipping to the visible rectangle, display
extent or `[0, frame_duration_ns]` occurs in this calculation. Negative
or beyond-screen positions can produce delays outside that interval.

## Numerical examples

With coincident view/screen origins, real dimensions 1000 by 2000,
frame duration 16,000,000 ns and input rectangle `(200, 500, 400, 900)`:

| Effective rotation | Top delay, ns | Bottom delay, ns |
| --- | --- | --- |
| 0 | 4,000,000 | 7,200,000 |
| 1 | 3,200,000 | 6,400,000 |
| 2 | 8,800,000 | 12,000,000 |
| 3 | 9,600,000 | 12,800,000 |

For the point rectangle `(200, 500, 200, 500)`, both outputs are equal:
4,000,000, 3,200,000, 12,000,000 or 12,800,000 ns for rotations 0–3.
Combining screen rotation 1 with hardware rotation 3 returns rotation 0's
result, demonstrating the additional hardware orientation term.

With view origin `(10, 20)`, screen origin `(100, 200)`, point `(210, 520)`
and rotation 0, translated Y is 700 and both delays are 5,600,000 ns.
For coincident origins, Y = -100 produces -800,000 ns and Y = 2500
produces 20,000,000 ns. These examples check the lack of a local clamp;
they are not assertions about points admitted by a particular device.

The uniform-latency controller adds the bottom delay to its frame-timing
expression before bounding against the prediction span. A longer bottom
delay can therefore change the retained prediction fraction, subject to
the separate [cutoff rules](uniform-latency-findings.md).

## Validation and remaining work

The APK digest and all three native byte streams were verified. JNI
registration entries, singleton construction, vtable targets, native
setter/getter fields, rectangle operations and conversion instructions
were checked. Fresh class decompilation confirmed the Android rotation
and real-pixel dimension arguments. Disposable reconstruction checked all
four rotations, combined hardware rotation, point rectangles, translated
origins, unclamped positions, unchanged outputs on guard returns and
float-derived frame durations.

The helper's numerical mapping and configuration field identities are
established statically. The external prediction callback's timing producer,
runtime display configuration and device timing behavior remain separate
work. No SDK code changed and no device capture or new SDOCX fixture was
used. This live display-timing model should not be reapplied to stored
stroke timestamps during export.
