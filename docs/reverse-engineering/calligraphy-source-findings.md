# Calligraphy saved-source geometry

## Evidence and scope

Static ARM64 instructions from Samsung Notes 4.4.45.37 and the pinned APK in
[Sources and validation](README.md#sources-and-validation). The calligraphy
library below is byte-identical to its APK member, 166,320 bytes. Shared
libraries use the same APK's accepted source pins.

| APK member under `lib/arm64-v8a/` | SHA-256 |
| --- | --- |
| `libSPenMontblancCalligraphyPen.so` | `75aa5312fba02e21ff01d713187d2fd8d015ac4781e2d58da85ae700114e6f42` |
| `libSPenDrawing.so` | `788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd` |
| `libSPenPenCommon.so` | `afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d` |
| `libSPenBase.so` | `e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb` |

This covers selected MontblancCalligraphyPen V1, one ordinary saved object,
its orientation/extent consumers and generated geometry. It does not establish
native execution, numerical or visual parity, complete SVG contours, shaders,
other profiles or multistroke appearance. Existing [pen selection](pen-selection-findings.md),
[recording](stroke-recording-findings.md) and [Painting sources](painting-source-findings.md)
establish the separate registry, stored channels and editable-source boundaries.

## Settings and saved callback

The native registry contains full preload name `MontblancCalligraphyPen` and
its library stem (`PenManagerST::buildList`, `0x4da60`, table `0x78968`). Drawing
reads the saved pen name at `0x81a80` and requests manager data at `0x81ad4`.
Plugin `createPenInst`, `0x1cf5c`, constructs the pen at `0x1cf74`; constructor
GOT `0x2ba10` → vtable `0x2a2b8` installs primary address point `0x2a2c8`.
Attribute 4 succeeds in `GetPenAttribute`, `0x1cd2c`, so Drawing applies saved
advanced settings to slot 120 (`0x81bc4`), bound by relocation `0x2a340` to
inherited `Pen::SetAdvancedSetting`. The constructor supplies `1;` at `0x1ca70`.

Pen slot 232 binds `GetStrokeDrawableGL`, `0x1cd3c` (`0x2a3b0`). It calls
`getVersion` but uses fixed `versionTable` offset +8, value 1 (GOT `0x2ba08` →
`0x300f8`). Both constructor branches instantiate V1 (`0x1cdac`/`0x1cdb8`);
matching cached member 128 reuses the drawable (`0x1cd64`–`0x1cd70`). This is
this getter's selection contract, not a universal plugin settings grammar.

Drawing's count-one, `StrokeType != 1` route calls drawable secondary slot 40
(`0x82b54`–`0x82b70`). V1 constructor `0x1d774` installs primary `0x2a668`
and secondary `0x2a718`. Relocation `0x2a740` → thunk `0x1ed7c` adjusts `this`
by -8 and invokes saved `RedrawPen`, `0x1ec7c`. It reads tool/count/XY/pressure/
time/tilt/orientation (`0x1ecac`–`0x1ecf4`) and timestamp[0] without a local
empty/null guard (`0x1ed04`). A separate MotionEvent is constructed at `0x1ed28`,
with no optional 64-bit time array. Primary slot 144 (`0x1ed40`, relocation
`0x2a6f8`) binds event redraw `0x1e408`. Both adapter and thunk discard its
result and return true (`0x1ed44`, `0x1ed8c`), which does not certify emission.

## Optional channels and reused state

Base MotionEvent constructor `0xbfd84` requires nonnull XY/pressure/time pointers
(`0xbfdd8`–`0xbfde0`). On its admitted array path it copies the final sample into
allocated endpoint storage and adds count−1 preceding samples by value. Both
stylus pointers must be nonnull (`0xbff00`–`0xbff10`); otherwise endpoint
orientation stays initialized zero (`0xbff20`) and historical batches receive
zero tilt/orientation (`0xc00c4`–`0xc00d8`). One present stylus pointer does not
independently populate that channel here. These event defaults are distinct
from absent saved optional channels.

`GetHistorySize`, `0xc07a8`, counts historical samples. V1 rejects null event/
rectangle or nonpositive history before terminal processing (`0x1e438`–`0x1e458`).
A valid one-sample saved record therefore reaches inner false, while the outer
adapter still returns true. This is not an executed malformed-input result or
a conclusion about a visible dot.

Fresh V1 direction members 276/280 are 1/0; angle member 284 is 0
(`0x1d7dc`–`0x1d7e8`). Saved redraw initializes XY from historical sample 0 and
sets flags 80/272 true (`0x1e4b4`–`0x1e50c`, `0x1e578`/`0x1e57c`), but does
not reset those direction/angle members. Because the getter reuses its drawable,
fresh-constructor direction cannot be assumed for each record's short-stroke path.

## Orientation and historical emission

Historical indices 1 through history-size−1 supply XY and orientation to
`redrawLine` (`0x1e598`, `0x1e5ac`, `0x1e5bc`, `0x1e5d0`); f64 event XY is
narrowed to f32. Pressure reaches the event but is not the extent supplied by
this historical producer. `redrawLine`, `0x1e92c`, exits before state changes
when both finite XY deltas from accepted members 116/120 are below 1.5.
An admitted movement with flag 272 false arms it and updates accepted XY.
The first armed movement with flag 80 true initializes control/angle state,
clears both flags and emits no mesh (`0x1e990`–`0x1ea10`). Later armed movements
construct a quadratic centerline from previous generated XY, stored control
point and the midpoint of that control point/current XY (`0x1ea14`–`0x1ea64`).

For finite orientation o, native conversion multiplies f32 `o * 180` using an
f32 immediate, widens that rounded result, divides by f64 π at `0x110f8`, then
narrows to f32 (`0x1e9b0`–`0x1e9c4`, `0x1ea80`–`0x1ea8c`). The degree fold is
[0,180] → 90−angle, [-180,0) → −90−angle, otherwise → 0. This is not
shortest-angle wrapping; NaN behavior is outside this finite description.

When absolute difference exceeds 1 degree, the endpoint moves by ±1 degree,
with lower bound 0 if old angle >0, otherwise −90 (`0x1eac8`–`0x1eb08`). When
difference is at most 1, comparison old against target selects **max(old,target)**
(`0x1eaf8`/`0x1eafc`). The following comparison new against old separately
selects **min(old,new)** for `getRepeat` (`0x1eb0c`/`0x1eb18`). That helper
receives path length and this minimum angle, not pen width (`0x1eb28`).

`getRepeat`, `0x1f368`, transforms length by second-argument intervals:
≤5: ×0.5; ≤10: ÷3; ≤15: ×0.25; ≤20: ÷5; ≤30: ×0.125; ≤40: ÷7;
≤55: ÷9; otherwise: ÷10. It truncates f32 to signed integer, selects at least
2 and adds 10. This does not measure native path-length accuracy. Sampled angle
and position increments are f32; each angle widens, multiplies by f64 π and
divides by f64 180 at `0x110f0` before `sincos` (`0x1eb7c`–`0x1eb90`). Narrowed
sin/cos populate members 276/280 before `getPosTan` and `drawPoint`. A failed
position query exits the loop; an extra midpoint `drawPoint` follows at `0x1ec08`.
Stored endpoint angle does not certify recomputation of the direction pair there.

## Extent, terminal geometry and source authority

Drawing applies saved pen size through slot 16 (`0x81be8`–`0x81bf8`). Its actual
morphable route is slot 288 → `GetMorphable`, `0x1ceac`, returning `this+96`
(relocation `0x2a3e8`, interface address point `0x2a478`). Saved fixed-width
flag/value go through interface slots 32/48 (`0x81f3c`/`0x81f50`, `0x81f6c`/
`0x81f7c`), bound by `0x2a498`/`0x2a4a8` to thunks `0x1cefc`/`0x1cf34`.
They write the same pen-data byte 9/+12 read by `drawPoint` at `0x1f20c`/`0x1f214`.
Historical emission supplies half pen size (`0x1eb30`–`0x1eb58`); enabled fixed
width replaces that extent directly with the saved value, without halving it.
The finite extent is selected as at least f32 1 (`0x1f230`/`0x1f234`).

Redraw copies the final endpoint event and calls `endPen` at `0x1e708`. This
routine does not derive nib direction from terminal orientation. If initial
flag 80 remains true, it calls `drawPoint` at last accepted historical XY,
members 116/120, with half pen size (`0x1ddf0`–`0x1de18`); it need not have
initialized an angle. Otherwise terminal completion uses cubic or quadratic
centerline according to flag 272 (`0x1de38`–`0x1de98`), passes full pen size as
`getRepeat`'s second argument (`0x1dec4`) and supplies half size to repeated
`drawPoint` calls (`0x1df28`). These retain existing direction; position-query
success gates those calls. Bounds/valid-pointer returns do not certify appearance.

`drawPoint` submits Vector4(X,Y,cos,sin) plus extent to RTV1 `AddPoint`
(`0x1f2a4`–`0x1f2c8`). That consumer derives four corners around previous/current
XY (`0x20450`–`0x2047c`), handles initial/later cached edges, and calls `buildMesh`
into three render vectors at members 200/208/216 (`0x205e8`–`0x20618`). Derived
centerline samples, nib state and mesh vertices are separate from saved channels.

Current Rust `ink::prepare_stroke` selects Fountain/Marker2/Marker4 geometry;
this profile borrows original samples with `InkSupport::Approximate`. Its
[retained metadata](stroke-metadata-findings.md) and channels distinguish optional
orientation, pen/settings identity and fixed width from pressure. Original
source bytes/channels remain authoritative; generated geometry must not replace
them. Drawable reuse and terminal gates prevent an unconditional claim of
per-record deterministic native replay from source channels alone.
