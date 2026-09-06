# SPI alpha residual coefficient coding

## Evidence and scope

Recovered from Samsung Notes 4.4.45.37 ARM64 `libSPenBase.so` in the
[identified APK](README.md#sources-and-validation). Addresses below refer
to that ELF. An independent scratch bit reader reproduced 352 coefficient
arrays from the [30 native-generated bitmaps](spi-codec-validation.md),
matching both native signed values and consumed bits.

This specifies one residual representation inside alpha mode 3, whose
[block prefix](spi-data-packet-findings.md#block-mode-prefixes-select-six-dispatch-entries)
is `0001`. It does not independently decode the preceding prediction and
partition fields or reconstruct final alpha pixels. Those stages still ran
in Samsung's native decoder during complete-image comparisons.

The complete-image tests used wire color index 4, header flags `0xe0` and
packet byte B zero. Only the coefficient branch with worker byte 56 zero
is covered. This is a partial specification, not general SPI decoding or
device-export compatibility.

## Alpha mode 3 selects the zero-selector coefficient path

The alpha routine at `0x68824` sets worker submode 49 to 1 without reading
submode bits when packet byte B is zero. Packet byte B equal to 1 instead
reads two bits at `0x68874`; that variant remains outside this scope.

Call `0x68884` invokes `0x6d438` with selector zero. Stores at `0x6d440`
and `0x6d444` set worker bytes 56 and 58 to zero. This selects the
coefficient branch at `0x6a89c` and skips the conditional `0x6abec`
processing before reconstruction in the traced alpha path.

Submode 1 reads a split flag into worker byte 2374 at `0x68eb8`. The
unsplit branch calls `0x6a70c` at `0x69464`; the split branch halves the
working side and calls it at `0x68ed8`, followed by further sections.
Within `0x6a70c`, byte 2374 controls another split bit, stored at 2375.
The resulting coefficient side is stored at worker byte 29 and its
base-2 logarithm at byte 31.

Before the residual bytes, `0x6a7bc` calls prediction-mode reader
`0x6b584`, and `0x6a7d8` calls coded-partition reader `0x6b704`. The
scratch trace takes their resulting state as input; these two readers
have not been independently reproduced here. A coded partition is
zero-filled at `0x6a858`, then populated with signed 16-bit coefficients.

## Scan order maps run positions into the coefficient array

Let `side` be 4, 8 or 16 and `area = side * side`. The scan variant is
zero when `side == 16` or the prediction mode is 17. Otherwise, the byte
table at `0x2a3e2` supplies the variant for prediction modes 0 through 17:

```text
1 2 0 0 1 1 0 2 2 0 0 1 1 0 0 2 2 0
```

The native scan pointer comes from the table at `0xf4630`, resolved
through GOT entry `0xeee58`, at byte offset
`log2(side) * 24 + variant * 8`. Its entries are unsigned 16-bit indexes
into the row-major coefficient array.

| Variant | Order |
| --- | --- |
| 0 | Alternating diagonals |
| 1 | Rows: indexes `0, 1, …, area - 1` |
| 2 | Columns: index `row * side + column`, with column as the outer loop |

For variant 0, enumerate diagonals `d = 0` through `2 * side - 2`.
Within each diagonal, rows range from `max(0, d - side + 1)` through
`min(side - 1, d)`, ascending for even `d` and descending for odd `d`.
The column is `d - row`. Thus the 4×4 sequence is:

```text
0 4 1 2 5 8 12 9 6 3 7 10 13 14 11 15
```

All nine static scan tables for sides 4, 8 and 16 were compared with
independently generated permutations. Side 16 always selects variant 0
in this branch; its other two tables were not exercised by payload tests.

## Nonzero count and signed run tokens

All fields consume bits most-significant first without byte alignment.
A positive-integer code contains `k` zeros, a set bit and `k` suffix bits:

```text
positive = 2^k + suffix
```

The array begins with positive nonzero count `N`. The native read starts
at `0x6a908`, forms the count at `0x6a9c0`, and rejects counts greater
than `area` at `0x6a9c8`. A coded array therefore has at least one token;
the preceding coded-partition state represents absent coefficient data.

Each of the `N` tokens contains another positive code `M`, followed by one
sign bit. For `M < 128`, the table at `0x2ad24` maps `M - 1` to a pair
`(magnitude, zero_run)`. These are its 127 pairs, in increasing `M` order:

```text
(1,0) (1,1) (2,0) (1,2) (3,0) (1,3) (4,0) (1,4)
(2,1) (1,5) (5,0) (1,6) (6,0) (1,7) (3,1) (2,2)
(1,8) (7,0) (1,9) (8,0) (1,10) (9,0) (4,1) (1,11)
(2,3) (1,12) (10,0) (1,13) (3,2) (11,0) (1,14) (5,1)
(2,4) (12,0) (1,15) (6,1) (13,0) (2,5) (1,16) (14,0)
(1,17) (4,2) (3,3) (7,1) (15,0) (1,18) (2,6) (1,20)
(1,19) (16,0) (1,21) (8,1) (17,0) (1,24) (2,7) (1,22)
(5,2) (1,25) (3,4) (18,0) (9,1) (1,23) (2,8) (19,0)
(4,3) (10,1) (1,26) (20,0) (6,2) (2,9) (21,0) (3,5)
(11,1) (1,27) (22,0) (2,10) (7,2) (1,31) (12,1) (1,30)
(23,0) (1,28) (1,29) (5,3) (4,4) (1,32) (2,11) (24,0)
(13,1) (8,2) (3,6) (25,0) (2,13) (2,12) (14,1) (26,0)
(1,34) (1,33) (6,3) (15,1) (27,0) (4,5) (1,35) (2,14)
(9,2) (5,4) (28,0) (3,7) (16,1) (29,0) (10,2) (17,1)
(30,0) (7,3) (3,8) (31,0) (18,1) (1,36) (11,2) (32,0)
(1,37) (3,9) (33,0) (2,15) (19,1) (6,4) (34,0)
```

For `M >= 128`, the escape calculation at `0x6ab08` through `0x6ab1c` is:

```text
escaped = M - 128
zero_run = escaped % area
magnitude = floor(escaped / area) + 1
```

Compact and escape forms can represent the same pair. For example,
`M = 1` and `M = 128` both represent magnitude 1 with no leading zero.

Initialize `next_scan_index = 0`. For each token:

```text
scan_index = next_scan_index + zero_run
coefficient_index = scan[scan_index]
coefficients[coefficient_index] = magnitude if sign == 0 else -magnitude
next_scan_index = scan_index + 1
```

Unwritten entries remain zero. The native store is at `0x6ab4c`.
After the last token, `0x6aba0` stores partition metadata 1 if `N == 1`
and the final scan index is zero, or 7 otherwise. This metadata has been
checked without assigning a broader transform meaning to those values.

## Validation and bounds

The APK digest, extracted ELF identity, cited instruction bytes, compact
pair table, prediction-to-scan table and scan-pointer bindings were checked.

- All 352 coded arrays across the 30 generated images matched native
  signed coefficients and exact bit consumption: 182 arrays of side 4
  and 170 of side 8. Scan variants 0, 1 and 2 appeared 166, 90 and 96
  times respectively, with all eight starting bit positions represented.
- Their 8310 tokens comprised 3286 compact forms and 5024 escape forms.
  Both signs occurred; observed magnitudes reached 255 and zero runs 56.
  Final images still matched every input pixel through the native decoder.
- 3824 isolated native coefficient reads matched independently assembled
  payloads and partition metadata: 2032 compact-table cases, 1680 escape
  cases, 56 fully populated arrays and 56 mixed-token arrays.
- The isolated checks exercised every compact pair, both signs, all eight
  starting positions, sides 4/8/16, each reachable scan variant, the final
  scan position, and selected magnitudes 1, 2, 34, 255 and 32767.
- 24 isolated cases with count `area + 1`, spanning the three sides and
  all starting positions, reached native status `-202` and were rejected
  by the independent reader.

The isolated reads entered `0x6a89c` with prepared worker/reader state and
stopped after metadata storage, before subsequent partition processing.
They executed 331 distinct native instructions without imported host
functions. They validate the coefficient branch, not preceding field
parsing or the validity of those constructed arrays as complete images.

The scratch reader requires each scan index to be below `area` and each
magnitude to fit a positive signed 16-bit value. Native code at `0x6ab28`
instead reads the scan entry first, then checks that the resulting index
is at most 255 at `0x6ab2c`. This is not equivalent bounds checking.
Malformed runs, oversized magnitudes and truncated bitstreams have not
been matched against native behavior.

## Remaining work

Independently decode prediction modes and coded-partition masks, then
reconstruct neighboring-edge prediction and coefficient combination in
`0x6ce0c`. Other mode-3 submodes, nonzero selector paths and primary-pass
compressed modes remain unresolved. Device-exported SPI files and visual
references remain necessary for compatibility validation.

Only Markdown findings are maintained. Scratch readers, native harnesses
and generated artifacts are disposable local tooling. No SDK code changed.
