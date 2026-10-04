# Native shape fill paints

## Evidence boundary

These are static arm64 findings and bounded native captures from Samsung
Notes 4.4.45.37. Addresses are
ELF virtual addresses in the libraries extracted under the ignored
`scratch/apk-analysis-native/arm64-v8a/` directory.

| Library | SHA-256 |
| --- | --- |
| `libSPenModel.so` | `4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a` |
| `libSPenDrawing.so` | `788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd` |
| `libSPenComposer.so` | `52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f` |
| `libSPenSkia.so` | `42636cb9ac06cc286114b42c1b9d8f4b78d33761843251cde2b443b101ffb88d` |
| `libSPenBase.so` | `e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb` |

The decompiled SDK `shapeeffect/SpenFillColorEffect.java` and
`SpenFillPatternEffect.java` provide API names and Java admission rules;
those rules do not establish what native binary loading accepts. The native
capture section identifies executed geometry, shader construction and command
boundaries; no native PDF export or paired Samsung appearance is certified.
The [shape frames](shape-line-findings.md) and
[outline styles](shape-style-findings.md) describe the surrounding frames,
path commands and outlines.

## Fill selection and record boundary

Type-7 flexible field bit 5 contains a four-byte effect byte length, a
one-byte effect kind, then that many effect bytes. The byte length excludes
the length prefix and kind byte. Native
`ObjectShapeBinaryHandler::ApplyBinary_FillEffect` (`0x3a890c`) checks the
bounded payload, constructs the selected effect, applies its bytes and then
sets it on the shape. Absent bit 5 returns success without applying an effect.

The preceding type-7 pen-data field 3 has a four-byte boundary even though its
meaning is unknown. Model `ObjectShapeData::ApplyBinary_PenData` (`0x3ac340`)
tests bit 3 at `0x3ac3b4`, checks four bytes at `0x3ac3b8–0x3ac3c4`, then
advances the cursor by four at `0x3ac3c8–0x3ac3d0`. It does not load, convert
or store their value. Field 4's advanced-settings ID follows immediately.
The modern `ObjectShape::NewApplyBinary` calls own-frame loading at
`0x399e38`; that loader applies pen data at `0x3a9764` and fill at `0x3a9780`
through the same checked cursor. This slot therefore precedes the known fill
without assigning it an invented numeric type.

The current pen-data writer (`0x3ac284–0x3ac33c`) emits only fields 2 and 4;
it does not emit field 3. The own-frame writer calls it at `0x3a8ff0` before
the fill. This scoped omission does not identify an older producer or prove
every native save route drops these bytes. The slot was traced statically,
without a containing-record execution. Rust retains field 3 as optional
`NativeShape.pen_data_field_3_raw` bytes and continues decoding later pen
settings and fill. Its semantic meaning remains unknown.
Type-8 field 3 remains the separate line-path contract.

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
general partition formulas and final native clip-state behavior are not
certified by execution; bounded partition commands are captured below.

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

## Shape frames and inherited paint coordinates

Static Drawing `ObjectShapeDrawing::SetObject` snapshots drawn bounds through
object slot 160 at `0x8a414`. Gradient setup instead reads current rotation
and common unrotated rectangle through slots 136/168 (`0x9d9fc`, `0x9da1c`);
Model ObjectShape relocations `0x4945e8`, `0x494600`, `0x494608` identify
these methods. Gradient endpoints/centers/partitions use that common frame,
not the temporary bitmap's extent; admitted rotation is baked into geometry,
while the shader-local matrix remains null.

Drawn bounds have another precision boundary: parameterless GetDrawnRect
truncates float rotation to i32 (`0x397d48`). Its implementation compares that
integer angle with actual float rotation and, when unequal, requests another
template path (`0x3a645c`–`0x3a647c`, helper `0x20d68c`) for bounds. Drawing
still retrieves the display path through `0x8bc94`. Extent calculation and
painted geometry must remain separate; this is not a new paint-local matrix.

Pattern setup copies rows/colors, without reading an object frame or angle
(`0x9c464`–`0x9c46c`). Inner `drawPath` converts display commands, then saves,
uniformly scales and translates the existing canvas by minus the supplied
rectangle's left/top (`0x8a8c0`, `0x8a8d0`, `0x8a8e0`). The path adapter
forwards that same canvas to fill drawing at `0x8bb00`. Thus, at command level,
the pattern shares supplied path coordinates with no separately installed
object-origin/angle transform. This source inference does not establish
sampled page phase: inherited transforms, optimized addressing and later
bitmap composition remain distinct, as the native capture below shows.
The [image-fill consumer](image-effects-findings.md#ordinary-image-effect-paths-differ-from-image-filled-vector-shapes)
instead constructs explicit local matrices from its saved coefficients and
selected common/path-bound frame; it is not the pattern contract.

Normal type-7 own loading refreshes/magnifies geometry (`0x3a96b8`,
`0x3a96d8`) before loading fill at `0x3a9780`; the fill loader receives version
and byte length, without a geometry matrix or magnification argument.
Final data-only writes restore the common frame (`0x399eec`, `0x399ef8`).
The inspected rectangle/rotation mutators update common/template geometry,
not gradient or pattern coefficients; rectangle editing also updates crop/
bound-image flip state (`0x3a6b1c`, `0x3a6b7c`). This bounds the direct native
bodies, not arbitrary notification callbacks, compatible legacy loading or
every coedit route. No actual movement/load appearance execution is claimed.

## Solid fill subpaths and background fills

Solid color does not always mean one fill of the outline path.
`SetFillColorEffectSolid` (`0x9d798`) asks type-7 shapes for their fill-path
count, caps it at ten and sets up individual paints. `DrawEffectColor`
then converts and draws those fill paths (`0x9cda8–0x9ce38`). Fill-type 0
sets transfer-mode value 0; types 2/3/4/5 install a color-matrix image filter
with respective RGB offsets `+0.2`, `+0.4`, `-0.2`, `-0.4`, multiplied by
255. The matrix preserves alpha (`0x9d888–0x9d90c`, helper `0x9f144`;
constants `0x517dc`, `0x517f8`, `0x517d4`, `0x517c8`). Java names these
transparent, lighten-less, lighten, darken-less and darken.

Model keeps base and display fill lists at template implementation `+152`
and `+160`, distinct from the outline pair. Count reads the base list
(`0x20f9fc`); ordinary `GetFillPath` returns a display entry and the fill-type
word at Path `+8` (`0x20fcbc`). Common append `0x20d2bc` copies the supplied
contour/type into the base list, then rotates a copy by template angle `+48`
for the display list (`0x20d34c–0x20d35c`, `0x20d3b8–0x20d55c`). Drawing's
ten-path cap is a separate consumer limit.

These positive source branches derive closed fill contours from expected
angle-zero template geometry; they do not establish every template or safe
admission of arbitrary saved/custom commands.

| Template | Loader → fill generator | Derived contours in paint order |
| --- | --- | --- |
| Can, 26 | `0x22a238→0x229fa0` | 12 commands/type 1; 10 commands/type 2 |
| Cube, 27 | `0x22b2dc→0x22b760` | Three five-command contours, types 1/2/4 |
| Bevel, 40 | `0x238e60→0x238a84` | Five five-command contours, types 1/3/2/5/4 |

The inspected version-17 WDoc own writer saves one display outline
(`0x3a8e64–0x3a8e8c`), angle (`0x3a8e58–0x3a8e5c`) and controls
(`0x3a8ee4–0x3a8f20`), without independent template fill lists. Its loader
calls the selected template with mode 1, horizontal/vertical flip flags and
saved angle (`0x3a95ac–0x3a95c0`). Common `0x20bad0` stores those flags and
angle; the mode-1 path branch inverse-rotates supplied commands before base
installation (`0x20c278–0x20c2b4`). The three loaders then regenerate fills.
Late saved controls narrow f64→f32 and call the template control slot
(`0x3a9618–0x3a9628`), which can regenerate outline and fills again
(Can `0x22a984`, Cube `0x22aeb8`, Bevel `0x239e44`). See the
[saved-control findings](shape-path-findings.md#saved-control-points-can-regenerate-the-preceding-outline).

Angle changes rebuild display fills from base entries (`0x20da84–0x20db28`).
Common rectangle resize transforms and refreshes both lists while retaining
types (`0x20e07c–0x20e1b0`); concrete rectangle setters also regenerate
fills. Template generation reads horizontal/vertical flip flags; these do
not imply another generic flip of already oriented saved commands. Native
generators assume their source command layout: Cube reads fixed indices up
to 10 after only a nonnull-path/segment gate. No complete saved-load scene
or rendered shading comparison was executed for these source findings.

Rust retains the template ID, path, controls, angle and flip flags, but
`render_shape` fills one stored outline without deriving these contour/type
lists or replaying template controls. The inspected face geometry is derived
from retained inputs, rather than missing independent saved face records;
reconstruction depends on the native restore/control/float/transform order.

Native kind 4 `FillBackgroundEffect` writes one `f32` transparency value
(`GetBinary`, `0x3b536c`; loader `0x3b53d4`), not an image resource ID or
vector background scene. `ObjectShapeDrawing::SetBackgroundImage` stores
an external `CanvasBitmap const*` at implementation `+48` (`0x8a480`),
initially null (`0x8a31c`). `SetEffect` forwards it to the owned fill
effect (`0x8a4f4–0x8a508`). The ordinary `drawObjectShape` route constructs
a fresh drawer, then sets its object/effects and calls `DrawPath`
(`0x8070c–0x80744`) without supplying this background pointer. Previously
drawing page paper does not itself bind that paper to this effect.

`SetFillBackgroundEffect` returns false when the pointer is absent
(`0x9c9f8–0x9c9fc→0x9ca84`), but fill `SetEffect` ignores that result and
returns true (`0x9c3e4–0x9c3e8→0x9c314`); the outer drawer also ignores
the fill setup status. This is not proof that the whole shape disappears.
With a supplied bitmap, setup installs its bitmap shader with tile
arguments 0/0 and null local matrix (`0x9ca00–0x9ca1c`), without reading
source bounds, rotation or transparency. The kind-4 draw branch submits
the supplied outline and main paint to the existing bitmap canvas under
its current state (`0x9cb90–0x9cba4`). The upstream scene and transparency
application remain unresolved; the consumed bitmap alone does not
identify or recover an underlying vector composition.

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

`NativeShape.fill_source` and `ShapeStyle.paint_source` expose
[`ShapePaintSource`](../../crates/sdocx/src/shape_paint_source.rs) separately from rendering.
Canonical color records retain dormant gradient settings, ordered stops, ARGB,
raw flags/enums and trailing bytes. `PaintFloat32` preserves nonfinite bits and
signed zero without finite-value rejection. Pattern sources retain eight tile rows,
both ARGB colors and trailing bytes. Noncanonical color masks keep original payload
bytes. Admitted ordinary 42/62-byte image fills expose signed bindings, mode,
raw offset/scale/transparency bits, the rotatable byte, version-gated nine-patch
fields and trailing bytes. Other image/coedit forms and background effects remain
opaque. Gradients, patterns and image fills remain unsupported by the renderer.

The preserved record needs to remain distinct from the paint that a native
drawing route can consume: stop order, all stops, ARGB alpha and raw enum
values describe source data; the ten-stop cap and recognized shader types
describe one renderer's admission. Rectangular gradients and solid template
subpaths also require geometry beyond a single outline fill.

SVG linear/radial gradients, vector pattern cells and piecewise clipped
linear paints can express these source families without new raster images.
That representability is an implementation constraint, not a current SDK
support or appearance-parity claim. Final canvas clip state, pattern phase,
contextual color translation, other template fill-path generators and the
background-fill scene dependency remain bounded evidence gaps.

## Executed paint geometry and commands

Temporary Rust/Unicorn probes executed original Model/Drawing routines and
Skia shader construction, path mutation and paint installation. Forty-six
gradient cases and fourteen pattern cases each matched across five fresh
machines with allocation/stack fills `0x00`, `0x55`, `0xa5`, `0xff`, `0x00`.
An independent reviewer reproduced the final gradient capture SHA-256
`30c9be10e1657208277d7e6f2a3529e3b55003816896142de25ed6c8b670f222` and
pattern capture `a689416683e615e10f7f66d1cdbe0a8434a34873fa83dbf578e8510ea63a8f90`.

Native Drawing constructors (`0x9bee0`, `0x9bfc4`) initialize owned state;
the admitted null context bypasses contextual color translation. Model
construction, setters, getters and selected binary reading/writing execute
natively. Drawing's gradient dispatcher (`0x9d9a4`) receives explicit
rectangle/rotation values through virtual slots 136/168 and itself computes
the midpoint, adjusts negative rotation and initializes point buffers. That
source interface is not a native Model `ObjectShape` or a complete scene.
Bounded host allocation/free/reallocation, byte copies and single-thread
mutex stand-ins support execution; all remaining native imports trap by name.
Native `sincos` calls use Linux libm, identified through the actual linked
symbol with `dladdr`, SHA-256
`6d567d53e895273ca14a1f9dc164fc6c8d39aed2f60aa46a733c2784228915f3`.
This is not execution of Android libm.

For bounds `(10.25,20.5,310.25,70.5)`, observed radial shader inputs are:

| Position / rotation | Center | Radius |
| --- | --- | --- |
| `(0.5,0.5)`, zero | `(160.25,45.5)` | `152.06906127929688` |
| `(0.25,0.75)`, zero | `(85.25,58)` | `304.13812255859375` |
| `(0.25,0.75)`, rotatable `37.25°` | `(92.98367309570312,10.052974700927734)` | `304.13812255859375` |

One `f32` step above `0.5` still produces half-radius; two steps produce
exactly `2^-23` difference and full radius. Both axes were exercised, as were
tall translated bounds, rotation disabled, negative rotation and 90 degrees.
For binary stop counts 1/10/11/255, native count getters and binary writers
retain the entire count and stop bytes. Drawing sends at most the first ten
to the observed shader factories, preserving saved order and duplicate stops.

Zero-rotation centered rectangular cases generate midpoint-to-edge-midpoint
linear shaders and three closed triangular paths. Corners generate two
shaders and one triangular path. Rotatable centered/corner cases generate
rotated points. For centered `37.25°` with rotation disabled, native instead
uses axis-aligned bounds of the rotated corners: approximately
`(25.717346,-65.194145,294.782654,156.194153)`. Tested position
`(0.25,0.75)` and center-offset equality at `2^-23` produce no new partition
path and two zero-endpoint shaders, using the dispatcher's zeroed buffers.
These are observed renderer routes, not restrictions on saved positions.

Original `DrawEffectColor` (`0x9cc00`) and Skia `clipPath`/`isRect`
(`0x19b700`) execute through recording canvas callbacks. Centered cases draw
the same outline four times, sending operation 0 for successive partitions
between draws, then operation 2 for all three. Corners draw twice and send
operation 0 then 2 for one partition. Antialias is false throughout. External
canvas callbacks record commands and return success without applying clip
state; final clip state and pixels are excluded. The first draw precedes
partition clips. A vector
implementation must account for potential overlap and blend order; disjoint
triangle fills cannot be assumed equivalent.

Pattern construction, sixteen-byte binary roundtrip, native getters and the
complete Drawing byte-generation loop execute, as does Base's empty
`Bitmap()` wrapper constructor (`0xa3cb0`). Execution stops before the
pixel-buffer `Bitmap::Construct` call (`0x9c8d4`): width/height 8, row stride
32, format 1, final booleans 0/1.
All 896 observed cells agree with row/MSB order and exact ARGB-to-RGBA
conversion, including transparent colors. That byte-generator capture excludes
pixel-buffer `Bitmap::Construct`, CanvasBitmapFactory, shader execution and
tile phase; the following pattern-construction capture extends its boundary.

### Native vector clip-stack state

A separate probe submits the prior captured partition operation sequence
directly to native `SkClipStack::clipDevPath` (`0x1a38b8`), then executes
native bounds queries, element iteration/replay and optional save/restore.
This is not an integrated effect-dispatcher/canvas scene. Eight cases match
across five fresh machines with varied heap/caller-storage fills; independent
source rebuild reproduces SHA-256
`9a91a711ef5c0ed184fcf09b7c11ee997dbbb1d38b8e8aba521155c3d3145209`.

Incoming clips are wide-open, a rectangle, a triangle and an explicit cubic
path; source paths are constructed by native path methods. Element replay
retains native path references and ordered operation/antialias values. The
operation-0/2 sequence leaves six added elements. For incoming rectangle or
triangle metadata bounds `(120,30,180,60)`, final native `getBounds`
(`0x1a2d24`) reports `(10,21,310,71)`. These bounds include `BoundsType`
metadata, not an exact visible region; the wide-open/difference cases are
not finite-region certificates. Native save/restore returns a structurally
equal original stack through native `operator==`; that equality is not a
claim of pixel-region equivalence. RasterClip, device clipping and pixels
remain excluded. Vector exporters must preserve paths, operations, ordering
and scope rather than substitute these metadata rectangles.

Native RawIter (`0x1d41ac`) on replayed path references retains the original
fractional midpoint/corners, including `(10.25,20.5)`, and incoming cubic
control points. The antialias-false metadata branch (`0x1a1908–0x1a191c`)
adds the literal `f32` vector
`(0.44999998807907104,0.5,0.5,0.5)` from `0x88870` and applies `FRINTM`.
That metadata adjustment does not rewrite the captured vector coordinates.

### Native pattern construction and coordinate addressing

A separate fifteen-case capture continues original `SetFillPatternEffect`
(`0x9c7d4`) through Base's pixel-buffer `Bitmap::Construct` and `CreateShared`,
Drawing's `CanvasBitmapFactory` (`0x6eea4`), original SkBitmap/Spen_SkCanvas
construction, bitmap shader creation and paint installation. There is no
`installPixels` bridge. Native source-bitmap lock/unlock verifies its retained
256 bytes equal the Drawing generator, including hidden RGB at alpha zero
and partial-alpha color controls. The shader receives width/height 8,
color-type/alpha-type 4/2, repeat modes 1/1 and null local matrix. This is
source-tile construction, not output rasterization.

Ten transform controls and five additional alpha/color controls each match
across five fresh allocation/byte-generator stack fills. Independent rebuild
reproduces SHA-256
`860fd971c381f26b65ba8a765fe4bb3ecddb2f9174cd325eadc08e7f0cfa953b`.
Host services additionally supply bounded byte fill and ignored Android
logging. Unsupported imports still trap.

Original `computeTotalInverse` (`0x20bb5c`) concatenates canvas matrix,
shader-local matrix and optional context-local matrix before inversion;
`ContextRec` matrix pointers are at offsets 16/24. Identity, translation,
nonuniform scale, shear and reflection succeed; singular scale returns false.
Null-local cases use the original installed shader. Local-matrix controls
explicitly create another native shader from its retained bitmap. These
supplied matrices are controls, not additional recovered pattern-wire fields.
`asABitmap` (`0x17a730`) reports an identity matrix even with a nonidentity
shader-local matrix; that query alone omits local/context transforms.

Actual default SkPaint construction, shader `createContext` (`0x20bd10`)
and native `chooseProcs` (`0x18180c`) also execute. Context inverse matrices
retain the geometric inverse, while selected coordinate-procedure state
normalizes scale/shear by tile dimensions and specializes translation-only
addressing. With supplied canvas translation `(3.25,-2.5)`, the geometric
inverse translation is `(-3.25,2.5)` but default no-filter procedure state uses
`(-3,2)`. Original coordinate procedures add device-point half offsets and
wrap negative coordinates; identity spans starting at 0/8 match, as do
-9/7/15. No sampling procedure or output pixels execute. Addressing
specialization must not replace preserved source/context transforms. Full
application canvas phase, filtered appearance and alpha blending remain
uncertified.
