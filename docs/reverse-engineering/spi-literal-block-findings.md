# SPI mode-5 literal blocks

## Evidence and scope

Recovered from Samsung Notes 4.4.45.37 ARM64 `libSPenBase.so` in the
[identified APK](README.md#sources-and-validation). Addresses below refer
to that ELF.

Mode 5 carries literal byte planes. An independent scratch implementation
reconstructed these payloads in native-generated samples and assembled
complete images accepted by Samsung's decoder under Unicorn. This extends the
[native codec validation](spi-codec-validation.md) to an independently
specified block format. Other modes and general SDK decoding remain open.

The complete-image checks used API color value 500, wire color index 4,
header flags `0xe0`, and no packet buffer-copy shortcut. They do not
establish behavior for every header flag, color type or reference state.

## Alignment follows the four-bit mode prefix

The [mode prefix](spi-data-packet-findings.md#block-mode-prefixes-select-six-dispatch-entries)
for mode 5 is `0011`. Its payload callback is `0x68774` in the primary
pass and `0x68950` in the additional, alpha pass.

Both callbacks discard the remainder of the current byte. The primary
callback loads buffered-bit count and read pointer at `0x68790` through
`0x68798`, subtracts the number of buffered whole bytes, and clears the
bit buffer at `0x687a4`. The alpha callback does the same at `0x6896c`
through `0x6897c`.

In terms of the bit position at the start of the mode prefix:

```text
payload_byte_offset = ceil((mode_prefix_bit_offset + 4) / 8)
```

The skipped bits have no zero check. All 16 alignment nibbles after a
byte-aligned mode prefix were accepted in complete native image tests.
Prefixes may themselves cross byte boundaries: two native-generated
mode-5 blocks started at bit 7 of a byte. An independent reader must align
after consuming the complete prefix, rather than skipping a fixed byte.

## A literal plane is 256 bytes in row order

Routine `0x6a0c8` reads sixteen eight-bit values for each of sixteen rows.
The first and last reads within a row are `0x6a0f0` and `0x6a1e0`.
The row counter decrements at `0x6a1e4`, and the destination advances by
the supplied stride at `0x6a1ec`. These mode callbacks supply stride 16.

The primary callback invokes this routine three times:

| Plane | Worker pointer offset | Read call |
| --- | --- | --- |
| 0 | 8416 | `0x687ac` |
| 1 | 8424 | `0x687bc` |
| 2 | 8432 | `0x687cc` |

The primary payload is therefore 768 consecutive bytes: all of plane 0,
then all of plane 1, then all of plane 2. Within each plane, X changes
fastest. There are no per-row lengths or entropy codes in this payload.

The alpha callback invokes the same reader once at `0x68988`, using the
worker pointer at offset 8440. Its payload is one 256-byte plane.

The matching native writer corroborates the layout. After writing the
final two selector bits as 3 at `0x785a4`, it flushes to a byte boundary
at `0x785ac`. Calls at `0x785b8`, `0x785c4` and `0x785d0` write the
three planes through `0x7c57c`. That routine writes eight bits at
`0x7c5a4` and loops until it has consumed 256 source bytes at `0x7c5ac`.

## Plane bytes map directly to the tested pixel channels

For API value 500, a pixel's four memory bytes map as follows:

| Pixel byte offset | Literal source |
| --- | --- |
| 0 | Primary plane 2 |
| 1 | Primary plane 0 |
| 2 | Primary plane 1 |
| 3 | Alpha plane |

This mapping was checked against the original input bytes and native
plane buffers. It agrees with the input/output conversion traced in
[codec validation](spi-codec-validation.md#the-fourth-byte-connects-to-alpha-handling).
No red/blue channel names are assigned here.

With the tested header flag B set, the primary pixel callback at
`0x5dccc` takes the direct branch at `0x5dd08`. Calls at `0x5dda0`,
`0x5ddbc` and `0x5ddd8` copy the three worker planes into the current
frame. Its destination offset is:

```text
(block_row * frame_stride + block_column) * 16
```

The alpha pixel callback at `0x5e2f0` copies its plane through `0x5e388`
at offset `pixel_y * frame_stride + pixel_x`. Edge blocks still contain
all 256 bytes per plane; output is cropped to the declared image width
and height. Independently assembled samples used zero for pixels beyond
those edges.

The native payload callbacks also clear worker byte 2493 and set the
per-column flag to 1 at `0x687e8` or `0x6899c`. They update auxiliary
marker arrays through `0x5e92c` and `0x5e9a0`. These state updates still
matter when tracing interactions with other block modes; literal pixel
copying alone does not specify the complete mixed-mode decoder.

## Complete literal images establish packet and pass ordering

The independent constructor used the following previously recovered
[header](spi-header-findings.md) values: capacity dimensions zero, actual
image dimensions, color index 4, row-group size, additional-buffer count
zero, and flags `0xe0`.

Each [kind-2 packet](spi-data-packet-findings.md) had the corresponding
group index, byte A/B zero, shortcut zero, selector 2, bytes C/D/E equal
to 24/23/5, and reserved bits zero. Those auxiliary values were copied
from the native-generated samples; their wider semantics remain unassigned.
The packet's big-endian length was set to the complete constructed length.

For each group, the constructor emitted every primary-pass block in row
order, then every alpha-pass block in the same order. It did not alternate
color and alpha after each spatial block. Each block used a byte containing
`0011` plus four padding bits, followed by its literal planes.

For these all-literal packets:

```text
primary_block_bytes = 1 + 3 * 256 = 769
alpha_block_bytes = 1 + 256 = 257
packet_bytes = 14 + group_block_count * 1026
complete_spi_bytes = 28 + packet_count * 14 + total_block_count * 1026
```

The 28 outer bytes comprise two four-byte block lengths and the 20-byte
header. These exact sizes describe this construction. They do not bound
other encodings; the earlier [capacity failure](spi-codec-validation.md#a-wrapper-derived-capacity-failed-in-the-single-worker-experiment)
occurred in native output using a different mixture of modes.

Six tests split 33×49 images into groups of one, two or three block rows,
producing four, two or two packets respectively. Both decoders recovered
the pixels exactly, including the clipped final group. The packets were
provided to the native single-worker consumer together in one data block.
Out-of-order groups, duplicate groups and threaded decoding were not tested.

## Validation

- Reconstructed all 24 mode-5 primary blocks from eight native-generated
  gradient/random samples. Independent plane bytes and final bit positions
  matched the native reader; visible color bytes matched the source pixels.
- Checked literal reads at all eight possible starting bit positions with
  zero and one padding bits, for both one and three planes: 32 native cases.
  These stopped after plane reading and before auxiliary-state updates.
- Constructed 30 complete all-literal images covering the six sizes and
  five patterns in [codec validation](spi-codec-validation.md#synthetic-coverage).
  The independent decoder and Samsung's decoder both recovered every
  input byte. These samples exercised mode 5 in both passes.
- Checked six multiple-packet images and all 16 padding-nibble values on
  a 17×17 random image. Both decoders recovered the original pixels.
- Rejected ten truncated-plane cases in the independent reader. These
  checks do not characterize native malformed-input handling.

The 52 complete constructed-image checks executed 2738 distinct native
instructions. The separate alignment checks executed 238 native
instructions without host replacements. APK identity, instruction bytes,
writer/reader correspondence and recorded output hashes were verified.

Representative independently constructed SPI outputs are:

| Input | SPI bytes | SHA-256 |
| --- | --- | --- |
| 1×1 solid `12 34 56 ff` | 1068 | `5958db83eb60c1a1cad0e94552ef5e4158e7814ee276c3330ad89bf10273dacb` |
| 16×16 gradient | 1068 | `23d083ff6d99524966f9660c504425f240323c7d4061474448bfb80db884e6c4` |
| 33×49 random, one packet | 12354 | `503e3db4122d589751dde5370a10a9a4092d81e0fe24b15d1c14636eb1a3f8b6` |

These differ from the native encoder's outputs because every block is
forced to literal mode. The maintained artifact is this specification;
the constructor, decoder, emulator and generated images remain disposable
local tooling. No SDK code changed.

## Remaining work

Next targets are the other modes' prediction and residual coding, the
auxiliary marker state, reference-buffer behavior, color/flag variants
and malformed-input limits. An independent decoder for arbitrary SPI
images is still incomplete. Device-exported files and rendered references
remain necessary for compatibility validation.
