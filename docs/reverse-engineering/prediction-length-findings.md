# Prediction length control

## Evidence and scope

Confirmed by static inspection of Samsung Notes 4.4.45.37 ARM64
`libSPenComposer.so` from the APK identified in the
[knowledge base](README.md#sources-and-validation). The
[presentation trace](stroke-prediction-findings.md) binds presenter member
528 to `PredStrokeLengthController` and establishes the separate Marker2
prediction drawable.

This trace recovers the controller's final sample-prefix selection,
counter updates and event reconstruction. Its optional uniform-latency
stage runs before this selection and remains a separate numerical target.
Disabling uniform latency does not bypass the final sample-prefix limit.
None of these steps directly appends prediction samples to Marker2's
stored stroke array.

## The controller constructs a new event from a bounded prefix

The primary vtable address point is `0x580f20`. Slot 24 at `0x580f38`
resolves to `0x4d555c`, identified by the signature string at `0x1c5596`
as `PredStrokeLengthController::TransformPredStrokeLength`.
`OnPredictTouch` calls it at `0x4d96d4` for a nonnull prediction event with
pointer-0 tool type 2, checked at `0x4d96b4`–`0x4d96bc`.

The method first saves the event's accumulated matrix, applies its inverse
to the input event at `0x4d55d0`, and copies/transforms the controller's
saved anchor into the same coordinate space at `0x4d55e8`–`0x4d55f4`.
It gathers pointer-0 history in ascending index order, followed by current,
into 72-byte `PointerCoords` records. Its optional uniform-latency branch
can replace that working list. The disabled branch at `0x4d5970` reaches
the common final selection at `0x4d645c`.

For a nonempty working list of N records, the final selection is:

```text
selected_index = min(index_budget, N - 1)
next_bound = selected_index + 2
output.current = working[selected_index]
output.history = working[0:selected_index]
```

The comparison and selected-index store are at `0x4d6478`–`0x4d6488`.
The current-record copy uses the selected index at `0x4d64b4`; the new
one-pointer event is constructed at `0x4d6530`. The loop at
`0x4d6540`–`0x4d6568` appends exactly the preceding records as history.
Later records do not enter that result.

An index budget of zero therefore selects the earliest supplied prediction
sample. When history exists, this need not be the input event's current
sample. The saved real-event anchor is not automatically inserted into
the output history by this final selection.

The method applies the saved matrix back to both the input event and the
new result at `0x4d6574` and `0x4d6580`, then returns the new event at
`0x4d65ac`. This restores their coordinate frame; it is not a guarantee of
bit-identical X/Y after inverse/forward floating-point transforms.

## Constructor and setup initialize the index budget

The constructor's vector at `0x1f93d0` contains integers `(0, 1, 0, 5)`.
The store at `0x4d549c` initializes these members:

| Offset | Meaning | Initial value |
| --- | --- | --- |
| 8 | Index budget | 0 |
| 12 | Exclusive bound for the next budget increment | 1 |
| 16 | Update phase | 0 |
| 20 | Update period | 5 |

Slot 40, relocation `0x580f48`, resolves to setup method `0x4d6788`.
It resets budget/bound to `(0, 1)`, resets phase to zero, and chooses:

```text
period = max(1, trunc(f32(input_rate * 0.0833333358168602)))
```

The multiplier is the exact float at `0x1f911c`, bits `0x3daaaaab`.
Multiplication and float-to-integer conversion occur at
`0x4d6794`–`0x4d6798`; `0x4d67a4`–`0x4d67ac` enforce the minimum 1.
For finite positive input rates, 12, 60, 90 and 120 produce periods
1, 5, 7 and 10 respectively. This is truncation, not rounding to the
nearest integer.

The presenter supplies member 396 at `0x4d8370` and calls this slot at
`0x4d837c`. Its constructor initializes that float to 60 at
`0x4d6fc0`/`0x4d6fe4`. These are constructor/setup observations, not a
measurement of any device's runtime refresh rate or prediction cadence.

## A true update advances the budget periodically

Slot 32, relocation `0x580f40`, resolves to `0x4d671c`. For a true input
boolean it performs:

```text
phase = (phase + 1) % period
if phase == 0 and index_budget + 1 < next_bound:
    index_budget += 1
```

The native signed divide/remainder is at `0x4d6734`–`0x4d673c`.
The bound comparison and conditional increment are at
`0x4d6748`–`0x4d6758`. Final output selection updates `next_bound`, so
the amount that can be exposed depends on the supplied working list.
Selection clamps the index used for output without writing the clamped
value back into the budget itself.

`OnPredictTouch` supplies this boolean at `0x4d9a70`. It is the negated
`RectF::IsEmpty` result for the bounds returned by helper `0x4da0b8`,
through `0x4d989c` or `0x4d9a50`. It should not be described as a timer
that advances unconditionally on every input event or display frame.

For a synthetic fixed working list with X values `[10, 20, 30, 40]`,
constructor period 5, and one true update after each selection:

| Selection calls | Selected current X | Historical X values |
| --- | --- | --- |
| 1–5 | 10 | None |
| 6–10 | 20 | `[10]` |
| 11–15 | 30 | `[10, 20]` |
| 16 onward | 40 | `[10, 20, 30]` |

This is an instruction-derived state sequence, not a measured timing
schedule. The budget can reach 4 for this four-record list, while final
selection continues clamping to index 3.

## A false update resets only the exact InkPen2 name

The false branch compares the supplied pen string with
`com.samsung.android.sdk.pen.pen.preload.InkPen2`, constant `0x1e2ee4`,
through `String::CompareTo` at `0x4d676c`. A nonzero comparison returns
without changing the counters. Equality writes zero to index budget and
phase at `0x4d6774`–`0x4d6778`.

Marker2 therefore retains those counters on this false-update path.
InkPen2 resets them. Neither branch changes period or next bound here.
This is a pen-specific controller rule, separate from the upstream
[InkPen2 beautifier](inkpen2-input-findings.md).

## Validation and remaining work

The APK digest and Composer byte stream were verified. Vtable targets,
signature/name strings, sample collection order, matrix calls, final
prefix construction, constructor constants and counter branches were
checked against the ARM64 image. Disposable reconstruction checked five
setup rates, 21 successive output selections, a shorter supplied list,
Marker2 counter retention and exact-name InkPen2 reset.

The uniform-latency coefficient and cutoff interpolation still need their
own documented contract, including caller timing fields and display
configuration. No native execution, new device fixture or SDK change was
used here. The recovered sample prefix belongs to temporary prediction
presentation; stored Marker2 replay should continue to use decoded points.
