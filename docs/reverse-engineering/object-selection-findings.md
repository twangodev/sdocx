# Object intersection selection and saved container rotation

## Evidence and scope

Inspected Samsung Notes 4.4.45.37 ARM64 `libSPenModel.so`, `libSPenBase.so`,
`libSPenDrawing.so` and `libSPenComposer.so` from the local APK extraction.
The APK identity is recorded in the knowledge-base index. These are native
control-flow and vtable findings, without new Samsung SDOCX/PDF captures.
Addresses below identify their library explicitly. No Python oracle is needed
for this investigation.

## Query rectangles and root selection

Composer `NotePDFExporterRasterListX::getPageObjectList`, `0x35ad54`, obtains
page width and height at `0x35ad8c` and `0x35ad98`, converts them to floats,
and supplies `(0, 0, width, height)` to `WPage::FindObjectInRectIntersect`
at `0x35adbc`. The type and render masks remain separate arguments.

Composer `NoteCapturePage::CapturePage(ISPBitmap*, RectF*, LayerMode)`,
`0x32ee44`, constructs the same full-page rectangle at `0x32ee9c`–`0x32eeb8`.
A non-null caller rectangle replaces it at `0x32eebc`–`0x32eecc`.
`drawPage` receives this rectangle at `0x32efb0` and forwards it unchanged
through `drawObject`, the pass methods and `getCloneObjectList`. Output bitmap
dimensions and scale determine a separate drawing matrix; they do not replace
the selection rectangle.

Model `ObjectManager::FindObjectInRectIntersect`, `0x35e670`, calls
`ObjectBase::IsIntersect` at `0x35e740`. That method, `0x2d2470`, implements:

```text
selection = object.getSelectionRect()
if query.Contains(selection): return true
if !query.IsIntersect(selection): return false
return object.hasValidDataInIntersectRect(intersection(query, selection))
```

The virtual selection call is at `0x2d24a8`; containment, overlap and
intersection calls are at `0x2d24bc`, `0x2d250c` and `0x2d2524`; the virtual
content check is at `0x2d253c`. Model relocations `0x492318` and `0x492320`
identify slots 528 and 536 as the two named object methods.

Base `RectF::Contains(RectF)`, `0xb1158`, includes coincident edges and does
not reject empty rectangles. A contained point or zero-width selection can
therefore be accepted before the content check. Base
`RectF::IsIntersect(RectF)`, `0xb127c`, requires positive query width and
height and strict overlap on all four sides. An external rectangle touching
only the query edge fails that test. One strict rectangle-overlap predicate
does not reproduce the combined selection operation.

## Selection depends on object content

| Object | Verified behavior |
| --- | --- |
| Base object | `getSelectionRect`, Model `0x2d4000`, dispatches virtual `GetDrawnRect`. The base partial-content check, `0x2d406c`, returns true for an existing implementation. |
| Stroke | `GetRect`, Model `0x2e6b18`, starts from base bounds and extends right/bottom by one when width/height is zero. `getSelectionRect`, `0x2e6d9c`, expands this by half pen size at `0x2e6de0`–`0x2e6dec`, then applies rotated bounds when needed. |
| Shape | `getSelectionRect`, Model `0x39c33c`, uses path bounds with outline width/join through `Path::GetBounds` at `0x39c40c`; the no-path branch uses its rotated geometry rectangle. |
| Container | Inherited `getSelectionRect` dispatches `ObjectContainer::GetDrawnRect`, Model `0x371848` (relocation `0x493e58`). This uses computed container bounds, child-derived maximum margins and rotation. |

For partially intersecting strokes, Model
`ObjectStroke::hasValidDataInIntersectRect`, `0x2e6e88`, checks successive
sample-segment rectangles expanded by half pen size at `0x2e6f10`–`0x2e6f54`.
A one-sample stroke has no segment in this loop; full containment bypasses it.

Shape partial-content checking at Model `0x39c578` includes template margins,
path intersection and containment of the overlap rectangle's center.
Line checking at `0x3875dc` reaches `Path::IsIntersect` at `0x38765c`.
Image checking at `0x420cc8` returns true when image implementation byte 56
is clear. When set, it loads the bitmap, applies supported flips and calls
`ObjectUtil::HasValidPixelInRect` at `0x420dec`. The serialized meaning of
that image flag remains unverified.

Container `GetRect`, Model `0x371fc4`, computes bounds from child handles and
skips invisible children at `0x3720b0`. It is not a direct read of the
serialized parent rectangle. A stored-bbox shortcut would therefore miss
pen margins, rotations, paths, child-derived bounds and partial-content tests.

## Saved container rotations are applied to children

Model `ObjectContainer::SetRotation(float, bool)`, `0x371754`, calls
`ObjectContainerImpl::SetRotation`, `0x374570`, at `0x3717d4`, then updates
its own base rotation. The implementation computes the change from the
current angle at `0x3745f4`, rotates each child's rectangle about the group
center at `0x37466c`, calls the child's virtual `SetRect` at `0x37468c`, and
updates the child's rotation at `0x3746b4`. Model relocations `0x492130`
and `0x492188` identify the setters as `SetRect(RectF, bool, bool)` and
`SetRotation(float, bool)`.

Drawing's type-4 branch at `0x7fccc`–`0x7fd24` recursively dispatches children
on the existing canvas and pen canvas. It applies no inherited parent matrix.
Saved child geometry and angles already reflect this rotation operation;
adding a parent SVG rotation would apply it again.

## SDK scope and remaining work

The Rust renderer preserves ordered roots, containers and render passes.
Native intersection selection remains unimplemented: use typed per-object
selection and partial-content checks before claiming this additional parity.
Retain current leaf transforms and avoid culling on serialized root bounds.

The inspected collector, selection methods and container drawing branch do
not consult `IsOutOfCanvasEnabled` or its base-data byte 63. This establishes
no selection bypass in these paths, not the flag's behavior in every editor
operation or clipping path. Image-flag serialization, detailed path/margin
algorithms, other container edit operations and captured pixel fidelity remain
separate work.
