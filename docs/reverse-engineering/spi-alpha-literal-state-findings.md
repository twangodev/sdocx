# SPI alpha literal marker state

## Evidence and scope

Recovered from Samsung Notes 4.4.45.37 ARM64 `libSPenBase.so` in the
[identified APK](README.md#sources-and-validation). Alpha mode 5 reads
literal pixels correctly but supplies a pixel coordinate to a helper
that uses marker-cell offsets. This can change later prediction even
when every store remains inside the marker allocation. Other positions
write beyond that allocation and can corrupt availability state.

These findings extend the [literal pixel layout](spi-literal-block-findings.md),
[alpha marker layout](spi-alpha-state-findings.md) and
[mixed prediction rules](spi-mixed-prediction-findings.md). Constructed
sequences use wire color index 4, API output color 500, retained
references, header flags `0xa0`/`0xb0`, cache capacities one/three,
and an initial B-zero image followed by B-one images. Color uses either
sampling mode and Q zero/30/51; alpha remains Q zero.

Native execution occurs under Unicorn with controlled allocations.
The independent decoder reproduces marker writes that fit the requested
buffer and rejects those that cross its end. This is a bounded research
model, not SDK support or a device-export compatibility claim.

## The literal callback uses pixel X without scaling

Alpha mode-5 entry `0x68950` aligns the bitreader at `0x6896c`–`0x6897c`
and reads one 16-by-16 byte plane through `0x6a0c8`, called at
`0x68988`. It sets the current block's availability byte to one at
`0x6899c`, then calls marker helper `0x5e9a0` at `0x689ac`:

| Argument | Source |
| --- | --- |
| Marker structure | Worker offset 2400 |
| Row stride | Unsigned halfword at worker 2464, loaded at `0x689a0` |
| Marker offset | Pixel X at worker 24, loaded directly at `0x689a4` |
| Fourth argument | Context byte 1024, loaded at `0x689a8`; ignored by this helper |

The helper loads the working pointer from structure offset 24, adds the
supplied offset at `0x5e9ac`, and writes `0x02020202` at four consecutive
row strides. Its stores are `0x5e9b8`, `0x5e9bc`, `0x5e9c0` and
`0x5e9c4`; it returns at `0x5e9c8`. It performs no bounds check and
does not inspect its fourth argument. The literal entry returns success
at `0x689b0`, without a packet-B or header-flag condition around the reset.

Ordinary non-intra resets instead use worker word 32. For section zero,
`0x6d3f0` computes that offset as `pixel_x/4` at
`0x6d414`–`0x6d42c`. Thus the literal callback's offset is four times
the ordinary offset at every nonzero block column.

## Exact write footprint and allocation boundary

Let C be the number of 16-pixel block columns and c the current zero-based
column, with `0 <= c < C`. Offsets below are relative to the requested
alpha marker allocation, and intervals exclude their upper endpoint:

```text
S = 4*C + 1
capacity = 9*S
O = 5*S + 1
pixel_x = 16*c

write(k) = [O + 16*c + k*S, O + 16*c + k*S + 4), k = 0..3
largest_end = 8*S + 16*c + 5
all_writes_fit = 4*c + 1 <= C
```

For the last block column, the final store ends `12*C - 12` bytes past
the requested allocation's end. This is a distance to the last store's
end, not a count of distinct bytes written outside the buffer.

When all writes fit, they reset the marker rectangle belonging to block
column `4*c`, rather than column c. Column zero behaves like an ordinary
reset. At later fitting columns, the current block's marker cells remain
stale and a later block's cells become mode 2. When writes do not fit,
some can also cross marker-row boundaries before reaching the allocation
boundary. Increasing allocation capacity alone does not correct their
meaning.

Isolated helper validation covers every column for C one through 64,
plus five boundary/representative columns for C 128, 256, 512, 1024 and
2048. Both fourth-argument values zero and one are tested against
independently seeded complete buffers and guards. All 4210 calls match
the four-store rule: 1118 stay within the requested capacity and 3092
cross it. The helper executes eleven distinct native instructions and
no imported functions. Large-C cases exercise the pointer helper, not
complete images, inside an oversized emulator buffer.

## Stale markers change prediction without crossing the allocation

A two-frame, 65-by-33 case has C five, stride 21, origin 106 and capacity
189. Its alpha literal at pixel `(16,16)`, column one, writes four-byte
ranges starting at offsets 122, 143, 164 and 185. The last range ends
exactly at the allocation boundary. These are column four's markers;
the literal leaves column one's markers unchanged.

The preceding block row established mode 6 at column one and mode 8 at
column two. The following intra block at `(32,16)` uses prediction prefix
`10`, which reuses its left marker. Native decoding therefore selects
mode 6. A hypothetical model that changes the literal offset to
`pixel_x/4` instead selects mode 2. Both interpretations consume the
same number of bits through that block and complete the packet, but
their second frames differ at 561 visible alpha pixels. All three color
channels remain identical.

Six native two-frame comparisons cover both sampling flags and fresh
allocation fills zero, `0xa5` and `0xff`. Every native result matches
the independent unscaled-offset model, including prediction selection,
marker grids, cache state and pixels. The native second-frame digest is
`47679405f241a3881620b9c230d22e6b2ef369baebb27a994bcd8542e23c2358`;
the hypothetical scaled-offset result is
`72c55ce5c1e7268dcf32244becb2ff4fc72c4e5b5b83e89b467a3cb203b3ebc1`.

This comparison establishes why silently scaling the offset would not
reproduce the tested decoder. It does not establish the encoder's
intended behavior or prescribe a compatibility policy for all SPI files.

## Writes beyond the buffer can corrupt availability

The alpha marker allocation is stored at worker 2456 after `malloc`
at `0x5ce60`, filled with two at `0x5ce74`, and assigned its working
pointer at worker 2424 through `0x5ce80`. The next allocation requests
`2*C + 4` availability bytes at `0x5ce9c` and stores its base at worker
2480 through `0x5cea0`. Initialization clears it at `0x5cd60` and
sets worker 2472 to `availability_base + C + 3` at `0x5cd6c`.

Whether a marker overrun reaches that next allocation depends on the
allocator. Two emulator layouts make the effect observable: one rounds
each request to 32 bytes; the other also leaves 4096 unused bytes after
every allocation. The input bytes and native decoder are unchanged.

The probes place an alpha literal in the last column of the first block
row, followed by an intra block using prediction mode 6 in that column
of the next row. Both images are 48 pixels high:

| C / width | Marker capacity | Compact availability offset | Literal store offsets | Availability bytes touched by marker stores |
| --- | --- | --- | --- | --- |
| 3 / 48 | 117 | 128 | 98, 111, 124, 137 | 9 |
| 4 / 64 | 153 | 160 | 134, 151, 168, 185 | 8, 9, 10, 11 |

Availability offsets are relative to the marker allocation; touched byte
indices are relative to the availability allocation. Every listed store
is four bytes. The literal separately writes its own availability byte
to one before the marker helper, so an entire before/after availability
comparison also includes that ordinary update.

In the compact layout, the following target intra block receives mask
`0x1211` for C three or `0x2221` for C four. The padded layout receives
the ordinary binary mask `0x1011`. The non-binary masks take the alpha
completion helper's range-check branch at `0x6e1dc` to return
`0x6e324`, bypassing the ordinary completion cases.

All twelve native two-frame sequences return success. Across both widths
and allocation fills zero/`0xa5`/`0xff`, each compact/padded pair differs
at 407 visible alpha pixels while its color channels remain identical.
Thus successful native decoding does not guarantee allocator-independent
output for these inputs. These controlled emulator results do not
establish a device allocator's layout, a device crash or host-memory
corruption. The bounded independent decoder rejects these cases rather
than modeling writes into another allocation.

## Complete bounded sequence validation

The corpus contains 108 five-frame sequences, 540 frames and 24840 block
operations. Dimensions are 16 by 49, 65 by 49 and 129 by 65, with one,
two or three block rows per packet group, both sampling modes, cache
capacities one/three and Q zero/30/51. Later frames interleave literals,
intra prediction, temporal updates, selected-plane updates and copies.

All alpha literals satisfy `4*c + 1 <= C`. There are 1296 such blocks:
624 at column zero, 432 at column one and 240 at column two. All 540
frames match independently reconstructed pixels. Native execution covers
14096 distinct instructions.

Comparisons also check bit consumption, complete B-one marker grids
before/after each block, intra prediction edges and working planes,
temporal motion/residuals, both cache orders, the shared cache fill count
and cached block pixels. The model obtains reconstruction inputs from
packet bytes and its own state, not observed native markers or edges.

Eight five-frame repeats retain their recorded output hashes with fresh
allocations filled with `0xa5` or `0xff`. Eight earlier full-size temporal,
reduced temporal, reference-cache and mixed-prediction sequences also
retain their recorded hashes across 48 frames through the extended model.
Separate boundary checks accept all 559 fitting footprint cases and
reject all 1546 crossing cases before changing the marker buffer.

The representative 129-by-65 sequence with two block rows per group,
Q 51, three cache entries and reduced color has artifact SHA-256
`fa6f99438722ce14acd55be5b3cde5420ac531a72f960019faab6f67ec79c266`.
Its fifth frame's visible output SHA-256 is
`9548b2aa94cbf36cdd574e04b40d2a5119671e43adba6106f1b159077a58830a`.
Artifacts use disposable length-prefixed sequence storage, not a
recovered SDOCX wrapper layout.

All 108 artifact digests and 540 independently reconstructed frame
digests were checked after generation. The APK/ELF identity, thirty cited
instruction words, marker callback bindings and existing availability
dispatch tables were checked against the binary. All 801 local
documentation links and 181 heading anchors pass validation.

## Encoder coverage and remaining work

Twenty-seven native encoder calls use dimensions 16 by 16, 33 by 17 and
65 by 17, quality options one/24/51, fixed color bytes, and alpha noise,
a modular ramp or alternating zero/255. Generous output capacities avoid
the earlier wrapper-capacity limitation. Native decoding recovers every
visible alpha byte exactly. The resulting 153 alpha blocks use mode 3
129 times and mode 0 twenty-four times; none uses literal mode 5.
This sample does not establish that the encoder never emits alpha literals.

The write footprint and the tested interactions are now recovered.
Remaining questions are actual encoder emission, real-export usage,
portable handling of cases outside the bounded model, other color/header
configurations and broader malformed-input limits. Native behavior that
depends on writes into adjacent allocations cannot define a portable
pixel result from the format bytes alone. Device exports and rendered
references are still needed for compatibility validation.

Generated sequences and scripts remain disposable. Maintained changes
are Markdown-only; no SDK code changed.
