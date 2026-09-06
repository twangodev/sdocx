# SPI alpha prediction and partition fields

## Evidence and scope

Recovered from Samsung Notes 4.4.45.37 ARM64 `libSPenBase.so` in the
[identified APK](README.md#sources-and-validation). These findings extend
the [coefficient representation](spi-alpha-residual-findings.md) into an
independent reader for complete mode-3 alpha payloads, given the starting
prediction-marker state.

The reader matched all 86 mode-3 alpha blocks in the
[30 native-generated images](spi-codec-validation.md), including their
prediction modes, partition masks, coefficients, marker writes and exact
bit consumption. Another 98 constructed images exercised mask values and
the unsplit 16×16 path absent from that original sample set.

Scope remains wire color index 4, header flags `0xe0`, packet byte B zero,
implicit alpha submode 1 and worker selector byte 56 zero. The initial
neighbor-state setup and final pixel reconstruction still run in Samsung's
decoder. Complete independent image decoding and device-file validation
remain open.

## The block divides into one or four sections

After mode prefix `0001`, read one bit `divided`. The native dispatch
stores it at worker byte 2374 at `0x68eb8`.

| `divided` | Sections | Section side | Section order |
| --- | --- | --- | --- |
| 0 | 1 | 16 | Whole block |
| 1 | 4 | 8 | Top-left, top-right, bottom-left, bottom-right |

With `divided == 1`, each section begins with another bit `split`. With
`divided == 0`, `split` is implicitly zero and consumes no bit. The read
at `0x6a750` and store at `0x6a764` implement this selection. The partition
side and count are:

```text
partition_side = section_side >> split
partition_count = 4 if split == 1 else 1
```

A split section has four 4×4 partitions in the same quadrant order. An
unsplit section has one 8×8 or 16×16 partition. Native stores at
`0x6a778` and calculation at `0x6a790` establish these values.

For each section, read all partition prediction modes, then one coded
mask, then the coefficient arrays selected by that mask. Calls at
`0x6a7bc` and `0x6a7d8` select prediction reader `0x6b584` and mask reader
`0x6b704`. No byte alignment occurs between these fields or sections.

## Prediction modes use the left and above markers

Each marker represents a 4×4 pixel cell. The alpha marker-plane pointer
is at worker offset 2424 and its row stride at unsigned 16-bit offset
2464. The coordinate helper `0x6d3f0` computes four candidate offsets
at worker offsets 32, 36, 40 and 44.

For section index `s`, partition index `p`, block pixel X `x`, marker
stride `stride` and section side `base`, the offset relative to that
marker-plane pointer is:

```text
section_origin = floor(x / 4) + (s % 2) * 2 + floor(s / 2) * 2 * stride
partition_origin = section_origin
                 + (p % 2) * (base / 8)
                 + floor(p / 2) * (base / 8) * stride
```

The pointer already refers to the current block-row working region;
this formula does not add the document pixel Y. For an unsplit section,
only partition zero is used.

Reader `0x6b584` loads the left marker at `partition_origin - 1` and
the above marker at `partition_origin - stride`, at `0x6b5cc` and
`0x6b5d0`. In the supported marker domain 0–17, normalize either neighbor
value 17 to 2 for neighbor selection. Let the normalized values be `L`
and `A`.

| Prefix | Remaining bits | Result |
| --- | --- | --- |
| `1`, with `L == A` | None | `L` |
| `1`, with `L != A` | One selector bit | 0 selects `L`; 1 selects `A` |
| `01` | One selector bit | 0 selects mode 2; 1 selects mode 17 |
| `00` | Four-bit index | Index into ascending modes excluding `L` and 2 |

For the four-bit branch, build candidates from integers 0 through 17,
omitting `L` and 2, then select the indexed entry. When `L == 2`, only
one distinct value is omitted: there are 17 candidates but the four-bit
index reaches only the first 16. Mode 17 remains available through `011`.
The native conditional increments at `0x6b644` through `0x6b664` match
this rule. The above marker does not affect this branch.

The decoded mode is written into worker bytes 51–54. Split sections
write one byte per partition at `0x6b678`; unsplit sections replicate
partition zero's mode across all four bytes at `0x6b6fc`. The marker
plane is updated immediately: one cell for a 4×4 partition, a 2×2 cell
region for an unsplit 8×8 section, or a 4×4 region for an unsplit 16×16
section. Later partitions and sections therefore see earlier writes.

## A table maps the coded mask

For side 4, read the same positive-integer code used by the
[residual reader](spi-alpha-residual-findings.md#nonzero-count-and-signed-run-tokens).
Subtract one to obtain index 0–63. Worker selector zero chooses the first
64-byte row at `0x2aa9c`, through selector table `0x29214`. Its values in
index order are:

```text
63 60 62 61 28 52 44 56 31 47 55 59 12 48 30 54
46 58 20 29 53 45 40 57 51 15 0 16 4 14 50 32
8 13 49 36 24 22 43 23 42 21 41 38 34 26 18 6
39 10 2 33 17 27 5 37 35 1 25 9 19 7 11 3
```

For sides 8 and 16, read only a unary code: `k` zeros followed by a set
bit, with no suffix. Use `k` as the index into `0x2abdc`:

```text
k:     0 1 2 3 4 5 6 7
mask:  4 0 6 7 5 2 1 3
```

The native reader stores the resulting byte at worker offset 2376 at
`0x6b878`. The side-4 index check at `0x6b7e8` rejects a code above 64;
the other branch's check at `0x6b85c` rejects a unary count above 7.
Both return `-202` through `0x6b880` for the tested oversized codes.

For partition count `P` and zero-based partition `p`, its coefficient
array is present when mask bit `P + 1 - p` is set. Thus a single
partition uses bit 2, while four partitions use bits 5, 4, 3 and 2.
Instructions `0x6a840` through `0x6a848` perform that test.

A present array uses the prediction-dependent scan and signed run tokens
already specified in the residual findings. An absent array consumes no
coefficient bits and sets its metadata byte to zero at `0x6a828`.
Mask bits 0 and 1 do not select coefficient arrays in this reader; no
broader semantic meaning is assigned to them here. All mask values were
accepted in the constructed full-image checks.

## Validation

The APK digest, ELF identity, cited instructions, mask tables and selector
binding were checked. Native comparisons included:

- 86 complete mode-3 payloads from 30 encoded images: 344 sections,
  518 prediction modes and 352 coded coefficient arrays. All used the
  divided-block path; 58 sections split into 4×4 partitions and 286
  remained 8×8. Exact coefficient values, metadata, marker state and
  section/block end-bit positions matched the independent reader.
- 7376 isolated prediction reads covered all 18 output modes, all
  left/above mode combinations for neighbor selection, every four-bit
  index for every left mode, dedicated modes 2/17, all eight starting
  bit positions and marker writes for sides 4/8/16.
- 640 isolated mask reads covered every table entry at all eight starting
  positions. Another 24 oversized-code cases returned native `-202` and
  were rejected by the independent reader.
- 96 coordinate-helper cases checked four section indexes, four block
  X positions, three marker strides and both section sizes. These
  validate the offset calculation, not the enclosing buffer dimensions.
- 98 independently assembled images passed the native decoder and
  independent payload trace. They cover all 64 side-4 masks, all eight
  side-8/16 masks, unsplit 16×16 blocks and sizes 1×1, 16×16, 17×17 and
  31×33. Their 494 sections comprise 344 of side 4, 120 of side 8 and
  30 of side 16.

The isolated field checks executed 356 distinct native instructions.
Their only host import was `memset`, used to fill marker regions.
The 98 constructed images exercised 4639 distinct native instructions.

For complete payload traces, the initial marker bytes were captured after
native neighbor preparation. The independent reader then consumed the
whole payload without taking subsequent prediction, mask or coefficient
values from native execution; those native values were comparison targets.
Final pixels remained native-produced. The original 30 images retained
exact pixel round trips; the 98 new images establish payload acceptance
and field agreement, not an independent pixel-rendering result.

## Remaining work

Recover marker initialization and block-row transitions independently,
then specify neighbor-edge preparation, prediction pixels and residual
combination in `0x6ce0c`. Other submodes, selector values, invalid neighbor
states and general truncated-payload behavior remain unresolved. No SDK
code changed; maintained results are Markdown-only, with scratch tooling
and generated cases remaining disposable local artifacts.
