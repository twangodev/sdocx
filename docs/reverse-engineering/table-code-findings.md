# Native table and code-block records

## Evidence

Analyzed Samsung Notes 4.4.45.37, APK SHA-256
`daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667`.
Addresses are ARM64 virtual addresses in `libSPenModel.so` unless stated otherwise. This pass
uses native serializers/readers and synthetic records without new SDOCX files.

| Symbol | Address | Confirmed behavior |
| --- | --- | --- |
| `ObjectCodeBlock::NewGetBinary` | `0x474b7c` | Serializes `ObjectBase`, then code-block data |
| `ObjectCodeBlockImpl::GetOwnBinary` | `0x475178` | Typed frame 23 |
| `ObjectCodeBlockImpl::GetBinary_FlexibleData` | `0x475230` | Bits 0 and 1 contain separately sized title and body text objects |
| `ObjectTable::NewGetBinary` | `0x3d9f0c` | Serializes `ObjectShape`, then table data |
| `ObjectShape::NewGetBinary` | `0x399b40` | Shape-base chain, followed by the type-7 shape frame |
| `ObjectTableImpl::GetOwnBinary` | `0x3cf1ec` | Typed frame 22 |
| `ObjectTableImpl::GetBinary_FlexibleData` | `0x3cf310` | Size limits, column widths, rows and additional table styles |
| `TableRow::NewGetBinary` | `0x3c515c` | Untyped row record with a relative flexible offset and two masks |
| `TableRow::GetBinary_FixedData` | `0x3c5230` | Height, index and individually sized cells |
| `TableRow::GetBinary_FlexibleData` | `0x3c53cc` | Maximum height under bit 9, then minimum height under bit 1 |
| `TableRow::ApplyBinary_FlexibleData` | `0x3c5840` | Reads those two fields in the same nonnumerical bit order |
| `TableCell::NewGetBinary` | `0x3c2e4c` | Untyped cell record with a relative flexible offset and two masks |
| `TableCell::GetBinary_FixedData` | `0x3c2f50` | Cell coordinates, spans, color, editability and sized text |
| `TableCell::GetBinary_FlexibleData` | `0x3c3150` | Field bit 0 contains a sized `TableBorder` record |
| `TableBorder::NewGetBinarySize` | `0x3dc9a4` | Current border payload is 73 bytes |
| `TableBorder::NewGetBinary` | `0x3dc9ac` | Untyped header followed by four 16-byte edge styles |
| `JNI_TableCellBorder::ConvertToCBorderStyle` | `0x320254` | Maps Java color, width, start radius and end radius to the native edge fields |

## Object frame chains

The code-block chain is `0 + 23`. Its type-23 frame has no currently written
fixed fields. Flexible bit 0 contains a `u32` text-object payload size followed
by the title object; bit 1 uses the same layout for the body. Sizes exclude
their own four-byte prefixes. Missing title or body is representable.

The table chain is `0 + 6 + 7 + 22`. The call at `0x3d9f4c` uses
`ObjectShape::NewGetBinary`, which calls `ObjectShapeBase::NewGetBinary` at
`0x399c08` and writes its own shape frame at `0x399c20`. The existing embedded
decoder finds the type-22 frame after the base. The native table record also
contains inherited shape geometry and styles.

## Table properties and flexible fields

`ObjectTableImpl::GetBinary_Property` at `0x3cf2cc` writes heading-column
enabled under bit 0, heading-row enabled under bit 1, and maximum-height
**disabled** under bit 2. These map to implementation bytes 185, 184 and 186,
respectively. The setters at `0x3cef0c`, `0x3ceed8` and `0x3d17dc` confirm
the names and distinguish heading rows from heading columns.

The type-22 frame has no currently written fixed fields. Its flexible fields
are serialized in ascending bit order:

| Bit | Encoding | Meaning | Getter or implementation member |
| ---: | --- | --- | --- |
| 0 | `f32` | Global minimum column width | `ObjectTable::GetMinColumnWidth`, `0x3d4364`; member 12; default 10 |
| 1 | `f32` | Global minimum row height | `ObjectTable::GetMinRowHeight`, `0x3d36bc`; member 8; default 10 |
| 2 | `u32` count, then `f32[]` | Actual column widths | Vector at offset 32 |
| 3 | `u32` count, then sized row records | Rows | Each size excludes its own prefix |
| 4 | Four `f64` coordinates | Content bounds | `GetContentRect`, `0x3c6f6c`; rectangle at offset 16 |
| 5 | `u32` size, then border record | Outer table border | `GetBorderStyles`, `0x3cb7f8`; pointer at offset 152 |
| 6 | `u8` | Auto-fit mode | `GetAutoFitOption`, `0x3da474`; byte at offset 168 |
| 7 | `u32` count, then `f32[]` | Minimum column widths | `GetMinColumnWidth`, `0x3ca290`; vector at offset 56 |
| 8 | `u32` count, then `f32[]` | Maximum column widths | `GetMaxColumnWidth`, `0x3ca2b8`; vector at offset 80 |
| 9 | `f32` | Maximum table height | `GetMaxHeight`, `0x3d11a4`; offset 176 |
| 10 | `f32` | Maximum table width | `GetMaxWidth`, `0x3d11ac`; offset 180 |
| 11 | `u32` size, then border record | Default cell border | `GetDefaultCellBorderStyles`, `0x3cb920`; pointer at offset 160 |
| 12 | `u32` ARGB | Heading background color | `GetHeadingBackgroundColor`, `0x3cc03c`; offset 188 |
| 13 | `u32` ARGB | Default cell background color | `ObjectTable::GetDefaultCellBackgroundColor`, `0x3d7ec0`; offset 192 |

The constants in `SpenObjectTable.java` identify auto-fit values 0 as none,
1 as horizontal, 2 as vertical and 3 as both. The writer omits the default
value 3. The SDK represents absent optional fields as `None` and preserves
unrecognized mode bytes as `TableAutoFit::Other`; it does not materialize
native defaults into fields that were absent from the file.

Bits 0/1 were previously mislabeled as cell padding. The reader stores them
at members 12/8 (`0x3cfb38`–`0x3cfbd4`); the public getters establish their
size-limit meanings independently. Rust fields and serialized inspection keys
are now `min_column_width` and `min_row_height`; deserialization also accepts
the old JSON keys. Rust field access and emitted inspection keys change, while
the raw optional values and rendering behavior are unchanged. Cell text margins
are separate: absent
margins default to zero in `TextCommonImpl` (`0x3e747c`–`0x3e7484`), and the
table-cell constructor does not replace them.

Cold Drawing measurement reads saved row heights and column widths directly
(`0xaaa60`, `0xaaab8`) and does not consult auto-fit mode. Model auto-fit acts
through `fitTableRect` and `SetAutoFitOption` (`0x3c6b94`, `0x3cbaa0`), outside
that cold measurement path. An absent mode still means native default 3;
accepting its existing geometry does not interpret absence as mode 0.

`RichTextTable.style` now exposes these properties alongside existing actual
column widths, rows and cells. `TableRecordMetadata` retains complete masks
and separate fixed/flexible trailing bytes for table, row, cell and border
records. The complete embedded object also remains available through
`RichTextObjectSpan.object_data`, including inherited shape frames.

## Row and cell boundaries

Rows and cells are sized by their parent and do not have typed-frame headers.
Their own records begin with:

```text
u32 flexible_offset_relative_to_record_start
u8 property_mask_length
property_mask_bytes
u8 field_mask_length
field_mask_bytes
fixed_data
flexible_data
```

The current native writer uses one property-mask byte and two field-mask bytes,
giving a nine-byte header. It writes offset zero when no flexible fields are
present; otherwise it records the end of the fixed data. Parent size prefixes
are excluded from this relative offset.

A row's fixed data is `f32` height, `u32` row index, `u32` cell count, then a
`u32` payload size and cell record per cell. A cell's fixed data contains
`u32` column index, row span, column span and ARGB background; four `f64`
coordinates; `u8` editability; and a sized rich-text object.
Cell property bit 0 identifies an owned background color.

The editability byte was previously mislabeled as vertical alignment. The
writer loads member 80 at `0x3c30a0` and writes it at `0x3c30ac`.
`TableCell::SetEditable`, `0x3c24a0`, stores the same member at `0x3c24b0`;
`IsEditable`, `0x3c24c4`, reads it directly. The reader loads the byte at
`0x3c344c`, compares it with zero at `0x3c3458`, uses `cset ne` at
`0x3c345c`, then stores the boolean at `0x3c3460`. Every nonzero byte means
editable, including 2 and 255. Rust exposes `editable: bool` with this
normalization; the retained embedded object record preserves the original bytes.

The SDK now bounds each fixed reader at the declared flexible offset. Previously
it only rejected offsets beyond the record after parsing the fields, allowing
a malformed fixed-field length to consume flexible bytes. Invalid offsets now
fail before those reads. Offset zero remains valid with an empty field mask.
The shared mask reader supports wider future masks without truncating the
check for set bits. Unknown embedded bytes remain in the original object data.

## Row height and cell-border findings

Row flexible fields are **not** in ascending bit order. Both writer and reader
place `f32` maximum height under bit 9 before `f32` minimum height under bit 1.
`GetMaxHeight` at `0x3c4244` reads member offset 124; `GetMinHeight` at
`0x3c4300` reads offset 128, matching the serialization accesses. Default maximum
height is `f32::MAX`, and default minimum height is zero. `RichTextTableRow`
now exposes both values when present. If any unknown row field bit is set,
neither constraint is decoded, and the complete flexible payload is retained.
The nonnumerical native order does not establish where an unknown field would
appear, so decoding known fields past that uncertainty would be unsafe.

Cell flexible bit 0 contains a four-byte size followed by a table-border
payload. The analyzed writer writes size 73 and then calls
`TableBorder::NewGetBinary` (`0x3c31a0`–`0x3c31bc`). `RichTextTableCell.border`
decodes that record within the declared payload size. Its bytes cannot be
treated as additional cell text.

## Shared cell text layout

Addresses in this section refer to the named libraries rather than Model.
Drawing `ObjectTableCellLayout` derives from ordinary `ObjectTextLayout`:
its constructor calls the base constructor at `0x8c008`. Its `updateBound`,
`0x8c328`, calls the base implementation at `0x8c338`, then unconditionally
sets the underlying `TextLayout` gravity to 0 at `0x8c33c`–`0x8c34c`.
This is top gravity and overrides the cell content's stored gravity. The
editability byte must not select top, center or bottom gravity.

Widget `ObjectTextLayout::updateBound`, `0xd713c`, reads the content object's
four margins and multiplies each by document density and local text scale
before `TextLayout::SetMargin` at `0xd7220`. The cell adapter retains this
margin setup before overriding gravity. Model `TableCell::SetMargin`,
`0x3c2554`, delegates to the content object's `ObjectShapeText::SetMargin`
at `0x3c2560`; it does not maintain a separate cell alignment field.

Drawing `TableLayout::updateCell`, `0xae914`, supplies the measured cell-frame
width and height through `ObjectTextLayout::SetLayoutWidth` at `0xae9f4` and
`SetLayoutHeight` at `0xaea04`. `TableLayout::layoutCell`, `0xb06d4`, supplies
the same dimensions at `0xb07bc` and `0xb07cc`, then lays out the complete
text range at `0xb07dc`–`0xb07f0`. All cell lines consequently pass through
the shared text engine; native drawing does not extract the first physical line.

These findings establish adapter inputs and editability semantics. They do
not prove complete row sizing, merged-cell layout, page splitting or visual
parity for arbitrary cells.

## Export row sizing and merged-frame ownership

The ARM64 Drawing library used here has SHA-256
`788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd`;
Model has SHA-256
`4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a`.

Drawing's cold `TableLayout::init`, `0xaa6b4`, reads actual row heights at
`0xaaa60` and column widths at `0xaaab8`. `extendRowBySplit`, `0xaaf2c`,
measures cell height differences and grows the row for a strictly positive
maximum difference (`0xab0ac`–`0xab0f0`). The warm
`updatePositionFromRow`, `0xaecf4`, selects the largest measured frame-owner
height across every column position. Only a result below the small-height threshold is replaced by
`ObjectTable::GetMinRowHeight` (`0xaedfc`–`0xaee10`); the stored minimum is
not an unconditional lower clamp on every nonempty row.

These selected cold/warm export routines do not read the saved row maximum.
Model exposes that value through `ObjectTable::GetMaxRowHeight`, `0x3d3844`,
and `TableRow::GetMaxHeight`, `0x3c4244`, but its presence must not reject the
export preparation path or introduce a maximum-height clamp. Rust accepts
saved row maxima while retaining their parsed values. The table-wide
maximum-height flag is separate and still outside the supported prepared path.

### Warm row sizing from cached owner measurements

For each row from the supplied start index onward, Drawing
`updatePositionFromRow`, `0xaecf4`, calls `ObjectTable::GetFrameCell` for each
column (`0xaed90`). It looks up the owner's cached text layout and skips null
layouts. Widget `ObjectTextLayout::GetMeasuredHeight`, `0xd3b78`, reads the
cached `f32` at offset 404; no text shaping occurs in this routine. An owner
from an earlier row contributes its full measured height again. Covered raw
cells' cached heights do not contribute, even when those cells appear in the
separate paint-visible list.

The maximum starts at zero. A result strictly below `0.001f` is replaced by
the indexed saved row minimum. Heights at or above that threshold are not
clamped to the minimum. The saved row maximum is ignored. Only an absolute
`f32` difference strictly greater than `0.001f` changes geometry: `extendRow`
changes the physical row height, then `offsetFromRow` moves subsequent rows.
Rows before the start index retain their frames.

[The warm-row capture](../../conformance/table-warm-rows.json), SHA-256
`4a1b17063dadcf9d8d3ca819dbf92b6f75f689f87b3e1f38ab4369d3f215610e`,
contains 142 inputs and 1,189 slot frames. It covers merged owners, covered-span
chains, supplied/null layout measurements, threshold-adjacent heights, saved
minima/maxima, start indices and pending gaps. Each input produces identical
outputs with allocation fills `0x00`, `0xa5` and `0xff`. The
[Rust capture module](../../conformance/native_table/rows.rs) checks Model,
Drawing and Base hashes recorded below, plus Widget SHA-256
`cfaaccbfd62763f0e514271cc372c0de7b6df41f0d2f991887b8b9584abd1ec9`.

The [Rust regression](../../crates/sdocx/src/render/table/pagination/native_row_tests.rs)
matches every coordinate and pending-gap bit using production row updates.
Preparation caches the bounded `TableGrid` ownership map once and warm sizing
reads owner measurements through that map. Export preparation still accepts
only unmerged grids. The capture supplies measured heights directly and uses
the isolated text initialization described below; it does not establish native
text measurement, complete merged pagination or device appearance.

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --warm-rows scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenWidget.so
```

### Cold frame cache and row updates

`TableLayout::init`, `0xaa6b4`, starts grid coordinates at the half-width of
the widest drawable outer border. Missing serialized border styles retain
Model's constructed one-unit black border, giving a half-width of 0.5. Each
stored slot receives a rectangle with its saved column width and row height.
The next row starts at the preceding row's first-slot bottom
(`0xaaa6c`–`0xaaa80`); the next column starts at the preceding column's right
in the first grid row (`0xaaac8`–`0xaaad8`). Native Base `RectF::OffSet`,
`0xb11a4`, adds those origins using `f32` arithmetic. The initializer also
stores the rectangle in a cache keyed by the raw cell pointer
(`0xaac90`–`0xaae08`). It does not expand rectangles by row/column spans in
this path. This establishes initial cache geometry, not final merged layout.

Drawing `extendRow`, `0xaff74`, adds its supplied amount to the bottom of
every stored slot and corresponding cached frame in that row. It does not
move later rows. `offsetFromRow`, `0xade0c`, moves stored slots and cached
frames from a supplied row onward. Before movement, a pending gap of at least
`f32::EPSILON` changes the amount: positive movement consumes the gap and
stops if more than epsilon remains. Negative movement clears the gap and
recursively removes its height before applying the original movement. These
two routines use stored cells directly, independently of frame ownership.

[The frame capture](../../conformance/table-cold-frames.json), SHA-256
`498bfb15e663cfeb648ed51a8c0ed2a2490b09fa47b578d1419e0d7eac0cdb84`,
records 93 inputs, 1,082 initial/final slot frames and 532 explicit row changes.
Cases include unit/merged grids, covered-span chains, fractional/large values,
zero saved row heights, border-width selection, gap consumption/removal and
deterministic sequences of row updates. All outputs are identical with fresh
allocations filled by `0x00`, `0xa5` or `0xff`. The capture hash checks Model
and Drawing as above, plus Base SHA-256
`e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb`.

The [Rust harness module](../../conformance/native_table/frames.rs) runs native
frame initialization, row updates, rectangle arithmetic, containers and callback
copies. Local ELF relocations and explicit PLT bindings resolve those native
calls. Text initialization is isolated: the harness replaces the base
text-layout constructor with zeroed storage and omits `SetObject` and
`SetTextScale`. Update amounts
and pending gaps are supplied inputs. Consequently this capture does not prove
native text measurement, the decision to grow a row, final merged-frame sizing,
pagination selection or device appearance.

The [Rust regression](../../crates/sdocx/src/render/table/pagination/native_frame_tests.rs)
matches initial/final coordinate bits and final pending-gap bits for every input.
It uses the same frame initializer and row updates as export preparation, before
text measurement. Export still restricts preparation to unmerged grids; matching
these frame primitives does not establish complete merged-table support.

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --cold-frames scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenBase.so > /tmp/table-cold-frames.json
cmp /tmp/table-cold-frames.json conformance/table-cold-frames.json
```

### Measured bounds and first-page minima

Drawing `updateMeasuredRect`, `0xab168`, resolves `(0,0)` and the last grid
position through `GetFrameCell`. It reads their cached rectangles and unions
those two frames with Base `RectF::ExtendRect`, `0xb1724`; it does not union
every stored cell. The union is the content rectangle at layout offset 652.
Model's four `GetDrawnBorder*Width` getters supply independent edge widths.
Each width is the maximum of the outer edge and borders on the corresponding
physical grid edge, including inherited default cell borders. The getters read
stored widths without filtering transparent colors. Drawing expands each side
by half its width, then offsets the measured rectangle to origin `(0,0)` using
Base `RectF::OffSet`. The expanded rectangle is at layout offset 668.

Drawing `getMinRowHeightInFirstPage`, `0xac9f0`, resolves every column in the
requested row through its frame owner. An owner's nonempty text layout
contributes its first-line height plus top margin. An absent text layout or
empty text contributes the cached measured height. Widget `GetTextLayout`,
`0xd39ac`, reads offset 368; `GetMeasuredHeight`, `0xd3b78`, reads offset 404.
The virtual getters called at `0xaca98`, `0xacab4` and `0xacacc` are Text
`GetTextLength`, `0x8b104`, `GetLineHeight`, `0x8b3ac`, and `GetTopMargin`,
`0x8b90c`. The branch tests text length, not line count.

The maximum starts at zero. If it remains zero, the routine uses the physical
first row's height, even for a request concerning a later row. It does not
substitute the requested row's saved minimum. When the origin flag is enabled,
the result includes `content.top - measured.top`; after measured-origin
normalization this is the content's top coordinate. `GetMinHeightInFirstPage`,
`0xac9d4`, requests row zero with this flag enabled.

[The geometry capture](../../conformance/table-measured-geometry.json), SHA-256
`e877970b7966c5d10c9df478c4d608d9d56db26f288f7e1ec0d68adeffad09fa`,
records 138 table states, 1,185 supplied/cold slot frames and 818 minimum-height
queries with both origin-flag values. Cases include endpoint owners from earlier
rows/columns, covered-span chains, endpoint-cache union, asymmetric edge widths,
nonempty/empty text, absent text layouts and the first-row fallback. Every output
is identical with fresh allocation fills `0x00`, `0xa5` and `0xff`.

The [Rust capture module](../../conformance/native_table/geometry.rs) checks the
Model, Drawing, Base and Widget hashes recorded above, plus Text SHA-256
`5483711673a499743625eb3275e34b37a006919af346212b46b8d8857834308b`.
Bounds, border getters, rectangle arithmetic and text getters execute native
code. Text initialization remains isolated; cached frames and text metrics are
supplied inputs, with synthetic storage for the native getters. This establishes
selection and arithmetic, not text shaping, final merged frames, pagination
selection or device appearance.

The [Rust regressions](../../crates/sdocx/src/render/table/native_geometry_tests.rs)
match content/measured bounds, edge widths and every minimum-height query by
`f32` bits. Production bounds and first-page sizing use the same owner lookup
as warm row sizing. Export preparation remains restricted to unmerged grids.

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --measured-geometry scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenWidget.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenText.so > /tmp/table-measured-geometry.json
cmp /tmp/table-measured-geometry.json conformance/table-measured-geometry.json
```

### Row split caches and warm text selection

Drawing `updateSplitOfARow`, `0xb24cc`, resolves the first column's frame
owner (`0xb2524`). It selects supplied table split rectangles whose top is at
or below that owner's cached frame top, then offsets them by the negative owner
top using Base `RectF::OffSet`. Rectangle order is retained; it is not sorted.
The resulting native list is compared with the first owner's cached list.

`isSameRectList`, `0xb2e18`, first checks pointer identity and null pointers.
Distinct nonnull lists must have the same count and a first rectangle whose
four coordinates differ by at most `0.001f`, through Base `RectF::Equal`,
`0xb14b0`. Later rectangles are not compared. Distinct empty lists therefore
compare unequal: their first-rectangle lookups return null. A missing list and
a stored empty list are separate cache states.

If the comparison succeeds, the new list is discarded and every cell cache
remains unchanged. Otherwise the first owner's list is replaced. For columns
one onward, replacement happens only when `GetCell(row,column)` equals
`GetFrameCell(row,column)` (`0xb2710`–`0xb2734`). Those cells receive separate
copies of the first owner's local list, even when their own frame tops differ.
Covered raw cells retain their previous list or absent-cache state. A first
owner from an earlier row can make the comparison succeed before an independent
cell in the requested row has received any list.

Drawing `layoutRow`, `0xb0420`, also compares raw cells with frame owners before
`layoutCell` (`0xb0484`–`0xb04b4`, `0xb0644`–`0xb0674`). It lays out only cells
that own their frames. Its current/previous first-list lookups use first-column
frame owners (`0xb04f4`, `0xb050c`), independently of the physical row frames
that row offsets move. These text-selection branches are disassembly findings;
the split capture below does not execute text layout.

[The split capture](../../conformance/table-row-splits.json), SHA-256
`0e77850a6312268018e9e27ceeab0dc7f41654690de13e0171982b9fa6f2ff2e`,
records 139 inputs, 1,068 split updates and 10,828 cell-cache snapshots. Cases
include column/row merges, owners from earlier rows, covered-span chains,
first-rectangle threshold boundaries, changes to later rectangles, empty lists
and unsorted supplied bands. Every output is identical with allocation fills
`0x00`, `0xa5` and `0xff`.

The [Rust capture module](../../conformance/native_table/splits.rs) checks Model,
Drawing and Base hashes recorded above. Frame lookup, list construction/copying,
cache comparison, rectangle arithmetic and deletion paths execute native code.
Host allocation/deletion interfaces, text initialization, single-thread mutex
operations and diagnostic interfaces are isolated. In particular, the native
list's empty first-element lookup takes its diagnostic path and returns null;
the capture does not validate its logging or thread-local error storage.
The results establish split-cache selection, not text shaping, first-line
relocation decisions, row compression, complete pagination or device appearance.

The [Rust regression](../../crates/sdocx/src/render/table/pagination/native_split_tests.rs)
matches every changed flag, cache-presence state and rectangle coordinate bit.
Cell split caches are typed `Option<BandList>` values. Warm splitting uses the
first owner's top, copies only to frame-owning cells and preserves stale lists
when the native comparison succeeds. Export preparation still accepts only
unmerged grids.

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --row-splits scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenBase.so > /tmp/table-row-splits.json
cmp /tmp/table-row-splits.json conformance/table-row-splits.json
```

### Warm-row movement and relayout decisions

Drawing `layoutRow`, `0xb0420`, first updates the requested row's split caches.
When that update changes a cache, it runs `updatePositionFromRowForFirstLineCut`
and lays out only cells that own their frames. It returns true even when the
requested row contains no self-owning cells. Distinct empty caches compare
unequal, so repeated empty-list updates retain this relayout branch.

When the split cache is unchanged, row zero returns false. Later rows use the
current and previous first-column frame owners. With a pending gap greater than
`0.001f`, an empty previous split list prevents movement. Otherwise the gap is
removed only when the current first-line minimum is at most
`pending_gap - previous_first_band_height`. Gap removal returns true without
calling `layoutCell` or updating the cached split list.

Without that pending-gap branch, movement requires
`first_line_minimum - current_first_band.top > f32::EPSILON`. The displacement
is the first band's bottom. Physical rows move through `offsetFromRow`, the
current pending gap receives that displacement, and split caches are updated
before self-owning cells are laid out. Equality at either comparison boundary
does not select the greater-than branch.

`updatePositionFromRowForFirstLineCut`, `0xb2360`, returns immediately for row
zero or a missing/empty first-owner split cache. A pending gap above `0.001f`
is removed, split caches are updated, and the routine retries against the new
cache. Otherwise it uses the same first-line/first-band comparison and
displacement as above. Earlier-row owners supply bands independently of the
physical rows that move.

Drawing `layoutFromRow`, `0xaa448`, calls `layoutRow`,
`updatePositionFromRow` and `updatePositionForRowBottom` in that order for every
remaining row (`0xaa480`–`0xaa4ac`). It does not use `layoutRow`'s return value
to suppress sizing or compression.

[The warm-control capture](../../conformance/table-warm-control.json), SHA-256
`b78403ccecffe594cecf18eea48ba60c4c56808905049230589a9c9ab42c30ab`,
records 148 inputs and 1,207 initial slots. Its 1,089 actions contain 354 direct
first-line calls, 415 row-layout calls and 320 ordered layout/sizing/compression
sequences, yielding 10,892 slot snapshots and 418 `layoutCell` calls. Inputs
cover merged owners, rows without self owners, empty/missing caches, both
comparison thresholds, gap removal, retries and supplied minima. Every output
is identical with allocation fills `0x00`, `0xa5` and `0xff`.

The [native Rust harness](../../conformance/native_table/control.rs) checks the
same five library hashes as the compression capture. Native row control,
owner lookup, list operations, metric getters, sizing and compression execute
unchanged. The `layoutCell` boundary records cell selection without shaping;
the supplied two-line measurements remain fixed. Initialization, host memory
interfaces, single-thread mutex operations and diagnostics are isolated. The
ordered sequences call the three native routines directly; final observer
notifications from `layoutFromRow` are outside the capture.

The [Rust regression](../../crates/sdocx/src/render/table/pagination/native_control_tests.rs)
uses the production scheduler with a measurement callback and matches every
changed flag, ordered cell selection, frame coordinate, pending gap and split
cache bit. This establishes control flow with fixed caches, not native shaping,
merged-frame construction, nested content or complete device pagination.

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --warm-control scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenWidget.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenText.so > /tmp/table-warm-control.json
cmp /tmp/table-warm-control.json conformance/table-warm-control.json
```

### Row-bottom compression from owner caches

Drawing `getLastIntersectSplitRect`, `0xb304c`, reads the first column's frame
owner and that owner's cached split list. It traverses the list in stored order,
retaining the last rectangle whose top is at most the physical row height. It
stops at the first rectangle beyond that height; unsorted lists are not searched
for later matches. The owner supplies local bands, while raw row frames supply
the height. A covered row therefore does not reinterpret the bands relative to
its own physical top. Missing or empty lists return an empty rectangle.

`getRowOffsetForLastEmptyInPage`, `0xb294c`, requires a nonempty selected
rectangle, then resolves frame owners across the requested row. For each owner
with a cached `TextLayout`, it reads `GetLineCount` and `GetLineBottom` for the
last line; missing text layouts are skipped. The maximum starts at zero, so
negative line bottoms cannot lower it. Cached measured height and cell bottom
margin do not participate in this comparison.

If that maximum is strictly below the selected band's top, the returned offset
is `(band.top - physical_row_height) - half_border_width`, with native `f32`
rounding after each subtraction. Equality does not compress. Drawing
`updatePositionForRowBottom`, `0xb28ec`, applies only negative offsets: it
extends the current raw row and moves subsequent rows through `offsetFromRow`.
That movement retains the existing pending-gap consumption rules. Split caches
are unchanged by this operation, even when row movement makes them stale.

[The compression capture](../../conformance/table-row-bottom.json), SHA-256
`c2ea6ef80738b101c32d54e910275c3a7308916da286684f89ad4af1af9e3d50`,
records 148 inputs, 1,216 initial slots, 425 updates and 4,368 updated slot
snapshots. Inputs include first/other-column merges, covered-span chains,
missing caches, missing text layouts, empty/unsorted bands, later pending gaps,
and values immediately around line-bottom and physical-height boundaries.
All outputs are identical with allocation fills `0x00`, `0xa5` and `0xff`.

The [native Rust harness](../../conformance/native_table/bottom.rs) checks the
Model, Drawing, Base, Widget and Text hashes recorded above. Compression,
owner lookup, list traversal, native text getters and frame updates execute
unchanged. Text initialization, host allocation/deletion, single-thread mutex
operations and diagnostic interfaces are isolated. Present layouts receive two
supplied line records: first-line height 10.25, top margin 12.5, measured height
300, and the case's independent last-line bottom. The capture establishes
control flow with these cached measurements, not native text shaping or full
merged preparation.

The [Rust regression](../../crates/sdocx/src/render/table/pagination/native_bottom_tests.rs)
matches selected-band, displacement, frame-coordinate and pending-gap bits.
Production compression reads owner caches through the bounded `TableGrid`.
Prepared exports still accept only unmerged grids; full merged shaping, nested
objects, complete pagination and device appearance remain outside this evidence.

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --row-bottom scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenBase.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenWidget.so \
  scratch/apk-analysis-native/arm64-v8a/libSPenText.so > /tmp/table-row-bottom.json
cmp /tmp/table-row-bottom.json conformance/table-row-bottom.json
```

### Stored cells and frame owners

Model `ObjectTableImpl::GetCell`, `0x3c7570`, indexes the stored row and column
vectors directly (`0x3c75a8`–`0x3c75b0`). The public
`ObjectTable::GetCell`, `0x3d2be0`, validates the requested indices and reads
the same stored slot (`0x3d2c30`–`0x3d2c60`); it does not substitute a frame owner.
`GetFrameCell`, `0x3c75c0`, is a different producer: it searches earlier
positions backwards, tests whether their row/column spans cover the requested
position (`0x3c7630`–`0x3c7698`), and follows a covering cell's own origin
(`0x3c76c0`–`0x3c76cc`). Without a covering earlier cell it returns the
stored cell at the requested position (`0x3c7754`–`0x3c7764`). This is not
equivalent to expanding every serialized cell into an independent visible box.

Drawing `layoutRow`, `0xb0420`, compares `GetCell` with `GetFrameCell` and
lays out text only when they return the same cell
(`0xb0478`–`0xb04c8`). `updateMeasuredRect`, `0xab168`, obtains the first
and last grid positions through `GetFrameCell` before reading cached frames
(`0xab198`–`0xab238`). Merged layout therefore needs distinct stored slots,
frame owners, paint-visible cells and cached frame geometry. Sparse records,
final merged-frame construction and rowspan growth are not established by these traces.

### Native frame ownership and paint visibility

[The ownership capture](../../conformance/table-ownership.json) records native
`GetFrameCell` and whole-grid `GetVisibleCells` outputs for 279 synthetic dense
grids: seven named cases, all sixteen positive in-bounds span combinations for
a 2×2 grid, and 256 deterministic mixed-span cases up to 5×5. Stored origins
match their physical row and column positions. The Model library SHA-256 is
`4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a`.

`GetFrameCell`, `0x3c75c0`, searches earlier candidates in reverse row/column
order. It includes spans on cells already covered by another cell, then follows
the selected candidate's stored origin until no earlier span covers it.
An origin outside the originally covering rectangle can consequently resolve
to that rectangle's owner through a covered cell's retained span.

Whole-grid `GetVisibleCells`, `0x3c7784`, calls the range implementation at
`0x3c77b0`. Its forward row-major scan tests a visited bit at
`0x3c7b48`–`0x3c7b60`, appends only an unvisited stored cell at
`0x3c7bb4`–`0x3c7c5c`, then marks that cell's span at
`0x3c7c60`–`0x3c7cc0`. Covered cells do not contribute their retained spans to
this mask. Drawing's ordinary and text-canvas paths request this list at
`0xa6174` and `0xa6894`, respectively.

For a one-row, four-column grid with spans `(1,2)` at column 0 and `(1,3)`
at covered column 1, native frame owners are `[0,0,0,0]`, but paint-visible
cells are `[0,2,3]`. The equivalent four-row chain produces the same distinction.
Thus selecting only cells whose frame owner is themselves does not reproduce
the native paint list. These synthetic cases establish the two local APIs;
they do not establish which overlapping retained spans Samsung's editor emits.

The [Rust capture harness](../../conformance/native_table.rs) executes the
unmodified native routines and their vector helpers in ARM64 Unicorn emulation.
Intra-library PLT entries resolve to their native implementations; only
allocation, deletion and memory fill use host interfaces. Fresh allocations
filled with `0x00`, `0xa5` and `0xff` give identical outputs across all cases:
7,197 frame-owner calls and 837 visible-list calls. The inputs do not exercise
empty/sparse rows, invalid origins, zero/overflowing spans, clipping subranges,
native table layout, or a device-rendered reference.

```sh
rustc --edition 2024 -D warnings -C panic=abort conformance/native_table.rs \
  -L native=scratch/apk-analysis-runtime/python/unicorn/lib \
  -C link-arg=-Wl,-rpath,"$PWD/scratch/apk-analysis-runtime/python/unicorn/lib" \
  -o /tmp/sdocx-native-table
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  > /tmp/table-ownership.json
cmp /tmp/table-ownership.json conformance/table-ownership.json
```

### Rust saved-frame painting

The [Rust grid model](../../crates/sdocx/src/render/table/grid.rs) preserves stored
slots, resolves frame owners and selects paint-visible cells separately. Its
regression matches all 2,399 owner positions and 279 visible lists in the pinned
native capture. Input validation requires a dense grid, matching stored origins,
positive in-bounds spans, at most 65,536 slots and at most 1,048,576 accumulated
span positions. These bounds are Rust work limits, not recovered Samsung limits.

Merged tables retain saved cell frames and paint only native-visible cells.
Sparse records and invalid spans retain the saved-cell fallback. Both report
`UnsupportedContent`: merged frame sizing, rowspan growth and pagination are
still unimplemented. [Vector-output regressions](../../crates/sdocx/tests/embedded_text_layout.rs)
cover horizontal, vertical, rectangular and covered-span-chain cases in both
themes and all three layout constraints. They check identical SVG preview/replay,
selectable PDF source, matching baselines and zero PDF image resources. This is
visibility and transport coverage; it does not establish native merged geometry.

### Column minimum cache selection

Drawing `TableLayout::GetMinColumnLayoutWidth`, `0xab7a0`, starts with Model's
indexed `ObjectTable::GetMinColumnWidth`, `0x3da870`. That getter reads the
per-column minimum vector at implementation offset 56, independently of the
global minimum at offset 12. An absent table returns zero.

For each stored row, Drawing uses `GetCell` directly and looks up that cell's
cached layout (`0xab80c`–`0xab848`). It does not resolve frame owners or skip
covered cells. A missing layout causes an immediate return of the saved
per-column minimum, discarding values from preceding rows. An existing layout
with no optional width contributes nothing. When every layout exists and at
least one optional width is present, the result is the larger of the saved
minimum and the **smallest** present cached width (`0xab850`–`0xab898`). For
example, cached widths 100 and 25 with a saved minimum of 10 return 25; replacing
either layout with a missing entry returns 10. With no rows or no present
optional widths, the getter returns the saved minimum.

The optional payload is a float at cell-layout offset 620, followed by its
presence byte at 624. `ObjectTableCellLayout::GetMinLayoutWidth`, `0x8c078`,
returns those fields. The traced producer `getCellLayoutWidth`, `0x8c13c`,
takes the maximum of each line's width plus its paragraph rectangle width and
indent, then adds the text layout's left and right margins (`0x8c19c`–`0x8c238`). It
sets the optional width when that result is at least the configured layout
width; otherwise it clears the presence byte and returns the configured
width (`0x8c23c`–`0x8c260`). This producer trace is separate from the capture
of supplied cache values below.

[The column-minimum capture](../../conformance/table-column-minima.json),
SHA-256 `571f54fbc638314d34080bedee692d7010d42574844d21af03869cf56aacc68f`,
contains 179 synthetic cache inputs and 182 column queries. It covers all 81
four-row combinations of missing layouts, unset optional widths and present
widths, both with unit spans and with a four-row owner span. Other cases check
column independence, reversed value order, fractional float bits, unset payloads
containing stale values or NaNs, global-minimum independence and an absent
table. Allocation fills `0x00`, `0xa5` and `0xff` produce identical captures
across 546 native getter calls.

The [Rust capture module](../../conformance/native_table/columns.rs) constructs
the cache with native hash-map insertion and verifies entries with native
lookup. Both ELF libraries are hash checked and all load segments are mapped;
PLT calls resolve to native implementations except allocation/deletion. Cache
widths are supplied inputs: this capture does not execute native text shaping,
column resizing, cold merged-frame construction or Samsung device rendering.
Rust export preparation retains saved column widths and does not implement
this column-minimum cache getter.

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --column-minima scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so \
  > /tmp/table-column-minima.json
cmp /tmp/table-column-minima.json conformance/table-column-minima.json
```

### Outer-border initialization

`TableLayout::getHalfBorderWidth`, `0xb325c`, iterates
`ObjectTable::GetBorderPath` (`0xb3288`) and selects positive widths with
nonzero ARGB values (`0xb32ac`–`0xb32c4`) before halving the maximum.
Model's `GetBorderPath`, `0x3cb464`, reads the outer border pointer at member
152 and constructs its four paths. This initializer does not inspect every
cell border. The later per-edge drawn-bound widths include boundary cells and
do not gate on color; those two measurements must remain distinct.

### Native border paths

[The border-path capture](../../conformance/table-border-paths.json), SHA-256
`4cae7223084cff60765bddb5e0b9926e8a706b741bf7cb9949cebfe4e2f8dc0d`,
records native `GetCellBorderPath` (`0x3cad9c`) and `GetBorderPath`
(`0x3cb464`) outputs for twelve synthetic grids. The 69 requested cell paths
contain 364 segments; the twelve outer queries contain eight segments. Captures
under allocation fills `0x00`, `0xa5` and `0xff` agree byte for byte. These are
Model outputs; they do not capture Drawing, pagination or device appearance.

`getCellBorder`, `0x3c7510`, reads the stored cell's border pointer at member 96
and falls back to the table's default-cell pointer at member 160
(`0x3c7554`–`0x3c755c`). A present cell border replaces the whole default;
a zero-width edge does not select that edge from the default. Null pointers in
the synthetic capture are distinct from omitted serialized fields.

`GetCellBorderPath` first resolves `GetFrameCell` at `0x3caddc`. It constructs
the owner's left, top, right and bottom perimeter from the corresponding stored
boundary cells. A merged rectangle can therefore have different styles along
one side, including styles on covered cells. It does not expand just the visible
origin's border around the merged rectangle. Path construction retains zero
colors and nonpositive widths; later Drawing filters are separate.

The outer path is independent of those cell styles. Its ordered endpoints run
bottom-to-top on the left, left-to-right on top, top-to-bottom on the right and
right-to-left on the bottom. Cell-path segments run top-to-bottom on both
vertical sides and left-to-right on both horizontal sides. Start/end radius
values must follow the captured endpoint direction.

Model's `TableBorder` constructor produces black (`0xff000000`), one-unit-wide
edges with zero radii. The capture executes that constructor for the default
case. `ObjectTableImpl` constructs both the outer and default-cell borders at
`0x3c5d44` and `0x3c5d68`. Missing serialized styles consequently do not imply
native null pointers.

Segment coordinates use `f32` prefix sums, followed by subtraction of the
current row height or column width to recover its start. The large-origin,
fractional-size case preserves the resulting cancellation and rounding. The
captured line equations also preserve native fused multiply-add results;
neither follows a sum performed once in `f64` and narrowed at the end.

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --border-paths > /tmp/table-border-paths.json
cmp /tmp/table-border-paths.json conformance/table-border-paths.json
```

The Rust border model matches every captured segment, style and endpoint,
including the native `f32` staging. Its topology is bounded to 65,536 cells;
paint work is independently bounded to 1,048,576 perimeter segments after
frame-owner propagation. Invalid numeric inputs report `InvalidBounds`;
unsupported topology or excessive paint work report `UnsupportedContent`.

### Border painting

Drawing's `drawTableCellWithoutText`, `0xa748c`, scans paint-visible cells and
calls Model's perimeter builder at `0xa7c34`. It rejects ARGB zero and exact
width zero (`0xa7c6c`–`0xa7c7c`), then raises widths below one to one. Negative
widths survive Model and reach this minimum-width branch. A nonzero ARGB with
zero alpha still participates in selection, although its paint is transparent.

Shared boundaries generally select the later cell's left/top edge. With an
active outer border, Drawing suppresses the first-column left edge, first-row
top edge and all right edges (`0xa7c98`–`0xa7cd0`). A bottom edge is retained
when the next row's pending gap exceeds `0.001f`. Without an active outer
border, the right edge is retained only for a cell originating in the last
column, and bottom edges are retained on the last row or before a pending gap
(`0xa7cdc`–`0xa7d0c`). This uses the layout's pending-gap cache at member 856,
not a difference inferred from rounded cell frames.

`getTableBorderStyle`, `0xa6fb4`, aggregates the outer paths: the last nonzero
ARGB with positive width supplies the color, and the maximum drawable width
supplies thickness after a one-unit minimum. Horizontal-edge radius maxima
supply `rx`; vertical-edge maxima supply `ry` (`0xa70e8`–`0xa711c`). Radii
are read even from inactive edges. When both radii are positive,
`drawTableBorder`, `0xa8218`, paints one rounded rectangle with this aggregate
style. Otherwise it retains each outer edge's style and endpoint direction.
The rounded cell backgrounds are painted separately; non-perimeter corners
are squared by additional rectangles (`0xa7b10`–`0xa7c0c`).

[The Drawing outline capture](../../conformance/table-border-drawing.json),
SHA-256 `63b57494de2caa428cd110021ab5ec36d5a93316ecf373bdd87bb1cbcb288c87`,
executes native `getTableBorderStyle` against Model's native outer paths.
It covers 25 style inputs at seven positive canvas X scales (`0.125`, `0.3`,
`0.5`, `0.75`, `1`, `2`, `4`): 175 outputs, each repeated under allocation
fills `0x00`, `0xa5` and `0xff`. Both libraries are hash-checked. The host
supplies the canvas matrix getter, allocation, deletion and memory fill;
style aggregation and Model constructors/path helpers execute natively.

The outputs confirm all sixteen active-edge masks, absent outer pointers,
constructor defaults, nonpositive widths, nonzero colors with zero alpha,
signed radius maxima and radii from inactive edges. All-negative radii remain
negative; the aggregation does not clamp them to zero. After selecting the
drawable width, native Drawing raises it to `1.0f / canvas_scale` when
`width * canvas_scale < 1.0f`. The Rust border model matches all four output
fields, including exact `f32` width bits, for every captured scale. Document
vectors use the unit-scale result.

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --drawing-borders scratch/apk-analysis-native/arm64-v8a/libSPenDrawing.so \
  > /tmp/table-border-drawing.json
cmp /tmp/table-border-drawing.json conformance/table-border-drawing.json
```

Spannable paths use prepared content and cell frames; saved paths retain Model
coordinates with drawing offsets. Outer content bounds are distinct from the
border-expanded measured rectangle (`TableLayout::updateMeasuredRect`,
`0xab168`; `GetContentRect`, `0xab4a8`). Cell backgrounds and borders are
painted in cell order, then the outer border; text uses a separate pass.

The Rust vector painter applies these rules with typed lines and rectangles,
stored edge colors/alpha, shared theme conversion and metadata-derived radii.
Its width policy is the native unit-canvas policy. Native `drawLinePath`,
`0xa8008`, also enforces a one-pixel minimum using the canvas X scale;
zoom-dependent screen coverage is not reproduced by the document-space vectors.
The outline aggregation has captured Drawing execution; edge selection,
background corner squaring and paint-pass ordering have assembly evidence.
The capture does not execute the canvas's line/rectangle painting or clipping.
Public SVG/replay and PDF regressions check edge precedence, widths, opacity,
axis radii, selectable source and zero image resources. Native device appearance,
merged prepared sizing and complete border clipping across split pages remain
outside this evidence.

### Cell background selection

[The background capture](../../conformance/table-backgrounds.json), SHA-256
`46fe91b4a788c5276534b641b4a7224604a2410154724cd8ce56296809b78f44`,
records `TableCell::GetBackgroundColor(bool)` (`0x3c2384`) and
`ObjectTableImpl::GetCellBackgroundColor(int,int,bool)` (`0x3cbf7c`). The
hash-checked Rust harness executes both getters, native row forwarding and
the bound callback operators. Their outputs agree for 40 synthetic 3×3 grids:
720 selected colors across both values of the heading-override argument.
The inputs cover all heading flag combinations, owned/unowned cells, zero and
partial-alpha colors, and unit/merged spans.

`ObjectTableImpl::getCellBgColor`, `0x3c8cfc`, selects heading color when the
stored row index is zero with heading-row enabled, or the stored column index
is zero with heading-column enabled. Otherwise it selects the default cell
color. The constructor initializes both colors to ARGB zero at `0x3c5d2c`.
`TableCell::GetBackgroundColor` returns this inherited value when the cell
has no owned background, or when heading override is enabled for a heading
cell; otherwise it returns the owned color. The table getter selects the raw
stored cell, independently of merged-frame ownership.

Drawing requests heading override at `0xa79f8`, then calls the context's color
conversion with theme mode 3 at `0xa7ad0`. `Context::GetColor`, `0x6c39c` in
`libSPenView.so` (SHA-256
`c4a17e4232c2074d3833604974d75ac961fab4d9651949bb2552704c86621dc8`),
selects the active default color theme for that mode. The light and dark
theme primitives preserve the input alpha. Drawing passes the resulting ARGB
to `SPPaint::SetColor` at `0xa7ae0`; no opaque beige substitution appears in
this path.

The Rust fill model matches every captured selected color. SVG/replay and PDF
preserve the alpha of inherited and owned fills, including zero. Text contrast
uses the fill composited over the resolved paper, in both measurement and final
drawing. This retains the renderer's documented contrast policy; the captures
do not establish device appearance or application-level theme selection.

```sh
/tmp/sdocx-native-table scratch/apk-analysis-native/arm64-v8a/libSPenModel.so \
  --backgrounds > /tmp/table-backgrounds.json
cmp /tmp/table-backgrounds.json conformance/table-backgrounds.json
```

Rust regressions exercise saved row maxima below measured content height through
callback preparation, final drawing, light/dark SVG preview, replay and retained
PDF. They require unchanged source ranges, baselines and vector text, with no
PDF image resources. These establish the metadata-invariance and transport
contracts; the current locked Samsung corpus has no dedicated row-maximum or
merged/sparse table reference capture.

## Shared code-block text layout

Drawing `CodeBlockLayout::initConstants`, `0x73084`, resolves these IDs through
`Constant::GetPixels`. The Content library stores the values in 24-byte records
at `0x77e8 + 24 * id`. Each record below has unit kind 3, document density,
and rounding mode 3, no rounding. These values come from the native table,
not measurements of the captured PDF.

| Constant ID | Value before density scaling | Layout use |
| ---: | ---: | --- |
| 332 | 16 | Left padding |
| 333 | 12 | Top padding |
| 334 | 16 | Right padding |
| 335 | 12 | Bottom padding |
| 338 | 12 | Horizontal gap between title and copy button |
| 339 | 8 | Vertical gap around body |
| 342 | 24 | Copy-button width and height |
| 343 | 12 | Rounded radius |

Drawing `CodeBlockLayout::Measure`, `0x732fc`, positions the copy button at
`width - rightPadding - copySize`, `topPadding`. The title frame extends from
left padding to `copy.left - horizontalGap`, using the copy button's top and
bottom. The title measurement return is discarded at `0x73410`; the original
frame is retained at `0x73428`.

The body starts at `copy.bottom + verticalGap`, with width
`width - leftPadding - rightPadding`. Both objects use ordinary
`ObjectTextDrawing` through `measuredObject`, `0x73694`, which returns
`bottom = input.top + TextLayout::GetHeight(false)` at `0x73764`–`0x73770`.
The final object bottom is `original.top + measuredBody.bottom + verticalGap
+ bottomPadding` at `0x73584`–`0x735a0`: the vertical gap is reserved both
above and below the body. Split-page padding rectangles can shift these frames;
their full behavior remains unverified.

The adapter supplies frames and padding, not fixed text baselines or line
advances. Paragraph styling, wrapping, margins and text metrics belong in the
same text engine used for other rich-text objects.

## Border records

Borders use the same untyped offset/mask header as rows and cells. The current
writer emits offset zero, a one-byte zero property mask and a two-byte zero
field mask, followed by four edge records. The resulting size is
`9 + 4 * 16 = 73` bytes, excluding the parent's four-byte size prefix.

Each edge stores `u32` ARGB color, `f32` width, `f32` start radius and `f32` end
radius. The wire order is **left, top, right, bottom**: `GetBorderStyleLeft`
at `0x3dc830` returns the first in-memory style, and the top/right/bottom
getters at `0x3dc834`, `0x3dc83c` and `0x3dc844` add 20, 40 and 60 bytes.
The serializer copies only the first 16 bytes of each 20-byte in-memory style;
the two runtime flags at offsets 16 and 17 are not serialized.

The JNI conversion writes color at offset 0 and the three floats at offsets
4, 8 and 12. Its field-name strings at `0x14c26b`, `0x1512ec`, `0x137f52`
and `0x156998` are `color`, `width`, `startRadius` and `endRadius`, confirming
the public names independently of decompiled class field order. The exact
corner geometry used by the native renderer remains to be investigated.

## Validation and limits

The note parser's synthetic unit tests cover variable masks, valid zero offsets,
cell identity/geometry, all offsets that truncate fixed cell or row data, offsets
beyond the record, wider field masks and nested cells. Additional tests cover
all 14 table fields individually and together, every truncated field prefix,
all combinations of the three table flags, known and unknown auto-fit modes,
independent edge colors and radii, sized-border boundaries, bounded column
allocations, and the nonnumerical row-height field order. Unknown bytes stay
available for inspection. The existing embedded table/code-block tests continue
to pass. These tests establish bounds and structure, not Samsung rendering
fidelity.

The shared Rust engine measures and paints supported embedded table/code text.
Table border painting follows the captured Model geometry and traced Drawing
rules within the evidence limits above; standalone table decoding is not
established. Inherited shape data and native layout behavior are separate from
the bounded embedded-record decoding described here. Current layout support
and transport limits are documented in
[vector text support](../text-vector-support.md).
