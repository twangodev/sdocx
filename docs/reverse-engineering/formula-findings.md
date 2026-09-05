# Native formula records

## Evidence

Analyzed Samsung Notes 4.4.45.37, APK SHA-256
`daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667`.
Addresses are ARM64 virtual addresses in `libSPenModel.so` unless another
library is named. Writers, readers and getters establish the binary layout;
these findings do not depend on new Samsung-generated documents.

| Symbol | Address | Confirmed behavior |
| --- | --- | --- |
| `ObjectFormula::NewGetBinary` | `0x425860` | Writes base and formula frames, chain `0 + 11` |
| `ObjectFormulaImpl::GetOwnBinary` | `0x4333f4` | Type-11 frame with one property byte, two field bytes and no current fixed data |
| `ObjectFormulaImpl::getBinary_Property` | `0x42edb0` | Trigonometry and plottable flags |
| `ObjectFormulaImpl::getBinary_FlexibleData` | `0x42eddc` | Writes all 16 flexible fields |
| `ObjectFormulaImpl::applyBinary_FlexibleData` | `0x42fdd4` | Reads the same fields, including bit 3 before bit 2 |
| `ObjectFormulaImpl::getBinary_StrokeData` | `0x42f484` | Counted and separately sized stroke objects |
| `ObjectFormulaImpl::applyBinary_StrokeData` | `0x4306dc` | Creates type-1 objects from those bounded binaries |
| `ObjectFormulaImpl::getBinary_FlexiableDataLabelGraph` | `0x42f674` | Serializes labels, stroke-index sets, relations and graph endpoints |
| `ObjectFormulaImpl::applyBinary_FlexiableDataLabelGraph` | `0x430824` | Restores the same nested layout |
| `SPen::ReadString2` | `0x2788ac` | Reads an unsigned `u16` UTF-16 unit count without a null sentinel |

The base and formula calls occur at `0x4258a0` and `0x4258b8`. The current
formula header is 15 bytes; the flexible offset is 15 when fields exist and
zero otherwise. Formula objects do not inherit shape frames.

Property bit 0 is `has_trigonometry_calculation`, confirmed by
`ObjectFormula::HasTrigonometryCalculation` at `0x42964c` reading implementation
byte 269. Bit 1 is `plottable`, confirmed by `ObjectFormula::IsPlottable` at
`0x42a0d4` reading byte 281.

## Flexible fields

Physical order is **0, 1, 3, 2, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15**.
The nine-patch rectangle precedes the image media ID even though its bit is
higher. Both writer and reader establish this ordering.

| Bit | Encoding | Meaning | Getter or implementation offset |
| ---: | --- | --- | --- |
| 0 | LaTeX list | Input expressions | `GetLatex` at `0x42e768`, vector at 16 |
| 1 | Four `f64` coordinates | LaTeX result rectangle | `GetLatexResultRect` at `0x42e828`, member at 144 |
| 3 | Four `i32` coordinates | Nine-patch rectangle | `GetNinePatchRect` at `0x425fb4`, member at 212 |
| 2 | `i32` | LaTeX image media ID | Image ID at 208, mapped through image-common state at 232 |
| 4 | LaTeX list | Calculated LaTeX results | `GetLatexResult` at `0x432678`, vector at 40 |
| 5 | `u32` | Angle mode | `GetAngleType` at `0x43271c`, member at 284 |
| 6 | `f32` | Font size | `ObjectFormula::GetFontSize` at `0x4298d0`, member at 272 |
| 7 | Stroke list | Source strokes | Object list at 88 |
| 8 | Stroke list | Answer strokes | Object list at 104 |
| 9 | `u16` unit count, UTF-16LE | Answer string | `GetAnswerString` at `0x432704`, string at 128 |
| 10 | `u32` ARGB | Answer stroke color | `GetAnswerStrokeColor` at `0x4326c4`, member at 120 |
| 11 | Four `f64` coordinates | Relative original formula rectangle | `GetRelativeOriginalFormulaRect` at `0x42e848`, member at 176 |
| 12 | Four `f64` coordinates | Relative original answer rectangle | `GetRelativeOriginalAnswerRect` at `0x42e868`, member at 192 |
| 13 | `u32` | Expression type, raw enum | `ObjectFormula::GetExpressionType` at `0x42a540`, member at 288 |
| 14 | Label graph list | Recognition structure | Vector at 296 |
| 15 | LaTeX list | Substitutions | `GetSubstitutionLatex` at `0x42e7ec`, vector at 64 |

Each LaTeX list starts with a `u32` count. Each string has a `u16` byte count
followed by UTF-8, with no terminator or null sentinel. The getter mappings
distinguish calculated results in bit 4 from substitutions in bit 15.

Bit 9 uses a different representation. The call at `0x42ffa4` reaches
`ReadString2`: `0x2788e0` reads the unsigned 16-bit length, `0x2788e8` doubles it,
and `0x278904` passes those UTF-16 units to `String::Set`. There is no sentinel
branch: `0xffff` means 65,535 units. Other native string readers have nullable
lengths; their behavior must not be applied to this field.

The writer omits angle mode zero, font size zero and answer color
`0xff000000`. The SDK preserves absence separately from stored values. Angle
modes use `MathAngleType`; unknown expression values remain `expression_type_raw`.
The image media ID remains signed because the native reader only remaps
nonnegative values. This inspection API does not resolve the image asset or
apply nine-patch scaling.

## Embedded strokes

Both stroke lists use the same encoding:

```text
u32 stroke_count
repeat stroke_count:
    u32 object_byte_count
    u8[object_byte_count] base_and_stroke_frames
```

Each object is a complete `0 + 1` frame chain. The size excludes its own prefix;
there is no outer page-object hash or child count. The reader constructs type 1
at `0x43073c`–`0x430744`. `FormulaStroke` exposes common metadata, decoded stroke
channels and the complete original nested bytes, including frame extensions.

## Label graphs

```text
u32 graph_count
repeat graph_count:
    u32 label_count
    repeat label_count:
        u32 text_byte_count
        u8[text_byte_count] text_utf8
        f64 left, top, right, bottom
        u32 stroke_index_count
        u32[stroke_index_count] stroke_indices
    u32 relation_count
    repeat relation_count:
        u32 from_label
        u32 to_label
        u32 kind
    u32 start_label
    u32 end_label
```

The writer calls `U32string2string` at `0x42f870`, converting the native char32
label text to UTF-8. Its length prefix is **32-bit bytes**, unlike the 16-bit
LaTeX prefixes and the answer's UTF-16 unit count. The rectangle is written at
`0x42f91c`, index count follows at `0x42f930`, and index values are written at
`0x42f968`. The native label struct is 64 bytes: string at 0, rectangle at 24,
and index set at 40. Its in-memory layout is not the wire layout.

The index reader at `0x430aa4` sign-extends a 32-bit value into a native set.
The SDK retains its original `u32` bits in `stroke_indices`. The recognition
bridge below establishes that these values refer to recognition strokes;
the SDK does not assume they directly index the formula's stored stroke list.
Native relations occupy 24 bytes but only write 12.
The three reader stores are at `0x430cf4`, `0x430d14` and `0x430d30`.

`LabelGraph::PrintLabelGraph` at `0xb27b4` in `libSPenBase.so` confirms that the
first two relation values index the labels: loads at `0xb2940` and indexing at
`0xb2944` select 64-byte label records. The third value selects a relation name
for the printed separator. `FormulaLabelRelation::kind` names the eight values
established by the native lookup table below, while `kind_raw` retains the
original bits. Out-of-range references remain inspectable values; the API never
indexes with them.

The two graph-tail reads at `0x430e58` and `0x430e70` widen `u32` wire values
into native members at offsets 48 and 56. They are the start and end label
indices, exposed as `start_label` and `end_label`. Values are preserved even
when they do not resolve to a label in the same graph.

### Recognition bridge

The APK also contains `libSPenRecognizerMathRecognition.so` and
`libSPenRecogUIFeature.so`. In the recognition library,
`Math::HME::Expression::LabelGraph::Node::getStrokeIndexes` at `0xe0200`
returns the set at offset 40. Its neighboring getters establish the label at
offset 0 and rectangle at offset 24 (`0xe01f4`, `0xe01f8`).

A conversion helper at `0x1d10b0` in the UI library walks the recognizer's nodes
and constructs 64-byte labels. At `0x1d1228` it calls `getStrokeIndexes`, then
copies that set into the destination label at offset 40 (`0x1d1238`–`0x1d1240`).
The node loop uses `x21` as its zero-based index, initialized at `0x1d1100`
and incremented at `0x1d152c`. It compares the current node with
`getStartNode` and `getEndNode` at `0x1d1428` and `0x1d1448`. Matching indices
are stored at destination graph offsets 48 and 56 at `0x1d1440` and `0x1d1460`.

`SPen::MathUtils::convertLabelGraph` connects the recognizer-facing
`HwrLabelGraph` and serialized `SPen::LabelGraph` forms in both directions
(`0x1d61f8` and `0x1d66fc`). It copies the label's index set at offset 40;
the relevant source/destination accesses are `0x1d62e4`–`0x1d62ec` and
`0x1d6324`–`0x1d632c`. The endpoint pair at offsets 48/56 is copied together
at `0x1d65d0`–`0x1d65d4` and `0x1d6a84`–`0x1d6a88`. This ties the recognition
getter meanings to the members written by the formula serializer.

### Relation names

The initializer at `0xb2f20` in `libSPenBase.so` copies 128 bytes from
`0xecfe8` at `0xb2f38`–`0xb2f4c`, then constructs the map at `0xf7a08` with
eight entries at `0xb2f54`–`0xb2f68`. Each entry occupies 16 bytes: a 32-bit
enum key, alignment padding and a relocated pointer to a string. The print
routine looks up the relation's member at offset 16 in that same map, whose
tree root is at `0xf7a10`.

| Raw value | Native name | Entry address | String address |
| ---: | --- | --- | --- |
| 0 | Unknown | `0xecfe8` | `0x3252e` |
| 1 | Right | `0xecff8` | `0x31da0` |
| 2 | Subscript | `0xed008` | `0x30f9f` |
| 3 | Superscript | `0xed018` | `0x31615` |
| 4 | Inside | `0xed028` | `0x2f21a` |
| 5 | Below | `0xed038` | `0x2d571` |
| 6 | Above | `0xed048` | `0x2c97e` |
| 7 | Index | `0xed058` | `0x2dd52` |

The pointers are `R_AARCH64_RELATIVE` relocations, so reading only their
unrelocated file bytes would miss the names. `FormulaLabelRelationKind` keeps
native zero (`Unknown`) distinct from unmapped numbers (`Other(u32)`). These
names establish the stored categories; layout rules and the exact interpretation
of `Index` still need tracing.

## SDK inspection and validation

`StoredObject::formula_metadata(page_bytes)` and its `_with_limits` counterpart
inspect outer type 11. `FormulaMetadata::parse_bytes` and
`parse_bytes_with_limits` accept a complete `0 + 11` payload directly, including
an entry from `MathMetadata::formula_objects`. Math-envelope inspection keeps
those entries raw and does not implicitly decode them.

`max_entry_size` bounds the input. Every list, including nested labels, index
values and relations, shares one formula-wide `max_objects_per_page` budget
reported as `math entries`. Minimum record sizes are checked before allocation.
Source and answer strokes also share `max_strokes_per_page`; each stroke obeys
`max_points_per_stroke`. All text fields obey `max_text_characters`, measured in
UTF-16 units. No formula execution or recursive math decoding occurs.

Complete masks and separate fixed, flexible and post-frame trailing data are
preserved. Known fields cover all bits through 15, so later unknown fields
remain in flexible trailing bytes. Ordinary page decoding still reports
`UnsupportedObjectType` for formulas: this API exposes stored structure and
does not add expression layout or rendering.

Twelve synthetic integration tests cover all fields and their native order,
truncation of every field prefix beside a decoy frame, nested strokes and
extensions, labels and relations, cumulative budgets, invalid strings and
rectangles, empty/surrogate-pair/65,535-unit answers, unknown masks/enums,
stored-object bounds and explicit inspection of an embedded math formula. The
relation test covers every mapped name, future values and an out-of-range
label reference without dereferencing it. Endpoint tests cover an empty graph
and valid start/end indices, while the complete fixture preserves unresolved
endpoints and raw stroke-index bits.

Remaining work includes expression enum semantics, matching recognition stroke
indices to stored strokes, image resolution, and native layout/evaluation. Samsung
SDOCX/PDF pairs are still needed to verify real writer variants and visual
output.

The drawing path now has a separate trace covering image/ink precedence,
placement dependencies and the expression setter's accepted range; see
[formula rendering findings](formula-rendering-findings.md).
