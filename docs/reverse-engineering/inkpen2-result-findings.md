# InkPen2 output selection and candidate lifetime

## Evidence and scope

Confirmed by static inspection of Samsung Notes 4.4.45.37 ARM64
`libSPenPenCommon.so` and `libSPenBase.so` from the APK identified in the
[knowledge base](README.md#sources-and-validation). This continues the
[coordinate prediction](inkpen2-prediction-findings.md) into
`PointBeautifier::getPredictedPenEvent`, PenCommon `0x5d05c`.

Prediction appends candidates to a separate vector. Result construction
selects a current candidate and some earlier candidates as history, resets
their resampled state to -1, and consumes the vector on success. Its two
distance checks use different predecessors. They do not establish a
300-unit maximum gap between every pair of output samples.

The selection below describes ordinary finite arithmetic. These are input
preprocessing rules before the [Kalman stage](inkpen2-kalman-findings.md),
coordinate transforms and recorder dispatch, not export-time rules for
already stored strokes.

## The candidate vector is separate from the real-point deque

`PointBeautifier` owns these members:

| Parent offset | Meaning | Evidence |
| --- | --- | --- |
| 120, 128, 136 | Candidate vector begin, end, capacity end | Append `0x5db60`–`0x5db90`; end update `0x5dc9c` |
| 144 | Real-point deque | `addRealPoint`, `0x5d444` |
| 208 | Saved 56-byte anchor record | Down copy `0x5c610`–`0x5c618`; result copy `0x5d290`–`0x5d298` |
| 232, 236 | Saved anchor X/Y, within that record | Coordinate loads through `0x5d0c0`/`0x5d0c4` |

Candidate entries use the same in-memory
[`HistoricalEvent` layout](inkpen2-input-findings.md#queue-records-retain-both-time-channels)
as the real-point queue. `addPredictedPoint`, `0x5db4c`, copies the 56-byte
record and advances the vector's end. This vector is not the real-point
deque's rolling 11-sample fit window.

Every `OnTouch` starts by setting candidate end equal to begin at
`0x5c050`–`0x5c054`. It then processes that input event and invokes
prediction, potentially once per historical admission attempt. Thus
candidates belong to the current `OnTouch` batch; failed result selection
does not retain them across the next `OnTouch` call.

Down admission copies the admitted current record into the saved anchor
at `0x5c610`–`0x5c618`. Successful result construction later replaces that
anchor with its selected current candidate. These are coordinates before
the subsequent Kalman correction and output transforms. The saved anchor
should not be equated with the final filtered coordinate sent to drawing.

## Current selection scans backward using original adjacency

At `0x5d094`–`0x5d0a0`, equal candidate begin/end returns a null result.
Otherwise the method starts with the last candidate. For candidate index
`i`, its comparison reference is:

```text
reference = candidates[i - 1] if i > 0 else saved_anchor
```

The reference selection occurs at `0x5d100`–`0x5d118`. The method computes
Euclidean X/Y distance and compares it against float 300, whose bits
`0x43960000` are loaded at `0x5d0d0`. A distance greater than 300 moves
backward through `0x5d140`–`0x5d148`. Equality is accepted. The first
passing candidate encountered becomes the new event's current sample.

If every candidate fails, the method returns null through `0x5d0e0`
without changing the candidate end or saved anchor. Repeating result
selection alone therefore encounters the same candidates; the next
`OnTouch` clears them as described above.

For `i > 0`, this test uses the immediately preceding original candidate,
even if that predecessor will later be omitted from output history. It
does not compare every possible current sample directly with the saved
anchor or the last accepted historical sample.

## History selection scans forward from the saved anchor

After choosing current index `j`, the method constructs the event from
`candidates[j]` through `initNewPenEvent` at `0x5d184`. If `j > 0`, it
examines candidates 0 through `j - 1` in ascending order:

```text
previous = saved_anchor
history = []
for candidate in candidates[0:j]:
    if distance(previous, candidate) <= 300:
        history.append(candidate)
        previous = candidate
```

The first iteration loads the saved anchor at `0x5d1d4`–`0x5d1e0`.
The threshold comparison is at `0x5d208`, and distances above 300 skip
the append through `0x5d20c`. Passing candidates call native `AddBatch`
at `0x5d23c`, then update the comparison coordinates at `0x5d248`.
Skipped candidates do not update that reference.

Base `MotionEvent::AddBatch`, `0xc01b4`, appends the supplied sample into
its historical vector at implementation offsets 80/88/96. The ordinary
append stores the sample pointer and updates history end at
`0xc0258`–`0xc025c`; it does not promote that supplied historical sample
to current. The current sample remains the one chosen before this loop.

Both distance calculations subtract X/Y as floats, compute a float square
sum with fused multiply-add, and take a float square root. If the initial
square sum produces a nonfinite check result, separate paths at
`0x5d150` and `0x5d25c` recompute the norm using double products before
converting back to float. The two branch conditions differ for unordered
comparisons, so the finite-input pseudocode above is not a general NaN
sanitization contract. The threshold is in this stage's coordinate space;
this trace does not assign it a physical unit.

## Reconstructed examples expose the two predecessor rules

For these one-dimensional examples, Y is zero and saved anchor X is zero.
Numbers label candidate X coordinates in original order:

| Candidates | Selected current X | Historical X values | Reason |
| --- | --- | --- | --- |
| `[300]` | 300 | None | Equality passes |
| `[300.0000305175781]` | No result | None | Next float above 300 fails |
| `[100, 200, 900]` | 200 | `[100]` | Tail gap 700 fails; preceding gap 100 passes |
| `[100, 1000, 200, 210]` | 210 | `[100, 200]` | Skipping 1000 leaves 100 as the history reference |
| `[100, 1000, 1010]` | 1010 | `[100]` | Current passes against 1000, which history omits |
| `[1000, 1010]` | 1010 | None | Current passes original adjacency even when no history passes |
| `[500, 1000]` | No result | None | Both candidate tests fail |

The `[100, 1000, 1010]` case has a 910-unit gap from its last historical
sample to current. The `[1000, 1010]` case produces current 1010 despite
the saved anchor being zero. These follow from separate selection passes;
the current sample is not rechecked against the final accepted history.

These are synthetic candidate-vector examples. They establish the local
selector's behavior, not that a particular device gesture necessarily
produces these candidates through the earlier fit and rejection stages.
The two-dimensional boundary `(180, 240)` also passes at distance 300,
while `(300, 300)` fails; this is a Euclidean threshold, not separate
per-axis bounds.

## Selected output loses the candidate resampled classification

Before current-event construction, `0x5d174`–`0x5d178` overwrites the
selected candidate's resampled field at record offset 52 with -1.
`initNewPenEvent` copies that field into the native record at
`0x5d3bc`–`0x5d3c4`.

The history loop also writes -1 at `0x5d1cc` before testing each candidate,
including ones subsequently skipped. Its `AddBatch` call explicitly
passes -1 at `0x5d238`. Consequently the output event's selected current
and accepted historical samples carry -1 rather than preserving a
candidate's earlier 0 or 1 classification.

This occurs after admission and prediction copied the channel. It is
separate from Marker2's
[non-resampled anchor selection](stroke-prediction-findings.md), which
reads resampled classifications without rewriting the input samples.
Names containing prediction do not imply that output samples are marked
resampled = 1.

The current constructor retains the candidate's nanoseconds and restores
absolute milliseconds with saved down time at `0x5d3b4`. The history loop
loads both candidate time fields at `0x5d224` and adds down time at
`0x5d230`. Neither geometric pass compares these time fields or creates
new timestamps.

## Success consumes the entire candidate vector

After construction, `0x5d28c` sets candidate end equal to begin. It clears
the whole pending vector, including candidates after the selected current
index and historical candidates omitted by the distance check. It does
not remove only the selected records or preserve a rejected suffix for
the next result call.

The selected current record is copied to parent offset 208 at
`0x5d290`–`0x5d298`, including its rewritten resampled value. A second
result request without another append sees an empty vector. The upstream
real-point deque has a separate lifetime and is not cleared here.

`GetResult` subsequently runs the enabled Kalman filter and coordinate
transform. The caller's
[no-result fallback](inkpen2-input-findings.md#result-filtering-and-the-no-result-fallback-differ)
remains relevant when the candidate vector is empty or every candidate
fails current selection.

## Validation and remaining work

The APK digest and both native byte streams were verified. The vector
offsets, down-anchor copy, reverse/forward branches, exact threshold,
native history append, resampled stores and complete-vector clear were
checked against ARM64 instructions. Disposable reconstruction covered
empty input, exact/adjacent float boundaries, two-dimensional distances,
tail rejection, skipped-history references, large resulting gaps,
all-rejected candidates and a nonzero saved anchor.

These checks complete the local candidate-selection contract without
native execution or a new device fixture. They do not establish visual
parity, settings prevalence, or which synthetic candidate sequences are
reachable from actual hardware input. Matching InkPen2 SDOCX/PDF fixtures
remain the way to validate stored geometry and rendered output. The SDK
should preserve decoded samples rather than replaying these admission,
prediction, selection and smoothing stages during export.
