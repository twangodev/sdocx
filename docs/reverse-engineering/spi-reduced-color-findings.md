# SPI mode-3 color decoding with reduced secondary planes

## Evidence and scope

Recovered from Samsung Notes 4.4.45.37 ARM64 `libSPenBase.so` in the
[identified APK](README.md#sources-and-validation). This extends the
[full-size color findings](spi-color-intra-findings.md) and
[quantized transforms](spi-quantized-color-findings.md) to primary mode 3,
submode 1 with one 16-by-16 byte plane and two 8-by-8 byte planes.

The tested configuration uses wire color index 4, header flags `0xf0`,
packet byte B zero and API output color value 500. The selected quantizer
is bounded to 0–51. Independently constructed streams exercise the native
decoder; these are not device exports or samples emitted by the native
encoder. Other submodes and configurations remain outside this result.
Maintained changes are Markdown-only; no SDK code changed.

## Header bit 4 selects the reduced path

The header reader reads flag D at `0x67ebc` and stores it at `0x67ec0`
in field-structure byte 17, decoder-context byte 113. Thus changing the
last header byte from `0xe0` to `0xf0` sets this flag.

Primary mode-3 entry `0x68470` selects submode 1 when packet byte B is
zero. It loads context byte 113 at `0x68528` and stores worker byte 2493
at `0x68530`. The following bit selects Q from the packet quantizers:
zero selects C; one selects `min(C, D)`. Helper `0x6d438` stores Q at
worker bytes 56 and 58 and its secondary-plane mapping at byte 59.

The reduced path branches around the signed forward color conversion
after edge gathering. Payload dispatch `0x68d58` reaches `0x692b8`;
the reduced branch begins at `0x692c4`. The corresponding full-size
branch at `0x6969c` reads separate payloads for three signed planes.

The reduced result instead consists of three byte planes. For API color
value 500, planes 0, 1 and 2 still supply output byte positions 1, 2 and 0.
Alpha retains its separate pass.

## Secondary edges use different horizontal and vertical filters

The existing [frame-neighbor rules](spi-alpha-state-findings.md) gather
33-byte left and above arrays for each plane, including the corner at
index zero. Above arrays start at worker offset `9568 + 33*channel`;
left arrays start at `9700 + 33*channel`.

The reduced branch filters entries 1–32 of each secondary array in place.
Calls `0x68638` and `0x68658` use context callback 1416, relocation
`0xeee78`, resolving to above filter `0x5e74c`. Calls `0x68648` and
`0x68668` use callback 1424, relocation `0xeee80`, resolving to left
filter `0x5e7a8`.

Let a and l denote the original above and left samples at indices 1–32,
renumbered 0–31. Their sixteen filtered samples are:

```text
above_filtered[0] = (3*a[0] + a[1] + 2) >> 2
above_filtered[i] = (a[2*i-1] + 2*a[2*i] + a[2*i+1] + 2) >> 2
    for i = 1..15

left_filtered[i] = (l[2*i] + l[2*i+1] + 1) >> 1
    for i = 0..15
```

These overwrite entries 1–16. The corner at zero and entries 17–32
retain their original values. Plane 0 keeps its original byte edges.
This branch does not invoke the full-size path's `0x5ea48` forward
color conversion.

## One subdivision field covers all three planes

Read one divided bit at `0x692c4`. Zero selects one section with a
16-by-16 primary partition and two 8-by-8 secondary partitions. One
selects four sections in top-left, top-right, bottom-left, bottom-right
order, each covering an 8-by-8 primary region and two 4-by-4 secondary
regions.

For each section, native code calls field reader `0x6a340` at
`0x693a4`, dequantization/transform wrapper `0x6a55c` at `0x693b4`,
and byte reconstruction `0x6c278` at `0x693c4`.

The field sequence for a section is:

1. If divided, read a split bit. A split section has four 4-by-4 primary
   partitions; an unsplit section has one 8-by-8 primary partition.
   This bit does not subdivide either secondary partition.
2. Read one primary prediction mode per primary partition, using the
   existing neighbor-marker syntax. Update only the plane-0 marker grid.
3. Read the shared secondary prediction field described below.
4. Read one partition mask using the primary partition side and Q.
5. Read coded primary partitions in partition order, then coded secondary
   plane 1 and coded secondary plane 2.

Worker byte 29 stores the primary partition side. Byte 30 stores the
secondary side: 8 for an undivided block or 4 for a divided block,
independent of the section's split bit.

Primary prediction uses the earlier [marker-state rules](spi-alpha-state-findings.md)
and [prediction-field syntax](spi-alpha-payload-findings.md), including
immediate updates for each partition. Secondary planes have no independent
prediction-marker fields in this payload.

### Secondary prediction can inherit the first primary mode

At `0x6a3f4`, read one bit. Zero stores sentinel 4 in worker byte 55.
One reads two more bits at `0x6a404`: values 0, 1 and 2 select those
prediction modes; value 3 selects planar mode 17.

During reconstruction, `0x6c4d0`–`0x6c4dc` replaces sentinel 4 with
the section's first primary prediction mode from worker byte 51. This
one resolved mode applies to both secondary partitions. Inheritance can
therefore select any of the eighteen primary modes, even in a split
section whose other primary partitions use different predictions.

### The mask includes both secondary coefficient arrays

Mask reader `0x6b704` is called at `0x6a428`. Side 4 uses the
[five Q-dependent mask tables](spi-quantized-color-findings.md#the-side-4-partition-mask-depends-on-q).
Sides 8 and 16 use the existing eight-entry unary table.

For P primary partitions, numbered p from zero:

| Mask bit | Coefficient array |
| --- | --- |
| `P + 1 - p` | Primary partition p |
| 1 | Secondary plane 1 |
| 0 | Secondary plane 2 |

Every coded array uses reader `0x6b8a8`, the
[quantized token representation and fixed zigzag scan](spi-quantized-color-findings.md#quantized-coefficients-use-a-fixed-zigzag-scan).
This applies even when Q is zero. A reduced block never switches to the
full-size zero-Q coefficient reader or its prediction-dependent scans.

Primary coefficient calls occur at `0x6a4a8`; secondary calls occur at
`0x6a508` and `0x6a534`. Primary flags occupy worker bytes 2368–2371;
the secondary flags occupy 2372 and 2373. As before, flag 0 means uncoded,
flag 1 means one token at scan index zero, and flag 7 is the general
coded case.

For section s and primary partition p, the signed-16 coefficient array
begins at worker byte `320 + 128*s + 2*p*primary_side*primary_side`.
Secondary channel c, numbered 1 or 2, begins at
`832 + 512*(c-1) + 64*s`. Consequently, divided secondary sections have
32 signed-16 slots between starts even though a section uses only 16.
This spacing belongs to native scratch storage, not the serialized stream.

## Secondary quantization has its own mapping

Helper `0x6d438` maps the selected Q through the 52-byte table at
`0x291d4`. The entries, indexed by Q, are:

```text
 0  1  2  3  4  5  6  7  8  9 10 11 12
13 14 15 16 17 18 19 20 21 22 23 24 25
26 27 28 29 29 30 31 32 32 33 34 34 35
35 36 36 37 37 37 38 38 38 39 39 39 39
```

Plane 0 uses Q. Both secondary planes use this mapped value. The
partition-mask table still uses Q, not the secondary value.

Wrapper `0x6a55c` invokes primary processing through `0x6abec` at
`0x6a580`. It loads the secondary quantizer from worker byte 59 at
`0x6a598` and `0x6a658`. It uses the same recovered
[scaling and inverse-transform arithmetic](spi-quantized-color-findings.md)
for secondary side 4 or 8. Side-4 scaling and inverse callbacks resolve
to `0x61224` and `0x60674`; side-8 callbacks resolve to `0x61520` and
`0x608f4`.

There is no Q-zero bypass here. A coded DC array still undergoes scaling
and the inverse DC operation before its single value is broadcast during
pixel combination.

## Reconstruction works directly in byte planes

Plane 0 is reconstructed into the 16-by-16 byte buffer at worker pointer
8416. Its partition edges and prediction follow the
[alpha pixel rules](spi-alpha-pixel-findings.md): side-4 helper `0x677b8`,
side-8 helper `0x67950`, directional/planar prediction, edge filtering
and DC boundary adjustment where selected. This includes the previously
recovered side-4 above-tail behavior at primary position (4, 4).

The reduced path does not apply the zero-Q mode-0/mode-1 residual
accumulation used by the earlier full-size path. Residuals have already
passed through dequantization and inverse transformation.

Secondary byte buffers are at worker pointers 8424 and 8432, with
stride 8. Their four section offsets are `(0, 4, 32, 36)`, from table
`0x2a7d0`, read at `0x6c4f8`. Side 8 copies the first seventeen external
edge bytes. Side 4 calls helper `0x67a44` at `0x6c530` and `0x6c61c`.

### Secondary side-4 edges use the already reconstructed 8-by-8 plane

Each edge has nine bytes: one corner and eight samples. Let L and A be
the filtered external arrays, including their corner at zero.

| Section position | Above edge | Left edge |
| --- | --- | --- |
| (0, 0) | `A[0..8]` | `L[0..8]` |
| (4, 0) | `A[4..12]` | Its above corner, four decoded left pixels, then four copies of the last pixel |
| (0, 4) | `L[4]`, then all eight decoded pixels in row 3 | `L[4..12]` |
| (4, 4) | Decoded pixel (3, 3), row-3 pixels 4–7, then four copies of pixel (7, 3) | The same corner, column-3 pixels 4–7, then four copies of pixel (3, 7) |

Here the displayed array ranges include both endpoints. The secondary
bottom-right partition repeats the last above sample; it does not use
the primary side-4 above-tail behavior.

Secondary prediction directly invokes planar callback `0x62f44` or
directional callback `0x628b8`. It does not run the primary edge filter
`0x6353c` or DC boundary adjustment `0x63780`. The resolved prediction
mode is shared, but each secondary plane has its own edges and pixels.

### Byte combination broadcasts DC and preserves native wrapping

All three planes use context callback 1496, relocation `0xeedc0`, which
resolves to `0x618a0`. Plane 0 calls it at `0x6c370`; secondary calls
occur at `0x6c5e4` and `0x6c6d0`.

Flag 0 copies prediction bytes. Flag 1 adds transformed coefficient zero
to every prediction byte. Flag 7 adds the corresponding transformed
coefficient. For either coded case, wrap the sum to signed 16 bits before
the native byte conversion:

```text
v = signed16(prediction + residual)
pixel = v if 0 <= v <= 255 else ((-v) >> 15) & 255
```

This normally clamps to 0–255, but wrapped value −32768 produces byte 1.
Isolated native checks exercise that boundary for all three partition
sides and both coded flags. The full-size quantized path instead uses
its separate signed-plane combination callback and bit-depth bounds.

## Output expands secondary planes without inverse color conversion

The submode-1 output branch at `0x5de30` sets block availability. With
sampling enabled, `0x5de78` copies the primary byte block into frame
plane 0. Calls `0x5de94` and `0x5deb0` expand secondary planes through
context callback 1512, resolving to `0x600a8`.

This is the same [asymmetric expansion recovered for mode 2](spi-differential-block-findings.md).
Vertical expansion preserves the first and last input rows. Between
adjacent rows a and b, insert `(3*a+b+2)>>2` and `(a+3*b+2)>>2`.
Horizontal expansion preserves input values at even columns, inserts
rounded pair averages at odd columns, and repeats the last value at
column 15. The resulting secondary planes are 16 by 16.

This branch skips signed inverse color conversion `0x5ed8c`. Complete
image output retains the established channel order and crops partial
edge blocks to the declared dimensions.

## Validation and remaining work

The independent reader derives fields, markers, edges and pixels from
header and packet bytes alone. Native intermediate state supplies only
comparison targets. Checks cover consumed bits, subdivision, primary and
secondary prediction modes, masks, flags, coefficient arrays, transformed
residuals, partition edges, prediction bytes, reconstructed byte buffers,
expanded planes and final output pixels.

All 828 constructed images match the native decoder. They cover:

- 560 single-block images: every mask code for primary sides 4, 8 and 16
  at Q values 0, 1, 22, 27, 32, 37 and 51, covering all five mask banks.
- 52 mapping images: every Q from 0 through 51 in 17-by-33 images with
  two block rows per packet.
- 216 mixed images: dimensions 1 by 33, 17 by 33 and 65 by 65; packet
  groups of one, two or three block rows; four variants; Q values 0, 21,
  30, 36, 50 and 51.

Together they contain 1821 reduced mode-3 blocks, 5361 sections and 12811
coded coefficient arrays. Primary section sides 4, 8 and 16 occur 3144,
1576 and 641 times. All eighteen prediction modes occur for both primary
and secondary planes. The corpus also contains 416 differential blocks,
398 palette blocks, 3320 alpha blocks and 1192 packets, with copy and
literal blocks interleaved. Native comparisons execute 8717 distinct
instructions. Every stored stream digest and independently decoded pixel
digest was checked again after generation.

The isolated primitive checks cover 200 edge-filter cases, 400 secondary
edge cases and 720 combination cases with independent source, prediction
and destination strides. Guard bytes remain intact. The 52 mapping bytes
match the ELF. These checks execute 475 distinct native instructions
without imported functions.

Eight full native repeats, using fresh allocation fills `0xa5` and `0xff`,
preserve state and pixels for Q-zero and Q-51 mask images and mixed images
across packet groups. All 1485 earlier corpus cases retain their recorded
pixels, as do the seven native-generated quality-sweep images.

One representative mixed stream is the 65-by-65, two-block-row-group,
Q-30 variant. Its SPI SHA-256 is
`af7e78bcaeeb531596404f8e400fae9f284bd8311b449b95466f3b39c752f410`;
its output-pixel SHA-256 is
`bc173ef94734cf9190c85886614bb72e3792a22d0aae8d82228e80a2e8e46c57`.
The APK, extracted ELF, cited instruction words, callback bindings and
numeric tables were checked against the binary. Scratch scripts and
generated streams remain disposable local artifacts.

The [temporal trace](spi-temporal-block-findings.md) adds submodes 0 and 2
with full-size reference planes. Reduced temporal submode 2, primary
submode 3, multi-reference selection, alpha literal marker
behavior, broader malformed-input handling and SDK integration remain
open. Device-exported SPI data and rendered references are still needed
to establish compatibility with real documents.
