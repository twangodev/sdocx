# InkPen2 V9 saved-stroke source authority

## Evidence and selected profile

These independently reviewed static ARM64 traces establish the explicitly
selected InkPen2 V9 ordinary single saved-stroke consumer. They do not establish
native execution, geometry equality or rendered appearance parity. Addresses
are ELF virtual addresses in Samsung Notes 4.4.45.37 APK SHA-256
`daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667`.

| Source | SHA-256 |
| --- | --- |
| `libSPenInkPen2.so` | `61806b1a7b11c89d0b9c8fa620ac7407d21eda9ac7555c280dbeb38076501c7c` |
| `libSPenPenCommon.so` | `afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d` |

The InkPen2 ELF matches its APK entry. The [registry](pen-selection-findings.md)
and [Drawing adapter](inkpen-v4-findings.md#evidence-and-admission) resolve and
configure the selected pen from saved name/settings/size. Count 1 and
StrokeType != 1 invokes drawable secondary-interface slot 40
(`0x82b54–0x82b70`). InkPen2 V9 secondary relocation `0x68db0` reaches thunk
`0x4e8d0` (this-8), then `RedrawPen(ObjectStroke*,RectF*)`, `0x4e1d0`.

InkPen2 constructor builds `9;` (`0x3ad28–0x3ad4c`), but construction defaults
are not authority for absent saved settings. The signed selector
`0x3b130–0x3b368` clamps to 1–9; table `0x70848` selects drawing 9 / outline 6
for explicit `9;`. Only drawing V9 is traced. Curve byte +8, fixed-width byte
+9, fixed width +12, line type +16 and rainbow byte +64 have actual setters
at `0x3b670`, `0x3b6a8`, `0x3b6e0`, `0x3b718`, `0x3b7c8`.

The plain numerical profile requires positive count and usable saved arrays,
fixed-width disabled, line type 0, rainbow disabled, finite positive size,
spacing and inverse scales, finite intermediate geometry and a representable
i32 sampling count. Rainbow distance must be finite and positive: the terminal
dot still evaluates its color helper. Curve on/off is explicit below; dash,
rainbow and fixed-width substitutions are separate branches. This saved path
does not reapply the [live beautifier](inkpen2-input-findings.md).

## Derived cache and initial state

Saved entry `0x4e1d0–0x4e614` selects original runtime handle unless -1,
otherwise current handle. Existing point data bypasses recomputation only if
inverse scale matches and redrawing-dirty is clear (`0x4e320–0x4e334`);
that branch supplies cached float geometry and pen bounds, not saved samples.
Dirty or scale-mismatched data is unreferenced before reconstruction.

Reconstruction installs the callback vector, assigns RT inverse scales
+296/+300, and obtains saved initial tolerance, rainbow offset and size level
(`0x4e430–0x4e480`). Successful redraw can copy generated floats into point
cache, store inverse scales/bounds and conditionally clear dirty after
SetPointData succeeds. Original handle resets -1 when strokeData exists,
even if SetPointData fails (`0x4e4f4–0x4e558`). Redraw success does not guarantee
cache publication. No saved sample-array writes occur in this adapter.

V9 constructor `0x4cbc8` initializes spacing +88=1.0f32, residual +92=0,
tool threshold +96=20, alternation byte +100=0, cached identity matrix and
inverse scales 1. `getInverseScale`, `0x4db58–0x4dca8`, compares 36 matrix
bytes; a changed matrix derives reciprocal f32 column norms and sets spacing
to f32(f64(min(inverseX,inverseY))*0.82), using f64 bits
`0x3fea3d70a3d70a3d`. An unchanged initial identity can retain spacing 1;
it does not universally select recomputed 0.82. Perspective/nonfinite matrix
behavior is outside the finite contract.

Lower saved redraw `0x4e614–0x4e8d0` seeds control/midpoint/last-emitted XY,
no-emission byte +376=1, radius=f32(size/2.2f32), residual=current spacing,
and dash offset=-spacing. Divisor 2.2 has f32 bits `0x400ccccd`. First signed
i32 time seeds working i64 times; speed, five history slots and index reset.
Saved IsShape seeds byte +48, separately from curve. The complete saved reset
sequence does **not** reset alternation +100. Cache recomputation can reuse a
version-matched drawable; constructor-zero alternation applies only to a new
V9. Reused incoming state may affect its first tool-1 decision.

## Interior movement and radius

Indices 1..count-1 forward XY, sign-extended saved i32 time, bounds, tool==1
and emission=true to `drawLine`, `0x4dcb8–0x4e1c8`. Pressure/tilt/orientation
are not interior operands. Distance from retained control is
sqrt(fma(dx,dx,f32(dy*dy))); a separate double overflow branch is present.
Except IsShape+fixed-width bypass, distance below saved tolerance skips
geometry and ordinary control/time commit. Tool 1 with distance<20 toggles
+100: old nonzero skips, old zero proceeds. Other tools or distance>=20
proceed and set +100=1 (`0x4dda8–0x4ddd8`). Skipped points are not
unconditional speed-history samples.

Curve-on constructs a quadratic from prior midpoint through control to
f32 midpoint(control,input); curve-off constructs a line control→input.
SmPath reset/getLength/getPosTan supplies length and sampled XY
(`0x4dd54–0x4de0c`). Those helper internals are not reconstructed here.
Speed uses measured path length, separately from the admission chord.
At `0x4de10–0x4dedc`, denominator candidate is f32(f32(time-priorTime)*1000),
or 1000 for zero delta. Denominator is **min(candidate,65535)**, with no
positive lower clamp; negative deltas remain negative. Resolution factor is
1 for signed minimum configured dimension zero, otherwise f32(1440/minDim);
see [size and configuration provenance](pen-size-findings.md).
Instantaneous speed=f32(f32(f32(length/denominator)*factor)*1000).
It enters a five-slot ring, whose ascending f32 sum is divided by 5.
Startup includes the other four zero slots.

Arithmetic below uses f32 operations unless f64 is stated. With old radius W,
configured size S and f32 average speed v, target follows:

| Ordered finite v | Target radius |
| --- | --- |
| v>5 | max(S/3.75,W*0.96f32) |
| 3<v<=5 | max(S/3.75,W*0.975f32) |
| f64(v)>0.8 and v<=3 | max(S/3.75,W*0.99f32) |
| f64(v)<0.2 | min(S*0.5,W*1.05f32) |
| f64(v)>=0.2 and v<0.5 | min(S*0.5,W*1.02f32) |
| remaining finite interval | W |

Fast branches use MAX (`0x4df6c–0x4df70`); slow branches use MIN
(`0x4e158–0x4e1a0`). Thresholds 0.2/0.8 are f64; 0.5/3/5 are f32.
Multiplier bits are 0.96=`0x3f75c28f`, 0.975=`0x3f79999a`,
0.99=`0x3f7d70a4`, 1.05=`0x3f866666`, 1.02=`0x3f828f5c`.
Sampling starts at residual; radius increment is
(target-W)/f32(trunc_i32(f32((length-residual)/spacing))+1).
While distance<=length, successful getPosTan emits a point and increments
radius; failed lookup still advances spacing. Completion commits residual,
new control, target radius, instantaneous speed and time (`0x4e0ac–0x4e108`).

## Terminal and callback geometry

The adapter materializes last XY/time/pressure plus optional tilt/orientation
or zero in a terminal MotionEvent (`0x4e7c8–0x4e840`). Complete `endPen`,
`0x4d1f8–0x4d51c`, reads **only X/Y**, narrowing to f32. Curve-on ends its
quadratic at actual end XY; curve-off ends its line there. No fresh speed or
radius update occurs. If no point emitted (+376=1), one control-position dot
uses half current radius (`0x4d3b4–0x4d3c0`): a single-sample dot starts with
size/2.2 then halves it before the radius floor. Otherwise terminal sampling
uses current radius while distance<=length; failed lookup still advances
spacing. Plain type 0 independently emits an exact endpoint and grows bounds
by 1; saved caller separately unions and grows by 1 (`0x4e844–0x4e858`).
The special dot branch skips the ordinary endpoint/grow branch.

`drawPoint`, `0x4ea94–0x4ecc0`, substitutes fixedWidth*0.5 when enabled,
then fmaxnm(radius,0.1f32), bits `0x3dcccccd`, and clears +376. Saved
emission=true extends bounds by point±radius and calls RTV3 AddPoint.
PenCommon GetBuffer `0x50640–0x50678` returns callback+40's float vector;
SetBuffer `0x4a5b8–0x4a5c4` installs that pointer at RT+80. RTV3 wrapper
`0x54210` invokes virtual slot 64, relocation `0x690b0`→`0x53cc8`.
It appends six geometry floats, followed by wrapper RGB floats:

- a=f32(r/sx), b=f32(r/sy), t=min(a,b), for positive inverse scales sx/sy.
- k=t<0.5 ? f32(0.5/t) : 1; coverage=f32(1/k); k=fmaxnm(k,1).
- ux=f32(f32(a*k)+0.5), uy likewise; m=max(ux,uy).
- Extents=f32(sx*ux),f32(sy*uy); edge=f32(f32(f32(m-1)/m)*0.5).
- Stored order: X,Y,extentX,extentY,edge,coverage,R,G,B.

These nine derived floats include scale-dependent half-pixel support; they
are not raw XY/radius triples. The emission=false branch stores 12-byte
XY/radius triples, but it is not this saved callback route. Shader coverage,
alpha composition, final vector outlines and SVG/PDF appearance are unproved.

## Saved channels and current Rust boundary

XY and interior saved time affect this plain path; size/resolution, saved
tolerance/tool/IsShape, incoming alternation and inverse scale also matter.
Pressure/tilt/orientation are fetched and terminal-materialized but are not
numerical operands of the reached geometry chain. A usable pressure array is
still required because terminal construction dereferences it. This does not
justify discarding channels or generalizing to live input, other versions or
effects; [recording](stroke-recording-findings.md) remains separate authority.

[prepare_stroke](../../crates/sdocx/src/ink.rs) does not admit InkPen2 V9;
it retains original points. [stroke_paint](../../crates/sdocx/src/render.rs)
uses generic width*(0.3+0.7*clamped pressure), not this time/history law.
Saved samples/settings remain the source; generated caches are derived.
Native execution, SmPath sampling precision, arbitrary incoming state, cache
lifetime, other effects/versions, shaders and native PDF equality are unproved.
