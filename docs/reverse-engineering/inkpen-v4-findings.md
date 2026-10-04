# InkPen V4 saved-stroke source authority

## Evidence and admission

Samsung Notes 4.4.45.37 APK SHA-256
`daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667`.
These independently reviewed static ARM64 traces establish a selected V4,
ordinary single saved stroke with curve enabled and no clean reusable cache.
They do not establish native execution, geometry or appearance parity.
Addresses are ELF virtual addresses; the InkPen ELF matches its APK entry.

| Source | SHA-256 |
| --- | --- |
| `libSPenInkPen.so` | `939a7c719838306d693f96701994486b03f5c7eea35f16f25c6a89d84daba401` |
| `libSPenDrawing.so` | `788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd` |
| `libSPenPenCommon.so` | `afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d` |
| `libSPenBase.so` | `e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb` |

The [registry](pen-selection-findings.md#built-in-native-registry) resolves
InkPen separately from the [InkPen2 live-input path](inkpen2-input-findings.md).
InkPen's constructor supplies `4;` (`0x244f4–0x24514`), but saved advanced
settings can replace that default. `GetStrokeDrawableGL`, `0x247ec–0x24910`,
selects versions 1–4 and reaches V4 construction at `0x248a4`; its cached
drawable is reused only for the matching selected version.
Drawing configures the actual registered pen from saved settings and size
(`0x81bb0–0x81bf8`). The count-1, StrokeType != 1 branch of `redrawIPen`,
`0x82abc`, calls the GL drawable's secondary-interface slot 40
(`0x82b54–0x82b70`). V4 relocation `0x42850` binds thunk `0x317c0`, which
adjusts `this` by -8 and reaches `RedrawPen(ObjectStroke*,RectF*)`, `0x30dc0`.

## Saved inputs and startup

The complete saved entry obtains point data by original runtime handle, or
runtime handle when the former is -1 (`0x30e88–0x30ed8`). A nonnull cached
point buffer with redrawing-dirty clear bypasses recomputation (`0x30ee8–0x30eec`). That
branch supplies the cached buffer and `PenStrokeData::GetPenRect`, `0x313b8`,
rather than the Model object's raw rectangle. Dirty data is unreferenced first.
Fresh reconstruction installs a callback float buffer (`0x30ef8–0x30f20`).
InkPenData byte 8 selects curve; zero selects a separate no-curve consumer.
The constructor defaults to curve enabled, fixed width disabled, value zero;
setters at `0x24a44`, `0x24a7c`, `0x24ab4` alter those runtime fields.

The curve branch requires positive count and reads saved XY, signed i32 time,
pressure, tilt and orientation (`0x30f48–0x30f78`). It does not repair missing
point/time/pressure arrays. First XY seeds start/control/midpoint/last-tested
points at drawable offsets 80/88/96/104; movement and alternating byte reset,
and the first signed timestamp seeds offset 272 (`0x30f80–0x30fa0`).
Tools 1/3 substitute pressure 0.5; otherwise startup uses raw pressure without
an upper cap. Width is `f32(pressure*size)`, retained at offset 288, and the
initial dot receives half that width (`0x30fac–0x30ffc`). Output bounds are
then reset to first XY (`0x31000–0x31014`), after the initial dot call.
Later samples again substitute 0.5 for tools 1/3, but other saved pressure
is capped only when ordered greater than 1 (`0x31078–0x31084`). There is no
lower-pressure clamp. Interior calls receive direct sign-extended timestamps,
XY and selected pressure (`0x31090`); tilt/orientation are not width operands.

## Movement and width state

The complete `reDrawLine`, `0x30a38–0x30db8`, uses ordered f32 operations.
`fma` below means one fused instruction; equations describe finite inputs.
If both absolute XY differences from last-tested position are below 1.5,
it returns without changing any retained state (`0x30a70–0x30a84`).
Otherwise distance is `sqrt(fma(dx,dx,dy*dy))`. Signed i64 subtraction and
multiplication form `(time-priorTime)*1000` before conversion to f32.
Denominator is 50000 when delta<1 or priorTime==0, otherwise that conversion.
Movement is `f32(f32(f32(distance*10000)/denominator)*scale)`;
scale is 1 for minimum configured screen dimension zero, otherwise
`f32(1440/f32(minDimension))` (`0x30aa0–0x30b10`). PenCommon's
`getSettingData`, `0x464a0`, returns Pen+24; `SetScreenResolution`, `0x460b4`,
writes Pen+56/+60, proving the read fields' configured ownership.
Saved size reaches the shared setting data; see [size provenance](pen-size-findings.md).

Current time and movement are stored before a second skip test. Distance below
threshold toggles byte 284; an old nonzero byte skips geometry but updates
last-tested XY. Width and curve anchors remain unchanged (`0x30b18–0x30b3c`,
`0x30d60`). An old zero byte proceeds; distance not below threshold sets 1.
The saved entry sets threshold 50 for tool 1 and 5 otherwise (`0x30e24–0x30e58`).
For accepted geometry, v is `f32(f32(oldMovement+newMovement)*0.5)`.
The factor follows this source order (`0x30b64–0x30be4`):

| Ordered finite v | f32 factor operations |
| --- | --- |
| v<4.2 | 1 |
| 4.2<=v<9.5 | a=v+(-4.2); b=a/(-5.3); c=fma(b,0.4,0.4); factor=c+0.6 |
| 9.5<=v<35 | a=v+(-9.5); b=a/(-25.5); c=fma(b,0.5,0.5); factor=c+0.1 |
| v>=35 | 0.1 |

The non-exact constants have f32 bits 4.2=`0x40866666`, -4.2=`0xc0866666`,
-5.3=`0xc0a9999a`, 0.4=`0x3ecccccd`, 0.6=`0x3f19999a`, 0.1=`0x3dcccccd`.
With old width W and configured size S, sum=f32(factor+pressure),
half=f32(sum*0.5), candidate=f32(fma(half,S,W)*0.5) (`0x30c34–0x30c54`).
For finite values, absolute width change above 1 replaces candidate with W±1
toward it. Otherwise, change<=1 with W<1 and W>candidate instead subtracts
0.5 from W. Target is the ordered selection with floor `f32(S/10)`
(`0x30c58–0x30ca0`, `0x30d94–0x30db0`): actual division, not approximate
multiplication by 0.1. NaN/overflow inputs have instruction-specific predicates;
these formulas do not imply validation, saturation or a general pressure clamp.

## Curve sampling and ordinary completion

Accepted geometry computes midpoint(previousControl,currentXY) with per-axis
f32 add then multiply by 0.5. It calls SmPath rewind, moveTo(start),
quadTo(previousControl,midpoint), resetPath(false) (`0x30bf0–0x30c30`).
`getRepeat`, `0x31cac–0x31d74`, receives the truncated **minimum** of old and
target width. Its width bands scale path length L as follows:

| Integer width | Scaled length |
| --- | --- |
| <3 | L |
| 3 | L/1.5 |
| 4–5 | L*0.5 |
| 6–10 | L/3 |
| 11–15 | L*0.25 |
| 16–20 | L/5 |
| 21–30 | L/6 |
| 31–40 | L/7 |
| 41–55 | L*0.125 |
| 56–80 | L/9 |
| >80 | L/10 |

Count is truncation of `f32(f32(max(trunc(scaledLength),2)+1)*1.5)`.
The first band comparison is signed; later bounds use unsigned conditions.
For ordinary positive count n, position requests start at 0 and add f32 L/n;
width starts at W and adds the separately computed signed width change/n.
Each successful getPosTan emits half the current width (`0x30ccc–0x30d48`).
A failed lookup stops emission, but target width, midpoint/start and current
control/last-tested XY are still stored (`0x30d4c–0x30d60`). SmPath length and
position internals remain a library boundary; no exact endpoint coverage follows.

Saved completion constructs one MotionEvent from the last raw sample,
including optional tilt/orientation or zero, with first timestamp as downTime
and source 0 (`0x310a8–0x31150`). Base stores downTime at `0xbfad8`, copies
72-byte coordinates at `0xbfbfc`, and `GetEventTime`, `0xc0634–0xc0648`,
returns coordinate time minus downTime. Completion therefore uses last−first,
while interior calls use direct saved timestamps. No first-time-zero assumption
is justified. Source 0 also excludes endPen's tool-2/source-0x1002 override.

The full `endPen`, `0x2f43c–0x2f88c`, upper-caps selected pressure and computes
width with that distinct clock. It has no interior skip tests or width/time/
movement commit. Byte 284 chooses cubic(priorControl, midpoint(control,end),
end), or quadratic(priorControl,end) (`0x2f6c4–0x2f710`). Sampling starts at
zero: `0x2f7f0` clears the register previously holding L. No extra exact-L
sample follows. The saved caller ignores the endpoint bool, unions its bounds,
grows by 1 and supplies callback bounds (`0x31164–0x31194`).

## Derived geometry and current Rust boundary

`drawPoint`, `0x3095c–0x30a38`, optionally replaces radius with fixedWidth*0.5
when data byte 9 is set, then replaces an ordered radius<1 with 1.
It unions point±radius into bounds and calls RT slot 64. V4 actually installs
**RTV1**, not RTV4 (`0x2edb8–0x2ede8`, relocation `0x42a70`). RTV1 AddPoint,
`0x32ca4–0x32f24`, appends derived float triples x,y,f32(radius+0.5) into
its installed vector when nonnull; a null vector returns without emission.
That extra 0.5 belongs to the buffer, not drawPoint's bounds radius.
Fixed width alters emission radius without bypassing upstream width state.

When a stroke-data record exists, fresh cache insertion copies the generated
buffer and derived pen bounds; dirty clearing requires SetPointData success
(`0x31198–0x3121c`). A separate
extended rectangle is queued to RT (`0x31220–0x31248`). Queue execution and
callback lifetime are not guaranteed by the ordinary true return.
Saved channels remain distinct from generated points, bounds and cache state;
no saved-array setter is reached in this bounded chain. Current Rust uses
the [generic fallback](stroke-rendering-findings.md), without this V4 law.
Other versions, no-curve, StrokeTip, clean-cache reuse, live input and full
shader behavior are separate. [Opacity](pen-opacity-findings.md) and
[native PDF transport](native-pdf-stroke-findings.md) do not certify vector
outlines or complete visual parity for this source trace.
