# Stroke rendering findings

## Root cause of top-right artifacts

The former visible-stroke parser bypassed the structural page/layer/object
parser and advanced through records using offsets that happened to cancel on
common files.

- `0x79` is the low byte of the 121-byte base-frame size, not an extension
  marker.
- The value called `data_len` is the complete stroke-frame size.
- The `StartPointMinusThree` fallback begins three bytes early.
- At that shifted position, common property mask `0x25` is interpreted as a
  `u16` point count: decimal 37.
- The fallback prefers the parse containing more points, so every legitimate
  stroke shorter than 37 points can be replaced by misaligned data.

That misalignment produces the long strokes landing near the top-right corner.
It is deterministic, not a coordinate transform or SVG renderer problem.

## Correct traversal

```text
page layer collection
  -> layer object tree
  -> outer object type 1
  -> payload frame type 0 (base)
  -> payload frame type 1 (stroke)
  -> point_count at stroke frame +18
  -> point channels bounded by flexible_offset
  -> style fields selected by field mask
```

Compressed channels are stored by channel: first X/Y as `f64`, Q10.5 signed-
magnitude coordinate deltas, first pressure as `f32`, Q3.12 pressure deltas,
first timestamp as `i32`, `u16` timestamp deltas, then optional tilt and
orientation channels in the same Q3.12 form. Two tool/input bytes finish the
fixed channel block.

The full bit tables and uncompressed form are in
[`file-format.md`](file-format.md#stroke-frame).

## Implemented fix

The Rust parser now walks `StoredPage` layers and recursive object records,
then decodes type-0/type-1 frames within each stroke's payload. It no longer has
selection-by-larger-point-count or `StartPointMinusThree`. Channel presence
comes from the stroke property mask; fixed channel bytes must end exactly at
the declared flexible-data boundary. Malformed strokes return an error with
their page ID and payload offset instead of silently returning partial content.

Unknown objects remain in the stored tree, and unexposed flexible data stays
bounded by its frame. Non-stroke semantic interpretation remains best-effort.

`structural_strokes.rs` covers the synthetic format cases in normal CI.
`stroke_conformance.rs` checks all three original handwritten fixtures against
the native audit. It passes on the new decoder and fails on the old decoder's
handwritten point count (322,406 versus the expected 321,776).
