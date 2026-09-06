# SPI native bitmap round trips

## Evidence and scope

Samsung Notes 4.4.45.37 ARM64 `libSPenBase.so` from the
[identified APK](README.md#sources-and-validation) was executed under
Unicorn 2.1.4. Thirty synthetic bitmaps were encoded and decoded by its
native codec with exact recovery of every input pixel byte.

This establishes a way to generate and check compressed SPI samples
without SDOCX files. It does not establish device-export compatibility,
an independent decoder, or SDK support. The native codec performed the
pixel work; its complete compression format is not yet specified.

## Executed API sequence

The ELF's loadable segments and resolved local relocations were mapped
into the emulator. Native codec instructions, tables, bit readers,
transforms and output conversion ran unchanged. Host implementations
supplied only these imported memory operations:

| PLT address | Import | Supplied behavior |
| --- | --- | --- |
| `0xe68d0` | `malloc` | Aligned allocation from a bounded emulated heap |
| `0xe7550` | `calloc` | Allocation followed by zero filling |
| `0xe68f0` | `free` | Allocation ownership check and bookkeeping |
| `0xe7bd0` | `memset` | Fill the requested byte range |
| `0xe7be0` | `memcpy` | Copy the requested byte range |
| `0xe6110` | `__memcpy_chk` | Check the supplied destination bound, then copy |

The heap did not recycle freed addresses. No Android runtime, file I/O,
logging or thread callbacks were supplied. Unknown imports, invalid
execution addresses and an instruction limit stopped execution rather
than returning fabricated codec success.

The encoder sequence matched the operations in the
[bitmap writer](spi-media-findings.md#extension-dispatch-selects-the-maetel-writer):

1. Initialize at `0x5c390` and construct at `0x7636c`.
2. Set property 2100 to integer 3 through `0x7609c`.
3. Emit a header through `0x75fe8`.
4. Supply the bitmap through `0x76068`.
5. Produce the data block through `0x76028`.

The 64-byte constructor input was zeroed, then its first nine little-endian
32-bit words were set to `[1, width, height, 1, 1, 0, 24, 0, 500]`.
The callback pointer at offset 48 remained null, selecting one worker at
`0x76458` through `0x7645c`. The APK file wrapper supplies callbacks with
a requested worker count of four at `0xd7d60` through `0xd7d74`; that
threaded configuration was not executed.

Output descriptors stored the destination pointer at offset 32 and its
capacity at 48. A zeroed 336-byte input-frame descriptor supplied width
and height at 32/36, API color value 500 at 48, stride `width * 4` at 52,
and the pixel pointer at 88. The 32-byte result storage was preserved
between header and data calls, including its group-count member.

Each result was decoded in a fresh emulator: initialize, construct at
`0x5d528` with `[selector = 1, callback_pointer = null]`, consume the
header and data in separate `0x5da00` calls, and output pixels through
`0x5da34`. Both consumed-byte counts matched the supplied block lengths.
Output used the same color value and stride, with frame height also at
offset 68. The returned dimensions matched the input dimensions.

## Synthetic coverage

Each of six sizes used five patterns:

| Size | Coverage |
| --- | --- |
| 1×1 | Single pixel and padding |
| 2×3 | Small rectangular image |
| 16×16 | Exactly one block |
| 17×17 | Crossing both block boundaries |
| 31×33 | Partial columns and three block rows |
| 33×49 | Three block columns and four block rows |

Pixels were four consecutive bytes. The patterns were:

- Solid `12 34 56 ff` and solid `12 34 56 00`.
- A gradient with bytes `floor(x * 255 / max(width - 1, 1))`,
  `floor(y * 255 / max(height - 1, 1))`, `(17*x + 31*y) % 256`, and
  `(13*x + 7*y) % 256`.
- A checker alternating `ff 00 00 ff` and `00 ff ff 00`, with the latter
  at even `x + y`.
- Four random bytes per pixel from Python `random.Random(0).randrange(256)`,
  resetting the generator for each size.

All 30 outputs matched all four input channels exactly, including color
bytes beneath zero alpha. This is measured behavior for this configuration
and sample set; it does not imply that every quality or coding mode is
lossless.

The successful tests supplied `8 * width * height + 4096` bytes of output
capacity and checked an additional 64-byte guard. That experimental
capacity was sufficient for these samples; it is not a proven codec bound.

All headers were 20 bytes, with color index 4 and flags `0xe0`. Each image
used one kind-2 packet with group index zero and shortcut flag zero; its
row-group size covered all block rows. Outer framing was assembled as
the two little-endian length-prefixed blocks recovered from the wrapper.
The file-writing wrapper itself was not executed.

Representative resulting SPI bytes are identified below. Sizes include
the outer eight length bytes and the 20-byte header.

| Input | SPI bytes | SPI SHA-256 |
| --- | --- | --- |
| 1×1 solid `12 34 56 ff` | 149 | `cfd279bde0c65b53e85d1a09b7cd71ab1a9abf71676b3958ceb56a070b2ff776` |
| 16×16 gradient | 1330 | `c179d13e6f42e3fea82ca7279044101a15c534305c131529276d931f14006d5f` |
| 33×49 random | 9377 | `2a46902aa5318688b1a9a16feeae4badae8d0cad482916ec3cdef291919df826` |

The 30 tests executed 12,967 distinct native instructions. Instrumentation
recorded 125 primary-pass blocks and 125 additional-pass blocks. Primary
block modes were 0, 1, 4 and 5; additional-pass modes were 0, 1 and 3.
[Mode-prefix decoding](spi-data-packet-findings.md#block-mode-prefixes-select-six-dispatch-entries)
was separately checked for all 256 possible first bytes. Mode 2 payloads
were not exercised by these bitmap samples.

Four samples—1×1 solid, 16×16 gradient, 17×17 random and 33×49 checker—were
each repeated with fresh allocations filled with `0xa5` and then `0xff`.
All eight repeats retained identical encoded bytes and exact pixel output.
These checks reduce dependence on initially zero memory; they are not a
general uninitialized-memory audit.

## The fourth byte connects to alpha handling

`BitmapFactory::RestorePremultipliedAlpha` at `0xa9ad8` reads byte 3 at
`0xa9afc` and uses it to rescale bytes 0–2. Its inverse at `0xa7f78` also
loads byte 3 at `0xa7f90`. This connects the wrapper's fourth byte to
alpha without assigning red/blue names to the other byte positions.

For API color value 500, the native input-conversion branch at `0x75120`
through `0x752d4` selects the fourth plane for that byte; `0x75648`
through `0x75650` copies it. The additional-pass mode-3 routine at
`0x68824` accesses this plane through the descriptor's offset 56 at
`0x688a4`. Output conversion reads the fourth plane and writes byte 3
at `0x5f9c4` through `0x5f9d4`. These connections identify the additional
pass as alpha for the tested format.

The round trips invoke codec APIs directly, without the bitmap wrapper's
premultiplication conversions. Exact recovery of hidden color bytes is
therefore a codec result, not a claim about saving premultiplied bitmaps.

## A wrapper-derived capacity failed in the single-worker experiment

The wrapper allocates `ceil(width/16) * ceil(height/16) * 1026 + 60` bytes
at `0xd7d8c` through `0xd7db8`. This gives 1086 bytes for a 16×16 bitmap.
Using that declared capacity for the gradient above produced:

| Declared capacity | Encoder status | Reported data bytes | Guard changes | Decoder result |
| --- | --- | --- | --- | --- |
| 1086 | 0 | 1091 | First five bytes changed | `-202` |
| 4096 | 0 | 1302 | None | Exact pixel recovery |

The capacity experiment reserved guard memory, so it did not overrun the
host process. Instrumentation captured the native call at `0x74e54`
requesting `memcpy(destination, packet, 1091)` for the smaller destination.
The copy length comes from the writer pointer difference at `0x74e48`.
The host memory-copy implementation honored that native request.

This demonstrates that the formula and a success return did not provide
a sufficient output guarantee in this single-worker codec configuration.
It does not prove the same result in the file wrapper's threaded path or
on a device. A future SDK encoder needs independently checked capacity
and error propagation rather than adopting this formula as a guarantee.

## Remaining work

The APK digest, extracted ELF, cited instruction bytes, import bindings,
mode-dispatch tables and output digests were checked. Generated samples,
raw pixels and the emulator harness remain disposable local artifacts.
Only Markdown findings are maintained here.

Remaining targets include mode payload syntax, prediction and residual
coding, mode 2, multiple packets, buffer-copy shortcuts, other color and
quality settings, malformed-input behavior and independent reconstruction.
Device-exported SPI files and rendered references remain necessary for
compatibility validation. No SDK code changed.
