# Shape and line frames

## Native evidence

This milestone uses Samsung Notes 4.4.45.37, arm64 `libSPenModel.so`, plus
the decompiled `SpenObjectShape`, `SpenObjectLine` and `shapeeffect` Java APIs.
The native sources remain ignored, alongside the APK analysis artifacts.

| Symbol/address | Contract |
| --- | --- |
| `ObjectShape::NewGetBinary`, `0x399b40` | Shape chain `0 + 6 + 7`. |
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

Type 6 fixed data contains a `u32` magnetic-point count and pairs of `f64`,
a `u32` connection-block size followed by that block (beginning with a `u32`
record count), then one reserved byte. The size excludes its own size prefix.
Type 6 has no fill color; its color effect describes the outline.

Type 7 fixed data begins with a `u32` shape type, four `f64` local coordinates,
an `f32` corner value, a sized path and a one-byte control-point count with
16 bytes per point. Outer shape objects then append another four `f64`
rectangle coordinates; images and text boxes omit this rectangle. Flexible
fields include sized `TextCommon` at bit 0, one text-control byte at bit 1,
pen reference at bit 2, pen color at bit 4 and a sized fill at bit 5. The fill
size excludes both the size prefix and the following one-byte effect kind.
Color fills use effect kind 1; image fills use kind 2.

Type 8 fixed data starts with one-byte line type, one routing byte, one-byte
control-point count and pairs of `f64`. Two endpoint pairs, two four-`f64`
rectangles and a four-byte setting follow. Its minimum fixed size is 103 bytes.
Flexible bits 1/2 hold four-byte pen/color values; bit 3 holds a native path.
Unknown preceding fields cannot be skipped by assuming an arbitrary width.

The Java constants identify oval 1, triangle 2, right triangle 3, rectangle 4,
rounded rectangle 5 and diamond 8. Line types are straight 0, elbow 1 and
curve 2. Unknown values must remain identifiable rather than becoming a
rectangle or straight line.

## Implementation target and limits

Decode bounded geometry and effects into dedicated shape/line model values,
retain native references and unsupported geometry, and replace the remaining
UUID/text scanning in page parsing. Render common geometric shapes and straight
lines using explicit styles. Preserve embedded shape text through `TextCommon`.

Native layout evidence does not establish Samsung visual equivalence. Custom
paths, specialized templates, connector routing, gradients, compound outlines
and arrowhead fidelity need further work and reference exports. Real Samsung
shape/line fixtures and matching PDFs remain necessary for visual comparison.
