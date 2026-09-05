# Page capture composition

## Evidence and scope

Confirmed against Samsung Notes 4.4.45.37 ARM64 `libSPenComposer.so`,
`libSPenModel.so` and `libSPenGraphics.so`. Addresses below belong to Composer
unless marked Model or Graphics.
These are native capture-path findings. No new SDOCX or Samsung PDF captures
were available to validate the resulting pixels.

## Capture sequence

`NoteCapturePage::drawPage`, `0x32dd44`, checks page readiness and bitmap
creation before drawing. For capture mode 0 its call sequence is:

| Call address | Operation |
| --- | --- |
| `0x32ddd4`, `0x32dddc` | Update background settings, then draw background |
| `0x32de5c` | Draw template |
| `0x32de6c` | Draw PDF background |
| `0x32de7c` | Draw body text |
| `0x32de88` | Draw sticky memo |
| `0x32dee0` | Draw objects |

These are calls, not a guarantee that each category paints pixels. The
individual methods have their own content and setting checks. Other capture
modes can skip categories: `0x32de48`–`0x32de50` permits the template through
sticky-memo group only for modes 0, 1 and 3.

`drawObject`, `0x32e700`, returns when its object-enabled flag is false or
`WPage::GetObjectCount(true)` is zero. It then dispatches these passes:

| Pass | Method | Dispatch call | Capture modes | Object layer filter |
| --- | --- | --- | --- | --- |
| Base | `0x32ff58` | `0x32e7e0` | 0, 1, 4 | `1`, supplied at `0x32ffb0` |
| Top | `0x3300cc` | `0x32e7fc` | 0, 2 | `2`, supplied at `0x330128` |
| Masking | `0x33036c` | `0x32e82c` | 0, 5 | `4`, supplied at `0x3303c4` |

Mode 0 therefore draws base objects, top objects, then masking objects.
Capture-mode numbers and object render-layer IDs are different domains.

## Per-object render-layer selection

Each pass calls `NoteCapturePage::getCloneObjectList`, `0x330494`. At
`0x3304f4` it calls the `WPage::FindObjectInRectIntersect` overload that takes
both a type mask and a layer filter.

Model `ObjectManager::isMatchLayerFilter`, `0x35e5cc`, supplies the render-ID
part of that selection. For ordinary objects it calls `ObjectBase::GetRenderLayerId`
at `0x35e60c`, shifts the filter right by that ID at `0x35e610`, and tests
bit 0 at `0x35e614`. Combined with the capture pass names and masks, this
establishes:

| Stored render-layer ID | Render-filter bit |
| ---: | --- |
| 0 | Base |
| 1 | Top, subject to the collector's stroke-only restriction |
| 2 | Masking |

These IDs come from common type-0 flexible field 21, decoded as a signed
32-bit value. They do not identify the physical stored layer containing the
object. AArch64's variable shift masks its shift count; that incidental
machine behavior is not evidence that other or negative IDs are canonical
aliases for these three values.

Strokes have an override. The filter checks type 1 at Model `0x35e5ec` and
calls `ObjectStroke::IsTopLayerPen` at `0x35e5f8`. A top-layer pen is accepted
when filter bit 1 is set, and rejected otherwise, without consulting its
common render-layer ID. Non-top-layer strokes follow the ordinary ID test.
Consequently, sorting solely by the stored render-layer ID cannot reproduce
this selection.

The top-layer pen flag is serialized stroke property bit 6. Model
`ObjectStrokeBinaryHandler::m_ApplyBinary_Property`, `0x2ed138`, extracts that
bit at `0x2ed174` and stores it at stroke-data offset 341 at `0x2ed180`.
`IsTopLayerPen`, `0x2e1ea0`, reads the same byte at `0x2e1ea8`. The property
writer reads it at `0x2ec104` and sets mask `0x40` at `0x2ec110`.

`ObjectFlexibleMetadata::render_layer()` exposes the three names through
`ObjectRenderLayer::{Base, Top, Masking}`. Unknown signed values remain
`Other(i32)`, absent fields remain `None`, and `render_layer_id` retains its
original value. The accessor identifies the stored field; it does not apply
the stroke override or choose a physical layer.

The surrounding intersection collector has an additional type gate. In
`ObjectManager::FindObjectInRectIntersect`, `0x35e670`, instructions
`0x35e6cc`–`0x35e6dc` reduce the object-type mask to bit 0 when the render
filter is exactly 2. The type test uses bit `type - 1`, so only strokes can
enter this top-only query. Combined filter 7 keeps the original type mask.
Thus a non-stroke object with common render ID 1 passes the helper's Top
bit test but is excluded by the earlier type test. See
[object order and container selection](object-order-findings.md#top-only-selection-restricts-the-object-type-mask)
for the complete decision table.

The same collector selects root objects without traversing container children.
The draw dispatcher later visits those children in their stored order on the
existing canvas, without repeating page render-pass selection. An ordered
renderer must retain these container boundaries.

## Top-pass composition

Base and masking objects use the destination bitmap directly. The top pass
creates an intermediate bitmap at `0x3301bc`, using the destination's width
and height, and draws its object list into that bitmap through the renderer's
virtual slot 16 at `0x330200`. It then composites the intermediate bitmap
onto the destination through canvas slot 312 at `0x3302c4`.

Before composition, `SPPaint::SetXFermode` at `0x330270` receives native mode
16 or 17. A page with a PDF selects 16 at `0x330214`. Otherwise, the capture
settings convert the page background color at `0x33024c`, and
`Color::IsDarkColor` tests the result at `0x330258`: dark selects 17, the
other branch selects 16.

The graphics implementation establishes the actual operations:

| Paint mode | Shader mode | Operation on unpremultiplied source and destination RGB |
| ---: | ---: | --- |
| 16 | 1 | Component-wise minimum: darken |
| 17 | 2 | Component-wise maximum: lighten |

Graphics `SPPaint::SetXFermode`, `0xbdafc`, stores the mode at paint offset
44. `SPBitmapDrawable::DrawBitmapRT`, `0x95f24`, reads that member at
`0x9632c`. Its test at `0x96370`–`0x96378` selects the advanced branch for
exactly modes 16 and 17. At `0x96504`–`0x96518`, mode 16 becomes shader
value 1 and mode 17 becomes 2. The value is passed at `0x9651c` to the
uniform object at shader member 56.

The drawable initializes its shader member 48 at `0x951ec`–`0x951f4` through
helper `0x954e0`. That helper constructs `SPBitmapAdvancedBlendingShader`
at `0x9556c`. The shader constructor's program inputs resolve through
relocations `0xd7280` and `0xd7288` to its exported vertex and fragment
sources, including the fragment source at `0x56fce`.

`SPBitmapAdvancedBlendingShader` construction associates member 56 with the
string `uBlendingMode` at `0xc4a84`–`0xc4a94`; the string is at `0x4ccaf`.
The exported fragment-shader source at Graphics `0x56fce` uses component-wise
minimum for uniform value 1 and maximum for value 2. This resolves the
mapping from the capture paint mode to the shader equation.

The shader first applies paint alpha, edge coverage and optional tint, then
unpremultiplies source and destination RGB. If their alphas are `As` and
`Ad`, and their unpremultiplied colors are `Cs` and `Cd`, its output is:

```text
overlap = As * Ad
source_only = As * (1 - Ad)
destination_only = Ad * (1 - As)
output_rgb = blend(Cs, Cd) * overlap
           + Cs * source_only
           + Cd * destination_only
output_alpha = overlap + source_only + destination_only
```

The output RGB is premultiplied. For example, an opaque source channel 0.8
and destination channel 0.4 produce 0.4 with darken and 0.8 with lighten.
With source alpha 0.5 and destination alpha 1, those channels produce 0.4
and 0.6 respectively, both with output alpha 1. This establishes the blend
math for this GPU path; texture sampling, edge coverage, color conversion
and pen rasterization still need separate conformance checks.

## Clone state and collection boundaries

`getCloneObjectList` walks the returned list and appends clones in that order
at `0x330600`. Strokes use `CloneDrawData` at `0x330550`. Other objects use
`ObjectFactory::CreateObject` and the copy virtual at `0x330568`–`0x330584`.
For selected originals, the clone receives alpha 1 at `0x3305c4`. If such a
clone is type 2 or 7, `ComponentText::SetTextVisibility(true, true)` runs at
`0x3305f4`. This is additional evidence that component text visibility
includes runtime editing state; it is separate from the common serialized
object visibility check in [object drawing findings](object-drawing-findings.md).

Model `ObjectManager::FindObjectInRectIntersect`, `0x35e670`, traverses its
existing object list, checks intersection at `0x35e740`, applies the layer
filter at `0x35e750`, and adds matches at `0x35e760`. This is a different
collection path from `LayerManagerBase::GetAllLayerObjectList`, whose replay
order comparator is documented separately. The intersection collector does
not itself establish a replay-order sort or visible-layer traversal.

The intervening path uses the currently assigned physical layer:

| Library/address | Operation |
| --- | --- |
| WDoc `WPage::FindObjectInRectIntersect`, `0xc5054` | Load objects at `0xc5090`, then forward to the page implementation at `0xc50cc` |
| Model `PageImplBase::FindObjectInRectIntersect`, `0x345250` | Load its layer manager at member 144 and that manager's object handler at member 64 |
| Model `ObjectHandlerBase::FindObjectInRectIntersect`, `0x365944` | Load the handler's assigned layer from member 0 at `0x3659b0`, then call the layer at `0x3659c4` |
| Model `LayerDocBase::FindObjectInRectIntersect`, `0x33f1a0` | Forward to the layer's object manager at `0x33f1ac` |

Model `PageImplBase::GetLayerManager`, `0x3468d0`, independently identifies
member 144. `LayerManagerBase::m_SetCurrentLayer`, `0x3476dc`, stores the
same layer pointer in manager member 40 and handler member 0 at
`0x3476e8`–`0x3476ec`. Thus this intersection query is scoped to the current
physical layer, while its filter chooses a render pass within that layer.
The subsequent [saved physical-layer investigation](page-layer-selection-findings.md)
traces the loader's assignment of the serialized current-layer index and
Standard list-page export's reuse of the note's existing page pointers.
Those inspected setup paths preserve the separate physical layers. The SDK
now constructs semantic page objects from the saved current layer while
retaining all layers in its structural representation.

SDK exports also need an ordered object representation and stroke render
properties before this evidence can support a complete composition change.
New captures should combine body text, ordinary strokes, highlighters, masking and
overlapping objects across physical layers, with light, dark and PDF
backgrounds.

## Vector PDF export uses a separate collection path

`NotePDFExporterFactory::CreateNotePDFExporter`, `0x360610`, chooses the
raster exporter when the low 32 bits of its option are 0, constructing it
at `0x360674`. Option 1 constructs `NotePDFExporterVector` at `0x360654`.
These are distinct implementations; the capture pass sequence above cannot
be assumed to describe every PDF export.

The normal Java Standard PDF option passes native factory type 0 with
different editable/system-font flags, selecting `NotePDFExporterRasterListX`
for list-page notes. Its call chain and paint sequence are now mapped in
[Standard PDF composition findings](standard-pdf-composition-findings.md).
The native `VectorList` implementation described below is a separate path.

The vector list-page implementation calls `WNote::GetPageList` at `0x3618b0`,
gets a `WPage` at `0x3618f0`, and saves it in exporter member 176 at
`0x361998`. `NotePDFExporterVectorList::exportPage`, `0x361aac`, calls
`exportBackground`, `exportBodyText` and `exportObjects`, in that order,
at `0x361aec`, `0x361af8` and `0x361b04`.

`exportObjects`, `0x361d7c`, calls the page's virtual slot 48 at `0x361dc4`.
This slot is resolved by the WDoc `WPage` vtable relocation at `0x103ae8`
to `WPage::GetObjectList()`, `0xc4450`. That method loads objects and calls
Model `PageImplBase::GetObjectList`, `0x3450e4`. The implementation follows
the same manager/handler/current-layer members described above and calls
`LayerDocBase::GetObjectList`, `0x33e94c`. The layer returns its object
manager's existing list at `0x33e954`–`0x33e960`.

This path does not call the replay-sorted all-layer collector or the
intersection collector used by `NoteCapturePage`. It obtains the current
physical layer's whole object list through a separate API. Higher-level
note preparation and any mutation of list order remain separate questions.

Within the vector export loop:

| Address | Operation |
| --- | --- |
| `0x361e3c`–`0x361e50` | Read the next object and test whether its type is 1 |
| `0x361e5c` | Add a stroke to a temporary object list |
| `0x361e64`–`0x361ec8` | Accumulate the stroke bounds and advance a stroke counter |
| `0x361ed4`–`0x361f04` | If the temporary list is nonempty, create its list PDF exporter |
| `0x361fcc`, `0x361fe4` | Invoke that exporter's virtual slot 16, then clear the temporary list |
| `0x36201c`–`0x36204c` | For a non-stroke object, create its individual PDF exporter |
| `0x362104`, `0x362190` | Invoke the individual exporter's slot 16, then advance the source list |

A non-stroke object therefore triggers export of preceding accumulated
strokes before its individual export. This establishes an export sequence
that can interleave strokes with other content. The list-exporter factory
creates `ObjectStrokePdfExporter`, which draws each exported batch into a
bitmap and embeds it as a PDF image. The factory, coordinate scaling, PNG
handoff and distinct opacity mechanisms are now traced in
[native PDF stroke findings](native-pdf-stroke-findings.md). Pen-specific
blending and higher-level export preparation remain separate questions.

The tail condition also needs separate treatment. At `0x361ebc`–`0x361ecc`,
the loop increments its stroke counter and compares it against the original
total object count. The ordinary iterator-end path reaches list destruction
at `0x3621bc`–`0x3621c0`. These instructions alone do not establish correct
flushing for every mixed list ending in strokes. Do not generalize the
observed non-stroke-triggered flush into an unconditional final flush without
checking list preparation or runtime behavior.

The Standard X implementation has its own explicit final bitmap flush at
`0x35b99c`. The unresolved condition above does not establish a defect in
that public export option.

The raster list exporter reaches capture directly: `capturePage`,
`0x3594a8`, passes its page into `NoteCapturePage::SetPageContents` at
`0x359554`, then calls `saveThumbnailByPage` at `0x3595c0`. That helper
calls `NoteCapturePage::CapturePage` at `0x356e5c` with the supplied layer
mode. `SetPageContents`, `0x330758`, assigns the page and body-text inputs
and updates document width/density; it does not itself merge physical layers.
This narrows where any flattening or current-layer changes must occur, but
does not establish how every raster or vector export variant is prepared.
