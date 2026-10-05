# Erasing and preserved stroke geometry

## Evidence and scope

Samsung Notes 4.4.45.37 distinguishes the handwriting remover's cut and
whole-object operations from a stroke's saved eraser property. The inspected
ordinary cut route replaces an original stroke with ordinary stroke fragments;
it does not require the reader to recover an editor gesture and subtract its
path at export time.

The dispatch, object mutation and handwriting save-boundary traces are static
ARM64 and decompiled Java findings. Separate bounded native executions confirm collision
predicates, coordinate preparation, retained-part construction, generated XY,
fragment channel arrays and timestamp arithmetic. Those captures stop before
object copying or the final `SetPoint` body. Additional executions inspect
legacy-rectangle loader prefixes, complete modern common-frame writing and
saved-stroke redraw adapters. Neither full editor interaction nor save/reload
after a device erasing gesture was executed. Brush erasing and end-to-end
saved eraser-property rendering remain unresolved contracts.

The APK is the artifact identified in the
[knowledge base](README.md#sources-and-validation). Library identities are:

| Library | SHA-256 |
| --- | --- |
| `libSPenComposer.so` | `52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f` |
| `libSPenModel.so` | `4fbcf6d4213e929f1535d32abb487743643fd5d0dfc366e50dfeb2e7d8015b7a` |
| `libSPenBase.so` | `e10da0116946691cf68302437ef261282e1dfe0eec15bf2dfa66093286985deb` |
| `libSPenWDoc.so` | `1fc540573cc07f3e52466fd048568c8522119c6952cf22213b135ead00af57f6` |
| `libSPenDrawing.so` | `788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd` |
| `libSPenPenCommon.so` | `afd39c0d55ec5cf47153be48222c8a9ddc0057fc772dd870af9ced74a59ec33d` |
| `libSPenMarker2.so` | `f1b2ceea921baac78b722cbd5423732f42d7bec19ccb63d428cf429582412269` |
| `libSPenEraser.so` | `b95ab0451ec8f7cb22ed4a83b1b6e678e441c8486098cb03702ca5a7031eeffb` |

Addresses belong to Composer unless a different library is named. Unexported
Composer methods are identified through signature strings, constructor RTTI,
vtable relocations and connected call sites, rather than objdump's nearest
exported-symbol labels.

## Remover settings select two different mutations

Decompiled `com/samsung/android/sdk/pen/SpenSettingRemoverInfo.java` declares:

| Setting | Value |
| --- | ---: |
| `CUTTER_TYPE_CUT` | 0 |
| `CUTTER_TYPE_REMOVE` | 1 |
| `CUTTER_TARGET_ALL` | 0 |
| `CUTTER_TARGET_STROKE` | 1 |
| `CUTTER_TARGET_HIGHLIGHTER` | 2 |
| `CUTTER_TARGET_TAPE` | 4 |

Its ordinary constructor sets type 1. These are remover-setting values, not
stored object types or stroke-property bits. The separate
`SpenSettingEraserInfo` has pen/text types 0/1 and does not establish the same
cut/remove mapping.

`WritingViewRemoverAction` constructs `WritingCutterRemover` at member 392
through `0x51d72c` and `WritingEraserRemover` at member 400 through `0x525f68`.
Their primary vtables are `0x584c88` and `0x585068`, respectively. Setting
helper `0x51ca28` reads the supplied type and selects `action[392 + type * 8]`
at `0x51ca54`, storing the active pointer at member 384. It also retains size,
target and remove-shape-enabled fields at members 480, 484 and 488.

The loop at `0x51ca84` sends target through slot 96 and remove-shape-enabled
through slot 120 to both components. The type selection is an unchecked native
index in this helper; this does not make arbitrary setting values valid.
Size/zoom/stretched-scale handling is independently described in
[zoom scale findings](zoom-scale-findings.md#writing-view-scale-dispatch-reaches-the-removers).

The cut-task generator at `0x51f628`, identified by the signature string at
`0x1d1a7b`, iterates supplied objects and rejects invisible objects. Its initial
type mask `0x200182` permits types 1, 7, 8 and 21: Stroke, Shape, Line and Math.
Shapes take a separate collision/removal route at `0x51fbb0`–`0x51fc30`.
The type mask alone therefore does not establish that shape outlines are split
into stroke fragments.

Stroke admission is not a generic pen-name-independent geometry test. The
ordinary branch calls `IsTopLayerPen` at `0x51fc44`; another helper at
`0x51fdd4` recognizes Marker3, Marker4, StraightHighlighter and StraightMarker.
Either controller flag at members 168/169 also permits admission. Model
`IsTopLayerPen`, `0x2e1ea0`, reads stroke-data byte 341, serialized as property
bit 6; see [capture composition](capture-composition-findings.md#per-object-render-layer-selection).
A nonnull pen name is required, but these gates are not a closed pen whitelist
or evidence that every admitted profile is rendered by the SDK. Math also
passes its type-1 Formula stroke/answer-stroke children to producer `0x520078`.

## Collision uses copied coordinates and pen widths

Preparation helper `0x51ffa4` requires nonnull source PointF and pressure arrays.
It copies PointF samples into controller vector 224 through `0x521fe0`, adds
integer page offsets 156/160 as `f32`, then scales each axis around the cutter
center using `f32` FMA when scale 172/176 differs from 1. Pressure is a presence
gate, not a transformed channel. Generated-endpoint helper `0x521828` divides
around that center and subtracts the offsets to return to source coordinates.
Native capture leaves the original arrays untouched. Identity round-trips
bit-for-bit, while offset `(17,-33)` changes restored `(0.1,-0.3)` through
rounding; those collision-space round trips do not replace retained source data.

Except exact ObliquePen, producer `0x520078` chooses a coefficient through
ordered, case-sensitive substring searches at `0x520144`–`0x520468`:

| First matching family substring | Coefficient bits | Value |
| --- | --- | ---: |
| `Straight` or `straight` | `0x00000000` | 0 |
| `Marker` or `marker` | `0x3dcccccd` | 0.1 |
| `Brush` or `brush` | `0x3f733333` | 0.95 |
| Otherwise | `0x3e19999a` | 0.15 |

`0x520484`–`0x5204ac` stores `size * coefficient` at controller 184,
`size * (1 - coefficient)` at 188 and a factor at 192. Exact full FountainPen
identity selects factor 0.25; the other ordinary names select 0.5:

```text
R(p) = f32_fma(controller184, source_pressure, controller188) * controller192
```

This collision radius does not replay the pen's rendering kernel or intersect
its final visible outline. These calculations do not query source timestamps,
tilt or orientation or clamp pressure. Captured negative/above-one pressures
exercise the same arithmetic without establishing editor admission of them.

TapePen's exact-name branch, `0x520538`–`0x520978`, uses stroke rotation from
virtual slot 136, converts degrees with `0x3c8efa35`, and calls `sincosf`.
Model vtable relocation `0x4930a8` resolves that slot to
`ObjectBase::GetRotation`, `0x2cbd08`. It tests a rotated spine extending
`0.75 * R` on each side with a `0.25 * R` cap. This is a capsule/strip heuristic;
arbitrary tape angles were not executed by the captures below.

Exact ObliquePen instead uses axis-aligned offsets with half-extent
`(size * 0x3f3504f3) * 0.5`, at `0x5201ec`–`0x520440`. That classification
ignores pressure and bypasses writes to 184/188/192, although the later endpoint
helper still reads them. Constructor `0x51d80c`/`0x51d818` seeds those fields
as 1/0/1. Native replay of identical Oblique source/statuses produces endpoints
±10.5 from those seeds versus ±12.775 after a preceding native size-6 InkPen
call. This inherited controller state is preserved by the resulting saved
points; an exporter cannot recover it by erasing an idealized outline.

## Collision statuses become retained parts

For ordinary CUT, segment/gesture proper crossings mark zero-status neighbors
as 1 at `0x5204ec`–`0x52051c`. Sample cap/strip tests then mark affected samples
as 2 at `0x52079c`–`0x520830`. Oblique and Tape use their dedicated predicates
before the same retained-run scanner at `0x520a9c`–`0x520b60`.
It starts a run at a 2→below-2 transition or a 1→1 pair, and ends it before
a below-2→2 transition or another 1→1 pair. Outside-array neighbors are 2.
Thus source X `[-100,100]`, pressures `[0,1]`, and vertical cutter movement
`(0,-20)`→`(0,20)` produce two one-point parts: `[0,0]` with tail −13 and
`[1,1]` with head +13. All-status-2 output instead queues a whole-delete task.

Generated XY is attempted only when neighbor distance is strictly greater than
`min(cutterRadius,20)`. `0x520ba0`/`0x520bb0` selects 20 when radius is greater;
native comparisons confirm thresholds 10 and 20 for radii 10 and 40.
Tail calls `0x521334` with indices `end,end+1` and selector 1; head uses
`start-1,start` and selector 0. Rejected coordinates are discarded even if the
helper already wrote its output.

A proper crossing selects the larger endpoint radius at `0x521428`–`0x521440`.
Otherwise cap/strip tests select the radius of the involved endpoint. The helper
adds cutter radius at `0x5214b8`, without interpolating pressure or radius at
the intersection. With size 6 and pressures `[0,1]`, source `[-100,0]` yields
tail −13, while `[0,100]` yields head +12.55. The later channel-array builder
independently copies pressure/time/stylus values from retained source samples.

For `abs(dx) <= 0.5`, `0x5214ac`–`0x521528` treats the source as vertical at
its first X. Otherwise the quadratic mixes earlier `f32` products/FMA with
`f64` discriminant/root arithmetic at `0x521578`–`0x5215d8` and
`0x521760`–`0x5217bc`, narrows roots to `f32`, then computes Y by `f32` FMA.
`0x51f388` orders candidates along source direction; selector 1 chooses the
first and selector 0 the second. Component boundary rules differ:
`0x51f158` rejects determinant-zero touches, `0x51f1fc` accepts its cap-radius
equality, and `0x51f28c` accepts its straight strip boundary. The producer ORs
these predicates; one component's rejection does not establish a cutter miss.

## Cut tasks contain retained ranges, not a saved erase path

`WritingRemoverCutExecuter::Cut`, `0x522438`, is identified by its signature
string at `0x1c7596`. Its input vector advances in 80-byte task records. Each
task contains a vector of 56-byte parts, which supply a source stroke and a
retained inclusive sample range to helper `0x523998`.

The fields consumed by that helper are:

| Part offset | Consumption |
| ---: | --- |
| 8 | Signed first sample index |
| 12 | Signed last sample index, inclusive |
| 16 | Signed Y offset used only by the optional vertical rejection gate |
| 24 | Nullable pointer to a generated head coordinate |
| 40 | Nullable pointer to a generated tail coordinate |

The task's generated coordinate pointers are runtime data. They are not a new
serialized mask channel. The helper reads the source's ordinary point,
pressure, timestamp, tilt and orientation arrays through the existing
`ObjectStroke` getters at `0x523a28`–`0x523a58`.
The producer stores one XY holder for each accepted head or tail. Repeated tail
samples arise in this array builder, not from two tail-coordinate holders.

For `last < first`, helper `0x523998` returns false before constructing arrays.
The instructions otherwise compute the output sample count as:

```text
N = last - first + 1
N += head_pointer != null ? 1 : 0
N += tail_pointer != null ? 2 : 0
```

These bounds belong to an editor-generated part. The trace does not prove
bounds checking against arbitrary malicious source-array indices.

## Generated endpoints retain native sample multiplicity

At `0x523aa4`–`0x523adc`, a nonnull head contributes one generated XY pair.
Its pressure and timestamp are copied from source sample `first`.
`0x523aec`–`0x523b44` then copies the inclusive original range as complete
channel slices. Head presence shifts the destination by one sample; it does
not replace the first original sample.

At `0x523b48`–`0x523bd0`, a nonnull tail contributes the same generated XY
pair twice. Both new samples copy pressure and timestamp from source `last`.
The repeated tail is intentional instruction-level behavior and was reproduced
by native execution. It must not be deduplicated merely because the two XY
pairs are equal.

Tilt and orientation are copied only when both source getters return nonnull
pointers. Their generated head/tail values come from `first`/`last` in the
same way, with two tail copies at `0x523d58`–`0x523d80`. The branch with either
pointer null uses the non-stylus `SetPoint` overload. This is a paired-channel
gate, not independent retention of whichever stylus channel happens to exist.

For a retained range containing a single original point, a head and tail
produce four output samples. Pressure, timestamps and stylus channels may all
repeat across those four samples even though the generated endpoints differ.
Those repetitions affect pen replay history and cannot be inferred solely from
the final visible outline.

## Timestamp origin and append time change separately

After constructing the arrays, the optional controller flag at member 76
checks each output Y against integer bounds at controller members 80 and 84.
It adds the part's integer offset at member 16 as `f32`. If every sample falls
outside the inclusive interval, the helper rejects the entire part at
`0x523c48` without `SetPoint`.

On accepted output, `0x523c64`–`0x523c80` subtracts the first output timestamp
from every output timestamp using 32-bit integer subtraction. Zero and repeated
timestamps are retained. The operation does not interpolate time at the
generated geometric intersection.

The fragment append time is computed separately at `0x523ca8`–`0x523ccc`:

```text
difference = signed_source_last_timestamp - signed_rebased_output_last_timestamp
scale = source.IsMillisecondMode() ? 1000 : 1
fragment_append_time = source.GetAppendTime() - difference * scale
```

The native multiplier's polarity follows `tst` plus `csneg`: the true
millisecond-mode branch selects the already multiplied negative difference.
Its name must not be used to invert the recovered arithmetic. Neither this
formula nor timestamp rebasing converts the source timestamp array before
copying it.

For supplied source timestamps `[100,120,140,160,180]`, retained indices
1–3 and original append time 100000, output timestamps are `[0,20,40]`.
The computed append time is 99860 with millisecond mode false and −40000
with it true. Adding head/tail repetitions preserves that final timestamp.
A one-point retained range has zero rebased final time.

## Fragments remain ordinary stroke objects

Helper `0x523998` has two mutation modes. With its fourth argument
bit 0 set, it calls `SetPoint` on the supplied stroke itself at `0x523dbc` or
`0x523e60`. Without that bit it creates object type 1, calls `Construct`,
invokes virtual slot 184 to copy the supplied source, and then replaces its
point arrays at `0x523f24`–`0x523f68` or `0x523fe8`–`0x52403c`.

The outer cut executor can also pre-create a type-1 object and copy a source
before selecting the in-place helper mode at `0x5228a0`–`0x5228cc` and
`0x522b78`. The other call at `0x5229d0` selects the helper's creation mode.
Consequently, “every part modifies the original object” and “every helper
invocation creates a new object” are both incorrect descriptions.

The outer executor queues source objects for removal and resulting objects
with their insertion positions. Its ordinary document calls are:

| Call | Document slot | Operation |
| --- | ---: | --- |
| `0x522f78` | 96 | Remove the source-object list for a page |
| `0x523380` | 56 | Insert the result-object list with its position array |
| `0x52344c` | — | Assign each inserted object's computed append time |

For the mode-0 `WritingWNote` adapter from
[stroke insertion findings](stroke-insertion-findings.md), slot 96 resolves to
`0x4f727c` and forwards to `WPage::RemoveObjectList`. Slot 56 resolves to
`0x4f6e90` and forwards to `WPage::InsertObjectList(ObjectList const*,
ArrayListInt const&)` at `0x4f6f68`. This route sets each resulting stroke's
millisecond-mode flag true at `0x4f6f20`; it does not rescale its timestamp
array. The cut helper's append-time calculation occurs before that insertion
flag change, using the copied source flag.

The whole-object remover has a separate executor at `0x52687c`. It groups
selected objects by page and calls document slot 96 at `0x526a2c`, without
calling the fragment-array helper. Math/formula child removal additionally
calls `RemoveStroke`, `RemoveAnswerStroke` and `RemoveFormula` at
`0x526cdc`, `0x526cb0` and `0x526d50`. Those calls establish native child-list
mutation, not SDK formula rendering support.

Its generator `0x527140` invokes `0x527a8c`, with a width heuristic distinct
from CUT. Static dispatch at `0x527af0`–`0x527bf4` and `0x5282d4`–`0x5285e0`
gives Brush/brush coefficient 0.95 with factor 0.3, and Fountain/fountain
substring factor 0.25 instead of CUT's exact full name. Oblique/oblique and
Tape/tape specialized routes are substring-selected. The ordinary pair cap
at `0x528084`–`0x5280ac` adds neighboring pressures before its FMA/factor,
without averaging. These whole-remover calculations were not executed here.

## Saved state and editor history are separate boundaries

WDoc `WPage::InsertObjectList`, `0xc3d04`, delegates to Model
`PageImplBase::InsertObjectList`, `0x345028`, and its object handler.
`WPage::RemoveObjectList`, `0xc3f08`, delegates through
`PageImplBase::RemoveObjectList`, `0x345044`, and `LayerDocBase` to the
object manager. These calls alter the current document object collection.
They are more than an editor preview that only paints transparent pixels.

The page writer's layer route is WDoc
`WPageSaveHandler::Save_LayerDoc`, `0xd6b60`: it obtains the current layer
list at `0xd6ba4` and calls each layer's `WLayer::Save` at `0xd6c2c`.
Model supplies `LayerDocSaveHandler::Save_Objects_WDoc`, `0x3552bc`, and
`WriteDefaultObject`, `0x354f78`, for ordinary saved records. Combined with
type-1 fragment creation, this connects the mutation to the existing object
serialization route. It is static save-path evidence, not an executed
cut/save/reload capture.

Undo listeners, detached originals and gesture callbacks may retain earlier
objects outside the current layer list. Their presence does not make those
objects additional visible saved strokes. The complete eraser-specific undo
packing and document-history persistence contract has not been recovered;
this trace does not assert that every archive discards all edit history.

## Saved eraser properties and Masking are independent

Model `ObjectStroke::SetEraserEnabled`, `0x2e1710`, and
`IsEraserEnabled`, `0x2e17a4`, operate on stroke-data byte 332. Normal WDoc
stroke property bit 3 serializes that flag; see
[stroke metadata findings](stroke-metadata-findings.md#properties-and-polarity).
The handwriting fragment helper replaces ordinary point channels and does not
set that flag or create an Eraser-named stroke. Its existence does not establish
how brush eraser objects should render.

### Stored state and redraw inputs

Model ordinary object copy preserves byte 332 at `0x2e3b40`–`0x2e3b54`;
history packing and equality also inspect it. The separate draw-data copy,
`CopyDrawingData`, `0x2eb144`–`0x2eb3fc`, copies point channels and drawing
settings but omits that byte. PenCommon's JNI `setEraserEnabled`, `0x414a8`,
and `isEraserEnabled`, `0x414b0`, are literal `mov w0, wzr; ret` stubs.
The Java pen-mode setter treats the false return as an error.

Drawing `drawObjectStroke`, `0x81a20`–`0x825a4`, obtains a pen by saved name,
applies settings and passes the object to `redrawIPen` at `0x82590`, with no
identified eraser-flag getter or setter call. Marker2 V1's saved-object adapter,
`0x21c58`–`0x21d1c`, and Eraser's adapter, `0x1a610`–`0x1a6d4`, obtain tool
type, count, XY, pressure, timestamps, tilt and orientation for a `MotionEvent`.
Neither obtains eraser byte 332 or partial-rectangle data.

With supplied three-point native state, toggling byte 332 from 0 to 1 leaves
every constructor argument unchanged for both adapters. Actual Model getters
and `CopyTempPointToRealPoint` execute with empty temporary vectors; capture
stops at the external `MotionEvent` constructor entry, before its body, event
redraw or painting. This establishes the adapter boundary, not every saved
eraser-enabled object's appearance.

### The dedicated Eraser compositor

Ordinary Marker2 V1 selects `uIsEraserMode = 0` at `0x24208`; see
[pen opacity](pen-opacity-findings.md#marker2-v1-separates-mask-coverage-from-color-composition).
Eraser's separate `EraserStrokeDrawableRTV1::Draw` calls its mask at
`0x1bb2c` and compositor at `0x1bb88`. Its `drawComposite`, `0x1bdb8`, disables
blending at `0x1bddc`, binds the existing-layer and stroke-coverage textures,
and supplies member 56 as uniform `Alpha` at `0x1be84`–`0x1be94`. Shader
construction identifies the texture/uniform slots at `0x1d060`–`0x1d0a4`.

Embedded `EraserCompositeShader::szFragmentShader`, `0xdead`, attenuates
existing premultiplied color `Cd` and alpha `Ad` using coverage `m` and
drawable alpha `a`:

```text
Aout = Ad * (1 - a * m)
Cout = Cd * (1 - a * m)
```

Nonpositive resulting alpha outputs zero; already transparent destination
fragments are discarded. For ordinary alpha/coverage inputs this is a
destination-out paint operation. This is static compositor evidence: it does
not establish that saved bit 3 selects the plugin, that ordinary CUT saves an
Eraser object, or that common Masking objects subtract paint.

### Render-layer selection

Common render-layer ID 2 is called Masking, but
[capture composition](capture-composition-findings.md#per-object-render-layer-selection)
shows that it is a pass-selection field. TapePen insertion assigns that ID.
It is neither stroke eraser bit 3 nor a persisted partial-cut command. A mask
used to union pen stamps is another independent geometry/compositing concept.
Converting all Masking objects into subtractive vector paths would misread
the recovered fields.

## Legacy rectangles are skipped by modern native loading and writing

Common flexible field 1 contains a `u16` count and `16 * count` rectangle
bytes. Model `m_ApplyOwnBinary_FlexibleArea`, `0x2db794`–`0x2db7c0`, writes
the count to `BaseData + 104`, bounds-checks the payload, and advances the
caller pointer without reading or copying coordinates. Its separate
`sm_GetBaseData_FlexibleArea`, `0x2dc238`–`0x2dc24c`, likewise retains only
the count and advances by `2 + 16 * count`. `GetPartialRectCount`,
`0x2caacc`, reads that count. The stroke field 5 reader uses it to skip
`4 * count` companion bytes at `0x2ed974`–`0x2ed998`. These readers do not
interpret those payloads as erasure intervals; their historical numeric
meaning remains unresolved.

Executed prefixes with counts 0, 1, 2 and 17 and arbitrary coordinate bytes
consume exactly those spans. Only the rotation-default bytes 68–71 and count
bytes 104–107 change in supplied `BaseData`. Both captures stop immediately
after the rectangle branch, before later flexible fields.

A separate capture then executes the same object's complete modern public
`ObjectBase::NewGetBinarySize`, `0x2d11ac`, and `NewGetBinary`, `0x2d1248`.
`GetOwnBinarySize` adds `2 + 16 * count` for positive counts at
`0x2da8b4`–`0x2da8e0`, but `GetOwnBinary`, `0x2daad8`, omits flexible field 1:

| Loaded count | Estimated capacity | Emitted frame bytes | Unused capacity |
| ---: | ---: | ---: | ---: |
| 0 | 121 | 121 | 0 |
| 1 | 139 | 121 | 18 |
| 2 | 155 | 121 | 34 |
| 17 | 395 | 121 | 274 |

All twenty emitted frames are byte-identical, with declared length 121 and
flexible mask `0x40000`, whose rectangle bit 1 is absent. The retained count
is unchanged and every unused output byte remains at its supplied fill. The
extra estimate is unused capacity, not a malformed declared frame length.

The writer capture supplies zeroed object/base state, explicit `-1` media/user
IDs, a valid UUID and empty Bundles. Native Base UUID methods, Bundle emptiness,
point predicate, Model format-version dispatch and complete modern common
writer execute unchanged. Remaining loader fields, constructors, stroke-specific
writing, archive saves and older compatibility representations do not execute.

## Bounded native execution and corpus limits

Temporary Rust Unicorn probes execute unchanged APK instructions through the
repository's native-machine loader. Ink groups compare initial memory fills
`[0,85,165,255,0]`, except the 13 width-prefix calls; rectangle/redraw groups use
`[0,85,170,255,57]`. Independent rebuild/replay reproduced every output hash:

| Capture | Native calls | Output SHA-256 |
| --- | ---: | --- |
| Fragment array helper `0x523998` | 100 | `f6abeb0fcfa3e486ee168a22c80a1c2f2852da0b41936a2cc62174293db4bc8a` |
| Width prefix and endpoint `0x521334` | 13 + 290 | `b92cad9c977f02a435b23d95fce0602ab7513194567cf24c0cf89f9e12750439` |
| Collision/part producer `0x520078` | 60 + 5 prior Ink calls | `2d8851942195c29669006667eb87875260abb76515eaef8a4c265c9f8918a261` |
| Three collision predicates | 240 | `b42d9ab8afd588412fe56026ff4b691449edf2c80cb14320e8f842370cf16505` |
| Preparation/inverse helpers | 155 | `0319e59807874ba4eae27d796d80602ccde9ee54dbf3d99b7a8271d4127ae547` |
| Rectangle loaders, redraw adapters and pen-mode stubs | 40 + 20 + 10 | `1dc3cd75a938cf0d47dbc2bfe98466b2ba765d2537f3420f06373fc275c77a6c` |
| Reader prefix → modern common size/write | 20 × 3 | `347db6dd8e0818c4e2e07f3183cfeab368c2bf25d52c5da60d8e992077251693` |

The array capture supplies five source samples, range/endpoint cases, paired
stylus presence and millisecond mode. It hosts source getters, append time,
allocation/memory-copy imports and terminal `SetPoint` recording. Computed
append time comes from native register/spill state. Helper mode 1 executes
through construction of arrays, stopping before the `SetPoint` body; it does
not certify the collision producer by itself.

The separate producer capture supplies already prepared PointF/pressure arrays,
`IsTopLayerPen=true`, target 0, constructor coefficient seeds and ASCII strings.
It executes collision, generated XY, part allocation and reference counting,
stopping at `0x520f50` before Factory/Copy or at `0x521204` before the deletion-
task builder. The 290 endpoint calls execute the complete endpoint helper.
Base PointF setters and RectF offset/union operations execute natively.
The three predicate captures require no hosted geometry.

Preparation/inverse capture executes `0x51ffa4` and `0x521828` completely with
supplied getters, bounded memmove and a preallocated output vector. It inverts
prepared source samples, rather than generated intersections. Negative/tiny
scales, large offsets and unusual pressure values are supplied boundary inputs,
not editor admission evidence. Arbitrary Tape sin/cos, object copying, complete
gesture interaction, document mutation and binary save remain outside the ink
capture boundaries. These are local research captures, not retained SDK
conformance fixtures or a second erase implementation.

The rectangle/redraw inputs likewise come from supplied memory, not a saved
eraser-enabled document. Allocation and terminal constructor recording are
hosted. Modern common-frame writing runs completely; whole-object/archive
save, event redraw, graphics, shader execution and pixels remain outside these
captures.

The seven documents in the
[rendering corpus inventory](rendering-corpus-findings.md) contain 7,300
inspected strokes. All have saved eraser false and absent legacy partial-
rectangle data. That is not evidence that nobody erased those notes:
ordinary cut fragments are still normal strokes. The corpus has no paired
before/after erasing sequence that establishes the origin of any particular
fragment or proves full device parity for the operation.

## Consequences for the Rust vector pipeline

The existing stroke decoder retains every saved point, pressure, timestamp,
tilt and orientation, including repeated samples and zero-time runs. Ordinary
cut output fits its existing type-1 channel contract. A vector export consuming
saved fragments does not need a second editor erasing implementation to recover
that ordinary final state.

`StrokeProperties` retains saved eraser bit 3. Reconstructed fountain, Marker2
and Marker4 profiles reject it; the pressure fallback does not establish its
native rendering semantics. Metadata retention therefore does not certify rendering
of a saved eraser-enabled object. The ordinary handwriting cutter and that
rendering gap must be tracked independently.

Given original page bytes, metadata inspection retains raw common `[u8; 16]`
rectangle records and stroke `[u8; 4]` companions. `StoredObject` indexes a
borrowed payload; optional `StoredArchivePage.source_bytes` owns the original
page bytes when requested. Native loading and modern common writing cannot
recover the discarded coordinates. The retained source bytes and decoded
metadata therefore have distinct preservation boundaries;
see [vector retention](vector-retention-findings.md#original-page-source-ownership).

The preservation constraint is to retain source fragments, channel order and
style identity through geometry preparation. Equal XY samples cannot generally
be discarded because fragment endpoints deliberately repeat time, pressure
and stylus data. Replaying a guessed erase gesture or rasterizing a subtraction
would lose the native saved-state boundary recovered here.
