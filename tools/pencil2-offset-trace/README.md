# Pencil2 grain-offset lifecycle controls

The crate fixtures `pencil2-offset-lifecycle-native.json` and
`pencil2-offset-lifecycle-sized-native.json` retain the complete native receipts
for the square and non-square controls. The test-only Rust model compares the
captured uniform bytes across start-point changes and cached repeated draws.
These synthetic ARM64-emulation/Mesa controls do not verify a production cache,
saved-note rendering, native scheduling, or device fidelity.

Run `python3 tools/pencil2-offset-trace/lifecycle-probe.py` or
`python3 tools/pencil2-offset-trace/lifecycle-sized-probe.py` in the original
capture workspace. Each writes its corresponding `*-receipt.json`; inspect
that receipt before replacing the crate fixture. These output files are temporary
recapture copies; remove them after inspection if they match the retained fixtures.
Fixture tests need only `cargo test -p sdocx --features render,serde --lib ink::pencil2::offset`.

Native recapture is not standalone: the scripts require the existing Drawing
capture harness, scratch native libraries, and temporary capture helpers listed
with SHA-256 hashes in each fixture's `dependencies`. Those local inputs are
not bundled here. Original APK provenance, emulator and transitive runtime
libraries are not fully pinned. The retained fixtures remain usable without
those native capture dependencies.
