# SPI alpha neighbor state and independent images

## Evidence and scope

Recovered from Samsung Notes 4.4.45.37 ARM64 `libSPenBase.so` in the
[identified APK](README.md#sources-and-validation). The independent
[payload](spi-alpha-payload-findings.md) and
[pixel](spi-alpha-pixel-findings.md) readers now initialize and advance
their own alpha neighbor state. Complete synthetic images decode without
native parsing or native state as an input.

The scratch image decoder in this trace supports primary color modes 0, 1 and 5,
and alpha modes 0, 1 and 3, with wire color index 4, header flags `0xe0`
and packet byte B zero. Every pixel matched Samsung's native decoder in
155 complete-image cases. General SPI compatibility, other configurations,
device-export validation and SDK integration remain open.

## The frame has a border filled with 128

For width `W` and height `H`, let:

```text
C = ceil(W / 16)
R = ceil(H / 16)
padded_width = 16*C
padded_height = 16*R
```

The tested native frame has a 16-pixel border on each side. Its byte-plane
stride is `padded_width + 32`; the first image pixel is at offset
`16*stride + 16` from the plane's allocation pointer. Constructor
`0x5fdf0` supplies the border dimensions through `0x5fe18` and initializes
the alpha allocation with value 128 at `0x5ff88` through `0x5ff9c`.

The internal frame descriptor stores the alpha allocation pointer at
offset 24, the first image-pixel pointer at 56, and the stride at 64.
Stores at `0x5ffbc`, `0x5ffdc` and `0x5fff4` establish those members.
All 155 image checks verified the stride and image-pointer offset.

The independent model initializes its padded image plane with 128 and
returns 128 for border coordinates. Decoded blocks overwrite their full
16×16 regions, including padding beyond the visible width or height.
Visible output is cropped only after block reconstruction.

## Prediction markers retain two sets of block rows

Each prediction marker describes a 4×4 pixel cell. The marker stride is:

```text
S = 4*C + 1
```

The extra column is a left sentinel. Native initialization at `0x5cd84`
through `0x5cdb8` computes `S` and reserves `9*S` bytes per marker plane.
For alpha, allocation/pointer fields are at worker offsets 2456 and 2424.
The alpha buffer is filled with mode 2 through `0x5ce6c`–`0x5ce74`;
`0x5ce80` stores a working pointer at allocation offset `5*S + 1`.

The buffer has nine rows of `S` bytes:

| Rows | Role |
| --- | --- |
| 0 | Leading guard row |
| 1–4 | Previous block row's four marker rows |
| 5–8 | Current block row's four marker rows |

Before a packet group, `0x5d0c0` through `0x5d0d4` reset rows 1–4 to
mode 2. Current rows are retained; the decoder updates each visited block.
At the end of an alpha block row, `0x6d56c` through `0x6d578` copy all
four current rows into the previous-row region. This is a copy, not a swap
or a clear of the current rows.

Mode 3 updates the markers while reading each partition, as specified in
the payload findings. Copy modes 0 and 1 reset the corresponding 4×4
marker-cell rectangle to mode 2 through `0x5e9a0`. Their helper arguments
use `pixel_x / 4`, selected at `0x6802c` and `0x68228`.

## Block availability follows packet groups

The separate availability buffer has `2*(C + 2)` bytes. Its working
pointer is at allocation offset `C + 3`; each row has a sentinel on both
sides. Initialization uses `0x5ce8c` through `0x5cea4` and `0x5cd54`
through `0x5cd6c`. At row completion, `0x6d57c` through `0x6d5a4` copy
the current availability row, including sentinels, to the previous row.

Successful alpha modes 0, 1 and 3 mark the current column available at
`0x67ff0`, `0x680f0` and `0x68918`. For the tested configuration, ordinary
neighbor availability therefore follows already decoded block positions.

The above-row condition is `block_row > group_start_row`, determined
from worker offsets 22 and 2384 in `0x68838` through `0x6885c`. It is
false for the first row of every packet group, even if an earlier packet
has already decoded pixels above it. A copy mode can still reference
those earlier pixels; prediction treats that group boundary differently.

With an above row available, `0x6e0c4` through `0x6e10c` combine four
availability bits into a selector:

```text
selector = left + 16*above + 256*above_right + 4096*above_left
```

This selects the native edge-completion branch. The rules below cover
the neighbor patterns reached by modes 0/1/3 in the tested configuration.
The later [mixed-prediction trace](spi-mixed-prediction-findings.md)
recovers all sixteen binary availability patterns and combines intra and
temporal blocks in retained-reference sequences.

## External alpha edges can be reconstructed from earlier pixels

For a block at padded-image origin `(x, y)`, the raw above array contains
33 bytes from `(x-1, y-1)` through `(x+31, y-1)`. The raw left edge
contains 16 bytes at `(x-1, y)` through `(x-1, y+15)`.

Helper `0x6d5a8` copies the above array only when the above row is
available. It sets left entry zero from the above corner in that case,
or to 128 otherwise, then copies the sixteen left pixels. Routine
`0x6e0b4` fills unavailable edges and the remaining extension entries.

The independent rules are:

| Block position | Left array `L[0..32]` | Above array `A[0..32]` |
| --- | --- | --- |
| First column, first row of group | All 128 | All 128 |
| Later column, first row of group | `L[0]=128`; entries 1–16 from the left block; repeat entry 16 through 32 | All equal to `L[1]` |
| First column, later row, multiple columns | All equal to raw `A[1]` | Raw above pixels, with `A[0]=A[1]` |
| First column, later row, one-column image | All equal to raw `A[1]` | Raw above pixels; preserve border corner 128; repeat `A[16]` through 32 |
| Later column, later row | Raw above-left corner, sixteen left pixels, then repeat `L[16]` through 32 | Raw above pixels; at the final column, repeat `A[16]` through 32 |

The first-row branches include `0x6e2c8` and `0x6e19c`. For later rows,
the first-column branches resolve to `0x6e374` when an above-right block
exists and `0x6e45c` for a one-column image. Interior/final-column branches
include `0x6e420` and `0x6e540`.

This explains why the left and above corner bytes can differ: at a group
boundary the left corner remains 128 while the above array uses the
first available left pixel. The one-column, later-row case similarly
preserves the raw border corner only in the above array.

## Alpha literal marker writes need separate treatment

The [literal pixel layout](spi-literal-block-findings.md) remains verified,
but its alpha marker callback has a different coordinate argument.
At `0x689a4`, mode 5 passes pixel X directly to `0x5e9a0`, without the
division by four used by copy modes. The helper writes four bytes of
value 2 at each of four row strides, starting from its supplied offset.

Instrumented native decoding of literal-only images confirmed these
writes outside the requested alpha marker allocation:

| Width × height | Marker bytes requested | Four-byte stores crossing the requested range |
| --- | --- | --- |
| 1×1 | 45 | 0 |
| 17×1 | 81 | 2 |
| 33×1 | 117 | 3 |
| 65×1 | 189 | 6 |

For width 17, marker origin is 46 and stride is 9. The second block at
pixel X 16 writes at allocation offsets 62, 71, 80 and 89; the last two
stores cross the requested 81-byte range. Native stores occur at
`0x5e9b8`, `0x5e9bc`, `0x5e9c0` and `0x5e9c4`.

All four images still returned their exact literal pixels. These were
direct codec calls under Unicorn with recorded allocation sizes and
memory writes; no host-memory overrun or device reproduction is claimed.
Pixel agreement alone did not validate auxiliary marker writes.

The new decoder rejects alpha mode 5 instead of modeling writes outside
its marker buffer. Its interaction with following predicted blocks and
the intended portable behavior remain open. The earlier literal-only
pixel reader does not consume these prediction markers.

## Validation

The APK identity, ELF bytes, cited instructions, availability dispatches,
marker initialization and frame-layout relationships were checked.

- All 125 alpha blocks in the 30 original native-generated images decoded
  independently from their alpha-pass bit offsets. Native color decoding
  supplied those offsets for this trace only. Full marker buffers before
  and after each block, edge arrays, bit consumption, padded alpha blocks
  and visible alpha bytes matched without native neighbor-state inputs.
- Thirty complete images replaced the original color data with literal
  planes while retaining the native-generated alpha streams. The complete
  independent image reader located both passes itself and recovered every
  original visible alpha byte.
- Eighty complete 16×16 images covered all 64 side-4 partition masks and
  all eight masks for sides 8 and 16.
- Forty-five mixed images used sizes 1×33, 16×49, 17×33, 33×49 and 65×65,
  with one, two or three block rows per packet group and three mode
  variants. They combined primary modes 0/1/5 with alpha modes 0/1/3,
  including copies across groups, clipped edges and one-column frames.

The 155 complete images contain 224 packet groups and 655 alpha blocks:
430 mode-3 blocks, 135 mode-0 copies and 90 mode-1 copies. All 18 prediction
modes occurred. Every visible pixel matched between the independent reader
and Samsung's native decoder. Native comparisons exercised 5515 distinct
instructions. The marker-write probe above is separate from these images.

The independent image reader uses only the header and packet bytes. Native
state is observed for comparison, not passed into reconstruction. Scratch
code and generated artifacts remain disposable; maintained results are
Markdown-only and no SDK code changed.

## Remaining work

Subsequent [palette work](spi-palette-block-findings.md) adds primary mode 4
and independent decoding of the 30 original native-generated images.
The [differential trace](spi-differential-block-findings.md) also adds primary
mode 2. The [mode-3 color trace](spi-color-intra-findings.md) adds full-size
planes with zero quantization and independent color marker grids. The
[mixed-prediction trace](spi-mixed-prediction-findings.md) extends edge
completion and marker transitions to temporal neighbors. Resolve alpha
literal marker behavior and extend other packet/header variants and
malformed-input handling. Device-exported files are still needed to
establish compatibility beyond these synthetic cases. The selected
configuration now has complete independent decoding for its supported
color and alpha mode combinations.
