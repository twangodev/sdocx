# Native shape fill paints

## Evidence boundary

These are static arm64 findings from Samsung Notes 4.4.45.37. Addresses are
ELF virtual addresses in the libraries extracted under the ignored
`scratch/apk-analysis-native/arm64-v8a/` directory.

| Library | SHA-256 |
| --- | --- |
| `libSPenModel.so` | `4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a` |
| `libSPenDrawing.so` | `788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd` |
| `libSPenComposer.so` | `52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f` |

The decompiled SDK `shapeeffect/SpenFillColorEffect.java` and
`SpenFillPatternEffect.java` provide API names and Java admission rules;
those rules do not establish what native binary loading accepts. No fill
shader, native PDF export or paired Samsung appearance has been executed for
these findings. The [shape frames](shape-line-findings.md) and
[outline styles](shape-style-findings.md) describe the surrounding frames,
path commands and outlines.

## Fill selection and record boundary

Type-7 flexible field bit 5 contains a four-byte effect byte length, a
one-byte effect kind, then that many effect bytes. The byte length excludes
the length prefix and kind byte. Native
`ObjectShapeBinaryHandler::ApplyBinary_FillEffect` (`0x3a890c`) checks the
bounded payload, constructs the selected effect, applies its bytes and then
sets it on the shape. Absent bit 5 returns success without applying an effect.

`ObjectShapeData::CreateEffect` (`0x3abfe0`) has four branches: color 1,
image 2, pattern 3 and background 4. An unknown kind returns null after
setting an error. The Java `SpenFillEffectBase` factory admits only 1–3;
it cannot establish that native kind 4 is invalid.

`ShapeDrawingFillEffect::SetEffect` (`0x9c2e4`) dispatches those same four
kinds, copying the saved effect into the matching drawing effect. Object
kind 8 returns success immediately without fill setup. The line draw route
also omits fill drawing, as described in the outline findings.

## Color effect wire data

`FillColorEffect::GetBinary` (`0x3b649c`) writes this canonical record;
`GetBinarySize` (`0x3b640c`) returns `18 + 8n` for `n` stops.
All multibyte fields use the native little-endian representation.

| Byte offset | Encoding | Meaning |
| --- | --- | --- |
| 0 | `u8`, written as 1 | Property-mask byte count |
| 1 | `u8` | Bit 0: gradient rather than solid; bit 1: gradient rotatable |
| 2–5 | `u32` | Solid ARGB, including alpha |
| 6 | `u8` | Gradient type |
| 7–8 | `u16` | Linear angle |
| 9–12 | `f32` | Gradient position x |
| 13–16 | `f32` | Gradient position y |
| 17 | `u8` | Stop count |
| `18 + 8i` | `u32` | Stop `i` ARGB |
| `22 + 8i` | `f32` | Stop `i` position |

The writer stores the low sixteen bits of the runtime signed integer angle
and low eight bits of the runtime gradient type. The loader zero-extends
those stored values (`0x3b6640–0x3b6664`); a negative runtime angle does not
round-trip as a negative integer. Neither matrix, spread mode, color-space
selection nor a separate fill-opacity scalar appears in this record.
Gradient settings and stops are written even when color kind is solid.

`ApplyBinary` (`0x3b659c`) advances past one prefix byte and reads a single
flags byte; it does not implement arbitrary-length property masks or
validate the prefix value. It masks only flags bits 0 and 1. It copies the
position floats directly and reconstructs stops in serialized order without
range checks on positions, finite-value checks, sorting or the Java ten-stop
limit (`0x3b6688–0x3b67dc`). Its per-field buffer checks remain relevant:
this is not admission of truncated records. Zero stops clears the runtime
stop vector and succeeds. Runtime stop IDs are generated anew rather than
serialized: each twelve-byte runtime entry contains ID, color and position;
the wire contains only color and position.

Native `Construct(ColorType)` (`0x3b579c`) zeroes the state, retains its
supplied color type and appends blue `0xff0000ff` at 0 and white `0xffffffff`
at 1. Solid color, gradient type, angle, position and rotatable flag therefore
start at zero. The Java no-argument constructor also supplies the two stops.
Java `appendGradientColor` admits at most ten and sorts by position;
`setGradientColor` also sorts. Native `AppendGradientColor` (`0x3b588c`)
has the ten-stop check and a sorting call. The binary loader bypasses that
mutator, so setter behavior must remain separate from load behavior.

## Gradient drawing admission

`SetFillColorEffectGradient` (`0x9d9a4`) fails for a nonpositive stop count,
uses `min(count, 10)` for positive counts (`0x9dac0–0x9dad0`) and dispatches
only these types:

| Stored gradient type | Native drawing branch |
| --- | --- |
| 0 | Linear, `0x9daf0–0x9dcb0` |
| 1 | Radial, `0x9dd1c`, helper `0x9de20` |
| 2 | Rectangular, `0x9dd44`, helper `0x9e224` |
| Other | Error branch `0x9dd70–0x9dda4` |

Java names type 3 `GRADIENT_PATH`, but `setGradientType` rejects values
above 2 and this native drawing dispatcher does not support 3. The constant
alone is not evidence of a path-gradient rendering contract. Loaded records
can retain more than ten stops even though this drawing route uses only the
first ten. One stop passes this admission check; downstream shader validity
is a separate question.

Linear and radial branches pass saved stop positions and ARGB colors into
Skia shader constructors. When a drawing-context color translator exists,
colors first pass through its virtual slot `0x50` with role argument 3
(`0x9dc14–0x9dc30`, `0x9dfbc–0x9dfd8`). The color translation policy is
not established here. Shader calls use tile-mode value 0, flags 0 and null
local matrix (`0x9dc50–0x9dc6c`, `0x9dff8–0x9e018`). Those are runtime
choices, not additional saved fields.

### Linear geometry and precision

`ShapeDrawingCommon::GetLinearGradientPoint` (`0x9bb94`) computes two
endpoints from the object's unrotated rectangle and saved integer angle.
It compares the angle with the rectangle's `atanf(height / width)` diagonal
angle to select intersections with opposing left/right or top/bottom edges.
The helper includes signed remainder by 180, subtraction of 180 when the
input exceeds 179, and endpoint reversal for inputs at least 180; it is not
an unrestricted angle-normalization API.

The helper uses `f32` rectangle differences, centers, diagonal-angle
calculation and radians conversion, then double `tan` and fused double
multiply/add or multiply/subtract before narrowing endpoints to `f32`
(`0x9bc48–0x9bcf4`). A generic bounding-box SVG angle formula need not
produce these exact endpoints.

The caller reads the object's rotation, adds 360 once if it is negative,
and rotates the endpoints around the rectangle center only when the
rotatable flag is set and the resulting angle is greater than `0x34000000`
(approximately `1.1920929e-7`). Rotation uses double `sincos` and fused
double products, followed by narrowing and `f32` center additions
(`0x9da00–0x9da40`, `0x9db30–0x9dbdc`). This threshold test is literal;
it does not use the rotation's absolute value.

### Radial geometry

The radial branch calls its helper with the rectangle height first and width
second (`0x9dd1c–0x9dd3c`). For a finite ordinary rectangle `(l,t,r,b)`,
write `W = r-l`, `H = b-t` and saved position `(px,py)`. The helper constructs
center `(l + W*px, t + H*py)` using `f32` fused operations. Position is
therefore proportional to the shape rectangle; it is not a stored absolute
page coordinate.

Radius starts as `sqrt(fma(H, H, W*W))` in `f32`. If both
`abs(px - 0.5)` and `abs(py - 0.5)` are strictly less than the literal
`0x34000000` threshold, radius is multiplied by 0.5. Equality keeps the full
diagonal length, as does any larger difference. The
noncentral position is rotated around the rectangle center only when the
rotatable/positive-angle condition succeeds (`0x9dea0–0x9df7c`). A centered
radial gradient on a nonsquare rectangle therefore has a circular radius
of half its diagonal; it is not simply an ellipse fitted to its bounds.
No independent focal point or elliptical transform is stored in this effect.

### Rectangular gradient clipping

`SetFillColorEffectRectangle` (`0x9e224`) initializes four fill paints,
reads the same position and stop records, constructs linear shaders and
polygonal partition paths, and has separate rotated and unrotated branches.
The four shader/paint slots are at drawing implementation offsets `0x688`,
`0x710`, `0x798` and `0x820`; three partition paths are at `0x8a8`,
`0x8c0` and `0x8d8`.

`DrawEffectColor` (`0x9cc00`) draws the shape with the first paint, applies
path clip-operation value 0 with antialias false, draws the second paint,
and conditionally repeats with the third and fourth. It later applies
clip-operation value 2 to the partition paths (`0x9cc5c–0x9cd30`,
`0x9ce40–0x9ce4c`). Thus native rectangular gradients are a sequence of
vector paths, clips and linear paints, not one radial shader. The exact
partition formulas and native clip-state behavior are not certified by
execution in this document.

## Pattern tiles

`FillPatternEffect::GetBinary` (`0x3ba448`) writes exactly sixteen bytes:
eight row bytes, then foreground ARGB at byte 8 and background ARGB at byte
12. `ApplyBinary` (`0x3ba4c0`) bounds-checks those three regions and copies
them without interpreting an additional hatch enum. Native `Construct`
(`0x3ba0b0`) and the Java constructor use eight zero rows, opaque-black
foreground and transparent-black background. Java exposes eight `char`
values; the native stored representation is eight bytes.

`SetFillPatternEffect` (`0x9c7d4`) processes rows in order 0–7 and bits in
order 7–0. A one selects foreground and a zero selects background. It writes
RGBA bytes for all 64 cells (`0x9c858–0x9c8b0`), constructs an 8×8 bitmap,
and creates a bitmap shader with both tile-mode arguments 1 and null matrix
(`0x9c8b4–0x9c900`). The pattern branch then draws the shape path with that
paint. There are no saved tile dimensions, origin, angle or scale in the
sixteen-byte effect.

The authored source is a one-bit tile plus two colors. It can be retained
and expressed as an 8×8 repeating vector cell pattern without reproducing
the native bitmap intermediate. Exact phase and cell scaling under the
surrounding canvas transform remain uncaptured; the literal shader arguments
alone do not establish page-space units.

## Solid fill subpaths and background fills

Solid color does not always mean one fill of the outline path.
`SetFillColorEffectSolid` (`0x9d798`) asks type-7 shapes for their fill-path
count, caps it at ten and sets up individual paints. `DrawEffectColor`
then converts and draws those fill paths (`0x9cda8–0x9ce38`). Fill-type 0
sets transfer-mode value 0; types 2/3/4/5 install a color-matrix image filter
with respective RGB offsets `+0.2`, `+0.4`, `-0.2`, `-0.4`, multiplied by
255. The matrix preserves alpha (`0x9d888–0x9d90c`, helper `0x9f144`;
constants `0x517dc`, `0x517f8`, `0x517d4`, `0x517c8`). Java names these
transparent, lighten-less, lighten, darken-less and darken. The generation
of template-specific fill paths is separate from the effect wire record;
an outline path alone does not prove complete shading geometry.

Native kind 4 `FillBackgroundEffect` writes one `f32` transparency value
(`GetBinary`, `0x3b536c`; loader `0x3b53d4`). Its drawing setup requires an
externally supplied background bitmap, creates a bitmap shader with tile
arguments 0/0 and null matrix, and fails if that bitmap is absent
(`0x9c9e4–0x9ca84`). This record does not carry an image resource ID or
vector background scene. The intended source scene and transparency
application are not recovered by this setup trace.

## PDF export is a separate paint capability

In Composer, `PdfPathAdapter::SetFillColorEffect` (`0x345d68`) updates the
path's fill mode/color only when color type is 0. A gradient makes no fill
update. `ObjectShapePdfExporter::ExportObject` (`0x34ef10`) copies a color
effect only for effect kind 1 and passes it to that adapter. Its separate
image adapter is called when the whole path export returns false
(`0x34f20c–0x34f230`); this is not a demonstrated per-gradient fallback.
The older `ObjectShapePDFWriter::WriteObject` (`0x37c810`) likewise draws
fill only for color-effect kind 1 with solid color type 0
(`0x37c9cc–0x37ca7c`). Its solid alpha is `f32(alphaByte / 255) * opacity`.

These static routes establish neither vector gradient export nor correct
pattern/image fill export. Successful path export alone must not be treated
as proof that all fill semantics survived. Preserving the authored paint
records gives Rust exporters more information than those solid-only adapter
paths; it does not require copying Samsung's export omissions.

## Consequences for this codebase

`crates/sdocx/src/shape.rs::read_paint` retains the complete bounded payload
for gradient effects through `ShapePaint::Unsupported`. It reads positions
and stops but does not expose them as typed semantic data; for solid paints
it keeps only ARGB and discards the dormant gradient configuration. The
finite-float checks in Rust are stricter than the native binary copy route.
Pattern, image and background effects remain opaque kind/data records.

The preserved record needs to remain distinct from the paint that a native
drawing route can consume: stop order, all stops, ARGB alpha and raw enum
values describe source data; the ten-stop cap and recognized shader types
describe one renderer's admission. Rectangular gradients and solid template
subpaths also require geometry beyond a single outline fill.

SVG linear/radial gradients, vector pattern cells and piecewise clipped
linear paints can express these source families without new raster images.
That representability is an implementation constraint, not a current SDK
support or appearance-parity claim. Exact rectangular partitions, pattern
phase, contextual color translation, template fill-path generation and the
background-fill scene dependency remain bounded evidence gaps.
