# Rotated Arc scale controls

Run with `SCALE=0.5` and `SCALE=2`, `SCALE_FIXTURE` set to the corresponding
`crates/sdocx/tests/fixtures/native-arc-scaled-*-frame.bin`, and the same three
arguments as `tools/named_arc_rotation/replay.py`. Each run checks the patched
capture source, full dependency hashes, Unicorn, host math, five-fill compact
receipts and the exact framed bytes. Both controls retain the loaded path after
both setters. The approximation gate accepts the 0.5 scale and falls back at
2 because its strict 0.000025 guard is smaller than the observed residual.

These are bounded host Unicorn recordings with a substituted recording canvas;
Android math, device pixels and native PDF appearance are not established.
