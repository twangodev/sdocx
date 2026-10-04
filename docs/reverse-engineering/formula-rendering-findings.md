# Native formula drawing

## Evidence

Analyzed Samsung Notes 4.4.45.37, APK SHA-256
`daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667`.
The following ARM64 traces establish drawing decisions.
Addresses in the first two sections are in `libSPenDrawing.so`.

The drawn-bounds producer/helper findings below are static source traces, without
native execution, new geometry captures or device appearance validation. Inspected
ARM64 copies match these APK entries byte for byte:

| Library | SHA-256 |
| --- | --- |
| `libSPenModel.so` | `4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a` |
| `libSPenBase.so` | `e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb` |
| `libSPenDrawing.so` | `788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd` |

## Image and stroke precedence

`ObjectDrawing::drawObjectFormula` at `0x817b8` first calls
`ObjectFormulaDrawing::DrawObject` at `0x81818`. A true return skips both stroke
lists at `0x8181c`. Otherwise, it draws source strokes in list order, then answer
strokes in list order. The source-list getter is called at `0x81840`, its stroke
draw at `0x818a0`, the answer-list getter at `0x818d8`, and its stroke draw at
`0x81938`.

`ObjectFormulaDrawing::drawFormula` at `0x83888` requires an image path and a
retrievable bitmap. `isFormulaImageExist` at `0x83960` asks
`ObjectFormula::GetLatexImagePath` and rejects null/empty paths.
`getFormulaImage` at `0x83994` prefers `GetCacheImage`; if absent, it asks
`GetLatexImage`. Both failures produce a false availability result.

The success returned by `drawFormula` records bitmap retrieval, not successful
raster upload or painting. The flag is saved at `0x838d4` and returned at
`0x8393c`; the result of `drawFormulaBitmap` at `0x83914` does not replace it.
Thus the native path can suppress ink even if a later graphics allocation
fails.

This drawing path does not evaluate LaTeX strings. Stored expressions, answer
text and label graphs are recognition/editing data; the visible representation
comes from the image or the embedded stroke lists. Drawing both representations
unconditionally would duplicate formula content.

## Image rectangle and cache

`drawFormulaBitmap` at `0x83b7c` retrieves the formula's LaTeX result rectangle
at `0x83bdc` and passes it as the destination rectangle to a canvas virtual call
at `0x83c78`. Before that draw, it retrieves the formula's drawn rectangle
at `0x83bec` and translates the canvas by that rectangle's top-left coordinates
at `0x83c20`. Canvas state is saved at `0x83c08` and restored at `0x83c8c`.

`getFormulaDrawnRect` at `0x8423c` invokes object virtual slot 160, then subtracts
the drawing origin supplied by `SetPos` from all rectangle coordinates.
`SetPos` stores that origin at drawing-data offsets 16 and 20 (`0x8373c`);
the subtraction occurs at `0x84280`–`0x84294`. In `libSPenModel.so`, the
`ObjectFormula` vtable at `0x497e50` resolves slot 160 to
`ObjectFormula::GetDrawnRect`, through the relocation at `0x497f00`.
Slot 168 is separately `ObjectBase::GetRect` (`0x497f08`).

`getResizedFormulaBitmap` at `0x83f64` allocates a temporary bitmap from the
LaTeX result rectangle's width and height, converted to integer dimensions.
It draws the complete source bitmap into a zero-origin rectangle of those
dimensions through `drawBitmap` at `0x84110`. This path does not read the
formula's nine-patch rectangle. That does not establish whether other rendering
paths or versions use the field.

Transient preview state changes alpha to 76/255 in the resized bitmap at
`0x841ac`–`0x841b4`. The state is time-dependent (`isPreviewState`, `0x83e2c`)
and is not a persisted formula appearance flag.

Image placement depends on the enclosing object/canvas transform and
pen-dependent drawn bounds. The stored base rectangle alone is not a proven
substitute for the native drawn rectangle.

## Visible-stroke bounds

These addresses are in `libSPenModel.so`.

`ObjectFormulaImpl::GetDrawnRect` at `0x4325f4` refreshes its cached rectangle
using `GetRectByStrokeList(true)` (`0x432614`–`0x432618`).
`GetRectByStrokeList` at `0x431e4c` unions the source list at implementation
offset 88 and the answer list at 104. The union helper at `0x4324a4` calls
`ObjectBase::IsVisible` at `0x432520` and excludes invisible strokes. With its
boolean argument true it uses object virtual slot 160 (`GetDrawnRect`); false
selects slot 168 (`GetRect`).

`ObjectStrokeImpl::GetDrawnRect`, `0x2e9488`, first checks the refresh flag
(`0x2e94b4`); otherwise it returns cached bounds at implementation `+364–376`.
On refresh, it expands nonrotated bounds according to category `+328` and pen
size `+292`. This category is derived from the pen name, not the serialized
stroke type or tool type. `SetPenName` calls `SetPenType` at `0x2e8a34` before
storing the name/reference. On attach, successful saved-name-ID binding resolves
the string and calls `SetPenType` (`0x2e6670/0x2e667c`). Its string comparisons
at `0x2ea3c0–0x2ea4f4` select the categories below; a changed category requests
drawn-bound refresh at `0x2ea538`. Names share the full prefix
`com.samsung.android.sdk.pen.pen.preload.`; suffixes are literal name matches.

| Native category | Matched name suffixes | Expansion scalar per side |
| --- | --- | --- |
| 0, 4, 8, 9, 12 | Marker, BrushPen, InkPen, ObliquePen, TapePen, respectively | `size + 4` |
| 1 | Beautify, Beautify2 | `size * 0.5 + 20` |
| 3, 5 | Crayon/Crayon2, Pencil2, respectively | `size * 2 + 4` |
| 6, 7 | Pencil3, PatternPen, respectively | `size * 9 + 4` |
| 10 | OilBrush3 | `size * 0.5 * f32(1.3) + 4` |
| 11 | ColoredPencil | `size * 35 + 4` |
| Remaining values | Null/unmatched name selects category 2 | `size * 0.5 + 4` |

OilBrush3's literal is at `0x13ca52`; its comparison at `0x2ea4dc` selects 10
at `0x2ea57c`, stored to `+328` at `0x2ea510`. Category 10 first multiplies
size by f32 `0.5` (`0x2e9554`), then performs f32 fused multiply-add with
`0x1644fc` plus 4 (`0x2e955c`). That constant is bits `0x3fa66666`, exactly
`1.2999999523162842`; a real-number formula does not imply device bit parity.

The expansion call is `0x2e9604`. Base `RectF::IncreaseRect`, `0xb15c0`, maps
`(left, top, right, bottom)` to `(left-s, top-s, right+s, bottom+s)` in f32,
without validity normalization. The scalar applies to each side, not the whole
width/height. Nonzero rotation subsequently uses `GetRotatedBound` at
`0x2e9660`. These cached drawn bounds are separate from the stroke silhouette.

Formula image placement uses this visible-stroke union's drawn origin, as traced
above. [Capture selection](object-selection-findings.md#selection-depends-on-object-content)
has a different stroke rectangle: `getSelectionRect`, `0x2e6d9c`, reads `GetRect`
and expands by **half pen size** (`0x2e6de8/0x2e6dec`), then rotates. It does not
use the category margins above. Pen-dependent drawn bounds, query selection,
and original vector geometry therefore need distinct representation.

## Embedded-vector coordinate authority

These static Model/Drawing paths establish dispatch, not executed mutation,
loading or appearance; embedded coordinates remain distinct from the formula box.

Formula `SetRect(RectF,bool,bool)` (`0x42c1d8`) has distinct branches. With the
first boolean false, old/new signed width and height must differ by at most
native f32 `0.01` (`0x42c318`–`0x42c35c`); accepted movement passes the f32
left/top delta to `OffsetStrokeList` (`0x42c3ac`). That helper offsets each source
and answer stroke rectangle, then calls its actual slot40
`SetRect(RectF,false,forwarded_second_bool)` (`0x433b08`, `0x433bc0`). There is
no visibility gate in these loops. Stroke SetRect reaches the already documented
[sample-mutating paths](object-transform-findings.md#stroke-edits-update-samples-that-the-modern-writer-actually-serializes).
With the first boolean true, Formula bypasses those checks and traversal and
calls inherited Base three-boolean SetRect (`0x42c294`–`0x42c2bc`).

Math `SetRectDataOnly` (`0x4587e8`) uses requested left/top displacement,
ignoring requested right/bottom as scale inputs. `OffsetChildObjectList`
(`0x45b5e8`) offsets formula children at +16 and separately referred strokes at
+136 through slot40 with `[false,true]`, then recomputes visible child bounds
plus margins (`0x45b074`–`0x45b094`). Connected plots are a different list at +88
(`0x45bd74`); +136 is identified by `GetReferredStrokeList` (`0x459620`).
Its [runtime pointer lifecycle and separate saved recognition UUID groups](math-findings.md#referred-strokes-and-recognition-identity)
are distinct; a saved-record bridge into the referred list remains unproven.
Ordinary Math SetRect first rejects undersized destinations; after that check,
its first-boolean-true branch logs and returns success without offset/base edit
(`0x4585a0`–`0x4585f4`).

Inherited dispatch differs again. Runtime slot48 is Base three-boolean SetRect
for Formula and Math (relocations `0x497e90`, `0x4992e0`), so the container's
normal child-slot48 call bypasses their slot40 offset overrides. Formula slot480
is inherited Base SetRectDataOnly (`0x498040`); Math slot480 is its own override
(`0x499490`). Both inherit Base SetRotation (relocations `0x497ed8`/`0x497ee0`,
`0x499328`/`0x499330`), which changes angle metadata without direct embedded-list
rotation. Base rectangle/angle setters still have callbacks and history;
absence of direct traversal does not certify every callback side effect.

The formula nested writer delegates complete child size/write methods
(`0x42f560`, `0x42f5ac`) without subtracting the formula origin or undoing its
angle. Its reader creates actual strokes and delegates each modern child reader
(`0x43073c`–`0x4307b0`); Math similarly creates formulas and delegates their
modern readers (`0x45a7c0`–`0x45a834`). The calls pass declared sizes, orientation
and an integer argument, rather than the enclosing own-reader's float scale.
Formula result/original rectangles separately narrow and scale their saved
f64 coordinates (`0x42fe54`–`0x42fe80`, `0x430004`–`0x430098`). No formula-box
rebase of embedded samples is visible.

Drawing dispatch passes the existing canvases to formulas (`0x7fde0`–`0x7fe14`)
and to Math's formula list and separate referred-stroke pass
(`0x7fe1c`–`0x7ff2c`). Its formula ink loops apply no enclosing formula/math local
matrix before stroke drawing (`0x818a0`, `0x81938`). Result/relative-original
rectangles and base angles therefore do not justify an added SVG parent transform.

Formula Copy creates fresh strokes and delegates their Copy
(`0x4329f0`–`0x432a2c`); Math Copy similarly creates fresh formulas
(`0x45b494`–`0x45b4ec`), without formula-origin displacement in those copy loops.
OnAttach forwards shared context/layer to embedded strokes and conditionally
registers ImageCommon (`0x42ad4c`, `0x42add0`, `0x42ae28`). Stroke attachment
resolves pen/settings IDs through that context's StringIDManager and may clear
unresolved IDs (`0x2e664c`–`0x2e66f4`).

Rust FormulaStroke retains nested bytes, base metadata and decoded sample/style
channels. Its pen/settings strings remain unresolved until existing public
StrokeResources resolves them; formula inspection does not call that resolver
or resolve the image. Retained vectors do not certify native appearance.

## Expression-type constraint

`ObjectFormula::SetExpressionType` at `0x42a134` in `libSPenModel.so` rejects
unsigned values greater than or equal to 2 at `0x42a178`–`0x42a17c`. Accepted
values are stored at implementation offset 288. The names of 0 and 1 are not
established by this setter, so the inspection API retains `expression_type_raw`.
The serialized reader accepts the stored value without this setter's check.
The editing API's range restriction is not a serialized-format invariant.

The similarly named `HwrMathExpression::SetExprType` in `libSPenHwrData.so`
stores a 16-bit value at offset 184 (`0x3a998`). Its `IsAssign` method reads a
different 32-bit calculation-type member at offset 372 (`0x3a9d8`). Neither
establishes the persisted formula enum's names. No mapping between these enums
has been confirmed.

## SDK behavior and evidence limits

Formula inspection decodes both stroke lists, image media ID and result
rectangle. Automatic formula rendering is absent. Final placement and
appearance have not been checked against a paired Samsung SDOCX/PDF export.
