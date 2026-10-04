# Native stroke properties and pen metadata

## Evidence and scope

Confirmed against Samsung Notes 4.4.45.37 ARM64 `libSPenModel.so`. The native
property writer is `ObjectStrokeBinaryHandler::m_GetBinary_Property`,
`0x2ec080`, and the reader is `m_ApplyBinary_Property`, `0x2ed138`.
The flexible writer is `m_GetBinary_FlexibleData`, `0x2ec5dc`, and the reader
is `m_ApplyBinary_FlexibleData`, `0x2ed720`.

The SDK exposes `StoredObject::stroke_metadata(page_bytes)` and
`stroke_metadata_with_limits`. `StrokeMetadata` contains the common base,
native property flags, point count, raw tool type, optional pen settings,
original masks and trailing object data. It inspects the normal WDoc
type-0/type-1 chain. The alternate coedit representation, which embeds
strings instead of normal string-table IDs, remains outside this API.

## Properties and polarity

The existing [property bit table](file-format.md#stroke-property-mask) was
rechecked against both the property reader and the named getters:

| Property | Stroke-data offset | Getter |
| --- | ---: | --- |
| Compressed/curve representation | 40 | `IsCurveEnabled`, `0x2e067c` |
| Replay-only | 324 | `IsReplayOnlyEnabled`, `0x2e0790` |
| Eraser | 332 | `IsEraserEnabled`, `0x2e17a4` |
| Millisecond timestamps | 333 | `IsMillisecondMode`, `0x2e1408` |
| Fixed width | 334 | `IsFixedWidthEnabled`, `0x2e18bc` |
| Fixed opacity | 335 | `IsFixedOpacityEnabled`, `0x2e708c` |
| Top-layer pen | 341 | `IsTopLayerPen`, `0x2e1ea0` |
| Alpha lock | 360 | `IsAlphaLock`, `0x2e25a4` |
| Binary-added | 381 | `IsBinaryAdded`, `0x2e5aa8` |
| Rainbow effect | 404 | `IsRainbowEffectEnabled`, `0x2e318c` |
| Generated | 468 | `IsGenerated`, `0x2e2b34` |
| Reveal mode | 492 | `IsRevealMode`, `0x2e7498` |
| Straighten | 493 | `IsStraighten`, `0x2e7598` |

Bit 2 is the reader's separate output indicating stylus-channel presence.
The SDK exposes it as `stylus_channels`. Bits 8 and 10 have inverse polarity:
their absence sets `binary_added` and `generated` to true. This is the
reader's explicit assignment, not a guessed constructor default. The writer
likewise sets those bits only when the corresponding native members are
false. Unknown property bits remain in `property_mask`.

The [straight-stroke producer](stroke-recording-findings.md#straight-stroke-creation-materializes-new-channels)
marks shape identity separately: native member 380 is restored from common
ExtraData integer `extra_key_stroke_shape`, a typed Bundle field separate from
straighten member 493/property bit 13, generated's inverse bit 10 and alignment.

The raw two-byte tool/input field follows all point channels. `GetToolType`,
`0x2e1550`, reads stroke-data member 316 and normalizes values outside 0–4
to zero. `tool_type_raw` retains the stored value, including unknown values.

## Tape visibility and reveal controls

Static Java/ARM64 tracing against the pinned APK connects app tape controls to
saved properties and rendering; no native save/reload or pixel comparison was
executed. Model/WDoc hashes are pinned in the
[common-object findings](object-base-findings.md#evidence-and-scope). Other
libraries used by this trace have these SHA-256 values:

| Library | SHA-256 |
| --- | --- |
| Composer | `52b83157198368da3a3855a721bfc7d3aafde4e644ce25b5d6eab3b6b510d39f` |
| Drawing | `788bf413ddeb0b9d352062c5f1b7b8ed11babca911df72691da58ff1a0a5a4bd` |
| TapePen | `ca0793d7cc8759ca6b21770cf866b796ddb59c2bd219798eb653061076c95d07` |
| Renderer | `f38df5db5e64f80c0641b6cee980e14533bd78e8e6eb34f250bd703d28119aed` |
| Graphics | `aac858ce3a9d0353d760b4b0ef09f1e88b0d4a87f5e0906fe8d53936ee8a6621` |

The app's `HwToolbarTapePen.functionEnable` applies tape settings through
`HwSettingTapePenInfo` (`HwToolbarTapePen.java:40–44`,
`HwSettingTapePenInfo.java:48–51`). The existing
[insertion trace](stroke-insertion-findings.md#the-pen-action-dispatches-the-completed-object)
appends the recorded TapePen stroke and assigns render-layer ID 2; that pass ID
is separate from the saved reveal property and physical layer selection.

The Hide All Tapes listener (`HwSettingTapePenLayout.java:279–287`) negates the
note's preference and reaches `HwSettingPresenter.java:687–694`. This writes
`SpenWNote.setTapeVisibility` before delegating to the live writing manager.
WDoc `SetTapeVisibility`, `0x9ee74`, changes note byte 852 and marks it dirty;
`SaveNoteFile` reads it at `0xaf4bc` and emits inverted property bit 4 at `0xaf598`.
See [note-header findings](note-header-findings.md). The live delegation requires
initialized `WritingToolManager` and a nonzero native view pointer, so the saved
note preference alone does not prove that every per-object update ran.

Composer JNI `0x321494` reaches `WritingView::SetAllTapePenVisibility`,
`0x5336a0` (signature string `0x1cb2f2`). It requests stroke objects in render
filter 4, then requires the exact TapePen name. It calls `SetRevealMode(!visible)`
on returned original stroke pointers when needed (`0x5337f0–0x533830`), retaining
the adapter's collection/layer scope rather than proving every archived layer
was visited. Model `SetRevealMode`, `0x2e73ec`, directly changes only byte 492
and calls `SetChanged` (`0x2e7440–0x2e7444`); the property writer/reader emit and
restore positive bit 14 (`0x2ec190–0x2ec1a0`, `0x2ed1ac–0x2ed1b8`). This is
saved state of the same object, without a direct point/UUID/geometry rewrite.
Callback side effects and conditional [save-time identity changes](object-base-findings.md#modification-time-during-native-saving)
remain separate from that direct-body claim.

Conditionally reached single/double-tap handlers call `handleTapePenRevealTap`
at `0x419ee4/0x41a000`; it reaches `ToggleTapePenRevealModeAtPosition`,
`0x5339ac`, and toggles the matched original TapePen's flag at `0x533bb4`,
then requests redraw. It does not write the note-wide preference in this body.
Remove All Tapes is another action: its listener dispatches `TaskEraseAllTapePen`,
whose `removeAllObject` calls `page.removeStrokes` with that TapePen name
(`TaskEraseAllTapePen.java:39–41`). Reveal is not that source-removal operation.

Drawing `drawObjectStroke` reads saved reveal at `0x82548` and forwards it through
the pen's pattern interface at `0x8255c`. TapePen `GetPatternable`, `0x2afd4`,
and its setter thunk `0x2b098` store data byte 18; saved-object `RedrawPen`,
`0x3739c`, queues it to RT `SetRevealMode`, `0x3c600`, via task `Run`, `0x3840c`.
The selected non-rainbow composite substitutes outline color with alpha ×0.8
(`0x3b9b0–0x3b9ec`). Reveal then attenuates the current destination by
`1 - 0.9 * interior_mask` (`0x3bc7c`, Renderer blend activation `0x514cc`).
Full interior coverage retains roughly 10% of prior output, not full transparency.

Target ownership differs: the direct non-rainbow branch activates the supplied
bitmap before ordinary and reveal composites (`0x3ae54/0x3aefc`). Graphics slot
40 resolves to `ActivateFrameBufferRT`, `0x8dfc4`. The other composite branch
uses a fetched intermediate before rebinding the supplied bitmap
(`0x3aa58–0x3aa88`, `0x3ad54/0x3adc4`). Previous contents of that supplied bitmap
remain untraced; neither Tape-only attenuation nor universal exposure of
underlying page ink is established. Raster surface attenuation does not itself
delete or subtract underlying stored vector geometry.

The [Standard PDF tape pass](standard-pdf-composition-findings.md#standard-list-page-paint-sequence)
reaches ordinary `ObjectDrawing` through its bitmap writer (`0x37c554`), so it
also reaches this consumer when TapePen resolution succeeds. That does not
establish every export policy, vector-only native output or pixel parity.
Rust retains note `tape_visible()` and stroke `reveal_mode`, but the current
production renderer reads neither; the plugin registry entry does not implement
these presentation controls.

## Optional pen fields

Fields are consumed in ascending mask-bit order. Their exact stored types
are exposed without substituting native defaults or resolving string-table
references:

| Bit | SDK field | Stored representation | Reader evidence |
| ---: | --- | --- | --- |
| 0 | `legacy_pen_name_id` | `i32` | Read `0x2ed798`; fallback assignment `0x2eda38`–`0x2eda68` |
| 1 | `advanced_pen_setting_id` | `i32` | Normal WDoc read `0x2ed8ac`, store member 32 at `0x2ed8d0` |
| 2 | `color_argb` | `u32` | Read/store member 288 at `0x2ed8ec`–`0x2ed8f0` |
| 3 | `pen_size` | `f32` | Read to member 292 at `0x2ed904`–`0x2ed918` |
| 4 | `field_4_raw` | `u8` | Read to member 312 at `0x2ed968`–`0x2ed96c` |
| 5 | `legacy_partial_rectangle_data` | Four bytes per common partial rectangle | Count and bounded skip `0x2ed974`–`0x2ed998` |
| 7 | `pen_name_id` | `i32` | Normal WDoc read `0x2eda10`, store member 16 at `0x2eda34` |
| 8 | `fixed_width` | `f32` | Read to member 336 at `0x2eda78`–`0x2eda8c` |
| 9 | `size_level` | `i32` | Read to member 296 at `0x2edabc`–`0x2edacc` |
| 10 | `particle_density` | `i32` | Read to member 300 at `0x2edae4`–`0x2edaf4` |
| 11 | `rendering_level` | `i32` | Read to member 308 at `0x2edb0c`–`0x2edb1c` |
| 12 | `original_width` | `i32` | Read to member 320 at `0x2edb40`–`0x2edb50` |
| 13 | `initial_tolerance` | `f32` | Read to member 356 at `0x2edb64`–`0x2edb78` |
| 14 | `line_type_raw` | `u16` | Read to member 384 at `0x2edbb8`–`0x2edbbc` |
| 15 | `dash_offset` | `f32` | Read to member 388 at `0x2edbd4`–`0x2edbe4` |
| 16 | `stroke_type_raw` | `u16` | Read to member 464 at `0x2edc0c`–`0x2edc10` |
| 17 | `pen_repeat_distance` | `f32` | Read to member 472 at `0x2edc1c`–`0x2edc30` |
| 18 | `particle_size` | `f32` | Read to member 304 at `0x2edc48`–`0x2edc58` |
| 19 | `pattern_index` | `i32` | Read to member 476 at `0x2edc84`–`0x2edc88` |
| 20 | `pattern_scale` | `f32` | Read to member 480 at `0x2edca4`–`0x2edcb4` |
| 21 | `particle_level` | `i32` | Read to member 484 at `0x2edce0`–`0x2edce4` |
| 22 | `rainbow_distance` | `i32` | Read to member 408 at `0x2edd0c`–`0x2edd10` |
| 23 | `rainbow_offset` | `f32` | Read to member 416 at `0x2edd28`–`0x2edd38` |
| 24 | `gradient_colors_argb` | `u16` count, then `u32` ARGB values | Count at `0x2edd78`; bounded value loop `0x2edda4`–`0x2eddd0` |
| 25 | `color_type_raw` | `u16` | Read to member 488 at `0x2eddec`–`0x2eddf0` |

The integer/float distinctions also match `GetSizeLevel`, `GetParticleDensity`,
`GetRenderingLevel`, `GetOriginalWidth`, `GetParticleSize`, `GetPatternIndex`,
`GetPatternScale`, `GetParticleLevel`, `GetRainbowDistance` and
`GetRainbowOffset`. In particular, original width is an integer field;
particle size and pattern scale are floating-point fields.

### Legacy pen names and partial rectangles

The reader retains field 0 while processing the other fields. If the
pen-name member is `-1` and
the legacy value is not `-1`, `0x2eda38`–`0x2eda68` resolves/copies that
legacy reference into the pen-name member. The SDK keeps both
stored references independently, including signed sentinel values.

The original inspection mislabeled fields 1 and 7 and consequently called
field 0 a legacy advanced-settings reference. The subsequent getter trace
corrected this: `ObjectStroke::GetPenName`, `0x2de974`, reads implementation
member 16; `GetAdvancedPenSetting`, `0x2dec00`, reads member 32. Both the
reader and writer agree with these getters. See
[pen selection findings](pen-selection-findings.md#stored-reference-identity)
for the complete chain and the regression that resolves a pen name and
version through deliberately distinct string IDs.

The call at `0x2ed978` is `ObjectBase::GetPartialRectCount`, followed by a
left shift of two at `0x2ed97c`. Field 5 consequently occupies four bytes
per common partial rectangle. Its elements are skipped by this reader;
their numerical meaning remains unresolved. The SDK preserves each element
as four raw bytes. It obtains the count from common flexible field 1,
after any decoded rotation, and validates the count's 16-byte rectangle
records against their own common frame. It does not use the stroke's point
count or search for a later pen setting to infer the boundary.

Field 4 remains unnamed. Constructor `0x2e87cc`–`0x2e87dc` zeros member
312 indirectly; the reader stores zero for absence at `0x2ed93c` and
zero-extends the present byte. Writer `0x2ec7e8`–`0x2ec808` emits its low
byte only when the 32-bit member is nonzero. A present zero therefore becomes
absent on native re-save, while the SDK preserves its `Option<u8>` presence.
The direct transfers in `Copy` (`0x2e391c`) and `CopyDrawingData`
(`0x2eb144`), and comparisons in `Equals` (`0x2e4160`), omit member 312;
this does not establish every indirect helper or callback's behavior.
`GetStrokeType`, `0x2e16b0`, uses member 464 and does not name field 4.

Bit 6 has no established serialized field contract. The native reader tests
bit 5 then bit 7 at `0x2ed940`/`0x2ed944`, without a bit-6 consume, skip
or error branch; that omission does not prove a zero-byte field contract.
The current flexible writer emits neither field 5 nor field 6 and does not
OR either bit; it does not clear caller-supplied bits. The SDK stops before
bit 6 or unknown higher fields, sets `first_unparsed_field`, and preserves
the remaining flexible tail so their bytes cannot shift later known fields.

Field 5's unresolved historical meaning and the separate modern common
rectangle writer omission are documented in
[eraser preservation](eraser-preservation-findings.md#legacy-rectangles-are-skipped-by-modern-native-loading-and-writing).

## Boundaries and rendering implications

Explicit inspection shares the fixed-channel boundary calculation with
ordinary stroke decoding. It checks the point limit before channel access,
including compressed, uncompressed, stylus and zero-point variants. It
does not allocate or semantically decode the coordinate arrays. Ordinary
stroke decoding still checks coordinates and channel values for finiteness.

The metadata payload is limited by `max_entry_size`. Legacy partial-rectangle
elements and gradient colors share the `max_object_metadata_entries` budget.
Their complete byte ranges are checked before allocation. Optional floats
must be finite; pen size also retains the existing nonnegative check.
Unknown enum values and negative integer values are retained. Absent fields,
present zero values and present empty lists remain distinguishable.

Ordinary `Stroke` decoding shares the style-prefix reader for pen references,
color and width. Explicit metadata inspection provides the complete ARGB value,
including transparent colors, and the additional pen properties. Rendering
consumes the supported subset: [fountain geometry and opacity](fountain-parity.md)
and [Marker4 geometry and opacity](marker4-rendering-findings.md) have separate
evidence. Decoding a property does not establish its rendering support.
Top-layer strokes require the separate capture selection and blend behavior
documented in [capture composition findings](capture-composition-findings.md).
The [pen opacity trace](pen-opacity-findings.md) confirms
alpha-preserving color conversion and Marker2 V1 mask/composite behavior.
It also establishes that the fixed-opacity setting has no effect through
the inspected DefaultPen and Marker bindings, while Marker2–4 expose no
morphable interface in that drawing path.

Eleven synthetic tests cover all 25 mapped fields, truncated prefixes of every
field, both inverted properties, future mask bytes, unknown-field stops,
signed IDs, transparent colors, empty lists, aggregate budgets, malformed
base rectangles, nonfinite style values and all four channel encodings.
They also distinguish pen-name IDs from advanced-settings IDs and preserve
legacy names independently of modern references and sentinel values.
The partial-rectangle test has zero stroke points and two partial rectangles,
so a mistaken dependency on point count cannot pass through alignment.
These are parser contract checks; new Samsung captures remain necessary
for visual conformance of the additional rendering properties.

## XML and separately transported stroke binary

Source-only traces use the same 4.4.45.37 ARM64 APK, SHA-256
`daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667`.
Both additional extracted ELF files were byte-compared with their APK members:

| ELF under `scratch/apk-analysis-native/arm64-v8a/` | SHA-256 |
| --- | --- |
| `libSPenWordDocCoedit.so` | `82a73d24732efe4f5c3c385b9fcb0ccfb05970507ba50039d252c2abb7f0c2af` |
| `libSPenXmlSerializer.so` | `7be7af380ae378f91c0e565dcc022479c3f87aa965a5190f245f4d006cb6f36a` |

WordDocCoedit's `CoeditObjectStroke` constructor, `0x3b4e8`, passes literal
XML format `1` to `ObjectXmlSerializerFactory::CreateObject` at `0x3b51c`.
XmlSerializer's factory dispatches native type `1` to `ObjectStrokeXmlSerializer`
(`0x110e70`–`0x110e90`); `initialSubSerializer`, `0x14c534`, selects its coedit
adapter at `0x14c56c`. `CoeditNote::GetStrokeData`, `0x47b54`, requests a wrapper
and calls that serializer's binary getter (`0x47c54`/`0x47c64`), appending returned
bytes/count plus UUID and modified-time strings without checking binary pointer/
count success. `ApplyBinaryData`, `0x41c28`, resolves a supplied UUID, checks
native type `1`, then calls its binary apply API at `0x41ce8`. These are concrete
callers; an output record does not prove successful receipt or storage of channels.

Coedit `ComposeAttribute`, `0x14cbac`, writes identity/hash/time, pen size,
fixed width, color, rectangle and rotation. Only library-global inclusion byte
`0x1885e8` enables its base64 `strokeBinary` attribute (`0x14ccc0`–`0x14ccf4`).
The setter `0x14c4bc` returns whether the flag changed; the note caller `0x47810`
refreshes cached XML only on that change. Its element composer `0x14ce4c` emits
no stroke fields. With inline binary disabled, these composers supply no separate
XY/pressure/time/tilt/orientation arrays; the separate binary getter remains
available. Sync has a different gate: `ComposeElement`, `0x14ddf0`, embeds a
base64 `strokeBinary` child only for `BelongsToContainer()` (`0x14de34`), with
reader application at `0x14dc68`. XML nesting alone does not establish completeness.

`m_GetStrokeBinary`, `0x14b738`, prefixes native data with 12 bytes: little-endian
words `0xffffffff`, `4`, `4000` at `STREAM_HEADER_ARRAY`, `0x187990`. Format `1`
calls Model `GetBinaryByCoedit(DocumentType=2)` at `0x14b81c`. Model `0x2e5ef0`
writes the common prefix then sets handler coedit mode true (`0x2e5f90`) and
uses the existing modern stroke writer. That writer commits temporary points
(`0x2ee3fc`), serializes the current XY/pressure/time tuple and optional stylus
channels, using the shared compressed reducer (`0x2ee428`) or promoted PointD
coordinates/raw channel arrays (`0x2ee48c`–`0x2ee548`). These bytes describe current
native data, not necessarily original pre-mutation archive bytes. Flexible bits
1/7 carry advanced settings/pen name as u16 UTF-16-unit counts and raw UTF-16
bytes (`0x2ec654`/`0x2ec81c`), instead of normal u32 string IDs. Their reader binds
strings into a live StringIDManager (`0x2ed804`/`0x2ed9e8`); inline names do not
make application independent of its model context. The wrapper is not an archive
record header, and the current Rust metadata API excludes this coedit variant.

Binary application requires model context. `m_ApplyStrokeBinary`, `0x14b924`,
selects page width for orientations `0`/`2`, height otherwise, then strips the
header using its offset-4 word plus eight; headerless coedit input is rejected.
The orientation-aware overload `0x14ce54` snapshots existing style, rectangle,
rotation, hash and time, and calls slot 416 (`0x14cf40`), resolved by Model
relocation `0x4931c0` to `SetBinaryByCoedit`, `0x2e5b08`. Its base loader derives
target/stored-extent magnification (`0x2db4b8`–`0x2db4cc`), and the stroke reader
scales decoded XY when nonunit (`0x2ee8bc`–`0x2ee8f0`). Afterwards the overload
restores style, calls actual `SetRotation` then `SetRect(false)` and restores
hash/time (`0x14cf48`–`0x14cfb8`), even if inner application returned negative.
Those setters can rotate/rescale XY; metadata restoration is neither unchanged
received coordinate bits nor a demonstrated transactional rollback.

Parsing runs common attributes before coedit stroke attributes (`0x14be3c`),
then child elements (`0x11c5cc`/`0x11c5f8`). Coedit applies nonempty binary first
(`0x14ca9c`), returning false on failure before later reads of XML color/size/
fixed width (`0x14cac0`/`0x14cad4`/`0x14cae8`). Present scalar attributes therefore
apply later; absent binary does not itself fail parsing. `m_OnFinishParsing`,
`0x14bd3c`, checks nonnull source/serializer state and sets a common implementation
flag if its pointer exists (`0x14bd64`–`0x14bd6c`), without proving received channels.

These static traces do not establish ordinary archive use of XML, actual server
payload modes or an executed exchange. Rust's existing channel representation
can retain decoded samples without another renderer; XML metadata, binary sidecar
and original archive bytes remain separate preservation boundaries. Neither
style-only XML nor parse success justifies reconstructing missing samples or
replacing original source bytes with the potentially transformed applied object.
