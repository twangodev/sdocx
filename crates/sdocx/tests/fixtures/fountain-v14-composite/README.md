# Fountain V14 host composite regression

`native.json` retains 36 actual Mesa RGBA8 mask/prior/output readbacks from
`tools/fountain_v14/capture.py`. The capture script SHA-256 is recorded in the
receipt and matches the retained script. The receipt also retains original
saved-stroke input, native stream and shader hashes, native library identities,
and archival helper provenance.

The test checks the enhanced-AA MAX coverage mask followed by native source-over
color compositing for both origins, scales 1 and 2, three color/alpha values and
three verified backdrops. It compares every byte and checks the captured hashes.
Full-F32 mask sampling is a negative control with 18 differing output bytes.
This covers the captured host composite stage, not direct Drawing paint,
full-page output, or Samsung device fidelity.

The Rust regression is self-contained. Re-running the original native capture
requires the archival helper tree and original native libraries named in the
receipt; these are not distributed in the crate.
