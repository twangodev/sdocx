# Native plot records

## Evidence

Analyzed Samsung Notes 4.4.45.37, APK SHA-256
`daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667`.
Addresses in the table below are ARM64 virtual addresses in `libSPenModel.so`.
Native writers, readers, getters and JNI field mappings establish this layout
without new Samsung-generated documents.

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

## Saved source and generated appearance

The mapped current writer stores expressions, ordered substitution strings,
styles, angle mode and a mathematical viewport. It has no known saved
point-array, path-command or numeric-axis-label-string field. The viewport is
separate from base-frame placement; these findings do not decode future tails
or establish arbitrary chart-data formats.

In `libSPenRecogUIFeature.so`, `MathManager::Plot` (`0x1a828c`) selects a graph
and reads angle mode. At `0x1a8398`–`0x1a849c`, nonempty substitution strings
become solver input; otherwise the primary LaTeX becomes the single input.
It passes that vector and requested float range to `MathSolver::Plot` at
`0x1a8514`. The solver (`0x1d5248`) calculates preceding strings into one
context at `0x1d534c`, then plots the last string at `0x1d53d4`. Preserving
substitution order therefore preserves meaningful solver input.

`libSPenObjectControl.so`'s `setGraphCoordinateData` (`0x12182c`) caches
derived point lists through `setMultiplePoint`. Its listener call at
`0x121e90` supplies object, graph index, float range and an output point
collection. The returned x/y float pairs become runtime `PointF` lists;
`drawGraphs` calls a pen-event drawing route at `0x120cec`. Composer's
editor initialization binds its `ContentsView::onSolverPlot`
(`0x413c18`, `0x416b40`) through the `NoteObjectView` forwarding callback
(`0x3d757c`). That solver method calls `MathManager::Plot` at `0x41691c`
and copies its point collections. Numeric axis labels are generated by
`setGraduation`'s calls to `GetNumberString(float,float)` at `0x11da7c`
and `0x11de70`.

### Derived segment boundaries, precision and lifetime

The solver callback returns nested `vector<float>` lists; `setMultiplePoint`
turns each into a separate `PointF` vector (`0x121ec8–0x1220c0`). XY pairs
have eight-byte stride, and its temporary copy rounds down to complete pairs
(`0x121ee0–0x121ef4`), discarding an unpaired final float. No scalar sentinel,
jump-distance test or finite rejection splits a list in this converter.
Those list boundaries establish the representation of breaks, rather than
the numerical discontinuity criteria used by the solver.

For ordered finite mathematical bounds, coordinates clamp to a rectangle
padded by `min(width, height) * f32_bits(0x3c23d70a)` (approximately 0.01;
`0x121e34–0x121ec4`). With finite padded bounds, infinities clamp, while
unordered NaNs remain in the point list (`0x121f24–0x121f64`); this does not
establish actual solver output or downstream pen admission.
`setMultipleViewPoint` maps coordinates in
`f32` (`0x121b70–0x121bbc`). Outside padded view X bounds it emits edge X
and rectangle-center Y, skipping the source Y mapping; otherwise it clamps
mapped Y (`0x121b90–0x121be4`). It creates no new segment boundary.
`setMultipleEvent` skips lists shorter than two points and creates separate
MotionEvents for the others (`0x120e00–0x120e88`), widening the `f32` pairs
to `f64` (`0x120e58`, `0x120ecc`). This widening restores no earlier precision.
The animation count can limit a segment's prefix; `drawPenGraph` submits each
event separately (`0x121110–0x121120`), without concatenating adjacent lists.

The view's Graph owns mathematical points at +88, view points at +104 and
events at +112. A cached rectangle containing the requested mathematical
range bypasses sampling (`0x121860–0x121864`); a miss replaces its points.
`Load` conditionally clears graphs and empties cached ranges in its existing
graph update branch (`0x11b30c–0x11b32c`, `0x11b9fc–0x11ba00`). Rectangle
updates select sampling with flags intersecting 0x6 and view/event rebuilding
with flags intersecting 0x7 (`0x11ce54–0x11ce78`). View rebuilding releases
the previous derived points/events (`0x11e23c–0x11e274`), and Graph destruction
releases its collections (`0x1229c8–0x1229e4`). Regeneration is conditional.
The current Model writer (`0x453210–0x45360c`) does not serialize these caches;
expressions, ordered inputs, viewport/style and derived geometry retain
separate authority. This inspection adds no numerical or appearance parity.

`ObjectPlotView::SetCapturedBitmap` renders the view at `0x11c000` and installs
the bitmap through `ObjectBase::SetCapturedThumbnailBitmap` at `0x11c00c`.
The reference is [common flexible field 17](object-flexible-findings.md), not
a plot-own field or the unresolved legacy value. In `libSPenDrawing.so`,
`ObjectPlotDrawing::DrawObject` (`0x89f60`) draws this captured bitmap into the
object rectangle (`0x8a038`–`0x8a070`). Without one, this function logs and
returns; it contains no solver/curve fallback. This establishes that generic
drawing function, not all Samsung preview or export routes.

The Rust inspection API retains original plot source without evaluation.
`PlotMetadata.base.flexible_metadata()` can expose the captured-thumbnail media
ID, but does not own the image bytes. High-level page decoding provides neither
plot geometry nor that captured appearance. Preserving an original image asset
preserves its existing appearance; replacing the expressions and ordered inputs
with that image would discard recoverable source. Generated paths would be a
separate reconstruction artifact, not a decoded saved vector channel.

## Validation and evidence limits

Five synthetic integration tests cover all six fields, multiple graph styles
and substitutions, all truncated field prefixes with a following decoy frame,
unknown masks/modes/visibility, zero offsets, cumulative counts, UTF-8 decoding
and UTF-16-unit limits, malformed types and invalid payload bounds. Existing
math-envelope tests exercise the shared size/count helpers.

The ordered solver-input contract and this editor's point/label producers are
established by static inspection. Equation grammar, numerical sampling and
discontinuities, complete layout, export-route selection and real-document
capture-resource persistence remain unverified. Older variants and unknown
future fields are not established. Synthetic parser cases do not validate
plotted appearance or writer variants beyond this APK.
