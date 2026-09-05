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

Model `ObjectManager::isMatchLayerFilter`, `0x35e5cc`, resolves the meaning
of that filter. For ordinary objects it calls `ObjectBase::GetRenderLayerId`
at `0x35e60c`, shifts the filter right by that ID at `0x35e610`, and tests
bit 0 at `0x35e614`. Combined with the capture pass names and masks, this
establishes:

| Stored render-layer ID | Capture pass |
| ---: | --- |
| 0 | Base |
| 1 | Top |
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
This does not yet prove whether a higher-level export caller changes the
current layer or prepares a flattened page before capture. That setup must
be traced before restricting SDK exports to a single stored layer.

SDK exports also need an ordered object representation and stroke render
properties before this evidence can support a complete composition change.
New captures should combine body text, ordinary strokes, highlighters, masking and
overlapping objects across physical layers, with light, dark and PDF
backgrounds.
