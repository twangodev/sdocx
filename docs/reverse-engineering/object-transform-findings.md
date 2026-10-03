# Saved object geometry and container transforms

## Evidence and scope

Samsung Notes 4.4.45.37, ARM64, APK SHA-256
`daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667`.
The inspected library identities are:

| Library | SHA-256 |
| --- | --- |
| `libSPenModel.so` | `4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a` |
| `libSPenBase.so` | `e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb` |
| `libSPenDrawing.so` | `788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd` |

Most evidence is static reader, writer, mutator and drawing control flow.
Two bounded Rust/Unicorn research replays execute unchanged native numeric
helpers, described below. No complete editor operation, new saved Samsung
document, paired PDF export or pixel comparison executes here.

The type-4 container is a nested object list whose edits update child geometry.
Its ordinary drawing branch does not apply a parent affine matrix. This extends
the [saved rotation trace](object-selection-findings.md#saved-container-rotations-are-applied-to-children)
and [ordered child traversal](object-order-findings.md#containers-are-selected-as-objects-then-draw-their-children).
It does not establish transform semantics for every other parent object type.

## Double wire coordinates do not imply double editing geometry

Model `ObjectBaseBinaryHandler::ApplyOwnBinary`, `0x2db0e0`, reads the four
stored bounds as doubles at `0x2db3a4`–`0x2db3a8`, narrows each to a float at
`0x2db3b4`–`0x2db3c0`, and supplies them to Base `RectF::Set` at `0x2db3c4`.
The runtime rectangle is four floats at base-data offset 8.

The matching writer, `GetOwnBinary`, `0x2daad8`, loads those four runtime
floats at `0x2dabd0`, promotes them with `fcvtl` at `0x2dabd4`–`0x2dabd8`,
and writes four doubles at `0x2dabdc`–`0x2dabe0`. Saving cannot recover
precision discarded on loading. The SDK's raw double metadata and Samsung's
float geometry are therefore distinct representations.

Owner-page scaling happens after narrowing: `0x2db4b8`–`0x2db4c0` converts
the requested dimension and stored owner width to floats and divides them.
For a non-unit result, `0x2db4e8` scales all four rectangle components in
float arithmetic. This is load-time magnification, not an inherited container
matrix. The [optional field map](object-flexible-findings.md#modern-typed-frame-field-order)
records the owner dimensions, pivot and span snapshot separately.

Base `PointF::GetRotatedPoint`, `0xb0f10`, uses mixed precision rather than
one double affine calculation:

1. A zero float angle returns the original float point.
2. The angle is promoted and converted to radians in double arithmetic
   (`0xb0f34`–`0xb0f5c`).
3. Point-minus-pivot differences are computed as floats before promotion
   (`0xb0f60`–`0xb0f70`).
4. The trigonometric products use doubles, including fused multiply-adds
   (`0xb0f74`–`0xb0f80`).
5. Each rotated offset narrows to float before the float pivot addition
   (`0xb0f84`–`0xb0f90`).

Changing subtraction or final addition to doubles changes this contract even
when the same sine and cosine are used. This trace does not certify the host
math library against Samsung's `sincos` implementation.

## Container payloads contain editing flags, not a saved affine matrix

Model `ObjectContainer::NewGetBinary`, `0x3711cc`, writes the common base
binary at `0x37120c`, then a 14-byte type-4 frame at `0x371218`–`0x371254`.
`ObjectContainerImpl::GetOwnBinarySize`, `0x374dec`, also returns 14; its
writer, `0x374df4`, writes the same frame. There is a one-byte property mask
and an empty one-byte field mask, with no geometry or affine payload in this
own frame. Parent bounds and rotation belong to the common base frame.

`NewApplyBinary`, `0x3712cc`, loads the common base first, then dispatches
the remainder to `ObjectContainerImpl::ApplyOwnBinary` at `0x3713f4`.
The latter reads the property and field masks through `0x374efc` and
`0x374f0c`. Its own reader does not apply a stored matrix to children.
Children are loaded separately in the [container envelope sequence](object-order-findings.md#container-loading-preserves-a-nested-sequence).

| Container property | Runtime member | Writer and reader behavior |
| --- | --- | --- |
| Ungrouping enabled | Implementation byte 64 | Property bit 0 is positive; getter `IsUngroupable`, `0x36fb0c` |
| Invisible-child resizing enabled | Implementation byte 65 | Writer sets bit 1 when disabled; reader `0x374d10`–`0x374d28` enables it when bit 1 is clear or the passed format version is below 14 |

`SetInvisibleChildResizingEnabled`, `0x36fb6c`, writes byte 65 at
`0x36fbc0`; `IsInvisibleChildResizingEnabled`, `0x36fc18`, reads it.
This container-specific version rule is not an archive-format version mapping.
The complete mapped common fields also do not establish a generic affine
inheritance field. Opaque extension bytes retain their existing evidence
limits.

## Rotation moves child centers and updates each child's own angle

Model `ObjectContainer::SetRotation(float, bool)`, `0x371754`, obtains
the parent's **qualified base rectangle** through `ObjectBase::GetRect`
at `0x3717a4`. It does not call the container's virtual, child-derived
`GetRect` here. The implementation receives that rectangle and the new angle
at `0x3717d4`.

`ObjectContainerImpl::SetRotation`, `0x374570`, reads the current parent
angle through virtual slot 136 at `0x3745d4`. It computes
`delta = new_angle - current_angle` as a float at `0x3745f4`, and the
group center as float endpoint sums times `0.5` at `0x3745e8`–`0x3745fc`.
For each resolvable child handle:

- Read its virtual rectangle at `0x374630`.
- Unless all four components are exactly zero, move its rectangle about the
  group center using Base `RectF::GetRotatedRect` at `0x37466c`, then call
  its virtual three-argument `SetRect` at `0x37468c`.
- Read its own angle, add the same delta in float arithmetic at `0x3746a4`,
  and call its virtual `SetRotation(float, bool)` at `0x3746b4`.

There is no child visibility gate in this rotation loop. An all-zero rectangle
skips rectangle mutation but still reaches the angle setter.

Base `RectF::GetRotatedRect`, `0xb1954`, rotates the **rectangle center**
through `PointF::GetRotatedPoint` at `0xb1990`. It retains the original
signed width and height, reconstructs left/top as rotated-center minus half
the dimensions, and computes right/bottom by adding the dimensions
(`0xb19a4`–`0xb19b4`). It is not the enclosing axis-aligned bounds of four
rotated corners. Base `GetRotatedBound`, `0xb1850`, is the separate helper
that rotates all four corners and combines their extrema.

After the implementation succeeds, the container calls the qualified
`ObjectBase::SetRotation` at `0x3717f8`, updating the parent's own angle.
That metadata coexists with the children's edited geometry. It is not an
additional drawing transform waiting to be applied.

## Resize is a recursive mutation, including the method named DataOnly

Model `ObjectContainer::SetRect(RectF, bool, bool, bool)`, `0x3719b8`,
has a first-boolean branch that bypasses child mutation. The normal branch
uses the virtual current rectangle at `0x371af4`, normalizes destination
edge order, and computes separate float width and height ratios at
`0x371b58`–`0x371b84`. A zero old extent leaves its ratio at zero.

The child loop at `0x371bf4`–`0x371e14` performs these distinct operations:

| Operation | Native evidence |
| --- | --- |
| Read child rectangle and own angle | Virtual slots 168 and 136, `0x371c28` and `0x371c40` |
| Undo parent rotation for the child's center when nonzero | `RectF::GetRotatedRect` at `0x371c78` |
| Classify child-relative rotation | `GetNearestDirection(child_angle - parent_angle)` at `0x371c90`–`0x371ca8` |
| Scale dimensions with normal ratios for directions 0/180, swapped ratios for 90/270 | `0x371ccc`–`0x371d20`, `0x371e50`–`0x371e68` |
| Transform center, retaining destination edge inversions | Float fused `fmadd`/`fmsub` at `0x371d40`, `0x371d44`, `0x371d4c` |
| Restore parent rotation around the new group center | `RectF::GetRotatedRect` at `0x371d90` |
| Pass inverted child edge order to the child mutator when required | `0x371dac`–`0x371de8` |
| Mutate the child itself | Virtual slot 48, four-argument `SetRect`, at `0x371e04` |

The implementation then updates the parent's base rectangle through
qualified `ObjectBase::SetRect` at `0x371f78`. Nested containers receive
their own override through the virtual child call; this is recursive editing
rather than deferred parent scaling or shearing on export.

When invisible-child resizing is disabled, `0x371cb0`–`0x371cc0` tests
child visibility. The false branch at `0x371e1c` still moves the center by
the old-to-new origin change and handles edge inversions, but omits the
dimension scaling and center-offset multiplication. It still reaches the
child's `SetRect`. Hidden children are not simply left untouched.

`ObjectContainer::SetRectDataOnly`, `0x373018`, repeats the child geometry
algorithm: its child call at `0x3733b8` uses virtual slot 480,
`SetRectDataOnly`; its final parent write at `0x373534` is the qualified
base `SetRectDataOnly`. The name does not mean shallow parent metadata editing.
Its center propagation also uses float fused operations at
`0x373300`, `0x373304`, `0x37330c`.

The normal mutation paths temporarily set connectable children to connection
mode 1 and later restore mode 0. Missing runtime handles are removed from the
child vector. Those are editing side effects, not serialized transforms.

## The direction helper does not use ordinary modulo for every angle

Model `ObjectContainerImpl::GetNearestDirection`, `0x37389c`, computes
a float quotient by 360, truncates it to an integer, then uses different
positive/negative branches before quarter-turn classification. For ordinary
nonnegative values, boundaries 45/135/225/315 select the next quarter-turn.
The negative branch is not a general mathematical modulo operation.

A bounded research replay runs the unchanged function for 22 supplied angles
across memory fills `0`, `0x55`, `0xa5`, `0xff`, `0`; the repeated zero pass
agrees. These include adjacent float values below 45, 135 and 225.

| Supplied angle | Native result |
| ---: | ---: |
| `44.999996` / `45` | `0` / `90` |
| `134.99998` / `135` | `90` / `180` |
| `224.99998` / `225` | `180` / `270` |
| `315` / `360` / `450` | `0` / `0` / `90` |
| `-45` / `-90` / `-315` | `0` / `270` / `90` |
| `-360` / `-450` / `-720` | `-1` / `-1` / `-1` |

In the inspected resize caller, a result outside 0/90/180/270 bypasses the
dimension multiplications while center propagation continues. Normalizing all
angles with `rem_euclid(360)` would erase this behavior. The replay supplies
register inputs and filled implementation storage; no object construction,
group resize, serialization or drawing executes. Its canonical JSON SHA-256
is `078dcb524695646c052a86c3781257427adddbfc20fc959f171825feaa233dde`.

## Stroke edits update samples that the modern writer actually serializes

Model `ObjectStroke::SetRectDataOnly`, `0x2e6040`, copies temporary points
to the real-point buffer at `0x2e60cc`, writes the base rectangle, then
calls `ObjectStrokeImpl::ApplyRect` at `0x2e614c`. The ordinary history
branch of `ObjectStroke::SetRect`, `0x2de034`, reaches that same helper
at `0x2de2b4`; its no-history path dispatches `SetRectDataOnly` at
`0x2de3dc`.

`ApplyRect`, `0x2e8e14`, modifies the `PointF` vector at implementation
offset 48. Its general path optionally unrotates each point around the old
rectangle center, subtracts the old left/top, multiplies by separate float
ratios, handles destination edge inversions, adds the new left/top, and
restores rotation around the new center (`0x2e8f94`–`0x2e9060`). Zero old
width or height leaves that axis ratio zero. A separate unit-size-old-rectangle
case whose successive samples all coincide moves every sample to the new
rectangle center (`0x2e8e68`–`0x2e8eb4`, `0x2e9078`–`0x2e9164`).

The bounded zero-angle replay executes this unchanged helper with actual
Base rectangle and point arithmetic and actual Model `SetRefreshDrawnRect`.
It supplies point-vector storage and an owner/base implementation with null
model context, ignores reached Android logging, and repeats eight cases across
the same five fills. For old bounds `(10,20,30,40)` and points
`(10,20)`, `(15,30)`, `(30,40)`:

| Destination bounds | Native middle sample |
| --- | --- |
| `(100,200,160,240)` | `(115,220)` |
| `(160,200,100,240)` | `(145,220)` |
| `(100,240,160,200)` | `(115,220)` |
| `(160,240,100,200)` | `(145,220)` |

The other cases cover zero old width, zero old height, coincident samples in
unit old bounds, and a large fractional origin. No group caller, constructor,
pen channel, history, nonzero trigonometry, serialization or drawing executes
in this replay. Its canonical JSON SHA-256 is
`2d786f19c3514c7d4d3a7908c221dfa7c07799d7f167b378f22e80130df86329`.

Rotation similarly changes the samples. `ObjectStroke::SetRotation`,
`0x2de4e0`, copies temporary points, updates base angle data, then calls
`ObjectStrokeImpl::SetRotationPoint`, `0x2e8d14`. That helper computes
`new_angle - old_angle` in float at `0x2e8d8c` and overwrites every real
point with its rotation about the stroke rectangle center. The inspected
point-transform loops do not update pressure, timestamp, tilt or orientation
buffers. This is not a claim that every pen style is invariant under every
other editing operation.

The producer-to-writer connection is explicit. `ObjectStrokeBinaryHandler::NewGetBinary`,
`0x2ee39c`, first calls `CopyTempPointToRealPoint` at `0x2ee3fc`. Its compressed
path passes the tuple beginning with the same point vector at offset 48 to
the reducer (`0x2ee418`–`0x2ee428`). Its uncompressed path loads those float
points at `0x2ee48c`–`0x2ee498`, promotes through `PointD::Set(PointF)`, and
writes 16-byte coordinate pairs at `0x2ee4ac`. It does not undo the saved
rotation before writing. Stored double sample coordinates on that branch
therefore originate in edited float samples.

## Consequences for the current Rust representation

The recognized type-4 `PageObjectContent::Container` preserves child order;
ordinary native Drawing `drawObject` recursively supplies the existing canvas
and pen canvas at `0x7fcf4`–`0x7fd04`, without consulting the parent rectangle
or angle for an inherited transform. Rendering a parent SVG rotation on top
of these child records would apply the recovered container rotation twice.
Likewise, a generic rotation applied to stroke samples solely because the
common base angle is nonzero would repeat the stroke's already baked edit.
Leaf-specific consumers remain separate: shape and image geometry cannot be
assigned the stroke rule from a shared metadata field alone.

Shape saving provides a concrete different representation. Model
`ObjectShape::NewGetBinary`, `0x399b40`, snapshots geometry bounds, drawn
bounds and rotation, temporarily puts drawn bounds and zero rotation into the
common type-0 frame, then restores the original geometry and angle after writing
the frames (`0x399bd0`–`0x399be8`, `0x399cb4`–`0x399ccc`). Its
[saved-bounds refresh](text-layout-findings.md#saved-bounds-refresh)
separately recovers geometry from the shape data. A common base rectangle is
therefore not a universal substitute for the leaf's retained geometry.

Parent metadata still matters for inspection and future edits: the rotation
mutator uses the stored base rectangle as its pivot source, while selection
uses [child-derived bounds and margins](object-selection-findings.md#selection-depends-on-object-content).
Replacing both with one flattened bounding box loses that distinction.
The physical indices retain the parent record's location for metadata inspection
using the original page bytes; the semantic container currently carries children
rather than this editing state.

For unsupported parent kinds, `page.rs::decode_objects` traverses children
separately after retaining the opaque physical parent. The verified type-4
drawing rule does not certify that fallback for other groups, math/formula
parents or unknown object families. Child recovery can preserve useful vectors
while parent clipping, visibility, compositing or coordinate semantics remain
unimplemented. Structural retention and complete visual preservation are
different guarantees.
