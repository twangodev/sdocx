# Native plot records

## Evidence

Analyzed Samsung Notes 4.4.45.37, APK SHA-256
`daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667`.
Addresses are ARM64 virtual addresses in `libSPenModel.so`. Native writers,
readers, getters and JNI field mappings establish this layout without new
Samsung-generated documents.

| Symbol | Address | Confirmed behavior |
| --- | --- | --- |
| `ObjectPlot::NewGetBinary` | `0x451908` | Writes base and plot frames, giving the chain `0 + 20` |
| `ObjectPlotImpl::GetOwnBinary` | `0x45315c` | Type-20 frame, one property-mask byte, two field-mask bytes, no current fixed fields |
| `ObjectPlotImpl::GetBinary_FlexibleData` | `0x453210` | Writes fields 1 through 5 in ascending bit order |
| `ObjectPlotImpl::ApplyBinary_FlexibleData` | `0x4537e4` | Also accepts a four-byte legacy field under bit 0 |
| `ObjectPlotImpl::GetCoordinateRect` | `0x453d5c` | Four floats at implementation offset 16 |
| `ObjectPlotImpl::GetCoordinateColor` | `0x453d8c` | ARGB value at offset 32 |
| `ObjectPlotImpl::GetBackgroundColor` | `0x453db8` | ARGB value at offset 36 |
| `ObjectPlot::GetAngleType` | `0x452354` | Four-byte mode at implementation offset 56 |
| `JNI_GraphDataList::GetJGraphData` | `0x454140` | Confirms graph expression, substitutions, color, width and visibility field names |
| `JNI_GraphDataList::GetCGraphData` | `0x4543bc` | Converts those Java fields back to the same native members |

The base and plot serialization calls occur at `0x451948` and `0x45195c`.
The current type-20 header is 15 bytes and uses offset 15 when flexible data
exists, or zero otherwise. No property bits have mapped semantics; the current
writer emits a zero property mask.

## Flexible fields

| Bit | Encoding | Meaning |
| ---: | --- | --- |
| 0 | `u32` | Legacy value; reader skips it, current writer does not emit it |
| 1 | Four `f64` coordinates | Plot coordinate rectangle |
| 2 | `u32` ARGB | Coordinate color |
| 3 | `u32` ARGB | Background color |
| 4 | `u32` count, then graph records | Graph expressions and styles |
| 5 | `u32` | Angle mode |

The coordinate rectangle is separate from base-frame placement. Its values are
widened from native floats on write and narrowed on read. Reversed axes remain
representable. Both color fields default to `0xff000000` in the omission checks.
The writer omits angle mode zero, unlike the math envelope's default of two.
`SpenObjectPlot.java` declares degree = 0, radian = 1 and all = 2.

The bit-0 reader at `0x453820`–`0x45383c` checks and advances four bytes without
assigning a member. Its original purpose is unresolved. The SDK retains the
value as `legacy_field_0` rather than guessing a media reference.

## Graph records

There is no size prefix or field mask around each graph:

```text
u16 latex_byte_count
u8[latex_byte_count] latex_utf8
u32 color_argb
f32 line_width
u8 visibility
u32 substitution_count
repeat substitution_count:
    u16 substitution_byte_count
    u8[substitution_byte_count] substitution_utf8
```

The writer accesses native graph members at offsets 0 (LaTeX string), 24
(substitution vector), 48 (color), 52 (line width) and 56 (visibility). The JNI
mapping resolves field-name strings `latex` at `0x157569`, `substitutionLatexs`
at `0x141af6`, `color` at `0x14c26b`, `lineWidth` at `0x14b7d7`, and `isShow`
at `0x14d1a8`. Stores at `0x454590`, `0x4545b8` and `0x4545cc` corroborate
the scalar layout.

The byte-text lengths count bytes, not UTF-16 units. Empty strings are valid;
there is no null-string sentinel in these records. The reader at `0x453a14`
compares visibility to exactly 1. `PlotGraph::is_visible` mirrors that behavior,
while `visibility_raw` preserves other values. Graph line widths are retained
as stored; this inspection API does not lay out or evaluate expressions.

## SDK inspection

`StoredObject::plot_metadata(page_bytes)` and `plot_metadata_with_limits`
decode only outer-type-20 records. `PlotMetadata` exposes the base metadata,
optional plot fields, ordered `PlotGraph` values, complete masks and separate
fixed/flexible/post-frame trailing data. Absent optional fields remain absent.
Unrecognized angle values use `MathAngleType::Other`.

`max_entry_size` bounds the selected object payload. Graphs and their
substitution strings share a per-object `max_objects_per_page` budget, reported
as `math entries`. Count checks include the minimum remaining bytes before
vector allocation: 15 bytes per graph and two bytes per substitution. Decoded
LaTeX strings obey `max_text_characters` measured in UTF-16 units, matching the
existing limit's meaning even though these fields use UTF-8 on disk.

Known fields cover bits 0 through 5, so later unknown fields remain as flexible
trailing bytes. The complete original payload remains available through
`StoredObject::payload`. The semantic page model still reports
`UnsupportedObjectType` for plots, because expression evaluation and graph
rendering are not implemented.

## Validation and next work

Five synthetic integration tests cover all six fields, multiple graph styles
and substitutions, all truncated field prefixes with a following decoy frame,
unknown masks/modes/visibility, zero offsets, cumulative counts, UTF-8 decoding
and UTF-16-unit limits, malformed types and invalid payload bounds. Existing
math-envelope tests exercise the shared size/count helpers.

Remaining work includes interpreting formula-to-plot relationships, native graph
evaluation and layout, and any captured-bitmap persistence in older variants.
Real SDOCX/PDF pairs are needed to compare plotted output and verify writer
variants beyond this APK.
