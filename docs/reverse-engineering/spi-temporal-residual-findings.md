# SPI temporal partition masks and residual coefficients

## Evidence and scope

Recovered from Samsung Notes 4.4.45.37 ARM64 `libSPenBase.so` in the
[identified APK](README.md#sources-and-validation). This is the residual
syntax used by the [temporal mode-3 paths](spi-temporal-block-findings.md),
including primary submode 2 with full-size color planes and alpha
submode 2. It differs from both the earlier
[alpha intra representation](spi-alpha-residual-findings.md) and
[quantized color intra representation](spi-quantized-color-findings.md).

The recovered partitions have sides 8 and 16. Independent parsing,
native coefficient comparisons and complete multi-frame comparisons
validate the stated configuration. No SDK implementation changed.

## A plane has one split bit and a temporal mask

Primary full-size submode 2 calls plane reader `0x6ae90` three times,
once per color plane. It reads a split bit at `0x6aec8`. Zero selects
one 16-by-16 partition; one selects four 8-by-8 partitions in raster
order. There are no prediction-mode fields: prediction comes from a
reference image and a motion vector.

Worker byte 2375 stores the split bit, byte 29 stores the side and
byte 31 stores its base-two logarithm. For side 16, read zero bits until
the first one and use the zero count as the index into the existing
eight-byte mask table at `0x2abdc`.

For side 8, read a positive integer N in the existing leading-zero
representation and use `N - 1` as the index into a 64-byte row. The row
selector remains the Q-to-class mapping at `0x29214`:

| Q | Row |
| --- | --- |
| 0–21 | 0 |
| 22–26 | 1 |
| 27–31 | 2 |
| 32–36 | 3 |
| 37–50 | 4 |
| 51 | 0 |

The temporal mask rows begin at `0x2abe4`, also reached through relocation
`0xeef08`. They are different from the intra side-4 rows at `0x2aa9c`:

```text
row 0:
63 61 60 62 53 59 55 57 29 45 47 31 52 28 56 44
54 58 48 30 49 13 46 20 21 41 51 15 12 43 40 23
50 14 22 42 37 25 24 36 32 8 16 17 39 4 26 9
27 33 5 11 38 7 10 19 34 6 18 35 0 1 2 3

row 1:
60 61 63 62 52 28 56 44 48 45 29 57 12 53 20 40
16 0 54 30 58 13 36 24 46 4 59 47 32 49 31 55
8 21 41 50 14 15 22 51 43 37 17 25 42 33 23 9
5 26 1 38 18 6 27 10 34 35 39 19 2 11 3 7

row 2:
60 61 28 52 44 56 20 12 48 0 16 4 40 32 8 36
24 62 63 45 53 57 29 13 49 41 21 30 54 58 46 50
5 17 14 22 37 25 9 33 42 18 1 6 26 47 38 10
34 59 31 55 2 15 51 43 23 27 19 11 7 35 39 3

row 3:
0 60 4 16 8 32 12 44 48 56 28 61 52 40 20 36
24 1 62 63 45 2 57 29 53 41 9 13 33 49 5 17
21 6 18 10 25 37 34 46 58 14 30 50 42 54 22 47
59 26 38 3 31 43 15 55 51 11 35 7 19 23 39 27

row 4:
0 60 32 8 4 16 12 48 56 28 44 52 40 20 24 36
61 1 2 45 57 9 41 33 29 13 53 62 49 5 17 63
21 25 37 10 58 46 34 14 18 6 42 47 50 30 59 54
43 3 22 38 31 26 15 55 11 51 35 27 23 19 39 7
```

The independent reader bounds Q to 0–51, side-8 N to 1–64 and the
side-16 zero count to 0–7. The native branch at `0x6af0c`–`0x6afa8`
indexes these tables without equivalent explicit bounds.

For P partitions, partition p consumes coefficients when mask bit
`P + 1 - p` is set. In the full-size path, each color plane has its own
mask; bits 1 and 0 do not introduce further arrays. The reduced temporal
path uses those bits separately and remains outside the complete decoder
in this milestone.

## Coefficient tokens have 63 compact entries per side

Coefficient reader `0x6bb5c` takes a bit reader, scan pointer,
destination signed-16 array, coefficient-storage offset and log2(side).
Calls from `0x6ae90` occur at `0x6b03c`; alpha uses the same reader at
`0x698b8` and `0x6992c`.

Read positive integer K, the token count. Start the next scan index at
zero. For each token, read positive integer M followed by one sign bit.
The zero-count branch at `0x6bc54`–`0x6bc60` selects:

| M | Magnitude and run |
| --- | --- |
| 1–63 | Side-specific compact pair at index `M - 1` |
| 64 and above | `magnitude = ((M - 1) >> (2*log2(side))) - 1`; `run = (M - 1) & (side*side - 1)` |

The pointer table at `0xf6918` is indexed by log2(side). Its side-8
entry resolves to `0x2ae22`; side 16 resolves to `0x2aea0`. Each entry
below is `magnitude:run`, in increasing M order:

```text
side 8:
1:0 1:1 2:0 1:2 1:3 3:0 1:4 1:5 2:1
4:0 1:6 1:7 5:0 1:8 1:9 2:2 6:0 1:10
3:1 1:11 7:0 1:12 1:13 8:0 2:3 1:14 9:0
4:1 1:15 1:16 10:0 2:4 1:17 3:2 1:18 11:0
1:19 1:20 5:1 1:21 12:0 2:5 1:22 13:0 1:23
1:24 14:0 1:25 6:1 1:26 15:0 3:3 1:27 2:6
4:2 16:0 1:28 17:0 1:29 7:1 1:30 1:31 18:0

side 16:
1:0 1:1 1:2 1:3 1:4 2:0 1:5 1:6 1:7
1:8 1:9 3:0 1:10 1:11 2:1 1:12 1:13 1:14
1:15 4:0 1:16 1:17 1:18 2:2 1:19 1:20 1:21
5:0 1:22 1:23 3:1 1:24 1:26 1:25 1:27 1:29
6:0 1:28 2:3 1:30 1:32 1:31 1:33 7:0 1:34
1:35 1:36 4:1 1:37 1:38 2:4 1:41 1:39 1:40
8:0 1:44 1:42 1:45 1:43 3:2 2:5 1:49 1:48
```

Set `index = next_index + run`, write the signed magnitude to
`coefficients[scan[index]]`, then set `next_index = index + 1`.
The scan is the side's existing zigzag variant. Sign 1 negates the
magnitude, and the native halfword store wraps to signed 16 bits.

A canonical positive-magnitude escape is
`M = (magnitude + 1)*side*side + run + 1`. Low escapes have observable
noncanonical values:

- For side 8, M = 64 produces magnitude −1 and run 63; M = 65–128
  produces magnitude zero. Positive magnitude 1 begins at M = 129.
- For side 16, M = 64–256 produces magnitude −1; M = 257–512 produces
  magnitude zero. Positive magnitude 1 begins at M = 513.

The sign bit still negates those values. This is acceptance behavior of
constructed inputs, not evidence that the native encoder emits them.

The reader returns flag 1 when K is one and the last logical scan index
is zero; otherwise it returns 7. Uncoded partitions have flag 0 and
consume no token stream. These flags depend on token positions, even when
the resulting coefficient value is zero.

## Native bounds do not enforce the partition extent

Unlike the previously recovered intra reader, `0x6bb5c` has no explicit
K-versus-area check. Two isolated streams with `K = side*side + 1`
were accepted when the extra scan entry was mapped back to zero in the
test scan buffer. The independent reader rejects both counts.

At `0x6bc04`, native code computes `limit = 256 - signed16(offset)`.
At `0x6bcb0`, it loads a scan value using the accumulated scan index
before checking `mapped_index <= limit` at `0x6bcb4`–`0x6bcb8`.
It does not first bound the accumulated index to the partition area.

Twenty isolated scan-table probes used offsets 0, 64, 128 and 192.
Each tested zero, `limit - 1`, `limit`, `limit + 1` and 65535. Native
accepted the first three and returned −202 for the last two. In
particular, offset zero accepts mapped index 256, one element beyond
a 256-element coefficient region. These probes used deliberately enlarged
test storage and substituted scan entries; they are not claims about
ordinary valid encoded partitions.

The independent reader requires both `K <= side*side` and every
accumulated scan index below `side*side` before reading the scan. It
does not reproduce the native out-of-partition access.

## Validation and remaining work

All 11040 isolated coefficient streams matched native coefficients,
flags and consumed bits at all eight initial bit alignments:

- 2016 compact-token cases cover both signs of all 63 entries at both sides.
- 768 escape cases cover both signs, three run positions and magnitudes
  through 65536, including signed-16 wrapping.
- 8224 low-escape cases cover every M from 64 through twice the area.
- Sixteen dense arrays and sixteen mixed compact/escape arrays cover
  repeated token consumption and scan advancement.

These checks executed 206 distinct native instructions without imported
functions. Complete temporal sequence comparisons additionally validate
the mask tables and coefficient arrays in their decoder context. APK/ELF
identity, numeric tables and cited instruction bytes were verified.

[Reduced temporal decoding](spi-reduced-temporal-findings.md) separately
validates bundled primary/secondary residuals and their reconstruction.
Primary submode 3, broader malformed-input policy and device-export
compatibility remain open. Scratch tools and
generated streams remain disposable; maintained findings are Markdown-only.
