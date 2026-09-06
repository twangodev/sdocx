# SPI media framing and native codec entry points

## Evidence and scope

Confirmed by static inspection of Samsung Notes 4.4.45.37 ARM64
`libSPenBase.so` from the [identified APK](README.md#sources-and-validation).
All addresses below are in Base.

The [document image-cache trace](document-image-cache-findings.md) follows
page-cache bitmap saving into `BitmapFactory::SaveBitmap`. Its extension
dispatch identifies SPI as input to Samsung's Maetel codec wrapper.
This investigation recovers the outer framing and native entry points.
The [header trace](spi-header-findings.md) now resolves the selected
codec's 20-byte header packet. Compressed pixels remain undecoded.

## Extension dispatch selects the Maetel writer

`BitmapFactory::SaveBitmap`, `0xa94fc`, extracts the final dot-separated
filename component and compares it with supported extensions. Comparisons
at `0xa96e4` and `0xa96f8` use strings `spi` at `0x2f294` and `SPI`
at `0x328f3`, respectively. Both select the branch at `0xa9a94`.

That branch supplies the destination path, pixel buffer, width, height,
stride and integer quality argument to `write_maetel_argb` through
relocation `0xef988` at `0xa9aac`. The export resolves to `0xd7b24`.
The factory calls `RestorePremultipliedAlpha` before this extension branch
at `0xa960c` and restores premultiplication after the writer at `0xa9a1c`.
Those operations matter to eventual pixel comparisons; the name `argb`
alone does not establish the byte order of a standalone decoder's output.

The page-cache caller supplies quality 100. The Maetel wrapper changes
that value to 24 at `0xd7bb0` through `0xd7bbc`, then stores it in the
codec setup at `0xd7cb4`. The interpretation of 24, including whether it
implies lossless coding, has not been established.

## The wrapper writes two length-prefixed blocks

The writer opens a file and constructs a codec encoder. Its successful
path requests an encoded header through `0x75fe8` at `0xd7e1c`, supplies
the source image through `0x76068` at `0xd7e70`, and requests encoded
image data through `0x76028` at `0xd7e8c`.

Each output block is written when the returned length is positive:

| Output | Length write | Payload write |
| --- | --- | --- |
| Encoded header | `0xd7e50`, four bytes from the codec's length result | `0xd7e64`, that many bytes from the output buffer |
| Encoded image data | `0xd7ec0`, four bytes from the codec's length result | `0xd7ed4`, that many bytes from the output buffer |

The inspected ARM64 implementation writes native little-endian 32-bit
lengths. The corresponding buffer reader expects:

```text
i32_le header_length
bytes  encoded_header[header_length]
i32_le image_data_length
bytes  encoded_image_data[image_data_length]
```

Both lengths must be positive in that reader. This layout describes the
wrapper's framing; it does not assign fields inside either codec block.
There is no PNG or JPEG wrapper in this save branch.

The encoder's temporary output capacity is computed at `0xd7d8c`
through `0xd7db0` as:

```text
ceil(width / 16) * ceil(height / 16) * 1026 + 60
```

This is an allocation formula for the native output buffer, not a decoded
tile schema or a safe allocation rule for arbitrary untrusted dimensions.

## The buffer reader checks a marker inside the first block

The exported buffer overload of `read_maetel_argb`, `0xd7184`, requires
a nonnull input and more than five bytes. It then compares the two bytes
at input offset 4 with constants at `0xf6c38` and `0xf6c3a`:

| Constant address | Bytes | Little-endian integer |
| --- | --- | --- |
| `0xf6c38` | `aa 00` | `0x00aa` |
| `0xf6c3a` | `aa 01` | `0x01aa` |

The comparisons and accepted branches are `0xd71f4` through `0xd7218`.
These bytes belong to the encoded header, after its four-byte length.
They are not an ASCII `SPI` signature at file offset zero.

The reader loads the first length at `0xd7384`, requires it to be at least
1 and fit within the remaining input at `0xd7390` through `0xd739c`,
then passes a copy of that block to codec operation `0x5da00` at
`0xd73dc`. It rejects a negative result or a consumed-byte count that
does not equal the supplied header length, through `0xd7400`.

After obtaining the image properties, it locates the second length
immediately after the first block at `0xd78cc` through `0xd78d4`.
That length must also be positive and fit the remaining buffer. It
submits the block through the same codec operation at `0xd7920`, checks
the consumed-byte count through `0xd7940`, and conditionally requests
pixel output through `0x5da34` at `0xd7954`.

The recovered success path does not compare the end of the second block
with the end of the entire supplied buffer. It therefore does not establish
that trailing bytes are rejected. A future SDK reader needs its own
explicit bounds and trailing-data policy.

## Dimensions and color information come from codec queries

After accepting the encoded header, the wrapper calls codec query
operation `0x5d938` with these integer property IDs:

| Property ID | Wrapper use | Call site |
| --- | --- | --- |
| 201 | Width output | `0xd741c` |
| 202 | Height output | `0xd7430` |
| 413 | Encoded color-type value | `0xd7444` |

The accepted color-type values are 400, 500 and 501, checked at
`0xd7448` through `0xd745c`. These numbers are codec API values. The
[header trace](spi-header-findings.md#color-indices-differ-from-wrapper-color-type-values)
resolves the wire byte to table indices 2, 4 and 5 for those values.

The wrapper requests output color type 500 and a stride of width times
four at `0xd7478` through `0xd74d8`. That establishes a four-byte pixel
output buffer for this wrapper. It does not yet establish the complete
channel order, color conversion or compressed sample coding.

## Validation and remaining work

The Base ELF stream and APK digest were verified against the archive.
Exported symbol addresses, imported writer dispatch, marker constants,
length operations and cited instructions were checked against their
binary bytes. Disposable framing checks covered both accepted markers,
positive lengths, truncation and the wrapper's allowance for trailing
bytes. They did not invoke or emulate the native codec.

The [header trace](spi-header-findings.md) follows consumption at
`0x5da00` and property queries at `0x5d938` into concrete implementations.
Pixel output at `0x5da34` and image-data consumption remain further
targets. A real SPI payload and rendered
reference are still needed to validate a future decoder. No SPI pixel
decoding, device execution or SDK support is claimed here.
