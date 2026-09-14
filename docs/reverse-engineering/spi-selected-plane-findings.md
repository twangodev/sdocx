# SPI temporal updates to one selected color plane

## Evidence and scope

Recovered from Samsung Notes 4.4.45.37 ARM64 `libSPenBase.so` in the
[identified APK](README.md#sources-and-validation). Primary mode 3,
submode 3 reconstructs one color plane at the current block coordinates.
It obtains the other two color planes from the previous completed image.

This extends the [temporal sequence decoder](spi-temporal-block-findings.md)
using independently constructed serialized inputs. Tests use wire color
index 4, API output color 500, header flags `0xa0` or `0xb0`, one cache
entry, and packet byte B one after the initial image. Valid-plane sequence
reconstruction is bounded to selectors 0–2 and adjusted Q values 0–51.
Selector 3 and out-of-range adjusted Q are investigated separately below.
These are native codec comparisons, not device exports or SDK support.

## Entry selects one plane and adjusts the shared quantizer

When packet byte B equals one, primary entry `0x68470` reads a two-bit
submode at `0x684d4`. For submode 3, it:

1. Clears sampling byte 2493 at `0x684f8`, regardless of header flag D.
2. Selects `min(C,D)` through `0x68500`–`0x68504`.
3. Reads a two-bit plane selector at `0x68510` and stores it in worker
   byte 50 at `0x68514`.

There is no quantizer-choice bit, motion-vector predictor call, signed
motion delta or spatial prediction-mode field in this payload.
Dispatch `0x693d8` then performs:

```text
Q = max(0, min(C,D) - signed8(E))
```

The signed load of E is at `0x693e0`; the subtraction and lower clamp
are at `0x693e4`–`0x693ec`. Call `0x693f0` stores the resulting Q through
`0x6d438`. Every selected color plane uses Q directly. Selecting plane 1
or 2 does not switch to the secondary-plane mapping used by reduced
submode 2.

For API output color 500:

| Selector | Native plane | Output byte position |
| --- | --- | --- |
| 0 | Descriptor pointer 32 | 1 |
| 1 | Descriptor pointer 40 | 2 |
| 2 | Descriptor pointer 48 | 0 |

The alpha pass remains separate. Its own submode 3 reaches the dispatch
error identified in the earlier temporal findings.

### The adjustment has no upper clamp

Isolated execution from `0x693d8` to immediately before the call at
`0x693f0` covers all 52 base Q values and all 256 signed byte values of
E: 13312 cases. All match the equation above. Of these, 5330 produce
Q greater than 51; the largest is 179 from base Q 51 and E −128.

The next helper uses Q as an index into its 52-byte mapping table at
`0x291d4`. These probes stop before that lookup. They establish the
arithmetic and absence of an upper clamp in this step, not successful
decoding or a complete malformed-input policy for larger values.

## Reference prediction uses the same block coordinates

At `0x693f4`–`0x69410`, the dispatcher loads the previous completed
image from context pointer 1000 and computes `y*stride+x`. It copies
one 16-by-16 reference region into the selected prediction buffer:

| Selector | Reference selection | Prediction pointer |
| --- | --- | --- |
| 0 | `0x69424`–`0x69434` | Worker 8448 |
| 1 | `0x69b74`–`0x69b88` | Worker 8456 |
| 2 | `0x69b64`–`0x69b88` | Worker 8464 |

These are full byte planes even when header flag D is set. No color
conversion, reference downsampling or upsampling occurs. The selected
output buffer is worker pointer `8416 + 8*selector`.

In the tested sequences, selected-plane blocks have availability byte
zero and retain a zero current-row motion-vector slot. Later motion-coded
blocks therefore see this position through the existing temporal neighbor
rules. Comparisons include selected-plane blocks interleaved with literal
and submode-0/2 blocks across multiple block rows and packet groups.

## Nonzero Q uses ordinary temporal partitions

The Q-nonzero branch begins at `0x69b98`. Read one split bit at
`0x69bb8`: zero gives one 16-by-16 partition, and one gives four
8-by-8 partitions. Side-8 masks use the Q-dependent temporal table;
side-16 masks use the eight-entry unary table. Read only partitions
selected by mask bits `P+1-p`, for partition count P and index p.
Mask bits 1 and 0 introduce no additional arrays.

The payload uses the existing
[temporal coefficient tokens and fixed zigzag scan](spi-temporal-residual-findings.md).
Reader calls are `0x69e88`, `0x69ef8`, `0x69f68` and `0x6a038` for
the four side-8 partitions, or `0x69fd4` for side 16. Arrays start at
worker byte `320 + 512*selector + 128*p`. The side-8 reader's native
offset arguments are 0, 64, 128 and 192.

Each coded array uses Q for the recovered
[dequantization and inverse transform](spi-quantized-color-findings.md).
The side-8 scale pointer includes the temporal-bank offset 768 at
`0x69e44`; that bank is
[identical to the intra bank](spi-reduced-temporal-findings.md#every-coded-array-is-transformed-even-at-q-zero)
in this ELF. Uncoded flags are cleared individually at worker bytes
2368–2371. Bytes 2372/2373 are cleared at `0x6a07c`.

Call `0x6a080` invokes reconstruction `0x6cc58`. It combines each
partition with the corresponding region of the selected reference plane
through callback 1496, `0x618a0`, called at `0x6ccf4`. Flag 1 broadcasts
the inverse-transformed DC value; flag 7 combines individual residuals;
flag 0 copies the prediction. Results retain the previously recovered
signed-16 wrapping and native byte clipping.

## Q zero always uses four 8-by-8 partitions

The branch at `0x69c5c` calls field reader `0x6b06c`. It consumes a
one-bit field at `0x6b0a4` and stores it in byte 2375, but computes side
8 with a constant right shift at `0x6b0b0`. Thus both values of that bit
select the same four 8-by-8 arrays. Bytes 29/30 become 8 and byte 31
becomes 3.

The mask is always the Q-zero side-8 temporal mask, encoded as positive
code N indexing table entry `N-1`. Read partitions indicated by bits
5, 4, 3 and 2. Each uses the same temporal coefficient reader, with
calls at `0x6b204`, `0x6b244`, `0x6b284` and `0x6b2c4`. All four arrays
are cleared first. The coefficient scan pointer at `0xf4630 + 3*24`
resolves to `0x29c12`, the same side-8 zigzag used by nonzero Q.

Call `0x69c74` invokes `0x6cd24`, which always reconstructs four side-8
partitions at offsets 0, 8, 128 and 136 of the 16-by-16 output. It uses
callback 1504, `0x61db0`, with no dequantization, inverse transform,
DC broadcast or spatial residual accumulation. A nonzero flag applies
individual coefficients to reference bytes.

### Missing partitions can suppress the first residual

The Q-zero reader stores a successful coefficient-reader flag at the
correct partition byte. However, every uncoded branch clears byte 2368:

| Uncoded partition | Store instruction | Cleared flag byte |
| --- | --- | --- |
| 0 | `0x6b1bc` | 2368 |
| 1 | `0x6b1c4` | 2368 |
| 2 | `0x6b1cc` | 2368 |
| 3 | `0x6b1d4` | 2368 |

Consequently, if partition 0 was coded and any later partition is absent,
the first partition's coefficients are decoded but never added to the
reference. Its coded flag survives only when all four primary mask bits
are set. This behavior is reproduced by the independent pixel decoder.

For example, mask 32 codes only partition 0. The later uncoded branches
clear its flag, and the selected plane copies the reference unchanged.
Mask 60 codes all four primary partitions and preserves partition 0's
flag. Low mask bits 1/0 do not change either case.

Uncoded partitions 1–3 leave their own old flag bytes intact. Their
coefficient arrays were cleared, so the stale flags do not introduce
nonzero residuals in the tested reconstruction path. Secondary flags
2372/2373 are explicitly cleared at `0x6b2d8`.

## Output updates one cache plane and copies two reference planes

Submode-3 output dispatch is at `0x5dee0`–`0x5df38`. It obtains the
current per-block cache index through worker pointer 2392 and block
ordinal 18, then looks up context pointer `1136 + 8*cache_index`.
It writes the selected reconstructed plane into that cache image and
the current destination image. It copies the other two planes directly
from context reference image 1000 at the same block coordinates.

The cache-write calls for selected planes 0, 1 and 2 are `0x5df58`,
`0x5e10c` and `0x5e090`. The corresponding current-image writes are
`0x5df74`, `0x5e148` and `0x5e0ec`. This path does not call cache-order
helper `0x6e728`. The complete policy for multiple cache entries remains
separate work.

### Selector 3 is accepted but writes no primary color plane

The two-bit selector can also contain 3. Reference selection skips all
three color-copy branches at `0x69420`, and primary output skips every
color/cache write at `0x5df38`. Residual processing can still touch
channel-3 scratch storage, but this does not establish an alpha update:
the separate alpha pass still runs afterwards.

Eighteen four-frame native comparisons cover Q values 0, 1 and 51,
both header sampling flags, and allocation fills 0, `0xa5` and `0xff`.
After an initial literal image, repeated selector-3 primary blocks leave
the destination image's old color pixels untouched. Because completed
frames rotate destination/reference descriptors, the visible color
alternates between the initialized 128-valued image and the original
literal image. An alpha submode-0 pass retains the initial alpha pixels.

These observed successful calls should not be interpreted as a fourth
supported color selector. The bounded independent sequence decoder
accepts selectors 0–2 and rejects selector 3.

## Validation and remaining work

All 1740 independently constructed corpus sequences match native output
for all 3984 frames:

- 1488 two-frame sequences cover all three color selectors and every
  temporal mask. Q zero uses side 8; Q values 1, 22, 27, 32, 37 and 51
  use both sides 8 and 16.
- 252 four-frame sequences mix selected-plane, literal and submode-0/2
  primary blocks, with temporal alpha blocks. They cover dimensions
  1 by 33, 17 by 33 and 65 by 65; one, two or three block rows per
  packet; both header sampling flags; and seven C/D/E combinations,
  including swapped C/D, positive/negative adjustments and flooring at zero.

The corpus contains 7074 selected-plane blocks, with selectors 0/1/2
occurring 2344/2428/2302 times. It exercises 5610 side-8 payloads and
1464 side-16 payloads. Their partitions include 4692 DC-only arrays,
8316 general arrays and 10896 uncoded arrays. In 840 blocks, Q-zero
flag clearing suppresses a coded first partition.

Native comparisons include consumed bits, selected Q, raw coefficient
arrays and reader flags, split/mask fields, transformed coefficients,
effective combination flags, selected output blocks, all three stored
color planes and complete visible pixels. Mixed temporal blocks retain
their earlier motion and residual checks. The corpus executes 7779
distinct native instructions.

The isolated Q-zero field tests cover every mask, both values of the
ignored bit, all eight initial bit alignments, all four coefficient-array
slots and two different initial flag patterns: 8192 cases. They compare
the complete 2048-byte coefficient region, all six flags, dimensions,
mask and consumed bits. Of these, 3584 confirm suppression of a coded
first partition. Together with the 13312 quantizer-adjustment probes,
these checks execute 371 distinct native instructions without imports.

Six paired full-decoder comparisons flip only the ignored Q-zero bit
and retain identical pixels. Eight four-frame native repeats with fresh
allocation fills `0xa5` and `0xff` also match independent reconstruction.
Eight prior full-size/reduced temporal sequences preserve their recorded
output digests after the disposable decoder was extended.

Eighteen additional four-frame sequences trace 324 selected-plane blocks
through output. They compare the prediction buffer with the independently
recovered previous-frame region, verify zero current-row motion slots and
availability bytes, and confirm that only the selected cache plane changes.

All 1740 corpus artifact digests and 3984 independently reconstructed
frame digests were verified again after generation. The eighteen
selector-3 artifacts and their 72 expected frame digests were also checked.
The representative 65-by-65, two-block-row-group, reduced-header sequence
with C 30, D 20, E −17 and variant 1 has SHA-256
`b19d0fc126a34b13bd8b2a7c14aaa8f191d2b7dd35ec59e8cf20774fa6d3cdad`.
Its fourth frame's output SHA-256 is
`eed749d0295b4a3e44016c04d406e9ca16dedd1effd7be93247ffe263b7bc220`.
The disposable artifact prefixes the header and each separately submitted
image-data block with a little-endian length. This is test storage, not
a recovered multi-frame SDOCX wrapper layout.

APK/ELF identity, 59 cited instruction words, nine coefficient-reader
calls, two scan pointers, two combination callbacks and 372 mask/class
table bytes were verified against the native binary. Local documentation
paths and heading links also pass validation.

Multiple-cache selection, mixed temporal/intra spatial-edge fallback,
broader malformed-input handling and device-export compatibility remain
open. Maintained changes are Markdown-only; scripts and constructed
sequences remain disposable local evidence.
