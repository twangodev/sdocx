# Object drawing visibility and dispatch

## Evidence and scope

Confirmed against Samsung Notes 4.4.45.37, ARM64 `libSPenDrawing.so` and
`libSPenModel.so`. The APK digest is recorded in the knowledge-base index.
These findings come from native control flow and vtable relocations; no new
SDOCX or Samsung PDF captures were available.

## Common object visibility

`ObjectDrawing::drawObject` begins at Drawing `0x7fa24`. It calls
`ObjectBase::IsVisible` at `0x7fa94`. A false result branches at `0x7fa98` to
`0x7fb70`, which sets the return value to true and exits through `0x7fb74`–
`0x7fba0`. The object-specific renderer is never called on that branch.
Here, success means the draw request was handled; it does not imply pixels
were painted.

This check precedes the type dispatch and container traversal. The jump table
at `0x52464` contains 24 unsigned 16-bit offsets, indexed by object type minus
one. Each destination is `0x7fae0 + 4 * offset`. Type 4 resolves to `0x7fccc`:

| Address | Operation |
| --- | --- |
| `0x7fcd4` | `ObjectContainer::GetObjectCount(true)` |
| `0x7fcec` | `ObjectContainer::GetObject(index)` |
| `0x7fd04` | Recursive `ObjectDrawing::drawObject` for that child |
| `0x7fd14`–`0x7fd20` | Advance the index and check the count |

A hidden container returns before this loop. A visible container dispatches
each child through the same visibility check. A hidden child does not stop
the remaining siblings from being visited.

Serialized type-0 property bit 3 supplies `IsVisible`. Its decoder and getter
are independently mapped in [common object findings](object-base-findings.md).
Editing flags such as locked, selectable and removable do not replace this
visibility gate.

`DrawObjectList` at `0x7f098` reaches this dispatcher through its ordinary path
at `0x7f4dc`, its intermediate-alpha path at `0x7f43c`, and its selected-object
path at `0x7f488`. The last path therefore does not override `IsVisible`.

## Separate drawing conditions

There are additional conditions whose serialized or runtime inputs need
separate treatment:

- Types 2 and 7 first call `ComponentText::IsTextVisible` at `0x7fa88`.
  False branches through a log message at `0x7fb44` and reaches the same
  successful early return. This is a separate condition from common visibility.
- After common visibility, vtable slot 472 is called at `0x7faa8`. The
  `ObjectBase` relocation at Model `0x4922e0` resolves that slot to
  `IsAllContentFileAvailable`, `0x2d45dc`. False takes a separate fallback
  drawing branch at `0x7fba4`; it is not another hidden-object test.
- Object alpha is checked in `DrawObjectList` and the public single-object
  `DrawObject`, before the common dispatcher. The list path has selection
  and drawing-state conditions at `0x7f454`–`0x7f488`. These runtime alpha
  branches do not establish a serialized opacity field.
- Type 21 resolves to `0x7fe18`, obtains `ObjectMath::GetFormulaList`, and
  calls `drawObjectFormula` directly at `0x7fe80`. The embedded formula loop
  does not recursively call the common dispatcher. Do not generalize the
  container-child rule to every embedded formula or stroke list.

The last distinction supplements [formula drawing findings](formula-rendering-findings.md).
The math object's own common visibility is checked before its formula loop.

## Layer collection is a different operation

Model `LayerManagerBase::GetAllLayerObjectList(bool)` at `0x34a28c` traverses
layer object lists. At `0x34a470`, a false argument selects the
`ObjectBase::IsVisible` check at `0x34a4c0`; hidden objects advance to the next
entry at `0x34a58c`. A true argument bypasses that visibility check and includes
the object. This parameter is effectively an include-hidden option.

The method does not call `LayerDocBase::IsVisible`. Its output is also sorted:
the callback loaded at `0x34a358`–`0x34a35c` resolves through relocation
`0x4a3de8` to `sm_SortObjectByReplayOrderASC`, `0x34a65c`. The comparator and
its relation to final paint order need further investigation.

Layer visibility has its own inverted property bit, documented in
[layer findings](layer-findings.md). This collection method alone does not
prove how layer visibility is applied to a final page export. Layer compositing,
transparency, alpha lock and shadows remain separate rendering work.

## SDK behavior and validation

Page decoding omits a recognized object's semantic content and child subtree
when its complete supported common metadata decodes successfully with
`visible == false`. This affects strokes, images, standalone text, shapes,
lines and recognized containers. The physical `StoredPage` tree, original
payloads and integrity trailers remain accessible. Declared object, nesting
and stroke-count limits still include hidden records.

Unknown outer types retain the existing child traversal policy. A coincidental
type-0-looking payload does not establish their inheritance or visibility.
Unreadable common metadata also leaves the existing decoder path in place:
malformed supported objects still report their decoding error, and opaque
unsupported parents can still retain accessible children.

Hidden content is omitted before media resolution and semantic diagnostics.
For example, a hidden image with an unavailable media ID produces neither an
image element nor an unresolved-media warning. Structural diagnostics and
explicit metadata inspection still apply to the retained records.

Synthetic archive tests cover hidden leaves, hidden containers with visible
children, visible containers with mixed children, unchanged stored payloads,
unknown-parent fallback, malformed supported bases, and resource accounting
across hidden subtrees. Separate image, text, shape and line cases exercise
their semantic paths; SVG checks verify that hidden text and images stay out
of exports. Existing rendering fixtures now set the native visible bit in
their common frames.

Validation passed for workspace tests with all features, Clippy with warnings
denied, Rust 1.92 workspace checking, and the WASM target. The existing
`01-basic-formatting` corpus also passed its locked hashes and parser/layout
expectations. Its reference PDF was read from the local LFS cache into a
temporary corpus directory because the `hf` checkout's PDF was absent. This
existing formatting fixture does not establish hidden-object visual parity.

New Samsung captures should include visible and hidden versions of the same
objects and containers, hidden layers, and overlapping objects across layers.
They are needed to validate visual parity and the remaining collection and
compositing behavior.
