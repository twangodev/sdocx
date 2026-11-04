# Historical public Montblanc comparison

This retired synthetic probe selected `HostUnitScale` explicitly and compared public SVG and direct PDF exports with retained host-Mesa frames and generic output. It assumed one output pixel per page unit and a fresh drawable per stroke. The measurements in `conformance/montblanc-public-optin-host-comparison.json` do not establish saved-note, device or arbitrary-transform parity.

The native RGB was premultiplied; the comparison added the paper contribution `252 * (1 - alpha)` without multiplying RGB by alpha again. Retained frames do not establish the native fixed-function blend state. Native captures, receipts and maintained Rust tests remain; the standalone Cargo exporter and external-reader comparator have been retired.

Run `python3 tools/montblanc/compare.py` to verify the retained synthetic input, six native captures and frames, four primary triangle streams and the 2× crop controls. This native-only check does not invoke Cargo or external readers. Original script hashes in comparison receipts remain historical provenance.
