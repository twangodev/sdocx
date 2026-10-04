# Native image drawing and effect activation

## Evidence and scope

These are static ARM64 findings from Samsung Notes 4.4.45.37, APK SHA-256
`daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667`.
The inspected ELF files were compared byte-for-byte with their APK entries.
Addresses below are ELF virtual addresses in Drawing unless otherwise qualified.

| Library | SHA-256 |
| --- | --- |
| `libSPenDrawing.so` | `788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd` |
| `libSPenModel.so` | `4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a` |
| `libSPenXmlSerializer.so` | `7be7af380ae378f91c0e565dcc022479c3f87aa965a5190f245f4d006cb6f36a` |
| `libSPenWDoc.so` | `1fc540573cc07f3e52466fd048568c8522119c6952cf22213b135ead00af57f6` |
| `libSPenGraphics.so` | `aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621` |
| `libSPenBase.so` | `e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb` |
| `libSPenSkia.so` | `42636cb9ac06cc286114b42c1b9d8f4b78d33761843251cde2b443b101ffb88d` |

The [image serialization findings](image-findings.md) describe the saved fields
and existing paired-document coverage. No native execution or new Samsung
document/export pair was used for the drawing findings here.

## Image-fill byte offsets and defaults

Model `FillImageEffect::GetBinary`, `0x3b90c8`, writes the normal 62-byte
payload below. Offsets are relative to the fill payload, excluding the
enclosing effect type and size. The suffix writes are at `0x3b91cc`–`0x3b924c`.

| Offset | Saved value |
| ---: | --- |
| 0 | `u8` fill mode: Java names 0 stretch and 1 tiling |
| 1 | `i32` main media bind ID |
| 5 | Four `f32` stretch offsets |
| 21 | Two `f32` tiling offsets |
| 29 / 33 | `f32` tiling scales X / Y |
| 37 | `f32` transparency |
| 41 | One-byte rotatable flag |
| 42 | Four `i32` nine-patch coordinates |
| 58 | `i32` nine-patch width |

The coedit writer replaces the four-byte ID with a 64-byte hash, shifting
every suffix offset by 60 and making the payload 122 bytes. The hash copy
is at `0x3b9174`–`0x3b9180`; the ordinary ID store is at `0x3b91c4`.
The coedit entry point `GetBinaryByCoedit`, `0x3b9594`, enables the alternate
mode at `0x3b95b4`, invokes the common writer, then clears the mode byte.

`ApplyBinary`, `0x3b9298`, normalizes any nonzero rotatable byte to true at
`0x3b947c`–`0x3b948c`. It consumes the nine-patch suffix only when its
unsigned version argument is at least 28, at `0x3b9488`–`0x3b9490`.
This is the fill reader's version argument, not an established archive
version-number rule.

Native `FillImageEffect::Construct` calls its initialization helper at
`0x3b80a4`; the helper at `0x3b8110`–`0x3b8160`
sets stretch/tiling offsets and transparency to zero, tiling scales to 100,
fill mode to 0, image index to -1, and rotatable to false. In contrast,
`com/samsung/android/sdk/pen/document/shapeeffect/SpenFillImageEffect.java`
initializes rotatable to true. That Java constructor is not evidence for
the native `Construct` defaults. The Java setters allow arbitrary
float offsets, scales and transparency; its fill-mode setter rejects values
outside 0–1.

## Original placement precision and coedit span state

Type-3 field 17 writes four `f64` coordinates, but its native storage is
an integer `Rect`. Model's writer converts the stored signed integers to
doubles at `0x3a4740`–`0x3a4768`. The reader converts those doubles toward
zero to 64-bit integers, keeps their low 32 bits, and only then applies
`Rect::Scale` for non-unit magnification, at `0x3a4bf8`–`0x3a4c2c`.
For finite coordinates in the representable range, a fractional wire value
therefore does not remain fractional in native original-placement storage.
The Java `SpenObjectImage` API also exposes `originalRect` as `Rect`, whereas
displayed/drawn bounds use `RectF`.

Type-3 field 19 is a saved span-attribute payload under a coedit writer gate.
At `0x3a47e8`–`0x3a482c`, the writer requires an available context,
`IsInitializeCoeditData == false`, `IsCoeditMode == true`, and
`ObjectBase::BelongsToSpan == true`. Model getters `0x2ac29c` and
`0x2ac2dc` identify the two context checks independently.

The payload at `0x3a4830`–`0x3a4884` is a four-byte path size, the saved
attribute path encoded with document-type argument 2, saved crop `Rect`
at `ObjectShapeImage` +200, then saved original-placement `Rect` at +184.
Model `ObjectShape::UpdateAttValue`, `0x39c298`, establishes these roles:
it copies the current path to `ObjectShapeData` +296 at `0x39c308`–`0x39c314`,
then copies live crop +36 and original +148 into saved +200 and +184 at
`0x39c318`–`0x39c324`, without integer-to-float conversion. ShapeImpl
constructs its data at +8 and image component at +312 (`0x3a560c`–`0x3a5620`).

This method preserves a previously pending saved snapshot after qualified
Base update (`0x39c2b4`–`0x39c2c4`); otherwise it clears saved geometry and
copies only when `BelongsToSpan` is true (`0x39c2c8`–`0x39c324`). Image span
transitions reach this producer: `OnBelongedToSpan` invokes virtual slot +384
at `0x2d2024`, and ObjectImage's relocation `0x497cd8` resolves to `0x39c298`.
The [common span lifecycle](object-flexible-findings.md) remains separate.

The reader writes saved members directly at `0x3a4ca0`–`0x3a4d0c`, without
scaling them or calling live rectangle setters, and marks
`SetSavedAttValue(true)` after successful rectangle reads at `0x3a4d20`.
Field absence clears saved path and rectangles at `0x3a4c4c`–`0x3a4c84`.
This establishes load preservation, not automatic restoration into live
geometry, malformed-input recovery, or a generic image clipping mask.

## Coedit XML consumes saved geometry separately from live attributes

XmlSerializer `initialSubSerializer`, `0x136338`–`0x136368`, installs the
coedit shape serializer for XML format value 1. Its full attribute writers
choose by `BelongsToSpan`, without checking `HasSavedAttValue`:
`ComposePathAttribute` selects saved ShapeImpl +304 over `GetPath()` at
`0x1370d8`–`0x1370f4`; crop selects saved impl +512 over live +348 at
`0x1378b0`–`0x1378d4`; original selects saved +496 over live +460 at
`0x137b2c`–`0x137b50`. Invalid selected paths log and return success without
writing a path attribute (`0x1370f8`–`0x13717c`). `ComposeDisplayInfoAttribute`
calls these routines, but admits crop/original output using the **live**
rectangles' `Rect::IsNull` checks (`0x133e10`–`0x133e40`). Admission and the
selected output rectangle therefore use different members while belonging.

Incoming coedit XML also updates snapshots: `ParseCropRectAttribute` stores
changed live crop and clears its cache, then copies live crop to saved crop
when belonging (`0x137774`–`0x1377ac`). `ParseOriginalRectAttribute` stores
live original unconditionally and copies it to saved original when belonging
(`0x1379fc`–`0x137a28`). `ParseDisplayInfoAttribute` copies its parsed path
into saved impl +304 when belonging (`0x133b28`–`0x133b44`) and calls the two
rectangle parsers at `0x133cb8` and `0x133cc8`.

Partial attribute bodies instead use live getters (`0x13900c`, `0x13a288`,
`0x13a384`) and setters (`0x138f44`, `0x13a208`, `0x13a304`). The separate
`GetPartialData` backup route retrieves an integer-keyed String, then passes
a null backup value (`0x1209dc`–`0x1209f8`); it does not establish numeric
field-19 restoration. These XML consumers establish synchronization source
roles, not rendered appearance or clipping behavior.

Current Rust `image.rs:227`–`229` skips the sized field-19 path and 32 rectangle
bytes; its live crop/original projection does not retain the saved baseline.
The [original-byte carrier](vector-retention-findings.md#original-page-bytes-are-external-to-the-parsed-model) can preserve those
bytes separately: `StoredObject::payload` borrows caller-supplied page bytes
(`storage.rs:215`–`218`), rather than making a typed image own them.

## Shape effects and image pixels have separate drawing calls

`ObjectDrawing::drawObjectImage`, `0x80f54`, constructs an
`ObjectImageDrawing`, installs the image object, calls `DrawPath` at `0x80fbc`,
then calls `DrawBitmap` at `0x80fd0`. These are separate operations.

`ObjectImageDrawing::SetObject`, `0x846fc`, calls
`ObjectShape::HasVisibleEffect` at `0x84720` and stores that result at internal
offset 8. Only a true result installs the object in the embedded
`ObjectShapeDrawing`. `DrawPath`, `0x84794`, checks that byte at
`0x847ac`–`0x847b0`; false returns success without drawing a path. True calls
`ObjectShapeDrawing::SetEffect` at `0x847c4` and its `DrawPath` at `0x847e4`.
Consequently, path presence alone does not establish that this shape-effect
pass paints anything.

`getImageFromObject`, `0x84978`, requests `ComponentImage::GetCacheImage`
first at `0x84994`. If that pointer is null, it requests `GetImage` at
`0x849d0`. Model `ComponentImage::GetImage`, `0x3a1a74`, checks that the fill
effect has type 2 and then calls `FillImageEffect::GetImage` at `0x3a1aa8`.
This route does not fetch the separate original-image or border-image reference.

`getImageSPBitmap`, `0x849e8`, skips additional image processing when the
input came from the object cache. Otherwise `isExtraDrawingExist`,
`0x863fc`, selects processing for any of these conditions:

- Positive legacy line-border width with nonzero border type.
- Border type 4, even when its line-border width is zero.
- Positive fill-effect transparency.
- Enabled hint text with a nonnull text pointer.
- A nonempty crop rectangle or a nonempty nine-patch rectangle.

The checks occur at `0x8640c`, `0x86424`–`0x8642c`, `0x86434`–`0x8643c`,
`0x86444`, `0x86450` and `0x86474`. The selected
`getResizedImageBitmap`, `0x86478`, calls `calculateDstSize`, then
`drawImage`, and stores the resulting bitmap through `setCacheImageToObject`
at `0x86508`. This object-local processed-image cache is separate from the
[document canvas cache](document-image-cache-findings.md).

## Ordinary image effect paths differ from image-filled vector shapes

`ShapePathFactory::CreateShapePathType`, `0x8b66c`, selects the saved shape
path for type 7, but `ImagePathType` for types 2/3. Its `GetPath`, `0x8bc98`,
synthesizes six segments from `GetRect` and `GetRotation`: move, four lines
including return to the first corner, then close (`0x8bd30`–`0x8be20`). It
does not read the inherited saved custom path or the coedit attribute path.
The standard type-3 shape-effect route therefore does not establish a custom
outline mask for the subsequent ordinary image pixel draw.

Generic `ShapeDrawingFillEffect::SetFillImageEffect`, `0x9c558`, returns true
immediately for type 3 at `0x9c584`–`0x9c594`. Its freshly initialized
prepared-canvas pointer is null (`0x9c14c`–`0x9c154`); `DrawEffectImage`
returns false without that pointer (`0x9cf60`–`0x9cf64`, `0x9d164`). The
surrounding path-effect method still draws a visible outline. For admitted
non-image objects, image-fill setup instead prepares a source-sized canvas.

For a type-7 image fill, stretch mode clips the supplied vector path on a
temporary canvas before changing the bitmap matrix: clip-operation value 5,
antialias false (`0x9d20c`, `0x9d4c0`). It draws the whole prepared bitmap
with a null source rectangle, restores, then composites at the rotated-bounds
origin (`0x9d2d4`–`0x9d2fc`, `0x9d550`–`0x9d578`). The clip is local to this
fill. Tile mode draws the same vector path with a bitmap shader, without an
explicit clipPath call (`0x9d0ac`–`0x9d120`, `0x9d364`–`0x9d3b0`); both
shader tile-mode arguments are 1. These numeric Skia operations are recorded
literally here. The generic fill draw does not read ordinary crop,
OriginalRect or nine-patch settings; its supplied cache pixels may already
have been processed.

Generic setup truncates float rotation toward zero to a signed integer
before computing rotated bounds (`0x9c66c`–`0x9c680`). Rotatable stretch uses
the unrotated object bounds; nonrotatable stretch uses the actual SkPath
bounds (`0x9c6f0`–`0x9c764`, `0x9d3b8`–`0x9d4b8`). An axis whose stretch
offsets sum strictly above 1 is reflected. The clip precedes the reflected
and rotated bitmap matrix (`0x9d214`–`0x9d28c`, `0x9d4dc`–`0x9d508`).
Tile setup multiplies saved scales by f32 `0.01` (`0x9c7ac`, `0x9c7b8`);
rotatable tiles anchor to the unrotated rectangle and use integer rotation,
while nonrotatable tiles anchor to actual path bounds without that rotation.

Generic fill alpha uses one fused f32 `255 - transparency * 255`, unsigned
conversion toward zero, then `SkPaint::setAlpha`, without an explicit clamp
(`0x9d08c`–`0x9d094`, `0x9d2a0`–`0x9d2ac`, `0x9d344`–`0x9d34c`,
`0x9d51c`–`0x9d528`). Skia `0x1cac70` inserts only the argument's low eight
bits. This differs from the ordinary image pixel alpha contract below;
nonfinite inputs remain unexecuted.

## Crop editing updates separate rectangles and invalidates cached pixels

Model `ObjectImage::CropImage`, `0x420644`, compares and, when changed,
updates its second argument as crop Rect first, its first as OriginalRect
second, and its third through the displayed RectF setter last (`0x420734`,
`0x4207b8`, `0x42085c`–`0x420878`).
Earlier setter failure exits before later updates, with no rollback here.
It neither derives the integer rectangles from display bounds nor changes
fill stretch/tiling settings. `ObjectShapeImage::SetCropRect` clears the
cache (`0x3b3dd4`–`0x3b3dec`); `SetOriginalRect` stores its rectangle and
marks changed without clearing it (`0x3b3e58`–`0x3b3e68`). `GetCacheImage`
returns an existing bitmap or loads its stored cache path (`0x3b4444`–
`0x3b4490`). This editing route does not establish all cache-production paths.

## Pixel placement uses drawn bounds, local size and rotation

`drawImageBitmap`, `0x84b20`, calls object virtual slots 160 and 168 at
`0x84b68` and `0x84b84`. Model's `ObjectImage` address point is `0x497b58`:
relocations `0x497bf8` and `0x497c00` identify those methods as
`ObjectShape::GetDrawnRect` and `ObjectShapeBase::GetRect`. Slot 136,
called at `0x84be0`, resolves through `0x497be0` to `ObjectBase::GetRotation`.

The canvas operations are save, translate to the drawn rectangle's center,
rotate by the stored angle, then translate by minus half the local width and
height. It draws the whole supplied bitmap source rectangle into
`[0, 0, local_width, local_height]`, then restores the canvas. The calls are
at `0x84ba0`, `0x84bcc`, `0x84bf0`, `0x84c20`, `0x84ccc` and `0x84ce0`.
Graphics relocations `0xd0308`, `0xd0370`, `0xd0350`, `0xd0420` and
`0xd0310` identify the relevant `SPCanvas` operations as `Save`,
`PreTranslate`, `PreRotate`, the source/destination `DrawBitmap` overload,
and `Restore`.

This final placement function does not read `OriginalRect`. The existing
Rust clipping reconstruction for the measured cropped span is described in
[image findings](image-findings.md#images-embedded-in-document-text-flow);
that measured result does not establish the full native original-image editing
or regeneration lifecycle.

## Fill transparency is fractional and applies before borders and hint text

`getFillEffectTransparency`, `0x8596c`, returns zero for object types other
than 3. For type 3 it constructs a `FillImageEffect`, obtains the object's
fill effect at `0x859c4`, and returns `GetTransparency` at `0x859cc`.
This float is distinct from the image component's Boolean transparency API.

For positive transparency `t`, `drawImage`, `0x852a4`, calculates paint alpha
as `255 - t * 255` using a fused `f32` multiply-subtract at `0x8531c`,
converts toward zero, clamps to
`[0, 255]`, and calls `SkPaint::setAlpha` at `0x85330`. The arithmetic and
clamp are at `0x85308`–`0x8532c`. Nonpositive values bypass that update.
These instructions do not establish useful behavior for non-finite values.

The zero-alpha gate at `0x85338`–`0x8533c` skips image pixels. Its internal
offset 135 is the alpha byte of the `SkPaint` at offset 40, not a separately
serialized image property: Skia `setAlpha`, `0x1cac68`, writes the high byte
of the color at paint offset 92. The default paint constructor at Skia
`0x1c9c14` initializes that alpha to 255.

After image painting, positive transparency causes alpha to be reset to 255
at `0x85460`–`0x85468`. `drawBorder` and `drawHintText` are then called at
`0x85478` and `0x85488`. Thus even completely transparent image pixels do
not suppress these later operations. This ordering does not establish the
opacity of the surrounding object list or page compositing pass.

## Crop takes precedence over nine-patch

For a nonzero paint alpha and a successfully created source canvas bitmap,
`drawImage` selects exactly one pixel path:

| Condition | Drawing operation | Evidence |
| --- | --- | --- |
| Crop rectangle is nonempty | Convert its four signed integer coordinates to floats and use it as the source rectangle | `0x85370`–`0x853d0` |
| Crop is empty and nine-patch rectangle is nonempty | Obtain the nine-patch rectangle and ratio, then draw nine patches | `0x853dc`–`0x8541c` |
| Both are empty | Draw the full source bitmap with a null source-rectangle argument | `0x85424`–`0x85448` |

The crop and ordinary paths dispatch Skia canvas slot 112. Skia relocation
`0x295910` resolves it to `Spen_SkCanvas::drawBitmapRectToRect`.
An active crop therefore bypasses nine-patch drawing even when both fields
are populated.

`hasCropRect`, `0x85a2c`, and `hasNinePatchRect`, `0x85a94`, both negate
`Rect::IsEmpty`. Base `Rect::IsEmpty`, `0xb0984`, checks
`left >= right || top >= bottom`; it does not check whether every coordinate
is zero. Zero-area rectangles at nonzero coordinates and inverted rectangles
are inactive under this gate. Nine-patch width alone is not an activation test.

## Nine-patch destination geometry and rounding

`drawNinePatchImage`, `0x856d0`, builds source cuts
`[0, left, right, source_width]` and
`[0, top, bottom, source_height]`. Given destination bounds
`[L, T, R, B]` and supplied ratio `q`, its candidate internal destination cuts
are:

```text
x1 = L + left * q
x2 = R - (source_width - right) * q
y1 = T + top * q
y2 = B - (source_height - bottom) * q
```

The arithmetic is at `0x85740`–`0x857ac`. These equations use fused `f32`
multiply-add/subtract instructions at `0x85780`, `0x85784`, `0x857a4` and
`0x857ac`; separately rounded multiplication and addition/subtraction do not
describe the instruction order. Source dimensions minus the second source cut
are calculated as signed 32-bit integer differences before conversion to float.
If either candidate center interval
is not strictly positive, `0x857b8`–`0x857c4` replaces both axes' internal cuts
with `L + left`, `L + right`, `T + top`, `T + bottom`. It then rounds the
first internal cut on each axis upward and the second downward at
`0x857c8`–`0x857dc`. The nested loops at `0x858a8`–`0x85920` issue all nine
source/destination draws. There is no per-patch positive-area test in those loops.

The ratio comes from Model `ObjectShapeImpl::GetNinePatchRatio`, `0x3a6c24`:
current context page width divided by the saved fill nine-patch width. The
provider is exactly `ModelContext::GetPageWidth`, `0x2ab694`: invoke its
`std::function<int()>` callable at context +1200 through slot +48, otherwise
read the stored integer at +1156. Missing context or zero denominator returns
1; the getter does not require positive values. Two signed-i32-to-f32
conversions precede f32 division (`0x3a6c7c`–`0x3a6c84`).

Model `ComponentImage::SetImage(String const*, Rect)`, `0x3a17dc`, snapshots
that page width when context exists, calls `SetNinePatchWidth` at `0x3a18b4`,
then `SetImageUri` at `0x3a18c8`. With no active image, the width is pending
member +68 (`0x3b89c0`–`0x3b89e4`); image binding passes it to
`ImageCommon::AddImage` (`0x3b86d0`–`0x3b86e4`). The Bitmap overload reaches
this String setter at `0x3a1a14`. Thus these producers capture a reference
page width, rather than using the source bitmap's pixel width. Other arbitrary
fill-effect producers and universal physical units are not established here.

Model `SetPageSize`, `0x2ab678`, and `PageImplBase::SetWidth`, `0x343748`,
populate/synchronize context +1156. WDoc `WPageImpl::OnAttach` copies its
page's current width/height into that context (`0xced90`–`0xcedac`), as does
`OnContextChanged` (`0xcfba8`–`0xcfbc4`). WDoc's fixed-header reader passes
saved page width to `SetWidth` at `0xd2c90`; `LoadHeader_Scale`, `0xd3760`,
can subsequently replace it with the requested dimension or an orientation-
dependent scaled/truncated width (`0xd37bc`–`0xd3824`). The numerator can
therefore reflect the loaded/scaled page. These ratio routines query neither
DPI nor browser zoom; upstream choices of requested dimensions and later
canvas scaling remain separate from this bounded contract.

Border images share that numerator but have an independent denominator:
Model `ObjectShapeImage::GetImageBorderNinePatchRatio`, `0x3b3f94`, reads
the border image's own `ImageCommon` width (`0x3b3fa8`–`0x3b3ff0`). Its
attached history-backed setter snapshots page width and supplies it to a new
image (`0x3a2660`–`0x3a26ac`, `0x3a279c`–`0x3a27b4`); a detached new-image
branch supplies zero (`0x3a25a4`–`0x3a25ac`). Main and border ratios can differ.
Drawing passes these separate ratios to the same slicing helper at `0x8541c`
and `0x85e00`, respectively.

Saved main cuts/width pass unchanged into `ImageCommon` during fill loading
(Model `0x3b94ac`–`0x3b94dc`). Border cuts and width likewise remain unscaled
(`0x3b47ac`–`0x3b47b4`, `0x3b480c`–`0x3b481c`), while the separate float
border side-width rectangle is magnified (`0x3b47d8`–`0x3b47ec`).
The SDK currently reads and discards both reference widths in `image.rs`
(main at 337, border at 221); it reports nonzero main cuts as unsupported.
Raw page bytes retain these inputs, which are absent from the high-level image.

## Legacy border drawing remains present despite Java deprecation

The decompiled SDK file
`com/samsung/android/sdk/pen/document/SpenObjectImage.java` names border
types 0–4 as none, square, shadow, dot and image. Its Java API deprecates
these settings, including declaring image and shadow borders unsupported
as of 4.0.0. Native `drawBorder`, `0x85b00`, still contains these branches:

| Type | Native operation | Evidence |
| ---: | --- | --- |
| 1 | Stroke a rectangle | `0x85c30`–`0x85c4c` |
| 2 | Stroke an open path from bottom-left through bottom-right to top-right; no blur call occurs in this branch | `0x85c64`–`0x85cb0` |
| 3 | Stroke a rectangle with dash intervals `[line_border_width, line_border_width]`, phase 0 | `0x85cd0`–`0x85d3c` |
| 4 | Load the separate border image and draw it across the full destination bitmap with its own nine-patch rectangle and ratio | `0x85d54`–`0x85e00` |

Except for type 4, entry requires positive line-border width and nonzero type
through `hasLineBolder`, `0x86350`. The stroked rectangle expands the
image destination by half the legacy line-border width plus half the inherited
line-style width, at `0x85b90`–`0x85bc8`. Paint color and stroke width come
from the legacy image-component getters at `0x85bf8` and `0x85c14`.

`calculateDstSize`, `0x850b0`, allocates image-border type 4 dimensions as
`ceil(source_width + left_width + right_width)` and
`ceil(source_height + top_width + bottom_width)`, and insets the image
destination by those side widths. Other active line borders use
`ceil(source_width + 2 * line_border_width)` and the corresponding height,
with a line-border-width inset. Inactive borders retain the source dimensions.
These operations are at `0x85110`–`0x85284` and are performed in `f32`.
The line-border dimension sums specifically use fused multiply-add at
`0x85228` and `0x8522c`; image-border side widths use sequential additions.

## Unresolved boundaries

This static trace establishes the selected Drawing methods and their branch
contracts. It does not establish device-export pixels, malformed geometry
admission, full cache invalidation, original-image regeneration, or complete
cache-production transforms. The traced image-filled vector shape clipping
does not establish ordinary-image custom-mask support. Deprecated Java API
declarations do not prove native saved-field branches
are unreachable, and native branch presence does not prove a current Samsung
document producer exercises them.
