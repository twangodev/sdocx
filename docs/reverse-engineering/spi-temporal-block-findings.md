# SPI temporal mode-3 blocks and reference images

## Evidence and scope

Recovered from Samsung Notes 4.4.45.37 ARM64 `libSPenBase.so` in the
[identified APK](README.md#sources-and-validation). Primary and alpha
mode 3 can copy from a previous image or add residuals to a motion-shifted
reference block. An independent scratch decoder now reconstructs those
paths across complete constructed image sequences.

The tested configuration uses wire color index 4, API output color 500,
header flags `0xa0`, one additional reference buffer, and packet byte B
zero for the initial image and one for subsequent images. Primary blocks
combine literals with mode-3 submodes 0 and 2. Alpha starts with the
earlier intra submode 1 and then uses submodes 0 and 2. Primary submode 2
uses full-size color planes and selected Q values from 0–51.

These are constructed codec sequences. They were not emitted by the
native encoder or exported from a device. The result does not establish
that ordinary single-image SDOCX caches use temporal coding. SDK support,
other configurations and device-export compatibility remain open.

## Header bit 6 controls reference retention

Header flag B, bit 6 of the last header byte, becomes context byte 111.
The earlier `0xe0` configuration sets it; `0xa0` clears it. Header setup
allocates two internal image descriptors at context pointers 992 and
1000 through `0x5ccf4`–`0x5cd2c`, even when this flag is set.

The complete-image branch at `0x5cb80` first checks that all packet
groups have been consumed. With flag B clear, `0x5cba4`–`0x5cbb0`
rotates the descriptors:

```text
completed = context[992]
previous = context[1000]
context[984] = completed
context[992] = previous
context[1000] = completed
```

Context 984 supplies the exposed output image; context 992 becomes the
destination for the next image; context 1000 supplies the completed image
as a reference. The stores partly use a pointer into the context at offset
916, so the 128-bit store at `0x5cbb0` writes offsets 984 and 992.

With flag B set, `0x5cba0` bypasses this rotation. A three-frame native
comparison verified alternating descriptor pointers with flags `0xa0`
and unchanged pointers with `0xe0`. A separate two-frame test showed
submode 0 reproducing the first image's color and alpha when retention
was enabled. With `0xe0`, the same reference-copy payload instead read
the separately initialized 128-valued reference image.

The tested retained configuration also sets header byte 18 to one.
When flag B is clear, setup at `0x5cf24` allocates that additional image,
and `0x60478` initializes per-block reference-index arrays. The output
paths update that cache as well as the destination image. The count-one
case avoids selection among multiple cached references; the complete
reference-cache selection policy remains open.

## Packet byte B exposes the submode field

Primary entry `0x68470` compares worker byte 8, packet byte B, with one
at `0x684bc`. If equal, `0x684d4` reads two bits and stores submode byte
49 at `0x684e0`. Otherwise it selects submode 1 without these bits.

Alpha entry `0x68824` performs the same comparison at `0x68860`, reads
the two bits at `0x68874` when B is one, and otherwise uses submode 1.
Alpha then sets Q to zero through `0x68884`.

Dispatcher `0x68d58` uses separate four-entry tables for primary and
alpha passes:

| Submode | Primary target and role | Alpha target and role |
| --- | --- | --- |
| 0 | `0x68e9c` → `0x6a204`: predicted-motion reference copy | `0x68dd0`: predicted-motion reference copy |
| 1 | `0x692b8`: intra reconstruction | `0x68eac`: intra reconstruction |
| 2 | `0x69124`: motion deltas and residuals | `0x68f7c`: motion deltas and residuals |
| 3 | `0x693d8`: selected-plane update | `0x69a2c`: returns the dispatch error |

The tables are at `0x2a950` and `0x2a948`, respectively. The separate
[selected-plane trace](spi-selected-plane-findings.md) validates primary
submode 3. Its entry selects `min(C,D)`, forces sampling off and reads
a two-bit plane selector. The dispatcher then subtracts signed packet
byte E and floors Q at zero; the linked findings recover its distinct
Q-zero payload and flag behavior.

Primary submode 0 selects Q = C without a further quantizer-choice bit.
Submode 2 reads one bit: zero selects C; one selects `min(C,D)`. For
submode 2, header flag D still selects full-size versus reduced secondary
planes. This document covers flag D zero; the
[reduced temporal trace](spi-reduced-temporal-findings.md) covers flag D
one. Neither
submode 0 nor submode 2 reads spatial prediction-mode fields.

## Motion prediction uses neighboring block vectors

Both passes call `0x5e52c`. Native vectors are pairs of signed 32-bit
pixel offsets. The previous-row array begins at worker offset 4308;
the current-row array begins at 6356. For block column c, the vectors
used by the predictor are:

| Name | Native byte offset |
| --- | --- |
| L, left | `6356 + 8*c` |
| A, above | `4308 + 8*(c+1)` |
| R, above-right | `4308 + 8*(c+2)` |
| D, above-left | `4308 + 8*c` |

The predicted vector is stored at `6356 + 8*(c+1)`. Row-finish helpers
`0x6d454` and `0x6d51c` copy 2048 bytes from the current array to the
previous array and clear the current array. The tested images require
at most five block columns; larger array-capacity limits remain unvalidated.

The helper receives a sixteen-bit neighbor mask built by `0x6d690`
for primary blocks and `0x6e0b4` for alpha. Bits 0, 4, 8 and 12 encode
the flags for L, A, R and D. In the tested streams, a literal or intra
neighbor has flag one; a temporal submode-0/2 neighbor has flag zero.
This is separate from the prediction-mode marker grids.

On the first row of a packet group, the three upper flags are one and
the left flag comes from the already decoded left block. Position
(column 0, row 0) is forced to `0x1111`. On later rows, all four flags
come from the current and previous availability rows. Padded availability
entries start at zero.

Define `closest(X,Y,Z)` independently for each vector component: choose
Y when `abs(Z-X) > abs(Z-Y)`; otherwise choose X. Ties choose X. The
native `sabd` and unsigned vector comparison preserve the full distance
between signed-32 endpoints, including distances above `INT32_MAX`.

| Neighbor mask | Predicted vector |
| --- | --- |
| `0x0000`, `0x1000` | `closest(L,A,R)` |
| `0x0001` | `closest(A,R,D)` |
| `0x0010` | `closest(L,R,D)` |
| `0x0100` | `closest(L,A,D)` |
| `0x0011`, `0x1011` | R |
| `0x0101`, `0x1001`, `0x1101` | A |
| `0x0110`, `0x1010`, `0x1100`, `0x1110` | L |
| `0x0111` | D |
| `0x1111` | (0, 0) |

This componentwise selection need not choose one complete input vector.
The lookup tables at `0x2a3f3`, `0x2a405`, `0x2a417` and `0x2a429`
select the native branches. Other small values in the gaps between valid
masks return without writing the output; the independent reader rejects
such masks.

## Submode 0 copies a block without a residual payload

Primary submode 0 calls the predictor at `0x6a254`, adds its vector
to the block's pixel position and clamps each source coordinate:

```text
source_x = clamp(signed32(block_x + dx), -16, ceil(width/16)*16)
source_y = clamp(signed32(block_y + dy), -16, ceil(height/16)*16)
```

Worker halfwords 2488 and 2490 hold those aligned upper limits, assigned
at `0x5c460` and `0x5c464`. Source positions can enter the allocated
16-pixel border. The three 16-by-16 byte planes are copied from context
1000 to worker pointers 8416, 8424 and 8432 at `0x6a2c0`, `0x6a2e0`
and `0x6a300`.

Alpha submode 0 calls the same predictor at `0x68e08`, applies the same
coordinate limits, and copies the reference alpha plane to worker pointer
8440 at `0x68e70`. No motion-delta or residual bits follow either pass's
submode-zero field.

The frame allocations and their borders initially contain byte 128.
Decoded blocks also populate padded rows and columns beyond the declared
visible dimensions. Temporal copies therefore use padded decoded pixels
where present and the initialized outer border elsewhere. Output is
cropped to the declared width and height.

## Submode 2 adds signed pixel deltas and plane residuals

After prediction, read two positive integers H and V using the existing
leading-zero code. Convert each to a signed delta:

```text
delta(n) = -(n >> 1) if n is odd else n >> 1
```

Thus codes 1, 2, 3, 4 and 5 mean 0, +1, −1, +2 and −2 pixels. There
is no sixteen-pixel multiplier. Add each delta to its predicted component
with signed-32 wrapping. Primary reads occur at `0x69160` and `0x69198`;
the result is stored at worker offsets 8404/8408 and in the current
motion row through `0x691d8`. Alpha reads occur at `0x68fc4` and
`0x68ffc` and update the motion-row pair directly, rather than those
primary worker fields.

Both paths clamp the resulting source coordinates as for submode 0.
Primary full-size reconstruction reads three independent plane payloads
through `0x6ae90`. Each has one split bit, one mask and coefficient
streams for its coded partitions. Alpha reads the equivalent single-plane
syntax in its dispatcher branch. The
[temporal residual findings](spi-temporal-residual-findings.md) specify
the distinct side-8 mask banks and side-specific compact token tables.

For primary Q nonzero, calls `0x69508`, `0x695b4` and `0x69678` use
the already recovered [dequantization and inverse transforms](spi-quantized-color-findings.md).
All three full-size planes use Q directly. Q zero bypasses these steps;
alpha always uses Q zero in this entry path.

Reconstruction `0x6caa0` copies the motion-selected 16-by-16 reference
plane into a prediction buffer and combines each partition with it.
Its selection at `0x6cb98`–`0x6cbd4` uses context callback 1496 for
Q nonzero and callback 1504 for Q zero:

- Q nonzero uses `0x618a0`: a flag-1 transformed coefficient is broadcast
  across the partition; flag 7 supplies corresponding transformed values.
- Q zero uses `0x61db0`: coefficients remain spatial residual samples,
  including a flag-1 array whose only nonzero sample is at index zero.
  It does not broadcast that value or accumulate residuals by direction.
- Flag 0 copies the reference prediction without residual changes.

Both coded byte paths add with signed-16 wrapping and use the earlier
[native byte conversion](spi-reduced-color-findings.md#byte-combination-broadcasts-dc-and-preserves-native-wrapping),
including the −32768-to-1 boundary. These are byte planes throughout;
the full-size intra path's signed forward/inverse color conversion is
not part of temporal submode 2.

## Validation and remaining work

Native sequences are opened once through constructor `0x5d528` and
header consumer `0x5da00`. Each complete image's packet groups are then
fed through `0x5da00` on that same decoder, with output obtained through
`0x5da34`. No native codec instructions, frame buffers or motion state
are patched to supply reference pixels. Host replacements remain limited
to the imported memory operations listed in the
[codec validation](spi-codec-validation.md).

The independent sequence reader derives its reference images, motion
state, payload fields and pixels from serialized bytes. Native checks
then compare neighbor masks, predicted vectors, primary final vectors,
coefficient flags and arrays, primary transformed residuals and partition
fields, consumed bits, reconstructed byte blocks, frame planes and all
visible output bytes. Native values are comparison targets, not inputs
to independent reconstruction.

All 720 constructed sequences match native output for all 1872 frames:

- 504 two-frame sequences cover all 64 side-8 masks and eight side-16
  masks at Q values 0, 1, 22, 27, 32, 37 and 51.
- 216 four-frame sequences cover image sizes 1 by 33, 17 by 33 and
  65 by 65; groups of one, two or three block rows; four variants;
  and Q values 0, 21, 30, 36, 50 and 51.

They contain 4914 primary submode-2 blocks, 1476 primary submode-0
blocks, 6012 alpha submode-2 blocks and 1836 alpha submode-0 blocks.
The residual payloads contain 31131 coded partitions: 10442 with flag 1
and 20689 with flag 7, plus 28101 uncoded partitions. The complete
comparisons execute 6641 distinct native instructions. Motion deltas
include zero, single-pixel shifts, larger positive and negative shifts,
and values large enough to exercise both source-coordinate clamps.

The isolated motion tests cover 8320 cases across all sixteen valid
neighbor masks, including signed-32 endpoints, ties and eight block-column
positions. Another 56 gap-mask cases preserve the output guard bytes
without writing a vector. These tests execute 136 native instructions
without imported functions. The residual reader has a further 11040
isolated comparisons described in the linked findings.

Eight native repeats with fresh allocation fills `0xa5` and `0xff`
preserve intermediate state and output for Q-zero/Q-51 mask sequences
and mixed sequences across packet groups. All 720 artifact digests and
1872 independently reconstructed frame digests were checked again after
generation.

The representative Q-30, 65-by-65, two-block-row-group sequence has
artifact SHA-256
`358f238f7f4d1b9d0cfe1a839a875e28c94436bb4980f6a9615b04ddcbb4615b`.
Its fourth frame's output SHA-256 is
`327032d42437aef0dc3356582d6e67379bdfb5df771e552cd2a2541dcc1ad208`.
The disposable artifact stores a little-endian length before the header
and each separately submitted image-data block. This is test storage,
not a claim that the SDOCX bitmap wrapper emits that multi-frame layout.
APK/ELF identity, cited instruction words, dispatch/callback bindings and
numeric tables were verified against the binary.

[Reduced temporal submode 2](spi-reduced-temporal-findings.md) is covered
separately, as is [primary submode 3](spi-selected-plane-findings.md).
Multi-reference selection,
mixed temporal/intra edge fallback, alpha literal marker behavior and
broader malformed-input limits remain open. In particular, successful
submode-0/2 sequences do not establish all packet-B-one combinations.
Device exports and rendered references remain necessary for compatibility
validation. Maintained changes are Markdown-only; generated sequences
and scratch decoders remain disposable.
