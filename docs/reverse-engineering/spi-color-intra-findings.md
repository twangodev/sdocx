# SPI mode-3 color reconstruction with zero quantization

## Evidence and scope

Recovered from Samsung Notes 4.4.45.37 ARM64 `libSPenBase.so` in the
[identified APK](README.md#sources-and-validation). Primary mode 3 has
independent complete-image decoding for submode 1, full-size color planes
and selected quantizer zero. These checks use wire color index 4, header
flags `0xe0`, packet byte B zero and API color value 500.

An independent scratch reader matched 24 native-generated images and
128 constructed images, including intermediate fields, neighbor state,
signed color planes and final pixels. The native-generated corpus contains
23 primary mode-3 blocks; some images use only the previously recovered
modes. The constructed corpus contains 329 primary mode-3 blocks and
exercises all 18 prediction modes and all partition-mask codes.

The complete reader supports primary modes 0/1/2/3/4/5 and alpha modes
0/1/3 within these limits. Later [quantized color work](spi-quantized-color-findings.md)
adds nonzero quantization for the same full-size submode. The
[reduced-plane trace](spi-reduced-color-findings.md) adds header flags `0xf0`
with separate secondary-plane sizing and quantization. Other submodes
remain open. No SDK implementation changed, and generated
codec inputs do not establish compatibility with device-exported documents.

## A block selects its quantizer before three plane payloads

Primary prefix `0001` selects `0x68470`. With packet byte B zero,
`0x6851c`–`0x68524` sets worker submode byte 49 to 1. The sampling flag
comes from context byte 113 through `0x68528`–`0x68530`; it is zero in
the tested configuration. Unlike mode 2, this path does not read a sampling
bit from the block payload.

The next bit, read at `0x68534`, chooses between the packet's eight-bit
[C and D fields](spi-data-packet-findings.md#kind-2-has-a-14-byte-prefix):

| Bit | Selected quantizer Q |
| --- | --- |
| 0 | C |
| 1 | Minimum of C and D |

The minimum is selected at `0x6853c`–`0x68548`; `0x68550`–`0x68554`
uses C directly. Helper `0x6d438`, called at `0x68558`, stores Q at worker
byte 56. Thus a packet with C = 24 and D = 0 can still select the recovered
zero-quantizer path for an individual block.

Dispatcher `0x68d58` uses the primary submode table at `0x2a950` with
branch base `0x68e9c`. Entry 1 reaches `0x692b8`, then full-size sampling
reaches `0x6969c`. The three plane payloads are sequential and independently
select their subdivision:

| Plane | Read divided bit | Section-field reader | Signed pixel reconstruction |
| --- | --- | --- | --- |
| 0 | `0x696a0` | `0x69744` → `0x6a70c` | `0x69728` → `0x6d0e8` |
| 1 | `0x69a60` | `0x69b24` → `0x6a70c` | `0x69b08` → `0x6d0e8` |
| 2 | `0x69ca0` | `0x69d7c` → `0x6a70c` | `0x69db4` → `0x6d0e8` |

Divided zero gives one 16×16 section. Divided one gives four 8×8
sections, each with a split bit selecting one 8×8 partition or four 4×4
partitions. There is no alignment between these plane payloads.

With Q zero, `0x6a85c`–`0x6a860` selects the same coefficient branch at
`0x6a89c` already recovered for [alpha residuals](spi-alpha-residual-findings.md).
Each section uses the same prediction codes, partition masks, scan choices,
signed run tokens and coded flags described in the
[alpha payload findings](spi-alpha-payload-findings.md). The three planes
can use different subdivision and prediction choices.

Q nonzero instead calls coefficient reader `0x6b8a8` at `0x6a890` and
adds the transform stage `0x6abec`, called for the three planes at
`0x69760`, `0x69b40` and `0x69d98`. The subsequent
[quantized color findings](spi-quantized-color-findings.md) recover those paths.

## Color planes have separate prediction-marker state

The marker lookup at `0x6b584` includes the plane index: `0x6b5a0`
scales it by eight, and `0x6b5b8` loads the corresponding pointer at worker
offset `2400 + 8*plane`. The allocated marker-grid bases are at
`2432 + 8*plane`, with a shared stride at 2464.

Each plane follows the [alpha marker-grid layout and packet lifecycle](spi-alpha-state-findings.md).
Markers begin at 2; a packet resets the four preceding marker rows to 2.
After a block row, its four current marker rows become the preceding rows.
Mode-3 partitions update their own plane's cells immediately. Other primary
block modes in these images reset the corresponding 4×4 marker cells to 2
for all three planes. No prediction-marker state is shared between color
planes or with alpha.

The independent reader derives each marker grid from earlier fields.
Native checks compare the entire allocated grid before every primary
mode-3 block and after every section, including images that alternate
mode 3 with copy, literal, palette and differential blocks across packets.

## External byte neighbors undergo a reversible color transform

The decoder gathers each plane's byte edges through `0x6d5a8`, called at
`0x685ac`, `0x685dc` and `0x68608`, then completes missing neighbors
through `0x6d690` at `0x68620`. The tested blocks follow the
[previously recovered edge rules](spi-alpha-state-findings.md#external-alpha-edges-can-be-reconstructed-from-earlier-pixels):
group starts use neutral or left-derived neighbors, and subsequent rows
use earlier decoded frame pixels.

For this configuration, context byte 1024 selects signed color processing.
Calls at `0x686c0` and `0x686ec` use `0x5ea48` to convert all 33 above
triples and all 33 left triples into three arrays of 16-bit values.
Converted above arrays begin at worker offsets 12576, 12642 and 12708;
left arrays begin at 12774, 12840 and 12906.

Let A, B and C be the primary byte-plane values. For API color value 500,
these correspond to output byte positions 1, 2 and 0, respectively, as in
[literal blocks](spi-literal-block-findings.md). The forward transform is:

```text
d = C - B
t = B + floor(d / 2)
e = A - t
Y = t + floor(e / 2)
E = e + 256
D = d + 256
```

The stored plane order is Y, E, D. Valid byte inputs produce Y in 0–255
and E/D in 1–511. Negative halves round down. Scalar instructions at
`0x5ed34`–`0x5ed78` and the vector path beginning at `0x5eba0` implement
this transform. External color edges must be converted together before
predicting any of the three signed planes.

## Prediction and residual addition retain 16-bit samples

Reconstruction helper `0x6d0e8` writes a separate 16×16 signed plane
through worker pointer `12552 + 8*plane`, with stride 16 samples. It uses
the converted external edges and earlier reconstructed partitions within
that plane.

Partition-edge selection follows the [alpha reconstruction geometry](spi-alpha-pixel-findings.md).
The signed edge helpers are `0x67b20` for side 4 and `0x67cb4` for side 8.
The side-4 branch at `0x67bb0` retains the same above-array tail at
partition origin (4,4): it writes entries 0–4 while entries 5–8 retain
values from the preceding partition. Complete native traces verify those
entries before prediction.

Prediction uses the same direction and filtering tables at `0x2aa58` and
`0x2aa69`, but signed-sample callbacks replace the byte-output callbacks:

| Operation | Native helper or callback |
| --- | --- |
| Planar prediction, mode 17 | Context 1552, relocation `0xeed88` → `0x64210` |
| Directional prediction | Context 1560, relocation `0xeed80` → `0x63d8c` |
| Edge smoothing | `0x66fe8` |
| DC boundary adjustment | `0x671a8` |
| Q-zero residual accumulation, modes 0/1 | Context 1544, relocation `0xeedd0` → `0x620ec` |
| Q-zero prediction/residual combination | Context 1576, relocation `0xeede0` → `0x62618` |

The prediction equations match the alpha model over the tested 9-bit edge
domain. For coded partitions, mode 0 accumulates residuals vertically and
mode 1 horizontally before combination. Helper `0x62618` adds prediction
and residual with signed-16 wrapping, without byte clipping. Its vector
addition is at `0x626f8`–`0x626fc`; the scalar addition is at
`0x62740`–`0x62744`. Uncoded partitions copy prediction samples.

Q nonzero selects context callback 1568 instead, resolving through
`0xeedd8` to `0x62254`. Its [quantized reconstruction](spi-quantized-color-findings.md#combination-broadcasts-dc-and-clamps-to-each-planes-bit-depth)
is recovered separately.

## The output callback converts signed planes back to bytes

Primary pixel callback `0x5ddf4` handles submode 1 and full-size planes
through `0x5df9c`. With context byte 1024 set, `0x5dfc8` calls `0x5ed8c`
to convert the three signed planes into the frame byte planes at offsets
32, 40 and 48.

For reconstructed samples Y, E and D, the independent inverse is:

```text
e = signed16(E) - 256
d = signed16(D) - 256
t = signed16(Y - floor(e / 2))
A = clip_byte(t + e)
B = clip_byte(t - floor(d / 2))
C = clip_byte(B + d)
```

`signed16` retains the low sixteen bits and interprets them as signed.
Here `clip_byte(v)` first applies `signed16`, preserves values 0–255,
and otherwise returns `((-v) >> 15) & 255`. This preserves the native
signed-16 clipping behavior, including its unusual result 1 at −32768.

The third output uses the already clipped B byte. Scalar instructions
`0x5f184`–`0x5f190` store B, then explicitly use its low eight bits when
forming C. Using the unclipped intermediate instead differs on constructed
out-of-range samples. The forward and inverse equations recover valid
byte triples exactly; this is not an exhaustive characterization of every
malformed signed-16 input to all prediction and conversion paths.

## Native-generated quality settings expose the next decoding gap

A 16×16 input with API byte tuple
`((3*x+2*y)%256, (x+5*y)%256, (7*((x+y)//2))%256, 255)`
emits one primary mode-3 block and one alpha mode-3 block at each of seven
encoder quality arguments. Every primary block selects C directly:

| Encoder quality | Packet C | Packet D | Selected Q | Complete SPI bytes | Input bytes preserved out of 1024 |
| --- | --- | --- | --- | --- | --- |
| 0 | 0 | 0 | 0 | 982 | 1024 |
| 1 | 1 | 1 | 1 | 473 | 974 |
| 4 | 4 | 4 | 4 | 282 | 981 |
| 8 | 8 | 8 | 8 | 238 | 876 |
| 16 | 16 | 16 | 16 | 172 | 701 |
| 24 | 24 | 23 | 24 | 137 | 508 |
| 32 | 32 | 27 | 32 | 125 | 409 |

These are measured native encode/decode results, not a general quality
mapping or fidelity metric. In particular, quality 24 is not universally
lossless. At this milestone only Q zero was independently decoded;
the later [quantized decoder](spi-quantized-color-findings.md) matches all
seven outputs. The Q-zero SPI
SHA-256 is `73ceef275fbcf6a65acf685bbb4000fbae25c9a9cdf713ebeab30f2abe04a161`;
the decoded pixels hash to
`fe10dfe40e46151052bd470688f4731a9d6d8d1ff519b2f5c6fb36112ae03965`.

## Validation

The APK identity, extracted ELF, cited instruction words, dispatch table,
prediction tables and callback relocations were checked.

- Primitive comparisons covered 80 forward transforms with lengths
  1, 7, 8, 15, 16, 17, 33 and 256; 100 inverse transforms of 256 pixels;
  540 prediction grids across sides 4/8/16 and all 18 modes; and 180
  combination grids with residuals drawn from the full signed-16 range
  and flags 0/1/7.
  Inverse cases used valid component ranges and separate random inputs in
  −1024–1024. Prediction edges included zero, 511 and random 9-bit values.
  These comparisons used Python `random.Random(0)` and executed 4294
  distinct native instructions.
- The 24 native-generated quality-zero images combine six sizes
  (1×1, 2×3, 16×16, 17×33, 33×49 and 65×65) with smooth, striped,
  checker and random patterns. All input bytes were recovered. They contain
  23 mode-3 color blocks, 276 sections and 324 partitions, plus 11 mode-2
  blocks and other previously recovered modes. These comparisons executed
  8006 distinct native instructions. The 16×16 smooth image is the Q-zero
  sweep input above, counted once in this corpus.
- Eighty constructed 16×16 images cover all 64 side-4 masks and all eight
  masks for each of sides 8 and 16 in all three color planes. Another 48
  images combine sizes 1×33, 17×33, 33×49 and 65×65, one to three block
  rows per packet, four selector values and mixed primary/alpha modes.
  The 128 images contain 329 mode-3 color blocks, 2787 sections and 7395
  partitions, plus 87 differential and 632 alpha blocks across 200 packets.
  All 18 prediction modes occur; both choices of the quantizer bit occur
  with Q zero. These comparisons executed 10326 distinct native instructions.
- Every mode-3 comparison checks consumed bits, fields, coefficients,
  complete marker grids, converted external edges, per-partition edges,
  signed predictions, reconstructed signed planes and converted byte output.
  Eight repeats with fresh allocation fills `0xa5` and `0xff` retain the
  same state and pixels, including mixed blocks and multiple packet groups.
- The 325 prior original, alpha, palette and differential image cases retain
  their recorded pixels. Artifact digests and independent pixels also match
  all 152 new corpus records.

The complete independent reader consumes only header and packet bytes;
native state is compared afterward. Scratch readers, generators, native
harnesses and generated artifacts remain disposable. Maintained changes
are Markdown-only.

## Remaining work

The [quantized color trace](spi-quantized-color-findings.md) now recovers
nonzero-Q coefficients, scaling, inverse transforms and reconstruction.
The [reduced-plane trace](spi-reduced-color-findings.md) adds submode 1 with
reduced secondary planes. The [temporal trace](spi-temporal-block-findings.md)
adds primary and alpha submodes 0 and 2 with retained reference images.
Reduced temporal planes, submode 3, multi-reference selection,
alpha literal marker behavior, other header
and packet configurations, malformed-input limits, SDK integration and
device-export validation also remain open.
