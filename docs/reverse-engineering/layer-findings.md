# WDoc layer metadata

## Evidence

Analyzed Samsung Notes 4.4.45.37, APK SHA-256
`daed1eff8c8ee9dfb8afe2771e39e893a8808f3230d6d522a8aa647db09b8667`.
The evidence is decompiled Java and ARM64 `libSPenModel.so`; these findings
do not depend on a new Samsung document export.

| Source | Confirmed behavior |
| --- | --- |
| `LayerDocSaveHandler::Save_LayerData_WDoc`, `0x3542e8` | Absolute flexible offset, layer number and mask-ordered fields |
| `0x354480`–`0x3544a0` within that writer | Transparency is written as one byte |
| `0x354608`–`0x354664` within that writer | Field bit 6 contains a length-prefixed shadow effect |
| `LayerDocBase::IsAlphaLock`, `0x340a7c` | Implementation offset 212 is alpha lock |
| `LayerDocBase::IsShadowEffectVisisble`, `0x33dde0` | Implementation offset 213 is shadow visibility |
| `ShadowEffect::GetBinarySize` / `GetBinary`, `0x2b90a4` / `0x2b90ac` | Current shadow payload size is 20 bytes |
| Decompiled `n1/u.java:525-546` | Java reader follows the absolute offset and consumes one transparency byte |
| Decompiled `n1/u.java:1177-1251` | Java writer reserves 12 bytes, writes fields, then backfills the header |

## Layout

The layer header's size includes its size word and both fixed and flexible
fields. Its flexible offset is absolute within the uncompressed page, unlike
the relative flexible offset used by typed object frames. The next byte after
the layer header begins the four-byte top-level object count.

The fixed portion contains the layer number after the two length-prefixed masks.
Unknown fixed bytes can precede the declared flexible offset. Native property
bits are:

| Bit | Meaning |
| ---: | --- |
| 0 | Invisible |
| 1 | Event forwardable |
| 2 | Locked |
| 3 | Alpha locked |
| 4 | Shadow visible |

Flexible fields occur in ascending bit order:

| Bit | Encoding |
| ---: | --- |
| 0 | Transparency, `u8` |
| 1 | Background color, `u32` |
| 2 | Name, `utf16_u16` |
| 3 | UUID, `utf16_u16` |
| 4 | Modified time, signed `i64` |
| 5 | Thumbnail media ID, `u32` |
| 6 | Shadow effect, `u32` byte count followed by payload |

The native shadow serializer copies three four-byte numeric fields, four color
bytes and a final four-byte field. The layer writer tests the first three as
floating-point values when deciding whether an effect differs from the default.
The SDK currently retains this payload without assigning names to its numeric
fields or interpreting its rendering behavior.

## Java transparency discrepancy

The decompiled Java writer calls `f2.a.U(randomAccessFile, i34)` for non-default
transparency (`n1/u.java:1200-1206`), writing four bytes. Its reader uses
`readByte()` and the native writer explicitly writes one byte. These paths
therefore disagree in the analyzed APK. This is a source-level discrepancy;
no captured export establishes whether the four-byte path is used for such
layers in practice. The SDK decoder follows the one-byte native/read contract.
It does not guess a writer variant from arbitrary padding or identity strings.

## SDK behavior and validation

`StoredLayer` records its header offset and size. `metadata(page_bytes)` and
`metadata_with_limits(page_bytes, limits)` expose the known fields using the
original uncompressed page bytes. They preserve full masks, unknown fixed and
flexible tails, and the sized shadow payload. Optional fields remain absent
when their mask bits are unset; they do not invent layer UUIDs or timestamps.

Metadata decoding is explicit. Structural parsing can still retain a layer
whose metadata is unknown or malformed, while a metadata request returns the
specific error. The decoder bounds fixed fields and flexible fields separately,
honors text limits, and cannot consume an object's count, hash or sibling layer.
Layer visibility, transparency, alpha lock and shadow effects are exposed for
future rendering work; this change does not apply them to the rendered page.
The semantic decoder selects the saved current physical layer, as documented
in [saved physical-layer selection](page-layer-selection-findings.md).

`crates/sdocx/tests/layer_metadata.rs` covers complete native fields, nonempty
Unicode strings, negative timestamps, independent layers, wide masks, unknown
tails, zero-offset headers without flexible fields, malformed offsets, every
truncated known-field prefix, invalid UTF-16 and allocation bounds.

Layer UUID and modified-time decoding supplies the missing input for logical
layer-hash verification. The next step is optional integrity reporting across
objects, layers, pages, the note trailer and manifest links. New captures should
include non-default layer transparency and visible shadows to validate the
discrepant writer path and measure rendering fidelity.
