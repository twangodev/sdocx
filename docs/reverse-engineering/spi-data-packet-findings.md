# SPI data packets and block groups

## Evidence and scope

Recovered from Samsung Notes 4.4.45.37 ARM64 `libSPenBase.so` in the
[identified APK](README.md#sources-and-validation). All addresses below
refer to that ELF.

The [SPI header trace](spi-header-findings.md) resolves packet kind 1.
This trace resolves the kind-2 prefix and its connection to decoder block
coordinates. Isolated native routines were executed under Unicorn with
synthetic inputs. Subsequent [native codec tests](spi-codec-validation.md)
round-trip complete synthetic images, and the
[literal-block trace](spi-literal-block-findings.md) independently
reconstructs mode 5. Full pixel syntax and general independent decoding
remain unresolved.

## Kind 2 has a 14-byte prefix

The packet writer at `0x79dac` emits marker `0xaa`, kind 2 and an initially
zero 32-bit length through `0x79dc8`, `0x79dd8` and `0x79de8`. It then
writes eight bytes of packed fields. The matching field reader is
`0x67eec`; the single-worker consumer calls it at `0x5cb1c` after reading
the common six-byte packet prefix.

Offsets below start at the kind-2 packet, within the outer encoded-data
block. Bit 0 is the most significant bit of byte 0. The packed fields
use the same big-endian, most-significant-bit-first helpers as kind 1.

| Bit offsets | Encoding | Recovered field | Read / write call |
| --- | --- | --- | --- |
| 0–7 | `u8` | Marker `0xaa` | `0x5c970` / `0x79dc8` |
| 8–15 | `u8` | Packet kind 2 | `0x5c984` / `0x79dd8` |
| 16–47 | `u32_be` | Complete packet length, initially zero | `0x5c994` / `0x79de8` |
| 48–55 | `u8` | Unassigned byte A | `0x67f0c` / `0x79df8` |
| 56–71 | `u16_be` | Block-row group index | `0x67f1c` / `0x79e08` |
| 72–79 | `u8` | Unassigned byte B | `0x67f2c` / `0x79e18` |
| 80 | `u1` | Buffer-copy shortcut flag | `0x67f38` / `0x79e24` |
| 81–82 | `u2` | Unassigned selector | `0x67f5c` / `0x79e34` |
| 83–90 | `u8` | Unassigned byte C | `0x67f6c` / `0x79e44` |
| 91–98 | `u8` | Unassigned byte D | `0x67f7c` / `0x79e54` |
| 99–106 | `u8` | Unassigned byte E | `0x67f8c` / `0x79e64` |
| 107–110 | `u4` | Required zero | `0x67f9c` / `0x79e74` |
| 111 | `u1` | Required zero | `0x67fac` / `0x79e88` |

The nine-byte native field storage has a different order: byte B at
offset 0, byte A at 1, selector at 2, bytes C/D/E at 3/4/5, group index
as a native `u16` at 6, and the shortcut flag at 8. A direct memory copy
does not reproduce the packed prefix.

This synthetic output from the isolated writer uses byte A = 3, group
index `0x1234`, byte B = 5, shortcut = 1, selector = 2, and bytes C/D/E
= `0x67`/`0x89`/`0xab`:

```text
aa 02 00 00 00 00 03 12 34 05 cc f1 35 60
```

This is a prefix before finalization, not a complete valid image packet.

## The encoder patches the length after writing the packet

The group encoder at `0x6e9a4` constructs field storage and calls the
prefix writer at `0x6eb8c`, initially clearing the shortcut flag at
`0x6eb84`. A later branch can restore saved writer state and emit the
prefix with shortcut = 1 at `0x73b40` through `0x73bc8`.

Both paths reach finalization at `0x73bcc`. After flushing the bit writer,
the code subtracts packet start from the current output pointer at
`0x73bf4`. Stores at `0x73c0c`, `0x73c10`, `0x73c14` and `0x73c18`
patch bytes 2–5 with that length in big-endian order. This length includes
the packet prefix. The next store, `0x73c1c`, sets byte A to zero.

The packet length also serves as a boundary in the multi-worker consumer.
The scan at `0x5ca18` through `0x5ca90` saves each field-start pointer,
advances it by the declared packet length minus six, and reads the next
`aa 02` prefix. Worker dispatch loads a saved pointer at `0x5c710` and
calls the same field reader at `0x5c720`. This is static control-flow
evidence; malformed multi-packet input and worker execution were not tested.

## The group index selects rows of 16 by 16 blocks

Header consumption computes block columns and rows by adding 15 to image
width and height and shifting right by four, then stores them at context
offsets 1012 and 1014 through `0x5cc04` and `0x5cc14`.

Kind-2 fields occupy worker offset 8. Group setup at `0x5d024` loads the
index from worker offset 14 and the header's row-group size from context
offset 106. It multiplies them at `0x5d04c` and stores the starting block
row at worker offset 2384 through `0x5d074`.

The ordinary traversal at `0x5c41c` computes:

```text
block_columns = ceil(width / 16)
block_rows = ceil(height / 16)
first_block_row = group_index * row_group_size
end_block_row = min(first_block_row + row_group_size, block_rows)
```

The row loop uses this exclusive end, selected at `0x5c458`, and visits
columns from zero to `block_columns - 1`. Before each block it stores:

| Worker offset | Value | Store |
| --- | --- | --- |
| 18 | Low 16 bits of `row * block_columns + column` | `0x5c4f4` |
| 20 | Block column | `0x5c4d4` |
| 22 | Block row | `0x5c4dc` |
| 24 | Pixel X = `column * 16` | `0x5c4ec` |
| 26 | Pixel Y = `row * 16` | `0x5c4e4` |

It calls block routine `0x6b36c` at `0x5c558`, two selected callbacks at
`0x5c570` and `0x5c590`, then a row-completion callback at `0x5c5c0`.
These operations still need tracing before pixel coding can be specified.

The loop performs one pass, plus another when context byte 1025 is 1;
`0x5c5ec` through `0x5c604` controls repetition and clears buffered bits
between passes. The header consumer sets this byte for API color values
43 and 500–503 at `0x5cbfc` through `0x5cc34`. The later
[input/output trace](spi-codec-validation.md#the-fourth-byte-connects-to-alpha-handling)
identifies the additional pass as alpha for API value 500.

## Block-mode prefixes select six dispatch entries

Routine `0x6b36c` reads one bit at `0x6b384`. A set bit selects mode 0.
Otherwise it reads another bit at `0x6b39c`; a set bit selects mode 1.
Two clear bits cause a two-bit read at `0x6b3c0`, selecting mode 2 plus
that value. The selected mode is stored at worker offset 48.

The first callback table resolves through GOT entry `0xeef00` to
`0xf68b8`, with six pointers per pass. The following callback table is
at `0xebd38`, also with six pointers per pass:

| Prefix bits | Mode | Primary payload callback | Additional-pass payload callback |
| --- | --- | --- | --- |
| `1` | 0 | `0x67fcc` | `0x67fcc` |
| `01` | 1 | `0x68054` | `0x68054` |
| `0000` | 2 | `0x68264` | `0x6881c` |
| `0001` | 3 | `0x68470` | `0x68824` |
| `0010` | 4 | `0x68718` | `0x6881c` |
| `0011` | 5 | `0x68774` | `0x68950` |

The entry at `0x6881c` returns `-202`, so modes 2 and 4 are rejected by
the additional-pass payload dispatch even though their prefixes parse.
For modes 2–5, the prefix reader also clears worker byte 2495. It leaves
that byte unchanged for modes 0 and 1.

An isolated native check covered all 256 possible first bytes, verifying
the selected mode, consumption of exactly 1/2/4 bits, and that state
change. The [complete native bitmap tests](spi-codec-validation.md)
exercise modes 0, 1, 3, 4 and 5. The subsequent
[mode-5 trace](spi-literal-block-findings.md) resolves its literal planes.
The other payload formats remain only partly traced; their mode numbers
do not yet imply named compression algorithms.

## The shortcut copies existing buffers

A nonzero shortcut flag bypasses the ordinary block loop and calls
`0x6e570` at `0x5c40c`. That routine computes the group's clipped row
extent, then uses `memcpy` at `0x6e5e8`, `0x6e600` and `0x6e618` to
copy three plane regions from the buffer descriptor at context offset
1000 to the descriptor at 992. When context byte 1025 is nonzero, a
fourth region is copied at `0x6e638`.

This establishes buffer reuse as the shortcut's effect. The source
buffers' lifecycle and whether a standalone cache image can use this
path remain unresolved.

## Local rejection rules and validation

The field reader receives header flag B, stored at context offset 111,
through `0x5cb14`. If that flag and the packet shortcut flag are both
nonzero, it returns `-202` at `0x67f4c`. Nonzero reserved bits also
return `-202` through `0x67fa4` and `0x67fb4`. The local field reader
does not range-check the group index, selector or unassigned bytes.
Acceptance there does not establish a valid complete packet.

During ordinary traversal, context value 1360 = 1 causes an early return
of positive status 402 at `0x5c60c`. It is distinct from negative decode
errors.

The APK digest, Base ELF bytes, cited instructions and `memcpy` import
were checked. Native ARM64 execution covered:

- 257 prefix encodes, each matching the proposed layout and decoding back
  to its source fields.
- 144 field-reader cases covering reserved bits, both flags, all selector
  values and the extrema of each unassigned byte and group index.
- 16 group-start calculations, stopped before buffer initialization.
- 125 first-block or empty-group cases across image and group boundaries,
  stopped before block payload decoding.
- The cancellation return, dispatch to the shortcut routine, and six
  isolated finalization cases checking length bytes and byte A clearing.

These checks executed 450 distinct APK instructions without replacing
native helpers. They did not execute the buffer-copy shortcut, complete
block traversal, pixel callbacks, allocation or worker threads. Later
[native codec tests](spi-codec-validation.md) execute complete synthetic
bitmap round trips. Device SPI payloads and rendered references remain
needed for compatibility validation. No SDK code changed.
