# SPI alpha pixel reconstruction

## Evidence and scope

Recovered from Samsung Notes 4.4.45.37 ARM64 `libSPenBase.so` in the
[identified APK](README.md#sources-and-validation). The independent
[alpha payload reader](spi-alpha-payload-findings.md) now feeds scratch
pixel reconstruction that matches Samsung's native mode-3 alpha output.

Comparisons covered 518 partitions from the 30 native-generated images
and 1526 partitions from 98 independently assembled images. Partition-edge
arrays, predicted pixels, accumulated coded residuals and reconstructed
pixels were compared separately, not just the final bitmap.

Scope remains wire color index 4, header flags `0xe0`, packet byte B zero,
implicit alpha submode 1 and worker selector byte 56 zero. Each block's
starting prediction-marker state and two external pixel-edge arrays were
supplied by native neighbor preparation. Marker initialization, block-row
transitions and external edge construction are still required for a
standalone SPI decoder. Other passes and configurations remain outside
this result.

## Reconstruction callbacks

Routine `0x6ce0c` reconstructs one section. Constructor stores resolve
these context callbacks through the indicated GOT entries:

| Context offset | GOT entry | Routine | Role in this path |
| --- | --- | --- | --- |
| 1432 | `0xeed78` | `0x62f44` | Planar prediction |
| 1440 | `0xeed70` | `0x628b8` | DC or directional prediction |
| 1504 | `0xeedc8` | `0x61db0` | Prediction/residual combination |
| 1544 | `0xeedd0` | `0x620ec` | Residual accumulation |

The calls at `0x6cf84`, `0x6d054` and `0x6d0a4` invoke prediction and
accumulation. The zero-selector branch selects context offset 1504
through `0x6d0a8`, then calls it at `0x6cf04`.

The following `mode` values are prediction modes 0–17 from the payload,
not the outer block-mode number 3. Partition side `n` is 4, 8 or 16.

## Partition edges come from external arrays and earlier pixels

The alpha block's external left array starts at worker offset 9799;
the above array starts at 9667. Each contains 33 bytes. A partition
receives arrays `L` and `A`, each of length `2*n + 1`: index zero is a
corner reference, indexes 1 through `n` border the partition, and the
remaining indexes extend below or to the right.

For side 16, `0x6d008` through `0x6d028` copy both complete external
arrays. Side 8 uses helper `0x67950`; side 4 uses `0x677b8`. Their jump
tables at `0x2a824` and `0x2a814` select the section/partition position.

Sections and their subpartitions are visited in the quadrant order
specified in the payload findings. For partition origin `(x, y)` inside
the 16×16 block:

- A left edge on `x == 0` uses the external left array starting at `y`.
  An above edge on `y == 0` uses the external above array starting at `x`.
- Interior edge pixels come from earlier reconstructed partitions. Where
  an extension enters an unavailable partition or leaves the block,
  repeat the last available edge value, except for the case below.
- An interior corner is the pixel at `(x - 1, y - 1)`. At one external
  boundary, the corresponding external corner supplies both arrays.
  At the block origin, each array retains its own external corner;
  `L[0]` and `A[0]` need not be equal.

One side-4 case retains part of the preceding scratch array. At block
origin `(4, 4)`, branch `0x67850` writes above entries 0–4 but leaves
entries 5–8 unchanged. The preceding partition at `(0, 4)` initialized
those entries through `0x678ec`. Consequently this partition's above
extension repeats its own entries 1–4, rather than repeating only entry
4. Native comparisons checked this sequential behavior explicitly.

## Selected modes smooth the edges before prediction

The table at `0x2aa69`, resolved through GOT entry `0xeef30`, controls
filtering for prediction modes 0–16. Each row is indexed by mode:

```text
side 4:  0 0 0 1 0 0 1 0 0 1 0 0 0 0 0 0 0
side 8:  0 0 0 1 1 1 1 1 1 1 0 0 0 0 0 0 0
side 16: 0 0 0 1 1 1 1 1 1 1 1 1 1 1 1 1 1
```

A set entry invokes `0x6353c` on both edges. Its filtered corner is
shared, even when the original corners differ:

```text
corner = floor((L[1] + 2*L[0] + A[1] + 2) / 4)
L_filtered[0] = A_filtered[0] = corner
E_filtered[i] = floor((E[i-1] + 2*E[i] + E[i+1] + 2) / 4)
E_filtered[2*n] = E[2*n]
```

The interior equation applies to either edge `E` for `1 <= i < 2*n`.
The corner stores are at `0x63560`/`0x63564`; the scalar interior stores
are at `0x63754`/`0x63774`. Mode 17 bypasses this filtering path.

## Planar, DC and directional prediction

Mode 17 invokes the planar routine. For pixel `(x, y)`, both zero-based:

```text
P[x,y] = floor((n
             + (n-1-x)*L[y+1] + (x+1)*A[n]
             + (n-1-y)*A[x+1] + (y+1)*L[n]) / (2*n))
```

The endpoints are `A[n]` and `L[n]`, not entries `n+1`. The scalar
calculation is visible at `0x63028` through `0x63058`; sides 8 and 16
also exercise the vectorized implementation.

Other prediction modes map through `0x2aa58`, resolved by GOT entry
`0xeef38`, to a direction code `d`:

```text
mode: 0  1  2 3 4 5 6  7  8  9 10 11 12 13 14 15 16
d:    5 13  0 1 3 7 9 11 15 17  2  4  6  8 10 12 14
```

Direction zero fills the partition with the DC value:

```text
dc = floor((sum(L[1..n]) + sum(A[1..n]) + n) / (2*n))
```

The inclusive sums and rounding are implemented by the DC branch of
`0x628b8`, ending at `0x62f08` through `0x62f3c`.

For a nonzero direction, set `horizontal = (d >= 10)`. Then:

```text
delta = d - (13 if horizontal else 5)
angle_steps = [0, 5, 13, 21, 32]
inverse_angles = [0, 1638, 630, 390, 256]
angle = sign(delta) * angle_steps[abs(delta)]
main = L if horizontal else A
secondary = A if horizontal else L
```

The angle table at `0x29190` and unsigned 16-bit inverse table at
`0x29196` resolve through GOT entries `0xeee08` and `0xeee10`.
For negative angles, extend the main reference to negative indexes:

```text
main[-i] = secondary[floor((128 + i*inverse_angles[abs(delta)]) / 256)]
```

Here `i` runs from 1 through `-floor(n*angle/32)`. Native negative
extension is implemented at `0x62b90` through `0x62c18`.

For every row when vertical, or every column when horizontal, use
zero-based position `p` and coordinate `q` within that row/column:

```text
offset = floor((p+1)*angle / 32)
fraction = (p+1)*angle - 32*offset
index = q + offset + 1
value = floor(((32-fraction)*main[index]
             + fraction*main[index+1] + 16) / 32)
```

Store at `(q, p)` for vertical or `(p, q)` for horizontal. Fraction zero
does not require the second reference value in an independent reader.
The offset/fraction split occurs at `0x62cc4`/`0x62cc8`; scalar output
paths include `0x62d80` through `0x62d9c` and `0x62dc0` through `0x62ddc`.

## Mode 2 adjusts the DC boundary

After DC prediction, call `0x6d078` invokes `0x63780` using the original
edges. Only the first row and column change. With `w = 3, 2, 1` for sides
4, 8, 16 respectively:

```text
P[0,0] = floor((w*(L[1]+A[1]) + (8-2*w)*dc + 4) / 8)
P[x,0] = floor((w*A[x+1] + (8-w)*dc + 4) / 8)     for x > 0
P[0,y] = floor((w*L[y+1] + (8-w)*dc + 4) / 8)     for y > 0
```

The native side-4, side-8 and side-16 branches begin at `0x63798`,
`0x63be0` and `0x63874`. Interior pixels retain `dc`.

## Residual accumulation and byte conversion

The decoded coefficients are signed 16-bit values. Routine `0x620ec`
modifies them in place according to the prediction mode:

| Prediction mode | Operation |
| --- | --- |
| 0 | Starting with row 1, add the already accumulated coefficient above |
| 1 | Starting with column 1, add the already accumulated coefficient to the left |
| 2–17 | Leave coefficients unchanged |

Each addition wraps to 16 bits. Horizontal stores occur at `0x62138`;
the scalar vertical stores occur at `0x62244`, with a vectorized path
also present. Partition traversal does not accumulate across boundaries.

Routine `0x61db0` copies the prediction when the partition metadata byte
is zero. Otherwise it adds each accumulated coefficient to its predicted
byte, again retaining the signed 16-bit sum `v`. Its byte result is:

```text
output = v                       when 0 <= v <= 255
output = ((-v) >> 15) & 255       otherwise
```

The shift in this expression is arithmetic. The scalar native path at
`0x61fa4` through `0x61fc4` and vector paths agree on the tested domain.
This behaves like clipping except at signed minimum `v == -32768`,
which produces byte 1. For example:

| Coefficient | Prediction | Signed 16-bit sum | Output |
| --- | --- | --- | --- |
| −32768 | 0 | −32768 | 1 |
| 32767 | 1 | −32768 | 1 |
| −32767 | 0 | −32767 | 0 |
| 32767 | 0 | 32767 | 255 |
| −1 | 1 | 0 | 0 |

These extreme cases were constructed for the native routine. They are
not evidence that device exports normally produce such residuals.

## Validation

The APK digest, ELF identity, cited instructions, callback bindings,
direction/filter/angle tables and edge-dispatch tables were checked.

- The 30 original bitmaps retained exact native round trips. Independent
  reconstruction matched all 344 mode-3 sections and 518 partitions:
  232 of side 4 and 286 of side 8.
- The 98 constructed images additionally matched 1526 partitions,
  including side 16 and all previously tested mask values. Their pixel
  comparisons used the same independently parsed payloads and supplied
  block-boundary arrays, not native predicted pixels or residual values.
- 540 isolated prediction pipelines covered all 18 modes, all three
  sides and ten edge patterns: constants 0/1/128/255, a byte ramp,
  alternating 0/255 and four deterministic random cases.
- 200 sequential edge-helper comparisons covered every 4×4 and 8×8
  position with those pattern families, including differing external
  corner values and the retained above-extension case.
- 432 residual-accumulation cases covered all 18 modes, all three sides,
  contiguous/padded rows, signed extremes and mixed values. Padding
  remained unchanged.
- 1188 combination cases covered metadata 0/1/7, sides 4/8/16,
  contiguous/padded strides, eleven coefficient values and six prediction
  bytes. Both scalar and vector paths matched the signed-minimum behavior.

Intermediate native checks compared prepared edges and predicted pixels
at `0x6d07c`, accumulated coded coefficients at `0x6d0a8`, and output
pixels before the section return at `0x6d0b0`. The independent pixel
buffer began empty for each block and retained its own earlier partition
results. External edges were captured once per block and verified to
remain unchanged through its later sections.

The isolated checks executed 1680 distinct native instructions. Only
`memcpy`, `memset` and `__memcpy_chk` were supplied by host code; the
prediction, accumulation and combination instructions ran natively.
The 98 constructed images exercised 4639 distinct native instructions.

## Remaining work

Remove the remaining dependency on native prediction-marker and external
edge preparation, including block-row resets and unavailable neighbors.
Then combine alpha reconstruction with the other independently recovered
block modes. Primary compressed color modes, reference buffers, other
packet/header settings and malformed-input behavior remain open, as does
device-export compatibility. Maintained findings are Markdown-only; no SDK
code changed and scratch implementations remain disposable local tooling.
