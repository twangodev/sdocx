# Shape and line frames

## Native evidence

The native findings use Samsung Notes 4.4.45.37, arm64 `libSPenModel.so`, plus
the decompiled `SpenObjectShape`, `SpenObjectLine` and `shapeeffect` Java APIs.
The native sources remain ignored, alongside the APK analysis artifacts.

| Symbol/address | Contract |
| --- | --- |
| `ObjectShape::NewGetBinary`, `0x399b40` | Shape chain `0 + 6 + 7`; snapshots unrotated bounds, drawn bounds and rotation, then writes type 0 with drawn bounds and zero rotation. |
| `ObjectLine::NewGetBinary`, `0x386a64` | Line chain `0 + 6 + 8`. |
| `ObjectShapeBase::NewGetBinary`, helper `0x37c6b4–0x37ca9c` | Type 6 connection data and optional sized line color/style effects at field bits 2/3. |
| `ObjectShapeBinaryHandler::GetOwnBinary`, `0x3a8dd0–0x3a9228` | Type 7 geometry, an extra rectangle for outer type 7, then text/pen/fill fields. |
| `ObjectLineBinaryHandler::GetOwnBinary`, `0x38bc34–0x38bed8` | Type 8 line type, start direction, control points, endpoints, two rectangles and saved `f32` common rotation. |
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

At `0x399bb8–0x399be8`, the writer stores `GetRotation()` and temporarily
clears the common rotation only for shape objects. Shape geometry uses the
first type-7 rectangle and this angle, rather than the drawn type-0 bounds.

Type 8 fixed data starts with one-byte line type, one start-direction byte, one-byte
control-point count and pairs of `f64`. Two endpoint pairs, two four-`f64`
rectangles and a saved `f32` common rotation follow. Its minimum fixed size is 103 bytes.
Flexible bits 1/2 hold four-byte advanced-pen-settings/name IDs; bit 3 holds a native path.
Unknown preceding fields cannot be skipped by assuming an arbitrary width.
The [connector findings](connector-routing-findings.md) distinguish this saved
state from connection identities, native load adjustments and edit-time routing.

Both pen fields are signed string-resource references, confirmed by native
setters/getters through `StringIDManager`; neither encodes a color. Their raw
values, including negative sentinels, are preserved. Outline color comes from
the type-6 `LineColorEffect`.

### Legacy line pen field and string attachment

On complete valid type-8 input, flexible bit 0 precedes bits 1/2/3 and
contains an `i32` legacy pen-name ID followed by four uninterpreted bytes.
Model `ApplyOwnBinary` (`0x38c300–0x38c334`) reads the ID, then skips those
four bytes. Its second four-byte check can fail and still proceed
(`0x38c328`), so this establishes the complete eight-byte carrier rather than
strict malformed-input admission. Modern settings/name fields then follow;
the bit-3 path is consumed independently of bit 0 (`0x38c33c–0x38c4c0`).
When the current name is exactly `-1`, the legacy temporary is assigned
at `0x38c458`. Legacy `-1` skips lookup; other legacy values use an available
context manager before that store. Other current names, including other
negative values, take precedence. The inspected current own writer
(`0x38bc34–0x38bed8`) omits bit 0. The skipped bytes' meaning and historical
producer remain unknown; no actual saved field-0 fixture was executed.

Selected modern Shape/Line consumers use the attached WDoc note's
[string namespace](painting-source-findings.md#layer-records-and-the-10000-object-split).
For context-bearing objects, including reused/direct decoder calls,
Shape pen loading (`0x3ac380–0x3ac418`) and Line loading
(`0x38c358–0x38c40c`) call `GetString` then `Bind(String const*)` when a
manager is available, ignore its result and store the original wire ID after
normal return. This does not prove successful missing-reference admission:
`GetString` returns null for a missing ID (`0x2a6710–0x2a673c`), and the reached
binding chain can pass that null receiver to Base `String::CompareTo`
(`0xc49e4`), which dereferences it without a local guard. No crash was executed.

Fresh ordinary WDoc loading has a different order. The actual factory Shape/Line
branches (`0x36dba8/0x36dbe8`, `0x36d8dc/0x36d910`) reach the BaseImpl constructor's
explicit null context store (`0x2d718c`). `SetLoadByCoedit` writes only its flag;
the inspected shared modern decoding prefix keeps the object unattached.
On the selected ordinary Line and text-absent Shape paths, pen readers therefore
skip string lookup/binding and retain signed IDs without validating their strings.
The fresh Shape text helper's bit-0-absent branch skips text construction
(`0x3b2214–0x3b2264`). Later checked layer insertion (`0x34e624–0x34e640`)
calls `OnAttach`; Base attachment then installs the supplied context (`0x2cfe70`).
Shape attachment next checks the supplied context's `+608` callable: nonzero
returned `w0` skips forwarding to embedded TextCommon (`0x39a664–0x39a690`).
Otherwise an existing TextCommon receives that context (`0x39a6a0`) and stores
it in TextImpl `+104` (`0x3e4de4`), distinct from owner BaseImpl `+56`.
A null supplied context does not create a new context in these attachment bodies.
This static proof does not close present-text Shape callbacks, arbitrary callback
reentry, malformed records or whole-note load success; it does not change the
context-bearing reader limitation above.

Derived pen attachment is subject to the context's mode gate
(`0x39a65c–0x39a6b0`, `0x386fbc–0x387008`). It calls `Bind(int)` only for
nonnegative IDs and ignores
failure (Shape `0x3ae59c–0x3ae644`; Line `0x3895a0–0x389648`). Without queued
local strings, those branches leave saved IDs unchanged. Queued strings can
overwrite IDs during attachment. Native Copy obtains current source strings
and passes them to destination setters (Shape `0x39819c–0x3981cc`; Line
`0x38922c–0x38925c`), binding strings rather than cloning numeric IDs or source bytes.
These are selected source contracts, not a pen-rendering parity result.

Rust retains the complete legacy carrier in `NativeLine.legacy_pen_source`,
separately from modern IDs, and reads the following known fields and path.
It rejects incomplete carriers, unlike the native second-check continuation
above. Pen rendering and string resolution remain unsupported.

Native WDoc paths start with a `u32` command count. Move/line commands have two
`f64` values, quadratic/oval four, cubic/arc six, and close none. The type-8
field has no separate byte-length prefix. Known command widths locate the
following field; an unknown verb makes the remaining bounded bytes opaque.
The [path findings](shape-path-findings.md) recover runtime float narrowing,
curve expansion and the distinct Model and Drawing contour contracts.

The Java constants identify oval 1, triangle 2, right triangle 3, rectangle 4,
rounded rectangle 5 and diamond 8. Line types are straight 0, elbow 1 and
curve 2. Unknown values must remain identifiable rather than becoming a
rectangle or straight line.

## Implemented model and rendering

`PageElement::Shape(NativeShape)` and `PageElement::Line(NativeLine)` expose
bounded geometry, outline/fill effects and native pen references. Shape text
reuses the rich-text decoder and its text/span/nesting limits. Object type and
declared frame boundaries determine decoding.

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
image regression also verifies that the inherited shape angle is not
mistaken for a corner radius when it matches the image rotation.

The external rich-text fixture passed validation. The
[historical fixture audit](fixture-validation.md) retained all 7,182 strokes
and 924,442 points, with all 21 media hashes verified; those inputs are retired.
A disposable synthetic archive was converted through the CLI to SVG and PNG
and visually checked for geometry, rotation, transparency and curved paths.
This is runtime coverage, not a Samsung reference comparison.

## Rendering limits

Native contracts and synthetic tests do not establish Samsung visual equivalence
for every shape/line variant. The [fixture 02 comparison](shapes-dot-calibration-findings.md)
covers its saved shape paths and diagonal line. Rounded/specialized templates,
arc/oval path commands, connector routing, pen simulation,
gradients, dashed/compound outlines and arrowheads remain incomplete. Known
basic templates may render approximately when unsupported adjustments exist.
Text wrapping, margins, gravity and embedded-object layout retain the existing
text-renderer limitations. An empty report does not certify a lossless render.

## Saved shape paths (fixture 02)

The 02 fixture contains explicit drawing paths for all five shapes, including
pentagon 11 and hexagon 6. The shared SVG converter uses those paths before
falling back to the basic templates. Adjustment control points are retained;
a complete saved path already expresses their effect. Unsupported verbs or
trailing bytes reject the entire path rather than substitute a basic shape.

Additional arm64 evidence from the same APK:

- `ObjectShapeImpl::GetPath` (`0x3a5b04`) and
  `ObjectShapeBinaryHandler::GetOwnBinary` (`0x3a8e60`) both read the template
  at implementation offset 32 and call `0x20d630`, returning its drawing path.
- `ObjectShapeData::SetRotation` (`0x3abc04`) reaches `0x20cb6c`, then
  `0x20d9e8`: it copies the original path into the drawing path and rotates
  its coordinates. `ObjectShapeDrawing::ShapePathType::GetPath` (`0x8bc90`
  in `libSPenDrawing.so`) consumes that same path. Applying the saved angle
  again would double-rotate it. Path-less basic templates still need the angle.

Synthetic regressions cover adjusted paths, rotations, fill/outline alpha,
quadratic/cubic segments, unknown template IDs with explicit geometry,
truncation, non-finite coordinates, unknown verbs and trailing path bytes.

## Shape orientation and text-editability properties (`0x01`, `0x02`, `0x04`)

Type-7 property bits 0/1 retain the actual horizontal/vertical orientation as
`NativeShape::horizontal_flip` and `NativeShape::vertical_flip`. These differ
from type-0 `metadata.flip_enabled`, which records permission to flip rather
than the current orientation. Model writer `GetShapeBinary_PropertyFlag`
(`0x3a7f84`) calls horizontal getter `0x20d8c4` and ORs `0x01` at `0x3a7fb0`;
vertical getter `0x20d928` supplies `0x02` at `0x3a7fc8`. The saved-shape loader
passes those decoded flags at `0x3a95ac`/`0x3a95b8` into common loader
`0x20bad0`, which stores orientation at bytes 16/17. Rendering the saved path
does not require another flip; template text-frame/control replay uses these
actual flags independently of capability.

The arm64 `libSPenModel.so` identifies type-7 property bit 2 as text
editability. `GetShapeBinary_PropertyFlag` (`0x3a7f84`) reads byte 21 of
`ObjectShapeText` (the shape pointer at offset 56) and writes bit 2.
`ApplyShapeBinary_Format28Data` (`0x3a8674`) extracts bit 2 at `0x3a86e4`
and calls `ObjectShapeText::SetTextEditable(bool)` at `0x3a86e8`.
That setter (`0x3b3108`) reads the same member at `0x3b3138`.

The decoder exposes this as `NativeShape::text_editable`. It controls editing
permission; saved geometry rendering does not depend on it. Fixture 02 sets
this bit on all five shapes; the decoder accepts it without unsupported
geometry warnings. Other unknown property bits and trailing geometry bytes
still produce separate diagnostics. Synthetic tests cover both editability
values, unknown bits alone and combined with `0x04`, and unchanged SVG output.
