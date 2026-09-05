# Shape and line frames

## Native evidence

This milestone uses Samsung Notes 4.4.45.37, arm64 `libSPenModel.so`, plus
the decompiled `SpenObjectShape`, `SpenObjectLine` and `shapeeffect` Java APIs.
The native sources remain ignored, alongside the APK analysis artifacts.

| Symbol/address | Contract |
| --- | --- |
| `ObjectShape::NewGetBinary`, `0x399b40` | Shape chain `0 + 6 + 7`; snapshots unrotated bounds, drawn bounds and rotation, then writes type 0 with drawn bounds and zero rotation. |
| `ObjectLine::NewGetBinary`, `0x386a64` | Line chain `0 + 6 + 8`. |
| `ObjectShapeBase::NewGetBinary`, helper `0x37c6b4–0x37ca9c` | Type 6 connection data and optional sized line color/style effects at field bits 2/3. |
| `ObjectShapeBinaryHandler::GetOwnBinary`, `0x3a8dd0–0x3a9228` | Type 7 geometry, an extra rectangle for outer type 7, then text/pen/fill fields. |
| `ObjectLineBinaryHandler::GetOwnBinary`, `0x38bc34–0x38bed8` | Type 8 line type, routing setting, control points, endpoints, two rectangles and a four-byte setting. |
| `LineStyleEffect::GetBinary`, `0x395b44` | Twelve bytes: float width, compound, dash, cap, join, begin arrow type/size, end arrow type/size. |
| `LineStyleEffect::Construct`, `0x3953e4` | Default width 2.0, remaining style enums zero. |
| `LineColorEffect::GetBinary`, `0x393968` | Property mask, color kind, ARGB, gradient settings and stops. Default is opaque black (`Construct`, `0x392e08`). |
| `FillColorEffect::GetBinary`, `0x3b649c` | Property mask encodes solid/gradient and rotation, followed by ARGB, gradient settings and stops. |
| `ObjectLine::GetConnectorPosition`, `0x382d50` | Exposes the same endpoint coordinates written at implementation offsets 116/124. |
| `ObjectLineImpl::SetRotation`, `0x387db4`; `GetConnectorPosition`, `0x388b14` | Rotation updates endpoints/path. Stored line geometry already includes rotation. |
| `ObjectShapeData::GetBinary_PenData`, `0x3ac284`; `GetPenName`, `0x3aba3c`; `SetAdvancedPenSetting`, `0x3aba5c` | Type-7 bits 2/4 are string IDs for pen name/advanced settings at offsets 272/288. |
| `ObjectLineImpl::SetPenName`, `0x387914`; `SetAdvancedPenSetting`, `0x3879a4` | Type-8 bits 1/2 are advanced-settings/name IDs at offsets 32/16, the reverse of shape field order. |
| `Path::GetBinary`, WDoc branch `0x2efe10–0x2effac` | Command count, one-byte verbs and `f64` coordinates, distinct from the non-WDoc float encoding. |
| `Path` command constructors, `0x2ef688–0x2efa40` | Move 1, line 2, quadratic 3, cubic 4, arc 5, close 6 and oval 7. |

Type 6 fixed data contains a `u32` magnetic-point count and pairs of `f64`,
a `u32` connection-block size followed by that block (beginning with a `u32`
record count), then one reserved byte. The size excludes its own size prefix.
Type 6 has no fill color; its color effect describes the outline.

Type 7 fixed data begins with a `u32` shape type, four `f64` local coordinates,
an `f32` rotation, a sized path and a one-byte control-point count with
16 bytes per point. Outer shape objects then append another four `f64`
rectangle coordinates; images and text boxes omit this rectangle. Flexible
fields include sized `TextCommon` at bit 0, one text-area-mode byte at bit 1,
pen-name ID at bit 2, advanced-pen-settings ID at bit 4 and a sized fill at bit 5. The fill
size excludes both the size prefix and the following one-byte effect kind.
Color fills use effect kind 1; image fills use kind 2.

The rotation field corrects the earlier image note's provisional "radius"
interpretation: `0x399bb8–0x399be8` stores `GetRotation()` and temporarily
clears the common rotation only for shape objects. Render shape geometry from
the first type-7 rectangle and this angle, not from the drawn type-0 bounds.

Type 8 fixed data starts with one-byte line type, one routing byte, one-byte
control-point count and pairs of `f64`. Two endpoint pairs, two four-`f64`
rectangles and a four-byte setting follow. Its minimum fixed size is 103 bytes.
Flexible bits 1/2 hold four-byte advanced-pen-settings/name IDs; bit 3 holds a native path.
Unknown preceding fields cannot be skipped by assuming an arbitrary width.

Both pen fields are signed string-resource references, confirmed by native
setters/getters through `StringIDManager`; neither encodes a color. Their raw
values, including negative sentinels, are preserved. Outline color comes from
the type-6 `LineColorEffect`.

Native WDoc paths start with a `u32` command count. Move/line commands have two
`f64` values, quadratic/oval four, cubic/arc six, and close none. The type-8
field has no separate byte-length prefix. Known command widths locate the
following field; an unknown verb makes the remaining bounded bytes opaque.

The Java constants identify oval 1, triangle 2, right triangle 3, rectangle 4,
rounded rectangle 5 and diamond 8. Line types are straight 0, elbow 1 and
curve 2. Unknown values must remain identifiable rather than becoming a
rectangle or straight line.

## Implemented model and rendering

`PageElement::Shape(NativeShape)` and `PageElement::Line(NativeLine)` expose
bounded geometry, outline/fill effects and native pen references. Shape text
reuses the rich-text decoder and its text/span/nesting limits. The remaining
UUID, bounding-box and UTF-16 scanners have been removed from page parsing;
object type and declared frame boundaries determine decoding.

SVG rendering supports ovals, triangles, right triangles, rectangles and
diamonds, using the unrotated geometry rectangle and its type-7 rotation.
Straight lines use the stored endpoints, including reversed or horizontal
lines. Elbow/curve lines with supported native paths render move, line,
quadratic, cubic and close commands. Paths must begin with a move. Unknown
line types and unsupported paths are not replaced with invented straight lines.
Solid fills and outlines preserve ARGB alpha; outline width, cap and join are
applied. The native default outline is black at width 2.0. Explicit no-outline
paint remains distinct from unsupported paint. Embedded shape text uses the
existing text renderer.

Detected unsupported geometry, styles, pen rendering and extension fields
produce `UnsupportedShapeFeature` in `ParseReport`, CLI conversion and WASM
inspection. `StoredPage` retains object boundaries for accessing payloads from
the original uncompressed page bytes. Model values retain custom path bytes,
unknown template IDs, pen IDs and unsupported paints.
Fields after an unknown preceding field are not decoded using guessed offsets.

## Regression evidence

Eighteen synthetic archive tests in `structural_shapes.rs` cover:

- Explicit geometry versus drawn bounds, rotation, independent outline/fill
  colors, alpha, default styles, reversed endpoints and SVG curves.
- Embedded Unicode text and UTF-16 spans, pen-reference order, nested objects,
  multiple layers and decoy text in unsupported objects.
- Every payload truncation, effect and geometry boundaries, oversized counts,
  non-finite coordinates, invalid widths and text/object limits.
- Unknown templates, verbs, preceding fields, future frames/masks, gradients,
  arrow/dash settings, and paths that lack an initial move.

The two-object preservation regression fails at `3c78cd2`: the old parser
returns zero elements. The updated parser returns both native objects. The
comparison used an isolated archived checkout and a separate Cargo target.
An image regression also verifies that the inherited shape angle is not
mistaken for a corner radius when it matches the image rotation.

Workspace tests, Clippy, formatting, Rust 1.88 and the WASM target check pass.
The external rich-text fixture passed during the migration. The
[historical fixture audit](fixture-validation.md) retained all 7,182 strokes
and 924,442 points, with all 21 media hashes verified; those inputs are retired.
A disposable synthetic archive was converted through the CLI to SVG and PNG
and visually checked for geometry, rotation, transparency and curved paths.
This is runtime coverage, not a Samsung reference comparison.

## Remaining limits and next work

Native contracts and synthetic tests do not establish Samsung visual equivalence.
Obtain real shape/line documents with matching Samsung PDF exports to measure
placement, template geometry and style fidelity. Rounded/specialized templates,
shape custom paths, arc/oval path commands, connector routing, pen simulation,
gradients, dashed/compound outlines and arrowheads remain incomplete. Known
basic templates may render approximately when unsupported adjustments exist.
Text wrapping, margins, gravity and embedded-object layout retain the existing
text-renderer limitations. An empty report does not certify a lossless render.
