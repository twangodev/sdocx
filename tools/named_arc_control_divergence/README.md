# Saved Arc control divergence

Run from the repository root, using the native inputs and Unicorn from the
rotated Arc replay:

```sh
ALTER_OFFSET=501 ALTER_FRAME=/tmp/arc-first.bin python3 tools/named_arc_control_divergence/replay.py "$PWD" /absolute/arm64-v8a /absolute/unicorn/lib
ALTER_OFFSET=517 ALTER_FRAME=/tmp/arc-second.bin python3 tools/named_arc_control_divergence/replay.py "$PWD" /absolute/arm64-v8a /absolute/unicorn/lib
```

Each run writes a native 37-degree moved Arc, adds 10 to one serialized f64
control x coordinate, then feeds exactly the framed bytes to the native
loader. Five runs use allocation fills 0, 85, 165, 255, 0. Source dependencies,
patched capture, Unicorn, host math, compact native control, initial angle,
IsSame, paths and Drawing receipts and the mutated frame are checked. The first
control edit produces IsSame false/true; the second produces true/false.

This is bounded host Unicorn execution with a recording canvas and substituted
host math. It does not establish Android math, device pixels, or native PDF
appearance. The native float comparisons and regeneration behavior outside
these two edited controls and captured positive rectangle are not covered.

Two boundary controls also have five-fill receipts. The native first setter
retains the original path for a +0.00006 saved-x change and replaces it for
+0.0001:

```sh
ALTER_OFFSET=501 ALTER_FRAME=/tmp/arc-near.bin DELTA=0.00006 python3 tools/named_arc_control_divergence/replay.py "$PWD" /absolute/arm64-v8a /absolute/unicorn/lib
ALTER_OFFSET=501 ALTER_FRAME=/tmp/arc-outside.bin DELTA=0.0001 python3 tools/named_arc_control_divergence/replay.py "$PWD" /absolute/arm64-v8a /absolute/unicorn/lib
```

The renderer candidate uses a stricter 0.000025 coordinate margin. This is a
heuristic buffer, not a proven bound on Android or host transcendental drift.

The second saved x control has a different transition: +0.0001 retains both
paths, while +0.00015 causes the second setter to replace the path. Both are
five-fill receipt-checked:

```sh
ALTER_OFFSET=517 ALTER_FRAME=/tmp/arc-second-near.bin DELTA=0.0001 python3 tools/named_arc_control_divergence/replay.py "$PWD" /absolute/arm64-v8a /absolute/unicorn/lib
ALTER_OFFSET=517 ALTER_FRAME=/tmp/arc-second-outside.bin DELTA=0.00015 python3 tools/named_arc_control_divergence/replay.py "$PWD" /absolute/arm64-v8a /absolute/unicorn/lib
```
