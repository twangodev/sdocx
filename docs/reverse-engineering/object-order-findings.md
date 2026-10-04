# Object order and container render selection

## Evidence and scope

Confirmed against Samsung Notes 4.4.45.37, ARM64 `libSPenModel.so`,
`libSPenBase.so`, `libSPenDrawing.so` and `libSPenComposer.so`. The APK
SHA-256 is recorded in the knowledge-base index. These findings trace
serialization, loading, grouping/ungrouping edits, intersection selection and
drawing without new SDOCX/PDF pairs. The extracted libraries were rechecked
against their archived bytes, including WDoc's forwarding layer.

The native paths retain file order within the current physical layer and
retain child order within a container. Standard list-page PDF export then
selects separate render passes. Its top-only intersection query additionally
restricts object types to strokes before applying the render-layer matcher.

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

## Grouping moves source children and changes root order

The source mutation trace uses Model SHA-256
`4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a`
and Base SHA-256
`e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb`.
Addresses below are in Model except the explicitly named Base UUID constructor.

`PageDoc::GroupObject`, `0x32fe4c`, resolves the current physical layer
at `0x32fe84–0x32fe98`, then reaches `LayerDocBase::GroupObject`,
`0x33ebe0` → `ObjectManager::GroupObject`, `0x35cb90` →
`LayerDocImpl::GroupObject`, `0x34eeb4`. The manager requires more than
one supplied object, rejects duplicate pointers and records their original
root indices through `GetObjectIndex` at `0x35cdb0`. That lookup searches
the current layer's roots, not arbitrary nested children.

The layer constructs an empty type-4 parent at `0x34ef24`; common construction
allocates its UUID at `0x2c9478–0x2c9490`. Base `Uuid::Uuid`, `0xaa964`,
generates it through `uuid_create`/`uuid_make`/`uuid_export`. Each selected
child is detached, removed from the root list and appended as the same
pointer (`0x34efb8–0x34efe0`). On success the children retain their facades,
runtime handles and common UUID objects; the parent independently generates
its UUID rather than copying child identity.

The loop chooses children by ascending original root index
(`0x34ef4c–0x34ef94`), irrespective of supplied-list order. It inserts the
parent at `max_selected_original_index − selected_count + 1`
(`0x34f038–0x34f048`). Unselected roots retain their relative order.
`PageDoc::UngroupObject`, `0x3301f4`, reaches the corresponding layer facade
and manager (`0x33ec3c`, `0x35d304`); the manager requires a root container.
`LayerDocImpl::UngroupObject`, `0x34f110`, obtains its current child pointer
list and inserts the same children contiguously at `group_index + child_index`
(`0x34f130–0x34f1fc`). It does not restore scattered original positions.
Thus `A B C D E`, grouped using `D,B`, becomes `A C [B D] E`; ordinary
ungroup becomes `A C B D E`. This example follows source insertion arithmetic,
not a recorded editor execution. History stores the original index vector
separately; both grouping branches reach the same producer.

Append binds children and sets container membership (`0x373b2c–0x373b70`).
After insertion, the ordinary container attach branch recurses with its final
context and physical layer (`0x3727ac–0x3727c8`); a context callback can skip
recursion at `0x372764–0x372778`. Base attachment writes context `impl+56`,
physical layer `impl+32` and cookie layer `impl+116`. Saved render-layer
selection is separately `common+212`; these common attachment writes do not
replace it with the parent's render layer.

Append computes parent bounds from visible children's rectangles and nearest
cardinal rotation (`CalcRect`, `0x373784`, using `f32`). It sets `impl+66=1`
then calls parent `SetRect(RectF,true)` (`0x373b7c–0x373b94`). The reached
`0x371994` wrapper and four-argument body `0x3719b8` clear this flag and
take `t_SetRectOnlyData` at `0x371aa0`, bypassing the child resize loop.
This parent bounds update does not convert children into local coordinates.

Ungroup defers final release until after child insertion/attachment/binding
(`0x34f164–0x34f20c`), preserving the parent-owned list during the move.
Attachment/detachment still update context resources; common detach can
return before cleanup (`0x2d0110–0x2d0124`). No universal inert-callback or
transactional rollback guarantee is established. The ungroup producer does
not call `ObjectContainerImpl::RemoveObject`, whose separate path clears
membership at `0x374014`; becoming a root alone does not prove that bit resets.

Rust retains ordered container children and typed decoded geometry in
`PageObject`, with source offsets and render-layer metadata. Common UUIDs
decode separately; this tree is not the native manager/history reparenting
lifecycle. The trace does not establish UI grouping, history replay,
save/reload, subsequent connector resolution or appearance equivalence.

## Container XML embeds children and appends fresh objects

This source trace additionally uses `libSPenXmlSerializer.so`, SHA-256
`7be7af380ae378f91c0e565dcc022479c3f87aa965a5190f245f4d006cb6f36a`,
and `libSPenWordDocCoedit.so`, SHA-256
`82a73d24732efe4f5c3c385b9fcb0ccfb05970507ba50039d252c2abb7f0c2af`.
Addresses in this section are in XmlSerializer unless another library is
named. WordDocCoedit's extracted bytes match its APK member.

WordDocCoedit `CoeditObjectContainer` constructor, `0x38ad4`, calls
`ObjectXmlSerializerFactory::CreateObject` at `0x38b08` with literal format
`1`. The factory `0x110e2c` dispatches type 4 to the container constructor
at `0x110ee0`; common initialization selects its Coedit branch for format 1
(`0x11e084–0x11e0b4`). Container `ComposeElement`, `0x10de04`, gets the
current child list and chooses Coedit for format 1, otherwise Sync.

Coedit `composeObjectListCoedit`, `0x10df28`, opens `subObjectList`, writes
`dummy="0"`, and creates a same-format serializer per child before calling
virtual Compose (`0x10dfcc–0x10dfe8`). Common Compose `0x11c648` emits an
`object` element containing that child's attributes/elements; nested
containers recurse. Sync `composeObjectListSync`, `0x10e124`, also calls
child Compose (`0x10e238–0x10e24c`), opening `objectList` lazily. With nonnull
context and `GetSyncMode()==3`, it skips children whose facade byte `+8` is
zero (`0x10e200–0x10e214`). The emitted children retain traversal order;
this raw gate is not assigned an inferred semantic name.

Both lists carry child XML representations rather than UUID-only membership.
That does not establish all original vector channels inline: format-1 stroke
[`strokeBinary` remains gated by its library-global inclusion flag](stroke-metadata-findings.md#xml-and-separately-transported-stroke-binary)
(`0x14ccbc–0x14ccf4`). Container composition does not bypass this child gate.

`ParseElement`, `0x10d6e4`, accepts either list tag and reaches
`parseObjectList`, `0x10d898`. It walks XML siblings, skips non-`object`
elements and empty `type` attributes, converts each nonempty type, and calls
Model `ObjectFactory::CreateObject(type,false)` at `0x10d9bc`. It creates a
same-format child serializer, passes the parent context and calls child Parse
(`0x10d9d0–0x10da08`). Successful children reach `SetAppendByXml(true)` →
Model `ObjectContainerImpl::AppendObject` → clear flag (`0x10da18–0x10da34`).
This constructs and appends fresh objects in sibling order; it does not
resolve container membership through existing UUID targets.

Common parsing can overwrite the fresh child's UUID from XML `id`
(`0x11cc88–0x11cc90`); the same UUID string does not imply pointer reuse.
The list loop has no UUID coalescing, preclear, removal or replacement.
Reached Model append checks runtime handles, not UUID strings. Factory,
Parse or append failure can occur after earlier children were admitted;
this is not an atomic or idempotent replacement of an existing child list.

Unfinished child serializers receive the parent's current child pointer list
then virtual `OnFinishParsing` (`0x10db54–0x10db70`). Both returns are ignored.
Common related-object lookup `0x11e1d0` returns the first UUID match or null;
this deferred reference work is separate from fresh-child membership.
Derived callbacks can fail; cleanup destroys pending serializers rather than
removing admitted children. In WordDocCoedit's included-page
`CoeditNote::InsertObject(int,String&)`, `0x43ebc`, `SetXml` at `0x43f54`
is followed by `insertObject` at `0x43f64` without checking its boolean.
That local caller behavior does not establish server admission rules.

Rust's ordered container tree comes from nested binary records. This trace
does not establish SDK XML/session support, binary source completeness from
XML alone, actual Sync-mode frequency, complete session exchange, or
save/reload behavior after partial parsing.

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

An absent serialized render-layer field uses the constructed Base default.
`ObjectBase::Construct(type, bool)`, `0x2c93a8`, allocates base data and calls
its constructor helper at `0x2c967c`. The store at `0x2c9724` initializes
base-data offset 212 to zero; `GetRenderLayerId`, `0x2d1660`, reads that same
member. The metadata accessor still preserves field absence as `None`.

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

## SDK behavior and evidence limits

The high-level SDK page retains one ordered `PageObject` tree with typed
containers, source offsets and render-layer metadata. Root selection precedes
child traversal; selected children draw in place without independent pass
classification. Hidden recognized objects suppress their subtrees during
decoding. Borrowed stroke and element views refer to the same content.

Rust archive regressions cover mixed strokes/text, nested containers, visibility,
Base/Top/Masking order, top-pen overrides, rejected root IDs and dense replay
indices. [Native intersection selection](object-selection-findings.md) remains
unimplemented; its per-object bounds and content checks cannot be replaced by
stored-bbox overlap. Saved container rotations already update child geometry
and angles, so rendering does not add an inherited parent rotation.
Group-ID strings are not used to reconstruct container membership.

The [Standard PDF trace](standard-pdf-composition-findings.md#ordinary-objects-retain-interleaving-and-flush-the-tail)
establishes image/text flush boundaries and the explicit final bitmap flush.
Top-pass bitmap blending and pen-level opacity remain distinct. The synthetic
regressions do not establish native pixel parity or which unusual
render-ID/container combinations occur in editor-generated notes.
