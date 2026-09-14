# SPI quantized mode-3 color decoding

## Evidence and scope

Recovered from Samsung Notes 4.4.45.37 ARM64 `libSPenBase.so` in the
[identified APK](README.md#sources-and-validation). This extends the
[zero-quantizer color decoder](spi-color-intra-findings.md) with nonzero
quantization for primary mode 3, submode 1 and full-size color planes.
The independent scratch reader reproduces native pixels for the complete
seven-quality sweep, including its quality-24 gradient.
It also matches 240 new native-generated images and 768 constructed images,
including intermediate signed planes and every final output byte.

The supported configuration remains wire color index 4, header flags
`0xe0`, packet byte B zero and API color value 500. The selected block
quantizer Q is bounded to 0–51 in the independent reader. Subdivision,
prediction fields, separate color marker grids, external color conversion
and final inverse color conversion follow the earlier findings.

Q zero retains the earlier residual path. Q nonzero changes the side-4
mask table, coefficient escape representation, coefficient scan, scaling,
inverse transform and prediction/residual combination. Later
[reduced-plane work](spi-reduced-color-findings.md) recovers the same submode
with header flags `0xf0`, including its distinct Q-zero behavior. Other
submodes, broader configurations and device-export validation remain open.
Maintained changes are Markdown-only; no SDK code changed.

## The side-4 partition mask depends on Q

Mask reader `0x6b704` loads Q from worker byte 56 at `0x6b724`. For
side 4, it reads positive integer P, requires `P <= 64`, then indexes
one of five 64-byte rows beginning at `0x2aa9c`. The row is selected
through the byte table at `0x29214`, read at `0x6b7bc`:

| Selected Q | Mask-table row |
| --- | --- |
| 0–21 | 0 |
| 22–26 | 1 |
| 27–31 | 2 |
| 32–36 | 3 |
| 37–50 | 4 |
| 51 | 0 |

The Q = 51 entry is the observed native byte, also exercised in complete
images. This does not infer the intended design of that boundary. The
native side-4 branch rejects Q above 51 at `0x6b7ac`–`0x6b7b0` and a
table-row byte above 4 at `0x6b7c0`–`0x6b7c4`. The side-8/16 branch
does not perform those Q checks.

Row 0 is already listed in the [alpha payload findings](spi-alpha-payload-findings.md#a-table-maps-the-coded-mask).
The remaining rows, indexed by `P - 1`, are:

```text
row 1:
60 63 62 61 28 52 44 56 12 48 20 40 31 47 59 16
55 4 32 8 30 54 46 58 29 0 36 53 45 24 57 15
51 50 14 13 49 22 43 42 23 21 41 34 18 38 10 6
26 17 33 39 9 5 35 37 27 25 19 11 2 7 1 3

row 2:
60 63 61 62 28 12 52 44 56 48 20 40 4 16 8 32
0 31 59 47 55 36 24 29 45 53 57 30 15 58 46 54
13 51 49 50 14 43 21 41 23 42 22 33 34 17 9 5
35 18 10 6 11 39 37 27 25 38 19 26 7 1 2 3

row 3:
60 28 12 52 44 56 63 20 48 40 4 16 8 61 32 62
0 36 24 31 59 29 47 55 57 53 45 13 30 15 58 49
51 46 54 21 14 50 43 41 23 42 22 33 9 17 35 5
34 11 10 37 25 18 27 39 6 19 1 38 7 26 2 3

row 4:
60 12 28 20 4 48 44 52 56 40 8 16 0 32 36 24
63 61 62 31 59 29 57 47 13 55 45 53 49 51 15 21
41 30 58 43 46 54 23 50 14 33 9 42 22 17 35 5
34 11 25 37 1 27 10 18 19 6 39 7 2 26 38 3
```

Sides 8 and 16 retain the eight-entry unary mask table at `0x2abdc`.
Partition p of P partitions is coded when mask bit `P + 1 - p` is set.
Uncoded partitions consume no coefficient payload and receive flag 0.

## Quantized coefficients use a fixed zigzag scan

The Q-nonzero branch of section reader `0x6a70c` calls `0x6b8a8` at
`0x6a890`. Its arguments are the bit reader, scan pointer, destination
signed-16 array, side length and flag-byte destination.

The caller obtains the scan from the first pointer in the side's
24-byte row at `0xf4630`, through `0x6a864`–`0x6a884`. Unlike Q zero,
this is always variant 0: the [zigzag scan](spi-alpha-residual-findings.md)
for that side, independent of prediction mode. All three color planes use
the same rule.

Read a positive integer N using the existing leading-zero representation.
N is the number of tokens, bounded by `side*side` at
`0x6b990`–`0x6b9a4`. The caller clears each coded coefficient array
before invoking this reader. Initialize the next scan index to zero.
For each token, read positive integer M and one sign bit:

| M | Magnitude and run |
| --- | --- |
| 1–127 | Existing compact pair at `0x2ad24[M - 1]` |
| 128 and above | `magnitude = ((M - 1) >> 8) - 1`; `run = (M - 1) & 255` |

For a positive magnitude m and run r, the ordinary escape is therefore
`M = 256*(m + 1) + r + 1`. The eight-bit run field is independent of
partition size. Instructions `0x6ba78`–`0x6ba94` choose and split the
escape; `0x6bac4`–`0x6bad0` read a compact pair.

Set `scan_index = next_index + run`, store the signed magnitude at
`scan[scan_index]`, and set `next_index = scan_index + 1`. The sign
operation at `0x6bae4`–`0x6bafc` negates when the sign bit is 1 and
stores only the low sixteen bits. Large magnitudes wrap rather than clamp.

Native low escape codes have observable noncanonical behavior:

- M = 128–256 produces magnitude −1 and runs 127–255.
- M = 257–512 produces magnitude zero and runs 0–255.
- M = 513 starts magnitude 1, run zero.

Thus the declared token count is not necessarily the number of nonzero
stored values on arbitrary input. A negative escape magnitude reverses
the apparent sign-bit meaning. Isolated checks preserve these effects;
they do not establish that the native encoder emits those forms.

On success, `0x6bb04`–`0x6bb38` sets flag 1 when there is exactly one
token and its scan index is zero. Other coded arrays receive flag 7.
The flag depends on the tokens and their positions, including a token
whose stored value becomes zero.

### The native scan check is weaker than a partition bound

The native reader adds the run, loads `scan[scan_index]`, then checks
whether the loaded destination index exceeds 255 at
`0x6bad4`–`0x6bae0`. It does not first bound the accumulated scan index
by `side*side`, nor compare the loaded index with the partition's area.

Eighty isolated probes supplied one entry beyond a side-4 or side-8 scan
in a larger emulated table. Values 0, 100 and 255 were accepted and wrote
the corresponding element of a separately allocated 256-element output.
Values 256 and 65535 returned −202. The independent reader rejects every
one of those out-of-scan cases before the lookup.

These probes establish this routine's logical bounds behavior using padded
emulated inputs and destinations. They do not reproduce a host memory
overrun or a device vulnerability. Oversized token counts were separately
rejected before any coefficient or flag write.

## Dequantization uses six scale families and signed-16 wrapping

Stage `0x6abec` runs between field parsing and prediction. For the
recovered full-size submode-1 path, all three color planes call it with
the same selected Q. Worker byte 58 holds that Q; the separate mapping
at worker byte 59 does not replace it in this path.

The byte table at `0x291a0`, loaded through relocation `0xeee18`, packs
`(Q % 6) << 4 | (Q / 6)` for Q = 0–51. Instructions
`0x6ac08`–`0x6ac58` split it into quotient q and remainder r. Side 4
uses four partitions, side 8 one, and side 16 one; only coded partitions
are scaled and transformed.

| Side | Dequantization callback | Scale source |
| --- | --- | --- |
| 4 | Context 1488, `0xeeda8` → `0x61224` | `0xeee40` → `0x29448`, 16 values per r |
| 8 | Context 1480, `0xeedb0` → `0x61520` | `0xeee48` → `0x295c8`, 64 values per r |
| 16 | Context 1472, `0xeedb8` → `0x61818` | `0xeee28` → `0x29208`, one value per r |

For side 4, the scale at row y, column x uses class `(y % 2) + (x % 2)`:

| r | Class 0 | Class 1 | Class 2 |
| --- | --- | --- | --- |
| 0 | 160 | 208 | 256 |
| 1 | 176 | 224 | 288 |
| 2 | 208 | 256 | 320 |
| 3 | 224 | 288 | 368 |
| 4 | 256 | 320 | 400 |
| 5 | 288 | 368 | 464 |

For side 8, repeat this 4×4 class tile in both directions:

```text
0 1 2 1
1 3 4 3
2 4 5 4
1 3 4 3
```

| r | Class 0 | Class 1 | Class 2 | Class 3 | Class 4 | Class 5 |
| --- | --- | --- | --- | --- | --- | --- |
| 0 | 320 | 304 | 400 | 288 | 384 | 512 |
| 1 | 352 | 336 | 448 | 304 | 416 | 560 |
| 2 | 416 | 384 | 528 | 368 | 496 | 672 |
| 3 | 448 | 416 | 560 | 400 | 528 | 720 |
| 4 | 512 | 480 | 640 | 448 | 608 | 816 |
| 5 | 576 | 544 | 736 | 512 | 688 | 928 |

For side 16, the uniform scale for r = 0..5 is `40, 45, 51, 57, 64, 72`.
The side-4 and side-8 tables each contain two identical six-row banks;
submode 1 selects the first bank.

Let p be the signed coefficient multiplied by its scale. Set
`shift = q - 4` for side 4, `q - 6` for side 8, or `q - 3` for side 16:

```text
scaled = signed16(p << shift)                         if shift >= 0
scaled = signed16((p + 2^(-shift-1)) >> (-shift))      otherwise
```

Right shifts are arithmetic and round with the shown positive bias.
Flag 1 scales only coefficient zero and leaves the rest of the array
untouched; other flags scale the entire array. The pipeline invokes these
helpers only for coded partitions. No saturation occurs at this stage.

## Inverse transforms preserve intermediate wrapping and orientation

Each dequantization call is followed by an in-place inverse transform:

| Side | Transform callback |
| --- | --- |
| 4 | Context 1448, `0xeed90` → `0x60674` |
| 8 | Context 1456, `0xeed98` → `0x608f4` |
| 16 | Context 1464, `0xeeda0` → `0x60b28` |

Here `wrap(v)` means signed-16 conversion. Every right shift below is
arithmetic; the resulting low sixteen bits match the native logical-shift
instructions where those are used immediately before a halfword store.

### Side 4

For a four-element vector a, define:

```text
e0 = a0 + a2
e1 = a0 - a2
o0 = a1 + (a3 >> 1)
o1 = a3 - (a1 >> 1)
F4(a) = [e0+o0, e1-o1, e1+o1, e0-o0]
```

First apply F4 to every input column, wrapping each result to produce an
intermediate matrix T. Then process each row of T using the same e/o
definitions, but replace e0/e1 with `wrap(e0)+32` and `wrap(e1)+32`,
and o0/o1 with their wrapped values. Shift each final F4 result right
by six, wrap it, and store the row's results as an output column.

The first column pass begins at `0x606a4`. The second pass explicitly
wraps intermediates at `0x60760`–`0x6078c`; final stores at
`0x608b8`–`0x608dc` establish the transposed output orientation.

### Side 8

First replace input coefficient zero with `wrap(coefficient[0] + 32)`.
For an eight-element vector a, define:

```text
e0 = a0 + a4
e1 = a0 - a4
e2 = a2 + (a6 >> 1)
e3 = a6 - (a2 >> 1)
E = [e0+e2, e1-e3, e1+e3, e0-e2]
b0 = a5 - a3 - a7 - (a7 >> 1)
b1 = a7 - a3 - (a3 >> 1) + a1
b2 = a7 + a5 + (a5 >> 1) - a1
b3 = a5 + a3 + a1 + (a1 >> 1)
O = [b3-(b0>>2), b2-(b1>>2), b1+(b2>>2), b0+(b3>>2)]
F8(a) = [E0+O0, E1-O1, E2+O2, E3+O3,
         E3-O3, E2-O2, E1+O1, E0-O0]
```

Apply F8 to each input column, wrapping each result into T. Apply F8 to
each row of T, shift its results right by six, wrap, and write them as an
output column. There is no additional final rounding bias. The native
input adjustment is at `0x60930`–`0x60938`, intermediate halfword stores
at `0x609c4`–`0x60a04`, and final transposed stores at
`0x60ac4`–`0x60b00`.

### Side 16

The 16×16 signed transform matrix M is at `0x29248`, reached through
relocation `0xeee38`. Its first eight columns are below. Recover the other
half with `M[r][15-c] = M[r][c]` for even r and `-M[r][c]` for odd r.

```text
 64  64  64  64  64  64  64  64
 90  87  80  70  57  43  25   9
 89  75  50  18 -18 -50 -75 -89
 87  57   9 -43 -80 -90 -70 -25
 83  36 -36 -83 -83 -36  36  83
 80   9 -70 -87 -25  57  90  43
 75 -18 -89 -50  50  89  18 -75
 70 -43 -87   9  90  25 -80 -57
 64 -64 -64  64  64 -64 -64  64
 57 -80 -25  90  -9 -87  43  70
 50 -89  18  75 -75 -18  89 -50
 43 -90  57  25 -87  70   9 -80
 36 -83  83 -36 -36  83 -83  36
 25 -70  90 -80  43   9 -57  87
 18 -50  75 -89  89 -75  50 -18
  9 -25  43 -57  70 -80  87 -90
```

For input matrix C and sums over k = 0..15:

```text
T[column][sample] = wrap((sum(M[k][sample] * C[k][column]) + 64) >> 7)
output[row][column] = wrap((sum(M[k][column] * T[k][row]) + 2048) >> 12)
```

The first pass rounds and stores signed-16 intermediates through
`0x60e30`–`0x60ed0`. The second pass begins at `0x60f14`, rounds with
2048 and stores through `0x611d4`. It does not saturate intermediate values.

### Flag 1 uses only the DC coefficient

Each transform's flag-1 branch updates only element zero. If its input
is d, the result is `(d + 32) >> 6` for sides 4 and 8, or
`(d + 65) >> 7` for side 16. The side-16 result follows the two shifts
at `0x60b74`–`0x60b8c`; its rounding differs from the smaller transforms.
All other elements remain untouched and are ignored by flag-1 combination.

## Combination broadcasts DC and clamps to each plane's bit depth

For Q nonzero, signed prediction helper `0x6d0e8` selects context
callback 1568, relocation `0xeedd8` → `0x62254`. At
`0x6d114`–`0x6d138`, plane zero receives bit depth 8 and the other
two planes bit depth 9. The callback receives this ninth argument on the
stack and forms maximum value `(1 << depth) - 1` at
`0x62264`–`0x62274`.

| Flag | Reconstructed partition |
| --- | --- |
| 0 | Copy prediction samples without clipping |
| 1 | Add transformed element zero to every prediction sample |
| 7 | Add the corresponding transformed element to each prediction sample |

For flags 1 and 7, wrap the sum to signed 16 bits, then clamp it to
0–255 for plane zero or 0–511 for the other two planes. The flag-7
vector/scalar paths are at `0x62358`–`0x6238c` and
`0x623d4`–`0x623e8`. Flag 1 broadcasts element zero through
`0x62404`–`0x62438` and applies the same clipping. This path does not
apply Q-zero vertical/horizontal residual accumulation for prediction
modes 0/1.

After all three signed planes are reconstructed, the earlier
[inverse color transform](spi-color-intra-findings.md#the-output-callback-converts-signed-planes-back-to-bytes)
produces their frame bytes. Subsequent blocks use those decoded frame
pixels, with separate prediction-marker state for each plane.

## Validation

The initial native-generated quality sweep supplied 33 coded arrays and
590 tokens at Q = 1, 4, 8, 16, 24 and 32. Their sides were 4 (12 arrays),
8 (five) and 16 (16). All coefficient values, consumed bits, flags,
dequantized arrays, inverse transforms, prediction state and final pixels
matched independently. Seventy-seven tokens used escapes; the largest
decoded magnitude was 1341. Q zero retained its previously matched pixels.
The quality-24 SPI hashes to
`32ca74da29d1cc1f67080654f625714445c5ce5401f41b4edb71b8c51b46616b`;
its independently decoded pixels hash to
`633aa07d085a65b7e3654f33769e6c7a89a5c07bf5881d46f3973787d691ac5d`.

Isolated coefficient validation covered 9392 streams at all eight initial
bit alignments: 2032 compact-code/sign cases, 1152 ordinary escapes,
24 dense arrays, 24 mixed arrays and 6160 low-escape cases. Escapes
included signed-16 wrapping and magnitudes through 65536. Twenty-four
oversized token counts returned −202 without changing coefficients or
flags. The 80 scan-boundary probes above separately characterized logical
lookup bounds. These checks executed 317 distinct native instructions.

Primitive checks covered 3120 dequantization arrays across Q = 0–51,
all three sides and flags 1/7; 612 full inverse transforms with signed-16
boundary and random inputs; all 65536 signed-16 DC inputs for each side
(196608 DC checks); and 360 combination grids spanning both bit depths,
flags 0/1/7, signed-16 inputs and padded destination strides. Output guards
were checked for array scaling, full transforms and combination. These
checks used Python `random.Random(0)` and executed 1380 distinct native
instructions with no imported host calls.

Complete-image validation added two corpora:

- 240 native-generated images used encoder qualities 1, 6, 21, 22, 26,
  27, 31, 32, 36, 37, 50 and 51; sizes 1×1, 2×3, 16×16, 17×33 and
  65×65; and smooth, striped, checker and random patterns. They contain
  232 mode-3 color blocks, 900 sections and 919 coded quantized partitions,
  alongside 120 differential blocks, 596 palette blocks and 1632 alpha
  blocks. Some inputs emit only previously recovered modes. Native block
  quantizer selection also exercised Q values different from the encoder's
  quality argument. Comparisons executed 11979 distinct native instructions.
- 768 constructed images include every side-4 mask code at Q = 1, 22,
  27, 32, 37 and 51, plus every side-8/16 mask code at those Q values
  (480 images). Another 288 images mix primary and alpha modes at
  Q = 1, 6, 21, 26, 31, 36, 50 and 51, sizes 1×33, 17×33 and 65×65,
  one to three block rows per packet and all four packet selectors.
  They contain 1944 mode-3 color blocks, 16488 sections and 43848
  partitions: 7719 with flag 1, 15417 with flag 7 and 20712 uncoded.
  They also contain 504 differential blocks, 480 palette blocks and
  3744 alpha blocks across 1184 packets. All 18 prediction modes occur.
  Comparisons executed 11708 distinct native instructions.

Every full-image comparison starts independent decoding from header and
packet bytes alone. Native marker grids, partition fields, coefficients,
scales, transforms, neighbor arrays, prediction grids, reconstructed signed
planes and final pixels are compared afterward. Native state and offsets
are not supplied as inputs to the complete reader.

Eight additional native repeats with fresh allocation fills `0xa5` and
`0xff` check state and pixels for a mask image, mixed images across packet
groups, and a native-generated quality-51 image. The 477 earlier original,
alpha, palette, differential and zero-quantizer image cases retain their
recorded output, and all seven quality-sweep outputs now match independently.
Scratch code and generated artifacts remain disposable.

## Remaining work

The [reduced-plane trace](spi-reduced-color-findings.md) now recovers mode-3
submode 1 with reduced secondary planes. Recover the other submodes, then
extend reference selection and other header/packet configurations. Alpha literal
marker behavior, broader malformed-input policy, SDK integration and real
document/rendered-reference validation remain open. The recovered path
does not establish arbitrary SPI compatibility.
