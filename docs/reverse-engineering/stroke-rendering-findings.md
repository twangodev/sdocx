# Stroke rendering findings

## Root cause of top-right artifacts

The visible-stroke parser currently bypasses the structural page/layer/object
parser and advances through records using offsets that happen to cancel on
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

## Required fix

Delete selection-by-larger-point-count and `StartPointMinusThree`. Parse the
outer record and generic frames first, then decode channel presence from the
stroke property mask. Unknown fields should remain bounded by frame size and
preserved or skipped without affecting sibling records.
