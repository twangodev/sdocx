# SPI differential color blocks

## Evidence and scope

Recovered from Samsung Notes 4.4.45.37 ARM64 `libSPenBase.so` in the
[identified APK](README.md#sources-and-validation). Primary mode 2 reads
byte symbols, expands repeat runs and reconstructs color relative to
earlier rows. It does not use the mode-4 palette or alpha coefficient syntax.

An independent scratch implementation matched 104 complete synthetic
images against the native decoder, including intermediate symbols,
prediction prefixes, reconstructed planes and every output pixel. The
complete reader now supports primary modes 0/1/2/4/5 and alpha modes 0/1/3
for wire color index 4, header flags `0xe0` and packet byte B zero.
Mode-2 images exercised all four values of the packet's two-bit selector.

These were constructed codec inputs. The original 30 native-generated
images contain no mode-2 blocks. Later [mode-3 color work](spi-color-intra-findings.md)
adds native-generated mode-2 cases and primary mode 3 with zero quantization.
Device exports, other configurations and SDK integration remain open.

## A sampling bit follows the mode prefix

Primary prefix `0000` selects `0x68264`. The routine reads one bit at
`0x682b0` and stores it at worker byte 2493 through `0x682d0`:

| Sampling bit | First color plane | Second and third color planes |
| --- | --- | --- |
| 0 | 256 symbols, 16×16 reconstruction | 256 symbols each, 16×16 reconstruction |
| 1 | 256 symbols, 16×16 reconstruction | 64 symbols each, then expansion to 16×16 |

The three symbol streams follow immediately, without byte alignment
between them. `0x683e8` calls the shared dispatcher `0x689c0`. With sampling
zero and mode byte 48 equal to 2, it calls `0x6bd24` for all three planes
at `0x68af8`, `0x68b08` and `0x68b18`. Sampling one calls `0x6bd24` for
the first plane and `0x6bebc` for the other two through `0x68a08`,
`0x68a18` and `0x68a28`.

The output-plane pointers remain worker offsets 8416, 8424 and 8432.
For the tested API color value 500, they supply output byte positions
1, 2 and 0, respectively, as in [literal](spi-literal-block-findings.md)
and [palette](spi-palette-block-findings.md) blocks.

## Symbol codes use unary prefixes and an escape

The shared symbol reader is `0x6b3e8`, called with output count 256 at
`0x6bd90` or count 64 at `0x6bf28`.

For the independently modeled symbol forms, count zero bits `q` before
the next one bit, then read a two-bit remainder `r`:

| Prefix and remainder | Decoded byte |
| --- | --- |
| `0 <= q <= 20` | `4*q + r` |
| `q = 21`, `r <= 2` | `84 + r` |
| `q = 21`, `r = 3` | Read the following eight bits verbatim |

Thus values 0–86 have a direct form; 87–255 use the escape. The escape
also accepts values below 87. Examples are `100` for zero, `101` for one,
and 21 zero bits followed by `111` and an eight-bit value for an escape.

The native leading-zero table at `0x2a958` has 256 entries:
`table[0] = 8`, otherwise `8 - bit_length(value)`. Reads at `0x6b4c0`,
`0x6b4d0`, `0x6b4e8` and `0x6b4fc` combine it with bit-reader alignment;
`0x6b514` consumes the prefix and `0x6b520` reads the remainder. The
direct/escape decision is `0x6b524` through `0x6b548`. Prefixes longer
than 21 zero bits are outside the independent reader's supported syntax.

## Matching prior symbols introduce a repeat field

The reader tracks two previous literal values, initialized to 255 and 0
at `0x6b428` through `0x6b430`. At the start of each iteration:

1. If the two values match, read positive integer `P` with the
   [copy-code representation](spi-copy-block-findings.md), then compute
   `R = (P - 1) & 255`.
2. Append `R` copies of the matching byte.
3. Read and append one new symbol using the code above.
4. Shift the two remembered literal values and check the output count.

The first literal zero therefore enables a repeat field immediately:
it matches the initialized prior zero. Repeats can have length zero.
The repeat count is masked to eight bits at `0x6b484`–`0x6b488` and
expanded through `0x6b49c`. A repeat always precedes a new literal;
it is not a standalone terminating run.

The native count check occurs after the new literal at `0x6b458`, not
after repeat expansion. Isolated probes confirmed writes beyond the
requested logical output count while returning zero:

| Requested symbols | First literal | Repeat count | Final literal | Bytes written |
| --- | --- | --- | --- | --- |
| 64 | 0 | 63 | 1 | 65 |
| 64 | 0 | 255 | 1 | 257 |
| 256 | 0 | 255 | 1 | 257 |

These probes used a larger emulated destination to observe the writes.
They establish a missing logical count check in this routine, not a host
memory overrun or device reproduction. The independent reader rejects a
repeat unless space remains for its required final literal. Positive
values 1, 257, 513 and 65537 all produced repeat count zero in separate
native checks, confirming the byte truncation.

## The packet selector controls differential reconstruction

The [kind-2 selector](spi-data-packet-findings.md#kind-2-has-a-14-byte-prefix)
at bits 81–82 occupies worker byte 10. Both plane readers copy it to byte
4300, then set step `s` at 4301 and range count `n` at 4302:

| Selector | Step `s` | Range count `n` | Reconstruction branch |
| --- | --- | --- | --- |
| 0 | 1 | 256 | Byte-vector addition |
| 1 | 2 | 129 | Scalar wrap adjustment and clamp |
| 2 | 4 | 65 | Scalar wrap adjustment and clamp |
| 3 | 1 | 256 | Scalar wrap adjustment and clamp |

These assignments occur at `0x6bd34`–`0x6bd8c` and
`0x6becc`–`0x6bf24`. They identify the selector's mode-2 effect; they do
not fully assign its meaning in other coding modes.

For symbol byte `c`, the signed difference is:

```text
d = c / 2             when c is even
d = -(c + 1) / 2      when c is odd
v = previous_pixel + s*d
```

Selector zero adds modulo 256. For nonzero selectors, let
`period = s*n`, `low = -floor(s/2)` and `high = 255 + floor(s/2)`.
The 8×8 reader first replaces `v` by the signed remainder after division
by `period`, using truncation toward zero at `0x6bf7c`–`0x6bf80`.
The 16×16 reader does not perform that division.

Both scalar paths then add `period` if `v < low`, subtract `period` if
`v > high`, and clamp the result into 0–255 through a lookup table.
The full-size adjustment is `0x6bde4`–`0x6bdfc`; the reduced adjustment
is `0x6bf84`–`0x6bf9c`.

The 512-byte table at `0xf46a8` is `clamp(index - 128, 0, 255)`.
Constructor `0x5d564`–`0x5d56c` loads it through relocation `0xeee68`,
adds 128 and stores the indexed pointer at context offset 1176. The
scratch reader bounds its lookup domain; arbitrary out-of-domain symbols
and malformed native table accesses are not characterized here.

## Prediction prefixes retain earlier bytes in the reduced path

Mode 2 gathers three external above/left edges through `0x6d5a8`, then
completes missing neighbors with `0x6d690`. For the available-block patterns
in these images, each plane follows the [alpha edge rules](spi-alpha-state-findings.md#external-alpha-edges-can-be-reconstructed-from-earlier-pixels):
128 at the first block of a group, the first left pixel across a group's
first row, and decoded above-row pixels on later rows.

The output buffers each have sixteen preceding bytes. Native worker
initialization fills these regions with 128 at `0x5d258`–`0x5d29c`.
Full-size mode 2 overwrites all sixteen prefix bytes from above-edge
entries 1–16 at `0x683b8`–`0x683e0`. Each reconstructed pixel then refers
to the byte sixteen positions earlier, using this prefix for its first row.

With reduced secondary planes, native helper `0x5e74c` filters above
entries 1–32 into sixteen bytes. For those 32 input bytes `a`, the filter is:

```text
f[0] = (3*a[0] + a[1] + 2) / 4
f[i] = (a[2*i-1] + 2*a[2*i] + a[2*i+1] + 2) / 4, i = 1..15
```

Only `f[0..7]` are copied into the last eight prefix bytes at `0x683a4`
and `0x683b0`. The first eight prefix bytes retain their earlier values.
They can come from initialization or a previous full-size mode-2 block,
including one in a prior packet group.

For selectors 1–3, the 8×8 reader uses the byte eight positions earlier
at `0x6bf70`, so its first row sees the new eight-byte prefix. Selector
zero instead loads sixteen prefix bytes at `0x6c158` and accumulates
sixteen-byte vectors at `0x6c184`–`0x6c1a0`. Its 64 outputs therefore use
distance 16, including the retained first half of the prefix. The complete
independent reader models this distinction; simply applying an eight-wide
predictor to every reduced block did not describe the native instructions.

## Reduced planes expand with asymmetric interpolation

Calls at `0x6840c` and `0x68424` use context callback 1512. Initialization
at `0x5d6fc`–`0x5d710` selects relocation `0xeeea0`, resolving to `0x600a8`.
Each 8×8 plane is expanded in place to 16×16.

Vertical expansion preserves the first and last rows. Between each pair
of source rows `a` and `b`, it inserts `(3*a+b+2)/4` followed by
`(a+3*b+2)/4`, component by component. This yields sixteen eight-byte rows.
For each such row, output columns `2*x` retain source pixel `x`; columns
`2*x+1` use `(pixel[x]+pixel[x+1]+1)/2`. The final pixel is repeated at
the right edge. All divisions are integer divisions with the shown rounding.

## Validation

The APK identity, extracted ELF, cited instruction words, leading-zero
table, clamp table, callbacks and normal neighbor dispatches were checked.

- 6160 symbol streams matched the native reader, consumed bits and output
  guards: all 256 byte values as single symbols and repeated 256-symbol
  streams, repeat boundaries, ascending/descending values and paired runs,
  each at all eight initial bit alignments. These executed 270 distinct native instructions.
- 696 redundant escapes, 32 repeat-count truncation cases and 24 logical
  output-overrun cases confirmed the distinctions above. They executed
  277 distinct native instructions; the overrun cases are intentionally rejected
  by the independent reader.
- 1572 plane reconstructions matched native bytes, step/range metadata,
  output guards and consumed bits. Both sizes covered selectors 0–3:
  every symbol 0–255 for selectors 0/3, 0–128 for selector 1 and 0–64 for
  selector 2, plus 160 random planes from Python `random.Random(0)`.
  A further 100 edge
  downsampling and 100 in-place expansion cases matched exactly. Combined,
  these checks executed 676 distinct native instructions.
- 104 complete images used sizes 1×1, 1×33, 17×33, 33×49 and 65×65,
  one to three block rows per packet, all four selectors and two variants.
  Their 248 packets contained 716 mode-2 blocks: 340 full-size and 376
  with reduced secondary planes. They also contained 132 palette blocks,
  literal/copy color blocks and 1112 alpha blocks. All native symbols,
  above arrays, persistent prefix bytes, reconstructed planes and final
  pixels matched the independent decoder. These comparisons executed
  5178 distinct native instructions.
- Eight full-image repeats with fresh allocation fills `0xa5` and `0xff`
  retained exact intermediate state and pixels, including reduced selector-zero
  blocks and multiple packets.
- All 221 previous original, alpha and palette images retained their
  recorded pixel digests with the extended independent reader.

The complete reader uses only the header and packet bytes. Native state
is compared afterward, not provided as reconstruction input. Scratch code
and generated artifacts remain disposable; maintained changes are
Markdown-only and no SDK code changed.

## Remaining work

The [mode-3 color trace](spi-color-intra-findings.md) now recovers full-size
planes with zero quantization; the [quantized color trace](spi-quantized-color-findings.md)
adds nonzero quantization for the same submode. Reduced mode-3 planes,
other submodes, other header/packet paths and reference-buffer behavior remain open.
Alpha literal marker behavior and general malformed-input policy
also remain open. Mode 2 now has independent decoding in the tested
configuration, but these synthetic comparisons do not establish arbitrary
SPI compatibility or validate the device document-saving wrapper.
