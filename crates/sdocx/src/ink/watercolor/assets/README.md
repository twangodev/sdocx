# WaterColor RTV1 pattern

`pattern.rg8` is the 256×256 native pattern's interleaved R/G bytes in upload
order. The shader consumes these two channels; the original B/A channels are
constant zero/255. The Rust regression compares every channel against the
retained native `conformance/watercolor-pattern.png`.

- R/G asset SHA-256: `b28f0150d930aa8c95534b0fe8c70803079956632328b882bde03936fa54d2b8`
- Native PNG SHA-256: `b321ea19bf487ac9a1e8cf2d99d4e76efe3ede4fe31406558340cf2af01194b6`

The native source is `libSPenWaterColorBrush.so`'s `g_penPtn` at RVA `0x1259c`,
recorded by the pinned WaterColor acquisition harness. Production uses the raw
channels directly, avoiding PNG decoding during stroke paint.
