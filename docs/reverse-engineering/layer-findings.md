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

## Native mask admission and metadata rewrite

The current WDoc route reaches the generic `Load_LayerData`, Model `0x3559a4`:
WDoc `WPageImpl::LoadLayerForChild`, `0xd0848`, calls `WLayer::Load` at
`0xd0894`; Model `WLayer::Load` calls that reader at `0x342188`. Saving reaches
`Save_LayerData_WDoc` through Model `WLayer::Save` at `0x341db4`.

`Load_PropertyFlag`, `0x355fc0`, and `Load_FieldCheckFlag`, `0x3560cc`, reject
mask widths greater than four before reading the mask bytes
(`0x35600c`–`0x356010`, `0x356118`–`0x35611c`). The helpers leave unread output
bytes unchanged, but `Load_LayerData` first clears both four-byte outputs
(`0x355a3c`, `0x355a58`), so accepted short masks zero-extend on this route.
Only property bits 0–4 and flexible bits 0–6 affect known runtime fields.
Accepted higher bits are not retained as source masks by this reader.

Unknown fixed bytes are skipped by seeking to the absolute flexible start
(`0x355ab8`–`0x355ad8`); unknown flexible tails are skipped by seeking to the
declared header end (`0x355ebc`–`0x355edc`). Its second `int&` output records
the cursor before the latter seek (`0x355eb8`), which can precede the header end.

The native writer reconstructs both masks as one-byte values
(`0x354334`–`0x354348`, `0x354728`–`0x354774`) from known runtime fields. It
does not copy the source masks or extension tails. Native admission of an
unknown bit or tail therefore does not establish its preservation during a
metadata rewrite; a five-byte mask instead fails admission.

A Rust harness using Unicorn's C API executed fourteen actual native mask-helper
calls and fifteen layer-header loads across implementation fills `00`, `a5` and
`ff`. Empty masks, bit 7 and four-byte bit-31 masks loaded; five-byte masks
failed. All nine accepted native rewrites matched their asserted 24-byte
known-field header for their runtime flags, dropping two fixed extension bytes
and three flexible extension bytes. File operations and diagnostics were host
boundaries. The exercised branch had no name, UUID or thumbnail and a default
shadow; constructors, page hashes and full archive saving were not exercised.
The capture SHA-256 is
`6be841dcb5969c85acb5982242010e142306850d4e8a9f39a34940403f2eac2b`.

The Rust metadata decoder retains full masks and both tails, including widths
this APK rejects. That preserves source structure without claiming native
loadability or meanings for higher bits. The basic `StoredLayer` low-byte
flags and `header_extra` are weaker: `header_extra` starts after the layer
number, excluding the preceding mask bytes. Logical
[integrity checks](integrity-findings.md) do not certify preservation of these
extensions.

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
Layer visibility, transparency, alpha lock and shadow effects are exposed as
metadata. This decoder does not apply those effects to the rendered page.
The semantic decoder selects the saved current physical layer, as documented
in [saved physical-layer selection](page-layer-selection-findings.md).

`crates/sdocx/tests/layer_metadata.rs` covers complete native fields, nonempty
Unicode strings, negative timestamps, independent layers, wide masks, unknown
tails, zero-offset headers without flexible fields, malformed offsets, every
truncated known-field prefix, invalid UTF-16 and allocation bounds.

Layer UUID and modified-time decoding supplies the missing input for logical
layer-hash verification through the optional checks described in
[integrity findings](integrity-findings.md). The discrepant transparency writer
and visible-shadow rendering lack matching captured evidence.
