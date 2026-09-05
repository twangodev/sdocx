# Native PDF stroke rasterization

## Evidence and scope

Confirmed against Samsung Notes 4.4.45.37 ARM64 `libSPenComposer.so`,
`libSPenDrawing.so` and `libSPenPdf.so`. Addresses below identify those
libraries explicitly. The APK digest is recorded in the knowledge-base index.
This investigation uses native instructions and vtable relocations; no new
Samsung SDOCX/PDF pairs or runtime captures were available.

The native vector list exporter batches strokes between individual object
exports, as described in [capture composition findings](capture-composition-findings.md).
Those batches become bitmap images. The word `Vector` in the native exporter
name does not mean that its stroke output consists of PDF paths.

The normal Java Standard PDF option actually selects the separate
`NotePDFExporterRasterListX` implementation. Its confirmed selection,
highlighter Darken blend and final object-batch flush are documented in
[Standard PDF composition findings](standard-pdf-composition-findings.md).
The native `VectorList` behavior below must not be attributed to that public
option solely from the class name.

## Batch factory and bitmap coordinates

Composer `ObjectPdfExporterFactory::CreateObjectListPdfExporter`, `0x34e840`,
returns no exporter for an empty list. It reads the first object at `0x34e888`
and requires type 1 at `0x34e890`–`0x34e898`. The successful branch constructs
`ObjectStrokePdfExporter` with the list at `0x34e8d0`. The factory does not
individually type-check every entry; the surrounding collector supplies the
stroke batch.

The list constructor, `0x34f508`, constructs its member-88 `ObjectList` at
`0x34f5e0` and initializes it from the supplied list through
`ObjectList::Construct(ObjectList const*)` at `0x34f5f0`.
`SetPosition`, `0x34f960`, stores the four supplied rectangle coordinates at
members 104–119. `ObjectPdfExporter::SetScale`, `0x34e0bc`, separately stores
its float at member 72. That member is export scale, not opacity.

`ObjectStrokePdfExporter::ExportObject`, Composer `0x34f684`, follows this
successful path:

| Address | Operation |
| --- | --- |
| `0x34f728`–`0x34f738` | Read rectangle width/height and convert each float to a signed integer with truncation toward zero |
| `0x34f76c` | Create a bitmap with those integer dimensions |
| `0x34f77c` | Create an `ObjectDrawing` through `DrawingFactory::CreateDrawing` |
| `0x34f78c`–`0x34f7c0` | Build the drawing matrix with the negated rectangle left/top as translation |
| `0x34f7c4` | Set the renderer's system-font option through virtual slot 144 |
| `0x34f7ec` | Draw the batch into the bitmap through virtual slot 16, passing the translation matrix and no clip rectangle |
| `0x34f7f4` | Release the renderer |
| `0x34f808` | Construct a shared `PdfImageAdapter` through helper `0x3496ac` |
| `0x34f814` | Pass the bitmap to `PdfImageAdapter::SetImage` |
| `0x34f81c`–`0x34f824` | Copy the original rectangle to the image export information |
| `0x34f8c8`–`0x34f8d0` | Copy export scale to image export information member 60 |
| `0x34f8d8`, `0x34f8e4` | Release the bitmap, then export the image to the requested PDF page |

The shared-adapter helper reaches the named `PdfImageAdapter` constructor
at `0x349844`. `PdfObjectAdapter::SetScale`, `0x3450c8`, independently
identifies information member 60 as scale. In this method the scale does
not multiply the intermediate bitmap's dimensions. Fractional bounds are
therefore retained for placement while bitmap dimensions use integer conversion.

Drawing `DrawingFactory::CreateDrawing`, `0x74934`, constructs
`ObjectDrawing` at `0x74950` and calls its `Construct` at `0x7495c`.
The concrete vtable starts at `0xc2f60`, with its address point at `0xc2f70`.
Relocation `0xc2f80` resolves slot 16 to `DrawObjectList`, `0x7f098`;
relocation `0xc3000` resolves slot 144 to `SetSystemFontEnabled`, `0x805b4`.
This is the ordinary object-list renderer, not `NoteCapturePage`'s explicit
base/top/masking pass compositor.

## PNG handoff and retained pixel alpha

Composer `PdfImageAdapter::SetImage(ISPBitmap*)`, `0x344130`, obtains the
bitmap dimensions and calls `saveBitmap` at `0x3441b0` with the full rectangle.
`saveBitmap`, `0x3442b4`, allocates four bytes per pixel at `0x3442fc`–
`0x34430c`, creates a CPU bitmap, and copies the GPU bitmap through virtual
slot 32 at `0x34435c`. It saves under `/temp_export_pdf/` with a `.png` suffix
at `0x344448`. The suffix comes from the string at Composer `0x1d06cb`.

`SetImage` stores that path in export information member 80 and sets byte
104 at `0x3441e8`. That byte controls temporary-file deletion: PDF
`PdfiumImageHandler::AddImage`, `0x82810`, creates the image at `0x828f4`,
tests byte 104 at `0x828f8`, and unlinks the file at `0x82914` when it exists.
It is not an alpha or blend flag.

Composer `PdfObjectAdapter::ExportObject` reaches
`PdfExporter::ExportImage` at `0x344de4`. In PDF, `ExportImage`, `0xa892c`,
forwards through virtual slot 600. Relocation `0xb0658` in the
`PDFEnginePdfium` vtable resolves that slot to `AddImage`, `0x72724`, which
calls `PdfiumImageHandler::AddImage` at `0x72750`.

The latter calls `PdfiumImpl::CreateImageObject(document, path)`, `0x8a620`.
Its non-JPEG branch loads the bitmap at `0x8a728`, calls `CreateARGBBuffer`
with its boolean argument true at `0x8a750`, creates a PDFium bitmap at
`0x8a778`, and supplies it to `CPDF_Image::SetImage` at `0x8a7ac`.

`CreateARGBBuffer`, PDF `0x8aa7c`, copies each four-byte pixel at
`0x8ab58`–`0x8ab5c`, then swaps bytes 0 and 2 at `0x8ab68`–`0x8ab74`.
With the supplied true argument, it reads byte 3 and converts each color
channel using integer `channel * 255 / alpha`; zero alpha produces zero
color channels. The conversion is at `0x8ab84`–`0x8abf4` and leaves byte 3
unchanged. This establishes an alpha-preserving conversion from premultiplied
color data at the PDF handoff. It does not establish identical rasterization
or color rounding between the native pen engine and the SDK.

## Object alpha, pen opacity and composition are separate

Drawing `ObjectDrawing` initializes its object-alpha option at member 80
to false: the zero store at `0x7ef38` covers members 67–82.
`SetObjectAlphaEnabled`, `0x80574`, and `IsObjectAlphaEnabled`, `0x8057c`,
identify the same byte. `Construct`, `0x7f000`, leaves it unchanged. The
stroke PDF exporter sets the system-font option but does not enable this
object-alpha option on its newly created renderer.

In `DrawObjectList`, an object with `0 < GetAlpha() < 1` takes the ordinary
drawing path when this option is false, through `0x7f3d4`–`0x7f3d8` and
`0x7f4c8`–`0x7f4dc`. When enabled, the separate intermediate-bitmap branch
applies common object alpha. The nonpositive-alpha case also has a selected
object condition at `0x7f454`–`0x7f470`. These runtime conditions do not
identify a serialized common-opacity field.

The stroke renderer still configures the pen from its own inputs:

| Drawing call site | Input |
| --- | --- |
| `0x81a80`, `0x81ad4` | Pen name and pen-manager lookup |
| `0x81bb0` | Advanced pen settings, when supported by the pen |
| `0x81bcc`, `0x81be8` | Color type and pen size |
| `0x81c1c`–`0x81c48` | Stroke color, context color conversion, then pen virtual slot 80 |
| `0x81d54`, `0x81d88`, `0x81e30` | Particle density, particle size and rendering level |
| `0x81f98`–`0x81fac` | Fixed-opacity flag passed to a pen sub-interface |
| `0x821f4` | Gradient colors |
| `0x824b8`, `0x824e8` | Pattern index and scale |

The serialized counterparts are recorded in
[stroke metadata findings](stroke-metadata-findings.md). Retained ARGB,
fixed opacity, common object alpha, and the capture compositor's darken or
lighten mode are distinct inputs. No one field replaces the others.

Neither this batch exporter nor the traced PDF image insertion path calls
the top-layer capture compositor or explicitly sets its darken/lighten
mode. Pen-specific blending can still occur inside the drawing engine.
Higher-level export preparation may also alter the supplied list or note.
The evidence therefore does not establish that every native PDF highlighter
uses normal blending, or that raster and vector export look identical.
The Standard X implementation explicitly assigns Darken through a different
PDF writer, as recorded in the linked composition findings.

## SDK implications and remaining validation

The SDK can continue to preserve SVG paths where supported; matching the
native choice to rasterize strokes is not itself a fidelity requirement.
Accurate output still needs ordered content, pen settings, coverage and
opacity behavior. PDF reference inspection must distinguish native stroke
bitmaps from vector paths before attributing image differences to SDK
geometry alone.

The mixed-list final-batch condition remains unresolved as documented in
the capture findings. Rechecking the iterator tail confirmed no additional
flush between iterator exhaustion and list destruction. List preparation
and runtime examples are still needed to establish the practical effect.
Useful new pairs include ordinary pens and highlighters crossing text or
images, overlapping strokes with different alpha, and pages ending in
strokes after a non-stroke object, exported through each available PDF mode.
