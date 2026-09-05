# Pen-action input filtering and stroke splitting

## Evidence and scope

Confirmed by static inspection of Samsung Notes 4.4.45.37 ARM64
`libSPenComposer.so` and `libSPenDrawing.so` from the APK identified in the
[knowledge base](README.md#sources-and-validation). This follows the
[touch recorder](stroke-recording-findings.md) upstream into
`NoteWritingViewPenAction`.

This action has a pen-specific handwriting filter and a separate branch
that ends and restarts long strokes. The recovered raster-drawing bindings
connect the latter's counter to the recorded model point count. These are
input-processing rules, not instructions to filter or split an existing
decoded stroke again during export.

## The handwriting filter selects InkPen2

Composer `NoteWritingViewPenAction::OnTouch`, `0x422444`, reads byte 465
and the enable byte 464 at `0x4226b0`–`0x4226bc`. When the former is zero
and the latter is nonzero, it calls `handWritingBeautification` at
`0x4226c8`. A true result skips the ordinary drawing branch.

The constructor, `0x422354`, initializes bytes 464/465 to true/false at
`0x422378`, creates a `PointBeautifier` in member 456 and enables its filter
at `0x4223a0`. `SetBeatificationEnabled`, `0x4230e4`, writes byte 464.
On a down event, `OnTouch` sets byte 465 from the sign bit of the event time
at `0x422590`–`0x42259c`; a negative initial time therefore bypasses this
particular filter for the subsequent gesture processing.

`handWritingBeautification`, `0x422d14`, requires all of these conditions:

| Condition | Evidence |
| --- | --- |
| The pen-setting string is nonnull | Member-16 setting, slot 16, at `0x422d44`–`0x422d50`; null checks at `0x422d6c` and `0x422d88` |
| Tool type is 2 or 6 | `GetToolType` and comparisons at `0x422d60`–`0x422d90` |
| The string exactly matches `com.samsung.android.sdk.pen.pen.preload.InkPen2` | `String::CompareTo` at `0x422da0`, literal at virtual address `0x1e2ee4` |

A different name returns false. In particular, Marker2 does not enter
this filter. That does not establish that Marker2 bypasses every earlier
or later input transformation.

For a matching input, the function copies the event at `0x422dd0`, obtains
the original event's transformation matrix, inverts it at `0x422de0` and
transforms the copy with that inverse at `0x422dec`. It then passes the
copy to `PointBeautifier::OnTouch` at `0x422df8` and takes `GetResult` at
`0x422e04`.

If a result exists, it transforms that result with the original matrix at
`0x422e20` and sends the result to the drawing interface at `0x422e6c`.
The filter therefore operates after undoing the event's accumulated
transform and restores that transform before dispatching its result.
The [view input trace](view-input-transform-findings.md) identifies shared
child-view operations that accumulate this matrix. Its runtime configuration
and the numerical filter remain separate investigation targets.

If no result exists, actions 0 and 1 take a fallback: `ApplyFilter` receives
the copy at `0x422ee0`, then the drawing interface receives the original
event argument at `0x422ef4`. Other actions receive no drawing dispatch in
this no-result branch. The function still returns true at `0x423018`;
the caller does not subsequently dispatch the ordinary input. The fallback
argument identities are confirmed, but this trace does not infer the
effects of `ApplyFilter` or event-copy ownership from their names.

## The ordinary raster branch exposes the recorder's count

`NoteWritingView::createActions`, `0x425050`, supplies the drawing argument
to the pen-action constructor at `0x425358`. Its lookup follows view member
728, that object's member 632, then member 648. The pen-action base
constructor at `0x4fed14` stores the supplied `IWritingViewDrawing*` in
member 416 at `0x4fed74`.

The view-construction chain establishes that member 648 comes from the
factory at `0x50e1c8`: the outer view stores the object constructed at
`0x509494` in member 728; that constructor creates the member-632 object
through `0x509a60`; the latter stores the factory result in member 648 at
`0x509b4c`. The factory has four branches:

| Factory value | Constructor | RTTI class name |
| --- | --- | --- |
| 0 | `0x50e358` | `WritingViewRasterDrawing` |
| 1 | `0x512f98` | `WritingViewVectorDrawing` |
| 2 | `0x51811c` | `WritingViewNoCacheVectorDrawing` |
| 3 | `0x518cc0` | `WritingViewRasterDrawingWithBackground` |

This trace resolves the count interface for branch 0; the table does not
claim that all four implementations have identical input processing or
that a particular device always selects branch 0.

The raster constructor's GOT entry `0x5a2e10` resolves to vtable `0x5839e0`.
Its primary address point is `0x5839f0`; slot 208 at relocation `0x583ac0`
resolves to `0x50faf0`. That function returns the object in member 8 of the
member-64 wrapper.

The constructor creates that returned object through `0x4d0110` at
`0x50e4f4`. The helper calls the constructor at `0x4d1eb8`, whose GOT entry
`0x5a2bc0` resolves to vtable `0x580c68`. Its RTTI identifies
`LowLatencyStrokeView`. The constructor installs primary address point
`0x580c78` at `0x4d1efc`.

| LowLatencyStrokeView slot | Relocation | Implementation | Recorder operation |
| --- | --- | --- | --- |
| 208 | `0x580d48` | `0x4d4364` | `GetStrokeInfo` with release true |
| 216 | `0x580d50` | `0x4d43bc` | `GetStrokeInfo` with release false |
| 224 | `0x580d58` | `0x4d43cc` | `IsDrawn` |
| 232 | `0x580d60` | `0x4d43d8` | `GetStrokePointCount` |

These implementations retrieve the recorder through member 72 then member
40 and forward to the named Drawing imports. The release-true call is at
`0x4d43b8`; the count tail call is at `0x4d43e0`. Drawing
`TouchStrokeDrawing::GetStrokePointCount`, `0xb8160`, returns zero if no
stroke is present, otherwise calls `ObjectStroke::GetPointCount` at
`0xb8168`. The count is therefore stored input samples, not generated pen
stamps or a renderer's vertex count.

The ordinary event dispatch is also distinct from the counter lookup.
Raster slot 16 resolves to `0x50e914`, which calls the wrapper at
`0x512d1c`; that wrapper dispatches the original event through the stroke
view's slot 136 at `0x512d9c`. The subsequent
[presentation and prediction trace](stroke-prediction-findings.md) follows
that method into the presenter and distinguishes its real-event recording
from prediction drawing.

## A long gesture can become multiple objects

After its ordinary member-416 drawing dispatch at `0x4226e0`, the pen
action checks whether the original action was move, value 2, at
`0x422728`. Only this branch retrieves the stroke view and queries slot 232
at `0x42274c`.

The comparison is signed `count < 0xffdd` at `0x422750`–`0x422758`.
Thus counts below 65501 return without splitting; counts of 65501 or more
enter the split sequence:

1. Set the same event's action to up, value 1, at `0x422764`, and dispatch
   it again through the drawing interface at `0x422778`.
2. Check `IsDrawn` at `0x4227b8`. If true, pop the recorded stroke through
   slot 208 at `0x4227d4` and, if successful, pass it to the action's
   `addStrokeProcess` slot 112 at `0x42280c`.
3. Run the available completion callback, set the same event's action to
   down, value 0, at `0x42282c`, and dispatch it at `0x422840`.

The restart occurs even if the drawn/pop checks do not reach
`addStrokeProcess`. This sequence changes the action on the existing event;
it does not explicitly clear its history or manufacture a coordinate
array in `NoteWritingViewPenAction`. Downstream routing must still be
traced before asserting exactly which boundary samples are repeated in
the resulting objects.

The model's independent append ceiling is 65535, as established in the
[recording trace](stroke-recording-findings.md#repeated-coordinates-are-retained-by-the-model).
65501 is a controller trigger checked after a move dispatch, not a file
format maximum. The up event is processed before the pop, so the final
stored count need not equal 65501. This also does not establish a bound on
how many historical samples a dispatched event can add.

For this ordinary branch, a continuous user gesture can consequently
yield multiple model objects. Export should preserve their stored object
boundaries; joining adjacent strokes merely because their endpoints meet
can change per-object pen state and composition. The successful InkPen2
beautification branch returns before this ordinary split check, so the
trace does not establish the same splitting behavior for that path.

## Validation and next targets

The APK digest and the Composer/Drawing library bytes were checked against
the extracted APK entries. The filter literal, factory constructor
bindings, RTTI, primary-vtable offsets, forwarding functions and numerical
threshold were checked against the ARM64 instructions and relocations.
No new device fixtures or native execution were used, and no SDK rendering
code changed.

The [presentation trace](stroke-prediction-findings.md) establishes the
ordinary real-event dispatch and separate Marker2 V2 prediction drawable.
The [finalization](stroke-finalization-findings.md) and
[insertion](stroke-insertion-findings.md) traces resolve optional coordinate
replacement and later page-offset translation. Remaining APK targets include
transforms before recording, prediction algorithms, the PointBeautifier's
numerical behavior and insertion callbacks. Long-gesture and InkPen2 SDOCX/PDF pairs can
eventually validate stored counts, split boundaries and filtered geometry.
