# SPI intra prediction beside temporal blocks

## Evidence and scope

Recovered from Samsung Notes 4.4.45.37 ARM64 `libSPenBase.so` in the
[identified APK](README.md#sources-and-validation). Intra blocks can
follow temporal blocks within the same image. Their external prediction
edges depend on four block-availability bytes, independently of the
prediction-mode marker grids used to read partition modes.

The independent sequence decoder now combines primary mode-3 submodes
0–3, alpha submodes 0–2, literals and temporal/spatial copy blocks. Tests
use wire color index 4, API output color 500, retained references, header
flags `0xa0`/`0xb0`, and cache capacities one or three. An initial
B-zero image precedes B-one images. Full-size and reduced color paths
are covered, including Q zero, 30 and 51. Alpha remains Q zero.

These are constructed codec sequences compared with the APK's native
decoder. They establish the tested prediction and state transitions,
not device-export compatibility or SDK support.

## Edge pixels come from the current image

For block column c, block row r and packet-group start row g:

```text
x = 16*c
y = 16*r
has_above = r > g
```

Primary entry `0x68470` identifies intra submode 1 at `0x68560`.
It gathers byte edges from the current destination descriptor, context
992, through calls `0x685ac`, `0x685dc` and `0x68608` to `0x6d5a8`.
Alpha submode 1 uses the same gatherer at `0x688c8`.

The gatherer's source pointer is the pixel immediately left of the
block's origin. For each plane, its raw arrays are:

| Array | Gathered samples |
| --- | --- |
| `A[0..32]`, when `has_above` | Pixels `(x-1,y-1)` through `(x+31,y-1)` |
| `L[0]` | `A[0]` when `has_above`, otherwise 128 |
| `L[1..16]` | Pixels `(x-1,y)` through `(x-1,y+15)` |

The gatherer leaves A untouched when no above row is permitted. It also
leaves `L[17..32]` for the completion helper. Border pixels come from
the [128-filled frame border](spi-alpha-state-findings.md#the-frame-has-a-border-filled-with-128).
The later helpers finish every required edge entry for the valid binary
availability patterns below.

Color byte arrays start at worker `9568 + 33*plane` for A and
`9700 + 33*plane` for L. Alpha uses plane index three, hence offsets
9667 and 9799. Completion is `0x6d690` for all three color planes,
called at `0x68620`, and `0x6e0b4` for alpha, called at `0x688e8`.
Neither completion helper loads a previous-image descriptor: unavailable
edges are synthesized from the gathered arrays or constants.

## Four availability bytes select the completion rule

With an above row permitted, the helpers combine neighboring bytes as:

```text
mask = left + 16*above + 256*above_right + 4096*above_left
```

The availability-row stride is `block_columns + 2`, including sentinel
bytes. The current-row pointer is worker 2472. The color helper loads
the neighbors at `0x6d6c4`–`0x6d6d0` and stores the mask at worker 4304
through `0x6d6e8`; alpha does so at `0x6e0e8`–`0x6e10c`.

For the tested configuration, intra blocks and spatial copies have
availability one. Temporal mode-3 submodes 0/2/3 and mode-0 front-cache
reuse have availability zero. Primary intra output sets one at
`0x5de30`; alpha intra completion sets it at `0x68918`. Marker contents
do not determine these availability bytes.

The same mask also supplies the previously recovered
[motion predictor](spi-temporal-block-findings.md#motion-prediction-uses-neighboring-block-vectors).
Thus mixing intra blocks into a temporal row affects later reference
motion as well as later intra edge completion.

### Complete rules for all sixteen binary masks

In this table, every right-hand-side value comes from the original
gathered arrays. `mean(a,b) = (a+b+1)//2`. A range assigned one value
is filled with that value; unlisted entries remain unchanged. The four
hexadecimal digits represent above-left, above-right, above and left,
in that order.

| Mask | Above-array updates | Left-array updates |
| --- | --- | --- |
| `0x0000` | `A[0..32]=128` | `L[0..32]=128` |
| `0x0001` | `A[0..32]=L[1]` | `L[17..32]=L[16]` |
| `0x0010` | `A[17..32]=A[16]` | `L[0..32]=A[1]` |
| `0x0011` | `A[0]=mean(A[1],L[1])`; `A[17..32]=A[16]` | `L[0]=mean(A[1],L[1])`; `L[17..32]=L[16]` |
| `0x0100` | `A[0..16]=A[17]` | `L[0..32]=A[17]` |
| `0x0101` | `A[0..16]=mean(A[17],L[1])` | `L[0]=mean(A[17],L[1])`; `L[17..32]=L[16]` |
| `0x0110` | `A[0]=A[1]` | `L[0..32]=A[1]` |
| `0x0111` | `A[0]=mean(A[1],L[1])` | `L[0]=mean(A[1],L[1])`; `L[17..32]=L[16]` |
| `0x1000` | `A[1..32]=A[0]` | `L[1..32]=L[0]` |
| `0x1001` | `A[1..32]=A[0]` | `L[17..32]=L[16]` |
| `0x1010` | `A[17..32]=A[16]` | `L[1..32]=L[0]` |
| `0x1011` | `A[17..32]=A[16]` | `L[17..32]=L[16]` |
| `0x1100` | `A[1..16]=mean(A[0],A[17])` | `L[1..32]=L[0]` |
| `0x1101` | `A[1..16]=mean(A[0],A[17])` | `L[17..32]=L[16]` |
| `0x1110` | Unchanged | `L[1..32]=L[0]` |
| `0x1111` | Unchanged | `L[17..32]=L[16]` |

The alpha dispatch tables contain one-byte instruction offsets at
`0x2afae`, `0x2afc0`, `0x2afd2` and `0x2afe4`. Their respective code
bases are `0x6e258`, `0x6e140`, `0x6e298` and `0x6e1f8`.
Color tables at `0x2af1e`, `0x2af42`, `0x2af66` and `0x2af8a` use
16-bit offsets with bases `0x6d928`, `0x6d71c`, `0x6d9a0` and
`0x6d868`. Entries select `base + 4*offset`; each table handles
low-digit indices 0, 1, 16 and 17. The independent model implements
the equations rather than indexing native tables.

An unavailable above-left block does not always remove its gathered
corner. For `0x0001`, L[0] remains the raw corner while A is filled
from L[1]. For `0x0010`, A[0] remains raw while L is filled from A[1].
These asymmetries match the earlier single-image edge rules and now
also occur beside temporal blocks. Uniformly replacing both corners
would change directional prediction.

### Group-first rows take a separate branch

When `has_above` is false, the helpers do not dispatch through the table
above. Color branches from `0x6d79c`; alpha branches from `0x6e17c`.
They follow:

| Position/availability | Completed edges | Stored mask |
| --- | --- | --- |
| `c == 0 && r == 0` | Both arrays entirely 128 | `0x1111` |
| Otherwise, left byte is zero | Both arrays entirely 128 | `0x1110` |
| Otherwise, left byte is nonzero | Preserve `L[0..16]`, extend with `L[16]`, fill all A with `L[1]` | `0x1111` |

The initial origin check uses absolute block row r, not `r-g`.
In valid constructed groups, the first column's left sentinel is zero.
Consequently the first block of a later group has 128-valued edges and
mask `0x1110`, even though decoded pixels exist in the preceding group.
These first-row mask values also have motion-prediction meanings; they
must not be interpreted as four independently available spatial neighbors.

## Edge completion precedes color conversion or reduction

Full-size color intra prediction converts the completed byte arrays
together through `0x5ea48`, called for above/left at `0x686c0` and
`0x686ec`. It produces the three signed working planes documented in
the [color-intra findings](spi-color-intra-findings.md). Completion uses
the original byte samples and rounded byte averages, before conversion.

For reduced color, primary edges stay full size. Secondary above and
left edges are filtered after completion through callbacks 1416/1424,
called at `0x68638`, `0x68648`, `0x68658` and `0x68668`. The
[reduced-color findings](spi-reduced-color-findings.md) specify those
filters and partition reconstruction. Alpha uses the completed byte
arrays directly.

Full-size color reconstruction remains in signed working buffers until
output conversion `0x5ed8c`, called at `0x5dfc8`. Reduced secondary
planes remain 8 by 8 until output upsampling at `0x5de94`/`0x5deb0`.
Comparisons observe these pre-output representations as well as the final
stored blocks; comparing them prematurely with displayed bytes would
conflate different stages.

## Temporal blocks reset prediction markers independently of availability

The [marker grids](spi-alpha-state-findings.md#prediction-markers-retain-two-sets-of-block-rows)
remain separate for color planes and alpha. Intra partition parsing
updates them with prediction modes 0–17. The tested non-intra blocks,
including temporal mode-3 submodes 0/2/3, reset their block's 4-by-4
marker-cell rectangle to mode 2. They can therefore have markers equal
to two while their block-availability byte remains zero.

Primary submode-0 marker reset calls the pass-selected callback at
`0x6a328`; temporal alpha uses `0x69a20`; selected-plane completion
uses `0x6a0a8`. The callback table selected through GOT entry `0xeeec0`
contains primary helper `0x5e92c` and alpha helper `0x5e9a0`. The primary
helper fills all three color marker planes when context byte 1024 is set,
as in these color-index-four comparisons.

These temporal callbacks use worker word 32 as their marker offset.
For section zero, `0x6d3f0` computes `pixel_x/4` at
`0x6d414`–`0x6d42c`. This is distinct from the unresolved alpha-literal
marker-coordinate behavior described in the earlier state findings.

The independent mixed decoder maintains markers across blocks, copies
current marker rows into the previous-row region at row completion, and
resets previous marker rows at each packet-group boundary. It also
maintains motion availability, rotating image descriptors and the
[per-block reference caches](spi-reference-cache-findings.md). Intra
outputs refresh the cache through `0x5dfdc` for primary color; selected
plane updates retain their different cache-write behavior.

## Validation and remaining work

The isolated completion suite uses 106 independent pairs of 33-byte
edge arrays: constants, ramps, deterministic random values, and an
impulse at every left/above entry. It compares the entire 13000-byte
worker region, including unrelated channels and guard bytes:

- 3392 color calls cover all sixteen masks, all patterns and two
  block-column positions. Each call checks all three planes.
- 3392 alpha calls cover the same combinations.
- 2544 group-first-row calls cover both helpers, initial/later absolute
  rows, three columns and both seeded left-availability values.

All 9328 calls match the independent rules. They execute 952 distinct
native instructions without imported functions.

The complete corpus contains 396 sequences, 1116 frames and 22608 block
operations. Its 288 two-frame cases construct every neighbor mask around
an interior intra block, for both sampling modes, Q zero/30/51, and
partition sides 4/8/16. The other 108 five-frame cases mix intra,
temporal, literal, cache-reuse and spatial-copy blocks at dimensions
1 by 33, 17 by 33 and 65 by 65, with one to three block rows per
packet group and cache capacities one/three.

All 1116 frames match native output. The B-one images contain 4314 intra
blocks, and every binary availability mask occurs in each pass. Their
17070 intra sections include 7344 side-4, 7344 side-8 and 2382 side-16
sections, exercising all eighteen prediction modes. The complete corpus
executes 16627 distinct native instructions.

The comparisons check independently decoded bit consumption, complete
visible pixels, both cache orders, the shared cache fill counter and
cached block pixels. For B-one blocks they also check complete marker
grids before and after decoding. Intra checks include the selected
availability mask, completed/converted external edges, signed full-size
color planes or reduced byte planes, and final stored blocks. Interleaved
temporal blocks retain their motion and residual checks.

Eight five-frame repeats use fresh allocations filled with `0xa5` or
`0xff`. Eight earlier full-size temporal, reduced temporal, selected-plane
and cache-selection sequences preserve their recorded output digests
across 46 frames after decoding through the extended mixed model.

The representative 65-by-65, two-block-row-group sequence with three
cache entries, Q 51 and reduced color has artifact SHA-256
`2fd5e98aae0cb9120c604db1ffd884e5c60dbeb4daa77552505e5cf4bc4b0794`.
Its fifth frame's visible output SHA-256 is
`7f5a793d903ea1ae45a2501b586f4a0dc9f3bf7586b84e6bba1c29f09b77907f`.
Artifacts use the same disposable length-prefixed sequence storage as
the earlier temporal experiments, not a recovered SDOCX wrapper layout.

The APK/ELF identity, 48 cited instruction words, 216 dispatch-table
bytes and both marker callbacks were verified against the native binary.
All 396 artifact digests and 1116 independently reconstructed frame
digests were checked after generation. Local documentation paths and
heading anchors also pass validation.

These findings close the ordinary binary-availability edge rules for the
tested intra/temporal mixtures. Remaining work includes alpha literal
marker behavior, non-binary availability values, broader malformed-input
limits, other color/header configurations and real-file compatibility.
Generated artifacts and scripts remain disposable. Maintained changes
are Markdown-only, and no SDK code changed.
