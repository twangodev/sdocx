# SPI color palettes and run-length blocks

## Evidence and scope

Recovered from Samsung Notes 4.4.45.37 ARM64 `libSPenBase.so` in the
[identified APK](README.md#sources-and-validation). Primary mode 4 stores
three color planes using a cached palette and runs of palette indices.
Its independent scratch reader now combines with
[copies](spi-copy-block-findings.md), [literals](spi-literal-block-findings.md)
and [alpha reconstruction](spi-alpha-state-findings.md) to decode all 30
original [native-generated images](spi-codec-validation.md) byte for byte.
Neither the color streams nor their alpha-pass offsets need replacement
or native parsing as an input to the complete image reader.

The complete-image scope is primary modes 0/1/4/5 and alpha modes 0/1/3,
wire color index 4, header flags `0xe0` and packet byte B zero. Native
instructions, intermediate buffers and final pixels were compared under
Unicorn. This remains synthetic codec validation; device-exported files,
other modes/configurations and SDK integration remain open.

## Mode 4 selects the palette reader

The primary mode-4 prefix is `0010`. Dispatch enters `0x68718`, clears
worker byte 2493 at `0x68728`, then calls `0x689c0` at `0x6872c`.
The callee checks worker mode byte 48 at `0x68a30` through `0x68a40`.
Mode 4 reaches the palette path at `0x68a44`; the same function's other
branches serve different coding paths and are outside these findings.

Palette fields follow the mode prefix immediately. Eight-bit palette
components are packed bit fields, with no byte alignment before them.
The block ends after its final run or constant-color triplet. Ordinary
pass alignment still applies before the alpha pass.

## A worker retains two palette buffers

| Worker offset | Representation | Role |
| --- | --- | --- |
| 2500 | 32-bit integer | Cached palette entry count |
| 2504 | 32-bit integer | Cached palette index width |
| 2508 | 768 bytes | Cached 256-entry palette |
| 3276 | 768 bytes | Temporary replacement palette |
| 4044 | 256 bytes | Current block's expanded indices |
| 8416 / 8424 / 8432 | Pointers | Three 256-byte output planes |

Each palette entry has three bytes, in the same plane order as
[primary literals](spi-literal-block-findings.md). For the tested API
color value 500, those planes become output byte positions 1, 2 and 0;
alpha becomes output byte 3. These are byte-position mappings, without
assigning channel names to the API's first three bytes.

Packet initialization at `0x5d060` addresses worker offset `0x9c4`
(2500), and `0x5d07c` stores eight zero bytes there. Thus the entry count
and index width reset together for each packet group. The two 768-byte
buffers are retained. Their complete contents matched the independent
state before and after every traced palette block, including group changes.

## The control prefix selects reuse, extension or replacement

After the mode prefix, read:

| Control bits | Operation | Palette used for output |
| --- | --- | --- |
| `0` | Reuse cached metadata and bytes | Cached palette |
| `10` | Supply new metadata and append any missing entries | Cached palette |
| `11` | Supply new metadata and a replacement palette | Temporary palette |

The first control bit is read at `0x68a4c`. If set, the second is read
at `0x68a58`, and a four-bit index width `b` is read at `0x68a6c`.
Reuse instead loads cached width/count at `0x68b24` and `0x68b28`.

For `10` or `11` with nonzero width:

1. Read `b` bits and add one to obtain entry count `N`.
2. Reject `N > 256`; native `0x68a94` through `0x68a9c` branch to
   status `-202` at `0x68bf4`.
3. For replacement, read `3*N` eight-bit components into the temporary
   palette. For extension, begin at the old cached count and read only
   entries up to `N-1` into the cached palette.
4. Read index/run pairs until all 256 block positions are filled.

Extension does not require an increase in count. If `N` is at or below
the previous count, the component loop reads nothing and existing bytes
remain. Selection of the extension start occurs at `0x68b38` through
`0x68b40`; the component loop is `0x68ac4` through `0x68adc`.

After reconstruction, `0x68cf4` and `0x68cf8` store the resulting count
and width. Replacement additionally copies all 768 temporary bytes into
the cached palette through `0x68d00`–`0x68d0c`, including entries beyond
the newly declared count. Other operations retain the existing buffer.

## Runs expand palette indices in row order

For each run, read `b` index bits followed by a positive integer `L`.
The positive integer uses the [copy-code representation](spi-copy-block-findings.md):
`k` zero bits, a one bit, and a `k`-bit suffix `s`, giving `L = 2^k + s`.
There is no subtract-one adjustment to the run length.

Routine `0x68c34` reads the index; `0x68c54` calls the positive-code
reader. It repeats that index for `L` positions beginning at the current
linear offset. A run may cross a 16-pixel row boundary. A run extending
past position 255 returns `-202` through `0x68c70`–`0x68c74` and
`0x68c9c`–`0x68ca4`.

Once exactly 256 positions have been filled, `0x68cb8` through `0x68ce4`
map each stored index `i` to palette components at `3*i`, `3*i+1` and
`3*i+2`, writing one byte into each output plane. Linear position `p`
corresponds to pixel `(p % 16, p / 16)` within the block. Successful
reconstruction marks the current block available at `0x68d20`.

The native reader accepts widths 9–15 when the declared count is within
256. Indices are stored as bytes at `0x68c4c`, so wider index fields are
truncated to their low eight bits. Lookup does not compare that byte
against the declared count. Isolated probes verified both truncation and
reads of retained entries beyond the count. These observations describe
the tested native routine; they do not establish canonical encoder output
or a recommended SDK acceptance policy.

## Zero-width blocks have distinct update behavior

Width zero omits both the entry-count field and all index/run pairs.
The decoder fills all 256 pixels using the first entry of the selected
palette. Native vector stores perform the fill at `0x68b80` through
`0x68bec`.

The update behavior depends on the control prefix:

| Control | Components consumed | Output color | Cached count afterward |
| --- | --- | --- | --- |
| `0`, cached width zero | None | Cached entry zero | Previous count |
| `11`, new width zero | Three bytes into temporary entry zero | New temporary entry zero | 1 |
| `10`, new width zero | Three bytes into temporary entry zero | Existing cached entry zero | 0 |

The last row is a native behavior worth preserving in the findings:
the new triplet is consumed but does not supply that block's pixels or
replace the cached palette. The reads always target worker offsets
3276–3278 at `0x68b54`, `0x68b64` and `0x68b74`, while extension selected
the cached output pointer at `0x68b3c`. The cached width becomes zero.

Likewise, a reuse control at the start of a later packet sees reset
width/count zero and can fill from a prior packet's retained entry zero.
Constructed complete images verified this across packet boundaries and
intervening alpha passes. No initial native palette contents were supplied
as inputs to the independent complete-image reader.

## Validation

The APK identity, extracted ELF and cited instruction words were checked.
The complete decoder consumes only SPI header and packet bytes; native
state is observed for comparison, not supplied to reconstruction.

- All 30 original native-generated images decoded independently with exact
  recovery of every input byte, including color beneath zero alpha. Native
  redecoding checked 71 palette blocks: 30 replacements, 23 extensions and
  18 reuses, plus 125 alpha blocks. Full cached/temporary palettes, stored
  count/width, expanded indices, three output planes and bit consumption
  matched. These image comparisons executed 5290 distinct native instructions.
- 36 constructed complete images used dimensions 1×33, 17×33, 33×49 and
  65×65, with one, two or three block rows per packet and three variants.
  They covered 90 packets, 323 palette blocks and 414 alpha blocks,
  combining palette, literal and copy color blocks with predicted and copied
  alpha. Replacement, extension, reuse, zero-width updates and packet
  resets matched intermediate state and every final pixel. Their native
  comparisons executed 4413 distinct instructions.
- 1536 isolated palette payloads matched native output, both complete palette
  buffers, metadata, indices, availability writes and consumed bits. They
  covered all eight bit alignments, all widths 0–15, counts through 256,
  replacement/extension/reuse, equal or decreasing counts, all run lengths
  1–256, low-byte truncation and lookup beyond the declared count. Palette
  buffers began with distinct nonzero patterns to expose retained bytes.
- 80 isolated invalid payloads agreed on rejection: 56 oversized palette
  counts and 24 runs extending past the block. All returned native `-202`.
  The combined isolated checks executed 399 distinct native instructions;
  imported calls were only `memcpy` and `memset`.
- Eight full-image repeats used fresh allocation fills `0xa5` and `0xff`
  for three original images and one constructed image with multiple packets.
  Complete palette-state comparisons and all pixels remained identical.
- The 155 prior independent alpha images retained their recorded pixel
  digests when decoded by the extended reader.

Scratch readers, generators, emulation harnesses and generated artifacts
remain disposable. Maintained changes are Markdown-only; no SDK code changed.

## Remaining work

The [differential trace](spi-differential-block-findings.md) now adds primary
mode 2. Primary color mode 3, other packet/header configurations, reference
buffers, alpha literal marker behavior and broader malformed-input handling
remain open. The selected configuration has independent complete-image
decoding for every block mode emitted by the original 30-image native corpus.
That corpus does not establish compatibility with arbitrary SPI files or
with the document-saving wrapper's premultiplication and threading behavior.
