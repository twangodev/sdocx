# Native formula drawing

## Evidence

Analyzed Samsung Notes 4.4.45.37, APK SHA-256
`daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667`.
The following ARM64 traces establish drawing decisions without new documents.
Addresses in the first two sections are in `libSPenDrawing.so`.

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
fails. A portable renderer should report missing/unsupported image resources
explicitly and distinguish that failure from an image-free formula.

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
and is not a persisted formula appearance flag. Reproducing it in static export
would require separate justification.

The enclosing object/canvas transform and exact pen-dependent drawn bounds
remain necessary to place these images faithfully. The stored base rectangle
alone is not a proven substitute for the native drawn rectangle.

## Visible-stroke bounds

These addresses are in `libSPenModel.so`.

`ObjectFormulaImpl::GetDrawnRect` at `0x4325f4` refreshes its cached rectangle
using `GetRectByStrokeList(true)` (`0x432614`–`0x432618`).
`GetRectByStrokeList` at `0x431e4c` unions the source list at implementation
offset 88 and the answer list at 104. The union helper at `0x4324a4` calls
`ObjectBase::IsVisible` at `0x432520` and excludes invisible strokes. With its
boolean argument true it uses object virtual slot 160 (`GetDrawnRect`); false
selects slot 168 (`GetRect`).

`ObjectStrokeImpl::GetDrawnRect` at `0x2e9488` expands the stroke rectangle
according to the native pen category at implementation offset 328 and pen size
at offset 292. The scalar passed to `RectF::IncreaseRect` at `0x2e9604` is:

| Native category | Expansion scalar |
| --- | --- |
| 0, 4, 8, 9, 12 | `size + 4` |
| 1 | `size * 0.5 + 20` |
| 3, 5 | `size * 2 + 4` |
| 6, 7 | `size * 9 + 4` |
| 10 | `size * 0.5 * constant_at_0x1644fc + 4` |
| 11 | `size * 35 + 4` |
| Remaining values | `size * 0.5 + 4` |

The category names, category-10 constant and exact `IncreaseRect` convention
still need mapping before this table becomes a portable geometry algorithm.
For nonzero object rotation, the native code subsequently applies
`RectF::GetRotatedBound` at `0x2e9660`. This is a separate calculation from
stroking the decoded point sequence.

## Expression-type constraint

`ObjectFormula::SetExpressionType` at `0x42a134` in `libSPenModel.so` rejects
unsigned values greater than or equal to 2 at `0x42a178`–`0x42a17c`. Accepted
values are stored at implementation offset 288. The names of 0 and 1 are not
established by this setter, so the inspection API retains `expression_type_raw`.
The serialized reader accepts the stored value without this setter's check;
the parser should not reject future enum values merely because this APK's
editing API would reject them.

The similarly named `HwrMathExpression::SetExprType` in `libSPenHwrData.so`
stores a 16-bit value at offset 184 (`0x3a998`). Its `IsAssign` method reads a
different 32-bit calculation-type member at offset 372 (`0x3a9d8`). Neither
establishes the persisted formula enum's names. No mapping between these enums
has been confirmed.

## Implementation status

Formula inspection decodes both stroke lists, image media ID and result
rectangle. Automatic formula rendering is still absent. The next dependencies
are common visibility metadata, native pen-category/drawn-bound mapping,
formula image resolution, and enclosing transforms. Real SDOCX/PDF pairs will
be needed to check final placement and appearance against Samsung exports.
