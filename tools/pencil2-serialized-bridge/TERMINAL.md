# Pencil2 terminal opacity controls

`terminal-zero-control.py` and `terminal-variants-control.py` captured four
synthetic, setter-built Pencil2 V14 descriptors before native Update. The two
receipts record input and descriptor hashes. The crate-local `terminal-*.vbo`
fixtures match all four recorded byte lengths and SHA-256 hashes.

The Rust regression compares every descriptor byte for short, long and reversed
final segments ending at zero pressure. It separately checks the small nonzero
terminal opacity produced by accumulated binary32 interpolation. These controls
cover geometry; they do not establish GPU submission, device pixels or saved-note
parity. Frame hashes in the receipts are historical observations, not pixel
assertions in this regression.

Capture requires the original project-local native harness: `fresh-object-control.py`,
`capture.py`, `../pencil2-independent-drawing/capture.py`,
`../pencil2-mixed-drawing/capture.patch`,
`../pencil2-public-replay/drawing_native_target_translated_probe.py`, and that
probe's pinned `tmp/vector-parity-plan/pen-family-audit/pencil2-v14-geometry-v12/capture_candidate.py`
and transitive ARM64/Mesa dependencies. These local dependencies are not bundled
by this regression. The scripts retain their original absolute checkout path.
The current local `fresh-object-control.py` differs from the historical hash in
both receipts; reproducing capture requires restoring the recorded harness
version. The original receipt hashes are preserved, not updated to imply a new
capture. Both retained terminal capture scripts still match their receipt hashes.

Run the portable regression with:

```sh
cargo test -p sdocx --features render,serde --lib terminal_zero_tests
```
