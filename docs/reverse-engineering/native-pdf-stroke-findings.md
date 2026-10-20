# Native PDF stroke rasterization

## Evidence and scope

Confirmed against Samsung Notes 4.4.45.37 ARM64 `libSPenComposer.so`,
`libSPenDrawing.so` and `libSPenPdf.so`. Addresses below identify those
libraries explicitly. The APK digest is recorded in the knowledge-base index.
The batch-export findings below use native instructions and vtable relocations,
without runtime or device-export validation. The separate per-pen writer has
the bounded execution evidence described below.

The native vector list exporter batches strokes between individual object
exports, as described in [capture composition findings](capture-composition-findings.md).
Those batches become bitmap images. The word `Vector` in the native exporter
name does not mean that its stroke output consists of PDF paths.

The normal Java Standard PDF option actually selects the separate
`NotePDFExporterRasterListX` implementation. Its confirmed selection,
highlighter Darken blend and final object-batch flush are documented in
[Standard PDF composition findings](standard-pdf-composition-findings.md).
The class name alone does not establish that the public option uses the
native `VectorList` behavior below.

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
The [pen opacity findings](pen-opacity-findings.md) continue through concrete
plugin bindings, theme conversion, queued ARGB settings and Marker2 V1's
separate maximum-coverage mask and source-over color composite.

Neither this batch exporter nor the traced PDF image insertion path calls
the top-layer capture compositor or explicitly sets its darken/lighten
mode. Pen-specific blending can still occur inside the drawing engine.
Higher-level export preparation may also alter the supplied list or note.
The evidence therefore does not establish that every native PDF highlighter
uses normal blending, or that raster and vector export look identical.
The Standard X implementation explicitly assigns Darken through a different
PDF writer, as recorded in the linked composition findings.

## Separate per-pen vector writer

Composer also has a path-writing family distinct from the batch-to-bitmap
exporter above. `ObjectPDFWriter::WriteObject` (`0x37b600`) dispatches stroke
type 1 through `ObjectStrokePDFWriter::WriteObject` (`0x37e134`), which calls
`PDFPenFactory::CreatePDFPen` (`0x375598`) and the returned pen's virtual slot
`+16`. `NotePDFWriterVector::drawObjects` (`0x377880`) reaches this family;
its connection to the public Notes Java/JNI export route remains unproven.

The [Fountain writer oracle](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/fountain-pdf-writer-native.json)
executes the native factory and a separately constructed `PDFFountainPen`,
verifies their vtable identity and dispatches its native `WriteObject`
(`0x373dbc`) through slot `+16`. Model setters reconstruct the saved channels;
getters verify their bytes before native `PDFPen::getDrawPointData` and the
initialized pen plugin produce geometry. This is not original-frame loading.

The substituted PDF sink records circles, rather than rendered PDF bytes.
Pinned PDF vtables identify writer slots `+0x58/+0x68/+0x78` as
`CreatePaint/CreatePath/DrawPath`, path slot `+0x28` as
`Circle(PointF,float,bool)`, and paint slots `+0x18/+0x20` as
`SetColor/SetStyle`. Circle center and radius arrive in `s0/s1/s2`, with the
bool in `w1`. The three synthetic cases emit 19, 7 and 19 circles; the
source-derived 161-point V14 case emits 20. The seven-circle case uses raw
settings `18;0;100;`, whose Drawing selection is V16.

The native helper overrides saved tolerance: V14 `GetDrawnPointData`
(FountainPen `0x730ac`) sets `1.5`, while V16 (`0x78ed8`) sets float32 `2.6`.
Both set repeat distance `0.82` and enter redraw with collection flag false.
For the 161-point witness, the PDF's 20 circle triples exactly match the prefix
of the 22 native GL/Rust dots at tolerance `1.5`; source-tolerance GL/Rust
instead produce 24 dots. The complete saved-GL stream is a separate contract.

A three-point BrushPen control executes Composer `PDFBrushPen::WriteObject`
(`0x3735c0`) and captures 26 circle triples from native `GetDrawnPointData`.
At factory scale 1 these match the sink arguments exactly. Paired GL radii
also match, but GL centers include `tilt * radius`: BrushPen `drawPattern`
(`0x5c6f8`/`0x5c6fc`) applies that offset before `RTV1::AddPoint`. All 26
records satisfy this mapping within `1e-7`; the GL stream SHA-256 is
`57405d81981cd9deb40190fc185572dc9d5885ff5a92cf885e1ef53d652cee92`.
The helper sets tolerance 2.6 (`0x5c200`), so this one-to-one correspondence
does not establish other profiles. The mocked sink records solid circles,
not the GL texture field or serialized PDF. Capture and mapping remain in
ignored `tmp/vector-parity-plan/native-pdf-brush-route/`.

There is also a backend limitation. In pinned `libSPenPdf.so`,
`PDFEnginePdfium::CreatePDFPath` (`0x73270`) returns null, and its `Circle`
(`0x73288`) is a bare return. `PDFPath::Circle` (`0x66500`) forwards through
engine slot `+704`, whose PDFium vtable relocation at `0xb06c0` identifies that
stub. Its generic `DrawPath` (`0x73478`, engine slot `+1032`) is also a bare
return. A [native path API replay](https://github.com/twangodev/sdocx/blob/a24179384fbe043a8b5f003ee0c7bebeb394d106/conformance/pdf-writer-path-native.json)
executes the concrete engine and writer/path/paint methods: path creation and
construction fail while paint construction succeeds. It opens no document.
These entrypoints do not serialize the captured circles; the separate
implemented `PdfiumPathHandler` route is outside this sink contract.

This confirms native command emission, with host context and adapter boundaries
recorded in the fixture. It does not establish PDFium serialization, device
pixels or use by Standard, print or another public Notes export mode. These
circles therefore do not replace the GL coverage contract or serve as a
device-export reference.

## SDK behavior and evidence limits

The SDK preserves supported SVG paths and ordered content. Native stroke
bitmaps and vector paths use different coverage/compositing contracts, so
image differences alone do not isolate a geometry error. Pen settings,
coverage and opacity have the limits recorded in the linked rendering findings.

The mixed-list final-batch condition remains unresolved as documented in
the capture findings. The iterator tail has no additional flush between
iterator exhaustion and list destruction. The static trace does
not establish the practical effect after list preparation or runtime batching.
