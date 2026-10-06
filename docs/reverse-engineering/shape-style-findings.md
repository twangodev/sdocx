# Native shape outline styles and arrowheads

## Evidence boundary

These findings come from static arm64 disassembly of Samsung Notes
4.4.45.37 `libSPenDrawing.so`, SHA-256
`788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd`,
and `libSPenModel.so`, SHA-256
`4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a`,
and the decompiled SDK class
`com.samsung.android.sdk.pen.document.shapeeffect.SpenLineStyleEffect`.
Addresses below are ELF virtual addresses. The native input remains under
the ignored `scratch/apk-analysis-native/arm64-v8a/` directory. No native
execution or Samsung appearance comparison was performed for these contracts.
The [shape and line findings](shape-line-findings.md) describe the saved
frames, path encoding and existing renderer separately.

## Saved style fields and loading

In `libSPenModel.so`, `LineStyleEffect::GetBinary` (`0x395b44`)
writes exactly twelve bytes:

| Byte offset | Encoding | Field |
| --- | --- | --- |
| 0–3 | `f32` | Width |
| 4 | `u8` | Compound type |
| 5 | `u8` | Dash type |
| 6 | `u8` | Cap type |
| 7 | `u8` | Join type |
| 8 | `u8` | Begin arrow type |
| 9 | `u8` | Begin arrow size |
| 10 | `u8` | End arrow type |
| 11 | `u8` | End arrow size |

`LineStyleEffect::ApplyBinary` (`0x395bec`) loads the enum bytes directly
without restricting them to the named SDK values. It multiplies the saved
width by the supplied scale using `f32` arithmetic
(`0x395cf4–0x395d08`). The drawing contracts below consume the loaded width,
rather than the unscaled saved float.

The Java `setCompoundType` accepts only 0, 1 and 4, although the same class
defines named values 2 and 3. Native `SetCompoundType`
(`0x3955d4–0x3955ec`, Model) directly stores the integer. Thus the Java setter
is not evidence that other compound values cannot occur in a loaded style.
The named arrow types are 0 none, 1 arrow, 2 open, 3 stealth, 4 diamond and
5 oval; named sizes are 0 normal, 1 small and 2 big. Neither the loader nor
the Java arrow setters validate those ranges.

## Path commands at the drawing boundary

`ShapeDrawingCommon::convertToPath` (`0x9bd24`) dispatches runtime verbs
1–7 to move, line, quadratic, cubic, arc, close and oval operations. Runtime
segments have a 28-byte stride; their coordinates are `f32`.
The arc branch copies the first four floats into `SkRect`, loads start/sweep
angles from the next two floats, and calls
`Spen_SkPath::arcTo(rect, start, sweep, true)` (`0x9bd54–0x9bd74`).
It is a rectangle-and-angles arc with the force-move argument enabled, not
an SVG endpoint arc. The oval branch calls `addOval(rect, Direction(1))`
(`0x9bdb8–0x9bdd4`). The direction enum's numeric meaning is not established
by this call alone. An unrecognized verb returns without adding a path
command (`0x9bdec`); this drawing boundary does not report a validation error.

## Outline activation

`ShapeDrawingLineEffect::SetEffect` (`0x9f854`) records the object type,
constructs default color/style effects, copies the object's effects, and
configures its `SkPaint` (`0x9f8f4–0x9f930`). Objects of type 7 supply
`ObjectShape::GetPath`; type 8 supplies `ObjectLine::GetPath`
(`0x9f984–0x9f9c8`; type-7 path call at `0x9f9b4`). This is the native
drawing path, rather than a fresh primitive inferred from the object's
bounding rectangle.

Construction sets the visibility byte to one (`0x9f6b8`), and `SetEffect`
copies the saved line-color effect (`0x9f900`). Color kind 1 dispatches
known linear/radial/rectangular gradients (`0x9fb40`); linear/radial helpers
install their shaders on the main `SkPaint` (`0xa23d8`, `0xa2760`). The
line-color loader treats the whole saved property byte as rotatable when
nonzero (`0x393ac8–0x393adc`), unlike fill's separate bits 0/1. The
[fill findings](shape-fill-findings.md#skia-stop-construction) describe their
shared stop construction and geometry. This establishes static activation
and paint installation, without an executed outline scene or pixel claim.

Color kind 2 disables the outline by clearing the visibility byte
(`SetLineColorEffect`, `0x9faec–0x9faf4`). The corresponding
`isLineShow` getter reads that byte at implementation offset `0x24`
(`0xa01f8–0xa020c`). `AbsShapePathType::DrawEffect` draws the fill first
and invokes the outline only when this getter succeeds
(`0x8bab0–0x8bb3c`; fill/outline calls at `0x8bb00`/`0x8bb10`).

`LinePathType::DrawEffect` uses a different route (`0x8bba4`): it never calls
the fill effect. A visible line with either nonzero arrow type calls the
general `DrawEffect`; a visible line without arrows calls `DrawLineEffect`
directly (`0x8bbdc–0x8bc2c`). `HasLineArrow` tests nonzero begin/end values,
without restricting them to the named SDK enum range (`0xa0178–0xa01e8`).

## Width, dashes, caps and joins

`SetLineStyleEffect` (`0x9fcec`) reads the width and replaces a negative
value with zero before calling `SkPaint::setStrokeWidth`
(`0x9fd14–0x9fd3c`). The following table uses this effective width `w`.
Intervals and phase are passed to `SkDashPathEffect(const float*, int, float)`.
They are lengths in the drawing path's coordinate system; the phase is a
literal float, not multiplied by `w`.

| Saved dash type | SDK name | Intervals | Phase | Branch |
| --- | --- | --- | --- | --- |
| 0 | Solid | No new dash effect | — | `0x9fde4–0x9fdf8` |
| 1 | Round dot | `[0.01, 2w]` | 0 | `0x9fe90–0x9fec8` |
| 2 | Square dot | `[2w, w]` | 1 | `0x9ff58–0x9ff84` |
| 3 | Dash | `[4w, 3w]` | 1 | `0x9fdfc–0x9fe30` |
| 4 | Dash dot | `[4w, 3w, w, 3w]` | 1 | `0x9fed0–0x9ff0c` |
| 5 | Long dash | `[8w, 3w]` | 1 | `0x9fd6c–0x9fda0` |
| 6 | Long dash dot | `[8w, 3w, w, 3w]` | 1 | `0x9ff14–0x9ff50` |
| 7 | Long dash dot dot | `[8w, 3w, w, 3w, w, 3w]` | 1 | `0x9fe48–0x9fe88` |

The two loaded interval constants are floats `[8, 3]` at `0x518d8` and
`[4, 3]` at `0x51918`. The short round-dot interval is the immediate float
bit pattern `0x3c23d70a`, approximately `0.01`, and is independent of width.
Values outside the named dash branches add no dash effect in this function.
These branches do not explicitly clear a previously installed effect.

Cap and join values are applied only when their unsigned value is less than
3 (`0x9ff88–0x9ffbc`). SDK cap names are butt/round/square for 0/1/2;
join names are miter/round/bevel. The dash branches do not override the cap:
the label "round dot" alone does not establish a round-cap activation rule.
This trace does not establish the downstream appearance of zero-width or
degenerate dash intervals.

## Compound outlines

`DrawEffect` (`0xa0258`) forces compound type 0 for object type 8,
irrespective of its saved style (`0xa02ac–0xa02bc`). For other object
types, saved values 2 and 3 are normalized to 1 before calling
`GetDrawLineInfo` (`0xa0310–0xa031c`). The SDK names 2/3 as thick-thin and
thin-thick, but this drawing route does not preserve that distinction.

`GetDrawLineInfo` (`0xa0810`) accepts only the following effective values;
other values return false after setting a native error.

| Effective compound type | Pass widths | Count | Addresses |
| --- | --- | --- | --- |
| 0: Simple | `[w]` | 1 | `0xa0830–0xa083c` |
| 1: Double | `[w, w / 3]` | 2 | `0xa0878–0xa088c` |
| 4: Triple | `[w, 2(w / 3), w / 3]` | 3 | `0xa0894–0xa08b0` |

The widths are successive draws of the same path (`0xa039c–0xa03d8`).
Even-index passes draw the outline paint. Odd-index passes set transfer mode
1 and alpha zero, draw the path, then restore alpha (`DrawLine`,
`0xa119c`, `0xa1288–0xa12d0`). Thus the native compound effect removes a
central band from the temporary surface and, for triple, draws its center
again. It is not generated by translating the path into separate parallel
strokes. The precise downstream Skia transfer-mode behavior and edge coverage
have not been captured here.

## Pen conversion activation and derived storage

The compiled pen conversion branch requires an activation byte at Drawing
implementation offset `0x388`, simple compound, solid color and solid dash
(`0xa0378–0xa0398`, `0xa185c–0xa187c`). The implementation constructor
clears that byte and initializes its pen-name pointer to null
(`0x9f734`, `0x9f754`). `SetEffect` clears the byte again for normal shapes
and lines (`0x9f8f0`), before loading effects, without reading their saved
pen-name/settings references. Normal construction and this object setup
therefore do not activate the conversion. No positive producer was found in
the inspected class; external private-pointer writes and other native versions
are not excluded.

`ConvertToObjectStroke` (`0xa08f8`) iterates the supplied drawing path into
private sampled-point storage. `MakeLineStroke` (`0xa1f3c`) allocates fresh
`ObjectStroke` instances from those samples, supplies pressure `1.0` and
timestamp `0` for every point (`0xa1fe4–0xa1ff0`), assigns paint-derived size
and color, and appends them to its private list (`0xa2038–0xa2058`). These
are generated samples, not retained original stylus pressure/time/tilt data.
The destructor deletes the owned strokes and list (`0x9f428–0x9f478`);
no per-conversion list clear or revision-identity lookup was found in this
seam, so the list is not evidence of a coherent rendering cache.

The bounded `drawObjectStroke` body (`0xa0d94–0xa117c`) does not use its
`CanvasBitmap` argument or submit its constructed motion event to a pen.
It configures pen properties, offsets the event, then destroys it
(`0xa1140–0xa1148`). Property setters may have separate side effects;
their bodies are not exhaustively established here. This function's name
does not establish active shape-pen replay.

The model's own shape writer serializes its template drawing path
(`0x3a8e74–0x3a8e8c`) and saved pen resource IDs (`0x3a8ff0`,
`GetBinary_PenData` at `0x3ac284`), rather than this Drawing-owned list.
The inspected conversion creates private derived objects without replacing
the source shape. This is not a claim that every drawing/export/save route
is free of model mutation.

## Arrow size and local geometry

`setArrowSize` (`0xa00bc`) computes three integer nominal lengths from `w`.
The formulas below apply to finite widths whose converted products and
integer sums remain within the signed 32-bit range.

| Saved size | SDK name | Nominal length `L` |
| --- | --- | --- |
| 0 | Normal | `trunc(3w) + 10` |
| 1 | Small | `trunc(2.5w) + 5` |
| 2 | Big | `trunc(4w) + 15` |

The first two products use float-to-double widening followed by integer
truncation, with double constants `[3, 2.5]` at `0x51850` and integer
addends `[10, 5]` at `0x51890`; the third uses fixed-point integer
conversion (`0xa00cc–0xa00f4`). The same initialization appears in
`SetEffect` (`0x9f9e0–0x9fa2c`). Begin/end size values directly index this
three-entry array in the arrow routines; those accesses do not clamp an
out-of-range saved value.

`DrawEffect` calls `DrawBeginArrow`/`DrawEndArrow` only for object type 8
with a non-null stored path and positive segment count
(`0xa0498–0xa0504`). `DrawEndArrow` independently requires at least two
segments (`0xa1524–0xa1528`). `DrawArrow` (`0xa1ad4`) translates the
canvas to the supplied endpoint, then rotates it. The following coordinates
are local to that translated/rotated canvas:

| Arrow type | Local geometry | Addresses |
| --- | --- | --- |
| 1: Arrow | Closed triangle `(-L/2, L) → (0, 0) → (L/2, L)` | `0xa1e2c–0xa1e78` |
| 2: Open arrow | Open path `(-L/2, L) → (0, 1.118w) → (L/2, L)` | `0xa1cc8–0xa1d18` |
| 3: Stealth | Filled path `(0, L/2) → (-L/2, L) → (0, 0) → (L/2, L) → (0, L/2)` | `0xa1d54–0xa1dcc` |
| 4: Diamond | Centered square of side `L`, with an additional 45-degree rotation | `0xa1dd4–0xa1e24` |
| 5: Oval | Circle centered at `(0, 0)`, radius `L/2` | `0xa1c8c–0xa1cb0` |

The `1.118` factor is an `f32` constant at `0x517cc` (bits
`0x3f8f1aa0`). The open arrow uses the outline paint after temporarily
clearing its path effect; the previous effect is restored afterward
(`0xa1d1c–0xa1d4c`). Filled arrow shapes use a separate fill paint carrying
the outline paint's color (`0xa1b40–0xa1b6c`).

For nonzero arrows, `ConvertToPath` handles a short first or last line
segment specially. If its measured length is below `1.7w`, it emits a move
instead of that line and marks an endpoint flag (`0xa06ec–0xa0798`; double
constant `1.7` at `0x51898`). `DrawArrow` consumes that flag in a separate
temporary-surface branch for types 1–3 (`0xa1b94–0xa1c68`). Consequently,
the arrow path is not the only operation involved in endpoint appearance.

The native angle calculation is also retained literally as static evidence:
`DrawBeginArrow` calls `atan2f`, multiplies its result by `360` in `f32`
(immediate bits `0x43b40000`),
widens to `f64`, divides by `6.283185307179586` at `0x51910`, narrows to
`f32`, and adds `90` (`0xa1490–0xa14d0`). `DrawEndArrow` uses the same
scale and subtracts `90` (`0xa16ac–0xa16ec`). This has the conventional
degrees-per-radian scale, with the specific intermediate rounding stated
above. The resulting orientation,
cubic-endpoint treatment, overlap removal and native pixels have not been
verified by execution.
