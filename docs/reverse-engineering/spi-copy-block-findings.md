# SPI block copies within a frame

## Evidence and scope

Recovered from Samsung Notes 4.4.45.37 ARM64 `libSPenBase.so` in the
[identified APK](README.md#sources-and-validation). Addresses below refer
to that ELF. Native routines were executed under Unicorn and compared
with independent scratch bit readers, writers and image reconstruction.

For the tested packet configuration, mode 0 copies the block to the left
or above; mode 1 selects the block above or supplies a displacement.
Together with [mode-5 literals](spi-literal-block-findings.md), these
operations now support independently constructed images containing three
different block modes.

This specification covers packet byte B = 0, stored at worker offset 8,
and header flag B set, stored at decoder-context offset 111. Complete
image tests used wire color index 4 and header flags `0xe0`, as in the
[native-generated samples](spi-codec-validation.md). The similarly named
packet byte and header flag are different fields. Nonzero packet byte B
enters reference-buffer paths that are not fully specified here.

## Mode 0 has no payload after its one-bit prefix

The [mode prefix](spi-data-packet-findings.md#block-mode-prefixes-select-six-dispatch-entries)
`1` selects routine `0x67fcc` in both passes. With packet byte B zero,
the routine loads one of two displacement pairs into worker offsets
8404 and 8408 at `0x6801c`:

| Block position | Displacement `(dx, dy)` | Constant |
| --- | --- | --- |
| Pixel X is nonzero | `(16, 0)` | `0x2a918` |
| Pixel X is zero | `(0, 16)` | `0x2a858` |

The source origin is `(pixel_x - dx, pixel_y - dy)`. Thus mode 0 copies
from the block immediately left, except at the left edge where it copies
from the block above. It consumes no additional bits and does not align
to a byte boundary. In this configuration the top-left block cannot use
mode 0: its source would be above the image.

## Mode 1 selects an upward copy or codes two displacements

Prefix `01` selects routine `0x68054`. Packet byte B zero branches to
`0x680dc`. After clearing worker byte 2495 and setting the column flag,
it reads one selector bit at `0x680f4`:

| Selector | Payload | Displacement |
| --- | --- | --- |
| 1 | None | `(0, 16)` |
| 0 | Two positive-integer codes, `H` then `V` | Signed horizontal offset and upward vertical offset |

The selector-1 branch at `0x681a4` sets X displacement to zero and Y
displacement to 16. The complete block code is therefore `011`.

For selector 0, the integer helper `0x6c1b4` returns a leading-zero
count `k` and stores the following `k` suffix bits. The represented
positive integer is:

```text
N = 2^k + suffix
```

Its bit representation is `k` zero bits, one set bit, then the `k`
suffix bits. For example, integers 1, 2, 3, 4 and 5 are encoded as
`1`, `010`, `011`, `00100` and `00101` respectively. The helper uses
the byte leading-zero table at `0x2a958`, consumes the prefix through
`0x6c240`, and reads the suffix through `0x6c250`.

The displacement calculation at `0x681c4` through `0x68204` is:

```text
horizontal_blocks = H / 2                 when H is even
horizontal_blocks = -floor(H / 2)         when H is odd
dx = horizontal_blocks * 16
dy = (V - 1) * 16
source_x = pixel_x - dx
source_y = pixel_y - dy
```

Horizontal codes therefore map to block offsets `0, +1, -1, +2, -2, …`.
A negative horizontal displacement selects a source to the right;
vertical displacement is nonnegative in this representation. The tests
used already decoded source blocks, including blocks to the upper right.

Representative complete block codes are:

| `(dx, dy)` | Complete mode-1 bits | Selection |
| --- | --- | --- |
| `(0, 16)` | `011` | Short form for the block above |
| `(0, 16)` | `0101010` | The same offset through two integer codes |
| `(16, 0)` | `0100101` | One block left |
| `(32, 0)` | `010001001` | Two blocks left |
| `(-16, 16)` | `010011010` | One block right and one row above |
| `(0, 32)` | `0101011` | Two block rows above |

No alignment follows these displacement fields. The next block starts
at the next bit; the existing pass-boundary logic aligns after the final
block of a pass.

## The encoder uses matching signed and unsigned integer writers

The native mode-1 branch writes its selector at `0x6f3d8`. When it is
zero, it calls the signed writer `0x8e29c` at `0x6f3f0` and unsigned
writer `0x8e1c4` at `0x6f400`.

For the tested values, the signed writer maps input `s` to positive code
`2*s` when `s > 0`, and `1 - 2*s` otherwise. The unsigned writer maps
input `u` to positive code `u + 1`. This agrees with the decoder's
horizontal and vertical mappings. The verified writer inputs span
−2048 through 2048 for selected signed values and 0 through 4096 for
selected unsigned values. Larger domains and overflow behavior remain
uncharacterized.

## Pixel callbacks copy 16 by 16 regions

The primary pixel callback is `0x5da68`. With packet byte B zero, it
subtracts the decoded offsets at `0x5daf0` through `0x5db10`. With
header flag B set, `0x5db30` selects the current frame as the source.
Calls at `0x5dc1c`, `0x5dc3c` and `0x5dc5c` copy its three primary
planes into the destination block.

The alpha callback is `0x5e188`. Its corresponding branch computes the
source offset at `0x5e25c` through `0x5e26c` and copies the current
frame's alpha plane at `0x5e2e8`.

Both use the function at decoder-context offset 1584. Constructor store
`0x5d774` resolves it through GOT entry `0xeee88` to `0x5fb74`. That
routine copies sixteen rows of sixteen bytes, using the supplied source
and destination strides. It runs as native ARM64 loads/stores in the
emulator; the pixel-copy operation was not replaced by host code.

Valid references across packet groups were exercised: the later group's
first block row could copy pixels retained from an earlier group. The
tested header configuration preserves that current-frame content while
the per-group working state is reset.

## Native origin checks differ between color and alpha

The primary callback checks source X and Y separately against zero and
the frame's width/height at `0x5daf8` through `0x5db20`. Selected invalid
origins return `-1999`, through `0x5dbec` or `0x5dc64`.

The alpha callback instead checks the computed linear source offset
against zero and `height * stride` at `0x5e270` through `0x5e280`.
Failure returns `-1999` at `0x5e294`. That is not an identical coordinate
check, nor a general validation of the complete copied rectangle.

The independent scratch decoder applies source-coordinate checks in
both passes. Positive cases and selected failure cases agree with the
native decoder; arbitrary malformed-input acceptance has not been matched.
Copies from future, overlapping or uninitialized regions were not tested.

## Validation

The APK identity, ELF instruction bytes, two displacement constants,
leading-zero table and block-copy dispatch binding were checked.

- All 69 copy blocks in seven native-generated samples matched the
  independent offsets, bit consumption and source pixels: 54 mode-0
  blocks and 15 mode-1 upward copies.
- 84 isolated native field-reader cases verified generic displacements
  with X offsets −48 through 48 in steps of 16 and Y offsets 0 through
  48, at starting bit offsets 0, 1 and 7. They stopped before auxiliary
  marker updates and did not attempt pixel copies.
- 328 native positive-code reads covered values 1–32 and selected
  boundaries through 65537 at all eight starting bit positions.
- 224 signed/unsigned native writer cases matched independently produced
  bits, also spanning all eight starting positions.
- 54 complete independently assembled images combined modes 0, 1 and 5
  at sizes 33×33, 65×49 and 79×65. Groups contained one, two or three
  block rows. Both forms of the mode-1 upward offset, longer left/up
  offsets, upper-right copies, alpha and clipped edges were exercised.
  Every pixel matched in the independent decoder and Samsung's decoder.
- Five images with invalid copy origins were rejected by both decoders;
  the native result was `-1999`. They covered mode 0 at the first block,
  mode-1 sources left/above/right of the image, and an alpha copy above
  the first block.

The 54 complete images exercised 1944 block operations and 3024 distinct
native instructions. Isolated field checks executed 260 native
instructions, and writer checks executed 186, without imported host calls.
These are synthetic compatibility checks, not device-file validation.

The independent encoder/decoder, emulator harness and generated artifacts
remain disposable local tooling. Maintained conclusions are Markdown-only.
No SDK code changed.

## Remaining work

Modes 2, 3 and 4 still need complete independent payload reconstruction.
The [alpha residual trace](spi-alpha-residual-findings.md) specifies one
mode-3 coefficient representation, with prediction and final pixels still
unresolved. Other packet-byte and header-flag combinations, reference
selection, auxiliary marker state, integer limits and malformed copy
regions also remain open.
The current scratch decoder supports only the specified combination of
literal blocks and copies within a frame. General SPI compatibility still
requires device-exported files and rendered references.
