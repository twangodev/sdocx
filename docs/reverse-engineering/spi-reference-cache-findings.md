# SPI reference-cache selection and temporal copy blocks

## Evidence and scope

Recovered from Samsung Notes 4.4.45.37 ARM64 `libSPenBase.so` in the
[identified APK](README.md#sources-and-validation). Header byte 18 gives
the number of cached images. Each block has separate color and alpha
rank orders, but those orders share one fill counter. Modes 0 and 1 can
reuse these cached blocks when packet byte B is nonzero.

Independent sequence comparisons use wire color index 4, API output
color 500, retention enabled, and cache capacities 1–5. The main corpus
uses header flags `0xa0`/`0xb0`, packet B zero for the initial image,
and B one thereafter. Additional comparisons cover `0x80`/`0x90` and
resetting the cache order with a later B-zero image. This extends the
[temporal decoder](spi-temporal-block-findings.md); it does not establish
device-export compatibility or SDK support.

## Cache ranks are stored per block and per pass

Header reader `0x67e8c` obtains byte 18, which becomes context byte 108.
With header flag B clear, allocation loop `0x5cf24`–`0x5cf6c` creates
that many cached image descriptors. These are additional to the rotating
destination and previous-image descriptors at context pointers 992/1000.

For block ordinal `i = row*block_columns + column`, the relevant layout is:

| Context offset | Meaning |
| --- | --- |
| `1048 + 8*r` | Pointer to the color rank-r byte array; entry i is a physical cache index |
| `1088 + 8*r` | Corresponding alpha rank-r array |
| 1128 | Pointer to the shared per-block fill-count byte array |
| `1136 + 8*s` | Cached image descriptor for physical slot s |

The 40-byte spacing between rank-array families leaves five pointer
positions per pass. Capacities 1–5 are the tested scope; this layout is
not evidence that the native header reader validates an upper bound of
five. Zero and larger capacities are outside the independent decoder.

Initialization `0x60478` fills rank r's array with r at `0x6051c` and
`0x60548`, and clears the shared count array at `0x60574`. Cached pixel
planes initially contain 128 in the tested configuration. The block loop
stores the ordinal at worker byte 18 through `0x5c4f4` and selects the
pass's rank-array family at worker pointer 2392 through `0x5c47c`.

For each B-zero block, `0x5c518`–`0x5c53c` resets the current pass's
order to identity. Store `0x5c54c` sets the shared count to one. This
happens separately during color and alpha processing. It does not clear
the other physical cache images' pixel planes. A subsequent output write
updates the selected front slot's channels.

## Ordinary writes and explicit selections move a rank to the front

Helper `0x6e728` updates cache order for the current block/pass. It returns
immediately for packet B zero. For B nonzero, let N be capacity, U the
shared count, and O the selected pass's rank order. Within `0 <= U <= N`:

| Operation | Rank moved to the front | New shared count |
| --- | --- | --- |
| Ordinary refresh, `U < N` | U | `U + 1` |
| Ordinary refresh, `U == N` | `N - 1` | U |
| Explicit selection R, `0 <= R < N` | R | `max(U, R + 1)` |

Moving rank r to the front preserves the relative order of all other
entries. The native helper implements adjacent swaps: full ordinary
refresh at `0x6e894`–`0x6e8c0`, partial refresh at
`0x6e8dc`–`0x6e900`, and explicit selection at `0x6e928`–`0x6e94c`.
An ordinary refresh with U zero leaves the order unchanged and sets U to
one through `0x6e970`–`0x6e978`.

The count belongs to the block, not to either pass. For example, start
with N three, both orders `[0,1,2]`, and U one after an initial image.
An ordinary color refresh chooses slot 1, produces color order `[1,0,2]`,
and sets U two. An ordinary alpha refresh then chooses slot 2, produces
alpha order `[2,0,1]`, and sets U three. These are different cache choices
within the same block position. U alone cannot establish that every rank
has previously received pixels for both passes.

When explicit R is at least U, the helper's fill loop increments U but
copies the existing front slot to itself. Both descriptor selections at
`0x6e79c` and `0x6e84c` use the same front rank. It does not populate the
newly selected physical slot from the front image. Thus selecting an
unwritten rank can produce the initialized 128-valued block even though
the shared count has advanced past that rank.

Ordinary primary output calls the helper at `0x5dd14`, then writes its
three color planes to the new front cache slot and current image. Alpha
output similarly calls it at `0x5e338`. In contrast,
[selected-plane submode 3](spi-selected-plane-findings.md#output-updates-one-cache-plane-and-copies-two-reference-planes)
does not rotate ranks or advance U. It updates only the selected plane
of the existing front color slot; its other output color planes come
from the previous completed image.

## Modes 0 and 1 have different syntax when B is nonzero

Mode-prefix `1` selects mode 0. Entry `0x67fcc` consumes no further
payload for B nonzero. Output copies the current front cache block at
the same coordinates, without calling the order helper. This differs
from its [B-zero spatial-copy meaning](spi-copy-block-findings.md).

Mode-prefix `01` selects mode 1. Its field reader at `0x68054` follows:

1. If N is greater than one, read an explicit-selection bit at
   `0x6809c`. Otherwise explicit selection is implicitly false and
   this bit is absent.
2. If explicit, read a positive integer R through `0x680b4`. R is the
   rank directly; there is no subtraction by one. Store its low byte
   at worker 2494, with explicit flag 2495 set. Valid constructed
   sequences select ranks 1 through `N-1`; mode 0 supplies front reuse.
3. Otherwise decode a source/displacement branch as described below.

An explicit selection advances the rank order before output copies the
new front slot's block. It does not replace cached pixels. Primary and
alpha copy outputs call the order helper at `0x5dabc` and `0x5e1bc`,
respectively, when handling mode 1 in the retained configuration.

### Header flag C controls the non-explicit displacement grammar

Header flag C, bit 5 of byte 19, is context byte 112. For non-explicit
mode 1 with B nonzero, `0x68114` checks it:

| Flag C | Next bit | Displacement branch |
| --- | --- | --- |
| Clear | Absent | Spatial branch |
| Set | 1 | Spatial branch |
| Set | 0 | Pixel-offset vertical branch |

For the vertical branch, read positive integer V and use:

```text
signed_code(1) = 0
signed_code(2k) = k
signed_code(2k+1) = -k
dx = 0
dy = signed_code(V)
source = (x - dx, y - dy)
```

This dy is in pixels. The field reader stores zero horizontal displacement
at `0x68138` and the decoded vertical displacement at `0x68204`.

The spatial branch instead reads one of these prefixes:

| Spatial prefix | Further fields | `(dx,dy)` in pixels |
| --- | --- | --- |
| `1` | None | `(16,0)` |
| `01` | None | `(0,16)` |
| `00` | Positive integers H, V | `(16*signed_code(H), 16*(V-1))` |

These paths begin at `0x68180`, `0x6819c` and `0x681b0`. Spatial copies
set the block's motion-availability byte to one at `0x6817c`; the
vertical branch leaves the initial zero in the tested header configuration.
Constructed sequences interleave these copies with motion-coded blocks.

## Alpha's vertical branch reads the current destination image

Primary mode-1 output checks source-branch byte 2496 at `0x5db44`.
The spatial branch reads the current image, context 992. The vertical
branch loads the previous completed image, context 1000, at `0x5db50`.
It first copies the source region into the chosen cache slot at
`0x5dba0`, `0x5dbc0` and `0x5dbe0`, then into the current image.

Alpha mode-1 output does not inspect byte 2496. Both branches read the
current destination alpha plane loaded at `0x5e1d4`. That destination
can still contain pixels from two images earlier because the completed
image and destination descriptors rotate. It is distinct from the
previous completed image used by the primary vertical branch.

Alpha computes the linear source offset at `0x5e26c`. Its local check
rejects a negative offset or one at least `visible_height*stride`, with
`-1999` at `0x5e294`. It does not separately validate X/Y or the full
16-by-16 region. The B-nonzero primary path bypasses the separate
B-zero origin checks. These are native checks, not a safe input policy
for a future SDK decoder.

Alpha first copies the source into the cache at `0x5e2d0`, then copies
that source into the current destination at `0x5e2e8`. Callback 1584,
`0x5fb74`, loads and stores one 16-byte row before advancing to the
next row. With an overlapping positive dy below 16, later source rows
therefore read earlier destination writes. The current output can differ
from the immutable source block already saved in the cache.

For example, dy one at block y 16 saves original rows 15–30 into the
cache, but repeats original row 15 across destination rows 16–31.
Sixty-two isolated alpha-output comparisons cover dy −15 through 15
with both source-branch byte values and independently seeded row/column
gradients. All match sequential row copying; the 30 positive-dy cases
produce different cached and displayed blocks. Both branch-byte values
produce identical alpha results.

## A rank equal to capacity reaches an ignored helper error

The explicit field check at `0x680c8`–`0x680d0` compares N with the
signed low byte of R and accepts equality. For R equal to N, the
cache helper advances U to N, then fails the physical-rank bound at
`0x6e834` and returns `-202`. It leaves the rank order and cached
pixels unchanged.

Neither copy-output caller checks that helper return. They continue
using the existing front entry and eventually return success. Twenty-four
three-frame comparisons cover N 2–5, each pass separately, and allocation
fills zero, `0xa5` and `0xff`. Each outer decode succeeds despite the
traced helper error, preserves the initial front pixels, and retains
them on a following mode-0 frame.

This is a measured error-propagation discrepancy. The bounded independent
rank decoder rejects R at or above N. The signed-byte comparison also
requires separate analysis for large positive codes that wrap or become
negative; those values are outside these whole-sequence comparisons.

## Validation and remaining work

The main corpus contains 315 independently assembled sequences and 2835
frames, with 76140 block operations. Capacities 1–5 each use three
families: ordinary writes before cache reads, early selection of an
unwritten rank, and different refresh/reuse patterns for color and alpha.
Dimensions are 16 by 16, 17 by 33 and 65 by 65, with one to three block
rows per packet where valid. Primary reconstruction mixes literal,
full-size temporal, reduced temporal and selected-plane payloads at
Q zero, 30 and 51; alpha mixes intra initialization, temporal residuals
and cache/spatial copies.

Comparisons check consumed block bits, complete visible output, and all
four cached 16-by-16 plane regions in every physical slot at the current
block position, before and after each block. They also check both pass
orders and the shared fill count. Earlier
motion, coefficient and transformed-block checks remain active for the
interleaved temporal payloads. All frames and intermediate comparisons
match; the corpus executes 8119 distinct native instructions. It includes
17059 mode-0 front reuses, 8653 explicit-rank selections, 15298 vertical
copies, 7062 spatial copies and 28068 initialization/refresh blocks.

Isolated cache-helper comparisons cover all permutations of capacities
1–5, all U values zero through N, both passes, ordinary refresh and
every valid explicit rank: 10076 transitions. Guard bytes in the other
block positions and all seeded cached pixels remain unchanged. Another
40 cases cover R equal to capacity and 20 cover B-zero no-ops. The
helper suite, including decoder/header initialization, executes 1817
distinct native instructions with only allocation and memory-fill imports.

Additional successful sequence checks comprise:

- Six pairs with flag C clear/set, 60 frames, covering all three spatial
  prefixes, signed horizontal offsets and both secondary sampling flags.
- Two sequences containing a later B-zero image, 21 frames, verifying
  order resets while preserving the other cache slots' pixels.
- Eight allocation-fill repeats, 76 frames, using `0xa5`/`0xff` with
  capacities two/five and early or asymmetric cache selections.
- Six earlier full-size, reduced and selected-plane sequences, 24 frames,
  preserving their recorded artifact and output digests.

The representative five-entry asymmetric sequence uses dimensions
65 by 65, two block rows per packet, and reduced Q-51 refreshes. Its
artifact SHA-256 is
`817408e0ee0e2f6b8c01f65bb60b4e2f499fd95c99aba82c5259ace184addac1`;
its eleventh frame's visible output SHA-256 is
`dd78a18a3dc5bb8dab7b796b3919f0af0eacad2de59dd65122ac6e6000c190f6`.
The disposable artifact stores a little-endian length before the header
and each submitted image-data block. It is test storage, not evidence
that the SDOCX bitmap wrapper emits this multi-frame layout.

All 315 artifact digests and 2835 independently reconstructed frame
digests were checked again after generation. The APK digest, extracted
ELF, 52 cited instruction words, cache layout and copy callback binding
were checked, as were 771 local documentation links and 176 heading
anchors. Generated sequences, native harnesses
and independent decoder scripts remain disposable local artifacts;
maintained changes are Markdown-only.

The [mixed-prediction trace](spi-mixed-prediction-findings.md) adds ordinary
intra/temporal spatial-edge completion and marker transitions. Remaining
work includes alpha literal marker behavior, capacities outside 1–5,
wrapped rank codes, larger displacement limits and a complete malformed-input
policy. These experiments do not establish which temporal/cache configurations Samsung
actually emits in device-exported SDOCX files. Real files and rendered
references remain necessary for that compatibility claim. No SDK code changed.
