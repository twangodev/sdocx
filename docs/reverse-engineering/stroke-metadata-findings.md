# Native stroke properties and pen metadata

## Evidence and scope

Confirmed against Samsung Notes 4.4.45.37 ARM64 `libSPenModel.so`. This
extends the earlier channel and style-prefix investigation. The native
property writer is `ObjectStrokeBinaryHandler::m_GetBinary_Property`,
`0x2ec080`, and the reader is `m_ApplyBinary_Property`, `0x2ed138`.
The flexible writer is `m_GetBinary_FlexibleData`, `0x2ec5dc`, and the reader
is `m_ApplyBinary_FlexibleData`, `0x2ed720`.

The SDK now exposes `StoredObject::stroke_metadata(page_bytes)` and
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

The raw two-byte tool/input field follows all point channels. `GetToolType`,
`0x2e1550`, reads stroke-data member 316 and normalizes values outside 0–4
to zero. `tool_type_raw` retains the stored value, including unknown values.

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

Bit 6 has no established serialized field contract. It and unknown higher
bits stop optional decoding before consumption, set `first_unparsed_field`,
and preserve the whole remaining flexible tail. A future field cannot shift
the known fields that follow it. Field 4 has a known one-byte width but no
confirmed semantic name.

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
color and width. It retains its existing RGB and width output and its
bounded handling of later optional fields. Explicit metadata inspection
provides the complete ARGB value, including transparent colors, and the
additional pen properties. It does not yet apply these fields to SVG or PDF.
Top-layer strokes require the separate capture selection and blend behavior
documented in [capture composition findings](capture-composition-findings.md).
The subsequent [pen opacity trace](pen-opacity-findings.md) confirms
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

Workspace tests with all features, Clippy with warnings denied, Rust 1.92
checking and the WASM target passed. The existing `01-basic-formatting`
corpus retained its locked hashes and parser/layout expectations, using the
temporary corpus copy with its reference PDF from the local LFS cache.
