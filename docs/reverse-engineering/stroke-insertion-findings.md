# Completed stroke insertion and page coordinates

## Evidence and scope

Confirmed by static inspection of Samsung Notes 4.4.45.37 ARM64
`libSPenComposer.so`, `libSPenWDoc.so`, `libSPenModel.so` and
`libSPenBase.so` from the APK identified in the
[knowledge base](README.md#sources-and-validation).

The ordinary pen-action insertion path selects a document adapter from
the note's page mode. Mode 0 selects a page using the stroke's first point,
then translates all recorded points by that page's negative offset.
Mode 1 appends to page 0 without this translation. Both adapters set the
stroke's millisecond flag without rescaling its timestamp array.

This follows the [recording](stroke-recording-findings.md) and optional
[finalization](stroke-finalization-findings.md) stages. It does not recover
the entire screen-to-note input transform or establish the behavior of
callbacks that consume an insertion before the ordinary append.

## The pen action dispatches the completed object

`NoteWritingViewPenAction::addStrokeProcess`, Composer `0x4230ec`, forwards
the supplied stroke to `0x500424`. Signature strings at `0x1f4b05` and
`0x1c3924` identify this helper as
`WritingViewPenAction::addStroke(ObjectStroke*)`.

Before normal insertion, the helper checks the pen name against
`com.samsung.android.sdk.pen.pen.preload.TapePen`, literal `0x1b89af`.
A match calls `ObjectBase::SetRenderLayerId(2)` at `0x500488`. This is a
render-layer assignment, separate from the page's selected physical layer.

An optional callback at action member 400 receives a one-object list
through slot 48 at `0x5004cc`. If it returns true, the helper clears the
drawing state and returns without the normal document append. A false
action flag at member 442 also bypasses that append.

On the ordinary enabled path, the helper gets the drawn rectangle and
intersects a local copy with the action's rectangle at `0x500520`.
Intersection controls a drawing call at `0x50053c`; it does not control
the later append. After clearing the drawing state at `0x50054c`, a
nonnull document at action member 24 receives the original stroke through
virtual slot 16 at `0x500564`.

Successful insertion reaches a document notification at `0x500588` and
an optional action callback at `0x5005a4`. Failure logs an append error.
Those callbacks remain outside the coordinate guarantees below.

## Construction binds page mode to the append implementation

`ContentsView::SetDocument(WNote*)`, `0x417940`, gets the page mode at
`0x417a0c`. The recovered mode-specific constructions are:

| Page mode | Adapter RTTI | Primary vtable | Append slot 16 |
| --- | --- | --- | --- |
| 0 | `NoteWritingWNote` | `0x575e18` | `0x429f54`, forwarding to `0x4f66a4` |
| 1 | `NoteWritingContinuousWNote` | `0x574d40` | `0x4249e0`, forwarding to `0x4ed1e4` |

Mode 0 calls the `WritingWNote` base constructor at `0x417a98`, then
installs the derived vtable at `0x417aac`. Mode 1 calls the
`WritingContinuousWNote` base constructor at `0x417a34` and installs its
derived vtable at `0x417a48`. These assignments resolve through GOT
entries `0x5a20b0` and `0x5a20a0`, respectively. No fallback behavior for
other mode values is inferred here.

The resulting adapter is stored at contents member 2056 and passed to
`NoteWritingView::SetDocument(WNote*, IWritingDocument*)` at `0x417b68`.
That method, `0x425b24`, forwards it to `0x5324c8` at `0x425b50`.
The latter follows view members 728 and 632, then calls `0x50af4c`
at `0x532654`.

When the document changes, `0x50af4c` iterates twenty action pointers
starting at member 696 and calls each nonnull action's slot 88 at
`0x50afa0`. The note pen action occupies member 712, assigned at
`0x425538`; replacing it also forwards an existing document through
slot 88 at `0x425554`. Its slot-88 relocation `0x574b68` resolves to
`0x500268`, which stores the document at action member 24.

This connects the constructed mode-specific adapter to the exact document
pointer used by `addStroke`, rather than relying only on class names or
an unconnected append implementation.

## Mode 0 selects a page from the first stored point

The mode-0 append implementation, `0x4f66a4`, handles object type 1 as a
stroke at `0x4f6828`–`0x4f6834`. It calls `ObjectStroke::GetPoint` at
`0x4f683c`, copies the first X/Y pair, and passes that point to document
slot 360 at `0x4f685c`.

Slot 360, relocation `0x575f80`, resolves to `0x4fa0a0`. This method
queries the document's spatial index through
`SpatialIndexing::SearchIntersectedData(PointF const&, ...)` at
`0x4fa110`. For each returned page it builds a 24-byte record containing:

| Record offset | Value | Evidence |
| --- | --- | --- |
| 0 | Page runtime handle | `0x4fa164`–`0x4fa168` |
| 4 | Index found through the runtime-handle map | `0x4fa174`–`0x4fa184` |
| 8, 12 | Page X/Y offsets | `0x4fa158`, `0x4fa184`–`0x4fa188` |
| 16, 20 | Page width and height | `0x4fa194`, `0x4fa1a4` |

On a successful nonempty query, append takes the first result's page index
at `0x4f6864`–`0x4f6868` and calls document slot 24 with that index and
the original object at `0x4f6810`.

The stroke branch does not select a page from the bounds' center, inspect
all points, or split the stroke across each intersected page. Other object
types have separate geometry-based branches. The spatial index's boundary
inclusivity and result order for overlapping pages remain unresolved.
The first-point read also does not establish a safe empty-stroke contract.

## Mode 0 translates every point into the selected page

Slot 24, relocation `0x575e30`, resolves to `0x4f690c`. It gets the
selected `WPage` at `0x4f697c` and its offset at `0x4f6984`.
WDoc `WPage::GetOffset`, `0xc3854`, returns the two signed integer
coordinates stored at implementation member 172.

For a stroke, the adapter negates those integer offsets at `0x4f6a14`
and `0x4f6a18`, then performs:

| Composer call site | Operation |
| --- | --- |
| `0x4f6a20` | `ObjectStroke::ApplyOffset(-page_x, -page_y)` |
| `0x4f6a2c` | `ObjectStroke::SetMillisecondMode(true)` |
| `0x4f6a40` | Selected page's append slot 32 with the same stroke |

Model `ObjectStroke::ApplyOffset`, `0x2dfc3c`, flushes temporary points
before a nonzero translation. Its direct path calls
`ObjectStrokeImpl::ApplyOffset`, `0x2e91b8`, at `0x2dfec4` and offsets
the object's rectangle at `0x2dfef4`–`0x2dff04`.

The implementation converts both signed integer offsets to floats at
`0x2e91d0`–`0x2e91d4`, loops over the logical point count at member 36,
and adds the offset to every X/Y pair in the vector at member 48.
Its call at `0x2e9208` resolves to Base `PointF::operator+=`, `0xb0ff0`,
whose instruction at `0xb0ff8` adds two float components. It then marks
the object and drawn-rectangle cache dirty.

Thus this stage computes page-local coordinates using float addition of
the negated integer page offset. It preserves point count, stroke width
and the parallel pressure, tilt, orientation and timestamp arrays. The
translation itself does not clip points to page bounds. These statements
describe this adapter and offset operation; they do not imply that earlier
input processing left coordinates or width unchanged.

## Mode 1 uses page 0 and preserves the supplied coordinates

The continuous adapter's append implementation, `0x4ed1e4`, checks object
type 1 and sets millisecond mode at `0x4ed210`. It gets page 0 at
`0x4ed21c` and tail-calls that page's append slot 32 at `0x4ed234`.
There is no page-offset application in this method.

Its indexed variant, `0x4ed238`, also uses page 0 regardless of the
supplied index. It sets the flag at `0x4ed264`, gets page 0 at
`0x4ed270`, and appends at `0x4ed288`.

Model `ObjectStroke::SetMillisecondMode(bool)`, `0x2e135c`, compares and
updates implementation byte 333 at `0x2e13a0`–`0x2e13b0`, then marks the
object changed. It does not traverse or rescale recorded timestamps.
The SDK already exposes the serialized millisecond flag in stroke
metadata; this finding does not require another timestamp conversion.

## Both adapters append through the page's object handler

WDoc vtable relocation `0x103ad8` binds page slot 32 to
`WPage::AppendObject`, `0xc3974`. It ensures page objects are loaded at
`0xc39e0`, then forwards to Model `PageImplBase::AppendObject` at
`0xc39fc`.

The Model method, `0x344ff8`, follows the page's manager at member 144
and object handler at manager member 64. This is the handler bound to
the [selected physical layer](page-layer-selection-findings.md), rather
than a traversal of every physical layer.

`ObjectHandlerBase::AppendObject`, `0x363540`, rejects null or already
attached objects and can record replay metadata before calling
`LayerDocBase::AppendObject` at `0x3636d0`. The latter, `0x33e11c`, wraps
the object in a list. `AppendObjectList`, `0x33e22c`, obtains the existing
object count and passes that count as the insertion position at
`0x33e26c`. Its insertion overloads reach
`ObjectManager::InsertObjectList` at `0x33e574`.

The optional replay-recording helper is distinct from the millisecond
setter and recorded timestamp array. For example, its stroke branch reads
the first and last timestamps at `0x363838`–`0x36384c` to calculate
replay metadata; this read does not rewrite those samples. Full downstream
notifications and callback-driven edits remain separate investigations.

## Validation and SDK implications

The APK digest and all four library byte streams were verified. Adapter
construction, RTTI, append and setter vtable bindings, the first-point
load, integer-offset negation, float-coordinate addition and flag-only
timestamp update were checked against their instructions and relocations.
These are static results; no new device fixture or native execution was
used, and no SDK code changed.

For decoding and page rendering, keep the stored coordinates authoritative:
the mode-0 insertion path has already subtracted the selected page's origin.
Applying this subtraction again during replay would translate the stroke
twice. The adapter's page selection also does not justify splitting an
existing stored stroke at every page boundary.

The [view input trace](view-input-transform-findings.md) resolves child
position and inverse-matrix application before recording. Runtime matrix
configuration and its relationship to pen size remain open. New SDOCX/PDF
pairs containing a stroke across a page boundary, a second-page stroke and
zoomed drawing can test the recovered
page assignment and distinguish input scaling from insertion translation.
