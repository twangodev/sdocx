# Saved physical-layer selection

## Evidence and scope

Confirmed against Samsung Notes 4.4.45.37, ARM64 `libSPenModel.so`,
`libSPenWDoc.so`, `libSPenBase.so` and `libSPenComposer.so`. Their extracted
bytes were compared with the APK, whose SHA-256 is
`daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667`.

The page loader assigns the serialized current-layer index to the object
handler used by page queries. Standard list-page PDF export copies the
note's existing page pointers and queries that handler. The inspected
loading and export setup paths preserve the separate physical layers;
they do not create a page containing the combined objects of every layer.

This closes the physical-layer setup question left by the
[capture collector](capture-composition-findings.md#clone-state-and-collection-boundaries)
and [Standard PDF investigation](standard-pdf-composition-findings.md#note-preparation-and-layer-scope).
It does not establish every editor operation or export variant. A caller
can change the current layer in memory before invoking these APIs.

## Load all layers, then select by position

Model `PageImplBase::LoadLayer`, `0x346ad4`, reads the collection at the
page's layer offset:

| Model call site | Operation |
| --- | --- |
| `0x346b24` | Read the two-byte layer count |
| `0x346b4c` | Read the two-byte current-layer index |
| `0x346b60` | Clear the existing layer manager |
| `0x346ba0`–`0x346bc0` | Allocate, construct and attach each layer |
| `0x346be4` | Load that layer through page implementation virtual slot 32 |
| `0x346bf8` | Append the loaded layer to the manager's list at member 24 |
| `0x346c04` | Bind the layer instance |
| `0x346c70`–`0x346c78` | Load the saved index and call `List::Get(index)` |
| `0x346c80` | Assign the result to manager member 40 |
| `0x346c84`–`0x346c8c` | Assign the same result to object-handler member 0 |

The native reader sign-extends both two-byte fields. Negative counts are
rejected; an index that produces a null layer fails at `0x346c90`. The SDK
retains the existing unsigned raw fields and rejects zero counts and indices
outside the collection. Its default layer-count limit is 64.

Selection uses a list position, not the layer header's numeric identity.
There is no lookup by layer number, selection of the first nonempty layer,
or merge of sibling layers in this method. Optional post-load cleanup
clears changed flags without replacing the loaded object lists.

WDoc relocation `0x103b50` identifies virtual slot 32 as
`WPageImpl::LoadLayerForChild`, `0xd0848`. It calls `WLayer::Load` at
`0xd0894` on the supplied layer and handles that layer's hash afterward.
This confirms that the loop loads separate WDoc layer records.

## Lazy page loading reaches that selection

`WPage::m_LoadObject`, WDoc `0xc22e8`, checks loaded state at `0xc2300`.
When needed, it invokes `WPage::LoadObject(false, false)` at `0xc2314`.
The boolean overload, `0xc7de8`, takes the model mutex and invokes
`WPageLoadHandler::LoadObject` at `0xc7e3c`.

The handler, `0xd5088`, reads the page header, seeks to its layer offset
and calls `LoadLayer` at `0xd54e0`. It sets loaded state only after success
at `0xd54f4`. `WPageLoadHandler::LoadLayer`, `0xd501c`, forwards through
implementation slot 24 at `0xd503c`; relocation `0x103b48` resolves that
slot to `WPageImpl::LoadLayer`, `0xd07f8`. Its call at `0xd0810` reaches
the Model loader above. The successful wrapper clears changed flags.

The already traced `WPage::FindObjectInRectIntersect` loads page objects
at `0xc5090` before dispatching into the manager and object handler.
The handler uses its assigned layer at Model `0x3659b0`. Consequently,
lazy loading and object collection agree on the saved layer selection.

## Standard PDF keeps the note's page objects

Composer `NotePDFExporterRasterListX::initializeExport`, `0x35a40c`,
passes its array at member 296 to `WNote::GetPageList` at `0x35a488`.
WDoc `WNote::GetPageList`, `0x944f4`, forwards to `ArrayList::Copy` at
`0x94504`, with the note implementation's page array at member 312.

Base `ArrayList::Copy`, `0xd1ba8`, computes the number of eight-byte
entries and copies their storage with `memmove` at `0xd1c48`. It does
not traverse or clone the pointed-to pages. The exporter array therefore
contains the same `WPage` pointers held by the note.

The X delegate's `ExportFile`, `0x3576d4`, creates its page capture and
calls `NoteCapturePage::SetWNote` at `0x357740`. `SetWNote`, `0x3308ac`,
assigns note/body-text pointers and updates width, density and the coedit
font option. It does not replace pages or select or merge physical layers.

The list exporter checks PDF continuity, captures backgrounds and exports
objects from this array. `capturePages` gets a page at `0x35a6f8` and
stores it at member 272 at `0x35a6fc`. Its `captureBackground` reaches
`SetPageContents` at `0x35aa44`, whose page assignment likewise preserves
the supplied pointer. Finally, `getPageObjectList` queries the array's
page through `WPage::FindObjectInRectIntersect` at `0x35adbc`.

Together with the previously inspected Java task and body-text setup,
these paths support using the saved current physical layer for a freshly
parsed page. The three Base, Top and Masking render passes are filters
within that physical layer, not a way to combine physical layer records.

## SDK behavior and validation

The semantic page decoder now traverses the layer at `current_layer_index`.
Strokes, text boxes, images and supported descendants in inactive physical
layers remain available through `StoredPage`; they are not added to the
high-level page or its SVG/PDF output. An empty selected layer produces
no page objects even when another layer contains content. Document body
text and page background/template fields remain independent of this choice.

Structural parsing still visits every layer, retains all object envelopes,
checks their boundaries and reports unknown object types. Object and stroke
count limits include inactive layers and descendants. Semantic diagnostics
and payload decoding apply to the selected layer, so an unsupported object
or malformed stroke in an inactive layer does not affect visible content.
Explicit metadata access remains available for inspecting such records.

Synthetic regressions cover both selected indices, nonsequential layer
numbers, empty selected layers, inactive malformed payloads, invalid inactive
object boundaries, cross-layer stroke limits, text/SVG selection, global
image resolution and retained structural diagnostics. Workspace tests with
all features, Clippy with warnings denied, Rust 1.92 workspace checks, the
WASM target and the existing locked external corpus all passed. The corpus
check used the temporary directory containing the cached reference PDF,
preserving the missing PDF in the `hf` checkout.

That corpus does not supply a captured multilayer comparison. New multilayer
SDOCX/PDF pairs are still needed to measure
fidelity and observe editor operations that change the selected layer.

This change does not infer physical-layer opacity or visibility composition
from the metadata. Ordered object rendering, pen behavior and single-page
PDF segmentation remain separate APK investigations.
