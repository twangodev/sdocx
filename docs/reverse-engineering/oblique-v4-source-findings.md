# Oblique V4 saved-source geometry

## Evidence and scope

Static ARM64 instructions from Samsung Notes 4.4.45.37 and the pinned APK in
[Sources and validation](README.md#sources-and-validation). Addresses are ELF
virtual addresses; the Oblique library is byte-identical to its APK member,
444,312 bytes. Shared libraries use the same APK's accepted source pins.

| APK member under `lib/arm64-v8a/` | SHA-256 |
| --- | --- |
| `libSPenObliquePen.so` | `9b76bdb709db9f56c5d35a1a615fe41c57ceef73c559c91efaf5bf436f90e0ee` |
| `libSPenDrawing.so` | `788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd` |
| `libSPenPenCommon.so` | `afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d` |

This covers selected ObliquePen V4, one ordinary saved object with
`StrokeType != 1`, curve enabled and no clean reusable point cache. It does
not establish native execution, numerical or appearance parity, complete
contours, shaders, other versions, effects or live input. Existing
[pen selection](pen-selection-findings.md), [recording](stroke-recording-findings.md)
and [Painting sources](painting-source-findings.md) establish separate stored
channels and editable-source boundaries.

## Settings and saved callback

The registry resolves full preload name ObliquePen to its library through
PenCommon `buildList`, `0x4da60`, pairs at `0x78968`. Drawing obtains saved
name/manager data at `0x81a80`/`0x81ad4`. Constructor `0x3625c` loads GOT
`0x6ef18` → vtable `0x6c1f8`, installs primary address point `0x6c208` at
`0x362a8`, and supplies `4;` through slot 120 (`0x36348`–`0x3636c`).
Attribute 4 succeeds (`0x366f0`/`0x366f8`), so Drawing applies saved advanced
settings at `0x81bb0`/`0x81bc4`; relocation `0x6c280` binds inherited
`Pen::SetAdvancedSetting`. Saved size reaches slot 16 (`0x81be8`/`0x81bf8`),
relocation `0x6c218` → inherited `Pen::SetSize`.

Pen slot 232 (`0x6c2f0`) binds `GetStrokeDrawableGL`, `0x36704`. It calls
`getVersion`, clamps its index to 1–4 (`0x36718`–`0x36738`), and reads pairs
at `0x73ec8` through GOT `0x6ef10`; selected first words are 1, 2, 3, 4.
Explicit version 4 constructs V4 at `0x367f4`–`0x36808`; a matching cached
drawable is reused (`0x36740`–`0x36750`). Exported V5 code does not establish
V5 selection from this pinned table.

Drawing's count-one, `StrokeType != 1` route (`0x82b54`–`0x82b70`) calls
drawable secondary slot 40. V4 constructor `0x3f314`–`0x3f32c` installs primary
`0x6c948` and secondary `0x6c9f8` through GOT `0x6eff8` → vtable `0x6c938`.
Relocation `0x6ca20` → thunk `0x411b8` adjusts `this` by -8 and enters saved
`RedrawPen(ObjectStroke*,RectF*)`, `0x40928`.

## Saved inputs and interior geometry

The adapter requires nonnull stroke/rectangle and initialized canvas/RT
(`0x40958`–`0x40974`). It converts configured f32 size S with `FCVTZU w9,s0`
and stores u32 RT member 60 (`0x409dc`–`0x409e8`); fixed-enable separately
reaches member 424. `SetPenSizeData`, `0x450cc`, agrees. For finite
nonnegative S < 2^32, that integer is trunc(S); this describes neither other
input domains nor rounding of the original serialized f32 size.

Original/runtime handles locate transient point data (`0x40a2c`–`0x40a68`).
Matching inverse scale and a clean point buffer bypass reconstruction
(`0x40a7c`–`0x40a94`). Fresh reconstruction installs callback buffer RT 264,
inverse scale 352/356 and saved initial tolerance (`0x40b5c`–`0x40bd0`), then
calls lower `redraw`, `0x40d64`, with emission true.

Lower redraw requires positive count/non-null XY (`0x40db0`–`0x40dc8`) and
gets time, pressure, tilt and orientation (`0x40dd4`–`0x40df8`). Tool 1 selects
distance threshold 50, otherwise 5 (`0x40e04`–`0x40e30`). First XY seeds
drawable 100/124/132 and RT previous XY 344; flags 88/89 reset true
(`0x40e34`–`0x40e68`). Control/midpoint 108/116 are not reset in that slice.
The curve interior supplies successive XY/emission true to `redrawLine`,
`0x405a0` (`0x40e6c`–`0x40ea0`), without pressure/time/tilt/orientation operands.
Its first admitted segment sets control and clears flag 88 without emission
(`0x40630`–`0x40640`). Later sampled positions reach `drawPoint`, `0x41648`,
with half size; `GetRepeat` receives full size (`0x4069c`–`0x40744`). These
input roles do not establish executed SmPath sampling parity.

## Fixed width, diagonal core and inverse scale

Saved curve/fixed-enable/fixed-value really reach the morphable interface.
Primary slot 288 (`0x6c328`) → `GetMorphable`, `0x36b00`, returns plugin +96;
constructor `0x362ac` installs address point `0x6c3f8`. Drawing supplies them
at `0x81f20`/`0x81f50`/`0x81f7c`. Interface slots 16/32/48 bind thunks
`0x36b18`/`0x36b50`/`0x36b88` (`0x6c408`/`0x6c418`/`0x6c428`), writing
PenData bytes 8/9 and f32 member 12. Constructor defaults are curve enabled,
fixed disabled, value zero (`0x36314`/`0x36324`); saved values replace them.

`drawPoint` selects local extent B as half fixed value when enabled, otherwise
incoming half size (`0x41684`–`0x41694`). Its true-emission branch calls RTV1
`AddPoint(Vector2<float>)` with XY only (`0x41698`–`0x416a4`), without passing
B or updating RT size. B enters local bounds: for finite ordered values,
`r = fma(max(B,1), c, 1)`, with f32 c = `0.7071067690849304`, bytes `f304353f`
at `0x22700` (`0x416b4`–`0x416c0`). Previous emitted XY at drawable 132 and
supplied XY enter that extent (`0x416c4`–`0x41714`). This is a local bounds
consumer, not a universal fixed-width no-op.

Actual RT `AddPoint`, `0x45288`, loads previous XY 344, calls `lineToMesh`,
`0x45374`, at `0x45324`, then `makeTriangles`, `0x455d0`, at `0x45348` with
generated corners and callback buffer 264. V4 inlines RTV1 construction
(`0x3f3a4`–`0x3f44c`, GOT `0x6f078` → vtable `0x6cd60`), initializing member
200 to c from `0x227ac`; standalone ctor `0x445fc`/`0x44668` agrees.
Reached size setter `0x450cc`, inverse setter `0x47cf0`, redraw-state setter
PenCommon `0x4a4a4` and color setter `0x450dc` do not overwrite member 200.
This does not establish global immutability across every reused drawable.

For the admitted S domain, the core loads u32 trunc(S), uses `UCVTF` to f32,
halves it, selects at least 1, then multiplies by current runtime coefficient
K at member 200 (`0x45374`–`0x4539c`). K starts as c; substituting c requires
that initialization to remain. Local bounds c is a separate literal. Let a
denote that staged f32 result using K. Core corners are previousXY+(a,-a),
previousXY+(-a,a), currentXY+(a,-a), currentXY+(-a,a), with third component 1
(`0x453a8`–`0x453f8`). Their diagonal direction is independent of supplied
stylus orientation. Complete `lineToMesh` through `0x455cc` has no fixed-value
or member-424 read. Inverse scale 352/356 enters additional fringe corners
through near-coincident versus normalized-direction branches (`0x45404`–`0x455cc`),
separate from those core corners. `makeTriangles` compares core separation
against f32 ~1e-6 (`0x45600`–`0x45660`); coincident pairs can append nothing.

## Terminal and retained state

Lower redraw builds a temporary MotionEvent: final XY is promoted f32→f64;
final signed timestamp supplies event time, first supplies down-time, each
zero if absent (`0x40f14`–`0x40f38`, `0x40f7c`–`0x40fa8`). Final pressure
defaults to 0.5; tilt/orientation each independently default to zero when their
array is absent (`0x40f44`–`0x40f74`). These defaults do not fill saved arrays.
It calls `endPen` with emission true, unions its rectangle and destroys the
event (`0x40fb8`–`0x40fdc`). Complete `endPen`, `0x3f8b4`–`0x3fc24`, reads
only event X/Y and narrows to f32 (`0x3f910`–`0x3f924`); supplied pressure,
tilt, orientation and time do not enter this terminal geometry. Its bool
reflects event/rectangle validity, not guaranteed sampled-point admission.

Terminal rect includes drawable 100, retained control 108/midpoint 116 and
final XY (`0x3f928`–`0x3f970`). If flag 88 remains true, `drawPoint` uses
member 124 with half size (`0x3f974`–`0x3f9a4`), seeded as first XY at startup.
Included 108/116 can retain constructor/prior-call state if no segment replaced
them. This concerns returned bounds, not a demonstrated visible reuse effect;
the coincident-corner gate prevents an always-drawn one-sample tap claim.
Otherwise curve-enabled tail chooses cubic versus quadratic by flag 89
(`0x3f9d4`–`0x3fa18`, `0x3fb20`–`0x3fb34`), samples with full size for
`GetRepeat`, emits half size, then final XY and expands rect by 1
(`0x3fb34`–`0x3fbe8`). The same fixed local-bounds/core-size split applies.
The separate no-curve terminal branch builds a line (`0x3fa20`–`0x3fb1c`);
its interior and complete sampling law are outside this scope.

## Source authority and Rust boundary

Successful fresh reconstruction may cache derived callback data, record inverse
scale/pen rect, conditionally clear dirty state and reset original runtime
handle (`0x40c34`–`0x40c84`). This is transient geometry; clean-cache output is
outside the fresh equations. Original XY/pressure/time/optional stylus channels,
unrounded f32 size, fixed-width data and pen/settings identity remain separate.
The diagonal policy differs from [calligraphy's orientation law](calligraphy-source-findings.md)
and [InkPen's pressure/time width](inkpen-v4-findings.md). Current Rust
`ink::prepare_stroke` recognizes Oblique but borrows original samples with
`InkSupport::Approximate`. Retain those source channels and settings rather
than truncating stored size, inventing saved nib directions or replacing
original data with mesh/cache buffers.
