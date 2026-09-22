# FountainPen live tip investigation

> The reproduction commands in this note refer to experiment scripts and
> fixtures removed during test cleanup, including `conformance/fountain-live.json`
> and `conformance/fountain-smoother.json`. Recover them from Git revision
> `40de721` in a separate checkout. Saved V16 geometry in the SDK, plus
> `conformance/fountain_native.py` and `conformance/fountain_v14_native.py`,
> are current. Live tip rendering is not implemented. See
> [current validation](../../conformance/README.md#native-geometry-checks).

Saved-stroke redraw and live-event rendering are separate paths. The V16
geometry in `ink/fountain.rs`, and the archived V14 reconstruction, do not
reproduce PointTipManager's live state machine.
This document records the verified configuration layer and the remaining
event-processing work, from Samsung Notes 4.4.45.37's hash-pinned libraries.

## Applicability correction

The cubic StrokeSmoother and PointBeautifier/Kalman sections below reconstruct
shared PenCommon components explored while tracing the live pipeline. They
must **not** be treated as required FountainPen preprocessing. The verified
WritingViewPenAction gate selects PointBeautifier specifically for InkPen2
with tool type 2 or 6; FountainPen takes the ordinary branch. See the engine
selection evidence at the end of this document. WidthSmoothManager and
PointTipManager remain directly established parts of the fountain path.

## Configuration

PointTipManager's constructor (`PenCommon 0x54784`) starts with an active
event limit of 20, maxima of 100 events / 50,000 microseconds / 50 distance
units, and DPI `(375, 375)`. FountainPen's constructor overrides the event
maximum to 30 and the time maximum to 150,000, then calls SetTipLength with
unit 0 and value 100 (`FountainPen 0x61304..0x61340`). These overrides matter;
the generic PointTipManager defaults are not the FountainPen defaults.

`SetTipLength(Unit, long long)` (`PenCommon 0x54a58`) stores the original
64-bit length and unit, then interprets the signed low 32 bits of length as a
percentage clamped to 0..100. It updates one limit, retaining the others:

| Unit value | Updated field |
| --- | --- |
| 0 | Event maximum times percentage, truncated integer division by 100 |
| 1 | Microsecond maximum times percentage, truncated integer division by 100 |
| 2 | Distance maximum times percentage, float32 division by 100 |
| 3 | No limit field updated |
| Other values | Same field update as unit 0 |

This table describes the setter, not the complete retention semantics.
`GetTipLength` returns the original unclamped 64-bit input. Maxima setters
write their fields without recomputing the current active limit. `SetDpi`
stores separate float32 horizontal/vertical DPI. StoreAllEvents is a separate
boolean.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_tip_native.py
```

The oracle executes the native constructor, configuration setters and getters
with the existing bounded ARM64 harness. It verifies all 66 unit/length
combinations, including negative values, values above 100, signed-32-bit
overflow and 64-bit values, plus DPI and StoreAllEvents toggles. No live-event
prediction is claimed by this test.

## Verified live queue partitioning

`conformance/fountain_live_native.py` executes real MotionEvent constructors,
history batching, channel getters, cloning, PointTipManager::Update, and all
three non-tip/original-tip/rendered-tip event getters. Unlike the saved-stroke
primitive oracle, it does not stub MotionEvent getters. The shared ELF loader
now resolves weak function exports too, allowing the actual native matrix
constructors, transforms and inverse to execute. Input events are checked by
round-tripping their native getters before Update receives them.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_live_native.py --output /tmp/fountain-live.json
python3 conformance/fountain_live_model.py
python3 conformance/fountain_live_model.py /tmp/fountain-live.json
```

The independent scalar model matches every output channel exactly over 50
sequences / 1,845 updates. Cases cover all four units, lengths 0/50/100,
several event rates, long-distance motion, batched versus individual events,
prediction replacement/removal, and seeded mixed sequences with duplicate
timestamps and varying pressure/tilt. `fountain-live.json` at revision `40de721`
contains inputs, observable tip counts, and SHA-256 digests of each complete
native snapshot. The full optional output contains those snapshots for
inspection. Both native and scalar tools verify the same references; the
digests cover all three output events, their actions and every time/position/
pressure/tilt value, not just the counts.

The verified partition rules are:

- Down and up update the position anchor but do not enqueue a sample or flush
  the retained queue. This describes PointTipManager itself; the surrounding
  pen's down/up behavior must be traced separately.
- Move appends every historical sample and then the current sample, preserving
  pressure and tilt. Native MotionEvent getters expose timestamps relative to
  down time. The test uses down time 1000 ms and verifies that normalization.
- Supplied predicted samples are kept separately from original samples. The
  original-tip event contains retained real samples; the rendered-tip event
  appends the currently supplied prediction. A subsequent move without a
  prediction removes the old prediction. Up does not itself remove it.
- Unit 0 retains at most the configured event count, reduced by truncating
  the float32 predicted time horizon in milliseconds divided by 2.77777767.
  The resulting count is floored at zero (`0x560f8..0x56118`).
- Unit 1 commits real samples while their age relative to the last predicted
  sample (or latest real sample when prediction is absent), times 1000,
  exceeds the configured microsecond limit. Equality remains in the tip
  (`0x56810..0x56828`). Even a zero limit retains the latest real sample when
  there is no later prediction.
- Unit 2 uses accumulated path distance in millimeters, including the
  predicted extension, against the configured distance limit. Input positions
  are inverse-transformed, converted with horizontal/vertical DPI and 25.4,
  then accumulated with float32 arithmetic. The current model verifies the
  identity-transform, 375-DPI case; other transforms remain to be tested.
- Unit 3 commits all real samples. Committed samples accumulate in the
  non-tip array until the caller clears it; these sequences deliberately do
  not invoke that clearing operation.

The native harness supplies predictions as the second Update argument. This
verifies how predictions are consumed, not how another component generates
their coordinates. GetTipStrokeEvent assembles those points; matching its
output does not establish an upstream prediction algorithm.

## Remaining live pipeline

The next native entry is `PointTipManager::Update` (`0x54c6c`), which consumes
the current and historical MotionEvent channels and invokes
`updatePointLists` (`0x55ce8`). The latter dispatches by unit and partitions
the tip/non-tip data. The event-count path also adjusts its count using time
differences, so it must not be replaced by a simple last-N-points queue.

GetNonTipPointList (`0x56b00`), GetOriginTipStrokeEvent (`0x56cc0`) and
GetTipStrokeEvent (`0x56e68`) are covered for the event sequences above.
GetTipPointsCoord (`0x573a8`), SetTipPointsCoord (`0x57608`) and
GetSmoothedPoints (`0x57774`) are covered by the coordinate sequences below.
The main-to-tip state-copy blocks are now covered below. The enclosing live
geometry and down/move/up width-smoothing transitions now have an executable
native integration harness below. Prediction flags, non-identity transforms,
non-stylus input and GPU rendering remain outside that harness. Neither
configuration nor queue partitioning alone establishes live stroke parity.


## Coordinate editing and point extraction

`conformance/fountain_tip_coords.py` executes 30 native sequences: zero, one,
two, four and 35 moves; 0/1/17-ms intervals; with and without predictions;
and two successive coordinate edits per sequence. Run with:

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_tip_coords.py
```

The scalar expectations match every returned channel, including NaN cases:

- GetTipPointsCoord returns retained real points with double coordinates.
  Timestamps are relative to the first move in these sequences, **not** the
  first currently retained point: after five points leave the queue, the
  first returned time is five intervals rather than zero.
- SetTipPointsCoord pairs replacements by index. Supplied timestamps are
  ignored, even when deliberately unrelated and decreasing. Disassembly
  shows a count mismatch throws `std::logic_error`; this exception path is
  not executed by the current harness.
- Let `w = f32(f32(t - first_time) / f32(last_time - first_time))` using
  retained native timestamps. Each coordinate becomes
  `fma(replacement, double(w), old * double(f32(1 - w)))`.
  Thus the earliest point stays fixed and the last receives the full edit.
  Repeated edits operate on already edited coordinates. Pressure, tilt and
  event timestamps are preserved, and separate predictions remain unchanged.
- A single point or all-identical timestamps produces NaN coordinates through
  zero-duration division. This is observed native behavior, not a suggested
  SDK policy.
- Both the original-tip and rendered-tip events expose the edited real
  coordinates. “Original” does not mean immutable pre-edit coordinates.
- With the default storage configuration, GetSmoothedPoints returns retained
  real coordinates converted to float32, duplicating both endpoints. It
  excludes predictions and committed non-tip points. Empty input returns one
  `(0, 0)` point. No smoothing arithmetic runs in this getter. Its additional
  stored-point branch and StoreAllEvents mode remain unverified.

The separate `StrokeSmoother::Transform` entry (`0x58c44`) dispatches through
an implementation vtable. Its algorithm and enclosing caller still need
execution-based reconstruction; the getter name alone is not evidence for it.

## Cubic stroke smoother: executable reference

`conformance/fountain_smoother_native.py` now executes the concrete
`CSAPSPenStrokeSmoother::Transform` (`0x70788`) through its complete native
`csaps::UnivariateCubicSmoothingSpline` construction, sparse solve and evaluation.
The 42 cases in `conformance/fountain-smoother.json` at revision `40de721` cover lengths
0/1/4/5/10/30, strengths 0/25/50/75/100, time offsets and intervals, duplicate
and backward timestamps, and all-identical timestamps. Run:

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_smoother_native.py
```

This is a native reference. The independent fitted-coordinate model is
described in the following section. Allocation, memory primitives, logging, strlen and host glibc libm
(pow/powf/expf) are callbacks. Android libm bit parity is not established.
The direct concrete entry avoids needing dynamic vtable relocations; the
wrapper and enclosing fountain caller are not exercised by these cases.

Observed implementation:

- Fewer than five input records returns false without edits.
- It builds time/X/Y arrays, accepting only strictly increasing float32
  timestamps relative to the first input timestamp. Other records map to the
  last accepted index; output timestamps themselves remain unchanged.
- The constructor stores a float32 factor: strength <= 0 gives 1, strength
  >= 100 gives 0. Between those bounds the factor is
  `1 - expf(-5.841355800628662 * powf(1 - strength/100, 0.6666666865348816))`,
  with float32 intermediate arithmetic.
- Default weights are 1 at the endpoints and 1/N inside. Additional weights
  favor the beginning of the stroke; the branch depends on the first timestamp
  and retained smoother state, reconstructed in the following section.
- X and Y are fitted independently over the accepted times and evaluated at
  those same times. The result is mapped back to every original record.

The independent scalar reconstruction of `csaps::NormalizeSmooth` (`0x651f0`)
exactly matches 36 native cases. For span L, sample count N, weights w and
input factor s:

```
effective_times = 1 + L² / sum((t[i] - t[i-1])²)
effective_weights = sum(w)² / sum(w²)
k = 80 * abs(L)³ * N⁻² * effective_times⁻½ * effective_weights⁻½
normalized = float32(s / fma(1-s, k, s))
```

The formula uses double arithmetic until its final float conversion. Degenerate
normalization beyond the covered cases and alternate spline/state modes
remain outside the independent model described below. This research does not
change saved-stroke rendering or establish that this smoother must be applied
again to already saved coordinates.

## Independent spline reconstruction

`conformance/fountain_smoother_model.py` now independently reproduces the
smoother's fitted coordinates at its accepted sample times, using dense
Gaussian elimination rather than the APK's Eigen sparse solver. It requires
Python 3.13+ and host libm, but no APK, Unicorn, NumPy or spline package.

```sh
python3 conformance/fountain_smoother_model.py
```

The original 42 cases match within 5.51e-14 coordinate units. Five additional
state sequences (`conformance/fountain-smoother-state.json`) exercise 65
transforms: growing prefixes of 4 through 80 points, followed by sliding
30-point windows, at five strengths. Across both sets the maximum difference
is 1.28e-8, occurring at strength 100. Checks use 1e-8 for the original cases
and 1e-7 for longer stateful cases. These are numerical comparisons, not
bit-identical solver claims. Native references still use host libm callbacks.

For accepted times with spacings h, build an N-by-(N-2) matrix Q with column i:
`[1/h[i], -1/h[i]-1/h[i+1], 1/h[i+1]]` at rows i through i+2. R is
tridiagonal, diagonal `2*(h[i]+h[i+1])`, adjacent off-diagonal `h[i+1]`.
With diagonal weight matrix W and normalized smoothing factor p, each axis
solves:

```
A = 6*(1-p)*transpose(Q)*inverse(W)*Q + p*R
u = solve(A, transpose(Q)*y)
fitted = y - 6*(1-p)*inverse(W)*Q*u
```

This reconstructs the output needed by Transform, which evaluates only at
its input knots. It does not yet reconstruct arbitrary-time spline evaluation
or all native sparse-solver rounding behavior.

The default weighting/state path is now matched as well:

- Start with endpoint weights 1 and interior weights 1/N.
- When the first input timestamp is zero, add `15*sqrt(1-i/40)` to the
  first min(N,40) weights, with float32 arithmetic at every operation.
  Remember the relative time of the last boosted point as the anchor.
- Otherwise find the first accepted relative time at least
  `anchor - first_input_timestamp`. If found at index k, walk backward from
  k to zero. At backward offset j, add `10*sqrt(j/40)`, preserving the native
  float32 calculation order `sqrt(1 + (40-j)/-40)`.
  This branch does not update the anchor.
- Non-increasing float32 times reuse the last accepted coordinate index.
  All-identical times return false without rewriting input coordinates.

Remaining scope includes the alternate distance-based weighting flag,
non-default enable/configuration paths, arbitrary-time evaluation, integration
with the enclosing live fountain caller, and the prediction generator. The
independent model is research conformance code; production rendering is not
changed by these commits.

## V16 caller verification and cubic-smoother applicability

The cubic smoother above is a verified **PenCommon component**, not an
established step in the fountain pen. The earlier exploration of its name
must not be read as evidence that V16 applies cubic fitting to input strokes.
A scan of dynamic imports across the extracted APK libraries found no imports
of `StrokeSmoother::Transform` or `GetTipPointsCoord`; this negative static
result alone cannot exclude indirect/dynamic use elsewhere.

The actual V16 event entry is `SetEventsForDraw` (`0x776dc`):

1. A null current event takes the error path (not exercised by this harness).
2. Down calls PointTipManager::Reset and resets PenTolerance to the down
   position converted from double to float32.
3. Every non-null event and its supplied prediction are passed directly to
   PointTipManager::Update. No coordinate-edit or cubic-fit call intervenes.

`conformance/fountain_ingress_native.py` executes that complete entry with
real MotionEvents and native manager/tolerance calls. The surrounding drawable
pointers are assembled from the inspected native object layout. All 1,845
updates match the existing queue reference snapshots, and a second down after
40 moves clears the committed data, retained tip and prediction. The following
move matches a fresh manager. An instruction hook rejects any execution of
exported StrokeSmoother/CSAPSPenStrokeSmoother entries during these tests.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_ingress_native.py
```

The Draw path is a separate boundary: it obtains/clears committed non-tip
samples (`0x76730..0x7673c`) and calls movePen on them (`0x76754`). At stroke
end it obtains the real tip (`0x7678c`), completes width smoothing when enabled
(`0x767c8`), and feeds movePen/endPen (`0x767e4`, `0x767f8`). The separate
FountainPenStrokeTipDrawableGL gets real-plus-predicted tip samples at
`0xa4094`. The executable Draw integration below now covers these calls;
GPU execution remains separate. Their smoothing is WidthSmoothManager; the cubic
coordinate smoother is not demonstrated on this path. Upstream prediction
and preprocessing still require tracing outside the fountain event entry.

The next concrete upstream lead is libSPenEngine's imports of
PointBeautifier::OnTouch, ApplyFilter and GetResult. libSPenPredictor also
exports CreatePredictor and NNPredictor methods. Their presence identifies
investigation targets, not proof that a particular predictor/model is enabled
for fountain strokes on a given device.

## Upstream PointBeautifier execution

The engine contains a concrete caller that constructs PointBeautifier
(`libSPenEngine.so:0x1394c4`) and enables its filter (`0x1394dc`). Its event
routine copies a MotionEvent, calls OnTouch (`0x13a108`), obtains GetResult
(`0x13a114`), and passes a non-null result to a virtual consumer. On up, it
also calls ApplyFilter (`0x13a1a0`) before passing the original up event to
that consumer. The consumer's identity and the conditions selecting this
engine path remain unverified; do not assume every fountain stroke uses it.

`conformance/fountain_beautifier_native.py` executes the complete PenCommon
OnTouch/GetResult path with the optional PenKalmanFilter enabled and disabled.
It records 12 sequences / 216 input events: 1/10/50-ms intervals, individual
and three-sample historical batches, down/move/up and changing pressure.
No prediction or filtering function is stubbed. Only host allocation/memory,
logging and double `pow` are callbacks. Identity transforms and stylus tool 2
are used. These are native references, not an independent filter model.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_beautifier_native.py
```

Observed boundaries:

- All 216 input events remain unchanged by OnTouch; modified coordinates are
  returned in separate events. Batching and interval change output availability.
- Disabling the Kalman filter does **not** disable PointBeautifier's coordinate
  processing. For down (0,0), then moves (1,5) and (2,0), each 10 ms apart,
  the unfiltered result is approximately (2.1,1.666667); the filtered result
  is approximately (2.099580,1.666333), at the second move's timestamp.
- At 10-ms intervals the first down and first move return no result in these
  sequences. At 50 ms the output schedule differs. A universal fixed-count
  or fixed-latency assumption is not supported.
- GetResult (`0x5cfe8`) obtains getPredictedPenEvent, conditionally applies
  PenKalmanFilter::transformPenEvent, then transforms the returned event into
  the stored coordinate frame.
- ApplyFilter (`0x5d304`) returns false when disabled or given a null event.
  When enabled with a valid event, it calls transformPenEvent and destroys
  the returned event. In 24 executed enabled/disabled checks, the original
  event remains unchanged and the return value matches the enabled flag.
  The null-event branch is static evidence only.

This separates PointBeautifier's own prediction/coordinate processing from
its optional Kalman stage. Both algorithms and the engine selection/consumer
still need independent reconstruction and caller verification. The separate
NNPredictor library must not be assumed to generate these particular results.

## Independent Kalman reconstruction

`conformance/fountain_kalman_model.py` reconstructs the diagonal handler used
by PenKalmanFilter. `conformance/fountain_kalman_native.py` executes native
getFilteredValue (`0x5b238`) and compares outputs plus all 16 covariance entries
on 600 deterministic updates across the four constructor-created handlers.
Every float32 value matches exactly, including enabled updates, disabled-mask
bypass, up actions and repeated down resets.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_kalman_native.py
python3 conformance/fountain_kalman_model.py
```

The second command requires no APK or Unicorn. It takes the independently
captured *unfiltered* PointBeautifier results as input and exactly reproduces
all 92 filtered output samples in the six corresponding enabled-filter
sequences, including their historical batches. This isolates and verifies the
Kalman contribution; it does not independently generate the unfiltered data.

The handler maintains a four-component value and a diagonal covariance P.
Initial values are zero and P is identity. All components share the same
scalar covariance under the constructor's diagonal configuration. With f32
rounding after **each** arithmetic operation:

```
Ppred = f32(P + Q)
K = f32(Ppred * f32(1 / f32(Ppred + R)))
x = f32(x + f32(K * f32(measurement - x)))
P = f32(f32(1 - K) * Ppred)
```

Down copies the measurement and resets P to 1. Disabled masks return the
measurement without changing stored state, even for down. Other tested
actions (move and up) use the same update. The reciprocal-then-multiply gain
and separate multiplication/addition are required for the observed exact
float32 agreement. This stage has no timestamp-dependent transition term.

Native constructor noise values are:

| Handler | Q | R |
| --- | --- | --- |
| Position | 6.000000212225132e-6 | 0.00020000000949949026 |
| Orientation | 4.999999987376214e-7 | 9.999999747378752e-5 |
| Tilt | 4.999999987376214e-7 | 9.999999747378752e-5 |
| Pressure | 1.9999999949504854e-6 | 0.0001500000071246177 |

The position R is one float32 step above rounding the decimal literal 0.0002;
using that shorter literal loses exact agreement. The native constructor's
activation mask is 1, so the tested normal event path filters position while
preserving pressure and tilt. The primitive tests deliberately call all four
handlers directly; they do not establish that the engine enables those other
channels. Non-diagonal/custom configurations and the wrapper's other tools,
transforms and pointer counts remain unverified. PointBeautifier's preceding
prediction algorithm and engine selection still require reconstruction.

## Independent PointBeautifier prediction math

`conformance/fountain_prediction_model.py` reconstructs doPredict (`0x5c864`)
from a supplied retained real-point queue. The native verifier checks 148
constructed queues and also observes 264 actual doPredict calls inside the
12 OnTouch sequences. It reads the native input queue at entry, independently
computes the expected result, observes addPredictedPoint without replacing it,
and compares at return. Emission/rejection, coordinates, timestamp, pressure
and tilt match exactly in every case. No native prediction code is stubbed.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_prediction_native.py
```

The synthetic queues cover 0/1/2/3/4/8/10/12/13 points, 0/1/10/50-ms intervals,
linear, zigzag, zero-X and seeded random coordinates, plus duplicate/backward
times. They deliberately include counts that may not be admitted by the
normal event queue; those cases establish primitive behavior only.

The reconstructed algorithm:

1. Fewer than three retained records emits nothing. Otherwise include the
   first record, and include each later record only when its time exceeds
   its immediate predecessor's time. This comparison is against the original
   predecessor, **not** the last accepted record.
2. Subtract the first retained timestamp and fit X and Y independently by
   ordinary linear least squares in double precision. Input coordinates have
   already been rounded to float32. The native two-coefficient solve uses
   fused multiply-adds; determinant less than 1e-7 leaves zero coefficients.
3. If either axis has both coefficients exactly zero, reject the prediction.
   Thus a stroke lying on X=0 or Y=0 can hit this native rejection condition;
   it is not an independent-model error.
4. For M accepted records, the extrapolation horizon is integer division
   `16 / (12-M)` truncated toward zero. AArch64 SDIV produces zero when the
   denominator is zero. Evaluate both lines at the last retained relative
   timestamp plus that horizon, then round to float32.
5. Reject a result farther from the last retained point than
   `80 * distance(first,last) / elapsed_ms`, preserving native float32
   operation order. If elapsed time is less than one millisecond, use 300
   instead. The distance check uses endpoint displacement, not path length.
6. An accepted predicted record keeps the **last real record's timestamp**,
   pressure and tilt while replacing its coordinates. The extrapolation
   horizon does not advance that stored timestamp.

The independent model matches the predictor calculation inside real events,
but still receives the retained queue from native code. Queue admission,
trimming, output assembly/deduplication, complete transform handling and
engine path selection remain separate unfinished work. This is distinct from
the NNPredictor component and from the PointTipManager's consumption of supplied
predictions. No production renderer behavior changes in this increment.

## Independent event preprocessing model

`conformance/fountain_beautifier_model.py` now combines admission, retained
queue state, prediction, output assembly and optional Kalman filtering without
reading native queue state or consuming native intermediate results. It
matches 468 complete events in 18 sequences, including whether an output exists,
action, timestamps, coordinates, pressure and tilt. It also matches the
observable input-event coordinates after OnTouch.

```sh
python3 conformance/fountain_beautifier_model.py
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_beautifier_native.py
```

The six added `fountain-beautifier-stress.json` sequences use seeded mixed
single/historical batches, duplicate and backward timestamps, pressure outside
[0,1], and coordinate scales 1/100/1000, with filtering off/on. Native references
are regenerated only with the explicit `--write-reference` option.

Verified state and assembly behavior:

- Each OnTouch clears pending predictions. Down replaces the real queue with
  its current point at relative time zero and resets the previous-output
  anchor. Down also initializes the enabled Kalman stage.
- A move with history processes its historical samples, **excluding the
  current sample**. A move without history processes its current sample.
  Prediction runs after each admission attempt, including a rejected attempt.
- Move rejects times older than the last real sample, but accepts equal times.
  Prediction subsequently handles the equal-time records as described above.
- Admission retains the last 11 real records. Up appends its current sample
  without the move timestamp guard, predicts, then clears the real queue.
- Coordinates and channels are float32; pressure is capped at 1 in the real
  queue with no lower clamp for these finite samples.
- Output assembly scans candidates backward until one is at most 300 units
  from its preceding candidate (or from the previous output anchor at index
  zero). If none qualifies, it returns no event.
- It constructs an event with that candidate as the current point and visits
  earlier candidates in order, including only jumps of at most 300 from the
  last accepted point. The current point remains present even if earlier
  candidates were excluded. It updates the anchor to the chosen current point,
  then applies optional Kalman filtering. The event keeps the input action.

The earlier 216 regular-event checks showed no input mutation because their
coordinates were already exactly representable. Broader evidence corrects any
universal inference from that: MotionEvent's identity transform rounds double
input coordinates through float32. The new model reproduces this conversion;
input pressure is not capped by that transform, even though queue admission
caps its copied pressure. This effect is about coordinate precision, not the
irregular timestamps that happened to reveal it.

Current verification covers single-pointer stylus events, identity transforms,
finite coordinates and the native harness's fixed down-time convention.
Non-identity transforms, metadata outside the captured channels, engine path
selection, and integration into a complete live drawable/GPU execution still
remain. This establishes an independent model for the tested preprocessing
path, not completion of all fountain rendering behavior.


## Engine selection: PointBeautifier is an InkPen2 special case

The constructor at Engine `0x139410` belongs to WritingViewPenAction, as
identified by its log tag and adjacent RTTI name at `0x71b90`. It constructs
and enables a PointBeautifier, but construction alone does not select it for
every pen.

The event gate at `0x139eb0` tests tool type 2. Other tool types take the
secondary check at `0x139ffc`, which accepts only type 6. Both accepted types
then require a non-null name equal to
`com.samsung.android.sdk.pen.pen.preload.InkPen2` (constant `0x642d4`, compare
at `0x13a020`). Only that combination reaches the helper call at `0x13a030`
that invokes PointBeautifier::OnTouch/GetResult. All other combinations,
including FountainPen, reach the ordinary branch at `0x139d18`.

`conformance/fountain_engine_gate.py` executes these native instructions for
48 combinations: tool IDs 0..7 and null/empty/FountainPen/InkPen/InkPen2/
InkPen2X names. It uses real MotionEvents and their native tool getters.
String::CompareTo is a host lexical comparison reading the actual native
constant. No gate instruction is replaced. The test stops at the two branch
successors; it does not execute the entire writing-view state machine.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_engine_gate.py
```

The additional Engine library is hash-pinned:
`79a8024586ce58ceeca613ddfce159e92274c597c5a863dd0aaf3e8e32e1b505`.
The enclosing gate itself is reached only under earlier writing-view conditions;
the local test does not broaden those conditions or rule out unrelated callers
elsewhere. It does establish that this traced PointBeautifier route is not the
normal FountainPen route. Applying its reconstructed prediction or Kalman
processing to saved fountain coordinates would therefore lack supporting
caller evidence and could change the stroke incorrectly.

Further fountain work should follow its actual main/tip Draw orchestration,
width-history backup/restore, supplied prediction source and rasterization.
Alternate modes of these separately reconstructed components are not, by
themselves, missing fountain features.

## Incoming prediction length and presenter acceptance

The object at TouchPresenter offset 528 is a **PredStrokeLengthController**,
not the predictor that generates future coordinates. The presenter constructor
calls Engine factory `0xfb6cc`, which allocates 152 bytes and calls constructor
`0xff15c`. Its vtable relocation at `0x18be18` resolves to `0x17bf88`;
slot 24 is `TransformPredStrokeLength` (`0xff24c`) and slot 32 is the
drawable-region feedback method (`0x10040c`). Native log signatures name
this class and transformation explicitly.

`conformance/fountain_prediction_length.py` executes the real factory,
controller transformation, feedback and complete incoming OnPredictTouch
calls. Its independent model covers the constructor-default path with
uniform latency disabled (byte 104 is zero):

- State at offsets 8/12/16/20 starts as `(index=0, limit=1, phase=0,
  interval=5)`.
- For N input samples including history and current, select
  `k=min(index,N-1)`, return samples 0 through k, and set `limit=k+2`.
  A shorter input clamps the selected sample, not the stored index.
- Drawable-region feedback advances phase modulo interval. On phase zero,
  increment index only if `index+1 < limit`.
- False feedback leaves this state unchanged for FountainPen. The reset
  branch specifically compares the name with InkPen2; it is not a general
  fountain reset.

All **120 input/feedback sequences** match the independent model, including
growing and shrinking prediction batches and interspersed false feedback.
Coordinates, pressure, tilt, timestamps and ordering match the selected
prefix, and the caller's event remains unchanged for the tested identity
transform. Native code removes the input event's transform before processing
and restores it on the caller event and returned event; nonidentity precision
is not checked by this suite.

The controller is also bound into the complete native presenter, with **11
successive incoming frames** executing its real transformation and drawing
the fountain tip. Another **nine complete calls** check acceptance boundaries:

- If the presenter's stored down time (offset 320) exceeds the incoming
  event's nonnegative down time, the event is rejected before transformation.
- Tool 2 enters the length controller. The normal subsequent gate uses the
  returned event, including its shortened endpoint timestamp.
- A true value from latency-configuration virtual slot 144 directly selects
  the accepted-prediction branch for the tested stylus events.
- Otherwise `TouchPresenter::CheckThreshold` (`0x103c30`) requires a nonzero
  provider count and error strictly less than the threshold, followed by an
  enabled byte at presenter offset 393 and returned event time greater than
  or equal to the previous prediction time at offset 304. Equality of error
  and threshold rejects; equality of timestamps accepts.

The timestamp boundary fixture supplies prediction samples at relative times
20, 30 and 40. The fresh length controller returns time 20. Previous time 20
accepts and previous time 21 falls back, although the original input extended
to time 40. A previous time newer than the last real time also reaches the
presenter's existing early-return path after fallback in these fixtures.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_prediction_length.py
```

Device policy is supplied through explicit configuration/provider interfaces;
these tests do not establish which flag values a physical device chooses.
String comparison supplies FountainPen metadata while checking the actual
native LaserPen/InkPen2 query constants. Uniform-latency-enabled resampling,
runtime interval configuration, prediction generation and other tools remain
outside this model. The complete presenter calls have no pending main-event
batch: their tip draws consume the previously populated PointTipManager.
Connecting the shortened prediction to that manager through the pending
TouchStrokeDrawing batch still requires integration evidence. These results
must not be used to apply prediction-prefix trimming to saved stroke samples.

## V16 committed-state snapshot and temporary tip replay

V16's `movePen` saves its current state into shared FountainPenData at
`0x771d4..0x77290`, after processing the current sample. The temporary tip's
`startPenOrig` loads that snapshot at `0xa4444..0xa4500`. These are separate
drawable objects: the tip does not rewind or overwrite the main drawable.

The exact byte mappings are below. Offsets are decimal, relative to each
object; these describe storage, without assigning unverified semantic names
to individual fields.

| Main drawable | Shared data | Bytes | Tip drawable |
| --- | --- | --- | --- |
| 88 | 104 | 2 | 41 |
| 92 | 108 | 4 | not restored |
| 96 | 112 | 16 | 44 |
| 112 | 128 | 8 | 60 |
| 120 | 136 | 8 | 68 |
| 128 | 144 | 8 | 76 |
| 272 | 152 | 1 | 224 |
| 276 | 156 | 4 | 228 |
| 280 | 160 | 8 | 232 |
| 288 | 168 | 4 | 240 |
| 292 | 172 | 36 | 244 |
| 328 | 208 | 8 | 280 |
| 336 | 216 | 4 | 288 |
| 340 | 220 | 4 | 292 |
| 344 | 224 | 4 | 296 |
| 348 | 228 | 4 | 300 |

Saving also sets shared byte 232 to 1. Restoring copies 44 bytes from the
main PenTolerance object (shared pointer at 32) to the separate tip tolerance
(shared pointer at 40). The tolerance copy matters: replay starts with the
main stroke's current admission state, rather than an empty filter.

`conformance/fountain_tip_state.py` executes both native instruction blocks
against 128 deterministic arbitrary-byte patterns. It verifies all 512 bytes
of each destination, including untouched gaps, and restores each snapshot
three times after replacing temporary state. All 128 saves and 384 restores
match the independent mapping exactly; main/shared source state stays intact.
This is a block-level state-transfer test, not a full Draw or geometry test.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_tip_state.py
```

Static control flow also establishes how width calculation reuses tip code.
`CalculateSmoothedWidths` (`0xa4be0`) returns immediately for zero tip length.
Otherwise it sets WidthSmoothManager mode 1, clears tip offset 312, calls
`startPenOrig` then `movePenOrig` if given a real-tip event, optionally calls
`endPenOrig` for the end event, and finishes in mode 2. These calls pass the
rendering flag as false. Actual tip `Draw` obtains the real-plus-predicted
event, checks for action 2, and calls the same start/move methods with the
flag true (`0xa40e4..0xa4118`). Thus each start reloads the committed snapshot,
including after a width-only pass. Width-history mutations and the complete
rendered result require the integrated executable test below; the snapshot
copy alone does not establish their correctness.

## Executable V16 main/tip Draw integration

`conformance/fountain_draw_native.py` now executes the actual V16 and tip
constructors, `SetEventsForDraw`, main `Draw`, and tip `Draw` for all 1,845
events in the 50 queue sequences. This includes the native width-only tip
pass, final width completion on up, saved-state restoration, real MotionEvent
history, PenTolerance, SmPath, drawPoint, and both renderer AddPoint methods.
It captures native instance attributes after SetTotalLength and before the
return callback releases the buffer. These are instance records, not pixels.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_draw_native.py
```

The test also runs every sequence without drawing the temporary tip. All
committed stamp and attribute-buffer hashes must match between the two runs.
Each tip Draw must leave the main drawable and its tolerance unchanged.
There are 20,249 committed stamps and 108,629 temporary-tip stamps in the
captured run. Every emitted attribute record is additionally checked against
the independent float32 stamp-attribute reconstruction used for RTV5, with
the tip's extra distance and RGB channels. The checked reference records
input hashes, return/error codes, stamp counts/hashes and buffer sizes/hashes.
Update it explicitly with `--write-reference` after reviewing a change.

This harness supplies an identity canvas matrix, pen size 8, opaque color
`0xff112233`, variable width, no rainbow mode, tool 2, and source 0. It
assembles the shared configuration and secondary tip-interface vtable; the
vtable points to the actual native adjustment thunk. A host queue accepts
rendering messages without executing them. Logging, memory/allocation
primitives and Error::SetError's Android TLS diagnostic storage use host
callbacks. The latter captures code 7 for 460 empty-tip attempts: the harness
calls tip Draw after every move, including moves with no retained tip.
These false returns are part of the reference, not successful draws.

The loader's demangled destructor lookup cannot distinguish the deleting and
non-deleting PenReturnCallback destructors. This harness explicitly selects
the non-deleting native entry at PenCommon `0x50424`, since Draw constructs
the callback on the stack. No drawable geometry entry is replaced.

This establishes executable live geometry and renderer-attribute evidence
for the stated configuration. It does not execute the asynchronous render
messages, EGL/GL, fragment shaders, canvas clearing/compositing, or an actual
Android input/prediction source. An independent model of the entire live
geometry state machine and those additional configurations remain unfinished.

### Tip distance attribute is not evidence of a visible fade

Tip AddPoint (`0xa6a8c`) writes an eleven-float record: position (2), extent
(2), input alpha, distance attribute, inner radius, subpixel alpha, and RGB
(3). The native subpixel/AA calculation matches RTV5. SetTotalLength
(`0xa7398`) can transform the distance attribute to
`1 - (1 - endpoint_alpha) * distance / total_length`, with float32 arithmetic
and a fused final operation. Its default endpoint alpha is float32 0.9.
Every SetTotalLength call in these ordinary variable-width sequences receives
zero. The separate attribute suite now exercises positive lengths directly;
see [live-tip raster checks](fountain-rasterization.md#live-tip-attributes-and-shader-execution).

The extracted vertex shader at `0x432de` passes the distance attribute to
the fragment stage, but the fragment shader at `0x43626` never uses it.
Its output uses circle coverage, subpixel alpha, input alpha, per-stamp RGB
and uniform color alpha. Therefore neither the field name nor its setter
justifies adding distance fading to the reconstructed visible tip. The GPU
suite now verifies this with 2,516 byte-identical renders while changing the
distance attribute. The intermediate shader stages, blend selection and
copy-quad construction now also have checks in the linked raster findings.
Alternate configurations and the full native surface lifecycle remain
separate checks.

### TouchStrokeDrawing dispatch and stored sample boundary

`conformance/fountain_touch_drawing.py` loads the hash-pinned Drawing library
and executes `TouchStrokeDrawing::OnTouch` (`0xb71a4`) for an already-started
V16 stroke. Its normal tip-capable branch calls drawable `SetEventsForDraw`
with the real event and a separate prediction, then `Draw(RectF*)`. The
V16 smoother getter returns null; this path does not apply the shared
stroke smoother.

Ninety move updates, with 1, 3 or 8 samples and predictions enabled/disabled,
produce byte-identical main and tip meshes and identical stamps compared
with direct V16 ingress/draw replay. The real Drawing code also executes
`unionUpdatedRectIntoStrokeRect` and `addEventPointsToObjectStroke`
(`0xb7dc0`). At the observed `ObjectStroke::AddPoint` boundary, all 360 real
samples preserve float32 positions, pressure, tilt, zero orientation and
integer milliseconds relative to the event's down time. Predicted samples
are not forwarded to that persistence method. This is evidence about stroke
sample storage, not a timestamp per undo operation or file serialization.

The fixture supplies the existing stroke, pen interfaces and identity canvas;
locking is a host no-op, and `ObjectStroke::AddPoint` records arguments instead
of mutating a native model object. Down/up/cancel lifecycle, the presenter's
pending-event list, Android scheduling and final framebuffer pixels remain
outside this comparison. Run from the repository root:

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_touch_drawing.py
```

### Pending real events and shortened prediction integration

`conformance/fountain_pending_events.py` now connects the native prediction
controller and complete `OnPredictTouch` call to Drawing's `OnTouch` and the
V16 renderer. The fixture supplies a native-layout pending event list;
Engine's `DrawStroke` (`0x102738`) traverses it and calls the real drawing
method for each move. Accepted predictions are shortened by the actual
controller before this traversal, and the same shortened event is passed
to each pending move. The rejection branch supplies null predictions.
The native cleanup helper (`0x102a38`) destroys events and empties the list.

Sixty-four complete presenter frames cover one/three queued moves, two real
samples per move, accepted/rejected predictions and page clipping on/off.
Both the main and tip meshes and stamps match direct native replay with an
independently selected prediction prefix. The controller grows that prefix
from one to two samples across these frames. Accepted and rejected runs
produce different geometry, so the prediction comparison is not vacuous.
The persistence boundary records exactly the real samples, and the pending
list returns to its empty sentinel state after every frame. Canvas traces
also establish main save/draw/restore before bitmap copy and tip drawing.

This closes the earlier gap where incoming presenter tests populated the
point manager separately and left the pending list empty. The list producer,
stroke creation/end/cancel lifecycle, uniform-latency policy, asynchronous
renderer execution and final framebuffer comparison remain separate work.
Canvas calls and model persistence use the same explicit host boundaries as
the preceding fixtures; these checks do not establish Android scheduling.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_pending_events.py
```

### Ending and cancelling an existing stroke

`conformance/fountain_stroke_end.py` executes the pending-batch helper with
an up or cancel event after 1, 5 or 15 moves, with earlier predictions on/off
and identity/nonidentity presenter matrices (24 cases). The real Drawing
and FountainPen routines execute; the same existing-stroke/model/canvas
boundaries apply as above.

For up, V16 ingress and Draw flush the remaining points. Captured main meshes
and stamps match direct native up replay. Drawing forwards the final real
sample to `ObjectStroke::AddPoint`, preserving its channels and relative
timestamp. For cancel, `TouchStrokeDrawing::cancelStroke` (`0xb75b4`) appends
no sample, submits `SetCanvasCleared` through the configured normal drawable
when the dim drawable is absent, and clears the drawing object's drawn flag
at offset 72. The submitted task targets the main RT, virtual offset 80,
with argument 1. This test observes submission, not execution. It also checks
that cancellation leaves the main drawable object and accumulated stroke
bounds unchanged at this boundary; cancellation is not evidence of deleting
the model stroke or resetting every renderer history field.

`DrawStroke` branches on the last queued event: up/cancel return
`GetStrokeRect()` (`Drawing 0xb8170`), the whole accumulated stroke bounds,
whereas the move branch projects the current batch's union through the
presenter matrix. The returned up/cancel bounds and presenter field 188
match the drawer's bounds even with a nonidentity presenter matrix. The
fixture's canvas still supplies identity geometry transforms, so this is a
check of branch selection and bounds propagation, not transformed pixels.
Stroke creation/ownership and complete final presenter-frame behavior remain
unverified by this test.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_stroke_end.py
```

### Real-event terminal presentation and prediction shutdown

The final real-event handler is distinct from `OnPredictTouch`. Engine
`0x1020a8` sets presenter byte 344 for up/cancel and stores the event's
relative time at offset 312. Its terminal branch inverse-projects the old
tip bounds (field 236), rounds them outward, and unions them with the current
real-event dirty bounds (field 188). It presents the real-event bitmap using
the actual up/cancel action, then calls the native prediction-resource
cleanup helper (`0x1030c0`). It does not run another ordinary predicted-tip
frame for this terminal branch.

`conformance/fountain_final_frame.py` executes that branch from `0x1020a8`
to its common successor `0x1026c4`, supplying the real caller's register and
stack locals after upstream drawing/setup. Eighteen cases cover up/cancel,
empty/integer/fractional old tip bounds, and identity/scaled/translated
matrices. An independent inverse-transform/round-out/union calculation
matches final presentation bounds. The trace verifies presentation before
prediction bitmap detachment and bitmap/canvas release, followed by the
second null-queue detachment. Native instructions clear fields 88, 104 and
112 and set the prediction-disabled flag. A subsequent complete
`OnPredictTouch` returns false without presentation or state changes.

This corrects a tempting but invalid end-to-end fixture: manually queueing an
up event and calling `OnPredictTouch` does not reproduce the real-event
handler's terminal path. Such a fixture can instead take the old-prediction
timestamp guard and retain dirty regions without final presentation.

The terminal fixture now executes native
`DrawingUtil::SetPredictionPenBitmap` and `PenStrokeTipDrawableGL::SetCanvas`.
Only `SPGraphicsFactory::ReleaseBitmap/ReleaseCanvas` remain observed host
boundaries at that level. Upstream real-event setup, the complete enclosing
real-event handler, and Android resource lifetime are not yet established
by this bounded branch test.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_final_frame.py
```

### Native prediction-canvas detachment and deferred unref

The terminal fixture also executes Drawing `SetPredictionPenBitmap`
(`0x74d6c`) for both cleanup calls. With a null bitmap it obtains the pen's
tip drawable, calls `SetCanvas(nullptr)`, calls `SetCanvasCleared`, and
passes null to `ReleaseCanvas`. Native tip `SetCanvas` (`PenCommon 0x525f4`)
queues a Member1 task containing the old canvas before clearing the tip's
canvas pointer. The second detach sees an already-null canvas and does not
queue another unref, but it still submits another clear notification.

All 18 terminal cases now verify exactly one old-canvas unref task and two
tip clear tasks. The harness resolves the native unref member-function GOT
relocation at PenCommon `0x7a520`; it does not substitute a host detach
implementation. After verifying submission has not unrefed the canvas, it
executes the captured Member1 body (`0x52a44`) and actual `UnrefCanvas`
(`0x52894`), observing exactly one call to the old canvas's virtual slot 208.
That last virtual method is a host observation boundary. This establishes
deferred dispatch and its captured target, not scheduler ordering, the
canvas reference count implementation, or safe Android object destruction.

### Prediction canvas attachment and reuse

`conformance/fountain_prediction_canvas.py` executes the matching non-null
`DrawingUtil::SetPredictionPenBitmap` path. Its graphics factory boundary
supplies a canvas; native DrawingUtil requests it with bitmap type 1, calls
the native tip `SetCanvas`, submits the tip clear notification, and releases
the temporary factory canvas reference. On a new canvas, native SetCanvas
calls canvas slot 200 (ref), reads bitmap width/height, sets the real
`PenGLDataManager` queue at offset 56, and submits a Member3 task targeting
RT virtual offset 72 with `(width, height, canvasQueue)`.

Four attach/reuse/detach sequences cover 64x128, 1080x1920, 0x0 and 0x64
canvases. The native dimension guard is `(width | height) != 0`: 0x64 still
submits initialization, whereas 0x0 retains the canvas pointer/reference but
does not assign the data-manager queue or enqueue initialization. With the
manager queue still null, the subsequent clear notification has no queue
on which to submit its task. Reattaching the same canvas avoids an extra ref
and initialization; DrawingUtil still invokes the clear notification.

The suite executes the captured initialization task body (`PenCommon
0x4a274`) and dispatches its exact dimensions and queue through RT slot 72
to native `CreatePenCanvas` (`FountainPen 0xa68ec`). The resulting manager
acquisition and private-surface clear are observed at host interfaces. Detachment uses the
old canvas's own queue to submit unref, including the 0x0 case where the
data manager never adopted a queue. Its captured native unref task is then
executed and reaches the expected old canvas.

Factory allocation, actual reference counting, shader initialization and
Android worker scheduling remain observation boundaries. These checks establish native orchestration across those
boundaries rather than a full renderer/resource-lifetime simulation.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_prediction_canvas.py
```

The attachment fixture now connects those previously separate task and
render-thread setup checks. The actual data manager provides the queue;
native CreatePenCanvas requests a separate private surface with the supplied
dimensions, flags `(0, 0, 6, 2)`, clears it with argument 0, and records its
pointer and `(width, height, tileCount)` in RT fields 160 and 240/244/248.
Tests assert that the private surface differs from the attached destination
canvas. Executing the subsequently captured clear task (`0x63804`) reaches
native `ClearPenCanvas` and clears that private surface. Executing the
initialization task again with the RT's initialized flag set still acquires
and clears the surface but skips the shader-initialization callback.

The 64x128 and 0x64 cases use one tile; 1080x1920 uses two. The 0x0 case
continues to produce no initialization task. Metadata names remain host
placeholders at the manager-acquisition boundary, so this integration does
not test name-cache lookup or texture allocation. Shader initialization is
also observed rather than executed here. Task bodies execute explicitly in
the harness, without claiming native worker scheduling or final GL pixels.

### Native tip resource initialization with supplied shader-cache hits

`conformance/fountain_tip_init.py` replaces the previous initialization
callback with the complete native `FountainPenStrokeTipDrawableRT::Init`
(`0xa5dac`), reached through the captured canvas-initialization task. GPU
factories and shader-cache lookup are explicit host boundaries; descriptor
construction and the Init control flow execute natively.

The test verifies two geometry allocations: primitive 5 with two vertex
streams, and primitive 4 with one stream. Their native descriptors have
component counts `(4)`, `(4,2,2,3)` and `(2)`, strides 16, 44 and 8 bytes,
and per-attribute offsets `(0)`, `(0,16,24,32)` and `(0)`. The renderer's
attribute-byte-size GOT relocation is resolved to its actual table; leaving
that pointer unresolved can silently produce invalid offsets in emulation.
The initial uploads contain four static vertices and six copy-quad vertices.
The two native blend descriptors are source-over `(ADD, ONE,
ONE_MINUS_SRC_ALPHA)` and maximum coverage `(MAX, ONE, ONE)` for both RGB
and alpha, consistent with the separately tested GLES mapping.

Init requests the cached alpha, composite and direct-tip shaders in that
order, increments each supplied cache node's reference count once, stores
the handles at RT offsets 200/208/216, registers the managed object and sets
ready byte 144. Three repeated Init calls preserve all seven resource
handles without allocating more geometry/blend objects, uploading vertices
again, or incrementing shader references. Registration itself remains a
host observation boundary. This fixture supplies cache hits and key objects;
cache misses, shader construction/compilation, native cache synchronization
and GPU allocation remain unverified by this test.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_tip_init.py
```

### Shader construction on supplied cache misses

`conformance/fountain_shader_creation.py` extends Init to all eight hit/miss
combinations for its three shaders. Cache lookup/insertion remain supplied
interfaces, but cache-miss branches, shader allocations, native constructors,
binding type checks, node reference increments and resource assignments
execute from the APK. The six shader-source GOT entries are resolved to the
hash-pinned library's actual strings, and program factory calls must request
these exact vertex/fragment address pairs:

| Shader | Sources | Requested uniforms |
| --- | --- | --- |
| TipAlpha | `0x42c2a`, `0x42ea9` | `ProjectionMatrix` |
| TipComposite | `0x430fd`, `0x431b3` | `uSrcTexture`, `uInputColor` |
| Tip | `0x432de`, `0x43626` | `ProjectionMatrix`, `inputColor` |

The supplied parameter bindings use the concrete native-expected types
2 (matrix), 8 (texture) and 5 (color), rather than the permissive unknown-type
fallback. Tests verify program debug names, exact uniform spelling, shader
program-field assignments, one reference per acquired shader, and no new
program or binding requests on a second Init. Only missing cache entries
construct programs; mixed cache states preserve the same alpha/composite/
direct-tip acquisition order.

The program factory remains a compilation boundary: this does not execute
the native GLES compiler or native shader-cache tree allocation/locking.
The separate GPU suite executes extracted source; this test connects those
source identities to native construction, without proving driver parity.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_shader_creation.py
```

### Complete tip render-thread Draw commands

`conformance/fountain_tip_render.py` connects native Init/cache-miss shader
construction to the complete tip RT `Draw` method (`0xa74c0`). Sixteen cases
cover all enhanced/redraw/rainbow flag combinations with identity and
scaled/translated input matrices. The destination/private subbitmap matrix
is identity; the supplied input matrix reaches the projection binding intact.

The direct path activates source-over when enhanced is false, otherwise MAX,
binds the destination and direct-tip shader, uploads `ProjectionMatrix` and
`inputColor`, submits four vertices per instance, and resolves the destination.
With enhanced enabled and redraw or rainbow set, Draw instead binds the
private surface, activates MAX and the alpha shader, uploads its projection,
and submits the instances there. It resolves that surface, switches to
source-over and the destination/composite shader, binds private texture unit
0, uploads `uSrcTexture=0` and `uInputColor`, uploads the six-vertex full copy
quad, submits it, and resolves the destination. Exact observed command order
and uniform values match these expectations. Null destination and empty
vertex-vector cases submit no commands at all.

All native control flow, matrix composition, uniform wrapper dispatch and
copy-quad construction execute. Graphics factories, program/binding methods,
subbitmap operations and depth-state disabling remain host interfaces.
A nonempty post-update vertex vector and one instance are supplied explicitly;
this does not yet connect live geometry upload to framebuffer pixels. Dirty
copy rectangles and nonidentity subbitmap projection remain covered only by
separate helper tests, not this complete-call fixture.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_tip_render.py
```

### Live geometry through queued buffer handoff and RT upload

`conformance/fountain_live_render.py` removes the supplied instance vector
from the complete Draw fixture. It generates real down/move/up events with
varying pressure and move predictions, executes native V16 ingress and
main/tip geometry, captures the tip callback's actual queued Member2 task,
and executes its body (`PenCommon 0x506b4`). The member-function GOT entry at
`0x7a438` is resolved to native `PenDrawableRT::SendDataToGPU` (`0x4a5c4`),
which installs the captured vector at RT offset 88. Native tip Update
(`0xa746c`) derives the instance count from the vector length divided by
44 bytes and uploads stream 1.

Eighteen live frames produce 837 tip stamps. Every upload is byte-identical
to the native geometry buffer already checked against the independent
attribute model. Complete direct, MAX and intermediate Draw calls use those
computed counts, with four vertices per instance. Empty frames upload and
draw nothing. Native Clear runs between frames; after it clears the vector
pointer, a further Draw must submit no commands even though the old numeric
instance-count field may remain.

The harness explicitly sequences captured buffer handoff, Update, Draw and
Clear; it does not execute the complete draw-task scheduler or buffer-delete
tasks. Renderer properties such as color/mode and target interfaces are
supplied by the fixture. The host upload/draw interfaces observe real bytes
and arguments but do not rasterize them. This closes the supplied-vector gap
in the previous complete-call test, while leaving asynchronous ordering and
final framebuffer parity open.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_live_render.py
```

### Captured native render-message execution

`conformance/fountain_render_task.py` executes the actual `PenGLRenderMsg`
body (`PenCommon 0x4676c`) captured from the live tip callback, immediately
after its captured buffer-handoff task. The callback's render-message vtable
GOT entry is resolved to `0x774b8`; the fixture identifies the emitted task
by that vtable and its real tip RT target rather than constructing a task.

Seventy-two frames cover down/move/up sequences in direct, MAX and
intermediate modes with one or two bitmap tiles. Native message code calls
RT Update once, computes the transformed dirty bounds, enables clipping for
each tile, performs rectangle intersection tests, invokes complete native
Draw only for the intersecting tile, restores clipping for every visited
tile, and finally calls native Clear. The second tile is deliberately outside
the stroke bounds. Tests compare upload bytes with the generated geometry,
verify the instance count, require Update first and Clear last, check balanced
per-tile clipping, and verify that Clear removes the vector pointer and
resets the redraw flag. No host Update/Draw/Clear orchestration is used in
this fixture.

This advances beyond the earlier explicit RT sequence. The test still
executes captured messages synchronously, supplies canvas/bitmap/GPU
interfaces and renderer mode/color properties, and does not execute deferred
vector-deletion messages or an Android worker queue. Depth/background-map
settable interfaces are absent in this fountain configuration. Actual
thread scheduling, tile pixel coverage and final framebuffer parity remain
outside its evidence.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_render_task.py
```

### Deferred vector deletion after captured render messages

`conformance/fountain_buffer_lifetime.py` captures the tip callback's emitted
buffer-handoff, render and vector-delete tasks, resolving their native
vtable relocations. Seventeen task chains from an 18-event down/move/up
sequence have exactly that submission order. The deletion task's vector
pointer matches the handoff task's primary buffer; the outline buffer is
null in this configuration.

The test executes each captured body in submission order. The actual delete
body (`PenCommon 0x45e6c`) invokes `operator delete` first on allocated vector
storage, then on the vector object. At this point the native render task has
already performed the byte-checked upload and draw and cleared RT field 88.
The allocator boundary records each deletion exactly once and poisons the
corresponding storage capacity/object ranges. A Unicorn native-memory-read
hook rejects any subsequent access to those ranges. Later frames and an
extra post-delete Draw produce no such accesses; the extra Draw submits no
commands. This covers actual deferred vector-delete code rather than merely
observing its submission.

The fixture assumes FIFO execution of the observed task chain and does not
establish the Android queue's synchronization or rejection policy. Other
queued property operations and main-stroke vector deletions are not executed
by this fixture. Allocations remain monotonic and host deletion does not
reuse addresses, so the check is evidence for ownership/order under this
execution model, not a general native allocator or concurrency proof.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_buffer_lifetime.py
```

### Captured tip render-property updates

`conformance/fountain_render_properties.py` extends the buffer-lifetime
fixture to execute every task emitted by tip Draw in these configurations.
Sixty chains, from twelve six-event down/move/up sequences, emit exactly:
SetPenData, SetEnhancedAntiAlias, SetRainbowMode, SetRect, buffer handoff,
render, and vector deletion. Unknown task vtables fail the test. Native
task bodies and setters execute in submission order; GPU interfaces and
FIFO scheduling retain the previously described host boundaries.

Tip `startPenOrig` (`0xa4238`) copies inverse scale directly to RT offset
224 at `0xa4518`, before geometry generation. It queues SetPenData from
config width/color, SetEnhancedAntiAlias from config byte 52, and
SetRainbowMode from shared-data byte 240. Draw subsequently queues SetRect
from its extended dirty bounds. The task factories copy their arguments;
the tasks do not retain pointers to the source config or stack rectangle.

Resolved FountainPen GOT entries are `0xd5f00/08/10` for RectF, boolean,
and float/int task vtables, `0xd6218` for the tip boolean task vtable,
`0xd5f28/38/58` for the common setters, and `0xd6220` for the tip rainbow
setter. Native task bodies are `0x6690c`, `0x66938`, `0x66960`, and
`0xa5c90`. PenCommon SetPenData (`0x4a558`) converts packed ARGB to
float32 RGBA channels divided by 255 and converts the float width to an
unsigned integer at RT offset 60. The fractional-width fixture, 8.75,
therefore produces 8 in that field; this does not imply geometric radius
is integer-quantized.

Three colors cover alpha 0, 128, and 255, crossed with antialiasing and
rainbow flags. Assertions check native setters replace poisoned preset
properties, inverse-scale transfer, rectangle task payload/storage, exact
mesh uploads, and the resulting direct/composite shader color uniforms.
Buffer deletion retains the poison/read-guard checks. Rainbow fixtures
supply an explicit 600-unit distance period and red/green/blue palette in
shared data; these are test inputs, not established application defaults.
Native rainbow color code executes, with `fmodf` supplied at the C runtime
boundary. Palette interpolation itself is not independently modeled by
this fixture; [the separate rainbow conformance model](fountain-rainbow.md)
checks it against native V17, tip and preview functions.

This removes preset renderer color/mode/bounds from the connected task
test. It does not establish application configuration defaults, Android
thread ordering, GPU framebuffer parity, or historical-version parity.

```sh
PYTHONPATH=scratch/apk-analysis-runtime/python python3 conformance/fountain_render_properties.py
```
