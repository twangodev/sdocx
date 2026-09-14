# SPI temporal decoding with reduced secondary residuals

## Evidence and scope

Recovered from Samsung Notes 4.4.45.37 ARM64 `libSPenBase.so` in the
[identified APK](README.md#sources-and-validation). This extends the
[temporal reference decoder](spi-temporal-block-findings.md) to primary
mode 3, submode 2 with header flag D set. The tested header has wire
color index 4, flags `0xb0` and one reference-cache entry; temporal packets
have byte B equal to one. API output uses color value 500.

The independent decoder consumes constructed serialized sequences.
Native instructions execute unchanged and supply comparison results,
including intermediate coefficients and complete output pixels. These
sequences are not device exports. Maintained changes are Markdown-only;
SDK integration remains separate work.

## One mask covers primary and secondary residuals

The submode and quantizer selection, predicted motion, signed deltas,
source-coordinate clamps and reference-frame rotation follow the
[full-size temporal path](spi-temporal-block-findings.md). Header flag D
selects the reduced branch at `0x691dc`. Its three calls are:

| Call site | Target | Operation |
| --- | --- | --- |
| `0x69294` | `0x6adbc` | Read the bundled coefficient payload |
| `0x692a0` | `0x6a55c` | Dequantize and inverse-transform coded arrays |
| `0x692b0` | `0x6c70c` | Reconstruct three byte planes from the reference |

Reader `0x6adbc` first calls `0x6ae90` for primary channel zero. Read
one split bit: zero selects one 16-by-16 primary partition; one selects
four 8-by-8 primary partitions in raster order. Then read the
[temporal partition mask](spi-temporal-residual-findings.md), using the
eight-entry unary table for side 16 or the Q-dependent 64-entry table
for side 8.

For P primary partitions, primary partition p uses mask bit `P+1-p`.
Read all coded primary arrays, then secondary plane 1 if mask bit 1 is
set, then secondary plane 2 if mask bit 0 is set. There are no additional
split, mask or prediction-mode fields for the secondary planes.

Both secondary arrays are always 8 by 8. Calls `0x6ae34` and `0x6ae60`
invoke coefficient reader `0x6bb5c` with log-side argument 3. They use the
same [temporal tokens and fixed zigzag scan](spi-temporal-residual-findings.md)
as the primary arrays, including at Q zero. The scan selection at
`0x6addc`–`0x6ae04` subtracts one from primary log-side for an unsplit
block, yielding log-side 3 in either case.

Worker byte 29 holds primary side 16 or 8. Byte 30 is 8 for an unsplit
block and 16 for a split block; it is not the secondary coefficient-array
dimension in this submode. Primary coefficients begin at worker byte
`320 + 128*p`, with p zero for an unsplit block. Secondary arrays begin
at bytes 832 and 1344. Flags occupy bytes 2368–2371 for primary partitions
and 2372/2373 for the secondary planes.

## Every coded array is transformed even at Q zero

Primary arrays use selected Q; secondary arrays use the
[52-entry secondary quantizer mapping](spi-reduced-color-findings.md#secondary-quantization-has-its-own-mapping).
Mask selection continues to use the primary Q.

The call at `0x692a0` is unconditional. Consequently, reduced temporal
Q-zero blocks still use dequantization and inverse transforms. They also
broadcast transformed DC values, unlike the full-size temporal Q-zero
path that combines individual untransformed coefficients.

The wrappers select a second bank of side-4/8 scale tables when worker
submode byte 49 equals two. Primary side-8 selection occurs at
`0x6acf8`–`0x6ad04`; secondary selection occurs at
`0x6a610`–`0x6a61c` and `0x6a6d0`–`0x6a6dc`. In this ELF, the
temporal banks are byte-identical to the earlier intra banks:

| Side | Relocation | Intra bank | Temporal bank | Bytes per bank |
| --- | --- | --- | --- | --- |
| 4 | `0xeee40` | `0x29448` | `0x29508` | 192 |
| 8 | `0xeee48` | `0x295c8` | `0x298c8` | 768 |

Side 4 is included here to identify the shared wrapper's second bank;
the reduced temporal payload uses only sides 8 and 16. Primary side 16
uses the existing scalar table through `0x6ad78`–`0x6ad94`.
The previously recovered
[integer scaling and inverse arithmetic](spi-quantized-color-findings.md)
therefore applies without new constants.

## Secondary flags control whether reference pixels are filtered

Reconstruction `0x6c70c` first copies a motion-selected 16-by-16 reference
region for each channel into prediction buffers at worker pointers
8448, 8456 and 8464. Primary partitions combine their transformed
residuals with this full-size reference using context callback 1496,
`0x618a0`, called at `0x6c840`. Primary output remains 16 by 16.

Each secondary flag selects a different reconstruction path:

| Flag | Secondary reconstruction |
| --- | --- |
| 0 | Copy all 256 reference bytes unchanged |
| 1 | Add the transformed DC value to each of the 256 reference bytes |
| 7 | Reduce the reference to 8 by 8, add the 8-by-8 residual, then expand to 16 by 16 |

Plane-1 dispatch is at `0x6c854`–`0x6c864`; plane 2 dispatch is at
`0x6c984`–`0x6c994`. The flag-1 calls to `0x618a0` occur at
`0x6c88c` and `0x6ca30`, with side and strides all 16. Thus a constant
secondary correction preserves the reference's full spatial detail.

For flag 7, calls `0x6c940` and `0x6ca48` reduce the prediction in place
through context callback 1520. Calls `0x6c968` and `0x6ca70` combine the
64 residuals at side and strides 8. Calls `0x6c980` and `0x6ca9c` expand
the result in place through callback 1512, `0x600a8`, using the existing
[asymmetric upsampling filter](spi-differential-block-findings.md#reduced-planes-expand-with-asymmetric-interpolation).

An all-zero general residual still takes this reduction/expansion path.
It can therefore change the reference pixels even though its numerical
residual is zero. An uncoded residual preserves them exactly.

Both coded cases retain the earlier
[signed-16 addition and native byte conversion](spi-reduced-color-findings.md#byte-combination-broadcasts-dc-and-preserves-native-wrapping).
No signed forward/inverse color conversion occurs. All three final
planes are 16-by-16 byte buffers at worker pointers 8416, 8424 and 8432;
the shared temporal output path copies them into the current frame.

## Reference reduction rounds horizontally before averaging rows

Constructor store `0x5d724` binds callback 1520 through relocation
`0xeeea8` to `0x60218`. It calls horizontal filter `0x60278` into a
stack temporary, then vertical filter `0x60398` into the destination.
The temporary makes identical source and destination pointers safe.

Let R be the 16-by-16 reference block, H the 16-by-8 horizontal result,
and D the 8-by-8 reduced prediction:

```text
H[y,0] = (3*R[y,0] + R[y,1] + 2) >> 2
H[y,x] = (R[y,2*x-1] + 2*R[y,2*x] + R[y,2*x+1] + 2) >> 2
    for x = 1..7

D[y,x] = (H[2*y,x] + H[2*y+1,x] + 1) >> 1
    for y = 0..7, x = 0..7
```

The first stage rounds before the second stage. Combining these into
one weighted sum with one final rounding would change some outputs.
The helper hardcodes packed input stride 16 and output stride 8;
register arguments X2/X3 do not provide general stride support here.
The last horizontal sample uses input columns 13, 14 and 15. Vertical
reduction pairs rows 0/1 through 14/15 without an extra edge extension.

## Validation and remaining work

Sequence comparisons cover consumed bits, motion availability and vectors,
raw coefficient arrays and flags, primary split/mask fields, every coded
transformed array, full reconstructed blocks, stored frame planes and
visible API pixels. Initial frames use literal primary blocks and intra
alpha blocks. Later frames mix primary literal and temporal submode-0/2
blocks with temporal alpha submode-0/2 blocks.

All 772 corpus sequences match native output for all 1976 frames:

- 504 two-frame sequences cover every side-8 and side-16 mask at
  Q values 0, 1, 22, 27, 32, 37 and 51, exercising every mask bank.
- 216 four-frame sequences cover dimensions 1 by 33, 17 by 33 and
  65 by 65; groups of one, two or three block rows; four variants;
  and Q values 0, 21, 30, 36, 50 and 51.
- 52 two-frame sequences exercise every primary Q and its secondary
  mapping with all six primary/secondary partitions coded.

They contain 4966 reduced primary submode-2 blocks, 1476 primary
submode-0 blocks, 6064 alpha submode-2 blocks and 1836 alpha submode-0
blocks. Across these payloads, 22569 partitions are coded: 7593 with
flag 1 and 14976 with flag 7, plus 21265 uncoded partitions. The corpus
executes 7056 distinct native instructions.

The isolated filter comparisons cover 465 input patterns: six constants,
all 256 single-pixel impulses, two directional ramps, a checkerboard and
200 deterministic random blocks. Each runs with separate and identical
source/destination pointers, for 930 native comparisons. Destination
guards and unused tails remain intact, and separate source buffers are
unchanged. These comparisons execute 151 distinct native instructions
without imported functions. Both scale-bank pairs also match byte for
byte in the ELF.

Targeted two-frame sequences cover all nine combinations of secondary
flags 0/1/7, both primary sides and Q values 0, 30 and 51: 54 sequences
and 108 frames. Flag 1 uses a DC coefficient of 600; flag 7 uses a
non-DC token whose magnitude is zero. These distinguish reference copying,
full-size DC correction and filtering without a numerical residual.

Eight four-frame repeats with fresh allocation fills `0xa5` and `0xff`
also match all intermediate and output comparisons. Four prior full-size
temporal sequences retain their recorded output digests after extending
the disposable decoder.

All 826 stored sequence digests and 2084 independently reconstructed frame
digests, including the targeted flag cases, were checked again after
generation. The representative Q-30, 65-by-65, two-block-row-group,
variant-1 sequence has SHA-256
`fcea7475e47d34c8bcd6230d00466148701490c7c706316beb2a50a88f50e0a4`;
its fourth frame's output has SHA-256
`ed0b8664b79f7ec2fae167d9e70ed7d983ce7718036203fcd16c22e85b7e3ce3`.
The disposable storage prefixes the header and each image-data block with
a little-endian length; it does not assert a multi-frame SDOCX wrapper
layout. APK/ELF identity, forty cited instruction words, callback/table
relocations and local documentation links were verified.

Primary submode 3, multi-reference selection, mixed temporal/intra edge
fallback, broader malformed-input handling and device-export compatibility
remain open. This result does not establish every packet-B-one path or
the SDOCX wrapper's use of multi-frame SPI data.
