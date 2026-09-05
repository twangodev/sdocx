# Marker2 curve sampling and stroke completion

## Evidence and scope

Confirmed by static inspection of Samsung Notes 4.4.45.37 ARM64
`libSPenBase.so`, `libSPenMarker2.so` and `libSPenPenCommon.so` from the APK
identified in the [knowledge base](README.md#sources-and-validation). This continues the
[Marker2 V1/V2 rendering trace](marker2-rendering-findings.md#shared-point-generation)
through path measurement, stored-point replay and the ordinary end operation.
Addresses below belong to Base unless explicitly identified as Marker2.

The ordinary Marker2 path samples a midpoint-smoothed quadratic using
approximate distances. Its redraw and end routines do not append a separate
stamp at the final input coordinate. These findings establish the operations
inside these drawables; they do not establish which synthetic points an
upstream recorder may have added or measured equivalence with Samsung PDFs.

## SmPath owns the distance approximation

Marker2 V2 calls `SmPath::resetPath(false)` at `0x22d14`, `getLength` at
`0x22d1c` and `getPosTan` at `0x22d58`. The imported functions resolve to
Base's `SmPath`, whose traced measurement routines use their own segment
and point buffers. They do not forward these operations to `SkPathMeasure`.

`resetPath`, `0xbd34c`, invalidates the cached length and resets the measured
buffers. `getLength`, `0xbd370`, calls `helper_buildSegments` when the length
is negative. The builder at `0xbc594` dispatches a quadratic to
`helper_compute_quad_segs` at `0xbc6a0`, starting its integer parameter
interval at 0 and `0x7fff`.

For a quadratic with start `P0`, control `P1` and end `P2`, the recursive
helper, `0xbcde0`, measures this componentwise deviation:

```text
deviation = abs(0.5 * P1 - 0.25 * (P0 + P2))
flatness = max(deviation.x, deviation.y)
```

This is the difference between the quadratic's midpoint and the chord's
midpoint. Instructions at `0xbce28`–`0xbce60` evaluate it with single-precision
vector arithmetic and compare it to 0.5. The helper subdivides only when
both conditions hold:

- The integer parameter interval is at least `0x400` wide (`0xbce1c`).
- Flatness exceeds 0.5 (`0xbce5c`–`0xbce60`).

Subdivision uses de Casteljau midpoints at one half. The integer midpoint
is `(start + end) >> 1` at `0xbce80`–`0xbce88`; the left recursion runs
before the right at `0xbceb4` and `0xbcecc`. The integer bound limits
subdivision even when the geometric deviation remains large.

Each leaf contributes the Euclidean distance between its endpoints through
the helper at `0xbcd90`, called from `0xbcee0`. That helper normally computes
a single-precision square root; if its squared-distance check is nonfinite,
it retries the squared sum in double precision. It does not integrate the
quadratic's analytic arclength.

The accumulated distance is single precision. A leaf gets a segment record
only when adding its chord increases that accumulated value
(`0xbcee4`–`0xbceec`). The eight-byte record contains:

| Part | Meaning | Evidence |
| --- | --- | --- |
| First four bytes | Cumulative distance | `0xbcfa0` |
| Packed bits 0–14 | Original curve's point index | `0xbcf94` |
| Packed bits 15–29 | Leaf's ending integer parameter | `0xbcf9c` |
| Packed bits 30–31 | Segment type; quadratic is 1 | `0xbcfa4` |

The builder retains the original quadratic's control and endpoint in its
point buffer at `0xbc6c0` and `0xbc820`. Subdivision creates measurement
records; position evaluation still uses the original quadratic.

## Distance lookup interpolates the curve parameter

`getPosTan`, `0xbd3a0`, builds the measurement if needed. It returns false
for a zero-length path or an empty segment buffer at `0xbd3f0`–`0xbd400`.
For finite input, it clamps the requested distance into the measured range
at `0xbd404`–`0xbd41c`.

`helper_distanceToSegment`, `0xbc494`, locates the first cumulative segment
distance at or beyond the request. It reads the preceding cumulative
distance when present. If the preceding segment belongs to the same original
curve, its ending parameter supplies the interpolation start; otherwise the
starting parameter is zero (`0xbc524`–`0xbc55c`).

The integer parameters are scaled by the float at `0x41730`. Its bits are
`0x380000fd`, giving `3.0518498533638194e-5`. This equals the rounded float
literal `0.0000305185`; it differs from the rounded result of `1 / 32767`.
Consequently, scaling integer `32767` produces approximately
`0.9999996423721313`, not exactly 1.

The interpolation at `0xbc560`–`0xbc58c` is:

```text
t = start_t
  + (requested_distance - start_distance)
  * (end_t - start_t)
  / (end_distance - start_distance)
```

For a quadratic, `getPosTan` evaluates two linear interpolations followed
by a third at `0xbd48c`–`0xbd4ac`, equivalent algebraically to
`B(t) = (1 - t)^2 * P0 + 2 * (1 - t) * t * P1 + t^2 * P2`.
Its instructions use fused vector multiply-add operations. The sampled
position is therefore on the quadratic at an approximately distance-derived
parameter, rather than a linear interpolation between leaf endpoints.

The Marker2 loop advances its requested distance by spacing and carries
the remainder into the next accepted quadratic. Skipped input does not
update the previous input point or midpoint. An accepted quadratic can
update those points and the remainder without emitting a stamp if it is
shorter than the pending sampling distance. The return value reports stamp
emission, not merely acceptance of the incoming coordinate.

## Stored-point replay adds no terminal extrapolation

Marker2 V2's `RedrawPen(ObjectStroke const*, RectF*)`, `0x22e14`, reads the
stored tool type, point count and coordinate/pressure/time arrays. It passes
the count unchanged to the array-based `MotionEvent` constructor at
`0x22ec0`, then invokes its MotionEvent redraw slot at `0x22ed8`.

Base's array-based constructor, `0xbfd84`, separates the final input from
the preceding history. For valid arrays with at least two points:

| Event channel | Source array indices | Evidence |
| --- | --- | --- |
| Current X/Y | Last point, `count - 1` | `0xbfee8`, `0xbfef4`, `0xbff28` |
| Historical X/Y | 0 through `count - 2`, in order | `0xc0018`–`0xc0074`, `0xc0098`–`0xc00ec` |

`AddBatch`, `0xc01b4`, copies each supplied point into a historical record
and appends it to the history vector at members 80/88. It does not replace
the current coordinate. The getters confirm this separation:
`GetHistoricalPos`, `0xc0858`, reads the history vector; `GetX`, `0xc0ad8`,
and `GetY`, `0xc0b34`, read the current vector at member 48.

Marker2 V2's MotionEvent redraw, `0x22808`, then:

1. Emits historical point 0 and initializes its smoothing state from it
   (`0x228ec`–`0x2293c`).
2. Sends historical points starting at index 1 through `drawLine`
   (`0x229d8`–`0x22a6c`).
3. Sends the current point through `drawLine` once (`0x22aa0`).
4. Updates the affected rectangle and finishes the render callback.

The tail has no additional `AddPoint`, quadratic or line to the current
coordinate. Its queued member at GOT `0x303e8` resolves to
`PenDrawableRT::SetRect(RectF)`; the call at `0x22b30` supplies that rectangle.
PenCommon's setter at `0x4a4ac` only stores its four rectangle components.

PenCommon's `PenReturnCallback` destructor, `0x50424`, queues
`SendDataToGPU` through GOT `0x7a438` before constructing a render message
with the drawable and canvas. `SendDataToGPU`, `0x4a5c4`, assigns the two
buffer pointers. The [ordinary render-thread path](marker2-rendering-findings.md#size-clamping-and-stamp-geometry)
uploads and draws the supplied point instances.

The constructor itself adds no duplicated or extrapolated terminal point.
This does not resolve whether the stored array already contains synthetic
points. A single-point array also needs separate producer investigation:
this constructor creates no history for it, while the normal Marker2 redraw
starts from historical index 0 without a count check. That is not sufficient
evidence that a normal saved Samsung dot reaches this path with one point.

## The ordinary end operation keeps the same smoothing rule

Marker2 V2 `Draw`, `0x2201c`, dispatches action 1 to `endPen` at `0x2213c`.
The end routine, `0x22390`, processes all historical inputs through
`drawLine` at `0x22490` and the current coordinate at `0x22520`, using the
same tool/source-dependent sampling flag as movement.

It does not force the current coordinate past the distance/alternation
filters and does not append a terminal stamp afterward. For an accepted
last input, the generated quadratic ends at the midpoint of the preceding
accepted input and that last input, as established by `drawLine` at
`0x22cf4`–`0x22d08`. The last emitted stamp can precede even that midpoint
because spacing and parameter approximation still apply.

The end routine enlarges its affected rectangle using approximately
`width / 2 * 1.3 + 4` at `0x22574`–`0x22594`. The exact stored double at
Marker2 `0x11c20` is `1.2999999523162842`. This modifies the redraw bounds;
it does not extend the point geometry. After `endPen`, `Draw` unions the
rectangle, queues `SetRect` and completes the render callback.

V1's `endPen`, `0x20ff0`, and V2's `0x22390` have the same 144 instructions
after normalizing their local branch addresses and version-specific call
names. Their shared `drawLine` behavior was established in the earlier
V1/V2 comparison.

## A small synthetic consequence

Consider a normal redraw with just `(0, 0)` and `(4, 0)`, default spacing 1,
and the alternating skip flag disabled. The initial stamp is `(0, 0)`.
The final input produces a quadratic with X coordinates `[0, 0, 2]`.
Its flatness is exactly 0.5, so the measurement uses one chord of length 2.

| Requested measurement distance | Approximate parameter | Approximate stamp X |
| --- | ---: | ---: |
| Initial stamp | — | 0 |
| 1 | 0.4999998212 | 0.4999996424 |
| 2 | 0.9999996424 | 1.9999985695 |

These values are arithmetic derived from the instructions and decoded
constant, not output from executing the Android library. They show why
even a collinear quadratic need not yield equally spaced coordinates under
this approximation. If the alternating skip flag is enabled, the first
4-unit move is skipped instead, leaving only the initial stamp in this
two-input example. Neither example includes a forced stamp at X = 4.

## SDK implications and validation

A Marker2 renderer needs to preserve accepted-input filtering, midpoint
construction, this distance approximation and spacing carry together.
Replacing them with exact arclength sampling or adding an unconditional
endpoint changes the recovered native point sequence. Mask coverage, size
conversion, projection and final composition remain separate parts of the
visible result.

The APK digest and all three extracted libraries were rechecked. V1/V2 end
instructions were compared, the parameter constant was decoded directly,
and the synthetic example's single-precision arithmetic was checked with
assertions. No SDK code changed and no visual parity claim follows from
these static checks.

[Touch-recording findings](stroke-recording-findings.md) now explain repeated
tap coordinates, Marker2's null replacement provider and replay source reset.
Upstream event preprocessing, single-point imports, StrokeTip and other pen
plugins remain open. New SDOCX/PDF pairs with short straight
strokes, taps, bends and widely spaced final samples can test these findings
against saved data and visible output.
