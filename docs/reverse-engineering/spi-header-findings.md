# SPI codec header layout

## Evidence and scope

Recovered from Samsung Notes 4.4.45.37 ARM64 `libSPenBase.so` in the
[identified APK](README.md#sources-and-validation). All addresses below
refer to that ELF.

The [SPI wrapper trace](spi-media-findings.md) identified two outer
length-prefixed blocks. This trace resolves the selected codec's header
packet, including dimensions, color indices and flags. Isolated ARM64
reader and writer routines were also executed under Unicorn with synthetic
inputs. No complete SPI image or device behavior was validated.

## Native dispatch reaches a matching header reader and writer

The decoder constructor at `0x5d528` installs its block-consumption
function at context offset 1384 through `0x5d650`. GOT entry `0xeeed8`
resolves it to `0x5c8ac`. Operation `0x5da00` dispatches this member at
`0x5da20` after checking the context tag.

Encoder setup at `0x75cbc` stores its header-emission function at context
offset 1008 through `0x75d18`. GOT entry `0xeef50` resolves it to
`0x73f3c`. The wrapper's operation `0x75fe8` dispatches that member,
which calls the packet writer at `0x79c98` through `0x74754`.

## The emitted header packet is 20 bytes

The writer emits marker `0xaa`, kind 1 and constant length 20 at
`0x79cb4`, `0x79cc4` and `0x79cd4`. The decoder consumes this six-byte
prefix at `0x5c970` through `0x5c994`, then calls the field reader,
`0x67dac`, at `0x5c9b0`.

Offsets below start at the encoded-header block, after the outer
four-byte little-endian length:

| Byte offset | Encoding | Recovered field | Read / write call |
| --- | --- | --- | --- |
| 0 | `u8` | Marker `0xaa` | `0x5c970` / `0x79cb4` |
| 1 | `u8` | Packet kind 1 | `0x5c984` / `0x79cc4` |
| 2 | `u32_be` | Packet length, emitted as 20 | `0x5c994` / `0x79cd4` |
| 6 | `u8` | Unassigned header byte | `0x67dc4` / `0x79ce4` |
| 7 | `u16_be` | First size pair, width component | `0x67dd4` / `0x79cf4` |
| 9 | `u16_be` | First size pair, height component | `0x67df0` / `0x79d04` |
| 11 | `u16_be` | Image width | `0x67e0c` / `0x79d14` |
| 13 | `u16_be` | Image height | `0x67e28` / `0x79d24` |
| 15 | `u8` | Color index | `0x67e44` / `0x79d34` |
| 16 | `u16_be` | Row-group size | `0x67e54` / `0x79d44` |
| 18 | `u8` | Additional buffer-count byte | `0x67e8c` / `0x79d54` |
| 19, bit 7 | `u1` | Unassigned flag A | `0x67e98` / `0x79d60` |
| 19, bit 6 | `u1` | Unassigned flag B | `0x67ea4` / `0x79d6c` |
| 19, bit 5 | `u1` | Unassigned flag C | `0x67eb0` / `0x79d78` |
| 19, bit 4 | `u1` | Unassigned flag D | `0x67ebc` / `0x79d84` |
| 19, bits 3–0 | `u4` | Required zero | `0x67ecc`, `0x67edc` / `0x79d94`, `0x79da8` |

The field reader fills an 18-byte native structure whose padding and member
order differ from this packed layout. Copying that structure's memory
would not reproduce a header.

The bit reader fills its word most-significant byte first at `0x8de0c`
through `0x8de24`, then extracts from the high end at `0x8dfb4`.
Its single-bit operation returns bit 31 at `0x8e004`. The writer mirrors
this ordering through `0x8e138`, `0x8e0c8` and the byte flush at
`0x8e094` through `0x8e0a4`. Header fields are big-endian; the outer
wrapper's block lengths are little-endian.

This synthetic header encodes width 4660, height 9029, color index 4,
row-group size 2 and flags A/C:

```text
aa 01 00 00 00 14 03 00 00 00 00 12 34 23 45 04 00 02 04 a0
```

Its outer header-length prefix would be `14 00 00 00`. Native routines
emitted and read the example; its arbitrary auxiliary values are not a
recommendation for producing valid images.

## Dimensions connect to the wrapper's property queries

The field reader places width and height at decoder-context offsets 102
and 104. The consumer publishes them to offsets 1008 and 1010 at
`0x5cbe4` and `0x5cbf4`. Query IDs 201 and 202 return these values at
`0x5d9c4` and `0x5d978`, connecting the wire fields to bitmap dimensions.

The first size pair occupies context offsets 98 and 100. When both are
nonzero, the consumer copies them to capacity members 1020 and 1022;
otherwise it grows those members to accommodate the image. The path at
`0x5ca9c` through `0x5caf0` rejects a capacity smaller than the image.
This establishes their size-limit behavior without assigning an original
vendor field name. Encoder setup clears this pair along with the header
structure at `0x74254`.

The row-group field occupies context offset 106. For positive values,
the division at `0x5cc64` and store at `0x5cca8` establish:

```text
group_count = ceil(ceil(height / 16) / row_group_size)
```

The [data-packet trace](spi-data-packet-findings.md) connects packet group
indices to rows of 16 by 16 blocks; their pixel coding remains unresolved.
Header byte 18 drives
an additional buffer-allocation loop at `0x5cf24` through `0x5cf6c`;
its broader codec meaning remains unassigned.

## Color indices differ from wrapper color-type values

The wire byte at offset 15 is stored at context offset 110. Query 413
loads it at `0x5d9dc` and indexes the table at `0x29170`, resolved
through GOT entry `0xeee00`:

| Wire index | Codec API value |
| --- | --- |
| 0 | 13 |
| 1 | 43 |
| 2 | 400 |
| 3 | 401 |
| 4 | 500 |
| 5 | 501 |
| 6 | 502 |
| 7 | 503 |

The bitmap wrapper accepts API values 400, 500 and 501, corresponding to
wire indices 2, 4 and 5. Channel order and compression semantics remain
unassigned. The field reader itself accepts arbitrary index bytes; a
future SDK lookup must validate an index before accessing its own mapping.

## Native acceptance has distinct boundaries

The field reader checks all four 16-bit size values against 32768 and
returns `-202` for larger values. Zero passes these local checks. It also
requires the row-group size to be at most `ceil(height / 16)` at
`0x67e68`, but does not reject zero there. These results describe local
parser behavior, not valid dimensions or safe allocation limits.

All four high flag bits are accepted independently. The low three bits
are checked together at `0x67ed4`, followed by the final bit at
`0x67ee0`; a nonzero low nibble returns `-202`.

The bitmap wrapper's preliminary check accepts `aa 00` and `aa 01`.
The selected codec consumer is stricter: kind 1 enters the header reader,
kind 2 enters the data path, and kind 0 reaches `-202` at `0x5cfe0`.
Passing the `aa 00` probe does not establish an alternate supported header.

For kind 1, the consumer reads the embedded 32-bit length but does not
compare it with 20 before entering the field reader. The encoder emits
20; the outer wrapper separately compares consumed bytes with its block
length. These are distinct checks.

## Validation and remaining work

The APK digest, Base ELF stream, constructor dispatch bindings, bit-order
helpers, property table and cited instructions were checked against the
binary bytes.

Unicorn executed the isolated native packet writer and field reader for
101 synthetic cases. Each produced the specified 20-byte layout and
recovered the source fields. Another 57 cases checked size limits,
row-group bounds, color bytes and reserved-bit combinations. Nine
packet-dispatch cases checked marker/kind rejection and showed that
varying the embedded length still reaches the kind-1 field reader.
Accepted dispatch cases stopped at that reader's entry. These checks
executed 427 distinct APK instructions, without running the full decoder.

The routines used the APK's bit-reading and writing helpers without host
replacements. The subsequent [data-packet trace](spi-data-packet-findings.md)
recovers kind-2 prefixes and block coordinates. Real-file compatibility,
auxiliary field semantics, pixel reconstruction and rendering remain
unvalidated. No SDK code changed.
