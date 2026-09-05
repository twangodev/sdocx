# Object order and container render selection

## Evidence and scope

Confirmed against Samsung Notes 4.4.45.37, ARM64 `libSPenModel.so`,
`libSPenBase.so`, `libSPenDrawing.so` and `libSPenComposer.so`. The APK
SHA-256 is recorded in the knowledge-base index. These findings trace
serialization, loading, intersection selection and drawing without new
SDOCX/PDF pairs. The extracted libraries were rechecked against their archived
bytes, including WDoc's forwarding layer.

The native paths retain file order within the current physical layer and
retain child order within a container. Standard list-page PDF export then
selects separate render passes. Its top-only intersection query additionally
restricts object types to strokes before applying the render-layer matcher.
This condition was missing from the earlier composition notes.

## Loading preserves the stored sequence

Model `LayerDocLoadHandler::Load_ObjectList_WDoc`, `0x358410`, reads one
object envelope at a time and loads its payload. It inserts the resulting
object at `0x3585b8` through `LayerDocImpl::insertObject`.

The insertion position has two branches:

- At `0x3585a4`–`0x3585ac`, use the current object-list count, appending the
  newly loaded object.
- When the load option is enabled, `0x358598`–`0x35859c` uses the loop index
  plus layer-implementation member 176. This retains sequential positions
  after that prefix; it does not use an object's replay metadata.

`LayerDocImpl::insertObject`, `0x34e608`, forwards the supplied position to
`ObjectList::Insert` at `0x34e624`. `ObjectList::Insert`, `0x2dcfe8`, forwards
to Base `List::Insert`, `0x9dbbc`. That implementation inserts a linked-list
node at the requested position, or calls `List::Add` at `0x9dc84` when the
position reaches or exceeds the count. None of these insertion methods
compares replay values or sorts by object type.

The loader checks and assigns a missing replay order only after insertion,
at `0x3585e0`–`0x358614`. Its loop advances the file-record index at
`0x3586dc`–`0x3586e4`. The separate
[all-layer replay sort](object-drawing-findings.md#replay-order-is-a-distinct-64-bit-value)
does not supply the ordering for this load path.

## Container loading preserves a nested sequence

The type-4 branch at `0x3586ec` calls `ReadObjectContainer_WDoc` at
`0x358720` and rejoins the same insertion path at `0x358724`.

`ReadObjectContainer_WDoc`, `0x358a74`, reads the container's own payload
at `0x358acc`. It then consumes the declared child count in order. Ordinary
children use `ReadDefaultObject_WDoc` at `0x358bc4`; nested type-4 children
recurse at `0x358c1c`. Each successfully decoded child reaches
`ObjectContainer::AppendObject` at `0x358bd4` before the next child is read.
The parent remains one entry in its containing layer or container.

`ObjectContainerImpl::AppendObject`, `0x373974`, appends a runtime handle
to its child vector. When capacity is available, `0x373a90` writes the
handle at the previous end. Its allocation path copies the existing handles
in order and places the new handle after them at `0x373aec`–`0x373b20`.
It binds the child and marks container membership; it does not sort the vector.

`ObjectContainer::GetObject`, `0x36f6b0`, indexes that vector at `0x36f6d8`
and resolves the handle at `0x36f6e0`. `GetObjectCount(true)`, `0x36f758`,
returns its length through `0x36f774`–`0x36f77c`, including hidden children.
The draw dispatcher checks their visibility separately.

## Saving walks the same list and child order

`LayerDocSaveHandler::Save_Objects_WDoc`, `0x3552bc`, walks the layer's
object list with `BeginTraversal`, `GetData` and `NextData` at `0x355398`,
`0x355420` and `0x3555e0`. It writes a type byte, then uses
`WriteObjectContainer` at `0x355504` for type 4 or `WriteDefaultObject`
at `0x355560` otherwise. PDF dummy objects and rejected bound-file checks
have explicit skip paths; the retained records keep traversal order.

The optional `ObjectUtil::GroupObjectStrokes` call at `0x3553d8` does not
construct type-4 containers or reorder entries. Its implementation,
`0x464fa8`, walks the list and sets group IDs on selected strokes at
`0x4650c4` and `0x465100`. This grouping metadata must not be confused
with the physical child envelopes of a container.
`ObjectBase::SetGroupId`, `0x2cfb1c`, allocates a string and generates its
UUID at `0x2cfbec`; its other path clears that string. It does not create
a child list or replace the stroke with a different object type.

`WriteObjectContainer`, `0x354bc8`, writes the parent payload, obtains its
child list at `0x354cc8`, traverses it at `0x354cd4`/`0x354d14` and advances
at `0x354e3c`. Nested containers recurse at `0x354dbc`; ordinary child
payloads are written at `0x354e1c`.

## Top-only selection restricts the object type mask

Model `ObjectManager::FindObjectInRectIntersect`, `0x35e670`, receives the
object-type mask in `w1` and the render-layer filter in `w2`. Before walking
the layer list, it performs this selection:

| Address | Operation |
| --- | --- |
| `0x35e6cc` | Compute `object_type_mask & 1` |
| `0x35e6d0` | Compare the render-layer filter with exactly 2 |
| `0x35e6dc` | Use the reduced type mask when equal, the original mask otherwise |
| `0x35e71c`–`0x35e728` | Test bit `object_type - 1` in the effective type mask |
| `0x35e740` | Check rectangle intersection |
| `0x35e750` | Apply `isMatchLayerFilter` |
| `0x35e760` | Append the accepted object to the output list |

The argument order was checked through the complete Standard caller chain.
Composer supplies `w1 = 0x00ffffff` and `w2 = requested_filter` at
`0x35adb0`–`0x35adb4`. WDoc `WPage` preserves them at `0xc509c`–`0xc50a0`;
Model `PageImplBase`, `ObjectHandlerBase` and `LayerDocBase` forward them
unchanged to this collector.

Type 1 is a stroke, so a query with render filter 2 can return only strokes.
This restriction applies to exactly 2, not to every combined mask containing
bit 1. A query with filter 7 retains the caller's complete type mask.

`isMatchLayerFilter`, `0x35e5cc`, has its own separate rule: a stroke whose
`IsTopLayerPen` result is true matches exactly when render-filter bit 1 is
set, regardless of its common render-layer ID. Other objects, including
non-top-layer strokes, use the bit selected by `GetRenderLayerId`.

For an intersecting object with a known ID and an original type mask that
includes it, the combined query therefore has this truth table:

| Object | Common render ID | Top-layer pen | Base filter 1 | Top filter 2 | Masking filter 4 | Combined filter 7 |
| --- | ---: | --- | --- | --- | --- | --- |
| Stroke | 0 | false | yes | no | no | yes |
| Stroke | 1 | false | no | yes | no | yes |
| Stroke | 2 | false | no | no | yes | yes |
| Stroke | any known ID | true | no | yes | no | yes |
| Text, image, shape, line or container | 0 | n/a | yes | no | no | yes |
| Text, image, shape, line or container | 1 | n/a | no | no | no | yes |
| Text, image, shape, line or container | 2 | n/a | no | no | yes | yes |

These results describe the inspected collector, not which combinations the
editor normally writes. Unknown and negative render IDs remain separate raw
values; AArch64 shift-count masking does not establish semantic aliases.

Both the page-capture top pass and Standard list-page PDF highlighter pass
reach this collector with filter 2 and an otherwise broad type mask. Thus
their top batches contain strokes, not arbitrary objects with common ID 1.
The metadata accessor `ObjectFlexibleMetadata::render_layer()` identifies
the stored ID; it does not promise inclusion in a particular export pass.

## Containers are selected as objects, then draw their children

The intersection collector walks the layer implementation's object list at
member 56. It tests each root and appends that same pointer. There is no
container-child recursion between its type, intersection and layer checks.

Drawing `ObjectDrawing::DrawObjectList`, `0x7f098`, also traverses its supplied
list in order. It gets the current object at `0x7f3a8`, reaches ordinary
`drawObject` at `0x7f4dc`, and advances through `NextData` at `0x7f5e4`.
Alpha branches can use an intermediate bitmap, but retain this traversal.

In `drawObject`, the type-4 branch gets child count at `0x7fcd4`, obtains
child index zero at `0x7fcec`, recursively calls the same dispatcher at
`0x7fd04`, and increments the index at `0x7fd14`. It supplies the existing
canvas and pen canvas. The dispatcher applies common visibility but does
not call `isMatchLayerFilter` for each child or create a separate top pass.

Consequently, for a base container already accepted by this query, its child
draw calls remain inside that container's position in the base pass. A
child's stored top-layer flag does not independently move it into the page's
top batch through this path. This is a control-flow conclusion; the pen
renderer can still apply its own pixel behavior to that child.

## Consequences for the SDK

The high-level SDK page currently separates strokes from other elements and
flattens supported descendants. SVG draws the entire stroke collection before
the element collection. This loses native interleaving and the root/container
boundaries needed to apply the confirmed render selection.

An ordered scene representation needs to retain root order, child order,
container boundaries, visibility and each root's render-selection inputs.
Pass selection must happen before child traversal. Sorting a flattened list
by timestamps, object type or each descendant's render ID cannot reproduce
this pipeline. Group-ID strings also cannot reconstruct container membership.

The existing [Standard PDF trace](standard-pdf-composition-findings.md#ordinary-objects-retain-interleaving-and-flush-the-tail)
establishes image/text flush boundaries and the explicit final bitmap flush.
Those boundaries should be preserved when converting the ordered scene to
SVG/PDF. Top-pass bitmap blending and pen-level opacity remain distinct.

Useful synthetic regressions for that implementation include stroke/image/
stroke, stroke/text/stroke, mixed children inside a visible base container,
hidden containers, and top-only versus combined query masks. New captured
pairs are still required to validate pixels and determine which unusual
render-ID/container combinations occur in editor-generated notes.
