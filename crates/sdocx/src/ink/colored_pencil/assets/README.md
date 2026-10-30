# ColoredPencil point atlas

`point.r8` is the native 10x30 point atlas's R channel in upload order, with three
stacked 10x10 tips. The native texture's other channels are zero. The palette,
source alpha and stamp opacity are applied by the shader, not baked into this
asset. Native upload SHA-256 and source library pins are retained in
`conformance/colored-pencil-paint-native.json` and the new canvas receipt.

R8 SHA-256: `673423c6fdae1ba344c452deefeb51b7b11365fead0bfbdd4b345eed4027f405`.
